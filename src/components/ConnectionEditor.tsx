import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { Node, Protocol } from "../lib/vaultApi";
import { colors, ghostBtn, input, label, primaryBtn } from "./styles";
import { GatewaySection, type GwForm, emptyGw, gwFromModel, gwToModel } from "./GatewaySection";
import { open } from "@tauri-apps/plugin-dialog";

const PROTOCOLS: Protocol[] = ["ssh", "rdp", "vnc", "telnet"];
const DEFAULT_PORT: Record<Protocol, number> = { ssh: 22, rdp: 3389, vnc: 5900, telnet: 23 };

type Props = {
  node: Node | null;
  onSave: (node: Node) => Promise<void>;
  onConnect?: (id: string) => void;
};

export function ConnectionEditor({ node, onSave, onConnect }: Props) {
  const editingId = node?.type === "connection" ? node.id : null;
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState<Protocol>("ssh");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [domain, setDomain] = useState("");
  const [keyPath, setKeyPath] = useState("");
  const [gw, setGw] = useState<GwForm>(emptyGw);
  const [relayUrl, setRelayUrl] = useState("");
  const [relayAgentId, setRelayAgentId] = useState("");
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const e = node?.type === "connection" ? node : null;
    setName(e?.name ?? "");
    setProtocol(e?.conn.protocol ?? "ssh");
    setHost(e?.conn.host ?? "");
    setPort(e?.conn.port ? String(e.conn.port) : "");
    setUsername(e?.conn.credentials.username ?? "");
    setPassword(e?.conn.credentials.password ?? "");
    setDomain(e?.conn.credentials.domain ?? "");
    setKeyPath(e?.conn.credentials.key_path ?? "");
    setGw(gwFromModel(e?.conn.gateway));
    setRelayUrl(e?.conn.relay?.url ?? "");
    setRelayAgentId(e?.conn.relay?.agent_id ?? "");
    setSaved(false);
  }, [node]);

  async function submit(ev: { preventDefault: () => void }) {
    ev.preventDefault();
    if (!name || !host) return;
    setBusy(true);
    try {
      const built: Node = {
        type: "connection",
        id: editingId ?? crypto.randomUUID(),
        name,
        conn: {
          protocol,
          host,
          port: port ? Number(port) : undefined,
          credentials: {
            username: username || undefined,
            password: password || undefined,
            domain: domain || undefined,
            key_path: keyPath || undefined,
          },
          gateway: gwToModel(gw),
          relay:
            relayUrl.trim() && relayAgentId.trim()
              ? { url: relayUrl.trim(), agent_id: relayAgentId.trim() }
              : undefined,
        },
      };
      await onSave(built);
      setSaved(true);
    } finally {
      setBusy(false);
    }
  }

  async function browseKey() {
    const p = await open({ multiple: false, title: "Select SSH private key" });
    if (typeof p === "string") setKeyPath(p);
  }

  const isRdp = protocol === "rdp";

  return (
    <form onSubmit={submit} style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, maxWidth: 420 }}>
      <h2 style={{ margin: 0, fontSize: 15, color: colors.text }}>
        {editingId ? "Edit connection" : "New connection"}
      </h2>

      <Field label="Name">
        <input style={input} value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. web-prod" />
      </Field>

      <div style={{ display: "flex", gap: 10 }}>
        <Field label="Protocol" style={{ flex: 1 }}>
          <select
            style={{ ...input, cursor: "pointer" }}
            value={protocol}
            onChange={(e) => setProtocol(e.target.value as Protocol)}
          >
            {PROTOCOLS.map((p) => (
              <option key={p} value={p}>
                {p.toUpperCase()}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Port" style={{ width: 110 }}>
          <input
            style={input}
            value={port}
            onChange={(e) => setPort(e.target.value.replace(/[^0-9]/g, ""))}
            placeholder={String(DEFAULT_PORT[protocol])}
          />
        </Field>
      </div>

      <Field label="Host">
        <input style={input} value={host} onChange={(e) => setHost(e.target.value)} placeholder="e.g. 192.168.1.10" />
      </Field>

      <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
        <div style={{ ...label, marginBottom: 8 }}>Credentials (empty = inherit from folder)</div>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <input style={input} value={username} onChange={(e) => setUsername(e.target.value)} placeholder="username" />
          <input
            style={input}
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="password"
          />
          {isRdp && (
            <input style={input} value={domain} onChange={(e) => setDomain(e.target.value)} placeholder="domain (RDP)" />
          )}
          <div style={{ display: "flex", gap: 8 }}>
            <input style={input} value={keyPath} onChange={(e) => setKeyPath(e.target.value)} placeholder="SSH private key (path, optional)" />
            <button type="button" style={ghostBtn} onClick={browseKey} title="Pick a private key file">Browse…</button>
          </div>
        </div>
      </div>

      <GatewaySection value={gw} onChange={setGw} />

      <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
        <div style={{ ...label, marginBottom: 8 }}>Relay (NAT traversal, optional)</div>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <input
            style={input}
            value={relayUrl}
            onChange={(e) => setRelayUrl(e.target.value)}
            placeholder="relay URL (e.g. wss://relay.privum.cloud)"
          />
          <input
            style={input}
            value={relayAgentId}
            onChange={(e) => setRelayAgentId(e.target.value)}
            placeholder="agent id (registered on the relay)"
          />
        </div>
        <div style={{ fontSize: 11, color: "#8b949e", marginTop: 6 }}>
          SSH only for now. Set both to reach this host through the agent (behind NAT). Host above
          is the target as the agent sees it — use 127.0.0.1 for the agent machine itself.
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <button type="submit" disabled={busy || !name || !host} style={{ ...primaryBtn, opacity: busy || !name || !host ? 0.6 : 1 }}>
          {busy ? "…" : "Save"}
        </button>
        {editingId && onConnect && (
          <button
            type="button"
            onClick={() => onConnect(editingId)}
            style={{ ...primaryBtn, background: "#2ea043" }}
            title="Open session (same as double-click in the tree)"
          >
            Connect ▸
          </button>
        )}
        {saved && <span style={{ color: "#7ee787", fontSize: 12 }}>Saved ✓</span>}
      </div>
    </form>
  );
}

function Field({ label: text, children, style }: { label: string; children: ReactNode; style?: CSSProperties }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4, ...style }}>
      <span style={label}>{text}</span>
      {children}
    </div>
  );
}
