use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInstallStatusPayload {
    pub provider_id: String,
    pub installed: bool,
    pub health_status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchImportedSupplierPayload {
    pub provider_id: String,
    pub source_id: String,
    pub name: String,
    pub note: Option<String>,
    pub profile_name: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub config_json: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchImportPayload {
    pub db_path: String,
    pub suppliers: Vec<CcSwitchImportedSupplierPayload>,
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
    pub profile_name: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenThreadInHappyRequest {
    pub provider_id: String,
    pub thread_id: Option<String>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTargetStatusPayload {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub available: bool,
    pub detail: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectWithTargetRequest {
    pub project_path: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectWithTargetResponse {
    pub launched: bool,
    pub target_id: String,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectGitBranchRequest {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGitBranchPayload {
    pub status: String,
    pub branch: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenNewThreadInTerminalRequest {
    pub provider_id: String,
    pub profile_name: Option<String>,
    pub env: Option<HashMap<String, String>>,
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
    pub profile_name: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub project_path: Option<String>,
    pub terminal_theme: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartNewEmbeddedTerminalRequest {
    pub provider_id: String,
    pub profile_name: Option<String>,
    pub env: Option<HashMap<String, String>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPayload {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub version: String,
    pub enabled_json: String,
    pub compatibility_json: String,
    pub readme_url: Option<String>,
    pub repo_owner: Option<String>,
    pub repo_name: Option<String>,
    pub repo_branch: Option<String>,
    pub installed_at: i64,
}

impl From<agentdock_core::skills::Skill> for SkillPayload {
    fn from(skill: agentdock_core::skills::Skill) -> Self {
        SkillPayload {
            id: skill.id,
            name: skill.name,
            description: skill.description,
            source: skill.source,
            version: skill.version,
            enabled_json: skill.enabled_json,
            compatibility_json: skill.compatibility_json,
            readme_url: skill.readme_url,
            repo_owner: skill.repo_owner,
            repo_name: skill.repo_name,
            repo_branch: skill.repo_branch,
            installed_at: skill.installed_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRepoPayload {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub branch: String,
    pub enabled: bool,
    pub created_at: i64,
}

impl From<agentdock_core::skills::SkillRepo> for SkillRepoPayload {
    fn from(repo: agentdock_core::skills::SkillRepo) -> Self {
        SkillRepoPayload {
            id: repo.id,
            owner: repo.owner,
            name: repo.name,
            branch: repo.branch,
            enabled: repo.enabled,
            created_at: repo.created_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSkillFromPathRequest {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSkillFromGitRequest {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleSkillEnabledRequest {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleSkillEnabledForProviderRequest {
    pub id: String,
    pub provider: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallSkillRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSkillRepoRequest {
    pub owner: String,
    pub name: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSkillRepoRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallDiscoveredSkillRequest {
    pub skill: DiscoverableSkillPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverableSkillPayload {
    pub key: String,
    pub name: String,
    pub description: String,
    pub directory: String,
    pub readme_url: Option<String>,
    pub repo_owner: String,
    pub repo_name: String,
    pub repo_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverSkillInstallProgressPayload {
    pub key: String,
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSkillPayload {
    pub key: String,
    pub name: String,
    pub description: String,
    pub directory: String,
    pub provider: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProviderSkillsRequest {
    pub skill_keys: Vec<String>,
}

impl From<crate::skills::ProviderSkill> for ProviderSkillPayload {
    fn from(skill: crate::skills::ProviderSkill) -> Self {
        Self {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            directory: skill.directory,
            provider: skill.provider,
            path: skill.path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerPayload {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub target: String,
    pub args_json: String,
    pub headers_json: String,
    pub env_json: String,
    pub scope_providers: Vec<String>,
    pub enabled: bool,
    pub version: String,
    pub created_at: String,
    pub updated_at: String,
    pub has_secret: bool,
    pub secret_header_name: Option<String>,
    pub last_tested_at: Option<String>,
    pub last_test_status: Option<String>,
    pub last_test_message: Option<String>,
    pub last_test_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpFieldErrorPayload {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMcpServerRequest {
    pub id: Option<String>,
    pub name: String,
    pub transport: String,
    pub target: String,
    pub args_json: Option<String>,
    pub headers_json: Option<String>,
    pub env_json: Option<String>,
    pub scope_providers: Option<Vec<String>>,
    pub enabled: bool,
    pub version: Option<String>,
    pub secret_header_name: Option<String>,
    pub secret_token: Option<String>,
    pub clear_secret: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMcpServerResponsePayload {
    pub server: Option<McpServerPayload>,
    pub field_errors: Vec<McpFieldErrorPayload>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMcpServerRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleMcpServerEnabledRequest {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestMcpConnectionRequest {
    pub id: Option<String>,
    pub transport: String,
    pub target: String,
    pub args_json: Option<String>,
    pub headers_json: Option<String>,
    pub env_json: Option<String>,
    pub secret_header_name: Option<String>,
    pub secret_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionTestResultPayload {
    pub success: bool,
    pub error_summary: Option<String>,
    pub duration_ms: i64,
    pub checked_at: String,
    pub field_errors: Vec<McpFieldErrorPayload>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMcpConfigsRequest {
    pub provider_ids: Option<Vec<String>>,
    pub simulate_failure_provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMcpProviderResultPayload {
    pub provider_id: String,
    pub success: bool,
    pub message: Option<String>,
    pub backup_path: Option<String>,
    pub server_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMcpConfigsResponsePayload {
    pub success: bool,
    pub rolled_back: bool,
    pub message: Option<String>,
    pub results: Vec<SyncMcpProviderResultPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOperationLogPayload {
    pub id: i64,
    pub mcp_id: Option<String>,
    pub action: String,
    pub actor: String,
    pub details_json: String,
    pub created_at: String,
}
