//! The History routes: list, search, one conversation, and resume.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::history::archive::{archive_paths, copy_transcript};
use marshell_core::history::marks;
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5557;
const ID: &str = "00000000-0000-4000-8000-000000000001";
const BASIC: &str = include_str!("fixtures/claude/transcripts/2.1.309/basic.jsonl");
const NO_SUCH: &str = "No such conversation.";
const NO_COPY: &str = "Marshell has no copy of this conversation yet.";
const RUNNING_IN_A_TAB: &str = "This conversation is already running in a tab. Use Go to session.";
const NO_CONVERSATION: &str = "Claude Code no longer has this conversation, so it can't be resumed.";

struct Env {
    state: AppState,
    app: axum::Router,
    home: tempfile::TempDir,
    work: tempfile::TempDir,
}

/// A profile whose agent "claude" runs the fake agent in `print` mode: it prints its own arguments.
fn printer() -> Profile {
    Profile {
        id: "printer".into(),
        name: "Printer".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "print".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

fn env() -> Env {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    env_in(home, work)
}

/// Like `printer`, but the agent waits on its input instead of exiting, so its tab stays Running.
fn waiter() -> Profile {
    Profile {
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "echo".into()],
        ..printer()
    }
}

fn env_in(home: tempfile::TempDir, work: tempfile::TempDir) -> Env {
    env_with(home, work, printer())
}

fn env_with(home: tempfile::TempDir, work: tempfile::TempDir, profile: Profile) -> Env {
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.path().to_path_buf(),
        vec![profile],
    )
    .with_agent_check(|_| true);
    Env {
        app: router(state.clone()),
        state,
        home,
        work,
    }
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
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

/// The status and body text of a request.
async fn call(app: &axum::Router, request: Request<Body>) -> (StatusCode, String) {
    let res = app.clone().oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let (status, text) = call(app, req("GET", uri, None)).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

fn resume_body() -> Value {
    json!({ "cols": 80, "rows": 24 })
}

/// Indexes the session `id` for tab "t1" with its transcript file in `dir`, as a SessionStart does.
fn index_session(env: &Env, id: &str, dir: &Path) -> PathBuf {
    let transcript = dir.join(format!("{id}.jsonl"));
    std::fs::write(&transcript, BASIC).unwrap();
    let body = json!({
        "hook_event_name": "SessionStart",
        "session_id": id,
        "transcript_path": transcript.to_string_lossy(),
        "cwd": env.work.path().to_string_lossy(),
    });
    assert!(env.state.history().note_event("t1", None, &body, 1000).is_none());
    transcript
}

/// Indexes the session, copies its transcript into the archive and reads it into the index.
fn seed(env: &Env, id: &str) -> PathBuf {
    let transcript = index_session(env, id, env.work.path());
    let meta = env.state.history().latest_for_tab("t1").unwrap();
    copy_transcript(env.home.path(), &meta).unwrap();
    env.state.history().reindex(&meta);
    transcript
}

#[tokio::test]
async fn list_and_search_after_a_copy() {
    let env = env();
    seed(&env, ID);

    let (status, list) = get_json(&env.app, "/v1/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list[0]["session_id"], ID);
    assert_eq!(list[0]["title"], "Retry limit work");
    assert_eq!(list[0]["turns"], 2);

    let (_, other_agent) = get_json(&env.app, "/v1/history?agent=codex").await;
    assert_eq!(other_agent, json!([]));
    let (_, claude) = get_json(&env.app, "/v1/history?agent=claude&since_ms=1").await;
    assert_eq!(claude.as_array().unwrap().len(), 1);

    let (status, hits) = get_json(&env.app, "/v1/history/search?q=retry").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hits[0]["session_id"], ID, "{hits}");
    assert!(hits[0]["snippet"].as_str().unwrap().contains('\u{2}'));

    // No query, an empty one, and one made only of punctuation all find nothing, with no error.
    for uri in [
        "/v1/history/search",
        "/v1/history/search?q=",
        "/v1/history/search?q=%22",
        "/v1/history/search?q=NEAR%28",
    ] {
        let (status, none) = get_json(&env.app, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(none, json!([]), "{uri}");
    }
}

#[tokio::test]
async fn conversation_returns_turns() {
    let env = env();
    seed(&env, ID);

    let (status, conversation) = get_json(&env.app, &format!("/v1/history/claude/{ID}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(conversation["session_id"], ID);
    assert_eq!(conversation["agent"], "claude");
    assert_eq!(conversation["turns"].as_array().unwrap().len(), 2, "{conversation}");
    assert_eq!(conversation["limited"], false);
    assert_eq!(conversation["tab_id"], "t1");
}

#[tokio::test]
async fn conversation_uses_the_parsed_title_before_backfill() {
    let env = env();
    index_session(&env, ID, env.work.path());
    let meta = env.state.history().latest_for_tab("t1").unwrap();
    copy_transcript(env.home.path(), &meta).unwrap();
    let (_, conversation) = get_json(&env.app, &format!("/v1/history/claude/{ID}")).await;
    assert_eq!(conversation["title"], "Retry limit work");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_tab_renames_keep_ended_tabs_and_conversations_together() {
    let env = env();
    seed(&env, ID);
    reserve_tab(&env, "t1", Some("claude"));
    env.state
        .registry()
        .set_state("t1", marshell_protocol::api::SessionState::Ended);
    for round in 0..10 {
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(20));
        let mut tasks = Vec::new();
        for n in 0..20 {
            let app = env.app.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                let (status, text) = call(
                    &app,
                    req(
                        "PATCH",
                        "/v1/sessions/t1",
                        Some(json!({"name":format!("name{round}-{n}")})),
                    ),
                )
                .await;
                assert_eq!(status, StatusCode::NO_CONTENT, "{text}");
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        let (_, list) = get_json(&env.app, "/v1/history").await;
        assert_eq!(list[0]["title"], env.state.registry().snapshot().sessions[0].name);
    }
}

#[tokio::test]
async fn a_session_without_an_archive_has_no_conversation_yet() {
    let env = env();
    // Indexed by its SessionStart, but never copied.
    index_session(&env, ID, env.work.path());

    let (status, text) = call(&env.app, req("GET", &format!("/v1/history/claude/{ID}"), None)).await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_COPY));
}

#[tokio::test]
async fn an_archive_that_cannot_be_read_is_a_limited_conversation() {
    let env = env();
    seed(&env, ID);
    // Not zstd: the read fails, and the route answers from the index row instead of an error.
    let (archive, _) = archive_paths(env.home.path(), "claude", ID);
    std::fs::write(&archive, b"this is not a zstd frame").unwrap();

    let (status, conversation) = get_json(&env.app, &format!("/v1/history/claude/{ID}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(conversation["limited"], true);
    assert_eq!(conversation["turns"], json!([]));
    assert_eq!(conversation["title"], "Retry limit work");
    assert_eq!(conversation["tab_id"], "t1");
    assert_eq!(conversation["cwd"], env.work.path().to_string_lossy().as_ref());
}

#[tokio::test]
async fn bad_ids_are_404_without_touching_files() {
    let env = env();
    seed(&env, ID);
    // A real archive under another agent's name: if the route opened it, the answer would not be a 404.
    let (claude_archive, _) = archive_paths(env.home.path(), "claude", ID);
    let (codex_archive, _) = archive_paths(env.home.path(), "codex", ID);
    std::fs::create_dir_all(codex_archive.parent().unwrap()).unwrap();
    std::fs::copy(&claude_archive, &codex_archive).unwrap();

    let long = "a".repeat(65);
    let unknown = "00000000-0000-4000-8000-0000000000ff";
    let gets = [
        "/v1/history/claude/..%2F..%2Fx".to_string(),
        "/v1/history/claude/%2E%2E".to_string(),
        format!("/v1/history/claude/{long}"),
        format!("/v1/history/codex/{ID}"),
        format!("/v1/history/claude/{unknown}"),
    ];
    for uri in gets {
        let (status, text) = call(&env.app, req("GET", &uri, None)).await;
        assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH), "GET {uri}");
    }
    let posts = [
        "/v1/history/claude/..%2F..%2Fx/resume".to_string(),
        format!("/v1/history/claude/{long}/resume"),
        format!("/v1/history/codex/{ID}/resume"),
        format!("/v1/history/claude/{unknown}/resume"),
    ];
    for uri in posts {
        let (status, text) = call(&env.app, req("POST", &uri, Some(resume_body()))).await;
        assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH), "POST {uri}");
    }
    assert_eq!(env.state.live_session_count(), 0);
}

/// The rows `peek` returns once `text` shows up in them.
async fn peek_until(app: &axum::Router, tab: &str, text: &str) -> String {
    let start = Instant::now();
    loop {
        let (_, body) = get_json(app, &format!("/v1/sessions/{tab}/peek?lines=30")).await;
        let rows = body["rows"].to_string();
        if rows.contains(text) {
            return rows;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "never saw {text:?} in {rows}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn resume_starts_claude_with_the_session_in_its_folder() {
    let env = env();
    // The transcript is Claude's own file, outside the Marshell home; the archive is Marshell's copy.
    let claude_dir = tempfile::tempdir().unwrap();
    let transcript = index_session(&env, ID, claude_dir.path());
    let meta = env.state.history().latest_for_tab("t1").unwrap();
    copy_transcript(env.home.path(), &meta).unwrap();
    env.state.history().reindex(&meta);

    let (status, text) = call(
        &env.app,
        req("POST", &format!("/v1/history/claude/{ID}/resume"), Some(resume_body())),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let created: Value = serde_json::from_str(&text).unwrap();
    let tab = created["tab_id"].as_str().unwrap().to_string();
    let rows = peek_until(&env.app, &tab, &format!("--resume {ID}")).await;
    assert!(rows.contains("claude"), "{rows}");
    assert!(!rows.contains("--continue"), "{rows}");

    let (_, sessions) = get_json(&env.app, "/v1/sessions").await;
    let row = sessions["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab.as_str())
        .unwrap();
    assert_eq!(row["agent"], "claude");
    assert_eq!(row["cwd"], env.work.path().to_string_lossy().as_ref());
    if let Some(session) = env.state.remove_session(&tab) {
        session.force_kill();
    }

    // Claude's file goes: the answer is the same refusal the tab Resume gives, and nothing starts.
    std::fs::remove_file(&transcript).unwrap();
    let (status, text) = call(
        &env.app,
        req("POST", &format!("/v1/history/claude/{ID}/resume"), Some(resume_body())),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["reason"], NO_CONVERSATION);
    assert_eq!(env.state.live_session_count(), 0);
}

#[tokio::test]
async fn resume_refuses_a_conversation_running_in_a_tab() {
    common::tie_children_to_test();
    // A printer would exit at once and end its tab before the second Resume, racing the refusal.
    let env = env_with(tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), waiter());
    let claude_dir = tempfile::tempdir().unwrap();
    index_session(&env, ID, claude_dir.path());
    let meta = env.state.history().latest_for_tab("t1").unwrap();
    copy_transcript(env.home.path(), &meta).unwrap();
    env.state.history().reindex(&meta);
    let uri = format!("/v1/history/claude/{ID}/resume");

    // The first Resume opens a tab that runs the conversation.
    let (status, text) = call(&env.app, req("POST", &uri, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let created: Value = serde_json::from_str(&text).unwrap();
    let tab = created["tab_id"].as_str().unwrap().to_string();
    // Claude's SessionStart hook arrives from that tab: the index now has the conversation in it.
    let body = json!({
        "hook_event_name": "SessionStart",
        "session_id": ID,
        "transcript_path": claude_dir.path().join(format!("{ID}.jsonl")).to_string_lossy(),
        "cwd": env.work.path().to_string_lossy(),
    });
    env.state.history().note_event(&tab, None, &body, 2000);
    assert_eq!(env.state.live_session_count(), 1);

    // A second Resume would start a second Claude on it: refused, and nothing starts.
    let (status, text) = call(&env.app, req("POST", &uri, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    let refusal: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(refusal["reason"], RUNNING_IN_A_TAB);
    assert_eq!(env.state.live_session_count(), 1);

    // The tab moved on to a newer conversation (after /clear): the old one may be resumed again.
    let newer = "00000000-0000-4000-8000-000000000002";
    let newer_path = claude_dir.path().join(format!("{newer}.jsonl"));
    std::fs::write(&newer_path, BASIC).unwrap();
    let body = json!({
        "hook_event_name": "SessionStart",
        "session_id": newer,
        "transcript_path": newer_path.to_string_lossy(),
        "cwd": env.work.path().to_string_lossy(),
    });
    env.state.history().note_event(&tab, None, &body, 3000);
    let (status, text) = call(&env.app, req("POST", &uri, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(env.state.live_session_count(), 2);
    let second: Value = serde_json::from_str(&text).unwrap();
    for t in [tab.as_str(), second["tab_id"].as_str().unwrap()] {
        if let Some(session) = env.state.remove_session(t) {
            session.force_kill();
        }
    }
}

#[tokio::test]
async fn routes_need_the_token() {
    let env = env();
    seed(&env, ID);
    let routes = [
        ("GET", "/v1/history".to_string()),
        ("GET", "/v1/history/search?q=retry".to_string()),
        ("GET", format!("/v1/history/claude/{ID}")),
        ("POST", format!("/v1/history/claude/{ID}/resume")),
        ("PATCH", format!("/v1/history/claude/{ID}")),
        ("DELETE", format!("/v1/history/claude/{ID}")),
        ("PUT", format!("/v1/history/claude/{ID}/bookmarks/0")),
        ("DELETE", format!("/v1/history/claude/{ID}/bookmarks/0")),
        ("GET", "/v1/history/bookmarks".into()),
        ("POST", "/v1/sessions/t1/bookmark".into()),
        ("GET", format!("/v1/history/claude/{ID}/recordings")),
        ("GET", format!("/v1/history/claude/{ID}/recordings/0/frames")),
    ];
    for (method, uri) in routes {
        let mut request = req(method, &uri, Some(resume_body()));
        request.headers_mut().remove(header::AUTHORIZATION);
        let (status, _) = call(&env.app, request).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    assert_eq!(env.state.live_session_count(), 0);
}

#[tokio::test]
async fn history_off_is_503() {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    // A directory where the index file belongs: the index can't open, so history is off for this run.
    std::fs::create_dir_all(home.path().join("history").join("index.sqlite")).unwrap();
    let env = env_in(home, tempfile::tempdir().unwrap());

    for (method, uri) in [
        ("GET", "/v1/history".to_string()),
        ("GET", "/v1/history/search?q=retry".to_string()),
        ("GET", format!("/v1/history/claude/{ID}")),
        ("POST", format!("/v1/history/claude/{ID}/resume")),
        ("PATCH", format!("/v1/history/claude/{ID}")),
        ("DELETE", format!("/v1/history/claude/{ID}")),
        ("PUT", format!("/v1/history/claude/{ID}/bookmarks/0")),
        ("DELETE", format!("/v1/history/claude/{ID}/bookmarks/0")),
        ("GET", "/v1/history/bookmarks".into()),
        ("POST", "/v1/sessions/t1/bookmark".into()),
    ] {
        let (status, text) = call(&env.app, req(method, &uri, Some(resume_body()))).await;
        assert_eq!(
            (status, text.as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, "History is off for this run."),
            "{method} {uri}"
        );
    }
    // An unknown agent is still a 404 first.
    let (status, text) = call(&env.app, req("GET", "/v1/history/gemini/x", None)).await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
}

fn reserve_tab(env: &Env, tab: &str, agent: Option<&str>) {
    env.state.registry().reserve(
        tab,
        marshell_core::registry::Relaunch {
            profile_id: "printer".into(),
            agent: agent.map(str::to_string),
            cwd: env.work.path().to_string_lossy().into_owned(),
            mode: None,
            name: "Original tab".into(),
        },
    );
}

#[tokio::test]
async fn patch_sets_name_tags_pin_and_archive() {
    let env = env();
    seed(&env, ID);
    let uri = format!("/v1/history/claude/{ID}");
    let (status, text) = call(
        &env.app,
        req(
            "PATCH",
            &uri,
            Some(json!({
                "name": "  My work  ", "tags": [" Bug ", "bug", "ui"], "pinned": true, "archived": true
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(get_json(&env.app, "/v1/history").await.1, json!([]));
    let (_, list) = get_json(&env.app, "/v1/history?archived=true&tag=BUG").await;
    assert_eq!(list[0]["title"], "My work");
    assert_eq!(list[0]["tags"], json!(["Bug", "ui"]));
    assert_eq!(list[0]["pinned"], true);
    assert_eq!(list[0]["archived"], true);
    assert_eq!(list[0]["named"], true);
    let (_, conversation) = get_json(&env.app, &uri).await;
    assert_eq!(conversation["title"], "My work");
    assert_eq!(
        conversation["marks"],
        json!({"name":"My work", "tags":["Bug","ui"], "pinned":true, "archived":true, "bookmarked_turns":[]})
    );
    assert_eq!(get_json(&env.app, "/v1/history/search?q=retry").await.1, json!([]));
    let (_, hits) = get_json(&env.app, "/v1/history/search?q=retry&archived=true").await;
    assert_eq!(hits[0]["title"], "My work");
    let (status, _) = call(&env.app, req("PATCH", &uri, Some(json!({"name":"", "archived":false})))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = get_json(&env.app, "/v1/history").await;
    assert_eq!(list[0]["title"], "Retry limit work");
    assert_eq!(list[0]["named"], false);
    assert_eq!(list[0]["tags"], json!(["Bug", "ui"]));
    assert_eq!(list[0]["pinned"], true);
}

#[tokio::test]
async fn patch_refuses_bad_tags_with_the_message() {
    let env = env();
    seed(&env, ID);
    let tags: Vec<_> = (0..21).map(|n| format!("tag{n}")).collect();
    let (status, text) = call(
        &env.app,
        req("PATCH", &format!("/v1/history/claude/{ID}"), Some(json!({"tags":tags}))),
    )
    .await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "A conversation can have up to 20 tags.")
    );
}

#[tokio::test]
async fn renaming_a_running_conversation_renames_its_tab() {
    let env = env();
    seed(&env, ID);
    reserve_tab(&env, "t1", Some("claude"));
    let uri = format!("/v1/history/claude/{ID}");
    let (status, _) = call(&env.app, req("PATCH", &uri, Some(json!({"name":"  Shared\nname  "})))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(env.state.registry().snapshot().sessions[0].name, "Shared name");
    let (status, _) = call(&env.app, req("PATCH", &uri, Some(json!({"name":""})))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(env.state.registry().snapshot().sessions[0].name, "Retry limit work");
}

#[tokio::test]
async fn renaming_a_tab_names_its_conversation_and_the_next_one() {
    let env = env();
    seed(&env, ID);
    reserve_tab(&env, "t1", Some("claude"));
    let (status, _) = call(
        &env.app,
        req("PATCH", "/v1/sessions/t1", Some(json!({"name":"Shared name"}))),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(get_json(&env.app, "/v1/history").await.1[0]["title"], "Shared name");
    let newer = "00000000-0000-4000-8000-000000000002";
    seed(&env, newer);
    let (_, list) = get_json(&env.app, "/v1/history").await;
    assert_eq!(list.as_array().unwrap().len(), 2);
    for row in list.as_array().unwrap() {
        assert_eq!(row["title"], "Shared name");
    }
}

#[tokio::test]
async fn bookmark_turn_on_and_off() {
    use marshell_core::history::marks::Marks;
    let env = env();
    seed(&env, ID);
    let uri = format!("/v1/history/claude/{ID}");
    let (_, c) = get_json(&env.app, &uri).await;
    let at = c["turns"][0]["at_ms"].as_u64().unwrap();
    // Two live bookmarks within the first turn resolve to one turn, and toggle-off removes both.
    marks::write(
        env.home.path(),
        "claude",
        ID,
        &Marks {
            bookmarks: vec![at, at + 1],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        get_json(&env.app, &uri).await.1["marks"]["bookmarked_turns"],
        json!([0])
    );
    let turn = format!("{uri}/bookmarks/0");
    assert_eq!(
        call(&env.app, req("DELETE", &turn, None)).await.0,
        StatusCode::NO_CONTENT
    );
    assert!(marks::read(env.home.path(), "claude", ID).bookmarks.is_empty());
    assert_eq!(call(&env.app, req("PUT", &turn, None)).await.0, StatusCode::NO_CONTENT);
    assert_eq!(marks::read(env.home.path(), "claude", ID).bookmarks, vec![at]);
    assert_eq!(
        get_json(&env.app, &uri).await.1["marks"]["bookmarked_turns"],
        json!([0])
    );
    assert_eq!(
        get_json(&env.app, "/v1/history?bookmarked=true").await.1[0]["bookmarks"],
        1
    );
    assert_eq!(
        call(&env.app, req("DELETE", &turn, None)).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get_json(&env.app, "/v1/history/bookmarks").await.1, json!([]));
}

#[tokio::test]
async fn bookmark_now_marks_the_tabs_conversation() {
    let env = env();
    seed(&env, ID);
    reserve_tab(&env, "t1", Some("claude"));
    let (status, text) = call(&env.app, req("POST", "/v1/sessions/t1/bookmark", None)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap(),
        json!({"title":"Retry limit work"})
    );
    let (status, bookmarks) = get_json(&env.app, "/v1/history/bookmarks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bookmarks[0]["session_id"], ID);
    assert_eq!(bookmarks[0]["turn"], 1);
}

#[tokio::test]
async fn bookmark_now_refusals() {
    let env = env();
    reserve_tab(&env, "t1", Some("claude"));
    reserve_tab(&env, "t2", Some("codex"));
    reserve_tab(&env, "t3", None);
    for (tab, message) in [
        (
            "t1",
            "Nothing to bookmark yet: the conversation starts with its first prompt.",
        ),
        ("t2", "Bookmarks work in Claude Code sessions for now."),
        ("t3", "Bookmarks work in Claude Code sessions for now."),
    ] {
        let (status, text) = call(&env.app, req("POST", &format!("/v1/sessions/{tab}/bookmark"), None)).await;
        assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, message));
    }
}

#[tokio::test]
async fn bookmark_now_waits_for_the_first_prompt() {
    let env = env();
    let transcript = index_session(&env, ID, env.work.path());
    reserve_tab(&env, "t1", Some("claude"));
    let message = "Nothing to bookmark yet: the conversation starts with its first prompt.";
    let (status, text) = call(&env.app, req("POST", "/v1/sessions/t1/bookmark", None)).await;
    assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, message));
    assert!(
        get_json(&env.app, "/v1/history/bookmarks")
            .await
            .1
            .as_array()
            .unwrap()
            .is_empty()
    );
    let prompt = json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": ID,
        "transcript_path": transcript.to_string_lossy(),
        "cwd": env.work.path().to_string_lossy(),
        "prompt": "hello",
    });
    assert!(env.state.history().note_event("t1", None, &prompt, 2000).is_none());
    let (status, text) = call(&env.app, req("POST", "/v1/sessions/t1/bookmark", None)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(marks::read(env.home.path(), "claude", ID).bookmarks.len(), 1);
}

#[tokio::test]
async fn bookmarking_an_unknown_turn_is_refused() {
    let env = env();
    seed(&env, ID);
    // The index counts turns from 0; the person reads them from 1, as on the Bookmark turn buttons.
    let uri = format!("/v1/history/claude/{ID}");
    let (status, text) = call(&env.app, req("PUT", &format!("{uri}/bookmarks/999"), None)).await;
    assert_eq!(
        (status, text.as_str()),
        (
            StatusCode::CONFLICT,
            "Marshell hasn't indexed turn 1000 of this conversation yet."
        )
    );
    assert!(marks::read(env.home.path(), "claude", ID).bookmarks.is_empty());
    assert_eq!(get_json(&env.app, &uri).await.1["marks"]["bookmarked_turns"], json!([]));
    // Turning one off for an unknown turn is a harmless no-op.
    let (status, _) = call(&env.app, req("DELETE", &format!("{uri}/bookmarks/999"), None)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_removes_only_marshells_copy() {
    use marshell_core::{history::marks, store::bak_path};
    let env = env();
    let transcript = seed(&env, ID);
    let (archive, meta) = archive_paths(env.home.path(), "claude", ID);
    let marks = marks::marks_path(env.home.path(), "claude", ID);
    marks::write(env.home.path(), "claude", ID, &Default::default()).unwrap();
    let files = [archive, meta, marks];
    for path in &files {
        std::fs::copy(path, bak_path(path)).unwrap();
    }
    let uri = format!("/v1/history/claude/{ID}");
    assert_eq!(
        call(&env.app, req("DELETE", &uri, None)).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get_json(&env.app, &uri).await.0, StatusCode::NOT_FOUND);
    assert_eq!(get_json(&env.app, "/v1/history").await.1, json!([]));
    assert_eq!(get_json(&env.app, "/v1/history/search?q=retry").await.1, json!([]));
    for path in &files {
        assert!(!path.exists());
        assert!(!bak_path(path).exists());
    }
    assert_eq!(std::fs::read_to_string(transcript).unwrap(), BASIC);
    // A session whose archive and marks never existed can still be deleted.
    index_session(&env, "uncopied", env.work.path());
    assert_eq!(
        call(&env.app, req("DELETE", "/v1/history/claude/uncopied", None))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn delete_refuses_a_running_conversation() {
    let env = env();
    seed(&env, ID);
    reserve_tab(&env, "t1", Some("claude"));
    let uri = format!("/v1/history/claude/{ID}");
    let (status, text) = call(&env.app, req("DELETE", &uri, None)).await;
    assert_eq!(
        (status, text.as_str()),
        (
            StatusCode::CONFLICT,
            "This conversation is running in a tab. Close the tab first, then delete Marshell's copy."
        )
    );
    assert!(archive_paths(env.home.path(), "claude", ID).0.exists());
    // A running tab on a different conversation does not block deleting this one.
    env.state.history().note_event("t1", None, &json!({"hook_event_name":"SessionStart", "session_id":"newer", "transcript_path":env.work.path().join("newer.jsonl"), "cwd":env.work.path()}), 2000);
    assert_eq!(
        call(&env.app, req("DELETE", &uri, None)).await.0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn delete_refuses_a_copy_in_progress() {
    let env = env();
    seed(&env, ID);
    let history = env.state.history();
    history.set_settle(Duration::from_secs(60));
    marshell_core::history::recorder::schedule_copy(history.clone(), history.latest_for_tab("t1").unwrap());
    let result = call(&env.app, req("DELETE", &format!("/v1/history/claude/{ID}"), None)).await;
    history.flush(Duration::from_secs(5));
    assert_eq!(
        (result.0, result.1.as_str()),
        (
            StatusCode::CONFLICT,
            "Marshell is saving this conversation right now. Try again in a moment."
        )
    );
    assert!(archive_paths(env.home.path(), "claude", ID).0.exists());
}

#[tokio::test]
async fn delete_rejects_a_bad_id() {
    let env = env();
    let transcript = seed(&env, ID);
    let (archive, meta) = archive_paths(env.home.path(), "claude", ID);
    let before = [std::fs::read(&archive).unwrap(), std::fs::read(&meta).unwrap()];
    for uri in [
        "/v1/history/claude/..%2Fx".to_string(),
        format!("/v1/history/codex/{ID}"),
        "/v1/history/claude/unknown".to_string(),
    ] {
        let (status, text) = call(&env.app, req("DELETE", &uri, None)).await;
        assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
    }
    assert_eq!(std::fs::read(&archive).unwrap(), before[0]);
    assert_eq!(std::fs::read(&meta).unwrap(), before[1]);
    assert_eq!(std::fs::read_to_string(transcript).unwrap(), BASIC);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn marks_writes_do_not_lose_an_edit() {
    let env = env();
    seed(&env, ID);
    // Twenty tasks start together. Each field's requests are ordered so its last applied value is known;
    // the two fields race each other, catching a read-modify-write that loses the other field.
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(20));
    let mut tasks = Vec::new();
    let mut previous = [None, None];
    for n in 0..20 {
        let kind = n % 2;
        let prior = previous[kind].take();
        let (done, next) = tokio::sync::oneshot::channel::<()>();
        previous[kind] = Some(next);
        let app = env.app.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            if let Some(prior) = prior {
                prior.await.unwrap();
            }
            let body = if kind == 0 {
                json!({"tags":[format!("tag{}", n / 2)]})
            } else {
                json!({"pinned":n / 2 % 2 == 1})
            };
            let (status, text) = call(&app, req("PATCH", &format!("/v1/history/claude/{ID}"), Some(body))).await;
            let _ = done.send(());
            assert_eq!(status, StatusCode::OK, "{text}");
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let marks = marshell_core::history::marks::read(env.home.path(), "claude", ID);
    assert_eq!(marks.tags, ["tag9"]);
    assert!(marks.pinned);
    let (_, list) = get_json(&env.app, "/v1/history").await;
    assert_eq!(list[0]["tags"], json!(["tag9"]));
    assert_eq!(list[0]["pinned"], true);
    for file in std::fs::read_dir(archive_paths(env.home.path(), "claude", ID).0.parent().unwrap()).unwrap() {
        let name = file.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.ends_with(".part") && !name.ends_with(".tmp"), "{name}");
    }
}

/// A tab's row in the sessions list.
async fn tab_row(app: &axum::Router, tab: &str) -> Value {
    let (_, sessions) = get_json(app, "/v1/sessions").await;
    sessions["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab)
        .unwrap_or_else(|| panic!("no tab {tab} in {sessions}"))
        .clone()
}

fn tab_of(text: &str) -> String {
    serde_json::from_str::<Value>(text).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn name_conversation(env: &Env, name: &str) {
    let (status, _) = call(
        &env.app,
        req(
            "PATCH",
            &format!("/v1/history/claude/{ID}"),
            Some(json!({ "name": name })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn fork_starts_a_tab_with_fork_session_even_while_running() {
    let env = env();
    seed(&env, ID);
    // Tab t1 is Running and holds the conversation, so Resume refuses it.
    reserve_tab(&env, "t1", Some("claude"));
    name_conversation(&env, "Mine").await;
    let resume = format!("/v1/history/claude/{ID}/resume");
    let (status, _) = call(&env.app, req("POST", &resume, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let fork = format!("/v1/history/claude/{ID}/fork");
    let (status, text) = call(&env.app, req("POST", &fork, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let tab = tab_of(&text);
    let rows = peek_until(&env.app, &tab, &format!("--resume {ID} --fork-session")).await;
    assert!(rows.contains("claude"), "{rows}");
    let row = tab_row(&env.app, &tab).await;
    assert_eq!(row["name"], "Mine (fork)");
    assert_eq!(row["agent"], "claude");
    assert_eq!(row["cwd"], env.work.path().to_string_lossy().as_ref());

    // With no name the person gave, the fork's tab has the default name.
    name_conversation(&env, "").await;
    let (status, text) = call(&env.app, req("POST", &fork, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let name = tab_row(&env.app, &tab_of(&text)).await["name"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!name.contains("(fork)"), "{name}");

    // Claude's file gone: the same refusal as Resume. A bad id is a 404 before anything else.
    std::fs::remove_file(env.work.path().join(format!("{ID}.jsonl"))).unwrap();
    let (status, text) = call(&env.app, req("POST", &fork, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["reason"], NO_CONVERSATION);
    let (status, text) = call(
        &env.app,
        req("POST", "/v1/history/claude/..%2Fx/fork", Some(resume_body())),
    )
    .await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
}

#[tokio::test]
async fn resume_opens_the_tab_under_the_name() {
    let env = env();
    seed(&env, ID);
    name_conversation(&env, "Mine").await;
    let resume = format!("/v1/history/claude/{ID}/resume");
    let (status, text) = call(&env.app, req("POST", &resume, Some(resume_body()))).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let tab = tab_of(&text);
    assert_eq!(tab_row(&env.app, &tab).await["name"], "Mine");

    // The conversation Claude starts in that tab takes the name too.
    let newer = "00000000-0000-4000-8000-000000000002";
    let newer_path = env.work.path().join(format!("{newer}.jsonl"));
    std::fs::write(&newer_path, BASIC).unwrap();
    let body = json!({
        "hook_event_name": "SessionStart",
        "session_id": newer,
        "transcript_path": newer_path.to_string_lossy(),
        "cwd": env.work.path().to_string_lossy(),
    });
    env.state.history().note_event(&tab, None, &body, 5000);
    let (_, list) = get_json(&env.app, "/v1/history").await;
    let row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["session_id"] == newer)
        .unwrap();
    assert_eq!(row["title"], "Mine");
    assert_eq!(row["named"], true);

    // A conversation with no name of its own opens under the default name.
    name_conversation(&env, "").await;
    let (_, text) = call(&env.app, req("POST", &resume, Some(resume_body()))).await;
    assert_ne!(tab_row(&env.app, &tab_of(&text)).await["name"], "Mine");
}

/// The status, content type and body of a request.
async fn call_typed(app: &axum::Router, request: Request<Body>) -> (StatusCode, String, String) {
    let res = app.clone().oneshot(request).await.unwrap();
    let status = res.status();
    let kind = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
    (status, kind, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn export_routes_answer_both_formats_and_404_without_a_copy() {
    let env = env();
    // A prompt that holds a key, in Marshell's copy.
    let transcript = index_session(&env, ID, env.work.path());
    std::fs::write(
        &transcript,
        BASIC.replace("Add a limit of five attempts", "use key sk-ant-abcdefgh12345678"),
    )
    .unwrap();
    let meta = env.state.history().latest_for_tab("t1").unwrap();
    // No copy yet.
    let uri = format!("/v1/history/claude/{ID}/export?format=markdown");
    let (status, _, text) = call_typed(&env.app, req("GET", &uri, None)).await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_COPY));
    copy_transcript(env.home.path(), &meta).unwrap();
    env.state.history().reindex(&meta);

    // Markdown, redacted unless asked otherwise.
    let (status, kind, text) = call_typed(&env.app, req("GET", &uri, None)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(kind, "text/markdown; charset=utf-8");
    assert!(text.starts_with("# Retry limit work\n\n- Folder: "), "{text}");
    assert!(text.contains("## Turn 2\n\n> use key <anthropic-key>\n"), "{text}");
    assert!(!text.contains("abcdefgh12345678"));
    let (_, _, plain) = call_typed(&env.app, req("GET", &format!("{uri}&redact=false"), None)).await;
    assert!(plain.contains("> use key sk-ant-abcdefgh12345678\n"), "{plain}");

    // JSON.
    let json_uri = format!("/v1/history/claude/{ID}/export?format=json");
    let (status, kind, text) = call_typed(&env.app, req("GET", &json_uri, None)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(kind, "application/json");
    let parsed: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["session_id"], ID);
    assert_eq!(parsed["turns"][1]["prompt"], "use key <anthropic-key>");
    let (_, _, plain) = call_typed(&env.app, req("GET", &format!("{json_uri}&redact=false"), None)).await;
    assert_eq!(
        serde_json::from_str::<Value>(&plain).unwrap()["turns"][1]["prompt"],
        "use key sk-ant-abcdefgh12345678"
    );

    // A bad format is a 400; a bad or unknown conversation is a 404 before any lookup.
    let (status, _, _) = call_typed(
        &env.app,
        req("GET", &format!("/v1/history/claude/{ID}/export?format=pdf"), None),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, text) = call_typed(
        &env.app,
        req("GET", "/v1/history/claude/..%2Fx/export?format=json", None),
    )
    .await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
    let other = "00000000-0000-4000-8000-0000000000ff";
    let (status, _, text) = call_typed(
        &env.app,
        req("GET", &format!("/v1/history/claude/{other}/export"), None),
    )
    .await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
}

/// 2026-10-20T09:00:00Z, the time of the first line of the basic transcript fixture.
const RECORDED_AT: u64 = 1_792_486_800_000;
const CAST_NAME: &str = "t1-1792486800000.cast";

/// Seeds the session with a sidecar that lists a recording of 13 output events (ten short lines, then three
/// of 400 kB), one prompt mark at 2 s and the size-limit marker, and reads its transcript into the index.
fn seed_recorded(env: &Env, id: &str) {
    use marshell_core::pty::record::{CastEncoder, CastHeader, header_line};
    let dir = marshell_core::paths::recordings_dir(env.home.path());
    std::fs::create_dir_all(&dir).unwrap();
    let mut text = header_line(&CastHeader {
        cols: 80,
        rows: 24,
        started_unix: RECORDED_AT / 1000,
        shell: None,
    });
    for n in 0..10 {
        text += &CastEncoder::default()
            .output(1.0 + n as f64, format!("line {n}\r\n").as_bytes())
            .unwrap();
    }
    text += &CastEncoder::marker(2.0, "prompt");
    for n in 0..3 {
        text += &CastEncoder::default()
            .output(20.0 + n as f64, &vec![b'z'; 400_000])
            .unwrap();
    }
    text += &CastEncoder::marker(30.0, "Recording stopped: it reached the size limit.");
    std::fs::write(dir.join(CAST_NAME), text).unwrap();

    let transcript = env.work.path().join(format!("{id}.jsonl"));
    std::fs::write(&transcript, BASIC).unwrap();
    for (event, at_ms) in [("SessionStart", RECORDED_AT), ("Stop", RECORDED_AT + 60_000)] {
        let body = json!({
            "hook_event_name": event,
            "session_id": id,
            "transcript_path": transcript.to_string_lossy(),
            "cwd": env.work.path().to_string_lossy(),
        });
        // A Stop asks for a copy of the transcript; this seed makes its own below.
        env.state.history().note_event("t1", Some(CAST_NAME), &body, at_ms);
    }
    let meta = env.state.history().latest_for_tab("t1").unwrap();
    copy_transcript(env.home.path(), &meta).unwrap();
    env.state.history().reindex(&meta);
}

const NO_PART: &str = "No such part of the recording.";

#[tokio::test]
async fn recording_frames_serves_a_compressed_part() {
    let env = env();
    seed_recorded(&env, ID);
    let base = format!("/v1/history/claude/{ID}/recordings");
    let (status, plain_parts) = get_json(&env.app, &base).await;
    assert_eq!(status, StatusCode::OK);
    let (status, first) = get_json(&env.app, &format!("{base}/0/frames")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["screen"].is_string());
    assert!(!first["events"].as_array().unwrap().is_empty());
    let offset = first["next_offset"].as_u64().unwrap();
    let next_url = format!("{base}/0/frames?offset={offset}");
    let (status, next) = get_json(&env.app, &next_url).await;
    assert_eq!(status, StatusCode::OK);
    let path = marshell_core::paths::recordings_dir(env.home.path()).join(CAST_NAME);
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(
        path.with_extension("cast.zst"),
        zstd::encode_all(bytes.as_slice(), 3).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(&path).unwrap();
    let (status, parts) = get_json(&env.app, &base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parts, plain_parts);
    assert_eq!(parts[0]["file"], CAST_NAME);
    let (status, compressed) = get_json(&env.app, &format!("{base}/0/frames")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(compressed, first);
    let (status, compressed) = get_json(&env.app, &next_url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(compressed, next);
}

#[tokio::test]
async fn recordings_route_lists_parts_and_frames_play_them() {
    let env = env();
    seed_recorded(&env, ID);
    let (_, conversation) = get_json(&env.app, &format!("/v1/history/claude/{ID}")).await;
    let first_turn = conversation["turns"][0]["at_ms"].as_u64().unwrap();
    assert!(first_turn > RECORDED_AT);

    let (status, parts) = get_json(&env.app, &format!("/v1/history/claude/{ID}/recordings")).await;
    assert_eq!(status, StatusCode::OK);
    let parts = parts.as_array().unwrap();
    assert_eq!(parts.len(), 1);
    let part = &parts[0];
    assert_eq!(part["index"], 0);
    assert_eq!(part["file"], CAST_NAME);
    assert_eq!((part["cols"].as_u64(), part["rows"].as_u64()), (Some(80), Some(24)));
    assert_eq!(part["started_ms"], RECORDED_AT);
    assert_eq!(part["ended_ms"], RECORDED_AT + 30_000);
    // The span starts at the first prompt and ends at the conversation's last event.
    assert_eq!(part["span_from_ms"], first_turn);
    assert_eq!(part["span_to_ms"], RECORDED_AT + 30_000);
    assert_eq!(part["marks_ms"], json!([RECORDED_AT + 2000]));
    assert_eq!(part["stopped"], "Recording stopped: it reached the size limit.");

    // Chain the frames from the start until the end: every output event comes once, in order.
    let base = format!("/v1/history/claude/{ID}/recordings/0/frames");
    let (status, first) = get_json(&env.app, &base).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["screen"].is_string());
    let mut sizes: Vec<usize> = Vec::new();
    let mut page = first;
    let mut pages = 0;
    loop {
        pages += 1;
        for event in page["events"].as_array().unwrap() {
            if event["code"] == "o" {
                sizes.push(event["data"].as_str().unwrap().len());
            }
        }
        match page["next_offset"].as_u64() {
            Some(offset) => {
                let (status, next) = get_json(&env.app, &format!("{base}?offset={offset}")).await;
                assert_eq!(status, StatusCode::OK);
                assert!(next["screen"].is_null());
                page = next;
            }
            None => break,
        }
    }
    assert!(pages >= 2, "the 1.2 MB of output should not fit one answer");
    assert_eq!(sizes.len(), 13);
    assert_eq!(sizes[10..], [400_000; 3]);

    // A seek into the middle: the screen is what a replay up to there shows.
    let from = RECORDED_AT + 5500;
    let (_, seek) = get_json(&env.app, &format!("{base}?from_ms={from}&to_ms={}", from + 2000)).await;
    assert!(seek["screen"].as_str().unwrap().contains("line 4"));
    assert!(!seek["screen"].as_str().unwrap().contains("line 5"));
    let times: Vec<u64> = seek["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["at_ms"].as_u64().unwrap())
        .collect();
    assert_eq!(times, [RECORDED_AT + 6000, RECORDED_AT + 7000]);
    assert!(seek["next_offset"].is_null());
}

#[tokio::test]
async fn a_conversation_with_no_recording_has_no_parts() {
    let env = env();
    seed(&env, ID);
    let (status, parts) = get_json(&env.app, &format!("/v1/history/claude/{ID}/recordings")).await;
    assert_eq!((status, parts), (StatusCode::OK, json!([])));
}

#[tokio::test]
async fn frames_route_answers_404_for_an_unknown_part_and_400_for_a_bad_offset() {
    let env = env();
    seed_recorded(&env, ID);
    let base = format!("/v1/history/claude/{ID}/recordings");
    let (status, text) = call(&env.app, req("GET", &format!("{base}/1/frames"), None)).await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_PART));
    // Three bytes into the first event line.
    let cast = std::fs::read(marshell_core::paths::recordings_dir(env.home.path()).join(CAST_NAME)).unwrap();
    let inside = cast.iter().position(|b| *b == b'\n').unwrap() + 1 + 3;
    let (status, text) = call(&env.app, req("GET", &format!("{base}/0/frames?offset={inside}"), None)).await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::BAD_REQUEST, "Not a frame boundary.")
    );
}

#[tokio::test]
async fn recordings_route_404s_an_unknown_conversation() {
    let env = env();
    let missing = "00000000-0000-4000-8000-0000000000ff";
    for uri in [
        format!("/v1/history/claude/{missing}/recordings"),
        format!("/v1/history/claude/{missing}/recordings/0/frames"),
        "/v1/history/claude/..%2Fx/recordings".to_string(),
        format!("/v1/history/codex/{ID}/recordings"),
    ] {
        let (status, text) = call(&env.app, req("GET", &uri, None)).await;
        assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH), "{uri}");
    }
}

#[tokio::test]
async fn export_asciicast_answers_cast_and_404s_without_a_recording() {
    let env = env();
    seed_recorded(&env, ID);
    let uri = format!("/v1/history/claude/{ID}/export?format=asciicast");
    let (status, kind, text) = call_typed(&env.app, req("GET", &uri, None)).await;
    assert_eq!((status, kind.as_str()), (StatusCode::OK, "application/x-asciicast"));
    let mut lines = text.lines();
    let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(
        (header["version"].as_u64(), header["width"].as_u64()),
        (Some(2), Some(80))
    );
    assert!(header["title"].is_string());
    let events: Vec<Value> = lines.map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|e| e[0].is_number() && e[1].is_string() && e[2].is_string())
    );
    let (status, raw) = call(&env.app, req("GET", &format!("{uri}&redact=false"), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(raw.lines().count() >= events.len());

    // A conversation with no recording, and an unknown one.
    let bare = self::env();
    seed(&bare, ID);
    let (status, text) = call(&bare.app, req("GET", &uri, None)).await;
    assert_eq!(
        (status, text.as_str()),
        (StatusCode::NOT_FOUND, "Marshell has no recording of this conversation.")
    );
    let missing = "00000000-0000-4000-8000-0000000000ff";
    let (status, text) = call(
        &env.app,
        req(
            "GET",
            &format!("/v1/history/claude/{missing}/export?format=asciicast"),
            None,
        ),
    )
    .await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
}

/// Seeds a Marshell home for the slice 5a live check: the five transcript fixtures as archived Claude sessions, each with
/// its sidecar and zstd copy, in two project folders. The transcripts sit in `<seed>/../transcripts`; the one for
/// `…0b4` is deleted after its copy, so Resume refuses it. Run with
/// `MARSHELL_SEED_HOME=<scratch>\home cargo test -p marshell-core --test history_api seed_history_home -- --ignored`.
#[test]
#[ignore]
fn seed_history_home() {
    use marshell_core::history::SessionMeta;
    let home = PathBuf::from(std::env::var("MARSHELL_SEED_HOME").expect("MARSHELL_SEED_HOME names the home to seed"));
    let root = home.parent().expect("the seed home has a parent folder").to_path_buf();
    let transcripts = root.join("transcripts");
    let (demo, billing) = (root.join("work").join("demo"), root.join("work").join("billing"));
    for dir in [&home, &transcripts, &demo, &billing] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let hour = 3_600_000;
    let sessions: [(&str, &str, &Path, u64); 5] = [
        ("00000000-0000-4000-8000-0000000000b1", BASIC, &demo, 2 * hour),
        (
            "00000000-0000-4000-8000-0000000000b2",
            include_str!("fixtures/claude/transcripts/2.1.309/tools.jsonl"),
            &demo,
            5 * hour,
        ),
        (
            "00000000-0000-4000-8000-0000000000b3",
            include_str!("fixtures/claude/transcripts/2.1.309/subagent-compact.jsonl"),
            &billing,
            26 * hour,
        ),
        (
            "00000000-0000-4000-8000-0000000000b4",
            include_str!("fixtures/claude/transcripts/2.1.309/commands.jsonl"),
            &billing,
            9 * 24 * hour,
        ),
        (
            "00000000-0000-4000-8000-0000000000b5",
            include_str!("fixtures/claude/transcripts/9.9.9/future.jsonl"),
            &demo,
            40 * 24 * hour,
        ),
    ];
    for (n, (id, text, cwd, age)) in sessions.into_iter().enumerate() {
        let transcript = transcripts.join(format!("{id}.jsonl"));
        std::fs::write(&transcript, text).unwrap();
        let meta = SessionMeta {
            agent: "claude".into(),
            session_id: id.into(),
            tab_id: format!("seed-tab-{n}"),
            cwd: cwd.to_string_lossy().into_owned(),
            transcript_path: transcript.to_string_lossy().into_owned(),
            started_ms: now - age - 60_000,
            updated_ms: now - age,
            previous_in_tab: None,
            recordings: vec![],
            client: Default::default(),
        };
        copy_transcript(&home, &meta).unwrap();
        if id.ends_with("b4") {
            std::fs::remove_file(&transcript).unwrap();
        }
    }
}

const CLAUDE_OUTSIDE: &str = "00000000-0000-4000-8000-0000000000e1";
const CODEX_OUTSIDE: &str = "00000000-0000-4000-8000-0000000000e2";

#[tokio::test]
async fn import_status_route_reports_progress() {
    use marshell_core::history::import::{ImportRoots, Timing};
    let env = env();
    // Without an importer the route answers from the settings alone.
    let (status, idle) = get_json(&env.app, "/v1/history/import").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(idle, json!({ "on": true, "copying": false, "done": 0, "total": 0 }));

    let claude = tempfile::tempdir().unwrap();
    let codex = tempfile::tempdir().unwrap();
    let project = claude.path().join("p1");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join(format!("{CLAUDE_OUTSIDE}.jsonl")), BASIC).unwrap();
    let day = codex.path().join("2026").join("10").join("21");
    std::fs::create_dir_all(&day).unwrap();
    let rollout = json!({
        "timestamp": "2026-10-21T09:00:00.000Z", "ordinal": 0, "type": "session_meta",
        "payload": { "id": CODEX_OUTSIDE, "timestamp": "2026-10-21T09:00:00.000Z", "cwd": "/work/demo",
            "originator": "codex-tui", "thread_source": "user" },
    });
    std::fs::write(
        day.join(format!("rollout-2026-10-21T09-00-00-{CODEX_OUTSIDE}.jsonl")),
        format!("{rollout}\n"),
    )
    .unwrap();

    env.state.start_importer_with(
        ImportRoots {
            claude: Some(claude.path().to_path_buf()),
            codex: Some(codex.path().to_path_buf()),
        },
        Timing {
            settle: Duration::from_millis(100),
            rescan: Duration::from_millis(300),
        },
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let (status, body) = get_json(&env.app, "/v1/history/import").await;
        assert_eq!(status, StatusCode::OK);
        if body == json!({ "on": true, "copying": false, "done": 2, "total": 2 }) {
            break;
        }
        assert!(Instant::now() < deadline, "the import did not finish: {body}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(archive_paths(env.home.path(), "claude", CLAUDE_OUTSIDE).0.exists());
    assert!(archive_paths(env.home.path(), "codex", CODEX_OUTSIDE).0.exists());

    // The settings route flips the switch.
    let (_, mut settings) = get_json(&env.app, "/v1/settings").await;
    assert_eq!(settings["history_import"], true);
    settings["history_import"] = json!(false);
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(settings))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, off) = get_json(&env.app, "/v1/history/import").await;
    assert_eq!(off["on"], false);
}

#[tokio::test]
async fn import_status_route_refuses_without_the_bearer() {
    let env = env();
    let mut request = req("GET", "/v1/history/import", None);
    request.headers_mut().remove(header::AUTHORIZATION);
    let (status, _) = call(&env.app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

const CODEX_ID: &str = "00000000-0000-4000-8000-0000000000c2";
const CODEX_ROLLOUT: &str = include_str!("fixtures/codex/exec.jsonl");

/// Copies the exec fixture into the archive as a Codex conversation run outside Marshell. The rollout file sits
/// in a Codex-style folder under `dir`, and the conversation's folder is the env's work folder.
fn seed_codex(env: &Env, dir: &Path) -> PathBuf {
    let folder = dir.join("sessions/2026/10/24");
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join(format!("rollout-2026-10-24T09-00-00-{CODEX_ID}.jsonl"));
    std::fs::write(&source, CODEX_ROLLOUT).unwrap();
    let meta = marshell_core::history::SessionMeta {
        agent: "codex".into(),
        session_id: CODEX_ID.into(),
        tab_id: String::new(),
        cwd: env.work.path().to_string_lossy().into_owned(),
        transcript_path: source.to_string_lossy().into_owned(),
        started_ms: 1_000,
        updated_ms: 2_000,
        previous_in_tab: None,
        recordings: Vec::new(),
        client: marshell_protocol::history::Client::Terminal,
    };
    assert!(env.state.history().import(&meta, &source).unwrap());
    source
}

#[tokio::test]
async fn a_codex_conversation_opens_from_the_archive() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    seed_codex(&env, dir.path());
    let (status, body) = get_json(&env.app, &format!("/v1/history/codex/{CODEX_ID}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["agent"], "codex");
    assert_eq!(body["session_id"], CODEX_ID);
    assert!(!body["turns"].as_array().unwrap().is_empty(), "{body}");
}

#[tokio::test]
async fn search_finds_a_codex_turn() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    seed_codex(&env, dir.path());
    let (status, hits) = get_json(&env.app, "/v1/history/search?q=tests").await;
    assert_eq!(status, StatusCode::OK, "{hits}");
    assert!(hits.as_array().unwrap().iter().any(|h| h["agent"] == "codex"), "{hits}");
}

#[tokio::test]
async fn export_works_for_a_codex_conversation() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    seed_codex(&env, dir.path());
    let uri = format!("/v1/history/codex/{CODEX_ID}/export?format=markdown");
    let (status, text) = call(&env.app, req("GET", &uri, None)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(text.contains("Run the tests"), "{text}");
}

#[tokio::test]
async fn an_unknown_agent_is_not_found() {
    let env = env();
    let (status, text) = call(&env.app, req("GET", &format!("/v1/history/gemini/{ID}"), None)).await;
    assert_eq!((status, text.as_str()), (StatusCode::NOT_FOUND, NO_SUCH));
}

#[tokio::test]
async fn fork_refuses_a_codex_conversation() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    seed_codex(&env, dir.path());
    let (status, text) = call(
        &env.app,
        req(
            "POST",
            &format!("/v1/history/codex/{CODEX_ID}/fork"),
            Some(resume_body()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["reason"], "Fork works only for Claude Code conversations.");
    assert_eq!(env.state.live_session_count(), 0);
}

#[tokio::test]
async fn resume_opens_codex_in_the_conversations_folder() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    seed_codex(&env, dir.path());
    let (status, text) = call(
        &env.app,
        req(
            "POST",
            &format!("/v1/history/codex/{CODEX_ID}/resume"),
            Some(resume_body()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let created: Value = serde_json::from_str(&text).unwrap();
    let tab = created["tab_id"].as_str().unwrap().to_string();
    let rows = peek_until(&env.app, &tab, &format!("resume {CODEX_ID}")).await;
    assert!(rows.contains("codex"), "{rows}");
    assert!(!rows.contains("--last"), "{rows}");
    let (_, sessions) = get_json(&env.app, "/v1/sessions").await;
    let row = sessions["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab.as_str())
        .unwrap();
    assert_eq!(row["agent"], "codex");
    assert_eq!(row["cwd"], env.work.path().to_string_lossy().as_ref());
    if let Some(session) = env.state.remove_session(&tab) {
        session.force_kill();
    }
}

/// Writes a file of `body` that was last modified `days` days ago.
fn write_aged(path: &Path, body: &[u8], days: u64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(days * 86_400))
        .unwrap();
}

/// The storage thread with nothing to wait for: an hour between ticks, and files count at once.
fn start_storage(env: &Env) {
    env.state
        .start_storage_with(marshell_core::history::storage::StorageTiming {
            tick: Duration::from_secs(3600),
            cleanup_every: Duration::from_secs(3600),
            settle: Duration::ZERO,
        });
}

#[tokio::test]
async fn storage_preview_refuses_without_the_bearer() {
    let env = env();
    start_storage(&env);
    let mut request = req("POST", "/v1/history/storage/preview", Some(json!({ "limit_days": 30 })));
    request.headers_mut().remove(header::AUTHORIZATION);
    let (status, _) = call(&env.app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn storage_report_refuses_without_the_bearer_and_answers_503_without_storage() {
    let env = env();
    let (status, _) = call(&env.app, req("GET", "/v1/history/storage", None)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "no storage thread yet");
    start_storage(&env);
    let mut request = req("GET", "/v1/history/storage", None);
    request.headers_mut().remove(header::AUTHORIZATION);
    let (status, _) = call(&env.app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = call(&env.app, req("GET", "/v1/history/storage", None)).await;
    assert_eq!(status, StatusCode::OK);
    let report: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(report["transcripts"], 0);
    assert_eq!(report["backup"]["dir"], Value::Null);
}

#[tokio::test]
async fn storage_preview_refuses_a_limit_off_the_list_and_answers_503_without_storage() {
    let env = env();
    let (status, _) = call(
        &env.app,
        req("POST", "/v1/history/storage/preview", Some(json!({ "limit_days": 30 }))),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "no storage thread yet");
    start_storage(&env);
    for body in [
        json!({ "limit_gb": 3 }),
        json!({ "limit_days": 7 }),
        json!({ "limit_gb": 0 }),
    ] {
        let (status, _) = call(&env.app, req("POST", "/v1/history/storage/preview", Some(body.clone()))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let (status, body) = call(&env.app, req("POST", "/v1/history/storage/preview", Some(json!({})))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({ "files": 0, "bytes": 0, "oldest_ms": null })
    );
}

#[tokio::test]
async fn the_settings_route_refuses_a_limit_off_the_list_and_keeps_a_listed_one() {
    let env = env();
    let (_, settings) = get_json(&env.app, "/v1/settings").await;
    assert_eq!(settings["history_limit_gb"], Value::Null);
    assert_eq!(settings["history_limit_days"], Value::Null);
    for (gb, days) in [(json!(3), Value::Null), (Value::Null, json!(7))] {
        let mut bad = settings.clone();
        bad["history_limit_gb"] = gb;
        bad["history_limit_days"] = days;
        let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(bad))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let mut good = settings.clone();
    good["history_limit_gb"] = json!(5);
    good["history_limit_days"] = json!(90);
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(good))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, back) = get_json(&env.app, "/v1/settings").await;
    assert_eq!(back["history_limit_gb"], 5);
    assert_eq!(back["history_limit_days"], 90);
}

#[tokio::test]
async fn the_settings_route_refuses_a_backup_folder_with_the_reason_and_does_not_save_it() {
    let env = env();
    let (_, settings) = get_json(&env.app, "/v1/settings").await;
    assert_eq!(settings["history_backup_dir"], Value::Null);
    let outer = tempfile::tempdir().unwrap();
    let theirs = tempfile::tempdir().unwrap();
    let target = theirs.path().join("Marshell history");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(
        target.join(".marshell-backup.json"),
        r#"{"install_id":"00000000000000000000000000000000"}"#,
    )
    .unwrap();
    let unmarked = tempfile::tempdir().unwrap();
    std::fs::create_dir(unmarked.path().join("Marshell history")).unwrap();
    std::fs::write(unmarked.path().join("Marshell history").join("notes.txt"), b"mine").unwrap();
    let cases = [
        (outer.path().join("nope"), "That folder doesn't exist."),
        (
            env.home.path().to_path_buf(),
            "Choose a folder outside Marshell's own folder.",
        ),
        (
            theirs.path().to_path_buf(),
            "That folder holds another Marshell's backup. Choose another folder.",
        ),
        (
            unmarked.path().to_path_buf(),
            "That folder already has a \"Marshell history\" folder that Marshell didn't make. Choose another folder.",
        ),
    ];
    for (dir, copy) in cases {
        let mut bad = settings.clone();
        bad["history_backup_dir"] = json!(dir.to_string_lossy());
        let (status, body) = call(&env.app, req("PUT", "/v1/settings", Some(bad))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{dir:?}");
        assert_eq!(body, copy);
        let (_, back) = get_json(&env.app, "/v1/settings").await;
        assert_eq!(back["history_backup_dir"], Value::Null, "{dir:?} was saved");
    }
    assert_eq!(
        std::fs::read(unmarked.path().join("Marshell history").join("notes.txt")).unwrap(),
        b"mine"
    );
    let good = tempfile::tempdir().unwrap();
    let mut ok = settings.clone();
    ok["history_backup_dir"] = json!(good.path().to_string_lossy());
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(ok))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, back) = get_json(&env.app, "/v1/settings").await;
    assert_eq!(back["history_backup_dir"], json!(good.path().to_string_lossy()));
}

#[tokio::test]
async fn a_saved_age_limit_removes_what_the_preview_listed_and_the_last_cleanup_says_so() {
    let env = env();
    let rec = env.home.path().join("history/recordings");
    write_aged(&rec.join("t1-1000.cast.zst"), b"12345", 40);
    write_aged(&rec.join("t2-2000.cast.zst"), b"1234567", 60);
    write_aged(&rec.join("t3-3000.cast.zst"), b"new", 1);
    // A pinned conversation's recording is as old as the rest, and stays.
    write_aged(&rec.join("t4-4000.cast.zst"), b"pinned", 100);
    let transcript = env.work.path().join(format!("{ID}.jsonl"));
    std::fs::write(&transcript, BASIC).unwrap();
    let meta = marshell_core::history::SessionMeta {
        agent: "claude".into(),
        session_id: ID.into(),
        tab_id: "t4".into(),
        cwd: env.work.path().to_string_lossy().into_owned(),
        transcript_path: transcript.to_string_lossy().into_owned(),
        started_ms: 1,
        updated_ms: 2,
        previous_in_tab: None,
        recordings: vec![marshell_core::history::RecordingLink {
            file: "t4-4000.cast".into(),
            first_ms: 1,
            last_ms: 2,
        }],
        client: Default::default(),
    };
    marshell_core::history::archive::write_meta(env.home.path(), &meta).unwrap();
    let pinned = marks::Marks {
        pinned: true,
        ..marks::Marks::default()
    };
    marks::write(env.home.path(), "claude", ID, &pinned).unwrap();
    start_storage(&env);

    let (status, preview) = call(
        &env.app,
        req("POST", "/v1/history/storage/preview", Some(json!({ "limit_days": 30 }))),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let preview: Value = serde_json::from_str(&preview).unwrap();
    assert_eq!(preview["files"], 2);
    assert_eq!(preview["bytes"], 12);
    let oldest = preview["oldest_ms"].as_u64().unwrap();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let sixty_days_ago = now_ms - 60 * 86_400_000;
    assert!(oldest.abs_diff(sixty_days_ago) < 60_000, "{oldest} vs {sixty_days_ago}");
    // Previewing removes nothing.
    assert!(rec.join("t1-1000.cast.zst").exists() && rec.join("t2-2000.cast.zst").exists());
    let storage = env.state.storage().unwrap();
    // The first tick found no limit saved.
    assert_eq!(storage.last_cleanup(), None);

    let (_, mut settings) = get_json(&env.app, "/v1/settings").await;
    settings["history_limit_days"] = json!(30);
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(settings))).await;
    assert_eq!(status, StatusCode::OK);
    let deadline = Instant::now() + Duration::from_secs(5);
    while storage.last_cleanup().is_none() {
        assert!(Instant::now() < deadline, "the cleanup did not run");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let run = storage.last_cleanup().unwrap();
    assert_eq!((run.files, run.bytes), (2, 12));
    assert!(!rec.join("t1-1000.cast.zst").exists());
    assert!(!rec.join("t2-2000.cast.zst").exists());
    assert!(rec.join("t3-3000.cast.zst").exists());
    assert!(
        rec.join("t4-4000.cast.zst").exists(),
        "the pinned conversation's recording stays"
    );
}

#[tokio::test]
async fn the_report_shows_the_saved_backup_folder_at_once() {
    let env = env();
    start_storage(&env);
    let (_, settings) = get_json(&env.app, "/v1/settings").await;
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut to_a = settings.clone();
    to_a["history_backup_dir"] = json!(a.path().to_string_lossy());
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(to_a))).await;
    assert_eq!(status, StatusCode::OK);
    let storage = env.state.storage().unwrap();
    let started = std::time::Instant::now();
    while storage.backup_status().last_copy_ms.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the mirror to folder A never finished"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let (_, report) = get_json(&env.app, "/v1/history/storage").await;
    assert_eq!(report["backup"]["dir"], json!(a.path().to_string_lossy()));
    assert!(report["backup"]["last_copy_ms"].is_u64());
    // The thread ends here, so the new folder can't be mirrored before the report is read.
    storage.stop();
    let mut to_b = settings.clone();
    to_b["history_backup_dir"] = json!(b.path().to_string_lossy());
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(to_b))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, report) = get_json(&env.app, "/v1/history/storage").await;
    assert_eq!(report["backup"]["dir"], json!(b.path().to_string_lossy()));
    assert_eq!(report["backup"]["last_copy_ms"], Value::Null);
    assert_eq!(report["backup"]["error"], Value::Null);
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(settings))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, report) = get_json(&env.app, "/v1/history/storage").await;
    assert_eq!(report["backup"]["dir"], Value::Null);
    assert_eq!(report["backup"]["last_copy_ms"], Value::Null);
    assert_eq!(report["backup"]["error"], Value::Null);
}

#[tokio::test]
async fn a_cleanup_keeps_the_images_of_a_tab_restored_from_the_last_run() {
    let env = env();
    // A tab from the last run: in the sidebar as Ended, with no session.
    let saved = marshell_protocol::api::SavedTab {
        tab_id: "t7".into(),
        number: 7,
        name: "Last run".into(),
        agent: None,
        profile_id: "printer".into(),
        cwd: env.work.path().to_string_lossy().into_owned(),
        mode: None,
        muted: false,
    };
    env.state
        .registry()
        .seed_ended(&[saved], |tab| marshell_core::registry::Relaunch {
            profile_id: tab.profile_id.clone(),
            agent: None,
            cwd: tab.cwd.clone(),
            mode: None,
            name: tab.name.clone(),
        });
    let image = marshell_core::paths::attachments_dir(env.home.path())
        .join("t7")
        .join("a.png");
    write_aged(&image, b"png", 90);
    let (_, mut settings) = get_json(&env.app, "/v1/settings").await;
    settings["history_limit_days"] = json!(30);
    let (status, _) = call(&env.app, req("PUT", "/v1/settings", Some(settings))).await;
    assert_eq!(status, StatusCode::OK);
    start_storage(&env);
    let storage = env.state.storage().unwrap();
    let started = Instant::now();
    while storage.last_cleanup().is_none() {
        assert!(started.elapsed() < Duration::from_secs(20), "no cleanup ran");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(image.exists(), "the image of a tab still in the sidebar was removed");
}
