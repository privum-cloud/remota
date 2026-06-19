use roxmltree::Document as XmlDoc;
use uuid::Uuid;

use crate::model::{Connection, Credentials, Node, Protocol};

fn map_protocol(p: &str) -> Protocol {
    match p.to_ascii_uppercase().as_str() {
        "RDP" => Protocol::Rdp,
        "VNC" => Protocol::Vnc,
        "TELNET" => Protocol::Telnet,
        _ => Protocol::Ssh, // SSH1/SSH2/IntApp/... → trata como SSH
    }
}

/// Parse do `confCons.xml` do mRemoteNG → árvore de nós do Remota.
/// NOTA: as passwords (cifradas AES-GCM pelo mRemoteNG) NÃO são decifradas nesta
/// versão — entram vazias. Decifragem fica para afinar com um ficheiro real.
pub fn parse_confcons(xml: &str) -> Result<Vec<Node>, String> {
    let doc = XmlDoc::parse(xml).map_err(|e| e.to_string())?;
    Ok(parse_children(doc.root_element()))
}

fn parse_children(parent: roxmltree::Node) -> Vec<Node> {
    let mut out = Vec::new();
    for el in parent.children().filter(|n| n.is_element() && n.has_tag_name("Node")) {
        let name = el.attribute("Name").unwrap_or("(sem nome)").to_string();
        let typ = el.attribute("Type").unwrap_or("");

        if typ.eq_ignore_ascii_case("Container") {
            out.push(Node::Folder {
                id: Uuid::new_v4().to_string(),
                name,
                defaults: Credentials::default(),
                gateway: None,
                children: parse_children(el),
            });
        } else {
            let host = el.attribute("Hostname").unwrap_or("").to_string();
            let protocol = map_protocol(el.attribute("Protocol").unwrap_or("SSH2"));
            let port = el.attribute("Port").and_then(|s| s.parse::<u16>().ok());
            let username = el.attribute("Username").filter(|s| !s.is_empty()).map(String::from);
            let domain = el.attribute("Domain").filter(|s| !s.is_empty()).map(String::from);
            out.push(Node::Connection {
                id: Uuid::new_v4().to_string(),
                name,
                conn: Connection {
                    protocol,
                    host,
                    port,
                    credentials: Credentials { username, password: None, domain },
                    gateway: None,
                },
            });
        }
    }
    out
}

/// Conta as conexões (folhas) numa árvore — para o relatório de import.
pub fn count_connections(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Connection { .. } => 1,
            Node::Folder { children, .. } => count_connections(children),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Connections Name="Connections" ConfVersion="2.6">
  <Node Name="Prod" Type="Container">
    <Node Name="web" Type="Connection" Hostname="10.0.0.1" Protocol="SSH2" Port="22" Username="root" Password="enc" Domain="" />
    <Node Name="win" Type="Connection" Hostname="10.0.0.2" Protocol="RDP" Port="3389" Username="admin" Domain="CORP" />
  </Node>
  <Node Name="solo" Type="Connection" Hostname="10.0.0.9" Protocol="VNC" Port="5900" />
</Connections>"#;

    #[test]
    fn parses_folders_and_connections() {
        let nodes = parse_confcons(SAMPLE).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(count_connections(&nodes), 3);

        match &nodes[0] {
            Node::Folder { name, children, .. } => {
                assert_eq!(name, "Prod");
                assert_eq!(children.len(), 2);
                match &children[1] {
                    Node::Connection { conn, .. } => {
                        assert_eq!(conn.protocol, Protocol::Rdp);
                        assert_eq!(conn.host, "10.0.0.2");
                        assert_eq!(conn.port, Some(3389));
                        assert_eq!(conn.credentials.domain.as_deref(), Some("CORP"));
                    }
                    _ => panic!("esperava conexão"),
                }
            }
            _ => panic!("esperava pasta"),
        }

        match &nodes[1] {
            Node::Connection { name, conn, .. } => {
                assert_eq!(name, "solo");
                assert_eq!(conn.protocol, Protocol::Vnc);
                assert_eq!(conn.port, Some(5900));
            }
            _ => panic!("esperava conexão solo"),
        }
    }
}
