//! Sign-in and update tabs: the core builds the command, refuses what does not fit, keeps a sign-in tab out of the
//! recordings and never remembers or repeats one. No test resolves a real CLI: the "installed" CLI is `fake-agent`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;
const ORIGIN: &str = "http://tauri.localhost";

fn fake_cli(_command: &str) -> Option<PathBuf> {
    Some(PathBuf::from(env!("CARGO_BIN_EXE_fake-agent")))
}

fn no_cli(_command: &str) -> Option<PathBuf> {
    None
}

fn app_in(home: &Path, find: fn(&str) -> Option<PathBuf>) -> axum::Router {
    common::tie_children_to_test();
    let profile = Profile {
        id: "sleeper".into(),
        name: "sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    router(
        AppState::new(
            TOKEN.into(),
            PORT,
            vec![ORIGIN.into()],
            home.to_path_buf(),
            vec![profile],
        )
        .with_agent_path(find),
    )
}

fn api(method: &str, path: &str, body: Option<serde_json::Value>) -> Request<Body> {
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

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn create(app: &axum::Router, extra: serde_json::Value) -> (StatusCode, String) {
    let mut body = serde_json::json!({ "cols": 80, "rows": 24 });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    send(app, api("POST", "/v1/sessions", Some(body))).await
}

fn tab_of(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn sessions(app: &axum::Router) -> serde_json::Value {
    serde_json::from_str(&send(app, api("GET", "/v1/sessions", None)).await.1).unwrap()
}

fn casts_of(home: &Path, tab: &str) -> Vec<String> {
    let dir = marshell_core::paths::recordings_dir(home);
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(&format!("{tab}-")))
                .collect()
        })
        .unwrap_or_default()
}

async fn wait_for<F: FnMut() -> bool>(mut done: F) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    done()
}

#[tokio::test]
async fn cli_task_with_anything_else_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path(), fake_cli);
    let task = serde_json::json!({ "agent": "claude", "kind": "sign_in" });
    let others = [
        serde_json::json!({ "shell": ["x"] }),
        serde_json::json!({ "agent": "claude" }),
        serde_json::json!({ "prompt": "hi" }),
        serde_json::json!({ "mode": "plan" }),
        serde_json::json!({ "preset": "deepseek" }),
        serde_json::json!({ "tool": { "tool": "a", "preset": "b", "fields": {} } }),
    ];
    for other in others {
        let mut body = serde_json::json!({ "cli_task": task });
        body.as_object_mut().unwrap().extend(other.as_object().unwrap().clone());
        let (status, text) = create(&app, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{other}");
        assert_eq!(text, "A sign-in or update tab takes nothing else.", "{other}");
    }
    assert_eq!(sessions(&app).await["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn cli_task_for_an_unknown_agent_is_400() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path(), fake_cli);
    let (status, text) = create(
        &app,
        serde_json::json!({ "cli_task": { "agent": "emacs", "kind": "sign_in" } }),
    )
    .await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "Unknown agent: emacs.")
    );
}

#[tokio::test]
async fn cli_task_for_a_missing_cli_is_409() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path(), no_cli);
    for (agent, name) in [
        ("claude", "Claude Code"),
        ("codex", "Codex"),
        ("antigravity", "Antigravity"),
    ] {
        for kind in ["sign_in", "update"] {
            let (status, text) = create(
                &app,
                serde_json::json!({ "cli_task": { "agent": agent, "kind": kind } }),
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT, "{agent} {kind}");
            assert_eq!(text, format!("{name} isn't installed."));
        }
    }
    assert_eq!(sessions(&app).await["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn cli_task_refusals_from_the_command_start_nothing() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path(), fake_cli);
    let (status, text) = create(
        &app,
        serde_json::json!({ "cli_task": { "agent": "antigravity", "kind": "sign_in" } }),
    )
    .await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "Antigravity has no sign-in command.")
    );
    // The fake CLI sits in the build folder: not WinGet, not npm, not a native install.
    let (status, text) = create(
        &app,
        serde_json::json!({ "cli_task": { "agent": "codex", "kind": "update" } }),
    )
    .await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::CONFLICT, "Marshell can't tell how Codex was installed.")
    );
    assert_eq!(sessions(&app).await["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn a_sign_in_tab_has_no_recording() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path(), fake_cli);

    // A plain running session is recorded, so the 404 below is the sign-in tab's own doing.
    let (status, control) = create(&app, serde_json::json!({ "profile_id": "sleeper" })).await;
    assert_eq!(status, StatusCode::OK);
    let control = tab_of(&control);
    let path = format!("/v1/sessions/{control}/recording");
    assert_eq!(send(&app, api("GET", &path, None)).await.0, StatusCode::OK);
    assert!(wait_for(|| !casts_of(home.path(), &control).is_empty()).await);

    let (status, body) = create(
        &app,
        serde_json::json!({ "cli_task": { "agent": "claude", "kind": "sign_in" } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tab = tab_of(&body);
    let list = sessions(&app).await;
    let row = list["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab)
        .unwrap();
    assert_eq!(row["name"], "Sign in to Claude Code");
    assert_eq!(row["agent"], serde_json::Value::Null);
    assert_eq!(row["state"]["kind"], "running");
    assert_eq!(
        send(&app, api("GET", &format!("/v1/sessions/{tab}/recording"), None))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(casts_of(home.path(), &tab).is_empty());
}

#[tokio::test]
async fn a_cli_task_tab_is_not_remembered_and_reopens_as_a_shell() {
    let home = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let app = app_in(home.path(), fake_cli);
    let (status, body) = create(
        &app,
        serde_json::json!({
            "cwd": folder.path().to_string_lossy(),
            "cli_task": { "agent": "codex", "kind": "sign_in" },
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tab = tab_of(&body);

    // Nothing is remembered for the folder, and the request's folder is not used.
    let (_, layout) = send(&app, api("GET", "/v1/layout", None)).await;
    let layout: serde_json::Value = serde_json::from_str(&layout).unwrap();
    assert_eq!(layout["launcher"], serde_json::json!({}));
    let list = sessions(&app).await;
    assert_ne!(list["sessions"][0]["cwd"], folder.path().to_string_lossy().as_ref());

    // Closed and reopened, the tab is the default shell, which is recorded; it does not sign in again.
    send(&app, api("DELETE", &format!("/v1/sessions/{tab}"), None)).await;
    let (status, reopened) = send(&app, api("POST", "/v1/sessions/reopen", None)).await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    let reopened = tab_of(&reopened);
    let list = sessions(&app).await;
    let row = list["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == reopened)
        .unwrap();
    assert_eq!(row["profile_id"], "custom");
    assert_eq!(row["agent"], serde_json::Value::Null);
    assert_eq!(row["name"], "Sign in to Codex");
    assert!(wait_for(|| !casts_of(home.path(), &reopened).is_empty()).await);
    // Close it so no shell outlives the test.
    send(&app, api("DELETE", &format!("/v1/sessions/{reopened}"), None)).await;
}
