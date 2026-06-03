use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type ProviderResult<T> = Result<T, ProviderError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Codex,
    ClaudeCode,
    #[serde(rename = "opencode")]
    OpenCode,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Codex => "codex",
            ProviderId::ClaudeCode => "claude_code",
            ProviderId::OpenCode => "opencode",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCode {
    CredentialMissing,
    CredentialExpired,
    PermissionDenied,
    Timeout,
    UpstreamUnavailable,
    InvalidResponse,
    NotImplemented,
    Unknown,
}

#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
#[error("{code:?}: {message}")]
pub struct ProviderError {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self {
            code: ProviderErrorCode::NotImplemented,
            message: message.into(),
            retryable: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealthStatus {
    Healthy,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderHealthCheckRequest {
    pub profile_name: String,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderHealthCheckResult {
    pub provider_id: ProviderId,
    pub status: ProviderHealthStatus,
    pub checked_at: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadSummary {
    pub id: String,
    pub provider_id: ProviderId,
    pub account_id: Option<String>,
    pub project_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub last_active_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResumeThreadRequest {
    pub thread_id: String,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResumeThreadResult {
    pub thread_id: String,
    pub resumed: bool,
    pub message: Option<String>,
}

pub trait ProviderAdapter: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    fn health_check(
        &self,
        request: ProviderHealthCheckRequest,
    ) -> ProviderResult<ProviderHealthCheckResult>;
    fn list_threads(&self, project_path: Option<&str>) -> ProviderResult<Vec<ThreadSummary>>;
    fn resume_thread(&self, request: ResumeThreadRequest) -> ProviderResult<ResumeThreadResult>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockAgentTaskKind {
    Orchestration,
    ScheduledCheck,
    ChatCommand,
    RemoteCommand,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockAgentTaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockAgentScheduleCadence {
    Hourly,
    Weekly,
    Cron,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockAgentChatConnectorKind {
    Webhook,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockAgentRemoteCommandStatus {
    Requested,
    Approved,
    Running,
    Succeeded,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockAgentAuditOutcome {
    Allowed,
    Denied,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentTask {
    pub id: String,
    pub kind: DockAgentTaskKind,
    pub status: DockAgentTaskStatus,
    pub title: String,
    pub objective: String,
    pub provider_id: Option<ProviderId>,
    pub target_thread_id: Option<String>,
    pub schedule_id: Option<String>,
    pub chat_connector_id: Option<String>,
    pub remote_command_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentSchedule {
    pub id: String,
    pub task_id: Option<String>,
    pub cadence: DockAgentScheduleCadence,
    pub cron_expression: Option<String>,
    pub timezone: String,
    pub enabled: bool,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentChatConnector {
    pub id: String,
    pub name: String,
    pub kind: DockAgentChatConnectorKind,
    pub enabled: bool,
    pub config_json: String,
    pub secret_ref: Option<String>,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentChatCommandRequest {
    pub connector_id: String,
    pub external_message_id: Option<String>,
    pub actor: String,
    pub text: String,
    pub received_at: String,
    pub raw_payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentChatCommandResult {
    pub accepted: bool,
    pub task: Option<DockAgentTask>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentRemoteCommand {
    pub id: String,
    pub task_id: Option<String>,
    pub device_id: Option<String>,
    pub status: DockAgentRemoteCommandStatus,
    pub command: String,
    pub args_json: String,
    pub working_dir: Option<String>,
    pub policy_json: String,
    pub requested_by: String,
    pub approved_at: Option<String>,
    pub executed_at: Option<String>,
    pub result_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentRemoteCommandRequest {
    pub id: String,
    pub task_id: Option<String>,
    pub device_id: Option<String>,
    pub command: String,
    pub args_json: String,
    pub working_dir: Option<String>,
    pub policy_json: String,
    pub requested_by: String,
    pub requested_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentRemoteCommandDecision {
    pub accepted: bool,
    pub command: DockAgentRemoteCommand,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockAgentAuditLog {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub subject_type: String,
    pub subject_id: Option<String>,
    pub outcome: DockAgentAuditOutcome,
    pub details_json: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::{DockAgentTask, DockAgentTaskKind, DockAgentTaskStatus, ProviderId};

    #[test]
    fn dock_agent_task_serializes_snake_case_provider_fields() {
        let task = DockAgentTask {
            id: "task-1".to_string(),
            kind: DockAgentTaskKind::Orchestration,
            status: DockAgentTaskStatus::Queued,
            title: "Review stale work".to_string(),
            objective: "Inspect open worker threads.".to_string(),
            provider_id: Some(ProviderId::Codex),
            target_thread_id: Some("thread-1".to_string()),
            schedule_id: None,
            chat_connector_id: None,
            remote_command_id: None,
            created_at: "2026-06-03T00:00:00Z".to_string(),
            updated_at: "2026-06-03T00:00:00Z".to_string(),
            started_at: None,
            completed_at: None,
            metadata_json: "{}".to_string(),
        };

        let payload = serde_json::to_value(task).expect("task should serialize");

        assert_eq!(payload["kind"], "orchestration");
        assert_eq!(payload["status"], "queued");
        assert_eq!(payload["provider_id"], "codex");
        assert_eq!(payload["target_thread_id"], "thread-1");
    }
}
