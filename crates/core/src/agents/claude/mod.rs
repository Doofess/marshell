//! Claude Code's side of a permission request: the JSON a `PermissionRequest` hook answers with.

pub mod hooks;
pub mod status;

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The leading `X.Y.Z` of `claude --version` output (`2.1.302 (Claude Code)`). Nothing else counts:
/// a `v` in front, or fewer than three numbers, gives `None`.
pub fn parse_version(stdout: &str) -> Option<String> {
    let first = stdout.split_whitespace().next()?;
    let parts: Vec<&str> = first.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then(|| first.to_string())
}

/// Runs `<exe> --version` and parses it. The process is killed when it takes longer than `timeout`.
pub fn run_version(exe: &Path, timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console window flashes.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on its own thread, so a full pipe cannot hold the child past the deadline.
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut out);
        out
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    // After a kill the reader is left alone: a grandchild may still hold the pipe open.
    if !status?.success() {
        return None;
    }
    let out = reader.join().ok()?;
    parse_version(&String::from_utf8_lossy(&out))
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
