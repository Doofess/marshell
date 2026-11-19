//! A text tab's three-way merge: `POST /v1/files/merge`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";
/// The most bytes one text may take: a draft's cap.
const TEXT_CAP: usize = 3 * 1024 * 1024;

/// The app's router over a Marshell home in a temp folder.
struct Setup {
    _home: tempfile::TempDir,
    app: axum::Router,
}

fn setup() -> Setup {
    let home = tempfile::tempdir().unwrap();
    let app = router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.path().to_path_buf(),
        Vec::new(),
    ));
    Setup { _home: home, app }
}

fn request(base: &str, mine: &str, disk: &str, bearer: bool) -> Request<Body> {
    let body = serde_json::json!({ "base": base, "mine": mine, "disk": disk }).to_string();
    let mut builder = Request::builder()
        .method("POST")
        .uri("/v1/files/merge")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::CONTENT_TYPE, "application/json");
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    builder.body(Body::from(body)).unwrap()
}

async fn merge(s: &Setup, base: &str, mine: &str, disk: &str) -> (StatusCode, String) {
    let res = s.app.clone().oneshot(request(base, mine, disk, true)).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 8 * 1024 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|_| panic!("not JSON: {text}"))
}

#[tokio::test]
async fn merge_is_refused_without_the_bearer() {
    let s = setup();
    let res = s
        .app
        .clone()
        .oneshot(request("a\n", "b\n", "c\n", false))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn merge_answers_the_text_and_its_conflicts() {
    let s = setup();
    let (status, body) = merge(&s, "a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        json(&body),
        serde_json::json!({ "text": "a\n<<<<<<< mine\nX\n=======\nY\n>>>>>>> disk\nc\n", "conflicts": 1 })
    );
}

#[tokio::test]
async fn merge_of_edits_apart_has_no_conflicts() {
    let s = setup();
    let (status, body) = merge(&s, "a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body), serde_json::json!({ "text": "A\nb\nC\n", "conflicts": 0 }));
}

#[tokio::test]
async fn merge_refuses_a_text_over_the_cap() {
    let s = setup();
    let over = "x".repeat(TEXT_CAP + 1);
    for (base, mine, disk) in [
        (over.as_str(), "a", "b"),
        ("a", over.as_str(), "b"),
        ("a", "b", over.as_str()),
    ] {
        let (status, body) = merge(&s, base, mine, disk).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body, "A text is over 3 MB.");
    }
    // A text at the cap is merged.
    let at = "x".repeat(TEXT_CAP);
    let (status, _) = merge(&s, &at, &at, &at).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn merge_refuses_sides_that_differ_too_much() {
    let s = setup();
    let base: String = (0..10_001).map(|i| format!("line {i}\n")).collect();
    let disk: String = (0..10_001).map(|i| format!("other {i}\n")).collect();
    let (status, body) = merge(&s, &base, "line 0\nmine\n", &disk).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, "The two versions differ in too many lines.");
    // The person's side is held to the same limit.
    let (status, body) = merge(&s, &base, &disk, "line 0\n").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, "The two versions differ in too many lines.");
}

#[tokio::test]
async fn merge_refuses_a_result_over_the_cap() {
    let s = setup();
    // Both sides fit, and the conflict that holds both does not.
    let mine = format!("{}\n", "m".repeat(1_700_000));
    let disk = format!("{}\n", "d".repeat(1_700_000));
    let (status, body) = merge(&s, "a\n", &mine, &disk).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, "The merged text would be over 3 MB.");
}
