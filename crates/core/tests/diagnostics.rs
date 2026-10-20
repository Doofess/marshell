//! `/v1/diagnostics/*` routes. In this test binary nothing calls `diagnostics::log::init`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn app(home: &std::path::Path) -> axum::Router {
    router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        Vec::new(),
    ))
}

async fn call(app: &axum::Router, method: &str, path: &str, body: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    let body = match body {
        Some(text) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(text.to_string())
        }
        None => Body::empty(),
    };
    app.clone().oneshot(builder.body(body).unwrap()).await.unwrap()
}

#[tokio::test]
async fn logging_route_reports_unavailable_without_init() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());

    let res = call(&app, "GET", "/v1/diagnostics/logging", None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json, serde_json::json!({ "available": false, "detailed": false }));

    let res = call(&app, "PUT", "/v1/diagnostics/logging", Some(r#"{"detailed":true}"#)).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(&bytes[..], b"Logging is not running.");
}

#[tokio::test]
async fn logging_routes_need_the_token() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let req = Request::builder()
        .method("GET")
        .uri("/v1/diagnostics/logging")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn perf_needs_the_token() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let req = Request::builder()
        .method("GET")
        .uri("/v1/perf")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn perf_lists_each_open_session() {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
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
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.path().to_path_buf(),
        vec![profile],
    );
    let app = router(state.clone());

    let res = call(&app, "GET", "/v1/perf", None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["sessions"].as_array().unwrap().len(), 0);

    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": work.path().to_string_lossy(),
    });
    let res = call(&app, "POST", "/v1/sessions", Some(&body.to_string())).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let tab = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = call(&app, "GET", "/v1/perf", None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let sessions = json["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["tab_id"], tab.as_str());
    assert!(sessions[0]["usage"]["processes"].as_u64().unwrap() >= 1);
    if cfg!(windows) {
        assert!(json["core"]["memory_bytes"].is_u64());
        assert!(sessions[0]["usage"]["memory_bytes"].is_u64());
    }
    assert!(json["sampled_at_ms"].as_u64().unwrap() > 0);

    if let Some(session) = state.remove_session(&tab) {
        session.force_kill();
    }
}
