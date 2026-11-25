//! The default folder: where a session starts when nothing names a folder, what `GET /v1/default-folder` says, how
//! Settings changes it, and that only Marshell's own folder is ever made (at start, and when Settings goes back to it).
//! Every test names a scratch folder: none reads or makes the real `~/Marshell`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, CoreConfig, router, start};
use marshell_protocol::api::{Profile, Settings, ShellFamily};
use std::path::{Path, PathBuf};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5558;
const ORIGIN: &str = "http://tauri.localhost";

fn fake_cli(_command: &str) -> Option<PathBuf> {
    Some(PathBuf::from(env!("CARGO_BIN_EXE_fake-agent")))
}

fn sleeper() -> Profile {
    Profile {
        id: "sleeper".into(),
        name: "sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

/// A state whose profile has no folder. `default_folder` is the core's own folder, None for a core with none.
fn state_in(home: &Path, default_folder: Option<PathBuf>) -> AppState {
    common::tie_children_to_test();
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![sleeper()],
    )
    .with_agent_path(fake_cli)
    .with_agent_check(|_| true);
    match default_folder {
        Some(dir) => state.with_default_folder(dir),
        None => state,
    }
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

async fn json_get(app: &axum::Router, path: &str) -> serde_json::Value {
    let (status, text) = send(app, api("GET", path, None)).await;
    assert_eq!(status, StatusCode::OK, "{path}: {text}");
    serde_json::from_str(&text).unwrap()
}

async fn put_settings(app: &axum::Router, settings: &Settings) -> (StatusCode, String) {
    send(
        app,
        api("PUT", "/v1/settings", Some(serde_json::to_value(settings).unwrap())),
    )
    .await
}

/// POST /v1/sessions with `extra` added to a plain body; returns the tab id.
async fn create(app: &axum::Router, extra: serde_json::Value) -> String {
    let mut body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper" });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (status, text) = send(app, api("POST", "/v1/sessions", Some(body))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    serde_json::from_str::<serde_json::Value>(&text).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The `cwd` the listing gives a tab.
async fn cwd_of(app: &axum::Router, tab: &str) -> String {
    let list = json_get(app, "/v1/sessions").await;
    list["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab)
        .unwrap_or_else(|| panic!("no row for {tab}"))["cwd"]
        .as_str()
        .unwrap()
        .to_string()
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn home_text() -> String {
    text(&dirs::home_dir().unwrap())
}

#[tokio::test]
async fn a_session_with_no_folder_starts_in_the_default_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    std::fs::create_dir_all(&own).unwrap();
    let app = router(state_in(&tmp.path().join("home"), Some(own.clone())));
    let tab = create(&app, serde_json::json!({})).await;
    assert_eq!(cwd_of(&app, &tab).await, text(&own));
}

#[tokio::test]
async fn a_requested_folder_beats_the_default_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    let asked = tmp.path().join("asked");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::create_dir_all(&asked).unwrap();
    let app = router(state_in(&tmp.path().join("home"), Some(own)));
    let tab = create(&app, serde_json::json!({ "cwd": text(&asked) })).await;
    assert_eq!(cwd_of(&app, &tab).await, text(&asked));
}

#[tokio::test]
async fn a_missing_default_folder_gives_way_to_home() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    let app = router(state_in(&tmp.path().join("home"), Some(own.clone())));
    let tab = create(&app, serde_json::json!({})).await;
    assert_eq!(cwd_of(&app, &tab).await, home_text());
    assert!(!own.exists(), "starting a session made the default folder");
}

#[tokio::test]
async fn a_restart_keeps_the_sessions_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    let a = tmp.path().join("a");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::create_dir_all(&a).unwrap();
    let state = state_in(&tmp.path().join("home"), Some(own));
    let app = router(state.clone());
    let tab = create(
        &app,
        serde_json::json!({ "cwd": text(&a), "agent": "claude", "mode": "manual" }),
    )
    .await;
    let body = serde_json::json!({ "mode": "bypass", "cols": 80, "rows": 24 });
    let (status, answer) = send(&app, api("POST", &format!("/v1/sessions/{tab}/restart"), Some(body))).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(cwd_of(&app, &tab).await, text(&a));
    if let Some(session) = state.remove_session(&tab) {
        session.force_kill();
    }
}

#[tokio::test]
async fn a_sign_in_tab_stays_in_the_home_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    std::fs::create_dir_all(&own).unwrap();
    let app = router(state_in(&tmp.path().join("home"), Some(own)));
    let tab = create(
        &app,
        serde_json::json!({ "profile_id": null, "cli_task": { "agent": "claude", "kind": "sign_in" } }),
    )
    .await;
    assert_eq!(cwd_of(&app, &tab).await, home_text());
}

#[tokio::test]
async fn default_folder_answers_path_exists_and_chosen() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    let app = router(state_in(&tmp.path().join("home"), Some(own.clone())));
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": text(&own), "exists": false, "chosen": false })
    );
    std::fs::create_dir_all(&own).unwrap();
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": text(&own), "exists": true, "chosen": false })
    );
    let settings = Settings {
        default_folder: Some(text(&other)),
        ..Settings::default()
    };
    assert_eq!(put_settings(&app, &settings).await.0, StatusCode::OK);
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": text(&other), "exists": true, "chosen": true })
    );
}

#[tokio::test]
async fn a_core_with_no_default_folder_answers_none() {
    let tmp = tempfile::tempdir().unwrap();
    let app = router(state_in(&tmp.path().join("home"), None));
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": null, "exists": false, "chosen": false })
    );
}

#[tokio::test]
async fn a_file_where_the_default_folder_should_be_is_reported_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let own = tmp.path().join("Marshell");
    std::fs::write(&own, "not a folder").unwrap();
    let app = router(state_in(&tmp.path().join("home"), Some(own.clone())));
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": text(&own), "exists": false, "chosen": false })
    );
    let tab = create(&app, serde_json::json!({})).await;
    assert_eq!(cwd_of(&app, &tab).await, home_text());
}

#[tokio::test]
async fn default_folder_is_refused_without_the_bearer() {
    let tmp = tempfile::tempdir().unwrap();
    let app = router(state_in(&tmp.path().join("home"), None));
    let mut req = api("GET", "/v1/default-folder", None);
    req.headers_mut().remove(header::AUTHORIZATION);
    assert_eq!(send(&app, req).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn put_takes_a_default_folder_that_is_there() {
    let tmp = tempfile::tempdir().unwrap();
    let chosen = tmp.path().join("chosen");
    std::fs::create_dir_all(&chosen).unwrap();
    let app = router(state_in(&tmp.path().join("home"), None));
    let settings = Settings {
        default_folder: Some(text(&chosen)),
        ..Settings::default()
    };
    assert_eq!(put_settings(&app, &settings).await.0, StatusCode::OK);
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": text(&chosen), "exists": true, "chosen": true })
    );
}

#[tokio::test]
async fn put_refuses_a_default_folder_that_is_not_there() {
    let tmp = tempfile::tempdir().unwrap();
    let nope = tmp.path().join("nope");
    let app = router(state_in(&tmp.path().join("home"), None));
    let settings = Settings {
        default_folder: Some(text(&nope)),
        ..Settings::default()
    };
    let (status, body) = put_settings(&app, &settings).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::BAD_REQUEST, "That folder isn't there.")
    );
    assert!(!nope.exists(), "the refused save made the folder");
    assert!(json_get(&app, "/v1/settings").await["default_folder"].is_null());
}

#[tokio::test]
async fn going_back_to_marshells_folder_makes_it() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let chosen = tmp.path().join("chosen");
    std::fs::create_dir_all(&chosen).unwrap();
    // A chosen folder was saved in an earlier run.
    let saved = Settings {
        default_folder: Some(text(&chosen)),
        ..Settings::default()
    };
    std::fs::write(
        marshell_core::paths::settings_file(&home),
        serde_json::to_string(&saved).unwrap(),
    )
    .unwrap();
    let own = tmp.path().join("Marshell");
    let app = router(state_in(&home, Some(own.clone())));
    assert_eq!(json_get(&app, "/v1/default-folder").await["chosen"], true);
    assert!(!own.exists());

    let (status, body) = put_settings(&app, &Settings::default()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(own.is_dir(), "going back to Marshell's folder did not make it");
    assert_eq!(
        json_get(&app, "/v1/default-folder").await,
        serde_json::json!({ "path": text(&own), "exists": true, "chosen": false })
    );
}

#[tokio::test]
async fn put_refuses_a_share_as_the_default_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let app = router(state_in(&tmp.path().join("home"), None));
    let settings = Settings {
        default_folder: Some(r"\\server\share\x".into()),
        ..Settings::default()
    };
    let (status, body) = put_settings(&app, &settings).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::BAD_REQUEST, "Choose a folder on this PC.")
    );
}

fn config(home: &Path, default_folder: Option<PathBuf>) -> CoreConfig {
    CoreConfig {
        home: home.to_path_buf(),
        allowed_origins: vec![],
        programs_dir: None,
        notify: None,
        focus: None,
        notify_paused: Default::default(),
        window_hidden: Default::default(),
        import: None,
        doctor_on_start: false,
        default_folder,
    }
}

#[tokio::test]
async fn start_makes_the_default_folder_before_it_serves() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let own = tmp.path().join("Marshell");
    let core = start(config(&home, Some(own.clone()))).await.unwrap();
    // The endpoint file is written last, so a window that found it finds the folder too.
    assert!(own.is_dir(), "start returned before the default folder was made");
    assert!(marshell_core::paths::endpoint_file(&home).exists());
    // The effective path never goes into settings.json.
    if let Ok(file) = std::fs::read_to_string(marshell_core::paths::settings_file(&home)) {
        let saved: serde_json::Value = serde_json::from_str(&file).unwrap();
        assert!(saved["default_folder"].is_null(), "{file}");
    }
    core.shutdown();
}

#[tokio::test]
async fn start_does_not_make_a_chosen_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let chosen = tmp.path().join("chosen");
    let own = tmp.path().join("Marshell");
    let saved = Settings {
        default_folder: Some(text(&chosen)),
        ..Settings::default()
    };
    std::fs::write(
        marshell_core::paths::settings_file(&home),
        serde_json::to_string(&saved).unwrap(),
    )
    .unwrap();
    let core = start(config(&home, Some(own.clone()))).await.unwrap();
    assert!(!chosen.exists(), "start made the folder Settings names");
    assert!(
        !own.exists(),
        "start made Marshell's folder though Settings names another"
    );
    core.shutdown();
}
