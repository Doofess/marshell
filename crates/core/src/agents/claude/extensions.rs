//! Claude Code's MCP servers and skills for the Doctor: where it keeps them, how each is turned off, and what its
//! skill listing cuts. The readers take their roots as arguments and never read the environment.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::jsonc::read_value;
use crate::doctor::ext::{
    Caps, CommandState, CostRule, Expand, ExtRoots, Hints, Inherit, Inventory, Launch, Listing, NoSwitch, NotMeasured,
    Origin, Server, Skill, SkillState, SkillUse, Switch, Transport, UsedAs, created_ms, sanitize_tool_part,
    skill_files, unreadable,
};
use crate::doctor::frontmatter::{self, Front, SkillMeta};

/// Claude Code cuts `description` plus `when_to_use` at this many characters.
const PER_SKILL_CAP: usize = 1536;

/// JSON files read once each; a file that exists and doesn't parse is remembered in `unreadable`.
#[derive(Default)]
struct Files {
    cache: HashMap<PathBuf, Option<Value>>,
    unreadable: Vec<(PathBuf, String)>,
}

impl Files {
    fn json(&mut self, path: &Path) -> Option<Value> {
        if let Some(v) = self.cache.get(path) {
            return v.clone();
        }
        let value = match std::fs::read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => match read_value(&text) {
                    Ok(v) => Some(v),
                    Err(_) => self.fail(path, unreadable::json(&text)),
                },
                Err(_) => self.fail(path, unreadable::NOT_UTF8.to_string()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => self.fail(path, unreadable::io(&e)),
        };
        self.cache.insert(path.to_path_buf(), value.clone());
        value
    }

    fn fail(&mut self, path: &Path, message: String) -> Option<Value> {
        self.unreadable.push((path.to_path_buf(), message));
        None
    }
}

/// A JSON pointer segment: `~` is `~0` and `/` is `~1`.
fn esc(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

/// A project path as `~/.claude.json` keys it: forward slashes, no trailing slash, case ignored.
fn norm(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn transport(entry: &Value, command: &dyn Fn(&str) -> CommandState) -> Transport {
    let kind = entry.get("type").and_then(Value::as_str).unwrap_or("");
    let cmd = entry
        .get("command")
        .and_then(Value::as_str)
        .filter(|c| !c.trim().is_empty());
    let has_url = entry.get("url").and_then(Value::as_str).is_some_and(|u| !u.is_empty());
    if matches!(kind, "http" | "streamable-http" | "sse") || (cmd.is_none() && has_url) {
        Transport::Remote
    } else if let Some(c) = cmd {
        Transport::Stdio {
            command: c.to_string(),
            state: command(c),
        }
    } else {
        Transport::Nothing
    }
}

struct Where<'a> {
    file: &'a Path,
    origin: Origin,
}

/// What `launch_of` and `cost_of` need besides the entry.
struct Ctx<'a> {
    home: &'a Path,
    timeout_secs: u64,
    /// `ENABLE_TOOL_SEARCH`, from the user settings, else Marshell's own.
    tool_search: Option<&'a str>,
    /// The project folder, for project and local servers.
    project: Option<&'a Path>,
    /// The plugin's key and install folder, for a plugin server.
    plugin: Option<(&'a str, &'a Path)>,
    approved: &'a dyn Fn(&str) -> bool,
}

/// `MCP_TIMEOUT` (ms) rounded up to seconds, else 30, at most 60.
fn timeout_of(user_settings: Option<&Value>) -> u64 {
    let raw = user_settings.and_then(|s| s.get("env")?.get("MCP_TIMEOUT"));
    let ms = match raw {
        Some(Value::String(t)) => t.trim().parse::<u64>().ok(),
        Some(v) => v.as_u64(),
        None => None,
    };
    match ms {
        Some(ms) if ms > 0 => ms.div_ceil(1000).min(60),
        _ => 30,
    }
}

fn pairs(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn launch_of(name: &str, entry: &Value, remote: bool, ctx: &Ctx) -> Result<Launch, NotMeasured> {
    if remote {
        return Err(NotMeasured::Remote);
    }
    let Some(command) = entry
        .get("command")
        .and_then(Value::as_str)
        .filter(|c| !c.trim().is_empty())
    else {
        // A server with no command has nothing to start.
        return Err(NotMeasured::NotApproved);
    };
    if !(ctx.approved)(name) {
        return Err(NotMeasured::NotApproved);
    }
    let mut set = Vec::new();
    if let Some(project) = ctx.project {
        set.push(("CLAUDE_PROJECT_DIR".to_string(), project.display().to_string()));
    }
    if let Some((_, install)) = ctx.plugin {
        set.push(("CLAUDE_PLUGIN_ROOT".to_string(), install.display().to_string()));
    }
    Ok(Launch {
        command: command.to_string(),
        args: strings(entry.get("args")),
        env: pairs(entry.get("env")),
        inherit: Inherit::All,
        set,
        cwd: Some(ctx.project.unwrap_or(ctx.home).to_path_buf()),
        expand: Expand::Braces,
        timeout_secs: ctx.timeout_secs,
    })
}

fn cost_of(prefix: String, entry: &Value, tool_search: Option<&str>) -> CostRule {
    let names = Listing::Names {
        prefix: prefix.clone(),
        instructions_cap: Some(4096),
    };
    let full = Listing::Full {
        prefix: prefix.clone(),
        desc_cap: Some(4096),
    };
    let listing = if entry.get("alwaysLoad").and_then(Value::as_bool) == Some(true) {
        full
    } else {
        match tool_search.map(|t| t.trim().to_lowercase()).as_deref() {
            Some("false") => full,
            Some("auto") => Listing::FullUnder {
                percent: 10,
                prefix,
                desc_cap: Some(4096),
                instructions_cap: Some(4096),
            },
            Some(t) if t.starts_with("auto:") => match t["auto:".len()..].parse::<u32>() {
                Ok(percent) => Listing::FullUnder {
                    percent,
                    prefix,
                    desc_cap: Some(4096),
                    instructions_cap: Some(4096),
                },
                Err(_) => names,
            },
            _ => names,
        }
    };
    CostRule {
        listing,
        always_load_meta: Some("anthropic/alwaysLoad"),
        only: None,
        except: Vec::new(),
    }
}

/// The servers in an `mcpServers` object. `off` and `switch` are asked per name.
fn servers_in(
    map: Option<&Value>,
    at: Where,
    command: &dyn Fn(&str) -> CommandState,
    off: &dyn Fn(&str) -> bool,
    switch: &dyn Fn(&str) -> Result<Switch, NoSwitch>,
    ctx: &Ctx,
    out: &mut Vec<Server>,
) {
    let Some(map) = map.and_then(Value::as_object) else {
        return;
    };
    for (name, entry) in map {
        let transport = transport(entry, command);
        // A plugin server reads `plugin:<plugin>:<server>` in the CLI.
        let display = match ctx.plugin {
            Some((key, _)) => format!("plugin:{}:{name}", key.split('@').next().unwrap_or(key)),
            None => name.clone(),
        };
        let prefix = format!("mcp__{}__", sanitize_tool_part(&display));
        out.push(Server {
            name: name.clone(),
            file: at.file.to_path_buf(),
            origin: at.origin.clone(),
            launch: launch_of(name, entry, transport == Transport::Remote, ctx),
            cost: cost_of(prefix.clone(), entry, ctx.tool_search),
            used_as: UsedAs::ToolPrefix(prefix),
            transport,
            off: off(name),
            switch: switch(name),
        });
    }
}

/// The text after the frontmatter block, or all of it when there is none.
fn body_of(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|l| l.trim()) == Some("---")
        && let Some(close) = lines.iter().skip(1).position(|l| l.trim() == "---")
    {
        return lines[close + 2..].join("\n");
    }
    text.to_string()
}

fn first_paragraph(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    body_of(text)
        .lines()
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

struct Parsed {
    name: String,
    listed_chars: usize,
    first_line_chars: usize,
    hidden: bool,
}

/// What the listing shows of the skill at `file`; None when the file can't be read.
fn parse_skill(folder: &str, file: &Path) -> Option<Parsed> {
    let text = std::fs::read_to_string(file).ok()?;
    let meta = match frontmatter::read(&text) {
        Front::Read(m) | Front::Unclear(m) => m,
        Front::NoBlock => SkillMeta::default(),
    };
    let described = meta.description.as_deref().map(str::trim).filter(|d| !d.is_empty());
    let description = match described {
        Some(d) => d.to_string(),
        None => first_paragraph(&text),
    };
    let when = meta.when_to_use.as_deref().unwrap_or("").chars().count();
    let name = meta
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(folder)
        .to_string();
    Some(Parsed {
        name,
        listed_chars: description.chars().count() + when,
        first_line_chars: description.lines().next().unwrap_or("").chars().count(),
        hidden: meta.hidden,
    })
}

/// The `skillOverrides` values for `name` in these settings files.
fn overrides<'a>(settings: &'a [Option<Value>], name: &str) -> Vec<&'a str> {
    settings
        .iter()
        .flatten()
        .filter_map(|s| s.get("skillOverrides")?.get(name)?.as_str())
        .collect()
}

fn skills_in(
    dir: &Path,
    origin: &Origin,
    prefix: Option<&str>,
    settings: &[Option<Value>],
    switch: &dyn Fn(&str) -> Result<Switch, NoSwitch>,
    out: &mut Vec<Skill>,
) {
    for (folder, file) in skill_files(dir) {
        let Some(p) = parse_skill(&folder, &file) else {
            continue;
        };
        let name = match prefix {
            Some(pre) => format!("{pre}:{}", p.name),
            None => p.name,
        };
        let ov = overrides(settings, &name);
        out.push(Skill {
            created_ms: created_ms(&file),
            used_as: vec![SkillUse::Name(name.clone())],
            file,
            origin: origin.clone(),
            state: SkillState::Ok,
            listed_chars: p.listed_chars,
            first_line_chars: p.first_line_chars,
            name_only: ov.contains(&"name-only"),
            off: ov.contains(&"off"),
            // "user-invocable-only" keeps the skill out of the model's listing, as `disable-model-invocation` does.
            hidden: p.hidden || ov.contains(&"user-invocable-only"),
            switch: switch(&name),
            name,
        });
    }
}

fn skill_switch(file: &Path, name: &str) -> Result<Switch, NoSwitch> {
    Ok(Switch::JsonSet {
        file: file.to_path_buf(),
        pointer: format!("/skillOverrides/{}", esc(name)),
        value: "\"off\"".to_string(),
    })
}

/// `command` judges a stdio server's command. `projects` are the folders to read project files in.
pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory {
    let mut files = Files::default();
    let mut servers = Vec::new();
    let mut skills = Vec::new();
    let claude_json = files.json(&roots.claude_json);
    let user_settings = files.json(&roots.claude_settings);
    let timeout_secs = timeout_of(user_settings.as_ref());
    let tool_search = user_settings
        .as_ref()
        .and_then(|s| s.get("env")?.get("ENABLE_TOOL_SEARCH")?.as_str())
        .or(roots.claude_tool_search.as_deref());
    let everyone = |_: &str| true;
    let base = || Ctx {
        home: &roots.home,
        timeout_secs,
        tool_search,
        project: None,
        plugin: None,
        approved: &everyone,
    };

    // User-level servers.
    servers_in(
        claude_json.as_ref().and_then(|v| v.get("mcpServers")),
        Where {
            file: &roots.claude_json,
            origin: Origin::User,
        },
        command,
        &|_| false,
        &|_| Err(NoSwitch::ClaudeUser),
        &base(),
        &mut servers,
    );

    // User-level skills.
    skills_in(
        &roots.claude_dir.join("skills"),
        &Origin::User,
        None,
        std::slice::from_ref(&user_settings),
        &|n| skill_switch(&roots.claude_settings, n),
        &mut skills,
    );

    // Plugins that are turned on.
    let enabled: Vec<String> = user_settings
        .as_ref()
        .and_then(|s| s.get("enabledPlugins"))
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v.as_bool() == Some(true))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    if !enabled.is_empty() {
        let installed = files.json(&roots.claude_dir.join("plugins").join("installed_plugins.json"));
        for key in enabled {
            let Some(install) = installed
                .as_ref()
                .and_then(|i| i.get("plugins")?.get(&key)?.get(0)?.get("installPath")?.as_str())
                .map(PathBuf::from)
            else {
                continue;
            };
            let mcp = install.join(".mcp.json");
            let plugin_servers = files.json(&mcp);
            servers_in(
                plugin_servers.as_ref().and_then(|v| v.get("mcpServers")),
                Where {
                    file: &mcp,
                    origin: Origin::Plugin(key.clone()),
                },
                command,
                &|_| false,
                &|_| Err(NoSwitch::Plugin(key.clone())),
                &Ctx {
                    plugin: Some((&key, &install)),
                    ..base()
                },
                &mut servers,
            );
            let plugin = key.split('@').next().unwrap_or(&key).to_string();
            skills_in(
                &install.join("skills"),
                &Origin::Plugin(key.clone()),
                Some(&plugin),
                std::slice::from_ref(&user_settings),
                &|_| Err(NoSwitch::Plugin(key.clone())),
                &mut skills,
            );
        }
    }

    // Projects.
    for project in projects {
        let wanted = norm(&project.to_string_lossy());
        let entries: Vec<(String, Value)> = claude_json
            .as_ref()
            .and_then(|v| v.get("projects"))
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter(|(k, _)| norm(k) == wanted)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        // Local scope: the project's entry in `~/.claude.json`.
        for (key, entry) in &entries {
            let disabled = strings(entry.get("disabledMcpServers"));
            servers_in(
                entry.get("mcpServers"),
                Where {
                    file: &roots.claude_json,
                    origin: Origin::Project(project.clone()),
                },
                command,
                &|n| disabled.iter().any(|d| d == n),
                &|n| {
                    Ok(Switch::JsonAdd {
                        file: roots.claude_json.clone(),
                        pointer: format!("/projects/{}/disabledMcpServers", esc(key)),
                        name: n.to_string(),
                    })
                },
                &Ctx {
                    project: Some(project),
                    ..base()
                },
                &mut servers,
            );
        }

        // Project scope: `<p>/.mcp.json`.
        let local_file = project.join(".claude").join("settings.local.json");
        let local = files.json(&local_file);
        let shared = files.json(&project.join(".claude").join("settings.json"));
        let mcp = project.join(".mcp.json");
        let mcp_json = files.json(&mcp);
        let mut disabled: Vec<String> = Vec::new();
        for s in [&local, &shared, &user_settings].into_iter().flatten() {
            disabled.extend(strings(s.get("disabledMcpjsonServers")));
        }
        for (_, entry) in &entries {
            disabled.extend(strings(entry.get("disabledMcpjsonServers")));
        }
        // Approval counts only in a folder whose trust dialog was accepted: Claude Code starts no project server before
        // that, and a cloned repo can ship its own `settings.local.json` (decision 17). Trust is read from the folder's
        // own entry; one inherited from a parent folder isn't, so such a server reads as not approved.
        let trusted = entries
            .iter()
            .any(|(_, e)| e.get("hasTrustDialogAccepted").and_then(Value::as_bool) == Some(true));
        let approved = |name: &str| {
            let counts = |s: &Value| {
                s.get("enableAllProjectMcpServers").and_then(Value::as_bool) == Some(true)
                    || strings(s.get("enabledMcpjsonServers")).iter().any(|n| n == name)
            };
            trusted
                && ([&user_settings, &local, &shared].into_iter().flatten().any(&counts)
                    || entries.iter().any(|(_, e)| counts(e)))
        };
        servers_in(
            mcp_json.as_ref().and_then(|v| v.get("mcpServers")),
            Where {
                file: &mcp,
                origin: Origin::Project(project.clone()),
            },
            command,
            &|n| disabled.iter().any(|d| d == n),
            &|n| {
                Ok(Switch::JsonAdd {
                    file: local_file.clone(),
                    pointer: "/disabledMcpjsonServers".to_string(),
                    name: n.to_string(),
                })
            },
            &Ctx {
                project: Some(project),
                approved: &approved,
                ..base()
            },
            &mut servers,
        );

        // Project skills.
        skills_in(
            &project.join(".claude").join("skills"),
            &Origin::Project(project.clone()),
            None,
            &[local, shared, user_settings.clone()],
            &|n| skill_switch(&local_file, n),
            &mut skills,
        );
    }

    Inventory {
        agent: "claude",
        distro: None,
        found: true,
        caps: Caps {
            per_skill: Some(PER_SKILL_CAP),
            listing_first_line_over: None,
        },
        servers,
        skills,
        unreadable: files.unreadable,
        hints: Hints {
            move_server: |name, _| {
                format!(
                    "To load it only there, add it in that project with claude mcp add -s local, and remove the user-level entry with claude mcp remove {name} -s user."
                )
            },
            move_skill: |_, project| {
                format!(
                    "To load it only there, move its folder to {}\\.claude\\skills.",
                    project.display()
                )
            },
        },
        cost_note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::ext::{NoSwitch, Origin, Server, Skill, SkillState, Switch, Transport};
    use serde_json::json;
    use std::path::Path;

    fn fake_home(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("test-ext-claude-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn roots(h: &Path) -> ExtRoots {
        ExtRoots {
            home: h.to_path_buf(),
            claude_dir: h.join(".claude"),
            claude_json: h.join(".claude.json"),
            claude_settings: h.join(".claude").join("settings.json"),
            codex_home: h.join(".codex"),
            qwen_dir: h.join(".qwen"),
            claude_tool_search: None,
            qwen_runtime: h.join(".qwen"),
            gemini_dir: h.join(".gemini"),
        }
    }

    fn put(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn found(_: &str) -> CommandState {
        CommandState::Found
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
    fn claude_scopes_and_switches() {
        let h = fake_home("scopes");
        let r = roots(&h);
        let proj = h.join("proj");
        let cache = h.join("plugcache");
        put(
            &r.claude_json,
            &json!({
                "mcpServers": {"usr": {"command": "u"}},
                "projects": {"C:/x/proj": {"mcpServers": {"loc": {"command": "l"}}}}
            })
            .to_string(),
        );
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"shared": {"command": "s"}}}).to_string(),
        );
        put(
            &r.claude_settings,
            &json!({"enabledPlugins": {"plug@mk": true, "off@mk": false}}).to_string(),
        );
        put(
            &r.claude_dir.join("plugins").join("installed_plugins.json"),
            &json!({"version": 2, "plugins": {"plug@mk": [{"scope": "user", "installPath": cache.to_string_lossy()}]}})
                .to_string(),
        );
        put(
            &cache.join(".mcp.json"),
            &json!({"mcpServers": {"ps": {"command": "p"}}}).to_string(),
        );

        let local = PathBuf::from(r"C:\x\proj");
        let inv = inventory(&r, &[local.clone(), proj.clone()], &found);

        assert_eq!(inv.agent, "claude");
        assert_eq!(inv.distro, None);
        assert!(inv.found);
        assert_eq!(inv.caps.per_skill, Some(1536));
        assert!(inv.unreadable.is_empty());

        let u = server(&inv, "usr");
        assert_eq!(u.origin, Origin::User);
        assert_eq!(u.switch, Err(NoSwitch::ClaudeUser));
        assert!(!u.off);
        assert_eq!(u.file, r.claude_json);

        let l = server(&inv, "loc");
        assert_eq!(l.origin, Origin::Project(local));
        assert_eq!(
            l.switch,
            Ok(Switch::JsonAdd {
                file: r.claude_json.clone(),
                pointer: "/projects/C:~1x~1proj/disabledMcpServers".into(),
                name: "loc".into()
            })
        );

        let s = server(&inv, "shared");
        assert_eq!(s.origin, Origin::Project(proj.clone()));
        assert_eq!(s.file, proj.join(".mcp.json"));
        assert_eq!(
            s.switch,
            Ok(Switch::JsonAdd {
                file: proj.join(".claude").join("settings.local.json"),
                pointer: "/disabledMcpjsonServers".into(),
                name: "shared".into()
            })
        );

        let p = server(&inv, "ps");
        assert_eq!(p.origin, Origin::Plugin("plug@mk".into()));
        assert_eq!(p.switch, Err(NoSwitch::Plugin("plug@mk".into())));
        assert_eq!(inv.servers.len(), 4);
    }

    #[test]
    fn claude_off_lists() {
        let h = fake_home("off");
        let r = roots(&h);
        let proj = h.join("proj");
        put(
            &r.claude_json,
            &json!({"projects": {
                "c:/X/Proj/": {"mcpServers": {"loc": {"command": "l"}, "live": {"command": "l"}}, "disabledMcpServers": ["loc"]},
                proj.to_string_lossy().replace('\\', "/"): {"disabledMcpjsonServers": ["from-entry"]}
            }})
            .to_string(),
        );
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"shared": {"command": "s"}, "other": {"command": "o"}, "from-entry": {"command": "e"},
                "from-user": {"command": "e"}, "from-project": {"command": "e"}}})
            .to_string(),
        );
        put(
            &proj.join(".claude").join("settings.local.json"),
            &json!({"disabledMcpjsonServers": ["shared"]}).to_string(),
        );
        put(
            &proj.join(".claude").join("settings.json"),
            &json!({"disabledMcpjsonServers": ["from-project"]}).to_string(),
        );
        put(
            &r.claude_settings,
            &json!({"disabledMcpjsonServers": ["from-user"]}).to_string(),
        );

        let inv = inventory(&r, &[PathBuf::from(r"C:\x\proj"), proj], &found);

        assert!(server(&inv, "loc").off);
        assert!(!server(&inv, "live").off);
        assert!(server(&inv, "shared").off);
        assert!(server(&inv, "from-entry").off);
        assert!(server(&inv, "from-user").off);
        assert!(server(&inv, "from-project").off);
        assert!(!server(&inv, "other").off);
    }

    #[test]
    fn claude_skills() {
        let h = fake_home("skills");
        let r = roots(&h);
        let proj = h.join("proj");
        let cache = h.join("plugcache");
        let skills = r.claude_dir.join("skills");
        put(
            &skills.join("nofm").join("SKILL.md"),
            "Does the thing\n\nMore text later.\n",
        );
        put(
            &skills.join("withfm").join("SKILL.md"),
            "---\nname: renamed\ndescription: abcd\nwhen_to_use: efg\ndisable-model-invocation: true\n---\nBody\n",
        );
        put(
            &skills.join("weird").join("SKILL.md"),
            "---\nname: \"a/b~c\"\ndescription: d\n---\n",
        );
        put(
            &r.claude_settings,
            &json!({
                "enabledPlugins": {"plug@mk": true},
                "skillOverrides": {"nofm": "off", "renamed": "name-only", "a/b~c": "user-invocable-only"}
            })
            .to_string(),
        );
        put(
            &r.claude_dir.join("plugins").join("installed_plugins.json"),
            &json!({"plugins": {"plug@mk": [{"installPath": cache.to_string_lossy()}]}}).to_string(),
        );
        put(
            &cache.join("skills").join("skill").join("SKILL.md"),
            "Plugin skill text\n",
        );
        put(
            &proj.join(".claude").join("skills").join("ps").join("SKILL.md"),
            "---\ndescription: xy\n---\n",
        );

        let inv = inventory(&r, std::slice::from_ref(&proj), &found);

        let n = skill(&inv, "nofm");
        assert_eq!(n.state, SkillState::Ok);
        assert_eq!(n.listed_chars, "Does the thing".chars().count());
        assert_eq!(n.origin, Origin::User);
        assert!(n.off);
        assert!(!n.name_only);
        assert_eq!(n.file, skills.join("nofm").join("SKILL.md"));
        assert_eq!(
            n.switch,
            Ok(Switch::JsonSet {
                file: r.claude_settings.clone(),
                pointer: "/skillOverrides/nofm".into(),
                value: "\"off\"".into()
            })
        );

        let w = skill(&inv, "renamed");
        assert_eq!(w.listed_chars, 7);
        assert!(w.name_only);
        assert!(!w.off);
        assert!(w.hidden);

        let x = skill(&inv, "a/b~c");
        assert!(!x.off);
        // "user-invocable-only" keeps it out of the model's listing, like `disable-model-invocation`.
        assert!(x.hidden);
        assert!(!n.hidden);
        assert_eq!(
            x.switch,
            Ok(Switch::JsonSet {
                file: r.claude_settings.clone(),
                pointer: "/skillOverrides/a~1b~0c".into(),
                value: "\"off\"".into()
            })
        );

        let p = skill(&inv, "plug:skill");
        assert_eq!(p.origin, Origin::Plugin("plug@mk".into()));
        assert_eq!(p.switch, Err(NoSwitch::Plugin("plug@mk".into())));
        assert_eq!(p.listed_chars, "Plugin skill text".chars().count());

        let q = skill(&inv, "ps");
        assert_eq!(q.origin, Origin::Project(proj.clone()));
        assert_eq!(q.listed_chars, 2);
        assert_eq!(
            q.switch,
            Ok(Switch::JsonSet {
                file: proj.join(".claude").join("settings.local.json"),
                pointer: "/skillOverrides/ps".into(),
                value: "\"off\"".into()
            })
        );
        assert_eq!(inv.skills.len(), 5);
    }

    #[test]
    fn claude_transports() {
        let h = fake_home("transports");
        let r = roots(&h);
        put(
            &r.claude_json,
            &json!({"mcpServers": {
                "sse": {"type": "sse", "url": "u"},
                "http": {"type": "http", "command": "ignored", "url": "u"},
                "bare": {"url": "u"},
                "none": {},
                "std": {"command": "node"}
            }})
            .to_string(),
        );
        let inv = inventory(&r, &[], &|c: &str| {
            if c == "node" {
                CommandState::Missing
            } else {
                CommandState::Found
            }
        });
        assert_eq!(server(&inv, "sse").transport, Transport::Remote);
        assert_eq!(server(&inv, "http").transport, Transport::Remote);
        assert_eq!(server(&inv, "bare").transport, Transport::Remote);
        assert_eq!(server(&inv, "none").transport, Transport::Nothing);
        assert_eq!(
            server(&inv, "std").transport,
            Transport::Stdio {
                command: "node".into(),
                state: CommandState::Missing
            }
        );
    }

    #[test]
    fn claude_unreadable_json() {
        let h = fake_home("unreadable");
        let r = roots(&h);
        let proj = h.join("proj");
        put(&proj.join(".mcp.json"), "{");
        put(&r.claude_json, "  \n");
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(inv.servers.is_empty());
        assert_eq!(inv.unreadable.len(), 1);
        assert_eq!(inv.unreadable[0].0, proj.join(".mcp.json"));
        assert!(!inv.unreadable[0].1.is_empty());
    }

    #[test]
    fn claude_unreadable_files_are_described_in_one_sentence() {
        let h = fake_home("unreadable-message");
        let r = roots(&h);
        let (bad, binary) = (h.join("bad"), h.join("binary"));
        put(
            &bad.join(".mcp.json"),
            "{\n  \"mcpServers\": {\n    \"x\": \"sk-SECRET\" oops\n  }\n}\n",
        );
        std::fs::create_dir_all(&binary).unwrap();
        std::fs::write(binary.join(".mcp.json"), [0xff, 0xfe, 0x41]).unwrap();
        let inv = inventory(&r, &[bad.clone(), binary.clone()], &found);
        let message = |dir: &Path| {
            let file = dir.join(".mcp.json");
            inv.unreadable.iter().find(|(f, _)| *f == file).unwrap().1.clone()
        };
        assert_eq!(message(&bad), "It doesn't parse (line 4).");
        assert_eq!(message(&binary), "It isn't UTF-8 text.");
    }

    #[test]
    fn claude_mcpjson_approval() {
        let h = fake_home("approval");
        let r = roots(&h);
        let proj = h.join("proj");
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"a": {"command": "x"}, "b": {"command": "x"}, "c": {"command": "x"}, "d": {"command": "x"}}})
                .to_string(),
        );
        put(
            &proj.join(".claude").join("settings.local.json"),
            &json!({"enabledMcpjsonServers": ["a"]}).to_string(),
        );
        put(
            &proj.join(".claude").join("settings.json"),
            &json!({"enabledMcpjsonServers": ["b", "c"]}).to_string(),
        );
        let setup = |trust: bool, all: bool| {
            put(
                &r.claude_json,
                &json!({"projects": {proj.to_string_lossy().replace('\\', "/"): {"hasTrustDialogAccepted": trust}}})
                    .to_string(),
            );
            let mut user = json!({});
            if all {
                user = json!({"enableAllProjectMcpServers": true});
            }
            put(&r.claude_settings, &user.to_string());
        };

        // Found in the 7c2 final review: Claude Code starts no project server in a folder whose trust dialog wasn't
        // accepted, and a cloned repo can ship its own settings.local.json. So no approval counts without trust.
        setup(false, false);
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        for name in ["a", "b", "c", "d"] {
            assert_eq!(server(&inv, name).launch, Err(NotMeasured::NotApproved), "{name}");
        }
        setup(false, true);
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert_eq!(server(&inv, "d").launch, Err(NotMeasured::NotApproved));
        // Trusted: a is approved in settings.local.json, b and c in the committed file; d is not approved.
        setup(true, false);
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(server(&inv, "a").launch.is_ok());
        assert!(server(&inv, "c").launch.is_ok());
        assert_eq!(server(&inv, "d").launch, Err(NotMeasured::NotApproved));
        // d: enableAllProjectMcpServers in the user settings, in a trusted folder.
        setup(true, true);
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(server(&inv, "d").launch.is_ok());
    }

    #[test]
    fn claude_launch_and_cost() {
        let h = fake_home("launch");
        let r = roots(&h);
        let cache = h.join("plugcache");
        put(
            &r.claude_json,
            &json!({"mcpServers": {
                "usr": {"command": "u", "args": ["-y", "p"], "env": {"K": "v"}},
                "always": {"command": "u", "alwaysLoad": true},
                "remote": {"url": "https://example.test"}
            }})
            .to_string(),
        );
        put(
            &r.claude_settings,
            &json!({"enabledPlugins": {"plug@mk": true}, "env": {"ENABLE_TOOL_SEARCH": "auto:5", "MCP_TIMEOUT": "90000"}})
                .to_string(),
        );
        put(
            &r.claude_dir.join("plugins").join("installed_plugins.json"),
            &json!({"plugins": {"plug@mk": [{"installPath": cache.to_string_lossy()}]}}).to_string(),
        );
        put(
            &cache.join(".mcp.json"),
            &json!({"mcpServers": {"srv": {"command": "p"}}}).to_string(),
        );
        put(
            &r.claude_dir.join("skills").join("sk").join("SKILL.md"),
            "---\ndescription: d\n---\n",
        );

        let inv = inventory(&r, &[], &found);
        let p = server(&inv, "srv");
        let launch = p.launch.as_ref().unwrap();
        assert_eq!(
            launch.set,
            vec![("CLAUDE_PLUGIN_ROOT".to_string(), cache.display().to_string())]
        );
        assert_eq!(p.used_as, UsedAs::ToolPrefix("mcp__plugin_plug_srv__".into()));
        assert_eq!(launch.timeout_secs, 60);
        assert_eq!(launch.expand, Expand::Braces);
        assert!(matches!(p.cost.listing, Listing::FullUnder { percent: 5, .. }));
        assert_eq!(p.cost.always_load_meta, Some("anthropic/alwaysLoad"));
        let u = server(&inv, "usr").launch.as_ref().unwrap();
        assert_eq!(u.args, vec!["-y".to_string(), "p".to_string()]);
        assert_eq!(u.env, vec![("K".to_string(), "v".to_string())]);
        assert_eq!(u.cwd, Some(h.clone()));
        assert!(matches!(
            server(&inv, "always").cost.listing,
            Listing::Full {
                desc_cap: Some(4096),
                ..
            }
        ));
        assert_eq!(server(&inv, "remote").launch, Err(NotMeasured::Remote));
        assert_eq!(skill(&inv, "sk").used_as, vec![SkillUse::Name("sk".into())]);
        assert!(skill(&inv, "sk").created_ms.is_some());

        // Without the user setting: names by default, 30 s, and Marshell's own value counts.
        put(
            &r.claude_settings,
            &json!({"enabledPlugins": {"plug@mk": true}}).to_string(),
        );
        let inv = inventory(&r, &[], &found);
        assert_eq!(server(&inv, "srv").launch.as_ref().unwrap().timeout_secs, 30);
        assert!(matches!(
            server(&inv, "usr").cost.listing,
            Listing::Names {
                instructions_cap: Some(4096),
                ..
            }
        ));
        let mut r2 = roots(&h);
        r2.claude_tool_search = Some("false".into());
        let inv = inventory(&r2, &[], &found);
        assert!(matches!(server(&inv, "usr").cost.listing, Listing::Full { .. }));
    }
}
