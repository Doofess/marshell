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
