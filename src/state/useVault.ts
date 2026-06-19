import { useCallback, useEffect, useState } from "react";
import { type Document, type Node, vaultApi } from "../lib/vaultApi";

export type VaultStatus = "checking" | "locked" | "unlocked";

/** Estado central do cofre + ações, sobre os comandos Tauri. */
export function useVault() {
  const [status, setStatus] = useState<VaultStatus>("checking");
  const [exists, setExists] = useState(false);
  const [tree, setTree] = useState<Document>({ nodes: [] });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    vaultApi
      .exists()
      .then((e) => {
        setExists(e);
        setStatus("locked");
      })
      .catch((err) => setError(String(err)));
  }, []);

  const refresh = useCallback(async () => {
    setTree(await vaultApi.listTree());
  }, []);

  const unlock = useCallback(async (password: string) => {
    setError(null);
    try {
      await vaultApi.unlock(password);
      setTree(await vaultApi.listTree());
      setExists(true);
      setStatus("unlocked");
    } catch (e) {
      setError(String(e));
      throw e;
    }
  }, []);

  const lock = useCallback(async () => {
    await vaultApi.lock();
    setTree({ nodes: [] });
    setStatus("locked");
  }, []);

  const save = useCallback(
    async (parentId: string | null, node: Node) => {
      await vaultApi.saveConnection(parentId, node);
      await refresh();
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await vaultApi.deleteNode(id);
      await refresh();
    },
    [refresh],
  );

  return { status, exists, tree, error, unlock, lock, refresh, save, remove };
}
