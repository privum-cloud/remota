import type { CSSProperties } from "react";
import { colors } from "./styles";

export type CtxItem = { label: string; onClick: () => void } | "sep";

export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: CtxItem[]; onClose: () => void }) {
  return (
    <>
      <div
        onClick={onClose}
        onContextMenu={(e) => { e.preventDefault(); onClose(); }}
        style={{ position: "fixed", inset: 0, zIndex: 60 }}
      />
      <div style={{ ...box, top: y, left: x }}>
        {items.map((it, i) =>
          it === "sep" ? (
            <div key={i} style={{ height: 1, background: colors.border, margin: "4px 6px" }} />
          ) : (
            <div
              key={i}
              onClick={() => { onClose(); it.onClick(); }}
              style={item}
              onMouseEnter={(e) => (e.currentTarget.style.background = "#222834")}
              onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
            >
              {it.label}
            </div>
          ),
        )}
      </div>
    </>
  );
}

const box: CSSProperties = {
  position: "fixed",
  zIndex: 61,
  background: colors.panel,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: 4,
  minWidth: 190,
  boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
};
const item: CSSProperties = { padding: "7px 12px", cursor: "pointer", color: colors.text, borderRadius: 4, fontSize: 13 };
