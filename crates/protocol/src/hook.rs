use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a bridge call asks of the core. The bridge sends it in `X-Marshell-Role`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum HookRole {
    Event,
    Context,
    Permission,
    Statusline,
    Record,
}

impl HookRole {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "event" => Some(Self::Event),
            "context" => Some(Self::Context),
            "permission" => Some(Self::Permission),
            "statusline" => Some(Self::Statusline),
            "record" => Some(Self::Record),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Context => "context",
            Self::Permission => "permission",
            Self::Statusline => "statusline",
            Self::Record => "record",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum PermissionBehavior {
    Allow,
    Deny,
}

/// How long an answer to a permission request lasts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum PermissionScope {
    #[default]
    Once,
    Session,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PermissionDecision {
    pub behavior: PermissionBehavior,
    #[serde(default)]
    pub scope: PermissionScope,
    pub message: Option<String>,
}

pub const HEADER_AGENT: &str = "x-marshell-agent";
pub const HEADER_TAB: &str = "x-marshell-tab";
pub const HEADER_ROLE: &str = "x-marshell-role";
pub const HEADER_VERSION: &str = "x-marshell-version";

/// The most a bridge call may post: 1 MiB.
pub const HOOK_BODY_LIMIT: usize = 1_048_576;
