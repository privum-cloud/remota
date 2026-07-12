declare module "@novnc/novnc" {
  /** Subconjunto mínimo da API do noVNC RFB usado pelo Remota. */
  export default class RFB {
    constructor(target: Element, url: string, options?: Record<string, unknown>);
    scaleViewport: boolean;
    background: string;
    disconnect(): void;
    sendCredentials(creds: { password?: string; username?: string }): void;
    addEventListener(type: string, listener: (e: Event) => void): void;
    removeEventListener(type: string, listener: (e: Event) => void): void;
  }
}
