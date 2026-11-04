//! The Doctor: checks over what Marshell set up for each CLI, as pure functions over facts read elsewhere.

pub mod checks;

pub use checks::{AgentFacts, FileState, RETENTION_OTHERS, display_name, issues, items};
