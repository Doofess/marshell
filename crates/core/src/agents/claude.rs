//! Claude Code's side of a permission request: the JSON a `PermissionRequest` hook answers with.

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
                .unwrap_or("Denied in Marshell.");
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
                let rules: Vec<Value> = request
                    .get("permission_suggestions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|s| s["type"] == "addRules" && s["behavior"] == "allow")
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
