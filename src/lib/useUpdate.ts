import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

import { vaultApi, type UpdatePolicy } from "./vaultApi";

/**
 * Where the update conversation has got to.
 *
 * `quiet` covers every uninteresting case at once — no newer version, checking turned off, the
 * machine offline — because they all mean the same thing to the person looking at the window:
 * nothing to do.
 */
export type UpdatePhase =
  | { name: "quiet" }
  | { name: "available"; version: string }
  | { name: "installing"; percent: number | null }
  | { name: "installed" }
  | { name: "failed"; message: string };

export function useUpdate() {
  const [policy, setPolicy] = useState<UpdatePolicy | null>(null);
  const [phase, setPhase] = useState<UpdatePhase>({ name: "quiet" });
  // Held from the check so installing does not have to ask again.
  const pending = useRef<Update | null>(null);

  const readPolicy = useCallback(async () => {
    const next = await vaultApi.updatePolicy();
    setPolicy(next);
    return next;
  }, []);

  const look = useCallback(async () => {
    try {
      const found = await check();
      if (!found) return false;
      pending.current = found;
      setPhase({ name: "available", version: found.version });
      return true;
    } catch {
      // Offline, GitHub having a bad minute, a manifest not published yet. None of it is the
      // user's problem, and none of it should interrupt someone who opened Remota to reach a
      // server.
      return false;
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      let current: UpdatePolicy;
      try {
        current = await readPolicy();
      } catch {
        // Without the policy we do not know whether we are allowed to look, and the safe reading
        // of that is: do not.
        return;
      }
      if (!current.enabled || cancelled) return;
      await look();
    })();
    return () => {
      cancelled = true;
    };
  }, [readPolicy, look]);

  /** Download and install, then restart into the new version. */
  const install = useCallback(async () => {
    const found = pending.current;
    if (!found) return;

    setPhase({ name: "installing", percent: null });
    let total = 0;
    let seen = 0;
    try {
      await found.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          setPhase({ name: "installing", percent: total ? 0 : null });
        } else if (event.event === "Progress") {
          seen += event.data.chunkLength;
          setPhase({
            name: "installing",
            percent: total ? Math.min(100, Math.round((seen / total) * 100)) : null,
          });
        } else if (event.event === "Finished") {
          setPhase({ name: "installed" });
        }
      });
      setPhase({ name: "installed" });
      await relaunch();
    } catch (e: unknown) {
      // A cancelled administrator prompt lands here too, which is why the failure always offers
      // the download page rather than only an apology.
      setPhase({ name: "failed", message: String(e) });
    }
  }, []);

  const dismiss = useCallback(() => setPhase({ name: "quiet" }), []);

  /** Menu-driven check. Returns whether anything was found, so the caller can say "up to date". */
  const checkNow = useCallback(async () => look(), [look]);

  const setEnabled = useCallback(
    async (enabled: boolean) => {
      await vaultApi.setUpdateCheck(enabled);
      await readPolicy();
    },
    [readPolicy],
  );

  return { policy, phase, install, dismiss, setEnabled, checkNow };
}
