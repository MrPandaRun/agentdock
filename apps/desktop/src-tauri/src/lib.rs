use provider_claude::{ClaudeAdapter, ClaudeSendMessageResult, ClaudeThreadMessage};
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeThreadSummaryPayload {
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
struct ClaudeThreadMessagePayload {
    role: String,
    content: String,
    timestamp_ms: Option<i64>,
    kind: String,
    collapsed: bool,
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

#[tauri::command]
async fn list_claude_threads(
    project_path: Option<String>,
) -> Result<Vec<ClaudeThreadSummaryPayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let adapter = ClaudeAdapter::new();
        let threads = adapter
            .list_thread_overviews(project_path.as_deref())
            .map_err(|error| {
                format!(
                    "Failed to list Claude threads ({:?}): {}",
                    error.code, error.message
                )
            })?;

        Ok(threads
            .into_iter()
            .map(|overview| ClaudeThreadSummaryPayload {
                id: overview.summary.id,
                provider_id: overview.summary.provider_id.as_str().to_string(),
                project_path: overview.summary.project_path,
                title: overview.summary.title,
                tags: overview.summary.tags,
                last_active_at: overview.summary.last_active_at,
                last_message_preview: overview.last_message_preview,
            })
            .collect::<Vec<ClaudeThreadSummaryPayload>>())
    })
    .await
    .map_err(|error| format!("Failed to scan Claude threads: {error}"))?
}

#[tauri::command]
async fn get_claude_thread_messages(
    thread_id: String,
) -> Result<Vec<ClaudeThreadMessagePayload>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let adapter = ClaudeAdapter::new();
        let messages = adapter.get_thread_messages(&thread_id).map_err(|error| {
            format!(
                "Failed to load Claude thread messages ({:?}): {}",
                error.code, error.message
            )
        })?;
        Ok(messages
            .into_iter()
            .map(|message: ClaudeThreadMessage| ClaudeThreadMessagePayload {
                role: message.role,
                content: message.content,
                timestamp_ms: message.timestamp_ms,
                kind: message.kind,
                collapsed: message.collapsed,
            })
            .collect::<Vec<ClaudeThreadMessagePayload>>())
    })
    .await
    .map_err(|error| format!("Failed to load Claude thread messages: {error}"))?
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

fn map_send_message_result(result: ClaudeSendMessageResult) -> SendClaudeMessageResponse {
    SendClaudeMessageResponse {
        thread_id: result.thread_id,
        response_text: result.response_text,
        raw_output: result.raw_output,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_claude_threads,
            get_claude_thread_messages,
            send_claude_message
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
