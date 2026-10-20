//! Tabs are saved to layout.json as they change, and a new core on the same home lists them as Ended.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use std::path::Path;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";

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

fn state_in(home: &Path) -> AppState {
    common::tie_children_to_test();
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![sleeper_profile()],
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

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create(app: &axum::Router, name: &str) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper", "name": name });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn sessions(app: &axum::Router) -> serde_json::Value {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    json_of(res).await
}

/// The tab names in layout.json, or None while the file is missing or half-read.
fn saved_names(home: &Path) -> Option<Vec<String>> {
    let text = std::fs::read(home.join("layout.json")).ok()?;
    let doc: serde_json::Value = serde_json::from_slice(&text).ok()?;
    let names = doc["tabs"]
        .as_array()?
        .iter()
        .map(|t| t["name"].as_str().unwrap_or_default().to_string())
        .collect();
    Some(names)
}

/// Polls (the save waits out a short delay) until layout.json lists exactly these tab names.
async fn wait_for_saved(home: &Path, want: &[&str]) {
    let want: Vec<String> = want.iter().map(|s| s.to_string()).collect();
    let start = Instant::now();
    loop {
        let got = saved_names(home);
        if got.as_ref() == Some(&want) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "layout.json never listed {want:?}; last read: {got:?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn restart_restores_tabs_as_ended() {
    let home = tempfile::tempdir().unwrap();
    let first = state_in(home.path());
    let app = router(first.clone());
    let a = create(&app, "alpha").await;
    let b = create(&app, "beta").await;
    wait_for_saved(home.path(), &["alpha", "beta"]).await;
    let before = sessions(&app).await;

    // Stop the first core's processes and drop it, then start another on the same home.
    for id in [&a, &b] {
        if let Some(session) = first.remove_session(id) {
            session.force_kill();
        }
    }
    drop(app);
    drop(first);
    let second = state_in(home.path());
    let app = router(second.clone());
    let after = sessions(&app).await;

    let rows = after["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for (row, old) in rows.iter().zip(before["sessions"].as_array().unwrap()) {
        assert_eq!(row["state"]["kind"], "ended");
        assert_eq!(row["tab_id"], old["tab_id"]);
        assert_eq!(row["number"], old["number"]);
        assert_eq!(row["name"], old["name"]);
        assert_eq!(row["profile_id"], "sleeper");
    }
    assert_eq!(rows[0]["name"], "alpha");
    assert_eq!(rows[1]["name"], "beta");
    // Nothing was started for them.
    assert_eq!(second.live_session_count(), 0);
}

#[tokio::test]
async fn closed_tabs_are_not_saved() {
    let home = tempfile::tempdir().unwrap();
    let state = state_in(home.path());
    let app = router(state.clone());
    let a = create(&app, "alpha").await;
    create(&app, "beta").await;
    wait_for_saved(home.path(), &["alpha", "beta"]).await;

    let res = app
        .clone()
        .oneshot(api("DELETE", &format!("/v1/sessions/{a}"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    wait_for_saved(home.path(), &["beta"]).await;
    for row in sessions(&app).await["sessions"].as_array().unwrap() {
        if let Some(session) = state.remove_session(row["tab_id"].as_str().unwrap()) {
            session.force_kill();
        }
    }
}
