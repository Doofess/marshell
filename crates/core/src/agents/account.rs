//! Sign-in state, read from each CLI's own status command. The raw output is parsed here and never leaves.

use marshell_protocol::account::SignIn;

/// A non-empty string field of `value`.
fn text_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
}

/// `claude auth status` (JSON): the state, the email and the plan. Anything unreadable is `Unknown`.
pub fn claude_status(stdout: &str, code: i32) -> (SignIn, Option<String>, Option<String>) {
    let parsed: Option<serde_json::Value> = serde_json::from_str(stdout).ok();
    match parsed.as_ref().and_then(|v| v.get("loggedIn")?.as_bool()) {
        Some(true) => {
            let value = parsed.as_ref().expect("loggedIn came from it");
            (
                SignIn::SignedIn,
                text_field(value, "email"),
                text_field(value, "subscriptionType"),
            )
        }
        Some(false) => (SignIn::SignedOut, None, None),
        // Signed out can exit 1 and print nothing parseable.
        None if code == 1 && parsed.is_none() => (SignIn::SignedOut, None, None),
        None => (SignIn::Unknown, None, None),
    }
}

/// `codex login status`: the state and how it is signed in. The text itself is never returned.
pub fn codex_status(text: &str, code: i32) -> (SignIn, Option<String>) {
    let lower = text.to_lowercase();
    if code != 0 || lower.contains("not logged in") {
        return (SignIn::SignedOut, None);
    }
    if !lower.contains("logged in") {
        return (SignIn::Unknown, None);
    }
    let method = if lower.contains("chatgpt") {
        Some("ChatGPT")
    } else if lower.contains("api key") {
        Some("an API key")
    } else {
        None
    };
    (SignIn::SignedIn, method.map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn claude_signed_in() {
        let out = r#"{"loggedIn":true,"authMethod":"claude.ai","email":"a@example.com","subscriptionType":"max"}"#;
        assert_eq!(claude_status(out, 0), (SignIn::SignedIn, s("a@example.com"), s("max")));
    }

    #[test]
    fn claude_signed_in_without_email() {
        let out = r#"{"loggedIn":true,"authMethod":"api_key"}"#;
        assert_eq!(claude_status(out, 0), (SignIn::SignedIn, None, None));
        let empty = r#"{"loggedIn":true,"email":"","subscriptionType":""}"#;
        assert_eq!(claude_status(empty, 0), (SignIn::SignedIn, None, None));
    }

    #[test]
    fn claude_signed_out() {
        let out = r#"{"loggedIn":false,"authMethod":"none"}"#;
        assert_eq!(claude_status(out, 1), (SignIn::SignedOut, None, None));
        assert_eq!(claude_status("", 1), (SignIn::SignedOut, None, None));
    }

    #[test]
    fn claude_unknown() {
        assert_eq!(claude_status("garbage", 0), (SignIn::Unknown, None, None));
        assert_eq!(claude_status("", 2), (SignIn::Unknown, None, None));
    }

    #[test]
    fn codex_chatgpt() {
        assert_eq!(
            codex_status("Logged in using ChatGPT\n", 0),
            (SignIn::SignedIn, s("ChatGPT"))
        );
    }

    #[test]
    fn codex_api_key_never_returns_the_key() {
        let got = codex_status("Logged in using an API key - sk-proj-***abcd\n", 0);
        assert_eq!(got, (SignIn::SignedIn, s("an API key")));
        assert!(!got.1.unwrap().contains("sk-"));
    }

    #[test]
    fn codex_signed_out() {
        assert_eq!(codex_status("Not logged in\n", 1), (SignIn::SignedOut, None));
        assert_eq!(codex_status("Not logged in", 0), (SignIn::SignedOut, None));
    }

    #[test]
    fn codex_other_text() {
        assert_eq!(codex_status("Logged in\n", 0), (SignIn::SignedIn, None));
    }
}
