//! Restart starts an agent session again in another mode, in the same row. Every refusal
//! comes before the old process is touched.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn state_in(home: &Path, agent_on_path: fn(&str) -> bool) -> AppState {
    common::tie_children_to_test();
    let profile = Profile {
        id: "sleeper".into(),
        name: "sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![profile],
    )
    .with_agent_check(agent_on_path)
}

fn api(method: &str, path: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn sessions(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    json_of(res).await
}

/// Starts a session in `cwd`, an agent one when `agent` is given, and returns its tab id.
async fn create(app: &axum::Router, cwd: &Path, agent: Option<&str>, mode: Option<&str>) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy(),
        "agent": agent, "mode": mode,
    });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn restart(app: &axum::Router, tab: &str, mode: &str) -> axum::response::Response {
    let body = serde_json::json!({ "mode": mode, "cols": 80, "rows": 24 });
    app.clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/restart"), Some(body)))
        .await
        .unwrap()
}

fn stop(state: &AppState, tab: &str) {
    if let Some(session) = state.remove_session(tab) {
        session.force_kill();
    }
}

#[tokio::test]
async fn restart_refuses_a_plain_shell_without_killing_it() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path(), |_| true);
    let app = router(state.clone());
    let tab = create(&app, work.path(), None, None).await;
    let old = state.session(&tab).unwrap();

    let res = restart(&app, &tab, "plan").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(res).await["reason"], "Only an agent session has a mode.");
    // The same process is still in the map and still running; the row did not change.
    assert!(Arc::ptr_eq(&old, &state.session(&tab).unwrap()));
    assert!(!old.has_exited());
    let row = sessions(&app).await["sessions"][0].clone();
    assert_eq!(row["state"]["kind"], "running");
    assert!(row["mode"].is_null());
    stop(&state, &tab);
}

#[tokio::test]
async fn restart_refuses_an_agent_that_is_not_installed_without_killing_it() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    // Installed when the session starts, gone when the restart looks.
    static INSTALLED: AtomicBool = AtomicBool::new(true);
    let state = state_in(home.path(), |_| INSTALLED.load(Ordering::SeqCst));
    let app = router(state.clone());
    let tab = create(&app, work.path(), Some("claude"), Some("manual")).await;
    let old = state.session(&tab).unwrap();
    INSTALLED.store(false, Ordering::SeqCst);

    let res = restart(&app, &tab, "plan").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_of(res).await["reason"],
        "Claude Code isn't installed or isn't on PATH."
    );
    assert!(Arc::ptr_eq(&old, &state.session(&tab).unwrap()));
    assert!(!old.has_exited());
    let row = sessions(&app).await["sessions"][0].clone();
    assert_eq!(row["state"]["kind"], "running");
    assert_eq!(row["mode"], "manual");
    stop(&state, &tab);
}

#[tokio::test]
async fn restart_unknown_tab_is_404() {
    let home = tempfile::tempdir().unwrap();
    let app = router(state_in(home.path(), |_| true));
    assert_eq!(restart(&app, "nope", "plan").await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn restart_starts_a_new_process_in_the_same_row_in_the_new_mode() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path(), |_| true);
    let app = router(state.clone());
    let tab = create(&app, work.path(), Some("claude"), Some("manual")).await;
    let other = create(&app, work.path(), None, None).await;
    let before = sessions(&app).await;
    let old = state.session(&tab).unwrap();

    let res = restart(&app, &tab, "bypass").await;
    assert_eq!(res.status(), StatusCode::OK);
    let started = json_of(res).await;
    assert_eq!(started["tab_id"], tab.as_str());
    assert_eq!(started["number"], before["sessions"][0]["number"]);

    let after = sessions(&app).await;
    let rows = after["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["tab_id"], tab.as_str());
    assert_eq!(rows[0]["name"], before["sessions"][0]["name"]);
    assert_eq!(rows[0]["mode"], "bypass");
    assert_eq!(rows[0]["state"]["kind"], "running");
    assert_eq!(rows[1], before["sessions"][1]);
    // A new process is in the map under the same id.
    assert!(!Arc::ptr_eq(&old, &state.session(&tab).unwrap()));
    assert_eq!(state.live_session_count(), 2);

    // The old process is killed; its exit must not mark the new run Exited.
    let start = Instant::now();
    while !old.has_exited() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the old process never exited"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(sessions(&app).await["sessions"][0]["state"]["kind"], "running");
    stop(&state, &tab);
    stop(&state, &other);
}

#[tokio::test]
async fn a_setup_result_on_create_lists_back_and_a_restart_clears_it() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path(), |_| true);
    let app = router(state.clone());
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": work.path().to_string_lossy(),
        "agent": "claude", "mode": "manual",
        "setup": {"tab_id": "t-setup", "outcome": {"kind": "failed", "code": 1}},
    });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tab = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    let listed = sessions(&app).await;
    assert_eq!(
        listed["sessions"][0]["setup"],
        serde_json::json!({"tab_id": "t-setup", "outcome": {"kind": "failed", "code": 1}})
    );
    assert_eq!(restart(&app, &tab, "bypass").await.status(), StatusCode::OK);
    assert!(sessions(&app).await["sessions"][0].get("setup").is_none());
    stop(&state, &tab);
}
