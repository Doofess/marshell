use super::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{ResumeError, TaskPlanRequest};
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
