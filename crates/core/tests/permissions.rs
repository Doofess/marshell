mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;

fn app(hold: Duration) -> (axum::Router, AppState) {
    app_on(PORT, hold)
}

fn app_on(port: u16, hold: Duration) -> (axum::Router, AppState) {
    app_with_codex(port, hold, "0.163.0")
}

/// An app whose Codex version is pinned, so no test runs the `codex --version` on this machine.
fn app_with_codex(port: u16, hold: Duration, codex_version: &str) -> (axum::Router, AppState) {
    let home = tempfile::tempdir().unwrap().keep();
    let st = AppState::new(
        TOKEN.into(),
        port,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_version(Some("2.1.302".into()))
    .with_codex_version(Some(codex_version.into()))
    .with_agent_path(|_| None)
    .with_permission_hold(hold);
    // A row with no process: enough for a request to name an open tab.
    for tab in ["t0", "t1", "t2"] {
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
    (router(st.clone()), st)
}

fn base(method: &str, path: &str) -> axum::http::request::Builder {
    base_on(PORT, method, path)
}

fn base_on(port: u16, method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{port}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

fn hook_request(agent: &str, tab: Option<&str>, body: &serde_json::Value) -> Request<Body> {
    hook_request_on(PORT, agent, tab, body)
}

fn hook_request_on(port: u16, agent: &str, tab: Option<&str>, body: &serde_json::Value) -> Request<Body> {
    let mut b = base_on(port, "POST", "/v1/hook")
        .header("x-marshell-role", "permission")
        .header("x-marshell-agent", agent)
        .header("x-marshell-version", "0.0.0");
    if let Some(t) = tab {
        b = b.header("x-marshell-tab", t);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

type Answer = (StatusCode, Option<String>, Vec<u8>);

fn spawn_hook(app: &axum::Router, tab: Option<&str>, body: serde_json::Value) -> tokio::task::JoinHandle<Answer> {
    spawn_hook_on(PORT, app, tab, body)
}

fn spawn_hook_on(
    port: u16,
    app: &axum::Router,
    tab: Option<&str>,
    body: serde_json::Value,
) -> tokio::task::JoinHandle<Answer> {
    spawn_agent_hook_on(port, "claude", app, tab, body)
}

fn spawn_agent_hook(
    agent: &str,
    app: &axum::Router,
    tab: Option<&str>,
    body: serde_json::Value,
) -> tokio::task::JoinHandle<Answer> {
    spawn_agent_hook_on(PORT, agent, app, tab, body)
}

fn spawn_agent_hook_on(
    port: u16,
    agent: &str,
    app: &axum::Router,
    tab: Option<&str>,
    body: serde_json::Value,
) -> tokio::task::JoinHandle<Answer> {
    let app = app.clone();
    let req = hook_request_on(port, agent, tab, &body);
    tokio::spawn(async move {
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let ct = res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_string());
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, ct, bytes.to_vec())
    })
}

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn pending(app: &axum::Router) -> Vec<serde_json::Value> {
    let res = app
        .clone()
        .oneshot(base("GET", "/v1/permissions").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    match json_of(res).await {
        serde_json::Value::Array(a) => a,
        other => panic!("not an array: {other}"),
    }
}

/// Polls until `want` says the list is right, or fails after 5 s.
async fn wait_pending(app: &axum::Router, want: impl Fn(&[serde_json::Value]) -> bool) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let got = pending(app).await;
        if want(&got) {
            return got;
        }
        assert!(Instant::now() < deadline, "timed out; pending is {got:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_one(app: &axum::Router) -> serde_json::Value {
    wait_pending(app, |p| p.len() == 1).await.remove(0)
}

async fn post_to(app: &axum::Router, path: &str, body: &str) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(base("POST", path).body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn bash_request() -> serde_json::Value {
    serde_json::json!({"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"npm test"}})
}

fn rid_of(entry: &serde_json::Value) -> String {
    entry["rid"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn decision_answers_the_held_hook() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t1"), bash_request());
    let entry = wait_one(&app).await;
    assert_eq!(entry["tool"], "Bash");
    assert_eq!(entry["tab"], "t1");
    assert_eq!(entry["agent"], "claude");
    assert_eq!(entry["request"]["tool_input"]["command"], "npm test");
    let (status, _) = post_to(
        &app,
        &format!("/v1/permissions/{}/decision", rid_of(&entry)),
        r#"{"behavior":"allow"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, ct, body) = hook.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ct.as_deref(), Some("application/json"));
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}})
    );
    assert!(pending(&app).await.is_empty());
}

fn codex_request(command: &str) -> serde_json::Value {
    serde_json::json!({"hook_event_name":"PermissionRequest","tool_name":"Bash",
        "tool_input":{"command":command,"description":"Allow running the requested command?"}})
}

fn codex_answer(decision: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":decision}})
}

fn answer_json(answer: &Answer) -> serde_json::Value {
    serde_json::from_slice(&answer.2).unwrap()
}

#[tokio::test]
async fn a_codex_permission_is_held_and_answered() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_agent_hook("codex", &app, Some("t1"), codex_request("Set-Content out.txt hi"));
    let entry = wait_one(&app).await;
    assert_eq!(
        (entry["agent"].as_str(), entry["tab"].as_str()),
        (Some("codex"), Some("t1"))
    );
    let (status, _) = post_to(
        &app,
        &format!("/v1/permissions/{}/decision", rid_of(&entry)),
        r#"{"behavior":"allow"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let answer = hook.await.unwrap();
    assert_eq!(answer.0, StatusCode::OK);
    assert_eq!(
        String::from_utf8(answer.2).unwrap(),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
    );
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn a_codex_permission_shows_a_card_with_no_always_rule() {
    let (app, st) = app(Duration::from_secs(30));
    let hook = spawn_agent_hook("codex", &app, Some("t1"), codex_request("Set-Content out.txt hi"));
    wait_one(&app).await;
    let views = st.permission_views();
    assert_eq!(views.len(), 1);
    assert_eq!((views[0].agent.as_str(), views[0].tab.as_str()), ("codex", "t1"));
    assert_eq!(views[0].always_rule, None);
    assert_eq!(
        views[0].detail,
        marshell_protocol::hook::PermissionDetail::Bash {
            command: "Set-Content out.txt hi".into(),
            cwd: None
        }
    );
    hook.abort();
}

#[tokio::test]
async fn a_codex_project_decision_is_409() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_agent_hook("codex", &app, Some("t0"), codex_request("npm test"));
    let rid = rid_of(&wait_one(&app).await);
    let (status, text) = post_to(
        &app,
        &format!("/v1/permissions/{rid}/decision"),
        r#"{"behavior":"allow","scope":"project"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        text,
        "Codex can't keep an always-allow rule from Marshell. Allow once, or answer in the terminal."
    );
    assert_eq!(pending(&app).await.len(), 1);
    assert!(!hook.is_finished());
    hook.abort();
}

/// An unrecorded Codex version only limits the row's status note; the card and the answer still work.
#[tokio::test]
async fn a_codex_permission_is_answered_on_an_unrecorded_version() {
    let (app, st) = app_with_codex(PORT, Duration::from_secs(30), "0.170.0");
    let hook = spawn_agent_hook("codex", &app, Some("t1"), codex_request("Set-Content out.txt hi"));
    let entry = wait_one(&app).await;
    assert_eq!(
        (entry["agent"].as_str(), entry["tab"].as_str()),
        (Some("codex"), Some("t1"))
    );
    assert_eq!(st.permission_views().len(), 1);
    let (status, _) = post_to(
        &app,
        &format!("/v1/permissions/{}/decision", rid_of(&entry)),
        r#"{"behavior":"allow"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let answer = hook.await.unwrap();
    assert_eq!(answer.0, StatusCode::OK);
    assert_eq!(
        answer_json(&answer),
        codex_answer(serde_json::json!({"behavior":"allow"}))
    );
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn two_codex_tabs_get_their_own_answers() {
    let (app, _) = app(Duration::from_secs(30));
    let first = spawn_agent_hook("codex", &app, Some("t1"), codex_request("first command"));
    wait_pending(&app, |p| p.len() == 1).await;
    let second = spawn_agent_hook("codex", &app, Some("t2"), codex_request("second command"));
    let entries = wait_pending(&app, |p| p.len() == 2).await;
    let rid_for = |tab: &str| rid_of(entries.iter().find(|e| e["tab"] == tab).unwrap());
    // Answer the later tab first, with a different decision from the earlier one.
    let (status, _) = post_to(
        &app,
        &format!("/v1/permissions/{}/decision", rid_for("t2")),
        r#"{"behavior":"deny","message":"second only"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let second = second.await.unwrap();
    assert_eq!(
        answer_json(&second),
        codex_answer(serde_json::json!({"behavior":"deny","message":"second only"}))
    );
    // The first tab is still waiting, and its request is the only one left.
    assert!(!first.is_finished());
    let left = pending(&app).await;
    assert_eq!((left.len(), left[0]["tab"].as_str()), (1, Some("t1")));
    let (status, _) = post_to(
        &app,
        &format!("/v1/permissions/{}/decision", rid_for("t1")),
        r#"{"behavior":"allow"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        answer_json(&first.await.unwrap()),
        codex_answer(serde_json::json!({"behavior":"allow"}))
    );
}

#[tokio::test]
async fn no_decision_returns_204() {
    let (app, _) = app(Duration::from_millis(100));
    let hook = spawn_agent_hook("codex", &app, Some("t1"), codex_request("npm test"));
    let (status, ct, body) = hook.await.unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty(), "{ct:?}");
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn release_answers_204_with_no_body() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let entry = wait_one(&app).await;
    let (status, _) = post_to(&app, &format!("/v1/permissions/{}/release", rid_of(&entry)), "").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, body) = hook.await.unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty());
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn hold_expiry_releases() {
    let (app, _) = app(Duration::from_millis(200));
    let started = Instant::now();
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let (status, _, body) = tokio::time::timeout(Duration::from_secs(1), hook)
        .await
        .expect("the hold did not expire in 1 s")
        .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty());
    assert!(started.elapsed() >= Duration::from_millis(150));
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn second_decision_gets_404() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    let path = format!("/v1/permissions/{rid}/decision");
    assert_eq!(
        post_to(&app, &path, r#"{"behavior":"deny"}"#).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        post_to(&app, &path, r#"{"behavior":"allow"}"#).await.0,
        StatusCode::NOT_FOUND
    );
    // The first decision is the one the hook got.
    let (_, _, body) = hook.await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["hookSpecificOutput"]["decision"]["behavior"], "deny");

    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    assert_eq!(
        post_to(&app, &format!("/v1/permissions/{rid}/release"), "").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        post_to(
            &app,
            &format!("/v1/permissions/{rid}/decision"),
            r#"{"behavior":"allow"}"#
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(hook.await.unwrap().0, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn racing_decisions_have_exactly_one_winner() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    let path = format!("/v1/permissions/{rid}/decision");
    let (a, b) = tokio::join!(
        post_to(&app, &path, r#"{"behavior":"allow"}"#),
        post_to(&app, &path, r#"{"behavior":"deny"}"#),
    );
    let mut codes = [a.0, b.0];
    codes.sort();
    assert_eq!(codes, [StatusCode::NO_CONTENT, StatusCode::NOT_FOUND]);
    assert_eq!(hook.await.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn dropped_bridge_removes_the_request() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    hook.abort();
    let _ = hook.await;
    let start = Instant::now();
    wait_pending(&app, |p| p.is_empty()).await;
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(
        post_to(
            &app,
            &format!("/v1/permissions/{rid}/decision"),
            r#"{"behavior":"allow"}"#
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn typing_in_the_tab_releases_its_requests() {
    let (app, st) = app(Duration::from_secs(30));
    let mine = spawn_hook(&app, Some("t1"), bash_request());
    let other = spawn_hook(&app, Some("t2"), bash_request());
    wait_pending(&app, |p| p.len() == 2).await;
    st.holds().release_tab("t1");
    assert_eq!(mine.await.unwrap().0, StatusCode::NO_CONTENT);
    let left = wait_one(&app).await;
    assert_eq!(left["tab"], "t2");
    assert!(!other.is_finished());
    other.abort();
}

#[tokio::test]
async fn scoped_allow_without_a_rule_is_409() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    let (status, text) = post_to(
        &app,
        &format!("/v1/permissions/{rid}/decision"),
        r#"{"behavior":"allow","scope":"session"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(text, "This request has no rule to add.");
    assert_eq!(pending(&app).await.len(), 1);
    assert!(!hook.is_finished());
    hook.abort();
}

#[tokio::test]
async fn unsupported_agent_decision_is_409_and_stays_pending() {
    let (app, st) = app(Duration::from_secs(30));
    let (_hold, _rx) = st.holds().open(Some("t0".into()), "grok".into(), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    let (status, text) = post_to(
        &app,
        &format!("/v1/permissions/{rid}/decision"),
        r#"{"behavior":"allow"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(text, "This agent can't take a decision yet.");
    assert_eq!(pending(&app).await.len(), 1);
}

#[tokio::test]
async fn permission_without_a_view_is_answered_at_once() {
    let (app, _) = app(Duration::from_secs(120));
    let req = hook_request("antigravity", Some("t0"), &bash_request());
    let response = tokio::time::timeout(Duration::from_secs(1), app.clone().oneshot(req))
        .await
        .expect("a request without a card was held")
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn unknown_rid_is_404_and_bad_body_is_400() {
    let (app, _) = app(Duration::from_secs(30));
    assert_eq!(
        post_to(&app, "/v1/permissions/nope/decision", r#"{"behavior":"allow"}"#)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post_to(&app, "/v1/permissions/nope/release", "").await.0,
        StatusCode::NOT_FOUND
    );
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    let path = format!("/v1/permissions/{rid}/decision");
    assert_eq!(post_to(&app, &path, "not json").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        post_to(&app, &path, r#"{"behavior":"maybe"}"#).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(pending(&app).await.len(), 1);
    hook.abort();
}

#[tokio::test]
async fn a_permission_request_is_also_logged() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t0"), bash_request());
    wait_one(&app).await;
    let res = app
        .clone()
        .oneshot(base("GET", "/v1/hook/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let events = json_of(res).await;
    assert_eq!(events.as_array().unwrap().len(), 1);
    assert_eq!(events[0]["role"], "permission");
    hook.abort();
}

#[tokio::test]
async fn permissions_require_the_token() {
    let (app, _) = app(Duration::from_secs(30));
    let req = Request::builder()
        .uri("/v1/permissions")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

/// A router whose one profile runs the fake agent asleep, and a session started in it.
async fn app_with_session(hold: Duration) -> (axum::Router, String, tempfile::TempDir) {
    common::tie_children_to_test();
    use marshell_protocol::api::{Profile, ShellFamily};
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap().keep();
    let sleeper = Profile {
        id: "sleeper".into(),
        name: "sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: false,
        terminal_theme: None,
        detected: true,
    };
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        vec![sleeper],
    )
    .with_claude_version(Some("2.1.302".into()))
    .with_permission_hold(hold);
    let app = router(st);
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": dir.path().to_string_lossy(), "agent": null,
    });
    let (status, text) = post_json(&app, "/v1/sessions", &body).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    (app, json["tab_id"].as_str().unwrap().to_string(), dir)
}

async fn post_json(app: &axum::Router, path: &str, body: &serde_json::Value) -> (StatusCode, String) {
    let req = base("POST", path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// Holds a request for `tab`, sends `body` to `path`, and checks the send was accepted and released the hold.
async fn assert_releases(app: &axum::Router, tab: &str, path: &str, body: serde_json::Value) {
    let hook = spawn_hook(app, Some(tab), bash_request());
    wait_one(app).await;
    let (status, text) = post_json(app, path, &body).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{path}: {text}");
    let answer = tokio::time::timeout(Duration::from_secs(5), hook)
        .await
        .unwrap_or_else(|_| panic!("{path} left the request held"))
        .unwrap();
    assert_eq!(answer.0, StatusCode::NO_CONTENT, "{path}");
    wait_pending(app, |p| p.is_empty()).await;
}

#[tokio::test]
async fn text_typed_through_the_api_releases_the_tab() {
    let (app, tab, _dir) = app_with_session(Duration::from_secs(30)).await;
    let path = format!("/v1/sessions/{tab}/input");
    assert_releases(&app, &tab, &path, serde_json::json!({ "text": "4" })).await;
}

#[tokio::test]
async fn dropped_paths_release_the_tab() {
    let (app, tab, dir) = app_with_session(Duration::from_secs(30)).await;
    let file = dir.path().join("dropped.txt");
    std::fs::write(&file, b"x").unwrap();
    let path = format!("/v1/sessions/{tab}/paste-paths");
    let body = serde_json::json!({ "paths": [file.to_string_lossy()] });
    assert_releases(&app, &tab, &path, body).await;
}

#[tokio::test]
async fn a_pasted_attachment_releases_the_tab() {
    let (app, tab, _dir) = app_with_session(Duration::from_secs(30)).await;
    let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, b'r', b'e', b's', b't'];
    let req = base("POST", &format!("/v1/sessions/{tab}/attachments"))
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(png.to_vec()))
        .unwrap();
    let saved = json_of(app.clone().oneshot(req).await.unwrap()).await;
    let name = saved["name"].as_str().unwrap();
    let path = format!("/v1/sessions/{tab}/paste");
    assert_releases(&app, &tab, &path, serde_json::json!({ "attachment": name, "text": "" })).await;
}

#[tokio::test]
async fn no_tab_permission_is_answered_at_once() {
    let (app, _) = app(Duration::from_secs(120));
    let hook = spawn_hook(&app, None, bash_request());
    let (status, _, body) = tokio::time::timeout(Duration::from_secs(1), hook)
        .await
        .expect("a request with no tab was held")
        .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty());
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn closed_tab_permission_is_answered_at_once() {
    let (app, _) = app(Duration::from_secs(120));
    let hook = spawn_hook(&app, Some("no-such-tab"), bash_request());
    let (status, _, body) = tokio::time::timeout(Duration::from_secs(1), hook)
        .await
        .expect("a request for a tab that is not open was held")
        .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty());
    assert!(pending(&app).await.is_empty());
}

fn live_of(st: &AppState, tab: &str) -> marshell_protocol::api::LiveStatus {
    let row = st.registry().snapshot().sessions.into_iter().find(|s| s.tab_id == tab);
    row.unwrap().live.unwrap()
}

#[tokio::test]
async fn decision_sets_working_thinking() {
    use marshell_protocol::api::LiveState;
    for behavior in ["allow", "deny"] {
        let (app, st) = app(Duration::from_secs(30));
        let hook = spawn_hook(&app, Some("t1"), bash_request());
        let rid = rid_of(&wait_one(&app).await);
        assert_eq!(live_of(&st, "t1").state, LiveState::NeedsPermission, "{behavior}");
        let body = format!(r#"{{"behavior":"{behavior}"}}"#);
        let (status, _) = post_to(&app, &format!("/v1/permissions/{rid}/decision"), &body).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let after = live_of(&st, "t1");
        assert_eq!(
            (after.state, after.phrase.as_str()),
            (LiveState::Working, "Thinking"),
            "{behavior}"
        );
        assert_eq!(hook.await.unwrap().0, StatusCode::OK);
    }
}

#[tokio::test]
async fn deciding_one_request_keeps_the_tab_waiting_for_the_other() {
    use marshell_protocol::api::LiveState;
    let (app, st) = app(Duration::from_secs(30));
    let first = spawn_hook(&app, Some("t1"), bash_request());
    let first_rid = rid_of(&wait_one(&app).await);
    let second = spawn_hook(&app, Some("t1"), bash_request());
    let entries = wait_pending(&app, |p| p.len() == 2).await;
    let second_rid = entries.iter().map(rid_of).find(|rid| rid != &first_rid).unwrap();
    for (rid, expected) in [
        (first_rid, LiveState::NeedsPermission),
        (second_rid, LiveState::Working),
    ] {
        let (status, _) = post_to(
            &app,
            &format!("/v1/permissions/{rid}/decision"),
            r#"{"behavior":"allow"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(live_of(&st, "t1").state, expected);
    }
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    assert_eq!(second.await.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn release_leaves_the_status_alone() {
    use marshell_protocol::api::LiveState;
    let (app, st) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, Some("t1"), bash_request());
    let rid = rid_of(&wait_one(&app).await);
    let (status, _) = post_to(&app, &format!("/v1/permissions/{rid}/release"), "").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(hook.await.unwrap().0, StatusCode::NO_CONTENT);
    assert_eq!(live_of(&st, "t1").state, LiveState::NeedsPermission);
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The next text frame of `/v1/events`, parsed.
async fn next_frame(ws: &mut Ws) -> serde_json::Value {
    use futures_util::StreamExt;
    let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .expect("no frame within 10 s")
        .expect("socket ended")
        .expect("socket error");
    match msg {
        tokio_tungstenite::tungstenite::Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

/// The data of the next `permissions` frame, skipping `sessions` frames.
async fn next_permissions(ws: &mut Ws) -> Vec<serde_json::Value> {
    loop {
        let frame = next_frame(ws).await;
        if frame["type"] == "permissions" {
            return frame["data"].as_array().unwrap().clone();
        }
        assert_eq!(frame["type"], "sessions");
    }
}

#[tokio::test]
async fn events_socket_sends_permissions_on_open_and_change() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (app, _) = app_on(port, Duration::from_secs(30));
    tokio::spawn(axum::serve(listener, app.clone()).into_future());

    let mut req = format!("ws://127.0.0.1:{port}/v1/events")
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{TOKEN}").parse().unwrap(),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    // The sessions snapshot comes first, then the (empty) permission list.
    assert_eq!(next_frame(&mut ws).await["type"], "sessions");
    let second = next_frame(&mut ws).await;
    assert_eq!(second["type"], "permissions");
    assert_eq!(second["data"], serde_json::json!([]));

    let hook = spawn_hook_on(port, &app, Some("t1"), bash_request());
    let views = loop {
        let views = next_permissions(&mut ws).await;
        if !views.is_empty() {
            break views;
        }
    };
    assert_eq!(views.len(), 1);
    assert_eq!(views[0]["tab"], "t1");
    assert_eq!(views[0]["agent"], "claude");
    assert_eq!(views[0]["detail"]["kind"], "bash");
    assert_eq!(views[0]["detail"]["command"], "npm test");
    let rid = views[0]["rid"].as_str().unwrap().to_string();

    let path = format!("/v1/permissions/{rid}/decision");
    let decide = base_on(port, "POST", &path)
        .body(Body::from(r#"{"behavior":"allow"}"#))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(decide).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(next_permissions(&mut ws).await, Vec::<serde_json::Value>::new());
    assert_eq!(hook.await.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn decision_sends_sessions_before_permissions() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (app, _) = app_on(port, Duration::from_secs(30));
    tokio::spawn(axum::serve(listener, app.clone()).into_future());
    let mut req = format!("ws://127.0.0.1:{port}/v1/events")
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{TOKEN}").parse().unwrap(),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(next_frame(&mut ws).await["type"], "sessions");
    assert_eq!(next_frame(&mut ws).await["type"], "permissions");

    // Both channels can be ready together; exercise that choice across repeated decisions.
    for _ in 0..32 {
        let hook = spawn_hook_on(port, &app, Some("t1"), bash_request());
        let (mut waiting, mut rid) = (false, None);
        while !waiting || rid.is_none() {
            let frame = next_frame(&mut ws).await;
            match frame["type"].as_str().unwrap() {
                "sessions" => {
                    let row = frame["data"]["sessions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["tab_id"] == "t1")
                        .unwrap();
                    waiting = row["live"]["state"] == "needs_permission";
                }
                "permissions" => rid = Some(rid_of(&frame["data"][0])),
                other => panic!("unexpected frame: {other}"),
            }
        }
        let path = format!("/v1/permissions/{}/decision", rid.unwrap());
        let decide = base_on(port, "POST", &path)
            .body(Body::from(r#"{"behavior":"allow"}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(decide).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );
        let frame = next_frame(&mut ws).await;
        assert_eq!(
            frame["type"], "sessions",
            "permission removal arrived before the live status"
        );
        let row = frame["data"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tab_id"] == "t1")
            .unwrap();
        assert_eq!(row["live"]["state"], "working");
        let frame = next_frame(&mut ws).await;
        assert_eq!(frame["type"], "permissions");
        assert_eq!(frame["data"], serde_json::json!([]));
        assert_eq!(hook.await.unwrap().0, StatusCode::OK);
    }
}

// Antigravity. Only a risky request in a Bypass tab waits for a card; every other call passes at once
// (204, so the bridge prints nothing) and agy's own mode decides.

const AGY_CWD: &str = if cfg!(windows) { "C:/proj" } else { "/proj" };

/// An app with Antigravity tabs: `a1` and `a2` in Bypass, `m1` in Manual, `p1` in Plan, `e1` in Auto-edit, `f1` in
/// Full auto, and `s1`, a shell tab (no agent) in Bypass. `tc` runs Claude Code. The agy version is pinned.
fn agy_app(hold: Duration, agy_version: &str) -> (axum::Router, AppState) {
    use marshell_protocol::api::SessionMode;
    let home = tempfile::tempdir().unwrap().keep();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_version(Some("2.1.302".into()))
    .with_codex_version(Some("0.163.0".into()))
    .with_agy_version(Some(agy_version.into()))
    .with_agent_path(|_| None)
    .with_permission_hold(hold);
    for (tab, agent, mode) in [
        ("a1", Some("antigravity"), SessionMode::Bypass),
        ("a2", Some("antigravity"), SessionMode::Bypass),
        ("m1", Some("antigravity"), SessionMode::Manual),
        ("p1", Some("antigravity"), SessionMode::Plan),
        ("e1", Some("antigravity"), SessionMode::AutoEdit),
        ("f1", Some("antigravity"), SessionMode::FullAuto),
        ("s1", None, SessionMode::Bypass),
        ("tc", Some("claude"), SessionMode::Bypass),
    ] {
        st.registry().reserve(
            tab,
            marshell_core::registry::Relaunch {
                profile_id: "none".into(),
                agent: agent.map(str::to_owned),
                cwd: AGY_CWD.into(),
                mode: Some(mode),
                name: tab.into(),
                preset: None,
            },
        );
    }
    (router(st.clone()), st)
}

/// What the bridge posts for agy's `PreToolUse`: the body plus the event name it adds.
fn agy_request(command: &str) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PreToolUse",
        "conversationId": "00000000-0000-4000-8000-000000000002",
        "modelName": "gemini-3.8-flash-high",
        "stepIdx": 2,
        "workspacePaths": [],
        "toolCall": {"name": "run_command", "args": {"CommandLine": command, "WaitMsBeforeAsync": 5000}}
    })
}

const RISKY: &str = "rm -rf build";
const SAFE: &str = "echo hi";

fn agy_deny(reason: &str) -> serde_json::Value {
    serde_json::json!({"decision": "deny", "reason": reason})
}

async fn decide(app: &axum::Router, rid: &str, body: &str) {
    let (status, _) = post_to(app, &format!("/v1/permissions/{rid}/decision"), body).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

fn row_of(st: &AppState, tab: &str) -> marshell_protocol::api::SessionInfo {
    st.registry()
        .snapshot()
        .sessions
        .into_iter()
        .find(|s| s.tab_id == tab)
        .unwrap()
}

#[tokio::test]
async fn an_antigravity_risky_request_in_bypass_waits_for_the_card() {
    use marshell_protocol::api::LiveState;
    let (app, st) = agy_app(Duration::from_secs(30), "1.4.7");
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(RISKY));
    let entry = wait_one(&app).await;
    assert_eq!(
        (entry["agent"].as_str(), entry["tab"].as_str()),
        (Some("antigravity"), Some("a1"))
    );
    assert_eq!(entry["request"]["toolCall"]["args"]["CommandLine"], RISKY);
    assert!(!hook.is_finished());
    let views = st.permission_views();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].risk.as_deref(), Some("Deletes files recursively"));
    assert_eq!(views[0].always_rule, None);
    let live = live_of(&st, "a1");
    assert_eq!(live.state, LiveState::NeedsPermission);
    assert!(live.phrase.contains(RISKY), "{}", live.phrase);
    decide(&app, &rid_of(&entry), r#"{"behavior":"deny"}"#).await;
    assert_eq!(hook.await.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn an_antigravity_request_in_manual_passes_at_once() {
    let (app, st) = agy_app(Duration::from_secs(30), "1.4.7");
    for tab in ["m1", "p1", "e1", "f1"] {
        for command in [RISKY, SAFE] {
            let hook = spawn_agent_hook("antigravity", &app, Some(tab), agy_request(command));
            let (status, _, body) = tokio::time::timeout(Duration::from_secs(2), hook)
                .await
                .expect("a pass waited")
                .unwrap();
            assert_eq!(status, StatusCode::NO_CONTENT, "{tab} {command}");
            assert!(body.is_empty(), "{tab} {command}");
        }
    }
    assert!(pending(&app).await.is_empty());
    assert!(st.permission_views().is_empty());
}

#[tokio::test]
async fn a_safe_antigravity_request_in_bypass_passes_at_once() {
    let (app, _) = agy_app(Duration::from_secs(30), "1.4.7");
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(SAFE));
    let (status, _, body) = tokio::time::timeout(Duration::from_secs(2), hook)
        .await
        .expect("a pass waited")
        .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(body.is_empty());
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn two_antigravity_tabs_each_get_their_own_answer() {
    let (app, _) = agy_app(Duration::from_secs(30), "1.4.7");
    let first = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request("rm -rf first"));
    wait_pending(&app, |p| p.len() == 1).await;
    let second = spawn_agent_hook("antigravity", &app, Some("a2"), agy_request("rm -rf second"));
    let entries = wait_pending(&app, |p| p.len() == 2).await;
    let entry_for = |tab: &str| entries.iter().find(|e| e["tab"] == tab).unwrap().clone();
    assert_eq!(
        entry_for("a1")["request"]["toolCall"]["args"]["CommandLine"],
        "rm -rf first"
    );
    assert_eq!(
        entry_for("a2")["request"]["toolCall"]["args"]["CommandLine"],
        "rm -rf second"
    );
    // Answer the later tab first, with a different decision from the earlier one.
    decide(
        &app,
        &rid_of(&entry_for("a2")),
        r#"{"behavior":"deny","message":"second only"}"#,
    )
    .await;
    let second = second.await.unwrap();
    assert_eq!(
        (second.0, answer_json(&second)),
        (StatusCode::OK, agy_deny("second only"))
    );
    // The first tab is still waiting, and its request is the only one left.
    assert!(!first.is_finished());
    let left = pending(&app).await;
    assert_eq!((left.len(), left[0]["tab"].as_str()), (1, Some("a1")));
    decide(&app, &rid_of(&entry_for("a1")), r#"{"behavior":"allow"}"#).await;
    let first = first.await.unwrap();
    assert_eq!(
        (first.0, answer_json(&first)),
        (StatusCode::OK, serde_json::json!({"decision": "ask"}))
    );
}

#[tokio::test]
async fn an_antigravity_request_from_an_unrecorded_version_still_reaches_the_card() {
    let (app, st) = agy_app(Duration::from_secs(30), "1.9.0");
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(RISKY));
    let entry = wait_one(&app).await;
    assert_eq!(entry["tab"], "a1");
    assert_eq!(st.permission_views().len(), 1);
    // No guessed status: the row carries the note instead.
    let row = row_of(&st, "a1");
    assert_eq!(
        row.status_note.as_deref(),
        Some("Antigravity 1.9.0 is not one Marshell has recorded yet")
    );
    assert!(row.live.is_none());
    decide(&app, &rid_of(&entry), r#"{"behavior":"allow"}"#).await;
    assert_eq!(
        answer_json(&hook.await.unwrap()),
        serde_json::json!({"decision": "ask"})
    );
}

#[tokio::test]
async fn an_antigravity_release_denies() {
    let (app, _) = agy_app(Duration::from_secs(30), "1.4.7");
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(RISKY));
    let rid = rid_of(&wait_one(&app).await);
    let (status, _) = post_to(&app, &format!("/v1/permissions/{rid}/release"), "").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let answer = hook.await.unwrap();
    assert_eq!(answer.0, StatusCode::OK);
    assert_eq!(answer.1.as_deref(), Some("application/json"));
    assert_eq!(answer_json(&answer), agy_deny("Denied in Marshell."));
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn an_antigravity_hold_that_times_out_denies() {
    let (app, _) = agy_app(Duration::from_secs(1), "1.4.7");
    let started = Instant::now();
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(RISKY));
    let answer = tokio::time::timeout(Duration::from_secs(5), hook)
        .await
        .expect("the hold did not expire in 5 s")
        .unwrap();
    assert!(started.elapsed() >= Duration::from_millis(900));
    assert_eq!(answer.0, StatusCode::OK);
    assert_eq!(answer_json(&answer), agy_deny("Denied in Marshell."));
    assert!(pending(&app).await.is_empty());
}

#[tokio::test]
async fn allow_on_an_antigravity_card_answers_ask() {
    let (app, _) = agy_app(Duration::from_secs(30), "1.4.7");
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(RISKY));
    let rid = rid_of(&wait_one(&app).await);
    decide(&app, &rid, r#"{"behavior":"allow"}"#).await;
    let answer = hook.await.unwrap();
    assert_eq!(answer.0, StatusCode::OK);
    assert_eq!(answer.1.as_deref(), Some("application/json"));
    assert_eq!(String::from_utf8(answer.2).unwrap(), r#"{"decision":"ask"}"#);
}

#[tokio::test]
async fn an_antigravity_project_decision_is_409() {
    let (app, _) = agy_app(Duration::from_secs(30), "1.4.7");
    let hook = spawn_agent_hook("antigravity", &app, Some("a1"), agy_request(RISKY));
    let rid = rid_of(&wait_one(&app).await);
    let (status, body) = post_to(
        &app,
        &format!("/v1/permissions/{rid}/decision"),
        r#"{"behavior":"allow","scope":"project"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body.contains("Antigravity can't keep an always-allow rule from Marshell. Allow this once instead."));
    assert_eq!(pending(&app).await.len(), 1);
    assert!(!hook.is_finished());
}

#[tokio::test]
async fn a_claude_release_still_answers_204() {
    let (app, _) = agy_app(Duration::from_secs(30), "1.4.7");
    for agent in ["claude", "codex"] {
        let hook = spawn_agent_hook(agent, &app, Some("s1"), bash_request());
        let rid = rid_of(&wait_one(&app).await);
        let (status, _) = post_to(&app, &format!("/v1/permissions/{rid}/release"), "").await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, body) = hook.await.unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT, "{agent}");
        assert!(body.is_empty(), "{agent}");
    }
    // And the timeout is still a 204 for them.
    let (app, _) = agy_app(Duration::from_secs(1), "1.4.7");
    let hook = spawn_agent_hook("claude", &app, Some("s1"), bash_request());
    let (status, _, body) = hook.await.unwrap();
    assert_eq!((status, body.is_empty()), (StatusCode::NO_CONTENT, true));
}

#[tokio::test]
async fn a_pass_never_shows_needs_permission() {
    use marshell_protocol::api::LiveState;
    let (app, st) = agy_app(Duration::from_secs(30), "1.4.7");
    for (tab, command) in [("m1", RISKY), ("a1", SAFE)] {
        let hook = spawn_agent_hook("antigravity", &app, Some(tab), agy_request(command));
        assert_eq!(hook.await.unwrap().0, StatusCode::NO_CONTENT);
        let live = live_of(&st, tab);
        assert_eq!(live.state, LiveState::Working, "{tab}");
        assert!(!live.phrase.starts_with("Wants to"), "{}", live.phrase);
    }
}

#[tokio::test]
async fn a_pass_raises_no_banner() {
    // The notifier follows a row's attention and a Needs permission state; a pass sets neither.
    let (app, st) = agy_app(Duration::from_secs(30), "1.4.7");
    for tab in ["m1", "e1", "f1"] {
        let hook = spawn_agent_hook("antigravity", &app, Some(tab), agy_request(RISKY));
        assert_eq!(hook.await.unwrap().0, StatusCode::NO_CONTENT, "{tab}");
        let row = row_of(&st, tab);
        assert!(row.attention.is_none(), "{tab}");
        assert_eq!(
            row.live.as_ref().map(|l| l.state),
            Some(marshell_protocol::api::LiveState::Working),
            "{tab}"
        );
    }
    assert!(st.permission_views().is_empty());
}

#[tokio::test]
async fn an_antigravity_request_from_a_shell_tab_passes() {
    let (app, st) = agy_app(Duration::from_secs(30), "1.4.7");
    // `s1` runs no agent (an Antigravity IDE started from a plain shell inherits its tab id); `tc` runs
    // Claude Code; `zz` is no tab at all; and a post with no tab id has none to hold for.
    for tab in [Some("s1"), Some("tc"), Some("zz"), None] {
        let hook = spawn_agent_hook("antigravity", &app, tab, agy_request(RISKY));
        let (status, _, body) = tokio::time::timeout(Duration::from_secs(2), hook)
            .await
            .expect("a pass waited")
            .unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT, "{tab:?}");
        assert!(body.is_empty(), "{tab:?}");
    }
    assert!(pending(&app).await.is_empty());
    for tab in ["s1", "tc"] {
        assert!(row_of(&st, tab).live.is_none(), "{tab} took antigravity's status");
    }
}

#[tokio::test]
async fn a_held_request_view_carries_the_hold_deadline() {
    let hold = Duration::from_secs(45);
    let (app, st) = agy_app(hold, "1.4.7");
    for (agent, tab, body) in [
        ("claude", "s1", bash_request()),
        ("codex", "s1", codex_request("npm test")),
        ("antigravity", "a1", agy_request(RISKY)),
    ] {
        let hook = spawn_agent_hook(agent, &app, Some(tab), body);
        let entry = wait_one(&app).await;
        let views = st.permission_views();
        assert_eq!(views.len(), 1, "{agent}");
        assert_eq!(
            views[0].deadline_ms,
            Some(entry["at_ms"].as_u64().unwrap() + 45_000),
            "{agent}"
        );
        assert_eq!(
            views[0].deadline_ms.unwrap(),
            views[0].at_ms + hold.as_millis() as u64,
            "{agent}"
        );
        post_to(&app, &format!("/v1/permissions/{}/release", rid_of(&entry)), "").await;
        hook.await.unwrap();
    }
}
