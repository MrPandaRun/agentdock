use provider_contract::{
    ProviderAdapter, ProviderError, ProviderErrorCode, ProviderHealthCheckRequest,
    ProviderHealthCheckResult, ProviderHealthStatus, ProviderId, ProviderResult,
    ResumeThreadRequest, ResumeThreadResult, SwitchContextSummary, ThreadSummary,
};
use serde_json::Value;
use std::cmp::Reverse;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CLAUDE_CONFIG_DIR_ENV: &str = "AGENTDOCK_CLAUDE_CONFIG_DIR";
const CLAUDE_BINARY_ENV: &str = "AGENTDOCK_CLAUDE_BIN";
const MESSAGE_KIND_TEXT: &str = "text";
const MESSAGE_KIND_TOOL: &str = "tool";

#[derive(Debug, Clone)]
struct ThreadRecord {
    summary: ThreadSummary,
    source_path: PathBuf,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeThreadMessage {
    pub role: String,
    pub content: String,
    pub timestamp_ms: Option<i64>,
    pub kind: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeThreadOverview {
    pub summary: ThreadSummary,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeSendMessageResult {
    pub thread_id: String,
    pub response_text: String,
    pub raw_output: String,
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

    pub fn get_thread_messages(&self, thread_id: &str) -> ProviderResult<Vec<ClaudeThreadMessage>> {
        let thread_record = self.find_thread_record(thread_id)?;
        let messages = load_thread_messages(&thread_record.source_path);
        Ok(messages
            .into_iter()
            .map(|message| ClaudeThreadMessage {
                role: message.role,
                content: message.content,
                timestamp_ms: message.timestamp_ms,
                kind: message.kind,
                collapsed: message.collapsed,
            })
            .collect())
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

    pub fn send_message(
        &self,
        thread_id: &str,
        prompt: &str,
        project_path: Option<&str>,
    ) -> ProviderResult<ClaudeSendMessageResult> {
        self.ensure_cli_reachable()?;

        let trimmed_prompt = prompt.trim();
        if trimmed_prompt.is_empty() {
            return Err(provider_error(
                ProviderErrorCode::InvalidResponse,
                "Prompt cannot be empty".to_string(),
                false,
            ));
        }

        let thread_record = self.find_thread_record(thread_id)?;
        let cwd = project_path
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(ToString::to_string)
            .or_else(|| {
                if thread_record.summary.project_path == "." {
                    None
                } else {
                    Some(thread_record.summary.project_path.clone())
                }
            });

        let binary = self.claude_binary();
        let mut command = Command::new(&binary);
        command
            .arg("--print")
            .arg("--output-format")
            .arg("json")
            .arg("--resume")
            .arg(thread_id)
            .arg(trimmed_prompt);

        if let Some(dir) = &cwd {
            if !Path::new(dir).exists() {
                // If the original project path no longer exists, fallback to current process cwd.
                command.current_dir(".");
            } else {
                command.current_dir(dir);
            }
        }

        let output = command.output().map_err(|error| {
            provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Failed to execute Claude Code CLI ({binary}): {error}"),
                true,
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "Unknown Claude CLI failure".to_string()
            };
            return Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Claude Code CLI failed for thread {thread_id}: {detail}"),
                true,
            ));
        }

        let raw_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let response_text = extract_claude_print_response(&raw_output);

        Ok(ClaudeSendMessageResult {
            thread_id: thread_id.to_string(),
            response_text,
            raw_output,
        })
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

        let mut records = Vec::new();
        for path in files {
            if let Some(record) = parse_thread_file(&path) {
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

    fn summarize_switch_context(&self, thread_id: &str) -> ProviderResult<SwitchContextSummary> {
        let thread_record = self.find_thread_record(thread_id)?;
        let messages = load_thread_messages(&thread_record.source_path);

        let first_user_message = messages.iter().find(|msg| msg.role == "user");
        let latest_user_message = messages.iter().rev().find(|msg| msg.role == "user");

        let objective = first_user_message
            .map(|msg| truncate_text(&msg.content, 180))
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| format!("Continue Claude thread {thread_id}"));

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
            .unwrap_or_else(|| vec![format!("Resume Claude thread {thread_id}")]);

        Ok(SwitchContextSummary {
            objective,
            constraints,
            pending_tasks,
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

fn parse_thread_file(path: &Path) -> Option<ThreadRecord> {
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
    let title = path_basename(&project_path)
        .map(ToString::to_string)
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
            Some((trimmed.to_string(), 0))
        }
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

fn file_last_modified_ms(path: &Path) -> Option<i64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(duration.as_millis() as i64)
}

fn path_basename(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

fn load_thread_messages(path: &Path) -> Vec<MessageRecord> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

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

        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let timestamp_ms = parse_timestamp_ms(&parsed);
        let content = match message.get("content") {
            Some(content) => content,
            None => continue,
        };

        let records = extract_message_records(&role, content, timestamp_ms);
        messages.extend(records);
    }

    messages
}

fn parse_timestamp_ms(value: &Value) -> Option<i64> {
    let (_, timestamp_ms) = extract_timestamp(value)?;
    if timestamp_ms > 0 {
        Some(timestamp_ms)
    } else {
        None
    }
}

fn extract_message_records(
    role: &str,
    value: &Value,
    timestamp_ms: Option<i64>,
) -> Vec<MessageRecord> {
    match value {
        Value::String(text) => sanitize_cli_markup_text(text)
            .map(|content| vec![text_record(role, content, timestamp_ms)])
            .unwrap_or_default(),
        Value::Array(items) => {
            let mut records = Vec::new();
            for item in items {
                records.extend(extract_message_records_from_block(role, item, timestamp_ms));
            }
            records
        }
        Value::Object(_) => extract_message_records_from_block(role, value, timestamp_ms),
        _ => Vec::new(),
    }
}

fn extract_message_records_from_block(
    role: &str,
    value: &Value,
    timestamp_ms: Option<i64>,
) -> Vec<MessageRecord> {
    match value {
        Value::String(text) => sanitize_cli_markup_text(text)
            .map(|content| vec![text_record(role, content, timestamp_ms)])
            .unwrap_or_default(),
        Value::Object(object) => {
            let block_type = object.get("type").and_then(Value::as_str);
            match block_type {
                Some("thinking") | Some("redacted_thinking") => Vec::new(),
                Some("text") => object
                    .get("text")
                    .and_then(Value::as_str)
                    .and_then(sanitize_cli_markup_text)
                    .map(|content| vec![text_record(role, content, timestamp_ms)])
                    .unwrap_or_default(),
                Some("tool_use") => summarize_tool_use_block(object)
                    .map(|content| vec![tool_record(role, content, timestamp_ms)])
                    .unwrap_or_default(),
                Some("tool_result") => summarize_tool_result_block(object)
                    .map(|content| vec![tool_record(role, content, timestamp_ms)])
                    .unwrap_or_default(),
                Some("server_tool_use") => summarize_server_tool_use_block(object)
                    .map(|content| vec![tool_record(role, content, timestamp_ms)])
                    .unwrap_or_default(),
                _ => {
                    if let Some(text) = object.get("text").and_then(Value::as_str) {
                        if let Some(content) = sanitize_cli_markup_text(text) {
                            return vec![text_record(role, content, timestamp_ms)];
                        }
                    }
                    if let Some(nested) = object.get("content") {
                        return extract_message_records(role, nested, timestamp_ms);
                    }
                    Vec::new()
                }
            }
        }
        Value::Array(items) => {
            let mut records = Vec::new();
            for item in items {
                records.extend(extract_message_records_from_block(role, item, timestamp_ms));
            }
            records
        }
        _ => Vec::new(),
    }
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

fn summarize_tool_use_block(object: &serde_json::Map<String, Value>) -> Option<String> {
    let tool_name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Tool");
    let detail = object.get("input").and_then(extract_tool_input_summary);
    if let Some(detail) = detail {
        return Some(format!("{tool_name} {detail}"));
    }
    Some(tool_name.to_string())
}

fn summarize_tool_result_block(object: &serde_json::Map<String, Value>) -> Option<String> {
    let is_error = object
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content = object.get("content").map(extract_text).unwrap_or_default();
    let normalized = normalize_tool_result_text(&content)?;
    if is_error {
        return Some(format!("Tool error\n{normalized}"));
    }
    Some(normalized)
}

fn summarize_server_tool_use_block(object: &serde_json::Map<String, Value>) -> Option<String> {
    let tool_name = object
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("ServerTool");
    let detail = object
        .get("input")
        .and_then(extract_tool_input_summary)
        .or_else(|| {
            object
                .get("text")
                .and_then(Value::as_str)
                .map(normalize_inline_text)
        });
    if let Some(detail) = detail {
        return Some(format!("{tool_name} {detail}"));
    }
    Some(tool_name.to_string())
}

fn extract_tool_input_summary(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
                return Some(format!("pattern: \"{}\"", pattern.trim()));
            }
            let description = object
                .get("description")
                .and_then(Value::as_str)
                .map(normalize_inline_text);
            if let Some(command) = object.get("command").and_then(Value::as_str) {
                let command_line = truncate_text(&normalize_inline_text(command), 220);
                if let Some(description) = description {
                    return Some(format!("{description}\nIN {command_line}"));
                }
                return Some(format!("IN {command_line}"));
            }
            if let Some(description) = description {
                return Some(description);
            }
            if let Some(path) = object.get("path").and_then(Value::as_str) {
                return Some(format!("path: {}", truncate_text(path.trim(), 80)));
            }
            if let Some(query) = object.get("query").and_then(Value::as_str) {
                return Some(format!("query: {}", truncate_text(query.trim(), 100)));
            }
            if let Some(url) = object.get("url").and_then(Value::as_str) {
                return Some(format!("url: {}", truncate_text(url.trim(), 100)));
            }
            None
        }
        Value::String(text) => Some(truncate_text(&normalize_inline_text(text), 100)),
        _ => None,
    }
}

fn normalize_tool_result_text(raw: &str) -> Option<String> {
    let cleaned = strip_ansi_escapes(raw);
    let lines = cleaned
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .map(|line| truncate_text(line.trim(), 220))
        .collect::<Vec<String>>();

    if lines.is_empty() {
        return None;
    }

    if lines.len() == 1 {
        return Some(format!("OUT {}", lines[0]));
    }

    Some(format!("OUT\n{}", lines.join("\n")))
}

fn normalize_inline_text(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
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

fn sanitize_cli_markup_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains("<local-command-") {
        return None;
    }

    if let Some(command_name) = extract_tag_content(trimmed, "command-name") {
        if !command_name.trim().is_empty() {
            return Some(command_name);
        }
    }

    if let Some(command_message) = extract_tag_content(trimmed, "command-message") {
        if !command_message.trim().is_empty() {
            return Some(command_message);
        }
    }

    if trimmed.contains("<command-args>") {
        return None;
    }

    Some(trimmed.to_string())
}

fn extract_tag_content(input: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{tag}>");
    let close_tag = format!("</{tag}>");
    let start = input.find(&open_tag)? + open_tag.len();
    let end = input[start..].find(&close_tag)? + start;
    let content = input[start..end].trim();
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

fn extract_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_string(),
        Value::Array(items) => items
            .iter()
            .map(extract_text)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<String>>()
            .join("\n"),
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return text.to_string();
            }
            if let Some(content) = object.get("content") {
                return extract_text(content);
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn extract_claude_print_response(stdout: &str) -> String {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(text) = parse_claude_response_json(trimmed) {
        return text;
    }

    for line in trimmed.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(text) = parse_claude_response_json(line) {
            return text;
        }
    }

    trimmed.to_string()
}

fn parse_claude_response_json(input: &str) -> Option<String> {
    let value: Value = serde_json::from_str(input).ok()?;

    if let Some(text) = value.get("result").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    if let Some(output) = value.get("output") {
        let text = extract_text(output);
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    if let Some(message) = value.get("message") {
        if let Some(content) = message.get("content") {
            let text = extract_text(content);
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
        let text = extract_text(message);
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    let text = extract_text(&value);
    if !text.trim().is_empty() {
        return Some(text);
    }

    None
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

fn build_last_message_preview(path: &Path) -> Option<String> {
    let messages = load_thread_messages(path);
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

    let normalized = normalize_inline_text(&message.content);
    if normalized.is_empty() {
        return None;
    }
    Some(truncate_text(&normalized, 140))
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
    fn summarize_switch_context_uses_thread_messages() {
        let config_dir = test_temp_dir("summary").join(".claude");
        let session_path = config_dir.join("projects/demo/session-ctx.jsonl");

        write_lines(
            &session_path,
            &[
                r#"{"sessionId":"session-ctx","cwd":"/workspace/demo","timestamp":"1700000000000","isMeta":true}"#,
                r#"{"sessionId":"session-ctx","cwd":"/workspace/demo","timestamp":"1700000000100","message":{"role":"user","content":"Build a config parser for Claude settings."}}"#,
                r#"{"sessionId":"session-ctx","cwd":"/workspace/demo","timestamp":"1700000000200","message":{"role":"assistant","content":"I will inspect settings.json and fallback to claude.json."}}"#,
                r#"{"sessionId":"session-ctx","cwd":"/workspace/demo","timestamp":"1700000000300","message":{"role":"user","content":"Add recursive session scan for projects dir."}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(config_dir);
        let summary = adapter
            .summarize_switch_context("session-ctx")
            .expect("summary should be generated");

        assert!(summary.objective.contains("Build a config parser"));
        assert!(summary.pending_tasks[0].contains("recursive session scan"));
        assert!(summary
            .constraints
            .iter()
            .any(|constraint| constraint.contains("/workspace/demo")));
    }

    #[test]
    fn get_thread_messages_returns_timestamp_and_content() {
        let config_dir = test_temp_dir("thread-messages").join(".claude");
        let session_path = config_dir.join("projects/demo/session-msg.jsonl");

        write_lines(
            &session_path,
            &[
                r#"{"sessionId":"session-msg","cwd":"/workspace/demo","timestamp":"1700000000000","isMeta":true}"#,
                r#"{"sessionId":"session-msg","cwd":"/workspace/demo","timestamp":"1700000000100","message":{"role":"user","content":"Hello Claude"}}"#,
                r#"{"sessionId":"session-msg","cwd":"/workspace/demo","timestamp":"1700000000200","message":{"role":"assistant","content":"Hi there"}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(config_dir);
        let messages = adapter
            .get_thread_messages("session-msg")
            .expect("messages should be loaded");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].timestamp_ms, Some(1_700_000_000_100));
        assert_eq!(messages[0].kind, MESSAGE_KIND_TEXT);
        assert!(!messages[0].collapsed);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].kind, MESSAGE_KIND_TEXT);
    }

    #[test]
    fn get_thread_messages_extracts_tool_events_and_hides_local_command_payloads() {
        let config_dir = test_temp_dir("thread-filter").join(".claude");
        let session_path = config_dir.join("projects/demo/session-filter.jsonl");

        write_lines(
            &session_path,
            &[
                r#"{"sessionId":"session-filter","cwd":"/workspace/demo","timestamp":"1700000000000","message":{"role":"user","content":"<command-message>init</command-message>\n<command-name>/init</command-name>"}}"#,
                r#"{"sessionId":"session-filter","cwd":"/workspace/demo","timestamp":"1700000000100","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#,
                r#"{"sessionId":"session-filter","cwd":"/workspace/demo","timestamp":"1700000000200","message":{"role":"user","content":[{"type":"tool_result","content":"long command output"}]}}"#,
                r#"{"sessionId":"session-filter","cwd":"/workspace/demo","timestamp":"1700000000300","message":{"role":"user","content":"<local-command-stdout>hidden output</local-command-stdout>"}}"#,
                r#"{"sessionId":"session-filter","cwd":"/workspace/demo","timestamp":"1700000000400","message":{"role":"assistant","content":[{"type":"text","text":"Visible assistant answer"}]}}"#,
            ],
        );

        let adapter = ClaudeAdapter::new().with_config_dir(config_dir);
        let messages = adapter
            .get_thread_messages("session-filter")
            .expect("messages should be loaded");

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "/init");
        assert_eq!(messages[0].kind, MESSAGE_KIND_TEXT);
        assert_eq!(messages[1].kind, MESSAGE_KIND_TOOL);
        assert!(messages[1].collapsed);
        assert!(messages[1].content.contains("Bash"));
        assert!(messages[1].content.contains("IN ls"));
        assert_eq!(messages[2].kind, MESSAGE_KIND_TOOL);
        assert!(messages[2].content.contains("OUT"));
        assert!(messages[2].content.contains("long command output"));
        assert_eq!(messages[3].role, "assistant");
        assert_eq!(messages[3].content, "Visible assistant answer");
    }

    #[test]
    fn sanitize_cli_markup_text_extracts_command_name() {
        let raw = "<command-message>init</command-message>\n<command-name>/init</command-name>";
        assert_eq!(sanitize_cli_markup_text(raw), Some("/init".to_string()));
    }

    #[test]
    fn normalize_tool_result_text_preserves_multiline_preview() {
        let raw = "line one\nline two\nline three";
        let output = normalize_tool_result_text(raw).expect("preview should exist");
        assert!(output.starts_with("OUT\n"));
        assert!(output.contains("line two"));
    }

    #[test]
    fn parse_claude_print_response_handles_json_result() {
        let output = r#"{"type":"result","result":"Done"}"#;
        assert_eq!(extract_claude_print_response(output), "Done");
    }

    #[test]
    fn parse_claude_print_response_falls_back_to_plain_text() {
        let output = "Plain response line";
        assert_eq!(extract_claude_print_response(output), "Plain response line");
    }
}
