//! `POST /v1/sessions/{tab}/fill`: the answers, and that the text reaches the pty as it is.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::pty::{Pull, Session};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;

fn state(home: &Path) -> AppState {
    common::tie_children_to_test();
    let profile = |id: &str, mode: &str| Profile {
        id: id.into(),
        name: id.into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), mode.into()],
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
        vec![profile("echoer", "echo"), profile("printer", "print")],
    )
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

async fn body_of(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn start(app: &axum::Router, profile: &str, cwd: &Path) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": profile, "cwd": cwd.to_string_lossy() });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body_of(res).await).unwrap();
    json["tab_id"].as_str().unwrap().to_string()
}

fn output_has(session: &Session, needle: &str, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if let Pull::Data { bytes, .. } = session.pull(0, 64 * 1024)
            && String::from_utf8_lossy(&bytes).contains(needle)
        {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn fill_body(text: &str, wait: bool) -> Option<serde_json::Value> {
    Some(serde_json::json!({ "text": text, "wait": wait }))
}

#[tokio::test]
async fn fill_without_wait_writes_the_text() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    let tab = start(&app, "echoer", tmp.path()).await;
    let session = st.session(&tab).unwrap();
    let route = format!("/v1/sessions/{tab}/fill");
    let text = "fix the tests \u{2014} please";
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body(text, false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // No Enter was added: the agent has not seen a whole line yet.
    std::thread::sleep(Duration::from_millis(400));
    assert!(!output_has(&session, "got:", Duration::ZERO));
    // The person presses Enter, and the line that arrives is exactly the text.
    session.write_input(b"\r".to_vec());
    assert!(output_has(&session, &format!("got:{text}"), Duration::from_secs(10)));

    // A control character is refused with the line, and nothing is written.
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body("a\x1b[13u", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_of(res).await,
        "The text can't hold line breaks or other control characters: Marshell never presses Enter."
    );
    let res = app
        .clone()
        .oneshot(api("POST", &route, fill_body(&"x".repeat(4001), true)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_of(res).await, "The text is over 4000 characters.");

    // An unknown session.
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions/nope/fill", fill_body("x", false)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body_of(res).await, "No such session.");

    // Like every route, it is refused without the bearer.
    let mut req = api("POST", &route, fill_body("x", false));
    req.headers_mut().remove(header::AUTHORIZATION);
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn fill_refuses_an_ended_row() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(&tmp.path().join("home"));
    let app = router(st.clone());
    // The printer writes a line and exits.
    let tab = start(&app, "printer", tmp.path()).await;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
        let json: serde_json::Value = serde_json::from_str(&body_of(res).await).unwrap();
        let row = json["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tab_id"] == tab.as_str())
            .unwrap()
            .clone();
        if row["state"]["kind"] != "running" {
            break;
        }
        assert!(Instant::now() < deadline, "the printer never exited: {row}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for wait in [false, true] {
        let res = app
            .clone()
            .oneshot(api(
                "POST",
                &format!("/v1/sessions/{tab}/fill"),
                fill_body("hello", wait),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(body_of(res).await, r#"{"reason":"ended"}"#);
    }
}
