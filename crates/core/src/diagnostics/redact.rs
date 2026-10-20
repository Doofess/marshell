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
    marshell_home: Vec<FolderRule>,
    user_home: Vec<FolderRule>,
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
    let boundary = if parts[0].len() == 2 && parts[0].ends_with(':') {
        r"[^A-Za-z0-9_.\-\\]"
    } else {
        r"[^A-Za-z0-9_.\-/\\]"
    };
    let pattern = format!(r"(?i)(?P<pre>^|{boundary})(?:\\{{2,4}}\?\\{{1,2}})?{root}{body}(?P<post>[^A-Za-z0-9_]|$)");
    Some(FolderRule {
        matcher: Regex::new(&pattern).ok()?,
        placeholder,
        parts: parts.iter().map(|p| p.to_lowercase()).collect(),
    })
}

fn replace_folder<'a>(rule: &FolderRule, mut text: Cow<'a, str>) -> Cow<'a, str> {
    // A closure: `$post` in a template would read the placeholder's `<` wrongly.
    // Repeat so a restored boundary can also precede the next occurrence; a copy next to another is done in
    // two passes, and the cap only guards against a placeholder that matches again.
    for _ in 0..16 {
        let replaced = rule.matcher.replace_all(&text, |c: &Captures| {
            format!("{}{}{}", &c["pre"], rule.placeholder, &c["post"])
        });
        if replaced == text {
            return text;
        }
        text = Cow::Owned(replaced.into_owned());
    }
    text
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
        let user_name = user_name
            .map(|n| n.trim().to_string())
            .filter(|n| n.chars().count() >= 3)
            .and_then(|n| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(&n))).ok());
        Redactor {
            tokens: Mutex::new(Vec::new()),
            marshell_home: Vec::new(),
            user_home: Vec::new(),
            user_name,
        }
        .with_folder_aliases(user_home, marshell_home)
    }

    /// Adds known spellings; only `global` discovers aliases from the filesystem.
    fn with_folder_aliases(
        mut self,
        user_home: impl IntoIterator<Item = PathBuf>,
        marshell_home: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        self.user_home
            .extend(user_home.into_iter().filter_map(|p| folder_rule(&p, "~")));
        self.marshell_home.extend(
            marshell_home
                .into_iter()
                .filter_map(|p| folder_rule(&p, "<marshell-home>")),
        );
        // Inside any spelling of the user's home, the `~` rule already covers it.
        self.marshell_home
            .retain(|m| !self.user_home.iter().any(|u| m.parts.starts_with(&u.parts)));
        self
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
        for rule in &self.marshell_home {
            out = replace_folder(rule, out);
        }
        for rule in &self.user_home {
            out = replace_folder(rule, out);
        }
        if let Some(re) = &self.user_name {
            out = replace_regex(re, "<user>", out);
        }
        out
    }
}

/// The existing Windows 8.3 spelling, when available and different from the given path.
#[cfg(windows)]
fn short_path(p: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let mut wide: Vec<u16> = p.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return None;
    }
    wide.push(0);
    // SAFETY: `wide` is NUL-terminated and valid for the call. A null output with size zero
    // asks for the required buffer length, including the terminating NUL.
    let size = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u16; size as usize];
    // SAFETY: the input remains valid and `buf` has `size` writable UTF-16 units.
    let len = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), size) };
    if len == 0 || len >= size {
        return None;
    }
    let short = PathBuf::from(OsString::from_wide(&buf[..len as usize]));
    (short != p).then_some(short)
}

#[cfg(not(windows))]
fn short_path(_: &Path) -> Option<PathBuf> {
    None
}

/// The redactor for this process: the user's profile folder, the Marshell home and the user name.
pub fn global() -> &'static Redactor {
    static GLOBAL: OnceLock<Redactor> = OnceLock::new();
    GLOBAL.get_or_init(|| {
        let name_var = if cfg!(windows) { "USERNAME" } else { "USER" };
        let user_home = dirs::home_dir();
        let marshell_home = paths::home_dir().ok();
        let user_short = user_home.as_deref().and_then(short_path);
        let marshell_short = marshell_home.as_deref().and_then(short_path);
        Redactor::new(user_home, marshell_home, std::env::var(name_var).ok())
            .with_folder_aliases(user_short, marshell_short)
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
    fn redacts_repeated_and_url_spellings_of_the_home() {
        let r = Redactor::new(Some(PathBuf::from(r"C:\Users\Jane Doe")), None, None);
        for (input, want) in [
            (r"C:\Users\Jane Doe;C:\Users\Jane Doe", "~;~"),
            ("file:///C:/Users/Jane Doe/x", "file:///~/x"),
            (r"\\\\?\\C:\\Users\\Jane Doe", "~"),
            (r"C:\Users\Jane Doe2", r"C:\Users\Jane Doe2"),
        ] {
            assert_eq!(r.redact(input), want, "{input}");
        }
    }

    #[test]
    fn redacts_the_short_form_of_the_home() {
        let r = Redactor::new(Some(PathBuf::from(r"C:\Users\Jane Doe")), None, None)
            .with_folder_aliases([PathBuf::from(r"C:\Users\JANEDO~1")], []);
        for input in [r"C:\Users\JANEDO~1\AppData\x", r"c:\users\janedo~1\AppData\x"] {
            assert_eq!(r.redact(input), r"~\AppData\x");
        }
        let r = Redactor::new(
            Some(PathBuf::from(r"C:\Users\Jane Doe")),
            Some(PathBuf::from(r"D:\Marshell Home")),
            None,
        )
        .with_folder_aliases([PathBuf::from(r"C:\Users\JANEDO~1")], [PathBuf::from(r"D:\MARSHE~1")]);
        assert_eq!(r.redact(r"D:\MARSHE~1\logs"), r"<marshell-home>\logs");
    }

    #[cfg(windows)]
    #[test]
    fn redacts_a_windows_short_path() {
        let temp = tempfile::tempdir().unwrap();
        let long = temp.path().join("folder with spaces");
        std::fs::create_dir(&long).unwrap();
        if let Some(short) = short_path(&long) {
            assert_ne!(short, long);
            let r = Redactor::new(Some(long), None, None).with_folder_aliases([short.clone()], []);
            assert_eq!(r.redact(&short.to_string_lossy()), "~");
        }
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
