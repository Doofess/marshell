//! Antigravity CLI's history, read for the Doctor's usage items: the `SKILL.md` files its conversations viewed. Reads
//! only what is new in each transcript since the cursor, and never writes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agents::common::rfc3339_ms;
use crate::doctor::ext::ExtRoots;
use crate::doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines, norm_path};

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// Conversation id to workspace, from `history.jsonl`. A missing or broken file, or line, gives nothing.
fn workspaces(history: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(bytes) = std::fs::read(history) else {
        return map;
    };
    for line in String::from_utf8_lossy(&bytes).lines() {
        let Ok(rec) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let (Some(id), Some(ws)) = (
            rec.get("conversationId").and_then(Value::as_str),
            rec.get("workspace").and_then(Value::as_str),
        ) {
            map.insert(id.to_string(), ws.to_string());
        }
    }
    map
}

/// A conversation's transcript: the untruncated one when there is one.
fn transcript(brain_dir: &Path) -> Option<PathBuf> {
    let logs = brain_dir.join(".system_generated").join("logs");
    ["transcript_full.jsonl", "transcript.jsonl"]
        .iter()
        .map(|n| logs.join(n))
        .find(|p| p.is_file())
}

fn record_events(rec: &Value, at: u64, cwd: &Option<String>, out: &mut Vec<Event>) {
    let Some(calls) = rec.get("tool_calls").and_then(Value::as_array) else {
        return;
    };
    for call in calls {
        if call.get("name").and_then(Value::as_str) != Some("view_file") {
            continue;
        }
        // `args` is an object, or JSON text holding one.
        let parsed;
        let args = match call.get("args") {
            Some(Value::String(s)) => {
                parsed = serde_json::from_str::<Value>(s).unwrap_or(Value::Null);
                &parsed
            }
            Some(v) => v,
            None => continue,
        };
        let Some(path) = args.get("AbsolutePath").and_then(Value::as_str) else {
            continue;
        };
        if path.to_ascii_lowercase().ends_with("skill.md") {
            out.push(Event {
                kind: EventKind::Used,
                key: UseKey::SkillFile(norm_path(path)),
                at_ms: at,
                cwd: cwd.clone(),
            });
        }
    }
}

/// Reads the new lines of one transcript. `None` when the file can't be read now.
fn read_file(path: &Path, old: Option<&Cursor>, place: Option<&String>) -> Option<(Vec<Event>, Cursor)> {
    let (lines, mut cur) = new_lines(path, old).ok()?;
    if let Some(p) = place {
        cur.cwd = Some(p.clone());
    }
    let mut events = Vec::new();
    for line in lines {
        if cur.first_ms.is_some() && !line.contains("view_file") {
            continue;
        }
        let Ok(rec) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(at) = rec.get("created_at").and_then(Value::as_str).and_then(rfc3339_ms) else {
            continue;
        };
        cur.first_ms.get_or_insert(at);
        record_events(&rec, at, &cur.cwd, &mut events);
    }
    Some((events, cur))
}

/// One scan of Antigravity's history, reading each transcript from where `store` left off.
pub fn scan(roots: &ExtRoots, store: &AgentStore) -> Scan {
    let base = roots.gemini_dir.join("antigravity-cli");
    let brain = base.join("brain");
    match std::fs::read_dir(&brain) {
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
    let places = workspaces(&base.join("history.jsonl"));
    let mut out = Scan::default();
    for dir in subdirs(&brain) {
        let Some(file) = transcript(&dir) else { continue };
        let id = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let key = file.display().to_string();
        if let Some((events, cur)) = read_file(&file, store.cursors.get(&key), places.get(id)) {
            out.events.extend(events);
            out.cursors.insert(key, cur);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::common::rfc3339_ms;
    use crate::doctor::usage::{Event, EventKind, UseKey};
    use serde_json::json;
    use std::path::Path;

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

    fn ev(key: &str, at: &str, cwd: Option<&str>) -> Event {
        Event {
            kind: EventKind::Used,
            key: UseKey::SkillFile(key.into()),
            at_ms: ms(at),
            cwd: cwd.map(String::from),
        }
    }

    const T1: &str = "2026-10-10T10:00:00.000Z";
    const T2: &str = "2026-10-10T09:00:00.000Z";
    const T3: &str = "2026-10-11T10:00:00.000Z";

    fn logs(h: &Path, id: &str) -> std::path::PathBuf {
        let d = h
            .join(".gemini")
            .join("antigravity-cli")
            .join("brain")
            .join(id)
            .join(".system_generated")
            .join("logs");
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn view(t: &str, name: &str, args: serde_json::Value) -> String {
        json!({"type":"MODEL","source":"model","status":"DONE","step_index":1,"created_at":t,
            "tool_calls":[{"name":name,"args":args}]})
        .to_string()
    }

    fn put(path: &Path, lines: &[String]) {
        std::fs::write(path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    }

    #[test]
    fn antigravity_events() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dir = home.join(".gemini/antigravity-cli");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("history.jsonl"),
            [
                json!({"display":"a","timestamp":1,"workspace":r"C:\dev\x","conversationId":"c1"}).to_string(),
                json!({"display":"b","timestamp":2,"workspace":r"C:\dev\z"}).to_string(),
                "not json".to_string(),
                String::new(),
            ]
            .join("\n"),
        )
        .unwrap();
        put(
            &logs(home, "c1").join("transcript_full.jsonl"),
            &[
                view(
                    T1,
                    "view_file",
                    json!({"AbsolutePath":r"C:\Users\Home Office\.gemini\skills\foo\SKILL.md"}),
                ),
                // Out of time order, a lower-case name, forward slashes.
                view(T2, "view_file", json!({"AbsolutePath":"C:/dev/s/bar/skill.md"})),
                view(T1, "view_file", json!({"AbsolutePath":"C:/dev/readme.md"})),
                view(T1, "run_command", json!({"AbsolutePath":"C:/dev/other/SKILL.md"})),
                view(T1, "view_file", json!({"AbsolutePath":"C:/dev/SKILL.md.bak"})),
                "also not json".to_string(),
            ],
        );
        // No full transcript: the short one is read. An id the history doesn't know has no place.
        put(
            &logs(home, "c2").join("transcript.jsonl"),
            &[view(T3, "view_file", json!({"AbsolutePath":"C:/dev/s/baz/SKILL.md"}))],
        );
        // Both present: only the full one is read.
        put(
            &logs(home, "c3").join("transcript_full.jsonl"),
            &[view(T3, "view_file", json!({"AbsolutePath":"C:/dev/s/full/SKILL.md"}))],
        );
        put(
            &logs(home, "c3").join("transcript.jsonl"),
            &[view(T3, "view_file", json!({"AbsolutePath":"C:/dev/s/short/SKILL.md"}))],
        );
        let s = scan(&roots(home), &AgentStore::default());
        let x = Some(r"C:\dev\x");
        let want = vec![
            ev("c:/users/home office/.gemini/skills/foo/skill.md", T1, x),
            ev("c:/dev/s/bar/skill.md", T2, x),
            ev("c:/dev/s/baz/skill.md", T3, None),
            ev("c:/dev/s/full/skill.md", T3, None),
        ];
        assert_eq!(s.events, want);
        assert_eq!(s.unreadable, None);
        assert_eq!(s.cursors.len(), 3);
        let c1 = logs(home, "c1").join("transcript_full.jsonl");
        let c = &s.cursors[&c1.display().to_string()];
        assert_eq!(c.first_ms, Some(ms(T1)));
        assert_eq!(c.cwd.as_deref(), Some(r"C:\dev\x"));

        // Incremental: nothing new, nothing found.
        let store = AgentStore {
            cursors: s.cursors,
            ..AgentStore::default()
        };
        assert!(scan(&roots(home), &store).events.is_empty());

        // A missing history.jsonl: the events have no place.
        std::fs::remove_file(dir.join("history.jsonl")).unwrap();
        let s = scan(&roots(home), &AgentStore::default());
        assert_eq!(s.events.len(), 4);
        assert!(s.events.iter().all(|e| e.cwd.is_none()));

        // No Antigravity folder: no history, and not an error.
        let none = tempfile::tempdir().unwrap();
        assert_eq!(scan(&roots(none.path()), &AgentStore::default()), Scan::default());
    }
}
