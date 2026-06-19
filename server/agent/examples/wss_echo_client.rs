//! Diagnostic: prove the relay's DATA plane end-to-end over public wss.
//!
//! Connects as the *client* leg of an already-brokered session and round-trips a payload.
//! Pair it with a `remota-agent` pointed at the same relay whose target is a local echo
//! service, after `POST /session` returns {session_id, token}.
//!
//! Usage:
//!   wss_echo_client <relay_base> <session_id> <token>
//!   wss_echo_client wss://relay.privum.cloud <SID> <TOK>

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Same rustls provider install the agent needs for wss://.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut args = std::env::args().skip(1);
    let base = args.next().expect("arg1 relay_base (e.g. wss://relay.privum.cloud)");
    let session = args.next().expect("arg2 session_id");
    let token = args.next().expect("arg3 token");
    let url = format!(
        "{}/data/{}?token={}&role=client",
        base.trim_end_matches('/'),
        session,
        token
    );

    let (ws, _) = connect_async(&url).await?;
    let (mut tx, mut rx) = ws.split();

    let payload = b"hello-remota-public".to_vec();
    tx.send(Message::Binary(payload.clone())).await?;

    let got = tokio::time::timeout(std::time::Duration::from_secs(8), async {
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Binary(d) => return Some(d),
                Message::Text(t) => return Some(t.into_bytes()),
                _ => {}
            }
        }
        None
    })
    .await?;

    match got {
        Some(d) if d == payload => {
            println!("ECHO OK: {} bytes round-tripped through the relay data plane", d.len());
            Ok(())
        }
        Some(d) => {
            eprintln!("MISMATCH: got {:?}", String::from_utf8_lossy(&d));
            std::process::exit(2)
        }
        None => {
            eprintln!("NO ECHO (timeout) — data leg did not bridge");
            std::process::exit(3)
        }
    }
}
