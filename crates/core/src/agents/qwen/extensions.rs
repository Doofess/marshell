//! Qwen Code's MCP servers and skills for the Doctor: where it keeps them, how each is turned off, and what its
//! skill listing cuts. The readers take their roots as arguments and never read the environment.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::hash::hash_bytes;
use crate::config::jsonc::read_value;
use crate::doctor::ext::{
    Caps, CommandState, CostRule, Expand, ExtRoots, Hints, Inherit, Inventory, Launch, Listing, NoSwitch, NotMeasured,
    Origin, Server, Skill, SkillState, SkillUse, Switch, Transport, UsedAs, created_ms, sanitize_tool_part,
    skill_files, unreadable,
};
use crate::doctor::frontmatter::{self, Front, SkillMeta};

/// Qwen Code cuts every non-bundled description to its first line once the listing passes this many characters.
const LISTING_CUT: usize = 8000;

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

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// `*` in `pattern` matches any run of characters; without one the match is exact.
fn glob(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(i) => rest = &rest[i + middle.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

fn transport(entry: &Value, command: &dyn Fn(&str) -> CommandState) -> Transport {
    let text = |key: &str| entry.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    if text("httpUrl").is_some() || text("url").is_some() {
        Transport::Remote
    } else if let Some(c) = text("command") {
        Transport::Stdio {
            command: c.to_string(),
            state: command(c),
        }
    } else {
        Transport::Nothing
    }
}

/// An extension is skipped when `extension-enablement.json` carries a `!` (turn off) rule for it.
fn extension_disabled(enablement: &Option<Value>, name: &str) -> bool {
    enablement
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)))
        .map(|(_, v)| strings(v.get("overrides")).iter().any(|o| o.starts_with('!')))
        .unwrap_or(false)
}

struct Source<'a> {
    file: &'a Path,
    origin: Origin,
    /// Whether the user's `mcp.excluded` entry can be edited to turn these off.
    switchable: bool,
    /// How the CLI expands variables in these entries.
    expand: Expand,
    /// The entries need an approval in `mcpApprovals.json` (project `.mcp.json` and `.qwen/settings.json`).
    needs_approval: bool,
}

/// What the approval and cost rules read besides the entry.
struct Cx<'a> {
    /// `tools.toolSearch.enabled` isn't `false`.
    tool_search: bool,
    /// The parsed `mcpApprovals.json`.
    approvals: &'a Option<Value>,
    /// `security.folderTrust.enabled` is on and the project isn't trusted: no project server starts.
    untrusted: bool,
}

/// The canonical form Qwen Code hashes an entry by (`hashMcpServerConfig`, chunk-VABYHACO.js): the entry without
/// its top-level `scope`, `extensionName` and `description`, then `JSON.stringify` with a replacer that sorts the keys
/// of every nested object (arrays keep their order).
fn canonical(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let body: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", Value::String(k.clone()), canonical(&m[k])))
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonical).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

fn approval_hash(entry: &Value) -> String {
    let mut top = entry.as_object().cloned().unwrap_or_default();
    for dropped in ["scope", "extensionName", "description"] {
        top.remove(dropped);
    }
    hash_bytes(canonical(&Value::Object(top)).as_bytes())
}

/// What Qwen Code makes of a `.mcp.json` command entry before hashing it (`normalizeClaudeMcpServer`,
/// chunk-XBXDDFWT.js): its `type` is dropped unless that is `sdk`.
fn normalized_mcp_json(entry: &Value) -> Value {
    let mut entry = entry.clone();
    if let Some(m) = entry.as_object_mut() {
        let command_like = ["command", "httpUrl", "tcp"]
            .iter()
            .any(|k| m.get(*k).is_some_and(|v| !v.is_null() && v != false && v != ""));
        if command_like && m.get("type").is_some_and(|t| t != "sdk") {
            m.remove("type");
        }
    }
    entry
}

fn approved(cx: &Cx, project: &Path, name: &str, entry: &Value) -> bool {
    if cx.untrusted {
        return false;
    }
    let key = project.to_string_lossy().to_lowercase();
    let Some(record) = cx
        .approvals
        .as_ref()
        .and_then(|a| a.get(&key))
        .and_then(|p| p.get(name))
    else {
        return false;
    };
    record.get("status").and_then(Value::as_str) == Some("approved")
        && record.get("hash").and_then(Value::as_str) == Some(approval_hash(entry).as_str())
}

fn env_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn launch_of(name: &str, entry: &Value, remote: bool, at: &Source, cx: &Cx) -> Result<Launch, NotMeasured> {
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
    if at.needs_approval {
        let Origin::Project(project) = &at.origin else {
            return Err(NotMeasured::NotApproved);
        };
        let hashed = if at.file.file_name().is_some_and(|f| f == ".mcp.json") {
            normalized_mcp_json(entry)
        } else {
            entry.clone()
        };
        if !approved(cx, project, name, &hashed) {
            return Err(NotMeasured::NotApproved);
        }
    }
    Ok(Launch {
        command: command.to_string(),
        args: strings(entry.get("args")),
        env: env_pairs(entry.get("env")),
        inherit: Inherit::All,
        set: Vec::new(),
        cwd: entry.get("cwd").and_then(Value::as_str).map(PathBuf::from),
        expand: at.expand,
        timeout_secs: 30,
    })
}

fn cost_of(name: &str, entry: &Value, cx: &Cx) -> CostRule {
    let full = Listing::Full {
        prefix: format!("mcp__{}__", sanitize_tool_part(name)),
        desc_cap: None,
    };
    let always = entry.get("alwaysLoadTools").and_then(Value::as_bool) == Some(true);
    let listing = if cx.tool_search && !always {
        Listing::FirstLines {
            heading: name.to_string(),
            cap: 160,
        }
    } else {
        full
    };
    CostRule {
        listing,
        always_load_meta: None,
        only: entry.get("includeTools").map(|v| strings(Some(v))),
        except: strings(entry.get("excludeTools")),
    }
}

/// Whether `project` is trusted by `trustedFolders.json` (`{ "<folder>": "TRUST_FOLDER" | "TRUST_PARENT" |
/// "DO_NOT_TRUST" }`): the deepest rule whose folder holds the project wins, and on a tie a distrust wins. A project
/// no rule names isn't trusted.
fn folder_trusted(rules: &Option<Value>, project: &Path) -> bool {
    let norm = |p: &str| p.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let here = norm(&project.to_string_lossy());
    let mut best: Option<(usize, bool)> = None;
    for (path, level) in rules.as_ref().and_then(Value::as_object).into_iter().flatten() {
        let (root, trusted) = match level.as_str() {
            Some("TRUST_FOLDER") => (norm(path), true),
            Some("TRUST_PARENT") => (
                norm(
                    &Path::new(path)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ),
                true,
            ),
            Some("DO_NOT_TRUST") => (norm(path), false),
            _ => continue,
        };
        if root.is_empty() || !(here == root || here.starts_with(&format!("{root}/"))) {
            continue;
        }
        let depth = root.split('/').filter(|s| !s.is_empty()).count();
        best = match best {
            Some((d, t)) if d > depth || (d == depth && !t) => Some((d, t)),
            _ => Some((depth, trusted)),
        };
    }
    best.is_some_and(|(_, trusted)| trusted)
}

fn servers_in(
    map: Option<&Value>,
    at: Source,
    command: &dyn Fn(&str) -> CommandState,
    excluded: &[String],
    user_settings: &Path,
    cx: &Cx,
    out: &mut Vec<Server>,
) {
    let Some(map) = map.and_then(Value::as_object) else {
        return;
    };
    for (name, entry) in map {
        let transport = transport(entry, command);
        out.push(Server {
            name: name.clone(),
            file: at.file.to_path_buf(),
            origin: at.origin.clone(),
            launch: launch_of(name, entry, transport == Transport::Remote, &at, cx),
            cost: cost_of(name, entry, cx),
            used_as: UsedAs::ToolPrefix(format!("mcp__{}__", sanitize_tool_part(name))),
            transport,
            off: excluded.iter().any(|p| glob(p, name)),
            switch: match (&at.origin, at.switchable) {
                (Origin::Plugin(p), _) => Err(NoSwitch::Plugin(p.clone())),
                _ => Ok(Switch::JsonAdd {
                    file: user_settings.to_path_buf(),
                    pointer: "/mcp/excluded".to_string(),
                    name: name.clone(),
                }),
            },
        });
    }
}

/// Qwen Code's name rule: letters, digits, `_`, `:`, `.` and `-`.
fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'))
}

fn skills_in(
    dir: &Path,
    origin: &Origin,
    ext: Option<&str>,
    disabled: &[String],
    user_settings: &Path,
    out: &mut Vec<Skill>,
) {
    for (folder, file) in skill_files(dir) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let (front, meta) = match frontmatter::read(&text) {
            Front::Read(m) => (Some(true), m),
            Front::Unclear(m) => (Some(false), m),
            Front::NoBlock => (None, SkillMeta::default()),
        };
        let given = meta.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        let description = meta.description.as_deref().map(str::trim).filter(|d| !d.is_empty());
        let state = match front {
            None => SkillState::NoBlock,
            Some(false) => SkillState::Ok,
            Some(true) if given.is_none() => SkillState::Missing("name"),
            Some(true) if description.is_none() => SkillState::Missing("description"),
            Some(true) if !given.is_some_and(name_ok) => SkillState::BadName,
            Some(true) => SkillState::Ok,
        };
        let bare = given.unwrap_or(&folder).to_string();
        let name = match ext {
            Some(e) => format!("{e}:{bare}"),
            None => bare.clone(),
        };
        let off = disabled
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&bare) || (ext.is_some() && d.eq_ignore_ascii_case(&name)));
        let description = description.unwrap_or("");
        out.push(Skill {
            created_ms: created_ms(&file),
            used_as: vec![SkillUse::Name(name.clone())],
            switch: match origin {
                Origin::Plugin(p) => Err(NoSwitch::Plugin(p.clone())),
                _ => Ok(Switch::JsonAdd {
                    file: user_settings.to_path_buf(),
                    pointer: "/skills/disabled".to_string(),
                    name: bare,
                }),
            },
            name,
            file,
            origin: origin.clone(),
            state,
            listed_chars: description.chars().count(),
            first_line_chars: description.lines().next().unwrap_or("").chars().count(),
            name_only: false,
            off,
            hidden: false,
        });
    }
}

/// Qwen Code's switch adds a name to the person's global `mcp.excluded` or `skills.disabled`, which turns off every
/// server or skill with that name: skills match without regard to case, and an extension's `ext:name` matches by its
/// bare name. An entry that shares its name with another that is in use (not off, and not broken itself) gets no
/// switch, so turning it off can't turn off one that works.
fn report_only_when_name_is_shared(servers: &mut [Server], skills: &mut [Skill]) {
    let broken = |s: &Server| match &s.transport {
        Transport::Stdio { state, .. } => matches!(state, CommandState::Missing | CommandState::NotOnPath),
        Transport::Nothing => true,
        Transport::Remote => false,
    };
    let in_use: Vec<(String, bool)> = servers.iter().map(|s| (s.name.clone(), !s.off && !broken(s))).collect();
    for (i, s) in servers.iter_mut().enumerate() {
        let shared = in_use
            .iter()
            .enumerate()
            .any(|(j, (name, used))| j != i && *used && *name == s.name);
        if !s.off && shared && s.switch.is_ok() {
            s.switch = Err(NoSwitch::SameName);
        }
    }
    let bare = |name: &str| name.rsplit(':').next().unwrap_or(name).to_lowercase();
    let in_use: Vec<(String, bool)> = skills
        .iter()
        .map(|s| (bare(&s.name), !s.off && s.state == SkillState::Ok))
        .collect();
    for (i, s) in skills.iter_mut().enumerate() {
        let mine = bare(&s.name);
        let shared = in_use
            .iter()
            .enumerate()
            .any(|(j, (name, used))| j != i && *used && *name == mine);
        if !s.off && shared && s.switch.is_ok() {
            s.switch = Err(NoSwitch::SameName);
        }
    }
}

/// `command` judges a stdio server's command. `projects` are the folders to read project files in.
pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory {
    let mut files = Files::default();
    let mut servers = Vec::new();
    let mut skills = Vec::new();
    let user_file = roots.qwen_dir.join("settings.json");
    let user = files.json(&user_file);
    let user_excluded = strings(user.as_ref().and_then(|u| u.get("mcp")).and_then(|m| m.get("excluded")));
    let user_disabled = strings(
        user.as_ref()
            .and_then(|u| u.get("skills"))
            .and_then(|m| m.get("disabled")),
    );
    let setting = |path: &[&str]| path.iter().try_fold(user.as_ref()?, |v, k| v.get(*k));
    let approvals = files.json(&roots.qwen_dir.join("mcpApprovals.json"));
    let folder_trust = setting(&["security", "folderTrust", "enabled"]).and_then(Value::as_bool) == Some(true);
    let trusted_folders = if folder_trust {
        files.json(&roots.qwen_dir.join("trustedFolders.json"))
    } else {
        None
    };
    let mut cx = Cx {
        tool_search: setting(&["tools", "toolSearch", "enabled"]).and_then(Value::as_bool) != Some(false),
        approvals: &approvals,
        untrusted: false,
    };

    // User level.
    servers_in(
        user.as_ref().and_then(|u| u.get("mcpServers")),
        Source {
            file: &user_file,
            origin: Origin::User,
            switchable: true,
            expand: Expand::Dollar,
            needs_approval: false,
        },
        command,
        &user_excluded,
        &user_file,
        &cx,
        &mut servers,
    );
    for dir in [roots.qwen_dir.join("skills"), roots.home.join(".agents").join("skills")] {
        skills_in(&dir, &Origin::User, None, &user_disabled, &user_file, &mut skills);
    }

    // Extensions that aren't turned off.
    let extensions_dir = roots.qwen_dir.join("extensions");
    let enablement = files.json(&extensions_dir.join("extension-enablement.json"));
    let mut extensions: Vec<(String, PathBuf)> = std::fs::read_dir(&extensions_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
                .collect()
        })
        .unwrap_or_default();
    extensions.sort();
    for (name, dir) in extensions {
        if extension_disabled(&enablement, &name) {
            continue;
        }
        let manifest = dir.join("qwen-extension.json");
        let value = files.json(&manifest);
        servers_in(
            value.as_ref().and_then(|v| v.get("mcpServers")),
            Source {
                file: &manifest,
                origin: Origin::Plugin(name.clone()),
                switchable: false,
                expand: Expand::None,
                needs_approval: false,
            },
            command,
            &user_excluded,
            &user_file,
            &cx,
            &mut servers,
        );
        skills_in(
            &dir.join("skills"),
            &Origin::Plugin(name.clone()),
            Some(&name),
            &user_disabled,
            &user_file,
            &mut skills,
        );
    }

    // Projects.
    for project in projects {
        cx.untrusted = folder_trust && !folder_trusted(&trusted_folders, project);
        let own_file = project.join(".qwen").join("settings.json");
        let own = files.json(&own_file);
        let mut excluded = user_excluded.clone();
        excluded.extend(strings(
            own.as_ref().and_then(|u| u.get("mcp")).and_then(|m| m.get("excluded")),
        ));
        let mut disabled = user_disabled.clone();
        disabled.extend(strings(
            own.as_ref()
                .and_then(|u| u.get("skills"))
                .and_then(|m| m.get("disabled")),
        ));

        servers_in(
            own.as_ref().and_then(|v| v.get("mcpServers")),
            Source {
                file: &own_file,
                origin: Origin::Project(project.clone()),
                switchable: true,
                expand: Expand::Dollar,
                needs_approval: true,
            },
            command,
            &excluded,
            &user_file,
            &cx,
            &mut servers,
        );
        let mcp_file = project.join(".mcp.json");
        let mcp = files.json(&mcp_file);
        servers_in(
            mcp.as_ref().and_then(|v| v.get("mcpServers")),
            Source {
                file: &mcp_file,
                origin: Origin::Project(project.clone()),
                switchable: true,
                expand: Expand::None,
                needs_approval: true,
            },
            command,
            &excluded,
            &user_file,
            &cx,
            &mut servers,
        );
        for dir in [
            project.join(".qwen").join("skills"),
            project.join(".agents").join("skills"),
        ] {
            skills_in(
                &dir,
                &Origin::Project(project.clone()),
                None,
                &disabled,
                &user_file,
                &mut skills,
            );
        }
    }

    report_only_when_name_is_shared(&mut servers, &mut skills);

    Inventory {
        agent: "qwen",
        distro: None,
        found: true,
        caps: Caps {
            per_skill: None,
            listing_first_line_over: Some(LISTING_CUT),
        },
        servers,
        skills,
        unreadable: files.unreadable,
        hints: Hints {
            move_server: |_, project| {
                format!(
                    "To load it only there, move its entry to mcpServers in {}\\.qwen\\settings.json.",
                    project.display()
                )
            },
            move_skill: |_, project| {
                format!(
                    "To load it only there, move its folder to {}\\.qwen\\skills.",
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
            .join(format!("test-ext-qwen-{name}"));
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
    fn qwen_servers_and_globs() {
        let h = fake_home("servers");
        let r = roots(&h);
        let user = r.qwen_dir.join("settings.json");
        let proj = h.join("proj");
        put(
            &user,
            &json!({
                "mcpServers": {"github": {"command": "g"}, "gitlab": {"httpUrl": "u"}},
                "mcp": {"excluded": ["git*hub"]}
            })
            .to_string(),
        );
        put(
            &proj.join(".qwen").join("settings.json"),
            &json!({"mcpServers": {"psrv": {"command": "x"}}, "mcp": {"excluded": ["*-off"]}}).to_string(),
        );
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"mj": {"url": "u"}, "mj-off": {"url": "u"}}}).to_string(),
        );
        put(
            &r.qwen_dir.join("extensions").join("myext").join("qwen-extension.json"),
            &json!({"mcpServers": {"esrv": {"command": "e"}}}).to_string(),
        );
        put(
            &r.qwen_dir.join("extensions").join("offext").join("qwen-extension.json"),
            &json!({"mcpServers": {"hidden": {"command": "e"}}}).to_string(),
        );
        put(
            &r.qwen_dir.join("extensions").join("extension-enablement.json"),
            &json!({"offext": {"overrides": ["!/*"]}}).to_string(),
        );

        let inv = inventory(&r, std::slice::from_ref(&proj), &found);

        assert_eq!(inv.agent, "qwen");
        assert_eq!(inv.distro, None);
        assert!(inv.found);
        assert_eq!(inv.caps.listing_first_line_over, Some(8000));
        assert!(inv.unreadable.is_empty());

        let switch = |name: &str| {
            Ok(Switch::JsonAdd {
                file: user.clone(),
                pointer: "/mcp/excluded".into(),
                name: name.into(),
            })
        };
        let gh = server(&inv, "github");
        assert!(gh.off);
        assert_eq!(gh.origin, Origin::User);
        assert_eq!(
            gh.transport,
            Transport::Stdio {
                command: "g".into(),
                state: CommandState::Found
            }
        );
        let gl = server(&inv, "gitlab");
        assert!(!gl.off);
        assert_eq!(gl.transport, Transport::Remote);
        assert_eq!(gl.switch, switch("gitlab"));

        let ps = server(&inv, "psrv");
        assert_eq!(ps.origin, Origin::Project(proj.clone()));
        assert_eq!(ps.switch, switch("psrv"));
        assert!(!ps.off);

        let mj = server(&inv, "mj");
        assert_eq!(mj.origin, Origin::Project(proj.clone()));
        assert_eq!(mj.file, proj.join(".mcp.json"));
        assert_eq!(mj.transport, Transport::Remote);
        assert!(!mj.off);
        assert!(server(&inv, "mj-off").off);

        let es = server(&inv, "esrv");
        assert_eq!(es.origin, Origin::Plugin("myext".into()));
        assert_eq!(es.switch, Err(NoSwitch::Plugin("myext".into())));

        assert!(inv.servers.iter().all(|s| s.name != "hidden"));
    }

    #[test]
    fn qwen_skill_rules() {
        let h = fake_home("skills");
        let r = roots(&h);
        let skills = r.qwen_dir.join("skills");
        let user = r.qwen_dir.join("settings.json");
        let proj = h.join("proj");
        put(&skills.join("noname").join("SKILL.md"), "---\ndescription: d\n---\n");
        put(
            &skills.join("badname").join("SKILL.md"),
            "---\nname: \"a b\"\ndescription: d\n---\n",
        );
        put(
            &skills.join("jp").join("SKILL.md"),
            "---\nname: 日本:x\ndescription: d\n---\n",
        );
        put(&skills.join("nodesc").join("SKILL.md"), "---\nname: nodesc\n---\n");
        put(&skills.join("plain").join("SKILL.md"), "no frontmatter at all\n");
        put(
            &skills.join("fancy").join("SKILL.md"),
            "---\nname: fancy\ndescription: >\n  folded text\n  here\n---\n",
        );
        put(
            &h.join(".agents").join("skills").join("shared").join("SKILL.md"),
            "---\nname: shared\ndescription: abc\nfoo: [1\n---\n",
        );
        put(
            &r.qwen_dir
                .join("extensions")
                .join("myext")
                .join("skills")
                .join("x")
                .join("SKILL.md"),
            "---\nname: x\ndescription: d\n---\n",
        );
        put(
            &proj.join(".qwen").join("skills").join("ps").join("SKILL.md"),
            "---\nname: ps\ndescription: p\n---\n",
        );
        put(
            &proj.join(".qwen").join("settings.json"),
            &json!({"skills": {"disabled": ["PS"]}}).to_string(),
        );
        put(&user, &json!({"skills": {"disabled": ["X"]}}).to_string());

        let inv = inventory(&r, std::slice::from_ref(&proj), &found);

        assert_eq!(skill(&inv, "noname").state, SkillState::Missing("name"));
        assert_eq!(skill(&inv, "a b").state, SkillState::BadName);
        let jp = skill(&inv, "日本:x");
        assert_eq!(jp.state, SkillState::Ok);
        assert_eq!(jp.origin, Origin::User);
        assert_eq!(jp.listed_chars, 1);
        assert_eq!(skill(&inv, "nodesc").state, SkillState::Missing("description"));
        assert_eq!(skill(&inv, "plain").state, SkillState::NoBlock);
        assert_eq!(skill(&inv, "fancy").state, SkillState::Ok);
        let sh = skill(&inv, "shared");
        assert_eq!(sh.state, SkillState::Ok, "an unclear block never reads as broken");
        assert_eq!(
            sh.file,
            h.join(".agents").join("skills").join("shared").join("SKILL.md")
        );

        let x = skill(&inv, "myext:x");
        assert_eq!(x.origin, Origin::Plugin("myext".into()));
        assert!(x.off);
        assert_eq!(x.switch, Err(NoSwitch::Plugin("myext".into())));

        let ps = skill(&inv, "ps");
        assert_eq!(ps.origin, Origin::Project(proj));
        assert!(ps.off);
        assert_eq!(
            ps.switch,
            Ok(Switch::JsonAdd {
                file: user,
                pointer: "/skills/disabled".into(),
                name: "ps".into()
            })
        );
    }

    #[test]
    fn a_server_name_another_server_uses_is_report_only() {
        let h = fake_home("same-server");
        let r = roots(&h);
        let user = r.qwen_dir.join("settings.json");
        let proj = h.join("proj");
        put(
            &user,
            &json!({"mcpServers": {"github": {"command": "here"}}}).to_string(),
        );
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"github": {"command": "gone"}, "solo": {"command": "gone"}}}).to_string(),
        );
        let judge = |c: &str| {
            if c == "gone" {
                CommandState::NotOnPath
            } else {
                CommandState::Found
            }
        };

        let inv = inventory(&r, std::slice::from_ref(&proj), &judge);

        let project_github = inv
            .servers
            .iter()
            .find(|s| s.name == "github" && s.origin == Origin::Project(proj.clone()))
            .unwrap();
        assert_eq!(project_github.switch, Err(NoSwitch::SameName));
        let user_github = inv.servers.iter().find(|s| s.origin == Origin::User).unwrap();
        assert!(user_github.switch.is_ok(), "{user_github:?}");
        assert!(server(&inv, "solo").switch.is_ok());
    }

    #[test]
    fn a_skill_name_another_skill_uses_is_report_only() {
        let h = fake_home("same-skill");
        let r = roots(&h);
        let skills = r.qwen_dir.join("skills");
        put(&skills.join("one").join("SKILL.md"), "---\nname: dup\n---\n");
        put(
            &skills.join("two").join("SKILL.md"),
            "---\nname: Dup\ndescription: d\n---\n",
        );
        put(&skills.join("three").join("SKILL.md"), "---\nname: alone\n---\n");

        let inv = inventory(&r, &[], &found);

        assert_eq!(skill(&inv, "dup").state, SkillState::Missing("description"));
        assert_eq!(skill(&inv, "dup").switch, Err(NoSwitch::SameName));
        assert!(skill(&inv, "Dup").switch.is_ok());
        assert!(skill(&inv, "alone").switch.is_ok());
    }

    #[test]
    fn qwen_empty_settings_file() {
        let h = fake_home("empty");
        let r = roots(&h);
        put(&r.qwen_dir.join("settings.json"), "");
        let inv = inventory(&r, &[], &found);
        assert!(inv.servers.is_empty());
        assert!(inv.unreadable.is_empty());
    }

    #[test]
    fn qwen_unreadable_settings() {
        let h = fake_home("unreadable");
        let r = roots(&h);
        put(&r.qwen_dir.join("settings.json"), "{");
        let inv = inventory(&r, &[], &found);
        assert!(inv.servers.is_empty());
        assert_eq!(inv.unreadable.len(), 1);
        assert_eq!(inv.unreadable[0].0, r.qwen_dir.join("settings.json"));
    }

    #[test]
    fn qwen_unreadable_files_are_described_in_one_sentence() {
        let h = fake_home("unreadable-message");
        let r = roots(&h);
        let settings = r.qwen_dir.join("settings.json");
        put(
            &settings,
            "{\n  \"mcpServers\": {\n    \"x\": \"sk-SECRET\" oops\n  }\n}\n",
        );
        let manifest = r.qwen_dir.join("extensions").join("e").join("qwen-extension.json");
        std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        std::fs::write(&manifest, [0xff, 0xfe, 0x41]).unwrap();
        let inv = inventory(&r, &[], &found);
        let message = |file: &Path| inv.unreadable.iter().find(|(f, _)| f == file).unwrap().1.clone();
        assert_eq!(message(&settings), "It doesn't parse (line 4).");
        assert_eq!(message(&manifest), "It isn't UTF-8 text.");
    }

    /// SHA-256 of `{"args":["srv.js","--flag"],"command":"node","env":{"A":"1","B":"2"}}`, computed with Python.
    const HASH: &str = "d848d877eb27620aba3ea3b9d4e885dbb1a22a96ad43f07b249de5f8becf298e";

    fn approval_setup(name: &str, arg: &str, status: &str) -> (PathBuf, ExtRoots, PathBuf) {
        let h = fake_home(name);
        let r = roots(&h);
        let proj = h.join("proj");
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"p": {
                "command": "node", "env": {"B": "2", "A": "1"}, "description": "ignored", "args": ["srv.js", arg]
            }}})
            .to_string(),
        );
        put(
            &r.qwen_dir.join("mcpApprovals.json"),
            &json!({proj.to_string_lossy().to_lowercase(): {"p": {"hash": HASH, "status": status}}}).to_string(),
        );
        put(
            &r.qwen_dir.join("settings.json"),
            &json!({"mcpServers": {"u": {"command": "user-cmd"}}}).to_string(),
        );
        (h, r, proj)
    }

    #[test]
    fn qwen_approval_hash() {
        let (_h, r, proj) = approval_setup("approval-ok", "--flag", "approved");
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(server(&inv, "p").launch.is_ok());
        assert!(server(&inv, "u").launch.is_ok());

        let (_h, r, proj) = approval_setup("approval-edited", "--other", "approved");
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert_eq!(server(&inv, "p").launch, Err(NotMeasured::NotApproved));
        assert!(server(&inv, "u").launch.is_ok());

        let (_h, r, proj) = approval_setup("approval-rejected", "--flag", "rejected");
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert_eq!(server(&inv, "p").launch, Err(NotMeasured::NotApproved));
        assert!(server(&inv, "u").launch.is_ok());

        // folderTrust on, and no trustedFolders.json: no project server starts, the user's still does.
        let (_h, r, proj) = approval_setup("approval-trust", "--flag", "approved");
        put(
            &r.qwen_dir.join("settings.json"),
            &json!({"mcpServers": {"u": {"command": "user-cmd"}}, "security": {"folderTrust": {"enabled": true}}})
                .to_string(),
        );
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert_eq!(server(&inv, "p").launch, Err(NotMeasured::NotApproved));
        assert!(server(&inv, "u").launch.is_ok());
        put(
            &r.qwen_dir.join("trustedFolders.json"),
            &json!({proj.to_string_lossy(): "TRUST_FOLDER"}).to_string(),
        );
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(server(&inv, "p").launch.is_ok());
    }

    /// Qwen Code hashes a `.mcp.json` command entry after `normalizeClaudeMcpServer`, which drops its `type` unless
    /// that is `sdk`.
    #[test]
    fn qwen_mcp_json_type_is_dropped_before_hashing() {
        let (_h, r, proj) = approval_setup("approval-type", "--flag", "approved");
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"p": {
                "type": "stdio", "command": "node", "env": {"B": "2", "A": "1"}, "args": ["srv.js", "--flag"]
            }}})
            .to_string(),
        );
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(server(&inv, "p").launch.is_ok());

        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"p": {
                "type": "sdk", "command": "node", "env": {"B": "2", "A": "1"}, "args": ["srv.js", "--flag"]
            }}})
            .to_string(),
        );
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert_eq!(server(&inv, "p").launch, Err(NotMeasured::NotApproved));
    }

    #[test]
    fn qwen_launch_and_cost() {
        let h = fake_home("launch");
        let r = roots(&h);
        let proj = h.join("proj");
        put(
            &proj.join(".qwen").join("settings.json"),
            &json!({"mcpServers": {"s": {"command": "c", "cwd": "w", "includeTools": ["a"], "excludeTools": ["b"]}}})
                .to_string(),
        );
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"m": {"command": "c"}}}).to_string(),
        );
        put(
            &r.qwen_dir.join("settings.json"),
            &json!({"mcpServers": {"u": {"command": "c", "alwaysLoadTools": true}, "my srv": {"command": "c"}}})
                .to_string(),
        );
        // Approve both project servers by their hashes, whatever those are.
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        let s = server(&inv, "s");
        assert_eq!(s.launch, Err(NotMeasured::NotApproved));
        assert_eq!(s.cost.only, Some(vec!["a".to_string()]));
        assert_eq!(s.cost.except, vec!["b".to_string()]);
        assert_eq!(
            s.cost.listing,
            Listing::FirstLines {
                heading: "s".into(),
                cap: 160
            }
        );
        let key = proj.to_string_lossy().to_lowercase();
        let hash = |v: Value| approval_hash(&v);
        put(
            &r.qwen_dir.join("mcpApprovals.json"),
            &json!({key: {
                "s": {"status": "approved", "hash": hash(json!({"command": "c", "cwd": "w", "includeTools": ["a"], "excludeTools": ["b"]}))},
                "m": {"status": "approved", "hash": hash(json!({"command": "c"}))}
            }})
            .to_string(),
        );
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        let s = server(&inv, "s").launch.as_ref().unwrap();
        assert_eq!(s.expand, Expand::Dollar);
        assert_eq!(s.cwd, Some(PathBuf::from("w")));
        assert_eq!(s.timeout_secs, 30);
        assert_eq!(server(&inv, "m").launch.as_ref().unwrap().expand, Expand::None);
        assert!(matches!(
            server(&inv, "u").cost.listing,
            Listing::Full { desc_cap: None, .. }
        ));
        assert_eq!(
            server(&inv, "my srv").used_as,
            UsedAs::ToolPrefix("mcp__my_srv__".into())
        );
        assert_eq!(server(&inv, "s").cost.only, Some(vec!["a".to_string()]));

        put(
            &r.qwen_dir.join("settings.json"),
            &json!({"mcpServers": {"u": {"command": "c"}}, "tools": {"toolSearch": {"enabled": false}}}).to_string(),
        );
        let inv = inventory(&r, std::slice::from_ref(&proj), &found);
        assert!(matches!(server(&inv, "u").cost.listing, Listing::Full { prefix: ref p, .. } if p == "mcp__u__"));
    }
}
