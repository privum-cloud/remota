//! remota-relay — self-hosted broker (WSS) for remota-agent connections.
//!
//! See docs/superpowers/plans/2026-06-19-m-agent-0-relay-agent-mvp.md
//! and docs/superpowers/specs/2026-06-19-remota-agent-relay-design.md.

use remota_proto::{AgentMsg, RelayMsg};

#[tokio::main]
async fn main() {
    println!("remota-relay {} (skeleton)", env!("CARGO_PKG_VERSION"));
    println!("Control messages available: {}", message_catalog());
    println!("Next: WSS control channel (/agent/control) + session brokering (M-agent-0 T3/T4).");
}

/// Sanity reference to the shared protocol so it's wired into the build.
fn message_catalog() -> String {
    let reg = AgentMsg::Heartbeat;
    let ok = RelayMsg::Registered { ok: true };
    format!("{:?} / {:?}", reg, ok)
}
