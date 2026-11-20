//! Copilot CLI's session event logs (`session-state/<id>/events.jsonl`), read into History's common conversation
//! format.

use super::super::common::{DETAIL_CAP, cap, created_summary, edited_summary, relative, rfc3339_ms};
use marshell_protocol::history::{Client, Conversation, EditedFile, ToolCard, Turn, TurnItem};
use serde_json::Value;
use std::collections::HashMap;

const TEXT_CAP: usize = 100_000;
const PROMPT_CAP: usize = 20_000;
const TITLE_CAP: usize = 200;
const SUMMARY_CAP: usize = 200;

/// The first `session.start` event's identity. Copilot's log doesn't tell a `-p` run from an interactive one
/// (6n2 decision 15), so `client` is always [`Client::Terminal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotMeta {
    pub id: String,
    pub started_ms: u64,
    pub cwd: String,
    pub branch: Option<String>,
    pub version: Option<String>,
    pub client: Client,
}

struct Pending {
    turn: usize,
    item: usize,
    name: String,
    input: Value,
}

static NULL: Value = Value::Null;

/// Reads an event log in file order, skipping malformed lines and event types not understood.
pub fn parse(text: &str, session_id: &str) -> Conversation {
    let cwd = meta(text).map(|m| m.cwd).unwrap_or_default();
    let mut turns: Vec<Turn> = Vec::new();
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let (mut version, mut untested) = (None, false);
    let (mut branch, mut model) = (None, None);

    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let data = v.get("data").unwrap_or(&NULL);
        match str_at(&v, "type") {
            Some(kind @ ("session.start" | "session.resume")) => {
                if let Some(ver) = non_empty(data, "copilotVersion") {
                    untested |= super::version_gate(Some(ver)) != super::Gate::Known;
                    version = Some(ver.to_string());
                } else if kind == "session.start" {
                    // A start that names no version leaves the log unverified.
                    untested = true;
                }
                if let Some(b) = data.get("context").and_then(|c| non_empty(c, "branch")) {
                    branch = Some(b.to_string());
                }
                if let Some(m) = non_empty(data, "selectedModel") {
                    model = Some(m.to_string());
                }
            }
            Some("session.model_change") => {
                if let Some(m) = non_empty(data, "newModel") {
                    model = Some(m.to_string());
                }
            }
            Some("user.message") => {
                // A continuation Copilot sends itself in autopilot stays in the turn before it.
                if data.get("isAutopilotContinuation").and_then(Value::as_bool) == Some(true) && !turns.is_empty() {
                    continue;
                }
                let prompt = str_at(data, "content").unwrap_or_default().trim();
                if !prompt.is_empty() {
                    turns.push(Turn {
                        index: turns.len() as u32,
                        at_ms: str_at(&v, "timestamp").and_then(rfc3339_ms).unwrap_or(0),
                        prompt: cap(prompt, PROMPT_CAP),
                        items: Vec::new(),
                    });
                }
            }
            Some("assistant.message") => {
                if let Some(m) = non_empty(data, "model") {
                    model = Some(m.to_string());
                }
                if let Some(turn) = turns.last_mut()
                    && let Some(text) = non_empty(data, "content").filter(|t| !t.trim().is_empty())
                {
                    turn.items.push(TurnItem::Text {
                        markdown: cap(text, TEXT_CAP),
                    });
                }
            }
            Some("tool.execution_start") => {
                let (Some(turn_at), Some(id)) = (turns.len().checked_sub(1), non_empty(data, "toolCallId")) else {
                    continue;
                };
                let name = str_at(data, "toolName").unwrap_or_default();
                let input = data.get("arguments").cloned().unwrap_or(Value::Null);
                let items = &mut turns[turn_at].items;
                pending.insert(
                    id.to_string(),
                    Pending {
                        turn: turn_at,
                        item: items.len(),
                        name: name.to_string(),
                        input: input.clone(),
                    },
                );
                items.push(TurnItem::Tool {
                    card: tool_card(name, &input, None, &cwd),
                });
            }
            Some("tool.execution_complete") => {
                let Some(call) = non_empty(data, "toolCallId").and_then(|id| pending.remove(id)) else {
                    continue;
                };
                if let Some(TurnItem::Tool { card }) = turns.get_mut(call.turn).and_then(|t| t.items.get_mut(call.item))
                {
                    *card = tool_card(&call.name, &call.input, Some(data), &cwd);
                }
            }
            Some("session.compaction_complete") => {
                if data.get("success").and_then(Value::as_bool) == Some(true)
                    && let Some(turn) = turns.last_mut()
                {
                    turn.items.push(TurnItem::Compacted);
                }
            }
            _ => {}
        }
    }

    let title = cap(turns.first().map(|t| t.prompt.as_str()).unwrap_or_default(), TITLE_CAP);
    Conversation {
        agent: "copilot".into(),
        session_id: session_id.into(),
        tab_id: String::new(),
        title,
        cwd,
        branch,
        model,
        limited: version.is_none() || untested,
        version,
        turns,
        marks: Default::default(),
    }
}

/// Reads metadata from the first `session.start` event with a nonempty `sessionId`.
pub fn meta(text: &str) -> Option<CopilotMeta> {
    text.lines().find_map(|line| {
        let v = serde_json::from_str::<Value>(line).ok()?;
        if str_at(&v, "type") != Some("session.start") {
            return None;
        }
        let data = v.get("data")?;
        let context = data.get("context").unwrap_or(&NULL);
        Some(CopilotMeta {
            id: non_empty(data, "sessionId")?.to_string(),
            started_ms: non_empty(data, "startTime")
                .or_else(|| str_at(&v, "timestamp"))
                .and_then(rfc3339_ms)
                .unwrap_or(0),
            cwd: str_at(context, "cwd").unwrap_or_default().to_string(),
            branch: non_empty(context, "branch").map(String::from),
            version: non_empty(data, "copilotVersion").map(String::from),
            client: Client::Terminal,
        })
    })
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn non_empty<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    str_at(v, key).filter(|s| !s.is_empty())
}

/// The card for a tool call: from its start alone while `done` is `None`, else with the `tool.execution_complete`
/// event's data.
fn tool_card(name: &str, input: &Value, done: Option<&Value>, cwd: &str) -> ToolCard {
    let result = done.and_then(|d| d.get("result"));
    let content = result.and_then(|r| non_empty(r, "content"));
    let patch = result.and_then(|r| non_empty(r, "detailedContent"));
    let failure = done
        .and_then(|d| d.get("error"))
        .filter(|e| !e.is_null())
        .map(|e| non_empty(e, "message"));
    let exit_code = done
        .and_then(|d| d.get("shellExecution"))
        .and_then(|s| s.get("exitCode"))
        .and_then(Value::as_i64);
    // A shell command that exits non-zero still reports `success: true`.
    let error = done.is_some_and(|d| d.get("success").and_then(Value::as_bool) == Some(false))
        || failure.is_some()
        || exit_code.is_some_and(|code| code != 0);
    let mut detail = content.or(failure.flatten()).map(String::from);
    let mut file = None;
    let path = non_empty(input, "path").map(|p| relative(p, cwd));
    let summary = match name {
        "powershell" | "bash" => match non_empty(input, "command").filter(|c| !c.trim().is_empty()) {
            Some(command) => {
                detail = Some(match content.map(str::trim).filter(|s| !s.is_empty()) {
                    Some(streams) => format!("{}\n\n{streams}", cap(command, DETAIL_CAP)),
                    None => cap(command, DETAIL_CAP),
                });
                format!(
                    "Ran {}{}",
                    cap(command.trim().lines().next().unwrap_or_default(), 80),
                    if error { ", failed" } else { "" }
                )
            }
            None => format!("Used {}", cap(name, SUMMARY_CAP)),
        },
        "create" | "edit" => match path {
            Some(path) => {
                let quoted = cap(&path, SUMMARY_CAP);
                let counts = patch.map(patch_counts);
                let given = done.is_some() && !error;
                if name == "create" {
                    let file_text = non_empty(input, "file_text");
                    let lines = file_text
                        .map(|t| t.lines().count())
                        .or(counts.map(|c| c.0))
                        .unwrap_or(0);
                    if given {
                        file = Some(EditedFile {
                            path,
                            added: Some(lines as u32),
                            removed: Some(0),
                            created: true,
                        });
                        detail = file_text.map(String::from).or(detail);
                        created_summary(&quoted, lines)
                    } else {
                        format!("Created {quoted}")
                    }
                } else if given {
                    file = Some(EditedFile {
                        path,
                        added: counts.map(|c| c.0 as u32),
                        removed: counts.map(|c| c.1 as u32),
                        created: false,
                    });
                    match counts {
                        Some((added, removed)) => {
                            detail = patch.map(String::from);
                            edited_summary(&quoted, added, removed)
                        }
                        None => format!("Edited {quoted}"),
                    }
                } else {
                    format!("Edited {quoted}")
                }
            }
            None => format!("Used {}", cap(name, SUMMARY_CAP)),
        },
        "view" => path
            .map(|p| format!("Read {}", cap(&p, SUMMARY_CAP)))
            .unwrap_or_else(|| format!("Used {}", cap(name, SUMMARY_CAP))),
        "ask_user" => match non_empty(input, "message").filter(|m| !m.trim().is_empty()) {
            Some(message) => format!("Asked {}", cap(message.trim().lines().next().unwrap_or_default(), 80)),
            None => format!("Used {}", cap(name, SUMMARY_CAP)),
        },
        _ => format!("Used {}", cap(name, SUMMARY_CAP)),
    };
    ToolCard {
        name: cap(name, SUMMARY_CAP),
        summary,
        detail: detail.filter(|d| !d.is_empty()).map(|d| cap(&d, DETAIL_CAP)),
        file,
        error,
    }
}

/// Count only hunk lines, leaving the `---`/`+++` file headers out of the totals.
fn patch_counts(patch: &str) -> (usize, usize) {
    let (mut added, mut removed, mut in_hunk) = (0, 0, false);
    for line in patch.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk {
            added += usize::from(line.starts_with('+'));
            removed += usize::from(line.starts_with('-'));
        }
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::history::{EditedFile, ToolCard, TurnItem};
    use serde_json::{Value, json};

    const HELLO: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000001.jsonl");
    const SHELL: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000026.jsonl");
    const CREATE: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000047.jsonl");
    const EDIT: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000068.jsonl");
    const VIEW: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000089.jsonl");
    const EXIT3: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000110.jsonl");
    const VIEW_FAILED: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000131.jsonl");
    const REPO: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000152.jsonl");
    const WSL_BASH: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000167.jsonl");
    const ASK: &str =
        include_str!("../../../tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000206.jsonl");
    const SID: &str = "00000000-0000-4000-8000-000000000001";

    fn events(text: &str) -> Vec<Value> {
        text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn jsonl(events: &[Value]) -> String {
        events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
    }

    fn event(kind: &str, data: Value) -> Value {
        json!({"type": kind, "data": data, "id": "hand-written", "timestamp": "2026-11-20T17:00:00.000Z", "parentId": null})
    }

    fn cards(conv: &Conversation) -> Vec<&ToolCard> {
        conv.turns
            .iter()
            .flat_map(|turn| &turn.items)
            .filter_map(|item| match item {
                TurnItem::Tool { card } => Some(card),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_text_reply_is_one_turn() {
        let all = events(HELLO);
        let resumed = all.iter().position(|v| v["type"] == "session.resume").unwrap();
        let conv = parse(&jsonl(&all[..resumed]), SID);
        assert_eq!(conv.agent, "copilot");
        assert_eq!(conv.session_id, SID);
        assert!(conv.tab_id.is_empty());
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(conv.turns[0].index, 0);
        assert_eq!(conv.turns[0].prompt, "hello"); // `transformedContent` has a date line in front.
        assert_eq!(conv.turns[0].at_ms, 1_795_192_378_288);
        assert_eq!(
            conv.turns[0].items,
            vec![TurnItem::Text {
                markdown: "done".into()
            }]
        );
        assert_eq!(
            crate::history::parse_for("copilot", HELLO, SID),
            Some(parse(HELLO, SID))
        );
    }

    #[test]
    fn each_tool_becomes_a_card() {
        let shell = parse(SHELL, SID);
        let shell = cards(&shell);
        assert_eq!(shell.len(), 1);
        assert_eq!(shell[0].name, "powershell");
        assert_eq!(shell[0].summary, "Ran mkdir made");
        assert!(shell[0].detail.as_deref().unwrap().starts_with("mkdir made"));
        assert!(shell[0].detail.as_deref().unwrap().contains("Directory:"));
        assert!(!shell[0].error);
        assert!(shell[0].file.is_none());
        let bash = parse(WSL_BASH, SID);
        assert_eq!(cards(&bash)[0].name, "bash");
        assert_eq!(cards(&bash)[0].summary, "Ran mkdir made");

        let write = parse(CREATE, SID);
        let write = cards(&write);
        assert_eq!(write.len(), 1);
        assert_eq!(write[0].name, "create");
        assert_eq!(write[0].summary, "Created notes.txt, 1 line");
        assert_eq!(write[0].detail.as_deref(), Some("marshell\n"));
        assert_eq!(
            write[0].file,
            Some(EditedFile {
                path: "notes.txt".into(),
                added: Some(1),
                removed: Some(0),
                created: true,
            })
        );
        assert!(!write[0].error);

        let edit = parse(EDIT, SID);
        let edit = cards(&edit);
        assert_eq!(edit.len(), 1);
        assert_eq!(edit[0].name, "edit");
        assert_eq!(edit[0].summary, "Edited notes.txt, 1 added, 1 removed");
        assert_eq!(
            edit[0].file,
            Some(EditedFile {
                path: "notes.txt".into(),
                added: Some(1),
                removed: Some(1),
                created: false,
            })
        );
        assert!(
            edit[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("-marshell\n+marshell edited")
        );
        assert!(!edit[0].error);

        let view = parse(VIEW, SID);
        let view = cards(&view);
        assert_eq!(view.len(), 1);
        assert_eq!(view[0].name, "view");
        assert_eq!(view[0].summary, "Read notes.txt");
        assert!(view[0].file.is_none());
        assert!(!view[0].error);
    }

    #[test]
    fn a_failed_tool_is_an_error_card() {
        let conv = parse(VIEW_FAILED, SID);
        let failed = cards(&conv);
        assert_eq!(failed.len(), 1);
        assert!(failed[0].error);
        assert_eq!(failed[0].summary, "Read nope.txt");
        assert!(failed[0].detail.as_deref().unwrap().contains("does not exist"));
        assert!(failed[0].file.is_none());
        // A failed edit gives no EditedFile.
        let mut all = events(EDIT);
        for e in &mut all {
            if e["type"] == "tool.execution_complete" {
                e["data"]["success"] = json!(false);
                e["data"]["error"] = json!({"message": "no match", "code": "failure"});
            }
        }
        let conv = parse(&jsonl(&all), SID);
        let edit = cards(&conv);
        assert!(edit[0].error);
        assert!(edit[0].file.is_none());
    }

    #[test]
    fn the_title_is_the_first_prompt() {
        assert_eq!(parse(HELLO, SID).title, "hello");
        let mut first = events(HELLO);
        let at = first.iter().position(|v| v["type"] == "user.message").unwrap();
        first[at]["data"]["content"] = json!("x".repeat(201));
        assert_eq!(parse(&jsonl(&first), SID).title, format!("{}…", "x".repeat(200)));
    }

    #[test]
    fn a_compaction_event_is_a_compacted_item() {
        // Copilot's `session-events.schema.json` names it; none was recorded.
        let mut all = events(HELLO);
        all.push(event(
            "session.compaction_complete",
            json!({"success": true, "preCompactionTokens": 100}),
        ));
        let conv = parse(&jsonl(&all), SID);
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(
            conv.turns[1].items,
            vec![
                TurnItem::Text {
                    markdown: "done".into()
                },
                TurnItem::Compacted
            ]
        );
        // A compaction that did not succeed is not one.
        let last = all.len() - 1;
        all[last]["data"]["success"] = json!(false);
        assert_eq!(parse(&jsonl(&all), SID), parse(HELLO, SID));
    }

    #[test]
    fn a_shell_command_that_exits_non_zero_is_an_error_card() {
        let conv = parse(EXIT3, SID);
        let shell = cards(&conv);
        assert_eq!(shell.len(), 1);
        assert!(shell[0].error);
        assert_eq!(shell[0].summary, "Ran cmd /c exit 3, failed");
        assert!(shell[0].detail.as_deref().unwrap().contains("exit code 3"));
        assert!(shell[0].file.is_none());
    }

    #[test]
    fn a_question_is_a_card_with_its_answer() {
        let conv = parse(ASK, SID);
        assert_eq!(conv.turns.len(), 1);
        let ask = cards(&conv);
        assert_eq!(ask.len(), 1);
        assert_eq!(ask[0].name, "ask_user");
        assert_eq!(ask[0].summary, "Asked Which one?");
        assert_eq!(ask[0].detail.as_deref(), Some("User responded: yes"));
        assert!(!ask[0].error);
        assert!(ask[0].file.is_none());
    }

    #[test]
    fn hook_and_bookkeeping_events_add_nothing() {
        let conv = parse(WSL_BASH, SID);
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(conv.turns[0].prompt, "RUN mkdir made");
        assert_eq!(conv.turns[0].items.len(), 2); // the bash card, then "done"
        assert_eq!(
            conv.turns[0].items[1],
            TurnItem::Text {
                markdown: "done".into()
            }
        );
        let kinds: Vec<String> = events(WSL_BASH)
            .iter()
            .map(|e| e["type"].as_str().unwrap().to_string())
            .collect();
        for kind in [
            "hook.start",
            "hook.end",
            "session.usage_record",
            "session.shutdown",
            "system.message",
            "instruction.update",
        ] {
            assert!(kinds.iter().any(|k| k == kind), "{kind} is in the fixture");
        }
        // A hand-written continuation of the same kinds changes nothing either.
        let mut all = events(WSL_BASH);
        for kind in [
            "hook.start",
            "system.message",
            "session.model_change",
            "assistant.turn_end",
        ] {
            all.push(event(kind, json!({"content": "not a turn", "newModel": "x"})));
        }
        assert_eq!(parse(&jsonl(&all), SID).turns, conv.turns);
    }

    #[test]
    fn a_resumed_session_reads_on_as_one_conversation() {
        let conv = parse(HELLO, SID);
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[1].index, 1);
        assert_eq!(conv.turns[1].prompt, "hello again");
        assert_eq!(conv.turns[1].at_ms, 1_795_192_395_309);
        assert_eq!(
            conv.turns[1].items,
            vec![TurnItem::Text {
                markdown: "done".into()
            }]
        );
        // An autopilot continuation stays in the turn before it.
        let mut all = events(HELLO);
        let resumed = all.iter().position(|v| v["type"] == "user.message").unwrap();
        all.insert(
            resumed + 1,
            event(
                "user.message",
                json!({"content": "continue", "isAutopilotContinuation": true}),
            ),
        );
        all.insert(
            resumed + 2,
            event("assistant.message", json!({"content": "more", "toolRequests": []})),
        );
        let conv = parse(&jsonl(&all), SID);
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[0].prompt, "hello");
        assert_eq!(
            conv.turns[0].items,
            vec![
                TurnItem::Text {
                    markdown: "more".into()
                },
                TurnItem::Text {
                    markdown: "done".into()
                }
            ]
        );
    }

    #[test]
    fn an_unknown_event_type_is_skipped_and_the_turns_around_it_stay() {
        let mut all = events(HELLO);
        let at = all.iter().position(|v| v["type"] == "assistant.message").unwrap();
        all.insert(
            at,
            event("session.future_thing", json!({"content": "x", "content2": [1, 2]})),
        );
        all.insert(at, json!({"no": "type"}));
        all.insert(at, json!([1, 2, 3]));
        all.insert(at, event("assistant.message", json!("not an object")));
        all.insert(at, event("user.message", json!({"content": 7})));
        all.insert(at, event("tool.execution_start", json!({"toolName": "powershell"})));
        all.insert(at, event("tool.execution_complete", json!({"toolCallId": "nobody"})));
        assert_eq!(parse(&jsonl(&all), SID), parse(HELLO, SID));
    }

    #[test]
    fn a_truncated_last_line_is_ignored() {
        assert_eq!(
            parse(
                &format!("{HELLO}\n{{\"type\":\"assistant.message\",\"data\":{{\"con"),
                SID
            ),
            parse(HELLO, SID)
        );
        assert_eq!(meta(&format!("{HELLO}\n{{\"type\":\"session.start\"")), meta(HELLO));
        assert!(parse("\nnot json\n{", SID).turns.is_empty());
        assert!(meta("\nnot json\n{").is_none());
        // A cut inside a multi-byte character is not valid JSON and is skipped, not a panic.
        let cut = format!("{HELLO}\n{{\"type\":\"user.message\",\"data\":{{\"content\":\"é");
        assert_eq!(parse(&cut, SID), parse(HELLO, SID));
    }

    #[test]
    fn model_and_version_come_from_the_events() {
        let conv = parse(REPO, SID);
        assert_eq!(conv.model.as_deref(), Some("marshell-fake"));
        assert_eq!(conv.version.as_deref(), Some("1.0.106"));
        assert_eq!(conv.branch.as_deref(), Some("work-branch"));
        assert_eq!(conv.cwd, r"C:\Users\user\AppData\Local\Temp\copilot-6n2-t1\proj");
        assert!(parse(HELLO, SID).branch.is_none());
        let mut all = events(HELLO);
        let reply = all.iter_mut().rev().find(|v| v["type"] == "assistant.message").unwrap();
        reply["data"]["model"] = json!("another-model");
        assert_eq!(parse(&jsonl(&all), SID).model.as_deref(), Some("another-model"));
        all.push(event("session.model_change", json!({"newModel": "third-model"})));
        assert_eq!(parse(&jsonl(&all), SID).model.as_deref(), Some("third-model"));
    }

    #[test]
    fn limited_follows_the_version_gate() {
        for version in [
            None,
            Some(""),
            Some("garbage"),
            Some("1.0.105"),
            Some("1.0.106"),
            Some("1.0.120"),
            Some("1.1.0"),
        ] {
            let mut all = events(HELLO);
            for e in &mut all {
                if e["type"] == "session.start" {
                    e["data"]["copilotVersion"] = json!(version);
                }
            }
            assert_eq!(
                parse(&jsonl(&all), SID).limited,
                super::super::version_gate(version) != super::super::Gate::Known,
                "{version:?}"
            );
        }
    }

    #[test]
    fn meta_reads_id_cwd_branch_and_start() {
        let m = meta(REPO).unwrap();
        assert_eq!(m.id, "00000000-0000-4000-8000-000000000152");
        assert_eq!(m.started_ms, 1_795_192_854_195);
        assert_eq!(m.cwd, r"C:\Users\user\AppData\Local\Temp\copilot-6n2-t1\proj");
        assert_eq!(m.branch.as_deref(), Some("work-branch"));
        assert_eq!(m.version.as_deref(), Some("1.0.106"));
        assert_eq!(m.client, Client::Terminal); // `-p` and interactive runs read alike.
        let plain = meta(HELLO).unwrap();
        assert!(plain.branch.is_none());
        assert_eq!(plain.started_ms, 1_795_192_378_081);
        assert_eq!(meta(WSL_BASH).unwrap().cwd, "/tmp/copilot-6n2-t1/proj");
        // Only the first session.start counts.
        let mut all = events(REPO);
        all.push(event(
            "session.start",
            json!({"sessionId": "later", "context": {"cwd": "C:/later"}}),
        ));
        let text = format!("not json\n{{\"type\":\"user.message\"}}\n{}", jsonl(&all));
        assert_eq!(meta(&text).unwrap(), m);
    }

    #[test]
    fn wrong_typed_fields_never_panic() {
        let kinds = [
            "session.start",
            "session.resume",
            "session.model_change",
            "user.message",
            "assistant.message",
            "tool.execution_start",
            "tool.execution_complete",
            "session.compaction_complete",
        ];
        let junk = [
            json!(null),
            json!(7),
            json!("é".repeat(5)),
            json!([1, {"a": null}]),
            json!({"content": 1, "toolCallId": 2, "toolName": [], "arguments": "x", "result": "y", "error": 3,
                   "shellExecution": {"exitCode": "3"}, "success": "no", "context": 5, "selectedModel": {},
                   "startTime": "é", "copilotVersion": 1, "sessionId": []}),
            json!({"toolCallId": "c", "toolName": "edit", "arguments": {"path": "é/é"}, "result": {"detailedContent": 1}}),
        ];
        let mut all = Vec::new();
        for kind in kinds {
            for data in &junk {
                all.push(event(kind, data.clone()));
                all.push(json!({"type": kind}));
            }
        }
        // A turn first, so the tool events have somewhere to land.
        all.insert(0, event("user.message", json!({"content": "hi"})));
        let text = jsonl(&all);
        let conv = parse(&text, SID);
        assert_eq!(conv.turns[0].prompt, "hi");
        let _ = meta(&text);
        // Huge strings are cut, on a character boundary.
        let big = "é".repeat(200_000);
        let huge = [
            event("user.message", json!({"content": big})),
            event(
                "tool.execution_start",
                json!({"toolCallId": "c", "toolName": "bash", "arguments": {"command": big}}),
            ),
            event(
                "tool.execution_complete",
                json!({"toolCallId": "c", "success": true, "result": {"content": big}}),
            ),
        ];
        let conv = parse(&jsonl(&huge), SID);
        assert_eq!(conv.turns[0].prompt.chars().count(), 20_001);
        assert!(conv.turns[0].items.len() == 1);
    }

    #[test]
    fn meta_is_none_without_a_session_start() {
        assert!(meta("").is_none());
        let rest: Vec<Value> = events(HELLO)
            .into_iter()
            .filter(|e| e["type"] != "session.start")
            .collect();
        assert!(meta(&jsonl(&rest)).is_none());
        let blank = event("session.start", json!({"sessionId": "", "context": {"cwd": "x"}}));
        assert!(meta(&blank.to_string()).is_none());
    }
}
