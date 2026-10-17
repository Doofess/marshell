//! The core's list of sessions: numbers, names, order and recently closed.
//! The UI never invents any of these; it reads them from `snapshot()` or
//! follows `subscribe()`.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::pty::Mark;
use marshell_protocol::api::{CommandState, SavedTab, SessionInfo, SessionMode, SessionState, SessionsSnapshot};
use tokio::sync::watch;

/// How many closed sessions we remember for Reopen.
const MAX_CLOSED: usize = 10;

/// What we need to start the same session again.
#[derive(Debug, Clone)]
pub struct Relaunch {
    pub profile_id: String,
    pub agent: Option<String>,
    pub cwd: String,
    /// The mode it started in. None for a plain shell.
    pub mode: Option<SessionMode>,
    /// The base name, as given (a renamed name when the user renamed it).
    pub name: String,
}

/// Why `begin_resume` said no.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeRefusal {
    NotFound,
    AlreadyRunning,
}

pub struct Entry {
    pub info: SessionInfo,
    /// How to reopen this session.
    pub spec: Relaunch,
    /// Which spawn of this tab is current. A process reports its exit with the generation it
    /// started with, so an old process (killed by a restart) cannot mark the new one Exited.
    pub generation: u64,
}

struct State {
    open: Vec<Entry>,
    /// The last generation handed out.
    last_generation: u64,
    /// Newest first.
    closed: VecDeque<Entry>,
}

impl State {
    fn snapshot(&self) -> SessionsSnapshot {
        SessionsSnapshot {
            sessions: self.open.iter().map(|e| e.info.clone()).collect(),
            closed: self.closed.iter().map(|e| e.info.clone()).collect(),
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.last_generation += 1;
        self.last_generation
    }

    fn lowest_free_number(&self) -> u32 {
        let mut number = 1;
        while self.open.iter().any(|e| e.info.number == number) {
            number += 1;
        }
        number
    }
}

pub struct Registry {
    state: Mutex<State>,
    tx: watch::Sender<SessionsSnapshot>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let state = State {
            open: Vec::new(),
            last_generation: 0,
            closed: VecDeque::new(),
        };
        let (tx, _rx) = watch::channel(state.snapshot());
        Self {
            state: Mutex::new(state),
            tx,
        }
    }

    /// Adds a Running row at the end and returns its number (lowest free) and the generation of
    /// the process about to start. If another open session already has exactly this name, the
    /// shown name becomes "<name> <number>". The stored `Relaunch` keeps the name.
    pub fn reserve(&self, tab_id: &str, relaunch: Relaunch) -> (u32, u64) {
        let generation = self.state.lock().unwrap().next_generation();
        (self.adopt(tab_id, relaunch, generation), generation)
    }

    /// Adds a Running row at the end for a process that is already running (the shell panel's),
    /// whose exit callback was built with `generation`, and returns its number. Same naming rule
    /// as `reserve`.
    pub fn adopt(&self, tab_id: &str, relaunch: Relaunch, generation: u64) -> u32 {
        let mut state = self.state.lock().unwrap();
        let number = state.lowest_free_number();
        let taken = state.open.iter().any(|e| e.info.name == relaunch.name);
        let name = if taken {
            format!("{} {}", relaunch.name, number)
        } else {
            relaunch.name.clone()
        };
        state.open.push(Entry {
            info: SessionInfo {
                tab_id: tab_id.to_string(),
                number,
                name,
                agent: relaunch.agent.clone(),
                profile_id: relaunch.profile_id.clone(),
                cwd: relaunch.cwd.clone(),
                project: crate::projects::project_of(&relaunch.cwd),
                mode: relaunch.mode,
                state: SessionState::Running,
                command: CommandState::Unknown,
                paused: false,
            },
            spec: relaunch,
            generation,
        });
        self.tx.send_replace(state.snapshot());
        number
    }

    /// Adds the saved tabs as Ended rows, in the given order, with their ids and numbers.
    /// Nothing is started. If two tabs share a number, the later one gets the lowest free number.
    /// `relaunch_of` says how each tab would be started again.
    pub fn seed_ended(&self, tabs: &[SavedTab], relaunch_of: impl Fn(&SavedTab) -> Relaunch) {
        let mut state = self.state.lock().unwrap();
        for tab in tabs {
            let generation = state.next_generation();
            let number = if state.open.iter().any(|e| e.info.number == tab.number) {
                state.lowest_free_number()
            } else {
                tab.number
            };
            state.open.push(Entry {
                info: SessionInfo {
                    tab_id: tab.tab_id.clone(),
                    number,
                    name: tab.name.clone(),
                    agent: tab.agent.clone(),
                    profile_id: tab.profile_id.clone(),
                    cwd: tab.cwd.clone(),
                    project: crate::projects::project_of(&tab.cwd),
                    mode: tab.mode,
                    state: SessionState::Ended,
                    command: CommandState::Unknown,
                    paused: false,
                },
                spec: relaunch_of(tab),
                generation,
            });
        }
        self.tx.send_replace(state.snapshot());
    }

    /// The open rows in tab order, whatever their state: what layout.json keeps.
    pub fn saved_tabs(&self) -> Vec<SavedTab> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .map(|e| SavedTab {
                tab_id: e.info.tab_id.clone(),
                number: e.info.number,
                name: e.info.name.clone(),
                agent: e.info.agent.clone(),
                profile_id: e.info.profile_id.clone(),
                cwd: e.info.cwd.clone(),
                mode: e.info.mode,
            })
            .collect()
    }

    /// Starts a Resume: an Ended or Exited tab becomes Running, in the same row (same id, number,
    /// name and place). The check and the change happen under one lock, so of two calls at once
    /// only one succeeds. Returns how to start it, the state it had (for `set_state` if the start
    /// fails) and the generation of the process to start.
    pub fn begin_resume(&self, tab_id: &str) -> Result<(Relaunch, SessionState, u64), ResumeRefusal> {
        let mut state = self.state.lock().unwrap();
        let generation = state.next_generation();
        let entry = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id)
            .ok_or(ResumeRefusal::NotFound)?;
        if entry.info.state == SessionState::Running {
            return Err(ResumeRefusal::AlreadyRunning);
        }
        let previous = std::mem::replace(&mut entry.info.state, SessionState::Running);
        entry.info.command = CommandState::Unknown;
        entry.info.paused = false;
        entry.generation = generation;
        let relaunch = entry.spec.clone();
        self.tx.send_replace(state.snapshot());
        Ok((relaunch, previous, generation))
    }

    /// Starts a Restart: the tab, in any state, becomes Running in the same row with the new mode
    /// (kept for Resume and layout.json too). The generation moves on, so the old process's exit
    /// no longer counts. Returns how to start it and the new generation. The caller has already
    /// checked that the restart can happen, and kills the old process after this.
    pub fn begin_restart(&self, tab_id: &str, mode: SessionMode) -> Result<(Relaunch, u64), ResumeRefusal> {
        let mut state = self.state.lock().unwrap();
        let generation = state.next_generation();
        let entry = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id)
            .ok_or(ResumeRefusal::NotFound)?;
        entry.info.mode = Some(mode);
        entry.spec.mode = Some(mode);
        entry.info.state = SessionState::Running;
        entry.info.command = CommandState::Unknown;
        entry.info.paused = false;
        entry.generation = generation;
        let relaunch = entry.spec.clone();
        self.tx.send_replace(state.snapshot());
        Ok((relaunch, generation))
    }

    /// Puts a tab's state back, when a Resume could not start its process.
    pub fn set_state(&self, tab_id: &str, new_state: SessionState) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) {
            entry.info.state = new_state;
            self.tx.send_replace(state.snapshot());
        }
    }

    /// A spawn failed: drop the row and free its number. Not added to closed.
    pub fn release(&self, tab_id: &str) {
        let mut state = self.state.lock().unwrap();
        let before = state.open.len();
        state.open.retain(|e| e.info.tab_id != tab_id);
        if state.open.len() != before {
            self.tx.send_replace(state.snapshot());
        }
    }

    /// A process exited. Ignored when `generation` is not the entry's current one: that process
    /// was replaced by a Restart or Resume, and the row belongs to the new one.
    pub fn mark_exited(&self, tab_id: &str, generation: u64, code: i32) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id && e.generation == generation)
        {
            entry.info.state = SessionState::Exited { code };
            entry.info.paused = false;
            self.tx.send_replace(state.snapshot());
        }
    }

    /// The generation of the tab's current process, if the tab is open.
    pub fn generation(&self, tab_id: &str) -> Option<u64> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .find(|e| e.info.tab_id == tab_id)
            .map(|e| e.generation)
    }

    /// The process was frozen or thawed. Ignored when `generation` is not the entry's current one:
    /// the row belongs to a newer process. A row that is not Running is never paused. Broadcasts
    /// only on a change.
    pub fn set_paused(&self, tab_id: &str, generation: u64, paused: bool) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id && e.generation == generation)
            && entry.info.paused != paused
            && (!paused || entry.info.state == SessionState::Running)
        {
            entry.info.paused = paused;
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Folds a shell-integration mark into the tab's command state. Marks for
    /// an exited or unknown tab are ignored. Broadcasts only on a change.
    pub fn mark(&self, tab_id: &str, mark: Mark) {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) else {
            return;
        };
        if entry.info.state != SessionState::Running {
            return;
        }
        let last_exit = match entry.info.command {
            CommandState::Prompt { last_exit } => last_exit,
            _ => None,
        };
        let next = match mark {
            Mark::Executed => CommandState::Running,
            Mark::Finished(Some(code)) => CommandState::Prompt { last_exit: Some(code) },
            Mark::Finished(None) | Mark::PromptStart => CommandState::Prompt { last_exit },
            Mark::CommandStart | Mark::CommandLine(_) => return,
        };
        if entry.info.command != next {
            entry.info.command = next;
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Trims the name; an empty name or an unknown tab returns false.
    pub fn rename(&self, tab_id: &str, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) else {
            return false;
        };
        // Keep the relaunch name too, so a reopen keeps the renamed name.
        entry.info.name = name.to_string();
        entry.spec.name = name.to_string();
        self.tx.send_replace(state.snapshot());
        true
    }

    /// `tab_ids` must hold every open id exactly once. Numbers do not change.
    pub fn reorder(&self, tab_ids: &[String]) -> bool {
        let mut state = self.state.lock().unwrap();
        // Same length, every id open, no repeats: that is a permutation.
        let is_permutation = tab_ids.len() == state.open.len()
            && tab_ids.iter().all(|id| state.open.iter().any(|e| &e.info.tab_id == id))
            && tab_ids.iter().enumerate().all(|(i, id)| !tab_ids[..i].contains(id));
        if !is_permutation {
            return false;
        }
        state
            .open
            .sort_by_key(|e| tab_ids.iter().position(|id| *id == e.info.tab_id));
        self.tx.send_replace(state.snapshot());
        true
    }

    /// Moves the session to the closed list (newest first, max 10) and
    /// frees its number. Unknown id returns None and changes nothing.
    pub fn close(&self, tab_id: &str) -> Option<Relaunch> {
        let mut state = self.state.lock().unwrap();
        let index = state.open.iter().position(|e| e.info.tab_id == tab_id)?;
        let mut entry = state.open.remove(index);
        // Closing kills the process, which thaws it first.
        entry.info.paused = false;
        let spec = entry.spec.clone();
        state.closed.push_front(entry);
        state.closed.truncate(MAX_CLOSED);
        self.tx.send_replace(state.snapshot());
        Some(spec)
    }

    /// The open rows (any state) whose cwd `in_folder` accepts.
    pub fn sessions_where(&self, in_folder: impl Fn(&str) -> bool) -> Vec<SessionInfo> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .filter(|e| in_folder(&e.info.cwd))
            .map(|e| e.info.clone())
            .collect()
    }

    /// Drops every row whose cwd `in_folder` accepts, Ended and Exited ones too, and the closed
    /// entries (they could only reopen into a folder that is going away). Returns the tab ids of
    /// the open rows dropped. Not added to the closed list.
    pub fn drop_where(&self, in_folder: impl Fn(&str) -> bool) -> Vec<String> {
        let mut state = self.state.lock().unwrap();
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut state.open)
            .into_iter()
            .partition(|e| in_folder(&e.info.cwd));
        state.open = kept;
        let closed_before = state.closed.len();
        state.closed.retain(|e| !in_folder(&e.info.cwd));
        if !gone.is_empty() || state.closed.len() != closed_before {
            self.tx.send_replace(state.snapshot());
        }
        gone.into_iter().map(|e| e.info.tab_id).collect()
    }

    /// True while the tab is in the open list.
    pub fn is_open(&self, tab_id: &str) -> bool {
        let state = self.state.lock().unwrap();
        state.open.iter().any(|e| e.info.tab_id == tab_id)
    }

    /// Looks at a closed entry without removing it: its tab id and how to relaunch it.
    /// None means the most recent. Reopen peeks first, so a failed start loses nothing.
    pub fn peek_closed(&self, tab_id: Option<&str>) -> Option<(String, Relaunch)> {
        let state = self.state.lock().unwrap();
        let entry = match tab_id {
            Some(id) => state.closed.iter().find(|e| e.info.tab_id == id)?,
            None => state.closed.front()?,
        };
        Some((entry.info.tab_id.clone(), entry.spec.clone()))
    }

    /// Removes a closed entry for Reopen. None means the most recent.
    pub fn take_closed(&self, tab_id: Option<&str>) -> Option<Relaunch> {
        let mut state = self.state.lock().unwrap();
        let index = match tab_id {
            Some(id) => state.closed.iter().position(|e| e.info.tab_id == id)?,
            None if state.closed.is_empty() => return None,
            None => 0,
        };
        let entry = state.closed.remove(index)?;
        self.tx.send_replace(state.snapshot());
        Some(entry.spec)
    }

    pub fn snapshot(&self) -> SessionsSnapshot {
        self.state.lock().unwrap().snapshot()
    }

    pub fn subscribe(&self) -> watch::Receiver<SessionsSnapshot> {
        self.tx.subscribe()
    }

    /// Core shutdown: forget everything.
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.open.clear();
        state.closed.clear();
        self.tx.send_replace(state.snapshot());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relaunch(name: &str) -> Relaunch {
        Relaunch {
            profile_id: "pwsh".into(),
            agent: None,
            cwd: "C:/work".into(),
            mode: None,
            name: name.into(),
        }
    }

    #[test]
    fn drop_where_removes_open_and_closed_rows_in_the_folder() {
        let reg = Registry::new();
        let at = |name: &str, cwd: &str| Relaunch {
            cwd: cwd.into(),
            ..relaunch(name)
        };
        reg.reserve("a", at("a", "/in"));
        reg.reserve("b", at("b", "/out"));
        reg.reserve("c", at("c", "/in"));
        reg.reserve("d", at("d", "/in"));
        reg.close("d");
        reg.mark_exited("c", generation_of(&reg, "c"), 0);

        assert_eq!(reg.sessions_where(|cwd| cwd == "/in").len(), 2);
        assert_eq!(
            reg.drop_where(|cwd| cwd == "/in"),
            vec!["a".to_string(), "c".to_string()]
        );
        let snapshot = reg.snapshot();
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].tab_id, "b");
        assert!(snapshot.closed.is_empty());
    }

    /// The generation the entry's current process was started with.
    fn generation_of(reg: &Registry, tab_id: &str) -> u64 {
        let state = reg.state.lock().unwrap();
        state.open.iter().find(|e| e.info.tab_id == tab_id).unwrap().generation
    }

    fn numbers(reg: &Registry) -> Vec<(String, u32)> {
        reg.snapshot()
            .sessions
            .iter()
            .map(|s| (s.tab_id.clone(), s.number))
            .collect()
    }

    #[test]
    fn begin_resume_refuses_running() {
        let reg = Registry::new();
        reg.reserve("t1", relaunch("Shell"));
        assert_eq!(reg.begin_resume("t1").unwrap_err(), ResumeRefusal::AlreadyRunning);
        assert_eq!(reg.begin_resume("nope").unwrap_err(), ResumeRefusal::NotFound);

        reg.mark_exited("t1", generation_of(&reg, "t1"), 2);
        let (spec, previous, _) = reg.begin_resume("t1").unwrap();
        assert_eq!(spec.name, "Shell");
        assert_eq!(previous, SessionState::Exited { code: 2 });
        assert_eq!(reg.snapshot().sessions[0].state, SessionState::Running);
        // The second call finds it Running.
        assert_eq!(reg.begin_resume("t1").unwrap_err(), ResumeRefusal::AlreadyRunning);

        reg.set_state("t1", previous);
        assert_eq!(reg.snapshot().sessions[0].state, SessionState::Exited { code: 2 });
    }

    #[test]
    fn stale_exit_is_ignored() {
        let reg = Registry::new();
        let (_, first) = reg.reserve(
            "t1",
            Relaunch {
                agent: Some("claude".into()),
                mode: Some(SessionMode::Manual),
                ..relaunch("Claude")
            },
        );
        let (spec, second) = reg.begin_restart("t1", SessionMode::Plan).unwrap();
        assert_ne!(first, second);
        assert_eq!(spec.mode, Some(SessionMode::Plan));
        // The killed process reports its exit after the restart: the new run is not Exited.
        reg.mark_exited("t1", first, 0);
        let row = reg.snapshot().sessions.remove(0);
        assert_eq!(row.state, SessionState::Running);
        assert_eq!(row.mode, Some(SessionMode::Plan));
        reg.mark_exited("t1", second, 3);
        assert_eq!(reg.snapshot().sessions[0].state, SessionState::Exited { code: 3 });
    }

    #[test]
    fn set_paused_publishes() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.set_paused("a", generation, true);
        assert!(rx.has_changed().unwrap());
        assert!(reg.snapshot().sessions[0].paused);
        reg.set_paused("a", generation, false);
        assert!(!reg.snapshot().sessions[0].paused);
    }

    #[test]
    fn set_paused_ignores_a_stale_generation() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        reg.set_paused("a", generation + 1, true);
        assert!(!reg.snapshot().sessions[0].paused);
        reg.set_paused("nope", generation, true);
    }

    #[test]
    fn exit_clears_paused() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        reg.set_paused("a", generation, true);
        reg.mark_exited("a", generation, 1);
        let row = &reg.snapshot().sessions[0];
        assert_eq!(row.state, SessionState::Exited { code: 1 });
        assert!(!row.paused);
    }

    #[test]
    fn restart_clears_paused() {
        let reg = Registry::new();
        reg.reserve(
            "a",
            Relaunch {
                agent: Some("claude".into()),
                ..relaunch("A")
            },
        );
        reg.set_paused("a", generation_of(&reg, "a"), true);
        reg.begin_restart("a", SessionMode::Plan).unwrap();
        assert!(!reg.snapshot().sessions[0].paused);
    }

    #[test]
    fn begin_restart_unknown_tab_is_not_found() {
        let reg = Registry::new();
        assert_eq!(
            reg.begin_restart("nope", SessionMode::Plan).unwrap_err(),
            ResumeRefusal::NotFound
        );
    }

    #[test]
    fn begin_restart_keeps_the_row_and_saves_the_mode() {
        let reg = Registry::new();
        reg.reserve(
            "a",
            Relaunch {
                agent: Some("claude".into()),
                ..relaunch("A")
            },
        );
        reg.reserve("b", relaunch("B"));
        reg.mark("b", Mark::Executed);
        let mut rx = reg.subscribe();
        reg.mark_exited("a", generation_of(&reg, "a"), 1);
        rx.borrow_and_update();
        let (spec, _) = reg.begin_restart("a", SessionMode::Bypass).unwrap();
        assert!(rx.has_changed().unwrap());
        assert_eq!(spec.mode, Some(SessionMode::Bypass));
        let snap = reg.snapshot();
        assert_eq!(snap.sessions[0].tab_id, "a");
        assert_eq!(snap.sessions[0].number, 1);
        assert_eq!(snap.sessions[0].state, SessionState::Running);
        assert_eq!(snap.sessions[0].command, CommandState::Unknown);
        assert_eq!(snap.sessions[0].mode, Some(SessionMode::Bypass));
        // The saved layout and a later Resume see the new mode too.
        assert_eq!(reg.saved_tabs()[0].mode, Some(SessionMode::Bypass));
        assert_eq!(snap.sessions[1].command, CommandState::Running);
    }

    #[test]
    fn numbers_are_lowest_free() {
        let reg = Registry::new();
        for i in 1..=6 {
            assert_eq!(reg.reserve(&format!("t{i}"), relaunch("Shell")).0, i);
        }
        reg.close("t4");
        assert_eq!(reg.reserve("t7", relaunch("Shell")).0, 4);
    }

    #[test]
    fn reorder_never_renumbers() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.reserve("b", relaunch("B"));
        reg.reserve("c", relaunch("C"));
        assert!(reg.reorder(&["c".into(), "a".into(), "b".into()]));
        assert_eq!(numbers(&reg), vec![("c".into(), 3), ("a".into(), 1), ("b".into(), 2)]);
    }

    #[test]
    fn reorder_rejects_non_permutation() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.reserve("b", relaunch("B"));
        let before = numbers(&reg);
        assert!(!reg.reorder(&["a".into()]));
        assert!(!reg.reorder(&["a".into(), "b".into(), "c".into()]));
        assert!(!reg.reorder(&["a".into(), "a".into()]));
        assert_eq!(numbers(&reg), before);
    }

    #[test]
    fn registry_marks_exit_and_keeps_number() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.reserve("b", relaunch("B"));
        reg.mark_exited("b", generation_of(&reg, "b"), 3);
        let snap = reg.snapshot();
        assert_eq!(snap.sessions[1].state, SessionState::Exited { code: 3 });
        assert_eq!(snap.sessions[1].number, 2);
        // The number stays taken while the exited row is open.
        assert_eq!(reg.reserve("c", relaunch("C")).0, 3);
    }

    #[test]
    fn release_frees_number_without_closed_entry() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.release("a");
        let snap = reg.snapshot();
        assert!(snap.sessions.is_empty() && snap.closed.is_empty());
        assert_eq!(reg.reserve("b", relaunch("B")).0, 1);
    }

    #[test]
    fn closed_list_caps_at_ten_newest_first() {
        let reg = Registry::new();
        for i in 1..=12 {
            let id = format!("t{i}");
            reg.reserve(&id, relaunch(&format!("S{i}")));
            reg.close(&id);
        }
        let snap = reg.snapshot();
        assert_eq!(snap.closed.len(), 10);
        assert_eq!(snap.closed[0].tab_id, "t12");
        assert_eq!(snap.closed[9].tab_id, "t3");
    }

    #[test]
    fn rename_trims_and_rejects_empty() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        assert!(reg.rename("a", "  api  "));
        assert_eq!(reg.snapshot().sessions[0].name, "api");
        assert!(!reg.rename("a", "   "));
        assert!(!reg.rename("missing", "x"));
        assert_eq!(reg.snapshot().sessions[0].name, "api");
    }

    #[test]
    fn subscribe_sees_every_change() {
        let reg = Registry::new();
        let mut rx = reg.subscribe();
        assert!(!rx.has_changed().unwrap());
        reg.reserve("a", relaunch("A"));
        assert!(rx.has_changed().unwrap());
        assert_eq!(rx.borrow_and_update().sessions.len(), 1);
        reg.mark_exited("a", generation_of(&reg, "a"), 0);
        assert!(rx.has_changed().unwrap());
    }

    fn command(reg: &Registry) -> CommandState {
        reg.snapshot().sessions[0].command.clone()
    }

    #[test]
    fn mark_folds_into_command_state() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        assert_eq!(command(&reg), CommandState::Unknown);
        reg.mark("a", Mark::Finished(None));
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: None });
        reg.mark("a", Mark::PromptStart);
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: None });
        reg.mark("a", Mark::CommandStart);
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: None });
        reg.mark("a", Mark::Executed);
        assert_eq!(command(&reg), CommandState::Running);
        reg.mark("a", Mark::Finished(Some(2)));
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(2) });
        reg.mark("a", Mark::PromptStart);
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(2) });
        // An empty Enter (D without a code) keeps the last code.
        reg.mark("a", Mark::Finished(None));
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(2) });
        reg.mark("a", Mark::Executed);
        reg.mark("a", Mark::Finished(Some(0)));
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(0) });
    }

    #[test]
    fn marks_after_exit_are_ignored() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed);
        reg.mark_exited("a", generation_of(&reg, "a"), 1);
        reg.mark("a", Mark::Finished(Some(1)));
        assert_eq!(command(&reg), CommandState::Running);
        reg.mark("missing", Mark::PromptStart);
    }

    #[test]
    fn unchanged_state_does_not_broadcast() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        reg.mark("a", Mark::PromptStart);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        reg.mark("a", Mark::PromptStart);
        reg.mark("a", Mark::CommandStart);
        reg.mark("a", Mark::Finished(None));
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn duplicate_name_gets_number_suffix() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("Claude Code"));
        reg.reserve("b", relaunch("Claude Code"));
        reg.reserve("c", relaunch("Codex"));
        let names: Vec<String> = reg.snapshot().sessions.into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["Claude Code", "Claude Code 2", "Codex"]);
    }

    #[test]
    fn close_then_take_closed_reopens_with_stored_name() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("Claude Code"));
        reg.rename("a", "api");
        let spec = reg.close("a").unwrap();
        assert_eq!(spec.name, "api");
        assert!(reg.close("a").is_none());
        let again = reg.take_closed(None).unwrap();
        assert_eq!(again.name, "api");
        assert!(reg.snapshot().closed.is_empty());
        assert!(reg.take_closed(None).is_none());
    }

    #[test]
    fn peek_closed_leaves_the_entry_in_place() {
        let reg = Registry::new();
        for id in ["a", "b"] {
            reg.reserve(id, relaunch(id));
            reg.close(id);
        }
        let (id, spec) = reg.peek_closed(None).unwrap();
        assert_eq!((id.as_str(), spec.name.as_str()), ("b", "b"));
        let (id, _) = reg.peek_closed(Some("a")).unwrap();
        assert_eq!(id, "a");
        assert!(reg.peek_closed(Some("zzz")).is_none());
        assert_eq!(reg.snapshot().closed.len(), 2);
    }

    #[test]
    fn is_open_follows_reserve_close_and_release() {
        let reg = Registry::new();
        assert!(!reg.is_open("a"));
        reg.reserve("a", relaunch("a"));
        assert!(reg.is_open("a"));
        reg.close("a");
        assert!(!reg.is_open("a"));
        reg.reserve("b", relaunch("b"));
        reg.release("b");
        assert!(!reg.is_open("b"));
    }

    #[test]
    fn take_closed_by_id_and_clear() {
        let reg = Registry::new();
        for id in ["a", "b", "c"] {
            reg.reserve(id, relaunch(id));
            reg.close(id);
        }
        assert_eq!(reg.take_closed(Some("b")).unwrap().name, "b");
        assert!(reg.take_closed(Some("b")).is_none());
        let ids: Vec<String> = reg.snapshot().closed.into_iter().map(|s| s.tab_id).collect();
        assert_eq!(ids, vec!["c", "a"]);
        reg.reserve("d", relaunch("d"));
        reg.clear();
        let snap = reg.snapshot();
        assert!(snap.sessions.is_empty() && snap.closed.is_empty());
    }

    fn saved(id: &str, number: u32, name: &str) -> SavedTab {
        SavedTab {
            tab_id: id.into(),
            number,
            name: name.into(),
            agent: Some("claude".into()),
            profile_id: "pwsh".into(),
            cwd: "C:/work".into(),
            mode: None,
        }
    }

    fn seed(reg: &Registry, tabs: &[SavedTab]) {
        reg.seed_ended(tabs, |t| Relaunch {
            profile_id: t.profile_id.clone(),
            agent: t.agent.clone(),
            cwd: t.cwd.clone(),
            mode: t.mode,
            name: t.name.clone(),
        });
    }

    #[test]
    fn snapshot_carries_mode() {
        let reg = Registry::new();
        reg.reserve(
            "t1",
            Relaunch {
                agent: Some("claude".into()),
                mode: Some(SessionMode::Plan),
                ..relaunch("Claude")
            },
        );
        assert_eq!(reg.snapshot().sessions[0].mode, Some(SessionMode::Plan));
        assert_eq!(reg.saved_tabs()[0].mode, Some(SessionMode::Plan));
    }

    #[test]
    fn seed_ended_keeps_order_ids_and_numbers() {
        let reg = Registry::new();
        seed(&reg, &[saved("b", 5, "Beta"), saved("a", 2, "Alpha")]);
        let snap = reg.snapshot();
        let rows: Vec<_> = snap
            .sessions
            .iter()
            .map(|s| (s.tab_id.as_str(), s.number, s.name.as_str()))
            .collect();
        assert_eq!(rows, vec![("b", 5, "Beta"), ("a", 2, "Alpha")]);
        for row in &snap.sessions {
            assert_eq!(row.state, SessionState::Ended);
            assert_eq!(row.command, CommandState::Unknown);
            assert_eq!(row.agent.as_deref(), Some("claude"));
            assert_eq!((row.profile_id.as_str(), row.cwd.as_str()), ("pwsh", "C:/work"));
        }
        // The numbers stay taken: the next new tab takes the lowest free one.
        assert_eq!(reg.reserve("c", relaunch("C")).0, 1);
        assert_eq!(
            reg.saved_tabs().iter().map(|t| t.number).collect::<Vec<_>>(),
            vec![5, 2, 1]
        );
    }

    #[test]
    fn seed_ended_renumbers_a_duplicate_number() {
        let reg = Registry::new();
        seed(&reg, &[saved("a", 1, "A"), saved("b", 1, "B"), saved("c", 3, "C")]);
        let numbers: Vec<u32> = reg.snapshot().sessions.iter().map(|s| s.number).collect();
        assert_eq!(numbers, vec![1, 2, 3]);
    }

    #[test]
    fn saved_tabs_lists_open_entries_of_any_state_in_order() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.reserve("b", relaunch("B"));
        reg.reserve("c", relaunch("C"));
        reg.mark_exited("b", generation_of(&reg, "b"), 1);
        reg.close("c");
        reg.reorder(&["b".into(), "a".into()]);
        let tabs = reg.saved_tabs();
        assert_eq!(
            tabs.iter().map(|t| t.tab_id.as_str()).collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert_eq!(
            (tabs[0].number, tabs[0].name.as_str(), tabs[0].cwd.as_str()),
            (2, "B", "C:/work")
        );
    }
}
