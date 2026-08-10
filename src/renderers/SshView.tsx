import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { clipRead, clipWrite } from "../lib/clipboard";

/** Terminal SSH: liga ao WS do gateway (que fala russh ao host) via xterm.js. */
export function SshView({ wsUrl, onClosed }: { wsUrl: string; onClosed?: () => void }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const term = new Terminal({
      fontSize: 13,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
      cursorBlink: true,
      theme: { background: "#0f1115", foreground: "#e6e6e6" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    fit.fit();

    // Abre o WS já com as dimensões reais → o PTY nasce do tamanho certo (k9s/vim ocupam tudo).
    const ws = new WebSocket(`${wsUrl}&cols=${term.cols}&rows=${term.rows}`);
    ws.binaryType = "arraybuffer";
    const enc = new TextEncoder();

    // Copy/paste no terminal: Ctrl+Shift+C copia a seleção; Ctrl+Shift+V cola.
    term.attachCustomKeyEventHandler((e) => {
      if (e.type === "keydown" && e.ctrlKey && e.shiftKey) {
        if (e.code === "KeyC") {
          const sel = term.getSelection();
          if (sel) clipWrite(sel);
          return false;
        }
        if (e.code === "KeyV") {
          clipRead().then((t) => { if (t && ws.readyState === WebSocket.OPEN) ws.send(enc.encode(t)); });
          return false;
        }
      }
      return true;
    });

    // Mensagem de controlo (texto/JSON) — distinta dos keystrokes (binário).
    const sendResize = () => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }));
      }
    };

    ws.onmessage = (ev) => {
      if (typeof ev.data === "string") term.write(ev.data);
      else term.write(new Uint8Array(ev.data));
    };
    ws.onopen = () => sendResize(); // garante o PTY alinhado assim que liga
    ws.onclose = () => {
      term.write("\r\n\x1b[90m[session ended]\x1b[0m\r\n");
      onClosed?.();
    };

    const onData = term.onData((d) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(enc.encode(d));
    });

    // Reajusta + avisa o remoto sempre que o PAINEL muda de tamanho (não só a janela).
    const refit = () => {
      fit.fit();
      sendResize();
    };
    const ro = new ResizeObserver(() => refit());
    ro.observe(el);
    window.addEventListener("resize", refit);

    return () => {
      ro.disconnect();
      window.removeEventListener("resize", refit);
      onData.dispose();
      ws.close();
      term.dispose();
    };
  }, [wsUrl]);

  return <div ref={ref} style={{ width: "100%", height: "100%", background: "#0f1115", padding: 4, boxSizing: "border-box" }} />;
}
