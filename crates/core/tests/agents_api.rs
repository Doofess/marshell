use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::paths;
use marshell_core::projects::{canonical, record};
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use std::path::PathBuf;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5556;

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    home: PathBuf,
    settings: PathBuf,
}

/// A temp Marshell home, a project holding the settings file, and no detected profiles, so no
/// `claude` is ever run. The settings path is handed to the state, never read from the environment.
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    record(&home, &project, 1).unwrap();
    let settings = project.join("settings.json");
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    )
    .with_claude_settings(settings.clone());
    Env {
        _dir: dir,
        app: router(st),
        home,
        settings,
    }
}

fn base(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
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
    let (status, bytes) = post_raw(app, uri, body).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn post_raw(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Vec<u8>) {
    let req = base("POST", uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
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

const WANT: fn() -> Value = || json!({"status": true, "statusline": true});

#[tokio::test]
async fn setup_reports_missing_file() {
    let e = env();
    let (status, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    assert_eq!(setup["agent"], "claude");
    assert_eq!(setup["found"], false);
    assert_eq!(setup["settings_state"], "missing");
    assert_eq!(setup["hooks"], "none");
    assert_eq!(setup["has_own_statusline"], false);
    assert_eq!(setup["settings_path"], e.settings.to_string_lossy().as_ref());
    assert_eq!(
        setup["bridge_path"],
        paths::bridge_exe(&e.home).to_string_lossy().as_ref()
    );
    assert!(setup.get("version").is_none(), "{setup}");
}

#[tokio::test]
async fn plan_then_apply_writes_the_hooks() {
    let e = env();
    std::fs::write(&e.settings, "{\n  \"model\": \"opus\"\n}\n").unwrap();
    let bridge = paths::bridge_exe(&e.home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();

    let (status, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["can_apply"], true, "{preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");

    let written: Value = serde_json::from_str(&std::fs::read_to_string(&e.settings).unwrap()).unwrap();
    assert_eq!(written["model"], "opus");
    assert_eq!(written["hooks"].as_object().unwrap().len(), 8, "{written}");

    let (_, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(setup["settings_state"], "ok", "{setup}");
    assert_eq!(setup["hooks"], "installed", "{setup}");
}

#[tokio::test]
async fn remove_plan_after_install_restores_the_file() {
    let e = env();
    let original = "{\r\n  // mine\r\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"},\r\n}\r\n";
    std::fs::write(&e.settings, original).unwrap();
    let bridge = paths::bridge_exe(&e.home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();

    let (_, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_ne!(std::fs::read_to_string(&e.settings).unwrap(), original);
    let (_, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(setup["hooks"], "installed", "{setup}");
    assert_eq!(setup["chained"], true, "{setup}");

    let (status, removal) = post(&e.app, "/v1/agents/claude/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{removal}");
    assert_eq!(removal["statusline_left"], false);
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(std::fs::read_to_string(&e.settings).unwrap(), original);
}

#[tokio::test]
async fn unknown_agent_is_404() {
    let e = env();
    let (status, _) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&e.app, "/v1/agents/codex/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&e.app, "/v1/agents/codex/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unparsable_settings_is_422_with_the_line() {
    let e = env();
    std::fs::write(&e.settings, "{\n  \"a\": 1,\n  \"b\": ,\n}\n").unwrap();
    let (status, bytes) = post_raw(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = String::from_utf8(bytes).unwrap();
    assert!(
        message.ends_with("(line 3), so Marshell will not edit it."),
        "{message}"
    );
    assert!(message.starts_with(&*e.settings.to_string_lossy()), "{message}");

    let (status, _) = post_raw(&e.app, "/v1/agents/claude/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(setup["settings_state"], "unparsable", "{setup}");
    assert_eq!(setup["parse_error"]["line"], 3, "{setup}");
    assert_eq!(setup["hooks"], "none");
}
