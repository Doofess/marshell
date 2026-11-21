//! Which file a Copilot CLI event says its file tool wrote.

use crate::agents::common::str_at;
use serde_json::Value;

/// A tool's arguments: `toolArgs` is an object in 1.0.106, and a JSON string is parsed first (as `status.rs` reads them).
fn tool_args(body: &Value) -> Option<Value> {
    match body.get("toolArgs")? {
        Value::String(text) => serde_json::from_str(text).ok(),
        other => Some(other.clone()),
    }
}

/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event, for a result
/// that isn't `success`, and for `postToolUseFailure`.
pub fn written(body: &Value) -> Vec<String> {
    if str_at(body, "/hook_event_name") != Some("postToolUse")
        || str_at(body, "/toolResult/resultType") != Some("success")
    {
        return Vec::new();
    }
    if !matches!(str_at(body, "/toolName"), Some("create" | "edit")) {
        return Vec::new();
    }
    tool_args(body)
        .and_then(|args| str_at(&args, "/path").map(str::to_string))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn body(event: &str, tool: &str, args: Value, result: &str) -> Value {
        json!({"hook_event_name": event, "toolName": tool, "toolArgs": args, "toolResult": {"resultType": result}})
    }

    #[test]
    fn create_and_edit_report_their_path_and_a_failure_does_not() {
        let args = json!({"path": "C:\\proj\\a.html", "file_text": "x"});
        for tool in ["create", "edit"] {
            assert_eq!(
                written(&body("postToolUse", tool, args.clone(), "success")),
                ["C:\\proj\\a.html"],
                "{tool}"
            );
            // The string form status.rs accepts.
            let text = json!(args.to_string());
            assert_eq!(
                written(&body("postToolUse", tool, text, "success")),
                ["C:\\proj\\a.html"],
                "{tool}"
            );
        }
        assert!(written(&body("postToolUse", "create", args.clone(), "failure")).is_empty());
        assert!(written(&body("postToolUse", "create", args.clone(), "denied")).is_empty());
        assert!(written(&body("postToolUseFailure", "create", args.clone(), "success")).is_empty());
        assert!(written(&body("preToolUse", "create", args.clone(), "success")).is_empty());
        assert!(written(&body("postToolUse", "view", args.clone(), "success")).is_empty());
        assert!(written(&body("postToolUse", "powershell", json!({"command": "x"}), "success")).is_empty());
        assert!(written(&body("postToolUse", "create", json!("not json {"), "success")).is_empty());
        let mut no_result = body("postToolUse", "create", args, "success");
        no_result.as_object_mut().unwrap().remove("toolResult");
        assert!(written(&no_result).is_empty());
    }

    #[test]
    fn the_recorded_failure_reports_nothing() {
        let mut failure: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/copilot/hooks/1.0.106/postToolUseFailure.json"
        ))
        .unwrap();
        failure["hook_event_name"] = json!("postToolUseFailure");
        assert!(written(&failure).is_empty());
    }
}
