//! Atomic replace and remove for the user's config files. These are the user's files, so they are not
//! made private: the folder keeps its permissions and a new file gets the default mode.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scanner or an editor can hold a file for a moment, so a rename or remove gets five tries.
const TRIES: u32 = 5;
const RETRY_PAUSE: Duration = Duration::from_millis(50);

/// The file a write lands in: the link's target when `path` is a symlink, else `path` itself.
/// A link whose target is missing still resolves, so the write creates the target and keeps the link.
pub fn resolve_link(path: &Path) -> io::Result<PathBuf> {
    let mut current = path.to_path_buf();
    // A cycle of links is an error on the OS's side too; stop after the same bound it uses.
    for _ in 0..40 {
        if !std::fs::symlink_metadata(&current).is_ok_and(|m| m.file_type().is_symlink()) {
            return Ok(current);
        }
        let target = std::fs::read_link(&current)?;
        current = match current.parent() {
            Some(parent) => parent.join(target),
            None => target,
        };
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

/// Writes `bytes` to `.<name>.marshell-<pid>-<n>.tmp` in the same folder, fsyncs it, keeps the old file's
/// unix mode, and renames it over `path` (5 tries, 50 ms apart). Missing parent folders are created.
pub fn replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace_with(path, bytes, |from, to| std::fs::rename(from, to))
}

/// Removes `path` with the same retries; a missing file is Ok.
pub fn remove_file(path: &Path) -> io::Result<()> {
    retry(|| match std::fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    })
}

/// Runs `op` up to `TRIES` times with a pause between, and returns the last error if none worked.
fn retry(mut op: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut tries = 0;
    loop {
        match op() {
            Ok(()) => return Ok(()),
            Err(e) if tries + 1 >= TRIES => return Err(e),
            Err(_) => {
                tries += 1;
                std::thread::sleep(RETRY_PAUSE);
            }
        }
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.marshell-{}-{n}.tmp", std::process::id()))
}

fn replace_with(path: &Path, bytes: &[u8], rename: impl Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let temp = temp_path(path);
    let result = stage(path, &temp, bytes).and_then(|()| retry(|| rename(&temp, path)));
    if result.is_err() {
        // Never leave a temp file behind.
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// Writes the new content to `temp`, flushed to disk, with the old file's mode when there is one.
fn stage(path: &Path, temp: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(temp)?;
    file.write_all(bytes)?;
    #[cfg(unix)]
    if let Ok(old) = std::fs::metadata(path) {
        file.set_permissions(old.permissions())?;
    }
    #[cfg(not(unix))]
    let _ = path;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn replaces_contents_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        replace_file(&file, b"first").unwrap();
        replace_file(&file, b"second").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"second");
        assert_eq!(names(dir.path()), ["settings.json"]);
    }

    #[test]
    fn creates_missing_parent_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".claude").join("settings.local.json");
        replace_file(&file, b"{}").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"{}");
    }

    #[test]
    fn a_locked_rename_is_retried() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.json");
        std::fs::write(&file, b"old").unwrap();
        let failures = Cell::new(0);
        replace_with(&file, b"new", |from, to| {
            if failures.get() < 2 {
                failures.set(failures.get() + 1);
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "held by a scanner"))
            } else {
                std::fs::rename(from, to)
            }
        })
        .unwrap();
        assert_eq!(failures.get(), 2);
        assert_eq!(std::fs::read(&file).unwrap(), b"new");
        assert_eq!(names(dir.path()), ["a.json"]);
    }

    #[test]
    fn a_rename_that_keeps_failing_leaves_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.json");
        std::fs::write(&file, b"old").unwrap();
        let attempts = Cell::new(0);
        let result = replace_with(&file, b"new", |_, _| {
            attempts.set(attempts.get() + 1);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "held"))
        });
        assert!(result.is_err());
        assert_eq!(attempts.get(), 5);
        assert_eq!(std::fs::read(&file).unwrap(), b"old");
        assert_eq!(names(dir.path()), ["a.json"], "no .tmp is left");
    }

    #[cfg(unix)]
    #[test]
    fn keeps_unix_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.json");
        std::fs::write(&file, b"old").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
        replace_file(&file, b"new").unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o640);
    }

    #[test]
    fn writes_through_a_link_to_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        let link = dir.path().join("link.json");
        std::fs::write(&real, b"old").unwrap();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&real, &link);
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(&real, &link);
        if let Err(e) = made {
            eprintln!("skipped: cannot create a symlink here: {e}");
            return;
        }
        replace_file(&resolve_link(&link).unwrap(), b"new").unwrap();
        assert_eq!(std::fs::read(&real).unwrap(), b"new");
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read(&link).unwrap(), b"new");
    }

    #[test]
    fn a_plain_file_resolves_to_itself() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.json");
        assert_eq!(resolve_link(&file).unwrap(), file);
    }

    #[test]
    fn remove_missing_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        remove_file(&dir.path().join("nope.json")).unwrap();
        let file = dir.path().join("a.json");
        std::fs::write(&file, b"x").unwrap();
        remove_file(&file).unwrap();
        assert!(!file.exists());
    }
}
