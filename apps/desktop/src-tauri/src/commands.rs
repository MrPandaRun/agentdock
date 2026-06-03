use provider_contract::ProviderId;
use provider_sophon::SophonAdapter;
use tauri::Emitter;

use crate::dock_agent::{self, DockAgentContext};
use crate::payloads::{
    AddSkillRepoRequest, CcSwitchImportPayload, ClaudeThreadRuntimeStatePayload,
    CloseEmbeddedTerminalRequest, CodexThreadRuntimeStatePayload,
    CompleteDockAgentRemoteCommandRequest, DeleteMcpServerRequest,
    DiscoverSkillInstallProgressPayload, DockAgentAuditLogPayload,
    DockAgentChatCommandRequestPayload, DockAgentChatCommandResultPayload,
    DockAgentChatConnectorPayload, DockAgentDueScheduleEnqueuePayload,
    DockAgentRemoteCommandDecisionPayload, DockAgentRemoteCommandPayload,
    DockAgentRemoteCommandRequestPayload, DockAgentSchedulePayload, DockAgentTaskPayload,
    EnqueueDueDockAgentSchedulesRequest, GetClaudeThreadRuntimeStateRequest,
    GetCodexThreadRuntimeStateRequest, GetDockAgentEntityRequest,
    GetOpenCodeThreadRuntimeStateRequest, GetProjectGitBranchRequest,
    GetSophonThreadRuntimeStateRequest, InstallDiscoveredSkillRequest, InstallSkillFromGitRequest,
    InstallSkillFromPathRequest, InstallSophonCliPayload, ListDockAgentAuditLogsRequest,
    ListDockAgentTasksRequest, McpConnectionTestResultPayload, McpOperationLogPayload,
    McpServerPayload, OpenCodeThreadRuntimeStatePayload, OpenNewThreadInTerminalRequest,
    OpenProjectWithTargetRequest, OpenProjectWithTargetResponse, OpenTargetStatusPayload,
    OpenThreadInHappyRequest, OpenThreadInTerminalRequest, OpenThreadInTerminalResponse,
    ProjectGitBranchPayload, ProviderInstallStatusPayload, RemoveSkillRepoRequest,
    ResizeEmbeddedTerminalRequest, SaveMcpServerRequest, SaveMcpServerResponsePayload,
    SkillPayload, SkillRepoPayload, SophonConductorSessionPayload, SophonThreadRuntimeStatePayload,
    StartDockAgentRemoteCommandRequest, StartEmbeddedTerminalRequest,
    StartEmbeddedTerminalResponse, StartNewEmbeddedTerminalRequest,
    StartSophonConductorSessionRequest, SyncMcpConfigsRequest, SyncMcpConfigsResponsePayload,
    SyncSophonAccountSettingsPayload, SyncSophonAccountSettingsRequest, TestMcpConnectionRequest,
    ThreadSummaryPayload, ToggleMcpServerEnabledRequest, ToggleSkillEnabledForProviderRequest,
    ToggleSkillEnabledRequest, UninstallSkillRequest, WriteEmbeddedTerminalInputRequest,
};
use crate::provider_id::parse_provider_id;
use crate::skills::{DiscoverableSkill, SkillsContext};
use crate::sophon_account;
use crate::sophon_install;
use crate::{
    ccswitch, mcp, open_targets, payloads::ImportProviderSkillsRequest,
    payloads::ProviderSkillPayload, provider_health, skills, terminal, threads,
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
pub fn get_sophon_workspace_path() -> Result<String, String> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| "Failed to resolve home directory".to_string())?;
    Ok(home_dir
        .join(".sophon")
        .join("workspace")
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub async fn install_sophon_cli(app: tauri::AppHandle) -> Result<InstallSophonCliPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        sophon_install::install_sophon_cli_cmd(&app).map(|response| InstallSophonCliPayload {
            installed: response.installed,
            binary_path: response.binary_path,
            message: response.message,
        })
    })
    .await
    .map_err(|error| format!("Failed to install Sophon CLI: {error}"))?
}

#[tauri::command]
pub async fn sync_sophon_account_settings(
    request: SyncSophonAccountSettingsRequest,
) -> Result<SyncSophonAccountSettingsPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        sophon_account::sync_sophon_account_settings(request)
    })
    .await
    .map_err(|error| format!("Failed to sync Sophon account settings: {error}"))?
}

#[tauri::command]
pub async fn list_dock_agent_tasks(
    app: tauri::AppHandle,
    request: ListDockAgentTasksRequest,
) -> Result<Vec<DockAgentTaskPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::list_tasks_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to list Dock Agent tasks: {error}"))?
}

#[tauri::command]
pub async fn create_dock_agent_task(
    app: tauri::AppHandle,
    request: DockAgentTaskPayload,
) -> Result<DockAgentTaskPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::create_task_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to create Dock Agent task: {error}"))?
}

#[tauri::command]
pub async fn create_dock_agent_schedule(
    app: tauri::AppHandle,
    request: DockAgentSchedulePayload,
) -> Result<DockAgentSchedulePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::create_schedule_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to create Dock Agent schedule: {error}"))?
}

#[tauri::command]
pub async fn get_dock_agent_schedule(
    app: tauri::AppHandle,
    request: GetDockAgentEntityRequest,
) -> Result<DockAgentSchedulePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::get_schedule_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to get Dock Agent schedule: {error}"))?
}

#[tauri::command]
pub async fn list_due_dock_agent_schedules(
    app: tauri::AppHandle,
    now: String,
) -> Result<Vec<DockAgentSchedulePayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::list_due_schedules_cmd(&ctx, now)
    })
    .await
    .map_err(|error| format!("Failed to list due Dock Agent schedules: {error}"))?
}

#[tauri::command]
pub async fn enqueue_due_dock_agent_schedules(
    app: tauri::AppHandle,
    request: EnqueueDueDockAgentSchedulesRequest,
) -> Result<Vec<DockAgentDueScheduleEnqueuePayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::enqueue_due_schedules_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to enqueue due Dock Agent schedules: {error}"))?
}

#[tauri::command]
pub async fn create_dock_agent_chat_connector(
    app: tauri::AppHandle,
    request: DockAgentChatConnectorPayload,
) -> Result<DockAgentChatConnectorPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::create_chat_connector_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to create Dock Agent chat connector: {error}"))?
}

#[tauri::command]
pub async fn get_dock_agent_chat_connector(
    app: tauri::AppHandle,
    request: GetDockAgentEntityRequest,
) -> Result<DockAgentChatConnectorPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::get_chat_connector_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to get Dock Agent chat connector: {error}"))?
}

#[tauri::command]
pub async fn handle_dock_agent_chat_command(
    app: tauri::AppHandle,
    request: DockAgentChatCommandRequestPayload,
) -> Result<DockAgentChatCommandResultPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::handle_chat_command_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to handle Dock Agent chat command: {error}"))?
}

#[tauri::command]
pub async fn request_dock_agent_remote_command(
    app: tauri::AppHandle,
    request: DockAgentRemoteCommandRequestPayload,
) -> Result<DockAgentRemoteCommandDecisionPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::request_remote_command_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to request Dock Agent remote command: {error}"))?
}

#[tauri::command]
pub async fn get_dock_agent_remote_command(
    app: tauri::AppHandle,
    request: GetDockAgentEntityRequest,
) -> Result<DockAgentRemoteCommandPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::get_remote_command_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to get Dock Agent remote command: {error}"))?
}

#[tauri::command]
pub async fn start_dock_agent_remote_command(
    app: tauri::AppHandle,
    request: StartDockAgentRemoteCommandRequest,
) -> Result<DockAgentRemoteCommandPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::start_remote_command_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to start Dock Agent remote command: {error}"))?
}

#[tauri::command]
pub async fn complete_dock_agent_remote_command(
    app: tauri::AppHandle,
    request: CompleteDockAgentRemoteCommandRequest,
) -> Result<DockAgentRemoteCommandPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::complete_remote_command_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to complete Dock Agent remote command: {error}"))?
}

#[tauri::command]
pub async fn list_dock_agent_audit_logs(
    app: tauri::AppHandle,
    request: ListDockAgentAuditLogsRequest,
) -> Result<Vec<DockAgentAuditLogPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = DockAgentContext::from_app_handle(&app)?;
        dock_agent::list_audit_logs_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to list Dock Agent audit logs: {error}"))?
}

#[tauri::command]
pub async fn list_sophon_conductor_sessions() -> Result<Vec<SophonConductorSessionPayload>, String>
{
    tauri::async_runtime::spawn_blocking(move || {
        SophonAdapter::new()
            .list_conductor_sessions()
            .map(|sessions| {
                sessions
                    .into_iter()
                    .map(|session| SophonConductorSessionPayload {
                        id: session.id,
                        title: session.title,
                        workspace_path: session.workspace_path,
                        status: session.status,
                        created_at: session.created_at,
                        last_active_at: session.last_active_at,
                        worker_agents: session.worker_agents,
                        linked_thread_keys: session.linked_thread_keys,
                    })
                    .collect()
            })
            .map_err(|error| {
                format!(
                    "Failed to list Sophon conductor sessions ({:?}): {}",
                    error.code, error.message
                )
            })
    })
    .await
    .map_err(|error| format!("Failed to list Sophon conductor sessions: {error}"))?
}

#[tauri::command]
pub async fn start_sophon_conductor_session(
    request: StartSophonConductorSessionRequest,
) -> Result<SophonConductorSessionPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        SophonAdapter::new()
            .start_conductor_session(&request.workspace_path)
            .map(|session| SophonConductorSessionPayload {
                id: session.id,
                title: session.title,
                workspace_path: session.workspace_path,
                status: session.status,
                created_at: session.created_at,
                last_active_at: session.last_active_at,
                worker_agents: session.worker_agents,
                linked_thread_keys: session.linked_thread_keys,
            })
            .map_err(|error| {
                format!(
                    "Failed to start Sophon conductor session ({:?}): {}",
                    error.code, error.message
                )
            })
    })
    .await
    .map_err(|error| format!("Failed to start Sophon conductor session: {error}"))?
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
pub async fn get_sophon_thread_runtime_state(
    request: GetSophonThreadRuntimeStateRequest,
) -> Result<SophonThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        threads::get_sophon_thread_runtime_state(&request.thread_id)
    })
    .await
    .map_err(|error| format!("Failed to load Sophon runtime state: {error}"))?
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
pub async fn list_open_targets() -> Result<Vec<OpenTargetStatusPayload>, String> {
    tauri::async_runtime::spawn_blocking(open_targets::list_open_targets)
        .await
        .map_err(|error| format!("Failed to list open targets: {error}"))?
}

#[tauri::command]
pub async fn open_project_with_target(
    request: OpenProjectWithTargetRequest,
) -> Result<OpenProjectWithTargetResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        open_targets::open_project_with_target(&request.project_path, &request.target_id)
    })
    .await
    .map_err(|error| format!("Failed to open project with target: {error}"))?
}

#[tauri::command]
pub async fn get_project_git_branch(
    request: GetProjectGitBranchRequest,
) -> Result<ProjectGitBranchPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        open_targets::get_project_git_branch(&request.project_path)
    })
    .await
    .map_err(|error| format!("Failed to get project git branch: {error}"))?
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
        skills::toggle_skill_enabled_for_provider_cmd(
            &ctx,
            &request.id,
            &request.provider,
            request.enabled,
        )
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
pub async fn discover_skills(
    app: tauri::AppHandle,
    force_refresh: Option<bool>,
) -> Result<Vec<DiscoverableSkill>, String> {
    let force = force_refresh.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = SkillsContext::from_app_handle(&app)?;
        skills::discover_skills_cmd_with_cache(&ctx, force)
    })
    .await
    .map_err(|error| format!("Failed to discover skills: {error}"))?
}

#[tauri::command]
pub async fn scan_provider_skills(
    app: tauri::AppHandle,
) -> Result<Vec<ProviderSkillPayload>, String> {
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

#[tauri::command]
pub async fn list_mcp_servers(app: tauri::AppHandle) -> Result<Vec<McpServerPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::list_mcp_servers_cmd(&ctx)
    })
    .await
    .map_err(|error| format!("Failed to list MCP servers: {error}"))?
}

#[tauri::command]
pub async fn list_mcp_operation_logs(
    app: tauri::AppHandle,
    limit: Option<u32>,
) -> Result<Vec<McpOperationLogPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::list_mcp_operation_logs_cmd(&ctx, limit)
    })
    .await
    .map_err(|error| format!("Failed to list MCP operation logs: {error}"))?
}

#[tauri::command]
pub async fn save_mcp_server(
    app: tauri::AppHandle,
    request: SaveMcpServerRequest,
) -> Result<SaveMcpServerResponsePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::save_mcp_server_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to save MCP server: {error}"))?
}

#[tauri::command]
pub async fn delete_mcp_server(
    app: tauri::AppHandle,
    request: DeleteMcpServerRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::delete_mcp_server_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to delete MCP server: {error}"))?
}

#[tauri::command]
pub async fn toggle_mcp_server_enabled(
    app: tauri::AppHandle,
    request: ToggleMcpServerEnabledRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::toggle_mcp_server_enabled_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to toggle MCP server status: {error}"))?
}

#[tauri::command]
pub async fn test_mcp_server_connection(
    app: tauri::AppHandle,
    request: TestMcpConnectionRequest,
) -> Result<McpConnectionTestResultPayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::test_mcp_server_connection_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to test MCP connection: {error}"))?
}

#[tauri::command]
pub async fn sync_mcp_configs(
    app: tauri::AppHandle,
    request: SyncMcpConfigsRequest,
) -> Result<SyncMcpConfigsResponsePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = mcp::McpContext::from_app_handle(&app)?;
        mcp::sync_mcp_configs_cmd(&ctx, request)
    })
    .await
    .map_err(|error| format!("Failed to sync MCP configs: {error}"))?
}
