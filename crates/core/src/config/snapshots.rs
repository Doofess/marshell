//! Restore points: before a ChangeSet writes a file, the old bytes are copied to
//! `<home>/snapshots/<id>/<n>.bin` and listed in `snapshots/index.json`. The last
//! `KEEP_PER_FILE` copies of each file are kept.

use crate::auth::new_id;
use crate::config::hash::hash_bytes;
use crate::projects::same_path;
use crate::store::{self, Versioned};
use anyhow::Context;
use marshell_protocol::config::{Applies, ConfigScope, SnapshotFileInfo, SnapshotInfo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Copies kept per file; older ones are dropped when a new snapshot is taken.
pub const KEEP_PER_FILE: usize = 20;

/// One read-modify-write of the index at a time.
static INDEX_LOCK: Mutex<()> = Mutex::new(());

/// A file about to change: what it holds now, and the hash of what will be written.
pub struct SnapshotInput {
    pub path: PathBuf,
    /// The current bytes, or `None` when the file does not exist yet.
    pub before: Option<Vec<u8>>,
    /// Hash of the bytes about to be written, `None` when the file is being removed.
    pub after_hash: Option<String>,
    pub scope: ConfigScope,
    pub applies: Applies,
}

#[derive(Serialize, Deserialize, Clone)]
struct FileEntry {
    #[serde(flatten)]
    info: SnapshotFileInfo,
    /// The blob number in the snapshot's folder; absent when the file did not exist before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blob: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    id: String,
    created_ms: u64,
    changeset_id: String,
    title: String,
    files: Vec<FileEntry>,
}

impl Entry {
    fn info(&self) -> SnapshotInfo {
        SnapshotInfo {
            id: self.id.clone(),
            created_ms: self.created_ms,
            changeset_id: self.changeset_id.clone(),
            title: self.title.clone(),
            files: self.files.iter().map(|f| f.info.clone()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    #[serde(default)]
    snapshots: Vec<Entry>,
}

impl Versioned for Index {
    const VERSION: u32 = 1;
}

/// `s` followed by 12 lowercase hex characters, so an id can never name another folder.
fn valid_id(id: &str) -> bool {
    id.len() == 13
        && id.starts_with('s')
        && id[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub struct Snapshots {
    dir: PathBuf,
}

impl Snapshots {
    pub fn new(home: &Path) -> Self {
        Self {
            dir: crate::paths::snapshots_dir(home),
        }
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn load(&self) -> anyhow::Result<Index> {
        Ok(store::read::<Index>(&self.index_path())?.unwrap_or_default())
    }

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Saves the old bytes of every file and returns the new snapshot's id.
    pub fn take(
        &self,
        changeset_id: &str,
        title: &str,
        files: &[SnapshotInput],
        now_ms: u64,
    ) -> anyhow::Result<String> {
        let _guard = Self::lock();
        let id = new_id("s");
        let folder = self.dir.join(&id);
        let result = self.take_locked(&id, &folder, changeset_id, title, files, now_ms);
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&folder);
        }
        result.map(|()| id)
    }

    fn take_locked(
        &self,
        id: &str,
        folder: &Path,
        changeset_id: &str,
        title: &str,
        files: &[SnapshotInput],
        now_ms: u64,
    ) -> anyhow::Result<()> {
        let mut entries = Vec::new();
        for (n, file) in files.iter().enumerate() {
            let blob = match &file.before {
                Some(bytes) => {
                    store::write_bytes_atomic(&folder.join(format!("{n}.bin")), bytes)?;
                    Some(n as u32)
                }
                None => None,
            };
            entries.push(FileEntry {
                info: SnapshotFileInfo {
                    path: file.path.to_string_lossy().into_owned(),
                    before_hash: file.before.as_deref().map(hash_bytes),
                    after_hash: file.after_hash.clone(),
                    scope: file.scope,
                    applies: file.applies,
                },
                blob,
            });
        }
        let mut index = self.load()?;
        index.snapshots.push(Entry {
            id: id.to_string(),
            created_ms: now_ms,
            changeset_id: changeset_id.to_string(),
            title: title.to_string(),
            files: entries,
        });
        for file in files {
            self.trim(&mut index, &file.path.to_string_lossy());
        }
        store::write(&self.index_path(), &index).context("saving the snapshot index")?;
        // Only after the index is saved: the folders it no longer lists.
        self.sweep(&index);
        Ok(())
    }

    /// Keeps the newest `KEEP_PER_FILE` entries that hold `path`; the rest lose that file.
    fn trim(&self, index: &mut Index, path: &str) {
        let mut holders: Vec<usize> = (0..index.snapshots.len())
            .filter(|&i| index.snapshots[i].files.iter().any(|f| same_path(&f.info.path, path)))
            .collect();
        // Newest first; within one millisecond, the later one in the index is newer.
        holders.sort_by_key(|&i| std::cmp::Reverse((index.snapshots[i].created_ms, i)));
        for &i in holders.iter().skip(KEEP_PER_FILE) {
            let entry = &mut index.snapshots[i];
            let folder = self.dir.join(&entry.id);
            entry.files.retain(|f| {
                let drop = same_path(&f.info.path, path);
                if drop && let Some(n) = f.blob {
                    let _ = std::fs::remove_file(folder.join(format!("{n}.bin")));
                }
                !drop
            });
        }
        index.snapshots.retain(|e| !e.files.is_empty());
    }

    /// Removes the folders of snapshots the index no longer lists (best effort).
    fn sweep(&self, index: &Index) {
        let Ok(read) = std::fs::read_dir(&self.dir) else { return };
        for item in read.flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if valid_id(&name) && !index.snapshots.iter().any(|e| e.id == name) {
                let _ = std::fs::remove_dir_all(item.path());
            }
        }
    }

    /// Snapshots newest first, only those holding `path` when it is given.
    pub fn list(&self, path: Option<&Path>) -> anyhow::Result<Vec<SnapshotInfo>> {
        let _guard = Self::lock();
        let mut entries = self.load()?.snapshots;
        entries.reverse();
        entries.sort_by_key(|e| std::cmp::Reverse(e.created_ms));
        let want = path.map(|p| p.to_string_lossy().into_owned());
        Ok(entries
            .iter()
            .filter(|e| match &want {
                Some(w) => e.files.iter().any(|f| same_path(&f.info.path, w)),
                None => true,
            })
            .map(Entry::info)
            .collect())
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Option<SnapshotInfo>> {
        if !valid_id(id) {
            return Ok(None);
        }
        let _guard = Self::lock();
        Ok(self.load()?.snapshots.iter().find(|e| e.id == id).map(Entry::info))
    }

    /// `None`: no such snapshot or file in it. `Some(None)`: the file did not exist before.
    pub fn blob(&self, id: &str, path: &Path) -> anyhow::Result<Option<Option<Vec<u8>>>> {
        if !valid_id(id) {
            return Ok(None);
        }
        let _guard = Self::lock();
        let index = self.load()?;
        let want = path.to_string_lossy();
        let Some(file) = index
            .snapshots
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.files.iter().find(|f| same_path(&f.info.path, &want)))
        else {
            return Ok(None);
        };
        match file.blob {
            None => Ok(Some(None)),
            Some(n) => {
                let blob = self.dir.join(id).join(format!("{n}.bin"));
                let bytes = std::fs::read(&blob).with_context(|| format!("reading {}", blob.display()))?;
                Ok(Some(Some(bytes)))
            }
        }
    }

    /// Removes the snapshot and its blobs. An unknown id is not an error.
    pub fn discard(&self, id: &str) -> anyhow::Result<()> {
        if !valid_id(id) {
            return Ok(());
        }
        let _guard = Self::lock();
        let mut index = self.load()?;
        let before = index.snapshots.len();
        index.snapshots.retain(|e| e.id != id);
        if index.snapshots.len() != before {
            store::write(&self.index_path(), &index)?;
        }
        let _ = std::fs::remove_dir_all(self.dir.join(id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn input(path: &Path, before: Option<&[u8]>) -> SnapshotInput {
        SnapshotInput {
            path: path.to_path_buf(),
            before: before.map(<[u8]>::to_vec),
            after_hash: Some("after".into()),
            scope: ConfigScope::User,
            applies: Applies::NewSessions,
        }
    }

    fn a() -> PathBuf {
        std::env::temp_dir().join("marshell-snap-a").join("settings.json")
    }

    fn b() -> PathBuf {
        std::env::temp_dir().join("marshell-snap-b").join("config.toml")
    }

    #[test]
    fn take_and_read_back_a_blob() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        let id = snaps.take("c1", "Add hook", &[input(&a(), Some(b"old"))], 5).unwrap();
        assert!(id.starts_with('s') && id.len() == 13);
        let info = snaps.get(&id).unwrap().unwrap();
        assert_eq!(info.changeset_id, "c1");
        assert_eq!(info.title, "Add hook");
        assert_eq!(info.created_ms, 5);
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.files[0].after_hash.as_deref(), Some("after"));
        assert!(info.files[0].before_hash.is_some());
        assert_eq!(snaps.blob(&id, &a()).unwrap(), Some(Some(b"old".to_vec())));
    }

    #[test]
    fn a_file_that_did_not_exist_reads_back_as_absent() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        let id = snaps.take("c1", "Create", &[input(&a(), None)], 1).unwrap();
        assert_eq!(snaps.blob(&id, &a()).unwrap(), Some(None));
        assert_eq!(snaps.get(&id).unwrap().unwrap().files[0].before_hash, None);
        assert_eq!(snaps.blob(&id, &b()).unwrap(), None);
    }

    #[test]
    fn keeps_the_last_20_per_file() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        let mut first = String::new();
        for n in 1..=21u64 {
            let id = snaps
                .take(&format!("c{n}"), "t", &[input(&a(), Some(n.to_string().as_bytes()))], n)
                .unwrap();
            if n == 1 {
                first = id;
            }
            if n == 10 {
                snaps.take("cb", "t", &[input(&b(), Some(b"b"))], 10).unwrap();
            }
        }
        let listed = snaps.list(Some(&a())).unwrap();
        assert_eq!(listed.len(), KEEP_PER_FILE);
        assert!(listed.iter().all(|s| s.created_ms != 1));
        assert!(snaps.get(&first).unwrap().is_none());
        assert!(!crate::paths::snapshots_dir(home.path()).join(&first).exists());
        assert_eq!(snaps.list(Some(&b())).unwrap().len(), 1);
    }

    #[test]
    fn same_millisecond_keeps_the_newest() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        let mut last = String::new();
        for _ in 0..=KEEP_PER_FILE {
            last = snaps.take("c", "t", &[input(&a(), Some(b"x"))], 7).unwrap();
        }
        assert!(snaps.get(&last).unwrap().is_some());
        assert_eq!(snaps.list(Some(&a())).unwrap().len(), KEEP_PER_FILE);
    }

    #[test]
    fn list_is_newest_first() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        for n in [3u64, 1, 2] {
            snaps.take("c", "t", &[input(&a(), Some(b"x"))], n).unwrap();
        }
        let times: Vec<u64> = snaps.list(None).unwrap().iter().map(|s| s.created_ms).collect();
        assert_eq!(times, [3, 2, 1]);
    }

    #[test]
    fn unknown_and_malformed_ids_are_none() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        assert!(snaps.get("s000000000000").unwrap().is_none());
        assert!(snaps.get("../index").unwrap().is_none());
        assert!(snaps.blob("..\\x", &a()).unwrap().is_none());
        snaps.discard("../index").unwrap();
    }

    #[test]
    fn discard_removes_the_entry_and_its_blobs() {
        let home = tempfile::tempdir().unwrap();
        let snaps = Snapshots::new(home.path());
        let id = snaps.take("c1", "t", &[input(&a(), Some(b"old"))], 1).unwrap();
        let keep = snaps.take("c2", "t", &[input(&a(), Some(b"new"))], 2).unwrap();
        snaps.discard(&id).unwrap();
        assert!(snaps.get(&id).unwrap().is_none());
        assert!(!crate::paths::snapshots_dir(home.path()).join(&id).exists());
        assert!(snaps.get(&keep).unwrap().is_some());
    }
}
