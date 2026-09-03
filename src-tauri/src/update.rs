//! How a new version reaches an installation.
//!
//! An AppImage is a single file the running process owns; it replaces itself and restarts without
//! asking anyone for anything. A `.deb` or `.rpm` belongs to the system package manager, so
//! replacing one runs `dpkg`/`rpm` through `pkexec` and the system puts up its administrator
//! prompt.
//!
//! That difference is not an implementation detail to hide. Someone who clicks "Update" and is
//! unexpectedly asked for a root password learns to type root passwords into whatever asks — and
//! this application manages SSH keys, bastion credentials and remote desktops. Teaching that habit
//! would cost more than the stale version it fixed. So the interface says which of the two is
//! about to happen, before the click.

use serde::Serialize;

/// Where someone is sent when Remota cannot finish the job itself.
pub const RELEASES_URL: &str = "https://github.com/privum-cloud/remota/releases/latest";

/// What installing a new version will cost the person watching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Replaces itself and restarts. Nothing is asked of the user.
    SelfInstall,
    /// Can replace itself, but the system will ask for an administrator password first, because
    /// the package manager owns the files.
    NeedsAdmin,
}

/// Decide from what the running process can see.
///
/// `appimage` is the `APPIMAGE` environment variable, which the AppImage runtime sets to the path
/// of the image it is running. Nothing else sets it, so its presence is what separates an AppImage
/// from a `.deb` or `.rpm` unpacked into the same place on disk.
pub fn delivery_from(appimage: Option<&str>) -> Delivery {
    if appimage.is_some_and(|path| !path.is_empty()) {
        Delivery::SelfInstall
    } else {
        Delivery::NeedsAdmin
    }
}

/// Decide for the process running right now.
pub fn delivery() -> Delivery {
    delivery_from(std::env::var("APPIMAGE").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_appimage_is_one_file_the_process_already_owns() {
        assert_eq!(
            delivery_from(Some("/home/someone/Apps/Remota.AppImage")),
            Delivery::SelfInstall
        );
    }

    #[test]
    fn a_package_managed_install_warns_that_a_password_is_coming() {
        assert_eq!(delivery_from(None), Delivery::NeedsAdmin);
    }

    #[test]
    fn an_empty_appimage_variable_is_not_an_appimage() {
        // An exported-but-empty variable is a shell leaving something behind, not a runtime
        // announcing itself. Believing it would promise a quiet update and then produce a
        // password prompt anyway.
        assert_eq!(delivery_from(Some("")), Delivery::NeedsAdmin);
    }

    #[test]
    fn every_updater_endpoint_is_https() {
        // The plugin refuses a plain-http endpoint, and it refuses it during initialisation —
        // which takes the whole application down with it. The window never opens, so the vault
        // never opens, so one wrong line of configuration costs somebody every connection they
        // own.
        //
        // Nothing else catches this. No test builds a window, CI never launches one, and the
        // configuration is compiled into the binary — so reading the file here is reading exactly
        // what ships.
        let config = shipped_config();
        let endpoints = config["plugins"]["updater"]["endpoints"]
            .as_array()
            .expect("the updater needs somewhere to ask");
        assert!(!endpoints.is_empty(), "an updater with no endpoint asks nobody");

        for endpoint in endpoints {
            let url = endpoint.as_str().expect("endpoints are strings");
            assert!(
                url.starts_with("https://"),
                "the updater refuses this at startup and the app will not open: {url}"
            );
        }
    }

    #[test]
    fn the_configured_public_key_is_the_remota_key() {
        // Pasting another project's key here would ship an app that rejects every update we sign,
        // and the failure would only appear on someone else's machine.
        let config = shipped_config();
        let pubkey = config["plugins"]["updater"]["pubkey"]
            .as_str()
            .expect("the updater needs a public key");
        let decoded =
            String::from_utf8(base64_decode(pubkey).expect("the pubkey is base64")).expect("text");
        assert!(
            decoded.contains("3FFC41078BBA8B43"),
            "this is not Remota's signing key: {decoded}"
        );
    }

    /// The configuration as it will be compiled into the binary.
    fn shipped_config() -> serde_json::Value {
        let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"))
            .expect("tauri.conf.json sits beside Cargo.toml");
        serde_json::from_str(&raw).expect("valid JSON")
    }

    /// Minimal base64 decode, so the test needs no new dependency.
    fn base64_decode(s: &str) -> Option<Vec<u8>> {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut buf = 0u32;
        let mut bits = 0u32;
        for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
            let v = TABLE.iter().position(|t| *t == c)? as u32;
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
        Some(out)
    }
}
