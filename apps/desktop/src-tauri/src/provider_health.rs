use provider_claude::ClaudeAdapter;
use provider_codex::CodexAdapter;
use provider_contract::{
    ProviderAdapter, ProviderHealthCheckRequest, ProviderHealthCheckResult, ProviderHealthStatus,
};
use provider_opencode::OpenCodeAdapter;

use crate::payloads::ProviderInstallStatusPayload;

pub fn list_provider_install_statuses(
    project_path: Option<&str>,
) -> Result<Vec<ProviderInstallStatusPayload>, String> {
    let profile_name = "default".to_string();
    let project_path_owned = project_path.map(ToString::to_string);

    let codex = CodexAdapter::new()
        .health_check(ProviderHealthCheckRequest {
            profile_name: profile_name.clone(),
            project_path: project_path_owned.clone(),
        })
        .map_err(|error| {
            format!(
                "Failed to check Codex health ({:?}): {}",
                error.code, error.message
            )
        })?;
    let claude = ClaudeAdapter::new()
        .health_check(ProviderHealthCheckRequest {
            profile_name: profile_name.clone(),
            project_path: project_path_owned.clone(),
        })
        .map_err(|error| {
            format!(
                "Failed to check Claude Code health ({:?}): {}",
                error.code, error.message
            )
        })?;
    let opencode = OpenCodeAdapter::new()
        .health_check(ProviderHealthCheckRequest {
            profile_name,
            project_path: project_path_owned,
        })
        .map_err(|error| {
            format!(
                "Failed to check OpenCode health ({:?}): {}",
                error.code, error.message
            )
        })?;

    Ok(vec![
        map_provider_install_status(codex),
        map_provider_install_status(claude),
        map_provider_install_status(opencode),
    ])
}

fn map_provider_install_status(result: ProviderHealthCheckResult) -> ProviderInstallStatusPayload {
    ProviderInstallStatusPayload {
        provider_id: result.provider_id.as_str().to_string(),
        installed: !is_cli_missing(&result),
        health_status: health_status_as_str(result.status).to_string(),
        message: result.message,
    }
}

fn is_cli_missing(result: &ProviderHealthCheckResult) -> bool {
    if result.status != ProviderHealthStatus::Offline {
        return false;
    }
    result
        .message
        .as_deref()
        .map(|message| message.contains("CLI not found in PATH"))
        .unwrap_or(false)
}

fn health_status_as_str(status: ProviderHealthStatus) -> &'static str {
    match status {
        ProviderHealthStatus::Healthy => "healthy",
        ProviderHealthStatus::Degraded => "degraded",
        ProviderHealthStatus::Offline => "offline",
    }
}

#[cfg(test)]
mod tests {
    use provider_contract::{ProviderHealthCheckResult, ProviderHealthStatus, ProviderId};

    use super::{health_status_as_str, is_cli_missing};

    #[test]
    fn marks_cli_missing_when_offline_not_found_message_present() {
        let result = ProviderHealthCheckResult {
            provider_id: ProviderId::Codex,
            status: ProviderHealthStatus::Offline,
            checked_at: "0".to_string(),
            message: Some("Codex CLI not found in PATH: codex".to_string()),
        };

        assert!(is_cli_missing(&result));
    }

    #[test]
    fn does_not_mark_cli_missing_when_degraded() {
        let result = ProviderHealthCheckResult {
            provider_id: ProviderId::ClaudeCode,
            status: ProviderHealthStatus::Degraded,
            checked_at: "0".to_string(),
            message: Some("settings missing".to_string()),
        };

        assert!(!is_cli_missing(&result));
    }

    #[test]
    fn maps_health_status_to_snake_case_string() {
        assert_eq!(health_status_as_str(ProviderHealthStatus::Healthy), "healthy");
        assert_eq!(health_status_as_str(ProviderHealthStatus::Degraded), "degraded");
        assert_eq!(health_status_as_str(ProviderHealthStatus::Offline), "offline");
    }
}
