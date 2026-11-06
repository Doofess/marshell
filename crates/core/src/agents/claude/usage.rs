//! Claude Code's history, read for the Doctor's usage items: MCP tool calls, skill uses, and what each session had
//! available. Reads only what is new in each transcript since the cursor, and never writes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agents::common::rfc3339_ms;
use crate::doctor::ext::ExtRoots;
use crate::doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines};

/// A line is worth parsing only when it holds one of these (or while the file's first time is still unknown).
const MARKERS: [&str; 4] = [
    "\"tool_use\"",
    "<command-",
    "\"deferred_tools_delta\"",
    "\"skill_listing\"",
];

fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.is_file())
        .collect();
    v.sort();
    v
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// Every transcript: `projects/*/*.jsonl` and `projects/*/*/subagents/*.jsonl`.
fn history_files(projects: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for project in subdirs(projects) {
        out.extend(jsonl_files(&project));
        for session in subdirs(&project) {
            out.extend(jsonl_files(&session.join("subagents")));
        }
    }
    out
}

fn event(kind: EventKind, key: UseKey, at_ms: u64, cwd: &Option<String>) -> Event {
    Event {
        kind,
        key,
        at_ms,
        cwd: cwd.clone(),
    }
}

/// The name in `<command-name>/name</command-name>`.
fn command_name(text: &str) -> Option<String> {
    let rest = &text[text.find("<command-name>")? + "<command-name>".len()..];
    let name = rest[..rest.find("</command-name>")?].trim();
    let name = name.strip_prefix('/').unwrap_or(name);
    (!name.is_empty()).then(|| name.to_string())
}

fn record_events(rec: &Value, at: u64, cwd: &Option<String>, out: &mut Vec<Event>) {
    match rec.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            let Some(items) = rec.pointer("/message/content").and_then(Value::as_array) else {
                return;
            };
            for item in items {
                if item.get("type").and_then(Value::as_str) != Some("tool_use") {
                    continue;
                }
                let Some(name) = item.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if name.starts_with("mcp__") {
                    out.push(event(EventKind::Used, UseKey::Tool(name.to_string()), at, cwd));
                } else if name == "Skill"
                    && let Some(skill) = item.pointer("/input/skill").and_then(Value::as_str)
                {
                    out.push(event(EventKind::Used, UseKey::SkillName(skill.to_string()), at, cwd));
                }
            }
        }
        Some("user") => {
            if let Some(text) = rec.pointer("/message/content").and_then(Value::as_str)
                && text.starts_with("<command-")
                && let Some(name) = command_name(text)
            {
                out.push(event(EventKind::Used, UseKey::SkillName(name), at, cwd));
            }
        }
        Some("attachment") => {
            let Some(att) = rec.get("attachment") else { return };
            let names = |field: &str| -> Vec<String> {
                att.get(field)
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default()
            };
            match att.get("type").and_then(Value::as_str) {
                Some("deferred_tools_delta") => {
                    for n in names("addedNames").into_iter().filter(|n| n.starts_with("mcp__")) {
                        out.push(event(EventKind::Available, UseKey::Tool(n), at, cwd));
                    }
                }
                Some("skill_listing") => {
                    for n in names("names") {
                        out.push(event(EventKind::Available, UseKey::SkillName(n), at, cwd));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// Reads the new lines of one transcript. `None` when the file can't be read now.
fn read_file(path: &Path, old: Option<&Cursor>) -> Option<(Vec<Event>, Cursor)> {
    let (lines, mut cur) = new_lines(path, old).ok()?;
    let mut events = Vec::new();
    for line in lines {
        if cur.first_ms.is_some() && !MARKERS.iter().any(|m| line.contains(m)) {
            continue;
        }
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

/// `~/.claude.json` `skillUsage`: each name's last use, with no place.
fn skill_usage(claude_json: &Path) -> Vec<Event> {
    let Ok(text) = std::fs::read_to_string(claude_json) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(map) = json.get("skillUsage").and_then(Value::as_object) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(name, v)| {
            let at = v.get("lastUsedAt")?;
            let ms = at
                .as_u64()
                .or_else(|| at.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
                .or_else(|| at.as_str().and_then(rfc3339_ms))?;
            Some(event(EventKind::LastUse, UseKey::SkillName(name.clone()), ms, &None))
        })
        .collect()
}

/// One scan of Claude Code's history, reading each transcript from where `store` left off.
pub fn scan(roots: &ExtRoots, store: &AgentStore) -> Scan {
    let projects = roots.claude_dir.join("projects");
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
    out.events.extend(skill_usage(&roots.claude_json));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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

    fn put(path: &Path, lines: &[String]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    }

    fn ms(s: &str) -> u64 {
        rfc3339_ms(s).unwrap()
    }

    fn ev(kind: EventKind, key: UseKey, at: &str, cwd: Option<&str>) -> Event {
        Event {
            kind,
            key,
            at_ms: ms(at),
            cwd: cwd.map(String::from),
        }
    }

    const T1: &str = "2026-11-01T10:00:00.000Z";
    const T2: &str = "2026-11-01T09:00:00.000Z";
    const T3: &str = "2026-11-02T10:00:00.000Z";

    fn at(t: &str) -> String {
        format!(r#""timestamp":"{t}","cwd":"c:/dev/x""#)
    }

    #[test]
    fn claude_events() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let main = home.join(".claude").join("projects").join("p1").join("s1.jsonl");
        let sub = home
            .join(".claude")
            .join("projects")
            .join("p1")
            .join("s1")
            .join("subagents")
            .join("agent-a.jsonl");
        put(
            &main,
            &[
                r#"{"type":"queue-operation","operation":"enqueue"}"#.to_string(),
                format!(
                    r#"{{"type":"user",{},"message":{{"content":"<command-message>ship</command-message>\n<command-name>/marshell-ship</command-name>"}}}}"#,
                    at(T1)
                ),
                // Out of time order: an earlier record after a later one.
                format!(
                    r#"{{"type":"assistant",{},"message":{{"content":[{{"type":"text","text":"x"}},{{"type":"tool_use","name":"mcp__gh__list","input":{{}}}},{{"type":"tool_use","name":"Read","input":{{}}}},{{"type":"tool_use","name":"Skill","input":{{"skill":"superpowers:writing-plans","args":"y"}}}}]}}}}"#,
                    at(T2)
                ),
                format!(
                    r#"{{"type":"attachment",{},"attachment":{{"type":"deferred_tools_delta","addedNames":["mcp__gh__list","Read"]}}}}"#,
                    at(T1)
                ),
                format!(
                    r#"{{"type":"attachment",{},"attachment":{{"type":"skill_listing","names":["marshell-ship"],"isInitial":true}}}}"#,
                    at(T1)
                ),
                // Not events: a command tag in a tool result, an mcp__ name in a request blob, "tool_use" in text.
                format!(
                    r#"{{"type":"user",{},"message":{{"content":[{{"type":"tool_result","content":"<command-name>/nope</command-name>"}}]}}}}"#,
                    at(T1)
                ),
                format!(
                    r#"{{"type":"api-request-blob",{},"body":"mcp__blob__tool <command-name>/nope2</command-name>"}}"#,
                    at(T1)
                ),
                format!(
                    r#"{{"type":"assistant",{},"message":{{"content":[{{"type":"text","text":"a \"tool_use\" block named mcp__text__t"}}]}}}}"#,
                    at(T1)
                ),
                "this is not json but says tool_use".to_string(),
            ],
        );
        put(
            &sub,
            &[format!(
                r#"{{"type":"assistant","isSidechain":true,{},"message":{{"content":[{{"type":"tool_use","name":"mcp__plugin_a_b__q","input":{{}}}}]}}}}"#,
                at(T3)
            )],
        );
        let s = scan(&roots(home), &AgentStore::default());
        let cwd = Some("c:/dev/x");
        let want = vec![
            ev(EventKind::Used, UseKey::SkillName("marshell-ship".into()), T1, cwd),
            ev(EventKind::Used, UseKey::Tool("mcp__gh__list".into()), T2, cwd),
            ev(
                EventKind::Used,
                UseKey::SkillName("superpowers:writing-plans".into()),
                T2,
                cwd,
            ),
            ev(EventKind::Available, UseKey::Tool("mcp__gh__list".into()), T1, cwd),
            ev(EventKind::Available, UseKey::SkillName("marshell-ship".into()), T1, cwd),
            ev(EventKind::Used, UseKey::Tool("mcp__plugin_a_b__q".into()), T3, cwd),
        ];
        assert_eq!(s.events, want);
        assert_eq!(s.unreadable, None);
        assert_eq!(s.cursors.len(), 2);
        let c = &s.cursors[&main.display().to_string()];
        assert_eq!(c.first_ms, Some(ms(T1)), "the first timed record, not the earliest");
        assert_eq!(c.cwd.as_deref(), Some("c:/dev/x"));
        assert_eq!(s.cursors[&sub.display().to_string()].first_ms, Some(ms(T3)));

        // No history folder: no history, and not an error.
        let none = tempfile::tempdir().unwrap();
        assert_eq!(scan(&roots(none.path()), &AgentStore::default()), Scan::default());
    }

    #[test]
    fn claude_incremental() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let f = home.join(".claude").join("projects").join("p1").join("s1.jsonl");
        put(
            &f,
            &[format!(
                r#"{{"type":"assistant","timestamp":"{T1}","cwd":"c:/dev/x","message":{{"content":[{{"type":"tool_use","name":"mcp__a__b","input":{{}}}}]}}}}"#
            )],
        );
        let r = roots(home);
        let s1 = scan(&r, &AgentStore::default());
        assert_eq!(s1.events.len(), 1);
        let store = AgentStore {
            cursors: s1.cursors,
            ..AgentStore::default()
        };
        // Nothing new: no events, and the file keeps its first time.
        let s2 = scan(&r, &store);
        assert!(s2.events.is_empty());
        assert_eq!(s2.cursors[&f.display().to_string()].first_ms, Some(ms(T1)));

        let mut h = std::fs::OpenOptions::new().append(true).open(&f).unwrap();
        // A line with no marker is skipped once the first time is known; a half line waits.
        writeln!(
            h,
            r#"{{"type":"user","timestamp":"{T3}","message":{{"content":"hi mcp__skipped__x"}}}}"#
        )
        .unwrap();
        writeln!(
            h,
            r#"{{"type":"assistant","timestamp":"{T3}","cwd":"c:/dev/y","message":{{"content":[{{"type":"tool_use","name":"mcp__c__d","input":{{}}}}]}}}}"#
        )
        .unwrap();
        write!(h, r#"{{"type":"assistant","timestamp":"{T3}","mess"#).unwrap();
        drop(h);
        let store = AgentStore {
            cursors: s2.cursors,
            ..AgentStore::default()
        };
        let s3 = scan(&r, &store);
        assert_eq!(
            s3.events,
            vec![ev(
                EventKind::Used,
                UseKey::Tool("mcp__c__d".into()),
                T3,
                Some("c:/dev/y")
            )]
        );
        assert_eq!(s3.cursors[&f.display().to_string()].first_ms, Some(ms(T1)));
    }

    #[test]
    fn claude_skill_usage_json() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".claude.json"),
            r#"{"skillUsage":{"marshell-ship":{"usageCount":3,"lastUsedAt":1762000000000},"bad":{"usageCount":1}}}"#,
        )
        .unwrap();
        let s = scan(&roots(tmp.path()), &AgentStore::default());
        assert_eq!(
            s.events,
            vec![Event {
                // A last use with no place: it raises the last use and isn't a place (7c2 final review).
                kind: EventKind::LastUse,
                key: UseKey::SkillName("marshell-ship".into()),
                at_ms: 1_762_000_000_000,
                cwd: None,
            }]
        );
        // It says nothing about coverage.
        assert!(s.cursors.is_empty());
    }

    #[test]
    fn claude_history_folder_that_cant_be_listed() {
        let tmp = tempfile::tempdir().unwrap();
        // A "folder" that is a file can't be listed.
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::write(tmp.path().join(".claude/projects"), "x").unwrap();
        let s = scan(&roots(tmp.path()), &AgentStore::default());
        assert_eq!(s.unreadable.as_deref(), Some("Its history folder can't be listed."));
        assert!(s.events.is_empty());
    }
}
