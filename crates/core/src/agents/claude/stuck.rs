//! Claude Code's hook events as the neutral signals the stuck tracker reads.

use crate::stuck::StuckSignal;
use serde_json::Value;

/// What one hook event means to the stuck tracker. A tool call's key sorts the JSON keys, so the
/// same call is the same key however the input happens to be ordered.
pub fn stuck_signal(body: &Value) -> StuckSignal {
    match body.get("hook_event_name").and_then(Value::as_str) {
        Some("UserPromptSubmit") => StuckSignal::Prompt,
        Some("PreToolUse") => {
            let mut key = serde_json::json!([body.get("tool_name"), body.get("tool_input")]);
            key.sort_all_objects();
            StuckSignal::Call(key.to_string())
        }
        _ => StuckSignal::Progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn key_of(body: &Value) -> String {
        match stuck_signal(body) {
            StuckSignal::Call(key) => key,
            other => panic!("expected a call, got {other:?}"),
        }
    }

    #[test]
    fn pre_tool_use_is_a_call() {
        let body = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "pwd"}});
        assert!(matches!(stuck_signal(&body), StuckSignal::Call(_)));
        let other = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "ls"}});
        assert_ne!(key_of(&body), key_of(&other));
        let named = json!({"hook_event_name": "PreToolUse", "tool_name": "Other", "tool_input": {"command": "pwd"}});
        assert_ne!(key_of(&body), key_of(&named));
    }

    #[test]
    fn a_prompt_is_a_prompt() {
        assert_eq!(
            stuck_signal(&json!({"hook_event_name": "UserPromptSubmit"})),
            StuckSignal::Prompt
        );
    }

    #[test]
    fn any_other_event_is_progress() {
        for event in ["PostToolUse", "Notification", "Stop", "SubagentStop", "Unknown"] {
            assert_eq!(stuck_signal(&json!({"hook_event_name": event})), StuckSignal::Progress);
        }
        assert_eq!(stuck_signal(&json!({"no": "name"})), StuckSignal::Progress);
    }

    #[test]
    fn key_ignores_json_key_order() {
        let a: Value = serde_json::from_str(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Tool","tool_input":{"b":[{"y":2,"x":1}],"a":0}}"#,
        )
        .unwrap();
        let b: Value = serde_json::from_str(
            r#"{"tool_input":{"a":0,"b":[{"x":1,"y":2}]},"tool_name":"Tool","hook_event_name":"PreToolUse"}"#,
        )
        .unwrap();
        assert_eq!(key_of(&a), key_of(&b));
    }
}
