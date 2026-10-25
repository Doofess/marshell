use crate::api::SessionMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// How a CLI takes a first prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "style", rename_all = "kebab-case")]
#[ts(export)]
pub enum PromptStyle {
    /// The prompt is a trailing argument.
    Positional,
    /// The prompt follows this flag.
    Flag { flag: String },
    /// The CLI takes no first prompt.
    None,
}

/// Everything Marshell needs to find, launch and recognise an agent CLI that has no Rust adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct AgentManifest {
    pub id: String,
    pub name: String,
    pub cli: String,
    pub mark: String,
    /// Program names to look up on PATH, in order.
    pub commands: Vec<String>,
    /// Resolved paths containing one of these (case-insensitive, `/` read as `\`) are not this agent.
    #[serde(default)]
    pub reject_paths: Vec<String>,
    /// A regex the `--version` output must match.
    #[serde(default)]
    pub version_shape: Option<String>,
    /// Executable names (without extension) that identify a running process as this agent.
    #[serde(default)]
    pub exe_names: Vec<String>,
    /// npm package names that install this CLI.
    #[serde(default)]
    pub packages: Vec<String>,
    pub prompt: PromptStyle,
    /// Arguments that continue the last conversation, if the CLI has any.
    #[serde(rename = "continue")]
    pub continue_args: Option<Vec<String>>,
    /// The flags for each mode the CLI supports.
    #[serde(default)]
    pub modes: BTreeMap<SessionMode, Vec<String>>,
}

/// Why one own-agent entry was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OwnAgentProblem {
    pub index: u32,
    pub id: Option<String>,
    pub reason: String,
}

/// One agent the launcher can offer, as the core sees it on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CatalogAgent {
    pub id: String,
    pub name: String,
    pub cli: String,
    pub mark: String,
    /// Claude Code, Codex and Antigravity have a Rust adapter; every other agent is a manifest.
    pub adapter: bool,
    /// Defined by the person in `agents.json`.
    pub own: bool,
    /// The agent's program resolves on PATH (or in a profile that runs it) as this agent.
    pub found: bool,
    pub version: Option<String>,
    pub prompt: PromptStyle,
    /// The modes the launcher offers for this agent.
    pub modes: Vec<SessionMode>,
    /// The CLI has a way to continue its last conversation.
    pub can_continue: bool,
}

/// A saved way to start an agent (a DeepSeek key setup, for instance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CatalogPreset {
    pub id: String,
    pub name: String,
    /// The agent the preset starts.
    pub agent: String,
    pub mark: String,
    /// Why the preset can't start yet, if it can't.
    pub problem: Option<String>,
}

/// Everything the launcher offers: adapters first, then the shipped manifests, then the person's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentCatalog {
    pub agents: Vec<CatalogAgent>,
    pub presets: Vec<CatalogPreset>,
}

/// The agents in `agents.json`, and the entries or the file the core refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OwnAgents {
    /// The entries as stored, so a problem's `index` points into this list. A file that can't be read gives none.
    pub agents: Vec<AgentManifest>,
    pub problems: Vec<OwnAgentProblem>,
    /// SHA-256 (hex) of the file's bytes, or "" when there is no file. A save names the version it was based on.
    pub version: String,
}

/// A request to replace the own agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OwnAgentsSave {
    pub agents: Vec<AgentManifest>,
    /// The `version` of the `OwnAgents` this edit started from.
    pub version: String,
}
