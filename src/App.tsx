import { type CSSProperties, type ReactNode, useState } from "react";
import { useVault } from "./state/useVault";
import { useSessions } from "./state/useSessions";
import { useUpdate } from "./lib/useUpdate";
import { VaultUnlock } from "./components/VaultUnlock";
import { PasswordPrompt } from "./components/PasswordPrompt";
import { SetPasswordPrompt } from "./components/SetPasswordPrompt";
import { UpdateBanner } from "./components/UpdateBanner";
import { ConnectionTree } from "./components/ConnectionTree";
import { ConnectionEditor } from "./components/ConnectionEditor";
import { FolderEditor } from "./components/FolderEditor";
import { TabBar } from "./components/TabBar";
import { SessionView } from "./components/SessionView";
import { TrashPanel } from "./components/TrashPanel";
import { MenuBar, type Menu } from "./components/MenuBar";
import { findConnWithChain, findNode, findParentId, isInSubtree, nodeExists } from "./lib/tree";
import { resolveCreds, resolveGateway } from "./lib/inherit";
import type { Node } from "./lib/vaultApi";
import { vaultApi, vaultErrorText } from "./lib/vaultApi";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import { colors } from "./components/styles";

type Editing = { kind: "connection" | "folder"; node: Node | null; parentId: string | null };

export default function App() {
  const v = useVault();
  const s = useSessions();
  const u = useUpdate();
  const [editing, setEditing] = useState<Editing>({ kind: "connection", node: null, parentId: null });
  const [editorExpanded, setEditorExpanded] = useState<boolean>(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(() => {
    const saved = localStorage.getItem("remota:sidebarCollapsed");
    return saved === "true";
  });
  const [active, setActive] = useState<string>("");
  const [notice, setNotice] = useState<string | null>(null);
  // Connect-time password prompt (connections with always_prompt). Resolves with the typed
  // password, or null if cancelled. The value is handed straight to the session, never stored.
  const [pwPrompt, setPwPrompt] = useState<{ name: string; resolve: (pw: string | null) => void } | null>(null);
  const promptPassword = (name: string) => new Promise<string | null>((resolve) => setPwPrompt({ name, resolve }));
  const [setPwModal, setSetPwModal] = useState<{ title: string; resolve: (pw: string | null) => void } | null>(null);
  const promptNewPassword = (title: string) => new Promise<string | null>((resolve) => setSetPwModal({ title, resolve }));

  const toggleSidebar = () => {
    setSidebarCollapsed((prev) => {
      const next = !prev;
      localStorage.setItem("remota:sidebarCollapsed", String(next));
      return next;
    });
  };

  // Built once and rendered in every branch below — including the locked screen. Whoever has not
  // opened Remota in a month is exactly the person who has not heard about the release that fixed
  // something, and they should not have to unlock a vault to find out.
  const updateBanner = u.policy ? (
    <UpdateBanner
      phase={u.phase}
      delivery={u.policy.delivery}
      releasesUrl={u.policy.releasesUrl}
      onInstall={u.install}
      onDismiss={u.dismiss}
    />
  ) : null;

  if (v.status === "checking") return <Center>Checking vault…</Center>;
  if (v.status === "locked")
    return (
      <div style={{ display: "flex", flexDirection: "column", height: "100vh" }}>
        {updateBanner}
        <div style={{ flex: 1, minHeight: 0 }}>
          <VaultUnlock exists={v.exists} error={v.error} onUnlock={v.unlock} />
        </div>
      </div>
    );

  const selected = editing.node;
  // novo nó vai dentro da pasta selecionada (se houver), senão na raiz.
  const targetParent = selected?.type === "folder" ? selected.id : null;

  function selectNode(n: Node) {
    setEditing({ kind: n.type === "folder" ? "folder" : "connection", node: n, parentId: findParentId(v.tree, n.id) });
    setEditorExpanded(true);
    setActive("editor");
  }
  function newConnectionAt(parentId: string | null) {
    setEditing({ kind: "connection", node: null, parentId });
    setEditorExpanded(true);
    setActive("editor");
  }
  function newFolderAt(parentId: string | null) {
    setEditing({ kind: "folder", node: null, parentId });
    setEditorExpanded(true);
    setActive("editor");
  }
  const newConnection = () => newConnectionAt(targetParent);
  const newFolder = () => newFolderAt(targetParent);
  function lock() {
    v.lock();
    setEditing({ kind: "connection", node: null, parentId: null });
    setEditorExpanded(false);
    setActive("");
  }

  function handleToggleEditor() {
    if (editorExpanded) {
      if (active === "editor") {
        setEditorExpanded(false);
        if (s.tabs.length > 0) {
          setActive(s.tabs[s.tabs.length - 1].id);
        } else {
          setActive("");
        }
      } else {
        setActive("editor");
      }
    } else {
      setEditorExpanded(true);
      setActive("editor");
    }
  }

  async function saveNode(n: Node) {
    // mantém o nó no lugar: existente → pai atual; novo → pai-alvo da criação.
    const parentId = nodeExists(v.tree, n.id) ? findParentId(v.tree, n.id) : editing.parentId;
    await v.save(parentId, n);
    setEditing({ kind: n.type === "folder" ? "folder" : "connection", node: n, parentId });
  }

  async function openConn(id: string, forceNew = false) {
    // Duplo-clique: se já há uma aba aberta desta conexão, volta a ela (não abre outra).
    if (!forceNew) {
      const existing = s.tabs.find((t) => t.connId === id);
      if (existing) { setActive(existing.id); return; }
    }
    const found = findConnWithChain(v.tree, id);
    if (!found) return;
    const eff = resolveCreds(found.chain, found.node.conn);
    const gw = resolveGateway(found.gateways, found.node.conn);
    let password = eff.password;
    if (found.node.conn.always_prompt) {
      const typed = await promptPassword(found.node.name);
      if (typed === null) return; // cancelled → abort the open, no tab
      password = typed; // use the typed password; ignore any inherited/resolved one
    }
    const newId = await s.openSession({
      connId: id,
      title: found.node.name.trim(),
      protocol: found.node.conn.protocol,
      host: found.node.conn.host,
      port: found.node.conn.port,
      username: eff.username,
      password,
      keyPath: eff.key_path,
      domain: eff.domain,
      gateway: gw,
      relay: found.node.conn.relay,
    });
    setActive(newId);
  }
  function closeTab(id: string) {
    s.closeSession(id);
    setActive((cur) => {
      if (cur !== id) return cur;
      const remaining = s.tabs.filter((t) => t.id !== id);
      if (remaining.length > 0) return remaining[remaining.length - 1].id;
      return editorExpanded ? "editor" : "";
    });
  }

  // Arrastar um nó para dentro de uma pasta (ou raiz). O backend (upsert) trata do mover.
  async function moveNode(nodeId: string, targetParentId: string | null) {
    if (nodeId === targetParentId) return;
    const node = findNode(v.tree, nodeId);
    if (!node) return;
    if (findParentId(v.tree, nodeId) === targetParentId) return; // já está lá
    if (node.type === "folder" && targetParentId && isInSubtree(node, targetParentId)) return; // evita ciclo
    await v.save(targetParentId, node);
  }

  async function importMremoteng() {
    const path = await open({ multiple: false, filters: [{ name: "mRemoteNG confCons", extensions: ["xml"] }] });
    if (typeof path !== "string") return; // cancelado
    try {
      const r = await vaultApi.importMremoteng(path);
      await v.refresh();
      setNotice(r.message);
    } catch (e) {
      setNotice("Import failed: " + String(e));
    }
  }

  async function exportConnections() {
    const path = await save({ defaultPath: "remota-connections.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const r = await vaultApi.exportConnections(path);
      setNotice(r.message);
    } catch (e) {
      const err = vaultErrorText(e);
      if (err.includes("WouldOverwriteVault")) {
        setNotice("That's your live vault file — pick a different file for the export.");
        return;
      }
      setNotice("Export failed: " + err);
    }
  }

  async function exportConnectionsEncrypted() {
    const path = await save({
      defaultPath: "remota-connections.dat",
      filters: [{ name: "Remota encrypted", extensions: ["dat"] }],
    });
    if (typeof path !== "string") return;
    const pw = await promptNewPassword("Set a password for this export");
    if (pw === null) return; // cancelled → nothing is written
    try {
      const r = await vaultApi.exportConnectionsEncrypted(path, pw);
      setNotice(r.message);
    } catch (e) {
      const err = vaultErrorText(e);
      if (err.includes("WouldOverwriteVault")) {
        setNotice("That's your live vault file — pick a different file for the export.");
        return;
      }
      setNotice("Export failed: " + err);
    }
  }

  async function importRemotaJson() {
    const path = await open({ multiple: false, filters: [{ name: "Remota", extensions: ["dat", "json"] }] });
    if (typeof path !== "string") return;
    const name = path.split(/[/\\]/).pop() ?? "export";
    const attempt = async (pw?: string): Promise<void> => {
      try {
        const r = await vaultApi.importRemotaJson(path, pw);
        await v.refresh();
        setNotice(r.message);
      } catch (e) {
        const err = vaultErrorText(e);
        if (err.includes("NeedsPassword")) {
          const typed = await promptPassword(name);
          if (typed === null) return; // cancelled → no import
          return attempt(typed);
        }
        if (err.includes("Crypto")) {
          setNotice("Could not decrypt — wrong password or the file is corrupt.");
          return;
        }
        setNotice("Import failed: " + err);
      }
    };
    await attempt();
  }

  async function deleteNode(id: string) {
    const node = findNode(v.tree, id);
    const name = node?.name?.trim() ?? "este item";
    const isFolder = node?.type === "folder";
    const msg = isFolder
      ? `Move the folder "${name}" and its contents to the Trash?`
      : `Move the connection "${name}" to the Trash?`;
    const ok = await confirm(msg, { title: "Move to Trash", kind: "warning", okLabel: "Move to Trash", cancelLabel: "Cancel" });
    if (!ok) return;
    await v.remove(id); // soft-delete → lixeira (restaurável)
    if (selected?.id === id) setEditing({ kind: "connection", node: null, parentId: null });
  }

  async function deleteForever(id: string) {
    const ok = await confirm("Delete this item permanently? This cannot be undone.", { title: "Delete forever", kind: "warning", okLabel: "Delete forever", cancelLabel: "Cancel" });
    if (ok) await v.deleteForever(id);
  }
  async function emptyTrash() {
    const ok = await confirm("Permanently delete everything in the Trash? This cannot be undone.", { title: "Empty Trash", kind: "warning", okLabel: "Empty Trash", cancelLabel: "Cancel" });
    if (ok) await v.emptyTrash();
  }

  const menus: Menu[] = [
    {
      title: "File",
      items: [
        { label: "New connection", onClick: newConnection },
        { label: "New folder", onClick: newFolder },
        "sep",
        { label: sidebarCollapsed ? "Show Connections Sidebar" : "Collapse Connections Sidebar", onClick: toggleSidebar },
        { label: editorExpanded ? "Collapse Editor" : "Expand Editor", onClick: handleToggleEditor },
        "sep",
        { label: "Import from mRemoteNG (confCons.xml)…", onClick: importMremoteng },
        { label: "Import connections (Remota)…", onClick: importRemotaJson },
        { label: "Export connections (encrypted)…", onClick: exportConnectionsEncrypted },
        { label: "Export connections (unencrypted backup)…", onClick: exportConnections },
        "sep",
        { label: "Lock vault", onClick: lock },
      ],
    },
    {
      title: "Help",
      items: [
        {
          // A check the user asked for reports the boring answer too; staying silent would look
          // like the button did nothing.
          label: "Check for updates now",
          onClick: async () => {
            const found = await u.checkNow();
            if (!found) setNotice("Remota is up to date.");
          },
        },
        {
          label: u.policy?.enabled
            ? "✓ Check for updates automatically"
            : "Check for updates automatically",
          onClick: () => void u.setEnabled(!u.policy?.enabled),
        },
        "sep",
        { label: "About Remota", onClick: () => setNotice("Remota — multi-protocol remote connection manager for Linux. Clean-room, AGPLv3.") },
      ],
    },
  ];

  return (
    <div style={shell}>
      <MenuBar menus={menus} />
      {updateBanner}
      <div className="tricolore">
        <span style={{ background: "#009246" }} />
        <span style={{ background: "#f4f4f4" }} />
        <span style={{ background: "#ce2b37" }} />
      </div>

      <div style={toolbar}>
        <strong style={{ fontSize: 14, letterSpacing: 0.3 }}>Remota</strong>
        <div style={{ width: 1, height: 18, background: colors.border, margin: "0 6px" }} />
        <button
          style={{
            ...toolBtn,
            color: !sidebarCollapsed ? colors.accent : colors.dim,
            borderColor: !sidebarCollapsed ? colors.accent : colors.border,
          }}
          onClick={toggleSidebar}
          title={sidebarCollapsed ? "Expandir menu Connections" : "Recolher menu Connections"}
        >
          {!sidebarCollapsed ? "◧ Connections ▾" : "◧ Connections ▸"}
        </button>
        <button style={toolBtn} onClick={newConnection}>+ Connection</button>
        <button style={toolBtn} onClick={newFolder}>+ Folder</button>
        <button
          style={{ ...toolBtn, color: editorExpanded ? colors.accent : colors.dim, borderColor: editorExpanded ? colors.accent : colors.border }}
          onClick={handleToggleEditor}
          title={editorExpanded ? "Recolher aba Editor" : "Expandir aba Editor"}
        >
          {editorExpanded ? "✎ Editor ▾" : "✎ Editor ▸"}
        </button>
        <button style={toolBtn} onClick={lock}>Lock</button>
      </div>

      {notice && (
        <div style={banner}>
          <span style={{ fontSize: 13 }}>{notice}</span>
          <button onClick={() => setNotice(null)} style={{ ...toolBtn, marginLeft: "auto" }}>×</button>
        </div>
      )}

      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
        <aside
          style={{
            ...sidebar,
            width: sidebarCollapsed ? 36 : 320,
            background: sidebarCollapsed ? "#12141a" : "transparent",
            cursor: sidebarCollapsed ? "pointer" : "default",
            userSelect: sidebarCollapsed ? "none" : "auto",
          }}
          onClick={sidebarCollapsed ? toggleSidebar : undefined}
          title={sidebarCollapsed ? "Clique para expandir o menu Connections" : undefined}
        >
          {sidebarCollapsed ? (
            <>
              <div
                style={{
                  height: 32,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  borderBottom: `1px solid ${colors.border}`,
                }}
              >
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleSidebar();
                  }}
                  style={iconBtn}
                  title="Expandir menu Connections"
                >
                  ▸
                </button>
              </div>
              <div
                style={{
                  flex: 1,
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  paddingTop: 16,
                  gap: 10,
                }}
              >
                <span style={{ fontSize: 13, opacity: 0.7 }}>🗂</span>
                <span
                  style={{
                    writingMode: "vertical-rl",
                    transform: "rotate(180deg)",
                    fontSize: 10,
                    letterSpacing: 1.5,
                    textTransform: "uppercase",
                    color: colors.dim,
                    fontWeight: 600,
                  }}
                >
                  Connections
                </span>
              </div>
            </>
          ) : (
            <>
              <div
                style={{
                  ...sidebarHead,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <span>Connections</span>
                <button
                  type="button"
                  onClick={toggleSidebar}
                  style={iconBtn}
                  title="Recolher menu Connections"
                >
                  ◂
                </button>
              </div>
              <div style={{ flex: 1, overflow: "auto", minHeight: 0 }}>
                <ConnectionTree
                  doc={v.tree}
                  selectedId={selected?.id ?? null}
                  onSelect={selectNode}
                  onOpen={(n) => {
                    if (n.type === "connection") openConn(n.id);
                  }}
                  onOpenNew={(n) => {
                    if (n.type === "connection") openConn(n.id, true);
                  }}
                  onDelete={deleteNode}
                  onNewConnection={newConnectionAt}
                  onNewFolder={newFolderAt}
                  onMove={moveNode}
                />
              </div>
              <TrashPanel
                trash={v.tree.trash ?? []}
                onRestore={v.restore}
                onDeleteForever={deleteForever}
                onEmpty={emptyTrash}
              />
            </>
          )}
        </aside>

        <main style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
          <TabBar
            tabs={s.tabs}
            activeId={active}
            editorExpanded={editorExpanded}
            onToggleEditor={handleToggleEditor}
            onActivate={(id) => {
              if (id === "editor") {
                setEditorExpanded(true);
              }
              setActive(id);
            }}
            onClose={closeTab}
            onReconnect={s.reconnect}
            onDuplicate={async (id) => { const nid = await s.duplicate(id); if (nid) setActive(nid); }}
          />
          <div style={{ flex: 1, position: "relative", minHeight: 0 }}>
            <div style={{ ...pane, overflow: "auto", display: active === "editor" && editorExpanded ? "block" : "none" }}>
              <div style={{ display: "flex", justifyContent: "flex-end", padding: "10px 16px 0", maxWidth: 420 }}>
                <button
                  type="button"
                  onClick={handleToggleEditor}
                  style={{ ...toolBtn, fontSize: 11, padding: "2px 8px", display: "flex", alignItems: "center", gap: 4 }}
                  title="Recolher aba Editor"
                >
                  <span>▴ Recolher Editor</span>
                </button>
              </div>
              {editing.kind === "folder" ? (
                <FolderEditor node={editing.node} onSave={saveNode} />
              ) : (
                <ConnectionEditor node={editing.node} onSave={saveNode} onConnect={openConn} />
              )}
            </div>
            {s.tabs.map((t) => (
              <div key={t.id} style={{ ...pane, display: active === t.id ? "block" : "none" }}>
                <SessionView tab={t} onDead={() => s.markDead(t.id)} />
              </div>
            ))}
            {(!editorExpanded || active !== "editor") && !s.tabs.some((t) => t.id === active) && (
              <div
                style={{
                  height: "100%",
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 12,
                  color: colors.dim,
                  padding: 24,
                  userSelect: "none",
                }}
              >
                <div style={{ fontSize: 36, opacity: 0.4 }}>🖥️</div>
                <div style={{ fontSize: 15, fontWeight: 500, color: colors.text }}>Nenhuma sessão aberta</div>
                <div style={{ fontSize: 13, maxWidth: 380, textAlign: "center", lineHeight: 1.5 }}>
                  Dê um duplo clique em uma conexão na barra lateral para conectar, ou expanda a aba Editor para gerenciar conexões.
                </div>
                <button
                  type="button"
                  onClick={() => {
                    setEditorExpanded(true);
                    setActive("editor");
                  }}
                  style={{
                    ...toolBtn,
                    borderColor: colors.accent,
                    color: colors.text,
                    padding: "6px 14px",
                    marginTop: 4,
                  }}
                >
                  ✎ Expandir Editor
                </button>
              </div>
            )}
          </div>
        </main>
      </div>

      {pwPrompt && (
        <PasswordPrompt
          name={pwPrompt.name}
          onSubmit={(pw) => { pwPrompt.resolve(pw); setPwPrompt(null); }}
          onCancel={() => { pwPrompt.resolve(null); setPwPrompt(null); }}
        />
      )}

      {setPwModal && (
        <SetPasswordPrompt
          title={setPwModal.title}
          onSubmit={(pw) => { setPwModal.resolve(pw); setSetPwModal(null); }}
          onCancel={() => { setPwModal.resolve(null); setSetPwModal(null); }}
        />
      )}

      <footer style={statusbar}>
        <span style={{ color: "#7ee787" }}>● vault unlocked</span>
        <Sep />
        <span>gateway 127.0.0.1 (local)</span>
        <Sep />
        <span>{s.tabs.length} {s.tabs.length === 1 ? "open session" : "open sessions"}</span>
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
const sidebar: CSSProperties = {
  borderRight: `1px solid ${colors.border}`,
  overflow: "hidden",
  display: "flex",
  flexDirection: "column",
  transition: "width 0.15s ease",
  flexShrink: 0,
};
const iconBtn: CSSProperties = {
  background: "transparent",
  border: "none",
  color: colors.dim,
  cursor: "pointer",
  padding: "2px 6px",
  borderRadius: 4,
  fontSize: 12,
  lineHeight: 1,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};
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
