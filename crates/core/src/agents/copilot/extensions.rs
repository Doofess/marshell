//! Copilot CLI's MCP servers, skills and plugins for the Doctor: where it keeps them, how each is turned off, which of
//! a project's it starts, and which skills it refuses or only names. The reader takes its roots as arguments and never
//! reads the environment.
//!
//! Copilot's `config.json` may hold account tokens. Only the keys asked for (`trustedFolders`, `installedPlugins`) are
//! taken out of it, the file's parsed value is dropped at once, and nothing of it is kept in a type that prints, put
//! in an item or logged. A plugin's folder comes from that file, so it is followed only when it is inside Copilot's
//! `installed-plugins` folder, and the paths in a plugin's manifest only when they stay inside the plugin's folder.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::jsonc::read_value;
use crate::config::preview::{MAX_CONFIG_BYTES, TOO_LARGE_DETAIL};
use crate::doctor::ext::{
    Caps, CommandState, CostRule, Expand, ExtRoots, Hints, Inherit, Inventory, Launch, Listing, NoSwitch, NotMeasured,
    Origin, Server, Skill, SkillState, SkillUse, Switch, Transport, UsedAs, created_ms, skill_files, unreadable,
};
use crate::doctor::frontmatter::{self, Front};

/// The GitHub server Copilot CLI loads itself, whoever is signed in.
pub const GITHUB_SERVER: &str = "github-mcp-server";

/// What a server's tools cost is counted in full, and Copilot may do better for some models.
const COST_NOTE: &str = "Marshell counts the tools' full definitions. Copilot CLI may load them on demand for some models, which costs less.";

/// How long Marshell waits for a server's tool list when the entry names no `timeout`.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// JSON files read once each; a file that exists and can't be read as JSON is remembered in `unreadable`. It holds
/// parsed files, so it has no `Debug`.
#[derive(Default)]
struct Files {
    cache: HashMap<PathBuf, Option<Value>>,
    unreadable: Vec<(PathBuf, String)>,
}

impl Files {
    /// The file's value, or None when it isn't there or can't be read (then it is in `unreadable`). Comments and
    /// trailing commas are fine.
    fn json(&mut self, path: &Path) -> Option<Value> {
        if let Some(v) = self.cache.get(path) {
            return v.clone();
        }
        let value = self.load(path);
        self.cache.insert(path.to_path_buf(), value.clone());
        value
    }

    fn load(&mut self, path: &Path) -> Option<Value> {
        if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_CONFIG_BYTES) {
            return self.fail(path, TOO_LARGE_DETAIL.to_string());
        }
        match std::fs::read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => match read_value(&text) {
                    Ok(v) => Some(v),
                    Err(_) => self.fail(path, unreadable::json(&text)),
                },
                Err(_) => self.fail(path, unreadable::NOT_UTF8.to_string()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => self.fail(path, unreadable::io(&e)),
        }
    }

    /// A file is reported once, however many times it is read (`config.json` is read for each key asked of it).
    fn fail(&mut self, path: &Path, message: String) -> Option<Value> {
        if !self.unreadable.iter().any(|(p, _)| p == path) {
            self.unreadable.push((path.to_path_buf(), message));
        }
        None
    }

    /// The text of a `SKILL.md`, or None when it isn't there, isn't UTF-8 text, can't be read, or is over the size
    /// limit (then it is in `unreadable`). Never more than the limit is read.
    fn skill_text(&mut self, path: &Path) -> Option<String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .ok()?
            .take(MAX_CONFIG_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            self.fail(path, TOO_LARGE_DETAIL.to_string());
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    /// `key` of Copilot's `config.json` and nothing else of it. The file is read each time and never kept.
    fn config_field(&mut self, config: &Path, key: &str) -> Option<Value> {
        self.load(config)?.get(key).cloned()
    }

    /// The names in the list at `pointer` of the settings file `file`. A file that is missing or has no such list gives
    /// an empty one; a file that doesn't read, isn't an object, or whose list isn't an array of strings gives Err, and
    /// the switch that would add to it is refused.
    fn disabled(&mut self, file: &Path, pointer: &str) -> Result<Vec<String>, ()> {
        let root = self.json(file);
        if self.unreadable.iter().any(|(p, _)| p == file) {
            return Err(());
        }
        let Some(root) = root else { return Ok(Vec::new()) };
        if !root.is_object() {
            return Err(());
        }
        match root.pointer(pointer) {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => items.iter().map(|v| v.as_str().map(str::to_string).ok_or(())).collect(),
            Some(_) => Err(()),
        }
    }
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// `folder`'s parts for comparing: either slash, no trailing separator, no case.
fn parts(path: &str) -> Vec<String> {
    let path = path.trim().replace('\\', "/").to_lowercase();
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    path.split('/').filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// Whether Copilot trusts `folder`: it is a listed folder or under one (`trustedFolders` in `config.json`),
/// compared part by part, without case and with either slash.
pub fn trusted(folder: &Path, trusted_folders: &[String]) -> bool {
    let here = parts(&folder.to_string_lossy());
    trusted_folders
        .iter()
        .map(|f| parts(f))
        .any(|listed| !listed.is_empty() && here.starts_with(&listed))
}

/// `trustedFolders` of the user's `config.json`: an array of strings, else nothing.
fn trusted_folders(files: &mut Files, config: &Path) -> Vec<String> {
    match files.config_field(config, "trustedFolders") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect::<Option<Vec<String>>>()
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Copilot skips a server whose name is empty or holds a control character.
fn name_ok(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(char::is_control)
}

fn transport(entry: &Value, command: &dyn Fn(&str) -> CommandState) -> Transport {
    let text = |key: &str| entry.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    if text("url").is_some() {
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

fn env_pairs(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// The entry's `timeout` (milliseconds) in whole seconds, rounded up.
fn timeout_secs(entry: &Value) -> u64 {
    let ms = entry.get("timeout");
    ms.and_then(Value::as_u64)
        .map(|ms| ms.div_ceil(1000))
        .or_else(|| {
            ms.and_then(Value::as_f64)
                .filter(|f| f.is_finite() && *f > 0.0)
                .map(|f| (f / 1000.0).ceil() as u64)
        })
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
}

fn launch_of(entry: &Value, transport: &Transport, trusted: bool) -> Result<Launch, NotMeasured> {
    match transport {
        Transport::Remote => Err(NotMeasured::Remote),
        // A server with no command has nothing to start.
        Transport::Nothing => Err(NotMeasured::NotApproved),
        Transport::Stdio { command, .. } if trusted => Ok(Launch {
            command: command.clone(),
            args: strings(entry.get("args")),
            env: env_pairs(entry.get("env")),
            inherit: Inherit::All,
            set: Vec::new(),
            cwd: entry.get("cwd").and_then(Value::as_str).map(PathBuf::from),
            expand: Expand::DollarDefault,
            timeout_secs: timeout_secs(entry),
        }),
        Transport::Stdio { .. } => Err(NotMeasured::NotApproved),
    }
}

fn cost_of(name: &str, entry: &Value) -> CostRule {
    // `tools` names the tools that load; `["*"]` is all of them.
    let only = entry
        .get("tools")
        .and_then(Value::as_array)
        .filter(|tools| !tools.iter().any(|t| t.as_str() == Some("*")))
        .map(|tools| strings(Some(&Value::Array(tools.clone()))));
    CostRule {
        listing: Listing::Full {
            prefix: format!("{name}-"),
            desc_cap: None,
        },
        always_load_meta: None,
        only,
        except: strings(entry.get("excludeTools")),
    }
}

/// The user's settings file and what it says about turning servers off.
struct Settings {
    file: PathBuf,
    /// `disabledMcpServers`, or Err when it can't be edited.
    disabled: Result<Vec<String>, ()>,
    /// `disabledSkills`, or Err when it can't be edited.
    disabled_skills: Result<Vec<String>, ()>,
}

impl Settings {
    fn switch(&self, name: &str) -> Result<Switch, NoSwitch> {
        match &self.disabled {
            Ok(_) => Ok(Switch::JsonAdd {
                file: self.file.clone(),
                pointer: DISABLED_SERVERS.to_string(),
                name: name.to_string(),
            }),
            Err(()) => Err(NoSwitch::CantEdit(self.file.clone())),
        }
    }

    fn off(&self, name: &str) -> bool {
        self.disabled.as_ref().is_ok_and(|l| l.iter().any(|n| n == name))
    }

    fn skill_switch(&self, name: &str) -> Result<Switch, NoSwitch> {
        match &self.disabled_skills {
            Ok(_) => Ok(Switch::JsonAdd {
                file: self.file.clone(),
                pointer: DISABLED_SKILLS.to_string(),
                name: name.to_string(),
            }),
            Err(()) => Err(NoSwitch::CantEdit(self.file.clone())),
        }
    }

    fn skill_off(&self, name: &str) -> bool {
        self.disabled_skills.as_ref().is_ok_and(|l| l.iter().any(|n| n == name))
    }
}

const DISABLED_SERVERS: &str = "/disabledMcpServers";
const DISABLED_SKILLS: &str = "/disabledSkills";

/// A skill's name is at most this many characters, and its description too.
const NAME_MAX: usize = 64;
const DESCRIPTION_MAX: usize = 1024;
/// Copilot describes skills in full up to this many characters of listing: the block's opening line is 19, and each
/// skill costs its name, its description and 91 of tags and line break (research note, finding 6).
const LISTING_BUDGET: usize = 15_000;
const LISTING_OPENING: usize = 19;
const ENTRY_OVERHEAD: usize = 91;

/// Pushes the servers of `map`. `extra_off` are names a project's own settings turn off.
#[allow(clippy::too_many_arguments)]
fn servers_in(
    map: Option<&Value>,
    file: &Path,
    origin: &Origin,
    trusted: bool,
    extra_off: &[String],
    settings: &Settings,
    command: &dyn Fn(&str) -> CommandState,
    out: &mut Vec<Server>,
) {
    let Some(map) = map.and_then(Value::as_object) else {
        return;
    };
    for (name, entry) in map.iter().filter(|(name, _)| name_ok(name)) {
        let transport = transport(entry, command);
        out.push(Server {
            name: name.clone(),
            file: file.to_path_buf(),
            origin: origin.clone(),
            launch: launch_of(entry, &transport, trusted),
            cost: cost_of(name, entry),
            used_as: UsedAs::Server(name.clone()),
            transport,
            off: settings.off(name) || extra_off.iter().any(|n| n == name),
            switch: settings.switch(name),
        });
    }
}

/// Copilot turns servers off by exact name, and a project's server with a user server's name replaces it. A server
/// that shares its name with another in use (not off, and not broken itself) gets no switch, so turning it off can't
/// turn off one that works.
fn report_only_when_name_is_shared(servers: &mut [Server]) {
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
}

/// The servers in a project's `.mcp.json`: under `mcpServers`, or the whole object when it has no such key.
fn project_map(root: &Value) -> &Value {
    root.get("mcpServers").unwrap_or(root)
}

/// What a project's own `.github/copilot/settings*.json` list under `key` (`disabledMcpServers`, `disabledSkills`).
fn project_off(files: &mut Files, project: &Path, key: &str) -> Vec<String> {
    let mut names = Vec::new();
    for name in ["settings.json", "settings.local.json"] {
        let file = project.join(".github").join("copilot").join(name);
        names.extend(strings(files.json(&file).as_ref().and_then(|v| v.get(key))));
    }
    names
}

/// Copilot refuses a skill whose name doesn't start with an ASCII letter or digit and go on with ASCII letters,
/// digits, `-`, `_`, `.`, `:` and spaces.
fn skill_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | ' '))
}

/// The length in characters of the first line of a skill's body that isn't blank, with its frontmatter left out.
fn first_body_line_chars(text: &str) -> usize {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return 0;
    }
    let Some(close) = lines.iter().skip(1).position(|l| l.trim() == "---").map(|p| p + 1) else {
        return 0;
    };
    lines[close + 1..]
        .iter()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map_or(0, |l| l.chars().count())
}

/// Pushes the skills in `found` (folder name, `SKILL.md`). A plugin's skill is named `<plugin>:<skill>` and has no
/// switch. `extra_off` are names a project's own settings turn off.
fn skills_in(
    files: &mut Files,
    found: Vec<(String, PathBuf)>,
    origin: &Origin,
    extra_off: &[String],
    settings: &Settings,
    out: &mut Vec<Skill>,
) {
    for (folder, file) in found {
        let Some(text) = files.skill_text(&file) else {
            continue;
        };
        let (closed, meta) = match frontmatter::read(&text) {
            Front::Read(m) => (Some(true), m),
            Front::Unclear(m) => (Some(false), m),
            Front::NoBlock => (None, Default::default()),
        };
        let given = meta.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        let bare = given.unwrap_or(&folder).to_string();
        let description = meta.description.as_deref().map(str::trim).filter(|d| !d.is_empty());
        let state = match closed {
            None => SkillState::NoBlock,
            // Some line wasn't understood: nothing may be called wrong in it.
            Some(false) => SkillState::Ok,
            Some(true) if bare.chars().count() > NAME_MAX => SkillState::TooLong("name", NAME_MAX),
            Some(true) if !skill_name_ok(&bare) => SkillState::BadName,
            Some(true) if description.is_some_and(|d| d.chars().count() > DESCRIPTION_MAX) => {
                SkillState::TooLong("description", DESCRIPTION_MAX)
            }
            Some(true) => SkillState::Ok,
        };
        let name = match origin {
            Origin::Plugin(plugin) => format!("{plugin}:{bare}"),
            _ => bare,
        };
        out.push(Skill {
            created_ms: created_ms(&file),
            used_as: vec![SkillUse::File(file.clone()), SkillUse::Name(name.clone())],
            switch: match origin {
                Origin::Plugin(plugin) => Err(NoSwitch::Plugin(plugin.clone())),
                _ => settings.skill_switch(&name),
            },
            off: settings.skill_off(&name) || extra_off.contains(&name),
            listed_chars: description.map_or_else(|| first_body_line_chars(&text), |d| d.chars().count()),
            first_line_chars: description
                .and_then(|d| d.lines().next())
                .map_or(0, |l| l.chars().count()),
            name_only: false,
            hidden: meta.hidden,
            name,
            file,
            origin: origin.clone(),
            state,
        });
    }
}

/// The skills Copilot skips or can't switch apart, and the ones its listing only names.
fn settle_skills(skills: &mut [Skill]) {
    // A project's skill hides a personal skill of the same name from Copilot.
    let project_names: HashSet<String> = skills
        .iter()
        .filter(|s| matches!(s.origin, Origin::Project(_)) && s.state == SkillState::Ok)
        .map(|s| s.name.clone())
        .collect();
    for s in skills
        .iter_mut()
        .filter(|s| s.origin == Origin::User && project_names.contains(&s.name))
    {
        s.off = true;
    }

    // Copilot turns skills off by exact name, so a skill that shares its name with another in use (not off, and not
    // broken itself) gets no switch: turning it off would turn off one that works.
    let in_use: Vec<(String, bool)> = skills
        .iter()
        .map(|s| (s.name.clone(), !s.off && s.state == SkillState::Ok))
        .collect();
    for (i, s) in skills.iter_mut().enumerate() {
        let shared = in_use
            .iter()
            .enumerate()
            .any(|(j, (name, used))| j != i && *used && *name == s.name);
        if !s.off && shared && s.switch.is_ok() {
            s.switch = Err(NoSwitch::SameName);
        }
    }

    // The listing: project skills by name, then personal ones, then plugins'. Every skill from the first that would
    // take the listing past its budget on is only named.
    let rank = |s: &Skill| match s.origin {
        Origin::Project(_) => 0,
        Origin::User => 1,
        _ => 2,
    };
    let mut order: Vec<usize> = (0..skills.len())
        .filter(|&i| !skills[i].off && !skills[i].hidden && skills[i].state == SkillState::Ok)
        .collect();
    order.sort_by(|&a, &b| {
        (rank(&skills[a]), &skills[a].name, &skills[a].file).cmp(&(rank(&skills[b]), &skills[b].name, &skills[b].file))
    });
    let mut total = LISTING_OPENING;
    let mut over = false;
    for i in order {
        let s = &mut skills[i];
        let cost = s.name.chars().count() + s.listed_chars + ENTRY_OVERHEAD;
        if over || total + cost > LISTING_BUDGET {
            over = true;
            s.name_only = true;
        } else {
            total += cost;
        }
    }
}

/// `path` as its parts, with `.` and `..` resolved by the text alone and either slash taken, or None when the path
/// isn't absolute or `..` would climb out of its drive or share.
fn absolute_parts(path: &str) -> Option<Vec<String>> {
    let text = path.replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    let drive = text.len() >= 2 && text.as_bytes()[0].is_ascii_alphabetic() && text.as_bytes()[1] == b':';
    if !drive && !text.starts_with('/') {
        return None;
    }
    // What `..` can't climb above: a drive, or a share's server and name.
    let floor = if drive {
        1
    } else if text.starts_with("//") {
        2
    } else {
        0
    };
    let mut parts: Vec<String> = Vec::new();
    for part in text.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if part == ".." {
            if parts.len() <= floor {
                return None;
            }
            parts.pop();
        } else {
            parts.push(part.to_string());
        }
    }
    Some(parts)
}

/// One part of a path that can be joined without leaving the folder it is joined to.
fn safe_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && !part.chars().any(|c| matches!(c, '/' | '\\' | ':') || c.is_control())
}

fn same_part(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// A plugin's folder from its `cache_path`: the path, with `..` and either slash resolved and the case ignored on
/// Windows, must be inside `installed` (never the folder itself, and never by string prefix alone). The folder comes
/// back built from `installed` and the parts below it.
fn folder_inside(installed: &Path, given: &str) -> Option<PathBuf> {
    let root = absolute_parts(&installed.to_string_lossy())?;
    let parts = absolute_parts(given)?;
    if parts.len() <= root.len() || !root.iter().zip(&parts).all(|(a, b)| same_part(a, b)) {
        return None;
    }
    let below = &parts[root.len()..];
    below
        .iter()
        .all(|p| safe_part(p))
        .then(|| below.iter().fold(installed.to_path_buf(), |dir, p| dir.join(p)))
}

/// A path from a plugin's manifest: relative, and inside the plugin's `folder` once `..` is resolved. The plugin's
/// folder itself (`.`) counts.
fn path_in(folder: &Path, relative: &str) -> Option<PathBuf> {
    let text = relative.replace('\\', "/");
    if text.starts_with('/') || text.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in text.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if part == ".." {
            parts.pop()?;
        } else if safe_part(part) {
            parts.push(part);
        } else {
            return None;
        }
    }
    Some(parts.iter().fold(folder.to_path_buf(), |dir, p| dir.join(p)))
}

/// The manifests Copilot looks for in a plugin's folder, in its order.
const MANIFESTS: [&str; 4] = [
    "plugin.json",
    ".plugin/plugin.json",
    ".github/plugin/plugin.json",
    ".claude-plugin/plugin.json",
];

/// The servers and skills of the plugin `name` in `folder`.
fn read_plugin(
    files: &mut Files,
    folder: &Path,
    name: &str,
    settings: &Settings,
    command: &dyn Fn(&str) -> CommandState,
    servers: &mut Vec<Server>,
    skills: &mut Vec<Skill>,
) {
    let origin = Origin::Plugin(name.to_string());
    let manifest_file = MANIFESTS.iter().map(|m| folder.join(m)).find(|f| f.is_file());
    let manifest = manifest_file.as_ref().and_then(|f| files.json(f));
    let first = servers.len();
    let mut read: Vec<PathBuf> = Vec::new();

    if let (Some(manifest), Some(file)) = (&manifest, &manifest_file) {
        match manifest.get("mcpServers") {
            Some(object @ Value::Object(_)) => {
                servers_in(Some(object), file, &origin, false, &[], settings, command, servers)
            }
            Some(Value::String(relative)) => {
                if let Some(path) = path_in(folder, relative) {
                    let value = files.json(&path);
                    servers_in(
                        value.as_ref().map(project_map),
                        &path,
                        &origin,
                        false,
                        &[],
                        settings,
                        command,
                        servers,
                    );
                    read.push(path);
                }
            }
            _ => {}
        }
    }
    let mcp = folder.join(".mcp.json");
    if !read.contains(&mcp) {
        let value = files.json(&mcp);
        servers_in(
            value.as_ref().map(project_map),
            &mcp,
            &origin,
            false,
            &[],
            settings,
            command,
            servers,
        );
    }
    for s in &mut servers[first..] {
        s.switch = Err(NoSwitch::Plugin(name.to_string()));
    }

    let skills_dir = match manifest.as_ref().and_then(|m| m.get("skills")) {
        Some(Value::String(relative)) => path_in(folder, relative),
        _ => Some(folder.join("skills")),
    };
    if let Some(dir) = skills_dir {
        skills_in(files, skill_files(&dir), &origin, &[], settings, skills);
    }
}

/// The plugins in `installedPlugins` of the user's `config.json` that are enabled and not turned off in the user's
/// settings, each read from its folder.
fn read_plugins(
    files: &mut Files,
    roots: &ExtRoots,
    settings: &Settings,
    command: &dyn Fn(&str) -> CommandState,
    servers: &mut Vec<Server>,
    skills: &mut Vec<Skill>,
) {
    let Some(Value::Array(entries)) = files.config_field(&roots.copilot_dir.join("config.json"), "installedPlugins")
    else {
        return;
    };
    let installed = roots.copilot_dir.join("installed-plugins");
    let user_settings = files.json(&settings.file);
    let mut seen = HashSet::new();
    for entry in &entries {
        let text = |key: &str| entry.get(key).and_then(Value::as_str);
        let (Some(name), Some(marketplace)) = (text("name"), text("marketplace")) else {
            continue;
        };
        if entry.get("enabled") != Some(&Value::Bool(true)) || !safe_part(name) || !safe_part(marketplace) {
            continue;
        }
        let key = format!("{name}@{marketplace}");
        let turned_off = user_settings
            .as_ref()
            .and_then(|s| s.get("enabledPlugins")?.get(&key))
            .is_some_and(|v| *v == Value::Bool(false));
        let folder = match entry.get("cache_path") {
            None | Some(Value::Null) => Some(installed.join(marketplace).join(name)),
            Some(Value::String(path)) => folder_inside(&installed, path),
            Some(_) => None,
        };
        if let (false, Some(folder)) = (turned_off, folder)
            && seen.insert(key)
        {
            read_plugin(files, &folder, name, settings, command, servers, skills);
        }
    }
}

/// `command` judges a stdio server's command. `projects` are the folders to read project files in. The servers come
/// from `mcp-config.json`, each project's `.mcp.json` and the installed plugins, plus the GitHub server Copilot loads
/// itself. The skills come from Copilot's and the user's folders, each project's and the plugins'.
pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory {
    let mut files = Files::default();
    let mut servers = Vec::new();
    let mut skills = Vec::new();
    let settings_file = roots.copilot_dir.join("settings.json");
    let settings = Settings {
        disabled: files.disabled(&settings_file, DISABLED_SERVERS),
        disabled_skills: files.disabled(&settings_file, DISABLED_SKILLS),
        file: settings_file,
    };

    // User level.
    let user_file = roots.copilot_dir.join("mcp-config.json");
    let user = files.json(&user_file);
    servers_in(
        user.as_ref().and_then(|u| u.get("mcpServers")),
        &user_file,
        &Origin::User,
        true,
        &[],
        &settings,
        command,
        &mut servers,
    );

    let mut user_skills = vec![roots.copilot_dir.join("skills")];
    if !roots.copilot_home_set {
        user_skills.push(roots.home.join(".agents").join("skills"));
    }
    for dir in user_skills {
        skills_in(
            &mut files,
            skill_files(&dir),
            &Origin::User,
            &[],
            &settings,
            &mut skills,
        );
    }

    // Installed plugins.
    read_plugins(&mut files, roots, &settings, command, &mut servers, &mut skills);

    // Projects. `trustedFolders` is read once, when a project has a server to start.
    let mut trusted_list: Option<Vec<String>> = None;
    for project in projects {
        let found: Vec<(String, PathBuf)> = [".github", ".agents", ".claude"]
            .iter()
            .flat_map(|dir| skill_files(&project.join(dir).join("skills")))
            .collect();
        if !found.is_empty() {
            let off = project_off(&mut files, project, "disabledSkills");
            skills_in(
                &mut files,
                found,
                &Origin::Project(project.clone()),
                &off,
                &settings,
                &mut skills,
            );
        }

        let mcp_file = project.join(".mcp.json");
        let mcp = files.json(&mcp_file);
        let map = mcp.as_ref().map(project_map);
        if !map.and_then(Value::as_object).is_some_and(|m| !m.is_empty()) {
            continue;
        }
        let extra_off = project_off(&mut files, project, "disabledMcpServers");
        let list =
            trusted_list.get_or_insert_with(|| trusted_folders(&mut files, &roots.copilot_dir.join("config.json")));
        servers_in(
            map,
            &mcp_file,
            &Origin::Project(project.clone()),
            trusted(project, list),
            &extra_off,
            &settings,
            command,
            &mut servers,
        );
    }

    // The GitHub server comes with Copilot, unless the person's own config names a server of that name.
    if !roots.copilot_offline && !servers.iter().any(|s| s.name == GITHUB_SERVER) {
        servers.push(Server {
            name: GITHUB_SERVER.to_string(),
            file: settings.file.clone(),
            origin: Origin::BuiltIn,
            transport: Transport::Remote,
            off: settings.off(GITHUB_SERVER),
            switch: settings.switch(GITHUB_SERVER),
            launch: Err(NotMeasured::Remote),
            cost: cost_of(GITHUB_SERVER, &Value::Null),
            used_as: UsedAs::Server(GITHUB_SERVER.to_string()),
        });
    }

    report_only_when_name_is_shared(&mut servers);
    settle_skills(&mut skills);

    Inventory {
        agent: "copilot",
        distro: None,
        found: true,
        // Copilot cuts no description. It names the skills past its listing budget.
        caps: Caps {
            per_skill: None,
            listing_first_line_over: None,
            listing_named_over: Some(LISTING_BUDGET),
        },
        servers,
        skills,
        unreadable: files.unreadable,
        hints: Hints {
            move_server: |_, project| {
                format!(
                    "To load it only there, move its entry to mcpServers in {}\\.mcp.json.",
                    project.display()
                )
            },
            move_skill: |_, project| {
                format!(
                    "To load it only there, move its folder to {}\\.github\\skills.",
                    project.display()
                )
            },
        },
        cost_note: Some(COST_NOTE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::preview::{MAX_CONFIG_BYTES, TOO_LARGE_DETAIL};
    use crate::doctor::ext::{
        CostRule, Expand, Inherit, Launch, Listing, NoSwitch, NotMeasured, Origin, Server, Switch, Transport, UsedAs,
        unreadable,
    };

    const COST_NOTE: &str = "Marshell counts the tools' full definitions. Copilot CLI may load them on demand for some models, which costs less.";

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
            copilot_dir: h.join(".copilot"),
            copilot_home_set: false,
            copilot_offline: false,
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
            .unwrap_or_else(|| panic!("no server {name}: {:?}", inv.servers))
    }

    fn in_project<'a>(inv: &'a Inventory, name: &str, project: &Path) -> &'a Server {
        inv.servers
            .iter()
            .find(|s| s.name == name && s.origin == Origin::Project(project.to_path_buf()))
            .unwrap_or_else(|| panic!("no server {name} in {}", project.display()))
    }

    fn switch(r: &ExtRoots, name: &str) -> Result<Switch, NoSwitch> {
        Ok(Switch::JsonAdd {
            file: r.copilot_dir.join("settings.json"),
            pointer: "/disabledMcpServers".into(),
            name: name.into(),
        })
    }

    fn user_servers(r: &ExtRoots, text: &str) {
        put(&r.copilot_dir.join("mcp-config.json"), text);
    }

    #[test]
    fn a_user_server_in_a_file_with_comments_and_trailing_commas() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        user_servers(
            &r,
            "// my servers\n{\n  \"mcpServers\": {\n    \"echo\": { \"command\": \"node\", \"args\": [\"a.js\", \"--x\"],\n      \"env\": {\"K\": \"v\", \"N\": 3}, \"cwd\": \"C:/w\", },\n  }, // done\n}\n",
        );

        let inv = inventory(&r, &[], &found);

        assert_eq!(inv.agent, "copilot");
        assert_eq!(inv.distro, None);
        assert!(inv.found && inv.skills.is_empty() && inv.unreadable.is_empty());
        assert_eq!(inv.cost_note, Some(COST_NOTE));
        let s = server(&inv, "echo");
        assert_eq!(s.origin, Origin::User);
        assert_eq!(s.file, r.copilot_dir.join("mcp-config.json"));
        assert_eq!(
            s.transport,
            Transport::Stdio {
                command: "node".into(),
                state: CommandState::Found
            }
        );
        assert!(!s.off);
        assert_eq!(s.switch, switch(&r, "echo"));
        assert_eq!(s.used_as, UsedAs::Server("echo".into()));
        assert_eq!(
            s.launch,
            Ok(Launch {
                command: "node".into(),
                args: vec!["a.js".into(), "--x".into()],
                env: vec![("K".into(), "v".into())],
                inherit: Inherit::All,
                set: vec![],
                cwd: Some(PathBuf::from("C:/w")),
                expand: Expand::DollarDefault,
                timeout_secs: 30,
            })
        );
    }

    #[test]
    fn a_servers_own_timeout_is_rounded_up_to_seconds() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        user_servers(
            &r,
            r#"{"mcpServers": {"a": {"command": "c", "timeout": 1500}, "b": {"command": "c", "timeout": 60000},
                "c": {"command": "c", "timeout": 0}, "d": {"command": "c", "timeout": "soon"}}}"#,
        );
        let inv = inventory(&r, &[], &found);
        let secs = |n: &str| server(&inv, n).launch.as_ref().unwrap().timeout_secs;
        assert_eq!((secs("a"), secs("b"), secs("c"), secs("d")), (2, 60, 30, 30));
    }

    #[test]
    fn a_url_is_remote_and_an_entry_with_neither_is_nothing_and_odd_names_are_skipped() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        user_servers(
            &r,
            r#"{"mcpServers": {
                "web": {"url": "https://example.com/mcp"},
                "bare": {"args": ["x"]},
                "blank": {"command": "  "},
                "text": "node",
                "": {"command": "c"},
                "bad\u0007name": {"command": "c"},
                "tab\tname": {"command": "c"}
            }}"#,
        );
        let inv = inventory(&r, &[], &found);

        let web = server(&inv, "web");
        assert_eq!(web.transport, Transport::Remote);
        assert_eq!(web.launch, Err(NotMeasured::Remote));
        for nothing in ["bare", "blank", "text"] {
            assert_eq!(server(&inv, nothing).transport, Transport::Nothing, "{nothing}");
        }
        let mut names: Vec<&str> = inv.servers.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["bare", "blank", "github-mcp-server", "text", "web"]);
    }

    #[test]
    fn a_name_is_off_by_exact_match_and_a_project_server_also_by_its_projects_own_settings() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let (p1, p2) = (h.path().join("p1"), h.path().join("p2"));
        put(
            &r.copilot_dir.join("settings.json"),
            "{\n  // mine\n  \"disabledMcpServers\": [\"echo\", \"uonly\",],\n}\n",
        );
        user_servers(
            &r,
            r#"{"mcpServers": {"echo": {"command": "c"}, "Echo": {"command": "c"}, "uonly": {"command": "c"}, "plain": {"command": "c"}}}"#,
        );
        for p in [&p1, &p2] {
            put(
                &p.join(".mcp.json"),
                r#"{"mcpServers": {"a": {"command": "c"}, "b": {"command": "c"}, "c": {"command": "c"}, "d": {"command": "c"}}}"#,
            );
        }
        put(
            &p1.join(".github").join("copilot").join("settings.json"),
            r#"{"disabledMcpServers": ["a"]}"#,
        );
        put(
            &p1.join(".github").join("copilot").join("settings.local.json"),
            r#"{"disabledMcpServers": ["b", 7]}"#,
        );
        put(
            &p1.join(".github").join("copilot").join("other.json"),
            r#"{"disabledMcpServers": ["c"]}"#,
        );

        let inv = inventory(&r, &[p1.clone(), p2.clone()], &found);

        assert!(server(&inv, "echo").off);
        assert!(server(&inv, "uonly").off);
        assert!(!server(&inv, "Echo").off, "the match is exact");
        assert!(!server(&inv, "plain").off);
        assert!(in_project(&inv, "a", &p1).off);
        assert!(in_project(&inv, "b", &p1).off);
        assert!(!in_project(&inv, "c", &p1).off);
        assert!(
            !in_project(&inv, "a", &p2).off,
            "another project's settings don't count"
        );
    }

    #[test]
    fn a_user_list_turns_off_a_project_server_too() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let p = h.path().join("p");
        put(&r.copilot_dir.join("settings.json"), r#"{"disabledMcpServers": ["a"]}"#);
        put(&p.join(".mcp.json"), r#"{"mcpServers": {"a": {"command": "c"}}}"#);
        let inv = inventory(&r, std::slice::from_ref(&p), &found);
        assert!(in_project(&inv, "a", &p).off);
    }

    #[test]
    fn a_projects_mcp_json_with_or_without_the_wrapper_starts_only_in_a_trusted_folder() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let (wrapped, bare, other) = (h.path().join("Wrapped"), h.path().join("bare"), h.path().join("other"));
        put(&wrapped.join(".mcp.json"), r#"{"mcpServers": {"w": {"command": "c"}}}"#);
        put(
            &bare.join(".mcp.json"),
            "// no wrapper\n{\"b\": {\"command\": \"c\", \"args\": [\"1\"]},}\n",
        );
        put(&other.join(".mcp.json"), r#"{"mcpServers": {"o": {"command": "c"}}}"#);
        // The github folder's file is never read.
        put(
            &wrapped.join(".github").join("mcp.json"),
            r#"{"mcpServers": {"gh": {"command": "c"}}}"#,
        );
        put(
            &r.copilot_dir.join("config.json"),
            &format!(
                "// User settings\n// more\n{{\n  \"trustedFolders\": [{:?}, {:?}],\n  \"other\": 1\n}}\n",
                wrapped.to_string_lossy().to_uppercase(),
                format!("{}/", bare.to_string_lossy().replace('\\', "/")),
            ),
        );
        let projects = [wrapped.clone(), bare.clone(), other.clone()];

        let inv = inventory(&r, &projects, &found);

        for (name, project) in [("w", &wrapped), ("b", &bare), ("o", &other)] {
            let s = in_project(&inv, name, project);
            assert_eq!(s.file, project.join(".mcp.json"), "{name}");
            assert_eq!(s.switch, switch(&r, name), "{name}");
            assert_eq!(
                s.transport,
                Transport::Stdio {
                    command: "c".into(),
                    state: CommandState::Found
                }
            );
        }
        assert!(in_project(&inv, "w", &wrapped).launch.is_ok());
        assert_eq!(in_project(&inv, "b", &bare).launch.as_ref().unwrap().args, ["1"]);
        assert_eq!(in_project(&inv, "o", &other).launch, Err(NotMeasured::NotApproved));
        assert!(inv.servers.iter().all(|s| s.name != "gh"));
        assert!(inv.unreadable.is_empty());

        // A trusted parent folder covers the project under it.
        put(
            &r.copilot_dir.join("config.json"),
            &format!("{{\"trustedFolders\": [{:?}]}}", h.path().to_string_lossy()),
        );
        let inv = inventory(&r, &projects, &found);
        assert!(in_project(&inv, "o", &other).launch.is_ok());
    }

    #[test]
    fn a_folder_is_trusted_when_it_is_or_is_under_a_listed_one() {
        let list = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let trusts = |folder: &str, l: &[&str]| trusted(Path::new(folder), &list(l));
        assert!(trusts(r"c:\dev\proj\sub", &["C:/Dev/Proj/"]));
        assert!(trusts(r"C:\Dev\Proj", &["c:/dev/proj"]));
        assert!(trusts("c:/dev/proj/sub/deeper", &[r"c:\dev\proj\"]));
        assert!(trusts("c:/dev/proj", &["c:/nope", "c:/dev"]));
        assert!(!trusts(r"c:\dev\proj", &["c:/dev/pro"]));
        assert!(!trusts("c:/dev", &["c:/dev/proj"]));
        assert!(!trusts("c:/dev/proj", &[]));
        assert!(!trusts("c:/dev/proj", &["", "/", "  "]));
    }

    #[test]
    fn a_config_json_that_is_odd_trusts_nothing() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let project = h.path().join("proj");
        put(&project.join(".mcp.json"), r#"{"mcpServers": {"p": {"command": "c"}}}"#);
        let path = project.to_string_lossy().into_owned();
        let config = r.copilot_dir.join("config.json");
        let launch = |r: &ExtRoots| {
            inventory(r, std::slice::from_ref(&project), &found)
                .servers
                .iter()
                .find(|s| s.name == "p")
                .unwrap()
                .launch
                .clone()
        };
        for text in [
            format!("{{\"trustedFolders\": {path:?}}}"),
            format!("{{\"trustedFolders\": [{path:?}, 5]}}"),
            format!("{{\"trustedFolders\": {{\"a\": {path:?}}}}}"),
            "{\"trustedFolders\": null}".to_string(),
            "[1, 2]".to_string(),
            "{}".to_string(),
            format!("{{\"trustedFolders\": [{path:?}"),
        ] {
            put(&config, &text);
            assert_eq!(launch(&r), Err(NotMeasured::NotApproved), "{text}");
        }
        // No config.json at all.
        std::fs::remove_file(&config).unwrap();
        assert_eq!(launch(&r), Err(NotMeasured::NotApproved));
        put(&config, &format!("{{\"trustedFolders\": [{path:?}]}}"));
        assert!(launch(&r).is_ok());
    }

    #[test]
    fn a_config_json_that_doesnt_parse_is_reported_without_quoting_it() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let project = h.path().join("proj");
        put(&project.join(".mcp.json"), r#"{"mcpServers": {"p": {"command": "c"}}}"#);
        let config = r.copilot_dir.join("config.json");
        put(&config, "{\n  \"token\": \"SECRET-6N3\" oops\n}\n");

        let inv = inventory(&r, std::slice::from_ref(&project), &found);

        // The parser names the line after the one it stops at, as it does for the other CLIs' files.
        assert_eq!(inv.unreadable, vec![(config, "It doesn't parse (line 3).".to_string())]);
        assert!(!format!("{inv:?}").contains("SECRET-6N3"));
    }

    #[test]
    fn no_value_of_config_json_reaches_the_inventory() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let project = h.path().join("proj");
        put(&project.join(".mcp.json"), r#"{"mcpServers": {"p": {"command": "c"}}}"#);
        put(
            &r.copilot_dir.join("config.json"),
            &format!(
                "// User settings\n{{\"token\": \"SECRET-6N3\", \"copilot_tokens\": {{\"a\": \"SECRET-6N3\"}}, \"trustedFolders\": [{:?}]}}",
                project.to_string_lossy()
            ),
        );
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert!(in_project(&inv, "p", &project).launch.is_ok());
        assert!(!format!("{inv:?}").contains("SECRET-6N3"));

        // The same with `installedPlugins` in it: an entry's other fields and the rest of the file stay out.
        let plugin = r.copilot_dir.join("installed-plugins").join("mk").join("pl");
        put(
            &plugin.join("plugin.json"),
            r#"{"mcpServers": {"ps": {"command": "c"}}}"#,
        );
        skill_md(&plugin.join("skills"), "ps", &md(None, Some("d")));
        put(
            &r.copilot_dir.join("config.json"),
            &format!(
                "// User settings\n{{\"token\": \"SECRET-6N3\", \"copilot_tokens\": {{\"a\": \"SECRET-6N3\"}}, \"trustedFolders\": [{:?}], \"installedPlugins\": [{{\"name\": \"pl\", \"marketplace\": \"mk\", \"enabled\": true, \"token\": \"SECRET-6N3\", \"auth\": {{\"k\": \"SECRET-6N3\"}}}}]}}",
                project.to_string_lossy()
            ),
        );
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert!(in_project(&inv, "p", &project).launch.is_ok());
        assert!(has_server(&inv, "ps") && has_skill(&inv, "pl:ps"));
        assert!(!format!("{inv:?}").contains("SECRET-6N3"));

        // And with a config.json that doesn't parse.
        put(
            &r.copilot_dir.join("config.json"),
            "{\"installedPlugins\": [], \"token\": \"SECRET-6N3\" oops}",
        );
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert_eq!(inv.unreadable.len(), 1);
        assert!(!format!("{inv:?}").contains("SECRET-6N3"));
    }

    #[test]
    fn the_cost_counts_full_definitions_and_the_entrys_tool_filters() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        user_servers(
            &r,
            r#"{"mcpServers": {
                "some": {"command": "c", "tools": ["a", "b"], "excludeTools": ["b"]},
                "star": {"command": "c", "tools": ["*"]},
                "none": {"command": "c"},
                "empty": {"command": "c", "tools": []},
                "odd": {"command": "c", "tools": "a", "excludeTools": "b"}
            }}"#,
        );
        let inv = inventory(&r, &[], &found);
        let cost = |n: &str| server(&inv, n).cost.clone();
        let full = |n: &str| Listing::Full {
            prefix: format!("{n}-"),
            desc_cap: None,
        };
        assert_eq!(
            cost("some"),
            CostRule {
                listing: full("some"),
                always_load_meta: None,
                only: Some(vec!["a".into(), "b".into()]),
                except: vec!["b".into()],
            }
        );
        for n in ["star", "none", "odd"] {
            assert_eq!(cost(n).listing, full(n), "{n}");
            assert_eq!(cost(n).only, None, "{n}");
            assert!(cost(n).except.is_empty(), "{n}");
        }
        assert_eq!(cost("empty").only, Some(vec![]));
    }

    #[test]
    fn the_github_server_comes_with_the_cli_unless_it_is_offline_or_the_person_names_one() {
        let h = tempfile::tempdir().unwrap();
        let mut r = roots(h.path());
        let settings = r.copilot_dir.join("settings.json");

        let inv = inventory(&r, &[], &found);
        assert_eq!(inv.servers.len(), 1, "{:?}", inv.servers);
        let gh = server(&inv, GITHUB_SERVER);
        assert_eq!(GITHUB_SERVER, "github-mcp-server");
        assert_eq!(gh.origin, Origin::BuiltIn);
        assert_eq!(gh.file, settings);
        assert_eq!(gh.transport, Transport::Remote);
        assert_eq!(gh.launch, Err(NotMeasured::Remote));
        assert_eq!(gh.switch, switch(&r, GITHUB_SERVER));
        assert_eq!(gh.used_as, UsedAs::Server(GITHUB_SERVER.into()));
        assert!(!gh.off);
        assert!(inv.servers.iter().all(|s| s.name != "computer-use"));

        put(&settings, r#"{"disabledMcpServers": ["github-mcp-server"]}"#);
        assert!(server(&inventory(&r, &[], &found), GITHUB_SERVER).off);

        // A configured server of that name, in the user's file or a project's, takes its place.
        put(&settings, "{}");
        user_servers(&r, r#"{"mcpServers": {"github-mcp-server": {"command": "mine"}}}"#);
        let inv = inventory(&r, &[], &found);
        assert_eq!(inv.servers.len(), 1);
        assert_eq!(server(&inv, GITHUB_SERVER).origin, Origin::User);
        std::fs::remove_file(r.copilot_dir.join("mcp-config.json")).unwrap();
        let project = h.path().join("p");
        put(
            &project.join(".mcp.json"),
            r#"{"mcpServers": {"github-mcp-server": {"url": "u"}}}"#,
        );
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert_eq!(inv.servers.len(), 1);
        assert_eq!(inv.servers[0].origin, Origin::Project(project.clone()));
        std::fs::remove_file(project.join(".mcp.json")).unwrap();

        r.copilot_offline = true;
        assert!(inventory(&r, &[], &found).servers.is_empty());
    }

    #[test]
    fn two_live_servers_with_one_name_are_report_only_and_a_server_already_off_frees_the_other() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let project = h.path().join("proj");
        user_servers(
            &r,
            r#"{"mcpServers": {"dup": {"command": "c"}, "Echo": {"command": "c"}, "solo": {"command": "c"}}}"#,
        );
        put(
            &project.join(".mcp.json"),
            r#"{"mcpServers": {"dup": {"command": "c"}, "echo": {"command": "c"}}}"#,
        );

        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert_eq!(server(&inv, "dup").switch, Err(NoSwitch::SameName));
        assert_eq!(in_project(&inv, "dup", &project).switch, Err(NoSwitch::SameName));
        assert_eq!(server(&inv, "solo").switch, switch(&r, "solo"));
        assert_eq!(server(&inv, "Echo").switch, switch(&r, "Echo"));
        assert_eq!(server(&inv, "echo").switch, switch(&r, "echo"));

        // The project's own settings already turn its `dup` off: the user's keeps its switch.
        put(
            &project.join(".github").join("copilot").join("settings.json"),
            r#"{"disabledMcpServers": ["dup"]}"#,
        );
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert!(in_project(&inv, "dup", &project).off);
        let user_dup = inv
            .servers
            .iter()
            .find(|s| s.name == "dup" && s.origin == Origin::User)
            .unwrap();
        assert_eq!(user_dup.switch, switch(&r, "dup"));

        // A copy that can't start doesn't count as in use.
        put(&project.join(".github").join("copilot").join("settings.json"), "{}");
        let judge = |c: &str| {
            if c == "gone" {
                CommandState::NotOnPath
            } else {
                CommandState::Found
            }
        };
        put(
            &project.join(".mcp.json"),
            r#"{"mcpServers": {"dup": {"command": "gone"}}}"#,
        );
        let inv = inventory(&r, std::slice::from_ref(&project), &judge);
        let user_dup = inv
            .servers
            .iter()
            .find(|s| s.name == "dup" && s.origin == Origin::User)
            .unwrap();
        assert!(user_dup.switch.is_ok());
    }

    #[test]
    fn a_list_that_isnt_an_array_of_strings_leaves_every_server_with_no_switch() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let settings = r.copilot_dir.join("settings.json");
        user_servers(&r, r#"{"mcpServers": {"a": {"command": "c"}, "web": {"url": "u"}}}"#);
        for text in [
            r#"{"disabledMcpServers": "a"}"#,
            r#"{"disabledMcpServers": ["a", 1]}"#,
            r#"{"disabledMcpServers": null}"#,
            r#"{"disabledMcpServers": {"a": true}}"#,
            r#"["a"]"#,
        ] {
            put(&settings, text);
            let inv = inventory(&r, &[], &found);
            assert_eq!(inv.servers.len(), 3, "{text}");
            for s in &inv.servers {
                assert_eq!(
                    s.switch,
                    Err(NoSwitch::CantEdit(settings.clone())),
                    "{text}: {}",
                    s.name
                );
                assert!(!s.off, "{text}: {}", s.name);
            }
        }
        // A settings.json that doesn't parse is the same, and is reported.
        put(&settings, "{ \"disabledMcpServers\": [\"a\"] oops");
        let inv = inventory(&r, &[], &found);
        assert_eq!(server(&inv, "a").switch, Err(NoSwitch::CantEdit(settings.clone())));
        assert_eq!(inv.unreadable.len(), 1);
        assert_eq!(inv.unreadable[0].0, settings);
        // Comments, a trailing comma and a BOM are fine, and an empty list or none at all too.
        put(
            &settings,
            "\u{feff}{\r\n // c\r\n \"disabledMcpServers\": [\"a\",],\r\n}\r\n",
        );
        let inv = inventory(&r, &[], &found);
        assert!(server(&inv, "a").off);
        assert_eq!(server(&inv, "web").switch, switch(&r, "web"));
        put(&settings, r#"{"disabledMcpServers": []}"#);
        assert_eq!(server(&inventory(&r, &[], &found), "a").switch, switch(&r, "a"));
    }

    #[test]
    fn an_mcp_config_that_cant_be_read_is_reported_and_gives_no_server() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let file = r.copilot_dir.join("mcp-config.json");
        let project = h.path().join("proj");
        let only_github = |inv: &Inventory| inv.servers.iter().all(|s| s.name == GITHUB_SERVER);

        put(&file, "{\n  \"mcpServers\": {\"x\": \"sk-SECRET\" oops}\n}\n");
        let inv = inventory(&r, &[], &found);
        assert!(only_github(&inv));
        assert_eq!(
            inv.unreadable,
            vec![(file.clone(), "It doesn't parse (line 2).".to_string())]
        );
        assert!(!format!("{inv:?}").contains("SECRET"));

        std::fs::write(&file, [0xff, 0xfe, 0x41]).unwrap();
        let inv = inventory(&r, &[], &found);
        assert!(only_github(&inv));
        assert_eq!(inv.unreadable, vec![(file.clone(), unreadable::NOT_UTF8.to_string())]);

        std::fs::write(&file, vec![b' '; MAX_CONFIG_BYTES as usize + 1]).unwrap();
        let inv = inventory(&r, &[], &found);
        assert!(only_github(&inv));
        assert_eq!(inv.unreadable, vec![(file.clone(), TOO_LARGE_DETAIL.to_string())]);

        // A project's file the same way; a file that isn't an object, or holds no servers, is read as having none.
        std::fs::remove_file(&file).unwrap();
        put(&project.join(".mcp.json"), "[1, 2");
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert!(only_github(&inv));
        assert_eq!(inv.unreadable.len(), 1);
        for text in ["[1, 2]", "{}", "\"x\"", "", "{\"mcpServers\": [1]}"] {
            put(&project.join(".mcp.json"), text);
            let inv = inventory(&r, std::slice::from_ref(&project), &found);
            assert!(only_github(&inv), "{text}");
            assert!(inv.unreadable.is_empty(), "{text}");
        }
    }

    #[test]
    fn the_copilot_folder_is_the_one_in_the_roots_and_the_hints_name_copilots_files() {
        let h = tempfile::tempdir().unwrap();
        let mut r = roots(h.path());
        r.copilot_dir = h.path().join("elsewhere");
        put(
            &r.copilot_dir.join("mcp-config.json"),
            r#"{"mcpServers": {"a": {"command": "c"}}}"#,
        );
        put(
            &h.path().join(".copilot").join("mcp-config.json"),
            r#"{"mcpServers": {"wrong": {"command": "c"}}}"#,
        );
        let inv = inventory(&r, &[], &found);
        assert_eq!(server(&inv, "a").switch, switch(&r, "a"));
        assert!(inv.servers.iter().all(|s| s.name != "wrong"));
        let project = Path::new(r"C:\dev\p");
        assert_eq!(
            (inv.hints.move_server)("a", project),
            r"To load it only there, move its entry to mcpServers in C:\dev\p\.mcp.json."
        );
        assert_eq!(
            (inv.hints.move_skill)("a", project),
            r"To load it only there, move its folder to C:\dev\p\.github\skills."
        );
    }

    // ---- skills ----

    fn skill_md(dir: &Path, folder: &str, text: &str) -> PathBuf {
        let file = dir.join(folder).join("SKILL.md");
        put(&file, text);
        file
    }

    fn md(name: Option<&str>, description: Option<&str>) -> String {
        let mut text = String::from("---\n");
        if let Some(n) = name {
            text.push_str(&format!("name: {n}\n"));
        }
        if let Some(d) = description {
            text.push_str(&format!("description: {d}\n"));
        }
        text.push_str("---\nThe body.\n");
        text
    }

    fn skill_of<'a>(inv: &'a Inventory, name: &str) -> &'a Skill {
        inv.skills.iter().find(|s| s.name == name).unwrap_or_else(|| {
            panic!(
                "no skill {name}: {:?}",
                inv.skills.iter().map(|s| &s.name).collect::<Vec<_>>()
            )
        })
    }

    fn skill_in<'a>(inv: &'a Inventory, name: &str, project: &Path) -> &'a Skill {
        inv.skills
            .iter()
            .find(|s| s.name == name && s.origin == Origin::Project(project.to_path_buf()))
            .unwrap_or_else(|| panic!("no skill {name} in {}", project.display()))
    }

    fn by_folder<'a>(inv: &'a Inventory, folder: &str) -> &'a Skill {
        inv.skills
            .iter()
            .find(|s| s.file.parent().and_then(Path::file_name).is_some_and(|f| f == folder))
            .unwrap_or_else(|| panic!("no skill in folder {folder}"))
    }

    fn skill_switch(r: &ExtRoots, name: &str) -> Result<Switch, NoSwitch> {
        Ok(Switch::JsonAdd {
            file: r.copilot_dir.join("settings.json"),
            pointer: "/disabledSkills".into(),
            name: name.into(),
        })
    }

    fn named_only(inv: &Inventory) -> Vec<String> {
        let mut names: Vec<String> = inv
            .skills
            .iter()
            .filter(|s| s.name_only)
            .map(|s| s.name.clone())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn skills_come_from_copilots_own_folders_and_never_from_claudes_user_folder() {
        let h = tempfile::tempdir().unwrap();
        let mut r = roots(h.path());
        let project = h.path().join("proj");
        skill_md(&r.copilot_dir.join("skills"), "own", &md(None, Some("d")));
        skill_md(&r.copilot_dir.join("skills"), ".dot", &md(None, Some("d")));
        skill_md(&h.path().join(".agents").join("skills"), "shared", &md(None, Some("d")));
        skill_md(
            &h.path().join(".claude").join("skills"),
            "claudes",
            &md(None, Some("d")),
        );
        skill_md(&project.join(".github").join("skills"), "gh", &md(None, Some("d")));
        skill_md(&project.join(".agents").join("skills"), "ag", &md(None, Some("d")));
        skill_md(&project.join(".claude").join("skills"), "cl", &md(None, Some("d")));
        skill_md(&project.join(".copilot").join("skills"), "nope", &md(None, Some("d")));

        let inv = inventory(&r, std::slice::from_ref(&project), &found);

        let mut names: Vec<&str> = inv.skills.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["ag", "cl", "gh", "own", "shared"]);
        assert_eq!(skill_of(&inv, "own").origin, Origin::User);
        assert_eq!(
            skill_of(&inv, "own").file,
            r.copilot_dir.join("skills").join("own").join("SKILL.md")
        );
        assert_eq!(skill_of(&inv, "shared").origin, Origin::User);
        for n in ["gh", "ag", "cl"] {
            assert_eq!(skill_of(&inv, n).origin, Origin::Project(project.clone()), "{n}");
        }
        assert_eq!(inv.caps.per_skill, None);
        assert_eq!(inv.caps.listing_first_line_over, None);
        assert_eq!(inv.caps.listing_named_over, Some(15_000));

        // With COPILOT_HOME set Copilot skips ~/.agents/skills.
        r.copilot_home_set = true;
        let inv = inventory(&r, std::slice::from_ref(&project), &found);
        assert!(inv.skills.iter().all(|s| s.name != "shared"));
        assert!(inv.skills.iter().any(|s| s.name == "own"));
    }

    #[test]
    fn a_skills_name_is_its_frontmatters_or_its_folders_and_the_rules_decide_its_state() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let dir = r.copilot_dir.join("skills");
        let rep = |c: &str, n: usize| c.repeat(n);
        let cases: Vec<(&str, String, String, SkillState)> = vec![
            (
                "nob",
                "just text, no frontmatter\n".into(),
                "nob".into(),
                SkillState::NoBlock,
            ),
            (
                "open",
                "---\nname: open\ndescription: d\n".into(),
                "open".into(),
                SkillState::NoBlock,
            ),
            ("n64", md(Some(&rep("n", 64)), Some("d")), rep("n", 64), SkillState::Ok),
            (
                "n65",
                md(Some(&rep("n", 65)), Some("d")),
                rep("n", 65),
                SkillState::TooLong("name", 64),
            ),
            (
                "both",
                md(Some(&rep("m", 65)), Some(&rep("d", 2000))),
                rep("m", 65),
                SkillState::TooLong("name", 64),
            ),
            (
                "bang",
                md(Some("bad!name"), Some("d")),
                "bad!name".into(),
                SkillState::BadName,
            ),
            (
                "lead",
                md(Some("-lead"), Some("d")),
                "-lead".into(),
                SkillState::BadName,
            ),
            ("uni", md(Some("café"), Some("d")), "café".into(), SkillState::BadName),
            (
                "fine",
                md(Some("a b:c.d_e-f"), Some("d")),
                "a b:c.d_e-f".into(),
                SkillState::Ok,
            ),
            ("d1024", md(None, Some(&rep("d", 1024))), "d1024".into(), SkillState::Ok),
            (
                "d1025",
                md(None, Some(&rep("d", 1025))),
                "d1025".into(),
                SkillState::TooLong("description", 1024),
            ),
            ("wide", md(None, Some(&rep("é", 1024))), "wide".into(), SkillState::Ok),
            (
                "wider",
                md(None, Some(&rep("é", 1025))),
                "wider".into(),
                SkillState::TooLong("description", 1024),
            ),
            ("noname", md(None, Some("d")), "noname".into(), SkillState::Ok),
            ("nodesc", md(Some("nodesc"), None), "nodesc".into(), SkillState::Ok),
            (
                "unclear",
                format!("---\nname: unclear\nnot a pair\ndescription: {}\n---\n", rep("d", 2000)),
                "unclear".into(),
                SkillState::Ok,
            ),
        ];
        for (folder, text, _, _) in &cases {
            skill_md(&dir, folder, text);
        }

        let inv = inventory(&r, &[], &found);

        for (folder, _, name, state) in cases {
            let s = by_folder(&inv, folder);
            assert_eq!(s.name, name, "{folder}");
            assert_eq!(s.state, state, "{folder}");
        }
    }

    #[test]
    fn what_a_skill_lists_is_its_description_or_the_first_line_of_its_body() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let dir = r.copilot_dir.join("skills");
        skill_md(
            &dir,
            "desc",
            "---\nname: desc\ndescription:   abcde  \n---\nBody line.\n",
        );
        skill_md(
            &dir,
            "body",
            "\u{feff}---\r\nname: body\r\n---\r\n\r\n  First line here  \r\nsecond\r\n",
        );
        skill_md(&dir, "empty", "---\nname: empty\n---\n\n\n");
        skill_md(
            &dir,
            "blank",
            "---\nname: blank\ndescription: \"\"\n---\nFrom the body\n",
        );

        let inv = inventory(&r, &[], &found);

        assert_eq!(skill_of(&inv, "desc").listed_chars, 5);
        assert_eq!(skill_of(&inv, "body").listed_chars, "First line here".len());
        assert_eq!(skill_of(&inv, "empty").listed_chars, 0);
        assert_eq!(skill_of(&inv, "blank").listed_chars, "From the body".len());
        assert!(inv.skills.iter().all(|s| s.state == SkillState::Ok));
    }

    #[test]
    fn disabled_skills_match_exactly_and_a_projects_own_settings_turn_off_its_skills_only() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let (p1, p2) = (h.path().join("p1"), h.path().join("p2"));
        put(
            &r.copilot_dir.join("settings.json"),
            "{\n  // mine\n  \"disabledSkills\": [\"good-skill\", \"u-off\",],\n}\n",
        );
        let user = r.copilot_dir.join("skills");
        for n in ["good-skill", "u-off", "plain"] {
            skill_md(&user, n, &md(None, Some("d")));
        }
        // The folder has another name: the skill's name is its frontmatter's, and a file system may ignore case.
        skill_md(&user, "upper", &md(Some("Good-Skill"), Some("d")));
        for p in [&p1, &p2] {
            for n in ["a", "b", "c"] {
                skill_md(&p.join(".github").join("skills"), n, &md(None, Some("d")));
            }
        }
        skill_md(&p1.join(".github").join("skills"), "only1", &md(None, Some("d")));
        put(
            &p1.join(".github").join("copilot").join("settings.json"),
            r#"{"disabledSkills": ["a"]}"#,
        );
        put(
            &p1.join(".github").join("copilot").join("settings.local.json"),
            r#"{"disabledSkills": ["b", 7]}"#,
        );
        put(
            &p1.join(".github").join("copilot").join("other.json"),
            r#"{"disabledSkills": ["c"]}"#,
        );

        let inv = inventory(&r, &[p1.clone(), p2.clone()], &found);

        assert!(skill_of(&inv, "good-skill").off);
        assert!(skill_of(&inv, "u-off").off);
        assert!(!skill_of(&inv, "Good-Skill").off, "the match is exact");
        assert!(!skill_of(&inv, "plain").off);
        assert!(skill_in(&inv, "a", &p1).off);
        assert!(skill_in(&inv, "b", &p1).off);
        assert!(!skill_in(&inv, "c", &p1).off);
        assert!(!skill_in(&inv, "a", &p2).off, "another project's settings don't count");
        // The switch always writes the user's settings, for a project's skill too.
        assert_eq!(skill_in(&inv, "only1", &p1).switch, skill_switch(&r, "only1"));
        assert_eq!(skill_of(&inv, "plain").switch, skill_switch(&r, "plain"));
        // A user list that names a project's skill turns it off.
        put(&r.copilot_dir.join("settings.json"), r#"{"disabledSkills": ["c"]}"#);
        let inv = inventory(&r, &[p1.clone(), p2.clone()], &found);
        assert!(skill_in(&inv, "c", &p1).off && skill_in(&inv, "c", &p2).off);
    }

    #[test]
    fn a_disabled_skills_list_that_isnt_an_array_of_strings_leaves_every_skill_with_no_switch() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let settings = r.copilot_dir.join("settings.json");
        skill_md(&r.copilot_dir.join("skills"), "a", &md(None, Some("d")));
        for text in [
            r#"{"disabledSkills": "a"}"#,
            r#"{"disabledSkills": ["a", 1]}"#,
            r#"{"disabledSkills": null}"#,
            r#"{"disabledSkills": {"a": true}}"#,
            r#"["a"]"#,
            r#"{"disabledSkills": ["a"] oops"#,
        ] {
            put(&settings, text);
            let inv = inventory(&r, &[], &found);
            let a = skill_of(&inv, "a");
            assert_eq!(a.switch, Err(NoSwitch::CantEdit(settings.clone())), "{text}");
            assert!(!a.off, "{text}");
        }
        put(
            &settings,
            "\u{feff}{\r\n // c\r\n \"disabledSkills\": [\"a\",],\r\n}\r\n",
        );
        assert!(skill_of(&inventory(&r, &[], &found), "a").off);
        put(&settings, "{}");
        assert_eq!(skill_of(&inventory(&r, &[], &found), "a").switch, skill_switch(&r, "a"));
    }

    #[test]
    fn a_skill_is_hidden_by_disable_model_invocation_and_is_used_by_its_file_or_its_name() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let dir = r.copilot_dir.join("skills");
        let quiet = skill_md(
            &dir,
            "quiet",
            "---\nname: quiet\ndescription: d\ndisable-model-invocation: true\n---\n",
        );
        skill_md(
            &dir,
            "loud",
            "---\nname: loud\ndescription: d\ndisable-model-invocation: false\n---\n",
        );

        let inv = inventory(&r, &[], &found);

        assert!(skill_of(&inv, "quiet").hidden);
        assert!(!skill_of(&inv, "loud").hidden);
        assert_eq!(
            skill_of(&inv, "quiet").used_as,
            vec![SkillUse::File(quiet), SkillUse::Name("quiet".into())]
        );
    }

    #[test]
    fn a_personal_skill_a_project_skill_shadows_is_off_and_live_twins_are_report_only() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let (p1, p2) = (h.path().join("p1"), h.path().join("p2"));
        let user = r.copilot_dir.join("skills");
        for n in ["shared", "solo", "Twin"] {
            skill_md(&user, n, &md(None, Some("d")));
        }
        for p in [&p1, &p2] {
            skill_md(&p.join(".github").join("skills"), "twin", &md(None, Some("d")));
        }
        skill_md(&p1.join(".github").join("skills"), "shared", &md(None, Some("d")));

        let inv = inventory(&r, &[p1.clone(), p2.clone()], &found);

        assert!(skill_of(&inv, "shared").off, "Copilot skips it for the project's");
        assert!(!skill_in(&inv, "shared", &p1).off);
        assert_eq!(skill_in(&inv, "shared", &p1).switch, skill_switch(&r, "shared"));
        assert_eq!(skill_of(&inv, "solo").switch, skill_switch(&r, "solo"));
        assert_eq!(skill_in(&inv, "twin", &p1).switch, Err(NoSwitch::SameName));
        assert_eq!(skill_in(&inv, "twin", &p2).switch, Err(NoSwitch::SameName));
        assert!(!skill_of(&inv, "Twin").off, "names that differ by case are two skills");
        assert_eq!(skill_of(&inv, "Twin").switch, skill_switch(&r, "Twin"));

        // One twin already off through its project's settings frees the other.
        put(
            &p2.join(".github").join("copilot").join("settings.json"),
            r#"{"disabledSkills": ["twin"]}"#,
        );
        let inv = inventory(&r, &[p1.clone(), p2.clone()], &found);
        assert!(skill_in(&inv, "twin", &p2).off);
        assert_eq!(skill_in(&inv, "twin", &p1).switch, skill_switch(&r, "twin"));
    }

    #[test]
    fn two_hundred_skills_are_described_131_and_named_69_as_copilot_did() {
        // Research note, finding 6: 19 + 131 * (8 + 15 + 91) = 14,953, and the next entry would make 15,067.
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        for i in 0..200 {
            skill_md(
                &r.copilot_dir.join("skills"),
                &format!("skill{i:03}"),
                &md(None, Some(&"d".repeat(15))),
            );
        }
        let inv = inventory(&r, &[], &found);
        assert_eq!(inv.skills.len(), 200);
        let named = named_only(&inv);
        assert_eq!(named.len(), 69);
        assert_eq!(named[0], "skill131");
        assert_eq!(named[68], "skill199");

        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        for i in 0..10 {
            skill_md(
                &r.copilot_dir.join("skills"),
                &format!("small{i}"),
                &md(None, Some("short")),
            );
        }
        assert!(named_only(&inventory(&r, &[], &found)).is_empty());
    }

    #[test]
    fn the_budget_is_spent_on_project_skills_first_and_on_listed_skills_only() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let project = h.path().join("proj");
        let pdir = project.join(".github").join("skills");
        for i in 0..131 {
            skill_md(&pdir, &format!("proj{i:04}"), &md(None, Some(&"d".repeat(15))));
        }
        // None of these takes from the budget: hidden, off, and one Copilot refuses.
        skill_md(
            &pdir,
            "0hidden",
            &format!(
                "---\nname: 0hidden\ndescription: {}\ndisable-model-invocation: true\n---\n",
                "h".repeat(1000)
            ),
        );
        skill_md(&pdir, "0off", &md(None, Some(&"o".repeat(1000))));
        skill_md(&pdir, "0long", &md(None, Some(&"l".repeat(1025))));
        put(&r.copilot_dir.join("settings.json"), r#"{"disabledSkills": ["0off"]}"#);
        // Sorts before every project skill by name, and is still listed after them.
        skill_md(
            &r.copilot_dir.join("skills"),
            "aaaaaaaa",
            &md(None, Some(&"d".repeat(15))),
        );

        let inv = inventory(&r, std::slice::from_ref(&project), &found);

        assert_eq!(named_only(&inv), ["aaaaaaaa"]);
        assert!(!skill_in(&inv, "0hidden", &project).name_only);
        assert!(!skill_in(&inv, "0off", &project).name_only);
        assert!(!skill_in(&inv, "0long", &project).name_only);
    }

    #[test]
    fn a_skill_that_makes_the_listing_exactly_15000_is_described_and_one_more_character_is_not() {
        let build = |last: usize| {
            let h = tempfile::tempdir().unwrap();
            let r = roots(h.path());
            let dir = r.copilot_dir.join("skills");
            // 19 + 14 * (3 + 906 + 91) = 14,019.
            for i in 0..14 {
                skill_md(&dir, &format!("s{i:02}"), &md(None, Some(&"d".repeat(906))));
            }
            // s14 costs 3 + last + 91: 887 makes 15,000 exactly.
            skill_md(&dir, "s14", &md(None, Some(&"d".repeat(last))));
            skill_md(&dir, "s15", &md(None, Some("d")));
            named_only(&inventory(&r, &[], &found))
        };
        assert_eq!(build(887), ["s15"]);
        assert_eq!(build(888), ["s14", "s15"]);
    }

    #[test]
    fn a_skill_file_over_the_size_limit_is_not_read_and_is_reported() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let dir = r.copilot_dir.join("skills");
        let mut text = md(None, Some("d"));
        text.push_str(&" ".repeat(MAX_CONFIG_BYTES as usize));
        let big = skill_md(&dir, "big", &text);
        skill_md(&dir, "small", &md(None, Some("d")));

        let inv = inventory(&r, &[], &found);

        assert!(inv.skills.iter().all(|s| s.name != "big"));
        assert!(inv.skills.iter().any(|s| s.name == "small"));
        assert_eq!(inv.unreadable, vec![(big, TOO_LARGE_DETAIL.to_string())]);
    }

    // ---- plugins ----

    fn installed(r: &ExtRoots) -> PathBuf {
        r.copilot_dir.join("installed-plugins")
    }

    /// `config.json` as Copilot writes it, with its two `//` lines, holding `installedPlugins`.
    fn plugins_config(r: &ExtRoots, plugins: &serde_json::Value) {
        put(
            &r.copilot_dir.join("config.json"),
            &format!(
                "// User settings\n// Edit with care\n{}\n",
                serde_json::json!({"installedPlugins": plugins})
            ),
        );
    }

    fn has_server(inv: &Inventory, name: &str) -> bool {
        inv.servers.iter().any(|s| s.name == name)
    }

    fn has_skill(inv: &Inventory, name: &str) -> bool {
        inv.skills.iter().any(|s| s.name == name)
    }

    /// A plugin folder with a manifest that holds one stdio server named `server`.
    fn plugin_with_server(dir: &Path, server: &str) {
        put(
            &dir.join("plugin.json"),
            &serde_json::json!({"mcpServers": {server: {"command": "c"}}}).to_string(),
        );
    }

    #[test]
    fn an_enabled_plugins_servers_and_skills_are_listed_and_none_can_be_switched() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let dir = installed(&r).join("mk").join("good");
        put(
            &dir.join("plugin.json"),
            r#"{"name": "good", "mcpServers": {"s1": {"command": "c"}, "web": {"url": "u"}}}"#,
        );
        put(&dir.join(".mcp.json"), r#"{"mcpServers": {"s2": {"command": "c"}}}"#);
        let foo = skill_md(&dir.join("skills"), "foo", &md(None, Some("plugin skill")));
        plugins_config(
            &r,
            &serde_json::json!([{"name": "good", "marketplace": "mk", "enabled": true, "version": "1.0"}]),
        );

        let inv = inventory(&r, &[], &found);

        let s1 = server(&inv, "s1");
        assert_eq!(s1.origin, Origin::Plugin("good".into()));
        assert_eq!(s1.file, dir.join("plugin.json"));
        assert_eq!(s1.switch, Err(NoSwitch::Plugin("good".into())));
        assert_eq!(s1.launch, Err(NotMeasured::NotApproved));
        assert_eq!(
            s1.transport,
            Transport::Stdio {
                command: "c".into(),
                state: CommandState::Found
            }
        );
        assert_eq!(s1.used_as, UsedAs::Server("s1".into()));
        let web = server(&inv, "web");
        assert_eq!(web.launch, Err(NotMeasured::Remote));
        assert_eq!(web.switch, Err(NoSwitch::Plugin("good".into())));
        let s2 = server(&inv, "s2");
        assert_eq!(s2.file, dir.join(".mcp.json"));
        assert_eq!(s2.origin, Origin::Plugin("good".into()));

        let skill = skill_of(&inv, "good:foo");
        assert_eq!(skill.origin, Origin::Plugin("good".into()));
        assert_eq!(skill.switch, Err(NoSwitch::Plugin("good".into())));
        assert_eq!(skill.file, foo);
        assert_eq!(
            skill.used_as,
            vec![SkillUse::File(foo), SkillUse::Name("good:foo".into())]
        );
        assert!(inv.unreadable.is_empty());
    }

    #[test]
    fn a_plugin_is_read_only_when_it_is_enabled_and_the_users_settings_dont_turn_it_off() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let names = ["on", "flag-off", "muted", "no-flag", "str-flag", "listed-on"];
        for n in names {
            plugin_with_server(&installed(&r).join("mk").join(n), &format!("srv-{n}"));
        }
        plugins_config(
            &r,
            &serde_json::json!([
                {"name": "on", "marketplace": "mk", "enabled": true},
                {"name": "flag-off", "marketplace": "mk", "enabled": false},
                {"name": "muted", "marketplace": "mk", "enabled": true},
                {"name": "no-flag", "marketplace": "mk"},
                {"name": "str-flag", "marketplace": "mk", "enabled": "true"},
                {"name": "listed-on", "marketplace": "mk", "enabled": true},
                {"name": "on", "marketplace": "mk", "enabled": true},
            ]),
        );
        put(
            &r.copilot_dir.join("settings.json"),
            r#"{"enabledPlugins": {"muted@mk": false, "listed-on@mk": true, "flag-off@mk": true}}"#,
        );

        let inv = inventory(&r, &[], &found);

        let mut seen: Vec<&str> = inv
            .servers
            .iter()
            .filter(|s| matches!(s.origin, Origin::Plugin(_)))
            .map(|s| s.name.as_str())
            .collect();
        seen.sort();
        assert_eq!(seen, ["srv-listed-on", "srv-on"], "a plugin listed twice is read once");
    }

    #[test]
    fn a_plugins_manifest_is_the_first_of_four_and_its_paths_are_followed_inside_its_folder() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let mk = installed(&r).join("mk");
        // Two manifests: the first in Copilot's order wins.
        put(
            &mk.join("first").join(".github").join("plugin").join("plugin.json"),
            r#"{"mcpServers": {"from-github": {"command": "c"}}}"#,
        );
        put(
            &mk.join("first").join(".claude-plugin").join("plugin.json"),
            r#"{"mcpServers": {"from-claude": {"command": "c"}}}"#,
        );
        put(
            &mk.join("dot").join(".plugin").join("plugin.json"),
            r#"{"mcpServers": {"from-dot": {"command": "c"}}}"#,
        );
        put(
            &mk.join("claude").join(".claude-plugin").join("plugin.json"),
            r#"{"mcpServers": {"only-claude": {"command": "c"}}}"#,
        );
        // A path to a file, with and without the `mcpServers` wrapper, and a skills folder of its own.
        put(
            &mk.join("paths").join("plugin.json"),
            r#"{"mcpServers": "./sub/../conf/servers.json", "skills": "./custom"}"#,
        );
        put(
            &mk.join("paths").join("conf").join("servers.json"),
            r#"{"bare": {"command": "c"}}"#,
        );
        skill_md(&mk.join("paths").join("custom"), "mine", &md(None, Some("d")));
        skill_md(&mk.join("paths").join("skills"), "default", &md(None, Some("d")));
        put(
            &mk.join("wrapped").join("plugin.json"),
            r#"{"mcpServers": "conf.json"}"#,
        );
        put(
            &mk.join("wrapped").join("conf.json"),
            r#"{"mcpServers": {"wrapped-srv": {"command": "c"}}}"#,
        );
        // The manifest names `.mcp.json` itself: its servers are listed once.
        put(
            &mk.join("again").join("plugin.json"),
            r#"{"mcpServers": "./.mcp.json"}"#,
        );
        put(
            &mk.join("again").join(".mcp.json"),
            r#"{"mcpServers": {"again-srv": {"command": "c"}}}"#,
        );
        plugins_config(
            &r,
            &serde_json::json!(
                ["first", "dot", "claude", "paths", "wrapped", "again"]
                    .map(|n| serde_json::json!({"name": n, "marketplace": "mk", "enabled": true}))
            ),
        );

        let inv = inventory(&r, &[], &found);

        assert!(has_server(&inv, "from-github") && !has_server(&inv, "from-claude"));
        assert!(has_server(&inv, "from-dot") && has_server(&inv, "only-claude"));
        assert!(has_server(&inv, "bare") && has_server(&inv, "wrapped-srv"));
        assert_eq!(
            server(&inv, "bare").file,
            mk.join("paths").join("conf").join("servers.json")
        );
        assert!(has_skill(&inv, "paths:mine"), "the manifest's skills folder");
        assert!(
            !has_skill(&inv, "paths:default"),
            "the default folder only when the manifest names none"
        );
        assert_eq!(inv.servers.iter().filter(|s| s.name == "again-srv").count(), 1);
    }

    #[test]
    fn a_cache_path_is_followed_only_inside_the_installed_plugins_folder() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let ip = installed(&r);
        let ip_text = ip.to_string_lossy().into_owned();
        // Each folder holds a server; only the ones inside `installed-plugins` may be read.
        let refused = [
            ("out", h.path().join("elsewhere").join("p1")),
            ("evil", r.copilot_dir.join("installed-plugins-evil").join("p2")),
            ("dots", r.copilot_dir.join("outside").join("p3")),
            ("root", ip.clone()),
            ("deep", r.copilot_dir.join("outside2").join("p6")),
            ("parent", r.copilot_dir.clone()),
        ];
        for (n, dir) in &refused {
            plugin_with_server(dir, &format!("leak-{n}"));
        }
        plugin_with_server(&ip.join("mk").join("p7"), "inside-slashes");
        plugin_with_server(&ip.join("mk").join("p8"), "inside-case");
        plugin_with_server(&ip.join("p9"), "inside-plain");
        let entry = |n: &str, path: String| serde_json::json!({"name": n, "marketplace": "mk", "enabled": true, "cache_path": path});
        plugins_config(
            &r,
            &serde_json::json!([
                entry("out", refused[0].1.to_string_lossy().into_owned()),
                entry("evil", refused[1].1.to_string_lossy().into_owned()),
                entry("dots", format!("{ip_text}/../outside/p3")),
                entry("root", ip_text.clone()),
                entry("deep", format!("{ip_text}\\x\\..\\..\\outside2\\p6")),
                entry("parent", format!("{ip_text}/..")),
                entry("p7", format!("{ip_text}\\mk\\sub\\..\\p7")),
                entry("p8", format!("{}/mk/p8", ip_text.to_uppercase())),
                entry("p9a", format!("{ip_text}/")),
                entry("p9", format!("{ip_text}/p9/")),
            ]),
        );

        let inv = inventory(&r, &[], &found);

        for (n, _) in &refused {
            assert!(!has_server(&inv, &format!("leak-{n}")), "{n} is outside");
        }
        assert!(has_server(&inv, "inside-slashes"));
        assert!(has_server(&inv, "inside-plain"));
        // Case counts on Linux and macOS and not on Windows.
        assert_eq!(has_server(&inv, "inside-case"), cfg!(windows));
        assert!(inv.unreadable.is_empty());
    }

    #[test]
    fn a_manifest_path_that_leaves_the_plugins_folder_is_not_followed() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let mk = installed(&r).join("mk");
        let outside = h.path().join("outside.json");
        put(&outside, r#"{"mcpServers": {"esc-abs": {"command": "c"}}}"#);
        put(
            &mk.join("esc.json"),
            r#"{"mcpServers": {"esc-dots": {"command": "c"}}}"#,
        );
        put(
            &mk.join("esc2.json"),
            r#"{"mcpServers": {"esc-deep": {"command": "c"}}}"#,
        );
        put(&mk.join("esc-skills").join("e").join("SKILL.md"), &md(None, Some("d")));
        skill_md(&mk, "zz", &md(None, Some("d")));
        skill_md(h.path(), "hh", &md(None, Some("d")));
        let cases = [
            ("a", serde_json::json!({"mcpServers": outside.to_string_lossy()})),
            ("b", serde_json::json!({"mcpServers": "../esc.json"})),
            ("c", serde_json::json!({"mcpServers": "sub/../../esc2.json"})),
            ("d", serde_json::json!({"mcpServers": "C:/outside.json"})),
            ("e", serde_json::json!({"mcpServers": "\\\\host\\share\\x.json"})),
            ("f", serde_json::json!({"skills": "../esc-skills"})),
            ("g", serde_json::json!({"skills": ".."})),
            ("h", serde_json::json!({"skills": h.path().to_string_lossy()})),
        ];
        for (n, manifest) in &cases {
            put(&mk.join(n).join("plugin.json"), &manifest.to_string());
        }
        plugins_config(
            &r,
            &serde_json::json!(
                cases
                    .iter()
                    .map(|(n, _)| serde_json::json!({"name": n, "marketplace": "mk", "enabled": true}))
                    .collect::<Vec<_>>()
            ),
        );

        let inv = inventory(&r, &[], &found);

        for n in ["esc-abs", "esc-dots", "esc-deep"] {
            assert!(!has_server(&inv, n), "{n}");
        }
        assert!(
            inv.skills.is_empty(),
            "{:?}",
            inv.skills.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(inv.unreadable.is_empty());
    }

    #[test]
    fn a_plugin_whose_name_or_marketplace_would_leave_the_folder_gives_no_plugin_and_no_failure() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let ip = installed(&r);
        plugin_with_server(&ip, "leak-root");
        plugin_with_server(&r.copilot_dir.join("x"), "leak-up");
        plugin_with_server(&ip.join("mk").join("a").join("b"), "leak-sep");
        plugin_with_server(&h.path().join("abs"), "leak-abs");
        let abs = h.path().join("abs").to_string_lossy().into_owned();
        let bad: Vec<(&str, &str)> = vec![
            ("..", "mk"),
            ("x", ".."),
            (".", "mk"),
            ("a/b", "mk"),
            ("a\\b", "mk"),
            ("x", "C:"),
            ("", "mk"),
            ("x", ""),
            (abs.as_str(), "mk"),
            ("x", abs.as_str()),
            ("x", "mk/../.."),
        ];
        plugins_config(
            &r,
            &serde_json::json!(
                bad.iter()
                    .map(|(n, m)| serde_json::json!({"name": n, "marketplace": m, "enabled": true}))
                    .collect::<Vec<_>>()
            ),
        );

        let inv = inventory(&r, &[], &found);

        assert!(inv.servers.iter().all(|s| s.name == GITHUB_SERVER), "{:?}", inv.servers);
        assert!(inv.skills.is_empty());
        assert!(inv.unreadable.is_empty());
    }

    #[test]
    fn installed_plugins_in_another_shape_give_no_plugin_and_no_failure() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        plugin_with_server(&installed(&r).join("mk").join("x"), "leak");
        for shape in [
            serde_json::json!("x"),
            serde_json::json!({"x": {"name": "x", "marketplace": "mk", "enabled": true}}),
            serde_json::json!([1, "x", null, [1]]),
            serde_json::json!([{"marketplace": "mk", "enabled": true}]),
            serde_json::json!([{"name": 5, "marketplace": "mk", "enabled": true}]),
            serde_json::json!([{"name": "x", "marketplace": ["mk"], "enabled": true}]),
            serde_json::json!([{"name": "x", "marketplace": "mk", "enabled": true, "cache_path": 5}]),
            serde_json::json!(null),
        ] {
            plugins_config(&r, &shape);
            let inv = inventory(&r, &[], &found);
            assert!(!has_server(&inv, "leak"), "{shape}");
            assert!(inv.unreadable.is_empty(), "{shape}");
        }
        // No config.json at all, or a settings.json whose enabledPlugins isn't an object.
        std::fs::remove_file(r.copilot_dir.join("config.json")).unwrap();
        assert!(!has_server(&inventory(&r, &[], &found), "leak"));
        plugins_config(
            &r,
            &serde_json::json!([{"name": "x", "marketplace": "mk", "enabled": true}]),
        );
        for text in [
            r#"{"enabledPlugins": "x@mk"}"#,
            r#"{"enabledPlugins": ["x@mk"]}"#,
            "[1]",
        ] {
            put(&r.copilot_dir.join("settings.json"), text);
            assert!(has_server(&inventory(&r, &[], &found), "leak"), "{text}");
        }
    }

    #[test]
    fn a_config_json_that_doesnt_parse_is_reported_once_though_two_fields_are_read() {
        let h = tempfile::tempdir().unwrap();
        let r = roots(h.path());
        let project = h.path().join("proj");
        put(&project.join(".mcp.json"), r#"{"mcpServers": {"p": {"command": "c"}}}"#);
        let config = r.copilot_dir.join("config.json");
        put(&config, "{\n  \"token\": \"SECRET-6N3\" oops\n}\n");

        let inv = inventory(&r, std::slice::from_ref(&project), &found);

        assert_eq!(inv.unreadable.iter().filter(|(p, _)| *p == config).count(), 1);
        assert_eq!(inv.unreadable.len(), 1);
        assert!(!format!("{inv:?}").contains("SECRET-6N3"));
        assert!(inv.servers.iter().all(|s| !matches!(s.origin, Origin::Plugin(_))));
    }
}
