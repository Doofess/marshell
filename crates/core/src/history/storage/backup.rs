//! The backup mirror: an exact copy of `<home>/history` in `<backup folder>/Marshell history/`.
//!
//! This is the one place Marshell deletes files in a folder the user chose, so it is strict. It writes and
//! removes only inside `<backup folder>/Marshell history/`, and only after the marker in that folder names this
//! install. Nothing beside that folder is ever touched. A symbolic link or junction is never followed: the
//! walks use `symlink_metadata`, a link in the source is not copied, and a link in the target is removed as the
//! link it is.

use super::Open;
use crate::{paths, store};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{File, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The folder inside the backup folder that holds the mirror.
pub const TARGET_NAME: &str = "Marshell history";

/// The file in the mirror that names the install it belongs to.
const MARKER: &str = ".marshell-backup.json";

/// Appended to a file's name while it is being copied.
const TMP_SUFFIX: &str = ".marshell-tmp";

/// Two modified times this close count as the same: FAT and some cloud folders keep only every other second.
const TIME_SLACK: Duration = Duration::from_secs(2);

#[derive(Serialize, Deserialize)]
struct Marker {
    install_id: String,
}

/// Why a folder can't be the backup folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRefusal {
    /// It isn't a folder that exists.
    Missing,
    /// It is inside Marshell's own folder, or holds it.
    InsideHome,
    /// Its "Marshell history" folder holds the backup of another install.
    OtherInstall,
    /// It already has a "Marshell history" folder with files in it that Marshell didn't make.
    NotOurs,
    /// The folder couldn't be checked just now (this install's id couldn't be read); trying again may work.
    Unchecked,
}

impl BackupRefusal {
    /// The words the person sees.
    pub fn message(self) -> &'static str {
        match self {
            BackupRefusal::Missing => "That folder doesn't exist.",
            BackupRefusal::InsideHome => "Choose a folder outside Marshell's own folder.",
            BackupRefusal::OtherInstall => "That folder holds another Marshell's backup. Choose another folder.",
            BackupRefusal::Unchecked => "Marshell couldn't check that folder just now. Try again in a minute.",
            BackupRefusal::NotOurs => {
                "That folder already has a \"Marshell history\" folder that Marshell didn't make. Choose another folder."
            }
        }
    }
}

impl std::fmt::Display for BackupRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for BackupRefusal {}

/// What one pass of the mirror did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MirrorReport {
    /// Files copied or replaced.
    pub copied: u32,
    /// Files and links removed from the mirror.
    pub removed: u32,
    /// Bytes copied.
    pub bytes: u64,
}

fn valid_id(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The id in `file`. None when the file is missing or holds something that isn't an id; an error when it can't be
/// read, so an id that is only locked for now isn't replaced.
fn read_id(file: &Path) -> io::Result<Option<String>> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(err) if matches!(err.kind(), io::ErrorKind::NotFound | io::ErrorKind::InvalidData) => return Ok(None),
        Err(err) => return Err(err),
    };
    let text = text.trim();
    Ok(valid_id(text).then(|| text.to_owned()))
}

/// This install's id: 32 hex characters kept in `<home>/history/install-id`, made on first use. A copy of the home
/// folder moved to another PC keeps it; a fresh install gets its own, which is what tells their backups apart.
pub fn install_id(home: &Path) -> anyhow::Result<String> {
    let file = paths::history_dir(home).join("install-id");
    if let Some(id) = read_id(&file).with_context(|| format!("reading {}", file.display()))? {
        return Ok(id);
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|err| anyhow::anyhow!("the OS random number generator failed: {err}"))?;
    let id = hex::encode(bytes);
    std::fs::create_dir_all(paths::history_dir(home))?;
    // Written whole beside the file, then put in place in one step, so a second caller never reads half an id.
    let mut tmp_name = file.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".{}.tmp", &id[..8]));
    let tmp = file.with_file_name(tmp_name);
    store::write_private_file(&tmp, id.as_bytes())?;
    let placed = if std::fs::symlink_metadata(&file).is_ok() {
        // Something unreadable is there: replace it.
        std::fs::rename(&tmp, &file)
    } else {
        // The first to link wins; the other one reads what won.
        match std::fs::hard_link(&tmp, &file) {
            Err(err) if err.kind() != io::ErrorKind::AlreadyExists => std::fs::rename(&tmp, &file),
            _ => Ok(()),
        }
    };
    let _ = std::fs::remove_file(&tmp);
    placed.with_context(|| format!("writing {}", file.display()))?;
    read_id(&file)
        .and_then(|id| id.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not an install id")))
        .with_context(|| format!("reading {}", file.display()))
}

/// Checks `dir` as a backup folder and returns the `Marshell history` path inside its canonical form.
fn checked(home: &Path, dir: &Path) -> Result<PathBuf, BackupRefusal> {
    if !dir.is_absolute() {
        return Err(BackupRefusal::Missing);
    }
    let canon = std::fs::canonicalize(dir).map_err(|_| BackupRefusal::Missing)?;
    if !canon.is_dir() {
        return Err(BackupRefusal::Missing);
    }
    let home_canon = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    if canon.starts_with(&home_canon) || home_canon.starts_with(&canon) {
        return Err(BackupRefusal::InsideHome);
    }
    let target = canon.join(TARGET_NAME);
    match std::fs::symlink_metadata(&target) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(target),
        // A link, a file, or a folder that can't be read.
        Err(_) => return Err(BackupRefusal::NotOurs),
        Ok(meta) if !meta.is_dir() => return Err(BackupRefusal::NotOurs),
        Ok(_) => {}
    }
    let marker = target.join(MARKER);
    match std::fs::symlink_metadata(&marker) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            // Without a marker only an empty folder may become the mirror.
            let empty = std::fs::read_dir(&target).is_ok_and(|mut entries| entries.next().is_none());
            if empty { Ok(target) } else { Err(BackupRefusal::NotOurs) }
        }
        Ok(meta) if meta.is_file() => {
            let theirs = std::fs::read_to_string(&marker)
                .ok()
                .and_then(|text| serde_json::from_str::<Marker>(&text).ok())
                .ok_or(BackupRefusal::NotOurs)?;
            // An id that can't be read can't be shown to match, so the folder is refused, but not as another
            // install's: the read may only have failed for now.
            match install_id(home) {
                Ok(ours) if ours == theirs.install_id => Ok(target),
                Ok(_) => Err(BackupRefusal::OtherInstall),
                Err(_) => Err(BackupRefusal::Unchecked),
            }
        }
        _ => Err(BackupRefusal::NotOurs),
    }
}

/// Whether `dir` may hold the mirror: it exists, is outside the home folder (and doesn't hold it), and its
/// `Marshell history` folder is absent, empty, or marked as this install's.
pub fn check_folder(home: &Path, dir: &Path) -> Result<(), BackupRefusal> {
    checked(home, dir).map(|_| ())
}

/// A file of the history to mirror.
struct Src {
    rel: PathBuf,
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
}

fn is_link_or_other(meta: &Metadata) -> bool {
    !meta.is_file() && !meta.is_dir()
}

/// Every regular file under `root`, by path relative to it, leaving out what is never mirrored: the search index
/// and its sidecars, the install id, and temp files. Links are skipped. A folder that can't be listed is an error:
/// files that merely could not be seen must never read as files that are gone.
fn list_source(root: &Path, rel: &Path, out: &mut Vec<Src>) -> anyhow::Result<()> {
    let dir = root.join(rel);
    for entry in std::fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry.with_context(|| format!("listing {}", dir.display()))?;
        let name = entry.file_name();
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            // Gone since it was listed.
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        let child = rel.join(&name);
        if meta.is_dir() {
            list_source(root, &child, out)?;
            continue;
        }
        if is_link_or_other(&meta) {
            continue;
        }
        let lossy = name.to_string_lossy();
        let top_level = rel.as_os_str().is_empty();
        if lossy.starts_with("index.sqlite")
            || (top_level && lossy == "install-id")
            || lossy.ends_with(".tmp")
            || lossy.ends_with(TMP_SUFFIX)
        {
            continue;
        }
        out.push(Src {
            rel: child,
            path,
            bytes: meta.len(),
            // A time that can't be read counts as just now, so the file waits.
            modified: meta.modified().unwrap_or_else(|_| SystemTime::now()),
        });
    }
    Ok(())
}

/// Removes a file or link, clearing a read-only flag first if that is what stops it. A link goes as itself: a
/// file link with `remove_file`, a folder link (Windows) with `remove_dir`, and what it points at is not touched.
fn remove_entry(path: &Path, meta: &Metadata) -> io::Result<()> {
    if meta.file_type().is_symlink() {
        return std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path));
    }
    match std::fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            let mut permissions = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            std::fs::set_permissions(path, permissions)?;
            std::fs::remove_file(path)
        }
        other => other,
    }
}

/// Removes from the mirror everything that isn't in `keep`, and the folders that end up empty. Returns whether
/// `dir` has nothing left in it. Never descends into a link.
fn clean(root: &Path, rel: &Path, keep: &HashSet<PathBuf>, removed: &mut u32) -> anyhow::Result<bool> {
    let dir = root.join(rel);
    let mut empty = true;
    for entry in std::fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry.with_context(|| format!("listing {}", dir.display()))?;
        let child = rel.join(entry.file_name());
        let path = entry.path();
        if rel.as_os_str().is_empty() && entry.file_name() == MARKER {
            empty = false;
            continue;
        }
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        if meta.is_dir() {
            if clean(root, &child, keep, removed)? {
                match std::fs::remove_dir(&path) {
                    Ok(()) => {}
                    Err(err) => {
                        tracing::warn!("could not remove the backup folder {}: {err}", path.display());
                        empty = false;
                    }
                }
            } else {
                empty = false;
            }
        } else if meta.is_file() && keep.contains(&child) {
            empty = false;
        } else {
            // A stale file, or a link: removed as itself, never followed.
            match remove_entry(&path, &meta) {
                Ok(()) => *removed += 1,
                Err(err) => {
                    tracing::warn!("could not remove {} from the backup: {err}", path.display());
                    empty = false;
                }
            }
        }
    }
    Ok(empty)
}

/// Whether the copy in the mirror already matches the source.
fn up_to_date(dest: &Path, src: &Src) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(dest) else {
        return false;
    };
    if !meta.is_file() || meta.len() != src.bytes {
        return false;
    }
    let Ok(time) = meta.modified() else {
        return false;
    };
    let apart = time.duration_since(src.modified).unwrap_or_else(|err| err.duration());
    apart <= TIME_SLACK
}

/// Makes the folders for `rel`'s parent inside `target`, refusing to pass through a link or a file.
fn ensure_parents(target: &Path, rel: &Path) -> anyhow::Result<()> {
    let mut current = target.to_path_buf();
    for part in rel.parent().into_iter().flat_map(Path::components) {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => anyhow::bail!("{} is in the way", current.display()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => std::fs::create_dir(&current)?,
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// A path that is free or holds a regular file, so a write there can't go through a link.
fn writable_slot(path: &Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => anyhow::bail!("{} is in the way", path.display()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Copies one file: to a temp name, with the source's modified time, then into place.
fn copy_one(target: &Path, src: &Src) -> anyhow::Result<()> {
    ensure_parents(target, &src.rel)?;
    let dest = target.join(&src.rel);
    let mut tmp_name = dest.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(TMP_SUFFIX);
    let tmp = dest.with_file_name(tmp_name);
    writable_slot(&dest)?;
    writable_slot(&tmp)?;
    let result = (|| -> anyhow::Result<()> {
        std::fs::copy(&src.path, &tmp)?;
        File::options().write(true).open(&tmp)?.set_modified(src.modified)?;
        std::fs::rename(&tmp, &dest)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// One pass of the mirror with sets already in hand. See `mirror_with`.
pub fn mirror(home: &Path, dir: &Path, open: &HashSet<PathBuf>, settle: Duration) -> anyhow::Result<MirrorReport> {
    let open = Open {
        recordings: open.clone(),
        tabs: HashSet::new(),
    };
    mirror_with(home, dir, &move || open.clone(), settle, &|| false)
}

/// Makes `<dir>/Marshell history/` an exact copy of `<home>/history`, leaving out the search index, the install id,
/// temp files, the recordings sessions still write and files modified within `settle`. Those are not copied, but a
/// copy already in the mirror stays until the file is really gone. `open` is asked after the history is listed,
/// never before. `stop` is asked between files.
///
/// The folder is checked first, every time: it can have been unplugged, replaced or taken over since it was saved.
pub fn mirror_with(
    home: &Path,
    dir: &Path,
    open: &dyn Fn() -> Open,
    settle: Duration,
    stop: &dyn Fn() -> bool,
) -> anyhow::Result<MirrorReport> {
    let target = checked(home, dir).map_err(anyhow::Error::new)?;
    let id = install_id(home)?;
    match std::fs::symlink_metadata(&target) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir(&target).with_context(|| format!("creating {}", target.display()))?
        }
        Err(err) => return Err(err).with_context(|| format!("reading {}", target.display())),
        Ok(_) => {}
    }
    let marker = target.join(MARKER);
    if std::fs::symlink_metadata(&marker).is_err() {
        let text = serde_json::to_vec_pretty(&Marker { install_id: id })?;
        store::write_private_file(&marker, &text).with_context(|| format!("writing {}", marker.display()))?;
    }

    // The history is listed first and the open set taken second: a recording that finishes in between reads as
    // open and waits for the next pass.
    let mut listed = Vec::new();
    list_source(&paths::history_dir(home), Path::new(""), &mut listed)?;
    let open_names: HashSet<OsString> = open()
        .recordings
        .iter()
        .filter_map(|path| path.file_name().map(OsString::from))
        .collect();
    let keep: HashSet<PathBuf> = listed.iter().map(|file| file.rel.clone()).collect();

    let mut report = MirrorReport::default();
    clean(&target, Path::new(""), &keep, &mut report.removed)?;

    let now = SystemTime::now();
    let mut first_error: Option<anyhow::Error> = None;
    for file in &listed {
        if stop() {
            return Ok(report);
        }
        let name = file.path.file_name().map(OsString::from).unwrap_or_default();
        // A time in the future counts as recent.
        let settled = now.duration_since(file.modified).is_ok_and(|age| age >= settle);
        if !settled || open_names.contains(&name) || file.rel.as_os_str() == MARKER {
            continue;
        }
        if up_to_date(&target.join(&file.rel), file) {
            continue;
        }
        match copy_one(&target, file) {
            Ok(()) => {
                report.copied += 1;
                report.bytes += file.bytes;
            }
            // Removed since it was listed: the next pass takes it out of the mirror.
            Err(_) if !file.path.exists() => {}
            Err(err) => {
                let err = err.context(format!("copying {}", file.rel.display()));
                tracing::warn!("{err:#}");
                first_error.get_or_insert(err);
            }
        }
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(report),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_AGO: u64 = 1000;

    fn history_file(home: &Path, rel: &str, body: &[u8], age_secs: u64) -> PathBuf {
        let path = paths::history_dir(home).join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, body).unwrap();
        set_age(&path, age_secs);
        path
    }

    fn set_age(path: &Path, age_secs: u64) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(age_secs))
            .unwrap();
    }

    fn mirrored(dir: &Path, rel: &str) -> PathBuf {
        dir.join(TARGET_NAME).join(rel)
    }

    fn run(home: &Path, dir: &Path) -> anyhow::Result<MirrorReport> {
        mirror(home, dir, &HashSet::new(), Duration::ZERO)
    }

    #[test]
    fn mirror_copies_new_and_changed_files_and_keeps_times() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let rec = history_file(home.path(), "recordings/t1-1000.cast.zst", b"recording", NOW_AGO);
        let side = history_file(home.path(), "archive/claude/a.json", b"sidecar", NOW_AGO);

        let report = run(home.path(), backup.path()).unwrap();
        assert_eq!((report.copied, report.removed, report.bytes), (2, 0, 16));
        let copy = mirrored(backup.path(), "recordings/t1-1000.cast.zst");
        assert_eq!(std::fs::read(&copy).unwrap(), b"recording");
        assert_eq!(
            std::fs::read(mirrored(backup.path(), "archive/claude/a.json")).unwrap(),
            b"sidecar"
        );
        let (a, b) = (
            std::fs::metadata(&rec).unwrap().modified().unwrap(),
            std::fs::metadata(&copy).unwrap().modified().unwrap(),
        );
        assert!(a.duration_since(b).unwrap_or_else(|e| e.duration()) < Duration::from_millis(5));
        // The marker names this install.
        let marker: Marker = serde_json::from_slice(&std::fs::read(mirrored(backup.path(), MARKER)).unwrap()).unwrap();
        assert_eq!(marker.install_id, install_id(home.path()).unwrap());

        // Nothing changed: nothing is copied.
        let report = run(home.path(), backup.path()).unwrap();
        assert_eq!((report.copied, report.removed), (0, 0));

        // A different size is copied; so is the same size at another time.
        std::fs::write(&side, b"sidecar, longer").unwrap();
        set_age(&side, NOW_AGO);
        std::fs::write(&rec, b"RECORDING").unwrap();
        set_age(&rec, NOW_AGO - 500);
        let report = run(home.path(), backup.path()).unwrap();
        assert_eq!(report.copied, 2);
        assert_eq!(std::fs::read(&copy).unwrap(), b"RECORDING");
        assert_eq!(
            std::fs::read(mirrored(backup.path(), "archive/claude/a.json")).unwrap(),
            b"sidecar, longer"
        );
    }

    #[test]
    fn mirror_removes_what_history_no_longer_has() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let rec = history_file(home.path(), "recordings/t1-1000.cast.zst", b"recording", NOW_AGO);
        history_file(home.path(), "archive/claude/keep.json", b"keep", NOW_AGO);
        let gone = [
            history_file(home.path(), "archive/claude/gone.jsonl.zst", b"transcript", NOW_AGO),
            history_file(home.path(), "archive/claude/gone.json", b"sidecar", NOW_AGO),
            history_file(home.path(), "archive/claude/gone.marks.json", b"marks", NOW_AGO),
        ];
        run(home.path(), backup.path()).unwrap();
        assert!(mirrored(backup.path(), "archive/claude/gone.json").exists());

        std::fs::remove_file(&rec).unwrap();
        for file in &gone {
            std::fs::remove_file(file).unwrap();
        }
        let report = run(home.path(), backup.path()).unwrap();
        assert_eq!((report.copied, report.removed), (0, 4));
        assert!(!mirrored(backup.path(), "recordings/t1-1000.cast.zst").exists());
        // The folder that emptied went with it.
        assert!(!mirrored(backup.path(), "recordings").exists());
        assert!(mirrored(backup.path(), "archive/claude/keep.json").exists());
        assert!(!mirrored(backup.path(), "archive/claude/gone.json").exists());
        // Stray files in the mirror go too, and the marker stays.
        std::fs::write(mirrored(backup.path(), "stray.txt"), b"x").unwrap();
        std::fs::write(mirrored(backup.path(), "archive/claude/half.json.marshell-tmp"), b"x").unwrap();
        let report = run(home.path(), backup.path()).unwrap();
        assert_eq!(report.removed, 2);
        assert!(mirrored(backup.path(), MARKER).exists());
    }

    #[test]
    fn mirror_leaves_out_the_index_open_recordings_and_temp_files() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        history_file(home.path(), "index.sqlite", b"db", NOW_AGO);
        history_file(home.path(), "index.sqlite-wal", b"wal", NOW_AGO);
        history_file(home.path(), "index.sqlite-shm", b"shm", NOW_AGO);
        history_file(home.path(), "install-id", b"0123456789abcdef0123456789abcdef", NOW_AGO);
        history_file(home.path(), "archive/claude/x.json.tmp", b"tmp", NOW_AGO);
        let open = history_file(home.path(), "recordings/t1-1000.cast", b"open", NOW_AGO);
        history_file(home.path(), "recordings/t2-2000.cast", b"finished", NOW_AGO);
        history_file(home.path(), "recordings/t3-3000.cast", b"recent", 0);
        history_file(home.path(), "archive/claude/ok.json", b"ok", NOW_AGO);

        let report = mirror(home.path(), backup.path(), &[open].into(), Duration::from_secs(10)).unwrap();
        assert_eq!(report.copied, 2);
        let target = backup.path().join(TARGET_NAME);
        let mut found: Vec<String> = Vec::new();
        fn walk(dir: &Path, base: &Path, found: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                if entry.path().is_dir() {
                    walk(&entry.path(), base, found);
                } else {
                    found.push(
                        entry
                            .path()
                            .strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        walk(&target, &target, &mut found);
        found.sort();
        assert_eq!(
            found,
            [
                ".marshell-backup.json",
                "archive/claude/ok.json",
                "recordings/t2-2000.cast"
            ]
        );

        // A copy already there stays while its file is open or recent, and goes once the file is gone.
        let ok = paths::history_dir(home.path()).join("archive/claude/ok.json");
        set_age(&ok, 0);
        std::fs::write(&ok, b"ok, changed").unwrap();
        let report = mirror(home.path(), backup.path(), &HashSet::new(), Duration::from_secs(10)).unwrap();
        assert_eq!(report.removed, 0);
        assert_eq!(
            std::fs::read(mirrored(backup.path(), "archive/claude/ok.json")).unwrap(),
            b"ok"
        );
        std::fs::remove_file(&ok).unwrap();
        mirror(home.path(), backup.path(), &HashSet::new(), Duration::from_secs(10)).unwrap();
        assert!(!mirrored(backup.path(), "archive/claude/ok.json").exists());
    }

    #[test]
    fn mirror_never_touches_anything_beside_its_target() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let beside = backup.path().join("holiday.jpg");
        std::fs::write(&beside, b"photo").unwrap();
        std::fs::create_dir(backup.path().join("Other")).unwrap();
        std::fs::write(backup.path().join("Other").join("recordings"), b"mine").unwrap();
        let rec = history_file(home.path(), "recordings/t1-1000.cast.zst", b"recording", NOW_AGO);
        let survives = || {
            assert_eq!(std::fs::read(&beside).unwrap(), b"photo");
            assert_eq!(
                std::fs::read(backup.path().join("Other").join("recordings")).unwrap(),
                b"mine"
            );
        };
        run(home.path(), backup.path()).unwrap();
        survives();
        std::fs::remove_file(&rec).unwrap();
        run(home.path(), backup.path()).unwrap();
        survives();
        // An empty history empties the mirror and nothing else.
        run(home.path(), backup.path()).unwrap();
        survives();
    }

    #[test]
    fn check_folder_refuses_home_inside_and_outside() {
        let outer = tempfile::tempdir().unwrap();
        let home = outer.path().join("home");
        std::fs::create_dir_all(home.join("history")).unwrap();
        std::fs::create_dir(home.join("sub")).unwrap();
        assert_eq!(check_folder(&home, &home), Err(BackupRefusal::InsideHome));
        assert_eq!(check_folder(&home, &home.join("sub")), Err(BackupRefusal::InsideHome));
        assert_eq!(check_folder(&home, outer.path()), Err(BackupRefusal::InsideHome));
        let elsewhere = tempfile::tempdir().unwrap();
        assert_eq!(check_folder(&home, elsewhere.path()), Ok(()));
        assert_eq!(
            BackupRefusal::InsideHome.message(),
            "Choose a folder outside Marshell's own folder."
        );
    }

    #[test]
    fn check_folder_refuses_another_installs_backup() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        history_file(home.path(), "recordings/t1-1000.cast.zst", b"recording", NOW_AGO);
        let target = backup.path().join(TARGET_NAME);
        std::fs::create_dir_all(target.join("recordings")).unwrap();
        std::fs::write(
            target.join(MARKER),
            r#"{"install_id":"00000000000000000000000000000000"}"#,
        )
        .unwrap();
        let theirs = target.join("recordings").join("t9-9.cast.zst");
        std::fs::write(&theirs, b"theirs").unwrap();

        assert_eq!(
            check_folder(home.path(), backup.path()),
            Err(BackupRefusal::OtherInstall)
        );
        assert!(run(home.path(), backup.path()).is_err());
        assert_eq!(std::fs::read(&theirs).unwrap(), b"theirs");
        assert!(!target.join("recordings").join("t1-1000.cast.zst").exists());
        assert_eq!(
            BackupRefusal::OtherInstall.message(),
            "That folder holds another Marshell's backup. Choose another folder."
        );

        // The folder this install made is accepted again, also after a pass of its own.
        let own = tempfile::tempdir().unwrap();
        run(home.path(), own.path()).unwrap();
        assert_eq!(check_folder(home.path(), own.path()), Ok(()));
        run(home.path(), own.path()).unwrap();
    }

    #[test]
    fn check_folder_refuses_an_unmarked_folder_with_files() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        history_file(home.path(), "a.json", b"a", NOW_AGO);
        let target = backup.path().join(TARGET_NAME);
        std::fs::create_dir(&target).unwrap();
        // Empty is fine: nothing of anyone's is there.
        assert_eq!(check_folder(home.path(), backup.path()), Ok(()));
        std::fs::write(target.join("notes.txt"), b"mine").unwrap();
        assert_eq!(check_folder(home.path(), backup.path()), Err(BackupRefusal::NotOurs));
        assert!(run(home.path(), backup.path()).is_err());
        assert_eq!(std::fs::read(target.join("notes.txt")).unwrap(), b"mine");
        // A marker that isn't one is no marker.
        std::fs::write(target.join(MARKER), b"not json").unwrap();
        assert_eq!(check_folder(home.path(), backup.path()), Err(BackupRefusal::NotOurs));
        assert_eq!(
            BackupRefusal::NotOurs.message(),
            "That folder already has a \"Marshell history\" folder that Marshell didn't make. Choose another folder."
        );
    }

    #[test]
    fn check_folder_refuses_a_missing_folder() {
        let home = tempfile::tempdir().unwrap();
        let outer = tempfile::tempdir().unwrap();
        assert_eq!(
            check_folder(home.path(), &outer.path().join("nope")),
            Err(BackupRefusal::Missing)
        );
        let file = outer.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(check_folder(home.path(), &file), Err(BackupRefusal::Missing));
        assert_eq!(
            check_folder(home.path(), Path::new("relative")),
            Err(BackupRefusal::Missing)
        );
        assert!(run(home.path(), &outer.path().join("nope")).is_err());
        assert_eq!(BackupRefusal::Missing.message(), "That folder doesn't exist.");
    }

    #[test]
    fn a_missing_history_folder_removes_nothing_from_the_mirror() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        history_file(home.path(), "a.json", b"a", NOW_AGO);
        run(home.path(), backup.path()).unwrap();
        std::fs::remove_dir_all(paths::history_dir(home.path())).unwrap();
        assert!(run(home.path(), backup.path()).is_err());
        assert!(mirrored(backup.path(), "a.json").exists());
    }

    #[test]
    fn the_install_id_is_made_once_and_kept() {
        let home = tempfile::tempdir().unwrap();
        let id = install_id(home.path()).unwrap();
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(install_id(home.path()).unwrap(), id);
        assert_ne!(install_id(tempfile::tempdir().unwrap().path()).unwrap(), id);
    }

    #[cfg(windows)]
    #[test]
    fn an_install_id_that_cannot_be_read_is_not_replaced() {
        use std::os::windows::fs::OpenOptionsExt;
        let home = tempfile::tempdir().unwrap();
        let id = install_id(home.path()).unwrap();
        let file = paths::history_dir(home.path()).join("install-id");
        // Another process holds the file and shares only delete (FILE_SHARE_DELETE): reading it fails, though it
        // holds a good id, and replacing it would work. With no sharing at all the replacing would fail too.
        for share in [0, 4] {
            let held = File::options().read(true).share_mode(share).open(&file).unwrap();
            assert!(install_id(home.path()).is_err(), "share mode {share}");
            drop(held);
        }
        assert_eq!(install_id(home.path()).unwrap(), id);
        std::fs::write(&file, b"not an id").unwrap();
        assert_ne!(install_id(home.path()).unwrap(), id, "an invalid id is still replaced");
    }

    #[cfg(windows)]
    #[test]
    fn an_install_id_that_cannot_be_read_just_now_is_not_another_installs_backup() {
        use std::os::windows::fs::OpenOptionsExt;
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let id = install_id(home.path()).unwrap();
        let target = backup.path().join(TARGET_NAME);
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join(MARKER), format!(r#"{{"install_id":"{id}"}}"#)).unwrap();
        let file = paths::history_dir(home.path()).join("install-id");

        // The id file is held, so it can't be read: that is not a different install's folder.
        let held = File::options().read(true).share_mode(4).open(&file).unwrap();
        assert_eq!(check_folder(home.path(), backup.path()), Err(BackupRefusal::Unchecked));
        drop(held);
        assert_eq!(check_folder(home.path(), backup.path()), Ok(()));
        assert_eq!(
            BackupRefusal::Unchecked.message(),
            "Marshell couldn't check that folder just now. Try again in a minute."
        );
    }

    #[cfg(unix)]
    #[test]
    fn mirror_does_not_follow_links() {
        use std::os::unix::fs::symlink;
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        history_file(home.path(), "a.json", b"a", NOW_AGO);
        // A link to a folder and to a file in the source: neither is copied.
        symlink(outside.path(), paths::history_dir(home.path()).join("linked")).unwrap();
        symlink(
            outside.path().join("secret.txt"),
            paths::history_dir(home.path()).join("secret-link.txt"),
        )
        .unwrap();
        run(home.path(), backup.path()).unwrap();
        assert!(!mirrored(backup.path(), "linked").exists());
        assert!(!mirrored(backup.path(), "secret-link.txt").exists());
        // A link in the mirror is removed as a link; what it points at survives.
        symlink(outside.path(), mirrored(backup.path(), "trap")).unwrap();
        run(home.path(), backup.path()).unwrap();
        assert!(std::fs::symlink_metadata(mirrored(backup.path(), "trap")).is_err());
        assert_eq!(std::fs::read(outside.path().join("secret.txt")).unwrap(), b"secret");
    }
}
