//! remota-relay binary — binds a TCP listener and serves the broker.
//!
//! Env:
//!   REMOTA_RELAY_LISTEN  (default 127.0.0.1:8787)
//!   REMOTA_ENROLL_TOKEN  (default "dev-enroll" — set a real secret in production)
//!
//! TLS is terminated by a reverse proxy in front of the relay (see lib.rs / M-agent-0 plan).

use remota_relay::build_app;

#[tokio::main]
async fn main() {
    let listen = std::env::var("REMOTA_RELAY_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let enroll = std::env::var("REMOTA_ENROLL_TOKEN").unwrap_or_else(|_| "dev-enroll".into());

    let app = build_app(enroll);
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|e| panic!("bind {listen}: {e}"));

    println!("remota-relay listening on ws://{listen}  (control: /agent/control, data: /data/{{id}})");
    axum::serve(listener, app).await.expect("relay serve");
}
