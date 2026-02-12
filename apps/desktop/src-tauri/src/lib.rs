use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use provider_claude::{
    ClaudeAdapter, ClaudeSendMessageResult, ClaudeThreadMessage, ClaudeThreadOverview,
    ClaudeThreadRuntimeState,
};
use provider_codex::{
    CodexAdapter, CodexThreadMessage, CodexThreadOverview, CodexThreadRuntimeState,
};
use provider_opencode::{
    OpenCodeAdapter, OpenCodeThreadMessage, OpenCodeThreadOverview, OpenCodeThreadRuntimeState,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

const SHELL_PATH_SENTINEL_START: &str = "__AGENTDOCK_PATH_START__";
const SHELL_PATH_SENTINEL_END: &str = "__AGENTDOCK_PATH_END__";
const SHELL_PATH_PROBE_COMMAND: &str =
    "printf '__AGENTDOCK_PATH_START__%s__AGENTDOCK_PATH_END__' \"$PATH\"";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadSummaryPayload {
    id: String,
    provider_id: String,
    project_path: String,
    title: String,
    tags: Vec<String>,
    last_active_at: String,
    last_message_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadMessagePayload {
    role: String,
    content: String,
    timestamp_ms: Option<i64>,
    kind: String,
    collapsed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetThreadMessagesRequest {
    thread_id: String,
    provider_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetClaudeThreadRuntimeStateRequest {
    thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeThreadRuntimeStatePayload {
    agent_answering: bool,
    last_event_kind: Option<String>,
    last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetCodexThreadRuntimeStateRequest {
    thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexThreadRuntimeStatePayload {
    agent_answering: bool,
    last_event_kind: Option<String>,
    last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetOpenCodeThreadRuntimeStateRequest {
    thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenCodeThreadRuntimeStatePayload {
    agent_answering: bool,
    last_event_kind: Option<String>,
    last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendClaudeMessageRequest {
    thread_id: String,
    content: String,
    project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendClaudeMessageResponse {
    thread_id: String,
    response_text: String,
    raw_output: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenThreadInTerminalRequest {
    thread_id: String,
    provider_id: String,
    project_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenNewThreadInTerminalRequest {
    provider_id: String,
    project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenThreadInTerminalResponse {
    launched: bool,
    command: String,
    terminal_app: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartEmbeddedTerminalRequest {
    thread_id: String,
    provider_id: String,
    project_path: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartNewEmbeddedTerminalRequest {
    provider_id: String,
    project_path: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartEmbeddedTerminalResponse {
    session_id: String,
    command: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteEmbeddedTerminalInputRequest {
    session_id: String,
    data: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResizeEmbeddedTerminalRequest {
    session_id: String,
    cols: u16,
    rows: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseEmbeddedTerminalRequest {
    session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddedTerminalOutputPayload {
    session_id: String,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddedTerminalExitPayload {
    session_id: String,
    status_code: Option<i32>,
}

struct EmbeddedTerminalSession {
    child: Mutex<Box<dyn portable_pty::Child + Send>>,
    stdin: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
}

static EMBEDDED_TERMINAL_SESSIONS: OnceLock<Mutex<HashMap<String, Arc<EmbeddedTerminalSession>>>> =
    OnceLock::new();
static EMBEDDED_TERMINAL_COUNTER: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
async fn list_threads(project_path: Option<String>) -> Result<Vec<ThreadSummaryPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let claude_threads = ClaudeAdapter::new()
            .list_thread_overviews(project_path.as_deref())
            .map_err(|error| {
                format!(
                    "Failed to list Claude threads ({:?}): {}",
                    error.code, error.message
                )
            })?;
        let codex_threads = CodexAdapter::new()
            .list_thread_overviews(project_path.as_deref())
            .map_err(|error| {
                format!(
                    "Failed to list Codex threads ({:?}): {}",
                    error.code, error.message
                )
            })?;
        let opencode_threads = OpenCodeAdapter::new()
            .list_thread_overviews(project_path.as_deref())
            .map_err(|error| {
                format!(
                    "Failed to list OpenCode threads ({:?}): {}",
                    error.code, error.message
                )
            })?;

        let mut threads =
            Vec::with_capacity(claude_threads.len() + codex_threads.len() + opencode_threads.len());
        threads.extend(claude_threads.into_iter().map(map_claude_thread_overview));
        threads.extend(codex_threads.into_iter().map(map_codex_thread_overview));
        threads.extend(
            opencode_threads
                .into_iter()
                .map(map_opencode_thread_overview),
        );
        threads.sort_by_key(|thread| {
            std::cmp::Reverse(sortable_last_active_at(&thread.last_active_at))
        });

        Ok(threads)
    })
    .await
    .map_err(|error| format!("Failed to scan thread list: {error}"))?
}

#[tauri::command]
async fn get_thread_messages(
    request: GetThreadMessagesRequest,
) -> Result<Vec<ThreadMessagePayload>, String> {
    tauri::async_runtime::spawn_blocking(move || match request.provider_id.as_str() {
        "claude_code" => {
            let messages = ClaudeAdapter::new()
                .get_thread_messages(&request.thread_id)
                .map_err(|error| {
                    format!(
                        "Failed to load Claude thread messages ({:?}): {}",
                        error.code, error.message
                    )
                })?;
            Ok(messages
                .into_iter()
                .map(map_claude_thread_message)
                .collect::<Vec<ThreadMessagePayload>>())
        }
        "codex" => {
            let messages = CodexAdapter::new()
                .get_thread_messages(&request.thread_id)
                .map_err(|error| {
                    format!(
                        "Failed to load Codex thread messages ({:?}): {}",
                        error.code, error.message
                    )
                })?;
            Ok(messages
                .into_iter()
                .map(map_codex_thread_message)
                .collect::<Vec<ThreadMessagePayload>>())
        }
        "opencode" => {
            let messages = OpenCodeAdapter::new()
                .get_thread_messages(&request.thread_id)
                .map_err(|error| {
                    format!(
                        "Failed to load OpenCode thread messages ({:?}): {}",
                        error.code, error.message
                    )
                })?;
            Ok(messages
                .into_iter()
                .map(map_opencode_thread_message)
                .collect::<Vec<ThreadMessagePayload>>())
        }
        unsupported => Err(format!(
            "Unsupported provider for message load: {unsupported}"
        )),
    })
    .await
    .map_err(|error| format!("Failed to load thread messages: {error}"))?
}

#[tauri::command]
async fn get_codex_thread_runtime_state(
    request: GetCodexThreadRuntimeStateRequest,
) -> Result<CodexThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = CodexAdapter::new()
            .get_thread_runtime_state(&request.thread_id)
            .map_err(|error| {
                format!(
                    "Failed to load Codex runtime state ({:?}): {}",
                    error.code, error.message
                )
            })?;
        Ok(map_codex_thread_runtime_state(state))
    })
    .await
    .map_err(|error| format!("Failed to load Codex runtime state: {error}"))?
}

#[tauri::command]
async fn get_claude_thread_runtime_state(
    request: GetClaudeThreadRuntimeStateRequest,
) -> Result<ClaudeThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = ClaudeAdapter::new()
            .get_thread_runtime_state(&request.thread_id)
            .map_err(|error| {
                format!(
                    "Failed to load Claude runtime state ({:?}): {}",
                    error.code, error.message
                )
            })?;
        Ok(map_claude_thread_runtime_state(state))
    })
    .await
    .map_err(|error| format!("Failed to load Claude runtime state: {error}"))?
}

#[tauri::command]
async fn get_opencode_thread_runtime_state(
    request: GetOpenCodeThreadRuntimeStateRequest,
) -> Result<OpenCodeThreadRuntimeStatePayload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = OpenCodeAdapter::new()
            .get_thread_runtime_state(&request.thread_id)
            .map_err(|error| {
                format!(
                    "Failed to load OpenCode runtime state ({:?}): {}",
                    error.code, error.message
                )
            })?;
        Ok(map_opencode_thread_runtime_state(state))
    })
    .await
    .map_err(|error| format!("Failed to load OpenCode runtime state: {error}"))?
}

#[tauri::command]
async fn send_claude_message(
    request: SendClaudeMessageRequest,
) -> Result<SendClaudeMessageResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let adapter = ClaudeAdapter::new();
        let result = adapter
            .send_message(
                &request.thread_id,
                &request.content,
                request.project_path.as_deref(),
            )
            .map_err(|error| {
                format!(
                    "Failed to send message to Claude thread ({:?}): {}",
                    error.code, error.message
                )
            })?;
        Ok(map_send_message_result(result))
    })
    .await
    .map_err(|error| format!("Failed to send Claude message: {error}"))?
}

#[tauri::command]
async fn open_thread_in_terminal(
    request: OpenThreadInTerminalRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let command = build_resume_command(&request)?;
        launch_in_terminal(&command)?;
        Ok(OpenThreadInTerminalResponse {
            launched: true,
            command,
            terminal_app: "Terminal".to_string(),
        })
    })
    .await
    .map_err(|error| format!("Failed to open terminal session: {error}"))?
}

#[tauri::command]
async fn open_new_thread_in_terminal(
    request: OpenNewThreadInTerminalRequest,
) -> Result<OpenThreadInTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let command = build_new_thread_command_from_parts(
            &request.provider_id,
            request.project_path.as_deref(),
        )?;
        launch_in_terminal(&command)?;
        Ok(OpenThreadInTerminalResponse {
            launched: true,
            command,
            terminal_app: "Terminal".to_string(),
        })
    })
    .await
    .map_err(|error| format!("Failed to open new thread terminal session: {error}"))?
}

#[tauri::command]
async fn start_embedded_terminal(
    app: tauri::AppHandle,
    request: StartEmbeddedTerminalRequest,
) -> Result<StartEmbeddedTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cols = clamp_terminal_cols(request.cols);
        let rows = clamp_terminal_rows(request.rows);
        let command = build_resume_command_from_parts(
            &request.provider_id,
            &request.thread_id,
            request.project_path.as_deref(),
        )?;
        let session_id = next_embedded_terminal_session_id();
        let (reader, session) = create_embedded_session(&command, cols, rows)?;
        terminal_sessions()
            .lock()
            .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?
            .insert(session_id.clone(), Arc::clone(&session));

        spawn_terminal_output_reader(app.clone(), session_id.clone(), reader);
        spawn_terminal_exit_watcher(app, session_id.clone(), session);

        Ok(StartEmbeddedTerminalResponse {
            session_id,
            command,
        })
    })
    .await
    .map_err(|error| format!("Failed to start embedded terminal: {error}"))?
}

#[tauri::command]
async fn start_new_embedded_terminal(
    app: tauri::AppHandle,
    request: StartNewEmbeddedTerminalRequest,
) -> Result<StartEmbeddedTerminalResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cols = clamp_terminal_cols(request.cols);
        let rows = clamp_terminal_rows(request.rows);
        let command = build_new_thread_command_from_parts(
            &request.provider_id,
            request.project_path.as_deref(),
        )?;
        let session_id = next_embedded_terminal_session_id();
        let (reader, session) = create_embedded_session(&command, cols, rows)?;
        terminal_sessions()
            .lock()
            .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?
            .insert(session_id.clone(), Arc::clone(&session));

        spawn_terminal_output_reader(app.clone(), session_id.clone(), reader);
        spawn_terminal_exit_watcher(app, session_id.clone(), session);

        Ok(StartEmbeddedTerminalResponse {
            session_id,
            command,
        })
    })
    .await
    .map_err(|error| format!("Failed to start new embedded terminal: {error}"))?
}

#[tauri::command]
async fn write_embedded_terminal_input(
    request: WriteEmbeddedTerminalInputRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = {
            let sessions = terminal_sessions()
                .lock()
                .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?;
            sessions.get(&request.session_id).cloned().ok_or_else(|| {
                format!(
                    "Embedded terminal session not found: {}",
                    request.session_id
                )
            })?
        };

        let mut stdin = session
            .stdin
            .lock()
            .map_err(|_| "Embedded terminal stdin lock poisoned".to_string())?;
        stdin
            .write_all(request.data.as_bytes())
            .map_err(|error| format!("Failed to write terminal input: {error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("Failed to flush terminal input: {error}"))?;
        Ok(())
    })
    .await
    .map_err(|error| format!("Failed to write embedded terminal input: {error}"))?
}

#[tauri::command]
async fn resize_embedded_terminal(request: ResizeEmbeddedTerminalRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = {
            let sessions = terminal_sessions()
                .lock()
                .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?;
            sessions.get(&request.session_id).cloned().ok_or_else(|| {
                format!(
                    "Embedded terminal session not found: {}",
                    request.session_id
                )
            })?
        };

        let cols = clamp_terminal_cols(Some(request.cols));
        let rows = clamp_terminal_rows(Some(request.rows));
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let master = session
            .master
            .lock()
            .map_err(|_| "Embedded terminal master lock poisoned".to_string())?;
        master
            .resize(size)
            .map_err(|error| format!("Failed to resize embedded terminal: {error}"))
    })
    .await
    .map_err(|error| format!("Failed to resize embedded terminal: {error}"))?
}

#[tauri::command]
async fn close_embedded_terminal(request: CloseEmbeddedTerminalRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = remove_embedded_terminal_session(&request.session_id);
        if let Some(session) = session {
            let mut child = session
                .child
                .lock()
                .map_err(|_| "Embedded terminal child lock poisoned".to_string())?;
            let _ = child.kill();
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("Failed to close embedded terminal: {error}"))?
}

fn map_send_message_result(result: ClaudeSendMessageResult) -> SendClaudeMessageResponse {
    SendClaudeMessageResponse {
        thread_id: result.thread_id,
        response_text: result.response_text,
        raw_output: result.raw_output,
    }
}

fn clamp_terminal_cols(value: Option<u16>) -> u16 {
    match value.unwrap_or(120) {
        0..=39 => 120,
        cols => cols.min(320),
    }
}

fn clamp_terminal_rows(value: Option<u16>) -> u16 {
    match value.unwrap_or(36) {
        0..=11 => 36,
        rows => rows.min(120),
    }
}

fn map_claude_thread_overview(overview: ClaudeThreadOverview) -> ThreadSummaryPayload {
    ThreadSummaryPayload {
        id: overview.summary.id,
        provider_id: overview.summary.provider_id.as_str().to_string(),
        project_path: overview.summary.project_path,
        title: overview.summary.title,
        tags: overview.summary.tags,
        last_active_at: overview.summary.last_active_at,
        last_message_preview: overview.last_message_preview,
    }
}

fn map_codex_thread_overview(overview: CodexThreadOverview) -> ThreadSummaryPayload {
    ThreadSummaryPayload {
        id: overview.summary.id,
        provider_id: overview.summary.provider_id.as_str().to_string(),
        project_path: overview.summary.project_path,
        title: overview.summary.title,
        tags: overview.summary.tags,
        last_active_at: overview.summary.last_active_at,
        last_message_preview: overview.last_message_preview,
    }
}

fn map_opencode_thread_overview(overview: OpenCodeThreadOverview) -> ThreadSummaryPayload {
    ThreadSummaryPayload {
        id: overview.summary.id,
        provider_id: overview.summary.provider_id.as_str().to_string(),
        project_path: overview.summary.project_path,
        title: overview.summary.title,
        tags: overview.summary.tags,
        last_active_at: overview.summary.last_active_at,
        last_message_preview: overview.last_message_preview,
    }
}

fn map_claude_thread_message(message: ClaudeThreadMessage) -> ThreadMessagePayload {
    ThreadMessagePayload {
        role: message.role,
        content: message.content,
        timestamp_ms: message.timestamp_ms,
        kind: message.kind,
        collapsed: message.collapsed,
    }
}

fn map_codex_thread_message(message: CodexThreadMessage) -> ThreadMessagePayload {
    ThreadMessagePayload {
        role: message.role,
        content: message.content,
        timestamp_ms: message.timestamp_ms,
        kind: message.kind,
        collapsed: message.collapsed,
    }
}

fn map_opencode_thread_message(message: OpenCodeThreadMessage) -> ThreadMessagePayload {
    ThreadMessagePayload {
        role: message.role,
        content: message.content,
        timestamp_ms: message.timestamp_ms,
        kind: message.kind,
        collapsed: message.collapsed,
    }
}

fn map_codex_thread_runtime_state(
    state: CodexThreadRuntimeState,
) -> CodexThreadRuntimeStatePayload {
    CodexThreadRuntimeStatePayload {
        agent_answering: state.agent_answering,
        last_event_kind: state.last_event_kind,
        last_event_at_ms: state.last_event_at_ms,
    }
}

fn map_opencode_thread_runtime_state(
    state: OpenCodeThreadRuntimeState,
) -> OpenCodeThreadRuntimeStatePayload {
    OpenCodeThreadRuntimeStatePayload {
        agent_answering: state.agent_answering,
        last_event_kind: state.last_event_kind,
        last_event_at_ms: state.last_event_at_ms,
    }
}

fn map_claude_thread_runtime_state(
    state: ClaudeThreadRuntimeState,
) -> ClaudeThreadRuntimeStatePayload {
    ClaudeThreadRuntimeStatePayload {
        agent_answering: state.agent_answering,
        last_event_kind: state.last_event_kind,
        last_event_at_ms: state.last_event_at_ms,
    }
}

fn sortable_last_active_at(raw: &str) -> i64 {
    let parsed = raw.parse::<i64>().unwrap_or(0);
    if parsed.abs() < 1_000_000_000_000 {
        parsed * 1000
    } else {
        parsed
    }
}

fn terminal_sessions() -> &'static Mutex<HashMap<String, Arc<EmbeddedTerminalSession>>> {
    EMBEDDED_TERMINAL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_embedded_terminal_session_id() -> String {
    let value = EMBEDDED_TERMINAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("embedded-terminal-{value}")
}

fn create_embedded_session(
    command: &str,
    cols: u16,
    rows: u16,
) -> Result<(Box<dyn Read + Send>, Arc<EmbeddedTerminalSession>), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to allocate PTY: {error}"))?;

    let mut cmd = CommandBuilder::new("sh");
    cmd.arg("-lc");
    cmd.arg(command);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("COLUMNS", cols.to_string());
    cmd.env("LINES", rows.to_string());

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|error| format!("Failed to spawn PTY child process: {error}"))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("Failed to clone PTY reader: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("Failed to capture PTY writer: {error}"))?;

    let session = Arc::new(EmbeddedTerminalSession {
        child: Mutex::new(child),
        stdin: Mutex::new(writer),
        master: Mutex::new(pair.master),
    });
    Ok((reader, session))
}

fn spawn_terminal_output_reader<R: Read + Send + 'static>(
    app: tauri::AppHandle,
    session_id: String,
    mut stream: R,
) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        let mut pending = Vec::new();
        loop {
            let read = match stream.read(&mut buffer) {
                Ok(size) => size,
                Err(_) => break,
            };
            if read == 0 {
                if !pending.is_empty() {
                    let data = String::from_utf8_lossy(&pending).to_string();
                    if !data.is_empty() {
                        let payload = EmbeddedTerminalOutputPayload {
                            session_id: session_id.clone(),
                            data,
                        };
                        let _ = app.emit("embedded-terminal-output", payload);
                    }
                    pending.clear();
                }
                break;
            }

            pending.extend_from_slice(&buffer[..read]);

            loop {
                match std::str::from_utf8(&pending) {
                    Ok(text) => {
                        if !text.is_empty() {
                            let payload = EmbeddedTerminalOutputPayload {
                                session_id: session_id.clone(),
                                data: text.to_string(),
                            };
                            let _ = app.emit("embedded-terminal-output", payload);
                        }
                        pending.clear();
                        break;
                    }
                    Err(error) => {
                        let valid_up_to = error.valid_up_to();
                        if valid_up_to > 0 {
                            let valid = &pending[..valid_up_to];
                            let payload = EmbeddedTerminalOutputPayload {
                                session_id: session_id.clone(),
                                data: String::from_utf8_lossy(valid).to_string(),
                            };
                            let _ = app.emit("embedded-terminal-output", payload);
                        }

                        match error.error_len() {
                            Some(error_len) => {
                                // True invalid bytes: skip the offending sequence and continue.
                                let drain_to = valid_up_to + error_len;
                                pending.drain(..drain_to);
                                let payload = EmbeddedTerminalOutputPayload {
                                    session_id: session_id.clone(),
                                    data: "\u{FFFD}".to_string(),
                                };
                                let _ = app.emit("embedded-terminal-output", payload);
                                if pending.is_empty() {
                                    break;
                                }
                            }
                            None => {
                                // Incomplete UTF-8 sequence at the end; keep remainder for next read.
                                pending.drain(..valid_up_to);
                                break;
                            }
                        }
                    }
                }
            }
        }
    });
}

fn spawn_terminal_exit_watcher(
    app: tauri::AppHandle,
    session_id: String,
    session: Arc<EmbeddedTerminalSession>,
) {
    thread::spawn(move || {
        enum PollStatus {
            Running,
            Exited(Option<i32>),
            Failed,
        }

        let status_code = loop {
            let poll = {
                let mut child = match session.child.lock() {
                    Ok(child) => child,
                    Err(_) => break None,
                };

                match child.try_wait() {
                    Ok(Some(status)) => PollStatus::Exited(Some(status.exit_code() as i32)),
                    Ok(None) => PollStatus::Running,
                    Err(_) => PollStatus::Failed,
                }
            };

            match poll {
                PollStatus::Exited(code) => break code,
                PollStatus::Failed => break None,
                PollStatus::Running => thread::sleep(Duration::from_millis(80)),
            }
        };

        remove_embedded_terminal_session(&session_id);
        let payload = EmbeddedTerminalExitPayload {
            session_id,
            status_code,
        };
        let _ = app.emit("embedded-terminal-exit", payload);
    });
}

fn remove_embedded_terminal_session(session_id: &str) -> Option<Arc<EmbeddedTerminalSession>> {
    terminal_sessions()
        .lock()
        .ok()
        .and_then(|mut sessions| sessions.remove(session_id))
}

fn build_resume_command(request: &OpenThreadInTerminalRequest) -> Result<String, String> {
    build_resume_command_from_parts(
        &request.provider_id,
        &request.thread_id,
        request.project_path.as_deref(),
    )
}

fn build_resume_command_from_parts(
    provider_id: &str,
    thread_id: &str,
    project_path: Option<&str>,
) -> Result<String, String> {
    let resume = match provider_id {
        "claude_code" => format!("claude --resume {}", shell_quote(thread_id)),
        "codex" => format!("codex resume {}", shell_quote(thread_id)),
        "opencode" => format!("opencode --session {}", shell_quote(thread_id)),
        _ => {
            return Err(format!(
                "Unsupported provider for terminal launch: {}",
                provider_id
            ));
        }
    };

    let project_path = project_path
        .map(str::trim)
        .filter(|path| !path.is_empty() && *path != ".");

    if let Some(path) = project_path {
        return Ok(format!("cd {} && {resume}", shell_quote(path)));
    }

    Ok(resume)
}

fn build_new_thread_command_from_parts(
    provider_id: &str,
    project_path: Option<&str>,
) -> Result<String, String> {
    let start = match provider_id {
        "claude_code" => "claude".to_string(),
        "codex" => "codex".to_string(),
        "opencode" => "opencode".to_string(),
        _ => {
            return Err(format!(
                "Unsupported provider for new thread launch: {}",
                provider_id
            ));
        }
    };

    let project_path = project_path
        .map(str::trim)
        .filter(|path| !path.is_empty() && *path != ".");

    if let Some(path) = project_path {
        return Ok(format!("cd {} && {start}", shell_quote(path)));
    }

    Ok(start)
}

#[cfg(target_os = "macos")]
fn launch_in_terminal(command: &str) -> Result<(), String> {
    let escaped = escape_applescript(command);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "tell application \"Terminal\" to do script \"{escaped}\""
        ))
        .arg("-e")
        .arg("tell application \"Terminal\" to activate")
        .output()
        .map_err(|error| format!("Failed to invoke osascript: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!("Failed to launch Terminal with command: {detail}"))
}

#[cfg(not(target_os = "macos"))]
fn launch_in_terminal(_command: &str) -> Result<(), String> {
    Err("Terminal launch is only supported on macOS for now".to_string())
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "macos")]
fn hydrate_path_from_login_shell() {
    let mut shells = Vec::new();

    if let Ok(shell) = std::env::var("SHELL") {
        let trimmed = shell.trim();
        if !trimmed.is_empty() {
            shells.push(trimmed.to_string());
        }
    }

    shells.push("/bin/zsh".to_string());
    shells.push("/bin/bash".to_string());

    for shell in shells {
        if let Some(path) = read_login_shell_path(&shell) {
            std::env::set_var("PATH", path);
            break;
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn hydrate_path_from_login_shell() {}

#[cfg(target_os = "macos")]
fn read_login_shell_path(shell: &str) -> Option<String> {
    let output = Command::new(shell)
        .arg("-ilc")
        .arg(SHELL_PATH_PROBE_COMMAND)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    extract_path_from_shell_output(&output.stdout)
}

fn extract_path_from_shell_output(stdout: &[u8]) -> Option<String> {
    let raw = String::from_utf8_lossy(stdout);
    let start = raw.find(SHELL_PATH_SENTINEL_START)? + SHELL_PATH_SENTINEL_START.len();
    let rest = &raw[start..];
    let end = rest.find(SHELL_PATH_SENTINEL_END)?;
    let path = rest[..end].trim();
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    hydrate_path_from_login_shell();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_threads,
            get_thread_messages,
            get_claude_thread_runtime_state,
            get_codex_thread_runtime_state,
            get_opencode_thread_runtime_state,
            send_claude_message,
            open_thread_in_terminal,
            open_new_thread_in_terminal,
            start_embedded_terminal,
            start_new_embedded_terminal,
            write_embedded_terminal_input,
            resize_embedded_terminal,
            close_embedded_terminal
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

#[cfg(test)]
mod tests {
    use super::extract_path_from_shell_output;

    #[test]
    fn extract_path_from_shell_output_reads_marker_payload() {
        let output = b"noise\n__AGENTDOCK_PATH_START__/usr/local/bin:/opt/homebrew/bin__AGENTDOCK_PATH_END__\n";
        let path = extract_path_from_shell_output(output).expect("path should parse");
        assert_eq!(path, "/usr/local/bin:/opt/homebrew/bin");
    }

    #[test]
    fn extract_path_from_shell_output_returns_none_without_markers() {
        let output = b"/usr/bin:/bin\n";
        assert!(extract_path_from_shell_output(output).is_none());
    }
}
