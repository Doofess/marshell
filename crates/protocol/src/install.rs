use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How an agent CLI was put on this machine, read from where its program sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum InstallKind {
    /// The vendor's own installer; the CLI updates itself with `<program> update`.
    Native,
    /// A global npm package.
    Npm { package: String },
    /// A WinGet package.
    Winget { id: String },
    /// Anywhere else; Marshell offers no update command.
    Unknown,
}

/// `GET /v1/agents/{agent}/install`: what Marshell knows about one agent CLI's install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AgentInstall {
    pub agent: String,
    pub found: bool,
    /// The version the program reports. None when it isn't found or can't be read.
    pub version: Option<String>,
    pub install: InstallKind,
    /// The update command as shown to the person. None for an unknown install.
    pub update_command: Option<String>,
    /// A newer version the CLI itself recorded (Codex's `version.json`). None when there is none.
    pub newer_version: Option<String>,
}
