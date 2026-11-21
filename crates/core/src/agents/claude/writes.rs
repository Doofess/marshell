//! Which file a Claude Code event says its file tool wrote.

use crate::agents::common::str_at;
use serde_json::Value;

/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event, and for a call
/// the event says failed (Claude Code sends `PostToolUseFailure` for one).
pub fn written(body: &Value) -> Vec<String> {
    if str_at(body, "/hook_event_name") != Some("PostToolUse") {
        return Vec::new();
    }
    match str_at(body, "/tool_name") {
        Some("Write" | "Edit" | "MultiEdit") => str_at(body, "/tool_input/file_path")
            .map(|path| vec![path.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(event: &str, tool: &str) -> serde_json::Value {
        json!({"hook_event_name": event, "tool_name": tool, "tool_input": {"file_path": "C:\\proj\\a.html"}})
    }

    #[test]
    fn write_edit_and_multiedit_report_their_file() {
        for tool in ["Write", "Edit", "MultiEdit"] {
            assert_eq!(written(&body("PostToolUse", tool)), ["C:\\proj\\a.html"], "{tool}");
        }
        for tool in ["Bash", "Read"] {
            assert!(written(&body("PostToolUse", tool)).is_empty(), "{tool}");
        }
        assert!(written(&body("PreToolUse", "Write")).is_empty());
        assert!(written(&body("PostToolUseFailure", "Write")).is_empty());
        assert!(written(&json!({"hook_event_name": "PostToolUse", "tool_name": "Write"})).is_empty());
    }
}
