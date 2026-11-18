//! Which of the person's files a text tab's save may write. A save reaches any file the person can
//! write, except Marshell's own folder and what an organization manages. The path's shape (UNC,
//! device names, links to them) is checked before this, by the files API's own rules.

use crate::config::guard::{MANAGED_NAMES, denied_for_os};
use crate::projects::{canonical, is_inside};
use std::path::{Path, PathBuf};

/// Why a save is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRefusal {
    /// The file is in Marshell's own folder.
    Own,
    /// An organization manages the file: its name or its folder says so.
    Managed,
}

impl SaveRefusal {
    /// The word the API uses for it (`FileStat::read_only`).
    pub fn word(self) -> &'static str {
        match self {
            SaveRefusal::Own => "own",
            SaveRefusal::Managed => "managed",
        }
    }

    /// The line a person reads.
    pub fn line(self) -> &'static str {
        match self {
            SaveRefusal::Own => "Marshell doesn't save files in its own folder.",
            SaveRefusal::Managed => "This file is managed by your organization, so Marshell doesn't save it.",
        }
    }
}

/// Why a save of `path` is refused, or None when it may go ahead. It reads the disk to resolve links.
pub fn save_refusal(path: &Path, marshell_home: &Path) -> Option<SaveRefusal> {
    save_refusal_with(path, marshell_home, &denied_for_os())
}

/// The file `path` names once every link on the way is followed. A file that isn't there yet is
/// placed by its folder, so a link to a folder can't hide where a new file would land.
fn resolved(path: &Path) -> PathBuf {
    if std::fs::symlink_metadata(path).is_ok() {
        return canonical(path);
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => canonical(path),
    }
}

/// Whether a file name is one an organization's settings use. On Windows the case doesn't count,
/// and neither do trailing dots and spaces, which the OS drops when it opens the file.
fn managed_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    let name = if cfg!(windows) {
        name.trim_end_matches(['.', ' '])
    } else {
        &name
    };
    MANAGED_NAMES.iter().any(|m| {
        if cfg!(windows) {
            name.eq_ignore_ascii_case(m)
        } else {
            name == *m
        }
    })
}

fn save_refusal_with(path: &Path, marshell_home: &Path, denied: &[PathBuf]) -> Option<SaveRefusal> {
    let real = resolved(path);
    let text = real.to_string_lossy();
    if is_inside(&text, &marshell_home.to_string_lossy()) {
        return Some(SaveRefusal::Own);
    }
    let in_denied = denied.iter().any(|d| is_inside(&text, &d.to_string_lossy()));
    // Both names count: the one asked for, and the one a link leads to.
    let managed_name = [path, &real].iter().any(|p| p.file_name().is_some_and(managed_name));
    (in_denied || managed_name).then_some(SaveRefusal::Managed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::canonical;
    use std::fs;

    /// `root`, with Marshell's home at `root/home` and an organization's folder at `root/org`.
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, Vec<PathBuf>) {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let home = root.join("home");
        fs::create_dir_all(home.join("agents")).unwrap();
        fs::create_dir_all(root.join("org")).unwrap();
        fs::create_dir_all(root.join("proj")).unwrap();
        let denied = vec![root.join("org")];
        (dir, root, home, denied)
    }

    /// A folder link from `link` to `target`: a symlink on Unix, a junction on Windows. False when it can't be made.
    fn link_folder(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success())
        }
    }

    #[test]
    fn guard_passes_a_project_file() {
        let (_dir, root, home, denied) = setup();
        let file = root.join("proj").join("a.txt");
        fs::write(&file, "x").unwrap();
        assert_eq!(save_refusal_with(&file, &home, &denied), None);
        // A file that isn't there yet passes too: a save may put back a deleted file.
        assert_eq!(
            save_refusal_with(&root.join("proj").join("new.txt"), &home, &denied),
            None
        );
    }

    #[test]
    fn guard_refuses_marshells_home_and_its_agents_folder() {
        let (_dir, _root, home, denied) = setup();
        for path in [
            home.join("settings.json"),
            home.join("agents").join("x.md"),
            home.join("new.txt"),
        ] {
            assert_eq!(
                save_refusal_with(&path, &home, &denied),
                Some(SaveRefusal::Own),
                "{path:?}"
            );
        }
    }

    #[test]
    fn guard_refuses_a_managed_name_in_any_case() {
        let (_dir, root, home, denied) = setup();
        let proj = root.join("proj");
        for name in ["managed-settings.json", "managed-mcp.json", "managed_config.toml"] {
            assert_eq!(
                save_refusal_with(&proj.join(name), &home, &denied),
                Some(SaveRefusal::Managed),
                "{name}"
            );
        }
        if cfg!(windows) {
            // The OS opens `name.` and `name ` as `name`.
            for name in ["Managed-Settings.JSON", "managed-settings.json.", "managed-mcp.json ."] {
                assert_eq!(
                    save_refusal_with(&proj.join(name), &home, &denied),
                    Some(SaveRefusal::Managed),
                    "{name}"
                );
            }
        }
        assert_eq!(
            save_refusal_with(&proj.join("managed-settings.json.bak"), &home, &denied),
            None
        );
    }

    #[test]
    fn guard_refuses_a_denied_folder() {
        let (_dir, root, home, denied) = setup();
        let path = root.join("org").join("policy.json");
        assert_eq!(save_refusal_with(&path, &home, &denied), Some(SaveRefusal::Managed));
    }

    #[test]
    fn guard_refuses_a_link_into_marshells_home() {
        let (_dir, root, home, denied) = setup();
        fs::write(home.join("settings.json"), "{}").unwrap();
        let link = root.join("proj").join("inside");
        let made = link_folder(&home, &link);
        // A junction needs no privilege, so on Windows the link is always made.
        assert!(made || !cfg!(windows));
        if !made {
            return;
        }
        // Through the link, a file that exists and one that doesn't yet.
        for name in ["settings.json", "new.txt"] {
            assert_eq!(
                save_refusal_with(&link.join(name), &home, &denied),
                Some(SaveRefusal::Own),
                "{name}"
            );
        }
    }

    #[test]
    fn guard_refuses_a_link_into_a_denied_folder() {
        let (_dir, root, home, denied) = setup();
        let link = root.join("proj").join("plain");
        let made = link_folder(&root.join("org"), &link);
        // A junction needs no privilege, so on Windows the link is always made.
        assert!(made || !cfg!(windows));
        if !made {
            return;
        }
        assert_eq!(
            save_refusal_with(&link.join("a.json"), &home, &denied),
            Some(SaveRefusal::Managed)
        );
    }
}
