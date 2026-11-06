//! Antigravity CLI's MCP servers and skills for the Doctor: where `agy` keeps them, how a server is turned off (a
//! skill can't be), and what makes a skill not load. Facts and sources: `docs/research/2026-11-06-mcp-and-skills.md`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::jsonc;
use crate::doctor::ext::{
    Caps, CommandState, CostRule, ExtRoots, Hints, Inventory, NoSwitch, NotMeasured, Origin, Server, Skill, SkillState,
    Switch, Transport, UsedAs, skill_files, unreadable as ext_unreadable,
};
use crate::doctor::frontmatter::{self, Front};

/// Antigravity's servers and skills at user level, in `projects`, and in its enabled plugins.
/// `command` judges a stdio server's command. Returns `distro: None` and `found: true`; the caller sets both.
pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory {
    let mut inv = Inventory {
        agent: "antigravity",
        distro: None,
        found: true,
        caps: Caps::default(),
        servers: Vec::new(),
        skills: Vec::new(),
        unreadable: Vec::new(),
        hints: Hints::default(),
        cost_note: None,
    };
    let gemini = roots.home.join(".gemini");
    let config = gemini.join("config");

    let user = config.join("mcp_config.json");
    if let Some(value) = read_json(&user, &mut inv.unreadable) {
        servers_of(&value, &user, &Origin::User, true, command, &mut inv.servers);
    }
    for project in projects {
        let file = project.join(".agents").join("mcp_config.json");
        if let Some(value) = read_json(&file, &mut inv.unreadable) {
            servers_of(
                &value,
                &file,
                &Origin::Project(project.clone()),
                false,
                command,
                &mut inv.servers,
            );
        }
    }
    let plugins = enabled_plugins(&config, &mut inv.unreadable);
    for (dir, folder) in &plugins {
        let file = folder.join("mcp_config.json");
        if let Some(value) = read_json(&file, &mut inv.unreadable) {
            servers_of(
                &value,
                &file,
                &Origin::Plugin(dir.clone()),
                false,
                command,
                &mut inv.servers,
            );
        }
    }

    let mut add = |dir: &Path, origin: Origin| {
        for (folder, file) in skill_files(dir) {
            inv.skills.push(skill_of(&folder, file, origin.clone()));
        }
    };
    add(&config.join("skills"), Origin::User);
    add(&gemini.join("antigravity-cli").join("skills"), Origin::User);
    add(&gemini.join("skills"), Origin::User);
    add(
        &gemini.join("antigravity-cli").join("builtin").join("skills"),
        Origin::Bundled,
    );
    for project in projects {
        add(
            &project.join(".agents").join("skills"),
            Origin::Project(project.clone()),
        );
        add(&project.join(".agent").join("skills"), Origin::Project(project.clone()));
    }
    for (dir, folder) in &plugins {
        add(&folder.join("skills"), Origin::Plugin(dir.clone()));
    }
    inv
}

/// A JSONC file as a value. A missing file says nothing, and an empty or all-whitespace one is `{}`; one that doesn't
/// parse goes in `unreadable`.
fn read_json(file: &Path, unreadable: &mut Vec<(PathBuf, String)>) -> Option<Value> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            unreadable.push((file.to_path_buf(), ext_unreadable::io(&e)));
            return None;
        }
    };
    match jsonc::read_value(&text) {
        Ok(value) => Some(value),
        Err(_) => {
            unreadable.push((file.to_path_buf(), ext_unreadable::json(&text)));
            None
        }
    }
}

/// The plugin folders under `<config>/plugins`, without those `<config>/config.json` marks
/// `plugins.<dir>.enabled == false`. Each is (folder name, folder).
fn enabled_plugins(config: &Path, unreadable: &mut Vec<(PathBuf, String)>) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(config.join("plugins")) else {
        return Vec::new();
    };
    // A `config.json` that can't be read doesn't hide a plugin, and isn't a file Marshell reports on.
    let marks = read_json(&config.join("config.json"), &mut Vec::new());
    let _ = unreadable;
    let mut out: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(dir, _)| {
            let enabled = marks
                .as_ref()
                .and_then(|m| m.pointer(&format!("/plugins/{}/enabled", escape(dir))));
            enabled.and_then(Value::as_bool) != Some(false)
        })
        .collect();
    out.sort();
    out
}

/// The `mcpServers` of `value`.
fn servers_of(
    value: &Value,
    file: &Path,
    origin: &Origin,
    switchable: bool,
    command: &dyn Fn(&str) -> CommandState,
    out: &mut Vec<Server>,
) {
    let Some(servers) = value.get("mcpServers").and_then(Value::as_object) else {
        return;
    };
    for (name, entry) in servers {
        let transport = if entry.get("serverUrl").is_some() || entry.get("url").is_some() {
            Transport::Remote
        } else if let Some(cmd) = entry.get("command").and_then(Value::as_str) {
            Transport::Stdio {
                command: cmd.to_string(),
                state: command(cmd),
            }
        } else {
            Transport::Nothing
        };
        let switch = match origin {
            Origin::User if switchable => Ok(Switch::JsonSet {
                file: file.to_path_buf(),
                pointer: format!("/mcpServers/{}/disabled", escape(name)),
                value: "true".to_string(),
            }),
            Origin::Plugin(dir) => Err(NoSwitch::Plugin(dir.clone())),
            _ => Err(NoSwitch::Shared(file.to_path_buf())),
        };
        out.push(Server {
            name: name.clone(),
            file: file.to_path_buf(),
            origin: origin.clone(),
            transport,
            off: entry.get("disabled").and_then(Value::as_bool) == Some(true),
            switch,
            launch: Err(NotMeasured::NotApproved),
            cost: CostRule::names_only(),
            used_as: UsedAs::Unknown,
        });
    }
}

/// A JSON pointer segment: `~` is `~0` and `/` is `~1`.
fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn skill_of(folder: &str, file: PathBuf, origin: Origin) -> Skill {
    let front = std::fs::read_to_string(&file).ok().map(|t| frontmatter::read(&t));
    let filled = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.trim().is_empty());
    let (meta, state) = match front {
        None => (Default::default(), SkillState::Ok),
        Some(Front::NoBlock) => (Default::default(), SkillState::NoBlock),
        Some(Front::Unclear(meta)) => (meta, SkillState::Ok),
        Some(Front::Read(meta)) => {
            let state = if !filled(&meta.name) {
                SkillState::Missing("name")
            } else if !filled(&meta.description) {
                SkillState::Missing("description")
            } else {
                SkillState::Ok
            };
            (meta, state)
        }
    };
    let name = meta
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(folder)
        .to_string();
    let description = meta.description.as_deref().unwrap_or("");
    let switch = match &origin {
        Origin::Bundled => Err(NoSwitch::Bundled),
        Origin::Plugin(dir) => Err(NoSwitch::Plugin(dir.clone())),
        _ => Err(NoSwitch::NoSkillSwitch),
    };
    Skill {
        created_ms: crate::doctor::ext::created_ms(&file),
        used_as: Vec::new(),
        name,
        file,
        origin,
        state,
        listed_chars: description.chars().count(),
        first_line_chars: description.lines().next().unwrap_or("").chars().count(),
        name_only: false,
        off: false,
        hidden: false,
        switch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh fake home under the repo's target folder.
    fn fake(name: &str) -> (PathBuf, ExtRoots) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("test-ext-agy-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".gemini").join("config")).unwrap();
        let roots = ExtRoots {
            home: root.clone(),
            claude_dir: root.join(".claude"),
            claude_json: root.join(".claude.json"),
            claude_settings: root.join(".claude").join("settings.json"),
            codex_home: root.join(".codex"),
            qwen_dir: root.join(".qwen"),
            claude_tool_search: None,
            qwen_runtime: root.join(".qwen"),
            gemini_dir: root.join(".gemini"),
        };
        (root, roots)
    }

    fn write(file: &Path, text: &str) {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }

    fn judge(cmd: &str) -> CommandState {
        if cmd == "node" {
            CommandState::Found
        } else {
            CommandState::Missing
        }
    }

    #[test]
    fn agy_zero_byte_config() {
        let (root, roots) = fake("zero");
        write(&root.join(".gemini").join("config").join("mcp_config.json"), "");
        let inv = inventory(&roots, &[], &judge);
        assert_eq!((inv.agent, inv.distro.clone(), inv.found), ("antigravity", None, true));
        assert!(inv.servers.is_empty());
        assert!(inv.unreadable.is_empty());

        write(&root.join(".gemini").join("config").join("mcp_config.json"), " \r\n\t");
        let inv = inventory(&roots, &[], &judge);
        assert!(inv.servers.is_empty() && inv.unreadable.is_empty());
    }

    #[test]
    fn agy_jsonc_and_disabled() {
        let (root, roots) = fake("jsonc");
        let file = root.join(".gemini").join("config").join("mcp_config.json");
        write(
            &file,
            "{\n  // my servers\n  \"mcpServers\": {\n    \"a/b\": {\"command\": \"node\", \"args\": [],},\n    \"off\": {\"command\": \"gone\", \"disabled\": true},\n    \"rem\": {\"serverUrl\": \"https://x\"},\n    \"rem2\": {\"url\": \"https://y\"},\n  },\n}\n",
        );
        let project = root.join("proj");
        let shared = project.join(".agents").join("mcp_config.json");
        write(&shared, "{\"mcpServers\": {\"pj\": {\"command\": \"node\"}}}");

        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert!(inv.unreadable.is_empty());
        let find = |n: &str| inv.servers.iter().find(|s| s.name == n).unwrap();
        let ab = find("a/b");
        assert!(!ab.off);
        assert_eq!(ab.origin, Origin::User);
        assert_eq!(
            ab.transport,
            Transport::Stdio {
                command: "node".into(),
                state: CommandState::Found
            }
        );
        assert_eq!(
            ab.switch,
            Ok(Switch::JsonSet {
                file: file.clone(),
                pointer: "/mcpServers/a~1b/disabled".into(),
                value: "true".into()
            })
        );
        assert!(find("off").off);
        assert_eq!(find("rem").transport, Transport::Remote);
        assert_eq!(find("rem2").transport, Transport::Remote);
        let pj = find("pj");
        assert_eq!(pj.origin, Origin::Project(project));
        assert_eq!(pj.switch, Err(NoSwitch::Shared(shared)));
    }

    #[test]
    fn agy_unparseable_config_is_unreadable() {
        let (root, roots) = fake("broken");
        let file = root.join(".gemini").join("config").join("mcp_config.json");
        write(&file, "{\"mcpServers\": ");
        let inv = inventory(&roots, &[], &judge);
        assert!(inv.servers.is_empty());
        assert_eq!(inv.unreadable.len(), 1);
        assert_eq!(inv.unreadable[0].0, file);
    }

    #[test]
    fn agy_unreadable_files_are_described_in_one_sentence() {
        let (root, roots) = fake("unreadable-message");
        let config = root.join(".gemini").join("config");
        let user = config.join("mcp_config.json");
        write(&user, "{\n  \"mcpServers\": {\n    \"x\": \"sk-SECRET\" oops\n  }\n}\n");
        let plugin = config.join("plugins").join("p").join("mcp_config.json");
        std::fs::create_dir_all(plugin.parent().unwrap()).unwrap();
        std::fs::write(&plugin, [0xff, 0xfe, 0x41]).unwrap();
        let inv = inventory(&roots, &[], &judge);
        let message = |file: &Path| inv.unreadable.iter().find(|(f, _)| f == file).unwrap().1.clone();
        assert_eq!(message(&user), "It doesn't parse (line 4).");
        assert_eq!(message(&plugin), "It isn't UTF-8 text.");
    }

    #[test]
    fn agy_plugins() {
        let (root, roots) = fake("plugins");
        let config = root.join(".gemini").join("config");
        write(
            &config.join("config.json"),
            "{\"plugins\": {\"offp\": {\"enabled\": false}, \"onp\": {\"enabled\": true}}}",
        );
        for dir in ["onp", "offp", "plain"] {
            write(
                &config.join("plugins").join(dir).join("mcp_config.json"),
                &format!("{{\"mcpServers\": {{\"{dir}_srv\": {{\"url\": \"https://x\"}}}}}}"),
            );
            write(
                &config
                    .join("plugins")
                    .join(dir)
                    .join("skills")
                    .join("sk")
                    .join("SKILL.md"),
                &format!("---\nname: {dir}-sk\ndescription: d\n---\n"),
            );
        }
        let inv = inventory(&roots, &[], &judge);
        let mut names: Vec<&str> = inv.servers.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["onp_srv", "plain_srv"]);
        assert_eq!(inv.servers[0].switch, Err(NoSwitch::Plugin("onp".into())));
        let mut skills: Vec<&str> = inv.skills.iter().map(|s| s.name.as_str()).collect();
        skills.sort();
        assert_eq!(skills, ["onp-sk", "plain-sk"]);
        assert_eq!(inv.skills[0].origin, Origin::Plugin("onp".into()));
        assert_eq!(inv.skills[0].switch, Err(NoSwitch::Plugin("onp".into())));
    }

    #[test]
    fn agy_skills_have_no_switch() {
        let (root, roots) = fake("skills");
        let gemini = root.join(".gemini");
        write(
            &gemini.join("config").join("skills").join("mine").join("SKILL.md"),
            "---\nname: mine\ndescription: Mine\n---\n",
        );
        write(
            &gemini
                .join("antigravity-cli")
                .join("builtin")
                .join("skills")
                .join("core")
                .join("SKILL.md"),
            "---\nname: core\ndescription: Core\n---\n",
        );
        write(
            &gemini.join("skills").join("noname").join("SKILL.md"),
            "---\ndescription: No name\n---\n",
        );
        write(
            &gemini.join("skills").join("nodesc").join("SKILL.md"),
            "---\nname: nodesc\n---\n",
        );
        write(&gemini.join("skills").join("plain").join("SKILL.md"), "just text\n");
        write(
            &gemini.join("skills").join("quoted").join("SKILL.md"),
            "---\nname: quoted\ndescription: >\n  folded\n  text\n---\n",
        );
        let project = root.join("proj");
        write(
            &project.join(".agent").join("skills").join("pj").join("SKILL.md"),
            "---\nname: pj\ndescription: P\n---\n",
        );

        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        let find = |n: &str| inv.skills.iter().find(|s| s.name == n).unwrap();
        let mine = find("mine");
        assert_eq!(mine.origin, Origin::User);
        assert_eq!(mine.switch, Err(NoSwitch::NoSkillSwitch));
        assert_eq!(mine.state, SkillState::Ok);
        assert_eq!(mine.listed_chars, 4);
        assert_eq!(find("core").origin, Origin::Bundled);
        assert_eq!(find("core").switch, Err(NoSwitch::Bundled));
        assert_eq!(find("noname").state, SkillState::Missing("name"));
        assert_eq!(find("nodesc").state, SkillState::Missing("description"));
        assert_eq!(find("plain").state, SkillState::NoBlock);
        assert_eq!(find("quoted").state, SkillState::Ok);
        let pj = find("pj");
        assert_eq!(pj.origin, Origin::Project(project));
        assert_eq!(pj.switch, Err(NoSwitch::NoSkillSwitch));
        assert!(!pj.off && !pj.hidden);
    }
}
