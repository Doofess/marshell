use marshell_protocol::hook::PermissionDecision;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::oneshot;

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
    tx: oneshot::Sender<Outcome>,
}

type Map = Mutex<HashMap<String, Entry>>;

fn lock(map: &Map) -> MutexGuard<'_, HashMap<String, Entry>> {
    map.lock().unwrap_or_else(|e| e.into_inner())
}

/// The permission requests being held. Memory only, vendor-neutral.
///
/// Every way an entry ends removes it under the one lock (`decide`, `release`, `release_tab`, the
/// `Hold` guard), and whoever removes it is the only one that can settle it. So of two racing
/// settlements exactly one wins.
#[derive(Default)]
pub struct Holds(Arc<Map>);

/// Removes the entry when dropped, so a bridge that disconnects (the handler future is dropped)
/// leaves nothing pending.
pub struct Hold {
    map: Arc<Map>,
    rid: String,
}

impl Drop for Hold {
    fn drop(&mut self) {
        lock(&self.map).remove(&self.rid);
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
        lock(&self.0).insert(rid.clone(), Entry { info, tx });
        (
            Hold {
                map: self.0.clone(),
                rid,
            },
            rx,
        )
    }

    /// Settles the request with a decision. False when the rid is unknown or already settled.
    pub fn decide(&self, rid: &str, decision: PermissionDecision) -> bool {
        self.settle(rid, Outcome::Decided(decision))
    }

    /// Lets the agent ask the person itself. False when the rid is unknown or already settled.
    pub fn release(&self, rid: &str) -> bool {
        self.settle(rid, Outcome::Released)
    }

    fn settle(&self, rid: &str, outcome: Outcome) -> bool {
        let entry = lock(&self.0).remove(rid);
        // A closed receiver means the handler is already gone: nobody was answered.
        entry.is_some_and(|e| e.tx.send(outcome).is_ok())
    }

    /// Releases every request of the tab. Cheap when nothing is pending.
    pub fn release_tab(&self, tab: &str) {
        let taken: Vec<Entry> = {
            let mut map = lock(&self.0);
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
        for e in taken {
            let _ = e.tx.send(Outcome::Released);
        }
    }

    /// One request, for validating a decision before it settles.
    pub fn get(&self, rid: &str) -> Option<PendingPermission> {
        lock(&self.0).get(rid).map(|e| e.info.clone())
    }

    /// Oldest first.
    pub fn pending(&self) -> Vec<PendingPermission> {
        let mut all: Vec<PendingPermission> = lock(&self.0).values().map(|e| e.info.clone()).collect();
        all.sort_by(|a, b| a.at_ms.cmp(&b.at_ms).then_with(|| a.rid.cmp(&b.rid)));
        all
    }
}
