use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What the person has to do when a tool needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum PromptKind {
    /// A question on the terminal.
    Text,
    /// A sign-in or approval in the browser.
    Browser,
}

/// A line of tool output that means the tool is waiting for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct NeedsYouPattern {
    /// A regular expression run on one line of output.
    pub pattern: String,
    /// What the session says it is waiting for, in Marshell's voice.
    pub phrase: String,
    pub kind: PromptKind,
}

/// A line of tool output that announces a local port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PortPattern {
    /// Capture group 1 is a URL, `host:port`, `[v6]:port` or a bare port.
    pub pattern: String,
}

/// A value a launch preset asks for before it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PresetField {
    pub id: String,
    pub label: String,
}

/// A command the launcher types into a new shell. Marshell never presses Enter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LaunchPreset {
    pub id: String,
    pub name: String,
    pub program: String,
    /// An argument may be a whole `{field-id}` placeholder.
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub fields: Vec<PresetField>,
}

/// How Marshell recognises a developer tool and what it reads from the tool's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ToolProfile {
    pub id: String,
    pub name: String,
    /// Program names, lowercase, no extension.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Path fragments in the first argument of a node or python child.
    #[serde(default)]
    pub scripts: Vec<String>,
    /// An `npx`-style launcher: its patterns join those of the tool it launches.
    #[serde(default)]
    pub launcher: bool,
    #[serde(default)]
    pub needs_you: Vec<NeedsYouPattern>,
    #[serde(default)]
    pub ports: Vec<PortPattern>,
    /// Command-line prefixes after the program (such as "auth login") whose port lines are transient.
    #[serde(default)]
    pub quiet_ports: Vec<String>,
    #[serde(default)]
    pub presets: Vec<LaunchPreset>,
}

/// Why one own-tool entry was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OwnToolProblem {
    pub index: u32,
    pub id: Option<String>,
    pub reason: String,
}
