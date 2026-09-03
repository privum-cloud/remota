//! RDP transport — an **RDCleanPath proxy** for the `ironrdp-web` (WASM) client.
//!
//! The browser does the RDP protocol + CredSSP/NLA itself. This proxy does the parts a
//! browser can't: TCP connect, the X.224 negotiation, and the TLS handshake to the Windows
//! host — then it returns the server's certificate chain (so the client can do CredSSP channel
//! binding) and relays plaintext RDP bytes between the WebSocket and the TLS socket.
//!
//! Protocol: the client sends an RDCleanPath *request* (DER) with the destination and the X.224
//! connection request; we reply with an RDCleanPath *response* (DER) carrying the X.224 confirm,
//! the server cert chain and the resolved address; after that it's a raw byte relay.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use ironrdp_rdcleanpath::{DetectionResult, RDCleanPath, RDCleanPathPdu};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use crate::gateway::SessionSpec;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub async fn proxy_rdp(socket: WebSocket, _spec: SessionSpec) {
    if let Err(e) = run(socket).await {
        eprintln!("rdp proxy error: {e}");
    }
}

async fn run(socket: WebSocket) -> Result<(), BoxErr> {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // 1) Read the RDCleanPath request (DER) from the WS and extract destination + X.224.
    let req_bytes = read_rdcleanpath(&mut ws_rx).await?;
    let pdu = RDCleanPathPdu::from_der(&req_bytes).map_err(|e| format!("bad RDCleanPath DER: {e}"))?;
    let (destination, x224_req) = match pdu.into_enum().map_err(|e| format!("bad RDCleanPath: {e}"))? {
        RDCleanPath::Request { destination, x224_connection_request, .. } => {
            (destination, x224_connection_request.as_bytes().to_vec())
        }
        _ => return Err("expected an RDCleanPath request".into()),
    };

    // 2) TCP + X.224 negotiation, then 3) TLS upgrade + capture the cert chain.
    let (host, port) = split_host_port(&destination);
    let (tls, x224_resp, certs) = match negotiate_and_tls(&x224_req, &host, port).await {
        Ok(v) => v,
        Err(e) => {
            // Best-effort: tell the client it was a connection/negotiation problem.
            let err = RDCleanPathPdu::new_general_error();
            if let Ok(der) = err.to_der() {
                let _ = ws_tx.send(Message::Binary(der.into())).await;
            }
            return Err(e);
        }
    };

    // 4) RDCleanPath response: X.224 confirm + server cert chain + resolved address.
    let resp = RDCleanPathPdu::new_response(format!("{host}:{port}"), x224_resp, certs)
        .map_err(|e| format!("build RDCleanPath response: {e}"))?;
    let resp_der = resp.to_der().map_err(|e| format!("encode RDCleanPath response: {e}"))?;
    ws_tx.send(Message::Binary(resp_der.into())).await?;

    // 5) Relay plaintext RDP bytes: WS <-> TLS (rustls encrypts to the server).
    relay(ws_tx, ws_rx, tls).await;
    Ok(())
}

/// TCP connect → send X.224 CR → read X.224 CC → TLS upgrade → capture peer cert chain.
async fn negotiate_and_tls(
    x224_req: &[u8],
    host: &str,
    port: u16,
) -> Result<(TlsStream<TcpStream>, Vec<u8>, Vec<Vec<u8>>), BoxErr> {
    let mut tcp = TcpStream::connect((host, port)).await?;
    tcp.write_all(x224_req).await?;
    let x224_resp = read_tpkt(&mut tcp).await?;

    // rustls 0.23 needs the process-default provider (ring) — shared with the relay client.
    crate::gateway::relay::ensure_crypto_provider();
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAllServerCert))
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let tls = connector.connect(server_name_for(host)?, tcp).await?;

    let certs = tls
        .get_ref()
        .1
        .peer_certificates()
        .map(|cs| cs.iter().map(|c| c.as_ref().to_vec()).collect())
        .unwrap_or_default();
    Ok((tls, x224_resp, certs))
}

async fn relay(
    mut ws_tx: futures_util::stream::SplitSink<WebSocket, Message>,
    mut ws_rx: SplitStream<WebSocket>,
    tls: TlsStream<TcpStream>,
) {
    let (mut tls_rd, mut tls_wr) = tokio::io::split(tls);

    let ws_to_tls = async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(b) => {
                    if tls_wr.write_all(&b).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let _ = tls_wr.shutdown().await;
    };
    let tls_to_ws = async move {
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            match tls_rd.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws_tx.send(Message::Binary(buf[..n].to_vec().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = ws_tx.send(Message::Close(None)).await;
    };
    tokio::select! {
        _ = ws_to_tls => {}
        _ = tls_to_ws => {}
    }
}

/// Accumulate WS frames until a full RDCleanPath PDU is buffered (DER self-describes its length).
async fn read_rdcleanpath(ws_rx: &mut SplitStream<WebSocket>) -> Result<Vec<u8>, BoxErr> {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match RDCleanPathPdu::detect(&buf) {
            DetectionResult::Detected { total_length, .. } => {
                buf.truncate(total_length);
                return Ok(buf);
            }
            DetectionResult::NotEnoughBytes => {}
            DetectionResult::Failed => return Err("malformed RDCleanPath framing".into()),
        }
        match ws_rx.next().await {
            Some(Ok(Message::Binary(b))) => buf.extend_from_slice(&b),
            Some(Ok(Message::Text(t))) => buf.extend_from_slice(t.as_bytes()),
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(Box::new(e)),
            None => return Err("WS closed before the RDCleanPath request".into()),
        }
        if buf.len() > 1 << 20 {
            return Err("RDCleanPath request too large".into());
        }
    }
}

/// Read one TPKT packet (X.224 confirm): `03 00 <len_hi> <len_lo>` header + body.
async fn read_tpkt(tcp: &mut TcpStream) -> Result<Vec<u8>, BoxErr> {
    let mut hdr = [0u8; 4];
    tcp.read_exact(&mut hdr).await?;
    if hdr[0] != 0x03 {
        return Err("target did not answer with a TPKT (X.224) packet".into());
    }
    let len = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
    if len < 4 {
        return Err("bad TPKT length".into());
    }
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&hdr);
    out.resize(len, 0);
    tcp.read_exact(&mut out[4..]).await?;
    Ok(out)
}

fn split_host_port(target: &str) -> (String, u16) {
    match target.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(3389)),
        None => (target.to_string(), 3389),
    }
}

fn server_name_for(host: &str) -> Result<rustls::pki_types::ServerName<'static>, BoxErr> {
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        Ok(rustls::pki_types::ServerName::IpAddress(ip.into()))
    } else {
        Ok(rustls::pki_types::ServerName::try_from(host.to_string())?)
    }
}

/// TLS verifier that accepts any server certificate (RDP hosts use self-signed certs; the real
/// authentication is CredSSP/NLA, done by the client using the cert chain we return).
#[derive(Debug)]
struct AcceptAllServerCert;

impl rustls::client::danger::ServerCertVerifier for AcceptAllServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        use rustls::SignatureScheme as S;
        vec![
            S::RSA_PKCS1_SHA256, S::RSA_PKCS1_SHA384, S::RSA_PKCS1_SHA512,
            S::ECDSA_NISTP256_SHA256, S::ECDSA_NISTP384_SHA384,
            S::RSA_PSS_SHA256, S::RSA_PSS_SHA384, S::RSA_PSS_SHA512,
            S::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live check of the hard part (X.224 negotiation + TLS + cert capture) against a real
    /// Windows RDP host. Run with:
    ///   RDP_TARGET=192.168.1.143:3389 cargo test -p remota gateway::rdp::tests::x224_tls -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn x224_tls_and_cert_capture() {
        let target = std::env::var("RDP_TARGET").expect("RDP_TARGET=host:port");
        let (host, port) = split_host_port(&target);

        // Minimal X.224 Connection Request asking for TLS + CredSSP (PROTOCOL_SSL | HYBRID).
        let x224_cr: [u8; 19] = [
            0x03, 0x00, 0x00, 0x13, // TPKT header, len 19
            0x0e, 0xe0, 0x00, 0x00, 0x00, 0x00, 0x00, // X.224 CR
            0x01, 0x00, 0x08, 0x00, 0x03, 0x00, 0x00, 0x00, // RDP Neg Req: requestedProtocols = 0x03
        ];

        let (_tls, x224_resp, certs) = negotiate_and_tls(&x224_cr, &host, port)
            .await
            .expect("negotiate + TLS to the RDP host");

        println!("X.224 confirm: {} bytes; cert chain: {} cert(s)", x224_resp.len(), certs.len());
        assert!(x224_resp.first() == Some(&0x03), "X.224 confirm should be a TPKT");
        assert!(!certs.is_empty(), "should have captured the server certificate chain over TLS");
        assert!(certs[0].len() > 100, "the leaf cert should be real DER");
    }
}
