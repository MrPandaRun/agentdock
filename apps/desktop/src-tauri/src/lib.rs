mod commands;
mod path_env;
mod payloads;
mod provider_id;
mod terminal;
mod threads;

use std::fs;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    path_env::hydrate_path_from_login_shell();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_threads,
            commands::get_claude_thread_runtime_state,
            commands::get_codex_thread_runtime_state,
            commands::get_opencode_thread_runtime_state,
            commands::open_thread_in_terminal,
            commands::open_new_thread_in_terminal,
            commands::start_embedded_terminal,
            commands::start_new_embedded_terminal,
            commands::write_embedded_terminal_input,
            commands::resize_embedded_terminal,
            commands::close_embedded_terminal
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("agentdock.db");
            agentdock_core::db::init_db(&db_path)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
