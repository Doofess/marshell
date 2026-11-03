//! Qwen Code's chat files, read into History's common conversation format.

use super::super::common::{DETAIL_CAP, cap, created_summary, edited_summary, relative, rfc3339_ms};
use marshell_protocol::history::{Client, Conversation, EditedFile, ToolCard, Turn, TurnItem};
use serde_json::Value;
use std::collections::HashMap;

const TEXT_CAP: usize = 100_000;
const PROMPT_CAP: usize = 20_000;
const TITLE_CAP: usize = 200;
const SUMMARY_CAP: usize = 200;

/// The first record's session identity. Qwen does not distinguish headless runs from terminal sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QwenMeta {
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

/// Reads a chat in file order, skipping malformed lines and record kinds not understood.
pub fn parse(text: &str, session_id: &str) -> Conversation {
    let cwd = meta(text).map(|m| m.cwd).unwrap_or_default();
    let mut turns: Vec<Turn> = Vec::new();
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let (mut version, mut untested) = (None, false);
    let (mut branch, mut model, mut custom_title) = (None, None, None);

    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(ver) = non_empty(&v, "version") {
            untested |= super::version_gate(Some(ver)) != super::Gate::Known;
            version = Some(ver.to_string());
        }
        if let Some(b) = non_empty(&v, "gitBranch") {
            branch = Some(b.to_string());
        }
        let parts = v.get("message").and_then(|m| m.get("parts")).and_then(Value::as_array);
        let parts = parts.map(Vec::as_slice).unwrap_or_default();
        match str_at(&v, "type") {
            Some("user" | "tool_result") => {
                let responses: Vec<&Value> = parts.iter().filter_map(|p| p.get("functionResponse")).collect();
                if str_at(&v, "type") == Some("user") && responses.is_empty() {
                    let prompt = text_parts(parts).trim().to_string();
                    if !prompt.is_empty() {
                        turns.push(Turn {
                            index: turns.len() as u32,
                            at_ms: str_at(&v, "timestamp").and_then(rfc3339_ms).unwrap_or(0),
                            prompt: cap(&prompt, PROMPT_CAP),
                            items: Vec::new(),
                        });
                    }
                    if let Some(m) = v.get("executionContext").and_then(|c| non_empty(c, "modelId")) {
                        model = Some(m.to_string());
                    }
                    continue;
                }
                let structured = v.get("toolCallResult");
                for response in &responses {
                    let Some(id) = str_at(response, "id") else {
                        continue;
                    };
                    let result = structured
                        .filter(|r| str_at(r, "callId").map_or(responses.len() == 1, |call_id| call_id == id));
                    finish_tool(id, Some(response), result, &mut pending, &mut turns, &cwd);
                }
                // A structured tool_result can stand alone without a functionResponse part.
                if responses.is_empty()
                    && let Some(id) = structured.and_then(|r| str_at(r, "callId"))
                {
                    finish_tool(id, None, structured, &mut pending, &mut turns, &cwd);
                }
            }
            Some("assistant") => {
                if let Some(m) = non_empty(&v, "model") {
                    model = Some(m.to_string());
                }
                let Some(turn_at) = turns.len().checked_sub(1) else {
                    continue;
                };
                for part in parts {
                    if part.get("thought").and_then(Value::as_bool) == Some(true) {
                        continue;
                    }
                    if let Some(text) = non_empty(part, "text").filter(|t| !t.trim().is_empty()) {
                        turns[turn_at].items.push(TurnItem::Text {
                            markdown: cap(text, TEXT_CAP),
                        });
                    }
                    if let Some(call) = part.get("functionCall") {
                        let name = str_at(call, "name").unwrap_or_default();
                        let input = call.get("args").cloned().unwrap_or(Value::Null);
                        let items = &mut turns[turn_at].items;
                        if let Some(id) = str_at(call, "id") {
                            pending.insert(
                                id.to_string(),
                                Pending {
                                    turn: turn_at,
                                    item: items.len(),
                                    name: name.to_string(),
                                    input: input.clone(),
                                },
                            );
                        }
                        items.push(TurnItem::Tool {
                            card: tool_card(name, &input, None, None, &cwd),
                        });
                    }
                }
            }
            Some("system") => match str_at(&v, "subtype") {
                Some("custom_title") => {
                    if let Some(title) = v.get("systemPayload").and_then(|p| non_empty(p, "customTitle")) {
                        let title = title.trim();
                        if !title.is_empty() {
                            custom_title = Some(cap(title, TITLE_CAP));
                        }
                    }
                }
                Some("chat_compression") => {
                    if let Some(turn) = turns.last_mut() {
                        turn.items.push(TurnItem::Compacted);
                    }
                }
                Some("session_model") => {
                    if let Some(m) = v.get("systemPayload").and_then(|p| non_empty(p, "modelId")) {
                        model = Some(m.to_string());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    let title =
        custom_title.unwrap_or_else(|| cap(turns.first().map(|t| t.prompt.as_str()).unwrap_or_default(), TITLE_CAP));
    Conversation {
        agent: "qwen".into(),
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

/// Reads metadata from the first valid record with a nonempty `sessionId`.
pub fn meta(text: &str) -> Option<QwenMeta> {
    text.lines().find_map(|line| {
        let v = serde_json::from_str::<Value>(line).ok()?;
        Some(QwenMeta {
            id: non_empty(&v, "sessionId")?.to_string(),
            started_ms: str_at(&v, "timestamp").and_then(rfc3339_ms).unwrap_or(0),
            cwd: str_at(&v, "cwd").unwrap_or_default().to_string(),
            branch: non_empty(&v, "gitBranch").map(String::from),
            version: non_empty(&v, "version").map(String::from),
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

fn text_parts(parts: &[Value]) -> String {
    parts
        .iter()
        .filter_map(|p| str_at(p, "text"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn finish_tool(
    id: &str,
    response: Option<&Value>,
    result: Option<&Value>,
    pending: &mut HashMap<String, Pending>,
    turns: &mut [Turn],
    cwd: &str,
) {
    let Some(call) = pending.remove(id) else {
        return;
    };
    if let Some(TurnItem::Tool { card }) = turns.get_mut(call.turn).and_then(|t| t.items.get_mut(call.item)) {
        *card = tool_card(&call.name, &call.input, response, result, cwd);
    }
}

fn tool_card(name: &str, input: &Value, response: Option<&Value>, result: Option<&Value>, cwd: &str) -> ToolCard {
    let display = result.and_then(|r| r.get("resultDisplay"));
    let response = response.and_then(|r| r.get("response"));
    let output = response.and_then(|r| str_at(r, "error").or_else(|| str_at(r, "output")));
    let error = result.is_some_and(|r| matches!(str_at(r, "status"), Some("error" | "cancelled")))
        || response.is_some_and(|r| r.get("error").is_some_and(|e| !e.is_null()))
        || display.is_some_and(|d| {
            matches!(str_at(d, "outcome"), Some("failed" | "cancelled"))
                || d.get("exitCode").and_then(Value::as_i64).is_some_and(|code| code != 0)
        });
    let mut detail = output.or_else(|| display.and_then(Value::as_str)).map(String::from);
    let mut file = None;
    let path = str_at(input, "file_path")
        .or_else(|| display.and_then(|d| str_at(d, "filePath")))
        .map(|p| relative(p, cwd));
    let summary = match name {
        "run_shell_command" => match non_empty(input, "command") {
            Some(command) => {
                let streams = display
                    .and_then(|d| non_empty(d, "text").or_else(|| non_empty(d, "output")))
                    .or(output)
                    .unwrap_or_default()
                    .trim_end();
                detail = Some(if streams.is_empty() {
                    command.to_string()
                } else {
                    format!("{command}\n\n{streams}")
                });
                format!(
                    "Ran {}{}",
                    cap(command.trim().lines().next().unwrap_or_default(), 80),
                    if error { ", failed" } else { "" }
                )
            }
            None => format!("Used {}", cap(name, SUMMARY_CAP)),
        },
        "write_file" | "edit" => match path {
            Some(path) => {
                let quoted = cap(&path, SUMMARY_CAP);
                let patch = display.and_then(|d| str_at(d, "fileDiff"));
                let counts = patch.map(patch_counts);
                // Qwen explicitly distinguishes creating from overwriting, including an existing empty file.
                let created = name == "write_file"
                    && output.is_some_and(|s| s.starts_with("Successfully created and wrote to new file:"));
                let content = display
                    .and_then(|d| str_at(d, "newContent"))
                    .or_else(|| str_at(input, "content"));
                if !error && (result.is_some() || response.is_some()) {
                    file = Some(EditedFile {
                        path,
                        added: counts.map(|c| c.0 as u32),
                        removed: counts.map(|c| c.1 as u32),
                        created,
                    });
                }
                if !error && created {
                    detail = content.map(String::from);
                    created_summary(&quoted, content.map(str::lines).map(Iterator::count).unwrap_or(0))
                } else if !error && let Some((added, removed)) = counts {
                    detail = patch.map(String::from);
                    edited_summary(&quoted, added, removed)
                } else {
                    format!("Edited {quoted}")
                }
            }
            None => format!("Used {}", cap(name, SUMMARY_CAP)),
        },
        "read_file" => path
            .map(|p| format!("Read {}", cap(&p, SUMMARY_CAP)))
            .unwrap_or_else(|| format!("Used {}", cap(name, SUMMARY_CAP))),
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

    const BASIC: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000002.jsonl");
    const REFUSED: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000026.jsonl");
    const REPO: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000046.jsonl");
    const SHELL: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000059.jsonl");
    const EDIT: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000079.jsonl");
    const FAILED: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000107.jsonl");
    const WRITE: &str =
        include_str!("../../../tests/fixtures/qwen/transcripts/0.26.5/00000000-0000-4000-8000-000000000131.jsonl");
    const SID: &str = "00000000-0000-4000-8000-000000000002";

    fn records(text: &str) -> Vec<Value> {
        text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn jsonl(records: &[Value]) -> String {
        records.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
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

    // The envelope and payload fields come from chunk-XBXDDFWT.js in Qwen's installed bundle.
    fn system_record(subtype: &str, payload: Value) -> Value {
        json!({
            "uuid": "custom-system-record", "parentUuid": "previous-record", "sessionId": SID,
            "timestamp": "2026-11-03T17:02:00.000Z", "type": "system", "subtype": subtype,
            "cwd": "C:/work/demo", "version": "0.26.5", "systemPayload": payload,
        })
    }

    #[test]
    fn a_text_reply_is_one_turn() {
        let all = records(BASIC);
        let resumed = all
            .iter()
            .position(|v| v["type"] == "user" && v["message"]["parts"][0]["text"] == "hello again")
            .unwrap();
        let conv = parse(&jsonl(&all[..resumed]), SID);
        assert_eq!(conv.agent, "qwen");
        assert_eq!(conv.session_id, SID);
        assert!(conv.tab_id.is_empty());
        assert_eq!(conv.turns.len(), 1); // memory_catalog_snapshot must not become a turn.
        assert_eq!(conv.turns[0].index, 0);
        assert_eq!(conv.turns[0].prompt, "hello");
        assert_eq!(conv.turns[0].at_ms, 1_793_725_138_724);
        assert_eq!(
            conv.turns[0].items,
            vec![TurnItem::Text {
                markdown: "done".into()
            }]
        );
        let conv = parse(BASIC, SID);
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[1].index, 1);
        assert_eq!(conv.turns[1].prompt, "hello again");
        assert_eq!(
            conv.turns[1].items,
            vec![TurnItem::Text {
                markdown: "done".into()
            }]
        );
        assert_eq!(crate::history::parse_for("qwen", BASIC, SID), Some(conv));
    }

    #[test]
    fn each_tool_becomes_a_card() {
        let shell = parse(SHELL, SID);
        assert_eq!(shell.turns.len(), 1);
        let shell = cards(&shell);
        assert_eq!(shell.len(), 1);
        assert_eq!(shell[0].name, "run_shell_command");
        assert_eq!(shell[0].summary, "Ran mkdir made");
        assert!(shell[0].detail.as_deref().unwrap().contains("mkdir made"));
        assert!(!shell[0].error);
        assert!(shell[0].file.is_none());

        let edit = parse(EDIT, SID);
        assert_eq!(edit.turns.len(), 1);
        let edit = cards(&edit);
        assert_eq!(edit.len(), 2);
        assert_eq!(edit[0].name, "read_file");
        assert_eq!(edit[0].summary, "Read notes.txt");
        assert!(edit[0].file.is_none());
        assert_eq!(edit[1].name, "edit");
        assert_eq!(edit[1].summary, "Edited notes.txt, 1 added, 1 removed");
        assert_eq!(
            edit[1].file,
            Some(EditedFile {
                path: "notes.txt".into(),
                added: Some(1),
                removed: Some(1),
                created: false,
            })
        );
        assert!(
            edit[1]
                .detail
                .as_deref()
                .unwrap()
                .contains("-marshell\n+marshell edited")
        );
        assert!(!edit[1].error);

        let write = parse(WRITE, SID);
        let write = cards(&write);
        assert_eq!(write.len(), 1);
        assert_eq!(write[0].name, "write_file");
        assert_eq!(write[0].summary, "Created notes.txt, 1 line");
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

        // The equivalent user/functionResponse shape must resolve the same pending calls.
        let mut alternate = records(EDIT);
        for record in &mut alternate {
            if record["type"] == "tool_result" {
                record["type"] = json!("user");
            }
        }
        assert_eq!(parse(&jsonl(&alternate), SID), parse(EDIT, SID));
    }

    #[test]
    fn a_failed_tool_is_an_error_card() {
        let conv = parse(FAILED, SID);
        assert_eq!(conv.turns.len(), 1);
        let failed = cards(&conv);
        assert_eq!(failed.len(), 1);
        assert!(failed[0].error);
        assert_eq!(failed[0].summary, "Ran exit 3, failed");
        assert!(failed[0].detail.as_deref().unwrap().contains("Exit Code: 3"));
        assert!(failed[0].file.is_none());
        let conv = parse(REFUSED, SID);
        let refused = cards(&conv);
        assert_eq!(refused.len(), 1);
        assert!(refused[0].error);
        assert!(refused[0].file.is_none());
        assert!(
            refused[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("File path must be absolute")
        );
    }

    #[test]
    fn the_title_is_the_last_custom_title_else_the_first_prompt() {
        assert_eq!(parse(BASIC, SID).title, "hello");
        let mut all = records(BASIC);
        all.push(system_record(
            "custom_title",
            json!({"customTitle": "Automatic title", "titleSource": "auto"}),
        ));
        all.push(system_record(
            "custom_title",
            json!({"customTitle": "My title", "titleSource": "manual"}),
        ));
        assert_eq!(parse(&jsonl(&all), SID).title, "My title");
        all.push(system_record(
            "custom_title",
            json!({"customTitle": "Last automatic title", "titleSource": "auto"}),
        ));
        assert_eq!(parse(&jsonl(&all), SID).title, "Last automatic title");
        let mut first = records(BASIC).remove(0);
        first["message"]["parts"][0]["text"] = json!("x".repeat(201));
        assert_eq!(parse(&jsonl(&[first]), SID).title, format!("{}…", "x".repeat(200)));
    }

    #[test]
    fn a_chat_compression_record_is_a_compacted_item() {
        let mut all = records(BASIC);
        all.push(system_record(
            "chat_compression",
            json!({
                "compressedHistory": [{"role": "user", "parts": [{"text": "Summary"}]}],
                "contextPrefixLength": 0, "summaryPrefixLength": 1,
            }),
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
    }

    #[test]
    fn a_truncated_last_line_is_ignored() {
        assert_eq!(
            parse(&format!("{BASIC}\n{{\"type\":\"assistant\""), SID),
            parse(BASIC, SID)
        );
        assert_eq!(meta(&format!("{BASIC}\n{{\"sessionId\":")), meta(BASIC));
        assert!(parse("\nnot json\n{", SID).turns.is_empty());
        assert!(meta("\nnot json\n{").is_none());
    }

    #[test]
    fn model_and_version_come_from_the_records() {
        let conv = parse(REPO, SID);
        assert_eq!(conv.model.as_deref(), Some("marshell-fake"));
        assert_eq!(conv.version.as_deref(), Some("0.26.5"));
        assert_eq!(conv.branch.as_deref(), Some("feature-x"));
        let mut all = records(BASIC);
        let assistant = all.iter_mut().rev().find(|v| v["type"] == "assistant").unwrap();
        assistant["model"] = json!("another-model");
        assistant["version"] = json!("0.26.9");
        // Qwen appends system records after the reply, carrying the version too.
        all.last_mut().unwrap()["version"] = json!("0.26.9");
        let conv = parse(&jsonl(&all), SID);
        assert_eq!(conv.model.as_deref(), Some("another-model"));
        assert_eq!(conv.version.as_deref(), Some("0.26.9"));
    }

    #[test]
    fn limited_follows_the_version_gate() {
        for version in [
            None,
            Some(""),
            Some("garbage"),
            Some("0.26.4"),
            Some("0.26.5"),
            Some("0.26.9"),
            Some("0.27.0"),
        ] {
            let mut all = records(BASIC);
            for record in &mut all {
                record["version"] = json!(version);
            }
            assert_eq!(
                parse(&jsonl(&all), SID).limited,
                super::super::version_gate(version) != super::super::Gate::Known,
                "{version:?}"
            );
        }
        let mut mixed = records(BASIC);
        mixed[0]["version"] = json!("0.27.0");
        assert!(parse(&jsonl(&mixed), SID).limited);
    }

    #[test]
    fn meta_reads_id_cwd_branch_and_start() {
        let meta = meta(REPO).unwrap();
        assert_eq!(meta.id, "00000000-0000-4000-8000-000000000046");
        assert_eq!(meta.started_ms, 1_793_725_283_288);
        assert_eq!(meta.cwd, r"C:\Users\user\AppData\Local\Temp\qwen-6l2-t1\repo");
        assert_eq!(meta.branch.as_deref(), Some("feature-x"));
        assert_eq!(meta.version.as_deref(), Some("0.26.5"));
        assert_eq!(meta.client, Client::Terminal); // Headless and interactive are indistinguishable.
        let mut all = records(REPO);
        all[1]["cwd"] = json!("C:/later");
        let text = format!("not json\n{{\"type\":\"system\"}}\n{}", jsonl(&all));
        assert_eq!(super::meta(&text).unwrap(), meta);
        assert!(super::meta(BASIC).unwrap().branch.is_none());
    }
}
