import type { Connection, Credentials, Gateway } from "./vaultApi";

/** Gateway efetivo: o da conexão, senão o da pasta mais próxima que o define. */
export function resolveGateway(folderGateways: (Gateway | undefined)[], conn: Connection): Gateway | undefined {
  if (conn.gateway) return conn.gateway;
  for (let i = folderGateways.length - 1; i >= 0; i--) {
    if (folderGateways[i]) return folderGateways[i];
  }
  return undefined;
}

/** Campo do filho vence se definido; senão herda do pai. Espelha o resolve do backend. */
export function mergeCreds(parent: Credentials, child: Credentials): Credentials {
  return {
    username: child.username ?? parent.username,
    password: child.password ?? parent.password,
    domain: child.domain ?? parent.domain,
  };
}

/** Aplica os defaults das pastas (raiz→pai) e depois as creds da conexão. */
export function resolveCreds(chain: Credentials[], conn: Connection): Credentials {
  let acc: Credentials = {};
  for (const defaults of chain) acc = mergeCreds(acc, defaults);
  return mergeCreds(acc, conn.credentials);
}
