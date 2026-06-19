import { type ReactNode, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useVault } from "./state/useVault";
import { VaultUnlock } from "./components/VaultUnlock";
import { ConnectionTree } from "./components/ConnectionTree";
import { ConnectionEditor } from "./components/ConnectionEditor";
import { VncView } from "./renderers/VncView";
import type { Node } from "./lib/vaultApi";
import { colors, ghostBtn, input, primaryBtn } from "./components/styles";

type SessionInfo = { wsUrl: string; kind: string };

export default function App() {
  const v = useVault();
  const [selected, setSelected] = useState<Node | null>(null);
  const [showVnc, setShowVnc] = useState(false);

  if (v.status === "checking") {
    return <Center>A verificar o cofre…</Center>;
  }
  if (v.status === "locked") {
    return <VaultUnlock exists={v.exists} error={v.error} onUnlock={v.unlock} />;
  }

  return (
    <div
      style={{
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        background: colors.bg,
        color: colors.text,
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <header style={{ display: "flex", alignItems: "center", gap: 12, padding: "10px 14px", borderBottom: `1px solid ${colors.border}` }}>
        <strong style={{ fontSize: 15 }}>Remota</strong>
        <span style={{ color: colors.dim, fontSize: 12 }}>cofre destravado</span>
        <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
          <button style={ghostBtn} onClick={() => { setShowVnc(false); setSelected(null); }}>+ Nova conexão</button>
          <button style={ghostBtn} onClick={() => setShowVnc((s) => !s)}>{showVnc ? "← Editor" : "Testar VNC"}</button>
          <button style={ghostBtn} onClick={() => { v.lock(); setSelected(null); }}>Bloquear</button>
        </div>
      </header>

      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
        <aside style={{ width: 340, borderRight: `1px solid ${colors.border}`, overflow: "auto" }}>
          <ConnectionTree
            doc={v.tree}
            selectedId={selected?.id ?? null}
            onSelect={(n) => { setShowVnc(false); setSelected(n); }}
            onDelete={(id) => { v.remove(id); if (selected?.id === id) setSelected(null); }}
          />
        </aside>
        <main style={{ flex: 1, overflow: "auto", minWidth: 0 }}>
          {showVnc ? (
            <VncTest />
          ) : (
            <ConnectionEditor node={selected} onSave={async (n) => { await v.save(null, n); setSelected(n); }} />
          )}
        </main>
      </div>
    </div>
  );
}

function VncTest() {
  const [host, setHost] = useState("192.168.1.242:5900");
  const [password, setPassword] = useState("");
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function openVnc() {
    setError(null);
    try {
      setSession(await invoke<SessionInfo>("open_session", { target: host, kind: "vnc" }));
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div style={{ padding: 12, display: "flex", gap: 8, alignItems: "center" }}>
        <input style={input} value={host} onChange={(e) => setHost(e.target.value)} placeholder="host:porta" />
        <input
          style={{ ...input, width: 180 }}
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="senha VNC"
        />
        <button style={primaryBtn} onClick={openVnc}>Abrir VNC</button>
      </div>
      {error && <div style={{ padding: "0 12px", color: colors.danger, fontFamily: "monospace", fontSize: 12 }}>{error}</div>}
      <div style={{ flex: 1, minHeight: 0 }}>
        {session && <VncView wsUrl={session.wsUrl} password={password || undefined} />}
      </div>
    </div>
  );
}

function Center({ children }: { children: ReactNode }) {
  return (
    <div style={{ height: "100vh", display: "grid", placeItems: "center", background: colors.bg, color: colors.dim, fontFamily: "system-ui, sans-serif" }}>
      {children}
    </div>
  );
}
