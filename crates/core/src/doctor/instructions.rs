//! What each CLI's instruction files load: a bounded, memoized reader, the import expander, the nested-file walk and the
//! pure `items` that turn each CLI's `CliLoad` into Doctor items. This module knows no CLI's file names or rules; those
//! live in `agents/<cli>/instructions.rs`, which build a `CliLoad` through `Place`, `Reader` and `expand`.
//! Nothing here writes a file, and no item quotes a file's content.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ignore::WalkBuilder;
use marshell_protocol::doctor::{DoctorGroup, DoctorItem, DoctorLoad, DoctorLoadFile, DoctorLoadWhen};

use super::checks::{ORDER, display_name};
use super::ext::{ExtRoots, tokens};
use crate::config::hash::hash_bytes;
use crate::config::preview::MAX_CONFIG_BYTES;
use crate::references::project_id;
use crate::server::{links_refused, local_file, refused};

/// Entries one nested walk looks at before it stops.
const WALK_CAP: usize = 10_000;
/// How long one run's instruction reads may take.
const READ_BUDGET: Duration = Duration::from_secs(10);
/// How many folders deep the nested walk looks.
const WALK_DEPTH: usize = 12;
/// The most files and the most folders deep `Reader::list` goes.
const LIST_FILES: usize = 1_000;
const LIST_DEPTH: usize = 8;
/// Waste: more than this many tokens at start, for one CLI in one place.
const WASTE_TOKENS: usize = 4_000;

/// Where one CLI's files are looked for. The server fills it; a test points every field into a tempdir.
#[derive(Clone, Copy)]
pub struct Place<'a> {
    pub roots: &'a ExtRoots,
    /// None: user level only.
    pub project: Option<&'a Path>,
    /// The walk up stops here, inclusive; None: the filesystem root.
    pub ceiling: Option<&'a Path>,
    /// Claude Code's managed folder; None reads none.
    pub claude_managed: Option<&'a Path>,
    pub qwen_system_settings: Option<&'a Path>,
    pub distro: Option<&'a str>,
}

impl Place<'_> {
    /// `project` and each folder above it up to the ceiling, outermost first. Empty without a project. A ceiling that
    /// isn't above the project stops the walk at the project itself, so it never reads past a folder the caller
    /// meant as the limit.
    pub fn ancestors(&self) -> Vec<PathBuf> {
        let Some(project) = self.project else {
            return Vec::new();
        };
        let mut chain = Vec::new();
        let mut reached = self.ceiling.is_none();
        for dir in project.ancestors() {
            if dir.as_os_str().is_empty() {
                break;
            }
            chain.push(dir.to_path_buf());
            if self.ceiling.is_some_and(|c| path_key(c) == path_key(dir)) {
                reached = true;
                break;
            }
        }
        if !reached {
            chain.truncate(1);
        }
        chain.reverse();
        chain
    }

    /// The nearest folder at or above `project`, not past the ceiling, that holds one of `markers`.
    pub fn marked_root(&self, cx: &mut Reader, markers: &[&str]) -> Option<PathBuf> {
        self.ancestors()
            .into_iter()
            .rev()
            .find(|dir| markers.iter().any(|m| cx.exists(&dir.join(m))))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    Start,
    WhenUsed,
    /// The reason, worded for "not loaded: <reason>".
    NotLoaded(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    File,
    Import { by: PathBuf },
    Rule,
    Memory,
    Managed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub role: Role,
    pub when: When,
    pub chars: usize,
    pub notes: Vec<String>,
    pub depth: u32,
}

/// Text a CLI cuts or skips for good: one Broken, report-only item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    pub id_part: String,
    pub path: Option<PathBuf>,
    pub title: String,
    pub detail: String,
    pub cannot_fix: String,
}

/// What one CLI loads in one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliLoad {
    pub agent: &'static str,
    pub distro: Option<String>,
    pub project: Option<PathBuf>,
    /// Paths under it show as `~/…`.
    pub home: PathBuf,
    pub files: Vec<LoadedFile>,
    /// Why no project file loads, for the title's brackets.
    pub reason: Option<String>,
    /// Whole sentences, shown under the CLI's heading.
    pub notes: Vec<String>,
    pub cuts: Vec<Cut>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    Missing,
    /// A network path, or a path a distro's reader may not open.
    Refused,
    TooLarge {
        bytes: u64,
    },
    Unreadable,
    Text(Arc<str>),
}

enum Guard {
    Windows,
    /// Only paths under this root.
    Distro(PathBuf),
}

/// Every instruction-file read of one run goes through here: guarded, capped at `MAX_CONFIG_BYTES`, memoized by
/// canonical path, and limited by a walk cap and a deadline.
pub struct Reader {
    guard: Guard,
    by_request: HashMap<PathBuf, Text>,
    by_canon: HashMap<PathBuf, Text>,
    nested: HashMap<PathBuf, Nested>,
    walk_cap: usize,
    deadline: Instant,
}

impl Reader {
    fn with(guard: Guard) -> Reader {
        Reader {
            guard,
            by_request: HashMap::new(),
            by_canon: HashMap::new(),
            nested: HashMap::new(),
            walk_cap: WALK_CAP,
            deadline: Instant::now() + READ_BUDGET,
        }
    }

    /// Windows paths, through the `server::local_file` guard.
    pub fn windows() -> Reader {
        Reader::with(Guard::Windows)
    }

    /// Only paths under `unc_root`; names match exactly.
    pub fn distro(unc_root: PathBuf) -> Reader {
        Reader::with(Guard::Distro(unc_root))
    }

    #[cfg(test)]
    pub fn with_walk_cap(cap: usize) -> Reader {
        let mut reader = Reader::windows();
        reader.walk_cap = cap;
        reader
    }

    pub fn out_of_time(&self) -> bool {
        Instant::now() >= self.deadline
    }

    fn ignore_case(&self) -> bool {
        matches!(self.guard, Guard::Windows)
    }

    /// The canonical path of a regular file, or why there is none.
    fn canon(&self, path: &Path) -> Result<PathBuf, Text> {
        match &self.guard {
            Guard::Windows => {
                let Some(text) = path.to_str() else {
                    return Err(Text::Unreadable);
                };
                if refused(text) || links_refused(path) {
                    return Err(Text::Refused);
                }
                local_file(text).map(PathBuf::from).ok_or(Text::Missing)
            }
            Guard::Distro(root) => {
                if !under_root(path, root) {
                    return Err(Text::Refused);
                }
                let canonical = std::fs::canonicalize(path).map_err(|_| Text::Missing)?;
                let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
                if !canonical.starts_with(&canonical_root) {
                    return Err(Text::Refused);
                }
                if !std::fs::metadata(&canonical).is_ok_and(|m| m.is_file()) {
                    return Err(Text::Missing);
                }
                Ok(canonical)
            }
        }
    }

    /// Whether the guard lets `path` be looked at at all, for folders and markers.
    fn allowed(&self, path: &Path) -> bool {
        match &self.guard {
            Guard::Windows => path.to_str().is_some_and(|t| !refused(t)) && !links_refused(path),
            Guard::Distro(root) => under_root(path, root),
        }
    }

    /// The canonical path of the file at `path`, when the guard lets it be looked at and it is a regular file.
    pub fn canonical(&mut self, path: &Path) -> Option<PathBuf> {
        self.canon(path).ok()
    }

    /// Lossy UTF-8, memoized: a file is read once per reader, even if it is gone by the second read.
    pub fn read(&mut self, path: &Path) -> Text {
        if let Some(text) = self.by_request.get(path) {
            return text.clone();
        }
        let result = match self.canon(path) {
            Err(text) => text,
            Ok(canonical) => match self.by_canon.get(&canonical) {
                Some(text) => text.clone(),
                None => {
                    let text = read_capped_text(&canonical);
                    self.by_canon.insert(canonical, text.clone());
                    text
                }
            },
        };
        self.by_request.insert(path.to_path_buf(), result.clone());
        result
    }

    /// Whether a folder is there, guarded.
    pub fn is_dir(&mut self, path: &Path) -> bool {
        self.allowed(path) && path.is_dir()
    }

    /// Whether a file or folder is there, guarded.
    pub fn exists(&mut self, path: &Path) -> bool {
        self.allowed(path) && path.exists()
    }

    /// Whether both paths are the same file, by canonical path.
    pub fn same_file(&mut self, a: &Path, b: &Path) -> bool {
        match (self.canon(a), self.canon(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }

    /// The files under `dir` whose names end in `suffix`, sorted by path; at most 1,000 files and 8 folders deep.
    pub fn list(&mut self, dir: &Path, suffix: &str, recursive: bool) -> Vec<PathBuf> {
        if !self.is_dir(dir) {
            return Vec::new();
        }
        let mut out = Vec::new();
        self.list_into(dir, suffix, recursive, 0, &mut out);
        out.sort();
        out
    }

    fn list_into(&self, dir: &Path, suffix: &str, recursive: bool, depth: usize, out: &mut Vec<PathBuf>) {
        if out.len() >= LIST_FILES || self.out_of_time() {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if out.len() >= LIST_FILES {
                return;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if recursive && depth < LIST_DEPTH {
                    self.list_into(&path, suffix, recursive, depth + 1, out);
                }
            } else if ends_with(&entry.file_name().to_string_lossy(), suffix, self.ignore_case()) {
                out.push(path);
            }
        }
    }

    /// Every `.md` file below `root` (not in it), found once per root.
    pub fn nested(&mut self, root: &Path) -> &Nested {
        if !self.nested.contains_key(root) {
            let found = self.walk(root);
            self.nested.insert(root.to_path_buf(), found);
        }
        &self.nested[root]
    }

    fn walk(&mut self, root: &Path) -> Nested {
        let mut found = Nested {
            files: Vec::new(),
            stopped: false,
        };
        if !self.is_dir(root) {
            return found;
        }
        let ignore_case = self.ignore_case();
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(false)
            .require_git(false)
            .follow_links(false)
            .max_depth(Some(WALK_DEPTH))
            .filter_entry(|entry| entry.file_name() != ".git")
            .build();
        let mut count = 0usize;
        for entry in walker {
            if self.out_of_time() {
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            if entry.depth() == 0 {
                continue;
            }
            count += 1;
            if count > self.walk_cap {
                found.stopped = true;
                break;
            }
            let is_file = entry.file_type().is_some_and(|t| t.is_file());
            if is_file && entry.depth() >= 2 && ends_with(&entry.file_name().to_string_lossy(), ".md", ignore_case) {
                found.files.push(entry.into_path());
            }
        }
        found.files.sort();
        found
    }
}

/// The `.md` files below a root, found once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nested {
    pub files: Vec<PathBuf>,
    /// The entry cap stopped the walk.
    pub stopped: bool,
}

impl Nested {
    /// The files whose names are one of `names`.
    pub fn named(&self, names: &[&str], ignore_case: bool) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|f| {
                f.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    names.iter().any(|want| {
                        if ignore_case {
                            n.eq_ignore_ascii_case(want)
                        } else {
                            n == *want
                        }
                    })
                })
            })
            .cloned()
            .collect()
    }
}

fn ends_with(name: &str, suffix: &str, ignore_case: bool) -> bool {
    if ignore_case {
        name.len() >= suffix.len()
            && name.is_char_boundary(name.len() - suffix.len())
            && name[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    } else {
        name.ends_with(suffix)
    }
}

/// Lexically under `root`, with no `..` on the way.
fn under_root(path: &Path, root: &Path) -> bool {
    path.starts_with(root) && !path.components().any(|c| matches!(c, Component::ParentDir))
}

fn read_capped_text(path: &Path) -> Text {
    let Ok(meta) = std::fs::metadata(path) else {
        return Text::Missing;
    };
    if meta.len() > MAX_CONFIG_BYTES {
        return Text::TooLarge { bytes: meta.len() };
    }
    let Ok(file) = std::fs::File::open(path) else {
        return Text::Unreadable;
    };
    let mut bytes = Vec::new();
    if file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes).is_err() {
        return Text::Unreadable;
    }
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Text::TooLarge {
            bytes: meta.len().max(bytes.len() as u64),
        };
    }
    Text::Text(Arc::from(String::from_utf8_lossy(&bytes).as_ref()))
}

/// A path as one comparable text: forward slashes, no trailing slash, ASCII-lowercased on Windows.
fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/');
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text.to_string()
    }
}

fn is_within(path: &Path, base: &Path) -> bool {
    let (path, base) = (path_key(path), path_key(base));
    path == base || path.strip_prefix(&base).is_some_and(|rest| rest.starts_with('/'))
}

/// `path` under `base` as a forward-slash text with the path's own case.
fn relative_to(path: &Path, base: &Path) -> Option<String> {
    let text = path.to_string_lossy().replace('\\', "/");
    let (key, base_key) = (path_key(path), path_key(base));
    // `path_key` only lowercases ASCII and trims a trailing slash, so byte offsets agree with `text`.
    let rest = key.strip_prefix(&base_key)?.strip_prefix('/')?;
    if rest.is_empty() {
        return None;
    }
    Some(text.get(text.len() - rest.len()..)?.to_string())
}

/// `.` and `..` resolved by the text alone.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSyntax {
    /// `@path`
    At,
    /// `@[label](path)`
    AgyLink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportBase {
    /// From the importing file's folder.
    File,
    Root(PathBuf),
}

#[derive(Clone)]
pub struct ImportRules {
    pub syntax: ImportSyntax,
    pub depth: u32,
    pub base: ImportBase,
    pub inside: Option<PathBuf>,
    /// "outside the project", "outside the repository"
    pub outside_reason: &'static str,
    /// None: a `~/` import is refused with `outside_reason`.
    pub home: Option<PathBuf>,
    /// A distro's UNC root: `/x` resolves under it.
    pub linux_root: Option<PathBuf>,
    /// What the CLI strips before counting (HTML comments, or nothing).
    pub clean: fn(&str) -> String,
}

/// The file's own row plus each import as a row (depth-first, in text order), each import read once per call.
pub fn expand(
    cx: &mut Reader,
    file: &Path,
    text: &str,
    rules: &ImportRules,
    role_of_root: Role,
    when: When,
) -> Vec<LoadedFile> {
    let cleaned = (rules.clean)(text);
    let mut run = Expander {
        cx,
        rules,
        when: &when,
        seen: HashSet::new(),
        out: vec![LoadedFile {
            path: file.to_path_buf(),
            role: role_of_root,
            when: when.clone(),
            chars: cleaned.chars().count(),
            notes: Vec::new(),
            depth: 0,
        }],
    };
    let key = run.cx.canonical(file).unwrap_or_else(|| file.to_path_buf());
    run.seen.insert(key);
    run.imports_of(file, &cleaned, 0);
    run.out
}

struct Expander<'a> {
    cx: &'a mut Reader,
    rules: &'a ImportRules,
    when: &'a When,
    seen: HashSet<PathBuf>,
    out: Vec<LoadedFile>,
}

enum Target {
    Path(PathBuf),
    Outside(PathBuf),
}

impl Expander<'_> {
    fn row(&mut self, path: PathBuf, by: &Path, depth: u32, when: When, chars: usize, notes: Vec<String>) {
        self.out.push(LoadedFile {
            path,
            role: Role::Import { by: by.to_path_buf() },
            when,
            chars,
            notes,
            depth,
        });
    }

    fn imports_of(&mut self, file: &Path, cleaned: &str, depth: u32) {
        for raw in imports_in(cleaned, self.rules.syntax) {
            let depth = depth + 1;
            let path = match resolve(&raw, file, self.rules) {
                Target::Outside(path) => {
                    let reason = self.rules.outside_reason.to_string();
                    self.row(path, file, depth, When::NotLoaded(reason), 0, Vec::new());
                    continue;
                }
                Target::Path(path) => path,
            };
            if depth > self.rules.depth {
                let when = When::NotLoaded("past the import limit".into());
                self.row(path, file, depth, when, 0, Vec::new());
                continue;
            }
            let (chars, notes, inner) = match self.cx.read(&path) {
                Text::Refused => {
                    let when = When::NotLoaded("Marshell doesn't read network paths".into());
                    self.row(path, file, depth, when, 0, Vec::new());
                    continue;
                }
                Text::Missing => {
                    self.row(path, file, depth, When::NotLoaded("not found".into()), 0, Vec::new());
                    continue;
                }
                Text::Unreadable => {
                    let when = When::NotLoaded("Marshell can't read it".into());
                    self.row(path, file, depth, when, 0, Vec::new());
                    continue;
                }
                Text::TooLarge { bytes } => (
                    usize::try_from(bytes).unwrap_or(usize::MAX),
                    vec!["over 4 MiB, size estimated from its bytes".to_string()],
                    None,
                ),
                Text::Text(text) => {
                    let cleaned = (self.rules.clean)(&text);
                    (cleaned.chars().count(), Vec::new(), Some(cleaned))
                }
            };
            let key = self.cx.canonical(&path).unwrap_or_else(|| path.clone());
            if !self.seen.insert(key) {
                self.row(
                    path,
                    file,
                    depth,
                    When::NotLoaded("already loaded".into()),
                    0,
                    Vec::new(),
                );
                continue;
            }
            self.row(path.clone(), file, depth, self.when.clone(), chars, notes);
            if let Some(cleaned) = inner {
                self.imports_of(&path, &cleaned, depth);
            }
        }
    }
}

fn resolve(raw: &str, file: &Path, rules: &ImportRules) -> Target {
    let path = if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        match &rules.home {
            Some(home) => home.join(rest),
            None => return Target::Outside(PathBuf::from(raw)),
        }
    } else if raw.starts_with('/') {
        match &rules.linux_root {
            Some(root) => root.join(raw.trim_start_matches('/')),
            None => PathBuf::from(raw),
        }
    } else if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        let base = match &rules.base {
            ImportBase::File => file.parent().map(Path::to_path_buf).unwrap_or_default(),
            ImportBase::Root(root) => root.clone(),
        };
        base.join(raw)
    };
    let path = normalize(&path);
    match &rules.inside {
        Some(inside) if !is_within(&path, inside) => Target::Outside(path),
        _ => Target::Path(path),
    }
}

/// A line that opens or closes a fenced code block.
fn is_fence(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("```") || line.starts_with("~~~")
}

/// Which characters of a line sit inside an inline code span.
fn code_mask(chars: &[char]) -> Vec<bool> {
    let mut mask = vec![false; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '`' {
            i += 1;
            continue;
        }
        let run = chars[i..].iter().take_while(|c| **c == '`').count();
        let mut j = i + run;
        let mut close = None;
        while j < chars.len() {
            if chars[j] == '`' {
                let len = chars[j..].iter().take_while(|c| **c == '`').count();
                if len == run {
                    close = Some(j + len);
                    break;
                }
                j += len;
            } else {
                j += 1;
            }
        }
        match close {
            Some(end) => {
                mask[i..end].iter_mut().for_each(|m| *m = true);
                i = end;
            }
            None => i += run,
        }
    }
    mask
}

/// The paths a text imports, in order. Nothing in a fenced block or an inline code span counts.
fn imports_in(text: &str, syntax: ImportSyntax) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let code = code_mask(&chars);
        let mut i = 0;
        while i < chars.len() {
            if chars[i] != '@' || code[i] {
                i += 1;
                continue;
            }
            match syntax {
                ImportSyntax::At => {
                    let boundary = i == 0 || chars[i - 1].is_whitespace();
                    let starts = chars
                        .get(i + 1)
                        .is_some_and(|c| matches!(c, '.' | '/' | '~') || c.is_alphabetic());
                    if !boundary || !starts {
                        i += 1;
                        continue;
                    }
                    let mut j = i + 1;
                    let mut path = String::new();
                    while j < chars.len() && !chars[j].is_whitespace() {
                        if chars[j] == '\\' && chars.get(j + 1) == Some(&' ') {
                            path.push(' ');
                            j += 2;
                        } else {
                            path.push(chars[j]);
                            j += 1;
                        }
                    }
                    found.push(path);
                    i = j;
                }
                ImportSyntax::AgyLink => {
                    let link = (chars.get(i + 1) == Some(&'['))
                        .then(|| chars[i + 2..].iter().position(|c| *c == ']').map(|p| i + 2 + p))
                        .flatten()
                        .filter(|close| chars.get(close + 1) == Some(&'('))
                        .and_then(|close| {
                            let end = chars[close + 2..].iter().position(|c| *c == ')')? + close + 2;
                            Some((close + 2, end))
                        });
                    match link {
                        Some((start, end)) => {
                            let path: String = chars[start..end].iter().collect();
                            if !path.trim().is_empty() {
                                found.push(path.trim().to_string());
                            }
                            i = end + 1;
                        }
                        None => i += 1,
                    }
                }
            }
        }
    }
    found
}

/// Block comments only, as Claude Code and Qwen Code strip them. One in a code fence is kept, and one that is never
/// closed is left as it is.
pub fn strip_html_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    let mut i = 0;
    while i < text.len() {
        let end = text[i..].find('\n').map_or(text.len(), |n| i + n + 1);
        let line = &text[i..end];
        if is_fence(line) {
            in_fence = !in_fence;
        } else if !in_fence
            && let Some(at) = line.find("<!--")
            && let Some(close) = text[i + at + 4..].find("-->")
        {
            out.push_str(&line[..at]);
            i += at + 4 + close + 3;
            continue;
        }
        out.push_str(line);
        i = end;
    }
    out
}

/// A key's value in the file's frontmatter: `Some("")` for a key with a list under it, None for no frontmatter or
/// no such key.
pub fn frontmatter_value(text: &str, key: &str) -> Option<String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return None;
        }
        if let Some(rest) = line.strip_prefix(key)
            && let Some(value) = rest.trim_start().strip_prefix(':')
        {
            let value = value.trim();
            let unquoted = ['"', '\'']
                .iter()
                .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
                .unwrap_or(value);
            return Some(unquoted.to_string());
        }
    }
    None
}

/// "600", "2.4k": from 1,000 up one decimal, rounded half up.
pub fn tokens_text(tokens: usize) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }
    let tenths = (tokens + 50) / 100;
    format!("{}.{}k", tenths / 10, tenths % 10)
}

/// The 8 hex digits of a one-file cut's id: the hash of the file's canonical path, forward slashes, lowercased on
/// Windows.
pub fn hash_part(path: &Path) -> String {
    let text = path.to_string_lossy();
    let canonical = (!refused(&text) && !links_refused(path))
        .then(|| std::fs::canonicalize(path).ok())
        .flatten()
        .map(|c| c.to_string_lossy().into_owned())
        .map(|c| match c.strip_prefix(r"\\?\") {
            Some(rest) if !rest.starts_with("UNC\\") => rest.to_string(),
            _ => c,
        });
    let mut text = canonical.unwrap_or_else(|| text.into_owned()).replace('\\', "/");
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    let mut hash = hash_bytes(text.as_bytes());
    hash.truncate(8);
    hash
}

fn cli_rank(agent: &str) -> usize {
    ORDER.iter().position(|a| *a == agent).unwrap_or(ORDER.len())
}

/// A path as the expansion shows it: project-relative, `~/…` under the home, else in full.
fn display(path: &Path, project: Option<&Path>, home: &Path) -> String {
    if let Some(rel) = project.and_then(|p| relative_to(path, p)) {
        return rel;
    }
    if let Some(rel) = relative_to(path, home) {
        return format!("~/{rel}");
    }
    path.display().to_string()
}

fn start_chars(load: &CliLoad) -> usize {
    load.files
        .iter()
        .filter(|f| matches!(f.when, When::Start))
        .map(|f| f.chars)
        .sum()
}

fn file_note(file: &LoadedFile, load: &CliLoad) -> String {
    let project = load.project.as_deref();
    let mut parts = Vec::new();
    if !matches!(file.when, When::NotLoaded(_)) {
        parts.push(format!("about {} tokens", tokens_text(tokens(file.chars))));
    }
    if let Role::Import { by } = &file.role {
        parts.push(format!("imported by {}", display(by, project, &load.home)));
    }
    if matches!(file.role, Role::Memory) {
        // The CLI's own limit ("first 200 lines or 25 KB") follows from the file's notes.
        parts.push("auto memory".to_string());
    }
    match &file.when {
        When::Start => {}
        When::WhenUsed => parts.push("loads when used".to_string()),
        When::NotLoaded(reason) => parts.push(format!("not loaded: {reason}")),
    }
    parts.extend(file.notes.iter().cloned());
    if matches!(file.role, Role::Managed) {
        parts.push("managed by your organisation".to_string());
    }
    parts.join(", ")
}

fn doctor_load(load: &CliLoad) -> DoctorLoad {
    let project = load.project.as_deref();
    DoctorLoad {
        agent: load.agent.to_string(),
        heading: format!(
            "{}: about {} tokens at start",
            display_name(load.agent),
            tokens_text(tokens(start_chars(load)))
        ),
        files: load
            .files
            .iter()
            .map(|f| DoctorLoadFile {
                path: display(&f.path, project, &load.home),
                when: match f.when {
                    When::Start => DoctorLoadWhen::Start,
                    When::WhenUsed => DoctorLoadWhen::WhenUsed,
                    When::NotLoaded(_) => DoctorLoadWhen::NotLoaded,
                },
                note: file_note(f, load),
                depth: f.depth,
            })
            .collect(),
        notes: load.notes.clone(),
    }
}

fn advice(agent: &str) -> Option<&'static str> {
    Some(match agent {
        "claude" => "Move the parts it needs only sometimes into .claude/rules/ files with paths:, or into a skill.",
        "codex" => "Move the parts about one folder into an AGENTS.md in that folder.",
        "antigravity" => "Move the parts it needs only sometimes into rules with trigger: model_decision.",
        "qwen" => "Move the parts it needs only sometimes into .qwen/rules/ files with paths:.",
        "copilot" => "Move the parts it needs only sometimes into .github/instructions/ files with applyTo.",
        _ => return None,
    })
}

/// The loads of one place: one project, "outside a project", or one distro's user level.
struct Line<'a> {
    project: Option<&'a Path>,
    distro: Option<&'a str>,
    loads: Vec<&'a CliLoad>,
}

impl Line<'_> {
    /// The ids' tail: `<project_id>` or `user`, then `@<distro>`.
    fn id_tail(&self) -> (String, String) {
        let place = self.project.map_or_else(|| "user".to_string(), project_id);
        let suffix = self.distro.map(|d| format!("@{d}")).unwrap_or_default();
        (place, suffix)
    }

    fn title(&self) -> String {
        let user = if self.distro.is_some() { "user " } else { "" };
        let lead = match (self.distro, self.project) {
            (Some(distro), _) => format!("In {distro}, at start: "),
            (None, Some(project)) => format!("{}: at start, ", folder_name(project)),
            (None, None) => "Outside a project: at start, ".to_string(),
        };
        let parts: Vec<String> = self
            .loads
            .iter()
            .enumerate()
            .map(|(i, load)| {
                let chars = start_chars(load);
                let cli = display_name(load.agent);
                let total = tokens_text(tokens(chars));
                let mut part = match (i, chars) {
                    (0, 0) => format!("{cli} loads no {user}instructions"),
                    (0, _) => format!("{cli} loads about {total} tokens of {user}instructions"),
                    (_, 0) => format!("{cli} none"),
                    _ => format!("{cli} {total}"),
                };
                if let Some(reason) = &load.reason {
                    part.push_str(&format!(" ({reason})"));
                }
                part
            })
            .collect();
        format!("{lead}{}.", parts.join(", "))
    }

    fn detail(&self) -> String {
        match self.distro {
            Some(distro) => format!("Projects inside {distro} aren't checked."),
            None => "Estimates: characters divided by 4. Files that load later aren't counted.".to_string(),
        }
    }

    /// What the Waste title says the CLI loads in.
    fn where_text(&self) -> String {
        match (self.distro, self.project) {
            (Some(distro), _) => distro.to_string(),
            (None, Some(project)) => folder_name(project),
            (None, None) => "every folder".to_string(),
        }
    }
}

fn folder_name(project: &Path) -> String {
    project.file_name().map_or_else(
        || project.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Every instruction item: one Info line per place, then a Waste item per CLI over 4,000 tokens at start, then the
/// cuts. Project lines come in `projects` order. No loads give no item.
pub fn items(loads: &[CliLoad], projects: &[PathBuf]) -> Vec<DoctorItem> {
    let mut lines: Vec<Line> = Vec::new();
    for load in loads {
        let project = load.project.as_deref();
        let distro = load.distro.as_deref();
        match lines
            .iter_mut()
            .find(|l| l.distro == distro && l.project.map(path_key) == project.map(path_key))
        {
            Some(line) => line.loads.push(load),
            None => lines.push(Line {
                project,
                distro,
                loads: vec![load],
            }),
        }
    }
    let project_rank = |line: &Line| match line.project {
        Some(p) => projects
            .iter()
            .position(|q| path_key(q) == path_key(p))
            .unwrap_or(projects.len()),
        None => usize::MAX,
    };
    lines.sort_by(|a, b| (a.distro, project_rank(a)).cmp(&(b.distro, project_rank(b))));
    for line in &mut lines {
        line.loads.sort_by_key(|l| cli_rank(l.agent));
    }

    let mk = |id: String,
              agent: Option<&str>,
              distro: Option<&str>,
              group: DoctorGroup,
              title: String,
              detail: String,
              cannot_fix: Option<String>,
              path: Option<String>,
              loads: Vec<DoctorLoad>| DoctorItem {
        id,
        agent: agent.map(str::to_string),
        distro: distro.map(str::to_string),
        group,
        title,
        detail,
        fixable: false,
        cannot_fix,
        path,
        open_agents: false,
        unchecked: false,
        action: None,
        loads,
    };

    let mut out = Vec::new();
    let mut waste = Vec::new();
    let mut cuts = Vec::new();
    for line in &lines {
        let (place, suffix) = line.id_tail();
        let id = match (line.project, line.distro) {
            (None, _) => format!("instr.user{suffix}"),
            (Some(_), _) => format!("instr.{place}{suffix}"),
        };
        out.push(mk(
            id,
            None,
            line.distro,
            DoctorGroup::Info,
            line.title(),
            line.detail(),
            None,
            None,
            line.loads.iter().map(|l| doctor_load(l)).collect(),
        ));

        for load in &line.loads {
            let total = tokens(start_chars(load));
            if total > WASTE_TOKENS {
                let mut start: Vec<&LoadedFile> = load.files.iter().filter(|f| matches!(f.when, When::Start)).collect();
                start.sort_by_key(|f| std::cmp::Reverse(f.chars));
                let largest: Vec<String> = start
                    .iter()
                    .take(3)
                    .map(|f| {
                        format!(
                            "{} ({})",
                            display(&f.path, load.project.as_deref(), &load.home),
                            tokens_text(tokens(f.chars))
                        )
                    })
                    .collect();
                waste.push(mk(
                    format!("instrsize.{}.{place}{suffix}", load.agent),
                    Some(load.agent),
                    line.distro,
                    DoctorGroup::Waste,
                    format!(
                        "{} loads about {} tokens of instructions at every start in {}",
                        display_name(load.agent),
                        tokens_text(total),
                        line.where_text()
                    ),
                    format!("The largest: {}.", largest.join(", ")),
                    advice(load.agent).map(str::to_string),
                    None,
                    Vec::new(),
                ));
            }
            for cut in &load.cuts {
                cuts.push(mk(
                    format!("instrcut.{}.{place}.{}{suffix}", load.agent, cut.id_part),
                    Some(load.agent),
                    line.distro,
                    DoctorGroup::Broken,
                    cut.title.clone(),
                    cut.detail.clone(),
                    Some(cut.cannot_fix.clone()),
                    cut.path
                        .as_deref()
                        .map(|p| display(p, load.project.as_deref(), &load.home)),
                    Vec::new(),
                ));
            }
        }
    }
    out.extend(waste);
    out.extend(cuts);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn rules() -> ImportRules {
        ImportRules {
            syntax: ImportSyntax::At,
            depth: 4,
            base: ImportBase::File,
            inside: None,
            outside_reason: "outside the project",
            home: None,
            linux_root: None,
            clean: |s| s.to_string(),
        }
    }

    fn expand_file(cx: &mut Reader, file: &Path, rules: &ImportRules) -> Vec<LoadedFile> {
        let text = fs::read_to_string(file).unwrap();
        expand(cx, file, &text, rules, Role::File, When::Start)
    }

    fn names(rows: &[LoadedFile]) -> Vec<String> {
        rows.iter()
            .map(|r| r.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    // The reader

    #[test]
    fn a_file_over_four_mib_is_too_large_with_its_size() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("big.md");
        fs::write(&file, vec![b'a'; 4 * 1024 * 1024 + 1]).unwrap();
        assert_eq!(Reader::windows().read(&file), Text::TooLarge { bytes: 4_194_305 });
        let exact = tmp.path().join("exact.md");
        fs::write(&exact, vec![b'a'; 4 * 1024 * 1024]).unwrap();
        assert!(matches!(Reader::windows().read(&exact), Text::Text(_)));
    }

    #[test]
    fn non_utf8_text_is_counted_lossily() {
        let tmp = tempfile::tempdir().unwrap();
        let utf16 = tmp.path().join("utf16.md");
        let bytes: Vec<u8> = "# Hi\n".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        fs::write(&utf16, [vec![0xff, 0xfe], bytes].concat()).unwrap();
        let png = tmp.path().join("pic.md");
        fs::write(&png, [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0xff, 0xfe, 0x00]).unwrap();
        let mut cx = Reader::windows();
        assert!(matches!(cx.read(&utf16), Text::Text(_)));
        assert!(matches!(cx.read(&png), Text::Text(_)));
    }

    #[test]
    fn the_reader_reads_a_file_once() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.md");
        fs::write(&file, "first").unwrap();
        let mut cx = Reader::windows();
        let first = cx.read(&file);
        assert_eq!(first, Text::Text(Arc::from("first")));
        fs::remove_file(&file).unwrap();
        assert_eq!(cx.read(&file), first);
    }

    #[test]
    fn a_missing_file_reads_as_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(Reader::windows().read(&tmp.path().join("nope.md")), Text::Missing);
    }

    #[test]
    fn a_network_path_is_refused() {
        assert_eq!(Reader::windows().read(Path::new("//host/share/x.md")), Text::Refused);
    }

    #[test]
    fn a_distro_reader_refuses_paths_outside_its_root() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("root").join("in.md"), "inside");
        write(&tmp.path().join("other").join("out.md"), "outside");
        let mut cx = Reader::distro(tmp.path().join("root"));
        assert_eq!(cx.read(&tmp.path().join("other").join("out.md")), Text::Refused);
        assert_eq!(
            cx.read(&tmp.path().join("root").join("..").join("other").join("out.md")),
            Text::Refused
        );
        assert_eq!(
            cx.read(&tmp.path().join("root").join("in.md")),
            Text::Text(Arc::from("inside"))
        );
    }

    #[test]
    fn list_finds_nested_rule_files_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let rules_dir = tmp.path().join("rules");
        write(&rules_dir.join("x").join("b.md"), "b");
        write(&rules_dir.join("a.md"), "a");
        write(&rules_dir.join("c.txt"), "c");
        let mut cx = Reader::windows();
        assert_eq!(
            cx.list(&rules_dir, ".md", true),
            vec![rules_dir.join("a.md"), rules_dir.join("x").join("b.md")]
        );
        assert_eq!(cx.list(&rules_dir, ".md", false), vec![rules_dir.join("a.md")]);
        assert!(cx.list(&tmp.path().join("none"), ".md", true).is_empty());
    }

    // The place

    #[test]
    fn ancestors_stop_at_the_ceiling() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = ExtRoots::under(&tmp.path().join("home"));
        let project = tmp.path().join("a").join("b");
        let place = Place {
            roots: &roots,
            project: Some(&project),
            ceiling: Some(tmp.path()),
            claude_managed: None,
            qwen_system_settings: None,
            distro: None,
        };
        assert_eq!(
            place.ancestors(),
            vec![tmp.path().to_path_buf(), tmp.path().join("a"), project.clone()]
        );
        let none = Place { project: None, ..place };
        assert!(none.ancestors().is_empty());
    }

    #[test]
    fn marked_root_never_passes_the_ceiling() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = ExtRoots::under(&tmp.path().join("home"));
        let ceiling = tmp.path().join("repo");
        let project = ceiling.join("sub");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let place = Place {
            roots: &roots,
            project: Some(&project),
            ceiling: Some(&ceiling),
            claude_managed: None,
            qwen_system_settings: None,
            distro: None,
        };
        let mut cx = Reader::windows();
        assert_eq!(place.marked_root(&mut cx, &[".git"]), None);
        fs::create_dir_all(ceiling.join(".git")).unwrap();
        assert_eq!(place.marked_root(&mut cx, &[".git"]), Some(ceiling));
    }

    // Imports

    #[test]
    fn an_at_import_inside_code_is_not_an_import() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("a.md"), "x");
        write(&tmp.path().join("b.md"), "y");
        let file = tmp.path().join("root.md");
        write(&file, "```\n@a.md\n```\nuse `@b.md` here, or `see @b.md` there\n");
        let rows = expand_file(&mut Reader::windows(), &file, &rules());
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn an_at_import_is_found_after_whitespace_and_stops_at_whitespace() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("a.md"), "x");
        write(&tmp.path().join("my notes.md"), "yy");
        let file = tmp.path().join("root.md");
        write(&file, "see @a.md and @my\\ notes.md\nmail me@a.md\n");
        let rows = expand_file(&mut Reader::windows(), &file, &rules());
        assert_eq!(names(&rows), ["root.md", "a.md", "my notes.md"]);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[1].role, Role::Import { by: file.clone() });
    }

    #[test]
    fn an_import_cycle_stops() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("a.md"), "@b.md");
        write(&tmp.path().join("b.md"), "@a.md");
        let rows = expand_file(&mut Reader::windows(), &tmp.path().join("a.md"), &rules());
        assert_eq!(names(&rows), ["a.md", "b.md", "a.md"]);
        assert_eq!(rows[2].when, When::NotLoaded("already loaded".into()));
        assert_eq!(rows[1].when, When::Start);
    }

    #[test]
    fn the_depth_limit_holds() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..6 {
            write(&tmp.path().join(format!("f{i}.md")), &format!("@f{}.md", i + 1));
        }
        write(&tmp.path().join("f6.md"), "end");
        let rows = expand_file(&mut Reader::windows(), &tmp.path().join("f0.md"), &rules());
        assert_eq!(rows.len(), 6);
        assert!(rows[1..5].iter().all(|r| r.when == When::Start));
        assert_eq!(rows[5].when, When::NotLoaded("past the import limit".into()));
        assert_eq!(rows[5].depth, 5);
    }

    #[test]
    fn an_import_outside_inside_gets_the_cli_reason() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("out.md"), "x");
        let file = tmp.path().join("repo").join("a.md");
        write(&file, "@../out.md");
        let rules = ImportRules {
            inside: Some(tmp.path().join("repo")),
            outside_reason: "outside the repository",
            ..rules()
        };
        let rows = expand_file(&mut Reader::windows(), &file, &rules);
        assert_eq!(rows[1].when, When::NotLoaded("outside the repository".into()));
        assert_eq!(rows[1].chars, 0);
    }

    #[test]
    fn a_home_import_without_a_home_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.md");
        write(&file, "@~/x.md");
        let rows = expand_file(&mut Reader::windows(), &file, &rules());
        assert_eq!(rows[1].when, When::NotLoaded("outside the project".into()));
    }

    #[test]
    fn a_home_import_resolves_under_the_home() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("home").join("x.md"), "abcd");
        let file = tmp.path().join("a.md");
        write(&file, "@~/x.md");
        let rules = ImportRules {
            home: Some(tmp.path().join("home")),
            ..rules()
        };
        let rows = expand_file(&mut Reader::windows(), &file, &rules);
        assert_eq!(rows[1].when, When::Start);
        assert_eq!(rows[1].chars, 4);
    }

    #[test]
    fn a_missing_import_is_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.md");
        write(&file, "@gone.md");
        let rows = expand_file(&mut Reader::windows(), &file, &rules());
        assert_eq!(rows[1].when, When::NotLoaded("not found".into()));
    }

    #[test]
    fn a_network_import_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.md");
        write(&file, "@//host/share/x.md");
        let rows = expand_file(&mut Reader::windows(), &file, &rules());
        assert_eq!(
            rows[1].when,
            When::NotLoaded("Marshell doesn't read network paths".into())
        );
    }

    #[test]
    fn a_linux_absolute_import_resolves_under_the_distro_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        write(&root.join("etc").join("x.md"), "abcdefgh");
        let file = root.join("home").join("CLAUDE.md");
        write(&file, "@/etc/x.md");
        let rules = ImportRules {
            linux_root: Some(root.clone()),
            ..rules()
        };
        let mut cx = Reader::distro(root.clone());
        let rows = expand_file(&mut cx, &file, &rules);
        assert_eq!(rows[1].path, root.join("etc").join("x.md"));
        assert_eq!(rows[1].when, When::Start);
        assert_eq!(rows[1].chars, 8);
    }

    #[test]
    fn an_agy_link_include_is_read() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("rules").join("sub").join("a.md"), "abc");
        let file = tmp.path().join("rules").join("r.md");
        write(&file, "Include @[x](sub/a.md) now.\n@plain.md is not one\n");
        let rules = ImportRules {
            syntax: ImportSyntax::AgyLink,
            ..rules()
        };
        let rows = expand_file(&mut Reader::windows(), &file, &rules);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].path, tmp.path().join("rules").join("sub").join("a.md"));
        assert_eq!(rows[1].chars, 3);
    }

    #[test]
    fn a_root_based_import_resolves_from_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("shared.md"), "abc");
        let file = tmp.path().join("deep").join("a.md");
        write(&file, "@shared.md");
        let rules = ImportRules {
            base: ImportBase::Root(tmp.path().to_path_buf()),
            ..rules()
        };
        let rows = expand_file(&mut Reader::windows(), &file, &rules);
        assert_eq!(rows[1].when, When::Start);
    }

    #[test]
    fn imports_are_cleaned_before_counting() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("b.md"), "<!-- only a comment -->");
        let file = tmp.path().join("a.md");
        write(&file, "@b.md<!-- note -->");
        let rules = ImportRules {
            clean: strip_html_comments,
            ..rules()
        };
        let rows = expand_file(&mut Reader::windows(), &file, &rules);
        assert_eq!(rows[0].chars, 5);
        assert_eq!(rows[1].chars, 0);
    }

    #[test]
    fn an_import_of_a_file_over_four_mib_counts_its_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("big.md"), vec![b'a'; 4 * 1024 * 1024 + 1]).unwrap();
        let file = tmp.path().join("a.md");
        write(&file, "@big.md");
        let rows = expand_file(&mut Reader::windows(), &file, &rules());
        assert_eq!(rows[1].chars, 4_194_305);
        assert_eq!(rows[1].notes, ["over 4 MiB, size estimated from its bytes"]);
    }

    // Text helpers

    #[test]
    fn html_block_comments_are_stripped() {
        assert_eq!(strip_html_comments("a<!-- x\ny -->b\n"), "ab\n");
        let fenced = "```\n<!-- x -->\n```\n";
        assert_eq!(strip_html_comments(fenced), fenced);
        assert_eq!(strip_html_comments("open <!-- never closed"), "open <!-- never closed");
    }

    #[test]
    fn frontmatter_values_are_found() {
        assert_eq!(
            frontmatter_value("---\npaths:\n  - src/**\n---\nbody", "paths"),
            Some(String::new())
        );
        assert_eq!(frontmatter_value("# no frontmatter\npaths: x", "paths"), None);
        assert_eq!(
            frontmatter_value("---\ntrigger: always_on\n---\n", "trigger"),
            Some("always_on".to_string())
        );
        assert_eq!(
            frontmatter_value("---\ntrigger: \"glob\"\n---\n", "trigger"),
            Some("glob".into())
        );
        assert_eq!(frontmatter_value("---\na: 1\n---\ntrigger: x", "trigger"), None);
    }

    // The nested walk

    #[test]
    fn the_walk_honours_gitignore_and_skips_dot_git() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        write(&root.join(".gitignore"), "node_modules/\n");
        write(&root.join("node_modules").join("AGENTS.md"), "x");
        write(&root.join(".git").join("AGENTS.md"), "x");
        write(&root.join("docs").join("AGENTS.md"), "x");
        write(&root.join("AGENTS.md"), "root file is not below the root");
        let mut cx = Reader::windows();
        let found = cx.nested(&root);
        assert_eq!(found.files, vec![root.join("docs").join("AGENTS.md")]);
        assert!(!found.stopped);

        // No `.git` folder at all: the `.gitignore` still counts.
        let bare = tmp.path().join("bare");
        write(&bare.join(".gitignore"), "build/\n");
        write(&bare.join("build").join("AGENTS.md"), "x");
        write(&bare.join("src").join("AGENTS.md"), "x");
        assert_eq!(cx.nested(&bare).files, vec![bare.join("src").join("AGENTS.md")]);
    }

    #[test]
    fn the_walk_stops_at_the_entry_cap() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..20 {
            write(&tmp.path().join(format!("d{i:02}")).join("AGENTS.md"), "x");
        }
        let mut cx = Reader::with_walk_cap(5);
        assert!(cx.nested(tmp.path()).stopped);
        let mut wide = Reader::windows();
        let all = wide.nested(tmp.path());
        assert!(!all.stopped);
        assert_eq!(all.files.len(), 20);
    }

    #[test]
    fn named_filters_by_case_rule() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("a").join("agents.md"), "x");
        write(&tmp.path().join("b").join("notes.md"), "x");
        let mut cx = Reader::windows();
        let found = cx.nested(tmp.path());
        assert_eq!(
            found.named(&["AGENTS.md"], true),
            vec![tmp.path().join("a").join("agents.md")]
        );
        assert!(found.named(&["AGENTS.md"], false).is_empty());
    }

    // Items

    fn load(agent: &'static str, project: Option<&Path>, home: &Path, files: Vec<LoadedFile>) -> CliLoad {
        CliLoad {
            agent,
            distro: None,
            project: project.map(Path::to_path_buf),
            home: home.to_path_buf(),
            files,
            reason: None,
            notes: Vec::new(),
            cuts: Vec::new(),
        }
    }

    fn start(path: PathBuf, chars: usize) -> LoadedFile {
        LoadedFile {
            path,
            role: Role::File,
            when: When::Start,
            chars,
            notes: Vec::new(),
            depth: 0,
        }
    }

    fn project_in(tmp: &Path) -> PathBuf {
        let project = tmp.join("proj");
        fs::create_dir_all(&project).unwrap();
        project
    }

    #[test]
    fn tokens_text_rounds() {
        assert_eq!(tokens_text(0), "0");
        assert_eq!(tokens_text(999), "999");
        assert_eq!(tokens_text(1_000), "1.0k");
        assert_eq!(tokens_text(2_449), "2.4k");
        assert_eq!(tokens_text(2_450), "2.5k");
        assert_eq!(tokens_text(10_000), "10.0k");
    }

    #[test]
    fn hash_parts_are_eight_hex_digits_and_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.md");
        fs::write(&file, "x").unwrap();
        let part = hash_part(&file);
        assert_eq!(part.len(), 8);
        assert!(part.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(part, hash_part(&file));
        assert_ne!(part, hash_part(&tmp.path().join("b.md")));
    }

    #[test]
    fn no_loads_give_no_items() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(items(&[], &[project_in(tmp.path())]).is_empty());
    }

    #[test]
    fn items_make_one_line_per_project() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let home = tmp.path().join("home");
        // Given out of order, shown in `checks::ORDER`.
        let loads = [
            load("codex", Some(&project), &home, Vec::new()),
            load(
                "claude",
                Some(&project),
                &home,
                vec![start(project.join("CLAUDE.md"), 12)],
            ),
        ];
        let got = items(&loads, std::slice::from_ref(&project));
        assert_eq!(got.len(), 1);
        let item = &got[0];
        assert_eq!(item.id, format!("instr.{}", project_id(&project)));
        assert_eq!(item.group, DoctorGroup::Info);
        assert!(!item.fixable);
        assert_eq!(
            item.title,
            "proj: at start, Claude Code loads about 3 tokens of instructions, Codex none."
        );
        assert_eq!(
            item.detail,
            "Estimates: characters divided by 4. Files that load later aren't counted."
        );
        assert_eq!(item.loads.len(), 2);
        assert_eq!(item.loads[0].heading, "Claude Code: about 3 tokens at start");
        assert_eq!(item.loads[0].files[0].path, "CLAUDE.md");
        assert_eq!(item.loads[0].files[0].note, "about 3 tokens");
    }

    #[test]
    fn project_lines_follow_the_projects_order() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let home = tmp.path().join("home");
        let loads = [
            load("claude", Some(&a), &home, Vec::new()),
            load("claude", Some(&b), &home, Vec::new()),
        ];
        let got = items(&loads, &[b.clone(), a.clone()]);
        assert!(got[0].title.starts_with("b:"));
        assert!(got[1].title.starts_with("a:"));
    }

    #[test]
    fn a_first_cli_with_nothing_loads_no_instructions() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let home = tmp.path().join("home");
        let loads = [
            load("claude", Some(&project), &home, Vec::new()),
            load(
                "codex",
                Some(&project),
                &home,
                vec![start(project.join("AGENTS.md"), 45)],
            ),
        ];
        let got = items(&loads, std::slice::from_ref(&project));
        assert_eq!(
            got[0].title,
            "proj: at start, Claude Code loads no instructions, Codex 12."
        );
    }

    #[test]
    fn a_reason_shows_beside_a_total() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let home = tmp.path().join("home");
        let mut codex = load(
            "codex",
            Some(&project),
            &home,
            vec![start(home.join(".codex").join("AGENTS.md"), 45)],
        );
        codex.reason = Some("Codex doesn't trust this folder".into());
        let loads = [load("claude", Some(&project), &home, Vec::new()), codex];
        let got = items(&loads, std::slice::from_ref(&project));
        assert_eq!(
            got[0].title,
            "proj: at start, Claude Code loads no instructions, Codex 12 (Codex doesn't trust this folder)."
        );
        assert_eq!(got[0].loads[1].files[0].path, "~/.codex/AGENTS.md");
    }

    #[test]
    fn over_4k_is_waste() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let home = tmp.path().join("home");
        let files = vec![
            start(project.join("CLAUDE.md"), 10_000),
            start(project.join("docs").join("b.md"), 3_000),
            start(project.join("c.md"), 2_000),
            start(project.join("d.md"), 1_004),
        ];
        let loads = [load("claude", Some(&project), &home, files)];
        let got = items(&loads, std::slice::from_ref(&project));
        assert_eq!(got.len(), 2);
        let waste = &got[1];
        assert_eq!(waste.id, format!("instrsize.claude.{}", project_id(&project)));
        assert_eq!(waste.group, DoctorGroup::Waste);
        assert!(!waste.fixable);
        assert_eq!(
            waste.title,
            "Claude Code loads about 4.0k tokens of instructions at every start in proj"
        );
        assert_eq!(
            waste.detail,
            "The largest: CLAUDE.md (2.5k), docs/b.md (750), c.md (500)."
        );
        assert_eq!(
            waste.cannot_fix.as_deref(),
            Some("Move the parts it needs only sometimes into .claude/rules/ files with paths:, or into a skill.")
        );
    }

    #[test]
    fn exactly_4k_is_not_waste() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let loads = [load(
            "claude",
            Some(&project),
            tmp.path(),
            vec![start(project.join("CLAUDE.md"), 16_000)],
        )];
        assert_eq!(items(&loads, std::slice::from_ref(&project)).len(), 1);
    }

    #[test]
    fn only_start_files_count_toward_waste() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let mut later = start(project.join("later.md"), 40_000);
        later.when = When::WhenUsed;
        let loads = [load("claude", Some(&project), tmp.path(), vec![later])];
        let got = items(&loads, std::slice::from_ref(&project));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].loads[0].files[0].note, "about 10.0k tokens, loads when used");
    }

    #[test]
    fn a_cut_is_broken_and_report_only() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let mut codex = load("codex", Some(&project), tmp.path(), Vec::new());
        codex.cuts.push(Cut {
            id_part: "budget".into(),
            path: Some(project.join("AGENTS.md")),
            title: "Codex cuts text".into(),
            detail: "It stops reading.".into(),
            cannot_fix: "Shorten the files.".into(),
        });
        let got = items(&[codex], std::slice::from_ref(&project));
        let cut = got.iter().find(|i| i.group == DoctorGroup::Broken).unwrap();
        assert_eq!(cut.id, format!("instrcut.codex.{}.budget", project_id(&project)));
        assert!(!cut.fixable);
        assert!(!cut.unchecked);
        assert_eq!(cut.agent.as_deref(), Some("codex"));
        assert_eq!(cut.cannot_fix.as_deref(), Some("Shorten the files."));
        assert_eq!(cut.path.as_deref(), Some("AGENTS.md"));
        assert_eq!(got.last().unwrap().id, cut.id, "cuts come last");
    }

    #[test]
    fn no_projects_gives_the_outside_line() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let files = vec![start(home.join(".claude").join("CLAUDE.md"), 20_000)];
        let loads = [load("claude", None, &home, files)];
        let got = items(&loads, &[]);
        assert_eq!(got[0].id, "instr.user");
        assert_eq!(
            got[0].title,
            "Outside a project: at start, Claude Code loads about 5.0k tokens of instructions."
        );
        assert_eq!(got[1].id, "instrsize.claude.user");
        assert_eq!(
            got[1].title,
            "Claude Code loads about 5.0k tokens of instructions at every start in every folder"
        );
        assert_eq!(got[1].detail, "The largest: ~/.claude/CLAUDE.md (5.0k).");
    }

    #[test]
    fn a_distro_line_says_user_instructions() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("h");
        let mut claude = load(
            "claude",
            None,
            &home,
            vec![start(home.join(".claude").join("CLAUDE.md"), 12)],
        );
        claude.distro = Some("Ubuntu".into());
        let mut codex = load("codex", None, &home, Vec::new());
        codex.distro = Some("Ubuntu".into());
        let got = items(&[claude, codex], &[]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "instr.user@Ubuntu");
        assert_eq!(got[0].distro.as_deref(), Some("Ubuntu"));
        assert_eq!(
            got[0].title,
            "In Ubuntu, at start: Claude Code loads about 3 tokens of user instructions, Codex none."
        );
        assert_eq!(got[0].detail, "Projects inside Ubuntu aren't checked.");
        assert_eq!(got[0].loads[0].files[0].path, "~/.claude/CLAUDE.md");
    }

    #[test]
    fn distro_waste_and_cut_ids_end_in_the_distro() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("h");
        let mut claude = load("claude", None, &home, vec![start(home.join("CLAUDE.md"), 20_000)]);
        claude.distro = Some("Ubuntu".into());
        claude.cuts.push(Cut {
            id_part: "abcd1234".into(),
            path: None,
            title: "t".into(),
            detail: "d".into(),
            cannot_fix: "c".into(),
        });
        let got = items(&[claude], &[]);
        assert_eq!(got[1].id, "instrsize.claude.user@Ubuntu");
        assert!(got[1].title.ends_with("in Ubuntu"));
        assert_eq!(got[2].id, "instrcut.claude.user.abcd1234@Ubuntu");
        assert_eq!(got[2].path, None);
    }

    #[test]
    fn notes_ride_under_the_heading() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let mut copilot = load("copilot", Some(&project), tmp.path(), Vec::new());
        copilot.notes = vec!["Copilot CLI documents no limit.".into()];
        let got = items(&[copilot], std::slice::from_ref(&project));
        assert_eq!(got[0].loads[0].notes, ["Copilot CLI documents no limit."]);
    }

    #[test]
    fn row_notes_use_the_fixed_phrases() {
        let tmp = tempfile::tempdir().unwrap();
        let project = project_in(tmp.path());
        let home = tmp.path().join("home");
        let mut import = start(project.join("sub").join("a.md"), 8);
        import.role = Role::Import {
            by: project.join("CLAUDE.md"),
        };
        import.depth = 1;
        let mut memory = start(home.join(".claude").join("MEMORY.md"), 4);
        memory.role = Role::Memory;
        memory.notes = vec!["first 200 lines".into()];
        let mut off = start(project.join("x.md"), 0);
        off.when = When::NotLoaded("not found".into());
        let mut managed = start(project.join("m.md"), 4);
        managed.role = Role::Managed;
        managed.notes = vec!["cut: the rest".into()];
        let loads = [load(
            "claude",
            Some(&project),
            &home,
            vec![import, memory, off, managed],
        )];
        let got = items(&loads, std::slice::from_ref(&project));
        let notes: Vec<&str> = got[0].loads[0].files.iter().map(|f| f.note.as_str()).collect();
        assert_eq!(
            notes,
            [
                "about 2 tokens, imported by CLAUDE.md",
                "about 1 tokens, auto memory, first 200 lines",
                "not loaded: not found",
                "about 1 tokens, cut: the rest, managed by your organisation",
            ]
        );
        assert_eq!(got[0].loads[0].files[2].when, DoctorLoadWhen::NotLoaded);
        assert_eq!(got[0].loads[0].files[0].depth, 1);
    }
}
