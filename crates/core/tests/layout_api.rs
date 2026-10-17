use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

fn app_in(home: &std::path::Path) -> axum::Router {
    router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        Vec::new(),
    ))
}

fn request(method: &str, path: &str, body: Option<serde_json::Value>) -> Request<Body> {
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

async fn body_bytes(res: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap().to_vec()
}

fn view(panes: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "panes": panes, "arrangement": null, "ratios": [], "active": 0, "zoomed": null,
        "sidebar": "rail", "sidebar_width": 320, "shell_hidden": true, "grouped": true
    })
}

#[tokio::test]
async fn get_on_a_fresh_home_is_the_default_layout() {
    let home = tempfile::tempdir().unwrap();
    let res = app_in(home.path())
        .oneshot(request("GET", "/v1/layout", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(doc["tabs"], serde_json::json!([]));
    assert_eq!(doc["view"]["sidebar_width"], 288);
    assert!(!home.path().join("layout.json").exists());
}

#[tokio::test]
async fn put_view_rejects_five_panes() {
    let home = tempfile::tempdir().unwrap();
    let body = view(serde_json::json!([null, null, null, null, null]));
    let res = app_in(home.path())
        .oneshot(request("PUT", "/v1/layout/view", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&body_bytes(res).await).contains("panes"));
    assert!(!home.path().join("layout.json").exists());
}

#[tokio::test]
async fn put_view_then_get_returns_it() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    let body = view(serde_json::json!([null, null]));
    let res = app
        .clone()
        .oneshot(request("PUT", "/v1/layout/view", Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app.oneshot(request("GET", "/v1/layout", None)).await.unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(doc["view"], body);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("layout.json")).unwrap()).unwrap();
    assert_eq!(saved["version"], 1);
    assert_eq!(saved["view"]["sidebar"], "rail");
}

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

async fn layout_of(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(request("GET", "/v1/layout", None)).await.unwrap();
    serde_json::from_slice(&body_bytes(res).await).unwrap()
}

#[tokio::test]
async fn create_records_launcher_choice() {
    let home = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let folder_path = folder.path().to_string_lossy().into_owned();
    let app = router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.path().to_path_buf(),
        vec![sleeper_profile()],
    ));
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": folder_path });
    let res = app
        .clone()
        .oneshot(request("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let choice = layout_of(&app).await["launcher"][&folder_path].clone();
    assert_eq!(
        choice,
        serde_json::json!({ "profile_id": "sleeper", "agent": null, "seq": 1, "mode": null })
    );
    // It is on disk too, not only in memory.
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.path().join("layout.json")).unwrap()).unwrap();
    assert_eq!(saved["launcher"][&folder_path]["profile_id"], "sleeper");
}

#[tokio::test]
async fn a_failed_create_records_nothing() {
    let home = tempfile::tempdir().unwrap();
    let app = app_in(home.path());
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "nope", "cwd": "C:/work" });
    let res = app
        .clone()
        .oneshot(request("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(layout_of(&app).await["launcher"], serde_json::json!({}));
}
