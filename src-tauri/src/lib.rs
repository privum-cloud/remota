mod commands;
mod gateway;
mod importer;
mod model;
mod settings;
mod update;
mod vault;

use std::sync::Arc;

use tauri::Manager;

use commands::AppState;
use gateway::SessionRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init());

    // Desktop only: a phone has no package to replace.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            // CryptoProvider (ring) para o rustls 0.23 — usado pelas conexões "relayed" (wss).
            gateway::relay::ensure_crypto_provider();
            let registry = Arc::new(SessionRegistry::new());
            let reg = registry.clone();
            // Inicia o gateway local (127.0.0.1:0) e captura a porta antes de seguir.
            let port = tauri::async_runtime::block_on(gateway::start(reg));
            // Cofre cifrado em ~/.config/remota/connections.dat (XDG).
            let vault_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("remota")
                .join("connections.dat");
            app.manage(AppState {
                registry,
                gateway_port: port,
                vault: crate::vault::VaultManager::new(vault_path),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_session,
            commands::unlock_vault,
            commands::lock_vault,
            commands::vault_exists,
            commands::list_tree,
            commands::save_connection,
            commands::delete_node,
            commands::restore_node,
            commands::delete_forever,
            commands::empty_trash,
            commands::import_mremoteng,
            commands::export_connections,
            commands::import_remota_json,
            commands::update_policy,
            commands::set_update_check
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
