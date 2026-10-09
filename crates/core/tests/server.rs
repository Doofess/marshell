use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use marshell_core::server::{router, AppState};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn app() -> axum::Router {
    router(AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()]))
}

fn get(path: &str) -> axum::http::request::Builder {
    Request::builder().uri(path).header(header::HOST, format!("127.0.0.1:{PORT}"))
}

#[tokio::test]
async fn health_requires_token() {
    let res = app().oneshot(get("/v1/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_with_token_is_ok() {
    let req = get("/v1/health").header(header::AUTHORIZATION, format!("Bearer {TOKEN}")).body(Body::empty()).unwrap();
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
        ureq::get(&url).header("Authorization", &format!("Bearer {token}")).call().unwrap().status()
    })
    .await
    .unwrap();
    assert_eq!(status, 200);
    core.shutdown();
}
