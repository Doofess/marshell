//! The one place the core starts git. Every call is blocking; handlers wrap it
//! in `tokio::task::spawn_blocking`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One git command: the directory it runs in and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStep {
    pub dir: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// The step that stopped `run_steps`, and what git said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepFailure {
    pub index: usize,
    pub output: GitOutput,
}

const NOT_INSTALLED: &str = "Git isn't installed, or isn't on PATH.";

/// Wraps an argument in quotes when it is empty or holds a space, a tab, a
/// line break or a quote. An inner quote becomes `\"`.
fn quote(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '\n', '\r', '"']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

/// The command as the user reads it. The `-c core.quotepath=off` that `run`
/// adds is left out on purpose.
pub fn display(step: &GitStep) -> String {
    let mut text = format!("git -C {}", quote(&step.dir.to_string_lossy()));
    for arg in &step.args {
        text.push(' ');
        text.push_str(&quote(arg));
    }
    text
}

/// `git -C dir -c core.quotepath=off args`, with the environment and window flag every run uses.
/// The caller sets stdin.
fn command(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        // Reading (`status`, `diff`) must not refresh the index, or it could take `index.lock`
        // and make a git command running in the same folder fail.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

fn start_error(e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!(NOT_INSTALLED)
    } else {
        anyhow::Error::new(e).context("Couldn't start git.")
    }
}

pub fn run(dir: &Path, args: &[&str]) -> anyhow::Result<GitOutput> {
    let output = command(dir, args).output().map_err(start_error)?;
    Ok(GitOutput {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Like `run`, with `stdin` written to git and the raw output returned (its exit code, and bytes
/// that need not be UTF-8). The input is written from its own thread while the output is drained,
/// so a long list can't fill a pipe and leave both sides waiting.
pub fn run_input(dir: &Path, args: &[&str], stdin: Vec<u8>) -> anyhow::Result<std::process::Output> {
    use std::io::Write;
    let mut child = command(dir, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(start_error)?;
    let pipe = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            // Git may exit before it reads everything; the exit code says what happened.
            let _ = pipe.write_all(&stdin);
        }
    });
    let output = child.wait_with_output().map_err(anyhow::Error::new);
    let _ = writer.join();
    output
}

/// Why `stream` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// Git finished and all its output was handed over.
    Done,
    /// `each` returned false, and git was killed.
    Asked,
    /// The deadline came, and git was killed.
    Deadline,
    /// The cancel flag was set, and git was killed.
    Cancelled,
}

/// A child that is killed and reaped when it goes out of scope, on every path, a panic included. On
/// Windows it is killed with every process it started: Git for Windows' `cmd\git.exe`, the one most
/// PATHs hold, is a launcher whose own child does the work.
struct Reaped(std::process::Child, crate::job::ProcessTree);

impl Reaped {
    fn new(child: std::process::Child) -> Self {
        let tree = crate::job::ProcessTree::adopt(&child);
        Reaped(child, tree)
    }
}

impl Drop for Reaped {
    fn drop(&mut self) {
        self.1.kill(&mut self.0);
        let _ = self.0.wait();
    }
}

/// How often a running stream looks at the clock and the cancel flag while git prints nothing.
const STREAM_TICK: std::time::Duration = std::time::Duration::from_millis(10);

/// Runs git and hands `each` its stdout in chunks as they come. Git is killed at `deadline` or as soon as
/// `cancel` is set, even while it prints nothing; `each` returning false kills it too. The child is
/// always killed and waited for before this returns. Git's own failure (an exit code other than 0 or
/// 1, the "found nothing" of `git grep`) is an error, but only when it ran to the end.
pub fn stream(
    dir: &Path,
    args: &[&str],
    deadline: std::time::Instant,
    cancel: &std::sync::atomic::AtomicBool,
    each: impl FnMut(&[u8]) -> bool,
) -> anyhow::Result<Stopped> {
    stream_cmd(command(dir, args), deadline, cancel, each)
}

/// `stream` for any command, so the tests can run one that prints nothing.
fn stream_cmd(
    mut cmd: Command,
    deadline: std::time::Instant,
    cancel: &std::sync::atomic::AtomicBool,
    mut each: impl FnMut(&[u8]) -> bool,
) -> anyhow::Result<Stopped> {
    use std::io::Read;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{RecvTimeoutError, sync_channel};
    use std::time::Instant;

    if cancel.load(Ordering::Relaxed) {
        return Ok(Stopped::Cancelled);
    }
    let mut child = Reaped::new(
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(start_error)?,
    );
    let mut out = child.0.stdout.take().expect("stdout is piped");
    // The pipe is read on its own thread, so this one can watch the clock and the flag while git is
    // silent. The small queue holds the reader back when `each` is slow.
    let (tx, rx) = sync_channel::<Vec<u8>>(4);
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match out.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Stopped::Cancelled);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(Stopped::Deadline);
        }
        match rx.recv_timeout(STREAM_TICK.min(deadline - now)) {
            Ok(chunk) => {
                if !each(&chunk) {
                    return Ok(Stopped::Asked);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // The output is over. Git should be about to exit; wait for it, still watching the clock.
    loop {
        match child.0.try_wait().map_err(anyhow::Error::new)? {
            Some(status) => {
                return match status.code() {
                    Some(0 | 1) => Ok(Stopped::Done),
                    Some(code) => Err(anyhow::anyhow!("Git stopped with exit code {code}.")),
                    None => Err(anyhow::anyhow!("Git was stopped.")),
                };
            }
            None if cancel.load(Ordering::Relaxed) => return Ok(Stopped::Cancelled),
            None if Instant::now() >= deadline => return Ok(Stopped::Deadline),
            None => std::thread::sleep(std::time::Duration::from_millis(2)),
        }
    }
}

/// Runs the steps in order and stops at the first one that fails. A step whose
/// git can't be started fails with the reason in `stderr`.
pub fn run_steps(steps: &[GitStep]) -> Result<(), StepFailure> {
    for (index, step) in steps.iter().enumerate() {
        let args: Vec<&str> = step.args.iter().map(String::as_str).collect();
        let output = run(&step.dir, &args).unwrap_or_else(|e| GitOutput {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
        });
        if !output.ok {
            return Err(StepFailure { index, output });
        }
    }
    Ok(())
}

/// The last non-empty line of git's stderr, trimmed.
pub fn last_line(stderr: &str) -> &str {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("")
}

/// Why a git call failed, in one line for "Git said: ...". The last `error:` or `fatal:` line of
/// stderr with that prefix removed (git's closing "Aborting" and its hints say less), else the
/// last non-empty stderr line, else the last non-empty stdout line (`commit` reports some
/// failures there), else a sentence saying git gave no reason.
pub fn said(out: &GitOutput) -> &str {
    let stderr_lines = || out.stderr.lines().map(str::trim);
    stderr_lines()
        .rfind(|l| l.starts_with("error:") || l.starts_with("fatal:"))
        .map(|l| l.split_once(':').map_or(l, |(_, rest)| rest.trim()))
        .filter(|l| !l.is_empty())
        .or_else(|| stderr_lines().rfind(|l| !l.is_empty()))
        .or_else(|| out.stdout.lines().map(str::trim).rfind(|l| !l.is_empty()))
        .unwrap_or("it failed without saying why")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    /// A command that prints nothing and stays alive for 10 seconds.
    fn sleeper() -> Command {
        #[cfg(windows)]
        {
            let mut cmd = Command::new("powershell");
            cmd.args(["-NoProfile", "-Command", "Start-Sleep 10"]);
            cmd
        }
        #[cfg(not(windows))]
        {
            let mut cmd = Command::new("sleep");
            cmd.arg("10");
            cmd
        }
    }

    #[test]
    fn stream_stops_when_each_says_so() {
        let cancel = AtomicBool::new(false);
        let mut chunks = 0;
        let stopped = stream(
            Path::new("."),
            &["--list-cmds=main"],
            Instant::now() + Duration::from_secs(10),
            &cancel,
            |chunk| {
                assert!(!chunk.is_empty());
                chunks += 1;
                false
            },
        )
        .unwrap();
        assert_eq!(stopped, Stopped::Asked);
        assert_eq!(chunks, 1);
    }

    #[test]
    fn stream_gives_done_when_git_finishes() {
        let cancel = AtomicBool::new(false);
        let mut bytes = 0;
        let stopped = stream(
            Path::new("."),
            &["--list-cmds=main"],
            Instant::now() + Duration::from_secs(10),
            &cancel,
            |chunk| {
                bytes += chunk.len();
                true
            },
        )
        .unwrap();
        assert_eq!(stopped, Stopped::Done);
        assert!(bytes > 0);
    }

    #[test]
    fn stream_kills_a_child_that_prints_nothing_at_the_deadline() {
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        let deadline = started + Duration::from_millis(300);
        let stopped = stream_cmd(sleeper(), deadline, &cancel, |_| true).unwrap();
        assert_eq!(stopped, Stopped::Deadline);
        assert!(
            started.elapsed() < Duration::from_millis(300 + 500),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn stream_kills_the_child_when_cancelled() {
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        let stopped = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(200));
                cancel.store(true, Ordering::Relaxed);
            });
            stream_cmd(sleeper(), started + Duration::from_secs(10), &cancel, |_| true).unwrap()
        });
        assert_eq!(stopped, Stopped::Cancelled);
        assert!(
            started.elapsed() < Duration::from_millis(200 + 500),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn stream_with_the_flag_already_set_runs_nothing_long() {
        let cancel = AtomicBool::new(true);
        let started = Instant::now();
        let stopped = stream_cmd(sleeper(), started + Duration::from_secs(10), &cancel, |_| true).unwrap();
        assert_eq!(stopped, Stopped::Cancelled);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn stream_says_when_the_program_cannot_start() {
        let cancel = AtomicBool::new(false);
        let cmd = Command::new("marshell-no-such-program");
        let err = stream_cmd(cmd, Instant::now() + Duration::from_secs(1), &cancel, |_| true).unwrap_err();
        assert!(err.to_string().contains("isn't installed"), "{err}");
    }

    /// Git for Windows' `cmd\git.exe`, the one most PATHs hold, is a launcher that runs the real git as its child:
    /// a cancelled search must end that child too, not only the launcher.
    #[cfg(windows)]
    #[test]
    fn stream_ends_the_childs_own_children_when_cancelled() {
        fn pid_alive(pid: u32) -> bool {
            let out = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
                .output()
                .expect("tasklist runs");
            String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
        }
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid.txt");
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/c", "powershell", "-NoProfile", "-Command"]).arg(format!(
            "$PID | Set-Content -LiteralPath '{}'; Start-Sleep 60",
            pid_file.display()
        ));
        let cancel = AtomicBool::new(false);
        let stopped = std::thread::scope(|s| {
            s.spawn(|| {
                let started = Instant::now();
                while std::fs::read_to_string(&pid_file).map_or(true, |t| t.trim().is_empty()) {
                    assert!(
                        started.elapsed() < Duration::from_secs(20),
                        "the grandchild never wrote its PID"
                    );
                    std::thread::sleep(Duration::from_millis(50));
                }
                cancel.store(true, Ordering::Relaxed);
            });
            stream_cmd(cmd, Instant::now() + Duration::from_secs(30), &cancel, |_| true).unwrap()
        });
        assert_eq!(stopped, Stopped::Cancelled);
        let grandchild: u32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
        let started = Instant::now();
        while pid_alive(grandchild) {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the grandchild {grandchild} outlived the cancel"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
