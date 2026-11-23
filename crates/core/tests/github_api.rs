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
        // Every address here is a GitHub one, so a mistake must not reach it: nothing connects (the proxy is the
        // discard port), nothing asks for credentials, and no ssh program runs.
        scratch.git(&["config", "http.proxy", "http://127.0.0.1:9"]);
        scratch.git(&["config", "credential.helper", ""]);
        scratch.git(&["config", "core.sshCommand", "false"]);
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
        self.git_input_at(&self.repo, args, input)
    }

    /// `git` in `dir`, as `git` is.
    fn git_at(&self, dir: &Path, args: &[&str]) -> String {
        self.git_input_at(dir, args, "")
    }

    fn git_input_at(&self, dir: &Path, args: &[&str], input: &str) -> String {
        use std::io::Write;
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
    let tab = open_in(&app, &scratch.repo).await;
    (app, tab)
}

/// A session started in `cwd` on a router; returns the tab.
async fn open_in(app: &axum::Router, cwd: &Path) -> String {
    let cwd = cwd.to_string_lossy();
    let body = json!({"cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd, "agent": null});
    let res = app
        .clone()
        .oneshot(request("POST", "/v1/sessions", Some(body), true))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
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
// ---- The push (Task 9) -----------------------------------------------------------------------------------------
//
// Every repository here has `https://github.com/octo-org/widgets.git` as its origin. The push reaches a bare
// repository in the temp folder instead, through settings the test hands the route (`with_push_config`): an
// `insteadOf` for that address and `protocol.file.allow`, followed by `protocol.https.allow=never` and
// `protocol.ssh.allow=never`, which come last on git's command line and so win. The repositories' own config
// points `http.proxy` at the discard port and has no credential helper or ssh program (see `Scratch::new`). No
// test reaches a network, and none touches the real `gh`, `~/.gitconfig` or any real repository.

const WIDGETS: &str = "https://github.com/octo-org/widgets.git";
const MOVED: &str = "The branch moved since you looked. Check the commits again.";
const RUNNING: &str = "A push of this checkout is already running.";
const ELSEWHERE: &str = "origin pushes somewhere other than octo-org/widgets, so push from the terminal.";

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A repository on `feat/login` (`main`'s commit, then `base`, then `head`), a bare repository that has `main`, and
/// GitHub's answer (by the fake gh) that its `feat/login` is at `base`.
struct Rig {
    scratch: Scratch,
    bare: PathBuf,
    first: String,
    base: String,
    head: String,
}

impl Rig {
    fn new() -> Rig {
        let scratch = Scratch::new(WIDGETS);
        let bare = scratch.root.join("bare.git");
        scratch.git_at(&scratch.root, &["init", "--bare", "-q", "-b", "main", &slash(&bare)]);
        let first = scratch.git(&["rev-parse", "HEAD"]);
        // A push to a path, not to `origin`.
        scratch.git(&["push", "-q", &slash(&bare), "main"]);
        scratch.git(&["switch", "-q", "-c", "feat/login"]);
        let base = scratch.commit("second");
        let head = scratch.commit("third");
        scratch.script(json!([status(&answer_at(OPEN_FAILING, &base))]));
        Rig {
            scratch,
            bare,
            first,
            base,
            head,
        }
    }

    /// What `with_push_config` gets: the push lands in the bare repository, and nothing else can be reached.
    fn config(&self) -> Vec<String> {
        vec![
            "protocol.file.allow=always".to_string(),
            format!("url.{}.insteadOf={WIDGETS}", slash(&self.bare)),
            "protocol.https.allow=never".to_string(),
            "protocol.ssh.allow=never".to_string(),
        ]
    }

    fn state(&self) -> AppState {
        state_with_gh(&self.scratch).with_push_config(self.config())
    }

    fn checkout(&self) -> String {
        self.scratch.repo.to_string_lossy().into_owned()
    }

    /// The commit the bare repository has at `branch`, or an empty text.
    fn bare_ref(&self, branch: &str) -> String {
        self.scratch.git_at(
            &self.bare,
            &[
                "for-each-ref",
                "--format=%(objectname)",
                &format!("refs/heads/{branch}"),
            ],
        )
    }

    /// A `pre-push` hook running `body` as a shell script.
    fn hook(&self, body: &str) {
        let path = self.scratch.repo.join(".git").join("hooks").join("pre-push");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    /// A hook that prints a line, waits until the returned file exists, and prints another.
    fn waiting_hook(&self) -> PathBuf {
        let go = self.scratch.root.join("go");
        self.hook(&format!(
            "echo \"hook started\"\nwhile [ ! -f \"{}\" ]; do sleep 0.2; done\necho \"hook done\"",
            slash(&go)
        ));
        go
    }
}

/// A call with the token: its status and its body (JSON when it is, else the text).
async fn call(app: &axum::Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(request(method, path, body, true)).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024).await.unwrap();
    let body =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

async fn push_at(app: &axum::Router, tab: &str, checkout: &str, branch: &str, head: &str) -> (StatusCode, Value) {
    let body = json!({"checkout": checkout, "branch": branch, "head": head});
    call(app, "POST", &format!("/v1/sessions/{tab}/push"), Some(body)).await
}

/// The confirmed push of the rig's branch; its id.
async fn start_push(rig: &Rig, app: &axum::Router, tab: &str) -> String {
    let (status, body) = push_at(app, tab, &rig.checkout(), "feat/login", &rig.head).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["id"].as_str().expect("an id").to_string()
}

/// Polls the push until `done` says so (a minute at most); its last answer.
async fn poll_push(app: &axum::Router, id: &str, mut done: impl FnMut(&Value) -> bool) -> Value {
    let started = Instant::now();
    loop {
        let (status, progress) = call(app, "GET", &format!("/v1/pushes/{id}?from=0"), None).await;
        assert_eq!(status, StatusCode::OK, "{progress}");
        if done(&progress) || started.elapsed() > Duration::from_secs(60) {
            return progress;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn finished(app: &axum::Router, id: &str) -> Value {
    poll_push(app, id, |p| p["state"] != "running").await
}

fn printed(progress: &Value) -> String {
    progress["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn push_runs_the_hook_and_streams_its_output() {
    let rig = Rig::new();
    let go = rig.waiting_hook();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    assert_eq!(github(&app, &tab, true).await["to_push"], 1);
    assert_eq!(rig.scratch.status_calls(), 1);
    let id = start_push(&rig, &app, &tab).await;
    assert_eq!(id.len(), 32);

    // While the hook waits, its first line is there already and the push runs.
    let running = poll_push(&app, &id, |p| printed(p).contains("hook started")).await;
    assert_eq!(running["state"], "running", "{running}");
    assert_eq!(rig.bare_ref("feat/login"), "", "nothing has moved yet");

    std::fs::write(&go, "").unwrap();
    let done = finished(&app, &id).await;
    assert_eq!(done["state"], "pushed", "{done}");
    let text = printed(&done);
    assert!(text.contains("hook done"), "{text}");
    assert!(text.contains("refs/heads/feat/login:refs/heads/feat/login"), "{text}");
    assert_eq!(rig.bare_ref("feat/login"), rig.head);
    // The pushed branch is tracked, which `-u` asks for.
    assert_eq!(rig.scratch.git(&["config", "branch.feat/login.remote"]), "origin");

    // The address git printed is the test's bare repository, not the bar's: that is told, and carried as the reason.
    let reason = done["reason"].as_str().expect("a reason");
    assert!(
        reason.starts_with("git pushed to ") && reason.ends_with(", which isn't octo-org/widgets."),
        "{reason}"
    );
    assert!(text.contains(reason), "the transcript says it too: {text}");

    // A reader that has seen `total` lines gets the last of them again.
    let total = done["total"].as_u64().unwrap();
    let (status, tail) = call(&app, "GET", &format!("/v1/pushes/{id}?from={total}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tail["start"], total - 1);
    assert_eq!(tail["lines"].as_array().unwrap().len(), 1);
    assert_eq!(tail["total"], total);
    // Beyond the end, and with no `from`: nothing breaks.
    let (status, beyond) = call(&app, "GET", &format!("/v1/pushes/{id}?from=999999"), None).await;
    assert_eq!((status, beyond["lines"].as_array().unwrap().len()), (StatusCode::OK, 0));
    let (status, all) = call(&app, "GET", &format!("/v1/pushes/{id}"), None).await;
    assert_eq!((status, all["start"].as_u64()), (StatusCode::OK, Some(0)));

    // A push that landed makes what GitHub said stale: the next look asks again, with no `fresh`.
    assert_eq!(github(&app, &tab, false).await["push"], Value::Null);
    assert_eq!(rig.scratch.status_calls(), 2);
}

/// Whether Windows lists a process with this id.
#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .any(|word| word == pid.to_string())
}

#[cfg(windows)]
#[tokio::test]
async fn push_cancel_ends_the_hook_and_moves_nothing() {
    /// Ends exactly the process with this id, if the test left it running.
    struct EndProcess(u32);
    impl Drop for EndProcess {
        fn drop(&mut self) {
            if process_is_running(self.0) {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &self.0.to_string(), "/F", "/T"])
                    .output();
            }
        }
    }

    let rig = Rig::new();
    // GitHub's branch (the bare repository's) is at `base`; the push would move it to `head`.
    rig.scratch.git(&[
        "push",
        "-q",
        &slash(&rig.bare),
        &format!("{}:refs/heads/feat/login", rig.base),
    ]);
    let pid_file = rig.scratch.root.join("hook.pid");
    // Git for Windows' shell: `$$` is its own number, and `winpid` is the one Windows has for it.
    rig.hook(&format!("cat /proc/$$/winpid > \"{}\"\nsleep 60", slash(&pid_file)));
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let id = start_push(&rig, &app, &tab).await;

    let started = Instant::now();
    let pid = loop {
        if let Ok(text) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = text.trim().parse::<u32>()
        {
            break pid;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "the hook never started");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let _end = EndProcess(pid);
    assert!(process_is_running(pid), "the hook's shell is not running");

    let (status, _) = call(&app, "POST", &format!("/v1/pushes/{id}/cancel"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let done = finished(&app, &id).await;
    assert_eq!(done["state"], "cancelled", "{done}");
    assert_eq!(done["reason"], Value::Null);

    let waited = Instant::now();
    while process_is_running(pid) {
        assert!(
            waited.elapsed() < Duration::from_secs(5),
            "the hook outlived the cancelled push"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(rig.bare_ref("feat/login"), rig.base, "the branch did not move");
    // Cancelling an ended push changes nothing.
    let (status, _) = call(&app, "POST", &format!("/v1/pushes/{id}/cancel"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(github(&app, &tab, false).await["push"], Value::Null);
}

#[tokio::test]
async fn a_rejected_push_ends_failed_with_gits_output() {
    let rig = Rig::new();
    // GitHub's branch holds a commit this one lacks.
    rig.scratch.git(&["switch", "-q", "-c", "side", "main"]);
    let side = rig.scratch.commit("side");
    rig.scratch.git(&[
        "push",
        "-q",
        &slash(&rig.bare),
        &format!("{side}:refs/heads/feat/login"),
    ]);
    rig.scratch.git(&["switch", "-q", "feat/login"]);
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let id = start_push(&rig, &app, &tab).await;
    let done = finished(&app, &id).await;
    assert_eq!(done["state"], "failed", "{done}");
    assert_eq!(done["reason"], "git ended with code 1.");
    let text = printed(&done);
    assert!(text.contains("rejected"), "{text}");
    assert_eq!(rig.bare_ref("feat/login"), side, "nothing was forced");
}

#[tokio::test]
async fn push_refuses_a_moved_head() {
    let rig = Rig::new();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    // A head, and a branch, that aren't the checkout's.
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/login", &rig.base).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(MOVED)));
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/login", &"0".repeat(40)).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(MOVED)));
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/other", &rig.head).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(MOVED)));
    // A commit made after the user looked.
    rig.scratch.commit("fourth");
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/login", &rig.head).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(MOVED)));
    assert_eq!(rig.bare_ref("feat/login"), "");
}

#[tokio::test]
async fn push_refuses_a_second_push() {
    let rig = Rig::new();
    let go = rig.waiting_hook();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    // Two clicks at once: one starts, the other is refused.
    let (checkout, head) = (rig.checkout(), rig.head.clone());
    let (a, b) = tokio::join!(
        push_at(&app, &tab, &checkout, "feat/login", &head),
        push_at(&app, &tab, &checkout, "feat/login", &head),
    );
    let mut got = [a, b];
    got.sort_by_key(|(status, _)| status.as_u16());
    assert_eq!(got[0].0, StatusCode::OK, "{got:?}");
    assert_eq!((got[1].0, &got[1].1["reason"]), (StatusCode::CONFLICT, &json!(RUNNING)));
    let id = got[0].1["id"].as_str().unwrap().to_string();
    // And a third, later.
    let (status, body) = push_at(&app, &tab, &checkout, "feat/login", &head).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(RUNNING)));

    std::fs::write(&go, "").unwrap();
    assert_eq!(finished(&app, &id).await["state"], "pushed");
    // Once it ended, the checkout may push again.
    let (status, again) = push_at(&app, &tab, &checkout, "feat/login", &head).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_ne!(again["id"], json!(id));
    assert_eq!(finished(&app, again["id"].as_str().unwrap()).await["state"], "pushed");
}

#[tokio::test]
async fn push_refuses_a_branch_that_starts_with_a_dash_or_a_plus() {
    for name in ["+main", "-x"] {
        let rig = Rig::new();
        let main_before = rig.bare_ref("main");
        rig.scratch.git(&["update-ref", &format!("refs/heads/{name}"), "HEAD"]);
        rig.scratch
            .git(&["symbolic-ref", "HEAD", &format!("refs/heads/{name}")]);
        let (app, tab) = open(&rig.state(), &rig.scratch).await;
        let (status, body) = push_at(&app, &tab, &rig.checkout(), name, &rig.head).await;
        assert_eq!(status, StatusCode::CONFLICT, "{name}: {body}");
        assert_eq!(body["reason"], "That isn't a branch name Marshell can push.");
        assert_eq!(rig.bare_ref("main"), main_before, "{name}");
        assert_eq!(rig.bare_ref(name), "", "{name}");
        assert_eq!(rig.bare_ref("feat/login"), "", "{name}");
    }
}

#[tokio::test]
async fn push_refuses_the_default_branch() {
    let rig = Rig::new();
    rig.scratch.git(&["switch", "-q", "main"]);
    rig.scratch.script(json!([status(&answer_at(NO_PULL, &rig.first))]));
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "main", &rig.first).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["reason"],
        "main is the default branch on GitHub. Marshell doesn't push it."
    );
    assert_eq!(rig.bare_ref("main"), rig.first);
}

#[tokio::test]
async fn push_refuses_an_origin_that_pushes_elsewhere() {
    // A push address of another repository, and a local path.
    for local in [false, true] {
        let rig = Rig::new();
        let pushurl = if local {
            slash(&rig.bare)
        } else {
            "https://github.com/someone-else/widgets.git".to_string()
        };
        rig.scratch.git(&["config", "remote.origin.pushurl", &pushurl]);
        let (app, tab) = open(&rig.state(), &rig.scratch).await;
        let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/login", &rig.head).await;
        assert_eq!(status, StatusCode::CONFLICT, "{pushurl}: {body}");
        assert_eq!(body["reason"], ELSEWHERE, "{pushurl}");
        assert_eq!(rig.bare_ref("feat/login"), "", "{pushurl}");
    }
}

#[tokio::test]
async fn push_refuses_another_checkout_than_the_confirmed_one() {
    let rig = Rig::new();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let other = rig.scratch.root.join("elsewhere");
    let (status, body) = push_at(&app, &tab, &other.to_string_lossy(), "feat/login", &rig.head).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["reason"],
        "The session moved to another folder. Look again before you go on."
    );
    let (status, body) = push_at(&app, &tab, "", "feat/login", &rig.head).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(rig.bare_ref("feat/login"), "");
}

/// The origin changed under an open sheet: the sentence is about GitHub, not about a session that moved.
#[tokio::test]
async fn push_for_an_origin_that_is_no_longer_github_says_it_could_not_check() {
    let rig = Rig::new();
    rig.scratch
        .git(&["config", "remote.origin.url", "https://gitlab.com/octo-org/widgets.git"]);
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/login", &rig.head).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["reason"],
        "Marshell couldn't check this branch with GitHub, so push from the terminal."
    );
    assert_eq!(rig.bare_ref("feat/login"), "");
}

#[tokio::test]
async fn push_refuses_when_pull_requests_are_switched_off() {
    let rig = Rig::new();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let (_, mut settings) = call(&app, "GET", "/v1/settings", None).await;
    settings["changes"]["pull_requests"] = json!(false);
    let (status, _) = call(&app, "PUT", "/v1/settings", Some(settings)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = push_at(&app, &tab, &rig.checkout(), "feat/login", &rig.head).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["reason"],
        "Pull requests from GitHub are switched off in Settings."
    );
    assert_eq!(rig.bare_ref("feat/login"), "");
}

/// Without the test's settings the route sends git its own: a repository's included file that turns the address into
/// a local path can't make git use the `file` transport.
#[tokio::test]
async fn a_file_address_is_refused_by_git_without_the_test_setting() {
    let rig = Rig::new();
    let include = rig.scratch.root.join("included.gitconfig");
    std::fs::write(
        &include,
        format!("[url \"{}\"]\n\tinsteadOf = {WIDGETS}\n", slash(&rig.bare)),
    )
    .unwrap();
    rig.scratch.git(&["config", "include.path", &slash(&include)]);
    // The state has no `with_push_config`.
    let (app, tab) = open(&state_with_gh(&rig.scratch), &rig.scratch).await;
    let id = start_push(&rig, &app, &tab).await;
    let done = finished(&app, &id).await;
    assert_eq!(done["state"], "failed", "{done}");
    assert!(
        done["reason"].as_str().unwrap().starts_with("git ended with code "),
        "{done}"
    );
    assert_eq!(rig.bare_ref("feat/login"), "");
}

#[tokio::test]
async fn a_running_push_is_named_in_the_status() {
    let rig = Rig::new();
    let go = rig.waiting_hook();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    assert_eq!(github(&app, &tab, true).await["push"], Value::Null);
    let id = start_push(&rig, &app, &tab).await;
    assert_eq!(github(&app, &tab, false).await["push"], json!(id));
    assert_eq!(github(&app, &tab, true).await["push"], json!(id));
    std::fs::write(&go, "").unwrap();
    assert_eq!(finished(&app, &id).await["state"], "pushed");
    assert_eq!(github(&app, &tab, false).await["push"], Value::Null);
    // An ended push stays readable.
    let (status, again) = call(&app, "GET", &format!("/v1/pushes/{id}?from=0"), None).await;
    assert_eq!((status, &again["state"]), (StatusCode::OK, &json!("pushed")));
}

#[tokio::test]
async fn stop_ends_every_running_push() {
    let rig = Rig::new();
    let _go = rig.waiting_hook();
    // A second checkout of the same repository, on a branch of its own.
    let second = rig.scratch.root.join("second-checkout");
    rig.scratch
        .git(&["worktree", "add", "-q", "-b", "feat/two", &slash(&second), "feat/login"]);
    let app = router(rig.state());
    let (one, two) = (open_in(&app, &rig.scratch.repo).await, open_in(&app, &second).await);
    let first_id = start_push(&rig, &app, &one).await;
    let (status, body) = push_at(&app, &two, &second.to_string_lossy(), "feat/two", &rig.head).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second_id = body["id"].as_str().unwrap().to_string();
    for id in [&first_id, &second_id] {
        let running = poll_push(&app, id, |p| printed(p).contains("hook started")).await;
        assert_eq!(running["state"], "running", "{running}");
    }

    let started = Instant::now();
    let (status, _) = call(&app, "POST", "/v1/pushes/stop", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(started.elapsed() < Duration::from_secs(4), "{:?}", started.elapsed());
    for id in [&first_id, &second_id] {
        let (_, progress) = call(&app, "GET", &format!("/v1/pushes/{id}?from=0"), None).await;
        assert_eq!(progress["state"], "cancelled", "stop waits for the end: {progress}");
    }
    assert_eq!(rig.bare_ref("feat/login"), "");
    assert_eq!(rig.bare_ref("feat/two"), "");
    // With nothing running, stop answers at once.
    let (status, _) = call(&app, "POST", "/v1/pushes/stop", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn push_of_an_unknown_id_is_404() {
    let rig = Rig::new();
    let (app, _tab) = open(&rig.state(), &rig.scratch).await;
    let unknown = "0123456789abcdef0123456789abcdef";
    for id in [unknown, "not-an-id", "..%2F..%2Fx", &unknown.to_uppercase()] {
        let (status, _) = call(&app, "GET", &format!("/v1/pushes/{id}?from=0"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
        let (status, _) = call(&app, "POST", &format!("/v1/pushes/{id}/cancel"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
    }
    let body = json!({"checkout": rig.checkout(), "branch": "feat/login", "head": rig.head});
    let (status, _) = call(&app, "POST", "/v1/sessions/no-such-tab/push", Some(body)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn push_routes_need_the_token() {
    let rig = Rig::new();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let id = "0123456789abcdef0123456789abcdef";
    let body = json!({"checkout": rig.checkout(), "branch": "feat/login", "head": rig.head});
    for (method, path, body) in [
        ("POST", format!("/v1/sessions/{tab}/push"), Some(body)),
        ("GET", format!("/v1/pushes/{id}?from=0"), None),
        ("POST", format!("/v1/pushes/{id}/cancel"), None),
        ("POST", "/v1/pushes/stop".to_string(), None),
    ] {
        let res = app.clone().oneshot(request(method, &path, body, false)).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    assert_eq!(rig.bare_ref("feat/login"), "");
}
// ---- Create, merge, auto-merge, ready and draft (Task 10) -------------------------------------------------------
//
// These act on a GitHub account, so each call is looked at as the fake gh recorded it: its exact arguments, and the
// JSON that went in on standard input. The status calls (no `--input`) are not "writes".

const PULL_ID: &str = "PR_kwDOAAAAAA";
const NOT_THIS_PULL: &str = "That isn't this branch's pull request any more. Look again.";
const MOVED_FOLDER: &str = "The session moved to another folder. Look again before you go on.";
const NEED_CLI: &str = "Pull requests need the GitHub CLI.";
const NEED_SIGN_IN: &str = "Pull requests need a GitHub sign-in. Run `gh auth login` in a terminal.";

impl Scratch {
    /// What the fake gh was called with that sent a body: `(arguments, the JSON on standard input)`.
    fn writes(&self) -> Vec<(Vec<String>, Value)> {
        std::fs::read_to_string(self.home().join("gh").join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|call| {
                call["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a.as_str() == Some("--input"))
            })
            .map(|call| {
                let args = call["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a.as_str().unwrap().to_string())
                    .collect();
                let stdin = serde_json::from_str(call["stdin"].as_str().unwrap()).unwrap_or(Value::Null);
                (args, stdin)
            })
            .collect()
    }
}

/// The rig's script: `mine` first (they match `--input`, which a status call never has), then the status answer.
fn script_with(rig: &Rig, mine: Value) {
    let mut entries = mine.as_array().unwrap().clone();
    entries.push(status(&answer_at(OPEN_FAILING, &rig.base)));
    rig.scratch.script(Value::Array(entries));
}

/// A router on the rig whose bar has shown pull request #7 (`PULL_ID`) for `feat/login`: the head it showed is
/// `rig.base`.
async fn seen(rig: &Rig) -> (axum::Router, String) {
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let reply = github(&app, &tab, true).await;
    assert_eq!(reply["pull"]["number"], 7, "{reply}");
    assert_eq!(reply["pull"]["id"], PULL_ID, "{reply}");
    (app, tab)
}

fn create_body(rig: &Rig) -> Value {
    json!({
        "checkout": rig.checkout(), "title": "Add the login form", "body": "Adds the form.\r\n\r\n\tCloses #3\n",
        "base": "main", "draft": true, "branch": "feat/login", "head": rig.head,
    })
}

async fn create(app: &axum::Router, tab: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", &format!("/v1/sessions/{tab}/github/create"), Some(body)).await
}

fn merge_body(rig: &Rig, method: &str, auto: bool) -> Value {
    json!({"checkout": rig.checkout(), "number": 7, "method": method, "head": rig.base, "id": PULL_ID, "auto": auto})
}

async fn merge(app: &axum::Router, tab: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", &format!("/v1/sessions/{tab}/github/merge"), Some(body)).await
}

fn pull_body(rig: &Rig, action: &str) -> Value {
    json!({"checkout": rig.checkout(), "id": PULL_ID, "number": 7, "action": action})
}

async fn pull_action(app: &axum::Router, tab: &str, body: Value) -> (StatusCode, Value) {
    call(app, "POST", &format!("/v1/sessions/{tab}/github/pull"), Some(body)).await
}

fn strs(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

const CREATED: &str = r#"{"number":12,"html_url":"https://github.com/octo-org/widgets/pull/12","state":"open"}"#;

#[tokio::test]
async fn create_sends_title_head_base_body_draft_on_stdin() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["repos/octo-org/widgets/pulls", "--input"], "exit": 0, "stdout": CREATED}]),
    );
    let (app, tab) = seen(&rig).await;
    let status_calls = rig.scratch.status_calls();
    let (status, body) = create(&app, &tab, create_body(&rig)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"number": 12, "url": "https://github.com/octo-org/widgets/pull/12"})
    );
    let writes = rig.scratch.writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(
        writes[0].0,
        strs(&[
            "api",
            "--hostname",
            "github.com",
            "repos/octo-org/widgets/pulls",
            "--input",
            "-"
        ])
    );
    assert_eq!(
        writes[0].1,
        json!({
            "title": "Add the login form", "head": "feat/login", "base": "main",
            "body": "Adds the form.\r\n\r\n\tCloses #3\n", "draft": true,
        })
    );
    // What the bar kept is no longer true: the next look asks again.
    github(&app, &tab, false).await;
    assert_eq!(rig.scratch.status_calls(), status_calls + 1);
}

#[tokio::test]
async fn create_shows_githubs_422_message() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["repos/octo-org/widgets/pulls", "--input"], "exit": 1,
            "stdout": r#"{"message":"Validation Failed","errors":[{"resource":"PullRequest","code":"custom","message":"A pull request already exists for octo-org:feat/login."}],"documentation_url":"https://docs.github.com/rest/pulls/pulls#create-a-pull-request","status":"422"}"#,
            "stderr": "gh: Validation Failed (HTTP 422)\n"}]),
    );
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let (status, body) = create(&app, &tab, create_body(&rig)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["reason"], "A pull request already exists for octo-org:feat/login.");
}

#[tokio::test]
async fn create_refuses_a_control_character_in_the_title() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["pulls", "--input"], "exit": 0, "stdout": CREATED}]),
    );
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let too_long = "x".repeat(257);
    let titles = [
        "Add\u{7} login",
        "Add\nlogin",
        "Add\u{202e}login",
        "",
        too_long.as_str(),
    ];
    for title in titles {
        let mut body = create_body(&rig);
        body["title"] = json!(title);
        let (status, answer) = create(&app, &tab, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{title:?}: {answer}");
    }
    // The other inputs are looked at too.
    for (field, value) in [
        ("body", json!("a\u{1b}[31mred")),
        ("body", json!("x".repeat(65_537))),
        ("base", json!("-x")),
        ("base", json!("a..b")),
        ("branch", json!("+feat")),
        ("head", json!("main")),
        ("head", json!("0".repeat(39))),
    ] {
        let mut body = create_body(&rig);
        body[field] = value.clone();
        let (status, answer) = create(&app, &tab, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}={value}: {answer}");
    }
    assert!(rig.scratch.writes().is_empty());
    // A title of exactly 256 characters passes.
    let mut body = create_body(&rig);
    body["title"] = json!("x".repeat(256));
    let (status, answer) = create(&app, &tab, body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(rig.scratch.writes().len(), 1);
}

#[tokio::test]
async fn create_refuses_a_branch_or_head_that_is_not_the_checkouts() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["pulls", "--input"], "exit": 0, "stdout": CREATED}]),
    );
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    for (field, value) in [("branch", json!("feat/other")), ("head", json!(rig.base))] {
        let mut body = create_body(&rig);
        body[field] = value;
        let (status, answer) = create(&app, &tab, body).await;
        assert_eq!(status, StatusCode::CONFLICT, "{answer}");
        assert_eq!(answer["reason"], MOVED);
    }
    assert!(rig.scratch.writes().is_empty());
}

#[tokio::test]
async fn create_keeps_only_a_usable_https_url_and_needs_a_number() {
    let rig = Rig::new();
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    for (answer, want) in [
        (
            r#"{"number":5,"html_url":"javascript:alert(1)"}"#,
            json!({"number": 5, "url": ""}),
        ),
        (
            r#"{"number":5,"html_url":"https://x.example/a b"}"#,
            json!({"number": 5, "url": ""}),
        ),
        (r#"{"number":5}"#, json!({"number": 5, "url": ""})),
    ] {
        script_with(
            &rig,
            json!([{"match": ["pulls", "--input"], "exit": 0, "stdout": answer}]),
        );
        let (status, body) = create(&app, &tab, create_body(&rig)).await;
        assert_eq!((status, body), (StatusCode::OK, want), "{answer}");
    }
    for answer in [
        r#"{"html_url":"https://github.com/octo-org/widgets/pull/1"}"#,
        r#"{"number":0}"#,
        r#"{"number":"7"}"#,
    ] {
        script_with(
            &rig,
            json!([{"match": ["pulls", "--input"], "exit": 0, "stdout": answer}]),
        );
        let (status, body) = create(&app, &tab, create_body(&rig)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{answer}: {body}");
        assert_eq!(body["reason"], "GitHub's answer couldn't be read.");
    }
}

#[tokio::test]
async fn merge_sends_the_head_the_bar_showed() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["-X", "PUT", "pulls/7/merge", "--input"], "exit": 0,
            "stdout": r#"{"sha":"abc","merged":true,"message":"Pull Request successfully merged"}"#}]),
    );
    let (app, tab) = seen(&rig).await;
    let status_calls = rig.scratch.status_calls();
    let (status, body) = merge(&app, &tab, merge_body(&rig, "squash", false)).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let writes = rig.scratch.writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(
        writes[0].0,
        strs(&[
            "api",
            "--hostname",
            "github.com",
            "-X",
            "PUT",
            "repos/octo-org/widgets/pulls/7/merge",
            "--input",
            "-"
        ])
    );
    assert_eq!(writes[0].1, json!({"merge_method": "squash", "sha": rig.base}));
    github(&app, &tab, false).await;
    assert_eq!(rig.scratch.status_calls(), status_calls + 1);
    for method in ["merge", "rebase"] {
        let (status, _) = merge(&app, &tab, merge_body(&rig, method, false)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(rig.scratch.writes().last().unwrap().1["merge_method"], method);
    }
}

#[tokio::test]
async fn merge_409_says_the_branch_moved() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["pulls/7/merge", "--input"], "exit": 1,
            "stdout": r#"{"message":"Head branch was modified. Review and try the merge again.","documentation_url":"https://docs.github.com/rest/pulls/pulls#merge-a-pull-request","status":"409"}"#,
            "stderr": "gh: Head branch was modified. Review and try the merge again. (HTTP 409)\n"}]),
    );
    let (app, tab) = seen(&rig).await;
    let (status, body) = merge(&app, &tab, merge_body(&rig, "merge", false)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["reason"],
        "Head branch was modified. Review and try the merge again."
    );
}

#[tokio::test]
async fn merge_405_says_conflicts() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["pulls/7/merge", "--input"], "exit": 1,
            "stdout": r#"{"message":"Pull Request has merge conflicts","documentation_url":"https://docs.github.com/rest/pulls/pulls#merge-a-pull-request","status":"405"}"#,
            "stderr": "gh: Pull Request has merge conflicts (HTTP 405)\n"}]),
    );
    let (app, tab) = seen(&rig).await;
    let (status, body) = merge(&app, &tab, merge_body(&rig, "merge", false)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["reason"], "Pull Request has merge conflicts");
}

const GRAPHQL_OK: &str = r#"{"data":{"x":{"clientMutationId":null}}}"#;

#[tokio::test]
async fn auto_merge_sends_the_expected_head() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["graphql", "--input"], "exit": 0, "stdout": GRAPHQL_OK}]),
    );
    let (app, tab) = seen(&rig).await;
    let (status, body) = merge(&app, &tab, merge_body(&rig, "rebase", true)).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let writes = rig.scratch.writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(
        writes[0].0,
        strs(&["api", "--hostname", "github.com", "graphql", "--input", "-"])
    );
    let query = writes[0].1["query"].as_str().expect("a query");
    assert!(query.starts_with("mutation("), "{query}");
    assert!(query.contains("enablePullRequestAutoMerge"), "{query}");
    // The values travel as variables, never in the text.
    assert!(
        !query.contains(PULL_ID) && !query.contains(&rig.base) && !query.contains("REBASE"),
        "{query}"
    );
    assert_eq!(
        writes[0].1["variables"],
        json!({"id": PULL_ID, "method": "REBASE", "oid": rig.base})
    );
    assert_eq!(writes[0].1.as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn a_graphql_answer_with_errors_is_a_refusal() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["graphql", "--input"], "exit": 0,
            "stdout": r#"{"data":{"enablePullRequestAutoMerge":null},"errors":[{"type":"UNPROCESSABLE","path":["enablePullRequestAutoMerge"],"message":"Pull request Auto merge is not allowed for this repository"}]}"#}]),
    );
    let (app, tab) = seen(&rig).await;
    let status_calls = rig.scratch.calls().iter().filter(|c| !c.contains("--input")).count();
    let (status, body) = merge(&app, &tab, merge_body(&rig, "merge", true)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["reason"],
        "Pull request Auto merge is not allowed for this repository"
    );
    // A refusal changed nothing: what the bar kept still stands.
    github(&app, &tab, false).await;
    assert_eq!(
        rig.scratch.calls().iter().filter(|c| !c.contains("--input")).count(),
        status_calls
    );
}

#[tokio::test]
async fn ready_and_draft_use_the_node_id() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["graphql", "--input"], "exit": 0, "stdout": GRAPHQL_OK}]),
    );
    let (app, tab) = seen(&rig).await;
    for (action, mutation) in [
        ("ready", "markPullRequestReadyForReview"),
        ("draft", "convertPullRequestToDraft"),
        ("cancel_auto_merge", "disablePullRequestAutoMerge"),
    ] {
        let (status, body) = pull_action(&app, &tab, pull_body(&rig, action)).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{action}: {body}");
        let writes = rig.scratch.writes();
        let (args, sent) = writes.last().unwrap();
        assert_eq!(
            *args,
            strs(&["api", "--hostname", "github.com", "graphql", "--input", "-"])
        );
        let query = sent["query"].as_str().unwrap();
        assert!(
            query.starts_with("mutation(") && query.contains(mutation),
            "{action}: {query}"
        );
        assert!(!query.contains(PULL_ID), "{query}");
        assert_eq!(sent["variables"], json!({"id": PULL_ID}), "{action}");
    }
    assert_eq!(rig.scratch.writes().len(), 3);
}

#[tokio::test]
async fn a_node_id_with_a_quote_is_400() {
    let rig = Rig::new();
    script_with(
        &rig,
        json!([{"match": ["graphql", "--input"], "exit": 0, "stdout": GRAPHQL_OK}]),
    );
    let (app, tab) = seen(&rig).await;
    let long = "A".repeat(101);
    for id in [
        "PR_x\"y",
        "PR_x') { id }",
        "PR x",
        "",
        "PR_\u{e9}",
        "PR_x\n",
        long.as_str(),
    ] {
        let mut body = merge_body(&rig, "merge", true);
        body["id"] = json!(id);
        let (status, answer) = merge(&app, &tab, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{id:?}: {answer}");
        let mut body = pull_body(&rig, "ready");
        body["id"] = json!(id);
        let (status, answer) = pull_action(&app, &tab, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{id:?}: {answer}");
    }
    // The other inputs: a head that isn't a commit id, a number below 1.
    let mut body = merge_body(&rig, "merge", true);
    body["head"] = json!("main");
    assert_eq!(merge(&app, &tab, body).await.0, StatusCode::BAD_REQUEST);
    let mut body = merge_body(&rig, "merge", false);
    body["number"] = json!(0);
    assert_eq!(merge(&app, &tab, body).await.0, StatusCode::BAD_REQUEST);
    let mut body = pull_body(&rig, "ready");
    body["number"] = json!(0);
    assert_eq!(pull_action(&app, &tab, body).await.0, StatusCode::BAD_REQUEST);
    assert!(rig.scratch.writes().is_empty());
}

#[tokio::test]
async fn a_node_id_the_core_did_not_report_is_refused() {
    let rig = Rig::new();
    script_with(&rig, json!([{"match": ["--input"], "exit": 0, "stdout": GRAPHQL_OK}]));
    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    // Nothing has been seen yet.
    let (status, body) = pull_action(&app, &tab, pull_body(&rig, "ready")).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(NOT_THIS_PULL)));
    github(&app, &tab, true).await;
    // A well-formed id of another pull request; and the right id with another number.
    let mut other = merge_body(&rig, "merge", true);
    other["id"] = json!("PR_kwDOZZZZZZ");
    let mut renumbered = merge_body(&rig, "merge", false);
    renumbered["number"] = json!(8);
    for body in [other, renumbered] {
        let (status, answer) = merge(&app, &tab, body).await;
        assert_eq!(
            (status, &answer["reason"]),
            (StatusCode::CONFLICT, &json!(NOT_THIS_PULL))
        );
    }
    let mut other = pull_body(&rig, "draft");
    other["id"] = json!("PR_kwDOZZZZZZ");
    let (status, answer) = pull_action(&app, &tab, other).await;
    assert_eq!(
        (status, &answer["reason"]),
        (StatusCode::CONFLICT, &json!(NOT_THIS_PULL))
    );
    assert!(rig.scratch.writes().is_empty(), "gh was run: {:?}", rig.scratch.calls());
}

#[tokio::test]
async fn actions_refuse_another_checkout_than_the_confirmed_one() {
    let rig = Rig::new();
    script_with(&rig, json!([{"match": ["--input"], "exit": 0, "stdout": CREATED}]));
    let (app, tab) = seen(&rig).await;
    let other = rig.scratch.root.join("elsewhere").to_string_lossy().into_owned();
    for checkout in [other.as_str(), ""] {
        let mut body = create_body(&rig);
        body["checkout"] = json!(checkout);
        let (status, answer) = create(&app, &tab, body).await;
        assert_eq!(
            (status, &answer["reason"]),
            (StatusCode::CONFLICT, &json!(MOVED_FOLDER)),
            "{answer}"
        );
        let mut body = merge_body(&rig, "merge", false);
        body["checkout"] = json!(checkout);
        let (status, answer) = merge(&app, &tab, body).await;
        assert_eq!(
            (status, &answer["reason"]),
            (StatusCode::CONFLICT, &json!(MOVED_FOLDER)),
            "{answer}"
        );
        let mut body = pull_body(&rig, "ready");
        body["checkout"] = json!(checkout);
        let (status, answer) = pull_action(&app, &tab, body).await;
        assert_eq!(
            (status, &answer["reason"]),
            (StatusCode::CONFLICT, &json!(MOVED_FOLDER)),
            "{answer}"
        );
    }
    assert!(rig.scratch.writes().is_empty(), "gh was run: {:?}", rig.scratch.calls());
}

#[tokio::test]
async fn the_repository_comes_from_the_checkout() {
    let rig = Rig::new();
    script_with(&rig, json!([{"match": ["--input"], "exit": 0, "stdout": CREATED}]));
    let (app, tab) = seen(&rig).await;
    let naming = json!({
        "repo": "evil/other", "repository": "evil/other", "owner": "evil", "name": "other",
        "host": "evil.example", "hostname": "evil.example", "url": "https://evil.example/evil/other",
        "path": "repos/evil/other/pulls",
    });
    let with = |mut body: Value| {
        for (key, value) in naming.as_object().unwrap() {
            body[key] = value.clone();
        }
        body
    };
    assert_eq!(create(&app, &tab, with(create_body(&rig))).await.0, StatusCode::OK);
    assert_eq!(
        merge(&app, &tab, with(merge_body(&rig, "merge", false))).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        merge(&app, &tab, with(merge_body(&rig, "merge", true))).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        pull_action(&app, &tab, with(pull_body(&rig, "ready"))).await.0,
        StatusCode::NO_CONTENT
    );
    let writes = rig.scratch.writes();
    assert_eq!(writes.len(), 4);
    for (args, sent) in &writes {
        assert!(!args.iter().any(|a| a.contains("evil")), "{args:?}");
        assert!(!sent.to_string().contains("evil"), "{sent}");
        assert_eq!(args[2], "github.com");
    }
    assert_eq!(writes[0].0[3], "repos/octo-org/widgets/pulls");
    assert_eq!(writes[1].0[5], "repos/octo-org/widgets/pulls/7/merge");
}

#[tokio::test]
async fn actions_say_why_github_could_not_be_reached_or_refused() {
    let rig = Rig::new();
    let (app, tab) = seen(&rig).await;
    let cases = [
        (
            json!({"exit": 4, "stdout": "", "stderr": "To get started with GitHub CLI, please run:  gh auth login\n"}),
            StatusCode::SERVICE_UNAVAILABLE,
            NEED_SIGN_IN.to_string(),
        ),
        (
            json!({"exit": 1, "stdout": "", "stderr": "error connecting to api.github.com\n"}),
            StatusCode::SERVICE_UNAVAILABLE,
            "Can't reach GitHub: error connecting to api.github.com".to_string(),
        ),
        (
            json!({"exit": 1, "stdout": r#"{"message":"API rate limit exceeded for user ID 1."}"#,
                "stderr": "gh: API rate limit exceeded for user ID 1. (HTTP 403)\n"}),
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub's limit for this sign-in is used up.".to_string(),
        ),
        (
            json!({"exit": 0, "stdout": "not json"}),
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub's answer couldn't be read.".to_string(),
        ),
        (
            json!({"exit": 1, "stdout": r#"{"message":"Not Found"}"#, "stderr": "gh: Not Found (HTTP 404)\n"}),
            StatusCode::CONFLICT,
            "GitHub has no repository octo-org/widgets that this sign-in can see.".to_string(),
        ),
    ];
    for (mut entry, want_status, want) in cases {
        entry["match"] = json!(["--input"]);
        rig.scratch.script(json!([entry]));
        let (status, body) = merge(&app, &tab, merge_body(&rig, "merge", false)).await;
        assert_eq!((status, &body["reason"]), (want_status, &json!(want)), "{body}");
    }
}

#[tokio::test]
async fn actions_need_gh_and_the_switch() {
    let rig = Rig::new();
    let state = state_in(&rig.scratch).with_tab_path(|| Some(std::ffi::OsString::new()));
    let (app, tab) = open(&state, &rig.scratch).await;
    let (status, body) = create(&app, &tab, create_body(&rig)).await;
    assert_eq!(
        (status, &body["reason"]),
        (StatusCode::SERVICE_UNAVAILABLE, &json!(NEED_CLI)),
        "{body}"
    );
    let (status, body) = pull_action(&app, &tab, pull_body(&rig, "ready")).await;
    assert_eq!(
        (status, &body["reason"]),
        (StatusCode::SERVICE_UNAVAILABLE, &json!(NEED_CLI)),
        "{body}"
    );

    let (app, tab) = open(&rig.state(), &rig.scratch).await;
    let (_, mut settings) = call(&app, "GET", "/v1/settings", None).await;
    settings["changes"]["pull_requests"] = json!(false);
    assert_eq!(
        call(&app, "PUT", "/v1/settings", Some(settings)).await.0,
        StatusCode::OK
    );
    let off = "Pull requests from GitHub are switched off in Settings.";
    let (status, body) = create(&app, &tab, create_body(&rig)).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(off)), "{body}");
    let (status, body) = merge(&app, &tab, merge_body(&rig, "merge", false)).await;
    assert_eq!((status, &body["reason"]), (StatusCode::CONFLICT, &json!(off)), "{body}");
    assert!(rig.scratch.writes().is_empty());
}

#[tokio::test]
async fn actions_for_an_unknown_tab_are_404() {
    let rig = Rig::new();
    let (app, _) = open(&rig.state(), &rig.scratch).await;
    let (status, _) = create(&app, "nope", create_body(&rig)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = merge(&app, "nope", merge_body(&rig, "merge", false)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = pull_action(&app, "nope", pull_body(&rig, "ready")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn action_routes_need_the_token() {
    let rig = Rig::new();
    script_with(&rig, json!([{"match": ["--input"], "exit": 0, "stdout": CREATED}]));
    let (app, tab) = seen(&rig).await;
    for (path, body) in [
        ("github/create", create_body(&rig)),
        ("github/merge", merge_body(&rig, "merge", false)),
        ("github/pull", pull_body(&rig, "ready")),
    ] {
        let res = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/sessions/{tab}/{path}"),
                Some(body),
                false,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    assert!(rig.scratch.writes().is_empty());
}
