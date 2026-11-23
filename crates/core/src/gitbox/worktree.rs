//! What git, comparing the work tree, could reach beyond the checkout.
//!
//! Git looks at every tracked path, which crosses each folder on its way, and a folder it finds at a
//! tracked path (a submodule's, or one where a tracked file was) it opens as a repository: its `.git`, the
//! git folder a `.git` file names, and what that folder's `commondir` and `HEAD` lead to. It does so on
//! every work-tree diff, `--ignore-submodules=all` or not. So before git is given the work tree, none of
//! those may be a network or device path, or a link `links_refused` refuses.

use super::{ALTERNATES_BUDGET, alternates_refused};
use crate::server::{MAX_POINTER_BYTES, links_refused, read_capped, refused};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::FileType;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// The most symbolic refs followed from a repository's `HEAD`, as many as git follows.
const MAX_SYMREFS: usize = 5;
/// The most tables a `reftable` folder may hold; one with more is refused unread.
const MAX_TABLES: usize = 256;

/// The index's paths, read once per change of the index.
#[derive(Debug, Default)]
pub struct Tracked {
    pub paths: HashSet<String>,
    /// Each folder on the way to a tracked path (`""` for the checkout), parents before children, with the
    /// names in it the index holds.
    folders: Vec<(String, Vec<Name>)>,
}

/// A name in a folder of the index: a tracked path, a folder on the way to one, or (in an odd index) both.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Name {
    name: String,
    tracked: bool,
    folder: bool,
}

/// How the look at the work tree ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Reach {
    Clear,
    Refused,
    /// The deadline came, or the caller cancelled.
    Late,
}

impl Tracked {
    /// The tracked paths from `git ls-files --cached -z`. A path that isn't UTF-8 is read byte for byte as
    /// Latin-1, as Git for Windows turns it into a file name.
    pub fn parse(bytes: &[u8]) -> Tracked {
        let mut paths = HashSet::new();
        // folder → name → (tracked, a folder on the way)
        let mut folders: BTreeMap<String, BTreeMap<String, (bool, bool)>> = BTreeMap::new();
        for raw in bytes.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            let path = as_file_name(raw);
            let mut folder = String::new();
            let mut parts = path.split('/').peekable();
            while let Some(part) = parts.next() {
                let slot = folders
                    .entry(folder.clone())
                    .or_default()
                    .entry(part.to_string())
                    .or_default();
                if parts.peek().is_none() {
                    slot.0 = true;
                } else {
                    slot.1 = true;
                }
                if !folder.is_empty() {
                    folder.push('/');
                }
                folder.push_str(part);
            }
            paths.insert(path);
        }
        let folders = folders
            .into_iter()
            .map(|(folder, names)| {
                let names = names
                    .into_iter()
                    .map(|(name, (tracked, folder))| Name { name, tracked, folder })
                    .collect();
                (folder, names)
            })
            .collect();
        Tracked { paths, folders }
    }
}

/// `bytes` as text: UTF-8 where it is, each other byte as the Latin-1 character it is.
fn as_file_name(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());
        text.extend(chunk.invalid().iter().map(|b| char::from(*b)));
    }
    text
}

/// Whether git, comparing the work tree at `checkout` with `tracked`, would reach a refused path. The
/// folders are looked at parents first, so no folder is read through a link before the link is checked.
pub fn reach(checkout: &Path, tracked: &Tracked, deadline: Instant, cancel: &AtomicBool) -> Reach {
    for (folder, names) in &tracked.folders {
        if Instant::now() >= deadline || cancel.load(Ordering::Relaxed) {
            return Reach::Late;
        }
        let dir = if folder.is_empty() {
            checkout.to_path_buf()
        } else {
            checkout.join(folder)
        };
        // The listing gives each entry's kind with no call per file. A name it doesn't hold as written (a
        // deleted file, another case, a short name) is looked up on its own, the way git looks it up.
        let listing: HashMap<String, FileType> = match std::fs::read_dir(&dir) {
            Ok(entries) => entries
                .flatten()
                .filter_map(|e| Some((e.file_name().into_string().ok()?, e.file_type().ok()?)))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => HashMap::new(),
        };
        for n in names {
            let path = dir.join(&n.name);
            let kind = match listing.get(&n.name) {
                Some(kind) => Some(*kind),
                None => std::fs::symlink_metadata(&path).ok().map(|m| m.file_type()),
            };
            let Some(kind) = kind else { continue };
            if kind.is_symlink() && links_refused(&path) {
                return Reach::Refused;
            }
            if n.tracked && (kind.is_dir() || kind.is_symlink()) && repository_refused(&path) {
                return Reach::Refused;
            }
        }
    }
    Reach::Clear
}

/// A small file git reads whole (a `.git` file, `commondir`, a ref), or None when it is over the cap or
/// can't be read.
fn pointer(file: &Path) -> Option<String> {
    read_capped(file, MAX_POINTER_BYTES).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `named`, as a git file writes it (relative to `base` when relative), or None when `refused` or
/// `links_refused` refuses it.
fn followed(base: &Path, named: &str) -> Option<PathBuf> {
    if refused(named) {
        return None;
    }
    let path = base.join(named);
    (!links_refused(&path)).then_some(path)
}

/// Whether git, opening `folder` as a repository, would reach a refused path: its `.git`, the git folder a
/// `.git` file names, and what `git_folder_refused` follows from there.
fn repository_refused(folder: &Path) -> bool {
    let dot = folder.join(".git");
    if links_refused(&dot) {
        return true;
    }
    let Ok(meta) = std::fs::metadata(&dot) else {
        return false;
    };
    if meta.is_dir() {
        return git_folder_refused(&dot);
    }
    if !meta.is_file() {
        return false;
    }
    let Some(text) = pointer(&dot) else {
        return true;
    };
    // Git takes what follows `gitdir: ` to the end of the file, less the line breaks at its end.
    let Some(rest) = text.strip_prefix("gitdir:") else {
        return false;
    };
    match followed(folder, rest.trim_start_matches(' ').trim_end_matches(['\r', '\n'])) {
        Some(git) => git_folder_refused(&git),
        None => true,
    }
}

/// Whether git, reading the repository whose git folder is `git` to find its `HEAD`, would reach a refused
/// path: `HEAD` and `commondir`, the common folder's `config`, `objects` (with their alternates), `refs`,
/// `packed-refs` and `reftable`, and each ref `HEAD` leads to.
fn git_folder_refused(git: &Path) -> bool {
    let commondir = git.join("commondir");
    if links_refused(&commondir) {
        return true;
    }
    let common = if commondir.is_file() {
        let Some(text) = pointer(&commondir) else {
            return true;
        };
        match followed(git, text.trim_end_matches(['\r', '\n'])) {
            Some(common) => common,
            None => return true,
        }
    } else {
        git.to_path_buf()
    };
    let paths = [
        git.join("HEAD"),
        git.join("config.worktree"),
        common.join("config"),
        common.join("objects"),
        common.join("refs"),
        common.join("packed-refs"),
        common.join("reftable"),
    ];
    if paths.iter().any(|p| links_refused(p))
        || alternates_refused(&common.join("objects"), 0, &mut ALTERNATES_BUDGET.clone())
    {
        return true;
    }
    if let Ok(tables) = std::fs::read_dir(common.join("reftable")) {
        for (n, table) in tables.flatten().enumerate() {
            if n >= MAX_TABLES || links_refused(&table.path()) {
                return true;
            }
        }
    }
    head_refused(git, &common)
}

/// Whether following `HEAD` through its symbolic refs reaches a refused path. A ref is looked for in the
/// git folder and in the common one, as git looks for a worktree's own refs and the shared ones.
fn head_refused(git: &Path, common: &Path) -> bool {
    let mut name = "HEAD".to_string();
    for _ in 0..=MAX_SYMREFS {
        let here = [git.join(&name), common.join(&name)];
        if here.iter().any(|p| links_refused(p)) {
            return true;
        }
        let Some(file) = here.into_iter().find(|p| p.is_file()) else {
            return false;
        };
        let Some(text) = pointer(&file) else {
            return true;
        };
        let Some(next) = text.strip_prefix("ref:") else {
            return false;
        };
        let next = next.trim();
        if refused(next) {
            return true;
        }
        name = next.to_string();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    /// A network path no test ever opens: `refused` reads only the text.
    const NETWORK: &str = "//192.0.2.1/share/x";

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(tmp.path());
        (tmp, root)
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn tracked(paths: &[&str]) -> Tracked {
        let mut bytes = Vec::new();
        for p in paths {
            bytes.extend_from_slice(p.as_bytes());
            bytes.push(0);
        }
        Tracked::parse(&bytes)
    }

    fn look(root: &Path, paths: &[&str]) -> Reach {
        reach(
            root,
            &tracked(paths),
            Instant::now() + Duration::from_secs(30),
            &AtomicBool::new(false),
        )
    }

    /// A git folder git would take for one: `HEAD`, `objects/` and `refs/`.
    fn git_folder(dir: &Path) {
        write(&dir.join("HEAD"), "ref: refs/heads/main\n");
        std::fs::create_dir_all(dir.join("objects")).unwrap();
        std::fs::create_dir_all(dir.join("refs").join("heads")).unwrap();
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

    #[test]
    fn parse_lists_the_folders_on_the_way_parents_first() {
        let t = tracked(&["b.txt", "src/a/x.rs", "src/b.rs", "src/a/y.rs"]);
        assert_eq!(t.paths.len(), 4);
        let folders: Vec<&str> = t.folders.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(folders, ["", "src", "src/a"]);
        let root: Vec<(&str, bool, bool)> = t.folders[0]
            .1
            .iter()
            .map(|n| (n.name.as_str(), n.tracked, n.folder))
            .collect();
        assert_eq!(root, [("b.txt", true, false), ("src", false, true)]);
        let src: Vec<(&str, bool, bool)> = t.folders[1]
            .1
            .iter()
            .map(|n| (n.name.as_str(), n.tracked, n.folder))
            .collect();
        assert_eq!(src, [("a", false, true), ("b.rs", true, false)]);
    }

    #[test]
    fn a_name_that_is_tracked_and_a_folder_is_both() {
        let t = tracked(&["a", "a/b"]);
        assert_eq!(
            t.folders[0].1,
            [Name {
                name: "a".into(),
                tracked: true,
                folder: true
            }]
        );
    }

    #[test]
    fn a_plain_checkout_is_clear() {
        let (_tmp, root) = temp();
        write(&root.join("a.txt"), "a\n");
        write(&root.join("src").join("b.rs"), "b\n");
        assert_eq!(
            look(&root, &["a.txt", "src/b.rs", "gone.txt", "missing/c.rs"]),
            Reach::Clear
        );
    }

    #[test]
    fn a_submodule_whose_git_file_names_a_network_folder_is_refused() {
        let (_tmp, root) = temp();
        write(
            &root.join("vendor").join("lib").join(".git"),
            &format!("gitdir: {NETWORK}\n"),
        );
        assert_eq!(look(&root, &["a.txt", "vendor/lib"]), Reach::Refused);
    }

    #[test]
    fn a_tracked_file_that_is_now_a_folder_is_looked_into() {
        let (_tmp, root) = temp();
        write(&root.join("docs").join(".git"), &format!("gitdir: {NETWORK}\n"));
        assert_eq!(look(&root, &["docs"]), Reach::Refused);
    }

    #[test]
    fn a_long_git_file_is_refused_unread() {
        let (_tmp, root) = temp();
        let padding = "\n".repeat(8 * 1024);
        write(&root.join("sub").join(".git"), &format!("gitdir: {NETWORK}{padding}"));
        assert_eq!(look(&root, &["sub"]), Reach::Refused);
    }

    #[test]
    fn a_submodule_with_its_git_folder_in_the_repository_is_clear() {
        let (_tmp, root) = temp();
        git_folder(&root.join(".git").join("modules").join("sub"));
        write(&root.join("sub").join(".git"), "gitdir: ../.git/modules/sub\n");
        write(&root.join("sub").join("s.txt"), "s\n");
        git_folder(&root.join("nested").join(".git"));
        assert_eq!(look(&root, &["sub", "nested"]), Reach::Clear);
    }

    #[test]
    fn a_commondir_on_the_network_is_refused() {
        let (_tmp, root) = temp();
        let dot = root.join("sub").join(".git");
        git_folder(&dot);
        write(&dot.join("commondir"), &format!("{NETWORK}\n"));
        assert_eq!(look(&root, &["sub"]), Reach::Refused);
    }

    #[test]
    fn alternates_of_a_submodule_on_the_network_are_refused() {
        let (_tmp, root) = temp();
        let dot = root.join("sub").join(".git");
        git_folder(&dot);
        write(
            &dot.join("objects").join("info").join("alternates"),
            &format!("{NETWORK}\n"),
        );
        assert_eq!(look(&root, &["sub"]), Reach::Refused);
    }

    #[cfg(windows)]
    #[test]
    fn a_name_the_disk_spells_otherwise_is_still_looked_at() {
        let (_tmp, root) = temp();
        write(&root.join("sub").join(".git"), &format!("gitdir: {NETWORK}\n"));
        assert_eq!(look(&root, &["SUB"]), Reach::Refused);
    }

    #[test]
    fn a_folder_on_the_way_that_links_to_the_network_is_refused() {
        let (_tmp, root) = temp();
        if !try_symlink(Path::new(r"\\192.0.2.1\share"), &root.join("src"), true) {
            eprintln!("skipped: no symlinks here");
            return;
        }
        assert_eq!(look(&root, &["src/a.rs"]), Reach::Refused);
    }

    #[test]
    fn a_head_that_names_a_ref_through_a_network_link_is_refused() {
        let (_tmp, root) = temp();
        let dot = root.join("sub").join(".git");
        git_folder(&dot);
        if !try_symlink(
            Path::new(r"\\192.0.2.1\share\main"),
            &dot.join("refs").join("heads").join("main"),
            false,
        ) {
            eprintln!("skipped: no symlinks here");
            return;
        }
        assert_eq!(look(&root, &["sub"]), Reach::Refused);
    }

    #[test]
    fn a_passed_deadline_is_late() {
        let (_tmp, root) = temp();
        write(&root.join("a.txt"), "a\n");
        let r = reach(
            &root,
            &tracked(&["a.txt"]),
            Instant::now() - Duration::from_secs(1),
            &AtomicBool::new(false),
        );
        assert_eq!(r, Reach::Late);
    }
}
