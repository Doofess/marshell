//! Images the app saves for a session, so the person can point the agent at them: a PDF region's PNG.

use super::AppState;
use super::files::IMAGE_CAP;
use super::sessions::cannot_resume;
use crate::profiles::detect::attachment_ref;
use crate::{paths, store};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{AttachmentSaved, PasteRequest, SessionInfo};
use std::time::{SystemTime, UNIX_EPOCH};

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// The agent the session runs: the one it was started as, else the one in the foreground of its shell.
pub(crate) fn effective_agent(info: &SessionInfo) -> Option<String> {
    info.agent
        .clone()
        .or_else(|| info.foreground.as_ref().and_then(|f| f.agent.clone()))
}

/// POST /v1/sessions/{tab}/attachments: the body is a PNG. 404 for an unknown tab, 400 for anything
/// else than a PNG, 413 over the image cap.
pub async fn save(State(st): State<AppState>, Path(tab): Path<String>, body: Bytes) -> Response {
    // `tab` becomes a folder name only after the registry found it; tab ids are made by the core.
    if st.session(&tab).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if body.len() as u64 > IMAGE_CAP {
        return (StatusCode::PAYLOAD_TOO_LARGE, "That image is over 20 MB.").into_response();
    }
    if !body.starts_with(&PNG_SIGNATURE) {
        return (StatusCode::BAD_REQUEST, "That isn't a PNG image.").into_response();
    }
    let agent = st
        .registry()
        .snapshot()
        .sessions
        .iter()
        .find(|s| s.tab_id == tab)
        .and_then(effective_agent);
    let dir = paths::attachments_dir(st.home()).join(&tab);
    let saved = tokio::task::spawn_blocking(move || save_png(&dir, &body)).await;
    match saved {
        Ok(Ok(path)) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let reference = attachment_ref(agent.as_deref(), &path);
            Json(AttachmentSaved {
                name,
                path: path.to_string_lossy().into_owned(),
                reference,
            })
            .into_response()
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "Couldn't save the image.").into_response(),
    }
}

const COMMENT_LIMIT: usize = 2000;

/// A comment goes into the prompt as typed text: no control characters (a line break would submit it,
/// an escape could close the bracketed paste early), and at most 2000 characters.
pub(crate) fn check_comment(text: &str) -> Result<(), &'static str> {
    if text.chars().any(char::is_control) {
        return Err("The comment can't contain line breaks or control characters.");
    }
    if text.chars().count() > COMMENT_LIMIT {
        return Err("The comment is over 2000 characters.");
    }
    Ok(())
}

/// The bytes to type: each part wrapped as its own bracketed paste when the program asked for that.
pub(crate) fn paste_bytes(parts: &[&str], bracketed: bool) -> Vec<u8> {
    let mut out = Vec::new();
    for part in parts {
        if bracketed {
            out.extend_from_slice(b"\x1b[200~");
        }
        out.extend_from_slice(part.as_bytes());
        if bracketed {
            out.extend_from_slice(b"\x1b[201~");
        }
    }
    out
}

/// `<digits>.png` or `<digits>-<digits>.png`: the names `save_png` makes.
fn is_attachment_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".png") else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match stem.split_once('-') {
        Some((ms, n)) => digits(ms) && digits(n),
        None => digits(stem),
    }
}

/// POST /v1/sessions/{tab}/paste: types the saved attachment's reference, then the comment, into the
/// session. No Enter is added. 404 unknown tab, 409 paused, 400 for a name that isn't a saved
/// attachment or a bad comment.
pub async fn paste(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<PasteRequest>) -> Response {
    let Some(session) = st.session(&tab) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if session.is_frozen() {
        return cannot_resume("It is paused.");
    }
    if let Err(why) = check_comment(&req.text) {
        return (StatusCode::BAD_REQUEST, why).into_response();
    }
    let not_saved = || (StatusCode::BAD_REQUEST, "That isn't a saved attachment.").into_response();
    if !is_attachment_name(&req.attachment) {
        return not_saved();
    }
    let path = paths::attachments_dir(st.home()).join(&tab).join(&req.attachment);
    if !path.is_file() {
        return not_saved();
    }
    let agent = st
        .registry()
        .snapshot()
        .sessions
        .iter()
        .find(|s| s.tab_id == tab)
        .and_then(effective_agent);
    let reference = attachment_ref(agent.as_deref(), &path);
    let comment = format!(" {}", req.text);
    let parts: Vec<&str> = if req.text.is_empty() {
        vec![&reference]
    } else {
        vec![&reference, &comment]
    };
    session.write_input(paste_bytes(&parts, session.bracketed_paste()));
    StatusCode::NO_CONTENT.into_response()
}

/// Writes `bytes` to `<dir>/<unix ms>.png`, or `<unix ms>-2.png` and up when that name is taken.
/// A file that already exists is never replaced.
fn save_png(dir: &std::path::Path, bytes: &[u8]) -> anyhow::Result<std::path::PathBuf> {
    store::create_private_dir(dir)?;
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    for n in 1u32.. {
        let name = if n == 1 {
            format!("{ms}.png")
        } else {
            format!("{ms}-{n}.png")
        };
        let path = dir.join(name);
        match store::write_new_private_file(&path, bytes) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!("the loop only ends by returning")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_bytes_wraps_each_part_when_the_program_asked_for_it() {
        assert_eq!(
            paste_bytes(&["\"a.png\"", " x.pdf, page 1: hi"], true),
            b"\x1b[200~\"a.png\"\x1b[201~\x1b[200~ x.pdf, page 1: hi\x1b[201~".to_vec()
        );
        assert_eq!(paste_bytes(&["\"a.png\"", " hi"], false), b"\"a.png\" hi".to_vec());
    }

    #[test]
    fn a_comment_has_no_control_characters_and_a_length_limit() {
        assert!(check_comment("x.pdf, page 2: is this right?").is_ok());
        for bad in ["a\nb", "a\rb", "a\tb", "a\x1b[201~b", "a\x7fb", "a\u{9b}201~b"] {
            assert!(check_comment(bad).is_err(), "{bad:?} should be refused");
        }
        assert!(check_comment(&"é".repeat(2000)).is_ok());
        assert!(check_comment(&"a".repeat(2001)).is_err());
    }
}
