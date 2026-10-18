//! The bridge between an agent CLI's hooks and the Marshell core. Every failure exits 0 with no
//! output and one line in `<home>/logs/bridge.log`; the token and the stdin body are never logged.

pub mod args;
pub mod endpoint;
mod job;
pub mod log;
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

/// Runs one bridge call and returns the bytes to print. `argv` is the arguments after the program
/// name. The caller prints the result and exits 0; nothing else sets the exit code.
pub fn run(argv: &[String], stdin: &mut dyn Read, env: Env) -> Vec<u8> {
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
    catch_unwind(AssertUnwindSafe(|| call(&args, stdin, env, home.as_deref()))).unwrap_or_else(|_| {
        if let Some(home) = &home {
            log::line(home, args.role.as_str(), "internal error");
        }
        Vec::new()
    })
}

fn call(args: &Args, stdin: &mut dyn Read, env: Env, home: Option<&Path>) -> Vec<u8> {
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
    let Some((_, port, token)) = endpoint::discover(env) else {
        return Vec::new();
    };
    let tab = env(&brand::env_var("TAB_ID")).filter(|t| !t.is_empty());
    let budget = post::budget(role);
    match post::post(port, &token, args, tab.as_deref(), &body, budget) {
        Ok((200, bytes)) if matches!(role, HookRole::Context | HookRole::Permission) => bytes,
        Ok((status, _)) if (200..300).contains(&status) => Vec::new(),
        Ok((status, _)) => fail(&format!("the core answered {status}")),
        Err(PostError::Closed) => Vec::new(),
        Err(PostError::Failed(msg)) => fail(&msg.replace(&token, "***")),
    }
}
