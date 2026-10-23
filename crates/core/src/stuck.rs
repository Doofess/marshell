//! Hook progress and repeated tool calls, independent of the session's live state.

use marshell_protocol::api::{Stuck, StuckReason};
use std::collections::VecDeque;

const WINDOW_MS: u64 = 240_000;

/// What one agent event means to the tracker, whichever CLI sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StuckSignal {
    /// One tool call, as a canonical key: the same call gives the same key.
    Call(String),
    /// The person sent a prompt, which clears the history.
    Prompt,
    /// Any other agent event: it only resets the quiet timer.
    Progress,
}

#[derive(Default)]
pub struct StuckTracker {
    calls: VecDeque<(u64, String)>,
    last_event_ms: Option<u64>,
    repeat_since: Option<u64>,
    repeated_key: Option<String>,
}

impl StuckTracker {
    pub fn on_event(&mut self, signal: StuckSignal, now_ms: u64) -> Option<Stuck> {
        self.last_event_ms = Some(now_ms);
        self.calls.retain(|(ms, _)| now_ms.saturating_sub(*ms) <= WINDOW_MS);
        match signal {
            StuckSignal::Prompt => {
                self.calls.clear();
                self.clear_repeat();
            }
            StuckSignal::Call(key) => {
                let different = self.repeated_key.as_ref().is_some_and(|previous| previous != &key);
                self.calls.push_back((now_ms, key.clone()));
                if different {
                    // The old call's entries would trip it again on its next call: it needs nine fresh ones.
                    if let Some(old) = self.repeated_key.take() {
                        self.calls.retain(|(_, seen)| seen != &old);
                    }
                    self.clear_repeat();
                } else if self.repeat_since.is_none() && self.calls.iter().filter(|(_, seen)| seen == &key).count() >= 9
                {
                    self.repeat_since = Some(now_ms);
                    self.repeated_key = Some(key);
                }
            }
            StuckSignal::Progress => {}
        }
        self.repeat()
    }

    pub fn quiet(&self, working: bool, now_ms: u64, quiet_ms: u64) -> Option<Stuck> {
        let last_event_ms = self.last_event_ms?;
        (working && now_ms.saturating_sub(last_event_ms) >= quiet_ms).then_some(Stuck {
            reason: StuckReason::Quiet,
            since_ms: last_event_ms.saturating_add(quiet_ms),
        })
    }

    pub(crate) fn repeat(&self) -> Option<Stuck> {
        self.repeat_since.map(|since_ms| Stuck {
            reason: StuckReason::Repeat,
            since_ms,
        })
    }

    pub(crate) fn clear_repeat(&mut self) {
        self.repeat_since = None;
        self.repeated_key = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{Stuck, StuckReason};

    fn call() -> StuckSignal {
        StuckSignal::Call("pwd".into())
    }

    fn other() -> StuckSignal {
        StuckSignal::Call("ls".into())
    }

    fn repeat(since_ms: u64) -> Option<Stuck> {
        Some(Stuck {
            reason: StuckReason::Repeat,
            since_ms,
        })
    }

    fn nine(tracker: &mut StuckTracker) {
        for ms in 1..=9 {
            let got = tracker.on_event(call(), ms);
            assert_eq!(got, if ms == 9 { repeat(9) } else { None });
        }
    }

    #[test]
    fn nine_identical_calls_in_four_minutes_is_stuck() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        assert_eq!(tracker.on_event(call(), 10), repeat(9));
        assert_eq!(tracker.on_event(call(), 240_010), repeat(9));
    }

    #[test]
    fn eight_is_not() {
        let mut tracker = StuckTracker::default();
        for ms in 1..=8 {
            assert_eq!(tracker.on_event(call(), ms), None);
        }
    }

    #[test]
    fn calls_older_than_four_minutes_drop_out() {
        let mut tracker = StuckTracker::default();
        for _ in 0..8 {
            tracker.on_event(call(), 1);
        }
        assert_eq!(tracker.on_event(call(), 240_002), None);
        let mut boundary = StuckTracker::default();
        for _ in 0..8 {
            boundary.on_event(call(), 1);
        }
        assert_eq!(boundary.on_event(call(), 240_001), repeat(240_001));
    }

    #[test]
    fn progress_between_repeats_does_not_reset() {
        let mut tracker = StuckTracker::default();
        for ms in 1..=9 {
            let expected = (ms == 9).then_some(Stuck {
                reason: StuckReason::Repeat,
                since_ms: 90,
            });
            assert_eq!(tracker.on_event(call(), ms * 10), expected);
            for offset in 1..=4 {
                assert_eq!(tracker.on_event(StuckSignal::Progress, ms * 10 + offset), expected);
            }
        }
    }

    #[test]
    fn different_call_clears() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        assert_eq!(tracker.on_event(other(), 10), None);
        assert_eq!(tracker.on_event(StuckSignal::Progress, 11), None);
        assert_eq!(tracker.on_event(call(), 12), None);
        assert_eq!(tracker.on_event(StuckSignal::Call("other".into()), 13), None);
    }

    #[test]
    fn going_back_to_the_old_call_needs_nine_fresh_repeats() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        assert_eq!(tracker.on_event(other(), 10), None);
        assert_eq!(tracker.on_event(call(), 11), None);
        for ms in 12..=18 {
            assert_eq!(tracker.on_event(call(), ms), None);
        }
        assert_eq!(tracker.on_event(call(), 19), repeat(19));
    }

    #[test]
    fn prompt_clears() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        assert_eq!(tracker.on_event(StuckSignal::Prompt, 10), None);
        for ms in 11..=18 {
            assert_eq!(tracker.on_event(call(), ms), None);
        }
    }

    #[test]
    fn quiet_ten_minutes_while_working_is_stuck() {
        let mut tracker = StuckTracker::default();
        assert_eq!(tracker.quiet(true, 600_100, 600_000), None);
        tracker.on_event(call(), 100);
        assert_eq!(tracker.quiet(true, 600_099, 600_000), None);
        let expected = Some(Stuck {
            reason: StuckReason::Quiet,
            since_ms: 600_100,
        });
        assert_eq!(tracker.quiet(true, 600_100, 600_000), expected);
        assert_eq!(tracker.quiet(true, 900_000, 600_000), expected);
    }

    #[test]
    fn quiet_outside_working_is_not() {
        let mut tracker = StuckTracker::default();
        tracker.on_event(call(), 100);
        assert_eq!(tracker.quiet(false, 900_000, 600_000), None);
    }

    #[test]
    fn any_event_is_progress() {
        for signal in [StuckSignal::Progress, StuckSignal::Prompt, call()] {
            let mut tracker = StuckTracker::default();
            tracker.on_event(call(), 100);
            assert!(tracker.quiet(true, 600_100, 600_000).is_some());
            tracker.on_event(signal, 600_100);
            assert_eq!(tracker.quiet(true, 600_101, 600_000), None);
            assert_eq!(
                tracker.quiet(true, 1_200_100, 600_000),
                Some(Stuck {
                    reason: StuckReason::Quiet,
                    since_ms: 1_200_100
                })
            );
            assert_eq!(tracker.quiet(true, 0, 600_000), None);
        }
    }
}
