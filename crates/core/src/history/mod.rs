//! The history of agent conversations: a compressed copy of each transcript, a sidecar describing it,
//! and an index over the sidecars. Transcripts are only ever read, never changed.

use crate::hooks::risk::is_remote_path;
use crate::store;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod archive;
pub mod index;
pub mod recorder;

/// What Marshell knows about one agent conversation. Stored as the `<id>.meta.json` sidecar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub agent: String,
    pub session_id: String,
    pub tab_id: String,
    pub cwd: String,
    pub transcript_path: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    /// The session this tab had before this one (after a /clear), if any.
    pub previous_in_tab: Option<String>,
}

impl store::Versioned for SessionMeta {
    const VERSION: u32 = 1;
}

/// A session id is used in file names, so it must be 1 to 64 letters, digits, `_` or `-`.
pub fn valid_session_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// True when the path is absolute, local (not UNC or a device path), and named exactly
/// `<session_id>.jsonl`. Makes no filesystem call.
pub fn transcript_allowed(path: &str, session_id: &str) -> bool {
    if !valid_session_id(session_id) || is_remote_path(path) {
        return false;
    }
    let path = Path::new(path);
    path.is_absolute()
        && path
            .file_name()
            .is_some_and(|name| *name == *format!("{session_id}.jsonl"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "00000000-0000-4000-8000-000000000001";

    fn dir_prefix() -> &'static str {
        if cfg!(windows) {
            r"C:\Users\user\.claude\projects\p\"
        } else {
            "/home/u/.claude/projects/p/"
        }
    }

    #[test]
    fn rejects_unsafe_ids_and_paths() {
        assert!(valid_session_id(ID));
        for bad in ["", "../x", "a/b", "a b"] {
            assert!(!valid_session_id(bad), "{bad:?}");
        }
        assert!(!valid_session_id(&"a".repeat(65)));

        assert!(transcript_allowed(&format!("{}{ID}.jsonl", dir_prefix()), ID));
        assert!(!transcript_allowed(&format!("p/{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!(r"\host\share\{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!(r"\?\C:\x\{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!("{}other.jsonl", dir_prefix()), ID));
        assert!(!transcript_allowed(&format!("{}{ID}.json", dir_prefix()), ID));
    }
}
