//! Codex CLI's rollout files, read into a common [`Conversation`]. The format is internal to Codex and
//! changes between versions, so parsing is best effort: what isn't understood is skipped, and a conversation
//! from an untested version is flagged `limited`. Only `session_meta`, `turn_context` and `event_msg` lines
//! are read; the model-level `response_item` lines are ignored. The shapes are in
//! `docs/research/2026-10-24-codex-rollouts.md`.

use super::super::common::{DETAIL_CAP, cap, created_summary, edited_summary, relative, rfc3339_ms};
use marshell_protocol::history::{Client, Conversation, EditedFile, ToolCard, Turn, TurnItem};
use serde_json::Value;

/// The oldest and newest Codex CLI versions whose rollouts were surveyed (inclusive). A pre-release suffix
/// such as `-alpha.17.2` is ignored when comparing.
pub const TESTED: (&str, &str) = ("0.162.0", "0.163.0");

const TEXT_CAP: usize = 100_000;
const PROMPT_CAP: usize = 20_000;
const TITLE_CAP: usize = 80;
/// Cap on a name, path or query quoted in a tool summary.
const SUMMARY_CAP: usize = 200;

const QUESTION_REPLY: &str = "send_user_message_question_reply";
const BROWSER_CONTEXT: &str = "in-app-browser-context";

/// What a rollout's first `session_meta` line says about the thread. Account identifiers are never read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutMeta {
    pub id: String,
    /// When the thread started, in ms since the Unix epoch; 0 when the line has no usable time.
    pub started_ms: u64,
    pub cwd: String,
    pub branch: Option<String>,
    pub cli_version: Option<String>,
    pub client: Client,
    /// A thread the person started, as opposed to a sub-agent or an auto-review thread.
    pub listed: bool,
}

/// The meta of a rollout, from its first line. `None` if that isn't a `session_meta` line with an id.
pub fn meta(first_line: &str) -> Option<RolloutMeta> {
    meta_of(&serde_json::from_str::<Value>(first_line).ok()?)
}

fn meta_of(line: &Value) -> Option<RolloutMeta> {
    if str_at(line, "type")? != "session_meta" {
        return None;
    }
    let p = line.get("payload")?;
    let id = str_at(p, "id").filter(|s| !s.is_empty())?.to_string();
    let started_ms = str_at(p, "timestamp")
        .or_else(|| str_at(line, "timestamp"))
        .and_then(rfc3339_ms)
        .unwrap_or(0);
    let non_empty = |v: Option<&str>| v.filter(|s| !s.is_empty()).map(String::from);
    Some(RolloutMeta {
        id,
        started_ms,
        cwd: str_at(p, "cwd").unwrap_or_default().to_string(),
        branch: non_empty(p.get("git").and_then(|g| str_at(g, "branch"))),
        cli_version: non_empty(str_at(p, "cli_version")),
        client: match str_at(p, "originator") {
            Some("codex_exec") => Client::Headless,
            Some("Codex Desktop" | "codex_work_desktop") => Client::Desktop,
            _ => Client::Terminal,
        },
        // Only the known child threads are left out: a source this version doesn't name is shown, not lost.
        listed: !matches!(str_at(p, "thread_source"), Some("subagent" | "guardian_review"))
            && p.get("source").and_then(|s| s.get("subagent")).is_none(),
    })
}

/// A turn being read: from its `task_started` until its `task_complete`.
struct Open {
    turn_id: String,
    at_ms: u64,
    prompt: Option<String>,
    items: Vec<TurnItem>,
}

/// Closes a turn. One with no prompt and no items is dropped.
fn finish(turns: &mut Vec<Turn>, open: Open) {
    if open.prompt.is_none() && open.items.is_empty() {
        return;
    }
    turns.push(Turn {
        index: turns.len() as u32,
        at_ms: open.at_ms,
        prompt: open.prompt.unwrap_or_default(),
        items: open.items,
    });
}

/// Reads a rollout's text. Never fails: lines that don't parse (a half-written last line among them), and
/// things not understood, are skipped.
pub fn parse(text: &str, session_id: &str) -> Conversation {
    let mut turns: Vec<Turn> = Vec::new();
    let mut open: Option<Open> = None;
    let mut meta: Option<RolloutMeta> = None;
    let mut meta_seen = false;
    let mut cwd = String::new();
    let mut model = None;

    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(payload) = v.get("payload") else {
            continue;
        };
        match str_at(&v, "type") {
            // Identity comes from the first session_meta only: a fork's later one is the source thread's.
            Some("session_meta") if !meta_seen => {
                meta_seen = true;
                meta = meta_of(&v);
                if let Some(m) = &meta {
                    cwd = m.cwd.clone();
                }
            }
            Some("turn_context") => {
                if let Some(m) = str_at(payload, "model").filter(|s| !s.is_empty()) {
                    model = Some(m.to_string());
                }
                if cwd.is_empty() {
                    cwd = str_at(payload, "cwd").unwrap_or_default().to_string();
                }
            }
            Some("event_msg") => match str_at(payload, "type") {
                Some("task_started") => {
                    if let Some(previous) = open.take() {
                        finish(&mut turns, previous);
                    }
                    let at_ms = payload
                        .get("started_at")
                        .and_then(Value::as_u64)
                        .map(|s| s.saturating_mul(1_000))
                        .or_else(|| str_at(&v, "timestamp").and_then(rfc3339_ms))
                        .unwrap_or(0);
                    open = Some(Open {
                        turn_id: str_at(payload, "turn_id").unwrap_or_default().to_string(),
                        at_ms,
                        prompt: None,
                        items: Vec::new(),
                    });
                }
                Some("item_completed") => {
                    if let (Some(turn), Some(item)) = (open.as_mut(), payload.get("item")) {
                        add_item(turn, item, &cwd);
                    }
                }
                Some("task_complete") => {
                    let id = str_at(payload, "turn_id").unwrap_or_default();
                    if open.as_ref().is_some_and(|o| o.turn_id == id)
                        && let Some(mut turn) = open.take()
                    {
                        let message = payload.get("error").and_then(|e| match e {
                            Value::String(s) => Some(s.as_str()),
                            _ => str_at(e, "message"),
                        });
                        if let Some(m) = message.filter(|m| !m.trim().is_empty()) {
                            turn.items.push(TurnItem::Text {
                                markdown: cap(m, TEXT_CAP),
                            });
                        }
                        finish(&mut turns, turn);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    // A turn still open at the end of the file (the CLI was closed, or the turn is running) is kept.
    if let Some(turn) = open.take() {
        finish(&mut turns, turn);
    }

    let version = meta.as_ref().and_then(|m| m.cli_version.clone());
    let title = {
        let first = turns
            .first()
            .map(|t| t.prompt.trim().lines().next().unwrap_or_default());
        cap(first.unwrap_or_default(), TITLE_CAP)
    };
    Conversation {
        agent: "codex".into(),
        session_id: session_id.into(),
        tab_id: String::new(),
        title,
        cwd,
        branch: meta.as_ref().and_then(|m| m.branch.clone()),
        model,
        limited: version.as_deref().is_none_or(is_untested),
        version,
        turns,
        marks: Default::default(),
    }
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

/// Adds what one `item_completed` item shows to the open turn.
fn add_item(turn: &mut Open, item: &Value, cwd: &str) {
    match str_at(item, "type") {
        Some("UserMessage") => {
            if turn.prompt.is_none() {
                turn.prompt = Some(prompt_text(item.get("content")));
            }
        }
        Some("AgentMessage") => {
            let text = text_blocks(item.get("content"), "Text").join("\n\n");
            if !text.trim().is_empty() {
                turn.items.push(TurnItem::Text {
                    markdown: cap(&text, TEXT_CAP),
                });
            }
        }
        Some("CommandExecution") => turn.items.push(TurnItem::Tool {
            card: command_card(item),
        }),
        Some("FileChange") => {
            let failed = str_at(item, "status") == Some("failed");
            if let Some(Value::Object(changes)) = item.get("changes") {
                for (path, change) in changes {
                    if let Some(card) = file_card(path, change, failed, cwd) {
                        turn.items.push(TurnItem::Tool { card });
                    }
                }
            }
        }
        Some("McpToolCall") => {
            let name = [str_at(item, "server"), str_at(item, "tool")]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(".");
            let name = if name.is_empty() {
                "MCP tool".into()
            } else {
                cap(&name, SUMMARY_CAP)
            };
            let error = str_at(item, "status") == Some("failed")
                || item
                    .get("result")
                    .and_then(|r| r.get("isError"))
                    .and_then(Value::as_bool)
                    == Some(true);
            turn.items.push(TurnItem::Tool {
                card: ToolCard {
                    summary: format!("Used {name}"),
                    name,
                    detail: None,
                    file: None,
                    error,
                },
            });
        }
        Some("Extension") if str_at(item, "kind") == Some("web.search") => {
            let action = item.get("action");
            let query = str_at(item, "query")
                .or_else(|| action.and_then(|a| str_at(a, "query")))
                .or_else(|| {
                    action
                        .and_then(|a| a.get("queries")?.as_array()?.first())
                        .and_then(Value::as_str)
                })
                .filter(|q| !q.trim().is_empty());
            turn.items.push(TurnItem::Tool {
                card: ToolCard {
                    name: "Web search".into(),
                    summary: query.map_or_else(|| "Web search".into(), |q| cap(q.trim(), SUMMARY_CAP)),
                    detail: None,
                    file: None,
                    error: false,
                },
            });
        }
        Some("ContextCompaction") => turn.items.push(TurnItem::Compacted),
        _ => {}
    }
}

/// The text of the blocks of `kind` in a message's `content` array (the tag's case is ignored).
fn text_blocks<'a>(content: Option<&'a Value>, kind: &str) -> Vec<&'a str> {
    match content {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| str_at(b, "type").is_some_and(|t| t.eq_ignore_ascii_case(kind)))
            .filter_map(|b| str_at(b, "text"))
            .collect(),
        _ => Vec::new(),
    }
}

/// What the person typed, from a `UserMessage`'s content: its text blocks joined by a line break, without
/// the desktop app's browser context, and a question reply as just the reply.
fn prompt_text(content: Option<&Value>) -> String {
    let joined = text_blocks(content, "text").join("\n");
    let text = remove_block(&joined, BROWSER_CONTEXT);
    let text = text.trim();
    let open = format!("<{QUESTION_REPLY}>");
    let close = format!("</{QUESTION_REPLY}>");
    let text = match text.strip_prefix(&open) {
        Some(inner) => inner.strip_suffix(&close).unwrap_or(inner),
        None => text,
    };
    cap(text.trim(), PROMPT_CAP)
}

/// `text` without every `<tag>…</tag>` block; an unclosed one is cut to the end.
fn remove_block(text: &str, tag: &str) -> String {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let mut out = String::new();
    let mut rest = text;
    while let Some(from) = rest.find(&open) {
        out.push_str(&rest[..from]);
        match rest[from..].find(&close) {
            Some(len) => rest = &rest[from + len + close.len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// A shell command as one card.
fn command_card(item: &Value) -> ToolCard {
    // `command` is the argv: the shell, its flag, then the script.
    let script = match item.get("command") {
        Some(Value::Array(argv)) => argv.last().and_then(Value::as_str),
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
    .filter(|s| !s.trim().is_empty());
    let error = str_at(item, "status") == Some("failed")
        || item
            .get("exit_code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0);
    let summary = match script {
        Some(s) => {
            let first = s.trim().lines().next().unwrap_or_default();
            format!("Ran {}{}", cap(first, 80), if error { ", failed" } else { "" })
        }
        None => "Used Shell".into(),
    };
    let output = str_at(item, "aggregated_output")
        .map(str::trim_end)
        .filter(|o| !o.is_empty());
    ToolCard {
        name: "Shell".into(),
        summary,
        detail: output.map(|o| cap(o, DETAIL_CAP)),
        file: None,
        error,
    }
}

/// The card for one file of a `FileChange`. `None` for a kind of change not understood.
fn file_card(path: &str, change: &Value, failed: bool, cwd: &str) -> Option<ToolCard> {
    let path = relative(path, cwd);
    let shown = cap(&path, SUMMARY_CAP);
    let lines_of = |key: &str| str_at(change, key).unwrap_or_default();
    let (summary, detail, added, removed, created) = match str_at(change, "type")? {
        "add" => {
            let content = lines_of("content");
            let n = content.lines().count();
            (created_summary(&shown, n), content.to_string(), n, 0, true)
        }
        "update" => {
            // The hunk headers go; the lines of the change stay, as the Files changed view reads them.
            let kept: Vec<&str> = lines_of("unified_diff")
                .lines()
                .filter(|l| l.starts_with([' ', '+', '-']))
                .collect();
            let count = |sign: char| kept.iter().filter(|l| l.starts_with(sign)).count();
            let (added, removed) = (count('+'), count('-'));
            (
                edited_summary(&shown, added, removed),
                kept.join("\n"),
                added,
                removed,
                false,
            )
        }
        "delete" => {
            let content = lines_of("content");
            let n = content.lines().count();
            let detail = content.lines().map(|l| format!("-{l}")).collect::<Vec<_>>().join("\n");
            (edited_summary(&shown, 0, n), detail, 0, n, false)
        }
        _ => return None,
    };
    let file = EditedFile {
        path,
        added: Some(added as u32),
        removed: Some(removed as u32),
        created,
    };
    Some(ToolCard {
        name: "apply_patch".into(),
        summary,
        detail: Some(detail).filter(|d| !d.is_empty()).map(|d| cap(&d, DETAIL_CAP)),
        file: (!failed).then_some(file),
        error: failed,
    })
}

/// Whether a Codex CLI version is outside [`TESTED`], or isn't `X.Y.Z` of plain numbers. A pre-release or
/// build suffix (`-alpha.17.2`) is ignored.
fn is_untested(version: &str) -> bool {
    fn parts(v: &str) -> Option<[u64; 3]> {
        let core = v.split(['-', '+']).next().unwrap_or_default();
        let mut it = core.split('.');
        let mut out = [0; 3];
        for slot in &mut out {
            let part = it
                .next()
                .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))?;
            *slot = part.parse().ok()?;
        }
        it.next().is_none().then_some(out)
    }
    match (parts(version), parts(TESTED.0), parts(TESTED.1)) {
        (Some(v), Some(lo), Some(hi)) => v < lo || v > hi,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TUI: &str = include_str!("../../../tests/fixtures/codex/tui.jsonl");
    const EXEC: &str = include_str!("../../../tests/fixtures/codex/exec.jsonl");
    const DESKTOP: &str = include_str!("../../../tests/fixtures/codex/desktop.jsonl");
    const SUBAGENT: &str = include_str!("../../../tests/fixtures/codex/subagent.jsonl");
    const UNKNOWN: &str = include_str!("../../../tests/fixtures/codex/unknown_version.jsonl");
    const SID: &str = "00000000-0000-4000-8000-0000000000aa";

    fn first_line(text: &str) -> &str {
        text.lines().next().unwrap()
    }

    fn cards(turn: &Turn) -> Vec<&ToolCard> {
        turn.items
            .iter()
            .filter_map(|i| match i {
                TurnItem::Tool { card } => Some(card),
                _ => None,
            })
            .collect()
    }

    fn texts(turn: &Turn) -> Vec<&str> {
        turn.items
            .iter()
            .filter_map(|i| match i {
                TurnItem::Text { markdown } => Some(markdown.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reads_the_meta_of_a_tui_session() {
        let m = meta(first_line(TUI)).unwrap();
        assert_eq!(m.id, "00000000-0000-4000-8000-0000000000c1");
        assert_eq!(m.started_ms, 1_792_486_800_000);
        assert_eq!(m.cwd, "C:\\work\\demo");
        assert_eq!(m.branch.as_deref(), Some("feature/demo"));
        assert_eq!(m.cli_version.as_deref(), Some("0.163.0"));
        assert_eq!(m.client, Client::Terminal);
        assert!(m.listed);
    }

    #[test]
    fn an_exec_run_is_headless() {
        let m = meta(first_line(EXEC)).unwrap();
        assert_eq!(m.client, Client::Headless);
        assert!(m.listed);
        assert_eq!(m.cwd, "/work/demo");
    }

    #[test]
    fn a_desktop_thread_is_desktop_app() {
        let m = meta(first_line(DESKTOP)).unwrap();
        assert_eq!(m.client, Client::Desktop);
        assert!(m.listed);
        let line = serde_json::json!({"type": "session_meta", "payload": {
            "id": "x", "originator": "codex_work_desktop", "thread_source": "chatgpt_handoff"}});
        let m = meta(&line.to_string()).unwrap();
        assert_eq!((m.client, m.listed), (Client::Desktop, true));
        let line = serde_json::json!({"type": "session_meta", "payload": {"id": "x", "originator": "something-new"}});
        assert_eq!(meta(&line.to_string()).unwrap().client, Client::Terminal);
    }

    #[test]
    fn a_thread_with_no_thread_source_is_listed() {
        // Every surveyed rollout names its source; one that doesn't is shown rather than lost.
        let line = serde_json::json!({"type": "session_meta", "payload": {"id": "x", "originator": "codex-tui"}});
        assert!(meta(&line.to_string()).unwrap().listed);
        for child in ["subagent", "guardian_review"] {
            let line = serde_json::json!({"type": "session_meta", "payload": {"id": "x", "thread_source": child}});
            assert!(!meta(&line.to_string()).unwrap().listed, "{child}");
        }
        let line = serde_json::json!({"type": "session_meta", "payload": {"id": "x",
            "source": {"subagent": {"thread_spawn": {"parent_thread_id": "p"}}}}});
        assert!(!meta(&line.to_string()).unwrap().listed);
    }

    #[test]
    fn a_subagent_thread_is_not_listed() {
        let m = meta(first_line(SUBAGENT)).unwrap();
        assert!(!m.listed);
        // Identity comes from the first session_meta; the second is the inherited history's.
        assert_eq!(m.id, "00000000-0000-4000-8000-0000000000c4");
        assert_eq!(m.cwd, "C:\\work\\demo");
        assert_eq!(m.client, Client::Desktop);
        assert_eq!(meta("not json"), None);
        assert_eq!(meta("{\"type\":\"event_msg\",\"payload\":{}}"), None);
        assert_eq!(meta(""), None);
    }

    #[test]
    fn turns_take_their_prompt_from_the_user_message() {
        let conv = parse(TUI, SID);
        assert_eq!(conv.agent, "codex");
        assert_eq!(conv.session_id, SID);
        assert_eq!(conv.tab_id, "");
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[0].prompt, "Explain the retry logic in src/app.ts");
        assert_eq!(conv.turns[1].prompt, "Add a limit of five attempts");
        assert_eq!((conv.turns[0].index, conv.turns[1].index), (0, 1));
        assert_eq!(conv.turns[0].at_ms, 1_792_486_801_000);
        assert_eq!(
            texts(&conv.turns[0]),
            [
                "Let me look at the file.",
                "The retry loop waits twice as long after each failure."
            ]
        );
        assert_eq!(
            conv.turns[1].items,
            vec![TurnItem::Text {
                markdown: "Done.\n\nThe loop now stops after five attempts.".into()
            }]
        );
        assert!(matches!(conv.turns[0].items[2], TurnItem::Text { .. }));
        let c = cards(&conv.turns[0]);
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0].name.as_str(), c[0].summary.as_str(), c[0].error),
            ("Shell", "Ran Get-Content src/app.ts", false)
        );
        assert_eq!(c[0].detail.as_deref(), Some("const a = 1;"));
        let json = serde_json::to_string(&conv).unwrap();
        assert!(!json.contains("SECRET-THINKING-TEXT"));
        assert!(!json.contains("RESPONSE-ITEM-TEXT"));
        assert_eq!(conv.title, "Explain the retry logic in src/app.ts");
        assert_eq!(conv.cwd, "C:\\work\\demo");
        assert_eq!(conv.branch.as_deref(), Some("feature/demo"));
        assert_eq!(conv.model.as_deref(), Some("gpt-demo-2"));
        assert_eq!(conv.version.as_deref(), Some("0.163.0"));
        assert!(!conv.limited);
    }

    #[test]
    fn a_question_reply_shows_its_inner_text() {
        let conv = parse(DESKTOP, SID);
        // The bare turn between them has no prompt and no items, so it is dropped; the open last turn stays.
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[0].prompt, "Fix the header");
        assert_eq!(conv.turns[1].prompt, "Yes, go ahead");
        assert_eq!(conv.turns[1].index, 1);
        assert_eq!(texts(&conv.turns[1]), ["Working on it."]);
    }

    #[test]
    fn file_changes_become_edit_cards() {
        let conv = parse(DESKTOP, SID);
        let patches: Vec<&ToolCard> = cards(&conv.turns[0])
            .into_iter()
            .filter(|c| c.name == "apply_patch")
            .collect();
        type Got<'a> = (&'a str, Option<(&'a str, Option<u32>, Option<u32>, bool)>);
        let got: Vec<Got> = patches
            .iter()
            .map(|c| {
                (
                    c.summary.as_str(),
                    c.file
                        .as_ref()
                        .map(|f| (f.path.as_str(), f.added, f.removed, f.created)),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    "Created src/new.ts, 3 lines",
                    Some(("src/new.ts", Some(3), Some(0), true))
                ),
                (
                    "Edited src/app.ts, 3 added, 2 removed",
                    Some(("src/app.ts", Some(3), Some(2), false))
                ),
                (
                    "Edited gone.txt, 0 added, 2 removed",
                    Some(("gone.txt", Some(0), Some(2), false))
                ),
            ]
        );
        assert!(patches.iter().all(|c| !c.error));
        // The cards come in the order the file lists the changes.
        assert_eq!(patches[0].detail.as_deref(), Some("line one\nline two\nline three"));
        assert_eq!(
            patches[1].detail.as_deref(),
            Some(" const a = 1;\n-const b = 2;\n+const b = 3;\n+const c = 4;\n return a;\n keep\n-old\n+new\n tail")
        );
        assert_eq!(patches[2].detail.as_deref(), Some("-x\n-y"));
    }

    #[test]
    fn other_tools_become_cards() {
        let conv = parse(DESKTOP, SID);
        let items = &conv.turns[0].items;
        assert_eq!(items.len(), 8);
        let TurnItem::Tool { card } = &items[3] else {
            panic!("{:?}", items[3])
        };
        assert_eq!(
            (card.name.as_str(), card.summary.as_str(), card.error),
            ("docs.search", "Used docs.search", false)
        );
        let TurnItem::Tool { card } = &items[4] else {
            panic!("{:?}", items[4])
        };
        assert_eq!((card.name.as_str(), card.error), ("docs.fetch", true));
        let TurnItem::Tool { card } = &items[5] else {
            panic!("{:?}", items[5])
        };
        assert_eq!(
            (card.name.as_str(), card.summary.as_str()),
            ("Web search", "css sticky header")
        );
        assert_eq!(items[6], TurnItem::Compacted);
        assert_eq!(
            items[7],
            TurnItem::Text {
                markdown: "The header is fixed.".into()
            }
        );
    }

    #[test]
    fn a_failed_command_is_an_error_card() {
        let conv = parse(EXEC, SID);
        let c = cards(&conv.turns[0]);
        assert_eq!(c.len(), 2);
        assert_eq!(
            (c[0].name.as_str(), c[0].summary.as_str(), c[0].error),
            ("Shell", "Ran cargo test, failed", true)
        );
        assert_eq!(c[0].detail.as_deref(), Some("running 3 tests\nerror: test failed"));
        assert_eq!((c[1].summary.as_str(), c[1].error), ("Ran ls", false));
        // A non-zero exit code alone also fails the card.
        let line = serde_json::json!({"type": "event_msg", "payload": {"type": "item_completed", "turn_id": "t", "item": {
            "type": "CommandExecution", "command": ["sh", "-c", "false"], "status": "completed", "exit_code": 1}}});
        let start = serde_json::json!({"type": "event_msg", "payload": {"type": "task_started", "turn_id": "t"}});
        let conv = parse(&format!("{start}\n{line}"), SID);
        assert!(cards(&conv.turns[0])[0].error);
    }

    #[test]
    fn a_failed_turn_shows_its_message() {
        let conv = parse(EXEC, SID);
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(conv.turns[0].prompt, "Run the tests");
        assert_eq!(texts(&conv.turns[0]), ["You have reached your usage limit."]);
        assert!(matches!(conv.turns[0].items.last(), Some(TurnItem::Text { .. })));
    }

    #[test]
    fn a_half_written_last_line_is_skipped() {
        let cut = &TUI[..TUI.trim_end().len() - 40];
        let conv = parse(cut, SID);
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[1].prompt, "Add a limit of five attempts");
        assert_eq!(conv.turns[1].items.len(), 1);
        let conv = parse(&format!("{}\n{{\"type\":\"event_m", first_line(EXEC)), SID);
        assert!(conv.turns.is_empty());
    }

    #[test]
    fn an_unknown_version_is_limited() {
        let conv = parse(UNKNOWN, SID);
        assert!(conv.limited);
        assert_eq!(conv.version.as_deref(), Some("0.170.0"));
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(conv.turns[0].prompt, "Hello future");
        assert_eq!(texts(&conv.turns[0]), ["Known text survives."]);
        assert_eq!(conv.turns[0].items.len(), 1);
    }

    #[test]
    fn an_alpha_of_a_tested_version_is_not_limited() {
        let conv = parse(EXEC, SID);
        assert_eq!(conv.version.as_deref(), Some("0.162.0-alpha.17.2"));
        assert!(!conv.limited);
        assert!(!is_untested("0.162.0-alpha.17.2"));
        assert!(!is_untested("0.163.0-alpha.10.2"));
        assert!(!is_untested("0.162.1"));
        assert!(is_untested("0.161.9"));
        assert!(is_untested("0.163.1"));
        assert!(is_untested("0.164.0-alpha.1"));
        assert!(is_untested("1.0.0"));
        assert!(is_untested("0.162"));
        assert!(is_untested("0.x.0"));
        assert!(is_untested(""));
    }

    #[test]
    fn a_missing_version_is_limited() {
        let start = serde_json::json!({"type": "event_msg", "payload": {"type": "task_started", "turn_id": "t"}});
        let msg = serde_json::json!({"type": "event_msg", "payload": {"type": "item_completed", "turn_id": "t", "item": {
            "type": "UserMessage", "content": [{"type": "text", "text": "hi"}]}}});
        let conv = parse(&format!("{start}\n{msg}"), SID);
        assert!(conv.limited);
        assert_eq!(conv.version, None);
        assert_eq!(conv.turns[0].prompt, "hi");
    }

    #[test]
    fn a_command_detail_is_capped() {
        let start = serde_json::json!({"type": "event_msg", "payload": {"type": "task_started", "turn_id": "t"}});
        let cmd = serde_json::json!({"type": "event_msg", "payload": {"type": "item_completed", "turn_id": "t", "item": {
            "type": "CommandExecution", "command": ["sh", "-c", "run"], "status": "completed", "exit_code": 0,
            "aggregated_output": "o".repeat(10_000)}}});
        let conv = parse(&format!("{start}\n{cmd}"), SID);
        let detail = cards(&conv.turns[0])[0].detail.clone().unwrap();
        assert_eq!(detail.chars().count(), 4_001);
        assert!(detail.ends_with('…'));
    }

    #[test]
    fn odd_input_never_fails() {
        for text in [
            "",
            "\n\n",
            "[]",
            "null",
            "{\"type\":\"event_msg\"}",
            "{\"type\":\"event_msg\",\"payload\":5}",
            "\u{feff}{}",
        ] {
            let conv: Conversation = parse(text, SID);
            assert!(conv.turns.is_empty(), "{text:?}");
            assert!(conv.limited);
        }
    }
}
