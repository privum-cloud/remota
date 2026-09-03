//! Preferences that are not secrets and not connections.
//!
//! The vault is encrypted and holds credentials; this file holds neither, so it stays plain JSON
//! next to it. It exists because the update check is the first network request Remota makes
//! without being asked, and a request like that needs an off switch the user can find.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Whether Remota may ask GitHub if a newer release exists.
    pub check_for_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { check_for_updates: true }
    }
}

/// `~/.config/remota/settings.json`, beside the vault.
pub fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("remota")
        .join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from(&path())
    }

    /// A missing or unreadable file is a fresh install; a corrupt one is a file we cannot trust.
    /// Both yield the defaults. Refusing to start because a preferences file is malformed would
    /// cost someone every connection they own over a setting that does not matter.
    pub fn load_from(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("remota-settings-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn a_fresh_install_checks_for_updates() {
        assert!(Settings::default().check_for_updates);
    }

    #[test]
    fn a_missing_file_is_a_fresh_install() {
        let p = temp("missing");
        assert_eq!(Settings::load_from(&p), Settings::default());
    }

    #[test]
    fn a_corrupt_file_falls_back_instead_of_failing() {
        // Refusing to start over a malformed preferences file would cost someone every
        // connection they own for the sake of one boolean.
        let p = temp("corrupt");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"{ this is not json").unwrap();
        assert_eq!(Settings::load_from(&p), Settings::default());
    }

    #[test]
    fn an_unknown_field_does_not_discard_the_known_ones() {
        // A settings file written by a newer Remota must still load in an older one.
        let p = temp("unknown");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, br#"{"check_for_updates": false, "future_option": 7}"#).unwrap();
        assert!(!Settings::load_from(&p).check_for_updates);
    }

    #[test]
    fn saving_then_loading_round_trips() {
        let p = temp("roundtrip");
        let s = Settings { check_for_updates: false };
        s.save_to(&p).unwrap();
        assert_eq!(Settings::load_from(&p), s);
    }
}
