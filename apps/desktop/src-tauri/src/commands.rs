use provider_contract::ProviderId;
use tauri::Emitter;

use crate::payloads::{
    AddSkillRepoRequest, CcSwitchImportPayload, ClaudeThreadRuntimeStatePayload,
    CloseEmbeddedTerminalRequest, CodexThreadRuntimeStatePayload,
    DiscoverSkillInstallProgressPayload,
    GetClaudeThreadRuntimeStateRequest, GetCodexThreadRuntimeStateRequest,
    GetOpenCodeThreadRuntimeStateRequest, InstallDiscoveredSkillRequest,
    InstallSkillFromGitRequest, InstallSkillFromPathRequest, OpenCodeThreadRuntimeStatePayload,
    OpenNewThreadInTerminalRequest, OpenThreadInHappyRequest, OpenThreadInTerminalRequest,
    OpenThreadInTerminalResponse, ProviderInstallStatusPayload, RemoveSkillRepoRequest,
    ResizeEmbeddedTerminalRequest, SkillPayload, SkillRepoPayload, StartEmbeddedTerminalRequest,
    StartEmbeddedTerminalResponse, StartNewEmbeddedTerminalRequest, ThreadSummaryPayload,
    ToggleSkillEnabledForProviderRequest, ToggleSkillEnabledRequest, UninstallSkillRequest,
    WriteEmbeddedTerminalInputRequest,
};
use crate::provider_id::parse_provider_id;
use crate::skills::{DiscoverableSkill, SkillsContext};
use crate::{
    ccswitch, payloads::ImportProviderSkillsRequest, payloads::ProviderSkillPayload, provider_health, skills,
    terminal, threads,
};

#[tauri::command]
pub async fn list_threads(
    project_path: Option<String>,
) -> Result<Vec<ThreadSummaryPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || threads::list_threads(project_path.as_deref()))
        .await
        .map_err(|error| format!("Failed to scan thread list: {error}"))?
}

#[tauri::command]
pub async fn list_provider_install_statuses(
    project_path: Option<String>,
) -> Result<Vec<ProviderInstallStatusPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        provider_health::list_provider_install_statuses(project_path.as_deref())
    })
    .await
    .map_err(|error| format!("Failed to load provider install statuses: {error}"))?
}

#[tauri::command]
pub async fn import_ccswitch_suppliers() -> Result<CcSwitchImportPayload, String> {
    tauri::async_runtime::spawn_blocking(ccswitch::import_suppliers_from_ccswitch)
        .await
        .map_err(|error| format!("Failed to import CC Switch suppliers: {error}"))?
}

#[tauri::command]
pub async fn get_codex_thread_runtime_state(
    request: GetCodexThreadRuntimeStateRequest,
) -> Result<CodexThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        threads::get_codex_thread_runtime_state(&request.thread_id)
    })
    .await
    .map_err(|error| format!("Failed to load Codex runtime state: {error}"))?
}

#[tauri::command]
pub async fn get_claude_thread_runtime_state(
    request: GetClaudeThreadRuntimeStateRequest,
) -> Result<ClaudeThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        threads::get_claude_thread_runtime_state(&request.thread_id)
    })
    .await
    .map_err(|error| format!("Failed to load Claude runtime state: {error}"))?
}

#[tauri::command]
pub async fn get_opencode_thread_runtime_state(
    request: GetOpenCodeThreadRuntimeStateRequest,
) -> Result<OpenCodeThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        threads::get_opencode_thread_runtime_state(&request.thread_id)
    })
    .await
    .map_err(|error| format!("Failed to load OpenCode runtime state: {error}"))?
}

#[tauri::command]
pub async fn open_thread_in_terminal(
    request: OpenThreadInTerminalRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let OpenThreadInTerminalRequest {
            thread_id,
            provider_id,
            profile_name,
            env,
            project_path,
        } = request;
        let provider_id = parse_provider_for_terminal_launch(&provider_id)?;
        terminal::open_thread_in_terminal(
            provider_id,
            &thread_id,
            profile_name.as_deref(),
            env,
            project_path.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to open terminal session: {error}"))?
}

#[tauri::command]
pub async fn open_thread_in_happy(
    request: OpenThreadInHappyRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let provider_id = parse_provider_for_happy_launch(&request.provider_id)?;
        terminal::open_thread_in_happy(
            provider_id,
            request.thread_id.as_deref(),
            request.project_path.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to open Happy integration: {error}"))?
}

#[tauri::command]
pub async fn is_happy_installed() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(terminal::is_happy_installed)
        .await
        .map_err(|error| format!("Failed to check Happy installation: {error}"))?
}

#[tauri::command]
pub async fn open_new_thread_in_terminal(
    request: OpenNewThreadInTerminalRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let OpenNewThreadInTerminalRequest {
            provider_id,
            profile_name,
            env,
            project_path,
        } = request;
        let provider_id = parse_provider_for_new_thread_launch(&provider_id)?;
        terminal::open_new_thread_in_terminal(
            provider_id,
            profile_name.as_deref(),
            env,
            project_path.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to open new thread terminal session: {error}"))?
}

#[tauri::command]
pub async fn start_embedded_terminal(
    app: tauri::AppHandle,
    request: StartEmbeddedTerminalRequest,
) -> Result<StartEmbeddedTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let StartEmbeddedTerminalRequest {
            thread_id,
            provider_id,
            profile_name,
            env,
            project_path,
            terminal_theme,
            cols,
            rows,
        } = request;
        let provider_id = parse_provider_for_terminal_launch(&provider_id)?;
        terminal::start_embedded_terminal(
            app,
            provider_id,
            &thread_id,
            profile_name.as_deref(),
            env,
            project_path.as_deref(),
            terminal_theme.as_deref(),
            cols,
            rows,
        )
    })
    .await
    .map_err(|error| format!("Failed to start embedded terminal: {error}"))?
}

#[tauri::command]
pub async fn start_new_embedded_terminal(
    app: tauri::AppHandle,
    request: StartNewEmbeddedTerminalRequest,
) -> Result<StartEmbeddedTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let StartNewEmbeddedTerminalRequest {
            provider_id,
            profile_name,
            env,
            project_path,
            terminal_theme,
            cols,
            rows,
        } = request;
        let provider_id = parse_provider_for_new_thread_launch(&provider_id)?;
        terminal::start_new_embedded_terminal(
            app,
            provider_id,
            profile_name.as_deref(),
            env,
            project_path.as_deref(),
            terminal_theme.as_deref(),
            cols,
            rows,
        )
    })
    .await
    .map_err(|error| format!("Failed to start new embedded terminal: {error}"))?
}

#[tauri::command]
pub async fn write_embedded_terminal_input(
    request: WriteEmbeddedTerminalInputRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        terminal::write_embedded_terminal_input(&request.session_id, &request.data)
    })
    .await
    .map_err(|error| format!("Failed to write embedded terminal input: {error}"))?
}

#[tauri::command]
pub async fn resize_embedded_terminal(
    request: ResizeEmbeddedTerminalRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        terminal::resize_embedded_terminal(&request.session_id, request.cols, request.rows)
    })
    .await
    .map_err(|error| format!("Failed to resize embedded terminal: {error}"))?
}

#[tauri::command]
pub async fn close_embedded_terminal(request: CloseEmbeddedTerminalRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        terminal::close_embedded_terminal(&request.session_id)
    })
    .await
    .map_err(|error| format!("Failed to close embedded terminal: {error}"))?
}

fn parse_provider_for_terminal_launch(raw: &str) -> Result<ProviderId, String> {
    parse_provider_id(raw).map_err(|_| format!("Unsupported provider for terminal launch: {raw}"))
}

fn parse_provider_for_new_thread_launch(raw: &str) -> Result<ProviderId, String> {
    parse_provider_id(raw).map_err(|_| format!("Unsupported provider for new thread launch: {raw}"))
}

fn parse_provider_for_happy_launch(raw: &str) -> Result<ProviderId, String> {
    let provider_id = parse_provider_id(raw)
        .map_err(|_| format!("Unsupported provider for Happy integration: {raw}"))?;
    match provider_id {
        ProviderId::ClaudeCode | ProviderId::Codex => Ok(provider_id),
        ProviderId::OpenCode => {
            Err("Happy integration currently supports claude_code and codex only".to_string())
        }
    }
}

#[tauri::command]
pub async fn list_skills(app: tauri::AppHandle) -> Result<Vec<SkillPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::list_skills_cmd(&ctx)
            .map(|skills| skills.into_iter().map(SkillPayload::from).collect())
    })
    .await
    .map_err(|error| format!("Failed to list skills: {error}"))?
}

#[tauri::command]
pub async fn install_skill_from_path(
    app: tauri::AppHandle,
    request: InstallSkillFromPathRequest,
) -> Result<SkillPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::install_skill_from_path_cmd(&ctx, &request.path).map(SkillPayload::from)
    })
    .await
    .map_err(|error| format!("Failed to install skill from path: {error}"))?
}

#[tauri::command]
pub async fn install_skill_from_git(
    app: tauri::AppHandle,
    request: InstallSkillFromGitRequest,
) -> Result<SkillPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::install_skill_from_git_cmd(&ctx, &request.url).map(SkillPayload::from)
    })
    .await
    .map_err(|error| format!("Failed to install skill from git: {error}"))?
}

#[tauri::command]
pub async fn install_discovered_skill(
    app: tauri::AppHandle,
    request: InstallDiscoveredSkillRequest,
) -> Result<SkillPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        let skill: crate::skills::DiscoverableSkill = request.skill.into();
        let mut emit_progress = |stage: &str, message: &str| {
            let payload = DiscoverSkillInstallProgressPayload {
                key: skill.key.clone(),
                stage: stage.to_string(),
                message: message.to_string(),
            };
            let _ = app.emit("discover-skill-install-progress", payload);
        };

        emit_progress("queued", "Queued for installation...");

        let result = skills::install_discovered_skill_cmd(&ctx, &skill, &mut emit_progress);
        match result {
            Ok(installed) => {
                emit_progress("completed", "Installed successfully");
                Ok(SkillPayload::from(installed))
            }
            Err(error) => {
                emit_progress("failed", &error);
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| format!("Failed to install discovered skill: {error}"))?
}

#[tauri::command]
pub async fn toggle_skill_enabled(
    app: tauri::AppHandle,
    request: ToggleSkillEnabledRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::toggle_skill_enabled_cmd(&ctx, &request.id, request.enabled)
    })
    .await
    .map_err(|error| format!("Failed to toggle skill: {error}"))?
}

#[tauri::command]
pub async fn uninstall_skill(
    app: tauri::AppHandle,
    request: UninstallSkillRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::uninstall_skill_cmd(&ctx, &request.id)
    })
    .await
    .map_err(|error| format!("Failed to uninstall skill: {error}"))?
}

#[tauri::command]
pub async fn toggle_skill_enabled_for_provider(
    app: tauri::AppHandle,
    request: ToggleSkillEnabledForProviderRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::toggle_skill_enabled_for_provider_cmd(&ctx, &request.id, &request.provider, request.enabled)
    })
    .await
    .map_err(|error| format!("Failed to toggle skill for provider: {error}"))?
}

#[tauri::command]
pub async fn list_skill_repos(app: tauri::AppHandle) -> Result<Vec<SkillRepoPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::list_skill_repos_cmd(&ctx)
            .map(|repos| repos.into_iter().map(SkillRepoPayload::from).collect())
    })
    .await
    .map_err(|error| format!("Failed to list skill repos: {error}"))?
}

#[tauri::command]
pub async fn add_skill_repo(
    app: tauri::AppHandle,
    request: AddSkillRepoRequest,
) -> Result<SkillRepoPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        let branch = request.branch.as_deref().unwrap_or("main");
        skills::add_skill_repo_cmd(&ctx, &request.owner, &request.name, branch)
            .map(SkillRepoPayload::from)
    })
    .await
    .map_err(|error| format!("Failed to add skill repo: {error}"))?
}

#[tauri::command]
pub async fn remove_skill_repo(
    app: tauri::AppHandle,
    request: RemoveSkillRepoRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::remove_skill_repo_cmd(&ctx, &request.id)
    })
    .await
    .map_err(|error| format!("Failed to remove skill repo: {error}"))?
}

#[tauri::command]
pub async fn discover_skills(app: tauri::AppHandle, force_refresh: Option<bool>) -> Result<Vec<DiscoverableSkill>, String> {
    let force = force_refresh.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::discover_skills_cmd_with_cache(&ctx, force)
    })
    .await
    .map_err(|error| format!("Failed to discover skills: {error}"))?
}

#[tauri::command]
pub async fn scan_provider_skills(app: tauri::AppHandle) -> Result<Vec<ProviderSkillPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        let skills = skills::scan_provider_skills_cmd(&ctx)?;
        Ok::<_, String>(skills.into_iter().map(ProviderSkillPayload::from).collect())
    })
    .await
    .map_err(|error| format!("Failed to scan provider skills: {error}"))?
}

#[tauri::command]
pub async fn import_provider_skills(
    app: tauri::AppHandle,
    request: ImportProviderSkillsRequest,
) -> Result<Vec<SkillPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        let skills = skills::import_provider_skills_cmd(&ctx, request.skill_keys)?;
        Ok(skills.into_iter().map(SkillPayload::from).collect())
    })
    .await
    .map_err(|error| format!("Failed to import provider skills: {error}"))?
}
