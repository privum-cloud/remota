import { type ReactNode, useEffect, useState } from "react";
import type { Node } from "../lib/vaultApi";
import { colors, input, label, primaryBtn } from "./styles";

export function FolderEditor({ node, onSave }: { node: Node | null; onSave: (n: Node) => Promise<void> }) {
  const editingId = node?.type === "folder" ? node.id : null;
  const existingChildren = node?.type === "folder" ? node.children : [];
  const [name, setName] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [domain, setDomain] = useState("");
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const f = node?.type === "folder" ? node : null;
    setName(f?.name ?? "");
    setUsername(f?.defaults.username ?? "");
    setPassword(f?.defaults.password ?? "");
    setDomain(f?.defaults.domain ?? "");
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
      <h2 style={{ margin: 0, fontSize: 15, color: colors.text }}>{editingId ? "Editar pasta" : "Nova pasta"}</h2>
      <Field label="Nome">
        <input style={input} value={name} onChange={(e) => setName(e.target.value)} placeholder="ex.: Produção" />
      </Field>
      <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
        <div style={{ ...label, marginBottom: 8 }}>Defaults herdados pelos filhos</div>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <input style={input} value={username} onChange={(e) => setUsername(e.target.value)} placeholder="utilizador" />
          <input style={input} type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="senha" />
          <input style={input} value={domain} onChange={(e) => setDomain(e.target.value)} placeholder="domínio" />
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <button type="submit" disabled={busy || !name} style={{ ...primaryBtn, opacity: busy || !name ? 0.6 : 1 }}>
          {busy ? "…" : "Guardar"}
        </button>
        {saved && <span style={{ color: "#7ee787", fontSize: 12 }}>Guardado ✓</span>}
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
