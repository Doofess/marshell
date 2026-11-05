//! The Doctor's routes: runs, the report and what `set_up` reads from each CLI's real plans. Every path is pinned on
//! the state, so nothing reads the real home and no CLI runs.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::paths;
use marshell_core::projects::{canonical, record};
use marshell_core::server::{AppState, DistroTarget, router};
use marshell_protocol::api::{Profile, ShellFamily};
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
    st: AppState,
    root: PathBuf,
    find: fn(&str) -> Option<PathBuf>,
    running: Option<fn() -> Vec<String>>,
    home: PathBuf,
    /// The distro's home, a plain folder that stands in for its share.
    wsl_home: PathBuf,
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
    build_with(find, on_start, None)
}

fn wsl_profile(name: &str) -> Profile {
    Profile {
        id: name.to_lowercase(),
        name: name.into(),
        command: vec!["wsl.exe".into(), "-d".into(), name.into()],
        family: ShellFamily::Wsl,
        cwd: None,
        env: Vec::new(),
        shell_integration: false,
        terminal_theme: None,
        detected: true,
    }
}

/// A Windows path as the distro's Linux sees it: `C:/x` is `/mnt/c/x`.
fn linux_path_of(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    match text.as_bytes() {
        [drive, b':', ..] if drive.is_ascii_alphabetic() => {
            format!("/mnt/{}{}", (*drive as char).to_ascii_lowercase(), &text[2..])
        }
        _ => text.to_string(),
    }
}

/// `running` set: a detected `Ubuntu` WSL profile, a pinned distro target (a plain folder, `Env::wsl_home`, stands in
/// for the share, and its bridge is the Windows one as the distro sees it) and that list of running distros.
fn build_with(find: fn(&str) -> Option<PathBuf>, on_start: bool, running: Option<fn() -> Vec<String>>) -> Env {
    build_on(find, on_start, running, PORT)
}

fn build_on(find: fn(&str) -> Option<PathBuf>, on_start: bool, running: Option<fn() -> Vec<String>>, port: u16) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let st = state_on(&root, find, on_start, running, port);
    Env {
        _dir: dir,
        app: router(st.clone()),
        st,
        home: root.join("home"),
        wsl_home: root.join("ubuntu-home"),
        claude: root.join("project").join("settings.json"),
        codex: root.join("project").join("codex-home").join("hooks.json"),
        agy_hooks: marshell_core::agents::antigravity::hooks::hooks_path(&root.join("project").join("agy-home")),
        agy_settings: marshell_core::agents::antigravity::hooks::settings_path(&root.join("project").join("agy-home")),
        qwen: root
            .join("project")
            .join("qwen-home")
            .join(".qwen")
            .join("settings.json"),
        root,
        find,
        running,
    }
}

/// The state over the folders under `root`, which a restart builds again on the same folders.
fn state_on(
    root: &Path,
    find: fn(&str) -> Option<PathBuf>,
    on_start: bool,
    running: Option<fn() -> Vec<String>>,
    port: u16,
) -> AppState {
    let home = root.join("home");
    let project = root.join("project");
    let codex_home = project.join("codex-home");
    let agy_home = project.join("agy-home");
    let qwen_home = project.join("qwen-home");
    let wsl_home = root.join("ubuntu-home");
    for d in [&home, &project, &codex_home, &agy_home, &qwen_home, &wsl_home] {
        std::fs::create_dir_all(d).unwrap();
    }
    record(&home, &project, 1).unwrap();
    let claude = project.join("settings.json");
    let codex = codex_home.join("hooks.json");
    let profiles = if running.is_some() {
        vec![wsl_profile("Ubuntu")]
    } else {
        Vec::new()
    };
    let st = AppState::new(
        TOKEN.into(),
        port,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        profiles,
    )
    .with_claude_settings(claude)
    .with_codex_hooks(codex)
    .with_agy_home(agy_home)
    .with_qwen_home(qwen_home)
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
    if !bridge.exists() {
        std::fs::write(&bridge, "").unwrap();
    }
    match running {
        Some(running) => st
            .with_distro_target(Some(DistroTarget {
                home: wsl_home,
                bridge: linux_path_of(&bridge),
            }))
            .with_running_distros(running),
        None => st,
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

    /// The router of a new `AppState` on the same folders, as after the app restarted.
    fn restarted(&self) -> axum::Router {
        router(state_on(&self.root, self.find, false, self.running, PORT))
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
    for (method, uri) in [
        ("POST", "/v1/doctor/run"),
        ("GET", "/v1/doctor/report"),
        ("POST", "/v1/doctor/fix"),
        ("POST", "/v1/doctor/fix/apply"),
        ("GET", "/v1/doctor/undo"),
        ("POST", "/v1/doctor/undo"),
    ] {
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
async fn cli_route_is_refused_without_the_bearer() {
    let e = env();
    let req = Request::builder()
        .method("POST")
        .uri("/v1/doctor/cli")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&e.app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
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

// ---- the hooks inside WSL distros ----

fn none_running() -> Vec<String> {
    Vec::new()
}

fn ubuntu_running() -> Vec<String> {
    vec!["Ubuntu".into()]
}

/// Sets up `agent`'s hooks in the Ubuntu distro, through the plan and apply routes.
async fn install_in_ubuntu(e: &Env, agent: &str) {
    let want = json!({"status": true, "statusline": true, "distro": "Ubuntu"});
    let (status, preview) = post(&e.app, &format!("/v1/agents/{agent}/hooks/plan"), want).await;
    assert_eq!(status, StatusCode::OK, "{agent}: {preview}");
    assert_eq!(preview["can_apply"], true, "{agent}: {preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{agent}: {applied}");
}

/// The ids of the report's items that are about the Ubuntu distro.
fn ubuntu_ids(report: &Value) -> Vec<&str> {
    report["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["distro"] == "Ubuntu")
        .map(|i| i["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_stopped_managed_distro_is_not_checked() {
    let e = build_with(found, false, Some(none_running));
    install_in_ubuntu(&e, "claude").await;
    // Had the file been read, this would be a parse item.
    let file = e.wsl_home.join(".claude").join("settings.json");
    std::fs::write(&file, "{not json").unwrap();
    let r = report(&e).await;
    assert_eq!(ubuntu_ids(&r), ["wsl.Ubuntu"], "{r}");
    let line = item(&r, "wsl.Ubuntu").unwrap();
    assert_eq!(line["group"], "info");
    assert_eq!(line["title"], "Not checked: Ubuntu isn't running.");
    assert_eq!(line["detail"], "Start it, then run the check again.");
    assert_eq!(line["fixable"], false);
    assert!(item(&r, "hooks.claude@Ubuntu").is_none());
}

#[tokio::test]
async fn a_running_distro_with_missing_hooks_is_broken() {
    let e = build_with(found, false, Some(ubuntu_running));
    install_in_ubuntu(&e, "claude").await;
    write(&e.wsl_home.join(".claude").join("settings.json"), &json!({}));
    let r = report(&e).await;
    let hooks = item(&r, "hooks.claude@Ubuntu").unwrap_or_else(|| panic!("{:?}", ids(&r)));
    assert_eq!(hooks["group"], "broken");
    assert_eq!(hooks["fixable"], true);
    assert_eq!(hooks["distro"], "Ubuntu");
    assert_eq!(hooks["title"], "Claude Code's hooks in Ubuntu are gone");
    assert!(item(&r, "wsl.Ubuntu").is_none());
}

/// Every CLI's entries, just after the install, are the distro bridge's exact ones: not Partial (the local bridge is
/// the wrong command) and not Orphaned (the local bridge file is not where the distro's path points).
#[cfg(windows)]
#[tokio::test]
async fn an_intact_distro_setup_reads_as_installed() {
    for agent in AGENTS {
        let e = build_with(found, false, Some(ubuntu_running));
        install_in_ubuntu(&e, agent).await;
        let r = report(&e).await;
        // Codex runs a hook only once it is trusted, which a new install is not.
        let expected: Vec<String> = if agent == "codex" {
            vec!["trust.codex@Ubuntu".into()]
        } else {
            Vec::new()
        };
        let got: Vec<String> = ubuntu_ids(&r).into_iter().map(str::to_string).collect();
        assert_eq!(got, expected, "{agent}: {r}");
    }
}

/// A distro's hooks are orphaned when the file its bridge path points at is gone: the path is judged as the Windows
/// path the distro's `/mnt/c/...` stands for, not as the local bridge.
#[cfg(windows)]
#[tokio::test]
async fn a_distro_whose_bridge_is_gone_is_orphaned() {
    let e = build_with(found, false, Some(ubuntu_running));
    install_in_ubuntu(&e, "claude").await;
    std::fs::remove_file(paths::bridge_exe(&e.home)).unwrap();
    let r = report(&e).await;
    let hooks = item(&r, "hooks.claude@Ubuntu").unwrap_or_else(|| panic!("{:?}", ids(&r)));
    assert_eq!(
        hooks["title"],
        "Claude Code's hooks in Ubuntu run a program that's missing"
    );
    assert_eq!(hooks["group"], "broken");
}

#[tokio::test]
async fn a_running_distro_whose_file_doesnt_parse_reports_that_cli_only() {
    let e = build_with(found, false, Some(ubuntu_running));
    for agent in ["claude", "codex"] {
        install_in_ubuntu(&e, agent).await;
    }
    std::fs::write(e.wsl_home.join(".codex").join("hooks.json"), "{not json").unwrap();
    let r = report(&e).await;
    let parse = item(&r, "parse.codex@Ubuntu").unwrap_or_else(|| panic!("{:?}", ids(&r)));
    assert_eq!(parse["group"], "broken");
    assert_eq!(parse["fixable"], false);
    assert!(
        parse["title"].as_str().unwrap().ends_with("hooks.json doesn't parse"),
        "{parse}"
    );
    // Claude's hooks in the same distro are read on their own.
    assert!(item(&r, "parse.claude@Ubuntu").is_none(), "{:?}", ids(&r));
    #[cfg(windows)]
    assert!(item(&r, "hooks.claude@Ubuntu").is_none(), "{:?}", ids(&r));
}

#[tokio::test]
async fn no_wsl_call_without_a_managed_distro() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn counting() -> Vec<String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        vec!["Ubuntu".into()]
    }

    // Marshell set up Claude on Windows only: no distro holds a managed path.
    let e = build_with(found, false, Some(counting));
    install(&e, "claude").await;
    let r = report(&e).await;
    assert!(r["items"].is_array());
    assert_eq!(CALLS.load(Ordering::SeqCst), 0, "no distro is managed");

    // With a managed distro the list is asked, once per run.
    let e = build_with(found, false, Some(counting));
    install_in_ubuntu(&e, "claude").await;
    report(&e).await;
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
}

// ---- fix and undo ----

/// A new report: asks for a run and waits for one that ended after the request. Call it only when no run is going
/// from before the files last changed.
async fn rerun(e: &Env) -> Value {
    rerun_on(&e.app).await
}

async fn rerun_on(app: &axum::Router) -> Value {
    let asked = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let (status, summary) = post(app, "/v1/doctor/run", json!({})).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{summary}");
    for _ in 0..200 {
        let (status, body) = get(app, "/v1/doctor/report").await;
        if status == StatusCode::OK && body["checked_at_ms"].as_u64().unwrap() >= asked {
            return body;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("no new report within 10 s");
}

/// Claude Code's settings and Codex's hooks, set up through the plans and then broken by hand: Claude's hooks and
/// status line gone, one of Codex's events gone.
async fn seed_broken(e: &Env) {
    install(e, "claude").await;
    install(e, "codex").await;
    write(&e.claude, &json!({"theme": "dark"}));
    let mut codex: Value = serde_json::from_str(&std::fs::read_to_string(&e.codex).unwrap()).unwrap();
    codex["hooks"]
        .as_object_mut()
        .unwrap()
        .remove("Stop")
        .expect("Codex had a Stop hook");
    write(&e.codex, &codex);
}

fn norm(path: &str) -> String {
    path.to_lowercase().replace('\\', "/")
}

fn managed_pairs(e: &Env) -> Vec<(String, Option<String>)> {
    let mut pairs: Vec<_> = marshell_core::config::managed::list(&e.home)
        .unwrap()
        .into_iter()
        .map(|m| (norm(&m.path), m.pointer))
        .collect();
    pairs.sort();
    pairs
}

fn fixable(report: &Value) -> Vec<&str> {
    report["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["fixable"] == true)
        .map(|i| i["id"].as_str().unwrap())
        .collect()
}

fn broken(report: &Value) -> Vec<&str> {
    report["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["group"] == "broken")
        .map(|i| i["id"].as_str().unwrap())
        .collect()
}

async fn fix_plan(app: &axum::Router, ids: &[&str]) -> (StatusCode, Value) {
    post(app, "/v1/doctor/fix", json!({ "item_ids": ids })).await
}

/// Plans and applies a fix; answers the last-fix record.
async fn fix(e: &Env, ids: &[&str]) -> Value {
    let (status, plan) = fix_plan(&e.app, ids).await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["preview"]["can_apply"], true, "{plan}");
    let (status, record) = post(&e.app, "/v1/doctor/fix/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{record}");
    record
}

fn last_fix_file(e: &Env) -> PathBuf {
    paths::doctor_last_fix(&e.home)
}

fn names(paths: &Value, file: &Path) -> bool {
    paths
        .as_array()
        .unwrap()
        .iter()
        .any(|p| norm(p.as_str().unwrap()) == norm(&file.to_string_lossy()))
}

#[tokio::test]
async fn the_seeded_broken_config_is_repaired_and_undone() {
    let e = env();
    seed_broken(&e).await;
    let claude_chain = paths::statusline_chain(&e.home, "claude");
    let seeded: Vec<(PathBuf, Option<Vec<u8>>)> = [e.claude.clone(), e.codex.clone(), claude_chain]
        .into_iter()
        .map(|p| {
            let bytes = std::fs::read(&p).ok();
            (p, bytes)
        })
        .collect();
    let managed_before = managed_pairs(&e);

    let r = rerun(&e).await;
    assert_eq!(
        fixable(&r),
        ["hooks.claude", "retention.claude", "hooks.codex"],
        "{:?}",
        ids(&r)
    );

    let record = fix(&e, &["hooks.claude", "retention.claude", "hooks.codex"]).await;
    assert_eq!(record["count"], 3, "{record}");
    assert_eq!(record["files_match"], true, "{record}");
    let text = std::fs::read_to_string(&e.claude).unwrap();
    assert!(text.contains("\"cleanupPeriodDays\": 365"), "{text}");

    // No fixable item is left; the Codex trust the person gives in /hooks is the one thing still broken.
    let r = rerun(&e).await;
    assert!(fixable(&r).is_empty(), "{:?}", fixable(&r));
    assert_eq!(broken(&r), ["trust.codex"], "{:?}", ids(&r));

    // The record is on disk, without the restart list.
    let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(last_fix_file(&e)).unwrap()).unwrap();
    assert_eq!(on_disk["snapshot_id"], record["snapshot_id"]);
    assert!(on_disk.get("restart").is_none(), "{on_disk}");

    let (status, got) = get(&e.app, "/v1/doctor/undo").await;
    assert_eq!((status, &got["files_match"]), (StatusCode::OK, &json!(true)), "{got}");
    let (status, result) = post(&e.app, "/v1/doctor/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["snapshot_id"].is_string(), "{result}");

    for (path, bytes) in &seeded {
        assert_eq!(&std::fs::read(path).ok(), bytes, "{} is as seeded", path.display());
    }
    assert_eq!(managed_pairs(&e), managed_before, "managed.json is as before the fix");
    assert!(!last_fix_file(&e).exists());
    assert_eq!(get(&e.app, "/v1/doctor/undo").await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        post(&e.app, "/v1/doctor/undo", json!({})).await.0,
        StatusCode::NO_CONTENT
    );

    // The hooks are broken again, not never set up.
    let r = rerun(&e).await;
    let hooks = item(&r, "hooks.claude").unwrap_or_else(|| panic!("{:?}", ids(&r)));
    assert_eq!(hooks["group"], "broken");
    assert!(item(&r, "setup.claude").is_none(), "{:?}", ids(&r));
}

// Found in the 7a final review: with the person's own hooks in an event, the repair removes Marshell's entries and
// appends them again, and undoing it dropped the setup's records while the file still held the entries.
// Antigravity is left out: its repair sets the one `marshell` hook whole, at the pointer the setup recorded.
#[tokio::test]
async fn undoing_a_repair_beside_the_persons_own_hooks_keeps_the_setups_records() {
    for agent in ["claude", "codex", "qwen"] {
        let e = env();
        seed_own_hooks(&e, agent);
        install(&e, agent).await;
        let managed_before = managed_pairs(&e);
        // Marshell's group in one event gone by hand; the person's own group there stays.
        let file = e.hooks_file(agent).to_path_buf();
        let mut hooks: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let (event, groups) = hooks["hooks"]
            .as_object_mut()
            .unwrap()
            .iter_mut()
            .find(|(_, g)| g.as_array().is_some_and(|a| a.len() == 2))
            .unwrap_or_else(|| panic!("{agent}: no event with the person's group and Marshell's"));
        let event = event.clone();
        groups
            .as_array_mut()
            .unwrap()
            .retain(|g| g.to_string().contains(&format!("my-{event}")));
        assert_eq!(groups.as_array().unwrap().len(), 1, "{agent} {event}: {groups}");
        write(&file, &hooks);

        let r = rerun(&e).await;
        let id = format!("hooks.{agent}");
        assert!(fixable(&r).contains(&id.as_str()), "{agent}: {:?}", ids(&r));
        fix(&e, &[id.as_str()]).await;
        let (status, result) = post(&e.app, "/v1/doctor/undo", json!({})).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {result}");
        assert_eq!(
            managed_pairs(&e),
            managed_before,
            "{agent}: managed.json is as before the fix"
        );
    }
}

#[tokio::test]
async fn hooks_and_retention_in_one_file_are_one_preview_and_retention_isnt_managed() {
    let e = env();
    install(&e, "claude").await;
    write(&e.claude, &json!({"theme": "dark"}));
    rerun(&e).await;

    let (status, plan) = fix_plan(&e.app, &["hooks.claude", "retention.claude"]).await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["item_ids"], json!(["hooks.claude", "retention.claude"]));
    assert_eq!(plan["preview"]["title"], "Fix 2 issues");
    let reason = plan["preview"]["reason"].as_str().unwrap();
    assert!(reason.contains("Claude Code's hooks are gone"), "{reason}");
    assert!(reason.contains("deletes transcripts after 30 days"), "{reason}");
    assert_eq!(reason.lines().count(), 2, "{reason}");
    let files = plan["preview"]["files"].as_array().unwrap();
    let in_claude = |f: &&Value| norm(f["path"].as_str().unwrap()) == norm(&e.claude.to_string_lossy());
    assert_eq!(files.iter().filter(in_claude).count(), 1, "{plan}");

    let (status, record) = post(&e.app, "/v1/doctor/fix/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{record}");
    // One snapshot, with one entry for the file.
    let snapshot =
        e.st.config()
            .snapshots()
            .get(record["snapshot_id"].as_str().unwrap())
            .unwrap()
            .unwrap();
    let in_snapshot = snapshot
        .files
        .iter()
        .filter(|f| norm(&f.path) == norm(&e.claude.to_string_lossy()))
        .count();
    assert_eq!(in_snapshot, 1);

    let value: Value = serde_json::from_str(&std::fs::read_to_string(&e.claude).unwrap()).unwrap();
    assert_eq!(value["cleanupPeriodDays"], 365);
    assert!(value["hooks"].is_object(), "{value}");
    let pointers: Vec<String> = managed_pairs(&e).into_iter().filter_map(|(_, p)| p).collect();
    assert!(pointers.iter().any(|p| p.starts_with("/hooks")), "{pointers:?}");
    assert!(
        !pointers.iter().any(|p| p.contains("cleanupPeriodDays")),
        "retention is the person's: {pointers:?}"
    );

    // Hooks off leaves the setting alone.
    remove(&e, "claude").await;
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&e.claude).unwrap()).unwrap();
    assert_eq!(value["cleanupPeriodDays"], 365, "{value}");
    assert!(
        value
            .get("hooks")
            .is_none_or(|h| h.as_object().is_none_or(|h| h.is_empty())),
        "{value}"
    );
}

#[tokio::test]
async fn an_edit_between_the_sheet_and_allow_is_a_409_and_writes_no_record() {
    let e = env();
    install(&e, "claude").await;
    write(&e.claude, &json!({"theme": "dark"}));
    rerun(&e).await;

    let (status, plan) = fix_plan(&e.app, &["hooks.claude", "retention.claude"]).await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    // The person edits the file while the sheet is open.
    write(&e.claude, &json!({"theme": "light"}));
    let before = std::fs::read(&e.claude).unwrap();

    let (status, conflict) = post(&e.app, "/v1/doctor/fix/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["reason"], "changed");
    assert!(names(&conflict["paths"], &e.claude), "{conflict}");
    assert_eq!(std::fs::read(&e.claude).unwrap(), before, "nothing was written");
    assert!(!last_fix_file(&e).exists());
    assert_eq!(get(&e.app, "/v1/doctor/undo").await.0, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_unknown_preview_is_a_404() {
    let e = env();
    let (status, _) = post(
        &e.app,
        "/v1/doctor/fix/apply",
        json!({"changeset_id": "c-nothing", "expected_hashes": []}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn undo_after_an_edit_is_a_409_naming_the_file() {
    let e = env();
    install(&e, "claude").await;
    write(&e.claude, &json!({"theme": "dark"}));
    rerun(&e).await;
    let record = fix(&e, &["hooks.claude"]).await;
    assert_eq!(record["files_match"], true);

    // The person edits the fixed file.
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(&e.claude).unwrap()).unwrap();
    value["theme"] = json!("light");
    write(&e.claude, &value);
    let edited = std::fs::read(&e.claude).unwrap();

    let (status, got) = get(&e.app, "/v1/doctor/undo").await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["files_match"], false);
    let (status, conflict) = post(&e.app, "/v1/doctor/undo", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["reason"], "changed");
    assert!(names(&conflict["paths"], &e.claude), "{conflict}");
    assert_eq!(std::fs::read(&e.claude).unwrap(), edited, "nothing was written");
    assert!(last_fix_file(&e).exists(), "the record stays");
}

#[tokio::test]
async fn undo_works_after_a_restart() {
    let e = env();
    seed_broken(&e).await;
    let seeded = (std::fs::read(&e.claude).unwrap(), std::fs::read(&e.codex).unwrap());
    rerun(&e).await;
    let record = fix(&e, &["hooks.claude", "retention.claude", "hooks.codex"]).await;

    let app = e.restarted();
    let (status, got) = get(&app, "/v1/doctor/undo").await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["snapshot_id"], record["snapshot_id"]);
    assert_eq!(got["item_ids"], record["item_ids"]);
    assert_eq!(got["count"], 3);
    assert_eq!(got["restart"], json!([]));
    assert_eq!(got["files_match"], true);

    let (status, result) = post(&app, "/v1/doctor/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(std::fs::read(&e.claude).unwrap(), seeded.0);
    assert_eq!(std::fs::read(&e.codex).unwrap(), seeded.1);
    assert_eq!(get(&app, "/v1/doctor/undo").await.0, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn fixing_an_item_that_isnt_fixable_is_a_422() {
    let e = env();
    // No report yet.
    let (status, body) = fix_plan(&e.app, &["hooks.claude"]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    install(&e, "claude").await;
    write(&e.claude, &json!({"theme": "dark"}));
    let r = rerun(&e).await;
    assert!(item(&r, "setup.qwen").is_some(), "{:?}", ids(&r));
    for ids in [
        &["setup.qwen"][..],
        &["nothing.here"],
        &["hooks.claude", "setup.qwen"],
        &[],
    ] {
        let (status, body) = fix_plan(&e.app, ids).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{ids:?}: {body}");
    }
    let (status, bytes) = send(
        &e.app,
        base("POST", "/v1/doctor/fix")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"item_ids": ["nothing.here"]}).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        "This item can't be fixed any more. Run the check again."
    );

    // The report says fixable, but the person set the hooks up again since.
    install(&e, "claude").await;
    let (status, body) = fix_plan(&e.app, &["hooks.claude"]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

/// Starts a session row in the registry, running; answers its generation.
fn reserve(e: &Env, tab: &str, profile: &str, agent: Option<&str>) -> u64 {
    e.st.registry()
        .reserve(
            tab,
            marshell_core::registry::Relaunch {
                profile_id: profile.into(),
                agent: agent.map(str::to_string),
                cwd: e.root.to_string_lossy().into_owned(),
                mode: None,
                name: tab.into(),
                preset: None,
                elevated: false,
            },
        )
        .1
}

#[tokio::test]
async fn the_restart_list_names_the_running_sessions_of_that_agent() {
    let e = build_with(found, false, Some(none_running));
    install(&e, "claude").await;
    write(&e.claude, &json!({"theme": "dark"}));
    // Numbers follow the order of starting, not of the tab ids.
    reserve(&e, "tab-z", "pwsh", Some("claude"));
    reserve(&e, "tab-codex", "pwsh", Some("codex"));
    reserve(&e, "tab-shell", "pwsh", None);
    reserve(&e, "tab-a", "pwsh", Some("claude"));
    reserve(&e, "tab-in-wsl", "ubuntu", Some("claude"));
    // An ended session has nothing to restart.
    let generation = reserve(&e, "tab-ended", "pwsh", Some("claude"));
    e.st.registry().mark_exited("tab-ended", generation, 0);
    rerun(&e).await;

    let (status, plan) = fix_plan(&e.app, &["hooks.claude"]).await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["restart"], json!(["tab-z", "tab-a"]), "{plan}");

    let (status, record) = post(&e.app, "/v1/doctor/fix/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["restart"], plan["restart"]);
    // This run of the app still has the list; the file on disk has none.
    let (_, got) = get(&e.app, "/v1/doctor/undo").await;
    assert_eq!(got["restart"], plan["restart"], "{got}");
    let on_disk = std::fs::read_to_string(last_fix_file(&e)).unwrap();
    assert!(!on_disk.contains("restart"), "{on_disk}");
}

#[tokio::test]
async fn a_distro_hooks_fix_uses_the_distro_plan() {
    let e = build_with(found, false, Some(ubuntu_running));
    install_in_ubuntu(&e, "claude").await;
    let file = e.wsl_home.join(".claude").join("settings.json");
    write(&file, &json!({}));
    reserve(&e, "tab-win", "pwsh", Some("claude"));
    reserve(&e, "tab-wsl", "ubuntu", Some("claude"));
    let r = rerun(&e).await;
    assert!(item(&r, "hooks.claude@Ubuntu").is_some(), "{:?}", ids(&r));

    let (status, plan) = fix_plan(&e.app, &["hooks.claude@Ubuntu"]).await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    // Only the distro's file, and only the session that runs in the distro.
    let files = plan["preview"]["files"].as_array().unwrap();
    let home = norm(&e.wsl_home.to_string_lossy());
    assert!(
        files
            .iter()
            .all(|f| norm(f["path"].as_str().unwrap()).starts_with(&home)),
        "{plan}"
    );
    assert_eq!(plan["restart"], json!(["tab-wsl"]), "{plan}");
    let (status, record) = post(&e.app, "/v1/doctor/fix/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{record}");

    // The distro's bridge path, not the Windows one.
    let bridge = linux_path_of(&paths::bridge_exe(&e.home));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains(&bridge), "{text}");
    assert!(text.contains("mshl1"), "{text}");
    #[cfg(windows)]
    {
        let r = rerun(&e).await;
        assert!(item(&r, "hooks.claude@Ubuntu").is_none(), "{:?}", ids(&r));
    }
}

#[tokio::test]
async fn a_finished_run_reaches_the_events_socket() {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let e = build_on(found, false, None, port);
    let app = e.app.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let mut req = format!("ws://127.0.0.1:{port}/v1/events")
        .into_client_request()
        .unwrap();
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("marshell.v1, token.{TOKEN}").parse().unwrap(),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();

    // The host header names this test's port.
    let run = Request::builder()
        .method("POST")
        .uri("/v1/doctor/run")
        .header(header::HOST, format!("127.0.0.1:{port}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&e.app, run).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let finished = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let Some(Ok(Message::Text(text))) = ws.next().await else {
                panic!("the socket ended");
            };
            let frame: Value = serde_json::from_str(text.as_str()).unwrap();
            if frame["type"] == "doctor" && frame["data"]["checked_at_ms"].is_u64() {
                return frame;
            }
        }
    })
    .await
    .expect("no finished doctor frame within 10 s");
    assert_eq!(finished["data"]["running"], false, "{finished}");
}
