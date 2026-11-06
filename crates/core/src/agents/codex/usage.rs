//! Codex CLI's history, read for the Doctor's usage items: MCP tool calls and skill reads. Reads its thread database
//! read-only and the new lines of its rollout files, and never writes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::agents::common::rfc3339_ms;
use crate::doctor::ext::ExtRoots;
use crate::doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines, norm_path, skill_md_paths};

const BAD_DB: &str = "Its history database isn't in a form Marshell reads.";
/// How far behind the cursor each scan of the history database reads again.
const LATE_ROWS_MS: u64 = 24 * 60 * 60 * 1000;

/// A rollout line is worth parsing only when it holds one of these (or while the file's first time is still unknown).
const MARKERS: [&str; 6] = [
    "McpToolCall",
    "CommandExecution",
    "custom_tool_call",
    "function_call",
    "session_meta",
    "turn_context",
];

fn event(key: UseKey, at_ms: u64, cwd: &Option<String>) -> Event {
    Event {
        kind: EventKind::Used,
        key,
        at_ms,
        cwd: cwd.clone(),
    }
}

/// Every string anywhere in a JSON value.
fn strings<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

/// A command given as one string or as an argv list.
fn command_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => Some(a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")),
        _ => None,
    }
}

fn push_skill_files(paths: Vec<String>, at: u64, cwd: &Option<String>, out: &mut Vec<Event>) {
    let mut seen: Vec<String> = Vec::new();
    for p in paths {
        if !seen.contains(&p) {
            out.push(event(UseKey::SkillFile(p.clone()), at, cwd));
            seen.push(p);
        }
    }
}

/// The `<path>` of each `<skill>` block in a message.
fn skill_block_paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find("<skill>") {
        let start = from + rel + "<skill>".len();
        let end = text[start..].find("</skill>").map_or(text.len(), |e| start + e);
        let mut block = &text[start..end];
        while let Some(p) = block.find("<path>") {
            let rest = &block[p + "<path>".len()..];
            let Some(close) = rest.find("</path>") else { break };
            let path = rest[..close].trim();
            if !path.is_empty() {
                out.push(norm_path(path));
            }
            block = &rest[close..];
        }
        from = end;
    }
    out
}

/// The text of a database column as a string, whatever its stored type.
fn text_at(row: &rusqlite::Row, i: usize) -> rusqlite::Result<Option<String>> {
    Ok(match row.get_ref(i)? {
        ValueRef::Text(t) | ValueRef::Blob(t) => Some(String::from_utf8_lossy(t).into_owned()),
        _ => None,
    })
}

fn open_read_only(path: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(Duration::from_secs(2))?;
    Ok(c)
}

/// Fails with `InvalidQuery` unless `table` has every one of `wanted`.
fn require_columns(c: &Connection, table: &str, wanted: &[&str]) -> rusqlite::Result<()> {
    let mut st = c.prepare(&format!("PRAGMA table_info({table})"))?;
    let have: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    if wanted.iter().all(|w| have.iter().any(|h| h == w)) {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

/// Thread id to its folder, without the `\\?\` prefix. No state database means no folders.
fn thread_folders(state: &Path) -> rusqlite::Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    if !state.exists() {
        return Ok(map);
    }
    let c = open_read_only(state)?;
    require_columns(&c, "threads", &["id", "cwd"])?;
    let mut st = c.prepare("SELECT id, cwd FROM threads")?;
    let mut rows = st.query([])?;
    while let Some(row) = rows.next()? {
        if let (Some(id), Some(cwd)) = (text_at(row, 0)?, text_at(row, 1)?) {
            let bare = cwd.strip_prefix("\\\\?\\").unwrap_or(&cwd).to_string();
            map.insert(id, bare);
        }
    }
    Ok(map)
}

fn db_item_events(kind: &str, item: &Value, at: u64, cwd: &Option<String>, out: &mut Vec<Event>) {
    match kind {
        "mcpToolCall" => {
            if let Some(server) = item.get("server").and_then(Value::as_str) {
                out.push(event(UseKey::Server(server.to_string()), at, cwd));
            }
        }
        "commandExecution" => {
            if let Some(text) = item.get("command").and_then(command_text) {
                push_skill_files(skill_md_paths(&text), at, cwd, out);
            }
        }
        "userMessage" => {
            let mut all = Vec::new();
            strings(item, &mut all);
            let paths = all.into_iter().flat_map(skill_block_paths).collect();
            push_skill_files(paths, at, cwd, out);
        }
        _ => {}
    }
}

/// Reads the thread database from the cursor on, read-only. Any SQLite error or an unexpected schema is an `Err`.
fn read_db(db: &Path, state: &Path, old: Option<&Cursor>) -> rusqlite::Result<(Vec<Event>, Cursor)> {
    let folders = thread_folders(state)?;
    let c = open_read_only(db)?;
    require_columns(
        &c,
        "thread_items",
        &["thread_id", "created_at_ms", "item_type", "item_json"],
    )?;
    let (min, max): (Option<i64>, Option<i64>) = c.query_row(
        "SELECT MIN(created_at_ms), MAX(created_at_ms) FROM thread_items",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let to_u64 = |v: i64| v.max(0) as u64;
    let old_to = old.map_or(0, |o| o.read_to);
    // Codex writes a row when its item ends, dated when it started, so a row can land behind the cursor: the last
    // day before it is read again (merging a use twice changes nothing). A rebuilt database that now ends before the
    // cursor is read from the start.
    let (start, floor) = match max {
        Some(m) if to_u64(m) >= old_to => (old_to.saturating_sub(LATE_ROWS_MS), old_to),
        _ => (0, 0),
    };
    let mut st = c.prepare(
        "SELECT thread_id, created_at_ms, item_type, item_json FROM thread_items \
         WHERE created_at_ms >= ?1 AND item_type IN ('mcpToolCall','commandExecution','userMessage') \
         ORDER BY created_at_ms, rowid",
    )?;
    let mut rows = st.query([start as i64])?;
    let mut events = Vec::new();
    let mut read_to = floor;
    while let Some(row) = rows.next()? {
        let at = to_u64(row.get::<_, i64>(1)?);
        read_to = read_to.max(at);
        let (Some(kind), Some(json)) = (text_at(row, 2)?, text_at(row, 3)?) else {
            continue;
        };
        let Ok(item) = serde_json::from_str::<Value>(&json) else {
            continue;
        };
        let cwd = text_at(row, 0)?.and_then(|t| folders.get(&t).cloned());
        db_item_events(&kind, &item, at, &cwd, &mut events);
    }
    let cursor = Cursor {
        len: 0,
        read_to,
        first_ms: min.map(to_u64),
        cwd: None,
    };
    Ok((events, cursor))
}

/// Every `rollout-*.jsonl` under `dir`, in date folders; compressed ones are left out.
fn rollout_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            if depth < 8 {
                rollout_files(&p, depth + 1, out);
            }
        } else if p.is_file()
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
        {
            out.push(p);
        }
    }
}

fn rollout_events(rec: &Value, at: u64, cwd: &Option<String>, out: &mut Vec<Event>) {
    let Some(payload) = rec.get("payload") else { return };
    match rec.get("type").and_then(Value::as_str) {
        Some("event_msg") => {
            let Some(item) = payload.get("item") else { return };
            match item.get("type").and_then(Value::as_str) {
                Some("McpToolCall") => {
                    if let Some(server) = item.get("server").and_then(Value::as_str) {
                        out.push(event(UseKey::Server(server.to_string()), at, cwd));
                    }
                }
                Some("CommandExecution") => {
                    if let Some(text) = item.get("command").and_then(command_text) {
                        push_skill_files(skill_md_paths(&text), at, cwd, out);
                    }
                }
                _ => {}
            }
        }
        // Other `response_item`s are never searched: `<skills_instructions>` lists every skill.
        Some("response_item") => {
            let field = match payload.get("type").and_then(Value::as_str) {
                Some("custom_tool_call") => "input",
                Some("function_call") => "arguments",
                _ => return,
            };
            let Some(text) = payload.get(field).and_then(Value::as_str) else {
                return;
            };
            // Arguments are JSON text, so a path inside has its backslashes doubled; read the strings it holds.
            let paths = match serde_json::from_str::<Value>(text) {
                Ok(v) if v.is_object() || v.is_array() => {
                    let mut all = Vec::new();
                    strings(&v, &mut all);
                    all.into_iter().flat_map(skill_md_paths).collect()
                }
                _ => skill_md_paths(text),
            };
            push_skill_files(paths, at, cwd, out);
        }
        _ => {}
    }
}

/// Reads the new lines of one rollout. `None` when the file can't be read now.
fn read_rollout(path: &Path, old: Option<&Cursor>) -> Option<(Vec<Event>, Cursor)> {
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
        if matches!(
            rec.get("type").and_then(Value::as_str),
            Some("session_meta" | "turn_context")
        ) && let Some(c) = rec.pointer("/payload/cwd").and_then(Value::as_str)
        {
            cur.cwd = Some(c.to_string());
        }
        if let Some(at) = at {
            rollout_events(&rec, at, &cur.cwd, &mut events);
        }
    }
    Some((events, cur))
}

/// One scan of Codex's history: its thread database (when there is one) and its rollouts, each from where `store` left
/// off. A database that exists but can't be read as described makes the whole scan unreadable, with no events.
pub fn scan(roots: &ExtRoots, store: &AgentStore) -> Scan {
    let home = &roots.codex_home;
    let unreadable = |why: &str| Scan {
        events: Vec::new(),
        cursors: store.cursors.clone(),
        unreadable: Some(why.to_string()),
    };
    let mut out = Scan::default();
    let db = home.join("thread_history_1.sqlite");
    if db.exists() {
        let key = format!("sqlite:{}", db.display());
        match read_db(&db, &home.join("state_5.sqlite"), store.cursors.get(&key)) {
            Ok((events, cur)) => {
                out.events.extend(events);
                out.cursors.insert(key, cur);
            }
            Err(_) => return unreadable(BAD_DB),
        }
    }
    let sessions = home.join("sessions");
    match std::fs::read_dir(&sessions) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return unreadable("Its history folder can't be listed."),
    }
    let mut files = Vec::new();
    rollout_files(&sessions, 0, &mut files);
    for file in files {
        let key = file.display().to_string();
        if let Some((events, cur)) = read_rollout(&file, store.cursors.get(&key)) {
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
    use crate::doctor::usage::{Event, EventKind, UseKey, norm_path};
    use rusqlite::Connection;
    use serde_json::json;
    use std::io::Write;
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

    fn ev(kind: EventKind, key: UseKey, at: u64, cwd: Option<&str>) -> Event {
        Event {
            kind,
            key,
            at_ms: at,
            cwd: cwd.map(String::from),
        }
    }

    const T1: &str = "2026-11-01T10:00:00.000Z";
    const T2: &str = "2026-11-01T09:00:00.000Z";
    const T3: &str = "2026-11-02T10:00:00.000Z";
    const T4: &str = "2026-11-03T10:00:00.000Z";

    const FOO: &str = "c:/users/home office/.codex/skills/foo/skill.md";

    fn codex_home(h: &Path) -> std::path::PathBuf {
        let d = h.join(".codex");
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn history_db(h: &Path, items_sql: &str) -> Connection {
        let c = Connection::open(codex_home(h).join("thread_history_1.sqlite")).unwrap();
        c.execute_batch(items_sql).unwrap();
        c
    }

    const ITEMS: &str = "CREATE TABLE thread_items (thread_id TEXT NOT NULL, turn_id TEXT NOT NULL, \
        item_id TEXT NOT NULL, rollout_ordinal INTEGER, created_at_ms INTEGER NOT NULL, item_json TEXT NOT NULL, \
        item_type TEXT NOT NULL, PRIMARY KEY (thread_id, turn_id, item_id));";

    fn state_db(h: &Path) {
        let c = Connection::open(codex_home(h).join("state_5.sqlite")).unwrap();
        c.execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY, cwd TEXT NOT NULL, title TEXT);")
            .unwrap();
        c.execute("INSERT INTO threads (id, cwd) VALUES ('t1', ?1)", [r"\\?\C:\dev\x"])
            .unwrap();
        c.execute("INSERT INTO threads (id, cwd) VALUES ('t2', ?1)", [r"\\?\C:\dev\y"])
            .unwrap();
    }

    fn put_item(c: &Connection, n: u32, thread: &str, at: u64, kind: &str, json: serde_json::Value) {
        c.execute(
            "INSERT INTO thread_items (thread_id, turn_id, item_id, created_at_ms, item_json, item_type) \
             VALUES (?1, 'turn', ?2, ?3, ?4, ?5)",
            rusqlite::params![thread, format!("i{n}"), at as i64, json.to_string(), kind],
        )
        .unwrap();
    }

    fn db_key(h: &Path) -> String {
        format!("sqlite:{}", h.join(".codex").join("thread_history_1.sqlite").display())
    }

    #[test]
    fn codex_db_events() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        state_db(home);
        let c = history_db(home, ITEMS);
        let (a, b, d, e) = (ms(T2), ms(T1), ms(T3), ms(T4));
        put_item(
            &c,
            1,
            "t1",
            a,
            "mcpToolCall",
            json!({"type":"mcpToolCall","server":"gh","tool":"list","result":{"text":"SKILL.md"}}),
        );
        put_item(
            &c,
            2,
            "t1",
            b,
            "commandExecution",
            json!({"type":"commandExecution","command":r"Get-Content 'C:\Users\Home Office\.codex\skills\foo\SKILL.md'","aggregatedOutput":""}),
        );
        put_item(
            &c,
            3,
            "t2",
            b,
            "commandExecution",
            json!({"type":"commandExecution","command":"Get-Content -LiteralPath 'C:/dev/s/bar/SKILL.md'"}),
        );
        // The output alone holds a SKILL.md path: not a use.
        put_item(
            &c,
            4,
            "t2",
            b,
            "commandExecution",
            json!({"type":"commandExecution","command":"dir","aggregatedOutput":r"C:\x\nope\SKILL.md"}),
        );
        put_item(
            &c,
            5,
            "t2",
            d,
            "userMessage",
            json!({"type":"userMessage","content":[{"type":"text","text":r"hi <skill><name>baz</name><path>C:\x\baz\SKILL.md</path></skill>"}]}),
        );
        put_item(
            &c,
            6,
            "t2",
            d,
            "userMessage",
            json!({"type":"userMessage","content":[{"type":"text","text":r"look at C:\x\no\SKILL.md"}]}),
        );
        // A kind the scan doesn't ask for, and a thread the state database doesn't know.
        put_item(
            &c,
            7,
            "t1",
            d,
            "agentMessage",
            json!({"type":"agentMessage","server":"nope","command":r"C:\x\no2\SKILL.md"}),
        );
        put_item(
            &c,
            8,
            "gone",
            e,
            "mcpToolCall",
            json!({"type":"mcpToolCall","server":"codex_apps"}),
        );
        drop(c);

        let r = roots(home);
        let s = scan(&r, &AgentStore::default());
        let x = Some(r"C:\dev\x");
        let y = Some(r"C:\dev\y");
        let want = vec![
            ev(EventKind::Used, UseKey::Server("gh".into()), a, x),
            ev(EventKind::Used, UseKey::SkillFile(FOO.into()), b, x),
            ev(EventKind::Used, UseKey::SkillFile("c:/dev/s/bar/skill.md".into()), b, y),
            ev(EventKind::Used, UseKey::SkillFile("c:/x/baz/skill.md".into()), d, y),
            ev(EventKind::Used, UseKey::Server("codex_apps".into()), e, None),
        ];
        assert_eq!(s.events, want);
        assert_eq!(s.unreadable, None);
        assert_eq!(norm_path(s.events[0].cwd.as_deref().unwrap()), "c:/dev/x");
        let cur = &s.cursors[&db_key(home)];
        assert_eq!(s.cursors.len(), 1);
        assert_eq!(cur.read_to, e);
        assert_eq!(cur.first_ms, Some(a));

        // A second scan from the cursor repeats only the rows of the day before it (d is exactly a day before e).
        let store = AgentStore {
            cursors: s.cursors,
            ..AgentStore::default()
        };
        let s2 = scan(&r, &store);
        assert_eq!(
            s2.events,
            vec![
                ev(EventKind::Used, UseKey::SkillFile("c:/x/baz/skill.md".into()), d, y),
                ev(EventKind::Used, UseKey::Server("codex_apps".into()), e, None),
            ]
        );
        assert_eq!(s2.cursors[&db_key(home)].first_ms, Some(a));

        // A new row after the cursor is found.
        let c = Connection::open(home.join(".codex").join("thread_history_1.sqlite")).unwrap();
        let later = e + 1000;
        put_item(
            &c,
            9,
            "t1",
            later,
            "mcpToolCall",
            json!({"type":"mcpToolCall","server":"new"}),
        );
        drop(c);
        let store = AgentStore {
            cursors: s2.cursors,
            ..AgentStore::default()
        };
        let s3 = scan(&r, &store);
        assert_eq!(
            s3.events,
            vec![
                ev(EventKind::Used, UseKey::SkillFile("c:/x/baz/skill.md".into()), d, y),
                ev(EventKind::Used, UseKey::Server("codex_apps".into()), e, None),
                ev(EventKind::Used, UseKey::Server("new".into()), later, x),
            ]
        );
        assert_eq!(s3.cursors[&db_key(home)].read_to, later);

        // No Codex folder: no history, and not an error.
        let none = tempfile::tempdir().unwrap();
        assert_eq!(scan(&roots(none.path()), &AgentStore::default()), Scan::default());
    }

    // Found in the 7c2 final review: Codex writes a row when its item ends, dated when it started, so a slow MCP call
    // can land after a scan whose cursor is already past its date.
    #[test]
    fn codex_db_rows_that_land_late_are_read() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        state_db(home);
        let c = history_db(home, ITEMS);
        let end = ms(T4);
        put_item(
            &c,
            1,
            "t1",
            end,
            "commandExecution",
            json!({"type":"commandExecution","command":"dir"}),
        );
        drop(c);
        let r = roots(home);
        let s = scan(&r, &AgentStore::default());
        assert_eq!(s.cursors[&db_key(home)].read_to, end);

        // The slow call started 10 minutes before the row the scan ended on, and lands now.
        let c = Connection::open(home.join(".codex").join("thread_history_1.sqlite")).unwrap();
        put_item(
            &c,
            2,
            "t2",
            end - 600_000,
            "mcpToolCall",
            json!({"type":"mcpToolCall","server":"slow"}),
        );
        drop(c);
        let store = AgentStore {
            cursors: s.cursors,
            ..AgentStore::default()
        };
        let s2 = scan(&r, &store);
        assert!(
            s2.events.contains(&ev(
                EventKind::Used,
                UseKey::Server("slow".into()),
                end - 600_000,
                Some(r"C:\dev\y")
            )),
            "{:?}",
            s2.events
        );
        assert_eq!(s2.cursors[&db_key(home)].read_to, end, "the cursor doesn't move back");

        // With nothing new, the cursor stays where it was, scan after scan.
        let store = AgentStore {
            cursors: s2.cursors,
            ..AgentStore::default()
        };
        assert_eq!(scan(&r, &store).cursors[&db_key(home)].read_to, end);
    }

    #[test]
    fn codex_db_wrong_schema_is_unreadable() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        state_db(home);
        let c = history_db(
            home,
            "CREATE TABLE thread_items (thread_id TEXT, created_at_ms INTEGER, item_type TEXT);
             INSERT INTO thread_items VALUES ('t1', 1762000000000, 'mcpToolCall');",
        );
        drop(c);
        // A readable rollout beside it: nothing half-read may reach the store.
        let rollout = home.join(".codex/sessions/2026/11/01/rollout-a.jsonl");
        std::fs::create_dir_all(rollout.parent().unwrap()).unwrap();
        let line = json!({"timestamp":T1,"type":"event_msg","payload":{"type":"item_completed","item":{"type":"McpToolCall","server":"gh"}}});
        std::fs::write(&rollout, format!("{line}\n")).unwrap();

        let mut old = AgentStore::default();
        old.cursors.insert("keep".into(), Default::default());
        let s = scan(&roots(home), &old);
        assert_eq!(
            s.unreadable.as_deref(),
            Some("Its history database isn't in a form Marshell reads.")
        );
        assert!(s.events.is_empty());
        assert_eq!(s.cursors, old.cursors, "coverage is left alone");

        // The state database without a cwd column is just as unreadable.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let c = Connection::open(codex_home(home).join("state_5.sqlite")).unwrap();
        c.execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY);").unwrap();
        drop(c);
        drop(history_db(home, ITEMS));
        let s = scan(&roots(home), &AgentStore::default());
        assert!(s.unreadable.is_some());
        assert!(s.events.is_empty());

        // Not a database at all.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            codex_home(tmp.path()).join("thread_history_1.sqlite"),
            "not sqlite at all",
        )
        .unwrap();
        let s = scan(&roots(tmp.path()), &AgentStore::default());
        assert!(s.unreadable.is_some());
    }

    fn rollout_line(t: &str, kind: &str, payload: serde_json::Value) -> String {
        json!({"timestamp":t,"ordinal":1,"type":kind,"payload":payload}).to_string()
    }

    #[test]
    fn codex_rollout_events() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let f = home
            .join(".codex")
            .join("sessions")
            .join("2026")
            .join("11")
            .join("01")
            .join("rollout-2026-11-01T10-00-00-abc.jsonl");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        let foo_cmd = r"Get-Content 'C:\Users\Home Office\.codex\skills\foo\SKILL.md'";
        let lines = vec![
            rollout_line(T1, "session_meta", json!({"id":"abc","cwd":r"C:\dev\x"})),
            // The skill list in every session: not use.
            rollout_line(
                T1,
                "response_item",
                json!({"type":"message","role":"developer","content":[{"type":"input_text","text":
                    r"<skills_instructions>- foo: x (file: C:\Users\Home Office\.codex\skills\foo\SKILL.md)</skills_instructions>"}]}),
            ),
            rollout_line(
                T1,
                "event_msg",
                json!({"type":"item_completed","item":{"type":"McpToolCall","server":"gh","tool":"list"}}),
            ),
            // Out of time order.
            rollout_line(
                T2,
                "event_msg",
                json!({"type":"item_completed","item":{"type":"McpToolCall","server":"older"}}),
            ),
            rollout_line(T3, "turn_context", json!({"cwd":r"C:\dev\y","model":"m"})),
            rollout_line(
                T3,
                "event_msg",
                json!({"type":"item_completed","item":{"type":"CommandExecution","command":["powershell","-Command",foo_cmd],"aggregated_output":""}}),
            ),
            rollout_line(
                T3,
                "event_msg",
                json!({"type":"item_completed","item":{"type":"CommandExecution","command":["cmd","/c","dir"],"aggregated_output":r"C:\x\nope\SKILL.md"}}),
            ),
            rollout_line(
                T3,
                "response_item",
                json!({"type":"custom_tool_call","name":"shell","input":"Get-Content -LiteralPath \"C:/dev/s/bar/SKILL.md\""}),
            ),
            rollout_line(
                T4,
                "response_item",
                json!({"type":"function_call","name":"shell","arguments":json!({"command":r"Get-Content 'C:\Users\Home Office\s\baz\SKILL.md'"}).to_string()}),
            ),
            rollout_line(
                T4,
                "response_item",
                json!({"type":"function_call_output","output":r"C:\x\nope2\SKILL.md"}),
            ),
            "not json but says McpToolCall".to_string(),
        ];
        std::fs::write(&f, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        // Compressed rollouts and other files are skipped.
        std::fs::write(f.with_file_name("rollout-old.jsonl.zst"), "zst").unwrap();
        std::fs::write(f.with_file_name("notes.jsonl"), lines[2].clone() + "\n").unwrap();

        let r = roots(home);
        let s = scan(&r, &AgentStore::default());
        let x = Some(r"C:\dev\x");
        let y = Some(r"C:\dev\y");
        let want = vec![
            ev(EventKind::Used, UseKey::Server("gh".into()), ms(T1), x),
            ev(EventKind::Used, UseKey::Server("older".into()), ms(T2), x),
            ev(EventKind::Used, UseKey::SkillFile(FOO.into()), ms(T3), y),
            ev(
                EventKind::Used,
                UseKey::SkillFile("c:/dev/s/bar/skill.md".into()),
                ms(T3),
                y,
            ),
            ev(
                EventKind::Used,
                UseKey::SkillFile("c:/users/home office/s/baz/skill.md".into()),
                ms(T4),
                y,
            ),
        ];
        assert_eq!(s.events, want);
        assert_eq!(s.unreadable, None);
        assert_eq!(s.cursors.len(), 1);
        let c = &s.cursors[&f.display().to_string()];
        assert_eq!(c.first_ms, Some(ms(T1)));
        assert_eq!(c.cwd.as_deref(), Some(r"C:\dev\y"));

        // Incremental: nothing new gives nothing; an appended line is found, with the folder kept.
        let store = AgentStore {
            cursors: s.cursors,
            ..AgentStore::default()
        };
        let s2 = scan(&r, &store);
        assert!(s2.events.is_empty());
        assert_eq!(s2.cursors[&f.display().to_string()].first_ms, Some(ms(T1)));
        let mut h = std::fs::OpenOptions::new().append(true).open(&f).unwrap();
        writeln!(
            h,
            "{}",
            rollout_line(
                T4,
                "event_msg",
                json!({"type":"item_completed","item":{"type":"McpToolCall","server":"late"}})
            )
        )
        .unwrap();
        drop(h);
        let store = AgentStore {
            cursors: s2.cursors,
            ..AgentStore::default()
        };
        let s3 = scan(&r, &store);
        assert_eq!(
            s3.events,
            vec![ev(EventKind::Used, UseKey::Server("late".into()), ms(T4), y)]
        );
    }
}
