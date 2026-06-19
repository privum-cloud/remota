import type { Gateway } from "../lib/vaultApi";
import { colors, input, label } from "./styles";

export type GwForm = { host: string; port: string; username: string; password: string };
export const emptyGw: GwForm = { host: "", port: "", username: "", password: "" };

export function gwFromModel(g?: Gateway): GwForm {
  return {
    host: g?.host ?? "",
    port: g?.port ? String(g.port) : "",
    username: g?.username ?? "",
    password: g?.password ?? "",
  };
}

export function gwToModel(f: GwForm): Gateway | undefined {
  if (!f.host.trim()) return undefined;
  return {
    host: f.host.trim(),
    port: f.port ? Number(f.port) : undefined,
    username: f.username || undefined,
    password: f.password || undefined,
  };
}

export function GatewaySection({ value, onChange, hint }: { value: GwForm; onChange: (v: GwForm) => void; hint?: string }) {
  const set = (k: keyof GwForm, val: string) => onChange({ ...value, [k]: val });
  return (
    <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
      <div style={{ ...label, marginBottom: 6 }}>Gateway / Jump host (SSH)</div>
      <div style={{ color: colors.dim, fontSize: 11, marginBottom: 8 }}>
        {hint ?? "Liga ao destino tunelado por este host (vazio = direto). Herdável pelas conexões dentro da pasta."}
      </div>
      <div style={{ display: "flex", gap: 10 }}>
        <input
          style={input}
          value={value.host}
          onChange={(e) => set("host", e.target.value)}
          placeholder="jump host (ex.: 192.168.1.242)"
        />
        <input
          style={{ ...input, width: 90 }}
          value={value.port}
          onChange={(e) => set("port", e.target.value.replace(/[^0-9]/g, ""))}
          placeholder="22"
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 10 }}>
        <input style={input} value={value.username} onChange={(e) => set("username", e.target.value)} placeholder="utilizador do jump" />
        <input style={input} type="password" value={value.password} onChange={(e) => set("password", e.target.value)} placeholder="senha do jump" />
      </div>
    </div>
  );
}
