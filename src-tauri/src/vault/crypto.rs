use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KdfParams {
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        // Perfil OWASP para Argon2id: 19 MiB, 2 iterações, 1 lane.
        Self {
            m_cost: 19456,
            t_cost: 2,
            p_cost: 1,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub enum VaultError {
    Crypto,
    BadFormat,
    Locked,
    Io(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Crypto => write!(
                f,
                "encryption/decryption failed (wrong password or corrupted data)"
            ),
            VaultError::BadFormat => write!(f, "invalid vault file format"),
            VaultError::Locked => write!(f, "vault is locked (unlock first)"),
            VaultError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}
impl std::error::Error for VaultError {}

pub fn derive_key(password: &str, salt: &[u8], params: KdfParams) -> Zeroizing<[u8; 32]> {
    let p = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(32))
        .expect("params Argon2 válidos");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), salt, key.as_mut_slice())
        .expect("derivação Argon2id");
    key
}

pub fn seal(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .expect("AES-256-GCM encrypt não falha com nonce/len válidos")
}

pub fn open(key: &[u8; 32], nonce: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| VaultError::Crypto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_then_open_roundtrips() {
        let params = KdfParams::default();
        let salt = [7u8; 16];
        let nonce = [3u8; 12];
        let key = derive_key("senha-correta", &salt, params);
        let ct = seal(&key, &nonce, b"documento secreto");
        let pt = open(&key, &nonce, &ct).expect("deve decifrar");
        assert_eq!(pt, b"documento secreto");
    }

    #[test]
    fn open_with_wrong_password_fails() {
        let params = KdfParams::default();
        let salt = [7u8; 16];
        let nonce = [3u8; 12];
        let good = derive_key("senha-correta", &salt, params);
        let bad = derive_key("senha-errada", &salt, params);
        let ct = seal(&good, &nonce, b"documento secreto");
        assert!(matches!(open(&bad, &nonce, &ct), Err(VaultError::Crypto)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = derive_key("x", &[1u8; 16], KdfParams::default());
        let nonce = [0u8; 12];
        let mut ct = seal(&key, &nonce, b"abc");
        let last = ct.len() - 1;
        ct[last] ^= 0xFF;
        assert!(open(&key, &nonce, &ct).is_err());
    }
}
