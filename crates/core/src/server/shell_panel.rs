//! The sidebar's shell panel: one pty of its own, kept out of the tab list and out of layout.json
//! (it has no registry row) until "Open in a pane" adopts it as a tab.

use super::AppState;
use super::sessions::{resolve, spawn_into_map, start_folder};
use crate::auth;
use crate::pty::Session;
use crate::registry::{Registry, Relaunch};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

type ApiError = (StatusCode, String);

/// The generation the panel shell's exit callback carries, and the row gets when the shell is
/// adopted. The panel has no row to restart or resume before that, so one value is enough.
pub(super) const PANEL_GENERATION: u64 = 0;

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

impl AppState {
    /// The id of the panel shell, while there is one. It may have exited since.
    pub(super) fn panel_tab(&self) -> Option<String> {
        self.0.shell_panel.lock().unwrap().as_ref().map(|p| p.tab_id.clone())
    }

    /// Whether the panel shell is running in a folder `in_folder` accepts.
    pub(crate) fn panel_running_where(&self, in_folder: impl Fn(&str) -> bool) -> bool {
        let panel = self.0.shell_panel.lock().unwrap();
        panel
            .as_ref()
            .is_some_and(|p| in_folder(&p.cwd) && is_alive(self, &p.tab_id))
    }

    /// Takes the panel shell out of the panel (so it is free again) when it started in a folder
    /// `in_folder` accepts, and returns its id. The caller closes the session.
    pub(crate) fn take_panel_where(&self, in_folder: impl Fn(&str) -> bool) -> Option<String> {
        let mut panel = self.0.shell_panel.lock().unwrap();
        if !panel.as_ref().is_some_and(|p| in_folder(&p.cwd)) {
            return None;
        }
        panel.take().map(|p| p.tab_id)
    }
}

/// Starts the default shell in the folder, or returns the one that is already running.
/// It never changes folder later: a second call with another folder returns the same shell.
pub async fn open(State(st): State<AppState>, Json(req): Json<PanelRequest>) -> Result<Json<PanelResponse>, ApiError> {
    if let Some(tab_id) = alive_panel(&st) {
        return Ok(Json(PanelResponse { tab_id }));
    }
    let run = resolve(&st, None, None, None, None, None, None).await?;
    let cwd = start_folder(&run, Some(req.cwd));
    let tab_id = auth::new_id("t");
    let shell = PanelShell {
        tab_id: tab_id.clone(),
        profile_id: run.profile_id.clone(),
        name: run.profile_name.clone(),
        cwd: cwd.clone(),
    };
    // The panel has no number: it only gets one when it is adopted.
    spawn_into_map(
        &st,
        &tab_id,
        0,
        PANEL_GENERATION,
        run,
        None,
        cwd,
        (req.cols, req.rows),
        None,
    )
    .await?;
    // Two requests can both start a shell; the first to store its id wins.
    let mut panel = st.0.shell_panel.lock().unwrap();
    if let Some(other) = panel.as_ref().map(|p| p.tab_id.clone()) {
        if is_alive(&st, &other) {
            drop(panel);
            if let Some(extra) = st.remove_session(&tab_id) {
                extra.kill();
            }
            return Ok(Json(PanelResponse { tab_id: other }));
        }
        // The stored one has exited: ours replaces it, and the finished process leaves the map.
        st.remove_session(&other);
    }
    *panel = Some(shell);
    Ok(Json(PanelResponse { tab_id }))
}

/// Turns the panel shell into a tab at the end of the list, still running. The panel is free again.
pub async fn adopt(State(st): State<AppState>) -> Result<Json<PanelResponse>, ApiError> {
    match take_and_adopt(&st, None) {
        Some(tab_id) => Ok(Json(PanelResponse { tab_id })),
        None => Err((StatusCode::NOT_FOUND, "There is no shell in the panel.".to_string())),
    }
}

/// Takes the panel shell out of the panel and registers it as a tab, in one critical section, so
/// when the route and the foreground watcher both try, exactly one adopts and the other gets None.
/// With `only`, the panel is taken only while it is still that shell: the watcher's reading may be
/// older than the panel it finds. None also when the shell has already exited.
pub(super) fn take_and_adopt(st: &AppState, only: Option<&str>) -> Option<String> {
    let mut panel = st.0.shell_panel.lock().unwrap();
    if only.is_some_and(|only| panel.as_ref().is_none_or(|p| p.tab_id != only)) {
        return None;
    }
    let shell = panel.take()?;
    let Some(session) = st.session(&shell.tab_id).filter(|s| !s.has_exited()) else {
        st.remove_session(&shell.tab_id);
        return None;
    };
    let tab_id = shell.tab_id;
    adopt_session(
        &st.0.registry,
        &session,
        Relaunch {
            profile_id: shell.profile_id,
            agent: None,
            cwd: shell.cwd,
            mode: None,
            name: shell.name,
            preset: None,
        },
    );
    Some(tab_id)
}

/// Gives a running pty a registry row. If the process exited before the row existed, its exit
/// callback found nothing to mark, so mark the row here. The exit code is read after the row is
/// added: `None` then means the callback has not run yet and will find the row.
fn adopt_session(registry: &Registry, session: &Session, relaunch: Relaunch) {
    let tab_id = session.id().to_string();
    registry.adopt(&tab_id, relaunch, PANEL_GENERATION);
    if let Some(code) = session.exit_code() {
        registry.mark_exited(&tab_id, PANEL_GENERATION, code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::SpawnSpec;
    use marshell_protocol::api::SessionState;

    #[test]
    fn a_shell_that_exited_before_adoption_is_adopted_as_exited() {
        let argv: Vec<String> = if cfg!(windows) {
            ["cmd", "/c", "exit 3"].map(String::from).to_vec()
        } else {
            ["sh", "-c", "exit 3"].map(String::from).to_vec()
        };
        // No exit callback: it already ran in the race, and found no row to mark.
        let session = Session::spawn(
            "t1".into(),
            SpawnSpec {
                argv,
                cwd: None,
                env: Vec::new(),
                env_remove: Vec::new(),
                cols: 80,
                rows: 24,
                on_exit: None,
                on_mark: None,
                record: None,
            },
        )
        .unwrap();
        let start = std::time::Instant::now();
        while session.exit_code().is_none() {
            assert!(start.elapsed() < std::time::Duration::from_secs(10), "it never exited");
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

        let registry = Registry::new();
        let relaunch = Relaunch {
            profile_id: "p".into(),
            agent: None,
            cwd: String::new(),
            mode: None,
            name: "Shell".into(),
            preset: None,
        };
        adopt_session(&registry, &session, relaunch);
        let row = registry.snapshot().sessions.remove(0);
        assert_eq!(row.tab_id, "t1");
        assert_eq!(row.state, SessionState::Exited { code: 3 });
    }
}
