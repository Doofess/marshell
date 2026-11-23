//! The settings Changes takes from a user's git config, and no others.
//!
//! The sandbox git folder reads no config file but its own (spike S15), so the few settings that change
//! how a file compares are read from the system, user and repository files and written to the sandbox's
//! command line. Only a key in `ALLOWED` with a value it accepts is passed on; a filter, a pager, an
//! alias or a path never is.

/// The values a key accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    /// One of these words.
    Words(&'static [&'static str]),
    /// A git boolean: true, false, yes, no, on, off, 1 or 0.
    Bool,
    /// A git boolean or one of these words.
    BoolOr(&'static [&'static str]),
    /// A whole number in this range, written as plain digits.
    Int(u32, u32),
}

const BOOLEANS: &[&str] = &["true", "false", "yes", "no", "on", "off", "1", "0"];

impl Allowed {
    /// Whether `value` is accepted. Words match in any case.
    pub fn accepts(&self, value: &str) -> bool {
        let is_word = |words: &[&str]| words.iter().any(|w| w.eq_ignore_ascii_case(value));
        match self {
            Allowed::Words(words) => is_word(words),
            Allowed::Bool => is_word(BOOLEANS),
            Allowed::BoolOr(words) => is_word(BOOLEANS) || is_word(words),
            Allowed::Int(low, high) => {
                !value.is_empty()
                    && value.len() <= 10
                    && value.bytes().all(|b| b.is_ascii_digit())
                    && value.parse::<u32>().is_ok_and(|n| (*low..=*high).contains(&n))
            }
        }
    }
}

/// Key (lowercase, as git prints it) and the values it accepts.
pub const ALLOWED: &[(&str, Allowed)] = &[
    ("core.autocrlf", Allowed::BoolOr(&["input"])),
    ("core.eol", Allowed::Words(&["lf", "crlf", "native"])),
    ("core.ignorecase", Allowed::Bool),
    ("core.symlinks", Allowed::Bool),
    ("core.filemode", Allowed::Bool),
    ("core.precomposeunicode", Allowed::Bool),
    ("core.trustctime", Allowed::Bool),
    ("core.longpaths", Allowed::Bool),
    ("core.fscache", Allowed::Bool),
    ("diff.indentheuristic", Allowed::Bool),
    ("core.checkstat", Allowed::Words(&["default", "minimal"])),
    ("diff.renames", Allowed::BoolOr(&["copies", "copy"])),
    // 1000 is git's default; a higher limit can keep `-M` past the deadline on every poll.
    ("diff.renamelimit", Allowed::Int(0, 1000)),
    (
        "diff.algorithm",
        Allowed::Words(&["myers", "minimal", "patience", "histogram"]),
    ),
];

/// Keys read from the repository's own file to learn its format. They are never passed on.
pub const FORMAT_KEYS: &[&str] = &[
    "core.repositoryformatversion",
    "extensions.objectformat",
    "extensions.refstorage",
    "extensions.worktreeconfig",
];

/// The extended regular expression for `git config --get-regexp`: every allowed key and every format key.
pub fn key_regex() -> String {
    let keys: Vec<String> = ALLOWED
        .iter()
        .map(|(key, _)| *key)
        .chain(FORMAT_KEYS.iter().copied())
        .map(|key| key.replace('.', "\\."))
        .collect();
    format!("^({})$", keys.join("|"))
}

/// The entries of `git config -z --get-regexp` output: `key`, a newline, `value`, a NUL. A key with no
/// newline is written without a value, which git reads as true.
pub fn parse_z(out: &[u8]) -> Vec<(String, String)> {
    out.split(|b| *b == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let text = String::from_utf8_lossy(entry);
            match text.split_once('\n') {
                Some((key, value)) => (key.to_string(), value.to_string()),
                None => (text.into_owned(), "true".to_string()),
            }
        })
        .collect()
}

/// The pair as it is passed on, or None when the key isn't allowed or the value isn't one it accepts.
/// Every allowed key is a two-part name, so lowercasing the key is what git's own spelling already is.
pub fn allowed_pair(key: &str, value: &str) -> Option<(String, String)> {
    let key = key.to_ascii_lowercase();
    let (_, rule) = ALLOWED.iter().find(|(k, _)| *k == key)?;
    rule.accepts(value).then(|| (key, value.to_string()))
}

/// Whether a git boolean reads as true.
pub fn is_true(value: &str) -> bool {
    ["true", "yes", "on", "1"].iter().any(|w| w.eq_ignore_ascii_case(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(key: &str, value: &str) -> bool {
        allowed_pair(key, value).is_some()
    }

    #[test]
    fn words_match_in_any_case_and_only_those() {
        assert!(ok("core.autocrlf", "input"));
        assert!(ok("core.autocrlf", "TRUE"));
        // Git reads every boolean spelling here, not only true and false.
        for word in ["yes", "no", "on", "off", "1", "0"] {
            assert!(ok("core.autocrlf", word), "{word}");
        }
        assert!(!ok("core.autocrlf", "evil"));
        assert!(!ok("core.autocrlf", ""));
        assert!(ok("core.eol", "CRLF"));
        assert!(!ok("core.eol", "true"));
        assert!(ok("diff.algorithm", "histogram"));
        assert!(!ok("diff.algorithm", "evil"));
        assert!(ok("core.checkstat", "minimal"));
    }

    #[test]
    fn booleans_and_the_rename_words() {
        for v in ["true", "false", "yes", "no", "on", "off", "1", "0", "Yes"] {
            assert!(ok("core.filemode", v), "{v}");
            assert!(ok("diff.renames", v), "{v}");
        }
        assert!(!ok("core.filemode", "copies"));
        assert!(!ok("core.filemode", "2"));
        assert!(ok("diff.renames", "copies"));
        assert!(ok("diff.renames", "copy"));
        assert!(!ok("diff.renames", "all"));
    }

    #[test]
    fn the_rename_limit_is_a_plain_integer_up_to_1000() {
        assert!(ok("diff.renamelimit", "0"));
        assert!(ok("diff.renamelimit", "1000"));
        assert!(!ok("diff.renamelimit", "1001"));
        assert!(!ok("diff.renamelimit", "5000"));
        assert!(!ok("diff.renamelimit", "-1"));
        assert!(!ok("diff.renamelimit", "+5"));
        assert!(!ok("diff.renamelimit", "1k"));
        assert!(!ok("diff.renamelimit", "0x10"));
        assert!(!ok("diff.renamelimit", ""));
        assert!(!ok("diff.renamelimit", "99999999999999999999"));
    }

    #[test]
    fn other_keys_are_dropped() {
        for key in [
            "core.pager",
            "core.fsmonitor",
            "diff.external",
            "filter.evil.clean",
            "alias.x",
            "core.hookspath",
        ] {
            assert!(!ok(key, "true"), "{key}");
        }
        assert_eq!(
            allowed_pair("core.autoCRLF", "input"),
            Some(("core.autocrlf".into(), "input".into()))
        );
    }

    #[test]
    fn z_output_is_split_into_keys_and_values() {
        let out = b"core.autocrlf\ninput\0core.filemode\0diff.algorithm\nmy\ners\0";
        assert_eq!(
            parse_z(out),
            vec![
                ("core.autocrlf".into(), "input".into()),
                ("core.filemode".into(), "true".into()),
                ("diff.algorithm".into(), "my\ners".into()),
            ]
        );
        assert!(parse_z(b"").is_empty());
    }

    #[test]
    fn the_regex_names_every_allowed_and_format_key() {
        let re = key_regex();
        assert!(re.starts_with("^(") && re.ends_with(")$"));
        for (key, _) in ALLOWED {
            assert!(re.contains(&key.replace('.', "\\.")), "{key}");
        }
        for key in FORMAT_KEYS {
            assert!(re.contains(&key.replace('.', "\\.")), "{key}");
        }
    }
}
