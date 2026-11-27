//! Where each CLI keeps its files, for one level (a user home, or a distro's).

use crate::doctor::ext::ExtRoots;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Roots {
    pub claude: PathBuf,
    pub codex: PathBuf,
    pub gemini: PathBuf,
    pub qwen: PathBuf,
    pub copilot: PathBuf,
    /// The level's user home, which its paths are shown under as `~/…`.
    pub home: PathBuf,
}

impl Roots {
    /// Each CLI's default folder under `home`.
    pub fn under(home: &Path) -> Roots {
        Roots {
            claude: home.join(".claude"),
            codex: home.join(".codex"),
            gemini: home.join(".gemini"),
            qwen: home.join(".qwen"),
            copilot: home.join(".copilot"),
            home: home.to_path_buf(),
        }
    }

    pub fn from_env(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> Roots {
        let set = |key: &str| env(key).filter(|v| !v.trim().is_empty()).map(PathBuf::from);
        let mut roots = Roots::under(user_home);
        if let Some(p) = set("CLAUDE_CONFIG_DIR") {
            roots.claude = p;
        }
        if let Some(p) = set("CODEX_HOME") {
            roots.codex = p;
        }
        roots.qwen = crate::agents::qwen::home(env, user_home);
        roots.copilot = crate::agents::copilot::home(env, user_home);
        roots
    }

    pub fn from_ext(r: &ExtRoots) -> Roots {
        Roots {
            claude: r.claude_dir.clone(),
            codex: r.codex_home.clone(),
            gemini: r.gemini_dir.clone(),
            qwen: r.qwen_dir.clone(),
            copilot: r.copilot_dir.clone(),
            home: r.home.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_from_env() {
        let home = Path::new("/h");
        let none = Roots::from_env(&|_| None, home);
        assert_eq!(none, Roots::under(home));
        assert_eq!(none.gemini, home.join(".gemini"));
        let env = |k: &str| (k == "CODEX_HOME").then(|| "/elsewhere".to_string());
        let r = Roots::from_env(&env, home);
        assert_eq!(r.codex, PathBuf::from("/elsewhere"));
        assert_eq!(
            Roots {
                codex: home.join(".codex"),
                ..r
            },
            Roots::under(home)
        );
        let blank = Roots::from_env(&|_| Some("  ".into()), home);
        assert_eq!(blank, Roots::under(home));
    }
}
