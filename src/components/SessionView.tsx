import type { SessionTab } from "../state/useSessions";
import { VncView } from "../renderers/VncView";
import { SshView } from "../renderers/SshView";
import { colors } from "./styles";

export function SessionView({ tab, onDead }: { tab: SessionTab; onDead?: () => void }) {
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
    return <VncView key={tab.epoch} wsUrl={tab.wsUrl} password={tab.password} onClosed={onDead} />;
  }

  if (tab.protocol === "ssh") {
    return <SshView key={tab.epoch} wsUrl={tab.wsUrl} onClosed={onDead} />;
  }

  if (tab.protocol === "rdp") {
    return (
      <div style={{ padding: 24, maxWidth: 580 }}>
        <p style={{ color: colors.text, fontSize: 15, margin: "0 0 10px" }}>
          <b>RDP is not available yet</b>
        </p>
        <p style={{ color: colors.dim, fontSize: 13, lineHeight: 1.6, margin: "0 0 12px" }}>
          Remota's RDP support is still in development. The in-app renderer (IronRDP) is wired up,
          but the gateway side that performs the RDP <b>NLA / CredSSP</b> handshake to the Windows
          host isn't finished yet — so RDP sessions can't open. This is on the roadmap.
        </p>
        <p style={{ color: colors.dim, fontSize: 13, lineHeight: 1.6, margin: 0 }}>
          <b>SSH works today.</b> RDP, VNC and Telnet share the same architecture and are being
          rolled out — follow or track progress at{" "}
          <span style={{ fontFamily: "monospace", color: colors.text }}>github.com/privum-cloud/remota</span>.
        </p>
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
