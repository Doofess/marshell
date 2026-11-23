mod common;

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
        "panes": panes, "active": 0,
        "sidebar": "rail", "sidebar_width": 320, "shell_hidden": true, "grouped": true, "recent_commands": [],
        "pane": { "open": false, "width": null, "sessions": {}, "recent": [] }
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
async fn put_view_rejects_two_panes() {
    let home = tempfile::tempdir().unwrap();
    let body = view(serde_json::json!([null, null]));
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
    let body = view(serde_json::json!([null]));
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

#[tokio::test]
async fn put_view_round_trips_the_pane() {
    let home = tempfile::tempdir().unwrap();
    // The pane's session must be one the file knows, or a load drops it.
    let saved = serde_json::json!({
        "version": 1,
        "tabs": [{ "tab_id": "a", "number": 1, "name": "a", "agent": null, "profile_id": "sleeper", "cwd": "C:/work" }],
        "view": view(serde_json::json!(["a"])),
        "launcher": {}
    });
    std::fs::write(home.path().join("layout.json"), saved.to_string()).unwrap();
    let app = app_in(home.path());
    let tab = |id: &str, path: &str| {
        serde_json::json!({
            "id": id, "kind": "file", "index": 0, "title": null, "opened_by": null, "unseen": false,
            "history": [{ "path": path, "line": 12, "col": null, "view": "source" }]
        })
    };
    let mut body = view(serde_json::json!(["a"]));
    body["pane"] = serde_json::json!({
        "open": true, "width": 480,
        "sessions": { "a": { "tabs": [tab("t1", "C:/a.md"), tab("t2", "C:/b.rs")], "active": "t2" } },
        "recent": [{ "project": "C:/work", "path": "C:/work/a.md" }]
    });
    let res = app
        .clone()
        .oneshot(request("PUT", "/v1/layout/view", Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app.oneshot(request("GET", "/v1/layout", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(doc["view"]["pane"], body["pane"]);
}

#[tokio::test]
async fn put_view_round_trips_a_browser_tab() {
    let home = tempfile::tempdir().unwrap();
    let saved = serde_json::json!({
        "version": 1,
        "tabs": [{ "tab_id": "a", "number": 1, "name": "a", "agent": null, "profile_id": "sleeper", "cwd": "C:/work" }],
        "view": view(serde_json::json!(["a"])),
        "launcher": {}
    });
    std::fs::write(home.path().join("layout.json"), saved.to_string()).unwrap();
    let app = app_in(home.path());
    let file = serde_json::json!({
        "id": "t1", "kind": "file", "index": 0, "title": null, "opened_by": null, "unseen": false,
        "history": [{ "path": "C:/a.md", "line": null, "col": null, "view": "rendered" }]
    });
    let browser = serde_json::json!({
        "id": "t2", "kind": "browser", "index": 0, "title": "Example Domain", "opened_by": null, "unseen": false,
        "history": [{ "path": "https://example.com/", "line": null, "col": null, "view": "rendered" }]
    });
    let mut body = view(serde_json::json!(["a"]));
    body["pane"] = serde_json::json!({
        "open": true, "width": null,
        "sessions": { "a": { "tabs": [file, browser], "active": "t2" } },
        "recent": []
    });
    let res = app
        .clone()
        .oneshot(request("PUT", "/v1/layout/view", Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app.oneshot(request("GET", "/v1/layout", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(doc["view"]["pane"], body["pane"]);
}

#[tokio::test]
async fn put_view_round_trips_an_opened_tab() {
    let home = tempfile::tempdir().unwrap();
    let saved = serde_json::json!({
        "version": 1,
        "tabs": [{ "tab_id": "a", "number": 1, "name": "a", "agent": null, "profile_id": "sleeper", "cwd": "C:/work" }],
        "view": view(serde_json::json!(["a"])),
        "launcher": {}
    });
    std::fs::write(home.path().join("layout.json"), saved.to_string()).unwrap();
    let app = app_in(home.path());
    let browser = serde_json::json!({
        "id": "t1", "kind": "browser", "index": 0, "title": null, "opened_by": "codex", "unseen": true,
        "history": [{ "path": "https://example.com/", "line": null, "col": null, "view": "rendered" }]
    });
    let mut body = view(serde_json::json!(["a"]));
    body["pane"] = serde_json::json!({
        "open": true, "width": null,
        "sessions": { "a": { "tabs": [browser], "active": "t1" } },
        "recent": []
    });
    let res = app
        .clone()
        .oneshot(request("PUT", "/v1/layout/view", Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app.oneshot(request("GET", "/v1/layout", None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(doc["view"]["pane"], body["pane"]);
}

fn sleeper_profile() -> marshell_protocol::api::Profile {
    common::tie_children_to_test();
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

fn saved_with_session_a(home: &std::path::Path) {
    let saved = serde_json::json!({
        "version": 1,
        "tabs": [{ "tab_id": "a", "number": 1, "name": "a", "agent": null, "profile_id": "sleeper", "cwd": "C:/work" }],
        "view": view(serde_json::json!(["a"])),
        "launcher": {}
    });
    std::fs::write(home.join("layout.json"), saved.to_string()).unwrap();
}

fn pane_body(panels: serde_json::Value) -> serde_json::Value {
    let browser = serde_json::json!({
        "id": "t1", "kind": "browser", "index": 0, "title": null, "opened_by": null, "unseen": false,
        "history": [{ "path": "https://example.com/", "line": null, "col": null, "view": "rendered" }]
    });
    let mut body = view(serde_json::json!(["a"]));
    body["pane"] = serde_json::json!({
        "open": true, "width": null, "panels": panels,
        "sessions": { "a": { "tabs": [browser], "active": "t1", "active_browser": "t1" } },
        "recent": []
    });
    body
}

#[tokio::test]
async fn put_view_keeps_the_stack() {
    let home = tempfile::tempdir().unwrap();
    saved_with_session_a(home.path());
    let app = app_in(home.path());
    let body = pane_body(serde_json::json!({
        "shown": ["browser", "files", "changes"], "weights": { "browser": 6000, "files": 14000, "changes": 8000 }, "filled": "changes"
    }));
    let res = app
        .clone()
        .oneshot(request("PUT", "/v1/layout/view", Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app.oneshot(request("GET", "/v1/layout", None)).await.unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&body_bytes(res).await).unwrap();
    assert_eq!(doc["view"]["pane"], body["pane"]);
}

/// A session's drafts at their bounds: 50 comments of 2,000 characters and a full quote each.
fn full_drafts() -> serde_json::Value {
    let comment = |i: usize| {
        serde_json::json!({
            "id": format!("c{i}"), "path": "src/a.ts", "side": "new", "start": 1, "end": 2, "rows": 100000,
            "first": "0123456789abcdef", "hash": "0123456789abcdef",
            "quote": vec![format!("+{}", "x".repeat(200)); 3],
            "text": "y".repeat(2000)
        })
    };
    serde_json::json!({
        "checkout": "C:/work", "base": "origin/main",
        "comments": (0..50).map(comment).collect::<Vec<_>>(),
        "folds": { "src/a.ts": true }
    })
}

#[tokio::test]
async fn a_view_of_three_megabytes_is_saved() {
    let home = tempfile::tempdir().unwrap();
    saved_with_session_a(home.path());
    let app = app_in(home.path());
    let sessions: serde_json::Map<String, serde_json::Value> = (0..30)
        .map(|i| {
            (
                format!("s{i}"),
                serde_json::json!({ "tabs": [], "active": null, "changes": full_drafts() }),
            )
        })
        .collect();
    let mut body = view(serde_json::json!(["a"]));
    body["pane"]["sessions"] = serde_json::Value::Object(sessions);
    let size = body.to_string().len();
    assert!(size > 3 * 1024 * 1024, "the body is only {size} bytes");
    let res = app
        .oneshot(request("PUT", "/v1/layout/view", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn put_view_refuses_a_bad_stack() {
    let home = tempfile::tempdir().unwrap();
    saved_with_session_a(home.path());
    let app = app_in(home.path());
    let body = pane_body(serde_json::json!({ "shown": ["files", "browser"], "weights": {}, "filled": null }));
    let res = app
        .oneshot(request("PUT", "/v1/layout/view", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let text = String::from_utf8(body_bytes(res).await).unwrap();
    assert!(text.contains("pane.panels.shown"), "{text}");
}
