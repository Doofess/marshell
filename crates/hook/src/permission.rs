//! Checks an answer to a permission request before the agent sees it. Whatever listens on the port in
//! `endpoint.json` might not be the core (a Marshell that has exited leaves the file behind), so only the
//! shapes the core gives are printed: allow, allow with rules the agent itself suggested, or deny.

use serde_json::{Map, Value};

/// True when `answer` is one the core gives for `request` (both as the bridge read them).
pub fn accepted(answer: &[u8], request: &[u8]) -> bool {
    let (Ok(answer), Ok(request)) = (
        serde_json::from_slice::<Value>(answer),
        serde_json::from_slice::<Value>(request),
    ) else {
        return false;
    };
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
