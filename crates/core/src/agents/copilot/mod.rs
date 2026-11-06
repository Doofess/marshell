//! GitHub Copilot CLI (`copilot`): its own hooks file and its status line, planned as config changes.

pub mod hooks;
pub mod status;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub use super::common::Gate;

/// `COPILOT_HOME` when it is set and not blank, otherwise `<user home>/.copilot`.
pub fn home(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    let value = env("COPILOT_HOME").unwrap_or_default();
    let value = value.trim();
    if value.is_empty() {
        user_home.join(".copilot")
    } else {
        PathBuf::from(value)
    }
}

/// The Copilot CLI versions whose hook events and status line are recorded in `tests/fixtures/copilot/`. One entry
/// per minor line: its oldest recorded patch.
pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(1, 0, 106)];

pub fn version_gate(version: Option<&str>) -> Gate {
    super::common::version_gate("Copilot CLI", FIXTURE_VERSIONS, version)
}

/// The `x.y.z` after `GitHub Copilot CLI ` on the first line of `copilot --version`, without the full stop.
pub fn parse_version(stdout: &str) -> Option<String> {
    let rest = stdout.lines().next()?.trim().strip_prefix("GitHub Copilot CLI ")?;
    let version = rest.split_whitespace().next()?.trim_end_matches('.');
    let core = version.split_once('-').map_or(version, |(core, _)| core);
    let parts: Vec<&str> = core.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then(|| version.to_string())
}

/// What a hook answers to add `note` for the model: `additionalContext` at the top level.
pub fn context_output(note: &str) -> Value {
    json!({ "additionalContext": note })
}

/// What makes Copilot skip its permission prompts in a launch with no mode flag, when it is set: the setting
/// `defaultPermissionMode` of `allow-all`, or a truthy `COPILOT_ALLOW_ALL`.
pub fn skips_all(settings_text: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let allow_all = settings_text
        .and_then(|text| crate::config::jsonc::read_value(text).ok())
        .is_some_and(|settings| settings.get("defaultPermissionMode").and_then(Value::as_str) == Some("allow-all"));
    if allow_all {
        return Some("defaultPermissionMode".to_string());
    }
    let truthy = env("COPILOT_ALLOW_ALL").is_some_and(|v| {
        let v = v.trim();
        v == "1" || v.eq_ignore_ascii_case("true")
    });
    truthy.then(|| "COPILOT_ALLOW_ALL".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_follows_copilot_home() {
        let user = Path::new("C:/Users/test user");
        assert_eq!(home(&|_| None, user), user.join(".copilot"));
        let env = |k: &str| (k == "COPILOT_HOME").then(|| " D:/c/home ".to_string());
        assert_eq!(home(&env, user), PathBuf::from("D:/c/home"));
    }

    #[test]
    fn a_blank_copilot_home_is_ignored() {
        let user = Path::new("C:/Users/test user");
        for value in ["", " \t "] {
            assert_eq!(home(&|_| Some(value.into()), user), user.join(".copilot"));
        }
    }

    #[test]
    fn copilot_1_0_106_is_known() {
        for v in ["1.0.106", "1.0.120"] {
            assert_eq!(version_gate(Some(v)), Gate::Known, "{v}");
        }
        assert_eq!(
            version_gate(None),
            Gate::Unknown("Copilot CLI's version could not be read".to_string())
        );
    }

    #[test]
    fn a_later_minor_is_not_recorded() {
        assert_eq!(
            version_gate(Some("1.1.0")),
            Gate::Unknown("Copilot CLI 1.1.0 is not one Marshell has recorded yet".to_string())
        );
    }

    #[test]
    fn an_earlier_patch_is_older() {
        assert_eq!(
            version_gate(Some("1.0.105")),
            Gate::Unknown("Copilot CLI 1.0.105 is older than the versions Marshell knows".to_string())
        );
    }

    #[test]
    fn parse_version_drops_the_full_stop() {
        let out = "GitHub Copilot CLI 1.0.106.\nRun 'copilot update' to check for updates.\n";
        assert_eq!(parse_version(out), Some("1.0.106".to_string()));
        assert_eq!(
            parse_version("GitHub Copilot CLI 1.0.107-0.\r\nmore"),
            Some("1.0.107-0".to_string())
        );
        for bad in [
            "",
            "1.0.106",
            "GitHub Copilot CLI .",
            "GitHub Copilot CLI 1.0.\n",
            "other\nGitHub Copilot CLI 1.0.106.",
        ] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_context_output_is_top_level_additional_context() {
        assert_eq!(
            context_output("References: 'a' = /r/a."),
            json!({"additionalContext": "References: 'a' = /r/a."})
        );
    }

    #[test]
    fn skips_all_reads_the_setting_and_the_env() {
        let no_env = |_: &str| None;
        let env = |v: &'static str| move |k: &str| (k == "COPILOT_ALLOW_ALL").then(|| v.to_string());
        assert_eq!(
            skips_all(Some(r#"{"defaultPermissionMode": "allow-all"}"#), &no_env).as_deref(),
            Some("defaultPermissionMode")
        );
        // JSONC: a comment, and a BOM.
        assert_eq!(
            skips_all(
                Some("\u{feff}// mine\n{\n  \"defaultPermissionMode\": \"allow-all\" // yes\n}\n"),
                &no_env
            )
            .as_deref(),
            Some("defaultPermissionMode")
        );
        for text in [
            r#"{"defaultPermissionMode": "manual"}"#,
            r#"{"defaultPermissionMode": "assisted"}"#,
            r#"{"defaultPermissionMode": true}"#,
            r#"{"ui": {"defaultPermissionMode": "allow-all"}}"#,
            "{}",
            "",
            "  \n",
            "not json {",
        ] {
            assert_eq!(skips_all(Some(text), &no_env), None, "{text:?}");
        }
        assert_eq!(skips_all(None, &no_env), None);
        for v in ["true", "TRUE", "True", "1", " 1 "] {
            assert_eq!(skips_all(None, &env(v)).as_deref(), Some("COPILOT_ALLOW_ALL"), "{v}");
        }
        for v in ["", "0", "false", "yes", "2"] {
            assert_eq!(skips_all(None, &env(v)), None, "{v}");
        }
        // The setting is named first when both are there.
        assert_eq!(
            skips_all(Some(r#"{"defaultPermissionMode":"allow-all"}"#), &env("true")).as_deref(),
            Some("defaultPermissionMode")
        );
    }
}
