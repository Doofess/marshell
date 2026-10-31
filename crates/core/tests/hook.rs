mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{LiveState, LiveStatus, SessionInfo};
use marshell_protocol::hook::HOOK_BODY_LIMIT;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;

fn sleeper_profile() -> marshell_protocol::api::Profile {
    marshell_protocol::api::Profile {
        id: "sleeper".into(),
        name: "Sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: marshell_protocol::api::ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

/// A Claude Code version Marshell has recorded; pinned so no test runs a real `claude`.
const KNOWN: &str = "2.1.302";

fn state(version: Option<&str>) -> AppState {
    state_with(version, vec![sleeper_profile()])
}

fn state_with(version: Option<&str>, profiles: Vec<marshell_protocol::api::Profile>) -> AppState {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap().keep();
    let user_home = home.join("user");
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        profiles,
    )
    .with_agent_check(|_| true)
    // No test here finds a program on this machine's PATH: CI has no Grok.
    .with_tab_path(|| None)
    .with_claude_version(version.map(str::to_string))
    .with_claude_compact_paths(user_home, Vec::new())
}

fn app() -> axum::Router {
    router(state(Some(KNOWN)))
}

fn base(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

fn hook(role: Option<&str>, agent: Option<&str>, tab: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut b = base("POST", "/v1/hook");
    if let Some(r) = role {
        b = b.header("x-marshell-role", r);
    }
    if let Some(a) = agent {
        b = b.header("x-marshell-agent", a);
    }
    if let Some(t) = tab {
        b = b.header("x-marshell-tab", t);
    }
    b.header("x-marshell-version", "0.0.0").body(Body::from(body)).unwrap()
}

async fn status(app: &axum::Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

async fn events(app: &axum::Router, tab: Option<&str>) -> Vec<serde_json::Value> {
    let path = match tab {
        Some(t) => format!("/v1/hook/events?tab={t}"),
        None => "/v1/hook/events".to_string(),
    };
    let res = app
        .clone()
        .oneshot(base("GET", &path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn hook_event_is_kept_under_its_live_tab() {
    let app = app();
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper" });
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let tab = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = hook(
        Some("event"),
        Some("claude"),
        Some(&tab),
        br#"{"hook_event_name":"Stop"}"#.to_vec(),
    );
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    let got = events(&app, Some(&tab)).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["role"], "event");
    assert_eq!(got[0]["agent"], "claude");
    assert_eq!(got[0]["body"]["hook_event_name"], "Stop");
    assert!(events(&app, None).await.is_empty());
}

#[tokio::test]
async fn hook_event_for_unknown_tab_goes_untabbed() {
    let app = app();
    let req = hook(Some("event"), Some("claude"), Some("nope"), br#"{"a":1}"#.to_vec());
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    assert_eq!(events(&app, None).await.len(), 1);
    assert!(events(&app, Some("nope")).await.is_empty());
}

#[tokio::test]
async fn hook_log_keeps_the_last_200() {
    let app = app();
    for i in 1..=205 {
        let req = hook(
            Some("event"),
            Some("claude"),
            None,
            format!(r#"{{"n":{i}}}"#).into_bytes(),
        );
        assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    }
    let got = events(&app, None).await;
    assert_eq!(got.len(), 200);
    assert_eq!(got[0]["body"]["n"], 6);
    assert_eq!(got[199]["body"]["n"], 205);
}

#[tokio::test]
async fn hook_rejects_non_json_body_with_400() {
    let req = hook(Some("event"), Some("claude"), None, b"not json".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hook_rejects_unknown_role_with_400() {
    let req = hook(Some("bogus"), Some("claude"), None, b"{}".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
    let req = hook(None, Some("claude"), None, b"{}".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hook_requires_agent_header_with_400() {
    let req = hook(Some("event"), None, None, b"{}".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hook_body_over_1_mib_gets_413() {
    let mut body = br#"{"x":""#.to_vec();
    body.resize(HOOK_BODY_LIMIT + 1, b'a');
    let req = hook(Some("event"), Some("claude"), None, body);
    assert_eq!(status(&app(), req).await, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn hook_requires_token() {
    let req = Request::builder()
        .method("POST")
        .uri("/v1/hook")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header("x-marshell-role", "event")
        .header("x-marshell-agent", "claude")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(status(&app(), req).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn context_role_answers_204_with_no_body() {
    let req = hook(Some("context"), Some("claude"), None, b"{}".to_vec());
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let bytes = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn closing_a_session_clears_its_hook_events() {
    let app = app();
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper" });
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let tab = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    let req = hook(Some("event"), Some("claude"), Some(&tab), br#"{"a":1}"#.to_vec());
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    assert_eq!(events(&app, Some(&tab)).await.len(), 1);

    let close = base("DELETE", &format!("/v1/sessions/{tab}"))
        .body(Body::empty())
        .unwrap();
    assert!(status(&app, close).await.is_success());
    assert!(events(&app, Some(&tab)).await.is_empty());
}

/// A recorded hook body or status line of Claude Code 2.1.302.
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/claude/{KNOWN}/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Starts a Claude Code session (the sleeper profile, launched as agent "claude") and returns its tab.
async fn start_claude(app: &axum::Router) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper", "agent": "claude" });
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn post_fixture(app: &axum::Router, role: &str, tab: &str, name: &str) -> StatusCode {
    let mut body = fixture(name);
    if role == "statusline" {
        // Recorded project paths must not make a test read settings outside its scratch home.
        let mut json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        json["workspace"] = serde_json::json!({});
        json["cwd"] = serde_json::Value::Null;
        body = serde_json::to_vec(&json).unwrap();
    }
    status(app, hook(Some(role), Some("claude"), Some(tab), body)).await
}

fn row(st: &AppState, tab: &str) -> SessionInfo {
    st.registry()
        .snapshot()
        .sessions
        .into_iter()
        .find(|s| s.tab_id == tab)
        .unwrap()
}

/// Ends the sleeper so the test leaves no process behind.
fn stop(st: &AppState, tab: &str) {
    if let Some(session) = st.remove_session(tab) {
        session.force_kill();
    }
}

#[tokio::test]
async fn session_start_then_a_prompt_makes_the_row_work() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;

    assert_eq!(
        post_fixture(&app, "event", &tab, "SessionStart").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(row(&st, &tab).live.unwrap().state, LiveState::Idle);
    assert_eq!(
        post_fixture(&app, "event", &tab, "UserPromptSubmit").await,
        StatusCode::NO_CONTENT
    );
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.live.unwrap().state, LiveState::Working);
    assert_eq!(info.status_note, None);
}

#[tokio::test]
async fn stop_gives_done_with_a_recap() {
    let st = state(Some(KNOWN)).with_done_settle(std::time::Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start_claude(&app).await;

    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    post_fixture(&app, "event", &tab, "Stop").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let live = row(&st, &tab).live.unwrap();
    stop(&st, &tab);
    assert_eq!(live.state, LiveState::Done);
    assert!(live.recap.is_some_and(|r| !r.text.is_empty()));
}

#[tokio::test]
async fn the_statusline_role_gives_the_context_percentage() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;

    assert_eq!(
        post_fixture(&app, "statusline", &tab, "statusline").await,
        StatusCode::NO_CONTENT
    );
    let live = row(&st, &tab).live.unwrap();
    stop(&st, &tab);
    assert_eq!(live.context_pct, Some(6));
    assert_eq!(live.model.as_deref(), Some("Opus 5.5"));
}

#[tokio::test]
async fn statusline_reads_the_model_and_project_compact_settings() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("user");
    let project = dir.path().join("project");
    let cwd = dir.path().join("cwd");
    for path in [&home, &project, &cwd] {
        std::fs::create_dir_all(path.join(".claude")).unwrap();
    }
    std::fs::write(home.join(".claude/settings.json"), r#"{"autoCompactWindow": 500000}"#).unwrap();
    std::fs::write(
        project.join(".claude/settings.local.json"),
        r#"{
        "autoCompactWindow": 400000,
        "modelSettings": {"claude-opus-5-5": {"autoCompactWindow": 300000}}
    }"#,
    )
    .unwrap();
    std::fs::write(cwd.join(".claude/settings.json"), r#"{"autoCompactWindow": 200000}"#).unwrap();
    let override_path = dir.path().join("hook-settings.json");
    std::fs::write(&override_path, r#"{"autoCompactWindow": 100000}"#).unwrap();
    let st = state(Some(KNOWN))
        .with_claude_compact_paths(home, Vec::new())
        .with_claude_settings(override_path);
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    let mut body: serde_json::Value = serde_json::from_slice(&fixture("statusline")).unwrap();
    body["workspace"]["project_dir"] = serde_json::json!(project);
    body["cwd"] = serde_json::json!(cwd);
    let mut percentages = Vec::new();
    for source in ["project", "cwd", "user"] {
        if source == "cwd" {
            body["workspace"] = serde_json::json!({});
        } else if source == "user" {
            body["cwd"] = serde_json::Value::Null;
        }
        assert_eq!(
            status(
                &app,
                hook(
                    Some("statusline"),
                    Some("claude"),
                    Some(&tab),
                    body.to_string().into_bytes()
                )
            )
            .await,
            StatusCode::NO_CONTENT
        );
        percentages.push(row(&st, &tab).live.unwrap().context_pct);
    }
    stop(&st, &tab);
    assert_eq!(percentages, vec![Some(19), Some(29), Some(12)]);
}

async fn finish_background_subagent(app: &axum::Router, st: &AppState, tab: &str) {
    for body in [
        serde_json::json!({ "hook_event_name": "SubagentStart", "agent_id": "a" }),
        serde_json::json!({ "hook_event_name": "Stop", "background_tasks": [
            { "id": "a", "type": "subagent", "status": "running" }
        ] }),
    ] {
        assert_eq!(
            status(
                app,
                hook(
                    Some("event"),
                    Some("claude"),
                    Some(tab),
                    serde_json::to_vec(&body).unwrap()
                )
            )
            .await,
            StatusCode::NO_CONTENT
        );
    }
    let waiting = row(st, tab).live.unwrap();
    assert_eq!(waiting.state, LiveState::Working);
    assert_eq!(waiting.phrase, "Waiting for subagents");
    assert_eq!(waiting.subagents, 1);
    assert_eq!(
        status(
            app,
            hook(
                Some("event"),
                Some("claude"),
                Some(tab),
                br#"{"hook_event_name":"PreToolUse","agent_id":"a","tool_name":"Glob"}"#.to_vec()
            )
        )
        .await,
        StatusCode::NO_CONTENT
    );
    let waiting = row(st, tab).live.unwrap();
    assert_eq!(waiting.state, LiveState::Working);
    assert_eq!(waiting.phrase, "Waiting for subagents");
    assert_eq!(
        status(
            app,
            hook(
                Some("event"),
                Some("claude"),
                Some(tab),
                br#"{"hook_event_name":"SubagentStop","agent_id":"a"}"#.to_vec()
            )
        )
        .await,
        StatusCode::NO_CONTENT
    );
    let waiting = row(st, tab).live.unwrap();
    assert_eq!(waiting.state, LiveState::Working);
    assert_eq!(waiting.phrase, "Waiting for subagents");
    assert_eq!(waiting.subagents, 0);
}

#[tokio::test]
async fn the_wait_falls_back_to_done_when_claude_does_not_resume() {
    let st = state(Some(KNOWN)).with_subagent_wait(std::time::Duration::from_millis(200));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    finish_background_subagent(&app, &st, &tab).await;

    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let live = row(&st, &tab).live.unwrap();
    stop(&st, &tab);
    assert_eq!(live.state, LiveState::Done);
    assert_eq!(live.phrase, "Done");
}

#[tokio::test]
async fn a_resumed_turn_is_not_settled_by_the_wait() {
    let st = state(Some(KNOWN)).with_subagent_wait(std::time::Duration::from_millis(200));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    finish_background_subagent(&app, &st, &tab).await;
    assert_eq!(
        post_fixture(&app, "event", &tab, "UserPromptSubmit").await,
        StatusCode::NO_CONTENT
    );

    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let live = row(&st, &tab).live.unwrap();
    stop(&st, &tab);
    assert_eq!(live.state, LiveState::Working);
    assert_eq!(live.phrase, "Thinking");
}

/// The live state of a tab as `GET /v1/sessions` shows it.
async fn live_over_http(app: &axum::Router, tab: &str) -> Option<serde_json::Value> {
    let res = app
        .clone()
        .oneshot(base("GET", "/v1/sessions").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    snapshot["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab)
        .map(|s| s["live"].clone())
}

/// The settle time in the tests that watch a held Done: long enough that a slow CI runner does not pass it
/// between the Stop and the next read.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(1);

/// The row's live status once it reads Done, or as it is after five seconds.
async fn settled(st: &AppState, tab: &str) -> LiveStatus {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let live = row(st, tab).live.unwrap();
        if live.state == LiveState::Done || std::time::Instant::now() > deadline {
            return live;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn stop_then_resume_never_shows_done() {
    use std::time::Duration;
    let st = state(Some(KNOWN)).with_done_settle(SETTLE);
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;

    // Poll every 20 ms across the Stop and the resume, as the app's own subscriber would see the rows.
    let poller = {
        let (app, tab) = (app.clone(), tab.clone());
        tokio::spawn(async move {
            let mut seen = Vec::new();
            for _ in 0..30 {
                let live = live_over_http(&app, &tab).await.unwrap();
                seen.push(live["state"].as_str().unwrap().to_string());
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            seen
        })
    };
    post_fixture(&app, "event", &tab, "Stop").await;
    tokio::time::sleep(Duration::from_millis(78)).await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let seen = poller.await.unwrap();
    let last = row(&st, &tab).live.unwrap();
    stop(&st, &tab);
    assert!(seen.iter().all(|s| s != "done"), "{seen:?}");
    assert_eq!((last.state, last.phrase.as_str()), (LiveState::Working, "Thinking"));
}

#[tokio::test]
async fn stop_alone_settles_to_done() {
    let st = state(Some(KNOWN)).with_done_settle(SETTLE);
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;

    let sent = std::time::Instant::now();
    post_fixture(&app, "event", &tab, "Stop").await;
    let early = row(&st, &tab);
    let early_at = sent.elapsed();
    let late = settled(&st, &tab).await;
    stop(&st, &tab);
    let early_live = early.live.unwrap();
    assert_eq!(
        early_live.state,
        LiveState::Working,
        "read {early_at:?} after the Stop, row {:?}",
        early.state
    );
    assert_eq!(late.state, LiveState::Done);
    assert_eq!(late.phrase, "Done");
    assert!(late.recap.is_some_and(|r| r.text.starts_with("I ran")));
}

#[tokio::test]
async fn a_status_line_update_does_not_cancel_the_held_done() {
    let st = state(Some(KNOWN)).with_done_settle(SETTLE);
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;

    let sent = std::time::Instant::now();
    post_fixture(&app, "event", &tab, "Stop").await;
    assert_eq!(
        post_fixture(&app, "statusline", &tab, "statusline").await,
        StatusCode::NO_CONTENT
    );
    let held = row(&st, &tab).live.unwrap();
    let held_at = sent.elapsed();
    let late = settled(&st, &tab).await;
    stop(&st, &tab);
    assert_eq!(held.state, LiveState::Working, "read {held_at:?} after the Stop");
    assert!(held.context_pct.is_some());
    assert_eq!(late.state, LiveState::Done);
    assert!(late.recap.is_some());
    assert_eq!(late.context_pct, held.context_pct);
}

#[tokio::test]
async fn a_held_permission_request_shows_needs_permission() {
    let st = state(Some(KNOWN)).with_permission_hold(std::time::Duration::from_millis(300));
    let app = router(st.clone());
    let tab = start_claude(&app).await;

    // The hold runs out with no decision: 204, and the agent asks itself.
    assert_eq!(
        post_fixture(&app, "permission", &tab, "PermissionRequest").await,
        StatusCode::NO_CONTENT
    );
    let live = row(&st, &tab).live.unwrap();
    stop(&st, &tab);
    assert_eq!(live.state, LiveState::NeedsPermission);
}

#[tokio::test]
async fn an_unknown_version_leaves_no_status_and_says_why() {
    for (version, note) in [
        (Some("2.2.0"), "Claude Code 2.2.0 is not one Marshell has recorded yet"),
        (None, "Claude Code's version could not be read"),
    ] {
        let st = state(version);
        let app = router(st.clone());
        let tab = start_claude(&app).await;

        post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
        post_fixture(&app, "statusline", &tab, "statusline").await;
        let info = row(&st, &tab);
        stop(&st, &tab);
        assert_eq!(info.live, None, "{version:?}");
        assert_eq!(info.status_note.as_deref(), Some(note));
    }
}

#[tokio::test]
async fn the_switch_off_for_claude_leaves_no_status_and_says_why() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    let settings = serde_json::json!({ "appearance": {}, "status_off": ["claude"] });
    let res = app
        .clone()
        .oneshot(
            base("PUT", "/v1/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(settings.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    post_fixture(&app, "statusline", &tab, "statusline").await;
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.live, None);
    assert_eq!(info.status_note.as_deref(), Some("Live status is off for Claude Code"));
}

fn pre_tool_use_in(mode: &str) -> Vec<u8> {
    let mut body: serde_json::Value = serde_json::from_slice(&fixture("PreToolUse")).unwrap();
    body["permission_mode"] = mode.into();
    serde_json::to_vec(&body).unwrap()
}

#[tokio::test]
async fn a_hook_body_with_a_permission_mode_changes_the_row_mode() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    assert_eq!(
        status(
            &app,
            hook(Some("event"), Some("claude"), Some(&tab), pre_tool_use_in("plan"))
        )
        .await,
        StatusCode::NO_CONTENT
    );
    let planned = row(&st, &tab).mode;
    // An unknown value changes nothing.
    status(
        &app,
        hook(Some("event"), Some("claude"), Some(&tab), pre_tool_use_in("later")),
    )
    .await;
    let unknown = row(&st, &tab).mode;
    stop(&st, &tab);
    assert_eq!(planned, Some(marshell_protocol::api::SessionMode::Plan));
    assert_eq!(unknown, planned);
}

#[tokio::test]
async fn subagent_hooks_count_the_running_subagents_on_the_row() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    let body = |event: &str, id: &str| {
        serde_json::to_vec(&serde_json::json!({ "hook_event_name": event, "agent_id": id })).unwrap()
    };
    for (event, id) in [("SubagentStart", "a"), ("SubagentStart", "b"), ("SubagentStop", "a")] {
        assert_eq!(
            status(&app, hook(Some("event"), Some("claude"), Some(&tab), body(event, id))).await,
            StatusCode::NO_CONTENT
        );
    }
    let count = row(&st, &tab).live.map(|l| l.subagents);
    stop(&st, &tab);
    assert_eq!(count, Some(1));
}

#[tokio::test]
async fn the_switch_off_for_claude_keeps_the_launch_mode() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    let launched = row(&st, &tab).mode;
    let settings = serde_json::json!({ "appearance": {}, "status_off": ["claude"] });
    app.clone()
        .oneshot(
            base("PUT", "/v1/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(settings.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    status(
        &app,
        hook(Some("event"), Some("claude"), Some(&tab), pre_tool_use_in("plan")),
    )
    .await;
    let after = row(&st, &tab).mode;
    stop(&st, &tab);
    assert_eq!(after, launched);
    assert_ne!(after, Some(marshell_protocol::api::SessionMode::Plan));
}

#[tokio::test]
async fn a_known_version_takes_the_note_away() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    st.registry().set_status_note(&tab, Some("old note".into()));

    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.status_note, None);
    assert!(info.live.is_some());
}

#[tokio::test]
async fn a_post_for_an_unknown_tab_changes_nothing() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    let before = st.registry().snapshot();

    assert_eq!(
        post_fixture(&app, "event", "nope", "UserPromptSubmit").await,
        StatusCode::NO_CONTENT
    );
    let untabbed = hook(Some("event"), Some("claude"), None, fixture("UserPromptSubmit"));
    assert_eq!(status(&app, untabbed).await, StatusCode::NO_CONTENT);
    let after = st.registry().snapshot();
    stop(&st, &tab);
    assert_eq!(before, after);
}

#[tokio::test]
async fn another_agents_events_do_not_touch_the_status() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;

    let req = hook(
        Some("event"),
        Some("antigravity"),
        Some(&tab),
        fixture("UserPromptSubmit"),
    );
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!((info.live, info.status_note), (None, None));
}

/// A body the history refuses (a bad session id, a transcript on a share) is still a Stop for the live status:
/// the status is folded before the history looks at the body, and the refusal writes nothing.
#[tokio::test]
async fn hostile_history_body_still_updates_live_status() {
    let st = state(Some(KNOWN)).with_done_settle(std::time::Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    // No earlier valid event: that one would put a (legitimate) row in the history.
    let mut body: serde_json::Value = serde_json::from_slice(&fixture("Stop")).unwrap();
    body["session_id"] = "../../x".into();
    body["transcript_path"] = r"\\host\share\x.jsonl".into();
    let req = hook(
        Some("event"),
        Some("claude"),
        Some(&tab),
        serde_json::to_vec(&body).unwrap(),
    );
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let live = row(&st, &tab).live.unwrap();
    let remembered = st.history().latest_for_tab(&tab);
    stop(&st, &tab);
    assert_eq!(live.state, LiveState::Done);
    assert!(live.recap.is_some_and(|r| !r.text.is_empty()));
    assert_eq!(remembered, None);
    let archive = st.home().join("history").join("archive");
    let written = std::fs::read_dir(&archive).map_or(0, |dir| dir.count());
    assert_eq!(written, 0, "something was written under {}", archive.display());
}

/// Starts a session of the sleeper profile launched as `agent` (None: a plain shell row) and returns its tab.
async fn start_as(app: &axum::Router, agent: Option<&str>) -> String {
    let mut body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper" });
    if let Some(a) = agent {
        body["agent"] = a.into();
    }
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn assert_dropped(st: &AppState, app: &axum::Router, tab: &str) {
    let started = std::time::Instant::now();
    let code = post_fixture(app, "permission", tab, "PermissionRequest").await;
    assert_eq!(code, StatusCode::NO_CONTENT);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "a hold was opened: {:?}",
        started.elapsed()
    );
    assert!(st.holds().pending().is_empty());
    assert_eq!(
        post_fixture(app, "event", tab, "UserPromptSubmit").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(post_fixture(app, "event", tab, "Stop").await, StatusCode::NO_CONTENT);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(row(st, tab).live.is_none());
    assert_eq!(st.history().latest_for_tab(tab), None);
}

#[tokio::test]
async fn a_claude_event_from_a_grok_tab_is_dropped_before_any_hold() {
    // A stand-in Grok, found and named without running anything. The version cache reads its modified time, so it exists.
    fn grok_stand_in() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("hook-grok-marshell-test.exe")
    }
    std::fs::write(grok_stand_in(), "").unwrap();
    let st = state(Some(KNOWN))
        .with_permission_hold(std::time::Duration::from_secs(30))
        .with_agent_path(|command| (command == "grok").then(grok_stand_in))
        .with_agent_versions(marshell_core::agents::identify::VersionCache::with_reader(|_| {
            Some("grok 1.2.3 (abc1234)\n".into())
        }));
    let app = router(st.clone());
    let tab = start_as(&app, Some("grok")).await;
    assert_dropped(&st, &app, &tab).await;
    stop(&st, &tab);
}

#[tokio::test]
async fn a_claude_event_from_a_claude_tab_still_folds() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let live = row(&st, &tab).live;
    stop(&st, &tab);
    assert_eq!(live.unwrap().state, LiveState::Working);
}

#[tokio::test]
async fn a_claude_event_from_a_shell_tab_running_grok_is_dropped() {
    let st = state(Some(KNOWN)).with_permission_hold(std::time::Duration::from_secs(30));
    let app = router(st.clone());
    let tab = start_as(&app, None).await;
    let generation = st.registry().generation(&tab).unwrap();
    st.registry().set_foreground(
        &tab,
        generation,
        Some(marshell_protocol::api::Foreground {
            program: "grok-marshell-test".into(),
            command_line: None,
            agent: Some("grok".into()),
            tool: None,
        }),
    );
    assert_eq!(st.registry().effective_agent(&tab).as_deref(), Some("grok"));
    assert_dropped(&st, &app, &tab).await;
    stop(&st, &tab);
}

/// What the foreground watcher records for a tab launched as Claude: `agent` runs in front of it and Claude is gone from
/// the foreground (`None`: Claude is still there).
fn displace(st: &AppState, tab: &str, agent: Option<&str>) {
    let generation = st.registry().generation(tab).unwrap();
    st.registry()
        .set_displacing_agent(tab, generation, agent.map(str::to_string));
}

#[tokio::test]
async fn a_claude_event_from_a_claude_tab_where_grok_took_over_is_dropped() {
    let st = state(Some(KNOWN)).with_permission_hold(std::time::Duration::from_secs(30));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    // Claude exited in its shell and the person typed `grok`, which runs Claude's own hooks.
    displace(&st, &tab, Some("grok"));
    assert_eq!(st.registry().effective_agent(&tab).as_deref(), Some("grok"));
    assert_dropped(&st, &app, &tab).await;
    stop(&st, &tab);
}

#[tokio::test]
async fn claude_keeps_its_hooks_while_it_is_in_the_foreground_and_before_any_sighting() {
    let st = state(Some(KNOWN)).with_permission_hold(std::time::Duration::from_millis(300));
    let app = router(st.clone());
    let tab = start_claude(&app).await;
    // No foreground sample yet: the agent the tab was launched as.
    assert_eq!(st.registry().effective_agent(&tab).as_deref(), Some("claude"));
    // Sampled, with Claude still in the chain (its Bash tool runs another agent): still Claude.
    displace(&st, &tab, None);
    assert_eq!(st.registry().effective_agent(&tab).as_deref(), Some("claude"));
    assert_eq!(
        post_fixture(&app, "permission", &tab, "PermissionRequest").await,
        StatusCode::NO_CONTENT
    );
    let live = row(&st, &tab).live.unwrap();
    // Grok takes over, then quits and Claude is typed again: Claude's hooks count again.
    displace(&st, &tab, Some("grok"));
    displace(&st, &tab, None);
    assert_eq!(st.registry().effective_agent(&tab).as_deref(), Some("claude"));
    stop(&st, &tab);
    assert_eq!(live.state, LiveState::NeedsPermission);
}

// ---- Agents inside WSL: a hook names the agent the tab runs ----

/// A WSL profile that starts the sleeper: only `-d Ubuntu` in its argv makes it a tab of that distro, as `wsl.exe -d Ubuntu`
/// would be. No test starts a real `wsl.exe`.
fn wsl_profile() -> marshell_protocol::api::Profile {
    let mut command = sleeper_profile().command;
    command.extend(["-d".to_string(), "Ubuntu".to_string()]);
    marshell_protocol::api::Profile {
        id: "wsl-ubuntu".into(),
        name: "Ubuntu".into(),
        command,
        family: marshell_protocol::api::ShellFamily::Wsl,
        ..sleeper_profile()
    }
}

/// A plain shell row (launched as no agent) of `profile_id`.
async fn start_shell(app: &axum::Router, profile_id: &str) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": profile_id });
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let code = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(code, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The state with a WSL tab; its distro's version is pinned, so no test asks a real `wsl.exe`.
fn wsl_state() -> AppState {
    state_with(Some(KNOWN), vec![sleeper_profile(), wsl_profile()]).with_wsl_version(Some(KNOWN.into()))
}

fn foreground_agent(st: &AppState, tab: &str) -> Option<String> {
    row(st, tab).foreground.and_then(|f| f.agent)
}

#[tokio::test]
async fn a_wsl_tab_takes_the_agent_from_its_hooks() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    assert_eq!(st.wsl_distro_of_tab(&tab).as_deref(), Some("Ubuntu"));
    assert_eq!(foreground_agent(&st, &tab), None);
    assert_eq!(
        post_fixture(&app, "event", &tab, "UserPromptSubmit").await,
        StatusCode::NO_CONTENT
    );
    let info = row(&st, &tab);
    stop(&st, &tab);
    let foreground = info.foreground.unwrap();
    assert_eq!(
        (foreground.program.as_str(), foreground.agent.as_deref()),
        ("claude", Some("claude"))
    );
    assert_eq!(info.live.unwrap().state, LiveState::Working);
}

#[tokio::test]
async fn a_wsl_tabs_history_row_reads_its_linux_paths_under_the_distro() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    let id = "00000000-0000-4000-8000-000000000001";
    let mut body: serde_json::Value = serde_json::from_slice(&fixture("SessionStart")).unwrap();
    body["session_id"] = serde_json::json!(id);
    body["transcript_path"] = serde_json::json!(format!("/home/u/.claude/projects/-home-u-proj/{id}.jsonl"));
    body["cwd"] = serde_json::json!("/home/u/proj");
    let sent = status(
        &app,
        hook(
            Some("event"),
            Some("claude"),
            Some(&tab),
            serde_json::to_vec(&body).unwrap(),
        ),
    )
    .await;
    let row = st.history().latest_for_tab(&tab);
    let stored = marshell_core::history::archive::read_meta(st.home(), "claude", id).unwrap();
    stop(&st, &tab);

    assert_eq!(sent, StatusCode::NO_CONTENT);
    let row = row.expect("the WSL tab's session is in the history");
    assert_eq!(
        row.transcript_path,
        format!(r"\\wsl.localhost\Ubuntu\home\u\.claude\projects\-home-u-proj\{id}.jsonl")
    );
    assert_eq!(row.cwd, r"\\wsl.localhost\Ubuntu\home\u\proj");
    assert_eq!(stored.unwrap().distro.as_deref(), Some("Ubuntu"));
}

#[tokio::test]
async fn a_wsl_tab_gates_on_the_distros_version() {
    // The Windows claude is too old to be recorded; the distro's is recorded, so live is folded.
    let st = state_with(Some("1.0.0"), vec![sleeper_profile(), wsl_profile()]).with_wsl_version(Some(KNOWN.into()));
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.live.unwrap().state, LiveState::Working);
    assert_eq!(info.status_note, None);
}

#[tokio::test]
async fn a_wsl_tab_without_a_version_gets_the_wsl_note() {
    // The Windows claude is recorded; the distro's version can't be read, and that is what counts.
    let st = state_with(Some(KNOWN), vec![sleeper_profile(), wsl_profile()]).with_wsl_version(None);
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.live, None);
    assert_eq!(
        info.status_note.as_deref(),
        Some("Can't read Claude Code's version in Ubuntu")
    );
}

#[tokio::test]
async fn a_status_line_names_the_agent_too() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    assert_eq!(
        post_fixture(&app, "statusline", &tab, "statusline").await,
        StatusCode::NO_CONTENT
    );
    let (agent, live) = (foreground_agent(&st, &tab), row(&st, &tab).live);
    stop(&st, &tab);
    assert_eq!(agent.as_deref(), Some("claude"));
    assert!(live.is_some());
}

#[tokio::test]
async fn session_end_clears_the_wsl_agent() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    assert_eq!(foreground_agent(&st, &tab).as_deref(), Some("claude"));
    assert_eq!(
        post_fixture(&app, "event", &tab, "SessionEnd").await,
        StatusCode::NO_CONTENT
    );
    let info = row(&st, &tab);
    stop(&st, &tab);
    // The row is a plain shell again, with no status left behind.
    assert_eq!(info.foreground, None);
    assert_eq!(info.live, None);
}

#[tokio::test]
async fn a_lone_session_end_in_a_wsl_tab_leaves_no_status() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    post_fixture(&app, "event", &tab, "SessionEnd").await;
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.foreground, None);
    assert_eq!(info.live, None);
}

#[tokio::test]
async fn a_non_wsl_shell_tab_is_not_marked_by_hooks() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "sleeper").await;
    assert_eq!(st.wsl_distro_of_tab(&tab), None);
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.foreground, None);
}

#[tokio::test]
async fn another_agents_hook_in_a_marked_wsl_tab_is_ignored() {
    let st = wsl_state().with_codex_version(Some("0.163.0".into()));
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    let before = row(&st, &tab).live;
    let body = serde_json::json!({ "hook_event_name": "Stop" })
        .to_string()
        .into_bytes();
    assert_eq!(
        status(&app, hook(Some("event"), Some("codex"), Some(&tab), body)).await,
        StatusCode::NO_CONTENT
    );
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!(info.foreground.and_then(|f| f.agent).as_deref(), Some("claude"));
    assert_eq!(info.live, before);
}

#[tokio::test]
async fn a_late_event_after_the_prompt_mark_does_not_mark_the_row_again() {
    let st = wsl_state();
    let app = router(st.clone());
    let tab = start_shell(&app, "wsl-ubuntu").await;
    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    // The agent exited and the shell is back at its prompt.
    let away = marshell_protocol::api::UiView {
        window_focused: false,
        visible_tabs: Vec::new(),
    };
    st.registry().mark(&tab, marshell_core::pty::Mark::PromptStart, &away);
    assert_eq!(foreground_agent(&st, &tab), None);
    post_fixture(&app, "event", &tab, "PostToolUse").await;
    let agent = foreground_agent(&st, &tab);
    stop(&st, &tab);
    assert_eq!(agent, None);
}

// ---- Project references: a matched alias's note reaches Claude Code and Codex ----

/// A project with a `.git` folder and a file an alias can name.
struct Project {
    _dir: tempfile::TempDir,
    repo: std::path::PathBuf,
}

fn project() -> Project {
    let dir = tempfile::tempdir().unwrap();
    let repo = marshell_core::projects::canonical(dir.path()).join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src").join("login.rs"), "fn login() {}").unwrap();
    Project { _dir: dir, repo }
}

fn alias(name: &str, path: &str) -> marshell_protocol::api::Alias {
    marshell_protocol::api::Alias {
        name: name.into(),
        path: path.into(),
    }
}

fn save_refs(st: &AppState, root: &std::path::Path, enabled: bool, aliases: Vec<marshell_protocol::api::Alias>) {
    marshell_core::references::save(
        st.home(),
        &marshell_protocol::api::ReferencesUpdate {
            root: root.to_string_lossy().into_owned(),
            enabled,
            aliases,
        },
    )
    .unwrap();
}

/// A session of the sleeper profile launched as `agent`, in `cwd`.
async fn start_in(app: &axum::Router, agent: &str, cwd: &std::path::Path) -> String {
    start_profile_in(app, "sleeper", agent, cwd).await
}

/// A session of `profile_id` launched as `agent`, in `cwd`.
async fn start_profile_in(app: &axum::Router, profile_id: &str, agent: &str, cwd: &std::path::Path) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": profile_id, "agent": agent, "cwd": cwd.to_string_lossy(),
    });
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let code = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(code, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Posts a `UserPromptSubmit` for `agent` and returns the status and the body text.
async fn submit(app: &axum::Router, agent: &str, tab: Option<&str>, prompt: &str) -> (StatusCode, String) {
    let body = serde_json::json!({"hook_event_name": "UserPromptSubmit", "prompt": prompt});
    let res = app
        .clone()
        .oneshot(hook(
            Some("event"),
            Some(agent),
            tab,
            serde_json::to_vec(&body).unwrap(),
        ))
        .await
        .unwrap();
    let code = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (code, String::from_utf8_lossy(&bytes).into_owned())
}

fn login_button() -> Vec<marshell_protocol::api::Alias> {
    vec![alias("login button", "src/login.rs")]
}

#[tokio::test]
async fn a_prompt_naming_an_alias_gets_its_note() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "claude", &p.repo).await;

    let (code, text) = submit(&app, "claude", Some(&tab), "fix the Login Button please").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let answer: serde_json::Value = serde_json::from_str(&text).unwrap();
    let file = p.repo.join("src").join("login.rs");
    let note = format!("References: 'login button' = {}.", file.display());
    assert_eq!(
        answer,
        serde_json::json!({"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": note}})
    );
    let shown = shown.expect("the chip's note");
    assert_eq!(shown.names, ["login button"]);
    assert_eq!(shown.note, note);
}

#[tokio::test]
async fn codex_gets_the_same_note_as_json() {
    let st = state(Some(KNOWN)).with_codex_version(Some("0.163.0".into()));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "codex", &p.repo).await;

    let (code, text) = submit(&app, "codex", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let answer: serde_json::Value = serde_json::from_str(&text).unwrap();
    let note = format!(
        "References: 'login button' = {}.",
        p.repo.join("src").join("login.rs").display()
    );
    assert_eq!(
        answer,
        serde_json::json!({"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": note}})
    );
    assert_eq!(shown.unwrap().names, ["login button"]);
}

#[tokio::test]
async fn no_match_is_a_204_and_clears_the_chip() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "claude", &p.repo).await;

    assert_eq!(
        submit(&app, "claude", Some(&tab), "fix the login button").await.0,
        StatusCode::OK
    );
    assert!(row(&st, &tab).references.is_some());
    let (code, text) = submit(&app, "claude", Some(&tab), "fix the logout link").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::NO_CONTENT);
    assert!(text.is_empty(), "{text}");
    assert_eq!(shown, None);
}

/// Turns the project into a git repo with one commit and adds a linked worktree beside it, which it returns.
fn linked_worktree(p: &Project) -> std::path::PathBuf {
    // The `.git` folder `project` made is not a repo yet.
    std::fs::remove_dir_all(p.repo.join(".git")).unwrap();
    let run = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&p.repo)
            .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["add", "."]);
    run(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "first",
    ]);
    let tree = p.repo.parent().unwrap().join("tree");
    run(&["worktree", "add", "-q", "-b", "side", &tree.to_string_lossy()]);
    marshell_core::projects::canonical(&tree)
}

#[tokio::test]
async fn a_worktree_tab_gets_paths_in_its_own_checkout() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    let tree = linked_worktree(&p);

    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "claude", &tree).await;
    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let note = serde_json::from_str::<serde_json::Value>(&text).unwrap()["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    let own = tree.join("src").join("login.rs");
    assert_eq!(note, format!("References: 'login button' = {}.", own.display()));
    assert!(
        !note.contains(&p.repo.join("src").to_string_lossy().into_owned()),
        "{note}"
    );
}

#[tokio::test]
async fn a_worktree_tab_checks_the_file_in_its_own_checkout() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    let tree = linked_worktree(&p);
    // The file is in the main checkout and was in the worktree too; here it is gone from the worktree only.
    assert!(p.repo.join("src").join("login.rs").exists());
    std::fs::remove_file(tree.join("src").join("login.rs")).unwrap();

    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "claude", &tree).await;
    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::NO_CONTENT, "{text}");
    assert!(text.is_empty(), "{text}");
    assert_eq!(shown, None);
}

#[tokio::test]
async fn a_prompt_resolves_in_the_folder_the_shell_is_in() {
    // Started in repo A; the shell then moved into repo B, where the agent runs.
    let a = project();
    let b = project();
    let reporter = marshell_protocol::api::Profile {
        id: "reporter".into(),
        name: "Reporter".into(),
        command: vec![
            env!("CARGO_BIN_EXE_fake-agent").into(),
            "folder".into(),
            b.repo.to_string_lossy().into_owned(),
        ],
        ..sleeper_profile()
    };
    let st = state_with(Some(KNOWN), vec![sleeper_profile(), reporter]);
    let app = router(st.clone());
    // Only B has references, so a note can only come from B's project.
    save_refs(&st, &b.repo, true, login_button());
    let tab = start_profile_in(&app, "reporter", "claude", &a.repo).await;
    // The shell's report arrives some time after the process starts.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while st.session(&tab).and_then(|s| s.current_cwd()).is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the shell never reported a folder"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let note = serde_json::from_str::<serde_json::Value>(&text).unwrap()["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        note,
        format!(
            "References: 'login button' = {}.",
            b.repo.join("src").join("login.rs").display()
        )
    );
    assert_eq!(shown.unwrap().names, ["login button"]);
}

#[tokio::test]
async fn a_disabled_project_or_unreadable_file_is_a_204() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    let tab = start_in(&app, "claude", &p.repo).await;

    // No file at all: references are off.
    assert_eq!(
        submit(&app, "claude", Some(&tab), "fix the login button").await.0,
        StatusCode::NO_CONTENT
    );
    // Saved but switched off.
    save_refs(&st, &p.repo, false, login_button());
    assert_eq!(
        submit(&app, "claude", Some(&tab), "fix the login button").await.0,
        StatusCode::NO_CONTENT
    );
    // On, and then the file is broken: the prompt goes on with nothing added.
    save_refs(&st, &p.repo, true, login_button());
    assert_eq!(
        submit(&app, "claude", Some(&tab), "fix the login button").await.0,
        StatusCode::OK
    );
    let file = st
        .home()
        .join("projects")
        .join(marshell_core::references::project_id(&p.repo))
        .join(marshell_core::references::FILE_NAME);
    std::fs::write(&file, "{ this is not json").unwrap();
    // The `.bak` still holds a readable copy, which is the fallback and is meant: the prompt is still answered.
    assert!(
        marshell_core::references::load(st.home(), &p.repo).is_ok(),
        "the .bak is read when the main file is broken"
    );
    std::fs::write(marshell_core::store::bak_path(&file), "{ this is not json either").unwrap();
    // Neither file is readable now, so the unreadable path itself is the one taken.
    assert!(
        marshell_core::references::load(st.home(), &p.repo).is_err(),
        "both files must be refused for this test to reach the unreadable path"
    );
    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::NO_CONTENT, "{text}");
    assert!(text.is_empty(), "{text}");
    assert_eq!(shown, None);
}

/// A WSL profile whose shell reports `folder` as its folder, as a distro's shell does over OSC 7 (a Linux path).
fn wsl_reporter(folder: &str) -> marshell_protocol::api::Profile {
    marshell_protocol::api::Profile {
        id: "wsl-reporter".into(),
        name: "Ubuntu".into(),
        command: vec![
            env!("CARGO_BIN_EXE_fake-agent").into(),
            "folder".into(),
            folder.into(),
            "-d".into(),
            "Ubuntu".into(),
        ],
        family: marshell_protocol::api::ShellFamily::Wsl,
        ..sleeper_profile()
    }
}

/// A WSL tab whose shell reports `folder`, once it has.
async fn wsl_tab_reporting(st: &AppState, app: &axum::Router, folder: &str) -> String {
    let tab = start_shell(app, "wsl-reporter").await;
    assert_eq!(st.wsl_distro_of_tab(&tab).as_deref(), Some("Ubuntu"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while st.session(&tab).and_then(|s| s.current_cwd()).is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the shell never reported {folder}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    tab
}

fn wsl_state_reporting(folder: &str) -> AppState {
    state_with(Some(KNOWN), vec![sleeper_profile(), wsl_reporter(folder)]).with_wsl_version(Some(KNOWN.into()))
}

/// `C:\a\b` as `/mnt/c/a/b`, spelled by hand.
#[cfg(windows)]
fn mnt_form(windows: &std::path::Path) -> String {
    let text = windows.to_string_lossy().into_owned();
    let (drive, rest) = text.split_at(2);
    format!("/mnt/{}{}", drive[..1].to_lowercase(), rest.replace('\\', "/"))
}

#[cfg(windows)]
#[tokio::test]
async fn a_wsl_prompt_gets_linux_paths_in_its_note() {
    let p = project();
    let linux_repo = mnt_form(&p.repo);
    let st = wsl_state_reporting(&linux_repo);
    let app = router(st.clone());
    save_refs(&st, &p.repo, true, login_button());
    let tab = wsl_tab_reporting(&st, &app, &linux_repo).await;

    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let answer: serde_json::Value = serde_json::from_str(&text).unwrap();
    let note = format!("References: 'login button' = {linux_repo}/src/login.rs.");
    assert_eq!(
        answer,
        serde_json::json!({"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": note}})
    );
    assert_eq!(shown.expect("the chip's note").names, ["login button"]);
}

#[cfg(windows)]
#[tokio::test]
async fn a_wsl_tab_started_in_a_windows_folder_gets_linux_paths() {
    let p = project();
    let st = state_with(Some(KNOWN), vec![sleeper_profile(), wsl_profile()]).with_wsl_version(Some(KNOWN.into()));
    let app = router(st.clone());
    save_refs(&st, &p.repo, true, login_button());
    // The shell reports nothing, so the start folder (a Windows path) is the one used.
    let tab = start_profile_in(&app, "wsl-ubuntu", "claude", &p.repo).await;

    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let note = serde_json::from_str::<serde_json::Value>(&text).unwrap()["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        note,
        format!("References: 'login button' = {}/src/login.rs.", mnt_form(&p.repo))
    );
}

#[tokio::test]
async fn a_wsl_tab_whose_folder_cannot_be_translated_gets_no_note() {
    let p = project();
    // A relative folder names no place a Windows path could be made from.
    let st = wsl_state_reporting("proj/sub");
    let app = router(st.clone());
    save_refs(&st, &p.repo, true, login_button());
    let tab = wsl_tab_reporting(&st, &app, "proj/sub").await;

    let (code, text) = submit(&app, "claude", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::NO_CONTENT, "{text}");
    assert!(text.is_empty(), "{text}");
    assert_eq!(shown, None);
}

/// Every file under `dir`, read as lossy text.
fn texts_under(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            texts_under(&path, out);
        } else if let Ok(bytes) = std::fs::read(&path) {
            out.push((path, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
}

#[tokio::test]
async fn the_prompt_is_not_kept() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "claude", &p.repo).await;

    let secret = "PROMPTSECRET-4417";
    let (code, _) = submit(&app, "claude", Some(&tab), &format!("{secret} fix the login button")).await;
    assert_eq!(code, StatusCode::OK);
    // agy's text is dropped too: this event is for another agent's tab, but the log keeps what was posted.
    let agy = serde_json::json!({"hook_event_name": "PostToolUse", "lastUserInput": "AGYSECRET-9921", "email": "who@example.com"});
    status(
        &app,
        hook(
            Some("event"),
            Some("antigravity"),
            Some(&tab),
            serde_json::to_vec(&agy).unwrap(),
        ),
    )
    .await;

    let logged = events(&app, Some(&tab)).await;
    let sessions = serde_json::to_string(&st.registry().snapshot().sessions).unwrap();
    let mut files = Vec::new();
    texts_under(st.home(), &mut files);
    stop(&st, &tab);

    assert!(!logged.is_empty());
    let logged = serde_json::to_string(&logged).unwrap();
    for text in [secret, "AGYSECRET-9921", "who@example.com"] {
        assert!(!logged.contains(text), "the hook log has {text}: {logged}");
        assert!(!sessions.contains(text), "the session list has {text}");
        for (path, content) in &files {
            assert!(!content.contains(text), "{} has {text}", path.display());
        }
    }
    // The rest of the body stays in the log.
    assert!(logged.contains("UserPromptSubmit"));
}

#[tokio::test]
async fn a_tab_not_opened_by_marshell_gets_nothing() {
    let st = state(Some(KNOWN)).with_codex_version(Some("0.163.0".into()));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());

    // No tab header, and a tab the registry never opened: nothing is added.
    assert_eq!(
        submit(&app, "claude", None, "fix the login button").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        submit(&app, "claude", Some("unknown-tab"), "fix the login button")
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    // A Codex prompt from a tab that runs Claude is not this tab's: no note and no chip.
    let tab = start_in(&app, "claude", &p.repo).await;
    let (code, _) = submit(&app, "codex", Some(&tab), "fix the login button").await;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);
    assert_eq!(code, StatusCode::NO_CONTENT);
    assert_eq!(shown, None);
}

/// Posts an Antigravity `PreInvocation` of `prompt` as call `n` (the bridge adds `hook_event_name` from `--event`).
async fn pre_invocation(app: &axum::Router, tab: &str, prompt: &str, n: u64) -> (StatusCode, String) {
    let body = serde_json::json!({"hook_event_name": "PreInvocation", "invocationNum": n, "lastUserInput": prompt});
    post_agy(app, tab, &body).await
}

async fn post_agy(app: &axum::Router, tab: &str, body: &serde_json::Value) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(hook(
            Some("event"),
            Some("antigravity"),
            Some(tab),
            serde_json::to_vec(body).unwrap(),
        ))
        .await
        .unwrap();
    let code = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (code, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn antigravity_gets_the_note_once_per_prompt() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "antigravity", &p.repo).await;

    let (code, text) = pre_invocation(&app, &tab, "fix the login button", 0).await;
    let first = row(&st, &tab).references;
    // The next model call of the same prompt: the note is already in the model's context.
    let (again, again_text) = pre_invocation(&app, &tab, "fix the login button", 1).await;
    let kept = row(&st, &tab).references;
    // A new prompt that names nothing clears the chip.
    let (fresh, fresh_text) = pre_invocation(&app, &tab, "fix the logout link", 0).await;
    let cleared = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!(code, StatusCode::OK, "{text}");
    let note = format!(
        "References: 'login button' = {}.",
        p.repo.join("src").join("login.rs").display()
    );
    let answer: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(answer, serde_json::json!({"injectSteps": [{"ephemeralMessage": note}]}));
    let first = first.expect("the chip's note");
    assert_eq!(first.names, ["login button"]);
    assert_eq!(first.note, note);

    assert_eq!(again, StatusCode::NO_CONTENT, "{again_text}");
    assert!(again_text.is_empty());
    assert_eq!(kept, Some(first));

    assert_eq!(fresh, StatusCode::NO_CONTENT, "{fresh_text}");
    assert_eq!(cleared, None);
}

#[tokio::test]
async fn antigravity_ignores_a_call_without_a_first_invocation_number() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "antigravity", &p.repo).await;

    let missing = serde_json::json!({"hook_event_name": "PreInvocation", "lastUserInput": "fix the login button"});
    let as_text = serde_json::json!({
        "hook_event_name": "PreInvocation", "invocationNum": "0", "lastUserInput": "fix the login button",
    });
    let a = post_agy(&app, &tab, &missing).await.0;
    let b = post_agy(&app, &tab, &as_text).await.0;
    let shown = row(&st, &tab).references;
    stop(&st, &tab);

    assert_eq!((a, b), (StatusCode::NO_CONTENT, StatusCode::NO_CONTENT));
    assert_eq!(shown, None);
}

#[tokio::test]
async fn antigravity_input_is_still_stripped() {
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let p = project();
    save_refs(&st, &p.repo, true, login_button());
    let tab = start_in(&app, "antigravity", &p.repo).await;

    let secret = "AGYPROMPT-5530";
    let (code, _) = pre_invocation(&app, &tab, &format!("{secret} fix the login button"), 0).await;
    assert_eq!(code, StatusCode::OK);
    pre_invocation(&app, &tab, &format!("{secret} fix the login button"), 1).await;

    let logged = serde_json::to_string(&events(&app, Some(&tab)).await).unwrap();
    let sessions = serde_json::to_string(&st.registry().snapshot().sessions).unwrap();
    let mut files = Vec::new();
    texts_under(st.home(), &mut files);
    stop(&st, &tab);

    assert!(logged.contains("PreInvocation"), "{logged}");
    assert!(!logged.contains("lastUserInput"), "{logged}");
    assert!(!logged.contains(secret), "the hook log has the prompt: {logged}");
    assert!(!sessions.contains(secret), "the session list has the prompt");
    for (path, content) in &files {
        assert!(!content.contains(secret), "{} has the prompt", path.display());
    }
}
