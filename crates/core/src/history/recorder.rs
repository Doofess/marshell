//! Records Claude Code sessions from its hook events: keeps the sessions index and sidecars up to date,
//! and copies the transcript into the archive on a blocking thread.

use super::index::{Index, Opened};
use super::marks::{self, Marks};
use super::recordings::CastCache;
use super::{RecordingLink, SessionMeta, archive, cast, sidecar_allowed_in, transcript_allowed};
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

/// A path a CLI in the WSL `distro` reported, as Windows reads it: a Linux absolute path goes through
/// `wsl::to_windows` (None when it won't translate), anything else is kept for the checks that follow to judge.
/// Without a distro the path is kept as it is.
fn as_windows(distro: Option<&str>, path: &str) -> Option<String> {
    match distro {
        Some(distro) if path.starts_with('/') => {
            crate::wsl::to_windows(distro, path).map(|p| p.to_string_lossy().into_owned())
        }
        _ => Some(path.to_string()),
    }
}

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
    /// in the same tab starts a new file). A Codex session is linked by the first event that names it, whatever
    /// the event (a quit with no prompt sends only `SessionEnd`), and never asks for a copy: the importer copies
    /// the rollout. An Antigravity conversation is linked the same way, for Resume only: its folder is the tab's
    /// and its `lastUserInput` is never read. `distro` is the WSL distro the tab's shell runs in: the Linux paths its
    /// agent reports (`transcript_path`, `cwd`) are read as Windows paths under that distro's share before anything
    /// checks, stores or copies them, and a path that won't translate records nothing. Blocking: call inside
    /// spawn_blocking.
    pub fn note_event(
        &self,
        agent: &str,
        tab_id: &str,
        recording: Option<&str>,
        body: &Value,
        at_ms: u64,
        distro: Option<&str>,
    ) -> Option<SessionMeta> {
        // Codex's and Antigravity's sessions are linked by the first event that names them, whatever it is.
        let (codex, antigravity) = match agent {
            "claude" => (false, false),
            "codex" => (true, false),
            "antigravity" => (true, true),
            _ => return None,
        };
        let recording = recording
            .filter(|name| cast::valid_recording_name(name))
            .map(str::to_string);
        let text = |key: &str| body.get(key).and_then(Value::as_str);
        // Only the fields below are read from an agy body: its `lastUserInput` is the person's own text.
        let (event, session_id, transcript_path, cwd) = if antigravity {
            // agy's body has no event name; the bridge adds it from `--event`. The folder is the tab's: agy's
            // `workspacePaths` is always empty. A tab the registry doesn't know has no folder, so no link.
            let cwd = self.registry.as_ref()?.cwd_of_tab(tab_id)?;
            (
                text("hook_event_name")?,
                text("conversationId")?,
                text("transcriptPath")?,
                cwd,
            )
        } else {
            (
                text("hook_event_name")?,
                text("session_id")?,
                text("transcript_path")?,
                text("cwd")?.to_string(),
            )
        };
        let (transcript_path, cwd) = (as_windows(distro, transcript_path)?, as_windows(distro, &cwd)?);
        let allowed = if codex {
            sidecar_allowed_in(agent, &transcript_path, session_id, distro)
        } else {
            transcript_allowed(&transcript_path, session_id, distro)
        };
        if !allowed {
            return None;
        }
        let mut state = self.marks.lock().unwrap();
        if event == "UserPromptSubmit" && !codex {
            state.prompted.insert(session_id.to_string());
        }
        let guard = self.index.lock().unwrap();
        let index = guard.as_ref()?;

        let row = match index.get(agent, session_id) {
            Ok(row) => row,
            Err(err) => {
                tracing::warn!("history index read failed: {err:#}");
                return None;
            }
        };
        // An imported rollout has a row with no tab: the link keeps its start.
        let imported = row.as_ref().filter(|row| codex && row.tab_id.is_empty()).cloned();
        let known = row.filter(|row| row.tab_id == tab_id);
        let is_new = known.is_none();
        let (started_ms, previous_in_tab) = match known {
            Some(row) => (row.started_ms, row.previous_in_tab),
            None if imported.is_some() => {
                let previous = index
                    .latest_for_tab(tab_id)
                    .ok()
                    .flatten()
                    .map(|row| row.session_id)
                    .filter(|id| id != session_id);
                (imported.as_ref().map_or(at_ms, |row| row.started_ms), previous)
            }
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
            agent: agent.to_string(),
            session_id: session_id.to_string(),
            tab_id: tab_id.to_string(),
            cwd,
            transcript_path,
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
            client: Default::default(),
            // Only Claude Code tabs have a preset; the registry has the tab while it is open.
            preset: self
                .registry
                .as_ref()
                .filter(|_| !codex)
                .and_then(|registry| registry.preset_of_tab(tab_id)),
            distro: distro.map(str::to_string),
        };
        let ends_turn = !codex && matches!(event, "Stop" | "StopFailure" | "SessionEnd");
        // An event of an agy conversation the tab went back to makes it the tab's latest again (the latest is the
        // row with the newest `updated_ms`); events of the tab's current one write nothing.
        let returned = antigravity
            && !is_new
            && index
                .latest_for_tab_of(tab_id, agent)
                .ok()
                .flatten()
                .is_some_and(|latest| latest.session_id != session_id);
        if is_new || returned || ends_turn || (!codex && event == "SessionStart") {
            if let Err(err) = index.upsert(&meta) {
                tracing::warn!("history index write failed: {err:#}");
            }
            if let Err(err) = archive::write_meta(&self.home, &meta) {
                tracing::warn!("history sidecar write failed: {err:#}");
            }
            if is_new && let Some(name) = state.names.get(tab_id) {
                let mut marks = marks::read(&self.home, agent, session_id);
                if marks.name.is_none() {
                    marks.name = Some(name.clone());
                    if let Err(err) = self.save_marks(index, agent, session_id, &marks) {
                        tracing::warn!("history name inheritance failed: {err:#}");
                    }
                }
            }
        } else if let Some(name) = &recording {
            match archive::read_meta(&self.home, agent, session_id) {
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

    /// The tab's most recent session of `agent`, if the index knows one.
    pub fn latest_for_tab_of(&self, tab_id: &str, agent: &str) -> Option<SessionMeta> {
        self.index
            .lock()
            .unwrap()
            .as_ref()?
            .latest_for_tab_of(tab_id, agent)
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
    pub fn search(&self, query: &str, include_archived: bool, headless: bool) -> anyhow::Result<Vec<SearchHit>> {
        match self.index.lock().unwrap().as_ref() {
            Some(index) => index.search(query, include_archived, headless),
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
        // Before anything is removed, so a delete that can't be remembered doesn't happen: the importer would
        // copy an unchanged source file back.
        let deleted_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        archive::add_tombstone(&self.home, agent, session_id, deleted_at_ms)?;
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
        if super::parse_for(&meta.agent, "", &meta.session_id).is_none() {
            return;
        }
        let (archive, _) = archive::archive_paths(&self.home, &meta.agent, &meta.session_id);
        // Taken before the read: a copy that lands in between leaves the session stale, and it is indexed again.
        let Ok(len) = std::fs::metadata(&archive).map(|m| m.len()) else {
            return;
        };
        let conversation = match archive::read_archive(&self.home, &meta.agent, &meta.session_id) {
            Ok(Some(text)) => super::parse_for(&meta.agent, &text, &meta.session_id),
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

    /// Copies a conversation the importer found outside Marshell into the archive and indexes it, on the
    /// caller's thread. The row goes into the index first, and the session counts as copying until the end, so
    /// "Delete Marshell's copy" refuses meanwhile and `flush` waits for it. `Ok(false)` when nothing was
    /// done: the session is being copied by the hook path (the next scan tries again), or history is off.
    /// A Marshell tab's ownership is kept by the index and the sidecar when `meta` has no tab. Blocking.
    pub fn import(&self, meta: &SessionMeta, source: &Path) -> anyhow::Result<bool> {
        self.import_bytes(meta, &archive::read_source(source)?)
    }

    /// Like `import`, for a transcript the caller has already read.
    pub fn import_bytes(&self, meta: &SessionMeta, bytes: &[u8]) -> anyhow::Result<bool> {
        let existed = {
            let _marks = self.marks.lock().unwrap();
            let mut copying = self.copying.lock().unwrap();
            if copying.contains_key(&meta.session_id) {
                return Ok(false);
            }
            let guard = self.index.lock().unwrap();
            let Some(index) = guard.as_ref() else {
                return Ok(false);
            };
            let existed = index.get(&meta.agent, &meta.session_id)?.is_some();
            index.upsert(meta)?;
            copying.insert(meta.session_id.clone(), None);
            existed
        };
        let result = match archive::copy_bytes(&self.home, meta, bytes) {
            Ok(_) => {
                // Not during a flush: shutdown is waiting, and the backfill reads this copy next start.
                if !self.hurry.load(Ordering::SeqCst) {
                    self.reindex(meta);
                }
                Ok(true)
            }
            Err(err) => {
                if !existed {
                    // No archive was made, so don't leave a row that lists nothing.
                    let _marks = self.marks.lock().unwrap();
                    if let Some(index) = self.index.lock().unwrap().as_ref() {
                        let _ = index.remove(&meta.agent, &meta.session_id);
                    }
                }
                Err(err)
            }
        };
        self.finish_copying(&meta.session_id);
        result
    }

    /// Ends an import's hold on the session. A hook copy asked for meanwhile waited on it (`schedule_copy` only
    /// queues behind a running copy), so it runs here, as `schedule_copy`'s own loop would run it.
    fn finish_copying(&self, session_id: &str) {
        loop {
            let next = {
                let mut copying = self.copying.lock().unwrap();
                match copying.get_mut(session_id).and_then(Option::take) {
                    Some(next) => next,
                    None => {
                        copying.remove(session_id);
                        return;
                    }
                }
            };
            let settle = *self.settle.lock().unwrap();
            self.settle_wait(settle);
            match archive::copy_transcript(&self.home, &next) {
                Ok(_) if !self.hurry.load(Ordering::SeqCst) => self.reindex(&next),
                Ok(_) => {}
                Err(err) => tracing::warn!("could not archive transcript {session_id}: {err:#}"),
            }
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

    /// The Marshell home folder this history lives in.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// True once a flush has started: shutdown is under way and nothing new should begin.
    pub fn is_hurrying(&self) -> bool {
        self.hurry.load(Ordering::SeqCst)
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
    use marshell_protocol::history::Client;
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
            history.note_event("claude", "t1", None, &body(START, home.path(), ID), 1000, None),
            None
        );
        let row = history.latest_for_tab("t1").unwrap();
        assert_eq!(row.session_id, ID);
        assert_eq!(row.started_ms, 1000);
        assert!(sidecar(home.path(), ID).exists());

        let stopped = history
            .note_event("claude", "t1", None, &body(STOP, home.path(), ID), 2000, None)
            .unwrap();
        assert_eq!(stopped.started_ms, 1000);
        assert_eq!(stopped.updated_ms, 2000);

        let before_bytes = std::fs::read(sidecar(home.path(), ID)).unwrap();
        let before_time = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        assert_eq!(
            history.note_event("claude", "t1", None, &body(PRE_TOOL, home.path(), ID), 3000, None),
            None
        );
        assert_eq!(std::fs::read(sidecar(home.path(), ID)).unwrap(), before_bytes);
        assert_eq!(
            std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap(),
            before_time
        );

        let ended = history
            .note_event("claude", "t1", None, &body(END, home.path(), ID), 4000, None)
            .unwrap();
        assert_eq!(ended.started_ms, 1000);
        assert_eq!(ended.updated_ms, 4000);
    }

    #[test]
    fn stop_failure_ends_a_turn() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        history.note_event("claude", "t1", None, &body(START, home.path(), ID), 1000, None);
        let failure = include_str!("../../tests/fixtures/claude/2.1.309/StopFailure.json");
        let meta = history
            .note_event("claude", "t1", None, &body(failure, home.path(), ID), 2000, None)
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
            .note_event("claude", "t1", None, &body(STOP, home.path(), ID), 2000, None)
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
            .note_event("claude", "t1", None, &body(STOP, home.path(), ID), 2000, None)
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
            .note_event("claude", "t1", None, &body(STOP, home.path(), ID), 3000, None)
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
        history.note_event("claude", "t1", None, &body(START, home.path(), "aaa"), 1000, None);
        history.note_event("claude", "t1", None, &body(START, home.path(), "bbb"), 2000, None);
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

        history.note_event("claude", "ta", Some(a), &body(START, home.path(), ID), 1000, None);
        assert_eq!(listed(), [a]);
        // Resumed in tab B.
        history.note_event("claude", "tb", Some(b), &body(START, home.path(), ID), 2000, None);
        assert_eq!(listed(), [a, b]);
        // A restart in tab B: a new file, with no start or stop event to write the sidecar for it.
        history.note_event("claude", "tb", Some(b2), &body(PRE_TOOL, home.path(), ID), 3000, None);
        assert_eq!(listed(), [a, b, b2]);
        // Nothing new: the sidecar is not rewritten.
        let before = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        history.note_event("claude", "tb", Some(b2), &body(PRE_TOOL, home.path(), ID), 3500, None);
        history.note_event("claude", "tb", None, &body(PRE_TOOL, home.path(), ID), 3600, None);
        assert_eq!(listed(), [a, b, b2]);
        assert_eq!(
            std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap(),
            before
        );
        // A name that is not a recording name is never listed.
        history.note_event(
            "claude",
            "tb",
            Some("..\\x.cast"),
            &body(STOP, home.path(), ID),
            4000,
            None,
        );
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
        history.note_event("claude", "ta", Some(a), &body(START, home.path(), ID), 1000, None);
        assert_eq!(links(), [link(a, 1000, 1000)]);
        history.note_event("claude", "ta", Some(a), &body(STOP, home.path(), ID), 5000, None);
        assert_eq!(links(), [link(a, 1000, 5000)]);
        // Resumed in tab B: A keeps where it ended, and B starts its own.
        history.note_event("claude", "tb", Some(b), &body(START, home.path(), ID), 9000, None);
        assert_eq!(links(), [link(a, 1000, 5000), link(b, 9000, 9000)]);
        history.note_event("claude", "tb", Some(b), &body(STOP, home.path(), ID), 12_000, None);
        assert_eq!(links(), [link(a, 1000, 5000), link(b, 9000, 12_000)]);
        // An event in a recording the sidecar already lists writes nothing.
        let before = std::fs::metadata(sidecar(home.path(), ID)).unwrap().modified().unwrap();
        history.note_event("claude", "tb", Some(b), &body(PRE_TOOL, home.path(), ID), 13_000, None);
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
            history.note_event("claude", "t1", None, &body(PRE_TOOL, home.path(), ID), 1500, None),
            None
        );
        assert_eq!(history.latest_for_tab("t1").unwrap().started_ms, 1500);
        assert!(sidecar(home.path(), ID).exists());
    }

    /// A Stop from a Linux Claude: Linux paths, as the CLI in the distro writes them.
    fn wsl_body(id: &str, transcript: &str, cwd: &str) -> Value {
        let mut v: Value = serde_json::from_str(STOP).unwrap();
        v["session_id"] = json!(id);
        v["transcript_path"] = json!(transcript);
        v["cwd"] = json!(cwd);
        v
    }

    // The transcript lives on the distro's share, which a test can't create, so these check what is recorded and
    // refused (note_event does no file I/O; the copy comes later and reads the stored path).
    #[test]
    fn note_event_translates_a_wsl_transcript_and_cwd() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let linux = format!("/home/u/.claude/projects/-home-u-proj/{ID}.jsonl");
        let meta = history
            .note_event(
                "claude",
                "t1",
                None,
                &wsl_body(ID, &linux, "/home/u/proj"),
                1000,
                Some("Ubuntu"),
            )
            .unwrap();
        let unc = format!(r"\\wsl.localhost\Ubuntu\home\u\.claude\projects\-home-u-proj\{ID}.jsonl");
        assert_eq!(meta.transcript_path, unc);
        assert_eq!(meta.cwd, r"\\wsl.localhost\Ubuntu\home\u\proj");
        assert_eq!(meta.distro.as_deref(), Some("Ubuntu"));
        // The sidecar and the index row carry the translated paths; only the sidecar keeps the distro.
        let stored = crate::history::archive::read_meta(home.path(), "claude", ID)
            .unwrap()
            .unwrap();
        assert_eq!(stored, meta);
        let row = history.latest_for_tab("t1").unwrap();
        assert_eq!((row.transcript_path, row.cwd), (meta.transcript_path, meta.cwd));

        // A folder under /mnt/<drive> is that drive's own folder.
        let mnt = format!("/mnt/c/Users/me/.claude/projects/p/{ID}.jsonl");
        let other = "00000000-0000-4000-8000-000000000002";
        let drive = history
            .note_event(
                "claude",
                "t2",
                None,
                &wsl_body(other, &mnt.replace(ID, other), "/mnt/c/Users/me/proj"),
                2000,
                Some("Ubuntu"),
            )
            .unwrap();
        assert_eq!(drive.cwd, r"C:\Users\me\proj");
        assert!(drive.transcript_path.starts_with(r"C:\Users\me\.claude"));

        // Codex and Antigravity bodies go the same way (their own rules still apply).
        let rollout = "/home/u/.codex/sessions/2026/10/31/rollout-2026-10-31T10-00-00-abc.jsonl";
        let mut codex = wsl_body("00000000-0000-4000-8000-000000000003", rollout, "/home/u/proj");
        codex["hook_event_name"] = json!("SessionStart");
        // It is linked, and asks for no copy (the importer copies rollouts).
        assert_eq!(
            history.note_event("codex", "t3", None, &codex, 3000, Some("Ubuntu")),
            None
        );
        let row = history.latest_for_tab_of("t3", "codex").unwrap();
        assert_eq!(
            row.transcript_path,
            r"\\wsl.localhost\Ubuntu\home\u\.codex\sessions\2026\10\31\rollout-2026-10-31T10-00-00-abc.jsonl"
        );
    }

    #[test]
    fn note_event_records_nothing_for_a_wsl_path_it_cannot_trust() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let good = format!("/home/u/.claude/projects/p/{ID}.jsonl");
        let bad_transcripts = [
            // Won't translate: a `..`, a relative path, no file name for this session.
            format!("/home/u/../../etc/{ID}.jsonl"),
            format!("home/u/.claude/projects/p/{ID}.jsonl"),
            "/home/u/.claude/projects/p/other.jsonl".to_string(),
            // Already Windows paths: only this distro's share is a place for them.
            format!(r"\\wsl.localhost\Debian\home\u\{ID}.jsonl"),
            format!(r"\\server\share\{ID}.jsonl"),
            format!(r"\\wsl.localhost\Ubuntu\..\x\{ID}.jsonl"),
            format!(r"\\?\UNC\wsl.localhost\Ubuntu\home\{ID}.jsonl"),
        ];
        for transcript in &bad_transcripts {
            let body = wsl_body(ID, transcript, "/home/u/proj");
            assert_eq!(
                history.note_event("claude", "t1", None, &body, 1000, Some("Ubuntu")),
                None,
                "{transcript}"
            );
        }
        // A folder that won't translate drops the event too.
        let body = wsl_body(ID, &good, "/home/u/../x");
        assert_eq!(
            history.note_event("claude", "t1", None, &body, 1000, Some("Ubuntu")),
            None
        );
        // A distro that can't sit in a path translates nothing.
        let body = wsl_body(ID, &good, "/home/u/proj");
        assert_eq!(
            history.note_event("claude", "t1", None, &body, 1000, Some(r"a\b")),
            None
        );
        assert_eq!(history.latest_for_tab("t1"), None);
        assert!(!sidecar(home.path(), ID).exists());
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
            assert_eq!(history.note_event("claude", "t1", None, &hostile, 1000, None), None);
        }
        assert_eq!(history.latest_for_tab("t1"), None);
        let archive = paths::archive_dir(home.path(), "claude");
        let files = std::fs::read_dir(archive).map(|d| d.count()).unwrap_or(0);
        assert_eq!(files, 0);
    }

    const CODEX_START: &str = include_str!("../../tests/fixtures/codex/hooks/0.163.0/SessionStart.json");
    const CODEX_END: &str = include_str!("../../tests/fixtures/codex/hooks/0.163.0/SessionEnd.json");

    /// A Codex fixture pointed at a rollout named as Codex names it, under `dir`.
    fn codex_body(fixture: &str, dir: &Path, id: &str) -> Value {
        let mut v: Value = serde_json::from_str(fixture).unwrap();
        v["session_id"] = json!(id);
        v["transcript_path"] = json!(
            dir.join("sessions")
                .join(format!("rollout-2026-10-24T19-31-03-{id}.jsonl"))
                .to_string_lossy()
        );
        v["cwd"] = json!(dir.to_string_lossy());
        v
    }

    #[test]
    fn a_codex_start_in_a_tab_records_the_tab() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let body = codex_body(CODEX_START, home.path(), ID);
        assert_eq!(
            history.note_event("codex", "t1", Some("t1-1000.cast"), &body, 1000, None),
            None
        );
        let row = history.get("codex", ID).unwrap().unwrap();
        assert_eq!((row.tab_id.as_str(), row.client), ("t1", Client::Marshell));
        assert_eq!(row.started_ms, 1000);
        let (_, side) = crate::history::archive::archive_paths(home.path(), "codex", ID);
        let meta = crate::history::archive::read_meta(home.path(), "codex", ID)
            .unwrap()
            .unwrap();
        assert!(side.exists());
        assert_eq!(
            (meta.agent.as_str(), meta.tab_id.as_str(), meta.client),
            ("codex", "t1", Client::Marshell)
        );
        assert_eq!(meta.recordings.len(), 1);
        assert_eq!(history.latest_for_tab("t1").unwrap().session_id, ID);
    }

    #[test]
    fn a_codex_rollout_path_with_the_wrong_name_is_ignored() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let mut body = codex_body(CODEX_START, home.path(), ID);
        for path in [
            home.path().join("sessions").join("notes.txt"),
            home.path().join("sessions").join(format!("{ID}.json")),
        ] {
            body["transcript_path"] = json!(path.to_string_lossy());
            assert_eq!(history.note_event("codex", "t1", None, &body, 1000, None), None);
        }
        body["transcript_path"] = json!("sessions/rollout-x.jsonl");
        assert_eq!(history.note_event("codex", "t1", None, &body, 1000, None), None);
        assert!(history.get("codex", ID).unwrap().is_none());
        assert!(!sidecar(home.path(), ID).exists());
        let (_, side) = crate::history::archive::archive_paths(home.path(), "codex", ID);
        assert!(!side.exists());
    }

    #[test]
    fn a_codex_session_with_only_session_end_is_linked() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        let body = codex_body(CODEX_END, home.path(), ID);
        assert_eq!(history.note_event("codex", "t1", None, &body, 5000, None), None);
        let row = history.get("codex", ID).unwrap().unwrap();
        assert_eq!((row.tab_id.as_str(), row.client), ("t1", Client::Marshell));
        let meta = crate::history::archive::read_meta(home.path(), "codex", ID)
            .unwrap()
            .unwrap();
        assert_eq!(meta.tab_id, "t1");
    }

    #[test]
    fn a_later_codex_event_does_not_rewrite_the_link() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        history.note_event(
            "codex",
            "t1",
            None,
            &codex_body(CODEX_START, home.path(), ID),
            1000,
            None,
        );
        let (_, side) = crate::history::archive::archive_paths(home.path(), "codex", ID);
        let before_bytes = std::fs::read(&side).unwrap();
        let before_time = std::fs::metadata(&side).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            history.note_event("codex", "t1", None, &codex_body(CODEX_END, home.path(), ID), 9000, None),
            None
        );
        assert_eq!(std::fs::read(&side).unwrap(), before_bytes);
        assert_eq!(std::fs::metadata(&side).unwrap().modified().unwrap(), before_time);
        let row = history.get("codex", ID).unwrap().unwrap();
        assert_eq!((row.started_ms, row.updated_ms), (1000, 1000));
    }

    const AGY_PRE: &str = include_str!("../../tests/fixtures/antigravity/hooks/1.4.7/PreInvocation.json");
    const AGY_START: &str = include_str!("../../tests/fixtures/antigravity/hooks/1.4.7/SessionStart.json");
    const AGY_ID_2: &str = "00000000-0000-4000-8000-000000000002";

    /// agy's hook body as the bridge posts it: the fixture plus the event name from `--event`, with the
    /// transcript where agy keeps it under `dir`.
    fn agy_body(fixture: &str, event: &str, dir: &Path, id: &str) -> Value {
        let mut v: Value = serde_json::from_str(fixture).unwrap();
        v["hook_event_name"] = json!(event);
        v["conversationId"] = json!(id);
        v["transcriptPath"] = json!(
            dir.join("antigravity-cli")
                .join("brain")
                .join(id)
                .join(".system_generated")
                .join("logs")
                .join("transcript_full.jsonl")
                .to_string_lossy()
        );
        v
    }

    /// A history whose registry has tab `t1`, started in `cwd`.
    fn agy_history(home: &Path, cwd: &str) -> History {
        let registry = Arc::new(Registry::new());
        registry.reserve(
            "t1",
            crate::registry::Relaunch {
                profile_id: "pwsh".into(),
                agent: Some("antigravity".into()),
                cwd: cwd.into(),
                mode: None,
                name: "agy".into(),
                preset: None,
            },
        );
        History::open(home).with_registry(registry)
    }

    #[test]
    fn an_antigravity_event_links_its_conversation_to_the_tab() {
        let home = tempfile::tempdir().unwrap();
        let history = agy_history(home.path(), "C:/work/app");
        let body = agy_body(AGY_PRE, "PreInvocation", home.path(), ID);
        assert_eq!(
            history.note_event("antigravity", "t1", Some("t1-1000.cast"), &body, 1000, None),
            None
        );
        let row = history.get("antigravity", ID).unwrap().unwrap();
        // The folder is the tab's: the fixture's workspacePaths is empty.
        assert_eq!(
            (row.tab_id.as_str(), row.cwd.as_str(), row.client),
            ("t1", "C:/work/app", Client::Marshell)
        );
        assert_eq!(row.transcript_path, body["transcriptPath"].as_str().unwrap());
        assert_eq!((row.started_ms, row.updated_ms), (1000, 1000));
        assert_eq!(history.latest_for_tab_of("t1", "antigravity").unwrap().session_id, ID);
        // Claude's lookup doesn't see it.
        assert_eq!(history.latest_for_tab_of("t1", "claude"), None);
        let meta = crate::history::archive::read_meta(home.path(), "antigravity", ID)
            .unwrap()
            .unwrap();
        assert_eq!((meta.agent.as_str(), meta.tab_id.as_str()), ("antigravity", "t1"));
        assert_eq!(meta.recordings.len(), 1);
        // Any event that names the conversation links it, `SessionStart` too.
        let other = agy_body(AGY_START, "SessionStart", home.path(), AGY_ID_2);
        assert_eq!(history.note_event("antigravity", "t1", None, &other, 2000, None), None);
        assert!(history.get("antigravity", AGY_ID_2).unwrap().is_some());
    }

    #[test]
    fn a_later_conversation_in_the_tab_replaces_the_earlier() {
        let home = tempfile::tempdir().unwrap();
        let history = agy_history(home.path(), "C:/work/app");
        let first = agy_body(AGY_PRE, "PreInvocation", home.path(), ID);
        let second = agy_body(AGY_PRE, "PreInvocation", home.path(), AGY_ID_2);
        history.note_event("antigravity", "t1", None, &first, 1000, None);
        history.note_event("antigravity", "t1", None, &second, 2000, None);
        let latest = history.latest_for_tab_of("t1", "antigravity").unwrap();
        assert_eq!(latest.session_id, AGY_ID_2);
        assert_eq!(latest.previous_in_tab.as_deref(), Some(ID));
        // Another event of the first one in the same tab makes it the tab's latest again.
        history.note_event("antigravity", "t1", None, &first, 3000, None);
        assert_eq!(history.latest_for_tab_of("t1", "antigravity").unwrap().session_id, ID);
    }

    #[test]
    fn an_antigravity_event_of_a_tab_the_registry_does_not_know_is_ignored() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path()).with_registry(Arc::new(Registry::new()));
        let body = agy_body(AGY_PRE, "PreInvocation", home.path(), ID);
        assert_eq!(history.note_event("antigravity", "t1", None, &body, 1000, None), None);
        assert!(history.get("antigravity", ID).unwrap().is_none());
    }

    #[test]
    fn an_antigravity_event_with_a_hostile_path_or_id_is_ignored() {
        let home = tempfile::tempdir().unwrap();
        let history = agy_history(home.path(), "C:/work/app");
        let mut outside = agy_body(AGY_PRE, "PreInvocation", home.path(), ID);
        outside["transcriptPath"] = json!(home.path().join("notes.jsonl").to_string_lossy());
        let bad_id = agy_body(AGY_PRE, "PreInvocation", home.path(), "../x");
        for hostile in [outside, bad_id] {
            assert_eq!(
                history.note_event("antigravity", "t1", None, &hostile, 1000, None),
                None
            );
        }
        assert_eq!(history.latest_for_tab_of("t1", "antigravity"), None);
    }

    /// Every file under `dir`, read as bytes, holds none of `needle`.
    fn assert_no_file_holds(dir: &Path, needle: &str) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                assert_no_file_holds(&path, needle);
            } else if let Ok(bytes) = std::fs::read(&path) {
                let hit = bytes.windows(needle.len()).any(|w| w == needle.as_bytes());
                assert!(!hit, "{} holds the prompt text", path.display());
            }
        }
    }

    #[test]
    fn the_last_user_input_is_not_kept() {
        let home = tempfile::tempdir().unwrap();
        let history = agy_history(home.path(), "C:/work/app");
        let body = agy_body(AGY_PRE, "PreInvocation", home.path(), ID);
        let typed = body["lastUserInput"].as_str().unwrap().to_string();
        assert!(!typed.is_empty());
        history.note_event("antigravity", "t1", None, &body, 1000, None);
        let row = history.get("antigravity", ID).unwrap().unwrap();
        assert!(!format!("{row:?}").contains(&typed));
        assert_no_file_holds(home.path(), &typed);
    }

    #[test]
    fn history_lists_no_antigravity_rows() {
        let home = tempfile::tempdir().unwrap();
        let history = agy_history(home.path(), "C:/work/app");
        history.note_event(
            "antigravity",
            "t1",
            None,
            &agy_body(AGY_PRE, "PreInvocation", home.path(), ID),
            1000,
            None,
        );
        assert!(history.get("antigravity", ID).unwrap().is_some());
        for agent in [None, Some("antigravity".to_string())] {
            let query = HistoryQuery {
                agent,
                headless: Some(true),
                archived: Some(true),
                ..Default::default()
            };
            assert!(history.list(&query).unwrap().is_empty());
            assert!(history.bookmarks(&query).unwrap().is_empty());
        }
    }

    const TOOLS: &str = include_str!("../../tests/fixtures/claude/transcripts/2.1.309/tools.jsonl");

    /// A transcript at `<dir>/<id>.jsonl` and the Stop event that asks for its copy.
    fn stopped(history: &History, dir: &Path, id: &str, text: &str, at_ms: u64) -> SessionMeta {
        std::fs::write(dir.join(format!("{id}.jsonl")), text).unwrap();
        history
            .note_event("claude", "t1", None, &body(STOP, dir, id), at_ms, None)
            .unwrap()
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
        assert!(history.search("typo", false, false).unwrap().is_empty());

        schedule_copy(history.clone(), meta);
        idle(&history).await;

        let hits = history.search("typo", false, false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, ID);
        assert!(hits[0].snippet.contains("\u{2}typo\u{3}"));
        let listed = history.list(&HistoryQuery::default()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].turns, listed[0].branch.as_deref()), (1, Some("main")));
        assert!(!listed[0].limited);
    }

    #[test]
    fn a_hook_copy_asked_for_during_an_import_still_runs() {
        let home = tempfile::tempdir().unwrap();
        let history = History::open(home.path());
        history.set_settle(Duration::from_millis(20));
        let mut hook = stopped(&history, home.path(), ID, TOOLS, 2000);
        hook.recordings.push(RecordingLink {
            file: "t1-1.cast".into(),
            first_ms: 1,
            last_ms: 2,
        });
        // An import holds the session, and a hook asks for a copy meanwhile: schedule_copy leaves it waiting.
        history.copying.lock().unwrap().insert(ID.into(), Some(hook));
        history.finish_copying(ID);
        assert!(history.is_idle());
        let meta = crate::history::archive::read_meta(home.path(), "claude", ID)
            .unwrap()
            .unwrap();
        assert_eq!(meta.recordings.len(), 1, "the waiting hook copy was dropped");
        assert_eq!(history.search("typo", false, false).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn backfill_indexes_what_was_archived_before_and_skips_what_is_current() {
        let home = tempfile::tempdir().unwrap();
        {
            let history = History::open(home.path());
            let meta = stopped(&history, home.path(), ID, TOOLS, 2000);
            // Archived, but never indexed: as after an upgrade from schema 1.
            crate::history::archive::copy_transcript(home.path(), &meta).unwrap();
            assert!(history.search("typo", false, false).unwrap().is_empty());
        }
        let history = Arc::new(History::open(home.path()));
        history.clone().backfill().await.unwrap();
        assert_eq!(history.search("typo", false, false).unwrap().len(), 1);
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
        assert!(history.search("typo", false, false).unwrap().is_empty());
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
