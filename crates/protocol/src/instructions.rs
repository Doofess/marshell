//! The instruction library's types (slice 8a): named docs at user and project level, and the copies
//! Marshell writes of them into each CLI's own rule files.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a library doc lives: in the Marshell home (every project) or in one project's repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DocScope {
    /// `<Marshell home>/instructions/<name>.md`, loaded in every project.
    User,
    /// `<project>/.marshell/instructions/<name>.md`, committed with the repo.
    Project,
}

/// One library doc as the Instructions page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LibraryDoc {
    pub name: String,
    pub scope: DocScope,
    /// The doc's file, as the page shows it.
    pub path: String,
    pub description: String,
    /// The CLIs the doc is for, in Marshell's CLI order.
    pub clis: Vec<String>,
    pub globs: Vec<String>,
    /// A project doc's On switch; always true for a user doc.
    pub enabled: bool,
    /// The doc's text after its frontmatter.
    pub body: String,
    /// The config service's hash of the doc's file, sent back with a save.
    pub hash: String,
    /// Why the doc's frontmatter or file name can't be read; such a doc gets no copies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// How one copy of a doc stands against the doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CopyState {
    /// The copy is what the doc renders now.
    InSync,
    /// The copy is as Marshell wrote it, from an older version of the doc.
    OutOfDate,
    /// The doc wants a copy here and there is none yet.
    Missing,
    /// The copy doesn't hash to its marker: someone edited it after Marshell wrote it.
    ChangedOutside,
    /// A copy no doc wants any more, as Marshell wrote it; Review removes it.
    Extra,
    /// A file of the copy's name that Marshell didn't write; Marshell leaves it.
    Taken,
    /// The doc gets no copy in this CLI; the row's notes say why.
    NoCopy,
    /// A stopped distro: its copies wait until it runs again.
    Waiting,
    /// Marshell may not write this copy.
    Refused,
}

/// One row of a doc's "Loads in" list, or of "Copies with no doc".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CopyRow {
    /// The CLI's id; absent only on a stopped distro's Waiting row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cli: Option<String>,
    /// The WSL distro the copy is in; absent on Windows and in a project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub distro: Option<String>,
    /// The copy's file, under `~/` or relative to the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    pub state: CopyState,
    /// Sentences the row shows under its state.
    pub notes: Vec<String>,
    /// Copilot CLI's row that reads Claude Code's `.claude/rules` copy.
    pub shared: bool,
}

/// A doc and its rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DocView {
    pub doc: LibraryDoc,
    pub rows: Vec<CopyRow>,
}

/// What the Instructions page shows: the user docs, and one project's docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LibraryView {
    /// The project shown, as its root folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
    /// Why the project's docs can't be read, such as a stopped distro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project_note: Option<String>,
    pub user_docs: Vec<DocView>,
    pub project_docs: Vec<DocView>,
    pub user_orphans: Vec<CopyRow>,
    pub project_orphans: Vec<CopyRow>,
    /// Copies Review would write or remove.
    pub out_of_date: u32,
    /// Copies of docs that exist, edited outside Marshell.
    pub changed_outside: u32,
    /// The CLIs found on this PC.
    pub found: Vec<String>,
}

/// A doc saved from the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DocSave {
    pub scope: DocScope,
    /// The project the page shows, for either scope; a project doc lives in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
    pub name: String,
    /// The doc's name before a rename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub old_name: Option<String>,
    pub description: String,
    pub clis: Vec<String>,
    pub globs: Vec<String>,
    pub enabled: bool,
    pub body: String,
    /// The doc file's hash when the page read it; absent for a new doc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expected_hash: Option<String>,
}

/// A saved doc: the snapshot the save made, and the page's view after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DocSaved {
    pub snapshot_id: String,
    pub view: LibraryView,
}

/// A doc named by its scope, project and name, never by a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DocRef {
    pub scope: DocScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
    pub name: String,
}

/// Review: every out-of-date, missing and extra copy, at user level and in the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LibraryReview {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
}

/// One of the two fixes for a copy changed outside Marshell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CopyFix {
    pub scope: DocScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
    pub name: String,
    pub cli: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub distro: Option<String>,
    /// False: "Put the doc's text back". True: "Use this copy's text".
    pub take_copy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_state_and_action_serialize() {
        assert_eq!(
            serde_json::to_string(&CopyState::ChangedOutside).unwrap(),
            "\"changed_outside\""
        );
        assert_eq!(
            serde_json::to_string(&crate::doctor::DoctorAction::UseCopy).unwrap(),
            "\"useCopy\""
        );
        assert_eq!(
            serde_json::to_string(&crate::doctor::DoctorAction::StopKeeping).unwrap(),
            "\"stopKeeping\""
        );
        let row = CopyRow {
            cli: None,
            distro: Some("MarshellTestDistro".into()),
            path: None,
            state: CopyState::Waiting,
            notes: vec![],
            shared: false,
        };
        assert_eq!(
            serde_json::to_string(&row).unwrap(),
            r#"{"distro":"MarshellTestDistro","state":"waiting","notes":[],"shared":false}"#
        );
    }
}
