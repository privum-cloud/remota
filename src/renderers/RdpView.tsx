import { useEffect, useRef, useState } from "react";
import init, {
  SessionBuilder,
  DesktopSize,
  Extension,
  DeviceEvent,
  InputTransaction,
  RotationUnit,
  ClipboardData,
  type ClipboardItem,
  type Session,
} from "ironrdp-wasm";

type Props = {
  proxyUrl: string; // ws:// do gateway, rota /rdp/{id}?token=…
  destination: string; // host:porta
  username: string;
  password: string;
  domain?: string;
  onClosed?: () => void;
};

// Inicializa o módulo WASM uma só vez.
let wasmReady: Promise<unknown> | null = null;
function ensureWasm() {
  if (!wasmReady) wasmReady = init();
  return wasmReady;
}

/** JS `KeyboardEvent.code` → scancode PC (set 1; teclas estendidas levam o prefixo 0xE0). */
const SCANCODE: Record<string, number> = {
  Escape: 0x01, Digit1: 0x02, Digit2: 0x03, Digit3: 0x04, Digit4: 0x05, Digit5: 0x06,
  Digit6: 0x07, Digit7: 0x08, Digit8: 0x09, Digit9: 0x0a, Digit0: 0x0b, Minus: 0x0c,
  Equal: 0x0d, Backspace: 0x0e, Tab: 0x0f,
  KeyQ: 0x10, KeyW: 0x11, KeyE: 0x12, KeyR: 0x13, KeyT: 0x14, KeyY: 0x15, KeyU: 0x16,
  KeyI: 0x17, KeyO: 0x18, KeyP: 0x19, BracketLeft: 0x1a, BracketRight: 0x1b,
  Enter: 0x1c, ControlLeft: 0x1d,
  KeyA: 0x1e, KeyS: 0x1f, KeyD: 0x20, KeyF: 0x21, KeyG: 0x22, KeyH: 0x23, KeyJ: 0x24,
  KeyK: 0x25, KeyL: 0x26, Semicolon: 0x27, Quote: 0x28, Backquote: 0x29,
  ShiftLeft: 0x2a, Backslash: 0x2b,
  KeyZ: 0x2c, KeyX: 0x2d, KeyC: 0x2e, KeyV: 0x2f, KeyB: 0x30, KeyN: 0x31, KeyM: 0x32,
  Comma: 0x33, Period: 0x34, Slash: 0x35, ShiftRight: 0x36,
  NumpadMultiply: 0x37, AltLeft: 0x38, Space: 0x39, CapsLock: 0x3a,
  F1: 0x3b, F2: 0x3c, F3: 0x3d, F4: 0x3e, F5: 0x3f, F6: 0x40, F7: 0x41, F8: 0x42,
  F9: 0x43, F10: 0x44, NumLock: 0x45, ScrollLock: 0x46,
  Numpad7: 0x47, Numpad8: 0x48, Numpad9: 0x49, NumpadSubtract: 0x4a, Numpad4: 0x4b,
  Numpad5: 0x4c, Numpad6: 0x4d, NumpadAdd: 0x4e, Numpad1: 0x4f, Numpad2: 0x50,
  Numpad3: 0x51, Numpad0: 0x52, NumpadDecimal: 0x53, F11: 0x57, F12: 0x58,
  NumpadEnter: 0xe01c, ControlRight: 0xe01d, NumpadDivide: 0xe035, PrintScreen: 0xe037,
  AltRight: 0xe038, Home: 0xe047, ArrowUp: 0xe048, PageUp: 0xe049, ArrowLeft: 0xe04b,
  ArrowRight: 0xe04d, End: 0xe04f, ArrowDown: 0xe050, PageDown: 0xe051, Insert: 0xe052,
  Delete: 0xe053, MetaLeft: 0xe05b, MetaRight: 0xe05c, ContextMenu: 0xe05d, Pause: 0xe11d45,
};

/** Liga eventos de rato/teclado do canvas à sessão RDP. Removidos via AbortSignal. */
function attachInput(canvas: HTMLCanvasElement, session: Session, signal: AbortSignal) {
  const apply = (ev: DeviceEvent) => {
    try {
      const tx = new InputTransaction();
      tx.addEvent(ev);
      session.applyInputs(tx);
    } catch {
      /* eventos de rato são frequentes; ignora falhas isoladas */
    }
  };

  canvas.addEventListener("keydown", (e) => {
    const sc = SCANCODE[e.code];
    if (sc == null) return;
    e.preventDefault();
    e.stopPropagation();
    apply(DeviceEvent.keyPressed(sc));
  }, { signal });

  canvas.addEventListener("keyup", (e) => {
    const sc = SCANCODE[e.code];
    if (sc == null) return;
    e.preventDefault();
    e.stopPropagation();
    apply(DeviceEvent.keyReleased(sc));
  }, { signal });

  canvas.addEventListener("mousemove", (e) => {
    const r = canvas.getBoundingClientRect();
    const x = Math.round((e.clientX - r.left) * (canvas.width / r.width));
    const y = Math.round((e.clientY - r.top) * (canvas.height / r.height));
    apply(DeviceEvent.mouseMove(x, y));
  }, { signal });

  canvas.addEventListener("mousedown", (e) => {
    e.preventDefault();
    canvas.focus();
    apply(DeviceEvent.mouseButtonPressed(e.button));
  }, { signal });

  canvas.addEventListener("mouseup", (e) => {
    e.preventDefault();
    apply(DeviceEvent.mouseButtonReleased(e.button));
  }, { signal });

  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    if (e.deltaY !== 0) apply(DeviceEvent.wheelRotations(true, e.deltaY > 0 ? -1 : 1, RotationUnit.Line));
    if (e.deltaX !== 0) apply(DeviceEvent.wheelRotations(false, e.deltaX > 0 ? -1 : 1, RotationUnit.Line));
  }, { signal, passive: false });

  canvas.addEventListener("contextmenu", (e) => e.preventDefault(), { signal });
}

export function RdpView({ proxyUrl, destination, username, password, domain, onClosed }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let session: Session | undefined;
    let cancelled = false;
    const ac = new AbortController();
    setErr(null);
    (async () => {
      await ensureWasm();
      if (cancelled || !canvasRef.current) return;
      const canvas = canvasRef.current;
      const b = new SessionBuilder();
      b.username(username);
      b.password(password);
      if (domain) b.serverDomain(domain);
      b.destination(destination);
      b.proxyAddress(proxyUrl);
      b.authToken("remota"); // o gateway não valida; o WASM exige um token
      b.desktopSize(new DesktopSize(1280, 720));
      b.renderCanvas(canvas);
      b.canvasResizedCallback(() => {});
      b.setCursorStyleCallback(() => {});
      b.setCursorStyleCallbackContext({});
      // Clipboard sharing (CLIPRDR). Remote (Windows) copied → put it on the local clipboard.
      b.remoteClipboardChangedCallback((data: ClipboardData) => {
        try {
          if (data.isEmpty()) return;
          for (const item of data.items() as ClipboardItem[]) {
            if (item.mimeType().startsWith("text/")) {
              const v = item.value();
              if (typeof v === "string" && v) navigator.clipboard?.writeText(v).catch(() => {});
              break;
            }
          }
        } catch {
          /* noop */
        }
      });
      // Remote asks for our clipboard (e.g. before pasting in Windows) → push the local clipboard.
      b.forceClipboardUpdateCallback(async () => {
        try {
          const text = await navigator.clipboard?.readText();
          const data = new ClipboardData();
          if (text) data.addText("text/plain", text);
          await session?.onClipboardPaste(data);
        } catch {
          try {
            await session?.onClipboardPaste(new ClipboardData());
          } catch {
            /* noop */
          }
        }
      });
      b.extension(new Extension("enable_credssp", true)); // NLA/CredSSP
      session = await b.connect();
      if (cancelled) {
        session.shutdown();
        return;
      }
      attachInput(canvas, session, ac.signal);
      canvas.focus();
      session
        .run()
        .then(() => { if (!cancelled) onClosed?.(); })
        .catch(() => { if (!cancelled) onClosed?.(); });
    })().catch((e) => {
      if (cancelled) return;
      // IronError traz backtrace()/message legível — evita "[object Object]".
      const anyE = e as { backtrace?: () => string; message?: string } | undefined;
      let msg: string;
      try {
        msg = anyE?.backtrace?.() || anyE?.message || String(e);
      } catch {
        msg = String(e);
      }
      setErr(msg);
      onClosed?.();
    });
    return () => {
      cancelled = true;
      ac.abort();
      try {
        session?.shutdown();
      } catch {
        /* noop */
      }
    };
  }, [proxyUrl, destination, username, password, domain]);

  return (
    <div style={{ width: "100%", height: "100%", position: "relative", background: "#000" }}>
      <canvas ref={canvasRef} tabIndex={0} style={{ width: "100%", height: "100%", outline: "none" }} />
      {err && (
        <div
          style={{
            position: "absolute",
            top: 8,
            left: 8,
            right: 8,
            color: "#ff6b6b",
            fontFamily: "monospace",
            fontSize: 12,
            background: "rgba(0,0,0,0.65)",
            padding: 8,
            borderRadius: 6,
          }}
        >
          RDP: {err}
        </div>
      )}
    </div>
  );
}
