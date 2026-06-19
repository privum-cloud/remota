//! remota-agent — connects out (WSS) to a remota-relay and tunnels local services.
//!
//! Runs on the remote machine (behind NAT). Phones home to the relay, registers,
//! and on demand opens data channels that bridge a local service (SSH/VNC/RDP) to the relay.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "remota-agent", version, about = "Remota self-hosted remote-access agent")]
struct Args {
    /// Relay WSS URL, e.g. wss://relay.example/agent/control
    #[arg(long)]
    relay: Option<String>,
    /// Enrollment token (one-time).
    #[arg(long)]
    token: Option<String>,
    /// Friendly name shown in Remota.
    #[arg(long)]
    name: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    println!("remota-agent {} (skeleton)", env!("CARGO_PKG_VERSION"));
    println!(
        "relay={:?} name={:?} token={}",
        args.relay,
        args.name,
        if args.token.is_some() { "<set>" } else { "<none>" }
    );
    println!("Next: connect WSS, Register, heartbeat, OpenChannel→local tunnel (M-agent-0 T3/T4).");
}
