import type { Gateway } from "../lib/vaultApi";
import { open } from "@tauri-apps/plugin-dialog";
import { colors, ghostBtn, input, label } from "./styles";

export type GwForm = { host: string; port: string; username: string; password: string; keyPath: string };
export const emptyGw: GwForm = { host: "", port: "", username: "", password: "", keyPath: "" };

export function gwFromModel(g?: Gateway): GwForm {
  return {
    host: g?.host ?? "",
    port: g?.port ? String(g.port) : "",
    username: g?.username ?? "",
    password: g?.password ?? "",
    keyPath: g?.key_path ?? "",
  };
}

export function gwToModel(f: GwForm): Gateway | undefined {
  if (!f.host.trim()) return undefined;
  return {
    host: f.host.trim(),
    port: f.port ? Number(f.port) : undefined,
    username: f.username || undefined,
    password: f.password || undefined,
    key_path: f.keyPath || undefined,
  };
}

export function GatewaySection({ value, onChange, hint }: { value: GwForm; onChange: (v: GwForm) => void; hint?: string }) {
  const set = (k: keyof GwForm, val: string) => onChange({ ...value, [k]: val });
  const browseKey = async () => {
    const p = await open({ multiple: false, title: "Select SSH private key (jump)" });
    if (typeof p === "string") set("keyPath", p);
  };
  return (
    <div style={{ borderTop: `1px solid ${colors.border}`, paddingTop: 12 }}>
      <div style={{ ...label, marginBottom: 6 }}>Gateway / Jump host (SSH)</div>
      <div style={{ color: colors.dim, fontSize: 11, marginBottom: 8 }}>
        {hint ?? "Connect to the target tunneled through this host (empty = direct). Inherited by connections in the folder."}
      </div>
      <div style={{ display: "flex", gap: 10 }}>
        <input
          style={input}
          value={value.host}
          onChange={(e) => set("host", e.target.value)}
          placeholder="jump host (e.g. 192.168.1.242)"
        />
        <input
          style={{ ...input, width: 90 }}
          value={value.port}
          onChange={(e) => set("port", e.target.value.replace(/[^0-9]/g, ""))}
          placeholder="22"
        />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 10 }}>
        <input style={input} value={value.username} onChange={(e) => set("username", e.target.value)} placeholder="jump username" />
        <input style={input} type="password" value={value.password} onChange={(e) => set("password", e.target.value)} placeholder="jump password" />
        <div style={{ display: "flex", gap: 8 }}>
          <input style={input} value={value.keyPath} onChange={(e) => set("keyPath", e.target.value)} placeholder="jump SSH key (path, optional)" />
          <button type="button" style={ghostBtn} onClick={browseKey} title="Pick a private key file">Browse…</button>
        </div>
      </div>
    </div>
  );
}
