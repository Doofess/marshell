use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whose file a shown setting comes from. Only the settings view uses it; it is not a write scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ViewScope {
    User,
    /// An organization's managed file (Claude Code), which wins over the user's.
    Managed,
}

/// What reading an agent's user settings file gave. No variant carries text from the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum ViewState {
    Ok,
    Missing,
    /// The file doesn't parse; `line` is the 1-based line of the first error.
    Unparsable {
        line: u32,
    },
    /// The file exists but can't be read.
    Unreadable,
}

/// One setting as it may be shown: secrets are already the word `hidden`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingValue {
    /// A dotted key for a curated setting (`permissions.defaultMode`), a top-level key otherwise.
    pub key: String,
    /// None when the file doesn't set it.
    pub value: Option<String>,
    pub scope: ViewScope,
}

/// `GET /v1/agents/{agent}/settings-view`: an agent's user settings file, read and never written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentSettingsView {
    pub agent: String,
    pub path: String,
    pub state: ViewState,
    /// The settings Marshell knows by name, in a fixed order.
    pub curated: Vec<SettingValue>,
    /// The file's other top-level keys, sorted.
    pub others: Vec<SettingValue>,
    /// The masked values written back out as pretty JSON (TOML for Codex); comments and layout are left out.
    pub raw: Option<String>,
}
