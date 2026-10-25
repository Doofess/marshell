//! The notifier end to end: hooks and the UI's view in, requests for the app shell out.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::notify::{self, NotifyRequest, NotifySink, SoundKind};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{NotifyChannel, Settings, UiView};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5556;

type Got = Arc<Mutex<Vec<NotifyRequest>>>;

/// A core with two open Claude rows (`t1`, `t2`, named after their tabs) and a notifier collecting into `Got`.
fn core() -> (axum::Router, AppState, Got, tokio::task::JoinHandle<()>) {
    let home = tempfile::tempdir().unwrap().keep();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_version(Some("2.1.302".into()))
    .with_permission_hold(Duration::from_secs(30));
    for tab in ["t1", "t2"] {
        st.registry().reserve(
            tab,
            marshell_core::registry::Relaunch {
                profile_id: "none".into(),
                agent: None,
                cwd: String::new(),
                mode: None,
                name: tab.into(),
                preset: None,
            },
        );
    }
    let got: Got = Arc::default();
    let sink: NotifySink = {
        let got = got.clone();
        Arc::new(move |req| got.lock().unwrap().push(req))
    };
    let notifier = notify::spawn(st.clone(), sink);
    (router(st.clone()), st, got, notifier)
}

fn base(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

async fn send(app: &axum::Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

async fn put_view(app: &axum::Router, focused: bool) {
    let view = UiView {
        window_focused: focused,
        visible_tabs: vec!["t1".into()],
    };
    let req = base("PUT", "/v1/ui/view")
        .body(Body::from(serde_json::to_vec(&view).unwrap()))
        .unwrap();
    assert_eq!(send(app, req).await, StatusCode::NO_CONTENT);
}

/// A permission request for `tab`, held until the test ends.
fn ask(app: &axum::Router, tab: &str) -> tokio::task::JoinHandle<StatusCode> {
    let body = serde_json::json!({
        "hook_event_name": "PermissionRequest",
        "tool_name": "Bash",
        "tool_input": { "command": "npm test" },
    });
    let req = base("POST", "/v1/hook")
        .header("x-marshell-role", "permission")
        .header("x-marshell-agent", "claude")
        .header("x-marshell-version", "0.0.0")
        .header("x-marshell-tab", tab)
        .body(Body::from(body.to_string()))
        .unwrap();
    let app = app.clone();
    tokio::spawn(async move { send(&app, req).await })
}

async fn wait_for(got: &Got, count: usize, within: Duration) -> Vec<NotifyRequest> {
    let deadline = Instant::now() + within;
    loop {
        let now = got.lock().unwrap().clone();
        if now.len() >= count || Instant::now() >= deadline {
            return now;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_request_in_a_background_window_asks_for_a_banner_a_sound_and_a_flash() {
    let (app, _st, got, notifier) = core();
    put_view(&app, false).await;
    // Let the notifier take its first look, which only records.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let hook = ask(&app, "t1");
    let seen = wait_for(&got, 1, Duration::from_secs(1)).await;
    assert_eq!(seen.len(), 1, "got {seen:?}");
    let banner = seen[0].banner.as_ref().expect("a banner");
    assert_eq!(banner.title, "t1 needs you");
    assert_eq!(banner.body, "Wants to run `npm test`");
    assert_eq!(banner.tab_id.as_deref(), Some("t1"));
    assert_eq!(seen[0].sound, Some(SoundKind::NeedsYou));
    assert!(seen[0].flash);

    // The window comes to the front: the next tab needing you says nothing.
    put_view(&app, true).await;
    let second = ask(&app, "t2");
    let seen = wait_for(&got, 2, Duration::from_millis(800)).await;
    assert_eq!(seen.len(), 1, "got {seen:?}");
    hook.abort();
    second.abort();
    notifier.abort();
}

#[tokio::test]
async fn stop_failure_raises_the_error_banner() {
    let (app, _st, got, notifier) = core();
    put_view(&app, false).await;
    // Let the notifier take its first look, which only records.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let body = std::fs::read(format!(
        "{}/tests/fixtures/claude/2.1.309/StopFailure.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let req = base("POST", "/v1/hook")
        .header("x-marshell-role", "event")
        .header("x-marshell-agent", "claude")
        .header("x-marshell-version", "0.0.0")
        .header("x-marshell-tab", "t1")
        .body(Body::from(body))
        .unwrap();
    assert_eq!(send(&app, req).await, StatusCode::NO_CONTENT);
    let seen = wait_for(&got, 1, Duration::from_secs(1)).await;
    assert_eq!(seen.len(), 1, "got {seen:?}");
    assert_eq!(seen[0].sound, Some(SoundKind::Error));
    let banner = seen[0].banner.as_ref().expect("a banner");
    assert_eq!(banner.title, "t1 stopped");
    assert_eq!(banner.body, "Stopped: Sign-in failed.");
    notifier.abort();
}

#[tokio::test]
async fn a_focus_reported_after_the_first_look_silences_the_next_request() {
    let (app, _st, got, notifier) = core();
    put_view(&app, false).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    // The notifier is idle between looks, so this report is what wakes it.
    put_view(&app, true).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let hook = ask(&app, "t1");
    let seen = wait_for(&got, 1, Duration::from_millis(800)).await;
    assert!(seen.is_empty(), "got {seen:?}");
    hook.abort();
    notifier.abort();
}

#[tokio::test]
async fn the_switches_are_read_from_memory_on_every_look() {
    let (app, _st, got, notifier) = core();
    put_view(&app, false).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut settings = Settings::default();
    settings.notifications.needs_you = NotifyChannel {
        banner: false,
        sound: false,
    };
    let put = base("PUT", "/v1/settings")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&settings).unwrap()))
        .unwrap();
    assert_eq!(send(&app, put).await, StatusCode::OK);
    let hook = ask(&app, "t1");
    let seen = wait_for(&got, 1, Duration::from_millis(800)).await;
    assert!(seen.is_empty(), "got {seen:?}");
    hook.abort();
    notifier.abort();
}
