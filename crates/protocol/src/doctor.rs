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

/// A button a Doctor row carries besides the fix's checkbox. Keep and Stop keeping change what Marshell
/// remembers about a file; Use this copy's text opens a fix preview that writes files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DoctorAction {
    /// "Keep my file": Marshell accepts the person's part of a file it set up.
    Keep,
    /// "Stop keeping": Marshell takes that acceptance back.
    StopKeeping,
    /// "Use this copy's text": a library copy edited outside Marshell gives its text to the doc.
    UseCopy,
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
    /// A button on the row that changes what Marshell remembers, not a file of a CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<DoctorAction>,
    /// The instruction files each CLI loads, for a row that lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<DoctorLoad>>")]
    pub loads: Vec<DoctorLoad>,
    /// Files the row can open in the side pane, one button each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<DoctorOpen>>")]
    pub opens: Vec<DoctorOpen>,
}

/// A file a row can open in the side pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorOpen {
    /// Absolute.
    pub path: String,
    /// The line to put the caret on, counted from 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub line: Option<u32>,
    /// The button's words, such as "Open CLAUDE.md".
    pub label: String,
}

/// When a CLI loads an instruction file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DoctorLoadWhen {
    Start,
    WhenUsed,
    NotLoaded,
}

/// One instruction file in a CLI's load order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorLoadFile {
    /// Project-relative, `~/…`, or full.
    pub path: String,
    pub when: DoctorLoadWhen,
    /// The row's note, already worded ("about 600 tokens, imported by CLAUDE.md").
    pub note: String,
    /// 0 for a file the CLI finds itself; 1 for a file it imports, 2 for one that file imports, and so on.
    pub depth: u32,
}

/// The instruction files one CLI loads in one place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorLoad {
    pub agent: String,
    /// "Claude Code: about 2.4k tokens at start"
    pub heading: String,
    pub files: Vec<DoctorLoadFile>,
    /// Sentences shown under the heading ("Copilot CLI documents no limit.").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub notes: Vec<String>,
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

/// The Doctor row's button to press: `keep` is true for "Keep my file", false for "Stop keeping".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorKeepRequest {
    /// The item's id, such as `hooksfile.copilot`.
    pub id: String,
    pub keep: bool,
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
            action: None,
            loads: Vec::new(),
            opens: Vec::new(),
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
    fn actions_are_camel_case_and_left_out_when_absent() {
        assert_eq!(serde_json::to_value(DoctorAction::Keep).unwrap(), "keep");
        assert_eq!(serde_json::to_value(DoctorAction::StopKeeping).unwrap(), "stopKeeping");
        assert!(
            !serde_json::to_value(item())
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("action")
        );
        let mut with = item();
        with.action = Some(DoctorAction::StopKeeping);
        assert_eq!(serde_json::to_value(&with).unwrap()["action"], "stopKeeping");
    }

    #[test]
    fn an_item_with_nothing_to_open_has_no_opens_key() {
        let json = serde_json::to_value(item()).unwrap();
        assert!(!json.as_object().unwrap().contains_key("opens"));

        let mut with = item();
        with.opens = vec![DoctorOpen {
            path: "C:/p/CLAUDE.md".into(),
            line: Some(4),
            label: "Open CLAUDE.md".into(),
        }];
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!(
            json["opens"],
            serde_json::json!([{ "path": "C:/p/CLAUDE.md", "line": 4, "label": "Open CLAUDE.md" }])
        );
        with.opens[0].line = None;
        let json = serde_json::to_value(&with).unwrap();
        assert!(!json["opens"][0].as_object().unwrap().contains_key("line"));

        let back: DoctorItem = serde_json::from_value(serde_json::to_value(item()).unwrap()).unwrap();
        assert!(back.opens.is_empty());
    }

    #[test]
    fn an_item_without_loads_serializes_without_the_key() {
        let json = serde_json::to_value(item()).unwrap();
        assert!(!json.as_object().unwrap().contains_key("loads"));

        let mut with = item();
        with.loads = vec![DoctorLoad {
            agent: "claude".into(),
            heading: "Claude Code: about 600 tokens at start".into(),
            files: vec![DoctorLoadFile {
                path: "CLAUDE.md".into(),
                when: DoctorLoadWhen::WhenUsed,
                note: "about 600 tokens".into(),
                depth: 0,
            }],
            notes: Vec::new(),
        }];
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!(json["loads"][0]["files"][0]["when"], "whenUsed");
        assert!(!json["loads"][0].as_object().unwrap().contains_key("notes"));
        assert_eq!(serde_json::from_value::<DoctorItem>(json).unwrap(), with);
        // A report written before the field existed still reads.
        let old = serde_json::to_value(item()).unwrap();
        assert!(serde_json::from_value::<DoctorItem>(old).unwrap().loads.is_empty());
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
