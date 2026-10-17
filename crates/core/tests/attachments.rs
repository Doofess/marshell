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

async fn paste(app: &axum::Router, tab: &str, attachment: &str, text: &str) -> axum::response::Response {
    let body = serde_json::json!({ "attachment": attachment, "text": text });
    call(app, "POST", &format!("/v1/sessions/{tab}/paste"), Some(body)).await
}

#[tokio::test]
async fn paste_refuses_a_name_that_isnt_a_saved_attachment() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    for name in ["../x.png", "a.txt", "1.png"] {
        let res = paste(&s.app, &tab, name, "hi").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{name}");
    }
}

#[tokio::test]
async fn paste_refuses_a_comment_with_a_line_break() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let out = saved(post_png(&s.app, &tab, png_body(b"rest")).await).await;
    let res = paste(&s.app, &tab, &out.name, "a\nb").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn paste_of_a_saved_attachment_is_accepted() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let out = saved(post_png(&s.app, &tab, png_body(b"rest")).await).await;
    let res = paste(&s.app, &tab, &out.name, "x.pdf, page 1: is this right?").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = paste(&s.app, &tab, &out.name, "").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn paste_into_a_paused_session_is_refused() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let out = saved(post_png(&s.app, &tab, png_body(b"rest")).await).await;
    let res = call(&s.app, "POST", &format!("/v1/sessions/{tab}/pause"), None).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = paste(&s.app, &tab, &out.name, "hi").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn paste_to_an_unknown_tab_is_refused() {
    let s = setup();
    let res = paste(&s.app, "nope", "1.png", "hi").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

async fn paste_paths(app: &axum::Router, tab: &str, paths: &[String]) -> axum::response::Response {
    let body = serde_json::json!({ "paths": paths });
    call(app, "POST", &format!("/v1/sessions/{tab}/paste-paths"), Some(body)).await
}

fn file_in(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, b"x").unwrap();
    path.to_string_lossy().into_owned()
}

/// A session whose program turns bracketed paste on and appends every byte typed to it to a file.
/// Returns the router, the tab and that file.
async fn capturing(s: &Setup) -> (axum::Router, String, std::path::PathBuf) {
    let log = s.dir.path().join("typed.log");
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let command = vec![fake.into(), "capture-input".into(), log.to_string_lossy().into_owned()];
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        s._home.path().to_path_buf(),
        vec![profile("capture", command)],
    );
    let app = router(state.clone());
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "capture", "cwd": s.dir.path().to_string_lossy(), "agent": null,
    });
    let res = call(&app, "POST", "/v1/sessions", Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let tab = json["tab_id"].as_str().unwrap().to_string();
    let session = state.session(&tab).unwrap();
    // The program sets raw input before it asks for bracketed paste, so this also means it is ready.
    for _ in 0..200 {
        if session.bracketed_paste() {
            return (app, tab, log);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the program never turned bracketed paste on");
}

async fn typed(log: &Path, at_least: usize) -> Vec<u8> {
    for _ in 0..200 {
        let bytes = std::fs::read(log).unwrap_or_default();
        if bytes.len() >= at_least {
            // Give a stray extra write the chance to show up.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            return std::fs::read(log).unwrap_or_default();
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    std::fs::read(log).unwrap_or_default()
}

#[tokio::test]
async fn pastes_each_path_as_its_own_bracketed_paste() {
    let s = setup();
    let (app, tab, log) = capturing(&s).await;
    let a = file_in(s.dir.path(), "my file.txt");
    let b = file_in(s.dir.path(), "r\u{e9}sum\u{e9} \u{65e5}\u{672c}.txt");
    let res = paste_paths(&app, &tab, &[a.clone(), b.clone()]).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let expected = format!("\x1b[200~\"{a}\"\x1b[201~\x1b[200~ \x1b[201~\x1b[200~\"{b}\"\x1b[201~").into_bytes();
    let got = typed(&log, expected.len()).await;
    assert_eq!(String::from_utf8_lossy(&got), String::from_utf8_lossy(&expected));
    assert!(!got.contains(&b'\r') && !got.contains(&b'\n'));
}

#[tokio::test]
async fn pastes_a_folder_path() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let folder = s.dir.path().to_string_lossy().into_owned();
    let res = paste_paths(&s.app, &tab, &[folder]).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn refuses_a_relative_path() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let res = paste_paths(&s.app, &tab, &["notes/todo.txt".into()]).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn refuses_a_missing_path() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let missing = s.dir.path().join("nope.txt").to_string_lossy().into_owned();
    let res = paste_paths(&s.app, &tab, &[missing]).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(&bytes[..], b"That path doesn't exist.");
}

#[tokio::test]
async fn refuses_an_empty_list() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let res = paste_paths(&s.app, &tab, &[]).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn refuses_33_paths() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let one = s.dir.path().to_string_lossy().into_owned();
    let res = paste_paths(&s.app, &tab, &vec![one.clone(); 32]).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = paste_paths(&s.app, &tab, &vec![one; 33]).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn refuses_a_path_with_a_quote_or_control_char() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let base = s.dir.path().to_string_lossy().into_owned();
    for bad in ["a\"b", "a\nb", "a\rb", "a\x1b[201~b", "a\x7fb", "a\u{9b}b"] {
        let res = paste_paths(&s.app, &tab, &[format!("{base}{}{bad}", std::path::MAIN_SEPARATOR)]).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{bad:?}");
    }
}

#[tokio::test]
async fn refuses_paths_for_a_paused_session() {
    let s = setup();
    let tab = session(&s.app, s.dir.path()).await;
    let res = call(&s.app, "POST", &format!("/v1/sessions/{tab}/pause"), None).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = paste_paths(&s.app, &tab, &[s.dir.path().to_string_lossy().into_owned()]).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn refuses_paths_for_an_unknown_tab() {
    let s = setup();
    let res = paste_paths(&s.app, "nope", &[s.dir.path().to_string_lossy().into_owned()]).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
