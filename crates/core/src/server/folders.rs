//! `GET /v1/folders/check`: the launcher asks whether typed text is a folder it can start a session in.

use axum::Json;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::folders::{self, FolderRefusal};

#[derive(Deserialize)]
pub struct CheckQuery {
    path: String,
}

pub async fn check(Query(q): Query<CheckQuery>) -> Response {
    // The check looks at the disk: keep it off the async worker threads.
    let result = tokio::task::spawn_blocking(move || folders::check(&q.path)).await;
    match result {
        Ok(Ok(info)) => Json(info).into_response(),
        Ok(Err(FolderRefusal::NotFullPath)) => (StatusCode::BAD_REQUEST, "not a full path").into_response(),
        Ok(Err(FolderRefusal::Missing)) => (StatusCode::NOT_FOUND, "no folder there").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
