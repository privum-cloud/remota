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

    // O jump (se houver) tem de ficar vivo durante toda a sessão, senão o túnel fecha.
    let mut _jump_keepalive: Option<client::Handle<crate::gateway::tunnel::AcceptAllHandler>> = None;

    let mut handle = if let Some(relay) = &spec.relay {
        // Relay self-hosted (NAT traversal): SSH sobre o túnel wss até ao agente, que liga ao sshd do alvo.
        let (thost, tport) = split_host_port(&spec.target);
        let stream = crate::gateway::relay::connect_relay(relay, &thost, tport).await?;
        client::connect_stream(config.clone(), stream, ClientHandler).await?
    } else if let Some(gw) = &spec.gateway {
        // SSH ao jump host (password OU chave) → túnel direct-tcpip → SSH sobre esse stream.
        let jump = crate::gateway::tunnel::connect_jump(gw).await?;
        let (thost, tport) = split_host_port(&spec.target);
        let channel = jump
            .channel_open_direct_tcpip(thost, tport as u32, "127.0.0.1", 0)
            .await?;
        let target = client::connect_stream(config.clone(), channel.into_stream(), ClientHandler).await?;
        _jump_keepalive = Some(jump);
        target
    } else {
        client::connect(config.clone(), spec.target.as_str(), ClientHandler).await?
    };

    // Auth: chave SSH se houver key_path, senão password.
    let authed = if let Some(kp) = &spec.key_path {
        let key = russh::keys::load_secret_key(kp, None).map_err(|e| {
            format!(
                "could not load SSH private key '{kp}': {e}. Use the PRIVATE key (not the .pub file); \
                 passphrase-protected keys aren't supported yet."
            )
        })?;
        handle.authenticate_publickey(username.clone(), Arc::new(key)).await?
    } else {
        handle.authenticate_password(username.clone(), password).await?
    };
    if !authed {
        let how = if spec.key_path.is_some() { "SSH key" } else { "password" };
        return Err(
            format!("SSH authentication failed for user '{username}' using {how} — check the username and {how}.").into(),
        );
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

/// Separa "host:porta" em (host, porta). Default 22 se faltar/inválida.
fn split_host_port(target: &str) -> (String, u16) {
    match target.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(22)),
        None => (target.to_string(), 22),
    }
}
