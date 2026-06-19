//! In-memory registry of online agents (control-channel side).

use std::collections::HashMap;
use serde::Serialize;
use tokio::sync::{mpsc, Mutex};

use remota_proto::RelayMsg;

/// A live agent control connection: metadata + a channel to push messages to its WS writer task.
#[derive(Clone)]
pub struct AgentConn {
    pub name: String,
    pub os: String,
    pub capabilities: Vec<String>,
    pub tx: mpsc::Sender<RelayMsg>,
}

/// Public (debug) view of an online agent, for `GET /agents`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentInfo {
    pub agent_id: String,
    pub name: String,
    pub os: String,
    pub capabilities: Vec<String>,
}

#[derive(Default)]
pub struct Registry {
    inner: Mutex<HashMap<String, AgentConn>>,
}

impl Registry {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }

    pub async fn insert(&self, agent_id: String, conn: AgentConn) {
        self.inner.lock().await.insert(agent_id, conn);
    }

    pub async fn remove(&self, agent_id: &str) {
        self.inner.lock().await.remove(agent_id);
    }

    /// Clone of the message channel for a given agent, if online.
    pub async fn sender(&self, agent_id: &str) -> Option<mpsc::Sender<RelayMsg>> {
        self.inner.lock().await.get(agent_id).map(|c| c.tx.clone())
    }

    pub async fn list(&self) -> Vec<AgentInfo> {
        self.inner
            .lock()
            .await
            .iter()
            .map(|(id, c)| AgentInfo {
                agent_id: id.clone(),
                name: c.name.clone(),
                os: c.os.clone(),
                capabilities: c.capabilities.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_conn(name: &str) -> AgentConn {
        let (tx, _rx) = mpsc::channel(1);
        AgentConn { name: name.into(), os: "linux".into(), capabilities: vec!["cli".into()], tx }
    }

    #[tokio::test]
    async fn insert_list_remove() {
        let reg = Registry::new();
        assert!(reg.list().await.is_empty());

        reg.insert("a1".into(), dummy_conn("pi")).await;
        let list = reg.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].agent_id, "a1");
        assert_eq!(list[0].name, "pi");
        assert!(reg.sender("a1").await.is_some());
        assert!(reg.sender("nope").await.is_none());

        reg.remove("a1").await;
        assert!(reg.list().await.is_empty());
        assert!(reg.sender("a1").await.is_none());
    }
}
