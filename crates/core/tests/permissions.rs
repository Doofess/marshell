use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;

fn app(hold: Duration) -> (axum::Router, AppState) {
    let home = tempfile::tempdir().unwrap().keep();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_permission_hold(hold);
    (router(st.clone()), st)
}

fn base(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

fn hook_request(agent: &str, tab: Option<&str>, body: &serde_json::Value) -> Request<Body> {
    let mut b = base("POST", "/v1/hook")
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
    let app = app.clone();
    let req = hook_request("claude", tab, &body);
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

#[tokio::test]
async fn release_answers_204_with_no_body() {
    let (app, _) = app(Duration::from_secs(30));
    let hook = spawn_hook(&app, None, bash_request());
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
    let hook = spawn_hook(&app, None, bash_request());
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
    let hook = spawn_hook(&app, None, bash_request());
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

    let hook = spawn_hook(&app, None, bash_request());
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
    let hook = spawn_hook(&app, None, bash_request());
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
    let hook = spawn_hook(&app, None, bash_request());
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
    let hook = spawn_hook(&app, None, bash_request());
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
    let (app, _) = app(Duration::from_secs(30));
    let req = hook_request("codex", None, &bash_request());
    let hook = {
        let app = app.clone();
        tokio::spawn(async move { app.oneshot(req).await.unwrap().status() })
    };
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
    hook.abort();
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
    let hook = spawn_hook(&app, None, bash_request());
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
    let hook = spawn_hook(&app, None, bash_request());
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
