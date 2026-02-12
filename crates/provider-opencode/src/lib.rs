use provider_contract::{
    ProviderAdapter, ProviderError, ProviderErrorCode, ProviderHealthCheckRequest,
    ProviderHealthCheckResult, ProviderHealthStatus, ProviderId, ProviderResult,
    ResumeThreadRequest, ResumeThreadResult, SwitchContextSummary, ThreadSummary,
};
use serde_json::Value;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const OPENCODE_DATA_DIR_ENV: &str = "AGENTDOCK_OPENCODE_DATA_DIR";
const OPENCODE_BINARY_ENV: &str = "AGENTDOCK_OPENCODE_BIN";
const MESSAGE_KIND_TEXT: &str = "text";
const MESSAGE_KIND_TOOL: &str = "tool";
const OPENCODE_AGENT_ACTIVITY_WINDOW_MS: i64 = 120_000;

#[derive(Debug, Clone)]
struct ThreadRecord {
    summary: ThreadSummary,
    session_id: String,
    sort_key: i64,
}

#[derive(Debug, Clone)]
struct MessageRecord {
    role: String,
    content: String,
    timestamp_ms: Option<i64>,
    kind: String,
    collapsed: bool,
}

#[derive(Debug, Clone)]
struct OpenCodeMessageNode {
    id: String,
    role: String,
    created_ms: Option<i64>,
    completed_ms: Option<i64>,
    timestamp_ms: Option<i64>,
    sort_key: i64,
    summary_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeThreadMessage {
    pub role: String,
    pub content: String,
    pub timestamp_ms: Option<i64>,
    pub kind: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeThreadOverview {
    pub summary: ThreadSummary,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeThreadRuntimeState {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenCodeSemanticEventKind {
    UserMessage,
    AgentReasoning,
    AgentTool,
    AgentMessage,
    TurnCompleted,
}

impl OpenCodeSemanticEventKind {
    fn as_str(self) -> &'static str {
        match self {
            OpenCodeSemanticEventKind::UserMessage => "user_message",
            OpenCodeSemanticEventKind::AgentReasoning => "agent_reasoning",
            OpenCodeSemanticEventKind::AgentTool => "agent_tool",
            OpenCodeSemanticEventKind::AgentMessage => "agent_message",
            OpenCodeSemanticEventKind::TurnCompleted => "turn_completed",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpenCodeAdapter {
    data_dir_override: Option<PathBuf>,
    cli_binary_override: Option<String>,
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_data_dir<P: Into<PathBuf>>(mut self, data_dir: P) -> Self {
        self.data_dir_override = Some(data_dir.into());
        self
    }

    pub fn with_cli_binary<S: Into<String>>(mut self, cli_binary: S) -> Self {
        self.cli_binary_override = Some(cli_binary.into());
        self
    }

    pub fn get_thread_messages(
        &self,
        thread_id: &str,
    ) -> ProviderResult<Vec<OpenCodeThreadMessage>> {
        self.find_thread_record(thread_id)?;
        let messages = load_thread_messages(&self.opencode_storage_dir(), thread_id);
        Ok(messages
            .into_iter()
            .map(|message| OpenCodeThreadMessage {
                role: message.role,
                content: message.content,
                timestamp_ms: message.timestamp_ms,
                kind: message.kind,
                collapsed: message.collapsed,
            })
            .collect())
    }

    pub fn get_thread_runtime_state(
        &self,
        thread_id: &str,
    ) -> ProviderResult<OpenCodeThreadRuntimeState> {
        self.find_thread_record(thread_id)?;
        Ok(load_thread_runtime_state(
            &self.opencode_storage_dir(),
            thread_id,
        ))
    }

    pub fn list_thread_overviews(
        &self,
        project_path: Option<&str>,
    ) -> ProviderResult<Vec<OpenCodeThreadOverview>> {
        let mut records = self.scan_thread_records();

        if let Some(filter) = project_path {
            records.retain(|record| record.summary.project_path.starts_with(filter));
        }

        records.sort_by_key(|record| Reverse(record.sort_key));
        let storage_dir = self.opencode_storage_dir();
        Ok(records
            .into_iter()
            .map(|record| OpenCodeThreadOverview {
                last_message_preview: build_last_message_preview(&storage_dir, &record.session_id),
                summary: record.summary,
            })
            .collect())
    }

    fn opencode_binary(&self) -> String {
        if let Some(binary) = &self.cli_binary_override {
            return binary.clone();
        }
        if let Ok(binary) = std::env::var(OPENCODE_BINARY_ENV) {
            let trimmed = binary.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        "opencode".to_string()
    }

    fn opencode_data_dir(&self) -> PathBuf {
        if let Some(path) = &self.data_dir_override {
            return path.clone();
        }

        if let Ok(path) = std::env::var(OPENCODE_DATA_DIR_ENV) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }

        if let Some(path) = default_opencode_data_dir() {
            return path;
        }

        PathBuf::from(".opencode")
    }

    fn opencode_storage_dir(&self) -> PathBuf {
        self.opencode_data_dir().join("storage")
    }

    fn opencode_sessions_dir(&self) -> PathBuf {
        self.opencode_storage_dir().join("session")
    }

    fn opencode_projects_dir(&self) -> PathBuf {
        self.opencode_storage_dir().join("project")
    }

    fn scan_thread_records(&self) -> Vec<ThreadRecord> {
        let mut files = Vec::new();
        collect_json_files_recursive(&self.opencode_sessions_dir(), &mut files);

        let project_map = load_project_worktree_map(&self.opencode_projects_dir());
        let mut records = Vec::new();
        for path in files {
            if let Some(record) = parse_session_file(&path, &project_map) {
                records.push(record);
            }
        }

        records.sort_by_key(|record| Reverse(record.sort_key));
        records
    }

    fn find_thread_record(&self, thread_id: &str) -> ProviderResult<ThreadRecord> {
        self.scan_thread_records()
            .into_iter()
            .find(|record| record.summary.id == thread_id)
            .ok_or_else(|| {
                provider_error(
                    ProviderErrorCode::InvalidResponse,
                    format!("OpenCode thread not found: {thread_id}"),
                    false,
                )
            })
    }

    fn ensure_cli_reachable(&self) -> ProviderResult<()> {
        let binary = self.opencode_binary();
        match Command::new(&binary).arg("--version").output() {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("OpenCode CLI not found in PATH: {binary}"),
                false,
            )),
            Err(error) => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Failed to execute OpenCode CLI ({binary}): {error}"),
                true,
            )),
        }
    }
}

impl ProviderAdapter for OpenCodeAdapter {
    fn provider_id(&self) -> ProviderId {
        ProviderId::OpenCode
    }

    fn health_check(
        &self,
        request: ProviderHealthCheckRequest,
    ) -> ProviderResult<ProviderHealthCheckResult> {
        let checked_at = now_unix_millis().to_string();
        let binary = self.opencode_binary();

        match Command::new(&binary).arg("--version").output() {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProviderHealthCheckResult {
                    provider_id: ProviderId::OpenCode,
                    status: ProviderHealthStatus::Offline,
                    checked_at,
                    message: Some(format!("OpenCode CLI not found in PATH: {binary}")),
                });
            }
            Err(error) => {
                return Err(provider_error(
                    ProviderErrorCode::UpstreamUnavailable,
                    format!("Failed to execute OpenCode CLI ({binary}): {error}"),
                    true,
                ));
            }
        }

        let sessions_dir = self.opencode_sessions_dir();
        if !sessions_dir.exists() {
            return Ok(ProviderHealthCheckResult {
                provider_id: ProviderId::OpenCode,
                status: ProviderHealthStatus::Degraded,
                checked_at,
                message: Some(format!(
                    "OpenCode sessions directory not found at {} (profile={})",
                    sessions_dir.display(),
                    request.profile_name
                )),
            });
        }

        Ok(ProviderHealthCheckResult {
            provider_id: ProviderId::OpenCode,
            status: ProviderHealthStatus::Healthy,
            checked_at,
            message: Some(format!(
                "OpenCode CLI reachable, sessions directory loaded ({})",
                request.profile_name
            )),
        })
    }

    fn list_threads(&self, project_path: Option<&str>) -> ProviderResult<Vec<ThreadSummary>> {
        let mut records = self.scan_thread_records();

        if let Some(filter) = project_path {
            records.retain(|record| record.summary.project_path.starts_with(filter));
        }

        records.sort_by_key(|record| Reverse(record.sort_key));
        Ok(records.into_iter().map(|record| record.summary).collect())
    }

    fn resume_thread(&self, request: ResumeThreadRequest) -> ProviderResult<ResumeThreadResult> {
        self.ensure_cli_reachable()?;
        let thread_record = self.find_thread_record(&request.thread_id)?;

        let project_path = request
            .project_path
            .clone()
            .filter(|path| !path.trim().is_empty())
            .or_else(|| {
                if thread_record.summary.project_path == "." {
                    None
                } else {
                    Some(thread_record.summary.project_path.clone())
                }
            });

        let mut command = format!("{} --session {}", self.opencode_binary(), request.thread_id);
        if let Some(path) = project_path {
            command = format!("cd {} && {command}", shell_quote(&path));
        }

        Ok(ResumeThreadResult {
            thread_id: request.thread_id,
            resumed: true,
            message: Some(format!(
                "OpenCode thread is resumable. Run command in terminal: {command}"
            )),
        })
    }

    fn summarize_switch_context(&self, thread_id: &str) -> ProviderResult<SwitchContextSummary> {
        let thread_record = self.find_thread_record(thread_id)?;
        let messages = self.get_thread_messages(thread_id)?;

        let first_user_message = messages.iter().find(|msg| msg.role == "user");
        let latest_user_message = messages.iter().rev().find(|msg| msg.role == "user");

        let objective = first_user_message
            .map(|msg| truncate_text(&msg.content, 180))
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| format!("Continue OpenCode thread {thread_id}"));

        let mut constraints =
            vec!["Preserve existing project constraints and coding style.".to_string()];
        if thread_record.summary.project_path != "." {
            constraints.push(format!(
                "Use project directory: {}",
                thread_record.summary.project_path
            ));
        }

        let pending_tasks = latest_user_message
            .map(|msg| {
                format!(
                    "Continue from latest request: {}",
                    truncate_text(&msg.content, 140)
                )
            })
            .map(|line| vec![line])
            .unwrap_or_else(|| vec![format!("Resume OpenCode thread {thread_id}")]);

        Ok(SwitchContextSummary {
            objective,
            constraints,
            pending_tasks,
        })
    }
}

fn load_project_worktree_map(projects_dir: &Path) -> HashMap<String, String> {
    if !projects_dir.exists() {
        return HashMap::new();
    }

    let entries = match fs::read_dir(projects_dir) {
        Ok(entries) => entries,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let parsed: Value = match serde_json::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        let id = parsed.get("id").and_then(Value::as_str);
        let worktree = parsed.get("worktree").and_then(Value::as_str);
        if let (Some(id), Some(worktree)) = (id, worktree) {
            map.insert(id.to_string(), worktree.to_string());
        }
    }

    map
}

fn collect_json_files_recursive(root: &Path, output: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files_recursive(&path, output);
            continue;
        }

        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            output.push(path);
        }
    }
}

fn parse_session_file(path: &Path, project_map: &HashMap<String, String>) -> Option<ThreadRecord> {
    let raw = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;

    let parent_id = parsed
        .get("parentID")
        .or_else(|| parsed.get("parentId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if parent_id.is_some() {
        return None;
    }

    let session_id = parsed
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(ToString::to_string)
        })?;

    let project_id = parsed.get("projectID").and_then(Value::as_str);
    let project_path = parsed
        .get("directory")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            project_id
                .and_then(|id| project_map.get(id))
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| ".".to_string());

    let title = parsed
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToString::to_string)
        .or_else(|| path_basename(&project_path).map(ToString::to_string))
        .unwrap_or_else(|| format!("OpenCode session {}", truncate_text(&session_id, 8)));

    let created_ms = extract_timestamp_ms(parsed.get("time").and_then(|time| time.get("created")));
    let updated_ms = extract_timestamp_ms(parsed.get("time").and_then(|time| time.get("updated")));
    let sort_key = updated_ms
        .or(created_ms)
        .or_else(|| file_last_modified_ms(path))
        .unwrap_or(0);

    let summary = ThreadSummary {
        id: session_id.clone(),
        provider_id: ProviderId::OpenCode,
        account_id: None,
        project_path,
        title,
        tags: vec!["opencode".to_string()],
        last_active_at: updated_ms
            .or(created_ms)
            .unwrap_or_else(now_unix_millis)
            .to_string(),
    };

    Some(ThreadRecord {
        summary,
        session_id,
        sort_key,
    })
}

fn load_thread_messages(storage_dir: &Path, session_id: &str) -> Vec<MessageRecord> {
    let message_dir = storage_dir.join("message").join(session_id);
    if !message_dir.exists() {
        return Vec::new();
    }

    let mut message_files = Vec::new();
    collect_json_files_recursive(&message_dir, &mut message_files);

    let mut nodes = message_files
        .into_iter()
        .filter_map(|path| parse_message_file(&path))
        .collect::<Vec<OpenCodeMessageNode>>();
    nodes.sort_by_key(|node| node.sort_key);

    let mut records = Vec::new();
    for node in nodes {
        let mut parts = load_part_records(storage_dir, &node.id, &node.role, node.timestamp_ms);
        if parts.is_empty() {
            if node.role == "user" {
                if let Some(summary_title) = node
                    .summary_title
                    .as_ref()
                    .and_then(|text| normalize_text(text))
                {
                    parts.push(text_record("user", summary_title, node.timestamp_ms));
                }
            }
        }
        records.extend(parts);
    }

    records
}

fn parse_message_file(path: &Path) -> Option<OpenCodeMessageNode> {
    let raw = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;

    let id = parsed
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(ToString::to_string)
        })?;

    let role = parsed
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("assistant")
        .to_string();

    let created_ms = extract_timestamp_ms(parsed.get("time").and_then(|time| time.get("created")));
    let completed_ms =
        extract_timestamp_ms(parsed.get("time").and_then(|time| time.get("completed")));
    let timestamp_ms = completed_ms.or(created_ms);
    let sort_key = timestamp_ms
        .or_else(|| file_last_modified_ms(path))
        .unwrap_or(0);

    let summary_title = parsed
        .get("summary")
        .and_then(|value| value.get("title"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    Some(OpenCodeMessageNode {
        id,
        role,
        created_ms,
        completed_ms,
        timestamp_ms,
        sort_key,
        summary_title,
    })
}

fn load_part_records(
    storage_dir: &Path,
    message_id: &str,
    role: &str,
    timestamp_ms: Option<i64>,
) -> Vec<MessageRecord> {
    let parts_dir = storage_dir.join("part").join(message_id);
    if !parts_dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(parts_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<PathBuf>>();
    files.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .unwrap_or_default()
    });

    let mut records = Vec::new();
    for path in files {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let parsed: Value = match serde_json::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        let part_type = parsed.get("type").and_then(Value::as_str).unwrap_or("");
        match part_type {
            "text" => {
                if let Some(content) = parsed
                    .get("text")
                    .and_then(Value::as_str)
                    .and_then(normalize_text)
                {
                    records.push(text_record(role, content, timestamp_ms));
                }
            }
            "reasoning" => {
                if let Some(content) = parsed
                    .get("text")
                    .and_then(Value::as_str)
                    .and_then(normalize_text)
                {
                    records.push(tool_record(
                        role,
                        format!("Reasoning\n{}", truncate_text(&content, 800)),
                        timestamp_ms,
                    ));
                }
            }
            "tool" => {
                if let Some(content) = summarize_tool_part(&parsed) {
                    records.push(tool_record(role, content, timestamp_ms));
                }
            }
            _ => {}
        }
    }

    records
}

fn summarize_tool_part(part: &Value) -> Option<String> {
    let tool_name = part
        .get("tool")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Tool");

    let state = part.get("state");
    let input = state.and_then(|value| value.get("input"));
    let output = state.and_then(|value| value.get("output"));

    let input_summary = input.and_then(summarize_tool_input);
    let output_summary = output.and_then(summarize_tool_output);

    if input_summary.is_none() && output_summary.is_none() {
        return Some(tool_name.to_string());
    }

    let mut lines = vec![tool_name.to_string()];
    if let Some(input_summary) = input_summary {
        lines.push(format_io_block("IN", &input_summary));
    }
    if let Some(output_summary) = output_summary {
        lines.push(format_io_block("OUT", &output_summary));
    }

    Some(lines.join("\n"))
}

fn format_io_block(label: &str, value: &str) -> String {
    if value.contains('\n') {
        format!("{label}\n{value}")
    } else {
        format!("{label} {value}")
    }
}

fn summarize_tool_input(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => normalize_text(text).map(|text| truncate_text(&text, 260)),
        Value::Object(object) => {
            if let Some(command) = object.get("command") {
                let rendered = render_command(command);
                if !rendered.is_empty() {
                    return Some(truncate_text(&rendered, 260));
                }
            }

            if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
                return Some(format!("pattern: {}", truncate_text(pattern.trim(), 180)));
            }

            if let Some(path) = object.get("path").and_then(Value::as_str) {
                return Some(format!("path: {}", truncate_text(path.trim(), 180)));
            }

            serde_json::to_string(value)
                .ok()
                .and_then(|raw| normalize_text(&raw))
                .map(|raw| truncate_text(&raw, 260))
        }
        Value::Array(_) => serde_json::to_string(value)
            .ok()
            .and_then(|raw| normalize_text(&raw))
            .map(|raw| truncate_text(&raw, 260)),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn summarize_tool_output(value: &Value) -> Option<String> {
    let raw = match value {
        Value::String(text) => text.to_string(),
        _ => serde_json::to_string(value).ok()?,
    };

    let cleaned = strip_ansi_escapes(&raw);
    let lines = cleaned
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .take(10)
        .map(|line| truncate_text(line.trim(), 220))
        .collect::<Vec<String>>();

    if lines.is_empty() {
        return None;
    }

    Some(lines.join("\n"))
}

fn render_command(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<&str>>()
            .join(" "),
        _ => String::new(),
    }
}

fn strip_ansi_escapes(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut in_escape = false;

    for ch in raw.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() || ch == '~' {
                in_escape = false;
            }
            continue;
        }

        if ch == '\u{1b}' {
            in_escape = true;
            continue;
        }

        output.push(ch);
    }

    output
}

fn load_thread_runtime_state(storage_dir: &Path, session_id: &str) -> OpenCodeThreadRuntimeState {
    let message_dir = storage_dir.join("message").join(session_id);
    if !message_dir.exists() {
        return OpenCodeThreadRuntimeState {
            agent_answering: false,
            last_event_kind: None,
            last_event_at_ms: None,
        };
    }

    let mut message_files = Vec::new();
    collect_json_files_recursive(&message_dir, &mut message_files);
    let mut nodes = message_files
        .into_iter()
        .filter_map(|path| parse_message_file(&path))
        .collect::<Vec<OpenCodeMessageNode>>();
    nodes.sort_by_key(|node| node.sort_key);

    let mut last_kind: Option<OpenCodeSemanticEventKind> = None;
    let mut last_event_at_ms: Option<i64> = None;
    let mut latest_in_progress_assistant_at: Option<i64> = None;

    for node in nodes {
        let fallback_ts = node.timestamp_ms.or(node.created_ms);
        if node.role == "user" {
            last_kind = Some(OpenCodeSemanticEventKind::UserMessage);
            if let Some(ts) = fallback_ts {
                last_event_at_ms = Some(ts);
            }
            continue;
        }

        if node.role == "assistant" {
            let part_events = load_part_event_kinds(storage_dir, &node.id, fallback_ts);
            if part_events.is_empty() {
                last_kind = Some(OpenCodeSemanticEventKind::AgentMessage);
                if let Some(ts) = fallback_ts {
                    last_event_at_ms = Some(ts);
                }
            } else {
                for (kind, timestamp_ms) in part_events {
                    last_kind = Some(kind);
                    if let Some(ts) = timestamp_ms {
                        last_event_at_ms = Some(ts);
                    }
                }
            }

            if node.completed_ms.is_none() {
                if let Some(ts) = node.created_ms.or(fallback_ts) {
                    latest_in_progress_assistant_at = Some(ts);
                }
            }
        }
    }

    let agent_answering = latest_in_progress_assistant_at
        .map(|ts| now_unix_millis().saturating_sub(ts) <= OPENCODE_AGENT_ACTIVITY_WINDOW_MS)
        .unwrap_or(false);

    OpenCodeThreadRuntimeState {
        agent_answering,
        last_event_kind: last_kind.map(|kind| kind.as_str().to_string()),
        last_event_at_ms,
    }
}

fn load_part_event_kinds(
    storage_dir: &Path,
    message_id: &str,
    fallback_ts: Option<i64>,
) -> Vec<(OpenCodeSemanticEventKind, Option<i64>)> {
    let parts_dir = storage_dir.join("part").join(message_id);
    if !parts_dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(parts_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<PathBuf>>();
    files.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .unwrap_or_default()
    });

    let mut events = Vec::new();
    for path in files {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let parsed: Value = match serde_json::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        let part_type = parsed.get("type").and_then(Value::as_str).unwrap_or("");
        let kind = match part_type {
            "reasoning" => Some(OpenCodeSemanticEventKind::AgentReasoning),
            "tool" => Some(OpenCodeSemanticEventKind::AgentTool),
            "text" => Some(OpenCodeSemanticEventKind::AgentMessage),
            "step-finish" => Some(OpenCodeSemanticEventKind::TurnCompleted),
            _ => None,
        };

        if let Some(kind) = kind {
            let timestamp_ms = extract_timestamp_ms(
                parsed
                    .get("time")
                    .and_then(|time| time.get("end"))
                    .or_else(|| parsed.get("time").and_then(|time| time.get("start"))),
            )
            .or(fallback_ts);
            events.push((kind, timestamp_ms));
        }
    }

    events
}

fn build_last_message_preview(storage_dir: &Path, session_id: &str) -> Option<String> {
    let messages = load_thread_messages(storage_dir, session_id);
    let message = messages
        .iter()
        .rev()
        .find(|record| record.kind == MESSAGE_KIND_TEXT && !record.content.trim().is_empty())
        .or_else(|| {
            messages
                .iter()
                .rev()
                .find(|record| !record.content.trim().is_empty())
        })?;

    let normalized = message
        .content
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    if normalized.is_empty() {
        return None;
    }

    Some(truncate_text(&normalized, 140))
}

fn extract_timestamp_ms(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    match value {
        Value::Number(number) => number.as_i64().map(normalize_epoch),
        Value::String(raw) => raw.trim().parse::<i64>().ok().map(normalize_epoch),
        _ => None,
    }
}

fn normalize_epoch(raw: i64) -> i64 {
    if raw.abs() < 1_000_000_000_000 {
        raw * 1000
    } else {
        raw
    }
}

fn normalize_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn provider_error(code: ProviderErrorCode, message: String, retryable: bool) -> ProviderError {
    ProviderError {
        code,
        message,
        retryable,
    }
}

fn truncate_text(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut chars = input.chars();
    let mut result = String::with_capacity(max_chars);
    for _ in 0..max_chars {
        match chars.next() {
            Some(ch) => result.push(ch),
            None => return input.to_string(),
        }
    }

    if chars.next().is_some() {
        result.push_str("...");
    }

    result
}

fn file_last_modified_ms(path: &Path) -> Option<i64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(duration.as_millis() as i64)
}

fn path_basename(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn default_home_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home));
        }
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home));
        }
    }

    let home_drive = std::env::var("HOMEDRIVE").ok()?;
    let home_path = std::env::var("HOMEPATH").ok()?;
    let combined = format!("{home_drive}{home_path}");
    if combined.trim().is_empty() {
        return None;
    }

    Some(PathBuf::from(combined))
}

fn default_opencode_data_dir() -> Option<PathBuf> {
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        let trimmed = xdg_data_home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("opencode"));
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("opencode"));
        }
    }

    default_home_dir().map(|home| home.join(".local").join("share").join("opencode"))
}

fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\"'\"'"))
}

fn text_record(role: &str, content: String, timestamp_ms: Option<i64>) -> MessageRecord {
    MessageRecord {
        role: role.to_string(),
        content,
        timestamp_ms,
        kind: MESSAGE_KIND_TEXT.to_string(),
        collapsed: false,
    }
}

fn tool_record(role: &str, content: String, timestamp_ms: Option<i64>) -> MessageRecord {
    MessageRecord {
        role: role.to_string(),
        content,
        timestamp_ms,
        kind: MESSAGE_KIND_TOOL.to_string(),
        collapsed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_temp_dir(name: &str) -> PathBuf {
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "agentdock-provider-opencode-{name}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("test temp dir should be creatable");
        dir
    }

    fn write_json(path: &Path, payload: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be creatable");
        }
        fs::write(path, payload).expect("file should be writable");
    }

    #[test]
    fn list_threads_reads_opencode_sessions() {
        let data_dir = test_temp_dir("list-threads").join("opencode");
        let project_id = "proj-a";
        write_json(
            &data_dir.join("storage").join("project").join("proj-a.json"),
            &format!(
                r#"{{"id":"{project_id}","worktree":"/workspace/a","time":{{"updated":1760000000123}}}}"#
            ),
        );
        write_json(
            &data_dir
                .join("storage")
                .join("session")
                .join(project_id)
                .join("ses_a.json"),
            &format!(
                r#"{{"id":"ses_a","projectID":"{project_id}","directory":"/workspace/a","title":"Session A","time":{{"created":1760000000000,"updated":1760000000999}}}}"#
            ),
        );

        let adapter = OpenCodeAdapter::new().with_data_dir(&data_dir);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "ses_a");
        assert_eq!(threads[0].provider_id, ProviderId::OpenCode);
        assert_eq!(threads[0].project_path, "/workspace/a");
        assert_eq!(threads[0].title, "Session A");
    }

    #[test]
    fn list_threads_ignores_child_agent_sessions() {
        let data_dir = test_temp_dir("list-threads-child-filter").join("opencode");
        let project_id = "proj-child";
        let parent_session_id = "ses_parent";
        let child_session_id = "ses_child";

        write_json(
            &data_dir
                .join("storage")
                .join("project")
                .join("proj-child.json"),
            &format!(
                r#"{{"id":"{project_id}","worktree":"/workspace/filter","time":{{"updated":1760000000123}}}}"#
            ),
        );
        write_json(
            &data_dir
                .join("storage")
                .join("session")
                .join(project_id)
                .join(format!("{parent_session_id}.json")),
            &format!(
                r#"{{"id":"{parent_session_id}","projectID":"{project_id}","directory":"/workspace/filter","title":"Parent Session","time":{{"created":1760000000000,"updated":1760000001000}}}}"#
            ),
        );
        write_json(
            &data_dir
                .join("storage")
                .join("session")
                .join(project_id)
                .join(format!("{child_session_id}.json")),
            &format!(
                r#"{{"id":"{child_session_id}","projectID":"{project_id}","directory":"/workspace/filter","parentID":"{parent_session_id}","title":"Child Session (@explore subagent)","time":{{"created":1760000000001,"updated":1760000001001}}}}"#
            ),
        );

        let adapter = OpenCodeAdapter::new().with_data_dir(&data_dir);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, parent_session_id);
        assert_eq!(threads[0].title, "Parent Session");
    }

    #[test]
    fn get_thread_messages_reads_text_and_tool_parts() {
        let data_dir = test_temp_dir("messages").join("opencode");
        let session_id = "ses_msg";
        write_json(
            &data_dir
                .join("storage")
                .join("session")
                .join("global")
                .join(format!("{session_id}.json")),
            &format!(
                r#"{{"id":"{session_id}","projectID":"global","directory":"/workspace/b","title":"Session B","time":{{"created":1760001000000,"updated":1760001000099}}}}"#
            ),
        );

        write_json(
            &data_dir
                .join("storage")
                .join("message")
                .join(session_id)
                .join("msg_user.json"),
            &format!(
                r#"{{"id":"msg_user","sessionID":"{session_id}","role":"user","time":{{"created":1760001000001}}}}"#
            ),
        );
        write_json(
            &data_dir
                .join("storage")
                .join("part")
                .join("msg_user")
                .join("prt_001.json"),
            &format!(
                r#"{{"id":"prt_001","sessionID":"{session_id}","messageID":"msg_user","type":"text","text":"hello opencode"}}"#
            ),
        );

        write_json(
            &data_dir
                .join("storage")
                .join("message")
                .join(session_id)
                .join("msg_assistant.json"),
            &format!(
                r#"{{"id":"msg_assistant","sessionID":"{session_id}","role":"assistant","time":{{"created":1760001001001,"completed":1760001002001}},"finish":"stop"}}"#
            ),
        );
        write_json(
            &data_dir
                .join("storage")
                .join("part")
                .join("msg_assistant")
                .join("prt_001.json"),
            &format!(
                r#"{{"id":"prt_001","sessionID":"{session_id}","messageID":"msg_assistant","type":"tool","tool":"grep","state":{{"status":"completed","input":{{"pattern":"hello"}},"output":"found"}}}}"#
            ),
        );
        write_json(
            &data_dir
                .join("storage")
                .join("part")
                .join("msg_assistant")
                .join("prt_002.json"),
            &format!(
                r#"{{"id":"prt_002","sessionID":"{session_id}","messageID":"msg_assistant","type":"text","text":"done"}}"#
            ),
        );

        let adapter = OpenCodeAdapter::new().with_data_dir(&data_dir);
        let messages = adapter
            .get_thread_messages(session_id)
            .expect("messages should load");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].kind, MESSAGE_KIND_TEXT);
        assert_eq!(messages[0].content, "hello opencode");

        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].kind, MESSAGE_KIND_TOOL);
        assert!(messages[1].content.contains("grep"));
        assert!(messages[1].content.contains("IN"));
        assert!(messages[1].content.contains("OUT"));

        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].kind, MESSAGE_KIND_TEXT);
        assert_eq!(messages[2].content, "done");
    }

    #[test]
    fn runtime_state_marks_in_progress_assistant_as_answering() {
        let data_dir = test_temp_dir("runtime-answering").join("opencode");
        let session_id = "ses_runtime";
        let now = now_unix_millis();

        write_json(
            &data_dir
                .join("storage")
                .join("session")
                .join("global")
                .join(format!("{session_id}.json")),
            &format!(
                r#"{{"id":"{session_id}","projectID":"global","directory":"/workspace/c","title":"Runtime","time":{{"created":{now},"updated":{now}}}}}"#
            ),
        );

        write_json(
            &data_dir
                .join("storage")
                .join("message")
                .join(session_id)
                .join("msg_assistant.json"),
            &format!(
                r#"{{"id":"msg_assistant","sessionID":"{session_id}","role":"assistant","time":{{"created":{}}}}}"#,
                now - 2_000
            ),
        );

        write_json(
            &data_dir
                .join("storage")
                .join("part")
                .join("msg_assistant")
                .join("prt_001.json"),
            &format!(
                r#"{{"id":"prt_001","sessionID":"{session_id}","messageID":"msg_assistant","type":"reasoning","text":"thinking","time":{{"start":{},"end":{}}}}}"#,
                now - 1_500,
                now - 1_000
            ),
        );

        let adapter = OpenCodeAdapter::new().with_data_dir(&data_dir);
        let state = adapter
            .get_thread_runtime_state(session_id)
            .expect("runtime state should load");

        assert!(state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_reasoning"));
    }

    #[test]
    fn runtime_state_marks_completed_assistant_as_not_answering() {
        let data_dir = test_temp_dir("runtime-idle").join("opencode");
        let session_id = "ses_runtime_done";
        let now = now_unix_millis();

        write_json(
            &data_dir
                .join("storage")
                .join("session")
                .join("global")
                .join(format!("{session_id}.json")),
            &format!(
                r#"{{"id":"{session_id}","projectID":"global","directory":"/workspace/d","title":"Runtime","time":{{"created":{now},"updated":{now}}}}}"#
            ),
        );

        write_json(
            &data_dir
                .join("storage")
                .join("message")
                .join(session_id)
                .join("msg_assistant.json"),
            &format!(
                r#"{{"id":"msg_assistant","sessionID":"{session_id}","role":"assistant","time":{{"created":{},"completed":{}}}}}"#,
                now - 4_000,
                now - 2_000
            ),
        );

        write_json(
            &data_dir
                .join("storage")
                .join("part")
                .join("msg_assistant")
                .join("prt_001.json"),
            &format!(
                r#"{{"id":"prt_001","sessionID":"{session_id}","messageID":"msg_assistant","type":"text","text":"done","time":{{"start":{},"end":{}}}}}"#,
                now - 2_000,
                now - 2_000
            ),
        );

        let adapter = OpenCodeAdapter::new().with_data_dir(&data_dir);
        let state = adapter
            .get_thread_runtime_state(session_id)
            .expect("runtime state should load");

        assert!(!state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_message"));
    }
}
