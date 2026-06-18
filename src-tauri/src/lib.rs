mod commands;
mod gateway;
mod model;
mod vault;

use std::sync::Arc;

use tauri::Manager;

use commands::AppState;
use gateway::SessionRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let registry = Arc::new(SessionRegistry::new());
            let reg = registry.clone();
            // Inicia o gateway local (127.0.0.1:0) e captura a porta antes de seguir.
            let port = tauri::async_runtime::block_on(gateway::start(reg));
            app.manage(AppState {
                registry,
                gateway_port: port,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::open_session])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
