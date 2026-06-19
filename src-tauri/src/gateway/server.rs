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

use crate::gateway::{SessionKind, SessionRegistry, SessionSpec};

#[derive(Deserialize)]
struct TokenQuery {
    token: String,
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
        Some(spec) => ws.on_upgrade(move |socket| crate::gateway::ssh::proxy_ssh(socket, spec)),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn bridge_raw_tcp(socket: WebSocket, spec: SessionSpec) {
    debug_assert_eq!(spec.kind, SessionKind::RawTcp);
    let tcp = match tokio::net::TcpStream::connect(&spec.target).await {
        Ok(s) => s,
        Err(_) => return,
    };
    let (mut tcp_r, mut tcp_w) = tcp.into_split();
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
}
