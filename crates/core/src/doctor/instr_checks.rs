//! What is wrong inside the instruction files (slice 7d2): imports whose file doesn't exist, and backticked paths
//! that don't. `findings` reads the run's loads through the run's reader, so no file is read twice; `items` words
//! them. Nothing here logs: a quoted line is file content, and the logs are exported (decision 14).
//!
//! The module names no CLI's files. What differs between CLIs (import syntax, whether HTML comments are read, what a
//! CLI does with a dead import) comes from each load's `imports` and from `dead_import_drops`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use marshell_protocol::doctor::{DoctorGroup, DoctorItem, DoctorOpen};

use super::checks::{dead_import_drops, display_name};
use super::ext::tokens;
use super::instr_text::{
    Block, Indent, IndentKind, Manager, Said, backticked_paths, blocks, indents, package_managers,
};
use super::instructions::{
    CliLoad, ImportSyntax, LoadedFile, Reader, Role, Text, When, cli_rank, display, folder_name, hash_part, id_tail,
    imports_at, is_within, mask_html_comments, normalize, path_key, tokens_text,
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
        /// The file has bytes that aren't UTF-8, so the fix would refuse it. The reader decodes lossily and puts
        /// U+FFFD where such bytes were, so the text it already returned tells; no second read is needed (a file
        /// that really holds U+FFFD is counted too, and just isn't offered the fix).
        lossy: bool,
        agents: Vec<&'static str>,
    },
    StalePaths {
        seat: Seat,
        file: PathBuf,
        paths: Vec<(usize, String)>,
    },
    /// A file one CLI loads twice at start.
    LoadedTwice {
        seat: Seat,
        agent: &'static str,
        file: PathBuf,
        /// How each of the two loads came about: None by its own name, Some(the importing file) as an import.
        first: Option<PathBuf>,
        second: Option<PathBuf>,
        chars: usize,
    },
    /// Paragraphs that are in two files (`a` and `b`), or twice in one (`a == b`).
    Repeats {
        seat: Seat,
        a: PathBuf,
        b: PathBuf,
        agents: Vec<&'static str>,
        /// How many distinct paragraphs are repeated.
        count: usize,
        /// Where the first repeated paragraph is in `a` and in `b`.
        lines: (usize, usize),
        quote: String,
        chars: usize,
    },
    Clash {
        seat: Seat,
        kind: ClashKind,
        agents: Vec<&'static str>,
        a: Spot,
        b: Spot,
    },
}

/// One line that takes a side in a clash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub file: PathBuf,
    pub line: usize,
    pub says: String,
    pub quote: String,
    /// The file is outside the project the load is for.
    pub user_level: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClashKind {
    Manager,
    TabsSpaces,
    Widths,
}

impl Finding {
    fn seat(&self) -> &Seat {
        match self {
            Finding::DeadImport { seat, .. }
            | Finding::StalePaths { seat, .. }
            | Finding::LoadedTwice { seat, .. }
            | Finding::Repeats { seat, .. }
            | Finding::Clash { seat, .. } => seat,
        }
    }
}

/// Every finding in these loads. `cx` is the reader that made them, so no file is read twice.
pub fn findings(loads: &[CliLoad], cx: &mut Reader) -> Vec<Finding> {
    let mut out = dead_imports(loads, cx);
    out.extend(stale_paths(loads, cx));
    out.extend(loaded_twice(loads, cx));
    let mut texts = Texts::default();
    out.extend(repeats(loads, cx, &mut texts));
    out.extend(clashes(loads, cx, &mut texts));
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

/// Whether a file outside the project sits above it: CLIs walk up past the project, so a parent folder's
/// `CLAUDE.md` is loaded for it and belongs to it, as does a rules file under a dot-folder of a parent (`.claude`,
/// `.github`) that is not the home's own. Only the user's own files (`~/.claude/CLAUDE.md`, a Codex home's
/// `AGENTS.md`) are user-level.
fn above_the_project(path: &Path, project: &Path, home: &Path) -> bool {
    if path.parent().is_some_and(|parent| is_within(project, parent)) {
        return true;
    }
    // Anything under the home is the user's own, whatever folders lie between it and the file (a scratch or CI home
    // can itself sit under a dot-folder of a parent of the project).
    if is_within(path, home) {
        return false;
    }
    let file = path_key(path);
    project.ancestors().skip(1).any(|ancestor| {
        let base = path_key(ancestor);
        let Some(rest) = file.strip_prefix(&base).and_then(|rest| rest.strip_prefix('/')) else {
            return false;
        };
        // A dot-folder with the file somewhere below it.
        rest.split_once('/')
            .is_some_and(|(folder, _)| folder.starts_with('.') && folder != "." && folder != "..")
    })
}

/// For each row of a load, whether its file is a user-level one. A file inside the load's project is the project's;
/// an import is its root file's (the nearest earlier row that isn't an import, as in `looked_at`); a root file
/// outside the project is the project's when it is above it, user-level when not. With no project, all are
/// user-level.
fn user_level_rows(load: &CliLoad) -> Vec<bool> {
    let Some(project) = &load.project else {
        return vec![true; load.files.len()];
    };
    let mut root_user = true;
    load.files
        .iter()
        .map(|file| {
            let inside = is_within(&file.path, project);
            if !matches!(file.role, Role::Import { .. }) {
                root_user = !inside && !above_the_project(&file.path, project, &load.home);
            }
            !inside && root_user
        })
        .collect()
}

/// The seat of a file: the project's, or None for a user-level one.
fn seat_of(load: &CliLoad, user: bool) -> Seat {
    Seat {
        project: load.project.clone().filter(|_| !user),
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
        let users = user_level_rows(load);
        for (i, row) in load.files.iter().enumerate() {
            let (Role::Import { by }, When::NotLoaded(why), Some(raw)) = (&row.role, &row.when, &row.raw) else {
                continue;
            };
            if why != "not found" {
                continue;
            }
            let keys = keys.get_or_insert_with(|| load.files.iter().map(|f| key_of(cx, &f.path)).collect());
            let by_key = key_of(cx, by);
            let Some(by_row) = (0..i).find(|&j| looked[j] && keys[j] == by_key) else {
                continue;
            };
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
                        seat: seat_of(load, users[by_row]),
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
            lossy: text.contains('\u{FFFD}'),
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
                    seat: seat_of(load, false),
                    file: row.path.clone(),
                    paths,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Duplicates and clashes (decisions 5 and 7)

/// The rows these checks look at: loaded at start, looked at (decision 15), and not empty; each with whether its
/// file is a user-level one.
fn start_rows(load: &CliLoad) -> Vec<(&LoadedFile, bool)> {
    looked_at(load)
        .into_iter()
        .zip(user_level_rows(load))
        .zip(&load.files)
        .filter(|((ok, _), file)| *ok && matches!(file.when, When::Start) && file.chars > 0)
        .map(|((_, user), file)| (file, user))
        .collect()
}

/// A file one load holds, once however many rows it has.
struct Held {
    path: PathBuf,
    key: String,
    /// The file is a user-level one (not the project's and not above it).
    user: bool,
}

fn held(load: &CliLoad, cx: &mut Reader) -> Vec<Held> {
    let mut out: Vec<Held> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (row, user) in start_rows(load) {
        let key = key_of(cx, &row.path);
        if seen.insert(key.clone()) {
            out.push(Held {
                path: row.path.clone(),
                key,
                user,
            });
        }
    }
    out
}

/// What the text rules find in one file.
#[derive(Default)]
struct Parsed {
    blocks: Vec<Block>,
    managers: Vec<Said<Manager>>,
    indents: Vec<Said<Indent>>,
}

/// Each file read and parsed once, whatever loads hold it.
#[derive(Default)]
struct Texts {
    by_key: HashMap<String, Parsed>,
}

impl Texts {
    /// Parses what isn't parsed yet. False when the run is out of time; the files after that one are not parsed.
    fn load(&mut self, cx: &mut Reader, files: &[Held]) -> bool {
        for file in files {
            if self.by_key.contains_key(&file.key) {
                continue;
            }
            if cx.out_of_time() {
                return false;
            }
            let parsed = match cx.read(&file.path) {
                Text::Text(text) => Parsed {
                    blocks: blocks(&text),
                    managers: package_managers(&text),
                    indents: indents(&text),
                },
                _ => Parsed::default(),
            };
            self.by_key.insert(file.key.clone(), parsed);
        }
        true
    }

    /// A file `load` has parsed.
    fn of(&self, file: &Held) -> &Parsed {
        &self.by_key[&file.key]
    }
}

/// Files one CLI loads twice at start. A file loaded twice is one finding for each CLI and seat.
fn loaded_twice(loads: &[CliLoad], cx: &mut Reader) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut done: Vec<(&'static str, Seat, String)> = Vec::new();
    for load in loads {
        let rows = start_rows(load);
        let keys: Vec<String> = rows.iter().map(|(row, _)| key_of(cx, &row.path)).collect();
        // The file's first two rows: a third one adds nothing.
        let mut places: HashMap<&str, (usize, usize)> = HashMap::new();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for (j, key) in keys.iter().enumerate() {
            let entry = places.entry(key.as_str()).or_insert((0, j));
            entry.0 += 1;
            if entry.0 == 2 {
                pairs.push((entry.1, j));
            }
        }
        for (i, j) in pairs {
            let seat = seat_of(load, rows[i].1);
            let id = (load.agent, seat.clone(), keys[j].clone());
            if done.contains(&id) {
                continue;
            }
            done.push(id);
            let by = |(row, _): (&LoadedFile, bool)| match &row.role {
                Role::Import { by } => Some(by.clone()),
                _ => None,
            };
            out.push(Finding::LoadedTwice {
                seat,
                agent: load.agent,
                file: rows[i].0.path.clone(),
                first: by(rows[i]),
                second: by(rows[j]),
                chars: rows[j].0.chars,
            });
        }
    }
    out
}

/// The blocks two files, or one file twice, have in common.
struct Shared {
    count: usize,
    lines: (usize, usize),
    quote: String,
    chars: usize,
}

fn tally(out: &mut Option<Shared>, block: &Block, other_line: usize) {
    match out {
        Some(shared) => {
            shared.count += 1;
            shared.chars += block.chars;
        }
        None => {
            *out = Some(Shared {
                count: 1,
                lines: (block.line, other_line),
                quote: block.quote.clone(),
                chars: block.chars,
            });
        }
    }
}

/// The distinct blocks of `a` that `b` has too, in `a`'s order.
fn shared_between(a: &[Block], b: &[Block]) -> Option<Shared> {
    // Each text's first line in `b`, looked up rather than searched: a long file has thousands of blocks.
    let mut first_in_b: HashMap<&str, usize> = HashMap::new();
    for block in b {
        first_in_b.entry(block.folded.as_str()).or_insert(block.line);
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = None;
    for block in a {
        let Some(other_line) = first_in_b.get(block.folded.as_str()) else {
            continue;
        };
        if seen.insert(block.folded.as_str()) {
            tally(&mut out, block, *other_line);
        }
    }
    out
}

/// The distinct blocks a file has in two places, at the first of them.
fn shared_within(blocks: &[Block]) -> Option<Shared> {
    // The line of each text's second copy.
    let mut copies: HashMap<&str, (usize, Option<usize>)> = HashMap::new();
    for block in blocks {
        let entry = copies.entry(block.folded.as_str()).or_insert((0, None));
        entry.0 += 1;
        if entry.0 == 2 {
            entry.1 = Some(block.line);
        }
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = None;
    for block in blocks {
        let Some((_, Some(second))) = copies.get(block.folded.as_str()) else {
            continue;
        };
        if seen.insert(block.folded.as_str()) {
            tally(&mut out, block, *second);
        }
    }
    out
}

struct RepeatGroup {
    seat: Seat,
    keys: (String, String),
    a: PathBuf,
    b: PathBuf,
    agents: Vec<&'static str>,
    shared: Shared,
}

/// Paragraphs in two files one CLI loads at start, or twice in one, grouped across loads by the pair of files.
fn repeats(loads: &[CliLoad], cx: &mut Reader, texts: &mut Texts) -> Vec<Finding> {
    let mut groups: Vec<RepeatGroup> = Vec::new();
    for load in loads {
        let files = held(load, cx);
        if !texts.load(cx, &files) {
            break;
        }
        for (x, fx) in files.iter().enumerate() {
            // `files` holds a file once, so a file loaded twice in this load is not paired with itself.
            for fy in &files[x..] {
                let (bx, by) = (&texts.of(fx).blocks, &texts.of(fy).blocks);
                let shared = if fx.key == fy.key {
                    shared_within(bx)
                } else {
                    shared_between(bx, by)
                };
                let Some(shared) = shared else { continue };
                let seat = seat_of(load, fx.user && fy.user);
                let keys = if fx.key <= fy.key {
                    (fx.key.clone(), fy.key.clone())
                } else {
                    (fy.key.clone(), fx.key.clone())
                };
                match groups.iter_mut().find(|g| g.seat == seat && g.keys == keys) {
                    Some(group) => {
                        if !group.agents.contains(&load.agent) {
                            group.agents.push(load.agent);
                        }
                    }
                    None => groups.push(RepeatGroup {
                        seat,
                        keys,
                        a: fx.path.clone(),
                        b: fy.path.clone(),
                        agents: vec![load.agent],
                        shared,
                    }),
                }
            }
        }
    }
    groups
        .into_iter()
        .map(|mut group| {
            group.agents.sort_by_key(|a| cli_rank(a));
            Finding::Repeats {
                seat: group.seat,
                a: group.a,
                b: group.b,
                agents: group.agents,
                count: group.shared.count,
                lines: group.shared.lines,
                quote: group.shared.quote,
                chars: group.shared.chars,
            }
        })
        .collect()
}

const CLASH_KINDS: [ClashKind; 3] = [ClashKind::Manager, ClashKind::TabsSpaces, ClashKind::Widths];

/// A line that takes a side: `file` is an index into the load's held files.
struct Side {
    file: usize,
    line: usize,
    says: String,
    quote: String,
}

/// The lines of a load that say something of this kind, in the load's order. Two sides clash when what they say
/// differs.
fn sides(kind: ClashKind, files: &[Held], texts: &Texts) -> Vec<Side> {
    let mut out = Vec::new();
    for (file, held) in files.iter().enumerate() {
        let parsed = texts.of(held);
        let side = |line: usize, says: String, quote: &str| Side {
            file,
            line,
            says,
            quote: quote.to_string(),
        };
        match kind {
            ClashKind::Manager => out.extend(
                parsed
                    .managers
                    .iter()
                    .map(|s| side(s.line, s.what.name().to_string(), &s.quote)),
            ),
            ClashKind::TabsSpaces => out.extend(parsed.indents.iter().filter_map(|s| {
                let says = match s.what.kind? {
                    IndentKind::Tabs => "tabs",
                    IndentKind::Spaces => "spaces",
                };
                Some(side(s.line, says.to_string(), &s.quote))
            })),
            ClashKind::Widths => out.extend(
                parsed
                    .indents
                    .iter()
                    .filter_map(|s| Some(side(s.line, format!("width {}", s.what.width?), &s.quote))),
            ),
        }
    }
    // Two lines for each thing a file says are enough to find every first pair (the second is for a line that says
    // two things, which can't clash with itself), and they keep the scan small however long a file is.
    let mut kept: HashMap<(usize, String), usize> = HashMap::new();
    out.retain(|side| {
        let lines = kept.entry((side.file, side.says.clone())).or_default();
        *lines += 1;
        *lines <= 2
    });
    out
}

/// Package managers, tabs against spaces and indent widths that clash in what one CLI loads at start, user-level files
/// included. One finding for each kind and seat; the CLIs are taken in the Doctor's order and the first with a pair
/// gives it.
fn clashes(loads: &[CliLoad], cx: &mut Reader, texts: &mut Texts) -> Vec<Finding> {
    let mut order: Vec<&CliLoad> = loads.iter().collect();
    order.sort_by_key(|load| cli_rank(load.agent));
    let helds: Vec<Vec<Held>> = order.iter().map(|load| held(load, cx)).collect();
    let ready: Vec<bool> = helds.iter().map(|files| texts.load(cx, files)).collect();
    let mut done: Vec<(ClashKind, Seat)> = Vec::new();
    let mut out = Vec::new();
    for (n, load) in order.iter().enumerate() {
        if !ready[n] {
            continue;
        }
        let files = &helds[n];
        let outside = |file: &Held| file.user;
        for kind in CLASH_KINDS {
            let all = sides(kind, files, texts);
            // The first pair for the project's item, then for the user-level one: by the later line's place in the
            // load, then the earlier's.
            let mut best: [Option<(usize, usize)>; 2] = [None, None];
            'scan: for j in 1..all.len() {
                for i in 0..j {
                    let (a, b) = (&all[i], &all[j]);
                    if a.says == b.says || (a.file == b.file && a.line == b.line) {
                        continue;
                    }
                    let user = usize::from(outside(&files[a.file]) && outside(&files[b.file]));
                    best[user].get_or_insert((i, j));
                    if best.iter().all(Option::is_some) {
                        break 'scan;
                    }
                }
            }
            for (user, pair) in best.into_iter().enumerate() {
                let Some((i, j)) = pair else { continue };
                let seat = seat_of(load, user == 1);
                if done.contains(&(kind, seat.clone())) {
                    continue;
                }
                done.push((kind, seat.clone()));
                let (ka, kb) = (&files[all[i].file].key, &files[all[j].file].key);
                let mut agents: Vec<&'static str> = Vec::new();
                for (m, other) in order.iter().enumerate() {
                    let same_place = other.distro == load.distro
                        && (seat.project.is_none()
                            || other.project.as_deref().map(path_key) == seat.project.as_deref().map(path_key));
                    let holds = |key: &String| helds[m].iter().any(|h| &h.key == key);
                    if same_place && holds(ka) && holds(kb) && !agents.contains(&other.agent) {
                        agents.push(other.agent);
                    }
                }
                let spot = |side: &Side| Spot {
                    file: files[side.file].path.clone(),
                    line: side.line,
                    says: side.says.clone(),
                    quote: side.quote.clone(),
                    user_level: outside(&files[side.file]),
                };
                out.push(Finding::Clash {
                    seat,
                    kind,
                    agents,
                    a: spot(&all[i]),
                    b: spot(&all[j]),
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
        Finding::LoadedTwice { agent, file, .. } => {
            format!("instrtwice.{agent}.{place}.{}{suffix}", hash_part(file))
        }
        Finding::Repeats { a, b, .. } => {
            let (x, y) = if path_key(a) <= path_key(b) { (a, b) } else { (b, a) };
            format!("instrrepeat.{place}.{}.{}{suffix}", hash_part(x), hash_part(y))
        }
        Finding::Clash { kind, .. } => {
            let kind = match kind {
                ClashKind::Manager => "manager",
                ClashKind::TabsSpaces => "tabs",
                ClashKind::Widths => "widths",
            };
            format!("instrclash.{place}.{kind}{suffix}")
        }
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

fn opens(seat: &Seat, file: &Path, line: Option<usize>) -> Vec<DoctorOpen> {
    if seat.distro.is_some() {
        return Vec::new();
    }
    let name = file
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    vec![DoctorOpen {
        path: open_path(file),
        line: line.and_then(|line| u32::try_from(line).ok()),
        label: format!("Open {name}"),
    }]
}

/// The buttons for a finding about two files. Two files with one name (a project's `AGENTS.md` and the user's own)
/// would get the same label, so both then say their path as the item's detail shows it.
fn opens_of_two(seat: &Seat, a: (&Path, usize), b: (&Path, usize)) -> Vec<DoctorOpen> {
    let mut out = opens(seat, a.0, Some(a.1));
    out.extend(opens(seat, b.0, Some(b.1)));
    if let [first, second] = out.as_mut_slice()
        && first.label == second.label
    {
        first.label = format!("Open {}", display(a.0, seat.project.as_deref(), &seat.home));
        second.label = format!("Open {}", display(b.0, seat.project.as_deref(), &seat.home));
    }
    out
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
            lossy,
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
                None if editable(file) && !*lossy => {
                    item.fixable = true;
                    item.unchecked = true;
                }
                None => item.cannot_fix = Some("Marshell doesn't change this file.".to_string()),
            }
            item.opens = opens(seat, file, Some(*line));
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
            item.opens = opens(seat, file, paths.first().map(|(line, _)| *line));
        }
        Finding::LoadedTwice {
            seat,
            agent,
            file,
            first,
            second,
            chars,
        } => {
            let name = |path: &PathBuf| display(path, seat.project.as_deref(), &seat.home);
            let how = match (first, second) {
                (None, Some(by)) | (Some(by), None) => {
                    format!("Once by its own name and once as an import of {}", name(by))
                }
                (Some(a), Some(b)) => format!("Once as an import of {} and once as an import of {}", name(a), name(b)),
                (None, None) => "Twice by its own name".to_string(),
            };
            item.agent = Some((*agent).to_string());
            let place = seat
                .project
                .as_deref()
                .map_or_else(String::new, |project| format!(" in {}", folder_name(project)));
            item.title = format!("{} loads {} twice{place}", display_name(agent), name(file));
            item.detail = format!("{how}. About {} tokens are sent twice.", tokens_text(tokens(*chars)));
            item.opens = opens(seat, file, None);
        }
        Finding::Repeats {
            seat,
            a,
            b,
            agents,
            count,
            lines,
            quote,
            chars,
        } => {
            let paragraphs = match count {
                1 => "a paragraph".to_string(),
                n => format!("{n} paragraphs"),
            };
            let names: Vec<&str> = agents.iter().map(|a| display_name(a)).collect();
            let verb = if names.len() == 1 { "loads" } else { "load" };
            let name = |path: &PathBuf| display(path, seat.project.as_deref(), &seat.home);
            let sent = format!("About {} tokens are sent twice.", tokens_text(tokens(*chars)));
            if path_key(a) == path_key(b) {
                item.title = format!("{} repeats {paragraphs}", shown(seat, a));
                item.detail = format!(
                    "{} {verb} it. The first is at lines {} and {}: “{quote}”. {sent}",
                    list(&names),
                    lines.0,
                    lines.1
                );
                item.opens = opens(seat, a, Some(lines.0));
            } else {
                item.title = format!("{} and {} repeat {paragraphs}", name(a), shown(seat, b));
                item.detail = format!(
                    "{} {verb} both. The first is at {}:{} and {}:{}: “{quote}”. {sent}",
                    list(&names),
                    name(a),
                    lines.0,
                    name(b),
                    lines.1
                );
                item.opens = opens_of_two(seat, (a, lines.0), (b, lines.1));
            }
        }
        Finding::Clash {
            seat,
            kind,
            agents,
            a,
            b,
        } => {
            let what = match kind {
                ClashKind::Manager => "Two package managers are named",
                ClashKind::TabsSpaces => "Tabs and spaces are both asked for",
                ClashKind::Widths => "Two indent widths are asked for",
            };
            let place = match &seat.project {
                Some(project) => format!("in {}", folder_name(project)),
                None => "at user level".to_string(),
            };
            let said = |spot: &Spot| {
                format!(
                    "{}:{}{} says {}: “{}”.",
                    display(&spot.file, seat.project.as_deref(), &seat.home),
                    spot.line,
                    if spot.user_level && seat.project.is_some() {
                        " (user level)"
                    } else {
                        ""
                    },
                    spot.says,
                    spot.quote
                )
            };
            let names: Vec<&str> = agents.iter().map(|a| display_name(a)).collect();
            let verb = if names.len() == 1 { "loads" } else { "load" };
            item.title = format!("{what} {place}");
            item.detail = format!("{} {} {} {verb} both.", said(a), said(b), list(&names));
            item.opens = if path_key(&a.file) == path_key(&b.file) {
                opens(seat, &a.file, Some(a.line))
            } else {
                opens_of_two(seat, (&a.file, a.line), (&b.file, b.line))
            };
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
    use crate::doctor::instr_text::quote;
    use crate::doctor::instructions::{CliLoad, ImportStyle, LoadedFile, Reader, Role, When};
    use marshell_protocol::doctor::{DoctorGroup, DoctorItem, DoctorOpen};
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

    fn file_chars(path: &Path, role: Role, raw: Option<&str>, chars: usize) -> LoadedFile {
        LoadedFile {
            chars,
            ..row(path, role, When::Start, raw)
        }
    }

    /// One line of exactly `n` characters made of words, which ends in a letter.
    fn para(n: usize, seed: &str) -> String {
        let mut text: String = format!("{seed} lorem ipsum dolor ").chars().cycle().take(n).collect();
        text.pop();
        text.push('z');
        text
    }

    fn of_kind<'a>(shown: &'a [DoctorItem], prefix: &str) -> Vec<&'a DoctorItem> {
        shown.iter().filter(|i| i.id.starts_with(prefix)).collect()
    }

    #[test]
    fn a_file_loaded_by_name_and_as_an_import_is_loaded_twice() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let claude = p.join("CLAUDE.md");
        let agents = p.join("AGENTS.md");
        write(&claude, "@AGENTS.md\n");
        write(&agents, &para(1000, "alpha"));
        let loads = vec![load(
            "copilot",
            &p,
            vec![
                file_row(&claude),
                file_chars(&agents, Role::Import { by: claude.clone() }, Some("AGENTS.md"), 1000),
                file_chars(&agents, Role::File, None, 1000),
            ],
        )];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert!(!found.iter().any(|f| matches!(f, Finding::Repeats { .. })), "{found:?}");
        let shown = items(&found, std::slice::from_ref(&p), &yes);
        let twice = of_kind(&shown, "instrtwice.");
        assert_eq!(twice.len(), 1, "{shown:?}");
        let item = twice[0];
        assert_eq!(item.title, "Copilot CLI loads AGENTS.md twice in proj");
        assert_eq!(
            item.detail,
            "Once by its own name and once as an import of CLAUDE.md. About 250 tokens are sent twice."
        );
        assert_eq!(item.group, DoctorGroup::Info);
        assert!(!item.fixable);
        assert_eq!(item.cannot_fix, None);
        assert_eq!(item.agent.as_deref(), Some("copilot"));
        assert_eq!(item.opens.len(), 1);
        assert_eq!(item.opens[0].label, "Open AGENTS.md");
        assert!(item.id.starts_with("instrtwice.copilot."), "{}", item.id);
        assert!(!item.id.contains('@'));
    }

    #[test]
    fn a_paragraph_in_two_files_is_a_repeat() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let (claude, agents) = (p.join("CLAUDE.md"), p.join("AGENTS.md"));
        let text = para(220, "beta");
        write(&claude, &format!("# T\n\n{text}\n"));
        write(&agents, &format!("a\n\nb\n\nc\n\n{text}\n"));
        let loads = vec![
            load("claude", &p, vec![file_row(&claude), file_row(&agents)]),
            load("copilot", &p, vec![file_row(&agents), file_row(&claude)]),
        ];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        let shown = items(&found, std::slice::from_ref(&p), &yes);
        let repeats = of_kind(&shown, "instrrepeat.");
        assert_eq!(repeats.len(), 1, "{shown:?}");
        let item = repeats[0];
        assert_eq!(item.title, "CLAUDE.md and AGENTS.md in proj repeat a paragraph");
        assert_eq!(
            item.detail,
            format!(
                "Claude Code and Copilot CLI load both. The first is at CLAUDE.md:3 and AGENTS.md:7: “{}”. About 55 \
                 tokens are sent twice.",
                quote(&text)
            )
        );
        assert!(
            item.detail
                .starts_with("Claude Code and Copilot CLI load both. The first is at CLAUDE.md:3 and AGENTS.md:7: “")
        );
        assert!(item.detail.ends_with("About 55 tokens are sent twice."));
        assert_eq!(item.group, DoctorGroup::Info);
        assert!(!item.fixable);
        assert_eq!(item.agent, None);
        assert_eq!(item.opens.len(), 2);
        assert_eq!(
            (item.opens[0].label.as_str(), item.opens[0].line),
            ("Open CLAUDE.md", Some(3))
        );
        assert_eq!(
            (item.opens[1].label.as_str(), item.opens[1].line),
            ("Open AGENTS.md", Some(7))
        );
        assert!(
            !shown.iter().any(|i| i.id.starts_with("instrtwice.")),
            "two different files are not loaded twice"
        );
    }

    #[test]
    fn three_repeated_paragraphs_are_plural() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let (claude, agents) = (p.join("CLAUDE.md"), p.join("AGENTS.md"));
        let text = format!(
            "{}\n\n{}\n\n{}\n",
            para(220, "one"),
            para(220, "two"),
            para(220, "three")
        );
        write(&claude, &text);
        write(&agents, &text);
        let loads = vec![load("claude", &p, vec![file_row(&claude), file_row(&agents)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let repeats = of_kind(&shown, "instrrepeat.");
        assert_eq!(repeats.len(), 1);
        assert_eq!(repeats[0].title, "CLAUDE.md and AGENTS.md in proj repeat 3 paragraphs");
        assert!(
            repeats[0].detail.ends_with("About 165 tokens are sent twice."),
            "{}",
            repeats[0].detail
        );
    }

    #[test]
    fn a_paragraph_twice_in_one_file() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let claude = p.join("CLAUDE.md");
        let text = para(240, "gamma");
        write(
            &claude,
            &format!("{}{text}\n{}{text}\n", "\n".repeat(11), "\n".repeat(27)),
        );
        let loads = vec![load("claude", &p, vec![file_row(&claude)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let repeats = of_kind(&shown, "instrrepeat.");
        assert_eq!(repeats.len(), 1, "{shown:?}");
        assert_eq!(repeats[0].title, "CLAUDE.md in proj repeats a paragraph");
        assert_eq!(
            repeats[0].detail,
            format!(
                "Claude Code loads it. The first is at lines 12 and 40: “{}”. About 60 tokens are sent twice.",
                quote(&text)
            )
        );
        assert_eq!(repeats[0].opens.len(), 1);
        assert_eq!(repeats[0].opens[0].line, Some(12));
    }

    #[test]
    fn two_package_managers_clash_and_the_user_level_one_is_marked() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let user = tmp.path().join("home").join(".claude").join("CLAUDE.md");
        let agents = p.join("AGENTS.md");
        write(&user, "Always run npm install first.\n");
        write(&agents, "# Rules\nUse pnpm for the frontend.\n");
        let loads = vec![load("claude", &p, vec![file_row(&user), file_row(&agents)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        let item = clashes[0];
        assert_eq!(item.title, "Two package managers are named in proj");
        assert_eq!(
            item.detail,
            "~/.claude/CLAUDE.md:1 (user level) says npm: “Always run npm install first.”. AGENTS.md:2 says pnpm: \
             “Use pnpm for the frontend.”. Claude Code loads both."
        );
        assert_eq!(item.group, DoctorGroup::Info);
        assert!(!item.fixable);
        assert_eq!(item.agent, None);
        assert_eq!(item.opens.len(), 2);
        assert_eq!(item.opens[0].line, Some(1));
        assert_eq!(item.opens[1].line, Some(2));
        assert!(item.id.ends_with(".manager"), "{}", item.id);
    }

    #[test]
    fn two_files_with_one_name_get_buttons_that_tell_them_apart() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let root = p.join("AGENTS.md");
        let nested = p.join("docs").join("AGENTS.md");
        write(&root, "Use pnpm for the frontend.\n");
        write(&nested, "Always run npm install first.\n");
        let loads = vec![load("codex", &p, vec![file_row(&root), file_row(&nested)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        let labels: Vec<&str> = clashes[0].opens.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["Open AGENTS.md", "Open docs/AGENTS.md"]);
    }

    #[test]
    fn a_long_file_keeps_only_the_lines_that_can_clash() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let agents = p.join("AGENTS.md");
        let mut text = "Use pnpm for the frontend.\n".repeat(5_000);
        text.push_str("Run npm install first.\n");
        write(&agents, &text);
        let loads = vec![load("claude", &p, vec![file_row(&agents)])];
        let mut cx = Reader::windows();
        let files = held(&loads[0], &mut cx);
        let mut texts = Texts::default();
        assert!(texts.load(&mut cx, &files));
        // Two lines for each thing a file says are enough to find every first pair, however long the file is.
        assert_eq!(sides(ClashKind::Manager, &files, &texts).len(), 3);
        let clash = findings(&loads, &mut cx).into_iter().find_map(|f| match f {
            Finding::Clash { a, b, .. } => Some((a.line, b.line)),
            _ => None,
        });
        assert_eq!(clash, Some((1, 5_001)));
    }

    #[test]
    fn a_line_that_names_two_managers_clashes_with_a_later_line_only() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let agents = p.join("AGENTS.md");
        write(&agents, "Run npm install or pnpm install.\n\nRun npm install first.\n");
        let loads = vec![load("claude", &p, vec![file_row(&agents)])];
        let mut cx = Reader::windows();
        let clash = findings(&loads, &mut cx).into_iter().find_map(|f| match f {
            Finding::Clash { a, b, .. } => Some((a.says, a.line, b.says, b.line)),
            _ => None,
        });
        assert_eq!(clash, Some(("pnpm".to_string(), 1, "npm".to_string(), 3)));
    }

    #[test]
    fn many_copies_of_one_paragraph_are_one_repeat() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let agents = p.join("AGENTS.md");
        let one = format!("{}\n\n", para(240, "same"));
        write(&agents, &one.repeat(300));
        let loads = vec![load("claude", &p, vec![file_row(&agents)])];
        let mut cx = Reader::windows();
        let repeat = findings(&loads, &mut cx).into_iter().find_map(|f| match f {
            Finding::Repeats { count, lines, .. } => Some((count, lines)),
            _ => None,
        });
        assert_eq!(repeat, Some((1, (1, 3))));
    }

    #[test]
    fn tabs_against_spaces_and_two_widths_are_their_own_items() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let agents = p.join("AGENTS.md");
        write(
            &agents,
            "Indent with tabs.\nUse 2 spaces for indentation.\nindent width 4\n",
        );
        let loads = vec![load("claude", &p, vec![file_row(&agents)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 2, "{shown:?}");
        let tabs = clashes.iter().find(|i| i.id.contains(".tabs")).expect("tabs");
        assert_eq!(tabs.title, "Tabs and spaces are both asked for in proj");
        assert_eq!(
            tabs.detail,
            "AGENTS.md:1 says tabs: “Indent with tabs.”. AGENTS.md:2 says spaces: “Use 2 spaces for indentation.”. \
             Claude Code loads both."
        );
        assert_eq!(tabs.opens.len(), 1);
        let widths = clashes.iter().find(|i| i.id.contains(".widths")).expect("widths");
        assert_eq!(widths.title, "Two indent widths are asked for in proj");
        assert_eq!(
            widths.detail,
            "AGENTS.md:2 says width 2: “Use 2 spaces for indentation.”. AGENTS.md:3 says width 4: “indent width 4”. \
             Claude Code loads both."
        );
    }

    #[test]
    fn two_clis_with_different_pairs_are_one_item() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let (claude, agents, sub) = (
            p.join("CLAUDE.md"),
            p.join("AGENTS.md"),
            p.join("sub").join("AGENTS.md"),
        );
        write(&claude, "Use pnpm for the frontend.\n");
        write(&agents, "Always run npm install first.\n");
        write(&sub, "Use yarn for everything.\n");
        let loads = vec![
            load("codex", &p, vec![file_row(&agents), file_row(&sub)]),
            load("claude", &p, vec![file_row(&claude), file_row(&agents)]),
        ];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        assert_eq!(
            clashes[0].detail,
            "CLAUDE.md:1 says pnpm: “Use pnpm for the frontend.”. AGENTS.md:1 says npm: “Always run npm install \
             first.”. Claude Code loads both."
        );
        assert!(clashes[0].id.starts_with("instrclash."));
        assert!(clashes[0].id.ends_with(".manager"));
    }

    #[test]
    fn a_clash_between_user_level_files_is_one_item_at_user_level() {
        let tmp = tempfile::tempdir().unwrap();
        let (p1, p2) = (proj(tmp.path()), tmp.path().join("proj2"));
        fs::create_dir_all(&p2).unwrap();
        let rules = tmp.path().join("home").join(".claude");
        let (u1, u2) = (rules.join("CLAUDE.md"), rules.join("rules").join("style.md"));
        write(&u1, "Always run npm install first.\n");
        write(&u2, "Use pnpm for the frontend.\n");
        let (a1, a2) = (p1.join("AGENTS.md"), p2.join("AGENTS.md"));
        write(&a1, "Use yarn for it.\n");
        write(&a2, "Use yarn for it.\n");
        let loads = vec![
            load("claude", &p1, vec![file_row(&u1), file_row(&u2), file_row(&a1)]),
            load("claude", &p2, vec![file_row(&u1), file_row(&u2), file_row(&a2)]),
        ];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), &[p1.clone(), p2.clone()], &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 3, "{shown:?}");
        assert_eq!(clashes[0].title, "Two package managers are named in proj");
        assert_eq!(clashes[1].title, "Two package managers are named in proj2");
        assert_eq!(clashes[2].title, "Two package managers are named at user level");
        assert_eq!(clashes[2].id, "instrclash.user.manager");
        assert_eq!(
            clashes[2].detail,
            "~/.claude/CLAUDE.md:1 says npm: “Always run npm install first.”. ~/.claude/rules/style.md:1 says pnpm: \
             “Use pnpm for the frontend.”. Claude Code loads both."
        );
        assert_eq!(
            clashes[0].detail,
            "~/.claude/CLAUDE.md:1 (user level) says npm: “Always run npm install first.”. AGENTS.md:1 says yarn: \
             “Use yarn for it.”. Claude Code loads both."
        );
    }

    #[test]
    fn memory_and_managed_files_are_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let agents = p.join("AGENTS.md");
        let other = tmp.path().join("home").join(".claude").join("MEMORY.md");
        write(&agents, "Use pnpm for the frontend.\n");
        write(&other, "Always run npm install first.\n");
        for role in [Role::Memory, Role::Managed] {
            let loads = vec![load(
                "claude",
                &p,
                vec![file_row(&agents), row(&other, role, When::Start, None)],
            )];
            let mut cx = Reader::windows();
            assert!(findings(&loads, &mut cx).is_empty());
        }
    }

    #[test]
    fn a_distro_gets_duplicates_and_clashes() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let dir = tmp.path().join("home").join(".claude");
        let (u1, u2) = (dir.join("CLAUDE.md"), dir.join("style.md"));
        write(&u1, "Always run npm install first.\n");
        write(&u2, "Use pnpm for the frontend.\n");
        let mut distro = load("claude", &p, vec![file_row(&u1), file_row(&u2)]);
        distro.distro = Some("Ubuntu".into());
        let mut cx = Reader::windows();
        let shown = items(&findings(&[distro], &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        assert_eq!(clashes[0].title, "Two package managers are named at user level");
        assert!(clashes[0].id.ends_with("@Ubuntu"), "{}", clashes[0].id);
        assert_eq!(clashes[0].distro.as_deref(), Some("Ubuntu"));
        assert!(clashes[0].opens.is_empty());
    }

    #[test]
    fn a_dead_import_in_a_file_that_is_not_utf8_is_not_offered_a_fix() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let file = p.join("CLAUDE.md");
        fs::write(&file, b"Caf\xE9\n@docs/old.md\n").unwrap();
        let target = p.join("docs").join("old.md");
        let loads = vec![load(
            "claude",
            &p,
            vec![file_row(&file), dead_row(&file, &target, "docs/old.md")],
        )];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        assert_eq!(shown.len(), 1, "{shown:?}");
        assert!(!shown[0].fixable);
        assert_eq!(
            shown[0].cannot_fix.as_deref(),
            Some("Marshell doesn't change this file.")
        );
    }

    #[test]
    fn a_file_above_the_project_is_the_projects_and_not_marked_user_level() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let above = tmp.path().join("CLAUDE.md");
        let agents = p.join("AGENTS.md");
        write(&above, "Always run npm install first.\n");
        write(&agents, "Use pnpm for the frontend.\n");
        let loads = vec![load("claude", &p, vec![file_row(&above), file_row(&agents)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        assert_eq!(clashes[0].title, "Two package managers are named in proj");
        assert!(!clashes[0].detail.contains("(user level)"), "{}", clashes[0].detail);
    }

    #[test]
    fn two_files_above_the_project_that_clash_are_the_projects_item() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        // A parent folder's file, and one under a dot-folder of that parent (a worktree's `.claude` rules).
        let (a, b) = (
            tmp.path().join("CLAUDE.md"),
            tmp.path().join(".claude").join("rules").join("style.md"),
        );
        write(&a, "Always run npm install first.\n");
        write(&b, "Use pnpm for the frontend.\n");
        let loads = vec![load("claude", &p, vec![file_row(&a), file_row(&b)])];
        let mut cx = Reader::windows();
        let shown = items(&findings(&loads, &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        assert_eq!(clashes[0].title, "Two package managers are named in proj");
        assert!(!clashes[0].detail.contains("(user level)"), "{}", clashes[0].detail);
    }

    #[test]
    fn a_user_file_under_a_home_that_holds_the_project_is_still_user_level() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let p = home.join("proj");
        fs::create_dir_all(&p).unwrap();
        let user = home.join(".claude").join("CLAUDE.md");
        let agents = p.join("AGENTS.md");
        write(&user, "Always run npm install first.\n");
        write(&agents, "Use pnpm for the frontend.\n");
        let mut l = load("claude", &p, vec![file_row(&user), file_row(&agents)]);
        l.home = home;
        let mut cx = Reader::windows();
        let shown = items(&findings(&[l], &mut cx), std::slice::from_ref(&p), &yes);
        let clashes = of_kind(&shown, "instrclash.");
        assert_eq!(clashes.len(), 1, "{shown:?}");
        assert_eq!(clashes[0].title, "Two package managers are named in proj");
        assert!(clashes[0].detail.contains("(user level)"), "{}", clashes[0].detail);
    }

    #[test]
    fn a_dead_import_in_a_file_above_the_project_sits_with_the_project() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let above = tmp.path().join("CLAUDE.md");
        write(&above, "@gone.md\n");
        let target = tmp.path().join("gone.md");
        let loads = vec![load(
            "claude",
            &p,
            vec![file_row(&above), dead_row(&above, &target, "gone.md")],
        )];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        assert_eq!(found.len(), 1, "{found:?}");
        let Finding::DeadImport { seat, .. } = &found[0] else {
            panic!("not a dead import: {found:?}");
        };
        assert_eq!(seat.project.as_deref(), Some(p.as_path()));
    }

    #[test]
    fn an_import_follows_its_root_files_place_and_repeats_and_twice_use_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = proj(tmp.path());
        let (a, b) = (tmp.path().join("CLAUDE.md"), tmp.path().join("AGENTS.md"));
        let text = para(220, "gamma");
        write(&a, &format!("@AGENTS.md\n\n{text}\n"));
        write(&b, &format!("{text}\n"));
        let loads = vec![load(
            "copilot",
            &p,
            vec![
                file_row(&a),
                file_chars(&b, Role::Import { by: a.clone() }, Some("AGENTS.md"), 1000),
                file_chars(&b, Role::File, None, 1000),
            ],
        )];
        let mut cx = Reader::windows();
        let found = findings(&loads, &mut cx);
        let seats: Vec<&Seat> = found
            .iter()
            .filter(|f| matches!(f, Finding::Repeats { .. } | Finding::LoadedTwice { .. }))
            .map(Finding::seat)
            .collect();
        assert_eq!(seats.len(), 2, "{found:?}");
        assert!(
            seats.iter().all(|s| s.project.as_deref() == Some(p.as_path())),
            "{seats:?}"
        );
    }

    #[test]
    fn the_check_modules_log_nothing() {
        let needle = concat!("tracing", "::");
        assert!(!include_str!("instr_checks.rs").contains(needle));
        assert!(!include_str!("instr_text.rs").contains(needle));
    }
}
