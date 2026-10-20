use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether the core's log is running, and whether it writes the detailed (debug) level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoggingState {
    /// False when the core was started without a log (the detailed switch has nothing to act on).
    pub available: bool,
    /// Marshell's own crates log at debug until the app exits. Never saved.
    pub detailed: bool,
}

/// What a group of processes uses right now.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcessUsage {
    /// How many of the group's processes could be read.
    pub processes: u32,
    /// Working set of the readable processes, summed. None when none could be read.
    #[ts(type = "number | null")]
    pub memory_bytes: Option<u64>,
    /// Share of the whole machine's CPU since the last reading, in percent. None until a process has been read twice.
    pub cpu_percent: Option<f32>,
}

/// One open session's process tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionUsage {
    pub tab_id: String,
    pub usage: ProcessUsage,
}

/// `GET /v1/perf`: startup time and the resource use of the core and of each open session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PerfSnapshot {
    /// From the app's `main` to the first core serving. None until both have happened.
    #[ts(type = "number | null")]
    pub startup_ms: Option<u64>,
    pub core: ProcessUsage,
    pub sessions: Vec<SessionUsage>,
    /// Unix time of this reading, in milliseconds.
    #[ts(type = "number")]
    pub sampled_at_ms: u64,
}
