//! Which files a Codex event says its `apply_patch` tool wrote.

use crate::agents::common::str_at;
use serde_json::Value;

/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event. A patch names
/// each file on a `*** Add File:`, `*** Update File:` or `*** Move to:` line; a `*** Delete File:` line names nothing.
/// Empty too when `tool_response` is a string whose first line is `Exit code: <n>` with n other than 0; any other shape
/// of it (absent, not a string, no such first line) counts as a success, since what a failed patch sends isn't verified.
pub fn written(body: &Value) -> Vec<String> {
    if str_at(body, "/hook_event_name") != Some("PostToolUse") || str_at(body, "/tool_name") != Some("apply_patch") {
        return Vec::new();
    }
    if str_at(body, "/tool_response")
        .and_then(|text| text.lines().next())
        .and_then(|first| first.strip_prefix("Exit code:"))
        .and_then(|code| code.trim().parse::<i64>().ok())
        .is_some_and(|code| code != 0)
    {
        return Vec::new();
    }
    let Some(patch) = str_at(body, "/tool_input/command") else {
        return Vec::new();
    };
    patch
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            let rest = ["*** Add File:", "*** Update File:", "*** Move to:"]
                .iter()
                .find_map(|marker| line.strip_prefix(marker))?
                .trim();
            (!rest.is_empty()).then(|| rest.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture(name: &str) -> Value {
        let text = match name {
            "PostToolUse-apply_patch.json" => {
                include_str!("../../../tests/fixtures/codex/hooks/0.163.0/PostToolUse-apply_patch.json")
            }
            "PostToolUse.json" => include_str!("../../../tests/fixtures/codex/hooks/0.163.0/PostToolUse.json"),
            "PreToolUse-apply_patch.json" => {
                include_str!("../../../tests/fixtures/codex/hooks/0.163.0/PreToolUse-apply_patch.json")
            }
            other => panic!("no fixture {other}"),
        };
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_patch_reports_the_files_it_adds_updates_and_moves_to() {
        let body = json!({"hook_event_name": "PostToolUse", "tool_name": "apply_patch", "tool_input": {"command": "*** Begin Patch\n*** Add File: out/report.html\n+<p>hi</p>\n*** Update File: index.html\n*** Move to: site/index.html\n*** Delete File: old.html\n*** End Patch"}});
        assert_eq!(written(&body), ["out/report.html", "index.html", "site/index.html"]);
    }

    #[test]
    fn the_recorded_patch_reports_its_file() {
        assert_eq!(written(&fixture("PostToolUse-apply_patch.json")), ["notes.md"]);
    }

    #[test]
    fn a_patch_whose_response_starts_with_a_failed_exit_code_reports_nothing() {
        let mut body = fixture("PostToolUse-apply_patch.json");
        let response = body["tool_response"]
            .as_str()
            .unwrap()
            .replacen("Exit code: 0", "Exit code: 1", 1);
        assert!(response.starts_with("Exit code: 1\n"));
        body["tool_response"] = Value::String(response);
        assert!(
            written(&body).is_empty(),
            "a failed exit code still reported the patch's files"
        );
    }

    #[test]
    fn a_patch_with_no_response_or_no_exit_code_line_still_reports_its_files() {
        let mut body = fixture("PostToolUse-apply_patch.json");
        body["tool_response"] = json!({"output": "Exit code: 1"});
        assert_eq!(written(&body), ["notes.md"]);
        body["tool_response"] = json!("patch failed");
        assert_eq!(written(&body), ["notes.md"]);
        body.as_object_mut().unwrap().remove("tool_response");
        assert_eq!(written(&body), ["notes.md"]);
    }

    #[test]
    fn a_shell_call_and_a_pre_tool_event_report_nothing() {
        assert!(written(&fixture("PostToolUse.json")).is_empty());
        assert!(written(&fixture("PreToolUse-apply_patch.json")).is_empty());
    }

    #[test]
    fn a_carriage_return_and_an_empty_name_are_not_part_of_a_path() {
        let body = json!({"hook_event_name": "PostToolUse", "tool_name": "apply_patch", "tool_input": {"command": "*** Add File: a.html\r\n*** Add File: \n*** Update File:   b.html  \n"}});
        assert_eq!(written(&body), ["a.html", "b.html"]);
    }
}
