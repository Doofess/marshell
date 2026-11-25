//! The sidebar shell panel runs in its own pty: not a tab, not saved, until "Open in a pane" adopts it.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn state_in(home: &Path) -> AppState {
    state_with(home, None, None)
}

/// A state whose profile starts in `profile_cwd` and whose core has `default_folder` (None: no default folder).
/// Tests name scratch folders only; none reads or makes the real `~/Marshell`.
fn state_with(home: &Path, profile_cwd: Option<&Path>, default_folder: Option<&Path>) -> AppState {
    common::tie_children_to_test();
    let profile = Profile {
        id: "sleeper".into(),
        name: "Sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: profile_cwd.map(|p| p.to_string_lossy().into_owned()),
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![profile],
    );
    match default_folder {
        Some(dir) => state.with_default_folder(dir.to_path_buf()),
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

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn sessions(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    json_of(res).await
}

/// POST /v1/shell-panel in `cwd` (none: the request names no folder); returns the tab id and the folder it answers.
async fn open_panel_as(app: &axum::Router, cwd: Option<&Path>) -> (String, String) {
    let mut body = serde_json::json!({ "cols": 80, "rows": 24 });
    if let Some(cwd) = cwd {
        body["cwd"] = cwd.to_string_lossy().into_owned().into();
    }
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/shell-panel", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let answer = json_of(res).await;
    (
        answer["tab_id"].as_str().unwrap().to_string(),
        answer["cwd"].as_str().unwrap().to_string(),
    )
}

/// POST /v1/shell-panel in `cwd`; returns the tab id.
async fn open_panel(app: &axum::Router, cwd: &Path) -> String {
    open_panel_as(app, Some(cwd)).await.0
}

/// The paths GET /v1/projects lists.
async fn project_paths(app: &axum::Router) -> Vec<String> {
    let res = app.clone().oneshot(api("GET", "/v1/projects", None)).await.unwrap();
    json_of(res)
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["path"].as_str().unwrap().to_string())
        .collect()
}

async fn adopt(app: &axum::Router) -> axum::response::Response {
    app.clone()
        .oneshot(api("POST", "/v1/shell-panel/adopt", None))
        .await
        .unwrap()
}

/// The ids in layout.json's `tabs`, or none when the file is not there yet.
fn saved_ids(home: &Path) -> Vec<String> {
    std::fs::read(home.join("layout.json"))
        .ok()
        .and_then(|text| serde_json::from_slice::<serde_json::Value>(&text).ok())
        .and_then(|doc| doc["tabs"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t["tab_id"].as_str().map(String::from))
        .collect()
}

async fn wait_for_saved(home: &Path, count: usize) {
    let start = Instant::now();
    while saved_ids(home).len() != count {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "layout.json never listed {count} tabs"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn listed(snapshot: &serde_json::Value, tab: &str) -> Option<serde_json::Value> {
    snapshot["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab)
        .cloned()
}

#[tokio::test]
async fn panel_shell_is_not_listed() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let panel = open_panel(&app, home.path()).await;
    assert!(listed(&sessions(&app).await, &panel).is_none());
    // It runs, so the pty route can find it.
    assert!(state.session(&panel).is_some());
    state.session(&panel).unwrap().force_kill();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_panel_requests_at_once_share_one_shell() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let app = app.clone();
        let cwd = home.path().to_path_buf();
        tasks.push(tokio::spawn(async move { open_panel_as(&app, Some(&cwd)).await }));
    }
    let (first, first_cwd) = tasks.remove(0).await.unwrap();
    let (second, second_cwd) = tasks.remove(0).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first_cwd, second_cwd);
    // The loser's extra pty is gone from the map.
    assert_eq!(state.live_session_count(), 1);
    state.session(&first).unwrap().force_kill();
}

#[tokio::test]
async fn panel_shell_is_reused() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let first = open_panel(&app, home.path()).await;
    let second = open_panel(&app, home.path()).await;
    assert_eq!(first, second);
    state.session(&first).unwrap().force_kill();
}

#[tokio::test]
async fn a_panel_shell_that_exited_is_replaced() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let first = open_panel(&app, home.path()).await;
    state.session(&first).unwrap().force_kill();
    let start = Instant::now();
    while !state.session(&first).unwrap().has_exited() {
        assert!(start.elapsed() < Duration::from_secs(5), "the panel shell never exited");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // Nothing to adopt, and the dead one is not handed out again.
    assert_eq!(adopt(&app).await.status(), StatusCode::NOT_FOUND);
    let second = open_panel(&app, home.path()).await;
    assert_ne!(first, second);
    state.session(&second).unwrap().force_kill();
}

#[tokio::test]
async fn adopt_without_a_panel_shell_is_404() {
    let home = tempfile::tempdir().unwrap();
    let app = router(state_in(home.path()));
    assert_eq!(adopt(&app).await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn adopt_lists_it_and_frees_the_panel() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let panel = open_panel(&app, home.path()).await;

    let res = adopt(&app).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_of(res).await["tab_id"], panel.as_str());

    let row = listed(&sessions(&app).await, &panel).expect("the adopted shell is a tab");
    assert_eq!(row["state"]["kind"], "running");
    assert_eq!(row["name"], "Sleeper");
    assert_eq!(row["profile_id"], "sleeper");

    // The panel is free again: the next one is a new shell, and the tab keeps running.
    let next = open_panel(&app, home.path()).await;
    assert_ne!(next, panel);
    assert!(state.session(&panel).is_some());
    state.session(&panel).unwrap().force_kill();
    state.session(&next).unwrap().force_kill();
}

#[tokio::test]
async fn panel_shell_is_not_saved_to_layout() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    // A real tab makes the saver write layout.json.
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": home.path().to_string_lossy(),
    });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tab = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    wait_for_saved(home.path(), 1).await;

    let panel = open_panel(&app, home.path()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(saved_ids(home.path()), vec![tab.clone()]);

    // Once adopted it is a tab like any other, and is saved.
    assert_eq!(adopt(&app).await.status(), StatusCode::OK);
    wait_for_saved(home.path(), 2).await;
    assert_eq!(saved_ids(home.path()), vec![tab.clone(), panel.clone()]);
    state.session(&tab).unwrap().force_kill();
    state.session(&panel).unwrap().force_kill();
}

/// The `.cast` files in the home's recordings folder that belong to `tab`.
fn casts_of(home: &Path, tab: &str) -> Vec<String> {
    std::fs::read_dir(home.join("history").join("recordings"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(&format!("{tab}-")) && n.ends_with(".cast"))
        .collect()
}

async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(10), "{what}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn a_panel_shell_that_ends_unadopted_leaves_no_recording() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let first = open_panel(&app, home.path()).await;
    assert_eq!(
        casts_of(home.path(), &first).len(),
        1,
        "the panel records from the start"
    );
    state.session(&first).unwrap().force_kill();
    wait_until("the panel shell never exited", || {
        state.session(&first).unwrap().has_exited()
    })
    .await;
    // The next open finds the dead shell and drops it, and its recording with it.
    let second = open_panel(&app, home.path()).await;
    wait_until("the recording stayed", || casts_of(home.path(), &first).is_empty()).await;
    assert_eq!(casts_of(home.path(), &second).len(), 1);
    state.session(&second).unwrap().force_kill();
}

#[tokio::test]
async fn an_adopted_panel_shell_keeps_its_recording() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let panel = open_panel(&app, home.path()).await;
    assert_eq!(adopt(&app).await.status(), StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(casts_of(home.path(), &panel).len(), 1);
    state.session(&panel).unwrap().force_kill();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_duplicate_loser_panel_recording_goes() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let app = app.clone();
        let cwd = home.path().to_path_buf();
        tasks.push(tokio::spawn(async move { open_panel(&app, &cwd).await }));
    }
    let first = tasks.remove(0).await.unwrap();
    let _ = tasks.remove(0).await.unwrap();
    wait_until("the loser's recording stayed", || {
        std::fs::read_dir(home.path().join("history").join("recordings"))
            .map(|d| d.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
            == 1
    })
    .await;
    assert_eq!(casts_of(home.path(), &first).len(), 1);
    state.session(&first).unwrap().force_kill();
}

#[tokio::test]
async fn a_panel_with_no_folder_starts_in_the_default_folder() {
    let home = tempfile::tempdir().unwrap();
    let default = tempfile::tempdir().unwrap();
    let state = state_with(home.path(), None, Some(default.path()));
    let app = router(state.clone());
    let (panel, cwd) = open_panel_as(&app, None).await;
    assert_eq!(Path::new(&cwd), default.path());
    state.session(&panel).unwrap().force_kill();
}

#[tokio::test]
async fn the_profile_folder_wins_for_the_panel() {
    let home = tempfile::tempdir().unwrap();
    let default = tempfile::tempdir().unwrap();
    let profile_folder = tempfile::tempdir().unwrap();
    let state = state_with(home.path(), Some(profile_folder.path()), Some(default.path()));
    let app = router(state.clone());
    let (panel, cwd) = open_panel_as(&app, None).await;
    assert_eq!(Path::new(&cwd), profile_folder.path());
    state.session(&panel).unwrap().force_kill();
}

#[tokio::test]
async fn a_panel_with_a_folder_starts_there() {
    let home = tempfile::tempdir().unwrap();
    let default = tempfile::tempdir().unwrap();
    let named = tempfile::tempdir().unwrap();
    let state = state_with(home.path(), None, Some(default.path()));
    let app = router(state.clone());
    let (panel, cwd) = open_panel_as(&app, Some(named.path())).await;
    assert_eq!(Path::new(&cwd), named.path());
    state.session(&panel).unwrap().force_kill();
}

#[tokio::test]
async fn a_running_panel_answers_its_own_folder() {
    let home = tempfile::tempdir().unwrap();
    let default = tempfile::tempdir().unwrap();
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let state = state_with(home.path(), None, Some(default.path()));
    let app = router(state.clone());
    let (first, first_cwd) = open_panel_as(&app, Some(a.path())).await;
    let (second, second_cwd) = open_panel_as(&app, Some(b.path())).await;
    assert_eq!(first, second);
    assert_eq!(Path::new(&first_cwd), a.path());
    assert_eq!(Path::new(&second_cwd), a.path());
    state.session(&first).unwrap().force_kill();
}

#[tokio::test]
async fn a_panel_in_the_default_folder_records_no_project() {
    let home = tempfile::tempdir().unwrap();
    let default = tempfile::tempdir().unwrap();
    let state = state_with(home.path(), None, Some(default.path()));
    let app = router(state.clone());
    let (panel, _) = open_panel_as(&app, None).await;
    assert_eq!(project_paths(&app).await, Vec::<String>::new());
    state.session(&panel).unwrap().force_kill();
}

#[tokio::test]
async fn a_panel_in_a_named_folder_records_it() {
    let home = tempfile::tempdir().unwrap();
    let default = tempfile::tempdir().unwrap();
    let named = tempfile::tempdir().unwrap();
    let state = state_with(home.path(), None, Some(default.path()));
    let app = router(state.clone());
    let (panel, _) = open_panel_as(&app, Some(named.path())).await;
    assert_eq!(
        project_paths(&app).await,
        vec![named.path().to_string_lossy().into_owned()]
    );
    state.session(&panel).unwrap().force_kill();
}
