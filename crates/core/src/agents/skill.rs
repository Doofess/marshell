//! The built-in `marshell` skill: its text, where each CLI keeps it, whether the file on disk is ours, and the
//! plans that install, update and remove it. The plans go through `ConfigService` like every other write.

use crate::config::hash::hash_bytes;
use marshell_protocol::agents::SkillInstall;
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use std::path::{Path, PathBuf};

/// The one `SKILL.md` all three CLIs read.
pub const SKILL_TEXT: &str = include_str!("skill/SKILL.md");

/// The `hash_bytes` hex of every earlier shipped `SKILL_TEXT`. A release that changes the text adds the old hash.
pub const PREVIOUS: &[&str] = &[];

/// Where `agent`'s user-level skill file goes, or None for an agent without skills. `CLAUDE_CONFIG_DIR` moves
/// Claude Code's only when it is set and not blank.
pub fn skill_path(agent: &str, env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> Option<PathBuf> {
    let root = match agent {
        "claude" => match env("CLAUDE_CONFIG_DIR").filter(|v| !v.trim().is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => user_home.join(".claude"),
        },
        "codex" => user_home.join(".agents"),
        "antigravity" => user_home.join(".gemini").join("config"),
        _ => return None,
    };
    Some(root.join("skills").join("marshell").join("SKILL.md"))
}

/// What the file at the skill path holds, judged by its bytes. None for a missing file.
pub fn state_of(bytes: Option<&[u8]>) -> SkillInstall {
    state_with(bytes, PREVIOUS)
}

fn state_with(bytes: Option<&[u8]>, previous: &[&str]) -> SkillInstall {
    match bytes {
        None => SkillInstall::None,
        Some(b) if b == SKILL_TEXT.as_bytes() => SkillInstall::Installed,
        Some(b) if previous.contains(&hash_bytes(b).as_str()) => SkillInstall::Outdated,
        Some(_) => SkillInstall::Edited,
    }
}

fn edit(path: &Path, op: EditOp, bytes: Option<&[u8]>) -> FileEdit {
    FileEdit {
        path: path.to_string_lossy().into_owned(),
        scope: ConfigScope::User,
        op,
        expected_hash: bytes.map(hash_bytes),
        applies: Applies::NewSessions,
    }
}

/// Creates the file, or replaces what is there (checked against its hash). None when it is already current.
pub fn install_edit(path: &Path, bytes: Option<&[u8]>) -> Option<FileEdit> {
    let content = SKILL_TEXT.to_string();
    match state_of(bytes) {
        SkillInstall::None => Some(edit(path, EditOp::CreateFile { content }, None)),
        SkillInstall::Outdated | SkillInstall::Edited => Some(edit(path, EditOp::ReplaceText { content }, bytes)),
        SkillInstall::Installed => None,
    }
}

pub fn install_plan(agent_name: &str, path: &Path, bytes: Option<&[u8]>) -> ChangeSet {
    let verb = match state_of(bytes) {
        SkillInstall::None | SkillInstall::Installed => "Install",
        SkillInstall::Outdated | SkillInstall::Edited => "Update",
    };
    ChangeSet {
        title: format!("{verb} the marshell skill for {agent_name}"),
        reason: format!("So {agent_name} knows how to use the marshell command."),
        edits: install_edit(path, bytes).into_iter().collect(),
    }
}

pub fn remove_plan(agent_name: &str, path: &Path, bytes: Option<&[u8]>) -> ChangeSet {
    ChangeSet {
        title: format!("Remove the marshell skill for {agent_name}"),
        reason: "Marshell takes out the skill it installed.".to_string(),
        edits: bytes
            .map(|_| edit(path, EditOp::DeleteFile, bytes))
            .into_iter()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hash::hash_bytes;
    use marshell_protocol::agents::SkillInstall;
    use marshell_protocol::config::EditOp;
    use std::path::Path;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn skill_text_fits_every_cli() {
        assert!(SKILL_TEXT.starts_with("---\nname: marshell\ndescription: \""));
        let front: Vec<&str> = SKILL_TEXT.lines().skip(1).take_while(|l| *l != "---").collect();
        assert_eq!(front.len(), 2, "frontmatter keys: {front:?}");
        assert!(front[1].len() < 1024);
        assert!(SKILL_TEXT.len() < 24 * 1024);
        assert!(SKILL_TEXT.lines().count() < 500);
        assert!(!SKILL_TEXT.contains('\r'));
    }

    #[test]
    fn skill_paths_per_cli() {
        let home = Path::new("/h");
        let p = |agent: &str, env: &dyn Fn(&str) -> Option<String>| skill_path(agent, env, home);
        let tail = |root: &str| Path::new(root).join("skills/marshell/SKILL.md");
        assert_eq!(p("claude", &no_env), Some(tail("/h/.claude")));
        let set = |v: &'static str| move |k: &str| (k == "CLAUDE_CONFIG_DIR").then(|| v.to_string());
        assert_eq!(p("claude", &set("/c")), Some(tail("/c")));
        assert_eq!(p("claude", &set(" ")), Some(tail("/h/.claude")));
        assert_eq!(p("codex", &no_env), Some(tail("/h/.agents")));
        assert_eq!(p("antigravity", &no_env), Some(tail("/h/.gemini/config")));
        assert_eq!(p("grok", &no_env), None);
    }

    #[test]
    fn state_follows_the_file() {
        assert_eq!(state_of(None), SkillInstall::None);
        assert_eq!(state_of(Some(SKILL_TEXT.as_bytes())), SkillInstall::Installed);
        assert_eq!(state_of(Some(b"mine")), SkillInstall::Edited);
        assert_eq!(state_with(Some(b"old"), &[&hash_bytes(b"old")]), SkillInstall::Outdated);
    }

    #[test]
    fn plans_create_replace_or_nothing() {
        let path = Path::new("/h/.agents/skills/marshell/SKILL.md");

        let plan = install_plan("Codex", path, None);
        assert_eq!(plan.title, "Install the marshell skill for Codex");
        assert_eq!(plan.edits.len(), 1);
        assert!(matches!(plan.edits[0].op, EditOp::CreateFile { .. }));
        assert_eq!(plan.edits[0].expected_hash, None);

        let plan = install_plan("Codex", path, Some(b"mine"));
        assert_eq!(plan.title, "Update the marshell skill for Codex");
        assert_eq!(plan.edits.len(), 1);
        assert!(matches!(plan.edits[0].op, EditOp::ReplaceText { .. }));
        assert_eq!(plan.edits[0].expected_hash, Some(hash_bytes(b"mine")));

        assert!(
            install_plan("Codex", path, Some(SKILL_TEXT.as_bytes()))
                .edits
                .is_empty()
        );

        let plan = remove_plan("Codex", path, Some(SKILL_TEXT.as_bytes()));
        assert_eq!(plan.edits.len(), 1);
        assert!(matches!(plan.edits[0].op, EditOp::DeleteFile));
        assert_eq!(plan.edits[0].expected_hash, Some(hash_bytes(SKILL_TEXT.as_bytes())));

        assert!(remove_plan("Codex", path, None).edits.is_empty());
    }
}
