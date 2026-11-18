//! A text tab's save. These are the only routes that take a file path to write, so each path is
//! checked three times before a byte moves: its shape (`files::refused`, absolute, no `..`), the
//! links on the way (`files::links_refused`), and whose file it is (`userfiles::guard`).
//!
//! A save names the hash of the bytes the tab loaded. The file is written only while it still
//! holds them, and only after a copy of them is on disk (`userfiles::history`).

use super::{AppState, files};
use crate::config::hash::hash_bytes;
use crate::projects::same_path;
use crate::userfiles::guard::{SaveRefusal, save_refusal};
use crate::userfiles::history::{History, Saved};
use crate::userfiles::write::write_in_place;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{FileConflict, FileSaved, FileUnsave};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

const BAD_PATH: &str = "That path can't be saved.";
const READ_ONLY: &str = "The file is read-only or in use by another program.";

/// One save at a time, so two saves of one file can't both pass the hash check.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
pub struct SaveQuery {
    path: String,
    expect: Option<String>,
}

/// What the tab says the disk holds.
enum Expect {
    /// No file: the tab's file was deleted, and the save puts it back.
    NoFile,
    /// A file with this hash.
    Hash(String),
}

fn parse_expect(text: Option<&str>) -> Option<Expect> {
    let text = text?;
    if text == "none" {
        return Some(Expect::NoFile);
    }
    let hex = text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (text.len() == 64 && hex).then(|| Expect::Hash(text.to_string()))
}

/// How a save ended.
enum Done {
    Saved(FileSaved),
    /// The disk doesn't hold what the tab expected. The hash is the disk's, None for no file.
    Conflict(Option<String>),
    BadPath,
    Refused(SaveRefusal),
    ReadOnly,
    /// An undo names a save the history doesn't hold for this file.
    NoSuchSave,
    Failed(String),
}

impl IntoResponse for Done {
    fn into_response(self) -> Response {
        match self {
            Done::Saved(saved) => Json(saved).into_response(),
            Done::Conflict(hash) => (StatusCode::CONFLICT, Json(FileConflict { hash })).into_response(),
            Done::BadPath => (StatusCode::BAD_REQUEST, BAD_PATH).into_response(),
            Done::Refused(why) => (StatusCode::FORBIDDEN, why.line()).into_response(),
            Done::ReadOnly => (StatusCode::FORBIDDEN, READ_ONLY).into_response(),
            Done::NoSuchSave => StatusCode::NOT_FOUND.into_response(),
            Done::Failed(text) => (StatusCode::INTERNAL_SERVER_ERROR, text).into_response(),
        }
    }
}

/// The path when its text may be saved to: not refused, absolute, and with no `..` part. It reads
/// the string only.
fn shaped(path: &str) -> Option<PathBuf> {
    if files::refused(path) {
        return None;
    }
    let path = PathBuf::from(path);
    let dotted = path.components().any(|c| c == Component::ParentDir);
    (path.is_absolute() && !dotted).then_some(path)
}

/// Why `path` may not be written, from the disk: a link on the way that must not be followed, or a
/// file that is Marshell's own or an organization's. None when the save may go on.
fn barred(path: &Path, home: &Path) -> Option<Done> {
    if files::links_refused(path) {
        return Some(Done::BadPath);
    }
    save_refusal(path, home).map(Done::Refused)
}

/// What is at a path.
enum Disk {
    Nothing,
    /// A regular file's bytes and whether its permissions say read-only.
    File(Vec<u8>, bool),
    /// A regular file past the cap. No tab loaded it, so only its hash is read.
    Large(String),
    /// A folder, a device, or a file that can't be opened.
    Other,
}

fn read_disk(path: &Path) -> io::Result<Disk> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Disk::Nothing),
        Err(e) => return Err(e),
        Ok(meta) if !meta.is_file() => return Ok(Disk::Other),
        Ok(_) => {}
    }
    let Some((mut file, meta)) = files::open_regular(path) else {
        return Ok(Disk::Other);
    };
    let read_only = meta.permissions().readonly();
    let mut bytes = Vec::new();
    (&mut file).take(files::TEXT_CAP + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 <= files::TEXT_CAP {
        return Ok(Disk::File(bytes, read_only));
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    io::copy(&mut file, &mut hasher)?;
    Ok(Disk::Large(hex::encode(hasher.finalize())))
}

/// A write that failed, as an answer.
fn write_failed(e: io::Error) -> Done {
    // Windows: 32 is a sharing violation and 33 a lock violation, another program holds the file.
    let held = cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33));
    if e.kind() == io::ErrorKind::PermissionDenied || held {
        Done::ReadOnly
    } else {
        Done::Failed(e.to_string())
    }
}

/// The answer to a save that left `path` holding bytes of `hash`.
fn saved(path: &Path, hash: String, snapshot: Option<String>) -> Done {
    match std::fs::metadata(path) {
        Ok(meta) => Done::Saved(FileSaved {
            size: meta.len(),
            modified_ms: files::modified_ms(&meta),
            hash,
            snapshot,
        }),
        Err(e) => Done::Failed(e.to_string()),
    }
}

/// The disk's hash when it holds a file, for the answer to a save that found one it didn't expect.
fn found(disk: io::Result<Disk>) -> Done {
    match disk {
        Ok(Disk::File(bytes, _)) => Done::Conflict(Some(hash_bytes(&bytes))),
        Ok(Disk::Large(hash)) => Done::Conflict(Some(hash)),
        Ok(Disk::Nothing) => Done::Conflict(None),
        Ok(Disk::Other) => Done::BadPath,
        Err(e) => write_failed(e),
    }
}

/// The save itself, under the lock: read, compare, copy the old bytes, write.
fn save_checked(path: &Path, expect: &Expect, body: &[u8], home: &Path) -> Done {
    let _guard = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let new_hash = hash_bytes(body);
    match (read_disk(path), expect) {
        (Ok(Disk::Nothing), Expect::NoFile) => {
            // A save puts a file back; it never makes the folder the file was in.
            if !path.parent().is_some_and(Path::is_dir) {
                return Done::BadPath;
            }
            match write_in_place(path, body, true) {
                Ok(()) => saved(path, new_hash, None),
                // Something made the file between the read and the write.
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => found(read_disk(path)),
                Err(e) => write_failed(e),
            }
        }
        (Ok(Disk::File(bytes, read_only)), Expect::Hash(expected)) if hash_bytes(&bytes) == *expected => {
            if bytes == body {
                return saved(path, new_hash, None);
            }
            if read_only {
                return Done::ReadOnly;
            }
            replace(path, &bytes, body, new_hash, home)
        }
        (disk, _) => found(disk),
    }
}

/// Writes `new` over a file that holds `old`, once a copy of `old` is on disk. The caller holds the lock.
fn replace(path: &Path, old: &[u8], new: &[u8], new_hash: String, home: &Path) -> Done {
    let snapshot = match History::new(home).take(path, old, &new_hash, super::doctor::now_ms()) {
        Ok(id) => id,
        Err(e) => {
            return Done::Failed(format!(
                "The file's old text could not be copied, so it was not saved: {e:#}"
            ));
        }
    };
    match write_in_place(path, new, false) {
        Ok(()) => saved(path, new_hash, Some(snapshot)),
        Err(e) => write_failed(e),
    }
}

/// The undo itself, under the lock: the file goes back only while it holds what the save wrote.
fn unsave_checked(path: &Path, save: &Saved, home: &Path) -> Done {
    let _guard = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match read_disk(path) {
        Ok(Disk::File(bytes, read_only)) if hash_bytes(&bytes) == save.after_hash => {
            if read_only {
                return Done::ReadOnly;
            }
            // The undo is a save like any other, so it can be undone too.
            replace(path, &bytes, &save.before, hash_bytes(&save.before), home)
        }
        disk => found(disk),
    }
}

/// PUT /v1/files/content?path=<absolute>&expect=<hash|none>, the body the file's new bytes.
pub async fn save(State(st): State<AppState>, Query(q): Query<SaveQuery>, body: Bytes) -> Response {
    let (Some(path), Some(expect)) = (shaped(&q.path), parse_expect(q.expect.as_deref())) else {
        return Done::BadPath.into_response();
    };
    let home = st.home().to_path_buf();
    let done = tokio::task::spawn_blocking(move || {
        barred(&path, &home).unwrap_or_else(|| save_checked(&path, &expect, &body, &home))
    })
    .await;
    match done {
        Ok(done) => done.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// POST /v1/files/unsave: puts back what the save `snapshot` replaced at `path`.
pub async fn unsave(State(st): State<AppState>, Json(req): Json<FileUnsave>) -> Response {
    let Some(path) = shaped(&req.path) else {
        return Done::BadPath.into_response();
    };
    let home = st.home().to_path_buf();
    let done = tokio::task::spawn_blocking(move || {
        if let Some(no) = barred(&path, &home) {
            return no;
        }
        let save = match History::new(&home).get(&req.snapshot) {
            Ok(Some(save)) => save,
            Ok(None) => return Done::NoSuchSave,
            Err(e) => return Done::Failed(format!("{e:#}")),
        };
        // A save's copy goes back to the file it came from and to no other.
        if !same_path(&save.path.to_string_lossy(), &path.to_string_lossy()) {
            return Done::NoSuchSave;
        }
        unsave_checked(&path, &save, &home)
    })
    .await;
    match done {
        Ok(done) => done.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
