//! A conversation as markdown or JSON text, for saving or copying. Secrets and personal paths are taken out
//! field by field before the text is built, so the JSON still parses and the markdown keeps its shape.

use super::cast::{self, FrameStart};
use crate::diagnostics::redact::{self, Redactor};
use crate::paths;
use crate::pty::record::CastEncoder;
use marshell_protocol::history::{Conversation, RecordingPart, TurnItem};
use serde::Deserialize;
use std::borrow::Cow;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Json,
    Asciicast,
}

/// The conversation as markdown. Times are left out (the JSON has them) and so are tool details.
/// Blocks are separated by one blank line; consecutive tool and subagent lines are one list.
pub fn to_markdown(c: &Conversation) -> String {
    let mut blocks: Vec<String> = vec![format!("# {}", one_line(&c.title))];
    let mut facts = vec![format!("- Folder: {}", one_line(&c.cwd))];
    let optional = [("Branch", &c.branch), ("Model", &c.model), ("Claude Code", &c.version)];
    for (label, value) in optional {
        if let Some(value) = value {
            facts.push(format!("- {label}: {}", one_line(value)));
        }
    }
    facts.push(format!("- Session: {}", one_line(&c.session_id)));
    blocks.push(facts.join("\n"));
    for turn in &c.turns {
        blocks.push(format!("## Turn {}", turn.index + 1));
        let quoted: Vec<String> = turn.prompt.lines().map(|line| format!("> {line}")).collect();
        blocks.push(if quoted.is_empty() {
            "> ".into()
        } else {
            quoted.join("\n")
        });
        // The run of tool and subagent lines being built: it is one block.
        let mut list: Vec<String> = Vec::new();
        for item in &turn.items {
            match item {
                TurnItem::Tool { card } => list.push(format!("- {}", one_line(&card.summary))),
                TurnItem::Subagent { description, .. } => list.push(format!("- Subagent: {}", one_line(description))),
                TurnItem::Text { markdown } => {
                    flush(&mut blocks, &mut list);
                    blocks.push(markdown.trim_end_matches(['\n', '\r']).to_string());
                }
                TurnItem::Compacted => {
                    flush(&mut blocks, &mut list);
                    blocks.push("---\n*Conversation compacted*".into());
                }
            }
        }
        flush(&mut blocks, &mut list);
    }
    let mut text = blocks.join("\n\n");
    text.push('\n');
    text
}

/// Moves a finished run of list lines into `blocks`.
fn flush(blocks: &mut Vec<String>, list: &mut Vec<String>) {
    if !list.is_empty() {
        blocks.push(std::mem::take(list).join("\n"));
    }
}

/// A value shown after a label or in a list: its line breaks would end the line, so they become spaces.
fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// Redacts every string the export can show: the title, folder, branch, model and version, each prompt,
/// text, tool summary and detail, subagent field, and the person's name and tags. Identifiers (agent,
/// session and tab ids, tool names) stay as they are.
pub fn redact_conversation(c: &mut Conversation, r: &Redactor) {
    let clean = |text: &mut String| {
        if let std::borrow::Cow::Owned(redacted) = r.redact(text) {
            *text = redacted;
        }
    };
    let clean_opt = |text: &mut Option<String>| {
        if let Some(text) = text {
            clean(text);
        }
    };
    clean(&mut c.title);
    clean(&mut c.cwd);
    clean_opt(&mut c.branch);
    clean_opt(&mut c.model);
    clean_opt(&mut c.version);
    clean_opt(&mut c.marks.name);
    c.marks.tags.iter_mut().for_each(clean);
    for turn in &mut c.turns {
        clean(&mut turn.prompt);
        for item in &mut turn.items {
            match item {
                TurnItem::Text { markdown } => clean(markdown),
                TurnItem::Tool { card } => {
                    clean(&mut card.summary);
                    clean_opt(&mut card.detail);
                }
                TurnItem::Subagent {
                    description,
                    prompt,
                    result,
                } => {
                    clean(description);
                    clean(prompt);
                    clean_opt(result);
                }
                TurnItem::Compacted => {}
            }
        }
    }
}

/// The export text, with secrets and personal paths hidden when `redact` is set.
pub fn export_text(c: Conversation, format: ExportFormat, redact: bool) -> String {
    export_with(c, format, redact.then(redact::global))
}

fn export_with(mut c: Conversation, format: ExportFormat, redactor: Option<&Redactor>) -> String {
    if let Some(redactor) = redactor {
        redact_conversation(&mut c, redactor);
    }
    match format {
        ExportFormat::Markdown => to_markdown(&c),
        // Serializing plain data cannot fail.
        ExportFormat::Json => serde_json::to_string_pretty(&c).unwrap_or_default(),
        // The recording is built by `to_asciicast` from files, not from the conversation; the route never
        // asks for it here.
        ExportFormat::Asciicast => String::new(),
    }
}

/// The most an asciicast export may be, the cap `save_conversation` keeps.
pub const MAX_ASCIICAST_BYTES: usize = 64 * 1024 * 1024;

/// Output events this close together are one burst for redaction: a secret printed in one go may arrive
/// split across several reads.
const BURST_MS: u64 = 100;
/// The most output one merged burst holds.
const BURST_BYTES: usize = 64 * 1024;
/// Pause between one part's end and the next part's start.
const PART_GAP_MS: u64 = 1000;
/// The most event data one `frames` call is asked for.
const FRAMES_BUDGET: usize = 1 << 20;

#[derive(Debug)]
pub enum AsciicastError {
    TooLarge,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for AsciicastError {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

/// One event of the export, `at_ms` counted from its start.
struct Ev {
    at_ms: u64,
    code: &'static str,
    data: String,
}

/// The conversation's recording parts as one asciicast v2 file. Each part starts with the screen as it was at
/// the part's clip point, and the next part follows a second later after a resize and a `Part {n}` marker.
/// With a `redactor`, output is redacted in bursts and so is the title.
pub fn to_asciicast(
    home: &Path,
    parts: &[RecordingPart],
    title: &str,
    redactor: Option<&Redactor>,
) -> Result<String, AsciicastError> {
    to_asciicast_with_limit(home, parts, title, redactor, MAX_ASCIICAST_BYTES)
}

fn to_asciicast_with_limit(
    home: &Path,
    parts: &[RecordingPart],
    title: &str,
    redactor: Option<&Redactor>,
    limit: usize,
) -> Result<String, AsciicastError> {
    let Some(first) = parts.first() else {
        return Err(anyhow::anyhow!("there is no recording to export").into());
    };
    let dir = paths::recordings_dir(home);
    let mut events: Vec<Ev> = Vec::new();
    let mut raw = 0usize;
    let mut clock = 0u64;
    for (k, part) in parts.iter().enumerate() {
        if !cast::valid_recording_name(&part.file) {
            return Err(anyhow::anyhow!("{:?} is not a recording name", part.file).into());
        }
        let path = dir.join(&part.file);
        // The span's end is the last moment of the conversation, so it is included.
        let to_ms = part.span_to_ms.saturating_add(1);
        let base = if k == 0 { 0 } else { clock + PART_GAP_MS };
        let mut got = cast::frames(
            &path,
            part.started_ms,
            FrameStart::At(part.span_from_ms),
            to_ms,
            FRAMES_BUDGET,
        )
        .map_err(|err| anyhow::anyhow!("{err}"))?;
        // The header carries the first part's size; a different size at the clip point is a resize.
        if k > 0 || (got.cols, got.rows) != (part.cols, part.rows) {
            events.push(Ev {
                at_ms: base,
                code: "r",
                data: format!("{}x{}", got.cols, got.rows),
            });
        }
        if k > 0 {
            events.push(Ev {
                at_ms: base,
                code: "m",
                data: format!("Part {}", k + 1),
            });
        }
        if let Some(screen) = got.screen.take().filter(|s| !s.is_empty()) {
            raw += screen.len();
            events.push(Ev {
                at_ms: base,
                code: "o",
                data: screen,
            });
        }
        clock = base;
        loop {
            for e in got.events {
                let at_ms = base + e.at_ms.saturating_sub(part.span_from_ms);
                clock = clock.max(at_ms);
                raw += e.data.len() + 32;
                let code = match e.code.as_str() {
                    "o" => "o",
                    "r" => "r",
                    "m" => "m",
                    _ => continue,
                };
                events.push(Ev {
                    at_ms,
                    code,
                    data: e.data,
                });
            }
            if raw > limit {
                return Err(AsciicastError::TooLarge);
            }
            let Some(offset) = got.next_offset else { break };
            got = cast::frames(&path, part.started_ms, FrameStart::Offset(offset), to_ms, FRAMES_BUDGET)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
        }
    }
    if let Some(redactor) = redactor {
        events = redact_bursts(events, redactor);
    }
    let title = redactor.map_or(Cow::Borrowed(title), |r| r.redact(title));
    let header = serde_json::json!({
        "version": 2,
        "width": first.cols,
        "height": first.rows,
        "timestamp": first.started_ms / 1000,
        "title": title,
        "env": { "TERM": "xterm-256color" },
    });
    let mut text = format!("{header}\n");
    for e in &events {
        let t = e.at_ms as f64 / 1000.0;
        let line = match e.code {
            "r" => {
                let size = e
                    .data
                    .split_once('x')
                    .and_then(|(c, r)| Some((c.parse().ok()?, r.parse().ok()?)));
                match size {
                    Some((cols, rows)) => CastEncoder::resize(t, cols, rows),
                    None => continue,
                }
            }
            "m" => CastEncoder::marker(t, &e.data),
            _ => match CastEncoder::default().output(t, e.data.as_bytes()) {
                Some(line) => line,
                None => continue,
            },
        };
        text += &line;
        if text.len() > limit {
            return Err(AsciicastError::TooLarge);
        }
    }
    Ok(text)
}

/// Merges runs of output events less than 100 ms apart (up to 64 KiB each, at the first one's time), then
/// redacts each output event, so a secret split across reads is seen whole.
fn redact_bursts(events: Vec<Ev>, redactor: &Redactor) -> Vec<Ev> {
    let mut out: Vec<Ev> = Vec::with_capacity(events.len());
    let mut previous_ms = 0u64;
    for e in events {
        if e.code == "o"
            && let Some(last) = out.last_mut()
            && last.code == "o"
            && e.at_ms.saturating_sub(previous_ms) < BURST_MS
            && last.data.len() + e.data.len() <= BURST_BYTES
        {
            previous_ms = e.at_ms;
            last.data.push_str(&e.data);
            continue;
        }
        previous_ms = e.at_ms;
        out.push(e);
    }
    for e in &mut out {
        if e.code == "o"
            && let Cow::Owned(redacted) = redactor.redact(&e.data)
        {
            e.data = redacted;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::claude::transcript;
    use marshell_protocol::history::{ConversationMarks, ToolCard, Turn};
    use std::path::PathBuf;

    const TOOLS: &str = include_str!("../../tests/fixtures/claude/transcripts/2.1.309/tools.jsonl");
    const SUBAGENT: &str = include_str!("../../tests/fixtures/claude/transcripts/2.1.309/subagent-compact.jsonl");
    const SID: &str = "00000000-0000-4000-8000-0000000000aa";

    #[test]
    fn markdown_shape_matches() {
        let c = transcript::parse(TOOLS, SID);
        let expected = "\
# Fix the typo in src/app.ts and write notes

- Folder: C:/work/demo
- Branch: main
- Model: claude-sonnet-5-5
- Claude Code: 2.1.309
- Session: 00000000-0000-4000-8000-0000000000aa

## Turn 1

> Fix the typo in src/app.ts and write notes

- Edited src/app.ts, 2 added, 1 removed
- Created notes.md, 3 lines
- Ran cargo test, failed
- Read src/app.ts
- Searched for TODO
- Used Frobnicate

All done.
";
        assert_eq!(to_markdown(&c), expected);
    }

    #[test]
    fn markdown_covers_subagents_compaction_and_multiline_prompts() {
        let mut c = transcript::parse(SUBAGENT, SID);
        c.model = Some("claude-sonnet-5-5".into());
        c.turns[0].prompt = "first\n\nthird".into();
        let md = to_markdown(&c);
        assert!(md.contains("- Model: claude-sonnet-5-5\n"), "{md}");
        assert!(md.contains("\n> first\n> \n> third\n\n"), "{md}");
        assert!(
            md.ends_with(
                "I'll delegate this.\n\n- Subagent: Survey tests\n\n---\n*Conversation compacted*\n\nContinuing after the summary.\n"
            ),
            "{md}"
        );
    }

    fn hostile() -> Conversation {
        Conversation {
            agent: "claude".into(),
            session_id: SID.into(),
            tab_id: "t1".into(),
            title: "Use sk-ant-abcdefgh12345678 here".into(),
            cwd: r"C:\Users\someone\work".into(),
            branch: Some("fix/someone-token".into()),
            model: Some("m".into()),
            version: Some("2.1.309".into()),
            limited: false,
            turns: vec![Turn {
                index: 0,
                at_ms: 1,
                prompt: "my key is sk-ant-abcdefgh12345678".into(),
                items: vec![
                    TurnItem::Text {
                        markdown: r#"config {password: "x\"y"} done"#.into(),
                    },
                    TurnItem::Tool {
                        card: ToolCard {
                            name: "Bash".into(),
                            summary: "Ran git push with ghp_abcdefghijklmnopqrstuvwx1234".into(),
                            detail: Some(r"token=ghp_abcdefghijklmnopqrstuvwx1234 in C:\Users\someone\x".into()),
                            file: None,
                            error: false,
                        },
                    },
                    TurnItem::Subagent {
                        description: "Look at sk-ant-abcdefgh12345678".into(),
                        prompt: "password: hunter2".into(),
                        result: Some("found sk-ant-abcdefgh12345678".into()),
                    },
                ],
            }],
            marks: ConversationMarks {
                name: Some("sk-ant-abcdefgh12345678".into()),
                tags: vec!["sk-ant-abcdefgh12345678".into()],
                ..ConversationMarks::default()
            },
        }
    }

    /// Strings that must be gone from a redacted export, in either format. (JSON doubles a backslash, so
    /// the user name stands in for the folder there.)
    const SECRETS: [&str; 6] = [
        "abcdefgh12345678",
        "ghp_abcdefghijklmnopqrstuvwx1234",
        r#"x\"y"#,
        "hunter2",
        r"C:\Users\someone",
        "someone",
    ];

    #[test]
    fn redaction_covers_every_field_and_json_still_parses() {
        let redactor = Redactor::new(Some(PathBuf::from(r"C:\Users\someone")), None, Some("someone".into()));
        for format in [ExportFormat::Markdown, ExportFormat::Json] {
            let hidden = export_with(hostile(), format, Some(&redactor));
            for secret in SECRETS {
                assert!(!hidden.contains(secret), "{format:?} leaked {secret}: {hidden}");
            }
            let shown = export_with(hostile(), format, None);
            assert!(shown.contains("abcdefgh12345678"), "{format:?}: {shown}");
            assert!(shown.contains("someone"), "{format:?}: {shown}");
            if format == ExportFormat::Json {
                // Tool details and a subagent's prompt and result are JSON-only.
                for secret in ["ghp_abcdefghijklmnopqrstuvwx1234", "hunter2", r#"x\\\"y"#] {
                    assert!(shown.contains(secret), "{secret} in {shown}");
                }
                serde_json::from_str::<serde_json::Value>(&shown).expect("plain JSON parses");
                let back: Conversation = serde_json::from_str(&hidden).expect("redacted JSON parses");
                assert_eq!(back.turns[0].items.len(), 3);
                assert_eq!(back.marks.tags.len(), 1);
            }
        }
    }

    #[test]
    fn export_text_uses_the_process_redactor_only_when_asked() {
        let on = export_text(hostile(), ExportFormat::Json, true);
        assert!(!on.contains("abcdefgh12345678"), "{on}");
        assert!(on.contains("<anthropic-key>"), "{on}");
        let off = export_text(hostile(), ExportFormat::Json, false);
        assert!(off.contains("abcdefgh12345678"), "{off}");
    }

    mod asciicast {
        use super::*;
        use crate::pty::record::{CastHeader, header_line};
        use serde_json::Value;

        const T0: u64 = 1_761_300_000_000;
        const KEY: &str = "sk-ant-abcdefgh12345678";

        fn name(start: u64) -> String {
            format!("t1-{start}.cast")
        }

        /// Writes a recording of `size` that starts at `start` and holds `lines` (seconds, code, data).
        fn write(home: &Path, start: u64, size: (u16, u16), lines: &[(f64, &str, &str)]) -> RecordingPart {
            let dir = paths::recordings_dir(home);
            std::fs::create_dir_all(&dir).unwrap();
            let mut text = header_line(&CastHeader {
                cols: size.0,
                rows: size.1,
                started_unix: start / 1000,
                shell: None,
            });
            let mut last = 0.0;
            for &(t, code, data) in lines {
                last = t;
                text += &match code {
                    "o" => CastEncoder::default().output(t, data.as_bytes()).unwrap(),
                    "r" => CastEncoder::resize(t, 1, 1),
                    _ => CastEncoder::marker(t, data),
                };
            }
            std::fs::write(dir.join(name(start)), text).unwrap();
            let ended = start + (last * 1000.0).round() as u64;
            RecordingPart {
                index: 0,
                file: name(start),
                started_ms: start,
                ended_ms: ended,
                cols: size.0,
                rows: size.1,
                span_from_ms: start,
                span_to_ms: ended,
                marks_ms: Vec::new(),
                stopped: None,
            }
        }

        fn parsed(text: &str) -> (Value, Vec<(f64, String, String)>) {
            let mut lines = text.lines();
            let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
            let events = lines
                .map(|line| {
                    let v: Value = serde_json::from_str(line).unwrap();
                    let a = v.as_array().unwrap();
                    assert_eq!(a.len(), 3, "{line}");
                    (
                        a[0].as_f64().unwrap(),
                        a[1].as_str().unwrap().to_string(),
                        a[2].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            (header, events)
        }

        fn output(events: &[(f64, String, String)]) -> String {
            events.iter().filter(|e| e.1 == "o").map(|e| e.2.as_str()).collect()
        }

        #[test]
        fn asciicast_export_is_valid_v2() {
            let home = tempfile::tempdir().unwrap();
            let one = write(
                home.path(),
                T0,
                (80, 24),
                &[(1.0, "o", "one\r\n"), (2.0, "o", "two\r\n")],
            );
            let mut two = write(
                home.path(),
                T0 + 100_000,
                (100, 30),
                &[(0.5, "o", "three\r\n"), (3.0, "m", "prompt")],
            );
            two.index = 1;
            let text = to_asciicast(home.path(), &[one, two], "My chat", None).unwrap();
            let (header, events) = parsed(&text);
            assert_eq!(header["version"], 2);
            assert_eq!(
                (header["width"].as_u64(), header["height"].as_u64()),
                (Some(80), Some(24))
            );
            assert_eq!(header["timestamp"], T0 / 1000);
            assert_eq!(header["title"], "My chat");
            assert_eq!(header["env"]["TERM"], "xterm-256color");
            assert!(events.windows(2).all(|w| w[0].0 <= w[1].0), "{events:?}");
            assert!(events.iter().all(|e| matches!(e.1.as_str(), "o" | "r" | "m")));
            // Part 1 ends at 2.0 s; part 2 follows 1.0 s later with its size and a marker.
            let at = events.iter().position(|e| e.1 == "r").expect("a resize");
            assert_eq!(events[at], (3.0, "r".into(), "100x30".into()));
            assert_eq!(events[at + 1], (3.0, "m".into(), "Part 2".into()));
            assert_eq!(events[at + 2].1, "o");
            assert!(events.iter().any(|e| e.0 == 3.5 && e.2 == "three\r\n"), "{events:?}");
            assert!(events.iter().any(|e| e.0 == 6.0 && e.2 == "prompt"), "{events:?}");
        }

        #[test]
        fn asciicast_export_starts_with_the_screen_at_the_clip() {
            let home = tempfile::tempdir().unwrap();
            let mut part = write(
                home.path(),
                T0,
                (80, 24),
                &[
                    (1.0, "o", "line 0\r\n"),
                    (2.0, "o", "line 1\r\n"),
                    (3.0, "o", "line 2\r\n"),
                ],
            );
            part.span_from_ms = T0 + 2500;
            let text = to_asciicast(home.path(), &[part], "t", None).unwrap();
            let (_, events) = parsed(&text);
            assert_eq!(events.len(), 2, "{events:?}");
            assert_eq!(events[0].0, 0.0);
            assert!(events[0].2.contains("line 0") && events[0].2.contains("line 1"));
            assert!(!events[0].2.contains("line 2"));
            assert_eq!(events[1], (0.5, "o".into(), "line 2\r\n".into()));
        }

        #[test]
        fn asciicast_redaction_catches_a_key_split_across_events() {
            let home = tempfile::tempdir().unwrap();
            let part = write(
                home.path(),
                T0,
                (80, 24),
                &[
                    (1.000, "o", "key sk-ant-abc"),
                    (1.005, "o", "defgh12345678 done\r\n"),
                    (2.0, "o", r"C:\Users\someone\work> "),
                ],
            );
            let redactor = Redactor::new(Some(PathBuf::from(r"C:\Users\someone")), None, Some("someone".into()));
            let title = format!("use {KEY}");

            let hidden = to_asciicast(home.path(), std::slice::from_ref(&part), &title, Some(&redactor)).unwrap();
            let (header, events) = parsed(&hidden);
            // The key's halves sit on separate event lines in the file, so look at the output as a player shows it.
            let played = output(&events);
            for secret in [KEY, "abcdefgh12345678", "defgh12345678", r"C:\Users\someone", "someone"] {
                assert!(!hidden.contains(secret), "leaked {secret}: {hidden}");
                assert!(!played.contains(secret), "leaked {secret} on replay: {played}");
            }
            assert!(!header["title"].as_str().unwrap().contains("abcdefgh"));
            assert!(output(&events).contains("done"));

            let shown = to_asciicast(home.path(), &[part], &title, None).unwrap();
            let (header, events) = parsed(&shown);
            assert_eq!(header["title"], title.as_str());
            let all = output(&events);
            assert!(all.contains(KEY), "{all}");
            assert!(all.contains(r"C:\Users\someone"), "{all}");
        }

        #[test]
        fn asciicast_export_refuses_over_the_limit() {
            let home = tempfile::tempdir().unwrap();
            let part = write(home.path(), T0, (80, 24), &[(1.0, "o", &"x".repeat(5000))]);
            let parts = [part];
            assert!(matches!(
                to_asciicast_with_limit(home.path(), &parts, "t", None, 4000),
                Err(AsciicastError::TooLarge)
            ));
            assert!(to_asciicast_with_limit(home.path(), &parts, "t", None, 100_000).is_ok());
        }

        #[test]
        fn asciicast_export_leaves_out_another_conversations_output_in_an_earlier_tab() {
            use crate::history::recordings::{CastCache, parts};
            use crate::history::{RecordingLink, SessionMeta};
            let home = tempfile::tempdir().unwrap();
            // X runs in tab A, /clear starts Y in the same recording, then X is resumed in tab B.
            let in_a = write(
                home.path(),
                T0,
                (80, 24),
                &[
                    (2.0, "o", "x before the clear\r\n"),
                    (10.0, "o", "y after the clear\r\n"),
                    (12.0, "o", "y goes on\r\n"),
                ],
            );
            let in_b = write(
                home.path(),
                T0 + 20_000,
                (80, 24),
                &[(1.0, "o", "x resumed\r\n"), (5.0, "o", "x goes on\r\n")],
            );
            let meta = SessionMeta {
                agent: "claude".into(),
                session_id: "x".into(),
                tab_id: "t1".into(),
                cwd: String::new(),
                transcript_path: String::new(),
                started_ms: T0 + 1000,
                updated_ms: T0 + 25_000,
                previous_in_tab: None,
                recordings: vec![
                    RecordingLink {
                        file: in_a.file.clone(),
                        first_ms: T0 + 1000,
                        last_ms: T0 + 4000,
                    },
                    RecordingLink {
                        file: in_b.file.clone(),
                        first_ms: T0 + 20_000,
                        last_ms: T0 + 25_000,
                    },
                ],
                client: Default::default(),
            };
            let got = parts(home.path(), &meta, (T0 + 1000, T0 + 25_000), &CastCache::default());
            let text = to_asciicast(home.path(), &got, "x", None).unwrap();
            let (_, events) = parsed(&text);
            let all = output(&events);
            assert!(all.contains("x before the clear"), "{all}");
            assert!(all.contains("x resumed") && all.contains("x goes on"), "{all}");
            assert!(
                !all.contains("y after the clear") && !all.contains("y goes on"),
                "{all}"
            );
        }

        #[test]
        fn asciicast_export_refuses_a_hostile_file_name() {
            let home = tempfile::tempdir().unwrap();
            let mut part = write(home.path(), T0, (80, 24), &[(1.0, "o", "x")]);
            part.file = r"..\x.cast".into();
            assert!(matches!(
                to_asciicast(home.path(), &[part], "t", None),
                Err(AsciicastError::Failed(_))
            ));
        }
    }
}
