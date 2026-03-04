use std::path::Path;
use std::process::{Command, Output};

use crate::payloads::{
    OpenProjectWithTargetResponse, OpenTargetStatusPayload, ProjectGitBranchPayload,
};

const TERMINAL_APP_PATH: &str = "/System/Applications/Utilities/Terminal.app";
const ITERM_APP_PATH: &str = "/Applications/iTerm.app";
const WARP_APP_PATH: &str = "/Applications/Warp.app";
const WARP_BUNDLE_ID: &str = "dev.warp.Warp-Stable";
const ZED_APP_PATH: &str = "/Applications/Zed.app";
const SUBLIME_TEXT_APP_PATH: &str = "/Applications/Sublime Text.app";
const INTELLIJ_APP_PATHS: [&str; 3] = [
    "/Applications/IntelliJ IDEA.app",
    "/Applications/IntelliJ IDEA CE.app",
    "/Applications/IntelliJ IDEA Ultimate.app",
];
const WEBSTORM_APP_PATHS: [&str; 2] = [
    "/Applications/WebStorm.app",
    "/Applications/WebStorm EAP.app",
];
const PYCHARM_APP_PATHS: [&str; 3] = [
    "/Applications/PyCharm.app",
    "/Applications/PyCharm CE.app",
    "/Applications/PyCharm Professional.app",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenTargetId {
    Vscode,
    Cursor,
    Windsurf,
    Antigravity,
    Zed,
    Intellij,
    WebStorm,
    PyCharm,
    SublimeText,
    Terminal,
    ITerm,
    Warp,
}

impl OpenTargetId {
    fn all() -> [Self; 12] {
        [
            Self::Vscode,
            Self::Cursor,
            Self::Windsurf,
            Self::Antigravity,
            Self::Zed,
            Self::Intellij,
            Self::WebStorm,
            Self::PyCharm,
            Self::SublimeText,
            Self::Terminal,
            Self::ITerm,
            Self::Warp,
        ]
    }

    fn from_raw(raw: &str) -> Option<Self> {
        match raw {
            "vscode" => Some(Self::Vscode),
            "cursor" => Some(Self::Cursor),
            "windsurf" => Some(Self::Windsurf),
            "antigravity" => Some(Self::Antigravity),
            "zed" => Some(Self::Zed),
            "intellij" => Some(Self::Intellij),
            "webstorm" => Some(Self::WebStorm),
            "pycharm" => Some(Self::PyCharm),
            "sublime_text" => Some(Self::SublimeText),
            "terminal" => Some(Self::Terminal),
            "iterm" => Some(Self::ITerm),
            "warp" => Some(Self::Warp),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Vscode => "vscode",
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::Antigravity => "antigravity",
            Self::Zed => "zed",
            Self::Intellij => "intellij",
            Self::WebStorm => "webstorm",
            Self::PyCharm => "pycharm",
            Self::SublimeText => "sublime_text",
            Self::Terminal => "terminal",
            Self::ITerm => "iterm",
            Self::Warp => "warp",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Vscode => "VS Code",
            Self::Cursor => "Cursor",
            Self::Windsurf => "Windsurf",
            Self::Antigravity => "Antigravity",
            Self::Zed => "Zed",
            Self::Intellij => "IntelliJ IDEA",
            Self::WebStorm => "WebStorm",
            Self::PyCharm => "PyCharm",
            Self::SublimeText => "Sublime Text",
            Self::Terminal => "Terminal",
            Self::ITerm => "iTerm",
            Self::Warp => "Warp",
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::Terminal | Self::ITerm | Self::Warp => "terminal",
            _ => "ide",
        }
    }
}

#[derive(Debug)]
struct TargetDetection {
    installed: bool,
    available: bool,
    detail: Option<String>,
}

#[derive(Debug, Clone)]
enum LaunchStrategy {
    Cli { command: String },
    AppOpen {
        app_path: &'static str,
        app_label: &'static str,
    },
    TerminalApp,
    ITermApp,
    WarpBundleOpen,
}

pub fn list_open_targets() -> Result<Vec<OpenTargetStatusPayload>, String> {
    OpenTargetId::all()
        .into_iter()
        .map(|target| {
            let detection = detect_target(target)?;
            Ok(OpenTargetStatusPayload {
                id: target.as_str().to_string(),
                label: target.label().to_string(),
                installed: detection.installed,
                available: detection.available,
                detail: detection.detail,
                kind: target.kind().to_string(),
            })
        })
        .collect()
}

pub fn open_project_with_target(
    project_path: &str,
    target_id: &str,
) -> Result<OpenProjectWithTargetResponse, String> {
    let normalized_path = normalize_project_path(project_path)?;
    let path = Path::new(&normalized_path);
    if !path.exists() {
        return Err(format!("Project path does not exist: {normalized_path}"));
    }
    if !path.is_dir() {
        return Err(format!("Project path is not a directory: {normalized_path}"));
    }

    let target = OpenTargetId::from_raw(target_id)
        .ok_or_else(|| format!("Unsupported open target: {target_id}"))?;
    let launch_strategy = resolve_launch_strategy(target)?.ok_or_else(|| {
        format!(
            "{} is not installed or unavailable on this platform.",
            target.label()
        )
    })?;
    let command = launch_with_strategy(&launch_strategy, &normalized_path)?;

    Ok(OpenProjectWithTargetResponse {
        launched: true,
        target_id: target.as_str().to_string(),
        command,
    })
}

pub fn get_project_git_branch(project_path: &str) -> Result<ProjectGitBranchPayload, String> {
    let normalized_path = project_path.trim().to_string();
    if normalized_path.is_empty() || normalized_path == "-" {
        return Ok(ProjectGitBranchPayload {
            status: "path_missing".to_string(),
            branch: None,
            message: Some("Project path is empty.".to_string()),
        });
    }
    let path = Path::new(&normalized_path);
    if !path.exists() || !path.is_dir() {
        return Ok(ProjectGitBranchPayload {
            status: "path_missing".to_string(),
            branch: None,
            message: Some(format!("Path does not exist: {normalized_path}")),
        });
    }

    let output = match Command::new("git")
        .arg("-C")
        .arg(&normalized_path)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return Ok(ProjectGitBranchPayload {
                status: "error".to_string(),
                branch: None,
                message: Some(format!("Failed to run git command: {error}")),
            });
        }
    };

    if output.status.success() {
        let mut branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            return Ok(ProjectGitBranchPayload {
                status: "error".to_string(),
                branch: None,
                message: Some("Git branch output is empty".to_string()),
            });
        }

        if branch == "HEAD" {
            if let Some(hash) = read_head_short_hash(&normalized_path) {
                branch = format!("detached@{hash}");
            }
        }

        return Ok(ProjectGitBranchPayload {
            status: "ok".to_string(),
            branch: Some(branch),
            message: None,
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if is_not_git_repo_message(&stderr) {
        return Ok(ProjectGitBranchPayload {
            status: "no_repo".to_string(),
            branch: None,
            message: None,
        });
    }

    let message = if stderr.is_empty() {
        format!("Git command failed with status {}", output.status)
    } else {
        stderr
    };

    Ok(ProjectGitBranchPayload {
        status: "error".to_string(),
        branch: None,
        message: Some(message),
    })
}

fn detect_target(target: OpenTargetId) -> Result<TargetDetection, String> {
    match target {
        OpenTargetId::Vscode => detect_cli_or_app_target(
            &["code", "code-insiders"],
            &["/Applications/Visual Studio Code.app"],
            "VS Code",
        ),
        OpenTargetId::Cursor => detect_cli_or_app_target(
            &["cursor"],
            &["/Applications/Cursor.app"],
            "Cursor",
        ),
        OpenTargetId::Windsurf => detect_cli_or_app_target(
            &["windsurf"],
            &["/Applications/Windsurf.app"],
            "Windsurf",
        ),
        OpenTargetId::Antigravity => detect_cli_or_app_target(
            &["antigravity"],
            &["/Applications/Antigravity.app"],
            "Antigravity",
        ),
        OpenTargetId::Zed => {
            detect_cli_or_app_target(&["zed"], &[ZED_APP_PATH], "Zed")
        }
        OpenTargetId::Intellij => {
            detect_cli_or_app_target(&["idea"], &INTELLIJ_APP_PATHS, "IntelliJ IDEA")
        }
        OpenTargetId::WebStorm => {
            detect_cli_or_app_target(&["webstorm"], &WEBSTORM_APP_PATHS, "WebStorm")
        }
        OpenTargetId::PyCharm => {
            detect_cli_or_app_target(&["charm"], &PYCHARM_APP_PATHS, "PyCharm")
        }
        OpenTargetId::SublimeText => {
            detect_cli_or_app_target(&["subl"], &[SUBLIME_TEXT_APP_PATH], "Sublime Text")
        }
        OpenTargetId::Terminal => detect_app_target(TERMINAL_APP_PATH, "Terminal.app"),
        OpenTargetId::ITerm => detect_app_target(ITERM_APP_PATH, "iTerm.app"),
        OpenTargetId::Warp => detect_warp_target(),
    }
}

fn resolve_launch_strategy(target: OpenTargetId) -> Result<Option<LaunchStrategy>, String> {
    match target {
        OpenTargetId::Vscode => resolve_cli_or_app_strategy(
            &["code", "code-insiders"],
            &["/Applications/Visual Studio Code.app"],
            "VS Code",
        ),
        OpenTargetId::Cursor => resolve_cli_or_app_strategy(
            &["cursor"],
            &["/Applications/Cursor.app"],
            "Cursor",
        ),
        OpenTargetId::Windsurf => resolve_cli_or_app_strategy(
            &["windsurf"],
            &["/Applications/Windsurf.app"],
            "Windsurf",
        ),
        OpenTargetId::Antigravity => resolve_cli_or_app_strategy(
            &["antigravity"],
            &["/Applications/Antigravity.app"],
            "Antigravity",
        ),
        OpenTargetId::Zed => resolve_cli_or_app_strategy(&["zed"], &[ZED_APP_PATH], "Zed"),
        OpenTargetId::Intellij => {
            resolve_cli_or_app_strategy(&["idea"], &INTELLIJ_APP_PATHS, "IntelliJ IDEA")
        }
        OpenTargetId::WebStorm => {
            resolve_cli_or_app_strategy(&["webstorm"], &WEBSTORM_APP_PATHS, "WebStorm")
        }
        OpenTargetId::PyCharm => {
            resolve_cli_or_app_strategy(&["charm"], &PYCHARM_APP_PATHS, "PyCharm")
        }
        OpenTargetId::SublimeText => {
            resolve_cli_or_app_strategy(&["subl"], &[SUBLIME_TEXT_APP_PATH], "Sublime Text")
        }
        OpenTargetId::Terminal => {
            if cfg!(target_os = "macos") && app_path_exists(TERMINAL_APP_PATH) {
                Ok(Some(LaunchStrategy::TerminalApp))
            } else {
                Ok(None)
            }
        }
        OpenTargetId::ITerm => {
            if cfg!(target_os = "macos") && app_path_exists(ITERM_APP_PATH) {
                Ok(Some(LaunchStrategy::ITermApp))
            } else {
                Ok(None)
            }
        }
        OpenTargetId::Warp => {
            if let Some(command) = first_available_command(&["warp"])? {
                return Ok(Some(LaunchStrategy::Cli { command }));
            }
            if cfg!(target_os = "macos") && app_path_exists(WARP_APP_PATH) {
                return Ok(Some(LaunchStrategy::WarpBundleOpen));
            }
            Ok(None)
        }
    }
}

fn launch_with_strategy(strategy: &LaunchStrategy, project_path: &str) -> Result<String, String> {
    match strategy {
        LaunchStrategy::Cli { command } => {
            let output = Command::new(command)
                .arg(project_path)
                .output()
                .map_err(|error| format!("Failed to launch {command}: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to launch {command}: {}",
                    command_error_detail(&output)
                ));
            }
            Ok(display_cli_command(command, project_path))
        }
        LaunchStrategy::AppOpen {
            app_path,
            app_label,
        } => {
            launch_app_with_path(app_path, project_path)?;
            Ok(format!(
                "{}: open -a {} {}",
                app_label,
                shell_quote(app_path),
                shell_quote(project_path)
            ))
        }
        LaunchStrategy::TerminalApp => {
            launch_terminal_with_cd(project_path)?;
            Ok(format!("Terminal: cd {}", shell_quote(project_path)))
        }
        LaunchStrategy::ITermApp => {
            launch_iterm_with_cd(project_path)?;
            Ok(format!("iTerm: cd {}", shell_quote(project_path)))
        }
        LaunchStrategy::WarpBundleOpen => {
            launch_warp_bundle(project_path)?;
            Ok(format!(
                "open -b {WARP_BUNDLE_ID} {}",
                shell_quote(project_path)
            ))
        }
    }
}

fn detect_app_target(path: &str, app_name: &str) -> Result<TargetDetection, String> {
    let installed = app_path_exists(path);
    if !installed {
        return Ok(TargetDetection {
            installed: false,
            available: false,
            detail: Some(format!("{app_name} is not installed.")),
        });
    }

    if cfg!(target_os = "macos") {
        return Ok(TargetDetection {
            installed: true,
            available: true,
            detail: None,
        });
    }

    Ok(TargetDetection {
        installed: true,
        available: false,
        detail: Some("App launch is only supported on macOS.".to_string()),
    })
}

fn detect_warp_target() -> Result<TargetDetection, String> {
    let warp_cli_available = first_available_command(&["warp"])?.is_some();
    let warp_app_installed = app_path_exists(WARP_APP_PATH);
    let installed = warp_cli_available || warp_app_installed;
    let available = warp_cli_available || (cfg!(target_os = "macos") && warp_app_installed);

    let detail = if installed {
        None
    } else {
        Some("Warp CLI or app is not installed.".to_string())
    };

    Ok(TargetDetection {
        installed,
        available,
        detail,
    })
}

fn detect_cli_or_app_target(
    commands: &[&str],
    app_paths: &[&'static str],
    label: &str,
) -> Result<TargetDetection, String> {
    let cli_available = first_available_command(commands)?.is_some();
    let app_path = first_existing_app_path(app_paths);
    let installed = cli_available || app_path.is_some();
    let available = cli_available || (cfg!(target_os = "macos") && app_path.is_some());

    let detail = if installed {
        None
    } else {
        Some(format!(
            "{label} command and app were not detected on this machine."
        ))
    };

    Ok(TargetDetection {
        installed,
        available,
        detail,
    })
}

fn resolve_cli_or_app_strategy(
    commands: &[&str],
    app_paths: &[&'static str],
    app_label: &'static str,
) -> Result<Option<LaunchStrategy>, String> {
    if let Some(command) = first_available_command(commands)? {
        return Ok(Some(LaunchStrategy::Cli { command }));
    }

    if cfg!(target_os = "macos") {
        if let Some(app_path) = first_existing_app_path(app_paths) {
            return Ok(Some(LaunchStrategy::AppOpen {
                app_path,
                app_label,
            }));
        }
    }

    Ok(None)
}

fn app_path_exists(path: &str) -> bool {
    Path::new(path).is_dir()
}

fn first_existing_app_path(app_paths: &[&'static str]) -> Option<&'static str> {
    app_paths.iter().copied().find(|path| app_path_exists(path))
}

fn first_available_command(commands: &[&str]) -> Result<Option<String>, String> {
    for command in commands {
        if command_available(command)? {
            return Ok(Some((*command).to_string()));
        }
    }
    Ok(None)
}

fn command_available(command: &str) -> Result<bool, String> {
    let output = Command::new("sh")
        .arg("-lc")
        .arg(format!(
            "command -v {} >/dev/null 2>&1",
            shell_quote(command)
        ))
        .output()
        .map_err(|error| format!("Failed to check command availability: {error}"))?;
    Ok(output.status.success())
}

fn normalize_project_path(project_path: &str) -> Result<String, String> {
    let trimmed = project_path.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return Err("Project path is empty.".to_string());
    }
    Ok(trimmed.to_string())
}

fn display_cli_command(command: &str, project_path: &str) -> String {
    format!("{command} {}", shell_quote(project_path))
}

fn command_error_detail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    format!("exit status {}", output.status)
}

fn read_head_short_hash(project_path: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() {
        return None;
    }
    Some(hash)
}

fn is_not_git_repo_message(stderr: &str) -> bool {
    stderr.to_ascii_lowercase().contains("not a git repository")
}

#[cfg(target_os = "macos")]
fn launch_app_with_path(app_path: &str, project_path: &str) -> Result<(), String> {
    let output = Command::new("open")
        .arg("-a")
        .arg(app_path)
        .arg(project_path)
        .output()
        .map_err(|error| format!("Failed to open app at {app_path}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "Failed to open app at {app_path}: {}",
        command_error_detail(&output)
    ))
}

#[cfg(not(target_os = "macos"))]
fn launch_app_with_path(_app_path: &str, _project_path: &str) -> Result<(), String> {
    Err("App launch is only supported on macOS.".to_string())
}

#[cfg(target_os = "macos")]
fn launch_terminal_with_cd(project_path: &str) -> Result<(), String> {
    let command = format!("cd {}", shell_quote(project_path));
    let escaped_command = escape_applescript(&command);
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "tell application \"Terminal\" to do script \"{escaped_command}\""
        ))
        .arg("-e")
        .arg("tell application \"Terminal\" to activate")
        .output()
        .map_err(|error| format!("Failed to invoke osascript for Terminal: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "Failed to open Terminal: {}",
        command_error_detail(&output)
    ))
}

#[cfg(not(target_os = "macos"))]
fn launch_terminal_with_cd(_project_path: &str) -> Result<(), String> {
    Err("Terminal launch is only supported on macOS.".to_string())
}

#[cfg(target_os = "macos")]
fn launch_iterm_with_cd(project_path: &str) -> Result<(), String> {
    let command = format!("cd {}", shell_quote(project_path));
    let escaped_command = escape_applescript(&command);
    let script = format!(
        "tell application \"iTerm2\"\n\
           activate\n\
           set newWindow to (create window with default profile)\n\
           tell current session of newWindow\n\
             write text \"{escaped_command}\"\n\
           end tell\n\
         end tell"
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|error| format!("Failed to invoke osascript for iTerm: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "Failed to open iTerm: {}",
        command_error_detail(&output)
    ))
}

#[cfg(not(target_os = "macos"))]
fn launch_iterm_with_cd(_project_path: &str) -> Result<(), String> {
    Err("iTerm launch is only supported on macOS.".to_string())
}

#[cfg(target_os = "macos")]
fn launch_warp_bundle(project_path: &str) -> Result<(), String> {
    let output = Command::new("open")
        .arg("-b")
        .arg(WARP_BUNDLE_ID)
        .arg(project_path)
        .output()
        .map_err(|error| format!("Failed to open Warp app bundle: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "Failed to open Warp app bundle: {}",
        command_error_detail(&output)
    ))
}

#[cfg(not(target_os = "macos"))]
fn launch_warp_bundle(_project_path: &str) -> Result<(), String> {
    Err("Warp app launch is only supported on macOS.".to_string())
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use tempfile::tempdir;

    use super::{
        display_cli_command, get_project_git_branch, is_not_git_repo_message, open_project_with_target,
    };

    #[test]
    fn display_cli_command_quotes_project_path_for_antigravity() {
        let command = display_cli_command("antigravity", "/tmp/my project");
        assert_eq!(command, "antigravity '/tmp/my project'");
    }

    #[test]
    fn open_project_with_target_rejects_missing_path() {
        let error = open_project_with_target("/tmp/definitely-missing-agentdock-path", "vscode")
            .expect_err("missing path should be rejected");
        assert!(error.contains("does not exist"));
    }

    #[test]
    fn get_project_git_branch_returns_path_missing_for_invalid_path() {
        let payload = get_project_git_branch("/tmp/definitely-missing-agentdock-path")
            .expect("command should return payload");
        assert_eq!(payload.status, "path_missing");
    }

    #[test]
    fn get_project_git_branch_returns_no_repo_for_plain_directory() {
        let dir = tempdir().expect("tempdir should be created");
        let payload = get_project_git_branch(
            dir.path()
                .to_str()
                .expect("temp directory path should be valid UTF-8"),
        )
        .expect("command should return payload");
        assert_eq!(payload.status, "no_repo");
    }

    #[test]
    fn get_project_git_branch_returns_ok_for_git_repository() {
        let dir = tempdir().expect("tempdir should be created");
        let repo_path = dir
            .path()
            .to_str()
            .expect("temp directory path should be valid UTF-8")
            .to_string();
        run_git(&repo_path, &["init"]);
        run_git(&repo_path, &["config", "user.email", "agentdock@example.com"]);
        run_git(&repo_path, &["config", "user.name", "AgentDock"]);
        fs::write(dir.path().join("README.md"), "hello\n").expect("file should be written");
        run_git(&repo_path, &["add", "."]);
        run_git(&repo_path, &["commit", "-m", "init"]);

        let payload = get_project_git_branch(&repo_path).expect("command should return payload");
        assert_eq!(payload.status, "ok");
        assert!(payload.branch.unwrap_or_default().trim().len() > 0);
    }

    #[test]
    fn classifies_not_git_repo_error_message() {
        assert!(is_not_git_repo_message("fatal: not a git repository"));
        assert!(!is_not_git_repo_message("fatal: unknown revision"));
    }

    fn run_git(repo_path: &str, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
            .expect("git command should execute");
        if !output.status.success() {
            panic!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
