//! Folds Claude Code's hook events and status line into a session's live status.

use marshell_protocol::api::{LiveState, LiveStatus, Recap};
use serde_json::Value;

/// A ring's colour band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingLevel {
    Green,
    Yellow,
    Red,
}

/// Green below 70%, yellow from 70%, red from 90%.
pub fn ring_level(pct: f32) -> RingLevel {
    if pct >= 90.0 {
        RingLevel::Red
    } else if pct >= 70.0 {
        RingLevel::Yellow
    } else {
        RingLevel::Green
    }
}

/// The status a session has before anything is known.
fn blank(now_ms: u64) -> LiveStatus {
    LiveStatus {
        state: LiveState::Idle,
        phrase: String::new(),
        since_ms: now_ms,
        recap: None,
        model: None,
        effort: None,
        context_pct: None,
        five_hour_pct: None,
        seven_day_pct: None,
        subagents: 0,
    }
}

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

/// What a tool is doing, as line 2 says it.
fn tool_phrase(body: &Value) -> String {
    let tool = str_at(body, "/tool_name").unwrap_or("a tool");
    let (verb, field) = match tool {
        "Bash" | "PowerShell" => ("Running", "/tool_input/command"),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ("Editing", "/tool_input/file_path"),
        "Read" => ("Reading", "/tool_input/file_path"),
        _ => ("", ""),
    };
    match str_at(body, field).filter(|_| !verb.is_empty()) {
        Some(target) => format!("{verb} `{}`", first_line(target, 60)),
        None => format!("Using `{}`", first_line(tool, 60)),
    }
}

/// What a permission request asks for.
fn permission_phrase(body: &Value) -> String {
    match str_at(body, "/tool_input/command") {
        Some(cmd) => format!("Wants to run `{}`", first_line(cmd, 60)),
        None => format!(
            "Wants to use `{}`",
            first_line(str_at(body, "/tool_name").unwrap_or("a tool"), 60)
        ),
    }
}

/// The status after one hook event. None for an event that says nothing about status.
pub fn fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus> {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    let (state, phrase) = match body.get("hook_event_name").and_then(Value::as_str)? {
        "SessionStart" => (LiveState::Idle, String::new()),
        "UserPromptSubmit" => (LiveState::Working, "Thinking".to_string()),
        "PreToolUse" => (LiveState::Working, tool_phrase(body)),
        "PostToolUse" | "PostToolBatch" => (LiveState::Working, next.phrase.clone()),
        "PermissionRequest" => (LiveState::NeedsPermission, permission_phrase(body)),
        "Notification" => match body.get("notification_type").and_then(Value::as_str) {
            Some("idle_prompt") => (LiveState::Idle, String::new()),
            Some("permission_prompt") => (LiveState::NeedsPermission, next.phrase.clone()),
            _ => (next.state, next.phrase.clone()),
        },
        "Stop" => {
            if let Some(text) = str_at(body, "/last_assistant_message") {
                next.recap = Some(Recap {
                    text: first_line(text, 200),
                    at_ms: now_ms,
                });
            }
            (LiveState::Done, "Done".to_string())
        }
        "SessionEnd" => (LiveState::Ended, "Ended".to_string()),
        _ => return None,
    };
    if prev.is_some() && state != next.state {
        next.since_ms = now_ms;
    }
    next.state = state;
    next.phrase = phrase;
    Some(next)
}

/// A whole percentage from a JSON number, clamped to 0..=100. Anything else is unknown.
fn pct_at(body: &Value, pointer: &str) -> Option<u8> {
    let n = body.pointer(pointer)?.as_f64()?;
    n.is_finite().then(|| n.clamp(0.0, 100.0).round() as u8)
}

/// The status after a status line: model, effort and the percentages. State, phrase and recap stay.
/// A status line is a whole snapshot, so a field it leaves out is unknown, not its last value.
pub fn fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    next.model = str_at(body, "/model/display_name")
        .or_else(|| str_at(body, "/model/id"))
        .map(str::to_string);
    next.effort = str_at(body, "/effort/level").map(str::to_string);
    next.context_pct = pct_at(body, "/context_window/used_percentage");
    next.five_hour_pct = pct_at(body, "/rate_limits/five_hour/used_percentage");
    next.seven_day_pct = pct_at(body, "/rate_limits/seven_day/used_percentage");
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    macro_rules! fixture {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/claude/2.1.302/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    fn fold(prev: Option<&LiveStatus>, body: &Value) -> LiveStatus {
        fold_event(prev, body, 1_000).expect("a status event")
    }

    #[test]
    fn session_start_is_idle() {
        let s = fold(None, &fixture!("SessionStart"));
        assert_eq!(s.state, LiveState::Idle);
        assert_eq!(s.phrase, "");
    }

    #[test]
    fn a_prompt_is_working() {
        let s = fold(None, &fixture!("UserPromptSubmit"));
        assert_eq!(s.state, LiveState::Working);
        assert_eq!(s.phrase, "Thinking");
    }

    #[test]
    fn a_bash_tool_names_its_command() {
        let s = fold(None, &fixture!("PreToolUse"));
        assert_eq!(s.state, LiveState::Working);
        assert_eq!(s.phrase, "Running `echo marshell-spike-s10`");
    }

    #[test]
    fn file_tools_name_their_file() {
        let edit = json!({"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"src/auth.ts"}});
        assert_eq!(fold(None, &edit).phrase, "Editing `src/auth.ts`");
        let read = json!({"hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"a.rs"}});
        assert_eq!(fold(None, &read).phrase, "Reading `a.rs`");
        let other = json!({"hook_event_name":"PreToolUse","tool_name":"WebFetch","tool_input":{}});
        assert_eq!(fold(None, &other).phrase, "Using `WebFetch`");
    }

    #[test]
    fn a_tool_without_its_input_falls_back_to_its_name() {
        let s = fold(None, &json!({"hook_event_name":"PreToolUse","tool_name":"Bash"}));
        assert_eq!(s.phrase, "Using `Bash`");
    }

    #[test]
    fn a_long_command_keeps_its_first_line_and_60_chars() {
        let cmd = format!("{}\nsecond", "x".repeat(80));
        let s = fold(
            None,
            &json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":cmd}}),
        );
        assert_eq!(s.phrase, format!("Running `{}…`", "x".repeat(60)));
    }

    #[test]
    fn post_tool_use_keeps_the_phrase_and_since() {
        let pre = fold_event(None, &fixture!("PreToolUse"), 1_000).unwrap();
        let post = fold_event(Some(&pre), &fixture!("PostToolUse"), 5_000).unwrap();
        assert_eq!(post.state, LiveState::Working);
        assert_eq!(post.phrase, pre.phrase);
        assert_eq!(post.since_ms, 1_000);
        let batch = fold_event(Some(&post), &fixture!("PostToolBatch"), 6_000).unwrap();
        assert_eq!(batch.since_ms, 1_000);
    }

    #[test]
    fn since_changes_with_the_state() {
        let idle = fold_event(None, &fixture!("SessionStart"), 1_000).unwrap();
        let working = fold_event(Some(&idle), &fixture!("UserPromptSubmit"), 2_000).unwrap();
        assert_eq!(working.since_ms, 2_000);
    }

    #[test]
    fn a_permission_request_needs_you() {
        let s = fold(None, &fixture!("PermissionRequest"));
        assert_eq!(s.state, LiveState::NeedsPermission);
        assert_eq!(s.phrase, "Wants to run `python -c \"print(4242)\"`");
        let other = json!({"hook_event_name":"PermissionRequest","tool_name":"Write","tool_input":{}});
        assert_eq!(fold(None, &other).phrase, "Wants to use `Write`");
    }

    #[test]
    fn an_idle_notification_is_idle() {
        let working = fold(None, &fixture!("UserPromptSubmit"));
        let s = fold(Some(&working), &fixture!("Notification"));
        assert_eq!(s.state, LiveState::Idle);
    }

    #[test]
    fn a_permission_notification_needs_you_and_keeps_the_phrase() {
        let pre = fold(None, &fixture!("PreToolUse"));
        let n = json!({"hook_event_name":"Notification","notification_type":"permission_prompt","message":"x"});
        let s = fold(Some(&pre), &n);
        assert_eq!(s.state, LiveState::NeedsPermission);
        assert_eq!(s.phrase, pre.phrase);
    }

    #[test]
    fn another_notification_keeps_the_status() {
        let pre = fold(None, &fixture!("PreToolUse"));
        let n = json!({"hook_event_name":"Notification","notification_type":"auth_success"});
        assert_eq!(fold(Some(&pre), &n), pre);
    }

    #[test]
    fn stop_is_done_with_a_recap() {
        let s = fold_event(None, &fixture!("Stop"), 7_000).unwrap();
        assert_eq!(s.state, LiveState::Done);
        assert_eq!(s.phrase, "Done");
        let recap = s.recap.expect("a recap");
        assert!(recap.text.starts_with("I ran `echo marshell-spike-s10`"));
        assert_eq!(recap.at_ms, 7_000);
    }

    #[test]
    fn a_stop_without_a_message_keeps_the_last_recap() {
        let first = fold(None, &fixture!("Stop"));
        let empty = json!({"hook_event_name":"Stop","last_assistant_message":""});
        let s = fold_event(Some(&first), &empty, 9_000).unwrap();
        assert_eq!(s.recap, first.recap);
        let none = fold(None, &empty);
        assert_eq!(none.recap, None);
    }

    #[test]
    fn a_long_recap_keeps_its_first_line_and_200_chars() {
        let msg = format!("\n{}\nmore", "y".repeat(250));
        let s = fold(None, &json!({"hook_event_name":"Stop","last_assistant_message":msg}));
        assert_eq!(s.recap.unwrap().text, format!("{}…", "y".repeat(200)));
    }

    #[test]
    fn session_end_is_ended() {
        let working = fold(None, &fixture!("UserPromptSubmit"));
        let s = fold(Some(&working), &fixture!("SessionEnd"));
        assert_eq!(s.state, LiveState::Ended);
        assert_eq!(s.phrase, "Ended");
    }

    #[test]
    fn events_without_status_say_nothing() {
        assert_eq!(fold_event(None, &fixture!("InstructionsLoaded"), 1), None);
        assert_eq!(fold_event(None, &fixture!("MessageDisplay"), 1), None);
        assert_eq!(fold_event(None, &json!({"hook_event_name":"Nope"}), 1), None);
        assert_eq!(fold_event(None, &json!({"no":"name"}), 1), None);
    }

    #[test]
    fn an_event_keeps_the_status_lines_numbers() {
        let line = fold_statusline(None, &fixture!("statusline"), 1);
        let s = fold(Some(&line), &fixture!("UserPromptSubmit"));
        assert_eq!(s.context_pct, Some(6));
        assert_eq!(s.model.as_deref(), Some("Opus 5.5"));
    }

    #[test]
    fn the_status_line_fills_model_effort_and_rings() {
        let s = fold_statusline(None, &fixture!("statusline"), 1);
        assert_eq!(s.state, LiveState::Idle);
        assert_eq!(s.phrase, "");
        assert_eq!(s.model.as_deref(), Some("Opus 5.5"));
        assert_eq!(s.effort.as_deref(), Some("high"));
        assert_eq!(s.context_pct, Some(6));
        assert_eq!(s.five_hour_pct, Some(7));
        assert_eq!(s.seven_day_pct, Some(32));
    }

    #[test]
    fn the_status_line_keeps_state_phrase_and_recap() {
        let done = fold(None, &fixture!("Stop"));
        let s = fold_statusline(Some(&done), &fixture!("statusline"), 99);
        assert_eq!(s.state, LiveState::Done);
        assert_eq!(s.phrase, done.phrase);
        assert_eq!(s.recap, done.recap);
        assert_eq!(s.since_ms, done.since_ms);
    }

    #[test]
    fn missing_or_odd_percentages_are_unknown_or_clamped() {
        let s = fold_statusline(None, &json!({"context_window":{"used_percentage":140}}), 1);
        assert_eq!(s.context_pct, Some(100));
        assert_eq!(s.five_hour_pct, None);
        let s = fold_statusline(
            None,
            &json!({"context_window":{"used_percentage":null},"rate_limits":{"five_hour":{"used_percentage":-3}}}),
            1,
        );
        assert_eq!(s.context_pct, None);
        assert_eq!(s.five_hour_pct, Some(0));
        let s = fold_statusline(None, &json!({"context_window":{"used_percentage":"6"}}), 1);
        assert_eq!(s.context_pct, None);
        let s = fold_statusline(None, &json!({"context_window":{"used_percentage":69.6}}), 1);
        assert_eq!(s.context_pct, Some(70));
    }

    #[test]
    fn each_status_line_replaces_the_last_ones_numbers() {
        // A status line is a whole snapshot: a field it leaves out is unknown now, not its old value.
        let full = fold_statusline(None, &fixture!("statusline"), 1);
        let s = fold_statusline(Some(&full), &json!({}), 2);
        assert_eq!(s.five_hour_pct, None);
        assert_eq!(s.model, None);
    }

    #[test]
    fn ring_levels() {
        assert_eq!(ring_level(69.9), RingLevel::Green);
        assert_eq!(ring_level(70.0), RingLevel::Yellow);
        assert_eq!(ring_level(89.9), RingLevel::Yellow);
        assert_eq!(ring_level(90.0), RingLevel::Red);
    }
}
