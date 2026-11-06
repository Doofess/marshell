//! Finding the real Copilot CLI program. VS Code puts a `copilot-shim` folder on PATH whose `copilot` starts the real
//! one as a child; Marshell never launches that shim (decision 11), so it looks past it.

use crate::agents::identify::Missing;
use std::path::{Path, PathBuf};

/// The programs a PATH folder may hold, in the order they are tried: the winget and curl installs are a `.exe`, the npm
/// install a `.cmd`.
const NAMES: [&str; 2] = ["copilot.exe", "copilot.cmd"];

/// Where winget puts it, under `%LOCALAPPDATA%`.
const WINGET_DIR: &str = "GitHubCopilotCLI";

/// True when some segment of `path` is VS Code's `copilot-shim` folder. Case and slash direction don't matter.
pub fn in_shim_folder(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
        .split('\\')
        .any(|segment| segment == "copilot-shim")
}

/// The real Copilot CLI program, or why there is none.
/// 1. The first `copilot.exe` or `copilot.cmd` in a PATH folder that is not a `copilot-shim` folder.
/// 2. Otherwise `<local_app_data>/GitHubCopilotCLI/copilot.exe`, if it is a file.
/// 3. Otherwise `Rejected` with the shim's path when one was seen, else `NotOnPath`.
///
/// Touches the disk, so async callers go through `spawn_blocking`.
pub fn resolve(path_dirs: &[PathBuf], local_app_data: Option<&Path>) -> Result<PathBuf, Missing> {
    let mut shim = None;
    for dir in path_dirs {
        for name in NAMES {
            let candidate = dir.join(name);
            if !candidate.is_file() {
                continue;
            }
            if in_shim_folder(&candidate) {
                shim.get_or_insert(candidate);
            } else {
                return Ok(candidate);
            }
        }
    }
    if let Some(winget) = local_app_data.map(|base| base.join(WINGET_DIR).join(NAMES[0]))
        && winget.is_file()
    {
        return Ok(winget);
    }
    Err(match shim {
        Some(path) => Missing::Rejected { path },
        None => Missing::NotOnPath,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder holding an empty `file` (never run), inside `root`.
    fn folder_with(root: &Path, segments: &[&str], file: &str) -> PathBuf {
        let dir = segments.iter().fold(root.to_path_buf(), |dir, s| dir.join(s));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), "").unwrap();
        dir
    }

    #[test]
    fn the_shim_is_skipped_for_a_later_path_entry() {
        let root = tempfile::tempdir().unwrap();
        let shim = folder_with(root.path(), &["VS Code", "bin", "copilot-shim"], "copilot.exe");
        let real = folder_with(root.path(), &["npm"], "copilot.cmd");
        let found = resolve(&[shim, real.clone()], None).unwrap();
        assert_eq!(found, real.join("copilot.cmd"));
    }

    #[test]
    fn the_winget_folder_is_used_when_path_has_only_the_shim() {
        let root = tempfile::tempdir().unwrap();
        let shim = folder_with(root.path(), &["bin", "copilot-shim"], "copilot.cmd");
        let local = root.path().join("local");
        folder_with(&local, &["GitHubCopilotCLI"], "copilot.exe");
        let found = resolve(&[shim], Some(&local)).unwrap();
        assert_eq!(found, local.join("GitHubCopilotCLI").join("copilot.exe"));
    }

    #[test]
    fn only_the_shim_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let shim = folder_with(root.path(), &["bin", "copilot-shim"], "copilot.exe");
        let local = root.path().join("local");
        assert_eq!(
            resolve(std::slice::from_ref(&shim), Some(&local)),
            Err(Missing::Rejected {
                path: shim.join("copilot.exe")
            })
        );
    }

    #[test]
    fn nothing_is_not_on_path() {
        let root = tempfile::tempdir().unwrap();
        let empty = root.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(
            resolve(&[empty, root.path().join("missing")], None),
            Err(Missing::NotOnPath)
        );
        assert_eq!(resolve(&[], Some(root.path())), Err(Missing::NotOnPath));
        // A winget folder with no program in it is not a find either.
        std::fs::create_dir_all(root.path().join("GitHubCopilotCLI")).unwrap();
        assert_eq!(resolve(&[], Some(root.path())), Err(Missing::NotOnPath));
    }

    #[test]
    fn a_shim_folder_in_either_case_or_slash_is_skipped() {
        for spelled in ["Copilot-Shim", "COPILOT-SHIM", "copilot-shim"] {
            let root = tempfile::tempdir().unwrap();
            let shim = folder_with(root.path(), &["bin", spelled], "copilot.exe");
            assert!(
                matches!(resolve(&[shim], None), Err(Missing::Rejected { .. })),
                "{spelled}"
            );
        }
        // A forward-slash spelling of the same folder, and a segment that only contains the name.
        assert!(in_shim_folder(Path::new(
            "C:/Program Files/VS Code/bin/Copilot-Shim/copilot.exe"
        )));
        assert!(in_shim_folder(Path::new(r"c:\x\COPILOT-SHIM\copilot.cmd")));
        assert!(!in_shim_folder(Path::new(r"C:\x\copilot-shim-not\copilot.exe")));
        assert!(!in_shim_folder(Path::new(r"C:\x\my-copilot-shim\copilot.exe")));
        let root = tempfile::tempdir().unwrap();
        let near = folder_with(root.path(), &["copilot-shims"], "copilot.exe");
        assert_eq!(resolve(std::slice::from_ref(&near), None), Ok(near.join("copilot.exe")));
    }
}
