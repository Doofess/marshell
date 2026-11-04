//! The Doctor's routes: runs, the report and what `set_up` reads from each CLI's real plans. Every path is pinned on
//! the state, so nothing reads the real home and no CLI runs.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::paths;
use marshell_core::projects::{canonical, record};
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;
const AGENTS: [&str; 4] = ["claude", "codex", "antigravity", "qwen"];

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    home: PathBuf,
    claude: PathBuf,
    codex: PathBuf,
    agy_hooks: PathBuf,
    agy_settings: PathBuf,
    qwen: PathBuf,
}

/// Every CLI is found (at a path that doesn't exist, so nothing runs) and every file lives in the recorded project.
fn found(name: &str) -> Option<PathBuf> {
    Some(PathBuf::from(format!(r"Z:\nowhere\{name}.exe")))
}

fn env() -> Env {
    build(found, false)
}

fn build(find: fn(&str) -> Option<PathBuf>, on_start: bool) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    let codex_home = project.join("codex-home");
    let agy_home = project.join("agy-home");
    let qwen_home = project.join("qwen-home");
    for d in [&home, &project, &codex_home, &agy_home, &qwen_home] {
        std::fs::create_dir_all(d).unwrap();
    }
    record(&home, &project, 1).unwrap();
    let claude = project.join("settings.json");
    let codex = codex_home.join("hooks.json");
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    )
    .with_claude_settings(claude.clone())
    .with_codex_hooks(codex.clone())
    .with_agy_home(agy_home.clone())
    .with_qwen_home(qwen_home.clone())
    .with_qwen_system_settings(root.join("qwen-system.json"))
    .with_skill_home(root.join("skill-home"))
    .with_claude_compact_paths(root.join("claude-home"), vec![root.join("claude-managed.json")])
    .with_claude_version(Some("2.1.302".into()))
    .with_codex_version(Some("0.163.0".into()))
    .with_agy_version(Some("1.0.0".into()))
    .with_qwen_version(Some("0.26.5".into()))
    .with_agent_path(find)
    .with_doctor_on_start(on_start);
    let bridge = paths::bridge_exe(&home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();
    Env {
        _dir: dir,
        app: router(st),
        home,
        claude,
        codex,
        agy_hooks: marshell_core::agents::antigravity::hooks::hooks_path(&agy_home),
        agy_settings: marshell_core::agents::antigravity::hooks::settings_path(&agy_home),
        qwen: qwen_home.join(".qwen").join("settings.json"),
    }
}

impl Env {
    /// The file a CLI's hooks live in.
    fn hooks_file(&self, agent: &str) -> &Path {
        match agent {
            "claude" => &self.claude,
            "codex" => &self.codex,
            "antigravity" => &self.agy_hooks,
            _ => &self.qwen,
        }
    }
}

fn base(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    (status, bytes.to_vec())
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let (status, bytes) = send(app, base("GET", uri).body(Body::empty()).unwrap()).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = base("POST", uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, bytes) = send(app, req).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// The apply request a user sends after looking at `preview`.
fn shown(preview: &Value) -> Value {
    let hashes: Vec<Value> = preview["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| json!({"path": f["path"], "hash": f.get("before_hash").cloned().unwrap_or(Value::Null)}))
        .collect();
    json!({"changeset_id": preview["changeset_id"], "expected_hashes": hashes})
}

const WANT: fn() -> Value = || json!({"status": true, "statusline": true});

async fn install(e: &Env, agent: &str) {
    let (status, preview) = post(&e.app, &format!("/v1/agents/{agent}/hooks/plan"), WANT()).await;
    assert_eq!(status, StatusCode::OK, "{agent}: {preview}");
    assert_eq!(preview["can_apply"], true, "{agent}: {preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{agent}: {applied}");
}

/// Hooks off.
async fn remove(e: &Env, agent: &str) {
    let (status, removal) = post(&e.app, &format!("/v1/agents/{agent}/hooks/remove-plan"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{agent}: {removal}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{agent}: {applied}");
}

/// Asks for a run, then waits for the report. The first report of a state is the one that answers 200.
async fn report(e: &Env) -> Value {
    let (status, summary) = post(&e.app, "/v1/doctor/run", json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{summary}");
    wait_report(e).await
}

async fn wait_report(e: &Env) -> Value {
    for _ in 0..200 {
        let (status, body) = get(&e.app, "/v1/doctor/report").await;
        if status == StatusCode::OK {
            return body;
        }
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("no report within 10 s");
}

fn item<'a>(report: &'a Value, id: &str) -> Option<&'a Value> {
    report["items"].as_array().unwrap().iter().find(|i| i["id"] == id)
}

fn ids(report: &Value) -> Vec<&str> {
    report["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect()
}

fn write(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// A hook group of the person's own in each of `events`.
fn own_groups(events: &[&str]) -> Value {
    let hooks: serde_json::Map<String, Value> = events
        .iter()
        .map(|event| {
            (
                event.to_string(),
                json!([{"hooks": [{"type": "command", "command": format!("my-{event}")}]}]),
            )
        })
        .collect();
    json!({ "hooks": hooks })
}

/// The person's own hooks in every event Marshell's install uses, written before the install.
fn seed_own_hooks(e: &Env, agent: &str) {
    use marshell_core::agents::{antigravity, claude, codex, qwen};
    match agent {
        "claude" => write(&e.claude, &own_groups(&claude::hooks::STATUS_EVENTS)),
        "codex" => write(&e.codex, &own_groups(&codex::hooks::STATUS_EVENTS)),
        "qwen" => write(&e.qwen, &own_groups(&qwen::hooks::STATUS_EVENTS)),
        // Antigravity keeps Marshell's entries under one named hook; the person's own hooks have other names.
        _ => {
            let mut mine = serde_json::Map::new();
            for event in antigravity::hooks::STATUS_EVENTS.iter().chain(&["PreToolUse"]) {
                mine.insert(
                    event.to_string(),
                    json!([{"matcher": ".*", "hooks": [{"type": "command", "command": "my-hook"}]}]),
                );
            }
            write(&e.agy_hooks, &json!({ "mine": mine }));
        }
    }
}

#[tokio::test]
async fn set_up_follows_each_clis_real_plans() {
    for agent in AGENTS {
        let e = env();
        install(&e, agent).await;
        // Every Marshell entry gone by hand; the managed record still names the file.
        write(e.hooks_file(agent), &json!({}));
        if agent == "antigravity" {
            write(&e.agy_settings, &json!({}));
        }
        let r = report(&e).await;
        let hooks = item(&r, &format!("hooks.{agent}")).unwrap_or_else(|| panic!("{agent}: {:?}", ids(&r)));
        assert_eq!(hooks["group"], "broken", "{agent}");
        assert_eq!(hooks["fixable"], true, "{agent}");
        assert!(item(&r, &format!("setup.{agent}")).is_none(), "{agent}: {:?}", ids(&r));
    }
}

#[tokio::test]
async fn hooks_off_reads_as_never_set_up() {
    for agent in AGENTS {
        let e = env();
        seed_own_hooks(&e, agent);
        install(&e, agent).await;
        let entries = marshell_core::config::managed::list(&e.home).unwrap();
        if agent != "antigravity" {
            // The evidence: an event that already had a group is appended to, which the record keeps as `/-`.
            assert!(
                entries
                    .iter()
                    .any(|m| m.pointer.as_deref().is_some_and(|p| p.ends_with("/-"))),
                "{agent}: {entries:?}"
            );
        }
        remove(&e, agent).await;
        let text = std::fs::read_to_string(e.hooks_file(agent)).unwrap();
        assert!(text.contains("my-"), "{agent}: the person's hooks stay: {text}");

        let r = report(&e).await;
        assert!(item(&r, &format!("hooks.{agent}")).is_none(), "{agent}: {:?}", ids(&r));
        let line = item(&r, &format!("setup.{agent}")).unwrap_or_else(|| panic!("{agent}: {:?}", ids(&r)));
        assert_eq!(line["group"], "info", "{agent}");
    }
}

#[tokio::test]
async fn the_report_finds_claudes_hooks_gone() {
    let e = env();
    install(&e, "claude").await;
    write(&e.claude, &json!({"theme": "dark"}));
    let (status, summary) = post(&e.app, "/v1/doctor/run", json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{summary}");
    assert_eq!(summary["running"], true, "{summary}");
    let r = wait_report(&e).await;
    let hooks = item(&r, "hooks.claude").unwrap_or_else(|| panic!("{:?}", ids(&r)));
    assert_eq!(hooks["group"], "broken");
    assert_eq!(hooks["fixable"], true);
    assert_eq!(hooks["path"], e.claude.display().to_string());
    assert!(r["checked_at_ms"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn a_cli_never_set_up_gets_the_setup_info_line() {
    let e = build(
        |name| (name == "codex").then(|| PathBuf::from(r"Z:\nowhere\codex.exe")),
        false,
    );
    let r = report(&e).await;
    let line = item(&r, "setup.codex").unwrap_or_else(|| panic!("{:?}", ids(&r)));
    assert_eq!(line["group"], "info");
    assert_eq!(line["open_agents"], true);
    assert_eq!(line["fixable"], false);
    assert!(item(&r, "hooks.codex").is_none());
    // Not found: nothing to set up.
    assert!(item(&r, "setup.claude").is_none(), "{:?}", ids(&r));
}

#[tokio::test]
async fn doctor_routes_refuse_without_the_bearer() {
    let e = env();
    for (method, uri) in [("POST", "/v1/doctor/run"), ("GET", "/v1/doctor/report")] {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(&e.app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn no_run_at_start_unless_pinned_on() {
    let e = env();
    // Give a startup run the time it would need.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (status, _) = get(&e.app, "/v1/doctor/report").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let on = build(found, true);
    let r = wait_report(&on).await;
    assert!(r["items"].is_array());
}

#[tokio::test]
async fn a_second_request_joins_the_run_that_is_going() {
    let e = env();
    let (a, b) = tokio::join!(
        post(&e.app, "/v1/doctor/run", json!({})),
        post(&e.app, "/v1/doctor/run", json!({}))
    );
    assert_eq!((a.0, b.0), (StatusCode::ACCEPTED, StatusCode::ACCEPTED));
    let r = wait_report(&e).await;
    assert!(r["items"].is_array());
}
