use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type ProviderResult<T> = Result<T, ProviderError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Codex,
    ClaudeCode,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Codex => "codex",
            ProviderId::ClaudeCode => "claude_code",
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
pub struct SwitchContextSummary {
    pub objective: String,
    pub constraints: Vec<String>,
    pub pending_tasks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResumeThreadRequest {
    pub thread_id: String,
    pub project_path: Option<String>,
    pub context_summary: Option<SwitchContextSummary>,
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
    fn summarize_switch_context(&self, thread_id: &str) -> ProviderResult<SwitchContextSummary>;
}
