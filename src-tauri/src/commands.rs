use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::gateway::{SessionKind, SessionRegistry, SessionSpec};
use crate::model::{Document, Gateway, Node, Relay};
use crate::settings::Settings;
use crate::update::{delivery, Delivery, RELEASES_URL};
use crate::vault::{load_document, save_document, KdfParams, VaultError, VaultManager};

pub struct AppState {
    pub registry: Arc<SessionRegistry>,
    pub gateway_port: u16,
    pub vault: VaultManager,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub ws_url: String,
    pub kind: String,
}

pub fn build_ws_url(port: u16, spec: &SessionSpec) -> String {
    let route = match spec.kind {
        SessionKind::RawTcp => "session",
        SessionKind::RdpRdcleanpath => "rdp",
        SessionKind::Ssh => "ssh",
    };
    format!("ws://127.0.0.1:{port}/{route}/{}?token={}", spec.id, spec.token)
}

#[tauri::command]
pub fn open_session(
    state: State<AppState>,
    target: String,
    kind: String,
    username: Option<String>,
    password: Option<String>,
    key_path: Option<String>,
    gateway: Option<Gateway>,
    relay: Option<Relay>,
) -> SessionInfo {
    let session_kind = match kind.as_str() {
        "rdp" => SessionKind::RdpRdcleanpath,
        "ssh" => SessionKind::Ssh,
        _ => SessionKind::RawTcp,
    };
    let spec = state
        .registry
        .create_with_creds(target, session_kind, username, password, key_path, gateway, relay);
    SessionInfo {
        ws_url: build_ws_url(state.gateway_port, &spec),
        kind,
    }
}

#[tauri::command]
pub fn unlock_vault(state: State<AppState>, password: String) -> Result<(), VaultError> {
    state.vault.unlock(&password)
}

#[tauri::command]
pub fn lock_vault(state: State<AppState>) {
    state.vault.lock();
}

#[tauri::command]
pub fn vault_exists(state: State<AppState>) -> bool {
    state.vault.vault_exists()
}

#[tauri::command]
pub fn list_tree(state: State<AppState>) -> Result<Document, VaultError> {
    state.vault.tree()
}

#[tauri::command]
pub fn save_connection(
    state: State<AppState>,
    parent_id: Option<String>,
    node: Node,
) -> Result<(), VaultError> {
    state.vault.upsert(parent_id.as_deref(), node)
}

/// Soft-delete: move o nó para a lixeira (restaurável).
#[tauri::command]
pub fn delete_node(state: State<AppState>, id: String) -> Result<(), VaultError> {
    state.vault.delete(&id)
}

/// Restaura um item da lixeira para o sítio original.
#[tauri::command]
pub fn restore_node(state: State<AppState>, id: String) -> Result<(), VaultError> {
    state.vault.restore(&id)
}

/// Apaga definitivamente um item da lixeira.
#[tauri::command]
pub fn delete_forever(state: State<AppState>, id: String) -> Result<(), VaultError> {
    state.vault.delete_forever(&id)
}

/// Esvazia a lixeira.
#[tauri::command]
pub fn empty_trash(state: State<AppState>) -> Result<(), VaultError> {
    state.vault.empty_trash()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub connections: usize,
    pub message: String,
}

#[tauri::command]
pub fn import_mremoteng(state: State<AppState>, path: String) -> Result<ImportReport, VaultError> {
    let xml = std::fs::read_to_string(&path).map_err(|e| VaultError::Io(e.to_string()))?;
    let nodes = crate::importer::parse_confcons(&xml).map_err(|_| VaultError::BadFormat)?;
    let connections = state.vault.import(nodes)?;
    Ok(ImportReport {
        connections,
        message: format!(
            "{connections} connections imported from mRemoteNG. Passwords are left blank (decrypting the encrypted format is the next step)."
        ),
    })
}

/// Exporta a árvore de conexões para um JSON em claro (backup/portabilidade).
/// AVISO: contém as passwords em claro — é uma exportação explícita.
#[tauri::command]
pub fn export_connections(state: State<AppState>, path: String) -> Result<ImportReport, VaultError> {
    let doc = state.vault.tree()?;
    let json = serde_json::to_vec_pretty(&doc).map_err(|e| VaultError::Io(e.to_string()))?;
    std::fs::write(&path, &json).map_err(|e| VaultError::Io(e.to_string()))?;
    let connections = crate::importer::count_connections(&doc.nodes);
    Ok(ImportReport {
        connections,
        message: format!("{connections} connections exported to {path} (plaintext JSON)."),
    })
}

/// Seal the connection tree to `path` with its own `password` (vault format: AES-256-GCM +
/// Argon2id), then re-read and decrypt it to prove the write landed intact. A torn write
/// leaves a file that still sniffs as encrypted and fails only on a later import — catch it
/// here, on the machine that still holds the data. The live vault write path is untouched.
fn write_encrypted_export(path: &Path, password: &str, doc: &Document) -> Result<(), VaultError> {
    let json = serde_json::to_vec(doc).map_err(|e| VaultError::Io(e.to_string()))?;
    save_document(path, password, KdfParams::default(), &json)?;
    let back = load_document(path, password)?;
    if back != json {
        let _ = std::fs::remove_file(path);
        return Err(VaultError::Io("export verification failed".into()));
    }
    Ok(())
}

/// Exporta a árvore de conexões ENCRIPTADA, protegida por uma password própria (independente
/// da senha-mestra). Contraparte segura de `export_connections` (que grava JSON em claro).
#[tauri::command]
pub fn export_connections_encrypted(
    state: State<AppState>,
    path: String,
    password: String,
) -> Result<ImportReport, VaultError> {
    let doc = state.vault.tree()?;
    write_encrypted_export(Path::new(&path), &password, &doc)?;
    let connections = crate::importer::count_connections(&doc.nodes);
    Ok(ImportReport {
        connections,
        message: format!("{connections} connections exported to {path} (encrypted)."),
    })
}

/// Reimporta um JSON exportado pelo Remota (merge na raiz).
#[tauri::command]
pub fn import_remota_json(state: State<AppState>, path: String) -> Result<ImportReport, VaultError> {
    let bytes = std::fs::read(&path).map_err(|e| VaultError::Io(e.to_string()))?;
    let doc: Document = serde_json::from_slice(&bytes).map_err(|_| VaultError::BadFormat)?;
    let connections = state.vault.import(doc.nodes)?;
    Ok(ImportReport {
        connections,
        message: format!("{connections} connections imported from Remota JSON."),
    })
}

/// What Remota may do about a newer version, and whether it is allowed to look.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePolicy {
    pub delivery: Delivery,
    pub enabled: bool,
    pub releases_url: String,
    pub current_version: String,
}

/// Read before the vault is unlocked, because that is when it is needed: whoever has not opened
/// Remota in a month is exactly the person who has not heard about the release that fixed
/// something, and they should not have to unlock a vault to find out.
#[tauri::command]
pub fn update_policy() -> UpdatePolicy {
    UpdatePolicy {
        delivery: delivery(),
        enabled: Settings::load().check_for_updates,
        releases_url: RELEASES_URL.to_owned(),
        current_version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
pub fn set_update_check(enabled: bool) -> Result<bool, String> {
    let mut s = Settings::load();
    s.check_for_updates = enabled;
    s.save()?;
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::{SessionKind, SessionSpec};

    #[test]
    fn ws_url_vnc_usa_rota_session() {
        let spec = SessionSpec {
            id: "abc".into(),
            token: "tok".into(),
            target: "10.0.0.5:5900".into(),
            kind: SessionKind::RawTcp,
            username: None,
            password: None,
            key_path: None,
            gateway: None,
            relay: None,
        };
        assert_eq!(
            build_ws_url(7000, &spec),
            "ws://127.0.0.1:7000/session/abc?token=tok"
        );
    }

    #[test]
    fn ws_url_ssh_usa_rota_ssh() {
        let spec = SessionSpec {
            id: "s1".into(),
            token: "tk".into(),
            target: "10.0.0.9:22".into(),
            kind: SessionKind::Ssh,
            username: Some("root".into()),
            password: Some("x".into()),
            key_path: None,
            gateway: None,
            relay: None,
        };
        assert_eq!(
            build_ws_url(7000, &spec),
            "ws://127.0.0.1:7000/ssh/s1?token=tk"
        );
    }

    #[test]
    fn ws_url_rdp_usa_rota_rdp() {
        let spec = SessionSpec {
            id: "xyz".into(),
            token: "t2".into(),
            target: "win:3389".into(),
            kind: SessionKind::RdpRdcleanpath,
            username: None,
            password: None,
            key_path: None,
            gateway: None,
            relay: None,
        };
        assert_eq!(
            build_ws_url(7000, &spec),
            "ws://127.0.0.1:7000/rdp/xyz?token=t2"
        );
    }

    use crate::model::{Connection, Credentials, Protocol};
    use crate::vault::file::{is_encrypted, load_document};

    fn tmp_export(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("remota-export-{name}.dat"));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn doc_with_password(pw: &str) -> Document {
        Document {
            nodes: vec![Node::Connection {
                id: "c1".into(),
                name: "web".into(),
                conn: Connection {
                    protocol: Protocol::Ssh,
                    host: "10.0.0.1".into(),
                    port: None,
                    credentials: Credentials {
                        username: Some("root".into()),
                        password: Some(pw.into()),
                        ..Default::default()
                    },
                    gateway: None,
                    relay: None,
                    always_prompt: false,
                },
            }],
            trash: vec![],
        }
    }

    #[test]
    fn write_encrypted_export_roundtrips() {
        let path = tmp_export("roundtrip");
        let doc = doc_with_password("pw-secret");
        write_encrypted_export(&path, "P", &doc).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(is_encrypted(&bytes), "exported file must be encrypted");
        let plain = load_document(&path, "P").unwrap();
        assert_eq!(plain, serde_json::to_vec(&doc).unwrap(), "decrypts to the same tree");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_encrypted_export_hides_cleartext() {
        let path = tmp_export("hide");
        let doc = doc_with_password("hunter2-secret");
        write_encrypted_export(&path, "P", &doc).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let needle = b"hunter2-secret";
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle),
            "the password must not appear in cleartext in the exported bytes"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_encrypted_export_rejects_tampered_file() {
        let path = tmp_export("tamper");
        let doc = doc_with_password("pw");
        write_encrypted_export(&path, "P", &doc).unwrap();
        // Flip a byte in the ciphertext region (past the 41-byte header).
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            matches!(load_document(&path, "P"), Err(VaultError::Crypto)),
            "a corrupt ciphertext must fail to decrypt"
        );
        std::fs::remove_file(&path).ok();
    }
}
