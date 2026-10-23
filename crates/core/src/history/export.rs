//! A conversation as markdown or JSON text, for saving or copying. Secrets and personal paths are taken out
//! field by field before the text is built, so the JSON still parses and the markdown keeps its shape.

use crate::diagnostics::redact::{self, Redactor};
use marshell_protocol::history::{Conversation, TurnItem};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Json,
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
    }
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
}
