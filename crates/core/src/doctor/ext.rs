//! The Doctor's MCP-server and skill items: each CLI's inventory in, report items out. No I/O except `skill_files`.
//! This module knows no CLI's file names or rules; those live in the readers under `agents/<cli>/extensions.rs`.

use std::path::{Path, PathBuf};

use marshell_protocol::doctor::{DoctorGroup, DoctorItem};

use super::checks::display_name;
use crate::config::hash::hash_bytes;

const ORDER: [&str; 4] = ["claude", "codex", "antigravity", "qwen"];
const FIX_NOTE: &str = " The fix turns it off and doesn't delete it.";

/// Where the CLI's files are, as the server resolved them. A reader never reads the environment.
pub struct ExtRoots {
    /// The user home (UNC in a distro).
    pub home: PathBuf,
    /// CLAUDE_CONFIG_DIR or `<home>/.claude`.
    pub claude_dir: PathBuf,
    /// `<CLAUDE_CONFIG_DIR>/.claude.json` or `<home>/.claude.json`.
    pub claude_json: PathBuf,
    /// AppState's claude_settings_path.
    pub claude_settings: PathBuf,
    /// CODEX_HOME or `<home>/.codex`.
    pub codex_home: PathBuf,
    /// QWEN_HOME or `<home>/.qwen`.
    pub qwen_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    User,
    Project(PathBuf),
    Plugin(String),
    Bundled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// Adds `name` to the JSON array at `pointer` in `file`; a missing file or key starts an empty array.
    JsonAdd {
        file: PathBuf,
        pointer: String,
        name: String,
    },
    /// Sets `pointer` in `file` to the JSON text `value`.
    JsonSet {
        file: PathBuf,
        pointer: String,
        value: String,
    },
    /// Codex: `[mcp_servers.<name>] enabled = false` in `file`.
    CodexServer { file: PathBuf, name: String },
    /// Codex: a `[[skills.config]]` table `{ path = "<skill>", enabled = false }` in `file`.
    CodexSkill { file: PathBuf, skill: PathBuf },
}

/// Why an item has no switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSwitch {
    ClaudeUser,
    Shared(PathBuf),
    Plugin(String),
    NoSkillSwitch,
    Bundled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandState {
    Found,
    Missing,
    NotChecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Stdio { command: String, state: CommandState },
    Remote,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub file: PathBuf,
    pub origin: Origin,
    pub transport: Transport,
    pub off: bool,
    pub switch: Result<Switch, NoSwitch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillState {
    Ok,
    NoBlock,
    Missing(&'static str),
    BadName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    /// The skill's SKILL.md.
    pub file: PathBuf,
    pub origin: Origin,
    pub state: SkillState,
    /// What the CLI lists besides the name: `description` (+ `when_to_use` for Claude Code), in characters.
    pub listed_chars: usize,
    /// The description's first line, in characters (Qwen Code's cut).
    pub first_line_chars: usize,
    /// Listed by name only (Claude Code's `"name-only"`).
    pub name_only: bool,
    pub off: bool,
    pub hidden: bool,
    pub switch: Result<Switch, NoSwitch>,
}

/// What a CLI cuts. Set by its reader.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Caps {
    pub per_skill: Option<usize>,
    pub listing_first_line_over: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub agent: &'static str,
    pub distro: Option<String>,
    /// The CLI is on this machine; an inventory of a CLI that isn't gives no item.
    pub found: bool,
    pub caps: Caps,
    pub servers: Vec<Server>,
    pub skills: Vec<Skill>,
    /// Config files that exist and don't parse: (file, message).
    pub unreadable: Vec<(PathBuf, String)>,
}

/// The first 8 hex digits of `hash_bytes("<file>\n<name>")`, with the file as `display()` prints it.
pub fn key(file: &Path, name: &str) -> String {
    hash_bytes(format!("{}\n{name}", file.display()).as_bytes())[..8].to_string()
}

/// The report's items for these inventories.
pub fn items(inv: &[Inventory]) -> Vec<DoctorItem> {
    build(inv).into_iter().map(|(item, _)| item).collect()
}

/// The switch of the fixable item `id` in these inventories, or None.
pub fn switch_of(inv: &[Inventory], id: &str) -> Option<Switch> {
    build(inv)
        .into_iter()
        .find(|(item, _)| item.fixable && item.id == id)
        .and_then(|(_, sw)| sw)
}

/// Every `<dir>/<folder>/SKILL.md`, sorted by folder name. A missing dir is empty. Folders starting with `.` are
/// left to the readers.
pub fn skill_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let file = e.path().join("SKILL.md");
            (!name.starts_with('.') && e.path().is_dir() && file.is_file()).then_some((name, file))
        })
        .collect();
    out.sort();
    out
}

fn rank(agent: &str) -> usize {
    ORDER.iter().position(|a| *a == agent).unwrap_or(ORDER.len())
}

fn build(inv: &[Inventory]) -> Vec<(DoctorItem, Option<Switch>)> {
    let mut sorted: Vec<&Inventory> = inv.iter().collect();
    sorted.sort_by(|a, b| (&a.distro, rank(a.agent)).cmp(&(&b.distro, rank(b.agent))));
    let mut out = Vec::new();
    for i in sorted.into_iter().filter(|i| i.found) {
        inventory_items(i, &mut out);
    }
    out
}

fn tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

fn cannot_fix_of(no: &NoSwitch, cli: &str, server_name: Option<&str>) -> String {
    match no {
        NoSwitch::ClaudeUser => format!(
            "Claude Code can turn a user-level server off only per project. To remove it, run claude mcp remove {} -s user.",
            server_name.unwrap_or("<name>")
        ),
        NoSwitch::Shared(f) => format!(
            "Only {}, which the project shares, can turn it off. Marshell doesn't edit a project's shared files.",
            f.display()
        ),
        NoSwitch::Plugin(p) => format!("It comes from the plugin {p}, whose next update would undo a change."),
        NoSwitch::NoSkillSwitch => format!("{cli} has no switch for one skill."),
        NoSwitch::Bundled => format!("It comes with {cli}."),
    }
}

fn inventory_items(inv: &Inventory, out: &mut Vec<(DoctorItem, Option<Switch>)>) {
    let cli = display_name(inv.agent);
    let suffix = inv.distro.as_ref().map(|d| format!("@{d}")).unwrap_or_default();
    let place = inv.distro.as_ref().map(|d| format!(" in {d}")).unwrap_or_default();
    let mk = |id: String, group: DoctorGroup, title: String, detail: String, path: Option<&Path>| DoctorItem {
        id,
        agent: Some(inv.agent.to_string()),
        distro: inv.distro.clone(),
        group,
        title,
        detail,
        fixable: false,
        cannot_fix: None,
        path: path.map(|p| p.display().to_string()),
        open_agents: false,
    };

    // 1. Files that don't parse.
    let mut unreadable: Vec<&(PathBuf, String)> = inv.unreadable.iter().collect();
    unreadable.sort();
    for (file, message) in unreadable {
        let file_name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push((
            mk(
                format!("extfile.{}.{}{suffix}", inv.agent, key(file, "")),
                DoctorGroup::Info,
                format!("{cli}'s {file_name}{place} can't be read"),
                format!("{message} Its MCP servers and skills weren't checked."),
                Some(file),
            ),
            None,
        ));
    }

    // 2. Servers that can't start.
    let mut servers: Vec<&Server> = inv
        .servers
        .iter()
        .filter(|s| !s.off && s.origin != Origin::Bundled)
        .collect();
    servers.sort_by(|a, b| (&a.name, &a.file).cmp(&(&b.name, &b.file)));
    for s in servers {
        let why = match &s.transport {
            Transport::Stdio {
                command,
                state: CommandState::Missing,
            } => {
                format!("Its command, {command}, isn't on this PC.")
            }
            Transport::Nothing => "Its entry has no command and no URL.".to_string(),
            _ => continue,
        };
        let mut item = mk(
            format!("mcp.{}.{}{suffix}", inv.agent, key(&s.file, &s.name)),
            DoctorGroup::Broken,
            format!("{cli}'s MCP server {}{place} can't start", s.name),
            why,
            Some(&s.file),
        );
        let switch = fixability(&mut item, &s.switch, cli, Some(&s.name));
        out.push((item, switch));
    }

    // 3. Skills that don't load.
    let mut skills: Vec<&Skill> = inv
        .skills
        .iter()
        .filter(|s| !s.off && s.origin != Origin::Bundled)
        .collect();
    skills.sort_by(|a, b| (&a.name, &a.file).cmp(&(&b.name, &b.file)));
    for s in &skills {
        let why = match &s.state {
            SkillState::Ok => continue,
            SkillState::NoBlock => "Its SKILL.md doesn't start with a --- block that closes.".to_string(),
            SkillState::Missing(field) => format!("Its SKILL.md has no {field}."),
            SkillState::BadName => format!("Its name has characters {cli} refuses."),
        };
        let mut item = mk(
            format!("skill.{}.{}{suffix}", inv.agent, key(&s.file, &s.name)),
            DoctorGroup::Broken,
            format!("{cli}'s skill {}{place} doesn't load", s.name),
            why,
            Some(&s.file),
        );
        let switch = fixability(&mut item, &s.switch, cli, None);
        out.push((item, switch));
    }

    // 4. Descriptions the CLI cuts.
    if let Some(cap) = inv.caps.per_skill {
        for s in skills
            .iter()
            .filter(|s| !s.hidden && !s.name_only && s.listed_chars > cap)
        {
            let mut item = mk(
                format!("skilldesc.{}.{}{suffix}", inv.agent, key(&s.file, &s.name)),
                DoctorGroup::Waste,
                format!("{cli}'s skill {}{place} has a description {cli} cuts", s.name),
                format!("It is {} characters, and {cli} keeps the first {cap}.", s.listed_chars),
                Some(&s.file),
            );
            item.cannot_fix = Some("Shorten the description in its SKILL.md.".into());
            out.push((item, None));
        }
    }

    // 5 and 6. The listing cut and the cost, over the skills that are listed. A skill that doesn't load isn't listed.
    let listed: Vec<&Skill> = inv
        .skills
        .iter()
        .filter(|s| !s.off && !s.hidden && s.state == SkillState::Ok)
        .collect();
    let full_chars = |s: &Skill| -> usize {
        let desc = if s.name_only {
            0
        } else {
            inv.caps.per_skill.map_or(s.listed_chars, |c| s.listed_chars.min(c))
        };
        s.name.chars().count() + desc
    };
    let uncut_total: usize = listed
        .iter()
        .filter(|s| s.origin != Origin::Bundled)
        .map(|s| full_chars(s))
        .sum();
    let cut = inv.caps.listing_first_line_over.filter(|cap| uncut_total > *cap);
    if let Some(cap) = cut {
        let mut item = mk(
            format!("skilllisting.{}{suffix}", inv.agent),
            DoctorGroup::Waste,
            format!("{cli}{place} cuts its skills' descriptions to their first line"),
            format!("Its skill listing is {uncut_total} characters, over the {cap} it keeps in full."),
            None,
        );
        item.cannot_fix = Some("Shorten the longest descriptions, or turn off skills you don't use.".into());
        out.push((item, None));
    }
    if !listed.is_empty() {
        let counted = |s: &Skill| -> usize {
            if cut.is_some() && s.origin != Origin::Bundled && !s.name_only {
                s.name.chars().count() + s.first_line_chars
            } else {
                full_chars(s)
            }
        };
        let total: usize = listed.iter().map(|s| counted(s)).sum();
        let mut largest: Vec<(usize, &str)> = listed.iter().map(|s| (counted(s), s.name.as_str())).collect();
        largest.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let top: Vec<String> = largest
            .iter()
            .take(3)
            .map(|(c, n)| format!("{n} ({})", tokens(*c)))
            .collect();
        out.push((
            mk(
                format!("skillcost.{}{suffix}", inv.agent),
                DoctorGroup::Info,
                format!(
                    "{cli}'s skills{place} add about {} tokens to each session",
                    tokens(total)
                ),
                format!(
                    "{} listed, by name and description. The largest: {}.",
                    if listed.len() == 1 {
                        "1 skill is".to_string()
                    } else {
                        format!("{} skills are", listed.len())
                    },
                    top.join(", ")
                ),
                None,
            ),
            None,
        ));
    }
}

/// Sets `fixable` and `cannot_fix` (and the fix note) on a Broken item from its switch.
fn fixability(
    item: &mut DoctorItem,
    switch: &Result<Switch, NoSwitch>,
    cli: &str,
    server_name: Option<&str>,
) -> Option<Switch> {
    match switch {
        Ok(sw) => {
            item.fixable = true;
            item.detail.push_str(FIX_NOTE);
            Some(sw.clone())
        }
        Err(no) => {
            item.cannot_fix = Some(cannot_fix_of(no, cli, server_name));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::doctor::DoctorGroup;

    fn inv(agent: &'static str) -> Inventory {
        Inventory {
            agent,
            distro: None,
            found: true,
            caps: Caps::default(),
            servers: vec![],
            skills: vec![],
            unreadable: vec![],
        }
    }

    fn server(name: &str, file: &str, transport: Transport, switch: Result<Switch, NoSwitch>) -> Server {
        Server {
            name: name.into(),
            file: PathBuf::from(file),
            origin: Origin::User,
            transport,
            off: false,
            switch,
        }
    }

    fn missing(command: &str) -> Transport {
        Transport::Stdio {
            command: command.into(),
            state: CommandState::Missing,
        }
    }

    fn skill(name: &str, listed: usize) -> Skill {
        Skill {
            name: name.into(),
            file: PathBuf::from(format!("C:/h/skills/{name}/SKILL.md")),
            origin: Origin::User,
            state: SkillState::Ok,
            listed_chars: listed,
            first_line_chars: listed.min(10),
            name_only: false,
            off: false,
            hidden: false,
            switch: Err(NoSwitch::NoSkillSwitch),
        }
    }

    fn with_prefix<'a>(items: &'a [DoctorItem], prefix: &str) -> Vec<&'a DoctorItem> {
        items.iter().filter(|i| i.id.starts_with(prefix)).collect()
    }

    #[test]
    fn missing_command_is_broken_and_fixable_with_switch() {
        let file = PathBuf::from("C:/h/.codex/config.toml");
        let mut i = inv("codex");
        i.servers.push(server(
            "files",
            "C:/h/.codex/config.toml",
            missing("npx-x"),
            Ok(Switch::CodexServer {
                file: file.clone(),
                name: "files".into(),
            }),
        ));
        let items = items(&[i]);
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id, format!("mcp.codex.{}", key(&file, "files")));
        assert_eq!(it.agent.as_deref(), Some("codex"));
        assert_eq!(it.group, DoctorGroup::Broken);
        assert!(it.fixable);
        assert_eq!(it.title, "Codex's MCP server files can't start");
        assert_eq!(
            it.detail,
            "Its command, npx-x, isn't on this PC. The fix turns it off and doesn't delete it."
        );
        assert_eq!(it.cannot_fix, None);
        assert_eq!(it.path.as_deref(), Some(file.display().to_string().as_str()));
    }

    #[test]
    fn an_entry_with_nothing_to_run_is_broken() {
        let mut i = inv("qwen");
        i.servers.push(server(
            "empty",
            "C:/h/.qwen/settings.json",
            Transport::Nothing,
            Err(NoSwitch::Shared(PathBuf::from("C:/p/.qwen/settings.json"))),
        ));
        let items = items(&[i]);
        assert_eq!(items[0].detail, "Its entry has no command and no URL.");
        assert!(!items[0].fixable);
    }

    #[test]
    fn no_switch_reasons_are_the_cannot_fix_copy() {
        let cases = [
            (
                NoSwitch::ClaudeUser,
                "Claude Code can turn a user-level server off only per project. To remove it, run claude mcp remove s -s user.",
            ),
            (
                NoSwitch::Shared(PathBuf::from("C:/p/.mcp.json")),
                "Only C:/p/.mcp.json, which the project shares, can turn it off. Marshell doesn't edit a project's shared files.",
            ),
            (
                NoSwitch::Plugin("pdf@market".into()),
                "It comes from the plugin pdf@market, whose next update would undo a change.",
            ),
        ];
        for (no, expected) in cases {
            let mut i = inv("claude");
            i.servers.push(server("s", "C:/h/.claude.json", missing("zz"), Err(no)));
            let items = items(&[i]);
            assert_eq!(items.len(), 1);
            assert!(!items[0].fixable);
            assert_eq!(items[0].cannot_fix.as_deref(), Some(expected));
            assert_eq!(items[0].detail, "Its command, zz, isn't on this PC.");
        }
        let mut i = inv("codex");
        let mut s = skill("bad", 5);
        s.state = SkillState::NoBlock;
        i.skills.push(s);
        let items = items(&[i]);
        let broken = with_prefix(&items, "skill.");
        assert_eq!(broken.len(), 1);
        assert_eq!(
            broken[0].cannot_fix.as_deref(),
            Some("Codex has no switch for one skill.")
        );
        assert_eq!(broken[0].title, "Codex's skill bad doesn't load");
        assert_eq!(
            broken[0].detail,
            "Its SKILL.md doesn't start with a --- block that closes."
        );
    }

    #[test]
    fn skill_states_have_their_detail() {
        let mut i = inv("antigravity");
        let mut a = skill("a", 1);
        a.state = SkillState::Missing("description");
        a.switch = Ok(Switch::JsonAdd {
            file: "f".into(),
            pointer: "/x".into(),
            name: "a".into(),
        });
        let mut b = skill("b", 1);
        b.state = SkillState::BadName;
        i.skills = vec![b, a];
        let items = items(&[i]);
        let broken = with_prefix(&items, "skill.");
        assert_eq!(broken[0].title, "Antigravity's skill a doesn't load");
        assert_eq!(
            broken[0].detail,
            "Its SKILL.md has no description. The fix turns it off and doesn't delete it."
        );
        assert!(broken[0].fixable);
        assert_eq!(broken[1].detail, "Its name has characters Antigravity refuses.");
        assert_eq!(
            broken[0].path.as_deref(),
            Some(skill("a", 1).file.display().to_string().as_str())
        );
    }

    #[test]
    fn off_found_and_bundled_rules() {
        let mut i = inv("codex");
        let mut off_server = server("o", "f", missing("zz"), Err(NoSwitch::Bundled));
        off_server.off = true;
        i.servers.push(off_server);
        let mut off_skill = skill("x", 0);
        off_skill.state = SkillState::NoBlock;
        off_skill.off = true;
        i.skills.push(off_skill);
        assert!(items(&[i]).is_empty());

        let mut gone = inv("codex");
        gone.found = false;
        gone.servers
            .push(server("o", "f", missing("zz"), Err(NoSwitch::Bundled)));
        gone.unreadable.push((PathBuf::from("f"), "Bad.".into()));
        assert!(items(&[gone]).is_empty());

        let mut b = inv("codex");
        let mut sk = skill("imagegen", 0);
        sk.origin = Origin::Bundled;
        sk.state = SkillState::NoBlock;
        b.skills.push(sk);
        let mut loads = skill("pdf", 4);
        loads.origin = Origin::Bundled;
        b.skills.push(loads);
        let items = items(&[b]);
        assert!(with_prefix(&items, "skill.").is_empty());
        // A bundled skill that loads still counts toward the cost; one that doesn't load isn't listed.
        let cost = with_prefix(&items, "skillcost.");
        assert_eq!(cost.len(), 1);
        assert!(cost[0].detail.starts_with("1 skill is listed"), "{}", cost[0].detail);
    }

    #[test]
    fn long_description_uses_the_cap() {
        let mut i = inv("claude");
        i.caps.per_skill = Some(1536);
        i.skills.push(skill("long", 1537));
        i.skills.push(skill("exact", 1536));
        let mut hidden = skill("hidden", 5000);
        hidden.hidden = true;
        i.skills.push(hidden);
        let mut name_only = skill("nameonly", 5000);
        name_only.name_only = true;
        i.skills.push(name_only);
        let items = items(&[i]);
        let long = with_prefix(&items, "skilldesc.");
        assert_eq!(long.len(), 1);
        let it = long[0];
        assert_eq!(it.group, DoctorGroup::Waste);
        assert!(!it.fixable);
        assert_eq!(it.title, "Claude Code's skill long has a description Claude Code cuts");
        assert_eq!(
            it.detail,
            "It is 1537 characters, and Claude Code keeps the first 1536."
        );
        assert_eq!(
            it.cannot_fix.as_deref(),
            Some("Shorten the description in its SKILL.md.")
        );
        assert!(it.path.as_deref().unwrap().ends_with("SKILL.md"));
    }

    #[test]
    fn qwen_listing_cut() {
        let mut i = inv("qwen");
        i.caps.listing_first_line_over = Some(8000);
        i.skills.push(skill("a", 8000));
        let mut bundled = skill("bb", 100);
        bundled.origin = Origin::Bundled;
        i.skills.push(bundled);
        let items = items(&[i]);
        let l = with_prefix(&items, "skilllisting.");
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].id, "skilllisting.qwen");
        assert_eq!(l[0].group, DoctorGroup::Waste);
        assert_eq!(
            l[0].title,
            "Qwen Code cuts its skills' descriptions to their first line"
        );
        assert_eq!(
            l[0].detail,
            "Its skill listing is 8001 characters, over the 8000 it keeps in full."
        );
        assert_eq!(
            l[0].cannot_fix.as_deref(),
            Some("Shorten the longest descriptions, or turn off skills you don't use.")
        );
        assert_eq!(l[0].path, None);
        // a: 1 + 10 (first line), bb: 2 + 100 (bundled, kept whole) = 113 characters.
        let cost = with_prefix(&items, "skillcost.");
        assert_eq!(cost[0].title, "Qwen Code's skills add about 29 tokens to each session");

        // Exactly at the cap: no cut.
        let mut j = inv("qwen");
        j.caps.listing_first_line_over = Some(8000);
        j.skills.push(skill("a", 7999));
        assert!(with_prefix(&super::items(&[j]), "skilllisting.").is_empty());
    }

    #[test]
    fn cost_line() {
        let mut i = inv("claude");
        i.skills.push(skill("a", 7));
        let mut bb = skill("bb", 10);
        bb.name_only = true;
        i.skills.push(bb);
        i.skills.push(skill("ccc", 0));
        let mut off = skill("off", 1000);
        off.off = true;
        i.skills.push(off);
        let mut hidden = skill("hid", 1000);
        hidden.hidden = true;
        i.skills.push(hidden);
        let items = items(&[i]);
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.id, "skillcost.claude");
        assert_eq!(it.group, DoctorGroup::Info);
        assert!(!it.fixable);
        assert_eq!(it.path, None);
        // a: 8, bb: 2 (name only), ccc: 3 = 13 characters, 4 tokens.
        assert_eq!(it.title, "Claude Code's skills add about 4 tokens to each session");
        assert_eq!(
            it.detail,
            "3 skills are listed, by name and description. The largest: a (2), ccc (1), bb (1)."
        );
    }

    #[test]
    fn a_skill_that_does_not_load_costs_nothing() {
        let mut i = inv("codex");
        i.skills.push(skill("a", 7));
        let mut broken = skill("broken", 1000);
        broken.state = SkillState::Missing("description");
        i.skills.push(broken);
        let items = items(&[i]);
        let cost = items.iter().find(|it| it.id == "skillcost.codex").unwrap();
        // Only `a`: 8 characters, 2 tokens.
        assert_eq!(cost.title, "Codex's skills add about 2 tokens to each session");
        assert_eq!(
            cost.detail,
            "1 skill is listed, by name and description. The largest: a (2)."
        );
    }

    #[test]
    fn distro_ids_and_titles() {
        let mut i = inv("codex");
        i.distro = Some("Ubuntu".into());
        let file = PathBuf::from("//wsl$/Ubuntu/home/u/.codex/config.toml");
        i.servers.push(server(
            "x",
            "//wsl$/Ubuntu/home/u/.codex/config.toml",
            missing("zz"),
            Err(NoSwitch::Bundled),
        ));
        i.unreadable.push((file.clone(), "Bad.".into()));
        i.skills.push(skill("s", 4));
        let items = items(&[i]);
        assert_eq!(items.len(), 3);
        for it in &items {
            assert!(it.id.ends_with("@Ubuntu"), "{}", it.id);
            assert_eq!(it.distro.as_deref(), Some("Ubuntu"));
        }
        assert_eq!(items[0].title, "Codex's config.toml in Ubuntu can't be read");
        assert_eq!(items[1].title, "Codex's MCP server x in Ubuntu can't start");
        assert_eq!(
            items[2].title,
            "Codex's skills in Ubuntu add about 2 tokens to each session"
        );
        assert_eq!(items[0].id, format!("extfile.codex.{}@Ubuntu", key(&file, "")));
        assert_eq!(items[2].id, "skillcost.codex@Ubuntu");
    }

    #[test]
    fn unreadable_file_is_info() {
        let file = PathBuf::from("C:/h/.gemini/config/mcp_config.json");
        let mut i = inv("antigravity");
        i.unreadable.push((file.clone(), "Expected a value.".into()));
        let items = items(&[i]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, format!("extfile.antigravity.{}", key(&file, "")));
        assert_eq!(items[0].group, DoctorGroup::Info);
        assert!(!items[0].fixable);
        assert_eq!(items[0].cannot_fix, None);
        assert_eq!(items[0].title, "Antigravity's mcp_config.json can't be read");
        assert_eq!(
            items[0].detail,
            "Expected a value. Its MCP servers and skills weren't checked."
        );
        assert_eq!(items[0].path.as_deref(), Some(file.display().to_string().as_str()));
    }

    #[test]
    fn order_is_distro_then_cli_then_kind() {
        let mut qwen = inv("qwen");
        qwen.unreadable.push((PathBuf::from("q"), "Bad.".into()));
        let mut claude = inv("claude");
        claude.skills.push(skill("s", 4));
        claude
            .servers
            .push(server("z", "f", missing("zz"), Err(NoSwitch::ClaudeUser)));
        claude
            .servers
            .push(server("a", "f", missing("zz"), Err(NoSwitch::ClaudeUser)));
        let mut wsl = inv("claude");
        wsl.distro = Some("Debian".into());
        wsl.unreadable.push((PathBuf::from("w"), "Bad.".into()));
        let ids: Vec<String> = items(&[wsl, qwen, claude]).into_iter().map(|i| i.title).collect();
        assert_eq!(
            ids,
            [
                "Claude Code's MCP server a can't start",
                "Claude Code's MCP server z can't start",
                "Claude Code's skills add about 2 tokens to each session",
                "Qwen Code's q can't be read",
                "Claude Code's w in Debian can't be read",
            ]
        );
    }

    #[test]
    fn switch_of_finds_only_fixable_items() {
        let file = PathBuf::from("C:/h/.codex/config.toml");
        let sw = Switch::CodexServer {
            file: file.clone(),
            name: "files".into(),
        };
        let mut i = inv("codex");
        i.servers.push(server(
            "files",
            "C:/h/.codex/config.toml",
            missing("zz"),
            Ok(sw.clone()),
        ));
        i.servers.push(server(
            "other",
            "C:/h/.codex/config.toml",
            missing("zz"),
            Err(NoSwitch::Bundled),
        ));
        let mut s = skill("sk", 3);
        s.state = SkillState::NoBlock;
        let skill_sw = Switch::CodexSkill {
            file: file.clone(),
            skill: PathBuf::from("C:/h/skills/sk"),
        };
        s.switch = Ok(skill_sw.clone());
        i.skills.push(s);
        let inv_all = [i];
        let ids: Vec<String> = items(&inv_all).into_iter().map(|i| i.id).collect();
        let server_id = format!("mcp.codex.{}", key(&file, "files"));
        let other_id = format!("mcp.codex.{}", key(&file, "other"));
        assert!(ids.contains(&server_id) && ids.contains(&other_id));
        assert_eq!(switch_of(&inv_all, &server_id), Some(sw));
        assert_eq!(switch_of(&inv_all, &other_id), None);
        assert_eq!(switch_of(&inv_all, "mcp.codex.00000000"), None);
        let skill_id = ids.iter().find(|i| i.starts_with("skill.codex.")).unwrap();
        assert_eq!(switch_of(&inv_all, skill_id), Some(skill_sw));
    }

    #[test]
    fn key_is_stable() {
        let f = Path::new("C:/h/.codex/config.toml");
        let a = key(f, "files");
        assert_eq!(a.len(), 8);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a, key(f, "files"));
        assert_ne!(a, key(f, "other"));
        assert_ne!(a, key(Path::new("C:/h/other.toml"), "files"));
    }

    #[test]
    fn skill_files_lists_folders_with_a_skill_md() {
        let dir = std::env::temp_dir().join(format!("marshell-ext-skill-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (folder, has) in [("b", true), ("a", true), ("empty", false), (".system", true)] {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
            if has {
                std::fs::write(dir.join(folder).join("SKILL.md"), "x").unwrap();
            }
        }
        let got: Vec<String> = skill_files(&dir).into_iter().map(|(n, _)| n).collect();
        assert_eq!(got, ["a", "b"]);
        assert!(skill_files(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
