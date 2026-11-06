//! Folds Copilot CLI's hook events and status line into a session's live status.
//!
//! Copilot's bodies are camelCase (`toolName`, `toolArgs`), and carry no `hook_event_name` except `notification`: the
//! bridge adds it from its `--event` flag. Nothing here reads a prompt or an `initialPrompt`: the core takes them out
//! of the body at the door. Copilot's bodies carry no mode, and its status line carries no plan meter, so the row
//! keeps its launch mode.

use crate::agents::common::{blank, first_line, pct_of, relative, str_at, wants_phrase};
use marshell_protocol::api::{LiveState, LiveStatus};
use serde_json::Value;

/// A tool's arguments: `toolArgs` is an object in 1.0.106, and a JSON string is parsed first.
fn tool_args(body: &Value) -> Option<Value> {
    match body.get("toolArgs")? {
        Value::String(text) => serde_json::from_str(text).ok(),
        other => Some(other.clone()),
    }
}

/// The file a tool names, relative to the folder Copilot runs in when it is inside it.
fn file_of(args: &Value, body: &Value) -> Option<String> {
    let path = str_at(args, "/path")?;
    Some(relative(path, str_at(body, "/cwd").unwrap_or("")))
}

/// What a tool is doing, as line 2 says it.
fn tool_phrase(body: &Value) -> String {
    let tool = str_at(body, "/toolName").unwrap_or("a tool");
    let args = tool_args(body).unwrap_or(Value::Null);
    let phrase = match tool {
        "powershell" | "bash" | "shell" => {
            str_at(&args, "/command").map(|c| format!("Running `{}`", first_line(c, 60)))
        }
        "create" | "edit" => file_of(&args, body).map(|f| format!("Editing `{}`", first_line(&f, 60))),
        "view" => file_of(&args, body).map(|f| format!("Reading `{}`", first_line(&f, 60))),
        _ => None,
    };
    phrase.unwrap_or_else(|| format!("Using `{}`", first_line(tool, 60)))
}

/// Why Copilot stopped, as the row says it: the first line of its own `error.message` (`error` is
/// `{message, name, stack?}`), else "API error".
fn error_phrase(body: &Value) -> String {
    str_at(body, "/error/message")
        .map(|message| first_line(message.trim(), 120))
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "API error".to_string())
}

/// The status after one hook event. None for an event that says nothing about status. Any event can be the first
/// one a tab sees, so `prev` may be None for all of them.
pub fn fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus> {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    let event = body.get("hook_event_name").and_then(Value::as_str)?;
    let (state, phrase) = match event {
        "sessionStart" => (LiveState::Idle, String::new()),
        "userPromptSubmitted" => (LiveState::Working, "Thinking".to_string()),
        "preToolUse" => (LiveState::Working, tool_phrase(body)),
        // A dialog ends with the tool's result: the dialog's phrase no longer holds.
        "postToolUse" | "postToolUseFailure" => match next.state {
            LiveState::Working if !next.phrase.is_empty() => (LiveState::Working, next.phrase.clone()),
            _ => (LiveState::Working, "Thinking".to_string()),
        },
        // The body names its event `Notification`.
        "notification" | "Notification" => match body.get("notification_type").and_then(Value::as_str) {
            // Copilot's own dialog, with no card behind it.
            Some("permission_prompt") => (LiveState::NeedsPermission, wants_phrase(&next.phrase)),
            // `agent_idle` is a background agent, not the turn ending.
            _ => return None,
        },
        "agentStop" => (LiveState::Done, "Done".to_string()),
        "errorOccurred" if body.get("recoverable").and_then(Value::as_bool) == Some(false) => {
            (LiveState::Idle, error_phrase(body))
        }
        "sessionEnd" => (LiveState::Ended, "Ended".to_string()),
        _ => return None,
    };
    if prev.is_some() && state != next.state {
        next.since_ms = now_ms;
    }
    next.state = state;
    next.phrase = phrase;
    Some(next)
}

/// The status after a status line: the model, and the context percentage when Copilot knows the model's window (a
/// size of 0 or none is unknown, never 0 %). State and phrase stay, except on an Ended row: only a running Copilot
/// runs its status line, and none runs after `sessionEnd` (`/exit`), so a status line is a Copilot that runs again
/// (`/clear`'s new session, or a resumed one that sends nothing else until its first prompt). There is no mode and
/// no plan meter in it.
pub fn fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    if next.state == LiveState::Ended {
        next.state = LiveState::Idle;
        next.phrase = String::new();
        next.since_ms = now_ms;
    }
    next.model = str_at(body, "/model/display_name").map(str::to_string);
    let window = body
        .pointer("/context_window/context_window_size")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    next.context_pct = if window > 0.0 {
        body.pointer("/context_window/used_percentage")
            .and_then(Value::as_f64)
            .and_then(pct_of)
    } else {
        None
    };
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    macro_rules! fixture {
        ($dir:literal, $name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/copilot/",
                $dir,
                "/1.0.106/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    /// A recorded hook body with the `hook_event_name` the bridge adds from its `--event` flag.
    fn hook(event: &str, mut body: Value) -> Value {
        if body.get("hook_event_name").is_none() {
            body["hook_event_name"] = json!(event);
        }
        body
    }
    macro_rules! hook {
        ($event:literal, $name:literal) => {
            hook($event, fixture!("hooks", $name))
        };
    }

    fn fold(prev: Option<&LiveStatus>, body: &Value, now: u64) -> LiveStatus {
        fold_event(prev, body, now).expect("an event that folds")
    }

    #[test]
    fn session_start_is_idle() {
        for body in [
            hook!("sessionStart", "sessionStart"),
            hook!("sessionStart", "sessionStart-clear"),
            hook!("sessionStart", "sessionStart-resume"),
        ] {
            let live = fold(None, &body, 5);
            assert_eq!((live.state, live.phrase.as_str()), (LiveState::Idle, ""));
        }
        let working = fold(None, &hook!("userPromptSubmitted", "userPromptSubmitted"), 5);
        assert_eq!(
            fold(Some(&working), &hook!("sessionStart", "sessionStart-clear"), 9).state,
            LiveState::Idle
        );
    }

    #[test]
    fn a_prompt_is_working() {
        let live = fold(None, &hook!("userPromptSubmitted", "userPromptSubmitted"), 5);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Working, "Thinking"));
    }

    #[test]
    fn a_shell_tool_says_running_its_command() {
        let live = fold(None, &hook!("preToolUse", "preToolUse-powershell"), 5);
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
        for tool in ["bash", "shell"] {
            let mut body = hook!("preToolUse", "preToolUse-powershell");
            body["toolName"] = json!(tool);
            assert_eq!(fold(None, &body, 5).phrase, "Running `echo marshell`", "{tool}");
        }
    }

    #[test]
    fn create_and_edit_say_editing_and_view_says_reading() {
        assert_eq!(
            fold(None, &hook!("preToolUse", "preToolUse-create"), 5).phrase,
            "Editing `c.txt`"
        );
        assert_eq!(
            fold(None, &hook!("preToolUse", "preToolUse-edit"), 5).phrase,
            "Editing `edit-me.txt`"
        );
        assert_eq!(
            fold(None, &hook!("preToolUse", "preToolUse-view"), 5).phrase,
            "Reading `edit-me.txt`"
        );
        // A file outside the folder keeps its full path, and any other tool says Using.
        let mut outside = hook!("preToolUse", "preToolUse-view");
        outside["toolArgs"]["path"] = json!("D:\\elsewhere\\x.txt");
        assert_eq!(fold(None, &outside, 5).phrase, "Reading `D:\\elsewhere\\x.txt`");
        let other = json!({"hook_event_name": "preToolUse", "toolName": "web_fetch", "toolArgs": {"url": "x"}});
        assert_eq!(fold(None, &other, 5).phrase, "Using `web_fetch`");
        let none = json!({"hook_event_name": "preToolUse"});
        assert_eq!(fold(None, &none, 5).phrase, "Using `a tool`");
    }

    #[test]
    fn tool_args_as_a_json_string_are_read() {
        let mut body = hook!("preToolUse", "preToolUse-powershell");
        body["toolArgs"] = json!(body["toolArgs"].to_string());
        assert!(body["toolArgs"].is_string());
        assert_eq!(fold(None, &body, 5).phrase, "Running `echo marshell`");
        let mut edit = hook!("preToolUse", "preToolUse-edit");
        edit["toolArgs"] = json!(edit["toolArgs"].to_string());
        assert_eq!(fold(None, &edit, 5).phrase, "Editing `edit-me.txt`");
        let mut broken = hook!("preToolUse", "preToolUse-powershell");
        broken["toolArgs"] = json!("not json {");
        assert_eq!(fold(None, &broken, 5).phrase, "Using `powershell`");
    }

    #[test]
    fn a_permission_prompt_notification_needs_permission() {
        let running = fold(None, &hook!("preToolUse", "preToolUse-powershell"), 5);
        let live = fold(
            Some(&running),
            &hook!("notification", "notification-permission_prompt"),
            6,
        );
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::NeedsPermission, "Wants to run `echo marshell`")
        );
        let alone = fold(None, &hook!("notification", "notification-permission_prompt"), 6);
        assert_eq!(
            (alone.state, alone.phrase.as_str()),
            (LiveState::NeedsPermission, "Wants to use a tool")
        );
    }

    #[test]
    fn agent_idle_changes_nothing() {
        let running = fold(None, &hook!("preToolUse", "preToolUse-powershell"), 5);
        let mut body = hook!("notification", "notification-permission_prompt");
        body["notification_type"] = json!("agent_idle");
        assert_eq!(fold_event(Some(&running), &body, 6), None);
        assert_eq!(fold_event(None, &body, 6), None);
    }

    #[test]
    fn post_tool_use_after_a_permission_prompt_is_working() {
        let asking = fold(None, &hook!("notification", "notification-permission_prompt"), 5);
        for body in [
            hook!("postToolUse", "postToolUse"),
            hook!("postToolUseFailure", "postToolUseFailure"),
        ] {
            let live = fold(Some(&asking), &body, 6);
            assert_eq!((live.state, live.phrase.as_str()), (LiveState::Working, "Thinking"));
        }
        // A tool's result keeps what the tool was doing.
        let running = fold(None, &hook!("preToolUse", "preToolUse-powershell"), 5);
        let after = fold(Some(&running), &hook!("postToolUse", "postToolUse"), 6);
        assert_eq!(
            (after.state, after.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
        assert_eq!(after.since_ms, running.since_ms);
    }

    #[test]
    fn agent_stop_is_done() {
        let live = fold(None, &hook!("agentStop", "agentStop"), 5);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Done, "Done"));
    }

    #[test]
    fn an_unrecoverable_error_is_idle_with_its_phrase() {
        let body = json!({
            "hook_event_name": "errorOccurred",
            "error": {"message": "boom", "name": "Error"},
            "errorContext": "model_call",
            "recoverable": false
        });
        let working = fold(None, &hook!("userPromptSubmitted", "userPromptSubmitted"), 5);
        let live = fold(Some(&working), &body, 9);
        // Copilot's own message, first line only; Claude Code's error names are not Copilot's.
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Idle, "boom"));
        assert_eq!(live.since_ms, 9);
        let mut two_lines = body.clone();
        two_lines["error"]["message"] = json!("Model call failed\nat stack");
        assert_eq!(fold(Some(&working), &two_lines, 9).phrase, "Model call failed");
        for error in [json!({"name": "Error"}), json!({"message": "  "}), json!("rate_limit")] {
            let mut other = body.clone();
            other["error"] = error;
            assert_eq!(fold(Some(&working), &other, 9).phrase, "API error");
        }
    }

    #[test]
    fn a_recoverable_error_changes_nothing() {
        let working = fold(None, &hook!("userPromptSubmitted", "userPromptSubmitted"), 5);
        for recoverable in [json!(true), json!(null)] {
            let body =
                json!({"hook_event_name": "errorOccurred", "error": {"message": "x"}, "recoverable": recoverable});
            assert_eq!(fold_event(Some(&working), &body, 9), None);
        }
    }

    #[test]
    fn session_end_ends_it() {
        let live = fold(None, &hook!("sessionEnd", "sessionEnd"), 5);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Ended, "Ended"));
    }

    #[test]
    fn an_event_that_says_nothing_about_status_folds_to_none() {
        assert_eq!(fold_event(None, &json!({"hook_event_name": "preCompact"}), 5), None);
        assert_eq!(fold_event(None, &json!({}), 5), None);
    }

    #[test]
    fn a_status_line_after_an_end_is_idle_again() {
        let working = fold(None, &hook!("userPromptSubmitted", "userPromptSubmitted"), 5);
        let ended = fold(Some(&working), &hook!("sessionEnd", "sessionEnd-clear"), 6);
        assert_eq!(ended.state, LiveState::Ended);
        let live = fold_statusline(Some(&ended), &fixture!("statusline", "idle"), 7);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Idle, ""));
        assert_eq!(live.since_ms, 7);
        assert_eq!(live.model.as_deref(), Some("marshell-fake"));
        assert_eq!(live.context_pct, Some(0));
    }

    #[test]
    fn the_status_line_sets_model_and_context() {
        let working = fold(None, &hook!("preToolUse", "preToolUse-powershell"), 5);
        let live = fold_statusline(Some(&working), &fixture!("statusline", "working"), 9);
        assert_eq!(live.model.as_deref(), Some("marshell-fake"));
        assert_eq!(live.context_pct, Some(1));
        // State, phrase and the turn's start stay.
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
        assert_eq!(live.since_ms, working.since_ms);
        assert_eq!(
            (live.five_hour_pct, live.seven_day_pct, live.effort),
            (None, None, None)
        );
        let mut body = fixture!("statusline", "working");
        body["context_window"]["used_percentage"] = json!(37.4);
        assert_eq!(fold_statusline(None, &body, 5).context_pct, Some(37));
    }

    #[test]
    fn a_status_line_with_no_window_size_leaves_context_unknown() {
        for window in [json!(0), json!(null)] {
            let mut body = fixture!("statusline", "working");
            body["context_window"]["context_window_size"] = window;
            body["context_window"]["used_percentage"] = json!(0);
            let live = fold_statusline(None, &body, 5);
            assert_eq!(live.model.as_deref(), Some("marshell-fake"));
            assert_eq!(live.context_pct, None);
        }
        let mut gone = fixture!("statusline", "idle");
        gone["context_window"]
            .as_object_mut()
            .unwrap()
            .remove("context_window_size");
        assert_eq!(fold_statusline(None, &gone, 5).context_pct, None);
        // A known percentage doesn't linger once the window is unknown.
        let known = fold_statusline(None, &fixture!("statusline", "working"), 5);
        assert_eq!(known.context_pct, Some(1));
        assert_eq!(fold_statusline(Some(&known), &gone, 6).context_pct, None);
    }
}
