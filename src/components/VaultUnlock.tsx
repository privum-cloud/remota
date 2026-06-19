import { type FormEvent, useState } from "react";
import { colors, input, primaryBtn } from "./styles";

type Props = {
  exists: boolean;
  error: string | null;
  onUnlock: (password: string) => Promise<void>;
};

export function VaultUnlock({ exists, error, onUnlock }: Props) {
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!pw) return;
    setBusy(true);
    try {
      await onUnlock(pw);
    } catch {
      // erro exibido via prop `error`
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        height: "100vh",
        display: "grid",
        placeItems: "center",
        background: colors.bg,
        color: colors.text,
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <form onSubmit={submit} style={{ width: 320, display: "flex", flexDirection: "column", gap: 12 }}>
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 700, letterSpacing: 0.5 }}>Remota</h1>
        <p style={{ margin: 0, color: colors.dim, fontSize: 13, lineHeight: 1.4 }}>
          {exists
            ? "Cofre encontrado. Introduz a senha mestra para destravar."
            : "Primeiro uso — define a senha mestra que vai cifrar as tuas conexões."}
        </p>
        <input
          type="password"
          autoFocus
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          placeholder={exists ? "Senha mestra" : "Nova senha mestra"}
          style={input}
        />
        <button type="submit" disabled={busy || !pw} style={{ ...primaryBtn, opacity: busy || !pw ? 0.6 : 1 }}>
          {busy ? "…" : exists ? "Destravar" : "Criar cofre"}
        </button>
        {error && <div style={{ color: colors.danger, fontSize: 12 }}>{error}</div>}
      </form>
    </div>
  );
}
