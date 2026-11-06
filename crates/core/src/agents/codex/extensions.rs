//! Codex CLI's MCP servers and skills for the Doctor: where Codex keeps them, how one is turned off, what makes a skill
//! not load, and what Codex cuts from the skills' listing. Facts and sources: `docs/research/2026-11-06-mcp-and-skills.md`.

use std::path::{Path, PathBuf};

use serde_json::Value;
use toml_edit::{DocumentMut, Item, TableLike};

use crate::config::jsonc;
use crate::doctor::ext::{
    Caps, CommandState, CostRule, Expand, ExtRoots, Hints, Inherit, Inventory, Launch, Listing, NoSwitch, NotMeasured,
    Origin, Server, Skill, SkillState, SkillUse, Switch, Transport, UsedAs, sanitize_tool_part, skill_files,
    unreadable as ext_unreadable,
};
use crate::doctor::frontmatter::{self, Front};

/// Codex cuts a skill's description at this many characters.
const DESCRIPTION_CAP: usize = 1024;

/// One `[[skills.config]]` entry.
struct SkillRule {
    path: Option<String>,
    name: Option<String>,
    enabled: bool,
}

/// Codex's servers and skills at user level, in `projects`, and in its enabled plugins.
/// `command` judges a stdio server's command. Returns `distro: None` and `found: true`; the caller sets both.
pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory {
    let mut inv = Inventory {
        agent: "codex",
        distro: None,
        found: true,
        caps: Caps {
            per_skill: Some(DESCRIPTION_CAP),
            listing_first_line_over: None,
        },
        servers: Vec::new(),
        skills: Vec::new(),
        unreadable: Vec::new(),
        hints: Hints {
            move_server: |name, project| {
                format!(
                    "To load it only there, move its [mcp_servers.{name}] table to {}\\.codex\\config.toml. Codex reads that file only in a trusted project.",
                    project.display()
                )
            },
            move_skill: |_, project| {
                format!(
                    "To load it only there, move its folder to {}\\.agents\\skills.",
                    project.display()
                )
            },
        },
        cost_note: None,
    };
    let config = roots.codex_home.join("config.toml");
    let user = read_doc(&config, &mut inv.unreadable);

    if let Some(doc) = &user {
        servers_of(
            doc,
            &config,
            &Origin::User,
            true,
            Some(&roots.home),
            command,
            &mut inv.servers,
        );
    }
    for project in projects {
        let file = project.join(".codex").join("config.toml");
        if let Some(doc) = read_doc(&file, &mut inv.unreadable) {
            servers_of(
                &doc,
                &file,
                &Origin::Project(project.clone()),
                false,
                trusted(user.as_ref(), project).then_some(project.as_path()),
                command,
                &mut inv.servers,
            );
        }
    }

    let plugins = enabled_plugins(user.as_ref(), &roots.codex_home);
    for (id, folder) in &plugins {
        plugin_servers(id, folder, &roots.home, command, &mut inv.servers, &mut inv.unreadable);
    }

    let rules = user.as_ref().map(skill_rules).unwrap_or_default();
    let skills_dir = roots.codex_home.join("skills");
    let mut add = |dir: &Path, origin: Origin, prefix: Option<&str>| {
        for (folder, file) in skill_files(dir) {
            inv.skills
                .push(skill_of(&folder, file, origin.clone(), prefix, &config, &rules));
        }
    };
    add(&roots.home.join(".agents").join("skills"), Origin::User, None);
    add(&skills_dir, Origin::User, None);
    add(&skills_dir.join(".system"), Origin::Bundled, None);
    for project in projects {
        add(
            &project.join(".agents").join("skills"),
            Origin::Project(project.clone()),
            None,
        );
    }
    for (id, folder) in &plugins {
        let plugin = id.split('@').next().unwrap_or(id);
        add(&folder.join("skills"), Origin::Plugin(id.clone()), Some(plugin));
    }
    inv
}

/// A TOML file as a document. A missing file is `None` and says nothing; one that doesn't parse goes in `unreadable`.
fn read_doc(file: &Path, unreadable: &mut Vec<(PathBuf, String)>) -> Option<DocumentMut> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            unreadable.push((file.to_path_buf(), ext_unreadable::io(&e)));
            return None;
        }
    };
    match text.parse::<DocumentMut>() {
        Ok(doc) => Some(doc),
        Err(e) => {
            unreadable.push((file.to_path_buf(), ext_unreadable::toml(&text, &e)));
            None
        }
    }
}

/// The `[mcp_servers.<name>]` tables of `doc`. Only the user's own file can be switched.
/// `cwd` is the folder a server without its own `cwd` starts in; `None` means Codex doesn't read these servers
/// (an untrusted project).
fn servers_of(
    doc: &DocumentMut,
    file: &Path,
    origin: &Origin,
    switchable: bool,
    cwd: Option<&Path>,
    command: &dyn Fn(&str) -> CommandState,
    out: &mut Vec<Server>,
) {
    let Some(table) = doc.get("mcp_servers").and_then(Item::as_table_like) else {
        return;
    };
    for (name, item) in table.iter() {
        let Some(entry) = item.as_table_like() else {
            continue;
        };
        let transport = if entry.get("url").is_some() {
            Transport::Remote
        } else if let Some(cmd) = entry.get("command").and_then(Item::as_str) {
            Transport::Stdio {
                command: cmd.to_string(),
                state: command(cmd),
            }
        } else {
            Transport::Nothing
        };
        let json = item_json(item);
        let remote = transport == Transport::Remote;
        let switch = if switchable {
            Ok(Switch::CodexServer {
                file: file.to_path_buf(),
                name: name.to_string(),
            })
        } else {
            Err(NoSwitch::Shared(file.to_path_buf()))
        };
        out.push(Server {
            name: name.to_string(),
            file: file.to_path_buf(),
            origin: origin.clone(),
            transport,
            off: entry.get("enabled").and_then(Item::as_bool) == Some(false),
            switch,
            launch: launch_of(&json, remote, cwd),
            cost: cost_of(name, &json, false),
            used_as: UsedAs::Server(name.to_string()),
        });
    }
}

/// Codex starts a stdio server with an empty environment plus these variables from its own (`WINDOWS_CORE_ENV_VARS`).
const CORE_ENV_VARS: [&str; 23] = [
    "PATH",
    "PATHEXT",
    "SHELL",
    "COMSPEC",
    "SYSTEMROOT",
    "WINDIR",
    "SYSTEMDRIVE",
    "USERNAME",
    "USERDOMAIN",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "PROGRAMDATA",
    "LOCALAPPDATA",
    "APPDATA",
    "TEMP",
    "TMP",
    "TMPDIR",
    "POWERSHELL",
    "PWSH",
];

/// A config entry as JSON, so a config server and a plugin's `.mcp.json` entry are read by the same code.
fn item_json(item: &Item) -> Value {
    if let Some(t) = item.as_table_like() {
        Value::Object(t.iter().map(|(k, v)| (k.to_string(), item_json(v))).collect())
    } else if let Some(a) = item.as_array() {
        Value::Array(a.iter().map(|v| item_json(&Item::Value(v.clone()))).collect())
    } else if let Some(s) = item.as_str() {
        Value::String(s.to_string())
    } else if let Some(i) = item.as_integer() {
        Value::from(i)
    } else if let Some(f) = item.as_float() {
        Value::from(f)
    } else if let Some(b) = item.as_bool() {
        Value::Bool(b)
    } else {
        Value::Null
    }
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// How Codex starts a stdio server. `cwd` is where a server with no `cwd` of its own starts; `None` means Codex
/// doesn't read the entry (an untrusted project).
fn launch_of(entry: &Value, remote: bool, cwd: Option<&Path>) -> Result<Launch, NotMeasured> {
    if remote {
        return Err(NotMeasured::Remote);
    }
    let (Some(cwd), Some(command)) = (
        cwd,
        entry
            .get("command")
            .and_then(Value::as_str)
            .filter(|c| !c.trim().is_empty()),
    ) else {
        return Err(NotMeasured::NotApproved);
    };
    let mut inherit: Vec<String> = CORE_ENV_VARS.iter().map(|s| s.to_string()).collect();
    inherit.extend(strings(entry.get("env_vars")));
    let env = entry
        .get("env")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let secs = entry.get("startup_timeout_sec").and_then(Value::as_f64).map(f64::ceil);
    let ms = entry
        .get("startup_timeout_ms")
        .and_then(Value::as_f64)
        .map(|ms| (ms / 1000.0).ceil());
    Ok(Launch {
        command: command.to_string(),
        args: strings(entry.get("args")),
        env,
        inherit: Inherit::Only(inherit),
        set: Vec::new(),
        cwd: Some(
            entry
                .get("cwd")
                .and_then(Value::as_str)
                .map_or_else(|| cwd.to_path_buf(), PathBuf::from),
        ),
        expand: Expand::None,
        timeout_secs: secs.or(ms).unwrap_or(30.0).clamp(0.0, 60.0) as u64,
    })
}

/// What Codex lists up front for a server: one namespace line, or every tool in full when a plugin's `.mcp.json`
/// entry says `omit_tools_from: ["deferred"]`.
fn cost_of(name: &str, entry: &Value, plugin: bool) -> CostRule {
    let name = sanitize_tool_part(name);
    let direct = plugin && strings(entry.get("omit_tools_from")).iter().any(|s| s == "deferred");
    CostRule {
        listing: if direct {
            Listing::Full {
                prefix: format!("mcp__{name}__"),
                desc_cap: None,
            }
        } else {
            Listing::Namespace {
                namespace: format!("mcp__{name}"),
                cap: 250,
            }
        },
        always_load_meta: None,
        only: entry.get("enabled_tools").map(|v| strings(Some(v))),
        except: strings(entry.get("disabled_tools")),
    }
}

/// A path as Codex's trust table compares it: without a `\\?\` prefix, `\` as `/`, without a trailing `/` or case.
fn trust_key(path: &str) -> String {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Whether `[projects.'<key>'] trust_level = "trusted"` names the project folder or its git root, the nearest
/// ancestor holding `.git`.
fn trusted(user: Option<&DocumentMut>, project: &Path) -> bool {
    let Some(table) = user.and_then(|d| d.get("projects")).and_then(Item::as_table_like) else {
        return false;
    };
    let mut wanted = vec![trust_key(&project.display().to_string())];
    if let Some(root) = project.ancestors().find(|a| a.join(".git").exists()) {
        wanted.push(trust_key(&root.display().to_string()));
    }
    table.iter().any(|(key, item)| {
        wanted.contains(&trust_key(key))
            && item
                .as_table_like()
                .and_then(|t| t.get("trust_level"))
                .and_then(Item::as_str)
                == Some("trusted")
    })
}

/// The plugins `[plugins."<plugin>@<market>"]` marks `enabled = true`, each with its folder in Codex's cache:
/// `latest` when it exists, else the version folder that sorts last. A plugin with no folder is left out.
fn enabled_plugins(user: Option<&DocumentMut>, codex_home: &Path) -> Vec<(String, PathBuf)> {
    let Some(table) = user.and_then(|d| d.get("plugins")).and_then(Item::as_table_like) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, item) in table.iter() {
        let on = item
            .as_table_like()
            .and_then(|t| t.get("enabled"))
            .and_then(Item::as_bool)
            == Some(true);
        let Some((plugin, market)) = id.rsplit_once('@') else {
            continue;
        };
        if !on || plugin.is_empty() || market.is_empty() {
            continue;
        }
        let base = codex_home.join("plugins").join("cache").join(market).join(plugin);
        let latest = base.join("latest");
        let folder = if latest.is_dir() {
            Some(latest)
        } else {
            last_subfolder(&base)
        };
        if let Some(folder) = folder {
            out.push((id.to_string(), folder));
        }
    }
    out
}

/// The subfolder of `dir` that sorts last by name.
fn last_subfolder(dir: &Path) -> Option<PathBuf> {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();
    folders.pop()
}

/// A plugin's servers, from `.mcp.json` `mcpServers`. They have no switch of their own.
fn plugin_servers(
    id: &str,
    folder: &Path,
    home: &Path,
    command: &dyn Fn(&str) -> CommandState,
    out: &mut Vec<Server>,
    unreadable: &mut Vec<(PathBuf, String)>,
) {
    let file = folder.join(".mcp.json");
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            unreadable.push((file, ext_unreadable::io(&e)));
            return;
        }
    };
    let value = match jsonc::read_value(&text) {
        Ok(value) => value,
        Err(_) => {
            unreadable.push((file, ext_unreadable::json(&text)));
            return;
        }
    };
    let Some(servers) = value.get("mcpServers").and_then(|v| v.as_object()) else {
        return;
    };
    for (name, entry) in servers {
        let transport = if entry.get("url").is_some() {
            Transport::Remote
        } else if let Some(cmd) = entry.get("command").and_then(|v| v.as_str()) {
            Transport::Stdio {
                command: cmd.to_string(),
                state: command(cmd),
            }
        } else {
            Transport::Nothing
        };
        out.push(Server {
            name: name.clone(),
            file: file.clone(),
            origin: Origin::Plugin(id.to_string()),
            launch: launch_of(entry, transport == Transport::Remote, Some(home)),
            transport,
            off: false,
            switch: Err(NoSwitch::Plugin(id.to_string())),
            cost: cost_of(name, entry, true),
            used_as: UsedAs::Server(name.clone()),
        });
    }
}

/// The `[[skills.config]]` entries, in file order.
fn skill_rules(doc: &DocumentMut) -> Vec<SkillRule> {
    let Some(config) = doc
        .get("skills")
        .and_then(Item::as_table_like)
        .and_then(|t| t.get("config"))
    else {
        return Vec::new();
    };
    let rule = |t: &dyn TableLike| SkillRule {
        path: t.get("path").and_then(Item::as_str).map(str::to_string),
        name: t.get("name").and_then(Item::as_str).map(str::to_string),
        // An entry with no `enabled` doesn't turn anything off.
        enabled: t.get("enabled").and_then(Item::as_bool).unwrap_or(true),
    };
    if let Some(tables) = config.as_array_of_tables() {
        tables.iter().map(|t| rule(t)).collect()
    } else if let Some(array) = config.as_array() {
        array
            .iter()
            .filter_map(|v| v.as_inline_table())
            .map(|t| rule(t))
            .collect()
    } else {
        Vec::new()
    }
}

/// A path compared the way the rules compare it: `\` as `/`, without case.
pub(crate) fn same_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// A skill's path as Codex names it in a rule: in a distro, the distro's own path, not the `\\wsl.localhost` one.
pub(crate) fn rule_path(file: &Path) -> String {
    crate::wsl::share_to_linux(file).unwrap_or_else(|| file.display().to_string())
}

/// Whether the last rule naming the skill, by its path or its name, turns it off.
fn turned_off(rules: &[SkillRule], file: &Path, name: &str) -> bool {
    let here = same_path(&rule_path(file));
    rules
        .iter()
        .rfind(|r| r.path.as_deref().is_some_and(|p| same_path(p) == here) || r.name.as_deref() == Some(name))
        .is_some_and(|r| !r.enabled)
}

fn skill_of(
    folder: &str,
    file: PathBuf,
    origin: Origin,
    prefix: Option<&str>,
    config: &Path,
    rules: &[SkillRule],
) -> Skill {
    let front = std::fs::read_to_string(&file).ok().map(|t| frontmatter::read(&t));
    let (meta, state) = match front {
        None => (Default::default(), SkillState::Ok),
        Some(Front::NoBlock) => (Default::default(), SkillState::NoBlock),
        Some(Front::Unclear(meta)) => (meta, SkillState::Ok),
        Some(Front::Read(meta)) => {
            let described = meta.description.as_deref().is_some_and(|d| !d.trim().is_empty());
            let state = if described {
                SkillState::Ok
            } else {
                SkillState::Missing("description")
            };
            (meta, state)
        }
    };
    let own = meta
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(folder);
    let name = match prefix {
        Some(plugin) => format!("{plugin}:{own}"),
        None => own.to_string(),
    };
    let description = meta.description.as_deref().unwrap_or("");
    let off = turned_off(rules, &file, &name);
    let switch = match &origin {
        Origin::User | Origin::Project(_) => Ok(Switch::CodexSkill {
            file: config.to_path_buf(),
            skill: file.clone(),
        }),
        Origin::Bundled => Err(NoSwitch::Bundled),
        Origin::Plugin(id) => Err(NoSwitch::Plugin(id.clone())),
    };
    Skill {
        created_ms: crate::doctor::ext::created_ms(&file),
        used_as: vec![SkillUse::File(file.clone())],
        name,
        file,
        origin,
        state,
        listed_chars: description.chars().count(),
        first_line_chars: description.lines().next().unwrap_or("").chars().count(),
        name_only: false,
        off,
        hidden: false,
        switch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_distro_skill_turned_off_by_its_linux_path_reads_off() {
        let rule = |path: &str| SkillRule {
            path: Some(path.into()),
            name: None,
            enabled: false,
        };
        let file = Path::new(r"\\wsl.localhost\Ubuntu\home\a\.codex\skills\x\SKILL.md");
        assert!(turned_off(&[rule("/home/a/.codex/skills/x/SKILL.md")], file, "x"));
        assert!(!turned_off(&[rule("/home/a/.codex/skills/y/SKILL.md")], file, "x"));
        let windows = Path::new(r"C:\Users\a\.codex\skills\x\SKILL.md");
        assert!(turned_off(&[rule("c:/users/a/.codex/skills/x/SKILL.md")], windows, "x"));
    }

    /// A fresh fake home under the repo's target folder.
    fn fake(name: &str) -> (PathBuf, ExtRoots) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("test-ext-codex-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".codex")).unwrap();
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

    fn server<'a>(inv: &'a Inventory, name: &str) -> &'a Server {
        inv.servers
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no server {name}"))
    }

    fn skill<'a>(inv: &'a Inventory, name: &str) -> &'a Skill {
        inv.skills
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no skill {name}"))
    }

    #[test]
    fn codex_servers_off_and_switch() {
        let (root, roots) = fake("servers");
        let config = root.join(".codex").join("config.toml");
        write(
            &config,
            "[mcp_servers.a]\ncommand = \"node\"\n\n[mcp_servers.\"my.server\"]\ncommand = \"gone\"\nenabled = false\n\n[mcp_servers.remote]\nurl = \"https://x.example\"\n",
        );
        let project = root.join("proj");
        write(
            &project.join(".codex").join("config.toml"),
            "[mcp_servers.pj]\ncommand = \"node\"\n",
        );

        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert_eq!((inv.agent, inv.distro.clone(), inv.found), ("codex", None, true));
        assert_eq!(inv.caps.per_skill, Some(1024));
        assert!(inv.unreadable.is_empty());
        assert_eq!(inv.servers.len(), 4);

        let a = server(&inv, "a");
        assert_eq!(a.origin, Origin::User);
        assert!(!a.off);
        assert_eq!(
            a.transport,
            Transport::Stdio {
                command: "node".into(),
                state: CommandState::Found
            }
        );
        assert_eq!(
            a.switch,
            Ok(Switch::CodexServer {
                file: config.clone(),
                name: "a".into()
            })
        );

        let dotted = server(&inv, "my.server");
        assert!(dotted.off);
        assert_eq!(
            dotted.transport,
            Transport::Stdio {
                command: "gone".into(),
                state: CommandState::Missing
            }
        );
        assert_eq!(
            dotted.switch,
            Ok(Switch::CodexServer {
                file: config,
                name: "my.server".into()
            })
        );

        assert_eq!(server(&inv, "remote").transport, Transport::Remote);

        let pj = server(&inv, "pj");
        let project_file = project.join(".codex").join("config.toml");
        assert_eq!(pj.origin, Origin::Project(project));
        assert_eq!(pj.file, project_file);
        assert_eq!(pj.switch, Err(NoSwitch::Shared(project_file)));
    }

    #[test]
    fn codex_plugins_pick_latest() {
        let (root, roots) = fake("plugins");
        write(
            &root.join(".codex").join("config.toml"),
            "[plugins.\"on@mkt\"]\nenabled = true\n\n[plugins.\"off@mkt\"]\nenabled = false\n\n[plugins.\"vers@mkt\"]\nenabled = true\n",
        );
        let cache = root.join(".codex").join("plugins").join("cache").join("mkt");
        for v in ["1.0", "2.0", "latest"] {
            write(
                &cache.join("on").join(v).join(".mcp.json"),
                &format!(
                    "{{\"mcpServers\": {{\"from_{}\": {{\"command\": \"node\"}}}}}}",
                    v.replace('.', "_")
                ),
            );
        }
        write(
            &cache.join("off").join("latest").join(".mcp.json"),
            "{\"mcpServers\": {\"hidden\": {\"url\": \"https://x\"}}}",
        );
        write(
            &cache.join("vers").join("1.0").join(".mcp.json"),
            "{\"mcpServers\": {\"v1\": {\"url\": \"https://x\"}}}",
        );
        write(
            &cache.join("vers").join("2.0").join(".mcp.json"),
            "{\"mcpServers\": {\"v2\": {\"url\": \"https://x\"}}}",
        );

        let inv = inventory(&roots, &[], &judge);
        let mut names: Vec<&str> = inv.servers.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["from_latest", "v2"]);
        let s = server(&inv, "from_latest");
        assert_eq!(s.origin, Origin::Plugin("on@mkt".into()));
        assert_eq!(s.switch, Err(NoSwitch::Plugin("on@mkt".into())));
        assert_eq!(s.file, cache.join("on").join("latest").join(".mcp.json"));
        assert_eq!(
            s.transport,
            Transport::Stdio {
                command: "node".into(),
                state: CommandState::Found
            }
        );
        assert_eq!(server(&inv, "v2").transport, Transport::Remote);
    }

    #[test]
    fn codex_skills() {
        let (root, roots) = fake("skills");
        let config = root.join(".codex").join("config.toml");
        let skills = root.join(".codex").join("skills");
        write(
            &skills.join("sys").join("SKILL.md"),
            "---\nname: sys\ndescription: built in\n---\nbody\n",
        );
        write(
            &skills.join("alpha").join("SKILL.md"),
            "---\ndescription: Does alpha things\n---\nbody\n",
        );
        write(
            &skills.join("bravo").join("SKILL.md"),
            "---\nname: bravo\ndescription: Bravo\n---\nbody\n",
        );
        write(
            &skills.join("charlie").join("SKILL.md"),
            "---\nname: charlie\ndescription: Charlie\n---\nbody\n",
        );
        write(&skills.join("empty").join("SKILL.md"), "---\nname: empty\n---\nbody\n");
        write(&skills.join("plain").join("SKILL.md"), "no frontmatter here\n");
        write(
            &skills.join(".system").join("sys").join("SKILL.md"),
            "---\nname: sys\ndescription: built in\n---\n",
        );
        write(
            &root.join(".agents").join("skills").join("shared").join("SKILL.md"),
            "---\ndescription: Shared one\n---\n",
        );

        let backslashed = |p: &Path| p.display().to_string().replace('/', "\\");
        let bravo = backslashed(&skills.join("bravo").join("SKILL.md"));
        let charlie = backslashed(&skills.join("charlie").join("SKILL.md")).to_uppercase();
        write(
            &config,
            &format!(
                "[[skills.config]]\npath = '{bravo}'\nenabled = false\n\n[[skills.config]]\npath = '{charlie}'\nenabled = false\n\n[[skills.config]]\npath = '{charlie}'\nenabled = true\n\n[[skills.config]]\nname = \"alpha\"\nenabled = false\n"
            ),
        );

        let inv = inventory(&roots, &[], &judge);
        assert!(inv.unreadable.is_empty());
        let sys: Vec<&Skill> = inv.skills.iter().filter(|s| s.name == "sys").collect();
        assert_eq!(sys.len(), 2);
        let bundled = sys.iter().find(|s| s.origin == Origin::Bundled).unwrap();
        assert_eq!(bundled.switch, Err(NoSwitch::Bundled));

        let bravo = skill(&inv, "bravo");
        assert!(bravo.off);
        assert_eq!(bravo.origin, Origin::User);
        assert_eq!(
            bravo.switch,
            Ok(Switch::CodexSkill {
                file: config.clone(),
                skill: skills.join("bravo").join("SKILL.md")
            })
        );
        assert!(!skill(&inv, "charlie").off);
        let alpha = skill(&inv, "alpha");
        assert!(alpha.off);
        assert_eq!(alpha.listed_chars, "Does alpha things".len());
        assert_eq!(alpha.state, SkillState::Ok);

        assert_eq!(skill(&inv, "empty").state, SkillState::Missing("description"));
        assert_eq!(skill(&inv, "plain").state, SkillState::NoBlock);
        let shared = skill(&inv, "shared");
        assert_eq!(shared.origin, Origin::User);
        assert!(shared.switch.is_ok());
    }

    #[test]
    fn codex_project_and_plugin_skills() {
        let (root, roots) = fake("skills-other");
        let project = root.join("proj");
        write(
            &project.join(".agents").join("skills").join("pskill").join("SKILL.md"),
            "---\ndescription: Project one\n---\n",
        );
        write(
            &root.join(".codex").join("config.toml"),
            "[plugins.\"pl@mkt\"]\nenabled = true\n",
        );
        let folder = root
            .join(".codex")
            .join("plugins")
            .join("cache")
            .join("mkt")
            .join("pl")
            .join("1.0");
        write(
            &folder.join("skills").join("ps").join("SKILL.md"),
            "---\nname: ps\ndescription: Plugin one\n---\n",
        );

        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        let p = skill(&inv, "pskill");
        assert_eq!(p.origin, Origin::Project(project));
        assert!(matches!(p.switch, Ok(Switch::CodexSkill { .. })));
        let ps = skill(&inv, "pl:ps");
        assert_eq!(ps.origin, Origin::Plugin("pl@mkt".into()));
        assert_eq!(ps.switch, Err(NoSwitch::Plugin("pl@mkt".into())));
    }

    fn project_server(project: &Path) {
        write(
            &project.join(".codex").join("config.toml"),
            "[mcp_servers.pj]\ncommand = \"node\"\n",
        );
    }

    #[test]
    fn codex_project_trust() {
        let slashed = |p: &Path| p.display().to_string().replace('\\', "/").to_lowercase();
        let trusting = |key: &str| format!("[projects.'{key}']\ntrust_level = \"trusted\"\n");
        let launch = |inv: &Inventory| server(inv, "pj").launch.clone();

        // Trusted by its own folder, in lower-case forward slashes.
        let (root, roots) = fake("trust-own");
        let project = root.join("proj");
        project_server(&project);
        write(&root.join(".codex").join("config.toml"), &trusting(&slashed(&project)));
        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert!(launch(&inv).is_ok(), "{:?}", launch(&inv));

        // The same under a `\\?\` key with backslashes and capitals.
        let (root, roots) = fake("trust-verbatim");
        let project = root.join("proj");
        project_server(&project);
        let key = format!(
            r"\\?\{}",
            project.display().to_string().replace('/', "\\").to_uppercase()
        );
        write(&root.join(".codex").join("config.toml"), &trusting(&key));
        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert!(launch(&inv).is_ok(), "{:?}", launch(&inv));

        // A subfolder is trusted through its git root.
        let (root, roots) = fake("trust-git");
        let top = root.join("repo");
        std::fs::create_dir_all(top.join(".git")).unwrap();
        let project = top.join("sub");
        project_server(&project);
        write(&root.join(".codex").join("config.toml"), &trusting(&slashed(&top)));
        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert!(launch(&inv).is_ok(), "{:?}", launch(&inv));

        // Untrusted: no entry, another folder, or a level other than "trusted".
        let (root, roots) = fake("trust-none");
        let project = root.join("proj");
        project_server(&project);
        let other = format!(
            "[projects.'{}']\ntrust_level = \"trusted\"\n\n[projects.'{}']\ntrust_level = \"untrusted\"\n",
            slashed(&root.join("elsewhere")),
            slashed(&project)
        );
        write(&root.join(".codex").join("config.toml"), &other);
        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert_eq!(launch(&inv), Err(NotMeasured::NotApproved));
    }

    #[test]
    fn codex_launch_and_cost() {
        let (root, roots) = fake("launch");
        write(
            &root.join(".codex").join("config.toml"),
            "[mcp_servers.a]\ncommand = \"node\"\nargs = [\"x.js\"]\nenv_vars = [\"FOO\"]\nstartup_timeout_ms = 1500\nenabled_tools = [\"t1\"]\ndisabled_tools = [\"t2\"]\n\n[mcp_servers.a.env]\nK = \"v\"\n\n[mcp_servers.slow]\ncommand = \"node\"\nstartup_timeout_sec = 500\ncwd = \"w\"\n\n[mcp_servers.remote]\nurl = \"https://x\"\n\n[plugins.\"pl@mkt\"]\nenabled = true\n",
        );
        let cache = root
            .join(".codex")
            .join("plugins")
            .join("cache")
            .join("mkt")
            .join("pl")
            .join("latest");
        write(
            &cache.join(".mcp.json"),
            "{\"mcpServers\": {\"cua\": {\"command\": \"node\", \"startup_timeout_sec\": 45, \"omit_tools_from\": [\"deferred\"], \"enabled_tools\": [\"click\"]}, \"plain\": {\"command\": \"node\"}}}",
        );
        let inv = inventory(&roots, &[], &judge);

        let a = server(&inv, "a");
        let launch = a.launch.as_ref().unwrap();
        let Inherit::Only(names) = &launch.inherit else {
            panic!("{:?}", launch.inherit)
        };
        assert!(names.contains(&"PATH".to_string()) && names.contains(&"FOO".to_string()));
        assert_eq!(names.len(), 24);
        assert_eq!(launch.env, vec![("K".to_string(), "v".to_string())]);
        assert_eq!(launch.args, vec!["x.js".to_string()]);
        assert_eq!(launch.expand, Expand::None);
        assert_eq!(launch.timeout_secs, 2);
        assert_eq!(launch.cwd, Some(root.clone()));
        assert_eq!(
            a.cost.listing,
            Listing::Namespace {
                namespace: "mcp__a".into(),
                cap: 250
            }
        );
        assert_eq!(a.cost.only, Some(vec!["t1".to_string()]));
        assert_eq!(a.cost.except, vec!["t2".to_string()]);
        assert_eq!(a.used_as, UsedAs::Server("a".into()));

        let slow = server(&inv, "slow").launch.as_ref().unwrap();
        assert_eq!(slow.timeout_secs, 60);
        assert_eq!(slow.cwd, Some(PathBuf::from("w")));
        assert_eq!(server(&inv, "remote").launch, Err(NotMeasured::Remote));

        let cua = server(&inv, "cua");
        assert_eq!(
            cua.cost.listing,
            Listing::Full {
                prefix: "mcp__cua__".into(),
                desc_cap: None
            }
        );
        assert_eq!(cua.cost.only, Some(vec!["click".to_string()]));
        assert_eq!(cua.launch.as_ref().unwrap().timeout_secs, 45);
        assert!(matches!(server(&inv, "plain").cost.listing, Listing::Namespace { .. }));
        assert_eq!(server(&inv, "plain").launch.as_ref().unwrap().timeout_secs, 30);

        // A project server starts in its project.
        let project = root.join("proj");
        project_server(&project);
        let mut text = std::fs::read_to_string(root.join(".codex").join("config.toml")).unwrap();
        text.push_str(&format!(
            "\n[projects.'{}']\ntrust_level = \"trusted\"\n",
            project.display().to_string().replace('\\', "/").to_lowercase()
        ));
        write(&root.join(".codex").join("config.toml"), &text);
        let inv = inventory(&roots, std::slice::from_ref(&project), &judge);
        assert_eq!(server(&inv, "pj").launch.as_ref().unwrap().cwd, Some(project.clone()));

        // Hints and skill use.
        let hint = (inv.hints.move_server)("pj", Path::new(r"C:\p"));
        assert_eq!(
            hint,
            r"To load it only there, move its [mcp_servers.pj] table to C:\p\.codex\config.toml. Codex reads that file only in a trusted project."
        );
        assert_eq!(
            (inv.hints.move_skill)("s", Path::new(r"C:\p")),
            r"To load it only there, move its folder to C:\p\.agents\skills."
        );
    }

    #[test]
    fn codex_skill_use_is_its_file() {
        let (root, roots) = fake("skill-use");
        let file = root.join(".codex").join("skills").join("alpha").join("SKILL.md");
        write(&file, "---\ndescription: d\n---\n");
        let inv = inventory(&roots, &[], &judge);
        assert_eq!(skill(&inv, "alpha").used_as, vec![SkillUse::File(file)]);
    }

    #[test]
    fn codex_empty_config() {
        let (root, roots) = fake("empty");
        let inv = inventory(&roots, &[], &judge);
        assert!(inv.servers.is_empty() && inv.skills.is_empty() && inv.unreadable.is_empty());

        write(&root.join(".codex").join("config.toml"), "model = \"x\"\n");
        let inv = inventory(&roots, &[], &judge);
        assert!(inv.servers.is_empty() && inv.unreadable.is_empty());

        let config = root.join(".codex").join("config.toml");
        write(&config, "[mcp_servers.a\ncommand = \n");
        let inv = inventory(&roots, &[], &judge);
        assert!(inv.servers.is_empty());
        assert_eq!(inv.unreadable.len(), 1);
        assert_eq!(inv.unreadable[0].0, config);
    }

    #[test]
    fn codex_unreadable_files_are_described_in_one_sentence_that_never_quotes_them() {
        let (root, roots) = fake("unreadable-message");
        let config = root.join(".codex").join("config.toml");
        write(
            &config,
            "model = \"x\"\n\n[mcp_servers.a]\nbearer_token = \"sk-SECRET\n\n[plugins.\"p@m\"]\nenabled = true\n",
        );
        let inv = inventory(&roots, &[], &judge);
        let message = inv.unreadable.iter().find(|(f, _)| *f == config).unwrap().1.clone();
        assert_eq!(message, "It doesn't parse (line 4).");
        assert!(!message.contains("SECRET"), "{message}");

        // A plugin's `.mcp.json`, with the plugin on.
        write(&config, "[plugins.\"on@mkt\"]\nenabled = true\n");
        let plugin = root
            .join(".codex")
            .join("plugins")
            .join("cache")
            .join("mkt")
            .join("on")
            .join("latest")
            .join(".mcp.json");
        write(
            &plugin,
            "{\n  \"mcpServers\": {\n    \"x\": \"sk-SECRET\" oops\n  }\n}\n",
        );
        let inv = inventory(&roots, &[], &judge);
        let message = inv.unreadable.iter().find(|(f, _)| *f == plugin).unwrap().1.clone();
        assert_eq!(message, "It doesn't parse (line 4).");
        std::fs::write(&plugin, [0xff, 0xfe, 0x41]).unwrap();
        let inv = inventory(&roots, &[], &judge);
        let message = inv.unreadable.iter().find(|(f, _)| *f == plugin).unwrap().1.clone();
        assert_eq!(message, "It isn't UTF-8 text.");
    }
}
