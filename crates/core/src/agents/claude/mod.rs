//! Claude Code's side of a permission request: the JSON a `PermissionRequest` hook answers with.

pub mod compact;
pub mod hooks;
pub mod request;
pub mod status;
pub mod stuck;
pub mod transcript;

use std::path::Path;
use std::time::Duration;

/// The leading `X.Y.Z` of `claude --version` output (`2.1.302 (Claude Code)`). Nothing else counts:
/// a `v` in front, or fewer than three numbers, gives `None`.
pub fn parse_version(stdout: &str) -> Option<String> {
    let first = stdout.split_whitespace().next()?;
    let parts: Vec<&str> = first.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then(|| first.to_string())
}

/// The Claude Code versions whose hook events and status line are recorded in
/// `tests/fixtures/claude/<version>/`. One entry per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(2, 1, 302)];

pub use super::common::Gate;

/// Known: the same minor line as a recorded fixture set, and not older than its oldest patch.
pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Claude Code", FIXTURE_VERSIONS, version)
}

/// Runs `<exe> --version` and parses it. The process is killed when it takes longer than `timeout`.
pub fn run_version(exe: &Path, timeout: Duration) -> Option<String> {
    parse_version(&super::run_version_text(exe, timeout)?)
}

use marshell_protocol::hook::{PermissionBehavior, PermissionDecision, PermissionScope};
use serde_json::{Value, json};

/// The hook output for a decision on `request` (the hook's stdin JSON).
///
/// A scoped allow takes every `addRules` suggestion whose behavior is `allow` and rewrites its
/// destination: `session` for this session, `localSettings` for the project. A deny ignores the scope.
pub fn permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str> {
    let decision = match decision.behavior {
        PermissionBehavior::Deny => {
            let message = decision
                .message
                .as_deref()
                .filter(|m| !m.is_empty())
                .unwrap_or(super::common::DENIED_TEXT);
            json!({ "behavior": "deny", "message": message })
        }
        PermissionBehavior::Allow => {
            let destination = match decision.scope {
                PermissionScope::Once => None,
                PermissionScope::Session => Some("session"),
                PermissionScope::Project => Some("localSettings"),
            };
            let mut allow = json!({ "behavior": "allow" });
            if let Some(destination) = destination {
                let rules: Vec<Value> = request::allow_rule_suggestions(request)
                    .map(|s| {
                        let mut s = s.clone();
                        s["destination"] = json!(destination);
                        s
                    })
                    .collect();
                if rules.is_empty() {
                    return Err("This request has no rule to add.");
                }
                allow["updatedPermissions"] = Value::Array(rules);
            }
            allow
        }
    };
    Ok(json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_gate_knows_the_recorded_minor_line_from_its_oldest_patch() {
        use super::{Gate, version_gate};
        for v in ["2.1.302", "2.1.303", "2.1.999"] {
            assert_eq!(version_gate(Some(v)), Gate::Known, "{v}");
        }
        for v in ["2.2.0", "3.0.0"] {
            assert_eq!(
                version_gate(Some(v)),
                Gate::Unknown(format!("Claude Code {v} is not one Marshell has recorded yet")),
                "{v}"
            );
        }
        for v in ["2.1.301", "2.0.9"] {
            assert_eq!(
                version_gate(Some(v)),
                Gate::Unknown(format!("Claude Code {v} is older than the versions Marshell knows")),
                "{v}"
            );
        }
        let unread = Gate::Unknown("Claude Code's version could not be read".to_string());
        assert_eq!(version_gate(Some("garbage")), unread);
        assert_eq!(version_gate(Some("2.1")), unread);
        assert_eq!(version_gate(None), unread);
    }

    #[test]
    fn claude_gate_messages_are_unchanged() {
        use super::{Gate, version_gate};
        let note = |s: &str| Gate::Unknown(s.to_string());
        assert_eq!(
            version_gate(Some("2.2.0")),
            note("Claude Code 2.2.0 is not one Marshell has recorded yet")
        );
        assert_eq!(
            version_gate(Some("2.1.301")),
            note("Claude Code 2.1.301 is older than the versions Marshell knows")
        );
        assert_eq!(
            version_gate(Some("1.9.0")),
            note("Claude Code 1.9.0 is older than the versions Marshell knows")
        );
        assert_eq!(version_gate(None), note("Claude Code's version could not be read"));
        assert_eq!(version_gate(Some("2.1.302")), Gate::Known);
    }

    #[test]
    fn parse_version_takes_the_leading_semver() {
        use super::parse_version;
        assert_eq!(parse_version("2.1.302 (Claude Code)\n"), Some("2.1.302".to_string()));
        assert_eq!(parse_version("  1.0.0\r\n"), Some("1.0.0".to_string()));
        assert_eq!(parse_version("garbage"), None);
        assert_eq!(parse_version("v2.1.0"), None);
        assert_eq!(parse_version("2.1"), None);
        assert_eq!(parse_version(""), None);
    }

    use super::*;
    use marshell_protocol::hook::{PermissionBehavior, PermissionDecision, PermissionScope};
    use serde_json::{Value, json};

    fn decision(behavior: PermissionBehavior, scope: PermissionScope, message: Option<&str>) -> PermissionDecision {
        PermissionDecision {
            behavior,
            scope,
            message: message.map(str::to_string),
        }
    }

    fn request_with_suggestion() -> Value {
        json!({
            "tool_name": "Bash",
            "permission_suggestions": [
                {"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "npm test"}],
                 "behavior": "allow", "destination": "localSettings"},
                {"type": "setMode", "mode": "acceptEdits", "destination": "session"},
                {"type": "addRules", "rules": [{"toolName": "Bash"}], "behavior": "deny", "destination": "localSettings"}
            ]
        })
    }

    #[test]
    fn allow_once_is_bare_allow() {
        let d = decision(PermissionBehavior::Allow, PermissionScope::Once, None);
        let out = permission_output(&d, &request_with_suggestion()).unwrap();
        assert_eq!(
            out,
            json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}})
        );
    }

    #[test]
    fn deny_carries_the_message() {
        let d = decision(PermissionBehavior::Deny, PermissionScope::Session, Some("Not now"));
        let out = permission_output(&d, &json!({})).unwrap();
        assert_eq!(
            out["hookSpecificOutput"]["decision"],
            json!({"behavior":"deny","message":"Not now"})
        );
        let d = decision(PermissionBehavior::Deny, PermissionScope::Once, None);
        let out = permission_output(&d, &json!({})).unwrap();
        assert_eq!(
            out["hookSpecificOutput"]["decision"],
            json!({"behavior":"deny","message":"Denied in Marshell."})
        );
    }

    #[test]
    fn allow_for_session_adds_the_suggested_rules_to_session() {
        let d = decision(PermissionBehavior::Allow, PermissionScope::Session, None);
        let out = permission_output(&d, &request_with_suggestion()).unwrap();
        let decision = &out["hookSpecificOutput"]["decision"];
        assert_eq!(decision["behavior"], "allow");
        assert_eq!(
            decision["updatedPermissions"],
            json!([{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"npm test"}],
                    "behavior":"allow","destination":"session"}])
        );
    }

    #[test]
    fn allow_for_project_uses_local_settings() {
        let d = decision(PermissionBehavior::Allow, PermissionScope::Project, None);
        let out = permission_output(&d, &request_with_suggestion()).unwrap();
        assert_eq!(
            out["hookSpecificOutput"]["decision"]["updatedPermissions"],
            json!([{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"npm test"}],
                    "behavior":"allow","destination":"localSettings"}])
        );
    }

    #[test]
    fn scoped_allow_without_a_suggestion_is_an_error() {
        let d = decision(PermissionBehavior::Allow, PermissionScope::Session, None);
        assert_eq!(
            permission_output(&d, &json!({"tool_name":"Bash"})),
            Err("This request has no rule to add.")
        );
    }
}
