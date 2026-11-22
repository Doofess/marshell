//! The files a text tab's save replaced: before a save writes, the old bytes are copied to
//! `<home>/file-history/<id>.bin` and listed in `file-history/index.json`. The last `KEEP_PER_FILE`
//! copies of each file are kept, and `KEEP_BYTES` in all. They are kept apart from the config
//! service's snapshots, so the Doctor's undo list shows only config changes.

use crate::auth::new_id;
use crate::projects::same_path;
use crate::store::{self, Versioned};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Copies kept per file; a save past this drops the file's oldest.
pub const KEEP_PER_FILE: usize = 20;
/// Bytes of copies kept in all; a save past this drops the oldest of any file.
pub const KEEP_BYTES: u64 = 256 * 1024 * 1024;

/// One read-modify-write of the index at a time.
static INDEX_LOCK: Mutex<()> = Mutex::new(());

/// What one save replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub path: PathBuf,
    /// The file's bytes before the save.
    pub before: Vec<u8>,
    /// The hash of what the save wrote: an undo runs only while the file still holds it.
    pub after_hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    id: String,
    path: String,
    created_ms: u64,
    after_hash: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    #[serde(default)]
    saves: Vec<Entry>,
}

impl Versioned for Index {
    const VERSION: u32 = 1;
}

/// `f` followed by 12 lowercase hex characters, so an id can never name a file outside the folder.
fn valid_id(id: &str) -> bool {
    id.len() == 13
        && id.starts_with('f')
        && id[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub struct History {
    dir: PathBuf,
    keep_bytes: u64,
}

impl History {
    pub fn new(home: &Path) -> Self {
        Self {
            dir: crate::paths::file_history_dir(home),
            keep_bytes: KEEP_BYTES,
        }
    }

    /// A store that keeps `keep_bytes` in all, for the tests of that limit.
    #[cfg(test)]
    fn with_limit(home: &Path, keep_bytes: u64) -> Self {
        Self {
            keep_bytes,
            ..Self::new(home)
        }
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn blob_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.bin"))
    }

    /// Copies `before`, the bytes a save is about to replace at `path`, and returns the copy's id.
    /// The copy is on disk before the index names it, and the caller writes the file only after
    /// this returns. Older copies past the two limits go, never the one just taken.
    pub fn take(&self, path: &Path, before: &[u8], after_hash: &str, now_ms: u64) -> anyhow::Result<String> {
        let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let id = new_id("f");
        store::write_bytes_atomic(&self.blob_path(&id), before)?;
        let result = self.list(&id, path, before.len() as u64, after_hash, now_ms);
        if result.is_err() {
            let _ = std::fs::remove_file(self.blob_path(&id));
        }
        result.map(|()| id)
    }

    /// Adds the entry to the index, trims it, saves it, and only then removes the dropped copies.
    fn list(&self, id: &str, path: &Path, size: u64, after_hash: &str, now_ms: u64) -> anyhow::Result<()> {
        let mut index = store::read::<Index>(&self.index_path())?.unwrap_or_default();
        let path = path.to_string_lossy().into_owned();
        index.saves.push(Entry {
            id: id.to_string(),
            path: path.clone(),
            created_ms: now_ms,
            after_hash: after_hash.to_string(),
            size,
        });
        // The index is in the order the saves were taken, so the front is the oldest.
        let mut dropped = Vec::new();
        let of_file = index.saves.iter().filter(|e| same_path(&e.path, &path)).count();
        let mut extra = of_file.saturating_sub(KEEP_PER_FILE);
        index.saves.retain(|e| {
            let drop = extra > 0 && e.id != id && same_path(&e.path, &path);
            if drop {
                extra -= 1;
                dropped.push(e.id.clone());
            }
            !drop
        });
        let mut total: u64 = index.saves.iter().map(|e| e.size).sum();
        index.saves.retain(|e| {
            let drop = total > self.keep_bytes && e.id != id;
            if drop {
                total -= e.size;
                dropped.push(e.id.clone());
            }
            !drop
        });
        store::write(&self.index_path(), &index).context("saving the file history's index")?;
        for id in dropped {
            let _ = std::fs::remove_file(self.blob_path(&id));
        }
        Ok(())
    }

    /// When the newest copy of `path` was taken, None when the file has no copy. Only the index is read, under its lock, which is
    /// not held past the read.
    pub fn newest_ms(&self, path: &Path) -> anyhow::Result<Option<u64>> {
        let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let index = store::read::<Index>(&self.index_path())?.unwrap_or_default();
        let path = path.to_string_lossy();
        Ok(index
            .saves
            .iter()
            .filter(|e| same_path(&e.path, &path))
            .map(|e| e.created_ms)
            .max())
    }

    /// What the save `id` replaced. None for an id that is malformed, unknown or whose copy is gone.
    pub fn get(&self, id: &str) -> anyhow::Result<Option<Saved>> {
        if !valid_id(id) {
            return Ok(None);
        }
        let _guard = INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let index = store::read::<Index>(&self.index_path())?.unwrap_or_default();
        let Some(entry) = index.saves.into_iter().find(|e| e.id == id) else {
            return Ok(None);
        };
        match std::fs::read(self.blob_path(id)) {
            Ok(before) => Ok(Some(Saved {
                path: PathBuf::from(entry.path),
                before,
                after_hash: entry.after_hash,
            })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading the copy {id}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a() -> PathBuf {
        PathBuf::from(if cfg!(windows) { r"C:\proj\a.txt" } else { "/proj/a.txt" })
    }

    fn b() -> PathBuf {
        PathBuf::from(if cfg!(windows) { r"C:\proj\b.txt" } else { "/proj/b.txt" })
    }

    #[test]
    fn take_and_read_back() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        let id = history.take(&a(), b"old", "hash-of-new", 1).unwrap();
        let saved = history.get(&id).unwrap().unwrap();
        assert_eq!(saved.path, a());
        assert_eq!(saved.before, b"old");
        assert_eq!(saved.after_hash, "hash-of-new");
    }

    #[test]
    fn keeps_the_last_20_per_file() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        let ids: Vec<String> = (0..21u64)
            .map(|n| history.take(&a(), format!("v{n}").as_bytes(), "h", n).unwrap())
            .collect();
        assert!(history.get(&ids[0]).unwrap().is_none());
        for (n, id) in ids.iter().enumerate().skip(1) {
            assert_eq!(history.get(id).unwrap().unwrap().before, format!("v{n}").as_bytes());
        }
        assert!(!history.dir.join(format!("{}.bin", ids[0])).exists());
    }

    #[test]
    fn another_files_snapshots_are_not_counted() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        let first = history.take(&a(), b"a0", "h", 0).unwrap();
        for n in 1..=20u64 {
            history.take(&b(), b"b", "h", n).unwrap();
        }
        assert_eq!(history.get(&first).unwrap().unwrap().before, b"a0");
    }

    #[test]
    fn drops_the_oldest_past_the_byte_limit() {
        let home = tempfile::tempdir().unwrap();
        let history = History::with_limit(home.path(), 10);
        let first = history.take(&a(), b"1111", "h", 1).unwrap();
        let second = history.take(&b(), b"2222", "h", 2).unwrap();
        let third = history.take(&a(), b"3333", "h", 3).unwrap();
        assert!(history.get(&first).unwrap().is_none());
        assert_eq!(history.get(&second).unwrap().unwrap().before, b"2222");
        assert_eq!(history.get(&third).unwrap().unwrap().before, b"3333");
    }

    #[test]
    fn the_newest_is_never_dropped() {
        let home = tempfile::tempdir().unwrap();
        let history = History::with_limit(home.path(), 10);
        let id = history.take(&a(), &[7u8; 20], "h", 1).unwrap();
        assert_eq!(history.get(&id).unwrap().unwrap().before, vec![7u8; 20]);
    }

    #[test]
    fn unknown_and_malformed_ids_are_none() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        history.take(&a(), b"old", "h", 1).unwrap();
        for id in ["", "f123", "s0123456789ab", "f0123456789aZ", "../x", "f0123456789ab"] {
            assert!(history.get(id).unwrap().is_none(), "{id:?}");
        }
    }

    #[test]
    fn an_empty_file_is_a_snapshot() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        let id = history.take(&a(), b"", "h", 1).unwrap();
        assert_eq!(history.get(&id).unwrap().unwrap().before, Vec::<u8>::new());
    }

    #[test]
    fn newest_ms_reads_the_newest_copy_of_that_file_only() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        assert_eq!(history.newest_ms(&a()).unwrap(), None);
        history.take(&a(), b"a1", "h", 100).unwrap();
        history.take(&b(), b"b1", "h", 500).unwrap();
        history.take(&a(), b"a2", "h", 300).unwrap();
        assert_eq!(history.newest_ms(&a()).unwrap(), Some(300));
        assert_eq!(history.newest_ms(&b()).unwrap(), Some(500));
        let c = PathBuf::from(if cfg!(windows) { r"C:\proj\c.txt" } else { "/proj/c.txt" });
        assert_eq!(history.newest_ms(&c).unwrap(), None);
    }

    #[test]
    fn newest_ms_matches_the_path_as_the_os_compares_it() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        history.take(&a(), b"x", "h", 7).unwrap();
        let upper = PathBuf::from(a().to_string_lossy().to_uppercase());
        assert_eq!(history.newest_ms(&upper).unwrap().is_some(), cfg!(windows));
    }

    #[test]
    fn a_path_matches_as_the_os_compares_it() {
        let home = tempfile::tempdir().unwrap();
        let history = History::new(home.path());
        let lower = a();
        let upper = PathBuf::from(a().to_string_lossy().to_uppercase());
        let ids: Vec<String> = (0..21u64)
            .map(|n| {
                history
                    .take(if n % 2 == 0 { &lower } else { &upper }, b"x", "h", n)
                    .unwrap()
            })
            .collect();
        // Windows compares without case, so the two spellings are one file there and two elsewhere.
        assert_eq!(history.get(&ids[0]).unwrap().is_none(), cfg!(windows));
    }
}
