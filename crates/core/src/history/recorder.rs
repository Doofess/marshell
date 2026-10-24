//! Records Claude Code sessions from its hook events: keeps the sessions index and sidecars up to date,
//! and copies the transcript into the archive on a blocking thread.

use super::index::{Index, Opened};
use super::marks::{self, Marks};
use super::recordings::CastCache;
use super::{RecordingLink, SessionMeta, archive, cast, transcript_allowed};
use crate::agents::claude::transcript;
use crate::registry::Registry;
use marshell_protocol::api::SessionState;
use marshell_protocol::history::{ConversationMarks, HistoryEntry, HistoryPatch, HistoryQuery, SearchHit};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The agent whose hooks feed the history so far.
const AGENT: &str = "claude";

/// How long a transcript settles before it is copied: Claude writes the turn's last lines after the hook fires.
const SETTLE: Duration = Duration::from_millis(1500);

#[derive(Debug)]
pub enum BookmarkRefusal {
    NothingYet,
    AnotherAgent,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for BookmarkRefusal {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

impl std::fmt::Display for BookmarkRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NothingYet => f.write_str("Nothing to bookmark yet: the conversation starts with its first prompt."),
            Self::AnotherAgent => f.write_str("Bookmarks work in Claude Code sessions for now."),
            Self::Failed(err) => write!(f, "{err:#}"),
        }
    }
}

#[derive(Debug)]
pub enum BookmarkTurnRefusal {
    /// The index has no time for this turn (0-based), so a bookmark could not be placed on it. Shown 1-based.
    NotIndexed(u32),
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for BookmarkTurnRefusal {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

impl std::fmt::Display for BookmarkTurnRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotIndexed(turn) => write!(
                f,
                "Marshell hasn't indexed turn {} of this conversation yet.",
                u64::from(*turn) + 1
            ),
            Self::Failed(err) => write!(f, "{err:#}"),
        }
    }
}

#[derive(Debug)]
pub enum DeleteRefusal {
    RunningInATab,
    Copying,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for DeleteRefusal {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

impl std::fmt::Display for DeleteRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RunningInATab => {
                f.write_str("This conversation is running in a tab. Close the tab first, then delete Marshell's copy.")
            }
            Self::Copying => f.write_str("Marshell is saving this conversation right now. Try again in a moment."),
            Self::Failed(err) => write!(f, "{err:#}"),
        }
    }
}

/// What the marks lock guards besides the files it serializes: tab names and in-run prompt knowledge.
#[derive(Default)]
struct TabState {
    /// Tab names, inherited by the tab's next conversation.
    names: HashMap<String, String>,
    /// Sessions that had a `UserPromptSubmit` in this run. After a restart the parsed turns say the same.
    prompted: HashSet<String>,
}

pub struct History {
    home: PathBuf,
    /// None when the index could not be opened: history is off for this run.
    index: Mutex<Option<Index>>,
    /// Serializes marks read-modify-write, delete, and hook name inheritance. Also holds tab names and the
    /// sessions that have been prompted. Always take this before the index or copying lock; never wait on it
    /// while holding either.
    marks: Mutex<TabState>,
    /// Shared with the server so live checks use the current registry, even inside the marks lock.
    registry: Option<Arc<Registry>>,
    /// Sessions being copied right now. `Some` holds the newest request that arrived during the copy,
    /// which runs once more after it.
    copying: Mutex<HashMap<String, Option<SessionMeta>>>,
    /// The wait before each copy; tests shorten it.
    settle: Mutex<Duration>,
    /// Set by `flush`: every settle wait ends at once.
    hurry: AtomicBool,
    /// Summaries of recordings already scanned, for the Recording tab.
    casts: CastCache,
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
                    Opened::Upgraded { sessions } => {
                        tracing::info!("history index upgraded; {sessions} sidecar(s) re-listed, search fills in")
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
            marks: Mutex::new(TabState::default()),
            registry: None,
            copying: Mutex::new(HashMap::new()),
            settle: Mutex::new(SETTLE),
            hurry: AtomicBool::new(false),
            casts: CastCache::default(),
        }
    }

    pub(crate) fn with_registry(mut self, registry: Arc<Registry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Sets the wait before each copy (tests only need it short).
    pub fn set_settle(&self, settle: Duration) {
        *self.settle.lock().unwrap() = settle;
    }

    /// Validates the body, updates the index and sidecar when it is a session start, stop or end or a session not
    /// yet seen in this tab, and says whether a copy should follow. `recording` is the file name of the tab's
    /// recording; the sidecar lists it once, also when the event would not otherwise write the sidecar (a restart
    /// in the same tab starts a new file). Blocking: call inside spawn_blocking.
    pub fn note_event(&self, tab_id: &str, recording: Option<&str>, body: &Value, at_ms: u64) -> Option<SessionMeta> {
        let recording = recording
            .filter(|name| cast::valid_recording_name(name))
            .map(str::to_string);
        let text = |key: &str| body.get(key).and_then(Value::as_str);
        let event = text("hook_event_name")?;
        let session_id = text("session_id")?;
        let transcript_path = text("transcript_path")?;
        let cwd = text("cwd")?;
        if !transcript_allowed(transcript_path, session_id) {
            return None;
        }
        let mut state = self.marks.lock().unwrap();
        if event == "UserPromptSubmit" {
            state.prompted.insert(session_id.to_string());
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
            recordings: recording
                .iter()
                .map(|file| RecordingLink {
                    file: file.clone(),
                    first_ms: at_ms,
                    last_ms: at_ms,
                })
                .collect(),
        };
        let ends_turn = matches!(event, "Stop" | "StopFailure" | "SessionEnd");
        if is_new || ends_turn || event == "SessionStart" {
            if let Err(err) = index.upsert(&meta) {
                tracing::warn!("history index write failed: {err:#}");
            }
            if let Err(err) = archive::write_meta(&self.home, &meta) {
                tracing::warn!("history sidecar write failed: {err:#}");
            }
            if is_new && let Some(name) = state.names.get(tab_id) {
                let mut marks = marks::read(&self.home, AGENT, session_id);
                if marks.name.is_none() {
                    marks.name = Some(name.clone());
                    if let Err(err) = self.save_marks(index, AGENT, session_id, &marks) {
                        tracing::warn!("history name inheritance failed: {err:#}");
                    }
                }
            }
        } else if let Some(name) = &recording {
            match archive::read_meta(&self.home, AGENT, session_id) {
                Ok(Some(listed)) if !listed.recordings.iter().any(|l| &l.file == name) => {
                    if let Err(err) = archive::write_meta(&self.home, &meta) {
                        tracing::warn!("history sidecar write failed: {err:#}");
                    }
                }
                Ok(_) => {}
                Err(err) => tracing::warn!("history sidecar read failed: {err:#}"),
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

    /// False when the index could not be opened: history is off for this run.
    pub fn is_on(&self) -> bool {
        self.index.lock().unwrap().is_some()
    }

    /// The session's sidecar data from the index. None when the index doesn't have it, and when history is off.
    /// Blocking.
    pub fn get(&self, agent: &str, session_id: &str) -> anyhow::Result<Option<SessionMeta>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => index.get(agent, session_id),
            None => Ok(None),
        }
    }

    /// The recording summaries cache.
    pub fn casts(&self) -> &CastCache {
        &self.casts
    }

    /// When the conversation's first prompt with a known time was sent, from the index. None when it has none
    /// yet, and when history is off. Blocking.
    pub fn first_prompt_ms(&self, agent: &str, session_id: &str) -> anyhow::Result<Option<u64>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => Ok(index
                .turn_times(agent, session_id)?
                .into_iter()
                .map(|(_, at_ms)| at_ms)
                .find(|at_ms| *at_ms != 0)),
            None => Ok(None),
        }
    }

    /// The session as the list shows it. None when the index doesn't have it, and when history is off. Blocking.
    pub fn entry(&self, agent: &str, session_id: &str) -> anyhow::Result<Option<HistoryEntry>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => index.entry(agent, session_id),
            None => Ok(None),
        }
    }

    /// The listed conversations, newest first. Empty when history is off for this run. Blocking.
    pub fn list(&self, query: &HistoryQuery) -> anyhow::Result<Vec<HistoryEntry>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => index.list(query),
            None => Ok(Vec::new()),
        }
    }

    /// Full-text search over the indexed conversations. Empty when history is off for this run. Blocking.
    pub fn search(&self, query: &str, include_archived: bool) -> anyhow::Result<Vec<SearchHit>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => index.search(query, include_archived),
            None => Ok(Vec::new()),
        }
    }

    pub fn bookmarks(&self, query: &HistoryQuery) -> anyhow::Result<Vec<SearchHit>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => index.bookmarks(query),
            None => Ok(Vec::new()),
        }
    }

    /// Caller holds the marks lock from before the read through both writes.
    fn save_marks(&self, index: &Index, agent: &str, session_id: &str, marks: &Marks) -> anyhow::Result<()> {
        marks::write(&self.home, agent, session_id, marks)?;
        index.set_marks(agent, session_id, marks)
    }

    pub fn name_tab(&self, tab_id: &str, name: Option<&str>) {
        let mut state = self.marks.lock().unwrap();
        match name.and_then(marks::normalize_name) {
            Some(name) => {
                state.names.insert(tab_id.to_string(), name);
            }
            None => {
                state.names.remove(tab_id);
            }
        }
    }

    /// The tab route's whole rename, including an ended tab or one with no conversation yet.
    /// Keep registry, inherited name and marks in the same order across concurrent requests.
    pub(crate) fn rename_tab(&self, tab_id: &str, name: &str) -> anyhow::Result<bool> {
        let mut state = self.marks.lock().unwrap();
        let Some(name) = marks::normalize_name(name) else {
            return Ok(false);
        };
        if !self
            .registry
            .as_ref()
            .is_some_and(|registry| registry.rename(tab_id, &name))
        {
            return Ok(false);
        }
        state.names.insert(tab_id.to_string(), name.clone());
        let guard = self.index.lock().unwrap();
        if let Some(index) = guard.as_ref()
            && let Some(meta) = index.latest_for_tab(tab_id)?
        {
            let mut marks = marks::read(&self.home, &meta.agent, &meta.session_id);
            marks.name = Some(name);
            self.save_marks(index, &meta.agent, &meta.session_id, &marks)?;
        }
        Ok(true)
    }

    /// Same rule as Resume: a Running tab whose newest conversation is this one.
    fn running_in_tab(&self, index: &Index, meta: &SessionMeta) -> anyhow::Result<bool> {
        let running = self.registry.as_ref().is_some_and(|registry| {
            registry
                .snapshot()
                .sessions
                .iter()
                .any(|tab| tab.tab_id == meta.tab_id && tab.state == SessionState::Running)
        });
        Ok(running
            && index
                .latest_for_tab(&meta.tab_id)?
                .is_some_and(|latest| latest.session_id == meta.session_id))
    }

    pub fn patch(&self, agent: &str, session_id: &str, patch: &HistoryPatch) -> Result<Marks, &'static str> {
        let tags = patch.tags.as_deref().map(marks::normalize_tags).transpose()?;
        let mut state = self.marks.lock().unwrap();
        let guard = self.index.lock().unwrap();
        let index = guard.as_ref().ok_or("History is off for this run.")?;
        let failed = |err: anyhow::Error| {
            tracing::warn!("history marks write failed: {err:#}");
            "Could not save conversation marks."
        };
        let meta = index
            .get(agent, session_id)
            .map_err(failed)?
            .ok_or("No such conversation.")?;
        let mut marks = marks::read(&self.home, agent, session_id);
        if let Some(name) = &patch.name {
            marks.name = marks::normalize_name(name);
        }
        if let Some(tags) = tags {
            marks.tags = tags;
        }
        if let Some(pinned) = patch.pinned {
            marks.pinned = pinned;
        }
        if let Some(archived) = patch.archived {
            marks.archived = archived;
        }
        self.save_marks(index, agent, session_id, &marks).map_err(failed)?;
        if patch.name.is_some() && self.running_in_tab(index, &meta).map_err(failed)? {
            // Keep this inside the same lock, so a later rename can't be overwritten by an earlier response.
            if let Some(entry) = index.entry(agent, session_id).map_err(failed)?
                && let Some(registry) = &self.registry
            {
                registry.rename(&meta.tab_id, &entry.title);
            }
            match &marks.name {
                Some(name) => {
                    state.names.insert(meta.tab_id, name.clone());
                }
                None => {
                    state.names.remove(&meta.tab_id);
                }
            }
        }
        Ok(marks)
    }

    pub fn bookmark_turn(&self, agent: &str, session_id: &str, turn: u32, on: bool) -> Result<(), BookmarkTurnRefusal> {
        let _marks = self.marks.lock().unwrap();
        let guard = self.index.lock().unwrap();
        let index = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("History is off for this run."))?;
        if index.get(agent, session_id)?.is_none() {
            return Err(anyhow::anyhow!("No such conversation.").into());
        }
        let turns = index.turn_times(agent, session_id)?;
        let mut marks = marks::read(&self.home, agent, session_id);
        if on {
            let (_, at_ms) = turns
                .iter()
                .find(|(n, _)| *n == turn)
                .ok_or(BookmarkTurnRefusal::NotIndexed(turn))?;
            marks::add_bookmark(&mut marks, *at_ms);
        } else {
            marks.bookmarks.retain(|at| marks::turn_for(*at, &turns) != turn);
        }
        Ok(self.save_marks(index, agent, session_id, &marks)?)
    }

    pub fn bookmark_now(&self, tab_id: &str, at_ms: u64) -> Result<String, BookmarkRefusal> {
        let state = self.marks.lock().unwrap();
        if let Some(registry) = &self.registry {
            let tab = registry
                .snapshot()
                .sessions
                .into_iter()
                .find(|tab| tab.tab_id == tab_id)
                .ok_or(BookmarkRefusal::NothingYet)?;
            if tab.agent.as_deref() != Some(AGENT) {
                return Err(BookmarkRefusal::AnotherAgent);
            }
        }
        let guard = self.index.lock().unwrap();
        let index = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("History is off for this run."))?;
        let meta = index.latest_for_tab(tab_id)?.ok_or(BookmarkRefusal::NothingYet)?;
        if meta.agent != AGENT {
            return Err(BookmarkRefusal::AnotherAgent);
        }
        // The row exists from SessionStart; the conversation starts with its first prompt.
        if !state.prompted.contains(&meta.session_id) && index.turn_times(AGENT, &meta.session_id)?.is_empty() {
            return Err(BookmarkRefusal::NothingYet);
        }
        let entry = index
            .entry(AGENT, &meta.session_id)?
            .ok_or(BookmarkRefusal::NothingYet)?;
        let mut marks = marks::read(&self.home, AGENT, &meta.session_id);
        marks::add_bookmark(&mut marks, at_ms);
        self.save_marks(index, AGENT, &meta.session_id, &marks)?;
        Ok(entry.title)
    }

    pub fn marks(&self, agent: &str, session_id: &str) -> ConversationMarks {
        let _marks = self.marks.lock().unwrap();
        let marks = marks::read(&self.home, agent, session_id);
        let turns = self
            .index
            .lock()
            .unwrap()
            .as_ref()
            .map(|index| index.turn_times(agent, session_id));
        let turns = match turns {
            Some(Ok(turns)) => turns,
            Some(Err(err)) => {
                tracing::warn!("could not resolve bookmarks: {err:#}");
                Vec::new()
            }
            None => Vec::new(),
        };
        let mut bookmarked_turns: Vec<_> = marks.bookmarks.iter().map(|at| marks::turn_for(*at, &turns)).collect();
        bookmarked_turns.sort_unstable();
        bookmarked_turns.dedup();
        ConversationMarks {
            name: marks.name,
            tags: marks.tags,
            pinned: marks.pinned,
            archived: marks.archived,
            bookmarked_turns,
        }
    }

    pub fn delete_copy(&self, agent: &str, session_id: &str) -> Result<(), DeleteRefusal> {
        let _marks = self.marks.lock().unwrap();
        if self.copying.lock().unwrap().contains_key(session_id) {
            return Err(DeleteRefusal::Copying);
        }
        let guard = self.index.lock().unwrap();
        let index = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("History is off for this run."))?;
        if let Some(meta) = index.get(agent, session_id)?
            && self.running_in_tab(index, &meta)?
        {
            return Err(DeleteRefusal::RunningInATab);
        }
        index.remove(agent, session_id)?;
        drop(guard);
        let (archive, meta) = archive::archive_paths(&self.home, agent, session_id);
        for path in [archive, meta, marks::marks_path(&self.home, agent, session_id)] {
            for path in [&path, &crate::store::bak_path(&path)] {
                match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => return Err(anyhow::Error::from(err).into()),
                }
            }
        }
        Ok(())
    }

    /// Reads the session's archive copy and replaces what the index holds of it. The file is read and parsed
    /// with the index free; the lock is held only for the write. An archive that can't be read in full leaves
    /// the session listed, marked limited, with nothing to search. Does nothing when there is no archive or
    /// the agent isn't one whose transcripts Marshell reads. Blocking.
    pub fn reindex(&self, meta: &SessionMeta) {
        if meta.agent != AGENT {
            return;
        }
        let (archive, _) = archive::archive_paths(&self.home, &meta.agent, &meta.session_id);
        // Taken before the read: a copy that lands in between leaves the session stale, and it is indexed again.
        let Ok(len) = std::fs::metadata(&archive).map(|m| m.len()) else {
            return;
        };
        let conversation = match archive::read_archive(&self.home, &meta.agent, &meta.session_id) {
            Ok(Some(text)) => Some(transcript::parse(&text, &meta.session_id)),
            Ok(None) => return,
            Err(err) => {
                tracing::warn!("could not read archived transcript {}: {err:#}", meta.session_id);
                None
            }
        };
        let _marks = self.marks.lock().unwrap();
        let guard = self.index.lock().unwrap();
        let Some(index) = guard.as_ref() else {
            return;
        };
        // A delete may have finished while this archive was being parsed. Never recreate its row.
        if !matches!(index.get(&meta.agent, &meta.session_id), Ok(Some(_))) {
            return;
        }
        let written = match &conversation {
            Some(c) => index.index_conversation(meta, c, len),
            None => index.mark_limited(meta, len),
        };
        if let Err(err) = written {
            tracing::warn!("could not index conversation {}: {err:#}", meta.session_id);
        }
    }

    /// Indexes, newest first and one at a time on a blocking thread, every session whose archive the index
    /// hasn't read yet. Takes the index only for each write, so requests and hooks never wait on it long.
    /// Stops early when a flush has started.
    pub fn backfill(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn_blocking(move || {
            let stale = match self.index.lock().unwrap().as_ref().map(Index::stale) {
                Some(Ok(stale)) => stale,
                Some(Err(err)) => {
                    tracing::warn!("history backfill could not list sessions: {err:#}");
                    return;
                }
                None => return,
            };
            if !stale.is_empty() {
                tracing::info!("history backfill: indexing {} conversation(s)", stale.len());
            }
            for meta in stale {
                if self.hurry.load(Ordering::SeqCst) {
                    break;
                }
                self.reindex(&meta);
            }
        })
    }

    /// True when no copy is running or waiting.
    pub fn is_idle(&self) -> bool {
        self.copying.lock().unwrap().is_empty()
    }

    /// Sleeps `settle` in short slices, and stops at once when a flush has set the hurry.
    fn settle_wait(&self, settle: Duration) {
        let started = Instant::now();
        while !self.hurry.load(Ordering::SeqCst) {
            let left = settle.saturating_sub(started.elapsed());
            if left.is_zero() {
                break;
            }
            std::thread::sleep(left.min(Duration::from_millis(25)));
        }
    }

    /// Ends every settle wait now (and skips it for copies asked for later), then waits until no copy is
    /// running or waiting, or `cap` has passed. For shutdown: the hurry stays on. Blocking.
    pub fn flush(&self, cap: Duration) {
        self.hurry.store(true, Ordering::SeqCst);
        let started = Instant::now();
        while !self.is_idle() && started.elapsed() < cap {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

/// Copies on a blocking thread after the settle time, one copy at a time per session; a request during the wait or
/// a copy runs one more copy after it.
pub fn schedule_copy(history: Arc<History>, meta: SessionMeta) {
    {
        let _marks = history.marks.lock().unwrap();
        // A queued hook may reach this after a delete. Only a new hook can add the session again.
        if !matches!(history.get(&meta.agent, &meta.session_id), Ok(Some(_))) {
            return;
        }
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
            history.settle_wait(settle);
            match archive::copy_transcript(&history.home, &current) {
                // Not during a flush: shutdown is waiting, and the backfill reads this copy next start.
                Ok(_) if !history.hurry.load(Ordering::SeqCst) => history.reindex(&current),
                Ok(_) => {}
                Err(err) => tracing::warn!("could not archive transcript {id}: {err:#}"),
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

        assert_eq!(
            history.note_event("t1", None, &body(START, home.path(), ID), 1000),
            None
        );
        let row = history.latest_for_tab("t1").unwrap();
        assert_eq!(row.session_id, ID);
        assert_eq!(row.started_ms, 1000);
        assert!(sidecar(home.path(), ID).exists());

        let stopped = history
            .note_event("t1", None, &body(STOP, home.path(), ID), 2000)
            .unwrap();
        assert_eq!(stopped.started_ms, 1000);
        assert_eq!(stopped.updated_ms, 2000);

        let before_bytes = std::fs::read(sidecar(home.path(), ID)).unwrap();
        let before_time = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        assert_eq!(
            history.note_event("t1", None, &body(PRE_TOOL, home.path(), ID), 3000),
            None
        );
        assert_eq!(std::fs::read(sidecar(home.path(), ID)).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap(),
            before_time
        );

        let ended = history
            .note_event("t1", None, &body(END, home.path(), ID), 4000)
            .unwrap();
        assert_eq!(ended.started_ms, 1000);
        assert_eq!(ended.updated_ms, 4000);
    }

    #[test]
    fn stop_failure_ends_a_turn() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        history.note_event("t1", None, &body(START, home.path(), ID), 1000);
        let failure = include_str!("../../tests/fixtures/claude/2.1.309/StopFailure.json");
        let meta = history
            .note_event("t1", None, &body(failure, home.path(), ID), 2000)
            .unwrap();
        assert_eq!(meta.updated_ms, 2000);
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

        let meta = history
            .note_event("t1", None, &body(STOP, home.path(), ID), 2000)
            .unwrap();
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

    #[tokio::test]
    async fn flush_copies_at_once_without_waiting_the_settle() {
        let home = tempfile::tempdir().unwrap();
        let history = Arc::new(History::open(home.path()));
        history.set_settle(Duration::from_secs(10));
        let transcript = home.path().join(format!("{ID}.jsonl"));
        std::fs::write(&transcript, "{\"n\":1}\n{\"n\":2}\n").unwrap();

        let meta = history
            .note_event("t1", None, &body(STOP, home.path(), ID), 2000)
            .unwrap();
        schedule_copy(history.clone(), meta);
        assert!(!history.is_idle());

        let started = Instant::now();
        history.flush(Duration::from_secs(3));
        assert!(started.elapsed() < Duration::from_secs(2), "flush waited the settle");
        assert!(history.is_idle());
        let (archive, _) = crate::history::archive::archive_paths(home.path(), "claude", ID);
        let decoded = zstd::decode_all(std::fs::File::open(&archive).unwrap()).unwrap();
        assert_eq!(decoded, std::fs::read(&transcript).unwrap());

        // A copy asked for after the flush skips the wait too.
        std::fs::write(&transcript, "{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n").unwrap();
        let meta = history
            .note_event("t1", None, &body(STOP, home.path(), ID), 3000)
            .unwrap();
        schedule_copy(history.clone(), meta);
        history.flush(Duration::from_secs(3));
        let decoded = zstd::decode_all(std::fs::File::open(&archive).unwrap()).unwrap();
        assert!(String::from_utf8(decoded).unwrap().contains("{\"n\":3}"));
    }

    #[test]
    fn clear_links_previous_in_tab() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        history.note_event("t1", None, &body(START, home.path(), "aaa"), 1000);
        history.note_event("t1", None, &body(START, home.path(), "bbb"), 2000);
        let latest = history.latest_for_tab("t1").unwrap();
        assert_eq!(latest.session_id, "bbb");
        assert_eq!(latest.previous_in_tab.as_deref(), Some("aaa"));
        assert_eq!(history.latest_for_tab("t2"), None);
    }

    #[test]
    fn note_event_lists_each_tab_recording_once_in_order() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let listed = || {
            archive::read_meta(home.path(), "claude", ID)
                .unwrap()
                .unwrap()
                .recordings
                .into_iter()
                .map(|link| link.file)
                .collect::<Vec<_>>()
        };
        let (a, b, b2) = ("ta-1000.cast", "tb-2000.cast", "tb-3000.cast");

        history.note_event("ta", Some(a), &body(START, home.path(), ID), 1000);
        assert_eq!(listed(), [a]);
        // Resumed in tab B.
        history.note_event("tb", Some(b), &body(START, home.path(), ID), 2000);
        assert_eq!(listed(), [a, b]);
        // A restart in tab B: a new file, with no start or stop event to write the sidecar for it.
        history.note_event("tb", Some(b2), &body(PRE_TOOL, home.path(), ID), 3000);
        assert_eq!(listed(), [a, b, b2]);
        // Nothing new: the sidecar is not rewritten.
        let before = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        history.note_event("tb", Some(b2), &body(PRE_TOOL, home.path(), ID), 3500);
        history.note_event("tb", None, &body(PRE_TOOL, home.path(), ID), 3600);
        assert_eq!(listed(), [a, b, b2]);
        assert_eq!(
            std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap(),
            before
        );
        // A name that is not a recording name is never listed.
        history.note_event("tb", Some("..\\x.cast"), &body(STOP, home.path(), ID), 4000);
        assert_eq!(listed(), [a, b, b2]);
    }

    #[test]
    fn a_turn_end_moves_the_recordings_last_event_and_other_events_write_nothing() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let links = || {
            archive::read_meta(home.path(), "claude", ID)
                .unwrap()
                .unwrap()
                .recordings
        };
        let link = |file: &str, first_ms, last_ms| RecordingLink {
            file: file.into(),
            first_ms,
            last_ms,
        };
        let (a, b) = ("ta-1000.cast", "tb-2000.cast");
        history.note_event("ta", Some(a), &body(START, home.path(), ID), 1000);
        assert_eq!(links(), [link(a, 1000, 1000)]);
        history.note_event("ta", Some(a), &body(STOP, home.path(), ID), 5000);
        assert_eq!(links(), [link(a, 1000, 5000)]);
        // Resumed in tab B: A keeps where it ended, and B starts its own.
        history.note_event("tb", Some(b), &body(START, home.path(), ID), 9000);
        assert_eq!(links(), [link(a, 1000, 5000), link(b, 9000, 9000)]);
        history.note_event("tb", Some(b), &body(STOP, home.path(), ID), 12_000);
        assert_eq!(links(), [link(a, 1000, 5000), link(b, 9000, 12_000)]);
        // An event in a recording the sidecar already lists writes nothing.
        let before = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        history.note_event("tb", Some(b), &body(PRE_TOOL, home.path(), ID), 13_000);
        assert_eq!(links(), [link(a, 1000, 5000), link(b, 9000, 12_000)]);
        assert_eq!(
            std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap(),
            before
        );
    }

    #[test]
    fn an_unseen_session_is_recorded_on_any_event() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        assert_eq!(
            history.note_event("t1", None, &body(PRE_TOOL, home.path(), ID), 1500),
            None
        );
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
            assert_eq!(history.note_event("t1", None, &hostile, 1000), None);
        }
        assert_eq!(history.latest_for_tab("t1"), None);
        let archive = paths::archive_dir(home.path(), "claude");
        let files = std::fs::read_dir(archive).map(|d| d.count()).unwrap_or(0);
        assert_eq!(files, 0);
    }

    const TOOLS: &str = include_str!("../../tests/fixtures/claude/transcripts/2.1.309/tools.jsonl");

    /// A transcript at `<dir>/<id>.jsonl` and the Stop event that asks for its copy.
    fn stopped(history: &History, dir: &Path, id: &str, text: &str, at_ms: u64) -> SessionMeta {
        std::fs::write(dir.join(format!("{id}.jsonl")), text).unwrap();
        history.note_event("t1", None, &body(STOP, dir, id), at_ms).unwrap()
    }

    async fn idle(history: &History) {
        let started = Instant::now();
        while !history.is_idle() {
            assert!(started.elapsed() < Duration::from_secs(5), "copy did not finish");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn reindex_after_copy_makes_it_searchable() {
        let home = tempfile::tempdir().unwrap();
        let history = Arc::new(History::open(home.path()));
        history.set_settle(Duration::from_millis(20));
        let meta = stopped(&history, home.path(), ID, TOOLS, 2000);
        assert!(history.search("typo", false).unwrap().is_empty());

        schedule_copy(history.clone(), meta);
        idle(&history).await;

        let hits = history.search("typo", false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, ID);
        assert!(hits[0].snippet.contains("\u{2}typo\u{3}"));
        let listed = history.list(&HistoryQuery::default()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].turns, listed[0].branch.as_deref()), (1, Some("main")));
        assert!(!listed[0].limited);
    }

    #[tokio::test]
    async fn backfill_indexes_what_was_archived_before_and_skips_what_is_current() {
        let home = tempfile::tempdir().unwrap();
        {
            let history = History::open(home.path());
            let meta = stopped(&history, home.path(), ID, TOOLS, 2000);
            // Archived, but never indexed: as after an upgrade from schema 1.
            crate::history::archive::copy_transcript(home.path(), &meta).unwrap();
            assert!(history.search("typo", false).unwrap().is_empty());
        }
        let history = Arc::new(History::open(home.path()));
        history.clone().backfill().await.unwrap();
        assert_eq!(history.search("typo", false).unwrap().len(), 1);
        // Everything is current now: a second pass has nothing to do.
        assert!(
            history
                .index
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .stale()
                .unwrap()
                .is_empty()
        );
        history.backfill().await.unwrap();
    }

    #[test]
    fn an_unreadable_archive_leaves_the_session_listed_and_limited() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let meta = stopped(&history, home.path(), ID, TOOLS, 2000);
        let (archive, _) = crate::history::archive::archive_paths(home.path(), "claude", ID);
        std::fs::write(&archive, b"not zstd").unwrap();
        history.reindex(&meta);
        let listed = history.list(&HistoryQuery::default()).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].limited);
        assert!(history.search("typo", false).unwrap().is_empty());
        // Already read once: not stale again until the archive changes.
        assert!(
            history
                .index
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .stale()
                .unwrap()
                .is_empty()
        );
    }
}
