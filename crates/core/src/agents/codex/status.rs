//! Folds Codex CLI's hook events into a session's live status.

use crate::agents::common::blank;
use marshell_protocol::api::{LiveState, LiveStatus, Recap, SessionMode};
use serde_json::Value;

/// The permission mode a hook body reports (`permission_mode`, one of the five Codex documents).
/// None for a missing field (`SessionEnd` has none), a non-string or a value this build does not
/// know, so the row keeps the mode it has. A subagent's own events (they carry `agent_id`) report
/// the subagent's mode, not the session's: None too.
pub fn mode_of(body: &Value) -> Option<SessionMode> {
    if body.get("agent_id").is_some() {
        return None;
    }
    Some(match body.get("permission_mode")?.as_str()? {
        "default" => SessionMode::Manual,
        "acceptEdits" => SessionMode::AutoEdit,
        "plan" => SessionMode::Plan,
        "dontAsk" => SessionMode::DontAsk,
        "bypassPermissions" => SessionMode::Bypass,
        _ => return None,
    })
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

/// The path on a patch's first `*** Add File:`, `*** Update File:` or `*** Delete File:` line.
pub(crate) fn first_patch_file(patch: &str) -> Option<&str> {
    patch.lines().find_map(|line| {
        let path = ["*** Add File:", "*** Update File:", "*** Delete File:"]
            .iter()
            .find_map(|prefix| line.trim().strip_prefix(prefix))?
            .trim();
        (!path.is_empty()).then_some(path)
    })
}

/// Every path a patch writes, in order: its files and the targets they move to.
pub(crate) fn patch_files(patch: &str) -> impl Iterator<Item = &str> {
    patch.lines().filter_map(|line| {
        let path = ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"]
            .iter()
            .find_map(|prefix| line.trim().strip_prefix(prefix))?
            .trim();
        (!path.is_empty()).then_some(path)
    })
}

/// The file `apply_patch` touches first, cut to fit a row.
fn patched_file(body: &Value) -> Option<String> {
    first_patch_file(str_at(body, "/tool_input/command")?).map(|path| first_line(path, 60))
}

fn tool_name(body: &Value) -> &str {
    str_at(body, "/tool_name").unwrap_or("a tool")
}

/// What a tool is doing, as line 2 says it.
fn tool_phrase(body: &Value) -> String {
    match tool_name(body) {
        "Bash" => match str_at(body, "/tool_input/command") {
            Some(command) => format!("Running `{}`", first_line(command, 60)),
            None => "Using `Bash`".to_string(),
        },
        "apply_patch" => match patched_file(body) {
            Some(file) => format!("Editing `{file}`"),
            None => "Editing files".to_string(),
        },
        tool => format!("Using `{}`", first_line(tool, 60)),
    }
}

/// What a permission request asks for.
fn permission_phrase(body: &Value) -> String {
    let tool = tool_name(body);
    match (tool, str_at(body, "/tool_input/command")) {
        ("Bash", Some(command)) => format!("Wants to run `{}`", first_line(command, 60)),
        // The patch is the command of apply_patch: the row names its file, not the patch.
        ("apply_patch", _) => match patched_file(body) {
            Some(file) => format!("Wants to edit `{file}`"),
            None => "Wants to edit files".to_string(),
        },
        _ => format!("Wants to use `{}`", first_line(tool, 60)),
    }
}

/// The status after one hook event. None for an event that says nothing about status. Any event can
/// be the first (`SessionStart` comes with the first prompt), so `prev` may be None for all of them.
pub fn fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus> {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    let (state, phrase) = match body.get("hook_event_name").and_then(Value::as_str)? {
        "SessionStart" => (LiveState::Idle, String::new()),
        "UserPromptSubmit" => (LiveState::Working, "Thinking".to_string()),
        // A retried command sends this twice: the second changes nothing.
        "PreToolUse" => (LiveState::Working, tool_phrase(body)),
        "PermissionRequest" => (LiveState::NeedsPermission, permission_phrase(body)),
        // A denied request has no PostToolUse: its Stop ends the turn.
        "PostToolUse" => match next.state {
            LiveState::Working if !next.phrase.is_empty() => (LiveState::Working, next.phrase.clone()),
            _ => (LiveState::Working, "Thinking".to_string()),
        },
        "Stop" => {
            // `last_assistant_message` is null when the turn ended without a reply: an earlier
            // turn's recap does not stand for this one.
            next.recap = str_at(body, "/last_assistant_message").map(|text| Recap {
                text: first_line(text, 200),
                at_ms: now_ms,
            });
            (LiveState::Done, "Done".to_string())
        }
        // Codex sends no Stop after an interrupt.
        "Interrupt" => (LiveState::Idle, String::new()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    macro_rules! fixture {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/codex/hooks/0.163.0/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    fn fold(prev: Option<&LiveStatus>, body: &Value, at: u64) -> LiveStatus {
        fold_event(prev, body, at).expect("the event folds")
    }

    fn phrase_of(body: &Value) -> (LiveState, String) {
        let s = fold(None, body, 1);
        (s.state, s.phrase)
    }

    #[test]
    fn session_start_is_idle() {
        assert_eq!(phrase_of(&fixture!("SessionStart")), (LiveState::Idle, String::new()));
    }

    #[test]
    fn prompt_is_thinking() {
        assert_eq!(
            phrase_of(&fixture!("UserPromptSubmit")),
            (LiveState::Working, "Thinking".to_string())
        );
    }

    #[test]
    fn shell_tool_is_running_its_command() {
        assert_eq!(
            phrase_of(&fixture!("PreToolUse-shell")),
            (LiveState::Working, "Running `Set-Content out.txt hi`".to_string())
        );
        let long = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": format!("first line\nsecond\n{}", "x".repeat(100))}});
        assert_eq!(phrase_of(&long).1, "Running `first line`");
        let wide = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": "y".repeat(100)}});
        assert_eq!(phrase_of(&wide).1, format!("Running `{}…`", "y".repeat(60)));
    }

    #[test]
    fn apply_patch_is_editing_its_file() {
        assert_eq!(
            phrase_of(&fixture!("PreToolUse-apply_patch")),
            (LiveState::Working, "Editing `notes.md`".to_string())
        );
        for (line, file) in [
            ("*** Update File: src/a.rs", "src/a.rs"),
            ("*** Delete File: old.txt", "old.txt"),
        ] {
            let body = json!({"hook_event_name": "PreToolUse", "tool_name": "apply_patch",
                "tool_input": {"command": format!("*** Begin Patch\n{line}\n-x\n*** End Patch")}});
            assert_eq!(phrase_of(&body).1, format!("Editing `{file}`"));
        }
        let two = json!({"hook_event_name": "PreToolUse", "tool_name": "apply_patch",
            "tool_input": {"command": "*** Begin Patch\n*** Add File: one\n+a\n*** Update File: two\n*** End Patch"}});
        assert_eq!(phrase_of(&two).1, "Editing `one`");
    }

    #[test]
    fn apply_patch_without_a_file_line_is_editing_files() {
        for command in [
            json!("*** Begin Patch\n*** End Patch"),
            json!("*** Add File:   \n"),
            Value::Null,
        ] {
            let body = json!({"hook_event_name": "PreToolUse", "tool_name": "apply_patch",
                "tool_input": {"command": command}});
            assert_eq!(phrase_of(&body), (LiveState::Working, "Editing files".to_string()));
        }
    }

    #[test]
    fn another_tool_is_using_it() {
        let body = json!({"hook_event_name": "PreToolUse", "tool_name": "mcp__docs__search", "tool_input": {}});
        assert_eq!(phrase_of(&body).1, "Using `mcp__docs__search`");
    }

    #[test]
    fn permission_request_needs_permission() {
        assert_eq!(
            phrase_of(&fixture!("PermissionRequest-shell")),
            (
                LiveState::NeedsPermission,
                "Wants to run `Set-Content out.txt hi`".to_string()
            )
        );
        // The patch is the command of apply_patch: the row names the file, not the patch.
        assert_eq!(
            phrase_of(&fixture!("PermissionRequest-apply_patch")),
            (LiveState::NeedsPermission, "Wants to edit `notes.md`".to_string())
        );
        let other = json!({"hook_event_name": "PermissionRequest", "tool_name": "mcp__docs__search"});
        assert_eq!(phrase_of(&other).1, "Wants to use `mcp__docs__search`");
    }

    #[test]
    fn post_tool_after_a_permission_is_thinking() {
        let asked = fold(None, &fixture!("PermissionRequest-shell"), 10);
        let after = fold(Some(&asked), &fixture!("PostToolUse"), 20);
        assert_eq!((after.state, after.phrase.as_str()), (LiveState::Working, "Thinking"));
        assert_eq!(after.since_ms, 20);
    }

    #[test]
    fn post_tool_keeps_the_running_phrase() {
        let running = fold(None, &fixture!("PreToolUse-shell"), 10);
        let after = fold(Some(&running), &fixture!("PostToolUse"), 20);
        assert_eq!(after.phrase, "Running `Set-Content out.txt hi`");
        assert_eq!(after.since_ms, 10);
        // With nothing before it, there is no phrase to keep.
        assert_eq!(
            phrase_of(&fixture!("PostToolUse")),
            (LiveState::Working, "Thinking".to_string())
        );
    }

    #[test]
    fn stop_is_done_with_its_recap() {
        let s = fold(None, &fixture!("Stop-recap"), 5);
        assert_eq!((s.state, s.phrase.as_str()), (LiveState::Done, "Done"));
        let recap = s.recap.unwrap();
        assert_eq!(
            recap.text,
            "Approval was denied, so `Set-Content denied.txt no` did not run."
        );
        assert_eq!(recap.at_ms, 5);
        let long = json!({"hook_event_name": "Stop",
            "last_assistant_message": format!("Line one\nline two {}", "z".repeat(300))});
        assert_eq!(fold(None, &long, 1).recap.unwrap().text, "Line one");
        let wide = json!({"hook_event_name": "Stop", "last_assistant_message": "w".repeat(300)});
        assert_eq!(
            fold(None, &wide, 1).recap.unwrap().text,
            format!("{}…", "w".repeat(200))
        );
    }

    #[test]
    fn stop_without_a_reply_has_no_recap() {
        let s = fold(None, &fixture!("Stop"), 5);
        assert_eq!((s.state, s.recap), (LiveState::Done, None));
        // An earlier turn's recap does not stand for this turn.
        let earlier = fold(None, &fixture!("Stop-recap"), 1);
        let s = fold(Some(&earlier), &fixture!("Stop"), 5);
        assert_eq!(s.recap, None);
    }

    #[test]
    fn interrupt_is_idle() {
        let running = fold(None, &fixture!("PreToolUse-shell"), 10);
        let s = fold(Some(&running), &fixture!("Interrupt"), 20);
        assert_eq!((s.state, s.phrase.as_str(), s.since_ms), (LiveState::Idle, "", 20));
    }

    #[test]
    fn session_end_is_ended() {
        let running = fold(None, &fixture!("UserPromptSubmit"), 10);
        let s = fold(Some(&running), &fixture!("SessionEnd"), 20);
        assert_eq!((s.state, s.phrase.as_str()), (LiveState::Ended, "Ended"));
    }

    #[test]
    fn an_unknown_event_is_none() {
        for body in [
            json!({"hook_event_name": "PreCompact"}),
            json!({"hook_event_name": "SubagentStart"}),
            json!({"hook_event_name": 7}),
            json!({}),
        ] {
            assert_eq!(fold_event(None, &body, 1), None, "{body}");
        }
    }

    #[test]
    fn session_end_with_no_earlier_event_is_ended() {
        let s = fold(None, &fixture!("SessionEnd"), 7);
        assert_eq!((s.state, s.phrase.as_str(), s.since_ms), (LiveState::Ended, "Ended", 7));
    }

    #[test]
    fn a_denied_request_then_stop_is_done() {
        let asked = fold(None, &fixture!("PermissionRequest-shell"), 10);
        assert_eq!(asked.state, LiveState::NeedsPermission);
        let stopped = fold(Some(&asked), &fixture!("Stop"), 20);
        assert_eq!((stopped.state, stopped.phrase.as_str()), (LiveState::Done, "Done"));
        assert_eq!(stopped.since_ms, 20);
    }

    #[test]
    fn a_retried_tool_keeps_running() {
        let first = fold(None, &fixture!("PreToolUse-shell"), 10);
        let retry = fold(Some(&first), &fixture!("PreToolUse-shell"), 20);
        assert_eq!(retry, first);
        let asked = fold(Some(&retry), &fixture!("PermissionRequest-shell"), 30);
        let again = fold(Some(&asked), &fixture!("PreToolUse-shell"), 40);
        assert_eq!(again.state, LiveState::Working);
        let done = fold(Some(&again), &fixture!("PostToolUse"), 50);
        assert_eq!((done.state, done.since_ms), (LiveState::Working, 40));
    }

    #[test]
    fn since_changes_only_when_the_state_does() {
        let a = fold(None, &fixture!("UserPromptSubmit"), 10);
        let b = fold(Some(&a), &fixture!("PreToolUse-shell"), 20);
        assert_eq!(b.since_ms, 10);
        let c = fold(Some(&b), &fixture!("Stop"), 30);
        assert_eq!(c.since_ms, 30);
    }

    #[test]
    fn mode_of_reads_the_five_documented_values() {
        for (value, mode) in [
            ("default", SessionMode::Manual),
            ("acceptEdits", SessionMode::AutoEdit),
            ("plan", SessionMode::Plan),
            ("dontAsk", SessionMode::DontAsk),
            ("bypassPermissions", SessionMode::Bypass),
        ] {
            assert_eq!(mode_of(&json!({"permission_mode": value})), Some(mode), "{value}");
        }
        assert_eq!(mode_of(&fixture!("UserPromptSubmit")), Some(SessionMode::Manual));
        assert_eq!(mode_of(&json!({"permission_mode": "something new"})), None);
        assert_eq!(mode_of(&json!({"permission_mode": 3})), None);
        // A subagent reports its own mode.
        assert_eq!(mode_of(&json!({"permission_mode": "plan", "agent_id": "sub"})), None);
    }

    #[test]
    fn session_end_leaves_the_mode_alone() {
        assert_eq!(mode_of(&fixture!("SessionEnd")), None);
    }
}
