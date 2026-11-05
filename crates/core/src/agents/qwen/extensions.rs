//! Qwen Code's MCP servers and skills for the Doctor: where it keeps them, how each is turned off, and what its
//! skill listing cuts. The readers take their roots as arguments and never read the environment.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::jsonc::{EditError, read_value};
use crate::doctor::ext::{
    Caps, CommandState, ExtRoots, Inventory, NoSwitch, Origin, Server, Skill, SkillState, Switch, Transport,
    skill_files,
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
                    Err(e) => self.fail(path, edit_message(e)),
                },
                Err(_) => self.fail(path, "it isn't UTF-8 text".to_string()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => self.fail(path, e.to_string()),
        };
        self.cache.insert(path.to_path_buf(), value.clone());
        value
    }

    fn fail(&mut self, path: &Path, message: String) -> Option<Value> {
        self.unreadable.push((path.to_path_buf(), message));
        None
    }
}

fn edit_message(e: EditError) -> String {
    match e {
        EditError::DoesNotParse(m) => m,
        other => format!("{other:?}"),
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
}

fn servers_in(
    map: Option<&Value>,
    at: Source,
    command: &dyn Fn(&str) -> CommandState,
    excluded: &[String],
    user_settings: &Path,
    out: &mut Vec<Server>,
) {
    let Some(map) = map.and_then(Value::as_object) else {
        return;
    };
    for (name, entry) in map {
        out.push(Server {
            name: name.clone(),
            file: at.file.to_path_buf(),
            origin: at.origin.clone(),
            transport: transport(entry, command),
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

    // User level.
    servers_in(
        user.as_ref().and_then(|u| u.get("mcpServers")),
        Source {
            file: &user_file,
            origin: Origin::User,
            switchable: true,
        },
        command,
        &user_excluded,
        &user_file,
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
            },
            command,
            &user_excluded,
            &user_file,
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
            },
            command,
            &excluded,
            &user_file,
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
            },
            command,
            &excluded,
            &user_file,
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
}
