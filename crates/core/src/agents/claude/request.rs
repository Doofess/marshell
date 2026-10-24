//! Maps Claude's permission payload to the agent-neutral approve card.

use crate::agents::common::flatten_input;
use crate::hooks::{holds::PendingPermission, risk};
use marshell_protocol::hook::{DiffKind, DiffLine, PermissionDetail, PermissionView};
use serde_json::Value;
use std::path::Path;

/// Describes known tools, keeping malformed or unknown input visible as key/value lines.
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
        "Bash" => PermissionDetail::Bash {
            command: input["command"].as_str()?.into(),
            cwd: input["cwd"]
                .as_str()
                .filter(|cwd| {
                    request["cwd"].as_str().is_none_or(|base| {
                        !risk::paths_equal(&risk::resolve_path(Path::new(cwd), Path::new(base)), Path::new(base))
                    })
                })
                .map(str::to_owned),
        },
        "Edit" => {
            let path = input["file_path"].as_str()?.into();
            let patch = diffy::create_patch(input["old_string"].as_str()?, input["new_string"].as_str()?);
            let mut add = 0_u32;
            let mut del = 0_u32;
            for line in patch.hunks().iter().flat_map(|hunk| hunk.lines()) {
                match line {
                    diffy::Line::Insert(_) => add = add.saturating_add(1),
                    diffy::Line::Delete(_) => del = del.saturating_add(1),
                    diffy::Line::Context(_) => {}
                }
            }
            let hunk = patch
                .hunks()
                .first()
                .into_iter()
                .flat_map(|hunk| hunk.lines())
                .map(|line| {
                    let (kind, text) = match line {
                        diffy::Line::Context(text) => (DiffKind::Context, *text),
                        diffy::Line::Insert(text) => (DiffKind::Add, *text),
                        diffy::Line::Delete(text) => (DiffKind::Del, *text),
                    };
                    let text = text.strip_suffix('\n').unwrap_or(text);
                    DiffLine {
                        kind,
                        text: text.strip_suffix('\r').unwrap_or(text).into(),
                    }
                })
                .collect();
            PermissionDetail::Edit { path, add, del, hunk }
        }
        "Write" => {
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
        "WebFetch" => PermissionDetail::Fetch {
            url: input["url"].as_str()?.into(),
        },
        _ => {
            let (server, tool) = tool.strip_prefix("mcp__")?.split_once("__")?;
            if server.is_empty() || tool.is_empty() || !input.is_object() {
                return None;
            }
            PermissionDetail::Mcp {
                server: server.into(),
                tool: tool.into(),
                args: flatten_input(input),
            }
        }
    })
}

/// Shared with the permission response so only suggestions the CLI can accept count.
pub(super) fn allow_rule_suggestions(request: &Value) -> impl Iterator<Item = &Value> {
    request["permission_suggestions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["type"] == "addRules" && s["behavior"] == "allow")
}

/// Every rule `permission_output` writes for the project scope, so the card names all of them.
/// `None` when there is none, or when one can't be named (the card then offers no "Always").
pub fn always_rule(request: &Value) -> Option<String> {
    let mut rules = Vec::new();
    for suggestion in allow_rule_suggestions(request) {
        rules.extend(suggestion["rules"].as_array()?);
    }
    let rendered: Option<Vec<_>> = rules
        .into_iter()
        .map(|rule| {
            let tool = rule["toolName"].as_str().filter(|s| !s.is_empty())?;
            Some(match rule["ruleContent"].as_str().filter(|s| !s.is_empty()) {
                Some(content) => format!("{tool}({content})"),
                None => tool.to_owned(),
            })
        })
        .collect();
    let rendered = rendered?;
    (!rendered.is_empty()).then(|| rendered.join(", "))
}

/// Builds a card without changing the held request or answering it.
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
        always_rule: always_rule(&p.request),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::hook::{DiffKind, PermissionBehavior, PermissionDecision, PermissionScope};
    use serde_json::json;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/claude/2.1.302/PermissionRequest.json"
        ))
        .unwrap()
    }

    fn request(tool: &str, input: Value) -> Value {
        json!({"cwd": if cfg!(windows) { "C:/project" } else { "/project" }, "tool_name": tool, "tool_input": input})
    }

    #[test]
    fn detail_from_fixture() {
        assert_eq!(
            detail(&fixture()),
            PermissionDetail::Bash {
                command: "python -c \"print(4242)\"".into(),
                cwd: None
            }
        );
        assert_eq!(
            always_rule(&fixture()).as_deref(),
            Some("Bash(python -c \"print(4242)\")")
        );
    }

    #[test]
    fn detail_edit_counts_the_hunk() {
        let d = detail(&request(
            "Edit",
            json!({"file_path":"a.ts", "old_string":"same\nold\n", "new_string":"same\nnew\nextra\n"}),
        ));
        let PermissionDetail::Edit { path, add, del, hunk } = d else {
            panic!("expected Edit")
        };
        assert_eq!(path, "a.ts");
        assert_eq!((add, del), (2, 1));
        assert_eq!(
            hunk.iter()
                .map(|line| (line.kind, line.text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (DiffKind::Context, "same"),
                (DiffKind::Del, "old"),
                (DiffKind::Add, "new"),
                (DiffKind::Add, "extra")
            ]
        );
    }

    #[test]
    fn detail_edit_shows_first_hunk_and_counts_all_changes() {
        let middle = (0..20).map(|n| format!("line {n}\n")).collect::<String>();
        let d = detail(&request(
            "Edit",
            json!({"file_path":"a.ts", "old_string": format!("old\n{middle}last\n"), "new_string":format!("new\n{middle}end\n")}),
        ));
        let PermissionDetail::Edit { add, del, hunk, .. } = d else {
            panic!("expected Edit")
        };
        assert_eq!((add, del), (2, 2));
        assert!(hunk.iter().any(|line| line.text == "new"));
        assert!(!hunk.iter().any(|line| line.text == "end"));
    }

    #[test]
    fn detail_mcp_flattens_args() {
        let PermissionDetail::Mcp { server, tool, args } = detail(&request(
            "mcp__github__create_pr",
            json!({"title":"Fix", "nested":{"draft":true}, "labels":["bug"], "count":2}),
        )) else {
            panic!("expected Mcp")
        };
        assert_eq!((server.as_str(), tool.as_str()), ("github", "create_pr"));
        assert_eq!(
            args.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
            std::collections::BTreeMap::from([
                ("count".into(), "2".into()),
                ("labels".into(), "[\"bug\"]".into()),
                ("nested".into(), "{\"draft\":true}".into()),
                ("title".into(), "Fix".into())
            ])
        );
    }

    #[test]
    fn detail_unknown_tool_is_other() {
        assert_eq!(
            detail(&request("NotebookEdit", json!({"cell":3}))),
            PermissionDetail::Other {
                tool: "NotebookEdit".into(),
                args: vec![("cell".into(), "3".into())]
            }
        );
    }

    #[test]
    fn detail_missing_command_is_other() {
        assert_eq!(
            detail(&request("Bash", json!({"description":"Run tests"}))),
            PermissionDetail::Other {
                tool: "Bash".into(),
                args: vec![("description".into(), "Run tests".into())]
            }
        );
    }

    #[test]
    fn always_rule_none_for_add_directories_only() {
        let r = json!({"permission_suggestions":[{"type":"addDirectories", "directories":["."], "behavior":"allow"}, {"type":"setMode", "mode":"acceptEdits"}, {"type":"addRules", "behavior":"deny", "rules":[{"toolName":"Bash"}]}]});
        assert_eq!(always_rule(&r), None);
        let decision = PermissionDecision {
            behavior: PermissionBehavior::Allow,
            scope: PermissionScope::Project,
            message: None,
        };
        assert_eq!(
            super::super::permission_output(&decision, &r),
            Err("This request has no rule to add.")
        );
    }

    #[test]
    fn always_rule_uses_every_allow_suggestion_and_renders_each_rule() {
        let r = json!({"permission_suggestions":[{"type":"addRules", "behavior":"deny"}, {"type":"addRules", "behavior":"allow", "rules":[{"toolName":"Bash", "ruleContent":"npm test:*"}, {"toolName":"Read"}, {"toolName":"Edit", "ruleContent":""}]}, {"type":"addRules", "behavior":"allow", "rules":[{"toolName":"Write"}]}]});
        assert_eq!(always_rule(&r).as_deref(), Some("Bash(npm test:*), Read, Edit, Write"));
    }

    #[test]
    fn always_rule_names_every_rule_permission_output_writes() {
        let r = json!({"permission_suggestions":[
            {"type":"addRules", "behavior":"allow", "rules":[{"toolName":"Bash", "ruleContent":"npm test:*"}]},
            {"type":"addRules", "behavior":"allow", "rules":[{"toolName":"Bash", "ruleContent":"rm:*"}]}
        ]});
        assert_eq!(always_rule(&r).as_deref(), Some("Bash(npm test:*), Bash(rm:*)"));
        let unnamed = json!({"permission_suggestions":[
            {"type":"addRules", "behavior":"allow", "rules":[{"toolName":"Bash", "ruleContent":"npm test:*"}]},
            {"type":"addRules", "behavior":"allow", "rules":[{"ruleContent":"rm:*"}]}
        ]});
        assert_eq!(always_rule(&unnamed), None);
    }

    #[test]
    fn detail_write_preview_and_existence_resolve_against_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = request(
            "Write",
            json!({"file_path":"a.ts", "content":"one\ntwo\nthree\nfour\nfive\n"}),
        );
        r["cwd"] = json!(dir.path());
        assert_eq!(
            detail(&r),
            PermissionDetail::Write {
                path: "a.ts".into(),
                preview: "one\ntwo\nthree\nfour".into(),
                is_new: true
            }
        );
        std::fs::write(dir.path().join("a.ts"), "old").unwrap();
        assert!(matches!(detail(&r), PermissionDetail::Write { is_new: false, .. }));
    }

    #[test]
    fn detail_write_never_touches_a_remote_path() {
        // `.exists()` on a UNC path opens an SMB connection, which can leak NTLM credentials to a host a repo names.
        for path in [
            r"\\nohost.invalid\share\x.txt",
            "//nohost.invalid/share/x",
            r"\\.\pipe\x",
        ] {
            let r = request("Write", json!({"file_path": path, "content": "x"}));
            assert!(
                matches!(detail(&r), PermissionDetail::Write { is_new: false, .. }),
                "{path}"
            );
        }
    }

    #[test]
    fn detail_bash_cwd_and_web_fetch() {
        let mut r = request("Bash", json!({"command":"pwd"}));
        r["tool_input"]["cwd"] = r["cwd"].clone();
        assert!(matches!(detail(&r), PermissionDetail::Bash { cwd: None, .. }));
        r["tool_input"]["cwd"] = json!("../other");
        assert!(matches!(detail(&r), PermissionDetail::Bash { cwd:Some(ref cwd), .. } if cwd == "../other"));
        assert_eq!(
            detail(&request("WebFetch", json!({"url":"https://example.test"}))),
            PermissionDetail::Fetch {
                url: "https://example.test".into()
            }
        );
    }

    #[test]
    fn view_preserves_identity_and_dispatch_requires_known_agent_and_tab() {
        let mut p = PendingPermission {
            rid: "r1".into(),
            tab: Some("t1".into()),
            agent: "claude".into(),
            tool: Some("Bash".into()),
            at_ms: 123,
            request: fixture(),
        };
        let v = crate::agents::permission_view(&p).unwrap();
        assert_eq!(
            (&v.rid, &v.tab, &v.agent, v.at_ms),
            (&p.rid, p.tab.as_ref().unwrap(), &p.agent, 123)
        );
        assert_eq!(v.detail, detail(&p.request));
        assert_eq!(v.always_rule, always_rule(&p.request));
        assert_eq!(v.risk, None);
        p.request = request("Write", json!({"file_path":"../other", "content":"x"}));
        assert_eq!(view(&p).unwrap().risk.as_deref(), Some("Writes outside this project"));
        p.agent = "unknown".into();
        assert_eq!(crate::agents::permission_view(&p), None);
        p.agent = "claude".into();
        p.tab = None;
        assert_eq!(view(&p), None);
        assert_eq!(crate::agents::permission_view(&p), None);
    }

    #[test]
    fn malformed_and_odd_inputs_never_panic_or_lose_the_tool() {
        for input in [
            Value::Null,
            json!(42),
            json!([1, 2]),
            json!("שלום 🦀"),
            json!({"command":[], "file_path":true, "content":{}, "url":5}),
            json!({"command":""}),
            json!({"command":"x".repeat(100_000)}),
        ] {
            for tool in [
                "Bash",
                "Edit",
                "Write",
                "WebFetch",
                "NotebookEdit",
                "mcp__github__create_pr",
                "mcp____",
            ] {
                let r = request(tool, input.clone());
                let result = std::panic::catch_unwind(|| (detail(&r), always_rule(&r)));
                assert!(result.is_ok(), "{tool}");
                if let PermissionDetail::Other { tool: actual, .. } = result.unwrap().0 {
                    assert_eq!(actual, tool);
                }
            }
        }
        assert!(matches!(detail(&json!({})), PermissionDetail::Other { tool, .. } if !tool.is_empty()));
    }
}
