import { type CSSProperties } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import type { UpdateDelivery } from "../lib/vaultApi";
import type { UpdatePhase } from "../lib/useUpdate";
import { colors, ghostBtn, primaryBtn } from "./styles";

interface Props {
  phase: UpdatePhase;
  delivery: UpdateDelivery;
  releasesUrl: string;
  onInstall: () => void;
  onDismiss: () => void;
}

export function UpdateBanner({ phase, delivery, releasesUrl, onInstall, onDismiss }: Props) {
  if (phase.name === "quiet") return null;

  const openReleases = () => void openUrl(releasesUrl);

  if (phase.name === "installing") {
    return (
      <div style={bar} role="status">
        <span style={text}>
          {phase.percent === null ? "Downloading…" : `Downloading… ${phase.percent}%`}
        </span>
      </div>
    );
  }

  if (phase.name === "installed") {
    return (
      <div style={bar} role="status">
        <span style={text}>Installed. Restarting…</span>
      </div>
    );
  }

  if (phase.name === "failed") {
    return (
      <div style={{ ...bar, borderBottom: `1px solid ${colors.danger}` }} role="alert">
        <span style={text}>The update could not be installed. {phase.message}</span>
        <button style={{ ...ghostBtn, marginLeft: "auto" }} onClick={openReleases}>
          Download it
        </button>
        <button style={ghostBtn} onClick={onDismiss}>
          Not now
        </button>
      </div>
    );
  }

  return (
    <div style={bar} role="status">
      <span style={text}>
        Remota {phase.version} is out.
        {delivery === "needs_admin" &&
          // Said before the click, not after. Someone surprised by a root password prompt learns
          // to type root passwords into surprises — and this app manages bastion credentials.
          " Your system will ask for an administrator password, because your package manager owns this installation."}
      </span>
      <button style={{ ...primaryBtn, marginLeft: "auto" }} onClick={onInstall}>
        Update
      </button>
      <button style={ghostBtn} onClick={onDismiss}>
        Not now
      </button>
    </div>
  );
}

const bar: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "8px 12px",
  background: colors.panel,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};
const text: CSSProperties = { fontSize: 13, color: colors.text };
