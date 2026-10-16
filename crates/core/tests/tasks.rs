//! Task worktrees: names, plans, creation, and the repo root a session is grouped under.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::git::{GitStep, display, run};
use marshell_core::server::{AppState, router};
use marshell_core::{projects, tasks};
use marshell_protocol::api::{MergeOutcome, MergeRequest, Profile, RemoveRequest, ShellFamily, TaskCreateRequest};
use std::path::{Path, PathBuf};
use tower::ServiceExt;

/// A temp dir whose name has a space, like `C:\Users\Home Office`.
fn spaced_tmp() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("mar shell ").tempdir().unwrap()
}

/// The temp dir as the disk spells it (not `HOMEOF~1`), the way the core reports paths.
fn base(tmp: &tempfile::TempDir) -> PathBuf {
    projects::canonical(tmp.path())
}

/// A repo called `my repo` with one commit on `main`.
fn repo(parent: &Path) -> PathBuf {
    let dir = parent.join("my repo");
    std::fs::create_dir(&dir).unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.name", "T"][..],
        &["config", "user.email", "t@example.invalid"][..],
    ] {
        assert!(run(&dir, args).unwrap().ok);
    }
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    assert!(run(&dir, &["add", "a.txt"]).unwrap().ok);
    assert!(run(&dir, &["commit", "-q", "-m", "first"]).unwrap().ok);
    dir
}

fn request(cwd: &Path, name: &str, commands: Vec<String>) -> TaskCreateRequest {
    TaskCreateRequest {
        cwd: cwd.to_string_lossy().into_owned(),
        name: name.into(),
        commands,
    }
}

/// Plans the task, then creates it with the commands that plan showed, as the launcher does.
fn make(home: &Path, repo: &Path, name: &str, now_ms: u64) -> Result<marshell_protocol::api::Task, String> {
    let (plan, _) = tasks::plan_create(home, repo, name, now_ms).unwrap();
    tasks::create(home, &request(repo, name, plan.commands), now_ms)
}

#[test]
fn slug_rules() {
    assert_eq!(
        tasks::slug("Fix the login redirect, please!"),
        "fix-the-login-redirect-please"
    );
    assert_eq!(tasks::slug("  --A__b  "), "a-b");
    assert_eq!(tasks::slug("תקן את הכפתור 🙂"), "");
    assert_eq!(tasks::slug(""), "");

    // 60 characters: cut at the last "-" at or before 40, no trailing "-".
    let long = "one two three four five six seven eight nine ten eleven";
    let slug = tasks::slug(long);
    assert!(slug.len() <= 40, "{slug}");
    assert_eq!(slug, "one-two-three-four-five-six-seven-eight");
    assert!(!slug.ends_with('-'));

    // No "-" to cut at: a hard cut at 40.
    assert_eq!(tasks::slug(&"x".repeat(60)), "x".repeat(40));
}

#[test]
fn plan_names_branch_folder_and_command() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");

    let (plan, steps) = tasks::plan_create(&home, &repo, "Fix login", 1).unwrap();
    let path = home.join("worktrees").join("my repo").join("fix-login");
    assert_eq!(plan.name, "fix-login");
    assert_eq!(plan.branch, "marshell/fix-login");
    assert_eq!(plan.base_branch, "main");
    assert_eq!(PathBuf::from(&plan.repo_root), repo);
    assert_eq!(PathBuf::from(&plan.path), path);
    let expected = display(&GitStep {
        dir: repo.clone(),
        args: [
            "worktree",
            "add",
            "-b",
            "marshell/fix-login",
            &path.to_string_lossy(),
            "main",
        ]
        .iter()
        .map(|a| a.to_string())
        .collect(),
    });
    assert_eq!(plan.commands, vec![expected]);
    assert_eq!(steps.len(), 1);

    // From a sub folder the repo is still the repo.
    let sub = repo.join("src").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    let (from_sub, _) = tasks::plan_create(&home, &sub, "Fix login", 1).unwrap();
    assert_eq!(PathBuf::from(&from_sub.repo_root), repo);
    assert_eq!(from_sub.commands, plan.commands);
}

#[test]
fn plan_falls_back_and_deduplicates() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");

    // Nothing to name it from.
    let (plan, _) = tasks::plan_create(&home, &repo, "🙂", 1).unwrap();
    assert_eq!(plan.name, "task-1");

    // With task-1 in the registry the next is task-2.
    let first = make(&home, &repo, "🙂", 5).unwrap();
    assert_eq!(first.name, "task-1");
    let (plan, _) = tasks::plan_create(&home, &repo, "🙂", 6).unwrap();
    assert_eq!(plan.name, "task-2");

    // An existing branch makes the name fix-login-2.
    assert!(run(&repo, &["branch", "marshell/fix-login"]).unwrap().ok);
    let (plan, _) = tasks::plan_create(&home, &repo, "Fix login", 7).unwrap();
    assert_eq!(plan.name, "fix-login-2");
    assert_eq!(plan.branch, "marshell/fix-login-2");

    // So does an existing folder.
    std::fs::create_dir_all(home.join("worktrees").join("my repo").join("other")).unwrap();
    let (plan, _) = tasks::plan_create(&home, &repo, "other", 8).unwrap();
    assert_eq!(plan.name, "other-2");
}

#[test]
fn plan_refuses_outside_git_and_detached() {
    let tmp = spaced_tmp();
    let home = base(&tmp).join("home");
    let plain = base(&tmp).join("plain dir");
    std::fs::create_dir(&plain).unwrap();
    assert_eq!(
        tasks::plan_create(&home, &plain, "x", 1).unwrap_err(),
        "plain dir isn't a git repository, so it can't have a task worktree."
    );

    let repo = repo(&base(&tmp));
    assert!(run(&repo, &["checkout", "-q", "--detach"]).unwrap().ok);
    assert_eq!(
        tasks::plan_create(&home, &repo, "x", 1).unwrap_err(),
        "my repo is on a detached HEAD. Check out a branch first."
    );
}

#[test]
fn create_makes_the_worktree_and_records_it() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");

    let task = make(&home, &repo, "Fix login", 42).unwrap();
    let path = PathBuf::from(&task.path);
    assert!(path.join("a.txt").exists());
    let head = run(&path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
    assert_eq!(head.stdout.trim(), "marshell/fix-login");
    assert_eq!(task.created_ms, 42);
    assert_eq!(task.id.len(), 8);
    assert!(task.id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(task.base_branch, "main");

    assert_eq!(tasks::list(&home), vec![task.clone()]);
    assert_eq!(tasks::find(&home, &task.id), Some(task));
    assert_eq!(projects::repo_root(&path), Some(repo.clone()));
    // A sub folder of the worktree too.
    assert_eq!(projects::repo_root(&path.join("nope")), Some(repo));
}

#[test]
fn create_runs_exactly_the_planned_steps() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");

    let (plan, steps) = tasks::plan_create(&home, &repo, "Fix login", 1).unwrap();
    let shown: Vec<String> = steps.iter().map(display).collect();
    assert_eq!(shown, plan.commands);

    // `create` plans again and runs that plan: what it made is what was shown.
    let task = make(&home, &repo, "Fix login", 1).unwrap();
    assert_eq!(task.path, plan.path);
    assert_eq!(task.branch, plan.branch);
    assert!(
        run(
            &repo,
            &["rev-parse", "--verify", "--quiet", "refs/heads/marshell/fix-login"]
        )
        .unwrap()
        .ok
    );
}

#[test]
fn failed_create_records_nothing() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");

    // A file where the repo's worktree folder should be: git can't create the worktree inside it.
    std::fs::create_dir_all(home.join("worktrees")).unwrap();
    std::fs::write(home.join("worktrees").join("my repo"), "in the way").unwrap();

    let err = make(&home, &repo, "Fix login", 1).unwrap_err();
    assert!(err.starts_with("Couldn't create the worktree. Git said: "), "{err}");
    assert!(tasks::list(&home).is_empty());
}

#[test]
fn create_runs_only_the_commands_that_were_shown() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");
    let (shown, _) = tasks::plan_create(&home, &repo, "Fix login", 1).unwrap();

    // The name was taken while the user read the command: the re-plan says fix-login-2, so nothing runs.
    assert!(run(&repo, &["branch", "marshell/fix-login"]).unwrap().ok);
    let err = tasks::create(&home, &request(&repo, "Fix login", shown.commands.clone()), 2).unwrap_err();
    assert_eq!(err, CHANGED);
    assert!(tasks::list(&home).is_empty());
    assert!(!Path::new(&shown.path).exists());
    assert!(!branch_exists(&repo, "marshell/fix-login-2"));

    // Commands that were never shown are refused the same way.
    let err = tasks::create(&home, &request(&repo, "Fix login", Vec::new()), 3).unwrap_err();
    assert_eq!(err, CHANGED);
    assert!(tasks::list(&home).is_empty());

    // The fresh plan's commands go through.
    let (again, _) = tasks::plan_create(&home, &repo, "Fix login", 4).unwrap();
    let task = tasks::create(&home, &request(&repo, "Fix login", again.commands), 4).unwrap();
    assert_eq!(task.branch, "marshell/fix-login-2");
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    run(
        repo,
        &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")],
    )
    .unwrap()
    .ok
}

#[test]
fn a_task_cwd_records_its_repo_as_the_project() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");
    let task = make(&home, &repo, "Fix login", 1).unwrap();

    projects::record(&home, Path::new(&task.path), 9).unwrap();
    let recent = projects::list(&home);
    assert_eq!(recent.len(), 1);
    assert_eq!(PathBuf::from(&recent[0].path), repo);
}

#[test]
fn head_branch_is_none_when_detached() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    assert_eq!(projects::head_branch(&repo), Some("main".into()));
    assert!(run(&repo, &["checkout", "-q", "--detach"]).unwrap().ok);
    assert_eq!(projects::head_branch(&repo), None);
    assert!(projects::git_branch(&repo).is_some());
}

// ---- over HTTP ----

const TOKEN: &str = "test-token";
const PORT: u16 = 5556;

fn state_in(home: &Path) -> AppState {
    let profile = Profile {
        id: "sleeper".into(),
        name: "Sleeper".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "sleep".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.to_path_buf(),
        vec![profile],
    )
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

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 256 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn start_in(app: &axum::Router, cwd: &Path) -> String {
    let body = serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "sleeper", "cwd": cwd.to_string_lossy() });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn project_of(app: &axum::Router, tab: &str) -> String {
    let res = app.clone().oneshot(api("GET", "/v1/sessions", None)).await.unwrap();
    let list = json_of(res).await;
    let row = list["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tab_id"] == tab)
        .unwrap()
        .clone();
    row["project"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn session_info_carries_project() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");
    let task = make(&home, &repo, "Fix login", 1).unwrap();
    let plain = base(&tmp).join("plain dir");
    std::fs::create_dir(&plain).unwrap();

    let app = router(state_in(&home));
    let in_task = start_in(&app, Path::new(&task.path)).await;
    assert_eq!(PathBuf::from(project_of(&app, &in_task).await), repo);
    let in_plain = start_in(&app, &plain).await;
    assert_eq!(PathBuf::from(project_of(&app, &in_plain).await), plain);
}

#[tokio::test]
async fn task_routes_plan_create_list_and_refuse() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");
    let plain = base(&tmp).join("plain dir");
    std::fs::create_dir(&plain).unwrap();
    let app = router(state_in(&home));
    let body = serde_json::json!({ "cwd": repo.to_string_lossy(), "name": "Fix login" });

    let res = app
        .clone()
        .oneshot(api("POST", "/v1/tasks/plan", Some(body.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let plan = json_of(res).await;
    assert_eq!(plan["branch"], "marshell/fix-login");
    assert_eq!(plan["commands"].as_array().unwrap().len(), 1);

    // Commands the user never saw: 409 with the changed reason, and nothing is made.
    let mut unseen = body.clone();
    unseen["commands"] = serde_json::json!(["git worktree add somewhere-else"]);
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/tasks", Some(unseen)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(res).await["reason"], CHANGED);
    assert!(tasks::list(&home).is_empty());

    let mut shown = body.clone();
    shown["commands"] = plan["commands"].clone();
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/tasks", Some(shown)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let task = json_of(res).await;
    assert_eq!(task["path"], plan["path"]);

    let res = app.clone().oneshot(api("GET", "/v1/tasks", None)).await.unwrap();
    let listed = json_of(res).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["folder_missing"], false);

    // A task whose folder is gone stays listed, marked, so it can still be cleaned up.
    std::fs::remove_dir_all(task["path"].as_str().unwrap()).unwrap();
    let res = app.clone().oneshot(api("GET", "/v1/tasks", None)).await.unwrap();
    let listed = json_of(res).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], task["id"]);
    assert_eq!(listed[0]["folder_missing"], true);
    assert_eq!(tasks::list(&home).len(), 1);

    // Its remove plan works without the folder; removing it takes it off the list.
    let id = task["id"].as_str().unwrap();
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/tasks/{id}/remove/plan"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let plan = json_of(res).await;
    let body = serde_json::json!({ "commands": plan["commands"], "warnings": plan["warnings"] });
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/tasks/{id}/remove"), Some(body)))
        .await
        .unwrap();
    assert!(res.status().is_success(), "{}", res.status());
    let res = app.clone().oneshot(api("GET", "/v1/tasks", None)).await.unwrap();
    assert!(json_of(res).await.as_array().unwrap().is_empty());

    // Outside git: 409 with a reason.
    let body = serde_json::json!({ "cwd": plain.to_string_lossy(), "name": "x", "commands": [] });
    for route in ["/v1/tasks/plan", "/v1/tasks"] {
        let res = app
            .clone()
            .oneshot(api("POST", route, Some(body.clone())))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
        assert_eq!(
            json_of(res).await["reason"],
            "plain dir isn't a git repository, so it can't have a task worktree."
        );
    }
}

// ---- a task's changes ----

fn commit_all(dir: &Path, message: &str) {
    assert!(run(dir, &["add", "-A"]).unwrap().ok);
    assert!(run(dir, &["commit", "-q", "-m", message]).unwrap().ok);
}

fn new_task(tmp: &tempfile::TempDir) -> (PathBuf, marshell_protocol::api::Task) {
    let repo = repo(&base(tmp));
    let home = base(tmp).join("home");
    let task = make(&home, &repo, "Fix login", 1).unwrap();
    (home, task)
}

#[test]
fn changes_cover_committed_uncommitted_and_untracked() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);

    std::fs::write(dir.join("a.txt"), "a\nmore\n").unwrap();
    commit_all(&dir, "edit a");
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    std::fs::write(dir.join("b.txt"), "b\nchanged\n").unwrap();
    std::fs::create_dir(dir.join("new dir")).unwrap();
    std::fs::write(dir.join("new dir").join("c.txt"), "one\ntwo\n").unwrap();

    let changes = tasks::changes(&task).unwrap();
    let files: Vec<(&str, &str)> = changes
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.status.as_str()))
        .collect();
    assert_eq!(
        files,
        vec![
            ("a.txt", "modified"),
            ("b.txt", "added"),
            ("new dir/c.txt", "untracked")
        ]
    );
    assert_eq!(changes.ahead, 2);
    assert!(changes.uncommitted);
    assert!(!changes.truncated);
    assert!(changes.diff.contains("+++ b/new dir/c.txt"), "{}", changes.diff);
    assert!(changes.diff.contains("@@ -0,0 +1,2 @@"), "{}", changes.diff);
    assert!(changes.diff.contains("+one\n+two\n"), "{}", changes.diff);
}

#[test]
fn changes_of_a_clean_task_are_empty() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);

    let changes = tasks::changes(&task).unwrap();
    assert!(changes.files.is_empty());
    assert_eq!(changes.diff, "");
    assert_eq!(changes.ahead, 0);
    assert!(!changes.uncommitted);
    assert!(!changes.truncated);
}

#[test]
fn binary_untracked_file_is_named_not_dumped() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    std::fs::write(dir.join("blob.bin"), b"AAAA\0BBBBsecret").unwrap();

    let changes = tasks::changes(&task).unwrap();
    assert_eq!(changes.files.len(), 1);
    assert_eq!(changes.files[0].path, "blob.bin");
    assert_eq!(changes.files[0].status, "untracked");
    assert!(
        changes.diff.lines().any(|l| l == "Binary file blob.bin"),
        "{}",
        changes.diff
    );
    assert!(!changes.diff.contains("AAAA") && !changes.diff.contains("secret"));
    // A section of its own, so a splitter on `diff --git` lines doesn't fold it into the last file.
    assert!(
        changes
            .diff
            .contains("diff --git a/blob.bin b/blob.bin\nnew file mode 100644\nBinary file blob.bin\n"),
        "{}",
        changes.diff
    );
}

#[test]
fn untracked_line_count_includes_a_last_line_without_newline() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    std::fs::write(dir.join("tail.txt"), "one\ntwo\nthree").unwrap();

    let changes = tasks::changes(&task).unwrap();
    assert!(changes.diff.contains("@@ -0,0 +1,3 @@"), "{}", changes.diff);
    assert!(changes.diff.contains("+three"), "{}", changes.diff);
}

#[test]
fn deleted_renamed_and_large_files_are_reported() {
    let tmp = spaced_tmp();
    // Files that exist on the base branch, so the task can delete and rename them.
    let repo = repo(&base(&tmp));
    std::fs::write(repo.join("gone.txt"), "bye\n").unwrap();
    std::fs::write(repo.join("old.txt"), "keep me\nkeep me too\nand me\n").unwrap();
    commit_all(&repo, "more files");
    let home = base(&tmp).join("home");
    let task = make(&home, &repo, "Fix login", 1).unwrap();
    let dir = PathBuf::from(&task.path);
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    std::fs::rename(dir.join("old.txt"), dir.join("new.txt")).unwrap();
    assert!(run(&dir, &["add", "-A"]).unwrap().ok);
    std::fs::write(dir.join("huge.txt"), "z".repeat(300 * 1024)).unwrap();

    let changes = tasks::changes(&task).unwrap();
    let status = |p: &str| changes.files.iter().find(|f| f.path == p).map(|f| f.status.clone());
    assert_eq!(status("gone.txt").as_deref(), Some("deleted"));
    assert_eq!(status("new.txt").as_deref(), Some("renamed"));
    assert_eq!(status("huge.txt").as_deref(), Some("untracked"));
    assert!(
        changes.diff.lines().any(|l| l == "Large file huge.txt"),
        "{}",
        changes.diff
    );
    assert!(!changes.diff.contains("zzzz"));
}

#[test]
fn a_huge_diff_is_cut_at_a_file_boundary() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    // 25 untracked files of about 190 KiB each (under the large-file limit) pass the 4 MiB cap.
    for i in 0..25 {
        let line = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";
        std::fs::write(dir.join(format!("f{i:02}.txt")), line.repeat(3000)).unwrap();
    }

    let changes = tasks::changes(&task).unwrap();
    assert!(changes.truncated);
    assert!(changes.diff.len() <= 4 * 1024 * 1024);
    // Cut back to the start of a file section: at most one section (about 195 KB) is lost.
    assert!(changes.diff.len() > 4 * 1024 * 1024 - 200_000);
    assert!(changes.diff.ends_with('\n'));
    // Every section that is left is whole.
    let sections = changes.diff.lines().filter(|l| l.starts_with("diff --git ")).count();
    let body = changes.diff.lines().filter(|l| l.starts_with("+0123")).count();
    assert!(sections > 0);
    assert_eq!(body, sections * 3000);
    assert_eq!(changes.files.len(), 25);
}

#[tokio::test]
async fn changes_route_returns_changes_or_404() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    std::fs::write(Path::new(&task.path).join("n.txt"), "x\n").unwrap();
    let app = router(state_in(&home));

    let res = app
        .clone()
        .oneshot(api("GET", &format!("/v1/tasks/{}/changes", task.id), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_of(res).await;
    assert_eq!(body["files"][0]["path"], "n.txt");
    assert_eq!(body["files"][0]["status"], "untracked");
    assert_eq!(body["uncommitted"], true);

    let res = app
        .clone()
        .oneshot(api("GET", "/v1/tasks/nope/changes", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ---- merging a task back ----

/// A request that only asks for a plan.
fn merge_req(message: &str) -> MergeRequest {
    MergeRequest {
        message: message.into(),
        remove: false,
        commands: Vec::new(),
        warnings: Vec::new(),
    }
}

/// The request the dialog sends: the message and the commands the plan showed for it.
fn planned(task: &marshell_protocol::api::Task, message: &str) -> MergeRequest {
    let mut req = merge_req(message);
    req.commands = tasks::plan_merge(task, &req, 0).0.commands;
    req
}

/// A file's text with Windows line endings read as `\n` (git may check files out with `\r\n`).
fn text_of(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

fn head_of(dir: &Path) -> String {
    run(dir, &["rev-parse", "HEAD"]).unwrap().stdout.trim().to_string()
}

fn tracked_status(dir: &Path) -> String {
    run(dir, &["status", "--porcelain", "--untracked-files=no"])
        .unwrap()
        .stdout
}

fn commit_count(dir: &Path) -> usize {
    run(dir, &["rev-list", "--count", "HEAD"])
        .unwrap()
        .stdout
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn merge_plan_lists_commands_in_order() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    std::fs::write(dir.join("a.txt"), "task\n").unwrap();

    let (plan, steps) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(plan.blocker, None);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" add -A", task.path),
            format!("git -C \"{}\" commit -q -m \"Fix login\"", task.path),
            format!("git -C \"{}\" merge --squash --ff marshell/fix-login", task.repo_root),
            format!("git -C \"{}\" commit -q -m \"Fix login\"", task.repo_root),
        ]
    );
    assert_eq!(plan.commands.len(), 4);
    // The steps that run are the steps that are shown.
    let shown: Vec<String> = steps.iter().map(display).collect();
    assert_eq!(shown, plan.commands);

    // A clean worktree has nothing to add or commit; the message is trimmed.
    commit_all(&dir, "edit a");
    let (plan, _) = tasks::plan_merge(&task, &merge_req("  Fix login \n"), 0);
    assert_eq!(plan.blocker, None);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" merge --squash --ff marshell/fix-login", task.repo_root),
            format!("git -C \"{}\" commit -q -m \"Fix login\"", task.repo_root),
        ]
    );
}

#[test]
fn merge_plan_blockers() {
    // A tracked edit in the base checkout.
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    std::fs::write(repo.join("a.txt"), "edited in the base\n").unwrap();
    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("my repo has uncommitted changes. Commit or stash them, then merge.")
    );
    assert_eq!(plan.commands.len(), 4);

    // The base on another branch.
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    assert!(run(&repo, &["checkout", "-q", "-b", "other"]).unwrap().ok);
    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("my repo is on other, not main. Switch it to main, then merge.")
    );
    assert_eq!(plan.commands.len(), 4);

    // Nothing ahead and nothing uncommitted.
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("marshell/fix-login has no changes to merge.")
    );
    assert_eq!(plan.commands.len(), 2);

    // An empty message.
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let (plan, _) = tasks::plan_merge(&task, &merge_req(" \t "), 0);
    assert_eq!(plan.blocker.as_deref(), Some("Write a commit message."));
    assert_eq!(plan.commands.len(), 4);
}

#[test]
fn merge_squashes_into_one_commit() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    // Two commits and one uncommitted edit become one commit on main.
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    commit_all(&dir, "step 1");
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "step 2");
    std::fs::write(dir.join("a.txt"), "task\n").unwrap();
    let before = commit_count(&repo);

    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: false,
            removal_note: None
        }
    );
    assert_eq!(commit_count(&repo), before + 1);
    let subject = run(&repo, &["log", "-1", "--format=%s"]).unwrap().stdout;
    assert_eq!(subject.trim(), "Fix login");
    assert_eq!(text_of(&repo.join("a.txt")), "task\n");
    assert_eq!(text_of(&repo.join("b.txt")), "b\n");
    assert_eq!(run(&repo, &["status", "--porcelain"]).unwrap().stdout, "");
    // The task's folder was committed too.
    assert_eq!(run(&dir, &["status", "--porcelain"]).unwrap().stdout, "");
}

#[test]
fn merge_ignores_the_users_merge_ff_setting() {
    // Many people set merge.ff globally; `--squash` refuses `--no-ff`, and `only` would refuse a squash
    // that isn't a fast-forward. The shown step says `--ff`, which overrides both.
    for setting in ["false", "only"] {
        let tmp = spaced_tmp();
        let (home, task) = new_task(&tmp);
        let dir = PathBuf::from(&task.path);
        let repo = PathBuf::from(&task.repo_root);
        assert!(run(&repo, &["config", "merge.ff", setting]).unwrap().ok);
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        commit_all(&dir, "add b");
        // main moves on too, so the squash is not a fast-forward.
        std::fs::write(repo.join("c.txt"), "c\n").unwrap();
        commit_all(&repo, "add c");

        let req = planned(&task, "Fix login");
        assert!(
            req.commands.contains(&format!(
                "git -C \"{}\" merge --squash --ff marshell/fix-login",
                task.repo_root
            )),
            "{:?}",
            req.commands
        );
        let outcome = tasks::merge(&state_in(&home), &task, &req);
        assert!(
            matches!(outcome, MergeOutcome::Merged { .. }),
            "merge.ff={setting}: {outcome:?}"
        );
        assert_eq!(text_of(&repo.join("b.txt")), "b\n");
    }
}

#[test]
fn merge_refuses_when_blocked() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    let head = head_of(&repo);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();

    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, ""));
    assert_eq!(
        outcome,
        MergeOutcome::Failed {
            reason: "Write a commit message.".into()
        }
    );
    // Nothing ran: the task's file is still uncommitted and main is where it was.
    assert_eq!(head_of(&repo), head);
    assert!(
        !run(Path::new(&task.path), &["status", "--porcelain"])
            .unwrap()
            .stdout
            .is_empty()
    );
}

#[test]
fn untracked_files_in_base_do_not_block() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(repo.join("notes.txt"), "mine\n").unwrap();
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(plan.blocker, None);
    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: false,
            removal_note: None
        }
    );
    assert_eq!(std::fs::read_to_string(repo.join("notes.txt")).unwrap(), "mine\n");
}

#[test]
fn untracked_file_in_the_way_restores_base() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("n.txt"), "from the task\n").unwrap();
    commit_all(&dir, "add n");
    std::fs::write(repo.join("n.txt"), "untracked in the base\n").unwrap();
    let head = head_of(&repo);

    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    let MergeOutcome::Failed { reason } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(reason.starts_with("Couldn't merge. Git said: "), "{reason}");
    // Git's own error line, not just its closing "Aborting".
    assert!(reason.contains("untracked working tree files"), "{reason}");
    assert!(!reason.ends_with("Aborting"), "{reason}");
    assert_eq!(tracked_status(&repo), "");
    assert_eq!(head_of(&repo), head);
    // The user's own file is untouched.
    assert_eq!(
        std::fs::read_to_string(repo.join("n.txt")).unwrap(),
        "untracked in the base\n"
    );
}

#[test]
fn conflict_is_undone_and_listed() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("a.txt"), "task\n").unwrap();
    commit_all(&dir, "task edit");
    std::fs::write(repo.join("a.txt"), "main\n").unwrap();
    commit_all(&repo, "main edit");
    let head = head_of(&repo);

    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert_eq!(
        outcome,
        MergeOutcome::Conflict {
            files: vec!["a.txt".into()]
        }
    );
    assert_eq!(head_of(&repo), head);
    assert_eq!(tracked_status(&repo), "");
    assert_eq!(text_of(&repo.join("a.txt")), "main\n");
}

#[test]
fn failing_commit_hook_restores_base() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    // Written after the task's own commit, so only the commit on main is refused.
    let hook = repo.join(".git").join("hooks").join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let head = head_of(&repo);

    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    let MergeOutcome::Failed { reason } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(reason.starts_with("Couldn't commit on main. Git said: "), "{reason}");
    // The hook said nothing, but the sentence still ends with something.
    assert!(!reason.trim_end().ends_with("Git said:"), "{reason}");
    assert_eq!(head_of(&repo), head);
    assert_eq!(tracked_status(&repo), "");
    assert!(!repo.join("b.txt").exists());
}

/// Writes a hook into the repo's shared hooks folder.
fn install_hook(repo: &Path, name: &str, script: &str) {
    let hook = repo.join(".git").join("hooks").join(name);
    std::fs::write(&hook, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn worktree_commit_failure_leaves_base_untouched() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    // Hooks are shared by every worktree, so this one refuses only inside a task folder.
    install_hook(
        &repo,
        "pre-commit",
        "#!/bin/sh\ncase \"$(git rev-parse --show-toplevel)\" in */worktrees/*) exit 1;; esac\n",
    );
    let shown = planned(&task, "Fix login");
    let head = head_of(&repo);

    let outcome = tasks::merge(&state_in(&home), &task, &shown);
    let MergeOutcome::Failed { reason } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(
        reason.starts_with("Couldn't commit the task's changes. Git said: "),
        "{reason}"
    );
    assert!(!reason.trim_end().ends_with("Git said:"), "{reason}");
    assert_eq!(head_of(&repo), head);
    assert_eq!(tracked_status(&repo), "");
    assert!(!repo.join("b.txt").exists());
}

#[test]
fn merging_twice_reports_no_changes() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let state = state_in(&home);
    assert_eq!(
        tasks::merge(&state, &task, &planned(&task, "Fix login")),
        MergeOutcome::Merged {
            removed: false,
            removal_note: None
        }
    );
    let head = head_of(&repo);
    let none = MergeOutcome::Failed {
        reason: "marshell/fix-login has no changes to merge.".into(),
    };

    // Straight away nothing is ahead (same tree, parent, message and second: the same commit),
    // so the plan already says so.
    assert_eq!(tasks::merge(&state, &task, &planned(&task, "Again")), none);
    assert_eq!(head_of(&repo), head);

    // The task has a commit main lacks, but main already holds the same work: the squash stages
    // nothing, so there is no commit to make.
    let dir = PathBuf::from(&task.path);
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    commit_all(&dir, "add c in the task");
    std::fs::write(repo.join("c.txt"), "c\n").unwrap();
    commit_all(&repo, "add c in main");
    let head = head_of(&repo);
    let shown = planned(&task, "Fix login again");
    assert_eq!(shown.commands.len(), 2);
    assert_eq!(tasks::plan_merge(&task, &shown, 0).0.blocker, None);

    assert_eq!(tasks::merge(&state, &task, &shown), none);
    assert_eq!(head_of(&repo), head);
    assert_eq!(run(&repo, &["status", "--porcelain"]).unwrap().stdout, "");
}

#[test]
fn merge_refuses_when_commands_changed() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    // Shown while the folder was clean: two commands.
    let shown = planned(&task, "Fix login");
    assert_eq!(shown.commands.len(), 2);
    // Then more work arrives, so the plan now has four.
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    let head = head_of(&repo);

    let outcome = tasks::merge(&state_in(&home), &task, &shown);
    assert_eq!(
        outcome,
        MergeOutcome::Failed {
            reason: "The task changed while you were reading the commands. Review them again.".into()
        }
    );
    assert_eq!(head_of(&repo), head);
    assert_eq!(run(&repo, &["status", "--porcelain"]).unwrap().stdout, "");
    // Nothing ran in the task folder either.
    assert!(run(&dir, &["status", "--porcelain"]).unwrap().stdout.contains("c.txt"));
}

#[test]
fn a_restore_that_fails_is_reported() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let shown = planned(&task, "Fix login");
    // Another git process holds the base's index: the squash fails, and so does the reset.
    std::fs::write(repo.join(".git").join("index.lock"), "").unwrap();

    let outcome = tasks::merge(&state_in(&home), &task, &shown);
    let MergeOutcome::Failed { reason } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(reason.starts_with("Couldn't merge. Git said: "), "{reason}");
    assert!(
        reason.ends_with(" Couldn't put my repo back. Run git reset --merge in my repo."),
        "{reason}"
    );
}

#[tokio::test]
async fn merge_routes_plan_merge_and_404() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let app = router(state_in(&home));
    let body = serde_json::json!({ "message": "Fix login", "remove": false, "commands": [], "warnings": [] });

    let res = app
        .clone()
        .oneshot(api(
            "POST",
            &format!("/v1/tasks/{}/merge/plan", task.id),
            Some(body.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let plan = json_of(res).await;
    assert_eq!(plan["commands"].as_array().unwrap().len(), 4);
    assert!(plan["blocker"].is_null());

    let res = app
        .clone()
        .oneshot(api(
            "POST",
            &format!("/v1/tasks/{}/merge", task.id),
            Some(serde_json::json!({ "message": "Fix login", "remove": false, "commands": plan["commands"], "warnings": [] })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let outcome = json_of(res).await;
    assert_eq!(outcome["kind"], "merged");
    assert_eq!(outcome["removed"], false);

    for route in ["/v1/tasks/nope/merge/plan", "/v1/tasks/nope/merge"] {
        let res = app
            .clone()
            .oneshot(api("POST", route, Some(body.clone())))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{route}");
    }
}

// ---- typing into a session ----

fn echo_state(home: &Path) -> AppState {
    let profile = Profile {
        id: "echoer".into(),
        name: "Echoer".into(),
        command: vec![env!("CARGO_BIN_EXE_fake-agent").into(), "echo".into()],
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    };
    AppState::new(
        TOKEN.into(),
        PORT,
        vec!["http://tauri.localhost".into()],
        home.to_path_buf(),
        vec![profile],
    )
}

/// Whether the session's output holds `needle` within `wait`.
fn output_has(session: &marshell_core::pty::Session, needle: &str, wait: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + wait;
    loop {
        if let marshell_core::pty::Pull::Data { bytes, .. } = session.pull(0, 64 * 1024)
            && String::from_utf8_lossy(&bytes).contains(needle)
        {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[tokio::test]
async fn input_types_text_without_enter() {
    let tmp = spaced_tmp();
    let home = base(&tmp).join("home");
    let state = echo_state(&home);
    let app = router(state.clone());
    let body =
        serde_json::json!({ "cols": 80, "rows": 24, "profile_id": "echoer", "cwd": base(&tmp).to_string_lossy() });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/sessions", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tab = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    let session = state.session(&tab).unwrap();
    let route = format!("/v1/sessions/{tab}/input");

    let res = app
        .clone()
        .oneshot(api("POST", &route, Some(serde_json::json!({ "text": "hello" }))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // No Enter was added: the agent has not seen a whole line yet.
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert!(!output_has(&session, "got:hello", std::time::Duration::ZERO));
    // The user presses Enter, and the line arrives whole.
    session.write_input(b"\r".to_vec());
    assert!(output_has(&session, "got:hello", std::time::Duration::from_secs(10)));

    for text in ["a\nb", "a\rb"] {
        let res = app
            .clone()
            .oneshot(api("POST", &route, Some(serde_json::json!({ "text": text }))))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
        assert_eq!(&bytes[..], "The text can't contain a line break.".as_bytes());
    }

    let res = app
        .clone()
        .oneshot(api(
            "POST",
            "/v1/sessions/nope/input",
            Some(serde_json::json!({ "text": "x" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .clone()
        .oneshot(api("DELETE", &format!("/v1/sessions/{tab}"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

// ---- removing a task ----

fn remove_req(task: &marshell_protocol::api::Task, sessions: u32) -> RemoveRequest {
    let plan = tasks::plan_remove(task, sessions).0;
    RemoveRequest {
        commands: plan.commands,
        warnings: plan.warnings,
    }
}

fn branch_is_there(task: &marshell_protocol::api::Task) -> bool {
    let reference = format!("refs/heads/{}", task.branch);
    run(
        Path::new(&task.repo_root),
        &["rev-parse", "--verify", "--quiet", &reference],
    )
    .unwrap()
    .ok
}

#[test]
fn remove_plan_commands_and_warnings() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);

    // A clean task with nothing on its branch: the plain commands and no warnings.
    let (plan, steps) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" worktree remove \"{}\"", task.repo_root, task.path),
            format!("git -C \"{}\" branch -D marshell/fix-login", task.repo_root),
        ]
    );
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    assert_eq!(plan.sessions, 0);
    // The steps that run are the steps that are shown.
    let shown: Vec<String> = steps.iter().map(display).collect();
    assert_eq!(shown, plan.commands);

    // Uncommitted work: --force, and a warning.
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    let (plan, _) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.commands[0],
        format!(
            "git -C \"{}\" worktree remove --force \"{}\"",
            task.repo_root, task.path
        )
    );
    assert!(
        plan.warnings
            .contains(&"The worktree has uncommitted changes. Removing deletes them.".to_string()),
        "{:?}",
        plan.warnings
    );
    assert!(run(&dir, &["checkout", "--", "a.txt"]).unwrap().ok);

    // One commit that is not on main.
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    let (plan, _) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.warnings,
        vec!["marshell/fix-login has 1 commit that isn't on main. Removing deletes it.".to_string()]
    );
    // Two.
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    commit_all(&dir, "add c");
    let (plan, _) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.warnings,
        vec!["marshell/fix-login has 2 commits that aren't on main. Removing deletes them.".to_string()]
    );

    // Squash-merged: main holds the same files, so nothing is lost.
    let merged = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert_eq!(
        merged,
        MergeOutcome::Merged {
            removed: false,
            removal_note: None
        }
    );
    assert!(
        run(&repo, &["diff", "--quiet", "main", "marshell/fix-login"])
            .unwrap()
            .ok
    );
    let (plan, _) = tasks::plan_remove(&task, 0);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);

    // Sessions are counted.
    let (plan, _) = tasks::plan_remove(&task, 1);
    assert_eq!(
        plan.warnings,
        vec!["Closes 1 session running in this worktree.".to_string()]
    );
    assert_eq!(plan.sessions, 1);
    let (plan, _) = tasks::plan_remove(&task, 2);
    assert_eq!(
        plan.warnings,
        vec!["Closes 2 sessions running in this worktree.".to_string()]
    );
}

#[tokio::test]
async fn remove_closes_sessions_and_deletes_everything() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    let sub = Path::new(&task.path).join("deep").join("er");
    std::fs::create_dir_all(&sub).unwrap();
    let state = state_in(&home);
    let app = router(state.clone());
    let in_task = start_in(&app, Path::new(&task.path)).await;
    let in_sub = start_in(&app, &sub).await;
    let outside = start_in(&app, &repo).await;
    // A tab from an earlier run of the app, in the task folder.
    let old = marshell_protocol::api::SavedTab {
        tab_id: "old".into(),
        number: 9,
        name: "Old".into(),
        agent: None,
        profile_id: "sleeper".into(),
        cwd: task.path.clone(),
    };
    state
        .registry()
        .seed_ended(&[old], |tab| marshell_core::registry::Relaunch {
            profile_id: tab.profile_id.clone(),
            agent: None,
            cwd: tab.cwd.clone(),
            name: tab.name.clone(),
        });
    assert_eq!(state.registry().snapshot().sessions.len(), 4);

    // The route's plan counts the two running sessions.
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/tasks/{}/remove/plan", task.id), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let plan = json_of(res).await;
    assert_eq!(plan["sessions"], 2);

    let req = RemoveRequest {
        commands: serde_json::from_value(plan["commands"].clone()).unwrap(),
        warnings: serde_json::from_value(plan["warnings"].clone()).unwrap(),
    };
    let (st, t) = (state.clone(), task.clone());
    tokio::task::spawn_blocking(move || tasks::remove(&st, &t, &req))
        .await
        .unwrap()
        .unwrap();

    let snapshot = state.registry().snapshot();
    let ids: Vec<&str> = snapshot.sessions.iter().map(|s| s.tab_id.as_str()).collect();
    assert_eq!(ids, vec![outside.as_str()], "{snapshot:?}");
    assert!(state.session(&in_task).is_none());
    assert!(state.session(&in_sub).is_none());
    assert!(state.session(&outside).is_some());
    assert!(!Path::new(&task.path).exists());
    assert!(!branch_is_there(&task));
    assert!(tasks::list(&home).is_empty());
}

#[test]
fn remove_refuses_when_commands_changed() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let shown = remove_req(&task, 0);
    // Work arrives after the commands were shown: the plan now has --force.
    std::fs::write(dir.join("new.txt"), "x\n").unwrap();

    let err = tasks::remove(&state_in(&home), &task, &shown).unwrap_err();
    assert_eq!(
        err,
        "The task changed while you were reading the commands. Review them again."
    );
    assert!(dir.exists());
    assert_eq!(tasks::list(&home).len(), 1);
    assert!(branch_is_there(&task));
}

#[test]
fn merge_with_remove_cleans_up() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();

    // Uncommitted work is committed by the merge, so the shown removal is the clean one.
    let mut req = merge_req("Fix login");
    req.remove = true;
    let (plan, _) = tasks::plan_merge(&task, &req, 0);
    assert_eq!(plan.commands.len(), 6, "{:?}", plan.commands);
    assert_eq!(
        plan.commands[4],
        format!("git -C \"{}\" worktree remove \"{}\"", task.repo_root, task.path)
    );
    assert_eq!(
        plan.commands[5],
        format!("git -C \"{}\" branch -D marshell/fix-login", task.repo_root)
    );
    req.commands = plan.commands;

    let outcome = tasks::merge(&state_in(&home), &task, &req);
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: true,
            removal_note: None
        }
    );
    assert_eq!(text_of(&repo.join("b.txt")), "b\n");
    assert!(!dir.exists());
    assert!(!branch_is_there(&task));
    assert!(tasks::list(&home).is_empty());
}

#[cfg(windows)]
#[test]
fn a_removal_that_fails_after_a_merge_still_reports_the_merge() {
    use std::os::windows::fs::OpenOptionsExt;
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    // An ignored file, so the merge and the clean removal plan don't see it.
    std::fs::write(repo.join(".git").join("info").join("exclude"), "held.txt\n").unwrap();
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let _held = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(0)
        .open(dir.join("held.txt"))
        .unwrap();

    let mut req = merge_req("Fix login");
    req.remove = true;
    req.commands = tasks::plan_merge(&task, &req, 0).0.commands;
    let outcome = tasks::merge(&state_in(&home), &task, &req);
    let MergeOutcome::Merged {
        removed: false,
        removal_note: Some(note),
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    // The toast says why the worktree is still there.
    assert!(note.starts_with("Couldn't remove the worktree. Git said: "), "{note}");
    assert_eq!(text_of(&repo.join("b.txt")), "b\n");
    assert_eq!(tasks::list(&home).len(), 1);
}

#[cfg(windows)]
#[test]
fn remove_failure_keeps_the_task() {
    use std::os::windows::fs::OpenOptionsExt;
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    // std opens files so that they can be deleted; share_mode(0) forbids it.
    let held = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(0)
        .open(dir.join("held.txt"))
        .unwrap();
    let shown = remove_req(&task, 0);

    let err = tasks::remove(&state_in(&home), &task, &shown).unwrap_err();
    assert!(err.starts_with("Couldn't remove the worktree. Git said: "), "{err}");
    // The folder that is still there is named.
    assert!(
        err.ends_with(&format!("The folder {} is left on disk.", task.path)),
        "{err}"
    );
    assert_eq!(tasks::list(&home).len(), 1);
    assert!(branch_is_there(&task));

    // Git may have let go of the worktree before it failed on the folder. Once the file is free,
    // the task can still be removed, from whatever state git is in now.
    drop(held);
    let (plan, _) = tasks::plan_remove(&task, 0);
    let again = RemoveRequest {
        commands: plan.commands.clone(),
        warnings: plan.warnings.clone(),
    };
    tasks::remove(&state_in(&home), &task, &again).unwrap_or_else(|e| panic!("{e} for {plan:?}"));
    assert!(tasks::list(&home).is_empty());
    assert!(!branch_is_there(&task));
}

#[tokio::test]
async fn remove_routes_plan_remove_refuse_and_404() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let app = router(state_in(&home));
    let route = |what: &str| format!("/v1/tasks/{}/{what}", task.id);

    // A stale list is refused with a reason, and nothing is removed.
    let res = app
        .clone()
        .oneshot(api(
            "POST",
            &route("remove"),
            Some(serde_json::json!({ "commands": ["git status"], "warnings": [] })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_of(res).await["reason"],
        "The task changed while you were reading the commands. Review them again."
    );
    assert!(Path::new(&task.path).exists());

    let res = app
        .clone()
        .oneshot(api("POST", &route("remove/plan"), None))
        .await
        .unwrap();
    let plan = json_of(res).await;
    assert_eq!(plan["commands"].as_array().unwrap().len(), 2);
    let res = app
        .clone()
        .oneshot(api(
            "POST",
            &route("remove"),
            Some(serde_json::json!({ "commands": plan["commands"], "warnings": plan["warnings"] })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(!Path::new(&task.path).exists());

    for (path, body) in [
        ("/v1/tasks/nope/remove/plan", None),
        (
            "/v1/tasks/nope/remove",
            Some(serde_json::json!({ "commands": [], "warnings": [] })),
        ),
    ] {
        let res = app.clone().oneshot(api("POST", path, body)).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

// ---- removing a task: fix round 1 ----

const CHANGED: &str = "The task changed while you were reading the commands. Review them again.";

#[test]
fn remove_plan_fails_closed_when_it_cannot_tell() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    // The base branch is renamed, so "commits not on main" can't be asked.
    assert!(run(&repo, &["branch", "-m", "main", "trunk"]).unwrap().ok);

    let (plan, _) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.warnings,
        vec![
            "Couldn't tell whether marshell/fix-login has commits that aren't on main. Removing deletes the branch."
                .to_string()
        ]
    );
}

#[test]
fn remove_refuses_when_what_would_be_lost_changed() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let shown = remove_req(&task, 0);
    assert!(shown.warnings.is_empty());
    // A commit lands: the tree is still clean and the commands are the same, but the branch now
    // holds work the user did not see warned about.
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    assert_eq!(tasks::plan_remove(&task, 0).0.commands, shown.commands);

    let err = tasks::remove(&state_in(&home), &task, &shown).unwrap_err();
    assert_eq!(err, CHANGED);
    assert!(dir.exists());
    assert!(branch_is_there(&task));
    assert_eq!(tasks::list(&home).len(), 1);
}

/// A post-commit hook that runs `body` once, on the first commit anywhere in the repository.
fn install_once_hook(repo: &Path, marker: &Path, body: &str) {
    let marker = marker.to_string_lossy().replace('\\', "/");
    let script = format!(
        "#!/bin/sh\nunset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE\n[ -e \"{marker}\" ] && exit 0\n: > \"{marker}\"\n{body}\n"
    );
    install_hook(repo, "post-commit", &script);
}

fn merge_with_remove_after(
    late: impl Fn(&Path) -> String,
) -> (tempfile::TempDir, marshell_protocol::api::Task, PathBuf, MergeOutcome) {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    let mut req = merge_req("Fix login");
    req.remove = true;
    req.commands = tasks::plan_merge(&task, &req, 0).0.commands;
    // Something lands in the task folder after the merge's own commit.
    install_once_hook(&repo, &base(&tmp).join("ran"), &late(&dir));

    let outcome = tasks::merge(&state_in(&home), &task, &req);
    assert_eq!(tasks::list(&home).len(), 1);
    (tmp, task, dir, outcome)
}

#[test]
fn merge_keeps_the_task_when_its_branch_moved_before_the_removal() {
    let (_tmp, task, dir, outcome) = merge_with_remove_after(|dir| {
        format!(
            "git -C \"{}\" commit --allow-empty -q -m late",
            dir.to_string_lossy().replace('\\', "/")
        )
    });
    // The merge happened; the new commit is not in it, so nothing is deleted, and the note says why.
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: false,
            removal_note: Some("marshell/fix-login got new commits during the merge, so the worktree was kept.".into())
        }
    );
    assert!(dir.exists());
    assert!(branch_is_there(&task));
    assert_eq!(run(&dir, &["log", "-1", "--format=%s"]).unwrap().stdout.trim(), "late");
}

#[test]
fn merge_keeps_the_task_when_its_folder_got_dirty_before_the_removal() {
    let (_tmp, task, dir, outcome) =
        merge_with_remove_after(|dir| format!("echo late > \"{}/late.txt\"", dir.to_string_lossy().replace('\\', "/")));
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: false,
            removal_note: Some("The task folder changed during the merge, so the worktree was kept.".into())
        }
    );
    assert!(dir.join("late.txt").exists());
    assert!(branch_is_there(&task));
}

#[tokio::test]
async fn remove_closes_the_shell_panel_in_the_task_folder() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    let state = state_in(&home);
    let app = router(state.clone());
    let sub = Path::new(&task.path).join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let body = serde_json::json!({ "cwd": sub.to_string_lossy(), "cols": 80, "rows": 24 });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/shell-panel", Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let panel = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    assert!(state.session(&panel).is_some());

    let (st, t) = (state.clone(), task.clone());
    let plan = tokio::task::spawn_blocking(move || tasks::plan_remove(&t, tasks::running_sessions(&st, &t)).0)
        .await
        .unwrap();
    assert_eq!(plan.sessions, 1);
    assert_eq!(
        plan.warnings,
        vec!["Closes 1 session running in this worktree.".to_string()]
    );

    let req = RemoveRequest {
        commands: plan.commands,
        warnings: plan.warnings,
    };
    let (st, t) = (state.clone(), task.clone());
    tokio::task::spawn_blocking(move || tasks::remove(&st, &t, &req))
        .await
        .unwrap()
        .unwrap();
    assert!(state.session(&panel).is_none());
    assert!(!Path::new(&task.path).exists());
    // The panel is free again.
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/shell-panel/adopt", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // A panel somewhere else is left alone.
    let elsewhere = serde_json::json!({ "cwd": repo.to_string_lossy(), "cols": 80, "rows": 24 });
    let res = app
        .clone()
        .oneshot(api("POST", "/v1/shell-panel", Some(elsewhere)))
        .await
        .unwrap();
    let other = json_of(res).await["tab_id"].as_str().unwrap().to_string();
    assert!(state.session(&other).is_some());
}

#[test]
fn only_marshell_branches_are_removed() {
    let tmp = spaced_tmp();
    let (home, mut task) = new_task(&tmp);
    // The branch really is named something else.
    let repo = PathBuf::from(&task.repo_root);
    assert!(
        run(&repo, &["branch", "-m", "marshell/fix-login", "feature/other"])
            .unwrap()
            .ok
    );
    task.branch = "feature/other".into();

    let (plan, steps) = tasks::plan_remove(&task, 0);
    assert!(plan.commands.is_empty());
    assert!(steps.is_empty());
    assert_eq!(
        plan.warnings,
        vec!["Marshell only removes its own marshell/ branches.".to_string()]
    );
    let shown = RemoveRequest {
        commands: plan.commands,
        warnings: plan.warnings,
    };
    let err = tasks::remove(&state_in(&home), &task, &shown).unwrap_err();
    assert_eq!(err, "Marshell only removes its own marshell/ branches.");
    assert!(Path::new(&task.path).exists());

    // A merge asked to remove it merges and leaves the branch alone.
    let dir = PathBuf::from(&task.path);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    let mut req = merge_req("Fix login");
    req.remove = true;
    let (plan, _) = tasks::plan_merge(&task, &req, 0);
    assert_eq!(plan.commands.len(), 2);
    req.commands = plan.commands;
    assert_eq!(
        tasks::merge(&state_in(&home), &task, &req),
        MergeOutcome::Merged {
            removed: false,
            removal_note: None
        }
    );
    assert!(branch_is_there(&task));
    assert!(dir.exists());
    assert_eq!(tasks::list(&home).len(), 1);
}

#[test]
fn remove_finishes_when_the_folder_is_already_gone() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);

    // Deleted by hand: git still lists the worktree, as prunable.
    std::fs::remove_dir_all(&task.path).unwrap();
    let (plan, steps) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" worktree prune", task.repo_root),
            format!("git -C \"{}\" branch -D marshell/fix-login", task.repo_root),
        ]
    );
    assert_eq!(steps.iter().map(display).collect::<Vec<_>>(), plan.commands);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    tasks::remove(&state_in(&home), &task, &remove_req(&task, 0)).unwrap();
    assert!(!branch_is_there(&task));
    assert!(tasks::list(&home).is_empty());

    // Deleted by hand and already pruned: only the branch is left.
    let again = make(&home, &repo, "Second", 2).unwrap();
    std::fs::remove_dir_all(&again.path).unwrap();
    assert!(run(&repo, &["worktree", "prune"]).unwrap().ok);
    let (plan, _) = tasks::plan_remove(&again, 0);
    assert_eq!(
        plan.commands,
        vec![format!("git -C \"{}\" branch -D marshell/second", again.repo_root)]
    );
    tasks::remove(&state_in(&home), &again, &remove_req(&again, 0)).unwrap();
    assert!(!branch_is_there(&again));
    assert!(tasks::list(&home).is_empty());
}

#[test]
fn remove_finishes_when_git_no_longer_lists_the_worktree() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    // What a half-done `worktree remove` leaves: git's record is gone, the folder is not.
    std::fs::remove_dir_all(repo.join(".git").join("worktrees")).unwrap();
    assert!(Path::new(&task.path).exists());

    let (plan, _) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" worktree prune", task.repo_root),
            format!("git -C \"{}\" branch -D marshell/fix-login", task.repo_root),
        ]
    );
    assert_eq!(
        plan.warnings,
        vec![format!(
            "Git no longer lists the worktree. Its folder {} is left on disk.",
            task.path
        )]
    );
    tasks::remove(&state_in(&home), &task, &remove_req(&task, 0)).unwrap();
    assert!(!branch_is_there(&task));
    assert!(tasks::list(&home).is_empty());
    assert!(Path::new(&task.path).exists());
}

// ---- the task folder mid-rebase, mid-merge or off its branch ----

/// A task whose a.txt conflicts with a commit on main.
fn conflicting_task(tmp: &tempfile::TempDir) -> (PathBuf, marshell_protocol::api::Task) {
    let (home, task) = new_task(tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("a.txt"), "task\n").unwrap();
    commit_all(&dir, "task edit");
    std::fs::write(repo.join("a.txt"), "main\n").unwrap();
    commit_all(&repo, "main edit");
    (home, task)
}

#[test]
fn a_rebase_stopped_in_the_task_folder_blocks_the_merge_and_warns_the_removal() {
    let tmp = spaced_tmp();
    let (home, task) = conflicting_task(&tmp);
    let dir = PathBuf::from(&task.path);
    // The rebase stops on the conflict with HEAD detached.
    assert!(!run(&dir, &["rebase", "main"]).unwrap().ok);

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("A rebase is in progress in the task folder. Finish or abort it there, then merge.")
    );
    let main_before = head_of(Path::new(&task.repo_root));
    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert!(matches!(outcome, MergeOutcome::Failed { .. }), "{outcome:?}");
    assert_eq!(head_of(Path::new(&task.repo_root)), main_before);

    let (plan, _) = tasks::plan_remove(&task, 0);
    assert!(
        plan.warnings
            .contains(&"A rebase is in progress in the task folder. Removing abandons it.".to_string()),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_cherry_pick_stopped_in_the_task_folder_blocks_the_merge_and_warns_the_removal() {
    let tmp = spaced_tmp();
    let (home, task) = conflicting_task(&tmp);
    let dir = PathBuf::from(&task.path);
    // Stops on the conflict while HEAD stays on the task branch.
    assert!(!run(&dir, &["cherry-pick", "main"]).unwrap().ok);

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("A cherry-pick or revert is in progress in the task folder. Finish or abort it there, then merge.")
    );
    let main_before = head_of(Path::new(&task.repo_root));
    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert!(matches!(outcome, MergeOutcome::Failed { .. }), "{outcome:?}");
    assert_eq!(head_of(Path::new(&task.repo_root)), main_before);

    let (plan, _) = tasks::plan_remove(&task, 0);
    assert!(
        plan.warnings
            .contains(&"A cherry-pick or revert is in progress in the task folder. Removing abandons it.".to_string()),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn unresolved_conflicts_in_the_task_folder_block_the_merge_and_warn_the_removal() {
    let tmp = spaced_tmp();
    let (home, task) = conflicting_task(&tmp);
    let dir = PathBuf::from(&task.path);
    // `--quit` drops the cherry-pick state but leaves the conflicted files, as `stash pop` does.
    assert!(!run(&dir, &["cherry-pick", "main"]).unwrap().ok);
    assert!(run(&dir, &["cherry-pick", "--quit"]).unwrap().ok);

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("The task folder has files with unresolved conflicts. Resolve them there, then merge.")
    );
    let main_before = head_of(Path::new(&task.repo_root));
    let outcome = tasks::merge(&state_in(&home), &task, &planned(&task, "Fix login"));
    assert!(matches!(outcome, MergeOutcome::Failed { .. }), "{outcome:?}");
    assert_eq!(head_of(Path::new(&task.repo_root)), main_before);

    let (plan, _) = tasks::plan_remove(&task, 0);
    assert!(
        plan.warnings
            .contains(&"The task folder has files with unresolved conflicts.".to_string()),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_merge_stopped_in_the_task_folder_blocks_the_merge_and_warns_the_removal() {
    let tmp = spaced_tmp();
    let (_home, task) = conflicting_task(&tmp);
    let dir = PathBuf::from(&task.path);
    assert!(!run(&dir, &["merge", "--no-ff", "-q", "main"]).unwrap().ok);

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("A merge is in progress in the task folder. Finish or abort it there, then merge.")
    );
    let (plan, _) = tasks::plan_remove(&task, 0);
    assert!(
        plan.warnings
            .contains(&"A merge is in progress in the task folder. Removing abandons it.".to_string()),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_task_folder_off_its_branch_blocks_the_merge_and_counts_its_commits() {
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    assert!(run(&dir, &["checkout", "-q", "--detach"]).unwrap().ok);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "on a detached HEAD");

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"), 0);
    assert_eq!(
        plan.blocker.as_deref(),
        Some("marshell/fix-login isn't checked out in the task folder. Check it out there, then merge.")
    );
    let (plan, _) = tasks::plan_remove(&task, 0);
    assert_eq!(
        plan.warnings,
        vec![
            "marshell/fix-login isn't checked out in the task folder.".to_string(),
            "The task folder has 1 commit that isn't on main or marshell/fix-login. Removing deletes it.".to_string(),
        ]
    );
}

// ---- merge with remove: the sessions it closes, and why a removal didn't happen ----

async fn merge_plan_of(app: &axum::Router, task: &marshell_protocol::api::Task, remove: bool) -> serde_json::Value {
    let body = serde_json::json!({ "message": "Fix login", "remove": remove, "commands": [], "warnings": [] });
    let res = app
        .clone()
        .oneshot(api("POST", &format!("/v1/tasks/{}/merge/plan", task.id), Some(body)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

fn merge_req_from(plan: &serde_json::Value, remove: bool) -> MergeRequest {
    MergeRequest {
        message: "Fix login".into(),
        remove,
        commands: serde_json::from_value(plan["commands"].clone()).unwrap(),
        warnings: serde_json::from_value(plan["warnings"].clone()).unwrap(),
    }
}

#[tokio::test]
async fn merge_with_remove_shows_and_checks_the_sessions_it_closes() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let state = state_in(&home);
    let app = router(state.clone());
    let tab = start_in(&app, Path::new(&task.path)).await;

    // Without the removal nothing is closed, so nothing is said.
    let plan = merge_plan_of(&app, &task, false).await;
    assert_eq!(plan["warnings"], serde_json::json!([]));
    // With it, the sessions line Remove shows.
    let plan = merge_plan_of(&app, &task, true).await;
    assert_eq!(
        plan["warnings"],
        serde_json::json!(["Closes 1 session running in this worktree."])
    );

    // A request that didn't show the line is refused, and nothing is closed or merged.
    let mut unseen = merge_req_from(&plan, true);
    unseen.warnings.clear();
    let (st, t) = (state.clone(), task.clone());
    let outcome = tokio::task::spawn_blocking(move || tasks::merge(&st, &t, &unseen))
        .await
        .unwrap();
    assert_eq!(outcome, MergeOutcome::Failed { reason: CHANGED.into() });
    assert!(state.session(&tab).is_some());
    assert!(!PathBuf::from(&task.repo_root).join("b.txt").exists());

    let req = merge_req_from(&plan, true);
    let (st, t) = (state.clone(), task.clone());
    let outcome = tokio::task::spawn_blocking(move || tasks::merge(&st, &t, &req))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: true,
            removal_note: None
        }
    );
    assert!(state.session(&tab).is_none());
    assert!(!Path::new(&task.path).exists());
}

#[tokio::test]
async fn a_skipped_removal_after_a_merge_leaves_the_sessions_running() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let dir = PathBuf::from(&task.path);
    let repo = PathBuf::from(&task.repo_root);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    commit_all(&dir, "add b");
    let state = state_in(&home);
    let app = router(state.clone());
    let tab = start_in(&app, &dir).await;
    let plan = merge_plan_of(&app, &task, true).await;
    // The folder gets a new file after the merge's commit, so the removal won't go ahead.
    let late = format!("echo late > \"{}/late.txt\"", dir.to_string_lossy().replace('\\', "/"));
    install_once_hook(&repo, &base(&tmp).join("ran"), &late);

    let req = merge_req_from(&plan, true);
    let (st, t) = (state.clone(), task.clone());
    let outcome = tokio::task::spawn_blocking(move || tasks::merge(&st, &t, &req))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            removed: false,
            removal_note: Some("The task folder changed during the merge, so the worktree was kept.".into())
        }
    );
    assert!(
        state.session(&tab).is_some(),
        "a removal that is skipped must not close sessions"
    );
    assert!(dir.exists());
}
