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
    let state = st.clone();
    let to_save = settings.clone();
    // Saving touches the disk: keep it off the async worker threads. The settings lock is held from
    // before the file is written until the copy in memory is replaced, so two saves at once happen
    // one after the other and the file and the memory always hold the same settings.
    tokio::task::spawn_blocking(move || {
        let mut current = state.0.settings.lock().unwrap();
        crate::settings::save(&state.0.home, &to_save)?;
        *current = to_save;
        anyhow::Ok(())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Two saves at once must not interleave their disk write and their memory update, or the file
    /// and the served settings could end up holding different ones. So a save writes the file only
    /// while it holds the settings lock. Here a test holds the lock, standing in for another save
    /// that is midway, and the PUT must not touch the disk until it is let go.
    #[tokio::test(flavor = "multi_thread")]
    #[allow(clippy::await_holding_lock)] // The lock is held across the sleep on purpose.
    async fn put_writes_the_file_only_while_it_holds_the_settings_lock() {
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let mut settings = Settings::default();
        settings.appearance.accent = "teal".into();
        let file = crate::paths::settings_file(home.path());

        let held = st.0.settings.lock().unwrap();
        let task = tokio::spawn(put(State(st.clone()), Json(settings)));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !file.exists(),
            "the PUT wrote the file without waiting for the settings lock"
        );
        drop(held);

        task.await.unwrap().unwrap();
        assert!(file.exists());
        assert_eq!(st.0.settings.lock().unwrap().appearance.accent, "teal");
    }
}
