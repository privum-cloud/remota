//! Conexão "relayed": abre um stream ao destino ATRAVÉS de um `remota-relay` self-hosted.
//!
//! Fluxo: `POST /session` (corretagem, token single-use) → liga a perna `client` do
//! canal de dados (`/data/{id}` por wss) → faz a ponte dos bytes para um `tokio::io::duplex`.
//! A outra metade do duplex é devolvida como um stream tokio que o russh (ou o bridge raw)
//! conduz como se fosse um TCP direto. O agente do outro lado liga ao serviço local do alvo.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message as TMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::gateway::tunnel::AsyncStream;
use crate::model::Relay;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

/// Garante um CryptoProvider default (ring) para o rustls 0.23. Idempotente.
pub fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Liga a `target_host:target_port` ATRAVÉS do relay+agente; devolve um stream de bytes.
pub async fn connect_relay(
    relay: &Relay,
    target_host: &str,
    target_port: u16,
) -> Result<Box<dyn AsyncStream>, BoxErr> {
    ensure_crypto_provider();

    // 1) Corretar a sessão.
    let (session_id, token) = broker_session(relay, target_host, target_port).await?;

    // 2) Ligar a perna `client` do canal de dados (wss).
    let data_url = format!(
        "{}/data/{}?token={}&role=client",
        relay.url.trim_end_matches('/'),
        session_id,
        token
    );
    let (ws, _) = connect_async(&data_url)
        .await
        .map_err(|e| format!("relay data channel for session {session_id}: {e}"))?;

    // 3) Ponte wss <-> duplex; devolve a metade `near` como stream.
    let (near, far) = tokio::io::duplex(64 * 1024);
    tokio::spawn(pump(ws, far));
    Ok(Box::new(near))
}

/// `POST /session` ao relay → (session_id, token). HTTP escrito à mão (sem cliente HTTP).
async fn broker_session(
    relay: &Relay,
    target_host: &str,
    target_port: u16,
) -> Result<(String, String), BoxErr> {
    let (tls, host, port) = parse_relay_base(&relay.url)?;

    let body = format!(
        "{{\"agent_id\":\"{}\",\"target_host\":\"{}\",\"target_port\":{}}}",
        json_escape(&relay.agent_id),
        json_escape(target_host),
        target_port
    );
    let req = format!(
        "POST /session HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    let raw = if tls {
        let mut stream = tls_connect(&host, port).await?;
        stream.write_all(req.as_bytes()).await?;
        stream.flush().await?;
        read_http_response(&mut stream).await?
    } else {
        let mut stream = TcpStream::connect((host.as_str(), port)).await?;
        stream.write_all(req.as_bytes()).await?;
        stream.flush().await?;
        read_http_response(&mut stream).await?
    };

    parse_session_response(&raw)
        .map_err(|e| -> BoxErr { format!("could not reach agent '{}' via relay: {e}", relay.agent_id).into() })
}

/// Abre TLS (rustls/ring + raízes webpki) a `host:port`.
async fn tls_connect(
    host: &str,
    port: u16,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, BoxErr> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| format!("invalid relay host: {host}"))?;
    let tcp = TcpStream::connect((host, port)).await?;
    Ok(connector.connect(server_name, tcp).await?)
}

/// Lê EXATAMENTE cabeçalhos + Content-Length bytes (não `read_to_end` sobre TLS, que pode
/// dar UnexpectedEof se o servidor fechar sem close_notify).
async fn read_http_response<S>(stream: &mut S) -> Result<Vec<u8>, BoxErr>
where
    S: AsyncReadExt + Unpin,
{
    let mut buf = Vec::with_capacity(2048);
    let mut tmp = [0u8; 2048];

    // Cabeçalhos, até "\r\n\r\n".
    let header_end = loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err("relay closed before sending response headers".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 64 * 1024 {
            return Err("relay response headers too large".into());
        }
    };

    let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let want = header_end + content_length(&headers);
    while buf.len() < want {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            break; // servidor fechou; usa o que temos
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(buf)
}

fn parse_session_response(raw: &[u8]) -> Result<(String, String), BoxErr> {
    let text = String::from_utf8_lossy(raw);
    let status = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("");
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    if status != "200" {
        let reason = match status {
            "404" => "agent offline or not registered on the relay".to_string(),
            "403" => "forbidden — this machine's IP is not allowed to broker sessions (relay gate)".to_string(),
            _ => format!("relay returned HTTP {status}: {body}"),
        };
        return Err(reason.into());
    }
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("bad /session JSON: {e} ({body})"))?;
    let session_id = v["session_id"]
        .as_str()
        .ok_or("missing session_id")?
        .to_string();
    let token = v["token"].as_str().ok_or("missing token")?.to_string();
    Ok((session_id, token))
}

/// Ponte de bytes wss <-> duplex (mesma lógica do bridge do agente).
async fn pump(ws: WebSocketStream<MaybeTlsStream<TcpStream>>, duplex: tokio::io::DuplexStream) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (mut d_rd, mut d_wr) = tokio::io::split(duplex);

    let ws_to_d = async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                TMessage::Binary(b) => {
                    if d_wr.write_all(&b).await.is_err() {
                        break;
                    }
                }
                TMessage::Text(t) => {
                    if d_wr.write_all(t.as_bytes()).await.is_err() {
                        break;
                    }
                }
                TMessage::Close(_) => break,
                _ => {}
            }
        }
        let _ = d_wr.shutdown().await;
    };

    let d_to_ws = async move {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match d_rd.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws_tx.send(TMessage::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = ws_tx.send(TMessage::Close(None)).await;
    };

    tokio::select! {
        _ = ws_to_d => {}
        _ = d_to_ws => {}
    }
}

/// Deriva (tls, host, port) da URL base do relay (`wss://`, `ws://`, `https://`, `http://`).
fn parse_relay_base(url: &str) -> Result<(bool, String, u16), BoxErr> {
    let url = url.trim_end_matches('/');
    let (tls, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("relay url must start with wss:// or ws:// (got {url})").into());
    };
    let host_port = rest.split('/').next().unwrap_or(rest);
    let default_port = if tls { 443 } else { 80 };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(default_port)),
        None => (host_port.to_string(), default_port),
    };
    if host.is_empty() {
        return Err("relay url has empty host".into());
    }
    Ok((tls, host, port))
}

fn content_length(headers: &str) -> usize {
    for line in headers.lines() {
        if let Some(v) = line.strip_prefix("Content-Length:").or_else(|| line.strip_prefix("content-length:")) {
            return v.trim().parse().unwrap_or(0);
        }
    }
    0
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_relay_base_variants() {
        assert_eq!(parse_relay_base("wss://relay.privum.cloud").unwrap(), (true, "relay.privum.cloud".into(), 443));
        assert_eq!(parse_relay_base("ws://127.0.0.1:8787").unwrap(), (false, "127.0.0.1".into(), 8787));
        assert_eq!(parse_relay_base("https://r.example/").unwrap(), (true, "r.example".into(), 443));
        assert!(parse_relay_base("relay.example").is_err());
    }

    #[test]
    fn content_length_parses() {
        assert_eq!(content_length("HTTP/1.1 200 OK\r\nContent-Length: 42\r\n"), 42);
        assert_eq!(content_length("HTTP/1.1 200 OK\r\n"), 0);
    }

    #[test]
    fn parse_session_ok_and_error() {
        let ok = b"HTTP/1.1 200 OK\r\nContent-Length: 41\r\n\r\n{\"session_id\":\"s1\",\"token\":\"t1\"}";
        assert_eq!(parse_session_response(ok).unwrap(), ("s1".into(), "t1".into()));
        let bad = b"HTTP/1.1 404 Not Found\r\nContent-Length: 13\r\n\r\nagent offline";
        assert!(parse_session_response(bad).is_err());
    }

    /// Prova o caminho de DADOS vivo: liga ao sshd do alvo ATRAVÉS do relay e confirma o
    /// banner `SSH-2.0-`. Correr manualmente com um relay+agente reais:
    ///   RELAY_URL=wss://relay.privum.cloud AGENT_ID=<id> TARGET_HOST=127.0.0.1 TARGET_PORT=22 \
    ///     cargo test -p remota relay::tests::ssh_banner_over_relay -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn ssh_banner_over_relay() {
        let url = std::env::var("RELAY_URL").expect("RELAY_URL");
        let agent_id = std::env::var("AGENT_ID").expect("AGENT_ID");
        let host = std::env::var("TARGET_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("TARGET_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);

        let relay = Relay { url, agent_id };
        let stream = connect_relay(&relay, &host, port).await.expect("connect_relay");
        let (mut rd, _wr) = tokio::io::split(stream);
        let mut buf = [0u8; 64];
        let n = rd.read(&mut buf).await.expect("read banner");
        let banner = String::from_utf8_lossy(&buf[..n]);
        println!("relay banner: {banner:?}");
        assert!(banner.starts_with("SSH-2.0-"), "expected SSH banner, got {banner:?}");
    }

    /// Prova o caminho BIDIRECIONAL + auth + canal: SSH completo via russh sobre o
    /// `connect_relay` (KEX precisa de escrita, que o teste do banner não exercita). Correr:
    ///   RELAY_URL=wss://relay.privum.cloud AGENT_ID=vmlinuxdev SSH_USER=sysadmin \
    ///   SSH_KEY=/home/sysadmin/.ssh/id_rsa \
    ///     cargo test -p remota relay::tests::ssh_exec_over_relay -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn ssh_exec_over_relay() {
        use russh::client::{self, Config};
        use russh::ChannelMsg;

        let url = std::env::var("RELAY_URL").expect("RELAY_URL");
        let agent_id = std::env::var("AGENT_ID").expect("AGENT_ID");
        let host = std::env::var("TARGET_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("TARGET_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
        let user = std::env::var("SSH_USER").unwrap_or_else(|_| "sysadmin".into());
        let key_path = std::env::var("SSH_KEY")
            .unwrap_or_else(|_| format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap()));

        let relay = Relay { url, agent_id };
        let stream = connect_relay(&relay, &host, port).await.expect("connect_relay");

        let config = Arc::new(Config::default());
        let mut handle =
            client::connect_stream(config, stream, crate::gateway::tunnel::AcceptAllHandler)
                .await
                .expect("ssh connect_stream over relay");
        let key = russh::keys::load_secret_key(&key_path, None).expect("load ssh key");
        let authed = handle
            .authenticate_publickey(user, Arc::new(key))
            .await
            .expect("auth call");
        assert!(authed, "publickey auth failed (is the key in authorized_keys?)");

        let mut channel = handle.channel_open_session().await.expect("open session");
        channel.exec(true, "echo remota-ok").await.expect("exec");

        let mut out = Vec::new();
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => out.extend_from_slice(&data),
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
        let text = String::from_utf8_lossy(&out);
        println!("exec output: {text:?}");
        assert!(text.contains("remota-ok"), "expected 'remota-ok', got {text:?}");
    }
}
