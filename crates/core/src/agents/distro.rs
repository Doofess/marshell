//! Taking Marshell's hooks out of a WSL distro: one removal plan per agent, over the distro's own files.
//! Both the Agents dialog and `marshell uninstall` plan through here, so they never differ.

use std::io::ErrorKind;
use std::path::Path;

use marshell_protocol::config::ChangeSet;

use crate::agents::antigravity::hooks as agy;
use crate::agents::common::PlanError;
use crate::agents::{claude, codex, qwen};

/// The agents with hooks in a distro, in the order they are planned.
const AGENTS: [&str; 4] = ["claude", "codex", "antigravity", "qwen"];

/// A file's text, `None` when it is not there. Any other failure is reported against the file.
fn read_text(path: &Path) -> Result<Option<String>, PlanError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PlanError::DoesNotParse {
            path: path.to_path_buf(),
            problem: (1, e.to_string()),
        }),
    }
}

/// The plan to remove `agent`'s hooks from the distro whose home is `home` (a path under its share), and whether the
/// person's changed status line is left as it is. A distro's status line runs with `--no-chain`, so there is no chain.
/// The files are read on this thread: callers go through `spawn_blocking`.
pub(crate) fn remove_plan_of(
    agent: &str,
    home: &Path,
    marshell_home: &Path,
    distro: &str,
) -> Result<(ChangeSet, bool), PlanError> {
    match agent {
        "claude" => {
            let path = home.join(".claude").join("settings.json");
            let text = read_text(&path)?;
            let plan = claude::hooks::remove_plan(&path, home, text.as_deref(), None, None, Some(distro))?;
            Ok((plan.changes, plan.statusline_left))
        }
        "codex" => {
            let path = home.join(".codex").join("hooks.json");
            let text = read_text(&path)?;
            Ok((codex::hooks::remove_plan(&path, text.as_deref(), marshell_home)?, false))
        }
        "antigravity" => {
            let hooks_text = read_text(&agy::hooks_path(home))?;
            let settings_text = read_text(&agy::settings_path(home))?;
            let changes = agy::remove_plan(
                home,
                hooks_text.as_deref(),
                settings_text.as_deref(),
                None,
                None,
                Some(distro),
            )?;
            Ok((changes, false))
        }
        _ => {
            let path = home.join(".qwen").join("settings.json");
            let text = read_text(&path)?;
            let plan = qwen::hooks::remove_plan(&path, home, text.as_deref(), None, None, Some(distro))?;
            Ok((plan.changes, plan.statusline_left))
        }
    }
}

/// One entry per agent (`claude`, `codex`, `antigravity`, `qwen`): the plan to remove its hooks from the distro whose
/// home is `home`. `Ok(None)` means there is nothing of Marshell's to remove. A file that is refused or does not parse
/// is that agent's `Err`, so one bad file never hides the other agents' plans.
pub fn remove_plans(
    distro: &str,
    home: &Path,
    marshell_home: &Path,
) -> Vec<(&'static str, Result<Option<ChangeSet>, PlanError>)> {
    AGENTS
        .into_iter()
        .map(|agent| {
            let plan = remove_plan_of(agent, home, marshell_home, distro)
                .map(|(changes, _)| Some(changes).filter(|c| !c.edits.is_empty()));
            (agent, plan)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects;
    use serde_json::{Value, json};

    fn entry(agent: &str) -> Value {
        json!({
            "type": "command",
            "command": "/mnt/c/Users/x/.marshell/bin/marshell-hook.exe",
            "args": ["event", "--agent", agent, "--tag", "mshl1"],
            "timeout": 5,
        })
    }

    fn write(path: &Path, value: Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, value.to_string()).unwrap();
    }

    fn fake_home() -> tempfile::TempDir {
        let target = projects::canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        tempfile::tempdir_in(target).unwrap()
    }

    #[test]
    fn a_home_with_hooks_for_every_agent_has_four_plans() {
        let dir = fake_home();
        let home = dir.path();
        let grouped = |agent| json!({"hooks": {"Stop": [{"hooks": [entry(agent)]}]}});
        write(&home.join(".claude/settings.json"), grouped("claude"));
        write(&home.join(".codex/hooks.json"), grouped("codex"));
        write(&home.join(".qwen/settings.json"), grouped("qwen"));
        write(
            &agy::hooks_path(home),
            json!({"marshell": {"Stop": [entry("antigravity")]}}),
        );
        let plans = remove_plans("Ubuntu", home, &home.join("marshell"));
        let names: Vec<_> = plans.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["claude", "codex", "antigravity", "qwen"]);
        for (name, plan) in plans {
            assert!(matches!(plan, Ok(Some(_))), "{name}: {plan:?}");
        }
    }

    #[test]
    fn an_empty_home_has_nothing_to_remove() {
        let dir = fake_home();
        let plans = remove_plans("Ubuntu", dir.path(), &dir.path().join("marshell"));
        assert_eq!(plans.len(), 4);
        for (name, plan) in plans {
            assert!(matches!(plan, Ok(None)), "{name}: {plan:?}");
        }
    }

    #[test]
    fn a_file_that_does_not_parse_is_that_agents_error_only() {
        let dir = fake_home();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::write(home.join(".codex/hooks.json"), "{not json").unwrap();
        let plans = remove_plans("Ubuntu", home, &home.join("marshell"));
        for (name, plan) in plans {
            assert_eq!(plan.is_err(), name == "codex", "{name}: {plan:?}");
        }
    }
}
