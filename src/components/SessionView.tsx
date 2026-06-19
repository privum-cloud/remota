import type { SessionTab } from "../state/useSessions";
import { VncView } from "../renderers/VncView";
import { SshView } from "../renderers/SshView";
import { RdpView } from "../renderers/RdpView";
import { colors } from "./styles";

export function SessionView({ tab }: { tab: SessionTab }) {
  if (tab.error) {
    return (
      <div style={{ padding: 20 }}>
        <span style={{ color: colors.danger, fontFamily: "monospace", fontSize: 12 }}>
          Failed to open session: {tab.error}
        </span>
      </div>
    );
  }

  if (tab.protocol === "vnc") {
    // key={epoch} força remount no reconnect (token é de uso único).
    return <VncView key={tab.epoch} wsUrl={tab.wsUrl} password={tab.password} />;
  }

  if (tab.protocol === "ssh") {
    return <SshView key={tab.epoch} wsUrl={tab.wsUrl} />;
  }

  if (tab.protocol === "rdp") {
    return (
      <div style={{ width: "100%", height: "100%", display: "flex", flexDirection: "column" }}>
        <div style={{ padding: "6px 10px", fontSize: 12, color: colors.dim, background: "#1b2330", borderBottom: `1px solid ${colors.border}` }}>
          RDP: ironrdp-web renderer wired. RDCleanPath proxy on the gateway is still pending (spike T6 — validate against Windows NLA).
        </div>
        <div style={{ flex: 1, minHeight: 0 }}>
          <RdpView
            key={tab.epoch}
            proxyUrl={tab.wsUrl}
            destination={tab.target}
            username={tab.username ?? ""}
            password={tab.password ?? ""}
            domain={tab.domain}
          />
        </div>
      </div>
    );
  }

  return (
    <div style={{ padding: 20 }}>
      <p style={{ color: colors.text, fontSize: 14, margin: "0 0 8px" }}>
        <b>{tab.protocol.toUpperCase()}</b> session open on the gateway ✓
      </p>
      <p style={{ color: colors.dim, fontSize: 13, margin: "0 0 10px", maxWidth: 460 }}>
        The in-UI renderer is coming — Telnet via xterm.js. The transport (gateway + session token) already works.
      </p>
      <p style={{ color: colors.dim, fontFamily: "monospace", fontSize: 11 }}>
        {tab.target} · {tab.wsUrl}
      </p>
    </div>
  );
}
