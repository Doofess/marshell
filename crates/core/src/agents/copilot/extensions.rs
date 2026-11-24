//! Copilot CLI's MCP servers for the Doctor: where it keeps them, how each is turned off, and which of a project's
//! it starts. The reader takes its roots as arguments and never reads the environment.
//!
//! Copilot's `config.json` may hold account tokens. Only the one key asked for is taken out of it, the file's parsed
//! value is dropped at once, and nothing of it is kept in a type that prints, put in an item or logged.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::jsonc::read_value;
use crate::config::preview::{MAX_CONFIG_BYTES, TOO_LARGE_DETAIL};
use crate::doctor::ext::{
    Caps, CommandState, CostRule, Expand, ExtRoots, Hints, Inherit, Inventory, Launch, Listing, NoSwitch, NotMeasured,
    Origin, Server, Switch, Transport, UsedAs, unreadable,
};

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

    fn fail(&mut self, path: &Path, message: String) -> Option<Value> {
        self.unreadable.push((path.to_path_buf(), message));
        None
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
}

const DISABLED_SERVERS: &str = "/disabledMcpServers";

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

/// `command` judges a stdio server's command. `projects` are the folders to read project files in. The servers come
/// from `mcp-config.json` and each project's `.mcp.json`, plus the GitHub server Copilot loads itself.
pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory {
    let mut files = Files::default();
    let mut servers = Vec::new();
    let settings_file = roots.copilot_dir.join("settings.json");
    let settings = Settings {
        disabled: files.disabled(&settings_file, DISABLED_SERVERS),
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

    // Projects. `trustedFolders` is read once, when a project has a server to start.
    let mut trusted_list: Option<Vec<String>> = None;
    for project in projects {
        let mcp_file = project.join(".mcp.json");
        let mcp = files.json(&mcp_file);
        let map = mcp.as_ref().map(project_map);
        if !map.and_then(Value::as_object).is_some_and(|m| !m.is_empty()) {
            continue;
        }
        let mut extra_off = Vec::new();
        for name in ["settings.json", "settings.local.json"] {
            let file = project.join(".github").join("copilot").join(name);
            extra_off.extend(strings(
                files.json(&file).as_ref().and_then(|v| v.get("disabledMcpServers")),
            ));
        }
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

    Inventory {
        agent: "copilot",
        distro: None,
        found: true,
        caps: Caps::default(),
        servers,
        skills: Vec::new(),
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
    }
}
