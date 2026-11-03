//! Qwen Code (`qwen`): its hook entries and status line, planned as config changes in its user `settings.json`.

pub mod hooks;
pub mod request;
pub mod status;
pub mod transcript;

use serde_json::{Value, json};
use std::path::Path;

pub use super::common::Gate;

/// The Qwen Code versions whose hook events and status line are recorded in
/// `tests/fixtures/qwen/{hooks,statusline}/<version>/`. One entry per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(0, 26, 5)];

/// Known: the same minor line as a recorded fixture set, and not older than its oldest patch.
pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Qwen Code", FIXTURE_VERSIONS, version)
}

/// The version `qwen --version` prints: the bare `0.26.5`. `None` when the output holds no `x.y.z` first word.
pub fn parse_version(stdout: &str) -> Option<String> {
    let first = stdout.split_whitespace().next()?;
    let parts: Vec<&str> = first.split('.').collect();
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
