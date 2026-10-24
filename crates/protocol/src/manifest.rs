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
