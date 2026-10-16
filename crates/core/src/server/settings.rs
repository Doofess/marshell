use super::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::{FoundFile, Settings};

pub async fn get(State(st): State<AppState>) -> Json<Settings> {
    Json(st.0.settings.lock().unwrap().clone())
}

pub async fn put(
    State(st): State<AppState>,
    Json(settings): Json<Settings>,
) -> Result<Json<Settings>, (StatusCode, String)> {
    crate::settings::validate(&settings).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
    let home = st.0.home.clone();
    let to_save = settings.clone();
    // Saving touches the disk: keep it off the async worker threads.
    tokio::task::spawn_blocking(move || crate::settings::save(&home, &to_save))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    *st.0.settings.lock().unwrap() = settings.clone();
    Ok(Json(settings))
}

/// Windows Terminal's settings files, for the theme import. Read-only; on other platforms,
/// or when there is no local app-data folder, the list is empty.
pub async fn windows_terminal() -> Result<Json<Vec<FoundFile>>, (StatusCode, String)> {
    let found = tokio::task::spawn_blocking(|| match dirs::data_local_dir() {
        Some(local) if cfg!(windows) => crate::wt::find(&local),
        _ => Vec::new(),
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(found))
}
