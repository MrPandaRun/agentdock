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

const CODEX_HOME_DIR_ENV: &str = "AGENTDOCK_CODEX_HOME_DIR";
const CODEX_AGENT_ACTIVITY_WINDOW_MS: i64 = 120_000;

#[derive(Debug, Clone)]
struct ThreadRecord {
    summary: ThreadSummary,
    source_path: PathBuf,
    sort_key: i64,
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
        let official_titles = load_codex_thread_titles(&self.codex_home_dir());

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

fn load_codex_thread_titles(codex_home_dir: &Path) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    let state_path = codex_home_dir.join(".codex-global-state.json");
    let raw = match fs::read_to_string(state_path) {
        Ok(raw) => raw,
        Err(_) => return titles,
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(_) => return titles,
    };

    let title_entries = match parsed
        .get("thread-titles")
        .and_then(|value| value.get("titles"))
        .and_then(Value::as_object)
    {
        Some(entries) => entries,
        None => return titles,
    };

    for (thread_id, title_value) in title_entries {
        if let Some(title) = title_value
            .as_str()
            .and_then(non_empty_trimmed)
            .map(ToString::to_string)
        {
            titles.insert(thread_id.to_string(), title);
        }
    }

    titles
}

fn parse_thread_file(path: &Path, official_titles: &HashMap<String, String>) -> Option<ThreadRecord> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id: Option<String> = None;
    let mut project_path: Option<String> = None;
    let mut first_user_title: Option<String> = None;
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

        if first_user_title.is_none() {
            let record_type = parsed.get("type").and_then(Value::as_str);
            match record_type {
                Some("response_item") => {
                    if let Some(payload) = parsed.get("payload") {
                        let item_type = payload.get("type").and_then(Value::as_str);
                        let role = payload
                            .get("role")
                            .and_then(Value::as_str)
                            .unwrap_or("assistant");
                        if item_type == Some("message") && role == "user" {
                            first_user_title = extract_codex_preview_text(payload)
                                .map(|text| truncate_text(&text, 72));
                        }
                    }
                }
                Some("event_msg") => {
                    if let Some(payload) = parsed.get("payload") {
                        if payload.get("type").and_then(Value::as_str) == Some("user_message") {
                            first_user_title = payload
                                .get("message")
                                .and_then(Value::as_str)
                                .and_then(normalize_preview_text)
                                .map(|text| truncate_text(&text, 72));
                        }
                    }
                }
                _ => {}
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
    let title = official_titles
        .get(&session_id)
        .and_then(|title| non_empty_trimmed(title))
        .map(ToString::to_string)
        .or_else(|| first_user_title.filter(|text| !text.is_empty()))
        .or_else(|| path_basename(&project_path).map(ToString::to_string))
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

/// Lightweight last-message preview: scans the JSONL file and extracts the last
/// visible text content from response_item messages without full message parsing.
fn build_last_message_preview(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last_visible_text: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        let parsed: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if parsed.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }

        let payload = match parsed.get("payload") {
            Some(value) => value,
            None => continue,
        };

        let item_type = payload.get("type").and_then(Value::as_str);
        if item_type != Some("message") {
            continue;
        }

        if let Some(text) = extract_codex_preview_text(payload) {
            last_visible_text = Some(text);
        }
    }

    last_visible_text.map(|text| truncate_text(&text, 140))
}

/// Extract visible text from a Codex response_item message payload for preview.
fn extract_codex_preview_text(payload: &Value) -> Option<String> {
    let content = payload.get("content")?;
    match content {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("<user_instructions>")
                || trimmed.starts_with("<environment_context>")
            {
                None
            } else {
                normalize_preview_text(trimmed)
            }
        }
        Value::Array(items) => {
            let mut last_text: Option<String> = None;
            for item in items {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    if let Some(normalized) = normalize_preview_text(text) {
                        last_text = Some(normalized);
                    }
                }
            }
            last_text
        }
        _ => None,
    }
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
        assert_eq!(threads[0].title, "a");
    }

    #[test]
    fn list_threads_prefers_codex_official_title_map() {
        let codex_home = test_temp_dir("title-from-codex-state").join(".codex");
        let session_file = codex_home
            .join("sessions")
            .join("2026")
            .join("02")
            .join("12")
            .join("session-title.jsonl");

        write_lines(
            &session_file,
            &[
                r#"{"timestamp":"2026-02-12T10:00:00.000Z","type":"session_meta","payload":{"id":"codex-title","cwd":"/workspace/title-project"}}"#,
                r#"{"timestamp":"2026-02-12T10:00:01.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"  Implement   unified thread title strategy  "}]}}"#,
                r#"{"timestamp":"2026-02-12T10:00:02.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Working on it"}]}}"#,
                r#"{"timestamp":"2026-02-12T10:00:03.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Second request should not replace title"}]}}"#,
            ],
        );
        fs::write(
            codex_home.join(".codex-global-state.json"),
            r#"{"thread-titles":{"titles":{"codex-title":"Codex 官方标题"}}}"#,
        )
        .expect("global state should be writable");

        let adapter = CodexAdapter::new().with_home_dir(&codex_home);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "codex-title");
        assert_eq!(threads[0].title, "Codex 官方标题");
    }

    #[test]
    fn list_threads_falls_back_to_first_user_message_without_official_title() {
        let codex_home = test_temp_dir("title-fallback-user").join(".codex");
        let session_file = codex_home
            .join("sessions")
            .join("2026")
            .join("02")
            .join("12")
            .join("session-title.jsonl");

        write_lines(
            &session_file,
            &[
                r#"{"timestamp":"2026-02-12T10:00:00.000Z","type":"session_meta","payload":{"id":"codex-title","cwd":"/workspace/title-project"}}"#,
                r#"{"timestamp":"2026-02-12T10:00:01.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"  Implement   unified thread title strategy  "}]}}"#,
                r#"{"timestamp":"2026-02-12T10:00:02.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Working on it"}]}}"#,
            ],
        );

        let adapter = CodexAdapter::new().with_home_dir(&codex_home);
        let threads = adapter
            .list_threads(None)
            .expect("list_threads should work");

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "codex-title");
        assert_eq!(threads[0].title, "Implement unified thread title strategy");
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
