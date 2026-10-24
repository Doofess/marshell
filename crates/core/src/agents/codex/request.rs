//! Maps Codex's PermissionRequest payload to the agent-neutral approve card, and a decision to Codex's answer.

use super::status::{first_patch_file, patch_files};
use crate::agents::common::{DENIED_TEXT, flatten_input};
use crate::hooks::{holds::PendingPermission, risk};
use marshell_protocol::hook::{
    DiffKind, DiffLine, PermissionBehavior, PermissionDecision, PermissionDetail, PermissionScope, PermissionView,
};
use serde_json::{Value, json};
use std::path::Path;

/// Describes the request. `Bash` is a command, `apply_patch` an edit, and anything else (an MCP tool
/// included: no MCP name format was recorded) is shown by name with its raw input.
pub fn detail(request: &Value) -> PermissionDetail {
    let tool = request["tool_name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown tool");
    let input = &request["tool_input"];
    known_detail(tool, input).unwrap_or_else(|| PermissionDetail::Other {
        tool: tool.into(),
        args: flatten_input(input),
    })
}

fn known_detail(tool: &str, input: &Value) -> Option<PermissionDetail> {
    let command = input["command"].as_str()?;
    Some(match tool {
        // Codex's own reason (`tool_input.description`) has no place on the card yet.
        "Bash" => PermissionDetail::Bash {
            command: command.into(),
            cwd: None,
        },
        "apply_patch" => patch_detail(command)?,
        _ => return None,
    })
}

/// An edit of the patch's first file. The hunk is the whole patch body, so a second file's header
/// and lines are on the card too.
fn patch_detail(patch: &str) -> Option<PermissionDetail> {
    let path = first_patch_file(patch)?.to_owned();
    let (mut add, mut del) = (0_u32, 0_u32);
    let hunk = patch
        .lines()
        .filter(|line| !matches!(*line, "*** Begin Patch" | "*** End Patch"))
        .map(|line| {
            let (kind, text) = match line.as_bytes().first() {
                Some(b'+') => (DiffKind::Add, &line[1..]),
                Some(b'-') => (DiffKind::Del, &line[1..]),
                _ => (DiffKind::Context, line),
            };
            match kind {
                DiffKind::Add => add = add.saturating_add(1),
                DiffKind::Del => del = del.saturating_add(1),
                DiffKind::Context => {}
            }
            DiffLine {
                kind,
                text: text.into(),
            }
        })
        .collect();
    Some(PermissionDetail::Edit { path, add, del, hunk })
}

/// The card's edit names a patch's first file, but the patch may write others, or move a file
/// elsewhere: each is checked as an edit of its own.
fn later_file_risk(request: &Value, cwd: &Path) -> Option<String> {
    if request["tool_name"].as_str() != Some("apply_patch") {
        return None;
    }
    patch_files(request["tool_input"]["command"].as_str()?).find_map(|path| {
        let edit = PermissionDetail::Edit {
            path: path.into(),
            add: 0,
            del: 0,
            hunk: Vec::new(),
        };
        risk::classify(&edit, cwd)
    })
}

/// Builds a card without changing the held request or answering it. Codex keeps no rule from Marshell,
/// so there is no always rule.
pub fn view(p: &PendingPermission) -> Option<PermissionView> {
    let tab = p.tab.clone()?;
    let detail = detail(&p.request);
    let cwd = Path::new(p.request["cwd"].as_str().unwrap_or(""));
    Some(PermissionView {
        rid: p.rid.clone(),
        tab,
        agent: p.agent.clone(),
        at_ms: p.at_ms,
        risk: risk::classify(&detail, cwd).or_else(|| later_file_risk(&p.request, cwd)),
        detail,
        always_rule: None,
    })
}

/// The hook output that carries `decision` back to Codex, or why it can't. Codex keeps no rule from
/// Marshell, so the project scope is refused; an allow for the session is an allow once.
pub fn permission_output(decision: &PermissionDecision, _request: &Value) -> Result<Value, &'static str> {
    let decision = match decision.behavior {
        PermissionBehavior::Deny => {
            let message = decision
                .message
                .as_deref()
                .filter(|m| !m.is_empty())
                .unwrap_or(DENIED_TEXT);
            json!({ "behavior": "deny", "message": message })
        }
        PermissionBehavior::Allow => {
            if decision.scope == PermissionScope::Project {
                return Err(
                    "Codex can't keep an always-allow rule from Marshell. Allow once, or answer in the terminal.",
                );
            }
            json!({ "behavior": "allow" })
        }
    };
    Ok(json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/codex/hooks/0.163.0/PermissionRequest-shell.json"
        ))
        .unwrap()
    }

    fn patch() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/codex/hooks/0.163.0/PermissionRequest-apply_patch.json"
        ))
        .unwrap()
    }

    fn decision(behavior: PermissionBehavior, scope: PermissionScope, message: Option<&str>) -> PermissionDecision {
        PermissionDecision {
            behavior,
            scope,
            message: message.map(str::to_owned),
        }
    }

    #[test]
    fn a_shell_request_is_a_bash_detail() {
        assert_eq!(
            detail(&shell()),
            PermissionDetail::Bash {
                command: "Set-Content out.txt hi".into(),
                cwd: None
            }
        );
    }

    #[test]
    fn an_apply_patch_request_is_an_edit_of_its_file() {
        let PermissionDetail::Edit { path, add, del, hunk } = detail(&patch()) else {
            panic!("expected Edit")
        };
        assert_eq!(path, "notes.md");
        assert_eq!((add, del), (1, 0));
        let lines: Vec<_> = hunk.iter().map(|l| (l.kind, l.text.as_str())).collect();
        assert_eq!(
            lines,
            vec![(DiffKind::Context, "*** Add File: notes.md"), (DiffKind::Add, "hello")]
        );
    }

    #[test]
    fn a_patch_of_two_files_shows_both() {
        let request = json!({"tool_name": "apply_patch", "tool_input": {"command":
            "*** Begin Patch\n*** Update File: a.rs\n@@\n-old\n+new\n*** Delete File: .git/config\n*** End Patch"}});
        let PermissionDetail::Edit { path, add, del, hunk } = detail(&request) else {
            panic!("expected Edit")
        };
        assert_eq!((path.as_str(), add, del), ("a.rs", 1, 1));
        assert!(hunk.iter().any(|l| l.text == "*** Delete File: .git/config"));
    }

    #[test]
    fn a_patch_without_a_file_line_is_other_with_its_input() {
        let request = json!({"tool_name": "apply_patch", "tool_input": {"command": "garbage"}});
        assert_eq!(
            detail(&request),
            PermissionDetail::Other {
                tool: "apply_patch".into(),
                args: vec![("command".into(), "garbage".into())]
            }
        );
    }

    #[test]
    fn an_unknown_tool_is_other_with_its_input() {
        let request = json!({"tool_name": "mcp__files__read", "tool_input": {"path": "a.txt", "depth": 2}});
        assert_eq!(
            detail(&request),
            PermissionDetail::Other {
                tool: "mcp__files__read".into(),
                args: vec![("path".into(), "a.txt".into()), ("depth".into(), "2".into())]
            }
        );
        let nameless = json!({"tool_input": "raw"});
        assert_eq!(
            detail(&nameless),
            PermissionDetail::Other {
                tool: "Unknown tool".into(),
                args: vec![("input".into(), "raw".into())]
            }
        );
    }

    #[test]
    fn the_card_has_no_always_rule() {
        let p = PendingPermission {
            rid: "r".into(),
            tab: Some("t1".into()),
            agent: "codex".into(),
            tool: Some("Bash".into()),
            at_ms: 7,
            request: shell(),
        };
        let v = view(&p).unwrap();
        assert_eq!((v.tab.as_str(), v.agent.as_str(), v.at_ms), ("t1", "codex", 7));
        assert_eq!(v.always_rule, None);
        assert!(matches!(v.detail, PermissionDetail::Bash { .. }));
        assert!(view(&PendingPermission { tab: None, ..p }).is_none());
    }

    #[test]
    fn a_risky_later_file_in_a_patch_is_flagged() {
        let p = |patch: &str| PendingPermission {
            rid: "r".into(),
            tab: Some("t1".into()),
            agent: "codex".into(),
            tool: Some("apply_patch".into()),
            at_ms: 7,
            request: json!({ "tool_name": "apply_patch", "cwd": "C:/p", "tool_input": { "command": patch } }),
        };
        let added =
            "*** Begin Patch\n*** Update File: src/a.rs\n+x\n*** Add File: .git/hooks/pre-commit\n+y\n*** End Patch";
        assert_eq!(view(&p(added)).unwrap().risk.as_deref(), Some("Writes to .git"));
        let moved = "*** Begin Patch\n*** Update File: src/a.rs\n*** Move to: .git/config\n+x\n*** End Patch";
        assert_eq!(view(&p(moved)).unwrap().risk.as_deref(), Some("Writes to .git"));
        let plain = "*** Begin Patch\n*** Update File: src/a.rs\n+x\n*** End Patch";
        assert_eq!(view(&p(plain)).unwrap().risk, None);
    }

    #[test]
    fn allow_is_exactly_codexs_shape() {
        let out = permission_output(
            &decision(PermissionBehavior::Allow, PermissionScope::Once, None),
            &shell(),
        )
        .unwrap();
        assert_eq!(
            out.to_string(),
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
        );
    }

    #[test]
    fn deny_carries_the_message() {
        let out = permission_output(
            &decision(PermissionBehavior::Deny, PermissionScope::Once, Some("Not now")),
            &shell(),
        )
        .unwrap();
        assert_eq!(
            out.to_string(),
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Not now"}}}"#
        );
        for message in [None, Some("")] {
            let out = permission_output(
                &decision(PermissionBehavior::Deny, PermissionScope::Once, message),
                &shell(),
            )
            .unwrap();
            assert_eq!(out["hookSpecificOutput"]["decision"]["message"], "Denied in Marshell.");
        }
    }

    #[test]
    fn always_allow_is_refused() {
        assert_eq!(
            permission_output(
                &decision(PermissionBehavior::Allow, PermissionScope::Project, None),
                &shell()
            ),
            Err("Codex can't keep an always-allow rule from Marshell. Allow once, or answer in the terminal.")
        );
    }
}
