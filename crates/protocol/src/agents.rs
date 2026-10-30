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

/// Whether the `marshell` skill file is in place for an agent, judged by the file's hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SkillInstall {
    /// No skill file.
    None,
    /// The file holds the text this Marshell ships.
    Installed,
    /// The file holds a text an earlier Marshell shipped.
    Outdated,
    /// The file holds anything else: the person edited it, or Marshell didn't write it.
    Edited,
}

/// Whether Codex trusts the hook entries Marshell added, as its `config.toml` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookTrust {
    /// Every entry has a recorded hash that matches and is not disabled.
    Trusted,
    /// An entry has no recorded hash, or is disabled, and none has a different hash.
    Untrusted,
    /// An entry's recorded hash differs from the hash of what is in `hooks.json` now.
    Changed,
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
    /// The file the agent's hooks live in when that is not `settings_path` (Antigravity's `hooks.json`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hooks_path: Option<String>,
    pub settings_state: SettingsState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parse_error: Option<ParseProblem>,
    pub hooks: HookInstall,
    /// The PermissionRequest hook is in place.
    pub approve_installed: bool,
    /// A status line that is not Marshell's, or the one the chain keeps.
    pub has_own_statusline: bool,
    /// Ours is in place and the chain holds the user's.
    pub chained: bool,
    /// The person has to trust the hooks inside the agent before it runs them (Codex).
    pub needs_trust: bool,
    /// What the agent's own config says about Marshell's hook entries; absent when it holds none of ours
    /// or when the agent has no such trust.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trust: Option<HookTrust>,
    pub bridge_path: String,
    pub snapshots_dir: String,
    /// What the file at `skill_path` holds: none, Marshell's current skill, an older one, or other text.
    pub skill: SkillInstall,
    /// Where this agent's user-level `marshell` skill file goes.
    pub skill_path: String,
}

/// Whether one agent's CLI is on this machine, which is all Marshell knows about an agent it has no adapter for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentFound {
    /// The agent's id: "claude", "codex" or "antigravity".
    pub agent: String,
    /// A detected profile runs this agent, or its program is on PATH.
    pub found: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exe_path: Option<String>,
}

/// What an install plan should include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HookPlanRequest {
    /// Add the hook entries for status.
    pub status: bool,
    /// Keep the user's own status line working through the chain.
    pub statusline: bool,
    /// Add the PermissionRequest hook so requests can be answered from Marshell.
    #[serde(default)]
    pub approve: bool,
    /// Add the `marshell` skill file to the same preview.
    #[serde(default)]
    pub skill: bool,
}

/// What removing an agent's hooks would do: the preview, and whether the user's changed status
/// line stays as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemovePreview {
    pub preview: crate::config::ConfigPreview,
    /// The person changed `statusLine` since the install, so removal leaves it alone.
    pub statusline_left: bool,
}
