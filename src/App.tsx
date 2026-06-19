import { type CSSProperties, type ReactNode, useState } from "react";
import { useVault } from "./state/useVault";
import { useSessions } from "./state/useSessions";
import { VaultUnlock } from "./components/VaultUnlock";
import { ConnectionTree } from "./components/ConnectionTree";
import { ConnectionEditor } from "./components/ConnectionEditor";
import { TabBar } from "./components/TabBar";
import { SessionView } from "./components/SessionView";
import { findConnWithChain } from "./lib/tree";
import { resolveCreds } from "./lib/inherit";
import type { Node } from "./lib/vaultApi";
import { colors, ghostBtn } from "./components/styles";

export default function App() {
  const v = useVault();
  const s = useSessions();
  const [selected, setSelected] = useState<Node | null>(null);
  // aba ativa: "editor" ou o id de uma sessão.
  const [active, setActive] = useState<string>("editor");

  if (v.status === "checking") return <Center>A verificar o cofre…</Center>;
  if (v.status === "locked") return <VaultUnlock exists={v.exists} error={v.error} onUnlock={v.unlock} />;

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

  return (
    <div style={shell}>
      <header style={headerStyle}>
        <strong style={{ fontSize: 15 }}>Remota</strong>
        <span style={{ color: colors.dim, fontSize: 12 }}>cofre destravado</span>
        <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
          <button style={ghostBtn} onClick={() => { setSelected(null); setActive("editor"); }}>+ Nova conexão</button>
          <button style={ghostBtn} onClick={() => { v.lock(); setSelected(null); setActive("editor"); }}>Bloquear</button>
        </div>
      </header>

      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
        <aside style={sidebar}>
          <ConnectionTree
            doc={v.tree}
            selectedId={selected?.id ?? null}
            onSelect={(n) => { setSelected(n); setActive("editor"); }}
            onOpen={(n) => { if (n.type === "connection") openConn(n.id); }}
            onDelete={(id) => { v.remove(id); if (selected?.id === id) setSelected(null); }}
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
            {/* Editor (montado sempre; visível só na aba "editor") */}
            <div style={{ ...pane, overflow: "auto", display: active === "editor" ? "block" : "none" }}>
              <ConnectionEditor node={selected} onSave={async (n) => { await v.save(null, n); setSelected(n); }} />
            </div>
            {/* Sessões: todas montadas (mantêm a ligação viva), visível só a ativa */}
            {s.tabs.map((t) => (
              <div key={t.id} style={{ ...pane, display: active === t.id ? "block" : "none" }}>
                <SessionView tab={t} />
              </div>
            ))}
          </div>
        </main>
      </div>
    </div>
  );
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
const headerStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 12,
  padding: "10px 14px",
  borderBottom: `1px solid ${colors.border}`,
};
const sidebar: CSSProperties = { width: 340, borderRight: `1px solid ${colors.border}`, overflow: "auto" };
const pane: CSSProperties = { position: "absolute", inset: 0 };
