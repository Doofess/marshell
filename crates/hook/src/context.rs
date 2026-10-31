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
        // Antigravity's shape comes with its own task; until then nothing it is sent is printed.
        _ => false,
    }
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
        assert!(!accepted("antigravity", NOTE));
        assert!(!accepted("grok", NOTE));
    }
}
