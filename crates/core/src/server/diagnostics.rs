//! `/v1/diagnostics/*`: the detailed-logs switch (more routes come with the export).

use super::AppState;
use crate::diagnostics::log;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::diagnostics::{LoggingState, PerfSnapshot};
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
