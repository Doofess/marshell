use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which of an agent's config layers a file belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ConfigScope {
    User,
    Project,
    Local,
    Marshell,
}

/// When a change to a config file takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Applies {
    Live,
    NewSessions,
    RestartSession,
}

/// One file in a snapshot: its hashes before and after the change, and where it sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SnapshotFileInfo {
    pub path: String,
    /// Absent when the file did not exist before the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub before_hash: Option<String>,
    /// Absent when the change removed the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after_hash: Option<String>,
    pub scope: ConfigScope,
    pub applies: Applies,
}

/// A restore point taken before a ChangeSet was applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SnapshotInfo {
    pub id: String,
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub created_ms: u64,
    pub changeset_id: String,
    pub title: String,
    pub files: Vec<SnapshotFileInfo>,
}
