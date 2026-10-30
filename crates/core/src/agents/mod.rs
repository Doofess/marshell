//! One module per agent CLI. Each holds that agent's own JSON; the code around it stays vendor-neutral.

pub mod account;
pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod common;
pub mod identify;
pub mod install;
pub mod manifest;
pub mod own;
pub mod presets;
pub mod settings_view;
pub mod skill;

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Runs `<exe> --version` and returns stdout. The process is killed after `timeout`.
pub fn run_version_text(exe: &Path, timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on its own thread, so a full pipe cannot hold the child past the deadline.
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut out);
        out
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    // After a kill the reader is left alone: a grandchild may still hold the pipe open.
    if !status?.success() {
        return None;
    }
    let out = reader.join().ok()?;
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// Runs `<exe> <args>` with no stdin and returns its exit code and its stdout then stderr, joined. The process is
/// killed after `timeout`, which gives `None`, as does a program that can't start.
pub fn run_capture(exe: &Path, args: &[&str], timeout: Duration) -> Option<(i32, String)> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    // Each pipe is read on its own thread, so a full pipe cannot hold the child past the deadline.
    fn read(mut pipe: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = pipe.read_to_end(&mut out);
            out
        })
    }
    let out_reader = read(stdout);
    let err_reader = read(stderr);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                // After a kill the readers are left alone: a grandchild may still hold a pipe open.
                return None;
            }
        }
    };
    let mut text = String::from_utf8_lossy(&out_reader.join().ok()?).into_owned();
    text.push_str(&String::from_utf8_lossy(&err_reader.join().ok()?));
    Some((status.code().unwrap_or(-1), text))
}

/// The first version-shaped token, with an optional leading `v` removed and a suffix kept.
pub fn parse_any_version(stdout: &str) -> Option<String> {
    use std::sync::LazyLock;
    static VERSION: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+").expect("valid version pattern"));
    stdout.split_whitespace().find_map(|token| {
        let token = token.strip_prefix('v').unwrap_or(token);
        VERSION.is_match(token).then(|| token.to_string())
    })
}

/// Variables that identify the Claude Code session Marshell itself may have been started from;
/// user settings such as `CLAUDE_CODE_GIT_BASH_PATH` are not in it.
pub const INHERITED_MARKERS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_PID",
];

use crate::hooks::holds::PendingPermission;
use marshell_protocol::hook::{PermissionDecision, PermissionView};
use serde_json::Value;

/// Whether Marshell can show and answer this agent's permission requests.
pub fn supports_permissions(agent: &str) -> bool {
    matches!(agent, "claude" | "codex" | "antigravity")
}

/// A held request's card, when its agent is known and it belongs to a tab.
pub fn permission_view(p: &PendingPermission) -> Option<PermissionView> {
    p.tab.as_ref()?;
    if !supports_permissions(&p.agent) {
        return None;
    }
    match p.agent.as_str() {
        "claude" => claude::request::view(p),
        "codex" => codex::request::view(p),
        "antigravity" => antigravity::request::view(p),
        _ => None,
    }
}

/// The hook output that carries `decision` back to `agent`, or why it can't.
pub fn permission_output(agent: &str, decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str> {
    match agent {
        "claude" => claude::permission_output(decision, request),
        "codex" => codex::request::permission_output(decision, request),
        "antigravity" => antigravity::request::permission_output(decision, request),
        _ => Err("This agent can't take a decision yet."),
    }
}

/// What a released or timed-out hold answers instead of the 204. `None` for an agent whose own prompt takes
/// over (Claude Code, Codex). Antigravity gets a deny: only a Bypass tab is held, and there agy runs the call
/// on an empty answer.
pub fn release_output(agent: &str, _request: &Value) -> Option<Value> {
    match agent {
        "antigravity" => Some(antigravity::request::release_output()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::INHERITED_MARKERS;

    #[test]
    fn parse_any_version_reads_common_shapes() {
        for (input, want) in [
            ("codex-cli 0.47.0", Some("0.47.0")),
            ("v1.2.3", Some("1.2.3")),
            ("2.1.302 (Claude Code)", Some("2.1.302")),
            ("0.1.0-alpha.2", Some("0.1.0-alpha.2")),
            ("hello", None),
            ("", None),
            ("tool 1.2 nope v3.4.5 6.7.8", Some("3.4.5")),
        ] {
            assert_eq!(super::parse_any_version(input).as_deref(), want, "{input}");
        }
    }

    #[test]
    fn markers_name_the_session_identity_only() {
        assert_eq!(
            INHERITED_MARKERS,
            [
                "CLAUDECODE",
                "CLAUDE_CODE_ENTRYPOINT",
                "CLAUDE_CODE_CHILD_SESSION",
                "CLAUDE_CODE_SESSION_ID",
                "CLAUDE_CODE_SESSION_ATTENDED",
                "CLAUDE_CODE_MESSAGING_SOCKET",
                "CLAUDE_CODE_MESSAGING_TOKEN",
                "CLAUDE_CODE_EXECPATH",
                "CLAUDE_CODE_SSE_PORT",
                "CLAUDE_PID",
            ]
        );
        assert!(!INHERITED_MARKERS.contains(&"CLAUDE_CODE_GIT_BASH_PATH"));
        assert!(!INHERITED_MARKERS.contains(&"CLAUDE_CODE_USE_BEDROCK"));
    }
}
