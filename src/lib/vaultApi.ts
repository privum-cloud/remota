import { invoke } from "@tauri-apps/api/core";

// Tipos espelhando o modelo serde do backend Rust (src-tauri/src/model).
export type Protocol = "ssh" | "rdp" | "vnc" | "telnet";

export interface Credentials {
  username?: string;
  password?: string;
  domain?: string;
  key_path?: string;
}

/** Jump host (SSH ProxyJump): tunela a sessão por este host. */
export interface Gateway {
  host: string;
  port?: number;
  username?: string;
  password?: string;
  key_path?: string;
}

/** Relay self-hosted (NAT traversal): liga ao destino via um agente atrás de NAT. */
export interface Relay {
  url: string; // wss://relay.privum.cloud
  agent_id: string;
}

export interface Connection {
  protocol: Protocol;
  host: string;
  port?: number;
  credentials: Credentials;
  gateway?: Gateway;
  relay?: Relay;
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

/** Item na lixeira: o nó removido + o id da pasta-pai original. */
export interface TrashEntry {
  node: Node;
  parent_id?: string;
}

export interface Document {
  nodes: Node[];
  trash: TrashEntry[];
}

/** What installing a new version costs the user: nothing, or an administrator password. */
export type UpdateDelivery = "self_install" | "needs_admin";

export interface UpdatePolicy {
  delivery: UpdateDelivery;
  enabled: boolean;
  releasesUrl: string;
  currentVersion: string;
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
  restoreNode: (id: string) => invoke<void>("restore_node", { id }),
  deleteForever: (id: string) => invoke<void>("delete_forever", { id }),
  emptyTrash: () => invoke<void>("empty_trash"),
  importMremoteng: (path: string) =>
    invoke<{ connections: number; message: string }>("import_mremoteng", { path }),
  exportConnections: (path: string) =>
    invoke<{ connections: number; message: string }>("export_connections", { path }),
  importRemotaJson: (path: string) =>
    invoke<{ connections: number; message: string }>("import_remota_json", { path }),
  updatePolicy: () => invoke<UpdatePolicy>("update_policy"),
  setUpdateCheck: (enabled: boolean) => invoke<boolean>("set_update_check", { enabled }),
};
