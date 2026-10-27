use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::paths;
use marshell_core::projects::{canonical, record};
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use std::path::PathBuf;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5556;

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    home: PathBuf,
    settings: PathBuf,
}

/// A temp Marshell home, a project holding the settings file, no detected profiles and nothing on
/// PATH, so no `claude` is ever run. The settings path is handed to the state, never read from the
/// environment.
fn env() -> Env {
    env_with(|_| None)
}

fn env_with(agent_path: fn(&str) -> Option<PathBuf>) -> Env {
    build(agent_path, None)
}

/// What the state is told sits next to the app's exe.
enum Programs {
    /// A folder holding fake `marshell-hook` and `marshell`.
    Both,
    /// A folder with neither.
    Empty,
}

fn env_with_programs(programs: Programs) -> Env {
    build(|_| None, Some(programs))
}

fn build(agent_path: fn(&str) -> Option<PathBuf>, programs: Option<Programs>) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    record(&home, &project, 1).unwrap();
    let settings = project.join("settings.json");
    let mut st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    )
    .with_claude_settings(settings.clone())
    .with_agent_path(agent_path);
    if let Some(programs) = programs {
        let programs_dir = root.join("programs");
        std::fs::create_dir_all(&programs_dir).unwrap();
        if matches!(programs, Programs::Both) {
            for name in ["marshell-hook", "marshell"] {
                let file = programs_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
                std::fs::write(file, name).unwrap();
            }
        }
        st = st.with_programs_dir(programs_dir);
    }
    Env {
        _dir: dir,
        app: router(st),
        home,
        settings,
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
    let (status, bytes) = post_raw(app, uri, body).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn post_raw(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Vec<u8>) {
    let req = base("POST", uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
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

#[tokio::test]
async fn setup_reports_missing_file() {
    let e = env();
    let (status, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    assert_eq!(setup["agent"], "claude");
    assert_eq!(setup["found"], false);
    assert_eq!(setup["settings_state"], "missing");
    assert_eq!(setup["hooks"], "none");
    assert_eq!(setup["has_own_statusline"], false);
    assert_eq!(setup["settings_path"], e.settings.to_string_lossy().as_ref());
    assert_eq!(
        setup["bridge_path"],
        paths::bridge_exe(&e.home).to_string_lossy().as_ref()
    );
    assert!(setup.get("version").is_none(), "{setup}");
}

/// Sessions start `claude` through a shell, so no profile runs it; PATH is where it is found.
#[tokio::test]
async fn setup_finds_claude_on_path_without_a_profile() {
    // A path that does not exist: the version check fails and nothing is run.
    let e = env_with(|name| (name == "claude").then(|| PathBuf::from("Z:/nowhere/claude.exe")));
    let (status, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    assert_eq!(setup["found"], true);
    assert_eq!(setup["exe_path"], "Z:/nowhere/claude.exe");
    assert!(setup.get("version").is_none(), "{setup}");
}

/// The list names all three agents in order; Codex is absent, Antigravity is found by its program `agy`.
#[tokio::test]
async fn found_lists_each_agent_by_its_program() {
    let e = env_with(|name| match name {
        "claude" | "agy" => Some(PathBuf::from(format!("Z:/nowhere/{name}.exe"))),
        _ => None,
    });
    let (status, list) = get(&e.app, "/v1/agents").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(
        list,
        json!([
            {"agent": "claude", "found": true, "exe_path": "Z:/nowhere/claude.exe"},
            {"agent": "codex", "found": false},
            {"agent": "antigravity", "found": true, "exe_path": "Z:/nowhere/agy.exe"},
        ])
    );
}

#[tokio::test]
async fn plan_then_apply_writes_the_hooks() {
    let e = env();
    std::fs::write(&e.settings, "{\n  \"model\": \"opus\"\n}\n").unwrap();
    let bridge = paths::bridge_exe(&e.home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();

    let (status, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["can_apply"], true, "{preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");

    let written: Value = serde_json::from_str(&std::fs::read_to_string(&e.settings).unwrap()).unwrap();
    assert_eq!(written["model"], "opus");
    assert_eq!(
        written["hooks"].as_object().unwrap().len(),
        marshell_core::agents::claude::hooks::STATUS_EVENTS.len(),
        "{written}"
    );

    let (_, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(setup["settings_state"], "ok", "{setup}");
    assert_eq!(setup["hooks"], "installed", "{setup}");
}

#[tokio::test]
async fn remove_plan_after_install_restores_the_file() {
    let e = env();
    let original = "{\r\n  // mine\r\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"},\r\n}\r\n";
    std::fs::write(&e.settings, original).unwrap();
    let bridge = paths::bridge_exe(&e.home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();

    let (_, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_ne!(std::fs::read_to_string(&e.settings).unwrap(), original);
    let (_, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(setup["hooks"], "installed", "{setup}");
    assert_eq!(setup["chained"], true, "{setup}");

    let (status, removal) = post(&e.app, "/v1/agents/claude/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{removal}");
    assert_eq!(removal["statusline_left"], false);
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(std::fs::read_to_string(&e.settings).unwrap(), original);
}

#[tokio::test]
async fn an_unknown_agent_is_still_404() {
    let e = env();
    let (status, body) = get(&e.app, "/v1/agents/nope/setup").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, _) = post(&e.app, "/v1/agents/nope/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&e.app, "/v1/agents/nope/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&e.app, "/v1/agents/gemini/setup").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unparsable_settings_is_422_with_the_line() {
    let e = env();
    std::fs::write(&e.settings, "{\n  \"a\": 1,\n  \"b\": ,\n}\n").unwrap();
    let (status, bytes) = post_raw(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = String::from_utf8(bytes).unwrap();
    assert!(
        message.ends_with("(line 3), so Marshell will not edit it."),
        "{message}"
    );
    assert!(message.starts_with(&*e.settings.to_string_lossy()), "{message}");

    let (status, _) = post_raw(&e.app, "/v1/agents/claude/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, setup) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(setup["settings_state"], "unparsable", "{setup}");
    assert_eq!(setup["parse_error"]["line"], 3, "{setup}");
    assert_eq!(setup["hooks"], "none");
}

#[tokio::test]
async fn plan_puts_a_missing_bridge_back_from_the_programs_folder() {
    let e = env_with_programs(Programs::Both);
    std::fs::write(&e.settings, "{}\n").unwrap();
    assert!(!paths::bridge_exe(&e.home).exists());

    let (status, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(std::fs::read(paths::bridge_exe(&e.home)).unwrap(), b"marshell-hook");
    assert!(paths::cli_exe(&e.home).is_file());
}

#[tokio::test]
async fn plan_is_refused_when_the_bridge_cannot_be_put_back() {
    let e = env_with_programs(Programs::Empty);
    std::fs::write(&e.settings, "{}\n").unwrap();

    let (status, bytes) = post_raw(&e.app, "/v1/agents/claude/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = String::from_utf8(bytes).unwrap();
    assert!(message.contains("hook program is missing"), "{message}");
    assert!(
        message.contains(&*paths::bridge_exe(&e.home).to_string_lossy()),
        "{message}"
    );
    assert!(message.ends_with("Reinstall Marshell, then set up again."), "{message}");
    assert_eq!(std::fs::read_to_string(&e.settings).unwrap(), "{}\n");
    assert!(!paths::bridge_exe(&e.home).exists());
}

#[tokio::test]
async fn setup_never_copies_the_programs() {
    let e = env_with_programs(Programs::Both);
    let (status, _) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!paths::bridge_exe(&e.home).exists());
}

// ---- Antigravity ----

struct AgyEnv {
    _dir: tempfile::TempDir,
    app: axum::Router,
    home: PathBuf,
    hooks: PathBuf,
    settings: PathBuf,
}

/// A state whose Antigravity user home is a folder inside the recorded project, so the config service may write
/// it. `home_name` is the Marshell home's folder name; the bridge file exists when `bridge` is set.
fn agy_env_in(dir: tempfile::TempDir, home_name: &str, bridge: bool) -> AgyEnv {
    let root = canonical(dir.path());
    let home = root.join(home_name);
    let project = root.join("project");
    let user_home = project.join("user");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&user_home).unwrap();
    record(&home, &project, 1).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    )
    .with_claude_settings(project.join("settings.json"))
    .with_agy_home(user_home.clone())
    .with_agent_path(|_| None);
    if bridge {
        let bridge = paths::bridge_exe(&home);
        std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
        std::fs::write(&bridge, "").unwrap();
    }
    AgyEnv {
        _dir: dir,
        app: router(st),
        home,
        hooks: marshell_core::agents::antigravity::hooks::hooks_path(&user_home),
        settings: marshell_core::agents::antigravity::hooks::settings_path(&user_home),
    }
}

fn agy_env() -> AgyEnv {
    agy_env_in(tempfile::tempdir().unwrap(), "home", true)
}

async fn install_agy(e: &AgyEnv) {
    let (status, preview) = post(&e.app, "/v1/agents/antigravity/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["can_apply"], true, "{preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
}

#[tokio::test]
async fn antigravity_setup_reports_both_files() {
    let e = agy_env();
    let (status, setup) = get(&e.app, "/v1/agents/antigravity/setup").await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    assert_eq!(setup["agent"], "antigravity");
    assert_eq!(setup["found"], false);
    assert_eq!(setup["settings_state"], "missing");
    assert_eq!(setup["hooks"], "none");
    assert_eq!(setup["needs_trust"], false);
    assert!(setup.get("trust").is_none(), "{setup}");
    assert_eq!(setup["settings_path"], e.settings.to_string_lossy().as_ref());
    assert_eq!(setup["hooks_path"], e.hooks.to_string_lossy().as_ref());
    assert!(setup["hooks_path"].as_str().unwrap().ends_with("hooks.json"));
    assert!(setup["settings_path"].as_str().unwrap().ends_with("settings.json"));
    // Only Antigravity names a second file.
    let (_, claude) = get(&e.app, "/v1/agents/claude/setup").await;
    assert!(claude.get("hooks_path").is_none(), "{claude}");
    let (_, codex) = get(&e.app, "/v1/agents/codex/setup").await;
    assert!(codex.get("hooks_path").is_none(), "{codex}");

    install_agy(&e).await;
    let (_, setup) = get(&e.app, "/v1/agents/antigravity/setup").await;
    assert_eq!(setup["settings_state"], "ok", "{setup}");
    assert_eq!(setup["hooks"], "installed", "{setup}");
}

#[tokio::test]
async fn antigravity_setup_finds_agy_by_its_program_name() {
    // agy's program is `agy`, not its agent id: PATH knows only `agy`.
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_settings(root.join("settings.json"))
    .with_agy_home(root.join("user"))
    .with_agy_version(Some("1.4.7".into()))
    .with_agent_path(|command| (command == "agy").then(|| PathBuf::from(r"C:\tools\agy.exe")));
    let app = router(st);
    let (status, setup) = get(&app, "/v1/agents/antigravity/setup").await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    assert_eq!(setup["found"], true, "{setup}");
    assert_eq!(setup["exe_path"], r"C:\tools\agy.exe", "{setup}");
    assert_eq!(setup["version"], "1.4.7", "{setup}");
}

#[tokio::test]
async fn antigravity_install_then_remove_round_trips() {
    // Both files missing: they end missing (hooks.json deleted, settings.json gone with its statusLine).
    let e = agy_env();
    install_agy(&e).await;
    assert!(e.hooks.exists() && e.settings.exists());
    let (status, removal) = post(&e.app, "/v1/agents/antigravity/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{removal}");
    assert_eq!(removal["statusline_left"], false);
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(!e.hooks.exists(), "hooks.json should be gone");
    if e.settings.exists() {
        let left = std::fs::read_to_string(&e.settings).unwrap();
        assert!(!left.contains("mshl1"), "{left}");
    }
    let (_, setup) = get(&e.app, "/v1/agents/antigravity/setup").await;
    assert_eq!(setup["hooks"], "none", "{setup}");

    // The person's own files: BOM, CRLF, their own named hook and status line.
    let e = agy_env();
    let hooks = "\u{feff}{\r\n  \"mine\": {\r\n    \"enabled\": true,\r\n    \"Stop\": [{\"type\": \"command\", \"command\": \"my-stop\"}]\r\n  }\r\n}\r\n";
    let settings =
        "{\r\n  \"theme\": \"dark\",\r\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"}\r\n}\r\n";
    std::fs::create_dir_all(e.hooks.parent().unwrap()).unwrap();
    std::fs::create_dir_all(e.settings.parent().unwrap()).unwrap();
    std::fs::write(&e.hooks, hooks).unwrap();
    std::fs::write(&e.settings, settings).unwrap();
    install_agy(&e).await;
    let during = std::fs::read_to_string(&e.hooks).unwrap();
    assert!(during.contains("my-stop") && during.contains("mshl1"), "{during}");
    let (_, removal) = post(&e.app, "/v1/agents/antigravity/hooks/remove-plan", json!({})).await;
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(std::fs::read_to_string(&e.hooks).unwrap(), hooks);
    assert_eq!(std::fs::read_to_string(&e.settings).unwrap(), settings);
}

#[tokio::test]
async fn antigravity_plan_on_an_unparsable_hooks_file_is_422() {
    let e = agy_env();
    std::fs::create_dir_all(e.hooks.parent().unwrap()).unwrap();
    std::fs::write(&e.hooks, "{ \"mine\": [oops\n").unwrap();
    let (status, body) = post(&e.app, "/v1/agents/antigravity/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, _) = post(&e.app, "/v1/agents/antigravity/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(std::fs::read_to_string(&e.hooks).unwrap(), "{ \"mine\": [oops\n");
    let (_, setup) = get(&e.app, "/v1/agents/antigravity/setup").await;
    assert_eq!(setup["settings_state"], "unparsable", "{setup}");
}

#[tokio::test]
async fn antigravity_plan_with_a_bridge_path_agy_cant_run_is_422() {
    let e = agy_env_in(tempfile::tempdir().unwrap(), "100%home", true);
    let (status, body) = post_raw(&e.app, "/v1/agents/antigravity/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let text = String::from_utf8_lossy(&body);
    if cfg!(windows) {
        assert!(
            text.contains("Antigravity can't run Marshell's hook from")
                && text.contains("the path holds a % or a quote"),
            "{text}"
        );
    }
    assert!(!e.hooks.exists());
}

#[tokio::test]
async fn antigravity_entries_from_another_home_are_orphaned() {
    let e = agy_env();
    install_agy(&e).await;
    // The hooks stay while the bridge they run is gone (a moved Marshell home, a half-finished uninstall).
    std::fs::remove_file(paths::bridge_exe(&e.home)).unwrap();
    let (_, setup) = get(&e.app, "/v1/agents/antigravity/setup").await;
    assert_eq!(setup["hooks"], "orphaned", "{setup}");

    // A second app, another Marshell home with a bridge, over the same hooks.json.
    let other = {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let home = root.join("other-home");
        let user_home = e
            .hooks
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        std::fs::create_dir_all(paths::bridge_exe(&home).parent().unwrap()).unwrap();
        std::fs::write(paths::bridge_exe(&home), "").unwrap();
        let st = AppState::new(
            TOKEN.into(),
            PORT,
            vec!["http://tauri.localhost".into()],
            home,
            Vec::new(),
        )
        .with_agy_home(user_home)
        .with_agent_path(|_| None);
        (dir, router(st))
    };
    let (_, setup) = get(&other.1, "/v1/agents/antigravity/setup").await;
    assert_eq!(setup["hooks"], "orphaned", "{setup}");

    // The removal is offered, and it takes the entries out.
    let (status, removal) = post(&e.app, "/v1/agents/antigravity/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{removal}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(!e.hooks.exists());
}

// ---- Codex ----

/// A state whose Codex home is a folder inside the recorded project, so the config service may write it.
/// Returns the env and the `hooks.json` path. The bridge file exists, so installs read as installed.
fn codex_env() -> (Env, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    let codex_home = project.join("codex-home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&codex_home).unwrap();
    record(&home, &project, 1).unwrap();
    let hooks = codex_home.join("hooks.json");
    let settings = project.join("settings.json");
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    )
    .with_claude_settings(settings.clone())
    .with_codex_hooks(hooks.clone())
    .with_codex_version(Some("0.163.0".into()))
    .with_agent_path(|_| None);
    let bridge = paths::bridge_exe(&home);
    std::fs::create_dir_all(bridge.parent().unwrap()).unwrap();
    std::fs::write(&bridge, "").unwrap();
    (
        Env {
            _dir: dir,
            app: router(st),
            home,
            settings,
        },
        hooks,
    )
}

async fn install_codex(e: &Env) {
    let (status, preview) = post(&e.app, "/v1/agents/codex/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["can_apply"], true, "{preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
}

#[tokio::test]
async fn codex_setup_reports_a_missing_file() {
    let (e, hooks) = codex_env();
    let (status, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    assert_eq!(setup["agent"], "codex");
    assert_eq!(setup["settings_state"], "missing");
    assert_eq!(setup["hooks"], "none");
    assert_eq!(setup["needs_trust"], true);
    assert!(setup.get("trust").is_none(), "{setup}");
    assert_eq!(setup["version"], "0.163.0");
    assert_eq!(setup["has_own_statusline"], false);
    assert_eq!(setup["chained"], false);
    assert_eq!(setup["settings_path"], hooks.to_string_lossy().as_ref());
    let (_, claude) = get(&e.app, "/v1/agents/claude/setup").await;
    assert_eq!(claude["needs_trust"], false, "{claude}");
    assert!(claude.get("trust").is_none(), "{claude}");
}

#[tokio::test]
async fn codex_plan_then_apply_installs() {
    let (e, hooks) = codex_env();
    install_codex(&e).await;
    assert!(hooks.exists());
    let (_, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(setup["settings_state"], "ok", "{setup}");
    assert_eq!(setup["hooks"], "installed", "{setup}");
    assert_eq!(setup["approve_installed"], false, "{setup}");

    let want = json!({"status": true, "statusline": false, "approve": true});
    let (_, preview) = post(&e.app, "/v1/agents/codex/hooks/plan", want).await;
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let (_, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(setup["approve_installed"], true, "{setup}");
}

#[tokio::test]
async fn codex_remove_plan_after_install_leaves_the_users_hooks() {
    let (e, hooks) = codex_env();
    let mine = json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "my-stop"}]}]}});
    std::fs::write(&hooks, serde_json::to_string_pretty(&mine).unwrap()).unwrap();
    install_codex(&e).await;
    let installed = std::fs::read_to_string(&hooks).unwrap();
    assert!(
        installed.contains("my-stop") && installed.contains("mshl1"),
        "{installed}"
    );

    let (status, removal) = post(&e.app, "/v1/agents/codex/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{removal}");
    assert_eq!(removal["statusline_left"], false);
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&removal["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let after = std::fs::read_to_string(&hooks).unwrap();
    assert!(after.contains("my-stop"), "{after}");
    assert!(!after.contains("mshl1"), "{after}");
    let (_, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(setup["hooks"], "none", "{setup}");
}

/// A `config.toml` that records `hash_of` for each of Marshell's entries in `hooks_path`.
fn trust_toml(hooks_path: &std::path::Path, alter: Option<usize>) -> String {
    use marshell_core::agents::codex::trust::{hook_hash, snake_case};
    let root: Value = serde_json::from_str(&std::fs::read_to_string(hooks_path).unwrap()).unwrap();
    let mut out = String::new();
    let mut n = 0;
    for (event, groups) in root["hooks"].as_object().unwrap() {
        for (g, group) in groups.as_array().unwrap().iter().enumerate() {
            for (h, entry) in group["hooks"].as_array().unwrap().iter().enumerate() {
                if !entry["command"].as_str().unwrap().contains("mshl1") {
                    continue;
                }
                let mut hash = hook_hash(event, group, entry);
                if alter == Some(n) {
                    hash.push('0');
                }
                n += 1;
                out.push_str(&format!(
                    "[hooks.state.'{}:{}:{g}:{h}']
trusted_hash = \"{hash}\"
",
                    hooks_path.display(),
                    snake_case(event)
                ));
            }
        }
    }
    assert!(n > 0);
    out
}

#[tokio::test]
async fn codex_setup_reads_trust_from_config_toml() {
    let (e, hooks) = codex_env();
    install_codex(&e).await;
    let (_, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(setup["trust"], "untrusted", "{setup}");

    let config = hooks.with_file_name("config.toml");
    std::fs::write(&config, trust_toml(&hooks, None)).unwrap();
    let before = std::fs::read(&config).unwrap();
    let (_, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(setup["trust"], "trusted", "{setup}");
    assert_eq!(std::fs::read(&config).unwrap(), before);

    std::fs::write(&config, trust_toml(&hooks, Some(0))).unwrap();
    let before = std::fs::read(&config).unwrap();
    let (_, setup) = get(&e.app, "/v1/agents/codex/setup").await;
    assert_eq!(setup["trust"], "changed", "{setup}");
    assert_eq!(std::fs::read(&config).unwrap(), before);
}

// ---- Codex live status ----

/// A state with one open Codex row, "cx", and the Codex version pinned so no `codex` is ever run.
fn codex_live(version: Option<&str>) -> (AppState, axum::Router) {
    let home = tempfile::tempdir().unwrap().keep();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_codex_version(version.map(str::to_string))
    .with_agent_path(|_| None);
    st.registry().reserve(
        "cx",
        marshell_core::registry::Relaunch {
            profile_id: "test".into(),
            agent: Some("codex".into()),
            cwd: "C:/work".into(),
            mode: None,
            name: "Codex".into(),
            preset: None,
        },
    );
    let app = router(st.clone());
    (st, app)
}

async fn codex_event(app: &axum::Router, event: Value) -> StatusCode {
    let req = base("POST", "/v1/hook")
        .header("x-marshell-role", "event")
        .header("x-marshell-agent", "codex")
        .header("x-marshell-tab", "cx")
        .header("x-marshell-version", "0.0.0")
        .body(Body::from(event.to_string()))
        .unwrap();
    send(app, req).await.0
}

fn codex_row(st: &AppState) -> marshell_protocol::api::SessionInfo {
    st.registry()
        .snapshot()
        .sessions
        .into_iter()
        .find(|s| s.tab_id == "cx")
        .unwrap()
}

#[tokio::test]
async fn a_codex_event_folds_into_its_tab() {
    let (st, app) = codex_live(Some("0.163.0"));
    let prompt: Value =
        serde_json::from_str(include_str!("fixtures/codex/hooks/0.163.0/UserPromptSubmit.json")).unwrap();
    assert_eq!(codex_event(&app, prompt).await, StatusCode::NO_CONTENT);
    let row = codex_row(&st);
    let live = row.live.expect("the row has a live status");
    assert_eq!(
        (live.state, live.phrase.as_str()),
        (marshell_protocol::api::LiveState::Working, "Thinking")
    );
    assert_eq!(row.status_note, None);

    // A turn ends: Done shows at once, with the recap.
    let stop: Value = serde_json::from_str(include_str!("fixtures/codex/hooks/0.163.0/Stop-recap.json")).unwrap();
    codex_event(&app, stop).await;
    let live = codex_row(&st).live.unwrap();
    assert_eq!(live.state, marshell_protocol::api::LiveState::Done);
    assert!(live.recap.is_some());
}

#[tokio::test]
async fn an_unrecorded_codex_version_leaves_no_status_and_says_why() {
    for (version, note) in [
        (Some("0.170.0"), "Codex 0.170.0 is not one Marshell has recorded yet"),
        (None, "Codex's version could not be read"),
    ] {
        let (st, app) = codex_live(version);
        let prompt: Value =
            serde_json::from_str(include_str!("fixtures/codex/hooks/0.163.0/UserPromptSubmit.json")).unwrap();
        codex_event(&app, prompt).await;
        let row = codex_row(&st);
        assert_eq!(row.live, None, "{version:?}");
        assert_eq!(row.status_note.as_deref(), Some(note));
    }
}

#[tokio::test]
async fn codex_status_off_sets_its_note() {
    let (st, app) = codex_live(Some("0.163.0"));
    let (status, _) = send(
        &app,
        base("PUT", "/v1/settings")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"appearance": {}, "status_off": ["codex"]}).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let prompt: Value =
        serde_json::from_str(include_str!("fixtures/codex/hooks/0.163.0/UserPromptSubmit.json")).unwrap();
    codex_event(&app, prompt).await;
    let row = codex_row(&st);
    assert_eq!(row.live, None);
    assert_eq!(row.status_note.as_deref(), Some("Live status is off for Codex"));
}

// ---- Antigravity live status ----

/// A state with one open Antigravity row, "ag", and the Antigravity version pinned so no `agy` is ever run.
fn agy_live(version: Option<&str>) -> (AppState, axum::Router) {
    let home = tempfile::tempdir().unwrap().keep();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_agy_version(version.map(str::to_string))
    .with_agent_path(|_| None);
    st.registry().reserve(
        "ag",
        marshell_core::registry::Relaunch {
            profile_id: "test".into(),
            agent: Some("antigravity".into()),
            cwd: "C:/work".into(),
            mode: None,
            name: "Antigravity".into(),
            preset: None,
        },
    );
    let app = router(st.clone());
    (st, app)
}

/// Posts an agy body as the bridge does: the event name goes in the body, the role in the header.
async fn agy_post(app: &axum::Router, role: &str, event: Option<&str>, mut body: Value) -> StatusCode {
    if let Some(event) = event {
        body["hook_event_name"] = json!(event);
    }
    let req = base("POST", "/v1/hook")
        .header("x-marshell-role", role)
        .header("x-marshell-agent", "antigravity")
        .header("x-marshell-tab", "ag")
        .header("x-marshell-version", "0.0.0")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await.0
}

fn agy_row(st: &AppState) -> marshell_protocol::api::SessionInfo {
    st.registry()
        .snapshot()
        .sessions
        .into_iter()
        .find(|s| s.tab_id == "ag")
        .unwrap()
}

fn agy_fixture(dir: &str, name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/antigravity")
        .join(dir)
        .join("1.4.7")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn agy_hook(name: &str) -> Value {
    agy_fixture("hooks", name)
}

fn agy_statusline(name: &str) -> Value {
    agy_fixture("statusline", name)
}

#[tokio::test]
async fn an_antigravity_event_folds_into_its_tab() {
    use marshell_protocol::api::LiveState;
    let (st, app) = agy_live(Some("1.4.7"));
    let pre = agy_hook("PreToolUse-run_command");
    assert_eq!(
        agy_post(&app, "event", Some("PreToolUse"), pre).await,
        StatusCode::NO_CONTENT
    );
    let row = agy_row(&st);
    let live = row.live.expect("the row has a live status");
    assert_eq!(
        (live.state, live.phrase.as_str()),
        (LiveState::Working, "Running `echo hello-6d`")
    );
    assert_eq!(row.status_note, None);

    // The status line adds the model and the context, and leaves the state the hooks set.
    agy_post(&app, "statusline", None, agy_statusline("working")).await;
    let live = agy_row(&st).live.unwrap();
    assert_eq!(live.model.as_deref(), Some("Gemini 3.8 Flash (High)"));
    assert_eq!(live.context_pct, Some(1));
    assert_eq!(
        (live.state, live.phrase.as_str()),
        (LiveState::Working, "Running `echo hello-6d`")
    );

    // A turn ends.
    agy_post(&app, "event", Some("Stop"), agy_hook("Stop")).await;
    let live = agy_row(&st).live.unwrap();
    assert_eq!((live.state, live.phrase.as_str()), (LiveState::Done, "Done"));
    assert_eq!(live.recap, None);
}

#[tokio::test]
async fn an_unrecorded_antigravity_version_leaves_no_status_and_says_why() {
    for (version, note) in [
        (Some("1.5.0"), "Antigravity 1.5.0 is not one Marshell has recorded yet"),
        (None, "Antigravity's version could not be read"),
    ] {
        let (st, app) = agy_live(version);
        agy_post(&app, "event", Some("PreInvocation"), agy_hook("PreInvocation")).await;
        let row = agy_row(&st);
        assert_eq!(row.live, None, "{version:?}");
        assert_eq!(row.status_note.as_deref(), Some(note));
    }
}

#[tokio::test]
async fn the_event_log_keeps_no_last_user_input_or_email() {
    let (st, app) = agy_live(Some("1.4.7"));
    let stop = agy_hook("Stop");
    let line = agy_statusline("working");
    let input = stop["lastUserInput"].as_str().unwrap().to_string();
    let email = line["email"].as_str().unwrap().to_string();
    assert_eq!(
        agy_post(&app, "event", Some("Stop"), stop).await,
        StatusCode::NO_CONTENT
    );
    agy_post(&app, "statusline", None, line).await;

    // The row has no pty session, so its records are filed with the untabbed ones: the event in the ring, the
    // status line in the latest-status-line slot.
    let (status, events) = get(&app, "/v1/hook/events").await;
    assert_eq!(status, StatusCode::OK);
    let mut kept = events.to_string();
    assert!(events.as_array().is_some_and(|a| a.len() == 1), "{kept}");
    assert!(kept.contains("NO_TOOL_CALL"), "the rest of the body is kept: {kept}");
    let line = st.hook_log().latest_statusline(None).expect("the status line is kept");
    assert_eq!(line.body["agent_state"], "working");
    kept.push_str(&line.body.to_string());
    assert!(!kept.contains(&input), "{kept}");
    assert!(!kept.contains(&email), "{kept}");
    assert!(!kept.contains("lastUserInput"), "{kept}");
    assert!(line.body.get("email").is_none(), "{kept}");
    // Nor in the row.
    let row = format!("{:?}", agy_row(&st));
    assert!(!row.contains(&input) && !row.contains(&email), "{row}");
}

#[tokio::test]
async fn an_antigravity_status_line_with_pending_raises_needs_you() {
    let (st, app) = agy_live(Some("1.4.7"));
    agy_post(&app, "event", Some("PreToolUse"), agy_hook("PreToolUse-run_command")).await;
    assert_eq!(agy_row(&st).attention, None);

    agy_post(&app, "statusline", None, agy_statusline("pending")).await;
    let attention = agy_row(&st).attention.expect("agy's own prompt raises attention");
    assert_eq!(attention.message.as_deref(), Some("Waiting for you in the terminal"));
    assert!(attention.limited);
    // The status the hooks set is still there.
    assert_eq!(agy_row(&st).live.unwrap().phrase, "Running `echo hello-6d`");

    // The same status line again keeps it, and so does one that changes the context.
    agy_post(&app, "statusline", None, agy_statusline("pending")).await;
    assert!(agy_row(&st).attention.is_some());
    let mut more = agy_statusline("pending");
    more["context_window"]["used_percentage"] = json!(40.0);
    agy_post(&app, "statusline", None, more).await;
    assert!(agy_row(&st).attention.is_some());
    assert_eq!(agy_row(&st).live.unwrap().context_pct, Some(40));

    // The prompt is answered: the next status line has no pending key.
    agy_post(&app, "statusline", None, agy_statusline("working")).await;
    assert_eq!(agy_row(&st).attention, None);

    // Another raise ends with the next hook event, even one that leaves the status as it was.
    agy_post(&app, "statusline", None, agy_statusline("pending")).await;
    assert!(agy_row(&st).attention.is_some());
    agy_post(&app, "event", Some("PostInvocation"), agy_hook("PostInvocation")).await;
    assert_eq!(agy_row(&st).attention, None);
}

#[tokio::test]
async fn antigravity_status_off_sets_its_note() {
    let (st, app) = agy_live(Some("1.4.7"));
    let (status, _) = send(
        &app,
        base("PUT", "/v1/settings")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"appearance": {}, "status_off": ["antigravity"]}).to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    agy_post(&app, "event", Some("PreInvocation"), agy_hook("PreInvocation")).await;
    let row = agy_row(&st);
    assert_eq!(row.live, None);
    assert_eq!(row.status_note.as_deref(), Some("Live status is off for Antigravity"));
    // A status line raises nothing while status is off.
    agy_post(&app, "statusline", None, agy_statusline("pending")).await;
    assert_eq!(agy_row(&st).attention, None);
}

// ---- Install type ----

/// A state with the given agent files "found" (paths that don't exist, so nothing is run) and every version pinned.
fn install_env(codex_home: Option<&std::path::Path>) -> (tempfile::TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_version(Some("2.1.303".into()))
    .with_codex_version(Some("0.163.2".into()))
    .with_agy_version(None);
    if let Some(codex_home) = codex_home {
        st = st
            .with_codex_hooks(codex_home.join("hooks.json"))
            .with_agent_path(|name| {
                (name == "codex").then(|| PathBuf::from(r"Z:\nowhere\Programs\OpenAI\Codex\bin\codex.exe"))
            });
    } else {
        st = st.with_agent_path(|_| None);
    }
    (dir, router(st))
}

#[tokio::test]
async fn install_is_refused_without_the_bearer() {
    let (_dir, app) = install_env(None);
    let req = Request::builder()
        .method("GET")
        .uri("/v1/agents/claude/install")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn install_for_an_unknown_agent_is_404() {
    let (_dir, app) = install_env(None);
    let (status, body) = get(&app, "/v1/agents/nope/install").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn install_reports_not_found() {
    let (_dir, app) = install_env(None);
    let (status, body) = get(&app, "/v1/agents/claude/install").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["agent"], "claude");
    assert_eq!(body["found"], false);
    assert_eq!(body["version"], Value::Null);
    assert_eq!(body["install"]["kind"], "unknown");
    assert_eq!(body["update_command"], Value::Null);
    assert_eq!(body["newer_version"], Value::Null);
}

#[tokio::test]
async fn install_reads_codex_version_json() {
    let dir = tempfile::tempdir().unwrap();
    let codex_home = dir.path().join("codex-home");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join("version.json"),
        r#"{"latest_version":"0.164.0","last_checked_at":"2026-10-27T00:00:00Z","dismissed_version":null}"#,
    )
    .unwrap();
    let (_keep, app) = install_env(Some(&codex_home));
    let (status, body) = get(&app, "/v1/agents/codex/install?recheck=true").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["found"], true);
    assert_eq!(body["version"], "0.163.2");
    assert_eq!(body["install"]["kind"], "native");
    assert_eq!(body["update_command"], "codex update");
    assert_eq!(body["newer_version"], "0.164.0");

    // A dismissed version is no news.
    std::fs::write(
        codex_home.join("version.json"),
        r#"{"latest_version":"0.164.0","dismissed_version":"0.164.0"}"#,
    )
    .unwrap();
    let (_, body) = get(&app, "/v1/agents/codex/install").await;
    assert_eq!(body["newer_version"], Value::Null, "{body}");
}

/// The probe script of `install_recheck_reads_the_version_again`; the agent-path seam is a plain `fn`, so it reads this.
static VERSION_PROBE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[tokio::test]
async fn install_recheck_reads_the_version_again() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    // The script prints the text of a sibling file, so changing the version leaves the script's modified time as it was.
    let text = dir.path().join("probe-version.txt");
    std::fs::write(&text, "2.0.0\n").unwrap();
    let script = if cfg!(windows) {
        let script = dir.path().join("version-probe.cmd");
        std::fs::write(&script, "@echo off\r\ntype \"%~dp0probe-version.txt\"\r\n").unwrap();
        script
    } else {
        let script = dir.path().join("version-probe.sh");
        std::fs::write(&script, "#!/bin/sh\ncat \"$(dirname \"$0\")/probe-version.txt\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    };
    VERSION_PROBE.set(script).unwrap();
    // No version is pinned for Antigravity, so the program is asked and the answer is remembered.
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_agent_path(|name| (name == "agy").then(|| VERSION_PROBE.get().cloned()).flatten());
    let app = router(st);

    let (_, body) = get(&app, "/v1/agents/antigravity/install").await;
    assert_eq!(body["version"], "2.0.0", "{body}");

    std::fs::write(&text, "2.1.0\n").unwrap();
    let (_, body) = get(&app, "/v1/agents/antigravity/install").await;
    assert_eq!(body["version"], "2.0.0", "the remembered version: {body}");

    let (_, body) = get(&app, "/v1/agents/antigravity/install?recheck=true").await;
    assert_eq!(body["version"], "2.1.0", "{body}");
}

// ---- Sign-in status ----

static ANTIGRAVITY_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MISSING_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

type Runner = fn(&std::path::Path, &[&str]) -> Option<(i32, String)>;

/// A state whose agents are at paths that don't exist and whose status command is `runner`: nothing real is run.
fn account_app(find: fn(&str) -> Option<PathBuf>, runner: Runner) -> (tempfile::TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_agent_path(find)
    .with_status_runner(runner);
    (dir, router(st))
}

fn fake_path(name: &str) -> Option<PathBuf> {
    Some(PathBuf::from(format!(r"Z:\nowhere\{name}.exe")))
}

#[tokio::test]
async fn account_is_refused_without_the_bearer() {
    let (_dir, app) = account_app(fake_path, |_, _| None);
    let req = Request::builder()
        .method("GET")
        .uri("/v1/agents/claude/account")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn account_for_an_unknown_agent_is_404() {
    let (_dir, app) = account_app(fake_path, |_, _| None);
    let (status, body) = get(&app, "/v1/agents/nope/account").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn account_for_antigravity_is_no_command() {
    use std::sync::atomic::Ordering;
    let (_dir, app) = account_app(fake_path, |_, _| {
        ANTIGRAVITY_RUNS.fetch_add(1, Ordering::SeqCst);
        None
    });
    let (status, body) = get(&app, "/v1/agents/antigravity/account").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["agent"], "antigravity");
    assert_eq!(body["sign_in"], "no_command");
    assert_eq!(ANTIGRAVITY_RUNS.load(Ordering::SeqCst), 0, "nothing ran");
}

#[tokio::test]
async fn account_for_a_missing_cli_is_unknown() {
    use std::sync::atomic::Ordering;
    let (_dir, app) = account_app(
        |_| None,
        |_, _| {
            MISSING_RUNS.fetch_add(1, Ordering::SeqCst);
            None
        },
    );
    for agent in ["claude", "codex"] {
        let (status, body) = get(&app, &format!("/v1/agents/{agent}/account")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["agent"], agent);
        assert_eq!(body["sign_in"], "unknown");
        assert_eq!(body["email"], Value::Null);
        assert_eq!(body["plan"], Value::Null);
        assert_eq!(body["method"], Value::Null);
    }
    assert_eq!(MISSING_RUNS.load(std::sync::atomic::Ordering::SeqCst), 0, "nothing ran");
}

#[tokio::test]
async fn account_reads_claude_status_from_the_runner() {
    let (_dir, app) = account_app(fake_path, |_, args| {
        assert_eq!(args, ["auth", "status"]);
        Some((
            0,
            r#"{"loggedIn":true,"authMethod":"claude.ai","email":"a@example.com","subscriptionType":"max"}"#.into(),
        ))
    });
    let (status, body) = get(&app, "/v1/agents/claude/account").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sign_in"], "signed_in");
    assert_eq!(body["email"], "a@example.com");
    assert_eq!(body["plan"], "max");
    assert_eq!(body["method"], Value::Null);
    // Milliseconds since the Unix epoch (after 2020).
    assert!(body["checked_ms"].as_u64().unwrap() > 1_600_000_000_000, "{body}");
}

#[tokio::test]
async fn account_never_returns_the_codex_output() {
    let (_dir, app) = account_app(fake_path, |_, args| {
        assert_eq!(args, ["login", "status"]);
        Some((0, "Logged in using an API key - sk-proj-***abcd\n".into()))
    });
    let (status, body) = get(&app, "/v1/agents/codex/account").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sign_in"], "signed_in");
    assert_eq!(body["method"], "an API key");
    assert!(!body.to_string().contains("sk-"), "{body}");
}

#[tokio::test]
async fn account_is_unknown_when_the_command_times_out() {
    let (_dir, app) = account_app(fake_path, |_, _| None);
    let (_, body) = get(&app, "/v1/agents/codex/account").await;
    assert_eq!(body["sign_in"], "unknown", "{body}");
}

/// Every file the settings view reads sits in this temp folder: the user's Claude, Codex and Antigravity files and a
/// managed Claude file. Nothing under the real `~/.claude`, `~/.codex` or `~/.gemini` is named, and the real
/// organization folder is replaced by the temp managed file.
struct ViewEnv {
    _dir: tempfile::TempDir,
    app: axum::Router,
    claude: PathBuf,
    managed: PathBuf,
    codex: PathBuf,
    agy: PathBuf,
}

fn view_env() -> ViewEnv {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let home = root.join("home");
    let (claude, managed) = (
        root.join("claude").join("settings.json"),
        root.join("org").join("managed.json"),
    );
    let codex = root.join("codex").join("config.toml");
    let agy_home = root.join("agyhome");
    let agy = agy_home.join(".gemini").join("antigravity-cli").join("settings.json");
    for file in [&claude, &managed, &codex, &agy] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    }
    std::fs::create_dir_all(&home).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_settings(claude.clone())
    .with_codex_hooks(root.join("codex").join("hooks.json"))
    .with_agy_home(agy_home)
    .with_claude_compact_paths(root.join("user"), vec![managed.clone()]);
    ViewEnv {
        _dir: dir,
        app: router(st),
        claude,
        managed,
        codex,
        agy,
    }
}

#[tokio::test]
async fn settings_view_is_refused_without_the_bearer() {
    let e = view_env();
    let req = Request::builder()
        .method("GET")
        .uri("/v1/agents/claude/settings-view")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&e.app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn settings_view_for_an_unknown_agent_is_404() {
    let e = view_env();
    let (status, body) = get(&e.app, "/v1/agents/nope/settings-view").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn settings_view_reads_the_claude_file() {
    let e = view_env();
    std::fs::write(
        &e.claude,
        r#"{ // a comment
          "model": "opus", "theme": "dark",
          "env": {"ANTHROPIC_API_KEY": "sk-ant-secret"},
          "apiKeyHelper": "op read op://v/i/secret-ref",
          "mcpServers": {"s": {"command": "npx", "args": ["-y", "srv", "--api-key", "sk-arg-secret"]}},
          "statusLine": {"type": "command", "command": "bar --token tok-secret"}
        }"#,
    )
    .unwrap();
    std::fs::write(&e.managed, r#"{"theme": "light", "env": {"ORG": "org-secret"}}"#).unwrap();
    let (status, body) = get(&e.app, "/v1/agents/claude/settings-view").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"]["kind"], "ok");
    assert_eq!(body["path"], e.claude.to_string_lossy().as_ref());
    let curated = |key: &str| {
        body["curated"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == key)
            .unwrap()
            .clone()
    };
    assert_eq!(
        (curated("model")["value"].as_str(), curated("model")["scope"].as_str()),
        (Some("opus"), Some("user"))
    );
    assert_eq!(
        (curated("theme")["value"].as_str(), curated("theme")["scope"].as_str()),
        (Some("light"), Some("managed"))
    );
    assert_eq!(curated("theme")["lock"]["kind"], "managed");
    assert_eq!(curated("theme")["lock"]["path"], e.managed.to_string_lossy().as_ref());
    assert_eq!(curated("model")["control"], json!({"kind": "text"}));
    assert!(curated("model").get("lock").is_none());
    let raw = body["raw"].as_str().unwrap();
    assert!(raw.contains("hidden"), "{raw}");
    // A file that holds secrets gives its hash, and no text.
    assert_eq!(body["editable"], true);
    assert_eq!(body["text"], Value::Null);
    let bytes = std::fs::read(&e.claude).unwrap();
    assert_eq!(body["hash"], marshell_core::config::hash::hash_bytes(&bytes));
    let text = body.to_string();
    for secret in [
        "sk-ant-secret",
        "secret-ref",
        "sk-arg-secret",
        "tok-secret",
        "org-secret",
    ] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
}

#[tokio::test]
async fn settings_view_reads_codex_config_toml_beside_the_hooks_file() {
    let e = view_env();
    std::fs::write(
        &e.codex,
        "model = \"gpt-5\"\n[mcp_servers.x]\ncommand = \"node\"\nargs = [\"--token\", \"abc-secret\"]\n[mcp_servers.x.env]\nAPI_KEY = \"abc-secret\"\n[mcp_servers.x.headers]\nAuthorization = \"Bearer abc-secret\"\n",
    )
    .unwrap();
    let (status, body) = get(&e.app, "/v1/agents/codex/settings-view").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"]["kind"], "ok");
    assert_eq!(body["curated"][0]["key"], "model");
    assert_eq!(body["curated"][0]["value"], "gpt-5");
    assert!(body["raw"].as_str().unwrap().contains("hidden"));
    assert!(!body.to_string().contains("abc-secret"), "{body}");
    assert_eq!((body["editable"].as_bool(), &body["text"]), (Some(true), &Value::Null));
}

#[tokio::test]
async fn settings_view_gives_the_text_of_a_file_without_secrets() {
    let e = view_env();
    let claude = "{ // a note\n  \"model\": \"opus\"\n}\n";
    let codex = "# a note\nmodel = \"gpt-5\"\n\n[profiles.x]\nmodel = \"o3\"\n";
    std::fs::write(&e.claude, claude).unwrap();
    std::fs::write(&e.codex, codex).unwrap();
    for (agent, text) in [("claude", claude), ("codex", codex)] {
        let (status, body) = get(&e.app, &format!("/v1/agents/{agent}/settings-view")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["text"], text, "{agent}");
        assert_eq!(body["editable"], true, "{agent}");
        assert_eq!(
            body["hash"],
            marshell_core::config::hash::hash_bytes(text.as_bytes()),
            "{agent}"
        );
    }
}

#[tokio::test]
async fn settings_view_reads_the_antigravity_file() {
    let e = view_env();
    std::fs::write(&e.agy, r#"{"model": "gemini", "authToken": "agy-secret"}"#).unwrap();
    let (status, body) = get(&e.app, "/v1/agents/antigravity/settings-view").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["curated"][0]["value"], "gemini");
    assert!(!body.to_string().contains("agy-secret"), "{body}");
    assert_eq!((body["editable"].as_bool(), &body["text"]), (Some(true), &Value::Null));
    assert_eq!(body["raw_lock"], "hidden");
    assert_eq!(body["curated"][0]["control"]["kind"], "text");
}

#[tokio::test]
async fn settings_view_of_a_missing_file_is_missing() {
    let e = view_env();
    for agent in ["claude", "codex", "antigravity"] {
        let (status, body) = get(&e.app, &format!("/v1/agents/{agent}/settings-view")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["state"]["kind"], "missing", "{agent}");
        assert_eq!(body["curated"].as_array().unwrap().len(), 6);
        assert_eq!(body["raw"], Value::Null);
        assert_eq!(body["editable"], true, "{agent}");
        assert_eq!((&body["hash"], &body["text"]), (&Value::Null, &Value::Null), "{agent}");
    }
}

#[tokio::test]
async fn settings_view_of_a_file_that_does_not_parse_leaks_nothing() {
    let e = view_env();
    std::fs::write(
        &e.claude,
        "{\n\"model\": \"opus\",\n\"env\": {\"K\": \"sk-broken-secret\"} oops\n",
    )
    .unwrap();
    std::fs::write(&e.codex, "model = \"gpt-5\"\napi_key = \"sk-broken-secret\"\nbad = \n").unwrap();
    for agent in ["claude", "codex"] {
        let (status, body) = get(&e.app, &format!("/v1/agents/{agent}/settings-view")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["state"]["kind"], "unparsable", "{agent}: {body}");
        assert!(body["state"]["line"].as_u64().unwrap() >= 1);
        assert_eq!(body["curated"], json!([]));
        assert_eq!(body["others"], json!([]));
        assert_eq!(body["raw"], Value::Null);
        assert_eq!(body["editable"], false, "{agent}");
        assert_eq!((&body["hash"], &body["text"]), (&Value::Null, &Value::Null), "{agent}");
        assert!(!body.to_string().contains("sk-broken-secret"), "{body}");
        assert!(!body.to_string().contains("gpt-5"), "{body}");
    }
}

#[tokio::test]
async fn settings_view_of_a_folder_is_unreadable() {
    let e = view_env();
    std::fs::create_dir_all(&e.claude).unwrap();
    let (status, body) = get(&e.app, "/v1/agents/claude/settings-view").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["state"]["kind"], "unreadable", "{body}");
    assert_eq!(body["curated"], json!([]));
    assert_eq!(body["raw"], Value::Null);
    assert_eq!(
        (body["editable"].as_bool(), &body["hash"], &body["text"]),
        (Some(false), &Value::Null, &Value::Null)
    );
}

// ---- planning and undoing a settings change ----

/// Claude's and Codex's settings sit inside a recorded project, so the config service may write them. The managed
/// Claude file is outside it, so the service could never write it, and only the view reads it.
struct PlanEnv {
    _dir: tempfile::TempDir,
    app: axum::Router,
    claude: PathBuf,
    managed: PathBuf,
    codex: PathBuf,
}

fn plan_env() -> PlanEnv {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    let claude = project.join("claude").join("settings.json");
    let codex = project.join("codex").join("config.toml");
    let managed = root.join("org").join("managed.json");
    for file in [&claude, &codex, &managed] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    }
    std::fs::create_dir_all(&home).unwrap();
    record(&home, &project, 1).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_settings(claude.clone())
    .with_codex_hooks(project.join("codex").join("hooks.json"))
    .with_claude_compact_paths(root.join("user"), vec![managed.clone()]);
    PlanEnv {
        _dir: dir,
        app: router(st),
        claude,
        managed,
        codex,
    }
}

impl PlanEnv {
    fn path(&self, agent: &str) -> &PathBuf {
        if agent == "claude" { &self.claude } else { &self.codex }
    }

    /// The hash a form sends: the file's now, or null when it is missing.
    fn hash(&self, agent: &str) -> Value {
        match std::fs::read(self.path(agent)) {
            Ok(bytes) => json!(marshell_core::config::hash::hash_bytes(&bytes)),
            Err(_) => Value::Null,
        }
    }

    fn write(&self, agent: &str, text: &str) {
        std::fs::write(self.path(agent), text).unwrap();
    }

    fn read(&self, agent: &str) -> String {
        std::fs::read_to_string(self.path(agent)).unwrap()
    }

    /// A form edit against the file as it is now.
    async fn form(&self, agent: &str, edits: Value) -> (StatusCode, Vec<u8>) {
        let body = json!({"expected_hash": self.hash(agent), "change": {"kind": "form", "edits": edits}});
        post_raw(&self.app, &format!("/v1/agents/{agent}/settings/plan"), body).await
    }

    async fn raw(&self, agent: &str, text: &str) -> (StatusCode, Vec<u8>) {
        let body = json!({"expected_hash": self.hash(agent), "change": {"kind": "raw", "text": text}});
        post_raw(&self.app, &format!("/v1/agents/{agent}/settings/plan"), body).await
    }

    async fn undo(&self, agent: &str, snapshot_id: &str) -> (StatusCode, Vec<u8>) {
        let body = json!({"snapshot_id": snapshot_id});
        post_raw(&self.app, &format!("/v1/agents/{agent}/settings/undo"), body).await
    }

    /// Plans a form edit, expects 200, applies it and returns the snapshot id.
    async fn change(&self, agent: &str, edits: Value) -> String {
        let (status, bytes) = self.form(agent, edits).await;
        assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
        let plan: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(plan["preview"]["can_apply"], true, "{plan}");
        let (status, applied) = post(&self.app, "/v1/config/apply", shown(&plan["preview"])).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        applied["snapshot_id"].as_str().unwrap().to_string()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or_else(|_| panic!("not JSON: {}", text(bytes)))
}

fn edit(key: &str, value: &str) -> Value {
    json!([{"key": key, "value": value}])
}

#[tokio::test]
async fn settings_plan_is_refused_without_the_bearer() {
    let e = plan_env();
    let req = Request::builder()
        .method("POST")
        .uri("/v1/agents/claude/settings/plan")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"expected_hash": null, "change": {"kind": "form", "edits": []}}).to_string(),
        ))
        .unwrap();
    let (status, _) = send(&e.app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn settings_undo_is_refused_without_the_bearer() {
    let e = plan_env();
    let req = Request::builder()
        .method("POST")
        .uri("/v1/agents/claude/settings/undo")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"snapshot_id": "s1"}).to_string()))
        .unwrap();
    let (status, _) = send(&e.app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn settings_plan_for_an_unknown_agent_is_404() {
    let e = plan_env();
    let (status, _) = e.form("nope", json!([])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = e.undo("nope", "s1").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_claude_form_edit_plans_applies_and_writes_the_key() {
    let e = plan_env();
    e.write("claude", "{\n  \"theme\": \"dark\"\n}\n");
    let (status, bytes) = e.form("claude", edit("model", "\"sonnet\"")).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    let plan = json_of(&bytes);
    assert_eq!(plan["normalized"], false);
    assert_eq!(plan["preview"]["title"], "Change Claude Code settings");
    assert_eq!(plan["preview"]["reason"], "Edited in Settings.");
    assert_eq!(plan["preview"]["can_apply"], true);
    assert_eq!(plan["preview"]["files"][0]["path"], e.claude.to_string_lossy().as_ref());
    assert_eq!(plan["preview"]["files"][0]["applies"], "newSessions");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let now: Value = serde_json::from_str(&e.read("claude")).unwrap();
    assert_eq!(now, json!({"theme": "dark", "model": "sonnet"}));
}

#[tokio::test]
async fn a_claude_form_edit_can_remove_a_key_and_set_a_nested_one() {
    let e = plan_env();
    e.write(
        "claude",
        "{\"model\": \"opus\", \"permissions\": {\"allow\": [\"Bash\"]}}\n",
    );
    e.change(
        "claude",
        json!([
            {"key": "model"},
            {"key": "permissions.defaultMode", "value": "\"plan\""},
            {"key": "cleanupPeriodDays", "value": "30"}
        ]),
    )
    .await;
    let now: Value = serde_json::from_str(&e.read("claude")).unwrap();
    assert_eq!(
        now,
        json!({"permissions": {"allow": ["Bash"], "defaultMode": "plan"}, "cleanupPeriodDays": 30})
    );
}

const COMMENTED_TOML: &str = "# mine\nmodel = \"a\" # why\n\n[profiles.x]\nmodel = \"b\"\n";

#[tokio::test]
async fn a_codex_form_edit_keeps_comments() {
    let e = plan_env();
    e.write("codex", COMMENTED_TOML);
    e.change("codex", edit("model", "\"c\"")).await;
    assert_eq!(
        e.read("codex"),
        "# mine\nmodel = \"c\" # why\n\n[profiles.x]\nmodel = \"b\"\n"
    );
}

#[tokio::test]
async fn undo_puts_back_the_identical_bytes() {
    let e = plan_env();
    let claude: &[u8] = b"\xEF\xBB\xBF{\r\n  // keep\r\n  \"model\": \"opus\"\r\n}\r\n";
    std::fs::write(&e.claude, claude).unwrap();
    e.write("codex", COMMENTED_TOML);
    for (agent, value) in [("claude", "\"sonnet\""), ("codex", "\"c\"")] {
        let before = std::fs::read(e.path(agent)).unwrap();
        let snapshot = e.change(agent, edit("model", value)).await;
        assert_ne!(std::fs::read(e.path(agent)).unwrap(), before, "{agent}");
        let (status, bytes) = e.undo(agent, &snapshot).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {}", text(&bytes));
        let result = json_of(&bytes);
        assert_eq!(
            result["files"][0]["after_hash"],
            marshell_core::config::hash::hash_bytes(&before),
            "{agent}"
        );
        assert_eq!(std::fs::read(e.path(agent)).unwrap(), before, "{agent}");
    }
}

#[tokio::test]
async fn undo_after_a_hand_edit_is_409() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let snapshot = e.change("claude", edit("model", "\"sonnet\"")).await;
    e.write("claude", "{\"model\": \"mine\"}\n");
    let (status, bytes) = e.undo("claude", &snapshot).await;
    assert_eq!(status, StatusCode::CONFLICT, "{}", text(&bytes));
    let conflict = json_of(&bytes);
    assert_eq!(conflict["reason"], "changed");
    assert_eq!(conflict["paths"], json!([e.claude.to_string_lossy()]));
    assert_eq!(e.read("claude"), "{\"model\": \"mine\"}\n");
}

#[tokio::test]
async fn undo_of_another_files_snapshot_is_422() {
    let e = plan_env();
    let other = e.claude.parent().unwrap().join("other.json");
    let (status, preview) = post(
        &e.app,
        "/v1/config/preview",
        json!({"title": "t", "reason": "r", "edits": [{
            "path": other.to_string_lossy(), "scope": "user", "applies": "newSessions",
            "op": {"kind": "createFile", "content": "{}\n"}
        }]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let snapshot = applied["snapshot_id"].as_str().unwrap();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let (status, bytes) = e.undo("claude", snapshot).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{}", text(&bytes));
    assert!(other.exists());
    assert_eq!(e.read("claude"), "{\"model\": \"opus\"}\n");
    let (status, _) = e.undo("claude", "no-such-snapshot").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_form_edit_on_a_missing_file_creates_it_and_undo_removes_it() {
    let e = plan_env();
    for agent in ["claude", "codex"] {
        assert!(!e.path(agent).exists());
        let snapshot = e.change(agent, edit("model", "\"sonnet\"")).await;
        if agent == "claude" {
            let now: Value = serde_json::from_str(&e.read(agent)).unwrap();
            assert_eq!(now, json!({"model": "sonnet"}));
        } else {
            assert_eq!(e.read(agent), "model = \"sonnet\"\n");
        }
        let (status, bytes) = e.undo(agent, &snapshot).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {}", text(&bytes));
        assert!(!e.path(agent).exists(), "{agent}");
    }
}

#[tokio::test]
async fn a_plan_against_an_old_hash_is_409() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let body = json!({
        "expected_hash": marshell_core::config::hash::hash_bytes(b"something else"),
        "change": {"kind": "form", "edits": edit("model", "\"sonnet\"")}
    });
    let (status, bytes) = post_raw(&e.app, "/v1/agents/claude/settings/plan", body).await;
    assert_eq!(status, StatusCode::CONFLICT, "{}", text(&bytes));
    assert_eq!(json_of(&bytes)["reason"], "changed");
    assert_eq!(json_of(&bytes)["paths"], json!([e.claude.to_string_lossy()]));
    // A form opened on a missing file, and the file has appeared since.
    let body = json!({"expected_hash": null, "change": {"kind": "form", "edits": edit("model", "\"sonnet\"")}});
    let (status, _) = post_raw(&e.app, "/v1/agents/claude/settings/plan", body).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e.read("claude"), "{\"model\": \"opus\"}\n");
}

#[tokio::test]
async fn a_hand_edit_between_plan_and_apply_is_409() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let (_, bytes) = e.form("claude", edit("model", "\"sonnet\"")).await;
    let plan = json_of(&bytes);
    e.write("claude", "{\"model\": \"mine\"}\n");
    let (status, _) = post(&e.app, "/v1/config/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e.read("claude"), "{\"model\": \"mine\"}\n");
}

const SECRET: &str = "sk-live-9";

#[tokio::test]
async fn the_diff_never_shows_a_secret() {
    let e = plan_env();
    e.write(
        "claude",
        &format!(
            "{{\n  \"model\": \"opus\",\n  \"env\": {{\"ANTHROPIC_API_KEY\": \"{SECRET}\"}},\n  \"apiKeyHelper\": \"get-key --token {SECRET}\"\n}}\n"
        ),
    );
    let (status, bytes) = e.form("claude", edit("model", "\"sonnet\"")).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    assert!(!text(&bytes).contains(SECRET), "{}", text(&bytes));
    let plan = json_of(&bytes);
    assert_eq!(plan["normalized"], true);
    let diff = plan["preview"]["files"][0]["diff"].as_str().unwrap();
    assert!(diff.contains("sonnet") && diff.contains("hidden"), "{diff}");
    let id = plan["preview"]["changeset_id"].as_str().unwrap();
    let uri = format!("/v1/config/preview/{id}");
    let (status, stored) = send(&e.app, base("GET", &uri).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!text(&stored).contains(SECRET), "{}", text(&stored));
    assert_eq!(json_of(&stored), plan["preview"]);
    // The apply still writes the real bytes, secret included.
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(e.read("claude").contains(SECRET));
    assert!(!applied.to_string().contains(SECRET));
}

#[tokio::test]
async fn the_diff_of_a_codex_file_with_a_secret_is_masked_too() {
    let e = plan_env();
    e.write(
        "codex",
        &format!("model = \"gpt-5\"\n[mcp_servers.x.env]\nAPI_KEY = \"{SECRET}\"\n"),
    );
    let (status, bytes) = e.form("codex", edit("model", "\"o3\"")).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    assert!(!text(&bytes).contains(SECRET), "{}", text(&bytes));
    assert_eq!(json_of(&bytes)["normalized"], true);
}

#[tokio::test]
async fn undo_of_a_file_with_a_secret_stores_no_secret() {
    let e = plan_env();
    let original = format!("{{\"model\": \"opus\", \"env\": {{\"K\": \"{SECRET}\"}}}}\n");
    e.write("claude", &original);
    let snapshot = e.change("claude", edit("model", "\"sonnet\"")).await;
    let (status, bytes) = e.undo("claude", &snapshot).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    assert!(!text(&bytes).contains(SECRET));
    assert_eq!(e.read("claude"), original);
    // A hand edit that leaves a secret in the file: the 409 names no value.
    let snapshot = e.change("claude", edit("model", "\"haiku\"")).await;
    e.write("claude", &format!("{{\"env\": {{\"K\": \"{SECRET}-2\"}}}}\n"));
    let (status, bytes) = e.undo("claude", &snapshot).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(!text(&bytes).contains(SECRET));
}

#[tokio::test]
async fn a_file_without_secrets_shows_its_real_diff() {
    let e = plan_env();
    e.write("claude", "{\n  // keep me\n  \"model\": \"opus\"\n}\n");
    let (status, bytes) = e.form("claude", edit("model", "\"sonnet\"")).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    let plan = json_of(&bytes);
    assert_eq!(plan["normalized"], false);
    let diff = plan["preview"]["files"][0]["diff"].as_str().unwrap();
    assert!(
        diff.lines().any(|l| l.starts_with(' ') && l.contains("// keep me")),
        "{diff}"
    );
    assert!(diff.contains("-  \"model\": \"opus\""), "{diff}");
    assert!(diff.contains("+  \"model\": \"sonnet\""), "{diff}");
}

#[tokio::test]
async fn bad_form_edits_are_422_with_the_exact_message() {
    let e = plan_env();
    let managed_path = e.managed.to_string_lossy().into_owned();
    std::fs::write(&e.managed, r#"{"theme": "light"}"#).unwrap();
    let table = "approval_policy = { granular = { rules = true, sandbox_approval = true } }\n";
    // (agent, the file's text, the edits, the message)
    let cases: Vec<(&str, &str, Value, String)> = vec![
        (
            "claude",
            "{}",
            edit("env.X", "\"1\""),
            "env.X isn't one of the settings this form changes.".into(),
        ),
        (
            "codex",
            "",
            json!([{"key": "mcp_servers"}]),
            "mcp_servers isn't one of the settings this form changes.".into(),
        ),
        (
            "claude",
            "{}",
            edit("theme", "\"dark\""),
            format!("theme is set by {managed_path}, so Marshell can't change it here."),
        ),
        (
            "codex",
            table,
            edit("approval_policy", "\"never\""),
            "approval_policy is set as a table in the file, so the form can't change it.".into(),
        ),
        (
            "claude",
            "{\"model\": \"TOKEN=abc-secret\"}",
            edit("model", "\"sonnet\""),
            "model holds a secret value, so the form can't change it.".into(),
        ),
        (
            "codex",
            "model = \"API_KEY=abc-secret\"\n",
            json!([{"key": "model"}]),
            "model holds a secret value, so the form can't change it.".into(),
        ),
        (
            "claude",
            "{}",
            edit("cleanupPeriodDays", "0"),
            "cleanupPeriodDays needs a whole number of at least 1.".into(),
        ),
        (
            "claude",
            "{}",
            edit("cleanupPeriodDays", "1.5"),
            "cleanupPeriodDays needs a whole number of at least 1.".into(),
        ),
        (
            "claude",
            "{}",
            edit("cleanupPeriodDays", "\"30\""),
            "cleanupPeriodDays needs a whole number of at least 1.".into(),
        ),
        (
            "codex",
            "",
            edit("project_doc_max_bytes", "-1"),
            "project_doc_max_bytes needs a whole number of at least 0.".into(),
        ),
        (
            "claude",
            "{}",
            edit("effortLevel", "\"max\""),
            "effortLevel needs one of: low, medium, high, xhigh.".into(),
        ),
        (
            "codex",
            "",
            edit("approval_policy", "\"on-failure\""),
            "approval_policy needs one of: on-request, never.".into(),
        ),
        (
            "codex",
            "",
            edit("check_for_update_on_startup", "\"yes\""),
            "check_for_update_on_startup needs on or off.".into(),
        ),
        ("claude", "{}", edit("model", "\"\""), "model needs text.".into()),
        ("claude", "{}", edit("model", "7"), "model needs text.".into()),
        ("claude", "{}", edit("model", "not json"), "model needs text.".into()),
    ];
    for (agent, file, edits, message) in cases {
        e.write(agent, file);
        let (status, bytes) = e.form(agent, edits.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{agent} {edits}");
        assert_eq!(text(&bytes), message, "{agent} {edits}");
        assert!(!text(&bytes).contains("abc-secret"));
        assert_eq!(e.read(agent), file, "nothing is written");
    }
}

#[tokio::test]
async fn an_unparsable_file_and_an_empty_change_are_422() {
    let e = plan_env();
    e.write(
        "claude",
        "{\n  \"model\": \"opus\",\n  \"env\": {\"K\": \"sk-broken\"} oops\n",
    );
    let (status, bytes) = e.form("claude", edit("model", "\"sonnet\"")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let message = text(&bytes);
    assert!(
        message.starts_with("The file doesn't parse (line ") && message.ends_with("), so Marshell doesn't change it."),
        "{message}"
    );
    assert!(!message.contains("sk-broken"));
    e.write("codex", "model = \nx = 1\n");
    let (status, bytes) = e.form("codex", edit("model", "\"o3\"")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        text(&bytes),
        "The file doesn't parse (line 1), so Marshell doesn't change it."
    );
    // The value is already there, and removing a key that isn't there changes nothing.
    e.write("claude", "{\"model\": \"opus\"}\n");
    for edits in [edit("model", "\"opus\""), json!([{"key": "theme"}]), json!([])] {
        let (status, bytes) = e.form("claude", edits).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(text(&bytes), "Nothing changes.");
    }
}

#[tokio::test]
async fn raw_replaces_the_whole_text() {
    let e = plan_env();
    e.write("claude", "{\n  \"model\": \"opus\"\n}\n");
    let (status, bytes) = e.raw("claude", "{\"model\": \"haiku\"}\n").await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    let plan = json_of(&bytes);
    assert_eq!(plan["normalized"], false);
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(e.read("claude"), "{\"model\": \"haiku\"}\n");
    e.write("codex", COMMENTED_TOML);
    let (status, bytes) = e.raw("codex", "# new\nmodel = \"z\"\n").await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&json_of(&bytes)["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(e.read("codex"), "# new\nmodel = \"z\"\n");
}

#[tokio::test]
async fn raw_that_adds_a_secret_is_masked_in_the_diff() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let new = format!("{{\"model\": \"opus\", \"env\": {{\"K\": \"{SECRET}\"}}}}\n");
    let (status, bytes) = e.raw("claude", &new).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    assert!(!text(&bytes).contains(SECRET));
    assert_eq!(json_of(&bytes)["normalized"], true);
}

#[tokio::test]
async fn raw_on_a_file_with_secrets_is_422() {
    let e = plan_env();
    let message = "This file holds secret values, so Marshell doesn't edit it as text.";
    e.write("claude", &format!("{{\"env\": {{\"K\": \"{SECRET}\"}}}}\n"));
    let (status, bytes) = e.raw("claude", "{}").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), message);
    e.write("codex", &format!("[mcp_servers.x.env]\nAPI_KEY = \"{SECRET}\"\n"));
    let (status, bytes) = e.raw("codex", "").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), message);
    // A comment that names a secret counts as one.
    e.write("claude", "{\n  // api_key = 1\n  \"model\": \"opus\"\n}\n");
    let (status, bytes) = e.raw("claude", "{}").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), message);
}

#[tokio::test]
async fn raw_that_does_not_parse_names_the_line() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let (status, bytes) = e.raw("claude", "{\n  \"model\": \"a\",\n  \"x\": \n}\n").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), "The text doesn't parse at line 4.");
    let (status, bytes) = e.raw("claude", "[1]").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), "The text doesn't parse at line 1.");
    e.write("codex", "model = \"a\"\n");
    let (status, bytes) = e.raw("codex", "model = \"a\"\nx = \n").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), "The text doesn't parse at line 2.");
}

#[tokio::test]
async fn claude_raw_with_a_comment_is_refused() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"opus\"}\n");
    let (status, bytes) = e.raw("claude", "{\n  // mine\n  \"model\": \"a\"\n}\n").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        text(&bytes).starts_with("The text doesn't parse at line "),
        "{}",
        text(&bytes)
    );
    assert_eq!(e.read("claude"), "{\"model\": \"opus\"}\n");
}

#[tokio::test]
async fn raw_on_a_missing_file_is_422() {
    let e = plan_env();
    let (status, bytes) = e.raw("claude", "{}").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(&bytes), "There is no file to edit as text.");
    assert!(!e.claude.exists());
}

/// The hash a form sends for the file at `path`: the file's now, or null when it is missing.
fn hash_at(path: &std::path::Path) -> Value {
    match std::fs::read(path) {
        Ok(bytes) => json!(marshell_core::config::hash::hash_bytes(&bytes)),
        Err(_) => Value::Null,
    }
}

/// Plans a change to Antigravity's settings file, expects 200, applies it and returns the snapshot id.
async fn agy_change(e: &AgyEnv, expected_hash: Value, change: Value) -> String {
    let body = json!({"expected_hash": expected_hash, "change": change});
    let (status, bytes) = post_raw(&e.app, "/v1/agents/antigravity/settings/plan", body).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    let plan = json_of(&bytes);
    assert_eq!(plan["preview"]["can_apply"], true, "{plan}");
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&plan["preview"])).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    applied["snapshot_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn antigravity_form_edit_plans_applies_and_undoes() {
    let e = agy_env();
    let original = "{\"model\": \"x\"}";
    std::fs::create_dir_all(e.settings.parent().unwrap()).unwrap();
    std::fs::write(&e.settings, original).unwrap();
    let snapshot = agy_change(
        &e,
        hash_at(&e.settings),
        json!({"kind": "form", "edits": edit("toolPermission", "\"strict\"")}),
    )
    .await;
    let now = std::fs::read_to_string(&e.settings).unwrap();
    assert!(now.contains("\"toolPermission\": \"strict\""), "{now}");
    assert!(now.contains("\"model\": \"x\""), "{now}");
    let body = json!({"snapshot_id": snapshot});
    let (status, bytes) = post_raw(&e.app, "/v1/agents/antigravity/settings/undo", body).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    assert_eq!(std::fs::read(&e.settings).unwrap(), original.as_bytes());
}

#[tokio::test]
async fn antigravity_form_edit_creates_a_missing_file() {
    let e = agy_env();
    assert!(!e.settings.exists() && !e.settings.parent().unwrap().exists());
    let snapshot = agy_change(
        &e,
        Value::Null,
        json!({"kind": "form", "edits": edit("verbosity", "\"low\"")}),
    )
    .await;
    let now = std::fs::read_to_string(&e.settings).unwrap();
    assert!(now.contains("\"verbosity\": \"low\""), "{now}");
    let body = json!({"snapshot_id": snapshot});
    let (status, bytes) = post_raw(&e.app, "/v1/agents/antigravity/settings/undo", body).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
    assert!(!e.settings.exists());
}

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// How many byte-order marks the file starts with.
fn leading_boms(bytes: &[u8]) -> usize {
    let mut n = 0;
    while bytes[n * 3..].starts_with(&BOM) {
        n += 1;
    }
    n
}

impl PlanEnv {
    /// Plans a raw edit against the file as it is now, expects 200 and applies it.
    async fn apply_raw(&self, agent: &str, text_in: &str) {
        let (status, bytes) = self.raw(agent, text_in).await;
        assert_eq!(status, StatusCode::OK, "{}", text(&bytes));
        let plan = json_of(&bytes);
        let (status, applied) = post(&self.app, "/v1/config/apply", shown(&plan["preview"])).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
    }
}

#[tokio::test]
async fn a_codex_form_edit_keeps_the_files_bom() {
    let e = plan_env();
    e.write("codex", "\u{feff}model = \"a\"\n");
    e.change("codex", edit("model", "\"b\"")).await;
    let bytes = std::fs::read(&e.codex).unwrap();
    assert_eq!(leading_boms(&bytes), 1, "{bytes:?}");
    assert!(text(&bytes).contains("model = \"b\""));
}

#[tokio::test]
async fn a_claude_form_edit_keeps_the_files_bom() {
    let e = plan_env();
    e.write("claude", "\u{feff}{\"model\": \"a\"}");
    e.change("claude", edit("model", "\"b\"")).await;
    let bytes = std::fs::read(&e.claude).unwrap();
    assert_eq!(leading_boms(&bytes), 1, "{bytes:?}");
    assert!(text(&bytes).contains("\"b\""));
}

#[tokio::test]
async fn the_telemetry_row_is_refused() {
    let e = agy_env();
    let original = "{\"model\": \"x\"}";
    std::fs::create_dir_all(e.settings.parent().unwrap()).unwrap();
    std::fs::write(&e.settings, original).unwrap();
    let body = json!({
        "expected_hash": hash_at(&e.settings),
        "change": {"kind": "form", "edits": edit("enableTelemetry", "true")},
    });
    let (status, bytes) = post_raw(&e.app, "/v1/agents/antigravity/settings/plan", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        text(&bytes),
        "Antigravity keeps this with your Google account, not in this file. Change it with /config in Antigravity."
    );
    assert_eq!(std::fs::read_to_string(&e.settings).unwrap(), original);
}

#[tokio::test]
async fn a_raw_save_keeps_the_files_bom() {
    let e = plan_env();
    // Claude Code: the file had a BOM, the sent text has none.
    e.write("claude", "\u{feff}{\"model\": \"a\"}");
    e.apply_raw("claude", "{\"model\": \"b\"}").await;
    let bytes = std::fs::read(&e.claude).unwrap();
    assert_eq!(leading_boms(&bytes), 1, "{bytes:?}");
    assert!(text(&bytes).contains("\"b\""));
    // The sent text starts with a BOM itself: still one.
    e.write("claude", "\u{feff}{\"model\": \"a\"}");
    e.apply_raw("claude", "\u{feff}{\"model\": \"c\"}").await;
    let bytes = std::fs::read(&e.claude).unwrap();
    assert_eq!(leading_boms(&bytes), 1, "{bytes:?}");
    assert!(text(&bytes).contains("\"c\""));
    // No BOM in the file: a pasted one adds none.
    e.write("claude", "{\"model\": \"a\"}");
    e.apply_raw("claude", "\u{feff}{\"model\": \"d\"}").await;
    let bytes = std::fs::read(&e.claude).unwrap();
    assert_eq!(leading_boms(&bytes), 0, "{bytes:?}");
    assert!(text(&bytes).contains("\"d\""));
    // Codex.
    e.write("codex", "\u{feff}model = \"a\"\n");
    e.apply_raw("codex", "model = \"b\"\n").await;
    let bytes = std::fs::read(&e.codex).unwrap();
    assert_eq!(leading_boms(&bytes), 1, "{bytes:?}");
    assert!(text(&bytes).contains("model = \"b\""));
}

#[tokio::test]
async fn a_raw_save_is_refused_for_each_raw_lock() {
    let e = plan_env();
    let secret = format!("{{\"env\": {{\"K\": \"{SECRET}\"}}}}\n");
    let cases = [
        (
            "{\n  // mine\n  \"model\": \"a\"\n}\n",
            "Raw editing is off: this file isn't plain JSON, and Marshell saves it as plain JSON.",
        ),
        (
            "{\"model\": \"a\", \"model\": \"b\"}",
            "Raw editing is off: a key appears twice in this file.",
        ),
        (
            secret.as_str(),
            "This file holds secret values, so Marshell doesn't edit it as text.",
        ),
    ];
    for (file, message) in cases {
        e.write("claude", file);
        let (status, bytes) = e.raw("claude", "{}").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{file}");
        assert_eq!(text(&bytes), message, "{file}");
        assert_eq!(e.read("claude"), file);
    }
}

#[tokio::test]
async fn a_row_whose_value_masking_changed_is_locked_in_the_view() {
    let e = plan_env();
    e.write("claude", "{\"model\": \"TOKEN=abc-secret\"}");
    let (status, body) = get(&e.app, "/v1/agents/claude/settings-view").await;
    assert_eq!(status, StatusCode::OK);
    let rows = body["curated"].as_array().unwrap();
    let model = rows.iter().find(|s| s["key"] == "model").unwrap();
    assert_eq!(model["lock"], json!({"kind": "hidden"}));
    assert!(!body.to_string().contains("abc-secret"));
}
