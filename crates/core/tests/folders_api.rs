//! `GET /v1/folders/check`: a typed folder path answered with its canonical form, or a refusal.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

/// Percent-encodes everything but letters, digits and `-._~`.
fn enc(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn get(app: &axum::Router, path: &str, token: bool) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("GET")
        .uri(format!("/v1/folders/check?path={}", enc(path)))
        .header(header::HOST, format!("127.0.0.1:{PORT}"));
    if token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    app.clone().oneshot(builder.body(Body::empty()).unwrap()).await.unwrap()
}

#[tokio::test]
async fn check_answers_200_404_400() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.path().to_path_buf(),
        Vec::new(),
    );
    let app = router(state);
    let dir_text = dir.path().to_string_lossy().into_owned();

    let res = get(&app, &dir_text, true).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["name"], dir.path().file_name().unwrap().to_string_lossy().as_ref());
    assert!(json["path"].is_string());
    assert!(json["branch"].is_null());

    let missing = dir.path().join("nope").to_string_lossy().into_owned();
    assert_eq!(get(&app, &missing, true).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(get(&app, "src/main.rs", true).await.status(), StatusCode::BAD_REQUEST);
    if cfg!(windows) {
        assert_eq!(get(&app, "/x", true).await.status(), StatusCode::BAD_REQUEST);
    }
    assert_eq!(get(&app, &dir_text, false).await.status(), StatusCode::UNAUTHORIZED);
}
