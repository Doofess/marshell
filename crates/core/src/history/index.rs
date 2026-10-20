//! The sessions index: one SQLite table with a row per archived session. It can always be rebuilt
//! from the sidecars, so a damaged file is moved aside and replaced.

use super::{SessionMeta, archive};
use crate::{paths, store};
use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::{Path, PathBuf};

/// The schema version this build writes.
const SCHEMA_VERSION: i64 = 1;

/// What `Index::open` found.
pub enum Opened {
    /// No file was there; a new empty index (filled from sidecars if any exist).
    Created {
        sessions: usize,
    },
    Ok,
    /// The old file was broken: moved aside and rebuilt from the sidecars.
    Rebuilt {
        quarantined: PathBuf,
        sessions: usize,
    },
}

pub struct Index {
    conn: Connection,
}

/// What a check of an existing index file found.
enum Health {
    Good,
    Broken,
    Newer(i64),
}

impl Index {
    /// Err when the file is from a newer Marshell or can't be replaced (history is off this run).
    pub fn open(home: &Path) -> anyhow::Result<(Index, Opened)> {
        store::create_private_dir(&paths::history_dir(home))?;
        let file = paths::index_file(home);
        if file.is_dir() {
            anyhow::bail!("{} is a folder, so the history index can't be used", file.display());
        }
        let empty = std::fs::metadata(&file).map(|m| m.len() == 0).unwrap_or(true);
        if empty {
            let (conn, sessions) = Self::create(home, &file)?;
            return Ok((Index { conn }, Opened::Created { sessions }));
        }
        match check(&file) {
            Health::Good => {
                let conn = Connection::open(&file).context("opening the history index")?;
                Ok((Index { conn }, Opened::Ok))
            }
            Health::Newer(version) => anyhow::bail!(
                "{} was written by a newer Marshell (schema {version}); this one knows up to {SCHEMA_VERSION}",
                file.display()
            ),
            Health::Broken => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let quarantined = file.with_file_name(format!("index.sqlite.broken-{now}"));
                std::fs::rename(&file, &quarantined)
                    .with_context(|| format!("moving the broken index {} aside", file.display()))?;
                tracing::warn!("history index was broken; moved to {}", quarantined.display());
                let (conn, sessions) = Self::create(home, &file)?;
                Ok((Index { conn }, Opened::Rebuilt { quarantined, sessions }))
            }
        }
    }

    /// Makes a new index at `file` and fills it from the sidecars. Returns how many sessions went in.
    fn create(home: &Path, file: &Path) -> anyhow::Result<(Connection, usize)> {
        let mut conn = Connection::open(file).context("creating the history index")?;
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS sessions (
                 agent TEXT NOT NULL,
                 session_id TEXT NOT NULL,
                 tab_id TEXT NOT NULL,
                 cwd TEXT NOT NULL,
                 transcript_path TEXT NOT NULL,
                 started_ms INTEGER NOT NULL,
                 updated_ms INTEGER NOT NULL,
                 previous_in_tab TEXT,
                 PRIMARY KEY (agent, session_id)
             );
             CREATE INDEX IF NOT EXISTS sessions_by_tab ON sessions (tab_id, updated_ms);
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))
        .context("setting up the history index")?;
        let metas = archive::read_all_meta(home);
        let tx = conn.transaction()?;
        for meta in &metas {
            insert(&tx, meta)?;
        }
        tx.commit()?;
        Ok((conn, metas.len()))
    }

    /// Insert or replace by (agent, session_id).
    pub fn upsert(&self, meta: &SessionMeta) -> anyhow::Result<()> {
        insert(&self.conn, meta)
    }

    pub fn get(&self, agent: &str, session_id: &str) -> anyhow::Result<Option<SessionMeta>> {
        let sql = format!("SELECT {COLUMNS} FROM sessions WHERE agent = ?1 AND session_id = ?2");
        Ok(self
            .conn
            .query_row(&sql, params![agent, session_id], read_row)
            .optional()?)
    }

    /// The tab's session with the highest updated_ms.
    pub fn latest_for_tab(&self, tab_id: &str) -> anyhow::Result<Option<SessionMeta>> {
        let sql = format!("SELECT {COLUMNS} FROM sessions WHERE tab_id = ?1 ORDER BY updated_ms DESC LIMIT 1");
        Ok(self.conn.query_row(&sql, params![tab_id], read_row).optional()?)
    }
}

const COLUMNS: &str = "agent, session_id, tab_id, cwd, transcript_path, started_ms, updated_ms, previous_in_tab";

/// Opens the file and checks it, closing it again before returning.
fn check(file: &Path) -> Health {
    let Ok(conn) = Connection::open(file) else {
        return Health::Broken;
    };
    let Ok(version) = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)) else {
        return Health::Broken;
    };
    if version > SCHEMA_VERSION {
        return Health::Newer(version);
    }
    let integrity = conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0));
    if version == SCHEMA_VERSION && integrity.is_ok_and(|first| first == "ok") {
        Health::Good
    } else {
        Health::Broken
    }
}

fn insert(conn: &Connection, meta: &SessionMeta) -> anyhow::Result<()> {
    conn.execute(
        &format!("INSERT OR REPLACE INTO sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
        params![
            meta.agent,
            meta.session_id,
            meta.tab_id,
            meta.cwd,
            meta.transcript_path,
            meta.started_ms as i64,
            meta.updated_ms as i64,
            meta.previous_in_tab,
        ],
    )
    .context("saving a session in the history index")?;
    Ok(())
}

fn read_row(row: &Row) -> rusqlite::Result<SessionMeta> {
    Ok(SessionMeta {
        agent: row.get(0)?,
        session_id: row.get(1)?,
        tab_id: row.get(2)?,
        cwd: row.get(3)?,
        transcript_path: row.get(4)?,
        started_ms: row.get::<_, i64>(5)? as u64,
        updated_ms: row.get::<_, i64>(6)? as u64,
        previous_in_tab: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::archive;

    fn meta(session: &str, tab: &str, updated: u64, previous: Option<&str>) -> SessionMeta {
        SessionMeta {
            agent: "claude".into(),
            session_id: session.into(),
            tab_id: tab.into(),
            cwd: "c".into(),
            transcript_path: format!("p/{session}.jsonl"),
            started_ms: 1,
            updated_ms: updated,
            previous_in_tab: previous.map(Into::into),
        }
    }

    fn fresh() -> (tempfile::TempDir, Index) {
        let home = tempfile::tempdir().unwrap();
        let (index, _) = Index::open(home.path()).unwrap();
        (home, index)
    }

    #[test]
    fn upsert_get_and_replace() {
        let (_home, index) = fresh();
        index.upsert(&meta("a", "t1", 10, None)).unwrap();
        let newer = meta("a", "t2", 20, Some("z"));
        index.upsert(&newer).unwrap();
        assert_eq!(index.get("claude", "a").unwrap(), Some(newer));
        assert_eq!(index.get("claude", "missing").unwrap(), None);
        let rows: i64 = index
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn latest_for_tab_picks_the_newest_session_of_that_tab() {
        let (_home, index) = fresh();
        index.upsert(&meta("A", "t1", 10, None)).unwrap();
        let b = meta("B", "t1", 20, Some("A"));
        index.upsert(&b).unwrap();
        let c = meta("C", "t2", 30, None);
        index.upsert(&c).unwrap();
        assert_eq!(index.latest_for_tab("t1").unwrap(), Some(b));
        assert_eq!(index.latest_for_tab("t2").unwrap(), Some(c));
        assert_eq!(index.latest_for_tab("t3").unwrap(), None);
    }

    #[test]
    fn missing_index_is_built_from_sidecars() {
        let home = tempfile::tempdir().unwrap();
        archive::write_meta(home.path(), &meta("a", "t1", 1, None)).unwrap();
        archive::write_meta(home.path(), &meta("b", "t1", 2, None)).unwrap();
        let (index, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Created { sessions: 2 }));
        assert!(index.get("claude", "a").unwrap().is_some());
        assert!(index.get("claude", "b").unwrap().is_some());
    }

    #[test]
    fn garbage_index_is_quarantined_and_rebuilt() {
        let home = tempfile::tempdir().unwrap();
        archive::write_meta(home.path(), &meta("a", "t1", 1, None)).unwrap();
        let file = paths::index_file(home.path());
        std::fs::write(&file, b"not a database").unwrap();
        let (index, opened) = Index::open(home.path()).unwrap();
        let Opened::Rebuilt { quarantined, sessions } = opened else {
            panic!("expected Rebuilt");
        };
        assert_eq!(sessions, 1);
        let name = quarantined.file_name().unwrap().to_string_lossy().into_owned();
        let digits = name.strip_prefix("index.sqlite.broken-").unwrap();
        assert!(!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(std::fs::read(&quarantined).unwrap(), b"not a database");
        assert!(index.get("claude", "a").unwrap().is_some());
    }

    #[test]
    fn empty_index_file_is_rebuilt() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(paths::history_dir(home.path())).unwrap();
        std::fs::write(paths::index_file(home.path()), b"").unwrap();
        let (index, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Created { .. } | Opened::Rebuilt { .. }));
        index.upsert(&meta("a", "t1", 1, None)).unwrap();
    }

    #[test]
    fn newer_index_is_left_alone() {
        let home = tempfile::tempdir().unwrap();
        drop(Index::open(home.path()).unwrap());
        let file = paths::index_file(home.path());
        rusqlite::Connection::open(&file)
            .unwrap()
            .pragma_update(None, "user_version", 2)
            .unwrap();
        let before = std::fs::read(&file).unwrap();
        assert!(Index::open(home.path()).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }

    #[test]
    fn index_path_that_is_a_folder_is_an_error() {
        let home = tempfile::tempdir().unwrap();
        let file = paths::index_file(home.path());
        std::fs::create_dir_all(&file).unwrap();
        assert!(Index::open(home.path()).is_err());
        assert!(file.is_dir());
    }

    #[test]
    fn reopen_is_ok() {
        let home = tempfile::tempdir().unwrap();
        drop(Index::open(home.path()).unwrap());
        let (_, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Ok));
    }
}
