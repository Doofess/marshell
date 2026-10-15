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
