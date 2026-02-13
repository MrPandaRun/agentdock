use provider_contract::ProviderId;

use crate::payloads::{
    ClaudeThreadRuntimeStatePayload, CloseEmbeddedTerminalRequest, CodexThreadRuntimeStatePayload,
    GetClaudeThreadRuntimeStateRequest, GetCodexThreadRuntimeStateRequest,
    GetOpenCodeThreadRuntimeStateRequest, GetThreadMessagesRequest,
    OpenCodeThreadRuntimeStatePayload, OpenNewThreadInTerminalRequest, OpenThreadInTerminalRequest,
    OpenThreadInTerminalResponse, ResizeEmbeddedTerminalRequest, SendClaudeMessageRequest,
    SendClaudeMessageResponse, StartEmbeddedTerminalRequest, StartEmbeddedTerminalResponse,
    StartNewEmbeddedTerminalRequest, ThreadMessagePayload, ThreadSummaryPayload,
    WriteEmbeddedTerminalInputRequest,
};
use crate::provider_id::parse_provider_id;
use crate::{terminal, threads};

#[tauri::command]
pub async fn list_threads(
    project_path: Option<String>,
) -> Result<Vec<ThreadSummaryPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || threads::list_threads(project_path.as_deref()))
        .await
        .map_err(|error| format!("Failed to scan thread list: {error}"))?
}

#[tauri::command]
pub async fn get_thread_messages(
    request: GetThreadMessagesRequest,
) -> Result<Vec<ThreadMessagePayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let provider_id = parse_provider_for_message_load(&request.provider_id)?;
        threads::get_thread_messages(provider_id, &request.thread_id)
    })
    .await
    .map_err(|error| format!("Failed to load thread messages: {error}"))?
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
pub async fn send_claude_message(
    request: SendClaudeMessageRequest,
) -> Result<SendClaudeMessageResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        threads::send_claude_message(
            &request.thread_id,
            &request.content,
            request.project_path.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to send Claude message: {error}"))?
}

#[tauri::command]
pub async fn open_thread_in_terminal(
    request: OpenThreadInTerminalRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let provider_id = parse_provider_for_terminal_launch(&request.provider_id)?;
        terminal::open_thread_in_terminal(
            provider_id,
            &request.thread_id,
            request.project_path.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("Failed to open terminal session: {error}"))?
}

#[tauri::command]
pub async fn open_new_thread_in_terminal(
    request: OpenNewThreadInTerminalRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let provider_id = parse_provider_for_new_thread_launch(&request.provider_id)?;
        terminal::open_new_thread_in_terminal(provider_id, request.project_path.as_deref())
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
        let provider_id = parse_provider_for_terminal_launch(&request.provider_id)?;
        terminal::start_embedded_terminal(
            app,
            provider_id,
            &request.thread_id,
            request.project_path.as_deref(),
            request.terminal_theme.as_deref(),
            request.cols,
            request.rows,
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
        let provider_id = parse_provider_for_new_thread_launch(&request.provider_id)?;
        terminal::start_new_embedded_terminal(
            app,
            provider_id,
            request.project_path.as_deref(),
            request.terminal_theme.as_deref(),
            request.cols,
            request.rows,
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

fn parse_provider_for_message_load(raw: &str) -> Result<ProviderId, String> {
    parse_provider_id(raw).map_err(|_| format!("Unsupported provider for message load: {raw}"))
}

fn parse_provider_for_terminal_launch(raw: &str) -> Result<ProviderId, String> {
    parse_provider_id(raw).map_err(|_| format!("Unsupported provider for terminal launch: {raw}"))
}

fn parse_provider_for_new_thread_launch(raw: &str) -> Result<ProviderId, String> {
    parse_provider_id(raw).map_err(|_| format!("Unsupported provider for new thread launch: {raw}"))
}
