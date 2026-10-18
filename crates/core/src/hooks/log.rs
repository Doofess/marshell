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
    pub fn push(&self, rec: HookEventRecord, live: impl Fn(&str) -> bool) {
        let mut rings = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match rec.tab.clone() {
            Some(tab) if live(&tab) => push_capped(rings.by_tab.entry(tab).or_default(), rec),
            _ => push_capped(&mut rings.untabbed, rec),
        }
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
