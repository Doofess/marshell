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

/// Wraps an argument in quotes when it is empty or holds a space, a tab or a
/// quote. An inner quote becomes `\"`.
fn quote(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '"']) {
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

pub fn run(dir: &Path, args: &[&str]) -> anyhow::Result<GitOutput> {
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
    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!(NOT_INSTALLED)
        } else {
            anyhow::Error::new(e).context("Couldn't start git.")
        }
    })?;
    Ok(GitOutput {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
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
