use super::AppState;
use crate::pty::{Session, SpawnSpec};
use crate::{auth, shell};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use marshell_protocol::api::{CreateSessionRequest, CreateSessionResponse};
use marshell_protocol::brand;
use std::path::PathBuf;

pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, (StatusCode, String)> {
    let argv = req.shell.unwrap_or_else(shell::default_shell);
    if argv.is_empty() {
        // `CommandBuilder::from_argv` would panic on an empty argv; refuse it before spawning.
        return Err((StatusCode::BAD_REQUEST, "The shell argv is empty.".into()));
    }
    let tab_id = auth::new_id("t");
    let spec = SpawnSpec {
        argv,
        cwd: req.cwd.map(PathBuf::from).or_else(dirs::home_dir),
        env: vec![
            (brand::env_var("TAB_ID"), tab_id.clone()),
            (brand::env_var("AGENT"), "shell".into()),
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
        ],
        cols: req.cols,
        rows: req.rows,
    };
    let id = tab_id.clone();
    // Spawning touches the OS (process creation): keep it off the async worker threads.
    let session = tokio::task::spawn_blocking(move || Session::spawn(id, spec))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not start the shell: {e:#}"),
            )
        })?;
    st.insert_session(session);
    Ok(Json(CreateSessionResponse { tab_id }))
}
