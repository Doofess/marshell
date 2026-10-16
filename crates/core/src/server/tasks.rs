use super::AppState;
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{MergeRequest, ResumeError, TaskPlanRequest};
use std::path::Path;

fn refused(reason: String) -> Response {
    (StatusCode::CONFLICT, Json(ResumeError { reason })).into_response()
}

fn failed(err: tokio::task::JoinError) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The tasks whose folder still exists. A task whose folder was deleted stays in the file.
pub async fn list(State(st): State<AppState>) -> Response {
    let listed = tokio::task::spawn_blocking(move || {
        let mut tasks = crate::tasks::list(&st.0.home);
        tasks.retain(|t| Path::new(&t.path).exists());
        tasks
    })
    .await;
    match listed {
        Ok(tasks) => Json(tasks).into_response(),
        Err(e) => failed(e),
    }
}

/// What creating the task would run, without running it.
pub async fn plan(State(st): State<AppState>, Json(req): Json<TaskPlanRequest>) -> Response {
    let planned = tokio::task::spawn_blocking(move || {
        crate::tasks::plan_create(&st.0.home, Path::new(&req.cwd), &req.name, now_ms())
    })
    .await;
    match planned {
        Ok(Ok((plan, _steps))) => Json(plan).into_response(),
        Ok(Err(reason)) => refused(reason),
        Err(e) => failed(e),
    }
}

pub async fn create(State(st): State<AppState>, Json(req): Json<TaskPlanRequest>) -> Response {
    let made = tokio::task::spawn_blocking(move || crate::tasks::create(&st.0.home, &req, now_ms())).await;
    match made {
        Ok(Ok(task)) => Json(task).into_response(),
        Ok(Err(reason)) => refused(reason),
        Err(e) => failed(e),
    }
}

/// The task's changes against its base branch. 404 for an id we don't know.
pub async fn changes(State(st): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    let read = tokio::task::spawn_blocking(move || {
        crate::tasks::find(&st.0.home, &id).map(|task| crate::tasks::changes(&task))
    })
    .await;
    match read {
        Ok(Some(Ok(changes))) => Json(changes).into_response(),
        Ok(Some(Err(reason))) => refused(reason),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => failed(e),
    }
}

/// What merging the task would run, and what stops it. 404 for an id we don't know.
pub async fn merge_plan(
    State(st): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(req): Json<MergeRequest>,
) -> Response {
    let planned = tokio::task::spawn_blocking(move || {
        crate::tasks::find(&st.0.home, &id).map(|task| crate::tasks::plan_merge(&task, &req).0)
    })
    .await;
    match planned {
        Ok(Some(plan)) => Json(plan).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => failed(e),
    }
}

/// Merges the task into its base branch. A conflict or a git failure is an answer, not an error.
pub async fn merge(
    State(st): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(req): Json<MergeRequest>,
) -> Response {
    let merged = tokio::task::spawn_blocking(move || {
        crate::tasks::find(&st.0.home, &id).map(|task| crate::tasks::merge(&st, &task, &req))
    })
    .await;
    match merged {
        Ok(Some(outcome)) => Json(outcome).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => failed(e),
    }
}
