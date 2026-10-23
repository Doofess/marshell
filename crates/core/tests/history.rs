mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::history::SessionMeta;
use marshell_core::history::archive::archive_paths;
use marshell_core::history::recorder::{History, schedule_copy};
use marshell_core::server::{AppState, router};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ID: &str = "00000000-0000-4000-8000-000000000001";
const STOP: &str = include_str!("fixtures/claude/2.1.302/Stop.json");

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

fn app(home: PathBuf) -> axum::Router {
    common::tie_children_to_test();
    router(
        AppState::new(
            TOKEN.into(),
            PORT,
            vec!["http://tauri.localhost".into()],
            home,
            vec![sleeper_profile()],
        )
        .with_agent_check(|_| true)
        .with_claude_version(Some("2.1.302".into())),
    )
}

fn base(method: &str, path: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

/// Opens a tab running the sleeper and returns its id.
async fn open_tab(app: &axum::Router) -> String {
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
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A Stop body whose transcript is a real file named `<ID>.jsonl` in `dir`.
fn stop_body(dir: &Path) -> (Vec<u8>, PathBuf) {
    let transcript = dir.join(format!("{ID}.jsonl"));
    std::fs::write(&transcript, b"{\"a\":1}\n").unwrap();
    let mut v: serde_json::Value = serde_json::from_str(STOP).unwrap();
    v["transcript_path"] = transcript.to_string_lossy().into();
    (serde_json::to_vec(&v).unwrap(), transcript)
}

async fn post_event(app: &axum::Router, tab: &str, body: Vec<u8>) -> StatusCode {
    let req = base("POST", "/v1/hook")
        .header("x-marshell-role", "event")
        .header("x-marshell-agent", "claude")
        .header("x-marshell-tab", tab)
        .header("x-marshell-version", "0.0.0")
        .body(Body::from(body))
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

fn meta(transcript: &Path) -> SessionMeta {
    SessionMeta {
        agent: "claude".into(),
        session_id: ID.into(),
        tab_id: "t1".into(),
        cwd: "c".into(),
        transcript_path: transcript.to_string_lossy().into_owned(),
        started_ms: 1,
        updated_ms: 2,
        previous_in_tab: None,
    }
}

#[tokio::test]
async fn stops_in_a_row_copy_once_at_a_time_and_end_current() {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let history = Arc::new(History::open(home.path()));
    history.set_settle(Duration::from_millis(10));
    let transcript = home.path().join(format!("{ID}.jsonl"));
    // Lines that do not compress well, so a copy takes a moment.
    let mut x: u64 = 0x9E3779B97F4A7C15;
    let mut bytes = Vec::new();
    while bytes.len() < 2 * 1024 * 1024 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        bytes.extend_from_slice(format!("{{\"n\":\"{x:016x}{x:016x}\"}}\n").as_bytes());
    }
    std::fs::write(&transcript, &bytes).unwrap();

    // Copies run only for a conversation the index knows, as after its SessionStart hook.
    let start = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": ID,
        "transcript_path": transcript.to_string_lossy(),
        "cwd": "c",
    });
    history.note_event("t1", &start, 1);
    let m = meta(&transcript);
    for i in 0..5 {
        schedule_copy(history.clone(), m.clone());
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().append(true).open(&transcript).unwrap();
        writeln!(file, "{{\"line\":{i}}}").unwrap();
    }

    let started = Instant::now();
    while !history.is_idle() {
        assert!(started.elapsed() < Duration::from_secs(5), "copies did not finish");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (archive, _) = archive_paths(home.path(), "claude", ID);
    let decoded = zstd::decode_all(std::fs::File::open(&archive).unwrap()).unwrap();
    assert_eq!(decoded, std::fs::read(&transcript).unwrap());
    let tmp = std::fs::read_dir(archive.parent().unwrap())
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(tmp, 0);
}

#[tokio::test]
async fn hook_route_archives_on_stop() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path().to_path_buf());
    let tab = open_tab(&app).await;
    let (body, transcript) = stop_body(home.path());

    assert_eq!(post_event(&app, &tab, body).await, StatusCode::NO_CONTENT);

    let (archive, sidecar) = archive_paths(home.path(), "claude", ID);
    let started = Instant::now();
    while !(archive.exists() && sidecar.exists()) {
        assert!(started.elapsed() < Duration::from_secs(5), "no archive after 5 s");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // The archive is written in one rename, so once it is there it is whole.
    let decoded = zstd::decode_all(std::fs::File::open(&archive).unwrap()).unwrap();
    assert_eq!(decoded, std::fs::read(&transcript).unwrap());
}

#[tokio::test]
async fn history_is_off_when_the_index_cannot_open() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("history").join("index.sqlite")).unwrap();

    let history = History::open(home.path());
    let (body, _) = stop_body(home.path());
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(history.note_event("t1", &json, 1000), None);
    assert_eq!(history.latest_for_tab("t1"), None);

    let app = app(home.path().to_path_buf());
    let tab = open_tab(&app).await;
    assert_eq!(post_event(&app, &tab, body).await, StatusCode::NO_CONTENT);
    let (archive, _) = archive_paths(home.path(), "claude", ID);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!archive.exists());
}
