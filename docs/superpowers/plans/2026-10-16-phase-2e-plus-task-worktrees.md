# Slice 2e+: projects and task worktrees — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A session can start as a *task* in its own git worktree. Marshell shows each git command before running it, lets you review the task's changes against its base branch, merge them back as one squashed commit, and remove the worktree. The sidebar groups sessions by git repository.

**Architecture:** The core runs `git` as a child process (`std::process::Command`, no new crates) through one small runner. A task registry, `~/.marshell/tasks.json`, records each task: its repo, base branch, branch and folder. Every git operation is built first as a list of `GitStep`s. The same list is returned as display strings for the confirm sheet ("plan") and then executed ("run"), so what the user saw is exactly what runs. The UI gets a worktree chip and task-name field in the launcher, and a Review sheet that holds the changes, Merge back and Remove.

**Tech Stack:** Rust (axum, tokio `spawn_blocking`, serde, ts-rs bindings), React + TypeScript, Vitest, Cargo integration tests with `tempfile` and real `git`.

**Spec:** `docs/PLAN.md:768` (slice 2e+), `:305` (launcher), `:274` (chip sentence), `:330` (review to merge); `docs/BRIEF.md:53`. The user decisions of 2026-10-16 below are binding and are written into PLAN by Task 10.

## User decisions (2026-10-16)

1. Marshell runs git itself (`git worktree add -b …`) and shows every command first. This works for any CLI and for plain shells, and no CLI's own `--worktree` flag is used.
2. Worktrees live at `~/.marshell/worktrees/<repo-name>/<task-name>` (under the core's home dir), never inside the repo.
3. **Merge back:**
   - Uncommitted changes in the worktree are committed there first.
   - Then `git merge --squash <branch>` runs in the base checkout, followed by one commit on the base branch with a message the user can edit.
4. **Project:** a project is the git repository root. Subfolders and task worktrees group under their main repo, and a folder outside git is its own project.
5. **Conflicts:**
   - Before merging, check that the base checkout has no tracked changes.
   - On a conflict, undo the merge so the base is left as it was, and list the conflicting files.
   - Offer to type a prompt asking the task's agent to fix them.
6. After a merge, a checkbox "Remove the worktree and branch afterwards" is ticked by default, and the removal commands are shown alongside the merge commands.
7. **Task name:**
   - It is a slug of the first prompt, editable before launch.
   - The branch is `marshell/<name>`.
   - With no usable prompt the name is `task-<n>`.
8. **Review:** a "Review changes" sheet opened from a task tab's sidebar row, the palette, or Ctrl+Shift+R. It holds the changed files, the diff against the base, and Merge back and Remove.

## Global Constraints

- Keep the Rust simple: plain functions, `std::process::Command`, no new crates (AGENTS.md "Do not add dependencies…").
- Never run a git command the user has not seen. Every command appears in a sheet before it runs. Plan and run share one builder, and a test asserts they match.
- Never write inside the user's repo except through the shown git commands. No `.gitignore` edits and no config edits.
- Git child processes on Windows get `CREATE_NO_WINDOW` (`0x0800_0000`), so no console window flashes.
- The environment is `GIT_TERMINAL_PROMPT=0`, and each call gets `-c core.quotepath=off` so paths come back unescaped.
- Errors show the reason in one sentence. A failed git call adds git's last non-empty stderr line after "Git said: ".
- The vendor-neutral rule holds: no task code branches on which agent runs. The only agent-specific value is the agent's display name in "Ask <Agent> to fix them", taken from the existing `CLOSE_BAR_NAMES`/`AGENT_NAMES`.
- UI copy is sentence case, and the exact strings are listed in each task. Everything is keyboard-reachable, with no colour-only signals (PLAN UX rules).
- Windows 11 is the only tested platform. Paths are built with `Path::join`, never with string `/`.
- Tests must not depend on the developer's global git identity. Test repos set `user.name`/`user.email` with `git -C <repo> config` inside the temp repo.

## Review Focus

1. **Paths with spaces** (the user's home is `C:\Users\Home Office`). Commands must run and display correctly. Tested in Tasks 1 and 2 with a temp dir whose name contains a space.
2. **A running shell that holds the worktree folder on Windows.** Remove must close the task's sessions first, then retry `worktree remove` and succeed. Tested in Task 5.
3. **Untracked files in the base checkout.** They must not block a merge. If git refuses because a merge would overwrite one, the base is restored and git's reason is shown. Tested in Task 4.
4. **A first prompt with no ASCII letters** (Hebrew, emoji). It must give `task-<n>`, never an empty or invalid branch. Tested in Task 2 (Rust) and Task 6 (TS).
5. **A commit that fails** (a pre-commit hook in the base repo exits 1). The merge reports git's reason and the base checkout is left clean. Tested in Task 4.

---

## File structure

**Rust (core):**
- Create `crates/core/src/git.rs` — `GitStep`, `display`, `run`, `run_steps`; the only place that spawns git.
- Create `crates/core/src/tasks.rs` — the task registry (`tasks.json`), naming, and create/changes/merge/remove plans and runs.
- Modify `crates/core/src/projects.rs` — add `repo_root(dir)`; `record` stores the repo root for a task worktree.
- Create `crates/core/src/server/tasks.rs` — HTTP handlers.
- Modify `crates/core/src/server/mod.rs` (routes), `crates/core/src/server/sessions.rs` (input endpoint), `crates/core/src/registry.rs` (the `project` field), and `crates/core/src/lib.rs` (`mod git; mod tasks;`).
- Modify `crates/protocol/src/api.rs` — the new types below and `SessionInfo.project`.
- Tests: `crates/core/tests/git_runner.rs`, `crates/core/tests/tasks.rs`.

**Frontend:**
- Create `src/lib/api/tasks.ts` (client), `src/features/tasks/taskName.ts` (slug), `src/features/tasks/taskOf.ts` (session → task), `src/features/tasks/splitDiff.ts`.
- Create `src/features/tasks/CommandList.tsx` (+ css) — the shown commands.
- Create `src/features/tasks/ReviewSheet.tsx` (+ css, test, stories), `src/features/tasks/reviewState.ts` (+ test) — a pure reducer for the sheet's steps.
- Modify the launcher (`Launcher.tsx`, `launchChoice.ts`, `LiveLauncher.tsx`, `launchPrograms.ts`), `toRow.ts`, `grouping.ts`, `src/features/sidebar/types.ts`, `SessionRow.tsx`, `src/lib/keymap.ts`, `LiveApp.tsx`, and `LivePalette.tsx`.

## Protocol types (Task 2 adds them; later tasks use them exactly)

```rust
// crates/protocol/src/api.rs — all derive Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS; #[ts(export)]
pub struct Task { pub id: String, pub name: String, pub branch: String, pub base_branch: String,
                  pub repo_root: String, pub path: String, pub created_ms: u64 }
pub struct TaskPlanRequest { pub cwd: String, pub name: String }          // name: raw text or prompt; core slugs it
pub struct TaskPlan { pub name: String, pub branch: String, pub base_branch: String,
                      pub repo_root: String, pub path: String, pub commands: Vec<String> }
pub struct ChangedFile { pub path: String, pub status: String }           // "added" | "modified" | "deleted" | "renamed" | "untracked"
pub struct TaskChanges { pub files: Vec<ChangedFile>, pub diff: String, pub truncated: bool,
                         pub ahead: u32, pub uncommitted: bool }
pub struct MergeRequest { pub message: String, pub remove: bool }
pub struct MergePlan { pub commands: Vec<String>, pub blocker: Option<String> }
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MergeOutcome { Merged { removed: bool }, Conflict { files: Vec<String> }, Failed { reason: String } }
pub struct RemovePlan { pub commands: Vec<String>, pub warnings: Vec<String>, pub sessions: u32 }
pub struct InputRequest { pub text: String }
// SessionInfo gains: pub project: String   // the git repo root of cwd, or cwd itself outside git
```

## HTTP routes (all under the existing auth guard)

| Route | Body → Response | Task |
|---|---|---|
| `GET /v1/tasks` | → `Vec<Task>` | 2 |
| `POST /v1/tasks/plan` | `TaskPlanRequest` → `TaskPlan` · 409 `{reason}` | 2 |
| `POST /v1/tasks` | `TaskPlanRequest` → `Task` · 409 `{reason}` | 2 |
| `GET /v1/tasks/{id}/changes` | → `TaskChanges` · 404 | 3 |
| `POST /v1/tasks/{id}/merge/plan` | `MergeRequest` → `MergePlan` | 4 |
| `POST /v1/tasks/{id}/merge` | `MergeRequest` → `MergeOutcome` | 4 |
| `POST /v1/sessions/{tab}/input` | `InputRequest` → 204 · 400 when the text has `\r`/`\n` · 404 | 4 |
| `POST /v1/tasks/{id}/remove/plan` | → `RemovePlan` | 5 |
| `POST /v1/tasks/{id}/remove` | → 204 · 409 `{reason}` | 5 |

The 409 body reuses the existing `ResumeError { reason }` shape. Its name stays as it is.

---

### Task 1: The git runner

**Files:**
- Create: `crates/core/src/git.rs`; modify `crates/core/src/lib.rs` (`pub mod git;`)
- Test: `crates/core/tests/git_runner.rs`

**Interfaces:**
- Produces:
  - `pub struct GitStep { pub dir: PathBuf, pub args: Vec<String> }`
  - `pub fn display(step: &GitStep) -> String`
  - `pub struct GitOutput { pub ok: bool, pub stdout: String, pub stderr: String }`
  - `pub fn run(dir: &Path, args: &[&str]) -> anyhow::Result<GitOutput>`. `Err` only when git can't be started, with the message `Git isn't installed, or isn't on PATH.`
  - `pub fn run_steps(steps: &[GitStep]) -> Result<(), StepFailure>`, where `pub struct StepFailure { pub index: usize, pub output: GitOutput }`. It stops at the first non-ok step.
  - `pub fn last_line(stderr: &str) -> &str` returns the last non-empty trimmed line.

- [ ] **Step 1: Write the failing tests** in `crates/core/tests/git_runner.rs`:
  - `display_quotes_paths_with_spaces`: `GitStep{dir:"C:\\Users\\Home Office\\app", args:["worktree","add","-b","marshell/x","C:\\a b\\x","main"]}` displays exactly as `git -C "C:\Users\Home Office\app" worktree add -b marshell/x "C:\a b\x" main`. An arg is wrapped in `"` when it is empty or contains a space, a tab or `"`, and an inner `"` becomes `\"`. The `-c core.quotepath=off` the runner adds is **not** displayed.
  - `run_reports_status_and_output`: in a `tempfile::tempdir()` whose path ends in `"with space"`:
    - `run(dir, &["init","-q","-b","main"])` gives `ok`;
    - `run(dir, &["rev-parse","--abbrev-ref","HEAD"])` gives stdout `"HEAD\n"` or `"main\n"` (empty repo);
    - `run(dir, &["checkout","nope"])` gives `!ok` with a non-empty `last_line(stderr)`.
  - `run_steps_stops_at_first_failure`: three steps where the second fails gives `Err(StepFailure{index:1,..})`, and a marker the third step would create does not exist.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test git_runner`. Expected: FAIL to compile (`marshell_core::git` not found).
- [ ] **Step 3: Implement** `git.rs`.
  - Each call is `Command::new("git")` with `-C <dir>`, `-c core.quotepath=off` and the args, plus `env("GIT_TERMINAL_PROMPT","0")` and stdin set to null.
  - On Windows add `creation_flags(0x0800_0000)` via `std::os::windows::process::CommandExt`.
  - Decode output with `String::from_utf8_lossy`.
  - A spawn error of kind `NotFound` maps to the `Git isn't installed…` message.
  - These functions are blocking. Callers in handlers wrap them in `tokio::task::spawn_blocking`.
- [ ] **Step 4: Run** `cargo test -p marshell-core --test git_runner`. Expected: 3 passed.
- [ ] **Step 5: Commit** `feat(core): a git runner that shows each command before it runs`.

### Task 2: Repo roots, the task registry and task creation

**Files:**
- Modify: `crates/protocol/src/api.rs` (every type in "Protocol types"), `crates/core/src/projects.rs`, `crates/core/src/registry.rs`, `src/features/sessions/toRow.ts`, `src/features/sidebar/types.ts`, `src/features/sidebar/grouping.ts`
- Create: `crates/core/src/tasks.rs`, `crates/core/src/server/tasks.rs`; routes in `server/mod.rs`
- Regenerate bindings (`pnpm check:generated` must pass)
- Test: `crates/core/tests/tasks.rs`, `src/features/sessions/toRow.test.ts`, `src/features/sidebar/grouping.test.ts`

**Interfaces:**
- Consumes: Task 1's `GitStep`, `display`, `run`, `run_steps`, `last_line`.
- Produces:
  - `projects::repo_root(dir: &Path) -> Option<PathBuf>`. This is the **main** checkout's root. For a linked worktree it follows the `.git` file to its `gitdir`, reads that dir's `commondir` file (resolved against that gitdir), and returns the common dir's parent. It reads files only and never runs git.
  - `tasks::slug(text: &str) -> String`, which may be empty.
  - `tasks::list(home) -> Vec<Task>` and `tasks::find(home, id) -> Option<Task>`.
  - `tasks::plan_create(home, cwd: &Path, name: &str, now_ms) -> Result<(TaskPlan, Vec<GitStep>), String>` and `tasks::create(home, req, now_ms) -> Result<Task, String>`.
  - `SessionInfo.project` and `RowModel.projectKey: string`.

- [ ] **Step 1: Write the failing Rust tests** in `crates/core/tests/tasks.rs`. A helper `repo(dir)` runs `init -q -b main`, `config user.name T`, `config user.email t@example.invalid` and one commit of `a.txt`. Use a temp dir containing a space for both the repo and the home.
  - `slug_rules`:
    - `slug("Fix the login redirect, please!")` is `"fix-the-login-redirect-please"`.
    - `slug("  --A__b  ")` is `"a-b"`.
    - `slug("תקן את הכפתור 🙂")` is `""`.
    - A 60-character prompt gives at most 40 characters, cut at the last `-` at or before 40 when one exists, with no trailing `-`.
    - The rules: lowercase ASCII; every run of non-`[a-z0-9]` becomes one `-`; trim `-`.
  - `plan_names_branch_folder_and_command`:
    - For `plan_create(home, repo, "Fix login", t)`:
      - `name == "fix-login"`, `branch == "marshell/fix-login"` and `base_branch == "main"`;
      - `path == home/worktrees/<repo dir name>/fix-login`;
      - `commands == [display(git -C <repo> worktree add -b marshell/fix-login <path> main)]`.
    - From a **subfolder** of the repo, `repo_root` is still the repo.
  - `plan_falls_back_and_deduplicates`:
    - An empty slug gives `task-1`.
    - With a task `task-1` already in the registry, the next is `task-2`.
    - When branch `marshell/fix-login` already exists, or the folder exists, the name becomes `fix-login-2`.
  - `plan_refuses_outside_git_and_detached`:
    - A plain temp dir gives `Err("<dir name> isn't a git repository, so it can't have a task worktree.")`.
    - After `checkout -q --detach`, it gives `Err("<repo name> is on a detached HEAD. Check out a branch first.")`.
  - `create_makes_the_worktree_and_records_it`:
    - `create` gives a `Task`, and its `path` exists with `a.txt` in it.
    - `git -C path rev-parse --abbrev-ref HEAD` gives `marshell/fix-login`.
    - `list(home)` contains it, and `projects::repo_root(path)` is the repo.
  - `create_runs_exactly_the_planned_steps`: the `Vec<GitStep>` `plan_create` returns, rendered with `display`, equals `TaskPlan.commands`; `create` executes that same Vec. Structure `create` as `let (plan, steps) = plan_create(..)?; run_steps(&steps)…`, and assert on the plan output.
  - `session_info_carries_project`: start a session (fake-agent profile as in `tests/shell_panel.rs`) with cwd = a task's path. Its `SessionInfo.project` is the main repo path, and with cwd = a non-git temp dir it is that dir.
  - `failed_create_records_nothing`: pre-create the branch so the step fails mid-way (bypass dedupe by calling the run with the stale plan). `create` gives `Err` starting `"Couldn't create the worktree. Git said: "`, and `list(home)` is empty.
- [ ] **Step 2: Write the failing TS tests:**
  - `toRow.test.ts`: `toRow(info with cwd "C:\\w\\app\\src", project "C:\\w\\app")` gives `project === "app"` and `projectKey === "C:\\w\\app"`.
  - `grouping.test.ts`: two rows whose `project` is `"app"` but whose `projectKey` differ form **two** groups, and the label stays `"app"`.
- [ ] **Step 3: Run** `cargo test -p marshell-core --test tasks` and `pnpm test -- toRow grouping`. Expected: compile failure and assertion failures.
- [ ] **Step 4: Implement.**
  - **`tasks.rs`:**
    - It uses a `Versioned` `FileDoc { tasks: Vec<Task> }` in `home/tasks.json` (pattern: `projects.rs`) with a `static TASKS_LOCK: Mutex<()>`.
    - `id` is 8 hex characters from `getrandom`.
    - The worktree folder name is the task name.
    - The repo-name folder is `repo_root.file_name()`.
    - The base branch is `projects::git_branch(&repo_root)`. A detached HEAD shows as a 7-hex string, so detect it as `HEAD` not starting with `ref:`: make `git_branch`'s detached branch distinguishable with a new `pub fn head_branch(dir) -> Option<String>` that returns `None` when detached, and keep `git_branch` as it is.
    - Branch existence is checked with `git rev-parse --verify --quiet refs/heads/<branch>` (not shown, read-only).
  - **Registry:** `SessionInfo.project` is computed where `SessionInfo` is built (`registry.rs` `reserve` and `seed_ended`) as `repo_root(cwd).unwrap_or(cwd)`, as a string.
  - **`projects::record`:** for a cwd under `home/worktrees`, record its `repo_root` instead.
  - **Handlers:**
    - They run inside `spawn_blocking`.
    - `plan` and `create` errors map to 409 with `{reason}`.
    - `GET /v1/tasks` also drops tasks whose `path` no longer exists, from the response only.
  - **TS:**
    - `RowModel` gains `projectKey: string`.
    - `toRow` sets `project: lastSegment(info.project)` and `projectKey: info.project`.
    - `groupRows` keys on `projectKey` and `Group.project` stays the label.
    - Update fixtures with `projectKey` equal to their `project`.
- [ ] **Step 5: Run** `cargo test --workspace`, `pnpm test` and `pnpm check:generated`. Expected: all pass.
- [ ] **Step 6: Commit** `feat(tasks): create a task worktree, and group sessions by repository`.

### Task 3: A task's changes

**Files:** modify `crates/core/src/tasks.rs` and `server/tasks.rs`; test `crates/core/tests/tasks.rs`.

**Interfaces:**
- Consumes: `tasks::find`, `git::run`.
- Produces: `tasks::changes(task: &Task) -> Result<TaskChanges, String>`.

- [ ] **Step 1: Write failing tests.**
  - `changes_cover_committed_uncommitted_and_untracked`: in a created task:
    - commit an edit to `a.txt`;
    - leave an unstaged edit to a new tracked file `b.txt` (add and commit it first, then edit);
    - create untracked `new dir/c.txt` with two lines.

    `changes()` then gives:
    - `files` sorted by path: `a.txt` modified, `b.txt` modified (or added), `new dir/c.txt` untracked;
    - `ahead == 2`, `uncommitted == true`, `truncated == false`;
    - `diff` containing `+++ b/new dir/c.txt` and `@@ -0,0 +1,2 @@`.
  - `changes_of_a_clean_task_are_empty`: `files` is empty, `diff` is `""`, and `ahead` is 0.
  - `binary_untracked_file_is_named_not_dumped`: an untracked file holding a NUL byte appears in `files`, and `diff` has the line `Binary file <path>` and none of its bytes.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test tasks changes`. Expected: FAIL.
- [ ] **Step 3: Implement.** All of these are read-only git calls and are never shown.
  - `mb = git merge-base <base_branch> HEAD` (in `task.path`).
  - `diff = git diff --no-color --no-ext-diff <mb>` (working tree against the merge base).
  - Statuses come from `git diff --name-status -z <mb>`, with A, M, D and R as `added`, `modified`, `deleted` and `renamed`. For `R` the new path is used.
  - Untracked files come from `git ls-files --others --exclude-standard -z`. Each is appended as `diff --git a/P b/P\nnew file mode 100644\n--- /dev/null\n+++ b/P\n@@ -0,0 +1,N @@\n` plus its lines prefixed `+`. A file is binary when its first 8 KiB holds a NUL, and a file over 256 KiB gets only the `Binary file`-style line `Large file <path>`.
  - `ahead = git rev-list --count <base_branch>..HEAD`.
  - `uncommitted` is true when `git status --porcelain` is non-empty.
  - Cap the total `diff` at 4 MiB, cutting at a line boundary, and set `truncated`.
  - The handler returns 404 for an unknown id.
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(tasks): list a task's changes against its base`.

### Task 4: Merge back, conflicts, and typing into a session

**Files:** modify `tasks.rs`, `server/tasks.rs`, `server/sessions.rs` (input) and `server/mod.rs`; test `crates/core/tests/tasks.rs`.

**Interfaces:**
- Consumes: Tasks 1–3; `AppState` session lookup (as `remove_session` and `resume` use it) and the session's pty writer (as `pty_ws.rs` writes input).
- Produces:
  - `tasks::plan_merge(task, req: &MergeRequest) -> (MergePlan, Vec<GitStep>)`, where the steps are the merge steps only and the removal steps come from Task 5 when `req.remove`;
  - `tasks::merge(st: &AppState, task: &Task, req: &MergeRequest) -> MergeOutcome`, which takes `AppState` because Task 5's `remove` needs the session registry;
  - `POST /v1/sessions/{tab}/input`.

- [ ] **Step 1: Write failing tests.**
  - `merge_plan_lists_commands_in_order`. With an uncommitted edit in the worktree and message `Fix login`, `commands` is exactly:
    1. `git -C "<path>" add -A`
    2. `git -C "<path>" commit -q -m "Fix login"`
    3. `git -C "<repo>" merge --squash marshell/fix-login`
    4. `git -C "<repo>" commit -q -m "Fix login"`

    With a clean worktree, steps 1–2 are absent. With `remove: true`, Task 5's removal commands follow; in this task assert only that `remove:false` has 4 entries.
  - `merge_plan_blockers`. `blocker` is one of these, and the commands are still returned:
    - a tracked edit in the base checkout gives `"<repo name> has uncommitted changes. Commit or stash them, then merge."`;
    - the base on another branch gives `"<repo name> is on <other>, not main. Switch it to main, then merge."`;
    - no commits ahead and a clean worktree give `"marshell/fix-login has no changes to merge."`;
    - an empty message after trim gives `"Write a commit message."`.
  - `merge_squashes_into_one_commit`:
    - The result is `Merged{removed:false}`.
    - The base log gained exactly one commit with subject `Fix login`, and `a.txt` on main has the task's content.
    - `git status --porcelain` in the base is empty.
  - `untracked_files_in_base_do_not_block`: an untracked `notes.txt` in the base does not block, and the merge succeeds.
  - `untracked_file_in_the_way_restores_base` (Review Focus 3):
    - The task adds `n.txt`, and the base has an untracked `n.txt`.
    - The result is `Failed{reason}`, where `reason` starts with `"Couldn't merge. Git said: "`.
    - The base's `git status --porcelain --untracked-files=no` is empty, and HEAD is unchanged.
  - `conflict_is_undone_and_listed`:
    - Edit the same line of `a.txt` on main and in the task.
    - The result is `Conflict{files:["a.txt"]}`.
    - The base HEAD is unchanged, `git status --porcelain --untracked-files=no` is empty, and `.git/MERGE_MSG`/`SQUASH_MSG` don't affect cleanliness.
  - `failing_commit_hook_restores_base` (Review Focus 5): write `.git/hooks/pre-commit` as `#!/bin/sh\nexit 1\n` in the base. The result is `Failed{reason}`, where `reason` starts with `"Couldn't commit on main. Git said: "`, and the base is clean with HEAD unchanged. This needs a POSIX `sh`; Git for Windows ships one.
  - `input_types_text_without_enter`:
    - POST `/v1/sessions/{tab}/input` with `{"text":"hello"}` to a fake-agent session (use `fake-agent echo` if it exists, else check `crates/core/src/bin`) gives 204, and `hello` reaches the pty.
    - `{"text":"a\nb"}` gives 400 with the body `The text can't contain a line break.`
    - An unknown tab gives 404.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test tasks merge input`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `merge` re-plans and refuses with `Failed{reason: blocker}` when a blocker is present. It then runs the worktree steps; a failure gives `Failed{"Couldn't commit the task's changes. Git said: …"}`.
  - It then runs `merge --squash`. On failure:
    - collect `git diff --name-only --diff-filter=U` in the base;
    - run `git reset -q --merge`, which is not a shown command and only restores what the shown command changed;
    - the result is `Conflict{files}` when the list is non-empty, else `Failed{"Couldn't merge. Git said: …"}`.
  - It then runs the commit. On failure it runs `git reset -q --merge` and gives `Failed{"Couldn't commit on <base>. Git said: …"}`.
  - "Dirty" means `git status --porcelain --untracked-files=no` is non-empty.
  - `merge` with `remove: true` then calls Task 5's `remove` (in this task leave a `// Task 5` call site that returns `removed:false`). This task's tests use `remove:false` only.
  - The input endpoint writes the UTF-8 bytes to the session's pty as-is, never adding `\r`.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(tasks): merge a task back as one commit, undoing conflicts`.

### Task 5: Remove a task

**Files:** modify `tasks.rs`, `server/tasks.rs` and `registry.rs` (dropping a tab by id); test `crates/core/tests/tasks.rs`.

**Interfaces:**
- Consumes: Tasks 1–4; the session registry (live and Ended rows), with the session kill path `close` uses.
- Produces:
  - `tasks::plan_remove(task, open_sessions: u32) -> (RemovePlan, Vec<GitStep>)`;
  - `tasks::remove(st: &AppState, task) -> Result<(), String>`;
  - `plan_merge` with `remove: true` appends `plan_remove`'s commands;
  - `merge` gives `Merged{removed:true}` after a successful removal.

- [ ] **Step 1: Write failing tests.**
  - `remove_plan_commands_and_warnings`:
    - For a clean, fully merged task, `commands` is exactly `git -C "<repo>" worktree remove "<path>"` and `git -C "<repo>" branch -D marshell/fix-login`, and `warnings` is empty.
    - When the worktree is dirty, the first command is `worktree remove --force "<path>"` and the warnings include `"The worktree has uncommitted changes. Removing deletes them."`.
    - For an unmerged task with 2 commits not reachable from the base, the warnings include `"marshell/fix-login has 2 commits that aren't on main. Removing deletes them."` (1 gives `commit`/`isn't`).
    - "Merged" for a squash means the base's tree contains the change. Count `git rev-list --count main..marshell/fix-login` and treat the task as merged when `git diff --quiet main marshell/fix-login` exits 0.
    - `sessions` gives `"Closes 1 session running in this worktree."` in `warnings`, pluralised for 2.
  - `remove_closes_sessions_and_deletes_everything` (Review Focus 2):
    - Start a fake-agent session with cwd = the task path, then remove.
    - The session is gone from the snapshot (neither open nor Ended), the folder is gone, the branch is gone, and `list(home)` no longer has the task.
    - On Windows a held folder makes the first `worktree remove` fail, so `remove` retries up to 5 times, 200 ms apart, after the sessions are killed.
  - `merge_with_remove_cleans_up`: the result is `Merged{removed:true}`, and the folder and branch are gone.
  - `remove_failure_keeps_the_task`: make the folder impossible to remove (on Windows keep a file open with `std::fs::File` in the test for the whole call). The result is `Err` starting `"Couldn't remove the worktree. Git said: "`, and the task is still in `list(home)`.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test tasks remove`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Sessions belong to the task when `same_path(cwd, task.path)`. Move `same_path` to `pub(crate)` in `projects.rs`, or make it `pub`.
  - Kill live ones as `close` does, and drop their rows, Ended rows included.
  - Run the steps and `git worktree prune` (not shown, housekeeping only).
  - Drop the task from `tasks.json` only after the steps succeed.
  - The `remove` handler maps `Err` to 409 `{reason}`.
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(tasks): remove a task's worktree and branch`.

### Task 6: Starting a task from the launcher

**Files:**
- Create: `src/lib/api/tasks.ts`, `src/features/tasks/taskName.ts` (+ test), `src/features/tasks/CommandList.tsx` (+ css)
- Modify: `src/features/launcher/Launcher.tsx`, `launchChoice.ts`, `src/features/sessions/LiveLauncher.tsx`, `launchPrograms.ts`
- Test: `taskName.test.ts`, `Launcher.test.tsx`, `launchPrograms.test.ts`

**Interfaces:**
- Consumes: `POST /v1/tasks/plan` and `POST /v1/tasks`; `ProjectInfo.branch` (non-null means the folder is in git).
- Produces:
  - `slugify(text: string): string`, the same rules as Rust `slug` (Task 2), with the same test vectors;
  - `tasksApi = { list, plan, create, changes, mergePlan, merge, removePlan, remove }` in `src/lib/api/tasks.ts`, following `src/lib/api/sessions.ts`;
  - `<CommandList commands={string[]} />`.

- [ ] **Step 1: Write failing tests.**
  - `taskName.test.ts`: the four `slug_rules` vectors from Task 2, and the frontend fallback `slugify("🙂") === ""`. The core picks `task-<n>`.
  - `Launcher.test.tsx`:
    - (a) The worktree chip is shown in the live launcher only when the chosen project has a `branch`. Remove the "hidden until phases 3 and 6" gate for this one chip; the other chips stay hidden.
    - (b) Turning it on shows a text field labelled `Task name`, pre-filled with `slugify(prompt)`, that follows the prompt until the user edits the field.
    - (c) Below the field, a hint reads `Branch marshell/<name>`, or `Branch marshell/task-…` when the name is empty.
  - `launchPrograms.test.ts`: with `worktree: true`, the launch goes through a confirm step. `launchRequest` is called with `cwd = task.path` only after `tasksApi.create` resolves. With `worktree: false` nothing changes.
  - `CommandList`: it renders each command as its own `<code>` in an `<ol>` labelled `Commands Marshell will run`, and a 2-command list has 2 items.
- [ ] **Step 2: Run** `pnpm test -- taskName Launcher launchPrograms CommandList`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - **Starting:** with the worktree chip on, Enter in the launcher calls `tasksApi.plan({cwd, name})` and swaps the launcher body for a confirm view.
    - The view has the title `Start a task in a new worktree`, the `CommandList`, and the buttons `Create and start` (default, Enter) and `Back` (Esc goes back to the sentence, keeping the choices).
    - A plan 409 shows its `reason` in place of the commands, and `Create and start` is disabled.
    - `Create and start` calls `tasksApi.create`, shows `Creating the worktree…` while pending, and then launches the session exactly as today with `cwd = task.path`.
    - A create failure shows the reason in the view, announced once via the existing live region pattern.
  - **Launcher memory:** remembered choices (T12 of 2e) never store the worktree flag. The chip starts off every time.
  - **Look:** follow the existing launcher tokens; `frontend-design` decides the confirm view's look within the 560 px sheet.
- [ ] **Step 4: Run** `pnpm test`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(launcher): start a task in its own worktree`.

### Task 7: The Review sheet: changes

**Files:**
- Create: `src/features/tasks/taskOf.ts` (+ test), `splitDiff.ts` (+ test), `reviewState.ts` (+ test), `ReviewSheet.tsx`, `ReviewSheet.css`, `ReviewSheet.test.tsx`, `ReviewSheet.stories.tsx`
- Uses: `src/components/DiffView`, `src/lib/useDialogFocus.ts` (the Appearance dialog's pattern)

**Interfaces:**
- Consumes: `tasksApi.list` and `tasksApi.changes`; `parseUnifiedDiff`.
- Produces:
  - `taskOf(info: SessionInfo, tasks: Task[]): Task | undefined`. It matches `info.cwd` to `task.path`, ignoring case and a trailing separator on Windows, like `same_path`.
  - `splitDiff(diff: string): { path: string; diff: string }[]`. It splits at each `diff --git a/… b/…` line and takes the path from the `b/` side.
  - `reviewReducer(state: ReviewState, action: ReviewAction): ReviewState`, with `type ReviewStep = "changes" | "merge" | "merging" | "conflict" | "remove" | "removing" | "done"`.
  - `<ReviewSheet task changes agentName onClose onMerge onRemove onAskAgent />`, as one component whose body follows the step.

- [ ] **Step 1: Write failing tests.**
  - `taskOf`: `C:\Users\Home Office\.marshell\worktrees\app\x\` matches a task path without the trailing `\`, case-insensitively on Windows.
  - `splitDiff` on a two-file diff gives two entries with the right paths, and a path with a space (`new dir/c.txt`) survives.
  - `ReviewSheet`:
    - The title is `Changes in marshell/fix-login`, with the subtitle `Against main · 2 commits ahead · uncommitted changes` (omit the parts that are zero or false).
    - The file list is a listbox. Each option shows a status word (`Added`/`Modified`/`Deleted`/`Renamed`/`New, not committed`) as text, not just colour, and the path. ArrowUp/Down move, and the selected file's diff shows in `DiffView` with `expanded`.
    - `truncated` shows `The diff is too large to show in full.`
    - An empty `files` shows the empty state `No changes yet.`, and Merge back is disabled.
    - The buttons are `Merge back…` and `Remove…`. Esc closes, and focus returns to the opener (`useDialogFocus`).
- [ ] **Step 2: Run** `pnpm test -- taskOf splitDiff reviewState ReviewSheet`. Expected: FAIL.
- [ ] **Step 3: Implement** the changes step and the reducer's `changes`/`merge`/`remove` transitions. The merge and remove step bodies are Task 8. `frontend-design` decides the look; reuse the sheet styling of the Appearance dialog and the DiffView tokens.
- [ ] **Step 4: Run** `pnpm test`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(review): a sheet with a task's changes`.

### Task 8: The Review sheet: merge back, conflicts and remove

**Files:** modify `ReviewSheet.tsx`/`.css`/`.test.tsx` and `reviewState.ts`/`.test.ts`.

**Interfaces:**
- Consumes: `tasksApi.mergePlan`, `merge`, `removePlan` and `remove`; `POST /v1/sessions/{tab}/input`; `CommandList`.
- Produces: the `onMerge(message, remove)`, `onRemove()` and `onAskAgent(text)` callbacks, wired in Task 9.

- [ ] **Step 1: Write failing tests** for `ReviewSheet` and the reducer.
  - **Merge step:**
    - It has a `Commit message` field pre-filled with the task name made readable (`fix-login` → `Fix login`: hyphens become spaces, the first letter uppercase).
    - It has the checkbox `Remove the worktree and branch afterwards`, checked.
    - It shows a `CommandList` from `mergePlan`, refetched (debounced 250 ms) when the message or the checkbox changes.
    - The `Merge` button is disabled while a `blocker` is shown, with the blocker text above the commands.
    - Back returns to the changes step.
  - **Merging:** `Merging…` is shown, announced once.
  - **`Merged`:** the sheet closes and the toast reads `Merged marshell/fix-login into main.`, plus ` Removed the worktree.` when `removed`.
  - **`Conflict{files}`:**
    - The heading reads `Merging stopped: 1 file conflicts with main. Nothing on main was changed.` (2 gives `files conflict`), followed by the file list.
    - The buttons are `Ask Claude Code to fix them`, using the agent name, and `Close`. The ask button is shown only when the task's session is a running agent.
    - Activating it calls `onAskAgent` with exactly ``Merging marshell/fix-login into main conflicts in a.txt. Rebase this branch on main and resolve those conflicts, then commit.`` (the files joined with `, `). The sheet then closes and focus goes to that session's terminal.
  - **`Failed{reason}`:** the reason is shown in the merge step and the Merge button is enabled again.
  - **Remove step:** the `CommandList` from `removePlan`, its `warnings` as a list, and the button `Remove`, labelled `Remove anyway` when there are warnings. The success toast is `Removed marshell/fix-login.`
- [ ] **Step 2: Run** `pnpm test -- ReviewSheet reviewState`. Expected: FAIL.
- [ ] **Step 3: Implement.** The conflict prompt contains no line break, so the core accepts it. The user presses Enter in the agent; Marshell never sends Enter (PLAN phase 2.5 rule: fill the prompt line, the user sends).
- [ ] **Step 4: Run** `pnpm test`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(review): merge back or remove a task, and hand conflicts to its agent`.

### Task 9: Wiring: shortcut, palette, sidebar row

**Files:** modify `src/lib/keymap.ts`, `LiveApp.tsx`, `LivePalette.tsx`, `SessionRow.tsx` (+ tests), `src/lib/bindings.test.ts` or `keymap.test.ts`.

**Interfaces:**
- Consumes: Tasks 6–8.
- Produces: the command `{ id: "review-task", label: "Review this task's changes", group: "Sessions", binding: { code: "KeyR" } }`.

- [ ] **Step 1: Write failing tests.**
  - `keymap.test.ts`: `review-task` is bound to `KeyR`, and the existing duplicate/reserved checker reports no problem on windows, mac or linux.
  - `LiveApp.test.tsx`:
    - With the active session in a task, Ctrl+Shift+R opens the Review sheet, and its keydown has `defaultPrevented === true` so the webview's hard reload never runs.
    - With a non-task session, Ctrl+Shift+R does nothing and announces `This session isn't a task.`
    - LiveApp fetches `tasksApi.list` on start and after every create, merge or remove.
  - `LivePalette`: `Review this task's changes` is in `hidden` unless the active session is a task.
  - `SessionRow`:
    - A task row shows a small `Task` text tag after the project name, with the branch available as `title`/accessible description.
    - It has a `Review changes` button reachable by keyboard (with the row's other actions), shown only on task rows.
- [ ] **Step 2: Run** `pnpm test -- keymap LiveApp LivePalette SessionRow`. Expected: FAIL.
- [ ] **Step 3: Implement.** Task-ness is `taskOf(info, tasks)`, and LiveApp keeps `tasks` in state. The cheat sheet picks the new command up from `COMMANDS` automatically; assert that it lists it.
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: all pass, clean build.
- [ ] **Step 5: Commit** `feat(window): open a task's review from its row, the palette or Ctrl+Shift+R`.

### Task 10: Docs

**Files:** `docs/PLAN.md` (§9 2e+ bullet; the keymap audit table gets a row `Review task | (none) | **R** · ⌘R | Decided 2026-10-16; the only free letter left. Its keydown is always prevented, so it never reloads the webview.`), `docs/BRIEF.md` (an amendment line).

- [ ] **Step 1:** In PLAN §9, under the 2e+ bullet (line 768), add "Slice 2e+ decisions (2026-10-16)" with the eight user decisions above, one line each. Also add the rulings any task records, and the routes table in short form.
- [ ] **Step 2:** In BRIEF amendments, add one line: "2026-10-16: task worktrees are created by Marshell's own git commands (shown first), live under `~/.marshell/worktrees`, and merge back as one squashed commit; projects are git repository roots."
- [ ] **Step 3:** Run `pnpm test` (docs-only change; this confirms nothing else moved). Expected: all pass.
- [ ] **Step 4: Commit** `docs: slice 2e+ task worktrees in PLAN and BRIEF`.

---

## Self-review notes

- **Spec coverage.** The PLAN items map to tasks like this:
  - create → Tasks 2 and 6;
  - review the diff → Tasks 3 and 7;
  - merge back → Tasks 4 and 8;
  - remove → Tasks 5 and 8;
  - every git command shown first → Tasks 1, 2, 4, 5, 6 and 8, plus the plan-equals-run test;
  - projects grouped by repo → Task 2.

  Start-from-issue and per-file comments to the agent stay in v2 (PLAN:330, 881).
- **Out of scope:**
  - opening a PR through `gh`;
  - checkpoints;
  - `marshell.json` setup scripts;
  - rebasing for the user;
  - a CLI's own `--worktree` flag.
- **Read-only git calls that are not shown:** `merge-base`, `diff`, `status`, `rev-list`, `rev-parse --verify`, `ls-files` and `worktree prune`. So are the `reset --merge` restores after a failed shown command, which only undo it. Task 10 documents this rule.
