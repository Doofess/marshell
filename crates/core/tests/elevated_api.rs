//! Administrator sessions through the local API: how they start and come back, what the token-guarded routes refuse
//! for them, and the trusted door the window uses instead. Rows that must already be elevated run on the local pty,
//! marked elevated (`spawn_marked_elevated_for_tests`); the real helper is covered in `crates/host/tests`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use futures_util::SinkExt;
use marshell_core::pty::elevated::ElevatedError;
use marshell_core::pty::{Session, SpawnSpec};
use marshell_core::registry::Relaunch;
use marshell_core::server::{AppState, CoreConfig, ElevatedSpawner, router, start};
use marshell_protocol::api::{Profile, ShellFamily};
use marshell_protocol::hook::{PermissionBehavior, PermissionDecision, PermissionScope};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5561;
const ORIGIN: &str = "http://tauri.localhost";

const API_WRITE: &str = "This session runs as administrator, so it takes input only from Marshell's own window.";
const ALWAYS: &str =
    "A session running as administrator can't keep an always-allow rule from Marshell. Allow this once instead.";
const PAUSE: &str = "Pause isn't available for a session running as administrator.";
const NOT_ELEVATED: &str = "Not an administrator session.";

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

fn spec(mode: &str) -> SpawnSpec {
    SpawnSpec {
        argv: vec![env!("CARGO_BIN_EXE_fake-agent").into(), mode.into()],
        cwd: None,
        env: Vec::new(),
        env_remove: Vec::new(),
        env_strip: None,
        cols: 80,
        rows: 24,
        on_exit: None,
        on_mark: None,
        on_tool: None,
        record: None,
        classify: None,
        initial_command: None,
    }
}

fn state_in(home: &Path) -> AppState {
    common::tie_children_to_test();
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![profile("sleeper", "sleep"), profile("echoer", "echo")],
    )
    .with_claude_version(Some("2.1.302".into()))
    .with_agent_path(|_| None)
    .with_permission_hold(Duration::from_secs(30))
}

/// An elevated spawner that runs the session on the local pty, marked elevated, and counts its calls.
fn marked_spawner(calls: Arc<AtomicUsize>) -> ElevatedSpawner {
    Arc::new(move |id, spec| {
        calls.fetch_add(1, Ordering::SeqCst);
        Session::spawn_marked_elevated_for_tests(id, spec)
    })
}

/// An elevated spawner that fails with `error`.
fn failing_spawner(error: fn() -> ElevatedError) -> ElevatedSpawner {
    Arc::new(move |_, _| Err(anyhow::Error::new(error())))
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
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn rows(app: &axum::Router) -> Vec<serde_json::Value> {
    let (status, body) = send(app, api("GET", "/v1/sessions", None)).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_str::<serde_json::Value>(&body).unwrap()["sessions"]
        .as_array()
        .unwrap()
        .clone()
}

fn create_body(profile_id: &str) -> serde_json::Value {
    serde_json::json!({ "cols": 80, "rows": 24, "profile_id": profile_id, "elevated": true })
}

/// A Running elevated row with a (locally run, marked) session behind it.
fn elevated_row(st: &AppState, tab: &str, mode: &str) -> Arc<Session> {
    st.registry().reserve(
        tab,
        Relaunch {
            profile_id: "sleeper".into(),
            agent: None,
            cwd: String::new(),
            mode: None,
            name: tab.into(),
            preset: None,
            elevated: true,
        },
    );
    let session = Session::spawn_marked_elevated_for_tests(tab.into(), spec(mode)).unwrap();
    st.insert_session(session.clone());
    session
}

/// A Running ordinary row with a session behind it.
fn plain_row(st: &AppState, tab: &str, mode: &str) -> Arc<Session> {
    st.registry().reserve(
        tab,
        Relaunch {
            profile_id: "sleeper".into(),
            agent: None,
            cwd: String::new(),
            mode: None,
            name: tab.into(),
            preset: None,
            elevated: false,
        },
    );
    let session = Session::spawn(tab.into(), spec(mode)).unwrap();
    st.insert_session(session.clone());
    session
}

fn screen(session: &Session) -> String {
    session.peek(24).join("\n")
}

async fn wait_for_screen(session: &Session, needle: &str) {
    let start = Instant::now();
    while !screen(session).contains(needle) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{needle:?} never showed; screen: {:?}",
            screen(session)
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

// -- create --

#[cfg(windows)]
#[tokio::test]
async fn create_elevated_declined_is_409_with_the_copy() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path()).with_elevated_spawner(failing_spawner(|| ElevatedError::Declined));
    let app = router(st.clone());
    let (status, body) = send(&app, api("POST", "/v1/sessions", Some(create_body("sleeper")))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body, "Administrator rights were declined. Nothing was started.");
    // Nothing was started: no row, no session.
    assert!(rows(&app).await.is_empty());
    assert_eq!(st.live_session_count(), 0);
}

#[cfg(windows)]
#[tokio::test]
async fn create_elevated_other_failures_are_500_with_their_copy() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path()).with_elevated_spawner(failing_spawner(|| ElevatedError::Missing));
    let app = router(st);
    let (status, body) = send(&app, api("POST", "/v1/sessions", Some(create_body("sleeper")))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        "The administrator helper (marshell-host) is missing next to Marshell. Reinstall Marshell."
    );
}

#[cfg(windows)]
#[tokio::test]
async fn create_elevated_starts_through_the_elevated_spawner_and_marks_the_row() {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path()).with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    let (status, body) = send(&app, api("POST", "/v1/sessions", Some(create_body("sleeper")))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tab = serde_json::from_str::<serde_json::Value>(&body).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(st.session(&tab).unwrap().is_elevated());
    let listed = rows(&app).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["elevated"], true);
    st.remove_session(&tab).unwrap().force_kill();
}

#[cfg(not(windows))]
#[tokio::test]
async fn create_elevated_off_windows_is_400() {
    let home = tempfile::tempdir().unwrap();
    let app = router(state_in(home.path()));
    let (status, body) = send(&app, api("POST", "/v1/sessions", Some(create_body("sleeper")))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "Running as administrator is available on Windows only for now.");
}

// -- the API's writes --

#[tokio::test]
async fn send_fill_and_attachments_refuse_an_elevated_row() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path());
    let app = router(st.clone());
    let session = elevated_row(&st, "te", "echo");
    let attempts = [
        ("/v1/sessions/te/input", serde_json::json!({ "text": "refused-input" })),
        (
            "/v1/sessions/te/fill",
            serde_json::json!({ "text": "refused-fill", "wait": false }),
        ),
        (
            "/v1/sessions/te/paste",
            serde_json::json!({ "attachment": "1.png", "text": "refused-paste" }),
        ),
        (
            "/v1/sessions/te/paste-paths",
            serde_json::json!({ "paths": [home.path().to_string_lossy()] }),
        ),
    ];
    for (path, body) in attempts {
        let (status, text) = send(&app, api("POST", path, Some(body))).await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}: {text}");
        assert!(text.contains(API_WRITE), "{path}: {text}");
    }
    // Nothing reached the program: the window's own door still works, and shows only its own line.
    st.trusted_for_tests().input("te", b"ok\r".to_vec()).unwrap();
    wait_for_screen(&session, "got:ok").await;
    assert!(!screen(&session).contains("refused"), "screen: {}", screen(&session));
    st.remove_session("te").unwrap().force_kill();
}

#[tokio::test]
async fn ws_input_to_an_elevated_row_is_dropped() {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let core = start(CoreConfig {
        home: home.path().to_path_buf(),
        allowed_origins: vec![],
        programs_dir: None,
        notify: None,
        focus: None,
        notify_paused: Default::default(),
        window_hidden: Default::default(),
        import: None,
    })
    .await
    .unwrap();
    let st = core.state.clone();
    let session = elevated_row(&st, "tw", "echo");
    let mut req = format!("ws://127.0.0.1:{}/v1/pty/tw", core.endpoint.port)
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{}", core.endpoint.token).parse().unwrap(),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    let mut resume = vec![0x13];
    resume.extend_from_slice(&0u64.to_be_bytes());
    ws.send(Message::Binary(resume.into())).await.unwrap();
    // An Input frame and a Resize frame, which a foreign client could send.
    let mut input = vec![0x10];
    input.extend_from_slice(b"dropped\r");
    ws.send(Message::Binary(input.into())).await.unwrap();
    let mut resize = vec![0x11];
    resize.extend_from_slice(&200u16.to_be_bytes());
    resize.extend_from_slice(&50u16.to_be_bytes());
    ws.send(Message::Binary(resize.into())).await.unwrap();
    // The socket still delivers output: the window's own input comes through the trusted door and is echoed.
    st.trusted_for_tests().input("tw", b"kept\r".to_vec()).unwrap();
    wait_for_screen(&session, "got:kept").await;
    assert!(!screen(&session).contains("dropped"), "screen: {}", screen(&session));
    // And the socket is still open and carries what the session printed.
    let frame = tokio::time::timeout(Duration::from_secs(5), futures_util::StreamExt::next(&mut ws))
        .await
        .expect("the socket should still deliver output");
    assert!(matches!(frame, Some(Ok(Message::Binary(_)))));
    // The drop left the size alone.
    st.trusted_for_tests().resize("tw", 100, 30).unwrap();
    st.remove_session("tw").unwrap().force_kill();
    core.shutdown();
}

#[tokio::test]
async fn pause_on_an_elevated_row_is_409() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path());
    let app = router(st.clone());
    elevated_row(&st, "te", "sleep");
    let (status, body) = send(&app, api("POST", "/v1/sessions/te/pause", None)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body.contains(PAUSE), "{body}");
    assert_eq!(rows(&app).await[0]["paused"], false);
    st.remove_session("te").unwrap().force_kill();
}

// -- approvals --

fn bash_request() -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PermissionRequest",
        "tool_name": "Bash",
        "tool_input": { "command": "npm test" },
        "permission_suggestions": [
            { "type": "addRules", "behavior": "allow", "rules": [{ "toolName": "Bash", "ruleContent": "npm test:*" }] }
        ]
    })
}

fn hook(app: &axum::Router, tab: &str) -> tokio::task::JoinHandle<(StatusCode, Vec<u8>)> {
    let req = Request::builder()
        .method("POST")
        .uri("/v1/hook")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header("x-marshell-role", "permission")
        .header("x-marshell-agent", "claude")
        .header("x-marshell-version", "0.0.0")
        .header("x-marshell-tab", tab)
        .body(Body::from(bash_request().to_string()))
        .unwrap();
    let app = app.clone();
    tokio::spawn(async move {
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, bytes.to_vec())
    })
}

async fn wait_pending(st: &AppState, count: usize) -> Vec<marshell_core::hooks::holds::PendingPermission> {
    let start = Instant::now();
    loop {
        let pending = st.holds().pending();
        if pending.len() == count {
            return pending;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "never {count} pending");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn decision(scope: PermissionScope) -> PermissionDecision {
    PermissionDecision {
        behavior: PermissionBehavior::Allow,
        scope,
        message: None,
    }
}

#[tokio::test]
async fn the_permission_route_refuses_an_elevated_row_and_trusted_decide_accepts() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path());
    let app = router(st.clone());
    elevated_row(&st, "te", "sleep");
    plain_row(&st, "tp", "sleep");
    let held = hook(&app, "te");
    let rid = wait_pending(&st, 1).await[0].rid.clone();

    // The token-guarded route refuses, and the request stays pending.
    let (status, body) = send(
        &app,
        api(
            "POST",
            &format!("/v1/permissions/{rid}/decision"),
            Some(serde_json::json!({ "behavior": "allow" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body.contains(API_WRITE), "{body}");
    assert_eq!(st.holds().pending().len(), 1);
    assert!(!held.is_finished());

    // The window's door answers it.
    st.trusted_for_tests()
        .decide(&rid, decision(PermissionScope::Once))
        .await
        .unwrap();
    let (status, _) = held.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert!(st.holds().pending().is_empty());

    // A request of an ordinary tab is not the trusted door's to answer.
    let ordinary = hook(&app, "tp");
    let rid = wait_pending(&st, 1).await[0].rid.clone();
    let refused = st
        .trusted_for_tests()
        .decide(&rid, decision(PermissionScope::Once))
        .await
        .unwrap_err();
    assert_eq!(refused.1, NOT_ELEVATED);
    assert_eq!(st.holds().pending().len(), 1);
    // The route still answers it, as before.
    let (status, _) = send(
        &app,
        api(
            "POST",
            &format!("/v1/permissions/{rid}/decision"),
            Some(serde_json::json!({ "behavior": "allow" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    ordinary.await.unwrap();
    for tab in ["te", "tp"] {
        st.remove_session(tab).unwrap().force_kill();
    }
}

#[tokio::test]
async fn always_scope_on_an_elevated_row_is_409_and_the_view_has_no_rule() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path());
    let app = router(st.clone());
    elevated_row(&st, "te", "sleep");
    plain_row(&st, "tp", "sleep");
    let elevated = hook(&app, "te");
    let ordinary = hook(&app, "tp");
    let pending = wait_pending(&st, 2).await;
    let rid_of = |tab: &str| {
        pending
            .iter()
            .find(|p| p.tab.as_deref() == Some(tab))
            .unwrap()
            .rid
            .clone()
    };

    let views = st.permission_views();
    let view_of = |tab: &str| views.iter().find(|v| v.tab == tab).unwrap();
    assert_eq!(view_of("te").always_rule, None);
    assert_eq!(view_of("tp").always_rule.as_deref(), Some("Bash(npm test:*)"));

    let refused = st
        .trusted_for_tests()
        .decide(&rid_of("te"), decision(PermissionScope::Project))
        .await
        .unwrap_err();
    assert_eq!(refused, (409, ALWAYS.to_string()));
    // It stays pending, and the window can still allow it once.
    assert_eq!(st.holds().pending().len(), 2);
    st.trusted_for_tests()
        .decide(&rid_of("te"), decision(PermissionScope::Once))
        .await
        .unwrap();
    elevated.await.unwrap();
    ordinary.abort();
    for tab in ["te", "tp"] {
        st.remove_session(tab).unwrap().force_kill();
    }
}

// -- trusted --

#[tokio::test]
async fn trusted_input_refuses_a_plain_session() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path());
    let plain = plain_row(&st, "tp", "echo");
    let trusted = st.trusted_for_tests();
    assert_eq!(trusted.input("tp", b"x\r".to_vec()), Err(NOT_ELEVATED.to_string()));
    assert_eq!(trusted.resize("tp", 100, 30), Err(NOT_ELEVATED.to_string()));
    assert!(trusted.input("nope", b"x".to_vec()).is_err());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!screen(&plain).contains("got:"), "screen: {}", screen(&plain));
    st.remove_session("tp").unwrap().force_kill();
}

// -- restart, resume, layout --

fn saved_elevated(home: &Path) -> Option<Vec<bool>> {
    let text = std::fs::read(home.join("layout.json")).ok()?;
    let doc: serde_json::Value = serde_json::from_slice(&text).ok()?;
    Some(
        doc["tabs"]
            .as_array()?
            .iter()
            .map(|t| t["elevated"].as_bool().unwrap_or(false))
            .collect(),
    )
}

#[tokio::test]
async fn elevated_round_trips_through_layout_and_restores_ended() {
    let home = tempfile::tempdir().unwrap();
    let first = state_in(home.path());
    let app = router(first.clone());
    elevated_row(&first, "te", "sleep");
    plain_row(&first, "tp", "sleep");
    let start = Instant::now();
    while saved_elevated(home.path()) != Some(vec![true, false]) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "layout.json never kept the flag: {:?}",
            saved_elevated(home.path())
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    for tab in ["te", "tp"] {
        first.remove_session(tab).unwrap().force_kill();
    }
    drop(app);
    drop(first);

    // A new core on the same home: both come back Ended, and nothing is started.
    let calls = Arc::new(AtomicUsize::new(0));
    let second = state_in(home.path()).with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(second.clone());
    let listed = rows(&app).await;
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0]["state"]["kind"], "ended");
    assert_eq!(listed[0]["elevated"], true);
    assert_eq!(listed[1]["elevated"], false);
    assert_eq!(second.live_session_count(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // Resume starts it elevated again, through the elevated spawner. The ordinary one does not use it.
    let (status, body) = send(
        &app,
        api(
            "POST",
            "/v1/sessions/te/resume",
            Some(serde_json::json!({ "cols": 80, "rows": 24 })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(second.session("te").unwrap().is_elevated());
    let (status, _) = send(
        &app,
        api(
            "POST",
            "/v1/sessions/tp/resume",
            Some(serde_json::json!({ "cols": 80, "rows": 24 })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!second.session("tp").unwrap().is_elevated());
    for tab in ["te", "tp"] {
        second.remove_session(tab).unwrap().force_kill();
    }
}

#[tokio::test]
async fn resume_of_an_elevated_tab_never_falls_back_to_a_plain_start() {
    let home = tempfile::tempdir().unwrap();
    let first = state_in(home.path());
    let app = router(first.clone());
    elevated_row(&first, "te", "sleep");
    let start = Instant::now();
    while saved_elevated(home.path()) != Some(vec![true]) {
        assert!(start.elapsed() < Duration::from_secs(5), "layout.json never saved");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    first.remove_session("te").unwrap().force_kill();
    drop(app);
    drop(first);

    let second = state_in(home.path()).with_elevated_spawner(failing_spawner(|| ElevatedError::Declined));
    let app = router(second.clone());
    let (status, body) = send(
        &app,
        api(
            "POST",
            "/v1/sessions/te/resume",
            Some(serde_json::json!({ "cols": 80, "rows": 24 })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body, "Administrator rights were declined. Nothing was started.");
    // Still Ended and elevated; no process of any kind.
    let listed = rows(&app).await;
    assert_eq!(listed[0]["state"]["kind"], "ended");
    assert_eq!(listed[0]["elevated"], true);
    assert_eq!(second.live_session_count(), 0);
}

// -- the guard --

#[tokio::test]
async fn the_changed_routes_are_refused_without_the_bearer() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path());
    let app = router(st.clone());
    elevated_row(&st, "te", "sleep");
    let routes = [
        (
            "POST",
            "/v1/sessions",
            serde_json::json!({ "cols": 80, "rows": 24, "elevated": true }),
        ),
        ("POST", "/v1/sessions/reopen", serde_json::json!({})),
        (
            "POST",
            "/v1/sessions/te/resume",
            serde_json::json!({ "cols": 80, "rows": 24 }),
        ),
        (
            "POST",
            "/v1/sessions/te/restart",
            serde_json::json!({ "mode": "manual", "cols": 80, "rows": 24 }),
        ),
        ("POST", "/v1/sessions/te/pause", serde_json::json!({})),
        ("POST", "/v1/sessions/te/input", serde_json::json!({ "text": "x" })),
        (
            "POST",
            "/v1/sessions/te/fill",
            serde_json::json!({ "text": "x", "wait": false }),
        ),
        (
            "POST",
            "/v1/sessions/te/paste",
            serde_json::json!({ "attachment": "1.png", "text": "" }),
        ),
        (
            "POST",
            "/v1/sessions/te/paste-paths",
            serde_json::json!({ "paths": [] }),
        ),
        (
            "POST",
            "/v1/permissions/r1/decision",
            serde_json::json!({ "behavior": "allow" }),
        ),
    ];
    for (method, path, body) in routes {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let (status, _) = send(&app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    st.remove_session("te").unwrap().force_kill();
}
