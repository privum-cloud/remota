//! Session brokering: single-use tokens + the `/data/{id}` rendezvous that pairs the
//! app (role=client) and the agent (role=agent) and bridges raw bytes between them.

use std::collections::HashMap;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{oneshot, Mutex};

/// A brokered session awaiting (or running) its data channel.
#[derive(Clone)]
pub struct SessionInfo {
    /// Single-use token shared by both legs (app + agent). Consumed when the pair is established.
    pub token: String,
    pub agent_id: String,
}

/// The first leg to reach `/data/{id}` parks here until the second leg arrives.
pub struct Parked {
    pub socket: WebSocket,
    pub role: String,
    pub done: oneshot::Sender<()>,
}

/// Map of session_id -> parked first leg.
pub type Rendezvous = Mutex<HashMap<String, Parked>>;

/// Map of session_id -> brokered session (token + target agent).
pub type Sessions = Mutex<HashMap<String, SessionInfo>>;

/// How long the first leg waits for its peer before giving up.
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);

/// Run the rendezvous for one `/data/{id}` connection.
///
/// First leg parks its socket and waits (with timeout). Second leg (must be the *other* role)
/// removes the parked socket and bridges the two. The single-use token is consumed here, the
/// moment a pair is established — not when the first leg arrives (that would strand the second).
pub async fn rendezvous(
    rv: &Rendezvous,
    sessions: &Sessions,
    session_id: String,
    role: String,
    socket: WebSocket,
) {
    let mut guard = rv.lock().await;
    if let Some(parked) = guard.remove(&session_id) {
        // We are the second leg.
        drop(guard);
        if parked.role == role {
            // Two legs of the same role can't form a tunnel — refuse and release the first.
            let _ = parked.done.send(());
            return;
        }
        // Pair established → burn the single-use token so no third leg can join.
        sessions.lock().await.remove(&session_id);
        bridge(socket, parked.socket).await;
        let _ = parked.done.send(());
    } else {
        // We are the first leg — park and wait for the peer (or time out).
        let (done_tx, done_rx) = oneshot::channel();
        guard.insert(session_id.clone(), Parked { socket, role, done: done_tx });
        drop(guard);
        tokio::select! {
            _ = done_rx => {}
            _ = tokio::time::sleep(PAIR_TIMEOUT) => {
                // Idempotent cleanup: if still present, the peer never came; if absent, the peer
                // already took our socket and is mid-bridge (the dropped done_rx is harmless).
                rv.lock().await.remove(&session_id);
            }
        }
    }
}

/// Bidirectional byte bridge between two WebSocket legs. Forwards Binary/Text both ways and
/// propagates Close; returns when either side ends.
async fn bridge(a: WebSocket, b: WebSocket) {
    let (mut a_tx, mut a_rx) = a.split();
    let (mut b_tx, mut b_rx) = b.split();

    let a2b = async move {
        while let Some(Ok(msg)) = a_rx.next().await {
            match msg {
                Message::Binary(_) | Message::Text(_) => {
                    if b_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => {
                    let _ = b_tx.send(Message::Close(None)).await;
                    break;
                }
                _ => {}
            }
        }
    };
    let b2a = async move {
        while let Some(Ok(msg)) = b_rx.next().await {
            match msg {
                Message::Binary(_) | Message::Text(_) => {
                    if a_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => {
                    let _ = a_tx.send(Message::Close(None)).await;
                    break;
                }
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = a2b => {}
        _ = b2a => {}
    }
}
