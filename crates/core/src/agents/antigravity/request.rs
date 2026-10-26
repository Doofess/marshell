//! Maps Antigravity's `PreToolUse` body to the agent-neutral approve card, and a decision to agy's answer.

use crate::agents::common::{DENIED_TEXT, flatten_input};
use crate::hooks::{holds::PendingPermission, risk};
use marshell_protocol::hook::{
    DiffKind, DiffLine, PermissionBehavior, PermissionDecision, PermissionDetail, PermissionScope, PermissionView,
};
use serde_json::{Value, json};
use std::path::Path;

/// Describes the request from its `toolCall { name, args }` (the 1.4.7 names and keys). A tool Marshell
/// doesn't know, `read_file` included, is shown by name with its arguments as `key: value` lines.
pub fn detail(request: &Value) -> PermissionDetail {
    let call = &request["toolCall"];
    let tool = call["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown tool");
    let args = &call["args"];
    known_detail(tool, args, request).unwrap_or_else(|| PermissionDetail::Other {
        tool: tool.into(),
        args: flatten_input(args),
    })
}

fn known_detail(tool: &str, args: &Value, request: &Value) -> Option<PermissionDetail> {
    Some(match tool {
        "run_command" => PermissionDetail::Bash {
            command: args["CommandLine"].as_str()?.into(),
            // agy's 1.4.7 `run_command` carried no folder in the recorded bodies; a `Cwd` argument, when
            // one comes, is named only if it is not the tab's folder.
            cwd: args["Cwd"]
                .as_str()
                .filter(|cwd| {
                    request["cwd"].as_str().is_none_or(|base| {
                        !risk::paths_equal(&risk::resolve_path(Path::new(cwd), Path::new(base)), Path::new(base))
                    })
                })
                .map(str::to_owned),
        },
        "edit_file" => {
            let path = args["Path"].as_str()?;
            let (target, replacement) = (args["Target"].as_str()?, args["Replacement"].as_str()?);
            let line = |kind, text: &str| DiffLine {
                kind,
                text: text.strip_suffix('\r').unwrap_or(text).into(),
            };
            let removed: Vec<_> = target.lines().map(|text| line(DiffKind::Del, text)).collect();
            let added: Vec<_> = replacement.lines().map(|text| line(DiffKind::Add, text)).collect();
            let (del, add) = (removed.len() as u32, added.len() as u32);
            PermissionDetail::Edit {
                path: path.into(),
                add,
                del,
                hunk: removed.into_iter().chain(added).collect(),
            }
        }
        "write_file" => {
            let path = args["Path"].as_str()?;
            let content = args["Content"].as_str()?;
            let cwd = Path::new(request["cwd"].as_str().unwrap_or(""));
            PermissionDetail::Write {
                path: path.into(),
                preview: content.lines().take(4).collect::<Vec<_>>().join("\n"),
                // Never stat a remote path: that connects to a host the repository may have chosen.
                is_new: !risk::is_remote_path(path) && !risk::resolve_path(Path::new(path), cwd).exists(),
            }
        }
        _ => {
            // No MCP name was recorded; `mcp_<server>_<tool>` (or with `__`) is the shape read here.
            let rest = tool.strip_prefix("mcp_")?.trim_start_matches('_');
            let (server, name) = rest.split_once("__").or_else(|| rest.split_once('_'))?;
            if server.is_empty() || name.is_empty() || !args.is_object() {
                return None;
            }
            PermissionDetail::Mcp {
                server: server.into(),
                tool: name.into(),
                args: flatten_input(args),
            }
        }
    })
}

/// The risk a call Marshell can't read carries (slice 6d decision 10).
const UNREADABLE: &str = "Marshell can't read this call";

/// The first risk the request carries, judged against the request's `cwd`: the core sets it to the tab's
/// folder, because agy's `workspacePaths` is empty. A call Marshell can't read is risky, so an agy update that
/// renames a tool or its arguments still reaches a card in Bypass: any `Other` but agy's own `read_file`.
pub fn risk_of(request: &Value) -> Option<String> {
    let cwd = Path::new(request["cwd"].as_str().unwrap_or(""));
    let detail = detail(request);
    if let PermissionDetail::Other { tool, .. } = &detail
        && !(tool == "read_file" && request["toolCall"]["args"]["Path"].is_string())
    {
        return Some(UNREADABLE.into());
    }
    risk::classify(&detail, cwd)
}

/// Builds a card without changing the held request or answering it. agy keeps no rule from Marshell, so
/// there is no always rule.
pub fn view(p: &PendingPermission) -> Option<PermissionView> {
    let tab = p.tab.clone()?;
    Some(PermissionView {
        rid: p.rid.clone(),
        tab,
        agent: p.agent.clone(),
        at_ms: p.at_ms,
        risk: risk_of(&p.request),
        detail: detail(&p.request),
        always_rule: None,
        deadline_ms: None,
    })
}

/// The hook output that carries `decision` back to agy, or why it can't. An allow answers `ask`, never `allow`:
/// agy keeps its own prompt for what `allow` would skip, and only Bypass holds, where `ask` runs the call. Deny
/// carries the person's words. agy keeps no rule from Marshell, so the project scope is refused.
pub fn permission_output(decision: &PermissionDecision, _request: &Value) -> Result<Value, &'static str> {
    match decision.behavior {
        PermissionBehavior::Deny => {
            let reason = decision
                .message
                .as_deref()
                .filter(|m| !m.is_empty())
                .unwrap_or(DENIED_TEXT);
            Ok(json!({ "decision": "deny", "reason": reason }))
        }
        PermissionBehavior::Allow if decision.scope == PermissionScope::Project => {
            Err("Antigravity can't keep an always-allow rule from Marshell. Allow this once instead.")
        }
        PermissionBehavior::Allow => Ok(json!({ "decision": "ask" })),
    }
}

/// What a release or the hold running out answers: a deny, because under `--dangerously-skip-permissions` (the
/// only mode that holds) an empty answer, `ask` and `force_ask` all run the call.
pub fn release_output() -> Value {
    json!({ "decision": "deny", "reason": DENIED_TEXT })
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::hook::{DiffKind, PermissionBehavior, PermissionScope};

    macro_rules! fixture {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../../tests/fixtures/antigravity/hooks/1.4.7/",
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

    #[test]
    fn a_run_command_is_a_bash_detail() {
        assert_eq!(
            detail(&fixture!("PreToolUse-run_command")),
            PermissionDetail::Bash {
                command: "echo hello-6d".into(),
                cwd: None
            }
        );
    }

    #[test]
    fn a_run_command_in_another_folder_names_it() {
        let request = json!({"cwd": "C:/work", "toolCall": {"name": "run_command",
            "args": {"CommandLine": "ls", "Cwd": "C:/work/sub"}}});
        assert_eq!(
            detail(&request),
            PermissionDetail::Bash {
                command: "ls".into(),
                cwd: Some("C:/work/sub".into())
            }
        );
        let same = json!({"cwd": "C:/work", "toolCall": {"name": "run_command",
            "args": {"CommandLine": "ls", "Cwd": "C:/work"}}});
        assert!(matches!(detail(&same), PermissionDetail::Bash { cwd: None, .. }));
    }

    #[test]
    fn a_write_file_is_a_write_detail() {
        let PermissionDetail::Write { path, preview, is_new } = detail(&fixture!("PreToolUse-write_file")) else {
            panic!("expected Write")
        };
        assert!(path.ends_with("notes.txt"), "{path}");
        assert_eq!(preview, "one");
        assert!(is_new);
    }

    #[test]
    fn an_edit_file_is_an_edit_detail() {
        let PermissionDetail::Edit { path, add, del, hunk } = detail(&fixture!("PreToolUse-edit_file")) else {
            panic!("expected Edit")
        };
        assert!(path.ends_with("notes.txt"), "{path}");
        assert_eq!((add, del), (1, 1));
        let lines: Vec<_> = hunk.iter().map(|l| (l.kind, l.text.as_str())).collect();
        assert_eq!(lines, vec![(DiffKind::Del, "one"), (DiffKind::Add, "two")]);
    }

    #[test]
    fn a_read_file_is_other_with_its_path() {
        for body in [
            fixture!("PreToolUse-read_file"),
            fixture!("PreToolUse-read_file-outside"),
        ] {
            let PermissionDetail::Other { tool, args } = detail(&body) else {
                panic!("expected Other")
            };
            assert_eq!(tool, "read_file");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].0, "Path");
        }
    }

    #[test]
    fn an_unknown_tool_is_other_with_its_args() {
        let request = json!({"toolCall": {"name": "browse_page", "args": {"Url": "https://example.com", "Depth": 2}}});
        assert_eq!(
            detail(&request),
            PermissionDetail::Other {
                tool: "browse_page".into(),
                args: vec![
                    ("Url".into(), "https://example.com".into()),
                    ("Depth".into(), "2".into())
                ]
            }
        );
        let nameless = json!({"stepIdx": 3});
        assert_eq!(
            detail(&nameless),
            PermissionDetail::Other {
                tool: "Unknown tool".into(),
                args: vec![("input".into(), "null".into())]
            }
        );
    }

    /// A call Marshell can't read is risky (slice 6d decision 10): a tool it doesn't know, or a known one without
    /// the arguments it reads. agy's own read and an MCP tool are not.
    #[test]
    fn a_call_marshell_cant_read_is_risky() {
        for request in [
            json!({"toolCall": {"name": "browse_page", "args": {"Url": "https://example.com"}}}),
            json!({"toolCall": {"name": "run_command", "args": {"Command": "rm -rf build"}}}),
            json!({"toolCall": {"name": "write_file", "args": {"Path": "a.txt"}}}),
            json!({"toolCall": {"name": "read_file", "args": {}}}),
            json!({"stepIdx": 3}),
        ] {
            assert_eq!(
                risk_of(&request).as_deref(),
                Some("Marshell can't read this call"),
                "{request}"
            );
        }
        for request in [
            fixture!("PreToolUse-read_file"),
            fixture!("PreToolUse-read_file-outside"),
            json!({"toolCall": {"name": "mcp_files_read", "args": {"path": "a.txt"}}}),
        ] {
            assert_eq!(risk_of(&request), None, "{request}");
        }
    }

    #[test]
    fn an_mcp_tool_is_an_mcp_detail() {
        let request = json!({"toolCall": {"name": "mcp_files_read", "args": {"path": "a.txt"}}});
        assert_eq!(
            detail(&request),
            PermissionDetail::Mcp {
                server: "files".into(),
                tool: "read".into(),
                args: vec![("path".into(), "a.txt".into())]
            }
        );
    }

    #[test]
    fn the_card_has_no_always_rule_and_names_the_risk() {
        let request =
            json!({"cwd": "C:/proj", "toolCall": {"name": "run_command", "args": {"CommandLine": "rm -rf build"}}});
        let p = PendingPermission {
            rid: "r".into(),
            tab: Some("t1".into()),
            agent: "antigravity".into(),
            tool: None,
            at_ms: 7,
            request,
        };
        let v = view(&p).unwrap();
        assert_eq!((v.tab.as_str(), v.agent.as_str(), v.at_ms), ("t1", "antigravity", 7));
        assert_eq!(v.always_rule, None);
        assert_eq!(v.risk.as_deref(), Some("Deletes files recursively"));
        assert!(view(&PendingPermission { tab: None, ..p }).is_none());
    }

    #[test]
    fn allow_answers_ask() {
        let out = permission_output(
            &decision(PermissionBehavior::Allow, PermissionScope::Once, None),
            &fixture!("PreToolUse-run_command"),
        )
        .unwrap();
        assert_eq!(out.to_string(), r#"{"decision":"ask"}"#);
    }

    #[test]
    fn deny_carries_the_reason() {
        let request = fixture!("PreToolUse-run_command");
        let out = permission_output(
            &decision(PermissionBehavior::Deny, PermissionScope::Once, Some("Not now")),
            &request,
        )
        .unwrap();
        assert_eq!(out.to_string(), r#"{"decision":"deny","reason":"Not now"}"#);
        for message in [None, Some("")] {
            let out = permission_output(
                &decision(PermissionBehavior::Deny, PermissionScope::Once, message),
                &request,
            )
            .unwrap();
            assert_eq!(out, json!({"decision": "deny", "reason": "Denied in Marshell."}));
        }
    }

    #[test]
    fn always_allow_is_refused_with_the_copy() {
        assert_eq!(
            permission_output(
                &decision(PermissionBehavior::Allow, PermissionScope::Project, None),
                &fixture!("PreToolUse-run_command")
            ),
            Err("Antigravity can't keep an always-allow rule from Marshell. Allow this once instead.")
        );
    }

    #[test]
    fn a_release_denies() {
        let out = release_output();
        assert_eq!(out, json!({"decision": "deny", "reason": DENIED_TEXT}));
        assert_eq!(out.as_object().unwrap().len(), 2);
    }

    #[test]
    fn no_answer_is_empty_allow_or_overwrite() {
        let request = fixture!("PreToolUse-run_command");
        let mut outs = vec![release_output()];
        for behavior in [PermissionBehavior::Allow, PermissionBehavior::Deny] {
            outs.push(permission_output(&decision(behavior, PermissionScope::Once, None), &request).unwrap());
        }
        for out in outs {
            let verdict = out["decision"].as_str().unwrap();
            assert!(matches!(verdict, "ask" | "deny"), "{out}");
            assert!(!out.to_string().contains("overwrite"));
        }
    }
}
