use marshell_protocol::hook::HookRole;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Records kept per tab, and again for the untabbed ones.
pub const HOOK_LOG_CAP: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct HookEventRecord {
    pub at_ms: u64,
    pub role: HookRole,
    pub agent: String,
    pub tab: Option<String>,
    pub bridge_version: String,
    pub body: serde_json::Value,
}

#[derive(Default)]
struct Rings {
    by_tab: HashMap<String, VecDeque<HookEventRecord>>,
    untabbed: VecDeque<HookEventRecord>,
    /// The newest status line per tab. The bridge sends one at every refresh, which would flood a ring.
    statusline_by_tab: HashMap<String, HookEventRecord>,
    statusline_untabbed: Option<HookEventRecord>,
}

/// The last events the bridge sent, per live tab. Memory only.
#[derive(Default)]
pub struct HookLog(Mutex<Rings>);

fn push_capped(ring: &mut VecDeque<HookEventRecord>, rec: HookEventRecord) {
    if ring.len() >= HOOK_LOG_CAP {
        ring.pop_front();
    }
    ring.push_back(rec);
}

impl HookLog {
    /// Files the record under its tab when `live` says the tab is a live session, else with the untabbed ones.
    /// A status line replaces the one before it instead of joining a ring.
    pub fn push(&self, rec: HookEventRecord, live: impl Fn(&str) -> bool) {
        let mut rings = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let statusline = rec.role == HookRole::Statusline;
        match rec.tab.clone() {
            Some(tab) if live(&tab) => {
                if statusline {
                    rings.statusline_by_tab.insert(tab, rec);
                } else {
                    push_capped(rings.by_tab.entry(tab).or_default(), rec);
                }
            }
            _ if statusline => rings.statusline_untabbed = Some(rec),
            _ => push_capped(&mut rings.untabbed, rec),
        }
    }

    /// The newest status line. `None` is the untabbed slot.
    pub fn latest_statusline(&self, tab: Option<&str>) -> Option<HookEventRecord> {
        let rings = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match tab {
            Some(t) => rings.statusline_by_tab.get(t).cloned(),
            None => rings.statusline_untabbed.clone(),
        }
    }

    /// Drops the tab's ring and its status line, for a tab that is gone.
    pub fn forget_tab(&self, tab: &str) {
        let mut rings = self.0.lock().unwrap_or_else(|e| e.into_inner());
        rings.by_tab.remove(tab);
        rings.statusline_by_tab.remove(tab);
    }

    /// Oldest first. `None` is the untabbed ring.
    pub fn recent(&self, tab: Option<&str>) -> Vec<HookEventRecord> {
        let rings = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match tab {
            Some(t) => rings
                .by_tab
                .get(t)
                .map(|r| r.iter().cloned().collect())
                .unwrap_or_default(),
            None => rings.untabbed.iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(role: HookRole, tab: Option<&str>, n: u32) -> HookEventRecord {
        HookEventRecord {
            at_ms: n as u64,
            role,
            agent: "claude".into(),
            tab: tab.map(str::to_string),
            bridge_version: "0".into(),
            body: serde_json::json!({ "n": n }),
        }
    }

    #[test]
    fn status_lines_keep_only_the_latest() {
        let log = HookLog::default();
        for n in 1..=300 {
            log.push(rec(HookRole::Statusline, Some("t1"), n), |_| true);
            log.push(rec(HookRole::Statusline, None, n), |_| true);
        }
        assert!(log.recent(Some("t1")).is_empty());
        assert!(log.recent(None).is_empty());
        assert_eq!(log.latest_statusline(Some("t1")).unwrap().body["n"], 300);
        assert_eq!(log.latest_statusline(None).unwrap().body["n"], 300);
        assert!(log.latest_statusline(Some("t2")).is_none());
    }

    #[test]
    fn a_status_line_for_a_dead_tab_is_untabbed() {
        let log = HookLog::default();
        log.push(rec(HookRole::Statusline, Some("gone"), 1), |_| false);
        assert!(log.latest_statusline(Some("gone")).is_none());
        assert_eq!(log.latest_statusline(None).unwrap().body["n"], 1);
    }

    #[test]
    fn other_events_still_fill_the_ring() {
        let log = HookLog::default();
        log.push(rec(HookRole::Event, Some("t1"), 1), |_| true);
        assert_eq!(log.recent(Some("t1")).len(), 1);
    }

    #[test]
    fn forget_tab_drops_ring_and_status_line() {
        let log = HookLog::default();
        log.push(rec(HookRole::Event, Some("t1"), 1), |_| true);
        log.push(rec(HookRole::Statusline, Some("t1"), 2), |_| true);
        log.push(rec(HookRole::Event, Some("t2"), 3), |_| true);
        log.forget_tab("t1");
        assert!(log.recent(Some("t1")).is_empty());
        assert!(log.latest_statusline(Some("t1")).is_none());
        assert_eq!(log.recent(Some("t2")).len(), 1);
    }
}
