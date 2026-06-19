pub mod inherit;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Ssh,
    Rdp,
    Vnc,
    Telnet,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Caminho para a chave privada SSH (auth por chave). Tem prioridade sobre a password no SSH.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

/// Relay self-hosted (remota-relay): liga ao destino através de um agente atrás de NAT.
/// Se presente numa conexão, o gateway usa o túnel wss do relay em vez de TCP direto.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Relay {
    /// URL base do relay, ex. `wss://relay.privum.cloud`.
    pub url: String,
    /// Id do agente alvo (registado no relay).
    pub agent_id: String,
}

/// Jump host (SSH ProxyJump): liga ao destino TUNELADO através deste host.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Gateway {
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub protocol: Protocol,
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default)]
    pub credentials: Credentials,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<Gateway>,
    /// Relay self-hosted (NAT traversal). Se presente, a sessão vai pelo túnel do relay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<Relay>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
    Folder {
        id: String,
        name: String,
        #[serde(default)]
        defaults: Credentials,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gateway: Option<Gateway>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default)]
        children: Vec<Node>,
    },
    Connection {
        id: String,
        name: String,
        conn: Connection,
    },
}

pub fn node_id(node: &Node) -> &str {
    match node {
        Node::Folder { id, .. } => id,
        Node::Connection { id, .. } => id,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub nodes: Vec<Node>,
}

impl Document {
    pub fn empty() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn find<'a>(&'a self, id: &str) -> Option<&'a Node> {
        fn walk<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
            for n in nodes {
                if node_id(n) == id {
                    return Some(n);
                }
                if let Node::Folder { children, .. } = n {
                    if let Some(found) = walk(children, id) {
                        return Some(found);
                    }
                }
            }
            None
        }
        walk(&self.nodes, id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        fn walk(nodes: &mut Vec<Node>, id: &str) -> bool {
            if let Some(pos) = nodes.iter().position(|n| node_id(n) == id) {
                nodes.remove(pos);
                return true;
            }
            for n in nodes.iter_mut() {
                if let Node::Folder { children, .. } = n {
                    if walk(children, id) {
                        return true;
                    }
                }
            }
            false
        }
        walk(&mut self.nodes, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_conn(host: &str) -> Connection {
        Connection {
            protocol: Protocol::Ssh,
            host: host.into(),
            port: None,
            credentials: Credentials::default(),
            gateway: None,
            relay: None,
        }
    }

    #[test]
    fn document_json_roundtrips() {
        let doc = Document {
            nodes: vec![Node::Folder {
                id: "f1".into(),
                name: "Prod".into(),
                defaults: Credentials { username: Some("root".into()), ..Default::default() },
                gateway: None,
                icon: None,
                children: vec![Node::Connection {
                    id: "c1".into(),
                    name: "web".into(),
                    conn: sample_conn("10.0.0.1"),
                }],
            }],
        };
        let json = serde_json::to_vec(&doc).unwrap();
        let back: Document = serde_json::from_slice(&json).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn find_locates_nested_node() {
        let doc = Document {
            nodes: vec![Node::Folder {
                id: "f1".into(),
                name: "Prod".into(),
                defaults: Credentials::default(),
                gateway: None,
                icon: None,
                children: vec![Node::Connection {
                    id: "c1".into(),
                    name: "web".into(),
                    conn: sample_conn("10.0.0.1"),
                }],
            }],
        };
        assert!(doc.find("c1").is_some());
        assert!(doc.find("missing").is_none());
    }

    #[test]
    fn remove_deletes_nested_node() {
        let mut doc = Document {
            nodes: vec![Node::Folder {
                id: "f1".into(),
                name: "Prod".into(),
                defaults: Credentials::default(),
                gateway: None,
                icon: None,
                children: vec![Node::Connection {
                    id: "c1".into(),
                    name: "web".into(),
                    conn: sample_conn("10.0.0.1"),
                }],
            }],
        };
        assert!(doc.remove("c1"));
        assert!(doc.find("c1").is_none());
        assert!(!doc.remove("c1"));
    }
}
