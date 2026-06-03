use provider_contract::{
    ProviderError, ProviderErrorCode, ProviderHealthCheckRequest, ProviderHealthStatus,
    ProviderResult, ResumeThreadRequest, ResumeThreadResult,
};
use serde::Deserialize;
use std::process::Command;

const SOPHON_BINARY_ENV: &str = "AGENTDOCK_SOPHON_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SophonThreadOverview {
    pub summary: SophonThreadSummary,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SophonThreadSummary {
    pub id: String,
    pub provider_id: String,
    pub account_id: Option<String>,
    pub project_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub last_active_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SophonHealthCheckResult {
    pub provider_id: String,
    pub status: ProviderHealthStatus,
    pub checked_at: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SophonThreadRuntimeState {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SophonConductorSession {
    pub id: String,
    pub title: String,
    pub workspace_path: String,
    pub status: String,
    pub created_at: String,
    pub last_active_at: String,
    pub worker_agents: Vec<String>,
    pub linked_thread_keys: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SophonAdapter {
    cli_binary_override: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliHealthPayload {
    status: String,
    checked_at: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliThreadSummaryPayload {
    id: String,
    project_path: String,
    title: String,
    tags: Vec<String>,
    last_active_at: String,
    last_message_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliRuntimeStatePayload {
    agent_answering: bool,
    last_event_kind: Option<String>,
    last_event_at_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliResumePayload {
    thread_id: String,
    resumed: bool,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliConductorSessionPayload {
    id: String,
    title: String,
    workspace_path: String,
    status: String,
    created_at: String,
    last_active_at: String,
    worker_agents: Vec<String>,
    linked_thread_keys: Vec<String>,
}

impl SophonAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cli_binary<S: Into<String>>(mut self, cli_binary: S) -> Self {
        self.cli_binary_override = Some(cli_binary.into());
        self
    }

    pub fn list_thread_overviews(
        &self,
        project_path: Option<&str>,
    ) -> ProviderResult<Vec<SophonThreadOverview>> {
        let mut args = vec!["threads", "list", "--json"];
        if let Some(project_path) = project_path {
            args.push("--project-path");
            args.push(project_path);
        }

        let threads: Vec<CliThreadSummaryPayload> = self.exec_json(&args)?;
        Ok(threads
            .into_iter()
            .map(|thread| SophonThreadOverview {
                last_message_preview: thread.last_message_preview,
                summary: SophonThreadSummary {
                    id: thread.id,
                    provider_id: "sophon".to_string(),
                    account_id: None,
                    project_path: thread.project_path,
                    title: thread.title,
                    tags: thread.tags,
                    last_active_at: thread.last_active_at,
                },
            })
            .collect())
    }

    pub fn get_thread_runtime_state(
        &self,
        thread_id: &str,
    ) -> ProviderResult<SophonThreadRuntimeState> {
        let runtime: CliRuntimeStatePayload =
            self.exec_json(&["threads", "runtime", "--thread-id", thread_id, "--json"])?;
        Ok(SophonThreadRuntimeState {
            agent_answering: runtime.agent_answering,
            last_event_kind: runtime.last_event_kind,
            last_event_at_ms: runtime.last_event_at_ms,
        })
    }

    pub fn list_conductor_sessions(&self) -> ProviderResult<Vec<SophonConductorSession>> {
        let sessions: Vec<CliConductorSessionPayload> =
            self.exec_json(&["conductor", "sessions", "list", "--json"])?;
        Ok(sessions
            .into_iter()
            .map(map_conductor_session_payload)
            .collect())
    }

    pub fn start_conductor_session(
        &self,
        workspace_path: &str,
    ) -> ProviderResult<SophonConductorSession> {
        let session: CliConductorSessionPayload = self.exec_json(&[
            "conductor",
            "sessions",
            "start",
            "--workspace",
            workspace_path,
            "--json",
        ])?;
        Ok(map_conductor_session_payload(session))
    }

    fn sophon_binary(&self) -> String {
        if let Some(binary) = &self.cli_binary_override {
            return binary.clone();
        }
        if let Ok(binary) = std::env::var(SOPHON_BINARY_ENV) {
            let trimmed = binary.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        "sophon".to_string()
    }

    fn ensure_cli_reachable(&self) -> ProviderResult<()> {
        let binary = self.sophon_binary();
        match Command::new(&binary).arg("--version").output() {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!(
                    "Sophon CLI version check failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
                false,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Sophon CLI not found in PATH: {binary}"),
                false,
            )),
            Err(error) => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Failed to execute Sophon CLI ({binary}): {error}"),
                true,
            )),
        }
    }

    fn exec_json<T: for<'de> Deserialize<'de>>(&self, args: &[&str]) -> ProviderResult<T> {
        self.ensure_cli_reachable()?;
        let binary = self.sophon_binary();
        let output = Command::new(&binary).args(args).output().map_err(|error| {
            provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Failed to execute Sophon CLI ({binary}): {error}"),
                true,
            )
        })?;

        if !output.status.success() {
            return Err(provider_error(
                ProviderErrorCode::InvalidResponse,
                format!(
                    "Sophon CLI command failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
                false,
            ));
        }

        serde_json::from_slice::<T>(&output.stdout).map_err(|error| {
            provider_error(
                ProviderErrorCode::InvalidResponse,
                format!("Failed to parse Sophon CLI JSON output: {error}"),
                false,
            )
        })
    }
    pub fn health_check(
        &self,
        _request: ProviderHealthCheckRequest,
    ) -> ProviderResult<SophonHealthCheckResult> {
        let payload: CliHealthPayload = self.exec_json(&["health", "--json"])?;
        Ok(SophonHealthCheckResult {
            provider_id: "sophon".to_string(),
            status: match payload.status.as_str() {
                "healthy" => ProviderHealthStatus::Healthy,
                "degraded" => ProviderHealthStatus::Degraded,
                _ => ProviderHealthStatus::Offline,
            },
            checked_at: payload.checked_at,
            message: payload.message,
        })
    }

    pub fn list_threads(
        &self,
        project_path: Option<&str>,
    ) -> ProviderResult<Vec<SophonThreadSummary>> {
        Ok(self
            .list_thread_overviews(project_path)?
            .into_iter()
            .map(|overview| overview.summary)
            .collect())
    }

    pub fn resume_thread(
        &self,
        request: ResumeThreadRequest,
    ) -> ProviderResult<ResumeThreadResult> {
        let payload: CliResumePayload =
            self.exec_json(&["threads", "resume", &request.thread_id, "--json"])?;
        Ok(ResumeThreadResult {
            thread_id: payload.thread_id,
            resumed: payload.resumed,
            message: payload.message,
        })
    }
}

fn provider_error(
    code: ProviderErrorCode,
    message: impl Into<String>,
    retryable: bool,
) -> ProviderError {
    ProviderError {
        code,
        message: message.into(),
        retryable,
    }
}

fn map_conductor_session_payload(payload: CliConductorSessionPayload) -> SophonConductorSession {
    SophonConductorSession {
        id: payload.id,
        title: payload.title,
        workspace_path: payload.workspace_path,
        status: payload.status,
        created_at: payload.created_at,
        last_active_at: payload.last_active_at,
        worker_agents: payload.worker_agents,
        linked_thread_keys: payload.linked_thread_keys,
    }
}

#[cfg(test)]
mod tests {
    use super::SophonAdapter;
    use provider_contract::{
        ProviderHealthCheckRequest, ProviderHealthStatus, ResumeThreadRequest,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("provider-sophon-{name}-{nanos}"));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[cfg(unix)]
    fn write_stub_cli() -> String {
        use std::os::unix::fs::PermissionsExt;

        let dir = test_temp_dir("stub-cli");
        let script_path = dir.join("sophon-stub.sh");
        fs::write(
            &script_path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "sophon 0.1.0"
  exit 0
fi
if [ "$1" = "health" ]; then
  echo '{"status":"healthy","checkedAt":"2026-03-09T00:00:00Z","message":"ok"}'
  exit 0
fi
if [ "$1" = "threads" ] && [ "$2" = "list" ]; then
  echo '[{"id":"sophon-a","projectPath":"/workspace/demo","title":"Demo","tags":["sophon"],"lastActiveAt":"2026-03-09T00:00:00Z","lastMessagePreview":"hello"}]'
  exit 0
fi
if [ "$1" = "threads" ] && [ "$2" = "runtime" ]; then
  echo '{"agentAnswering":false,"lastEventKind":"assistant_message","lastEventAtMs":1700000000000}'
  exit 0
fi
if [ "$1" = "threads" ] && [ "$2" = "resume" ]; then
  echo '{"threadId":"'"$3"'","resumed":true,"message":null}'
  exit 0
fi
if [ "$1" = "conductor" ] && [ "$2" = "sessions" ] && [ "$3" = "list" ]; then
  echo '[{"id":"conductor-a","title":"Demo Workspace","workspacePath":"/workspace/demo","status":"idle","createdAt":"2026-03-09T00:00:00Z","lastActiveAt":"2026-03-09T00:00:00Z","workerAgents":["codex"],"linkedThreadKeys":["codex:thread-1"]}]'
  exit 0
fi
if [ "$1" = "conductor" ] && [ "$2" = "sessions" ] && [ "$3" = "start" ]; then
  echo '{"id":"conductor-b","title":"Started Workspace","workspacePath":"'"$5"'","status":"idle","createdAt":"2026-03-09T00:00:00Z","lastActiveAt":"2026-03-09T00:00:00Z","workerAgents":[],"linkedThreadKeys":[]}'
  exit 0
fi
echo "unexpected args: $@" >&2
exit 1
"#,
        )
        .expect("stub cli should be written");

        let mut permissions = fs::metadata(&script_path)
            .expect("metadata should load")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("permissions should be updated");
        script_path.to_string_lossy().to_string()
    }

    #[cfg(unix)]
    #[test]
    fn health_check_reads_sophon_cli_json() {
        let adapter = SophonAdapter::new().with_cli_binary(write_stub_cli());
        let result = adapter
            .health_check(ProviderHealthCheckRequest {
                profile_name: "default".to_string(),
                project_path: None,
            })
            .expect("health check should succeed");

        assert_eq!(result.provider_id, "sophon");
        assert_eq!(result.status, ProviderHealthStatus::Healthy);
        assert_eq!(result.message.as_deref(), Some("ok"));
    }

    #[cfg(unix)]
    #[test]
    fn list_threads_reads_sophon_cli_json() {
        let adapter = SophonAdapter::new().with_cli_binary(write_stub_cli());
        let threads = adapter
            .list_thread_overviews(None)
            .expect("threads should load");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].summary.provider_id, "sophon");
        assert_eq!(threads[0].summary.id, "sophon-a");
        assert_eq!(threads[0].last_message_preview.as_deref(), Some("hello"));
    }

    #[cfg(unix)]
    #[test]
    fn resume_thread_round_trips_thread_id() {
        let adapter = SophonAdapter::new().with_cli_binary(write_stub_cli());
        let result = adapter
            .resume_thread(ResumeThreadRequest {
                thread_id: "resume-me".to_string(),
                project_path: None,
            })
            .expect("resume should succeed");

        assert_eq!(result.thread_id, "resume-me");
        assert!(result.resumed);
    }

    #[cfg(unix)]
    #[test]
    fn conductor_sessions_round_trip_from_cli() {
        let adapter = SophonAdapter::new().with_cli_binary(write_stub_cli());
        let sessions = adapter
            .list_conductor_sessions()
            .expect("sessions should load");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].workspace_path, "/workspace/demo");

        let created = adapter
            .start_conductor_session("/workspace/next")
            .expect("start should succeed");
        assert_eq!(created.workspace_path, "/workspace/next");
    }
}
