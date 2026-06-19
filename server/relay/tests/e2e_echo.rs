//! End-to-end proof for M-agent-0 T4: a client sends bytes through the relay, the relay
//! brokers a session, a real (in-process) agent bridges to a local echo TCP service, and the
//! bytes come back. No public IP, no open port on the "remote" side — only outbound WS.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use remota_agent::{run_agent, AgentConfig};
use remota_relay::build_app;

const TOKEN: &str = "test-enroll";

/// Start the relay on an ephemeral port; return its address.
async fn spawn_relay() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = build_app(TOKEN.to_string());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Start a TCP echo server on an ephemeral port; return its port.
async fn spawn_echo() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    match sock.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if sock.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    });
    port
}

/// Minimal HTTP POST /session (hand-rolled to avoid an HTTP client dependency).
/// Returns Some((session_id, token)) on 200, None otherwise (e.g. agent not yet registered).
async fn post_session(addr: SocketAddr, agent_id: &str, target_port: u16) -> Option<(String, String)> {
    let body = format!("{{\"agent_id\":\"{agent_id}\",\"target_port\":{target_port}}}");
    let req = format!(
        "POST /session HTTP/1.1\r\nHost: relay\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let mut sock = TcpStream::connect(addr).await.ok()?;
    sock.write_all(req.as_bytes()).await.ok()?;
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).await.ok()?;
    let text = String::from_utf8_lossy(&raw);

    let status_ok = text.lines().next().map(|l| l.contains(" 200 ")).unwrap_or(false);
    if !status_ok {
        return None;
    }
    let json = text.split("\r\n\r\n").nth(1)?;
    let v: serde_json::Value = serde_json::from_str(json.trim()).ok()?;
    Some((
        v["session_id"].as_str()?.to_string(),
        v["token"].as_str()?.to_string(),
    ))
}

#[tokio::test]
async fn echo_round_trips_through_relay_and_agent() {
    let relay = spawn_relay().await;
    let echo_port = spawn_echo().await;

    // The "remote" machine runs the agent — outbound WS only.
    let cfg = AgentConfig {
        relay_base: format!("ws://{relay}"),
        enroll_token: TOKEN.into(),
        agent_id: "test-agent".into(),
        name: "tester".into(),
        os: "linux".into(),
        capabilities: vec!["cli".into()],
    };
    tokio::spawn(async move {
        let _ = run_agent(cfg).await;
    });

    // Broker a session — retry until the agent has registered.
    let mut session = None;
    for _ in 0..100 {
        if let Some(s) = post_session(relay, "test-agent", echo_port).await {
            session = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let (session_id, token) = session.expect("session brokered (agent registered)");

    // The app connects to the data channel as the client leg.
    let url = format!("ws://{relay}/data/{session_id}?token={token}&role=client");
    let (ws, _) = connect_async(&url).await.expect("client data channel");
    let (mut tx, mut rx) = ws.split();

    tx.send(Message::Binary(b"hello-remota".to_vec()))
        .await
        .expect("send payload");

    let echoed = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(msg)) = rx.next().await {
            match msg {
                Message::Binary(d) => return Some(d),
                Message::Text(t) => return Some(t.into_bytes()),
                _ => {}
            }
        }
        None
    })
    .await
    .expect("did not time out")
    .expect("got an echo frame");

    assert_eq!(echoed, b"hello-remota");
}
