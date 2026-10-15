use super::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::ProjectInfo;

pub async fn get(State(st): State<AppState>) -> Result<Json<Vec<ProjectInfo>>, (StatusCode, String)> {
    // Listing reads files and checks folders: keep it off the async worker threads.
    let projects = tokio::task::spawn_blocking(move || crate::projects::list(&st.0.home))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(projects))
}
