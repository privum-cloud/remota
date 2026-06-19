import { type ReactNode, useEffect, useState } from "react";
import type { Node } from "../lib/vaultApi";
import { colors, input, label, primaryBtn } from "./styles";
import { GatewaySection, type GwForm, emptyGw, gwFromModel, gwToModel } from "./GatewaySection";

export function FolderEditor({ node, onSave }: { node: Node | null; onSave: (n: Node) => Promise<void> }) {
  const editingId = node?.type === "folder" ? node.id : null;
  const existingChildren = node?.type === "folder" ? node.children : [];
  const [name, setName] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [domain, setDomain] = useState("");
  const [gw, setGw] = useState<GwForm>(emptyGw);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const f = node?.type === "folder" ? node : null;
    setName(f?.name ?? "");
    setUsername(f?.defaults.username ?? "");
    setPassword(f?.defaults.password ?? "");
    setDomain(f?.defaults.domain ?? "");
    setGw(gwFromModel(f?.gateway));
    setSaved(false);
  }, [node]);

  async function submit(ev: { preventDefault: () => void }) {
    ev.preventDefault();
    if (!name) return;
    setBusy(true);
    try {
      const built: Node = {
        type: "folder",
        id: editingId ?? crypto.randomUUID(),
        name,
        defaults: {
          username: username || undefined,
          password: password || undefined,
          domain: domain || undefined,
        },
        gateway: gwToModel(gw),
        children: existingChildren, // preserva a subárvore ao editar
      };
      await onSave(built);
      setSaved(true);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16, maxWidth: 420 }}>
      <h2 style={{ margin: 0, fontSize: 15, color: colors.text }}>{editingId ? "Edit folder" : "New folder"}</h2>
      <Field label="Name">
        <input style={input} value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Production" />
      </Field>
      <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
        <div style={{ ...label, marginBottom: 8 }}>Defaults inherited by children</div>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <input style={input} value={username} onChange={(e) => setUsername(e.target.value)} placeholder="username" />
          <input style={input} type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="password" />
          <input style={input} value={domain} onChange={(e) => setDomain(e.target.value)} placeholder="domain" />
        </div>
      </div>
      <GatewaySection value={gw} onChange={setGw} hint="All connections inside this folder go out through this jump host (unless they define their own)." />

      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <button type="submit" disabled={busy || !name} style={{ ...primaryBtn, opacity: busy || !name ? 0.6 : 1 }}>
          {busy ? "…" : "Save"}
        </button>
        {saved && <span style={{ color: "#7ee787", fontSize: 12 }}>Saved ✓</span>}
      </div>
    </form>
  );
}

function Field({ label: text, children }: { label: string; children: ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span style={label}>{text}</span>
      {children}
    </div>
  );
}
