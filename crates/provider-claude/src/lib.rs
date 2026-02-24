use provider_contract::{
    ProviderAdapter, ProviderError, ProviderErrorCode, ProviderHealthCheckRequest,
    ProviderHealthCheckResult, ProviderHealthStatus, ProviderId, ProviderResult,
    ResumeThreadRequest, ResumeThreadResult, ThreadSummary,
};
use serde_json::Value;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const CLAUDE_CONFIG_DIR_ENV: &str = "AGENTDOCK_CLAUDE_CONFIG_DIR";
const CLAUDE_BINARY_ENV: &str = "AGENTDOCK_CLAUDE_BIN";
const CLAUDE_AGENT_ACTIVITY_WINDOW_MS: i64 = 120_000;

#[derive(Debug, Clone)]
struct ThreadRecord {
    summary: ThreadSummary,
    source_path: PathBuf,
    sort_key: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeThreadOverview {
    pub summary: ThreadSummary,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeThreadRuntimeState {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeSemanticEventKind {
    UserMessage,
    AgentReasoning,
    AgentTool,
    AgentProgress,
    AgentMessage,
    QueueDequeue,
    TurnCompleted,
}

impl ClaudeSemanticEventKind {
    fn as_str(self) -> &'static str {
        match self {
            ClaudeSemanticEventKind::UserMessage => "user_message",
            ClaudeSemanticEventKind::AgentReasoning => "agent_reasoning",
            ClaudeSemanticEventKind::AgentTool => "agent_tool",
            ClaudeSemanticEventKind::AgentProgress => "agent_progress",
            ClaudeSemanticEventKind::AgentMessage => "agent_message",
            ClaudeSemanticEventKind::QueueDequeue => "queue_dequeue",
            ClaudeSemanticEventKind::TurnCompleted => "turn_completed",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeAdapter {
    config_dir_override: Option<PathBuf>,
    cli_binary_override: Option<String>,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config_dir<P: Into<PathBuf>>(mut self, config_dir: P) -> Self {
        self.config_dir_override = Some(config_dir.into());
        self
    }

    pub fn with_cli_binary<S: Into<String>>(mut self, cli_binary: S) -> Self {
        self.cli_binary_override = Some(cli_binary.into());
        self
    }

    pub fn get_thread_runtime_state(
        &self,
        thread_id: &str,
    ) -> ProviderResult<ClaudeThreadRuntimeState> {
        let thread_record = self.find_thread_record(thread_id)?;
        Ok(load_thread_runtime_state(&thread_record.source_path))
    }

    pub fn list_thread_overviews(
        &self,
        project_path: Option<&str>,
    ) -> ProviderResult<Vec<ClaudeThreadOverview>> {
        let mut records = self.scan_thread_records();

        if let Some(filter) = project_path {
            records.retain(|record| record.summary.project_path.starts_with(filter));
        }

        records.sort_by_key(|record| Reverse(record.sort_key));
        Ok(records
            .into_iter()
            .map(|record| ClaudeThreadOverview {
                last_message_preview: build_last_message_preview(&record.source_path),
                summary: record.summary,
            })
            .collect())
    }

    fn claude_binary(&self) -> String {
        if let Some(binary) = &self.cli_binary_override {
            return binary.clone();
        }
        if let Ok(binary) = std::env::var(CLAUDE_BINARY_ENV) {
            let trimmed = binary.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        "claude".to_string()
    }

    fn claude_config_dir(&self) -> PathBuf {
        if let Some(path) = &self.config_dir_override {
            return path.clone();
        }
        if let Ok(path) = std::env::var(CLAUDE_CONFIG_DIR_ENV) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
        if let Some(home) = default_home_dir() {
            return home.join(".claude");
        }
        PathBuf::from(".claude")
    }

    fn claude_projects_dir(&self) -> PathBuf {
        self.claude_config_dir().join("projects")
    }

    fn claude_settings_path(&self) -> PathBuf {
        let config_dir = self.claude_config_dir();
        let settings_path = config_dir.join("settings.json");
        if settings_path.exists() {
            return settings_path;
        }

        // Compatibility: Claude previously used claude.json.
        let legacy_path = config_dir.join("claude.json");
        if legacy_path.exists() {
            return legacy_path;
        }

        settings_path
    }

    fn scan_thread_records(&self) -> Vec<ThreadRecord> {
        let mut files = Vec::new();
        collect_jsonl_files(&self.claude_projects_dir(), &mut files);
        let official_titles = load_claude_history_titles(&self.claude_config_dir());

        let mut records = Vec::new();
        for path in files {
            if let Some(record) = parse_thread_file(&path, &official_titles) {
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
                    format!("Claude thread not found: {thread_id}"),
                    false,
                )
            })
    }

    fn ensure_cli_reachable(&self) -> ProviderResult<()> {
        let binary = self.claude_binary();
        match Command::new(&binary).arg("--version").output() {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Claude Code CLI not found in PATH: {binary}"),
                false,
            )),
            Err(error) => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Failed to execute Claude Code CLI ({binary}): {error}"),
                true,
            )),
        }
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn provider_id(&self) -> ProviderId {
        ProviderId::ClaudeCode
    }

    fn health_check(
        &self,
        request: ProviderHealthCheckRequest,
    ) -> ProviderResult<ProviderHealthCheckResult> {
        let checked_at = now_unix_millis().to_string();
        let binary = self.claude_binary();

        match Command::new(&binary).arg("--version").output() {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProviderHealthCheckResult {
                    provider_id: ProviderId::ClaudeCode,
                    status: ProviderHealthStatus::Offline,
                    checked_at,
                    message: Some(format!("Claude Code CLI not found in PATH: {binary}")),
                });
            }
            Err(error) => {
                return Err(provider_error(
                    ProviderErrorCode::UpstreamUnavailable,
                    format!("Failed to execute Claude Code CLI ({binary}): {error}"),
                    true,
                ));
            }
        }

        let settings_path = self.claude_settings_path();
        if !settings_path.exists() {
            return Ok(ProviderHealthCheckResult {
                provider_id: ProviderId::ClaudeCode,
                status: ProviderHealthStatus::Degraded,
                checked_at,
                message: Some(format!(
                    "Claude settings file not found at {} (profile={})",
                    settings_path.display(),
                    request.profile_name
                )),
            });
        }

        let settings = match fs::read_to_string(&settings_path) {
            Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Ok(ProviderHealthCheckResult {
                        provider_id: ProviderId::ClaudeCode,
                        status: ProviderHealthStatus::Degraded,
                        checked_at,
                        message: Some(format!(
                            "Invalid Claude settings JSON at {}: {error}",
                            settings_path.display()
                        )),
                    });
                }
            },
            Err(error) => {
                return Ok(ProviderHealthCheckResult {
                    provider_id: ProviderId::ClaudeCode,
                    status: ProviderHealthStatus::Degraded,
                    checked_at,
                    message: Some(format!(
                        "Failed to read Claude settings {}: {error}",
                        settings_path.display()
                    )),
                });
            }
        };

        let auth_mode = detect_claude_auth_mode(&settings);
        Ok(ProviderHealthCheckResult {
            provider_id: ProviderId::ClaudeCode,
            status: ProviderHealthStatus::Healthy,
            checked_at,
            message: Some(format!(
                "Claude CLI reachable, settings loaded ({}, profile={})",
                auth_mode, request.profile_name
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

        let mut command = format!("{} --resume {}", self.claude_binary(), request.thread_id);
        if let Some(path) = project_path {
            command = format!("cd {} && {command}", shell_quote(&path));
        }

        Ok(ResumeThreadResult {
            thread_id: request.thread_id,
            resumed: true,
            message: Some(format!(
                "Claude thread is resumable. Run command in terminal: {command}"
            )),
        })
    }
}

fn detect_claude_auth_mode(settings: &Value) -> &'static str {
    let env_object = settings
        .get("env")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if has_non_empty(
        env_object
            .get("ANTHROPIC_AUTH_TOKEN")
            .and_then(Value::as_str),
    ) || has_non_empty(std::env::var("ANTHROPIC_AUTH_TOKEN").ok().as_deref())
    {
        return "auth_token";
    }

    if has_non_empty(env_object.get("ANTHROPIC_API_KEY").and_then(Value::as_str))
        || has_non_empty(std::env::var("ANTHROPIC_API_KEY").ok().as_deref())
    {
        return "api_key";
    }

    "oauth_or_unknown"
}

fn has_non_empty(value: Option<&str>) -> bool {
    value.map(|text| !text.trim().is_empty()).unwrap_or(false)
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

fn provider_error(code: ProviderErrorCode, message: String, retryable: bool) -> ProviderError {
    ProviderError {
        code,
        message,
        retryable,
    }
}

fn collect_jsonl_files(root: &Path, output: &mut Vec<PathBuf>) {
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
            collect_jsonl_files(&path, output);
            continue;
        }

        if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            output.push(path);
        }
    }
}

fn load_claude_history_titles(config_dir: &Path) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    let history_path = config_dir.join("history.jsonl");
    let file = match File::open(history_path) {
        Ok(file) => file,
        Err(_) => return titles,
    };
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let session_id = match parsed.get("sessionId").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => continue,
        };
        if titles.contains_key(session_id) {
            continue;
        }
        let display = match parsed.get("display").and_then(Value::as_str) {
            Some(value) => value,
            None => continue,
        };
        if display.trim_start().starts_with('/') {
            continue;
        }
        if let Some(title) = sanitize_preview_text(display) {
            titles.insert(session_id.to_string(), truncate_text(&title, 72));
        }
    }

    titles
}

fn parse_thread_file(path: &Path, official_titles: &HashMap<String, String>) -> Option<ThreadRecord> {
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.starts_with("agent-"))
        .unwrap_or(false)
    {
        return None;
    }

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id: Option<String> = None;
    let mut project_path: Option<String> = None;
    let mut first_user_title: Option<String> = None;
    let mut created_at: Option<String> = None;
    let mut last_active_at: Option<String> = None;
    let mut sort_key = file_last_modified_ms(path).unwrap_or(0);

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if session_id.is_none() {
            session_id = parsed
                .get("sessionId")
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }

        if project_path.is_none() {
            project_path = parsed
                .get("cwd")
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }

        if first_user_title.is_none()
            && parsed.get("isMeta").and_then(Value::as_bool) != Some(true)
            && parsed.get("isSidechain").and_then(Value::as_bool) != Some(true)
        {
            if let Some(message) = parsed.get("message") {
                let role = message
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("assistant");
                if role == "user" {
                    first_user_title = extract_preview_text(message)
                        .map(|text| truncate_text(&text, 72));
                }
            }
        }

        if let Some((timestamp_str, timestamp_ms)) = extract_timestamp(&parsed) {
            if created_at.is_none() {
                created_at = Some(timestamp_str.clone());
            }
            last_active_at = Some(timestamp_str);
            if timestamp_ms > 0 {
                sort_key = sort_key.max(timestamp_ms);
            }
        }
    }

    let session_id = session_id.or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .map(ToString::to_string)
    })?;

    let project_path = project_path.unwrap_or_else(|| ".".to_string());
    let title = official_titles
        .get(&session_id)
        .and_then(|title| non_empty_trimmed(title))
        .map(ToString::to_string)
        .or(first_user_title
        .filter(|text| !text.is_empty())
        .or_else(|| path_basename(&project_path).map(ToString::to_string)))
        .unwrap_or_else(|| format!("Claude session {}", truncate_text(&session_id, 8)));

    let summary = ThreadSummary {
        id: session_id,
        provider_id: ProviderId::ClaudeCode,
        account_id: None,
        project_path,
        title,
        tags: vec!["claude_code".to_string()],
        last_active_at: last_active_at
            .or(created_at)
            .unwrap_or_else(|| now_unix_millis().to_string()),
    };

    Some(ThreadRecord {
        summary,
        source_path: path.to_path_buf(),
        sort_key,
    })
}

fn extract_timestamp(value: &Value) -> Option<(String, i64)> {
    let timestamp = value.get("timestamp")?;

    match timestamp {
        Value::Number(number) => {
            let raw = number.as_i64()?;
            let ms = normalize_epoch(raw);
            Some((ms.to_string(), ms))
        }
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(parsed) = trimmed.parse::<i64>() {
                let ms = normalize_epoch(parsed);
                return Some((ms.to_string(), ms));
            }
            if let Some(ms) = parse_rfc3339_timestamp_ms(trimmed) {
                return Some((ms.to_string(), ms));
            }
            Some((trimmed.to_string(), 0))
        }
        _ => None,
    }
}

fn parse_rfc3339_timestamp_ms(value: &str) -> Option<i64> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).ok()?;
    let nanos = parsed.unix_timestamp_nanos();
    Some((nanos / 1_000_000) as i64)
}

fn normalize_epoch(raw: i64) -> i64 {
    if raw.abs() < 1_000_000_000_000 {
        raw * 1000
    } else {
        raw
    }
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

fn parse_timestamp_ms(value: &Value) -> Option<i64> {
    let (_, timestamp_ms) = extract_timestamp(value)?;
    if timestamp_ms > 0 {
        Some(timestamp_ms)
    } else {
        None
    }
}

fn load_thread_runtime_state(path: &Path) -> ClaudeThreadRuntimeState {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return ClaudeThreadRuntimeState {
                agent_answering: false,
                last_event_kind: None,
                last_event_at_ms: None,
            };
        }
    };
    let reader = BufReader::new(file);
    let mut last_kind: Option<ClaudeSemanticEventKind> = None;
    let mut last_event_at_ms: Option<i64> = None;

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(kind) = extract_semantic_event_kind(&parsed) {
            last_kind = Some(kind);
            if let Some(timestamp_ms) = parse_timestamp_ms(&parsed) {
                last_event_at_ms = Some(timestamp_ms);
            }
        }
    }

    let is_recent = last_event_at_ms
        .map(|timestamp_ms| {
            now_unix_millis().saturating_sub(timestamp_ms) <= CLAUDE_AGENT_ACTIVITY_WINDOW_MS
        })
        .unwrap_or(false);
    let agent_answering = is_recent
        && matches!(
            last_kind,
            Some(
                ClaudeSemanticEventKind::AgentReasoning
                    | ClaudeSemanticEventKind::AgentTool
                    | ClaudeSemanticEventKind::AgentProgress
                    | ClaudeSemanticEventKind::QueueDequeue
            )
        );

    ClaudeThreadRuntimeState {
        agent_answering,
        last_event_kind: last_kind.map(|kind| kind.as_str().to_string()),
        last_event_at_ms,
    }
}

fn extract_semantic_event_kind(record: &Value) -> Option<ClaudeSemanticEventKind> {
    if record.get("type").and_then(Value::as_str) == Some("queue-operation")
        && record.get("operation").and_then(Value::as_str) == Some("dequeue")
    {
        return Some(ClaudeSemanticEventKind::QueueDequeue);
    }

    if record
        .get("toolUseResult")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        == Some("completed")
    {
        return Some(ClaudeSemanticEventKind::TurnCompleted);
    }

    if record.get("type").and_then(Value::as_str) == Some("progress") {
        if let Some(kind) = record
            .get("data")
            .and_then(extract_semantic_event_kind_from_progress)
        {
            return Some(kind);
        }
    }

    let message = record.get("message")?;
    let role_hint = record.get("type").and_then(Value::as_str);
    extract_semantic_event_kind_from_message(message, role_hint)
}

fn extract_semantic_event_kind_from_progress(data: &Value) -> Option<ClaudeSemanticEventKind> {
    if let Some(progress_message) = data.get("message") {
        if let Some(message) = progress_message.get("message") {
            let role_hint = progress_message.get("type").and_then(Value::as_str);
            if let Some(kind) = extract_semantic_event_kind_from_message(message, role_hint) {
                return Some(kind);
            }
        }
    }

    match data.get("type").and_then(Value::as_str) {
        Some("bash_progress") | Some("hook_progress") => Some(ClaudeSemanticEventKind::AgentTool),
        Some("agent_progress") => Some(ClaudeSemanticEventKind::AgentProgress),
        Some(_) => Some(ClaudeSemanticEventKind::AgentProgress),
        None => None,
    }
}

fn extract_semantic_event_kind_from_message(
    message: &Value,
    role_hint: Option<&str>,
) -> Option<ClaudeSemanticEventKind> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .or(role_hint)
        .unwrap_or("unknown");

    let content = message.get("content");

    if role == "assistant" {
        if let Some(content) = content {
            if has_content_block_type(content, &["thinking", "redacted_thinking"]) {
                return Some(ClaudeSemanticEventKind::AgentReasoning);
            }
            if has_content_block_type(content, &["tool_use", "server_tool_use"]) {
                return Some(ClaudeSemanticEventKind::AgentTool);
            }
            if has_visible_text_content(content) {
                return Some(ClaudeSemanticEventKind::AgentMessage);
            }
        }

        return Some(ClaudeSemanticEventKind::AgentMessage);
    }

    if role == "user" {
        if let Some(content) = content {
            if has_content_block_type(content, &["tool_result"]) {
                return Some(ClaudeSemanticEventKind::AgentTool);
            }
            if has_visible_text_content(content) {
                return Some(ClaudeSemanticEventKind::UserMessage);
            }
        }

        return Some(ClaudeSemanticEventKind::UserMessage);
    }

    None
}

fn has_content_block_type(value: &Value, block_types: &[&str]) -> bool {
    match value {
        Value::Array(items) => items
            .iter()
            .any(|item| has_content_block_type(item, block_types)),
        Value::Object(object) => {
            if object
                .get("type")
                .and_then(Value::as_str)
                .map(|value| block_types.contains(&value))
                .unwrap_or(false)
            {
                return true;
            }

            if let Some(content) = object.get("content") {
                if has_content_block_type(content, block_types) {
                    return true;
                }
            }

            false
        }
        _ => false,
    }
}

fn has_visible_text_content(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => items.iter().any(has_visible_text_content),
        Value::Object(object) => {
            if object
                .get("type")
                .and_then(Value::as_str)
                .map(|value| value == "text" || value == "input_text" || value == "output_text")
                .unwrap_or(false)
            {
                return object
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|text| !text.trim().is_empty())
                    .unwrap_or(false);
            }

            if let Some(text) = object.get("text").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    return true;
                }
            }

            if let Some(content) = object.get("content") {
                if has_visible_text_content(content) {
                    return true;
                }
            }

            false
        }
        _ => false,
    }
}

/// Lightweight last-message preview: scans the JSONL file and extracts the last
/// visible text content (user or assistant) without full message parsing.
fn build_last_message_preview(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last_visible_text: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if parsed.get("isMeta").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if parsed.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }

        let message = match parsed.get("message") {
            Some(value) => value,
            None => continue,
        };

        if let Some(text) = extract_preview_text(message) {
            last_visible_text = Some(text);
        }
    }

    last_visible_text.map(|text| truncate_text(&text, 140))
}

/// Extract visible text from a message content value for preview purposes.
fn extract_preview_text(message: &Value) -> Option<String> {
    let content = message.get("content")?;
    match content {
        Value::String(text) => sanitize_preview_text(text),
        Value::Array(items) => {
            // Find last visible text block in the array.
            let mut last_text: Option<String> = None;
            for item in items {
                let block_type = item.get("type").and_then(Value::as_str);
                if matches!(block_type, Some("thinking") | Some("redacted_thinking") | Some("tool_use") | Some("tool_result") | Some("server_tool_use")) {
                    continue;
                }
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    if let Some(normalized) = sanitize_preview_text(text) {
                        last_text = Some(normalized);
                    }
                }
            }
            last_text
        }
        _ => None,
    }
}

fn sanitize_preview_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || is_internal_command_text(trimmed) {
        return None;
    }
    normalize_preview_text(trimmed)
}

fn is_internal_command_text(raw: &str) -> bool {
    raw.contains("<local-command-")
        || raw.contains("<command-")
        || raw.contains("</command-")
        || raw.contains("<environment_context>")
        || raw.contains("<user_instructions>")
}

fn normalize_preview_text(raw: &str) -> Option<String> {
    let normalized = raw
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn non_empty_trimmed(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
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

fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\"'\"'"))
}

fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_lines(path: &Path, lines: &[&str]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be creatable");
        }
        let payload = format!("{}\n", lines.join("\n"));
        fs::write(path, payload).expect("file should be writable");
    }

    fn write_owned_lines(path: &Path, lines: &[String]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should be creatable");
        }
        let payload = format!("{}\n", lines.join("\n"));
        fs::write(path, payload).expect("file should be writable");
    }

    fn test_temp_dir(name: &str) -> PathBuf {
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "agentdock-provider-claude-{name}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("test temp dir should be creatable");
        dir
    }

    #[test]
    fn settings_path_prefers_settings_json_over_legacy() {
        let config_dir = test_temp_dir("settings").join(".claude");
        fs::create_dir_all(&config_dir).expect("config dir should be created");

        let settings_path = config_dir.join("settings.json");
        let legacy_path = config_dir.join("claude.json");
        fs::write(&settings_path, "{}").expect("settings should be writable");
        fs::write(&legacy_path, "{}").expect("legacy should be writable");

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        assert_eq!(adapter.claude_settings_path(), settings_path);
    }

    #[test]
    fn list_threads_reads_claude_project_sessions() {
        let config_dir = test_temp_dir("list-threads").join(".claude");
        let session_file = config_dir
            .join("projects")
            .join("workspace-a")
            .join("session-1.jsonl");

        write_lines(
            &session_file,
            &[
                r#"{"sessionId":"session-1","cwd":"/workspace/a","timestamp":"1700000000000","isMeta":true}"#,
                r#"{"sessionId":"session-1","cwd":"/workspace/a","timestamp":"1700000000500","message":{"role":"user","content":"Implement provider adapter"}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "session-1");
        assert_eq!(threads[0].provider_id, ProviderId::ClaudeCode);
        assert_eq!(threads[0].project_path, "/workspace/a");
        assert_eq!(threads[0].title, "Implement provider adapter");
    }

    #[test]
    fn list_threads_prefers_history_display_title() {
        let config_dir = test_temp_dir("title-from-history").join(".claude");
        let session_file = config_dir
            .join("projects")
            .join("workspace-title")
            .join("session-title.jsonl");
        write_lines(
            &session_file,
            &[
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000000","isMeta":true}"#,
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000100","message":{"role":"user","content":[{"type":"text","text":"Build fallback title from first user message"}]}}"#,
            ],
        );
        write_lines(
            &config_dir.join("history.jsonl"),
            &[
                r#"{"display":"/init","timestamp":1700000000050,"project":"/workspace/title","sessionId":"session-title"}"#,
                r#"{"display":"History official title","timestamp":1700000000060,"project":"/workspace/title","sessionId":"session-title"}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "session-title");
        assert_eq!(threads[0].title, "History official title");
    }

    #[test]
    fn list_threads_skips_command_messages_for_title() {
        let config_dir = test_temp_dir("title-from-user").join(".claude");
        let session_file = config_dir
            .join("projects")
            .join("workspace-title")
            .join("session-title.jsonl");

        write_lines(
            &session_file,
            &[
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000000","isMeta":true}"#,
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000100","message":{"role":"assistant","content":[{"type":"text","text":"System intro"}]}}"#,
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000150","message":{"role":"user","content":"<command-message>init</command-message><command-name>/init</command-name>"}}"#,
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000200","message":{"role":"user","content":[{"type":"text","text":"  Build   terminal-only   workflow docs  "} ]}}"#,
                r#"{"sessionId":"session-title","cwd":"/workspace/title","timestamp":"1700000000300","message":{"role":"user","content":[{"type":"text","text":"Second request should not replace title"}]}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "session-title");
        assert_eq!(threads[0].title, "Build terminal-only workflow docs");
    }

    #[test]
    fn list_threads_respects_project_filter() {
        let config_dir = test_temp_dir("project-filter").join(".claude");

        write_lines(
            &config_dir.join("projects/workspace-a/session-a.jsonl"),
            &[
                r#"{"sessionId":"session-a","cwd":"/workspace/a","timestamp":"1700000000000"}"#,
                r#"{"sessionId":"session-a","cwd":"/workspace/a","timestamp":"1700000000100","message":{"role":"user","content":"A"}}"#,
            ],
        );
        write_lines(
            &config_dir.join("projects/workspace-b/session-b.jsonl"),
            &[
                r#"{"sessionId":"session-b","cwd":"/workspace/b","timestamp":"1700000000200"}"#,
                r#"{"sessionId":"session-b","cwd":"/workspace/b","timestamp":"1700000000300","message":{"role":"user","content":"B"}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let filtered = adapter
            .list_threads(Some("/workspace/a"))
            .expect("project filter should work");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "session-a");
    }

    #[test]
    fn list_thread_overviews_returns_last_visible_message_preview() {
        let config_dir = test_temp_dir("thread-overview").join(".claude");
        let session_path = config_dir.join("projects/demo/session-overview.jsonl");

        write_lines(
            &session_path,
            &[
                r#"{"sessionId":"session-overview","cwd":"/workspace/demo","timestamp":"1700000000000","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#,
                r#"{"sessionId":"session-overview","cwd":"/workspace/demo","timestamp":"1700000000100","message":{"role":"assistant","content":[{"type":"text","text":"Final assistant response"}]}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let overviews = adapter
            .list_thread_overviews(None)
            .expect("thread overviews should work");

        assert_eq!(overviews.len(), 1);
        assert_eq!(overviews[0].summary.id, "session-overview");
        assert_eq!(
            overviews[0].last_message_preview,
            Some("Final assistant response".to_string())
        );
    }

    #[test]
    fn health_check_reports_offline_when_cli_missing() {
        let config_dir = test_temp_dir("health-offline").join(".claude");
        let adapter = ClaudeAdapter::new()
            .with_config_dir(config_dir)
            .with_cli_binary("missing-claude-binary-123");

        let result = adapter
            .health_check(ProviderHealthCheckRequest {
                profile_name: "default".to_string(),
                project_path: None,
            })
            .expect("health check should return status");

        assert_eq!(result.status, ProviderHealthStatus::Offline);
    }

    #[test]
    fn health_check_is_healthy_with_valid_settings_and_cli() {
        let config_dir = test_temp_dir("health-healthy").join(".claude");
        fs::create_dir_all(&config_dir).expect("config dir should be creatable");
        fs::write(
            config_dir.join("settings.json"),
            r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"token-123"}}"#,
        )
        .expect("settings should be writable");

        let adapter = ClaudeAdapter::new()
            .with_config_dir(config_dir)
            .with_cli_binary("rustc");

        let result = adapter
            .health_check(ProviderHealthCheckRequest {
                profile_name: "default".to_string(),
                project_path: None,
            })
            .expect("health check should return status");

        assert_eq!(result.status, ProviderHealthStatus::Healthy);
    }

    #[test]
    fn runtime_state_marks_recent_progress_as_answering() {
        let config_dir = test_temp_dir("runtime-answering").join(".claude");
        let session_path = config_dir.join("projects/demo/session-runtime.jsonl");
        let now = now_unix_millis();

        write_owned_lines(
            &session_path,
            &[
                format!(
                    r#"{{"sessionId":"session-runtime","cwd":"/workspace/demo","timestamp":{},"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"hello"}}]}}}}"#,
                    now - 5_000
                ),
                format!(
                    r#"{{"sessionId":"session-runtime","cwd":"/workspace/demo","timestamp":{},"type":"progress","data":{{"type":"agent_progress","message":{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"Bash","input":{{"command":"ls"}}}}]}}}}}}}}"#,
                    now - 1_500
                ),
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let state = adapter
            .get_thread_runtime_state("session-runtime")
            .expect("runtime state should be readable");

        assert!(state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_tool"));
    }

    #[test]
    fn runtime_state_marks_recent_assistant_text_as_not_answering() {
        let config_dir = test_temp_dir("runtime-idle-text").join(".claude");
        let session_path = config_dir.join("projects/demo/session-runtime-idle.jsonl");
        let now = now_unix_millis();

        write_owned_lines(
            &session_path,
            &[
                format!(
                    r#"{{"sessionId":"session-runtime-idle","cwd":"/workspace/demo","timestamp":{},"type":"progress","data":{{"type":"agent_progress","message":{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"README.md"}}}}]}}}}}}}}"#,
                    now - 4_000
                ),
                format!(
                    r#"{{"sessionId":"session-runtime-idle","cwd":"/workspace/demo","timestamp":{},"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"done"}}]}}}}"#,
                    now - 1_000
                ),
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let state = adapter
            .get_thread_runtime_state("session-runtime-idle")
            .expect("runtime state should be readable");

        assert!(!state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_message"));
    }

    #[test]
    fn runtime_state_marks_old_progress_as_not_answering() {
        let config_dir = test_temp_dir("runtime-old-progress").join(".claude");
        let session_path = config_dir.join("projects/demo/session-runtime-old.jsonl");
        let now = now_unix_millis();

        write_owned_lines(
            &session_path,
            &[format!(
                r#"{{"sessionId":"session-runtime-old","cwd":"/workspace/demo","timestamp":{},"type":"progress","data":{{"type":"bash_progress","output":"running..."}}}}"#,
                now - 300_000
            )],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(&config_dir);
        let state = adapter
            .get_thread_runtime_state("session-runtime-old")
            .expect("runtime state should be readable");

        assert!(!state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_tool"));
    }

    #[test]
    fn parse_timestamp_ms_supports_rfc3339() {
        let value: Value = serde_json::from_str(r#"{"timestamp":"2026-02-12T10:00:00.000Z"}"#)
            .expect("json should parse");
        assert!(parse_timestamp_ms(&value).is_some());
    }
}
