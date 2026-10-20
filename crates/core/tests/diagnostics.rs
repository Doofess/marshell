//! `/v1/diagnostics/*` routes. In this test binary nothing calls `diagnostics::log::init`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
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
