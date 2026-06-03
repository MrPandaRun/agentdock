use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use provider_contract::ProviderId;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::Emitter;

use crate::command_utils::command_available;
use crate::payloads::{
    EmbeddedTerminalExitPayload, EmbeddedTerminalOutputPayload, OpenThreadInTerminalResponse,
    StartEmbeddedTerminalResponse,
};

struct EmbeddedTerminalSession {
    child: Mutex<Box<dyn portable_pty::Child + Send>>,
    stdin: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
}

static EMBEDDED_TERMINAL_SESSIONS: OnceLock<Mutex<HashMap<String, Arc<EmbeddedTerminalSession>>>> =
    OnceLock::new();
static EMBEDDED_TERMINAL_COUNTER: AtomicU64 = AtomicU64::new(1);
const SYNC_OUTPUT_START: &str = "\u{001b}[?2026h";
const SYNC_OUTPUT_END: &str = "\u{001b}[?2026l";

#[derive(Default)]
struct TerminalOutputBatcher {
    carry: String,
    sync_frame: String,
    in_sync_frame: bool,
}

impl TerminalOutputBatcher {
    fn push(&mut self, chunk: &str) -> Vec<String> {
        if chunk.is_empty() {
            return Vec::new();
        }

        let mut source = String::new();
        source.push_str(&self.carry);
        source.push_str(chunk);
        self.carry.clear();

        let mut emitted = Vec::new();
        let mut cursor = 0;

        while cursor < source.len() {
            if self.in_sync_frame {
                if let Some(relative_end) = source[cursor..].find(SYNC_OUTPUT_END) {
                    let end = cursor + relative_end + SYNC_OUTPUT_END.len();
                    self.sync_frame.push_str(&source[cursor..end]);
                    emitted.push(std::mem::take(&mut self.sync_frame));
                    self.in_sync_frame = false;
                    cursor = end;
                    continue;
                }

                let suffix_len = trailing_marker_prefix_len(&source[cursor..], SYNC_OUTPUT_END);
                let safe_end = source.len().saturating_sub(suffix_len);
                self.sync_frame.push_str(&source[cursor..safe_end]);
                self.carry.push_str(&source[safe_end..]);
                return emitted;
            }

            if let Some(relative_start) = source[cursor..].find(SYNC_OUTPUT_START) {
                let start = cursor + relative_start;
                if start > cursor {
                    emitted.push(source[cursor..start].to_string());
                }
                self.in_sync_frame = true;
                self.sync_frame
                    .push_str(&source[start..start + SYNC_OUTPUT_START.len()]);
                cursor = start + SYNC_OUTPUT_START.len();
                continue;
            }

            let remaining = &source[cursor..];
            let suffix_len = trailing_marker_prefix_len(remaining, SYNC_OUTPUT_START)
                .max(trailing_marker_prefix_len(remaining, SYNC_OUTPUT_END));
            let safe_end = source.len().saturating_sub(suffix_len);
            if safe_end > cursor {
                emitted.push(source[cursor..safe_end].to_string());
            }
            self.carry.push_str(&source[safe_end..]);
            return emitted;
        }

        emitted
    }

    fn flush_pending(&mut self) -> Option<String> {
        let mut pending = String::new();

        if self.in_sync_frame {
            pending.push_str(&self.sync_frame);
            self.sync_frame.clear();
            self.in_sync_frame = false;
        }

        if !self.carry.is_empty() {
            pending.push_str(&self.carry);
            self.carry.clear();
        }

        if pending.is_empty() {
            None
        } else {
            Some(pending)
        }
    }
}

fn trailing_marker_prefix_len(source: &str, marker: &str) -> usize {
    let max_len = source.len().min(marker.len().saturating_sub(1));
    for len in (1..=max_len).rev() {
        if source.ends_with(&marker[..len]) {
            return len;
        }
    }
    0
}

fn emit_terminal_output(app: &tauri::AppHandle, session_id: &str, data: String) {
    if data.is_empty() {
        return;
    }
    let payload = EmbeddedTerminalOutputPayload {
        session_id: session_id.to_string(),
        data,
    };
    let _ = app.emit("embedded-terminal-output", payload);
}

pub fn open_thread_in_terminal(
    provider_id: ProviderId,
    thread_id: &str,
    profile_name: Option<&str>,
    env: Option<HashMap<String, String>>,
    project_path: Option<&str>,
) -> Result<OpenThreadInTerminalResponse, String> {
    let command = build_resume_command_from_parts(
        provider_id,
        thread_id,
        profile_name,
        env.as_ref(),
        project_path,
    );
    launch_in_terminal(&command)?;
    Ok(OpenThreadInTerminalResponse {
        launched: true,
        command,
        terminal_app: "Terminal".to_string(),
    })
}

pub fn open_new_thread_in_terminal(
    provider_id: ProviderId,
    profile_name: Option<&str>,
    env: Option<HashMap<String, String>>,
    project_path: Option<&str>,
) -> Result<OpenThreadInTerminalResponse, String> {
    let command =
        build_new_thread_command_from_parts(provider_id, profile_name, env.as_ref(), project_path);
    launch_in_terminal(&command)?;
    Ok(OpenThreadInTerminalResponse {
        launched: true,
        command,
        terminal_app: "Terminal".to_string(),
    })
}

pub fn open_thread_in_happy(
    provider_id: ProviderId,
    thread_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<OpenThreadInTerminalResponse, String> {
    ensure_command_available("happy", "Happy CLI")?;
    let command = build_happy_command_from_parts(provider_id, thread_id, project_path)?;
    launch_in_terminal(&command)?;
    Ok(OpenThreadInTerminalResponse {
        launched: true,
        command,
        terminal_app: "Terminal".to_string(),
    })
}

pub fn is_happy_installed() -> Result<bool, String> {
    Ok(command_available("happy"))
}

pub fn start_embedded_terminal(
    app: tauri::AppHandle,
    provider_id: ProviderId,
    thread_id: &str,
    profile_name: Option<&str>,
    env: Option<HashMap<String, String>>,
    project_path: Option<&str>,
    terminal_theme: Option<&str>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<StartEmbeddedTerminalResponse, String> {
    let cols = clamp_terminal_cols(cols);
    let rows = clamp_terminal_rows(rows);
    let command = build_resume_command_from_parts(
        provider_id,
        thread_id,
        profile_name,
        env.as_ref(),
        project_path,
    );
    let session_id = next_embedded_terminal_session_id();
    let (reader, session) = create_embedded_session(&command, terminal_theme, cols, rows)?;
    terminal_sessions()
        .lock()
        .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?
        .insert(session_id.clone(), Arc::clone(&session));

    spawn_terminal_output_reader(app.clone(), session_id.clone(), reader);
    spawn_terminal_exit_watcher(app, session_id.clone(), session);

    Ok(StartEmbeddedTerminalResponse {
        session_id,
        command,
    })
}

pub fn start_new_embedded_terminal(
    app: tauri::AppHandle,
    provider_id: ProviderId,
    profile_name: Option<&str>,
    env: Option<HashMap<String, String>>,
    project_path: Option<&str>,
    terminal_theme: Option<&str>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<StartEmbeddedTerminalResponse, String> {
    let cols = clamp_terminal_cols(cols);
    let rows = clamp_terminal_rows(rows);
    let command =
        build_new_thread_command_from_parts(provider_id, profile_name, env.as_ref(), project_path);
    let session_id = next_embedded_terminal_session_id();
    let (reader, session) = create_embedded_session(&command, terminal_theme, cols, rows)?;
    terminal_sessions()
        .lock()
        .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?
        .insert(session_id.clone(), Arc::clone(&session));

    spawn_terminal_output_reader(app.clone(), session_id.clone(), reader);
    spawn_terminal_exit_watcher(app, session_id.clone(), session);

    Ok(StartEmbeddedTerminalResponse {
        session_id,
        command,
    })
}

pub fn write_embedded_terminal_input(session_id: &str, data: &str) -> Result<(), String> {
    let session = {
        let sessions = terminal_sessions()
            .lock()
            .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Embedded terminal session not found: {session_id}"))?
    };

    let mut stdin = session
        .stdin
        .lock()
        .map_err(|_| "Embedded terminal stdin lock poisoned".to_string())?;
    stdin
        .write_all(data.as_bytes())
        .map_err(|error| format!("Failed to write terminal input: {error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("Failed to flush terminal input: {error}"))?;
    Ok(())
}

pub fn resize_embedded_terminal(session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
    let session = {
        let sessions = terminal_sessions()
            .lock()
            .map_err(|_| "Embedded terminal sessions lock poisoned".to_string())?;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Embedded terminal session not found: {session_id}"))?
    };

    let cols = clamp_terminal_cols(Some(cols));
    let rows = clamp_terminal_rows(Some(rows));
    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let master = session
        .master
        .lock()
        .map_err(|_| "Embedded terminal master lock poisoned".to_string())?;
    master
        .resize(size)
        .map_err(|error| format!("Failed to resize embedded terminal: {error}"))
}

pub fn close_embedded_terminal(session_id: &str) -> Result<(), String> {
    let session = remove_embedded_terminal_session(session_id);
    if let Some(session) = session {
        let mut child = session
            .child
            .lock()
            .map_err(|_| "Embedded terminal child lock poisoned".to_string())?;
        let _ = child.kill();
    }
    Ok(())
}

pub fn clamp_terminal_cols(value: Option<u16>) -> u16 {
    match value.unwrap_or(120) {
        0..=39 => 120,
        cols => cols.min(320),
    }
}

pub fn clamp_terminal_rows(value: Option<u16>) -> u16 {
    match value.unwrap_or(36) {
        0..=11 => 36,
        rows => rows.min(120),
    }
}

fn terminal_sessions() -> &'static Mutex<HashMap<String, Arc<EmbeddedTerminalSession>>> {
    EMBEDDED_TERMINAL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_embedded_terminal_session_id() -> String {
    let value = EMBEDDED_TERMINAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("embedded-terminal-{value}")
}

fn create_embedded_session(
    command: &str,
    terminal_theme: Option<&str>,
    cols: u16,
    rows: u16,
) -> Result<(Box<dyn Read + Send>, Arc<EmbeddedTerminalSession>), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to allocate PTY: {error}"))?;

    let mut cmd = build_embedded_shell_command(command);
    cmd.env("TERM", "xterm-256color");
    cmd.env("TERM_PROGRAM", embedded_term_program());
    cmd.env("COLORFGBG", colorfgbg_for_theme(terminal_theme));
    cmd.env("COLUMNS", cols.to_string());
    cmd.env("LINES", rows.to_string());
    cmd.env("PI_CLEAR_ON_SHRINK", "1");

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|error| format!("Failed to spawn PTY child process: {error}"))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("Failed to clone PTY reader: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("Failed to capture PTY writer: {error}"))?;

    let session = Arc::new(EmbeddedTerminalSession {
        child: Mutex::new(child),
        stdin: Mutex::new(writer),
        master: Mutex::new(pair.master),
    });
    Ok((reader, session))
}

#[cfg(target_os = "windows")]
fn build_embedded_shell_command(command: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new("cmd.exe");
    cmd.arg("/C");
    cmd.arg(command);
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_embedded_shell_command(command: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new("sh");
    cmd.arg("-lc");
    cmd.arg(command);
    cmd
}

#[cfg(target_os = "windows")]
fn embedded_term_program() -> &'static str {
    "Windows_Command_Prompt"
}

#[cfg(target_os = "macos")]
fn embedded_term_program() -> &'static str {
    "AgentClaw_Embedded"
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn embedded_term_program() -> &'static str {
    "AgentClaw_Embedded"
}

fn colorfgbg_for_theme(terminal_theme: Option<&str>) -> &'static str {
    match terminal_theme {
        Some("light") => "0;15",
        _ => "15;0",
    }
}

fn spawn_terminal_output_reader<R: Read + Send + 'static>(
    app: tauri::AppHandle,
    session_id: String,
    mut stream: R,
) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        let mut pending = Vec::new();
        let mut batcher = TerminalOutputBatcher::default();
        loop {
            let read = match stream.read(&mut buffer) {
                Ok(size) => size,
                Err(_) => break,
            };
            if read == 0 {
                if !pending.is_empty() {
                    let data = String::from_utf8_lossy(&pending).to_string();
                    for chunk in batcher.push(&data) {
                        emit_terminal_output(&app, &session_id, chunk);
                    }
                    pending.clear();
                }
                if let Some(chunk) = batcher.flush_pending() {
                    emit_terminal_output(&app, &session_id, chunk);
                }
                break;
            }

            pending.extend_from_slice(&buffer[..read]);

            loop {
                match std::str::from_utf8(&pending) {
                    Ok(text) => {
                        for chunk in batcher.push(text) {
                            emit_terminal_output(&app, &session_id, chunk);
                        }
                        pending.clear();
                        break;
                    }
                    Err(error) => {
                        let valid_up_to = error.valid_up_to();
                        if valid_up_to > 0 {
                            let valid = &pending[..valid_up_to];
                            let data = String::from_utf8_lossy(valid).to_string();
                            for chunk in batcher.push(&data) {
                                emit_terminal_output(&app, &session_id, chunk);
                            }
                        }

                        match error.error_len() {
                            Some(error_len) => {
                                // True invalid bytes: skip the offending sequence and continue.
                                let drain_to = valid_up_to + error_len;
                                pending.drain(..drain_to);
                                for chunk in batcher.push("\u{FFFD}") {
                                    emit_terminal_output(&app, &session_id, chunk);
                                }
                                if pending.is_empty() {
                                    break;
                                }
                            }
                            None => {
                                // Incomplete UTF-8 sequence at the end; keep remainder for next read.
                                pending.drain(..valid_up_to);
                                break;
                            }
                        }
                    }
                }
            }
        }
    });
}

fn spawn_terminal_exit_watcher(
    app: tauri::AppHandle,
    session_id: String,
    session: Arc<EmbeddedTerminalSession>,
) {
    thread::spawn(move || {
        enum PollStatus {
            Running,
            Exited(Option<i32>),
            Failed,
        }

        let status_code = loop {
            let poll = {
                let mut child = match session.child.lock() {
                    Ok(child) => child,
                    Err(_) => break None,
                };

                match child.try_wait() {
                    Ok(Some(status)) => PollStatus::Exited(Some(status.exit_code() as i32)),
                    Ok(None) => PollStatus::Running,
                    Err(_) => PollStatus::Failed,
                }
            };

            match poll {
                PollStatus::Exited(code) => break code,
                PollStatus::Failed => break None,
                PollStatus::Running => thread::sleep(Duration::from_millis(80)),
            }
        };

        remove_embedded_terminal_session(&session_id);
        let payload = EmbeddedTerminalExitPayload {
            session_id,
            status_code,
        };
        let _ = app.emit("embedded-terminal-exit", payload);
    });
}

fn remove_embedded_terminal_session(session_id: &str) -> Option<Arc<EmbeddedTerminalSession>> {
    terminal_sessions()
        .lock()
        .ok()
        .and_then(|mut sessions| sessions.remove(session_id))
}

fn build_resume_command_from_parts(
    provider_id: ProviderId,
    thread_id: &str,
    profile_name: Option<&str>,
    env: Option<&HashMap<String, String>>,
    project_path: Option<&str>,
) -> String {
    let resume_base = match provider_id {
        ProviderId::ClaudeCode => format!("claude --resume {}", shell_quote(thread_id)),
        ProviderId::Codex => format!(
            "{} resume {}",
            resolve_provider_command(provider_id),
            shell_quote(thread_id)
        ),
        ProviderId::OpenCode => format!("opencode --session {}", shell_quote(thread_id)),
    };
    apply_env_and_profile_to_command(resume_base, env, profile_name, project_path)
}

fn build_new_thread_command_from_parts(
    provider_id: ProviderId,
    profile_name: Option<&str>,
    env: Option<&HashMap<String, String>>,
    project_path: Option<&str>,
) -> String {
    let start_base = resolve_provider_command(provider_id);
    apply_env_and_profile_to_command(start_base, env, profile_name, project_path)
}

fn resolve_provider_command(provider_id: ProviderId) -> String {
    match provider_id {
        ProviderId::ClaudeCode => "claude".to_string(),
        ProviderId::Codex => resolve_codex_command(),
        ProviderId::OpenCode => "opencode".to_string(),
    }
}

fn resolve_codex_command() -> String {
    if command_available("codex") {
        return "codex".to_string();
    }

    #[cfg(target_os = "macos")]
    {
        const CODEX_APP_BINARY: &str = "/Applications/Codex.app/Contents/Resources/codex";
        if command_available(CODEX_APP_BINARY) {
            return shell_quote(CODEX_APP_BINARY);
        }
    }

    "codex".to_string()
}

fn apply_env_and_profile_to_command(
    command: String,
    env: Option<&HashMap<String, String>>,
    profile_name: Option<&str>,
    project_path: Option<&str>,
) -> String {
    let project_path = project_path
        .map(str::trim)
        .filter(|path| !path.is_empty() && *path != ".");

    let mut entries = Vec::new();
    if let Some(env) = env {
        let mut keys = env.keys().collect::<Vec<_>>();
        keys.sort();
        for key in keys {
            let key_trimmed = key.trim();
            if key_trimmed.is_empty() {
                continue;
            }
            if let Some(value) = env.get(key) {
                entries.push((key_trimmed.to_string(), value.to_string()));
            }
        }
    }
    let profile_name = profile_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(profile_name) = profile_name {
        entries.push((
            "AGENTDOCK_ACTIVE_PROFILE".to_string(),
            profile_name.to_string(),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let mut segments = Vec::new();
        if let Some(path) = project_path {
            segments.push(format!("cd /d {}", shell_quote(path)));
        }
        for (key, value) in entries {
            segments.push(format!(
                "set \"{}={}\"",
                escape_cmd_fragment(&key),
                escape_cmd_fragment(&value)
            ));
        }
        segments.push(command);
        return segments.join(" && ");
    }

    #[cfg(not(target_os = "windows"))]
    {
        let with_env = if entries.is_empty() {
            command
        } else {
            let prefix = entries
                .iter()
                .map(|(key, value)| format!("{key}={}", shell_quote(value)))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{prefix} {command}")
        };

        if let Some(path) = project_path {
            format!("cd {} && {with_env}", shell_quote(path))
        } else {
            with_env
        }
    }
}

fn build_happy_command_from_parts(
    provider_id: ProviderId,
    thread_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<String, String> {
    let command = match provider_id {
        ProviderId::ClaudeCode => {
            if let Some(thread_id) = thread_id {
                format!("happy --resume {}", shell_quote(thread_id))
            } else {
                "happy".to_string()
            }
        }
        ProviderId::Codex => {
            if let Some(thread_id) = thread_id {
                format!("happy codex resume {}", shell_quote(thread_id))
            } else {
                "happy codex".to_string()
            }
        }
        ProviderId::OpenCode => {
            return Err(
                "Happy integration currently supports claude_code and codex only".to_string(),
            )
        }
    };

    Ok(apply_env_and_profile_to_command(
        command,
        None,
        None,
        project_path,
    ))
}

fn ensure_command_available(command: &str, command_label: &str) -> Result<(), String> {
    let available = command_available(command);
    if available {
        return Ok(());
    }

    Err(format!(
        "{command_label} is not available in PATH. Install it first, then retry."
    ))
}

#[cfg(target_os = "macos")]
fn launch_in_terminal(command: &str) -> Result<(), String> {
    let escaped = escape_applescript(command);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "tell application \"Terminal\" to do script \"{escaped}\""
        ))
        .arg("-e")
        .arg("tell application \"Terminal\" to activate")
        .output()
        .map_err(|error| format!("Failed to invoke osascript: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!("Failed to launch Terminal with command: {detail}"))
}

#[cfg(target_os = "windows")]
fn launch_in_terminal(command: &str) -> Result<(), String> {
    let output = Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("")
        .arg("cmd")
        .arg("/K")
        .arg(command)
        .output()
        .map_err(|error| format!("Failed to launch Command Prompt: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!("Failed to launch terminal with command: {detail}"))
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn launch_in_terminal(_command: &str) -> Result<(), String> {
    Err("Terminal launch is only supported on macOS and Windows for now".to_string())
}

#[cfg(target_os = "macos")]
fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
fn shell_quote(value: &str) -> String {
    format!("\"{}\"", escape_cmd_fragment(value))
}

#[cfg(not(target_os = "windows"))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "windows")]
fn escape_cmd_fragment(value: &str) -> String {
    value.replace('%', "%%").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use provider_contract::ProviderId;

    use super::{
        build_happy_command_from_parts, build_new_thread_command_from_parts,
        build_resume_command_from_parts, clamp_terminal_cols, clamp_terminal_rows,
        resolve_provider_command, shell_quote, TerminalOutputBatcher,
    };

    #[test]
    fn build_resume_command_quotes_thread_id_and_project_path() {
        let command = build_resume_command_from_parts(
            ProviderId::ClaudeCode,
            "thread id",
            None,
            None,
            Some("/tmp/my project"),
        );
        if cfg!(target_os = "windows") {
            assert_eq!(
                command,
                "cd /d \"/tmp/my project\" && claude --resume \"thread id\""
            );
        } else {
            assert_eq!(
                command,
                "cd '/tmp/my project' && claude --resume 'thread id'"
            );
        }
    }

    #[test]
    fn build_new_thread_command_supports_provider_without_project_path() {
        let command = build_new_thread_command_from_parts(ProviderId::OpenCode, None, None, None);
        assert_eq!(command, "opencode");
    }

    #[test]
    fn resolve_provider_command_finds_codex_command() {
        let command = resolve_provider_command(ProviderId::Codex);
        assert!(command == "codex" || command.contains("Codex.app/Contents/Resources/codex"));
    }

    #[test]
    fn build_resume_command_includes_profile_env_when_provided() {
        let command = build_resume_command_from_parts(
            ProviderId::Codex,
            "thread-id",
            Some("work"),
            None,
            None,
        );
        if cfg!(target_os = "windows") {
            assert_eq!(
                command,
                "set \"AGENTDOCK_ACTIVE_PROFILE=work\" && codex resume \"thread-id\""
            );
        } else {
            assert_eq!(
                command,
                "AGENTDOCK_ACTIVE_PROFILE='work' codex resume 'thread-id'"
            );
        }
    }

    #[test]
    fn build_new_thread_command_includes_profile_env_when_provided() {
        let command =
            build_new_thread_command_from_parts(ProviderId::ClaudeCode, Some("demo"), None, None);
        if cfg!(target_os = "windows") {
            assert_eq!(command, "set \"AGENTDOCK_ACTIVE_PROFILE=demo\" && claude");
        } else {
            assert_eq!(command, "AGENTDOCK_ACTIVE_PROFILE='demo' claude");
        }
    }

    #[test]
    fn build_new_thread_command_includes_custom_env_variables() {
        let mut env = HashMap::new();
        env.insert("OPENAI_API_KEY".to_string(), "sk-test".to_string());
        env.insert(
            "OPENAI_BASE_URL".to_string(),
            "https://proxy.example.com".to_string(),
        );
        let command = build_new_thread_command_from_parts(
            ProviderId::Codex,
            Some("team-profile"),
            Some(&env),
            None,
        );
        if cfg!(target_os = "windows") {
            assert_eq!(
                command,
                "set \"OPENAI_API_KEY=sk-test\" && set \"OPENAI_BASE_URL=https://proxy.example.com\" && set \"AGENTDOCK_ACTIVE_PROFILE=team-profile\" && codex"
            );
        } else {
            assert_eq!(
                command,
                "OPENAI_API_KEY='sk-test' OPENAI_BASE_URL='https://proxy.example.com' AGENTDOCK_ACTIVE_PROFILE='team-profile' codex"
            );
        }
    }

    #[test]
    fn build_happy_resume_command_for_claude() {
        let command = build_happy_command_from_parts(
            ProviderId::ClaudeCode,
            Some("thread-id"),
            Some("/tmp/proj"),
        )
        .expect("happy command should be built");
        if cfg!(target_os = "windows") {
            assert_eq!(
                command,
                "cd /d \"/tmp/proj\" && happy --resume \"thread-id\""
            );
        } else {
            assert_eq!(command, "cd '/tmp/proj' && happy --resume 'thread-id'");
        }
    }

    #[test]
    fn build_happy_new_command_for_codex_without_project_path() {
        let command = build_happy_command_from_parts(ProviderId::Codex, None, None)
            .expect("happy command should be built");
        assert_eq!(command, "happy codex");
    }

    #[test]
    fn build_happy_command_rejects_unsupported_provider() {
        let error = build_happy_command_from_parts(ProviderId::OpenCode, None, None)
            .expect_err("opencode should be rejected");
        assert_eq!(
            error,
            "Happy integration currently supports claude_code and codex only"
        );
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        if cfg!(target_os = "windows") {
            assert_eq!(shell_quote("a'b"), "\"a'b\"");
        } else {
            assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
        }
    }

    #[test]
    fn clamp_terminal_cols_respects_default_and_limits() {
        assert_eq!(clamp_terminal_cols(None), 120);
        assert_eq!(clamp_terminal_cols(Some(10)), 120);
        assert_eq!(clamp_terminal_cols(Some(400)), 320);
    }

    #[test]
    fn clamp_terminal_rows_respects_default_and_limits() {
        assert_eq!(clamp_terminal_rows(None), 36);
        assert_eq!(clamp_terminal_rows(Some(5)), 36);
        assert_eq!(clamp_terminal_rows(Some(200)), 120);
    }

    #[test]
    fn terminal_output_batcher_emits_plain_text_immediately() {
        let mut batcher = TerminalOutputBatcher::default();
        assert_eq!(batcher.push("hello"), vec!["hello".to_string()]);
        assert_eq!(batcher.flush_pending(), None);
    }

    #[test]
    fn terminal_output_batcher_batches_synchronized_output_until_end_marker() {
        let mut batcher = TerminalOutputBatcher::default();
        assert!(batcher.push("\u{001b}[?2026hpartial").is_empty());
        assert_eq!(
            batcher.push(" frame\u{001b}[?2026l"),
            vec!["\u{001b}[?2026hpartial frame\u{001b}[?2026l".to_string()]
        );
        assert_eq!(batcher.flush_pending(), None);
    }

    #[test]
    fn terminal_output_batcher_handles_split_markers_across_chunks() {
        let mut batcher = TerminalOutputBatcher::default();
        assert_eq!(batcher.push("a\u{001b}[?20"), vec!["a".to_string()]);
        assert!(batcher.push("26hmid").is_empty());
        assert_eq!(
            batcher.push("dle\u{001b}[?2026lz"),
            vec![
                "\u{001b}[?2026hmiddle\u{001b}[?2026l".to_string(),
                "z".to_string(),
            ]
        );
        assert_eq!(batcher.flush_pending(), None);
    }

    #[test]
    fn terminal_output_batcher_flushes_unfinished_sync_frame_on_exit() {
        let mut batcher = TerminalOutputBatcher::default();
        assert!(batcher.push("\u{001b}[?2026hunfinished").is_empty());
        assert_eq!(
            batcher.flush_pending(),
            Some("\u{001b}[?2026hunfinished".to_string())
        );
    }
}
