use super::AppState;
use crate::profiles::store;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::{IntegrationInfo, Profile, ProfilesDoc};

pub async fn get(State(st): State<AppState>) -> Json<ProfilesDoc> {
    Json(st.0.profiles.lock().unwrap().clone())
}

/// The shells detection found, as they were found. The profiles page resets an edited one to these.
pub async fn detected(State(st): State<AppState>) -> Json<Vec<Profile>> {
    Json(st.0.detected.clone())
}

/// Whether shell integration applies to this (possibly unsaved) profile, and the script it runs.
pub async fn integration(State(st): State<AppState>, Json(profile): Json<Profile>) -> Json<IntegrationInfo> {
    let dir = st.0.shell_dir.clone();
    Json(crate::shell::describe(&profile, dir.as_deref(), &|k| {
        std::env::var(k).ok()
    }))
}

pub async fn put(
    State(st): State<AppState>,
    Json(doc): Json<ProfilesDoc>,
) -> Result<Json<ProfilesDoc>, (StatusCode, String)> {
    store::validate(&doc).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
    let st2 = st.clone();
    // Saving touches the disk: keep it off the async worker threads.
    let saved = tokio::task::spawn_blocking(move || {
        store::save(&st2.0.home, &doc, &st2.0.detected)?;
        Ok::<_, anyhow::Error>(store::normalize(&doc, &st2.0.detected))
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    *st.0.profiles.lock().unwrap() = saved.clone();
    Ok(Json(saved))
}
