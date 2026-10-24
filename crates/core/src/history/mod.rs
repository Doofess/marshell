//! The history of agent conversations: a compressed copy of each transcript, a sidecar describing it,
//! and an index over the sidecars. Transcripts are only ever read, never changed.

use crate::hooks::risk::is_remote_path;
use crate::store;
use marshell_protocol::history::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod archive;
pub mod cast;
pub mod export;
pub mod index;
pub mod marks;
pub mod recorder;
pub mod recordings;

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
    /// The recordings (in `recordings/`) of the tabs this conversation ran in, oldest first.
    #[serde(default)]
    pub recordings: Vec<RecordingLink>,
    /// Where the conversation ran. Sidecars from before this field read as Marshell.
    #[serde(default)]
    pub client: Client,
}

/// One recording a conversation ran in, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingLink {
    /// The file name in `recordings/`.
    pub file: String,
    /// The first hook event of this conversation in that recording, unix ms.
    pub first_ms: u64,
    /// The last hook event of this conversation in that recording, unix ms.
    pub last_ms: u64,
}

impl store::Versioned for SessionMeta {
    const VERSION: u32 = 2;
}

/// A session id is used in file names and as a command-line argument, so it must be 1 to 64 letters, digits,
/// `_` or `-`, and start with a letter or digit (so it can't read as an option).
pub fn valid_session_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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
        // A leading `-` or `_` could read as an option.
        for bad in ["-x", "--dangerously-skip-permissions", "_x"] {
            assert!(!valid_session_id(bad), "{bad:?}");
        }
        assert!(valid_session_id("a-_"));
        assert!(valid_session_id("0"));

        assert!(transcript_allowed(&format!("{}{ID}.jsonl", dir_prefix()), ID));
        assert!(!transcript_allowed(&format!("p/{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!(r"\\host\share\{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!(r"\\?\C:\x\{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!(r"\\.\pipe\{ID}.jsonl"), ID));
        assert!(!transcript_allowed(&format!("{}other.jsonl", dir_prefix()), ID));
        assert!(!transcript_allowed(&format!("{}{ID}.json", dir_prefix()), ID));
    }
}
