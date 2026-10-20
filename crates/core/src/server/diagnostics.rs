//! `/v1/diagnostics/*`: the detailed-logs switch (more routes come with the export).

use crate::diagnostics::log;
use axum::Json;
use axum::http::StatusCode;
use marshell_protocol::diagnostics::LoggingState;
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
