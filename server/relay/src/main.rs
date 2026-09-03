//! remota-relay binary — binds a TCP listener and serves the broker.
//!
//! Env:
//!   REMOTA_RELAY_LISTEN  (default 127.0.0.1:8787)
//!   REMOTA_ENROLL_TOKEN  (REQUIRED — the relay refuses to start without a real secret)
//!
//! TLS is terminated by a reverse proxy in front of the relay (see lib.rs / M-agent-0 plan).

use remota_relay::{build_app, validate_enroll_token};

#[tokio::main]
async fn main() {
    let listen = std::env::var("REMOTA_RELAY_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());

    // No usable default: `/agent/control` is reachable from anywhere by design, so a placeholder
    // secret would let any host on the internet register an agent. Refuse to serve instead.
    let enroll = match validate_enroll_token(std::env::var("REMOTA_ENROLL_TOKEN").ok().as_deref()) {
        Ok(token) => token,
        Err(why) => {
            eprintln!("remota-relay: refusing to start.\n{why}");
            std::process::exit(1);
        }
    };

    let app = build_app(enroll);
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|e| panic!("bind {listen}: {e}"));

    println!("remota-relay listening on ws://{listen}  (control: /agent/control, data: /data/{{id}})");
    axum::serve(listener, app).await.expect("relay serve");
}
