import { invoke } from "@tauri-apps/api/core";

// Tipos espelhando o modelo serde do backend Rust (src-tauri/src/model).
export type Protocol = "ssh" | "rdp" | "vnc" | "telnet";

export interface Credentials {
  username?: string;
  password?: string;
  domain?: string;
}

/** Jump host (SSH ProxyJump): tunela a sessão por este host. */
export interface Gateway {
  host: string;
  port?: number;
  username?: string;
  password?: string;
}

export interface Connection {
  protocol: Protocol;
  host: string;
  port?: number;
  credentials: Credentials;
  gateway?: Gateway;
}

// Enum internamente-tageado (#[serde(tag = "type")]) do Rust.
export type Node =
  | {
      type: "folder";
      id: string;
      name: string;
      defaults: Credentials;
      gateway?: Gateway;
      icon?: string;
      children: Node[];
    }
  | { type: "connection"; id: string; name: string; conn: Connection };

export interface Document {
  nodes: Node[];
}

// Wrappers tipados dos comandos Tauri (Tauri v2 converte snake_case→camelCase nos args).
export const vaultApi = {
  exists: () => invoke<boolean>("vault_exists"),
  unlock: (password: string) => invoke<void>("unlock_vault", { password }),
  lock: () => invoke<void>("lock_vault"),
  listTree: () => invoke<Document>("list_tree"),
  saveConnection: (parentId: string | null, node: Node) =>
    invoke<void>("save_connection", { parentId, node }),
  deleteNode: (id: string) => invoke<void>("delete_node", { id }),
  importMremoteng: (path: string) =>
    invoke<{ connections: number; message: string }>("import_mremoteng", { path }),
  exportConnections: (path: string) =>
    invoke<{ connections: number; message: string }>("export_connections", { path }),
  importRemotaJson: (path: string) =>
    invoke<{ connections: number; message: string }>("import_remota_json", { path }),
};
