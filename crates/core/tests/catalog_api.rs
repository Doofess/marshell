//! `GET /v1/agents/catalog`, `GET /v1/agents/own` and `PUT /v1/agents/own`. Everything runs in a temp Marshell home.
//! No program is started: the PATH lookup and the `--version` reader are injected, and the only file that "resolves"
//! is an empty `grok-marshell-test.exe` in the temp folder.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::agents::identify::VersionCache;
use marshell_core::paths;
use marshell_core::server::{AppState, router};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;

/// Where the injected PATH lookup says `grok` is. An empty file made by `build`, never run.
fn grok_stand_in() -> PathBuf {
    std::env::temp_dir()
        .join(format!("marshell-catalog-api-{}", std::process::id()))
        .join("grok-marshell-test.exe")
}

fn only_grok(command: &str) -> Option<PathBuf> {
    (command == "grok").then(grok_stand_in)
}

struct Env {
    _dir: tempfile::TempDir,
    app: axum::Router,
    agents_file: PathBuf,
}

fn build() -> Env {
    let stand_in = grok_stand_in();
    std::fs::create_dir_all(stand_in.parent().unwrap()).unwrap();
    std::fs::write(&stand_in, "").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let st = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.clone(),
        Vec::new(),
    )
    .with_agent_path(only_grok)
    .with_agent_versions(VersionCache::with_reader(|_| Some("grok 1.2.3 (abc1234)\n".into())));
    Env {
        agents_file: paths::agents_file(&home),
        _dir: dir,
        app: router(st),
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
    let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, body)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    send(app, base("GET", uri).body(Body::empty()).unwrap()).await
}

async fn put(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = base("PUT", uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

fn own_agent(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "cli": name, "mark": "copilot",
        "commands": [format!("{id}-marshell-test")],
        "prompt": {"style": "flag", "flag": "--prompt"},
        "continue": null,
        "modes": {"manual": [], "bypass": ["--yolo"]},
    })
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn entry<'a>(catalog: &'a Value, id: &str) -> &'a Value {
    catalog["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {catalog}"))
}

#[tokio::test]
async fn catalog_lists_adapters_shipped_and_own_with_found_flags() {
    let e = build();
    let (status, saved) = put(
        &e.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("mine", "Mine")], "version": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");

    let (status, catalog) = get(&e.app, "/v1/agents/catalog").await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let ids: Vec<&str> = catalog["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "claude",
            "codex",
            "antigravity",
            "copilot",
            "cursor",
            "opencode",
            "amp",
            "grok",
            "qwen",
            "kimi",
            "aider",
            "crush",
            "mine"
        ]
    );
    assert_eq!(catalog["presets"], json!([]));

    // Adapters: nothing on the injected PATH, so not found; the five modes the launcher offers, Codex's Plan included.
    let claude = entry(&catalog, "claude");
    assert_eq!(claude["adapter"], true);
    assert_eq!(claude["own"], false);
    assert_eq!(claude["found"], false);
    assert_eq!(claude["version"], Value::Null);
    assert_eq!(claude["can_continue"], true);
    assert_eq!(claude["prompt"], json!({"style": "positional"}));
    assert_eq!(
        claude["modes"],
        json!(["manual", "plan", "auto-edit", "full-auto", "bypass"])
    );
    assert_eq!(entry(&catalog, "codex")["modes"], claude["modes"]);
    assert_eq!(
        entry(&catalog, "antigravity")["prompt"],
        json!({"style": "flag", "flag": "-i"})
    );

    // A shipped manifest agent: found through its injected command and version.
    let grok = entry(&catalog, "grok");
    assert_eq!(grok["adapter"], false);
    assert_eq!(grok["own"], false);
    assert_eq!(grok["found"], true);
    assert_eq!(grok["version"], "1.2.3");
    assert_eq!(grok["name"], "Grok");
    assert_eq!(grok["cli"], "Grok Build");
    assert_eq!(grok["mark"], "grok");
    assert_eq!(grok["can_continue"], true);
    assert_eq!(
        grok["modes"],
        json!(["manual", "plan", "auto-edit", "full-auto", "bypass"])
    );
    for other in ["copilot", "cursor", "opencode", "amp", "qwen", "kimi", "aider", "crush"] {
        assert_eq!(entry(&catalog, other)["found"], false, "{other}");
    }

    // An own agent: not found, marked generic, its modes as saved, no continue.
    let mine = entry(&catalog, "mine");
    assert_eq!(mine["adapter"], false);
    assert_eq!(mine["own"], true);
    assert_eq!(mine["found"], false);
    assert_eq!(mine["mark"], "generic");
    assert_eq!(mine["can_continue"], false);
    assert_eq!(mine["modes"], json!(["manual", "bypass"]));
    assert_eq!(mine["prompt"], json!({"style": "flag", "flag": "--prompt"}));
}

#[tokio::test]
async fn own_save_round_trips_and_reloads_on_change() {
    let e = build();
    let (status, before) = get(&e.app, "/v1/agents/own").await;
    assert_eq!(status, StatusCode::OK, "{before}");
    assert_eq!(before, json!({"agents": [], "problems": [], "version": ""}));

    let (status, saved) = put(
        &e.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("mine", "Mine")], "version": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let bytes = std::fs::read(&e.agents_file).unwrap();
    assert_eq!(saved["version"], sha(&bytes));
    assert_eq!(saved["problems"], json!([]));
    assert_eq!(saved["agents"][0]["mark"], "generic");
    let file: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(file["version"], 1);
    assert_eq!(file["agents"][0]["id"], "mine");

    let (_, again) = get(&e.app, "/v1/agents/own").await;
    assert_eq!(again, saved);

    // A hand edit, with a modified time that is certainly new.
    let edited = String::from_utf8(bytes).unwrap().replace("\"Mine\"", "\"Hand edited\"");
    std::fs::write(&e.agents_file, &edited).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(&e.agents_file).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(30)).unwrap();
    drop(file);

    let (_, own) = get(&e.app, "/v1/agents/own").await;
    assert_eq!(own["agents"][0]["name"], "Hand edited");
    assert_eq!(own["version"], sha(edited.as_bytes()));
    let (_, catalog) = get(&e.app, "/v1/agents/catalog").await;
    assert_eq!(entry(&catalog, "mine")["name"], "Hand edited");
}

#[tokio::test]
async fn own_save_refuses_a_stale_version() {
    let e = build();
    let (status, first) = put(
        &e.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("one", "One")], "version": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let before = std::fs::read(&e.agents_file).unwrap();

    let (status, body) = put(
        &e.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("two", "Two")], "version": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body, "agents.json changed since you opened it.");
    assert_eq!(std::fs::read(&e.agents_file).unwrap(), before);
}

#[tokio::test]
async fn own_save_refuses_invalid_entries() {
    let e = build();
    let (_, first) = put(
        &e.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("one", "One")], "version": ""}),
    )
    .await;
    let before = std::fs::read(&e.agents_file).unwrap();

    let (status, body) = put(
        &e.app,
        "/v1/agents/own",
        json!({
            "agents": [own_agent("claude", "Impostor"), own_agent("two", "Two"), own_agent("docker", "Tool")],
            "version": first["version"],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body,
        json!([
            {"index": 0, "id": "claude", "reason": "The id \"claude\" is taken by a built-in agent."},
            {"index": 2, "id": "docker", "reason": "The id \"docker\" is taken by a built-in tool."},
        ])
    );
    assert_eq!(std::fs::read(&e.agents_file).unwrap(), before);

    // With no file yet, a refusal creates none.
    let fresh = build();
    let (status, _) = put(
        &fresh.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("claude", "Impostor")], "version": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!fresh.agents_file.exists());
}

#[tokio::test]
async fn a_broken_agents_file_is_reported_and_kept() {
    for (text, why) in [
        ("{ not json", "it isn't valid JSON (line 1)"),
        (r#"{"version": 2, "agents": []}"#, "it was written by a newer Marshell"),
    ] {
        let e = build();
        std::fs::write(&e.agents_file, text).unwrap();

        let (status, own) = get(&e.app, "/v1/agents/own").await;
        assert_eq!(status, StatusCode::OK, "{own}");
        assert_eq!(
            own,
            json!({
                "agents": [],
                "problems": [{"index": 0, "id": null, "reason": format!("agents.json can't be read: {why}.")}],
                "version": sha(text.as_bytes()),
            })
        );
        // The catalog still answers, with no own agents.
        let (status, catalog) = get(&e.app, "/v1/agents/catalog").await;
        assert_eq!(status, StatusCode::OK, "{catalog}");
        assert_eq!(catalog["agents"].as_array().unwrap().len(), 12);

        // A save against it is refused with that problem, and the file stays as it was.
        let (status, body) = put(
            &e.app,
            "/v1/agents/own",
            json!({"agents": [own_agent("mine", "Mine")], "version": own["version"]}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body, own["problems"]);
        assert_eq!(std::fs::read_to_string(&e.agents_file).unwrap(), text);
        assert!(!marshell_core::store::bak_path(&e.agents_file).exists());
    }
}

#[tokio::test]
async fn catalog_routes_refuse_without_the_bearer() {
    let e = build();
    for (method, uri) in [
        ("GET", "/v1/agents/catalog"),
        ("GET", "/v1/agents/own"),
        ("PUT", "/v1/agents/own"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"agents": [], "version": ""}).to_string()))
            .unwrap();
        let (status, _) = send(&e.app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    assert!(!e.agents_file.exists());
}

/// A saved own agent reaches launching without a restart. No profile is detected, so a known agent still can't start;
/// what matters is that the refusal is no longer "Unknown agent".
#[tokio::test]
async fn a_saved_own_agent_is_known_to_session_create() {
    let e = build();
    let start = || {
        base("POST", "/v1/sessions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"cols": 80, "rows": 24, "agent": "mine"}).to_string()))
            .unwrap()
    };
    let (status, body) = send(&e.app, start()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "Unknown agent: mine.");

    let (status, saved) = put(
        &e.app,
        "/v1/agents/own",
        json!({"agents": [own_agent("mine", "Mine")], "version": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, body) = send(&e.app, start()).await;
    assert_ne!(body, "Unknown agent: mine.");
}
