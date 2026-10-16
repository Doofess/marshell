//! Tasks: git worktrees that Marshell made, kept in `~/.marshell/tasks.json`.
//! Creating one runs a single git command, and the user sees that command before it runs.

use crate::git::{self, GitStep};
use crate::projects;
use crate::store::{self, Versioned};
use marshell_protocol::api::{ChangedFile, Task, TaskChanges, TaskPlan, TaskPlanRequest};
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
    git::run_steps(&steps).map_err(|failure| {
        format!(
            "Couldn't create the worktree. Git said: {}",
            git::last_line(&failure.output.stderr)
        )
    })?;

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
            git::last_line(&out.stderr)
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
