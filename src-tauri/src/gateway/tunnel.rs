use std::sync::Arc;

use russh::client::{self, Config, Handle, Handler};
use russh::keys::{PrivateKeyWithHashAlg, PublicKey};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::model::Gateway;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

/// Stream genérico (TCP direto ou canal SSH tunelado).
pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

pub struct AcceptAllHandler;

impl Handler for AcceptAllHandler {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true) // homelab/dev — known_hosts é melhoria futura
    }
}

pub fn split_host_port(target: &str) -> (String, u16) {
    match target.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(22)),
        None => (target.to_string(), 22),
    }
}

/// Liga ao jump host por SSH e autentica por password.
pub async fn connect_jump(gw: &Gateway) -> Result<Handle<AcceptAllHandler>, BoxErr> {
    let config = Arc::new(Config::default());
    let port = gw.port.unwrap_or(22);
    let mut jump = client::connect(config, (gw.host.as_str(), port), AcceptAllHandler).await?;
    let user = gw.username.clone().unwrap_or_default();
    let ok = if let Some(kp) = &gw.key_path {
        let key = russh::keys::load_secret_key(kp, None)
            .map_err(|e| format!("failed to load jump SSH key {kp}: {e}"))?;
        let hash_alg = jump.best_supported_rsa_hash().await?.flatten();
        jump.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg))
            .await?
    } else {
        jump.authenticate_password(user, gw.password.clone().unwrap_or_default()).await?
    };
    if !ok.success() {
        return Err("jump host authentication failed".into());
    }
    Ok(jump)
}

/// Stream ao destino: direto (TCP) ou tunelado por um jump host (SSH `direct-tcpip`).
/// O `Handle` do jump devolvido tem de ficar VIVO durante toda a sessão (senão o túnel fecha).
pub async fn connect_target(
    target: &str,
    gateway: &Option<Gateway>,
) -> Result<(Box<dyn AsyncStream>, Option<Handle<AcceptAllHandler>>), BoxErr> {
    match gateway {
        None => {
            let tcp = tokio::net::TcpStream::connect(target).await?;
            Ok((Box::new(tcp), None))
        }
        Some(gw) => {
            let (host, port) = split_host_port(target);
            let jump = connect_jump(gw).await?;
            let channel = jump
                .channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0)
                .await?;
            Ok((Box::new(channel.into_stream()), Some(jump)))
        }
    }
}
