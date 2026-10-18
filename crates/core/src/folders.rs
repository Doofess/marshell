//! Checks a folder path the person typed in the launcher: is it a full path, and is a folder there.

use crate::projects;
use marshell_protocol::api::FolderInfo;
use std::path::PathBuf;

/// Why typed text is not a folder to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderRefusal {
    /// The text is not a full path for this OS (and not `~`), so it only filters the known folders.
    NotFullPath,
    /// A full path, but nothing is there, or what is there is not a folder.
    Missing,
}

/// The home folder for a leading `~` (alone, or followed by `/` or `\`), else the text as it is.
fn expand_tilde(text: &str) -> PathBuf {
    if let Some(rest) = text.strip_prefix('~')
        && (rest.is_empty() || rest.starts_with(['/', '\\']))
        && let Some(home) = dirs::home_dir()
    {
        let rest = rest.trim_start_matches(['/', '\\']);
        return if rest.is_empty() { home } else { home.join(rest) };
    }
    PathBuf::from(text)
}

/// Looks at `raw` as typed: trims it, expands `~`, and answers with the folder's canonical path,
/// name and branch. A path with `..` parts is fine: `canonical` resolves it.
pub fn check(raw: &str) -> Result<FolderInfo, FolderRefusal> {
    let path = expand_tilde(raw.trim());
    if !path.is_absolute() {
        return Err(FolderRefusal::NotFullPath);
    }
    if !path.is_dir() {
        return Err(FolderRefusal::Missing);
    }
    let path = projects::canonical(&path);
    let text = path.to_string_lossy().into_owned();
    Ok(FolderInfo {
        name: path
            .file_name()
            .map_or_else(|| text.clone(), |n| n.to_string_lossy().into_owned()),
        branch: projects::git_branch(&path),
        path: text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn text(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    #[test]
    fn an_existing_folder_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let got = check(&text(&dir)).unwrap();
        assert_eq!(got.path, projects::canonical(dir.path()).to_string_lossy());
        assert_eq!(got.name, dir.path().file_name().unwrap().to_string_lossy().into_owned());
        assert_eq!(got.branch, None);
    }

    #[test]
    fn a_folder_in_git_has_its_branch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(check(&text(&dir)).unwrap().branch.as_deref(), Some("main"));
    }

    #[test]
    fn a_missing_folder_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let nope = dir.path().join("nope");
        assert_eq!(check(&nope.to_string_lossy()), Err(FolderRefusal::Missing));
    }

    #[test]
    fn a_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(check(&file.to_string_lossy()), Err(FolderRefusal::Missing));
    }

    #[test]
    fn tilde_is_the_home_folder() {
        let home = projects::canonical(&dirs::home_dir().unwrap())
            .to_string_lossy()
            .into_owned();
        assert_eq!(check("~").unwrap().path, home);
        assert_eq!(check("~/").unwrap().path, home);
        if cfg!(windows) {
            assert_eq!(check("~\\").unwrap().path, home);
        }
    }

    #[test]
    fn a_relative_path_is_not_a_full_path() {
        for raw in ["src", "./x", "", "   "] {
            assert_eq!(check(raw), Err(FolderRefusal::NotFullPath), "{raw:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_rooted_path_without_a_drive_is_not_full_on_windows() {
        for raw in ["/x", "\\x"] {
            assert_eq!(check(raw), Err(FolderRefusal::NotFullPath), "{raw:?}");
        }
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check(&format!("  {}  ", text(&dir))).is_ok());
    }

    #[test]
    fn dot_dot_parts_are_resolved() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("a")).unwrap();
        let raw = Path::new(&text(&dir)).join("a").join("..");
        let got = check(&raw.to_string_lossy()).unwrap();
        assert_eq!(got.path, projects::canonical(dir.path()).to_string_lossy());
    }
}
