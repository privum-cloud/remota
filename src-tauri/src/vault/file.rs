use std::path::Path;

use crate::vault::{derive_key, open, seal, KdfParams, VaultError};

const VERSION: u8 = 1;
const HEADER_LEN: usize = 1 + 4 + 4 + 4 + 16 + 12; // 41

pub fn save_document(
    path: &Path,
    password: &str,
    params: KdfParams,
    plaintext: &[u8],
) -> Result<(), VaultError> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut salt).map_err(|e| VaultError::Io(e.to_string()))?;
    getrandom::getrandom(&mut nonce).map_err(|e| VaultError::Io(e.to_string()))?;

    let key = derive_key(password, &salt, params);
    let ciphertext = seal(&key, &nonce, plaintext);

    let mut buf = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    buf.push(VERSION);
    buf.extend_from_slice(&params.m_cost.to_le_bytes());
    buf.extend_from_slice(&params.t_cost.to_le_bytes());
    buf.extend_from_slice(&params.p_cost.to_le_bytes());
    buf.extend_from_slice(&salt);
    buf.extend_from_slice(&nonce);
    buf.extend_from_slice(&ciphertext);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| VaultError::Io(e.to_string()))?;
    }
    std::fs::write(path, &buf).map_err(|e| VaultError::Io(e.to_string()))
}

pub fn load_document(path: &Path, password: &str) -> Result<Vec<u8>, VaultError> {
    let buf = std::fs::read(path).map_err(|e| VaultError::Io(e.to_string()))?;
    open_document(&buf, password)
}

/// `true` if `bytes` look like a sealed document (the VERSION byte plus at least a full
/// header), as opposed to plaintext JSON (which begins with `{`). The discriminator is the
/// header, not the file extension, because exported files get renamed. Used to auto-detect
/// encrypted vs plaintext on import.
pub fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.first() == Some(&VERSION) && bytes.len() >= HEADER_LEN
}

/// Decrypt an already-read sealed document — the same format `save_document` writes. Split
/// out of `load_document` so the import path can sniff and decrypt from bytes it already has.
pub fn open_document(bytes: &[u8], password: &str) -> Result<Vec<u8>, VaultError> {
    if bytes.len() < HEADER_LEN || bytes[0] != VERSION {
        return Err(VaultError::BadFormat);
    }
    let m_cost = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
    let t_cost = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
    let p_cost = u32::from_le_bytes(bytes[9..13].try_into().unwrap());
    let salt: [u8; 16] = bytes[13..29].try_into().unwrap();
    let nonce: [u8; 12] = bytes[29..41].try_into().unwrap();
    let ciphertext = &bytes[41..];

    let params = KdfParams { m_cost, t_cost, p_cost };
    // Reject any header whose KDF params aren't the ones we write. Every file this app
    // produces (the vault and exports) seals with `KdfParams::default()`; refusing anything
    // else keeps an untrusted or corrupt import header from driving Argon2 to panic
    // (m_cost < MIN_M_COST) or to a pathological allocation (m_cost near u32::MAX), before any
    // key is derived. If the default is ever changed, bump VERSION and migrate here.
    if params != KdfParams::default() {
        return Err(VaultError::BadFormat);
    }
    let key = derive_key(password, &salt, params);
    open(&key, &nonce, ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::KdfParams;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        // nome único por teste; sem Date/rand: usa o nome do teste.
        p.push(format!("remota-test-{name}.dat"));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn save_then_load_roundtrips() {
        let path = tmp_path("roundtrip");
        save_document(&path, "mestra", KdfParams::default(), b"{\"nodes\":[]}").unwrap();
        let got = load_document(&path, "mestra").unwrap();
        assert_eq!(got, b"{\"nodes\":[]}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_with_wrong_password_fails() {
        let path = tmp_path("wrongpw");
        save_document(&path, "mestra", KdfParams::default(), b"segredo").unwrap();
        assert!(matches!(
            load_document(&path, "errada"),
            Err(crate::vault::VaultError::Crypto)
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn nonce_and_salt_differ_between_saves() {
        let p1 = tmp_path("rng1");
        let p2 = tmp_path("rng2");
        save_document(&p1, "m", KdfParams::default(), b"x").unwrap();
        save_document(&p2, "m", KdfParams::default(), b"x").unwrap();
        let a = std::fs::read(&p1).unwrap();
        let b = std::fs::read(&p2).unwrap();
        // bytes 13..41 = salt(16)+nonce(12) devem diferir (aleatórios)
        assert_ne!(a[13..41], b[13..41]);
        std::fs::remove_file(&p1).ok();
        std::fs::remove_file(&p2).ok();
    }

    #[test]
    fn is_encrypted_true_for_sealed_bytes() {
        let path = tmp_path("sniff-sealed");
        save_document(&path, "pw", KdfParams::default(), b"{\"nodes\":[]}").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(is_encrypted(&bytes), "a sealed vault file must sniff as encrypted");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn is_encrypted_false_for_plaintext_json() {
        assert!(!is_encrypted(b"{\"nodes\":[]}"));
    }

    #[test]
    fn is_encrypted_false_for_short_bytes() {
        // Starts with the VERSION byte but is shorter than a full header.
        assert!(!is_encrypted(&[1u8, 2, 3]));
    }

    #[test]
    fn open_document_roundtrips() {
        let path = tmp_path("open-roundtrip");
        save_document(&path, "pw", KdfParams::default(), b"secret payload").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let got = open_document(&bytes, "pw").unwrap();
        assert_eq!(got, b"secret payload");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_document_wrong_password_errs() {
        let path = tmp_path("open-wrongpw");
        save_document(&path, "pw", KdfParams::default(), b"secret payload").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(matches!(
            open_document(&bytes, "nope"),
            Err(crate::vault::VaultError::Crypto)
        ));
        std::fs::remove_file(&path).ok();
    }

    fn header_with_mcost(m_cost: u32) -> Vec<u8> {
        let mut buf = vec![VERSION];
        buf.extend_from_slice(&m_cost.to_le_bytes());
        buf.extend_from_slice(&2u32.to_le_bytes()); // t_cost (default)
        buf.extend_from_slice(&1u32.to_le_bytes()); // p_cost (default)
        buf.extend_from_slice(&[0u8; 16]); // salt
        buf.extend_from_slice(&[0u8; 12]); // nonce
        buf.extend_from_slice(&[0u8; 48]); // arbitrary "ciphertext" bytes
        buf
    }

    #[test]
    fn kdf_defaults_and_version_are_pinned() {
        // `open_document` rejects any header whose KDF params != `KdfParams::default()`, and it is
        // shared by the live-vault load path. So changing these defaults would make EVERY existing
        // vault and export stop opening. If you change them you MUST bump VERSION and add a
        // migration branch in `open_document`. This test pins the coupling so it can't drift
        // silently.
        let d = KdfParams::default();
        assert_eq!(
            (d.m_cost, d.t_cost, d.p_cost),
            (19456, 2, 1),
            "KDF defaults changed — bump VERSION and migrate in open_document"
        );
        assert_eq!(VERSION, 1, "on-disk VERSION changed — add a migration path");
    }

    #[test]
    fn open_document_rejects_nondefault_params() {
        // A header whose KDF params aren't the ones we write must be rejected as BadFormat
        // BEFORE any key derivation — otherwise a hostile import file could drive Argon2 to
        // panic (m_cost < MIN_M_COST) or attempt a multi-TiB allocation (m_cost = u32::MAX).
        let nondefault = header_with_mcost(19457); // default m_cost is 19456
        assert!(matches!(
            open_document(&nondefault, "pw"),
            Err(crate::vault::VaultError::BadFormat)
        ));
        // Regression guards for the dangerous inputs: these must NOT panic or allocate — the
        // param check rejects them before Argon2 runs. (Only safe to call once the guard exists.)
        assert!(matches!(
            open_document(&header_with_mcost(0), "pw"), // would panic in Params::new (m_cost < MIN)
            Err(crate::vault::VaultError::BadFormat)
        ));
        assert!(matches!(
            open_document(&header_with_mcost(u32::MAX), "pw"), // would attempt a ~4 TiB allocation
            Err(crate::vault::VaultError::BadFormat)
        ));
    }
}
