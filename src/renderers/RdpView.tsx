import { useEffect, useRef, useState } from "react";
import init, { SessionBuilder, DesktopSize, Extension } from "ironrdp-wasm";

type Props = {
  proxyUrl: string; // ws:// do gateway, rota /rdp/{id}?token=…
  destination: string; // host:porta
  username: string;
  password: string;
  domain?: string;
};

// Inicializa o módulo WASM uma só vez.
let wasmReady: Promise<unknown> | null = null;
function ensureWasm() {
  // Sem argumento: o glue do wasm-bindgen resolve o .wasm via new URL(..., import.meta.url),
  // que o Vite emite como asset automaticamente.
  if (!wasmReady) wasmReady = init();
  return wasmReady;
}

export function RdpView({ proxyUrl, destination, username, password, domain }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let session: { run: () => void; shutdown?: () => void } | undefined;
    let cancelled = false;
    setErr(null);
    (async () => {
      await ensureWasm();
      if (cancelled || !canvasRef.current) return;
      const b = new SessionBuilder();
      b.username(username);
      b.password(password);
      if (domain) b.serverDomain(domain);
      b.destination(destination);
      b.proxyAddress(proxyUrl);
      // ironrdp-web (Devolutions model) requires a proxy auth token in the RDCleanPath request.
      // Our gateway doesn't validate it, so any non-empty placeholder satisfies the client.
      b.authToken("remota");
      b.desktopSize(new DesktopSize(1280, 720));
      b.renderCanvas(canvasRef.current);
      b.extension(new Extension("enable_credssp", true)); // NLA/CredSSP
      session = await b.connect();
      session.run();
    })().catch((e) => {
      if (cancelled) return;
      // ironrdp's IronError carries a readable backtrace()/message — avoid "[object Object]".
      const anyE = e as { backtrace?: () => string; message?: string } | undefined;
      let msg: string;
      try {
        msg = anyE?.backtrace?.() || anyE?.message || String(e);
      } catch {
        msg = String(e);
      }
      setErr(msg);
    });
    return () => {
      cancelled = true;
      try {
        session?.shutdown?.();
      } catch {
        /* noop */
      }
    };
  }, [proxyUrl, destination, username, password, domain]);

  return (
    <div style={{ width: "100%", height: "100%", position: "relative", background: "#000" }}>
      <canvas ref={canvasRef} style={{ width: "100%", height: "100%" }} />
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
