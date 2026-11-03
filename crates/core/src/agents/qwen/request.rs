//! Maps Qwen Code's `PermissionRequest` body to the agent-neutral approve card, and a decision to Qwen's answer.
//!
//! The request carries `tool_name` and `tool_input` and no tool id (the 6l1 research note, Q3), so nothing here keys
//! on one.

use crate::agents::common::{DENIED_TEXT, edit_detail, flatten_input};
use crate::hooks::{holds::PendingPermission, risk};
use marshell_protocol::hook::{
    PermissionBehavior, PermissionDecision, PermissionDetail, PermissionScope, PermissionView,
};
use serde_json::{Value, json};
use std::path::Path;

/// Describes the request from its `tool_name` and `tool_input`. A tool Marshell doesn't know is shown by name with
/// its arguments as `key: value` lines.
pub fn detail(request: &Value) -> PermissionDetail {
    let tool = request["tool_name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown tool");
    let input = &request["tool_input"];
    known_detail(tool, input, request).unwrap_or_else(|| PermissionDetail::Other {
        tool: tool.into(),
        args: flatten_input(input),
    })
}

fn known_detail(tool: &str, input: &Value, request: &Value) -> Option<PermissionDetail> {
    Some(match tool {
        "run_shell_command" => PermissionDetail::Bash {
            command: input["command"].as_str()?.into(),
            // `directory` is the folder the command runs in, when it is not the tab's own.
            cwd: input["directory"]
                .as_str()
                .filter(|dir| !dir.is_empty())
                .filter(|dir| {
                    request["cwd"].as_str().is_none_or(|base| {
                        !risk::paths_equal(&risk::resolve_path(Path::new(dir), Path::new(base)), Path::new(base))
                    })
                })
                .map(str::to_owned),
        },
        "edit" | "replace" => edit_detail(
            input["file_path"].as_str()?,
            input["old_string"].as_str()?,
            input["new_string"].as_str()?,
        ),
        "write_file" => {
            let path = input["file_path"].as_str()?;
            let content = input["content"].as_str()?;
            let cwd = Path::new(request["cwd"].as_str().unwrap_or(""));
            PermissionDetail::Write {
                path: path.into(),
                preview: content.lines().take(4).collect::<Vec<_>>().join("\n"),
                // Never stat a remote path: that connects to a host the repository may have chosen.
                is_new: !risk::is_remote_path(path) && !risk::resolve_path(Path::new(path), cwd).exists(),
            }
        }
        _ => return None,
    })
}

/// Builds a card without changing the held request or answering it. Qwen keeps no rule from Marshell, so there is
/// no always rule.
pub fn view(p: &PendingPermission) -> Option<PermissionView> {
    let tab = p.tab.clone()?;
    let detail = detail(&p.request);
    let cwd = Path::new(p.request["cwd"].as_str().unwrap_or(""));
    Some(PermissionView {
        rid: p.rid.clone(),
        tab,
        agent: p.agent.clone(),
        at_ms: p.at_ms,
        risk: risk::classify(&detail, cwd),
        detail,
        always_rule: None,
        deadline_ms: None,
    })
}

/// The hook output that carries `decision` back to Qwen, or why it can't. An allow runs the call with no dialog; a
/// deny carries the person's words to the model. Qwen keeps no rule from Marshell and its bridge refuses an allow
/// that carries `updatedPermissions`, so "Always allow" (and an allow for the session) is refused here.
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
        PermissionBehavior::Allow if decision.scope != PermissionScope::Once => {
            return Err("Qwen Code can't keep an always-allow rule from Marshell. Allow this once instead.");
        }
        PermissionBehavior::Allow => json!({ "behavior": "allow" }),
    };
    Ok(json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::hook::DiffKind;

    macro_rules! hook {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/qwen/hooks/0.26.5/",
                $name,
                ".json"
            )))
            .unwrap()
        };
    }

    fn decision(behavior: PermissionBehavior, scope: PermissionScope, message: Option<&str>) -> PermissionDecision {
        PermissionDecision {
            behavior,
            scope,
            message: message.map(str::to_owned),
        }
    }

    fn pending(request: Value) -> PendingPermission {
        PendingPermission {
            rid: "r1".into(),
            tab: Some("t1".into()),
            agent: "qwen".into(),
            tool: request["tool_name"].as_str().map(str::to_owned),
            at_ms: 123,
            request,
        }
    }

    #[test]
    fn a_shell_request_shows_its_command() {
        let request = json!({
            "cwd": if cfg!(windows) { "C:/project" } else { "/project" },
            "tool_name": "run_shell_command",
            "tool_input": {"command": "npm test", "is_background": false, "description": "Run the tests"},
        });
        assert_eq!(
            detail(&request),
            PermissionDetail::Bash {
                command: "npm test".into(),
                cwd: None
            }
        );
        let mut elsewhere = request.clone();
        elsewhere["tool_input"]["directory"] = json!("../other");
        assert!(matches!(detail(&elsewhere), PermissionDetail::Bash { cwd: Some(ref d), .. } if d == "../other"));
        let v = view(&pending(request)).unwrap();
        assert_eq!((v.agent.as_str(), v.always_rule, v.risk), ("qwen", None, None));
    }

    #[test]
    fn a_write_shows_the_file() {
        let request = hook!("PermissionRequest-write_file");
        let PermissionDetail::Write { path, preview, is_new } = detail(&request) else {
            panic!("expected Write")
        };
        assert!(path.ends_with("proj6/b.txt"), "{path}");
        assert_eq!(preview, "marshell");
        // The recorded folder is not on this machine, so the file is not there either.
        assert!(is_new);
    }

    #[test]
    fn an_edit_counts_the_hunk() {
        let request = json!({
            "cwd": "C:/project",
            "tool_name": "edit",
            "tool_input": {"file_path": "a.txt", "old_string": "same\nold\n", "new_string": "same\nnew\n"},
        });
        let PermissionDetail::Edit { path, add, del, hunk } = detail(&request) else {
            panic!("expected Edit")
        };
        assert_eq!((path.as_str(), add, del), ("a.txt", 1, 1));
        assert_eq!(hunk[1].kind, DiffKind::Del);
    }

    #[test]
    fn an_unknown_tool_shows_raw_arguments() {
        let request = json!({"cwd": "C:/project", "tool_name": "web_fetch", "tool_input": {"url": "https://example.test", "n": 2}});
        assert_eq!(
            detail(&request),
            PermissionDetail::Other {
                tool: "web_fetch".into(),
                args: vec![("url".into(), "https://example.test".into()), ("n".into(), "2".into())]
            }
        );
        // A known tool whose input is not what Marshell expects is shown raw too, and nothing panics.
        for input in [Value::Null, json!(42), json!({"command": 3})] {
            let request = json!({"tool_name": "run_shell_command", "tool_input": input});
            assert!(
                matches!(detail(&request), PermissionDetail::Other { ref tool, .. } if tool == "run_shell_command")
            );
        }
        assert!(matches!(detail(&json!({})), PermissionDetail::Other { tool, .. } if !tool.is_empty()));
    }

    #[test]
    fn allow_and_deny_answer_in_qwens_shape() {
        let allow = permission_output(
            &decision(PermissionBehavior::Allow, PermissionScope::Once, None),
            &json!({}),
        );
        assert_eq!(
            allow,
            Ok(
                json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})
            )
        );
        let deny = |message| {
            permission_output(
                &decision(PermissionBehavior::Deny, PermissionScope::Once, message),
                &json!({}),
            )
        };
        assert_eq!(
            deny(None),
            Ok(json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest",
                "decision": {"behavior": "deny", "message": "Denied in Marshell."}}}))
        );
        assert_eq!(
            deny(Some("Not now")).unwrap()["hookSpecificOutput"]["decision"]["message"],
            "Not now"
        );
        // A deny never asks for a rule, whatever its scope.
        let scoped = permission_output(
            &decision(PermissionBehavior::Deny, PermissionScope::Project, None),
            &json!({}),
        );
        assert!(scoped.is_ok());
    }

    #[test]
    fn always_allow_is_refused_with_the_copy() {
        for scope in [PermissionScope::Project, PermissionScope::Session] {
            let request = json!({"permission_suggestions": [{"type": "addRules", "behavior": "allow",
                "rules": [{"toolName": "run_shell_command"}]}]});
            assert_eq!(
                permission_output(&decision(PermissionBehavior::Allow, scope, None), &request),
                Err("Qwen Code can't keep an always-allow rule from Marshell. Allow this once instead.")
            );
        }
    }

    #[test]
    fn a_card_needs_a_tab_and_the_dispatch_finds_it() {
        let mut p = pending(hook!("PermissionRequest-write_file"));
        let v = crate::agents::permission_view(&p).unwrap();
        assert_eq!(
            (v.rid.as_str(), v.tab.as_str(), v.agent.as_str(), v.at_ms),
            ("r1", "t1", "qwen", 123)
        );
        p.tab = None;
        assert_eq!(view(&p), None);
        assert_eq!(crate::agents::permission_view(&p), None);
    }
}
