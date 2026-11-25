use super::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::{DefaultFolder, FoundFile, Settings};

pub async fn get(State(st): State<AppState>) -> Json<Settings> {
    Json(st.0.settings.lock().unwrap().clone())
}

/// The folder new sessions start in when nothing names one: where it is, whether a folder is there now, and whether
/// Settings chose it (else it is Marshell's own).
pub async fn default_folder(State(st): State<AppState>) -> Json<DefaultFolder> {
    let chosen = st.0.settings.lock().unwrap().default_folder.is_some();
    let path = st.default_folder_path().map(|p| p.to_string_lossy().into_owned());
    let exists = st.default_folder_now().await.is_some();
    Json(DefaultFolder { path, exists, chosen })
}

/// Test hook: a test can park the folder check of one folder, to look at what else can run meanwhile.
#[cfg(test)]
static CHECK_GATE: std::sync::Mutex<Option<CheckGate>> = std::sync::Mutex::new(None);

/// The folder to park, what says the check has started, and what lets it go on.
#[cfg(test)]
type CheckGate = (String, std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>);

fn before_folder_check(_dir: &str) {
    #[cfg(test)]
    {
        let mut gate = CHECK_GATE.lock().unwrap();
        if gate.as_ref().is_some_and(|(dir, ..)| dir == _dir)
            && let Some((_, entered, release)) = gate.take()
        {
            drop(gate);
            let _ = entered.send(());
            let _ = release.recv();
        }
    }
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
    let refused = tokio::task::spawn_blocking(move || {
        // A folder that is already the backup folder isn't asked again here: an unplugged drive must not stop
        // the theme from saving. Every copy checks the folder anyway. The check reads the disk and can hang on a
        // network drive, so it runs before the settings lock is taken, which other requests wait for.
        // The default folder is looked at the same way: only a changed one, and never under the lock.
        let (saved_dir, saved_default) = {
            let saved = state.0.settings.lock().unwrap();
            (saved.history_backup_dir.clone(), saved.default_folder.clone())
        };
        if let Some(dir) = &to_save.history_backup_dir
            && saved_dir.as_ref() != Some(dir)
        {
            before_folder_check(dir);
            if let Err(refusal) =
                crate::history::storage::backup::check_folder(&state.0.home, std::path::Path::new(dir))
            {
                return anyhow::Ok(Some(refusal.message().to_owned()));
            }
        }
        // A new default folder must be there already: nothing is made for a path that arrives in a request.
        if let Some(dir) = &to_save.default_folder
            && saved_default.as_ref() != Some(dir)
            && !std::path::Path::new(dir).is_dir()
        {
            return anyhow::Ok(Some(crate::settings::FOLDER_MISSING.to_owned()));
        }
        let back_to_builtin = saved_default.is_some() && to_save.default_folder.is_none();
        let mut current = state.0.settings.lock().unwrap();
        let backup_changed = current.history_backup_dir != to_save.history_backup_dir;
        crate::settings::save(&state.0.home, &to_save)?;
        let import = to_save.history_import;
        let limits_changed = current.history_limit_gb != to_save.history_limit_gb
            || current.history_limit_days != to_save.history_limit_days;
        *current = to_save;
        if let Some(importer) = state.0.importer.get() {
            importer.set_on(import);
        }
        // The file is saved, which is where the cleanup and the backup read their settings from.
        if let Some(storage) = state.storage() {
            if limits_changed {
                storage.nudge(crate::history::storage::Nudge::Cleanup);
            } else if backup_changed {
                storage.nudge(crate::history::storage::Nudge::Tick);
            }
        }
        // No lock is held for the disk work below.
        drop(current);
        // Back to Marshell's own folder: make it, if the core has one. The path comes from the core's config, never
        // from the request, and a failure is only logged: the save stands and a start falls back to the home folder.
        if back_to_builtin
            && let Some(own) = &state.0.default_folder
            && let Err(e) = std::fs::create_dir_all(own)
        {
            tracing::warn!("could not make the default folder: {e}");
        }
        anyhow::Ok(None)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    if let Some(message) = refused {
        return Err((StatusCode::BAD_REQUEST, message));
    }
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

        let Json(answer) = task.await.unwrap().unwrap();
        assert_eq!(answer.appearance.accent, "teal");
        assert!(file.exists());
        assert_eq!(st.0.settings.lock().unwrap().appearance.accent, "teal");
    }

    /// The folder check can hang on a network drive, so it must not run while the settings lock is held: that
    /// lock is also taken by GET /v1/settings and by every hook event. The check is parked here, and the lock
    /// must be free meanwhile.
    #[tokio::test(flavor = "multi_thread")]
    async fn put_checks_the_backup_folder_without_the_settings_lock() {
        let home = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let dir = backup.path().to_string_lossy().into_owned();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let settings = Settings {
            history_backup_dir: Some(dir.clone()),
            ..Settings::default()
        };

        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        *CHECK_GATE.lock().unwrap() = Some((dir.clone(), entered_tx, release_rx));
        let task = tokio::spawn(put(State(st.clone()), Json(settings)));
        tokio::task::spawn_blocking(move || entered_rx.recv_timeout(Duration::from_secs(10)))
            .await
            .unwrap()
            .expect("the folder check never started");

        {
            let free = st.0.settings.try_lock();
            assert!(free.is_ok(), "the folder check runs while the settings lock is held");
        }
        assert!(!crate::paths::settings_file(home.path()).exists());
        release_tx.send(()).unwrap();

        let Json(answer) = task.await.unwrap().unwrap();
        assert_eq!(answer.history_backup_dir.as_deref(), Some(dir.as_str()));
        assert_eq!(
            st.0.settings.lock().unwrap().history_backup_dir.as_deref(),
            Some(dir.as_str())
        );
        assert!(crate::paths::settings_file(home.path()).exists());
    }

    /// A theme change must save while the default folder's drive is unplugged: a folder that is already the saved one
    /// is never looked at again.
    #[tokio::test(flavor = "multi_thread")]
    async fn put_does_not_look_at_an_unchanged_default_folder() {
        let home = tempfile::tempdir().unwrap();
        let gone = home.path().join("gone");
        let dir = gone.to_string_lossy().into_owned();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        st.0.settings.lock().unwrap().default_folder = Some(dir.clone());
        let mut settings = Settings {
            default_folder: Some(dir.clone()),
            ..Settings::default()
        };
        settings.appearance.accent = "teal".into();

        let Json(answer) = put(State(st.clone()), Json(settings)).await.unwrap();
        assert_eq!(answer.appearance.accent, "teal");
        let saved = st.0.settings.lock().unwrap().clone();
        assert_eq!(saved.appearance.accent, "teal");
        assert_eq!(saved.default_folder.as_deref(), Some(dir.as_str()));
        assert!(!gone.exists());
    }
}
