//! Records Claude Code sessions from its hook events: keeps the sessions index and sidecars up to date,
//! and copies the transcript into the archive on a blocking thread.

use super::index::{Index, Opened};
use super::{SessionMeta, archive, transcript_allowed};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The agent whose hooks feed the history so far.
const AGENT: &str = "claude";

/// How long a transcript settles before it is copied: Claude writes the turn's last lines after the hook fires.
const SETTLE: Duration = Duration::from_millis(1500);

pub struct History {
    home: PathBuf,
    /// None when the index could not be opened: history is off for this run.
    index: Mutex<Option<Index>>,
    /// Sessions being copied right now. `Some` holds the newest request that arrived during the copy,
    /// which runs once more after it.
    copying: Mutex<HashMap<String, Option<SessionMeta>>>,
    /// The wait before each copy; tests shorten it.
    settle: Mutex<Duration>,
}

impl History {
    /// Opens (and checks or rebuilds) the index; logs the outcome. History is off when the index can't open.
    pub fn open(home: &Path) -> History {
        let index = match Index::open(home) {
            Ok((index, opened)) => {
                match opened {
                    Opened::Ok => tracing::info!("history index is ready"),
                    Opened::Created { sessions } => {
                        tracing::info!("history index created from {sessions} sidecar(s)")
                    }
                    Opened::Rebuilt { quarantined, sessions } => tracing::warn!(
                        "history index was broken; moved to {} and rebuilt from {sessions} sidecar(s)",
                        quarantined.display()
                    ),
                }
                Some(index)
            }
            Err(err) => {
                tracing::warn!("history is off for this run: {err:#}");
                None
            }
        };
        History {
            home: home.to_path_buf(),
            index: Mutex::new(index),
            copying: Mutex::new(HashMap::new()),
            settle: Mutex::new(SETTLE),
        }
    }

    /// Sets the wait before each copy (tests only need it short).
    pub fn set_settle(&self, settle: Duration) {
        *self.settle.lock().unwrap() = settle;
    }

    /// Validates the body, updates the index and sidecar when it is a session start, stop or end or a session not
    /// yet seen in this tab, and says whether a copy should follow. Blocking: call inside spawn_blocking.
    pub fn note_event(&self, tab_id: &str, body: &Value, at_ms: u64) -> Option<SessionMeta> {
        let text = |key: &str| body.get(key).and_then(Value::as_str);
        let event = text("hook_event_name")?;
        let session_id = text("session_id")?;
        let transcript_path = text("transcript_path")?;
        let cwd = text("cwd")?;
        if !transcript_allowed(transcript_path, session_id) {
            return None;
        }
        let guard = self.index.lock().unwrap();
        let index = guard.as_ref()?;

        let known = match index.get(AGENT, session_id) {
            Ok(row) => row.filter(|row| row.tab_id == tab_id),
            Err(err) => {
                tracing::warn!("history index read failed: {err:#}");
                return None;
            }
        };
        let is_new = known.is_none();
        let (started_ms, previous_in_tab) = match known {
            Some(row) => (row.started_ms, row.previous_in_tab),
            None => {
                let previous = index
                    .latest_for_tab(tab_id)
                    .ok()
                    .flatten()
                    .map(|row| row.session_id)
                    .filter(|id| id != session_id);
                (at_ms, previous)
            }
        };
        let meta = SessionMeta {
            agent: AGENT.to_string(),
            session_id: session_id.to_string(),
            tab_id: tab_id.to_string(),
            cwd: cwd.to_string(),
            transcript_path: transcript_path.to_string(),
            started_ms,
            updated_ms: at_ms,
            previous_in_tab,
        };
        let ends_turn = matches!(event, "Stop" | "SessionEnd");
        if is_new || ends_turn || event == "SessionStart" {
            if let Err(err) = index.upsert(&meta) {
                tracing::warn!("history index write failed: {err:#}");
            }
            if let Err(err) = archive::write_meta(&self.home, &meta) {
                tracing::warn!("history sidecar write failed: {err:#}");
            }
        }
        ends_turn.then_some(meta)
    }

    /// The tab's most recent session, if the index knows one.
    pub fn latest_for_tab(&self, tab_id: &str) -> Option<SessionMeta> {
        self.index
            .lock()
            .unwrap()
            .as_ref()?
            .latest_for_tab(tab_id)
            .ok()
            .flatten()
    }

    /// True when no copy is running or waiting.
    pub fn is_idle(&self) -> bool {
        self.copying.lock().unwrap().is_empty()
    }
}

/// Copies on a blocking thread after the settle time, one copy at a time per session; a request during the wait or
/// a copy runs one more copy after it.
pub fn schedule_copy(history: Arc<History>, meta: SessionMeta) {
    {
        let mut copying = history.copying.lock().unwrap();
        if let Some(waiting) = copying.get_mut(&meta.session_id) {
            *waiting = Some(meta);
            return;
        }
        copying.insert(meta.session_id.clone(), None);
    }
    tokio::task::spawn_blocking(move || {
        let id = meta.session_id.clone();
        let mut current = meta;
        loop {
            let settle = *history.settle.lock().unwrap();
            std::thread::sleep(settle);
            if let Err(err) = archive::copy_transcript(&history.home, &current) {
                tracing::warn!("could not archive transcript {id}: {err:#}");
            }
            let mut copying = history.copying.lock().unwrap();
            match copying.get_mut(&id).and_then(Option::take) {
                Some(next) => current = next,
                None => {
                    copying.remove(&id);
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;
    use serde_json::{Value, json};
    use std::path::Path;

    const ID: &str = "00000000-0000-4000-8000-000000000001";
    const START: &str = include_str!("../../tests/fixtures/claude/2.1.302/SessionStart.json");
    const STOP: &str = include_str!("../../tests/fixtures/claude/2.1.302/Stop.json");
    const END: &str = include_str!("../../tests/fixtures/claude/2.1.302/SessionEnd.json");
    const PRE_TOOL: &str = include_str!("../../tests/fixtures/claude/2.1.302/PreToolUse.json");

    /// A fixture with its session id and transcript path pointed at a file under `dir`.
    fn body(fixture: &str, dir: &Path, id: &str) -> Value {
        let mut v: Value = serde_json::from_str(fixture).unwrap();
        v["session_id"] = json!(id);
        v["transcript_path"] = json!(dir.join(format!("{id}.jsonl")).to_string_lossy());
        v
    }

    fn sidecar(home: &Path, id: &str) -> std::path::PathBuf {
        crate::history::archive::archive_paths(home, "claude", id).1
    }

    #[test]
    fn note_event_records_start_stop_end() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());

        assert_eq!(history.note_event("t1", &body(START, home.path(), ID), 1000), None);
        let row = history.latest_for_tab("t1").unwrap();
        assert_eq!(row.session_id, ID);
        assert_eq!(row.started_ms, 1000);
        assert!(sidecar(home.path(), ID).exists());

        let stopped = history.note_event("t1", &body(STOP, home.path(), ID), 2000).unwrap();
        assert_eq!(stopped.started_ms, 1000);
        assert_eq!(stopped.updated_ms, 2000);

        let before_bytes = std::fs::read(sidecar(home.path(), ID)).unwrap();
        let before_time = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        assert_eq!(history.note_event("t1", &body(PRE_TOOL, home.path(), ID), 3000), None);
        assert_eq!(std::fs::read(sidecar(home.path(), ID)).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap(),
            before_time
        );

        let ended = history.note_event("t1", &body(END, home.path(), ID), 4000).unwrap();
        assert_eq!(ended.started_ms, 1000);
        assert_eq!(ended.updated_ms, 4000);
    }

    #[tokio::test]
    async fn copy_waits_for_the_turns_last_lines() {
        use std::io::Write;
        let home = tempfile::tempdir().unwrap();
        let history = Arc::new(History::open(home.path()));
        history.set_settle(Duration::from_millis(300));
        let transcript = home.path().join(format!("{ID}.jsonl"));
        std::fs::write(
            &transcript,
            "{\"n\":1}
",
        )
        .unwrap();

        let meta = history.note_event("t1", &body(STOP, home.path(), ID), 2000).unwrap();
        schedule_copy(history.clone(), meta);
        // Claude writes the turn's last lines a moment after the Stop hook fires.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut file = std::fs::OpenOptions::new().append(true).open(&transcript).unwrap();
        writeln!(file, "{{\"n\":2}}").unwrap();

        let started = std::time::Instant::now();
        while !history.is_idle() {
            assert!(started.elapsed() < Duration::from_secs(5), "copy did not finish");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (archive, _) = crate::history::archive::archive_paths(home.path(), "claude", ID);
        let decoded = zstd::decode_all(std::fs::File::open(&archive).unwrap()).unwrap();
        assert_eq!(decoded, std::fs::read(&transcript).unwrap());
        assert!(String::from_utf8(decoded).unwrap().contains("{\"n\":2}"));
    }

    #[test]
    fn clear_links_previous_in_tab() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        history.note_event("t1", &body(START, home.path(), "aaa"), 1000);
        history.note_event("t1", &body(START, home.path(), "bbb"), 2000);
        let latest = history.latest_for_tab("t1").unwrap();
        assert_eq!(latest.session_id, "bbb");
        assert_eq!(latest.previous_in_tab.as_deref(), Some("aaa"));
        assert_eq!(history.latest_for_tab("t2"), None);
    }

    #[test]
    fn an_unseen_session_is_recorded_on_any_event() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        assert_eq!(history.note_event("t1", &body(PRE_TOOL, home.path(), ID), 1500), None);
        assert_eq!(history.latest_for_tab("t1").unwrap().started_ms, 1500);
        assert!(sidecar(home.path(), ID).exists());
    }

    #[test]
    fn hostile_body_writes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());

        let mut bad_id = body(STOP, home.path(), ID);
        bad_id["session_id"] = json!("../../x");
        bad_id["transcript_path"] = json!(home.path().join("../../x.jsonl").to_string_lossy());
        let mut unc = body(STOP, home.path(), ID);
        unc["transcript_path"] = json!(format!(r"\\host\share\{ID}.jsonl"));
        let mut device = body(STOP, home.path(), ID);
        device["transcript_path"] = json!(format!(r"\\?\C:\x\{ID}.jsonl"));
        let mut other_name = body(STOP, home.path(), ID);
        other_name["transcript_path"] = json!(home.path().join("other.jsonl").to_string_lossy());
        let mut no_cwd = body(STOP, home.path(), ID);
        no_cwd.as_object_mut().unwrap().remove("cwd");

        for hostile in [bad_id, unc, device, other_name, no_cwd] {
            assert_eq!(history.note_event("t1", &hostile, 1000), None);
        }
        assert_eq!(history.latest_for_tab("t1"), None);
        let archive = paths::archive_dir(home.path(), "claude");
        let files = std::fs::read_dir(archive).map(|d| d.count()).unwrap_or(0);
        assert_eq!(files, 0);
    }
}
