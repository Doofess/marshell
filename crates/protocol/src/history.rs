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
    /// The call failed or was interrupted.
    pub error: bool,
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
}
