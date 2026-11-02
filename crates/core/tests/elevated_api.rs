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
const START_CREATE: &str = "Only Marshell's own window can start a session as administrator.";
const START_AGAIN: &str = "This session runs as administrator, so only Marshell's own window can start it.";
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

/// The window's door: a create request from JSON.
async fn trusted_create(
    st: &AppState,
    body: serde_json::Value,
) -> Result<marshell_protocol::api::CreateSessionResponse, String> {
    let req = serde_json::from_value(body).unwrap();
    st.trusted_for_tests().create(req).await
}

fn create_body(profile_id: &str) -> serde_json::Value {
    serde_json::json!({ "cols": 80, "rows": 24, "profile_id": profile_id, "elevated": true })
}

#[cfg(windows)]
const OLD_CODEX_ADMIN: &str = "Codex 0.162.9 can't run as administrator. Marshell needs Codex 0.163.0 or newer to keep an administrator Codex apart from your other Codex sessions. Update Codex, then try again.";
#[cfg(windows)]
const UNREADABLE_CODEX_ADMIN: &str =
    "Marshell couldn't read this Codex's version, so it can't start it as administrator.";

/// Never starts the real Codex; counts starts and substitutes the test child.
#[cfg(windows)]
fn codex_spawner(calls: Arc<AtomicUsize>) -> ElevatedSpawner {
    Arc::new(move |id, mut run| {
        calls.fetch_add(1, Ordering::SeqCst);
        run.argv = spec("sleep").argv;
        Session::spawn_marked_elevated_for_tests(id, run)
    })
}

#[cfg(windows)]
async fn trusted_codex_create(version: Option<&str>, refusal: Option<&str>) {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path())
        .with_codex_version(version.map(str::to_string))
        .with_agent_check(|_| true)
        .with_elevated_spawner(codex_spawner(calls.clone()));
    let app = router(st.clone());
    let mut body = create_body("sleeper");
    body["agent"] = serde_json::json!("codex");
    let result = trusted_create(&st, body).await;
    if let Some(reason) = refusal {
        assert_eq!(result.unwrap_err(), reason);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(st.live_session_count(), 0);
        assert!(rows(&app).await.is_empty());
    } else {
        let tab = result.unwrap().tab_id;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(st.session(&tab).unwrap().is_elevated());
        st.remove_session(&tab).unwrap().force_kill();
    }
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_create_elevated_codex_refuses_an_old_version_without_starting() {
    trusted_codex_create(Some("0.162.9"), Some(OLD_CODEX_ADMIN)).await;
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_create_elevated_codex_refuses_an_unreadable_version_without_starting() {
    for version in [None, Some("garbage")] {
        trusted_codex_create(version, Some(UNREADABLE_CODEX_ADMIN)).await;
    }
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_create_elevated_codex_starts_at_the_no_daemon_version_or_newer() {
    for version in ["0.163.0", "0.170.2"] {
        trusted_codex_create(Some(version), None).await;
    }
}

#[cfg(windows)]
async fn trusted_codex_start_again(action: &str) {
    use marshell_protocol::api::{RestartRequest, SessionMode};
    for (version, reason) in [(Some("0.162.9"), OLD_CODEX_ADMIN), (None, UNREADABLE_CODEX_ADMIN)] {
        let home = tempfile::tempdir().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let st = state_in(home.path())
            .with_codex_version(version.map(str::to_string))
            .with_agent_check(|_| true)
            .with_elevated_spawner(codex_spawner(calls.clone()));
        st.registry().reserve(
            "tc",
            Relaunch {
                profile_id: "sleeper".into(),
                agent: Some("codex".into()),
                cwd: String::new(),
                mode: Some(SessionMode::Manual),
                name: "Codex".into(),
                preset: None,
                elevated: true,
            },
        );
        let session = Session::spawn_marked_elevated_for_tests("tc".into(), spec("sleep")).unwrap();
        st.insert_session(session.clone());
        let trusted = st.trusted_for_tests();
        if action == "restart" {
            let restart = serde_json::from_value::<RestartRequest>(
                serde_json::json!({ "mode": "bypass", "cols": 80, "rows": 24 }),
            )
            .unwrap();
            assert_eq!(trusted.restart("tc", restart).await.unwrap_err(), reason);
            assert!(Arc::ptr_eq(&st.session("tc").unwrap(), &session));
            assert_eq!(st.registry().row("tc").unwrap().mode, Some(SessionMode::Manual));
            st.remove_session("tc").unwrap().force_kill();
        } else {
            st.remove_session("tc").unwrap().force_kill();
            st.registry()
                .mark_exited("tc", st.registry().generation("tc").unwrap(), 0);
            if action == "resume" {
                assert_eq!(trusted.resume("tc", 80, 24).await.unwrap_err(), reason);
                assert!(matches!(
                    st.registry().row("tc").unwrap().state,
                    marshell_protocol::api::SessionState::Exited { .. }
                ));
            } else {
                st.registry().close("tc");
                assert_eq!(trusted.reopen(Some("tc".into())).await.unwrap_err(), reason);
                assert!(st.registry().peek_closed(Some("tc")).is_some());
            }
            assert_eq!(st.live_session_count(), 0);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_elevated_codex_restart_refuses_before_changing_the_row() {
    trusted_codex_start_again("restart").await;
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_elevated_codex_resume_refuses_before_changing_the_row() {
    trusted_codex_start_again("resume").await;
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_elevated_codex_reopen_refuses_before_changing_the_row() {
    trusted_codex_start_again("reopen").await;
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
async fn trusted_create_elevated_declined_says_so() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path()).with_elevated_spawner(failing_spawner(|| ElevatedError::Declined));
    let app = router(st.clone());
    let err = trusted_create(&st, create_body("sleeper")).await.unwrap_err();
    assert_eq!(err, "Administrator rights were declined. Nothing was started.");
    // Nothing was started: no row, no session.
    assert!(rows(&app).await.is_empty());
    assert_eq!(st.live_session_count(), 0);
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_create_elevated_other_failures_say_their_copy() {
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path()).with_elevated_spawner(failing_spawner(|| ElevatedError::Missing));
    let err = trusted_create(&st, create_body("sleeper")).await.unwrap_err();
    assert_eq!(
        err,
        "The administrator helper (marshell-host) is missing next to Marshell. Reinstall Marshell."
    );
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_create_elevated_starts_through_the_elevated_spawner_and_marks_the_row() {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path()).with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    let tab = trusted_create(&st, create_body("sleeper")).await.unwrap().tab_id;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(st.session(&tab).unwrap().is_elevated());
    let listed = rows(&app).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["elevated"], true);
    st.remove_session(&tab).unwrap().force_kill();
}

/// Decision 5: elevation is never remembered, so an elevated start (whose agent mode is forced to Manual) leaves the
/// folder's launcher memory as it was.
#[cfg(windows)]
#[tokio::test]
async fn trusted_create_elevated_leaves_the_launcher_memory_alone() {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path()).with_elevated_spawner(marked_spawner(calls.clone()));
    let cwd = home.path().to_string_lossy().to_string();
    let mut body = create_body("sleeper");
    body["cwd"] = serde_json::Value::String(cwd);
    let tab = trusted_create(&st, body).await.unwrap().tab_id;
    let doc = marshell_core::layout::load(home.path());
    assert!(doc.launcher.is_empty(), "{:?}", doc.launcher);
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

// -- starting is the window's alone (decision 10) --

/// A Running elevated agent row (Claude, in `mode`) with a marked local session behind it.
fn elevated_agent_row(st: &AppState, tab: &str, mode: marshell_protocol::api::SessionMode) -> Arc<Session> {
    st.registry().reserve(
        tab,
        Relaunch {
            profile_id: "sleeper".into(),
            agent: Some("claude".into()),
            cwd: String::new(),
            mode: Some(mode),
            name: tab.into(),
            preset: None,
            elevated: true,
        },
    );
    let session = Session::spawn_marked_elevated_for_tests(tab.into(), spec("sleep")).unwrap();
    st.insert_session(session.clone());
    session
}

#[cfg(windows)]
#[tokio::test]
async fn api_create_elevated_is_409_and_starts_nothing() {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path()).with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    let (status, body) = send(&app, api("POST", "/v1/sessions", Some(create_body("sleeper")))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body, START_CREATE);
    // An explicit shell argv is no way round it.
    let mut shell = create_body("sleeper");
    shell["shell"] = serde_json::json!(["cmd.exe"]);
    let (status, body) = send(&app, api("POST", "/v1/sessions", Some(shell))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(rows(&app).await.is_empty());
}

#[cfg(windows)]
#[tokio::test]
async fn api_create_cli_task_elevated_is_409() {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path()).with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st);
    let mut body = create_body("sleeper");
    body["cli_task"] = serde_json::json!({ "agent": "claude", "kind": "sign_in" });
    let (status, text) = send(&app, api("POST", "/v1/sessions", Some(body))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    assert_eq!(text, START_CREATE);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn api_restart_and_resume_of_an_elevated_row_are_409() {
    use marshell_protocol::api::SessionMode;
    let home = tempfile::tempdir().unwrap();
    let st = state_in(home.path()).with_agent_check(|_| true);
    let app = router(st.clone());
    let old = elevated_agent_row(&st, "te", SessionMode::Manual);
    let attempts = [
        (
            "restart",
            serde_json::json!({ "mode": "bypass", "cols": 80, "rows": 24 }),
        ),
        ("resume", serde_json::json!({ "cols": 80, "rows": 24 })),
    ];
    for (verb, body) in attempts {
        let (status, text) = send(&app, api("POST", &format!("/v1/sessions/te/{verb}"), Some(body))).await;
        assert_eq!(status, StatusCode::CONFLICT, "{verb}: {text}");
        assert!(text.contains(START_AGAIN), "{verb}: {text}");
    }
    // Refused before anything was touched: the same process still runs, in the same mode.
    assert!(Arc::ptr_eq(&old, &st.session("te").unwrap()));
    assert!(!old.has_exited());
    assert_eq!(rows(&app).await[0]["mode"], "manual");
    st.remove_session("te").unwrap().force_kill();
}

#[tokio::test]
async fn api_reopen_of_a_saved_elevated_row_is_409_and_keeps_it() {
    use marshell_protocol::api::SessionMode;
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path())
        .with_agent_check(|_| true)
        .with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    elevated_agent_row(&st, "te", SessionMode::Manual);
    st.registry().close("te");
    st.remove_session("te").unwrap().force_kill();
    for body in [serde_json::json!({}), serde_json::json!({ "tab_id": "te" })] {
        let (status, text) = send(&app, api("POST", "/v1/sessions/reopen", Some(body))).await;
        assert_eq!(status, StatusCode::CONFLICT, "{text}");
        assert!(text.contains(START_AGAIN), "{text}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // The entry is still there for the window's Reopen.
    assert!(st.registry().peek_closed(Some("te")).is_some());
}

#[cfg(windows)]
#[tokio::test]
async fn trusted_create_of_an_elevated_agent_forces_manual_and_refuses_an_ordinary_request() {
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path())
        .with_agent_check(|_| true)
        .with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    let mut body = create_body("sleeper");
    body["agent"] = serde_json::json!("claude");
    body["mode"] = serde_json::json!("bypass");
    let tab = trusted_create(&st, body).await.unwrap().tab_id;
    let listed = rows(&app).await;
    assert_eq!(listed[0]["mode"], "manual");
    assert_eq!(listed[0]["elevated"], true);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    st.remove_session(&tab).unwrap().force_kill();

    // An ordinary request keeps using the API: the window's door takes none, and a sign-in tab is never elevated.
    let mut plain = create_body("sleeper");
    plain["elevated"] = serde_json::json!(false);
    assert_eq!(trusted_create(&st, plain).await.unwrap_err(), NOT_ELEVATED);
    let mut task = create_body("sleeper");
    task["cli_task"] = serde_json::json!({ "agent": "claude", "kind": "sign_in" });
    assert!(trusted_create(&st, task).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn trusted_restart_takes_the_bar_mode_and_refuses_an_ordinary_row() {
    use marshell_protocol::api::{RestartRequest, SessionMode};
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path())
        .with_agent_check(|_| true)
        .with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    elevated_agent_row(&st, "te", SessionMode::Manual);
    let bypass = || RestartRequest {
        mode: SessionMode::Bypass,
        cols: 80,
        rows: 24,
    };
    st.trusted_for_tests().restart("te", bypass()).await.unwrap();
    // The Bypass bar's choice is kept: it is the one path with the confirmation.
    assert_eq!(rows(&app).await[0]["mode"], "bypass");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(st.session("te").unwrap().is_elevated());

    st.registry().reserve(
        "tp",
        Relaunch {
            profile_id: "sleeper".into(),
            agent: Some("claude".into()),
            cwd: String::new(),
            mode: Some(SessionMode::Manual),
            name: "tp".into(),
            preset: None,
            elevated: false,
        },
    );
    let err = st.trusted_for_tests().restart("tp", bypass()).await.unwrap_err();
    assert_eq!(err, NOT_ELEVATED);
    st.remove_session("te").unwrap().force_kill();
}

#[cfg(windows)]
#[tokio::test]
async fn resume_and_reopen_start_an_elevated_agent_in_manual_whatever_it_was_saved_in() {
    use marshell_protocol::api::SessionMode;
    let home = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let st = state_in(home.path())
        .with_agent_check(|_| true)
        .with_elevated_spawner(marked_spawner(calls.clone()));
    let app = router(st.clone());
    let trusted = st.trusted_for_tests();

    // Resume: an ended row saved in Bypass.
    elevated_agent_row(&st, "te", SessionMode::Bypass);
    st.remove_session("te").unwrap().force_kill();
    st.registry()
        .mark_exited("te", st.registry().generation("te").unwrap(), 0);
    assert_eq!(rows(&app).await[0]["mode"], "bypass");
    trusted.resume("te", 80, 24).await.unwrap();
    assert_eq!(rows(&app).await[0]["mode"], "manual");
    st.remove_session("te").unwrap().force_kill();

    // Reopen: a closed row saved in Bypass.
    elevated_agent_row(&st, "tr", SessionMode::Bypass);
    st.registry().close("tr");
    st.remove_session("tr").unwrap().force_kill();
    let reopened = trusted.reopen(Some("tr".into())).await.unwrap();
    let listed = rows(&app).await;
    let row = listed.iter().find(|r| r["tab_id"] == reopened.tab_id).unwrap();
    assert_eq!(row["mode"], "manual");
    assert_eq!(row["elevated"], true);
    st.remove_session(&reopened.tab_id).unwrap().force_kill();

    // The window's Reopen takes administrator rows only.
    elevated_row(&st, "tq", "sleep");
    st.registry().close("tq");
    st.remove_session("tq").unwrap().force_kill();
    plain_row(&st, "tp", "sleep");
    st.registry().close("tp");
    st.remove_session("tp").unwrap().force_kill();
    assert_eq!(trusted.reopen(Some("tp".into())).await.unwrap_err(), NOT_ELEVATED);
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

    // The window's Resume starts it elevated again, through the elevated spawner. The ordinary one does not use it.
    second.trusted_for_tests().resume("te", 80, 24).await.unwrap();
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
    let err = second.trusted_for_tests().resume("te", 80, 24).await.unwrap_err();
    assert_eq!(err, "Administrator rights were declined. Nothing was started.");
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
