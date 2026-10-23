//! Claude Code's transcript format, read into a common [`Conversation`]. The format is internal to Claude
//! Code and changes between versions, so parsing is best effort: what isn't understood is skipped, and a
//! conversation from an untested version is flagged `limited`.

use marshell_protocol::history::{Conversation, ToolCard, Turn, TurnItem};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// The oldest and newest Claude Code versions whose transcripts were surveyed (inclusive).
pub const TESTED: (&str, &str) = ("2.1.278", "2.1.309");

const TEXT_CAP: usize = 100_000;
const DETAIL_CAP: usize = 4_000;
const PROMPT_CAP: usize = 20_000;
const TITLE_CAP: usize = 80;
/// Cap on a name, path, pattern or URL quoted in a tool summary.
const SUMMARY_CAP: usize = 200;

/// User strings that start with one of these tags are Claude Code's own, not a prompt.
const NOT_PROMPTS: [&str; 6] = [
    "task-notification",
    "local-command-caveat",
    "local-command-stdout",
    "bash-stdout",
    "command-message",
    "system-reminder",
];

/// A tool call waiting for its result.
struct Pending {
    turn: usize,
    item: usize,
    name: String,
    input: Value,
}

/// Reads a transcript's text. Never fails: lines that don't parse, and things not understood, are skipped.
pub fn parse(text: &str, session_id: &str) -> Conversation {
    let cwd = last_cwd(text);
    let mut turns: Vec<Turn> = Vec::new();
    let mut pending: HashMap<String, Pending> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    // The message id of the Text item that the next assistant text block of that message joins.
    let mut open_text: Option<String> = None;
    let (mut version, mut untested) = (None, false);
    let (mut branch, mut model) = (None, None);
    let (mut custom_title, mut ai_title) = (None, None);

    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if let Some(ver) = str_at(&v, "version").filter(|s| !s.is_empty()) {
            untested |= is_untested(ver);
            version = Some(ver.to_string());
        }
        match str_at(&v, "type") {
            Some("user") => {
                if v.get("isMeta").and_then(Value::as_bool) == Some(true)
                    || v.get("isCompactSummary").and_then(Value::as_bool) == Some(true)
                {
                    continue;
                }
                let content = v.get("message").and_then(|m| m.get("content"));
                let results = tool_results(content);
                if results.is_empty() {
                    if let Some(prompt) = content.and_then(prompt_text) {
                        let at_ms = str_at(&v, "timestamp").and_then(rfc3339_ms).unwrap_or(0);
                        turns.push(Turn {
                            index: turns.len() as u32,
                            at_ms,
                            prompt: cap(&prompt, PROMPT_CAP),
                            items: Vec::new(),
                        });
                        open_text = None;
                    }
                    continue;
                }
                // A line's toolUseResult belongs to its tool result only when there is just one.
                let structured = if results.len() == 1 {
                    v.get("toolUseResult")
                } else {
                    None
                };
                for block in results {
                    let Some(call) = str_at(block, "tool_use_id").and_then(|id| pending.remove(id)) else {
                        continue;
                    };
                    let output = block.get("content").map(block_text).unwrap_or_default();
                    let failed = block.get("is_error").and_then(Value::as_bool) == Some(true);
                    let Some(item) = turns.get_mut(call.turn).and_then(|t| t.items.get_mut(call.item)) else {
                        continue;
                    };
                    match item {
                        TurnItem::Tool { card } => {
                            *card = tool_card(&call.name, &call.input, Some(&output), structured, failed, &cwd)
                        }
                        TurnItem::Subagent { result, .. } => {
                            *result = Some(cap(&output, TEXT_CAP)).filter(|r| !r.trim().is_empty());
                        }
                        _ => {}
                    }
                }
            }
            Some("assistant") => {
                if v.get("isApiErrorMessage").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if let Some(uuid) = str_at(&v, "uuid")
                    && !seen.insert(uuid.to_string())
                {
                    continue;
                }
                let message = v.get("message");
                if let Some(b) = str_at(&v, "gitBranch").filter(|s| !s.is_empty()) {
                    branch = Some(b.to_string());
                }
                if let Some(m) = message.and_then(|m| str_at(m, "model")).filter(|s| !s.is_empty()) {
                    model = Some(m.to_string());
                }
                let Some(turn_at) = turns.len().checked_sub(1) else {
                    continue;
                };
                let message_id = message.and_then(|m| str_at(m, "id"));
                let blocks = match message.and_then(|m| m.get("content")) {
                    Some(Value::Array(blocks)) => blocks.as_slice(),
                    _ => &[],
                };
                for block in blocks {
                    match str_at(block, "type") {
                        Some("text") => {
                            let Some(t) = str_at(block, "text").filter(|t| !t.trim().is_empty()) else {
                                continue;
                            };
                            let items = &mut turns[turn_at].items;
                            match (items.last_mut(), message_id) {
                                (Some(TurnItem::Text { markdown }), Some(id)) if open_text.as_deref() == Some(id) => {
                                    markdown.push_str("\n\n");
                                    markdown.push_str(t);
                                    *markdown = cap(markdown, TEXT_CAP);
                                }
                                _ => {
                                    items.push(TurnItem::Text {
                                        markdown: cap(t, TEXT_CAP),
                                    });
                                    open_text = message_id.map(String::from);
                                }
                            }
                        }
                        Some("tool_use") => {
                            let name = str_at(block, "name").unwrap_or_default();
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let items = &mut turns[turn_at].items;
                            let item = if matches!(name, "Agent" | "Task") {
                                TurnItem::Subagent {
                                    description: cap(str_at(&input, "description").unwrap_or_default(), SUMMARY_CAP),
                                    prompt: cap(str_at(&input, "prompt").unwrap_or_default(), PROMPT_CAP),
                                    result: None,
                                }
                            } else {
                                TurnItem::Tool {
                                    card: tool_card(name, &input, None, None, false, &cwd),
                                }
                            };
                            if let Some(id) = str_at(block, "id") {
                                let call = Pending {
                                    turn: turn_at,
                                    item: items.len(),
                                    name: name.to_string(),
                                    input,
                                };
                                pending.insert(id.to_string(), call);
                            }
                            items.push(item);
                            open_text = None;
                        }
                        _ => {}
                    }
                }
            }
            Some("system") => {
                if str_at(&v, "subtype") == Some("compact_boundary")
                    && let Some(turn) = turns.last_mut()
                {
                    turn.items.push(TurnItem::Compacted);
                    open_text = None;
                }
            }
            Some("custom-title") => custom_title = title_at(&v, "customTitle").or(custom_title),
            Some("ai-title") => ai_title = title_at(&v, "aiTitle").or(ai_title),
            _ => {}
        }
    }

    let title = custom_title.or(ai_title).unwrap_or_else(|| {
        let first = turns
            .first()
            .map(|t| t.prompt.trim().lines().next().unwrap_or_default());
        cap(first.unwrap_or_default(), TITLE_CAP)
    });
    Conversation {
        agent: "claude".into(),
        session_id: session_id.into(),
        tab_id: String::new(),
        title,
        cwd,
        branch,
        model,
        limited: version.is_none() || untested,
        version,
        turns,
    }
}

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn title_at(v: &Value, key: &str) -> Option<String> {
    Some(str_at(v, key)?.trim().to_string()).filter(|t| !t.is_empty())
}

/// `s` cut to `max` characters, with `…` after the cut.
fn cap(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_string(),
    }
}

/// The `cwd` of the last line that has one, searched from the end.
fn last_cwd(text: &str) -> String {
    text.lines()
        .rev()
        .find_map(|line| {
            let v = serde_json::from_str::<Value>(line).ok()?;
            if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            Some(str_at(&v, "cwd")?.to_string()).filter(|c| !c.is_empty())
        })
        .unwrap_or_default()
}

/// The `tool_result` blocks of a user message's content.
fn tool_results(content: Option<&Value>) -> Vec<&Value> {
    match content {
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|b| str_at(b, "type") == Some("tool_result"))
            .collect(),
        _ => Vec::new(),
    }
}

/// The text of a tool result's `content`: a string, or the text blocks of an array.
fn block_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| str_at(b, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// What the person typed, from a user message's content. `None` for what Claude Code wrote itself.
fn prompt_text(content: &Value) -> Option<String> {
    let raw = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| match str_at(b, "type") {
                Some("text") => str_at(b, "text"),
                Some("image") => Some("[image]"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => return None,
    };
    let text = raw.trim();
    // Claude Code's own note that the person pressed Esc, written as a user line.
    if text.starts_with("[Request interrupted by user") && text.ends_with(']') {
        return None;
    }
    let prompt = match text
        .strip_prefix('<')
        .map(|rest| rest.split(['>', ' ']).next().unwrap_or_default())
    {
        Some("command-name") => {
            let name = between(text, "command-name")?;
            let args = between(text, "command-args").unwrap_or_default();
            let name = name.trim_start_matches('/');
            format!("/{name} {args}").trim_end().to_string()
        }
        Some("bash-input") => format!("! {}", between(text, "bash-input")?),
        Some(tag) if NOT_PROMPTS.contains(&tag) => return None,
        _ => text.to_string(),
    };
    (!prompt.is_empty()).then_some(prompt)
}

/// The trimmed text between `<tag>` and `</tag>`, if both are there and it isn't empty.
fn between<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let from = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let len = text[from..].find(&format!("</{tag}>"))?;
    Some(text[from..from + len].trim()).filter(|s| !s.is_empty())
}

/// A tool call as one line, with its detail. `output` is the result's text and `result` its
/// `toolUseResult`; both are `None` until the result arrives.
fn tool_card(
    name: &str,
    input: &Value,
    output: Option<&str>,
    result: Option<&Value>,
    failed: bool,
    cwd: &str,
) -> ToolCard {
    let name = cap(name, SUMMARY_CAP);
    let quote = |key: &str| {
        str_at(input, key)
            .filter(|s| !s.is_empty())
            .map(|s| cap(s, SUMMARY_CAP))
    };
    let path = |key: &str| quote(key).map(|p| relative(&p, cwd));
    let mut detail = None;
    let mut error = failed;
    let summary = match name.as_str() {
        "Edit" | "Write" => match path("file_path") {
            Some(p) => {
                let patch = result.map(patch_lines).unwrap_or_default();
                let created = result.and_then(|r| str_at(r, "type")) == Some("create");
                if name == "Write" && created {
                    let content = result
                        .and_then(|r| str_at(r, "content"))
                        .or_else(|| str_at(input, "content"));
                    let content = content.unwrap_or_default();
                    detail = Some(content.to_string()).filter(|c| !c.is_empty());
                    let n = content.lines().count();
                    format!("Created {p}, {n} {}", if n == 1 { "line" } else { "lines" })
                } else if result.is_some_and(|r| r.get("structuredPatch").is_some()) {
                    let count = |sign: char| patch.iter().filter(|l| l.starts_with(sign)).count();
                    detail = Some(patch.join("\n")).filter(|d| !d.is_empty());
                    format!("Edited {p}, {} added, {} removed", count('+'), count('-'))
                } else {
                    format!("Edited {p}")
                }
            }
            None => format!("Used {name}"),
        },
        "Bash" => match str_at(input, "command").filter(|c| !c.trim().is_empty()) {
            Some(command) => {
                let interrupted = result.and_then(|r| r.get("interrupted")).and_then(Value::as_bool) == Some(true);
                error |= interrupted;
                let streams: Vec<&str> = ["stdout", "stderr"]
                    .iter()
                    .filter_map(|k| result.and_then(|r| str_at(r, k)))
                    .map(str::trim_end)
                    .filter(|s| !s.is_empty())
                    .collect();
                let streams = if result.is_some_and(|r| r.get("stdout").is_some() || r.get("stderr").is_some()) {
                    streams.join("\n")
                } else {
                    output.unwrap_or_default().trim_end().to_string()
                };
                detail = Some(match streams.is_empty() {
                    true => command.to_string(),
                    false => format!("{command}\n\n{streams}"),
                });
                let first = command.trim().lines().next().unwrap_or_default();
                format!("Ran {}{}", cap(first, 80), if error { ", failed" } else { "" })
            }
            None => format!("Used {name}"),
        },
        "Read" => path("file_path")
            .map(|p| format!("Read {p}"))
            .unwrap_or_else(|| format!("Used {name}")),
        "Grep" => quote("pattern")
            .map(|p| format!("Searched for {p}"))
            .unwrap_or_else(|| format!("Used {name}")),
        "Glob" => quote("pattern")
            .map(|p| format!("Listed {p}"))
            .unwrap_or_else(|| format!("Used {name}")),
        "WebFetch" => quote("url")
            .map(|u| format!("Fetched {u}"))
            .unwrap_or_else(|| format!("Used {name}")),
        "WebSearch" => quote("query")
            .map(|q| format!("Searched the web for {q}"))
            .unwrap_or_else(|| format!("Used {name}")),
        _ => format!("Used {name}"),
    };
    ToolCard {
        name,
        summary,
        detail: detail.map(|d| cap(&d, DETAIL_CAP)),
        error,
    }
}

/// The lines of a `toolUseResult`'s `structuredPatch` that start with a space, `+` or `-`.
fn patch_lines(result: &Value) -> Vec<&str> {
    let Some(Value::Array(hunks)) = result.get("structuredPatch") else {
        return Vec::new();
    };
    hunks
        .iter()
        .filter_map(|h| h.get("lines")?.as_array())
        .flatten()
        .filter_map(Value::as_str)
        .filter(|l| l.starts_with([' ', '+', '-']))
        .collect()
}

/// `path` relative to `cwd` (with `/` separators) when it is inside it, else as given.
fn relative(path: &str, cwd: &str) -> String {
    let p = path.replace('\\', "/");
    let c = cwd.replace('\\', "/");
    let c = c.trim_end_matches('/');
    let inside = !c.is_empty()
        && p.len() > c.len()
        && p.is_char_boundary(c.len())
        && p[..c.len()].eq_ignore_ascii_case(c)
        && p.as_bytes()[c.len()] == b'/';
    if inside {
        p[c.len() + 1..].to_string()
    } else {
        path.to_string()
    }
}

/// Whether a Claude Code version is outside [`TESTED`], or isn't `X.Y.Z` of plain numbers.
fn is_untested(version: &str) -> bool {
    fn parts(v: &str) -> Option<[u64; 3]> {
        let mut it = v.split('.');
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

/// An RFC 3339 timestamp (`2026-10-20T09:00:00.123Z`) as ms since the Unix epoch. `None` if it doesn't
/// read, or is before 1970.
fn rfc3339_ms(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let digits = s.get(from..to)?;
        if digits.bytes().all(|c| c.is_ascii_digit()) {
            digits.parse().ok()
        } else {
            None
        }
    };
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=month_days).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = s.get(19..)?;
    let mut ms = 0;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits = frac.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let head = &frac[..digits.min(3)];
        ms = head.parse::<i64>().ok()? * 10_i64.pow(3 - head.len() as u32);
        rest = &frac[digits..];
    }
    let offset_minutes = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (h, m) = (
                rest.get(1..3)?.parse::<i64>().ok()?,
                rest.get(4..6)?.parse::<i64>().ok()?,
            );
            if rest.len() != 6 || rest.as_bytes()[3] != b':' || h > 23 || m > 59 {
                return None;
            }
            sign * (h * 60 + m)
        }
    };
    // Days since 1970-01-01 from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    let secs = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    u64::try_from(secs).ok().map(|s| s * 1_000 + ms as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC: &str = include_str!("../../../tests/fixtures/claude/transcripts/2.1.309/basic.jsonl");
    const TOOLS: &str = include_str!("../../../tests/fixtures/claude/transcripts/2.1.309/tools.jsonl");
    const SUBAGENT: &str = include_str!("../../../tests/fixtures/claude/transcripts/2.1.309/subagent-compact.jsonl");
    const COMMANDS: &str = include_str!("../../../tests/fixtures/claude/transcripts/2.1.309/commands.jsonl");
    const FUTURE: &str = include_str!("../../../tests/fixtures/claude/transcripts/9.9.9/future.jsonl");
    const SID: &str = "00000000-0000-4000-8000-0000000000aa";

    fn cards(conv: &Conversation) -> Vec<&ToolCard> {
        conv.turns
            .iter()
            .flat_map(|t| &t.items)
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

    /// A transcript of one prompt ("go") and then the given lines, each a JSON value.
    fn transcript(version: Option<&str>, lines: &[serde_json::Value]) -> String {
        let mut all = vec![serde_json::json!({
            "type": "user", "uuid": "00000000-0000-4000-8000-000000000001", "cwd": "C:/work/demo",
            "version": version, "timestamp": "2026-10-20T09:00:00.000Z",
            "message": {"role": "user", "content": "go"}
        })];
        all.extend(lines.iter().cloned());
        all.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")
    }

    fn assistant(n: u32, block: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "type": "assistant", "uuid": format!("00000000-0000-4000-8000-{n:012}"), "cwd": "C:/work/demo",
            "version": "2.1.309", "timestamp": "2026-10-20T09:00:01.000Z",
            "message": {"id": format!("msg_{n}"), "model": "claude-sonnet-5-5", "content": [block]}
        })
    }

    #[test]
    fn basic_turns_titles_and_merged_text() {
        let conv = parse(BASIC, SID);
        assert_eq!(conv.agent, "claude");
        assert_eq!(conv.session_id, SID);
        assert_eq!(conv.turns.len(), 2);
        assert_eq!(conv.turns[0].prompt, "Explain the retry logic in src/app.ts");
        assert_eq!(conv.turns[1].prompt, "Add a limit of five attempts");
        assert_eq!(conv.turns[0].index, 0);
        assert_eq!(conv.turns[1].index, 1);
        assert_eq!(conv.turns[0].at_ms, 1_792_486_803_000);
        assert_eq!(
            conv.turns[0].items,
            vec![TurnItem::Text {
                markdown: "The retry loop lives in `src/app.ts`.\n\nIt waits twice as long after each failure.".into()
            }]
        );
        assert_eq!(texts(&conv.turns[1]), ["Done, the loop now stops after five attempts."]);
        assert!(!serde_json::to_string(&conv).unwrap().contains("SECRET-THINKING-TEXT"));
        assert_eq!(conv.title, "Retry limit work");
        assert_eq!(conv.model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(conv.branch.as_deref(), Some("feature/demo"));
        assert_eq!(conv.version.as_deref(), Some("2.1.309"));
        assert_eq!(conv.cwd, "C:/work/demo");
        assert!(!conv.limited);
    }

    #[test]
    fn tools_become_cards() {
        let conv = parse(TOOLS, SID);
        let got: Vec<(&str, &str, bool)> = cards(&conv)
            .iter()
            .map(|c| (c.name.as_str(), c.summary.as_str(), c.error))
            .collect();
        assert_eq!(
            got,
            [
                ("Edit", "Edited src/app.ts, 2 added, 1 removed", false),
                ("Write", "Created notes.md, 3 lines", false),
                ("Bash", "Ran cargo test, failed", true),
                ("Read", "Read src/app.ts", false),
                ("Grep", "Searched for TODO", false),
                ("Frobnicate", "Used Frobnicate", false),
            ]
        );
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(texts(&conv.turns[0]), ["All done."]);
    }

    #[test]
    fn subagent_and_compaction() {
        let conv = parse(SUBAGENT, SID);
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(
            conv.turns[0].items,
            vec![
                TurnItem::Text {
                    markdown: "I'll delegate this.".into()
                },
                TurnItem::Subagent {
                    description: "Survey tests".into(),
                    prompt: "List every test file under src/.".into(),
                    result: Some("Found 3 test files.".into()),
                },
                TurnItem::Compacted,
                TurnItem::Text {
                    markdown: "Continuing after the summary.".into()
                },
            ]
        );
        let json = serde_json::to_string(&conv).unwrap();
        assert!(!json.contains("SIDECHAIN-TEXT"));
        assert!(!json.contains("SUMMARY-TEXT"));
    }

    #[test]
    fn commands_and_wrappers() {
        let conv = parse(COMMANDS, SID);
        let prompts: Vec<&str> = conv.turns.iter().map(|t| t.prompt.as_str()).collect();
        assert_eq!(prompts, ["/model opus", "! git status", "look at this [image]"]);
        assert_eq!(texts(&conv.turns[2]), ["Looks fine."]);
        assert!(!conv.limited);
    }

    #[test]
    fn interrupt_markers_are_not_prompts() {
        let user = |n: u32, content: serde_json::Value| {
            serde_json::json!({
                "type": "user", "uuid": format!("00000000-0000-4000-8000-{n:012}"), "cwd": "C:/work/demo",
                "version": "2.1.309", "timestamp": "2026-10-20T09:00:02.000Z",
                "message": {"role": "user", "content": content}
            })
        };
        let conv = parse(
            &transcript(
                Some("2.1.309"),
                &[
                    assistant(2, serde_json::json!({"type": "text", "text": "Working on it."})),
                    user(
                        3,
                        serde_json::json!([{"type": "text", "text": "[Request interrupted by user]"}]),
                    ),
                    user(4, serde_json::json!("[Request interrupted by user for tool use]")),
                    assistant(5, serde_json::json!({"type": "text", "text": "Stopped."})),
                ],
            ),
            SID,
        );
        let prompts: Vec<&str> = conv.turns.iter().map(|t| t.prompt.as_str()).collect();
        assert_eq!(prompts, ["go"]);
        assert_eq!(texts(&conv.turns[0]), ["Working on it.", "Stopped."]);
    }

    #[test]
    fn untested_version_is_limited() {
        let conv = parse(FUTURE, SID);
        assert!(conv.limited);
        assert_eq!(conv.version.as_deref(), Some("9.9.9"));
        assert_eq!(conv.turns.len(), 1);
        assert_eq!(conv.turns[0].prompt, "Hello future");
        assert_eq!(texts(&conv.turns[0]), ["Known text survives."]);
    }

    #[test]
    fn missing_version_is_limited() {
        let mut reply = assistant(2, serde_json::json!({"type": "text", "text": "hi"}));
        reply.as_object_mut().unwrap().remove("version");
        let conv = parse(&transcript(None, &[reply]), SID);
        assert!(conv.limited);
        assert_eq!(conv.version, None);
        assert_eq!(texts(&conv.turns[0]), ["hi"]);
    }

    #[test]
    fn version_gate_is_numeric_per_part() {
        assert!(!is_untested("2.1.278"));
        assert!(!is_untested("2.1.309"));
        assert!(!is_untested("2.1.300"));
        assert!(is_untested("2.1.277"));
        assert!(is_untested("2.1.310"));
        assert!(is_untested("2.2.0"));
        assert!(is_untested("1.9.999"));
        assert!(is_untested("2.1"));
        assert!(is_untested("2.1.x"));
        assert!(is_untested(""));
    }

    #[test]
    fn rfc3339_reads_to_unix_ms() {
        assert_eq!(rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_ms("2001-09-09T01:46:40Z"), Some(1_000_000_000_000));
        assert_eq!(rfc3339_ms("2001-09-09T01:46:40.5Z"), Some(1_000_000_000_500));
        assert_eq!(rfc3339_ms("2001-09-09T01:46:40.123456Z"), Some(1_000_000_000_123));
        assert_eq!(rfc3339_ms("2001-09-09T03:46:40.123+02:00"), Some(1_000_000_000_123));
        assert_eq!(rfc3339_ms("2000-03-01T00:00:00Z"), Some(951_868_800_000));
        assert_eq!(rfc3339_ms("2024-02-29T12:00:00Z"), Some(1_709_208_000_000));
        assert_eq!(rfc3339_ms("yesterday"), None);
        assert_eq!(rfc3339_ms("2026-13-01T00:00:00Z"), None);
        assert_eq!(rfc3339_ms("1969-12-31T23:59:59Z"), None);
    }

    #[test]
    fn caps_apply() {
        use serde_json::json;
        let lines = [
            assistant(2, json!({"type": "text", "text": "x".repeat(150_000)})),
            assistant(
                3,
                json!({"type": "tool_use", "id": "toolu_1", "name": "Bash", "input": {"command": "run"}}),
            ),
            json!({
                "type": "user", "uuid": "00000000-0000-4000-8000-000000000004", "version": "2.1.309",
                "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "x"}]},
                "toolUseResult": {"stdout": "o".repeat(10_000), "stderr": "", "interrupted": false}
            }),
            json!({
                "type": "user", "uuid": "00000000-0000-4000-8000-000000000005", "version": "2.1.309",
                "timestamp": "2026-10-20T09:00:09.000Z",
                "message": {"role": "user", "content": "p".repeat(30_000)}
            }),
        ];
        let conv = parse(&transcript(Some("2.1.309"), &lines), SID);
        let TurnItem::Text { markdown } = &conv.turns[0].items[0] else {
            panic!("no text")
        };
        assert_eq!(markdown.chars().count(), 100_001);
        assert!(markdown.ends_with('…'));
        let detail = cards(&conv)[0].detail.as_deref().unwrap();
        assert_eq!(detail.chars().count(), 4_001);
        assert!(detail.ends_with('…'));
        assert_eq!(conv.turns[1].prompt.chars().count(), 20_001);
        assert!(conv.turns[1].prompt.ends_with('…'));
    }

    #[test]
    fn title_falls_back_to_first_prompt() {
        let prompt = format!("{}\nsecond line", "t".repeat(100));
        let line = serde_json::json!({
            "type": "user", "uuid": "00000000-0000-4000-8000-000000000001", "version": "2.1.309",
            "message": {"role": "user", "content": prompt}
        });
        assert_eq!(parse(&line.to_string(), SID).title, format!("{}…", "t".repeat(80)));
        assert_eq!(parse(&transcript(Some("2.1.309"), &[]), SID).title, "go");
        assert_eq!(parse("", SID).title, "");
    }

    #[test]
    fn bash_detail_holds_command_and_output() {
        let conv = parse(TOOLS, SID);
        let cards = cards(&conv);
        assert_eq!(
            cards[2].detail.as_deref(),
            Some("cargo test\necho done\n\nrunning 3 tests\nerror: test failed")
        );
        assert_eq!(
            cards[0].detail.as_deref(),
            Some(" const a = 1;\n-const b = 2;\n+const b = 3;\n+const c = 4;\n return a;")
        );
        assert_eq!(cards[3].detail, None);
    }

    #[test]
    fn odd_input_never_fails() {
        for text in [
            "",
            "\n\n",
            "[]",
            "null",
            "{\"type\":\"user\"}",
            "{\"type\":\"assistant\",\"message\":5}",
            "\u{feff}{}",
        ] {
            let conv = parse(text, SID);
            assert!(conv.turns.is_empty(), "{text:?}");
            assert!(conv.limited);
        }
    }
}
