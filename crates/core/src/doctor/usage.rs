//! What each CLI used and when, for the Doctor's "not used in 30 days" and "used in one project only" items. This
//! module knows no CLI's file names or record shapes: the readers under `agents/<cli>/` turn a history into `Scan`s,
//! and everything here is pure except `new_lines`, which reads the new part of one file.
//!
//! The rule that runs through it: a use the scan can't attribute must never make an item "unused", and coverage after a
//! gap restarts from what is on disk, never from what was lost.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use marshell_protocol::doctor::{DoctorGroup, DoctorItem};
use serde::{Deserialize, Serialize};

use super::checks::display_name;
use super::ext::{Inventory, NotMeasured, Origin, SkillState, SkillUse, Switch, UsedAs, fixability, item_key};

/// "30 days" for every usage rule.
pub const WINDOW_MS: u64 = 30 * 24 * 60 * 60 * 1000;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// What a record names as used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UseKey {
    /// A tool call, by the tool's full name.
    Tool(String),
    /// A call to a server, by the server's name.
    Server(String),
    /// A skill, by the name the history gives (any case).
    SkillName(String),
    /// A skill, by its `SKILL.md` path (any form; `norm_path` is applied).
    SkillFile(String),
}

impl UseKey {
    /// The key's text form in `AgentStore::used` and `available`.
    pub fn text(&self) -> String {
        match self {
            UseKey::Tool(n) => format!("tool:{n}"),
            UseKey::Server(n) => format!("server:{n}"),
            UseKey::SkillName(n) => format!("skill:{}", n.to_lowercase()),
            UseKey::SkillFile(p) => format!("file:{}", norm_path(p)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// The key was used.
    Used,
    /// The CLI's history listed the key as there (a session's deferred tools or skill listing).
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub key: UseKey,
    pub at_ms: u64,
    /// Where the CLI was running, if the record says.
    pub cwd: Option<String>,
}

/// How far one history file has been read.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Cursor {
    /// The file's length when it was last read.
    pub len: u64,
    /// The byte offset after the last complete line read.
    pub read_to: u64,
    /// The time of the file's first record.
    pub first_ms: Option<u64>,
    /// The folder the file's session ran in, once a record has said.
    pub cwd: Option<String>,
}

/// What one scan of one CLI found: the events in new records, every history file present with its cursor, and whether
/// the history could be read at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub events: Vec<Event>,
    pub cursors: BTreeMap<String, Cursor>,
    pub unreadable: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Use {
    pub last_ms: u64,
    /// `norm_path(cwd)`, or `""` when unknown, to the last use there.
    pub places: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct AgentStore {
    /// The start of the history this store covers.
    pub oldest_ms: Option<u64>,
    /// When the last readable scan ran.
    pub scanned_ms: Option<u64>,
    pub cursors: BTreeMap<String, Cursor>,
    pub used: BTreeMap<String, Use>,
    pub available: BTreeMap<String, u64>,
    pub unreadable: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct UsageStore {
    pub agents: BTreeMap<String, AgentStore>,
    /// `<agent>:<item key>` to when Marshell first saw the item.
    pub first_sight: BTreeMap<String, u64>,
}

impl crate::store::Versioned for UsageStore {
    const VERSION: u32 = 1;
}

/// Lowercase, `\` to `/`, `\\?\` and a trailing `/` removed.
pub fn norm_path(p: &str) -> String {
    let lower = p.to_lowercase().replace('\\', "/");
    let bare = lower.strip_prefix("//?/").unwrap_or(&lower);
    bare.trim_end_matches('/').to_string()
}

fn drive_at(b: &[u8], i: usize) -> bool {
    i + 2 < b.len()
        && b[i].is_ascii_alphabetic()
        && b[i + 1] == b':'
        && (b[i + 2] == b'\\' || b[i + 2] == b'/')
        && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
}

/// The absolute `SKILL.md` paths in a command or argument text, normalised. A path starts at the nearest quote before
/// it when that quote comes after the last drive pattern, else at the nearest drive pattern; a `SKILL.md` with no drive
/// before it gives nothing.
pub fn skill_md_paths(text: &str) -> Vec<String> {
    const NAME: &str = "skill.md";
    let lower = text.to_ascii_lowercase();
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(NAME) {
        let at = from + rel;
        from = at + NAME.len();
        // `SKILL.md` as the last part of a path, not a word in a sentence.
        if at == 0 || !matches!(b[at - 1], b'\\' | b'/') {
            continue;
        }
        let last_drive = (0..at).rev().find(|&i| drive_at(b, i));
        let Some(drive) = last_drive else { continue };
        let quote = (0..at).rev().find(|&i| matches!(b[i], b'\'' | b'"' | b'`'));
        let start = match quote {
            Some(q) if q > drive => q + 1,
            _ => drive,
        };
        let path = &text[start..from];
        // Only an absolute path counts: a drive letter, after an optional `\\?\`.
        let rest = path
            .strip_prefix("\\\\?\\")
            .or_else(|| path.strip_prefix("//?/"))
            .unwrap_or(path);
        if drive_at(rest.as_bytes(), 0) {
            out.push(norm_path(path));
        }
    }
    out
}

/// The new complete lines of `file` after `cursor`. A file shorter than `cursor.read_to` is read from the start.
/// Returns the lines and the new cursor (`len`, `read_to` at the end of the last whole line; `first_ms` and `cwd` kept).
pub fn new_lines(file: &Path, cursor: Option<&Cursor>) -> std::io::Result<(Vec<String>, Cursor)> {
    let mut f = std::fs::File::open(file)?;
    let now_len = f.metadata()?.len();
    let old = cursor.cloned().unwrap_or_default();
    let start = if now_len < old.read_to { 0 } else { old.read_to };
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let whole = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    // `split` leaves an empty piece after the last newline.
    let pieces: Vec<&[u8]> = buf[..whole].split(|b| *b == b'\n').collect();
    let lines: Vec<String> = pieces[..pieces.len().saturating_sub(1)]
        .iter()
        .map(|l| String::from_utf8_lossy(l.strip_suffix(b"\r").unwrap_or(l)).into_owned())
        .collect();
    let next = Cursor {
        len: start + buf.len() as u64,
        read_to: start + whole as u64,
        first_ms: old.first_ms,
        cwd: old.cwd,
    };
    Ok((lines, next))
}

/// Folds one scan into the store.
pub fn merge(store: &mut AgentStore, scan: Scan, now_ms: u64) {
    let limit = now_ms.saturating_add(DAY_MS);
    for ev in scan.events {
        if ev.at_ms > limit {
            continue;
        }
        let key = ev.key.text();
        match ev.kind {
            EventKind::Used => {
                let u = store.used.entry(key).or_default();
                u.last_ms = u.last_ms.max(ev.at_ms);
                let place = ev.cwd.as_deref().map(norm_path).unwrap_or_default();
                let p = u.places.entry(place).or_insert(0);
                *p = (*p).max(ev.at_ms);
            }
            EventKind::Available => {
                let a = store.available.entry(key).or_insert(ev.at_ms);
                *a = (*a).min(ev.at_ms);
            }
        }
    }
    // A scan that couldn't read the history says nothing about what is on disk, so coverage waits for the next one.
    if scan.unreadable.is_none() {
        let disk = scan.cursors.values().filter_map(|c| c.first_ms).min();
        store.oldest_ms = match (store.scanned_ms, disk) {
            // Nothing on disk: whatever was covered is gone.
            (_, None) => None,
            // Older files may have been deleted unseen since the last scan.
            (Some(s), Some(d)) if d > s => Some(d),
            (_, Some(d)) => Some(store.oldest_ms.map_or(d, |o| o.min(d))),
        };
        store.scanned_ms = Some(now_ms);
    }
    store.cursors = scan.cursors;
    store.unreadable = scan.unreadable;
    let cutoff = now_ms.saturating_sub(WINDOW_MS);
    for u in store.used.values_mut() {
        u.places.retain(|_, t| *t >= cutoff);
    }
}

/// Joins two stores written from the same snapshot by runs that overlapped.
pub fn absorb(into: &mut UsageStore, other: &UsageStore) {
    for (agent, o) in &other.agents {
        let e = into.agents.entry(agent.clone()).or_default();
        // The later scan gives the coverage; on a tie, the one that covers less.
        if (o.scanned_ms, o.oldest_ms) > (e.scanned_ms, e.oldest_ms) {
            e.oldest_ms = o.oldest_ms;
            e.cursors = o.cursors.clone();
            e.unreadable = o.unreadable.clone();
        }
        e.scanned_ms = e.scanned_ms.max(o.scanned_ms);
        for (k, ou) in &o.used {
            let u = e.used.entry(k.clone()).or_default();
            u.last_ms = u.last_ms.max(ou.last_ms);
            for (place, t) in &ou.places {
                let p = u.places.entry(place.clone()).or_insert(*t);
                *p = (*p).max(*t);
            }
        }
        for (k, t) in &o.available {
            let a = e.available.entry(k.clone()).or_insert(*t);
            *a = (*a).min(*t);
        }
    }
    for (k, t) in &other.first_sight {
        let f = into.first_sight.entry(k.clone()).or_insert(*t);
        *f = (*f).min(*t);
    }
}

/// What the store holds for one item.
#[derive(Default)]
struct Hits {
    last: Option<u64>,
    places: BTreeMap<String, u64>,
    available: Option<u64>,
}

fn hits(store: &AgentStore, matches: &dyn Fn(&str) -> bool) -> Hits {
    let mut h = Hits::default();
    for (k, u) in &store.used {
        if matches(k) {
            h.last = h.last.max(Some(u.last_ms));
            for (place, t) in &u.places {
                let p = h.places.entry(place.clone()).or_insert(*t);
                *p = (*p).max(*t);
            }
        }
    }
    for (k, t) in &store.available {
        if matches(k) {
            h.available = Some(h.available.map_or(*t, |a| a.min(*t)));
        }
    }
    h
}

fn server_matches(used_as: &UsedAs) -> Box<dyn Fn(&str) -> bool + '_> {
    match used_as {
        UsedAs::ToolPrefix(p) => Box::new(move |k| k.strip_prefix("tool:").is_some_and(|n| n.starts_with(p.as_str()))),
        UsedAs::Server(n) => {
            let want = format!("server:{n}");
            Box::new(move |k| k == want)
        }
        UsedAs::Unknown => Box::new(|_| false),
    }
}

fn skill_matches(used_as: &[SkillUse]) -> impl Fn(&str) -> bool + '_ {
    move |k| {
        used_as.iter().any(|u| match u {
            SkillUse::File(f) => k == format!("file:{}", norm_path(&f.to_string_lossy())),
            SkillUse::Name(n) => {
                let Some(rest) = k.strip_prefix("skill:") else {
                    return false;
                };
                let (rest, n) = (rest.to_lowercase(), n.to_lowercase());
                if rest == n {
                    return true;
                }
                // A worktree-scoped name: `<folder>:<skill>`.
                let bare = n.rsplit(':').next().unwrap_or(&n);
                rest.rsplit_once(':')
                    .is_some_and(|(x, b)| (x.contains('/') || x.contains('\\')) && b == bare)
            }
        })
    }
}

/// The longest project that holds `place`, else `place` itself.
fn project_of(place: &str, projects: &[PathBuf]) -> (String, Option<PathBuf>) {
    let mut best: Option<(&PathBuf, String)> = None;
    for p in projects {
        let n = norm_path(&p.to_string_lossy());
        let holds = !n.is_empty() && (place == n || place.strip_prefix(n.as_str()).is_some_and(|r| r.starts_with('/')));
        if holds && best.as_ref().is_none_or(|(_, b)| n.len() > b.len()) {
            best = Some((p, n));
        }
    }
    match best {
        Some((p, n)) => (n, Some(p.clone())),
        None => (place.to_string(), None),
    }
}

/// The usage items of one Windows inventory: the history that can't be read, the coverage so far, the servers and
/// skills not used in 30 days, and the user-level ones used in one project only. `broken` says whether an item key
/// already has a Broken item, which then is the one verdict.
pub(crate) fn items(
    inv: &Inventory,
    store: Option<&UsageStore>,
    broken: &dyn Fn(&str) -> bool,
    projects: &[PathBuf],
    now_ms: u64,
    out: &mut Vec<(DoctorItem, Option<Switch>)>,
) {
    let (Some(store), None, true) = (store, &inv.distro, inv.found) else {
        return;
    };
    let cli = display_name(inv.agent);
    let agent = store.agents.get(inv.agent).cloned().unwrap_or_default();
    let mk = |id: String, group: DoctorGroup, title: String, detail: String, path: Option<String>| DoctorItem {
        id,
        agent: Some(inv.agent.to_string()),
        distro: None,
        group,
        title,
        detail,
        fixable: false,
        cannot_fix: None,
        path,
        open_agents: false,
        unchecked: false,
    };

    if let Some(why) = &agent.unreadable {
        out.push((
            mk(
                format!("usage.{}", inv.agent),
                DoctorGroup::Info,
                format!("{cli}'s history can't be read"),
                format!("{why} So its servers and skills aren't called unused."),
                None,
            ),
            None,
        ));
        return;
    }

    let mut servers: Vec<_> = inv
        .servers
        .iter()
        .filter(|s| !s.off && s.origin != Origin::Bundled)
        .collect();
    servers.sort_by(|a, b| (&a.name, &a.file).cmp(&(&b.name, &b.file)));
    let mut skills: Vec<_> = inv
        .skills
        .iter()
        .filter(|s| !s.off && s.origin != Origin::Bundled)
        .collect();
    skills.sort_by(|a, b| (&a.name, &a.file).cmp(&(&b.name, &b.file)));

    let judged_servers: Vec<_> = servers
        .iter()
        .filter(|s| {
            s.used_as != UsedAs::Unknown
                && !matches!(s.launch, Err(NotMeasured::NotApproved))
                && !broken(&item_key(&s.file, &s.origin, &s.name))
        })
        .collect();
    let judged_skills: Vec<_> = skills
        .iter()
        .filter(|s| !s.hidden && s.state == SkillState::Ok && !broken(&item_key(&s.file, &s.origin, &s.name)))
        .collect();

    let cutoff = now_ms.saturating_sub(WINDOW_MS);
    let covered = agent.oldest_ms.is_some_and(|o| o <= cutoff);
    if !covered && (!judged_servers.is_empty() || !judged_skills.is_empty()) {
        let (title, detail) = match agent.oldest_ms {
            Some(o) => {
                let d = (now_ms.saturating_sub(o) / DAY_MS).max(1);
                (
                    format!("{cli}'s history covers {d} day{}", if d == 1 { "" } else { "s" }),
                    "Servers and skills are called unused once it covers 30. Marshell keeps what it reads, so this grows each day it runs.".to_string(),
                )
            }
            None => (
                format!("{cli} has no history yet"),
                "Servers and skills are called unused once Marshell has read 30 days of it.".to_string(),
            ),
        };
        out.push((
            mk(format!("usage.{}", inv.agent), DoctorGroup::Info, title, detail, None),
            None,
        ));
    }

    if inv.agent == "antigravity" && !servers.is_empty() {
        out.push((
            mk(
                "usage.antigravity.mcp".to_string(),
                DoctorGroup::Info,
                "Marshell can't yet tell when Antigravity uses an MCP server".to_string(),
                "No Antigravity MCP call has been seen in a transcript, so its servers are never called unused."
                    .to_string(),
                None,
            ),
            None,
        ));
    }
    if !covered {
        return;
    }

    // One judged item: its hits, then the unused and the broad-scope verdicts.
    let judge = |kind: &str,
                 noun: &str,
                 name: &str,
                 file: &Path,
                 origin: &Origin,
                 created_ms: Option<u64>,
                 h: Hits,
                 switch: &Result<Switch, super::ext::NoSwitch>,
                 out: &mut Vec<(DoctorItem, Option<Switch>)>| {
        let key = item_key(file, origin, name);
        let since = [
            created_ms,
            h.available,
            store.first_sight.get(&format!("{}:{key}", inv.agent)).copied(),
        ]
        .into_iter()
        .flatten()
        .min();
        let used_in_window = h.last.is_some_and(|l| l >= cutoff);
        if !used_in_window && since.is_some_and(|s| s <= cutoff) {
            let (what, tail) = if kind == "mcp" {
                ("MCP server", format!("It still starts with every {cli} session."))
            } else {
                (
                    "skill",
                    "Its name and description still load into every session.".to_string(),
                )
            };
            let mut item = mk(
                format!("{kind}unused.{}.{key}", inv.agent),
                DoctorGroup::Waste,
                format!("{cli}'s {what} {name} wasn't used in 30 days"),
                tail,
                Some(file.display().to_string()),
            );
            let sw = fixability(&mut item, switch, cli, noun, name);
            out.push((item, sw));
            return;
        }
        if *origin == Origin::User && used_in_window {
            let mapped: Vec<(String, Option<PathBuf>)> = h
                .places
                .iter()
                .filter(|(_, t)| **t >= cutoff)
                .map(|(p, _)| project_of(p, projects))
                .collect();
            let Some(first) = mapped.first() else { return };
            if mapped.iter().any(|m| m.0 != first.0 || m.0.is_empty()) {
                return;
            }
            let project = first.1.clone().unwrap_or_else(|| PathBuf::from(&first.0));
            let hint = if kind == "mcp" {
                (inv.hints.move_server)(name, &project)
            } else {
                (inv.hints.move_skill)(name, &project)
            };
            let what = if kind == "mcp" { "MCP server" } else { "skill" };
            let mut item = mk(
                format!("{kind}broad.{}.{key}", inv.agent),
                DoctorGroup::Info,
                format!("{cli}'s {what} {name} is used in one project only"),
                format!("In the last 30 days it was used only in {}. {hint}", project.display()),
                Some(file.display().to_string()),
            );
            item.cannot_fix =
                Some("The Doctor doesn't move it, because a project's files are shared with its team.".to_string());
            out.push((item, None));
        }
    };

    for s in judged_servers {
        let h = hits(&agent, &*server_matches(&s.used_as));
        judge("mcp", "server", &s.name, &s.file, &s.origin, None, h, &s.switch, out);
    }
    for s in judged_skills {
        let h = hits(&agent, &skill_matches(&s.used_as));
        judge(
            "skill",
            "skill",
            &s.name,
            &s.file,
            &s.origin,
            s.created_ms,
            h,
            &s.switch,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::ext::{
        Caps, CommandState, CostRule, Hints, NoSwitch, Server, Signals, Skill, Transport, items as ext_items,
    };

    const D: u64 = DAY_MS;
    const NOW: u64 = 100 * D;

    fn inv(agent: &'static str) -> Inventory {
        Inventory {
            agent,
            distro: None,
            found: true,
            caps: Caps::default(),
            servers: vec![],
            skills: vec![],
            unreadable: vec![],
            hints: Hints::default(),
            cost_note: None,
        }
    }

    fn server(name: &str, used_as: UsedAs) -> Server {
        Server {
            name: name.into(),
            file: PathBuf::from("C:/h/.claude.json"),
            origin: Origin::User,
            transport: Transport::Stdio {
                command: "node".into(),
                state: CommandState::Found,
            },
            off: false,
            switch: Err(NoSwitch::ClaudeUser),
            launch: Err(NotMeasured::Remote),
            cost: CostRule::names_only(),
            used_as,
        }
    }

    fn skill(name: &str, used_as: Vec<SkillUse>, created: Option<u64>) -> Skill {
        Skill {
            name: name.into(),
            file: PathBuf::from(format!("C:/h/skills/{name}/SKILL.md")),
            origin: Origin::User,
            state: SkillState::Ok,
            listed_chars: 10,
            first_line_chars: 10,
            name_only: false,
            off: false,
            hidden: false,
            switch: Err(NoSwitch::NoSkillSwitch),
            used_as,
            created_ms: created,
        }
    }

    /// A store for `claude` whose history covers the window.
    fn covered() -> UsageStore {
        let mut s = UsageStore::default();
        s.agents.insert(
            "claude".into(),
            AgentStore {
                oldest_ms: Some(NOW - 31 * D),
                scanned_ms: Some(NOW),
                ..AgentStore::default()
            },
        );
        s
    }

    fn mark_used(s: &mut UsageStore, key: &str, at: u64, place: &str) {
        let u = s.agents.get_mut("claude").unwrap().used.entry(key.into()).or_default();
        u.last_ms = u.last_ms.max(at);
        u.places.insert(place.into(), at);
    }

    /// Marshell first saw every server 40 days ago.
    fn first_seen(store: &mut UsageStore, i: &Inventory) {
        for s in &i.servers {
            store.first_sight.insert(
                format!("claude:{}", item_key(&s.file, &s.origin, &s.name)),
                NOW - 40 * D,
            );
        }
    }

    fn run(i: &Inventory, store: &UsageStore, projects: &[PathBuf]) -> Vec<DoctorItem> {
        let mut out = Vec::new();
        items(i, Some(store), &|_| false, projects, NOW, &mut out);
        out.into_iter().map(|(it, _)| it).collect()
    }

    fn with_prefix<'a>(items: &'a [DoctorItem], prefix: &str) -> Vec<&'a DoctorItem> {
        items.iter().filter(|i| i.id.starts_with(prefix)).collect()
    }

    fn titles<'a>(items: &[&'a DoctorItem]) -> Vec<&'a str> {
        items.iter().map(|i| i.title.as_str()).collect()
    }

    fn cursor(first: u64) -> Cursor {
        Cursor {
            first_ms: Some(first),
            ..Cursor::default()
        }
    }

    fn scan_with(first: u64) -> Scan {
        Scan {
            cursors: BTreeMap::from([("a.jsonl".to_string(), cursor(first))]),
            ..Scan::default()
        }
    }

    #[test]
    fn norm_and_skill_paths() {
        assert_eq!(norm_path(r"\\?\C:\Users\Home Office\"), "c:/users/home office");
        let want = vec!["c:/users/home office/.agents/skills/x/skill.md".to_string()];
        for text in [
            r#""pwsh.exe" -Command "Get-Content 'C:\Users\Home Office\.agents\skills\x\SKILL.md'""#,
            "Get-Content -LiteralPath 'C:/Users/Home Office/.agents/skills/x/SKILL.md'",
            r#"["cat","C:\Users\Home Office\.agents\skills\x\SKILL.md"]"#,
        ] {
            assert_eq!(skill_md_paths(text), want, "{text}");
        }
        assert!(skill_md_paths("see SKILL.md for details").is_empty());
        assert!(skill_md_paths("cat ./x/SKILL.md").is_empty());
        assert_eq!(
            skill_md_paths(r"cat C:\a\skill.MD and D:\b\SKILL.md"),
            ["c:/a/skill.md", "d:/b/skill.md"]
        );
    }

    #[test]
    fn new_lines_is_incremental() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("h.jsonl");
        std::fs::write(&f, "one\r\ntwo\npart").unwrap();
        let old = Cursor {
            first_ms: Some(5),
            cwd: Some("c:/x".into()),
            ..Cursor::default()
        };
        let (lines, c) = new_lines(&f, Some(&old)).unwrap();
        assert_eq!(lines, ["one", "two"]);
        assert_eq!((c.len, c.read_to), (13, 9));
        assert_eq!((c.first_ms, c.cwd.as_deref()), (Some(5), Some("c:/x")));

        std::fs::write(&f, "one\r\ntwo\npartial\nfour\n").unwrap();
        let (lines, c2) = new_lines(&f, Some(&c)).unwrap();
        assert_eq!(lines, ["partial", "four"]);
        assert_eq!(c2.read_to, c2.len);

        // Nothing new.
        let (lines, c3) = new_lines(&f, Some(&c2)).unwrap();
        assert!(lines.is_empty());
        assert_eq!(c3.read_to, c2.read_to);

        // A shorter file is read from the start.
        std::fs::write(&f, "x\n").unwrap();
        let (lines, c4) = new_lines(&f, Some(&c3)).unwrap();
        assert_eq!(lines, ["x"]);
        assert_eq!((c4.len, c4.read_to), (2, 2));
        assert!(new_lines(&dir.path().join("gone"), None).is_err());
    }

    #[test]
    fn coverage_grows_and_resets_after_a_gap() {
        let mut s = AgentStore::default();
        merge(&mut s, scan_with(NOW - 28 * D), NOW);
        assert_eq!(s.oldest_ms, Some(NOW - 28 * D));

        // Two days later the CLI deleted day -28: the disk starts at -27, but the last scan saw -28.
        let later = NOW + 2 * D;
        merge(&mut s, scan_with(NOW - 27 * D), later);
        assert_eq!(s.oldest_ms, Some(NOW - 28 * D));
        assert_eq!(s.scanned_ms, Some(later));
        assert!(s.oldest_ms.unwrap() <= later - WINDOW_MS, "covered");

        // The disk now starts after the last scan: files were lost unseen, so coverage restarts from the disk.
        let much_later = later + 40 * D;
        merge(&mut s, scan_with(later + 5 * D), much_later);
        assert_eq!(s.oldest_ms, Some(later + 5 * D));

        // No history files at all: nothing is covered.
        merge(&mut s, Scan::default(), much_later + D);
        assert_eq!(s.oldest_ms, None);

        // A history that can't be read leaves coverage alone, and keeps the reason.
        let mut t = AgentStore::default();
        merge(&mut t, scan_with(NOW - 28 * D), NOW);
        let unreadable = Scan {
            unreadable: Some("It can't be read.".into()),
            ..Scan::default()
        };
        merge(&mut t, unreadable, NOW + D);
        assert_eq!((t.oldest_ms, t.scanned_ms), (Some(NOW - 28 * D), Some(NOW)));
        assert_eq!(t.unreadable.as_deref(), Some("It can't be read."));
    }

    #[test]
    fn merge_keeps_the_latest_use_the_earliest_listing_and_prunes_old_places() {
        let ev = |kind, key: UseKey, at, cwd: Option<&str>| Event {
            kind,
            key,
            at_ms: at,
            cwd: cwd.map(String::from),
        };
        let mut s = AgentStore::default();
        let scan = Scan {
            events: vec![
                ev(EventKind::Used, UseKey::Tool("t".into()), NOW - 5 * D, Some(r"C:\P\A")),
                ev(EventKind::Used, UseKey::Tool("t".into()), NOW - 9 * D, Some(r"C:\P\A")),
                ev(EventKind::Used, UseKey::Tool("t".into()), NOW - 50 * D, Some(r"C:\Old")),
                ev(EventKind::Used, UseKey::SkillName("Ship".into()), NOW - D, None),
                ev(
                    EventKind::Used,
                    UseKey::SkillFile(r"C:\S\SKILL.md".into()),
                    NOW - D,
                    None,
                ),
                ev(EventKind::Available, UseKey::Server("s".into()), NOW - 9 * D, None),
                ev(EventKind::Available, UseKey::Server("s".into()), NOW - 20 * D, None),
            ],
            ..Scan::default()
        };
        merge(&mut s, scan, NOW);
        let t = &s.used["tool:t"];
        assert_eq!(t.last_ms, NOW - 5 * D);
        assert_eq!(
            t.places.keys().collect::<Vec<_>>(),
            ["c:/p/a"],
            "the old place is pruned"
        );
        assert_eq!(s.used["skill:ship"].places.keys().collect::<Vec<_>>(), [""]);
        assert!(s.used.contains_key("file:c:/s/skill.md"));
        assert_eq!(s.available["server:s"], NOW - 20 * D);
    }

    #[test]
    fn future_events_are_dropped() {
        let at = |t| Event {
            kind: EventKind::Used,
            key: UseKey::Tool("t".into()),
            at_ms: t,
            cwd: None,
        };
        let mut s = AgentStore::default();
        let scan = Scan {
            events: vec![at(NOW + 2 * D), at(NOW + D - 1)],
            ..Scan::default()
        };
        merge(&mut s, scan, NOW);
        assert_eq!(s.used["tool:t"].last_ms, NOW + D - 1);
        // One that is only in the future leaves no key.
        let mut s = AgentStore::default();
        let scan = Scan {
            events: vec![at(NOW + 2 * D)],
            ..Scan::default()
        };
        merge(&mut s, scan, NOW);
        assert!(s.used.is_empty());
        // A clock near 0 doesn't underflow.
        merge(&mut s, Scan::default(), 5);
    }

    #[test]
    fn unused_needs_coverage_and_age() {
        let store = covered();
        let mut i = inv("claude");
        i.skills
            .push(skill("old", vec![SkillUse::Name("old".into())], Some(NOW - 40 * D)));
        i.skills
            .push(skill("new", vec![SkillUse::Name("new".into())], Some(NOW - 10 * D)));
        i.skills
            .push(skill("no-sign", vec![SkillUse::Name("no-sign".into())], None));
        let out = run(&i, &store, &[]);
        let unused = with_prefix(&out, "skillunused.");
        assert_eq!(unused.len(), 1, "{out:?}");
        let it = unused[0];
        assert_eq!(it.group, DoctorGroup::Waste);
        assert_eq!(it.title, "Claude Code's skill old wasn't used in 30 days");
        assert_eq!(it.detail, "Its name and description still load into every session.");
        assert!(it.cannot_fix.is_some() && !it.fixable);
        assert!(with_prefix(&out, "usage.").is_empty());

        // Used 20 days ago: not unused, even though it is old.
        let mut used = store.clone();
        mark_used(&mut used, "skill:old", NOW - 20 * D, "c:/p");
        assert!(with_prefix(&run(&i, &used, &[]), "skillunused.").is_empty());
        // Used 40 days ago: unused.
        let mut stale = store.clone();
        mark_used(&mut stale, "skill:old", NOW - 40 * D, "c:/p");
        assert_eq!(with_prefix(&run(&i, &stale, &[]), "skillunused.").len(), 1);

        // The CLI listing it, or Marshell's first sight, is a sign it was there.
        let mut listed = store.clone();
        let a = listed.agents.get_mut("claude").unwrap();
        a.available.insert("skill:new".into(), NOW - 35 * D);
        assert_eq!(with_prefix(&run(&i, &listed, &[]), "skillunused.").len(), 2);
        let mut seen = store.clone();
        let key = item_key(&i.skills[2].file, &Origin::User, "no-sign");
        seen.first_sight.insert(format!("claude:{key}"), NOW - 31 * D);
        assert_eq!(with_prefix(&run(&i, &seen, &[]), "skillunused.").len(), 2);

        // Not covered yet.
        let mut part = store.clone();
        part.agents.get_mut("claude").unwrap().oldest_ms = Some(NOW - 28 * D);
        let out = run(&i, &part, &[]);
        assert!(with_prefix(&out, "skillunused.").is_empty());
        let info = with_prefix(&out, "usage.claude");
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].group, DoctorGroup::Info);
        assert_eq!(info[0].title, "Claude Code's history covers 28 days");
        assert_eq!(
            info[0].detail,
            "Servers and skills are called unused once it covers 30. Marshell keeps what it reads, so this grows each day it runs."
        );
        part.agents.get_mut("claude").unwrap().oldest_ms = Some(NOW - D / 2);
        let out = run(&i, &part, &[]);
        assert_eq!(
            with_prefix(&out, "usage.claude")[0].title,
            "Claude Code's history covers 1 day"
        );
        part.agents.get_mut("claude").unwrap().oldest_ms = None;
        let out = run(&i, &part, &[]);
        let info = with_prefix(&out, "usage.claude");
        assert_eq!(info[0].title, "Claude Code has no history yet");
        assert_eq!(
            info[0].detail,
            "Servers and skills are called unused once Marshell has read 30 days of it."
        );
        // A CLI with nothing to judge gets no coverage line, and no store at all gives no items.
        assert!(run(&inv("claude"), &part, &[]).is_empty());
        let mut none = Vec::new();
        items(&i, None, &|_| false, &[], NOW, &mut none);
        assert!(none.is_empty());
    }

    #[test]
    fn an_unused_server_carries_the_switch() {
        let mut i = inv("claude");
        let mut s = server("gh", UsedAs::ToolPrefix("mcp__gh__".into()));
        s.switch = Ok(Switch::JsonAdd {
            file: PathBuf::from("C:/p/.claude/settings.local.json"),
            pointer: "/disabledMcpjsonServers".into(),
            name: "gh".into(),
        });
        i.servers.push(s);
        let mut store = covered();
        first_seen(&mut store, &i);
        let mut out = Vec::new();
        items(&i, Some(&store), &|_| false, &[], NOW, &mut out);
        assert_eq!(out.len(), 1);
        let (it, sw) = &out[0];
        assert!(it.id.starts_with("mcpunused.claude."));
        assert_eq!(it.group, DoctorGroup::Waste);
        assert_eq!(it.title, "Claude Code's MCP server gh wasn't used in 30 days");
        assert!(it.detail.starts_with("It still starts with every Claude Code session."));
        assert!(it.fixable && sw.is_some());
    }

    #[test]
    fn uses_count_for_every_match() {
        let old = Some(NOW - 40 * D);
        let mut i = inv("claude");
        let name = |n: &str| vec![SkillUse::Name(n.into())];
        i.skills.push(skill("marshell-ship", name("marshell-ship"), old));
        i.skills.push(skill(
            "superpowers:writing-plans",
            name("superpowers:writing-plans"),
            old,
        ));
        let by_file = vec![SkillUse::File(PathBuf::from(r"C:\Users\Home Office\s\SKILL.md"))];
        i.skills.push(skill("file-skill", by_file, old));
        i.skills.push(skill("plain", name("plain"), old));
        i.servers
            .push(server("plug", UsedAs::ToolPrefix("mcp__plugin_plug_srv__".into())));
        let mut twin = server("twin-a", UsedAs::ToolPrefix("mcp__twin__".into()));
        twin.file = PathBuf::from("C:/a.json");
        i.servers.push(twin.clone());
        twin.name = "twin-b".into();
        twin.file = PathBuf::from("C:/b.json");
        i.servers.push(twin);
        i.servers.push(server("named", UsedAs::Server("named".into())));
        let mut store = covered();
        first_seen(&mut store, &i);
        // Nothing used: all eight are unused.
        assert_eq!(with_prefix(&run(&i, &store, &[]), "skillunused.").len(), 4);
        assert_eq!(with_prefix(&run(&i, &store, &[]), "mcpunused.").len(), 4);

        for key in [
            "skill:.claude/worktrees/x:marshell-ship",
            "skill:SUPERPOWERS:writing-plans",
            "file:c:/users/home office/s/skill.md",
            "tool:mcp__plugin_plug_srv__x",
            "tool:mcp__twin__q",
            "server:named",
        ] {
            mark_used(&mut store, key, NOW - D, "c:/p");
        }
        let out = run(&i, &store, &[]);
        let unused = with_prefix(&out, "skillunused.");
        assert_eq!(
            titles(&unused),
            ["Claude Code's skill plain wasn't used in 30 days"],
            "{out:?}"
        );
        assert!(with_prefix(&out, "mcpunused.").is_empty(), "{out:?}");

        // A bare name isn't matched by another namespace's use, and a worktree folder needs a slash.
        let mut other = covered();
        mark_used(&mut other, "skill:other:marshell-ship", NOW - D, "c:/p");
        mark_used(&mut other, "skill:superpowers:writing-plans", NOW - D, "c:/p");
        let out = run(&i, &other, &[]);
        let unused = titles(&with_prefix(&out, "skillunused."));
        assert!(unused.iter().any(|t| t.contains("marshell-ship")), "{unused:?}");
        assert!(unused.iter().any(|t| t.contains("skill plain")), "{unused:?}");
        assert!(!unused.iter().any(|t| t.contains("writing-plans")), "{unused:?}");
    }

    #[test]
    fn broken_items_get_no_unused() {
        let mut i = inv("claude");
        let mut gone = server("gone", UsedAs::ToolPrefix("mcp__gone__".into()));
        gone.transport = Transport::Stdio {
            command: "C:/nope/x.exe".into(),
            state: CommandState::Missing,
        };
        i.servers.push(gone);
        i.servers.push(server("fine", UsedAs::ToolPrefix("mcp__fine__".into())));
        let mut bad = skill("bad", vec![SkillUse::Name("bad".into())], Some(NOW - 40 * D));
        bad.state = SkillState::NoBlock;
        i.skills.push(bad);
        i.skills
            .push(skill("ok", vec![SkillUse::Name("ok".into())], Some(NOW - 40 * D)));
        let mut store = covered();
        first_seen(&mut store, &i);
        let sig = Signals {
            usage: Some(&store),
            now_ms: NOW,
            ..Signals::default()
        };
        let out = ext_items(&[i.clone()], &sig);
        assert_eq!(with_prefix(&out, "mcp.").len(), 1, "{out:?}");
        assert_eq!(with_prefix(&out, "skill.").len(), 1, "{out:?}");
        let unused: Vec<&str> = out
            .iter()
            .filter(|i| i.id.contains("unused"))
            .map(|i| i.title.as_str())
            .collect();
        assert_eq!(
            unused,
            [
                "Claude Code's MCP server fine wasn't used in 30 days",
                "Claude Code's skill ok wasn't used in 30 days"
            ]
        );

        // A server that failed when measured is Broken too, so `broken` keeps it out of the unused items.
        let key = item_key(&i.servers[1].file, &Origin::User, "fine");
        let mut direct = Vec::new();
        items(&i, Some(&store), &|k| k == key, &[], NOW, &mut direct);
        assert!(direct.iter().all(|(it, _)| !it.title.contains("fine")));
    }

    #[test]
    fn items_that_cant_be_judged_are_left_alone() {
        let mut store = covered();
        let mut i = inv("claude");
        let mk = |name: &str| {
            let mut s = server(name, UsedAs::Server(name.into()));
            s.file = PathBuf::from(format!("C:/{name}.json"));
            s
        };
        let mut off = mk("off");
        off.off = true;
        let mut bundled = mk("bundled");
        bundled.origin = Origin::Bundled;
        let mut unknown = mk("unknown");
        unknown.used_as = UsedAs::Unknown;
        let mut unapproved = mk("unapproved");
        unapproved.launch = Err(NotMeasured::NotApproved);
        i.servers.extend([off, bundled, unknown, unapproved, mk("judged")]);
        let mut hidden = skill("hidden", vec![SkillUse::Name("hidden".into())], Some(NOW - 40 * D));
        hidden.hidden = true;
        i.skills.push(hidden);
        first_seen(&mut store, &i);
        let out = run(&i, &store, &[]);
        assert_eq!(
            titles(&out.iter().collect::<Vec<_>>()),
            ["Claude Code's MCP server judged wasn't used in 30 days"]
        );

        // A distro's inventory, and one that isn't found, get nothing.
        let mut d = i.clone();
        d.distro = Some("Ubuntu".into());
        assert!(run(&d, &store, &[]).is_empty());
        let mut gone = i.clone();
        gone.found = false;
        assert!(run(&gone, &store, &[]).is_empty());
    }

    #[test]
    fn broad_scope() {
        let mut store = covered();
        let mut i = inv("claude");
        i.skills
            .push(skill("ship", vec![SkillUse::Name("ship".into())], Some(NOW - 40 * D)));
        let mut proj = skill("proj", vec![SkillUse::Name("proj".into())], Some(NOW - 40 * D));
        proj.origin = Origin::Project(PathBuf::from(r"C:\dev\Airport"));
        i.skills.push(proj);
        let projects = [PathBuf::from(r"C:\dev\Airport"), PathBuf::from(r"C:\dev")];
        mark_used(
            &mut store,
            "skill:ship",
            NOW - 2 * D,
            "c:/dev/airport/.claude/worktrees/a",
        );
        mark_used(&mut store, "skill:ship", NOW - 3 * D, "c:/dev/airport");
        mark_used(&mut store, "skill:proj", NOW - 3 * D, "c:/dev/airport");
        // A place used long ago doesn't count.
        let u = store
            .agents
            .get_mut("claude")
            .unwrap()
            .used
            .get_mut("skill:ship")
            .unwrap();
        u.places.insert("c:/other".into(), NOW - 45 * D);

        let out = run(&i, &store, &projects);
        let broad = with_prefix(&out, "skillbroad.claude.");
        assert_eq!(broad.len(), 1, "{out:?}");
        assert_eq!(broad[0].group, DoctorGroup::Info);
        assert_eq!(broad[0].title, "Claude Code's skill ship is used in one project only");
        assert_eq!(
            broad[0].detail,
            r"In the last 30 days it was used only in C:\dev\Airport. Move it to that project."
        );
        assert_eq!(
            broad[0].cannot_fix.as_deref(),
            Some("The Doctor doesn't move it, because a project's files are shared with its team.")
        );
        assert!(!broad[0].fixable);
        assert_eq!(out.len(), 1, "a project-origin skill gets no item: {out:?}");

        // Two projects: no item.
        mark_used(&mut store, "skill:ship", NOW - D, "c:/dev/other");
        assert!(with_prefix(&run(&i, &store, &projects), "skillbroad.").is_empty());

        // One use at an unknown place: no item, and so for a known project plus an unknown place.
        let mut unknown = covered();
        mark_used(&mut unknown, "skill:ship", NOW - D, "");
        assert!(with_prefix(&run(&i, &unknown, &projects), "skillbroad.").is_empty());
        mark_used(&mut unknown, "skill:ship", NOW - D, "c:/dev/airport");
        assert!(with_prefix(&run(&i, &unknown, &projects), "skillbroad.").is_empty());

        // A folder no project holds names itself, and a lookalike prefix isn't inside the project.
        let mut alone = covered();
        mark_used(&mut alone, "skill:ship", NOW - D, "c:/dev/airportx");
        let out = run(&i, &alone, &[PathBuf::from(r"C:\dev\Airport")]);
        let broad = with_prefix(&out, "skillbroad.");
        assert_eq!(broad.len(), 1);
        assert!(
            broad[0]
                .detail
                .starts_with("In the last 30 days it was used only in c:/dev/airportx.")
        );
    }

    #[test]
    fn broad_scope_for_a_server() {
        let mut store = covered();
        let mut i = inv("claude");
        i.servers.push(server("gh", UsedAs::ToolPrefix("mcp__gh__".into())));
        mark_used(&mut store, "tool:mcp__gh__list", NOW - D, "c:/dev/airport");
        let out = run(&i, &store, &[PathBuf::from(r"C:\dev\Airport")]);
        let broad = with_prefix(&out, "mcpbroad.claude.");
        assert_eq!(broad.len(), 1, "{out:?}");
        assert_eq!(
            broad[0].title,
            "Claude Code's MCP server gh is used in one project only"
        );
    }

    #[test]
    fn unreadable_item() {
        let mut store = covered();
        store.agents.get_mut("claude").unwrap().unreadable = Some("It can't be read.".into());
        let mut i = inv("claude");
        i.skills
            .push(skill("old", vec![SkillUse::Name("old".into())], Some(NOW - 40 * D)));
        let out = run(&i, &store, &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "usage.claude");
        assert_eq!(out[0].group, DoctorGroup::Info);
        assert_eq!(out[0].title, "Claude Code's history can't be read");
        assert_eq!(
            out[0].detail,
            "It can't be read. So its servers and skills aren't called unused."
        );
    }

    #[test]
    fn antigravity_mcp_line() {
        let mut i = inv("antigravity");
        let mut store = UsageStore::default();
        store.agents.insert(
            "antigravity".into(),
            AgentStore {
                oldest_ms: Some(NOW - 31 * D),
                ..AgentStore::default()
            },
        );
        assert!(run(&i, &store, &[]).is_empty());
        let mut s = server("m", UsedAs::Unknown);
        s.origin = Origin::Project(PathBuf::from("C:/p"));
        i.servers.push(s);
        let out = run(&i, &store, &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "usage.antigravity.mcp");
        assert_eq!(out[0].group, DoctorGroup::Info);
        assert_eq!(
            out[0].title,
            "Marshell can't yet tell when Antigravity uses an MCP server"
        );
        assert_eq!(
            out[0].detail,
            "No Antigravity MCP call has been seen in a transcript, so its servers are never called unused."
        );
        // A server that is off doesn't count.
        i.servers[0].off = true;
        assert!(run(&i, &store, &[]).is_empty());
    }

    #[test]
    fn absorb_is_commutative() {
        let agent = |scanned: u64, oldest: u64, last: u64, place: &str, avail: u64| {
            let mut a = AgentStore {
                scanned_ms: Some(scanned),
                oldest_ms: Some(oldest),
                cursors: BTreeMap::from([(format!("f{scanned}"), cursor(oldest))]),
                ..AgentStore::default()
            };
            a.used.insert(
                "tool:t".into(),
                Use {
                    last_ms: last,
                    places: BTreeMap::from([(place.to_string(), last)]),
                },
            );
            a.available.insert("server:s".into(), avail);
            a
        };
        let mut x = UsageStore::default();
        x.agents.insert("claude".into(), agent(10, 1, 7, "c:/a", 5));
        x.first_sight.insert("claude:k".into(), 4);
        let mut y = UsageStore::default();
        y.agents.insert("claude".into(), agent(20, 3, 9, "c:/a", 2));
        y.agents.insert("codex".into(), agent(5, 5, 5, "c:/b", 5));
        y.first_sight.insert("claude:k".into(), 2);
        y.first_sight.insert("codex:j".into(), 8);

        let mut xy = x.clone();
        absorb(&mut xy, &y);
        let mut yx = y.clone();
        absorb(&mut yx, &x);
        assert_eq!(xy, yx);
        let c = &xy.agents["claude"];
        assert_eq!(
            (c.scanned_ms, c.oldest_ms),
            (Some(20), Some(3)),
            "the later scan gives coverage"
        );
        assert_eq!(c.cursors.keys().collect::<Vec<_>>(), ["f20"]);
        assert_eq!(c.used["tool:t"].last_ms, 9);
        assert_eq!(c.used["tool:t"].places["c:/a"], 9);
        assert_eq!(c.available["server:s"], 2);
        assert_eq!(xy.first_sight["claude:k"], 2);
        assert_eq!(xy.first_sight["codex:j"], 8);
        assert!(xy.agents.contains_key("codex"));
    }

    #[test]
    fn the_store_round_trips_and_reads_what_is_missing() {
        let mut s = UsageStore::default();
        merge(
            s.agents.entry("claude".into()).or_default(),
            scan_with(NOW - 5 * D),
            NOW,
        );
        let back: UsageStore = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        let sparse: UsageStore = serde_json::from_str(r#"{"version":1,"agents":{"claude":{}}}"#).unwrap();
        assert_eq!(sparse.agents["claude"], AgentStore::default());
    }
}
