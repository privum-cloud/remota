//! remota-relay — self-hosted broker. Agents connect out over WS to `/agent/control` and
//! register; the app brokers a session via `POST /session`; both legs meet at `/data/{id}`
//! where the relay bridges raw bytes. Single-use token per session.
//!
//! TLS is intentionally NOT handled here (MVP): terminate it at a reverse proxy
//! (Caddy/nginx, Let's Encrypt) in front of the relay. See the M-agent-0 plan.

pub mod registry;
pub mod session;

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::mpsc;
use uuid::Uuid;

use remota_proto::{AgentMsg, RelayMsg};

use registry::{AgentConn, AgentInfo, Registry};
use session::{rendezvous, Rendezvous, SessionInfo, Sessions};

#[derive(Clone)]
pub struct AppState {
    agents: Arc<Registry>,
    sessions: Arc<Sessions>,
    rendezvous: Arc<Rendezvous>,
    /// Simple shared enrollment secret an agent must present to register (MVP).
    enroll_token: Arc<String>,
}

impl AppState {
    pub fn new(enroll_token: String) -> Self {
        Self {
            agents: Arc::new(Registry::new()),
            sessions: Arc::new(Sessions::new(HashMap::new())),
            rendezvous: Arc::new(Rendezvous::new(HashMap::new())),
            enroll_token: Arc::new(enroll_token),
        }
    }
}

/// Build the axum app. `enroll_token` is the shared secret agents present on `Register`.
pub fn build_app(enroll_token: String) -> Router {
    Router::new()
        .route("/agent/control", get(agent_control))
        .route("/agents", get(list_agents))
        .route("/session", post(create_session))
        .route("/data/{id}", get(data_channel))
        .with_state(AppState::new(enroll_token))
}

/// Serialize a RelayMsg to a WS text frame.
fn relay_text(msg: &RelayMsg) -> Message {
    Message::Text(serde_json::to_string(msg).expect("RelayMsg serializes").into())
}

// ---- secrets --------------------------------------------------------------

/// The placeholder enrollment secret this relay used to fall back to. It is public knowledge
/// (it is in the git history and in the docs), so a deployment must never run with it.
pub const INSECURE_ENROLL_TOKEN: &str = "dev-enroll";

/// Shortest enrollment secret we accept. `install.sh` generates 40 random characters.
pub const MIN_ENROLL_TOKEN_LEN: usize = 16;

/// Compare two secrets without branching on their contents.
///
/// Both operands are hashed to a fixed-size digest first: `ct_eq` over the raw bytes is still
/// length-dependent, so hashing keeps *both* the contents and the length of the secret out of
/// the timing. Cheap enough — this runs once per connection attempt, not per byte.
fn secret_eq(a: &str, b: &str) -> bool {
    let a = Sha256::digest(a.as_bytes());
    let b = Sha256::digest(b.as_bytes());
    a.ct_eq(&b).into()
}

/// Validate the operator-supplied enrollment secret at startup.
///
/// The relay refuses to run without a real one: an unset or placeholder token means any host on
/// the internet can register an agent, and `/agent/control` is deliberately reachable from
/// anywhere (agents dial out from behind NAT). Fail loudly at boot rather than quietly serve.
pub fn validate_enroll_token(value: Option<&str>) -> Result<String, String> {
    const HOWTO: &str =
        "Generate one with:  head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 40\n\
         and set REMOTA_ENROLL_TOKEN (see server/deploy/relay.env.example).";
    let token = value.unwrap_or("").trim();
    if token.is_empty() {
        return Err(format!("REMOTA_ENROLL_TOKEN is not set.\n{HOWTO}"));
    }
    if token == INSECURE_ENROLL_TOKEN {
        return Err(format!(
            "REMOTA_ENROLL_TOKEN is still the placeholder {INSECURE_ENROLL_TOKEN:?}, which is \
             publicly known.\n{HOWTO}"
        ));
    }
    if token.chars().count() < MIN_ENROLL_TOKEN_LEN {
        return Err(format!(
            "REMOTA_ENROLL_TOKEN is too short (minimum {MIN_ENROLL_TOKEN_LEN} characters).\n{HOWTO}"
        ));
    }
    Ok(token.to_string())
}

// ---- control channel ------------------------------------------------------

async fn agent_control(ws: WebSocketUpgrade, State(st): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_agent_control(socket, st))
}

async fn handle_agent_control(socket: WebSocket, st: AppState) {
    let (mut sink, mut stream) = socket.split();

    // First frame must be a valid Register with the right enrollment token.
    let reg = match stream.next().await {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<AgentMsg>(t.as_str()).ok(),
        _ => None,
    };
    let (agent_id, name, os, capabilities, token) = match reg {
        Some(AgentMsg::Register { agent_id, name, os, capabilities, token }) => {
            (agent_id, name, os, capabilities, token)
        }
        _ => {
            let _ = sink.send(relay_text(&RelayMsg::Error { msg: "expected register".into() })).await;
            return;
        }
    };
    // Constant-time: `/agent/control` is open to the internet, so this comparison is the one
    // attacker-reachable check on a long-lived shared secret.
    if !secret_eq(&token, &st.enroll_token) {
        let _ = sink.send(relay_text(&RelayMsg::Error { msg: "bad enrollment token".into() })).await;
        return;
    }

    let (tx, mut rx) = mpsc::channel::<RelayMsg>(32);
    st.agents
        .insert(agent_id.clone(), AgentConn { name, os, capabilities, tx })
        .await;
    let _ = sink.send(relay_text(&RelayMsg::Registered { ok: true })).await;

    // Writer task: drain the per-agent channel onto the WS sink.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(relay_text(&msg)).await.is_err() {
                break;
            }
        }
    });

    // Reader loop: heartbeats keep presence; any close/error tears the agent down.
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(_) => { /* Heartbeat — presence implied by a live socket */ }
            Message::Close(_) => break,
            _ => {}
        }
    }

    st.agents.remove(&agent_id).await;
    writer.abort();
}

// ---- debug listing --------------------------------------------------------

async fn list_agents(State(st): State<AppState>) -> Json<Vec<AgentInfo>> {
    Json(st.agents.list().await)
}

// ---- session brokering ----------------------------------------------------

#[derive(Deserialize)]
struct OpenReq {
    agent_id: String,
    #[serde(default)]
    target_host: Option<String>,
    target_port: u16,
}

#[derive(Serialize)]
struct OpenResp {
    session_id: String,
    token: String,
}

async fn create_session(
    State(st): State<AppState>,
    Json(req): Json<OpenReq>,
) -> Result<Json<OpenResp>, (StatusCode, String)> {
    let tx = st
        .agents
        .sender(&req.agent_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "agent offline".into()))?;

    let session_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let target_host = req.target_host.unwrap_or_else(|| "127.0.0.1".into());

    st.sessions.lock().await.insert(
        session_id.clone(),
        SessionInfo { token: token.clone(), agent_id: req.agent_id.clone() },
    );

    tx.send(RelayMsg::OpenChannel {
        session_id: session_id.clone(),
        token: token.clone(),
        target_host,
        target_port: req.target_port,
    })
    .await
    .map_err(|_| (StatusCode::BAD_GATEWAY, "agent unreachable".into()))?;

    Ok(Json(OpenResp { session_id, token }))
}

// ---- data channel (rendezvous) -------------------------------------------

#[derive(Deserialize)]
struct DataQuery {
    token: String,
    role: String,
}

async fn data_channel(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    Query(q): Query<DataQuery>,
    State(st): State<AppState>,
) -> Response {
    // Validate token against the session, but do NOT consume it yet — both legs present it.
    // Reaching the comparison at all requires knowing `id`, a UUIDv4 that only the app and the
    // agent are told, so this is not an oracle on the token; compare in constant time anyway.
    // NOTE: a brokered session whose legs never pair is never removed from the map — see the
    // known-limitations list in SECURITY.md.
    let valid = match st.sessions.lock().await.get(&id) {
        Some(s) => secret_eq(&s.token, &q.token),
        None => false,
    };
    if !valid {
        return (StatusCode::FORBIDDEN, "bad session or token").into_response();
    }
    if q.role != "client" && q.role != "agent" {
        return (StatusCode::BAD_REQUEST, "role must be client|agent").into_response();
    }

    let role = q.role.clone();
    ws.on_upgrade(move |socket| async move {
        rendezvous(&st.rendezvous, &st.sessions, id, role, socket).await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_eq_matches_only_identical_secrets() {
        assert!(secret_eq("s3cret-token", "s3cret-token"));
        assert!(!secret_eq("s3cret-token", "s3cret-tokeM"));
        // Length differences must not be treated as a match either.
        assert!(!secret_eq("s3cret-token", "s3cret-token-longer"));
        assert!(!secret_eq("", "s3cret-token"));
        assert!(secret_eq("", ""));
    }

    #[test]
    fn enroll_token_must_be_set() {
        assert!(validate_enroll_token(None).is_err());
        assert!(validate_enroll_token(Some("")).is_err());
        assert!(validate_enroll_token(Some("   ")).is_err());
    }

    #[test]
    fn enroll_token_rejects_the_public_placeholder() {
        let err = validate_enroll_token(Some(INSECURE_ENROLL_TOKEN)).unwrap_err();
        assert!(err.contains("publicly known"), "{err}");
    }

    #[test]
    fn enroll_token_rejects_short_secrets() {
        assert!(validate_enroll_token(Some("short")).is_err());
        let ok = "x".repeat(MIN_ENROLL_TOKEN_LEN);
        assert_eq!(validate_enroll_token(Some(&ok)).unwrap(), ok);
    }

    #[test]
    fn enroll_token_is_trimmed() {
        // Env files and shell exports pick up stray whitespace; a token that differs only by
        // trailing newline must not silently become a *different* secret.
        let token = "  a-perfectly-fine-enrollment-secret  ";
        assert_eq!(
            validate_enroll_token(Some(token)).unwrap(),
            "a-perfectly-fine-enrollment-secret"
        );
    }
}
