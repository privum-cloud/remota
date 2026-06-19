//! remota-agent core. Connects out (WS) to a relay, registers, and on each `OpenChannel`
//! opens a data WS back to the relay and bridges it to a local TCP service.
//!
//! Exposed as a library so the relay's E2E test can drive a real agent in-process.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message as TMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use remota_proto::{AgentMsg, RelayMsg};

type DataWs = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Clone, Debug)]
pub struct AgentConfig {
    /// Relay base WS URL, e.g. `ws://127.0.0.1:8787` (no trailing path).
    pub relay_base: String,
    /// Enrollment secret presented on Register.
    pub enroll_token: String,
    /// Stable agent id (shown/targeted by the app).
    pub agent_id: String,
    pub name: String,
    pub os: String,
    pub capabilities: Vec<String>,
}

/// Connect, register, then service `OpenChannel` requests until the control socket closes.
pub async fn run_agent(cfg: AgentConfig) -> Result<()> {
    // rustls 0.23 requires a process-default CryptoProvider before any TLS. Install ring
    // explicitly so wss:// works deterministically regardless of feature unification.
    // Idempotent: ignore the error if another caller already installed one.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let base = cfg.relay_base.trim_end_matches('/').to_string();
    let control_url = format!("{base}/agent/control");

    let (ws, _) = connect_async(&control_url)
        .await
        .with_context(|| format!("connect control {control_url}"))?;
    let (mut tx, mut rx) = ws.split();

    // Register.
    let reg = AgentMsg::Register {
        agent_id: cfg.agent_id.clone(),
        name: cfg.name.clone(),
        os: cfg.os.clone(),
        capabilities: cfg.capabilities.clone(),
        token: cfg.enroll_token.clone(),
    };
    tx.send(TMessage::Text(serde_json::to_string(&reg)?)).await?;

    // Expect a Registered ack.
    match rx.next().await {
        Some(Ok(TMessage::Text(t))) => match serde_json::from_str::<RelayMsg>(t.as_str())? {
            RelayMsg::Registered { ok: true } => {}
            RelayMsg::Registered { ok: false } => bail!("relay rejected registration"),
            RelayMsg::Error { msg } => bail!("relay error: {msg}"),
            other => bail!("unexpected first message: {other:?}"),
        },
        other => bail!("no Registered ack: {other:?}"),
    }

    // Heartbeat keeper owns the write half.
    let heartbeat = tokio::spawn(async move {
        let mut iv = tokio::time::interval(Duration::from_secs(20));
        loop {
            iv.tick().await;
            let hb = serde_json::to_string(&AgentMsg::Heartbeat).unwrap();
            if tx.send(TMessage::Text(hb)).await.is_err() {
                break;
            }
        }
    });

    // Control loop: spawn a data bridge per OpenChannel.
    while let Some(msg) = rx.next().await {
        let msg = msg?;
        match msg {
            TMessage::Text(t) => match serde_json::from_str::<RelayMsg>(t.as_str()) {
                Ok(RelayMsg::OpenChannel { session_id, token, target_host, target_port }) => {
                    let base = base.clone();
                    tokio::spawn(async move {
                        if let Err(e) =
                            open_channel(base, session_id, token, target_host, target_port).await
                        {
                            eprintln!("remota-agent: data channel error: {e:#}");
                        }
                    });
                }
                Ok(RelayMsg::Error { msg }) => eprintln!("remota-agent: relay error: {msg}"),
                _ => {}
            },
            TMessage::Close(_) => break,
            _ => {}
        }
    }

    heartbeat.abort();
    Ok(())
}

/// Open the data WS back to the relay and bridge it to the local target.
async fn open_channel(
    relay_base: String,
    session_id: String,
    token: String,
    target_host: String,
    target_port: u16,
) -> Result<()> {
    let data_url = format!("{relay_base}/data/{session_id}?token={token}&role=agent");
    let (ws, _) = connect_async(&data_url)
        .await
        .with_context(|| format!("connect data {data_url}"))?;
    let tcp = TcpStream::connect((target_host.as_str(), target_port))
        .await
        .with_context(|| format!("connect target {target_host}:{target_port}"))?;
    bridge_ws_tcp(ws, tcp).await;
    Ok(())
}

/// Bidirectional bridge: WS Binary/Text → TCP, and TCP bytes → WS Binary. Ends on either close.
async fn bridge_ws_tcp(ws: DataWs, tcp: TcpStream) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (mut tcp_rd, mut tcp_wr) = tcp.into_split();

    let ws_to_tcp = async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                TMessage::Binary(d) => {
                    if tcp_wr.write_all(&d).await.is_err() {
                        break;
                    }
                }
                TMessage::Text(t) => {
                    if tcp_wr.write_all(t.as_bytes()).await.is_err() {
                        break;
                    }
                }
                TMessage::Close(_) => break,
                _ => {}
            }
        }
        let _ = tcp_wr.shutdown().await;
    };

    let tcp_to_ws = async move {
        let mut buf = vec![0u8; 16384];
        loop {
            match tcp_rd.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ws_tx.send(TMessage::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = ws_tx.send(TMessage::Close(None)).await;
    };

    tokio::select! {
        _ = ws_to_tcp => {}
        _ = tcp_to_ws => {}
    }
}
