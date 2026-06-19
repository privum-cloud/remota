import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { Node, Protocol } from "../lib/vaultApi";
import { colors, input, label, primaryBtn } from "./styles";

const PROTOCOLS: Protocol[] = ["ssh", "rdp", "vnc", "telnet"];
const DEFAULT_PORT: Record<Protocol, number> = { ssh: 22, rdp: 3389, vnc: 5900, telnet: 23 };

type Props = {
  node: Node | null;
  onSave: (node: Node) => Promise<void>;
};

export function ConnectionEditor({ node, onSave }: Props) {
  const editingId = node?.type === "connection" ? node.id : null;
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState<Protocol>("ssh");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [domain, setDomain] = useState("");
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
          },
        },
      };
      await onSave(built);
      setSaved(true);
    } finally {
      setBusy(false);
    }
  }

  const isRdp = protocol === "rdp";

  return (
    <form onSubmit={submit} style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, maxWidth: 420 }}>
      <h2 style={{ margin: 0, fontSize: 15, color: colors.text }}>
        {editingId ? "Editar conexão" : "Nova conexão"}
      </h2>

      <Field label="Nome">
        <input style={input} value={name} onChange={(e) => setName(e.target.value)} placeholder="ex.: web-prod" />
      </Field>

      <div style={{ display: "flex", gap: 10 }}>
        <Field label="Protocolo" style={{ flex: 1 }}>
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
        <Field label="Porta" style={{ width: 110 }}>
          <input
            style={input}
            value={port}
            onChange={(e) => setPort(e.target.value.replace(/[^0-9]/g, ""))}
            placeholder={String(DEFAULT_PORT[protocol])}
          />
        </Field>
      </div>

      <Field label="Host">
        <input style={input} value={host} onChange={(e) => setHost(e.target.value)} placeholder="ex.: 192.168.1.10" />
      </Field>

      <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
        <div style={{ ...label, marginBottom: 8 }}>Credenciais (vazio = herda da pasta)</div>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <input style={input} value={username} onChange={(e) => setUsername(e.target.value)} placeholder="utilizador" />
          <input
            style={input}
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="senha"
          />
          {isRdp && (
            <input style={input} value={domain} onChange={(e) => setDomain(e.target.value)} placeholder="domínio (RDP)" />
          )}
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <button type="submit" disabled={busy || !name || !host} style={{ ...primaryBtn, opacity: busy || !name || !host ? 0.6 : 1 }}>
          {busy ? "…" : "Guardar"}
        </button>
        {saved && <span style={{ color: "#7ee787", fontSize: 12 }}>Guardado ✓</span>}
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
