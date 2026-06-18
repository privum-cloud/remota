use std::path::PathBuf;
use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::model::{node_id, Document, Node};
use crate::vault::{load_document, save_document, KdfParams, VaultError};

struct Unlocked {
    password: Zeroizing<String>,
    doc: Document,
}

pub struct VaultManager {
    path: PathBuf,
    inner: Mutex<Option<Unlocked>>,
}

impl VaultManager {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            inner: Mutex::new(None),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }

    /// `true` se já existe um cofre em disco. Permite ao UI distinguir
    /// "definir senha mestra" (primeiro uso) de "destravar" (cofre existente).
    pub fn vault_exists(&self) -> bool {
        self.path.exists()
    }

    pub fn unlock(&self, password: &str) -> Result<(), VaultError> {
        let doc = if self.path.exists() {
            let bytes = load_document(&self.path, password)?;
            serde_json::from_slice::<Document>(&bytes).map_err(|_| VaultError::BadFormat)?
        } else {
            let doc = Document::empty();
            self.persist(password, &doc)?;
            doc
        };
        *self.inner.lock().unwrap() = Some(Unlocked {
            password: Zeroizing::new(password.to_string()),
            doc,
        });
        Ok(())
    }

    pub fn lock(&self) {
        *self.inner.lock().unwrap() = None;
    }

    pub fn tree(&self) -> Result<Document, VaultError> {
        let guard = self.inner.lock().unwrap();
        let u = guard.as_ref().ok_or(VaultError::Locked)?;
        Ok(u.doc.clone())
    }

    pub fn upsert(&self, parent_id: Option<&str>, node: Node) -> Result<(), VaultError> {
        let mut guard = self.inner.lock().unwrap();
        let u = guard.as_mut().ok_or(VaultError::Locked)?;
        // remove qualquer nó com o mesmo id (upsert), depois insere
        u.doc.remove(node_id(&node));
        insert_into(&mut u.doc.nodes, parent_id, node)?;
        let password = u.password.clone();
        let doc = u.doc.clone();
        drop(guard);
        self.persist(&password, &doc)
    }

    pub fn delete(&self, id: &str) -> Result<(), VaultError> {
        let mut guard = self.inner.lock().unwrap();
        let u = guard.as_mut().ok_or(VaultError::Locked)?;
        u.doc.remove(id);
        let password = u.password.clone();
        let doc = u.doc.clone();
        drop(guard);
        self.persist(&password, &doc)
    }

    fn persist(&self, password: &str, doc: &Document) -> Result<(), VaultError> {
        let bytes = serde_json::to_vec(doc).map_err(|e| VaultError::Io(e.to_string()))?;
        save_document(&self.path, password, KdfParams::default(), &bytes)
    }
}

fn insert_into(
    nodes: &mut Vec<Node>,
    parent_id: Option<&str>,
    node: Node,
) -> Result<(), VaultError> {
    match parent_id {
        None => {
            nodes.push(node);
            Ok(())
        }
        Some(pid) => {
            for n in nodes.iter_mut() {
                if let Node::Folder { id, children, .. } = n {
                    if id == pid {
                        children.push(node);
                        return Ok(());
                    }
                    if insert_into(children, Some(pid), node.clone()).is_ok() {
                        return Ok(());
                    }
                }
            }
            Err(VaultError::BadFormat) // pasta-pai não encontrada
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Connection, Credentials, Node, Protocol};

    fn tmp(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("remota-mgr-{name}.dat"));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn conn_node(id: &str, host: &str) -> Node {
        Node::Connection {
            id: id.into(),
            name: id.into(),
            conn: Connection {
                protocol: Protocol::Ssh,
                host: host.into(),
                port: None,
                credentials: Credentials::default(),
            },
        }
    }

    #[test]
    fn unlock_creates_empty_vault_on_first_use() {
        let mgr = VaultManager::new(tmp("first"));
        assert!(!mgr.is_unlocked());
        mgr.unlock("mestra").unwrap();
        assert!(mgr.is_unlocked());
        assert!(mgr.tree().unwrap().nodes.is_empty());
    }

    #[test]
    fn upsert_persists_and_reloads_with_password() {
        let path = tmp("persist");
        let mgr = VaultManager::new(path.clone());
        mgr.unlock("mestra").unwrap();
        mgr.upsert(None, conn_node("c1", "10.0.0.1")).unwrap();
        mgr.lock();
        // novo manager no mesmo arquivo, mesma senha
        let mgr2 = VaultManager::new(path);
        mgr2.unlock("mestra").unwrap();
        assert!(mgr2.tree().unwrap().find("c1").is_some());
    }

    #[test]
    fn wrong_password_on_existing_vault_fails() {
        let path = tmp("wrongpw");
        let mgr = VaultManager::new(path.clone());
        mgr.unlock("mestra").unwrap();
        mgr.upsert(None, conn_node("c1", "h")).unwrap();
        mgr.lock();
        let mgr2 = VaultManager::new(path);
        assert!(matches!(mgr2.unlock("errada"), Err(VaultError::Crypto)));
    }

    #[test]
    fn delete_removes_and_persists() {
        let path = tmp("del");
        let mgr = VaultManager::new(path);
        mgr.unlock("m").unwrap();
        mgr.upsert(None, conn_node("c1", "h")).unwrap();
        mgr.delete("c1").unwrap();
        assert!(mgr.tree().unwrap().find("c1").is_none());
    }

    #[test]
    fn tree_when_locked_errs() {
        let mgr = VaultManager::new(tmp("locked"));
        assert!(matches!(mgr.tree(), Err(VaultError::Locked)));
    }

    #[test]
    fn vault_exists_reflects_file_presence() {
        let mgr = VaultManager::new(tmp("exists"));
        assert!(!mgr.vault_exists(), "sem ficheiro ainda");
        mgr.unlock("m").unwrap();
        assert!(mgr.vault_exists(), "unlock no primeiro uso cria o ficheiro");
    }
}
