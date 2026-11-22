# Slice 6t: what 6r-2 left open — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close issues #167, #168 and #169: git never walks a folder link or runs in a folder learned from the shell, a folder the shell moved into still gets git's ignore rules, auto save never overwrites an agent's version without a copy, Files shows a loading ring while it waits, and the deferred minors are fixed.

**Architecture:** One rules engine in the core (`server/ignores.rs`, built on the `ignore` crate's `.gitignore` matcher) decides what git would ignore, by reading files only. The tree's marks, the file list behind Go to file and the name filter, and the content search all use it. Git runs only in the launch folder's checkout, only for what it reads from its index (`ls-files --cached`, `grep` over tracked files), always with a deadline. The frontend gets a centred ring in the Files tree area and small fixes in the tree, the pane and the editor.

**Tech Stack:** Rust (axum core, `ignore` crate 0.4), React 19 + TypeScript, Vitest (node, `renderToStaticMarkup`, source pins).

**Spec:** `docs/PLAN.md` §9, "Slice 6t decisions (2026-11-22)" 1 to 10, and issues #167, #168, #169. The 6r-2 plan (`docs/superpowers/plans/2026-11-21-phase-6r-2-file-tree.md`) says how the code it touches was built.

## Global Constraints

- Git never runs in a folder the shell reported (`Root::InCheckout`) or outside a checkout (`Root::Plain`). It may run only for `Root::Checkout`, the launch folder's checkout (6t decision 1; 6r-2 Final ruling, `files.rs` pin test).
- Every git run the Files panel starts has a deadline: the file list `LIST_BUDGET` (2 s, `files.rs:44`), the search `BUDGET` (3 s, `search.rs:29`), the tree's tracked list `TREE_GIT_BUDGET` = 2 s (new, `tree.rs`).
- No walk enters a link (symlink or junction), a `.git` folder, or a nested checkout (a folder below the root that holds `.git`).
- Reading a repo's rules never reaches the network: a `.gitignore` that is a link is not read (git 2.32 and later refuse it too), a `gitdir:` or `commondir` that names a network or device path, or one `files::links_refused` refuses, is not followed, and a pattern file over 1 MiB is skipped.
- Copy, exactly: "Listing files…", "Searching…", "Can't list the files in {folder}.", "Search for 200 characters or fewer.", "Collapsed all folders." (`…` is U+2026).
- The ring: 40 px, in the middle of the Files tree area, its line under it; shown only after 150 ms of waiting; no rotation under `prefers-reduced-motion: reduce` or `:root[data-motion="reduce"]`; system colours only in forced colours.
- New dependency: `ignore` 0.4 in `crates/core/Cargo.toml` only (MIT OR Unlicense, used under MIT), with an entry in `THIRD_PARTY_NOTICES.md` in the file's format (SCOPE.md dependency policy). The lockfile change goes in the same commit.
- Rust tests run `cargo test -p marshell-core -j 4` (never more jobs). `cargo fmt --all` before every Rust commit.
- Colours from `src/styles/` tokens only; good-css rules (logical properties, `oklch`/`color-mix`, motion inside `prefers-reduced-motion: no-preference`).
- Commit messages: `type(scope): sentence.` and end with the two attribution lines the session gives.

## Review Focus

1. A folder the shell moved into, inside a checkout whose `.git/config` sets `core.fsmonitor` to a program: listing, searching and polling the tree there start no git process. Pinned in Task 2 (the type), Tasks 3, 4 and 5 (injected runners that panic), and in the live check by a marker file the program would write and git's own trace.
2. A junction or symlink inside an untracked folder of the launch checkout, pointing at a big folder: neither the list nor the search reads anything through it, and the list is not slowed by it (Task 3 and Task 4 tests with a junction two levels down).
3. `.gitignore` edge rules: a negation (`*.log` then `!keep.log`), a deeper `.gitignore` overriding the root's, an ignored folder whose child a negation tries to bring back (git keeps it ignored), a folder-only pattern (`build/`) beside a file named `build`, and `.git/info/exclude` losing to any `.gitignore` (Task 1 tests).
4. Keyboard focus inside the tree's rows when the ring replaces them (the shell moved, so the name list is read again while the person is in the filtered rows): focus moves to the filter box and is never lost to the page (Task 8 test).
5. The first auto save after a restart, an auto save right after an agent's write, and two tabs saving the same file: each takes a copy only when the bytes it replaces aren't Marshell's last write there (Task 6 tests).

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/core/src/server/ignores.rs` (new) | `Ignores`, git's ignore rules for one checkout read from files; `walk_unignored`, the walk that lists what git would list. |
| `crates/core/src/server/files.rs` | `Root` and `project_root`; git's tracked list, kept until the index changes; the file list route (tracked from git, untracked from the walk). |
| `crates/core/src/server/search.rs` | The content search per `Root`; file links skipped. |
| `crates/core/src/git.rs` | A stdout read error ends `stream` as an error; `run_input` removed. |
| `crates/core/src/server/tree.rs` | Marks from `Ignores`, tracked files from git in a checkout only; `check-ignore` removed. |
| `crates/core/src/server/save.rs` | The last hash Marshell wrote per file; the auto-save copy rule. |
| `crates/core/src/layout.rs` | The saved tree root's length bound. |
| `src/features/files/FilesBusy.tsx`, `FilesBusy.css` (new) | The ring and its line. |
| `src/features/files/filesWait.ts` (new) | What Files is waiting for, and the 150 ms delay. |
| `src/features/files/filterText.ts`, `FilesPanel.tsx` | The long-search note, the waits, the failed-list note, Collapse all's announcement. |
| `src/features/files/treeModel.ts`, `FileTree.tsx`, `FileTree.css`, `reveal.ts`, `useTree.ts` | The note rows, the filter box edge, mixed slashes, the reveal race. |
| `src/features/editor/TextTab.tsx`, `src/features/pane/paneCommands.ts`, `src/features/sessions/LiveApp.tsx` | The marker scan, the tab limit for a preview, the closed session's tree save. |
| `docs/PLAN.md`, `docs/research/2026-11-22-files-leftovers-live-check.md` (new) | Decision 10's list, the platform note, the live check. |

---

### Task 1: The ignore rules engine

**Files:**
- Create: `crates/core/src/server/ignores.rs`
- Modify: `crates/core/src/server/mod.rs` (add `mod ignores;`), `crates/core/Cargo.toml`, `Cargo.lock`, `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Produces:
  - `pub(super) struct Ignores`, with:
    - `pub(super) fn new(checkout: &Path, top: &Path) -> Ignores`, which takes the user's global excludes file as below;
    - `pub(super) fn with_global(checkout: &Path, top: &Path, global: Option<&Path>) -> Ignores`, where tests pass `None` or a temp file;
    - `pub(super) fn ignored(&mut self, abs: &Path, is_dir: bool) -> bool`.
  - `pub(super) fn git_dir(checkout: &Path) -> Option<PathBuf>`: `<checkout>/.git` when it is a real folder, else the `gitdir:` its `.git` file names, taken relative to the checkout when it is relative (as a submodule writes it). Task 3 reads the index from it.
  - `pub(super) fn walk_unignored(ig: &mut Ignores, start: &Path, deadline: Instant, cancel: &AtomicBool, skip: &dyn Fn(&str) -> bool, each: &mut dyn FnMut(&str, bool) -> bool) -> crate::git::Stopped`.
    - It calls `each(rel, is_link)` for every file it keeps. `rel` is relative to `start`, `/`-separated, and each folder's entries come in sorted order.
    - `each` returning false stops it with `Stopped::Asked`. It returns `Deadline` or `Cancelled` when those stopped it, else `Done`.

`ignored` follows git, for `abs` below `top`; anything else is `false`. `top` is the checkout for the launch folder's checkout, and the reported folder for a folder the shell moved into. For each prefix of the path below `top` (its folders first, then the path itself), the deciding rules come in this order:
1. the `.gitignore` files from that prefix's parent folder up to the checkout, the deepest first;
2. `<common git dir>/info/exclude`;
3. the global file.

The first one with a say decides (`Gitignore::matched` returning `Ignore` or `Whitelist`; in one file the last matching line wins). The first ignored prefix makes the whole path ignored: git never brings back a child of an ignored folder.

Three more rules:
- `top` and the folders above it are never ignored themselves, but their `.gitignore` lines still apply below them. So a shell that moved into `node_modules/pkg` still lists it, as ripgrep started there does.
- A prefix below `top` that holds `.git` and isn't ignored ends the check: not ignored. Git doesn't judge a nested repo's files.
- A folder link counts as a folder for a `dir/` pattern, as git for Windows walks a junction like a folder.

Matching is case-insensitive on Windows and macOS (`GitignoreBuilder::case_insensitive(cfg!(any(windows, target_os = "macos")))`), where git sets `core.ignorecase` when it makes a repo.

Where the files come from:
- **Every `.git` probe** uses `symlink_metadata`. A `.git` that is a link is no checkout. A `.git` file is read with a 4 KiB cap.
- **The git dir:** the git dir is `git_dir(checkout)`, and the common git dir is what its `commondir` file names, if there is one. A `gitdir:` or `commondir` that names a network or device path, or one `files::links_refused` refuses, is not followed. Every pattern file under the git dir passes `links_refused` on its full path before it is read, so a linked `info` folder can't reach a share.
- **A `.gitignore` in the worktree** that is a link is skipped, as git 2.32 and later skip it.
- **The global file's path** comes from `ignore::gitignore::gitconfig_excludes_path()`, read through `links_refused`. The user's own file may be a local link, as dotfile managers make it.
  - Its lines are kept for the process, and read again when its length or modified time changes.
  - Its matcher, like `info/exclude`'s, is rooted at the checkout, so its `/`-patterns anchor there as git anchors them. Do not use `Gitignore::global()`: it roots the matcher at `""`.
- **Each pattern file** is read with a 1 MiB cap, its UTF-8 BOM dropped, decoded lossily, and fed to `GitignoreBuilder::add_line`.
- **Matchers:** one `Gitignore` is built per folder that holds a `.gitignore`, and cached in the `Ignores`. `matched` is always given the path relative to that matcher's own folder. Never use `matched_path_or_any_parents`, which panics on a path outside its root.

`walk_unignored` walks `start` top-down with `std::fs::read_dir` and `DirEntry::file_type()`. That costs no extra open per file, and `is_symlink()` is true for a junction (`files.rs:215`). It spots `.git` and `.gitignore` in the folder's own listing, and leaves out:
- folder links, which are never entered;
- `.git`, and a folder holding `.git` (a nested checkout);
- anything `ignored`;
- any file whose relative path `skip` names.

A file link goes to `each` with `is_link = true`: the list keeps it through `is_listable`, as `walk_files` does today, and the search skips it. Hidden files are listed, as git lists them.

- [ ] **Step 1: Add the dependency**

Run: `cargo add ignore@0.4 -p marshell-core`
Expected: `Adding ignore v0.4.x to dependencies`; `Cargo.lock` gains `ignore`, `globset`, `bstr`, `crossbeam-deque`, `crossbeam-epoch` (6t decision 12 approved the crate and its notice).

- [ ] **Step 2: Write the failing tests in `ignores.rs`'s `mod tests`**

Each test builds a temp folder with an empty `.git` folder (no git is run) and uses `Ignores::with_global(&root, &root, None)` unless it says otherwise. A test that needs a link skips itself when the OS refuses to make one; make a junction as `tree.rs`'s `a_junction_inside_the_root_is_never_entered` does.

```rust
#[test] fn a_pattern_in_the_root_gitignore_ignores_a_file_and_a_folder()      // ".gitignore" = "*.log\ndist/\n": a.log, dist ignored; a.txt not
#[test] fn a_negation_keeps_a_file_the_pattern_above_it_ignores()             // "*.log\n!keep.log\n": keep.log not ignored, a.log ignored
#[test] fn a_deeper_gitignore_overrides_the_root()                            // root "*.tmp", sub/.gitignore "!a.tmp": sub/a.tmp not ignored, b.tmp ignored
#[test] fn nothing_under_an_ignored_folder_comes_back()                       // "dist/\n!dist/keep.txt\n": dist/keep.txt still ignored
#[test] fn a_folder_only_pattern_leaves_a_file_of_that_name()                 // "build/": file `build` not ignored, folder `build` ignored
#[test] fn info_exclude_counts_after_every_gitignore()                        // .git/info/exclude "*.x"; sub/.gitignore "!a.x": sub/a.x not ignored, b.x ignored
#[test] fn the_global_file_counts_last_and_anchors_at_the_checkout()          // global "*.g\n/top.h\n": a.g and top.h ignored, sub/top.h not; ".gitignore" "!a.g" wins over it
#[test] fn a_worktree_reads_info_exclude_from_the_common_git_dir()            // `.git` file "gitdir: <tmp>/main/.git/worktrees/w", commondir "../..", exclude in main/.git/info
#[test] fn a_submodules_relative_gitdir_is_read()                             // sub/.git file "gitdir: ../.git/modules/sub", exclude "*.x" there: with checkout = sub, a.x ignored
#[test] fn a_path_outside_the_checkout_is_not_ignored()
#[test] fn files_in_a_nested_checkout_are_not_judged()                        // root "*.log", sub/.git folder, sub/a.log: not ignored
#[cfg(any(windows, target_os = "macos"))] #[test] fn patterns_match_without_case_where_git_does() // "thumbs.db\nbin/": Thumbs.db and folder Bin ignored
#[cfg(windows)] #[test] fn a_folder_link_counts_as_a_folder_for_a_folder_pattern() // junction node_modules → a folder, "node_modules/": ignored
#[test] fn a_bom_at_the_start_of_a_gitignore_is_dropped()                     // "\u{feff}*.log\n": a.log ignored
#[test] fn a_gitignore_that_is_a_link_is_not_read()                           // sub/.gitignore a symlink to a file holding "*": sub/a.txt not ignored
#[test] fn a_dot_git_that_is_a_link_is_not_a_checkout()                       // `.git` a junction/symlink to a real git dir: git_dir None, its info/exclude unread
#[test] fn a_gitdir_naming_a_network_path_is_not_followed()                   // `.git` file "gitdir: //server/share/x": git_dir None, nothing ignored, no panic
#[test] fn a_pattern_file_over_1_mib_is_skipped()                             // ".gitignore" of 1 MiB + 1 byte whose first line is "*.log": a.log not ignored
#[test] fn the_walk_lists_what_git_would_list_and_dotfiles()                  // .github/ci.yml, .env, src/a.rs listed; a.log, dist/x not
#[test] fn the_walk_never_enters_a_link_dot_git_or_a_nested_checkout()        // junction (Windows) / symlink (Unix) to a folder of files, sub/.git: none listed
#[test] fn the_walk_hands_a_file_link_over_as_a_link()                        // a file symlink: each gets (name, true)
#[test] fn the_walk_stops_when_asked_at_the_deadline_and_when_cancelled()     // each false after 3 → Asked, 3 seen; a past deadline → Deadline; cancel set → Cancelled
#[test] fn the_walk_leaves_out_what_skip_names()                              // skip(|p| p == "a.txt")
#[test] fn the_walk_from_a_sub_folder_applies_the_folders_above_it()          // root ".gitignore" "*.log", top = start = root/sub: sub/a.log not listed, paths relative to sub
#[test] fn a_walk_started_inside_an_ignored_folder_still_lists_it()           // root ".gitignore" "node_modules/\n*.log\n", top = start = node_modules/pkg: a.js listed, b.log not
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p marshell-core -j 4 server::ignores`
Expected: FAIL to compile (`Ignores` not found).

- [ ] **Step 4: Implement `Ignores`, `git_dir` and `walk_unignored` in `ignores.rs`**

- [ ] **Step 5: Run them to see them pass**

Run: `cargo test -p marshell-core -j 4 server::ignores`
Expected: all pass (25 on Windows).

- [ ] **Step 6: Add the notice and commit**

Add an `## ignore and globset` entry to `THIRD_PARTY_NOTICES.md` in the `## regex` entry's format (package links and versions from `Cargo.lock`, "MIT OR Unlicense, used here under MIT", the source <https://github.com/BurntSushi/ripgrep>, "Linked into `marshell-app.exe`; it reads `.gitignore` files for the Files panel.", then the MIT text from the crate's `LICENSE-MIT`).

```bash
cargo fmt --all
git add crates/core/src/server/ignores.rs crates/core/src/server/mod.rs crates/core/Cargo.toml Cargo.lock THIRD_PARTY_NOTICES.md
git commit -m "feat(core): the core reads git's ignore rules from a checkout's files, without running git."
```

Tasks 1 to 4 run `cargo clippy -p marshell-core --all-targets` (without `-D warnings`) before their commit: the only warnings allowed are dead code that a later task uses. From Task 5 on, clippy runs with `-D warnings`.

### Task 2: Project roots name where git may run

**Files:**
- Modify: `crates/core/src/server/files.rs:633-654` (`first_folder`, `project_root`), and the three callers `files.rs:665`, `search.rs:510`, `tree.rs:323`
- Test: `crates/core/src/server/files.rs` `mod tests`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub(super) enum Root {
      /// The launch folder's checkout: the only place git runs.
      Checkout(PathBuf),
      /// A folder inside a checkout that git must not run in (the shell reported it); its rules come from `Ignores`.
      InCheckout { root: PathBuf, checkout: PathBuf },
      /// A folder in no checkout.
      Plain(PathBuf),
  }
  impl Root { pub(super) fn path(&self) -> &Path }
  pub(super) fn project_root(row: Option<&SessionInfo>, reported: Option<PathBuf>) -> Option<Root>
  fn checkout_above(folder: &Path) -> Option<PathBuf>                       // checkout_above_with(folder, dirs::home_dir())
  fn checkout_above_with(folder: &Path, home: Option<&Path>) -> Option<PathBuf>
  ```
- Consumes: nothing new.

`checkout_above_with` walks from `folder` up and gives the first folder holding `.git` (file or folder), canonical. It uses file-system checks only, never git, and probes each `.git` with `symlink_metadata`. Two cases end the walk with no checkout:
- a `.git` that is a link, as Task 1 rules;
- the home folder's own `.git` when `folder` is below home (6t decision 11, the dotfiles repo). A session or shell in the home folder itself still gets the home repo.

`checkout_of` (`files.rs:282`) becomes `checkout_above(&row.cwd)`, so `bases_of` (:298) and `project_root` follow the same rules.

The order stays:
1. the launch folder's checkout, as `Root::Checkout`;
2. else the first folder of reported-then-launch: `InCheckout` when `checkout_above` finds a checkout for it, else `Plain`.

In this task the callers treat `InCheckout` exactly like `Plain`, so there's no behaviour change yet; `Checkout` takes today's `is_git == true` path.

- [ ] **Step 1: Replace the pin test**

Replace `git_never_runs_in_a_folder_learned_only_from_the_shell` (`files.rs:695`) with:

```rust
#[test]
fn a_folder_learned_from_the_shell_is_never_a_checkout_git_runs_in() {
    // repo/.git, repo/sub, plain/ (git init as the old test did)
    assert_eq!(project_root(None, Some(repo.join("sub"))), Some(Root::InCheckout { root: canon(repo/sub), checkout: canon(repo) }));
    assert_eq!(project_root(None, Some(repo.clone())), Some(Root::InCheckout { root: canon(repo), checkout: canon(repo) }));
    assert_eq!(project_root(None, Some(plain.clone())), Some(Root::Plain(canon(plain))));
}
#[test]
fn the_launch_folders_checkout_wins_over_a_reported_folder() {
    // a row whose cwd is repo/sub, reported = plain: Root::Checkout(canon(repo))
}
#[test]
fn a_repo_in_the_home_folder_counts_only_for_the_home_folder_itself() {
    // home/.git, home/projects/x: checkout_above_with(x, Some(home)) None; checkout_above_with(home, Some(home)) Some(home);
    // home/projects/y/.git: checkout_above_with(y/src, Some(home)) Some(y)
}
#[test]
fn a_dot_git_link_ends_the_walk_up_with_no_checkout() {
    // outer/.git folder, outer/inner/.git a junction (Windows) or symlink: checkout_above_with(inner, None) None; skip when the OS refuses the link
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p marshell-core -j 4 server::files`
Expected: FAIL to compile (`Root` not found).

- [ ] **Step 3: Implement `Root`, `checkout_above` and the new `project_root`; adapt the three callers with a `match` that keeps today's behaviour**

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p marshell-core -j 4`
Expected: all pass (search's and tree's tests unchanged).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/core/src/server/files.rs crates/core/src/server/search.rs crates/core/src/server/tree.rs
git commit -m "refactor(core): a project root says whether git may run there, and a folder the shell reported never says yes."
```

### Task 3: The file list: tracked files from git, the rest from the walk

**Files:**
- Modify: `crates/core/src/server/files.rs` (`git_files` :572 replaced, the `list` route :657)
- Test: `crates/core/src/server/files.rs` `mod tests`

**Interfaces:**
- Consumes: `Root` (Task 2); `Ignores`, `git_dir`, `walk_unignored` (Task 1); `git::stream` (`git.rs:152`).
- Produces, for Tasks 4 and 5:
  ```rust
  pub(super) struct Tracked { pub files: Vec<String>, pub cut: bool } // the index's paths, in git's (byte) order
  impl Tracked { pub(super) fn has(&self, rel: &str) -> bool }        // binary search
  pub(super) fn git_tracked(checkout: &Path, deadline: Instant) -> Option<Arc<Tracked>>
  ```
  `git_tracked` runs `const TRACKED_ARGS: [&str; 3] = ["ls-files", "--cached", "-z"]` through `git::stream`. It gives `None` when git can't run or exits with an error, and `cut: true` when the deadline stopped it. A path git prints more than once (a conflicted file, once per stage) is kept once.

  The answers are kept in a `static` map.
  - **The key** is the checkout plus the index file's length and modified time (`git_dir(checkout)/index`), read **before** git runs, so an answer is never filed under a newer index than it read. A missing index is its own key.
  - **Reuse:** until the key changes, the kept answer is given and no git runs. A cut answer is kept too, so a repo whose list takes over the deadline doesn't run git for it again on every request.
  - **Size:** the map holds at most 8 checkouts; adding a ninth drops the one used longest ago.
  - **Time resolution:** NTFS's 100 ns modified time catches every `git add`. On FAT or exFAT (2 s) two index writes in the same 2 s with the same length would keep the older answer until the next write; that's a known limit.

The list route per root:
- `Checkout(root)`:
  - `git_tracked(root, now + LIST_BUDGET)`, each file kept through `under_plain_folders` and `is_listable` as `git_files` does today.
  - Then `walk_unignored(Ignores::new(root, root), root, the same deadline, never cancelled, skip = Tracked::has, each)`. `each` keeps a file link only when `is_listable` passes, and stops at `MAX_LIST_FILES.saturating_sub(tracked)` files.
  - The result is the union, sorted. `truncated` is set when either part was cut or the cap was reached.
  - When `git_tracked` is `None`, the walk runs alone; it still knows the ignore rules.
- `InCheckout { root, checkout }`: `walk_unignored(Ignores::new(checkout, root), root, now + LIST_BUDGET, …, skip nothing, each)`, with the same `each`.
- `Plain(root)`: `walk_files(root)` as today.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn in_a_checkout_untracked_files_follow_gitignore()            // git init; commit keep.log with `git add -f`; .gitignore "*.log"; untracked a.log, b.txt → list has keep.log, b.txt, .gitignore; not a.log
#[cfg(windows)] #[test] fn in_a_checkout_a_junction_in_an_untracked_folder_is_never_walked() // new/real.txt, new/link → junction to a folder of 50 files: list = [.., new/real.txt], nothing from the target
#[test] fn a_reported_folder_in_a_checkout_lists_without_its_ignored_files() // list_root(Root::InCheckout over repo/sub, &|_, _| panic!("git ran")) with repo/.gitignore "*.log": sub/a.txt yes, sub/b.log no; paths relative to sub
#[test] fn the_tracked_list_reads_only_the_index()                     // TRACKED_ARGS == ["ls-files", "--cached", "-z"]: no --others, no --exclude-standard
#[test] fn the_tracked_list_is_read_again_only_when_the_index_changes() // count runs through a test-only runner: two calls → 1 run; `git add` a file → 2 runs, and the new file is there
#[test] fn a_cut_tracked_list_is_kept_until_the_index_changes()       // a runner that returns cut: two calls → 1 run, both cut
#[test] fn the_tracked_cache_keeps_8_checkouts()                       // 9 temp repos through a counting runner: the first is run again, the ninth is not
#[test] fn a_conflicted_file_is_listed_once()                          // a runner printing "a.txt\0a.txt\0a.txt\0": files == ["a.txt"]
#[test] fn without_git_a_checkout_is_listed_by_its_rules()            // list_root with tracked = |_, _| None: a.log left out
```

`in_a_checkout_untracked_files_follow_gitignore` and the junction test give today's answer through today's code too (`under_plain_folders` drops what git found through a link), so they guard the result, and `the_tracked_list_reads_only_the_index` is the one that fails first for #167.

Put the per-root work in `fn list_root(root: &Root, tracked: &dyn Fn(&Path, Instant) -> Option<Arc<Tracked>>) -> (Vec<String>, bool)`, so the last test can pass `&|_, _| None`; the route passes `&git_tracked`. To count runs, give the cache a `fn tracked_with(checkout, deadline, run: &dyn Fn(&Path, Instant) -> Option<Tracked>) -> Option<Arc<Tracked>>`; `git_tracked` calls it with the `git::stream` runner.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p marshell-core -j 4 server::files`
Expected: FAIL (`git_tracked`, `list_root` not found).

- [ ] **Step 3: Implement `git_tracked` and `list_root`; delete `git_files`; the route calls `list_root(&root, git_tracked)`**

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p marshell-core -j 4`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/core/src/server/files.rs
git commit -m "fix(files): a checkout's untracked files are walked by the core, so git never walks a folder link, and git's list has a deadline."
```

### Task 4: The content search per root, and a read error that is an error

**Files:**
- Modify: `crates/core/src/server/search.rs` (`run_search` :472, `grep_git` :363, `walk` :437, the route :500), `crates/core/src/git.rs` (the reader thread :188-202)
- Test: `search.rs` and `git.rs` `mod tests`

**Interfaces:**
- Consumes: `Root` (Task 2), `git_tracked` (Task 3), `Ignores`, `walk_unignored` (Task 1).
- Produces:
  - `pub(super) fn run_search(root: &Root, q: &SearchQuery, cancel: &AtomicBool) -> SearchReply`, which the route passes the `Root` to.
  - `fn run_search_with(root: &Root, q: &SearchQuery, cancel: &AtomicBool, grep: &dyn Fn(&Path, &SearchQuery, Instant, &AtomicBool, &mut Found) -> anyhow::Result<Stopped>, tracked: &dyn Fn(&Path, Instant) -> Option<Arc<Tracked>>) -> SearchReply`. `run_search` passes `grep_git` and `git_tracked`.
  - `fn grep_args(q: &SearchQuery) -> Vec<&str>`, the arguments `grep_git` gives git.
  - In `git.rs`, `fn pump(r: impl Read, tx: SyncSender<std::io::Result<Vec<u8>>>)`.

Per root, all under one `deadline = now + BUDGET`:
- `Checkout(root)`:
  1. `grep`, whose arguments no longer hold `--untracked`.
  2. `tracked(root, deadline)` for the skip set.
  3. `walk_unignored(Ignores::new(root, root), root, deadline, cancel, skip = Tracked::has, each)`. `each` skips a file link and runs `search_file` on every other file; it returns false once `Found` takes no more. Tracked files' lines therefore fill the 1,000-line cap first.
  - On `grep`'s `Err`: clear what it found, then run the walk with no skip.
  - When `tracked` is `None` (or cut) after a good grep, the walk skips only the paths `found` already holds.
  - The walk's `Stopped` goes to `Found::reply` as `grep_git`'s does, so a cancel still gives an empty reply and a deadline the time cut.
- `InCheckout { root, checkout }`: `walk_unignored(Ignores::new(checkout, root), root, …)` with the same `each`. Neither `grep` nor `tracked` is called.
- `Plain(root)`: `walk` as today, which now leaves out file links too: its `DirEntry::file_type()` says so, and any link is skipped.

`pump` sends each chunk, sends `Err(e)` on a read error and stops; `stream` then kills git and returns `Err`, so `run_search` takes the walk instead of a short reply.

- [ ] **Step 1: Write the failing tests**

```rust
// search.rs
#[test] fn in_a_checkout_an_ignored_untracked_file_is_not_searched()          // untracked a.log under "*.log" holds the text; b.txt holds it: only b.txt
#[cfg(windows)] #[test] fn in_a_checkout_a_junction_in_an_untracked_folder_is_not_searched_through() // new/link → folder with hits: no hit from it, new/real.txt found
#[test] fn a_reported_folder_in_a_checkout_skips_ignored_files_and_runs_no_git() // run_search_with(Root::InCheckout, …, grep and tracked that panic!("git ran")): sub/b.log skipped, sub/a.txt found
#[test] fn git_grep_searches_only_tracked_files()                            // grep_args(q) holds "grep" and no "--untracked"
#[test] fn the_walk_search_never_reads_a_file_link()                         // Plain root, a file symlink to a file with the text outside the root; skip if the OS refuses the link
// git.rs
#[test] fn a_read_error_ends_the_stream_as_an_error()                        // pump over a reader that returns Ok(b"a\0") then Err(io::Error::other("x")): the channel yields Ok then Err
```

Keep `finds_untracked_files_that_are_not_ignored` and `in_a_checkout_a_folder_link_is_not_searched_through`; update their calls to pass a `Root`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p marshell-core -j 4 -- server::search git::tests`
Expected: FAIL (`run_search_with`, `grep_args`, `pump` not found).

- [ ] **Step 3: Implement per-root `run_search`, drop `--untracked`, skip file links in `walk`, and `pump`**

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p marshell-core -j 4`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/core/src/server/search.rs crates/core/src/git.rs
git commit -m "fix(files): the search reads untracked files itself and never through a link, and a failed read of git's output falls back to the walk."
```

### Task 5: The tree's marks from the rules, with no `check-ignore`

**Files:**
- Modify: `crates/core/src/server/tree.rs` (`RunGit` :25, `check_ignore` :224, `mark_ignored` :256, `tree_with` :281, the route :315-324), `crates/core/src/git.rs` (`run_input` :93 removed)
- Test: `tree.rs` `mod tests`

**Interfaces:**
- Consumes: `Root` (Task 2), `Ignores` (Task 1), `Tracked`, `git_tracked` (Task 3).
- Produces:
  ```rust
  const TREE_GIT_BUDGET: Duration = Duration::from_secs(2);
  type TrackedFn<'a> = &'a dyn Fn(&Path, Instant) -> Option<Arc<Tracked>>;
  fn tree_with(root: &Root, asks: &[TreeAsk], tracked: TrackedFn) -> Vec<TreeDir>
  ```

Marks per root, only for `Listed` folders. `tracked` is called at most once per request, and only when a folder was listed and the rules marked a file in it:
- `Checkout(root)`:
  - An entry is ignored when `Ignores::new(root, root).ignored(abs, is_folder)` says so.
  - A file `Tracked::has` names is never marked, because git never calls a tracked file ignored; this matches `check-ignore`'s answer today.
  - `tracked` is called with `now + TREE_GIT_BUDGET`. A `None` or cut answer means the rules alone.
- `InCheckout { root, checkout }`: `Ignores::new(checkout, root)` over `root.join(dir).join(name)`; `tracked` is never called.
- `Plain`: nothing is marked, as today.

- [ ] **Step 1: Write the failing tests; rewrite the git-runner ones**

```rust
#[test] fn marks_ignored_entries_and_everything_in_an_ignored_folder()   // rewritten on a `.git` folder + ".gitignore" "*.log\ndist/\n", tracked = |_,_| None
#[test] fn a_file_git_tracks_is_never_marked_in_the_launch_checkout()   // ".gitignore" "*.log", tracked = |_,_| Some(Tracked { files: ["keep.log"], cut: false }): keep.log unmarked, a.log marked
#[test] fn a_reported_folder_in_a_checkout_gets_marks_and_runs_no_git() // Root::InCheckout, tracked = |_,_| panic!("git ran"): b.log marked
#[test] fn git_runs_only_when_a_listed_folder_has_an_ignored_file()     // rewritten: a counting `tracked`; a `same` round 0, a listed round with no ignored file 0, with a.log 1
#[test] fn a_cut_tracked_list_leaves_the_rules_alone()                  // tracked = |_,_| Some(Tracked { files: ["a.log"], cut: true }): a.log marked
```

Delete `a_path_git_refuses_falls_back_to_one_run_per_folder` (`tree.rs:659`); `check_ignore` and its per-folder retry are gone. Keep `outside_git_nothing_is_ignored` with `Root::Plain`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p marshell-core -j 4 server::tree`
Expected: FAIL to compile (`TrackedFn` not found, `tree_with` takes a `Root`).

- [ ] **Step 3: Implement; remove `check_ignore`, `Checked`, `RunGit` and `git::run_input`; the route passes `&git_tracked`**

- [ ] **Step 4: Run the crate's tests and clippy**

Run: `cargo test -p marshell-core -j 4` then `cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass; clippy clean (no dead code left).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/core/src/server/tree.rs crates/core/src/git.rs
git commit -m "fix(files): the tree's ignore marks come from the rules, so a folder the shell moved into gets them and git check-ignore no longer runs."
```

### Task 6: An auto save copies when someone else wrote the file

**Files:**
- Modify: `crates/core/src/server/save.rs` (`replace_with` :247, `copy_due` :49)
- Test: `save.rs` `mod tests`

**Interfaces:**
- Produces: `fn last_written(path: &Path) -> Option<String>` and `fn remember_written(path: &Path, hash: &str)`, over a `static WRITTEN: LazyLock<Mutex<HashMap<String, String>>>`.
  - The key is the path with its trailing separators trimmed and, on Windows, ASCII-lowercased, so two paths share a key exactly when `projects::same_path` (`projects.rs:52`) calls them the same.
  - The lock is held only around the map, never across I/O.

In `replace_with`, an auto save (`Copy::IfOlderThan`) takes its copy as `Copy::Always` would when the hash of the bytes it replaces isn't `last_written(path)`.

`remember_written(path, &hash)` goes in `saved()` (`save.rs:187`). Every write that completes ends there:
- the new-file write (`write_in_place`, :220);
- the "disk already holds these bytes" return (:228);
- `replace_with`;
- an undo (`unsave_checked` goes through `replace`, :307).

A write cut short and restored never reaches `saved()`, so it leaves the record as it was. The map lives in memory, so after a restart the first auto save of each file takes a copy.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn an_auto_save_over_another_programs_write_takes_a_copy_within_5_minutes() // save → copy 1; write the file directly; auto save (expect = the new hash) → copy 2
#[test] fn an_auto_save_over_its_own_last_write_keeps_the_5_minute_rule()           // save; auto save; auto save within 5 min → still 1 copy past the first save's
#[test] fn the_first_auto_save_of_a_file_marshell_never_wrote_takes_a_copy()        // a file made directly, an existing copy under 5 min old: auto save still copies
#[test] fn two_tabs_auto_saving_one_file_keep_the_5_minute_rule()                   // save (tab A), auto save expecting A's hash (tab B) → no extra copy
#[test] fn a_file_marshell_made_counts_as_its_own_write()                          // save_checked with Expect::NoFile, then an auto save within 5 min → no copy
```

Update `an_auto_save_takes_no_copy_when_the_newest_is_under_5_minutes_old` (:432), `a_cut_short_auto_save_that_cannot_restore_takes_the_copy_it_skipped` (:474) and `a_cut_short_auto_save_that_restores_takes_no_copy` (:493) so the file's first write goes through `save_checked`. Otherwise the new rule copies before the write, and they pass or fail for the wrong reason. The map is shared by tests that run at once; each test's temp path is its own key.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p marshell-core -j 4 server::save`
Expected: the first and third new tests FAIL (no copy taken).

- [ ] **Step 3: Implement the record and the rule**

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p marshell-core -j 4`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/core/src/server/save.rs
git commit -m "fix(editor): an auto save keeps a history copy whenever the file on disk isn't what Marshell last wrote there."
```

### Task 7: The saved tree root's bound, and router tests that clean up

**Files:**
- Modify: `crates/core/src/layout.rs` (`validate_pane` :512, `repair_pane` :333), `crates/core/src/server/tree.rs:710`, `crates/core/src/server/search.rs:1002`
- Test: `layout.rs` `mod tests`

**Interfaces:** none new.

- `validate_pane`: a session's `tree.root` longer than `MAX_PATH_CHARS` characters is `Err("pane.sessions.{session}.tree.root is longer than 4096 characters")`. An empty root stays valid (the tree hasn't been answered yet).
- `repair_pane`: such a root becomes `""` and its `expanded` is cleared (they were relative to it).
- The two `post` helpers keep their `TempDir` alive for the test and drop it at the end, instead of `tempdir().keep()`.

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn a_saved_tree_root_over_4096_characters_is_refused_and_repaired() // validate_pane(&with_tree(root = "a".repeat(4097))) is Err naming tree.root; repair gives root "" and expanded []
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p marshell-core -j 4 layout::tests::a_saved_tree_root`
Expected: FAIL.

- [ ] **Step 3: Implement; replace the two `.keep()` calls**

- [ ] **Step 4: Run the crate's tests and check the temp homes**

Run: `cargo test -p marshell-core -j 4`, then `grep -c "keep()" crates/core/src/server/tree.rs crates/core/src/server/search.rs`
Expected: all pass; `0` for both files.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/core/src/layout.rs crates/core/src/server/tree.rs crates/core/src/server/search.rs
git commit -m "fix(core): a saved tree root has the path length bound, and the tree and search router tests delete their temp homes."
```

### Task 8: Files says what it waits for

**Files:**
- Create: `src/features/files/FilesBusy.tsx`, `src/features/files/FilesBusy.css`, `src/features/files/filesWait.ts`, `src/features/files/filesWait.test.ts`, `src/features/files/FilesBusy.test.tsx`
- Modify: `src/features/files/filterText.ts`, `src/features/files/filterText.test.ts`, `src/features/files/FilesPanel.tsx`, `src/features/files/FilesPanel.test.tsx`, `src/features/files/filesActions.ts`, `src/features/preview/quickFiles.ts`, `src/features/preview/quickFiles.test.ts`, `src/features/sessions/LiveApp.tsx:1806-1808`, `src/features/sessions/LiveApp.test.tsx`

**Interfaces:**
- Produces:
  ```ts
  // filesWait.ts
  export const LISTING = "Listing files…";
  export const SEARCHING = "Searching…";
  export const RING_DELAY_MS = 150;
  /** The palette's words (`Palette.tsx:233-237`), with its fallback when there is no folder. */
  export const listFailed = (folder: string | null): string => (folder ? `Can't list the files in ${folder}.` : "Can't list files for this session.");
  /** What Files waits for, as the ring's line, or null. */
  export function filesWait(o: { mode: FilterMode; names: "loading" | "error" | "ready"; searchBusy: boolean }): string | null;
  /** Turns `on` into "shown" only after `ms` of being on; off at once. */
  export function createShownAfter(ms: number, set: (shown: boolean) => void): { update(on: boolean): void; dispose(): void };
  export function useShownAfter(on: boolean, ms?: number): boolean;
  /** Whether focus goes to the filter box: the wait has just begun and focus was in the tree area when it did. */
  export function focusToFilter(o: { waitBegan: boolean; focusWasInTree: boolean }): boolean;
  // quickFiles.ts
  /** The cached list for the tab, when it is still fresh and for this root; no request is made. */
  export function cachedFileListNow(tab: string, root: string | null, now?: () => number, cache?: Map<string, Entry>): FileList | null;
  // FilesBusy.tsx
  export function FilesBusy(p: { text: string }): JSX.Element;
  // filterText.ts
  export const LONG_NOTE = "Search for 200 characters or fewer.";
  // FilterMode gains { kind: "long" }
  // filesActions.ts
  export const COLLAPSED = "Collapsed all folders.";
  ```

Behaviour:
- `filesWait`: `names` mode with `names === "loading"` gives `LISTING`; `search` mode with `searchBusy` gives `SEARCHING`; anything else `null` (`short`, `long` and `none` never wait).
- `filterMode`: the text after `?` is counted in characters (`[...query].length`, as the core's `chars().count()`): under 2 is `short`, over 200 is `long`.
- `FilesPanel`:
  - **The name list's state:**
    - When the tree's `root` moves (not when the mode flips), `names` goes back to `"loading"` until the new list lands, so the old folder's rows are never shown as the new one's.
    - `names` starts from `cachedFileListNow(session, root)` when that has a list. That way the first key after a mount doesn't blank the tree for a frame when the list is already cached.
  - **The note** is `null` while `filesWait` is not null, so the old note clears at once. In `names` mode with `names === "error"` it is `listFailed(root === null ? null : folderName(root))`, and the unfiltered tree stays under it.
  - **The wait area:** while `wait !== null`, the tree area draws `<div className="files-wait" aria-busy="true">` (`block-size: 100%`) in place of `FileTree` or `SearchResults`, at once: decision 5 hides the old rows until the answer lands. `<FilesBusy text={wait} />` appears inside it once `useShownAfter(wait !== null)` is true, after 150 ms.
  - **Focus:** the tree area's wrapper keeps a ref of whether focus is inside it, set by `onFocusCapture` and cleared by `onBlurCapture` when `relatedTarget` is outside. A `useLayoutEffect` on `wait !== null` calls `filterBox.current.focus()` when `focusToFilter` says so. The ref is needed because React removes the focused row before any effect runs, so `document.activeElement` is already the body by then.
  - `long` mode's note is `LONG_NOTE`, and it sends no search (`useSearch` gets `query: null`). `useSearch` already reports busy when a query is scheduled, before its 300 ms timer (`useSearch.ts:40`), so the ring covers the whole wait without a change there.
  - Collapse all, from the handle (`FilesPanel.tsx:156`) and the path bar menu (:306), runs `tree.collapseAll()` and then `announce(COLLAPSED)`.
- `LiveApp`: the saved-state collapse (:1806-1808) also announces `COLLAPSED`.
- `FilesBusy`:
  - Renders an `svg` (40 × 40, `aria-hidden="true"`, class `files-busy__ring`) with a track circle and an arc, both `stroke="currentColor"`, and `<p className="files-busy__text">{text}</p>`.
  - CSS: centred in the area (`display: grid; place-content: center; justify-items: center; block-size: 100%; gap: var(--space-2)`); the ring in `var(--text-3)`, the track at `color-mix(in oklch, currentColor 25%, transparent)`, the line `var(--text-13)` in `var(--text-2)`.
  - Rotation `files-busy-spin 1s linear infinite`, only inside `@media (prefers-reduced-motion: no-preference) { :root:not([data-motion="reduce"]) … }`.
  - Forced colours: ring `CanvasText`, track `GrayText`, line `CanvasText`.

- [ ] **Step 1: Write the failing tests**

```ts
// filterText.test.ts
it("counts characters, not UTF-16 units, and says the 200 limit", () => {
  expect(filterMode("?😀")).toEqual({ kind: "short" });
  expect(filterMode("?" + "a".repeat(200))).toEqual({ kind: "search", query: "a".repeat(200) });
  expect(filterMode("?" + "a".repeat(201))).toEqual({ kind: "long" });
  expect(LONG_NOTE).toBe("Search for 200 characters or fewer.");
});
// filesWait.test.ts
it("waits for the name list only in names mode, and for a search only while it is busy", ...) // each FilterMode × names × busy
it("shows the ring only after 150 ms and hides it at once", ...)  // vi.useFakeTimers(); update(true); advance 149 → false; 1 → true; update(false) → false
it("a wait that ends inside 150 ms never shows the ring", ...)
it("sends focus to the filter box only when a wait begins with focus in the tree", ...) // focusToFilter: (true, true) true; (true, false), (false, true) false
it("says a failed list with the folder, or for the session when there is none", ...) // listFailed("src") "Can't list the files in src."; listFailed(null) "Can't list files for this session."
// quickFiles.test.ts
it("gives a fresh cached list for the same root without a request, and nothing otherwise", ...) // after fileListFor with a counting fetch: cachedFileListNow same root → the list, other root → null, past the cache's age → null; fetch count unchanged
// FilesBusy.test.tsx
it("is a 40 px ring hidden from a screen reader, with its line under it", ...) // markup: width="40" height="40" aria-hidden="true", <p class="files-busy__text">Searching…</p>
// FilesBusy.css (read as text in FilesBusy.test.tsx)
it("turns only when motion is allowed, and uses system colours in forced colours", ...) // the animation line sits inside prefers-reduced-motion: no-preference and :not([data-motion="reduce"]); the forced-colors block names only CanvasText/GrayText
// FilesPanel.test.tsx (source pins, as the file's other pins do)
it("hides the old note and the old rows at once while Files waits, and draws the ring after 150 ms", ...)
it("says a failed name list in the palette's words and keeps the tree", ...)
it("moves focus from the rows to the filter box when the wait replaces them", ...) // the wrapper's onFocusCapture/onBlurCapture ref and the useLayoutEffect calling focusToFilter
it("lists the names again when the root moves and starts from the cached list", ...) // setNames("loading") on a root change; useState seeded from cachedFileListNow
it("announces Collapsed all folders. from the menu and the handle", ...)
// LiveApp.test.tsx
it("a collapse through the saved tree announces Collapsed all folders.", ...)
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm test src/features/files src/features/sessions/LiveApp.test.tsx`
Expected: FAIL (new modules and exports missing).

- [ ] **Step 3: Implement `filesWait.ts`, `FilesBusy.tsx`, `FilesBusy.css`, the `long` mode, and the `FilesPanel` and `LiveApp` changes**

Read the good-css skill before writing `FilesBusy.css`.

- [ ] **Step 4: Run the frontend tests and the build**

Run: `pnpm test` then `pnpm build`
Expected: all pass (the styles suite's no-scrollbar, forced-colours and no-boxed-outline tests included); build ok.

- [ ] **Step 5: Commit**

```bash
git add src/features/files src/features/preview/quickFiles.ts src/features/preview/quickFiles.test.ts src/features/sessions/LiveApp.tsx src/features/sessions/LiveApp.test.tsx
git commit -m "feat(files): a ring in the middle of the tree says what Files waits for, a search over 200 characters says so, and Collapse all speaks."
```

### Task 9: The tree's small faults

**Files:**
- Modify: `src/features/files/treeModel.ts:80-115`, `src/features/files/FileTree.tsx:248-254`, `src/features/files/FileTree.css:208-224`, `src/features/files/reveal.ts:15-17`, `src/features/files/useTree.ts:88`, `:95-157`, `:256`
- Test: `treeModel.test.ts`, `FileTree.test.tsx`, `FileTree.css.test.ts`, `reveal.test.ts`, `useTree.test.ts`

**Interfaces:**
- Produces: `ReplyTag = { session: string; root: string | null; expanded: ReadonlySet<string> }` (the open folders when the request went out).

Fixes:
- A folder cut at 5,000 gets its "more" row as the last item of its set: every row of that folder has `setSize: size + 1`, and the more row has `posInSet: size + 1`. An empty folder's one row stays 1 of 1.
- Note rows take their selected state from `selectedProps(false, "selected", "file-tree__row file-tree__row--note")`, with no hand-written `aria-selected`.
- In forced colours, `.file-tree-filter` gets `outline: 1px solid CanvasText`, as `Palette.css:211` does. Its `:focus-visible` keeps the 2 px `Highlight`.
- `revealRefused` refuses any path starting with two slashes of either kind: `/^[\\/]{2}/`.
- `applyReply` drops a folder from `expanded` only when it was open when the request went out (`tag.expanded.has(e)`). The tag at `useTree.ts:256` is built with `expanded: new Set(data.expanded)`. `ReplyTag.expanded` is required, so every `applyReply` call in `useTree.test.ts` (about 12, lines 104-175) gains it, set to the data's open folders. The test near :174 that expects a folder to be dropped keeps passing only with that set.

- [ ] **Step 1: Write the failing tests**

```ts
// treeModel.test.ts (replaces "set size and position count only real entries")
it("a more row is the last of its folder's set", ...)  // 3 entries, more 10: real rows setSize 4, posInSet 1..3; more row 4 of 4; empty folder row 1 of 1
// FileTree.test.tsx
it("a note row's selected state comes from selectedProps", ...) // markup has class is-selected-mark and aria-selected="false"; source has no `aria-selected={false}`
// FileTree.css.test.ts
it("the filter box has an edge in forced colours", ...) // the forced-colors block holds `.file-tree-filter` with `outline: 1px solid CanvasText`
// reveal.test.ts
it("refuses mixed slashes at the start", ...) // revealRefused("/\\srv/x"), revealRefused("\\/srv/x") true; revealRefused("C:/x") false
// useTree.test.ts
it("a folder opened while a reply was out is kept", ...) // tag.expanded lacks "src/new"; the reply lists src without "new"; expanded still holds src/new
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm test src/features/files`
Expected: the five new tests FAIL.

- [ ] **Step 3: Implement the five fixes**

- [ ] **Step 4: Run the frontend tests and the build**

Run: `pnpm test` then `pnpm build`
Expected: all pass; build ok (`tsconfig.json` includes the tests, so a missed `expanded` fails here).

- [ ] **Step 5: Commit**

```bash
git add src/features/files
git commit -m "fix(files): note rows count in their folder's set, the filter box has an edge in forced colours, mixed slashes are refused and a reveal survives a reply."
```

### Task 10: The editor's and the pane's small faults

**Files:**
- Modify: `src/features/editor/TextTab.tsx:70-72`, `:371`, `:380`; `src/features/pane/paneCommands.ts:48-51`; `src/features/pane/paneState.ts:447-453` (`setTree`); `src/features/sessions/LiveApp.tsx:1034-1047`, `:2445`
- Test: `src/features/editor/TextTab.test.tsx`, `src/features/pane/paneCommands.test.ts`, `src/features/pane/paneState.test.ts`, `src/features/sessions/LiveApp.test.tsx`

**Interfaces:**
- Produces:
  - `export function factsOf(d: DocState, on: boolean, readOnly: boolean, scan: boolean): AutoSaveFacts`: `markers` is `scan && on && d.dirty && countConflicts(d.text) > 0`.
  - `export function openCheck(pane: PaneView, session: string | null | undefined, path: string, replacesPreview = false): "no-session" | "refused" | "ok"`.

Fixes:
- The timer effect (:380) calls `factsOf(…, false)`, and `saveIfDue` (:371) calls it with `true`. The whole text is scanned for markers once per pause, not on every render.
- `openFromTree` works out, before the check:
  - the open's `flag`, its `panel`, and the panel's `current` preview tab;
  - whether that tab is kept first (`keepFirst`).
  It then calls `openCheck(pane, session, path, flag.preview && current !== undefined && !kept)`. `previewTab` returns `PaneTab | undefined` (`paneState.ts:113`), so a `!== null` test would pass with no preview tab. At 32 tabs, a preview that replaces the panel's preview tab in place is allowed, as the reducer already allows it (`paneState.ts:364-368`). With no preview tab to replace, the open is refused and the tab-limit message (`TAB_LIMIT`) is said, never "Opened …".
- `onSaved` (:2445) dispatches `tree` only for a session still in `stateRef.current.infos` (`i.tab_id === session`). A flush from a session that just closed leaves no tree-only entry. `stateRef.current` is assigned during render (`LiveApp.tsx:830`), so it is current when the panel's dispose runs.
- `setTree` keeps no tree for a root longer than 4,096 characters (`[...root].length`, as the core counts), so a layout save never carries a root `validate_pane` refuses (Task 7). Such a root can only come from a hand-edited file or a path past Windows' usual limits.

- [ ] **Step 1: Write the failing tests**

```ts
// TextTab.test.tsx
it("the timer's facts skip the marker scan and the save's facts do it", ...) // a dirty doc with <<<<<<< markers: factsOf(d, true, false, false).markers false; (…, true).markers true
// paneCommands.test.ts
it("at 32 tabs an open that replaces the panel's preview tab is not refused", ...) // openCheck(pane32, s, "/new.ts") "refused"; openCheck(pane32, s, "/new.ts", true) "ok"
// paneState.test.ts
it("at 32 tabs with no preview tab a preview open changes nothing", ...) // paneReducer(pane32NoPreview, open preview "/new.ts") is the same state
it("a tree root over 4,096 characters is not kept", ...) // { type: "tree", root: "a".repeat(4097) } leaves tabsOf(...).tree undefined; 4,096 is kept
// LiveApp.test.tsx (source pins)
it("a tree open works out whether it replaces a preview before the tab-limit check", ...)
it("a tree save from a session that has closed is dropped", ...)
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm test src/features/editor/TextTab.test.tsx src/features/pane/paneCommands.test.ts src/features/pane/paneState.test.ts src/features/sessions/LiveApp.test.tsx`
Expected: the new tests FAIL except "with no preview tab a preview open changes nothing", which pins today's reducer.

- [ ] **Step 3: Implement the four fixes**

- [ ] **Step 4: Run the frontend tests and the build**

Run: `pnpm test` then `pnpm build`
Expected: all pass; build ok.

- [ ] **Step 5: Commit**

```bash
git add src/features/editor/TextTab.tsx src/features/editor/TextTab.test.tsx src/features/pane/paneCommands.ts src/features/pane/paneCommands.test.ts src/features/pane/paneState.ts src/features/pane/paneState.test.ts src/features/sessions/LiveApp.tsx src/features/sessions/LiveApp.test.tsx
git commit -m "fix(pane): a tree preview at the tab limit replaces the preview tab, a closed session's tree save is dropped, and auto save scans for markers once per pause."
```

### Task 11: The plan's record, and the live check

**Files:**
- Modify: `docs/PLAN.md` (6t decision 10, 6r-2 decision 24, the "Platform status" paragraph near line 655)
- Create: `docs/research/2026-11-22-files-leftovers-live-check.md`

- [ ] **Step 1: Write the record in `docs/PLAN.md`**

- 6t decision 10 ("Decided, not asked") was written with this plan. Add to it every `Ruling:` from the ledger that changes what a person sees or what the core reads or runs.
- 6r-2 decision 24 gains: "Amended by Slice 6t decision 3."
- "Platform status" gains one sentence for 6t decision 4 (the tree's `:` and `\` rule becomes Windows-only with the Linux and macOS work).

- [ ] **Step 2: Run the live check (marshell-live-check skill), with these rows**

| # | Check | Pass when |
| --- | --- | --- |
| 1 | A launch-folder checkout with an untracked folder holding a junction to a folder of 20,000 files | Go to file lists within 2 s and nothing from the junction; a `?` search finds a project file and nothing through the junction |
| 2 | A shell started outside git that `cd`s into a repo whose `.git/config` has `core.fsmonitor` pointing at a script that writes a marker file; the app started with `GIT_TRACE2_EVENT` set to a file in the scratch folder | The tree dims `node_modules` and `*.log`; the search skips them; the marker file never appears; the trace holds no git run in that repo whose `cmd_ancestry` passes through `marshell-app.exe` (a shell prompt's own git doesn't count) |
| 3 | The ring | After 150 ms of a slow name list or search: a 40 px ring in the middle of the tree area with "Listing files…" or "Searching…", no rows, the note line empty; no turning under emulated reduced motion; system colours under emulated forced colours (one screenshot each) |
| 4 | Long and short searches | "?😀" says "Type at least 2 characters to search."; 201 characters say "Search for 200 characters or fewer." and no request goes out |
| 5 | Collapse all | The announcer's region reads "Collapsed all folders." |
| 6 | Auto save after an agent's write | With auto save on, a write from outside then a typed edit: the history folder gains a copy |
| 7 | A folder cut at 5,000 | The more row reads `aria-posinset` = `aria-setsize` = 5,001 |
| 8 | Hand check with the user (6t decision 8) | With real focus inside a browser page, the user's own Ctrl+Shift+P opens the palette's file list |

- [ ] **Step 3: Write the research note in the skill's format, with every finding and its fix commit (failing test first)**

- [ ] **Step 4: Commit**

```bash
git add docs/PLAN.md docs/research/2026-11-22-files-leftovers-live-check.md
git commit -m "docs: slice 6t's rulings and its live check."
```

## Exit criteria

- Each of #167's three bullets and #168's two bullets is gone, by the tests above and live-check rows 1 and 2.
- Every #169 item is fixed or recorded: the hand check passes (row 8), and decision 4 is in "Platform status".
- `cargo test --workspace -j 4`, `pnpm test`, `pnpm test:claude`, `pnpm build`, clippy with `-D warnings`, `pnpm check:generated` and `pnpm tauri build --no-bundle` pass.
