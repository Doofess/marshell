//! Checks an answer to a permission request before the agent sees it. Whatever listens on the port in
//! `endpoint.json` might not be the core (a Marshell that has exited leaves the file behind), so only the
//! shapes the core gives are printed: allow, allow with rules the agent itself suggested, or deny.

use serde_json::{Map, Value};

/// What the bridge prints for a permission call it can't take to a card: no tab, no core, the core's
/// 204 or timeout, an answer that fails `accepted`. Empty for every agent: Claude and Codex take it as
/// "no opinion", and for Antigravity zero bytes with exit 0 is the neutral answer (`{}` would deny).
pub fn neutral(_agent: &str) -> &'static [u8] {
    b""
}

/// True when `answer` is one the core gives `agent` for `request` (both as the bridge read them).
pub fn accepted(agent: &str, answer: &[u8], request: &[u8]) -> bool {
    let (Ok(answer), Ok(request)) = (
        serde_json::from_slice::<Value>(answer),
        serde_json::from_slice::<Value>(request),
    ) else {
        return false;
    };
    if agent == "antigravity" {
        return antigravity(&answer);
    }
    let Some(top) = answer.as_object().filter(|top| top.len() == 1) else {
        return false;
    };
    let Some(out) = top.get("hookSpecificOutput").and_then(Value::as_object) else {
        return false;
    };
    if out.len() != 2 || out.get("hookEventName").and_then(Value::as_str) != Some("PermissionRequest") {
        return false;
    }
    let Some(decision) = out.get("decision").and_then(Value::as_object) else {
        return false;
    };
    match decision.get("behavior").and_then(Value::as_str) {
        Some("deny") => only(decision, "message") && decision.get("message").is_none_or(Value::is_string),
        Some("allow") => {
            only(decision, "updatedPermissions")
                && decision
                    .get("updatedPermissions")
                    .is_none_or(|rules| suggested(rules, &request))
        }
        _ => false,
    }
}

/// Antigravity's `PreToolUse` answer: a `decision` and at most a string `reason`. Never `overwrite`, and
/// never `{}`, which agy reads as a deny.
fn antigravity(answer: &Value) -> bool {
    let Some(top) = answer.as_object() else {
        return false;
    };
    matches!(
        top.get("decision").and_then(Value::as_str),
        Some("allow" | "deny" | "ask" | "force_ask")
    ) && top
        .iter()
        .all(|(k, v)| k == "decision" || (k == "reason" && v.is_string()))
}

/// The decision holds `behavior` and at most `key`.
fn only(decision: &Map<String, Value>, key: &str) -> bool {
    decision.keys().all(|k| k == "behavior" || k == key)
}

/// Every rule is one of the request's own allow suggestions, kept for the session or this project.
fn suggested(rules: &Value, request: &Value) -> bool {
    let Some(rules) = rules.as_array().filter(|rules| !rules.is_empty()) else {
        return false;
    };
    let suggestions: Vec<Value> = request["permission_suggestions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["type"] == "addRules" && s["behavior"] == "allow")
        .map(without_destination)
        .collect();
    rules.iter().all(|rule| {
        matches!(rule["destination"].as_str(), Some("session" | "localSettings"))
            && suggestions.contains(&without_destination(rule))
    })
}

fn without_destination(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("destination");
    }
    v
}

#[cfg(test)]
mod tests {
    use super::{accepted, neutral};

    const AGY_REQUEST: &[u8] = br#"{"toolName":"run_command","toolInput":{"CommandLine":"echo hi"}}"#;

    fn agy(answer: &str) -> bool {
        accepted("antigravity", answer.as_bytes(), AGY_REQUEST)
    }

    const REQUEST: &[u8] = br#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"Set-Content out.txt hi","description":"Allow running the requested command?"}}"#;

    #[test]
    fn accepts_codex_allow_and_deny() {
        for answer in [
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#,
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied in Marshell."}}}"#,
        ] {
            assert!(accepted("codex", answer.as_bytes(), REQUEST), "{answer}");
        }
        // A Codex request has no suggestions, so a scoped allow is never one the core gives.
        let scoped = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedPermissions":[{"type":"addRules","rules":[{"toolName":"Bash"}],"behavior":"allow","destination":"session"}]}}}"#;
        assert!(!accepted("codex", scoped.as_bytes(), REQUEST));
    }

    #[test]
    fn an_antigravity_allow_is_printed_as_it_is() {
        for decision in ["allow", "deny", "ask", "force_ask"] {
            assert!(agy(&format!(r#"{{"decision":"{decision}"}}"#)), "{decision}");
        }
    }

    #[test]
    fn an_antigravity_deny_with_a_reason_is_printed_as_it_is() {
        assert!(agy(r#"{"decision":"deny","reason":"Denied in Marshell."}"#));
        assert!(!agy(r#"{"decision":"deny","reason":7}"#));
        assert!(!agy(r#"{"decision":"maybe"}"#));
        assert!(!agy(r#"{"decision":true}"#));
        assert!(!agy("not json"));
    }

    #[test]
    fn a_codex_shaped_answer_on_antigravity_is_rejected() {
        let codex = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#;
        assert!(!agy(codex));
        // And the other way round: agy's shape is not one Claude or Codex are given.
        for agent in ["claude", "codex"] {
            assert!(!accepted(agent, br#"{"decision":"deny"}"#, REQUEST), "{agent}");
        }
    }

    #[test]
    fn an_antigravity_answer_with_an_overwrite_is_rejected() {
        assert!(!agy(r#"{"decision":"allow","overwrite":{"toolInput":{}}}"#));
        assert!(!agy(r#"{"decision":"allow","reason":"x","overwrite":null}"#));
    }

    #[test]
    fn an_empty_object_on_antigravity_is_rejected() {
        // agy reads `{}` as a deny, so it must never come out of the bridge.
        assert!(!agy("{}"));
        assert!(!agy(""));
    }

    #[test]
    fn the_antigravity_neutral_answer_is_empty() {
        for agent in ["antigravity", "claude", "codex", "other"] {
            assert!(neutral(agent).is_empty(), "{agent}");
        }
    }
}
