//! Which file a Qwen Code event says its file tool wrote.

use crate::agents::common::str_at;
use serde_json::Value;

/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event.
pub fn written(body: &Value) -> Vec<String> {
    if str_at(body, "/hook_event_name") != Some("PostToolUse") {
        return Vec::new();
    }
    match str_at(body, "/tool_name") {
        Some("write_file" | "edit" | "replace") => str_at(body, "/tool_input/file_path")
            .map(|path| vec![path.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture(name: &str) -> Value {
        let text = match name {
            "PostToolUse-write_file.json" => {
                include_str!("../../../tests/fixtures/qwen/hooks/0.26.5/PostToolUse-write_file.json")
            }
            "PreToolUse-write_file.json" => {
                include_str!("../../../tests/fixtures/qwen/hooks/0.26.5/PreToolUse-write_file.json")
            }
            "PostToolUse.json" => include_str!("../../../tests/fixtures/qwen/hooks/0.26.5/PostToolUse.json"),
            other => panic!("no fixture {other}"),
        };
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn the_recorded_write_reports_its_file() {
        assert_eq!(
            written(&fixture("PostToolUse-write_file.json")),
            ["C:/Users/user/AppData/Local/Temp/lc-6l1-2/proj6/b.txt"]
        );
    }

    #[test]
    fn edit_and_replace_report_their_file_and_other_calls_and_events_do_not() {
        for tool in ["write_file", "edit", "replace"] {
            let body =
                json!({"hook_event_name": "PostToolUse", "tool_name": tool, "tool_input": {"file_path": "/p/a.html"}});
            assert_eq!(written(&body), ["/p/a.html"], "{tool}");
        }
        let shell =
            json!({"hook_event_name": "PostToolUse", "tool_name": "run_shell_command", "tool_input": {"command": "x"}});
        assert!(written(&shell).is_empty());
        assert!(written(&fixture("PostToolUse.json")).is_empty());
        assert!(written(&fixture("PreToolUse-write_file.json")).is_empty());
    }
}
