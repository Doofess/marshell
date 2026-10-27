use crate::config::ConfigPreview;
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

/// The control a form offers for a curated setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum FieldControl {
    Text,
    /// A whole number of at least `min`.
    Number {
        min: u32,
    },
    Switch,
    /// One of `values`; a value the file holds outside them is still shown as it is.
    Choice {
        values: Vec<String>,
    },
}

/// Why a curated setting can't be edited from the form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum FieldLock {
    /// An organization's managed file sets it, and wins over the user's file.
    Managed { path: String },
    /// The file sets it to a table or an array.
    NotScalar,
    /// The value shows as other than it is, because masking changed it (`TOKEN=abc` in `model`), so a draft
    /// could never start from what is shown.
    Hidden,
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
    /// The form's control; set on the curated settings of an editable agent only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub control: Option<FieldControl>,
    /// Set on a curated setting the form must leave alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lock: Option<FieldLock>,
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
    /// Claude Code or Codex, and the file is `ok` or `missing`: a form edit can be planned.
    pub editable: bool,
    /// The SHA-256 of the file's bytes; None when the file is missing, doesn't parse or can't be read.
    pub hash: Option<String>,
    /// The file's own text, comments and layout included. Some only when the file is `ok` and masking changes
    /// nothing in it, so it holds no value the view would hide.
    pub text: Option<String>,
}

/// One form edit to a curated setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingEdit {
    /// The curated setting's dotted key.
    pub key: String,
    /// The new value as JSON text; None removes the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<String>,
}

/// What a settings change replaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum SettingsChange {
    /// Changes to curated settings, the rest of the file left as it is.
    Form { edits: Vec<SettingEdit> },
    /// The file's whole text.
    Raw { text: String },
}

/// `POST /v1/agents/{agent}/settings/plan`: a change planned against the file as the person last saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsPlanRequest {
    /// The `hash` the view gave; None when the file was missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expected_hash: Option<String>,
    pub change: SettingsChange,
}

/// A planned settings change, ready for `POST /v1/config/apply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsPlan {
    pub preview: ConfigPreview,
    /// True when the diff is of the masked render of the file instead of its own text, because the file holds a
    /// value the view hides.
    pub normalized: bool,
}

/// `POST /v1/agents/{agent}/settings/undo`: puts a settings file back as a snapshot of it had it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsUndoRequest {
    pub snapshot_id: String,
}
