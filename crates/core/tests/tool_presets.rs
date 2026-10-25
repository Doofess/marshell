//! Starting a tool preset through the API. The sessions run the fake agent in `sleep` mode as the profile's command and
//! the preset runs `cargo` as data only (the line is never executed), and every file lives in a temp Marshell home.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5563;
const ORIGIN: &str = "http://tauri.localhost";
/// A value nobody else writes, so finding it anywhere means a leak.
const VALUE: &str = "web-field-value-4711";

fn sleeper() -> Profile {
    common::tie_children_to_test();
    Profile {
        id: "sleeper".into(),
        name: "sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

fn state(home: &Path) -> AppState {
    std::fs::write(
        marshell_core::paths::tools_file(home),
        serde_json::json!({"version": 1, "tools": [{
        "id": "mine", "name": "Mine", "commands": ["mine-marshell-test"], "presets": [
            {"id": "build", "name": "Build it", "program": "cargo",
             "args": ["run", "{target}"], "fields": [{"id": "target", "label": "Target"}]},
            {"id": "gone", "name": "Not installed", "program": "no-such-program-marshell-test"}
        ]}]})
        .to_string(),
    )
    .unwrap();
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![sleeper()],
    )
    .with_agent_check(|_| true)
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

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 256 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A create-session body: the preset `mine/<preset>` with `fields`, plus any `extra` keys.
fn create_body(cwd: &Path, preset: &str, fields: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    let mut body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy(),
        "tool": {"tool": "mine", "preset": preset, "fields": fields},
    });
    if let (Some(map), Some(more)) = (body.as_object_mut(), extra.as_object()) {
        map.extend(more.clone());
    }
    body
}

async fn create(app: &axum::Router, body: serde_json::Value) -> (StatusCode, String) {
    send(app, api("POST", "/v1/sessions", Some(body))).await
}

fn tab_of(created: &str) -> String {
    serde_json::from_str::<serde_json::Value>(created).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn stop(state: &AppState, tabs: &[&str]) {
    for tab in tabs {
        if let Some(session) = state.remove_session(tab) {
            session.force_kill();
        }
    }
}

/// Every file under `dir`, with its bytes.
fn files_under(dir: &Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else if let Ok(bytes) = std::fs::read(&path) {
            out.push((path, bytes));
        }
    }
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[tokio::test]
async fn a_preset_with_an_agent_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));
    let fields = serde_json::json!({"target": VALUE});
    for extra in [
        serde_json::json!({"agent": "claude"}),
        serde_json::json!({"prompt": "hi"}),
        serde_json::json!({"mode": "manual"}),
        serde_json::json!({"preset": "deepseek"}),
    ] {
        let (status, text) = create(&app, create_body(work.path(), "build", fields.clone(), extra.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{extra}");
        assert_eq!(text, "A tool preset starts without an agent.");
    }
    let (_, list) = send(&app, api("GET", "/v1/sessions", None)).await;
    assert!(!list.contains("Build it"), "{list}");
}

#[tokio::test]
async fn a_request_that_names_no_known_preset_or_misses_a_field_is_refused() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let app = router(state(home.path()));
    let none = serde_json::json!({});
    let (status, text) = create(&app, create_body(work.path(), "nope", none.clone(), none.clone())).await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "Unknown tool preset: mine/nope.")
    );
    let mut body = create_body(work.path(), "build", none.clone(), none.clone());
    body["tool"]["tool"] = "vanished".into();
    let (status, text) = create(&app, body).await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "Unknown tool preset: vanished/build.")
    );
    let (status, text) = create(&app, create_body(work.path(), "build", none.clone(), none.clone())).await;
    assert_eq!((status, text.as_str()), (StatusCode::BAD_REQUEST, "Fill in Target."));
    let newline = serde_json::json!({"target": "a\nb"});
    let (status, text) = create(&app, create_body(work.path(), "build", newline, none.clone())).await;
    assert_eq!((status, text.as_str()), (StatusCode::BAD_REQUEST, "Fill in Target."));
    let (status, text) = create(&app, create_body(work.path(), "gone", none.clone(), none)).await;
    assert_eq!(
        (status, text.as_str()),
        (
            StatusCode::BAD_REQUEST,
            "no-such-program-marshell-test isn't installed, or isn't on PATH."
        )
    );
}

#[tokio::test]
async fn a_preset_session_relaunches_as_its_plain_shell() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let st = state(home.path());
    let app = router(st.clone());

    let fields = serde_json::json!({"target": VALUE});
    let (status, created) = create(&app, create_body(work.path(), "build", fields, serde_json::json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let tab = tab_of(&created);

    // The row is named after the preset and is a plain shell session of the profile.
    let row = st.registry().snapshot().sessions[0].clone();
    assert_eq!(
        (row.name.as_str(), row.agent.as_deref(), row.preset.as_deref()),
        ("Build it", None, None)
    );
    assert_eq!(row.profile_id, "sleeper");

    // A layout save with the open tab in it.
    let view = serde_json::json!({
        "panes": [tab], "arrangement": null, "ratios": [], "active": 0, "zoomed": null,
        "sidebar": "expanded", "sidebar_width": 280, "shell_hidden": false, "grouped": false
    });
    let (status, _) = send(&app, api("PUT", "/v1/layout/view", Some(view))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let started = std::time::Instant::now();
    let layout = loop {
        let bytes = std::fs::read(home.path().join("layout.json")).unwrap_or_default();
        let has_tab = serde_json::from_slice::<serde_json::Value>(&bytes)
            .is_ok_and(|doc| doc["tabs"].as_array().is_some_and(|tabs| !tabs.is_empty()));
        if has_tab {
            break bytes;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "layout.json never listed the tab"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    };
    let saved: serde_json::Value = serde_json::from_slice(&layout).unwrap();
    assert_eq!(saved["tabs"][0]["agent"], serde_json::Value::Null, "{saved}");
    assert_eq!(saved["tabs"][0]["profile_id"], "sleeper");
    // The launcher remembers nothing of a tool start.
    assert!(saved["launcher"].as_object().is_none_or(|m| m.is_empty()), "{saved}");

    // Close it and reopen it: the new tab is the plain shell in the same folder.
    let (status, _) = send(&app, api("DELETE", &format!("/v1/sessions/{tab}"), None)).await;
    assert!(status.is_success(), "{status}");
    let (status, reopened) = send(&app, api("POST", "/v1/sessions/reopen", None)).await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    let new_tab = tab_of(&reopened);
    let rows = st.registry().snapshot().sessions;
    let row = rows.iter().find(|r| r.tab_id == new_tab).unwrap();
    assert_eq!((row.agent.as_deref(), row.preset.as_deref()), (None, None));
    assert_eq!(row.profile_id, "sleeper");
    assert_eq!(row.cwd, work.path().to_string_lossy());

    // The value is in no response and no file.
    for text in [
        created.as_str(),
        reopened.as_str(),
        &serde_json::to_string(&st.registry().snapshot()).unwrap(),
        &send(&app, api("GET", "/v1/sessions", None)).await.1,
        &send(&app, api("GET", "/v1/layout", None)).await.1,
    ] {
        assert!(!text.contains(VALUE), "{text}");
    }
    let mut files = Vec::new();
    files_under(home.path(), &mut files);
    for (path, bytes) in files {
        assert!(!contains(&bytes, VALUE), "{} holds the field value", path.display());
    }
    stop(&st, &[&tab, &new_tab]);
}
