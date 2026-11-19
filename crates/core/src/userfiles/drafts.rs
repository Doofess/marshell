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

/// The most bytes all PDF drafts take together.
pub const PDF_DRAFTS_BYTES: u64 = 1024 * 1024 * 1024;

/// One read-modify-write of the folder at a time.
static DRAFTS_LOCK: Mutex<()> = Mutex::new(());

/// Why a draft was not kept.
#[derive(Debug)]
pub enum PutError {
    /// `MAX_DRAFTS` files already have a draft and this one is for another.
    Full,
    /// The PDF drafts would take more than `PDF_DRAFTS_BYTES`.
    PdfBudget,
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

/// What a PDF draft says beside its bytes, which are kept in the `.pdf` file next to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfDraftMeta {
    pub path: String,
    /// The SHA-256 of the file's bytes the notes started from. None when there was no file.
    #[serde(default)]
    pub base_hash: Option<String>,
    pub edited_ms: u64,
}

impl Versioned for PdfDraftMeta {
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
    pdf_budget: u64,
}

impl Drafts {
    pub fn new(home: &Path) -> Self {
        Self {
            dir: crate::paths::drafts_dir(home),
            limit: MAX_DRAFTS,
            pdf_budget: PDF_DRAFTS_BYTES,
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

    /// A store with a draft count limit and a PDF byte budget, for the tests of those.
    #[cfg(test)]
    fn with_limits(home: &Path, limit: usize, pdf_budget: u64) -> Self {
        Self {
            limit,
            pdf_budget,
            ..Self::new(home)
        }
    }

    fn pdf_dir(&self) -> PathBuf {
        self.dir.join("pdf")
    }

    /// The meta file of a PDF draft; its bytes are in the `.pdf` file beside it.
    fn pdf_file_of(&self, path: &str) -> PathBuf {
        let hex = hex::encode(Sha256::digest(key(path).as_bytes()));
        self.pdf_dir().join(format!("{}.json", &hex[..32]))
    }

    /// The meta files of PDF drafts, not their backups or temp files.
    fn pdf_files(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(self.pdf_dir()) else {
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

    fn file_of(&self, path: &str) -> PathBuf {
        let hex = hex::encode(Sha256::digest(key(path).as_bytes()));
        self.dir.join(format!("{}.json", &hex[..32]))
    }

    /// The draft in `file`, when it parses.
    fn read(file: &Path) -> Option<Draft> {
        store::read::<Stored>(file).ok().flatten().map(|s| s.draft)
    }

    /// The PDF draft's meta in `file`, when it parses and its bytes are there.
    fn read_pdf(file: &Path) -> Option<PdfDraftMeta> {
        let meta = store::read::<PdfDraftMeta>(file).ok().flatten()?;
        file.with_extension("pdf").is_file().then_some(meta)
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
            .chain(
                self.pdf_files()
                    .iter()
                    .filter_map(|f| Self::read_pdf(f))
                    .map(|m| DraftInfo {
                        path: m.path,
                        edited_ms: m.edited_ms,
                    }),
            )
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
        if !file.exists() && self.files().len() + self.pdf_files().len() >= self.limit {
            return Err(PutError::Full);
        }
        store::write(&file, &Stored { draft: draft.clone() }).map_err(PutError::Io)
    }

    /// The PDF draft of `path` and its bytes. None for none, a draft that doesn't parse or has lost
    /// its bytes, or one that holds another path.
    pub fn get_pdf(&self, path: &str) -> Option<(PdfDraftMeta, Vec<u8>)> {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let file = self.pdf_file_of(path);
        let meta = Self::read_pdf(&file).filter(|m| key(&m.path) == key(path))?;
        let bytes = std::fs::read(file.with_extension("pdf")).ok()?;
        Some((meta, bytes))
    }

    /// Keeps the PDF's unsaved notes, replacing the file's earlier draft. A replacement is never
    /// refused for the count, and its own old bytes don't count toward the budget.
    pub fn put_pdf(&self, path: &str, base_hash: Option<&str>, edited_ms: u64, bytes: &[u8]) -> Result<(), PutError> {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let file = self.pdf_file_of(path);
        let pdf = file.with_extension("pdf");
        if !file.exists() && self.files().len() + self.pdf_files().len() >= self.limit {
            return Err(PutError::Full);
        }
        let others: u64 = self
            .pdf_files()
            .iter()
            .filter(|f| **f != file)
            .filter_map(|f| std::fs::metadata(f.with_extension("pdf")).ok())
            .map(|m| m.len())
            .sum();
        if others.saturating_add(bytes.len() as u64) > self.pdf_budget {
            return Err(PutError::PdfBudget);
        }
        // The bytes first: a meta never names bytes that are not there.
        store::write_bytes_atomic(&pdf, bytes).map_err(PutError::Io)?;
        let meta = PdfDraftMeta {
            path: path.to_string(),
            base_hash: base_hash.map(str::to_string),
            edited_ms,
        };
        if let Err(e) = store::write(&file, &meta) {
            // A refused draft leaves none: these bytes would sit under the old meta's base, or under no meta the budget sees.
            let _ = std::fs::remove_file(&pdf);
            let _ = std::fs::remove_file(&file);
            let _ = std::fs::remove_file(store::bak_path(&file));
            return Err(PutError::Io(e));
        }
        Ok(())
    }

    /// Drops the draft of `path`, text or PDF, and its backup. Nothing to drop is fine.
    pub fn remove(&self, path: &str) {
        let _guard = DRAFTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let file = self.file_of(path);
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(store::bak_path(&file));
        let pdf_meta = self.pdf_file_of(path);
        let _ = std::fs::remove_file(&pdf_meta);
        let _ = std::fs::remove_file(store::bak_path(&pdf_meta));
        let _ = std::fs::remove_file(pdf_meta.with_extension("pdf"));
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

    fn pdf_at(drafts: &Drafts, name: &str, len: usize, edited_ms: u64) -> Result<(), PutError> {
        drafts.put_pdf(&p(name), Some("abc"), edited_ms, &vec![7u8; len])
    }

    #[test]
    fn pdf_drafts_round_trip() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put_pdf(&p("a.pdf"), Some("abc"), 9, b"%PDF-mine").unwrap();
        let (meta, bytes) = drafts.get_pdf(&p("a.pdf")).unwrap();
        assert_eq!(bytes, b"%PDF-mine");
        assert_eq!(meta.base_hash.as_deref(), Some("abc"));
        assert_eq!(meta.edited_ms, 9);
        assert_eq!(meta.path, p("a.pdf"));
        // No base hash is kept as none, and a replacement wins.
        drafts.put_pdf(&p("a.pdf"), None, 10, b"%PDF-two").unwrap();
        let (meta, bytes) = drafts.get_pdf(&p("a.pdf")).unwrap();
        assert_eq!(
            (meta.base_hash, meta.edited_ms, bytes),
            (None, 10, b"%PDF-two".to_vec())
        );
        assert!(drafts.get_pdf(&p("b.pdf")).is_none());
    }

    #[test]
    fn pdf_draft_whose_meta_cant_be_written_leaves_nothing_behind() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        // A folder where the meta goes makes its write fail after the bytes are written.
        let meta = drafts.pdf_file_of(&p("a.pdf"));
        std::fs::create_dir_all(&meta).unwrap();
        assert!(matches!(
            drafts.put_pdf(&p("a.pdf"), Some("abc"), 9, b"%PDF-mine"),
            Err(PutError::Io(_))
        ));
        assert!(!meta.with_extension("pdf").exists());
        assert!(drafts.get_pdf(&p("a.pdf")).is_none());
    }

    #[test]
    fn pdf_drafts_are_listed_with_text_drafts() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        drafts.put(&draft(&p("a.txt"), "", 1)).unwrap();
        pdf_at(&drafts, "b.pdf", 3, 3).unwrap();
        let list = drafts.list();
        assert_eq!(list.iter().map(|d| d.edited_ms).collect::<Vec<_>>(), [3, 1]);
        assert_eq!(list[0].path, p("b.pdf"));
    }

    #[test]
    fn pdf_drafts_removed_by_the_one_delete() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::new(home.path());
        pdf_at(&drafts, "a.pdf", 3, 1).unwrap();
        pdf_at(&drafts, "a.pdf", 4, 2).unwrap();
        drafts.remove(&p("a.pdf"));
        assert!(drafts.get_pdf(&p("a.pdf")).is_none());
        assert!(drafts.list().is_empty());
        let left: Vec<_> = std::fs::read_dir(home.path().join("drafts").join("pdf"))
            .unwrap()
            .flatten()
            .collect();
        assert!(left.is_empty(), "left behind: {left:?}");
    }

    #[test]
    fn pdf_drafts_keep_to_the_budget() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::with_limits(home.path(), MAX_DRAFTS, 100);
        pdf_at(&drafts, "a.pdf", 60, 1).unwrap();
        assert!(matches!(pdf_at(&drafts, "b.pdf", 41, 2), Err(PutError::PdfBudget)));
        assert!(drafts.get_pdf(&p("b.pdf")).is_none());
        pdf_at(&drafts, "b.pdf", 40, 2).unwrap();
        assert!(matches!(pdf_at(&drafts, "c.pdf", 1, 3), Err(PutError::PdfBudget)));
    }

    #[test]
    fn pdf_drafts_replacing_one_does_not_count_it_twice() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::with_limits(home.path(), MAX_DRAFTS, 100);
        pdf_at(&drafts, "a.pdf", 80, 1).unwrap();
        pdf_at(&drafts, "a.pdf", 90, 2).unwrap();
        assert!(matches!(pdf_at(&drafts, "a.pdf", 101, 3), Err(PutError::PdfBudget)));
        assert_eq!(drafts.get_pdf(&p("a.pdf")).unwrap().1.len(), 90);
    }

    #[test]
    fn pdf_drafts_count_toward_the_draft_limit() {
        let home = tempfile::tempdir().unwrap();
        let drafts = Drafts::with_limits(home.path(), 2, PDF_DRAFTS_BYTES);
        drafts.put(&draft(&p("a.txt"), "", 1)).unwrap();
        pdf_at(&drafts, "b.pdf", 1, 2).unwrap();
        assert!(matches!(pdf_at(&drafts, "c.pdf", 1, 3), Err(PutError::Full)));
        assert!(matches!(drafts.put(&draft(&p("d.txt"), "", 3)), Err(PutError::Full)));
        // A replacement is never refused for the count.
        pdf_at(&drafts, "b.pdf", 2, 4).unwrap();
    }
}
