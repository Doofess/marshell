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
        engine.set_view(view.borrow_and_update().clone(), std::time::Instant::now());
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            // `changed()` marks the value seen, so a wake by the view is remembered here.
            let mut view_woke = false;
            tokio::select! {
                changed = registry.changed() => if changed.is_err() { return },
                changed = view.changed() => {
                    if changed.is_err() { return }
                    view_woke = true;
                }
                _ = tick.tick() => {}
            }
            let now = std::time::Instant::now();
            if view_woke || view.has_changed().unwrap_or(false) {
                engine.set_view(view.borrow_and_update().clone(), now);
            }
            let snapshot = registry.borrow_and_update().clone();
            let holds = state.holds().views();
            let settings = state.notify_settings();
            let minute = clock::local_minute_of_day();
            let mut requests = engine.observe(&snapshot, &holds, &settings, minute, now);
            requests.extend(engine.tick(&settings, minute, now));
            for request in requests {
                sink(request);
            }
        }
    })
}
