import type { MouseEvent } from "react";
import type { Connection, Credentials, Document, Node } from "../lib/vaultApi";
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

type Props = {
  doc: Document;
  selectedId: string | null;
  onSelect: (node: Node) => void;
  onDelete: (id: string) => void;
};

export function ConnectionTree({ doc, selectedId, onSelect, onDelete }: Props) {
  if (doc.nodes.length === 0) {
    return <div style={{ padding: 16, color: colors.dim, fontSize: 13 }}>Sem conexões ainda — cria a primeira →</div>;
  }
  return (
    <div style={{ padding: 6 }}>
      {doc.nodes.map((n) => (
        <TreeNode
          key={n.id}
          node={n}
          depth={0}
          chain={[]}
          selectedId={selectedId}
          onSelect={onSelect}
          onDelete={onDelete}
        />
      ))}
    </div>
  );
}

function TreeNode({
  node,
  depth,
  chain,
  selectedId,
  onSelect,
  onDelete,
}: {
  node: Node;
  depth: number;
  chain: Credentials[];
  selectedId: string | null;
  onSelect: (node: Node) => void;
  onDelete: (id: string) => void;
}) {
  const pad = 8 + depth * 14;

  if (node.type === "folder") {
    return (
      <div>
        <div style={{ ...rowStyle(false), paddingLeft: pad, color: colors.dim }}>
          <span>📁 {node.name}</span>
          <DeleteBtn onClick={() => onDelete(node.id)} />
        </div>
        {node.children.map((c) => (
          <TreeNode
            key={c.id}
            node={c}
            depth={depth + 1}
            chain={[...chain, node.defaults]}
            selectedId={selectedId}
            onSelect={onSelect}
            onDelete={onDelete}
          />
        ))}
      </div>
    );
  }

  const eu = effectiveUser(chain, node.conn);
  const selected = node.id === selectedId;
  return (
    <div
      onClick={() => onSelect(node)}
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
              {eu.inherited ? " ↳herdado" : ""})
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
      title="Apagar"
      style={{ background: "transparent", border: "none", color: colors.dim, cursor: "pointer", fontSize: 14, lineHeight: 1 }}
    >
      ×
    </button>
  );
}
