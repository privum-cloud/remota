import { type CSSProperties, useState } from "react";
import { colors } from "./styles";

export type MenuItem = { label: string; onClick: () => void } | "sep";
export type Menu = { title: string; items: MenuItem[] };

export function MenuBar({ menus }: { menus: Menu[] }) {
  const [open, setOpen] = useState<string | null>(null);

  return (
    <div style={bar}>
      {menus.map((m) => (
        <div key={m.title} style={{ position: "relative" }}>
          <div
            onClick={() => setOpen((o) => (o === m.title ? null : m.title))}
            onMouseEnter={() => setOpen((o) => (o !== null ? m.title : o))}
            style={{ ...itemStyle, background: open === m.title ? colors.panel : "transparent" }}
          >
            {m.title}
          </div>
          {open === m.title && (
            <>
              <div onClick={() => setOpen(null)} style={{ position: "fixed", inset: 0, zIndex: 40 }} />
              <div style={dropdown}>
                {m.items.map((it, i) =>
                  it === "sep" ? (
                    <div key={i} style={{ height: 1, background: colors.border, margin: "4px 6px" }} />
                  ) : (
                    <div
                      key={i}
                      onClick={() => { setOpen(null); it.onClick(); }}
                      style={menuItemStyle}
                      onMouseEnter={(e) => (e.currentTarget.style.background = "#222834")}
                      onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                    >
                      {it.label}
                    </div>
                  ),
                )}
              </div>
            </>
          )}
        </div>
      ))}
    </div>
  );
}

const bar: CSSProperties = {
  display: "flex",
  alignItems: "stretch",
  height: 30,
  background: "#12141a",
  borderBottom: `1px solid ${colors.border}`,
  fontSize: 13,
  userSelect: "none",
};
const itemStyle: CSSProperties = { display: "flex", alignItems: "center", padding: "0 12px", cursor: "pointer", color: colors.text };
const dropdown: CSSProperties = {
  position: "absolute",
  top: 30,
  left: 0,
  zIndex: 41,
  background: colors.panel,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: 4,
  minWidth: 260,
  boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
};
const menuItemStyle: CSSProperties = { padding: "7px 12px", cursor: "pointer", color: colors.text, borderRadius: 4, fontSize: 13 };
