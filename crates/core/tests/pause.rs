//! Pause freezes a running session and Continue thaws it. The row's `paused` flag follows the
//! freeze, and typing into a paused session is refused.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn state_in(home: &Path) -> AppState {
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

async fn row(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    json_of(res).await["sessions"][0].clone()
}

async fn create(app: &axum::Router, cwd: &Path) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy(),
    });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn post(
    app: &axum::Router,
    tab: &str,
    action: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    app.clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/{action}"), body))
        .await
        .unwrap()
}

fn stop(state: &AppState, tab: &str) {
    if let Some(session) = state.remove_session(tab) {
        session.force_kill();
    }
}

#[tokio::test]
async fn pause_then_continue() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, work.path()).await;
    assert_eq!(row(&app).await["paused"], false);

    assert_eq!(post(&app, &tab, "pause", None).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(row(&app).await["paused"], true);
    assert!(state.session(&tab).unwrap().is_frozen());

    assert_eq!(
        post(&app, &tab, "continue", None).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(row(&app).await["paused"], false);
    assert!(!state.session(&tab).unwrap().is_frozen());
    stop(&state, &tab);
}

#[tokio::test]
async fn pause_unknown_tab_is_404() {
    let home = tempfile::tempdir().unwrap();
    let app = router(state_in(home.path()));
    assert_eq!(post(&app, "nope", "pause", None).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        post(&app, "nope", "continue", None).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn pause_ended_session_is_409() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, work.path()).await;
    let session = state.session(&tab).unwrap();
    session.kill();
    let start = Instant::now();
    while !session.has_exited() || row(&app).await["state"]["kind"] != "exited" {
        assert!(start.elapsed() < Duration::from_secs(10), "the session never exited");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let res = post(&app, &tab, "pause", None).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(res).await["reason"], "Only a running session can be paused.");
    assert_eq!(row(&app).await["paused"], false);
    stop(&state, &tab);
}

#[tokio::test]
async fn pause_twice_is_204() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, work.path()).await;
    assert_eq!(post(&app, &tab, "pause", None).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(post(&app, &tab, "pause", None).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(row(&app).await["paused"], true);
    stop(&state, &tab);
}

#[tokio::test]
async fn pause_while_a_start_is_claimed_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, work.path()).await;
    let _claim = state.claim_start(&tab).unwrap();

    for action in ["pause", "continue"] {
        let res = post(&app, &tab, action, None).await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(json_of(res).await["reason"], "It is already starting.");
    }
    assert!(!state.session(&tab).unwrap().is_frozen());
    stop(&state, &tab);
}

#[tokio::test]
async fn input_refused_while_paused() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, work.path()).await;
    let text = || Some(serde_json::json!({ "text": "hi" }));

    assert_eq!(post(&app, &tab, "pause", None).await.status(), StatusCode::NO_CONTENT);
    let res = post(&app, &tab, "input", text()).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(res).await["reason"], "It is paused.");

    assert_eq!(
        post(&app, &tab, "continue", None).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(post(&app, &tab, "input", text()).await.status(), StatusCode::NO_CONTENT);
    stop(&state, &tab);
}

#[tokio::test]
async fn settings_keep_pause_warned() {
    let home = tempfile::tempdir().unwrap();
    let app = router(state_in(home.path()));
    let current = json_of(app.clone().oneshot(api("GET", "/v1/settings", None)).await.unwrap()).await;
    assert_eq!(current["pause_warned"], false);

    let put = serde_json::json!({ "appearance": current["appearance"], "pause_warned": true });
    let res = app
        .clone()
        .oneshot(api("PUT", "/v1/settings", Some(put)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let back = json_of(app.oneshot(api("GET", "/v1/settings", None)).await.unwrap()).await;
    assert_eq!(back["pause_warned"], true);

    // A later run reads it from the file.
    assert!(marshell_core::settings::load(home.path()).pause_warned);
}
