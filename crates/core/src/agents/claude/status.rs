//! Folds Claude Code's hook events and status line into a session's live status.

use crate::agents::common::blank;
use marshell_protocol::api::{LiveState, LiveStatus, Recap, SessionMode};
use serde_json::Value;

/// The main turn has stopped while background subagents are still running.
pub const WAITING_PHRASE: &str = "Waiting for subagents";

/// A ring's colour band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingLevel {
    Green,
    Yellow,
    Red,
}

/// The permission mode a hook body reports (`permission_mode`). None for a missing field, a
/// non-string or a value this build does not know, so the row keeps the mode it has. A subagent's
/// own events (they carry `agent_id`) report the subagent's mode, not the session's: None too.
pub fn mode_of(body: &Value) -> Option<SessionMode> {
    if body.get("agent_id").is_some() {
        return None;
    }
    Some(match body.get("permission_mode")?.as_str()? {
        "default" | "manual" => SessionMode::Manual,
        "plan" => SessionMode::Plan,
        "acceptEdits" => SessionMode::AutoEdit,
        "auto" => SessionMode::FullAuto,
        "bypassPermissions" => SessionMode::Bypass,
        "dontAsk" => SessionMode::DontAsk,
        _ => return None,
    })
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

/// Folds a hook body into the set of subagent ids that are running. `SubagentStart` adds its
/// `agent_id` and `SubagentStop` takes it out. The main `Stop` keeps only the subagents its
/// `background_tasks` lists as running (Claude Code runs some in the background, past the turn's
/// end), or empties the set when the list is missing; `SessionEnd` empties it. So a missed
/// SubagentStop never leaves a stale count. Anything else, or a Start or Stop without a string
/// `agent_id`, changes nothing. True when the set changed.
pub fn fold_subagents(running: &mut std::collections::BTreeSet<String>, body: &Value) -> bool {
    let id = body.get("agent_id").and_then(Value::as_str);
    let keep = |set: &mut std::collections::BTreeSet<String>, now: std::collections::BTreeSet<String>| {
        let changed = *set != now;
        *set = now;
        changed
    };
    match body.get("hook_event_name").and_then(Value::as_str) {
        Some("SubagentStart") => id.is_some_and(|id| running.insert(id.to_string())),
        Some("SubagentStop") => id.is_some_and(|id| running.remove(id)),
        Some("Stop") => keep(
            running,
            running_subagents(body)
                .filter_map(|t| t.get("id")?.as_str().map(String::from))
                .collect(),
        ),
        Some("SessionEnd") => keep(running, Default::default()),
        _ => false,
    }
}

/// The background tasks the main turn reports as running subagents.
pub(crate) fn running_subagents(body: &Value) -> impl Iterator<Item = &Value> {
    body.get("background_tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|t| {
            t.get("type").and_then(Value::as_str) == Some("subagent")
                && t.get("status").and_then(Value::as_str) == Some("running")
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

/// A tool phrase turned into what a permission prompt asks, "Running `x`" into "Wants to run `x`".
fn wants_phrase(phrase: &str) -> String {
    const VERBS: [(&str, &str); 4] = [
        ("Running ", "Wants to run "),
        ("Editing ", "Wants to edit "),
        ("Reading ", "Wants to read "),
        ("Using ", "Wants to use "),
    ];
    if phrase.starts_with("Wants to") {
        return phrase.to_string();
    }
    for (seen, wants) in VERBS {
        if let Some(target) = phrase.strip_prefix(seen) {
            return format!("{wants}{target}");
        }
    }
    "Wants to use a tool".to_string()
}

/// Why a turn failed, as the row says it: our line for a known `error`, else Claude's own text.
fn error_phrase(body: &Value) -> String {
    let line = match str_at(body, "/error") {
        Some("rate_limit") => "Rate limit reached",
        Some("overloaded") => "Claude is overloaded",
        Some("authentication_failed") => "Sign-in failed",
        Some("oauth_org_not_allowed") => "Organization not allowed",
        Some("account_on_hold") => "Account on hold",
        Some("billing_error") => "Billing problem",
        Some("invalid_request") => "Invalid request",
        Some("model_not_found") => "Model not found",
        Some("server_error") => "Server error",
        Some("max_output_tokens") => "Reply hit the output limit",
        Some("cloud_credential_error") => "Cloud sign-in failed",
        _ => {
            return str_at(body, "/last_assistant_message")
                .map(|text| first_line(text, 120))
                .unwrap_or_else(|| "API error".to_string());
        }
    };
    line.to_string()
}

/// The status after one hook event. None for an event that says nothing about status.
pub fn fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus> {
    let mut next = prev.cloned().unwrap_or_else(|| blank(now_ms));
    let (state, phrase) = match body.get("hook_event_name").and_then(Value::as_str)? {
        "SessionStart" => (LiveState::Idle, String::new()),
        "UserPromptSubmit" => (LiveState::Working, "Thinking".to_string()),
        "PreToolUse" if str_at(body, "/tool_name") == Some("AskUserQuestion") => {
            (LiveState::NeedsQuestion, "Has a question".to_string())
        }
        "PreToolUse" => (LiveState::Working, tool_phrase(body)),
        // A question or a permission ends with the tool's result: the question's phrase no longer holds.
        "PostToolUse" | "PostToolBatch" => match next.state {
            LiveState::NeedsPermission | LiveState::NeedsQuestion => (LiveState::Working, "Thinking".to_string()),
            _ => (LiveState::Working, next.phrase.clone()),
        },
        "PermissionRequest" => (LiveState::NeedsPermission, permission_phrase(body)),
        "Notification" => match body.get("notification_type").and_then(Value::as_str) {
            // Claude sends this a minute after every Stop, so a finished turn stays Done. A turn that
            // is still marked busy was interrupted, and Claude sends no Stop for that.
            Some("idle_prompt") => match next.state {
                LiveState::Done | LiveState::Error | LiveState::Idle | LiveState::Ended => {
                    (next.state, next.phrase.clone())
                }
                // The turn is over but its subagents still run.
                LiveState::Working if next.phrase == WAITING_PHRASE => (next.state, next.phrase.clone()),
                _ => (LiveState::Idle, String::new()),
            },
            Some("permission_prompt") => (LiveState::NeedsPermission, wants_phrase(&next.phrase)),
            _ => (next.state, next.phrase.clone()),
        },
        "Stop" => {
            if let Some(text) = str_at(body, "/last_assistant_message") {
                next.recap = Some(Recap {
                    text: first_line(text, 200),
                    at_ms: now_ms,
                });
            }
            if running_subagents(body).next().is_some() {
                (LiveState::Working, WAITING_PHRASE.to_string())
            } else {
                (LiveState::Done, "Done".to_string())
            }
        }
        // A turn that ended on an API error: Claude sends this in place of Stop. The recap stays.
        "StopFailure" => (LiveState::Error, error_phrase(body)),
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
    next.five_hour_resets_at = body.pointer("/rate_limits/five_hour/resets_at").and_then(Value::as_u64);
    next.seven_day_pct = pct_at(body, "/rate_limits/seven_day/used_percentage");
    next
}

/// Measures current input tokens against the compact window, capped by the model's window.
/// Without a compact window or input count, keeps the status line's own percentage.
pub fn fold_statusline_with_window(
    prev: Option<&LiveStatus>,
    body: &Value,
    now_ms: u64,
    window: Option<u64>,
) -> LiveStatus {
    let mut next = fold_statusline(prev, body, now_ms);
    if let Some(window) = window
        && let Some(tokens) = body
            .pointer("/context_window/total_input_tokens")
            .and_then(Value::as_u64)
    {
        let model_window = body
            .pointer("/context_window/context_window_size")
            .and_then(Value::as_u64);
        let effective = model_window.map_or(window, |size| size.min(window));
        if effective > 0 {
            next.context_pct = Some((tokens as f64 * 100.0 / effective as f64).round().clamp(0.0, 100.0) as u8);
        }
    }
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

    #[test]
    fn statusline_uses_the_compact_window() {
        let body = fixture!("statusline");
        assert_eq!(
            fold_statusline_with_window(None, &body, 1, Some(300_000)).context_pct,
            Some(19)
        );
        assert_eq!(fold_statusline_with_window(None, &body, 1, None).context_pct, Some(6));
    }

    #[test]
    fn statusline_uses_the_smaller_model_window() {
        let mut body = fixture!("statusline");
        body["context_window"]["context_window_size"] = json!(200000);
        assert_eq!(
            fold_statusline_with_window(None, &body, 1, Some(300_000)).context_pct,
            Some(29)
        );
    }

    #[test]
    fn statusline_without_input_tokens_keeps_used_percentage() {
        let body = json!({"context_window": {"used_percentage": 6}});
        assert_eq!(
            fold_statusline_with_window(None, &body, 1, Some(300_000)).context_pct,
            Some(6)
        );
    }

    #[test]
    fn statusline_rounds_and_clamps_with_no_model_window() {
        for (tokens, want) in [(0, 0), (59500, 20), (400000, 100)] {
            let body = json!({"context_window": {"total_input_tokens": tokens}});
            assert_eq!(
                fold_statusline_with_window(None, &body, 1, Some(300_000)).context_pct,
                Some(want)
            );
        }
    }

    #[test]
    fn mode_of_maps_every_documented_value() {
        let of = |v: Value| mode_of(&json!({ "permission_mode": v }));
        assert_eq!(of(json!("default")), Some(SessionMode::Manual));
        assert_eq!(of(json!("manual")), Some(SessionMode::Manual));
        assert_eq!(of(json!("plan")), Some(SessionMode::Plan));
        assert_eq!(of(json!("acceptEdits")), Some(SessionMode::AutoEdit));
        assert_eq!(of(json!("auto")), Some(SessionMode::FullAuto));
        assert_eq!(of(json!("bypassPermissions")), Some(SessionMode::Bypass));
        assert_eq!(of(json!("dontAsk")), Some(SessionMode::DontAsk));
        assert_eq!(of(json!("somethingNew")), None);
        assert_eq!(of(json!(3)), None);
        assert_eq!(mode_of(&json!({})), None);
    }

    // A subagent's own events report its mode, which a custom agent can set for itself; the row follows the session's.
    #[test]
    fn mode_of_skips_a_subagents_own_events() {
        let sub = json!({ "permission_mode": "plan", "agent_id": "a1", "hook_event_name": "PreToolUse" });
        assert_eq!(mode_of(&sub), None);
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
    fn a_permission_notification_needs_you() {
        let pre = fold(None, &fixture!("PreToolUse"));
        let n = json!({"hook_event_name":"Notification","notification_type":"permission_prompt","message":"x"});
        let s = fold(Some(&pre), &n);
        assert_eq!(s.state, LiveState::NeedsPermission);
        assert_eq!(s.since_ms, 1_000);
    }

    #[test]
    fn idle_prompt_keeps_done_and_its_time() {
        let done = fold_event(None, &fixture!("Stop"), 1_000).unwrap();
        let idle = json!({"hook_event_name":"Notification","notification_type":"idle_prompt"});
        let s = fold_event(Some(&done), &idle, 61_000).unwrap();
        assert_eq!(s.state, LiveState::Done);
        assert_eq!(s.phrase, "Done");
        assert_eq!(s.since_ms, 1_000);
        assert_eq!(s, done);
    }

    #[test]
    fn idle_prompt_keeps_a_wait_for_subagents() {
        let mut waiting = blank(1_000);
        waiting.state = LiveState::Working;
        waiting.phrase = WAITING_PHRASE.to_string();
        let idle = json!({"hook_event_name":"Notification","notification_type":"idle_prompt"});
        let s = fold_event(Some(&waiting), &idle, 61_000).unwrap();
        assert_eq!(s.state, LiveState::Working);
        assert_eq!(s.phrase, WAITING_PHRASE);
        assert_eq!(s, waiting);
    }

    #[test]
    fn idle_prompt_ends_a_stuck_turn() {
        let pre = fold(None, &fixture!("PreToolUse"));
        let idle = json!({"hook_event_name":"Notification","notification_type":"idle_prompt"});
        let s = fold(Some(&pre), &idle);
        assert_eq!(s.state, LiveState::Idle);
        assert_eq!(s.phrase, "");
    }

    #[test]
    fn a_permission_prompt_says_what_it_wants_to_run() {
        let pre = fold(None, &fixture!("PreToolUse"));
        let bash = json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"npm test"}});
        let pre = fold(Some(&pre), &bash);
        let n = json!({"hook_event_name":"Notification","notification_type":"permission_prompt","message":"x"});
        let s = fold(Some(&pre), &n);
        assert_eq!(s.state, LiveState::NeedsPermission);
        assert_eq!(s.phrase, "Wants to run `npm test`");
    }

    #[test]
    fn a_permission_prompt_says_what_it_wants_to_edit_read_or_use() {
        let n = json!({"hook_event_name":"Notification","notification_type":"permission_prompt"});
        let asks = |tool: Value| fold(Some(&fold(None, &tool)), &n).phrase;
        let edit = json!({"hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"src/auth.ts"}});
        assert_eq!(asks(edit), "Wants to edit `src/auth.ts`");
        let read = json!({"hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"a.rs"}});
        assert_eq!(asks(read), "Wants to read `a.rs`");
        let other = json!({"hook_event_name":"PreToolUse","tool_name":"WebFetch","tool_input":{}});
        assert_eq!(asks(other), "Wants to use `WebFetch`");
        let prompt = json!({"hook_event_name":"UserPromptSubmit"});
        assert_eq!(asks(prompt), "Wants to use a tool");
        assert_eq!(fold(None, &n).phrase, "Wants to use a tool");
    }

    #[test]
    fn a_permission_prompt_after_a_permission_request_keeps_its_phrase() {
        let asked = fold(None, &fixture!("PermissionRequest"));
        let n = json!({"hook_event_name":"Notification","notification_type":"permission_prompt"});
        assert_eq!(fold(Some(&asked), &n).phrase, asked.phrase);
    }

    #[test]
    fn the_tool_result_after_a_question_or_a_permission_is_working_again() {
        let q = json!({"hook_event_name":"PreToolUse","tool_name":"AskUserQuestion","tool_input":{"questions":[]}});
        let post = json!({"hook_event_name":"PostToolUse","tool_name":"AskUserQuestion"});
        let s = fold(Some(&fold(None, &q)), &post);
        assert_eq!((s.state, s.phrase.as_str()), (LiveState::Working, "Thinking"));
        let bash = json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"npm test"}});
        let n = json!({"hook_event_name":"Notification","notification_type":"permission_prompt"});
        let asked = fold(Some(&fold(None, &bash)), &n);
        let s = fold(
            Some(&asked),
            &json!({"hook_event_name":"PostToolUse","tool_name":"Bash"}),
        );
        assert_eq!((s.state, s.phrase.as_str()), (LiveState::Working, "Thinking"));
    }

    #[test]
    fn ask_user_question_needs_you() {
        let q = json!({"hook_event_name":"PreToolUse","tool_name":"AskUserQuestion","tool_input":{"questions":[]}});
        let s = fold(None, &q);
        assert_eq!(s.state, LiveState::NeedsQuestion);
        assert_eq!(s.phrase, "Has a question");
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
    fn the_status_line_gives_when_the_five_hour_window_resets() {
        let s = fold_statusline(None, &fixture!("statusline"), 1);
        assert_eq!(s.five_hour_resets_at, Some(1792357200));
    }

    #[test]
    fn a_missing_or_odd_reset_time_is_unknown() {
        let s = fold_statusline(None, &json!({"rate_limits":{"five_hour":{"used_percentage":7}}}), 1);
        assert_eq!(s.five_hour_resets_at, None);
        let s = fold_statusline(None, &json!({"rate_limits":{"five_hour":{"resets_at":"soon"}}}), 1);
        assert_eq!(s.five_hour_resets_at, None);
        let full = fold_statusline(None, &fixture!("statusline"), 1);
        assert_eq!(fold_statusline(Some(&full), &json!({}), 2).five_hour_resets_at, None);
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

    macro_rules! fixture_309 {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/claude/2.1.309/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    #[test]
    fn stop_failure_is_error_with_our_line() {
        let done = fold_event(None, &fixture!("Stop"), 1_000).unwrap();
        let s = fold_event(Some(&done), &fixture_309!("StopFailure"), 5_000).unwrap();
        assert_eq!(s.state, LiveState::Error);
        assert_eq!(s.phrase, "Sign-in failed");
        assert_eq!(s.since_ms, 5_000);
        assert_eq!(s.recap, done.recap);
    }

    #[test]
    fn stop_failure_has_a_line_for_every_known_error() {
        for (error, line) in [
            ("rate_limit", "Rate limit reached"),
            ("overloaded", "Claude is overloaded"),
            ("authentication_failed", "Sign-in failed"),
            ("oauth_org_not_allowed", "Organization not allowed"),
            ("account_on_hold", "Account on hold"),
            ("billing_error", "Billing problem"),
            ("invalid_request", "Invalid request"),
            ("model_not_found", "Model not found"),
            ("server_error", "Server error"),
            ("max_output_tokens", "Reply hit the output limit"),
            ("cloud_credential_error", "Cloud sign-in failed"),
        ] {
            let body = json!({"hook_event_name":"StopFailure","error":error,"last_assistant_message":"ignored"});
            assert_eq!(fold(None, &body).phrase, line, "{error}");
        }
    }

    #[test]
    fn stop_failure_with_unknown_type_uses_claudes_text() {
        let body = json!({"hook_event_name":"StopFailure","error":"new_thing","last_assistant_message":"API Error: Something new
more"});
        let s = fold(None, &body);
        assert_eq!(s.state, LiveState::Error);
        assert_eq!(s.phrase, "API Error: Something new");
    }

    #[test]
    fn stop_failure_with_no_text_says_api_error() {
        let body = json!({"hook_event_name":"StopFailure","error":"unknown"});
        assert_eq!(fold(None, &body).phrase, "API error");
        let empty = json!({"hook_event_name":"StopFailure","error":"unknown","last_assistant_message":"  "});
        assert_eq!(fold(None, &empty).phrase, "API error");
    }

    #[test]
    fn idle_prompt_keeps_error() {
        let failed = fold_event(None, &fixture_309!("StopFailure"), 1_000).unwrap();
        let idle = json!({"hook_event_name":"Notification","notification_type":"idle_prompt"});
        let s = fold_event(Some(&failed), &idle, 61_000).unwrap();
        assert_eq!(s.state, LiveState::Error);
        assert_eq!(s.phrase, failed.phrase);
        assert_eq!(s.since_ms, failed.since_ms);
    }

    macro_rules! fixture_305 {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/claude/2.1.305/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    #[test]
    fn a_stop_with_running_background_subagents_keeps_working() {
        let working = fold(None, &fixture!("UserPromptSubmit"));
        let s = fold_event(Some(&working), &fixture_305!("Stop-background"), 7_000).unwrap();
        assert_eq!(s.state, LiveState::Working);
        assert_eq!(s.phrase, WAITING_PHRASE);
        let recap = s.recap.expect("a recap while waiting");
        assert!(
            recap
                .text
                .starts_with("I've started both Explore agents at the same time.")
        );
        assert_eq!(recap.at_ms, 7_000);
    }

    #[test]
    fn a_stop_without_running_subagents_is_done() {
        let working = fold(None, &fixture!("UserPromptSubmit"));
        for body in [
            json!({ "hook_event_name": "Stop", "background_tasks": [] }),
            json!({ "hook_event_name": "Stop", "background_tasks": [
                { "type": "subagent", "status": "completed" },
                { "type": "shell", "status": "running" },
            ] }),
            json!({ "hook_event_name": "Stop" }),
        ] {
            let s = fold(Some(&working), &body);
            assert_eq!(s.state, LiveState::Done, "{body}");
            assert_eq!(s.phrase, "Done", "{body}");
        }
    }

    // Recorded on 2.1.305: Claude ran the agent in the background, so the main Stop came while it still ran.
    #[test]
    fn a_background_subagent_outlives_the_main_stop() {
        use std::collections::BTreeSet;
        let mut set = BTreeSet::new();
        assert!(fold_subagents(&mut set, &fixture_305!("SubagentStart")));
        assert_eq!(set.len(), 1);
        assert!(!fold_subagents(&mut set, &fixture_305!("Stop-background")));
        assert_eq!(set.len(), 1);
        assert!(fold_subagents(&mut set, &fixture_305!("SubagentStop")));
        assert!(set.is_empty());
    }

    #[test]
    fn the_main_stop_keeps_only_the_subagents_it_lists_as_running() {
        use std::collections::BTreeSet;
        let mut set: BTreeSet<String> = ["a", "b", "c"].map(String::from).into();
        let stop = json!({ "hook_event_name": "Stop", "background_tasks": [
            { "id": "a", "type": "subagent", "status": "running" },
            { "id": "b", "type": "subagent", "status": "completed" },
            { "id": "s", "type": "shell", "status": "running" },
            { "id": "d", "type": "subagent", "status": "running" },
        ] });
        assert!(fold_subagents(&mut set, &stop));
        assert_eq!(set.iter().map(String::as_str).collect::<Vec<_>>(), ["a", "d"]);
        // Without the list (an older Claude Code), the main Stop clears the set.
        assert!(fold_subagents(&mut set, &json!({ "hook_event_name": "Stop" })));
        assert!(set.is_empty());
    }

    #[test]
    fn subagents_count_running_ids() {
        use std::collections::BTreeSet;
        let mut set = BTreeSet::new();
        let ev = |name: &str, id: Option<Value>| match id {
            Some(id) => json!({ "hook_event_name": name, "agent_id": id }),
            None => json!({ "hook_event_name": name }),
        };
        let ids = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>();
        assert!(fold_subagents(&mut set, &ev("SubagentStart", Some(json!("a")))));
        assert_eq!(ids(&set), ["a"]);
        assert!(fold_subagents(&mut set, &ev("SubagentStart", Some(json!("b")))));
        assert_eq!(ids(&set), ["a", "b"]);
        assert!(!fold_subagents(&mut set, &ev("SubagentStart", Some(json!("a")))));
        assert!(fold_subagents(&mut set, &ev("SubagentStop", Some(json!("a")))));
        assert_eq!(ids(&set), ["b"]);
        assert!(!fold_subagents(&mut set, &ev("SubagentStop", Some(json!("zzz")))));
        assert!(!fold_subagents(&mut set, &ev("PreToolUse", Some(json!("q")))));
        // No agent_id, or one that is not a string: nothing changes.
        for bad in [None, Some(json!(7)), Some(json!(null))] {
            assert!(!fold_subagents(&mut set, &ev("SubagentStart", bad.clone())));
            assert!(!fold_subagents(&mut set, &ev("SubagentStop", bad)));
        }
        assert_eq!(ids(&set), ["b"]);
        assert!(fold_subagents(&mut set, &ev("Stop", None)));
        assert!(set.is_empty());
        assert!(!fold_subagents(&mut set, &ev("Stop", None)));
        fold_subagents(&mut set, &ev("SubagentStart", Some(json!("c"))));
        assert!(fold_subagents(&mut set, &ev("SessionEnd", None)));
        assert!(set.is_empty());
    }
}
