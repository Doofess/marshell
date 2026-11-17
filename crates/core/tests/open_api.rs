//! `POST /v1/sessions/{tab}/open` and `POST /v1/pane/opens/done`: what `marshell open` asks for waits in the core
//! until the app has added the tab, and the command hears how it went.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{PaneOpen, Profile, ShellFamily};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5561;
const LATE: &str = "Marshell hasn't answered yet. The tab opens when its window does.";
const BAD_PATH: &str = "That path can't be opened in the side pane.";

fn state(home: &Path) -> AppState {
    common::tie_children_to_test();
    let profile = Profile {
        id: "echoer".into(),
        name: "echoer".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "echo".into()],
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
        vec![profile],
    )
}

fn api(method: &str, path: &str, bearer: bool, body: Option<serde_json::Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"));
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
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

async fn start_with(app: &axum::Router, cwd: &Path, agent: Option<&str>) -> String {
    let mut body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "echoer", "cwd": cwd.to_string_lossy() });
    if let Some(agent) = agent {
        body["agent"] = agent.into();
    }
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", true, Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body_of(res).await).unwrap();
    json["tab_id"].as_str().unwrap().to_string()
}

async fn start(app: &axum::Router, cwd: &Path) -> String {
    start_with(app, cwd, None).await
}

/// Asks for `target` in `tab`'s side pane, as the command does. The answer's status and its text.
async fn open(app: &axum::Router, tab: &str, target: &str, from: Option<&str>) -> (StatusCode, String) {
    let body = serde_json::json!({ "target": target, "from": from });
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/open"), true, Some(body)))
        .await
        .unwrap();
    (res.status(), body_of(res).await)
}

async fn done(app: &axum::Router, done: serde_json::Value) -> StatusCode {
    let body = serde_json::json!({ "done": done });
    app.clone()
        .oneshot(api("POST", "/v1/pane/opens/done", true, Some(body)))
        .await
        .unwrap()
        .status()
}

/// Plays the app: answers every request the queue shows through the done route, and keeps what it saw.
fn fake_app(st: &AppState, app: &axum::Router, refused: Option<&'static str>) -> Arc<Mutex<Vec<PaneOpen>>> {
    let seen: Arc<Mutex<Vec<PaneOpen>>> = Arc::default();
    let (mut rx, app, out) = (st.opens().subscribe(), app.clone(), seen.clone());
    tokio::spawn(async move {
        loop {
            let waiting = rx.borrow_and_update().clone();
            if !waiting.is_empty() {
                let answers: Vec<_> = waiting
                    .iter()
                    .map(|o| serde_json::json!({ "id": o.id, "refused": refused }))
                    .collect();
                out.lock().unwrap().extend(waiting);
                assert_eq!(done(&app, answers.into()).await, StatusCode::NO_CONTENT);
            }
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
    seen
}

fn waiting(st: &AppState) -> Vec<PaneOpen> {
    st.opens().subscribe().borrow().clone()
}

/// A file to open, and its path as text.
fn a_file(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, "x").unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn open_is_refused_without_the_bearer() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let body = serde_json::json!({ "target": a_file(tmp.path(), "a.md") });
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/sessions/{tab}/open"), false, Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(waiting(&st).is_empty());
}

#[tokio::test]
async fn done_is_refused_without_the_bearer() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    assert_eq!(
        open(&app, &tab, &a_file(tmp.path(), "a.md"), None).await.0,
        StatusCode::ACCEPTED
    );
    let id = waiting(&st)[0].id;
    let body = serde_json::json!({ "done": [{ "id": id }] });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/pane/opens/done", false, Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(waiting(&st).len(), 1);
}

#[cfg(windows)]
#[tokio::test]
async fn open_an_address_queues_it_and_answers_when_the_app_is_done() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let seen = fake_app(&st, &app, None);
    let (status, text) = open(&app, &tab, "https://example.com/a?b=c", None).await;
    assert_eq!((status, text.as_str()), (StatusCode::NO_CONTENT, ""));
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (
            seen[0].tab_id.as_str(),
            seen[0].kind.as_str(),
            seen[0].target.as_str(),
            seen[0].by.as_str()
        ),
        (tab.as_str(), "browser", "https://example.com/a?b=c", "shell")
    );
    assert!(waiting(&st).is_empty());
}

#[cfg(not(windows))]
#[tokio::test]
async fn open_an_address_says_pages_are_not_here_yet() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let (status, text) = open(&app, &tab, "https://example.com/", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(text, "Browser tabs aren't available on this system yet.");
    assert!(waiting(&st).is_empty());
}

#[tokio::test]
async fn open_a_file_queues_a_file_target() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let seen = fake_app(&st, &app, None);
    let file = a_file(tmp.path(), "a b.md");
    assert_eq!(open(&app, &tab, &file, None).await.0, StatusCode::NO_CONTENT);
    let seen = seen.lock().unwrap();
    assert_eq!(
        (seen[0].kind.as_str(), seen[0].target.as_str(), seen[0].by.as_str()),
        ("file", file.as_str(), "shell")
    );
}

#[tokio::test]
async fn open_names_the_callers_agent() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    // Pinned to scratch values, so no test runs a real `claude` or reads the user's settings.
    let st = state(&home)
        .with_claude_version(Some("2.1.302".into()))
        .with_claude_compact_paths(home.join("user"), Vec::new())
        .with_claude_settings(home.join("claude-settings.json"))
        .with_agent_check(|_| true)
        .with_tab_path(|| None);
    let app = router(st.clone());
    let shell = start(&app, tmp.path()).await;
    let agent = start_with(&app, tmp.path(), Some("claude")).await;
    let seen = fake_app(&st, &app, None);
    let file = a_file(tmp.path(), "a.md");
    // The agent opens a tab in its own pane, then in the shell's: the mark is the caller's either way.
    assert_eq!(open(&app, &agent, &file, Some(&agent)).await.0, StatusCode::NO_CONTENT);
    assert_eq!(open(&app, &shell, &file, Some(&agent)).await.0, StatusCode::NO_CONTENT);
    // And a shell that opens one in the agent's pane is a shell.
    assert_eq!(open(&app, &agent, &file, Some(&shell)).await.0, StatusCode::NO_CONTENT);
    let seen = seen.lock().unwrap();
    let by: Vec<_> = seen.iter().map(|o| (o.tab_id.as_str(), o.by.as_str())).collect();
    assert_eq!(
        by,
        [
            (agent.as_str(), "claude"),
            (shell.as_str(), "claude"),
            (agent.as_str(), "shell")
        ]
    );
}

#[tokio::test]
async fn open_from_an_unknown_tab_is_the_shell() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let seen = fake_app(&st, &app, None);
    let file = a_file(tmp.path(), "a.md");
    for from in [Some("nope"), Some(""), None] {
        assert_eq!(open(&app, &tab, &file, from).await.0, StatusCode::NO_CONTENT);
    }
    assert!(seen.lock().unwrap().iter().all(|o| o.by == "shell"));
    assert_eq!(seen.lock().unwrap().len(), 3);
}

#[cfg(windows)]
#[tokio::test]
async fn open_refuses_a_bad_address() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let long = format!("https://example.com/{}", "a".repeat(5000));
    for bad in [
        "http://",
        "https:///x",
        "http://:80/",
        "http://user:pw@h/",
        "http://@h/",
        "https://exa mple.com/",
        "https://example.com/a b",
        "https://example.com/\u{7}",
        "https://example.com/a\nb",
        "HTTP://user@EXAMPLE.COM/",
        long.as_str(),
    ] {
        let (status, text) = open(&app, &tab, bad, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}");
        assert_eq!(text, "That address can't be opened in the side pane.", "{bad:?}");
    }
    assert!(waiting(&st).is_empty());
    // Only the scheme makes an address: these wait as addresses, whatever the app then makes of them.
    for good in [
        "HTTP://EXAMPLE.COM/",
        "http://localhost:3000",
        "https://example.com/a?b=c@d",
        "http://[::1]:3000/",
    ] {
        assert_eq!(open(&app, &tab, good, None).await.0, StatusCode::ACCEPTED, "{good:?}");
    }
    let kinds: Vec<_> = waiting(&st).into_iter().map(|o| o.kind).collect();
    assert_eq!(kinds, ["browser"; 4]);
}

#[tokio::test]
async fn open_refuses_a_bad_path() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let long = tmp.path().join("a".repeat(5000)).to_string_lossy().into_owned();
    for bad in [
        "",
        "a.md",
        "..\\x.md",
        "example.com",
        "localhost:3000",
        "javascript:alert(1)",
        "file:///C:/x",
        r"\\server\share\x.md",
        "//server/share/x.md",
        r"\\.\COM1",
        r"\\?\C:\x.md",
        long.as_str(),
    ] {
        let (status, text) = open(&app, &tab, bad, None).await;
        assert_eq!((status, text.as_str()), (StatusCode::BAD_REQUEST, BAD_PATH), "{bad:?}");
    }
    let missing = tmp.path().join("missing.md").to_string_lossy().into_owned();
    let (status, text) = open(&app, &tab, &missing, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(text, format!("No file {missing}."));
    let folder = tmp.path().to_string_lossy().into_owned();
    let (status, text) = open(&app, &tab, &folder, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(text, format!("{folder} is a folder, not a file."));
    assert!(waiting(&st).is_empty());
}

#[tokio::test]
async fn open_for_no_such_session_is_a_404() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let (status, text) = open(&app, "nope", &a_file(tmp.path(), "a.md"), None).await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, "No such session."));
    assert!(waiting(&st).is_empty());
}

#[tokio::test]
async fn open_answers_the_apps_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    let file = a_file(tmp.path(), "a.md");
    for (refused, line) in [
        ("limit", "Session 1 already has 32 tabs in its side pane."),
        ("address", "That address can't be opened in the side pane."),
    ] {
        let st = state(&tmp.path().join(refused));
        let app = router(st.clone());
        let tab = start(&app, tmp.path()).await;
        fake_app(&st, &app, Some(refused));
        let (status, text) = open(&app, &tab, &file, None).await;
        assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, line));
        assert!(waiting(&st).is_empty());
    }
}

#[tokio::test]
async fn open_waits_five_seconds_then_answers_202() {
    let tmp = tempfile::tempdir().unwrap();
    // The wait is five seconds unless a test says otherwise.
    assert_eq!(state(&tmp.path().join("other")).open_wait(), Duration::from_secs(5));
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let file = a_file(tmp.path(), "a.md");
    let (status, text) = open(&app, &tab, &file, None).await;
    assert_eq!((status, text.as_str()), (StatusCode::ACCEPTED, LATE));
    // Nobody is waiting for the answer any more, and the request is still there for the app.
    let still = waiting(&st);
    assert_eq!(still.len(), 1);
    assert_eq!(still[0].target, file);
    // The app's late answer takes it off the queue.
    assert_eq!(
        done(&app, serde_json::json!([{ "id": still[0].id }])).await,
        StatusCode::NO_CONTENT
    );
    assert!(waiting(&st).is_empty());
}

#[tokio::test]
async fn the_65th_waiting_request_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(1));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let file = a_file(tmp.path(), "a.md");
    for n in 0..64 {
        assert_eq!(open(&app, &tab, &file, None).await.0, StatusCode::ACCEPTED, "{n}");
    }
    let (status, text) = open(&app, &tab, &file, None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        text,
        "Too many tabs are waiting to open. Try again when Marshell's window is back."
    );
    assert_eq!(waiting(&st).len(), 64);
    // Each has its own id.
    let mut ids: Vec<_> = waiting(&st).iter().map(|o| o.id).collect();
    ids.dedup();
    assert_eq!(ids.len(), 64);
}

#[tokio::test]
async fn closing_a_session_drops_its_requests() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let closing = start(&app, tmp.path()).await;
    let staying = start(&app, tmp.path()).await;
    let file = a_file(tmp.path(), "a.md");
    let ask = |tab: &str| {
        let (app, tab, file) = (app.clone(), tab.to_string(), file.clone());
        tokio::spawn(async move { open(&app, &tab, &file, None).await })
    };
    let (first, second) = (ask(&closing), ask(&staying));
    let mut rx = st.opens().subscribe();
    tokio::time::timeout(Duration::from_secs(10), async {
        while rx.borrow_and_update().len() < 2 {
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("both requests should be waiting");
    let res = app
        .clone()
        .oneshot(api("DELETE", &format!("/v1/sessions/{closing}"), true, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let (status, text) = first.await.unwrap();
    assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, "Session 1 has closed."));
    // The other session's request is still waiting.
    let left = waiting(&st);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].tab_id, staying);
    assert_eq!(
        done(&app, serde_json::json!([{ "id": left[0].id }])).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(second.await.unwrap().0, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn done_twice_is_harmless() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    let file = a_file(tmp.path(), "a.md");
    assert_eq!(open(&app, &tab, &file, None).await.0, StatusCode::ACCEPTED);
    assert_eq!(open(&app, &tab, &file, None).await.0, StatusCode::ACCEPTED);
    let ids: Vec<_> = waiting(&st).iter().map(|o| o.id).collect();
    for _ in 0..2 {
        assert_eq!(
            done(
                &app,
                serde_json::json!([{ "id": ids[0] }, { "id": ids[0], "refused": "limit" }])
            )
            .await,
            StatusCode::NO_CONTENT
        );
        // The other request is not the one that was answered.
        assert_eq!(waiting(&st).iter().map(|o| o.id).collect::<Vec<_>>(), [ids[1]]);
    }
}

#[tokio::test]
async fn done_for_an_unknown_id_is_harmless() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home")).with_open_wait(Duration::from_millis(50));
    let app = router(st.clone());
    let tab = start(&app, tmp.path()).await;
    assert_eq!(
        open(&app, &tab, &a_file(tmp.path(), "a.md"), None).await.0,
        StatusCode::ACCEPTED
    );
    assert_eq!(
        done(&app, serde_json::json!([{ "id": 4_000_000_000_u64 }, { "id": 0 }])).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(done(&app, serde_json::json!([])).await, StatusCode::NO_CONTENT);
    assert_eq!(waiting(&st).len(), 1);
}
