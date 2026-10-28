//! `POST /v1/sessions/{tab}/focus`: the sink gets the tab, and the answers when it can't.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5558;

fn state(home: &Path) -> AppState {
    common::tie_children_to_test();
    let profile = Profile {
        id: "echoer".into(),
        name: "echoer".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "echo".into()],
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
        vec!["http://tauri.localhost".into()],
        home.to_path_buf(),
        vec![profile],
    )
}

fn api(method: &str, path: &str, bearer: bool, body: Option<serde_json::Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"));
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn body_of(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn start(app: &axum::Router, cwd: &Path) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "echoer", "cwd": cwd.to_string_lossy() });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", true, Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body_of(res).await).unwrap();
    json["tab_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn focus_calls_the_sink_with_the_tab() {
    let tmp = tempfile::tempdir().unwrap();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = seen.clone();
    let st = state(&tmp.path().join("home")).with_focus(Arc::new(move |tab| sink.lock().unwrap().push(tab)));
    let app = router(st);
    let tab = start(&app, tmp.path()).await;
    let route = format!("/v1/sessions/{tab}/focus");
    let res = app.clone().oneshot(api("POST", &route, true, None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(*seen.lock().unwrap(), vec![tab]);

    // Like every route, it is refused without the bearer, and the sink is not called.
    let res = app.clone().oneshot(api("POST", &route, false, None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn focus_unknown_tab_is_404() {
    let tmp = tempfile::tempdir().unwrap();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = seen.clone();
    let st = state(&tmp.path().join("home")).with_focus(Arc::new(move |tab| sink.lock().unwrap().push(tab)));
    let app = router(st);
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions/nope/focus", true, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body_of(res).await, "No such session.");
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn focus_without_a_sink_is_503() {
    let tmp = tempfile::tempdir().unwrap();
    let app = router(state(&tmp.path().join("home")));
    let tab = start(&app, tmp.path()).await;
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/focus"), true, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body_of(res).await, "Marshell's window isn't there to focus.");
}
