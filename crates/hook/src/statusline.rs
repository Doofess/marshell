//! The status line role: the user's own status line command still runs and still prints, and the
//! core gets a copy of its input on the side. What the person sees never depends on the core.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use marshell_protocol::{brand, hook::HOOK_BODY_LIMIT, hook::HookRole};

use crate::args::Args;
use crate::post::{self, PostError};
use crate::{job, log, shell};

type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

const ROLE: &str = "statusline";

/// How long the original may run. Claude Code cancels a status line that is still running when the
/// next one starts, so a hung one would otherwise pile up processes.
const ORIGINAL_TIMEOUT: Duration = Duration::from_secs(10);

/// What marks a command as Marshell's own: it is the bridge calling itself if the chain holds one.
const OWN_TAG: &str = "--tag mshl1";

/// Runs the original status line command named by the chain file and returns the bytes to print:
/// its stdout when it exits 0, otherwise nothing, which is what Claude shows for a failing command.
/// A hung original is stopped with its whole process tree after `ORIGINAL_TIMEOUT`. An empty chain
/// (the person had no status line) prints nothing and logs nothing.
/// The forward to the core runs beside it, whatever the chain holds (the core reads context and plan
/// use from it), and is waited for only until its 100 ms budget is spent.
/// `home` is `None` when there is nowhere to find a chain file, and then nothing is printed.
pub fn run(home: Option<&Path>, agent: &str, stdin_bytes: &[u8], env: Env) -> Vec<u8> {
    let Some(home) = home else {
        return Vec::new();
    };
    let started = Instant::now();
    let budget = post::budget(HookRole::Statusline);
    let (done_tx, done_rx) = channel::<()>();
    if let Some((_, port, token)) = crate::endpoint::discover(env)
        && stdin_bytes.len() <= HOOK_BODY_LIMIT
    {
        let args = Args {
            role: HookRole::Statusline,
            agent: agent.to_string(),
            tag: None,
        };
        let tab = env(&brand::env_var("TAB_ID")).filter(|t| !t.is_empty());
        let body = stdin_bytes.to_vec();
        let home: PathBuf = home.to_path_buf();
        std::thread::spawn(move || {
            match post::post(port, &token, &args, tab.as_deref(), &body, budget) {
                Ok((status, _)) if !(200..300).contains(&status) => {
                    log::line(&home, ROLE, &format!("the core answered {status}"));
                }
                Err(PostError::Failed(msg)) => log::line(&home, ROLE, &msg.replace(&token, "***")),
                Ok(_) | Err(PostError::Closed) => {}
            }
            let _ = done_tx.send(());
        });
    } else {
        drop(done_tx);
    }

    let out = match read_chain(home, agent) {
        Ok(Some(command)) if command.contains(OWN_TAG) => {
            // Running it would start this bridge again, which would read the same chain, and so on.
            Err("the chain names Marshell's own status line, so it was not run".to_string())
        }
        Ok(Some(command)) => run_original(&command, stdin_bytes, env, ORIGINAL_TIMEOUT),
        Ok(None) => Ok(Vec::new()),
        Err(e) => Err(e),
    };

    // The forward gets what is left of its budget and no more; a late one is left behind and dies
    // with the process.
    let _ = done_rx.recv_timeout(budget.saturating_sub(started.elapsed()));

    match out {
        Ok(bytes) => bytes,
        Err(e) => {
            log::line(home, ROLE, &e);
            Vec::new()
        }
    }
}

/// `statusLine.command` from `<home>/agents/<agent>/statusline-chain.json`. `Ok(None)` is a chain
/// that says `"statusLine": null`: the person had no status line of their own.
pub fn read_chain(home: &Path, agent: &str) -> Result<Option<String>, String> {
    let path = home.join("agents").join(agent).join("statusline-chain.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read the chain file: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "the chain file is not valid JSON".to_string())?;
    if json.get("statusLine").is_some_and(serde_json::Value::is_null) {
        return Ok(None);
    }
    json.pointer("/statusLine/command")
        .and_then(|c| c.as_str())
        .filter(|c| !c.trim().is_empty())
        .map(|c| Some(c.to_string()))
        .ok_or_else(|| "the chain file has no statusLine.command".to_string())
}

/// Runs `command` through the resolved shell with `stdin_bytes` on its stdin. `Ok` holds its stdout
/// for an exit 0. After `timeout` the command and everything it started are ended and the result is
/// an error. No output text and no input appear in an error.
fn run_original(command: &str, stdin_bytes: &[u8], env: Env, timeout: Duration) -> Result<Vec<u8>, String> {
    let path_var = env("PATH").or_else(|| env("Path"));
    let sh = shell::resolve(env, &|p| p.is_file(), shell::find_git_on_path(path_var.as_deref()));
    let mut cmd = Command::new(&sh.program);
    cmd.args(&sh.args_before_command)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for var in ["COLUMNS", "LINES"] {
        if let Some(v) = env(var) {
            cmd.env(var, v);
        }
    }
    // Without a job a hung command is still killed, but not what it started. That is a fallback, not
    // a reason to show no status line.
    let job = job::Job::new().ok();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes for each refresh of the status line.
        // CREATE_SUSPENDED: the command runs only once it is in the job, so nothing it starts escapes.
        let suspended = if job.is_some() { 0x0000_0004 } else { 0 };
        cmd.creation_flags(0x0800_0000 | suspended);
    }
    let deadline = Instant::now() + timeout;
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot start the original status line ({}): {e}", sh.program.display()))?;
    if let Some(job) = &job {
        // A failed assignment leaves the command to run outside the job; the kill below still reaches it.
        let _ = job.assign(&child);
        if let Err(e) = job::resume(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("cannot start the original status line: {e}"));
        }
    }
    // Written on a thread: the child may print a lot before it reads, and both pipes are bounded.
    if let Some(mut stdin) = child.stdin.take() {
        let bytes = stdin_bytes.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    // Read on a thread too, so the wait below can give up on time. It ends when every holder of the pipe has.
    let (out_tx, out_rx) = channel::<Vec<u8>>();
    if let Some(mut stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            let _ = out_tx.send(bytes);
        });
    }
    let too_slow = || format!("the original status line took longer than {}", describe(timeout));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                if let Some(job) = &job {
                    job.terminate();
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err(too_slow());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(format!("cannot wait for the original status line: {e}")),
        }
    };
    // Whatever it left running goes with the job, which also lets the stdout pipe close.
    drop(job);
    let stdout = out_rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| too_slow())?;
    match status.code() {
        Some(0) => Ok(stdout),
        Some(code) => Err(format!("the original status line exited with {code}")),
        None => Err("the original status line was ended by a signal".to_string()),
    }
}

/// `10 s`, or `300 ms` for a limit that is not whole seconds (tests only).
fn describe(d: Duration) -> String {
    if d.subsec_millis() == 0 {
        format!("{} s", d.as_secs())
    } else {
        format!("{} ms", d.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hung_original_is_cut_off() {
        let path = std::env::var("PATH").unwrap_or_default();
        let env = move |k: &str| (k == "PATH").then(|| path.clone());
        // The shell is Git Bash even on Windows, where `>NUL` would create a file named NUL.
        let command = if cfg!(windows) {
            "ping -n 30 127.0.0.1 >/dev/null"
        } else {
            "sleep 30"
        };
        let started = Instant::now();
        let result = run_original(command, b"{}", &env, Duration::from_millis(300));
        let took = started.elapsed();
        let err = result.expect_err("a hung command has no output");
        assert!(err.contains("took longer than"), "{err}");
        assert!(took < Duration::from_secs(2), "took {took:?}");
    }

    /// What the original started dies with it: the background job would write the file after 2 s.
    #[cfg(windows)]
    #[test]
    fn a_cut_off_original_takes_its_children_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("survived").display().to_string().replace('\\', "/");
        let path = std::env::var("PATH").unwrap_or_default();
        let env = move |k: &str| (k == "PATH").then(|| path.clone());
        let command = format!("(sleep 2; touch '{marker}') & sleep 30");
        assert!(run_original(&command, b"{}", &env, Duration::from_millis(500)).is_err());
        std::thread::sleep(Duration::from_secs(3));
        assert!(!dir.path().join("survived").exists(), "a child outlived the cut-off");
    }
}
