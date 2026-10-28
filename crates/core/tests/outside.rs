//! The resume command and "Open in system terminal" through the API. Every session runs the fake agent in `sleep`
//! mode (the profile's command), and a refused open starts nothing: no test opens a console window.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5561;
const ORIGIN: &str = "http://tauri.localhost";

fn key() -> Option<String> {
    Some("sk-marshell-test-0000".into())
}

fn on_path(command: &str) -> Option<PathBuf> {
    Some(PathBuf::from(command))
}

fn sleeper() -> Profile {
    common::tie_children_to_test();
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

fn state(home: &Path) -> AppState {
    // An agent with no continue flag, as a person's own agents.json can hold.
    let own = json!({
        "id": "no-resume", "name": "No Resume", "cli": "No Resume", "mark": "copilot", "commands": ["no-resume"],
        "prompt": {"style": "none"}, "continue": null, "modes": {"manual": []},
    });
    std::fs::write(
        home.join("agents.json"),
        json!({"version": 1, "agents": [own]}).to_string(),
    )
    .unwrap();
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![sleeper()],
    )
    .with_agent_check(|_| true)
    .with_agent_path(on_path)
    .with_codex_version(Some("9.9.9".into()))
    .with_deepseek_key(key)
}

fn api(method: &str, path: &str, body: Option<Value>) -> Request<Body> {
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
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get(app: &axum::Router, path: &str) -> (StatusCode, Value) {
    let (status, text) = send(app, api("GET", path, None)).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

async fn post(app: &axum::Router, path: &str) -> (StatusCode, Value) {
    let (status, text) = send(app, api("POST", path, None)).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

/// Starts a session of `agent` (None: a plain shell) in `cwd`; its tab id.
async fn start(app: &axum::Router, agent: Option<&str>, cwd: &Path, extra: Value) -> String {
    let mut body = json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy(), "agent": agent,
    });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (status, text) = send(app, api("POST", "/v1/sessions", Some(body))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    serde_json::from_str::<Value>(&text).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Indexes `session` for `tab`, as the SessionStart hook does. The transcript file exists when `write` is set.
fn note(st: &AppState, agent: &str, tab: &str, session: &str, transcript: &Path, cwd: &Path, write: bool) {
    if write {
        std::fs::write(transcript, "{}\n").unwrap();
    }
    let body = json!({
        "hook_event_name": "SessionStart",
        "session_id": session,
        "transcript_path": transcript.to_string_lossy(),
        "cwd": cwd.to_string_lossy(),
    });
    assert!(st.history().note_event(agent, tab, None, &body, 1).is_none());
    assert!(st.history().latest_for_tab(tab).is_some(), "the row was not indexed");
}

fn reason(body: &Value) -> &str {
    body["reason"].as_str().unwrap_or_else(|| panic!("no reason in {body}"))
}

#[tokio::test]
async fn resume_command_by_id() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    let claude = start(&app, Some("claude"), work.path(), json!({"mode": "plan"})).await;
    note(
        &st,
        "claude",
        &claude,
        "sess-1",
        &work.path().join("sess-1.jsonl"),
        work.path(),
        true,
    );
    let (status, body) = get(&app, &format!("/v1/sessions/{claude}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["form"], "by_id");
    let command = body["command"].as_str().unwrap();
    assert!(command.contains("--resume sess-1"), "{command}");
    assert!(command.contains("--permission-mode plan"), "{command}");
    assert!(
        command.starts_with(&format!("cd '{}' && ", work.path().display())),
        "{command}"
    );
    assert!(!command.contains(['\r', '\n']));

    let codex = start(&app, Some("codex"), work.path(), json!({})).await;
    let rollout = work.path().join("rollout-2026-01-01T00-00-00-cdx-1.jsonl");
    note(&st, "codex", &codex, "cdx-1", &rollout, work.path(), true);
    let (status, body) = get(&app, &format!("/v1/sessions/{codex}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["form"], "by_id");
    let command = body["command"].as_str().unwrap();
    assert!(command.contains("codex resume cdx-1"), "{command}");
    assert!(!command.contains("--no-daemon"), "{command}");
}

#[tokio::test]
async fn resume_command_falls_back_to_continue_last_then_fresh() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    // No row for the tab: the CLI's own continue flag.
    let claude = start(&app, Some("claude"), work.path(), json!({})).await;
    let (status, body) = get(&app, &format!("/v1/sessions/{claude}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["form"], "continue_last");
    assert!(
        body["command"].as_str().unwrap().ends_with("claude --continue"),
        "{body}"
    );

    // A row whose transcript was never written: a new conversation, with no resume flag.
    let fresh = start(&app, Some("claude"), work.path(), json!({})).await;
    note(
        &st,
        "claude",
        &fresh,
        "new-1",
        &work.path().join("new-1.jsonl"),
        work.path(),
        false,
    );
    let (status, body) = get(&app, &format!("/v1/sessions/{fresh}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["form"], "fresh");
    let command = body["command"].as_str().unwrap();
    assert!(command.ends_with("claude"), "{command}");
}

#[tokio::test]
async fn resume_command_continue_last_for_a_manifest_agent() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));

    let copilot = start(&app, Some("copilot"), work.path(), json!({})).await;
    let (status, body) = get(&app, &format!("/v1/sessions/{copilot}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["form"], "continue_last");
    assert!(
        body["command"].as_str().unwrap().ends_with("copilot --continue"),
        "{body}"
    );

    // Aider's manifest has its own continue flag, so it has a command too.
    let aider = start(&app, Some("aider"), work.path(), json!({})).await;
    let (status, body) = get(&app, &format!("/v1/sessions/{aider}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["command"]
            .as_str()
            .unwrap()
            .ends_with("aider --restore-chat-history"),
        "{body}"
    );

    // An agent whose manifest has no `continue`: refused, for the copy and for the console.
    let own = start(&app, Some("no-resume"), work.path(), json!({})).await;
    let (status, body) = get(&app, &format!("/v1/sessions/{own}/resume-command")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(reason(&body), "No Resume has no resume command.");
}

#[tokio::test]
async fn resume_command_refuses_a_preset() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));

    let tab = start(&app, Some("claude"), work.path(), json!({"preset": "deepseek"})).await;
    let (status, body) = get(&app, &format!("/v1/sessions/{tab}/resume-command")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        reason(&body),
        "A DeepSeek session needs Marshell to start it, so there is no command to copy."
    );
}

#[tokio::test]
async fn open_outside_refuses_a_preset() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));

    let tab = start(&app, Some("claude"), work.path(), json!({"preset": "deepseek"})).await;
    let (status, body) = post(&app, &format!("/v1/sessions/{tab}/open-outside")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(reason(&body), "A DeepSeek session needs Marshell to start it.");
}

#[tokio::test]
async fn open_outside_refuses_a_running_tab() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));

    let tab = start(&app, Some("claude"), work.path(), json!({})).await;
    let (status, body) = post(&app, &format!("/v1/sessions/{tab}/open-outside")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(reason(&body), "End the session first.");
}

#[tokio::test]
async fn open_outside_refuses_a_plain_shell_and_an_unknown_tab() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));

    let shell = start(&app, None, work.path(), json!({})).await;
    assert_eq!(
        post(&app, &format!("/v1/sessions/{shell}/open-outside")).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        get(&app, &format!("/v1/sessions/{shell}/resume-command")).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(&app, "/v1/sessions/nope/open-outside").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, "/v1/sessions/nope/resume-command").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn resume_command_for_a_conversation_in_history() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    let tab = start(&app, Some("claude"), work.path(), json!({})).await;
    note(
        &st,
        "claude",
        &tab,
        "hist-1",
        &work.path().join("hist-1.jsonl"),
        work.path(),
        true,
    );

    // Copying works while the conversation runs in a tab; opening outside does not.
    let (status, body) = get(&app, "/v1/history/claude/hist-1/resume-command").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["form"], "by_id");
    let command = body["command"].as_str().unwrap();
    assert!(command.contains("claude --resume hist-1"), "{command}");
    assert!(
        command.starts_with(&format!("cd '{}' && ", work.path().display())),
        "{command}"
    );
    let (status, body) = post(&app, "/v1/history/claude/hist-1/open-outside").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(reason(&body), "End the session first.");

    // A conversation Marshell has never seen, and an id that isn't one.
    assert_eq!(
        get(&app, "/v1/history/claude/nope-1/resume-command").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&app, "/v1/history/claude/..%2Fx/resume-command").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(&app, "/v1/history/aider/hist-1/open-outside").await.0,
        StatusCode::NOT_FOUND
    );

    // The transcript file goes: the same refusal as Resume.
    std::fs::remove_file(work.path().join("hist-1.jsonl")).unwrap();
    let (status, body) = get(&app, "/v1/history/claude/hist-1/resume-command").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        reason(&body),
        "Claude Code no longer has this conversation, so it can't be resumed."
    );
}

#[tokio::test]
async fn the_new_routes_are_refused_without_the_bearer() {
    let home = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));
    for (method, path) in [
        ("GET", "/v1/sessions/t1/resume-command"),
        ("POST", "/v1/sessions/t1/open-outside"),
        ("GET", "/v1/history/claude/s1/resume-command"),
        ("POST", "/v1/history/claude/s1/open-outside"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(&app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::AUTHORIZATION, "Bearer wrong")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(&app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path} with a wrong token");
    }
}
