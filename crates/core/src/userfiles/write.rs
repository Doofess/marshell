//! Writing a person's file in place, as an editor does: the file keeps its permissions, its hard
//! links and its place for a program that watches it. The caller has a copy of the old bytes on
//! disk before it calls this (`history`), so a write that is cut short can be put back.

use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` as the whole of `path` and flushes them to disk. With `create` the file must not
/// exist yet; without it the file must exist, and it is written from its start and then cut to the
/// new length, so it is never empty on the way. A folder is never created.
pub fn write_in_place(path: &Path, bytes: &[u8], create: bool) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if create {
        options.create_new(true);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.set_len(bytes.len() as u64)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_over_a_longer_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "a much longer old text").unwrap();
        write_in_place(&path, b"new", false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn writes_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "old").unwrap();
        write_in_place(&path, b"", false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"");
    }

    #[test]
    fn keeps_a_hard_link() {
        let dir = tempfile::tempdir().unwrap();
        let (one, two) = (dir.path().join("one.txt"), dir.path().join("two.txt"));
        std::fs::write(&one, "old").unwrap();
        std::fs::hard_link(&one, &two).unwrap();
        write_in_place(&one, b"new text", false).unwrap();
        assert_eq!(std::fs::read(&two).unwrap(), b"new text");
    }

    #[test]
    fn without_create_a_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        let err = write_in_place(&path, b"new", false).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(!path.exists());
    }

    #[test]
    fn create_writes_a_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        write_in_place(&path, b"new", true).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn create_refuses_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "old").unwrap();
        let err = write_in_place(&path, b"new", true).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
    }

    #[test]
    fn does_not_create_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("a.txt");
        assert!(write_in_place(&path, b"new", true).is_err());
        assert!(!dir.path().join("missing").exists());
    }
}
