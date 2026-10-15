//! Recent project folders, kept in `~/.marshell/projects.json` for the launcher.
//! The branch is read from the folder's git files when the list is asked for, not stored.

use crate::store::{self, Versioned};
use marshell_protocol::api::ProjectInfo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// Windows paths ignore case, so `C:\Dev\App` and `c:\dev\app` are one project there.
fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// The current branch of the git repository that holds `dir`, if any.
/// A detached HEAD gives the first 7 characters of the commit.
pub fn git_branch(dir: &Path) -> Option<String> {
    let repo = dir.ancestors().find(|d| d.join(".git").exists())?;
    let dot_git = repo.join(".git");
    // A worktree has a `.git` file that points at the real git folder.
    let git_dir = if dot_git.is_file() {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let target = text.trim().strip_prefix("gitdir:")?.trim();
        repo.join(target)
    } else {
        dot_git
    };
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            Some(reference.strip_prefix("refs/heads/").unwrap_or(reference).to_string())
        }
        None => (!head.is_empty()).then(|| head.chars().take(7).collect()),
    }
}

/// Puts `cwd` at the top of the recent list. The user's home folder is never recorded.
/// Refuses when the file cannot be read (broken, or written by a newer Marshell),
/// so a file the user may want to repair is never overwritten.
pub fn record(home: &Path, cwd: &Path, now_ms: u64) -> anyhow::Result<()> {
    if dirs::home_dir().is_some_and(|user_home| same_path(&user_home.to_string_lossy(), &cwd.to_string_lossy())) {
        return Ok(());
    }
    let path = file(home);
    let mut doc = store::read::<FileDoc>(&path)?.unwrap_or_default();
    let cwd = cwd.to_string_lossy().into_owned();
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
