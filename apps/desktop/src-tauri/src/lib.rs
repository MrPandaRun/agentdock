mod ccswitch;
mod commands;
mod path_env;
mod payloads;
mod provider_health;
mod provider_id;
mod skills;
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
            commands::list_provider_install_statuses,
            commands::import_ccswitch_suppliers,
            commands::get_claude_thread_runtime_state,
            commands::get_codex_thread_runtime_state,
            commands::get_opencode_thread_runtime_state,
            commands::open_thread_in_terminal,
            commands::open_thread_in_happy,
            commands::is_happy_installed,
            commands::open_new_thread_in_terminal,
            commands::start_embedded_terminal,
            commands::start_new_embedded_terminal,
            commands::write_embedded_terminal_input,
            commands::resize_embedded_terminal,
            commands::close_embedded_terminal,
            commands::list_skills,
            commands::install_skill_from_path,
            commands::install_skill_from_git,
            commands::install_discovered_skill,
            commands::toggle_skill_enabled,
            commands::toggle_skill_enabled_for_provider,
            commands::uninstall_skill,
            commands::list_skill_repos,
            commands::add_skill_repo,
            commands::remove_skill_repo,
            commands::discover_skills,
            commands::scan_provider_skills,
            commands::import_provider_skills
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
