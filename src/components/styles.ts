import type { CSSProperties } from "react";

export const colors = {
  bg: "#0f1115",
  panel: "#171a21",
  border: "#262a33",
  text: "#e6e6e6",
  dim: "#9aa0aa",
  accent: "#4f8cff",
  danger: "#ff6b6b",
};

export const input: CSSProperties = {
  background: "#0d0f13",
  color: colors.text,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: "8px 10px",
  fontSize: 13,
  outline: "none",
  width: "100%",
  boxSizing: "border-box",
};

export const primaryBtn: CSSProperties = {
  background: colors.accent,
  color: "#fff",
  border: "none",
  borderRadius: 6,
  padding: "8px 12px",
  fontSize: 13,
  cursor: "pointer",
};

export const ghostBtn: CSSProperties = {
  background: "transparent",
  color: colors.dim,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: "6px 10px",
  fontSize: 12,
  cursor: "pointer",
};

export const label: CSSProperties = {
  fontSize: 11,
  color: colors.dim,
  textTransform: "uppercase",
  letterSpacing: 0.4,
};
