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
