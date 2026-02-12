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
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const CODEX_HOME_DIR_ENV: &str = "AGENTDOCK_CODEX_HOME_DIR";
const MESSAGE_KIND_TEXT: &str = "text";
const MESSAGE_KIND_TOOL: &str = "tool";
const CODEX_AGENT_ACTIVITY_WINDOW_MS: i64 = 120_000;

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
pub struct CodexThreadMessage {
    pub role: String,
    pub content: String,
    pub timestamp_ms: Option<i64>,
    pub kind: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexThreadOverview {
    pub summary: ThreadSummary,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexThreadRuntimeState {
    pub agent_answering: bool,
    pub last_event_kind: Option<String>,
    pub last_event_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct CodexAdapter {
    home_dir_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexSemanticEventKind {
    UserMessage,
    AgentReasoning,
    AgentTool,
    AgentMessage,
    TurnAborted,
}

impl CodexSemanticEventKind {
    fn as_str(self) -> &'static str {
        match self {
            CodexSemanticEventKind::UserMessage => "user_message",
            CodexSemanticEventKind::AgentReasoning => "agent_reasoning",
            CodexSemanticEventKind::AgentTool => "agent_tool",
            CodexSemanticEventKind::AgentMessage => "agent_message",
            CodexSemanticEventKind::TurnAborted => "turn_aborted",
        }
    }
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_home_dir<P: Into<PathBuf>>(mut self, home_dir: P) -> Self {
        self.home_dir_override = Some(home_dir.into());
        self
    }

    pub fn get_thread_messages(&self, thread_id: &str) -> ProviderResult<Vec<CodexThreadMessage>> {
        let thread_record = self.find_thread_record(thread_id)?;
        let messages = load_thread_messages(&thread_record.source_path);
        Ok(messages
            .into_iter()
            .map(|message| CodexThreadMessage {
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
    ) -> ProviderResult<CodexThreadRuntimeState> {
        let thread_record = self.find_thread_record(thread_id)?;
        Ok(load_thread_runtime_state(&thread_record.source_path))
    }

    pub fn list_thread_overviews(
        &self,
        project_path: Option<&str>,
    ) -> ProviderResult<Vec<CodexThreadOverview>> {
        let mut records = self.scan_thread_records();

        if let Some(filter) = project_path {
            records.retain(|record| record.summary.project_path.starts_with(filter));
        }

        records.sort_by_key(|record| Reverse(record.sort_key));
        Ok(records
            .into_iter()
            .map(|record| CodexThreadOverview {
                last_message_preview: build_last_message_preview(&record.source_path),
                summary: record.summary,
            })
            .collect())
    }

    fn codex_home_dir(&self) -> PathBuf {
        if let Some(path) = &self.home_dir_override {
            return path.clone();
        }

        if let Ok(path) = std::env::var(CODEX_HOME_DIR_ENV) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }

        if let Some(home) = default_home_dir() {
            return home.join(".codex");
        }

        PathBuf::from(".codex")
    }

    fn codex_sessions_dir(&self) -> PathBuf {
        self.codex_home_dir().join("sessions")
    }

    fn scan_thread_records(&self) -> Vec<ThreadRecord> {
        let mut files = Vec::new();
        collect_jsonl_files(&self.codex_sessions_dir(), &mut files);

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
                    format!("Codex thread not found: {thread_id}"),
                    false,
                )
            })
    }

    fn ensure_cli_reachable(&self) -> ProviderResult<()> {
        match Command::new("codex").arg("--version").output() {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                "Codex CLI not found in PATH: codex".to_string(),
                false,
            )),
            Err(error) => Err(provider_error(
                ProviderErrorCode::UpstreamUnavailable,
                format!("Failed to execute Codex CLI (codex): {error}"),
                true,
            )),
        }
    }
}

impl ProviderAdapter for CodexAdapter {
    fn provider_id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn health_check(
        &self,
        request: ProviderHealthCheckRequest,
    ) -> ProviderResult<ProviderHealthCheckResult> {
        let checked_at = now_unix_millis().to_string();

        match Command::new("codex").arg("--version").output() {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProviderHealthCheckResult {
                    provider_id: ProviderId::Codex,
                    status: ProviderHealthStatus::Offline,
                    checked_at,
                    message: Some("Codex CLI not found in PATH: codex".to_string()),
                });
            }
            Err(error) => {
                return Err(provider_error(
                    ProviderErrorCode::UpstreamUnavailable,
                    format!("Failed to execute Codex CLI (codex): {error}"),
                    true,
                ));
            }
        }

        let sessions_dir = self.codex_sessions_dir();
        if !sessions_dir.exists() {
            return Ok(ProviderHealthCheckResult {
                provider_id: ProviderId::Codex,
                status: ProviderHealthStatus::Degraded,
                checked_at,
                message: Some(format!(
                    "Codex sessions directory not found at {} (profile={})",
                    sessions_dir.display(),
                    request.profile_name
                )),
            });
        }

        Ok(ProviderHealthCheckResult {
            provider_id: ProviderId::Codex,
            status: ProviderHealthStatus::Healthy,
            checked_at,
            message: Some(format!(
                "Codex CLI reachable, sessions directory loaded ({})",
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

        let mut command = format!("codex resume {}", shell_quote(&request.thread_id));
        if let Some(path) = project_path {
            command = format!("cd {} && {command}", shell_quote(&path));
        }

        Ok(ResumeThreadResult {
            thread_id: request.thread_id,
            resumed: true,
            message: Some(format!(
                "Codex thread is resumable. Run command in terminal: {command}"
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
            .unwrap_or_else(|| format!("Continue Codex thread {thread_id}"));

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
            .unwrap_or_else(|| vec![format!("Resume Codex thread {thread_id}")]);

        Ok(SwitchContextSummary {
            objective,
            constraints,
            pending_tasks,
        })
    }
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
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id: Option<String> = None;
    let mut project_path: Option<String> = None;
    let mut last_active_at: Option<String> = None;
    let mut sort_key = file_last_modified_ms(path).unwrap_or(0);

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if parsed.get("type").and_then(Value::as_str) == Some("session_meta") {
            if let Some(payload) = parsed.get("payload") {
                if session_id.is_none() {
                    session_id = payload
                        .get("id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string);
                }
                if project_path.is_none() {
                    project_path = payload
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(ToString::to_string);
                }
            }
        }

        if let Some(timestamp_ms) = parse_timestamp_ms(parsed.get("timestamp")) {
            last_active_at = Some(timestamp_ms.to_string());
            sort_key = sort_key.max(timestamp_ms);
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
        .unwrap_or_else(|| format!("Codex session {}", truncate_text(&session_id, 8)));

    let summary = ThreadSummary {
        id: session_id,
        provider_id: ProviderId::Codex,
        account_id: None,
        project_path,
        title,
        tags: vec!["codex".to_string()],
        last_active_at: last_active_at.unwrap_or_else(|| now_unix_millis().to_string()),
    };

    Some(ThreadRecord {
        summary,
        source_path: path.to_path_buf(),
        sort_key,
    })
}

fn parse_timestamp_ms(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    match value {
        Value::Number(number) => {
            let raw = number.as_i64()?;
            Some(normalize_epoch(raw))
        }
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(numeric) = trimmed.parse::<i64>() {
                return Some(normalize_epoch(numeric));
            }
            parse_rfc3339_timestamp_ms(trimmed)
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

        let timestamp_ms = parse_timestamp_ms(parsed.get("timestamp"));
        if parsed.get("type").and_then(Value::as_str) == Some("response_item") {
            if let Some(payload) = parsed.get("payload") {
                messages.extend(extract_response_item_messages(payload, timestamp_ms));
            }
        }
    }

    messages
}

fn load_thread_runtime_state(path: &Path) -> CodexThreadRuntimeState {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return CodexThreadRuntimeState {
                agent_answering: false,
                last_event_kind: None,
                last_event_at_ms: None,
            };
        }
    };
    let reader = BufReader::new(file);
    let mut last_kind: Option<CodexSemanticEventKind> = None;
    let mut last_event_at_ms: Option<i64> = None;

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if let Some(kind) = extract_semantic_event_kind(&parsed) {
            last_kind = Some(kind);
            if let Some(timestamp_ms) = parse_timestamp_ms(parsed.get("timestamp")) {
                last_event_at_ms = Some(timestamp_ms);
            }
        }
    }

    let is_recent = last_event_at_ms
        .map(|timestamp_ms| {
            now_unix_millis().saturating_sub(timestamp_ms) <= CODEX_AGENT_ACTIVITY_WINDOW_MS
        })
        .unwrap_or(false);
    let agent_answering = is_recent
        && matches!(
            last_kind,
            Some(CodexSemanticEventKind::AgentReasoning | CodexSemanticEventKind::AgentTool)
        );

    CodexThreadRuntimeState {
        agent_answering,
        last_event_kind: last_kind.map(|kind| kind.as_str().to_string()),
        last_event_at_ms,
    }
}

fn extract_semantic_event_kind(record: &Value) -> Option<CodexSemanticEventKind> {
    let record_type = record.get("type").and_then(Value::as_str)?;
    match record_type {
        "event_msg" => extract_semantic_event_kind_from_event_msg(record.get("payload")?),
        "response_item" => extract_semantic_event_kind_from_response_item(record.get("payload")?),
        _ => None,
    }
}

fn extract_semantic_event_kind_from_event_msg(payload: &Value) -> Option<CodexSemanticEventKind> {
    let event_type = payload.get("type").and_then(Value::as_str)?;
    match event_type {
        "user_message" => Some(CodexSemanticEventKind::UserMessage),
        "agent_reasoning" => Some(CodexSemanticEventKind::AgentReasoning),
        "agent_message" => Some(CodexSemanticEventKind::AgentMessage),
        "turn_aborted" => Some(CodexSemanticEventKind::TurnAborted),
        _ => None,
    }
}

fn extract_semantic_event_kind_from_response_item(
    payload: &Value,
) -> Option<CodexSemanticEventKind> {
    let item_type = payload.get("type").and_then(Value::as_str)?;
    match item_type {
        "reasoning" => Some(CodexSemanticEventKind::AgentReasoning),
        "function_call"
        | "function_call_output"
        | "custom_tool_call"
        | "custom_tool_call_output" => Some(CodexSemanticEventKind::AgentTool),
        "message" => {
            let role = payload
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("assistant");
            if role == "user" {
                Some(CodexSemanticEventKind::UserMessage)
            } else {
                Some(CodexSemanticEventKind::AgentMessage)
            }
        }
        _ => None,
    }
}

fn extract_response_item_messages(
    payload: &Value,
    timestamp_ms: Option<i64>,
) -> Vec<MessageRecord> {
    let item_type = payload.get("type").and_then(Value::as_str);
    match item_type {
        Some("message") => {
            let role = payload
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("assistant");
            let text = payload
                .get("content")
                .and_then(extract_codex_message_text)
                .and_then(sanitize_codex_text);

            text.map(|content| vec![text_record(role, content, timestamp_ms)])
                .unwrap_or_default()
        }
        Some("function_call") => {
            let name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Tool");
            let arguments = payload
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("");
            let content = summarize_function_call(name, arguments);
            vec![tool_record("assistant", content, timestamp_ms)]
        }
        Some("function_call_output") => {
            let output = payload.get("output").and_then(Value::as_str).unwrap_or("");
            summarize_function_output(output)
                .map(|content| vec![tool_record("assistant", content, timestamp_ms)])
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn extract_codex_message_text(value: &Value) -> Option<String> {
    match value {
        Value::Array(items) => {
            let mut chunks = Vec::new();
            for item in items {
                if let Some(text) = extract_codex_content_text(item) {
                    if !text.trim().is_empty() {
                        chunks.push(text);
                    }
                }
            }

            if chunks.is_empty() {
                None
            } else {
                Some(chunks.join("\n\n"))
            }
        }
        Value::String(text) => Some(text.to_string()),
        Value::Object(_) => extract_codex_content_text(value),
        _ => None,
    }
}

fn extract_codex_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return Some(text.to_string());
            }
            if let Some(nested) = object.get("content") {
                return extract_codex_message_text(nested);
            }
            None
        }
        Value::Array(items) => {
            let mut chunks = Vec::new();
            for item in items {
                if let Some(text) = extract_codex_content_text(item) {
                    if !text.trim().is_empty() {
                        chunks.push(text);
                    }
                }
            }
            if chunks.is_empty() {
                None
            } else {
                Some(chunks.join("\n"))
            }
        }
        _ => None,
    }
}

fn sanitize_codex_text(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("<user_instructions>") || trimmed.starts_with("<environment_context>") {
        return None;
    }

    Some(trimmed.to_string())
}

fn summarize_function_call(name: &str, arguments: &str) -> String {
    if name == "shell" {
        if let Ok(parsed) = serde_json::from_str::<Value>(arguments) {
            if let Some(command) = parsed.get("command") {
                let rendered = render_command(command);
                if !rendered.is_empty() {
                    return format!("Shell\nIN {}", truncate_text(&rendered, 260));
                }
            }
        }
    }

    let normalized = arguments
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    if normalized.is_empty() {
        return name.to_string();
    }

    format!("{}\nIN {}", name, truncate_text(&normalized, 260))
}

fn summarize_function_output(raw: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(raw).ok();
    let output = parsed
        .as_ref()
        .and_then(|value| value.get("output"))
        .and_then(Value::as_str)
        .unwrap_or(raw);

    let cleaned = strip_ansi_escapes(output);
    let lines = cleaned
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .map(|line| truncate_text(line.trim(), 240))
        .collect::<Vec<String>>();

    if lines.is_empty() {
        return None;
    }

    if lines.len() == 1 {
        return Some(format!("OUT {}", lines[0]));
    }

    Some(format!("OUT\n{}", lines.join("\n")))
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

fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

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
            "agentdock-provider-codex-{name}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("test temp dir should be creatable");
        dir
    }

    #[test]
    fn list_threads_reads_codex_sessions() {
        let codex_home = test_temp_dir("list-threads").join(".codex");
        let session_file = codex_home
            .join("sessions")
            .join("2026")
            .join("02")
            .join("12")
            .join("session-a.jsonl");

        write_lines(
            &session_file,
            &[
                r#"{"timestamp":"2026-02-12T10:00:00.000Z","type":"session_meta","payload":{"id":"codex-a","cwd":"/workspace/a"}}"#,
                r#"{"timestamp":"2026-02-12T10:00:03.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}"#,
            ],
        );

        let adapter = CodexAdapter::new().with_home_dir(&codex_home);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "codex-a");
        assert_eq!(threads[0].provider_id, ProviderId::Codex);
        assert_eq!(threads[0].project_path, "/workspace/a");
    }

    #[test]
    fn get_thread_messages_extracts_text_and_tool_events() {
        let codex_home = test_temp_dir("thread-messages").join(".codex");
        let session_file = codex_home
            .join("sessions")
            .join("2026")
            .join("02")
            .join("12")
            .join("session-b.jsonl");

        write_lines(
            &session_file,
            &[
                r#"{"timestamp":"2026-02-12T10:00:00.000Z","type":"session_meta","payload":{"id":"codex-b","cwd":"/workspace/b"}}"#,
                r#"{"timestamp":"2026-02-12T10:00:01.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
                r#"{"timestamp":"2026-02-12T10:00:02.000Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"bash\",\"-lc\",\"ls\"]}"}}"#,
                r#"{"timestamp":"2026-02-12T10:00:03.000Z","type":"response_item","payload":{"type":"function_call_output","output":"{\"output\":\"file-a\\nfile-b\"}"}}"#,
                r#"{"timestamp":"2026-02-12T10:00:04.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}"#,
            ],
        );

        let adapter = CodexAdapter::new().with_home_dir(&codex_home);
        let messages = adapter
            .get_thread_messages("codex-b")
            .expect("messages should be loaded");

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].kind, MESSAGE_KIND_TEXT);
        assert_eq!(messages[1].kind, MESSAGE_KIND_TOOL);
        assert!(messages[1].content.contains("Shell"));
        assert!(messages[1].content.contains("IN"));
        assert_eq!(messages[2].kind, MESSAGE_KIND_TOOL);
        assert!(messages[2].content.contains("OUT"));
        assert_eq!(messages[3].role, "assistant");
        assert_eq!(messages[3].content, "done");
    }

    #[test]
    fn runtime_state_marks_recent_agent_reasoning_as_answering() {
        let codex_home = test_temp_dir("runtime-answering").join(".codex");
        let session_file = codex_home
            .join("sessions")
            .join("2026")
            .join("02")
            .join("12")
            .join("session-c.jsonl");

        let now = now_unix_millis();
        let lines = vec![
            format!(
                r#"{{"timestamp":{},"type":"session_meta","payload":{{"id":"codex-c","cwd":"/workspace/c"}}}}"#,
                now - 10_000
            ),
            format!(
                r#"{{"timestamp":{},"type":"event_msg","payload":{{"type":"user_message","message":"hello"}}}}"#,
                now - 9_000
            ),
            format!(
                r#"{{"timestamp":{},"type":"event_msg","payload":{{"type":"agent_reasoning","text":"thinking"}}}}"#,
                now - 2_000
            ),
        ];
        write_owned_lines(&session_file, &lines);

        let adapter = CodexAdapter::new().with_home_dir(&codex_home);
        let state = adapter
            .get_thread_runtime_state("codex-c")
            .expect("runtime state should be readable");

        assert!(state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_reasoning"));
    }

    #[test]
    fn runtime_state_marks_old_agent_activity_as_not_answering() {
        let codex_home = test_temp_dir("runtime-idle").join(".codex");
        let session_file = codex_home
            .join("sessions")
            .join("2026")
            .join("02")
            .join("12")
            .join("session-d.jsonl");

        let now = now_unix_millis();
        let lines = vec![
            format!(
                r#"{{"timestamp":{},"type":"session_meta","payload":{{"id":"codex-d","cwd":"/workspace/d"}}}}"#,
                now - 300_000
            ),
            format!(
                r#"{{"timestamp":{},"type":"event_msg","payload":{{"type":"user_message","message":"hello"}}}}"#,
                now - 280_000
            ),
            format!(
                r#"{{"timestamp":{},"type":"event_msg","payload":{{"type":"agent_reasoning","text":"thinking"}}}}"#,
                now - 260_000
            ),
        ];
        write_owned_lines(&session_file, &lines);

        let adapter = CodexAdapter::new().with_home_dir(&codex_home);
        let state = adapter
            .get_thread_runtime_state("codex-d")
            .expect("runtime state should be readable");

        assert!(!state.agent_answering);
        assert_eq!(state.last_event_kind.as_deref(), Some("agent_reasoning"));
    }
}
