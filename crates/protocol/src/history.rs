use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One archived agent conversation, read into the shape the History view shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Conversation {
    /// The agent's id, such as "claude".
    pub agent: String,
    pub session_id: String,
    /// The Marshell tab the conversation ran in.
    pub tab_id: String,
    /// The person's own title, else the agent's, else the first prompt's first line.
    pub title: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    /// The agent's version is untested, so parts of the conversation may be missing.
    pub limited: bool,
    pub turns: Vec<Turn>,
    #[serde(default)]
    pub marks: ConversationMarks,
}

/// The person's annotations, with bookmark times resolved to turns for the conversation view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConversationMarks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub archived: bool,
    pub bookmarked_turns: Vec<u32>,
}

/// Only supplied fields change. An empty name clears the person's name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Bookmarked {
    pub title: String,
}

/// One prompt and everything the agent did in answer to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Turn {
    /// 0-based, in order.
    pub index: u32,
    /// When the prompt was sent, in ms since the Unix epoch; 0 when the transcript has no usable time.
    #[ts(type = "number")]
    pub at_ms: u64,
    pub prompt: String,
    pub items: Vec<TurnItem>,
}

/// One thing in a turn, in the order it happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum TurnItem {
    /// What the agent said, as untrusted Markdown.
    Text {
        markdown: String,
    },
    Tool {
        card: ToolCard,
    },
    /// A task the agent handed to a subagent, with what the subagent reported back.
    Subagent {
        description: String,
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        result: Option<String>,
    },
    /// The agent compacted the conversation here.
    Compacted,
}

/// A tool call in one line, with the full input and output behind it where there is any.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ToolCard {
    pub name: String,
    /// "Edited src/app.ts, 2 added, 1 removed".
    pub summary: String,
    /// Plain text, capped at 4,000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
    /// The file an Edit or Write call changed. Not set for a failed call or any other tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub file: Option<EditedFile>,
    /// The call failed or was interrupted.
    pub error: bool,
}

/// One file changed by an Edit or Write call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EditedFile {
    /// Relative to the conversation's folder when inside it, else as the agent gave it.
    pub path: String,
    /// Lines added; `None` when the result didn't say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub added: Option<u32>,
    /// Lines removed; `None` when the result didn't say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub removed: Option<u32>,
    /// The call created the file.
    pub created: bool,
}

/// One row of the History list: an archived conversation as the index knows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryEntry {
    pub agent: String,
    pub session_id: String,
    pub tab_id: String,
    pub cwd: String,
    /// Empty until the conversation has been read; the view shows its own placeholder then.
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    /// The conversation couldn't be read in full: untested version, unreadable, or too big.
    pub limited: bool,
    pub turns: u32,
    #[ts(type = "number")]
    pub started_ms: u64,
    #[ts(type = "number")]
    pub updated_ms: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub bookmarks: u32,
    #[serde(default)]
    pub named: bool,
}

/// One full-text match: the turn it is in and a snippet with the hit between U+0002 and U+0003.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SearchHit {
    pub agent: String,
    pub session_id: String,
    pub title: String,
    pub cwd: String,
    pub turn: u32,
    #[ts(type = "number")]
    pub at_ms: u64,
    pub snippet: String,
}

/// The filters of the History list. `project` is the full working folder.
/// `archived: true` includes archived conversations.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub since_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bookmarked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub archived: Option<bool>,
}

/// One event of an asciicast recording, with its time already placed on the Unix clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CastEvent {
    /// The part's start plus the event's offset, in ms since the Unix epoch.
    #[ts(type = "number")]
    pub at_ms: u64,
    /// "o" output, "r" resize ("80x24") or "m" mark.
    pub code: String,
    pub data: String,
}

/// One recording file of a conversation (a tab's run), with the stretch of it the conversation covers.
/// All times are ms since the Unix epoch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecordingPart {
    /// Parts are numbered from 0 in time order.
    pub index: u32,
    /// The recording's file name in Marshell's recordings folder.
    pub file: String,
    #[ts(type = "number")]
    pub started_ms: u64,
    #[ts(type = "number")]
    pub ended_ms: u64,
    pub cols: u16,
    pub rows: u16,
    /// The conversation's span clipped to this part.
    #[ts(type = "number")]
    pub span_from_ms: u64,
    #[ts(type = "number")]
    pub span_to_ms: u64,
    /// Times of the prompt marks.
    #[ts(type = "Array<number>")]
    pub marks_ms: Vec<u64>,
    /// The "Recording stopped:" marker text, when the recording ends with one.
    pub stopped: Option<String>,
}

/// A stretch of a recording's events. `screen` comes only with the first answer of a seek.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecordingFrames {
    pub cols: u16,
    pub rows: u16,
    /// The screen at the start, as escape sequences.
    pub screen: Option<String>,
    pub events: Vec<CastEvent>,
    /// Where to continue, or null at the end of the range.
    #[ts(type = "number | null")]
    pub next_offset: Option<u64>,
}
