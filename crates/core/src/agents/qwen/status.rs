//! Folds Qwen Code's hook events and status line into a session's live status.
//!
//! Qwen's bodies are Claude-shaped (`hook_event_name`, snake_case keys). Nothing here reads a prompt, a
//! `submitted_prompt` or a `last_assistant_message`: the core takes them out of the body at the door. Qwen has no
//! subagent count, Done hold or stuck check, and its status line carries no mode and no plan meter.

use crate::agents::common::{blank, first_line, pct_of, relative, str_at, wants_phrase};
use marshell_protocol::api::{LiveState, LiveStatus, SessionMode};
use serde_json::Value;

/// The permission mode a hook body reports (`permission_mode`): `default`, `plan`, `auto_edit`, `auto` and `yolo`,
/// and `auto-edit` in a `PermissionRequest`. None for a missing field, a non-string or a value this build does not
/// know, so the row keeps the mode it has.
pub fn mode_of(body: &Value) -> Option<SessionMode> {
    Some(match body.get("permission_mode")?.as_str()? {
        "default" => SessionMode::Manual,
        "plan" => SessionMode::Plan,
        "auto_edit" | "auto-edit" => SessionMode::AutoEdit,
        "auto" => SessionMode::FullAuto,
        "yolo" => SessionMode::Bypass,
        _ => return None,
    })
}

/// The file a tool names, relative to the folder Qwen runs in when it is inside it.
fn file_of(body: &Value) -> Option<String> {
    let path = str_at(body, "/tool_input/file_path")?;
    Some(relative(path, str_at(body, "/cwd").unwrap_or("")))
}

/// What a tool is doing, as line 2 says it.
fn tool_phrase(body: &Value) -> String {
    let tool = str_at(body, "/tool_name").unwrap_or("a tool");
    let phrase = match tool {
        "run_shell_command" => str_at(body, "/tool_input/command").map(|c| format!("Running `{}`", first_line(c, 60))),
        "write_file" | "edit" | "replace" => file_of(body).map(|f| format!("Editing `{}`", first_line(&f, 60))),
        "read_file" => file_of(body).map(|f| format!("Reading `{}`", first_line(&f, 60))),
        _ => None,
    };
    phrase.unwrap_or_else(|| format!("Using `{}`", first_line(tool, 60)))
}

/// What a permission request asks for.
fn permission_phrase(body: &Value) -> String {
    let tool = str_at(body, "/tool_name").unwrap_or("a tool");
    let phrase = match tool {
        "run_shell_command" => {
            str_at(body, "/tool_input/command").map(|c| format!("Wants to run `{}`", first_line(c, 60)))
        }
        "write_file" | "edit" | "replace" => file_of(body).map(|f| format!("Wants to edit `{}`", first_line(&f, 60))),
        _ => None,
    };
    phrase.unwrap_or_else(|| format!("Wants to use `{}`", first_line(tool, 60)))
}

/// How long after a permission dialog starts, or after the last turn event while Working, an `idle_prompt` is taken
/// for the one Qwen sent just around it. A "No" in Qwen's own dialog sends its `idle_prompt` about 8 s later (Task 1's
/// recording).
const IDLE_PROMPT_RACE_MS: u64 = 1_000;

/// The status after one hook event. None for an event that says nothing about status. Any event can be the first
/// one a tab sees, so `prev` may be None for all of them.
pub fn fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus> {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    let event = body.get("hook_event_name").and_then(Value::as_str)?;
    if matches!(event, "UserPromptSubmit" | "PreToolUse" | "PostToolUse") {
        next.last_turn_event_ms = now_ms;
    }
    let (state, phrase) = match event {
        "SessionStart" => {
            if let Some(model) = str_at(body, "/model") {
                next.model = Some(model.to_string());
            }
            (LiveState::Idle, String::new())
        }
        // The prompt is gone from the body, so a prompt and a tool result sent back to the model (a second
        // `UserPromptSubmit` with an empty prompt) fold alike: the turn is going on.
        "UserPromptSubmit" => (LiveState::Working, "Thinking".to_string()),
        "PreToolUse" => (LiveState::Working, tool_phrase(body)),
        // A dialog ends with the tool's result: the dialog's phrase no longer holds.
        "PostToolUse" => match next.state {
            LiveState::Working if !next.phrase.is_empty() => (LiveState::Working, next.phrase.clone()),
            _ => (LiveState::Working, "Thinking".to_string()),
        },
        "PermissionRequest" => (LiveState::NeedsPermission, permission_phrase(body)),
        "Notification" => match body.get("notification_type").and_then(Value::as_str) {
            // Qwen sends this 10 to 20 ms after every `PreToolUse`, and a fast tool's `PostToolUse` can come
            // before it, so one within `IDLE_PROMPT_RACE_MS` of the last turn event is that one and the turn goes
            // on. A later one ends the turn: after a "No" in Qwen's own dialog, or Escape mid-turn ("Request
            // cancelled."), no `Stop` comes and this is all that says so. It fires after `Stop` too: a finished
            // turn stays as it is.
            // It also comes about 50 ms before each `PermissionRequest`, through its own bridge process, so one
            // that reaches the core just after the request is that one, not the end of a dialog.
            Some("idle_prompt") => match next.state {
                LiveState::Done | LiveState::Error | LiveState::Idle | LiveState::Ended => {
                    (next.state, next.phrase.clone())
                }
                LiveState::Working if now_ms.saturating_sub(next.last_turn_event_ms) < IDLE_PROMPT_RACE_MS => {
                    (next.state, next.phrase.clone())
                }
                LiveState::NeedsPermission if now_ms.saturating_sub(next.since_ms) < IDLE_PROMPT_RACE_MS => {
                    (next.state, next.phrase.clone())
                }
                _ => (LiveState::Idle, String::new()),
            },
            // Qwen shows its own dialog with no card behind it (released, or no hook decision).
            Some("permission_prompt") => (LiveState::NeedsPermission, wants_phrase(&next.phrase)),
            _ => (next.state, next.phrase.clone()),
        },
        "Stop" => {
            // The only reliable context %: the status line's is 0 for a model Qwen doesn't know.
            if let Some(ratio) = body.get("context_usage").and_then(Value::as_f64) {
                next.context_pct = pct_of(ratio * 100.0);
            }
            (LiveState::Done, "Done".to_string())
        }
        "StopFailure" => (LiveState::Error, crate::agents::claude::status::error_phrase(body)),
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

/// The status after a status line: the model, and the context percentage when Qwen knows the model's window. State,
/// phrase and the `Stop`'s context stay. Qwen sends 0 for the window and the percentage with a model it doesn't
/// know, and that is unknown, never 0 %. There is no mode and no plan meter in it.
pub fn fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    next.model = str_at(body, "/model/display_name").map(str::to_string);
    let window = body
        .pointer("/context_window/context_window_size")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if window > 0.0 {
        next.context_pct = body
            .pointer("/context_window/used_percentage")
            .and_then(Value::as_f64)
            .and_then(pct_of);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    macro_rules! fixture {
        ($dir:literal, $name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/qwen/",
                $dir,
                "/0.26.5/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }
    macro_rules! hook {
        ($name:literal) => {
            fixture!("hooks", $name)
        };
    }

    fn fold(prev: Option<&LiveStatus>, body: &Value, now: u64) -> LiveStatus {
        fold_event(prev, body, now).expect("an event that folds")
    }

    #[test]
    fn session_start_is_idle_with_its_model() {
        for name in [hook!("SessionStart"), hook!("SessionStart-resume")] {
            let live = fold(None, &name, 5);
            assert_eq!(live.state, LiveState::Idle);
            assert_eq!(live.model.as_deref(), Some("marshell-fake"));
        }
    }

    #[test]
    fn a_prompt_is_working() {
        let live = fold(None, &hook!("UserPromptSubmit"), 5);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Working, "Thinking"));
    }

    #[test]
    fn an_empty_prompt_continues_the_turn() {
        let working = fold(None, &hook!("PreToolUse-run_shell_command"), 5);
        let live = fold(Some(&working), &hook!("UserPromptSubmit-continuation"), 9);
        assert_eq!(live.state, LiveState::Working);
        // Still the same turn: the state did not change, so its start does not move.
        assert_eq!(live.since_ms, working.since_ms);
    }

    #[test]
    fn a_shell_tool_says_running_its_command() {
        let live = fold(None, &hook!("PreToolUse-run_shell_command"), 5);
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
    }

    #[test]
    fn a_write_edit_and_read_name_the_file_relative_to_the_folder() {
        assert_eq!(fold(None, &hook!("PreToolUse-write_file"), 5).phrase, "Editing `b.txt`");
        assert_eq!(fold(None, &hook!("PreToolUse-edit"), 5).phrase, "Editing `a.txt`");
        assert_eq!(fold(None, &hook!("PreToolUse-read_file"), 5).phrase, "Reading `a.txt`");
    }

    #[test]
    fn an_unknown_tool_says_using() {
        let body = json!({"hook_event_name": "PreToolUse", "tool_name": "web_fetch", "tool_input": {"url": "x"}});
        assert_eq!(fold(None, &body, 5).phrase, "Using `web_fetch`");
    }

    #[test]
    fn a_tool_result_keeps_the_phrase_and_clears_a_dialog() {
        let running = fold(None, &hook!("PreToolUse-run_shell_command"), 5);
        let after = fold(Some(&running), &hook!("PostToolUse"), 6);
        assert_eq!(
            (after.state, after.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
        let asking = fold(None, &hook!("PermissionRequest-write_file"), 5);
        let after = fold(Some(&asking), &hook!("PostToolUse-write_file"), 6);
        assert_eq!((after.state, after.phrase.as_str()), (LiveState::Working, "Thinking"));
    }

    #[test]
    fn a_permission_request_needs_permission() {
        let live = fold(None, &hook!("PermissionRequest-write_file"), 5);
        assert_eq!(live.state, LiveState::NeedsPermission);
        assert!(live.phrase.starts_with("Wants to edit `"), "{}", live.phrase);
        assert!(live.phrase.ends_with("b.txt`"), "{}", live.phrase);
    }

    #[test]
    fn a_permission_prompt_notification_needs_permission() {
        let writing = fold(None, &hook!("PreToolUse-write_file"), 5);
        let live = fold(Some(&writing), &hook!("Notification-permission_prompt"), 6);
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::NeedsPermission, "Wants to edit `b.txt`")
        );
        let alone = fold(None, &hook!("Notification-permission_prompt"), 6);
        assert_eq!(
            (alone.state, alone.phrase.as_str()),
            (LiveState::NeedsPermission, "Wants to use a tool")
        );
    }

    #[test]
    fn idle_prompt_right_after_a_tool_starts_keeps_working() {
        let working = fold(None, &hook!("PreToolUse-run_shell_command"), 5);
        let live = fold(Some(&working), &hook!("Notification-idle_prompt"), 6);
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
        assert_eq!(live.since_ms, working.since_ms);
    }

    #[test]
    fn idle_prompt_after_a_tool_finished_keeps_working() {
        let started = fold(None, &hook!("PreToolUse-run_shell_command"), 5);
        let finished = fold(Some(&started), &hook!("PostToolUse"), 6);
        let live = fold(Some(&finished), &hook!("Notification-idle_prompt"), 7);
        assert_eq!(live.state, LiveState::Working);
    }

    /// Escape mid-turn with no dialog up: Qwen prints "Request cancelled.", sends no Stop, and its idle_prompt is
    /// what ends the turn.
    #[test]
    fn idle_prompt_long_after_a_tool_started_is_idle() {
        let working = fold(None, &hook!("PreToolUse-run_shell_command"), 5);
        let live = fold(Some(&working), &hook!("Notification-idle_prompt"), 8_000);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Idle, ""));
    }

    #[test]
    fn idle_prompt_long_after_a_prompt_is_idle() {
        let working = fold(None, &hook!("UserPromptSubmit"), 5);
        let live = fold(Some(&working), &hook!("Notification-idle_prompt"), 8_000);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Idle, ""));
    }

    /// The race clock is the last turn event, not the turn's start (`since_ms`).
    #[test]
    fn idle_prompt_just_after_a_later_tool_keeps_the_turn() {
        let prompt = fold(None, &hook!("UserPromptSubmit"), 5);
        let tool = fold(Some(&prompt), &hook!("PreToolUse-run_shell_command"), 20_000);
        let live = fold(Some(&tool), &hook!("Notification-idle_prompt"), 20_015);
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::Working, "Running `echo marshell`")
        );
    }

    #[test]
    fn idle_prompt_after_a_dialog_is_idle() {
        // A "No" in Qwen's own dialog: the next event is an idle_prompt about 8 s later (Task 1's recording).
        let asking = fold(None, &hook!("Notification-permission_prompt"), 5);
        let live = fold(Some(&asking), &hook!("Notification-idle_prompt"), 8_000);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Idle, ""));
    }

    /// Qwen sends an idle_prompt about 50 ms before each PermissionRequest, each through its own bridge process, so
    /// the core can get them the other way round. A card that is waiting must not read Idle.
    #[test]
    fn idle_prompt_racing_a_permission_request_keeps_needs_permission() {
        let working = fold(None, &hook!("PreToolUse-write_file"), 100);
        let asking = fold(Some(&working), &hook!("PermissionRequest-write_file"), 150);
        let live = fold(Some(&asking), &hook!("Notification-idle_prompt"), 200);
        assert_eq!(live.state, LiveState::NeedsPermission);
        assert_eq!(live.phrase, asking.phrase);
    }

    #[test]
    fn idle_prompt_after_stop_leaves_the_finished_turn_done() {
        let done = fold(None, &hook!("Stop"), 5);
        let live = fold(Some(&done), &hook!("Notification-idle_prompt"), 6);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Done, "Done"));
    }

    #[test]
    fn stop_is_done_with_context_from_its_ratio() {
        let live = fold(None, &hook!("Stop"), 5);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Done, "Done"));
        assert_eq!(live.context_pct, Some(1));
        assert_eq!(live.recap, None);
        let mut half = hook!("Stop");
        half["context_usage"] = json!(0.5);
        assert_eq!(fold(None, &half, 5).context_pct, Some(50));
        let mut over = hook!("Stop");
        over["context_usage"] = json!(1.7);
        assert_eq!(fold(None, &over, 5).context_pct, Some(100));
        let mut none = hook!("Stop");
        none.as_object_mut().unwrap().remove("context_usage");
        assert_eq!(fold(None, &none, 5).context_pct, None);
    }

    #[test]
    fn a_stop_failure_is_an_error_with_its_phrase() {
        let body = json!({"hook_event_name": "StopFailure", "error": "rate_limit"});
        let live = fold(None, &body, 5);
        assert_eq!(
            (live.state, live.phrase.as_str()),
            (LiveState::Error, "Rate limit reached")
        );
    }

    #[test]
    fn session_end_ends_it() {
        let live = fold(None, &hook!("SessionEnd"), 5);
        assert_eq!((live.state, live.phrase.as_str()), (LiveState::Ended, "Ended"));
    }

    #[test]
    fn an_event_that_says_nothing_about_status_folds_to_none() {
        assert_eq!(fold_event(None, &json!({"hook_event_name": "PreCompact"}), 5), None);
        assert_eq!(fold_event(None, &json!({}), 5), None);
    }

    #[test]
    fn the_status_line_sets_model_and_context() {
        let mut body = fixture!("statusline", "working");
        body["context_window"]["context_window_size"] = json!(200000);
        body["context_window"]["used_percentage"] = json!(37.4);
        let live = fold_statusline(None, &body, 5);
        assert_eq!(live.model.as_deref(), Some("marshell-fake"));
        assert_eq!(live.context_pct, Some(37));
        assert_eq!(
            (live.five_hour_pct, live.seven_day_pct, live.effort),
            (None, None, None)
        );
    }

    #[test]
    fn a_status_line_with_no_window_size_leaves_context_unknown() {
        let live = fold_statusline(None, &fixture!("statusline", "idle"), 5);
        assert_eq!(live.model.as_deref(), Some("marshell-fake"));
        assert_eq!(live.context_pct, None);
        // What `Stop` measured stays: the line's 0 is no measurement.
        let stopped = fold(None, &hook!("Stop"), 4);
        let live = fold_statusline(Some(&stopped), &fixture!("statusline", "working"), 5);
        assert_eq!(live.context_pct, Some(1));
        assert_eq!(live.state, LiveState::Done);
    }

    #[test]
    fn mode_of_reads_both_auto_edit_spellings() {
        for spelling in ["auto_edit", "auto-edit"] {
            assert_eq!(
                mode_of(&json!({"permission_mode": spelling})),
                Some(SessionMode::AutoEdit)
            );
        }
        assert_eq!(mode_of(&json!({"permission_mode": "acceptEdits"})), None);
        assert_eq!(mode_of(&json!({"permission_mode": 3})), None);
        assert_eq!(mode_of(&json!({})), None);
    }

    #[test]
    fn mode_of_reads_each_recorded_mode() {
        for (name, want) in [
            (hook!("UserPromptSubmit"), SessionMode::Manual),
            (hook!("UserPromptSubmit-auto_edit"), SessionMode::AutoEdit),
            (hook!("UserPromptSubmit-auto"), SessionMode::FullAuto),
            (hook!("UserPromptSubmit-yolo"), SessionMode::Bypass),
            (hook!("UserPromptSubmit-plan"), SessionMode::Plan),
        ] {
            assert_eq!(mode_of(&name), Some(want));
        }
    }
}
