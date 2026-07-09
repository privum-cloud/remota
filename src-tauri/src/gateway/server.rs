use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::gateway::{SessionRegistry, SessionSpec};

#[derive(Deserialize)]
struct TokenQuery {
    token: String,
    /// Dimensões iniciais do terminal (SSH). Ignoradas pelas outras rotas.
    #[serde(default)]
    cols: Option<u16>,
    #[serde(default)]
    rows: Option<u16>,
}

pub fn router(registry: Arc<SessionRegistry>) -> Router {
    // axum 0.8: parâmetro de rota usa `{id}` (sintaxe `:id` faz panic no build).
    Router::new()
        .route("/session/{id}", get(session_handler))
        .route("/ssh/{id}", get(ssh_handler))
        .with_state(registry)
}

/// Bind em 127.0.0.1:0, spawna o servidor, retorna a porta efetiva.
pub async fn start(registry: Arc<SessionRegistry>) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = router(registry);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

async fn session_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    Query(q): Query<TokenQuery>,
    State(registry): State<Arc<SessionRegistry>>,
) -> Response {
    match registry.consume(&id, &q.token) {
        Some(spec) => ws.on_upgrade(move |socket| bridge_raw_tcp(socket, spec)),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn ssh_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    Query(q): Query<TokenQuery>,
    State(registry): State<Arc<SessionRegistry>>,
) -> Response {
    match registry.consume(&id, &q.token) {
        Some(spec) => {
            let cols = q.cols.filter(|c| *c > 0).unwrap_or(80);
            let rows = q.rows.filter(|r| *r > 0).unwrap_or(24);
            ws.on_upgrade(move |socket| crate::gateway::ssh::proxy_ssh(socket, spec, cols, rows))
        }
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn bridge_raw_tcp(socket: WebSocket, spec: SessionSpec) {
    // Direto, ou tunelado por um jump host (SSH direct-tcpip) se a sessão tiver gateway.
    // `_jump` mantém o túnel vivo durante a sessão.
    let (stream, _jump) = match crate::gateway::tunnel::connect_target(&spec.target, &spec.gateway).await {
        Ok(x) => x,
        Err(_) => return,
    };
    let (mut tcp_r, mut tcp_w) = tokio::io::split(stream);
    let (mut ws_tx, mut ws_rx) = socket.split();

    let ws_to_tcp = async {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(b) => {
                    if tcp_w.write_all(&b).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    let tcp_to_ws = async {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match tcp_r.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws_tx
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    };

    tokio::select! {
        _ = ws_to_tcp => {},
        _ = tcp_to_ws => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::{SessionKind, SessionRegistry};
    use futures_util::{SinkExt, StreamExt};
    use std::sync::Arc;
    use tokio_tungstenite::tungstenite::Message as TMessage;

    #[tokio::test]
    async fn raw_tcp_bridge_echoes_bytes() {
        // Echo TCP server real
        let echo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let echo_addr = echo.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = echo.accept().await.unwrap();
            let (mut r, mut w) = sock.split();
            tokio::io::copy(&mut r, &mut w).await.ok();
        });

        // Gateway
        let reg = Arc::new(SessionRegistry::new());
        let spec = reg.create(echo_addr.to_string(), SessionKind::RawTcp);
        let port = start(reg.clone()).await;

        // Cliente WS
        let url = format!("ws://127.0.0.1:{port}/session/{}?token={}", spec.id, spec.token);
        let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        ws.send(TMessage::Binary(b"ola-mundo".to_vec())).await.unwrap();
        let got = ws.next().await.unwrap().unwrap();
        assert_eq!(got.into_data(), b"ola-mundo");
    }

    #[tokio::test]
    async fn rejects_invalid_token() {
        let reg = Arc::new(SessionRegistry::new());
        let spec = reg.create("127.0.0.1:1".into(), SessionKind::RawTcp);
        let port = start(reg.clone()).await;
        let url = format!("ws://127.0.0.1:{port}/session/{}?token=errado", spec.id);
        assert!(tokio_tungstenite::connect_async(&url).await.is_err());
    }

    /// Prova (vivo) o fix do tamanho do PTY: o remoto vê as dimensões pedidas no `?cols=&rows=`
    /// e o resize (window_change) propaga. Precisa de sshd local + chave autorizada. Correr:
    ///   SSH_USER=sysadmin SSH_KEY=$HOME/.ssh/id_rsa \
    ///     cargo test -p remota gateway::server::tests::ssh_pty_size -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn ssh_pty_size_and_resize() {
        use crate::gateway::SessionKind;
        use std::time::Duration;

        let user = std::env::var("SSH_USER").unwrap_or_else(|_| "sysadmin".into());
        let key = std::env::var("SSH_KEY")
            .unwrap_or_else(|_| format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap()));

        let reg = Arc::new(SessionRegistry::new());
        let spec = reg.create_with_creds(
            "127.0.0.1:22".into(),
            SessionKind::Ssh,
            Some(user),
            None,
            Some(key),
            None,
            None,
        );
        let port = start(reg.clone()).await;

        let url = format!(
            "ws://127.0.0.1:{port}/ssh/{}?token={}&cols=203&rows=51",
            spec.id, spec.token
        );
        let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

        async fn wait_for(
            ws: &mut (impl StreamExt<Item = Result<TMessage, tokio_tungstenite::tungstenite::Error>> + Unpin),
            needle: &str,
        ) -> bool {
            let mut acc = String::new();
            tokio::time::timeout(Duration::from_secs(8), async {
                while let Some(Ok(m)) = ws.next().await {
                    match m {
                        TMessage::Binary(b) => acc.push_str(&String::from_utf8_lossy(&b)),
                        TMessage::Text(t) => acc.push_str(&t),
                        _ => {}
                    }
                    if acc.contains(needle) {
                        return true;
                    }
                }
                false
            })
            .await
            .unwrap_or(false)
        }

        // `stty size` imprime "rows cols".
        ws.send(TMessage::Binary(b"stty size\n".to_vec())).await.unwrap();
        assert!(wait_for(&mut ws, "51 203").await, "PTY inicial devia ser 51x203");

        // Resize via mensagem de controlo (texto/JSON) → window_change.
        ws.send(TMessage::Text("{\"type\":\"resize\",\"cols\":120,\"rows\":40}".into())).await.unwrap();
        ws.send(TMessage::Binary(b"stty size\n".to_vec())).await.unwrap();
        assert!(wait_for(&mut ws, "40 120").await, "após resize devia ser 40x120");
    }
}
