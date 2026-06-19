import { type CSSProperties, type ReactNode, useState } from "react";
import { useVault } from "./state/useVault";
import { useSessions } from "./state/useSessions";
import { VaultUnlock } from "./components/VaultUnlock";
import { ConnectionTree } from "./components/ConnectionTree";
import { ConnectionEditor } from "./components/ConnectionEditor";
import { FolderEditor } from "./components/FolderEditor";
import { TabBar } from "./components/TabBar";
import { SessionView } from "./components/SessionView";
import { MenuBar, type Menu } from "./components/MenuBar";
import { findConnWithChain, findNode, findParentId, nodeExists } from "./lib/tree";
import { resolveCreds } from "./lib/inherit";
import type { Node } from "./lib/vaultApi";
import { vaultApi } from "./lib/vaultApi";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { colors } from "./components/styles";

type Editing = { kind: "connection" | "folder"; node: Node | null; parentId: string | null };

export default function App() {
  const v = useVault();
  const s = useSessions();
  const [editing, setEditing] = useState<Editing>({ kind: "connection", node: null, parentId: null });
  const [active, setActive] = useState<string>("editor");
  const [notice, setNotice] = useState<string | null>(null);

  if (v.status === "checking") return <Center>A verificar o cofre…</Center>;
  if (v.status === "locked") return <VaultUnlock exists={v.exists} error={v.error} onUnlock={v.unlock} />;

  const selected = editing.node;
  // novo nó vai dentro da pasta selecionada (se houver), senão na raiz.
  const targetParent = selected?.type === "folder" ? selected.id : null;

  function selectNode(n: Node) {
    setEditing({ kind: n.type === "folder" ? "folder" : "connection", node: n, parentId: findParentId(v.tree, n.id) });
    setActive("editor");
  }
  function newConnectionAt(parentId: string | null) {
    setEditing({ kind: "connection", node: null, parentId });
    setActive("editor");
  }
  function newFolderAt(parentId: string | null) {
    setEditing({ kind: "folder", node: null, parentId });
    setActive("editor");
  }
  const newConnection = () => newConnectionAt(targetParent);
  const newFolder = () => newFolderAt(targetParent);
  function lock() {
    v.lock();
    setEditing({ kind: "connection", node: null, parentId: null });
    setActive("editor");
  }

  async function saveNode(n: Node) {
    // mantém o nó no lugar: existente → pai atual; novo → pai-alvo da criação.
    const parentId = nodeExists(v.tree, n.id) ? findParentId(v.tree, n.id) : editing.parentId;
    await v.save(parentId, n);
    setEditing({ kind: n.type === "folder" ? "folder" : "connection", node: n, parentId });
  }

  async function openConn(id: string) {
    const found = findConnWithChain(v.tree, id);
    if (!found) return;
    const eff = resolveCreds(found.chain, found.node.conn);
    const newId = await s.openSession({
      title: found.node.name,
      protocol: found.node.conn.protocol,
      host: found.node.conn.host,
      port: found.node.conn.port,
      username: eff.username,
      password: eff.password,
    });
    setActive(newId);
  }
  function closeTab(id: string) {
    s.closeSession(id);
    setActive((cur) => (cur === id ? "editor" : cur));
  }

  async function importMremoteng() {
    const path = await open({ multiple: false, filters: [{ name: "mRemoteNG confCons", extensions: ["xml"] }] });
    if (typeof path !== "string") return; // cancelado
    try {
      const r = await vaultApi.importMremoteng(path);
      await v.refresh();
      setNotice(r.message);
    } catch (e) {
      setNotice("Falha no import: " + String(e));
    }
  }

  async function deleteNode(id: string) {
    const node = findNode(v.tree, id);
    const name = node?.name ?? "este item";
    const isFolder = node?.type === "folder";
    const msg = isFolder
      ? `Apagar a pasta "${name}" e tudo o que está dentro? Esta ação não pode ser desfeita.`
      : `Apagar a conexão "${name}"? Esta ação não pode ser desfeita.`;
    const ok = await confirm(msg, { title: "Confirmar", kind: "warning", okLabel: "Apagar", cancelLabel: "Cancelar" });
    if (!ok) return;
    await v.remove(id);
    if (selected?.id === id) setEditing({ kind: "connection", node: null, parentId: null });
  }

  const menus: Menu[] = [
    {
      title: "Ficheiro",
      items: [
        { label: "Nova conexão", onClick: newConnection },
        { label: "Nova pasta", onClick: newFolder },
        "sep",
        { label: "Importar do mRemoteNG (confCons.xml)…", onClick: importMremoteng },
        { label: "Exportar conexões…", onClick: () => setNotice("Export (cópia do cofre cifrado connections.dat, ou JSON) chega a seguir.") },
        "sep",
        { label: "Bloquear cofre", onClick: lock },
      ],
    },
    {
      title: "Ajuda",
      items: [
        { label: "Sobre o Remota", onClick: () => setNotice("Remota — gestor de conexões remotas multi-protocolo para Linux. Clean-room, AGPLv3.") },
      ],
    },
  ];

  return (
    <div style={shell}>
      <MenuBar menus={menus} />

      <div style={toolbar}>
        <strong style={{ fontSize: 14, letterSpacing: 0.3 }}>Remota</strong>
        <div style={{ width: 1, height: 18, background: colors.border, margin: "0 6px" }} />
        <button style={toolBtn} onClick={newConnection}>+ Conexão</button>
        <button style={toolBtn} onClick={newFolder}>+ Pasta</button>
        <button style={toolBtn} onClick={lock}>Bloquear</button>
      </div>

      {notice && (
        <div style={banner}>
          <span style={{ fontSize: 13 }}>{notice}</span>
          <button onClick={() => setNotice(null)} style={{ ...toolBtn, marginLeft: "auto" }}>×</button>
        </div>
      )}

      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
        <aside style={sidebar}>
          <div style={sidebarHead}>Conexões</div>
          <ConnectionTree
            doc={v.tree}
            selectedId={selected?.id ?? null}
            onSelect={selectNode}
            onOpen={(n) => { if (n.type === "connection") openConn(n.id); }}
            onDelete={deleteNode}
            onNewConnection={newConnectionAt}
            onNewFolder={newFolderAt}
          />
        </aside>

        <main style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
          <TabBar
            tabs={s.tabs}
            activeId={active}
            onActivate={setActive}
            onClose={closeTab}
            onReconnect={s.reconnect}
            onDuplicate={async (id) => { const nid = await s.duplicate(id); if (nid) setActive(nid); }}
          />
          <div style={{ flex: 1, position: "relative", minHeight: 0 }}>
            <div style={{ ...pane, overflow: "auto", display: active === "editor" ? "block" : "none" }}>
              {editing.kind === "folder" ? (
                <FolderEditor node={editing.node} onSave={saveNode} />
              ) : (
                <ConnectionEditor node={editing.node} onSave={saveNode} onConnect={openConn} />
              )}
            </div>
            {s.tabs.map((t) => (
              <div key={t.id} style={{ ...pane, display: active === t.id ? "block" : "none" }}>
                <SessionView tab={t} />
              </div>
            ))}
          </div>
        </main>
      </div>

      <footer style={statusbar}>
        <span style={{ color: "#7ee787" }}>● cofre destravado</span>
        <Sep />
        <span>gateway 127.0.0.1 (local)</span>
        <Sep />
        <span>{s.tabs.length} {s.tabs.length === 1 ? "sessão aberta" : "sessões abertas"}</span>
      </footer>
    </div>
  );
}

function Sep() {
  return <span style={{ color: colors.border }}>│</span>;
}

function Center({ children }: { children: ReactNode }) {
  return (
    <div style={{ height: "100vh", display: "grid", placeItems: "center", background: colors.bg, color: colors.dim, fontFamily: "system-ui, sans-serif" }}>
      {children}
    </div>
  );
}

const shell: CSSProperties = {
  height: "100vh",
  display: "flex",
  flexDirection: "column",
  background: colors.bg,
  color: colors.text,
  fontFamily: "system-ui, sans-serif",
};
const toolbar: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "6px 10px",
  borderBottom: `1px solid ${colors.border}`,
  background: "#12141a",
};
const toolBtn: CSSProperties = {
  background: "transparent",
  color: colors.dim,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: "4px 10px",
  fontSize: 12,
  cursor: "pointer",
};
const banner: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "8px 12px",
  background: "#1b2330",
  borderBottom: `1px solid ${colors.border}`,
  color: colors.text,
};
const sidebar: CSSProperties = { width: 320, borderRight: `1px solid ${colors.border}`, overflow: "auto", display: "flex", flexDirection: "column" };
const sidebarHead: CSSProperties = {
  padding: "8px 12px",
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.6,
  color: colors.dim,
  borderBottom: `1px solid ${colors.border}`,
};
const pane: CSSProperties = { position: "absolute", inset: 0 };
const statusbar: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "4px 12px",
  borderTop: `1px solid ${colors.border}`,
  background: "#12141a",
  color: colors.dim,
  fontSize: 12,
};
