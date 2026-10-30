import { type FormEvent, useState } from "react";
import { colors, ghostBtn, input, primaryBtn } from "./styles";

type Props = {
  name: string;
  onSubmit: (password: string) => void;
  onCancel: () => void;
};

/** Connect-time password prompt for connections with `always_prompt`. The typed
 *  value lives only in the session tab's memory; it is never persisted. An empty
 *  submit is allowed — key/agent auth may not need a password. */
export function PasswordPrompt({ name, onSubmit, onCancel }: Props) {
  const [pw, setPw] = useState("");

  function submit(e: FormEvent) {
    e.preventDefault();
    onSubmit(pw);
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
        <div style={{ fontSize: 14, fontWeight: 600, color: colors.text }}>
          Password for {name}
        </div>
        <input
          type="password"
          autoFocus
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          placeholder="password"
          style={input}
        />
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <button type="button" style={ghostBtn} onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" style={primaryBtn}>
            Connect ▸
          </button>
        </div>
      </form>
    </div>
  );
}
