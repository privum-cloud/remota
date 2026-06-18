use crate::model::{Connection, Credentials};

pub fn merge_credentials(parent: &Credentials, child: &Credentials) -> Credentials {
    Credentials {
        username: child.username.clone().or_else(|| parent.username.clone()),
        password: child.password.clone().or_else(|| parent.password.clone()),
        domain: child.domain.clone().or_else(|| parent.domain.clone()),
    }
}

pub fn resolve_effective(folder_chain: &[Credentials], conn: &Connection) -> Connection {
    let mut acc = Credentials::default();
    for defaults in folder_chain {
        // defaults mais profundos sobrescrevem os mais rasos
        acc = merge_credentials(&acc, defaults);
    }
    let effective = merge_credentials(&acc, &conn.credentials);
    Connection {
        protocol: conn.protocol,
        host: conn.host.clone(),
        port: conn.port,
        credentials: effective,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Connection, Credentials, Protocol};

    fn creds(u: Option<&str>, p: Option<&str>, d: Option<&str>) -> Credentials {
        Credentials {
            username: u.map(String::from),
            password: p.map(String::from),
            domain: d.map(String::from),
        }
    }

    #[test]
    fn child_field_overrides_parent() {
        let parent = creds(Some("root"), Some("p1"), Some("CORP"));
        let child = creds(Some("admin"), None, None);
        let merged = merge_credentials(&parent, &child);
        assert_eq!(merged.username.as_deref(), Some("admin")); // override
        assert_eq!(merged.password.as_deref(), Some("p1")); // herdado
        assert_eq!(merged.domain.as_deref(), Some("CORP")); // herdado
    }

    #[test]
    fn resolve_applies_folder_chain_then_connection() {
        let chain = vec![
            creds(Some("root"), None, Some("CORP")), // pasta raiz
            creds(None, Some("segredo"), None),      // subpasta
        ];
        let conn = Connection {
            protocol: Protocol::Rdp,
            host: "win".into(),
            port: None,
            credentials: creds(Some("administrator"), None, None),
        };
        let eff = resolve_effective(&chain, &conn);
        assert_eq!(eff.credentials.username.as_deref(), Some("administrator"));
        assert_eq!(eff.credentials.password.as_deref(), Some("segredo"));
        assert_eq!(eff.credentials.domain.as_deref(), Some("CORP"));
        assert_eq!(eff.host, "win");
    }
}
