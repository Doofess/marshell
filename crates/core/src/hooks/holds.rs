use marshell_protocol::hook::{PermissionDecision, PermissionView};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::{oneshot, watch};

/// How a held request ended, as the hook handler sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Decided(PermissionDecision),
    Released,
}

/// A request the bridge is waiting on.
#[derive(Debug, Clone, Serialize)]
pub struct PendingPermission {
    pub rid: String,
    pub tab: Option<String>,
    pub agent: String,
    /// The request's `tool_name`.
    pub tool: Option<String>,
    pub at_ms: u64,
    pub request: Value,
}

struct Entry {
    info: PendingPermission,
    /// The card for the UI, built once when the hold opened (it can diff a big edit).
    view: Option<PermissionView>,
    tx: oneshot::Sender<Outcome>,
}

type Map = Mutex<HashMap<String, Entry>>;

fn lock(map: &Map) -> MutexGuard<'_, HashMap<String, Entry>> {
    map.lock().unwrap_or_else(|e| e.into_inner())
}

/// Counts changes to the pending list. Only ever bumped after the map lock is released.
type Changes = watch::Sender<u64>;

fn bump(changes: &Changes) {
    changes.send_modify(|n| *n = n.wrapping_add(1));
}

/// The permission requests being held. Memory only, vendor-neutral.
///
/// Every way an entry ends removes it under the one lock (`decide`, `release`, `release_tab`, the
/// `Hold` guard), and whoever removes it is the only one that can settle it. So of two racing
/// settlements exactly one wins. Each removal (and each open) bumps a change counter that
/// `subscribe` watches, after the lock is released.
pub struct Holds {
    map: Arc<Map>,
    changes: Arc<Changes>,
}

impl Default for Holds {
    fn default() -> Self {
        Self {
            map: Arc::default(),
            changes: Arc::new(watch::channel(0).0),
        }
    }
}

/// Removes the entry when dropped, so a bridge that disconnects (the handler future is dropped)
/// leaves nothing pending.
pub struct Hold {
    map: Arc<Map>,
    changes: Arc<Changes>,
    rid: String,
}

impl Drop for Hold {
    fn drop(&mut self) {
        // A request that was decided or released is gone already, and that bumped the counter.
        let removed = lock(&self.map).remove(&self.rid).is_some();
        if removed {
            bump(&self.changes);
        }
    }
}

fn new_rid() -> String {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).expect("the OS random number generator failed");
    hex::encode(buf)
}

impl Holds {
    pub fn open(&self, tab: Option<String>, agent: String, request: Value) -> (Hold, oneshot::Receiver<Outcome>) {
        let rid = new_rid();
        let (tx, rx) = oneshot::channel();
        let at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let tool = request.get("tool_name").and_then(Value::as_str).map(str::to_string);
        let info = PendingPermission {
            rid: rid.clone(),
            tab,
            agent,
            tool,
            at_ms,
            request,
        };
        // Before the lock: building a view can diff a large edit.
        let view = crate::agents::permission_view(&info);
        lock(&self.map).insert(rid.clone(), Entry { info, view, tx });
        bump(&self.changes);
        (
            Hold {
                map: self.map.clone(),
                changes: self.changes.clone(),
                rid,
            },
            rx,
        )
    }

    /// A counter that changes whenever the pending list does: on open, decide, release,
    /// release_tab and when a hold is dropped.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    /// Settles the request with a decision. False when the rid is unknown or already settled.
    pub fn decide(&self, rid: &str, decision: PermissionDecision) -> bool {
        self.decide_and_update(rid, decision, |_, _| {})
    }

    /// Updates the tab after a winning decision, before publishing the removal. The callback
    /// receives whether another request for the same tab is waiting. It runs under the map lock
    /// and must not access holds.
    pub(crate) fn decide_and_update(
        &self,
        rid: &str,
        decision: PermissionDecision,
        update: impl FnOnce(&PendingPermission, bool),
    ) -> bool {
        let answered = {
            let mut map = lock(&self.map);
            let Some(entry) = map.remove(rid) else {
                return false;
            };
            let answered = entry.tx.send(Outcome::Decided(decision)).is_ok();
            if answered {
                let waiting = map.values().any(|e| e.info.tab == entry.info.tab);
                update(&entry.info, waiting);
            }
            answered
        };
        bump(&self.changes);
        answered
    }

    /// Lets the agent ask the person itself. False when the rid is unknown or already settled.
    pub fn release(&self, rid: &str) -> bool {
        self.settle(rid, Outcome::Released)
    }

    fn settle(&self, rid: &str, outcome: Outcome) -> bool {
        let entry = lock(&self.map).remove(rid);
        let Some(entry) = entry else {
            return false;
        };
        bump(&self.changes);
        // A closed receiver means the handler is already gone: nobody was answered.
        entry.tx.send(outcome).is_ok()
    }

    /// Releases every request of the tab. Cheap when nothing is pending.
    pub fn release_tab(&self, tab: &str) {
        let taken: Vec<Entry> = {
            let mut map = lock(&self.map);
            if map.is_empty() {
                return;
            }
            let rids: Vec<String> = map
                .iter()
                .filter(|(_, e)| e.info.tab.as_deref() == Some(tab))
                .map(|(rid, _)| rid.clone())
                .collect();
            rids.iter().filter_map(|rid| map.remove(rid)).collect()
        };
        if taken.is_empty() {
            return;
        }
        bump(&self.changes);
        for e in taken {
            let _ = e.tx.send(Outcome::Released);
        }
    }

    /// One request, for validating a decision before it settles.
    pub fn get(&self, rid: &str) -> Option<PendingPermission> {
        lock(&self.map).get(rid).map(|e| e.info.clone())
    }

    /// Oldest first.
    pub fn pending(&self) -> Vec<PendingPermission> {
        let mut all: Vec<PendingPermission> = lock(&self.map).values().map(|e| e.info.clone()).collect();
        all.sort_by(|a, b| a.at_ms.cmp(&b.at_ms).then_with(|| a.rid.cmp(&b.rid)));
        all
    }

    /// The cards of the pending requests that have one, oldest first. Clones under the lock and
    /// returns, so the caller can send them without holding it.
    pub fn views(&self) -> Vec<PermissionView> {
        let mut all: Vec<(u64, String, PermissionView)> = lock(&self.map)
            .values()
            .filter_map(|e| Some((e.info.at_ms, e.info.rid.clone(), e.view.clone()?)))
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        all.into_iter().map(|(_, _, v)| v).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        json!({"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"npm test"}})
    }

    #[test]
    fn holds_subscribe_bumps_on_drop() {
        let holds = Holds::default();
        let mut rx = holds.subscribe();
        let start = *rx.borrow_and_update();

        let (hold, _rx) = holds.open(Some("t1".into()), "claude".into(), request());
        let opened = *rx.borrow_and_update();
        assert!(opened > start, "open did not bump");

        // The handler going away (the bridge left, or the hold timed out) is a change too.
        drop(hold);
        assert!(*rx.borrow_and_update() > opened, "drop did not bump");
        assert!(holds.pending().is_empty());
    }

    #[test]
    fn holds_subscribe_bumps_once_per_change() {
        let holds = Holds::default();
        let mut rx = holds.subscribe();
        let (hold, _r) = holds.open(Some("t1".into()), "claude".into(), request());
        let rid = holds.pending()[0].rid.clone();
        let opened = *rx.borrow_and_update();

        assert!(holds.release(&rid));
        let released = *rx.borrow_and_update();
        assert!(released > opened);
        // The guard dropping after the request was already settled changes nothing more.
        drop(hold);
        assert_eq!(*rx.borrow_and_update(), released);

        let (_h, _r) = holds.open(Some("t2".into()), "claude".into(), request());
        let before = *rx.borrow_and_update();
        holds.release_tab("t2");
        assert!(*rx.borrow_and_update() > before, "release_tab did not bump");
        // Nothing pending: no bump.
        let idle = *rx.borrow_and_update();
        holds.release_tab("t2");
        assert_eq!(*rx.borrow_and_update(), idle);
    }

    #[test]
    fn views_are_built_at_open_and_oldest_first() {
        let holds = Holds::default();
        let (a, _ra) = holds.open(Some("t1".into()), "claude".into(), request());
        let (_b, _rb) = holds.open(None, "claude".into(), request());
        let (_c, _rc) = holds.open(Some("t2".into()), "codex".into(), request());
        let (d, _rd) = holds.open(Some("t3".into()), "claude".into(), request());
        let (e, _re) = holds.open(Some("t4".into()), "claude".into(), request());
        {
            let mut map = lock(&holds.map);
            // Fixed, distinct ages make the order independent of clock resolution and hash order.
            for (hold, at_ms) in [(&a, 20), (&d, 10), (&e, 30)] {
                let entry = map.get_mut(&hold.rid).unwrap();
                entry.info.at_ms = at_ms;
                entry.view.as_mut().unwrap().at_ms = at_ms;
                entry.info.request["tool_input"]["command"] = json!("changed after opening");
            }
        }
        // No tab and a non-claude agent have no card.
        let views = holds.views();
        assert_eq!(
            views.iter().map(|v| v.tab.as_str()).collect::<Vec<_>>(),
            ["t3", "t1", "t4"]
        );
        for view in views {
            assert!(matches!(
                view.detail,
                marshell_protocol::hook::PermissionDetail::Bash { command, .. } if command == "npm test"
            ));
        }
    }
}
