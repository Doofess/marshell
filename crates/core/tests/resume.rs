//! Resume starts an Ended tab again in the same row: same id, number, name and place.

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

/// A profile that runs the fake agent in `mode`: "sleep" stays up, "print" writes its own arguments and ends.
fn profile(id: &str, mode: &str) -> Profile {
    Profile {
        id: id.into(),
        name: id.into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), mode.into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

fn state_in(home: &Path, profile_id: &str) -> AppState {
    state_running(home, profile_id, "sleep")
}

fn state_running(home: &Path, profile_id: &str, mode: &str) -> AppState {
    common::tie_children_to_test();
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![profile(profile_id, mode)],
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

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn sessions(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    json_of(res).await
}

async fn resume(app: &axum::Router, tab: &str) -> axum::response::Response {
    let body = serde_json::json!({ "cols": 80, "rows": 24 });
    app.clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/resume"), Some(body)))
        .await
        .unwrap()
}

/// True once layout.json lists `count` tabs (the save waits out a short delay).
async fn wait_for_saved(home: &Path, count: usize) {
    let start = Instant::now();
    loop {
        let saved = std::fs::read(home.join("layout.json"))
            .ok()
            .and_then(|text| serde_json::from_slice::<serde_json::Value>(&text).ok())
            .and_then(|doc| doc["tabs"].as_array().map(Vec::len));
        if saved == Some(count) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "layout.json never listed {count} tabs"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Opens two tabs in `cwd` (the second is the one to resume), then restarts the core on the
/// same home with `profile_id` as its only profile, so both come back Ended. Returns the new
/// state, its router and the two tab ids.
async fn restarted(home: &Path, cwd: &Path, profile_id: &str) -> (AppState, axum::Router, String, String) {
    restarted_with(home, cwd, profile_id, None, |_| true).await
}

/// Like `restarted`, but the tabs run `agent` and the restarted core finds agents with `agent_on_path`.
async fn restarted_with(
    home: &Path,
    cwd: &Path,
    profile_id: &str,
    agent: Option<&str>,
    agent_on_path: fn(&str) -> bool,
) -> (AppState, axum::Router, String, String) {
    restarted_running(home, cwd, profile_id, agent, agent_on_path, "sleep").await
}

/// Like `restarted_with`, but the restarted core's profile runs the fake agent in `mode`.
async fn restarted_running(
    home: &Path,
    cwd: &Path,
    profile_id: &str,
    agent: Option<&str>,
    agent_on_path: fn(&str) -> bool,
    mode: &str,
) -> (AppState, axum::Router, String, String) {
    let first = state_in(home, "sleeper").with_agent_check(|_| true);
    let app = router(first.clone());
    let mut ids = Vec::new();
    for name in ["alpha", "beta"] {
        let body = serde_json::json!({
            "cols": 80, "rows": 24, "profile_id": "sleeper", "name": name, "cwd": cwd.to_string_lossy(),
            "agent": agent,
        });
        let res = app
            .clone()
            .oneshot(api("POST", "/v1/sessions", Some(body)))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        ids.push(json_of(res).await["tab_id"].as_str().unwrap().to_string());
    }
    wait_for_saved(home, 2).await;
    for id in &ids {
        if let Some(session) = first.remove_session(id) {
            session.force_kill();
        }
    }
    drop(app);
    drop(first);
    let second = state_running(home, profile_id, mode).with_agent_check(agent_on_path);
    let app = router(second.clone());
    (second, app, ids[0].clone(), ids[1].clone())
}

fn stop_all(state: &AppState, ids: &[&str]) {
    for id in ids {
        if let Some(session) = state.remove_session(id) {
            session.force_kill();
        }
    }
}

#[tokio::test]
async fn resume_restarts_ended_tab_in_place() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let (state, app, a, b) = restarted(home.path(), work.path(), "sleeper").await;
    let before = sessions(&app).await;
    assert_eq!(before["sessions"][1]["state"]["kind"], "ended");

    let res = resume(&app, &b).await;
    assert_eq!(res.status(), StatusCode::OK);
    let started = json_of(res).await;
    assert_eq!(started["tab_id"], b.as_str());
    assert_eq!(started["number"], before["sessions"][1]["number"]);

    let after = sessions(&app).await;
    let rows = after["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], before["sessions"][0]);
    assert_eq!(rows[0]["tab_id"], a.as_str());
    // Same id, number, name and place; only the state changed.
    assert_eq!(rows[1]["tab_id"], b.as_str());
    assert_eq!(rows[1]["number"], before["sessions"][1]["number"]);
    assert_eq!(rows[1]["name"], "beta");
    assert_eq!(rows[1]["state"]["kind"], "running");
    assert_eq!(state.live_session_count(), 1);
    assert!(state.session(&b).is_some());
    stop_all(&state, &[&a, &b]);
}

// Several worker threads, so the two requests really run at the same time and one of them
// reaches begin_resume while the other is inside it. On one thread the second request would
// always be turned away by the early state check, and begin_resume's own check would go untested.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resume_twice_starts_one_process() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let (state, app, a, b) = restarted(home.path(), work.path(), "sleeper").await;

    let (first, second) = (app.clone(), app.clone());
    let (tab_one, tab_two) = (b.clone(), b.clone());
    let one = tokio::spawn(async move { resume(&first, &tab_one).await });
    let two = tokio::spawn(async move { resume(&second, &tab_two).await });
    let (one, two) = (one.await.unwrap(), two.await.unwrap());
    let mut codes = [one.status(), two.status()];
    codes.sort();
    assert_eq!(codes, [StatusCode::OK, StatusCode::CONFLICT]);
    let refused = if one.status() == StatusCode::CONFLICT { one } else { two };
    assert_eq!(json_of(refused).await["reason"], "It is already running.");
    assert_eq!(state.live_session_count(), 1);
    stop_all(&state, &[&a, &b]);
}

#[tokio::test]
async fn resume_refuses_missing_profile() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let (state, app, _a, b) = restarted(home.path(), work.path(), "other").await;

    let res = resume(&app, &b).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(res).await["reason"], "Its shell profile was removed.");
    // Nothing changed: still Ended, nothing started.
    assert_eq!(sessions(&app).await["sessions"][1]["state"]["kind"], "ended");
    assert_eq!(state.live_session_count(), 0);
}

#[tokio::test]
async fn resume_refuses_missing_folder() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let cwd = work.path().join("gone");
    std::fs::create_dir(&cwd).unwrap();
    let (state, app, _a, b) = restarted(home.path(), &cwd, "sleeper").await;
    std::fs::remove_dir(&cwd).unwrap();

    let res = resume(&app, &b).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_of(res).await["reason"],
        format!("Its folder no longer exists: {}", cwd.to_string_lossy())
    );
    assert_eq!(sessions(&app).await["sessions"][1]["state"]["kind"], "ended");
    assert_eq!(state.live_session_count(), 0);
}

#[tokio::test]
async fn resume_unknown_tab_is_404() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path(), "sleeper");
    let app = router(state);
    assert_eq!(resume(&app, "nope").await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn resume_refuses_an_agent_that_is_not_installed() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let (state, app, _a, b) = restarted_with(home.path(), work.path(), "sleeper", Some("claude"), |_| false).await;

    let res = resume(&app, &b).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_of(res).await["reason"],
        "Claude Code isn't installed or isn't on PATH."
    );
    assert_eq!(sessions(&app).await["sessions"][1]["state"]["kind"], "ended");
    assert_eq!(state.live_session_count(), 0);
}

#[tokio::test]
async fn resume_refuses_a_conversation_claude_no_longer_has() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let (state, app, _a, b) = restarted_with(home.path(), work.path(), "sleeper", Some("claude"), |_| true).await;
    // The index knows the tab's session, and so does the archive, but the transcript file is gone.
    let transcript = work.path().join("gone-1.jsonl");
    let body = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": "gone-1",
        "transcript_path": transcript.to_string_lossy(),
        "cwd": work.path().to_string_lossy(),
    });
    std::fs::write(&transcript, "{}\n").unwrap();
    assert!(state.history().note_event(&b, &body, 1).is_none());
    let meta = state.history().latest_for_tab(&b).unwrap();
    // Marshell holds a copy of it, then Claude's file goes: we had it and it was lost.
    marshell_core::history::archive::copy_transcript(home.path(), &meta).unwrap();
    std::fs::remove_file(&transcript).unwrap();

    let res = resume(&app, &b).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_of(res).await["reason"],
        "Claude Code no longer has this conversation, so it can't be resumed."
    );
    assert_eq!(sessions(&app).await["sessions"][1]["state"]["kind"], "ended");
    assert_eq!(state.live_session_count(), 0);
}

/// The rows `peek` returns once `text` shows up in them.
async fn peek_until(app: &axum::Router, tab: &str, text: &str) -> String {
    let start = Instant::now();
    loop {
        let res = app
            .clone()
            .oneshot(api("GET", &format!("/v1/sessions/{tab}/peek?lines=30"), None))
            .await
            .unwrap();
        let rows = json_of(res).await["rows"].to_string();
        if rows.contains(text) {
            return rows;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "never saw {text:?} in {rows}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn resume_starts_fresh_when_the_session_never_wrote_a_transcript() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    // The fake agent prints the arguments it was started with, so the command line can be read back.
    let (state, app, a, b) =
        restarted_running(home.path(), work.path(), "sleeper", Some("claude"), |_| true, "print").await;
    // SessionStart indexed the session; Claude writes the file only at the first message.
    let transcript = work.path().join("new-1.jsonl");
    let body = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": "new-1",
        "transcript_path": transcript.to_string_lossy(),
        "cwd": work.path().to_string_lossy(),
    });
    assert!(state.history().note_event(&b, &body, 1).is_none());

    let res = resume(&app, &b).await;
    assert_eq!(res.status(), StatusCode::OK);
    let rows = peek_until(&app, &b, "claude").await;
    assert!(!rows.contains("--resume"), "{rows}");
    assert!(!rows.contains("--continue"), "{rows}");
    assert_eq!(sessions(&app).await["sessions"][1]["tab_id"], b.as_str());
    stop_all(&state, &[&a, &b]);
}
