//! The core's list of sessions: numbers, names, order and recently closed.
//! The UI never invents any of these; it reads them from `snapshot()` or
//! follows `subscribe()`.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::pty::Mark;
use marshell_protocol::api::{CommandState, SavedTab, SessionInfo, SessionState, SessionsSnapshot};
use tokio::sync::watch;

/// How many closed sessions we remember for Reopen.
const MAX_CLOSED: usize = 10;

/// What we need to start the same session again.
#[derive(Debug, Clone)]
pub struct Relaunch {
    pub profile_id: String,
    pub agent: Option<String>,
    pub cwd: String,
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
}

struct State {
    open: Vec<Entry>,
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
            closed: VecDeque::new(),
        };
        let (tx, _rx) = watch::channel(state.snapshot());
        Self {
            state: Mutex::new(state),
            tx,
        }
    }

    /// Adds a Running row at the end and returns its number (lowest free).
    /// If another open session already has exactly this name, the shown
    /// name becomes "<name> <number>". The stored `Relaunch` keeps the name.
    pub fn reserve(&self, tab_id: &str, relaunch: Relaunch) -> u32 {
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
                state: SessionState::Running,
                command: CommandState::Unknown,
            },
            spec: relaunch,
        });
        self.tx.send_replace(state.snapshot());
        number
    }

    /// Adds a Running row at the end for a process that is already running (the shell panel's),
    /// and returns its number. Same naming rule as `reserve`.
    pub fn adopt(&self, tab_id: &str, relaunch: Relaunch) -> u32 {
        self.reserve(tab_id, relaunch)
    }

    /// Adds the saved tabs as Ended rows, in the given order, with their ids and numbers.
    /// Nothing is started. If two tabs share a number, the later one gets the lowest free number.
    /// `relaunch_of` says how each tab would be started again.
    pub fn seed_ended(&self, tabs: &[SavedTab], relaunch_of: impl Fn(&SavedTab) -> Relaunch) {
        let mut state = self.state.lock().unwrap();
        for tab in tabs {
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
                    state: SessionState::Ended,
                    command: CommandState::Unknown,
                },
                spec: relaunch_of(tab),
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
            })
            .collect()
    }

    /// Starts a Resume: an Ended or Exited tab becomes Running, in the same row (same id, number,
    /// name and place). The check and the change happen under one lock, so of two calls at once
    /// only one succeeds. Returns how to start it and the state it had, for `set_state` if the start fails.
    pub fn begin_resume(&self, tab_id: &str) -> Result<(Relaunch, SessionState), ResumeRefusal> {
        let mut state = self.state.lock().unwrap();
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
        let relaunch = entry.spec.clone();
        self.tx.send_replace(state.snapshot());
        Ok((relaunch, previous))
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

    pub fn mark_exited(&self, tab_id: &str, code: i32) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) {
            entry.info.state = SessionState::Exited { code };
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
            Mark::CommandStart => return,
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
        let entry = state.open.remove(index);
        let spec = entry.spec.clone();
        state.closed.push_front(entry);
        state.closed.truncate(MAX_CLOSED);
        self.tx.send_replace(state.snapshot());
        Some(spec)
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
            name: name.into(),
        }
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

        reg.mark_exited("t1", 2);
        let (spec, previous) = reg.begin_resume("t1").unwrap();
        assert_eq!(spec.name, "Shell");
        assert_eq!(previous, SessionState::Exited { code: 2 });
        assert_eq!(reg.snapshot().sessions[0].state, SessionState::Running);
        // The second call finds it Running.
        assert_eq!(reg.begin_resume("t1").unwrap_err(), ResumeRefusal::AlreadyRunning);

        reg.set_state("t1", previous);
        assert_eq!(reg.snapshot().sessions[0].state, SessionState::Exited { code: 2 });
    }

    #[test]
    fn numbers_are_lowest_free() {
        let reg = Registry::new();
        for i in 1..=6 {
            assert_eq!(reg.reserve(&format!("t{i}"), relaunch("Shell")), i);
        }
        reg.close("t4");
        assert_eq!(reg.reserve("t7", relaunch("Shell")), 4);
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
        reg.mark_exited("b", 3);
        let snap = reg.snapshot();
        assert_eq!(snap.sessions[1].state, SessionState::Exited { code: 3 });
        assert_eq!(snap.sessions[1].number, 2);
        // The number stays taken while the exited row is open.
        assert_eq!(reg.reserve("c", relaunch("C")), 3);
    }

    #[test]
    fn release_frees_number_without_closed_entry() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.release("a");
        let snap = reg.snapshot();
        assert!(snap.sessions.is_empty() && snap.closed.is_empty());
        assert_eq!(reg.reserve("b", relaunch("B")), 1);
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
        reg.mark_exited("a", 0);
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
        reg.mark_exited("a", 1);
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
        }
    }

    fn seed(reg: &Registry, tabs: &[SavedTab]) {
        reg.seed_ended(tabs, |t| Relaunch {
            profile_id: t.profile_id.clone(),
            agent: t.agent.clone(),
            cwd: t.cwd.clone(),
            name: t.name.clone(),
        });
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
        assert_eq!(reg.reserve("c", relaunch("C")), 1);
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
        reg.mark_exited("b", 1);
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
