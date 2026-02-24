use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummaryPayload {
    pub id: String,
    pub provider_id: String,
    pub project_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub last_active_at: String,
    pub last_message_preview: Option<String>,
}


#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetClaudeThreadRuntimeStateRequest {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeThreadRuntimeStatePayload {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCodexThreadRuntimeStateRequest {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadRuntimeStatePayload {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOpenCodeThreadRuntimeStateRequest {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeThreadRuntimeStatePayload {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}



#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenThreadInTerminalRequest {
    pub thread_id: String,
    pub provider_id: String,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenNewThreadInTerminalRequest {
    pub provider_id: String,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenThreadInTerminalResponse {
    pub launched: bool,
    pub command: String,
    pub terminal_app: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartEmbeddedTerminalRequest {
    pub thread_id: String,
    pub provider_id: String,
    pub project_path: Option<String>,
    pub terminal_theme: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartNewEmbeddedTerminalRequest {
    pub provider_id: String,
    pub project_path: Option<String>,
    pub terminal_theme: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartEmbeddedTerminalResponse {
    pub session_id: String,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteEmbeddedTerminalInputRequest {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeEmbeddedTerminalRequest {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseEmbeddedTerminalRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedTerminalOutputPayload {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedTerminalExitPayload {
    pub session_id: String,
    pub status_code: Option<i32>,
}
