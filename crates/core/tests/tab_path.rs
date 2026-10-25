//! Which PATH an agent's command is looked up on: the one a tab's program will really run under (the process plus the
//! registry, read through `with_tab_path` here), never Marshell's own. A test binary of its own, so the one test may set
//! this process's PATH without another thread reading it. The agent is an own agent whose command is an empty file named
//! `tabpath-marshell-test`, never run: its session starts the fake agent in `sleep` mode.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5559;
const ID: &str = "tabpath";
const COMMAND: &str = "tabpath-marshell-test";

/// What the injected tab PATH reads now.
static TAB: Mutex<Option<OsString>> = Mutex::new(None);

fn tab_path() -> Option<OsString> {
    TAB.lock().unwrap().clone()
}

fn set_tab(path: Option<&Path>) {
    *TAB.lock().unwrap() = path.map(|p| p.as_os_str().to_os_string());
}

fn profile(env: Vec<(String, String)>) -> Profile {
    common::tie_children_to_test();
    Profile {
        id: "sleeper".into(),
        name: "sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env,
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

/// A folder holding the empty program file.
fn folder_with_agent() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let name = if cfg!(windows) {
        format!("{COMMAND}.exe")
    } else {
        COMMAND.to_string()
    };
    std::fs::write(dir.path().join(name), "").unwrap();
    dir
}

fn state(home: &Path, profile: Profile) -> AppState {
    let agent = json!({
        "id": ID, "name": "Tab path", "cli": "Tab path", "mark": "copilot", "commands": [COMMAND],
        "prompt": {"style": "flag", "flag": "--prompt"}, "continue": null,
        "modes": {"manual": []},
    });
    std::fs::write(
        home.join("agents.json"),
        json!({"version": 1, "agents": [agent]}).to_string(),
    )
    .unwrap();
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.to_path_buf(),
        vec![profile],
    )
    .with_tab_path(tab_path)
}

fn api(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
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
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// The catalog's `found` for the own agent, and the status and text of starting it.
async fn found_and_create(st: &AppState, work: &Path) -> (bool, StatusCode, String) {
    let app = router(st.clone());
    let (_, catalog) = send(&app, api("GET", "/v1/agents/catalog", None)).await;
    let catalog: Value = serde_json::from_str(&catalog).unwrap();
    let found = catalog["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == ID)
        .unwrap_or_else(|| panic!("no {ID} in {catalog}"))["found"]
        .as_bool()
        .unwrap();
    let body = json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": work.to_string_lossy(),
        "agent": ID, "mode": "manual",
    });
    let (status, text) = send(&app, api("POST", "/v1/sessions", Some(body))).await;
    if status == StatusCode::OK {
        let tab = serde_json::from_str::<Value>(&text).unwrap()["tab_id"]
            .as_str()
            .unwrap()
            .to_string();
        if let Some(session) = st.remove_session(&tab) {
            session.force_kill();
        }
    }
    (found, status, text)
}

#[tokio::test]
async fn an_agent_is_looked_up_on_the_tab_path_not_on_marshells_own() {
    let work = tempfile::tempdir().unwrap();
    let holder = folder_with_agent();
    let elsewhere = tempfile::tempdir().unwrap();
    let not_installed = "Tab path isn't installed, or isn't on PATH.";

    // Installed while Marshell runs: only the tab's PATH holds it. Marshell's own PATH never will.
    set_tab(Some(holder.path()));
    let home = tempfile::tempdir().unwrap();
    let st = state(home.path(), profile(Vec::new()));
    let (found, status, text) = found_and_create(&st, work.path()).await;
    assert!(found, "the catalog reads it as not found");
    assert_eq!(status, StatusCode::OK, "{text}");

    // On Marshell's PATH only: the tab would print "not recognized", so it reads as not found.
    set_tab(Some(elsewhere.path()));
    let old = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![holder.path().to_path_buf()];
    dirs.extend(std::env::split_paths(&old));
    // SAFETY: this binary has this one test, and nothing else reads the environment while it runs.
    unsafe { std::env::set_var("PATH", std::env::join_paths(dirs).unwrap()) };
    let home = tempfile::tempdir().unwrap();
    let st = state(home.path(), profile(Vec::new()));
    let (found, status, text) = found_and_create(&st, work.path()).await;
    unsafe { std::env::set_var("PATH", &old) };
    assert!(!found, "the catalog reads it as found");
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    assert_eq!(text, not_installed);

    // A profile that sets its own PATH wins over the tab PATH, for starting the agent.
    let path_var = holder.path().to_string_lossy().into_owned();
    let home = tempfile::tempdir().unwrap();
    let st = state(home.path(), profile(vec![("PATH".into(), path_var)]));
    let (_, status, text) = found_and_create(&st, work.path()).await;
    assert_eq!(status, StatusCode::OK, "{text}");
}
