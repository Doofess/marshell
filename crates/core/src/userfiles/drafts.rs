//! The unsaved edits of text tabs: one draft per file in `<home>/drafts/<key>.json`, so a tab that
//! is closed or a Marshell that quits does not lose them. The draft routes never touch the file the
//! draft is about; a draft is kept only in Marshell's home.

use crate::store::{self, Versioned};
use marshell_protocol::api::{Draft, DraftInfo};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Drafts kept in all; a draft for a file that has none yet is refused past this.
pub const MAX_DRAFTS: usize = 500;

/// One read-modify-write of the folder at a time.
static DRAFTS_LOCK: Mutex<()> = Mutex::new(());

/// Why a draft was not kept.
#[derive(Debug)]
pub enum PutError {
    /// `MAX_DRAFTS` files already have a draft and this one is for another.
    Full,
    Io(anyhow::Error),
}

/// A draft as it is stored, with the file's version beside its fields.
#[derive(Serialize, Deserialize)]
struct Stored {
    #[serde(flatten)]
    draft: Draft,
}

impl Versioned for Stored {
    const VERSION: u32 = 1;
}

/// What names a file: Windows compares paths without case and takes `/` for `\`.
fn key(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\").to_lowercase()
    } else {
        path.to_string()
    }
}

pub struct Drafts {
    dir: PathBuf,
    limit: usize,
}

impl Drafts {
    pub fn new(home: &Path) -> Self {
        Self {
            dir: crate::paths::drafts_dir(home),
            limit: MAX_DRAFTS,
        }
    }

    /// A store that keeps `limit` drafts, for the tests of that limit.
    #[cfg(test)]
    fn with_limit(home: &Path, limit: usize) -> Self {
        Self {
            limit,
            ..Self::new(home)
        }
    }

    fn file_of(&self, path: &str) -> PathBuf {
        let hex = hex::encode(Sha256::digest(key(path).as_bytes()));
        self.dir.join(format!("{}.json", &hex[..32]))
    }

    /// The draft in `file`, when it parses.
    fn read(file: &Path) -> Option<Draft> {
        store::read::<Stored>(file).ok().flatten().map(|s| s.draft)
    }

    /// The files that hold drafts, not their backups or temp files.
    fn files(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|x| x == "json")
                    && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
            })
            .collect()
    }

    /// The drafts that parse, newest edit first.
    pub fn list(&self) -> Vec<DraftInfo> {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut drafts: Vec<DraftInfo> = self
            .files()
            .iter()
            .filter_map(|f| Self::read(f))
            .map(|d| DraftInfo {
                path: d.path,
                edited_ms: d.edited_ms,
            })
            .collect();
        drafts.sort_by_key(|d| std::cmp::Reverse(d.edited_ms));
        drafts
    }

    /// The draft of `path`. None for none, a file that doesn't parse, or one that holds another path.
    pub fn get(&self, path: &str) -> Option<Draft> {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        Self::read(&self.file_of(path)).filter(|d| key(&d.path) == key(path))
    }

    /// Keeps the draft, replacing the file's earlier one. A replacement is never refused.
    pub fn put(&self, draft: &Draft) -> Result<(), PutError> {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let file = self.file_of(&draft.path);
        if !file.exists() && self.files().len() >= self.limit {
            return Err(PutError::Full);
        }
        store::write(&file, &Stored { draft: draft.clone() }).map_err(PutError::Io)
    }

    /// Drops the draft of `path`, and its backup. Nothing to drop is fine.
    pub fn remove(&self, path: &str) {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let file = self.file_of(path);
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(store::bak_path(&file));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(path: &str, text: &str, edited_ms: u64) -> Draft {
        Draft {
            path: path.into(),
            text: text.into(),
            base_text: "base".into(),
            base_hash: Some("h".into()),
            encoding: "utf-8".into(),
            eol: "lf".into(),
            edited_ms,
        }
    }

    fn p(name: &str) -> String {
        if cfg!(windows) {
            format!(r"C:\proj\{name}")
        } else {
            format!("/proj/{name}")
        }
    }

    #[test]
    fn put_then_get() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        let d = draft(&p("a.txt"), "mine", 5);
        drafts.put(&d).unwrap();
        assert_eq!(drafts.get(&p("a.txt")), Some(d));
    }

    #[test]
    fn put_replaces() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft(&p("a.txt"), "one", 1)).unwrap();
        drafts.put(&draft(&p("a.txt"), "two", 2)).unwrap();
        assert_eq!(drafts.get(&p("a.txt")).unwrap().text, "two");
        assert_eq!(drafts.list().len(), 1);
    }

    #[test]
    fn remove_then_get_is_none() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft(&p("a.txt"), "one", 1)).unwrap();
        drafts.put(&draft(&p("a.txt"), "two", 2)).unwrap();
        drafts.remove(&p("a.txt"));
        assert_eq!(drafts.get(&p("a.txt")), None);
        // Nothing to remove is fine.
        drafts.remove(&p("a.txt"));
    }

    #[test]
    fn list_is_newest_first() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft(&p("a.txt"), "", 1)).unwrap();
        drafts.put(&draft(&p("b.txt"), "", 3)).unwrap();
        drafts.put(&draft(&p("c.txt"), "", 2)).unwrap();
        let list = drafts.list();
        assert_eq!(list.iter().map(|d| d.edited_ms).collect::<Vec<_>>(), [3, 2, 1]);
        assert_eq!(list[0].path, p("b.txt"));
    }

    #[test]
    fn get_ignores_a_cut_short_file() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft(&p("a.txt"), "one", 1)).unwrap();
        let file = drafts.file_of(&p("a.txt"));
        let bytes = std::fs::read(&file).unwrap();
        std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
        assert_eq!(drafts.get(&p("a.txt")), None);
        assert!(drafts.list().is_empty());
    }

    #[test]
    fn get_ignores_a_file_that_holds_another_path() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft(&p("a.txt"), "one", 1)).unwrap();
        std::fs::copy(drafts.file_of(&p("a.txt")), drafts.file_of(&p("b.txt"))).unwrap();
        assert_eq!(drafts.get(&p("b.txt")), None);
        assert!(drafts.get(&p("a.txt")).is_some());
    }

    #[cfg(windows)]
    #[test]
    fn the_key_ignores_case_and_slashes_on_windows() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft("C:/Proj/A.txt", "one", 1)).unwrap();
        assert_eq!(drafts.get(r"c:\proj\a.TXT").unwrap().text, "one");
        drafts.put(&draft(r"c:\PROJ\a.txt", "two", 2)).unwrap();
        assert_eq!(drafts.list().len(), 1);
    }

    #[test]
    fn the_501st_draft_is_refused_and_a_replacement_is_not() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::with_limit(home.path(), 2);
        drafts.put(&draft(&p("a.txt"), "", 1)).unwrap();
        drafts.put(&draft(&p("b.txt"), "", 2)).unwrap();
        assert!(matches!(drafts.put(&draft(&p("c.txt"), "", 3)), Err(PutError::Full)));
        drafts.put(&draft(&p("a.txt"), "again", 4)).unwrap();
        assert_eq!(drafts.get(&p("a.txt")).unwrap().text, "again");
        assert_eq!(drafts.get(&p("c.txt")), None);
    }
}
