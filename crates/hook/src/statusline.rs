//! The status line role: the user's own status line command still runs and still prints, and the
//! core gets a copy of its input on the side. What the person sees never depends on the core.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Instant;

use marshell_protocol::{brand, hook::HOOK_BODY_LIMIT, hook::HookRole};

use crate::args::Args;
use crate::post::{self, PostError};
use crate::{log, shell};

type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

const ROLE: &str = "statusline";

/// Runs the original status line command named by the chain file and returns the bytes to print:
/// its stdout when it exits 0, otherwise nothing, which is what Claude shows for a failing command.
/// The forward to the core runs beside it and is waited for only until its 100 ms budget is spent.
/// `home` is `None` when there is nowhere to find a chain file, and then nothing is printed.
pub fn run(home: Option<&Path>, agent: &str, stdin_bytes: &[u8], env: Env) -> Vec<u8> {
    let fail = |msg: &str| {
        if let Some(home) = home {
            log::line(home, ROLE, msg);
        }
        Vec::new()
    };
    let Some(home) = home else {
        return Vec::new();
    };
    let command = match read_chain(home, agent) {
        Ok(c) => c,
        Err(e) => return fail(&e),
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

    let out = run_original(&command, stdin_bytes, env);

    // The forward gets what is left of its budget and no more; a late one is left behind and dies
    // with the process.
    let _ = done_rx.recv_timeout(budget.saturating_sub(started.elapsed()));

    match out {
        Ok(bytes) => bytes,
        Err(e) => fail(&e),
    }
}

/// `statusLine.command` from `<home>/agents/<agent>/statusline-chain.json`.
fn read_chain(home: &Path, agent: &str) -> Result<String, String> {
    let path = home.join("agents").join(agent).join("statusline-chain.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read the chain file: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "the chain file is not valid JSON".to_string())?;
    json.pointer("/statusLine/command")
        .and_then(|c| c.as_str())
        .filter(|c| !c.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "the chain file has no statusLine.command".to_string())
}

/// Runs `command` through the resolved shell with `stdin_bytes` on its stdin. `Ok` holds its stdout
/// for an exit 0. No output text and no input appear in an error.
fn run_original(command: &str, stdin_bytes: &[u8], env: Env) -> Result<Vec<u8>, String> {
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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes for each refresh of the status line.
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot start the original status line ({}): {e}", sh.program.display()))?;
    // Written on a thread: the child may print a lot before it reads, and both pipes are bounded.
    if let Some(mut stdin) = child.stdin.take() {
        let bytes = stdin_bytes.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("cannot wait for the original status line: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(out.stdout),
        Some(code) => Err(format!("the original status line exited with {code}")),
        None => Err("the original status line was ended by a signal".to_string()),
    }
}
