//! A project's references: read them for the project holding a folder, and replace them. The file lives under the
//! Marshell home and no request names its location. A linked worktree reads and saves its main repo's references.

use super::AppState;
use crate::{projects, references};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use marshell_protocol::api::{ProjectReferences, ReferencesUpdate};
use serde::Deserialize;
use std::path::{Path, PathBuf};

type ApiError = (StatusCode, String);

#[derive(Deserialize)]
pub struct ReferencesQuery {
    cwd: String,
}

fn failed(err: tokio::task::JoinError) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn not_a_full_path() -> ApiError {
    (StatusCode::BAD_REQUEST, "not a full path".into())
}

/// The references of the project holding `cwd`.
pub async fn get(
    State(st): State<AppState>,
    Query(q): Query<ReferencesQuery>,
) -> Result<Json<ProjectReferences>, ApiError> {
    if !Path::new(&q.cwd).is_absolute() {
        return Err(not_a_full_path());
    }
    let home = st.0.home.clone();
    let found = tokio::task::spawn_blocking(move || {
        let root = PathBuf::from(projects::project_of(&q.cwd));
        references::info(&home, &root)
    })
    .await
    .map_err(failed)?;
    found
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))
}

/// Replaces the toggle and aliases of the project holding `root`. A refusal is a 422 with the sentence to show.
pub async fn put(State(st): State<AppState>, Json(req): Json<ReferencesUpdate>) -> Result<StatusCode, ApiError> {
    if !Path::new(&req.root).is_absolute() {
        return Err(not_a_full_path());
    }
    let home = st.0.home.clone();
    tokio::task::spawn_blocking(move || {
        if !Path::new(&req.root).is_dir() {
            return Err((StatusCode::BAD_REQUEST, format!("\"{}\" isn't a folder.", req.root)));
        }
        // Whatever folder of the project was named, the file belongs to the project's root.
        let update = ReferencesUpdate {
            root: projects::project_of(&req.root),
            ..req
        };
        references::save(&home, &update).map_err(|sentence| (StatusCode::UNPROCESSABLE_ENTITY, sentence))
    })
    .await
    .map_err(failed)??;
    Ok(StatusCode::NO_CONTENT)
}
