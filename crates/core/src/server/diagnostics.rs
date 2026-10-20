//! `/v1/diagnostics/*`: the detailed-logs switch and the redacted export.

use super::AppState;
use crate::diagnostics::{export, log};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use marshell_protocol::diagnostics::{ExportFile, ExportPreview, LoggingState, PerfSnapshot};
use serde::Deserialize;

pub async fn get_logging() -> Json<LoggingState> {
    Json(log::state())
}

#[derive(Deserialize)]
pub struct SetLogging {
    detailed: bool,
}

pub async fn put_logging(Json(body): Json<SetLogging>) -> Result<Json<LoggingState>, (StatusCode, String)> {
    if !log::set_detailed(body.detailed) {
        return Err((StatusCode::CONFLICT, "Logging is not running.".to_string()));
    }
    Ok(Json(log::state()))
}

pub async fn get_perf(State(st): State<AppState>) -> Result<Json<PerfSnapshot>, (StatusCode, String)> {
    // The process list is read with blocking calls.
    tokio::task::spawn_blocking(move || crate::diagnostics::perf::snapshot(&st))
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not read the process list: {e}"),
            )
        })
}

/// The same reply for an id that never existed, one that expired or was replaced, and a name that is not in it.
fn expired() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "This export has expired. Prepare it again.")
}

/// Builds the export and keeps it in memory. A second call replaces the first.
pub async fn post_export(State(st): State<AppState>) -> Result<Json<ExportPreview>, (StatusCode, String)> {
    let st2 = st.clone();
    // Reads files and may run `--version`.
    let files = tokio::task::spawn_blocking(move || export::collect(&st2))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not prepare the export: {e}"),
            )
        })?;
    let listed: Vec<ExportFile> = files
        .iter()
        .map(|f| ExportFile {
            name: f.name.clone(),
            bytes: f.bytes.len() as u64,
        })
        .collect();
    let total_bytes = listed.iter().map(|f| f.bytes).sum();
    let id = st.previews().put(files);
    Ok(Json(ExportPreview {
        id,
        files: listed,
        total_bytes,
    }))
}

#[derive(Deserialize)]
pub struct ExportName {
    name: String,
}

/// One file of the export, as it will be zipped.
pub async fn get_export_file(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ExportName>,
) -> Response {
    let Some(files) = st.previews().get(&id) else {
        return expired().into_response();
    };
    match files.iter().find(|f| f.name == q.name) {
        Some(file) => (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            file.bytes.clone(),
        )
            .into_response(),
        None => expired().into_response(),
    }
}

/// The whole export as a zip.
pub async fn get_export_zip(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(files) = st.previews().get(&id) else {
        return expired().into_response();
    };
    match tokio::task::spawn_blocking(move || export::zip(&files)).await {
        Ok(Ok(bytes)) => ([(header::CONTENT_TYPE, "application/zip")], bytes).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not build the zip: {e:#}"),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not build the zip: {e}"),
        )
            .into_response(),
    }
}
