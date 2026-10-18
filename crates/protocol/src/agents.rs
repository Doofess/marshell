use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How far an agent's settings file carries Marshell's hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookInstall {
    /// No Marshell entries.
    None,
    /// All entries and the status line are in place.
    Installed,
    /// Some of the 8 events or the status line are missing, or entries carry a different bridge path.
    Partial,
    /// Entries are present but the file at `bridge_path` is missing.
    Orphaned,
}

/// Whether an agent's settings file could be read for a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsState {
    Ok,
    Missing,
    Unparsable,
    ReadOnly,
}

/// Where and why a settings file failed to parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ParseProblem {
    /// 1-based line of the error.
    pub line: u32,
    pub message: String,
}

/// What Marshell knows about one agent's setup: the CLI, its settings file and the hooks in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentSetup {
    /// The agent's id, such as "claude".
    pub agent: String,
    /// A detected profile runs this agent.
    pub found: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exe_path: Option<String>,
    pub settings_path: String,
    pub settings_state: SettingsState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parse_error: Option<ParseProblem>,
    pub hooks: HookInstall,
    /// A status line that is not Marshell's, or the one the chain keeps.
    pub has_own_statusline: bool,
    /// Ours is in place and the chain holds the user's.
    pub chained: bool,
    pub bridge_path: String,
    pub snapshots_dir: String,
}

/// What an install plan should include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HookPlanRequest {
    /// Add the hook entries for status.
    pub status: bool,
    /// Keep the user's own status line working through the chain.
    pub statusline: bool,
}
