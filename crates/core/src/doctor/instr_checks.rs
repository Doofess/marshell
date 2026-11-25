//! What is wrong inside the instruction files (slice 7d2): imports whose file doesn't exist, and backticked paths
//! that don't. `findings` reads the run's loads through the run's reader, so no file is read twice; `items` words
//! them. Nothing here logs: a quoted line is file content, and the logs are exported (decision 14).
//!
//! The module names no CLI's files. What differs between CLIs (import syntax, whether HTML comments are read, what a
//! CLI does with a dead import) comes from each load's `imports` and from `dead_import_drops`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use marshell_protocol::doctor::{DoctorGroup, DoctorItem, DoctorOpen};

use super::checks::{dead_import_drops, display_name};
use super::instr_text::backticked_paths;
use super::instructions::{
    CliLoad, ImportSyntax, Reader, Role, Text, When, cli_rank, display, folder_name, hash_part, id_tail, imports_at,
    is_within, mask_html_comments, normalize, path_key,
};

/// Where a finding sits. `project` is None for one whose files are all at user level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Seat {
    pub project: Option<PathBuf>,
    pub distro: Option<String>,
    pub home: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    DeadImport {
        seat: Seat,
        file: PathBuf,
        target: PathBuf,
        syntax: ImportSyntax,
        /// The first line it is written on, and the import as written there (`@docs/setup.md`).
        line: usize,
        written: String,
        /// Every spelling of it in the file.
        raws: Vec<String>,
        /// Some CLI that reads the file reads HTML comments.
        in_comments: bool,
        agents: Vec<&'static str>,
    },
    StalePaths {
        seat: Seat,
        file: PathBuf,
        paths: Vec<(usize, String)>,
    },
}

impl Finding {
    fn seat(&self) -> &Seat {
        match self {
            Finding::DeadImport { seat, .. } | Finding::StalePaths { seat, .. } => seat,
        }
    }
}

/// Every finding in these loads. `cx` is the reader that made them, so no file is read twice.
pub fn findings(loads: &[CliLoad], cx: &mut Reader) -> Vec<Finding> {
    let mut out = dead_imports(loads, cx);
    out.extend(stale_paths(loads, cx));
    out
}

/// A file as one comparable text: its canonical path when there is one.
fn key_of(cx: &mut Reader, path: &Path) -> String {
    path_key(&cx.canonical(path).unwrap_or_else(|| path.to_path_buf()))
}

/// Which rows of a load are looked at: loaded at start or when used, and not under auto memory or a managed file
/// (decision 15). A row's root is the nearest earlier row that isn't an import; `expand` pushes a file's own row
/// first and its imports after it.
fn looked_at(load: &CliLoad) -> Vec<bool> {
    let mut root_ok = true;
    load.files
        .iter()
        .map(|file| {
            if !matches!(file.role, Role::Import { .. }) {
                root_ok = !matches!(file.role, Role::Memory | Role::Managed);
            }
            root_ok && matches!(file.when, When::Start | When::WhenUsed)
        })
        .collect()
}

fn seat_of(load: &CliLoad, file: &Path) -> Seat {
    Seat {
        project: load.project.clone().filter(|p| is_within(file, p)),
        distro: load.distro.clone(),
        home: load.home.clone(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Dead imports (decision 2)

struct DeadGroup {
    seat: Seat,
    file: PathBuf,
    target: PathBuf,
    syntax: ImportSyntax,
    raws: Vec<String>,
    agents: Vec<&'static str>,
    in_comments: bool,
    key: (Option<String>, String, String, ImportSyntax),
}

fn dead_imports(loads: &[CliLoad], cx: &mut Reader) -> Vec<Finding> {
    let mut groups: Vec<DeadGroup> = Vec::new();
    for load in loads {
        let Some(style) = load.imports else { continue };
        let looked = looked_at(load);
        // The files' keys, worked out once and only for a load that has a dead import.
        let mut keys: Option<Vec<String>> = None;
        for (i, row) in load.files.iter().enumerate() {
            let (Role::Import { by }, When::NotLoaded(why), Some(raw)) = (&row.role, &row.when, &row.raw) else {
                continue;
            };
            if why != "not found" {
                continue;
            }
            let keys = keys.get_or_insert_with(|| load.files.iter().map(|f| key_of(cx, &f.path)).collect());
            let by_key = key_of(cx, by);
            if !(0..i).any(|j| looked[j] && keys[j] == by_key) {
                continue;
            }
            let key = (
                load.distro.clone(),
                by_key,
                path_key(&normalize(&row.path)),
                style.syntax,
            );
            let at = match groups.iter().position(|g| g.key == key) {
                Some(at) => at,
                None => {
                    groups.push(DeadGroup {
                        seat: seat_of(load, by),
                        file: by.clone(),
                        target: row.path.clone(),
                        syntax: style.syntax,
                        raws: Vec::new(),
                        agents: Vec::new(),
                        in_comments: false,
                        key,
                    });
                    groups.len() - 1
                }
            };
            let group = &mut groups[at];
            if !group.raws.contains(raw) {
                group.raws.push(raw.clone());
            }
            if !group.agents.contains(&load.agent) {
                group.agents.push(load.agent);
            }
            group.in_comments |= style.in_comments;
        }
    }
    let mut out = Vec::new();
    for mut group in groups {
        group.agents.sort_by_key(|a| cli_rank(a));
        let Text::Text(text) = cx.read(&group.file) else {
            continue;
        };
        let seen = if group.in_comments {
            text.to_string()
        } else {
            mask_html_comments(&text)
        };
        let Some(hit) = imports_at(&seen, group.syntax)
            .into_iter()
            .find(|hit| group.raws.contains(&hit.raw))
        else {
            continue;
        };
        let written = text
            .lines()
            .nth(hit.line - 1)
            .and_then(|line| line.get(hit.start..hit.end))
            .map_or_else(|| format!("@{}", hit.raw), str::to_string);
        out.push(Finding::DeadImport {
            seat: group.seat,
            file: group.file,
            target: group.target,
            syntax: group.syntax,
            line: hit.line,
            written,
            raws: group.raws,
            in_comments: group.in_comments,
            agents: group.agents,
        });
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Stale paths (decision 4)

/// The `.gitignore` of each folder, read once, each as its own matcher rooted at its folder.
#[derive(Default)]
struct Ignores {
    by_dir: HashMap<PathBuf, Option<Gitignore>>,
}

impl Ignores {
    fn matcher(&mut self, cx: &mut Reader, dir: &Path) -> Option<&Gitignore> {
        self.by_dir
            .entry(dir.to_path_buf())
            .or_insert_with(|| {
                let Text::Text(text) = cx.read(&dir.join(".gitignore")) else {
                    return None;
                };
                let mut builder = GitignoreBuilder::new(dir);
                for line in text.lines() {
                    // A line the matcher can't read is left out, as git does.
                    let _ = builder.add_line(None, line);
                }
                builder.build().ok()
            })
            .as_ref()
    }

    /// Whether a `.gitignore` between the path's folder and the project's root covers `path`; the deepest one that
    /// has an answer decides. `.git/info/exclude` and the global excludes file are not read.
    fn covers(&mut self, cx: &mut Reader, project: &Path, path: &Path) -> bool {
        let depth = project.components().count();
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d.components().count() < depth || !d.starts_with(project) {
                break;
            }
            if let Some(matcher) = self.matcher(cx, d) {
                match matcher.matched_path_or_any_parents(path, false) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
            dir = d.parent();
        }
        false
    }
}

/// Whether a path is something the person should be told is missing.
fn is_stale(cx: &mut Reader, ignores: &mut Ignores, project: &Path, folder: &Path, path: &str) -> bool {
    let from_root = project.join(path);
    let from_file = folder.join(path);
    if cx.exists(&from_root) || cx.exists(&from_file) {
        return false;
    }
    // `origin/main`, `text/html`: a first part that is no folder and a last part with no extension is most likely not
    // a path.
    let mut parts = path.split('/');
    let first = parts.next().unwrap_or("");
    let last = parts.next_back().unwrap_or(first);
    if !matches!(first, "." | "..")
        && !cx.is_dir(&project.join(first))
        && !cx.is_dir(&folder.join(first))
        && Path::new(last).extension().is_none()
    {
        return false;
    }
    !(ignores.covers(cx, project, &normalize(&from_root)) || ignores.covers(cx, project, &normalize(&from_file)))
}

fn stale_paths(loads: &[CliLoad], cx: &mut Reader) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut done: Vec<(String, String)> = Vec::new();
    let mut ignores = Ignores::default();
    // Only Windows loads with a project: a distro's user level has no project to resolve from (decision 16).
    for load in loads.iter().filter(|l| l.distro.is_none()) {
        let Some(project) = &load.project else { continue };
        let looked = looked_at(load);
        for (row, _) in load.files.iter().zip(&looked).filter(|(_, ok)| **ok) {
            if cx.out_of_time() {
                return out;
            }
            if !is_within(&row.path, project) {
                continue;
            }
            let id = (path_key(project), key_of(cx, &row.path));
            if done.contains(&id) {
                continue;
            }
            done.push(id);
            let Text::Text(text) = cx.read(&row.path) else {
                continue;
            };
            let folder = row.path.parent().unwrap_or(project);
            let mut paths = Vec::new();
            for (line, path) in backticked_paths(&text) {
                if cx.out_of_time() {
                    return out;
                }
                if is_stale(cx, &mut ignores, project, folder, &path) {
                    paths.push((line, path));
                }
            }
            if !paths.is_empty() {
                out.push(Finding::StalePaths {
                    seat: seat_of(load, &row.path),
                    file: row.path.clone(),
                    paths,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Items

/// The id of the finding's item.
pub fn id_of(f: &Finding) -> String {
    let seat = f.seat();
    let (place, suffix) = id_tail(seat.project.as_deref(), seat.distro.as_deref());
    match f {
        Finding::DeadImport {
            file, target, syntax, ..
        } => {
            let how = match syntax {
                ImportSyntax::At => "at",
                ImportSyntax::AgyLink => "link",
            };
            format!(
                "instrimport.{place}.{}.{}.{how}{suffix}",
                hash_part(file),
                hash_part(target)
            )
        }
        Finding::StalePaths { file, .. } => format!("instrstale.{place}.{}{suffix}", hash_part(file)),
    }
}

/// "A", "A and B", "A, B and C".
fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_string(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The file as a title says it: project-relative and the project's name, `~/…` at user level.
fn shown(seat: &Seat, file: &Path) -> String {
    let name = display(file, seat.project.as_deref(), &seat.home);
    match &seat.project {
        Some(project) => format!("{name} in {}", folder_name(project)),
        None => name,
    }
}

/// A path as the editor opens it: no `\\?\` prefix.
fn open_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest.to_string(),
        _ => text.into_owned(),
    }
}

fn opens(seat: &Seat, file: &Path, line: usize) -> Vec<DoctorOpen> {
    if seat.distro.is_some() {
        return Vec::new();
    }
    let name = file
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    vec![DoctorOpen {
        path: open_path(file),
        line: u32::try_from(line).ok(),
        label: format!("Open {name}"),
    }]
}

fn item_of(f: &Finding, editable: &dyn Fn(&Path) -> bool) -> DoctorItem {
    let mut item = DoctorItem {
        id: id_of(f),
        agent: None,
        distro: f.seat().distro.clone(),
        group: DoctorGroup::Info,
        title: String::new(),
        detail: String::new(),
        fixable: false,
        cannot_fix: None,
        path: None,
        open_agents: false,
        unchecked: false,
        action: None,
        loads: Vec::new(),
        opens: Vec::new(),
    };
    match f {
        Finding::DeadImport {
            seat,
            file,
            line,
            written,
            agents,
            ..
        } => {
            let (drops, keeps): (Vec<&'static str>, Vec<&'static str>) =
                agents.iter().partition(|a| dead_import_drops(a));
            let name = |agents: &[&'static str]| list(&agents.iter().map(|a| display_name(a)).collect::<Vec<_>>());
            let mut detail = format!("Line {line}: {written}.");
            if !drops.is_empty() {
                let verb = if drops.len() == 1 { "loads" } else { "load" };
                detail.push_str(&format!(" {} {verb} nothing for it.", name(&drops)));
            }
            if !keeps.is_empty() {
                let verb = if keeps.len() == 1 { "leaves" } else { "leave" };
                detail.push_str(&format!(" {} {verb} it in as text.", name(&keeps)));
            }
            item.group = DoctorGroup::Broken;
            item.title = format!("{} imports a file that doesn't exist", shown(seat, file));
            item.detail = detail;
            match &seat.distro {
                Some(distro) => item.cannot_fix = Some(format!("Edit the file in {distro}.")),
                None if editable(file) => {
                    item.fixable = true;
                    item.unchecked = true;
                }
                None => item.cannot_fix = Some("Marshell doesn't change this file.".to_string()),
            }
            item.opens = opens(seat, file, *line);
        }
        Finding::StalePaths { seat, file, paths } => {
            let count = match paths.len() {
                1 => "a path that doesn't".to_string(),
                n => format!("{n} paths that don't"),
            };
            let mut parts: Vec<String> = paths
                .iter()
                .take(5)
                .map(|(line, path)| format!("Line {line}: {path}."))
                .collect();
            if paths.len() > 5 {
                parts.push(format!("And {} more.", paths.len() - 5));
            }
            item.title = format!("{} names {count} exist", shown(seat, file));
            item.detail = parts.join(" ");
            item.opens = opens(seat, file, paths.first().map_or(1, |(line, _)| *line));
        }
    }
    item
}

/// The findings as items, Broken first, then in `projects` order with user level last. `editable` says whether the
/// config service may write a path.
pub fn items(findings: &[Finding], projects: &[PathBuf], editable: &dyn Fn(&Path) -> bool) -> Vec<DoctorItem> {
    let place = |seat: &Seat| match (&seat.distro, &seat.project) {
        (Some(distro), _) => (2, 0, distro.clone()),
        (None, Some(project)) => (
            0,
            projects
                .iter()
                .position(|p| path_key(p) == path_key(project))
                .unwrap_or(projects.len()),
            String::new(),
        ),
        (None, None) => (1, 0, String::new()),
    };
    let mut made: Vec<(DoctorItem, (u8, usize, String))> = findings
        .iter()
        .map(|f| (item_of(f, editable), place(f.seat())))
        .collect();
    let group = |g: DoctorGroup| match g {
        DoctorGroup::Broken => 0,
        DoctorGroup::Waste => 1,
        DoctorGroup::Info => 2,
    };
    made.sort_by(|(a, pa), (b, pb)| (group(a.group), pa).cmp(&(group(b.group), pb)));
    made.into_iter().map(|(item, _)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::instructions::{CliLoad, ImportStyle, LoadedFile, Reader, Role, When};
    use marshell_protocol::doctor::{DoctorGroup, DoctorOpen};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn style(agent: &str) -> Option<ImportStyle> {
        let (syntax, in_comments) = match agent {
            "claude" | "qwen" => (ImportSyntax::At, false),
            "copilot" => (ImportSyntax::At, true),
            "antigravity" => (ImportSyntax::AgyLink, true),
            _ => return None,
        };
        Some(ImportStyle { syntax, in_comments })
    }

    fn load(agent: &'static str, project: &Path, rows: Vec<LoadedFile>) -> CliLoad {
        CliLoad {
            agent,
            distro: None,
            project: Some(project.to_path_buf()),
            home: project.parent().unwrap().join("home"),
            files: rows,
            reason: None,
            notes: Vec::new(),
            cuts: Vec::new(),
            imports: style(agent),
        }
    }

    fn row(path: &Path, role: Role, when: When, raw: Option<&str>) -> LoadedFile {
        LoadedFile {
            path: path.to_path_buf(),
            role,
            when,
            chars: 10,
            notes: Vec::new(),
            depth: 0,
            raw: raw.map(str::to_string),
        }
    }

    fn file_row(path: &Path) -> LoadedFile {
        row(path, Role::File, When::Start, None)
    }

    fn dead_row(by: &Path, target: &Path, raw: &str) -> LoadedFile {
        row(
            target,
            Role::Import { by: by.to_path_buf() },
            When::NotLoaded("not found".into()),
            Some(raw),
        )
    }

    fn proj(tmp: &Path) -> PathBuf {
        let p = tmp.join("proj");
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn yes(_: &Path) -> bool {
        true
    }

    fn dead_import_load(agent: &'static str, project: &Path, file: &Path, raw: &str) -> CliLoad {
        load(
            agent,
            project,
            vec![file_row(file), dead_row(file, &project.join("gone.md"), raw)],
        )
    }

    #[test]
    fn a_dead_import_names_its_line_and_the_clis_that_read_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("CLAUDE.md");
        write(&file, "# A\n\n\n@gone.md\n");
        let loads = vec![
            dead_import_load("claude", &p, &file, "gone.md"),
            dead_import_load("copilot", &p, &file, "gone.md"),
        ];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert_eq!(found.len(), 1);
        let Finding::DeadImport {
            line, agents, written, ..
        } = &found[0]
        else {
            panic!("not a dead import: {found:?}");
        };
        assert_eq!(*line, 4);
        assert_eq!(written, "@gone.md");
        assert_eq!(agents, &vec!["claude", "copilot"]);
        let shown = items(&found, std::slice::from_ref(&p), &yes);
        assert_eq!(shown.len(), 1);
        let item = &shown[0];
        assert_eq!(item.group, DoctorGroup::Broken);
        assert_eq!(item.title, "CLAUDE.md in proj imports a file that doesn't exist");
        assert_eq!(
            item.detail,
            "Line 4: @gone.md. Claude Code loads nothing for it. Copilot CLI leaves it in as text."
        );
        assert!(item.fixable);
        assert!(item.unchecked);
        assert_eq!(
            item.opens,
            vec![DoctorOpen {
                path: file.to_string_lossy().into_owned(),
                line: Some(4),
                label: "Open CLAUDE.md".into(),
            }]
        );
        assert_eq!(item.id, id_of(&found[0]));
    }

    #[test]
    fn a_comment_counts_only_for_a_cli_that_reads_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("CLAUDE.md");
        write(&file, "<!-- @gone.md -->\n\n@gone.md\n");
        let claude = dead_import_load("claude", &p, &file, "gone.md");
        let mut cx = Reader::windows();
        let found = findings(std::slice::from_ref(&claude), &mut cx);
        let Finding::DeadImport { line, in_comments, .. } = &found[0] else {
            panic!("not a dead import");
        };
        assert_eq!((*line, *in_comments), (3, false));

        let copilot = dead_import_load("copilot", &p, &file, "gone.md");
        let mut cx = Reader::windows();
        let found = findings(&[claude, copilot], &mut cx);
        assert_eq!(found.len(), 1);
        let Finding::DeadImport { line, in_comments, .. } = &found[0] else {
            panic!("not a dead import");
        };
        assert_eq!((*line, *in_comments), (1, true));
    }

    #[test]
    fn the_same_dead_import_twice_is_one_item_at_its_first_line() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("CLAUDE.md");
        write(&file, "# A\n\n@gone.md\n\n\n\n\n\n@./gone.md\n");
        let target = p.join("gone.md");
        let loads = vec![load(
            "claude",
            &p,
            vec![
                file_row(&file),
                dead_row(&file, &target, "gone.md"),
                dead_row(&file, &target, "./gone.md"),
            ],
        )];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert_eq!(found.len(), 1);
        let Finding::DeadImport { line, raws, .. } = &found[0] else {
            panic!("not a dead import");
        };
        assert_eq!(*line, 3);
        assert_eq!(raws.len(), 2);
    }

    #[test]
    fn a_dead_import_at_user_level_is_one_item_for_two_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let (p1, p2) = (proj(tmp.path()), tmp.path().join("proj2"));
        fs::create_dir_all(&p2).unwrap();
        let home = tmp.path().join("home");
        let file = home.join(".claude").join("CLAUDE.md");
        write(&file, "@gone.md\n");
        let target = home.join(".claude").join("gone.md");
        let mk = |p: &Path| load("claude", p, vec![file_row(&file), dead_row(&file, &target, "gone.md")]);
        let loads = vec![mk(&p1), mk(&p2)];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert_eq!(found.len(), 1);
        let Finding::DeadImport { seat, .. } = &found[0] else {
            panic!("not a dead import");
        };
        assert_eq!(seat.project, None);
        let shown = items(&found, &[p1, p2], &yes);
        assert_eq!(shown[0].title, "~/.claude/CLAUDE.md imports a file that doesn't exist");
        assert!(shown[0].id.starts_with("instrimport.user."), "{}", shown[0].id);
    }

    #[test]
    fn a_dead_import_marshell_may_not_edit_says_why() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("CLAUDE.md");
        write(&file, "@gone.md\n");
        let loads = vec![dead_import_load("claude", &p, &file, "gone.md")];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        let shown = items(&found, std::slice::from_ref(&p), &|_| false);
        assert!(!shown[0].fixable);
        assert!(!shown[0].unchecked);
        assert_eq!(
            shown[0].cannot_fix.as_deref(),
            Some("Marshell doesn't change this file.")
        );

        let mut distro = dead_import_load("claude", &p, &file, "gone.md");
        distro.distro = Some("Ubuntu".into());
        let mut cx = Reader::windows();
        let found = findings(&[distro], &mut cx);
        let shown = items(&found, std::slice::from_ref(&p), &yes);
        assert!(!shown[0].fixable);
        assert_eq!(shown[0].cannot_fix.as_deref(), Some("Edit the file in Ubuntu."));
        assert!(shown[0].opens.is_empty());
        assert_eq!(shown[0].distro.as_deref(), Some("Ubuntu"));
        assert!(shown[0].id.ends_with("@Ubuntu"), "{}", shown[0].id);
    }

    #[test]
    fn a_link_import_shows_as_written() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("GEMINI.md");
        write(&file, "see @[notes](sub/gone.md)\n");
        let loads = vec![load(
            "antigravity",
            &p,
            vec![
                file_row(&file),
                dead_row(&file, &p.join("sub").join("gone.md"), "sub/gone.md"),
            ],
        )];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert_eq!(found.len(), 1);
        let shown = items(&found, std::slice::from_ref(&p), &yes);
        assert_eq!(
            shown[0].detail,
            "Line 1: @[notes](sub/gone.md). Antigravity leaves it in as text."
        );
        assert!(shown[0].id.ends_with(".link"), "{}", shown[0].id);
    }

    #[test]
    fn an_import_of_a_managed_or_memory_file_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        for role in [Role::Managed, Role::Memory] {
            let file = p.join("MEMORY.md");
            write(&file, "@gone.md\n");
            let loads = vec![load(
                "claude",
                &p,
                vec![
                    row(&file, role, When::Start, None),
                    dead_row(&file, &p.join("gone.md"), "gone.md"),
                ],
            )];
            let mut cx = Reader::windows();
            assert!(findings(&loads, &mut cx).is_empty());
        }
    }

    fn stale_project(tmp: &Path) -> (PathBuf, PathBuf) {
        let p = proj(tmp);
        write(&p.join("src").join("a.ts"), "x");
        write(&p.join(".gitignore"), "dist/\n");
        write(&p.join("sub").join(".gitignore"), "*.gen.ts\n");
        write(&p.join("docs").join("notes.md"), "x");
        let file = p.join("AGENTS.md");
        write(
            &file,
            "# Notes\n`src/a.ts` exists.\n`src/gone.ts` is gone.\n`dist/app.js` is built.\n`sub/x.gen.ts` is \
             generated.\n`origin/main` is a branch.\n`src/gone` is gone too.\n",
        );
        write(&p.join("docs").join("AGENTS.md"), "see `./notes.md`\n");
        (p, file)
    }

    #[test]
    fn stale_paths_skip_what_exists_and_what_git_ignores() {
        let tmp = tempfile::tempdir().unwrap();
        let (p, file) = stale_project(tmp.path());
        let docs = p.join("docs").join("AGENTS.md");
        let loads = vec![
            load("codex", &p, vec![file_row(&file), file_row(&docs)]),
            load("copilot", &p, vec![file_row(&file)]),
        ];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert_eq!(found.len(), 1, "{found:?}");
        let Finding::StalePaths { file: f, paths, .. } = &found[0] else {
            panic!("not stale paths");
        };
        assert_eq!(f, &file);
        assert_eq!(
            paths,
            &vec![(3, "src/gone.ts".to_string()), (7, "src/gone".to_string())]
        );
        let shown = items(&found, std::slice::from_ref(&p), &yes);
        assert_eq!(shown[0].group, DoctorGroup::Info);
        assert_eq!(shown[0].title, "AGENTS.md in proj names 2 paths that don't exist");
        assert_eq!(shown[0].detail, "Line 3: src/gone.ts. Line 7: src/gone.");
        assert!(!shown[0].fixable);
        assert_eq!(shown[0].opens.len(), 1);
        assert_eq!(shown[0].opens[0].line, Some(3));
        assert_eq!(shown[0].opens[0].label, "Open AGENTS.md");
    }

    #[test]
    fn one_stale_path_is_singular() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("AGENTS.md");
        write(&p.join("src").join("a.ts"), "x");
        write(&file, "`src/gone.ts`\n");
        let loads = vec![load("codex", &p, vec![file_row(&file)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        assert_eq!(shown[0].title, "AGENTS.md in proj names a path that doesn't exist");
    }

    #[test]
    fn six_stale_paths_show_five_and_count_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("AGENTS.md");
        write(&p.join("src").join("a.ts"), "x");
        let text: String = (1..=6).map(|n| format!("`src/gone{n}.ts`\n")).collect();
        write(&file, &text);
        let loads = vec![load("codex", &p, vec![file_row(&file)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        assert_eq!(shown[0].title, "AGENTS.md in proj names 6 paths that don't exist");
        assert!(shown[0].detail.ends_with(" And 1 more."), "{}", shown[0].detail);
        assert!(shown[0].detail.contains("Line 5: src/gone5.ts."));
        assert!(!shown[0].detail.contains("gone6"));
    }

    #[test]
    fn a_user_level_file_and_a_distro_get_no_stale_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        write(&p.join("src").join("a.ts"), "x");
        let home_file = tmp.path().join("home").join(".claude").join("CLAUDE.md");
        write(&home_file, "`src/gone.ts`\n");
        let in_project = p.join("AGENTS.md");
        write(&in_project, "`src/gone.ts`\n");
        let loads = vec![load("claude", &p, vec![file_row(&home_file)])];
        let mut cx = Reader::windows();
        assert!(findings(&loads, &mut cx).is_empty());

        let mut distro = load("codex", &p, vec![file_row(&in_project)]);
        distro.distro = Some("Ubuntu".into());
        let mut cx = Reader::windows();
        assert!(findings(&[distro], &mut cx).is_empty());

        let mut no_project = load("codex", &p, vec![file_row(&in_project)]);
        no_project.project = None;
        let mut cx = Reader::windows();
        assert!(findings(&[no_project], &mut cx).is_empty());
    }

    #[test]
    fn ids_dont_change_between_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let p1 = proj(tmp.path());
        let p2 = tmp.path().join("other");
        fs::create_dir_all(&p2).unwrap();
        let f1 = p1.join("CLAUDE.md");
        let f2 = p2.join("CLAUDE.md");
        write(&f1, "@gone.md\n`src/x.ts`\n");
        write(&f2, "@gone.md\n`src/x.ts`\n");
        write(&p1.join("src").join("a.ts"), "x");
        write(&p2.join("src").join("a.ts"), "x");
        let loads = vec![
            load(
                "claude",
                &p1,
                vec![file_row(&f1), dead_row(&f1, &p1.join("gone.md"), "gone.md")],
            ),
            load(
                "claude",
                &p2,
                vec![file_row(&f2), dead_row(&f2, &p2.join("gone.md"), "gone.md")],
            ),
        ];
        let ids = || {
            let mut cx = Reader::windows();
            findings(&loads, &mut cx).iter().map(id_of).collect::<Vec<_>>()
        };
        let (first, second) = (ids(), ids());
        assert_eq!(first, second);
        assert_eq!(first.len(), 4);
        let unique: std::collections::HashSet<_> = first.iter().collect();
        assert_eq!(unique.len(), 4);
        assert!(first.iter().any(|id| id.starts_with("instrstale.")));
    }

    #[test]
    fn the_check_modules_log_nothing() {
        let needle = concat!("tracing", "::");
        assert!(!include_str!("instr_checks.rs").contains(needle));
        assert!(!include_str!("instr_text.rs").contains(needle));
    }
}
