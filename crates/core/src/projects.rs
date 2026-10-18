//! Recent project folders, kept in `~/.marshell/projects.json` for the launcher.
//! The branch is read from the folder's git files when the list is asked for, not stored.

use crate::store::{self, Versioned};
use marshell_protocol::api::ProjectInfo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The most projects kept; the oldest fall off.
const MAX_PROJECTS: usize = 30;

#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    last_used_ms: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct FileDoc {
    projects: Vec<Entry>,
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

fn file(home: &Path) -> PathBuf {
    home.join("projects.json")
}

/// Drops trailing `/` and `\`, but keeps a root: `/`, `C:\` and `C:/` stay as they are.
fn trim_separators(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.len() == path.len() {
        return path;
    }
    // All separators (a root), or just a drive letter ("C:"): keep one separator.
    if trimmed.is_empty() || (trimmed.len() == 2 && trimmed.ends_with(':')) {
        return &path[..trimmed.len() + 1];
    }
    trimmed
}

/// Whether `path` is a drive or filesystem root: `/`, `C:\`, `C:/` or a UNC share root.
pub fn is_root(path: &Path) -> bool {
    Path::new(trim_separators(&path.to_string_lossy())).parent().is_none()
}

/// Windows paths ignore case, so `C:\Dev\App` and `c:\dev\app` are one project there.
/// A trailing separator does not make a different folder.
pub(crate) fn same_path(a: &str, b: &str) -> bool {
    let (a, b) = (trim_separators(a), trim_separators(b));
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Whether `path` is `folder` or inside it. Both are tidied and resolved first, and compared the
/// way `same_path` does, so a path that has gone from the disk is still placed by its text.
pub(crate) fn is_inside(path: &str, folder: &str) -> bool {
    let path = canonical(Path::new(path)).to_string_lossy().into_owned();
    let folder = canonical(Path::new(folder)).to_string_lossy().into_owned();
    let (path, folder) = (trim_separators(&path), trim_separators(&folder));
    if same_path(path, folder) {
        return true;
    }
    // The next character after the folder's text must be a separator, so `task-2` is not in `task`.
    let (path, folder) = if cfg!(windows) {
        (path.to_ascii_lowercase(), folder.to_ascii_lowercase())
    } else {
        (path.to_string(), folder.to_string())
    };
    path.strip_prefix(folder.trim_end_matches(['/', '\\']))
        .is_some_and(|rest| rest.starts_with(['/', '\\']))
}

/// `record` reads, changes and writes the file; two at once would lose an entry or
/// collide on the temp file. Held for that whole read-modify-write.
static RECORD_LOCK: Mutex<()> = Mutex::new(());

/// The folder with the `.git` entry that holds `dir`, and the git folder that entry stands for.
/// A linked worktree or submodule has a `.git` file that points at its git folder.
fn locate_git(dir: &Path) -> Option<(&Path, PathBuf)> {
    let repo = dir.ancestors().find(|d| d.join(".git").exists())?;
    let dot_git = repo.join(".git");
    let git_dir = if dot_git.is_file() {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let target = text.trim().strip_prefix("gitdir:")?.trim();
        repo.join(target)
    } else {
        dot_git
    };
    Some((repo, git_dir))
}

/// What `HEAD` says: the branch it points at, or the commit when detached.
enum Head {
    Branch(String),
    Detached(String),
}

fn read_head(dir: &Path) -> Option<Head> {
    let (_, git_dir) = locate_git(dir)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            Some(Head::Branch(
                reference.strip_prefix("refs/heads/").unwrap_or(reference).to_string(),
            ))
        }
        None => (!head.is_empty()).then(|| Head::Detached(head.chars().take(7).collect())),
    }
}

/// The current branch of the git repository that holds `dir`, if any.
/// A detached HEAD gives the first 7 characters of the commit.
pub fn git_branch(dir: &Path) -> Option<String> {
    match read_head(dir)? {
        Head::Branch(name) | Head::Detached(name) => Some(name),
    }
}

/// Like `git_branch`, but `None` when HEAD is detached (or there is no repository).
pub fn head_branch(dir: &Path) -> Option<String> {
    match read_head(dir)? {
        Head::Branch(name) => Some(name),
        Head::Detached(_) => None,
    }
}

/// Resolves `.` and `..` in the path text alone, without touching the disk (no `\\?\` paths on Windows).
fn tidy(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The path as the disk spells it: short Windows names (`HOMEOF~1`) and case are resolved, so two
/// spellings of one folder compare equal. A path that does not exist is only tidied.
pub fn canonical(path: &Path) -> PathBuf {
    let tidy = tidy(path);
    let Ok(real) = std::fs::canonicalize(&tidy) else {
        return tidy;
    };
    // Windows gives `\\?\C:\dir`; the plain `C:\dir` is what everything else uses. A UNC share comes
    // as `\\?\UNC\server\share\dir` and goes back to `\\server\share\dir`.
    let text = real.to_string_lossy();
    let Some(rest) = text.strip_prefix(r"\\?\") else {
        return real;
    };
    if let Some(unc) = rest.strip_prefix(r"UNC\") {
        return PathBuf::from(format!(r"\\{unc}"));
    }
    if rest.as_bytes().get(1) == Some(&b':') {
        return PathBuf::from(rest);
    }
    real
}

/// The root of the **main** checkout of the repository that holds `dir`: for a linked worktree, not the
/// worktree itself. Reads files only, and never runs git. `None` outside a repository.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    let (checkout, git_dir) = locate_git(dir)?;
    // A linked worktree's git folder has a `commondir` file naming the main `.git` folder. A plain
    // checkout and a submodule have none: the checkout is its own root.
    if let Ok(text) = std::fs::read_to_string(git_dir.join("commondir")) {
        let common = tidy(&git_dir.join(text.trim()));
        if let Some(root) = common.parent() {
            return Some(canonical(root));
        }
    }
    Some(canonical(checkout))
}

/// The key sessions are grouped under: the repository root of `cwd`, or `cwd` itself outside git.
pub fn project_of(cwd: &str) -> String {
    if cwd.is_empty() {
        return String::new();
    }
    repo_root(Path::new(cwd)).map_or_else(|| cwd.to_string(), |root| root.to_string_lossy().into_owned())
}

/// Puts `cwd` at the top of the recent list. The user's home folder and a drive root are never recorded.
/// Refuses when the file cannot be read (broken, or written by a newer Marshell),
/// so a file the user may want to repair is never overwritten.
pub fn record(home: &Path, cwd: &Path, now_ms: u64) -> anyhow::Result<()> {
    // Root-ness is judged on the resolved path: `C:\x\..` and a link to a drive root are roots too.
    if is_root(&canonical(cwd))
        || dirs::home_dir().is_some_and(|user_home| same_path(&user_home.to_string_lossy(), &cwd.to_string_lossy()))
    {
        return Ok(());
    }
    // A task worktree is not a project of its own: its repository is.
    let in_task = cwd.starts_with(home.join("worktrees"));
    let root = in_task.then(|| repo_root(cwd)).flatten();
    let cwd = root.as_deref().unwrap_or(cwd);
    let _guard = RECORD_LOCK.lock().unwrap();
    let path = file(home);
    let mut doc = store::read::<FileDoc>(&path)?.unwrap_or_default();
    let cwd = trim_separators(&cwd.to_string_lossy()).to_string();
    doc.projects.retain(|p| !same_path(&p.path, &cwd));
    doc.projects.insert(
        0,
        Entry {
            path: cwd,
            last_used_ms: now_ms,
        },
    );
    doc.projects.truncate(MAX_PROJECTS);
    store::write(&path, &doc)
}

/// Recent projects, newest first. Folders that no longer exist are left out.
/// A missing, broken or newer-version file gives an empty list.
pub fn list(home: &Path) -> Vec<ProjectInfo> {
    let doc = store::read::<FileDoc>(&file(home)).unwrap_or_else(|err| {
        tracing::warn!("ignoring projects file: {err:#}");
        None
    });
    doc.unwrap_or_default()
        .projects
        .into_iter()
        .filter(|p| Path::new(&p.path).exists())
        .map(|p| {
            let dir = Path::new(&p.path);
            ProjectInfo {
                name: dir
                    .file_name()
                    .map_or_else(|| p.path.clone(), |n| n.to_string_lossy().into_owned()),
                branch: git_branch(dir),
                last_used_ms: p.last_used_ms,
                path: p.path,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn is_inside_means_the_folder_or_below_it() {
        assert!(is_inside("/w/task", "/w/task"));
        assert!(is_inside("/w/task/", "/w/task"));
        assert!(is_inside("/w/task/src/deep", "/w/task"));
        assert!(!is_inside("/w/task-2", "/w/task"));
        assert!(!is_inside("/w", "/w/task"));
        assert!(!is_inside("", "/w/task"));
        if cfg!(windows) {
            assert!(is_inside(r"C:\W\Task\src", r"c:\w\task"));
        }
    }

    fn repo_with_head(head: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git").join("HEAD"), head).unwrap();
        dir
    }

    #[test]
    fn branch_from_head_ref() {
        let repo = repo_with_head("ref: refs/heads/feature/x\n");
        fs::create_dir_all(repo.path().join("src").join("deep")).unwrap();
        // Found from a sub folder too.
        assert_eq!(
            git_branch(&repo.path().join("src").join("deep")),
            Some("feature/x".into())
        );
    }

    #[test]
    fn branch_detached_is_short_sha() {
        let repo = repo_with_head("0123456789abcdef0123456789abcdef01234567\n");
        assert_eq!(git_branch(repo.path()), Some("0123456".into()));
    }

    #[test]
    fn branch_through_gitdir_file() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("main").join(".git").join("worktrees").join("wt");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("HEAD"), "ref: refs/heads/wt-branch\n").unwrap();
        let work = dir.path().join("wt");
        fs::create_dir(&work).unwrap();
        fs::write(work.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        assert_eq!(git_branch(&work), Some("wt-branch".into()));
    }

    #[test]
    fn no_git_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(git_branch(dir.path()), None);
    }

    #[test]
    fn record_dedups_and_caps_at_30() {
        let home = tempfile::tempdir().unwrap();
        let projects = tempfile::tempdir().unwrap();
        let mut dirs = Vec::new();
        for i in 0..32 {
            let d = projects.path().join(format!("p{i}"));
            fs::create_dir(&d).unwrap();
            record(home.path(), &d, i as u64).unwrap();
            dirs.push(d);
        }
        // Using p5 again moves it to the top without adding a second entry.
        record(home.path(), &dirs[5], 101).unwrap();
        let list = list(home.path());
        assert_eq!(list.len(), 30);
        assert_eq!(list[0].name, "p5");
        assert_eq!(list[0].last_used_ms, 101);
        assert_eq!(list[1].name, "p31");
        assert_eq!(list.iter().filter(|p| p.name == "p5").count(), 1);
        // p0 and p1 were the oldest and fell off.
        assert!(!list.iter().any(|p| p.name == "p0" || p.name == "p1"));
    }

    #[test]
    fn list_drops_missing_paths() {
        let home = tempfile::tempdir().unwrap();
        let projects = tempfile::tempdir().unwrap();
        let keep = projects.path().join("keep");
        let gone = projects.path().join("gone");
        fs::create_dir(&keep).unwrap();
        fs::create_dir(&gone).unwrap();
        record(home.path(), &gone, 1).unwrap();
        record(home.path(), &keep, 2).unwrap();
        fs::remove_dir(&gone).unwrap();
        let list = list(home.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "keep");
        assert_eq!(list[0].path, keep.to_string_lossy());
    }

    #[test]
    fn home_is_not_recorded() {
        let home = tempfile::tempdir().unwrap();
        let user_home = dirs::home_dir().unwrap();
        record(home.path(), &user_home, 1).unwrap();
        assert!(!file(home.path()).exists());
    }

    #[test]
    fn home_with_trailing_separator_is_not_recorded() {
        let home = tempfile::tempdir().unwrap();
        let user_home = dirs::home_dir().unwrap().to_string_lossy().into_owned();
        for suffix in ["/", "\\", "//"] {
            record(home.path(), Path::new(&format!("{user_home}{suffix}")), 1).unwrap();
        }
        assert!(!file(home.path()).exists());
    }

    #[test]
    fn trailing_separator_dedups_to_one_entry() {
        let home = tempfile::tempdir().unwrap();
        let projects = tempfile::tempdir().unwrap();
        let plain = projects.path().to_string_lossy().into_owned();
        record(home.path(), Path::new(&plain), 1).unwrap();
        record(home.path(), Path::new(&format!("{plain}/")), 2).unwrap();
        record(home.path(), Path::new(&format!("{plain}\\")), 3).unwrap();
        let list = list(home.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].path, plain);
        assert_eq!(list[0].last_used_ms, 3);
    }

    #[test]
    fn a_root_is_not_recorded() {
        let home = tempfile::tempdir().unwrap();
        record(home.path(), Path::new(if cfg!(windows) { "C:\\" } else { "/" }), 1).unwrap();
        assert!(!file(home.path()).exists());
    }

    #[test]
    fn a_path_that_folds_to_a_root_is_not_recorded() {
        let home = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let drive_root = scratch.path().ancestors().last().unwrap().to_path_buf();
        let first = scratch
            .path()
            .components()
            .nth(if cfg!(windows) { 2 } else { 1 })
            .unwrap();
        // An existing folder and a missing one, then `..` back to the root; and a nested climb.
        let existing = drive_root.join(first.as_os_str()).join("..");
        let missing = drive_root.join("marshell-missing-probe").join("..");
        let nested = drive_root.join("marshell-a").join("marshell-b").join("..").join("..");
        for cwd in [existing, missing, nested] {
            record(home.path(), &cwd, 1).unwrap();
            assert!(!file(home.path()).exists(), "{}", cwd.display());
        }
    }

    #[test]
    fn is_root_holds_for_roots_only() {
        assert!(is_root(Path::new("/")));
        if cfg!(windows) {
            for root in ["C:\\", "C:/", "\\\\server\\share\\"] {
                assert!(is_root(Path::new(root)), "{root}");
            }
        }
        assert!(!is_root(Path::new("C:\\x")));
    }

    #[cfg(windows)]
    #[test]
    fn a_unc_path_loses_its_verbatim_prefix() {
        // The administrative share of this machine: reachable without any network.
        let shared = canonical(Path::new(r"\\localhost\C$\Windows"));
        let text = shared.to_string_lossy().into_owned();
        assert!(!text.starts_with(r"\\?\"), "{text}");
        assert!(text.to_lowercase().starts_with(r"\\localhost\c$\windows"), "{text}");
        assert!(is_root(&canonical(Path::new(r"\\localhost\C$"))));
    }

    #[test]
    fn trim_keeps_roots() {
        assert_eq!(trim_separators("/"), "/");
        assert_eq!(trim_separators("C:\\"), "C:\\");
        assert_eq!(trim_separators("C:/"), "C:/");
        assert_eq!(trim_separators("/a/b//"), "/a/b");
        assert_eq!(trim_separators("C:\\a\\"), "C:\\a");
        assert_eq!(trim_separators("/a"), "/a");
    }

    #[test]
    fn concurrent_records_all_survive() {
        let home = tempfile::tempdir().unwrap();
        let projects = tempfile::tempdir().unwrap();
        let handles: Vec<_> = (0..12)
            .map(|i| {
                let home = home.path().to_path_buf();
                let dir = projects.path().join(format!("p{i}"));
                std::fs::create_dir(&dir).unwrap();
                std::thread::spawn(move || record(&home, &dir, i as u64).unwrap())
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(list(home.path()).len(), 12);
    }

    #[test]
    fn record_leaves_a_newer_file_alone() {
        let home = tempfile::tempdir().unwrap();
        let projects = tempfile::tempdir().unwrap();
        let newer = r#"{"version":9,"projects":[]}"#;
        fs::write(file(home.path()), newer).unwrap();
        assert!(record(home.path(), projects.path(), 1).is_err());
        assert_eq!(fs::read_to_string(file(home.path())).unwrap(), newer);
        assert!(list(home.path()).is_empty());
    }
}
