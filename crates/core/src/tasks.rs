//! Tasks: git worktrees that Marshell made, kept in `~/.marshell/tasks.json`.
//! Creating one runs a single git command, and the user sees that command before it runs.

use crate::git::{self, GitStep};
use crate::projects;
use crate::server::AppState;
use crate::store::{self, Versioned};
use marshell_protocol::api::{
    ChangedFile, MergeOutcome, MergePlan, MergeRequest, RemovePlan, RemoveRequest, SessionState, Task, TaskChanges,
    TaskPlan, TaskPlanRequest,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The longest task name made from a prompt.
const MAX_NAME: usize = 40;

#[derive(Serialize, Deserialize, Default)]
struct FileDoc {
    tasks: Vec<Task>,
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

fn file(home: &Path) -> PathBuf {
    home.join("tasks.json")
}

/// `create` reads the file, makes the worktree and writes the file; two at once could pick the
/// same name. Held for all of it.
static TASKS_LOCK: Mutex<()> = Mutex::new(());

/// A folder-and-branch-safe name from free text: lowercase ASCII, every run of other characters
/// becomes one `-`, and at most 40 characters (cut at a `-` when there is one). May be empty.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out = out.trim_matches('-').to_string();
    if out.len() > MAX_NAME {
        // Everything is ASCII here, so byte positions are character positions.
        let cut = out[..=MAX_NAME].rfind('-').unwrap_or(MAX_NAME);
        out.truncate(cut);
    }
    out.trim_matches('-').to_string()
}

/// The tasks in the file, oldest first. A missing, broken or newer-version file gives none.
pub fn list(home: &Path) -> Vec<Task> {
    let doc = store::read::<FileDoc>(&file(home)).unwrap_or_else(|err| {
        tracing::warn!("ignoring tasks file: {err:#}");
        None
    });
    doc.unwrap_or_default().tasks
}

pub fn find(home: &Path, id: &str) -> Option<Task> {
    list(home).into_iter().find(|t| t.id == id)
}

fn new_id() -> String {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).expect("the OS random number generator failed");
    hex::encode(bytes)
}

fn folder_name(dir: &Path) -> String {
    dir.file_name().map_or_else(
        || dir.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Whether the branch exists. A read-only question, so it is not shown to the user.
fn branch_exists(repo: &Path, branch: &str) -> Result<bool, String> {
    let reference = format!("refs/heads/{branch}");
    git::run(repo, &["rev-parse", "--verify", "--quiet", &reference])
        .map(|out| out.ok)
        .map_err(|e| format!("{e:#}"))
}

/// What creating a task named after `name` in the repository holding `cwd` would do, with the
/// git steps that do it. The steps shown to the user and the steps run are the same list.
pub fn plan_create(home: &Path, cwd: &Path, name: &str, _now_ms: u64) -> Result<(TaskPlan, Vec<GitStep>), String> {
    let root = projects::repo_root(cwd).ok_or_else(|| {
        format!(
            "{} isn't a git repository, so it can't have a task worktree.",
            folder_name(cwd)
        )
    })?;
    let repo_name = folder_name(&root);
    let base = projects::head_branch(&root)
        .ok_or_else(|| format!("{repo_name} is on a detached HEAD. Check out a branch first."))?;

    let folder = home.join("worktrees").join(&repo_name);
    let taken: Vec<String> = list(home)
        .into_iter()
        .filter(|t| projects::same_path(&t.repo_root, &root.to_string_lossy()))
        .map(|t| t.name)
        .collect();
    let wanted = slug(name);
    let mut n = 1u32;
    let (task_name, branch, path) = loop {
        // An empty slug starts at task-1; a name that is taken gets -2, -3 ...
        let candidate = match (wanted.is_empty(), n) {
            (true, n) => format!("task-{n}"),
            (false, 1) => wanted.clone(),
            (false, n) => format!("{wanted}-{n}"),
        };
        let branch = format!("marshell/{candidate}");
        let path = folder.join(&candidate);
        if !taken.contains(&candidate) && !path.exists() && !branch_exists(&root, &branch)? {
            break (candidate, branch, path);
        }
        n += 1;
        if n > 1000 {
            return Err("Couldn't find a free name for the task.".into());
        }
    };

    let step = GitStep {
        dir: root.clone(),
        args: vec![
            "worktree".into(),
            "add".into(),
            "-b".into(),
            branch.clone(),
            path.to_string_lossy().into_owned(),
            base.clone(),
        ],
    };
    let plan = TaskPlan {
        name: task_name,
        branch,
        base_branch: base,
        repo_root: root.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        commands: vec![git::display(&step)],
    };
    Ok((plan, vec![step]))
}

/// Makes the worktree and records the task. Plans again first, so the name is free right now.
/// Nothing is recorded when git fails.
pub fn create(home: &Path, req: &TaskPlanRequest, now_ms: u64) -> Result<Task, String> {
    let _guard = TASKS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // A file we can't read (broken, or from a newer Marshell) stops us before anything is made.
    let mut doc = store::read::<FileDoc>(&file(home))
        .map_err(|e| format!("Couldn't read the task list: {e:#}"))?
        .unwrap_or_default();

    let (plan, steps) = plan_create(home, Path::new(&req.cwd), &req.name, now_ms)?;
    git::run_steps(&steps)
        .map_err(|failure| format!("Couldn't create the worktree. Git said: {}", git::said(&failure.output)))?;

    let task = Task {
        id: new_id(),
        name: plan.name,
        branch: plan.branch,
        base_branch: plan.base_branch,
        repo_root: plan.repo_root,
        path: plan.path,
        created_ms: now_ms,
    };
    doc.tasks.push(task.clone());
    store::write(&file(home), &doc)
        .map_err(|e| format!("The worktree was created, but the task couldn't be saved: {e:#}"))?;
    Ok(task)
}

/// The most diff text one answer carries.
const MAX_DIFF: usize = 4 * 1024 * 1024;
/// An untracked file bigger than this is named, not shown.
const MAX_UNTRACKED: u64 = 256 * 1024;
/// An untracked file is binary when these leading bytes hold a NUL.
const BINARY_SNIFF: usize = 8 * 1024;

/// Runs a read-only git command in the task folder and gives its output, or one sentence.
fn read(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git::run(dir, args).map_err(|e| format!("{e:#}"))?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(format!(
            "Couldn't list the task's changes. Git said: {}",
            git::said(&out)
        ))
    }
}

/// `git diff --name-status -z`: a status, then a path (two paths for a rename or copy).
fn tracked_files(name_status: &str) -> Vec<ChangedFile> {
    let mut files = Vec::new();
    let mut parts = name_status.split('\0').filter(|p| !p.is_empty());
    while let Some(code) = parts.next() {
        let letter = code.chars().next().unwrap_or('M');
        let first = parts.next();
        // A rename or copy lists the old path, then the new one; the new one is used.
        let path = if matches!(letter, 'R' | 'C') {
            parts.next()
        } else {
            first
        };
        let Some(path) = path else { break };
        let status = match letter {
            'A' | 'C' => "added",
            'D' => "deleted",
            'R' => "renamed",
            _ => "modified",
        };
        files.push(ChangedFile {
            path: path.to_string(),
            status: status.into(),
        });
    }
    files
}

/// An untracked file written as the diff git would show for a new file. A binary or large file is
/// only named.
fn untracked_diff(dir: &Path, path: &str) -> String {
    let full = dir.join(path);
    let Ok(meta) = std::fs::metadata(&full) else {
        return String::new();
    };
    if meta.len() > MAX_UNTRACKED {
        return format!("{}Large file {path}\n", new_file_header(path));
    }
    let Ok(bytes) = std::fs::read(&full) else {
        return String::new();
    };
    if bytes[..bytes.len().min(BINARY_SNIFF)].contains(&0) {
        return format!("{}Binary file {path}\n", new_file_header(path));
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = new_file_header(path);
    out.push_str(&format!("--- /dev/null\n+++ b/{path}\n"));
    if lines.is_empty() {
        // An empty file has no hunk.
        return out;
    }
    out.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
    for line in lines {
        out.push('+');
        out.push_str(line);
        if !line.ends_with('\n') {
            out.push_str("\n\\ No newline at end of file\n");
        }
    }
    out
}

/// The header lines that open one file's section of a diff, for a new file.
fn new_file_header(path: &str) -> String {
    format!("diff --git a/{path} b/{path}\nnew file mode 100644\n")
}

/// Cuts the diff to at most `max` bytes without leaving half a file: back to the start of the last
/// `diff --git` line that begins inside the cap. When that is the first line (one huge file), the
/// cut is just after the last line break.
fn cut_diff(text: &mut String, max: usize) {
    let head = &text.as_bytes()[..max];
    let marker = b"\ndiff --git ";
    let section = head.windows(marker.len()).rposition(|w| w == marker);
    let keep = match section {
        Some(i) => i + 1,
        None => head.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1),
    };
    text.truncate(keep);
}

/// What the task's folder holds beyond its base branch: committed, uncommitted and untracked
/// work. Read-only git calls, so none are shown to the user.
pub fn changes(task: &Task) -> Result<TaskChanges, String> {
    let dir = Path::new(&task.path);
    let base = task.base_branch.as_str();
    let merge_base = read(dir, &["merge-base", base, "HEAD"])?;
    let merge_base = merge_base.trim();

    // Prefixes and textconv are set here so a user's git config can't change the section headers.
    let diff_args = [
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        merge_base,
    ];
    let mut diff = read(dir, &diff_args)?;
    let mut files = tracked_files(&read(dir, &["diff", "--name-status", "-z", merge_base])?);

    let untracked = read(dir, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    for path in untracked.split('\0').filter(|p| !p.is_empty()) {
        files.push(ChangedFile {
            path: path.to_string(),
            status: "untracked".into(),
        });
        // Past the cap nothing more is shown, but the file is still listed.
        if diff.len() <= MAX_DIFF {
            diff.push_str(&untracked_diff(dir, path));
        }
    }
    let truncated = diff.len() > MAX_DIFF;
    if truncated {
        cut_diff(&mut diff, MAX_DIFF);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let ahead = read(dir, &["rev-list", "--count", &format!("{base}..HEAD")])?
        .trim()
        .parse()
        .unwrap_or(0);
    let uncommitted = !read(dir, &["status", "--porcelain"])?.trim().is_empty();
    Ok(TaskChanges {
        files,
        diff,
        truncated,
        ahead,
        uncommitted,
    })
}

/// Two merges into the same checkout at once would trip over each other, and a removal must not
/// start under a merge. Held for all of `merge` and all of `remove`. Always taken before
/// `TASKS_LOCK`, never after it.
static MERGE_LOCK: Mutex<()> = Mutex::new(());

/// A read-only git question asked in `dir`; the error is one sentence.
fn ask(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git::run(dir, args).map_err(|e| format!("{e:#}"))?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(format!(
            "Couldn't read {}. Git said: {}",
            folder_name(dir),
            git::said(&out)
        ))
    }
}

/// The first reason the merge can't go ahead, or None.
fn merge_blocker(task: &Task, message: &str) -> Result<Option<String>, String> {
    let repo = Path::new(&task.repo_root);
    let repo_name = folder_name(repo);
    // Tracked changes in the base would be mixed into the squash commit. Untracked files are the
    // user's own and stay out of it.
    if !ask(repo, &["status", "--porcelain", "--untracked-files=no"])?
        .trim()
        .is_empty()
    {
        return Ok(Some(format!(
            "{repo_name} has uncommitted changes. Commit or stash them, then merge."
        )));
    }
    let on = projects::head_branch(repo);
    if on.as_deref() != Some(task.base_branch.as_str()) {
        let on = on.unwrap_or_else(|| "a detached HEAD".into());
        return Ok(Some(format!(
            "{repo_name} is on {on}, not {}. Switch it to {}, then merge.",
            task.base_branch, task.base_branch
        )));
    }
    let dir = Path::new(&task.path);
    let ahead = ask(dir, &["rev-list", "--count", &format!("{}..HEAD", task.base_branch)])?;
    if ahead.trim() == "0" && !worktree_dirty(dir)? {
        return Ok(Some(no_changes(task)));
    }
    if message.is_empty() {
        return Ok(Some("Write a commit message.".into()));
    }
    Ok(None)
}

/// Whether the task folder has anything `git add -A` would commit.
fn worktree_dirty(dir: &Path) -> Result<bool, String> {
    Ok(!ask(dir, &["status", "--porcelain"])?.trim().is_empty())
}

/// What merging the task into its base branch would run, with the git steps that run it. The
/// steps are the task folder's own commit (when it has uncommitted work), then the squash and the
/// commit in the base checkout; the last two are always the base checkout's. A blocker is a
/// reason it can't go ahead; the commands are still listed so the sheet can show them.
pub fn plan_merge(task: &Task, req: &MergeRequest) -> (MergePlan, Vec<GitStep>) {
    let message = req.message.trim();
    let step = |dir: &str, args: &[&str]| GitStep {
        dir: PathBuf::from(dir),
        args: args.iter().map(|a| a.to_string()).collect(),
    };
    let mut steps = Vec::new();
    // If the folder can't be read the question comes back below as the blocker.
    if worktree_dirty(Path::new(&task.path)).unwrap_or(false) {
        steps.push(step(&task.path, &["add", "-A"]));
        steps.push(step(&task.path, &["commit", "-q", "-m", message]));
    }
    steps.push(step(&task.repo_root, &["merge", "--squash", &task.branch]));
    steps.push(step(&task.repo_root, &["commit", "-q", "-m", message]));
    if req.remove && own_branch(task) {
        // By then the merge has committed the folder's work and the branch is in the base, so
        // the removal is the plain one whatever the folder holds now.
        steps.extend(removal_steps(task, Worktree::Remove { force: false }, true));
    }

    let blocker = merge_blocker(task, message).unwrap_or_else(Some);
    let plan = MergePlan {
        commands: steps.iter().map(git::display).collect(),
        blocker,
    };
    (plan, steps)
}

/// Puts the base checkout back after a squash or commit that failed halfway. Not a shown
/// command: it only undoes what the shown command changed, in a checkout that was clean.
/// When that fails too, the sentence that tells the user what to run themselves.
fn restore_base(repo: &Path) -> Option<String> {
    match git::run(repo, &["reset", "-q", "--merge"]) {
        Ok(out) if out.ok => None,
        _ => {
            let name = folder_name(repo);
            Some(format!("Couldn't put {name} back. Run git reset --merge in {name}."))
        }
    }
}

/// `reason`, followed by the restore sentence when the restore failed.
fn with_restore_note(reason: String, note: Option<String>) -> MergeOutcome {
    let reason = match note {
        Some(note) => format!("{reason} {note}"),
        None => reason,
    };
    MergeOutcome::Failed { reason }
}

/// Squashes the task into its base branch as one commit. Plans again first and stops when the
/// commands differ from the ones the user was shown, or at a blocker. When the squash or the
/// commit fails, the base checkout is put back as it was.
pub fn merge(st: &AppState, task: &Task, req: &MergeRequest) -> MergeOutcome {
    let _guard = MERGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let failed = |reason: String| MergeOutcome::Failed { reason };
    let (plan, steps) = plan_merge(task, req);
    if plan.commands != req.commands {
        return failed(CHANGED.into());
    }
    if let Some(blocker) = plan.blocker {
        return failed(blocker);
    }
    let repo = Path::new(&task.repo_root);
    // The removal steps (when asked for) come last, after the base checkout's squash and commit.
    let removal_len = if req.remove && own_branch(task) { 2 } else { 0 };
    let (merge_steps, removal_steps) = steps.split_at(steps.len() - removal_len);
    let (worktree_steps, base_steps) = merge_steps.split_at(merge_steps.len() - 2);

    if let Err(f) = git::run_steps(worktree_steps) {
        return failed(format!(
            "Couldn't commit the task's changes. Git said: {}",
            git::said(&f.output)
        ));
    }

    // The branch as the merge is about to take it. A removal only goes ahead if it is still here.
    let merged_tip = branch_tip(repo, &task.branch);

    if let Err(f) = git::run_steps(&base_steps[..1]) {
        // A conflict leaves the files git couldn't merge in the index; list them, then undo.
        let conflicts = git::run(repo, &["diff", "--name-only", "--diff-filter=U"])
            .map(|out| out.stdout)
            .unwrap_or_default();
        let files: Vec<String> = conflicts
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect();
        let note = restore_base(repo);
        return match (files.is_empty(), note) {
            (false, None) => MergeOutcome::Conflict { files },
            // The file list doesn't help when the base wasn't put back.
            (false, note) => with_restore_note("Couldn't merge.".into(), note),
            (true, note) => with_restore_note(format!("Couldn't merge. Git said: {}", git::said(&f.output)), note),
        };
    }

    // A squash that stages nothing (the task's work is already in the base) has nothing to commit.
    if git::run(repo, &["diff", "--cached", "--quiet"]).is_ok_and(|out| out.ok) {
        return with_restore_note(no_changes(task), restore_base(repo));
    }

    if let Err(f) = git::run_steps(&base_steps[1..]) {
        let note = restore_base(repo);
        return with_restore_note(
            format!(
                "Couldn't commit on {}. Git said: {}",
                task.base_branch,
                git::said(&f.output)
            ),
            note,
        );
    }

    // The merge is done. When the removal fails the merge still stands, so it is not a failure.
    let removed = !removal_steps.is_empty() && {
        // Work that reached the task after the merge took its branch is not in the merge. Deleting
        // the branch or the folder would lose it, and the user never saw that in the commands.
        let moved = merged_tip.is_none() || branch_tip(repo, &task.branch) != merged_tip;
        let dirty = worktree_dirty(Path::new(&task.path)).unwrap_or(true);
        if moved || dirty {
            tracing::warn!("merged {}, but it changed since, so it was not removed", task.branch);
            false
        } else {
            close_sessions(st, task);
            match run_removal(st, task, removal_steps) {
                Ok(()) => true,
                Err(reason) => {
                    tracing::warn!("merged {}, but removing it failed: {reason}", task.branch);
                    false
                }
            }
        }
    };
    MergeOutcome::Merged { removed }
}

/// What `merge` and `remove` say when the commands they plan are not the ones the user saw.
const CHANGED: &str = "The task changed while you were reading the commands. Review them again.";

/// The sentence for a merge with nothing to merge. Also the blocker's.
fn no_changes(task: &Task) -> String {
    format!("{} has no changes to merge.", task.branch)
}

/// The git steps that remove a task: its worktree (`--force` when it holds work that git would
/// refuse to delete), then its branch. Both run in the repository, not in the folder going away.
fn removal_steps(task: &Task, worktree: Worktree, branch: bool) -> Vec<GitStep> {
    let step = |args: Vec<String>| GitStep {
        dir: PathBuf::from(&task.repo_root),
        args,
    };
    let mut steps = Vec::new();
    match worktree {
        Worktree::Remove { force } => {
            let mut args = vec!["worktree".to_string(), "remove".to_string()];
            if force {
                args.push("--force".into());
            }
            args.push(task.path.clone());
            steps.push(step(args));
        }
        Worktree::Prune => steps.push(step(vec!["worktree".into(), "prune".into()])),
        Worktree::Keep => {}
    }
    if branch {
        steps.push(step(vec!["branch".into(), "-D".into(), task.branch.clone()]));
    }
    steps
}

/// What the removal does about the worktree.
#[derive(Clone, Copy)]
enum Worktree {
    /// Nothing is left of it.
    Keep,
    /// Git lists it and its folder is there.
    Remove { force: bool },
    /// Git's record of it is stale (its folder is gone), or git has already dropped the record.
    Prune,
}

/// Marshell only creates `marshell/` branches, and only removes those.
const OWN_BRANCH_ONLY: &str = "Marshell only removes its own marshell/ branches.";

fn own_branch(task: &Task) -> bool {
    task.branch.starts_with("marshell/")
}

/// Whether git lists a worktree at the task's folder. Git prints the paths with forward slashes.
fn worktree_listed(task: &Task) -> Result<bool, String> {
    let out =
        git::run(Path::new(&task.repo_root), &["worktree", "list", "--porcelain"]).map_err(|e| format!("{e:#}"))?;
    if !out.ok {
        return Err(git::said(&out).to_string());
    }
    let slashed = |p: &str| p.replace('\\', "/").trim_start_matches("//?/").to_string();
    let ours = slashed(&task.path);
    Ok(out
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .any(|p| projects::same_path(&slashed(p), &ours)))
}

/// The commit the branch points at, when it can be read.
fn branch_tip(repo: &Path, branch: &str) -> Option<String> {
    let reference = format!("refs/heads/{branch}");
    let out = git::run(repo, &["rev-parse", "--verify", "--quiet", &reference])
        .ok()
        .filter(|o| o.ok)?;
    Some(out.stdout.trim().to_string())
}

/// How many commits the branch has that its base lacks, when the base doesn't already hold the
/// same files (a squash merge leaves the commits behind but the files in the base). None when git
/// can't say.
fn unmerged_commits(task: &Task) -> Option<u32> {
    let repo = Path::new(&task.repo_root);
    let range = format!("{}..{}", task.base_branch, task.branch);
    let count = git::run(repo, &["rev-list", "--count", &range]).ok().filter(|o| o.ok)?;
    let count: u32 = count.stdout.trim().parse().ok()?;
    let same_files = git::run(repo, &["diff", "--quiet", &task.base_branch, &task.branch, "--"])
        .ok()
        .is_some_and(|o| o.ok);
    Some(if same_files { 0 } else { count })
}

/// What removing the task would run and lose, with the git steps that run it. `open_sessions` is
/// how many running sessions the removal would close. The steps shown are the steps run.
pub fn plan_remove(task: &Task, open_sessions: u32) -> (RemovePlan, Vec<GitStep>) {
    if !own_branch(task) {
        let plan = RemovePlan {
            commands: Vec::new(),
            warnings: vec![OWN_BRANCH_ONLY.to_string()],
            sessions: 0,
        };
        return (plan, Vec::new());
    }
    let dir = Path::new(&task.path);
    let repo = Path::new(&task.repo_root);
    // A folder or branch that is already gone leaves nothing to run for it, so a removal that
    // stopped halfway can be finished.
    let folder_there = dir.exists();
    let branch_there = branch_exists(repo, &task.branch).unwrap_or(true);
    // When git can't say, it is taken as listed: the plain removal is the one that fails loudly.
    let listed = worktree_listed(task).unwrap_or(true);
    let worktree = match (folder_there, listed) {
        (true, true) => Worktree::Remove {
            force: worktree_dirty(dir).unwrap_or(false),
        },
        // A half-done removal: git let go of the worktree but the folder stayed behind.
        (true, false) => Worktree::Prune,
        // Deleted by hand: git still lists it as prunable.
        (false, true) => Worktree::Prune,
        (false, false) => Worktree::Keep,
    };
    let steps = removal_steps(task, worktree, branch_there);

    let mut warnings = Vec::new();
    if matches!(worktree, Worktree::Remove { force: true }) {
        warnings.push("The worktree has uncommitted changes. Removing deletes them.".to_string());
    }
    if folder_there && !listed {
        warnings.push(format!(
            "Git no longer lists the worktree. Its folder {} is left on disk.",
            task.path
        ));
    }
    if branch_there {
        match unmerged_commits(task) {
            Some(1) => warnings.push(format!(
                "{} has 1 commit that isn't on {}. Removing deletes it.",
                task.branch, task.base_branch
            )),
            Some(n) if n > 1 => warnings.push(format!(
                "{} has {n} commits that aren't on {}. Removing deletes them.",
                task.branch, task.base_branch
            )),
            Some(_) => {}
            // Fail closed: if git can't say, the user is told the branch may hold unseen work.
            None => warnings.push(format!(
                "Couldn't tell whether {} has commits that aren't on {}. Removing deletes the branch.",
                task.branch, task.base_branch
            )),
        }
    }
    match open_sessions {
        0 => {}
        1 => warnings.push("Closes 1 session running in this worktree.".into()),
        n => warnings.push(format!("Closes {n} sessions running in this worktree.")),
    }
    let plan = RemovePlan {
        commands: steps.iter().map(git::display).collect(),
        warnings,
        sessions: open_sessions,
    };
    (plan, steps)
}

/// Whether a session started in `cwd` holds the task's folder: it is there, or in a folder below.
fn in_task(task: &Task, cwd: &str) -> bool {
    projects::is_inside(cwd, &task.path)
}

/// How many sessions with a running process are in the task's folder.
pub fn running_sessions(st: &AppState, task: &Task) -> u32 {
    let rows = st.registry().sessions_where(|cwd| in_task(task, cwd));
    let tabs = rows.iter().filter(|s| s.state == SessionState::Running).count() as u32;
    // The shell panel has no registry row, but its shell holds the folder the same way.
    tabs + u32::from(st.panel_running_where(|cwd| in_task(task, cwd)))
}

/// Closes every session in the task's folder, the shell panel's included, and waits for their
/// processes to die so they no longer hold the folder.
fn close_sessions(st: &AppState, task: &Task) {
    // The rows go first, so a dying session's exit callback finds nothing to update.
    let mut tabs = st.registry().drop_where(|cwd| in_task(task, cwd));
    tabs.extend(st.take_panel_where(|cwd| in_task(task, cwd)));
    for tab in tabs {
        if let Some(session) = st.remove_session(&tab) {
            // Blocking (we are on a blocking thread), unlike the async kill.
            session.force_kill();
        }
    }
}

/// Windows lets go of a folder a just-killed process held a moment later, so the worktree step
/// is tried again.
const REMOVE_RETRIES: u32 = 5;
const REMOVE_RETRY_WAIT: std::time::Duration = std::time::Duration::from_millis(200);

/// Runs the removal steps and forgets the task. The task is forgotten only when every step worked.
/// The sessions are closed already. The caller holds `MERGE_LOCK`; `TASKS_LOCK` is taken only for
/// the last write, not while git runs.
fn run_removal(st: &AppState, task: &Task, steps: &[GitStep]) -> Result<(), String> {
    let leftover = |reason: String| {
        if Path::new(&task.path).exists() {
            format!("{reason} The folder {} is left on disk.", task.path)
        } else {
            reason
        }
    };
    for step in steps {
        let step = std::slice::from_ref(step);
        let args: Vec<&str> = step[0].args.iter().map(String::as_str).collect();
        if args.starts_with(&["worktree", "remove"]) {
            // The first failure is the one that says why; after a half-done removal git's later
            // answers are about the leftovers ("is not a working tree").
            let mut first_failure = None;
            for attempt in 0..=REMOVE_RETRIES {
                if attempt > 0 {
                    std::thread::sleep(REMOVE_RETRY_WAIT);
                }
                match git::run_steps(step) {
                    Ok(()) => {
                        first_failure = None;
                        break;
                    }
                    Err(f) => {
                        first_failure.get_or_insert(f);
                    }
                }
            }
            if let Some(f) = first_failure {
                return Err(leftover(format!(
                    "Couldn't remove the worktree. Git said: {}",
                    git::said(&f.output)
                )));
            }
        } else if args.starts_with(&["worktree", "prune"]) {
            git::run_steps(step).map_err(|f| {
                leftover(format!(
                    "Couldn't clear git's record of the worktree. Git said: {}",
                    git::said(&f.output)
                ))
            })?;
        } else {
            git::run_steps(step).map_err(|f| {
                leftover(format!(
                    "Couldn't delete the branch {}. Git said: {}",
                    task.branch,
                    git::said(&f.output)
                ))
            })?;
        }
    }
    let _guard = TASKS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut doc = store::read::<FileDoc>(&file(st.home()))
        .map_err(|e| format!("The task was removed, but the task list couldn't be read: {e:#}"))?
        .unwrap_or_default();
    doc.tasks.retain(|t| t.id != task.id);
    store::write(&file(st.home()), &doc)
        .map_err(|e| format!("The task was removed, but the task list couldn't be saved: {e:#}"))
}

/// Removes the task: closes its sessions, deletes its worktree and branch, and forgets it. Plans
/// again first and stops when the commands differ from the ones the user was shown.
pub fn remove(st: &AppState, task: &Task, req: &RemoveRequest) -> Result<(), String> {
    let _guard = MERGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if !own_branch(task) {
        return Err(OWN_BRANCH_ONLY.into());
    }
    let (plan, _) = plan_remove(task, running_sessions(st, task));
    if plan.commands != req.commands || plan.warnings != req.warnings {
        return Err(CHANGED.into());
    }
    close_sessions(st, task);
    // Closing the sessions can change what the folder holds. The user saw the plan with the
    // "Closes N sessions" line, and that line is the only one allowed to be gone now. When this
    // refuses, the sessions are already closed.
    let (plan, steps) = plan_remove(task, 0);
    let seen: Vec<&String> = req.warnings.iter().filter(|w| !is_sessions_line(w)).collect();
    if plan.commands != req.commands || plan.warnings.iter().ne(seen) {
        return Err(CHANGED.into());
    }
    run_removal(st, task, &steps)
}

/// The "Closes N session(s) running in this worktree." warning.
fn is_sessions_line(warning: &str) -> bool {
    warning.starts_with("Closes ") && warning.ends_with(" running in this worktree.")
}
