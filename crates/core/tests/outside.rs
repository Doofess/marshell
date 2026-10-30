//! The resume command and "Open in system terminal" through the API. Every session runs the fake agent in `sleep`
//! mode (the profile's command), and no test opens a console window: the state's console only records what it would run.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::outside::ConsoleProbe;
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5561;
const ORIGIN: &str = "http://tauri.localhost";
/// What the state's tab PATH is swapped to, so no test depends on this machine's PATH.
const TAB_PATH: &str = "/zz-marshell-tab-path";
const OPEN_IN_TERMINAL: &str = "It's open in a system terminal.";

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
    .with_tab_path(|| Some(TAB_PATH.into()))
    .with_console(record_console)
}

/// Stands in for the console window: writes the command it would run to `opened.txt` in its folder, and the
/// environment and PATH it was given to `env.txt` and `path.txt`. Its probe says "running" until the test creates
/// `exited.txt` there. A new console is running, so it removes a leftover `exited.txt`. It fails with "boom" while
/// `fail.txt` is there.
fn record_console(
    argv: &[String],
    env: &[(String, String)],
    path: Option<&OsStr>,
    cwd: &Path,
) -> std::io::Result<ConsoleProbe> {
    if cwd.join("fail.txt").exists() {
        return Err(std::io::Error::other("boom"));
    }
    std::fs::write(cwd.join("opened.txt"), argv.join("\n"))?;
    let lines: Vec<String> = env.iter().map(|(name, value)| format!("{name}={value}")).collect();
    std::fs::write(cwd.join("env.txt"), lines.join("\n"))?;
    std::fs::write(
        cwd.join("path.txt"),
        path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
    )?;
    let exited = cwd.join("exited.txt");
    let _ = std::fs::remove_file(&exited);
    Ok(Box::new(move || !exited.exists()))
}

/// Like `record_console`, but slow, so a second request arrives while the first is still starting.
fn slow_console(
    argv: &[String],
    env: &[(String, String)],
    path: Option<&OsStr>,
    cwd: &Path,
) -> std::io::Result<ConsoleProbe> {
    std::thread::sleep(std::time::Duration::from_millis(400));
    record_console(argv, env, path, cwd)
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

/// Posts Claude's `name` hook event (a 2.1.302 fixture) from `tab`.
async fn hook(app: &axum::Router, tab: &str, name: &str) {
    let file = format!(
        "{}/tests/fixtures/claude/2.1.302/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut req = api("POST", "/v1/hook", None);
    *req.body_mut() = Body::from(std::fs::read(file).unwrap());
    let headers = req.headers_mut();
    headers.insert("x-marshell-role", "event".parse().unwrap());
    headers.insert("x-marshell-agent", "claude".parse().unwrap());
    headers.insert("x-marshell-tab", tab.parse().unwrap());
    headers.insert("x-marshell-version", "0.0.0".parse().unwrap());
    let (status, text) = send(app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{name}: {text}");
}

/// Once Claude's SessionEnd arrives, the tab's shell living on doesn't hold the conversation: it opens outside.
#[tokio::test]
async fn open_outside_opens_a_tab_whose_agent_has_ended() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path())
        .with_claude_version(Some("2.1.302".into()))
        .with_claude_compact_paths(home.path().join("user"), Vec::new());
    let app = router(st.clone());

    let tab = start(&app, Some("claude"), work.path(), json!({})).await;
    hook(&app, &tab, "SessionStart").await;
    let (status, body) = post(&app, &format!("/v1/sessions/{tab}/open-outside")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(reason(&body), "End the session first.");
    assert!(!work.path().join("opened.txt").exists());

    hook(&app, &tab, "SessionEnd").await;
    assert!(st.session(&tab).is_some_and(|s| !s.has_exited()), "the shell lives on");
    let (status, body) = post(&app, &format!("/v1/sessions/{tab}/open-outside")).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let opened = std::fs::read_to_string(work.path().join("opened.txt")).unwrap();
    assert!(opened.contains("claude"), "{opened}");
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

/// Ends the tab's process and waits until its row is Exited.
async fn end_tab(st: &AppState, tab: &str) {
    st.session(tab).expect("the tab runs").force_kill();
    for _ in 0..200 {
        if st.registry().row(tab).is_some_and(|row| !row.running()) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("the tab never ended");
}

/// The console has gone: the probe of the console opened in `work` answers false from now on.
fn close_console(work: &Path) {
    std::fs::write(work.join("exited.txt"), "").unwrap();
}

/// True once a console was started in `work` since the last `forget_opened`.
fn opened(work: &Path) -> bool {
    work.join("opened.txt").exists()
}

fn forget_opened(work: &Path) {
    let _ = std::fs::remove_file(work.join("opened.txt"));
}

fn resume_body() -> Value {
    json!({"cols": 80, "rows": 24})
}

async fn post_json(app: &axum::Router, path: &str, body: Value) -> (StatusCode, Value) {
    let (status, text) = send(app, api("POST", path, Some(body))).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

/// A tab opened outside leaves its console to hold the conversation: opening again, a tab Resume, a Restart and a
/// History Resume or open are refused until the console is gone. Copying the command is not.
#[tokio::test]
async fn a_conversation_open_in_a_console_is_not_resumed_or_opened_again() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    let tab = start(&app, Some("claude"), work.path(), json!({})).await;
    note(
        &st,
        "claude",
        &tab,
        "sess-1",
        &work.path().join("sess-1.jsonl"),
        work.path(),
        true,
    );
    end_tab(&st, &tab).await;

    let open = format!("/v1/sessions/{tab}/open-outside");
    let (status, body) = post(&app, &open).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(opened(work.path()));
    forget_opened(work.path());

    // Every way to start this conversation again is refused while the console runs.
    let (status, body) = post(&app, &open).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(reason(&body), OPEN_IN_TERMINAL);
    let (status, body) = post(&app, "/v1/history/claude/sess-1/open-outside").await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    let (status, body) = post_json(&app, &format!("/v1/sessions/{tab}/resume"), resume_body()).await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    let restart = json!({"mode": "plan", "cols": 80, "rows": 24});
    let (status, body) = post_json(&app, &format!("/v1/sessions/{tab}/restart"), restart.clone()).await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    let (status, body) = post_json(&app, "/v1/history/claude/sess-1/resume", resume_body()).await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    assert!(!opened(work.path()), "no second console");
    assert!(
        st.session(&tab).is_some_and(|s| s.has_exited()),
        "the tab was not started again"
    );

    // Copying the command is unaffected, and another conversation is free.
    let (status, body) = get(&app, &format!("/v1/sessions/{tab}/resume-command")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = get(&app, "/v1/history/claude/sess-1/resume-command").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    note(
        &st,
        "claude",
        "ghost",
        "sess-2",
        &work.path().join("sess-2.jsonl"),
        work.path(),
        true,
    );
    let (status, body) = post_json(&app, "/v1/history/claude/sess-2/resume", resume_body()).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // The console goes: the conversation can be opened (and resumed) again.
    close_console(work.path());
    let (status, body) = post(&app, &open).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(opened(work.path()), "the new console opened");
    // ... and that one holds it in turn.
    let (status, body) = post(&app, "/v1/history/claude/sess-1/open-outside").await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    close_console(work.path());
    let (status, body) = post_json(&app, &format!("/v1/sessions/{tab}/resume"), resume_body()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        st.session(&tab).is_some_and(|s| !s.has_exited()),
        "Resume started the tab"
    );
}

#[tokio::test]
async fn a_conversation_opened_from_history_is_held_by_its_console() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    // A conversation whose tab Marshell no longer has.
    note(
        &st,
        "claude",
        "ghost",
        "hist-9",
        &work.path().join("hist-9.jsonl"),
        work.path(),
        true,
    );
    let (status, body) = post(&app, "/v1/history/claude/hist-9/open-outside").await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    forget_opened(work.path());

    let (status, body) = post(&app, "/v1/history/claude/hist-9/open-outside").await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    let (status, body) = post_json(&app, "/v1/history/claude/hist-9/resume", resume_body()).await;
    assert_eq!((status, reason(&body)), (StatusCode::CONFLICT, OPEN_IN_TERMINAL));
    assert!(!opened(work.path()));

    close_console(work.path());
    let (status, body) = post(&app, "/v1/history/claude/hist-9/open-outside").await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(opened(work.path()));
}

/// Without an id the console is held by agent and folder: another tab of that agent in the folder is refused, one
/// in another folder is not.
#[tokio::test]
async fn continue_last_is_held_per_agent_and_folder() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    let first = start(&app, Some("copilot"), work.path(), json!({})).await;
    let second = start(&app, Some("copilot"), work.path(), json!({})).await;
    let other = start(&app, Some("copilot"), elsewhere.path(), json!({})).await;
    for tab in [&first, &second, &other] {
        end_tab(&st, tab).await;
    }
    let (status, body) = post(&app, &format!("/v1/sessions/{first}/open-outside")).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    for tab in [&first, &second] {
        let (status, body) = post(&app, &format!("/v1/sessions/{tab}/open-outside")).await;
        assert_eq!(
            (status, reason(&body)),
            (StatusCode::CONFLICT, OPEN_IN_TERMINAL),
            "{tab}"
        );
        let (status, body) = post_json(&app, &format!("/v1/sessions/{tab}/resume"), resume_body()).await;
        assert_eq!(
            (status, reason(&body)),
            (StatusCode::CONFLICT, OPEN_IN_TERMINAL),
            "{tab}"
        );
    }
    let (status, body) = post(&app, &format!("/v1/sessions/{other}/open-outside")).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    close_console(work.path());
    let (status, body) = post(&app, &format!("/v1/sessions/{second}/open-outside")).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

/// A second open that arrives while the first console is still starting is refused, not run beside it.
#[tokio::test]
async fn a_second_open_while_one_is_starting_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path()).with_console(slow_console);
    let app = router(st.clone());
    note(
        &st,
        "claude",
        "ghost",
        "hist-7",
        &work.path().join("hist-7.jsonl"),
        work.path(),
        true,
    );

    let path = "/v1/history/claude/hist-7/open-outside";
    let (a, b) = tokio::join!(post(&app, path), post(&app, path));
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::NO_CONTENT, StatusCode::CONFLICT], "{a:?} {b:?}");
    let refused = if a.0 == StatusCode::CONFLICT { &a.1 } else { &b.1 };
    assert_eq!(reason(refused), OPEN_IN_TERMINAL);
}

/// A console that failed to start doesn't hold the conversation, and the error reaches the page as the OS said it.
#[tokio::test]
async fn a_failed_open_gives_the_conversation_back() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());
    note(
        &st,
        "claude",
        "ghost",
        "hist-8",
        &work.path().join("hist-8.jsonl"),
        work.path(),
        true,
    );

    std::fs::write(work.path().join("fail.txt"), "").unwrap();
    let path = "/v1/history/claude/hist-8/open-outside";
    let (status, text) = send(&app, api("POST", path, None)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    // The page puts "Couldn't open a system terminal: " in front itself.
    assert_eq!(text, "boom");

    std::fs::remove_file(work.path().join("fail.txt")).unwrap();
    let (status, body) = post(&app, path).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

/// The console gets the environment a session gets (the app's variables among them), the PATH its program is looked
/// up on is the tab PATH, and none of Marshell's markers are in it.
#[tokio::test]
async fn the_console_gets_the_apps_environment_and_the_tab_path() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());
    note(
        &st,
        "claude",
        "ghost",
        "hist-6",
        &work.path().join("hist-6.jsonl"),
        work.path(),
        true,
    );

    let (status, body) = post(&app, "/v1/history/claude/hist-6/open-outside").await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let env = std::fs::read_to_string(work.path().join("env.txt")).unwrap();
    // Cargo sets this in the test process only: it comes from the app's environment, not a profile.
    assert!(env.contains("CARGO_MANIFEST_DIR="), "{env}");
    for marker in ["MARSHELL_TAB_ID", "MARSHELL_SESSION_NUMBER", "MARSHELL_AGENT"] {
        assert!(!env.to_uppercase().contains(marker), "{marker} in the environment");
    }
    let bin = marshell_core::paths::bin_dir(home.path());
    let path_line = env
        .lines()
        .find(|l| l.to_uppercase().starts_with("PATH="))
        .unwrap_or_default();
    assert!(!path_line.contains(&*bin.to_string_lossy()), "{path_line}");
    assert_eq!(std::fs::read_to_string(work.path().join("path.txt")).unwrap(), TAB_PATH);
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
