use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

/// The tools in `tools.json`, and the entries or the file the core refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OwnTools {
    /// The entries as stored, so a problem's `index` points into this list. An entry that does not parse is listed as a
    /// placeholder built from what could be read of it. A file that can't be read gives none.
    pub tools: Vec<ToolProfile>,
    pub problems: Vec<OwnToolProblem>,
    /// SHA-256 (hex) of the file's bytes, or "" when there is no file. A save names the version it was based on.
    pub version: String,
}

/// A request to replace the own tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OwnToolsSave {
    pub tools: Vec<ToolProfile>,
    /// The `version` of the `OwnTools` this edit started from.
    pub version: String,
}

/// A launch preset as the launcher lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CatalogToolPreset {
    pub id: String,
    pub name: String,
    pub fields: Vec<PresetField>,
    /// Whether the preset's `program` resolves on the core's PATH.
    pub found: bool,
}

/// A tool profile as the launcher lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CatalogTool {
    pub id: String,
    pub name: String,
    /// Defined in `tools.json` rather than shipped.
    pub own: bool,
    pub presets: Vec<CatalogToolPreset>,
}

/// `GET /v1/tools`: the shipped and the own tools, and why own entries were refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ToolCatalog {
    pub tools: Vec<CatalogTool>,
    pub problems: Vec<OwnToolProblem>,
}

/// A launch preset to start in a new tab, with the values the person filled in. The values are used for this start
/// only: no layout, memory, history or log holds them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ToolLaunch {
    /// The tool's id.
    pub tool: String,
    /// The preset's id within the tool.
    pub preset: String,
    /// A value for each of the preset's fields, by field id.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

// No derived Debug: `fields` holds what the person typed, and a debug print must not show it.
impl std::fmt::Debug for ToolLaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolLaunch")
            .field("tool", &self.tool)
            .field("preset", &self.preset)
            .field("fields", &self.fields.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A local port a tab's tool announced or a process in the tab listens on, as the tab shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PortChip {
    pub port: u16,
    /// `scheme://host:port/path` with a loopback host: `localhost`, `127.0.0.1` or `[::1]`.
    pub url: String,
    /// The tool printed this port, as against a listener found in the TCP table.
    pub announced: bool,
}
