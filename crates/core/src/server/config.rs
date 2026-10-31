use super::AppState;
use crate::config::guard::{Guard, Refusal};
use crate::config::hash::hash_bytes;
use crate::config::preview::MAX_CONFIG_BYTES;
use crate::config::service::ApplyError;
use crate::config::write::resolve_link;
use crate::projects::canonical;
use axum::Json;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::config::{
    ApplyConflict, ApplyRequest, ChangeSet, ConfigFile, ConfigPreview, RefuseReason, SnapshotInfo,
};
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn failed(err: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
}

fn not_found(what: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("no such {what}")).into_response()
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: Option<String>,
}

fn refused_file(path: String, reason: RefuseReason) -> ConfigFile {
    ConfigFile {
        path,
        exists: false,
        hash: None,
        content: None,
        refused: Some(reason),
    }
}

/// Reads the file at `path` for the editor. Nothing is read for a path the guard refuses, except a
/// managed file, which is readable but marked so the UI knows it can't be written.
fn read_file(guard: &Guard, given: String) -> ConfigFile {
    let path = Path::new(&given);
    let mut refused = None;
    match guard.check(path) {
        Ok(_) => {}
        Err(Refusal::NotAbsolute) => return refused_file(given, RefuseReason::NotAbsolute),
        Err(Refusal::OutsideAllowed) => return refused_file(given, RefuseReason::OutsideAllowed),
        Err(Refusal::MarshellOwn) => return refused_file(given, RefuseReason::MarshellOwn),
        Err(Refusal::Managed) => refused = Some(RefuseReason::Managed),
    }
    let mut file = ConfigFile {
        path: given.clone(),
        exists: false,
        hash: None,
        content: None,
        refused,
    };
    let target = match resolve_link(&canonical(path)) {
        Ok(target) => target,
        Err(_) => {
            file.refused = Some(RefuseReason::Unreadable);
            return file;
        }
    };
    let meta = match std::fs::metadata(&target) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return file,
        Err(_) => {
            file.refused = Some(RefuseReason::Unreadable);
            return file;
        }
    };
    file.exists = true;
    if meta.len() > MAX_CONFIG_BYTES {
        file.refused = Some(RefuseReason::TooLarge);
        return file;
    }
    match std::fs::read(&target) {
        Ok(bytes) => {
            file.hash = Some(hash_bytes(&bytes));
            match String::from_utf8(bytes) {
                Ok(text) => file.content = Some(text),
                // A managed file keeps its own reason.
                Err(_) => file.refused = file.refused.or(Some(RefuseReason::NotUtf8)),
            }
        }
        Err(_) => file.refused = Some(RefuseReason::Unreadable),
    }
    file
}

pub async fn file(State(st): State<AppState>, Query(q): Query<PathQuery>) -> Response {
    let Some(path) = q.path else {
        return (StatusCode::BAD_REQUEST, "path is required").into_response();
    };
    match tokio::task::spawn_blocking(move || read_file(&st.0.config.guard(), path)).await {
        Ok(file) => Json(file).into_response(),
        Err(e) => failed(e),
    }
}

pub async fn preview(State(st): State<AppState>, Json(cs): Json<ChangeSet>) -> Response {
    match tokio::task::spawn_blocking(move || st.0.config.preview(cs)).await {
        Ok(preview) => Json(preview).into_response(),
        Err(e) => failed(e),
    }
}

pub async fn get_preview(State(st): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    match tokio::task::spawn_blocking(move || st.0.config.get_preview(&id)).await {
        Ok(Some(preview)) => Json::<ConfigPreview>(preview).into_response(),
        Ok(None) => not_found("preview"),
        Err(e) => failed(e),
    }
}

pub async fn apply(State(st): State<AppState>, Json(req): Json<ApplyRequest>) -> Response {
    match tokio::task::spawn_blocking(move || st.0.config.apply(&req, now_ms())).await {
        Ok(Ok(result)) => Json(result).into_response(),
        Ok(Err(ApplyError::UnknownChangeSet)) => not_found("preview"),
        Ok(Err(ApplyError::Conflict { reason, paths })) => {
            (StatusCode::CONFLICT, Json(ApplyConflict { reason, paths })).into_response()
        }
        Ok(Err(ApplyError::Failed(e))) => failed(format!("{e:#}")),
        Err(e) => failed(e),
    }
}

/// Snapshots store resolved targets, so `path` is resolved the same way before it is looked up.
fn list_snapshots(st: &AppState, path: Option<String>) -> anyhow::Result<Vec<SnapshotInfo>> {
    let resolved: Option<PathBuf> = match path {
        Some(p) => {
            let canon = canonical(Path::new(&p));
            Some(resolve_link(&canon).unwrap_or(canon))
        }
        None => None,
    };
    st.0.config.snapshots().list(resolved.as_deref())
}

pub async fn snapshots(State(st): State<AppState>, Query(q): Query<PathQuery>) -> Response {
    match tokio::task::spawn_blocking(move || list_snapshots(&st, q.path)).await {
        Ok(Ok(list)) => Json(list).into_response(),
        Ok(Err(e)) => failed(format!("{e:#}")),
        Err(e) => failed(e),
    }
}

pub async fn restore(State(st): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    let restore = move || {
        // After a restart the person may undo a change in a distro before any plan has looked the distro up.
        if let Ok(Some(info)) = st.0.config.snapshots().get(&id) {
            st.record_distros_of(info.files.iter().map(|f| Path::new(&f.path)));
        }
        st.0.config.restore_preview(&id)
    };
    match tokio::task::spawn_blocking(restore).await {
        Ok(Some(preview)) => Json(preview).into_response(),
        Ok(None) => not_found("snapshot"),
        Err(e) => failed(e),
    }
}
