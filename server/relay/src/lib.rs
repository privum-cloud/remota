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
    if token != *st.enroll_token {
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
    let valid = matches!(st.sessions.lock().await.get(&id), Some(s) if s.token == q.token);
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
