//! The drafts of text tabs: the unsaved edits a tab keeps in Marshell's home. These routes never
//! touch the file a draft is about, so they only read the path as a string.

use super::{AppState, files};
use crate::userfiles::drafts::{Drafts, PutError};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{Draft, DraftList};
use serde::Deserialize;
use std::path::Path;

const BAD_PATH: &str = "That path can't be saved.";
const FULL: &str = "Marshell keeps at most 500 drafts. Save some files first.";
const PDF_FULL: &str = "Marshell keeps at most 1 GB of unsaved PDF notes. Save some PDFs first.";
/// The most bytes a draft's text may take. A draft holds text as UTF-8, and a UTF-16 file at the cap
/// (two bytes a character on disk) can take three bytes a character here.
pub(super) const DRAFT_TEXT_CAP: u64 = files::TEXT_CAP / 2 * 3;

#[derive(Deserialize)]
pub struct DraftQuery {
    path: String,
}

/// What is wrong with the draft, as an answer. None when it may be kept.
fn invalid(draft: &Draft) -> Option<Response> {
    if files::refused(&draft.path) || !Path::new(&draft.path).is_absolute() {
        return Some((StatusCode::BAD_REQUEST, BAD_PATH).into_response());
    }
    if !["utf-8", "utf-8-bom", "utf-16le", "utf-16be"].contains(&draft.encoding.as_str()) {
        return Some((StatusCode::BAD_REQUEST, "The encoding is not one Marshell knows.").into_response());
    }
    if !["lf", "crlf"].contains(&draft.eol.as_str()) {
        return Some((StatusCode::BAD_REQUEST, "The eol is not one Marshell knows.").into_response());
    }
    if draft.text.len() as u64 > DRAFT_TEXT_CAP || draft.base_text.len() as u64 > DRAFT_TEXT_CAP {
        return Some(StatusCode::PAYLOAD_TOO_LARGE.into_response());
    }
    None
}

/// GET /v1/drafts: the files that have a draft, newest edit first.
pub async fn list(State(st): State<AppState>) -> Response {
    let home = st.home().to_path_buf();
    match tokio::task::spawn_blocking(move || Drafts::new(&home).list()).await {
        Ok(drafts) => Json(DraftList { drafts }).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// GET /v1/drafts/one?path=: the file's draft, or 204 when it has none. Not a 404: the editor asks for every file it opens, and
/// a webview logs each 404 as an error in its console.
pub async fn get(State(st): State<AppState>, Query(q): Query<DraftQuery>) -> Response {
    let home = st.home().to_path_buf();
    match tokio::task::spawn_blocking(move || Drafts::new(&home).get(&q.path)).await {
        Ok(Some(draft)) => Json(draft).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// PUT /v1/drafts/one: keeps the draft, replacing the file's earlier one.
pub async fn put(State(st): State<AppState>, Json(draft): Json<Draft>) -> Response {
    if let Some(no) = invalid(&draft) {
        return no;
    }
    let home = st.home().to_path_buf();
    match tokio::task::spawn_blocking(move || Drafts::new(&home).put(&draft)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(PutError::Full)) => (StatusCode::SERVICE_UNAVAILABLE, FULL).into_response(),
        // A text draft never puts bytes in the PDF budget.
        Ok(Err(PutError::PdfBudget)) => (StatusCode::SERVICE_UNAVAILABLE, PDF_FULL).into_response(),
        Ok(Err(PutError::Io(e))) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// DELETE /v1/drafts/one?path=: drops the file's draft. There being none is fine.
pub async fn delete(State(st): State<AppState>, Query(q): Query<DraftQuery>) -> Response {
    let home = st.home().to_path_buf();
    match tokio::task::spawn_blocking(move || Drafts::new(&home).remove(&q.path)).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct PdfPut {
    path: String,
    /// The hash of the bytes the notes started from, or `none`.
    base: String,
    edited: u64,
}

/// GET /v1/drafts/pdf?path=: the PDF's unsaved notes as a PDF, with the hash they started from in
/// `X-Base-Hash` (absent for none) and the time of the last edit in `X-Edited-Ms`; 204 for none.
pub async fn get_pdf(State(st): State<AppState>, Query(q): Query<DraftQuery>) -> Response {
    let home = st.home().to_path_buf();
    match tokio::task::spawn_blocking(move || Drafts::new(&home).get_pdf(&q.path)).await {
        Ok(Some((meta, bytes))) => {
            let mut res = (StatusCode::OK, bytes).into_response();
            let headers = res.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
            if let Some(hash) = meta.base_hash.as_deref().and_then(|h| HeaderValue::from_str(h).ok()) {
                headers.insert("x-base-hash", hash);
            }
            headers.insert("x-edited-ms", HeaderValue::from(meta.edited_ms));
            res
        }
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// PUT /v1/drafts/pdf?path=&base=<64 hex|none>&edited=<ms>, the body the PDF with its notes.
pub async fn put_pdf(State(st): State<AppState>, Query(q): Query<PdfPut>, body: Bytes) -> Response {
    let hex = q.base.len() == 64 && q.base.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if files::refused(&q.path) || !Path::new(&q.path).is_absolute() || !files::is_pdf(&q.path) {
        return (StatusCode::BAD_REQUEST, BAD_PATH).into_response();
    }
    if !hex && q.base != "none" {
        return (StatusCode::BAD_REQUEST, "The base hash is not one Marshell knows.").into_response();
    }
    let home = st.home().to_path_buf();
    let done = tokio::task::spawn_blocking(move || {
        let base = (q.base != "none").then_some(q.base.as_str());
        Drafts::new(&home).put_pdf(&q.path, base, q.edited, &body)
    })
    .await;
    match done {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(PutError::Full)) => (StatusCode::SERVICE_UNAVAILABLE, FULL).into_response(),
        Ok(Err(PutError::PdfBudget)) => (StatusCode::SERVICE_UNAVAILABLE, PDF_FULL).into_response(),
        Ok(Err(PutError::Io(e))) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
