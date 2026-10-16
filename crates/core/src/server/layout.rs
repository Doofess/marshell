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

/// How long the saver waits after a change, so a burst of changes is one write.
const SAVE_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

/// Keeps `layout.json` in step with the open tabs. After a change it waits a moment, then saves the
/// tabs and empties the panes whose tab is gone (the pane stays, so the others keep their places).
/// A failed save is logged and tried again on the next change. Without a running tokio runtime
/// (some tests) there is nothing to spawn on and nothing is saved.
pub(super) fn spawn_layout_saver(state: &AppState) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let mut changes = state.0.registry.subscribe();
    // A weak handle, so this task does not keep a dropped core alive.
    let inner = std::sync::Arc::downgrade(&state.0);
    runtime.spawn(async move {
        while changes.changed().await.is_ok() {
            tokio::time::sleep(SAVE_DELAY).await;
            // Changes that came in during the wait are covered by the save below.
            changes.borrow_and_update();
            let Some(inner) = inner.upgrade() else {
                return;
            };
            let state = AppState(inner);
            let saved = tokio::task::spawn_blocking(move || save_tabs(&state)).await;
            match saved {
                Ok(Ok(())) => {}
                Ok(Err(err)) => tracing::warn!("could not save layout.json: {err:#}"),
                Err(err) => tracing::warn!("saving layout.json failed: {err}"),
            }
        }
    });
}

/// Writes the registry's open tabs into the layout, under the layout lock like `put_view`.
fn save_tabs(state: &AppState) -> anyhow::Result<()> {
    let mut current = state.0.layout.lock().unwrap_or_else(|e| e.into_inner());
    let mut next = current.clone();
    next.tabs = state.0.registry.saved_tabs();
    for pane in &mut next.view.panes {
        if pane
            .as_ref()
            .is_some_and(|id| !next.tabs.iter().any(|t| &t.tab_id == id))
        {
            *pane = None;
        }
    }
    // Command-state changes also wake the saver; nothing to write for those.
    if next == *current {
        return Ok(());
    }
    crate::layout::save(&state.0.home, &next)?;
    *current = next;
    Ok(())
}
