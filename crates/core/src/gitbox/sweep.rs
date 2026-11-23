//! Removes the sandboxes (`git-sandbox/<hash>`) that are not needed any more.
//!
//! Each sandbox names its checkout in the file `OWNER_FILE`, written by `GitBox::reload`. A sandbox goes
//! when its checkout is gone or its file has not been touched for `UNUSED_AFTER`; it is rebuilt on the next
//! use. This is the only code that deletes a folder under `git-sandbox`, and `ours` is the only test of what
//! it may delete: a real folder (never a link or a junction), named by the hash of the path its own file
//! holds. Nothing else is touched, and a path that can't be looked at safely is never looked for.
//!
//! `GATE` keeps a removal from running while a sandbox is written. It is a plain thread lock like `Slot::work`
//! (`cache.rs`), never held in async code, and nothing that runs under it takes it again.

use super::{sandbox_dir, sandbox_name, write_file};
use crate::server::{links_refused, refused};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockWriteGuard, TryLockError};
use std::time::{Duration, SystemTime};

/// The file in a sandbox that holds its checkout's path, as UTF-8 with no line break.
pub const OWNER_FILE: &str = "marshell-checkout";
pub const UNUSED_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const TEMP_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
pub(super) const TOUCH_EVERY: Duration = Duration::from_secs(60 * 60);
/// The most bytes `OWNER_FILE` may hold.
const OWNER_CAP: u64 = 32 * 1024;
/// Held shared while a sandbox is written (`GitBox::reload`), and alone for each folder a sweep removes.
pub(super) static GATE: RwLock<()> = RwLock::new(());

/// What a sweep did: the sandboxes it removed, and the ones it looked at and left.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub removed: usize,
    pub kept: usize,
}

/// Why a sandbox may be removed.
#[derive(Clone, Copy)]
enum Why {
    /// Its checkout is gone (worked out before the gate was taken).
    Gone,
    /// Its file is older than `UNUSED_AFTER` at this time.
    Unused(SystemTime),
    /// It was asked for: `forget` and `remove_all`.
    Asked,
}

/// Whether `meta` is a folder that is not a link, a junction or any other reparse point.
fn real_dir(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_dir() && !reparse_point(meta)
}

/// Whether `meta` is a regular file that is not a link or a reparse point.
fn real_file(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_file() && !reparse_point(meta)
}

#[cfg(windows)]
fn reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn reparse_point(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

/// Whether `path` is a real folder, by `symlink_metadata`.
fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| real_dir(&m))
}

/// The one test of what may be removed: `Some((checkout, modified))` for a sandbox `dir` named `name`. The
/// name is 16 lower-case hex digits; `dir` is a real folder; its `OWNER_FILE` is a regular file of at most
/// 32 KiB holding UTF-8; and the name is the hash of the path in that file. `modified` is the file's.
fn ours(dir: &Path, name: &str) -> Option<(PathBuf, SystemTime)> {
    if name.len() != 16 || !name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    if !is_real_dir(dir) {
        return None;
    }
    let file = dir.join(OWNER_FILE);
    let meta = std::fs::symlink_metadata(&file).ok()?;
    if !real_file(&meta) || meta.len() > OWNER_CAP {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&file)
        .ok()?
        .take(OWNER_CAP + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > OWNER_CAP {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    if sandbox_name(Path::new(&text)) != name {
        return None;
    }
    Some((PathBuf::from(text), meta.modified().ok()?))
}

/// Whether the checkout is known not to be there: an absolute path that is not a network or device path, with
/// no link on the way that leads to one, and that the disk answers "not found" for. Any other answer (a bad
/// name, a drive that is not ready, no right to look) is not "gone". This asks the disk, so it runs before the
/// gate is taken.
fn gone(checkout: &Path) -> bool {
    let Some(text) = checkout.to_str() else {
        return false;
    };
    checkout.is_absolute()
        && !refused(text)
        && !links_refused(checkout)
        && std::fs::symlink_metadata(checkout).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

/// Whether the file's `modified` is more than `UNUSED_AFTER` before `now`. A time in the future is not old.
fn old(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified).is_ok_and(|age| age > UNUSED_AFTER)
}

/// The gate, alone, or None when a box is writing. A poisoned gate is used as it is.
#[cfg(not(test))]
fn exclusive() -> Option<RwLockWriteGuard<'static, ()>> {
    try_gate()
}

/// In a test the gate is waited for: the tests run side by side and some hold it for a whole check.
#[cfg(test)]
fn exclusive() -> Option<RwLockWriteGuard<'static, ()>> {
    let until = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(guard) = try_gate() {
            return Some(guard);
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// One try at the gate: None when it is busy.
fn try_gate() -> Option<RwLockWriteGuard<'static, ()>> {
    match GATE.try_write() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

/// Removes the sandbox `dir`, named `name`, for `why`. True when it was removed. The gate is taken alone, and
/// `ours` and the age are tested again under it, so a sandbox opened since the first look stays. A folder that
/// can't be removed and is still there gets its file back and counts as kept. `first` is the checkout the
/// first look read, which a `Why::Gone` removal must still find in the file.
fn remove_one(dir: &Path, name: &str, why: Why, first: Option<&Path>) -> bool {
    let Some(_gate) = exclusive() else {
        return false;
    };
    let Some((checkout, modified)) = ours(dir, name) else {
        return false;
    };
    match why {
        Why::Gone if first.is_some_and(|seen| seen != checkout) => return false,
        Why::Unused(now) if !old(modified, now) => return false,
        _ => {}
    }
    let Err(e) = std::fs::remove_dir_all(dir) else {
        return true;
    };
    if !dir.exists() {
        return true;
    }
    // Half a sandbox must not pass for a whole one: without `index` the next check copies the index and its
    // shared part again. Then the file goes back, so that a later sweep may try again.
    let _ = std::fs::remove_file(dir.join("index"));
    if let Some(text) = checkout.to_str() {
        let _ = write_file(dir, OWNER_FILE, text.as_bytes(), Some(modified));
    }
    tracing::warn!("couldn't remove the git sandbox {}: {e}", dir.display());
    false
}

/// Removes the temp files in `dir` that `write_file` leaves when it is interrupted: regular files named
/// `.*.tmp`, older than `TEMP_AFTER` at `now`, or of any age when `now` is None. Nothing happens unless `dir`
/// is a real folder. Under the gate, since a temp file is dated as its source while it is written.
fn remove_temps(dir: &Path, now: Option<SystemTime>) {
    let Some(_gate) = exclusive() else {
        return;
    };
    if !is_real_dir(dir) {
        return;
    }
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !(name.starts_with('.') && name.ends_with(".tmp")) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !real_file(&meta) {
            continue;
        }
        let stale = match now {
            None => true,
            Some(now) => meta
                .modified()
                .is_ok_and(|modified| now.duration_since(modified).is_ok_and(|age| age > TEMP_AFTER)),
        };
        if stale {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The folders in `git-sandbox` that are named like a sandbox and that `ours` accepts, when `git-sandbox` is a
/// real folder.
fn sandboxes(home: &Path) -> Vec<(PathBuf, String)> {
    let root = crate::paths::git_sandbox_dir(home);
    if !is_real_dir(&root) {
        return Vec::new();
    }
    std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let dir = entry.path();
            ours(&dir, &name).map(|_| (dir, name))
        })
        .collect()
}

/// Removes each sandbox whose checkout is gone or whose file is older than `UNUSED_AFTER` at `now`, and the
/// temp files older than `TEMP_AFTER` in the ones kept, in their `info` folders and in `git-sandbox` itself.
pub fn sweep(home: &Path, now: SystemTime) -> Swept {
    let mut swept = Swept::default();
    for (dir, name) in sandboxes(home) {
        let Some((checkout, modified)) = ours(&dir, &name) else {
            continue;
        };
        let why = if gone(&checkout) {
            Some(Why::Gone)
        } else if old(modified, now) {
            Some(Why::Unused(now))
        } else {
            None
        };
        if let Some(why) = why
            && remove_one(&dir, &name, why, Some(&checkout))
        {
            swept.removed += 1;
            continue;
        }
        swept.kept += 1;
        remove_temps(&dir, Some(now));
        remove_temps(&dir.join("info"), Some(now));
    }
    remove_temps(&crate::paths::git_sandbox_dir(home), Some(now));
    swept
}

/// Removes the sandbox of `checkout`, when there is one. True when one was removed.
pub fn forget(home: &Path, checkout: &Path) -> bool {
    let dir = sandbox_dir(home, checkout);
    let Some(name) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
        return false;
    };
    remove_one(&dir, &name, Why::Asked, None)
}

/// Removes every sandbox, then `empty-config` and the temp files in `git-sandbox` itself whatever their age,
/// and `git-sandbox` when it is then empty. The number of sandboxes removed. For uninstall.
pub fn remove_all(home: &Path) -> usize {
    let root = crate::paths::git_sandbox_dir(home);
    let mut removed = 0;
    for (dir, name) in sandboxes(home) {
        if remove_one(&dir, &name, Why::Asked, None) {
            removed += 1;
        }
    }
    remove_temps(&root, None);
    if let Some(_gate) = exclusive()
        && is_real_dir(&root)
    {
        let empty = root.join("empty-config");
        if std::fs::symlink_metadata(&empty).is_ok_and(|m| real_file(&m)) {
            let _ = std::fs::remove_file(&empty);
        }
        // Only an empty folder goes.
        let _ = std::fs::remove_dir(&root);
    }
    removed
}

/// Whether `remove_all` would remove something: `git-sandbox` holds a sandbox `ours` accepts, or
/// `empty-config`. For uninstall's question.
pub fn any(home: &Path) -> bool {
    if !sandboxes(home).is_empty() {
        return true;
    }
    let root = crate::paths::git_sandbox_dir(home);
    is_real_dir(&root) && std::fs::symlink_metadata(root.join("empty-config")).is_ok_and(|m| real_file(&m))
}

/// Runs `sweep` once, off the async threads, and logs how many it removed when that isn't zero.
pub fn spawn(home: PathBuf) {
    drop(tokio::task::spawn_blocking(move || {
        let swept = sweep(&home, SystemTime::now());
        if swept.removed > 0 {
            tracing::info!("removed {} unused git sandboxes", swept.removed);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::super::{sandbox_dir, sandbox_name};
    use super::*;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// A scratch folder holding a Marshell `home/` and the checkouts, all under one temp folder.
    struct Scratch {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        home: PathBuf,
    }

    fn scratch() -> Scratch {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(tmp.path());
        Scratch {
            home: root.join("home"),
            root,
            _tmp: tmp,
        }
    }

    fn date(path: &Path, modified: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    /// `sandbox_dir(home, checkout)` with a `HEAD` and an `OWNER_FILE` dated `modified`. The file's text is the
    /// absolute path, which is what `sandbox_dir` hashed.
    fn sandbox(home: &Path, checkout: &Path, modified: SystemTime) -> PathBuf {
        let dir = sandbox_dir(home, checkout);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("HEAD"), "ref: refs/heads/none\n").unwrap();
        let text = std::path::absolute(checkout).unwrap();
        std::fs::write(dir.join(OWNER_FILE), text.to_str().unwrap()).unwrap();
        date(&dir.join(OWNER_FILE), modified);
        dir
    }

    fn made(path: &Path) -> PathBuf {
        std::fs::create_dir_all(path).unwrap();
        path.to_path_buf()
    }

    fn name_of(dir: &Path) -> String {
        dir.file_name().unwrap().to_str().unwrap().to_string()
    }

    fn exists(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }

    /// A junction `link` to `target`, or false when Windows would not make one.
    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|out| out.status.success())
    }

    #[test]
    fn a_sandbox_whose_checkout_is_gone_is_removed() {
        let s = scratch();
        let now = SystemTime::now();
        let gone = sandbox(&s.home, &s.root.join("gone"), now);
        let here = made(&s.root.join("here"));
        let here_box = sandbox(&s.home, &here, now);
        assert_eq!(sweep(&s.home, now), Swept { removed: 1, kept: 1 });
        assert!(!gone.exists());
        assert!(here_box.exists());
    }

    #[test]
    fn a_sandbox_unused_for_seven_days_is_removed() {
        let s = scratch();
        let now = SystemTime::now();
        let here = made(&s.root.join("here"));
        let dir = sandbox(&s.home, &here, now);
        assert_eq!(sweep(&s.home, now + 6 * DAY), Swept { removed: 0, kept: 1 });
        assert!(dir.exists());
        let later = now + UNUSED_AFTER + Duration::from_secs(1);
        assert_eq!(sweep(&s.home, later), Swept { removed: 1, kept: 0 });
        assert!(!dir.exists());
    }

    #[test]
    fn a_file_dated_in_the_future_is_fresh() {
        let s = scratch();
        let now = SystemTime::now();
        let here = made(&s.root.join("here"));
        let dir = sandbox(&s.home, &here, now + 30 * DAY);
        assert_eq!(sweep(&s.home, now), Swept { removed: 0, kept: 1 });
        assert!(dir.exists());
    }

    #[test]
    fn a_folder_that_is_not_ours_is_left() {
        let s = scratch();
        let now = SystemTime::now();
        let root = made(&crate::paths::git_sandbox_dir(&s.home));
        // A folder with no file.
        let bare = made(&root.join("fedcba9876543210"));
        // A folder whose file names another path.
        let other = made(&root.join(sandbox_name(&s.root.join("a"))));
        std::fs::write(
            other.join(OWNER_FILE),
            std::path::absolute(s.root.join("b")).unwrap().to_str().unwrap(),
        )
        .unwrap();
        let notes = made(&root.join("notes"));
        let file = root.join("0123456789abcdef");
        std::fs::write(&file, "x").unwrap();
        // An upper-case name whose lower-case form is the hash of the path its file names.
        let (gone, upper) = (0..100)
            .map(|i| s.root.join(format!("gone{i}")))
            .map(|p| {
                let upper = sandbox_name(&p).to_uppercase();
                (p, upper)
            })
            .find(|(p, upper)| *upper != sandbox_name(p))
            .unwrap();
        let loud = made(&root.join(upper));
        std::fs::write(
            loud.join(OWNER_FILE),
            std::path::absolute(&gone).unwrap().to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(sweep(&s.home, now + 30 * DAY), Swept::default());
        for path in [&bare, &other, &notes, &file, &loud] {
            assert!(exists(path), "{path:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_link_named_like_a_sandbox_is_left() {
        let s = scratch();
        let now = SystemTime::now();
        let gone = s.root.join("gone");
        let real = made(&s.root.join("real"));
        std::fs::write(
            real.join(OWNER_FILE),
            std::path::absolute(&gone).unwrap().to_str().unwrap(),
        )
        .unwrap();
        let root = made(&crate::paths::git_sandbox_dir(&s.home));
        let link = root.join(sandbox_name(&std::path::absolute(&gone).unwrap()));
        if !junction(&link, &real) {
            return;
        }
        assert_eq!(sweep(&s.home, now + 30 * DAY), Swept::default());
        assert!(exists(&link));
        assert!(real.join(OWNER_FILE).is_file());
        std::fs::remove_dir(&link).unwrap();
    }

    #[test]
    fn a_checkout_on_the_network_is_never_looked_for() {
        let s = scratch();
        let now = SystemTime::now();
        let dir = sandbox(&s.home, Path::new("//server/share/repo"), now);
        assert_eq!(sweep(&s.home, now), Swept { removed: 0, kept: 1 });
        assert!(dir.exists());
        let later = now + UNUSED_AFTER + Duration::from_secs(1);
        assert_eq!(sweep(&s.home, later), Swept { removed: 1, kept: 0 });
        assert!(!dir.exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_checkout_that_cannot_be_looked_at_is_kept() {
        let s = scratch();
        let now = SystemTime::now();
        let dir = sandbox(&s.home, &s.root.join("a<b"), now);
        assert_eq!(sweep(&s.home, now), Swept { removed: 0, kept: 1 });
        assert!(dir.exists());
    }

    #[test]
    fn old_temp_files_go_and_new_ones_stay() {
        let s = scratch();
        let now = SystemTime::now();
        let here = made(&s.root.join("here"));
        let dir = sandbox(&s.home, &here, now);
        let info = made(&dir.join("info"));
        let loose = crate::paths::git_sandbox_dir(&s.home);
        let old_index = dir.join(".index.1-2.tmp");
        let new_index = dir.join(".index.3-4.tmp");
        let old_attributes = info.join(".attributes.5-6.tmp");
        let old_empty = loose.join(".empty-config.7-8.tmp");
        let keep = dir.join(".keep");
        for (path, age) in [
            (&old_index, 2 * DAY),
            (&new_index, Duration::ZERO),
            (&old_attributes, 2 * DAY),
            (&old_empty, 2 * DAY),
            (&keep, 2 * DAY),
        ] {
            std::fs::write(path, "x").unwrap();
            date(path, now - age);
        }
        assert_eq!(sweep(&s.home, now), Swept { removed: 0, kept: 1 });
        for gone in [&old_index, &old_attributes, &old_empty] {
            assert!(!exists(gone), "{gone:?}");
        }
        assert!(new_index.exists());
        assert!(keep.exists());
    }

    #[cfg(windows)]
    #[test]
    fn temp_files_behind_a_link_are_left() {
        let s = scratch();
        let now = SystemTime::now();
        let here = made(&s.root.join("here"));
        let dir = sandbox(&s.home, &here, now);
        let elsewhere = made(&s.root.join("elsewhere"));
        let temp = elsewhere.join(".x.tmp");
        std::fs::write(&temp, "x").unwrap();
        date(&temp, now - 2 * DAY);
        let info = dir.join("info");
        if !junction(&info, &elsewhere) {
            return;
        }
        assert_eq!(sweep(&s.home, now), Swept { removed: 0, kept: 1 });
        assert!(temp.exists());
        std::fs::remove_dir(&info).unwrap();
    }

    #[test]
    fn a_sandbox_opened_since_the_first_look_stays() {
        let s = scratch();
        let now = SystemTime::now();
        let here = made(&s.root.join("here"));
        let dir = sandbox(&s.home, &here, now - 8 * DAY);
        let name = name_of(&dir);
        let (_, first) = ours(&dir, &name).unwrap();
        assert!(old(first, now), "the first look says unused");
        // It is opened after that look: its file is dated now again.
        date(&dir.join(OWNER_FILE), now);
        assert!(!remove_one(&dir, &name, Why::Unused(now), None));
        assert!(dir.exists());
        // The same call removes it when the file really is old.
        date(&dir.join(OWNER_FILE), now - 8 * DAY);
        assert!(remove_one(&dir, &name, Why::Unused(now), None));
        assert!(!dir.exists());
    }

    /// Outside the tests a removal makes one try at the gate, and a sandbox that is being written stays.
    #[test]
    fn a_busy_gate_is_not_waited_for() {
        let _writing = GATE.read().unwrap_or_else(|e| e.into_inner());
        assert!(try_gate().is_none());
    }

    #[test]
    fn forget_removes_only_that_checkouts_sandbox() {
        let s = scratch();
        let now = SystemTime::now();
        let a = made(&s.root.join("a"));
        let b = made(&s.root.join("b"));
        let (box_a, box_b) = (sandbox(&s.home, &a, now), sandbox(&s.home, &b, now));
        assert!(forget(&s.home, &a));
        assert!(!box_a.exists());
        assert!(box_b.exists());
        assert!(!forget(&s.home, &a));
    }

    #[test]
    fn remove_all_takes_every_sandbox_and_the_folder() {
        let s = scratch();
        let now = SystemTime::now();
        let root = crate::paths::git_sandbox_dir(&s.home);
        for name in ["a", "b"] {
            sandbox(&s.home, &made(&s.root.join(name)), now);
        }
        std::fs::write(root.join("empty-config"), "").unwrap();
        std::fs::write(root.join(".x.tmp"), "x").unwrap();
        assert!(any(&s.home));
        assert_eq!(remove_all(&s.home), 2);
        assert!(!exists(&root));
        assert!(!any(&s.home));
    }

    #[test]
    fn remove_all_leaves_what_is_not_ours() {
        let s = scratch();
        let now = SystemTime::now();
        let root = crate::paths::git_sandbox_dir(&s.home);
        let dir = sandbox(&s.home, &made(&s.root.join("a")), now);
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        assert_eq!(remove_all(&s.home), 1);
        assert!(!dir.exists());
        assert!(root.join("notes.txt").exists());
        assert!(!any(&s.home));
    }
}
