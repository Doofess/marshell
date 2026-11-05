//! The Doctor: checks over what Marshell set up for each CLI, as pure functions over facts read elsewhere.

pub mod checks;
pub mod cli;
pub mod state;

pub use checks::{AgentFacts, FileState, RETENTION_OTHERS, display_name, issues, items};
pub use state::{CliRun, DoctorState, PendingFix, RunFlags};
