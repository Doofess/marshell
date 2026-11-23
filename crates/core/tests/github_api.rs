//! The one place Marshell runs the GitHub CLI, and the route that answers the pull request bar, run against a fake:
//! no test reaches the network or runs the real `gh`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::github::gh;
use marshell_core::server::{AppState, router};
use marshell_protocol::api::ShellFamily;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::ServiceExt;

use common::{ORIGIN, PORT, TOKEN};

#[test]
fn gh_runs_in_marshells_folder_with_the_body_on_stdin() {
    common::tie_children_to_test();
    let home = tempfile::tempdir().unwrap();
    let gh_dir = home.path().join("gh");
    std::fs::create_dir_all(&gh_dir).unwrap();
    let script = json!([
        {"match": ["api", "--hostname", "github.com", "repos/octo-org/widgets/pulls"], "exit": 0,
         "stdout": "{\"number\":7}", "stderr": ""},
    ]);
    std::fs::write(gh_dir.join("script.json"), script.to_string()).unwrap();

    let fake = Path::new(env!("CARGO_BIN_EXE_gh-marshell-test"));
    let body = json!({"title": "Add the widget", "body": "a-body-that-must-not-be-an-argument"});
    let got = gh::api(
        fake,
        home.path(),
        "github.com",
        &["-X", "POST", "repos/octo-org/widgets/pulls"],
        Some(&body),
    )
    .unwrap();
    assert_eq!(got, json!({"number": 7}));

    let calls = std::fs::read_to_string(gh_dir.join("calls.jsonl")).expect("the fake ran in <home>/gh");
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines.len(), 1, "{calls}");
    let call: Value = serde_json::from_str(lines[0]).unwrap();
    let args: Vec<&str> = call["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(
        args,
        [
            "api",
            "--hostname",
            "github.com",
            "-X",
            "POST",
            "repos/octo-org/widgets/pulls",
            "--input",
            "-"
        ]
    );
    let stdin: Value = serde_json::from_str(call["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(stdin, body);
    // The environment names it got: the prompts are off, and nothing of Marshell's own is passed along.
    let names: Vec<&str> = call["env_names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert!(names.contains(&"GH_PROMPT_DISABLED"), "{names:?}");
    assert!(
        names
            .iter()
            .all(|n| !n.starts_with("GIT_") && !n.starts_with("MARSHELL")),
        "{names:?}"
    );
}

const OPEN_FAILING: &str = include_str!("fixtures/github/open-failing.json");
const MERGED: &str = include_str!("fixtures/github/merged.json");
const NO_PULL: &str = include_str!("fixtures/github/no-pull.json");
const NO_BRANCH: &str = include_str!("fixtures/github/no-branch.json");
const CROSS_REPO: &str = include_str!("fixtures/github/cross-repo.json");

/// The path as the core spells it: canonical, without the `\\?\` prefix.
fn canon(path: &Path) -> PathBuf {
    let real = std::fs::canonicalize(path).unwrap();
    let text = real.to_string_lossy().into_owned();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

/// A temp folder holding a Marshell `home/`, an empty git config and a repository `repo/` on `main` with one
/// commit, whose `origin` is `origin` (in the repository's own config only).
struct Scratch {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
}

impl Scratch {
    fn new(origin: &str) -> Scratch {
        let tmp = tempfile::tempdir().unwrap();
        let root = canon(tmp.path());
        std::fs::write(root.join("empty.gitconfig"), "").unwrap();
        std::fs::create_dir(root.join("home")).unwrap();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        let scratch = Scratch { _tmp: tmp, root, repo };
        scratch.git(&["init", "-q", "-b", "main"]);
        scratch.git(&["remote", "add", "origin", origin]);
        scratch.commit("first");
        scratch
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// Plain git in the repository that never reads the person's own config or inherited `GIT_*` variables.
    fn git(&self, args: &[&str]) -> String {
        self.git_input(args, "")
    }

    /// `git` with `input` on its standard input.
    fn git_input(&self, args: &[&str], input: &str) -> String {
        use std::io::Write;
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(&self.repo)
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
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().expect("git runs");
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input.as_bytes()).unwrap();
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// `count` empty commits `c001`, `c002`... on the branch `name`, which starts at `from` (one `git fast-import`).
    fn many_commits(&self, name: &str, from: &str, count: usize) {
        let mut script = String::new();
        // After the commit they follow: git walks by date, and a child older than its parent confuses it.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        for n in 1..=count {
            let subject = format!("c{n:03}");
            script.push_str(&format!(
                "commit refs/heads/{name}\ncommitter T <t@example.invalid> {} +0000\ndata {}\n{subject}\n",
                now + n as u64,
                subject.len()
            ));
            if n == 1 {
                script.push_str(&format!("from {from}\n"));
            }
            script.push('\n');
        }
        self.git_input(&["fast-import", "--quiet"], &script);
    }

    /// A commit that adds a file named after `message`; returns its id.
    fn commit(&self, message: &str) -> String {
        std::fs::write(self.repo.join(format!("{}.txt", message.replace(' ', "-"))), message).unwrap();
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn script(&self, entries: Value) {
        let dir = self.home().join("gh");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("script.json"), entries.to_string()).unwrap();
    }

    /// What the fake gh was called with, in order, each as its arguments joined by a space.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.home().join("gh").join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                let call: Value = serde_json::from_str(line).unwrap();
                call["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    /// How many times the fake was asked for the status of a branch.
    fn status_calls(&self) -> usize {
        self.calls().iter().filter(|c| c.contains("graphql")).count()
    }
}

fn state_in(scratch: &Scratch) -> AppState {
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let profiles = vec![common::family_profile(
        "sleeper",
        vec![fake.into(), "sleep".into()],
        ShellFamily::Posix,
    )];
    AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()], scratch.home(), profiles)
}

/// The state with the fake gh, which the route finds by its full path.
fn state_with_gh(scratch: &Scratch) -> AppState {
    state_in(scratch).with_gh(PathBuf::from(env!("CARGO_BIN_EXE_gh-marshell-test")))
}

fn request(method: &str, path: &str, body: Option<Value>, token: bool) -> Request<Body> {
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

async fn json_of(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// A session started in the scratch repository; returns the router and the tab.
async fn open(state: &AppState, scratch: &Scratch) -> (axum::Router, String) {
    let app = router(state.clone());
    let cwd = scratch.repo.to_string_lossy();
    let body = json!({"cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd, "agent": null});
    let res = app
        .clone()
        .oneshot(request("POST", "/v1/sessions", Some(body), true))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tab = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    (app, tab)
}

async fn github(app: &axum::Router, tab: &str, fresh: bool) -> Value {
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/v1/sessions/{tab}/github"),
            Some(json!({"fresh": fresh})),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

/// A fixture's answer, with `patch` applied to the repository part of it.
fn answer(fixture: &str, patch: impl FnOnce(&mut Value)) -> Value {
    let mut value: Value = serde_json::from_str(fixture).unwrap();
    patch(&mut value["data"]["repository"]);
    value
}

/// A fixture's answer where GitHub's branch and the head of every pull request are `oid`.
fn answer_at(fixture: &str, oid: &str) -> Value {
    answer(fixture, |repo| {
        if !repo["ref"].is_null() {
            repo["ref"]["target"]["oid"] = json!(oid);
        }
        for pull in repo["pullRequests"]["nodes"].as_array_mut().unwrap() {
            pull["headRefOid"] = json!(oid);
        }
    })
}

/// The script entry that answers a status query with `answer`.
fn status(answer: &Value) -> Value {
    json!({"match": ["graphql"], "exit": 0, "json": answer})
}

async fn changes(app: &axum::Router, tab: &str, stamp: Option<&str>) -> Value {
    let body = json!({"stamp": stamp, "hide_whitespace": false});
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/v1/sessions/{tab}/changes"),
            Some(body),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

/// The marks of the tree's root, as `path:status` texts, once a check has answered (the first reply has none).
async fn tree_marks(app: &axum::Router, tab: &str) -> Vec<String> {
    let started = Instant::now();
    loop {
        let body = json!({"dirs": [{"dir": "", "stamp": null}]});
        let res = app
            .clone()
            .oneshot(request("POST", &format!("/v1/sessions/{tab}/tree"), Some(body), true))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let reply = json_of(res).await;
        let mut marks: Vec<String> = reply["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| format!("{}:{}", m["path"].as_str().unwrap(), m["status"].as_str().unwrap()))
            .collect();
        marks.sort();
        if !marks.is_empty() || started.elapsed() > Duration::from_secs(30) {
            return marks;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// A repository on `feat/login` with two commits after `main`'s, an open pull request there whose base is
/// `develop`, and `origin/develop` fetched at the first of them when `fetched`. The script answers with it.
fn on_a_pull_requests_branch(scratch: &Scratch, fetched: bool) {
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let second = scratch.commit("second");
    let third = scratch.commit("third");
    if fetched {
        scratch.git(&["update-ref", "refs/remotes/origin/develop", &second]);
    }
    let reply = answer_at(OPEN_FAILING, &third);
    let reply = answer(&reply.to_string(), |repo| {
        repo["pullRequests"]["nodes"][0]["baseRefName"] = json!("develop");
    });
    scratch.script(json!([status(&reply)]));
}

#[tokio::test]
async fn the_tree_and_the_panel_share_the_pull_requests_base() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    on_a_pull_requests_branch(&scratch, true);
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    assert_eq!(github(&app, &tab, true).await["pull"]["number"], 7);

    let first = changes(&app, &tab, None).await;
    assert_eq!(first["base"], "origin/develop", "{first}");
    assert_eq!(first["ahead"], 1);
    assert_eq!(first["pr_base_unfetched"], Value::Null);
    // The same state, asked again with the stamp: one stamp.
    let stamp = first["stamp"].as_str().unwrap().to_string();
    let second = changes(&app, &tab, Some(&stamp)).await;
    assert_eq!(second["kind"], "same");
    assert_eq!(second["stamp"], stamp.as_str());
    // The tree marks what differs from that base: only the third commit's file.
    assert_eq!(tree_marks(&app, &tab).await, ["third.txt:added"]);
}

#[tokio::test]
async fn the_diff_moves_to_the_pull_requests_base_after_github_answers() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    on_a_pull_requests_branch(&scratch, true);
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;

    let before = changes(&app, &tab, None).await;
    assert_eq!(before["base"], "main", "{before}");
    assert_eq!(before["ahead"], 2);
    let old = before["stamp"].as_str().unwrap().to_string();

    assert_eq!(github(&app, &tab, true).await["pull"]["number"], 7);
    let after = changes(&app, &tab, Some(&old)).await;
    assert_eq!(after["kind"], "listed", "{after}");
    assert_ne!(after["stamp"], old.as_str());
    assert_eq!(after["base"], "origin/develop");
    assert_eq!(after["ahead"], 1);
    assert_eq!(after["pr_base_unfetched"], Value::Null);
}

#[tokio::test]
async fn an_unfetched_pull_request_base_is_told_in_the_reply() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    on_a_pull_requests_branch(&scratch, false);
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let before = changes(&app, &tab, None).await;
    assert_eq!(before["pr_base_unfetched"], Value::Null);
    let old = before["stamp"].as_str().unwrap().to_string();

    assert_eq!(github(&app, &tab, true).await["pull"]["number"], 7);
    let after = changes(&app, &tab, Some(&old)).await;
    assert_eq!(after["kind"], "listed", "the note moves the stamp: {after}");
    assert_eq!(after["base"], "main");
    assert_eq!(after["pr_base_unfetched"], "develop");
}

#[tokio::test]
async fn github_off_runs_no_gh() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.script(json!([status(&answer_at(NO_PULL, &"1".repeat(40)))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let res = app
        .clone()
        .oneshot(request("GET", "/v1/settings", None, true))
        .await
        .unwrap();
    let mut settings = json_of(res).await;
    settings["changes"]["pull_requests"] = json!(false);
    let res = app
        .clone()
        .oneshot(request("PUT", "/v1/settings", Some(settings), true))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "off");
    assert!(
        !scratch.home().join("gh").join("calls.jsonl").exists(),
        "gh was run: {:?}",
        scratch.calls()
    );
}

#[tokio::test]
async fn github_without_gh_says_so() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    // A PATH with no folder in it holds no gh.
    let state = state_in(&scratch).with_tab_path(|| Some(std::ffi::OsString::new()));
    let (app, tab) = open(&state, &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "no_gh", "{reply}");
    assert_eq!(reply["repo"], "octo-org/widgets");
    assert_eq!(reply["host"], "github.com");
}

#[tokio::test]
async fn github_signed_out_says_so() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.script(json!([{"match": ["graphql"], "exit": 4, "stdout": "",
        "stderr": "To get started with GitHub CLI, please run:  gh auth login\n"}]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "signed_out", "{reply}");
    assert_eq!(reply["pull"], Value::Null);
}

#[tokio::test]
async fn github_for_a_gitlab_origin_is_none() {
    let scratch = Scratch::new("https://gitlab.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.script(json!([
        {"match": ["auth", "status"], "exit": 0, "json": {"hosts": {"github.com": [{"state": "success"}]}}},
        status(&answer_at(NO_PULL, &"1".repeat(40))),
    ]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "none", "{reply}");
    assert_eq!(scratch.status_calls(), 0, "{:?}", scratch.calls());
}

#[tokio::test]
async fn github_on_the_default_branch_says_so() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.script(json!([status(&answer_at(NO_PULL, &"1".repeat(40)))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "default_branch", "{reply}");
    assert_eq!(reply["branch"], "main");
    assert_eq!(reply["default_branch"], "main");
    assert_eq!(reply["repo"], "octo-org/widgets");
    assert_eq!(reply["host"], "github.com");
    assert_eq!(reply["checkout"], scratch.repo.to_string_lossy().as_ref());
}

#[tokio::test]
async fn github_counts_commits_to_push_from_githubs_branch() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let on_github = scratch.commit("second");
    scratch.commit("third");
    scratch.commit("fourth");
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &on_github))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(reply["branch"], "feat/login");
    assert_eq!(reply["default_branch"], "main");
    assert_eq!(reply["on_github"], true);
    assert_eq!(reply["to_push"], 2);
    assert_eq!(reply["github_ahead"], false);
    assert_eq!(reply["pull"]["number"], 7, "the first open pull request: {reply}");
    assert_eq!(reply["closed"], Value::Null);
    assert_eq!(reply["push"], Value::Null);
    assert_eq!(reply["push_blocked"], Value::Null);
    assert_eq!(reply["compare_url"], Value::Null);
    assert_eq!(reply["can_write"], true);
}

#[tokio::test]
async fn github_is_ahead_when_its_commit_is_missing() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.commit("second");
    scratch.script(json!([status(&answer_at(NO_PULL, &"2".repeat(40)))]));
    let state = state_with_gh(&scratch);
    let (app, tab) = open(&state, &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(reply["on_github"], true);
    assert_eq!(reply["github_ahead"], true, "{reply}");
    assert_eq!(reply["to_push"], 0);

    // GitHub's commit is here, and ahead of the branch: a commit the branch doesn't hold.
    scratch.git(&["switch", "-q", "-c", "other", "main"]);
    let ahead = scratch.commit("only on github");
    scratch.git(&["switch", "-q", "feat/login"]);
    scratch.script(json!([status(&answer_at(NO_PULL, &ahead))]));
    state.github().mark_stale(&scratch.repo);
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["github_ahead"], true, "{reply}");
    assert_eq!(reply["to_push"], 1, "the branch's own commit is still to push: {reply}");
}

#[tokio::test]
async fn a_branch_that_is_not_on_github_counts_from_the_base() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    let base = scratch.git(&["rev-parse", "HEAD"]);
    scratch.git(&["update-ref", "refs/remotes/origin/main", &base]);
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.commit("second");
    scratch.commit("third");
    scratch.script(json!([status(&answer(NO_BRANCH, |_| {}))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(reply["on_github"], false);
    assert_eq!(reply["github_ahead"], false);
    assert_eq!(reply["to_push"], 2);
    assert_eq!(reply["pull"], Value::Null);
}

#[tokio::test]
async fn a_merged_pull_request_of_another_history_is_not_shown() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "other", "main"]);
    let elsewhere = scratch.commit("another history");
    scratch.git(&["switch", "-q", "-c", "feat/login", "main"]);
    let mine = scratch.commit("second");
    // A branch name reused: the merged pull request's head is a commit this branch doesn't hold.
    scratch.script(json!([status(&answer_at(MERGED, &elsewhere))]));
    let state = state_with_gh(&scratch);
    let (app, tab) = open(&state, &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(reply["pull"], Value::Null, "{reply}");
    assert_eq!(reply["closed"], Value::Null);

    // Its head is in this branch's history: shown as merged.
    scratch.script(json!([status(&answer_at(MERGED, &mine))]));
    state.github().mark_stale(&scratch.repo);
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["pull"]["number"], 2, "{reply}");
    assert_eq!(reply["pull"]["state"], "merged");
}

#[tokio::test]
async fn a_second_fresh_ask_within_3_s_uses_the_snapshot() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let head = scratch.commit("second");
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &head))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let first = github(&app, &tab, true).await;
    let second = github(&app, &tab, true).await;
    assert_eq!(first["kind"], "ready", "{first}");
    assert_eq!(first, second);
    assert_eq!(scratch.calls().len(), 1, "{:?}", scratch.calls());
}

#[tokio::test]
async fn an_unreachable_ask_drops_the_snapshot() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let head = scratch.commit("second");
    scratch.script(json!([
        {"match": ["graphql"], "call": 1, "exit": 0, "json": answer_at(OPEN_FAILING, &head)},
        {"match": ["graphql"], "exit": 1, "stdout": "", "stderr": "dial tcp: lookup api.github.com: no such host\n"},
    ]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let first = github(&app, &tab, true).await;
    assert_eq!(first["kind"], "ready", "{first}");

    // Past the 3 s a snapshot answers a fresh ask on its own.
    tokio::time::sleep(Duration::from_millis(3200)).await;
    let second = github(&app, &tab, true).await;
    assert_eq!(second["kind"], "unreachable", "{second}");
    assert_eq!(second["pull"], Value::Null);
    assert!(second["reason"].as_str().unwrap().contains("no such host"), "{second}");
    assert_eq!(scratch.status_calls(), 2);

    // Nothing stale is served now: a look that doesn't ask GitHub has no snapshot, so it asks.
    let third = github(&app, &tab, false).await;
    assert_eq!(third["kind"], "unreachable", "{third}");
    assert_eq!(scratch.status_calls(), 3);
}

#[tokio::test]
async fn a_not_fresh_ask_recounts_and_runs_no_gh() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let on_github = scratch.commit("second");
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &on_github))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let first = github(&app, &tab, true).await;
    assert_eq!(first["to_push"], 0, "{first}");

    scratch.commit("third");
    let second = github(&app, &tab, false).await;
    assert_eq!(second["kind"], "ready", "{second}");
    assert_eq!(second["to_push"], 1, "{second}");
    assert_eq!(second["pull"]["number"], 7);
    assert_eq!(scratch.calls().len(), 1, "{:?}", scratch.calls());
}

#[tokio::test]
async fn a_not_fresh_ask_with_no_snapshot_asks_github() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let head = scratch.commit("second");
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &head))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, false).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(scratch.status_calls(), 1);
}

#[tokio::test]
async fn an_answer_from_before_mark_stale_is_not_kept() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let head = scratch.commit("second");
    let numbered = |n: u32| {
        answer(OPEN_FAILING, |repo| {
            repo["ref"]["target"]["oid"] = json!(head);
            let nodes = repo["pullRequests"]["nodes"].as_array_mut().unwrap();
            nodes.truncate(1);
            nodes[0]["headRefOid"] = json!(head);
            nodes[0]["number"] = json!(n);
        })
    };
    scratch.script(json!([
        {"match": ["graphql"], "call": 1, "sleep_ms": 1500, "exit": 0, "json": numbered(7)},
        {"match": ["graphql"], "call": 2, "exit": 0, "json": numbered(8)},
    ]));
    let state = state_with_gh(&scratch);
    let (app, tab) = open(&state, &scratch).await;
    let asking = {
        let (app, tab) = (app.clone(), tab.clone());
        tokio::spawn(async move { github(&app, &tab, true).await })
    };
    // The first ask is in flight once the fake has been called; it sleeps before it answers.
    let started = Instant::now();
    while scratch.status_calls() == 0 {
        assert!(started.elapsed() < Duration::from_secs(20), "gh was never called");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    state.github().mark_stale(&scratch.repo);
    let reply = asking.await.unwrap();
    assert_eq!(reply["pull"]["number"], 8, "the old answer came back: {reply}");
    assert_eq!(scratch.status_calls(), 2, "{:?}", scratch.calls());

    // The second answer is the one kept.
    let again = github(&app, &tab, false).await;
    assert_eq!(again["pull"]["number"], 8);
    assert_eq!(scratch.status_calls(), 2);
}

#[tokio::test]
async fn a_pushurl_elsewhere_blocks_the_push_and_keeps_the_bar() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let head = scratch.commit("second");
    scratch.git(&[
        "config",
        "remote.origin.pushurl",
        "https://github.com/someone-else/widgets.git",
    ]);
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &head))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(reply["pull"]["number"], 7);
    assert_eq!(
        reply["push_blocked"],
        "origin pushes somewhere other than octo-org/widgets, so push from the terminal."
    );
}

#[tokio::test]
async fn a_fork_links_to_the_parents_default_branch_with_the_branch_encoded() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "fix/#12"]);
    let head = scratch.commit("second");
    scratch.script(json!([status(&answer_at(CROSS_REPO, &head))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["kind"], "ready", "{reply}");
    assert_eq!(reply["branch"], "fix/#12");
    assert_eq!(reply["fork_of"], "up-org/widgets");
    assert_eq!(
        reply["compare_url"],
        "https://github.com/up-org/widgets/compare/trunk...octo-org:fix/%2312?expand=1"
    );
    // The branch travelled to GitHub as a value of its own.
    assert!(
        scratch.calls().iter().any(|c| c.contains("b=refs/heads/fix/#12")),
        "{:?}",
        scratch.calls()
    );
}

#[tokio::test]
async fn github_route_needs_the_token() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/v1/sessions/{tab}/github"),
            Some(json!({"fresh": true})),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(!scratch.home().join("gh").join("calls.jsonl").exists());
}

async fn plan(app: &axum::Router, tab: &str, create: bool) -> Value {
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/v1/sessions/{tab}/github/plan"),
            Some(json!({"create": create})),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

fn subjects(plan: &Value) -> Vec<String> {
    plan["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["subject"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn plan_lists_commits_newest_first_and_counts_the_rest() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let on_github = scratch.commit("second");
    scratch.many_commits("feat/login", &on_github, 103);
    let head = scratch.git(&["rev-parse", "HEAD"]);
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &on_github))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(got["refusal"], Value::Null, "{got}");
    assert_eq!(got["head"], head.as_str());
    assert_eq!(got["to_push"], 103);
    let listed = subjects(&got);
    assert_eq!(listed.len(), 100, "{got}");
    assert_eq!(listed[0], "c103");
    assert_eq!(listed[99], "c004");
    assert_eq!(got["more"], 3);
    assert_eq!(got["commits"][0]["id"], head[..7]);
    assert_eq!(got["checkout"], scratch.repo.to_string_lossy().as_ref());
    assert_eq!(got["branch"], "feat/login");
    assert_eq!(got["on_github"], true);
    assert_eq!(got["moved"], false);
    // A plan asks GitHub only when nothing is kept.
    assert_eq!(scratch.status_calls(), 1);
    plan(&app, &tab, false).await;
    assert_eq!(scratch.status_calls(), 1);
}

#[tokio::test]
async fn plan_title_is_the_only_subject_else_the_branch() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    let base = scratch.git(&["rev-parse", "HEAD"]);
    scratch.git(&["update-ref", "refs/remotes/origin/main", &base]);
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.commit("Add the login form");
    scratch.script(json!([status(&answer(NO_BRANCH, |_| {}))]));
    let state = state_with_gh(&scratch);
    let (app, tab) = open(&state, &scratch).await;
    let got = plan(&app, &tab, true).await;
    assert_eq!(got["on_github"], false);
    assert_eq!(subjects(&got), ["Add the login form"]);
    assert_eq!(got["title"], "Add the login form");
    assert_eq!(got["base"], "main");

    scratch.commit("Check the password");
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    let got = plan(&app, &tab, true).await;
    assert_eq!(got["to_push"], 2);
    assert_eq!(got["title"], "feat/login");
}

#[tokio::test]
async fn plan_body_is_the_template_else_the_subjects() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    let base = scratch.git(&["rev-parse", "HEAD"]);
    scratch.git(&["update-ref", "refs/remotes/origin/main", &base]);
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.commit("Add the form");
    scratch.commit("Check the password");
    scratch.script(json!([status(&answer(NO_BRANCH, |_| {}))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, true).await;
    assert_eq!(got["body"], "- Add the form\n- Check the password");

    let dir = scratch.repo.join(".github");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("PULL_REQUEST_TEMPLATE.md"), "## What\r\n\r\n## Why\n").unwrap();
    let got = plan(&app, &tab, true).await;
    assert_eq!(got["body"], "## What\r\n\r\n## Why\n");
    assert_eq!(got["title"], "feat/login");
}

#[tokio::test]
async fn plan_counts_uncommitted_and_new_files() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let on_github = scratch.commit("second");
    scratch.script(json!([status(&answer_at(OPEN_FAILING, &on_github))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(got["uncommitted"], 0, "{got}");
    assert_eq!(got["to_push"], 0);
    assert_eq!(got["commits"], json!([]));

    std::fs::write(scratch.repo.join("first.txt"), "changed").unwrap();
    std::fs::write(scratch.repo.join("staged.txt"), "staged").unwrap();
    scratch.git(&["add", "staged.txt"]);
    std::fs::write(scratch.repo.join("new-one.txt"), "n1").unwrap();
    std::fs::write(scratch.repo.join("new-two.txt"), "n2").unwrap();
    // The look at the checkout is reused for a moment; wait it out.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(got["uncommitted"], 4, "{got}");
}

#[tokio::test]
async fn plan_refuses_a_detached_head_and_the_default_branch() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.script(json!([status(&answer(NO_PULL, |_| {}))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(
        got["refusal"], "main is the default branch on GitHub. Marshell doesn't push it.",
        "{got}"
    );
    assert_eq!(got["commits"], json!([]));
    assert_eq!(got["branch"], "main");

    let head = scratch.git(&["rev-parse", "HEAD"]);
    scratch.git(&["switch", "-q", "--detach", "HEAD"]);
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(
        got["refusal"], "This checkout is on no branch, so there is nothing to push.",
        "{got}"
    );
    assert_eq!(got["branch"], "");
    assert_eq!(got["head"], head.as_str());
    assert_eq!(got["checkout"], scratch.repo.to_string_lossy().as_ref());
}

#[tokio::test]
async fn plan_says_when_github_is_ahead() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    scratch.commit("second");
    scratch.script(json!([status(&answer_at(NO_PULL, &"2".repeat(40)))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(
        got["refusal"], "GitHub's feat/login has commits this checkout doesn't. Pull them in the terminal.",
        "{got}"
    );
    assert_eq!(got["commits"], json!([]));
}

#[tokio::test]
async fn plan_refuses_when_origin_pushes_elsewhere() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    scratch.git(&["switch", "-q", "-c", "feat/login"]);
    let head = scratch.commit("second");
    scratch.git(&[
        "config",
        "remote.origin.pushurl",
        "https://github.com/someone-else/widgets.git",
    ]);
    scratch.script(json!([status(&answer_at(NO_PULL, &head))]));
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(
        got["refusal"], "origin pushes somewhere other than octo-org/widgets, so push from the terminal.",
        "{got}"
    );
}

#[tokio::test]
async fn plan_for_a_create_refuses_a_branch_with_an_open_pull_request_and_bases_on_it() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    on_a_pull_requests_branch(&scratch, true);
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let got = plan(&app, &tab, false).await;
    assert_eq!(got["refusal"], Value::Null, "{got}");
    assert_eq!(got["base"], "develop");
    let got = plan(&app, &tab, true).await;
    assert_eq!(got["refusal"], "This branch already has pull request #7.", "{got}");
}

#[tokio::test]
async fn plan_route_needs_the_token() {
    let scratch = Scratch::new("https://github.com/octo-org/widgets.git");
    let (app, tab) = open(&state_with_gh(&scratch), &scratch).await;
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/v1/sessions/{tab}/github/plan"),
            Some(json!({"create": false})),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(!scratch.home().join("gh").join("calls.jsonl").exists());
}
