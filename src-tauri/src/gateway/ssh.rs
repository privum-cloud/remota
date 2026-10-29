use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use russh::client::{self, Config, Handler};
use russh::keys::{PrivateKeyWithHashAlg, PublicKey};
use russh::ChannelMsg;

use crate::gateway::SessionSpec;

struct ClientHandler;

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        // Homelab/dev: aceita qualquer chave de host. (known_hosts = melhoria futura.)
        Ok(true)
    }
}

/// Mensagem de controlo do frontend (texto/JSON, distinta dos keystrokes binários).
#[derive(serde::Deserialize)]
struct Ctrl {
    #[serde(rename = "type")]
    t: String,
    cols: Option<u16>,
    rows: Option<u16>,
}

/// Conecta via SSH (russh), autentica por password, abre um PTY/shell e relaya
/// os bytes do canal <-> WebSocket (xterm.js no front). `cols`/`rows` = tamanho inicial do PTY.
pub async fn proxy_ssh(socket: WebSocket, spec: SessionSpec, cols: u16, rows: u16) {
    if let Err(e) = run(socket, spec, cols, rows).await {
        eprintln!("ssh proxy error: {e}");
    }
}

type SshErr = Box<dyn std::error::Error + Send + Sync>;

/// sshd's default MaxAuthTries. The cap below is best-effort: a server set lower
/// cuts the cascade short, which PR #3 now surfaces as a real error in the tab.
const MAX_AUTH_TRIES: usize = 6;

/// What the auth cascade attempted, kept so the final error can name each method.
enum Tried {
    Key(String),
    KeyLoadFailed(String),
    Agent(usize),
    AgentUnavailable,
    Password,
}

/// How many ssh-agent identities to offer: leave one sshd attempt for each
/// following stage that will actually run, so a saved password stays reachable.
/// `key_offered` is whether a key was *sent to the server* (a load failure spends
/// no attempt). Never returns 0 unless both other stages are present — and it
/// never disables the agent, only trims how many keys it offers.
fn agent_cap(key_offered: bool, password_present: bool) -> usize {
    MAX_AUTH_TRIES.saturating_sub(key_offered as usize + password_present as usize)
}

/// A human message naming every method the cascade tried, with the ssh-add hint
/// for passphrase-protected keys. `disconnected` switches the tail to name the
/// likely MaxAuthTries cause when the server dropped the transport mid-cascade.
fn auth_failure_message(user: &str, tried: &[Tried], disconnected: bool) -> String {
    let items: Vec<String> = tried
        .iter()
        .map(|t| match t {
            Tried::Key(kp) => format!("key '{kp}'"),
            Tried::KeyLoadFailed(kp) => format!("key '{kp}' (couldn't load)"),
            Tried::Agent(1) => "ssh-agent (1 key)".to_string(),
            Tried::Agent(n) => format!("ssh-agent ({n} keys)"),
            Tried::AgentUnavailable => "ssh-agent (unavailable)".to_string(),
            Tried::Password => "password".to_string(),
        })
        .collect();
    let tail = if disconnected {
        " — the server closed the connection (MaxAuthTries reached?)"
    } else {
        ""
    };
    format!(
        "SSH auth failed for '{user}' — tried {}{tail}. Check the credentials; \
         for a passphrase-protected key, load it into ssh-agent (ssh-add).",
        items.join(", ")
    )
}

async fn run(socket: WebSocket, spec: SessionSpec, cols: u16, rows: u16) -> Result<(), SshErr> {
    // Toda a fase pré-split (connect/auth/abrir PTY) pode falhar; se falhar, entregamos o
    // motivo real na aba ANTES de fechar — senão o front só mostra "[session ended]".
    let (_handle, mut channel, _keepalive) = match connect_and_open(&spec, cols, rows).await {
        Ok(v) => v,
        Err(e) => {
            // O xterm escreve frames de texto literalmente; a string é sempre a nossa
            // própria format!(...), logo UTF-8 válido (nunca bytes crus do servidor).
            let mut socket = socket;
            let _ = socket
                .send(Message::Text(format!("\r\n\x1b[31m{e}\x1b[0m\r\n").into()))
                .await;
            let _ = socket.close().await;
            return Err(e);
        }
    };

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Erro pós-split: além de avisar a aba, tem de voltar ao `proxy_ssh` p/ ir ao journal.
    let mut exit: Result<(), SshErr> = Ok(());

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
                    Some(Ok(Message::Binary(b))) => {
                        // Erro a meio da sessão (pós-split): avisa a aba e propaga ao journal.
                        if let Err(e) = channel.data(&b[..]).await {
                            let _ = ws_tx
                                .send(Message::Text(format!("\r\n\x1b[31mssh error: {e}\x1b[0m\r\n").into()))
                                .await;
                            exit = Err(e.into());
                            break;
                        }
                    }
                    Some(Ok(Message::Text(t))) => {
                        // Texto = controlo (JSON). resize → window_change (SIGWINCH no remoto → k9s/vim redesenham).
                        match serde_json::from_str::<Ctrl>(&t) {
                            Ok(c) if c.t == "resize" => {
                                let w = c.cols.unwrap_or(cols);
                                let h = c.rows.unwrap_or(rows);
                                let _ = channel.window_change(w as u32, h as u32, 0, 0).await;
                            }
                            _ => {
                                if let Err(e) = channel.data(t.as_bytes()).await {
                                    let _ = ws_tx
                                        .send(Message::Text(format!("\r\n\x1b[31mssh error: {e}\x1b[0m\r\n").into()))
                                        .await;
                                    exit = Err(e.into());
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => {}
                }
            }
        }
    }

    let _ = channel.eof().await;
    exit
}

/// Faz connect (direto/relay/jump), autentica e abre PTY+shell. Devolve o handle (tem de
/// ficar vivo durante a sessão), o canal, e o keepalive do jump. Qualquer erro aqui é
/// pré-split: o `run` entrega-o na aba.
async fn connect_and_open(
    spec: &SessionSpec,
    cols: u16,
    rows: u16,
) -> Result<
    (
        client::Handle<ClientHandler>,
        russh::Channel<client::Msg>,
        Option<client::Handle<crate::gateway::tunnel::AcceptAllHandler>>,
    ),
    SshErr,
> {
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
        // RSA keys must say which SHA to sign with; ask the server what it accepts.
        let hash_alg = handle.best_supported_rsa_hash().await?.flatten();
        handle
            .authenticate_publickey(
                username.clone(),
                PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg),
            )
            .await?
    } else {
        handle.authenticate_password(username.clone(), password).await?
    };
    if !authed.success() {
        let how = if spec.key_path.is_some() { "SSH key" } else { "password" };
        return Err(
            format!("SSH authentication failed for user '{username}' using {how} — check the username and {how}.").into(),
        );
    }

    let channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;

    Ok((handle, channel, _jump_keepalive))
}

/// Separa "host:porta" em (host, porta). Default 22 se faltar/inválida.
fn split_host_port(target: &str) -> (String, u16) {
    match target.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(22)),
        None => (target.to_string(), 22),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // sshd MaxAuthTries defaults to 6; each method offered costs one attempt.
    #[test]
    fn cap_leaves_one_attempt_for_each_following_stage() {
        assert_eq!(agent_cap(true, true), 4); // key tried + password to follow
        assert_eq!(agent_cap(false, false), 6); // agent is the only stage
        assert_eq!(agent_cap(false, true), 5); // password to follow
        assert_eq!(agent_cap(true, false), 5); // key already cost one
    }

    #[test]
    fn message_lists_everything_tried_with_ssh_add_hint() {
        let tried = [
            Tried::Key("~/.ssh/id_rsa".into()),
            Tried::Agent(3),
            Tried::Password,
        ];
        let m = auth_failure_message("alice", &tried, false);
        assert!(m.contains("alice"));
        assert!(m.contains("key '~/.ssh/id_rsa'"));
        assert!(m.contains("ssh-agent (3 keys)"));
        assert!(m.contains("password"));
        assert!(m.contains("ssh-add")); // passphrase hint always present
        assert!(!m.contains("closed the connection")); // not a disconnect
    }

    #[test]
    fn message_omits_password_when_none_was_tried() {
        let m = auth_failure_message("bob", &[Tried::Agent(1)], false);
        assert!(m.contains("ssh-agent (1 key)")); // singular
        assert!(!m.contains(", password"));
    }

    #[test]
    fn message_flags_a_key_that_could_not_load() {
        let m = auth_failure_message(
            "carol",
            &[
                Tried::KeyLoadFailed("~/.ssh/id_ed25519".into()),
                Tried::AgentUnavailable,
            ],
            false,
        );
        assert!(m.contains("couldn't load"));
        assert!(m.contains("ssh-agent (unavailable)"));
        assert!(m.contains("ssh-add"));
    }

    #[test]
    fn message_notes_server_disconnect() {
        // Server hung up mid-cascade (MaxAuthTries): report the disconnect, not a flat rejection.
        let m = auth_failure_message("dave", &[Tried::Key("~/.ssh/id_rsa".into())], true);
        assert!(m.contains("closed the connection"));
        assert!(m.contains("MaxAuthTries"));
    }
}
