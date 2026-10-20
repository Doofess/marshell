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
