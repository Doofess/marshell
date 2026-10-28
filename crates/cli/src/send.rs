//! `marshell send <session> <text>`: puts text on a session's prompt line. Marshell never presses Enter, so the
//! person does that themselves.
//!
//! Exit codes: 0 filled, 1 refused, failed or the app is closed.

use crate::client::{Client, NOT_RUNNING, resolve};
use marshell_protocol::api::SessionState;
use std::io::{self, Write};

pub fn run(
    env: &dyn Fn(&str) -> Option<String>,
    session: &str,
    text: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let Ok(client) = Client::connect(env) else {
        let _ = writeln!(err, "{NOT_RUNNING}");
        return 1;
    };
    let snapshot = match client.sessions() {
        Ok(snapshot) => snapshot,
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            let _ = writeln!(err, "{NOT_RUNNING}");
            return 1;
        }
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let Some(row) = resolve(&snapshot, session) else {
        let _ = writeln!(err, "No session {session}.");
        return 1;
    };
    let number = row.number;
    let ended = format!("Session {number} has ended, so it can't take text.");
    // An ended or exited row is refused here, so nothing is ever written to a dead pty.
    if row.state != SessionState::Running {
        let _ = writeln!(err, "{ended}");
        return 1;
    }
    // `send` fills the line at once: only `new --prompt` waits for the agent.
    match client.fill(&row.tab_id, text, false) {
        Ok(()) => {
            let _ = writeln!(out, "Filled session {number}'s prompt line.");
            0
        }
        Err(refusal) => {
            let _ = writeln!(err, "{}", refusal.message(number));
            1
        }
    }
}
