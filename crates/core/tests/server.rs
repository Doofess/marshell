use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn app() -> axum::Router {
    // Tests that call this never touch the disk, so the home folder does not need to exist.
    app_in(std::path::Path::new("no-such-marshell-home"))
}

fn app_in(home: &std::path::Path) -> axum::Router {
    router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![detected_profile()],
    ))
}

fn detected_profile() -> marshell_protocol::api::Profile {
    marshell_protocol::api::Profile {
        id: "cmd".into(),
        name: "Command Prompt".into(),
        command: vec!["cmd.exe".into()],
        family: marshell_protocol::api::ShellFamily::Cmd,
        cwd: None,
        env: Vec::new(),
        detected: true,
    }
}

fn profiles_request(method: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let builder = get("/v1/profiles")
        .method(method)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn profiles_get_then_put_roundtrip() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());

    let res = app.clone().oneshot(profiles_request("GET", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let before = json_body(res).await;
    assert_eq!(before["default_id"], "cmd");
    assert_eq!(before["profiles"].as_array().unwrap().len(), 1);

    let mut doc = before.clone();
    doc["profiles"].as_array_mut().unwrap().push(serde_json::json!({
        "id": "my-pwsh", "name": "My PowerShell", "command": ["pwsh.exe", "-NoLogo"],
        "family": "power_shell", "cwd": null, "env": [["FOO", "bar"]], "detected": false
    }));
    let res = app.clone().oneshot(profiles_request("PUT", Some(doc))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["profiles"].as_array().unwrap().len(), 2);

    let res = app.oneshot(profiles_request("GET", None)).await.unwrap();
    let after = json_body(res).await;
    assert_eq!(after["profiles"][1]["id"], "my-pwsh");
    assert_eq!(after["profiles"][0]["detected"], true);

    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("profiles.json")).unwrap()).unwrap();
    assert_eq!(saved["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(saved["profiles"][0]["id"], "my-pwsh");
}

#[tokio::test]
async fn profiles_put_invalid_is_400() {
    let home = tempfile::tempdir().unwrap();
    let doc = serde_json::json!({
        "profiles": [{
            "id": "bad", "name": "Bad", "command": [],
            "family": "cmd", "cwd": null, "env": [], "detected": false
        }],
        "default_id": "bad"
    });
    let res = app_in(home.path())
        .oneshot(profiles_request("PUT", Some(doc)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("command"));
    assert!(!home.path().join("profiles.json").exists());
}

fn get(path: &str) -> axum::http::request::Builder {
    Request::builder()
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
}

#[tokio::test]
async fn health_requires_token() {
    let res = app()
        .oneshot(get("/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_with_token_is_ok() {
    let req = get("/v1/health")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_host_is_forbidden_even_with_token() {
    let req = Request::builder()
        .uri("/v1/health")
        .header(header::HOST, "evil.example:5555")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn foreign_origin_is_forbidden() {
    let req = get("/v1/health")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn token_in_websocket_subprotocol_is_accepted() {
    let req = get("/v1/health")
        .header(header::SEC_WEBSOCKET_PROTOCOL, format!("marshell.v1, token.{TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn cors_preflight_from_allowed_origin() {
    let req = get("/v1/health")
        .method("OPTIONS")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
        .body(Body::empty())
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
}

#[tokio::test]
async fn start_writes_endpoint_file_and_serves() {
    let home = tempfile::tempdir().unwrap();
    let core = marshell_core::server::start(marshell_core::server::CoreConfig {
        home: home.path().to_path_buf(),
        allowed_origins: vec![],
    })
    .await
    .unwrap();
    let ep: marshell_protocol::api::Endpoint =
        serde_json::from_slice(&std::fs::read(home.path().join("run/endpoint.json")).unwrap()).unwrap();
    assert_eq!(ep.port, core.endpoint.port);
    let url = format!("http://127.0.0.1:{}/v1/health", ep.port);
    let token = ep.token.clone();
    let status = tokio::task::spawn_blocking(move || {
        ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .call()
            .unwrap()
            .status()
    })
    .await
    .unwrap();
    assert_eq!(status, 200);
    core.shutdown();
}

#[tokio::test]
async fn bad_bearer_token_is_unauthorized() {
    let req = get("/v1/health")
        .header(header::AUTHORIZATION, "Bearer nope")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bad_subprotocol_token_is_unauthorized() {
    let req = get("/v1/health")
        .header(header::SEC_WEBSOCKET_PROTOCOL, "marshell.v1, token.nope")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn empty_subprotocol_token_is_unauthorized() {
    let req = get("/v1/health")
        .header(header::SEC_WEBSOCKET_PROTOCOL, "marshell.v1, token.")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn allowed_origin_with_token_is_ok() {
    let req = get("/v1/health")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, ORIGIN)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn missing_host_is_forbidden() {
    let req = Request::builder()
        .uri("/v1/health")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn duplicate_host_is_forbidden() {
    let req = get("/v1/health")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn preflight_with_wrong_host_is_forbidden() {
    let req = Request::builder()
        .uri("/v1/health")
        .method("OPTIONS")
        .header(header::HOST, "evil.example:5555")
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

/// `CommandBuilder::from_argv` indexes `argv[0]`, so an empty argv must be refused before spawning.
#[tokio::test]
async fn empty_shell_argv_is_a_bad_request() {
    let req = get("/v1/sessions")
        .method("POST")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"shell":[],"cols":80,"rows":24}"#))
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("empty"), "body: {text}");
}

// ---- session endpoints ----

/// A profile that starts the fake agent in `sleep` mode, so a session stays open until closed.
fn sleeper_profile() -> marshell_protocol::api::Profile {
    marshell_protocol::api::Profile {
        id: "sleeper".into(),
        name: "Sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: marshell_protocol::api::ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        detected: true,
    }
}

fn sessions_app() -> axum::Router {
    router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        std::path::PathBuf::from("no-such-marshell-home"),
        vec![sleeper_profile()],
    ))
}

fn fake_argv(args: &[&str]) -> Vec<String> {
    let mut v = vec![env!("CARGO_BIN_EXE_fake-agent").to_string()];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

fn api(method: &str, path: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let builder = get(path)
        .method(method)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// POST /v1/sessions with the sleeper profile unless `extra` overrides it.
async fn create_session(app: &axum::Router, extra: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let mut body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper" });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let json = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, json)
}

async fn list_sessions(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await
}

async fn status_of(app: &axum::Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn create_returns_number_and_lists_session() {
    let app = sessions_app();
    let (s1, a) = create_session(&app, serde_json::json!({})).await;
    let (s2, b) = create_session(&app, serde_json::json!({})).await;
    assert_eq!((s1, s2), (StatusCode::OK, StatusCode::OK));
    assert_eq!((a["number"].as_u64(), b["number"].as_u64()), (Some(1), Some(2)));

    let list = list_sessions(&app).await;
    let rows = list["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["tab_id"], a["tab_id"]);
    assert_eq!(rows[1]["tab_id"], b["tab_id"]);
    assert_eq!(rows[0]["name"], "Sleeper");
    assert_eq!(rows[1]["name"], "Sleeper 2");
    assert_eq!(rows[0]["profile_id"], "sleeper");
    assert_eq!(rows[0]["state"]["kind"], "running");
    assert_eq!(list["closed"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn failed_spawn_releases_number() {
    let app = sessions_app();
    let (status, _) = create_session(&app, serde_json::json!({ "shell": ["definitely-not-a-program-xyz"] })).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(list_sessions(&app).await["sessions"].as_array().unwrap().len(), 0);

    let (status, ok) = create_session(&app, serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ok["number"], 1);
}

#[tokio::test]
async fn rename_and_reorder() {
    let app = sessions_app();
    let (_, a) = create_session(&app, serde_json::json!({})).await;
    let (_, b) = create_session(&app, serde_json::json!({})).await;
    let (a_id, b_id) = (a["tab_id"].as_str().unwrap(), b["tab_id"].as_str().unwrap());

    let rename = |name: &str| {
        api(
            "PATCH",
            &format!("/v1/sessions/{a_id}"),
            Some(serde_json::json!({ "name": name })),
        )
    };
    assert_eq!(status_of(&app, rename("  api  ")).await, StatusCode::NO_CONTENT);
    assert_eq!(status_of(&app, rename("   ")).await, StatusCode::BAD_REQUEST);
    let unknown = api("PATCH", "/v1/sessions/t_nope", Some(serde_json::json!({ "name": "x" })));
    assert_eq!(status_of(&app, unknown).await, StatusCode::NOT_FOUND);

    let reorder = |ids: &[&str]| {
        api(
            "POST",
            "/v1/sessions/reorder",
            Some(serde_json::json!({ "tab_ids": ids })),
        )
    };
    assert_eq!(status_of(&app, reorder(&[b_id, a_id])).await, StatusCode::NO_CONTENT);
    assert_eq!(status_of(&app, reorder(&[b_id])).await, StatusCode::BAD_REQUEST);
    assert_eq!(status_of(&app, reorder(&[a_id, a_id])).await, StatusCode::BAD_REQUEST);

    let rows = list_sessions(&app).await["sessions"].clone();
    assert_eq!(rows[0]["tab_id"], b["tab_id"]);
    assert_eq!(rows[1]["tab_id"], a["tab_id"]);
    assert_eq!(rows[1]["name"], "api");
    // Numbers stay with their sessions.
    assert_eq!(
        (rows[0]["number"].as_u64(), rows[1]["number"].as_u64()),
        (Some(2), Some(1))
    );
}

#[tokio::test]
async fn close_twice_is_ok_and_unknown_tab_404() {
    let app = sessions_app();
    let (_, a) = create_session(&app, serde_json::json!({})).await;
    let path = format!("/v1/sessions/{}", a["tab_id"].as_str().unwrap());
    assert_eq!(
        status_of(&app, api("DELETE", &path, None)).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        status_of(&app, api("DELETE", &path, None)).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        status_of(&app, api("DELETE", "/v1/sessions/t_never", None)).await,
        StatusCode::NO_CONTENT
    );
    let rename = api("PATCH", &path, Some(serde_json::json!({ "name": "x" })));
    assert_eq!(status_of(&app, rename).await, StatusCode::NOT_FOUND);

    let list = list_sessions(&app).await;
    assert_eq!(list["sessions"].as_array().unwrap().len(), 0);
    assert_eq!(list["closed"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn reopen_restores_profile_and_name() {
    let app = sessions_app();
    let (_, a) = create_session(&app, serde_json::json!({})).await;
    let a_id = a["tab_id"].as_str().unwrap();
    let rename = api(
        "PATCH",
        &format!("/v1/sessions/{a_id}"),
        Some(serde_json::json!({ "name": "api" })),
    );
    assert_eq!(status_of(&app, rename).await, StatusCode::NO_CONTENT);
    let close = api("DELETE", &format!("/v1/sessions/{a_id}"), None);
    assert_eq!(status_of(&app, close).await, StatusCode::NO_CONTENT);

    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions/reopen", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let reopened = json_body(res).await;
    assert_ne!(reopened["tab_id"], a["tab_id"]);
    assert_eq!(reopened["number"], 1);

    let list = list_sessions(&app).await;
    let rows = list["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tab_id"], reopened["tab_id"]);
    assert_eq!(rows[0]["name"], "api");
    assert_eq!(rows[0]["profile_id"], "sleeper");
    assert_eq!(rows[0]["state"]["kind"], "running");
    assert_eq!(list["closed"].as_array().unwrap().len(), 0);

    // Nothing left to reopen.
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions/reopen", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reopen_by_tab_id() {
    let app = sessions_app();
    let (_, a) = create_session(&app, serde_json::json!({ "name": "first" })).await;
    let (_, b) = create_session(&app, serde_json::json!({ "name": "second" })).await;
    for t in [&a, &b] {
        let path = format!("/v1/sessions/{}", t["tab_id"].as_str().unwrap());
        assert_eq!(
            status_of(&app, api("DELETE", &path, None)).await,
            StatusCode::NO_CONTENT
        );
    }
    // The most recent close is b; ask for a.
    let body = serde_json::json!({ "tab_id": a["tab_id"] });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions/reopen", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list = list_sessions(&app).await;
    assert_eq!(list["sessions"][0]["name"], "first");
    assert_eq!(list["closed"][0]["tab_id"], b["tab_id"]);
}

#[tokio::test]
async fn exited_session_is_ended_in_snapshot() {
    let app = sessions_app();
    let (status, a) = create_session(&app, serde_json::json!({ "shell": fake_argv(&["exit", "0"]) })).await;
    assert_eq!(status, StatusCode::OK);
    let mut kind = String::new();
    for _ in 0..50 {
        let list = list_sessions(&app).await;
        kind = list["sessions"][0]["state"]["kind"].as_str().unwrap().to_string();
        if kind == "exited" {
            assert_eq!(list["sessions"][0]["state"]["code"], 0);
            assert_eq!(list["sessions"][0]["tab_id"], a["tab_id"]);
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("session never showed as exited, last state: {kind}");
}

#[tokio::test]
async fn unknown_agent_is_400() {
    let app = sessions_app();
    let (status, body) = create_session(&app, serde_json::json!({ "agent": "emacs" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.as_str().unwrap().contains("Unknown agent"), "body: {body}");
    assert_eq!(list_sessions(&app).await["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn unknown_profile_is_400() {
    let app = sessions_app();
    let (status, body) = create_session(&app, serde_json::json!({ "profile_id": "nope" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.as_str().unwrap().contains("Unknown profile"), "body: {body}");
}

#[tokio::test]
async fn agent_session_is_named_after_the_agent() {
    let app = sessions_app();
    // The sleeper profile is POSIX family, so this runs `fake-agent sleep -c "claude; ..."`.
    // The fake agent ignores its args and sleeps, so the session starts: that is all we check.
    let (status, _) = create_session(&app, serde_json::json!({ "agent": "claude" })).await;
    assert_eq!(status, StatusCode::OK);
    let list = list_sessions(&app).await;
    assert_eq!(list["sessions"][0]["name"], "Claude Code");
    assert_eq!(list["sessions"][0]["agent"], "claude");
}
