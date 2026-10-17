//! Images the app saves for a session, so the person can point the agent at them: a PDF region's PNG.

use super::AppState;
use super::files::IMAGE_CAP;
use crate::profiles::detect::attachment_ref;
use crate::{paths, store};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{AttachmentSaved, SessionInfo};
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
