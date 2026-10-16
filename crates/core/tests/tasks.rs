//! Task worktrees: names, plans, creation, and the repo root a session is grouped under.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::git::{GitStep, display, run};
use marshell_core::server::{AppState, router};
use marshell_core::{projects, tasks};
use marshell_protocol::api::{MergeOutcome, MergeRequest, Profile, ShellFamily, TaskPlanRequest};
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

fn request(cwd: &Path, name: &str) -> TaskPlanRequest {
    TaskPlanRequest {
        cwd: cwd.to_string_lossy().into_owned(),
        name: name.into(),
    }
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
    let first = tasks::create(&home, &request(&repo, "🙂"), 5).unwrap();
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

    let task = tasks::create(&home, &request(&repo, "Fix login"), 42).unwrap();
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
    let task = tasks::create(&home, &request(&repo, "Fix login"), 1).unwrap();
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

    let err = tasks::create(&home, &request(&repo, "Fix login"), 1).unwrap_err();
    assert!(err.starts_with("Couldn't create the worktree. Git said: "), "{err}");
    assert!(tasks::list(&home).is_empty());
}

#[test]
fn a_task_cwd_records_its_repo_as_the_project() {
    let tmp = spaced_tmp();
    let repo = repo(&base(&tmp));
    let home = base(&tmp).join("home");
    let task = tasks::create(&home, &request(&repo, "Fix login"), 1).unwrap();

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
    let task = tasks::create(&home, &request(&repo, "Fix login"), 1).unwrap();
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

    let res = app.clone().oneshot(api("POST", "/v1/tasks", Some(body))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let task = json_of(res).await;
    assert_eq!(task["path"], plan["path"]);

    let res = app.clone().oneshot(api("GET", "/v1/tasks", None)).await.unwrap();
    assert_eq!(json_of(res).await.as_array().unwrap().len(), 1);

    // A task whose folder is gone drops out of the list, not out of the file.
    std::fs::remove_dir_all(task["path"].as_str().unwrap()).unwrap();
    let res = app.clone().oneshot(api("GET", "/v1/tasks", None)).await.unwrap();
    assert!(json_of(res).await.as_array().unwrap().is_empty());
    assert_eq!(tasks::list(&home).len(), 1);

    // Outside git: 409 with a reason.
    let body = serde_json::json!({ "cwd": plain.to_string_lossy(), "name": "x" });
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
    let task = tasks::create(&home, &request(&repo, "Fix login"), 1).unwrap();
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
    let task = tasks::create(&home, &request(&repo, "Fix login"), 1).unwrap();
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

fn merge_req(message: &str) -> MergeRequest {
    MergeRequest {
        message: message.into(),
        remove: false,
    }
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

    let (plan, steps) = tasks::plan_merge(&task, &merge_req("Fix login"));
    assert_eq!(plan.blocker, None);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" add -A", task.path),
            format!("git -C \"{}\" commit -q -m \"Fix login\"", task.path),
            format!("git -C \"{}\" merge --squash marshell/fix-login", task.repo_root),
            format!("git -C \"{}\" commit -q -m \"Fix login\"", task.repo_root),
        ]
    );
    assert_eq!(plan.commands.len(), 4);
    // The steps that run are the steps that are shown.
    let shown: Vec<String> = steps.iter().map(display).collect();
    assert_eq!(shown, plan.commands);

    // A clean worktree has nothing to add or commit; the message is trimmed.
    commit_all(&dir, "edit a");
    let (plan, _) = tasks::plan_merge(&task, &merge_req("  Fix login \n"));
    assert_eq!(plan.blocker, None);
    assert_eq!(
        plan.commands,
        vec![
            format!("git -C \"{}\" merge --squash marshell/fix-login", task.repo_root),
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
    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"));
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
    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"));
    assert_eq!(
        plan.blocker.as_deref(),
        Some("my repo is on other, not main. Switch it to main, then merge.")
    );
    assert_eq!(plan.commands.len(), 4);

    // Nothing ahead and nothing uncommitted.
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"));
    assert_eq!(
        plan.blocker.as_deref(),
        Some("marshell/fix-login has no changes to merge.")
    );
    assert_eq!(plan.commands.len(), 2);

    // An empty message.
    let tmp = spaced_tmp();
    let (_home, task) = new_task(&tmp);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let (plan, _) = tasks::plan_merge(&task, &merge_req(" \t "));
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

    let outcome = tasks::merge(&state_in(&home), &task, &merge_req("Fix login"));
    assert_eq!(outcome, MergeOutcome::Merged { removed: false });
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
fn merge_refuses_when_blocked() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    let repo = PathBuf::from(&task.repo_root);
    let head = head_of(&repo);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();

    let outcome = tasks::merge(&state_in(&home), &task, &merge_req(""));
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

    let (plan, _) = tasks::plan_merge(&task, &merge_req("Fix login"));
    assert_eq!(plan.blocker, None);
    let outcome = tasks::merge(&state_in(&home), &task, &merge_req("Fix login"));
    assert_eq!(outcome, MergeOutcome::Merged { removed: false });
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

    let outcome = tasks::merge(&state_in(&home), &task, &merge_req("Fix login"));
    let MergeOutcome::Failed { reason } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(reason.starts_with("Couldn't merge. Git said: "), "{reason}");
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

    let outcome = tasks::merge(&state_in(&home), &task, &merge_req("Fix login"));
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

    let outcome = tasks::merge(&state_in(&home), &task, &merge_req("Fix login"));
    let MergeOutcome::Failed { reason } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(reason.starts_with("Couldn't commit on main. Git said: "), "{reason}");
    assert_eq!(head_of(&repo), head);
    assert_eq!(tracked_status(&repo), "");
    assert!(!repo.join("b.txt").exists());
}

#[tokio::test]
async fn merge_routes_plan_merge_and_404() {
    let tmp = spaced_tmp();
    let (home, task) = new_task(&tmp);
    std::fs::write(Path::new(&task.path).join("b.txt"), "b\n").unwrap();
    let app = router(state_in(&home));
    let body = serde_json::json!({ "message": "Fix login", "remove": false });

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
        .oneshot(api("POST", &format!("/v1/tasks/{}/merge", task.id), Some(body.clone())))
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
