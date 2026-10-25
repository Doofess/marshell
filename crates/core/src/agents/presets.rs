//! Presets: a way to start an agent CLI that only changes its environment. DeepSeek runs on Claude Code through
//! DeepSeek's Anthropic-compatible endpoint. The row's agent stays "claude", so every hook, status and gate is unchanged.
//!
//! The key is read from `DEEPSEEK_API_KEY` at launch and goes into the one child's environment, as
//! `ANTHROPIC_AUTH_TOKEN`. Nothing here stores, logs or returns it: `PresetLaunch` has no `Debug`.

use marshell_protocol::api::ShellFamily;

pub const DEEPSEEK: &str = "deepseek";

/// The environment variable the key is read from.
pub const DEEPSEEK_KEY_VAR: &str = "DEEPSEEK_API_KEY";

pub const NO_KEY: &str = "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.";
pub const NOT_ON_WSL: &str = "DeepSeek starts from a Windows shell profile, not WSL.";
pub const CLAUDE_ONLY: &str = "DeepSeek runs on Claude Code only.";

/// What a preset adds to the child's environment, and what it takes away.
pub struct PresetLaunch {
    pub env: Vec<(String, String)>,
    pub env_remove: Vec<String>,
}

/// The variables DeepSeek documents for Claude Code (https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code/,
/// checked 2026-10-25), without the key.
const DEEPSEEK_ENV: [(&str, &str); 8] = [
    ("ANTHROPIC_BASE_URL", "https://api.deepseek.com/anthropic"),
    ("ANTHROPIC_MODEL", "deepseek-flash[1m]"),
    ("ANTHROPIC_DEFAULT_OPUS_MODEL", "deepseek-flash[1m]"),
    ("ANTHROPIC_DEFAULT_SONNET_MODEL", "deepseek-flash[1m]"),
    ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "deepseek-flash"),
    ("CLAUDE_CODE_SUBAGENT_MODEL", "deepseek-flash"),
    ("CLAUDE_CODE_EFFORT_LEVEL", "max"),
    ("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "786432"),
];

/// Variables that would point Claude Code at another provider, or send another credential to DeepSeek.
const ALWAYS_REMOVED: [&str; 3] = ["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", DEEPSEEK_KEY_VAR];

const TOKEN_VAR: &str = "ANTHROPIC_AUTH_TOKEN";

/// Whether a preset launch drops this variable from the environment it inherits: any `ANTHROPIC_*` (an
/// `ANTHROPIC_API_KEY` must not reach api.deepseek.com) and the two switches to other providers. Names compare
/// without case, as the Windows environment does.
pub fn strips(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("ANTHROPIC_") || ALWAYS_REMOVED.contains(&upper.as_str())
}

fn preset_sets(name: &str) -> bool {
    name.eq_ignore_ascii_case(TOKEN_VAR) || DEEPSEEK_ENV.iter().any(|(k, _)| name.eq_ignore_ascii_case(k))
}

/// The DeepSeek launch. `key` is `DEEPSEEK_API_KEY` as read now (empty or blank counts as unset), `parent` the names of
/// Marshell's own environment variables.
pub fn deepseek(key: Option<String>, family: ShellFamily, parent: &[String]) -> Result<PresetLaunch, String> {
    if family == ShellFamily::Wsl {
        return Err(NOT_ON_WSL.into());
    }
    let key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    let Some(key) = key else {
        return Err(NO_KEY.into());
    };
    let mut env = vec![(TOKEN_VAR.to_string(), key)];
    env.extend(DEEPSEEK_ENV.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let mut env_remove: Vec<String> = Vec::new();
    for name in parent.iter().map(String::as_str).chain(ALWAYS_REMOVED) {
        if strips(name) && !preset_sets(name) && !env_remove.iter().any(|r| r.eq_ignore_ascii_case(name)) {
            env_remove.push(name.to_string());
        }
    }
    Ok(PresetLaunch { env, env_remove })
}

/// The reason the preset can't start now, for the catalog. The key itself is not touched beyond "is it set".
pub fn problem(id: &str, key: Option<String>) -> Option<String> {
    match id {
        DEEPSEEK => key
            .map(|k| k.trim().is_empty())
            .unwrap_or(true)
            .then(|| NO_KEY.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-marshell-test-0000";

    fn parent(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn deepseek_env_is_the_documented_set() {
        let launch = deepseek(Some(KEY.into()), ShellFamily::PowerShell, &[]).unwrap();
        let get = |k: &str| launch.env.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(launch.env.len(), 9);
        assert_eq!(get("ANTHROPIC_BASE_URL"), Some("https://api.deepseek.com/anthropic"));
        assert_eq!(get("ANTHROPIC_AUTH_TOKEN"), Some(KEY));
        assert_eq!(get("ANTHROPIC_MODEL"), Some("deepseek-flash[1m]"));
        assert_eq!(get("ANTHROPIC_DEFAULT_OPUS_MODEL"), Some("deepseek-flash[1m]"));
        assert_eq!(get("ANTHROPIC_DEFAULT_SONNET_MODEL"), Some("deepseek-flash[1m]"));
        assert_eq!(get("ANTHROPIC_DEFAULT_HAIKU_MODEL"), Some("deepseek-flash"));
        assert_eq!(get("CLAUDE_CODE_SUBAGENT_MODEL"), Some("deepseek-flash"));
        assert_eq!(get("CLAUDE_CODE_EFFORT_LEVEL"), Some("max"));
        assert_eq!(get("CLAUDE_CODE_AUTO_COMPACT_WINDOW"), Some("786432"));
        // The token is the only place the key goes.
        assert_eq!(launch.env.iter().filter(|(_, v)| v.contains(KEY)).count(), 1);
        // The switches to other providers are always dropped, and so is the key's own variable.
        for name in ["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "DEEPSEEK_API_KEY"] {
            assert!(launch.env_remove.iter().any(|n| n == name), "{name}");
        }
        // The same set on Posix and cmd.
        assert!(deepseek(Some(KEY.into()), ShellFamily::Posix, &[]).is_ok());
        assert!(deepseek(Some(KEY.into()), ShellFamily::Cmd, &[]).is_ok());
    }

    #[test]
    fn deepseek_refuses_without_a_key_and_on_wsl() {
        for key in [None, Some(String::new()), Some("  ".into())] {
            let err = deepseek(key, ShellFamily::PowerShell, &[]).err().unwrap();
            assert_eq!(err, "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.");
        }
        let err = deepseek(Some(KEY.into()), ShellFamily::Wsl, &[]).err().unwrap();
        assert_eq!(err, "DeepSeek starts from a Windows shell profile, not WSL.");
        assert!(!err.contains(KEY));
        assert_eq!(problem(DEEPSEEK, None).as_deref(), Some(NO_KEY));
        assert_eq!(problem(DEEPSEEK, Some("".into())).as_deref(), Some(NO_KEY));
        assert_eq!(problem(DEEPSEEK, Some(KEY.into())), None);
    }

    #[test]
    fn inherited_anthropic_variables_are_stripped() {
        let inherited = parent(&[
            "PATH",
            "ANTHROPIC_API_KEY",
            "Anthropic_Custom_Headers",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_MODEL",
        ]);
        let launch = deepseek(Some(KEY.into()), ShellFamily::PowerShell, &inherited).unwrap();
        for name in ["ANTHROPIC_API_KEY", "Anthropic_Custom_Headers"] {
            assert!(launch.env_remove.iter().any(|n| n == name), "{name}");
            assert!(!launch.env.iter().any(|(n, _)| n == name), "{name}");
        }
        // What the preset sets is replaced, not removed (removal comes first, and would hide a value if it came last).
        for name in ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL", "PATH"] {
            assert!(!launch.env_remove.iter().any(|n| n == name), "{name}");
        }
        assert!(!launch.env.iter().any(|(_, v)| v == "ANTHROPIC_API_KEY"));
        assert!(strips("anthropic_api_key"));
        assert!(!strips("CLAUDE_CODE_GIT_BASH_PATH"));
    }
}
