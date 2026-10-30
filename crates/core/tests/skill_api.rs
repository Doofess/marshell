//! The skill's setup state and its plan routes, through the real router and the config service. Every path sits
//! under a temp folder: the state is told the user home with `with_skill_home`, which also ignores the process
//! environment, so a developer's own `CLAUDE_CONFIG_DIR` cannot move the files.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::agents::skill::SKILL_TEXT;
use marshell_core::projects::{canonical, record};
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use std::path::PathBuf;
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    /// The user home every skill path is under.
    user: PathBuf,
    settings: PathBuf,
}

/// A temp Marshell home and a recorded project that holds the user home, the settings file and Codex's hooks, so
/// the config service may write them. The skill home, Codex's hooks and Antigravity's home are all handed over.
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let home = root.join("home");
    let project = root.join("project");
    let user = project.join("user");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&user).unwrap();
    record(&home, &project, 1).unwrap();
    let settings = project.join("settings.json");
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home,
        Vec::new(),
    )
    .with_claude_settings(settings.clone())
    .with_codex_hooks(project.join("codex").join("hooks.json"))
    .with_agy_home(user.clone())
    .with_skill_home(user.clone())
    .with_agent_path(|_| None);
    Env {
        _dir: dir,
        app: router(st),
        user,
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

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    send(app, base("GET", uri).body(Body::empty()).unwrap()).await
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
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

/// Where `agent`'s skill file goes under `user`, by Task 1's layout.
fn skill_file(user: &std::path::Path, agent: &str) -> PathBuf {
    let root = match agent {
        "claude" => user.join(".claude"),
        "codex" => user.join(".agents"),
        _ => user.join(".gemini").join("config"),
    };
    root.join("skills").join("marshell").join("SKILL.md")
}

fn paths_of(preview: &Value) -> Vec<String> {
    preview["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn skill_routes_refuse_without_the_bearer() {
    let e = env();
    for route in ["skill/plan", "skill/remove-plan"] {
        let req = Request::builder()
            .method("POST")
            .uri(format!("/v1/agents/claude/{route}"))
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let (status, _) = send(&e.app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{route}");
    }
}

#[tokio::test]
async fn skill_routes_refuse_an_unknown_agent_like_the_hooks_routes() {
    let e = env();
    for route in ["skill/plan", "skill/remove-plan"] {
        let (status, _) = post(&e.app, &format!("/v1/agents/nope/{route}"), json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{route}");
    }
}

#[tokio::test]
async fn skill_installs_updates_and_removes_through_the_config_service() {
    for agent in ["claude", "codex", "antigravity"] {
        let e = env();
        let file = skill_file(&e.user, agent);
        let setup_uri = format!("/v1/agents/{agent}/setup");

        let (status, setup) = get(&e.app, &setup_uri).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {setup}");
        assert_eq!(setup["skill"], "none", "{agent}");
        assert_eq!(setup["skill_path"], file.to_string_lossy().as_ref(), "{agent}");

        // Install: the parent folders do not exist yet.
        let (status, preview) = post(&e.app, &format!("/v1/agents/{agent}/skill/plan"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {preview}");
        assert_eq!(paths_of(&preview), vec![file.to_string_lossy().to_string()], "{agent}");
        assert!(!file.exists(), "{agent}: a preview writes nothing");
        let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {applied}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), SKILL_TEXT, "{agent}");
        let (_, setup) = get(&e.app, &setup_uri).await;
        assert_eq!(setup["skill"], "installed", "{agent}");

        // Installed already: a preview with no files.
        let (status, preview) = post(&e.app, &format!("/v1/agents/{agent}/skill/plan"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {preview}");
        assert!(paths_of(&preview).is_empty(), "{agent}: {preview}");

        // The person's own text: shown as a diff, then replaced.
        std::fs::write(&file, b"mine").unwrap();
        let (_, setup) = get(&e.app, &setup_uri).await;
        assert_eq!(setup["skill"], "edited", "{agent}");
        let (status, preview) = post(&e.app, &format!("/v1/agents/{agent}/skill/plan"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {preview}");
        assert!(
            preview["title"]
                .as_str()
                .unwrap()
                .starts_with("Update the marshell skill")
        );
        let diff = preview["files"][0]["diff"].as_str().unwrap();
        assert!(
            diff.contains("-mine") && diff.contains("+name: marshell"),
            "{agent}: {diff}"
        );
        let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {applied}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), SKILL_TEXT, "{agent}");
        let (_, setup) = get(&e.app, &setup_uri).await;
        assert_eq!(setup["skill"], "installed", "{agent}");

        // Remove: the file and its folder go, `skills` stays.
        let (status, preview) = post(&e.app, &format!("/v1/agents/{agent}/skill/remove-plan"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {preview}");
        let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
        assert_eq!(status, StatusCode::OK, "{agent}: {applied}");
        assert!(!file.exists(), "{agent}");
        assert!(!file.parent().unwrap().exists(), "{agent}: the marshell folder goes");
        assert!(
            file.parent().unwrap().parent().unwrap().is_dir(),
            "{agent}: skills stays"
        );
        let (_, setup) = get(&e.app, &setup_uri).await;
        assert_eq!(setup["skill"], "none", "{agent}");

        // Nothing was written outside the skill home and the settings file's folder.
        let strays: Vec<_> = std::fs::read_dir(e.settings.parent().unwrap())
            .unwrap()
            .map(|d| d.unwrap().file_name())
            .collect();
        assert!(
            strays
                .iter()
                .all(|n| n == "user" || n == "codex" || n == "settings.json"),
            "{agent}: {strays:?}"
        );
    }
}

#[tokio::test]
async fn removing_keeps_a_marshell_folder_that_holds_other_files() {
    let e = env();
    let file = skill_file(&e.user, "codex");
    let (_, preview) = post(&e.app, "/v1/agents/codex/skill/plan", json!({})).await;
    post(&e.app, "/v1/config/apply", shown(&preview)).await;
    std::fs::write(file.with_file_name("notes.md"), b"keep").unwrap();
    let (_, preview) = post(&e.app, "/v1/agents/codex/skill/remove-plan", json!({})).await;
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(!file.exists());
    assert!(file.with_file_name("notes.md").exists());
}

#[tokio::test]
async fn skill_conflict_writes_nothing() {
    let e = env();
    let file = skill_file(&e.user, "codex");
    let (status, preview) = post(&e.app, "/v1/agents/codex/skill/plan", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"other").unwrap();
    let (status, body) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(std::fs::read(&file).unwrap(), b"other");
}

#[tokio::test]
async fn hooks_plan_with_skill_adds_the_skill_file() {
    let e = env();
    let skill = skill_file(&e.user, "claude").to_string_lossy().to_string();
    let settings = e.settings.to_string_lossy().to_string();
    let want = |skill: bool| json!({"status": true, "statusline": true, "approve": true, "skill": skill});

    let (status, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", want(true)).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let paths = paths_of(&preview);
    assert!(paths.contains(&settings) && paths.contains(&skill), "{paths:?}");

    let (status, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", want(false)).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    // The hooks' own files (the settings and the status line chain), and not the skill.
    let hooks_only = paths_of(&preview);
    assert!(
        hooks_only.contains(&settings) && !hooks_only.contains(&skill),
        "{hooks_only:?}"
    );
    let mut with_skill = hooks_only.clone();
    with_skill.push(skill.clone());
    let (mut got, mut want_paths) = (paths, with_skill);
    got.sort();
    want_paths.sort();
    assert_eq!(got, want_paths);

    // The skill is already in place: only the settings file.
    let (_, preview) = post(&e.app, "/v1/agents/claude/skill/plan", json!({})).await;
    let (status, applied) = post(&e.app, "/v1/config/apply", shown(&preview)).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let (status, preview) = post(&e.app, "/v1/agents/claude/hooks/plan", want(true)).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(paths_of(&preview), hooks_only);
}
