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

/// One edit to one config file. Edits to the same path are applied in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum EditOp {
    /// Sets the value at a JSON pointer. `value` is JSON text, so this crate needs no JSON value type.
    JsonSet {
        pointer: String,
        value: String,
    },
    JsonRemove {
        pointer: String,
    },
    /// Creates a file that must not exist yet.
    CreateFile {
        content: String,
    },
    /// Sets the key at a TOML key path, keeping the file's comments and key order. `value` is JSON text and
    /// must be a string, an integer, a float or a bool; anything else is refused as `BadValue`.
    TomlSet {
        key: Vec<String>,
        value: String,
    },
    TomlRemove {
        key: Vec<String>,
    },
    /// Replaces a whole file's text. `expected_hash` is required and the file must exist: the text is replaced
    /// only if the file still holds what the caller saw.
    ReplaceText {
        content: String,
    },
    /// Removes a file. `expected_hash` is required: the file is deleted only if it still holds what the caller saw.
    DeleteFile,
    /// Puts a file back the way a snapshot saved it.
    Restore {
        snapshot_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileEdit {
    pub path: String,
    pub scope: ConfigScope,
    pub op: EditOp,
    /// The hash of the file the edit was made against; a different hash on disk is a conflict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expected_hash: Option<String>,
    pub applies: Applies,
}

/// A set of edits that is previewed and applied as one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChangeSet {
    pub title: String,
    pub reason: String,
    pub edits: Vec<FileEdit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RefuseReason {
    NotAbsolute,
    Managed,
    OutsideAllowed,
    MarshellOwn,
    Unreadable,
    TooLarge,
    ReadOnly,
    NotUtf8,
    DoesNotParse,
    BadPointer,
    BadValue,
    Conflict,
    Exists,
    SnapshotMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum FileStatus {
    Changes,
    NoChange,
    Refused { reason: RefuseReason, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FilePreview {
    pub path: String,
    pub scope: ConfigScope,
    pub applies: Applies,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub before_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after_hash: Option<String>,
    /// A unified diff. Empty for a refused file and for a file with no change.
    pub diff: String,
    pub status: FileStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConfigPreview {
    pub changeset_id: String,
    pub title: String,
    pub reason: String,
    pub files: Vec<FilePreview>,
    /// True when a file changed on disk since the preview was made.
    pub stale: bool,
    pub can_apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExpectedHash {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApplyRequest {
    pub changeset_id: String,
    pub expected_hashes: Vec<ExpectedHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppliedFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApplyResult {
    pub snapshot_id: String,
    pub files: Vec<AppliedFile>,
}

/// The body of a 409: nothing was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApplyConflict {
    pub reason: String,
    pub paths: Vec<String>,
}

/// A config file as the core read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConfigFile {
    pub path: String,
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub refused: Option<RefuseReason>,
}
