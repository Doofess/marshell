//! Hook progress and repeated tool calls, independent of the session's live state.

use marshell_protocol::api::{Stuck, StuckReason};
use serde_json::Value;
use std::collections::VecDeque;

const WINDOW_MS: u64 = 240_000;

#[derive(Default)]
pub struct StuckTracker {
    calls: VecDeque<(u64, String)>,
    last_event_ms: Option<u64>,
    repeat_since: Option<u64>,
    repeated_key: Option<String>,
}

impl StuckTracker {
    pub fn on_event(&mut self, body: &Value, now_ms: u64) -> Option<Stuck> {
        self.last_event_ms = Some(now_ms);
        self.calls.retain(|(ms, _)| now_ms.saturating_sub(*ms) <= WINDOW_MS);
        match body.get("hook_event_name").and_then(Value::as_str) {
            Some("UserPromptSubmit") => {
                self.calls.clear();
                self.clear_repeat();
            }
            Some("PreToolUse") => {
                let mut key = serde_json::json!([body.get("tool_name"), body.get("tool_input")]);
                key.sort_all_objects();
                let key = key.to_string();
                let different = self.repeated_key.as_ref().is_some_and(|previous| previous != &key);
                self.calls.push_back((now_ms, key.clone()));
                if different {
                    self.clear_repeat();
                } else if self.repeat_since.is_none() && self.calls.iter().filter(|(_, seen)| seen == &key).count() >= 9
                {
                    self.repeat_since = Some(now_ms);
                    self.repeated_key = Some(key);
                }
            }
            _ => {}
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
    use serde_json::{Value, json};

    fn call() -> Value {
        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "pwd"}})
    }

    fn repeat(since_ms: u64) -> Option<Stuck> {
        Some(Stuck {
            reason: StuckReason::Repeat,
            since_ms,
        })
    }

    fn nine(tracker: &mut StuckTracker) {
        for ms in 1..=9 {
            let got = tracker.on_event(&call(), ms);
            assert_eq!(got, if ms == 9 { repeat(9) } else { None });
        }
    }

    #[test]
    fn nine_identical_calls_in_four_minutes_is_stuck() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        assert_eq!(tracker.on_event(&call(), 10), repeat(9));
        assert_eq!(tracker.on_event(&call(), 240_010), repeat(9));
    }

    #[test]
    fn eight_is_not() {
        let mut tracker = StuckTracker::default();
        for ms in 1..=8 {
            assert_eq!(tracker.on_event(&call(), ms), None);
        }
    }

    #[test]
    fn calls_older_than_four_minutes_drop_out() {
        let mut tracker = StuckTracker::default();
        for _ in 0..8 {
            tracker.on_event(&call(), 1);
        }
        assert_eq!(tracker.on_event(&call(), 240_002), None);
        let mut boundary = StuckTracker::default();
        for _ in 0..8 {
            boundary.on_event(&call(), 1);
        }
        assert_eq!(boundary.on_event(&call(), 240_001), repeat(240_001));
    }

    #[test]
    fn post_tool_use_between_repeats_does_not_reset() {
        let mut tracker = StuckTracker::default();
        for ms in 1..=9 {
            let expected = (ms == 9).then_some(Stuck {
                reason: StuckReason::Repeat,
                since_ms: 90,
            });
            assert_eq!(tracker.on_event(&call(), ms * 10), expected);
            for (offset, event) in ["PostToolUse", "Notification", "SubagentStop", "Stop"]
                .iter()
                .enumerate()
            {
                assert_eq!(
                    tracker.on_event(&json!({"hook_event_name": event}), ms * 10 + offset as u64 + 1),
                    expected
                );
            }
        }
    }

    #[test]
    fn different_call_clears() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        let mut other = call();
        other["tool_input"]["command"] = json!("ls");
        assert_eq!(tracker.on_event(&other, 10), None);
        assert_eq!(tracker.on_event(&json!({"hook_event_name": "PostToolUse"}), 11), None);
        assert_eq!(tracker.on_event(&call(), 12), repeat(12));
        other = call();
        other["tool_name"] = json!("OtherTool");
        assert_eq!(tracker.on_event(&other, 13), None);
    }

    #[test]
    fn prompt_clears() {
        let mut tracker = StuckTracker::default();
        nine(&mut tracker);
        assert_eq!(
            tracker.on_event(&json!({"hook_event_name": "UserPromptSubmit"}), 10),
            None
        );
        for ms in 11..=18 {
            assert_eq!(tracker.on_event(&call(), ms), None);
        }
    }

    #[test]
    fn quiet_ten_minutes_while_working_is_stuck() {
        let mut tracker = StuckTracker::default();
        assert_eq!(tracker.quiet(true, 600_100, 600_000), None);
        tracker.on_event(&call(), 100);
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
        tracker.on_event(&call(), 100);
        assert_eq!(tracker.quiet(false, 900_000, 600_000), None);
    }

    #[test]
    fn any_hook_event_is_progress() {
        for event in ["PostToolUse", "Notification", "Stop", "SubagentStop", "Unknown"] {
            let mut tracker = StuckTracker::default();
            tracker.on_event(&call(), 100);
            assert!(tracker.quiet(true, 600_100, 600_000).is_some());
            tracker.on_event(&json!({"hook_event_name": event}), 600_100);
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

    #[test]
    fn key_ignores_json_key_order() {
        let a: Value = serde_json::from_str(
            r#"{"hook_event_name":"PreToolUse","tool_name":"Tool","tool_input":{"b":[{"y":2,"x":1}],"a":0}}"#,
        )
        .unwrap();
        let b: Value = serde_json::from_str(
            r#"{"tool_input":{"a":0,"b":[{"x":1,"y":2}]},"tool_name":"Tool","hook_event_name":"PreToolUse"}"#,
        )
        .unwrap();
        let mut tracker = StuckTracker::default();
        for ms in 1..=8 {
            assert_eq!(tracker.on_event(if ms % 2 == 0 { &a } else { &b }, ms), None);
        }
        assert_eq!(tracker.on_event(&a, 9), repeat(9));
    }
}
