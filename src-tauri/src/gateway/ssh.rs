use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use russh::client::{self, Config, Handler};
use russh::keys::key::PublicKey;
use russh::ChannelMsg;

use crate::gateway::SessionSpec;

struct ClientHandler;

#[async_trait]
impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        // Homelab/dev: aceita qualquer chave de host. (known_hosts = melhoria futura.)
        Ok(true)
    }
}

/// Conecta via SSH (russh), autentica por password, abre um PTY/shell e relaya
/// os bytes do canal <-> WebSocket (xterm.js no front).
pub async fn proxy_ssh(socket: WebSocket, spec: SessionSpec) {
    if let Err(e) = run(socket, spec).await {
        eprintln!("ssh proxy error: {e}");
    }
}

async fn run(
    socket: WebSocket,
    spec: SessionSpec,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let username = spec.username.clone().unwrap_or_default();
    let password = spec.password.clone().unwrap_or_default();

    let config = Arc::new(Config::default());
    let mut handle = client::connect(config, spec.target.as_str(), ClientHandler).await?;

    let authed = handle.authenticate_password(username, password).await?;
    if !authed {
        return Err("autenticação SSH falhou (utilizador/senha)".into());
    }

    let mut channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;

    let (mut ws_tx, mut ws_rx) = socket.split();

    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if ws_tx.send(Message::Binary(data.to_vec().into())).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
            ws_msg = ws_rx.next() => {
                match ws_msg {
                    Some(Ok(Message::Binary(b))) => { channel.data(&b[..]).await?; }
                    Some(Ok(Message::Text(t))) => { channel.data(t.as_bytes()).await?; }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => {}
                }
            }
        }
    }

    let _ = channel.eof().await;
    Ok(())
}
