mod ccswitch;
mod command_utils;
mod commands;
mod dock_agent;
mod mcp;
mod open_targets;
mod path_env;
mod payloads;
mod provider_health;
mod provider_id;
mod skills;
mod sophon_account;
mod sophon_install;
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
            commands::get_sophon_workspace_path,
            commands::install_sophon_cli,
            commands::sync_sophon_account_settings,
            commands::list_sophon_conductor_sessions,
            commands::start_sophon_conductor_session,
            commands::import_ccswitch_suppliers,
            commands::get_claude_thread_runtime_state,
            commands::get_codex_thread_runtime_state,
            commands::get_opencode_thread_runtime_state,
            commands::get_sophon_thread_runtime_state,
            commands::open_thread_in_terminal,
            commands::open_thread_in_happy,
            commands::is_happy_installed,
            commands::list_open_targets,
            commands::open_project_with_target,
            commands::get_project_git_branch,
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
            commands::import_provider_skills,
            commands::list_mcp_servers,
            commands::list_mcp_operation_logs,
            commands::save_mcp_server,
            commands::delete_mcp_server,
            commands::toggle_mcp_server_enabled,
            commands::test_mcp_server_connection,
            commands::sync_mcp_configs,
            commands::list_dock_agent_tasks,
            commands::create_dock_agent_task,
            commands::create_dock_agent_schedule,
            commands::get_dock_agent_schedule,
            commands::list_due_dock_agent_schedules,
            commands::enqueue_due_dock_agent_schedules,
            commands::create_dock_agent_chat_connector,
            commands::get_dock_agent_chat_connector,
            commands::handle_dock_agent_chat_command,
            commands::request_dock_agent_remote_command,
            commands::get_dock_agent_remote_command,
            commands::start_dock_agent_remote_command,
            commands::complete_dock_agent_remote_command,
            commands::list_dock_agent_audit_logs,
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            if let Err(error) = sophon_install::ensure_managed_sophon_binary(&app_data_dir) {
                eprintln!("[SOPHON] Failed to prepare managed Sophon binary: {error}");
            }
            let db_path = app_data_dir.join("agentdock.db");
            agentdock_core::db::init_db(&db_path)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
