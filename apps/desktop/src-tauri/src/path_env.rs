#[cfg(target_os = "macos")]
use std::process::Command;

const SHELL_PATH_SENTINEL_START: &str = "__AGENTDOCK_PATH_START__";
const SHELL_PATH_SENTINEL_END: &str = "__AGENTDOCK_PATH_END__";
const SHELL_PATH_PROBE_COMMAND: &str =
    "printf '__AGENTDOCK_PATH_START__%s__AGENTDOCK_PATH_END__' \"$PATH\"";

#[cfg(target_os = "macos")]
pub fn hydrate_path_from_login_shell() {
    let mut shells = Vec::new();

    if let Ok(shell) = std::env::var("SHELL") {
        let trimmed = shell.trim();
        if !trimmed.is_empty() {
            shells.push(trimmed.to_string());
        }
    }

    shells.push("/bin/zsh".to_string());
    shells.push("/bin/bash".to_string());

    for shell in shells {
        if let Some(path) = read_login_shell_path(&shell) {
            std::env::set_var("PATH", path);
            break;
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hydrate_path_from_login_shell() {}

#[cfg(target_os = "macos")]
fn read_login_shell_path(shell: &str) -> Option<String> {
    let output = Command::new(shell)
        .arg("-ilc")
        .arg(SHELL_PATH_PROBE_COMMAND)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    extract_path_from_shell_output(&output.stdout)
}

pub fn extract_path_from_shell_output(stdout: &[u8]) -> Option<String> {
    let raw = String::from_utf8_lossy(stdout);
    let start = raw.find(SHELL_PATH_SENTINEL_START)? + SHELL_PATH_SENTINEL_START.len();
    let rest = &raw[start..];
    let end = rest.find(SHELL_PATH_SENTINEL_END)?;
    let path = rest[..end].trim();
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::extract_path_from_shell_output;

    #[test]
    fn extract_path_from_shell_output_reads_marker_payload() {
        let output =
            b"noise\n__AGENTDOCK_PATH_START__/usr/local/bin:/opt/homebrew/bin__AGENTDOCK_PATH_END__\n";
        let path = extract_path_from_shell_output(output).expect("path should parse");
        assert_eq!(path, "/usr/local/bin:/opt/homebrew/bin");
    }

    #[test]
    fn extract_path_from_shell_output_returns_none_without_markers() {
        let output = b"/usr/bin:/bin\n";
        assert!(extract_path_from_shell_output(output).is_none());
    }
}
