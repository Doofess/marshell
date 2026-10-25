//! The sessions index and search rows. Rebuilt from sidecars and marks files when damaged or outdated.

use super::{
    SessionMeta, archive,
    marks::{self, Marks},
};
use crate::{paths, store};
use anyhow::Context;
use marshell_protocol::history::{Client, Conversation, HistoryEntry, HistoryQuery, Place, SearchHit, Turn, TurnItem};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// 2 added conversation fields and full-text search; 3 caches the person's marks.
const SCHEMA_VERSION: i64 = 4;

/// Each session owns a block of this many full-text rowids (one per turn), so its rows can be replaced
/// by a rowid range instead of a scan of the whole table.
const TURNS_PER_BLOCK: i64 = 1 << 20;

/// Most rows the list returns, and most hits a search returns.
const LIST_LIMIT: i64 = 2000;
const SEARCH_LIMIT: i64 = 50;

/// Most words of a typed query that reach the index.
const MAX_QUERY_TERMS: usize = 32;

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
    /// The old file was from an older schema: moved aside and rebuilt from the sidecars.
    Upgraded {
        sessions: usize,
    },
}

pub struct Index {
    conn: Connection,
    home: PathBuf,
}

/// What a check of an existing index file found.
enum Health {
    Good,
    Broken,
    /// An intact index of an older schema.
    Older(i64),
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
            return Ok((Index::new(conn, home), Opened::Created { sessions }));
        }
        match check(&file) {
            Health::Good => {
                let conn = Connection::open(&file).context("opening the history index")?;
                Ok((Index::new(conn, home), Opened::Ok))
            }
            Health::Newer(version) => anyhow::bail!(
                "{} was written by a newer Marshell (schema {version}); this one knows up to {SCHEMA_VERSION}",
                file.display()
            ),
            Health::Older(version) => {
                let quarantined = move_aside(&file, &format!("v{version}"))?;
                tracing::info!(
                    "history index was schema {version}; moved to {} and rebuilt",
                    quarantined.display()
                );
                let (conn, sessions) = Self::create(home, &file)?;
                Ok((Index::new(conn, home), Opened::Upgraded { sessions }))
            }
            Health::Broken => {
                let quarantined = move_aside(&file, "broken")?;
                tracing::warn!("history index was broken; moved to {}", quarantined.display());
                let (conn, sessions) = Self::create(home, &file)?;
                Ok((Index::new(conn, home), Opened::Rebuilt { quarantined, sessions }))
            }
        }
    }

    fn new(conn: Connection, home: &Path) -> Index {
        Index {
            conn,
            home: home.to_path_buf(),
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
                 title TEXT NOT NULL DEFAULT '',
                 branch TEXT,
                 model TEXT,
                 version TEXT,
                 limited INTEGER NOT NULL DEFAULT 0,
                 turns INTEGER NOT NULL DEFAULT 0,
                 indexed_len INTEGER,
                 name TEXT,
                 pinned INTEGER NOT NULL DEFAULT 0,
                 archived INTEGER NOT NULL DEFAULT 0,
                 client TEXT NOT NULL DEFAULT 'marshell',
                 PRIMARY KEY (agent, session_id)
             );
             CREATE INDEX IF NOT EXISTS sessions_by_tab ON sessions (tab_id, updated_ms);
             CREATE TABLE IF NOT EXISTS tags (
                 agent TEXT NOT NULL, session_id TEXT NOT NULL, tag TEXT NOT NULL, key TEXT NOT NULL,
                 PRIMARY KEY (agent, session_id, key)
             );
             CREATE TABLE IF NOT EXISTS bookmarks (
                 agent TEXT NOT NULL, session_id TEXT NOT NULL, at_ms INTEGER NOT NULL,
                 PRIMARY KEY (agent, session_id, at_ms)
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS turns_fts USING fts5(
                 prompt, reply, tools, paths,
                 agent UNINDEXED, session_id UNINDEXED, turn UNINDEXED, at_ms UNINDEXED,
                 tokenize = 'unicode61 remove_diacritics 2'
             );
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))
        .context("setting up the history index")?;
        let metas = archive::read_all_meta(home);
        let tx = conn.transaction()?;
        for meta in &metas {
            insert(&tx, meta)?;
            let marks = marks::read(home, &meta.agent, &meta.session_id);
            replace_marks(&tx, &meta.agent, &meta.session_id, &marks)?;
        }
        tx.commit()?;
        Ok((conn, metas.len()))
    }

    /// Insert, or update the session's own fields by (agent, session_id); what indexing read stays.
    pub fn upsert(&self, meta: &SessionMeta) -> anyhow::Result<()> {
        insert(&self.conn, meta)
    }

    /// Replaces every cached annotation together. The marks file remains the source of truth.
    pub fn set_marks(&self, agent: &str, session_id: &str, marks: &Marks) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        replace_marks(&tx, agent, session_id, marks)?;
        tx.commit()?;
        Ok(())
    }

    /// Removes only index rows; the caller owns any archive-file deletion.
    pub fn remove(&self, agent: &str, session_id: &str) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        if let Some(block) = existing_block(&tx, agent, session_id)? {
            tx.execute(
                "DELETE FROM turns_fts WHERE rowid >= ?1 AND rowid < ?2",
                params![block, block + TURNS_PER_BLOCK],
            )?;
        }
        for table in ["tags", "bookmarks", "sessions"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE agent = ?1 AND session_id = ?2"),
                params![agent, session_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Prompt times in turn order, using the session's full-text rowid block.
    pub fn turn_times(&self, agent: &str, session_id: &str) -> anyhow::Result<Vec<(u32, u64)>> {
        let Some(block) = existing_block(&self.conn, agent, session_id)? else {
            return Ok(Vec::new());
        };
        let mut stmt = self
            .conn
            .prepare("SELECT turn, at_ms FROM turns_fts WHERE rowid >= ?1 AND rowid < ?2 ORDER BY rowid")?;
        let rows = stmt.query_map(params![block, block + TURNS_PER_BLOCK], |row| {
            Ok((row.get::<_, i64>(0)? as u32, row.get::<_, i64>(1)? as u64))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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

    /// The tab's session of `agent` with the highest updated_ms. A CLI started from inside another inherits the
    /// tab id, so a tab can hold rows of more than one agent.
    pub fn latest_for_tab_of(&self, tab_id: &str, agent: &str) -> anyhow::Result<Option<SessionMeta>> {
        let sql =
            format!("SELECT {COLUMNS} FROM sessions WHERE tab_id = ?1 AND agent = ?2 ORDER BY updated_ms DESC LIMIT 1");
        Ok(self.conn.query_row(&sql, params![tab_id, agent], read_row).optional()?)
    }

    /// Replaces the session's full-text rows and conversation fields in one transaction. The session's row is
    /// made from `meta` when it isn't there yet; an existing row keeps its own tab and times.
    /// `indexed_len` is the length of the archive file the conversation was read from.
    pub fn index_conversation(&self, meta: &SessionMeta, c: &Conversation, indexed_len: u64) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let block = session_block(&tx, meta)?;
        tx.execute(
            "UPDATE sessions SET title = ?3, branch = ?4, model = ?5, version = ?6, limited = ?7, turns = ?8,
                 indexed_len = ?9 WHERE agent = ?1 AND session_id = ?2",
            params![
                meta.agent,
                meta.session_id,
                c.title,
                c.branch,
                c.model,
                c.version,
                c.limited,
                c.turns.len() as i64,
                indexed_len as i64,
            ],
        )?;
        tx.execute(
            "DELETE FROM turns_fts WHERE rowid >= ?1 AND rowid < ?2",
            params![block, block + TURNS_PER_BLOCK],
        )?;
        let mut insert = tx.prepare(
            "INSERT INTO turns_fts (rowid, prompt, reply, tools, paths, agent, session_id, turn, at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for turn in c.turns.iter().take(TURNS_PER_BLOCK as usize) {
            let text = turn_text(turn);
            insert.execute(params![
                block + i64::from(turn.index),
                turn.prompt,
                text.reply,
                text.tools,
                text.paths,
                meta.agent,
                meta.session_id,
                i64::from(turn.index),
                turn.at_ms as i64,
            ])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(())
    }

    /// Records that the session's archive could not be read in full: it stays in the list, marked limited,
    /// with no search rows.
    pub fn mark_limited(&self, meta: &SessionMeta, indexed_len: u64) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let block = session_block(&tx, meta)?;
        tx.execute(
            "UPDATE sessions SET limited = 1, turns = 0, indexed_len = ?3 WHERE agent = ?1 AND session_id = ?2",
            params![meta.agent, meta.session_id, indexed_len as i64],
        )?;
        tx.execute(
            "DELETE FROM turns_fts WHERE rowid >= ?1 AND rowid < ?2",
            params![block, block + TURNS_PER_BLOCK],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The sessions, newest `updated_ms` first, narrowed by the filters that are set.
    pub fn list(&self, q: &HistoryQuery) -> anyhow::Result<Vec<HistoryEntry>> {
        let filters = filters();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS}
             FROM sessions
             WHERE {filters}
             ORDER BY updated_ms DESC, session_id LIMIT ?11"
        ))?;
        let rows = stmt.query_map(
            params![
                q.agent,
                q.project,
                q.branch,
                q.model,
                q.since_ms.map(|ms| ms as i64),
                q.tag.as_ref().map(|s| s.to_lowercase()),
                q.bookmarked.unwrap_or(false),
                q.archived.unwrap_or(false),
                place_name(q.place),
                q.headless.unwrap_or(false),
                LIST_LIMIT
            ],
            |row| read_entry(&self.conn, row),
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// One session as the list shows it, if the index has it.
    pub fn entry(&self, agent: &str, session_id: &str) -> anyhow::Result<Option<HistoryEntry>> {
        let sql = format!("SELECT {ENTRY_COLUMNS} FROM sessions WHERE agent = ?1 AND session_id = ?2");
        Ok(self
            .conn
            .query_row(&sql, params![agent, session_id], |row| read_entry(&self.conn, row))
            .optional()?)
    }

    /// Turns that match what a person typed, best first. The text only ever reaches SQLite as the bound
    /// parameter built by `fts_query`. Marks each hit in the snippet between U+0002 and U+0003.
    pub fn search(&self, query: &str, include_archived: bool, headless: bool) -> anyhow::Result<Vec<SearchHit>> {
        let Some(fts) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT turns_fts.agent, turns_fts.session_id, COALESCE(sessions.name, sessions.title), sessions.cwd, turns_fts.turn,
                    turns_fts.at_ms, snippet(turns_fts, -1, char(2), char(3), '…', 12)
             FROM turns_fts JOIN sessions
               ON sessions.agent = turns_fts.agent AND sessions.session_id = turns_fts.session_id
             WHERE turns_fts MATCH ?1 AND (?3 OR sessions.archived = 0) AND {}
             ORDER BY rank LIMIT ?2",
            client_filter("NULL", "?4")
        ))?;
        let rows = stmt.query_map(params![fts, SEARCH_LIMIT, include_archived, headless], |row| {
            Ok(SearchHit {
                agent: row.get(0)?,
                session_id: row.get(1)?,
                title: row.get(2)?,
                cwd: row.get(3)?,
                turn: row.get::<_, i64>(4)? as u32,
                at_ms: row.get::<_, i64>(5)? as u64,
                snippet: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Bookmarked turns, newest bookmark first. Multiple timestamps in one turn produce one hit.
    pub fn bookmarks(&self, q: &HistoryQuery) -> anyhow::Result<Vec<SearchHit>> {
        let filters = filters();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT sessions.agent, sessions.session_id, COALESCE(sessions.name, sessions.title),
                    sessions.cwd, bookmarks.at_ms, sessions.rowid
             FROM bookmarks JOIN sessions
               ON sessions.agent = bookmarks.agent AND sessions.session_id = bookmarks.session_id
             WHERE {filters}
             ORDER BY bookmarks.at_ms DESC, sessions.agent, sessions.session_id"
        ))?;
        let rows = stmt.query_map(
            params![
                q.agent,
                q.project,
                q.branch,
                q.model,
                q.since_ms.map(|ms| ms as i64),
                q.tag.as_ref().map(|s| s.to_lowercase()),
                q.bookmarked.unwrap_or(false),
                q.archived.unwrap_or(false),
                place_name(q.place),
                q.headless.unwrap_or(false),
            ],
            |row| {
                Ok((
                    SearchHit {
                        agent: row.get(0)?,
                        session_id: row.get(1)?,
                        title: row.get(2)?,
                        cwd: row.get(3)?,
                        turn: 0,
                        at_ms: row.get::<_, i64>(4)? as u64,
                        snippet: String::new(),
                    },
                    row.get::<_, i64>(5)?,
                ))
            },
        )?;
        let mut times = HashMap::new();
        let mut seen = HashSet::new();
        let mut hits = Vec::new();
        for row in rows {
            let (mut hit, rowid) = row?;
            let turns = match times.entry(rowid) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(self.turn_times(&hit.agent, &hit.session_id)?)
                }
            };
            hit.turn = marks::turn_for(hit.at_ms, turns);
            if !seen.insert((rowid, hit.turn)) {
                continue;
            }
            let prompt: Option<String> = self
                .conn
                .query_row(
                    "SELECT prompt FROM turns_fts WHERE rowid = ?1",
                    params![rowid * TURNS_PER_BLOCK + i64::from(hit.turn)],
                    |row| row.get(0),
                )
                .optional()?;
            let prompt = prompt.unwrap_or_default();
            hit.snippet = match prompt.char_indices().nth(120) {
                Some((cut, _)) => format!("{}…", &prompt[..cut]),
                None => prompt,
            };
            hits.push(hit);
            if hits.len() == SEARCH_LIMIT as usize {
                break;
            }
        }
        Ok(hits)
    }

    /// The sessions that have an archive whose length is not the one last indexed, newest first.
    pub fn stale(&self) -> anyhow::Result<Vec<SessionMeta>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS}, indexed_len FROM sessions ORDER BY updated_ms DESC"
        ))?;
        let rows = stmt.query_map([], |row| Ok((read_row(row)?, row.get::<_, Option<i64>>(9)?)))?;
        let mut stale = Vec::new();
        for row in rows {
            let (meta, indexed_len) = row?;
            let (archive, _) = archive::archive_paths(&self.home, &meta.agent, &meta.session_id);
            let Ok(file) = std::fs::metadata(&archive) else {
                continue;
            };
            if indexed_len != Some(file.len() as i64) {
                stale.push(meta);
            }
        }
        Ok(stale)
    }
}

/// The text of a turn as the searchable columns.
struct TurnText {
    reply: String,
    tools: String,
    paths: String,
}

fn turn_text(turn: &Turn) -> TurnText {
    let (mut reply, mut tools, mut paths) = (Vec::new(), Vec::new(), Vec::<&str>::new());
    for item in &turn.items {
        match item {
            TurnItem::Text { markdown } => reply.push(markdown.as_str()),
            TurnItem::Tool { card } => {
                tools.push(card.summary.as_str());
                if let Some(path) = card_path(&card.name, &card.summary)
                    && !paths.contains(&path)
                {
                    paths.push(path);
                }
            }
            TurnItem::Subagent {
                description, result, ..
            } => {
                tools.push(description.as_str());
                reply.extend(result.as_deref());
            }
            TurnItem::Compacted => {}
        }
    }
    TurnText {
        reply: reply.join("\n"),
        tools: tools.join("\n"),
        paths: paths.join("\n"),
    }
}

/// The file a file tool's summary names: "Edited src/a.ts, 2 added, 1 removed", "Created n.md, 3 lines", "Read p".
fn card_path<'a>(name: &str, summary: &'a str) -> Option<&'a str> {
    let rest = match name {
        "Edit" | "Write" | "NotebookEdit" => summary
            .strip_prefix("Edited ")
            .or_else(|| summary.strip_prefix("Created ")),
        "Read" => summary.strip_prefix("Read "),
        _ => None,
    }?;
    // Drop the counts after the path: ", 2 added", ", 1 removed", ", 3 lines".
    let mut path = rest;
    while let Some((head, tail)) = path.rsplit_once(", ") {
        let is_count = tail.split_once(' ').is_some_and(|(n, unit)| {
            n.parse::<u64>().is_ok() && matches!(unit, "added" | "removed" | "line" | "lines")
        });
        if !is_count {
            break;
        }
        path = head;
    }
    Some(path)
}

/// The first full-text rowid of an existing session, without creating a missing session.
fn existing_block(conn: &Connection, agent: &str, session_id: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT rowid FROM sessions WHERE agent = ?1 AND session_id = ?2",
        params![agent, session_id],
        |row| row.get::<_, i64>(0).map(|rowid| rowid * TURNS_PER_BLOCK),
    )
    .optional()
}

/// The first full-text rowid of the session's block, making the session's row first when it isn't there.
fn session_block(conn: &Connection, meta: &SessionMeta) -> anyhow::Result<i64> {
    conn.execute(
        &format!("INSERT OR IGNORE INTO sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
        params![
            meta.agent,
            meta.session_id,
            meta.tab_id,
            meta.cwd,
            meta.transcript_path,
            meta.started_ms as i64,
            meta.updated_ms as i64,
            meta.previous_in_tab,
            meta.client.as_str(),
        ],
    )?;
    let rowid: i64 = conn.query_row(
        "SELECT rowid FROM sessions WHERE agent = ?1 AND session_id = ?2",
        params![meta.agent, meta.session_id],
        |r| r.get(0),
    )?;
    Ok(rowid * TURNS_PER_BLOCK)
}

/// What a person typed, as an FTS5 query that can't be a syntax error: each word is a quoted phrase (a `"`
/// inside it doubled) and the last one is a prefix, so typing "refac" finds "refactor". None when nothing
/// searchable is left. The only path from typed text to `MATCH`; it is always passed as a bound parameter.
pub fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|word| word.chars().filter(|c| !c.is_control()).collect::<String>())
        .filter(|word| word.chars().any(|c| c.is_alphanumeric() || is_symbol(c)))
        .take(MAX_QUERY_TERMS)
        .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        return None;
    }
    Some(format!("{}*", terms.join(" ")))
}

/// A character beyond ASCII that is not a letter or digit, such as an emoji. ASCII punctuation doesn't count.
fn is_symbol(c: char) -> bool {
    !c.is_ascii() && !c.is_whitespace() && !c.is_alphanumeric()
}

/// Renames the index file to `index.sqlite.<tag>-<ms>`.
fn move_aside(file: &Path, tag: &str) -> anyhow::Result<PathBuf> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let moved = file.with_file_name(format!("index.sqlite.{tag}-{now}"));
    std::fs::rename(file, &moved).with_context(|| format!("moving the old index {} aside", file.display()))?;
    Ok(moved)
}

const COLUMNS: &str =
    "agent, session_id, tab_id, cwd, transcript_path, started_ms, updated_ms, previous_in_tab, client";

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
    let intact = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
        .is_ok_and(|first| first == "ok");
    match version {
        _ if !intact => Health::Broken,
        SCHEMA_VERSION => Health::Good,
        1..SCHEMA_VERSION => Health::Older(version),
        _ => Health::Broken,
    }
}

/// An import (no tab) over a row a Marshell tab owns keeps the owner, as `archive::write_meta` does.
const KEEP_OWNER: &str = "sessions.tab_id <> '' AND excluded.tab_id = ''";

fn insert(conn: &Connection, meta: &SessionMeta) -> anyhow::Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO sessions ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (agent, session_id) DO UPDATE SET cwd = excluded.cwd,
                 transcript_path = excluded.transcript_path,
                 started_ms = CASE WHEN {KEEP_OWNER} THEN min(started_ms, excluded.started_ms)
                     ELSE excluded.started_ms END,
                 updated_ms = CASE WHEN {KEEP_OWNER} THEN max(updated_ms, excluded.updated_ms)
                     ELSE excluded.updated_ms END,
                 previous_in_tab = CASE WHEN {KEEP_OWNER} THEN previous_in_tab ELSE excluded.previous_in_tab END,
                 client = CASE WHEN {KEEP_OWNER} THEN client ELSE excluded.client END,
                 tab_id = CASE WHEN {KEEP_OWNER} THEN tab_id ELSE excluded.tab_id END"
        ),
        params![
            meta.agent,
            meta.session_id,
            meta.tab_id,
            meta.cwd,
            meta.transcript_path,
            meta.started_ms as i64,
            meta.updated_ms as i64,
            meta.previous_in_tab,
            meta.client.as_str(),
        ],
    )
    .context("saving a session in the history index")?;
    Ok(())
}

/// The where-clause shared by list and bookmarks, so the same filters always select the same conversations.
/// Parameters ?1 to ?8 are the query's fields, ?9 the place and ?10 whether headless runs show.
fn filters() -> String {
    format!(
        "(?1 IS NULL OR sessions.agent = ?1) AND (?2 IS NULL OR cwd = ?2)
    AND (?3 IS NULL OR branch = ?3) AND (?4 IS NULL OR model = ?4)
    AND (?5 IS NULL OR updated_ms >= ?5)
    AND (?6 IS NULL OR EXISTS (SELECT 1 FROM tags WHERE tags.agent = sessions.agent
        AND tags.session_id = sessions.session_id AND tags.key = ?6))
    AND (NOT ?7 OR EXISTS (SELECT 1 FROM bookmarks WHERE bookmarks.agent = sessions.agent
        AND bookmarks.session_id = sessions.session_id))
    AND (?8 OR sessions.archived = 0)
    AND {}",
        client_filter("?9", "?10")
    )
}

/// The condition on where a conversation ran, for list, bookmarks and search. `place` is a parameter holding
/// NULL, 'marshell' or 'outside'; `headless` one holding whether headless runs show.
fn client_filter(place: &str, headless: &str) -> String {
    format!(
        "({place} IS NULL OR ({place} = 'marshell') = (sessions.client = 'marshell'))
    AND ({headless} OR sessions.client != 'headless')"
    )
}

fn place_name(place: Option<Place>) -> Option<&'static str> {
    place.map(|p| match p {
        Place::Marshell => "marshell",
        Place::Outside => "outside",
    })
}

fn replace_marks(conn: &Connection, agent: &str, session_id: &str, marks: &Marks) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE sessions SET name = ?3, pinned = ?4, archived = ?5 WHERE agent = ?1 AND session_id = ?2",
        params![agent, session_id, marks.name, marks.pinned, marks.archived],
    )?;
    conn.execute(
        "DELETE FROM tags WHERE agent = ?1 AND session_id = ?2",
        params![agent, session_id],
    )?;
    let mut tag = conn.prepare("INSERT OR IGNORE INTO tags (agent, session_id, tag, key) VALUES (?1, ?2, ?3, ?4)")?;
    for value in &marks.tags {
        tag.execute(params![agent, session_id, value, value.to_lowercase()])?;
    }
    conn.execute(
        "DELETE FROM bookmarks WHERE agent = ?1 AND session_id = ?2",
        params![agent, session_id],
    )?;
    let mut bookmark =
        conn.prepare("INSERT OR IGNORE INTO bookmarks (agent, session_id, at_ms) VALUES (?1, ?2, ?3)")?;
    for at in &marks.bookmarks {
        bookmark.execute(params![agent, session_id, *at as i64])?;
    }
    Ok(())
}

const ENTRY_COLUMNS: &str = "agent, session_id, tab_id, cwd, COALESCE(name, title), branch, model, version,
    limited, turns, started_ms, updated_ms, pinned, archived, name IS NOT NULL,
    (SELECT COUNT(*) FROM bookmarks WHERE bookmarks.agent = sessions.agent AND bookmarks.session_id = sessions.session_id),
    client";

fn read_entry(conn: &Connection, row: &Row) -> rusqlite::Result<HistoryEntry> {
    let agent: String = row.get(0)?;
    let session_id: String = row.get(1)?;
    let mut stmt = conn.prepare_cached("SELECT tag FROM tags WHERE agent = ?1 AND session_id = ?2 ORDER BY rowid")?;
    let tags = stmt
        .query_map(params![agent, session_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(HistoryEntry {
        agent,
        session_id,
        tab_id: row.get(2)?,
        cwd: row.get(3)?,
        title: row.get(4)?,
        branch: row.get(5)?,
        model: row.get(6)?,
        version: row.get(7)?,
        limited: row.get(8)?,
        turns: row.get::<_, i64>(9)? as u32,
        started_ms: row.get::<_, i64>(10)? as u64,
        updated_ms: row.get::<_, i64>(11)? as u64,
        tags,
        pinned: row.get(12)?,
        archived: row.get(13)?,
        named: row.get(14)?,
        bookmarks: row.get::<_, i64>(15)? as u32,
        client: Client::from_name(&row.get::<_, String>(16)?),
    })
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
        recordings: vec![],
        client: Client::from_name(&row.get::<_, String>(8)?),
        preset: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::archive;
    use crate::history::marks::{self, Marks};
    use marshell_protocol::history::TurnItem;

    fn meta(session: &str, tab: &str, updated: u64, previous: Option<&str>) -> SessionMeta {
        SessionMeta {
            agent: "claude".into(),
            session_id: session.into(),
            tab_id: tab.into(),
            cwd: "c".into(),
            transcript_path: format!("{}{session}.jsonl", if cfg!(windows) { r"C:\p\" } else { "/p/" }),
            started_ms: 1,
            updated_ms: updated,
            previous_in_tab: previous.map(Into::into),
            recordings: vec![],
            client: Default::default(),
            preset: None,
        }
    }

    fn fresh() -> (tempfile::TempDir, Index) {
        let home = tempfile::tempdir().unwrap();
        let (index, _) = Index::open(home.path()).unwrap();
        (home, index)
    }

    fn seed_marks(home: &Path) {
        for (id, updated, archived) in [("a", 1, false), ("b", 2, true)] {
            archive::write_meta(home, &meta(id, "t1", updated, None)).unwrap();
            marks::write(
                home,
                "claude",
                id,
                &Marks {
                    name: Some(format!("Named {id}")),
                    tags: vec!["Zed".into(), "Bug".into()],
                    pinned: !archived,
                    archived,
                    bookmarks: vec![100, 200],
                },
            )
            .unwrap();
        }
    }

    fn assert_rebuilt_marks(index: &Index) {
        let q = HistoryQuery {
            archived: Some(true),
            ..HistoryQuery::default()
        };
        let entries = index.list(&q).unwrap();
        assert_eq!(entries.len(), 2);
        for (entry, id, pinned, archived) in [(&entries[0], "b", false, true), (&entries[1], "a", true, false)] {
            assert_eq!(entry.session_id, id);
            assert_eq!(entry.title, format!("Named {id}"));
            assert_eq!(entry.tags, ["Zed", "Bug"]);
            assert_eq!(
                (entry.pinned, entry.archived, entry.named, entry.bookmarks),
                (pinned, archived, true, 2)
            );
        }
        let hits = index.bookmarks(&q).unwrap();
        assert_eq!(hits.len(), 2, "timestamps in the same turn collapse to one hit");
        for id in ["a", "b"] {
            let hit = hits.iter().find(|h| h.session_id == id).unwrap();
            assert_eq!(hit.title, format!("Named {id}"));
            assert_eq!((hit.turn, hit.at_ms, hit.snippet.as_str()), (0, 200, ""));
        }
        let hits = index.bookmarks(&HistoryQuery::default()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "a");
    }

    #[test]
    fn marks_survive_a_rebuild() {
        let (home, index) = fresh();
        seed_marks(home.path());
        drop(index);
        std::fs::remove_file(paths::index_file(home.path())).unwrap();
        let (index, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Created { sessions: 2 }));
        assert_rebuilt_marks(&index);
    }

    #[test]
    fn schema_2_index_upgrades_to_3_with_marks() {
        let home = tempfile::tempdir().unwrap();
        seed_marks(home.path());
        let file = paths::index_file(home.path());
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (agent TEXT, session_id TEXT, title TEXT);
            INSERT INTO sessions VALUES ('claude', 'old', 'not a sidecar'); PRAGMA user_version = 2;",
        )
        .unwrap();
        drop(conn);
        let (index, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Upgraded { sessions: 2 }));
        assert_eq!(
            index
                .conn
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
        assert!(index.get("claude", "old").unwrap().is_none());
        assert_rebuilt_marks(&index);
        let c = conversation("Parsed title", None, None, &["needle"]);
        index.index_conversation(&meta("a", "t1", 1, None), &c, 1).unwrap();
        assert_eq!(index.search("needle", false, false).unwrap()[0].title, "Named a");
        let entry = index.entry("claude", "a").unwrap().unwrap();
        assert_eq!(entry.tags, ["Zed", "Bug"]);
        assert!(entry.pinned && entry.named);
        assert_eq!(entry.bookmarks, 2);
    }

    fn marked_pair() -> (tempfile::TempDir, Index) {
        let (home, index) = fresh();
        for (id, updated) in [("a", 1), ("b", 2)] {
            index
                .index_conversation(
                    &meta(id, "t1", updated, None),
                    &conversation("Title", None, None, &["needle"]),
                    1,
                )
                .unwrap();
        }
        index
            .set_marks(
                "claude",
                "a",
                &Marks {
                    tags: vec!["Bug".into(), "UI".into()],
                    bookmarks: vec![1000],
                    ..Marks::default()
                },
            )
            .unwrap();
        (home, index)
    }

    #[test]
    fn list_hides_archived_unless_asked() {
        let (_home, index) = marked_pair();
        index
            .set_marks(
                "claude",
                "b",
                &Marks {
                    archived: true,
                    ..Marks::default()
                },
            )
            .unwrap();
        for archived in [None, Some(false)] {
            let entries = index
                .list(&HistoryQuery {
                    archived,
                    ..HistoryQuery::default()
                })
                .unwrap();
            assert_eq!(entries.iter().map(|e| e.session_id.as_str()).collect::<Vec<_>>(), ["a"]);
        }
        let entries = index
            .list(&HistoryQuery {
                archived: Some(true),
                ..HistoryQuery::default()
            })
            .unwrap();
        assert_eq!(
            entries.iter().map(|e| e.session_id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert!(entries[0].archived);
        assert!(index.entry("claude", "b").unwrap().unwrap().archived);
    }

    #[test]
    fn list_filters_by_tag_case_insensitively() {
        let (_home, index) = marked_pair();
        for tag in ["bug", "BUG", "ui"] {
            let q = HistoryQuery {
                tag: Some(tag.into()),
                ..HistoryQuery::default()
            };
            let entries = index.list(&q).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].session_id, "a");
            assert_eq!(entries[0].tags, ["Bug", "UI"]);
            assert_eq!(index.bookmarks(&q).unwrap().len(), 1);
        }
        let q = HistoryQuery {
            tag: Some("missing".into()),
            ..HistoryQuery::default()
        };
        assert!(index.list(&q).unwrap().is_empty());
        assert!(index.bookmarks(&q).unwrap().is_empty());
    }

    #[test]
    fn bookmarked_filter_lists_only_bookmarked() {
        let (_home, index) = marked_pair();
        let entries = index
            .list(&HistoryQuery {
                bookmarked: Some(true),
                ..HistoryQuery::default()
            })
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "a");
        assert_eq!(entries[0].bookmarks, 1);
        assert_eq!(
            index
                .list(&HistoryQuery {
                    bookmarked: Some(false),
                    ..HistoryQuery::default()
                })
                .unwrap()
                .len(),
            2
        );
    }

    fn with_client(session: &str, client: Client) -> SessionMeta {
        let mut m = meta(session, "", 1, None);
        m.client = client;
        m
    }

    fn three_clients() -> (tempfile::TempDir, Index) {
        let (home, index) = fresh();
        for (id, client) in [
            ("m", Client::Marshell),
            ("t", Client::Terminal),
            ("h", Client::Headless),
        ] {
            index
                .index_conversation(
                    &with_client(id, client),
                    &conversation("Title", None, None, &["needle"]),
                    1,
                )
                .unwrap();
        }
        (home, index)
    }

    fn ids(index: &Index, q: HistoryQuery) -> Vec<String> {
        let mut ids: Vec<String> = index.list(&q).unwrap().into_iter().map(|e| e.session_id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn an_import_upsert_keeps_the_marshell_owner() {
        let (_home, index) = fresh();
        index.upsert(&meta("s", "t1", 10, Some("p"))).unwrap();
        let mut imported = with_client("s", Client::Terminal);
        imported.updated_ms = 20;
        index.upsert(&imported).unwrap();
        let entry = index.entry("claude", "s").unwrap().unwrap();
        assert_eq!((entry.tab_id.as_str(), entry.client), ("t1", Client::Marshell));
        assert_eq!(entry.updated_ms, 20);
        // A resume in another tab still takes the session.
        index.upsert(&meta("s", "t2", 30, None)).unwrap();
        assert_eq!(index.entry("claude", "s").unwrap().unwrap().tab_id, "t2");
    }

    #[test]
    fn list_hides_headless_runs_unless_asked() {
        let (_home, index) = three_clients();
        assert_eq!(ids(&index, HistoryQuery::default()), ["m", "t"]);
        let all = HistoryQuery {
            headless: Some(true),
            ..HistoryQuery::default()
        };
        assert_eq!(ids(&index, all), ["h", "m", "t"]);
        let entry = index.entry("claude", "t").unwrap().unwrap();
        assert_eq!(entry.client, Client::Terminal);
    }

    #[test]
    fn place_filters_marshell_and_outside() {
        let (_home, index) = three_clients();
        let place = |place, headless| HistoryQuery {
            place: Some(place),
            headless,
            ..HistoryQuery::default()
        };
        assert_eq!(ids(&index, place(Place::Marshell, Some(true))), ["m"]);
        assert_eq!(ids(&index, place(Place::Outside, None)), ["t"]);
        assert_eq!(ids(&index, place(Place::Outside, Some(true))), ["h", "t"]);
    }

    #[test]
    fn search_leaves_out_headless_runs() {
        let (_home, index) = three_clients();
        let found = |headless| {
            let mut ids: Vec<String> = index
                .search("needle", false, headless)
                .unwrap()
                .into_iter()
                .map(|h| h.session_id)
                .collect();
            ids.sort();
            ids
        };
        assert_eq!(found(false), ["m", "t"]);
        assert_eq!(found(true), ["h", "m", "t"]);
    }

    #[test]
    fn an_index_from_schema_3_is_rebuilt_with_clients() {
        let home = tempfile::tempdir().unwrap();
        archive::write_meta(home.path(), &with_client("t", Client::Terminal)).unwrap();
        let file = paths::index_file(home.path());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (agent TEXT, session_id TEXT, title TEXT);
            PRAGMA user_version = 3;",
        )
        .unwrap();
        drop(conn);
        let (index, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Upgraded { sessions: 1 }));
        assert_eq!(index.entry("claude", "t").unwrap().unwrap().client, Client::Terminal);
    }

    #[test]
    fn search_skips_archived_unless_asked() {
        let (_home, index) = marked_pair();
        index
            .set_marks(
                "claude",
                "b",
                &Marks {
                    archived: true,
                    ..Marks::default()
                },
            )
            .unwrap();
        let hits = index.search("needle", false, false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "a");
        assert_eq!(index.search("needle", true, false).unwrap().len(), 2);
    }

    #[test]
    fn name_overrides_title_in_list_and_search() {
        let (_home, index) = marked_pair();
        index
            .set_marks(
                "claude",
                "a",
                &Marks {
                    name: Some("My name".into()),
                    pinned: true,
                    ..Marks::default()
                },
            )
            .unwrap();
        // Recorder updates and transcript reindexing must preserve the person's name and pin.
        index.upsert(&meta("a", "t1", 3, None)).unwrap();
        index
            .index_conversation(
                &meta("a", "t1", 3, None),
                &conversation("New parsed title", None, None, &["needle"]),
                2,
            )
            .unwrap();
        let entry = &index.list(&HistoryQuery::default()).unwrap()[0];
        assert_eq!(entry.title, "My name");
        assert!(entry.named && entry.pinned);
        let hits = index.search("needle", false, false).unwrap();
        assert_eq!(hits.iter().find(|h| h.session_id == "a").unwrap().title, "My name");
        index.set_marks("claude", "a", &Marks::default()).unwrap();
        let entry = index.entry("claude", "a").unwrap().unwrap();
        assert_eq!(entry.title, "New parsed title");
        assert!(!entry.named && !entry.pinned);
        assert!(entry.tags.is_empty());
        assert_eq!(entry.bookmarks, 0);
    }

    #[test]
    fn bookmarks_resolve_to_turns_newest_first() {
        let (_home, index) = fresh();
        let mut c = conversation(
            "Parsed title",
            Some("main"),
            Some("opus"),
            &["first", "middle", &"界".repeat(121)],
        );
        for (turn, at) in c.turns.iter_mut().zip([100, 200, 300]) {
            turn.at_ms = at;
        }
        index.index_conversation(&meta("a", "t1", 1, None), &c, 1).unwrap();
        index
            .set_marks(
                "claude",
                "a",
                &Marks {
                    name: Some("Named".into()),
                    bookmarks: vec![50, 100, 250, 260, 999],
                    ..Marks::default()
                },
            )
            .unwrap();
        assert_eq!(index.turn_times("claude", "a").unwrap(), [(0, 100), (1, 200), (2, 300)]);
        assert!(index.turn_times("claude", "missing").unwrap().is_empty());
        let hits = index.bookmarks(&HistoryQuery::default()).unwrap();
        assert_eq!(
            hits.iter().map(|h| (h.turn, h.at_ms)).collect::<Vec<_>>(),
            [(2, 999), (1, 260), (0, 100)]
        );
        assert_eq!(hits[0].snippet, format!("{}…", "界".repeat(120)));
        assert_eq!(hits[1].snippet, "middle");
        assert_eq!(hits[2].snippet, "first");
        assert!(hits.iter().all(|h| h.title == "Named" && h.cwd == "c"));
        for q in [
            HistoryQuery {
                agent: Some("codex".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                project: Some("other".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                branch: Some("dev".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                model: Some("sonnet".into()),
                ..HistoryQuery::default()
            },
            HistoryQuery {
                since_ms: Some(2),
                ..HistoryQuery::default()
            },
        ] {
            assert!(index.bookmarks(&q).unwrap().is_empty());
        }
        index.mark_limited(&meta("a", "t1", 1, None), 2).unwrap();
        let hits = index.bookmarks(&HistoryQuery::default()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].turn, hits[0].at_ms, hits[0].snippet.as_str()), (0, 999, ""));
    }

    #[test]
    fn remove_clears_every_table() {
        let (_home, index) = marked_pair();
        index
            .set_marks(
                "claude",
                "b",
                &Marks {
                    tags: vec!["Keep".into()],
                    bookmarks: vec![1000],
                    ..Marks::default()
                },
            )
            .unwrap();
        index.remove("claude", "a").unwrap();
        index.remove("claude", "a").unwrap();
        for table in ["sessions", "tags", "bookmarks", "turns_fts"] {
            let count: i64 = index
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE agent = 'claude' AND session_id = 'a'"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{table}");
            let count: i64 = index
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE agent = 'claude' AND session_id = 'b'"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{table} keeps the other session");
        }
        assert_eq!(index.search("needle", false, false).unwrap()[0].session_id, "b");
        assert_eq!(index.bookmarks(&HistoryQuery::default()).unwrap()[0].session_id, "b");
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

    /// A CLI started from inside another (a Claude tool call that runs `codex exec`) inherits the tab id, so one
    /// tab can hold rows of two agents. Each agent's resume looks only at its own.
    #[test]
    fn latest_for_tab_of_skips_other_agents() {
        let (_home, index) = fresh();
        let claude = meta("A", "t1", 10, None);
        index.upsert(&claude).unwrap();
        index
            .upsert(&SessionMeta {
                agent: "codex".into(),
                ..meta("B", "t1", 20, None)
            })
            .unwrap();
        assert_eq!(index.latest_for_tab_of("t1", "claude").unwrap(), Some(claude));
        assert_eq!(
            index.latest_for_tab_of("t1", "codex").unwrap().map(|m| m.session_id),
            Some("B".into())
        );
        assert_eq!(index.latest_for_tab_of("t1", "antigravity").unwrap(), None);
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
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
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

    const TOOLS: &str = include_str!("../../tests/fixtures/claude/transcripts/2.1.309/tools.jsonl");
    const TOOLS_ID: &str = "00000000-0000-4000-8000-0000000000aa";

    fn tools_conversation() -> Conversation {
        crate::agents::claude::transcript::parse(TOOLS, TOOLS_ID)
    }

    fn conversation(title: &str, branch: Option<&str>, model: Option<&str>, prompts: &[&str]) -> Conversation {
        Conversation {
            agent: "claude".into(),
            session_id: String::new(),
            tab_id: String::new(),
            title: title.into(),
            cwd: String::new(),
            branch: branch.map(Into::into),
            model: model.map(Into::into),
            version: Some("2.1.309".into()),
            limited: false,
            marks: Default::default(),
            turns: prompts
                .iter()
                .enumerate()
                .map(|(i, p)| Turn {
                    index: i as u32,
                    at_ms: 1000 + i as u64,
                    prompt: (*p).into(),
                    items: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn fts_query_quotes_every_term() {
        let table: &[(&str, Option<&str>)] = &[
            ("foo bar", Some("\"foo\" \"bar\"*")),
            ("say \"hi\"", Some("\"say\" \"\"\"hi\"\"\"*")),
            ("AND", Some("\"AND\"*")),
            ("col:val", Some("\"col:val\"*")),
            ("", None),
            ("   ", None),
            ("*", None),
            ("עברית 🙂", Some("\"עברית\" \"🙂\"*")),
            ("  spaced   out  ", Some("\"spaced\" \"out\"*")),
            ("!!! ???", None),
            ("-x", Some("\"-x\"*")),
            ("foo * bar", Some("\"foo\" \"bar\"*")),
        ];
        for (input, want) in table {
            assert_eq!(fts_query(input).as_deref(), *want, "{input:?}");
        }
        let many = (0..100).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert_eq!(fts_query(&many).unwrap().matches('"').count(), MAX_QUERY_TERMS * 2);
    }

    #[test]
    fn search_finds_prompt_reply_tool_and_path_with_marked_snippet() {
        let (_home, index) = fresh();
        let m = meta(TOOLS_ID, "t1", 10, None);
        index.index_conversation(&m, &tools_conversation(), 7).unwrap();

        // Each word is in one place only: the prompt, the reply, a tool summary.
        for (word, place) in [("typo", "prompt"), ("done", "reply"), ("Frobnicate", "tool")] {
            let hits = index.search(word, false, false).unwrap();
            assert_eq!(hits.len(), 1, "{word} ({place})");
            assert_eq!(hits[0].turn, 0);
            assert_eq!(hits[0].session_id, TOOLS_ID);
            assert!(
                hits[0].snippet.contains(&format!("\u{2}{word}\u{3}")),
                "{:?}",
                hits[0].snippet
            );
        }
        // A path is searched by its parts, and sits in its own column.
        let hits = index.search("notes.md", false, false).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains('\u{2}'));
        let paths: String = index
            .conn
            .query_row("SELECT paths FROM turns_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(paths, "src/app.ts\nnotes.md");
        let reply: String = index
            .conn
            .query_row("SELECT reply FROM turns_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(reply, "All done.");
        // The last word is a prefix; a word that is nowhere finds nothing.
        assert_eq!(index.search("frobn", false, false).unwrap().len(), 1);
        assert!(index.search("zebra", false, false).unwrap().is_empty());
    }

    #[test]
    fn indexing_again_replaces_the_rows_and_keeps_the_session_fields() {
        let (_home, index) = fresh();
        let m = meta("a", "t1", 10, None);
        index.upsert(&m).unwrap();
        index
            .index_conversation(
                &m,
                &conversation("First", Some("main"), None, &["alpha one", "beta two"]),
                5,
            )
            .unwrap();
        assert_eq!(index.search("beta", false, false).unwrap().len(), 1);
        index
            .index_conversation(&m, &conversation("Second", None, None, &["gamma three"]), 6)
            .unwrap();
        assert!(index.search("alpha", false, false).unwrap().is_empty());
        assert!(index.search("beta", false, false).unwrap().is_empty());
        assert_eq!(index.search("gamma", false, false).unwrap().len(), 1);
        // A later upsert of the session's own fields doesn't wipe what indexing read.
        index.upsert(&meta("a", "t1", 99, None)).unwrap();
        let entry = &index.list(&HistoryQuery::default()).unwrap()[0];
        assert_eq!((entry.title.as_str(), entry.turns, entry.updated_ms), ("Second", 1, 99));
        assert_eq!(index.search("gamma", false, false).unwrap().len(), 1);
        // A session whose archive can't be read stays listed, limited, with nothing to search.
        index.mark_limited(&m, 9).unwrap();
        assert!(index.search("gamma", false, false).unwrap().is_empty());
        let entry = &index.list(&HistoryQuery::default()).unwrap()[0];
        assert!(entry.limited);
    }

    #[test]
    fn search_survives_hostile_queries() {
        let (_home, index) = fresh();
        let m = meta(TOOLS_ID, "t1", 10, None);
        index.index_conversation(&m, &tools_conversation(), 7).unwrap();
        let empty = ["", "   ", "\t\n", "*", "**", "!!!", "...", "\"\"", "()", "-"];
        for q in empty {
            assert!(index.search(q, false, false).unwrap().is_empty(), "{q:?}");
        }
        let hostile = [
            "\"",
            "foo\"bar",
            "\"unterminated",
            "AND",
            "OR",
            "NOT",
            "a OR",
            "NEAR(",
            "NEAR(a b, 2)",
            "NEAR/2",
            "-x",
            "^typo",
            "col:val",
            "prompt:typo",
            "{prompt reply}:typo",
            "(((",
            ")",
            "typo)",
            "a*b",
            "x'; DROP TABLE sessions; --",
            "עברית 🙂",
            "🙂",
            "\u{0}typo",
            "\u{2}typo\u{3}",
            "typo\u{0}",
        ];
        for q in hostile {
            index.search(q, false, false).unwrap_or_else(|e| panic!("{q:?}: {e:#}"));
        }
        // Words are plain words: "AND" is not an operator, and a column name is not a column filter.
        assert_eq!(index.search("typo", false, false).unwrap().len(), 1);
        assert!(index.search("typo AND zebra", false, false).unwrap().is_empty());
        assert!(index.search("prompt:typo", false, false).unwrap().is_empty());
        assert_eq!(index.search("\u{0}typo", false, false).unwrap().len(), 1);
        let rows: i64 = index
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn list_filters_and_orders() {
        let (_home, index) = fresh();
        let sessions = [
            ("a", "C:/work/one", Some("main"), Some("opus"), 100),
            ("b", "C:/work/two", Some("dev"), Some("sonnet"), 300),
            ("c", "C:/work/one", None, Some("sonnet"), 200),
        ];
        for (id, cwd, branch, model, updated) in sessions {
            let mut m = meta(id, "t1", updated, None);
            m.cwd = cwd.into();
            index
                .index_conversation(&m, &conversation(&format!("title {id}"), branch, model, &["hi"]), 1)
                .unwrap();
        }
        let ids =
            |q: HistoryQuery| -> Vec<String> { index.list(&q).unwrap().into_iter().map(|e| e.session_id).collect() };
        assert_eq!(ids(HistoryQuery::default()), ["b", "c", "a"]);
        let query = |f: fn(&mut HistoryQuery)| {
            let mut q = HistoryQuery::default();
            f(&mut q);
            q
        };
        assert_eq!(ids(query(|q| q.agent = Some("claude".into()))), ["b", "c", "a"]);
        assert!(ids(query(|q| q.agent = Some("codex".into()))).is_empty());
        assert_eq!(ids(query(|q| q.project = Some("C:/work/one".into()))), ["c", "a"]);
        assert_eq!(ids(query(|q| q.branch = Some("dev".into()))), ["b"]);
        assert_eq!(ids(query(|q| q.model = Some("sonnet".into()))), ["b", "c"]);
        assert_eq!(ids(query(|q| q.since_ms = Some(200))), ["b", "c"]);
        assert!(ids(query(|q| q.since_ms = Some(301))).is_empty());
        let both = HistoryQuery {
            project: Some("C:/work/one".into()),
            model: Some("sonnet".into()),
            ..HistoryQuery::default()
        };
        assert_eq!(ids(both), ["c"]);
        let entry = &index.list(&HistoryQuery::default()).unwrap()[2];
        assert_eq!(entry.title, "title a");
        assert_eq!(
            (entry.branch.as_deref(), entry.turns, entry.limited),
            (Some("main"), 1, false)
        );
    }

    #[test]
    fn a_schema_1_index_is_rebuilt() {
        let home = tempfile::tempdir().unwrap();
        let file = paths::index_file(home.path());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        {
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch(
                "CREATE TABLE sessions (
                     agent TEXT NOT NULL, session_id TEXT NOT NULL, tab_id TEXT NOT NULL, cwd TEXT NOT NULL,
                     transcript_path TEXT NOT NULL, started_ms INTEGER NOT NULL, updated_ms INTEGER NOT NULL,
                     previous_in_tab TEXT, PRIMARY KEY (agent, session_id)
                 );
                 CREATE INDEX sessions_by_tab ON sessions (tab_id, updated_ms);
                 INSERT INTO sessions VALUES ('claude', 'old', 't1', 'c', '/x', 1, 2, NULL);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        for (id, updated) in [("a", 1), ("b", 2)] {
            let src = home.path().join(format!("{id}.jsonl"));
            std::fs::write(&src, TOOLS).unwrap();
            let mut m = meta(id, "t1", updated, None);
            m.transcript_path = src.to_string_lossy().into_owned();
            archive::copy_transcript(home.path(), &m).unwrap();
        }

        let (index, opened) = Index::open(home.path()).unwrap();
        assert!(matches!(opened, Opened::Upgraded { sessions: 2 }));
        let moved: Vec<String> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("index.sqlite.v1-"))
            .collect();
        assert_eq!(moved.len(), 1, "{moved:?}");
        assert!(moved[0]["index.sqlite.v1-".len()..].chars().all(|c| c.is_ascii_digit()));
        // The sessions come from the sidecars (the old file's own row does not carry over), none is indexed yet.
        let stale: Vec<String> = index.stale().unwrap().into_iter().map(|m| m.session_id).collect();
        assert_eq!(stale, ["b", "a"]);
        assert!(index.get("claude", "old").unwrap().is_none());
        let version: i64 = index.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        drop(index);
        assert!(matches!(Index::open(home.path()).unwrap().1, Opened::Ok));
    }

    #[test]
    fn stale_lists_only_archives_whose_length_changed() {
        let home = tempfile::tempdir().unwrap();
        let src = home.path().join("a.jsonl");
        std::fs::write(&src, TOOLS).unwrap();
        let mut m = meta("a", "t1", 1, None);
        m.transcript_path = src.to_string_lossy().into_owned();
        let (index, _) = Index::open(home.path()).unwrap();
        index.upsert(&m).unwrap();
        // No archive yet: nothing to index.
        assert!(index.stale().unwrap().is_empty());
        let archived = archive::copy_transcript(home.path(), &m).unwrap();
        assert_eq!(index.stale().unwrap().len(), 1);
        let len = std::fs::metadata(&archived).unwrap().len();
        index.index_conversation(&m, &tools_conversation(), len).unwrap();
        assert!(index.stale().unwrap().is_empty());
        std::fs::write(&src, format!("{TOOLS}\n")).unwrap();
        archive::copy_transcript(home.path(), &m).unwrap();
        assert_eq!(index.stale().unwrap().len(), 1);
    }

    #[test]
    #[ignore = "timing; run with --release"]
    fn search_1000_sessions_under_100ms() {
        let (_home, index) = fresh();
        let words: Vec<String> = (0..2000).map(|i| format!("word{i}")).collect();
        let mut seed = 12345u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        for s in 0..1000 {
            let m = meta(&format!("s{s}"), "t1", s, None);
            let mut c = conversation("synthetic", Some("main"), Some("opus"), &[]);
            for t in 0..20u32 {
                let mut text = |n: usize| {
                    (0..n)
                        .map(|_| words[next() % words.len()].as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                c.turns.push(Turn {
                    index: t,
                    at_ms: 1000,
                    prompt: text(50),
                    items: vec![TurnItem::Text { markdown: text(250) }],
                });
            }
            index.index_conversation(&m, &c, 1).unwrap();
        }
        let mut times: Vec<std::time::Duration> = (0..20)
            .map(|i| {
                let started = std::time::Instant::now();
                let hits = index
                    .search(&format!("word{} word{}", 10 + i, 20 + i), false, false)
                    .unwrap();
                let took = started.elapsed();
                assert!(hits.len() <= 50);
                took
            })
            .collect();
        times.sort();
        let median = times[times.len() / 2];
        eprintln!("search median over 1000 sessions: {median:?}");
        assert!(median < std::time::Duration::from_millis(100), "median {median:?}");
    }
}
