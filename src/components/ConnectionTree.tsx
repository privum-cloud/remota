import { type MouseEvent, useState } from "react";
import type { Connection, Credentials, Document, Node } from "../lib/vaultApi";
import { ContextMenu, type CtxItem } from "./ContextMenu";
import { colors } from "./styles";

const protoColor: Record<string, string> = {
  ssh: "#7ee787",
  rdp: "#79c0ff",
  vnc: "#ffa657",
  telnet: "#d2a8ff",
};

/** Username efetivo de uma conexão dada a cadeia de defaults das pastas-pai. */
function effectiveUser(chain: Credentials[], conn: Connection): { value?: string; inherited: boolean } {
  if (conn.credentials.username) return { value: conn.credentials.username, inherited: false };
  for (let i = chain.length - 1; i >= 0; i--) {
    if (chain[i].username) return { value: chain[i].username, inherited: true };
  }
  return { inherited: false };
}

type Dnd = {
  dragId: string | null;
  dropTarget: string | null; // id da pasta, ou "root"
  onStart: (id: string) => void;
  onOver: (target: string) => void;
  onLeave: () => void;
  onDrop: (parentId: string | null) => void;
};

type Props = {
  doc: Document;
  selectedId: string | null;
  onSelect: (node: Node) => void;
  onOpen: (node: Node) => void;
  onDelete: (id: string) => void;
  onNewConnection: (parentId: string | null) => void;
  onNewFolder: (parentId: string | null) => void;
  onMove: (nodeId: string, targetParentId: string | null) => void;
};

export function ConnectionTree({ doc, selectedId, onSelect, onOpen, onDelete, onNewConnection, onNewFolder, onMove }: Props) {
  const [menu, setMenu] = useState<{ x: number; y: number; items: CtxItem[] } | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [dragId, setDragId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);

  const dnd: Dnd = {
    dragId,
    dropTarget,
    onStart: (id) => setDragId(id),
    onOver: (target) => setDropTarget(target),
    onLeave: () => setDropTarget(null),
    onDrop: (parentId) => {
      if (dragId) onMove(dragId, parentId);
      setDragId(null);
      setDropTarget(null);
    },
  };

  function toggle(id: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function itemsFor(node: Node | null): CtxItem[] {
    if (!node) {
      return [
        { label: "New connection", onClick: () => onNewConnection(null) },
        { label: "New folder", onClick: () => onNewFolder(null) },
      ];
    }
    if (node.type === "folder") {
      return [
        { label: "New connection here", onClick: () => onNewConnection(node.id) },
        { label: "New folder here", onClick: () => onNewFolder(node.id) },
        "sep",
        { label: "Edit", onClick: () => onSelect(node) },
        { label: "Delete", onClick: () => onDelete(node.id) },
      ];
    }
    return [
      { label: "Connect", onClick: () => onOpen(node) },
      { label: "Edit", onClick: () => onSelect(node) },
      "sep",
      { label: "Delete", onClick: () => onDelete(node.id) },
    ];
  }

  function openMenu(e: MouseEvent, node: Node | null) {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, items: itemsFor(node) });
  }

  return (
    <div
      onContextMenu={(e) => openMenu(e, null)}
      onDragOver={(e) => { e.preventDefault(); setDropTarget("root"); }}
      onDrop={(e) => { e.preventDefault(); dnd.onDrop(null); }}
      style={{ padding: 6, minHeight: "100%", outline: dropTarget === "root" ? `2px dashed ${colors.accent}` : "none", outlineOffset: -2 }}
    >
      {doc.nodes.length === 0 ? (
        <div style={{ padding: 12, color: colors.dim, fontSize: 13 }}>Empty — right-click here to create.</div>
      ) : (
        doc.nodes.map((n) => (
          <TreeNode
            key={n.id}
            node={n}
            depth={0}
            chain={[]}
            selectedId={selectedId}
            collapsed={collapsed}
            onToggle={toggle}
            onSelect={onSelect}
            onOpen={onOpen}
            onDelete={onDelete}
            onContext={openMenu}
            dnd={dnd}
          />
        ))
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}

function TreeNode({
  node,
  depth,
  chain,
  selectedId,
  collapsed,
  onToggle,
  onSelect,
  onOpen,
  onDelete,
  onContext,
  dnd,
}: {
  node: Node;
  depth: number;
  chain: Credentials[];
  selectedId: string | null;
  collapsed: Set<string>;
  onToggle: (id: string) => void;
  onSelect: (node: Node) => void;
  onOpen: (node: Node) => void;
  onDelete: (id: string) => void;
  onContext: (e: MouseEvent, node: Node) => void;
  dnd: Dnd;
}) {
  const pad = 8 + depth * 14;

  if (node.type === "folder") {
    const isCollapsed = collapsed.has(node.id);
    const hasChildren = node.children.length > 0;
    return (
      <div>
        <div
          draggable
          onDragStart={(e) => { e.stopPropagation(); dnd.onStart(node.id); }}
          onDragOver={(e) => { e.preventDefault(); e.stopPropagation(); dnd.onOver(node.id); }}
          onDragLeave={() => dnd.onLeave()}
          onDrop={(e) => { e.preventDefault(); e.stopPropagation(); dnd.onDrop(node.id); }}
          onClick={() => onSelect(node)}
          onDoubleClick={() => onToggle(node.id)}
          onContextMenu={(e) => onContext(e, node)}
          style={{ ...rowStyle(node.id === selectedId), paddingLeft: pad, cursor: "pointer", color: colors.dim, ...(dnd.dropTarget === node.id ? { background: "#243447", outline: `1px solid ${colors.accent}` } : {}) }}
        >
          <span style={{ display: "flex", alignItems: "center", gap: 4, minWidth: 0 }}>
            <span
              onClick={(e) => { e.stopPropagation(); if (hasChildren) onToggle(node.id); }}
              style={{ width: 12, textAlign: "center", fontSize: 10, cursor: hasChildren ? "pointer" : "default" }}
            >
              {hasChildren ? (isCollapsed ? "▸" : "▾") : ""}
            </span>
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{node.icon ?? "📁"} {node.name}</span>
          </span>
          <DeleteBtn onClick={(e) => { e.stopPropagation(); onDelete(node.id); }} />
        </div>
        {!isCollapsed &&
          node.children.map((c) => (
            <TreeNode
              key={c.id}
              node={c}
              depth={depth + 1}
              chain={[...chain, node.defaults]}
              selectedId={selectedId}
              collapsed={collapsed}
              onToggle={onToggle}
              onSelect={onSelect}
              onOpen={onOpen}
              onDelete={onDelete}
              onContext={onContext}
              dnd={dnd}
            />
          ))}
      </div>
    );
  }

  const eu = effectiveUser(chain, node.conn);
  const selected = node.id === selectedId;
  return (
    <div
      draggable
      onDragStart={(e) => { e.stopPropagation(); dnd.onStart(node.id); }}
      onClick={() => onSelect(node)}
      onDoubleClick={() => onOpen(node)}
      onContextMenu={(e) => onContext(e, node)}
      title="Double-click to open · drag to move · right-click for options"
      style={{ ...rowStyle(selected), paddingLeft: pad, cursor: "pointer" }}
    >
      <span style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
        <span
          style={{
            fontSize: 10,
            fontWeight: 700,
            color: protoColor[node.conn.protocol] ?? colors.dim,
            textTransform: "uppercase",
            width: 42,
          }}
        >
          {node.conn.protocol}
        </span>
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {node.name}
          <span style={{ color: colors.dim }}> — {node.conn.host}</span>
          {eu.value && (
            <span style={{ color: eu.inherited ? colors.dim : colors.text, fontStyle: eu.inherited ? "italic" : "normal" }}>
              {" "}
              ({eu.value}
              {eu.inherited ? " ↳inherited" : ""})
            </span>
          )}
        </span>
      </span>
      <DeleteBtn onClick={(e) => { e.stopPropagation(); onDelete(node.id); }} />
    </div>
  );
}

function rowStyle(selected: boolean) {
  return {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
    padding: "6px 8px",
    borderRadius: 6,
    fontSize: 13,
    background: selected ? "#1f2530" : "transparent",
    color: colors.text,
  } as const;
}

function DeleteBtn({ onClick }: { onClick: (e: MouseEvent) => void }) {
  return (
    <button
      onClick={onClick}
      title="Delete"
      style={{ background: "transparent", border: "none", color: colors.dim, cursor: "pointer", fontSize: 14, lineHeight: 1 }}
    >
      ×
    </button>
  );
}
