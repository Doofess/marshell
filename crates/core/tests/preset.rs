//! The DeepSeek preset through the API. The sessions run the fake agent in `sleep` mode (the profile's command), never a
//! real `claude`, and every file lives in a temp Marshell home. The key is a test value read through the injected getter.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5558;
const ORIGIN: &str = "http://tauri.localhost";
const KEY: &str = "sk-marshell-test-0000";

fn key_set() -> Option<String> {
    Some(KEY.into())
}

fn key_unset() -> Option<String> {
    None
}

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

fn state_with(home: &Path, key: fn() -> Option<String>) -> AppState {
    AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![sleeper()],
    )
    .with_agent_check(|_| true)
    .with_deepseek_key(key)
    .with_parent_env(|| vec!["ANTHROPIC_API_KEY".into(), "PATH".into()])
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

async fn text_of(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), 256 * 1024).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    serde_json::from_str(&text_of(res).await).unwrap()
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    (res.status(), text_of(res).await)
}

fn create_body(cwd: &Path, agent: Option<&str>, preset: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy(),
        "agent": agent, "mode": "manual", "preset": preset,
    })
}

async fn create(app: &axum::Router, cwd: &Path, agent: Option<&str>, preset: Option<&str>) -> (StatusCode, String) {
    send(app, api("POST", "/v1/sessions", Some(create_body(cwd, agent, preset)))).await
}

fn tab_of(created: &str) -> String {
    serde_json::from_str::<serde_json::Value>(created).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn sessions(app: &axum::Router) -> serde_json::Value {
    json_of(app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap()).await
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

mod preset {
    use super::*;

    #[tokio::test]
    async fn the_key_never_reaches_the_layout_file() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let state = state_with(home.path(), key_set);
        let app = router(state.clone());

        let (status, created) = create(&app, work.path(), Some("claude"), Some("deepseek")).await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let tab = tab_of(&created);

        // The row says what it is. The agent stays Claude, so every Claude gate still applies.
        let list = sessions(&app).await;
        assert_eq!(list["sessions"][0]["agent"], "claude");
        assert_eq!(list["sessions"][0]["preset"], "deepseek");

        // A layout save, with the open tab in it.
        let view = serde_json::json!({
            "panes": [tab], "arrangement": null, "ratios": [], "active": 0, "zoomed": null,
            "sidebar": "expanded", "sidebar_width": 280, "shell_hidden": false, "grouped": false
        });
        let (status, _) = send(&app, api("PUT", "/v1/layout/view", Some(view))).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // The saver writes the open tabs a moment after a change.
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
        assert!(!contains(&layout, KEY), "the layout file holds the key");
        let saved: serde_json::Value = serde_json::from_slice(&layout).unwrap();
        assert_eq!(saved["tabs"][0]["preset"], "deepseek");
        assert_eq!(
            saved["launcher"][work.path().to_string_lossy().as_ref()]["preset"],
            "deepseek",
            "{saved}"
        );

        // The SessionInfo JSON of the row, the session list response and the layout response.
        let info = serde_json::to_string(&state.registry().snapshot().sessions[0]).unwrap();
        assert!(!info.contains(KEY));
        assert!(!created.contains(KEY));
        let (_, list_text) = send(&app, api("GET", "/v1/sessions", None)).await;
        assert!(!list_text.contains(KEY));
        let (_, layout_text) = send(&app, api("GET", "/v1/layout", None)).await;
        assert!(!layout_text.contains(KEY));
        let (_, catalog_text) = send(&app, api("GET", "/v1/agents/catalog", None)).await;
        assert!(!catalog_text.contains(KEY));

        // A SessionStart hook from the tab indexes the conversation: its sidecar and index row are written too.
        let claude_dir = tempfile::tempdir().unwrap();
        let transcript = claude_dir.path().join("s1.jsonl");
        std::fs::write(&transcript, "{}\n").unwrap();
        let event = serde_json::json!({
            "hook_event_name": "SessionStart", "session_id": "s1",
            "transcript_path": transcript.to_string_lossy(), "cwd": work.path().to_string_lossy(),
        });
        assert!(state.history().note_event("claude", &tab, None, &event, 1000).is_none());
        let sidecar = marshell_core::history::archive::read_meta(home.path(), "claude", "s1")
            .unwrap()
            .expect("the SessionStart wrote a sidecar");
        // The history record keeps the preset's name, so Open in new tab starts the conversation the same way.
        assert_eq!(sidecar.preset.as_deref(), Some("deepseek"));

        // Nothing else Marshell wrote under its home (the history index and sidecars, recordings, settings) holds it.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut files = Vec::new();
        files_under(home.path(), &mut files);
        assert!(
            files.iter().any(|(path, _)| path.to_string_lossy().contains("s1")),
            "no history file was written"
        );
        for (path, bytes) in &files {
            assert!(!contains(bytes, KEY), "{} holds the key", path.display());
        }
        stop(&state, &[&tab]);
    }

    #[tokio::test]
    async fn restart_and_reopen_keep_the_preset() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let state = state_with(home.path(), key_set);
        let app = router(state.clone());
        let (status, created) = create(&app, work.path(), Some("claude"), Some("deepseek")).await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let tab = tab_of(&created);

        // Restart in another mode.
        let body = serde_json::json!({ "mode": "plan", "cols": 80, "rows": 24 });
        let (status, text) = send(&app, api("POST", &format!("/v1/sessions/{tab}/restart"), Some(body))).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let row = sessions(&app).await["sessions"][0].clone();
        assert_eq!(row["preset"], "deepseek");
        assert_eq!(row["agent"], "claude");
        assert_eq!(row["mode"], "plan");

        // Close, then reopen: the closed entry keeps the preset, and so does the new row.
        let (status, _) = send(&app, api("DELETE", &format!("/v1/sessions/{tab}"), None)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, relaunch) = state.registry().peek_closed(None).unwrap();
        assert_eq!(relaunch.preset.as_deref(), Some("deepseek"));
        let (status, reopened) = send(&app, api("POST", "/v1/sessions/reopen", None)).await;
        assert_eq!(status, StatusCode::OK, "{reopened}");
        let row = sessions(&app).await["sessions"][0].clone();
        assert_eq!(row["preset"], "deepseek");
        assert_eq!(row["agent"], "claude");
        stop(&state, &[&tab_of(&reopened)]);
    }

    #[tokio::test]
    async fn a_restart_or_reopen_without_the_key_is_refused_and_never_a_plain_session() {
        static KEY_ON: AtomicBool = AtomicBool::new(true);
        fn key() -> Option<String> {
            KEY_ON.load(Ordering::SeqCst).then(|| KEY.to_string())
        }
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let state = state_with(home.path(), key);
        let app = router(state.clone());
        let (_, created) = create(&app, work.path(), Some("claude"), Some("deepseek")).await;
        let tab = tab_of(&created);
        let old = state.session(&tab).unwrap();

        KEY_ON.store(false, Ordering::SeqCst);
        let no_key = "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.";
        let body = serde_json::json!({ "mode": "plan", "cols": 80, "rows": 24 });
        let (status, text) = send(&app, api("POST", &format!("/v1/sessions/{tab}/restart"), Some(body))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
        assert_eq!(text, no_key);
        // Refused before the old process was touched.
        assert!(std::sync::Arc::ptr_eq(&old, &state.session(&tab).unwrap()));
        assert!(!old.has_exited());
        assert_eq!(sessions(&app).await["sessions"][0]["mode"], "manual");

        send(&app, api("DELETE", &format!("/v1/sessions/{tab}"), None)).await;
        let (status, text) = send(&app, api("POST", "/v1/sessions/reopen", None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
        assert_eq!(text, no_key);
        // Nothing started, and the closed entry is still there to reopen once the key is back.
        assert_eq!(sessions(&app).await["sessions"].as_array().unwrap().len(), 0);
        assert!(state.registry().peek_closed(None).is_some());
        stop(&state, &[&tab]);
    }

    #[tokio::test]
    async fn a_tab_from_the_last_run_resumes_on_deepseek_only_with_the_key() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        // The layout of the last run: a Claude tab started with the preset, and an old one with no `preset` at all.
        let tab = |id: &str, number: u32, preset: Option<&str>| {
            let mut t = serde_json::json!({
                "tab_id": id, "number": number, "name": id, "agent": "claude",
                "profile_id": "sleeper", "cwd": work.path().to_string_lossy(), "mode": "manual",
            });
            if let Some(p) = preset {
                t["preset"] = p.into();
            }
            t
        };
        let layout = serde_json::json!({
            "version": 1,
            "tabs": [tab("t_ds", 1, Some("deepseek")), tab("t_old", 2, None)],
            "view": { "panes": [null], "arrangement": null, "ratios": [], "active": 0, "zoomed": null,
                      "sidebar": "expanded", "sidebar_width": 280, "shell_hidden": false, "grouped": false },
            "launcher": {}
        });
        std::fs::write(home.path().join("layout.json"), layout.to_string()).unwrap();

        let resume = |tab: &str| {
            api(
                "POST",
                &format!("/v1/sessions/{tab}/resume"),
                Some(serde_json::json!({ "cols": 80, "rows": 24 })),
            )
        };
        // No key: the DeepSeek tab does not start as plain Claude. The old tab loads and resumes as it always did.
        let without = state_with(home.path(), key_unset);
        let app = router(without.clone());
        let rows = sessions(&app).await;
        assert_eq!(rows["sessions"][0]["preset"], "deepseek");
        assert!(rows["sessions"][1].get("preset").is_none());
        let (status, text) = send(&app, resume("t_ds")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
        assert_eq!(without.live_session_count(), 0);
        let (status, text) = send(&app, resume("t_old")).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        stop(&without, &["t_old"]);

        let with = state_with(home.path(), key_set);
        let app = router(with.clone());
        let (status, text) = send(&app, resume("t_ds")).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        stop(&with, &["t_ds"]);
    }

    #[tokio::test]
    async fn open_in_new_tab_starts_a_deepseek_conversation_on_deepseek_again() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let claude_dir = tempfile::tempdir().unwrap();
        // A conversation of a tab Marshell no longer has: only its sidecar knows the preset.
        let transcript = claude_dir.path().join("old1.jsonl");
        std::fs::write(&transcript, "{}\n").unwrap();
        let meta_event = serde_json::json!({
            "hook_event_name": "SessionStart", "session_id": "old1",
            "transcript_path": transcript.to_string_lossy(), "cwd": work.path().to_string_lossy(),
        });
        let seed = state_with(home.path(), key_set);
        assert!(
            seed.history()
                .note_event("claude", "t_gone", None, &meta_event, 1000)
                .is_none()
        );
        let mut sidecar = marshell_core::history::archive::read_meta(home.path(), "claude", "old1")
            .unwrap()
            .unwrap();
        sidecar.preset = Some("deepseek".into());
        marshell_core::history::archive::write_meta(home.path(), &sidecar).unwrap();
        let open = || {
            api(
                "POST",
                "/v1/history/claude/old1/resume",
                Some(serde_json::json!({ "cols": 80, "rows": 24 })),
            )
        };

        // Without the key it is refused: never a plain Claude session on Anthropic.
        let without = state_with(home.path(), key_unset);
        let app = router(without.clone());
        let (status, text) = send(&app, open()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
        assert_eq!(text, "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.");
        assert_eq!(without.live_session_count(), 0);

        let with = state_with(home.path(), key_set);
        let app = router(with.clone());
        let (status, text) = send(&app, open()).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let row = sessions(&app).await["sessions"][0].clone();
        assert_eq!(row["preset"], "deepseek");
        assert_eq!(row["agent"], "claude");
        stop(&with, &[&tab_of(&text)]);
    }

    #[tokio::test]
    async fn a_preset_is_refused_for_anything_but_claude_and_for_an_unknown_name() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let state = state_with(home.path(), key_set);
        let app = router(state.clone());

        for agent in [Some("codex"), Some("antigravity"), None] {
            let (status, text) = create(&app, work.path(), agent, Some("deepseek")).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{agent:?}");
            assert_eq!(text, "DeepSeek runs on Claude Code only.");
        }
        // An explicit argv runs as given: a preset has nothing to change there.
        let mut body = create_body(work.path(), Some("claude"), Some("deepseek"));
        body["shell"] = serde_json::json!([env!("CARGO_BIN_EXE_fake-agent"), "sleep"]);
        let (status, text) = send(&app, api("POST", "/v1/sessions", Some(body))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
        assert_eq!(text, "DeepSeek runs on Claude Code only.");

        let (status, text) = create(&app, work.path(), Some("claude"), Some("x")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(text, "Unknown preset: x.");
        assert_eq!(state.live_session_count(), 0);

        // No key.
        let keyless = router(state_with(home.path(), key_unset));
        let (status, text) = create(&keyless, work.path(), Some("claude"), Some("deepseek")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(text, "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.");
    }
}
