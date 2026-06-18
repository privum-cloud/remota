import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { VncView } from "./renderers/VncView";

type SessionInfo = { wsUrl: string; kind: string };

export default function App() {
  // Host real do homelab para validação E2E (ex.: x11vnc do groot).
  const [host, setHost] = useState("192.168.1.242:5900");
  const [password, setPassword] = useState("");
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function openVnc() {
    setError(null);
    try {
      const info = await invoke<SessionInfo>("open_session", {
        target: host,
        kind: "vnc",
      });
      setSession(info);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      <div style={{ padding: 8, display: "flex", gap: 8, alignItems: "center" }}>
        <input
          value={host}
          onChange={(e) => setHost(e.target.value)}
          placeholder="host:porta"
          style={{ flex: 1 }}
        />
        <input
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="senha VNC (opcional)"
          type="password"
        />
        <button onClick={openVnc}>Abrir VNC</button>
      </div>
      {error && (
        <div style={{ padding: 8, color: "#c00", fontFamily: "monospace" }}>{error}</div>
      )}
      <div style={{ flex: 1, minHeight: 0 }}>
        {session && <VncView wsUrl={session.wsUrl} password={password || undefined} />}
      </div>
    </main>
  );
}
