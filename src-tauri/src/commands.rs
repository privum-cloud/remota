use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::gateway::{SessionKind, SessionRegistry, SessionSpec};

pub struct AppState {
    pub registry: Arc<SessionRegistry>,
    pub gateway_port: u16,
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
    };
    format!("ws://127.0.0.1:{port}/{route}/{}?token={}", spec.id, spec.token)
}

#[tauri::command]
pub fn open_session(state: State<AppState>, target: String, kind: String) -> SessionInfo {
    let session_kind = match kind.as_str() {
        "rdp" => SessionKind::RdpRdcleanpath,
        _ => SessionKind::RawTcp,
    };
    let spec = state.registry.create(target, session_kind);
    SessionInfo {
        ws_url: build_ws_url(state.gateway_port, &spec),
        kind,
    }
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
        };
        assert_eq!(
            build_ws_url(7000, &spec),
            "ws://127.0.0.1:7000/session/abc?token=tok"
        );
    }

    #[test]
    fn ws_url_rdp_usa_rota_rdp() {
        let spec = SessionSpec {
            id: "xyz".into(),
            token: "t2".into(),
            target: "win:3389".into(),
            kind: SessionKind::RdpRdcleanpath,
        };
        assert_eq!(
            build_ws_url(7000, &spec),
            "ws://127.0.0.1:7000/rdp/xyz?token=t2"
        );
    }
}
