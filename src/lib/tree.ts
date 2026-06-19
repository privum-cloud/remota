import type { Credentials, Document, Node } from "./vaultApi";

export type ConnectionNode = Extract<Node, { type: "connection" }>;

/** Localiza uma conexão pelo id e devolve a cadeia de defaults das pastas-pai (raiz→pai). */
export function findConnWithChain(
  doc: Document,
  id: string,
): { node: ConnectionNode; chain: Credentials[] } | null {
  function walk(nodes: Node[], chain: Credentials[]): { node: ConnectionNode; chain: Credentials[] } | null {
    for (const n of nodes) {
      if (n.type === "connection" && n.id === id) return { node: n, chain };
      if (n.type === "folder") {
        const r = walk(n.children, [...chain, n.defaults]);
        if (r) return r;
      }
    }
    return null;
  }
  return walk(doc.nodes, []);
}
