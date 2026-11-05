//! One module per agent CLI. Each holds that agent's own JSON; the code around it stays vendor-neutral.

pub mod account;
pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod common;
pub mod distro;
pub mod identify;
pub mod install;
pub mod manifest;
pub mod own;
pub mod presets;
pub mod qwen;
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

/// Runs `<exe> <args>` with no stdin and returns its exit code and its stdout then stderr, joined. The process and
/// whatever it started are ended after `timeout`, which gives `None`, as does a program that can't start.
pub fn run_capture(exe: &Path, args: &[&str], timeout: Duration) -> Option<(i32, String)> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    let (code, stdout, stderr) = run_bounded(cmd, timeout)?;
    let mut text = String::from_utf8_lossy(&stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&stderr));
    Some((code, text))
}

/// Runs `cmd` with no stdin and no console window, and returns its exit code, stdout and stderr. `timeout` bounds the
/// whole call: the wait for the child, and then the wait for its pipes, which a grandchild that outlives the child may
/// keep open. When it passes, the child and the processes it started are ended and the result is `None`, not the
/// bytes read so far: a half-read answer is not one the callers can use. `None` too when the program can't start.
pub(crate) fn run_bounded(mut cmd: Command, timeout: Duration) -> Option<(i32, Vec<u8>, Vec<u8>)> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().ok()?;
    let tree = crate::job::ProcessTree::adopt(&child);
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        tree.kill(&mut child);
        let _ = child.wait();
        return None;
    };
    // Each pipe is read on its own thread, so a full pipe cannot hold the child past the deadline. The thread sends
    // its bytes when the pipe closes, which lets the caller wait for them with a deadline.
    fn read(mut pipe: impl std::io::Read + Send + 'static) -> std::sync::mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = pipe.read_to_end(&mut out);
            let _ = tx.send(out);
        });
        rx
    }
    let out_rx = read(stdout);
    let err_rx = read(stderr);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                tree.kill(&mut child);
                let _ = child.wait();
                // The readers are left alone: they end once the killed processes have closed the pipes.
                return None;
            }
        }
    };
    // The child is gone, but a grandchild may still hold a pipe open. Past the deadline, end it, so the readers end too.
    let take = |rx: &std::sync::mpsc::Receiver<Vec<u8>>| {
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).ok()
    };
    let stdout = take(&out_rx);
    let stderr = take(&err_rx);
    let (Some(stdout), Some(stderr)) = (stdout, stderr) else {
        tree.kill(&mut child);
        return None;
    };
    Some((status.code().unwrap_or(-1), stdout, stderr))
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
    matches!(agent, "claude" | "codex" | "antigravity" | "qwen")
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
        "qwen" => qwen::request::view(p),
        _ => None,
    }
}

/// The hook output that carries `decision` back to `agent`, or why it can't.
pub fn permission_output(agent: &str, decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str> {
    match agent {
        "claude" => claude::permission_output(decision, request),
        "codex" => codex::request::permission_output(decision, request),
        "antigravity" => antigravity::request::permission_output(decision, request),
        "qwen" => qwen::request::permission_output(decision, request),
        _ => Err("This agent can't take a decision yet."),
    }
}

/// What a released or timed-out hold answers instead of the 204. `None` for an agent whose own prompt takes
/// over (Claude Code, Codex, Qwen Code: no decision, so Qwen shows its own dialog). Antigravity gets a deny: only a
/// Bypass tab is held, and there agy runs the call on an empty answer.
pub fn release_output(agent: &str, _request: &Value) -> Option<Value> {
    match agent {
        "antigravity" => Some(antigravity::request::release_output()),
        "qwen" => None,
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

    #[cfg(windows)]
    mod tree {
        use std::path::Path;
        use std::time::{Duration, Instant};

        /// Long enough for a cold PowerShell to start and write its PID on a loaded CI runner (5 s wasn't).
        const TREE_TIMEOUT: Duration = Duration::from_secs(20);

        fn pid_alive(pid: u32) -> bool {
            let out = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
                .output()
                .expect("tasklist runs");
            String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
        }

        /// The args of a `cmd.exe /c powershell ...` tree: the grandchild writes its PID to `pid_file`, then sleeps 60 s
        /// (so a failed test leaves nothing behind for long).
        fn sleeper_args(pid_file: &Path) -> Vec<String> {
            let script = format!(
                "$PID | Set-Content -LiteralPath '{}'; Start-Sleep 60",
                pid_file.display()
            );
            ["/c", "powershell", "-NoProfile", "-Command"]
                .into_iter()
                .map(String::from)
                .chain([script])
                .collect()
        }

        fn grandchild_pid(pid_file: &Path) -> u32 {
            std::fs::read_to_string(pid_file)
                .expect("the grandchild wrote its PID before the timeout")
                .trim()
                .parse()
                .expect("a PID")
        }

        #[test]
        fn run_capture_gives_up_when_a_grandchild_holds_the_pipe_after_the_child_exits() {
            let started = Instant::now();
            // `start /b` returns at once and leaves ping running with the inherited pipes.
            let got = super::super::run_capture(
                Path::new("cmd.exe"),
                &["/c", "start", "/b", "ping", "-n", "20", "127.0.0.1"],
                Duration::from_secs(2),
            );
            assert!(got.is_none(), "{got:?}");
            assert!(started.elapsed() < Duration::from_secs(8), "{:?}", started.elapsed());
        }

        #[test]
        fn run_capture_timeout_ends_the_whole_process_tree() {
            let dir = tempfile::tempdir().unwrap();
            let pid_file = dir.path().join("pid.txt");
            let args = sleeper_args(&pid_file);
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let got = super::super::run_capture(Path::new("cmd.exe"), &args, TREE_TIMEOUT);
            assert!(got.is_none(), "{got:?}");
            let pid = grandchild_pid(&pid_file);
            assert!(!pid_alive(pid), "the grandchild {pid} outlived the timeout");
        }

        #[test]
        fn run_bounded_timeout_ends_the_whole_process_tree() {
            let dir = tempfile::tempdir().unwrap();
            let pid_file = dir.path().join("pid.txt");
            let mut cmd = std::process::Command::new("cmd.exe");
            cmd.args(sleeper_args(&pid_file));
            assert!(super::super::run_bounded(cmd, TREE_TIMEOUT).is_none());
            let pid = grandchild_pid(&pid_file);
            assert!(!pid_alive(pid), "the grandchild {pid} outlived the timeout");
        }

        #[test]
        fn run_bounded_returns_the_code_and_both_streams() {
            let mut cmd = std::process::Command::new("cmd.exe");
            cmd.args(["/c", "echo out& echo err 1>&2& exit 3"]);
            let (code, out, err) = super::super::run_bounded(cmd, Duration::from_secs(10)).unwrap();
            assert_eq!(code, 3);
            assert!(String::from_utf8_lossy(&out).contains("out"));
            assert!(String::from_utf8_lossy(&err).contains("err"));
        }
    }
}
