//! The bridge between an agent CLI's hooks and the Marshell core. Every failure exits 0 with no
//! output and one line in `<home>/logs/bridge.log`; the token and the stdin body are never logged.

pub mod args;
mod codex_daemon;
pub mod endpoint;
mod job;
pub mod log;
mod permission;
pub mod post;
pub mod record;
pub mod shell;
pub mod statusline;

use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use marshell_protocol::{brand, hook::HookRole};

use args::Args;
use post::PostError;

type Env<'a> = &'a dyn Fn(&str) -> Option<String>;
type Ancestors<'a> = &'a dyn Fn() -> Vec<String>;

/// Runs one bridge call and returns the bytes to print. `argv` is the arguments after the program
/// name. The caller prints the result and exits 0; nothing else sets the exit code.
pub fn run(argv: &[String], stdin: &mut dyn Read, env: Env) -> Vec<u8> {
    run_with(argv, stdin, env, &codex_daemon::ancestors)
}

/// `run`, with this process's ancestors' image paths (parent first) read by `ancestors`.
pub fn run_with(argv: &[String], stdin: &mut dyn Read, env: Env, ancestors: Ancestors) -> Vec<u8> {
    let home = endpoint::home(env);
    let args = match args::parse(argv) {
        Ok(a) => a,
        Err(e) => {
            if let Some(home) = &home {
                log::line(home, "-", &format!("bad arguments: {e}"));
            }
            return Vec::new();
        }
    };
    catch_unwind(AssertUnwindSafe(|| call(&args, stdin, env, ancestors, home.as_deref()))).unwrap_or_else(|_| {
        if let Some(home) = &home {
            log::line(home, args.role.as_str(), "internal error");
        }
        Vec::new()
    })
}

fn call(args: &Args, stdin: &mut dyn Read, env: Env, ancestors: Ancestors, home: Option<&Path>) -> Vec<u8> {
    let role = args.role;
    let fail = |msg: &str| {
        if let Some(home) = home {
            log::line(home, role.as_str(), msg);
        }
        Vec::new()
    };
    if role == HookRole::Statusline {
        // The original's own stdin is never capped: only the copy sent to the core is.
        let mut bytes = Vec::new();
        if let Err(e) = stdin.read_to_end(&mut bytes) {
            return fail(&format!("cannot read stdin: {e}"));
        }
        return statusline::run(home, &args.agent, &bytes, env);
    }
    let body = match post::read_capped(stdin) {
        Ok(Some(b)) => b,
        Ok(None) => return fail("stdin is over 1 MiB, nothing posted"),
        Err(e) => return fail(&format!("cannot read stdin: {e}")),
    };
    if role == HookRole::Record
        && let Some(home) = home
    {
        let dir = record::dir(env, home, &args.agent);
        if let Err(e) = record::save(&dir, role.as_str(), &body) {
            fail(&format!("cannot save the recording: {e}"));
        }
    }
    let tab = env(&brand::env_var("TAB_ID"))
        .filter(|t| !t.is_empty())
        // Codex's shared daemon keeps the tab id of the Codex that started it (slice 6a decision 12).
        .filter(|_| !(args.agent == "codex" && codex_daemon::runs_in_daemon(&ancestors())));
    if role == HookRole::Permission && tab.is_none() {
        // The core answers a request from outside Marshell with 204, so there is nothing to ask.
        return Vec::new();
    }
    let Some((_, port, token)) = endpoint::discover(env) else {
        return Vec::new();
    };
    let budget = post::budget(role);
    match post::post(port, &token, args, tab.as_deref(), &body, budget) {
        Ok((200, bytes)) if role == HookRole::Permission => {
            if permission::accepted(&bytes, &body) {
                bytes
            } else {
                fail("the answer is not one Marshell gives, nothing printed")
            }
        }
        Ok((200, bytes)) if role == HookRole::Context => bytes,
        Ok((status, _)) if (200..300).contains(&status) => Vec::new(),
        Ok((status, _)) => fail(&format!("the core answered {status}")),
        Err(PostError::Closed) => Vec::new(),
        Err(PostError::Failed(msg)) => fail(&msg.replace(&token, "***")),
    }
}
