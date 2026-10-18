use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::projects::{canonical, record};
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    home: PathBuf,
    project: PathBuf,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    record(&home, &project, 1).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    );
    Env {
        _dir: dir,
        app: router(st),
        home,
        project,
    }
}

fn base(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    (status, bytes.to_vec())
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let (status, bytes) = send(app, base("GET", uri).body(Body::empty()).unwrap()).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = base("POST", uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, bytes) = send(app, req).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn changeset(path: &Path, pointer: &str) -> Value {
    json!({
        "title": "Add hook",
        "reason": "r",
        "edits": [{
            "path": path.to_string_lossy(),
            "scope": "project",
            "op": {"kind": "jsonSet", "pointer": pointer, "value": "[]"},
            "applies": "newSessions"
        }]
    })
}

/// The apply request a user sends after looking at `preview`.
fn shown(preview: &Value) -> Value {
    let hashes: Vec<Value> = preview["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| json!({"path": f["path"], "hash": f.get("before_hash").cloned().unwrap_or(Value::Null)}))
        .collect();
    json!({"changeset_id": preview["changeset_id"], "expected_hashes": hashes})
}

#[tokio::test]
async fn preview_then_apply_writes_the_file() {
    let e = env();
    let path = e.project.join("settings.json");
    std::fs::write(&path, "{}\n").unwrap();
    let (status, preview) = post(&e.app, "/v1/config/preview", changeset(&path, "/hooks/Stop")).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["can_apply"], true);
    let (status, fetched) = get(
        &e.app,
        &format!("/v1/config/preview/{}", preview["changeset_id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["changeset_id"], preview["changeset_id"]);
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written, json!({"hooks": {"Stop": []}}));
    let (status, snaps) = get(
        &e.app,
        &format!("/v1/snapshots?path={}", encode(&path.to_string_lossy())),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snaps.as_array().unwrap().len(), 1, "{snaps}");
}

#[tokio::test]
async fn apply_after_an_outside_edit_is_409_and_the_file_is_untouched() {
    let e = env();
    let path = e.project.join("settings.json");
    std::fs::write(&path, "{}\n").unwrap();
    let (_, preview) = post(&e.app, "/v1/config/preview", changeset(&path, "/a")).await;
    std::fs::write(&path, "{\"mine\":1}").unwrap();
    let (status, body) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["reason"].is_string());
    assert!(body["paths"].is_array());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"mine\":1}");
}

#[tokio::test]
async fn restore_route_returns_a_preview_that_puts_the_file_back() {
    let e = env();
    let path = e.project.join("settings.json");
    let original = "{\"keep\": true}\n";
    std::fs::write(&path, original).unwrap();
    let (_, preview) = post(&e.app, "/v1/config/preview", changeset(&path, "/hooks/Stop")).await;
    let (_, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    let snapshot = applied["snapshot_id"].as_str().unwrap();
    let (status, restore) = post(&e.app, &format!("/v1/snapshots/{snapshot}/restore"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{restore}");
    assert_eq!(restore["can_apply"], true);
    let (status, body) = post(&e.app, "/v1/config/apply", shown(&restore)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let (status, _) = post(&e.app, "/v1/snapshots/nope/restore", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_managed_path_previews_as_refused() {
    let e = env();
    let path = e.project.join("managed-settings.json");
    let (status, preview) = post(&e.app, "/v1/config/preview", changeset(&path, "/a")).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["can_apply"], false);
    assert_eq!(preview["files"][0]["status"]["kind"], "refused");
    assert_eq!(preview["files"][0]["status"]["reason"], "managed");
    let (status, _) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(!path.exists());
}

#[tokio::test]
async fn unknown_preview_is_404() {
    let e = env();
    let (status, _) = get(&e.app, "/v1/config/preview/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(
        &e.app,
        "/v1/config/apply",
        json!({"changeset_id": "nope", "expected_hashes": []}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn config_routes_need_the_token() {
    let e = env();
    let req = Request::builder()
        .method("POST")
        .uri("/v1/config/preview")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let (status, _) = send(&e.app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let req = Request::builder()
        .method("GET")
        .uri("/v1/config/file?path=x")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&e.app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn file_route_reads_an_allowed_file_and_refuses_marshell_files() {
    let e = env();
    let path = e.project.join("settings.json");
    std::fs::write(&path, "{\"a\":1}").unwrap();
    let uri = |p: &Path| format!("/v1/config/file?path={}", encode(&p.to_string_lossy()));

    let (status, file) = get(&e.app, &uri(&path)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(file["exists"], true);
    assert_eq!(file["content"], "{\"a\":1}");
    assert_eq!(file["hash"].as_str().unwrap().len(), 64);
    assert!(file.get("refused").is_none());

    let (_, missing) = get(&e.app, &uri(&e.project.join("nope.json"))).await;
    assert_eq!(missing["exists"], false);
    assert!(missing.get("hash").is_none());

    std::fs::write(e.home.join("managed.json"), "[]").unwrap();
    let (_, own) = get(&e.app, &uri(&e.home.join("managed.json"))).await;
    assert_eq!(own["refused"], "marshellOwn");
    assert_eq!(own["exists"], false);
    assert!(own.get("content").is_none());

    let (_, relative) = get(&e.app, "/v1/config/file?path=settings.json").await;
    assert_eq!(relative["refused"], "notAbsolute");

    // A managed file is readable, and says it can't be written.
    let managed = e.project.join("managed-settings.json");
    std::fs::write(&managed, "{\"m\":1}").unwrap();
    let (_, m) = get(&e.app, &uri(&managed)).await;
    assert_eq!(m["refused"], "managed");
    assert_eq!(m["exists"], true);
    assert_eq!(m["content"], "{\"m\":1}");

    let bad = e.project.join("bad.bin");
    std::fs::write(&bad, [0xff, 0xfe, 0xfa]).unwrap();
    let (_, b) = get(&e.app, &uri(&bad)).await;
    assert_eq!(b["refused"], "notUtf8");
    assert!(b.get("content").is_none());
    assert!(b["hash"].is_string());

    let big = e.project.join("big.json");
    std::fs::write(&big, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    let (_, g) = get(&e.app, &uri(&big)).await;
    assert_eq!(g["refused"], "tooLarge");
    assert!(g.get("content").is_none());
}
