# Slice 6u: Changes leftovers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a Changes check cheap on a big checkout (issue #176), remove git sandbox folders that are no longer needed (issue #177), and see the 6r-3 review fixes in the running app.

**Architecture:** The Changes check (`crates/core/src/changes/`) keeps its link safety check's last clear answer for 10 s when that check was slow, and the walk for new files tells the ignore rules what its own folder listing showed. A new module `crates/core/src/gitbox/sweep.rs` owns everything that deletes under `<home>/git-sandbox`: a start-up sweep, a task's sandbox on removal, and all of them on uninstall. Each sandbox names its checkout in a file, and only a folder whose name is the hash of the path its file names is ever removed.

**Tech Stack:** Rust (`marshell-core`, `marshell-cli`), PowerShell for the release smoke script. No new dependency.

**Spec:** `docs/PLAN.md` §9, "Slice 6u decisions (2026-11-23)", decisions 1 to 5. Measurements: `docs/research/2026-11-23-changes-big-checkout.md`. Issues #176, #177, #178.

## Global Constraints

- Rust tests: `cargo test -p marshell-core -j 4` (never more jobs). Run `cargo fmt --all` before every commit; `cargo clippy --workspace --all-targets -j 4 -- -D warnings` must stay clean.
- Tests use a temp folder as the Marshell home, never the real `~/.marshell`. No test reaches the network: a "network" path in a test is text only (`//server/share/x`), or the device form `//?/C:/marshell-no-such-folder`.
- No new dependency. No change to `guard.rs`, token handling, or file permissions under the home.
- Deleting a folder under the home happens only in `crates/core/src/gitbox/sweep.rs`, and only through its `ours` test (Task 4). No other code removes a folder under `<home>/git-sandbox`.
- Constants, exact: `SLOW_REACH` 250 ms, `SLOW_REACH_EVERY` 10 s, `UNUSED_AFTER` 7 days, `TEMP_AFTER` 1 day, `TOUCH_EVERY` 1 hour, the file name `marshell-checkout`.
- Copy, exact: "The folder <path> (the Changes panel's copies of git's index) will be removed."
- Commit messages read `type(scope): what changed, as a sentence.` and end with the session's attribution lines. Workers never run `git add`, `commit`, `checkout`, `restore`, `reset`, `stash` or `clean`; the main session commits. Files keep LF line endings.
- A hook runs rustfmt on every edited Rust file: read the file again before the next edit.

## Review Focus

1. A second Marshell on the same home, or `marshell uninstall` while the app runs, removes a sandbox that is in use: the next check must rebuild it, not fail for good. Test in Task 4 (`a_sandbox_removed_under_a_live_box_is_rebuilt_by_refresh`).
2. A folder under `git-sandbox` that is a junction or link, with a valid file behind it: it must not be removed or followed. Test in Task 4 (`a_link_named_like_a_sandbox_is_left`).
3. A checkout path that can't be looked at for a reason other than "not found" (a bad name, a drive that answers "not ready"): the sandbox stays until the 7-day rule. A drive whose letter is gone answers "not found", so its sandbox goes; it is rebuilt on the next use. Test in Task 4 (`a_checkout_that_cannot_be_looked_at_is_kept`).
4. The clock moved back, so a sandbox's file is dated in the future: it counts as fresh. Test in Task 4 (`a_file_dated_in_the_future_is_fresh`).
5. The index changes while a slow link check's answer stands: the next check must look again, at once. Test in Task 3 (`a_slow_clear_look_stands_until_the_index_changes`).

---

### Task 1: The decisions, the measurements and the timing test (done while planning)

**Files:**
- Modify: `docs/PLAN.md` (§9: "Slice 6u decisions (2026-11-23)", and the fifth review's limits under 6r-3 decision 22)
- Create: `docs/research/2026-11-23-changes-big-checkout.md`
- Modify: `crates/core/src/changes/mod.rs` (tests: `timing_on_a_big_checkout`, `#[ignore]`)

- [ ] **Step 1:** Run `cargo test -p marshell-core -j 4 --lib changes::` and expect `0 failed`.
- [ ] **Step 2:** Commit the three files with this plan: `docs(changes): slice 6u's decisions, the big-checkout measurements and the plan.`

### Task 2: The walk tells the ignore rules what its listing showed (decision 4)

**Files:**
- Modify: `crates/core/src/server/ignores.rs` (`Ignores::listed`, `Walk::folder`; the code is in the tree from the measurement). `Walk::folder` calls `listed` only when the listing gave every entry: one it couldn't read may be a `.gitignore` or a `.git`.
- Test: the same file's `tests` module

**Interfaces:**
- Produces: `fn listed(&mut self, folder: &Path, has_rules: bool, clear: bool)` on `Ignores` (private to the module). `Walk::folder` calls it once per folder it lists, with `has_rules` = the listing holds a name equal to `.gitignore` without ASCII case, and `clear` = not the start folder and the listing holds no name equal to `.git` without ASCII case.

- [ ] **Step 1: Write the tests**

```rust
/// The walk's listing answers what the rules would ask the disk: no `.gitignore` here, no `.git` here.
#[test]
fn the_walk_hands_its_listing_to_the_rules() {
    let (_tmp, root) = repo();
    write(&root.join("a").join("x.txt"), "x");
    write(&root.join("b").join(".gitignore"), "*.log\n");
    write(&root.join("b").join("y.log"), "y");
    write(&root.join("b").join("z.txt"), "z");
    let mut ig = rules(&root);
    assert_eq!(listed(&mut ig, &root), ["a/x.txt", "b/.gitignore", "b/z.txt"]);
    // Three folders listed, and the disk asked once more: for the one `.gitignore` there is.
    assert_eq!(ig.asks, 1);
    assert!(matches!(ig.folders.get(&root.join("b")), Some(Some(_))));
}

/// Windows opens `.GitIgnore` as `.gitignore`, so a listing that holds it is no "no".
#[cfg(windows)]
#[test]
fn a_gitignore_in_another_case_still_applies_in_the_walk() {
    let (_tmp, root) = repo();
    write(&root.join("sub").join(".GitIgnore"), "*.log\n");
    write(&root.join("sub").join("a.log"), "a");
    write(&root.join("sub").join("b.txt"), "b");
    let mut ig = rules(&root);
    assert_eq!(listed(&mut ig, &root), ["sub/.GitIgnore", "sub/b.txt"]);
}

/// A `.GIT` folder is a `.git` to Windows: the folder that holds it is not marked clear.
#[cfg(windows)]
#[test]
fn a_git_folder_in_another_case_is_not_taken_as_clear() {
    let (_tmp, root) = repo();
    std::fs::create_dir_all(root.join("sub").join(".GIT")).unwrap();
    write(&root.join("sub").join("a.txt"), "a");
    let mut ig = rules(&root);
    let _ = listed(&mut ig, &root);
    assert!(ig.below_top.get(&root.join("sub")) == Some(&Folder::Nested));
}
```

  `Ignores` gets a test-only counter, `#[cfg(test)] asks: usize`, raised by `count_ask()` each time `folder_rules` or `folder_state` goes to the disk for a folder (the pattern of `GitBox::count_run`). What the listing hands over is by design the same state the rules would have worked out, so only the count tells the two apart.

- [ ] **Step 2: See the first test fail without the call.** Comment out the `self.ignores.listed(...)` line in `Walk::folder`, run `cargo test -p marshell-core -j 4 --lib server::ignores::tests::the_walk_hands_its`, expect FAIL with `left: 5, right: 1`; put the line back.
- [ ] **Step 3: Run** `cargo test -p marshell-core -j 4 --lib -- server::ignores server::files server::search server::tree changes::` and expect `0 failed`.
- [ ] **Step 4: Commit** `perf(files): the walk for new files asks the disk once per folder.`

### Task 3: A slow link safety check stands for 10 s (decision 1)

**Files:**
- Modify: `crates/core/src/changes/cache.rs` (`Reached`, `Entry::reached`, the two constants)
- Modify: `crates/core/src/changes/mod.rs` (`looked`, its call in `compute`)
- Test: both files' `tests` modules

**Interfaces:**
- Produces, in `cache.rs`:

```rust
/// A link safety check that took longer than this stands for `SLOW_REACH_EVERY`.
const SLOW_REACH: Duration = Duration::from_millis(250);
const SLOW_REACH_EVERY: Duration = Duration::from_secs(10);

/// The link safety check's last clear answer: the tracked paths it looked at, when it started and how long it took.
#[derive(Default)]
pub(super) struct Reached {
    pub of: Option<Arc<Tracked>>,
    pub at: Option<Instant>,
    pub took: Duration,
}

impl Reached {
    /// Whether the last clear answer still stands for `tracked`: it took over `SLOW_REACH`, started less than
    /// `SLOW_REACH_EVERY` ago, and looked at these very paths (`Arc::ptr_eq`: a new index is a new `Arc`).
    pub(super) fn stands(&self, tracked: &Arc<Tracked>) -> bool;
    /// `stands` with the time given, so a test moves the clock forward and never subtracts from an `Instant`.
    fn stands_at(&self, tracked: &Arc<Tracked>, now: Instant) -> bool;
    /// Keeps a clear answer for `tracked` from a look that started at `started` and ends now.
    pub(super) fn set(&mut self, tracked: &Arc<Tracked>, started: Instant);
}
```

  `Entry` gets `pub reached: Reached`.
- Produces, in `mod.rs`: `fn looked(checkout: &Path, tracked: &Arc<Tracked>, reached: &mut Reached, deadline: Instant, cancel: &AtomicBool) -> Reach`. It returns `Reach::Clear` without looking when `reached.stands(tracked)`; otherwise it runs `worktree::reach`, calls `reached.set` on `Clear`, and sets `*reached = Reached::default()` on anything else. `compute` calls `looked` where it called `worktree::reach`, and its comment says why a slow answer may stand (PLAN §9, slice 6u decision 1).

- [ ] **Step 1: Write the failing tests**

In `cache.rs`, in a new `#[cfg(test)] mod tests` (the file has none):

```rust
#[test]
fn a_clear_look_stands_only_when_it_was_slow_young_and_of_these_paths() {
    let tracked = Arc::new(Tracked::default());
    // Asked about a time in the future, so the test needs nothing of how long the PC has been up.
    let look = |took_ms: u64, age: Duration| Reached {
        of: Some(tracked.clone()),
        at: Some(Instant::now()),
        took: Duration::from_millis(took_ms),
    }
    .stands_at(&tracked, Instant::now() + age);
    assert!(look(251, Duration::from_secs(9)));
    assert!(!look(250, Duration::from_secs(1)), "a fast look runs on every check");
    assert!(!look(251, Duration::from_secs(11)));
    let slow = Reached { of: Some(tracked.clone()), at: Some(Instant::now()), took: Duration::from_millis(251) };
    assert!(slow.stands(&tracked));
    assert!(!slow.stands(&Arc::new(Tracked::default())), "other paths, even equal ones");
    assert!(!Reached::default().stands(&tracked));
}
```

In `mod.rs` tests (a gitlink path with no folder is clear; a `.git` file that names a device path is refused, as in `a_submodule_whose_git_file_names_a_network_path_is_refused`):

```rust
#[test]
fn a_slow_clear_look_stands_until_the_index_changes() {
    let scratch = Scratch::new();
    let never = AtomicBool::new(false);
    let tracked = Arc::new(Tracked::parse(b"vendor/lib\0a.txt\0"));
    let dot_git = scratch.repo.join("vendor").join("lib").join(".git");
    let mut reached = cache::Reached::default();
    let look = |tracked: &Arc<Tracked>, reached: &mut cache::Reached| looked(&scratch.repo, tracked, reached, far(), &never);

    assert_eq!(look(&tracked, &mut reached), Reach::Clear);
    // A fast look is not kept: the refused folder is seen on the next check.
    write(&dot_git, "gitdir: //?/C:/marshell-no-such-folder\n");
    assert_eq!(look(&tracked, &mut reached), Reach::Refused);
    assert!(reached.of.is_none(), "a refusal is never kept");

    std::fs::remove_file(&dot_git).unwrap();
    assert_eq!(look(&tracked, &mut reached), Reach::Clear);
    reached.took = Duration::from_millis(251);
    write(&dot_git, "gitdir: //?/C:/marshell-no-such-folder\n");
    assert_eq!(look(&tracked, &mut reached), Reach::Clear, "a slow clear look stands");

    // A new index is a new set of paths: looked at again at once.
    let again = Arc::new(Tracked::parse(b"vendor/lib\0a.txt\0"));
    assert_eq!(look(&again, &mut reached), Reach::Refused);
}
```

- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib changes::` and expect both to fail to compile (`Reached`, `looked` not found).
- [ ] **Step 3:** Implement `Reached`, `Entry::reached` and `looked`, and call `looked` from `compute` (destructure `reached` from `entry` beside `walk`).
- [ ] **Step 4:** Run `cargo test -p marshell-core -j 4 --lib changes::` and expect `0 failed`.
- [ ] **Step 5: Commit** `perf(changes): a link safety check that took over 250 ms stands for 10 s while the index is unchanged.`

### Task 4: Sandboxes name their checkout, and a sweep removes the ones not needed (decision 2)

**Files:**
- Create: `crates/core/src/gitbox/sweep.rs`
- Modify: `crates/core/src/gitbox/mod.rs` (`pub mod sweep;`, `pub fn sandbox_dir`, the file written in `reload`)
- Modify: `crates/core/src/server/mod.rs` (`serve`: one call after `state.start_storage();`)
- Modify: `crates/core/src/tasks.rs` (`run_removal`: one call)
- Test: `sweep.rs`, `gitbox/mod.rs` and `tasks.rs` test modules

**Interfaces:**
- Produces, in `gitbox/mod.rs`: `pub fn sandbox_dir(home: &Path, checkout: &Path) -> PathBuf` (`git_sandbox_dir(home)`, made absolute, joined with `sandbox_name` of the absolute `checkout`); `GitBox::open` uses it. `sandbox_name` becomes `pub(super)`.
- Produces, in `sweep.rs`:

```rust
/// The file in a sandbox that holds its checkout's path, as UTF-8 with no line break.
pub const OWNER_FILE: &str = "marshell-checkout";
pub const UNUSED_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const TEMP_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
pub(super) const TOUCH_EVERY: Duration = Duration::from_secs(60 * 60);
/// Held shared while a sandbox is written (`GitBox::reload`), and alone for each folder a sweep removes.
pub(super) static GATE: RwLock<()> = RwLock::new(());

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Swept { pub removed: usize, pub kept: usize }

/// Removes each sandbox whose checkout is gone or whose file is older than `UNUSED_AFTER` at `now`, and the
/// temp files older than `TEMP_AFTER` in the ones kept, in their `info` folders and in `git-sandbox` itself.
pub fn sweep(home: &Path, now: SystemTime) -> Swept;
/// Removes the sandbox of `checkout`, when there is one. True when one was removed.
pub fn forget(home: &Path, checkout: &Path) -> bool;
/// Removes every sandbox, then `empty-config` and the temp files in `git-sandbox` itself whatever their age,
/// and `git-sandbox` when it is then empty. The number of sandboxes removed. For uninstall.
pub fn remove_all(home: &Path) -> usize;
/// Whether `remove_all` would remove something: `git-sandbox` holds a sandbox `ours` accepts, or
/// `empty-config`. For uninstall's question.
pub fn any(home: &Path) -> bool;
/// Runs `sweep` once, off the async threads, and logs how many it removed when that isn't zero.
pub fn spawn(home: PathBuf);
```

**The rules, exact:**
- `ours(dir, name) -> Option<(PathBuf, SystemTime)>` is the one test of what may be removed. It is `Some((checkout, modified))` only when: `name` is 16 characters, each `0-9a-f`; `symlink_metadata(dir)` says a folder and not a link; `symlink_metadata(dir/OWNER_FILE)` says a regular file of at most 32 KiB; its bytes are UTF-8; and `sandbox_name(Path::new(text)) == name`. `modified` is that file's modified time.
- `gone(checkout)`: the path is absolute, `crate::server::refused(text)` is false, `crate::server::links_refused(checkout)` is false, and `symlink_metadata(checkout)` fails with `ErrorKind::NotFound`. Anything else is not gone.
- Old: `now.duration_since(modified)` is `Ok(d)` with `d > UNUSED_AFTER`. A time in the future is not old.
- One removal, used by `sweep`, `forget` and `remove_all`: take `GATE.try_write()` (a poisoned gate is used as it is; a busy gate means the folder is kept, or `forget` answers false); under it run `ours` again, and for the 7-day case test the age again with the modified time it gives now, so a sandbox opened since the first look stays; then `std::fs::remove_dir_all`. When that fails and the folder is still there, write `OWNER_FILE` back with the same text and modified time, count the folder as kept and `tracing::warn!`. `gone` is worked out before the gate is taken: it asks the disk.
- Nothing that runs under `GATE` takes it again or calls into `sweep.rs`.
- As built: a failed removal also deletes what is left of `index`, so the next check copies the index and its shared part again; the temp-file pass takes the gate too (a temp file is dated as its source while it is written); `ours` refuses any reparse point, and nothing is done when `git-sandbox` itself is a link. In tests the removal waits for the gate up to 60 s, because the tests share one process and some hold it for a whole check; `a_busy_gate_is_not_waited_for` pins the one try made outside tests. `sandbox_name` stays private (a child module sees it).
- A temp file is a regular file (by `symlink_metadata`) whose name starts with `.` and ends with `.tmp`. A sweep looks for them in each kept sandbox, in its `info` only when `symlink_metadata` says that is a folder and not a link, and in `git-sandbox` itself.
- `forget` removes `sandbox_dir(home, checkout)` when `ours` accepts it, whatever its age.
- `GitBox::reload` holds `GATE.read()` for all of its work (a poisoned gate is used as it is). After `write_static` it writes `OWNER_FILE` with `write_file` (so its modified time is now) when `symlink_metadata` says the file isn't there or isn't a regular file, or its modified time is older than `TOUCH_EVERY` or in the future. So a sandbox removed under a live box gets its file back on the next check. A checkout whose path isn't UTF-8 gets no file. This lock is held across file work and a `git config` run on purpose: it is a plain thread lock like `Slot::work` (`cache.rs`, the module comment), never held in async code.
- `serve` calls `crate::gitbox::sweep::spawn(state.home().to_path_buf())` right after `state.start_storage();`. `spawn` uses `tokio::task::spawn_blocking` and drops the handle.
- `run_removal` takes `let checkout = crate::projects::canonical(Path::new(&task.path));` before the steps loop (the folder can't be resolved once it is gone; Changes names its sandbox by the canonical path), and calls `crate::gitbox::sweep::forget(st.home(), &checkout)` after the loop and before `TASKS_LOCK` is taken. Its result is not used.

- [ ] **Step 1: Write the failing tests** in `sweep.rs`. A helper `fn sandbox(home: &Path, checkout: &Path, modified: SystemTime) -> PathBuf` makes `sandbox_dir(home, checkout)` with `HEAD` and an `OWNER_FILE` dated `modified` (`File::set_modified`) whose text is `std::path::absolute(checkout)`, the text `sandbox_dir` hashed (on Windows that turns `//server/share/repo` into `\\server\share\repo`). `home` and the checkouts are folders in one `tempfile::tempdir()`; `now` is `SystemTime::now()`. No test does arithmetic on `Instant` (a CI runner may have been up for less than an hour).

| Test | Setup | Assert |
| --- | --- | --- |
| `a_sandbox_whose_checkout_is_gone_is_removed` | sandbox for `<tmp>/gone` (never made), dated now; sandbox for `<tmp>/here` (made), dated now | `sweep` is `Swept { removed: 1, kept: 1 }`; the first folder is gone, the second stays |
| `a_sandbox_unused_for_seven_days_is_removed` | sandbox for `<tmp>/here`, dated now | `sweep(home, now + 6 days)` keeps it; `sweep(home, now + UNUSED_AFTER + 1 s)` removes it |
| `a_file_dated_in_the_future_is_fresh` | sandbox for `<tmp>/here`, dated now + 30 days | kept |
| `a_folder_that_is_not_ours_is_left` | a 16-hex folder with no file; one whose file names another path; a folder `notes`; a file named `0123456789abcdef`; a 16-hex folder whose name has an upper-case letter and whose file names a gone path | `Swept::default()`, all five still there |
| `a_link_named_like_a_sandbox_is_left` (`#[cfg(windows)]`, skip itself when `mklink /J` fails) | `<tmp>/real` holds an `OWNER_FILE` naming `<tmp>/gone`; a junction at `git-sandbox/<sandbox_name(<tmp>/gone)>` to `<tmp>/real` | `Swept::default()`; the junction and `<tmp>/real/marshell-checkout` still there |
| `a_checkout_on_the_network_is_never_looked_for` | sandbox for `//server/share/repo` (text only), dated now | kept; `sweep(home, now + UNUSED_AFTER + 1 s)` removes it |
| `a_checkout_that_cannot_be_looked_at_is_kept` (`#[cfg(windows)]`) | sandbox for `<tmp>\a<b` (a name Windows refuses), dated now | kept |
| `old_temp_files_go_and_new_ones_stay` | a kept sandbox with `.index.1-2.tmp` dated now − 2 days, `.index.3-4.tmp` dated now, `info/.attributes.5-6.tmp` dated now − 2 days, and `git-sandbox/.empty-config.7-8.tmp` dated now − 2 days; a file `.keep` dated now − 2 days | the three old `.tmp` files are gone; the new one and `.keep` stay |
| `temp_files_behind_a_link_are_left` (`#[cfg(windows)]`, skip itself when `mklink /J` fails) | a kept sandbox whose `info` is a junction to `<tmp>/elsewhere`, which holds `.x.tmp` dated now − 2 days | `<tmp>/elsewhere/.x.tmp` still there |
| `a_sandbox_opened_since_the_first_look_stays` | sandbox for `<tmp>/here` dated now − 8 days; the removal is asked for with that old time as the first look's, after the file was dated now again | kept (the test calls the one removal function directly with the stale first look) |
| `forget_removes_only_that_checkouts_sandbox` | sandboxes for `<tmp>/a` and `<tmp>/b` (both made) | `forget(home, a)` is true and only `a`'s is gone; `forget(home, a)` again is false |
| `remove_all_takes_every_sandbox_and_the_folder` | two sandboxes, `empty-config`, a new `.x.tmp` | `remove_all` is 2 and `git-sandbox` is gone; `any(home)` was true before and is false after |
| `remove_all_leaves_what_is_not_ours` | one sandbox and a file `notes.txt` in `git-sandbox` | `remove_all` is 1; `notes.txt` and `git-sandbox` stay; `any(home)` is then false |

In `gitbox/mod.rs` tests (they have a scratch repository and plain git already):

```rust
#[test]
fn open_names_the_checkout_in_the_sandbox() // OWNER_FILE's text is the box's checkout path, and sweep::sweep(home, now) keeps it

#[test]
fn a_sandbox_without_the_file_gets_one_when_it_is_opened() // remove the file, open again: it is back

#[test]
fn the_file_is_written_again_after_an_hour() // date the file 59 minutes back (set_modified), refresh(): the date is unchanged; date it 61 minutes back, refresh(): its modified time is within a minute of now; date it a day ahead, refresh(): within a minute of now again

#[test]
fn a_sandbox_removed_under_a_live_box_is_rebuilt_by_refresh() // sweep::remove_all(home) with the box alive, then refresh() is Ok and index, HEAD and OWNER_FILE are there
```

In `tasks.rs` tests, beside the existing removal tests: `removing_a_task_removes_its_sandbox`: `GitBox::open(home, task.path)` first, assert `sandbox_dir(home, task.path)` exists, remove the task as those tests do, assert it is gone.

- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 --lib -- gitbox:: tasks::` and expect the new tests to fail to compile.
- [ ] **Step 3:** Implement `sweep.rs` and the changes in `gitbox/mod.rs`, `server/mod.rs` and `tasks.rs` as the rules say.
- [ ] **Step 4:** Run `cargo test -p marshell-core -j 4 --lib -- gitbox:: tasks:: changes::` and expect `0 failed`; run `cargo clippy -p marshell-core --all-targets -j 4 -- -D warnings` and expect no warning.
- [ ] **Step 5: Commit** `feat(changes): a git sandbox names its checkout, and the ones not needed are removed at start and with a task's worktree.`

### Task 5: Uninstall removes the sandboxes (decision 3)

**Files:**
- Modify: `crates/cli/src/uninstall.rs` (`run`, a new `remove_sandboxes`)
- Modify: `crates/cli/examples/smoke-fixture.rs` (one sandbox in the home it is given)
- Modify: `scripts/release/smoke.ps1` (one check beside the A8 snapshots check)
- Test: `uninstall.rs` tests

**Interfaces:**
- Consumes: `marshell_core::gitbox::sweep::{any, remove_all, OWNER_FILE}`, `marshell_core::gitbox::sandbox_dir`, `marshell_core::paths::git_sandbox_dir`.

**The rules, exact:**
- `let has_sandbox = sweep::any(home);` is read where `has_bin` is. It doesn't depend on `others_failed`: no hook points into `git-sandbox`.
- In both branches, before the question and whenever `has_sandbox` (whether or not the bin line is printed), `run` prints: `The folder {git-sandbox} (the Changes panel's copies of git's index) will be removed.` with `paths::git_sandbox_dir(home).display()`. Where the bin line is printed too, this line follows it.
- The branch with no edits returns early only when `!has_bin && !has_sandbox`. Otherwise it prints the line or lines that apply, asks once, then removes bin when `has_bin`, calls `keep_note` when not, and removes the sandboxes when `has_sandbox`.
- The branch with edits removes the sandboxes after a successful apply, when `has_sandbox`, whether or not bin stays. A refused preview or a failed apply removes nothing, as now.
- `remove_sandboxes(home, out)` calls `sweep::remove_all(home)`; when `git-sandbox` still exists afterwards it prints `The folder {path} was left in place: it still holds files that were not removed.`
- Exit codes keep their rules: `ok` after the work, 1 when the question is declined. A home with only sandboxes left is now asked about, so declining there gives 1 where the run used to end with `ok` unasked.
- The smoke fixture makes the folder `<home>/smoke-checkout` and `sandbox_dir(home, that folder)` with `HEAD` and an `OWNER_FILE` whose text is `std::path::absolute` of that folder. The folder must exist: the app starts once more between the fixture and the uninstall (A6), and its sweep would remove a sandbox whose checkout is gone, so the A8 check would prove nothing. `smoke.ps1` adds, right after the A6 start's checks: `Check 'A6 the fixture git sandbox is still there' { Test-Path -LiteralPath (Join-Path $MHome 'git-sandbox') }`, and right after the A8 snapshots check: `Check 'A8 after uninstall: the home git-sandbox folder is gone' { -not (Test-Path -LiteralPath (Join-Path $MHome 'git-sandbox')) }`.

- [ ] **Step 1: Write the failing tests** in `uninstall.rs`, using the module's `Fixture`, `run_with` and the way `yes_flag_applies_and_removes_bin` and `nothing_installed_still_offers_to_remove_bin` set up. A helper `fn sandbox(home: &Path) -> PathBuf` makes one as the smoke fixture does.

| Test | Setup | Assert |
| --- | --- | --- |
| `uninstall_removes_the_git_sandboxes` | as `yes_flag_applies_and_removes_bin`, plus a sandbox | exit 0; the output holds the copy line with the folder's path; `git-sandbox` is gone; `managed.json` and `snapshots` stay |
| `only_the_sandboxes_left_still_asks` | no hooks, no bin, a sandbox; input `y` | the question is asked; `git-sandbox` is gone; exit 0 |
| `declining_keeps_the_sandboxes` | the same, input `n` | "Nothing changed."; the sandbox stays; exit 1 |
| `the_sandboxes_go_even_when_bin_stays` | as `a_codex_file_that_does_not_parse_keeps_bin_even_with_no_claude_hooks`, plus a sandbox, `--yes` | bin stays, its note is printed, `git-sandbox` is gone, the exit code is what that test expects |
| `the_sandbox_line_comes_before_the_question_when_bin_stays` | as `a_codex_file_that_does_not_parse_does_not_stop_claudes_removal`, plus a sandbox, input `y` | the copy line is in the output before "Apply these changes?"; the bin line is not; `git-sandbox` is gone |
| `a_file_marshell_did_not_make_keeps_the_folder` | a sandbox and `git-sandbox/notes.txt`, `--yes` | the sandbox is gone, `notes.txt` stays, the output holds "was left in place: it still holds files that were not removed." |
| `a_folder_with_only_other_files_is_not_asked_about` | no hooks, no bin, `git-sandbox/notes.txt` only | no question, no copy line, `notes.txt` stays, exit 0 |

- [ ] **Step 2:** Run `cargo test -p marshell-cli -j 4 uninstall` and expect the seven to fail.
- [ ] **Step 3:** Implement the rules.
- [ ] **Step 4:** Run `cargo test -p marshell-cli -j 4` and expect `0 failed`; `cargo build -p marshell-cli --examples -j 4` builds.
- [ ] **Step 5: Commit** `feat(cli): uninstall removes the Changes panel's git sandboxes.`

### Task 6: The numbers after, and the live check

**Files:**
- Modify: `docs/research/2026-11-23-changes-big-checkout.md` (a section "After slice 6u")
- Create: `docs/research/2026-11-23-changes-leftovers-live-check.md`

- [ ] **Step 1:** Run `timing_on_a_big_checkout` in a release build against the 20,000-file and the 100,000-file checkout and add the six `check #n` times and the parts to the research file. Expected at 100,000 files: checks 2 to 6 near 0.35 s (the link check stands), where they were 1.3 s.
- [ ] **Step 2:** Live check with the `marshell-live-check` skill, shell sessions only, a scratch home and a scratch `USERPROFILE`. Rows:

| # | What | Pass when |
| --- | --- | --- |
| 1 | Start with a scratch home that holds a sandbox for a gone folder, one dated 8 days back and one for the session's checkout dated now | after start only the third is left, and Changes shows the checkout's diff |
| 2 | Changes on the 100,000-file checkout, one file edited | the diff shows within the first two polls, and no "Git took too long to read the changes." over a minute |
| 3 | Show Changes from the command palette | the keyboard is in the panel after the palette closes (6r-3 review round 3) |
| 4 | Type in a comment box, ask for a box on another line | the first text is a draft, "Comment kept as a draft." is announced, the new box has the keyboard (round 4) |
| 5 | Ask again for the box that is open | it keeps its text and takes the keyboard (round 4) |
| 6 | Close the last session | the Changes strip reads "Changes" (round 4) |
| 7 | Fold two files, commit one of them away, fold another | the saved layout holds folds only for files in the diff (round 3) |
| 8 | Open a box, `cd` the shell to another repository, save the box | the comment keeps its lines and its label names the first checkout (round 3) |
| 9 | Edit a comment, discard it from the Send bar while the edit is open, save the edit | the text comes back as a new comment (round 4) |
| 10 | `core.autocrlf = yes` in the scratch user config, a tracked CRLF file | Changes doesn't show it as wholly changed (round 3) |

- [ ] **Step 3:** Write the rows' results, with what was seen, in the live-check file. A row that fails is fixed with a failing test first, or goes to the user.
- [ ] **Step 4: Commit** `docs(changes): slice 6u's numbers after the change and its live check.`

### Task 7: Review and ship

- [ ] **Step 1:** Final whole-branch review by the `marshell-reviewer` agent on Opus (review package, this plan, the Review Focus above), then one fix pass.
- [ ] **Step 2:** `marshell-ship`: the checks CI runs, the PR (body: "What it does", "Live check", "Known limits", "Checks"; `Closes #176`, `Closes #177`), the pre-merge review, green CI including the installer dry run, the merge.
- [ ] **Step 3:** Delete the ledger, update memory, list the scratch folders for the user's OK.
