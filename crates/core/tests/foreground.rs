//! The core reads what a shell session is running and puts it in the row's `foreground`. A known
//! agent started in the shell panel is handed off to the session list.
//!
//! Windows only: the programs run under `cmd.exe`, so the process chain has a shell to read.
#![cfg(windows)]

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
/// A copy of fake-agent that `agent_in` reads as Codex (the `codex-` prefix), under a name no real
/// program uses. Never `codex.exe`: the person's own Codex runs on this machine.
const CODEX_COPY: &str = "codex-marshell-test.exe";

fn profile(id: &str, family: ShellFamily, command: Vec<String>) -> Profile {
    Profile {
        id: id.into(),
        name: id.into(),
        command,
        family,
        cwd: None,
        env: Vec::new(),
        shell_integration: false,
        terminal_theme: None,
        detected: true,
    }
}

fn cmd(id: &str, flag: &str, line: String) -> Profile {
    let command = ["cmd.exe", "/d", flag]
        .map(String::from)
        .into_iter()
        .chain([line])
        .collect();
    profile(id, ShellFamily::Cmd, command)
}

/// `codex` comes first, so it is the default profile and the one the shell panel starts.
/// It runs the copy by a path, not a bare name: with NoDefaultCurrentDirectoryInExePath set, cmd
/// would find a real `codex` on PATH instead of the copy in the folder.
fn state_in(home: &Path) -> AppState {
    common::tie_children_to_test();
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let profiles = vec![
        cmd("codex", "/c", format!(r".\{CODEX_COPY} sleep")),
        cmd("prog", "/c", format!("{fake} sleep")),
        cmd("ends", "/k", format!("{fake} sleep-then-exit 3")),
        profile("sleeper", ShellFamily::Posix, vec![fake.into(), "sleep".into()]),
    ];
    AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()], home.to_path_buf(), profiles)
        .with_agent_check(|_| true)
        .with_codex_version(None)
}

/// A folder holding the Codex-named copy of fake-agent.
fn codex_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_fake-agent"), dir.path().join(CODEX_COPY)).unwrap();
    dir
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

async fn list(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    json_of(res).await
}

async fn row(app: &axum::Router, tab: &str) -> Option<serde_json::Value> {
    list(app).await["sessions"]
        .as_array()?
        .iter()
        .find(|s| s["tab_id"] == tab)
        .cloned()
}

/// Polls the row every 100 ms until `done` accepts it. The last row seen, if it never does.
async fn wait_for(
    app: &axum::Router,
    tab: &str,
    limit: Duration,
    done: impl Fn(&serde_json::Value) -> bool,
) -> Result<serde_json::Value, Option<serde_json::Value>> {
    let start = Instant::now();
    loop {
        let current = row(app, tab).await;
        if let Some(r) = &current
            && done(r)
        {
            return Ok(r.clone());
        }
        if start.elapsed() > limit {
            return Err(current);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn create(app: &axum::Router, profile_id: &str, cwd: &Path, agent: Option<&str>) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": profile_id, "cwd": cwd.to_string_lossy(), "agent": agent,
    });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn post(app: &axum::Router, tab: &str, action: &str) -> axum::response::Response {
    app.clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/{action}"), None))
        .await
        .unwrap()
}

/// POST /v1/shell-panel in `cwd`; returns the tab id.
async fn open_panel(app: &axum::Router, cwd: &Path) -> String {
    let body = serde_json::json!({ "cwd": cwd.to_string_lossy(), "cols": 80, "rows": 24 });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/shell-panel", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn adopt(app: &axum::Router) -> axum::response::Response {
    app.clone()
        .oneshot(api("POST", "/v1/shell-panel/adopt", None))
        .await
        .unwrap()
}

fn stop(state: &AppState, tab: &str) {
    if let Some(session) = state.remove_session(tab) {
        session.force_kill();
    }
}

fn has_foreground(r: &serde_json::Value) -> bool {
    !r["foreground"].is_null()
}

#[tokio::test]
async fn a_shell_running_a_program_shows_it() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, "prog", work.path(), None).await;

    let found = wait_for(&app, &tab, Duration::from_secs(10), has_foreground).await;
    stop(&state, &tab);
    let r = found.expect("no foreground within 10 s");
    assert_eq!(r["foreground"]["program"], "fake-agent");
    assert!(r["foreground"]["agent"].is_null());
    assert!(r["agent"].is_null());
}

#[tokio::test]
async fn a_shell_running_an_agent_names_it() {
    let home = tempfile::tempdir().unwrap();
    let dir = codex_dir();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, "codex", dir.path(), None).await;

    let found = wait_for(&app, &tab, Duration::from_secs(10), has_foreground).await;
    stop(&state, &tab);
    tokio::time::sleep(Duration::from_millis(500)).await;
    std::fs::remove_dir_all(dir.path()).ok();
    let r = found.expect("no foreground within 10 s");
    assert_eq!(r["foreground"]["agent"], "codex");
    assert_eq!(r["foreground"]["program"], "codex-marshell-test");
    // The launch agent does not change: Restart and Mode read it.
    assert!(r["agent"].is_null());
}

#[tokio::test]
async fn a_paused_session_is_not_polled() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, "prog", work.path(), None).await;
    // Frozen: the row must keep the foreground it had at that moment.
    // Freezing can be refused for a moment while the shell is still starting its program.
    let mut refusal = String::new();
    for _ in 0..20 {
        let res = post(&app, &tab, "pause").await;
        if res.status() == StatusCode::NO_CONTENT {
            refusal.clear();
            break;
        }
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        refusal = format!("{status} {}", String::from_utf8_lossy(&body));
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(refusal, "", "the pause was refused");

    // Its foreground is whatever it was when the pause took hold, null or not: on a slow cold
    // launch a check can land before the pause. From then on it must not change.
    let at_pause = row(&app, &tab).await.unwrap();
    assert_eq!(at_pause["paused"], true);
    let kept = at_pause["foreground"].clone();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(2500) {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let r = row(&app, &tab).await.unwrap();
        assert_eq!(r["foreground"], kept, "a paused session was polled: {r}");
    }

    // Thawed, it is polled again.
    assert_eq!(post(&app, &tab, "continue").await.status(), StatusCode::NO_CONTENT);
    let found = wait_for(&app, &tab, Duration::from_secs(10), has_foreground).await;
    stop(&state, &tab);
    assert!(found.is_ok(), "no foreground after Continue");
}

#[tokio::test]
async fn a_launched_agent_has_no_foreground() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = create(&app, "sleeper", work.path(), Some("claude")).await;

    tokio::time::sleep(Duration::from_secs(2)).await;
    let r = row(&app, &tab).await.unwrap();
    stop(&state, &tab);
    assert_eq!(r["agent"], "claude");
    assert!(r["foreground"].is_null());
}

#[tokio::test]
async fn the_foreground_clears_when_the_program_ends() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    // `cmd /k`: the shell survives the program, which exits after 3 s.
    let tab = create(&app, "ends", work.path(), None).await;

    let seen = wait_for(&app, &tab, Duration::from_secs(10), has_foreground).await;
    assert!(seen.is_ok(), "the program was never seen");
    let cleared = wait_for(&app, &tab, Duration::from_secs(5), |r| !has_foreground(r)).await;
    let kind = row(&app, &tab).await.unwrap()["state"]["kind"].clone();
    stop(&state, &tab);
    assert!(cleared.is_ok(), "the foreground never cleared");
    assert_eq!(kind, "running", "the shell should have survived the program");
}

#[tokio::test]
async fn an_agent_in_the_panel_is_handed_off() {
    let home = tempfile::tempdir().unwrap();
    let dir = codex_dir();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = open_panel(&app, dir.path()).await;
    // The panel is not in the list until the core hands it off.
    assert!(row(&app, &tab).await.is_none());

    let found = wait_for(&app, &tab, Duration::from_secs(10), |r| {
        r["foreground"]["agent"] == "codex"
    })
    .await;
    // The hand-off took the panel, so a second adopt finds nothing.
    let again = adopt(&app).await.status();
    stop(&state, &tab);
    tokio::time::sleep(Duration::from_millis(500)).await;
    std::fs::remove_dir_all(dir.path()).ok();
    let r = found.expect("the panel's agent was not handed off within 10 s");
    assert!(r["agent"].is_null());
    assert_eq!(r["state"]["kind"], "running");
    assert_eq!(again, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_adopt_by_hand_wins_the_panel_and_the_row_still_follows_its_program() {
    let home = tempfile::tempdir().unwrap();
    let dir = codex_dir();
    let state = state_in(home.path());
    let app = router(state.clone());
    let tab = open_panel(&app, dir.path()).await;

    let manual = adopt(&app).await;
    assert_eq!(manual.status(), StatusCode::OK);
    assert_eq!(json_of(manual).await["tab_id"], tab.as_str());

    // The row is listed once, and the watcher follows it as an ordinary row.
    let found = wait_for(&app, &tab, Duration::from_secs(10), |r| {
        r["foreground"]["agent"] == "codex"
    })
    .await;
    let rows = list(&app).await["sessions"].as_array().unwrap().len();
    stop(&state, &tab);
    tokio::time::sleep(Duration::from_millis(500)).await;
    std::fs::remove_dir_all(dir.path()).ok();
    assert!(found.is_ok(), "the adopted row never showed its agent: {found:?}");
    assert_eq!(rows, 1);
}
