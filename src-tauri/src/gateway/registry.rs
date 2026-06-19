use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum SessionKind {
    RawTcp,
    RdpRdcleanpath,
    Ssh,
}

#[derive(Clone, Debug)]
pub struct SessionSpec {
    pub id: String,
    pub token: String,
    pub target: String,
    pub kind: SessionKind,
    /// Credenciais para protocolos onde o gateway autentica (SSH). `None` p/ bridge cru.
    pub username: Option<String>,
    pub password: Option<String>,
    /// Caminho da chave privada SSH (auth por chave, prioritária sobre password).
    pub key_path: Option<String>,
    /// Jump host (SSH ProxyJump): se presente, liga ao destino tunelado por ele.
    pub gateway: Option<crate::model::Gateway>,
}

#[derive(Default)]
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionSpec>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, target: String, kind: SessionKind) -> SessionSpec {
        self.create_with_creds(target, kind, None, None, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_with_creds(
        &self,
        target: String,
        kind: SessionKind,
        username: Option<String>,
        password: Option<String>,
        key_path: Option<String>,
        gateway: Option<crate::model::Gateway>,
    ) -> SessionSpec {
        let spec = SessionSpec {
            id: Uuid::new_v4().to_string(),
            token: Uuid::new_v4().to_string(),
            target,
            kind,
            username,
            password,
            key_path,
            gateway,
        };
        self.sessions
            .lock()
            .unwrap()
            .insert(spec.id.clone(), spec.clone());
        spec
    }

    /// Valida id+token e **remove** a sessão (uso único). `None` se não casar.
    pub fn consume(&self, id: &str, token: &str) -> Option<SessionSpec> {
        let mut map = self.sessions.lock().unwrap();
        match map.get(id) {
            Some(s) if s.token == token => map.remove(id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_returns_distinct_id_and_token() {
        let reg = SessionRegistry::new();
        let a = reg.create("127.0.0.1:5900".into(), SessionKind::RawTcp);
        let b = reg.create("127.0.0.1:5900".into(), SessionKind::RawTcp);
        assert_ne!(a.id, b.id);
        assert_ne!(a.token, b.token);
        assert_eq!(a.kind, SessionKind::RawTcp);
    }

    #[test]
    fn consume_succeeds_once_then_fails() {
        let reg = SessionRegistry::new();
        let s = reg.create("host:3389".into(), SessionKind::RdpRdcleanpath);
        assert!(reg.consume(&s.id, &s.token).is_some());
        assert!(reg.consume(&s.id, &s.token).is_none(), "token deve ser uso único");
    }

    #[test]
    fn consume_rejects_wrong_token() {
        let reg = SessionRegistry::new();
        let s = reg.create("host:23".into(), SessionKind::RawTcp);
        assert!(reg.consume(&s.id, "token-errado").is_none());
    }
}
