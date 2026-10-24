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
async fn antigravity_is_still_404() {
    let e = env();
    let (status, _) = get(&e.app, "/v1/agents/antigravity/setup").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&e.app, "/v1/agents/antigravity/hooks/plan", WANT()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&e.app, "/v1/agents/antigravity/hooks/remove-plan", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&e.app, "/v1/agents/nope/setup").await;
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
