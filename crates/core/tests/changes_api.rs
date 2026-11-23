//! The Changes API: what the checkout the shell is in has changed, and the marks the Files tree carries.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::ShellFamily;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::ServiceExt;

use common::{ORIGIN, PORT, TOKEN};

/// `sleeper` reports no folder. `reporter` reports `reported` as its folder, as a shell does.
fn state_in(home: &Path, reported: &Path) -> AppState {
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let profiles = vec![
        common::family_profile("sleeper", vec![fake.into(), "sleep".into()], ShellFamily::Posix),
        common::family_profile(
            "reporter",
            vec![fake.into(), "folder".into(), reported.to_string_lossy().into_owned()],
            ShellFamily::Posix,
        ),
    ];
    AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()], home.to_path_buf(), profiles)
}

fn request(method: &str, path: &str, body: Option<serde_json::Value>, token: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"));
    if token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    app.clone().oneshot(request(method, path, body, true)).await.unwrap()
}

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// The path as the core spells it: canonical, without the `\\?\` prefix.
fn canon(path: &Path) -> String {
    let real = std::fs::canonicalize(path).unwrap();
    let text = real.to_string_lossy().into_owned();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A scratch folder with an empty git config to point plain git at, and a Marshell `home/`.
struct Scratch {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Scratch {
    fn new() -> Scratch {
        let tmp = tempfile::tempdir().unwrap();
        let root = PathBuf::from(canon(tmp.path()));
        std::fs::write(root.join("empty.gitconfig"), "").unwrap();
        std::fs::create_dir(root.join("home")).unwrap();
        Scratch { _tmp: tmp, root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// Plain git that never reads the person's own config or inherited `GIT_*` variables.
    fn git(&self, dir: &Path, args: &[&str]) -> String {
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(dir)
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().to_ascii_uppercase().starts_with("GIT_") {
                cmd.env_remove(name);
            }
        }
        cmd.env("GIT_CONFIG_GLOBAL", self.root.join("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0");
        let out = cmd.output().expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repository `name` with `a.txt`, `sub/b.txt` and `sub/gone.txt`, committed on `main`.
    fn repo(&self, name: &str) -> PathBuf {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        self.git(&dir, &["init", "-q", "-b", "main"]);
        write(&dir.join("a.txt"), "one\n");
        write(&dir.join("sub/b.txt"), "two\n");
        write(&dir.join("sub/gone.txt"), "three\n");
        self.git(&dir, &["add", "-A"]);
        self.git(&dir, &["commit", "-q", "-m", "first"]);
        dir
    }

    fn plain(&self, name: &str) -> PathBuf {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

/// A session of `profile_id` started in `cwd`.
async fn session(app: &axum::Router, profile_id: &str, cwd: &Path) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": profile_id, "cwd": cwd.to_string_lossy(), "agent": null,
    });
    let res = call(app, "POST", "/v1/sessions", Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

/// Waits until the shell of `tab` has reported its folder.
async fn reported(state: &AppState, tab: &str) {
    let started = Instant::now();
    while state.session(tab).and_then(|s| s.current_cwd()).is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the shell never reported a folder"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn changes(app: &axum::Router, tab: &str, stamp: Option<&str>) -> serde_json::Value {
    let body = serde_json::json!({ "stamp": stamp, "hide_whitespace": false });
    let res = call(app, "POST", &format!("/v1/sessions/{tab}/changes"), Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

async fn tree(app: &axum::Router, tab: &str, stamp: Option<u64>) -> serde_json::Value {
    let body = serde_json::json!({ "dirs": [{ "dir": "", "stamp": stamp }] });
    let res = call(app, "POST", &format!("/v1/sessions/{tab}/tree"), Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

/// The marks of a tree reply as `path:status` texts, sorted.
fn marks(reply: &serde_json::Value) -> Vec<String> {
    let mut marks: Vec<String> = reply["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| format!("{}:{}", m["path"].as_str().unwrap(), m["status"].as_str().unwrap()))
        .collect();
    marks.sort();
    marks
}

fn files(reply: &serde_json::Value) -> Vec<String> {
    let mut files: Vec<String> = reply["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| format!("{}:{}", f["path"].as_str().unwrap(), f["status"].as_str().unwrap()))
        .collect();
    files.sort();
    files
}

#[tokio::test]
async fn changes_lists_a_checkout_and_then_answers_same() {
    let scratch = Scratch::new();
    let repo = scratch.repo("repo");
    write(&repo.join("a.txt"), "one!\n");
    let app = router(state_in(&scratch.home(), &repo));
    let tab = session(&app, "sleeper", &repo).await;

    let first = changes(&app, &tab, None).await;
    assert_eq!(first["kind"], "listed");
    assert_eq!(first["folder"], canon(&repo));
    assert_eq!(first["moved"], false);
    assert_eq!(first["head"].as_str().map(str::len), Some(40), "HEAD's full commit id");
    assert_eq!(files(&first), ["a.txt:modified"]);
    assert!(first["diff"].as_str().unwrap().contains("+one!"));

    let stamp = first["stamp"].as_str().unwrap().to_string();
    let second = changes(&app, &tab, Some(&stamp)).await;
    assert_eq!(second["kind"], "same");
    assert_eq!(second["stamp"], stamp.as_str());
    assert_eq!(second["folder"], canon(&repo));
    assert_eq!(second["files"].as_array().unwrap().len(), 0);
    assert_eq!(second["diff"], "");

    // Another request for the same changes with whitespace hidden has another stamp, so it lists.
    let body = serde_json::json!({ "stamp": stamp, "hide_whitespace": true });
    let res = call(&app, "POST", &format!("/v1/sessions/{tab}/changes"), Some(body)).await;
    assert_eq!(json_of(res).await["kind"], "listed");
}

#[tokio::test]
async fn changes_in_a_plain_folder_is_no_repo() {
    let scratch = Scratch::new();
    let plain = scratch.plain("plain");
    let app = router(state_in(&scratch.home(), &plain));
    let tab = session(&app, "sleeper", &plain).await;
    let reply = changes(&app, &tab, None).await;
    assert_eq!(reply["kind"], "no_repo");
    assert_eq!(reply["folder"], plain.to_string_lossy().as_ref());
    assert_eq!(reply["files"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn changes_follows_the_shell_from_one_checkout_into_another() {
    let scratch = Scratch::new();
    let a = scratch.repo("a");
    let b = scratch.repo("b");
    write(&a.join("a.txt"), "launch\n");
    write(&b.join("sub/b.txt"), "reported\n");
    let state = state_in(&scratch.home(), &b);
    let app = router(state.clone());
    let tab = session(&app, "reporter", &a).await;
    reported(&state, &tab).await;
    let reply = changes(&app, &tab, None).await;
    assert_eq!(reply["kind"], "listed");
    assert_eq!(reply["folder"], canon(&b));
    assert_eq!(reply["moved"], true);
    assert_eq!(files(&reply), ["sub/b.txt:modified"]);
}

#[tokio::test]
async fn changes_for_an_unknown_tab_is_404() {
    let scratch = Scratch::new();
    let app = router(state_in(&scratch.home(), &scratch.root));
    let body = serde_json::json!({ "stamp": null, "hide_whitespace": false });
    let res = call(&app, "POST", "/v1/sessions/nope/changes", Some(body)).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn changes_needs_the_token() {
    let scratch = Scratch::new();
    let app = router(state_in(&scratch.home(), &scratch.root));
    let body = serde_json::json!({ "stamp": null, "hide_whitespace": false });
    let res = app
        .clone()
        .oneshot(request("POST", "/v1/sessions/nope/changes", Some(body), false))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// A session launched in a plain folder whose shell reports `inside`: the tree's root is `inside`, in a
/// checkout git doesn't run in, and the Changes route reads that checkout.
async fn shell_in(scratch: &Scratch, inside: &Path) -> (AppState, axum::Router, String) {
    let launch = scratch.plain("launch");
    let state = state_in(&scratch.home(), inside);
    let app = router(state.clone());
    let tab = session(&app, "reporter", &launch).await;
    reported(&state, &tab).await;
    (state, app, tab)
}

/// Polls the tree until its marks are not `before` and returns that reply.
async fn tree_until_marks_change(
    app: &axum::Router,
    tab: &str,
    stamp: Option<u64>,
    before: &[String],
) -> serde_json::Value {
    let started = Instant::now();
    loop {
        let reply = tree(app, tab, stamp).await;
        if marks(&reply) != before {
            return reply;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the marks never changed from {before:?}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn tree_marks_are_relative_to_the_root_and_skip_deleted() {
    let scratch = Scratch::new();
    let repo = scratch.repo("repo");
    write(&repo.join("a.txt"), "one!\n");
    write(&repo.join("sub/b.txt"), "two!\n");
    write(&repo.join("sub/new.txt"), "new\n");
    std::fs::remove_file(repo.join("sub/gone.txt")).unwrap();
    let (_state, app, tab) = shell_in(&scratch, &repo.join("sub")).await;

    // No check has run yet: the first reply has no marks, and the check starts in the background.
    let first = tree(&app, &tab, None).await;
    assert_eq!(first["root"], canon(&repo.join("sub")));
    assert_eq!(marks(&first), Vec::<String>::new());
    // The next polls have them: only what is under the root, with the root's prefix gone, and no deleted file.
    let later = tree_until_marks_change(&app, &tab, None, &[]).await;
    assert_eq!(marks(&later), ["b.txt:modified", "new.txt:untracked"]);
}

#[tokio::test]
async fn tree_marks_reach_a_same_reply_after_an_edit() {
    let scratch = Scratch::new();
    let repo = scratch.repo("repo");
    write(&repo.join("a.txt"), "one!\n");
    let (_state, app, tab) = shell_in(&scratch, &repo).await;

    let first = tree(&app, &tab, None).await;
    let stamp = first["dirs"][0]["stamp"].as_u64().unwrap();
    let before = tree_until_marks_change(&app, &tab, None, &[]).await;
    assert_eq!(marks(&before), ["a.txt:modified"]);

    // An edit in place does not touch the folder's modified time, so the folder stays `same`.
    write(&repo.join("sub/b.txt"), "two!\n");
    let after = tree_until_marks_change(&app, &tab, Some(stamp), &marks(&before)).await;
    assert_eq!(after["dirs"][0]["state"], "same");
    assert_eq!(marks(&after), ["a.txt:modified", "sub/b.txt:modified"]);
}

#[tokio::test]
async fn tree_marks_are_empty_outside_git() {
    let scratch = Scratch::new();
    let plain = scratch.plain("plain");
    write(&plain.join("a.txt"), "x\n");
    let app = router(state_in(&scratch.home(), &plain));
    let tab = session(&app, "sleeper", &plain).await;
    for _ in 0..3 {
        let reply = tree(&app, &tab, None).await;
        assert_eq!(marks(&reply), Vec::<String>::new());
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

// ---- The review command and the comments ----

/// A session whose program (`mode`: `capture-input` or `capture-input-plain`) appends every byte typed to it to
/// `typed.log` in `dir`. Claude's version and settings are pinned to scratch values, so no test runs a real `claude`.
/// With `agent` set the row is that agent's. Waits until the program takes input.
async fn agent_session(dir: &Path, agent: Option<&str>, mode: &str) -> (AppState, axum::Router, String, PathBuf) {
    let log = dir.join("typed.log");
    let home = dir.join("home");
    let command = vec![
        env!("CARGO_BIN_EXE_fake-agent").into(),
        mode.into(),
        log.to_string_lossy().into_owned(),
    ];
    let state = AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.clone(),
        vec![common::family_profile("capture", command, ShellFamily::Posix)],
    )
    .with_claude_version(Some("2.1.302".into()))
    .with_claude_compact_paths(home.join("user"), Vec::new())
    .with_claude_settings(home.join("claude-settings.json"))
    .with_agent_check(|_| true)
    .with_tab_path(|| None);
    let app = router(state.clone());
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "capture", "cwd": dir.to_string_lossy(), "agent": agent,
    });
    let res = call(&app, "POST", "/v1/sessions", Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    let tab = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    let session = state.session(&tab).unwrap();
    let started = Instant::now();
    loop {
        // `capture-input` asks for bracketed paste once its input is raw; the plain mode says it is ready.
        let ready = if mode == "capture-input" {
            session.bracketed_paste()
        } else {
            matches!(
                session.pull(0, 64 * 1024),
                marshell_core::pty::Pull::Data { bytes, .. } if String::from_utf8_lossy(&bytes).contains("plain-ready")
            )
        };
        if ready {
            return (state, app, tab, log);
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the program never took input"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn review(app: &axum::Router, tab: &str, body: serde_json::Value) -> axum::response::Response {
    call(app, "POST", &format!("/v1/sessions/{tab}/review"), Some(body)).await
}

async fn comments(app: &axum::Router, tab: &str, text: &str) -> axum::response::Response {
    let body = serde_json::json!({ "text": text });
    call(app, "POST", &format!("/v1/sessions/{tab}/comments"), Some(body)).await
}

async fn reason_of(res: axum::response::Response) -> String {
    json_of(res).await["reason"].as_str().unwrap_or_default().to_string()
}

#[tokio::test]
async fn review_types_the_command_with_no_enter() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input").await;
    let res = review(&app, &tab, serde_json::json!({ "base": null })).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_of(res).await["typed"], "/code-review");
    let bytes = common::typed(&log, 12).await;
    assert_eq!(bytes, b"/code-review");
    assert!(!bytes.contains(&b'\r') && !bytes.contains(&b'\n'));
}

#[tokio::test]
async fn review_types_a_request_for_an_agent_with_no_command() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("antigravity"), "capture-input").await;
    let res = review(&app, &tab, serde_json::json!({ "base": "main" })).await;
    assert_eq!(res.status(), StatusCode::OK);
    let want =
        "Review the changes in this checkout against main, file by file, and list each problem with its file and line.";
    assert_eq!(json_of(res).await["typed"], want);
    assert_eq!(common::typed(&log, want.len()).await, want.as_bytes());
}

#[tokio::test]
async fn review_refuses_a_bad_base() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input").await;
    for base in ["x".repeat(201), "ma\nin".to_string(), "ma\u{1b}in".to_string()] {
        let res = review(&app, &tab, serde_json::json!({ "base": base })).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{base:?}");
    }
    assert_eq!(common::typed(&log, 1).await, b"");
    let res = review(&app, "nope", serde_json::json!({ "base": null })).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn review_for_a_shell_with_no_agent_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), None, "capture-input").await;
    let res = review(&app, &tab, serde_json::json!({ "base": null })).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(reason_of(res).await, "no_agent");
    let res = comments(&app, &tab, "a").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(reason_of(res).await, "no_agent");
    assert_eq!(common::typed(&log, 1).await, b"");
}

#[tokio::test]
async fn comments_paste_bracketed_with_cr_line_breaks() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input").await;
    let res = comments(&app, &tab, "a\nb").await;
    assert_eq!(res.status(), StatusCode::OK);
    let reply = json_of(res).await;
    assert_eq!(reply["form"], "paste");
    assert!(reply["path"].is_null());
    assert_eq!(common::typed(&log, 11).await, b"\x1b[200~a\rb\x1b[201~");
    assert!(!tmp.path().join("home/attachments").exists());
}

#[tokio::test]
async fn comments_go_by_file_when_bracketed_paste_is_off() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input-plain").await;
    let res = comments(&app, &tab, "a\nb").await;
    assert_eq!(res.status(), StatusCode::OK);
    let reply = json_of(res).await;
    assert_eq!(reply["form"], "file");
    let path = PathBuf::from(reply["path"].as_str().unwrap());
    let folder = tmp.path().join("home/attachments").join(&tab);
    assert_eq!(path.parent().unwrap(), folder);
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("comments-") && name.ends_with(".md"), "{name}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\n");
    // No temp file is left behind.
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
    let want = format!("Read my review comments in \"{}\"", path.display());
    let bytes = common::typed(&log, want.len()).await;
    assert_eq!(bytes, want.as_bytes());
    assert!(!bytes.contains(&b'\r') && !bytes.contains(&b'\n') && !bytes.contains(&0x1b));
}

#[tokio::test]
async fn comments_over_paste_max_go_by_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input").await;
    let text = "x".repeat(16_001);
    let res = comments(&app, &tab, &text).await;
    assert_eq!(res.status(), StatusCode::OK);
    let reply = json_of(res).await;
    assert_eq!(reply["form"], "file");
    let path = PathBuf::from(reply["path"].as_str().unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{text}\n"));
    // One line, pasted as the program asked: the reference, not the comments.
    let want = format!("\x1b[200~Read my review comments in \"{}\"\x1b[201~", path.display());
    assert_eq!(common::typed(&log, want.len()).await, want.as_bytes());
    // Exactly 16,000 characters still go by paste.
    let res = comments(&app, &tab, &"y".repeat(16_000)).await;
    assert_eq!(json_of(res).await["form"], "paste");
}

#[tokio::test]
async fn comments_refuse_an_escape_or_a_cr() {
    let tmp = tempfile::tempdir().unwrap();
    let (_state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input").await;
    let too_long = "x".repeat(1024 * 1024 + 1);
    for text in ["a\u{1b}[201~b", "a\rb", "a\u{7f}b", "", too_long.as_str()] {
        let res = comments(&app, &tab, text).await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "{:?}",
            text.chars().take(8).collect::<String>()
        );
    }
    // A tab and a line break are text.
    let res = comments(&app, &tab, "a\tb\nc").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(common::typed(&log, 1).await, b"\x1b[200~a\tb\rc\x1b[201~");
    let res = comments(&app, "nope", "a").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// A Claude hook call carrying a recorded fixture.
fn hook_request(role: &str, tab: &str, name: &str) -> Request<Body> {
    let file = format!(
        "{}/tests/fixtures/claude/2.1.302/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut req = request("POST", "/v1/hook", None, true);
    *req.body_mut() = Body::from(std::fs::read(file).unwrap());
    let h = req.headers_mut();
    h.insert("x-marshell-role", role.parse().unwrap());
    h.insert("x-marshell-agent", "claude".parse().unwrap());
    h.insert("x-marshell-tab", tab.parse().unwrap());
    h.insert("x-marshell-version", "0.0.0".parse().unwrap());
    req
}

#[tokio::test]
async fn comments_wait_while_the_agent_asks() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, app, tab, log) = agent_session(tmp.path(), Some("claude"), "capture-input").await;
    let res = app
        .clone()
        .oneshot(hook_request("event", &tab, "SessionStart"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // The permission request holds its call open until it is answered.
    let held = tokio::spawn({
        let app = app.clone();
        let req = hook_request("permission", &tab, "PermissionRequest");
        async move { app.oneshot(req).await.unwrap().status() }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.holds().has_tab(&tab) {
        assert!(Instant::now() < deadline, "the permission request was never held");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let res = comments(&app, &tab, "a\nb").await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(reason_of(res).await, "waiting");
    let res = review(&app, &tab, serde_json::json!({ "base": null })).await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(reason_of(res).await, "waiting");
    // The card is still pending and nothing was written, to the pty or to disk.
    assert!(state.holds().has_tab(&tab));
    assert_eq!(common::typed(&log, 1).await, b"");
    assert!(!tmp.path().join("home/attachments").exists());
    let rid = state.holds().pending()[0].rid.clone();
    assert!(state.holds().release(&rid));
    assert_eq!(held.await.unwrap(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn review_and_comments_need_the_token() {
    let scratch = Scratch::new();
    let app = router(state_in(&scratch.home(), &scratch.root));
    for (path, body) in [
        ("/v1/sessions/nope/review", serde_json::json!({ "base": null })),
        ("/v1/sessions/nope/comments", serde_json::json!({ "text": "a" })),
    ] {
        let res = app
            .clone()
            .oneshot(request("POST", path, Some(body), false))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
}
