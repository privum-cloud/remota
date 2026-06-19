import { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Protocol } from "../lib/vaultApi";

type SessionInfo = { wsUrl: string; kind: string };

export interface SessionTab {
  id: string; // id único da aba (≠ id da conexão; permite múltiplas abas da mesma conexão)
  title: string;
  protocol: Protocol;
  target: string; // host:port
  password?: string;
  wsUrl: string;
  epoch: number; // incrementa no reconnect para forçar remount do renderizador
  error?: string;
}

export interface OpenOpts {
  title: string;
  protocol: Protocol;
  host: string;
  port?: number;
  password?: string;
}

const DEFAULT_PORT: Record<Protocol, number> = { ssh: 22, rdp: 3389, vnc: 5900, telnet: 23 };

async function openGateway(target: string, protocol: Protocol): Promise<{ wsUrl: string; error?: string }> {
  try {
    const info = await invoke<SessionInfo>("open_session", { target, kind: protocol });
    return { wsUrl: info.wsUrl };
  } catch (e) {
    return { wsUrl: "", error: String(e) };
  }
}

export function useSessions() {
  const [tabs, setTabs] = useState<SessionTab[]>([]);
  const tabsRef = useRef<SessionTab[]>([]);
  tabsRef.current = tabs;

  const openSession = useCallback(async (opts: OpenOpts): Promise<string> => {
    const port = opts.port ?? DEFAULT_PORT[opts.protocol];
    const target = `${opts.host}:${port}`;
    const id = crypto.randomUUID();
    const { wsUrl, error } = await openGateway(target, opts.protocol);
    setTabs((t) => [
      ...t,
      { id, title: opts.title, protocol: opts.protocol, target, password: opts.password, wsUrl, epoch: 0, error },
    ]);
    return id;
  }, []);

  const closeSession = useCallback((id: string) => {
    setTabs((t) => t.filter((x) => x.id !== id));
  }, []);

  const reconnect = useCallback(async (id: string) => {
    const tab = tabsRef.current.find((x) => x.id === id);
    if (!tab) return;
    const { wsUrl, error } = await openGateway(tab.target, tab.protocol);
    setTabs((t) => t.map((x) => (x.id === id ? { ...x, wsUrl, error, epoch: x.epoch + 1 } : x)));
  }, []);

  const duplicate = useCallback(
    async (id: string): Promise<string | null> => {
      const tab = tabsRef.current.find((x) => x.id === id);
      if (!tab) return null;
      const i = tab.target.lastIndexOf(":");
      return openSession({
        title: `${tab.title} (cópia)`,
        protocol: tab.protocol,
        host: tab.target.slice(0, i),
        port: Number(tab.target.slice(i + 1)),
        password: tab.password,
      });
    },
    [openSession],
  );

  return { tabs, openSession, closeSession, reconnect, duplicate };
}
