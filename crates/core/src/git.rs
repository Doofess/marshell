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
    stream_command(command(dir, args), deadline, cancel, each)
}

/// Reads `r` to its end and sends each chunk. A read error is sent too (an interrupted read is
/// retried), and ends the reading, so the receiver can tell a failed read from the end of the output.
fn pump(mut r: impl std::io::Read, tx: std::sync::mpsc::SyncSender<std::io::Result<Vec<u8>>>) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send(Ok(buf[..n].to_vec())).is_err() {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                let _ = tx.send(Err(e));
                break;
            }
        }
    }
}

/// How much of git's stderr is kept for the sentence of a failed run.
const STDERR_TAIL: usize = 4096;

/// Reads `r` to its end and keeps the last `STDERR_TAIL` bytes in `tail`.
fn keep_tail(mut r: impl std::io::Read, tail: &std::sync::Mutex<Vec<u8>>) {
    let mut buf = [0u8; 4096];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let Ok(mut kept) = tail.lock() else { break };
                kept.extend_from_slice(&buf[..n]);
                if kept.len() > STDERR_TAIL {
                    let over = kept.len() - STDERR_TAIL;
                    kept.drain(..over);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

/// `stream` for any command: the sandboxed git of Changes runs through it, and the tests run one that
/// prints nothing. A failed run (an exit code other than 0 or 1) says why, in `said`'s words.
pub(crate) fn stream_command(
    mut cmd: Command,
    deadline: std::time::Instant,
    cancel: &std::sync::atomic::AtomicBool,
    mut each: impl FnMut(&[u8]) -> bool,
) -> anyhow::Result<Stopped> {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{RecvTimeoutError, sync_channel};
    use std::time::Instant;

    if cancel.load(Ordering::Relaxed) {
        return Ok(Stopped::Cancelled);
    }
    let mut child = Reaped::new(
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(start_error)?,
    );
    let out = child.0.stdout.take().expect("stdout is piped");
    // Git's stderr is read on its own thread too, so a chatty git never blocks on a full pipe. Only the
    // last part is kept: the sentence that explains a failure comes last.
    let err = child.0.stderr.take().expect("stderr is piped");
    let tail = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let reader = {
        let tail = tail.clone();
        std::thread::spawn(move || keep_tail(err, &tail))
    };
    // The pipe is read on its own thread, so this one can watch the clock and the flag while git is
    // silent. The small queue holds the reader back when `each` is slow.
    let (tx, rx) = sync_channel::<std::io::Result<Vec<u8>>>(4);
    std::thread::spawn(move || pump(out, tx));
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Stopped::Cancelled);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(Stopped::Deadline);
        }
        match rx.recv_timeout(STREAM_TICK.min(deadline - now)) {
            // A failed read is not the end of git's output: what came before it is only part of it.
            Ok(Err(e)) => return Err(anyhow::Error::new(e).context("Git's output could not be read.")),
            Ok(Ok(chunk)) => {
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
                    Some(code) => {
                        // The pipe closes with git; a grandchild that kept it open doesn't hold this up.
                        let waited = Instant::now();
                        while !reader.is_finished() && waited.elapsed() < std::time::Duration::from_millis(200) {
                            std::thread::sleep(std::time::Duration::from_millis(2));
                        }
                        let stderr = tail
                            .lock()
                            .map(|t| String::from_utf8_lossy(&t).into_owned())
                            .unwrap_or_default();
                        let out = GitOutput {
                            ok: false,
                            stdout: String::new(),
                            stderr,
                        };
                        Err(anyhow::anyhow!("Git stopped with exit code {code}: {}", said(&out)))
                    }
                    None => Err(anyhow::anyhow!("Git was stopped.")),
                };
            }
            None if cancel.load(Ordering::Relaxed) => return Ok(Stopped::Cancelled),
            None if Instant::now() >= deadline => return Ok(Stopped::Deadline),
            None => std::thread::sleep(std::time::Duration::from_millis(2)),
        }
    }
}

/// Which of a watched run's pipes a chunk came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pipe {
    Out,
    Err,
}

/// How a watched run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The program exited. Any exit code is an answer, not an error; `None` is an end with no code.
    Exited(Option<i32>),
    /// The cancel flag was set, and the program and everything it started were killed.
    Cancelled,
}

/// How long a watched run waits for the pipes after the program has ended.
const PIPE_GRACE: std::time::Duration = std::time::Duration::from_millis(200);

/// Reads `r` to its end and sends each chunk with its pipe. A read error ends the reading.
fn pump_pipe(mut r: impl std::io::Read, pipe: Pipe, tx: std::sync::mpsc::Sender<(Pipe, Vec<u8>)>) {
    let mut buf = [0u8; 8192];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send((pipe, buf[..n].to_vec())).is_err() {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

/// Runs `cmd` with no deadline, no window and no standard input, and hands `each` every chunk of its output as it
/// arrives (two reader threads, one channel, `each` called on this thread). Any exit code is `Exited`: 1 is not
/// an error here. The cancel flag is looked at every 50 ms; when set while the program runs, it and everything it started are
/// killed and the result is `Cancelled`. Once the program has ended, the pipes get `PIPE_GRACE` more, then the run
/// returns even if something it started still holds a pipe: that is left running, and its reader ends on its own.
// Used by github::push (slice 6r-4a, Task 9).
#[allow(dead_code)]
pub(crate) fn run_watched(
    mut cmd: Command,
    cancel: &std::sync::atomic::AtomicBool,
    mut each: impl FnMut(Pipe, &[u8]),
) -> anyhow::Result<Ended> {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use std::time::{Duration, Instant};

    if cancel.load(Ordering::Relaxed) {
        return Ok(Ended::Cancelled);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let (mut child, tree) = crate::job::ProcessTree::spawn(&mut cmd).map_err(start_error)?;
    let (tx, rx) = channel::<(Pipe, Vec<u8>)>();
    let out = child.stdout.take().expect("stdout is piped");
    let err = child.stderr.take().expect("stderr is piped");
    let out_tx = tx.clone();
    std::thread::spawn(move || pump_pipe(out, Pipe::Out, out_tx));
    std::thread::spawn(move || pump_pipe(err, Pipe::Err, tx));
    let mut exit = None;
    let mut ended_at = None;
    let mut pipes_open = true;
    loop {
        if exit.is_none() && cancel.load(Ordering::Relaxed) {
            // A program that has already ended is past stopping, and its exit is the answer: a push may
            // have moved the branch.
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit = Some(status);
                    ended_at = Some(Instant::now());
                }
                _ => {
                    tree.kill(&mut child);
                    let _ = child.wait();
                    return Ok(Ended::Cancelled);
                }
            }
        }
        if pipes_open {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok((pipe, chunk)) => each(pipe, &chunk),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => pipes_open = false,
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
        if exit.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit = Some(status);
                    ended_at = Some(Instant::now());
                }
                Ok(None) => {}
                Err(e) => {
                    tree.kill(&mut child);
                    let _ = child.wait();
                    return Err(anyhow::Error::new(e).context("Couldn't wait for the program."));
                }
            }
        }
        if let Some(status) = exit
            && (!pipes_open || ended_at.is_some_and(|at| at.elapsed() >= PIPE_GRACE))
        {
            // What the readers already sent is handed over before returning.
            while let Ok((pipe, chunk)) = rx.try_recv() {
                each(pipe, &chunk);
            }
            return Ok(Ended::Exited(status.code()));
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

    /// A reader that gives some bytes, then fails.
    struct Failing(bool);

    impl std::io::Read for Failing {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.0 {
                return Err(std::io::Error::other("x"));
            }
            self.0 = true;
            buf[..2].copy_from_slice(b"a\0");
            Ok(2)
        }
    }

    #[test]
    fn a_read_error_ends_the_stream_as_an_error() {
        let (tx, rx) = std::sync::mpsc::sync_channel(4);
        pump(Failing(false), tx);
        assert_eq!(rx.recv().unwrap().unwrap(), b"a\0");
        assert_eq!(rx.recv().unwrap().unwrap_err().to_string(), "x");
        assert!(rx.recv().is_err());
    }

    #[test]
    fn a_failed_run_says_what_git_said() {
        let cancel = AtomicBool::new(false);
        let err = stream(
            Path::new("."),
            &["rev-parse", "--verify", "no-such-revision-marshell"],
            Instant::now() + Duration::from_secs(10),
            &cancel,
            |_| true,
        )
        .unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("Git stopped with exit code 128: "), "{text}");
        assert!(!text.contains("fatal:"), "the prefix is dropped: {text}");
        assert!(text.len() > "Git stopped with exit code 128: ".len(), "{text}");
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
        let stopped = stream_command(sleeper(), deadline, &cancel, |_| true).unwrap();
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
            stream_command(sleeper(), started + Duration::from_secs(10), &cancel, |_| true).unwrap()
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
        let stopped = stream_command(sleeper(), started + Duration::from_secs(10), &cancel, |_| true).unwrap();
        assert_eq!(stopped, Stopped::Cancelled);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn stream_says_when_the_program_cannot_start() {
        let cancel = AtomicBool::new(false);
        let cmd = Command::new("marshell-no-such-program");
        let err = stream_command(cmd, Instant::now() + Duration::from_secs(1), &cancel, |_| true).unwrap_err();
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
            stream_command(cmd, Instant::now() + Duration::from_secs(30), &cancel, |_| true).unwrap()
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

    /// `script` run by the system shell: `cmd /c` on Windows (its text goes in as written), `sh -c` elsewhere.
    fn shell(script: &str) -> Command {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("cmd");
            cmd.raw_arg(format!("/c {script}"));
            cmd
        }
        #[cfg(not(windows))]
        {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", script]);
            cmd
        }
    }

    #[test]
    fn watched_reads_both_pipes() {
        let cancel = AtomicBool::new(false);
        let (mut out, mut err) = (String::new(), String::new());
        let script = if cfg!(windows) {
            "echo to-out&echo to-err 1>&2"
        } else {
            "echo to-out; echo to-err >&2"
        };
        let ended = run_watched(shell(script), &cancel, |pipe, chunk| match pipe {
            Pipe::Out => out.push_str(&String::from_utf8_lossy(chunk)),
            Pipe::Err => err.push_str(&String::from_utf8_lossy(chunk)),
        })
        .unwrap();
        assert_eq!(ended, Ended::Exited(Some(0)));
        assert!(out.contains("to-out") && !out.contains("to-err"), "{out:?}");
        assert!(err.contains("to-err") && !err.contains("to-out"), "{err:?}");
    }

    #[test]
    fn watched_reports_exit_code_one() {
        let cancel = AtomicBool::new(false);
        let ended = run_watched(shell("exit 1"), &cancel, |_, _| {}).unwrap();
        assert_eq!(ended, Ended::Exited(Some(1)));
    }

    /// A push that has already ended can't be called cancelled: it may have moved the branch.
    #[test]
    fn watched_cancel_after_the_program_ended_reports_its_exit() {
        let cancel = AtomicBool::new(false);
        let ended = run_watched(shell("echo x& exit 3"), &cancel, |_, _| {
            // By the time this returns the program is long gone, and the flag is seen on the next turn.
            std::thread::sleep(std::time::Duration::from_millis(300));
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        })
        .unwrap();
        assert_eq!(ended, Ended::Exited(Some(3)));
    }

    #[test]
    fn watched_says_when_the_program_cannot_start() {
        let cancel = AtomicBool::new(false);
        let cmd = Command::new("marshell-no-such-program");
        assert!(run_watched(cmd, &cancel, |_, _| {}).is_err());
    }

    /// This test binary, run as a program that starts a long-lived grandchild (`ping`, which shares this
    /// program's pipes) and prints `PID <n>` for it. `mode` `hold` then exits and `wait` waits for the grandchild.
    #[cfg(windows)]
    fn grandchild_maker(mode: &str) -> Command {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["--ignored", "--exact", "git::tests::grandchild_helper", "--nocapture"])
            .env("MARSHELL_TEST_GRANDCHILD", mode);
        cmd
    }

    /// Not a test: the program `grandchild_maker` starts. It does nothing in an ordinary run.
    #[cfg(windows)]
    #[test]
    #[ignore = "run only as a child process of the tests that start it"]
    fn grandchild_helper() {
        use std::io::Write;
        let Ok(mode) = std::env::var("MARSHELL_TEST_GRANDCHILD") else {
            return;
        };
        // In `hold` mode the grandchild is left running on purpose; the test that started this program ends it.
        #[allow(clippy::zombie_processes)]
        let mut ping = Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdin(Stdio::null())
            .spawn()
            .expect("could not start ping");
        println!("PID {}", ping.id());
        std::io::stdout().flush().unwrap();
        if mode == "wait" {
            let _ = ping.wait();
        }
    }

    /// The `PID <n>` line `grandchild_helper` printed, once all of the line has arrived.
    #[cfg(windows)]
    fn pid_in(text: &str) -> Option<u32> {
        text.split_inclusive('\n')
            .filter(|l| l.ends_with('\n'))
            .find_map(|l| l.strip_prefix("PID ")?.trim().parse().ok())
    }

    #[cfg(windows)]
    fn ping_alive(pid: u32) -> bool {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .expect("tasklist runs");
        String::from_utf8_lossy(&out.stdout)
            .to_ascii_uppercase()
            .contains("PING.EXE")
    }

    /// Ends exactly the `ping` it was told about, by its PID, when the test is over, a failed test included.
    #[cfg(windows)]
    #[derive(Default)]
    struct EndPing(std::cell::Cell<Option<u32>>);

    #[cfg(windows)]
    impl Drop for EndPing {
        fn drop(&mut self) {
            if let Some(pid) = self.0.get()
                && ping_alive(pid)
            {
                let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output();
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn watched_returns_when_a_grandchild_keeps_a_pipe_open() {
        let cancel = AtomicBool::new(false);
        let ping = EndPing::default();
        let mut text = String::new();
        let started = Instant::now();
        let ended = run_watched(grandchild_maker("hold"), &cancel, |pipe, chunk| {
            if pipe == Pipe::Out {
                text.push_str(&String::from_utf8_lossy(chunk));
                if let Some(pid) = pid_in(&text) {
                    ping.0.set(Some(pid));
                }
            }
        })
        .unwrap();
        let took = started.elapsed();
        assert_eq!(ended, Ended::Exited(Some(0)));
        assert!(took < Duration::from_secs(2), "took {took:?}");
        let pid = ping.0.get().expect("the helper never printed the grandchild's PID");
        assert!(
            ping_alive(pid),
            "the grandchild was ended, and the run must leave it alone"
        );
    }

    #[cfg(windows)]
    #[test]
    fn watched_cancel_ends_a_grandchild_started_at_once() {
        let cancel = AtomicBool::new(false);
        let ping = EndPing::default();
        let mut text = String::new();
        let ended = run_watched(grandchild_maker("wait"), &cancel, |pipe, chunk| {
            if pipe == Pipe::Out {
                text.push_str(&String::from_utf8_lossy(chunk));
                if let Some(pid) = pid_in(&text) {
                    ping.0.set(Some(pid));
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        })
        .unwrap();
        assert_eq!(ended, Ended::Cancelled);
        let pid = ping.0.get().expect("the helper never printed the grandchild's PID");
        let started = Instant::now();
        while ping_alive(pid) {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the grandchild {pid} outlived the cancel"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
