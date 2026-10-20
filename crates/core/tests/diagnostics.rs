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

/// A state that runs no real `claude` and reads no real Claude Code settings.
fn export_app(home: &std::path::Path) -> axum::Router {
    router(
        AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()], home.to_path_buf(), Vec::new())
            .with_claude_settings(home.join("claude-settings.json"))
            .with_claude_version(None)
            .with_agent_path(|_| None)
            .with_agent_check(|_| false),
    )
}

async fn body_of(res: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(res.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec()
}

const EXPIRED: &[u8] = b"This export has expired. Prepare it again.";

#[tokio::test]
async fn export_preview_then_file_then_zip() {
    use std::io::Read;

    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("logs")).unwrap();
    std::fs::write(home.path().join("logs").join("marshell.log"), "one line\n").unwrap();
    let app = export_app(home.path());

    let res = call(&app, "POST", "/v1/diagnostics/export", None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let preview: serde_json::Value = serde_json::from_slice(&body_of(res).await).unwrap();
    let id = preview["id"].as_str().unwrap().to_string();
    let files = preview["files"].as_array().unwrap();
    let names: Vec<&str> = files.iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(names[0], "README.txt");
    assert!(names.contains(&"logs/marshell.log"), "{names:?}");
    assert_eq!(
        preview["total_bytes"].as_u64().unwrap(),
        files.iter().map(|f| f["bytes"].as_u64().unwrap()).sum::<u64>()
    );

    for file in files {
        let name = file["name"].as_str().unwrap();
        let res = call(
            &app,
            "GET",
            &format!("/v1/diagnostics/export/{id}/file?name={name}"),
            None,
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK, "{name}");
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/plain; charset=utf-8");
        let bytes = body_of(res).await;
        assert_eq!(bytes.len() as u64, file["bytes"].as_u64().unwrap(), "{name}");
        if name == "logs/marshell.log" {
            assert_eq!(bytes, b"one line\n");
        }
    }

    let res = call(&app, "GET", &format!("/v1/diagnostics/export/{id}/zip"), None).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/zip");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(body_of(res).await)).unwrap();
    let in_zip: Vec<String> = archive.file_names().map(|n| n.unwrap().into_owned()).collect();
    assert_eq!(in_zip, names);
    let mut text = String::new();
    archive
        .by_name("logs/marshell.log")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "one line\n");

    // Preparing again replaces the first export.
    let res = call(&app, "POST", "/v1/diagnostics/export", None).await;
    let second: serde_json::Value = serde_json::from_slice(&body_of(res).await).unwrap();
    assert_ne!(second["id"].as_str().unwrap(), id);
    let res = call(&app, "GET", &format!("/v1/diagnostics/export/{id}/zip"), None).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_export_is_404() {
    let home = tempfile::tempdir().unwrap();
    let app = export_app(home.path());

    for path in [
        "/v1/diagnostics/export/nope/file?name=README.txt",
        "/v1/diagnostics/export/nope/zip",
    ] {
        let res = call(&app, "GET", path, None).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(body_of(res).await, EXPIRED, "{path}");
    }

    // A real export with a name that is not in it.
    let res = call(&app, "POST", "/v1/diagnostics/export", None).await;
    let preview: serde_json::Value = serde_json::from_slice(&body_of(res).await).unwrap();
    let id = preview["id"].as_str().unwrap();
    let res = call(
        &app,
        "GET",
        &format!("/v1/diagnostics/export/{id}/file?name=nothing.txt"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body_of(res).await, EXPIRED);
}

#[tokio::test]
async fn export_routes_need_the_token() {
    let home = tempfile::tempdir().unwrap();
    let app = export_app(home.path());
    for (method, path) in [
        ("POST", "/v1/diagnostics/export"),
        ("GET", "/v1/diagnostics/export/abc/file?name=README.txt"),
        ("GET", "/v1/diagnostics/export/abc/zip"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path}"
        );
    }
}
