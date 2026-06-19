//! Control-channel messages between `remota-agent` and `remota-relay` (JSON over WSS).

use serde::{Deserialize, Serialize};

/// Agent → relay (control channel).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMsg {
    /// First message after connecting: enroll/register this agent.
    Register {
        agent_id: String,
        name: String,
        os: String,
        capabilities: Vec<String>, // "cli", "screen"
        token: String,
    },
    /// Periodic keepalive / presence.
    Heartbeat,
}

/// Relay → agent (control channel).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMsg {
    Registered { ok: bool },
    Error { msg: String },
    /// Ask the agent to open a data channel back to the relay and connect to a local target.
    /// The agent presents `token` (single-use per session) when it connects to `/data/{session_id}`.
    OpenChannel {
        session_id: String,
        token: String,
        target_host: String,
        target_port: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_register_roundtrips() {
        let m = AgentMsg::Register {
            agent_id: "a1".into(),
            name: "pi".into(),
            os: "linux".into(),
            capabilities: vec!["cli".into(), "screen".into()],
            token: "tok".into(),
        };
        let back: AgentMsg = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn relay_open_channel_roundtrips() {
        let m = RelayMsg::OpenChannel {
            session_id: "s1".into(),
            token: "tok".into(),
            target_host: "127.0.0.1".into(),
            target_port: 22,
        };
        let back: RelayMsg = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, back);
    }
}
