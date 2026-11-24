//! GitHub Copilot CLI's event logs, read for the Doctor's usage items: MCP tool calls and skill uses. Reads only what
//! is new in each `events.jsonl` since the cursor, and never writes. A log holds prompts, tool output and whole skill
//! bodies, so each line is parsed into a struct that names only the fields used here, and nothing else is kept.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::agents::common::rfc3339_ms;
use crate::doctor::ext::ExtRoots;
use crate::doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines, norm_path};

/// A value of the wanted type, or `None` when the field holds something else.
#[derive(Deserialize)]
#[serde(untagged)]
enum Lenient<T> {
    Value(T),
    Other(serde::de::IgnoredAny),
}

fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(match Option::<Lenient<T>>::deserialize(d)? {
        Some(Lenient::Value(v)) => Some(v),
        _ => None,
    })
}

#[derive(Deserialize, Default)]
struct Context {
    #[serde(default, deserialize_with = "lenient")]
    cwd: Option<String>,
}

/// The fields of an event's `data` that the scan reads. The rest (`content` among them) is skipped, not kept.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Data {
    #[serde(default, deserialize_with = "lenient")]
    context: Option<Context>,
    #[serde(default, deserialize_with = "lenient")]
    mcp_server_name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    mcp_config_server_name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    path: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    plugin_name: Option<String>,
}

#[derive(Deserialize)]
struct Line {
    #[serde(default, rename = "type", deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    timestamp: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    data: Option<Data>,
}

/// `session-state/<uuid>/events.jsonl`, for every folder named like a session id.
fn history_files(state: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(state) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(is_uuid) && p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|d| d.join("events.jsonl"))
        .filter(|f| f.is_file())
        .collect()
}

fn is_uuid(name: &str) -> bool {
    let parts: Vec<&str> = name.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn present(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn used(key: UseKey, at_ms: u64, cwd: &Option<String>) -> Event {
    Event {
        kind: EventKind::Used,
        key,
        at_ms,
        cwd: cwd.clone(),
    }
}

/// Reads the new lines of one log. `None` when the file can't be read now.
fn read_file(path: &Path, old: Option<&Cursor>) -> Option<(Vec<Event>, Cursor)> {
    let (lines, mut cur) = new_lines(path, old).ok()?;
    let mut events = Vec::new();
    for text in lines {
        let Ok(line) = serde_json::from_str::<Line>(&text) else {
            continue;
        };
        let at = line.timestamp.as_deref().and_then(rfc3339_ms);
        if let Some(t) = at {
            cur.first_ms.get_or_insert(t);
        }
        let data = line.data.unwrap_or_default();
        match line.kind.as_deref() {
            Some("session.start" | "session.resume") => {
                if let Some(cwd) = data.context.as_ref().and_then(|c| present(&c.cwd)) {
                    cur.cwd = Some(cwd.to_string());
                }
            }
            Some("tool.execution_start") => {
                let server = present(&data.mcp_config_server_name).or_else(|| present(&data.mcp_server_name));
                if let (Some(server), Some(at)) = (server, at) {
                    events.push(used(UseKey::Server(server.to_string()), at, &cur.cwd));
                }
            }
            Some("skill.invoked") => {
                let key = match (present(&data.path), present(&data.name)) {
                    (Some(path), _) => Some(UseKey::SkillFile(norm_path(path))),
                    (None, Some(name)) => Some(UseKey::SkillName(match present(&data.plugin_name) {
                        Some(plugin) => format!("{plugin}:{name}"),
                        None => name.to_string(),
                    })),
                    (None, None) => None,
                };
                if let (Some(key), Some(at)) = (key, at) {
                    events.push(used(key, at, &cur.cwd));
                }
            }
            _ => {}
        }
    }
    Some((events, cur))
}

/// One scan of Copilot CLI's event logs, reading each from where `store` left off.
pub fn scan(roots: &ExtRoots, store: &AgentStore) -> Scan {
    let state = roots.copilot_dir.join("session-state");
    match std::fs::read_dir(&state) {
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
    for file in history_files(&state) {
        let key = file.display().to_string();
        match read_file(&file, store.cursors.get(&key)) {
            Some((events, cur)) => {
                out.events.extend(events);
                cursors.insert(key, cur);
            }
            // A file that can't be opened now keeps the cursor it had.
            None => {
                if let Some(old) = store.cursors.get(&key) {
                    cursors.insert(key, old.clone());
                }
            }
        }
    }
    out.cursors = cursors;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MCP: &str = "00000000-0000-4000-8000-000000000274";
    const SKILL: &str = "00000000-0000-4000-8000-000000000251";
    const FIXTURE_CWD: &str = r"C:\Users\user\AppData\Local\Temp\copilot-6n3-t1\proj";
    const FIXTURE_SKILL: &str =
        r"C:\Users\user\AppData\Local\Temp\copilot-6n3-t1\proj\.github\skills\good-skill\SKILL.md";
    const T1: &str = "2026-11-01T10:00:00.000Z";
    const T2: &str = "2026-11-01T11:00:00.000Z";
    const T3: &str = "2026-11-01T12:00:00.000Z";
    const T4: &str = "2026-11-01T13:00:00.000Z";

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
            copilot_dir: h.join(".copilot"),
            copilot_home_set: false,
            copilot_offline: false,
        }
    }

    fn ms(s: &str) -> u64 {
        rfc3339_ms(s).unwrap()
    }

    fn fixture(id: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/copilot/transcripts/1.0.106")
            .join(format!("{id}.jsonl"));
        std::fs::read_to_string(p).unwrap()
    }

    /// `<home>/.copilot/session-state/<folder>/events.jsonl` with `text`.
    fn put(home: &Path, folder: &str, text: &str) -> PathBuf {
        let file = home
            .join(".copilot")
            .join("session-state")
            .join(folder)
            .join("events.jsonl");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, text).unwrap();
        file
    }

    fn lines(l: &[String]) -> String {
        l.iter().map(|l| format!("{l}\n")).collect()
    }

    fn rec(ty: &str, at: &str, data: &str) -> String {
        format!(r#"{{"type":"{ty}","data":{data},"id":"i","timestamp":"{at}"}}"#)
    }

    fn start(at: &str, cwd: &str) -> String {
        rec("session.start", at, &format!(r#"{{"context":{{"cwd":"{cwd}"}}}}"#))
    }

    fn call(at: &str, data: &str) -> String {
        rec("tool.execution_start", at, data)
    }

    fn ev(key: UseKey, at: &str, cwd: Option<&str>) -> Event {
        Event {
            kind: EventKind::Used,
            key,
            at_ms: ms(at),
            cwd: cwd.map(str::to_string),
        }
    }

    fn scan_of(home: &Path) -> Scan {
        scan(&roots(home), &AgentStore::default())
    }

    #[test]
    fn an_mcp_call_is_a_use_of_its_server_where_the_session_ran() {
        let tmp = tempfile::tempdir().unwrap();
        let file = put(tmp.path(), MCP, &fixture(MCP));
        let s = scan_of(tmp.path());
        assert_eq!(
            s.events,
            vec![ev(
                UseKey::Server("scratch-echo".into()),
                "2026-11-24T08:30:02.149Z",
                Some(FIXTURE_CWD)
            )]
        );
        assert_eq!(s.unreadable, None);
        let c = &s.cursors[&file.display().to_string()];
        assert_eq!(c.first_ms, Some(ms("2026-11-24T08:30:00.108Z")));
        assert_eq!(c.read_to, std::fs::metadata(&file).unwrap().len());
    }

    #[test]
    fn a_skill_use_is_keyed_by_its_file() {
        let tmp = tempfile::tempdir().unwrap();
        put(tmp.path(), SKILL, &fixture(SKILL));
        let s = scan_of(tmp.path());
        assert_eq!(
            s.events,
            vec![ev(
                UseKey::SkillFile(norm_path(FIXTURE_SKILL)),
                "2026-11-24T08:30:22.549Z",
                Some(FIXTURE_CWD)
            )]
        );
    }

    #[test]
    fn only_folders_named_like_a_session_are_read() {
        let tmp = tempfile::tempdir().unwrap();
        let text = fixture(MCP);
        put(tmp.path(), ".session-operation-locks", &text);
        put(tmp.path(), "not-a-session", &text);
        // A file with a session's name is no folder.
        let state = tmp.path().join(".copilot").join("session-state");
        std::fs::write(state.join(SKILL), &text).unwrap();
        assert_eq!(scan_of(tmp.path()), Scan::default());
        let good = put(tmp.path(), &MCP.to_uppercase(), &text);
        let s = scan_of(tmp.path());
        assert_eq!(s.events.len(), 1);
        assert_eq!(s.cursors.keys().collect::<Vec<_>>(), vec![&good.display().to_string()]);
    }

    #[test]
    fn the_server_is_named_by_the_config_key_else_the_server_name() {
        let tmp = tempfile::tempdir().unwrap();
        put(
            tmp.path(),
            MCP,
            &lines(&[
                start(T1, "c:/dev/x"),
                call(T2, r#"{"mcpServerName":"plug-srv","mcpConfigServerName":"srv"}"#),
                call(T3, r#"{"mcpServerName":"only"}"#),
                call(T4, r#"{"mcpServerName":"fallback","mcpConfigServerName":""}"#),
            ]),
        );
        let s = scan_of(tmp.path());
        assert_eq!(
            s.events,
            vec![
                ev(UseKey::Server("srv".into()), T2, Some("c:/dev/x")),
                ev(UseKey::Server("only".into()), T3, Some("c:/dev/x")),
                ev(UseKey::Server("fallback".into()), T4, Some("c:/dev/x")),
            ]
        );
    }

    #[test]
    fn a_call_with_no_server_and_other_events_give_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        put(
            tmp.path(),
            MCP,
            &lines(&[
                start(T1, "c:/dev/x"),
                call(T2, r#"{"toolName":"bash"}"#),
                call(T2, r#"{"mcpServerName":""}"#),
                call(T2, r#"{"mcpServerName":"  ","mcpConfigServerName":null}"#),
                call(T2, r#"{"mcpServerName":7}"#),
                // The assistant's request names the server too, and is not the call.
                rec(
                    "assistant.message",
                    T3,
                    r#"{"toolRequests":[{"name":"s-t","mcpServerName":"srv"}],"mcpServerName":"srv"}"#,
                ),
                rec("tool.execution_complete", T3, r#"{"mcpServerName":"srv"}"#),
            ]),
        );
        let s = scan_of(tmp.path());
        assert!(s.events.is_empty(), "{:?}", s.events);
        assert_eq!(s.cursors.len(), 1);
    }

    #[test]
    fn a_skill_with_no_path_is_keyed_by_its_name_with_the_plugin_in_front() {
        let tmp = tempfile::tempdir().unwrap();
        put(
            tmp.path(),
            MCP,
            &lines(&[
                rec("skill.invoked", T1, r#"{"name":"ship","pluginName":null}"#),
                rec(
                    "skill.invoked",
                    T2,
                    r#"{"name":"deploy","path":"","pluginName":"acme"}"#,
                ),
                rec("skill.invoked", T3, r#"{"name":"plain","pluginName":""}"#),
                rec("skill.invoked", T4, r#"{"pluginName":"acme"}"#),
                rec("skill.invoked", T4, r#"{"name":"  "}"#),
            ]),
        );
        let s = scan_of(tmp.path());
        assert_eq!(
            s.events,
            vec![
                ev(UseKey::SkillName("ship".into()), T1, None),
                ev(UseKey::SkillName("acme:deploy".into()), T2, None),
                ev(UseKey::SkillName("plain".into()), T3, None),
            ]
        );
    }

    #[test]
    fn a_resumed_session_changes_the_folder_of_what_follows() {
        let tmp = tempfile::tempdir().unwrap();
        let data = r#"{"mcpConfigServerName":"srv"}"#;
        put(
            tmp.path(),
            MCP,
            &lines(&[
                start(T1, "c:/dev/a"),
                call(T2, data),
                rec(
                    "session.resume",
                    T3,
                    r#"{"resumeTime":"x","context":{"cwd":"c:/dev/b"}}"#,
                ),
                call(T4, data),
                // A resume that names no folder leaves it as it was.
                rec("session.resume", T4, r#"{"context":{}}"#),
                call(T4, data),
            ]),
        );
        let s = scan_of(tmp.path());
        let srv = || UseKey::Server("srv".into());
        assert_eq!(
            s.events,
            vec![
                ev(srv(), T2, Some("c:/dev/a")),
                ev(srv(), T4, Some("c:/dev/b")),
                ev(srv(), T4, Some("c:/dev/b")),
            ]
        );
        assert_eq!(s.cursors.values().next().unwrap().cwd.as_deref(), Some("c:/dev/b"));
    }

    #[test]
    fn lines_that_are_not_events_are_skipped_and_the_rest_still_read() {
        let tmp = tempfile::tempdir().unwrap();
        let data = r#"{"mcpConfigServerName":"srv"}"#;
        put(
            tmp.path(),
            MCP,
            &lines(&[
                "not json".to_string(),
                "[1,2]".to_string(),
                "42".to_string(),
                "".to_string(),
                r#"{"type":5,"timestamp":7,"data":"x"}"#.to_string(),
                r#"{"type":"tool.execution_start","data":"x","timestamp":"2026-11-01T10:00:00.000Z"}"#.to_string(),
                r#"{"type":"tool.execution_start","data":{"mcpConfigServerName":"srv"}}"#.to_string(),
                r#"{"type":"tool.execution_start","data":{"mcpConfigServerName":"srv"},"timestamp":"yesterday"}"#
                    .to_string(),
                call(T1, data),
            ]),
        );
        let s = scan_of(tmp.path());
        // A call with no usable time gives nothing; the one with a time does.
        assert_eq!(s.events, vec![ev(UseKey::Server("srv".into()), T1, None)]);
        assert_eq!(s.cursors.values().next().unwrap().first_ms, Some(ms(T1)));
    }

    #[test]
    fn a_second_scan_reads_only_what_was_appended_and_whole_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let data = r#"{"mcpConfigServerName":"srv"}"#;
        let file = put(tmp.path(), MCP, &lines(&[start(T1, "c:/dev/x"), call(T2, data)]));
        let first = scan_of(tmp.path());
        assert_eq!(first.events.len(), 1);
        let key = file.display().to_string();
        let store = |s: Scan| AgentStore {
            cursors: s.cursors,
            ..AgentStore::default()
        };

        // Nothing new: nothing read.
        let second = scan(&roots(tmp.path()), &store(first.clone()));
        assert!(second.events.is_empty());
        assert_eq!(second.cursors[&key], first.cursors[&key]);

        // Appended lines give only their events, with the folder the file's start named.
        let mut text = std::fs::read_to_string(&file).unwrap();
        text.push_str(&format!("{}\n", call(T3, r#"{"mcpConfigServerName":"two"}"#)));
        // The last line has no newline yet.
        let half = call(T4, r#"{"mcpConfigServerName":"three"}"#);
        text.push_str(&half);
        std::fs::write(&file, &text).unwrap();
        let third = scan(&roots(tmp.path()), &store(second));
        assert_eq!(
            third.events,
            vec![ev(UseKey::Server("two".into()), T3, Some("c:/dev/x"))]
        );
        assert_eq!(third.cursors[&key].first_ms, Some(ms(T1)));

        // Once it has its newline it is read, once.
        std::fs::write(&file, format!("{text}\n")).unwrap();
        let fourth = scan(&roots(tmp.path()), &store(third));
        assert_eq!(
            fourth.events,
            vec![ev(UseKey::Server("three".into()), T4, Some("c:/dev/x"))]
        );
        let len = std::fs::metadata(&file).unwrap().len();
        assert_eq!(fourth.cursors[&key].read_to, len);
        assert!(scan(&roots(tmp.path()), &store(fourth)).events.is_empty());
    }

    #[test]
    fn a_file_that_shrank_is_read_again_from_its_start() {
        let tmp = tempfile::tempdir().unwrap();
        let data = r#"{"mcpConfigServerName":"srv"}"#;
        let file = put(
            tmp.path(),
            MCP,
            &lines(&[start(T1, "c:/dev/x"), call(T2, data), call(T3, data)]),
        );
        let first = scan_of(tmp.path());
        assert_eq!(first.events.len(), 2);
        std::fs::write(&file, lines(&[start(T1, "c:/dev/y"), call(T4, data)])).unwrap();
        let store = AgentStore {
            cursors: first.cursors,
            ..AgentStore::default()
        };
        let again = scan(&roots(tmp.path()), &store);
        assert_eq!(
            again.events,
            vec![ev(UseKey::Server("srv".into()), T4, Some("c:/dev/y"))]
        );
    }

    #[test]
    fn a_history_folder_that_cannot_be_listed_is_unreadable_and_keeps_the_cursors() {
        let tmp = tempfile::tempdir().unwrap();
        let copilot = tmp.path().join(".copilot");
        std::fs::create_dir_all(&copilot).unwrap();
        // A file stands where the folder should be.
        std::fs::write(copilot.join("session-state"), "").unwrap();
        let mut store = AgentStore::default();
        store.cursors.insert(
            "old".into(),
            Cursor {
                len: 9,
                read_to: 9,
                ..Cursor::default()
            },
        );
        let s = scan(&roots(tmp.path()), &store);
        assert_eq!(s.unreadable.as_deref(), Some("Its history folder can't be listed."));
        assert_eq!(s.cursors, store.cursors);
        assert!(s.events.is_empty());
    }

    #[test]
    fn no_history_folder_is_no_history_and_not_an_error() {
        let none = tempfile::tempdir().unwrap();
        assert_eq!(scan_of(none.path()), Scan::default());
    }

    #[cfg(windows)]
    #[test]
    fn a_file_that_cannot_be_opened_is_skipped_and_keeps_its_old_cursor() {
        use std::os::windows::fs::OpenOptionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let file = put(tmp.path(), MCP, &lines(&[start(T1, "c:/dev/x")]));
        let first = scan_of(tmp.path());
        let key = file.display().to_string();
        // Held with no sharing, the file can't be opened for reading.
        let _held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        let store = AgentStore {
            cursors: first.cursors.clone(),
            ..AgentStore::default()
        };
        let s = scan(&roots(tmp.path()), &store);
        assert_eq!(s.unreadable, None);
        assert!(s.events.is_empty());
        assert_eq!(s.cursors[&key], first.cursors[&key]);
    }

    #[test]
    fn what_a_log_carries_besides_the_fields_is_never_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let text = fixture(SKILL);
        assert!(text.contains("Body of the skill."));
        put(tmp.path(), SKILL, &text);
        let s = scan_of(tmp.path());
        assert_eq!(s.events.len(), 1);
        let shown = format!("{s:?} {:?}", s.events);
        assert!(!shown.contains("Body of the skill"), "{shown}");
        assert!(!shown.contains("GOOD-PROJECT"), "{shown}");
        // Nor in a cursor that is stored: only a length, a time and a folder.
        let kept = format!("{:?}", s.cursors);
        assert!(!kept.contains("Body of the skill"), "{kept}");
    }
}
