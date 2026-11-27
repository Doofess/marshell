//! The library planner (slice 8a): judges every copy of every doc at one level (a user home, a distro's home or a
//! project) and plans each change as config-service file edits. Nothing here writes to disk.
//!
//! Which files get written or deleted is decided here, so every path is read only when it is a plain file
//! (`copy::read_disk`), every edit path passes `editable`, and a copy the person edited or a file Marshell didn't write
//! is left as it is unless the pick names it.

use super::copy::{Disk, Judged, Unwanted, copy_body, file_copy, judge_file, judge_unwanted, read_disk, source_of};
use super::doc::{self, CLIS, Doc};
use super::roots::Roots;
use crate::agents::codex::library as codex;
use crate::agents::{antigravity, claude, copilot, qwen};
use crate::doctor::instructions::display;
use marshell_protocol::config::{Applies, ChangeSet, ConfigScope, EditOp, FileEdit};
use marshell_protocol::instructions::{CopyRow, CopyState, DocScope};
use std::path::{Path, PathBuf};

const NOT_WRITABLE: &str = "Marshell may not write here, so this copy is left out.";
const BROKEN: &str = "The doc's frontmatter can't be read, so its copies are left as they are.";
/// Decision 40's and 27's sentences start "Not installed on this PC"; in a distro they name the distro instead, since
/// the CLI may well be installed on Windows.
fn not_installed(distro: Option<&str>) -> String {
    format!("{}, so it gets no copy.", not_installed_where(distro))
}

fn teammates(distro: Option<&str>) -> String {
    format!("{}. The copy is for teammates who use it.", not_installed_where(distro))
}

fn not_installed_where(distro: Option<&str>) -> String {
    match distro {
        Some(d) => format!("Not installed in {d}"),
        None => "Not installed on this PC".to_string(),
    }
}
const NO_GLOBS: &str = "Codex has no globs, so this doc doesn't load in Codex.";
const NOT_TICKED: &str = "Not ticked for this doc.";
const OFF: &str = "This doc is off, so it gets no copy.";
const SHARED: &str = "Copilot CLI reads .claude/rules, so it loads this too.";
const CODEX_USER_WARNING: &str = "Your developer_instructions in ~/.codex/config.toml stop loading in this project, because the project sets its own.";
const EDITED: &str = "It was edited, so Marshell leaves it; delete it yourself if you don't need it.";

/// Where a level's copies live.
#[derive(Debug, Clone)]
pub enum Level {
    /// A user home: the Windows one, or one WSL distro's.
    User { roots: Roots, distro: Option<String> },
    /// One project's folder.
    Project { root: PathBuf, notes: Notes },
}

/// Decision 17's sentences for a project, read by the caller.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Notes {
    pub codex: Option<String>,
    pub codex_user_warning: bool,
    pub qwen: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Place {
    pub level: Level,
    /// The level's docs: user docs at a user level, the project's docs in a project.
    pub docs: Vec<Doc>,
    /// The CLIs found where the level is.
    pub found: Vec<&'static str>,
}

/// One physical copy. A copy shared by Claude Code and Copilot CLI is listed under `claude`; Codex's copies share their
/// file and differ by `doc`.
#[derive(Debug, Clone, PartialEq)]
pub struct CopyAt {
    pub doc: String,
    pub cli: &'static str,
    pub path: PathBuf,
    pub state: CopyState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelState {
    /// Per doc name, in doc order: one row per CLI in CLI order, then a row for each unwanted copy no CLI row shows.
    pub rows: Vec<(String, Vec<CopyRow>)>,
    /// Copies of docs that are gone.
    pub orphans: Vec<CopyRow>,
    pub out_of_date: u32,
    pub changed_outside: u32,
    pub copies: Vec<CopyAt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    /// Every out-of-date, missing and extra copy.
    Review,
    /// The Doctor's ticked fixes: Review's edits when `update` is true, and the named changed-outside copies put back.
    Fixes {
        update: bool,
        put_back: Vec<(String, &'static str)>,
    },
    /// That doc's copies only, as if the doc were gone.
    Delete(String),
    /// One changed-outside copy.
    PutBack(String, &'static str),
}

/// A CLI's rule files, from its adapter's `library.rs`.
struct Vendor {
    cli: &'static str,
    user_dir: fn(&Roots) -> PathBuf,
    project_dir: fn(&Path) -> PathBuf,
    file_name: fn(&str) -> String,
    own_name: fn(&str) -> Option<String>,
    frontmatter: fn(&[String]) -> String,
}

/// The CLIs whose copies are one file each, in the order of `CLIS`.
static VENDORS: [Vendor; 4] = [
    Vendor {
        cli: "claude",
        user_dir: claude::library::user_dir,
        project_dir: claude::library::project_dir,
        file_name: claude::library::file_name,
        own_name: claude::library::own_name,
        frontmatter: claude::library::frontmatter,
    },
    Vendor {
        cli: "antigravity",
        user_dir: antigravity::library::user_dir,
        project_dir: antigravity::library::project_dir,
        file_name: antigravity::library::file_name,
        own_name: antigravity::library::own_name,
        frontmatter: antigravity::library::frontmatter,
    },
    Vendor {
        cli: "qwen",
        user_dir: qwen::library::user_dir,
        project_dir: qwen::library::project_dir,
        file_name: qwen::library::file_name,
        own_name: qwen::library::own_name,
        frontmatter: qwen::library::frontmatter,
    },
    Vendor {
        cli: "copilot",
        user_dir: copilot::library::user_dir,
        project_dir: copilot::library::project_dir,
        file_name: copilot::library::file_name,
        own_name: copilot::library::own_name,
        frontmatter: copilot::library::frontmatter,
    },
];

fn vendor(cli: &str) -> Option<&'static Vendor> {
    VENDORS.iter().find(|v| v.cli == cli)
}

/// The CLI id from the list, for a name from a caller.
fn cli_id(cli: &str) -> Option<&'static str> {
    CLIS.iter().copied().find(|c| *c == cli)
}

fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn doc_scope(doc: &Doc) -> ConfigScope {
    match doc.scope {
        DocScope::User => ConfigScope::Marshell,
        DocScope::Project => ConfigScope::Project,
    }
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

struct Ctx<'a> {
    place: &'a Place,
    user_home: &'a Path,
    editable: &'a dyn Fn(&Path) -> bool,
}

impl Ctx<'_> {
    fn is_user(&self) -> bool {
        matches!(self.place.level, Level::User { .. })
    }

    fn project(&self) -> Option<&Path> {
        match &self.place.level {
            Level::Project { root, .. } => Some(root),
            Level::User { .. } => None,
        }
    }

    fn home(&self) -> &Path {
        match &self.place.level {
            Level::User { roots, .. } => &roots.home,
            Level::Project { .. } => self.user_home,
        }
    }

    fn show(&self, path: &Path) -> String {
        display(path, self.project(), self.home())
    }

    fn scope(&self) -> ConfigScope {
        if self.is_user() {
            ConfigScope::User
        } else {
            ConfigScope::Project
        }
    }

    fn distro(&self) -> Option<String> {
        match &self.place.level {
            Level::User { distro, .. } => distro.clone(),
            Level::Project { .. } => None,
        }
    }

    fn found(&self, cli: &'static str) -> bool {
        self.place.found.contains(&cli)
    }

    /// The distro the level is in: a distro user level's, or the distro of a project under its share.
    fn found_in(&self) -> Option<String> {
        match &self.place.level {
            Level::User { distro, .. } => distro.clone(),
            Level::Project { root, .. } => root.to_str().and_then(crate::wsl::share_distro).map(str::to_string),
        }
    }

    fn dir(&self, v: &Vendor) -> PathBuf {
        match &self.place.level {
            Level::User { roots, .. } => (v.user_dir)(roots),
            Level::Project { root, .. } => (v.project_dir)(root),
        }
    }

    /// The guard admits the path and, in a project, the path stays in the project once its links are resolved: a
    /// cloned repo's `.claude` linked to another folder must not take copies, or lose files, outside the repo. A link
    /// at user level is the person's own setup (dotfiles) and is followed.
    fn can_write(&self, path: &Path) -> bool {
        (self.editable)(path)
            && match &self.place.level {
                Level::Project { root, .. } => stays_inside(path, root),
                Level::User { .. } => true,
            }
    }

    fn row(&self, cli: &str, path: Option<&Path>, state: CopyState, notes: Vec<String>, shared: bool) -> CopyRow {
        CopyRow {
            cli: Some(cli.to_string()),
            distro: self.distro(),
            path: path.map(|p| self.show(p)),
            state,
            notes,
            shared,
            fixes: false,
        }
    }

    /// The doc's name as a copy's source line shows it.
    fn source(&self, doc: &Doc) -> String {
        source_of(doc, self.project(), self.user_home)
    }

    /// Notes a wanted copy's row carries whatever its state.
    fn context_notes(&self, doc: &Doc, cli: &'static str, size: usize) -> Vec<String> {
        let mut notes = Vec::new();
        if !self.is_user() && !self.found(cli) {
            notes.push(teammates(self.found_in().as_deref()));
        }
        if cli == "antigravity" {
            if size > antigravity::library::RULE_CAP {
                notes.push(format!(
                    "Antigravity CLI cuts a rule past {} bytes, and this copy is {} bytes.",
                    thousands(antigravity::library::RULE_CAP),
                    thousands(size)
                ));
            }
            if let Some(glob) = doc.globs.first() {
                notes.push(format!(
                    "Antigravity CLI matches globs anywhere in the path, so {glob} also matches a deeper folder of that name."
                ));
            }
        }
        if let Level::Project { notes: n, .. } = &self.place.level {
            if cli == "codex" {
                notes.extend(n.codex.clone());
                if n.codex_user_warning {
                    notes.push(CODEX_USER_WARNING.to_string());
                }
            }
            if cli == "qwen" {
                notes.extend(n.qwen.clone());
            }
        }
        notes
    }
}

/// Whether `path` stays inside `root` once the part of it that exists is resolved (links and junctions followed): a
/// copy not written yet is placed by its deepest folder that exists.
pub(crate) fn stays_inside(path: &Path, root: &Path) -> bool {
    let mut existing = path;
    while std::fs::symlink_metadata(existing).is_err() {
        match existing.parent() {
            Some(parent) => existing = parent,
            None => return false,
        }
    }
    // There, but not followable: a link whose target is gone, so where a write through it lands can't be known.
    if std::fs::metadata(existing).is_err() {
        return false;
    }
    let resolved = crate::projects::canonical(existing);
    crate::projects::is_inside(&resolved.to_string_lossy(), &root.to_string_lossy())
}

/// What a doc wants in one CLI.
enum Want {
    Copy,
    /// No copy, and why.
    No(String),
    /// Copilot CLI in a project whose doc is for Claude Code: it reads `.claude/rules`.
    Shared,
    /// The doc can't be read: its copies are left as they are.
    Broken,
}

fn want(ctx: &Ctx, doc: &Doc, cli: &'static str) -> Want {
    if doc.error.is_some() {
        return Want::Broken;
    }
    if doc.scope == DocScope::Project && !doc.enabled {
        return Want::No(OFF.to_string());
    }
    let ticked = |c: &'static str| doc.clis.contains(&c);
    if !ctx.is_user() && cli == "copilot" && ticked("claude") {
        return Want::Shared;
    }
    if !ticked(cli) {
        return Want::No(NOT_TICKED.to_string());
    }
    if cli == "codex" && !doc.globs.is_empty() {
        return Want::No(NO_GLOBS.to_string());
    }
    if ctx.is_user() && !ctx.found(cli) {
        return Want::No(not_installed(ctx.found_in().as_deref()));
    }
    Want::Copy
}

enum Kind {
    /// A copy the doc wants, with the file text (or Codex block) it should hold.
    Wanted(String),
    /// A copy no doc wants; `ours` when its hash holds, so Marshell wrote it and nobody edited it.
    Unwanted { ours: bool },
    /// A copy of a doc that can't be read, left as it is; `ours` as above, for a delete.
    Kept { ours: bool },
}

struct Item {
    doc: String,
    cli: &'static str,
    path: PathBuf,
    state: CopyState,
    kind: Kind,
    /// The file's text now, None when it doesn't exist (or is Codex's).
    text: Option<String>,
    hash: Option<String>,
    /// What the copy says now, for `take_copy_plan`.
    body: Option<String>,
    notes: Vec<String>,
}

impl Item {
    fn wanted(&self) -> bool {
        matches!(self.kind, Kind::Wanted(_))
    }

    /// The copy exists and its hash holds: a delete may take it.
    fn ours(&self) -> bool {
        match &self.kind {
            Kind::Wanted(_) => matches!(self.state, CopyState::InSync | CopyState::OutOfDate),
            Kind::Unwanted { ours } | Kind::Kept { ours } => *ours,
        }
    }
}

/// A Codex file Marshell reads blocks from.
struct CFile {
    path: PathBuf,
    text: Option<String>,
    blocks: Vec<codex::Block>,
    /// Why the file can't be written, as a wanted row's note: its blocks are neither written nor judged.
    refused: Option<String>,
}

struct Analysis {
    items: Vec<Item>,
    rows: Vec<(String, Vec<CopyRow>)>,
    orphans: Vec<CopyRow>,
    cfiles: Vec<CFile>,
}

fn cfile(ctx: &Ctx, path: PathBuf, sections: bool) -> CFile {
    let disk = if ctx.can_write(&path) {
        read_disk(&path)
    } else {
        Disk::Refused
    };
    let (text, mut refused) = match disk {
        Disk::Missing => (None, None),
        Disk::Refused => (None, Some(NOT_WRITABLE.to_string())),
        Disk::Text { text, .. } => (Some(text), None),
    };
    if refused.is_none() && !ctx.can_write(&path) {
        refused = Some(NOT_WRITABLE.to_string());
    }
    let mut blocks = Vec::new();
    if let (None, Some(t)) = (&refused, &text) {
        if sections {
            match codex::sections_in(t) {
                Ok(found) => blocks = found,
                Err(e) => refused = Some(e),
            }
        } else {
            blocks = codex::blocks_in(t);
        }
    }
    CFile {
        path,
        text,
        blocks,
        refused,
    }
}

/// Codex's files at the level: the one wanted blocks go to first, then (at a user level) the other one.
fn codex_files(ctx: &Ctx) -> Vec<CFile> {
    match &ctx.place.level {
        Level::User { roots, .. } => vec![
            cfile(ctx, codex::user_file(&roots.codex), false),
            cfile(ctx, codex::other_user_file(&roots.codex), false),
        ],
        Level::Project { root, .. } => vec![cfile(ctx, codex::project_config(root), true)],
    }
}

fn state_of(j: Judged) -> CopyState {
    match j {
        Judged::InSync => CopyState::InSync,
        Judged::OutOfDate => CopyState::OutOfDate,
        Judged::ChangedOutside => CopyState::ChangedOutside,
        Judged::Taken => CopyState::Taken,
    }
}

/// An existing copy no doc wants now (or a broken doc's): None when Marshell didn't write it.
#[allow(clippy::too_many_arguments)]
fn unwanted_item(
    doc: &str,
    cli: &'static str,
    path: &Path,
    text: Option<String>,
    hash: Option<String>,
    body: Option<String>,
    judged: Unwanted,
    broken: bool,
) -> Option<Item> {
    let ours = match judged {
        Unwanted::NotOurs => return None,
        Unwanted::Ours => true,
        Unwanted::Edited => false,
    };
    Some(Item {
        doc: doc.to_string(),
        cli,
        path: path.to_path_buf(),
        state: if ours {
            CopyState::Extra
        } else {
            CopyState::ChangedOutside
        },
        kind: if broken {
            Kind::Kept { ours }
        } else {
            Kind::Unwanted { ours }
        },
        text,
        hash,
        body,
        notes: Vec::new(),
    })
}

fn file_items(ctx: &Ctx, doc: &Doc, v: &Vendor, w: &Want) -> Vec<Item> {
    let path = ctx.dir(v).join((v.file_name)(&doc.name));
    let disk = if ctx.can_write(&path) {
        read_disk(&path)
    } else {
        Disk::Refused
    };
    if let Want::Copy = w {
        let content = file_copy(&(v.frontmatter)(&doc.globs), &doc.name, &ctx.source(doc), &doc.body);
        let mut notes = Vec::new();
        let (state, text, hash, body) = match disk {
            Disk::Refused => (CopyState::Refused, None, None, None),
            _ if !ctx.can_write(&path) => (CopyState::Refused, None, None, None),
            Disk::Missing => (CopyState::Missing, None, None, None),
            Disk::Text { text, hash } => {
                let state = state_of(judge_file(&content, &text, &doc.name));
                let body = copy_body(&text);
                (state, Some(text), Some(hash), body)
            }
        };
        match state {
            CopyState::Refused => notes.push(NOT_WRITABLE.to_string()),
            CopyState::Taken => notes.push(format!(
                "{} isn't a copy Marshell wrote, so Marshell leaves it.",
                ctx.show(&path)
            )),
            _ => {}
        }
        notes.extend(ctx.context_notes(doc, v.cli, content.len()));
        return vec![Item {
            doc: doc.name.clone(),
            cli: v.cli,
            path,
            state,
            kind: Kind::Wanted(content),
            text,
            hash,
            body,
            notes,
        }];
    }
    let broken = matches!(w, Want::Broken);
    match disk {
        Disk::Text { text, hash } if ctx.can_write(&path) => {
            let judged = judge_unwanted(&text, &doc.name);
            unwanted_item(&doc.name, v.cli, &path, Some(text), Some(hash), None, judged, broken)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn codex_items(ctx: &Ctx, cfiles: &[CFile], doc: &Doc, w: &Want) -> Vec<Item> {
    let mut items = Vec::new();
    let broken = matches!(w, Want::Broken);
    let mut skip = None;
    if let Want::Copy = w {
        let target = &cfiles[0];
        let text = codex::block_text(&doc.name, &ctx.source(doc), &doc.body);
        let expected = codex::blocks_in(&text).pop().map(|b| b.body).unwrap_or_default();
        let found = target.blocks.iter().find(|b| same(&b.name, &doc.name));
        let (state, body) = match (&target.refused, found) {
            (Some(_), _) => (CopyState::Refused, None),
            (None, None) => (CopyState::Missing, None),
            (None, Some(b)) => (state_of(codex::judge_block(&expected, b)), Some(b.body.clone())),
        };
        let mut notes: Vec<String> = target.refused.iter().cloned().collect();
        notes.extend(ctx.context_notes(doc, "codex", 0));
        items.push(Item {
            doc: doc.name.clone(),
            cli: "codex",
            path: target.path.clone(),
            state,
            kind: Kind::Wanted(text),
            text: None,
            hash: None,
            body,
            notes,
        });
        skip = Some(0);
    }
    for (i, cf) in cfiles.iter().enumerate() {
        if skip == Some(i) || cf.refused.is_some() {
            continue;
        }
        if let Some(b) = cf.blocks.iter().find(|b| same(&b.name, &doc.name)) {
            let judged = codex::judge_unwanted_block(b);
            items.extend(unwanted_item(
                &doc.name, "codex", &cf.path, None, None, None, judged, broken,
            ));
        }
    }
    items
}

/// An unwanted copy's notes: why the doc doesn't want it, and that an edited one stays.
fn unwanted_notes(it: &Item, why: Option<&String>) -> Vec<String> {
    let mut notes: Vec<String> = why.into_iter().cloned().collect();
    if matches!(it.kind, Kind::Unwanted { ours: false }) {
        notes.push(EDITED.to_string());
    }
    notes
}

/// A doc's rows and its items. `all` holds the items so far (the shared row reads Claude Code's).
fn doc_rows(ctx: &Ctx, cfiles: &[CFile], all: &mut Vec<Item>, doc: &Doc) -> Vec<CopyRow> {
    if doc.duplicate {
        // It wants nothing and holds nothing: its name is the first doc's, whose copies those are.
        let why = doc.error.clone().unwrap_or_default();
        return CLIS
            .iter()
            .map(|cli| ctx.row(cli, None, CopyState::NoCopy, vec![why.clone()], false))
            .collect();
    }
    let start = all.len();
    let mut rows = Vec::new();
    let mut used: Vec<usize> = Vec::new();
    for cli in CLIS {
        let w = want(ctx, doc, cli);
        let first = all.len();
        let items = match vendor(cli) {
            Some(v) => file_items(ctx, doc, v, &w),
            None => codex_items(ctx, cfiles, doc, &w),
        };
        all.extend(items);
        let mine = first..all.len();
        let row = match &w {
            Want::Broken => ctx.row(cli, None, CopyState::NoCopy, vec![BROKEN.to_string()], false),
            Want::Shared => {
                let claude = all[start..]
                    .iter()
                    .find(|i| i.cli == "claude" && i.wanted())
                    .expect("a doc for Claude Code has a Claude Code copy");
                ctx.row(cli, Some(&claude.path), claude.state, vec![SHARED.to_string()], true)
            }
            Want::Copy => {
                let at = mine
                    .clone()
                    .find(|&i| all[i].wanted())
                    .expect("a wanted copy has an item");
                used.push(at);
                let it = &all[at];
                let mut row = ctx.row(cli, Some(&it.path), it.state, it.notes.clone(), false);
                row.fixes = it.state == CopyState::ChangedOutside;
                row
            }
            Want::No(why) => match mine.clone().find(|&i| !all[i].wanted()) {
                Some(at) => {
                    used.push(at);
                    let it = &all[at];
                    ctx.row(cli, Some(&it.path), it.state, unwanted_notes(it, Some(why)), false)
                }
                None => ctx.row(cli, None, CopyState::NoCopy, vec![why.clone()], false),
            },
        };
        rows.push(row);
    }
    // An unwanted copy no CLI row shows (the `.github` copy beside a shared Copilot CLI row, a block in Codex's other
    // user file) gets a row of its own, so Review never removes what the page doesn't list.
    for (at, it) in all.iter().enumerate().skip(start) {
        if matches!(it.kind, Kind::Unwanted { .. }) && !used.contains(&at) {
            rows.push(ctx.row(it.cli, Some(&it.path), it.state, unwanted_notes(it, None), false));
        }
    }
    rows
}

fn orphan_row(ctx: &Ctx, it_cli: &str, path: &Path, name: &str, ours: bool) -> CopyRow {
    let note = if ours {
        format!("From {name}, a doc that is gone. Review removes it.")
    } else {
        format!(
            "From {name}, a doc that is gone. It was edited, so Marshell leaves it; delete it yourself if you don't need it."
        )
    };
    let state = if ours {
        CopyState::Extra
    } else {
        CopyState::ChangedOutside
    };
    ctx.row(it_cli, Some(path), state, vec![note], false)
}

/// Copies whose doc is gone, in the CLI's folder or Codex's files. A name that any doc has, without case, belongs
/// to that doc, so a `marshell-security-old.md` backup (whose own name is `security-old`) is no orphan and a file whose
/// name differs only by case is never both a wanted copy and an orphan.
fn orphans(ctx: &Ctx, cfiles: &[CFile], items: &mut Vec<Item>) -> Vec<CopyRow> {
    let known = |name: &str| ctx.place.docs.iter().any(|d| same(&d.name, name));
    let mut rows = Vec::new();
    for cli in CLIS {
        match vendor(cli) {
            Some(v) => {
                let Ok(entries) = std::fs::read_dir(ctx.dir(v)) else {
                    continue;
                };
                let mut files: Vec<(String, PathBuf)> = entries
                    .flatten()
                    .filter_map(|e| Some((e.file_name().to_str()?.to_string(), e.path())))
                    .collect();
                files.sort();
                for (file, path) in files {
                    let Some(name) = (v.own_name)(&file) else {
                        continue;
                    };
                    if known(&name) || !ctx.can_write(&path) {
                        continue;
                    }
                    let Disk::Text { text, hash } = read_disk(&path) else {
                        continue;
                    };
                    let judged = judge_unwanted(&text, &name);
                    if let Some(it) = unwanted_item(&name, v.cli, &path, Some(text), Some(hash), None, judged, false) {
                        rows.push(orphan_row(ctx, v.cli, &path, &name, it.ours()));
                        if it.ours() {
                            items.push(it);
                        }
                    }
                }
            }
            None => {
                for cf in cfiles.iter().filter(|c| c.refused.is_none()) {
                    for b in cf.blocks.iter().filter(|b| !known(&b.name)) {
                        let judged = codex::judge_unwanted_block(b);
                        if let Some(it) = unwanted_item(&b.name, "codex", &cf.path, None, None, None, judged, false) {
                            rows.push(orphan_row(ctx, "codex", &cf.path, &b.name, it.ours()));
                            if it.ours() {
                                items.push(it);
                            }
                        }
                    }
                }
            }
        }
    }
    rows
}

fn analyze(place: &Place, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> (Analysis, ConfigScope) {
    let ctx = Ctx {
        place,
        user_home,
        editable,
    };
    let cfiles = codex_files(&ctx);
    let mut items = Vec::new();
    let mut rows = Vec::new();
    for doc in &place.docs {
        let doc_rows = doc_rows(&ctx, &cfiles, &mut items, doc);
        rows.push((doc.name.clone(), doc_rows));
    }
    let orphans = orphans(&ctx, &cfiles, &mut items);
    (
        Analysis {
            items,
            rows,
            orphans,
            cfiles,
        },
        ctx.scope(),
    )
}

/// How each copy of each doc at the level stands, as the page shows it.
pub fn level_state(place: &Place, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> LevelState {
    let (a, _) = analyze(place, user_home, editable);
    let copies: Vec<CopyAt> = a
        .items
        .iter()
        .filter(|i| !matches!(i.kind, Kind::Kept { .. } | Kind::Unwanted { ours: false }))
        .map(|i| CopyAt {
            doc: i.doc.clone(),
            cli: i.cli,
            path: i.path.clone(),
            state: i.state,
        })
        .collect();
    let count = |states: &[CopyState]| copies.iter().filter(|c| states.contains(&c.state)).count() as u32;
    LevelState {
        out_of_date: count(&[CopyState::Missing, CopyState::OutOfDate, CopyState::Extra]),
        changed_outside: count(&[CopyState::ChangedOutside]),
        rows: a.rows,
        orphans: a.orphans,
        copies,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    Write,
    Remove,
    Keep,
}

fn review_fate(it: &Item) -> Fate {
    match (&it.kind, it.state) {
        (Kind::Wanted(_), CopyState::Missing | CopyState::OutOfDate) => Fate::Write,
        (Kind::Unwanted { ours: true }, _) => Fate::Remove,
        _ => Fate::Keep,
    }
}

fn put_back_fate(it: &Item, doc: &str, cli: &str) -> Fate {
    if it.wanted() && it.state == CopyState::ChangedOutside && same(&it.doc, doc) && it.cli == cli {
        Fate::Write
    } else {
        Fate::Keep
    }
}

fn fate(it: &Item, pick: &Pick) -> Fate {
    match pick {
        Pick::Review => review_fate(it),
        Pick::Fixes { update, put_back } => {
            let reviewed = if *update { review_fate(it) } else { Fate::Keep };
            if reviewed != Fate::Keep {
                return reviewed;
            }
            if put_back
                .iter()
                .any(|(doc, cli)| put_back_fate(it, doc, cli) == Fate::Write)
            {
                Fate::Write
            } else {
                Fate::Keep
            }
        }
        Pick::Delete(doc) => {
            if same(&it.doc, doc) && it.ours() {
                Fate::Remove
            } else {
                Fate::Keep
            }
        }
        Pick::PutBack(doc, cli) => put_back_fate(it, doc, cli),
    }
}

fn edit(scope: ConfigScope, path: &Path, op: EditOp, expected_hash: Option<String>) -> FileEdit {
    FileEdit {
        path: path.to_string_lossy().into_owned(),
        scope,
        op,
        expected_hash,
        applies: Applies::NewSessions,
    }
}

/// The edit that makes the file `new`: none when it already holds exactly that.
fn write_edit(
    scope: ConfigScope,
    path: &Path,
    new: &str,
    now: &Option<String>,
    hash: &Option<String>,
) -> Option<FileEdit> {
    match now {
        Some(t) if t == new => None,
        Some(_) => Some(edit(
            scope,
            path,
            EditOp::ReplaceText {
                content: new.to_string(),
            },
            hash.clone(),
        )),
        None => Some(edit(
            scope,
            path,
            EditOp::CreateFile {
                content: new.to_string(),
            },
            None,
        )),
    }
}

/// `force` names a wanted copy to write whatever its state: a Codex block whose text the doc just took still holds the
/// old hash.
fn edits_for(
    ctx_scope: ConfigScope,
    project_level: bool,
    a: &Analysis,
    pick: &Pick,
    force: Option<(&str, &str)>,
) -> Vec<FileEdit> {
    let fate_of = |i: &Item| {
        if force.is_some_and(|(doc, cli)| i.wanted() && same(&i.doc, doc) && i.cli == cli) {
            Fate::Write
        } else {
            fate(i, pick)
        }
    };
    let mut edits = Vec::new();
    for it in a.items.iter().filter(|i| i.cli != "codex") {
        match (fate_of(it), &it.kind) {
            (Fate::Write, Kind::Wanted(content)) => {
                edits.extend(write_edit(ctx_scope, &it.path, content, &it.text, &it.hash));
            }
            (Fate::Remove, _) if it.hash.is_some() => {
                edits.push(edit(ctx_scope, &it.path, EditOp::DeleteFile, it.hash.clone()));
            }
            _ => {}
        }
    }
    for cf in a.cfiles.iter().filter(|c| c.refused.is_none()) {
        let mine: Vec<(&Item, Fate)> = a
            .items
            .iter()
            .filter(|i| i.cli == "codex" && i.path == cf.path)
            .map(|i| (i, fate_of(i)))
            .collect();
        // A file nothing changes in is never rewritten: its text is only judged, never re-rendered.
        if mine.iter().all(|(_, f)| *f == Fate::Keep) {
            continue;
        }
        let mut wanted = Vec::new();
        let mut keep = Vec::new();
        for b in &cf.blocks {
            let f = mine
                .iter()
                .find(|(i, _)| same(&i.doc, &b.name))
                .map_or(Fate::Keep, |(_, f)| *f);
            if f == Fate::Keep {
                keep.push(b.name.clone());
            }
        }
        for (it, f) in &mine {
            if let (Fate::Write, Kind::Wanted(text)) = (f, &it.kind) {
                wanted.push((it.doc.clone(), text.clone()));
            }
        }
        let hash = cf.text.as_ref().map(|t| crate::config::hash::hash_bytes(t.as_bytes()));
        let new = if project_level {
            match codex::with_sections(cf.text.as_deref(), &wanted, &keep) {
                Ok(n) => n,
                Err(_) => continue,
            }
        } else {
            codex::with_blocks(cf.text.as_deref().unwrap_or(""), &wanted, &keep)
        };
        match new {
            Some(text) => edits.extend(write_edit(ctx_scope, &cf.path, &text, &cf.text, &hash)),
            None if cf.text.is_some() => edits.push(edit(ctx_scope, &cf.path, EditOp::DeleteFile, hash)),
            None => {}
        }
    }
    edits
}

/// One edit per file for what `pick` changes at the level.
pub fn plan_edits(place: &Place, pick: &Pick, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> Vec<FileEdit> {
    let (a, scope) = analyze(place, user_home, editable);
    edits_for(scope, !matches!(place.level, Level::User { .. }), &a, pick, None)
}

pub fn review_plan(places: &[Place], user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> ChangeSet {
    ChangeSet {
        title: "Update the copies".to_string(),
        reason: "Write each library doc's copies as the doc is now.".to_string(),
        edits: places
            .iter()
            .flat_map(|p| plan_edits(p, &Pick::Review, user_home, editable))
            .collect(),
    }
}

/// The doc's own file: a user doc is in the Marshell home, a project doc in its repo.
pub fn save_plan(doc: &Doc, old: Option<&Doc>, expected: Option<&str>) -> ChangeSet {
    let scope = doc_scope(doc);
    let content = doc::render(doc);
    let mut edits = Vec::new();
    match old {
        None => edits.push(edit(scope, &doc.path, EditOp::CreateFile { content }, None)),
        Some(old) if old.path == doc.path => {
            let hash = expected.map_or_else(|| old.hash.clone(), str::to_string);
            edits.push(edit(scope, &doc.path, EditOp::ReplaceText { content }, Some(hash)));
        }
        Some(old) => {
            let hash = expected.map_or_else(|| old.hash.clone(), str::to_string);
            edits.push(edit(scope, &doc.path, EditOp::CreateFile { content }, None));
            edits.push(edit(doc_scope(old), &old.path, EditOp::DeleteFile, Some(hash)));
        }
    }
    ChangeSet {
        title: format!("Save {}", doc.name),
        reason: "Write the doc's file.".to_string(),
        edits,
    }
}

/// The doc's file once, then every copy of it that is still as Marshell wrote it, in each place.
pub fn delete_plan(places: &[Place], name: &str, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> ChangeSet {
    let doc = places.iter().find_map(|p| doc::find(&p.docs, name));
    let mut edits = Vec::new();
    if let Some(d) = doc {
        edits.push(edit(doc_scope(d), &d.path, EditOp::DeleteFile, Some(d.hash.clone())));
    }
    for p in places {
        edits.extend(plan_edits(p, &Pick::Delete(name.to_string()), user_home, editable));
    }
    ChangeSet {
        title: format!("Delete {} and its copies", doc.map_or(name, |d| d.name.as_str())),
        reason: "Remove the doc and the copies Marshell wrote of it.".to_string(),
        edits,
    }
}

/// The wanted copy of `name` in `cli` when it is ChangedOutside now.
fn changed<'a>(a: &'a Analysis, name: &str, cli: &str) -> Option<&'a Item> {
    a.items
        .iter()
        .find(|i| i.wanted() && i.state == CopyState::ChangedOutside && same(&i.doc, name) && i.cli == cli)
}

pub fn put_back_plan(
    place: &Place,
    name: &str,
    cli: &str,
    user_home: &Path,
    editable: &dyn Fn(&Path) -> bool,
) -> Option<ChangeSet> {
    let cli = cli_id(cli)?;
    let (a, scope) = analyze(place, user_home, editable);
    let it = changed(&a, name, cli)?;
    let pick = Pick::PutBack(name.to_string(), cli);
    let edits = edits_for(scope, !matches!(place.level, Level::User { .. }), &a, &pick, None);
    if edits.is_empty() {
        return None;
    }
    let ctx = Ctx {
        place,
        user_home,
        editable,
    };
    Some(ChangeSet {
        title: format!("Put the doc's text back in {}", ctx.show(&it.path)),
        reason: "Write the doc's text over what was changed outside Marshell.".to_string(),
        edits,
    })
}

/// Why "Use this copy's text" has no plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CantTake {
    /// The copy is no longer changed outside Marshell, or its doc or CLI is gone.
    Gone,
    /// The copy's text holds a line Marshell's copies use as a marker, which a doc can't hold (as a save refuses one).
    Marker,
}

/// The copy's text becomes the doc's, and the copy is written again from the doc as it will be read back.
pub fn take_copy_plan(
    place: &Place,
    name: &str,
    cli: &str,
    user_home: &Path,
    editable: &dyn Fn(&Path) -> bool,
) -> Result<ChangeSet, CantTake> {
    let cli = cli_id(cli).ok_or(CantTake::Gone)?;
    let (a, _) = analyze(place, user_home, editable);
    let it = changed(&a, name, cli).ok_or(CantTake::Gone)?;
    let body = it.body.clone().ok_or(CantTake::Gone)?;
    if super::copy::first_marker(&body).is_some() {
        return Err(CantTake::Marker);
    }
    let doc = doc::find(&place.docs, name).ok_or(CantTake::Gone)?;
    let mut taken = doc.clone();
    taken.body = body;
    let rendered = doc::render(&taken);
    // The copy is planned from what the doc will read back as: reading trims the body's trailing blank lines.
    let reread = doc::parse(&doc.name, doc.scope, doc.path.clone(), rendered.as_bytes());
    if reread.error.is_some() {
        return Err(CantTake::Gone);
    }
    let docs = place
        .docs
        .iter()
        .map(|d| {
            if same(&d.name, &doc.name) {
                reread.clone()
            } else {
                d.clone()
            }
        })
        .collect();
    let after = Place { docs, ..place.clone() };
    let (planned, scope) = analyze(&after, user_home, editable);
    let pick = Pick::PutBack(doc.name.clone(), cli);
    let project = !matches!(place.level, Level::User { .. });
    let copy_edits = edits_for(scope, project, &planned, &pick, Some((&doc.name, cli)));
    let ctx = Ctx {
        place,
        user_home,
        editable,
    };
    let mut edits = vec![edit(
        doc_scope(doc),
        &doc.path,
        EditOp::ReplaceText { content: rendered },
        Some(doc.hash.clone()),
    )];
    edits.extend(copy_edits);
    Ok(ChangeSet {
        title: format!("Use the text of {} in {}", ctx.show(&it.path), doc.name),
        reason: "Make the copy's text the doc's. Its other copies stay out of date until Review.".to_string(),
        edits,
    })
}

pub struct Uninstall {
    pub changes: ChangeSet,
    /// Copies and blocks that were edited after Marshell wrote them, so they stay.
    pub left: Vec<String>,
}

/// Removes every copy Marshell wrote in the four user folders and Codex's two user files: a file or block whose hash
/// holds is Ours and goes; an edited one stays and is listed; a file Marshell didn't write is never touched or listed.
pub fn uninstall_plan(roots: &Roots) -> Uninstall {
    let show = |p: &Path| display(p, None, &roots.home);
    let mut edits = Vec::new();
    let mut left = Vec::new();
    for v in &VENDORS {
        let Ok(entries) = std::fs::read_dir((v.user_dir)(roots)) else {
            continue;
        };
        let mut files: Vec<(String, PathBuf)> = entries
            .flatten()
            .filter_map(|e| Some((e.file_name().to_str()?.to_string(), e.path())))
            .collect();
        files.sort();
        for (file, path) in files {
            let Some(name) = (v.own_name)(&file) else {
                continue;
            };
            let Disk::Text { text, hash } = read_disk(&path) else {
                continue;
            };
            match judge_unwanted(&text, &name) {
                Unwanted::Ours => edits.push(edit(ConfigScope::User, &path, EditOp::DeleteFile, Some(hash))),
                Unwanted::Edited => left.push(format!("Left {}: it was edited after Marshell wrote it.", show(&path))),
                Unwanted::NotOurs => {}
            }
        }
    }
    for path in [codex::user_file(&roots.codex), codex::other_user_file(&roots.codex)] {
        let Disk::Text { text, hash } = read_disk(&path) else {
            continue;
        };
        let mut keep = Vec::new();
        let mut ours = false;
        for b in codex::blocks_in(&text) {
            match codex::judge_unwanted_block(&b) {
                Unwanted::Ours => ours = true,
                _ => {
                    left.push(format!(
                        "Left the {} block in {}: it was edited after Marshell wrote it.",
                        b.name,
                        show(&path)
                    ));
                    keep.push(b.name);
                }
            }
        }
        if !ours {
            continue;
        }
        match codex::with_blocks(&text, &[], &keep) {
            None => edits.push(edit(ConfigScope::User, &path, EditOp::DeleteFile, Some(hash))),
            Some(new) if new != text => edits.push(edit(
                ConfigScope::User,
                &path,
                EditOp::ReplaceText { content: new },
                Some(hash),
            )),
            Some(_) => {}
        }
    }
    Uninstall {
        changes: ChangeSet {
            title: "Remove the instruction library's copies".to_string(),
            reason: "Take out each copy Marshell wrote in the CLIs' folders.".to_string(),
            edits,
        },
        left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::codex::library::block_text;
    use crate::config::hash::hash_file;
    use crate::instructions::copy::file_copy;
    use crate::instructions::doc::{self, CLIS};
    use marshell_protocol::config::EditOp;
    use marshell_protocol::instructions::DocScope;
    use std::fs;

    const ALL: [&str; 5] = CLIS;
    const CLAUDE: &str = ".claude/rules/marshell-security.md";
    const AGY: &str = ".agents/rules/marshell-security.md";
    const QWEN: &str = ".qwen/rules/marshell-security.md";
    const CODEX: &str = ".codex/config.toml";
    const GITHUB: &str = ".github/instructions/marshell-security.instructions.md";
    const DOC: &str = ".marshell/instructions/security.md";

    fn yes(_: &Path) -> bool {
        true
    }

    fn user() -> &'static Path {
        Path::new("/u")
    }

    fn project_doc(root: &Path, name: &str, text: &str) -> Doc {
        let path = root.join(".marshell").join("instructions").join(format!("{name}.md"));
        doc::parse(name, DocScope::Project, path, text.as_bytes())
    }

    fn security(root: &Path) -> Doc {
        project_doc(root, "security", "Be careful.\n")
    }

    fn place(root: &Path, docs: Vec<Doc>, found: &[&'static str]) -> Place {
        Place {
            level: Level::Project {
                root: root.to_path_buf(),
                notes: Notes::default(),
            },
            docs,
            found: found.to_vec(),
        }
    }

    fn state(p: &Place) -> LevelState {
        level_state(p, user(), &yes)
    }

    fn review(p: &Place) -> Vec<FileEdit> {
        plan_edits(p, &Pick::Review, user(), &yes)
    }

    /// Writes each edit to disk, checking that its hash is the file's hash now.
    fn apply(edits: &[FileEdit]) {
        for e in edits {
            let path = Path::new(&e.path);
            let now = hash_file(path).unwrap();
            assert_eq!(e.expected_hash, now, "expected_hash of {}", e.path);
            match &e.op {
                EditOp::CreateFile { content } => {
                    assert!(now.is_none(), "{} exists", e.path);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, content).unwrap();
                }
                EditOp::ReplaceText { content } => {
                    assert!(now.is_some(), "{} is missing", e.path);
                    fs::write(path, content).unwrap();
                }
                EditOp::DeleteFile => {
                    assert!(now.is_some(), "{} is missing", e.path);
                    fs::remove_file(path).unwrap();
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    fn rel(root: &Path, e: &FileEdit) -> String {
        Path::new(&e.path)
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn rels(root: &Path, edits: &[FileEdit]) -> Vec<String> {
        let mut v: Vec<String> = edits.iter().map(|e| rel(root, e)).collect();
        v.sort();
        v
    }

    fn edit_of<'a>(root: &Path, edits: &'a [FileEdit], path: &str) -> &'a FileEdit {
        edits.iter().find(|e| rel(root, e) == path).unwrap()
    }

    fn content(e: &FileEdit) -> &str {
        match &e.op {
            EditOp::CreateFile { content } | EditOp::ReplaceText { content } => content,
            other => panic!("no content in {other:?}"),
        }
    }

    fn states(st: &LevelState, doc: usize) -> Vec<CopyState> {
        st.rows[doc].1.iter().map(|r| r.state).collect()
    }

    fn read(root: &Path, path: &str) -> String {
        fs::read_to_string(root.join(path)).unwrap()
    }

    fn write(root: &Path, path: &str, text: &str) {
        let p = root.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    /// A project with `security` applied (all five CLIs found).
    fn applied(root: &Path) -> Place {
        let p = place(root, vec![security(root)], &ALL);
        apply(&review(&p));
        p
    }

    #[test]
    fn new_project_doc_wants_four_files() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = place(root, vec![security(root)], &ALL);
        let edits = review(&p);
        assert_eq!(rels(root, &edits), [AGY, CLAUDE, CODEX, QWEN]);
        assert!(edits.iter().all(|e| matches!(e.op, EditOp::CreateFile { .. })));
        assert!(edits.iter().all(|e| e.expected_hash.is_none()));
        assert!(edits.iter().all(|e| e.scope == ConfigScope::Project));
        assert!(edits.iter().all(|e| e.applies == Applies::NewSessions));
        let st = state(&p);
        assert_eq!(st.out_of_date, 4);
        assert_eq!(st.changed_outside, 0);
        let copilot = &st.rows[0].1[4];
        assert!(copilot.shared);
        assert_eq!(copilot.cli.as_deref(), Some("copilot"));
        assert_eq!(copilot.path.as_deref(), Some(CLAUDE));
        assert_eq!(copilot.state, CopyState::Missing);
        assert_eq!(
            copilot.notes,
            ["Copilot CLI reads .claude/rules, so it loads this too."]
        );
        assert_eq!(states(&st, 0)[..4], [CopyState::Missing; 4]);
    }

    #[test]
    fn copilot_alone_gets_github_instructions() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let d = project_doc(root, "security", "---\nclis: [copilot]\n---\nBe careful.\n");
        let p = place(root, vec![d], &ALL);
        assert_eq!(rels(root, &review(&p)), [GITHUB]);
        let row = &state(&p).rows[0].1[4];
        assert!(!row.shared);
        assert_eq!(row.state, CopyState::Missing);
        assert_eq!(row.path.as_deref(), Some(GITHUB));
    }

    #[test]
    fn hand_written_doc_with_claude_shares_with_copilot() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let d = project_doc(root, "security", "---\nclis: [claude]\n---\nBe careful.\n");
        let p = place(root, vec![d], &ALL);
        assert_eq!(rels(root, &review(&p)), [CLAUDE]);
        let st = state(&p);
        assert!(st.rows[0].1[4].shared);
        assert_eq!(st.rows[0].1[1].state, CopyState::NoCopy);
        assert_eq!(st.rows[0].1[1].notes, ["Not ticked for this doc."]);
    }

    #[test]
    fn github_copy_goes_once_claude_is_ticked() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let github_only = project_doc(root, "security", "---\nclis: [copilot]\n---\nBe careful.\n");
        apply(&review(&place(root, vec![github_only], &ALL)));
        assert!(root.join(GITHUB).exists());
        let both = project_doc(root, "security", "---\nclis: [claude, copilot]\n---\nBe careful.\n");
        let p = place(root, vec![both], &ALL);
        let edits = review(&p);
        assert_eq!(rels(root, &edits), [CLAUDE, GITHUB]);
        assert!(matches!(edit_of(root, &edits, GITHUB).op, EditOp::DeleteFile));
        assert!(matches!(edit_of(root, &edits, CLAUDE).op, EditOp::CreateFile { .. }));
        let st = state(&p);
        assert_eq!(st.out_of_date, 2);
        // The `.github` copy has a row of its own after the five, so Review removes nothing the page doesn't list.
        assert_eq!(st.rows[0].1.len(), 6);
        assert!(st.rows[0].1[4].shared);
        assert_eq!(st.rows[0].1[5].state, CopyState::Extra);
        assert_eq!(st.rows[0].1[5].path.as_deref(), Some(GITHUB));
        let copies: Vec<(&str, CopyState)> = st.copies.iter().map(|c| (c.cli, c.state)).collect();
        assert_eq!(copies, [("claude", CopyState::Missing), ("copilot", CopyState::Extra)]);
    }

    #[test]
    fn applied_review_is_in_sync() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = applied(root);
        let st = state(&p);
        assert_eq!(states(&st, 0), [CopyState::InSync; 5]);
        assert_eq!(st.out_of_date, 0);
        assert_eq!(st.changed_outside, 0);
        assert!(review(&p).is_empty());
    }

    #[test]
    fn crlf_checkout_is_in_sync() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = applied(root);
        for path in [CLAUDE, AGY, QWEN, CODEX] {
            let text = read(root, path).replace('\n', "\r\n");
            write(root, path, &text);
        }
        let st = state(&p);
        assert_eq!(states(&st, 0), [CopyState::InSync; 5]);
        assert!(review(&p).is_empty());
    }

    #[test]
    fn doc_change_makes_copies_out_of_date() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        applied(root);
        let p = place(root, vec![project_doc(root, "security", "New text.\n")], &ALL);
        let st = state(&p);
        assert_eq!(states(&st, 0), [CopyState::OutOfDate; 5]);
        assert_eq!(st.out_of_date, 4);
        let edits = review(&p);
        assert_eq!(rels(root, &edits), [AGY, CLAUDE, CODEX, QWEN]);
        assert!(edits.iter().all(|e| matches!(e.op, EditOp::ReplaceText { .. })));
        apply(&edits);
        assert_eq!(states(&state(&p), 0), [CopyState::InSync; 5]);
    }

    #[test]
    fn off_doc_removes_its_copies() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        applied(root);
        let off = project_doc(root, "security", "---\nenabled: false\n---\nBe careful.\n");
        let p = place(root, vec![off], &ALL);
        let edits = review(&p);
        assert_eq!(rels(root, &edits), [AGY, CLAUDE, CODEX, QWEN]);
        assert!(edits.iter().all(|e| matches!(e.op, EditOp::DeleteFile)));
        let st = state(&p);
        assert_eq!(
            states(&st, 0),
            [
                CopyState::Extra,
                CopyState::Extra,
                CopyState::Extra,
                CopyState::Extra,
                CopyState::NoCopy
            ]
        );
        assert_eq!(st.out_of_date, 4);
        apply(&edits);
        assert!(!root.join(CODEX).exists());
        assert!(review(&p).is_empty());
    }

    #[test]
    fn off_doc_with_no_copies_plans_nothing() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let off = project_doc(root, "security", "---\nenabled: false\n---\nBe careful.\n");
        let p = place(root, vec![off], &ALL);
        assert!(review(&p).is_empty());
        assert_eq!(state(&p).out_of_date, 0);
    }

    #[test]
    fn orphan_copy_is_removed() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        write(
            root,
            ".qwen/rules/marshell-gone.md",
            &file_copy("", "gone", "s", "Old\n"),
        );
        let p = place(root, vec![], &ALL);
        let edits = review(&p);
        assert_eq!(rels(root, &edits), [".qwen/rules/marshell-gone.md"]);
        assert!(matches!(edits[0].op, EditOp::DeleteFile));
        let st = state(&p);
        assert_eq!(st.orphans.len(), 1);
        assert_eq!(st.orphans[0].state, CopyState::Extra);
        assert_eq!(st.orphans[0].cli.as_deref(), Some("qwen"));
        assert_eq!(st.orphans[0].path.as_deref(), Some(".qwen/rules/marshell-gone.md"));
        assert_eq!(
            st.orphans[0].notes,
            ["From gone, a doc that is gone. Review removes it."]
        );
        assert_eq!(st.out_of_date, 1);
    }

    #[test]
    fn edited_orphan_stays() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let copy = file_copy("", "gone", "s", "Old\n").replace("Old", "Mine");
        write(root, ".qwen/rules/marshell-gone.md", &copy);
        let p = place(root, vec![], &ALL);
        assert!(review(&p).is_empty());
        let st = state(&p);
        assert_eq!(st.orphans.len(), 1);
        assert_eq!(st.orphans[0].state, CopyState::ChangedOutside);
        assert_eq!(
            st.orphans[0].notes,
            [
                "From gone, a doc that is gone. It was edited, so Marshell leaves it; delete it yourself if you don't need it."
            ]
        );
        assert_eq!(st.out_of_date, 0);
        assert_eq!(st.changed_outside, 0);
    }

    #[test]
    fn backup_with_a_copied_marker_is_ignored() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        write(
            root,
            ".qwen/rules/marshell-security-old.md",
            &file_copy("", "security", "s", "Old\n"),
        );
        let p = place(root, vec![], &ALL);
        assert!(review(&p).is_empty());
        let st = state(&p);
        assert!(st.orphans.is_empty());
        assert_eq!(st.out_of_date, 0);
    }

    #[test]
    fn broken_doc_keeps_its_copies() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        applied(root);
        let broken = project_doc(root, "security", "---\nclis: [claude]\nBe careful.\n");
        assert!(broken.error.is_some());
        let p = place(root, vec![broken], &ALL);
        assert!(review(&p).is_empty());
        let st = state(&p);
        assert!(st.orphans.is_empty());
        assert_eq!(st.out_of_date, 0);
        for row in &st.rows[0].1 {
            assert_eq!(
                row.notes,
                ["The doc's frontmatter can't be read, so its copies are left as they are."]
            );
        }
        let delete = plan_edits(&p, &Pick::Delete("security".into()), user(), &yes);
        assert_eq!(delete.len(), 4, "deleting a broken doc takes its untouched copies");
    }

    #[test]
    fn taken_name_is_left_alone() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        write(root, CLAUDE, "Mine\n");
        let p = place(root, vec![security(root)], &ALL);
        let st = state(&p);
        let note = ".claude/rules/marshell-security.md isn't a copy Marshell wrote, so Marshell leaves it.";
        assert_eq!(st.rows[0].1[0].state, CopyState::Taken);
        assert_eq!(st.rows[0].1[0].notes, [note]);
        assert_eq!(st.rows[0].1[4].state, CopyState::Taken);
        let edits = review(&p);
        assert_eq!(rels(root, &edits), [AGY, CODEX, QWEN]);
        let pick = Pick::Fixes {
            update: true,
            put_back: vec![("security".into(), "claude")],
        };
        let all = plan_edits(&p, &pick, user(), &yes);
        assert_eq!(rels(root, &all), [AGY, CODEX, QWEN]);
    }

    #[test]
    fn a_duplicate_doc_wants_nothing_and_freezes_nothing() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let first = codex_doc(root, "a", "A body");
        apply(&review(&place(root, vec![first.clone()], &ALL)));
        let mut second = codex_doc(root, "A", "Other body");
        second.duplicate = true;
        second.error = Some("Another doc here is named a.".into());
        let p = place(root, vec![first, second], &ALL);
        let st = state(&p);
        assert_eq!(states(&st, 0)[1], CopyState::InSync);
        assert_eq!(states(&st, 1), [CopyState::NoCopy; 5]);
        assert_eq!(st.rows[1].1[0].notes, ["Another doc here is named a."]);
        assert!(review(&p).is_empty());
        // The first doc's section still updates.
        let changed = place(root, vec![codex_doc(root, "a", "New body"), p.docs[1].clone()], &ALL);
        let edits = review(&changed);
        assert_eq!(edits.len(), 1);
        assert!(content(&edits[0]).contains("New body"));
    }

    #[test]
    fn user_agents_md_with_crlf_is_in_sync_and_a_blank_one_is_never_deleted() {
        let t = tempfile::tempdir().unwrap();
        let (p, home) = user_place(&t.path().join("one"), "Be careful.\n", &["codex"]);
        apply(&plan_edits(&p, &Pick::Review, &home, &yes));
        let crlf = read(&home, ".codex/AGENTS.md").replace('\n', "\r\n");
        write(&home, ".codex/AGENTS.md", &crlf);
        assert!(plan_edits(&p, &Pick::Review, &home, &yes).is_empty());
        assert_eq!(level_state(&p, &home, &yes).rows[0].1[1].state, CopyState::InSync);
        // A whitespace-only AGENTS.md that holds no block is the person's: nothing in a pick touches it.
        let (q, home) = user_place(&t.path().join("two"), "Be careful.\n", &["claude"]);
        write(&home, ".codex/AGENTS.md", "\n\n");
        assert_eq!(plan_edits(&q, &Pick::Review, &home, &yes).len(), 1);
        assert_eq!(read(&home, ".codex/AGENTS.md"), "\n\n");
        let gone = Place { docs: vec![], ..q };
        assert!(plan_edits(&gone, &Pick::Review, &home, &yes).is_empty());
    }

    #[test]
    fn a_delete_of_a_path_editable_refuses_is_not_planned() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        applied(root);
        let off = project_doc(
            root,
            "security",
            "---
enabled: false
---
Be careful.
",
        );
        let p = place(root, vec![off], &ALL);
        let editable = |path: &Path| {
            !path
                .components()
                .any(|c| c.as_os_str() == ".qwen" || c.as_os_str() == ".codex")
        };
        let edits = plan_edits(&p, &Pick::Review, user(), &editable);
        assert_eq!(rels(root, &edits), [AGY, CLAUDE]);
        assert_eq!(level_state(&p, user(), &editable).out_of_date, 2);
    }

    #[test]
    fn review_plan_joins_every_place() {
        let t = tempfile::tempdir().unwrap();
        let (a, home_a) = user_place(
            &t.path().join("a"),
            "Be careful.
",
            &["claude"],
        );
        let (b, home_b) = user_place(
            &t.path().join("b"),
            "Be careful.
",
            &["qwen"],
        );
        let cs = review_plan(&[a, b], &home_a, &yes);
        assert_eq!(cs.title, "Update the copies");
        assert_eq!(cs.reason, "Write each library doc's copies as the doc is now.");
        assert_eq!(cs.edits.len(), 2);
        assert!(cs.edits[0].path.starts_with(&*home_a.to_string_lossy()));
        assert!(cs.edits[1].path.starts_with(&*home_b.to_string_lossy()));
    }

    #[test]
    fn changed_outside_is_not_in_review() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = applied(root);
        let edited = read(root, QWEN).replace("Be careful.", "Mine.");
        write(root, QWEN, &edited);
        let st = state(&p);
        assert_eq!(st.rows[0].1[3].state, CopyState::ChangedOutside);
        assert_eq!(st.changed_outside, 1);
        assert_eq!(st.out_of_date, 0);
        assert!(review(&p).is_empty());
    }

    #[test]
    fn only_a_wanted_changed_copy_offers_fixes() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = applied(root);
        let edited = read(root, CLAUDE).replace("Be careful.", "Mine.");
        write(root, CLAUDE, &edited);
        let st = state(&p);
        let rows = &st.rows[0].1;
        assert_eq!(rows[0].state, CopyState::ChangedOutside);
        assert!(rows[0].fixes, "Claude Code's row has the fixes");
        assert_eq!(rows[4].state, CopyState::ChangedOutside);
        assert!(rows[4].shared);
        assert!(
            !rows[4].fixes,
            "the shared Copilot CLI row leaves the fixes to Claude Code's"
        );
        assert!(rows[1..4].iter().all(|r| !r.fixes));
        assert_eq!(st.changed_outside, 1);
    }

    #[test]
    fn edited_copy_of_an_unticked_cli_stays_without_fixes() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        applied(root);
        let edited = read(root, QWEN).replace("Be careful.", "Mine.");
        write(root, QWEN, &edited);
        let doc = project_doc(
            root,
            "security",
            "---\nclis: [claude, codex, antigravity, copilot]\n---\nBe careful.\n",
        );
        let p = place(root, vec![doc], &ALL);
        let st = state(&p);
        let qwen = &st.rows[0].1[3];
        assert_eq!(qwen.state, CopyState::ChangedOutside);
        assert!(!qwen.fixes);
        assert_eq!(
            qwen.notes,
            [
                "Not ticked for this doc.",
                "It was edited, so Marshell leaves it; delete it yourself if you don't need it."
            ]
        );
        assert_eq!(st.changed_outside, 0);
        assert!(st.copies.iter().all(|c| c.cli != "qwen"));
        assert!(!rels(root, &review(&p)).contains(&QWEN.to_string()));
    }

    /// A folder link at `link` to `target`: a symlink, or a junction on Windows without developer mode.
    fn link_dir(link: &Path, target: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, link).or_else(|_| {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()?
                .status;
            status
                .success()
                .then_some(())
                .ok_or_else(|| std::io::Error::other("mklink /J failed"))
        });
    }

    #[test]
    fn a_linked_folder_out_of_the_project_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("repo");
        fs::create_dir_all(&root).unwrap();
        let outside = t.path().join("outside");
        fs::create_dir_all(outside.join("rules")).unwrap();
        // A copy Marshell wrote elsewhere, which Review would otherwise take for an orphan here.
        let theirs = file_copy("", "gone", "s", "Theirs.\n");
        fs::write(outside.join("rules").join("marshell-gone.md"), &theirs).unwrap();
        if let Err(e) = link_dir(&root.join(".qwen"), &outside) {
            eprintln!("skipped: cannot create a folder link here: {e}");
            return;
        }
        let p = place(&root, vec![security(&root)], &ALL);
        let st = state(&p);
        assert_eq!(st.rows[0].1[3].state, CopyState::Refused);
        assert!(st.orphans.is_empty());
        let edits = review(&p);
        assert!(edits.iter().all(|e| !e.path.contains(".qwen")), "{edits:?}");
        assert_eq!(
            fs::read_to_string(outside.join("rules").join("marshell-gone.md")).unwrap(),
            theirs
        );
    }

    #[test]
    fn a_dangling_folder_link_is_not_inside() {
        // Where a link with no target leads can't be known, so a write through it doesn't count as inside the project.
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("repo");
        fs::create_dir_all(&root).unwrap();
        let root = crate::projects::canonical(&root);
        let outside = t.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        if let Err(e) = link_dir(&root.join(".qwen"), &outside) {
            eprintln!("skipped: cannot create a folder link here: {e}");
            return;
        }
        fs::remove_dir(&outside).unwrap();
        assert!(!stays_inside(
            &root.join(".qwen").join("rules").join("marshell-a.md"),
            &root
        ));
        assert!(stays_inside(
            &root.join(".claude").join("rules").join("marshell-a.md"),
            &root
        ));
    }

    #[test]
    fn refused_path_is_left_out() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = place(root, vec![security(root)], &ALL);
        let editable = |path: &Path| !path.components().any(|c| c.as_os_str() == ".qwen");
        let st = level_state(&p, user(), &editable);
        assert_eq!(st.rows[0].1[3].state, CopyState::Refused);
        assert_eq!(
            st.rows[0].1[3].notes,
            ["Marshell may not write here, so this copy is left out."]
        );
        assert_eq!(st.out_of_date, 3);
        let edits = plan_edits(&p, &Pick::Review, user(), &editable);
        assert_eq!(rels(root, &edits), [AGY, CLAUDE, CODEX]);
    }

    #[test]
    fn a_copy_over_4_mib_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        write(root, QWEN, &"x".repeat(4 * 1024 * 1024 + 1));
        let p = place(root, vec![security(root)], &ALL);
        let st = state(&p);
        assert_eq!(st.rows[0].1[3].state, CopyState::Refused);
        assert_eq!(rels(root, &review(&p)), [AGY, CLAUDE, CODEX]);
        // A too big file no doc wants is ignored: no row, no edit.
        write(root, ".agents/rules/marshell-gone.md", &"x".repeat(4 * 1024 * 1024 + 1));
        let q = place(root, vec![], &ALL);
        assert!(review(&q).is_empty());
        assert!(state(&q).orphans.is_empty());
    }

    #[test]
    fn codex_skips_a_doc_with_globs() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let d = project_doc(root, "security", "---\nglobs: ['src/**']\n---\nBe careful.\n");
        let p = place(root, vec![d], &ALL);
        let st = state(&p);
        assert_eq!(st.rows[0].1[1].state, CopyState::NoCopy);
        assert_eq!(
            st.rows[0].1[1].notes,
            ["Codex has no globs, so this doc doesn't load in Codex."]
        );
        assert_eq!(rels(root, &review(&p)), [AGY, CLAUDE, QWEN]);
        assert_eq!(
            st.rows[0].1[2].notes,
            [
                "Antigravity CLI matches globs anywhere in the path, so src/** also matches a deeper folder of that name."
            ]
        );
    }

    #[test]
    fn user_doc_skips_a_cli_not_found() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let d = doc::parse(
            "security",
            DocScope::User,
            t.path().join("mhome").join("instructions").join("security.md"),
            b"Be careful.\n",
        );
        let p = Place {
            level: Level::User {
                roots: Roots::under(&home),
                distro: Some("MarshellTestDistro".into()),
            },
            docs: vec![d],
            found: vec!["claude"],
        };
        let st = level_state(&p, &home, &yes);
        let agy = &st.rows[0].1[2];
        assert_eq!(agy.state, CopyState::NoCopy);
        // The CLI may be installed on Windows: the note names the distro, where it isn't.
        assert_eq!(agy.notes, ["Not installed in MarshellTestDistro, so it gets no copy."]);
        assert_eq!(agy.distro.as_deref(), Some("MarshellTestDistro"));
        assert_eq!(st.rows[0].1[0].state, CopyState::Missing);
        assert_eq!(
            st.rows[0].1[0].path.as_deref(),
            Some("~/.claude/rules/marshell-security.md")
        );
        assert_eq!(st.out_of_date, 1);
    }

    #[test]
    fn project_doc_notes_teammates() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = place(root, vec![security(root)], &["claude"]);
        let st = state(&p);
        assert_eq!(st.rows[0].1[1].state, CopyState::Missing);
        assert_eq!(
            st.rows[0].1[1].notes,
            ["Not installed on this PC. The copy is for teammates who use it."]
        );
        assert!(st.rows[0].1[0].notes.is_empty());
        assert_eq!(rels(root, &review(&p)), [AGY, CLAUDE, CODEX, QWEN]);
    }

    #[test]
    fn a_distro_projects_teammates_note_names_the_distro() {
        let root = Path::new(r"\\wsl.localhost\MarshellTestDistro\tmp\p");
        let p = place(root, vec![security(root)], &["claude"]);
        let st = state(&p);
        // The share doesn't exist here, so the row also says Marshell may not write it; only the teammates note counts.
        let notes = &st.rows[0].1[1].notes;
        assert!(
            notes.contains(&"Not installed in MarshellTestDistro. The copy is for teammates who use it.".to_string())
        );
        assert!(!notes.iter().any(|n| n.contains("on this PC")));
    }

    #[test]
    fn agy_size_warning() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let body = format!("{}\n", "x".repeat(24_001));
        let d = project_doc(root, "security", &body);
        let p = place(root, vec![d], &ALL);
        let edits = review(&p);
        let n = content(edit_of(root, &edits, AGY)).len();
        assert!(n > 24_001);
        let text = n.to_string();
        let with_comma = format!("{},{}", &text[..text.len() - 3], &text[text.len() - 3..]);
        let st = state(&p);
        assert_eq!(
            st.rows[0].1[2].notes,
            [format!(
                "Antigravity CLI cuts a rule past 24,000 bytes, and this copy is {with_comma} bytes."
            )]
        );
        assert!(st.rows[0].1[3].notes.is_empty());
    }

    #[test]
    fn project_notes_reach_the_codex_and_qwen_rows() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let mut p = place(root, vec![security(root)], &ALL);
        p.level = Level::Project {
            root: root.to_path_buf(),
            notes: Notes {
                codex: Some("Codex note.".into()),
                codex_user_warning: true,
                qwen: Some("Qwen note.".into()),
            },
        };
        let st = state(&p);
        assert_eq!(
            st.rows[0].1[1].notes,
            [
                "Codex note.",
                "Your developer_instructions in ~/.codex/config.toml stop loading in this project, because the project sets its own."
            ]
        );
        assert_eq!(st.rows[0].1[3].notes, ["Qwen note."]);
    }

    fn user_place(t: &Path, doc_text: &str, found: &[&'static str]) -> (Place, PathBuf) {
        let home = t.join("home");
        let d = doc::parse(
            "security",
            DocScope::User,
            t.join("mhome").join("instructions").join("security.md"),
            doc_text.as_bytes(),
        );
        let p = Place {
            level: Level::User {
                roots: Roots::under(&home),
                distro: None,
            },
            docs: vec![d],
            found: found.to_vec(),
        };
        (p, home)
    }

    #[test]
    fn user_level_copies() {
        let t = tempfile::tempdir().unwrap();
        let (p, home) = user_place(t.path(), "Be careful.\n", &["claude", "codex", "copilot"]);
        let edits = plan_edits(&p, &Pick::Review, &home, &yes);
        assert_eq!(
            rels(&home, &edits),
            [
                ".claude/rules/marshell-security.md",
                ".codex/AGENTS.md",
                ".copilot/instructions/marshell-security.instructions.md"
            ]
        );
        assert!(edits.iter().all(|e| e.scope == ConfigScope::User));
        let agents = content(edit_of(&home, &edits, ".codex/AGENTS.md"));
        assert!(agents.starts_with("<!-- marshell:begin doc security "));
        assert!(agents.contains("Be careful.\n<!-- marshell:end doc security -->"));
        assert!(
            !rels(&home, &edits)
                .iter()
                .any(|r| r.contains(".gemini") || r.contains(".qwen"))
        );
        apply(&edits);
        let st = level_state(&p, &home, &yes);
        assert_eq!(
            states(&st, 0),
            [
                CopyState::InSync,
                CopyState::InSync,
                CopyState::NoCopy,
                CopyState::NoCopy,
                CopyState::InSync
            ]
        );
        assert!(!st.rows[0].1[4].shared);
    }

    #[test]
    fn user_copy_of_a_cli_no_longer_found_is_extra() {
        let t = tempfile::tempdir().unwrap();
        let (p, home) = user_place(t.path(), "Be careful.\n", &ALL);
        apply(&plan_edits(&p, &Pick::Review, &home, &yes));
        assert!(home.join(".qwen/rules/marshell-security.md").exists());
        let q = Place {
            found: vec!["claude", "codex", "antigravity", "copilot"],
            ..p
        };
        let edits = plan_edits(&q, &Pick::Review, &home, &yes);
        assert_eq!(rels(&home, &edits), [".qwen/rules/marshell-security.md"]);
        assert!(matches!(edits[0].op, EditOp::DeleteFile));
        let st = level_state(&q, &home, &yes);
        assert_eq!(st.rows[0].1[3].state, CopyState::Extra);
        assert_eq!(st.out_of_date, 1);
    }

    #[test]
    fn codex_override_takes_the_blocks() {
        let t = tempfile::tempdir().unwrap();
        let (p, home) = user_place(t.path(), "Be careful.\n", &["codex"]);
        apply(&plan_edits(&p, &Pick::Review, &home, &yes));
        assert!(home.join(".codex/AGENTS.md").exists());
        write(&home, ".codex/AGENTS.override.md", "My own text.\n");
        let edits = plan_edits(&p, &Pick::Review, &home, &yes);
        assert_eq!(rels(&home, &edits), [".codex/AGENTS.md", ".codex/AGENTS.override.md"]);
        assert!(matches!(
            edit_of(&home, &edits, ".codex/AGENTS.md").op,
            EditOp::DeleteFile
        ));
        let over = content(edit_of(&home, &edits, ".codex/AGENTS.override.md"));
        assert!(over.starts_with("My own text.\n"));
        assert!(over.contains("marshell:begin doc security"));
        apply(&edits);
        let st = level_state(&p, &home, &yes);
        assert_eq!(st.rows[0].1[1].state, CopyState::InSync);
        assert!(plan_edits(&p, &Pick::Review, &home, &yes).is_empty());
    }

    fn codex_doc(root: &Path, name: &str, body: &str) -> Doc {
        project_doc(root, name, &format!("---\nclis: [codex]\n---\n{body}\n"))
    }

    #[test]
    fn two_codex_docs_share_a_file() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let docs = vec![codex_doc(root, "a", "A body"), codex_doc(root, "b", "B body")];
        let p = place(root, docs, &ALL);
        let edits = review(&p);
        assert_eq!(edits.len(), 1);
        let text = content(&edits[0]);
        assert!(text.contains("marshell:begin doc a ") && text.contains("marshell:begin doc b "));
        apply(&edits);
        let by_hand = read(root, CODEX)
            .replace("A body", "A hand")
            .replace("B body", "B hand");
        write(root, CODEX, &by_hand);
        let st = state(&p);
        assert_eq!(states(&st, 0)[1], CopyState::ChangedOutside);
        assert_eq!(states(&st, 1)[1], CopyState::ChangedOutside);
        let one = plan_edits(&p, &Pick::PutBack("a".into(), "codex"), user(), &yes);
        assert_eq!(one.len(), 1);
        let text = content(&one[0]);
        assert!(text.contains("A body") && !text.contains("A hand"));
        assert!(text.contains("B hand"), "b's section stays as it is");
        let b_before = by_hand.find("marshell:begin doc b ").unwrap();
        let b_after = text.find("marshell:begin doc b ").unwrap();
        assert_eq!(by_hand[b_before..], text[b_after..], "b's section is byte for byte");
    }

    #[test]
    fn fixes_touch_one_file_once() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let old = place(
            root,
            vec![codex_doc(root, "a", "A body"), codex_doc(root, "b", "B body")],
            &ALL,
        );
        apply(&review(&old));
        let by_hand = read(root, CODEX).replace("A body", "A hand");
        write(root, CODEX, &by_hand);
        let p = place(
            root,
            vec![codex_doc(root, "a", "A body"), codex_doc(root, "b", "B new")],
            &ALL,
        );
        let pick = Pick::Fixes {
            update: true,
            put_back: vec![("a".into(), "codex")],
        };
        let edits = plan_edits(&p, &pick, user(), &yes);
        assert_eq!(edits.len(), 1);
        let text = content(&edits[0]);
        assert!(text.contains("A body") && text.contains("B new"));
        assert!(!text.contains("A hand") && !text.contains("B body"));
        let none = Pick::Fixes {
            update: false,
            put_back: vec![],
        };
        assert!(plan_edits(&p, &none, user(), &yes).is_empty());
        let only_back = Pick::Fixes {
            update: false,
            put_back: vec![("a".into(), "codex")],
        };
        let one = plan_edits(&p, &only_back, user(), &yes);
        assert_eq!(one.len(), 1);
        assert!(content(&one[0]).contains("B body"), "b is not updated without update");
    }

    /// `security` applied, its doc file on disk, and the docs read back.
    fn on_disk(root: &Path) -> Place {
        write(root, DOC, "Be careful.\n");
        let docs = doc::read_dir(&root.join(".marshell/instructions"), DocScope::Project);
        let p = place(root, docs, &ALL);
        apply(&review(&p));
        p
    }

    #[test]
    fn delete_plan_holds_doc_and_copies() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = on_disk(root);
        let cs = delete_plan(std::slice::from_ref(&p), "Security", user(), &yes);
        assert_eq!(cs.title, "Delete security and its copies");
        assert_eq!(rels(root, &cs.edits), [AGY, CLAUDE, CODEX, DOC, QWEN]);
        assert!(cs.edits.iter().all(|e| matches!(e.op, EditOp::DeleteFile)));
        assert_eq!(rel(root, &cs.edits[0]), DOC);
        assert_eq!(cs.edits[0].scope, ConfigScope::Project);
        assert_eq!(cs.edits[0].expected_hash.as_deref(), Some(p.docs[0].hash.as_str()));
        apply(&cs.edits);
    }

    #[test]
    fn delete_plan_leaves_an_edited_copy() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = on_disk(root);
        let edited = read(root, QWEN).replace("Be careful.", "Mine.");
        write(root, QWEN, &edited);
        let cs = delete_plan(&[p], "security", user(), &yes);
        assert_eq!(rels(root, &cs.edits), [AGY, CLAUDE, CODEX, DOC]);
    }

    #[test]
    fn put_back_plan_rewrites_one_copy() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = on_disk(root);
        let original = read(root, QWEN);
        write(root, QWEN, &original.replace("Be careful.", "Mine."));
        let cs = put_back_plan(&p, "security", "qwen", user(), &yes).unwrap();
        assert_eq!(cs.title, "Put the doc's text back in .qwen/rules/marshell-security.md");
        assert_eq!(cs.edits.len(), 1);
        assert_eq!(content(&cs.edits[0]), original);
        assert!(matches!(cs.edits[0].op, EditOp::ReplaceText { .. }));
        apply(&cs.edits);
        assert_eq!(states(&state(&p), 0), [CopyState::InSync; 5]);
    }

    #[test]
    fn put_back_of_an_in_sync_copy_is_none() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = on_disk(root);
        assert!(put_back_plan(&p, "security", "qwen", user(), &yes).is_none());
        assert_eq!(
            take_copy_plan(&p, "security", "qwen", user(), &yes).err(),
            Some(CantTake::Gone)
        );
        assert!(put_back_plan(&p, "nope", "qwen", user(), &yes).is_none());
    }

    #[test]
    fn take_copy_plan_takes_the_copys_text() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = on_disk(root);
        write(root, QWEN, &read(root, QWEN).replace("Be careful.", "Mine."));
        let cs = take_copy_plan(&p, "security", "qwen", user(), &yes).unwrap();
        assert_eq!(cs.title, "Use the text of .qwen/rules/marshell-security.md in security");
        assert_eq!(
            cs.reason,
            "Make the copy's text the doc's. Its other copies stay out of date until Review."
        );
        assert_eq!(cs.edits.len(), 2);
        assert_eq!(rel(root, &cs.edits[0]), DOC);
        assert_eq!(cs.edits[0].expected_hash.as_deref(), Some(p.docs[0].hash.as_str()));
        assert!(content(&cs.edits[0]).ends_with("---\nMine.\n"));
        assert_eq!(rel(root, &cs.edits[1]), QWEN);
        apply(&cs.edits);
        let docs = doc::read_dir(&root.join(".marshell/instructions"), DocScope::Project);
        assert_eq!(docs[0].body, "Mine.\n");
        let st = state(&place(root, docs, &ALL));
        assert_eq!(
            states(&st, 0),
            [
                CopyState::OutOfDate,
                CopyState::OutOfDate,
                CopyState::OutOfDate,
                CopyState::InSync,
                CopyState::OutOfDate
            ]
        );
    }

    #[test]
    fn take_copy_plan_refuses_a_copy_holding_a_marker_line() {
        // Save refuses such a body; taken into the doc, the line would end the doc's Codex block early.
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let p = on_disk(root);
        let edited = read(root, QWEN).replace("Be careful.", "Mine.\n<!-- marshell:end doc security -->");
        write(root, QWEN, &edited);
        assert_eq!(
            take_copy_plan(&p, "security", "qwen", user(), &yes).err(),
            Some(CantTake::Marker)
        );
    }

    #[test]
    fn take_copy_plan_for_a_codex_section() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        write(root, ".marshell/instructions/a.md", "---\nclis: [codex]\n---\nA body\n");
        let docs = doc::read_dir(&root.join(".marshell/instructions"), DocScope::Project);
        let p = place(root, docs, &ALL);
        apply(&review(&p));
        write(root, CODEX, &read(root, CODEX).replace("A body", "A hand"));
        let cs = take_copy_plan(&p, "a", "codex", user(), &yes).unwrap();
        apply(&cs.edits);
        let docs = doc::read_dir(&root.join(".marshell/instructions"), DocScope::Project);
        assert_eq!(docs[0].body, "A hand\n");
        assert_eq!(states(&state(&place(root, docs, &ALL)), 0)[1], CopyState::InSync);
    }

    #[test]
    fn save_plan_renames() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let old = project_doc(root, "security", "Be careful.\n");
        let new = project_doc(root, "secrets", "Be careful.\n");
        let cs = save_plan(&new, Some(&old), Some("abc"));
        assert_eq!(cs.title, "Save secrets");
        assert_eq!(cs.edits.len(), 2);
        assert_eq!(rel(root, &cs.edits[0]), ".marshell/instructions/secrets.md");
        assert!(matches!(cs.edits[0].op, EditOp::CreateFile { .. }));
        assert_eq!(cs.edits[0].expected_hash, None);
        assert_eq!(content(&cs.edits[0]), doc::render(&new));
        assert_eq!(rel(root, &cs.edits[1]), DOC);
        assert!(matches!(cs.edits[1].op, EditOp::DeleteFile));
        assert_eq!(cs.edits[1].expected_hash.as_deref(), Some("abc"));
        assert!(cs.edits.iter().all(|e| e.scope == ConfigScope::Project));
    }

    #[test]
    fn save_plan_creates_and_replaces() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let d = project_doc(root, "security", "Be careful.\n");
        let created = save_plan(&d, None, None);
        assert_eq!(created.edits.len(), 1);
        assert!(matches!(created.edits[0].op, EditOp::CreateFile { .. }));
        assert_eq!(created.edits[0].expected_hash, None);
        let replaced = save_plan(&d, Some(&d), Some("abc"));
        assert_eq!(replaced.edits.len(), 1);
        assert!(matches!(replaced.edits[0].op, EditOp::ReplaceText { .. }));
        assert_eq!(replaced.edits[0].expected_hash.as_deref(), Some("abc"));
        let u = doc::parse("security", DocScope::User, t.path().join("security.md"), b"x\n");
        assert_eq!(save_plan(&u, None, None).edits[0].scope, ConfigScope::Marshell);
    }

    #[test]
    fn uninstall_plan_removes_ours_only() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let roots = Roots::under(&home);
        let copy = |name: &str| file_copy("", name, "s", "A\n");
        write(&home, ".claude/rules/marshell-a.md", &copy("a"));
        write(&home, ".gemini/config/rules/marshell-a.md", &copy("a"));
        write(&home, ".qwen/rules/marshell-a.md", &copy("a"));
        write(&home, ".copilot/instructions/marshell-a.instructions.md", &copy("a"));
        write(&home, ".qwen/rules/marshell-b.md", &copy("b").replace("A\n", "Mine\n"));
        write(&home, ".qwen/rules/marshell-mine.md", "Mine\n");
        write(&home, ".qwen/rules/style.md", &copy("style"));
        let a = block_text("a", "s", "A\n");
        let c = block_text("c", "s", "C\n").replace("\nC\n", "\nEdited\n");
        write(&home, ".codex/AGENTS.md", &format!("Mine\n\n{a}\n{c}"));
        let un = uninstall_plan(&roots);
        assert_eq!(un.changes.title, "Remove the instruction library's copies");
        let deletes: Vec<String> = un
            .changes
            .edits
            .iter()
            .filter(|e| matches!(e.op, EditOp::DeleteFile))
            .map(|e| rel(&home, e))
            .collect();
        assert_eq!(
            deletes,
            [
                ".claude/rules/marshell-a.md",
                ".gemini/config/rules/marshell-a.md",
                ".qwen/rules/marshell-a.md",
                ".copilot/instructions/marshell-a.instructions.md"
            ]
        );
        assert_eq!(un.changes.edits.len(), 5);
        assert!(un.changes.edits.iter().all(|e| e.scope == ConfigScope::User));
        let agents = edit_of(&home, &un.changes.edits, ".codex/AGENTS.md");
        assert_eq!(content(agents), format!("Mine\n\n{c}"));
        assert_eq!(
            un.left,
            [
                "Left ~/.qwen/rules/marshell-b.md: it was edited after Marshell wrote it.",
                "Left the c block in ~/.codex/AGENTS.md: it was edited after Marshell wrote it."
            ]
        );
        apply(&un.changes.edits);
        assert_eq!(read(&home, ".qwen/rules/marshell-mine.md"), "Mine\n");
        assert!(home.join(".qwen/rules/style.md").exists());
        assert!(home.join(".qwen/rules/marshell-b.md").exists());
    }

    #[test]
    fn uninstall_deletes_a_codex_file_that_held_only_ours() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        write(&home, ".codex/AGENTS.md", &block_text("a", "s", "A\n"));
        let un = uninstall_plan(&Roots::under(&home));
        assert_eq!(un.changes.edits.len(), 1);
        assert!(matches!(un.changes.edits[0].op, EditOp::DeleteFile));
        assert!(un.left.is_empty());
        assert!(
            uninstall_plan(&Roots::under(&t.path().join("none")))
                .changes
                .edits
                .is_empty()
        );
    }
}
