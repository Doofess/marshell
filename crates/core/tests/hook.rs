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
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap().keep();
    let user_home = home.join("user");
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        vec![sleeper_profile()],
    )
    .with_agent_check(|_| true)
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
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
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
    let st = state(Some(KNOWN)).with_permission_hold(std::time::Duration::from_secs(30));
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
        }),
    );
    assert_eq!(st.registry().effective_agent(&tab).as_deref(), Some("grok"));
    assert_dropped(&st, &app, &tab).await;
    stop(&st, &tab);
}
