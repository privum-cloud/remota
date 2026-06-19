import type { Connection, Credentials } from "./vaultApi";

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
