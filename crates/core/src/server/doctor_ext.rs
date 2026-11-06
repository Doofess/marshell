//! The Doctor's MCP servers and skills, on the server side: where each CLI's files are, the inventories read from them
//! and the edits that turn off the items the person picked. The items themselves come from `doctor::ext`, and each
//! CLI's file rules live in its `agents/<cli>/extensions.rs`.

use super::AppState;
use crate::config::hash::hash_bytes;
use crate::config::jsonc::read_value;
use crate::doctor::AgentFacts;
use crate::doctor::ext::{CommandState, ExtRoots, Inventory, Switch};
use axum::http::StatusCode;
use marshell_protocol::config::{Applies, ConfigScope, EditOp, FileEdit};
use serde_json::Value;
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, InlineTable, Item, Table, value};

type Fail = (StatusCode, String);

/// Why a switch can't be turned into an edit: the file isn't what the reader saw. The same words as the Doctor's other
/// refusals, with the 409 of a file that changed under the fix.
const CANT_FIX: &str = "This item can't be fixed any more. Run the check again.";

fn refuse() -> Fail {
    (StatusCode::CONFLICT, CANT_FIX.to_string())
}

/// Every CLI folder at its default under `home`. `claude_settings` is the pinned file, else `<home>/.claude/`.
fn roots_under(home: &Path, claude_settings: Option<PathBuf>) -> ExtRoots {
    ExtRoots {
        home: home.to_path_buf(),
        claude_dir: home.join(".claude"),
        claude_json: home.join(".claude.json"),
        claude_settings: claude_settings.unwrap_or_else(|| home.join(".claude").join("settings.json")),
        codex_home: home.join(".codex"),
        qwen_dir: home.join(".qwen"),
    }
}

/// Windows roots. With `ext_home` set by a test, every CLI folder is at its default under it and the environment isn't
/// read; `claude_settings` is the pinned `claude_settings` when set, else `<ext_home>/.claude/settings.json`.
/// Otherwise the user home with CLAUDE_CONFIG_DIR, CODEX_HOME and QWEN_HOME, and `claude_settings_path`. A unit test
/// that names no home gets none, so no test reads the developer's.
pub(crate) fn windows_roots(st: &AppState) -> Option<ExtRoots> {
    if let Some(home) = &st.0.ext_home {
        return Some(roots_under(home, st.0.claude_settings.clone()));
    }
    if cfg!(test) {
        return None;
    }
    let home = super::files::home_dir()?;
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let (claude_dir, claude_json) = match env("CLAUDE_CONFIG_DIR").map(PathBuf::from) {
        Some(dir) => {
            let json = dir.join(".claude.json");
            (dir, json)
        }
        None => (home.join(".claude"), home.join(".claude.json")),
    };
    Some(ExtRoots {
        claude_settings: st.claude_settings_path(&home),
        codex_home: env("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from),
        qwen_dir: crate::agents::qwen::home(&|k| std::env::var(k).ok(), &home),
        claude_dir,
        claude_json,
        home,
    })
}

/// Whether a stdio server's command is there: an absolute path by the file (with the extensions Windows runs when the
/// path has none), a relative path not at all, and a bare name by the PATH.
pub(crate) fn judge(command: &str, on_path: &dyn Fn(&str) -> bool) -> CommandState {
    let command = command.trim();
    if command.is_empty() {
        return CommandState::NotChecked;
    }
    if command.contains(['\\', '/']) {
        let path = Path::new(command);
        if !path.is_absolute() {
            return CommandState::NotChecked;
        }
        let there = path.exists()
            || (cfg!(windows)
                && ["exe", "cmd", "bat", "com"]
                    .iter()
                    .any(|ext| PathBuf::from(format!("{command}.{ext}")).exists()));
        return if there {
            CommandState::Found
        } else {
            CommandState::Missing
        };
    }
    if on_path(command) {
        CommandState::Found
    } else {
        CommandState::Missing
    }
}

fn read_inventory(
    agent: &str,
    roots: &ExtRoots,
    projects: &[PathBuf],
    command: &dyn Fn(&str) -> CommandState,
) -> Option<Inventory> {
    use crate::agents::{antigravity, claude, codex, qwen};
    Some(match agent {
        "claude" => claude::extensions::inventory(roots, projects, command),
        "codex" => codex::extensions::inventory(roots, projects, command),
        "antigravity" => antigravity::extensions::inventory(roots, projects, command),
        "qwen" => qwen::extensions::inventory(roots, projects, command),
        _ => return None,
    })
}

/// Every CLI's inventory on Windows (with Marshell's recent projects), then in each running managed distro (user level
/// only, commands not checked). `distro_homes` holds each running distro's user home. A CLI whose facts have an error
/// gets none. Reads files: callers are already off the async threads.
pub(crate) fn inventories(st: &AppState, facts: &[AgentFacts], distro_homes: &[(String, PathBuf)]) -> Vec<Inventory> {
    let finder = st.agent_finder(&[]);
    let windows = windows_roots(st);
    let projects: Vec<PathBuf> = if windows.is_some() {
        crate::projects::list(&st.0.home)
            .into_iter()
            .map(|p| PathBuf::from(p.path))
            .collect()
    } else {
        Vec::new()
    };
    let on_path = |command: &str| finder.on_path(command);
    let windows_judge = |command: &str| judge(command, &on_path);
    let not_checked = |_: &str| CommandState::NotChecked;

    let mut out = Vec::new();
    for f in facts.iter().filter(|f| f.error.is_none()) {
        let inv = match &f.distro {
            None => windows
                .as_ref()
                .and_then(|roots| read_inventory(f.agent, roots, &projects, &windows_judge)),
            Some(distro) => distro_homes
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(distro))
                .and_then(|(_, home)| read_inventory(f.agent, &roots_under(home, None), &[], &not_checked)),
        };
        if let Some(mut inv) = inv {
            inv.found = f.found;
            inv.distro = f.distro.clone();
            out.push(inv);
        }
    }
    out
}

/// The text of `file`, or None when it isn't there. A file that can't be read refuses.
fn read_text(file: &Path) -> Result<Option<String>, Fail> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(refuse()),
    }
}

fn hash_of(text: &Option<String>) -> Option<String> {
    text.as_ref().map(|t| hash_bytes(t.as_bytes()))
}

/// `User` for a file in a CLI's own folder under a user home (`~/.claude.json`, `~/.qwen/settings.json`), else `Local`
/// (a project's `.claude/settings.local.json`, a folder named by CLAUDE_CONFIG_DIR).
fn scope_of(file: &Path, homes: &[PathBuf]) -> ConfigScope {
    let in_cli_folder = homes.iter().any(|home| {
        file.strip_prefix(home)
            .ok()
            .and_then(|rest| rest.components().next())
            .is_some_and(|first| first.as_os_str().to_string_lossy().starts_with('.'))
    });
    if in_cli_folder {
        ConfigScope::User
    } else {
        ConfigScope::Local
    }
}

fn edit(file: &Path, homes: &[PathBuf], op: EditOp, expected_hash: Option<String>) -> FileEdit {
    FileEdit {
        path: file.to_string_lossy().into_owned(),
        scope: scope_of(file, homes),
        op,
        expected_hash,
        applies: Applies::NewSessions,
    }
}

/// The edits that turn off the picked items. `homes` are the user homes the inventories were read under.
/// - A `JsonAdd` list is read from the file now, the names not in it are appended, and the list is set once: one
///   `JsonSet` per (file, pointer). A value that isn't an array refuses.
/// - A `JsonSet` is one edit each.
/// - Codex's servers and skills in one `config.toml` are one `ReplaceText` (a `CreateFile` when the file isn't there),
///   built with `toml_edit` so the comments stay.
pub(crate) fn edits(switches: &[Switch], homes: &[PathBuf]) -> Result<Vec<FileEdit>, Fail> {
    let mut lists: Vec<(PathBuf, String, Vec<String>)> = Vec::new();
    let mut sets: Vec<(PathBuf, String, String)> = Vec::new();
    let mut codex: Vec<(PathBuf, Vec<&Switch>)> = Vec::new();
    for sw in switches {
        match sw {
            Switch::JsonAdd { file, pointer, name } => {
                match lists.iter_mut().find(|(f, p, _)| f == file && p == pointer) {
                    Some((_, _, names)) => {
                        if !names.contains(name) {
                            names.push(name.clone());
                        }
                    }
                    None => lists.push((file.clone(), pointer.clone(), vec![name.clone()])),
                }
            }
            Switch::JsonSet { file, pointer, value } => {
                if !sets.iter().any(|(f, p, _)| f == file && p == pointer) {
                    sets.push((file.clone(), pointer.clone(), value.clone()));
                }
            }
            Switch::CodexServer { file, .. } | Switch::CodexSkill { file, .. } => {
                match codex.iter_mut().find(|(f, _)| f == file) {
                    Some((_, group)) => group.push(sw),
                    None => codex.push((file.clone(), vec![sw])),
                }
            }
        }
    }

    let mut out = Vec::new();
    for (file, pointer, names) in &lists {
        let text = read_text(file)?;
        let root = match &text {
            Some(text) => read_value(text).map_err(|_| refuse())?,
            None => Value::Object(Default::default()),
        };
        let mut list = match root.pointer(pointer) {
            None => Vec::new(),
            Some(Value::Array(items)) => items.clone(),
            Some(_) => return Err(refuse()),
        };
        for name in names {
            if !list.iter().any(|v| v.as_str() == Some(name)) {
                list.push(Value::String(name.clone()));
            }
        }
        out.push(edit(
            file,
            homes,
            EditOp::JsonSet {
                pointer: pointer.clone(),
                value: Value::Array(list).to_string(),
            },
            hash_of(&text),
        ));
    }
    for (file, pointer, value) in &sets {
        let text = read_text(file)?;
        out.push(edit(
            file,
            homes,
            EditOp::JsonSet {
                pointer: pointer.clone(),
                value: value.clone(),
            },
            hash_of(&text),
        ));
    }
    for (file, group) in &codex {
        out.push(codex_edit(file, group, homes)?);
    }
    Ok(out)
}

/// The table `parent.key` of `doc`, made when it is missing. Something there that is not a table refuses.
fn table_in<'a>(parent: &'a mut Table, key: &str) -> Result<&'a mut Table, Fail> {
    if !parent.contains_key(key) {
        let mut table = Table::new();
        // Only the leaf tables print a header: `[mcp_servers."my.server"]`, not an empty `[mcp_servers]` above it.
        table.set_implicit(true);
        parent.insert(key, Item::Table(table));
    }
    parent.get_mut(key).and_then(Item::as_table_mut).ok_or_else(refuse)
}

fn codex_edit(file: &Path, group: &[&Switch], homes: &[PathBuf]) -> Result<FileEdit, Fail> {
    let text = read_text(file)?;
    let mut doc: DocumentMut = match &text {
        Some(text) => text.parse().map_err(|_| refuse())?,
        None => DocumentMut::new(),
    };
    for sw in group {
        match sw {
            Switch::CodexServer { name, .. } => {
                let servers = table_in(doc.as_table_mut(), "mcp_servers")?;
                let server = table_in(servers, name)?;
                server.set_implicit(false);
                server.insert("enabled", value(false));
            }
            Switch::CodexSkill { skill, .. } => {
                let path = skill.to_string_lossy().into_owned();
                let skills = table_in(doc.as_table_mut(), "skills")?;
                if !skills.contains_key("config") {
                    skills.insert("config", Item::ArrayOfTables(ArrayOfTables::new()));
                }
                match skills.get_mut("config") {
                    Some(Item::ArrayOfTables(rules)) => {
                        // A rule for this skill that is there already is switched off, not repeated.
                        if let Some(rule) = rules
                            .iter_mut()
                            .find(|r| r.get("path").and_then(Item::as_str) == Some(path.as_str()))
                        {
                            rule.insert("enabled", value(false));
                            continue;
                        }
                        let mut rule = Table::new();
                        rule.insert("path", value(path));
                        rule.insert("enabled", value(false));
                        rules.push(rule);
                    }
                    // `config = [{ path = ".." }]` written inline.
                    Some(Item::Value(toml_edit::Value::Array(rules))) => {
                        let mut rule = InlineTable::new();
                        rule.insert("path", path.as_str().into());
                        rule.insert("enabled", false.into());
                        rules.push(rule);
                    }
                    _ => return Err(refuse()),
                }
            }
            _ => {}
        }
    }
    let content = doc.to_string();
    let (op, expected_hash) = match &text {
        Some(_) => (EditOp::ReplaceText { content }, hash_of(&text)),
        None => (EditOp::CreateFile { content }, None),
    };
    Ok(edit(file, homes, op, expected_hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hash::hash_bytes;
    use crate::doctor::ext::{CommandState, Switch};
    use axum::http::StatusCode;
    use marshell_protocol::config::{ConfigScope, EditOp};
    use serde_json::{Value, json};
    use std::path::Path;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn add(file: &Path, pointer: &str, name: &str) -> Switch {
        Switch::JsonAdd {
            file: file.to_path_buf(),
            pointer: pointer.into(),
            name: name.into(),
        }
    }

    fn json_value(edit: &FileEdit) -> (String, Value) {
        match &edit.op {
            EditOp::JsonSet { pointer, value } => (pointer.clone(), serde_json::from_str(value).unwrap()),
            other => panic!("not a JsonSet: {other:?}"),
        }
    }

    #[test]
    fn json_adds_merge_per_list() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".qwen").join("settings.json");
        let text = "{\n  \"mcp\": { \"excluded\": [\"old\"] }\n}\n";
        write(&file, text);
        let switches = [
            add(&file, "/mcp/excluded", "a"),
            add(&file, "/skills/disabled", "s"),
            add(&file, "/mcp/excluded", "b"),
            // Already in the list: kept once.
            add(&file, "/mcp/excluded", "old"),
        ];
        let out = edits(&switches, &[dir.path().to_path_buf()]).unwrap();
        assert_eq!(out.len(), 2, "{out:?}");
        let hash = Some(hash_bytes(text.as_bytes()));
        for e in &out {
            assert_eq!(e.path, file.to_string_lossy());
            assert_eq!(e.expected_hash, hash);
            assert_eq!(e.scope, ConfigScope::User);
        }
        let by_pointer: std::collections::BTreeMap<String, Value> = out.iter().map(json_value).collect();
        assert_eq!(by_pointer["/mcp/excluded"], json!(["old", "a", "b"]));
        assert_eq!(by_pointer["/skills/disabled"], json!(["s"]));
    }

    #[test]
    fn a_missing_file_starts_an_empty_list_with_no_hash_and_a_project_file_is_local() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let file = dir
            .path()
            .join("code")
            .join("x")
            .join(".claude")
            .join("settings.local.json");
        let out = edits(&[add(&file, "/disabledMcpjsonServers", "srv")], &[home]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].expected_hash, None);
        assert_eq!(out[0].scope, ConfigScope::Local);
        assert_eq!(
            json_value(&out[0]),
            ("/disabledMcpjsonServers".to_string(), json!(["srv"]))
        );
    }

    #[test]
    fn codex_server_and_skills_are_one_replace() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".codex").join("config.toml");
        let text = "# my settings\nmodel = \"x\"\n\n[mcp_servers.other]\ncommand = \"o\" # keep this\n";
        write(&file, text);
        let skill = |n: &str| dir.path().join(".codex").join("skills").join(n).join("SKILL.md");
        let switches = [
            Switch::CodexSkill {
                file: file.clone(),
                skill: skill("one"),
            },
            Switch::CodexServer {
                file: file.clone(),
                name: "my.server".into(),
            },
            Switch::CodexSkill {
                file: file.clone(),
                skill: skill("two"),
            },
        ];
        let out = edits(&switches, &[dir.path().to_path_buf()]).unwrap();
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].expected_hash, Some(hash_bytes(text.as_bytes())));
        let EditOp::ReplaceText { content } = &out[0].op else {
            panic!("not a ReplaceText: {:?}", out[0].op);
        };
        assert!(content.contains("# my settings"), "{content}");
        assert!(content.contains("# keep this"), "{content}");
        let doc: toml::Table = content.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["my.server"]["enabled"].as_bool(), Some(false));
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("o"));
        let rules = doc["skills"]["config"].as_array().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["path"].as_str(), Some(skill("one").to_string_lossy().as_ref()));
        assert_eq!(rules[1]["enabled"].as_bool(), Some(false));
        // One key, quoted: not `my` holding `server`.
        assert!(doc["mcp_servers"].get("my").is_none(), "{content}");
    }

    #[test]
    fn codex_skill_with_no_config_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".codex").join("config.toml");
        let skill = dir.path().join(".codex").join("skills").join("s").join("SKILL.md");
        let out = edits(
            &[Switch::CodexSkill {
                file: file.clone(),
                skill,
            }],
            &[dir.path().to_path_buf()],
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].expected_hash, None);
        let EditOp::CreateFile { content } = &out[0].op else {
            panic!("not a CreateFile: {:?}", out[0].op);
        };
        let doc: toml::Table = content.parse().unwrap();
        assert_eq!(doc["skills"]["config"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn non_array_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".qwen").join("settings.json");
        write(&file, r#"{"mcp": {"excluded": "x"}}"#);
        let (status, _) = edits(&[add(&file, "/mcp/excluded", "a")], &[]).unwrap_err();
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[test]
    fn a_codex_server_that_is_not_a_table_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        write(&file, "mcp_servers = 3\n");
        let (status, _) = edits(&[Switch::CodexServer { file, name: "a".into() }], &[]).unwrap_err();
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[test]
    fn json_set_switches_carry_the_hash_once_each() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude").join("settings.json");
        let text = "{}\n";
        write(&file, text);
        let set = |n: &str| Switch::JsonSet {
            file: file.clone(),
            pointer: format!("/skillOverrides/{n}"),
            value: "\"off\"".into(),
        };
        let out = edits(&[set("a~1"), set("b"), set("b")], &[]).unwrap();
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out.iter().all(|e| e.expected_hash == Some(hash_bytes(text.as_bytes()))));
    }

    // Inventories.

    #[test]
    fn the_command_judge_reads_paths_and_names() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("tool.exe");
        write(&real, "");
        let on_path = |c: &str| c == "node";
        assert_eq!(judge("node", &on_path), CommandState::Found);
        assert_eq!(judge("ghost", &on_path), CommandState::Missing);
        assert_eq!(judge(&real.to_string_lossy(), &on_path), CommandState::Found);
        assert_eq!(
            judge(&dir.path().join("gone.exe").to_string_lossy(), &on_path),
            CommandState::Missing
        );
        assert_eq!(judge("./local/server", &on_path), CommandState::NotChecked);
    }
}
