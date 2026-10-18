use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::hook::HOOK_BODY_LIMIT;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;

fn sleeper_profile() -> marshell_protocol::api::Profile {
    marshell_protocol::api::Profile {
        id: "sleeper".into(),
        name: "Sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: marshell_protocol::api::ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

fn app() -> axum::Router {
    let home = tempfile::tempdir().unwrap().keep();
    router(AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        vec![sleeper_profile()],
    ))
}

fn base(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

fn hook(role: Option<&str>, agent: Option<&str>, tab: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut b = base("POST", "/v1/hook");
    if let Some(r) = role {
        b = b.header("x-marshell-role", r);
    }
    if let Some(a) = agent {
        b = b.header("x-marshell-agent", a);
    }
    if let Some(t) = tab {
        b = b.header("x-marshell-tab", t);
    }
    b.header("x-marshell-version", "0.0.0").body(Body::from(body)).unwrap()
}

async fn status(app: &axum::Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

async fn events(app: &axum::Router, tab: Option<&str>) -> Vec<serde_json::Value> {
    let path = match tab {
        Some(t) => format!("/v1/hook/events?tab={t}"),
        None => "/v1/hook/events".to_string(),
    };
    let res = app
        .clone()
        .oneshot(base("GET", &path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn hook_event_is_kept_under_its_live_tab() {
    let app = app();
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper" });
    let res = app
        .clone()
        .oneshot(
            base("POST", "/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let tab = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string();

    let req = hook(
        Some("event"),
        Some("claude"),
        Some(&tab),
        br#"{"hook_event_name":"Stop"}"#.to_vec(),
    );
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    let got = events(&app, Some(&tab)).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["role"], "event");
    assert_eq!(got[0]["agent"], "claude");
    assert_eq!(got[0]["body"]["hook_event_name"], "Stop");
    assert!(events(&app, None).await.is_empty());
}

#[tokio::test]
async fn hook_event_for_unknown_tab_goes_untabbed() {
    let app = app();
    let req = hook(Some("event"), Some("claude"), Some("nope"), br#"{"a":1}"#.to_vec());
    assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    assert_eq!(events(&app, None).await.len(), 1);
    assert!(events(&app, Some("nope")).await.is_empty());
}

#[tokio::test]
async fn hook_log_keeps_the_last_200() {
    let app = app();
    for i in 1..=205 {
        let req = hook(
            Some("event"),
            Some("claude"),
            None,
            format!(r#"{{"n":{i}}}"#).into_bytes(),
        );
        assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    }
    let got = events(&app, None).await;
    assert_eq!(got.len(), 200);
    assert_eq!(got[0]["body"]["n"], 6);
    assert_eq!(got[199]["body"]["n"], 205);
}

#[tokio::test]
async fn hook_rejects_non_json_body_with_400() {
    let req = hook(Some("event"), Some("claude"), None, b"not json".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hook_rejects_unknown_role_with_400() {
    let req = hook(Some("bogus"), Some("claude"), None, b"{}".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
    let req = hook(None, Some("claude"), None, b"{}".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hook_requires_agent_header_with_400() {
    let req = hook(Some("event"), None, None, b"{}".to_vec());
    assert_eq!(status(&app(), req).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hook_body_over_1_mib_gets_413() {
    let mut body = br#"{"x":""#.to_vec();
    body.resize(HOOK_BODY_LIMIT + 1, b'a');
    let req = hook(Some("event"), Some("claude"), None, body);
    assert_eq!(status(&app(), req).await, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn hook_requires_token() {
    let req = Request::builder()
        .method("POST")
        .uri("/v1/hook")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header("x-marshell-role", "event")
        .header("x-marshell-agent", "claude")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(status(&app(), req).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn context_role_answers_204_with_no_body() {
    let req = hook(Some("context"), Some("claude"), None, b"{}".to_vec());
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let bytes = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    assert!(bytes.is_empty());
}
