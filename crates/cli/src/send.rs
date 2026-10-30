//! `marshell send <session> <text>`: puts text on a session's prompt line. Marshell never presses Enter, so the
//! person does that themselves.
//!
//! Exit codes: 0 filled, 1 refused, failed or the app is closed.

use crate::client::{Client, NOT_RUNNING, resolve};
use marshell_protocol::api::SessionInfo;
use std::io::{self, Write};

/// The refusal for a row that can't take text: its process or its agent has ended.
fn ended_message(row: &SessionInfo) -> Option<String> {
    (!row.running()).then(|| format!("Session {} has ended, so it can't take text.", row.number))
}

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
    // An ended or exited row, or one whose agent has ended, is refused here, so nothing is written to a dead pty.
    if let Some(ended) = ended_message(row) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ended_live, sample};
    use marshell_protocol::api::SessionState;

    #[test]
    fn an_ended_exited_or_agent_ended_row_is_refused() {
        let message = "Session 3 has ended, so it can't take text.";
        assert_eq!(
            ended_message(&sample(3, "t", SessionState::Ended)).as_deref(),
            Some(message)
        );
        assert_eq!(
            ended_message(&sample(3, "t", SessionState::Exited { code: 0 })).as_deref(),
            Some(message)
        );
        let mut agent_ended = sample(3, "t", SessionState::Running);
        assert_eq!(ended_message(&agent_ended), None);
        agent_ended.live = Some(ended_live());
        assert_eq!(ended_message(&agent_ended).as_deref(), Some(message));
    }
}
