//! A project's references through the real router: reading them, saving them, and the refusals. Every file lives in
//! a temp folder, and the Marshell home is a temp folder too.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::projects::canonical;
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5572;

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    /// A checkout with a `.git` folder.
    repo: PathBuf,
}

fn env() -> Env {
    common::tie_children_to_test();
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let repo = root.join("repo");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_agent_path(|_| None);
    Env {
        _dir: dir,
        app: router(st),
        repo,
    }
}

fn base(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null), text)
}

fn query(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

async fn get_refs(app: &axum::Router, cwd: &str) -> (StatusCode, Value, String) {
    let uri = format!("/v1/references?cwd={}", query(cwd));
    send(app, base("GET", &uri).body(Body::empty()).unwrap()).await
}

async fn put_refs(app: &axum::Router, body: Value) -> (StatusCode, Value, String) {
    let req = base("PUT", "/v1/references")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn get_and_put_refuse_without_the_bearer() {
    let e = env();
    let plain = |method: &str, uri: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"root": "x", "enabled": true, "aliases": []}).to_string(),
            ))
            .unwrap()
    };
    for (method, uri) in [("GET", "/v1/references?cwd=x"), ("PUT", "/v1/references")] {
        let (status, _, _) = send(&e.app, plain(method, uri)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn put_then_get_round_trips() {
    let e = env();
    std::fs::create_dir_all(e.repo.join("docs")).unwrap();
    let root = text(&e.repo);

    let (status, body, _) = get_refs(&e.app, &root).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
    assert_eq!(body["aliases"], json!([]));

    let update = json!({
        "root": root,
        "enabled": true,
        "aliases": [
            {"name": " plan ", "path": "docs"},
            {"name": "gone", "path": "nowhere\\file.md"}
        ]
    });
    let (status, _, body_text) = put_refs(&e.app, update).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body_text}");

    let (status, body, _) = get_refs(&e.app, &root).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], true);
    assert_eq!(
        body["aliases"],
        json!([
            {"name": "plan", "path": "docs", "exists": true},
            {"name": "gone", "path": "nowhere/file.md", "exists": false}
        ])
    );
}

#[tokio::test]
async fn a_worktree_cwd_reads_the_main_repos_references() {
    let e = env();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&e.repo)
            .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    // The `.git` folder `env` made is not a repo yet.
    std::fs::remove_dir_all(e.repo.join(".git")).unwrap();
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(e.repo.join("a.txt"), "a").unwrap();
    git(&["add", "."]);
    git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "first",
    ]);
    let tree = e.repo.parent().unwrap().join("tree");
    git(&["worktree", "add", "-q", "-b", "side", &text(&tree)]);

    let root = text(&e.repo);
    let (status, _, body_text) = put_refs(
        &e.app,
        json!({"root": root, "enabled": true, "aliases": [{"name": "file", "path": "a.txt"}]}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body_text}");

    let (status, body, _) = get_refs(&e.app, &text(&tree)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], true);
    assert_eq!(body["root"], root);
    assert_eq!(body["aliases"][0]["name"], "file");
    assert_eq!(body["aliases"][0]["exists"], true);
}

#[tokio::test]
async fn put_refuses_a_path_outside_the_project() {
    let e = env();
    let (status, _, body) = put_refs(
        &e.app,
        json!({"root": text(&e.repo), "enabled": true, "aliases": [{"name": "x", "path": "../x"}]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, "\"../x\" isn't inside the project.");
}

#[tokio::test]
async fn put_refuses_a_root_that_is_not_a_folder() {
    let e = env();
    let missing = text(&e.repo.join("no-such-folder"));
    let (status, _, body) = put_refs(&e.app, json!({"root": missing, "enabled": true, "aliases": []})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, format!("\"{missing}\" isn't a folder."));
}

#[tokio::test]
async fn relative_cwd_is_a_400() {
    let e = env();
    let (status, _, body) = get_refs(&e.app, "repo/docs").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "not a full path");

    let (status, _, body) = put_refs(&e.app, json!({"root": "repo", "enabled": true, "aliases": []})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "not a full path");
}
