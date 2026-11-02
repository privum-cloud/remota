import { type FormEvent, useState } from "react";
import { colors, ghostBtn, input, primaryBtn } from "./styles";

type Props = {
  title: string;
  onSubmit: (password: string) => void;
  onCancel: () => void;
};

/** Set-a-password modal for the encrypted export. The password is independent of the master
 *  password; it protects only the exported file. Submit stays disabled until the two fields
 *  are non-empty and match, so an empty or mistyped password can't seal the file. */
export function SetPasswordPrompt({ title, onSubmit, onCancel }: Props) {
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const ok = pw.length > 0 && pw === confirm;

  function submit(e: FormEvent) {
    e.preventDefault();
    if (ok) onSubmit(pw);
  }

  return (
    <div
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
      style={{
        position: "fixed",
        inset: 0,
        display: "grid",
        placeItems: "center",
        background: "rgba(0,0,0,0.55)",
        zIndex: 1000,
      }}
    >
      <form
        onSubmit={submit}
        style={{
          width: 340,
          display: "flex",
          flexDirection: "column",
          gap: 12,
          padding: 20,
          background: colors.bg,
          border: `1px solid ${colors.border}`,
          borderRadius: 10,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
      >
        <div style={{ fontSize: 14, fontWeight: 600, color: colors.text }}>{title}</div>
        <input
          type="password"
          autoFocus
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          placeholder="password"
          style={input}
        />
        <input
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          placeholder="confirm password"
          style={input}
        />
        {confirm.length > 0 && pw !== confirm && (
          <div style={{ fontSize: 11, color: "#f85149" }}>Passwords don't match.</div>
        )}
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <button type="button" style={ghostBtn} onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" disabled={!ok} style={{ ...primaryBtn, opacity: ok ? 1 : 0.6 }}>
            Export ▸
          </button>
        </div>
      </form>
    </div>
  );
}
