//! `marshell list [--json]`: the open sessions of the running app.
//!
//! Exit codes: 0 listed, 1 the app is closed or could not be reached.

use crate::client::{Client, NOT_RUNNING};
use marshell_protocol::api::{SessionInfo, SessionState};
use serde::Serialize;
use std::io::{self, Write};

/// The table: a header and one row per session, padded with spaces. `"No sessions."` when there are none.
pub fn table(sessions: &[SessionInfo]) -> String {
    if sessions.is_empty() {
        return "No sessions.".to_string();
    }
    let mut rows = vec![["#", "NAME", "AGENT", "STATE", "MODE", "CWD"].map(String::from)];
    for s in sessions {
        rows.push([
            s.number.to_string(),
            s.name.clone(),
            s.agent.clone().unwrap_or_else(|| "-".into()),
            state_word(s).to_string(),
            mode_word(s).unwrap_or_else(|| "-".into()),
            s.cwd.clone(),
        ]);
    }
    let widths: Vec<usize> = (0..6)
        .map(|c| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0))
        .collect();
    rows.iter()
        .map(|row| {
            let line: Vec<String> = row
                .iter()
                .zip(&widths)
                .map(|(cell, w)| format!("{cell:<w$}", w = *w))
                .collect();
            line.join("  ").trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The sessions as a JSON array, one object per session.
pub fn json(sessions: &[SessionInfo]) -> String {
    #[derive(Serialize)]
    struct Row<'a> {
        number: u32,
        tab_id: &'a str,
        name: &'a str,
        agent: Option<&'a str>,
        state: &'static str,
        cwd: &'a str,
        project: &'a str,
        mode: Option<String>,
    }
    let rows: Vec<Row> = sessions
        .iter()
        .map(|s| Row {
            number: s.number,
            tab_id: &s.tab_id,
            name: &s.name,
            agent: s.agent.as_deref(),
            state: state_word(s),
            cwd: &s.cwd,
            project: &s.project,
            mode: mode_word(s),
        })
        .collect();
    serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())
}

fn state_word(s: &SessionInfo) -> &'static str {
    match s.state {
        SessionState::Running if !s.running() => "ended",
        SessionState::Running => "running",
        SessionState::Exited { .. } => "exited",
        SessionState::Ended => "ended",
    }
}

/// The mode as the protocol spells it (`auto-edit`, `full-auto`), or `None` when there is none.
fn mode_word(s: &SessionInfo) -> Option<String> {
    let value = serde_json::to_value(s.mode?).ok()?;
    value.as_str().map(str::to_string)
}

pub fn run(env: &dyn Fn(&str) -> Option<String>, json_output: bool, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Ok(client) = Client::connect(env) else {
        let _ = writeln!(err, "{NOT_RUNNING}");
        return 1;
    };
    match client.sessions() {
        Ok(snapshot) => {
            let text = if json_output {
                json(&snapshot.sessions)
            } else {
                table(&snapshot.sessions)
            };
            let _ = writeln!(out, "{text}");
            0
        }
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            let _ = writeln!(err, "{NOT_RUNNING}");
            1
        }
        Err(e) => {
            let _ = writeln!(err, "{e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ended_live;
    use crate::client::sample;
    use marshell_protocol::api::SessionState;

    #[test]
    fn list_table_and_json() {
        let mut shell = sample(2, "tab-b", SessionState::Exited { code: 3 });
        shell.agent = None;
        shell.mode = None;
        shell.name = "build".into();
        let sessions = vec![sample(1, "tab-a", SessionState::Running), shell];

        let text = table(&sessions);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert!(
            lines[0].starts_with('#') && lines[0].contains("NAME") && lines[0].contains("STATE"),
            "{text}"
        );
        assert!(
            lines[1].contains("Session 1") && lines[1].contains("claude") && lines[1].contains("running"),
            "{text}"
        );
        assert!(lines[2].contains("build") && lines[2].contains("exited"), "{text}");
        // Columns line up: every cell starts where the header's does.
        let name_col = lines[0].find("NAME").unwrap();
        assert_eq!(lines[1].find("Session 1"), Some(name_col));
        assert_eq!(lines[2].find("build"), Some(name_col));
        assert!(lines.iter().all(|l| l == &l.trim_end()), "no trailing spaces");

        assert_eq!(table(&[]), "No sessions.");

        let parsed: serde_json::Value = serde_json::from_str(&json(&sessions)).unwrap();
        assert_eq!(
            parsed[0],
            serde_json::json!({
                "number": 1, "tab_id": "tab-a", "name": "Session 1", "agent": "claude", "state": "running",
                "cwd": "C:\\work\\app", "project": "C:\\work", "mode": "plan",
            })
        );
        assert_eq!(parsed[1]["state"], "exited");
        assert!(parsed[1]["agent"].is_null() && parsed[1]["mode"].is_null());
        assert_eq!(json(&[]).trim(), "[]");
    }

    #[test]
    fn a_running_row_whose_agent_ended_is_state_ended() {
        let mut row = sample(5, "t", SessionState::Running);
        row.live = Some(ended_live());
        assert_eq!(state_word(&row), "ended");
        assert!(table(&[row.clone()]).contains("ended"));
        let parsed: serde_json::Value = serde_json::from_str(&json(&[row.clone()])).unwrap();
        assert_eq!(parsed[0]["state"], "ended");
        // A plain shell follows its process.
        row.agent = None;
        assert_eq!(state_word(&row), "running");
    }

    #[test]
    fn an_ended_row_is_state_ended() {
        let parsed: serde_json::Value = serde_json::from_str(&json(&[sample(4, "t", SessionState::Ended)])).unwrap();
        assert_eq!(parsed[0]["state"], "ended");
    }
}
