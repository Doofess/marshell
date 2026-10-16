use super::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::{LayoutDoc, ViewLayout};

pub async fn get(State(st): State<AppState>) -> Json<LayoutDoc> {
    Json(st.0.layout.lock().unwrap().clone())
}

/// Replaces the window layout and keeps the saved tabs as they are.
pub async fn put_view(
    State(st): State<AppState>,
    Json(view): Json<ViewLayout>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::layout::validate_view(&view).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
    let state = st.clone();
    // Saving touches the disk: keep it off the async worker threads. As with the settings, the lock is held
    // from before the file is written until the copy in memory is replaced, so the file and the memory agree.
    tokio::task::spawn_blocking(move || {
        let mut current = state.0.layout.lock().unwrap();
        let mut next = current.clone();
        next.view = view;
        crate::layout::save(&state.0.home, &next)?;
        *current = next;
        anyhow::Ok(())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    Ok(StatusCode::NO_CONTENT)
}
