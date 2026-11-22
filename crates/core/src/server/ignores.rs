//! Git's ignore rules, read from a checkout's own files without running git.
//!
//! The rules for one path are, in git's order, the `.gitignore` files from the path's folder up to the
//! checkout (the deepest first), then `<common git dir>/info/exclude`, then the user's global excludes
//! file. The first that has a say decides, and nothing under an ignored folder comes back.
//!
//! Reading a repo's rules never reaches the network: a `.gitignore` that is a link is not read, a
//! `gitdir:` or `commondir` that names a network or device path (or one `links_refused` refuses) is not
//! followed, and a pattern file over 1 MiB is skipped.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use super::files::{links_refused, refused};
use crate::git::Stopped;

/// The largest pattern file that is read.
const MAX_PATTERN_BYTES: u64 = 1024 * 1024;
/// The most of a `.git` file or a `commondir` file that is read.
const MAX_POINTER_BYTES: u64 = 4096;

/// Git sets `core.ignorecase` when it makes a repo on these systems.
pub(super) const IGNORE_CASE: bool = cfg!(any(windows, target_os = "macos"));

/// The whole of a regular file, or None when it is missing, not a regular file or over `cap` bytes.
fn read_capped(path: &Path, cap: u64) -> Option<Vec<u8>> {
    // `metadata` follows links; a FIFO or device is never opened.
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > cap {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= cap).then_some(bytes)
}

/// A pattern file's text: its BOM dropped, bad UTF-8 replaced.
fn pattern_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
}

/// A matcher for `text`, whose patterns are relative to `root`.
fn build(root: &Path, from: &Path, text: &str) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    let _ = builder.case_insensitive(IGNORE_CASE);
    for line in text.lines() {
        // A line that is not a valid pattern is skipped, as git skips it.
        let _ = builder.add_line(Some(from.to_path_buf()), line);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// What a `.git` file says, as a folder: its `gitdir:` line, relative to `checkout` when it is relative.
fn gitdir_of_file(checkout: &Path, dot_git: &Path) -> Option<PathBuf> {
    let text = String::from_utf8_lossy(&read_capped(dot_git, MAX_POINTER_BYTES)?).into_owned();
    let line = text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
    followed_folder(checkout, line)
}

/// `named` (as written in a git file) as a folder to follow, or None when it names a network or device
/// path, passes a link `links_refused` refuses, or isn't a folder. A relative name starts in `base`.
fn followed_folder(base: &Path, named: &str) -> Option<PathBuf> {
    if named.is_empty() || refused(named) {
        return None;
    }
    let path = base.join(named);
    if links_refused(&path) || !path.is_dir() {
        return None;
    }
    Some(path)
}

/// The git dir of `checkout`: `<checkout>/.git` when it is a real folder, else the `gitdir:` its `.git`
/// file names (relative to the checkout when it is relative, as a submodule writes it). A `.git` that
/// is a link is no checkout.
pub(super) fn git_dir(checkout: &Path) -> Option<PathBuf> {
    let dot_git = checkout.join(".git");
    let meta = std::fs::symlink_metadata(&dot_git).ok()?;
    let kind = meta.file_type();
    if kind.is_symlink() {
        None
    } else if kind.is_dir() {
        Some(dot_git)
    } else if kind.is_file() {
        gitdir_of_file(checkout, &dot_git)
    } else {
        None
    }
}

/// The common git dir of `git_dir`: what its `commondir` file names, else itself.
fn common_dir(git_dir: &Path) -> PathBuf {
    let file = git_dir.join("commondir");
    let named = (!links_refused(&file))
        .then(|| read_capped(&file, MAX_POINTER_BYTES))
        .flatten();
    named
        .and_then(|bytes| {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            followed_folder(git_dir, text.lines().next().unwrap_or("").trim())
        })
        .unwrap_or_else(|| git_dir.to_path_buf())
}

/// A file under the git dir, read only when no link on its way is refused.
fn read_in_git_dir(path: &Path) -> Option<String> {
    if links_refused(path) {
        return None;
    }
    read_capped(path, MAX_PATTERN_BYTES).map(|b| pattern_text(&b))
}

/// What the global file held when it was last read.
struct GlobalCache {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
    text: Arc<String>,
}

static GLOBAL: Mutex<Option<GlobalCache>> = Mutex::new(None);

/// The global excludes file's text. The user's own file may be a local link (dotfile managers make
/// them), so it passes `links_refused` and is followed. Its text is kept for the process and read again
/// when its length or modified time changes.
fn global_text(path: &Path) -> Option<Arc<String>> {
    if links_refused(path) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_PATTERN_BYTES {
        return None;
    }
    let (len, modified) = (meta.len(), meta.modified().ok());
    {
        let cache = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = cache.as_ref()
            && c.path == path
            && c.len == len
            && c.modified == modified
        {
            return Some(c.text.clone());
        }
    }
    let text = Arc::new(pattern_text(&read_capped(path, MAX_PATTERN_BYTES)?));
    *GLOBAL.lock().unwrap_or_else(|e| e.into_inner()) = Some(GlobalCache {
        path: path.to_path_buf(),
        len,
        modified,
        text: text.clone(),
    });
    Some(text)
}

/// What is known about a folder strictly between `top` and a path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Folder {
    /// Ignored, so everything under it is.
    Ignored,
    /// Holds `.git` and isn't ignored: git doesn't judge a nested repo's files.
    Nested,
    Clear,
}

/// Git's ignore rules for one checkout, below `top`.
pub(super) struct Ignores {
    checkout: PathBuf,
    top: PathBuf,
    /// `<common git dir>/info/exclude`, rooted at the checkout.
    exclude: Option<Gitignore>,
    /// The global excludes file, rooted at the checkout.
    global: Option<Gitignore>,
    /// One matcher per folder that holds a `.gitignore`, or None where it holds none.
    folders: HashMap<PathBuf, Option<Arc<Gitignore>>>,
    below_top: HashMap<PathBuf, Folder>,
}

impl Ignores {
    /// The rules of `checkout`, for paths below `top`, with the user's global excludes file.
    pub(super) fn new(checkout: &Path, top: &Path) -> Ignores {
        let global = ignore::gitignore::gitconfig_excludes_path();
        Ignores::with_global(checkout, top, global.as_deref())
    }

    /// As `new`, with `global` as the global excludes file (tests pass None or a temp file).
    pub(super) fn with_global(checkout: &Path, top: &Path, global: Option<&Path>) -> Ignores {
        let exclude = git_dir(checkout).and_then(|dir| {
            let file = common_dir(&dir).join("info").join("exclude");
            read_in_git_dir(&file).map(|text| build(checkout, &file, &text))
        });
        let global = global.and_then(|path| global_text(path).map(|text| build(checkout, path, &text)));
        Ignores {
            checkout: checkout.to_path_buf(),
            top: top.to_path_buf(),
            exclude,
            global,
            folders: HashMap::new(),
            below_top: HashMap::new(),
        }
    }

    /// The matcher of `folder`'s `.gitignore`, built the first time it is asked for.
    fn folder_rules(&mut self, folder: &Path) -> Option<Arc<Gitignore>> {
        if let Some(found) = self.folders.get(folder) {
            return found.clone();
        }
        let file = folder.join(".gitignore");
        // A `.gitignore` that is a link is skipped, as git 2.32 and later skip it.
        let is_link = std::fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink());
        let rules = if is_link {
            None
        } else {
            read_capped(&file, MAX_PATTERN_BYTES).map(|b| Arc::new(build(folder, &file, &pattern_text(&b))))
        };
        self.folders.insert(folder.to_path_buf(), rules.clone());
        rules
    }

    /// Whether the rules ignore `path` itself, whatever its parents are: the `.gitignore` files from
    /// its folder up to the checkout, then `info/exclude`, then the global file. `path` is below the
    /// checkout.
    fn rules_ignore(&mut self, path: &Path, is_dir: bool) -> bool {
        let mut folder = path.parent();
        while let Some(dir) = folder {
            if let Some(rules) = self.folder_rules(dir)
                && let Some(rel) = relative(dir, path)
            {
                match rules.matched(&rel, is_dir) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
            if dir == self.checkout {
                break;
            }
            folder = dir.parent();
        }
        let Some(rel) = relative(&self.checkout, path) else {
            return false;
        };
        for rules in [&self.exclude, &self.global].into_iter().flatten() {
            match rules.matched(&rel, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }

    /// A folder between `top` and a path: ignored, a nested checkout or neither.
    fn folder_state(&mut self, folder: &Path) -> Folder {
        if let Some(state) = self.below_top.get(folder) {
            return *state;
        }
        let state = if self.rules_ignore(folder, true) {
            Folder::Ignored
        } else if std::fs::symlink_metadata(folder.join(".git")).is_ok() {
            Folder::Nested
        } else {
            Folder::Clear
        };
        self.below_top.insert(folder.to_path_buf(), state);
        state
    }

    /// Whether git ignores `abs`. A path that isn't below `top` (and the checkout) is never ignored.
    /// `top` and the folders above it aren't ignored themselves, though their `.gitignore` lines still
    /// apply below them. A folder link counts as a folder for a `dir/` pattern: pass `is_dir` true.
    pub(super) fn ignored(&mut self, abs: &Path, is_dir: bool) -> bool {
        if !abs.starts_with(&self.checkout) {
            return false;
        }
        let Ok(below) = abs.strip_prefix(&self.top) else {
            return false;
        };
        let top = self.top.clone();
        let names: Vec<_> = below.components().collect();
        let mut prefix = top;
        for (i, name) in names.iter().enumerate() {
            prefix.push(name);
            if i + 1 == names.len() {
                return self.rules_ignore(&prefix, is_dir);
            }
            match self.folder_state(&prefix) {
                Folder::Ignored => return true,
                Folder::Nested => return false,
                Folder::Clear => {}
            }
        }
        false
    }
}

/// `path` below `base`, `/`-separated, for a matcher rooted at `base`.
fn relative(base: &Path, path: &Path) -> Option<String> {
    let below = path.strip_prefix(base).ok()?;
    let parts: Vec<_> = below
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

struct Walk<'a> {
    ignores: &'a mut Ignores,
    deadline: Instant,
    cancel: &'a AtomicBool,
    skip: &'a dyn Fn(&str) -> bool,
    each: &'a mut dyn FnMut(&str, bool) -> bool,
}

impl Walk<'_> {
    /// Lists `folder`, whose entries' paths start with `prefix`. `Done` means go on.
    fn folder(&mut self, folder: &Path, prefix: &str, is_start: bool) -> Stopped {
        if let Some(stopped) = self.interrupted() {
            return stopped;
        }
        let Ok(read) = std::fs::read_dir(folder) else {
            return Stopped::Done;
        };
        let mut entries: Vec<(String, std::fs::FileType)> = read
            .flatten()
            .filter_map(|e| Some((e.file_name().into_string().ok()?, e.file_type().ok()?)))
            .collect();
        // A folder that holds `.git` below the start is a nested checkout: git doesn't judge it.
        if !is_start && entries.iter().any(|(name, _)| name == ".git") {
            return Stopped::Done;
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, kind) in entries {
            if let Some(stopped) = self.interrupted() {
                return stopped;
            }
            if name == ".git" {
                continue;
            }
            let abs = folder.join(&name);
            let rel = format!("{prefix}{name}");
            if kind.is_dir() {
                if self.ignores.ignored(&abs, true) {
                    continue;
                }
                let stopped = self.folder(&abs, &format!("{rel}/"), false);
                if stopped != Stopped::Done {
                    return stopped;
                }
            } else {
                let is_link = kind.is_symlink();
                // A link to a folder is never entered. A link `links_refused` refuses isn't followed
                // here; it goes on as a link, for the caller to refuse.
                if is_link && !links_refused(&abs) && std::fs::metadata(&abs).is_ok_and(|m| m.is_dir()) {
                    continue;
                }
                if self.ignores.ignored(&abs, false) || (self.skip)(&rel) {
                    continue;
                }
                if !(self.each)(&rel, is_link) {
                    return Stopped::Asked;
                }
            }
        }
        Stopped::Done
    }

    fn interrupted(&self) -> Option<Stopped> {
        if self.cancel.load(Ordering::Relaxed) {
            Some(Stopped::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(Stopped::Deadline)
        } else {
            None
        }
    }
}

/// Walks `start` top-down, in sorted order, and calls `each(rel, is_link)` for every file git would
/// list: `rel` is relative to `start` and `/`-separated. A folder link, `.git`, a nested checkout,
/// anything ignored and any file `skip` names is left out; hidden files are kept. A file link goes to
/// `each` as a link. `each` returning false stops the walk with `Asked`.
pub(super) fn walk_unignored(
    ig: &mut Ignores,
    start: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
    skip: &dyn Fn(&str) -> bool,
    each: &mut dyn FnMut(&str, bool) -> bool,
) -> Stopped {
    Walk {
        ignores: ig,
        deadline,
        cancel,
        skip,
        each,
    }
    .folder(start, "", true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(tmp.path());
        (tmp, root)
    }

    /// A repo root: an empty `.git` folder, so no git is run.
    fn repo() -> (tempfile::TempDir, PathBuf) {
        let (tmp, root) = temp();
        std::fs::create_dir(root.join(".git")).unwrap();
        (tmp, root)
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn rules(root: &Path) -> Ignores {
        Ignores::with_global(root, root, None)
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(60)
    }

    /// Makes a symlink; false where the OS refuses, so the test can skip itself.
    fn try_symlink(target: &Path, link: &Path, folder: bool) -> bool {
        #[cfg(windows)]
        let made = if folder {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        };
        #[cfg(unix)]
        let made = {
            let _ = folder;
            std::os::unix::fs::symlink(target, link)
        };
        made.is_ok()
    }

    /// A link to a folder: a junction on Windows, a symlink elsewhere. False where it can't be made.
    fn link_folder(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success())
        }
        #[cfg(not(windows))]
        {
            try_symlink(target, link, true)
        }
    }

    fn listed(ig: &mut Ignores, start: &Path) -> Vec<String> {
        let mut seen = Vec::new();
        let stopped = walk_unignored(ig, start, far(), &AtomicBool::new(false), &|_| false, &mut |rel, _| {
            seen.push(rel.to_string());
            true
        });
        assert_eq!(stopped, Stopped::Done);
        seen
    }

    #[test]
    fn a_pattern_in_the_root_gitignore_ignores_a_file_and_a_folder() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "*.log\ndist/\n");
        let mut ig = rules(&root);
        assert!(ig.ignored(&root.join("a.log"), false));
        assert!(ig.ignored(&root.join("dist"), true));
        assert!(!ig.ignored(&root.join("a.txt"), false));
    }

    #[test]
    fn a_negation_keeps_a_file_the_pattern_above_it_ignores() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "*.log\n!keep.log\n");
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("keep.log"), false));
        assert!(ig.ignored(&root.join("a.log"), false));
    }

    #[test]
    fn a_deeper_gitignore_overrides_the_root() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "*.tmp\n");
        write(&root.join("sub/.gitignore"), "!a.tmp\n");
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("sub/a.tmp"), false));
        assert!(ig.ignored(&root.join("sub/b.tmp"), false));
    }

    #[test]
    fn nothing_under_an_ignored_folder_comes_back() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "dist/\n!dist/keep.txt\n");
        let mut ig = rules(&root);
        assert!(ig.ignored(&root.join("dist/keep.txt"), false));
    }

    #[test]
    fn a_folder_only_pattern_leaves_a_file_of_that_name() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "build/\n");
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("build"), false));
        assert!(ig.ignored(&root.join("build"), true));
    }

    #[test]
    fn info_exclude_counts_after_every_gitignore() {
        let (_tmp, root) = repo();
        write(&root.join(".git/info/exclude"), "*.x\n");
        write(&root.join("sub/.gitignore"), "!a.x\n");
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("sub/a.x"), false));
        assert!(ig.ignored(&root.join("sub/b.x"), false));
    }

    #[test]
    fn the_global_file_counts_last_and_anchors_at_the_checkout() {
        let (_tmp, root) = repo();
        let (_gtmp, groot) = temp();
        let global = groot.join("ignore");
        write(&global, "*.g\n/top.h\n");
        write(&root.join(".gitignore"), "!a.g\n");
        let mut ig = Ignores::with_global(&root, &root, Some(&global));
        assert!(ig.ignored(&root.join("b.g"), false));
        assert!(ig.ignored(&root.join("top.h"), false));
        assert!(!ig.ignored(&root.join("sub/top.h"), false));
        assert!(!ig.ignored(&root.join("a.g"), false));
    }

    #[test]
    fn a_worktree_reads_info_exclude_from_the_common_git_dir() {
        let (_tmp, base) = temp();
        let main_git = base.join("main/.git");
        let wt_git = main_git.join("worktrees/w");
        write(&wt_git.join("commondir"), "../..\n");
        write(&main_git.join("info/exclude"), "*.x\n");
        let checkout = base.join("w");
        write(&checkout.join(".git"), &format!("gitdir: {}\n", wt_git.display()));
        assert_eq!(git_dir(&checkout), Some(wt_git));
        let mut ig = rules(&checkout);
        assert!(ig.ignored(&checkout.join("a.x"), false));
        assert!(!ig.ignored(&checkout.join("a.y"), false));
    }

    #[test]
    fn a_submodules_relative_gitdir_is_read() {
        let (_tmp, root) = repo();
        let modules = root.join(".git/modules/sub");
        write(&modules.join("info/exclude"), "*.x\n");
        let sub = root.join("sub");
        write(&sub.join(".git"), "gitdir: ../.git/modules/sub\n");
        let mut ig = rules(&sub);
        assert!(ig.ignored(&sub.join("a.x"), false));
        assert!(!ig.ignored(&sub.join("a.y"), false));
    }

    #[test]
    fn a_path_outside_the_checkout_is_not_ignored() {
        let (_tmp, root) = repo();
        let (_other, elsewhere) = temp();
        write(&root.join(".gitignore"), "*\n");
        let mut ig = rules(&root);
        assert!(!ig.ignored(&elsewhere.join("a.log"), false));
        assert!(!ig.ignored(root.parent().unwrap(), true));
    }

    #[test]
    fn files_in_a_nested_checkout_are_not_judged() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "*.log\n");
        std::fs::create_dir_all(root.join("sub/.git")).unwrap();
        write(&root.join("sub/a.log"), "");
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("sub/a.log"), false));
        assert!(ig.ignored(&root.join("a.log"), false));
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn patterns_match_without_case_where_git_does() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "thumbs.db\nbin/\n");
        let mut ig = rules(&root);
        assert!(ig.ignored(&root.join("Thumbs.db"), false));
        assert!(ig.ignored(&root.join("Bin"), true));
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_link_counts_as_a_folder_for_a_folder_pattern() {
        let (_tmp, root) = repo();
        std::fs::create_dir(root.join("real")).unwrap();
        if !link_folder(&root.join("real"), &root.join("node_modules")) {
            return;
        }
        write(&root.join(".gitignore"), "node_modules/\n");
        let mut ig = rules(&root);
        assert!(ig.ignored(&root.join("node_modules"), true));
        assert!(!ig.ignored(&root.join("real"), true));
    }

    #[test]
    fn a_bom_at_the_start_of_a_gitignore_is_dropped() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "\u{feff}*.log\n");
        let mut ig = rules(&root);
        assert!(ig.ignored(&root.join("a.log"), false));
    }

    #[test]
    fn a_gitignore_that_is_a_link_is_not_read() {
        let (_tmp, root) = repo();
        write(&root.join("target.txt"), "*\n");
        write(&root.join("sub/a.txt"), "");
        if !try_symlink(&root.join("target.txt"), &root.join("sub/.gitignore"), false) {
            return;
        }
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("sub/a.txt"), false));
    }

    #[test]
    fn a_dot_git_that_is_a_link_is_not_a_checkout() {
        let (_tmp, base) = temp();
        let real = base.join("real-git");
        write(&real.join("info/exclude"), "*.x\n");
        let checkout = base.join("co");
        std::fs::create_dir_all(&checkout).unwrap();
        if !link_folder(&real, &checkout.join(".git")) {
            return;
        }
        assert_eq!(git_dir(&checkout), None);
        let mut ig = rules(&checkout);
        assert!(!ig.ignored(&checkout.join("a.x"), false));
    }

    #[test]
    fn a_gitdir_naming_a_network_path_is_not_followed() {
        let (_tmp, root) = temp();
        write(&root.join(".git"), "gitdir: //server/share/x\n");
        assert_eq!(git_dir(&root), None);
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("a.log"), false));
    }

    #[test]
    fn a_pattern_file_over_1_mib_is_skipped() {
        let (_tmp, root) = repo();
        let mut text = String::from("*.log\n");
        text.push_str(&"#".repeat(1024 * 1024 + 1 - text.len()));
        assert_eq!(text.len() as u64, MAX_PATTERN_BYTES + 1);
        write(&root.join(".gitignore"), &text);
        let mut ig = rules(&root);
        assert!(!ig.ignored(&root.join("a.log"), false));
    }

    #[test]
    fn the_walk_lists_what_git_would_list_and_dotfiles() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "*.log\ndist/\n");
        write(&root.join(".github/ci.yml"), "");
        write(&root.join(".env"), "");
        write(&root.join("src/a.rs"), "");
        write(&root.join("a.log"), "");
        write(&root.join("dist/x"), "");
        let mut ig = rules(&root);
        let got = listed(&mut ig, &root);
        assert_eq!(got, [".env", ".github/ci.yml", ".gitignore", "src/a.rs"]);
    }

    #[test]
    fn the_walk_never_enters_a_link_dot_git_or_a_nested_checkout() {
        let (_tmp, root) = repo();
        write(&root.join("real/f.txt"), "");
        std::fs::create_dir_all(root.join("sub/.git")).unwrap();
        write(&root.join("sub/g.txt"), "");
        if !link_folder(&root.join("real"), &root.join("linked")) {
            return;
        }
        let mut ig = rules(&root);
        let got = listed(&mut ig, &root);
        assert_eq!(got, ["real/f.txt"]);
    }

    #[test]
    fn the_walk_hands_a_file_link_over_as_a_link() {
        let (_tmp, root) = repo();
        write(&root.join("a.txt"), "");
        if !try_symlink(&root.join("a.txt"), &root.join("b.lnk"), false) {
            return;
        }
        let mut ig = rules(&root);
        let mut seen = Vec::new();
        let stopped = walk_unignored(
            &mut ig,
            &root,
            far(),
            &AtomicBool::new(false),
            &|_| false,
            &mut |rel, link| {
                seen.push((rel.to_string(), link));
                true
            },
        );
        assert_eq!(stopped, Stopped::Done);
        assert_eq!(seen, [("a.txt".to_string(), false), ("b.lnk".to_string(), true)]);
    }

    #[test]
    fn the_walk_stops_when_asked_at_the_deadline_and_when_cancelled() {
        let (_tmp, root) = repo();
        for name in ["a", "b", "c", "d", "e"] {
            write(&root.join(name), "");
        }
        let mut ig = rules(&root);
        let mut count = 0;
        let stopped = walk_unignored(
            &mut ig,
            &root,
            far(),
            &AtomicBool::new(false),
            &|_| false,
            &mut |_, _| {
                count += 1;
                count < 3
            },
        );
        assert_eq!((stopped, count), (Stopped::Asked, 3));

        let past = Instant::now() - Duration::from_secs(1);
        let stopped = walk_unignored(
            &mut ig,
            &root,
            past,
            &AtomicBool::new(false),
            &|_| false,
            &mut |_, _| true,
        );
        assert_eq!(stopped, Stopped::Deadline);

        let stopped = walk_unignored(
            &mut ig,
            &root,
            far(),
            &AtomicBool::new(true),
            &|_| false,
            &mut |_, _| true,
        );
        assert_eq!(stopped, Stopped::Cancelled);
    }

    #[test]
    fn the_walk_leaves_out_what_skip_names() {
        let (_tmp, root) = repo();
        write(&root.join("a.txt"), "");
        write(&root.join("b.txt"), "");
        let mut ig = rules(&root);
        let mut seen = Vec::new();
        let stopped = walk_unignored(
            &mut ig,
            &root,
            far(),
            &AtomicBool::new(false),
            &|p| p == "a.txt",
            &mut |rel, _| {
                seen.push(rel.to_string());
                true
            },
        );
        assert_eq!(stopped, Stopped::Done);
        assert_eq!(seen, ["b.txt"]);
    }

    #[test]
    fn the_walk_from_a_sub_folder_applies_the_folders_above_it() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "*.log\n");
        write(&root.join("sub/a.log"), "");
        write(&root.join("sub/b.txt"), "");
        let sub = root.join("sub");
        let mut ig = Ignores::with_global(&root, &sub, None);
        assert_eq!(listed(&mut ig, &sub), ["b.txt"]);
    }

    #[test]
    fn a_walk_started_inside_an_ignored_folder_still_lists_it() {
        let (_tmp, root) = repo();
        write(&root.join(".gitignore"), "node_modules/\n*.log\n");
        write(&root.join("node_modules/pkg/a.js"), "");
        write(&root.join("node_modules/pkg/b.log"), "");
        let pkg = root.join("node_modules/pkg");
        let mut ig = Ignores::with_global(&root, &pkg, None);
        assert_eq!(listed(&mut ig, &pkg), ["a.js"]);
    }
}
