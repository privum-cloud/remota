//! remota-agent binary. Runs on the remote machine (behind NAT), phones home to a relay.

use std::path::{Path, PathBuf};

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
    /// Friendly name shown in Remota (display label only).
    #[arg(long, default_value = "remota-agent")]
    name: String,
    /// Override the device id. If omitted, a random id is generated and persisted
    /// in <state-dir>/agent-id on first run, then reused (AnyDesk-style address).
    #[arg(long)]
    id: Option<String>,
    /// Where the generated device id is persisted.
    #[arg(long, default_value = "/var/lib/remota-agent")]
    state_dir: PathBuf,
    /// Comma-separated capabilities advertised to the relay.
    #[arg(long, value_delimiter = ',', default_value = "cli")]
    capabilities: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let agent_id = resolve_agent_id(args.id, &args.state_dir)?;

    let cfg = AgentConfig {
        relay_base: args.relay,
        enroll_token: args.token,
        agent_id,
        name: args.name,
        os: std::env::consts::OS.to_string(),
        capabilities: args.capabilities,
    };

    println!("remota-agent: device id = {}", cfg.agent_id);
    println!(
        "remota-agent: connecting to {} as \"{}\" (caps={:?})",
        cfg.relay_base, cfg.name, cfg.capabilities
    );
    run_agent(cfg).await
}

/// Resolve the device id: explicit override wins; else read the persisted id;
/// else generate a fresh random id and persist it under `state_dir`.
fn resolve_agent_id(explicit: Option<String>, state_dir: &Path) -> anyhow::Result<String> {
    if let Some(id) = explicit {
        return Ok(id);
    }
    let path = state_dir.join("agent-id");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let id = existing.trim().to_string();
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let id = generate_id();
    std::fs::create_dir_all(state_dir).map_err(|e| {
        anyhow::anyhow!("cannot create state dir {}: {e} (pass --state-dir to a writable path)", state_dir.display())
    })?;
    std::fs::write(&path, format!("{id}\n"))?;
    Ok(id)
}

/// 12 random digits grouped AnyDesk-style: XXXX-XXXX-XXXX (non-guessable address).
fn generate_id() -> String {
    let n = (Uuid::new_v4().as_u128() % 1_000_000_000_000) as u64;
    let s = format!("{n:012}");
    format!("{}-{}-{}", &s[0..4], &s[4..8], &s[8..12])
}
