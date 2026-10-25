//! The activity watcher: the fallback for an agent row that has no live status, launched as an agent
//! or typed into a shell. Once a second it looks at how much output each such session's pty has
//! produced and puts Working (output in the last 2 s) or Idle (quieter than that) in the registry as
//! `SessionInfo.activity`. A program that reports OSC 9;4 progress holds Working in the registry,
//! which turns the watcher's Idle into Working until the program clears it.
//!
//! It reads the stream end that `Session::counters` keeps, so there is no extra reader thread.

use super::AppState;
use marshell_protocol::api::Activity;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often the sessions are looked at.
const POLL: Duration = Duration::from_secs(1);

/// How long without output before a session counts as Idle.
const QUIET: Duration = Duration::from_secs(2);

/// What the watcher last saw of one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    /// The process the reading belongs to: a restart starts over.
    generation: u64,
    /// The pty stream end at the last change.
    last_end: u64,
    /// When the stream end was last seen to grow, or the session was first seen.
    last_change: Instant,
}

/// Starts the watcher on the current runtime. Does nothing outside one.
pub(super) fn spawn_watcher(state: &AppState) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    // A weak handle, so this task does not keep a dropped core alive.
    let inner = Arc::downgrade(&state.0);
    runtime.spawn(async move {
        let mut seen: HashMap<String, Seen> = HashMap::new();
        loop {
            tokio::time::sleep(POLL).await;
            let Some(inner) = inner.upgrade() else {
                return;
            };
            tick(&AppState(inner), &mut seen, Instant::now());
        }
    });
}

/// One pass. The registry and the sessions map are locked only for the moment of each read and
/// each write, and the reads are counter reads, so nothing here blocks.
fn tick(st: &AppState, seen: &mut HashMap<String, Seen>, now: Instant) {
    let candidates = st.0.registry.activity_candidates();
    // A tab that is no longer a candidate (paused, ended, has live status) is forgotten, so it
    // starts over when it comes back.
    seen.retain(|tab, _| candidates.iter().any(|(t, _)| t == tab));
    for (tab, generation) in candidates {
        let Some(session) = st.session(&tab) else {
            continue;
        };
        if session.has_exited() {
            continue;
        }
        let (end, _) = session.counters();
        let now_seen = step(seen.get(&tab).copied(), generation, end, now);
        seen.insert(tab.clone(), now_seen);
        st.0.registry
            .set_activity(&tab, generation, Some(activity_at(now_seen.last_change, now)));
    }
}

/// Folds one reading into what was seen before. The first sighting, a new generation and a grown
/// stream end all count as a change at `now`.
fn step(previous: Option<Seen>, generation: u64, end: u64, now: Instant) -> Seen {
    match previous {
        Some(seen) if seen.generation == generation && seen.last_end == end => seen,
        _ => Seen {
            generation,
            last_end: end,
            last_change: now,
        },
    }
}

/// Working while output came within `QUIET`, Idle once it has been quieter than that.
fn activity_at(last_change: Instant, now: Instant) -> Activity {
    if now.saturating_duration_since(last_change) < QUIET {
        Activity::Working
    } else {
        Activity::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_within_two_seconds_is_working_and_quiet_for_two_is_idle() {
        let t0 = Instant::now();
        assert_eq!(activity_at(t0, t0), Activity::Working);
        assert_eq!(activity_at(t0, t0 + Duration::from_millis(1999)), Activity::Working);
        assert_eq!(activity_at(t0, t0 + Duration::from_secs(2)), Activity::Idle);
        assert_eq!(activity_at(t0, t0 + Duration::from_secs(60)), Activity::Idle);
    }

    #[test]
    fn the_first_sighting_counts_as_a_change() {
        let t0 = Instant::now();
        let seen = step(None, 1, 500, t0);
        assert_eq!(seen.last_change, t0);
        assert_eq!(activity_at(seen.last_change, t0), Activity::Working);
    }

    #[test]
    fn an_unchanged_end_keeps_the_last_change_and_a_growing_end_resets_it() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let first = step(None, 1, 500, t0);
        let quiet = step(Some(first), 1, 500, at(3));
        assert_eq!(quiet.last_change, t0);
        assert_eq!(activity_at(quiet.last_change, at(3)), Activity::Idle);
        let grown = step(Some(quiet), 1, 640, at(4));
        assert_eq!(grown.last_change, at(4));
        assert_eq!(grown.last_end, 640);
        assert_eq!(activity_at(grown.last_change, at(4)), Activity::Working);
    }

    #[test]
    fn a_new_generation_for_the_same_tab_starts_over() {
        let t0 = Instant::now();
        let first = step(None, 1, 500, t0);
        // The restarted process has the same stream end by chance: still a fresh start.
        let later = t0 + Duration::from_secs(10);
        let restarted = step(Some(first), 2, 500, later);
        assert_eq!(restarted.generation, 2);
        assert_eq!(restarted.last_change, later);
        assert_eq!(activity_at(restarted.last_change, later), Activity::Working);
    }
}
