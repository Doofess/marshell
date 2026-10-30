//! A repo's `marshell.json`: read it, allow it, and start `setup` or `run` as a session. The core runs a script
//! only when the hash the person was shown is the file's current hash and that exact file was allowed.

use super::AppState;
use super::sessions::start_script;
use crate::{projects, scripts};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use marshell_protocol::api::{AllowScriptsRequest, CreateSessionResponse, RepoScripts, RunScriptRequest, ScriptKind};
use serde::Deserialize;
use std::path::{Path, PathBuf};

type ApiError = (StatusCode, String);

const CHANGED: &str = "marshell.json changed. Check the scripts again.";
const NOT_ALLOWED: &str = "This repo's scripts aren't allowed yet.";

#[derive(Deserialize)]
pub struct ScriptsQuery {
    cwd: String,
}

fn failed(err: tokio::task::JoinError) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

/// Reads the checkout holding `cwd`, off the async workers.
async fn read(st: &AppState, cwd: String) -> Result<RepoScripts, ApiError> {
    if !Path::new(&cwd).is_absolute() {
        return Err((StatusCode::BAD_REQUEST, "not a full path".into()));
    }
    let home = st.0.home.clone();
    tokio::task::spawn_blocking(move || scripts::read(Path::new(&cwd), &home))
        .await
        .map_err(failed)
}

pub async fn get(State(st): State<AppState>, Query(q): Query<ScriptsQuery>) -> Result<Json<RepoScripts>, ApiError> {
    Ok(Json(read(&st, q.cwd).await?))
}

/// Allows the repo's scripts, but only as the file is now: a different hash is a 409 and trusts nothing.
pub async fn allow(
    State(st): State<AppState>,
    Json(req): Json<AllowScriptsRequest>,
) -> Result<Json<RepoScripts>, ApiError> {
    let current = read(&st, req.cwd.clone()).await?;
    // A file that can't be read is never allowed.
    if let Some(problem) = current.problem {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, problem));
    }
    if current.hash.as_deref() != Some(req.hash.as_str()) {
        return Err((StatusCode::CONFLICT, CHANGED.into()));
    }
    let home = st.0.home.clone();
    let root = PathBuf::from(&current.root);
    let hash = req.hash;
    let saved = tokio::task::spawn_blocking(move || {
        // A linked worktree is trusted under its main checkout, as `scripts::read` looks it up.
        let repo = projects::repo_root(&root).unwrap_or(root);
        scripts::allow(&home, &repo, &hash)
    })
    .await
    .map_err(failed)?;
    saved.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    Ok(Json(read(&st, req.cwd).await?))
}

/// Starts `setup` or `run` in the checkout's root as a session named `Setup` or `Run`.
pub async fn run(
    State(st): State<AppState>,
    Json(req): Json<RunScriptRequest>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    let (kind, name) = match req.script {
        ScriptKind::Setup => (ScriptKind::Setup, "Setup"),
        ScriptKind::Run => (ScriptKind::Run, "Run"),
        ScriptKind::Teardown => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Teardown runs when a task is removed.".into(),
            ));
        }
    };
    // Read again: the file may have changed since the person was shown it.
    let current = read(&st, req.cwd).await?;
    if current.hash.as_deref() != Some(req.hash.as_str()) {
        return Err((StatusCode::CONFLICT, CHANGED.into()));
    }
    if !current.trusted {
        return Err((StatusCode::FORBIDDEN, NOT_ALLOWED.into()));
    }
    let script = match kind {
        ScriptKind::Setup => current.setup,
        _ => current.run,
    };
    let Some(script) = script else {
        let which = if kind == ScriptKind::Setup { "setup" } else { "run" };
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("marshell.json has no {which} script for this computer."),
        ));
    };
    let argv = scripts::argv(&scripts::default_shell(), &script);
    let size = (req.cols.unwrap_or(80), req.rows.unwrap_or(24));
    Ok(Json(start_script(&st, argv, name, current.root, size, kind).await?))
}
