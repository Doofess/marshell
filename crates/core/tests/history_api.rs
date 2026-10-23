//! The History routes: list, search, one conversation, and resume.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::history::archive::{archive_paths, copy_transcript};
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

fn env_in(home: tempfile::TempDir, work: tempfile::TempDir) -> Env {
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.path().to_path_buf(),
        vec![printer()],
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
    assert!(env.state.history().note_event("t1", &body, 1000).is_none());
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
    let env = env();
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
    env.state.history().note_event(&tab, &body, 2000);
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
    env.state.history().note_event(&tab, &body, 3000);
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
    // A bad id is still a 404 first.
    let (status, text) = call(&env.app, req("GET", "/v1/history/codex/x", None)).await;
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
    use marshell_core::history::marks::{self, Marks};
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
    // A session not yet parsed has no turn times: its bookmark falls back to 0.
    index_session(&env, "unparsed", env.work.path());
    assert_eq!(
        call(&env.app, req("PUT", "/v1/history/claude/unparsed/bookmarks/0", None))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(marks::read(env.home.path(), "claude", "unparsed").bookmarks, vec![0]);
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
    env.state.history().note_event("t1", &json!({"hook_event_name":"SessionStart", "session_id":"newer", "transcript_path":env.work.path().join("newer.jsonl"), "cwd":env.work.path()}), 2000);
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
        };
        copy_transcript(&home, &meta).unwrap();
        if id.ends_with("b4") {
            std::fs::remove_file(&transcript).unwrap();
        }
    }
}
