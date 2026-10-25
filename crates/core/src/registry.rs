//! The core's list of sessions: numbers, names, order and recently closed.
//! The UI never invents any of these; it reads them from `snapshot()` or
//! follows `subscribe()`.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::pty::Mark;
use crate::stuck::{StuckSignal, StuckTracker};
use crate::tools::{PortChip, ToolEvent};
use marshell_protocol::api::{
    Activity, Attention, CommandState, Foreground, LiveState, LiveStatus, SavedTab, SessionInfo, SessionMode,
    SessionState, SessionsSnapshot, UiView,
};
use tokio::sync::watch;

/// A bell this soon after the person's own typing is its echo (a shell's refused key), not a program asking.
const TYPING_ECHO: Duration = Duration::from_secs(1);

/// Whether a terminal program's signal may become attention on this row. Never on a row with hook status:
/// its hooks already report the same moments. A notification always counts; a bare bell only while a
/// program runs (at the prompt it is a failed Tab completion) and not right after the person typed.
pub(crate) fn attention_allowed(info: &SessionInfo, bell: bool, last_input: Option<Instant>, now: Instant) -> bool {
    if info.live.is_some() {
        return false;
    }
    if !bell {
        return true;
    }
    // An agent launched as the session is itself the program running; it has no prompt.
    let busy = info.agent.is_some() || info.command == CommandState::Running || info.foreground.is_some();
    busy && last_input.is_none_or(|at| now.saturating_duration_since(at) >= TYPING_ECHO)
}

/// How many port chips a row keeps.
const MAX_PORTS: usize = 8;

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
    /// The preset it started with ("deepseek"). Only the name: a restart or reopen recomputes the preset's
    /// environment, with the key, from the environment at that moment.
    pub preset: Option<String>,
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
    /// The ids of the subagents the agent reports running. Not serialized; the row shows its size.
    running: std::collections::BTreeSet<String>,
    /// The main turn has stopped while subagents still run. Not serialized.
    waiting: bool,
    /// A finished turn the row has not shown yet: held a moment, so a turn that resumes at once never
    /// reads Done. Cleared by the next hook event, a process change or the end of a subagent wait.
    /// Not serialized.
    pending_done: Option<LiveStatus>,
    stuck: StuckTracker,
    /// When the person last typed into the session. Not serialized.
    last_input: Option<Instant>,
    /// The program reported OSC 9;4 progress (normal or indeterminate) and has not cleared it: the
    /// row holds Working, whatever the output watcher says. Only a row on the output fallback takes
    /// it. Not serialized.
    progress_hold: bool,
    /// Another agent runs in front of a row that was launched as an agent, and the launched one is gone from the
    /// foreground: the agent the watcher saw. The row still says what it was launched as. Not serialized.
    displaced_by: Option<String>,
    /// The row's attention is a tool's prompt (a profiled command asked the person something), raised by
    /// `tool_event`. It is a full Needs you: looking at the tab does not clear it. Only typing, the tool's next
    /// output, the command's end and a new process take it. Not serialized.
    tool_attention: bool,
    /// Ports the last port poll saw listening that are not chips yet: a port becomes a chip on its second poll in a row.
    /// Not serialized.
    listen_pending: Vec<u16>,
    /// Announced chips a port poll has seen listening. An announced chip goes on a poll that misses it only once it was
    /// seen, so a slow server's chip survives until its listener appears. Not serialized.
    announced_seen: Vec<u16>,
}

impl Entry {
    /// Drops the announced chips, whose command has ended. True when there were some.
    fn drop_announced_ports(&mut self) -> bool {
        let before = self.info.ports.len();
        self.info.ports.retain(|p| !p.announced);
        self.announced_seen.clear();
        self.info.ports.len() != before
    }

    /// True for a row the output watcher fills: Running, not paused, without live status, and an
    /// agent, either launched as one or typed into a shell.
    fn takes_activity(&self) -> bool {
        self.info.state == SessionState::Running
            && !self.info.paused
            && self.info.live.is_none()
            && (self.info.agent.is_some() || self.info.foreground.as_ref().is_some_and(|f| f.agent.is_some()))
    }

    /// A plain shell row that no longer runs an agent in front keeps no output activity or hold.
    fn drop_activity_without_agent(&mut self) {
        if !self.takes_activity() && self.info.agent.is_none() {
            self.info.activity = None;
            self.progress_hold = false;
        }
    }

    fn awaits_subagents_end(&self) -> bool {
        self.info.state == SessionState::Running
            && self.waiting
            && self.running.is_empty()
            && self
                .info
                .live
                .as_ref()
                .is_some_and(|live| live.state == LiveState::Working)
    }

    /// Forgets what the old process reported: its live status, its output activity and the note
    /// that said why there was no status. A new process, or none, starts blank.
    fn clear_live(&mut self) {
        self.info.live = None;
        self.drop_attention();
        self.info.stuck = None;
        self.stuck = StuckTracker::default();
        self.info.activity = None;
        self.progress_hold = false;
        self.displaced_by = None;
        self.info.status_note = None;
        self.running.clear();
        self.waiting = false;
        self.pending_done = None;
        self.info.ports.clear();
        self.listen_pending.clear();
        self.announced_seen.clear();
    }

    /// Takes the row's attention, whoever raised it, and the claim a tool prompt had on it. True when there was some.
    fn drop_attention(&mut self) -> bool {
        self.tool_attention = false;
        self.info.attention.take().is_some()
    }

    /// Sets attention, or keeps the one already set: a flood of bells is one attention, from its first
    /// signal. A later message fills in a missing one. True when the row changed.
    fn note_attention(&mut self, message: Option<String>) -> bool {
        match &mut self.info.attention {
            Some(attention) => {
                if attention.message.is_none() && message.is_some() {
                    attention.message = message;
                    true
                } else {
                    false
                }
            }
            None => {
                let since_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                self.info.attention = Some(Attention { message, since_ms });
                true
            }
        }
    }

    fn clear_stuck_outside_working(&mut self) {
        match self.info.live.as_ref().map(|live| live.state) {
            None | Some(LiveState::Ended) => {
                self.info.stuck = None;
                self.stuck = StuckTracker::default();
            }
            Some(LiveState::Working) => {}
            _ => {
                self.info.stuck = None;
                self.stuck.clear_repeat();
            }
        }
    }
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
                preset: relaunch.preset.clone(),
                state: SessionState::Running,
                command: CommandState::Unknown,
                paused: false,
                foreground: None,
                live: None,
                activity: None,
                status_note: None,
                muted: false,
                attention: None,
                stuck: None,
                ports: Vec::new(),
            },
            spec: relaunch,
            generation,
            running: Default::default(),
            waiting: false,
            pending_done: None,
            stuck: StuckTracker::default(),
            last_input: None,
            progress_hold: false,
            displaced_by: None,
            tool_attention: false,
            listen_pending: Vec::new(),
            announced_seen: Vec::new(),
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
                    preset: tab.preset.clone(),
                    state: SessionState::Ended,
                    command: CommandState::Unknown,
                    paused: false,
                    foreground: None,
                    live: None,
                    activity: None,
                    status_note: None,
                    muted: tab.muted,
                    attention: None,
                    stuck: None,
                    ports: Vec::new(),
                },
                spec: relaunch_of(tab),
                generation,
                running: Default::default(),
                waiting: false,
                pending_done: None,
                stuck: StuckTracker::default(),
                last_input: None,
                progress_hold: false,
                displaced_by: None,
                tool_attention: false,
                listen_pending: Vec::new(),
                announced_seen: Vec::new(),
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
                muted: e.info.muted,
                preset: e.spec.preset.clone(),
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
        entry.info.foreground = None;
        entry.clear_live();
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
        entry.info.foreground = None;
        entry.clear_live();
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
            if entry.info.state != SessionState::Running {
                entry.info.stuck = None;
                entry.stuck = StuckTracker::default();
            }
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
            entry.info.foreground = None;
            entry.clear_live();
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
    /// the row belongs to a newer process. A row that is not Running is never paused. Freezing
    /// drops the output activity, so a frozen row does not keep saying Working. Broadcasts only on
    /// a change.
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
            if paused {
                entry.info.activity = None;
                entry.progress_hold = false;
            }
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Sets what a plain shell session is running. Applied only to a Running row that was not
    /// launched as an agent, is not paused (a frozen session keeps what it showed) and whose
    /// `generation` is current: a result computed before a restart belongs to the old process.
    /// A program is refused while the row's shell is at its prompt: nothing runs in its foreground,
    /// and a reading that lost a race with the D mark, or a GUI child left behind, must not stick.
    /// Broadcasts only on a change.
    pub fn set_foreground(&self, tab_id: &str, generation: u64, foreground: Option<Foreground>) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id && e.generation == generation)
            && entry.info.state == SessionState::Running
            && entry.info.agent.is_none()
            && !entry.info.paused
            && !(foreground.is_some() && matches!(entry.info.command, CommandState::Prompt { .. }))
            && entry.info.foreground != foreground
        {
            // On a shell without marks the foreground clearing is the only sign that a command ended. A shell that
            // reports a running command keeps its announced ports: the foreground may only be between two programs.
            let ended =
                foreground.is_none() && entry.info.foreground.is_some() && entry.info.command != CommandState::Running;
            entry.info.foreground = foreground;
            entry.drop_activity_without_agent();
            if ended {
                entry.drop_announced_ports();
            }
            self.tx.send_replace(state.snapshot());
        }
    }

    /// The rows the foreground watcher may look at: Running, not paused, not launched as an agent.
    /// Each is the tab id, its generation and whether it already shows a foreground program.
    pub fn foreground_candidates(&self) -> Vec<(String, u64, bool)> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .filter(|e| e.info.state == SessionState::Running && !e.info.paused && e.info.agent.is_none())
            .map(|e| (e.info.tab_id.clone(), e.generation, e.info.foreground.is_some()))
            .collect()
    }

    /// The rows the port watcher may look at: Running and not paused. Each is the tab id and its generation.
    pub fn ports_candidates(&self) -> Vec<(String, u64)> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .filter(|e| e.info.state == SessionState::Running && !e.info.paused)
            .map(|e| (e.info.tab_id.clone(), e.generation))
            .collect()
    }

    /// Whether the row was launched as an agent or shows one in front: its outermost process is the agent itself, so
    /// its own listeners get no chip.
    pub fn runs_agent(&self, tab_id: &str) -> bool {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .find(|e| e.info.tab_id == tab_id)
            .is_some_and(|e| e.info.agent.is_some() || e.info.foreground.as_ref().is_some_and(|f| f.agent.is_some()))
    }

    /// What the port watcher found: `ports` are the ports a process of the tab listens on (ascending). Applied to a
    /// Running, unpaused row of this `generation` only. The chip list is ordered by first sight, at most 8.
    ///
    /// - A listening port becomes a chip on its second poll in a row, and goes on the first poll that misses it.
    /// - An announced chip stays through polls that miss it until one has seen it; after that a miss drops it.
    /// - A listening port that is already a chip (announced or not) adds nothing.
    ///
    /// Broadcasts only when the list changes.
    pub fn set_listening(&self, tab_id: &str, generation: u64, ports: &[u16]) {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id && e.generation == generation)
        else {
            return;
        };
        if entry.info.state != SessionState::Running || entry.info.paused {
            return;
        }
        let before = entry.info.ports.clone();
        let seen_before = std::mem::take(&mut entry.announced_seen);
        let mut kept = Vec::with_capacity(before.len());
        for chip in before.iter().cloned() {
            let listening = ports.contains(&chip.port);
            if chip.announced {
                if listening {
                    entry.announced_seen.push(chip.port);
                } else if seen_before.contains(&chip.port) {
                    continue;
                } else {
                    // Not seen yet: the server may still be starting.
                }
                kept.push(chip);
            } else if listening {
                kept.push(chip);
            }
        }
        let pending = std::mem::take(&mut entry.listen_pending);
        for &port in ports {
            if kept.iter().any(|c| c.port == port) || entry.listen_pending.contains(&port) {
                continue;
            }
            if pending.contains(&port) && kept.len() < MAX_PORTS {
                kept.push(PortChip {
                    port,
                    url: format!("http://localhost:{port}/"),
                    announced: false,
                });
            } else {
                // First sight, or no room yet: asked again next poll.
                entry.listen_pending.push(port);
            }
        }
        if kept != before {
            entry.info.ports = kept;
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Sets what an agent row is doing as far as its output shows, for a row without live status.
    /// Applied only to a Running, unpaused row with no `live` that was launched as an agent or shows
    /// an agent typed into its shell, whose `generation` is current. While the program holds
    /// progress (OSC 9;4), any activity the watcher sets reads Working. Broadcasts only on a change.
    pub fn set_activity(&self, tab_id: &str, generation: u64, activity: Option<Activity>) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id && e.generation == generation)
            && entry.takes_activity()
        {
            let activity = if entry.progress_hold && activity.is_some() {
                Some(Activity::Working)
            } else {
                activity
            };
            if entry.info.activity != activity {
                entry.info.activity = activity;
                self.tx.send_replace(state.snapshot());
            }
        }
    }

    /// The rows the activity watcher may look at: Running, not paused, without live status, and
    /// launched as an agent or running an agent typed into the shell. Each is the tab id and its
    /// generation.
    pub fn activity_candidates(&self) -> Vec<(String, u64)> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .filter(|e| e.takes_activity())
            .map(|e| (e.info.tab_id.clone(), e.generation))
            .collect()
    }

    /// The person typed into the session: a bell in the next second is its echo, and they have seen
    /// whatever asked for their attention. Broadcasts only on a change.
    pub fn note_input(&self, tab_id: &str) {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) else {
            return;
        };
        entry.last_input = Some(Instant::now());
        if entry.drop_attention() {
            self.tx.send_replace(state.snapshot());
        }
    }

    /// What the tab's tool output showed. A prompt is a full Needs you on a Running row without hook status: it is
    /// raised even when the tab is on screen, and only typing, the tool's next output (`Cleared`), the command's end
    /// and a new process take it. Attention that was already there (a bell, a notification) is kept and stays the
    /// bell's; `Cleared` never takes it. An announced port is added if new, up to 8; the list's lifetime is the
    /// port watcher's. Broadcasts only on a change.
    pub fn tool_event(&self, tab_id: &str, event: ToolEvent) {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) else {
            return;
        };
        if entry.info.state != SessionState::Running {
            return;
        }
        let changed = match event {
            ToolEvent::NeedsYou(phrase) => {
                if entry.info.live.is_none() && entry.info.attention.is_none() {
                    entry.note_attention(Some(phrase));
                    entry.tool_attention = true;
                    true
                } else {
                    false
                }
            }
            ToolEvent::Cleared => entry.tool_attention && entry.drop_attention(),
            ToolEvent::Port(chip) => {
                if entry.info.ports.len() < MAX_PORTS && !entry.info.ports.iter().any(|p| p.port == chip.port) {
                    entry.info.ports.push(chip);
                    true
                } else {
                    false
                }
            }
        };
        if changed {
            self.tx.send_replace(state.snapshot());
        }
    }

    /// The person is looking at these sessions: with the window focused, their attention is seen.
    /// Broadcasts only on a change.
    pub fn clear_attention_seen(&self, view: &UiView) {
        if !view.window_focused {
            return;
        }
        let mut state = self.state.lock().unwrap();
        let mut changed = false;
        for entry in state
            .open
            .iter_mut()
            .filter(|e| view.visible_tabs.contains(&e.info.tab_id) && !e.tool_attention)
        {
            changed |= entry.drop_attention();
        }
        if changed {
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Folds a shell-integration mark into the tab's command state, and a terminal program's notification
    /// or bell into its attention, unless `view` already shows the tab in a focused window: attention the person
    /// can see is seen. Marks for an exited or unknown tab are ignored. Broadcasts only on a change.
    pub fn mark(&self, tab_id: &str, mark: Mark, view: &UiView) {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) else {
            return;
        };
        if entry.info.state != SessionState::Running {
            return;
        }
        if let Mark::Progress(progress) = mark {
            // Progress only drives the output fallback: a row with hook status ignores it. 1 and 3
            // hold Working; 0, 2 and 4 release the hold and the ordinary fallback resumes.
            let hold = match progress {
                1 | 3 => true,
                0 | 2 | 4 => false,
                _ => return,
            };
            if entry.takes_activity() {
                entry.progress_hold = hold;
                if hold && entry.info.activity != Some(Activity::Working) {
                    entry.info.activity = Some(Activity::Working);
                    self.tx.send_replace(state.snapshot());
                }
            }
            return;
        }
        let signal = match &mark {
            Mark::Notify(message) => Some((message.clone(), false)),
            Mark::Bell => Some((None, true)),
            _ => None,
        };
        if let Some((message, bell)) = signal {
            let on_screen = view.window_focused && view.visible_tabs.iter().any(|t| t == tab_id);
            if !on_screen
                && attention_allowed(&entry.info, bell, entry.last_input, Instant::now())
                && entry.note_attention(message)
            {
                self.tx.send_replace(state.snapshot());
            }
            return;
        }
        let last_exit = match entry.info.command {
            CommandState::Prompt { last_exit } => last_exit,
            _ => None,
        };
        // The command is over or the prompt is back: whatever ran is gone.
        let command_ended = matches!(mark, Mark::Finished(_) | Mark::PromptStart);
        let cleared = command_ended && entry.info.foreground.take().is_some();
        if cleared {
            entry.drop_activity_without_agent();
        }
        // The command's announced ports go with it; a listener that outlives it is the port watcher's to drop.
        let cleared = (command_ended && entry.drop_announced_ports()) | cleared;
        // A tool's prompt goes with its command. A bell's or hook's attention stays.
        let cleared = (command_ended && entry.tool_attention && entry.drop_attention()) | cleared;
        let next = match mark {
            Mark::Executed => CommandState::Running,
            Mark::Finished(Some(code)) => CommandState::Prompt { last_exit: Some(code) },
            Mark::Finished(None) | Mark::PromptStart => CommandState::Prompt { last_exit },
            Mark::CommandStart
            | Mark::CommandLine(_)
            | Mark::Cwd(_)
            | Mark::Notify(_)
            | Mark::Progress(_)
            | Mark::Bell => return,
        };
        let changed = entry.info.command != next;
        if changed {
            entry.info.command = next;
        }
        if changed || cleared {
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Folds a report from the agent into the tab's live status: `f` gets the current one and
    /// returns the next, or None to leave it. Applied only to an open Running row. Live status
    /// replaces any output activity. Broadcasts only on a change.
    pub fn update_live(&self, tab_id: &str, f: impl FnOnce(Option<&LiveStatus>) -> Option<LiveStatus>) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
            && let Some(next) = f(entry.info.live.as_ref())
            && entry.info.live.as_ref() != Some(&next)
        {
            entry.info.live = Some(next);
            entry.clear_stuck_outside_working();
            entry.info.activity = None;
            entry.drop_attention();
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Feeds a hook event, never a status line, into the session's stuck tracker.
    pub fn stuck_event(&self, tab_id: &str, signal: StuckSignal, now_ms: u64) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
        {
            let previous = entry.info.stuck.clone();
            entry.info.stuck = entry.stuck.on_event(signal, now_ms);
            entry.clear_stuck_outside_working();
            if entry.info.stuck != previous {
                self.tx.send_replace(state.snapshot());
            }
        }
    }

    /// Checks hook silence while Working. A repeat verdict takes precedence over quiet.
    pub fn check_stuck(&self, now_ms: u64, quiet_ms: u64) {
        let mut state = self.state.lock().unwrap();
        let mut changed = false;
        for entry in &mut state.open {
            let previous = entry.info.stuck.clone();
            if entry.info.state == SessionState::Running {
                let working = entry
                    .info
                    .live
                    .as_ref()
                    .is_some_and(|live| live.state == LiveState::Working);
                entry.info.stuck = entry
                    .stuck
                    .repeat()
                    .or_else(|| entry.stuck.quiet(working, now_ms, quiet_ms));
                entry.clear_stuck_outside_working();
            } else {
                entry.info.stuck = None;
                entry.stuck = StuckTracker::default();
            }
            changed |= entry.info.stuck != previous;
        }
        if changed {
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Follows the permission mode Claude Code reports: sets the row's mode and the mode Resume and
    /// the next launch use. Applied only to an open Running Claude row, and only on a change: the
    /// hooks reach any row whose process tree runs `claude`, and another agent's mode isn't theirs.
    pub fn set_live_mode(&self, tab_id: &str, mode: SessionMode) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
            && entry.info.agent.as_deref() == Some("claude")
            && entry.info.mode != Some(mode)
        {
            entry.info.mode = Some(mode);
            entry.spec.mode = Some(mode);
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Follows the subagents the agent reports: folds the hook body into the tab's running set and,
    /// when it changed, sets the live status's count to the set's size (and counts a SubagentStop that took
    /// an id out in `subagents_finished`, which the main Stop and SessionEnd never raise). Applied only to an open
    /// Running row that is Claude's or a plain shell's (where `claude` was typed), never another
    /// agent's. Broadcasts once per change.
    pub fn update_subagents(&self, tab_id: &str, body: &serde_json::Value) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
            && matches!(entry.info.agent.as_deref(), None | Some("claude"))
            && crate::agents::claude::status::fold_subagents(&mut entry.running, body)
        {
            let count = entry.running.len() as u32;
            let at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            let live = entry
                .info
                .live
                .get_or_insert_with(|| crate::agents::common::blank(at_ms));
            live.subagents = count;
            // Only a SubagentStop that took an id out is a subagent finishing; the main Stop and SessionEnd
            // clear the set too, and those are not.
            if body.get("hook_event_name").and_then(serde_json::Value::as_str) == Some("SubagentStop") {
                live.subagents_finished = live.subagents_finished.saturating_add(1);
            }
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Follows the main turn's wait for subagents on an open Running Claude or plain-shell row.
    /// Keeps Working rows showing the wait while leaving permission and question prompts visible.
    /// Broadcasts only when the live status changes.
    pub fn follow_subagent_wait(&self, tab_id: &str, body: &serde_json::Value) {
        use crate::agents::claude::status::{WAITING_PHRASE, running_subagents};

        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
            && matches!(entry.info.agent.as_deref(), None | Some("claude"))
        {
            // Every hook event is newer than a held Done; a Stop holds its own after this runs.
            entry.pending_done = None;
            if body.get("agent_id").is_none() {
                match body.get("hook_event_name").and_then(serde_json::Value::as_str) {
                    Some("Stop") => entry.waiting = running_subagents(body).next().is_some(),
                    Some("UserPromptSubmit" | "SessionStart" | "SessionEnd" | "PreToolUse") => {
                        entry.waiting = false;
                    }
                    _ => {}
                }
            }
            if entry.waiting
                && let Some(live) = entry.info.live.as_mut()
                && live.state == LiveState::Working
                && live.phrase != WAITING_PHRASE
            {
                live.phrase = WAITING_PHRASE.to_string();
                self.tx.send_replace(state.snapshot());
            }
        }
    }

    /// Reports whether an open Running row is still waiting for subagents with none left in its
    /// running set. Reads under the state lock and changes nothing.
    pub fn awaits_subagents_end(&self, tab_id: &str) -> bool {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .any(|entry| entry.info.tab_id == tab_id && entry.awaits_subagents_end())
    }

    /// Ends the wait for subagents when none remain: sets Done and its time on an open Running
    /// row still Working and waiting. Broadcasts once and returns true on a change;
    /// otherwise changes nothing and returns false.
    pub fn settle_subagent_wait(&self, tab_id: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.awaits_subagents_end()
            && let Some(live) = entry.info.live.as_mut()
        {
            live.state = LiveState::Done;
            live.phrase = "Done".to_string();
            live.since_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            entry.clear_stuck_outside_working();
            entry.waiting = false;
            entry.pending_done = None;
            self.tx.send_replace(state.snapshot());
            return true;
        }
        false
    }

    /// Holds a finished turn back: keeps `done` (state, phrase, time and recap) for `settle_done` and
    /// leaves the row as it is, without a broadcast, so a turn that resumes at once never reads Done.
    /// Applied only to an open Running Claude or plain-shell row; returns true when held. The next
    /// hook event for the tab (`follow_subagent_wait` sees them all), the end of a subagent wait or a
    /// process change drops it. A status line update does not.
    pub fn hold_done(&self, tab_id: &str, done: LiveStatus) -> bool {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
            && matches!(entry.info.agent.as_deref(), None | Some("claude"))
        {
            entry.pending_done = Some(done);
            return true;
        }
        false
    }

    /// Shows a held Done if it is still held and the row is Running: the state, phrase, time and recap
    /// of the Stop, over whatever a status line said since (or as the whole status when the row had
    /// none). Broadcasts once; otherwise changes nothing.
    pub fn settle_done(&self, tab_id: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && let Some(done) = entry.pending_done.take()
            && entry.info.state == SessionState::Running
        {
            if let Some(live) = entry.info.live.as_mut() {
                live.state = done.state;
                live.phrase = done.phrase;
                live.since_ms = done.since_ms;
                if done.recap.is_some() {
                    live.recap = done.recap;
                }
            } else if entry.info.status_note.is_none() {
                entry.info.live = Some(done);
                entry.info.activity = None;
            } else {
                return;
            }
            entry.clear_stuck_outside_working();
            self.tx.send_replace(state.snapshot());
        }
    }

    /// Sets why a Claude Code row has no live status (a gated version, or the switch is off), or
    /// takes the note away. A note also drops the live status, so a switch turned off mid-session
    /// leaves no stale status. Applied only to an open Running row. Broadcasts only on a change.
    pub fn set_status_note(&self, tab_id: &str, note: Option<String>) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id)
            && entry.info.state == SessionState::Running
            && (entry.info.status_note != note || (note.is_some() && entry.info.live.is_some()))
        {
            if note.is_some() {
                entry.info.live = None;
                entry.clear_stuck_outside_working();
                entry.pending_done = None;
            }
            entry.info.status_note = note;
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

    /// Mutes or unmutes an open tab's notifications. Returns false for an unknown tab; subscribers
    /// hear of it only when the value changes.
    pub fn set_muted(&self, tab_id: &str, muted: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        let Some(entry) = state.open.iter_mut().find(|e| e.info.tab_id == tab_id) else {
            return false;
        };
        if entry.info.muted != muted {
            entry.info.muted = muted;
            self.tx.send_replace(state.snapshot());
        }
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
        entry.info.foreground = None;
        entry.clear_live();
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

    /// The agent the open row runs: the one it was launched as while that agent is still in the foreground, else the other
    /// agent the watcher saw take its place (the launched one exited and the person typed another), else the one in a
    /// shell's foreground.
    pub fn effective_agent(&self, tab_id: &str) -> Option<String> {
        let state = self.state.lock().unwrap();
        let entry = state.open.iter().find(|e| e.info.tab_id == tab_id)?;
        let info = &entry.info;
        entry
            .displaced_by
            .clone()
            .or_else(|| info.agent.clone())
            .or_else(|| info.foreground.as_ref().and_then(|f| f.agent.clone()))
    }

    /// The rows launched as an agent that the watcher reads the foreground of: Running and not paused.
    /// Each is the tab id, its generation and the agent it was launched as.
    pub fn launched_candidates(&self) -> Vec<(String, u64, String)> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .filter(|e| e.info.state == SessionState::Running && !e.info.paused)
            .filter_map(|e| Some((e.info.tab_id.clone(), e.generation, e.info.agent.clone()?)))
            .collect()
    }

    /// Records that `agent` runs in front of a row launched as another agent, with the launched one gone from the
    /// foreground; `None` when it is not so. Ignored for an old `generation`, a row that is not Running or one that was
    /// not launched as an agent. Not part of the row, so nothing is broadcast.
    pub fn set_displacing_agent(&self, tab_id: &str, generation: u64, agent: Option<String>) {
        let mut state = self.state.lock().unwrap();
        if let Some(entry) = state
            .open
            .iter_mut()
            .find(|e| e.info.tab_id == tab_id && e.generation == generation)
            && entry.info.state == SessionState::Running
            && entry.info.agent.is_some()
        {
            entry.displaced_by = agent.filter(|a| entry.info.agent.as_deref() != Some(a.as_str()));
        }
    }

    /// The preset the tab started with, whether the tab is open or among the recently closed.
    pub fn preset_of_tab(&self, tab_id: &str) -> Option<String> {
        let state = self.state.lock().unwrap();
        state
            .open
            .iter()
            .chain(state.closed.iter())
            .find(|e| e.info.tab_id == tab_id)
            .and_then(|e| e.spec.preset.clone())
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
    use marshell_protocol::api::LiveState;

    fn relaunch(name: &str) -> Relaunch {
        Relaunch {
            profile_id: "pwsh".into(),
            agent: None,
            cwd: "C:/work".into(),
            mode: None,
            name: name.into(),
            preset: None,
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
    fn live_mode_updates_the_row_and_the_relaunch_spec() {
        let reg = Registry::new();
        let claude = Relaunch {
            agent: Some("claude".into()),
            mode: Some(SessionMode::Manual),
            ..relaunch("Claude")
        };
        reg.reserve("c", claude);
        reg.reserve("s", relaunch("Shell"));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();

        reg.set_live_mode("c", SessionMode::Plan);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        let mode_of = |tab: &str| {
            reg.snapshot()
                .sessions
                .into_iter()
                .find(|s| s.tab_id == tab)
                .unwrap()
                .mode
        };
        assert_eq!(mode_of("c"), Some(SessionMode::Plan));
        // The same mode again changes nothing and broadcasts nothing.
        reg.set_live_mode("c", SessionMode::Plan);
        assert!(!rx.has_changed().unwrap());
        // A plain shell row has no agent: nothing changes.
        reg.set_live_mode("s", SessionMode::Plan);
        assert_eq!(mode_of("s"), None);
        assert!(!rx.has_changed().unwrap());
        // Claude's hooks reach any row whose process tree runs `claude`; another agent's mode is not Claude's to set.
        let codex = Relaunch {
            agent: Some("codex".into()),
            mode: Some(SessionMode::Manual),
            ..relaunch("Codex")
        };
        reg.reserve("x", codex);
        rx.borrow_and_update();
        reg.set_live_mode("x", SessionMode::Bypass);
        assert_eq!(mode_of("x"), Some(SessionMode::Manual));
        assert!(!rx.has_changed().unwrap());
        reg.set_live_mode("nope", SessionMode::Plan);
        assert!(!rx.has_changed().unwrap());

        reg.mark_exited("c", generation_of(&reg, "c"), 0);
        rx.borrow_and_update();
        // A row that is not Running keeps its mode.
        reg.set_live_mode("c", SessionMode::Bypass);
        assert!(!rx.has_changed().unwrap());
        // Resume reads the spec: it reopens in the mode the agent was in.
        let (spec, _, _) = reg.begin_resume("c").unwrap();
        assert_eq!(spec.mode, Some(SessionMode::Plan));
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
        reg.mark("b", Mark::Executed, &away());
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
    fn set_muted_broadcasts_only_a_change_and_is_saved() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        assert!(reg.set_muted("a", true));
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        assert!(reg.set_muted("a", true));
        assert!(!rx.has_changed().unwrap());
        assert!(!reg.set_muted("missing", true));
        let tabs = reg.saved_tabs();
        assert!(tabs[0].muted);
        let again = Registry::new();
        seed(&again, &tabs);
        assert!(again.snapshot().sessions[0].muted);
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
        reg.mark("a", Mark::Finished(None), &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: None });
        reg.mark("a", Mark::PromptStart, &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: None });
        reg.mark("a", Mark::CommandStart, &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: None });
        reg.mark("a", Mark::Executed, &away());
        assert_eq!(command(&reg), CommandState::Running);
        reg.mark("a", Mark::Finished(Some(2)), &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(2) });
        reg.mark("a", Mark::PromptStart, &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(2) });
        // An empty Enter (D without a code) keeps the last code.
        reg.mark("a", Mark::Finished(None), &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(2) });
        reg.mark("a", Mark::Executed, &away());
        reg.mark("a", Mark::Finished(Some(0)), &away());
        assert_eq!(command(&reg), CommandState::Prompt { last_exit: Some(0) });
    }

    #[test]
    fn marks_after_exit_are_ignored() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        reg.mark_exited("a", generation_of(&reg, "a"), 1);
        reg.mark("a", Mark::Finished(Some(1)), &away());
        assert_eq!(command(&reg), CommandState::Running);
        reg.mark("missing", Mark::PromptStart, &away());
    }

    #[test]
    fn unchanged_state_does_not_broadcast() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        reg.mark("a", Mark::PromptStart, &away());
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        reg.mark("a", Mark::PromptStart, &away());
        reg.mark("a", Mark::CommandStart, &away());
        reg.mark("a", Mark::Finished(None), &away());
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
            muted: false,
            preset: None,
        }
    }

    fn seed(reg: &Registry, tabs: &[SavedTab]) {
        reg.seed_ended(tabs, |t| Relaunch {
            profile_id: t.profile_id.clone(),
            agent: t.agent.clone(),
            cwd: t.cwd.clone(),
            mode: t.mode,
            name: t.name.clone(),
            preset: None,
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

    fn running(program: &str) -> Option<Foreground> {
        Some(Foreground {
            program: program.into(),
            command_line: None,
            agent: None,
            tool: None,
        })
    }

    fn foreground(reg: &Registry, tab_id: &str) -> Option<Foreground> {
        let snap = reg.snapshot();
        snap.sessions
            .iter()
            .find(|s| s.tab_id == tab_id)
            .unwrap()
            .foreground
            .clone()
    }

    #[test]
    fn set_foreground_applies_to_a_running_shell_row() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.set_foreground("a", generation_of(&reg, "a"), running("docker"));
        assert_eq!(foreground(&reg, "a"), running("docker"));
        reg.set_foreground("a", generation_of(&reg, "a"), None);
        assert_eq!(foreground(&reg, "a"), None);
    }

    #[test]
    fn set_foreground_is_ignored_for_an_agent_an_exited_row_a_paused_row_and_a_stale_generation() {
        let reg = Registry::new();
        reg.reserve(
            "agent",
            Relaunch {
                agent: Some("claude".into()),
                ..relaunch("Claude")
            },
        );
        reg.set_foreground("agent", generation_of(&reg, "agent"), running("docker"));
        assert_eq!(foreground(&reg, "agent"), None);

        reg.reserve("exited", relaunch("E"));
        let generation = generation_of(&reg, "exited");
        reg.mark_exited("exited", generation, 0);
        reg.set_foreground("exited", generation, running("docker"));
        assert_eq!(foreground(&reg, "exited"), None);

        reg.reserve("paused", relaunch("P"));
        let generation = generation_of(&reg, "paused");
        reg.set_foreground("paused", generation, running("docker"));
        reg.set_paused("paused", generation, true);
        reg.set_foreground("paused", generation, None);
        assert_eq!(foreground(&reg, "paused"), running("docker"));

        reg.reserve("stale", relaunch("S"));
        let generation = generation_of(&reg, "stale");
        reg.set_foreground("stale", generation + 1, running("docker"));
        assert_eq!(foreground(&reg, "stale"), None);
        reg.set_foreground("nope", generation, running("docker"));
    }

    #[test]
    fn a_row_at_its_prompt_refuses_a_foreground() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        reg.mark("a", Mark::Executed, &away());
        reg.set_foreground("a", generation, running("git"));
        assert_eq!(foreground(&reg, "a"), running("git"));
        // The command finished while the reading was on its way: it must not land after the D mark.
        reg.mark("a", Mark::Finished(Some(0)), &away());
        assert_eq!(foreground(&reg, "a"), None);
        reg.set_foreground("a", generation, running("notepad"));
        assert_eq!(foreground(&reg, "a"), None);
        // A new command shows its program again.
        reg.mark("a", Mark::Executed, &away());
        reg.set_foreground("a", generation, running("notepad"));
        assert_eq!(foreground(&reg, "a"), running("notepad"));
    }

    #[test]
    fn set_foreground_broadcasts_only_on_a_change() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.set_foreground("a", generation, None);
        assert!(!rx.has_changed().unwrap());
        reg.set_foreground("a", generation, running("docker"));
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        reg.set_foreground("a", generation, running("docker"));
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn the_end_of_a_command_or_the_prompt_clears_the_foreground() {
        for mark in [Mark::Finished(Some(0)), Mark::Finished(None), Mark::PromptStart] {
            let reg = Registry::new();
            reg.reserve("a", relaunch("A"));
            reg.mark("a", Mark::Executed, &away());
            reg.set_foreground("a", generation_of(&reg, "a"), running("docker"));
            // Marks that do not end the command leave it.
            reg.mark("a", Mark::CommandStart, &away());
            assert_eq!(foreground(&reg, "a"), running("docker"));
            let mut rx = reg.subscribe();
            rx.borrow_and_update();
            reg.mark("a", mark, &away());
            assert_eq!(foreground(&reg, "a"), None);
            assert!(rx.has_changed().unwrap());
        }
    }

    #[test]
    fn exit_restart_resume_and_close_clear_the_foreground() {
        let reg = Registry::new();
        for id in ["exit", "restart", "resume", "close"] {
            reg.reserve(id, relaunch(id));
            reg.set_foreground(id, generation_of(&reg, id), running("docker"));
        }
        reg.mark_exited("exit", generation_of(&reg, "exit"), 0);
        assert_eq!(foreground(&reg, "exit"), None);

        reg.begin_restart("restart", SessionMode::Plan).unwrap();
        assert_eq!(foreground(&reg, "restart"), None);

        // A shell that exited without clearing, as a race could leave it, still resumes clean.
        reg.mark_exited("resume", generation_of(&reg, "resume"), 0);
        reg.begin_resume("resume").unwrap();
        assert_eq!(foreground(&reg, "resume"), None);

        reg.close("close");
        assert_eq!(reg.snapshot().closed[0].foreground, None);
    }

    #[test]
    fn foreground_candidates_are_running_unpaused_shell_rows() {
        let reg = Registry::new();
        reg.reserve("shell", relaunch("A"));
        reg.reserve(
            "agent",
            Relaunch {
                agent: Some("claude".into()),
                ..relaunch("B")
            },
        );
        reg.reserve("paused", relaunch("C"));
        reg.reserve("exited", relaunch("D"));
        reg.set_paused("paused", generation_of(&reg, "paused"), true);
        reg.mark_exited("exited", generation_of(&reg, "exited"), 0);
        reg.set_foreground("shell", generation_of(&reg, "shell"), running("docker"));
        let ids: Vec<_> = reg
            .foreground_candidates()
            .into_iter()
            .map(|(id, _, has)| (id, has))
            .collect();
        assert_eq!(ids, vec![("shell".to_string(), true)]);
    }

    #[test]
    fn a_launched_agent_gives_way_to_the_agent_that_took_its_place() {
        let reg = Registry::new();
        let launched = |agent: &str| Relaunch {
            agent: Some(agent.into()),
            ..relaunch("A")
        };
        reg.reserve("agent", launched("claude"));
        reg.reserve("shell", relaunch("B"));
        let generation = generation_of(&reg, "agent");
        // Only a Running row launched as an agent is read by the watcher for this.
        let candidates = reg.launched_candidates();
        assert_eq!(
            candidates,
            vec![("agent".to_string(), generation, "claude".to_string())]
        );

        assert_eq!(
            reg.effective_agent("agent").as_deref(),
            Some("claude"),
            "no sighting yet"
        );
        reg.set_displacing_agent("agent", generation, Some("grok".into()));
        assert_eq!(reg.effective_agent("agent").as_deref(), Some("grok"));
        // The row itself still says what it was launched as.
        assert_eq!(row(&reg, "agent").agent.as_deref(), Some("claude"));
        // An old process's reading, a plain shell row and the launched agent itself change nothing.
        reg.set_displacing_agent("agent", generation + 1, None);
        assert_eq!(reg.effective_agent("agent").as_deref(), Some("grok"));
        reg.set_displacing_agent("shell", generation_of(&reg, "shell"), Some("grok".into()));
        assert_eq!(reg.effective_agent("shell"), None);
        reg.set_displacing_agent("agent", generation, Some("claude".into()));
        assert_eq!(reg.effective_agent("agent").as_deref(), Some("claude"));
        // A restart forgets it.
        reg.set_displacing_agent("agent", generation, Some("grok".into()));
        reg.mark_exited("agent", generation, 0);
        assert_eq!(reg.effective_agent("agent").as_deref(), Some("claude"));
    }

    fn live(state: LiveState) -> LiveStatus {
        LiveStatus {
            state,
            phrase: String::new(),
            since_ms: 1,
            recap: None,
            model: None,
            effort: None,
            context_pct: None,
            five_hour_pct: None,
            five_hour_resets_at: None,
            seven_day_pct: None,
            subagents: 0,
            subagents_finished: 0,
        }
    }

    fn row(reg: &Registry, tab_id: &str) -> SessionInfo {
        reg.snapshot()
            .sessions
            .into_iter()
            .find(|s| s.tab_id == tab_id)
            .unwrap()
    }

    #[test]
    fn attention_allowed_follows_hooks_foreground_and_typing() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let shell = row(&reg, "a");
        let now = Instant::now();
        let ago = |ms: u64| now.checked_sub(Duration::from_millis(ms));
        // A notification counts at the prompt; a bell there is a failed Tab completion.
        assert!(attention_allowed(&shell, false, None, now));
        assert!(!attention_allowed(&shell, true, None, now));
        let running = SessionInfo {
            command: CommandState::Running,
            ..shell.clone()
        };
        assert!(attention_allowed(&running, true, None, now));
        let foreground = SessionInfo {
            foreground: Some(marshell_protocol::api::Foreground {
                program: "python".into(),
                command_line: None,
                agent: None,
                tool: None,
            }),
            ..shell.clone()
        };
        assert!(attention_allowed(&foreground, true, None, now));
        // An agent launched as the session is the program running; it has no prompt.
        let agent = SessionInfo {
            agent: Some("codex".into()),
            ..shell.clone()
        };
        assert!(attention_allowed(&agent, true, None, now));
        // The echo of the person's own typing is not a program asking.
        assert!(!attention_allowed(&running, true, ago(200), now));
        assert!(attention_allowed(&running, true, ago(2_000), now));
        assert!(attention_allowed(&running, false, ago(200), now));
        // Hooks already report these moments.
        let hooked = SessionInfo {
            live: Some(live(LiveState::Working)),
            ..running.clone()
        };
        assert!(!attention_allowed(&hooked, true, None, now));
        assert!(!attention_allowed(&hooked, false, None, now));
    }

    /// A window nobody is looking at.
    fn away() -> UiView {
        UiView {
            window_focused: false,
            visible_tabs: Vec::new(),
        }
    }

    #[test]
    fn attention_in_a_tab_on_screen_in_a_focused_window_is_not_raised() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        let seen = UiView {
            window_focused: true,
            visible_tabs: vec!["a".into()],
        };
        reg.mark("a", Mark::Notify(Some("Build done".into())), &seen);
        reg.mark("a", Mark::Bell, &seen);
        assert_eq!(row(&reg, "a").attention, None);
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn attention_in_a_hidden_or_unfocused_view_is_raised() {
        let views = [
            UiView {
                window_focused: false,
                visible_tabs: vec!["a".into()],
            },
            UiView {
                window_focused: true,
                visible_tabs: vec!["b".into()],
            },
            away(),
        ];
        for view in views {
            let reg = Registry::new();
            reg.reserve("a", relaunch("A"));
            let mut rx = reg.subscribe();
            rx.borrow_and_update();
            reg.mark("a", Mark::Notify(None), &view);
            assert!(row(&reg, "a").attention.is_some(), "{view:?}");
            assert!(rx.has_changed().unwrap(), "{view:?}");
        }
    }

    fn needs_you(phrase: &str) -> crate::tools::ToolEvent {
        crate::tools::ToolEvent::NeedsYou(phrase.into())
    }

    #[test]
    fn a_tool_prompt_raises_attention_and_typing_clears_it() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.tool_event("a", needs_you("Asks to continue"));
        let attention = row(&reg, "a").attention.expect("attention");
        assert_eq!(attention.message.as_deref(), Some("Asks to continue"));
        assert!(rx.has_changed().unwrap());
        // A second raise of the same prompt changes nothing.
        rx.borrow_and_update();
        reg.tool_event("a", needs_you("Asks to continue"));
        assert!(!rx.has_changed().unwrap());
        // Typing answers it.
        reg.note_input("a");
        assert_eq!(row(&reg, "a").attention, None);
        // More output (the cleared event) after the person answered has nothing left to take.
        rx.borrow_and_update();
        reg.tool_event("a", crate::tools::ToolEvent::Cleared);
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn a_cleared_event_takes_the_tool_prompt_but_not_a_bells_attention() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        reg.tool_event("a", needs_you("Asks to continue"));
        reg.tool_event("a", crate::tools::ToolEvent::Cleared);
        assert_eq!(row(&reg, "a").attention, None);
        // A bell's attention is not the tool's to take.
        reg.mark("a", Mark::Bell, &away());
        assert!(row(&reg, "a").attention.is_some());
        reg.tool_event("a", crate::tools::ToolEvent::Cleared);
        assert!(row(&reg, "a").attention.is_some());
        // And a bell's attention that was there first stays when a prompt shows and clears.
        reg.tool_event("a", needs_you("Asks to continue"));
        reg.tool_event("a", crate::tools::ToolEvent::Cleared);
        assert!(row(&reg, "a").attention.is_some());
    }

    #[test]
    fn a_cleared_event_does_not_take_a_bells_attention() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        reg.mark("a", Mark::Bell, &away());
        reg.tool_event("a", crate::tools::ToolEvent::Cleared);
        assert!(row(&reg, "a").attention.is_some());
    }

    #[test]
    fn a_tool_prompt_on_screen_stays_until_typed() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        // Raised while the tab is on screen in a focused window: a tool prompt is a full Needs you.
        reg.tool_event("a", needs_you("Waiting for sign-in in the browser"));
        assert!(row(&reg, "a").attention.is_some());
        reg.clear_attention_seen(&UiView {
            window_focused: true,
            visible_tabs: vec!["a".into()],
        });
        assert!(row(&reg, "a").attention.is_some(), "looking at it must not clear it");
        reg.note_input("a");
        assert_eq!(row(&reg, "a").attention, None);
    }

    #[test]
    fn a_tool_prompt_goes_when_its_command_ends() {
        for end in [Mark::Finished(Some(0)), Mark::Finished(None), Mark::PromptStart] {
            let reg = Registry::new();
            reg.reserve("a", relaunch("A"));
            reg.mark("a", Mark::Executed, &away());
            reg.tool_event("a", needs_you("Asks to continue"));
            let mut rx = reg.subscribe();
            rx.borrow_and_update();
            reg.mark("a", end.clone(), &away());
            assert_eq!(row(&reg, "a").attention, None, "{end:?}");
            assert!(rx.has_changed().unwrap(), "{end:?}");
        }
        // Not when the session ends or a status arrives from hooks either.
        let reg = Registry::new();
        let (_, generation) = reg.reserve("a", relaunch("A"));
        reg.tool_event("a", needs_you("Asks to continue"));
        reg.mark_exited("a", generation, 0);
        assert_eq!(row(&reg, "a").attention, None);
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.tool_event("a", needs_you("Asks to continue"));
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        assert_eq!(row(&reg, "a").attention, None);
    }

    #[test]
    fn a_tool_prompt_never_replaces_the_attention_already_there_and_ignores_other_rows() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Notify(Some("Build done".into())), &away());
        let before = row(&reg, "a").attention.unwrap();
        reg.tool_event("a", needs_you("Asks to continue"));
        assert_eq!(row(&reg, "a").attention, Some(before));
        // A row with hook status, an unknown tab and an exited row take nothing.
        let reg = Registry::new();
        let (_, generation) = reg.reserve("a", relaunch("A"));
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        reg.tool_event("a", needs_you("Asks to continue"));
        assert_eq!(row(&reg, "a").attention, None);
        reg.tool_event("missing", needs_you("Asks to continue"));
        reg.mark_exited("a", generation, 0);
        reg.tool_event("a", needs_you("Asks to continue"));
        assert_eq!(row(&reg, "a").attention, None);
    }

    #[test]
    fn announced_ports_are_added_once_up_to_eight() {
        use crate::tools::{PortChip, ToolEvent};
        let chip = |port: u16| PortChip {
            port,
            url: format!("http://localhost:{port}"),
            announced: true,
        };
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.tool_event("a", ToolEvent::Port(chip(3000)));
        assert_eq!(row(&reg, "a").ports, [chip(3000)]);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        reg.tool_event("a", ToolEvent::Port(chip(3000)));
        assert!(!rx.has_changed().unwrap());
        for port in 3001..3020 {
            reg.tool_event("a", ToolEvent::Port(chip(port)));
        }
        let ports = row(&reg, "a").ports;
        assert_eq!(ports.len(), 8);
        assert_eq!(ports[7], chip(3007));
        reg.tool_event("missing", ToolEvent::Port(chip(1)));
    }

    fn announced_chip(port: u16) -> crate::tools::PortChip {
        crate::tools::PortChip {
            port,
            url: format!("http://localhost:{port}/app"),
            announced: true,
        }
    }

    fn chip_ports(reg: &Registry, tab: &str) -> Vec<u16> {
        row(reg, tab).ports.iter().map(|p| p.port).collect()
    }

    #[test]
    fn an_announced_chip_outlives_a_slow_listener_and_goes_with_its_command() {
        use crate::tools::ToolEvent;
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        // A slow server: the chip stays through polls that miss it until one has seen it.
        reg.tool_event("a", ToolEvent::Port(announced_chip(3000)));
        reg.set_listening("a", generation, &[]);
        reg.set_listening("a", generation, &[]);
        assert_eq!(row(&reg, "a").ports, [announced_chip(3000)]);
        reg.set_listening("a", generation, &[3000]);
        reg.set_listening("a", generation, &[3000]);
        assert_eq!(
            row(&reg, "a").ports,
            [announced_chip(3000)],
            "it keeps the announced URL"
        );
        reg.set_listening("a", generation, &[]);
        assert_eq!(chip_ports(&reg, "a"), Vec::<u16>::new());
        // It goes with its command: the end mark.
        for end in [Mark::Finished(Some(0)), Mark::Finished(None), Mark::PromptStart] {
            reg.mark("a", Mark::Executed, &away());
            reg.tool_event("a", ToolEvent::Port(announced_chip(4000)));
            reg.mark("a", end, &away());
            assert_eq!(chip_ports(&reg, "a"), Vec::<u16>::new());
        }
        // A listener that outlives its command stays until a poll misses it.
        reg.mark("a", Mark::Executed, &away());
        reg.set_listening("a", generation, &[4100]);
        reg.set_listening("a", generation, &[4100]);
        reg.mark("a", Mark::Finished(Some(0)), &away());
        assert_eq!(chip_ports(&reg, "a"), [4100]);
        reg.set_listening("a", generation, &[]);
        assert_eq!(chip_ports(&reg, "a"), Vec::<u16>::new());
        // A shell without marks: the foreground clearing ends the command. While a command runs it doesn't.
        let reg = Registry::new();
        reg.reserve("b", relaunch("B"));
        reg.set_foreground("b", generation_of(&reg, "b"), running("node"));
        reg.tool_event("b", ToolEvent::Port(announced_chip(5000)));
        reg.set_foreground("b", generation_of(&reg, "b"), None);
        assert_eq!(chip_ports(&reg, "b"), Vec::<u16>::new());
        let reg = Registry::new();
        reg.reserve("c", relaunch("C"));
        reg.mark("c", Mark::Executed, &away());
        reg.set_foreground("c", generation_of(&reg, "c"), running("node"));
        reg.tool_event("c", ToolEvent::Port(announced_chip(5000)));
        reg.set_foreground("c", generation_of(&reg, "c"), None);
        assert_eq!(chip_ports(&reg, "c"), [5000]);
        // An exit clears the list, and the exited row takes none.
        let generation = generation_of(&reg, "c");
        reg.set_listening("c", generation, &[6000]);
        reg.set_listening("c", generation, &[6000]);
        assert_eq!(chip_ports(&reg, "c"), [5000, 6000]);
        reg.mark_exited("c", generation, 0);
        assert_eq!(chip_ports(&reg, "c"), Vec::<u16>::new());
        reg.set_listening("c", generation, &[6000]);
        reg.set_listening("c", generation, &[6000]);
        assert_eq!(chip_ports(&reg, "c"), Vec::<u16>::new());
    }

    #[test]
    fn an_announced_and_listening_port_is_one_chip() {
        use crate::tools::ToolEvent;
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        reg.tool_event("a", ToolEvent::Port(announced_chip(3000)));
        reg.set_listening("a", generation, &[3000]);
        reg.set_listening("a", generation, &[3000]);
        assert_eq!(row(&reg, "a").ports, [announced_chip(3000)]);
        // The other way round: the listener's chip first, then the announcement.
        reg.set_listening("a", generation, &[3000, 3001]);
        reg.set_listening("a", generation, &[3000, 3001]);
        reg.tool_event("a", ToolEvent::Port(announced_chip(3001)));
        assert_eq!(chip_ports(&reg, "a"), [3000, 3001]);
    }

    #[test]
    fn the_chip_list_stops_at_eight() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let generation = generation_of(&reg, "a");
        let mut rx = reg.subscribe();
        let all: Vec<u16> = (3000..3012).collect();
        reg.set_listening("a", generation, &all);
        assert_eq!(chip_ports(&reg, "a"), Vec::<u16>::new());
        rx.borrow_and_update();
        reg.set_listening("a", generation, &all);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        let expected: Vec<u16> = (3000..3008).collect();
        assert_eq!(chip_ports(&reg, "a"), expected);
        let first = row(&reg, "a").ports[0].clone();
        assert_eq!(first.url, "http://localhost:3000/");
        assert!(!first.announced);
        // The same reading again changes nothing; a stale generation is ignored.
        reg.set_listening("a", generation, &all);
        reg.set_listening("a", generation + 1, &[]);
        assert!(!rx.has_changed().unwrap());
        assert_eq!(chip_ports(&reg, "a"), expected);
        // A gone port frees a place for one that waited.
        reg.set_listening("a", generation, &[3001, 3008]);
        reg.set_listening("a", generation, &[3001, 3008]);
        assert_eq!(chip_ports(&reg, "a"), [3001, 3008]);
    }

    #[test]
    fn ports_candidates_are_running_unpaused_rows() {
        let reg = Registry::new();
        reg.reserve("shell", relaunch("A"));
        reg.reserve(
            "agent",
            Relaunch {
                agent: Some("claude".into()),
                ..relaunch("B")
            },
        );
        reg.reserve("paused", relaunch("C"));
        reg.reserve("exited", relaunch("D"));
        reg.set_paused("paused", generation_of(&reg, "paused"), true);
        reg.mark_exited("exited", generation_of(&reg, "exited"), 0);
        let ids: Vec<_> = reg.ports_candidates().into_iter().map(|(tab, _)| tab).collect();
        assert_eq!(ids, ["shell", "agent"]);
        assert!(!reg.runs_agent("shell"));
        assert!(reg.runs_agent("agent"));
        assert!(!reg.runs_agent("missing"));
        let codex = Foreground {
            agent: Some("codex".into()),
            ..running("node").unwrap()
        };
        reg.set_foreground("shell", generation_of(&reg, "shell"), Some(codex));
        assert!(reg.runs_agent("shell"));
    }

    #[test]
    fn notify_mark_sets_attention_once() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.mark("a", Mark::Notify(None), &away());
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        let first = row(&reg, "a").attention.expect("attention");
        assert_eq!(first.message, None);
        // A later message fills in the missing one, and the flood keeps the first time.
        reg.mark("a", Mark::Notify(Some("Build done".into())), &away());
        reg.mark("a", Mark::Notify(Some("Other".into())), &away());
        reg.mark("a", Mark::Notify(None), &away());
        let now = row(&reg, "a").attention.expect("attention");
        assert_eq!(now.message.as_deref(), Some("Build done"));
        assert_eq!(now.since_ms, first.since_ms);
        rx.borrow_and_update();
        reg.mark("a", Mark::Notify(Some("Again".into())), &away());
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn bell_counts_only_while_a_command_runs() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::PromptStart, &away());
        reg.mark("a", Mark::Bell, &away());
        assert_eq!(row(&reg, "a").attention, None);
        reg.mark("a", Mark::Executed, &away());
        reg.mark("a", Mark::Bell, &away());
        assert_eq!(row(&reg, "a").attention.expect("attention").message, None);
    }

    #[test]
    fn bell_right_after_typing_is_ignored() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Executed, &away());
        reg.note_input("a");
        reg.mark("a", Mark::Bell, &away());
        assert_eq!(row(&reg, "a").attention, None);
    }

    #[test]
    fn note_input_clears_attention() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Notify(Some("Build done".into())), &away());
        assert!(row(&reg, "a").attention.is_some());
        reg.note_input("a");
        assert_eq!(row(&reg, "a").attention, None);
        reg.note_input("missing");
    }

    #[test]
    fn focused_view_clears_visible_attention() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.reserve("b", relaunch("B"));
        reg.mark("a", Mark::Notify(None), &away());
        reg.mark("b", Mark::Notify(None), &away());
        reg.clear_attention_seen(&UiView {
            window_focused: true,
            visible_tabs: vec!["a".into()],
        });
        assert_eq!(row(&reg, "a").attention, None);
        assert!(row(&reg, "b").attention.is_some());
    }

    #[test]
    fn unfocused_view_keeps_it() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Notify(None), &away());
        reg.clear_attention_seen(&UiView {
            window_focused: false,
            visible_tabs: vec!["a".into()],
        });
        assert!(row(&reg, "a").attention.is_some());
    }

    #[test]
    fn hooked_row_ignores_signals_and_live_status_clears_attention() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.mark("a", Mark::Notify(Some("Build done".into())), &away());
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        assert_eq!(row(&reg, "a").attention, None);
        reg.mark("a", Mark::Notify(Some("Again".into())), &away());
        assert_eq!(row(&reg, "a").attention, None);
    }

    #[test]
    fn update_live_sets_the_status_and_hands_over_the_previous_one() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.update_live("a", |prev| {
            assert!(prev.is_none());
            Some(live(LiveState::Working))
        });
        assert_eq!(row(&reg, "a").live, Some(live(LiveState::Working)));
        reg.update_live("a", |prev| {
            assert_eq!(prev, Some(&live(LiveState::Working)));
            Some(live(LiveState::Done))
        });
        assert_eq!(row(&reg, "a").live, Some(live(LiveState::Done)));
        // A fold that has nothing to say leaves it.
        reg.update_live("a", |_| None);
        assert_eq!(row(&reg, "a").live, Some(live(LiveState::Done)));
    }

    fn fold_claude_event(reg: &Registry, body: serde_json::Value) {
        reg.update_live("a", |prev| {
            crate::agents::claude::status::fold_event(prev, &body, 1_000)
        });
        reg.update_subagents("a", &body);
        reg.follow_subagent_wait("a", &body);
    }

    fn stop_with_running_subagent(reg: &Registry) {
        fold_claude_event(
            reg,
            serde_json::json!({
                "hook_event_name": "Stop", "background_tasks": [
                    { "id": "x", "type": "subagent", "status": "running" }
                ]
            }),
        );
    }

    #[test]
    fn a_subagents_tool_events_keep_the_wait() {
        use crate::agents::claude::status::WAITING_PHRASE;
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        stop_with_running_subagent(&reg);
        fold_claude_event(
            &reg,
            serde_json::json!({
                "hook_event_name": "PreToolUse", "agent_id": "x", "tool_name": "Glob"
            }),
        );
        let waiting = row(&reg, "a").live.unwrap();
        assert_eq!(waiting.state, LiveState::Working);
        assert_eq!(waiting.phrase, WAITING_PHRASE);

        fold_claude_event(
            &reg,
            serde_json::json!({
                "hook_event_name": "PermissionRequest", "agent_id": "x"
            }),
        );
        assert_eq!(row(&reg, "a").live.unwrap().state, LiveState::NeedsPermission);
        fold_claude_event(
            &reg,
            serde_json::json!({
                "hook_event_name": "PostToolUse", "agent_id": "x"
            }),
        );
        let waiting = row(&reg, "a").live.unwrap();
        assert_eq!(waiting.state, LiveState::Working);
        assert_eq!(waiting.phrase, WAITING_PHRASE);

        fold_claude_event(
            &reg,
            serde_json::json!({
                "hook_event_name": "SubagentStop", "agent_id": "x"
            }),
        );
        assert!(reg.awaits_subagents_end("a"));
        assert!(reg.settle_subagent_wait("a"));
        assert_eq!(row(&reg, "a").live.unwrap().state, LiveState::Done);
    }

    #[test]
    fn a_resumed_turn_ends_the_wait() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        stop_with_running_subagent(&reg);
        fold_claude_event(
            &reg,
            serde_json::json!({
                "hook_event_name": "SubagentStop", "agent_id": "x"
            }),
        );
        assert!(reg.awaits_subagents_end("a"));
        fold_claude_event(&reg, serde_json::json!({ "hook_event_name": "UserPromptSubmit" }));
        let thinking = row(&reg, "a").live.unwrap();
        assert_eq!(thinking.state, LiveState::Working);
        assert_eq!(thinking.phrase, "Thinking");
        assert!(!reg.awaits_subagents_end("a"));
        assert!(!reg.settle_subagent_wait("a"));
    }

    #[test]
    fn settling_the_wait_turns_done_only_with_no_subagents_left() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        stop_with_running_subagent(&reg);
        let mut rx = reg.subscribe();
        assert!(!reg.awaits_subagents_end("a"));
        assert!(!reg.settle_subagent_wait("a"));
        assert_eq!(row(&reg, "a").live.unwrap().state, LiveState::Working);
        assert!(!rx.has_changed().unwrap());

        fold_claude_event(
            &reg,
            serde_json::json!({ "hook_event_name": "SubagentStop", "agent_id": "x" }),
        );
        rx.borrow_and_update();
        assert!(reg.awaits_subagents_end("a"));
        assert!(!rx.has_changed().unwrap());
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(reg.settle_subagent_wait("a"));
        let done = row(&reg, "a").live.unwrap();
        assert_eq!(done.state, LiveState::Done);
        assert_eq!(done.phrase, "Done");
        assert!(done.since_ms >= before);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        assert!(!reg.awaits_subagents_end("a"));
        assert!(!reg.settle_subagent_wait("a"));
        assert_eq!(row(&reg, "a").live, Some(done));
        assert!(!rx.has_changed().unwrap());

        let mut thinking = live(LiveState::Working);
        thinking.phrase = "Thinking".to_string();
        reg.update_live("a", |_| Some(thinking.clone()));
        rx.borrow_and_update();
        assert!(!reg.awaits_subagents_end("a"));
        assert!(!reg.settle_subagent_wait("a"));
        assert_eq!(row(&reg, "a").live, Some(thinking));
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn subagent_count_follows_the_running_set_and_clears_with_the_process() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        let start = |id: &str| serde_json::json!({ "hook_event_name": "SubagentStart", "agent_id": id });
        let count = |reg: &Registry| row(reg, "a").live.map(|l| l.subagents);
        reg.update_subagents("a", &start("x"));
        assert_eq!(count(&reg), Some(1));
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        reg.update_subagents("a", &start("y"));
        assert_eq!(count(&reg), Some(2));
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        // A repeat of a known id, or an unknown tab, changes nothing and says nothing.
        reg.update_subagents("a", &start("y"));
        reg.update_subagents("nope", &start("z"));
        assert!(!rx.has_changed().unwrap());
        // A restart clears the set: the next Start counts 1.
        reg.begin_restart("a", SessionMode::Plan).unwrap();
        assert_eq!(count(&reg), None);
        reg.update_subagents("a", &start("w"));
        assert_eq!(count(&reg), Some(1));
        // The process exiting clears it, and an exited row takes no more.
        let current = reg.state.lock().unwrap().open[0].generation;
        reg.mark_exited("a", current, 0);
        assert_eq!(count(&reg), None);
        assert!(reg.state.lock().unwrap().open[0].running.is_empty());
        reg.update_subagents("a", &start("v"));
        assert_eq!(count(&reg), None);
        // Another agent's row takes no count from Claude's hooks.
        reg.reserve(
            "x",
            Relaunch {
                agent: Some("codex".into()),
                ..relaunch("X")
            },
        );
        reg.update_subagents("x", &start("q"));
        assert_eq!(row(&reg, "x").live, None);
    }

    #[test]
    fn finished_subagents_are_counted_only_by_a_subagent_stop_that_removed_one() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let ev = |name: &str, id: Option<&str>| match id {
            Some(id) => serde_json::json!({ "hook_event_name": name, "agent_id": id }),
            None => serde_json::json!({ "hook_event_name": name }),
        };
        let finished = |reg: &Registry| row(reg, "a").live.map(|l| l.subagents_finished);
        reg.update_subagents("a", &ev("SubagentStart", Some("x")));
        reg.update_subagents("a", &ev("SubagentStart", Some("y")));
        assert_eq!(finished(&reg), Some(0));
        reg.update_subagents("a", &ev("SubagentStop", Some("x")));
        assert_eq!(finished(&reg), Some(1));
        // An id that was never running is no subagent finishing.
        reg.update_subagents("a", &ev("SubagentStop", Some("zzz")));
        assert_eq!(finished(&reg), Some(1));
        // The main Stop clears the set (y is not listed as running) and so does SessionEnd: neither counts.
        reg.update_subagents("a", &ev("Stop", None));
        assert_eq!(row(&reg, "a").live.map(|l| l.subagents), Some(0));
        assert_eq!(finished(&reg), Some(1));
        reg.update_subagents("a", &ev("SubagentStart", Some("w")));
        reg.update_subagents("a", &ev("SessionEnd", None));
        assert_eq!(finished(&reg), Some(1));
        // A restart gives the row a fresh status, so the count starts again.
        reg.update_subagents("a", &ev("SubagentStart", Some("v")));
        reg.update_subagents("a", &ev("SubagentStop", Some("v")));
        assert_eq!(finished(&reg), Some(2));
        reg.begin_restart("a", SessionMode::Plan).unwrap();
        assert_eq!(finished(&reg), None);
        reg.update_subagents("a", &ev("SubagentStart", Some("u")));
        assert_eq!(finished(&reg), Some(0));
    }

    fn working_on(phrase: &str) -> LiveStatus {
        let mut working = live(LiveState::Working);
        working.phrase = phrase.to_string();
        working
    }

    fn done_with_recap(text: &str) -> LiveStatus {
        let mut done = live(LiveState::Done);
        done.phrase = "Done".to_string();
        done.since_ms = 5;
        done.recap = Some(marshell_protocol::api::Recap {
            text: text.to_string(),
            at_ms: 5,
        });
        done
    }

    fn reg_working_on_a() -> Registry {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.update_live("a", |_| Some(working_on("Editing a.rs")));
        reg
    }

    fn stuck_repeat(reg: &Registry) {
        for ms in 1..=9 {
            reg.stuck_event("a", StuckSignal::Call("pwd".into()), ms);
        }
        assert_eq!(
            row(reg, "a").stuck.as_ref().unwrap().reason,
            marshell_protocol::api::StuckReason::Repeat
        );
    }

    #[test]
    fn stuck_clears_when_live_leaves_working() {
        for next in [
            LiveState::Done,
            LiveState::NeedsPermission,
            LiveState::NeedsQuestion,
            LiveState::Idle,
            LiveState::Error,
            LiveState::Ended,
        ] {
            let reg = reg_working_on_a();
            stuck_repeat(&reg);
            reg.update_live("a", |_| Some(live(next)));
            assert_eq!(row(&reg, "a").stuck, None);
            reg.stuck_event("a", StuckSignal::Progress, 10);
            reg.check_stuck(1_000_000, 600_000);
            assert_eq!(row(&reg, "a").stuck, None);
            reg.update_live("a", |_| Some(live(LiveState::Working)));
            reg.check_stuck(11, 600_000);
            assert_eq!(row(&reg, "a").stuck, None);
        }
        let reg = reg_working_on_a();
        stuck_repeat(&reg);
        reg.hold_done("a", done_with_recap("Done"));
        reg.settle_done("a");
        assert_eq!(row(&reg, "a").stuck, None);
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        reg.check_stuck(11, 600_000);
        assert_eq!(row(&reg, "a").stuck, None);

        let reg = reg_working_on_a();
        stop_with_running_subagent(&reg);
        stuck_repeat(&reg);
        fold_claude_event(
            &reg,
            serde_json::json!({"hook_event_name": "SubagentStop", "agent_id": "x"}),
        );
        assert!(reg.settle_subagent_wait("a"));
        assert_eq!(row(&reg, "a").stuck, None);
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        reg.check_stuck(11, 600_000);
        assert_eq!(row(&reg, "a").stuck, None);
    }

    #[test]
    fn check_stuck_broadcasts_only_on_change() {
        let reg = reg_working_on_a();
        let mut rx = reg.subscribe();
        reg.stuck_event("a", StuckSignal::Prompt, 100);
        assert!(!rx.has_changed().unwrap());
        reg.check_stuck(600_099, 600_000);
        assert!(!rx.has_changed().unwrap());
        reg.check_stuck(600_100, 600_000);
        assert!(rx.has_changed().unwrap());
        assert_eq!(
            row(&reg, "a").stuck,
            Some(marshell_protocol::api::Stuck {
                reason: marshell_protocol::api::StuckReason::Quiet,
                since_ms: 600_100
            })
        );
        rx.borrow_and_update();
        reg.check_stuck(900_000, 600_000);
        assert!(!rx.has_changed().unwrap());
        reg.stuck_event("a", StuckSignal::Progress, 900_001);
        assert!(rx.has_changed().unwrap());
        assert_eq!(row(&reg, "a").stuck, None);
        stuck_repeat(&reg);
        rx.borrow_and_update();
        reg.check_stuck(1_000_000, 600_000);
        assert!(!rx.has_changed().unwrap());
        reg.stuck_event("a", StuckSignal::Progress, 1_000_001);
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn stuck_tracker_resets_when_session_ends() {
        for end in 0..6 {
            let reg = reg_working_on_a();
            stuck_repeat(&reg);
            match end {
                0 => reg.mark_exited("a", generation_of(&reg, "a"), 0),
                1 => reg.set_state("a", SessionState::Ended),
                2 => reg.set_status_note("a", Some("Status off".into())),
                3 => {
                    reg.begin_restart("a", SessionMode::Manual).unwrap();
                }
                4 => {
                    reg.close("a");
                }
                _ => reg.update_live("a", |_| Some(live(LiveState::Ended))),
            }
            if end == 4 {
                let state = reg.state.lock().unwrap();
                assert_eq!(state.closed[0].info.stuck, None);
                assert_eq!(state.closed[0].stuck.quiet(true, 1_000_000, 600_000), None);
                continue;
            }
            assert_eq!(row(&reg, "a").stuck, None);
            reg.set_state("a", SessionState::Running);
            reg.update_live("a", |_| Some(live(LiveState::Working)));
            reg.check_stuck(1_000_000, 600_000);
            assert_eq!(row(&reg, "a").stuck, None);
            reg.stuck_event("a", StuckSignal::Call("pwd".into()), 10);
            assert_eq!(row(&reg, "a").stuck, None);
        }
    }

    #[test]
    fn held_done_keeps_working_until_settled() {
        let reg = reg_working_on_a();
        let mut rx = reg.subscribe();
        rx.borrow_and_update();

        assert!(reg.hold_done("a", done_with_recap("All done.")));
        let held = row(&reg, "a").live.unwrap();
        assert_eq!((held.state, held.phrase.as_str()), (LiveState::Working, "Editing a.rs"));
        assert_eq!(held.recap, None);
        assert!(!rx.has_changed().unwrap());

        reg.settle_done("a");
        let done = row(&reg, "a").live.unwrap();
        assert_eq!((done.state, done.phrase.as_str()), (LiveState::Done, "Done"));
        assert_eq!(done.recap.unwrap().text, "All done.");
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();

        // Settling again changes nothing.
        reg.settle_done("a");
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn a_held_done_is_only_for_an_open_running_claude_row() {
        let reg = Registry::new();
        assert!(!reg.hold_done("nope", done_with_recap("x")));
        reg.reserve(
            "x",
            Relaunch {
                agent: Some("codex".into()),
                ..relaunch("X")
            },
        );
        assert!(!reg.hold_done("x", done_with_recap("x")));
        reg.reserve("gone", relaunch("G"));
        reg.mark_exited("gone", generation_of(&reg, "gone"), 0);
        assert!(!reg.hold_done("gone", done_with_recap("x")));
    }

    #[test]
    fn newer_event_cancels_held_done() {
        let shown = |reg: &Registry| {
            let live = row(reg, "a").live.unwrap();
            (live.state, live.phrase)
        };
        for (event, state) in [
            (
                serde_json::json!({ "hook_event_name": "UserPromptSubmit" }),
                LiveState::Working,
            ),
            (
                serde_json::json!({ "hook_event_name": "PreToolUse", "tool_name": "Glob" }),
                LiveState::Working,
            ),
            (serde_json::json!({ "hook_event_name": "SessionEnd" }), LiveState::Ended),
        ] {
            let reg = reg_working_on_a();
            assert!(reg.hold_done("a", done_with_recap("All done.")));
            fold_claude_event(&reg, event.clone());
            let after = shown(&reg);
            assert_eq!(after.0, state, "{event}");
            if state == LiveState::Working && event["hook_event_name"] == "UserPromptSubmit" {
                assert_eq!(after.1, "Thinking");
            }
            let mut rx = reg.subscribe();
            rx.borrow_and_update();
            reg.settle_done("a");
            assert_eq!(shown(&reg), after, "{event}");
            assert!(!rx.has_changed().unwrap(), "{event}");
        }
    }

    #[test]
    fn a_status_line_update_does_not_cancel_a_held_done() {
        let reg = reg_working_on_a();
        assert!(reg.hold_done("a", done_with_recap("All done.")));
        let body = serde_json::json!({ "context_window": { "used_percentage": 42 } });
        reg.update_live("a", |prev| {
            Some(crate::agents::claude::status::fold_statusline(prev, &body, 2_000))
        });
        let held = row(&reg, "a").live.unwrap();
        assert_eq!((held.state, held.phrase.as_str()), (LiveState::Working, "Editing a.rs"));
        assert_eq!(held.context_pct, Some(42));

        reg.settle_done("a");
        let done = row(&reg, "a").live.unwrap();
        assert_eq!((done.state, done.phrase.as_str()), (LiveState::Done, "Done"));
        assert_eq!(done.recap.unwrap().text, "All done.");
        // What the status line said meanwhile is kept.
        assert_eq!(done.context_pct, Some(42));
    }

    #[test]
    fn a_process_change_drops_a_held_done() {
        let reg = reg_working_on_a();
        assert!(reg.hold_done("a", done_with_recap("All done.")));
        reg.begin_restart("a", SessionMode::Plan).unwrap();
        assert_eq!(row(&reg, "a").live, None);
        reg.update_live("a", |_| Some(working_on("Thinking")));
        reg.settle_done("a");
        assert_eq!(row(&reg, "a").live.unwrap().state, LiveState::Working);

        let reg = reg_working_on_a();
        assert!(reg.hold_done("a", done_with_recap("All done.")));
        reg.mark_exited("a", generation_of(&reg, "a"), 0);
        reg.settle_done("a");
        assert_eq!(row(&reg, "a").live, None);
    }

    #[test]
    fn update_live_on_an_unknown_or_exited_tab_does_nothing() {
        let reg = Registry::new();
        reg.reserve("gone", relaunch("G"));
        reg.mark_exited("gone", generation_of(&reg, "gone"), 0);
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.update_live("nope", |_| Some(live(LiveState::Working)));
        reg.update_live("gone", |_| Some(live(LiveState::Working)));
        assert!(!rx.has_changed().unwrap());
        assert_eq!(row(&reg, "gone").live, None);
    }

    #[test]
    fn update_live_broadcasts_only_on_a_change() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        assert!(!rx.has_changed().unwrap());
        reg.update_live("a", |_| Some(live(LiveState::Idle)));
        assert!(rx.has_changed().unwrap());
    }

    #[test]
    fn a_status_note_takes_the_live_status_away() {
        // Switching live status off mid-session must not leave the last status showing.
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        reg.update_live("a", |_| Some(live(LiveState::Working)));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.set_status_note("a", Some("off".into()));
        assert!(rx.has_changed().unwrap());
        assert_eq!(row(&reg, "a").live, None);
        // A row that already has the note and no status sends nothing new.
        rx.borrow_and_update();
        reg.set_status_note("a", Some("off".into()));
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn set_status_note_broadcasts_once_and_only_for_a_running_row() {
        let reg = Registry::new();
        reg.reserve("a", relaunch("A"));
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.set_status_note("a", Some("why".into()));
        assert!(rx.has_changed().unwrap());
        assert_eq!(row(&reg, "a").status_note.as_deref(), Some("why"));
        rx.borrow_and_update();
        reg.set_status_note("a", Some("why".into()));
        assert!(!rx.has_changed().unwrap());
        reg.set_status_note("a", None);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();
        reg.set_status_note("a", None);
        reg.set_status_note("nope", Some("why".into()));
        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn exit_restart_resume_and_close_clear_the_live_status() {
        let reg = Registry::new();
        for id in ["exit", "restart", "resume", "close"] {
            reg.reserve(id, relaunch(id));
            reg.update_live(id, |_| Some(live(LiveState::Working)));
            reg.set_status_note(id, Some("why".into()));
        }
        let blank = |info: SessionInfo| (info.live, info.activity, info.status_note);
        reg.mark_exited("exit", generation_of(&reg, "exit"), 0);
        assert_eq!(blank(row(&reg, "exit")), (None, None, None));

        reg.begin_restart("restart", SessionMode::Plan).unwrap();
        assert_eq!(blank(row(&reg, "restart")), (None, None, None));

        // A process that exited without clearing, as a race could leave it, still resumes clean.
        reg.mark_exited("resume", generation_of(&reg, "resume"), 0);
        reg.begin_resume("resume").unwrap();
        assert_eq!(blank(row(&reg, "resume")), (None, None, None));

        reg.close("close");
        assert_eq!(blank(reg.snapshot().closed[0].clone()), (None, None, None));
    }

    fn agent_relaunch(name: &str) -> Relaunch {
        Relaunch {
            agent: Some("claude".into()),
            ..relaunch(name)
        }
    }

    #[test]
    fn set_activity_is_refused_for_a_row_that_cannot_take_it() {
        let reg = Registry::new();
        reg.reserve("live", agent_relaunch("L"));
        reg.reserve("shell", relaunch("S"));
        reg.reserve("old", agent_relaunch("O"));
        reg.reserve("exited", agent_relaunch("E"));
        reg.reserve("paused", agent_relaunch("P"));
        reg.update_live("live", |_| Some(live(LiveState::Working)));
        reg.set_paused("paused", generation_of(&reg, "paused"), true);
        reg.mark_exited("exited", generation_of(&reg, "exited"), 0);
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        for (id, generation) in [
            ("live", generation_of(&reg, "live")),
            ("shell", generation_of(&reg, "shell")),
            ("old", generation_of(&reg, "old") + 1),
            ("exited", generation_of(&reg, "exited")),
            ("paused", generation_of(&reg, "paused")),
            ("nope", 1),
        ] {
            reg.set_activity(id, generation, Some(Activity::Working));
            assert!(!rx.has_changed().unwrap(), "{id} took an activity");
        }
        for id in ["live", "shell", "old", "exited", "paused"] {
            assert_eq!(row(&reg, id).activity, None, "{id}");
        }
    }

    #[test]
    fn set_activity_broadcasts_only_on_a_change() {
        let reg = Registry::new();
        reg.reserve("a", agent_relaunch("A"));
        let generation = generation_of(&reg, "a");
        let mut rx = reg.subscribe();
        rx.borrow_and_update();
        reg.set_activity("a", generation, Some(Activity::Working));
        assert!(rx.has_changed().unwrap());
        assert_eq!(row(&reg, "a").activity, Some(Activity::Working));
        rx.borrow_and_update();
        reg.set_activity("a", generation, Some(Activity::Working));
        assert!(!rx.has_changed().unwrap());
        reg.set_activity("a", generation, Some(Activity::Idle));
        assert!(rx.has_changed().unwrap());
        assert_eq!(row(&reg, "a").activity, Some(Activity::Idle));
        rx.borrow_and_update();
        reg.set_activity("a", generation, None);
        assert!(rx.has_changed().unwrap());
        assert_eq!(row(&reg, "a").activity, None);
    }

    #[test]
    fn exit_and_pause_leave_no_activity() {
        let reg = Registry::new();
        reg.reserve("exit", agent_relaunch("E"));
        reg.reserve("pause", agent_relaunch("P"));
        for id in ["exit", "pause"] {
            reg.set_activity(id, generation_of(&reg, id), Some(Activity::Working));
            assert_eq!(row(&reg, id).activity, Some(Activity::Working));
        }
        reg.mark_exited("exit", generation_of(&reg, "exit"), 0);
        assert_eq!(row(&reg, "exit").activity, None);
        reg.set_paused("pause", generation_of(&reg, "pause"), true);
        assert_eq!(row(&reg, "pause").activity, None);
    }

    #[test]
    fn activity_candidates_are_running_unpaused_agent_rows_without_live_status() {
        let reg = Registry::new();
        reg.reserve("agent", agent_relaunch("A"));
        reg.reserve("live", agent_relaunch("L"));
        reg.reserve("shell", relaunch("S"));
        reg.reserve("paused", agent_relaunch("P"));
        reg.reserve("exited", agent_relaunch("E"));
        reg.update_live("live", |_| Some(live(LiveState::Idle)));
        reg.set_paused("paused", generation_of(&reg, "paused"), true);
        reg.mark_exited("exited", generation_of(&reg, "exited"), 0);
        assert_eq!(
            reg.activity_candidates(),
            vec![("agent".to_string(), generation_of(&reg, "agent"))]
        );
    }

    fn typed(agent: Option<&str>) -> Foreground {
        Foreground {
            program: agent.unwrap_or("vim").into(),
            command_line: None,
            agent: agent.map(Into::into),
            tool: None,
        }
    }

    fn grok_relaunch(name: &str) -> Relaunch {
        Relaunch {
            agent: Some("grok".into()),
            ..relaunch(name)
        }
    }

    #[test]
    fn progress_holds_working_until_cleared() {
        let reg = Registry::new();
        reg.reserve("g", grok_relaunch("G"));
        let generation = generation_of(&reg, "g");
        reg.set_activity("g", generation, Some(Activity::Idle));
        assert_eq!(row(&reg, "g").activity, Some(Activity::Idle));
        // Working and indeterminate progress hold Working, and the quiet tick cannot set Idle.
        for state in [1, 3] {
            reg.mark("g", Mark::Progress(0), &away());
            reg.set_activity("g", generation, Some(Activity::Idle));
            reg.mark("g", Mark::Progress(state), &away());
            assert_eq!(row(&reg, "g").activity, Some(Activity::Working), "state {state}");
            reg.set_activity("g", generation, Some(Activity::Idle));
            assert_eq!(row(&reg, "g").activity, Some(Activity::Working), "state {state} tick");
            reg.set_activity("g", generation, Some(Activity::Working));
            assert_eq!(row(&reg, "g").activity, Some(Activity::Working));
        }
        // Clear, error and paused release the hold, and the ordinary fallback resumes.
        for state in [0, 2, 4] {
            reg.mark("g", Mark::Progress(1), &away());
            reg.mark("g", Mark::Progress(state), &away());
            reg.set_activity("g", generation, Some(Activity::Idle));
            assert_eq!(row(&reg, "g").activity, Some(Activity::Idle), "state {state}");
        }
        // The scanner never yields another state, but a stray one changes nothing.
        reg.mark("g", Mark::Progress(1), &away());
        reg.mark("g", Mark::Progress(9), &away());
        reg.set_activity("g", generation, Some(Activity::Idle));
        assert_eq!(row(&reg, "g").activity, Some(Activity::Working));
    }

    #[test]
    fn progress_is_ignored_by_hook_rows_plain_shells_and_ended_rows() {
        let reg = Registry::new();
        reg.reserve("hook", agent_relaunch("H"));
        reg.reserve("plain", relaunch("P"));
        reg.reserve("exited", grok_relaunch("E"));
        reg.update_live("hook", |_| Some(live(LiveState::Idle)));
        reg.set_foreground("plain", generation_of(&reg, "plain"), Some(typed(None)));
        let before = reg.snapshot();
        for id in ["hook", "plain"] {
            reg.mark(id, Mark::Progress(1), &away());
        }
        assert_eq!(reg.snapshot(), before);
        assert_eq!(row(&reg, "hook").live, Some(live(LiveState::Idle)));
        assert_eq!(row(&reg, "hook").activity, None);
        assert_eq!(row(&reg, "plain").activity, None);
        // A hold from a row that later gets live status or ends is gone with it.
        reg.mark("exited", Mark::Progress(1), &away());
        reg.mark_exited("exited", generation_of(&reg, "exited"), 0);
        assert_eq!(row(&reg, "exited").activity, None);
        reg.begin_resume("exited").unwrap();
        reg.set_activity("exited", generation_of(&reg, "exited"), Some(Activity::Idle));
        assert_eq!(row(&reg, "exited").activity, Some(Activity::Idle));
    }

    #[test]
    fn an_agent_typed_in_a_shell_gets_the_fallback() {
        let reg = Registry::new();
        reg.reserve("typed", relaunch("T"));
        reg.reserve("vim", relaunch("V"));
        reg.reserve("idle", relaunch("I"));
        reg.reserve("paused", relaunch("P"));
        let generation = generation_of(&reg, "typed");
        reg.set_foreground("typed", generation, Some(typed(Some("grok"))));
        reg.set_foreground("vim", generation_of(&reg, "vim"), Some(typed(None)));
        reg.set_foreground("paused", generation_of(&reg, "paused"), Some(typed(Some("grok"))));
        reg.set_paused("paused", generation_of(&reg, "paused"), true);
        assert_eq!(reg.activity_candidates(), vec![("typed".to_string(), generation)]);

        reg.set_activity("typed", generation, Some(Activity::Working));
        assert_eq!(row(&reg, "typed").activity, Some(Activity::Working));
        reg.set_activity("vim", generation_of(&reg, "vim"), Some(Activity::Working));
        reg.set_activity("idle", generation_of(&reg, "idle"), Some(Activity::Working));
        assert_eq!(row(&reg, "vim").activity, None);
        assert_eq!(row(&reg, "idle").activity, None);

        // Progress holds it too, and the prompt coming back takes the activity and the hold away.
        reg.mark("typed", Mark::Progress(1), &away());
        reg.set_activity("typed", generation, Some(Activity::Idle));
        assert_eq!(row(&reg, "typed").activity, Some(Activity::Working));
        reg.mark("typed", Mark::Finished(Some(0)), &away());
        assert_eq!(row(&reg, "typed").foreground, None);
        assert_eq!(row(&reg, "typed").activity, None);
        assert!(reg.activity_candidates().is_empty());
        reg.set_activity("typed", generation, Some(Activity::Idle));
        assert_eq!(row(&reg, "typed").activity, None);
    }
}
