//! Compresses finished plain recordings: `<name>.cast` becomes `<name>.cast.zst` and the plain file goes.
//! Readers resolve a recording's `.cast` name to whichever form is on disk (`cast::resolve`), so nothing that
//! links a recording needs to change.

use crate::history::cast::{started_ms, valid_recording_name};
use anyhow::Context;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The zstd level recordings are compressed at.
const LEVEL: i32 = 3;

/// A plain recording seen in a folder listing, with its modified time at that moment.
pub struct Listed {
    path: PathBuf,
    started_ms: u64,
    modified: SystemTime,
}

/// The plain `.cast` files in `dir` with valid names. Links and other kinds of entries are skipped. List the
/// folder before taking the set of open recordings, so a file that finishes in between is not read as open.
pub fn list_plain(dir: &Path) -> Vec<Listed> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut listed = Vec::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(started_ms) = started_ms(&name) else {
            continue;
        };
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        // A time that can't be read counts as just now, so the file waits.
        let modified = meta.modified().unwrap_or_else(|_| SystemTime::now());
        listed.push(Listed {
            path,
            started_ms,
            modified,
        });
    }
    listed
}

/// From a listing, the files that are not open and have not been modified within `settle` of `now`, oldest
/// first by the start time in the name.
pub fn select_finished(
    listed: Vec<Listed>,
    open: &HashSet<PathBuf>,
    now: SystemTime,
    settle: Duration,
) -> Vec<PathBuf> {
    let open_names: HashSet<OsString> = open.iter().filter_map(|p| p.file_name().map(OsString::from)).collect();
    let mut finished: Vec<Listed> = listed
        .into_iter()
        .filter(|file| {
            let open = file.path.file_name().is_some_and(|name| open_names.contains(name));
            // A modified time in the future also counts as recent.
            let settled = now.duration_since(file.modified).is_ok_and(|age| age >= settle);
            !open && settled
        })
        .collect();
    finished.sort_by(|a, b| (a.started_ms, &a.path).cmp(&(b.started_ms, &b.path)));
    finished.into_iter().map(|file| file.path).collect()
}

/// The plain `.cast` files in `dir` with valid names that are not in `open`, not modified within `settle`
/// of `now`, oldest first. The storage thread lists and takes `open` in two steps; this is both at once.
pub fn finished_plain(dir: &Path, open: &HashSet<PathBuf>, now: SystemTime, settle: Duration) -> Vec<PathBuf> {
    select_finished(list_plain(dir), open, now, settle)
}

/// `<path>.zst`.
fn zst_of(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".zst");
    PathBuf::from(name)
}

/// Compresses the plain recording at `path` and removes it. Returns the bytes saved. The compressed file is
/// written beside it as `.zst.tmp`, synced and renamed, so a `.zst` on disk is always complete before the
/// plain file goes. A plain file with a `.zst` beside it is removed without encoding.
pub fn compress(path: &Path) -> anyhow::Result<u64> {
    let valid = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(valid_recording_name);
    anyhow::ensure!(valid, "not a recording: {}", path.display());
    let zst = zst_of(path);
    let plain_len = std::fs::metadata(path)
        .with_context(|| format!("could not read {}", path.display()))?
        .len();
    if std::fs::symlink_metadata(&zst).is_ok_and(|meta| meta.is_file()) {
        let saved = plain_len.saturating_sub(std::fs::metadata(&zst).map(|m| m.len()).unwrap_or(0));
        remove_plain(path);
        return Ok(saved);
    }
    let mut tmp_name = zst.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let written = encode(path, &tmp)
        .and_then(|()| std::fs::rename(&tmp, &zst).with_context(|| format!("could not rename {}", tmp.display())));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    let zst_len = std::fs::metadata(&zst).map(|m| m.len()).unwrap_or(0);
    remove_plain(path);
    Ok(plain_len.saturating_sub(zst_len))
}

/// Streams `from` through zstd into a new file at `to` and syncs it.
fn encode(from: &Path, to: &Path) -> anyhow::Result<()> {
    let input = File::open(from).with_context(|| format!("could not open {}", from.display()))?;
    let mut output = File::create(to).with_context(|| format!("could not create {}", to.display()))?;
    let modified = input.metadata().and_then(|meta| meta.modified());
    zstd::stream::copy_encode(BufReader::new(input), &mut output, LEVEL)
        .with_context(|| format!("could not compress {}", from.display()))?;
    // Retention dates a recording by its modified time: the compressed copy keeps the plain one's.
    if let Ok(modified) = modified {
        output
            .set_modified(modified)
            .with_context(|| format!("could not date {}", to.display()))?;
    }
    output
        .sync_all()
        .with_context(|| format!("could not sync {}", to.display()))
}

/// Removes the plain file once its `.zst` is complete. A failure is logged; the next tick finds the plain file
/// beside its `.zst` and removes it.
fn remove_plain(path: &Path) {
    if let Err(err) = std::fs::remove_file(path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("could not remove {} after compressing it: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn write_old(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        let file = File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
        path
    }

    #[test]
    fn finished_plain_skips_open_recent_and_compressed() {
        let dir = tempfile::tempdir().unwrap();
        let open = write_old(dir.path(), "t1-1000.cast", b"a");
        std::fs::write(dir.path().join("t2-2000.cast"), b"b").unwrap();
        write_old(dir.path(), "t3-3000.cast.zst", b"c");
        let old = write_old(dir.path(), "t4-4000.cast", b"d");
        let older = write_old(dir.path(), "t5-500.cast", b"e");
        write_old(dir.path(), "notes.txt", b"f");
        let open_set: HashSet<PathBuf> = [open].into();
        let found = finished_plain(dir.path(), &open_set, SystemTime::now(), Duration::from_secs(10));
        assert_eq!(found, vec![older, old]);
    }

    #[test]
    fn compress_round_trips_and_removes_the_plain_file() {
        let dir = tempfile::tempdir().unwrap();
        let body = "{\"version\":2}\n[0.1,\"o\",\"hello\"]\n".repeat(500).into_bytes();
        let plain = write_old(dir.path(), "t1-1000.cast", &body);
        let saved = compress(&plain).unwrap();
        let zst = dir.path().join("t1-1000.cast.zst");
        assert!(!plain.exists());
        assert_eq!(zstd::decode_all(File::open(&zst).unwrap()).unwrap(), body);
        assert_eq!(saved, body.len() as u64 - std::fs::metadata(&zst).unwrap().len());
        assert!(!dir.path().join("t1-1000.cast.zst.tmp").exists());
    }

    #[test]
    fn compress_keeps_the_recordings_modified_time() {
        // Retention dates a recording by its modified time, so compressing it must not make it new.
        let dir = tempfile::tempdir().unwrap();
        let plain = write_old(dir.path(), "t1-1000.cast", b"{\"version\":2}\n");
        let then = std::fs::metadata(&plain).unwrap().modified().unwrap();
        compress(&plain).unwrap();
        let zst = std::fs::metadata(dir.path().join("t1-1000.cast.zst")).unwrap();
        assert_eq!(zst.modified().unwrap(), then);
    }

    #[test]
    fn a_plain_file_left_beside_its_zst_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let plain = write_old(dir.path(), "t1-1000.cast", b"plain");
        let zst = dir.path().join("t1-1000.cast.zst");
        let kept = zstd::encode_all(&b"plain"[..], 3).unwrap();
        std::fs::write(&zst, &kept).unwrap();
        compress(&plain).unwrap();
        assert!(!plain.exists());
        assert_eq!(std::fs::read(&zst).unwrap(), kept, "the existing .zst is not rewritten");
    }

    #[test]
    fn compress_succeeds_while_a_reader_holds_the_plain_file() {
        let dir = tempfile::tempdir().unwrap();
        let body = "[0.5,\"o\",\"abc\"]\n".repeat(2000).into_bytes();
        let plain = write_old(dir.path(), "t1-1000.cast", &body);
        let mut reader = File::open(&plain).unwrap();
        let mut first = [0u8; 10];
        reader.read_exact(&mut first).unwrap();
        compress(&plain).unwrap();
        assert!(!plain.exists());
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        assert_eq!([&first[..], &rest[..]].concat(), body);
    }
}
