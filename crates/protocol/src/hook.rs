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

/// One line of a diff shown on an approve card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DiffKind {
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

/// What a held permission request asks to do, in a form the approve card can show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum PermissionDetail {
    Bash {
        command: String,
        /// Only when it differs from the session's folder.
        cwd: Option<String>,
    },
    Edit {
        path: String,
        add: u32,
        del: u32,
        hunk: Vec<DiffLine>,
    },
    Write {
        path: String,
        /// The first lines of the new content.
        preview: String,
        is_new: bool,
    },
    Fetch {
        url: String,
    },
    Mcp {
        server: String,
        tool: String,
        args: Vec<(String, String)>,
    },
    /// A tool Marshell doesn't know, shown by name with its input as `key: value` lines.
    Other {
        tool: String,
        args: Vec<(String, String)>,
    },
}

/// A held permission request as the UI sees it (sent in the `permissions` events frame).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PermissionView {
    pub rid: String,
    pub tab: String,
    pub agent: String,
    /// When the request arrived, in Unix milliseconds.
    #[ts(type = "number")]
    pub at_ms: u64,
    pub detail: PermissionDetail,
    /// The plain reason the request is risky; absent when it isn't.
    pub risk: Option<String>,
    /// The exact rule "Always allow" would add, such as `Bash(npm test:*)`; absent when there is none.
    pub always_rule: Option<String>,
}

/// GET /v1/sessions/{tab}/peek body: the session's last rendered rows, as plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PeekRows {
    pub rows: Vec<String>,
}
