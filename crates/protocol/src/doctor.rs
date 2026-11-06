use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::ConfigPreview;

/// Which of the Doctor's report groups an item is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DoctorGroup {
    /// Something Marshell set up, or relies on, doesn't work.
    Broken,
    /// Something costs tokens or space for nothing.
    Waste,
    /// Worth knowing; nothing is wrong.
    Info,
}

/// One finding of the Doctor's checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorItem {
    /// Stable across runs for the same finding, such as `hooks.claude` or `hooks.codex@Ubuntu`.
    pub id: String,
    /// The agent the item is about; absent when it is about Marshell itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    /// The WSL distro the item is about; absent on Windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub distro: Option<String>,
    pub group: DoctorGroup,
    pub title: String,
    pub detail: String,
    /// "Fix X issues" can fix this item.
    pub fixable: bool,
    /// Why the item can't be fixed, or what the person does instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cannot_fix: Option<String>,
    /// The file the item is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    /// The row offers "Open Agents".
    pub open_agents: bool,
    /// A Broken item the page leaves unchecked at first: Marshell isn't sure enough to pick it for the person.
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub unchecked: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The result of one run of the checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorReport {
    pub items: Vec<DoctorItem>,
    #[ts(type = "number")]
    pub checked_at_ms: u64,
    #[ts(type = "number")]
    pub duration_ms: u64,
    /// When the CLI doctors last ran in this run of the app; absent before the first CLI run ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub cli_checked_at_ms: Option<u64>,
    /// When "Measure MCP servers" last ended in this run of the app; absent before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub measured_at_ms: Option<u64>,
}

/// What the sidebar and the tray show: whether a run is going and how many issues the last one found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorSummary {
    pub running: bool,
    /// The CLI doctors are running.
    #[serde(default)]
    pub cli_running: bool,
    /// "Measure MCP servers" is running.
    #[serde(default)]
    pub measuring: bool,
    /// Broken and Waste items in the last report.
    pub issues: u32,
    /// When the last run ended; absent before the first run ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub checked_at_ms: Option<u64>,
}

/// The items to fix, by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorFixRequest {
    pub item_ids: Vec<String>,
}

/// One preview that fixes the chosen items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorFixPlan {
    pub preview: ConfigPreview,
    pub item_ids: Vec<String>,
    /// Tab ids of the running sessions that pick the fix up only after a restart.
    pub restart: Vec<String>,
}

/// The last fix the Doctor applied, which "Undo last fix" puts back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorLastFix {
    pub snapshot_id: String,
    #[ts(type = "number")]
    pub fixed_at_ms: u64,
    pub item_ids: Vec<String>,
    pub count: u32,
    /// Tab ids, as in `DoctorFixPlan`. Empty for a record read back after the app restarted.
    pub restart: Vec<String>,
    /// Every fixed file is unchanged since the fix, so undo can put the backup back.
    pub files_match: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> DoctorItem {
        DoctorItem {
            id: "hooks.claude".into(),
            agent: None,
            distro: None,
            group: DoctorGroup::Broken,
            title: "Claude Code's hooks are gone".into(),
            detail: "The fix sets them up again.".into(),
            fixable: true,
            cannot_fix: None,
            path: None,
            open_agents: false,
            unchecked: false,
        }
    }

    #[test]
    fn absent_options_are_left_out() {
        let json = serde_json::to_value(item()).unwrap();
        let obj = json.as_object().unwrap();
        for key in ["agent", "distro", "cannot_fix", "path"] {
            assert!(!obj.contains_key(key), "{key} should be left out");
        }
        assert_eq!(obj["group"], "broken");
        assert_eq!(obj["open_agents"], false);
    }

    #[test]
    fn groups_are_camel_case() {
        assert_eq!(serde_json::to_value(DoctorGroup::Waste).unwrap(), "waste");
        assert_eq!(serde_json::to_value(DoctorGroup::Info).unwrap(), "info");
    }

    #[test]
    fn a_summary_round_trips() {
        let summary = DoctorSummary {
            running: true,
            cli_running: true,
            measuring: true,
            issues: 3,
            checked_at_ms: Some(1_762_300_000_000),
        };
        let back: DoctorSummary = serde_json::from_str(&serde_json::to_string(&summary).unwrap()).unwrap();
        assert_eq!(back, summary);
        let first: DoctorSummary = serde_json::from_str(r#"{"running":false,"issues":0}"#).unwrap();
        assert_eq!(first.checked_at_ms, None);
        assert!(!first.cli_running);
        assert!(!first.measuring);
    }
}
