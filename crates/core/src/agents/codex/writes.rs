//! Which files a Codex event says its `apply_patch` tool wrote.

use crate::agents::common::str_at;
use serde_json::Value;

/// The files this event says the CLI's file tool wrote, as the CLI gave them. Empty for any other event. A patch names
/// each file on a `*** Add File:`, `*** Update File:` or `*** Move to:` line; a `*** Delete File:` line names nothing.
pub fn written(body: &Value) -> Vec<String> {
    if str_at(body, "/hook_event_name") != Some("PostToolUse") || str_at(body, "/tool_name") != Some("apply_patch") {
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
