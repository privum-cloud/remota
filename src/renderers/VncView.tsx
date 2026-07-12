import { useEffect, useRef } from "react";
// @novnc/novnc não traz tipos TS — declaração em src/types/novnc.d.ts.
// O pacote exporta só a raiz (exports: "./core/rfb.js"); deep import não é permitido.
import RFB from "@novnc/novnc";

type Props = {
  wsUrl: string;
  /** Senha VNC, se o servidor exigir auth. */
  password?: string;
  /** Chamado quando a sessão cai/termina (indicador vermelho na aba). */
  onClosed?: () => void;
};

export function VncView({ wsUrl, password, onClosed }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const options: { credentials?: { password: string } } = {};
    if (password) options.credentials = { password };
    const rfb = new RFB(el, wsUrl, options);
    rfb.scaleViewport = true;
    rfb.background = "#000";
    const onDisconnect = () => onClosed?.();
    rfb.addEventListener("disconnect", onDisconnect);
    return () => {
      rfb.removeEventListener("disconnect", onDisconnect);
      rfb.disconnect();
    };
  }, [wsUrl, password]);

  return <div ref={containerRef} style={{ width: "100%", height: "100%" }} />;
}
