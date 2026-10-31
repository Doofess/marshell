//! Checks an answer to a prompt event before the agent sees it. Whatever listens on the port in `endpoint.json`
//! might not be the core, so only the one shape the core gives is printed: a note added to the prompt as context.

use serde_json::Value;

/// True when `answer` is one the core gives `agent` for a prompt event.
pub fn accepted(agent: &str, answer: &[u8]) -> bool {
    let Ok(answer) = serde_json::from_slice::<Value>(answer) else {
        return false;
    };
    match agent {
        "claude" | "codex" => hook_specific_output(&answer),
        "antigravity" => inject_steps(&answer),
        _ => false,
    }
}

/// `{"injectSteps":[{"ephemeralMessage":<string>}]}`: one step with one key, and no other key at the top.
fn inject_steps(answer: &Value) -> bool {
    let Some(top) = answer.as_object().filter(|top| top.len() == 1) else {
        return false;
    };
    let Some([step]) = top.get("injectSteps").and_then(Value::as_array).map(Vec::as_slice) else {
        return false;
    };
    step.as_object()
        .is_some_and(|step| step.len() == 1 && step.get("ephemeralMessage").is_some_and(Value::is_string))
}

/// `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":<string>}}` and no other key at
/// either level.
fn hook_specific_output(answer: &Value) -> bool {
    let Some(top) = answer.as_object().filter(|top| top.len() == 1) else {
        return false;
    };
    let Some(out) = top.get("hookSpecificOutput").and_then(Value::as_object) else {
        return false;
    };
    out.len() == 2
        && out.get("hookEventName").and_then(Value::as_str) == Some("UserPromptSubmit")
        && out.get("additionalContext").is_some_and(Value::is_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &[u8] =
        br#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"References: 'a' = /r/a."}}"#;

    #[test]
    fn claude_and_codex_take_the_context_object() {
        assert!(accepted("claude", NOTE));
        assert!(accepted("codex", NOTE));
    }

    const AGY: &[u8] = br#"{"injectSteps":[{"ephemeralMessage":"References: 'a' = /r/a."}]}"#;

    #[test]
    fn antigravity_takes_one_ephemeral_message() {
        assert!(accepted("antigravity", AGY));
    }

    #[test]
    fn antigravity_takes_nothing_near_it() {
        for answer in [
            &br#"{"injectSteps":[{"ephemeralMessage":"x"}],"continue":false}"#[..],
            br#"{"injectSteps":[{"ephemeralMessage":"x"},{"ephemeralMessage":"y"}]}"#,
            br#"{"injectSteps":[]}"#,
            br#"{"injectSteps":[{"userMessage":"x"}]}"#,
            br#"{"injectSteps":[{"ephemeralMessage":"x","userMessage":"y"}]}"#,
            br#"{"injectSteps":[{"ephemeralMessage":1}]}"#,
            br#"{"injectSteps":[{"ephemeralMessage":null}]}"#,
            br#"{"injectSteps":{"ephemeralMessage":"x"}}"#,
            br#"{"injectSteps":["x"]}"#,
            NOTE,
            b"not json",
            b"",
        ] {
            assert!(!accepted("antigravity", answer), "{}", String::from_utf8_lossy(answer));
        }
        // Each agent takes only its own shape.
        for agent in ["claude", "codex"] {
            assert!(!accepted(agent, AGY), "{agent}");
        }
    }

    #[test]
    fn nothing_else_is_accepted() {
        for answer in [
            &br#"{"decision":"block"}"#[..],
            br#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"x"},"continue":false}"#,
            br#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"x","extra":1}}"#,
            br#"{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"x"}}"#,
            br#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":null}}"#,
            br#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit"}}"#,
            br#"[]"#,
            b"not json",
            b"",
        ] {
            assert!(!accepted("claude", answer), "{}", String::from_utf8_lossy(answer));
        }
        assert!(!accepted("grok", NOTE));
        assert!(!accepted("grok", AGY));
    }
}
