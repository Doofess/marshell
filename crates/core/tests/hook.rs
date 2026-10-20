mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{LiveState, SessionInfo};
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
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        vec![sleeper_profile()],
    )
    .with_agent_check(|_| true)
    .with_claude_version(version.map(str::to_string))
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
    status(app, hook(Some(role), Some("claude"), Some(tab), fixture(name))).await
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
    let st = state(Some(KNOWN));
    let app = router(st.clone());
    let tab = start_claude(&app).await;

    post_fixture(&app, "event", &tab, "UserPromptSubmit").await;
    post_fixture(&app, "event", &tab, "Stop").await;
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

    let req = hook(Some("event"), Some("codex"), Some(&tab), fixture("UserPromptSubmit"));
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    let info = row(&st, &tab);
    stop(&st, &tab);
    assert_eq!((info.live, info.status_note), (None, None));
}
