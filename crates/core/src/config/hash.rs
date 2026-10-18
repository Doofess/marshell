//! File hashes: the SHA-256 of a file's raw bytes, as 64 lowercase hex characters.

use sha2::{Digest, Sha256};
use std::path::Path;

pub fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `Ok(None)` when the file doesn't exist.
pub fn hash_file(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(hash_bytes(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_sha256_hex() {
        assert_eq!(
            hash_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn missing_file_hashes_to_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(hash_file(&dir.path().join("nope.json")).unwrap(), None);
        std::fs::write(dir.path().join("a"), b"abc").unwrap();
        assert_eq!(
            hash_file(&dir.path().join("a")).unwrap().as_deref(),
            Some(hash_bytes(b"abc").as_str())
        );
    }
}
