use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use russh::client::{self, Config, Handler};
use russh::keys::agent::{client::AgentClient, AgentIdentity};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::ChannelMsg;

use crate::gateway::SessionSpec;

struct ClientHandler;

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _server_public_key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
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

/// Conecta via SSH (russh), autentica (cascata: chave → ssh-agent → password), abre um PTY/shell e relaya
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
    KeyLoadFailed(String, String), // (path, why) — surface the real reason, don't drop it
    Agent(usize),
    AgentUnavailable,
    Password,
}

/// How many ssh-agent identities to offer: leave one sshd attempt for each
/// following stage that will actually run, so a saved password stays reachable.
/// `key_offered` is whether a key was *sent to the server* (a load failure spends
/// no attempt). With MAX_AUTH_TRIES=6 this is at least 4; it only trims the agent's
/// share, never disabling it.
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
            Tried::KeyLoadFailed(kp, why) => {
                format!("key '{kp}' (couldn't load: {why}; use the private key file, not the .pub)")
            }
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

    authenticate(&mut handle, spec).await?;

    let channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;

    Ok((handle, channel, _jump_keepalive))
}

/// The server's preferred RSA signing hash. `best_supported_rsa_hash` first waits
/// for the server's EXT_INFO (a hard ~1s timeout), so it's computed lazily — only a
/// publickey stage needs it, and a password-only login to a server that never sends
/// EXT_INFO must not pay that wait. Memoized; a negotiation error is non-fatal (no
/// RSA hash hint, so auth just falls through to the next method).
async fn rsa_hash_for(
    handle: &client::Handle<ClientHandler>,
    memo: &mut Option<Option<HashAlg>>,
) -> Option<HashAlg> {
    if let Some(h) = memo {
        return *h;
    }
    let h = handle.best_supported_rsa_hash().await.ok().flatten().flatten();
    *memo = Some(h);
    h
}

/// Auth cascade: explicit key → ssh-agent (automatic) → saved password. Returns
/// `Ok` the moment a method succeeds; on exhaustion/disconnect returns a message
/// naming what was tried (which `run` delivers to the tab). Never `?`-propagates an
/// auth attempt: a transport drop ends the cascade (MaxAuthTries), a credential
/// rejection falls through to the next stage.
/// (russh maps a closed reply channel to `Ok(Failure)`, not `Err`, so a disconnect
/// can also look like a rejection; the next stage's send then fails fast as `Err`,
/// so at worst one no-op attempt is wasted — never a hang or a false success.)
async fn authenticate(
    handle: &mut client::Handle<ClientHandler>,
    spec: &SessionSpec,
) -> Result<(), SshErr> {
    let user = spec.username.clone().unwrap_or_default();
    let password = spec.password.as_deref().filter(|p| !p.is_empty()); // Some("") == absent
    let mut tried: Vec<Tried> = Vec::new();
    let mut rsa_hash: Option<Option<HashAlg>> = None; // lazy, memoized (see rsa_hash_for)

    // 1) Explicit key. A load failure (encrypted/.pub) spends no sshd attempt -> fall through.
    let mut key_offered = false;
    let mut offered_pub: Option<PublicKey> = None; // compared via .key_data() (comment differs)
    if let Some(kp) = &spec.key_path {
        match russh::keys::load_secret_key(kp, None) {
            Ok(key) => {
                offered_pub = Some(key.public_key().clone());
                key_offered = true;
                let ha = rsa_hash_for(handle, &mut rsa_hash).await;
                match handle
                    .authenticate_publickey(
                        user.clone(),
                        PrivateKeyWithHashAlg::new(Arc::new(key), ha),
                    )
                    .await
                {
                    Ok(r) if r.success() => return Ok(()),
                    Ok(_) => tried.push(Tried::Key(kp.clone())),
                    Err(_) => {
                        tried.push(Tried::Key(kp.clone()));
                        return Err(auth_failure_message(&user, &tried, true).into());
                    }
                }
            }
            Err(e) => tried.push(Tried::KeyLoadFailed(kp.clone(), e.to_string())),
        }
    }

    // 2) ssh-agent (automatic when $SSH_AUTH_SOCK is reachable; silent skip otherwise).
    match AgentClient::connect_env().await {
        Err(_) => tried.push(Tried::AgentUnavailable),
        Ok(mut agent) => {
            let cap = agent_cap(key_offered, password.is_some());
            let ids = agent.request_identities().await.unwrap_or_default();
            let mut offered = 0usize;
            let mut disconnected = false;
            for id in ids {
                if offered >= cap {
                    break;
                }
                let AgentIdentity::PublicKey { key, .. } = id else {
                    continue; // certs: no authenticate_*_with helper for a cert signer
                };
                if offered_pub.as_ref().map(|k| k.key_data()) == Some(key.key_data()) {
                    continue; // already tried as key_path
                }
                offered += 1;
                let ha = rsa_hash_for(handle, &mut rsa_hash).await;
                match handle
                    .authenticate_publickey_with(user.clone(), key, ha, &mut agent)
                    .await
                {
                    Ok(r) if r.success() => return Ok(()),
                    Ok(_) => {}
                    Err(russh::AgentAuthError::Send(_)) => {
                        disconnected = true; // the session channel is gone
                        break;
                    }
                    Err(_) => break, // agent died / ssh-add -c denied: stop offering, fall through
                }
            }
            tried.push(Tried::Agent(offered));
            if disconnected {
                return Err(auth_failure_message(&user, &tried, true).into());
            }
        }
    }

    // 3) Saved password (only when non-empty).
    if let Some(pw) = password {
        match handle.authenticate_password(user.clone(), pw.to_string()).await {
            Ok(r) if r.success() => return Ok(()),
            Ok(_) => tried.push(Tried::Password),
            Err(_) => {
                tried.push(Tried::Password);
                return Err(auth_failure_message(&user, &tried, true).into());
            }
        }
    }

    Err(auth_failure_message(&user, &tried, false).into())
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
                Tried::KeyLoadFailed("~/.ssh/id_ed25519.pub".into(), "unsupported key format".into()),
                Tried::AgentUnavailable,
            ],
            false,
        );
        assert!(m.contains("couldn't load"));
        assert!(m.contains("unsupported key format")); // real reason surfaced, not dropped
        assert!(m.contains(".pub")); // keep the "use the private key, not the .pub" steer
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
