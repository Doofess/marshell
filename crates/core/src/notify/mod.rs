//! Decides when Marshell tells the user something: an OS banner, a sound, a taskbar flash. The core decides
//! (`engine`) and words the banners (`copy`); the app shell shows and plays what it is handed.

pub mod clock;
pub mod copy;
pub mod engine;

use serde::Serialize;
use std::sync::Arc;

/// Which sound the app plays. Each is distinct, so the sound alone says what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundKind {
    NeedsYou,
    Escalation,
    Done,
    SubagentDone,
    Error,
}

/// What clicking a banner does: open the session, or open it at the request to look at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BannerAction {
    Open,
    Review,
}

/// An OS banner. The text never shows a command that a person must look at in the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Banner {
    pub title: String,
    pub body: String,
    pub meta: String,
    pub action: BannerAction,
    /// The session a click opens. None for a batch: a click opens the window only.
    pub tab_id: Option<String>,
}

/// One thing to tell the user. Any part may be absent: a muted sound with a banner, a flash alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotifyRequest {
    pub banner: Option<Banner>,
    pub sound: Option<SoundKind>,
    pub flash: bool,
}

/// Where the core hands what it decided. Called from the notifier's task: it must not block.
pub type NotifySink = Arc<dyn Fn(NotifyRequest) + Send + Sync>;

/// How often the notifier looks even when nothing changed: batch windows, held permissions and
/// reminders end by time, not by an event.
const TICK: std::time::Duration = std::time::Duration::from_millis(250);

/// Starts the notifier on the current runtime: it watches the sessions, the UI's view and the clock,
/// and calls `sink` for each thing the engine decides. The settings come from the core's memory, never
/// the disk. Ended by aborting the returned handle.
pub fn spawn(state: crate::server::AppState, sink: NotifySink) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut registry = state.registry().subscribe();
        let mut view = state.view();
        let mut engine = engine::Engine::new();
        engine.set_view(state.effective_view(), std::time::Instant::now());
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                changed = registry.changed() => if changed.is_err() { return },
                changed = view.changed() => if changed.is_err() { return },
                _ = tick.tick() => {}
            }
            let now = std::time::Instant::now();
            view.borrow_and_update();
            let snapshot = registry.borrow_and_update().clone();
            let minute = clock::local_minute_of_day();
            for request in poll(&state, &mut engine, &snapshot, minute, now) {
                sink(request);
            }
        }
    })
}

fn poll(
    state: &crate::server::AppState,
    engine: &mut engine::Engine,
    snapshot: &marshell_protocol::api::SessionsSnapshot,
    minute: u16,
    now: std::time::Instant,
) -> Vec<NotifyRequest> {
    // Check the shell's flag on every tick, even when the webview reports nothing new.
    engine.set_view(state.effective_view(), now);
    let holds = state.holds().views();
    let settings = state.notify_settings();
    let mut requests = engine.observe(snapshot, &holds, &settings, minute, now);
    requests.extend(engine.tick(&settings, minute, now));
    if state.notify_paused() {
        engine.discard_pending();
        requests.clear();
    }
    requests
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::AppState;
    use marshell_protocol::api::{
        CommandState, LiveState, LiveStatus, SessionInfo, SessionState, SessionsSnapshot, UiView,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    fn snapshot(state: LiveState, since_ms: u64) -> SessionsSnapshot {
        SessionsSnapshot {
            sessions: vec![SessionInfo {
                tab_id: "t1".into(),
                number: 1,
                name: "s1".into(),
                agent: Some("claude".into()),
                profile_id: "p".into(),
                cwd: String::new(),
                project: String::new(),
                mode: None,
                state: SessionState::Running,
                command: CommandState::Unknown,
                paused: false,
                foreground: None,
                activity: None,
                status_note: None,
                muted: false,
                attention: None,
                stuck: None,
                live: Some(LiveStatus {
                    state,
                    phrase: String::new(),
                    since_ms,
                    recap: None,
                    model: None,
                    effort: None,
                    context_pct: None,
                    five_hour_pct: None,
                    five_hour_resets_at: None,
                    seven_day_pct: None,
                    subagents: 0,
                    subagents_finished: 0,
                }),
                preset: None,
                ports: Vec::new(),
            }],
            closed: Vec::new(),
        }
    }

    #[test]
    fn nothing_is_delivered_while_paused_and_nothing_replays_after_resume() {
        let home = tempfile::tempdir().unwrap();
        let paused = Arc::new(AtomicBool::new(true));
        let state = AppState::new("t".into(), 1, vec![], home.path().into(), vec![])
            .with_notification_flags(paused.clone(), Default::default());
        let start = Instant::now();
        let mut engine = engine::Engine::starting_at(start);
        let at = |secs| start + Duration::from_secs(secs);
        assert!(poll(&state, &mut engine, &snapshot(LiveState::Working, 1), 720, at(0)).is_empty());
        let wait = snapshot(LiveState::NeedsPermission, 2);
        assert!(poll(&state, &mut engine, &wait, 720, at(1)).is_empty());
        // The permission's hold-wait deadline and batch window both expire during the pause.
        assert!(poll(&state, &mut engine, &wait, 720, at(2)).is_empty());
        assert!(poll(&state, &mut engine, &wait, 720, at(6)).is_empty());
        paused.store(false, Ordering::SeqCst);
        assert!(poll(&state, &mut engine, &wait, 720, at(7)).is_empty());
        // State kept advancing: the still-open wait can get its one-minute reminder.
        let reminder = poll(&state, &mut engine, &wait, 720, at(63));
        assert_eq!(reminder.len(), 1);
        assert_eq!(reminder[0].sound, Some(SoundKind::Escalation));
        poll(&state, &mut engine, &snapshot(LiveState::Working, 3), 720, at(64));
        let fresh = poll(&state, &mut engine, &snapshot(LiveState::NeedsQuestion, 4), 720, at(67));
        assert_eq!(fresh.len(), 1);
        assert!(fresh[0].banner.is_some());
        assert_eq!(fresh[0].sound, Some(SoundKind::NeedsYou));
        assert!(fresh[0].flash);
    }

    #[test]
    fn resuming_before_a_permission_deadline_does_not_replay_it() {
        let home = tempfile::tempdir().unwrap();
        let paused = Arc::new(AtomicBool::new(true));
        let state = AppState::new("t".into(), 1, vec![], home.path().into(), vec![])
            .with_notification_flags(paused.clone(), Default::default());
        let start = Instant::now();
        let mut engine = engine::Engine::starting_at(start);
        poll(&state, &mut engine, &snapshot(LiveState::Working, 1), 720, start);
        let wait = snapshot(LiveState::NeedsPermission, 2);
        assert!(poll(&state, &mut engine, &wait, 720, start + Duration::from_secs(1)).is_empty());
        paused.store(false, Ordering::SeqCst);
        assert!(poll(&state, &mut engine, &wait, 720, start + Duration::from_secs(2)).is_empty());
        let reminder = poll(&state, &mut engine, &wait, 720, start + Duration::from_secs(63));
        assert_eq!(reminder.len(), 1);
        assert_eq!(reminder[0].sound, Some(SoundKind::Escalation));
    }

    #[test]
    fn resuming_before_a_batch_deadline_does_not_replay_it() {
        let home = tempfile::tempdir().unwrap();
        let paused = Arc::new(AtomicBool::new(true));
        let state = AppState::new("t".into(), 1, vec![], home.path().into(), vec![])
            .with_notification_flags(paused.clone(), Default::default());
        let start = Instant::now();
        let mut engine = engine::Engine::starting_at(start);
        let mut snap = snapshot(LiveState::Working, 1);
        let mut second = snap.sessions[0].clone();
        second.tab_id = "t2".into();
        second.number = 2;
        snap.sessions.push(second);
        poll(&state, &mut engine, &snap, 720, start);
        for row in &mut snap.sessions {
            let live = row.live.as_mut().unwrap();
            live.state = LiveState::NeedsQuestion;
            live.since_ms = 2;
        }
        assert!(poll(&state, &mut engine, &snap, 720, start + Duration::from_secs(1)).is_empty());
        paused.store(false, Ordering::SeqCst);
        assert!(poll(&state, &mut engine, &snap, 720, start + Duration::from_secs(5)).is_empty());
    }

    #[test]
    fn a_view_report_clears_seen_attention_unless_the_window_is_hidden() {
        let home = tempfile::tempdir().unwrap();
        let state = AppState::new("t".into(), 1, vec![], home.path().into(), vec![]);
        state.registry().reserve(
            "t1",
            crate::registry::Relaunch {
                profile_id: "pwsh".into(),
                agent: None,
                cwd: "C:/work".into(),
                mode: None,
                name: "A".into(),
                preset: None,
            },
        );
        let view = UiView {
            window_focused: true,
            visible_tabs: vec!["t1".into()],
        };
        let attention = || state.registry().snapshot().sessions[0].attention.clone();
        state
            .registry()
            .mark("t1", crate::pty::Mark::Notify(None), &state.effective_view());
        state.set_window_hidden(true);
        state.report_view(view.clone());
        assert!(attention().is_some());
        state.set_window_hidden(false);
        state.report_view(view);
        assert_eq!(attention(), None);
    }

    #[test]
    fn a_hidden_window_has_no_tab_in_view() {
        let home = tempfile::tempdir().unwrap();
        let state = AppState::new("t".into(), 1, vec![], home.path().into(), vec![]);
        let view = UiView {
            window_focused: true,
            visible_tabs: vec!["t1".into()],
        };
        state.report_view(view.clone());
        let start = Instant::now();
        let at = |secs| start + Duration::from_secs(secs);
        let mut engine = engine::Engine::starting_at(start);
        let working = snapshot(LiveState::Working, 1);
        assert!(poll(&state, &mut engine, &working, 720, at(0)).is_empty());
        state.set_window_hidden(true);
        assert!(poll(&state, &mut engine, &working, 720, at(1)).is_empty());
        state.report_view(view.clone());
        assert!(!state.effective_view().window_focused);
        assert!(state.effective_view().visible_tabs.is_empty());
        assert!(poll(&state, &mut engine, &working, 720, at(20)).is_empty());
        let done = poll(&state, &mut engine, &snapshot(LiveState::Done, 2), 720, at(32));
        assert_eq!(done.len(), 1);
        assert!(done[0].banner.is_some());
        assert_eq!(done[0].sound, Some(SoundKind::Done));
        state.set_window_hidden(false);
        assert_eq!(state.effective_view(), view);
        assert!(poll(&state, &mut engine, &snapshot(LiveState::Working, 3), 720, at(33)).is_empty());
        assert!(poll(&state, &mut engine, &snapshot(LiveState::Done, 4), 720, at(64)).is_empty());
    }
}
