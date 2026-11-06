//! Qwen Code's history, read for the Doctor's usage items: MCP tool calls and skill uses. Reads only what is new in
//! each chat file since the cursor, and never writes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agents::common::rfc3339_ms;
use crate::doctor::ext::ExtRoots;
use crate::doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines};

/// `projects/*/chats/*.jsonl`, without the `*.ledger.jsonl` files.
fn history_files(projects: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(projects) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let mut out = Vec::new();
    for project in dirs {
        let Ok(rd) = std::fs::read_dir(project.join("chats")) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".jsonl") && !name.ends_with(".ledger.jsonl") && p.is_file()
            })
            .collect();
        files.sort();
        out.extend(files);
    }
    out
}

fn used(key: UseKey, at_ms: u64, cwd: &Option<String>) -> Event {
    Event {
        kind: EventKind::Used,
        key,
        at_ms,
        cwd: cwd.clone(),
    }
}

fn record_events(rec: &Value, at: u64, cwd: &Option<String>, out: &mut Vec<Event>) {
    let parts = rec.pointer("/message/parts").and_then(Value::as_array);
    match rec.get("type").and_then(Value::as_str) {
        Some("user") => {
            let first_text = parts
                .into_iter()
                .flatten()
                .find_map(|p| p.get("text").and_then(Value::as_str));
            if let Some(text) = first_text
                && let Some(rest) = text.strip_prefix('/')
                && let Some(word) = rest.split_whitespace().next()
            {
                out.push(used(UseKey::SkillName(word.to_string()), at, cwd));
            }
        }
        _ => {
            for call in parts.into_iter().flatten().filter_map(|p| p.get("functionCall")) {
                let name = call.get("name").and_then(Value::as_str);
                if let Some(n) = name.filter(|n| n.starts_with("mcp__")) {
                    out.push(used(UseKey::Tool(n.to_string()), at, cwd));
                }
                if name == Some("skill")
                    && let Some(skill) = call.pointer("/args/skill").and_then(Value::as_str)
                {
                    out.push(used(UseKey::SkillName(skill.to_string()), at, cwd));
                }
                // A deferred tool is called through a bridge tool that carries the MCP name in its arguments.
                if let Some(n) = call
                    .pointer("/args/name")
                    .and_then(Value::as_str)
                    .filter(|n| n.starts_with("mcp__"))
                {
                    out.push(used(UseKey::Tool(n.to_string()), at, cwd));
                }
            }
        }
    }
}

/// Reads the new lines of one chat file. `None` when the file can't be read now.
fn read_file(path: &Path, old: Option<&Cursor>) -> Option<(Vec<Event>, Cursor)> {
    let (lines, mut cur) = new_lines(path, old).ok()?;
    let mut events = Vec::new();
    for line in lines {
        let Ok(rec) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let at = rec.get("timestamp").and_then(Value::as_str).and_then(rfc3339_ms);
        if let Some(t) = at {
            cur.first_ms.get_or_insert(t);
        }
        if let Some(c) = rec.get("cwd").and_then(Value::as_str) {
            cur.cwd = Some(c.to_string());
        }
        if let Some(at) = at {
            record_events(&rec, at, &cur.cwd, &mut events);
        }
    }
    Some((events, cur))
}

/// One scan of Qwen Code's history, reading each chat file from where `store` left off.
pub fn scan(roots: &ExtRoots, store: &AgentStore) -> Scan {
    let projects = roots.qwen_runtime.join("projects");
    match std::fs::read_dir(&projects) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return Scan {
                events: Vec::new(),
                cursors: store.cursors.clone(),
                unreadable: Some("Its history folder can't be listed.".to_string()),
            };
        }
    }
    let mut out = Scan::default();
    let mut cursors = BTreeMap::new();
    for file in history_files(&projects) {
        let key = file.display().to_string();
        if let Some((events, cur)) = read_file(&file, store.cursors.get(&key)) {
            out.events.extend(events);
            cursors.insert(key, cur);
        }
    }
    out.cursors = cursors;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(h: &Path) -> ExtRoots {
        ExtRoots {
            home: h.to_path_buf(),
            claude_dir: h.join(".claude"),
            claude_json: h.join(".claude.json"),
            claude_settings: h.join(".claude").join("settings.json"),
            codex_home: h.join(".codex"),
            qwen_dir: h.join(".qwen"),
            claude_tool_search: None,
            qwen_runtime: h.join(".qwen"),
            gemini_dir: h.join(".gemini"),
        }
    }

    fn ms(s: &str) -> u64 {
        rfc3339_ms(s).unwrap()
    }

    fn ev(key: UseKey, at: &str) -> Event {
        Event {
            kind: EventKind::Used,
            key,
            at_ms: ms(at),
            cwd: Some("c:/dev/x".into()),
        }
    }

    const T1: &str = "2026-11-01T10:00:00.000Z";
    const T2: &str = "2026-11-01T11:00:00.000Z";
    const T3: &str = "2026-11-01T12:00:00.000Z";
    const T4: &str = "2026-11-01T13:00:00.000Z";
    const T5: &str = "2026-11-01T14:00:00.000Z";

    #[test]
    fn qwen_events() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let chats = home.join(".qwen").join("projects").join("c--dev-x").join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let rec = |t: &str, ty: &str, parts: &str| {
            format!(
                r#"{{"uuid":"u","sessionId":"s","timestamp":"{t}","type":"{ty}","cwd":"c:/dev/x","message":{{"role":"x","parts":[{parts}]}}}}"#
            )
        };
        let lines = [
            rec(T1, "user", r#"{"text":"/review the diff"}"#),
            rec(T2, "user", r#"{"text":"please /not-a-command"}"#),
            rec(
                T3,
                "assistant",
                r#"{"text":"ok"},{"functionCall":{"id":"1","name":"mcp__gh__list","args":{}}},{"functionCall":{"id":"2","name":"skill","args":{"skill":"Ship","args":"x"}}},{"functionCall":{"id":"3","name":"read_file","args":{"path":"a"}}}"#,
            ),
            rec(
                T4,
                "assistant",
                r#"{"functionCall":{"id":"4","name":"tool_call","args":{"name":"mcp__plug__q","arguments":{}}}},{"functionCall":{"id":"5","name":"tool_call","args":{"name":"other"}}}"#,
            ),
            "not json".to_string(),
        ];
        std::fs::write(
            chats.join("s1.jsonl"),
            lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
        )
        .unwrap();
        // A ledger file is not a chat.
        std::fs::write(
            chats.join("s1.ledger.jsonl"),
            format!(
                "{}\n",
                rec(
                    T5,
                    "assistant",
                    r#"{"functionCall":{"name":"mcp__ledger__t","args":{}}}"#
                )
            ),
        )
        .unwrap();

        let s = scan(&roots(home), &AgentStore::default());
        assert_eq!(
            s.events,
            vec![
                ev(UseKey::SkillName("review".into()), T1),
                ev(UseKey::Tool("mcp__gh__list".into()), T3),
                ev(UseKey::SkillName("Ship".into()), T3),
                ev(UseKey::Tool("mcp__plug__q".into()), T4),
            ]
        );
        assert_eq!(s.unreadable, None);
        assert_eq!(s.cursors.len(), 1);
        let c = &s.cursors[&chats.join("s1.jsonl").display().to_string()];
        assert_eq!(c.first_ms, Some(ms(T1)));

        // A second scan with the cursors finds nothing new.
        let store = AgentStore {
            cursors: s.cursors,
            ..AgentStore::default()
        };
        assert!(scan(&roots(home), &store).events.is_empty());

        // No history folder: no history, and not an error.
        let none = tempfile::tempdir().unwrap();
        assert_eq!(scan(&roots(none.path()), &AgentStore::default()), Scan::default());
    }
}
