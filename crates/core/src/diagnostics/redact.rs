//! Takes secrets, the user's home folder and the user name out of text before it is logged or exported.
//! Best-effort: the export preview exists so the person can read what leaves.

use crate::paths;
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};

/// Rules 2 to 5, in order: a pattern and what replaces each match.
static RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let rule = |pattern: &str, with: &'static str| (Regex::new(pattern).expect("a valid redaction pattern"), with);
    let mut rules = vec![
        rule(r"sk-ant-[A-Za-z0-9_-]{8,}", "<anthropic-key>"),
        rule(r"sk-(?:proj-)?[A-Za-z0-9_-]{20,}", "<openai-key>"),
        rule(r"gh[pousr]_[A-Za-z0-9]{20,}", "<github-token>"),
        rule(r"github_pat_[A-Za-z0-9_]{20,}", "<github-token>"),
        rule(r"AKIA[0-9A-Z]{16}", "<aws-key>"),
        rule(r"xox[abprs]-[A-Za-z0-9-]{10,}", "<slack-token>"),
        rule(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", "<jwt>"),
        rule(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}", "Bearer <redacted>"),
        // `Basic` is also an ordinary word, so a bare one needs a base64-looking value (a digit, `+`, `/` or
        // `=` in it); after `Authorization` any value goes.
        rule(
            r"(?i)(\bauthorization\W{0,8})basic\s+[A-Za-z0-9._~+/=-]{4,}",
            "${1}Basic <redacted>",
        ),
        rule(
            r"(?i)\bbasic\s+[A-Za-z]*[0-9+/=][A-Za-z0-9._~+/=-]{6,}",
            "Basic <redacted>",
        ),
    ];
    for prefix in [
        // The name may carry an environment-variable prefix; `max_tokens` is not a secret name.
        r#"[A-Za-z0-9_-]*(?:api[_-]?key|access[_-]?token|token|secret|password|passwd|pwd)(?:\\?["'])?\s*[:=]\s*"#,
        r"(?:^|[^A-Za-z0-9_-])--(?:api-key|apikey|token|access-token|secret|password|passwd)(?:=|\s+)",
    ] {
        // Keep the delimiters (including JSON escaping), replacing the entire quoted value first.
        for (quote, body) in [
            (r#"\\""#, r#"[^\\]*(?:\\[^"\\][^\\]*)*"#),
            (r"\\'", r"[^\\]*(?:\\[^'\\][^\\]*)*"),
            ("\"", r#"[^"\\]*(?:\\.[^"\\]*)*"#),
            ("'", r"[^'\\]*(?:\\.[^'\\]*)*"),
        ] {
            rules.push(rule(
                &format!("(?i)({prefix})({quote}){body}({quote})"),
                "${1}${2}<redacted>${3}",
            ));
        }
        rules.push(rule(&format!(r#"(?i)({prefix})[^\s"'\\&,;]+"#), "${1}<redacted>"));
    }
    // Greedy within the authority: a password may itself contain `@`.
    rules.push(rule(r"://[^/\s:@]+:[^/\s?#]+@", "://<redacted>@"));
    rules
});

/// A folder to hide: the matcher and what replaces it.
struct FolderRule {
    matcher: Regex,
    placeholder: &'static str,
    /// The folder's parts, lower-cased, to tell whether one folder sits inside another.
    parts: Vec<String>,
}

pub struct Redactor {
    tokens: Mutex<Vec<String>>,
    marshell_home: Option<FolderRule>,
    user_home: Option<FolderRule>,
    user_name: Option<Regex>,
}

/// The parts of a path without a `\\?\` prefix, and whether it starts with a separator.
fn path_parts(path: &str) -> (Vec<String>, bool) {
    let path = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix("//?/"))
        .unwrap_or(path);
    let rooted = path.starts_with(['\\', '/']);
    let parts = path
        .split(['\\', '/'])
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    (parts, rooted)
}

fn folder_rule(path: &Path, placeholder: &'static str) -> Option<FolderRule> {
    let (parts, rooted) = path_parts(&path.to_string_lossy());
    if parts.is_empty() {
        return None;
    }
    let body = parts
        .iter()
        .map(|p| regex::escape(p))
        .collect::<Vec<_>>()
        .join(r"[\\/]+");
    let root = if rooted { r"[\\/]+" } else { "" };
    // The folder must start and end at a boundary, so `C:\Users\me2` is not `C:\Users\me`. The character
    // after it is captured so it can be put back.
    let pattern = format!(r"(?i)(?P<pre>^|[^A-Za-z0-9_.\-/\\])(?:\\\\\?\\)?{root}{body}(?P<post>[^A-Za-z0-9_]|$)");
    Some(FolderRule {
        matcher: Regex::new(&pattern).ok()?,
        placeholder,
        parts: parts.iter().map(|p| p.to_lowercase()).collect(),
    })
}

fn replace_folder<'a>(rule: &FolderRule, text: Cow<'a, str>) -> Cow<'a, str> {
    // A closure: `$post` in a template would read the placeholder's `<` wrongly.
    let replaced = match rule.matcher.replace_all(&text, |c: &Captures| {
        format!("{}{}{}", &c["pre"], rule.placeholder, &c["post"])
    }) {
        Cow::Owned(s) => Some(s),
        Cow::Borrowed(_) => None,
    };
    match replaced {
        Some(s) => Cow::Owned(s),
        None => text,
    }
}

fn replace_regex<'a>(re: &Regex, with: &str, text: Cow<'a, str>) -> Cow<'a, str> {
    let replaced = match re.replace_all(&text, with) {
        Cow::Owned(s) => Some(s),
        Cow::Borrowed(_) => None,
    };
    match replaced {
        Some(s) => Cow::Owned(s),
        None => text,
    }
}

impl Redactor {
    pub fn new(user_home: Option<PathBuf>, marshell_home: Option<PathBuf>, user_name: Option<String>) -> Redactor {
        let user_home = user_home.and_then(|p| folder_rule(&p, "~"));
        let marshell_home = marshell_home
            .and_then(|p| folder_rule(&p, "<marshell-home>"))
            // Inside the user's home, the `~` rule already covers it.
            .filter(|m| !user_home.as_ref().is_some_and(|u| m.parts.starts_with(&u.parts)));
        let user_name = user_name
            .map(|n| n.trim().to_string())
            .filter(|n| n.chars().count() >= 3)
            .and_then(|n| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(&n))).ok());
        Redactor {
            tokens: Mutex::new(Vec::new()),
            marshell_home,
            user_home,
            user_name,
        }
    }

    /// Adds a secret (a launch's API token) that must never be written out.
    pub fn remember_token(&self, token: &str) {
        if token.is_empty() {
            return;
        }
        let mut tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        if !tokens.iter().any(|t| t == token) {
            tokens.push(token.to_string());
        }
    }

    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut out = Cow::Borrowed(text);
        {
            let tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
            for token in tokens.iter() {
                if out.contains(token.as_str()) {
                    out = Cow::Owned(out.replace(token.as_str(), "<token>"));
                }
            }
        }
        for (re, with) in RULES.iter() {
            out = replace_regex(re, with, out);
        }
        if let Some(rule) = &self.marshell_home {
            out = replace_folder(rule, out);
        }
        if let Some(rule) = &self.user_home {
            out = replace_folder(rule, out);
        }
        if let Some(re) = &self.user_name {
            out = replace_regex(re, "<user>", out);
        }
        out
    }
}

/// The redactor for this process: the user's profile folder, the Marshell home and the user name.
pub fn global() -> &'static Redactor {
    static GLOBAL: OnceLock<Redactor> = OnceLock::new();
    GLOBAL.get_or_init(|| {
        let name_var = if cfg!(windows) { "USERNAME" } else { "USER" };
        Redactor::new(dirs::home_dir(), paths::home_dir().ok(), std::env::var(name_var).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Redactor {
        Redactor::new(None, None, None)
    }

    #[test]
    fn redacts_quoted_and_escaped_values() {
        for input in [
            r#"$env:MY_TOKEN=\"abc123secret\""#,
            r#""password": "hunter 2 two""#,
            r"secret='hunter 2 two'",
            r"secret=\'hunter 2 two\'",
        ] {
            let out = plain().redact(input).into_owned();
            for secret in ["abc123secret", "hunter", "2 two"] {
                assert!(!out.contains(secret), "{input} -> {out}");
            }
        }
        assert_eq!(plain().redact("max_tokens: 4096"), "max_tokens: 4096");
    }

    #[test]
    fn redacts_secret_flags() {
        for input in [
            "--api-key abc123",
            "--token abc123",
            "--password=abc123",
            "--apikey abc123",
            "--access-token abc123",
            "--secret abc123",
            "--passwd abc123",
            r#"--token "abc123 with spaces""#,
            r#"--secret=\"abc123 with spaces\""#,
        ] {
            let out = plain().redact(input).into_owned();
            assert!(!out.contains("abc123"), "{input} -> {out}");
            assert!(!out.contains("with spaces"), "{input} -> {out}");
        }
        assert_eq!(plain().redact("--verbose abc"), "--verbose abc");
    }

    #[test]
    fn redacts_basic_authorization() {
        assert_eq!(
            plain().redact("Authorization: Basic dXNlcjpwYXNzd29yZA=="),
            "Authorization: Basic <redacted>"
        );
        assert_eq!(plain().redact("Basic dXNlcjpwYXNzd29yZA=="), "Basic <redacted>");
        assert_eq!(
            plain().redact("basic configuration of the app"),
            "basic configuration of the app"
        );
    }

    #[test]
    fn redacts_url_password_with_at() {
        let out = plain().redact("https://user:p@ss@host/x");
        assert!(!out.contains("ss@host"), "{out}");
        assert_eq!(out, "https://<redacted>@host/x");
    }

    #[test]
    fn redacts_known_key_shapes() {
        let r = plain();
        let cases = [
            ("a sk-ant-api03-abcdefgh b", "a <anthropic-key> b"),
            ("a sk-proj-abcdefghijklmnopqrstu b", "a <openai-key> b"),
            ("a sk-abcdefghijklmnopqrstuvwx b", "a <openai-key> b"),
            ("a ghp_abcdefghijklmnopqrstuvwx b", "a <github-token> b"),
            ("a github_pat_abcdefghijklmnopqrstuv b", "a <github-token> b"),
            ("a AKIAABCDEFGHIJKLMNOP b", "a <aws-key> b"),
            ("a xoxb-1234567890-abc b", "a <slack-token> b"),
            ("a eyJhbGciOiJI.eyJzdWIiOiIx.SflKxwRJSMeK b", "a <jwt> b"),
            (
                "Authorization: Bearer abcdef0123456789",
                "Authorization: Bearer <redacted>",
            ),
        ];
        for (input, want) in cases {
            assert_eq!(r.redact(input), want, "{input}");
        }
    }

    #[test]
    fn redacts_key_value_pairs_and_url_credentials() {
        let r = plain();
        let out = r.redact("ANTHROPIC_API_KEY=sk-x run");
        assert!(!out.contains("sk-x"), "{out}");
        assert!(out.contains("ANTHROPIC_API_KEY=<redacted>"), "{out}");
        assert_eq!(
            r.redact("https://h/p?token=abc123&x=1"),
            "https://h/p?token=<redacted>&x=1"
        );
        assert_eq!(r.redact(r#""password": "hunter2""#), r#""password": "<redacted>""#);
        assert_eq!(r.redact("https://me:pw@host/x"), "https://<redacted>@host/x");
        assert_eq!(
            r.redact("claude --flag ANTHROPIC_API_KEY=abc"),
            "claude --flag ANTHROPIC_API_KEY=<redacted>"
        );
    }

    #[test]
    fn redacts_remembered_tokens() {
        let r = plain();
        r.remember_token("0123abcd4567");
        assert_eq!(r.redact("GET /x 0123abcd4567 ok"), "GET /x <token> ok");
        r.remember_token("");
        assert_eq!(r.redact("nothing here"), "nothing here");
    }

    #[test]
    fn redacts_the_home_folder_in_every_spelling() {
        let r = Redactor::new(
            Some(PathBuf::from(r"C:\Users\Home Office")),
            Some(PathBuf::from(r"D:\mh")),
            None,
        );
        for (input, want) in [
            (r"C:\Users\Home Office\x", r"~\x"),
            ("c:/users/home office/x", "~/x"),
            (r"C:\\Users\\Home Office\\x", r"~\\x"),
            (r"\\?\C:\Users\Home Office\x", r"~\x"),
            (r#"open "C:\Users\Home Office""#, r#"open "~""#),
            (r"D:\mh\logs", r"<marshell-home>\logs"),
        ] {
            assert_eq!(r.redact(input), want, "{input}");
        }
        // A longer folder name is not the home.
        assert_eq!(r.redact(r"C:\Users\Home Office2\x"), r"C:\Users\Home Office2\x");
    }

    #[test]
    fn a_marshell_home_inside_the_user_home_is_just_a_tilde_path() {
        let r = Redactor::new(
            Some(PathBuf::from(r"C:\Users\me")),
            Some(PathBuf::from(r"C:\Users\me\.marshell")),
            None,
        );
        assert_eq!(r.redact(r"C:\Users\me\.marshell\logs"), r"~\.marshell\logs");
    }

    #[test]
    fn redacts_the_user_name_as_a_word() {
        let r = Redactor::new(None, None, Some("Home Office".into()));
        assert_eq!(r.redact("by Home Office today"), "by <user> today");
        let short = Redactor::new(None, None, Some("al".into()));
        assert_eq!(short.redact("also al"), "also al");
    }

    #[test]
    fn leaves_plain_text_alone() {
        let r = Redactor::new(
            Some(PathBuf::from(r"C:\Users\me")),
            Some(PathBuf::from(r"D:\mh")),
            Some("someone".into()),
        );
        r.remember_token("zzzzzzzz");
        assert!(matches!(r.redact("a plain line, nothing to hide"), Cow::Borrowed(_)));
    }
}
