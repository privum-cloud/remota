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

/** Id da pasta-pai de um nó, ou `null` se está na raiz. Usado para manter o nó no lugar ao editar. */
export function findParentId(doc: Document, id: string): string | null {
  function walk(nodes: Node[], parent: string | null): string | null | undefined {
    for (const n of nodes) {
      if (n.id === id) return parent;
      if (n.type === "folder") {
        const r = walk(n.children, n.id);
        if (r !== undefined) return r;
      }
    }
    return undefined; // não encontrado neste ramo
  }
  return walk(doc.nodes, null) ?? null;
}

/** Devolve o nó com este id (pasta ou conexão), ou null. */
export function findNode(doc: Document, id: string): Node | null {
  function walk(nodes: Node[]): Node | null {
    for (const n of nodes) {
      if (n.id === id) return n;
      if (n.type === "folder") {
        const r = walk(n.children);
        if (r) return r;
      }
    }
    return null;
  }
  return walk(doc.nodes);
}

/** `true` se um nó com este id já existe na árvore. */
export function nodeExists(doc: Document, id: string): boolean {
  function walk(nodes: Node[]): boolean {
    for (const n of nodes) {
      if (n.id === id) return true;
      if (n.type === "folder" && walk(n.children)) return true;
    }
    return false;
  }
  return walk(doc.nodes);
}
