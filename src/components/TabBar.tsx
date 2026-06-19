import { type CSSProperties, useState } from "react";
import type { SessionTab } from "../state/useSessions";
import { colors } from "./styles";

const protoColor: Record<string, string> = { ssh: "#7ee787", rdp: "#79c0ff", vnc: "#ffa657", telnet: "#d2a8ff" };

type Props = {
  tabs: SessionTab[];
  activeId: string; // "editor" | id da sessão
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onReconnect: (id: string) => void;
  onDuplicate: (id: string) => void;
};

export function TabBar({ tabs, activeId, onActivate, onClose, onReconnect, onDuplicate }: Props) {
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null);

  return (
    <div style={barStyle}>
      <div onClick={() => onActivate("editor")} style={tabStyle(activeId === "editor")}>
        ✎ Editor
      </div>
      {tabs.map((t) => (
        <div
          key={t.id}
          onClick={() => onActivate(t.id)}
          onContextMenu={(e) => { e.preventDefault(); setMenu({ id: t.id, x: e.clientX, y: e.clientY }); }}
          style={tabStyle(activeId === t.id)}
          title={`${t.protocol.toUpperCase()} — ${t.target}`}
        >
          <span style={{ width: 7, height: 7, borderRadius: 4, flex: "0 0 auto", background: t.error ? colors.danger : protoColor[t.protocol] ?? colors.dim }} />
          <span style={{ maxWidth: 150, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{t.title}</span>
          <button onClick={(e) => { e.stopPropagation(); onClose(t.id); }} style={closeBtn} title="Close">
            ×
          </button>
        </div>
      ))}
      {menu && (
        <Menu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Reconnect", fn: () => onReconnect(menu.id) },
            { label: "Duplicate tab", fn: () => onDuplicate(menu.id) },
            { label: "Close", fn: () => onClose(menu.id) },
          ]}
        />
      )}
    </div>
  );
}

function Menu({ x, y, items, onClose }: { x: number; y: number; items: { label: string; fn: () => void }[]; onClose: () => void }) {
  return (
    <>
      <div
        onClick={onClose}
        onContextMenu={(e) => { e.preventDefault(); onClose(); }}
        style={{ position: "fixed", inset: 0, zIndex: 50 }}
      />
      <div
        style={{
          position: "fixed",
          top: y,
          left: x,
          zIndex: 51,
          background: colors.panel,
          border: `1px solid ${colors.border}`,
          borderRadius: 6,
          padding: 4,
          minWidth: 150,
          boxShadow: "0 6px 20px rgba(0,0,0,0.4)",
        }}
      >
        {items.map((it) => (
          <div key={it.label} onClick={() => { it.fn(); onClose(); }} style={menuItem}>
            {it.label}
          </div>
        ))}
      </div>
    </>
  );
}

const barStyle: CSSProperties = {
  display: "flex",
  alignItems: "stretch",
  borderBottom: `1px solid ${colors.border}`,
  background: colors.panel,
  height: 36,
  overflowX: "auto",
};

function tabStyle(active: boolean): CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 8,
    padding: "0 12px",
    fontSize: 13,
    cursor: "pointer",
    color: active ? colors.text : colors.dim,
    background: active ? colors.bg : "transparent",
    borderRight: `1px solid ${colors.border}`,
    borderBottom: active ? `2px solid ${colors.accent}` : "2px solid transparent",
    userSelect: "none",
    whiteSpace: "nowrap",
  };
}

const closeBtn: CSSProperties = {
  background: "transparent",
  border: "none",
  color: colors.dim,
  cursor: "pointer",
  fontSize: 14,
  lineHeight: 1,
  padding: 0,
};

const menuItem: CSSProperties = { padding: "6px 10px", fontSize: 13, color: colors.text, cursor: "pointer", borderRadius: 4 };
