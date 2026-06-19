//! remota-agent binary. Runs on the remote machine (behind NAT), phones home to a relay.

use clap::Parser;
use uuid::Uuid;

use remota_agent::{run_agent, AgentConfig};

#[derive(Parser, Debug)]
#[command(name = "remota-agent", version, about = "Remota self-hosted remote-access agent")]
struct Args {
    /// Relay base WS URL, e.g. ws://relay.example:8787 (TLS: wss://relay.example).
    #[arg(long)]
    relay: String,
    /// Enrollment token (shared secret presented on register).
    #[arg(long)]
    token: String,
    /// Friendly name shown in Remota.
    #[arg(long, default_value = "remota-agent")]
    name: String,
    /// Stable agent id (default: random per run).
    #[arg(long)]
    id: Option<String>,
    /// Comma-separated capabilities advertised to the relay.
    #[arg(long, value_delimiter = ',', default_value = "cli")]
    capabilities: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let agent_id = args.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let cfg = AgentConfig {
        relay_base: args.relay,
        enroll_token: args.token,
        agent_id,
        name: args.name,
        os: std::env::consts::OS.to_string(),
        capabilities: args.capabilities,
    };

    println!(
        "remota-agent: connecting to {} as \"{}\" (id={}, caps={:?})",
        cfg.relay_base, cfg.name, cfg.agent_id, cfg.capabilities
    );
    run_agent(cfg).await
}
