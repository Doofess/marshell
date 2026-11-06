//! What the Doctor keeps between runs: the last report, whether a run is going, the summary the UI watches and the
//! fix waiting to be applied. A run is the only writer of the report.

use super::measure::Measurements;
use marshell_protocol::doctor::{DoctorItem, DoctorLastFix, DoctorReport, DoctorSummary};
use std::sync::Mutex;
use tokio::sync::watch;

/// The fix whose preview was shown and not yet applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFix {
    pub changeset_id: String,
    pub item_ids: Vec<String>,
    /// Tab ids of the running sessions that pick the fix up only after a restart.
    pub restart: Vec<String>,
}

/// The two facts about runs that must change together, so they share one mutex: a request and the end of a run each
/// read and write both under the lock, and neither can slip between the other's read and write. Two atomics would
/// let a request arrive after a run's last look at `again` and before it cleared `running`, and be lost. The lock is
/// never held across an `.await`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RunFlags {
    /// A run is going.
    pub running: bool,
    /// A request came in during the run: one more run follows it, however many requests there were.
    pub again: bool,
}

/// The CLI doctors' run: whether one is going, and what the last one found. Every report carries `items`, so a
/// background run keeps them. Never held across an `.await`.
#[derive(Debug, Default, Clone)]
pub struct CliRun {
    pub running: bool,
    pub items: Vec<DoctorItem>,
    /// When the last CLI run ended; `None` before the first.
    pub checked_at_ms: Option<u64>,
}

/// "Measure MCP servers": whether one is going, and what the last one found. Every report carries the results, so a
/// background run keeps them. Never held across an `.await`.
#[derive(Debug, Default, Clone)]
pub struct MeasureRun {
    pub running: bool,
    pub results: Option<Measurements>,
}

pub struct DoctorState {
    /// The last run's report; `None` until the first run ends.
    pub report: Mutex<Option<DoctorReport>>,
    /// Whether a run is going and whether another was asked for meanwhile. See `RunFlags`.
    pub run: Mutex<RunFlags>,
    /// The CLI doctors' run and findings. See `CliRun`.
    pub cli: Mutex<CliRun>,
    /// "Measure MCP servers": the run and its last results. See `MeasureRun`.
    pub measure: Mutex<MeasureRun>,
    /// What the usage scans found, loaded from `<home>/doctor/usage.json` on the first scan. Never held across I/O.
    pub usage: Mutex<Option<super::usage::UsageStore>>,
    /// What the sidebar and the events socket show.
    pub summary: watch::Sender<DoctorSummary>,
    pub pending: Mutex<Option<PendingFix>>,
    /// The last fix applied in this run of the app, with its restart list. `<home>/doctor/last-fix.json` keeps the rest
    /// across restarts.
    pub last_fix: Mutex<Option<DoctorLastFix>>,
}

impl Default for DoctorState {
    fn default() -> Self {
        Self {
            report: Mutex::new(None),
            run: Mutex::new(RunFlags::default()),
            cli: Mutex::new(CliRun::default()),
            measure: Mutex::new(MeasureRun::default()),
            usage: Mutex::new(None),
            summary: watch::channel(DoctorSummary {
                running: false,
                cli_running: false,
                measuring: false,
                issues: 0,
                checked_at_ms: None,
            })
            .0,
            pending: Mutex::new(None),
            last_fix: Mutex::new(None),
        }
    }
}
