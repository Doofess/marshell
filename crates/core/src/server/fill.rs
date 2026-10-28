//! `POST /v1/sessions/{tab}/fill`: puts text on a session's prompt line, for the `marshell` CLI. Like `/input` it
//! never presses Enter (SCOPE.md "Never"). Unlike `/input` it refuses every control character, not just a line
//! break, and with `wait` it holds the text back until the agent is ready for it.

use super::AppState;
use super::sessions::cannot_resume;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{FillRequest, SessionState};
use std::time::{Duration, Instant};

/// The refusal for text that holds a control character.
pub const TEXT_REFUSED: &str =
    "The text can't hold line breaks or other control characters: Marshell never presses Enter.";
/// The refusal for text over `MAX_CHARS` characters.
pub const TEXT_TOO_LONG: &str = "The text is over 4000 characters.";
const MAX_CHARS: usize = 4000;

/// How long `wait` holds the text back before giving up with `not_ready`.
pub const FILL_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the session's output must have been quiet before the agent counts as ready.
pub const FILL_QUIET: Duration = Duration::from_millis(1500);
/// How often readiness is looked at.
const POLL: Duration = Duration::from_millis(100);

/// Refuses text that could press a key other than a printable one: every C0 control (Enter, Tab, Ctrl+C and
/// the Escape that starts a CSI-u Enter or the end of a bracketed paste), DEL and every C1 control (U+009B is
/// a one-character CSI). Also text over 4000 characters.
pub fn check_text(text: &str) -> Result<(), &'static str> {
    if text.chars().any(|c| c < ' ' || ('\u{7f}'..='\u{9f}').contains(&c)) {
        return Err(TEXT_REFUSED);
    }
    if text.chars().count() > MAX_CHARS {
        return Err(TEXT_TOO_LONG);
    }
    Ok(())
}

/// Whether the agent can take the text. `end` is the pty stream end, `quiet_for` how long it has not grown.
/// Output has started and then gone quiet for `FILL_QUIET`. A Claude Code row with hooks also needs its
/// `SessionStart`, which runs only once the folder-trust dialog is answered: that dialog is quiet too.
pub fn ready(claude_hooked: bool, started: bool, end: u64, quiet_for: Duration) -> bool {
    end > 0 && quiet_for >= FILL_QUIET && (!claude_hooked || started)
}

pub async fn fill(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<FillRequest>) -> Response {
    if st.registry().row(&tab).is_none() {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    }
    if let Err(line) = check_text(&req.text) {
        return (StatusCode::BAD_REQUEST, line).into_response();
    }
    let deadline = Instant::now() + FILL_TIMEOUT;
    // The stream end last seen, and when it last changed.
    let mut seen: Option<(u64, Instant)> = None;
    loop {
        let Some(row) = st.registry().row(&tab) else {
            return (StatusCode::NOT_FOUND, "No such session.").into_response();
        };
        if row.state != SessionState::Running {
            return cannot_resume("ended");
        }
        let Some(session) = st.session(&tab) else {
            return (StatusCode::NOT_FOUND, "No such session.").into_response();
        };
        if session.has_exited() {
            return cannot_resume("ended");
        }
        if session.is_frozen() {
            return cannot_resume("It is paused.");
        }
        if !req.wait {
            return write(&st, &tab, &session, req.text);
        }
        let now = Instant::now();
        let (end, _) = session.counters();
        let changed = match seen {
            Some((last, at)) if last == end => at,
            _ => now,
        };
        seen = Some((end, changed));
        let hooked = row.agent.as_deref() == Some("claude") && row.status_note.is_none();
        if ready(hooked, st.registry().session_started(&tab), end, now - changed) {
            return write(&st, &tab, &session, req.text);
        }
        if now >= deadline {
            return cannot_resume("not_ready");
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Types the text into the session, as `/input` does: answering in the terminal.
fn write(st: &AppState, tab: &str, session: &crate::pty::Session, text: String) -> Response {
    st.holds().release_tab(tab);
    st.registry().note_input(tab);
    session.write_input(text.into_bytes());
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn check_text_refuses_controls() {
        for text in [
            "a\rb",
            "a\nb",
            "\x1b[13u",
            "\x1b[201~",
            "\x03",
            "\x04",
            "a\tb",
            "\u{9b}",
            "\x7f",
        ] {
            assert_eq!(check_text(text), Err(TEXT_REFUSED), "{text:?}");
        }
        assert_eq!(check_text(&"x".repeat(4001)), Err(TEXT_TOO_LONG));
        assert_eq!(check_text(&"x".repeat(4000)), Ok(()));
        assert_eq!(check_text("fix the tests — please"), Ok(()));
    }

    #[test]
    fn ready_waits_for_output_then_quiet() {
        assert!(!ready(false, false, 0, Duration::from_secs(10)));
        assert!(!ready(false, false, 10, Duration::from_millis(1499)));
        assert!(ready(false, false, 10, Duration::from_millis(1500)));
    }

    #[test]
    fn claude_waits_for_session_start() {
        assert!(!ready(true, false, 10, Duration::from_secs(60)));
        assert!(ready(true, true, 10, Duration::from_millis(1500)));
    }
}
