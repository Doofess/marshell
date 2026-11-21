//! The screenshots API: a page's PNG written to Downloads under a name the core makes.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::ScreenshotSaved;
use std::path::Path;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";
const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const AT: &str = "at=2026-11-21%20134501";

fn app_in(home: &Path) -> axum::Router {
    router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        Vec::new(),
    ))
}

async fn post(app: &axum::Router, query: &str, body: Vec<u8>, bearer: bool) -> (StatusCode, Vec<u8>) {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/v1/screenshots?{query}"))
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::CONTENT_TYPE, "image/png");
    if bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    let res = app.clone().oneshot(req.body(Body::from(body)).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, bytes.to_vec())
}

fn png(rest: &[u8]) -> Vec<u8> {
    let mut body = SIGNATURE.to_vec();
    body.extend_from_slice(rest);
    body
}

#[tokio::test]
async fn a_picture_is_refused_without_the_bearer() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    let (status, _) = post(&app, &format!("host=localhost:5173&{AT}"), png(b"x"), false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_body_without_the_png_signature_is_a_400() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    let (status, body) = post(&app, &format!("host=example.com&{AT}"), b"GIF89a".to_vec(), true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, b"That isn't a PNG image.");
}

#[tokio::test]
async fn a_time_that_cannot_name_a_file_is_a_400() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    for query in [
        "host=example.com&at=..%2F..%2Fx",
        "host=example.com&at=2026-11-21%2013%3A45%3A01",
        "host=example.com",
    ] {
        let (status, body) = post(&app, query, png(b"x"), true).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(body, b"That time can't be used in a file name.", "{query}");
    }
}

#[tokio::test]
async fn a_body_over_the_cap_is_a_413() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    let mut big = png(b"");
    big.resize(64 * 1024 * 1024 + 1, 0);
    let (status, body) = post(&app, &format!("host=example.com&{AT}"), big, true).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body, b"That picture is over 64 MB.");
}

/// The one test that reads `MARSHELL_DOWNLOADS`: the cases above answer before the folder is looked up, so
/// nothing else in this file's process depends on the variable.
#[tokio::test]
async fn a_picture_is_written_to_the_overridden_downloads_under_a_name_the_core_made() {
    let home = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    // SAFETY: set before the first request of the only test that reads it.
    unsafe { std::env::set_var("MARSHELL_DOWNLOADS", downloads.path()) };
    let app = app_in(home.path());

    let (status, body) = post(&app, &format!("host=localhost:5173&{AT}"), png(b"first"), true).await;
    assert_eq!(status, StatusCode::OK);
    let saved: ScreenshotSaved = serde_json::from_slice(&body).unwrap();
    assert_eq!(saved.name, "localhost-5173 2026-11-21 134501.png");
    assert_eq!(
        std::fs::read(downloads.path().join(&saved.name)).unwrap(),
        png(b"first")
    );

    // The same picture again is a new file, and the first one is untouched.
    let (status, body) = post(&app, &format!("host=localhost:5173&{AT}"), png(b"second"), true).await;
    assert_eq!(status, StatusCode::OK);
    let again: ScreenshotSaved = serde_json::from_slice(&body).unwrap();
    assert_eq!(again.name, "localhost-5173 2026-11-21 134501 (2).png");
    assert_eq!(
        std::fs::read(downloads.path().join(&saved.name)).unwrap(),
        png(b"first")
    );

    // A host made to climb out of the folder names a file in it, and nothing lands anywhere else.
    let (status, body) = post(&app, &format!("host=..%2F..%2Fescape%5C..%5Cx&{AT}"), png(b"x"), true).await;
    assert_eq!(status, StatusCode::OK);
    let climbed: ScreenshotSaved = serde_json::from_slice(&body).unwrap();
    assert_eq!(climbed.name, "escape-.-x 2026-11-21 134501.png");
    let parent = downloads.path().parent().unwrap();
    assert!(!parent.join("escape").exists() && !parent.join("escape-.-x 2026-11-21 134501.png").exists());
    assert_eq!(std::fs::read_dir(downloads.path()).unwrap().count(), 3);

    // A Downloads folder that isn't there is a 500 with its own words, and none is made.
    let gone = downloads.path().join("gone");
    unsafe { std::env::set_var("MARSHELL_DOWNLOADS", &gone) };
    let (status, body) = post(&app, &format!("host=example.com&{AT}"), png(b"x"), true).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, b"The Downloads folder wasn't found.");
    assert!(!gone.exists());
}
