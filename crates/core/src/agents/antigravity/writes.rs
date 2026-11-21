//! Which file an Antigravity event says its file tool wrote.

use crate::agents::common::str_at;
use serde_json::Value;

/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event, and for a call
/// whose `error` is a non-empty string.
pub fn written(body: &Value) -> Vec<String> {
    if str_at(body, "/hook_event_name") != Some("PostToolUse") {
        return Vec::new();
    }
    if body
        .pointer("/error")
        .and_then(Value::as_str)
        .is_some_and(|error| !error.is_empty())
    {
        return Vec::new();
    }
    match str_at(body, "/toolCall/name") {
        Some("write_file" | "edit_file") => str_at(body, "/toolCall/args/Path")
            .map(|path| vec![path.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// agy's recorded bodies have no PostToolUse for a write, so this is the recorded PreToolUse with the event's
    /// name changed and the `error` field a PostToolUse carries (`""` when the call worked).
    fn post(tool: &str, error: Option<&str>) -> Value {
        let mut body: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/antigravity/hooks/1.4.7/PreToolUse-write_file.json"
        ))
        .unwrap();
        body["hook_event_name"] = json!("PostToolUse");
        body["toolCall"]["name"] = json!(tool);
        if let Some(error) = error {
            body["error"] = json!(error);
        }
        body
    }

    const PATH: &str = "C:\\Users\\user\\AppData\\Local\\Temp\\claude\\C--dev-Airport\\00000000-0000-4000-8000-000000000001\\scratchpad\\lc-6d-1\\proj6d\\notes.txt";

    #[test]
    fn write_file_and_edit_file_report_their_path() {
        for tool in ["write_file", "edit_file"] {
            assert_eq!(written(&post(tool, Some(""))), [PATH], "{tool}");
        }
        assert!(written(&post("run_command", Some(""))).is_empty());
        assert!(written(&post("read_file", Some(""))).is_empty());
    }

    #[test]
    fn a_failed_write_reports_nothing() {
        assert!(written(&post("write_file", Some("permission denied"))).is_empty());
        assert_eq!(written(&post("write_file", Some(""))), [PATH]);
        assert_eq!(written(&post("write_file", None)), [PATH]);
    }

    #[test]
    fn a_pre_tool_event_reports_nothing() {
        let mut body = post("write_file", Some(""));
        body["hook_event_name"] = json!("PreToolUse");
        assert!(written(&body).is_empty());
    }
}
