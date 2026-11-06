//! Qwen Code (`qwen`): its hook entries and status line, planned as config changes in its user `settings.json`.

pub mod extensions;
pub mod hooks;
pub mod request;
pub mod status;
pub mod transcript;
pub mod usage;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Qwen's home itself, never an extra `.qwen` below `QWEN_HOME`. Relative overrides stay relative.
pub fn home(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    let value = env("QWEN_HOME").unwrap_or_default();
    let value = value.trim();
    if value.is_empty() {
        user_home.join(".qwen")
    } else {
        expand_home(value, user_home)
    }
}

/// A folder from a Qwen variable (`QWEN_HOME`, `QWEN_RUNTIME_DIR`) as Qwen's `Storage.resolvePath` reads it: `~`,
/// `~/x` and `~\x` are under the home; anything else, a relative path included, is taken as written.
pub fn expand_home(value: &str, user_home: &Path) -> PathBuf {
    if value == "~" {
        user_home.to_path_buf()
    } else if let Some(tail) = value.strip_prefix("~/").or_else(|| value.strip_prefix(r"~\")) {
        user_home.join(tail)
    } else {
        PathBuf::from(value)
    }
}

/// The auth method chosen in settings, without claiming the person is signed in.
pub fn auth_label(selected: &str) -> String {
    match selected {
        "qwen-oauth" => "Qwen OAuth".into(),
        "openai" => "an OpenAI-compatible API".into(),
        "openai-responses" => "the OpenAI Responses API".into(),
        "anthropic" => "an Anthropic API".into(),
        "gemini" => "a Gemini API".into(),
        "vertex-ai" => "Vertex AI".into(),
        other => format!("`{other}`"),
    }
}

pub use super::common::Gate;

/// The Qwen Code versions whose hook events and status line are recorded in
/// `tests/fixtures/qwen/{hooks,statusline}/<version>/`. One entry per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(0, 26, 5)];

/// Known: the same minor line as a recorded fixture set, and not older than its oldest patch.
pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Qwen Code", FIXTURE_VERSIONS, version)
}

/// Qwen Code's read-only system settings file on `os` (a `std::env::consts::OS` value), from the 6l2 research note.
pub fn system_settings_for(os: &str) -> &'static str {
    match os {
        "macos" => "/Library/Application Support/QwenCode/settings.json",
        "linux" => "/etc/qwen-code/settings.json",
        _ => r"C:\ProgramData\qwen-code\settings.json",
    }
}

/// The first word `qwen --version` prints: `x.y.z`, optionally followed by a non-empty `-<suffix>`.
pub fn parse_version(stdout: &str) -> Option<String> {
    let first = stdout.split_whitespace().next()?;
    let core = match first.split_once('-') {
        Some((_, "")) => return None,
        Some((core, _)) => core,
        None => first,
    };
    let parts: Vec<&str> = core.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then(|| first.to_string())
}

/// What `UserPromptSubmit` answers to add `note` for the model.
pub fn context_output(note: &str) -> Value {
    json!({ "hookSpecificOutput": { "hookEventName": "UserPromptSubmit", "additionalContext": note } })
}

/// The setting that skips every prompt, when Qwen's user settings enable it. There is none that a Marshell launch
/// would miss: the `--approval-mode` flag wins over `tools.approvalMode` in the settings (the 6l1 research note, Q11).
pub fn skips_all(_user_home: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_expands_a_leading_tilde() {
        let user = Path::new("C:/Users/test user");
        for value in [r"~\q", "~/q"] {
            assert_eq!(home(&|_| Some(value.into()), user), user.join("q"));
        }
        assert_eq!(home(&|_| Some("~".into()), user), user);
    }

    #[test]
    fn home_ignores_a_blank_value_and_takes_others_as_given() {
        let user = Path::new("C:/Users/test user");
        assert_eq!(home(&|_| None, user), user.join(".qwen"));
        for value in ["", " \t "] {
            assert_eq!(home(&|_| Some(value.into()), user), user.join(".qwen"));
        }
        for (value, want) in [
            (" D:/q/home ", "D:/q/home"),
            ("relative/q", "relative/q"),
            ("~other/q", "~other/q"),
        ] {
            assert_eq!(home(&|_| Some(value.into()), user), Path::new(want));
        }
    }

    #[test]
    fn a_preview_or_nightly_version_is_read() {
        for version in ["0.27.0-preview.1", "0.27.0-nightly.20261101.abc"] {
            assert_eq!(parse_version(version), Some(version.into()));
            assert_eq!(
                version_gate(Some(version)),
                Gate::Unknown(format!("Qwen Code {version} is not one Marshell has recorded yet"))
            );
        }
        for version in ["0.26.5-preview.1", "0.26.6-nightly.20261101"] {
            match version_gate(Some(version)) {
                Gate::Unknown(note) => assert!(note.contains(version), "{note}"),
                Gate::Known => panic!("{version} must not be Known"),
            }
        }
        assert_eq!(parse_version("0.27.0-"), None);
    }

    #[test]
    fn the_system_settings_file_depends_on_the_os() {
        assert_eq!(
            system_settings_for("windows"),
            r"C:\ProgramData\qwen-code\settings.json"
        );
        assert_eq!(
            system_settings_for("macos"),
            "/Library/Application Support/QwenCode/settings.json"
        );
        assert_eq!(system_settings_for("linux"), "/etc/qwen-code/settings.json");
    }

    #[test]
    fn auth_labels() {
        for (selected, label) in [
            ("qwen-oauth", "Qwen OAuth"),
            ("openai", "an OpenAI-compatible API"),
            ("openai-responses", "the OpenAI Responses API"),
            ("anthropic", "an Anthropic API"),
            ("gemini", "a Gemini API"),
            ("vertex-ai", "Vertex AI"),
            ("future", "`future`"),
        ] {
            assert_eq!(auth_label(selected), label);
        }
    }

    #[test]
    fn qwen_0_26_5_is_known() {
        for v in ["0.26.5", "0.26.9"] {
            assert_eq!(version_gate(Some(v)), Gate::Known, "{v}");
        }
    }

    #[test]
    fn a_later_minor_is_not_recorded() {
        assert_eq!(
            version_gate(Some("0.27.0")),
            Gate::Unknown("Qwen Code 0.27.0 is not one Marshell has recorded yet".to_string())
        );
    }

    #[test]
    fn an_earlier_version_is_older() {
        for v in ["0.26.4", "0.25.9"] {
            assert_eq!(
                version_gate(Some(v)),
                Gate::Unknown(format!("Qwen Code {v} is older than the versions Marshell knows")),
                "{v}"
            );
        }
    }

    #[test]
    fn an_unreadable_version_has_its_note() {
        assert_eq!(
            version_gate(None),
            Gate::Unknown("Qwen Code's version could not be read".to_string())
        );
    }

    #[test]
    fn the_context_output_is_additional_context() {
        assert_eq!(
            context_output("References: 'a' = /r/a."),
            json!({"hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": "References: 'a' = /r/a.",
            }})
        );
    }

    #[test]
    fn parse_version_reads_the_bare_version() {
        assert_eq!(parse_version("0.26.5\n"), Some("0.26.5".to_string()));
        assert_eq!(parse_version("  0.26.5\r\n"), Some("0.26.5".to_string()));
        assert_eq!(parse_version("v0.26.5"), None);
        assert_eq!(parse_version("0.26"), None);
        assert_eq!(parse_version("garbage"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn no_setting_skips_every_prompt() {
        assert_eq!(skips_all(Path::new("C:/Users/test")), None);
    }
}
