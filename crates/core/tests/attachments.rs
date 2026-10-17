//! The attachments API: a region's PNG saved under the session, and the reference the agent is told.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{AttachmentSaved, Profile, ShellFamily};
use std::path::Path;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";
const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

fn profile(id: &str, command: Vec<String>) -> Profile {
    Profile {
        id: id.into(),
        name: id.into(),
        command,
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: false,
        terminal_theme: None,
        detected: true,
    }
}

fn state_in(home: &Path) -> AppState {
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let profiles = vec![profile("sleeper", vec![fake.into(), "sleep".into()])];
    AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()], home.to_path_buf(), profiles)
}

fn request(method: &str, path: &str, body: Option<serde_json::Value>) -> Request<Body> {
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

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    app.clone().oneshot(request(method, path, body)).await.unwrap()
}

async fn post_png(app: &axum::Router, tab: &str, bytes: Vec<u8>) -> axum::response::Response {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/v1/sessions/{tab}/attachments"))
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(bytes))
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

async fn saved(res: axum::response::Response) -> AttachmentSaved {
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn session(app: &axum::Router, cwd: &Path) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy(), "agent": null,
    });
    let res = call(app, "POST", "/v1/sessions", Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["tab_id"].as_str().unwrap().to_string()
}

fn png_body(rest: &[u8]) -> Vec<u8> {
    let mut body = SIGNATURE.to_vec();
    body.extend_from_slice(rest);
    body
}

struct Setup {
    _home: tempfile::TempDir,
    dir: tempfile::TempDir,
    app: axum::Router,
}

fn setup() -> Setup {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = router(state_in(home.path()));
    Setup { _home: home, dir, app }
}

#[tokio::test]
async fn saves_a_png_under_the_tab_and_returns_its_reference() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let body = png_body(b"rest");
    let res = post_png(&s.app, &tab, body.clone()).await;
    assert_eq!(res.status(), StatusCode::OK);
    let out = saved(res).await;
    let path = Path::new(&out.path);
    assert!(path.exists());
    let tail = format!("attachments/{tab}/{}", out.name);
    assert!(
        out.path.replace('\\', "/").ends_with(&tail),
        "{} should end with {tail}",
        out.path
    );
    let stem = out.name.strip_suffix(".png").expect("a .png name");
    let mut parts = stem.splitn(2, '-');
    assert!(parts.next().unwrap().chars().all(|c| c.is_ascii_digit()));
    assert!(parts.next().is_none_or(|n| n.chars().all(|c| c.is_ascii_digit())));
    assert_eq!(std::fs::read(path).unwrap(), body);
    assert_eq!(out.reference, format!("\"{}\"", out.path));
}

#[tokio::test]
async fn two_saves_in_the_same_millisecond_get_two_names() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let a = saved(post_png(&s.app, &tab, png_body(b"one")).await).await;
    let b = saved(post_png(&s.app, &tab, png_body(b"two")).await).await;
    assert_ne!(a.name, b.name);
    assert_eq!(std::fs::read(&a.path).unwrap(), png_body(b"one"));
    assert_eq!(std::fs::read(&b.path).unwrap(), png_body(b"two"));
}

#[tokio::test]
async fn refuses_what_isnt_a_png() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let res = post_png(&s.app, &tab, b"GIF89a..".to_vec()).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn refuses_an_unknown_tab() {
    let s = setup();
    let res = post_png(&s.app, "nope", png_body(b"rest")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn refuses_a_body_over_20_mb() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let mut body = SIGNATURE.to_vec();
    body.resize(20 * 1024 * 1024 + 1, 0);
    let res = post_png(&s.app, &tab, body).await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
