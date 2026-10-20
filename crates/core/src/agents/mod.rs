//! One module per agent CLI. Each holds that agent's own JSON; the code around it stays vendor-neutral.

pub mod claude;

/// Variables that identify the Claude Code session Marshell itself may have been started from;
/// user settings such as `CLAUDE_CODE_GIT_BASH_PATH` are not in it.
pub const INHERITED_MARKERS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_PID",
];

use crate::hooks::holds::PendingPermission;
use marshell_protocol::hook::{PermissionDecision, PermissionView};
use serde_json::Value;

/// Whether Marshell can show and answer this agent's permission requests.
pub fn supports_permissions(agent: &str) -> bool {
    matches!(agent, "claude")
}

/// A held request's card, when its agent is known and it belongs to a tab.
pub fn permission_view(p: &PendingPermission) -> Option<PermissionView> {
    p.tab.as_ref()?;
    if !supports_permissions(&p.agent) {
        return None;
    }
    claude::request::view(p)
}

/// The hook output that carries `decision` back to `agent`, or why it can't.
pub fn permission_output(agent: &str, decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str> {
    match agent {
        "claude" => claude::permission_output(decision, request),
        _ => Err("This agent can't take a decision yet."),
    }
}

#[cfg(test)]
mod tests {
    use super::INHERITED_MARKERS;

    #[test]
    fn markers_name_the_session_identity_only() {
        assert_eq!(
            INHERITED_MARKERS,
            [
                "CLAUDECODE",
                "CLAUDE_CODE_ENTRYPOINT",
                "CLAUDE_CODE_CHILD_SESSION",
                "CLAUDE_CODE_SESSION_ID",
                "CLAUDE_CODE_SESSION_ATTENDED",
                "CLAUDE_CODE_MESSAGING_SOCKET",
                "CLAUDE_CODE_MESSAGING_TOKEN",
                "CLAUDE_CODE_EXECPATH",
                "CLAUDE_CODE_SSE_PORT",
                "CLAUDE_PID",
            ]
        );
        assert!(!INHERITED_MARKERS.contains(&"CLAUDE_CODE_GIT_BASH_PATH"));
        assert!(!INHERITED_MARKERS.contains(&"CLAUDE_CODE_USE_BEDROCK"));
    }
}
