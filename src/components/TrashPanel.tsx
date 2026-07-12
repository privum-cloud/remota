import { type CSSProperties, useState } from "react";
import type { TrashEntry } from "../lib/vaultApi";
import { colors } from "./styles";

type Props = {
  trash: TrashEntry[];
  onRestore: (id: string) => void;
  onDeleteForever: (id: string) => void;
  onEmpty: () => void;
};

/** Lixeira colapsável no fundo da sidebar: restaurar, apagar de vez, esvaziar. */
export function TrashPanel({ trash, onRestore, onDeleteForever, onEmpty }: Props) {
  const [open, setOpen] = useState(false);
  if (trash.length === 0) return null;

  return (
    <div style={{ borderTop: `1px solid ${colors.border}`, marginTop: "auto" }}>
      <div onClick={() => setOpen((o) => !o)} style={header}>
        <span style={{ width: 12, fontSize: 10 }}>{open ? "▾" : "▸"}</span>
        <span style={{ flex: 1 }}>🗑️ Trash ({trash.length})</span>
        {open && (
          <button type="button" onClick={(e) => { e.stopPropagation(); onEmpty(); }} style={emptyBtn} title="Permanently delete everything in the trash">
            Empty
          </button>
        )}
      </div>
      {open && (
        <div style={{ padding: "0 6px 8px" }}>
          {trash.map((e) => (
            <div key={e.node.id} className="tree-row" style={row}>
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}>
                {e.node.type === "folder" ? (e.node.icon ?? "📁") : "🔌"} {e.node.name}
              </span>
              <button type="button" onClick={() => onRestore(e.node.id)} style={iconBtn} title="Restore">↩</button>
              <button type="button" onClick={() => onDeleteForever(e.node.id)} style={{ ...iconBtn, fontSize: 15 }} title="Delete forever">×</button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

const header: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "8px 12px",
  cursor: "pointer",
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: colors.dim,
};
const emptyBtn: CSSProperties = {
  background: "transparent",
  color: colors.danger,
  border: `1px solid ${colors.border}`,
  borderRadius: 5,
  padding: "1px 7px",
  fontSize: 10,
  textTransform: "uppercase",
  cursor: "pointer",
};
const row: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "5px 8px",
  borderRadius: 6,
  fontSize: 13,
  color: colors.text,
};
const iconBtn: CSSProperties = {
  background: "transparent",
  border: "none",
  color: colors.dim,
  cursor: "pointer",
  fontSize: 13,
  lineHeight: 1,
  padding: "0 2px",
};
