pub mod crypto;
pub mod file;
pub use crypto::{derive_key, open, seal, KdfParams, VaultError};
