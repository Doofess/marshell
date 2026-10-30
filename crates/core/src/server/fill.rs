//! `POST /v1/sessions/{tab}/fill`: puts text on a session's prompt line, for the `marshell` CLI. Like `/input` it
//! never presses Enter (SCOPE.md "Never"). Unlike `/input` it refuses every control character, not just a line
//! break, and with `wait` it holds the text back until the agent is ready for it.

use super::AppState;
use super::sessions::cannot_resume;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::agents::HookInstall;
use marshell_protocol::api::{FillRequest, LiveState};
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

/// Whether a row waits for Claude Code's `SessionStart`: a Claude row with Marshell's hooks installed and live status
/// on. Without the hooks `SessionStart` never reaches Marshell, so quiet output is all there is to go on.
pub fn claude_hooked(agent: Option<&str>, status_note: Option<&str>, hooks: Option<HookInstall>) -> bool {
    agent == Some("claude") && status_note.is_none() && hooks == Some(HookInstall::Installed)
}

pub async fn fill(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<FillRequest>) -> Response {
    if st.registry().row(&tab).is_none() {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    }
    if let Err(line) = check_text(&req.text) {
        return (StatusCode::BAD_REQUEST, line).into_response();
    }
    // The 30 s start here, so a slow settings or version lookup counts against them.
    let deadline = Instant::now() + FILL_TIMEOUT;
    // Claude Code's settings are read once, off the async workers: with or without `wait`, a row with the hooks
    // installed is not typed into before its `SessionStart`.
    let claude = st
        .registry()
        .row(&tab)
        .is_some_and(|r| r.agent.as_deref() == Some("claude"));
    let hooks = if claude {
        let asker = st.clone();
        let lookup = tokio::task::spawn_blocking(move || super::agents::setup_of(&asker).ok().map(|s| s.hooks));
        match tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), lookup).await {
            Ok(done) => done.ok().flatten(),
            Err(_) => return cannot_resume("not_ready"),
        }
    } else {
        None
    };
    // The stream end last seen, and when it last changed.
    let mut seen: Option<(u64, Instant)> = None;
    loop {
        let Some(row) = st.registry().row(&tab) else {
            return (StatusCode::NOT_FOUND, "No such session.").into_response();
        };
        if !row.running() {
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
        // A pending answer would take the text as its own keys (a digit, `y`), so nothing is typed until it is
        // answered, and Marshell's held card is left alone. A Claude dialog dismissed with Escape sends no hook, so the
        // row keeps saying it waits: a key typed in the terminal since it appeared counts as its answer. Two more
        // answers can be pending with no dialog state. A hooked Claude row without its `SessionStart` may still show
        // the folder-trust dialog. An Antigravity row stays Working under its own tool prompt, and only the status
        // line says it shows; typing takes that flag, so a key typed since counts as its answer too.
        let hooked = claude_hooked(row.agent.as_deref(), row.status_note.as_deref(), hooks);
        let started = st.registry().session_started(&tab);
        let waiting = st.holds().has_tab(&tab)
            || row.live.as_ref().is_some_and(|l| {
                matches!(l.state, LiveState::NeedsPermission | LiveState::NeedsQuestion)
                    && !st.registry().typed_since(&tab, l.since_ms)
            })
            || (hooked && !started)
            || st.registry().agent_prompt_showing(&tab);
        if waiting && !req.wait {
            return cannot_resume("waiting");
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
        if !waiting && ready(hooked, started, end, now - changed) {
            return write(&st, &tab, &session, req.text);
        }
        if now >= deadline {
            return cannot_resume("not_ready");
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Types the text into the session. Unlike `/input` it never releases a held permission card: it is only called
/// with nothing pending.
fn write(st: &AppState, tab: &str, session: &crate::pty::Session, text: String) -> Response {
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
    fn only_installed_claude_hooks_wait_for_session_start() {
        assert!(claude_hooked(Some("claude"), None, Some(HookInstall::Installed)));
        // No hooks, or not all of them: SessionStart never comes, so quiet output is the signal.
        for hooks in [
            None,
            Some(HookInstall::None),
            Some(HookInstall::Partial),
            Some(HookInstall::Orphaned),
        ] {
            assert!(!claude_hooked(Some("claude"), None, hooks), "{hooks:?}");
        }
        // Live status off or an unknown version: the hooks may run, but nothing waits on them.
        assert!(!claude_hooked(
            Some("claude"),
            Some("off"),
            Some(HookInstall::Installed)
        ));
        assert!(!claude_hooked(Some("codex"), None, Some(HookInstall::Installed)));
        assert!(!claude_hooked(None, None, Some(HookInstall::Installed)));
    }

    #[test]
    fn claude_waits_for_session_start() {
        assert!(!ready(true, false, 10, Duration::from_secs(60)));
        assert!(ready(true, true, 10, Duration::from_millis(1500)));
    }
}
