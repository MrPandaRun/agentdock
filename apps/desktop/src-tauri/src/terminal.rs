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

pub fn open_thread_in_terminal(
    provider_id: ProviderId,
    thread_id: &str,
    project_path: Option<&str>,
) -> Result<OpenThreadInTerminalResponse, String> {
    let command = build_resume_command_from_parts(provider_id, thread_id, project_path);
    launch_in_terminal(&command)?;
    Ok(OpenThreadInTerminalResponse {
        launched: true,
        command,
        terminal_app: "Terminal".to_string(),
    })
}

pub fn open_new_thread_in_terminal(
    provider_id: ProviderId,
    project_path: Option<&str>,
) -> Result<OpenThreadInTerminalResponse, String> {
    let command = build_new_thread_command_from_parts(provider_id, project_path);
    launch_in_terminal(&command)?;
    Ok(OpenThreadInTerminalResponse {
        launched: true,
        command,
        terminal_app: "Terminal".to_string(),
    })
}

pub fn start_embedded_terminal(
    app: tauri::AppHandle,
    provider_id: ProviderId,
    thread_id: &str,
    project_path: Option<&str>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<StartEmbeddedTerminalResponse, String> {
    let cols = clamp_terminal_cols(cols);
    let rows = clamp_terminal_rows(rows);
    let command = build_resume_command_from_parts(provider_id, thread_id, project_path);
    let session_id = next_embedded_terminal_session_id();
    let (reader, session) = create_embedded_session(&command, cols, rows)?;
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
    project_path: Option<&str>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<StartEmbeddedTerminalResponse, String> {
    let cols = clamp_terminal_cols(cols);
    let rows = clamp_terminal_rows(rows);
    let command = build_new_thread_command_from_parts(provider_id, project_path);
    let session_id = next_embedded_terminal_session_id();
    let (reader, session) = create_embedded_session(&command, cols, rows)?;
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

    let mut cmd = CommandBuilder::new("sh");
    cmd.arg("-lc");
    cmd.arg(command);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("COLUMNS", cols.to_string());
    cmd.env("LINES", rows.to_string());

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

fn spawn_terminal_output_reader<R: Read + Send + 'static>(
    app: tauri::AppHandle,
    session_id: String,
    mut stream: R,
) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        let mut pending = Vec::new();
        loop {
            let read = match stream.read(&mut buffer) {
                Ok(size) => size,
                Err(_) => break,
            };
            if read == 0 {
                if !pending.is_empty() {
                    let data = String::from_utf8_lossy(&pending).to_string();
                    if !data.is_empty() {
                        let payload = EmbeddedTerminalOutputPayload {
                            session_id: session_id.clone(),
                            data,
                        };
                        let _ = app.emit("embedded-terminal-output", payload);
                    }
                    pending.clear();
                }
                break;
            }

            pending.extend_from_slice(&buffer[..read]);

            loop {
                match std::str::from_utf8(&pending) {
                    Ok(text) => {
                        if !text.is_empty() {
                            let payload = EmbeddedTerminalOutputPayload {
                                session_id: session_id.clone(),
                                data: text.to_string(),
                            };
                            let _ = app.emit("embedded-terminal-output", payload);
                        }
                        pending.clear();
                        break;
                    }
                    Err(error) => {
                        let valid_up_to = error.valid_up_to();
                        if valid_up_to > 0 {
                            let valid = &pending[..valid_up_to];
                            let payload = EmbeddedTerminalOutputPayload {
                                session_id: session_id.clone(),
                                data: String::from_utf8_lossy(valid).to_string(),
                            };
                            let _ = app.emit("embedded-terminal-output", payload);
                        }

                        match error.error_len() {
                            Some(error_len) => {
                                // True invalid bytes: skip the offending sequence and continue.
                                let drain_to = valid_up_to + error_len;
                                pending.drain(..drain_to);
                                let payload = EmbeddedTerminalOutputPayload {
                                    session_id: session_id.clone(),
                                    data: "\u{FFFD}".to_string(),
                                };
                                let _ = app.emit("embedded-terminal-output", payload);
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
    project_path: Option<&str>,
) -> String {
    let resume = match provider_id {
        ProviderId::ClaudeCode => format!("claude --resume {}", shell_quote(thread_id)),
        ProviderId::Codex => format!("codex resume {}", shell_quote(thread_id)),
        ProviderId::OpenCode => format!("opencode --session {}", shell_quote(thread_id)),
    };

    let project_path = project_path
        .map(str::trim)
        .filter(|path| !path.is_empty() && *path != ".");

    if let Some(path) = project_path {
        return format!("cd {} && {resume}", shell_quote(path));
    }

    resume
}

fn build_new_thread_command_from_parts(
    provider_id: ProviderId,
    project_path: Option<&str>,
) -> String {
    let start = match provider_id {
        ProviderId::ClaudeCode => "claude".to_string(),
        ProviderId::Codex => "codex".to_string(),
        ProviderId::OpenCode => "opencode".to_string(),
    };

    let project_path = project_path
        .map(str::trim)
        .filter(|path| !path.is_empty() && *path != ".");

    if let Some(path) = project_path {
        return format!("cd {} && {start}", shell_quote(path));
    }

    start
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

#[cfg(not(target_os = "macos"))]
fn launch_in_terminal(_command: &str) -> Result<(), String> {
    Err("Terminal launch is only supported on macOS for now".to_string())
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use provider_contract::ProviderId;

    use super::{
        build_new_thread_command_from_parts, build_resume_command_from_parts, clamp_terminal_cols,
        clamp_terminal_rows, shell_quote,
    };

    #[test]
    fn build_resume_command_quotes_thread_id_and_project_path() {
        let command = build_resume_command_from_parts(
            ProviderId::ClaudeCode,
            "thread id",
            Some("/tmp/my project"),
        );
        assert_eq!(
            command,
            "cd '/tmp/my project' && claude --resume 'thread id'"
        );
    }

    #[test]
    fn build_new_thread_command_supports_provider_without_project_path() {
        let command = build_new_thread_command_from_parts(ProviderId::OpenCode, None);
        assert_eq!(command, "opencode");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
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
}
