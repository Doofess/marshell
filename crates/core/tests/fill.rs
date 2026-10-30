//! `POST /v1/sessions/{tab}/fill`: the answers, and that the text reaches the pty as it is.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::pty::{Pull, Session};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;

fn state(home: &Path) -> AppState {
    common::tie_children_to_test();
    let profile = |id: &str, mode: &str| Profile {
        id: id.into(),
        name: id.into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), mode.into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.to_path_buf(),
        vec![profile("echoer", "echo"), profile("printer", "print")],
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

async fn body_of(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn start(app: &axum::Router, profile: &str, cwd: &Path) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": profile, "cwd": cwd.to_string_lossy() });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body_of(res).await).unwrap();
    json["tab_id"].as_str().unwrap().to_string()
}

fn output_has(session: &Session, needle: &str, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if let Pull::Data { bytes, .. } = session.pull(0, 64 * 1024)
            && String::from_utf8_lossy(&bytes).contains(needle)
        {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A state whose Claude Code version and settings are pinned to scratch values, so no test runs a real `claude`
/// or reads the user's settings.
fn claude_state(home: &Path) -> AppState {
    state(home)
        .with_claude_version(Some("2.1.302".into()))
        .with_claude_compact_paths(home.join("user"), Vec::new())
        .with_claude_settings(home.join("claude-settings.json"))
}

async fn start_claude(app: &axum::Router, cwd: &Path) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "echoer", "agent": "claude", "cwd": cwd.to_string_lossy(),
    });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    serde_json::from_str::<serde_json::Value>(&body_of(res).await).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A Claude hook call carrying a recorded fixture.
fn hook_request(role: &str, tab: &str, name: &str) -> Request<Body> {
    let file = format!(
        "{}/tests/fixtures/claude/2.1.302/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut req = api("POST", "/v1/hook", None);
    *req.body_mut() = Body::from(std::fs::read(file).unwrap());
    let h = req.headers_mut();
    h.insert("x-marshell-role", role.parse().unwrap());
    h.insert("x-marshell-agent", "claude".parse().unwrap());
    h.insert("x-marshell-tab", tab.parse().unwrap());
    h.insert("x-marshell-version", "0.0.0".parse().unwrap());
    req
}

fn fill_body(text: &str, wait: bool) -> Option<serde_json::Value> {
    Some(serde_json::json!({ "text": text, "wait": wait }))
}

#[tokio::test]
async fn fill_without_wait_writes_the_text() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, "echoer", tmp.path()).await;
    let session = st.session(&tab).unwrap();
    let route = format!("/v1/sessions/{tab}/fill");
    let text = "fix the tests \u{2014} please";
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body(text, false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // No Enter was added: the agent has not seen a whole line yet.
    std::thread::sleep(Duration::from_millis(400));
    assert!(!output_has(&session, "got:", Duration::ZERO));
    // The person presses Enter, and the line that arrives is exactly the text.
    session.write_input(b"\r".to_vec());
    assert!(output_has(&session, &format!("got:{text}"), Duration::from_secs(10)));

    // A control character is refused with the line, and nothing is written.
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body("a\x1b[13u", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_of(res).await,
        "The text can't hold line breaks or other control characters: Marshell never presses Enter."
    );
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body(&"x".repeat(4001), true)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_of(res).await, "The text is over 4000 characters.");

    // An unknown session.
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions/nope/fill", fill_body("x", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body_of(res).await, "No such session.");

    // Like every route, it is refused without the bearer.
    let mut req = api("POST", &route, fill_body("x", false));
    req.headers_mut().remove(header::AUTHORIZATION);
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn fill_refuses_an_ended_row() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    // The printer writes a line and exits.
    let tab = start(&app, "printer", tmp.path()).await;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
        let json: serde_json::Value = serde_json::from_str(&body_of(res).await).unwrap();
        let row = json["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tab_id"] == tab.as_str())
            .unwrap()
            .clone();
        if row["state"]["kind"] != "running" {
            break;
        }
        assert!(Instant::now() < deadline, "the printer never exited: {row}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for wait in [false, true] {
        let res = app
            .clone()
            .oneshot(api(
                "POST",
                &format!("/v1/sessions/{tab}/fill"),
                fill_body("hello", wait),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(body_of(res).await, r#"{"reason":"ended"}"#);
    }
}

/// An agent whose hooks reported SessionEnd while its shell lives on is ended for a fill.
#[tokio::test]
async fn fill_refuses_a_row_whose_agent_has_ended() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let st = claude_state(&home);
    let app = router(st.clone());
    let tab = start_claude(&app, tmp.path()).await;
    let hook = |name: &str| hook_request("event", &tab, name);
    let live = || st.registry().row(&tab).unwrap().live.map(|l| l.state);
    let res = app.clone().oneshot(hook("SessionStart")).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(live(), Some(marshell_protocol::api::LiveState::Idle));
    let res = app.clone().oneshot(hook("SessionEnd")).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(live(), Some(marshell_protocol::api::LiveState::Ended));
    // The pty is still alive; only the agent ended.
    assert!(st.session(&tab).is_some_and(|s| !s.has_exited()));
    for wait in [false, true] {
        let res = app
            .clone()
            .oneshot(api(
                "POST",
                &format!("/v1/sessions/{tab}/fill"),
                fill_body("hello", wait),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(body_of(res).await, r#"{"reason":"ended"}"#);
    }
    // A SessionStart (a `--resume` typed in the same tab) makes it take text again.
    let res = app.clone().oneshot(hook("SessionStart")).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app
        .clone()
        .oneshot(api(
            "POST",
            &format!("/v1/sessions/{tab}/fill"),
            fill_body("hello", false),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    if let Some(session) = st.remove_session(&tab) {
        session.force_kill();
    }
}

#[tokio::test]
async fn fill_refuses_a_paused_session() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, "echoer", tmp.path()).await;
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/pause"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // Like `/input`: a frozen process would take the text only once it is continued.
    for wait in [false, true] {
        let res = app
            .clone()
            .oneshot(api(
                "POST",
                &format!("/v1/sessions/{tab}/fill"),
                fill_body("hello", wait),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(body_of(res).await, r#"{"reason":"It is paused."}"#);
    }
}

/// A session that waits for an answer takes no text: the held card stays, and nothing reaches the pty.
#[tokio::test]
async fn fill_refuses_a_session_waiting_for_an_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let st = claude_state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start_claude(&app, tmp.path()).await;
    let session = st.session(&tab).unwrap();
    let res = app
        .clone()
        .oneshot(hook_request("event", &tab, "SessionStart"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // The permission request holds its call open until it is answered.
    let held = tokio::spawn({
        let app = app.clone();
        let req = hook_request("permission", &tab, "PermissionRequest");
        async move { app.oneshot(req).await.unwrap().status() }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !st.holds().has_tab(&tab) {
        assert!(Instant::now() < deadline, "the permission request was never held");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let route = format!("/v1/sessions/{tab}/fill");
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body("1", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body_of(res).await, r#"{"reason":"waiting"}"#);
    // The card is still pending: the fill did not release it to the CLI's own dialog.
    assert!(st.holds().has_tab(&tab));
    // Nothing was typed: the person's Enter reaches the echo agent as an empty line.
    session.write_input(b"\r".to_vec());
    assert!(output_has(&session, "got:", Duration::from_secs(10)));
    assert!(
        !output_has(&session, "got:1", Duration::from_millis(300)),
        "the text was typed"
    );

    // Answered: the state clears and the fill lands.
    let rid = st.holds().pending()[0].rid.clone();
    assert!(st.holds().release(&rid));
    assert_eq!(held.await.unwrap(), StatusCode::NO_CONTENT);
    let res = app
        .clone()
        .oneshot(hook_request("event", &tab, "PostToolUse"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let live = st.registry().row(&tab).unwrap().live.map(|l| l.state);
    assert_ne!(live, Some(marshell_protocol::api::LiveState::NeedsPermission));
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body("hello", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    if let Some(session) = st.remove_session(&tab) {
        session.force_kill();
    }
}

/// A question dismissed with Escape sends no hook, so the row keeps saying it has a question: a key typed in the
/// terminal since the question appeared ends the wait (found at the live recheck).
#[tokio::test]
async fn fill_goes_on_once_the_person_typed_after_the_question() {
    let tmp = tempfile::tempdir().unwrap();
    let st = claude_state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start_claude(&app, tmp.path()).await;
    let res = app
        .clone()
        .oneshot(hook_request("event", &tab, "SessionStart"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // Typed before the question: it doesn't answer it.
    let input = format!("/v1/sessions/{tab}/input");
    let typed = |text: &str| api("POST", &input, Some(serde_json::json!({ "text": text })));
    assert_eq!(
        app.clone().oneshot(typed("a")).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut question = hook_request("event", &tab, "PreToolUse");
    let mut body: serde_json::Value = serde_json::from_str(&body_of_request(&mut question).await).unwrap();
    body["tool_name"] = "AskUserQuestion".into();
    body["tool_input"] = serde_json::json!({ "questions": [] });
    *question.body_mut() = Body::from(body.to_string());
    assert_eq!(
        app.clone().oneshot(question).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    let live = st.registry().row(&tab).unwrap().live.map(|l| l.state);
    assert_eq!(live, Some(marshell_protocol::api::LiveState::NeedsQuestion));
    let route = format!("/v1/sessions/{tab}/fill");
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body("hello", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body_of(res).await, r#"{"reason":"waiting"}"#);

    // Escape in the terminal: no hook follows, and the row still says it has a question.
    assert_eq!(
        app.clone().oneshot(typed("\u{1b}")).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    let live = st.registry().row(&tab).unwrap().live.map(|l| l.state);
    assert_eq!(live, Some(marshell_protocol::api::LiveState::NeedsQuestion));
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body("hello", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    if let Some(session) = st.remove_session(&tab) {
        session.force_kill();
    }
}

async fn body_of_request(req: &mut Request<Body>) -> String {
    let body = std::mem::replace(req.body_mut(), Body::empty());
    String::from_utf8(axum::body::to_bytes(body, usize::MAX).await.unwrap().to_vec()).unwrap()
}

/// A Claude row with Marshell's hooks installed is not ready until its SessionStart arrives, however quiet it is.
#[tokio::test]
async fn fill_with_wait_holds_for_session_start_when_the_hooks_are_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    // Settings may be written only in a recorded project folder.
    let project = marshell_core::projects::canonical(tmp.path());
    std::fs::create_dir_all(&home).unwrap();
    marshell_core::projects::record(&home, &project, 1).unwrap();
    let st = claude_state(&home).with_claude_settings(project.join("settings.json"));
    let app = router(st.clone());
    // Install the hooks the way the app does: plan, then apply what was shown.
    let bridge = marshell_core::paths::bridge_exe(&home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();
    let post_json = |uri: &str, body: serde_json::Value| {
        let app = app.clone();
        let req = api("POST", uri, Some(body));
        async move {
            let res = app.oneshot(req).await.unwrap();
            let status = res.status();
            let json = serde_json::from_str::<serde_json::Value>(&body_of(res).await).unwrap_or_default();
            (status, json)
        }
    };
    let (status, preview) = post_json(
        "/v1/agents/claude/hooks/plan",
        serde_json::json!({"status": true, "statusline": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let hashes: Vec<serde_json::Value> = preview["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| serde_json::json!({"path": f["path"], "hash": f.get("before_hash").cloned().unwrap_or_default()}))
        .collect();
    let (status, applied) = post_json(
        "/v1/config/apply",
        serde_json::json!({"changeset_id": preview["changeset_id"], "expected_hashes": hashes}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{applied}");

    let tab = start_claude(&app, tmp.path()).await;
    let route = format!("/v1/sessions/{tab}/fill");
    let started = Instant::now();
    let filling = tokio::spawn({
        let app = app.clone();
        let req = api("POST", &route, fill_body("hello", true));
        async move { app.oneshot(req).await.unwrap().status() }
    });
    // Quiet for well over 1.5 s, but no SessionStart yet.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!filling.is_finished(), "filled before the SessionStart");
    let res = app
        .clone()
        .oneshot(hook_request("event", &tab, "SessionStart"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(filling.await.unwrap(), StatusCode::NO_CONTENT);
    assert!(started.elapsed() >= Duration::from_secs(3));
    if let Some(session) = st.remove_session(&tab) {
        session.force_kill();
    }
}
