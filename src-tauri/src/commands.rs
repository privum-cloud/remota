use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::gateway::{SessionKind, SessionRegistry, SessionSpec};
use crate::model::{Document, Gateway, Node, Relay};
use crate::vault::{VaultError, VaultManager};

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

#[tauri::command]
pub fn delete_node(state: State<AppState>, id: String) -> Result<(), VaultError> {
    state.vault.delete(&id)
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
}
