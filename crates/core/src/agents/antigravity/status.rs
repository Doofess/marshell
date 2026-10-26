//! Folds Antigravity CLI's hook events and status line into a session's live status.
//!
//! agy's hook bodies carry no event name: the bridge adds `hook_event_name` from its `--event` flag. A body's
//! `lastUserInput` and a status line's `email` are personal: `strip_personal` takes them out at the door, and
//! nothing here reads or copies either.

use crate::agents::common::blank;
use marshell_protocol::api::{LiveState, LiveStatus};
use serde_json::Value;

/// The first non-empty line, trimmed, cut at `max` characters with an ellipsis.
fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.chars().count() > max {
        format!("{}…", line.chars().take(max).collect::<String>())
    } else {
        line.to_string()
    }
}

fn str_at<'a>(body: &'a Value, pointer: &str) -> Option<&'a str> {
    body.pointer(pointer)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

/// The file name of a path agy sends with either kind of separator.
fn file_name(path: &str) -> &str {
    let path = path.trim_end_matches(['/', '\\']);
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn tool_name(body: &Value) -> &str {
    str_at(body, "/toolCall/name").unwrap_or("a tool")
}

/// What a tool is doing, as line 2 says it.
fn tool_phrase(body: &Value) -> String {
    let tool = tool_name(body);
    let named = |verb: &str| {
        str_at(body, "/toolCall/args/Path")
            .map(file_name)
            .filter(|name| !name.is_empty())
            .map(|name| format!("{verb} `{}`", first_line(name, 60)))
    };
    let phrase = match tool {
        "run_command" => {
            str_at(body, "/toolCall/args/CommandLine").map(|command| format!("Running `{}`", first_line(command, 60)))
        }
        "write_file" | "edit_file" => named("Editing"),
        "read_file" => named("Reading"),
        _ => None,
    };
    phrase.unwrap_or_else(|| format!("Using `{}`", first_line(tool, 60)))
}

/// What a held request asks for.
fn held_phrase(body: &Value) -> String {
    let tool = tool_name(body);
    match (tool, str_at(body, "/toolCall/args/CommandLine")) {
        ("run_command", Some(command)) => format!("Wants to run `{}`", first_line(command, 60)),
        _ => format!("Wants to use `{}`", first_line(tool, 60)),
    }
}

/// The status after one hook event. None for an event that says nothing about status. `held` says the
/// request waits for a card. Any event can be the first, so `prev` may be None for all of them.
pub fn fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64, held: bool) -> Option<LiveStatus> {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    let (state, phrase) = match body.get("hook_event_name").and_then(Value::as_str)? {
        "PreInvocation" => (LiveState::Working, "Thinking".to_string()),
        "PreToolUse" if held => (LiveState::NeedsPermission, held_phrase(body)),
        "PreToolUse" => (LiveState::Working, tool_phrase(body)),
        "PostToolUse" => match next.state {
            LiveState::Working if !next.phrase.is_empty() => (LiveState::Working, next.phrase.clone()),
            _ => (LiveState::Working, "Thinking".to_string()),
        },
        "Stop" => {
            // agy sends no reply text: an earlier turn's recap does not stand for this one.
            next.recap = None;
            match str_at(body, "/error") {
                Some(error) => (LiveState::Error, first_line(error, 120)),
                None => (LiveState::Done, "Done".to_string()),
            }
        }
        _ => return None,
    };
    if prev.is_some() && state != next.state {
        next.since_ms = now_ms;
    }
    next.state = state;
    next.phrase = phrase;
    Some(next)
}

fn pct_at(body: &Value, pointer: &str) -> Option<u8> {
    let n = body.pointer(pointer)?.as_f64()?;
    n.is_finite().then(|| n.clamp(0.0, 100.0).round() as u8)
}

/// The status after a status line: the model and the context percentage. The state and phrase stay as the
/// hooks set them, and there is no plan meter (agy's quota is per model and weekly, not a 5-hour window).
/// A status line is a whole snapshot, so a field it leaves out is unknown, not its last value.
pub fn fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    next.model = str_at(body, "/model/display_name")
        .or_else(|| str_at(body, "/model/id"))
        .map(str::to_string);
    next.context_pct = pct_at(body, "/context_window/used_percentage");
    next.five_hour_pct = None;
    next.five_hour_resets_at = None;
    next.seven_day_pct = None;
    next
}

/// Whether agy shows its own tool prompt: `Some(true)` when `tool_confirmation_pending` is true, `Some(false)`
/// when the key is absent or false (agy drops the key when no prompt shows), None for a body that is not an
/// object. agy runs the status line on a change, so the flag clears on the next run after the prompt is answered.
pub fn confirmation_pending(body: &Value) -> Option<bool> {
    let object = body.as_object()?;
    Some(object.get("tool_confirmation_pending").and_then(Value::as_bool) == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    macro_rules! hook {
        ($name:literal, $event:literal) => {{
            let mut body = serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/antigravity/hooks/1.4.7/",
                $name,
                ".json"
            )))
            .unwrap();
            body["hook_event_name"] = json!($event);
            body
        }};
    }

    macro_rules! statusline {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/antigravity/statusline/1.4.7/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    fn fold(prev: Option<&LiveStatus>, body: &Value, at: u64) -> LiveStatus {
        fold_event(prev, body, at, false).expect("the event folds")
    }

    fn phrase_of(body: &Value) -> (LiveState, String) {
        let s = fold(None, body, 1);
        (s.state, s.phrase)
    }

    #[test]
    fn pre_invocation_is_thinking() {
        assert_eq!(
            phrase_of(&hook!("PreInvocation", "PreInvocation")),
            (LiveState::Working, "Thinking".to_string())
        );
    }

    #[test]
    fn run_command_is_running_its_command() {
        assert_eq!(
            phrase_of(&hook!("PreToolUse-run_command", "PreToolUse")),
            (LiveState::Working, "Running `echo hello-6d`".to_string())
        );
        let long = json!({"hook_event_name": "PreToolUse", "toolCall": {"name": "run_command",
            "args": {"CommandLine": format!("first line\nsecond\n{}", "x".repeat(100))}}});
        assert_eq!(phrase_of(&long).1, "Running `first line`");
        let wide = json!({"hook_event_name": "PreToolUse", "toolCall": {"name": "run_command",
            "args": {"CommandLine": "y".repeat(100)}}});
        assert_eq!(phrase_of(&wide).1, format!("Running `{}…`", "y".repeat(60)));
    }

    #[test]
    fn write_and_edit_are_editing_their_file() {
        assert_eq!(
            phrase_of(&hook!("PreToolUse-write_file", "PreToolUse")),
            (LiveState::Working, "Editing `notes.txt`".to_string())
        );
        assert_eq!(
            phrase_of(&hook!("PreToolUse-edit_file", "PreToolUse")),
            (LiveState::Working, "Editing `notes.txt`".to_string())
        );
    }

    #[test]
    fn read_file_is_reading_its_file() {
        assert_eq!(
            phrase_of(&hook!("PreToolUse-read_file", "PreToolUse")),
            (LiveState::Working, "Reading `notes.txt`".to_string())
        );
        // A read outside the workspace folds the same way: the file name only.
        let outside = phrase_of(&hook!("PreToolUse-read_file-outside", "PreToolUse"));
        assert_eq!(outside, (LiveState::Working, "Reading `outside.txt`".to_string()));
    }

    #[test]
    fn a_path_with_either_separator_gives_its_file_name() {
        for path in ["C:\\work\\src\\a.rs", "/work/src/a.rs", "src/a.rs", "a.rs"] {
            let body =
                json!({"hook_event_name": "PreToolUse", "toolCall": {"name": "edit_file", "args": {"Path": path}}});
            assert_eq!(phrase_of(&body).1, "Editing `a.rs`", "{path}");
        }
    }

    #[test]
    fn another_tool_is_using_it() {
        let body = json!({"hook_event_name": "PreToolUse", "toolCall": {"name": "read_url_content", "args": {}}});
        assert_eq!(
            phrase_of(&body),
            (LiveState::Working, "Using `read_url_content`".to_string())
        );
        // A known tool without the argument it is named by falls back to the tool.
        let bare = json!({"hook_event_name": "PreToolUse", "toolCall": {"name": "run_command"}});
        assert_eq!(phrase_of(&bare).1, "Using `run_command`");
        let nameless = json!({"hook_event_name": "PreToolUse"});
        assert_eq!(phrase_of(&nameless).1, "Using `a tool`");
    }

    #[test]
    fn a_held_request_needs_permission() {
        let held = |body: &Value| {
            let s = fold_event(None, body, 1, true).unwrap();
            (s.state, s.phrase)
        };
        assert_eq!(
            held(&hook!("PreToolUse-run_command", "PreToolUse")),
            (LiveState::NeedsPermission, "Wants to run `echo hello-6d`".to_string())
        );
        assert_eq!(
            held(&hook!("PreToolUse-write_file", "PreToolUse")),
            (LiveState::NeedsPermission, "Wants to use `write_file`".to_string())
        );
    }

    #[test]
    fn post_tool_after_a_held_request_is_thinking() {
        let asked = fold_event(None, &hook!("PreToolUse-run_command", "PreToolUse"), 10, true).unwrap();
        let after = fold(Some(&asked), &hook!("PostToolUse", "PostToolUse"), 20);
        assert_eq!((after.state, after.phrase.as_str()), (LiveState::Working, "Thinking"));
        assert_eq!(after.since_ms, 20);
    }

    #[test]
    fn post_tool_keeps_the_running_phrase() {
        let running = fold(None, &hook!("PreToolUse-run_command", "PreToolUse"), 10);
        let after = fold(Some(&running), &hook!("PostToolUse", "PostToolUse"), 20);
        assert_eq!(after.phrase, "Running `echo hello-6d`");
        assert_eq!(after.since_ms, 10);
        // With nothing before it, there is no phrase to keep.
        assert_eq!(phrase_of(&hook!("PostToolUse", "PostToolUse")).1, "Thinking");
    }

    #[test]
    fn post_invocation_changes_nothing() {
        assert_eq!(
            fold_event(None, &hook!("PostInvocation", "PostInvocation"), 1, false),
            None
        );
        let running = fold(None, &hook!("PreToolUse-run_command", "PreToolUse"), 10);
        assert_eq!(
            fold_event(Some(&running), &hook!("PostInvocation", "PostInvocation"), 20, false),
            None
        );
    }

    #[test]
    fn stop_is_done_with_no_recap() {
        let mut working = fold(None, &hook!("PreToolUse-run_command", "PreToolUse"), 10);
        working.recap = Some(marshell_protocol::api::Recap {
            text: "an earlier turn".into(),
            at_ms: 5,
        });
        let done = fold(Some(&working), &hook!("Stop", "Stop"), 30);
        assert_eq!((done.state, done.phrase.as_str()), (LiveState::Done, "Done"));
        assert_eq!(done.recap, None);
        assert_eq!(done.since_ms, 30);
    }

    #[test]
    fn stop_with_an_error_is_error() {
        let mut stop = hook!("Stop", "Stop");
        stop["error"] = json!("quota exhausted\nretry later");
        assert_eq!(phrase_of(&stop), (LiveState::Error, "quota exhausted".to_string()));
        stop["error"] = json!("z".repeat(200));
        assert_eq!(phrase_of(&stop).1, format!("{}…", "z".repeat(120)));
        // Absent or blank is not an error.
        stop.as_object_mut().unwrap().remove("error");
        assert_eq!(phrase_of(&stop), (LiveState::Done, "Done".to_string()));
        stop["error"] = json!("  ");
        assert_eq!(phrase_of(&stop).0, LiveState::Done);
    }

    #[test]
    fn a_first_event_with_no_prev_folds() {
        for (name, body) in [
            ("PreInvocation", hook!("PreInvocation", "PreInvocation")),
            ("PreToolUse", hook!("PreToolUse-read_file", "PreToolUse")),
            ("PostToolUse", hook!("PostToolUse", "PostToolUse")),
            ("Stop", hook!("Stop", "Stop")),
        ] {
            let s = fold_event(None, &body, 77, false).unwrap_or_else(|| panic!("{name} folds"));
            assert_eq!(s.since_ms, 77, "{name}");
        }
    }

    #[test]
    fn session_start_and_unknown_events_fold_to_nothing() {
        assert_eq!(fold_event(None, &hook!("SessionStart", "SessionStart"), 1, false), None);
        assert_eq!(
            fold_event(None, &json!({"hook_event_name": "Whatever"}), 1, false),
            None
        );
        // A body with no event name (the bridge always adds one) says nothing.
        assert_eq!(
            fold_event(None, &json!({"toolCall": {"name": "run_command"}}), 1, false),
            None
        );
    }

    #[test]
    fn no_fold_carries_the_last_user_input() {
        let marker = "Run the shell command";
        for (name, body) in [
            ("PreInvocation", hook!("PreInvocation", "PreInvocation")),
            ("PreToolUse", hook!("PreToolUse-run_command", "PreToolUse")),
            ("PostToolUse", hook!("PostToolUse", "PostToolUse")),
            ("Stop", hook!("Stop", "Stop")),
        ] {
            for held in [false, true] {
                let Some(s) = fold_event(None, &body, 1, held) else {
                    continue;
                };
                let dump = format!("{s:?}");
                assert!(!dump.contains(marker), "{name} held={held}: {dump}");
            }
        }
        // The fixtures that carry the input do fold, so the check above is not vacuous.
        assert!(hook!("Stop", "Stop").to_string().contains(marker));
        let s = fold_statusline(None, &statusline!("working"), 1);
        assert!(!format!("{s:?}").contains("user@example.com"));
    }

    #[test]
    fn the_status_line_sets_model_and_context() {
        let s = fold_statusline(None, &statusline!("working"), 5);
        assert_eq!(s.model.as_deref(), Some("Gemini 3.8 Flash (High)"));
        assert_eq!(s.context_pct, Some(1));
        // The model id answers when there is no display name.
        let by_id = json!({"model": {"id": "gemini-x"}});
        assert_eq!(fold_statusline(None, &by_id, 5).model.as_deref(), Some("gemini-x"));
        // A snapshot that leaves them out says they are unknown.
        let bare = fold_statusline(Some(&s), &json!({}), 6);
        assert_eq!((bare.model, bare.context_pct), (None, None));
    }

    #[test]
    fn the_status_line_leaves_the_state_alone() {
        let running = fold(None, &hook!("PreToolUse-run_command", "PreToolUse"), 10);
        // agent_state says "initializing" or "idle" here: the hooks own the state.
        for body in [statusline!("idle"), statusline!("pending"), statusline!("initializing")] {
            let s = fold_statusline(Some(&running), &body, 99);
            assert_eq!(
                (s.state, s.phrase.as_str(), s.since_ms),
                (LiveState::Working, "Running `echo hello-6d`", 10)
            );
        }
        let first = fold_statusline(None, &statusline!("idle"), 42);
        assert_eq!((first.state, first.since_ms), (LiveState::Idle, 42));
    }

    #[test]
    fn the_status_line_sets_no_plan_meter() {
        let s = fold_statusline(None, &statusline!("working"), 1);
        assert_eq!(
            (s.five_hour_pct, s.five_hour_resets_at, s.seven_day_pct),
            (None, None, None)
        );
        let mut earlier = s.clone();
        earlier.five_hour_pct = Some(50);
        earlier.seven_day_pct = Some(10);
        let s = fold_statusline(Some(&earlier), &statusline!("working"), 2);
        assert_eq!((s.five_hour_pct, s.seven_day_pct), (None, None));
    }

    #[test]
    fn pending_is_read_from_pending_json() {
        assert_eq!(confirmation_pending(&statusline!("pending")), Some(true));
    }

    #[test]
    fn a_missing_pending_key_is_false() {
        assert_eq!(confirmation_pending(&statusline!("idle")), Some(false));
        assert_eq!(confirmation_pending(&statusline!("working")), Some(false));
        assert_eq!(
            confirmation_pending(&json!({"tool_confirmation_pending": false})),
            Some(false)
        );
        assert_eq!(
            confirmation_pending(&json!({"tool_confirmation_pending": "yes"})),
            Some(false)
        );
    }

    #[test]
    fn a_body_that_is_not_an_object_has_no_pending_answer() {
        for body in [json!(null), json!("x"), json!([true]), json!(3)] {
            assert_eq!(confirmation_pending(&body), None, "{body}");
        }
    }
}
