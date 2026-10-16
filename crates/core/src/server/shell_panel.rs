//! The sidebar's shell panel: one pty of its own, kept out of the tab list and out of layout.json
//! (it has no registry row) until "Open in a pane" adopts it as a tab.

use super::AppState;
use super::sessions::{resolve, spawn_into_map, start_folder};
use crate::auth;
use crate::registry::Relaunch;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

type ApiError = (StatusCode, String);

/// What adopting needs to know about the running panel shell.
pub(super) struct PanelShell {
    tab_id: String,
    profile_id: String,
    /// The profile's name, which becomes the tab's name.
    name: String,
    cwd: String,
}

#[derive(Deserialize)]
pub struct PanelRequest {
    cwd: String,
    cols: u16,
    rows: u16,
}

#[derive(Serialize)]
pub struct PanelResponse {
    tab_id: String,
}

/// True while the session is still in the sessions map and its process has not exited.
fn is_alive(st: &AppState, tab_id: &str) -> bool {
    st.session(tab_id).is_some_and(|s| !s.has_exited())
}

/// The id of the panel shell if it is alive. A stored shell that has exited is forgotten.
fn alive_panel(st: &AppState) -> Option<String> {
    let mut panel = st.0.shell_panel.lock().unwrap();
    let tab_id = panel.as_ref()?.tab_id.clone();
    if is_alive(st, &tab_id) {
        return Some(tab_id);
    }
    *panel = None;
    // The finished process is no use to anyone; drop it from the map too.
    st.remove_session(&tab_id);
    None
}

/// Starts the default shell in the folder, or returns the one that is already running.
/// It never changes folder later: a second call with another folder returns the same shell.
pub async fn open(State(st): State<AppState>, Json(req): Json<PanelRequest>) -> Result<Json<PanelResponse>, ApiError> {
    if let Some(tab_id) = alive_panel(&st) {
        return Ok(Json(PanelResponse { tab_id }));
    }
    let run = resolve(&st, None, None, None, false)?;
    let cwd = start_folder(&run, Some(req.cwd));
    let tab_id = auth::new_id("t");
    let shell = PanelShell {
        tab_id: tab_id.clone(),
        profile_id: run.profile_id.clone(),
        name: run.profile_name.clone(),
        cwd: cwd.clone(),
    };
    // The panel has no number: it only gets one when it is adopted.
    spawn_into_map(&st, &tab_id, 0, run, None, cwd, (req.cols, req.rows), None).await?;
    // Two requests can both start a shell; the first to store its id wins.
    let mut panel = st.0.shell_panel.lock().unwrap();
    if let Some(other) = panel.as_ref().map(|p| p.tab_id.clone())
        && is_alive(&st, &other)
    {
        drop(panel);
        if let Some(extra) = st.remove_session(&tab_id) {
            extra.kill();
        }
        return Ok(Json(PanelResponse { tab_id: other }));
    }
    *panel = Some(shell);
    Ok(Json(PanelResponse { tab_id }))
}

/// Turns the panel shell into a tab at the end of the list, still running. The panel is free again.
pub async fn adopt(State(st): State<AppState>) -> Result<Json<PanelResponse>, ApiError> {
    if alive_panel(&st).is_none() {
        return Err((StatusCode::NOT_FOUND, "There is no shell in the panel.".into()));
    }
    let Some(shell) = st.0.shell_panel.lock().unwrap().take() else {
        // Another adopt took it between the check and here.
        return Err((StatusCode::NOT_FOUND, "There is no shell in the panel.".into()));
    };
    let tab_id = shell.tab_id;
    st.0.registry.adopt(
        &tab_id,
        Relaunch {
            profile_id: shell.profile_id,
            agent: None,
            cwd: shell.cwd,
            name: shell.name,
        },
    );
    Ok(Json(PanelResponse { tab_id }))
}
