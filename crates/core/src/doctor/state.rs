//! What the Doctor keeps between runs: the last report, whether a run is going, the summary the UI watches and the
//! fix waiting to be applied. A run is the only writer of the report.

use marshell_protocol::doctor::{DoctorReport, DoctorSummary};
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use tokio::sync::watch;

/// The fix whose preview was shown and not yet applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFix {
    pub changeset_id: String,
    pub item_ids: Vec<String>,
    /// Tab ids of the running sessions that pick the fix up only after a restart.
    pub restart: Vec<String>,
}

pub struct DoctorState {
    /// The last run's report; `None` until the first run ends.
    pub report: Mutex<Option<DoctorReport>>,
    /// A run is going. Claimed with a compare-and-swap, so two requests start one run.
    pub running: AtomicBool,
    /// What the sidebar and the events socket show.
    pub summary: watch::Sender<DoctorSummary>,
    pub pending: Mutex<Option<PendingFix>>,
}

impl Default for DoctorState {
    fn default() -> Self {
        Self {
            report: Mutex::new(None),
            running: AtomicBool::new(false),
            summary: watch::channel(DoctorSummary {
                running: false,
                issues: 0,
                checked_at_ms: None,
            })
            .0,
            pending: Mutex::new(None),
        }
    }
}
