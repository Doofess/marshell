//! A repo's `marshell.json` through the real router: reading it, allowing it, and starting setup or run as a
//! session. Every file lives in a temp folder, and the Marshell home is a temp folder too.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::projects::canonical;
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5571;

const CHANGED: &str = "marshell.json changed. Check the scripts again.";
const NOT_ALLOWED: &str = "This repo's scripts aren't allowed yet.";

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
    // One profile, the default: a script tab is that profile running the script.
    let shell = Profile {
        id: "shell".into(),
        name: "Shell".into(),
        command: vec![marshell_core::scripts::default_shell()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        vec![shell],
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

fn query(path: &Path) -> String {
    let mut out = String::new();
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

async fn get_scripts(app: &axum::Router, cwd: &Path) -> (StatusCode, Value, String) {
    let uri = format!("/v1/scripts?cwd={}", query(cwd));
    send(app, base("GET", &uri).body(Body::empty()).unwrap()).await
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value, String) {
    let req = base("POST", uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

async fn allow(app: &axum::Router, cwd: &Path, hash: &str) -> (StatusCode, Value, String) {
    post(app, "/v1/scripts/allow", json!({"cwd": cwd, "hash": hash})).await
}

async fn run(app: &axum::Router, cwd: &Path, script: &str, hash: &str) -> (StatusCode, Value, String) {
    post(
        app,
        "/v1/scripts/run",
        json!({"cwd": cwd, "script": script, "hash": hash, "cols": 80, "rows": 24}),
    )
    .await
}

fn write_file(repo: &Path, body: &Value) {
    std::fs::write(repo.join("marshell.json"), body.to_string()).unwrap();
}

/// A script that writes `ok` to `marker.txt` in its folder.
fn marker_script() -> &'static str {
    if cfg!(windows) {
        "Set-Content marker.txt ok"
    } else {
        "echo ok > marker.txt"
    }
}

async fn hash_of(app: &axum::Router, repo: &Path) -> String {
    let (_, body, _) = get_scripts(app, repo).await;
    body["hash"].as_str().unwrap().to_string()
}

async fn wait_for(path: &Path) -> Option<String> {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.trim().is_empty()
        {
            return Some(text);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

#[tokio::test]
async fn every_route_is_refused_without_the_bearer() {
    let e = env();
    let plain = |method: &str, uri: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"cwd": "x", "hash": "h", "script": "run"}).to_string(),
            ))
            .unwrap()
    };
    for (method, uri) in [
        ("GET", "/v1/scripts?cwd=x"),
        ("POST", "/v1/scripts/allow"),
        ("POST", "/v1/scripts/run"),
    ] {
        let (status, _, _) = send(&e.app, plain(method, uri)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn get_reports_the_file_and_untrusted() {
    let e = env();
    write_file(&e.repo, &json!({"setup": "pnpm install", "run": "pnpm dev"}));
    let (status, body, _) = get_scripts(&e.app, &e.repo).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["trusted"], false);
    assert_eq!(body["setup"], "pnpm install");
    assert_eq!(body["run"], "pnpm dev");
    assert_eq!(body["teardown"], Value::Null);
    assert_eq!(body["hash"].as_str().unwrap().len(), 64);
    assert!(body["file"].as_str().unwrap().ends_with("marshell.json"));
    assert_eq!(body["problem"], Value::Null);
}

#[tokio::test]
async fn allow_then_get_is_trusted() {
    let e = env();
    write_file(&e.repo, &json!({"run": "pnpm dev"}));
    let hash = hash_of(&e.app, &e.repo).await;
    let (status, body, _) = allow(&e.app, &e.repo, &hash).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["trusted"], true);
    let (_, again, _) = get_scripts(&e.app, &e.repo).await;
    assert_eq!(again["trusted"], true);
    // A changed file asks again.
    write_file(&e.repo, &json!({"run": "pnpm dev --host"}));
    let (_, changed, _) = get_scripts(&e.app, &e.repo).await;
    assert_eq!(changed["trusted"], false);
}

#[tokio::test]
async fn allow_with_an_old_hash_is_409_and_trusts_nothing() {
    let e = env();
    write_file(&e.repo, &json!({"run": "pnpm dev"}));
    let old = hash_of(&e.app, &e.repo).await;
    write_file(&e.repo, &json!({"run": "pnpm dev --host"}));
    let (status, _, text) = allow(&e.app, &e.repo, &old).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(text, CHANGED);
    let (_, now, _) = get_scripts(&e.app, &e.repo).await;
    assert_eq!(now["trusted"], false);
    // The old file isn't trusted either, should it come back.
    write_file(&e.repo, &json!({"run": "pnpm dev"}));
    let (_, back, _) = get_scripts(&e.app, &e.repo).await;
    assert_eq!(back["trusted"], false);
}

#[tokio::test]
async fn allow_refuses_a_file_that_cannot_be_read() {
    let e = env();
    std::fs::write(e.repo.join("marshell.json"), "{ not json").unwrap();
    let (_, body, _) = get_scripts(&e.app, &e.repo).await;
    let hash = body["hash"].as_str().unwrap().to_string();
    let problem = body["problem"].as_str().unwrap().to_string();
    let (status, _, text) = allow(&e.app, &e.repo, &hash).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text, problem);
    let (_, after, _) = get_scripts(&e.app, &e.repo).await;
    assert_eq!(after["trusted"], false);
}

#[tokio::test]
async fn run_refuses_untrusted_changed_missing_and_teardown() {
    let e = env();
    write_file(&e.repo, &json!({"setup": marker_script(), "teardown": "echo bye"}));
    let hash = hash_of(&e.app, &e.repo).await;

    let (status, _, text) = run(&e.app, &e.repo, "setup", &hash).await;
    assert_eq!((status, text.as_str()), (StatusCode::FORBIDDEN, NOT_ALLOWED));

    allow(&e.app, &e.repo, &hash).await;

    let (status, _, text) = run(&e.app, &e.repo, "teardown", &hash).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{text}");

    let (status, _, text) = run(&e.app, &e.repo, "run", &hash).await;
    assert_eq!(
        (status, text.as_str()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "marshell.json has no run script for this computer."
        )
    );

    let (status, _, text) = run(&e.app, &e.repo, "setup", &"0".repeat(64)).await;
    assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, CHANGED));

    // Nothing ran for any of them.
    assert!(!e.repo.join("marker.txt").exists());
}

#[tokio::test]
async fn run_with_the_old_hash_after_the_file_changed_is_409() {
    let e = env();
    write_file(&e.repo, &json!({"run": marker_script()}));
    let old = hash_of(&e.app, &e.repo).await;
    allow(&e.app, &e.repo, &old).await;
    write_file(&e.repo, &json!({"run": "echo changed"}));
    let (status, _, text) = run(&e.app, &e.repo, "run", &old).await;
    assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, CHANGED));
}

#[tokio::test]
async fn run_starts_a_named_session_in_the_checkout_root() {
    let e = env();
    write_file(&e.repo, &json!({"run": marker_script()}));
    let hash = hash_of(&e.app, &e.repo).await;
    allow(&e.app, &e.repo, &hash).await;
    // Asked from a folder inside the checkout: the session starts in the root.
    let inner = e.repo.join("sub");
    std::fs::create_dir(&inner).unwrap();
    let (status, created, text) = run(&e.app, &inner, "run", &hash).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let tab = created["tab_id"].as_str().unwrap().to_string();

    let (_, snapshot, _) = send(&e.app, base("GET", "/v1/sessions").body(Body::empty()).unwrap()).await;
    let row = snapshot["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab.as_str())
        .expect("the row is listed")
        .clone();
    assert_eq!(row["name"], "Run");
    assert_eq!(row["script"], "run");
    assert_eq!(row["agent"], Value::Null);
    // The default profile, so a Resume or Restart of the tab runs that profile's shell, never a "custom" one.
    assert_eq!(row["profile_id"], "shell");
    assert_eq!(canonical(Path::new(row["cwd"].as_str().unwrap())), e.repo);

    let marker = wait_for(&e.repo.join("marker.txt")).await;
    assert!(marker.is_some(), "the script did not write its marker in 10 s");
    send(
        &e.app,
        base("DELETE", &format!("/v1/sessions/{tab}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
}

#[cfg(windows)]
#[tokio::test]
async fn a_script_with_quotes_and_ampersands_reaches_the_shell_whole() {
    let e = env();
    write_file(&e.repo, &json!({"setup": r#"Set-Content out.txt "a & b; 'c'""#}));
    let hash = hash_of(&e.app, &e.repo).await;
    allow(&e.app, &e.repo, &hash).await;
    let (status, created, text) = run(&e.app, &e.repo, "setup", &hash).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let tab = created["tab_id"].as_str().unwrap().to_string();
    let out = wait_for(&e.repo.join("out.txt")).await.expect("out.txt within 10 s");
    assert_eq!(out.trim_end(), "a & b; 'c'");
    send(
        &e.app,
        base("DELETE", &format!("/v1/sessions/{tab}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
}
