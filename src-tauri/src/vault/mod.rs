pub mod crypto;
pub mod file;
pub mod manager;
pub use crypto::{derive_key, open, seal, KdfParams, VaultError};
pub use file::{load_document, save_document};
pub use manager::VaultManager;
