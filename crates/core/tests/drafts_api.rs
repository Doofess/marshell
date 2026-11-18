//! The drafts of text tabs: `/v1/drafts` and `/v1/drafts/one`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";
const TEXT_CAP: usize = 2 * 1024 * 1024;
/// What a draft's text may take: a file at the cap, in the encoding that is smallest on disk.
const DRAFT_CAP: usize = TEXT_CAP / 2 * 3;

fn setup() -> (tempfile::TempDir, axum::Router) {
    let home = tempfile::tempdir().unwrap();
    let app = router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.path().to_path_buf(),
        Vec::new(),
    ));
    (home, app)
}

fn enc(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn request(method: &str, uri: &str, body: Vec<u8>, bearer: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::CONTENT_TYPE, "application/json");
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    builder.body(Body::from(body)).unwrap()
}

async fn call(app: &axum::Router, method: &str, uri: &str, body: &Value, bearer: bool) -> (StatusCode, Vec<u8>) {
    let body = if body.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(body).unwrap()
    };
    let res = app.clone().oneshot(request(method, uri, body, bearer)).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 32 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

fn one(path: &str) -> String {
    format!("/v1/drafts/one?path={}", enc(path))
}

fn path_in(dir: &std::path::Path, name: &str) -> String {
    dir.join(name).to_string_lossy().into_owned()
}

fn draft(path: &str, text: &str, edited_ms: u64) -> Value {
    json!({
        "path": path,
        "text": text,
        "base_text": "base",
        "base_hash": "abc",
        "encoding": "utf-8",
        "eol": "lf",
        "edited_ms": edited_ms,
    })
}

#[tokio::test]
async fn drafts_are_refused_without_the_bearer() {
    let (_home, app) = setup();
    let path = path_in(std::env::temp_dir().as_path(), "a.txt");
    for (method, uri, body) in [
        ("GET", "/v1/drafts".to_string(), Value::Null),
        ("GET", one(&path), Value::Null),
        ("PUT", "/v1/drafts/one".to_string(), draft(&path, "x", 1)),
        ("DELETE", one(&path), Value::Null),
    ] {
        let (status, _) = call(&app, method, &uri, &body, false).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn put_get_list_delete_round_trip() {
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (path_in(dir.path(), "a.txt"), path_in(dir.path(), "b.txt"));
    let da = draft(&a, "mine", 1);
    assert_eq!(
        call(&app, "PUT", "/v1/drafts/one", &da, true).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "PUT", "/v1/drafts/one", &draft(&b, "other", 2), true)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (status, body) = call(&app, "GET", &one(&a), &Value::Null, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), da);
    let (status, body) = call(&app, "GET", "/v1/drafts", &Value::Null, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"drafts": [{"path": b, "edited_ms": 2}, {"path": a, "edited_ms": 1}]})
    );
    assert_eq!(
        call(&app, "DELETE", &one(&a), &Value::Null, true).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", &one(&a), &Value::Null, true).await.0,
        StatusCode::NO_CONTENT
    );
    let (_, body) = call(&app, "GET", "/v1/drafts", &Value::Null, true).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["drafts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn put_refuses_a_unc_path() {
    let (_home, app) = setup();
    for path in [r"\\server\share\a.txt", "relative.txt"] {
        let (status, body) = call(&app, "PUT", "/v1/drafts/one", &draft(path, "x", 1), true).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(String::from_utf8_lossy(&body), "That path can't be saved.");
    }
}

#[tokio::test]
async fn put_refuses_an_unknown_encoding() {
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(dir.path(), "a.txt");
    let mut bad = draft(&path, "x", 1);
    bad["encoding"] = json!("latin-1");
    let (status, body) = call(&app, "PUT", "/v1/drafts/one", &bad, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&body).contains("encoding"));
    let mut bad = draft(&path, "x", 1);
    bad["eol"] = json!("cr");
    let (status, body) = call(&app, "PUT", "/v1/drafts/one", &bad, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&body).contains("eol"));
}

#[tokio::test]
async fn put_refuses_text_over_the_cap() {
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(dir.path(), "a.txt");
    let big = "a".repeat(DRAFT_CAP + 1);
    let (status, _) = call(&app, "PUT", "/v1/drafts/one", &draft(&path, &big, 1), true).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let mut base = draft(&path, "x", 1);
    base["base_text"] = json!(big);
    let (status, _) = call(&app, "PUT", "/v1/drafts/one", &base, true).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let at_cap = "a".repeat(DRAFT_CAP);
    let (status, _) = call(&app, "PUT", "/v1/drafts/one", &draft(&path, &at_cap, 1), true).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn put_keeps_the_text_of_a_utf16_file_at_the_file_cap() {
    // A UTF-16 file of 2 MiB holds 1 Mi characters, and each can take three bytes in the draft's UTF-8.
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(dir.path(), "a.txt");
    let text = "\u{8a9e}".repeat(TEXT_CAP / 2);
    assert_eq!(text.len(), TEXT_CAP / 2 * 3);
    let mut both = draft(&path, &text, 1);
    both["base_text"] = json!(text);
    let (status, _) = call(&app, "PUT", "/v1/drafts/one", &both, true).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn get_of_no_draft_is_a_204() {
    // Not a 404: the editor asks for every file it opens, and the webview logs each 404 as an error in its console.
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(dir.path(), "none.txt");
    assert_eq!(
        call(&app, "GET", &one(&path), &Value::Null, true).await.0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn delete_of_no_draft_is_a_204() {
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(dir.path(), "none.txt");
    assert_eq!(
        call(&app, "DELETE", &one(&path), &Value::Null, true).await.0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn a_draft_never_touches_its_file() {
    let (_home, app) = setup();
    let dir = tempfile::tempdir().unwrap();
    let path = path_in(dir.path(), "never-made.txt");
    let d = draft(&path, "mine", 1);
    assert_eq!(
        call(&app, "PUT", "/v1/drafts/one", &d, true).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", &one(&path), &Value::Null, true).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "DELETE", &one(&path), &Value::Null, true).await.0,
        StatusCode::NO_CONTENT
    );
    assert!(!dir.path().join("never-made.txt").exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
