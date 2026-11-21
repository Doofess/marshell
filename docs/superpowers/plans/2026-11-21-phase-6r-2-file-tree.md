# Slice 6r-2: the file tree, the path bar and the Files menu: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (the user's default). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Files panel gains a file tree of the session's project folder (with a name filter and a "?" content search), a path bar, preview tabs, and a ⋯ menu of editor settings that are also on a new Settings → Files page.

**Architecture:** Two new core routes read the disk: `POST /v1/sessions/{tab}/tree` lists asked folders one level deep with git's ignore marks and a per-folder stamp, and `POST /v1/sessions/{tab}/search` runs a plain-text search (`git grep` in a checkout, a capped walk outside one). The app keeps the tree as pure data (`treeModel.ts`) fed by a polling hook, draws it as a virtualized ARIA tree beside the open file, and saves the tree's state per session and its size per window in `layout.json`. Editor settings live in `settings.json` under `files` and reach CodeMirror through compartments.

**Tech Stack:** Rust (`marshell-protocol`, `marshell-core`), React 19 and TypeScript, CodeMirror 6, Vitest (node, `renderToStaticMarkup`), ts-rs, Tauri's opener plugin.

**Spec:** `docs/PLAN.md` §9, "Slice 6r decisions" 1, 3, 12 and 13, and "Slice 6r-2 decisions" 1 to 24. Reference screenshots (mechanics only, not the look): `C:\Users\Home Office\OneDrive\Projects\Marshell\screenshots\Screenshot 2026-11-20 233234.png` (the menu), `233740.png` and `232822.png` (the tree and path bar).

**Who does what:** the main thread writes Task 11's capability line, decides any copy or rule the plan doesn't fix, and does Task 15 (the plan's text and the live check), and checks every worker's diff and tests before ticking a task. Every other task goes to a `worker` (Sonnet) with the task's text, this header, the Global Constraints and the exact test command; `effort: high` is marked on the task. Each worker is told: touch only the task's files, never `git checkout`, `git restore` or `git stash`, and report the test command's last lines.

## Global Constraints

- Copy is exact, in sentence case, with no "!" and no "successfully". Quoted strings in this plan are the strings in the product and in the tests.
- Every new control works with the keyboard alone and in forced colors (system colors only in a `forced-colors` rule). No boxed outlines; colors come from the tokens in `src/styles/`. Read the `good-css` skill before writing declarations. Panels scroll with no scrollbar (`scrollbar-width: none`, as `.editor .cm-scroller` does).
- `Announcer` is the only live region; `SelectedMark`'s `selectedProps` is the only selected-state marker.
- The frontend talks only to the core API. The tree never writes, renames, moves or deletes a file (decision 4).
- New routes sit in the token-guarded router, each with a "refused without the bearer" test; blocking disk and process work runs in `spawn_blocking`; no `MutexGuard` is held across `.await`.
- `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`.
- No new crate or package (indent guides are our own extension). The one capability change is `"opener:allow-reveal-item-in-dir"` (decision 21).
- Rust: `cargo fmt --all` before each Rust commit; `cargo test ... -j 4`; clippy with `-D warnings` stays clean. A hook reformats edited Rust files: read the file again after it says so.
- Commit messages: `type(scope): what changed, as a sentence.` One commit per task unless a task says otherwise.
- Numbers, everywhere: tree folder cap 5,000 entries; at most 64 folders per tree request; at most 500 open folders saved per session; tree width 240 px at first, 160 px at least, 2,048 at most, the file keeps 240 px; narrow below 480 px of panel width; poll every 2,000 ms, a full re-read every 30,000 ms; search text 2 to 200 characters, 300 ms after the last key, at most 1,000 lines, files over 1,048,576 bytes skipped, a line cut to 240 characters, a 3,000 ms budget; name filter at most 500 matches; auto save 1,000 ms after the last key, with a file-history copy only when the newest is 300,000 ms old (decision 24).

## Review Focus

1. **A huge folder, or a slow one** (`node_modules` with 30,000 entries, a drive that answers slowly). Expected: the tree shows 5,000 entries and "And 25,000 more", stays scrollable without lag (virtualized rows), and polls never overlap. Pinned by Task 2's `cuts_a_folder_at_5000_and_counts_the_rest`, Task 7's `visible_rows_put_a_more_row_after_a_cut_folder` and Task 8's `a_poll_waits_for_the_one_in_flight`.
2. **Links and junctions** (pnpm's `node_modules`, a junction to `C:\`, a junction loop inside the project). Expected: a folder link is listed as a link and never entered, wherever it points; asking for a path through one is refused; nothing outside the project folder is listed; a folder of 5,000 ignored files answers without hanging git. Pinned by Task 2's link, junction and `a_folder_of_5000_ignored_files_answers` tests.
3. **The root changes or vanishes under the tree** (a `cd` outside git, a removed worktree, a deleted folder that was open). Expected: a new root starts with every folder closed; a gone folder closes and leaves the saved list; no error is said twice. The tree's open folders survive closing the session's last tab, and a reply or save meant for the session just left never lands on the next one. Pinned by Task 8's `a_new_root_closes_every_folder`, `a_gone_folder_is_dropped` and `a reply that comes after the session changed is dropped`, and Task 6's `closing a session's last tab keeps its tree`.
4. **A preview tab meets an edit, an agent's write or `marshell open`.** Expected: an edit keeps the tab before anything can replace it; a tab with unsaved edits is never replaced; an open by any other way keeps it. Pinned by Task 6's reducer tests.
5. **Auto save against an agent writing the same file, a merge left with conflicts, and file history.** Expected: a changed-on-disk conflict stops auto save and nothing on disk is overwritten; text with conflict markers and a read-only file are never auto-saved; auto saves don't push older history copies out. Pinned by Task 5's `auto_save_stops_at_a_conflict`, `never_auto_saves_text_that_still_holds_conflict_markers`, `never_auto_saves_read_only` and the `an_auto_save_takes_no_copy…` tests.
6. **A content search over minified or binary files, or one typed fast.** Expected: a 5 MB one-line bundle and a PNG are skipped, a long line is cut around its hit, and every older search's `git grep` is killed, even one that prints nothing. Pinned by Task 3's tests (`stream_kills_the_child_when_cancelled`, `stream_kills_a_child_that_prints_nothing_at_the_deadline`) and Task 9's `an_older_search_is_aborted`.

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/protocol/src/api.rs` | `FilesSettings`, `TreePosition`, `TreeState`, `TreeView`, the tree and search request and reply types; `preview` on `PaneTab`, `tree` on `SessionTabs` and `PaneView`. |
| `crates/core/src/settings.rs`, `layout.rs` | Reading and checking the new fields. |
| `crates/core/src/server/tree.rs` (new) | `POST /v1/sessions/{tab}/tree`. |
| `crates/core/src/server/search.rs` (new) | `POST /v1/sessions/{tab}/search`. |
| `crates/core/src/server/files.rs` | `project_root` shared by `list`, `tree` and `search`. |
| `src/features/files/` (new folder) | Everything the tree, the path bar, the filter, the search, the Files menu and Settings → Files draw or decide. |
| `src/features/editor/editorPrefs.ts`, `indentGuides.ts` (new) | The editor settings as CodeMirror extensions. |
| `src/features/pane/paneState.ts`, `SidePane.tsx` | Preview tabs; the tree's per-session and per-window state. |

---

### Task 1: The settings, preview tabs and the tree's saved state, in the protocol and the core

**Files:**
- Modify: `crates/protocol/src/api.rs` (`PaneTab` at 442, `SessionTabs` at 516, `PaneView` at 550, `Settings` at 1249), `crates/core/src/settings.rs` (`FileDoc`, `load`, `save`; a `lenient_files` beside `lenient_browser` at 48), `crates/core/src/layout.rs` (`lenient_session` at 124, `lenient_pane` at 45, `repair_pane` at 288, `validate_pane`), `src/features/pane/paneRules.ts` (`paneProblem` mirrors `validate_pane`), `src/features/appearance/settingsSync.ts` (both full `Settings` payloads carry `files` from the last read: `save` and `writeFlags`, as `browser` does), `src/features/appearance/settingsSync.test.ts` (the exact payload asserts gain `files`)
- Generated: `FilesSettings.ts`, `TreePosition.ts`, `TreeState.ts`, `TreeView.ts`, `Settings.ts`, `PaneTab.ts`, `SessionTabs.ts`, `PaneView.ts`
- Test: the `mod tests` of `settings.rs` and `layout.rs`; `src/features/pane/paneRules.test.ts`

**Interfaces:**
- Produces (Rust, in `api.rs`, each with `#[ts(export)]`):

```rust
#[serde(rename_all = "kebab-case")] pub enum TreePosition { #[default] Automatic, Left, Right }
#[serde(default)] pub struct FilesSettings {
    pub preview_tabs: bool,      // true
    pub auto_save: bool,         // false
    pub tab_indents: bool,       // true
    pub word_wrap: bool,         // false
    pub indent_guides: bool,     // true
    pub whitespace: bool,        // false
    pub scroll_past_end: bool,   // false
    pub tab_size: u8,            // 4; only 2, 4 or 8
    pub tree_position: TreePosition,
}
pub struct TreeState { pub root: String, pub expanded: Vec<String> }   // folders relative to root, "/" separators
pub struct TreeView { pub hidden: bool, pub width: Option<u32> }
// Settings gains     pub files: FilesSettings
// PaneTab gains      #[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub preview: Option<bool>   // Some(true) for a preview tab; the app never writes false
// SessionTabs gains  #[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub tree: Option<TreeState>
// PaneView gains     #[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub tree: Option<TreeView>
```

- Produces (TS, generated): `FilesSettings`, `TreePosition = "automatic" | "left" | "right"`, `TreeState`, `TreeView`, `PaneTab.preview?: boolean`, `SessionTabs.tree?: TreeState`, `PaneView.tree?: TreeView`.

- [ ] **Step 1: Write the failing tests**

```rust
// settings.rs
#[test] fn files_settings_default_to_the_reference_choices() { /* load(empty home).files == FilesSettings { preview_tabs: true, auto_save: false, tab_indents: true, word_wrap: false, indent_guides: true, whitespace: false, scroll_past_end: false, tab_size: 4, tree_position: TreePosition::Automatic } */ }
#[test] fn files_settings_round_trip() { /* every field changed, saved, loaded equal */ }
#[test] fn one_unreadable_files_value_reads_as_its_default_and_the_rest_is_kept() {
    // {"files": {"auto_save": true, "tab_size": 3, "tree_position": "top", "word_wrap": "yes"}}
    // → auto_save true, tab_size 4, tree_position Automatic, word_wrap false, the rest default
}
#[test] fn validate_refuses_a_tab_size_other_than_2_4_or_8() { /* settings::validate at 284: tab_size 3 → Err("files.tab_size must be 2, 4 or 8") */ }
// layout.rs
#[test] fn a_tab_keeps_preview_and_drops_an_unreadable_one() { /* "preview": true kept; "preview": "yes" → None, the tab kept */ }
#[test] fn a_session_keeps_its_tree_and_drops_bad_folders() {
    // {"root": "C:\\p", "expanded": ["src", "src/a", "", "/abs", "a/../b", "a\\b", 7]} → expanded ["src", "src/a"]
}
#[test] fn a_session_tree_keeps_at_most_500_folders() { /* 600 in, 500 out, the first 500 */ }
#[test] fn an_unreadable_session_tree_is_dropped_and_the_tabs_kept() { /* "tree": 5 and "tree": {"root": 3} */ }
#[test] fn the_window_tree_width_outside_160_to_2048_is_dropped() { /* 100 and 5000 → width None, hidden kept */ }
#[test] fn a_session_with_a_tree_and_no_tabs_is_kept() { /* repair_pane keeps {"tabs": [], "active": null, "tree": {..}}; drops {"tabs": [], "active": null} */ }
#[test] fn validate_pane_refuses_a_bad_tree() {
    // 501 folders: "pane.sessions.{s}.tree.expanded holds at most 500 folders"; a folder "a/../b": "pane.sessions.{s}.tree.expanded holds a folder that isn't relative";
    // width 100: "pane.tree.width must be 160 to 2048"; a session with no tabs and a tree passes
}
```

```ts
// paneRules.test.ts: paneProblem returns the field's name, as it does for "tabs"
it("names the tree's field validate_pane refuses", () => { /* 501 folders → "tree.expanded"; "a/../b" → "tree.expanded"; width 100 → "tree.width" */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 files_settings` (does not compile: `FilesSettings` is unknown).

- [ ] **Step 3: Implement.** `FilesSettings` has a hand-written `Default` (several fields are on). `lenient_files` takes each field alone like `lenient_browser`: a wrong type, a `tab_size` that isn't 2, 4 or 8, or an unknown position reads as the default with a `tracing::warn!` naming the field, never the value. A folder in `expanded` is kept when it is non-empty, at most 4,096 characters, has no `\`, no `:`, doesn't start with `/`, and no component is empty, `.` or `..` (`pub fn tree_folder_ok(f: &str) -> bool` in `layout.rs`, mirrored as `treeFolderOk` in `paneRules.ts` for Task 6). `lenient_session` drops a non-bool `preview`; for `tree` it drops the non-string items of `expanded` first, then drops the whole `tree` if it still doesn't parse (a non-string `root`, a non-object), before the session is parsed. `lenient_pane` does the same for `PaneView.tree`. `repair_pane` filters `expanded` with `tree_folder_ok`, keeps the first 500, drops a width outside 160 to 2048, and its `sessions.retain` at 361 keeps a session whose `tree` is set even with no tabs (`active` then None). `validate_pane` refuses with the messages in the test (its comment at 455 asks it to mirror `repair_pane`), and its active-tab rule at 563 already accepts no tabs with `active` null. Every `PaneTab { .. }`, `SessionTabs { .. }` and `PaneView { .. }` literal in the crates gets the new field.

- [ ] **Step 4: Regenerate and run.** Run: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`, then `cargo test -p marshell-core -j 4 settings::` and `cargo test -p marshell-core -j 4 layout::`, then `pnpm test src/features/appearance src/features/pane`, then `pnpm build`. Expected: all PASS.

- [ ] **Step 5: Commit.** `feat(protocol): settings.json holds the Files settings, and layout.json holds preview tabs and the file tree's state.`

### Task 2: The core lists the tree's folders (worker `effort: high`)

**Files:**
- Modify: `crates/core/src/server/files.rs` (factor `project_root` out of `list` at 609, no change in what `list` returns), `crates/core/src/server/mod.rs` (route beside `/v1/sessions/{tab}/files` at 1350), `crates/protocol/src/api.rs`
- Create: `crates/core/src/server/tree.rs`
- Generated: `TreeAsk.ts`, `TreeRequest.ts`, `TreeReply.ts`, `TreeDir.ts`, `TreeDirState.ts`, `TreeEntry.ts`, `TreeEntryKind.ts`

**Interfaces:**
- Produces (protocol):

```rust
pub struct TreeRequest { pub dirs: Vec<TreeAsk> }                         // 1 to 64
pub struct TreeAsk { pub dir: String, #[ts(type = "number | null")] pub stamp: Option<u64> }   // "" is the root
pub struct TreeReply { pub root: String, pub dirs: Vec<TreeDir> }         // one per ask, in order
pub struct TreeDir { pub dir: String, pub state: TreeDirState, #[ts(type = "number")] pub stamp: u64, pub entries: Vec<TreeEntry>, pub more: u32 }   // u64 as number, or check:generated fails on bigint
#[serde(rename_all = "kebab-case")] pub enum TreeDirState { Listed, Same, Gone, Refused }
pub struct TreeEntry { pub name: String, pub kind: TreeEntryKind, pub ignored: bool }
#[serde(rename_all = "kebab-case")] pub enum TreeEntryKind { File, Folder, Link }
```

- Produces (core): `pub(super) fn project_root(row: Option<&SessionInfo>, reported: Option<PathBuf>) -> Option<(PathBuf, bool)>` in `files.rs` (the root and whether it is a git checkout: the checkout of the launch folder, else the folder the shell reported, else the launch folder, each through `first_folder`); `pub async fn tree(State, Path(tab), Json(TreeRequest)) -> Response` in `tree.rs`. 404 with "This session has no folder." when `project_root` is None; 400 for 0 or over 64 asks.

- [ ] **Step 1: Write the failing tests** in `tree.rs` (a temp folder with `git init`, files made by the test; the route called through the router as `profiles.rs`'s `ask` helper does):

```rust
#[test] fn lists_folders_first_in_natural_order_ignoring_case() { /* b/, A/, file10.txt, File2.txt, a.txt → A, b, a.txt, File2.txt, file10.txt */ }
#[test] fn never_lists_dot_git() {}
#[test] fn marks_ignored_entries_and_everything_in_an_ignored_folder() { /* .gitignore "dist/\n*.log": dist ignored; x.log ignored; asking "dist" marks every entry ignored; a tracked file matching *.log is not ignored */ }
#[test] fn outside_git_nothing_is_ignored() {}
#[test] fn a_folder_unchanged_since_its_stamp_comes_back_same_with_no_entries() {}
#[test] fn refuses_parent_steps_absolute_backslash_colon_and_dot_git_dirs() { /* "..", "a/../..", "/etc", "C:/x", "a\\b", "a:b", ".git", "x/.git" → Refused, the other asks of the same request still answered */ }
#[test] fn a_missing_folder_is_gone() {}
#[test] fn cuts_a_folder_at_5000_and_counts_the_rest() { /* 5,003 files → 5,000 entries, more == 3 */ }
#[test] fn a_symlink_to_a_folder_is_a_link_and_asking_through_it_is_refused() { /* skipped where the test can't make a symlink */ }
#[cfg(windows)] #[test] fn a_junction_to_outside_the_root_is_a_link_and_refused_when_asked() { /* mklink /J made with std::process::Command("cmd", ["/C", "mklink", "/J", ..]) */ }
#[cfg(windows)] #[test] fn a_junction_inside_the_root_is_never_entered() { /* root/a → root/b: "a" listed as a link; asking "a" and "a/x" is Refused; asking "b" lists */ }
#[test] fn dot_git_in_any_case_is_refused() { /* ".GIT", "x/.Git" → Refused */ }
#[test] fn a_link_to_a_file_is_a_file() {}
#[test] fn a_folder_of_5000_ignored_files_answers() { /* 5,000 "*.log" files: the reply comes (no pipe deadlock) and all are ignored */ }
#[test] fn git_runs_only_when_a_folder_was_listed() { /* every ask Same → no check-ignore run (a counter on the runner, or the reply time) */ }
#[test] fn natural_cmp_orders_digits_as_numbers() {}
#[tokio::test] async fn more_than_64_asks_is_a_bad_request() {}
#[tokio::test] async fn a_session_with_no_folder_is_not_found() {}
#[tokio::test] async fn the_tree_is_refused_without_the_bearer() {}
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 server::tree`. Expected: does not compile.

- [ ] **Step 3: Implement.** A dir is checked as text first (the rules in the test's name, with `.git` compared ignoring case), then walked from the root one component at a time with `symlink_metadata`: a component that is a link or a junction (`file_type().is_symlink()`), a missing one (`Gone`) or a file (`Gone`) stops it, so no folder is ever entered through a link, inside the root or out. Then `refused` and `crate::projects::canonical(path)` must start with the canonical root and have no `.git` component in any case (which also catches a short name such as `GIT~1`), else `Refused`. `stamp` is `modified_ms` of the folder; an equal `stamp` in the ask answers `Same`. Entries come from `read_dir`: a name that isn't UTF-8 or is `.git` is left out; a symlink or junction (`file_type().is_symlink()`, which covers both on Windows) is left out when `links_refused`, else `Link` when its target is a folder and `File` when it is a file. The sort is folders and links first, then files, by a natural, case-insensitive compare (`natural_cmp(a: &str, b: &str) -> Ordering`, digits compared as numbers, with its own unit test). In a checkout, and only when at least one folder came back `Listed`, one `git check-ignore -z --stdin` run gets every listed entry's relative path (folders with a trailing `/`) plus each listed folder itself. It goes through a new `crate::git::run_input(dir, args, stdin: Vec<u8>) -> anyhow::Result<std::process::Output>`, which keeps `run`'s no-window flag, pipes stdin, stdout and stderr, writes stdin from its own thread and drains the output with `wait_with_output`, so a long list can't fill a pipe and hang both sides. Exit code 1 means none is ignored. Exit code 128 (a path inside a submodule) runs it again once per listed folder, and a folder whose run still fails gets no marks. An asked folder that is ignored marks all its entries ignored without asking git. Without git on PATH, nothing is ignored.

- [ ] **Step 4: Run them.** Run: `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `cargo test -p marshell-core -j 4 server::tree`, `cargo test -p marshell-core -j 4 server::files`. Expected: PASS (`files` unchanged).

- [ ] **Step 5: Commit.** `feat(core): a session's folders can be listed one level at a time, with git's ignore marks.`

### Task 3: The core searches file contents (worker `effort: high`)

**Files:**
- Create: `crates/core/src/server/search.rs`
- Modify: `crates/core/src/server/mod.rs` (route), `crates/core/src/git.rs` (a streaming runner), `crates/protocol/src/api.rs`
- Generated: `SearchRequest.ts`, `SearchReply.ts`, `SearchFile.ts`, `SearchLine.ts`, `SearchPart.ts`

**Interfaces:**
- Consumes: `project_root` (Task 2).
- Produces (protocol):

```rust
pub struct SearchRequest { pub q: String }
pub struct SearchReply { pub root: String, pub files: Vec<SearchFile>, pub cut: Option<SearchCut> }
#[serde(rename_all = "kebab-case")] pub enum SearchCut { Lines, Time }   // stopped at 1,000 lines, or at 3,000 ms
pub struct SearchFile { pub path: String, pub lines: Vec<SearchLine> }        // relative, "/" separators, sorted by path
pub struct SearchLine { pub line: u32, pub parts: Vec<SearchPart> }           // line from 1
pub struct SearchPart { pub text: String, pub hit: bool }
```

- Produces (core): `POST /v1/sessions/{tab}/search` with the body `SearchRequest { q: String }` (a body, so the text stays out of URLs and logs); 400 "Search for 2 to 200 characters." outside 2..=200 characters, and "Search for text on one line." when `q` holds `\n`, `\r` or `\0`; 404 "This session has no folder."; in `git.rs`:

```rust
pub enum Stopped { Done, Asked, Deadline, Cancelled }
/// Runs git and hands `each` its stdout in chunks as they come. A watchdog thread kills the child at `deadline` or as soon as
/// `cancel` is set, even while git prints nothing; `each` returning false kills it too.
pub fn stream(dir: &Path, args: &[&str], deadline: Instant, cancel: &AtomicBool, each: impl FnMut(&[u8]) -> bool) -> anyhow::Result<Stopped>;
```

`search.rs` parses the chunks with a small state machine for `git grep -n -z`'s `path\0line\0text\n` records (a NUL inside the text, past git's 8,000-byte binary check, is text), and keeps at most 4,096 bytes of a record's text, so a 500 MB minified line is never held whole.

- [ ] **Step 1: Write the failing tests** in `search.rs` (temp repos made by the test):

```rust
#[test] fn finds_plain_text_with_line_numbers() {}
#[test] fn ignores_case_unless_the_text_has_a_capital() { /* "todo" finds TODO and todo; "Todo" finds Todo only */ }
#[test] fn a_regex_character_is_plain_text() { /* "a.b" finds "a.b", not "axb" */ }
#[test] fn skips_ignored_binary_and_big_files() { /* dist/ ignored, a PNG, a 1,048,577-byte text file: none in the reply */ }
#[test] fn finds_untracked_files_that_are_not_ignored() {}
#[test] fn stops_at_1000_lines_and_says_so() { /* 1,200 matching lines → 1,000 lines, cut Some(Lines) */ }
#[test] fn a_record_parser_takes_a_nul_inside_the_text() { /* b"a.txt\x0012\x00x\x00y\n" → path a.txt, line 12, text "x\0y" lossily */ }
#[test] fn a_record_parser_keeps_4096_bytes_of_a_huge_line() {}
#[test] fn a_query_starting_with_a_dash_is_text() { /* "--help" and "-e" are searched for, not read as options */ }
// git.rs gets its first tests module:
#[test] fn stream_stops_when_each_says_so() {}
#[test] fn stream_kills_a_child_that_prints_nothing_at_the_deadline() { /* through `stream`'s inner `stream_cmd(cmd: Command, ..)`, with `powershell -NoProfile -Command Start-Sleep 10` on Windows and `sleep 10` elsewhere: returns Deadline within deadline + 500 ms */ }
#[test] fn stream_kills_the_child_when_cancelled() {}
#[test] fn outside_git_walks_and_skips_node_modules() {}
#[test] fn long_lines_are_cut_around_the_first_hit() { /* a 5,000-char line → text ≤ 241 chars, starts with "…", the hit part present */ }
#[test] fn parts_split_at_every_hit() { /* "a foo b foo" with "foo" → [a ][foo][ b ][foo] */ }
#[tokio::test] async fn a_query_under_2_or_over_200_characters_is_refused() {}
#[tokio::test] async fn search_is_refused_without_the_bearer() {}
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 server::search`.

- [ ] **Step 3: Implement.** Case-sensitive when the text has a character for which `char::is_uppercase` holds. In a checkout: `git grep -I -n -z --no-color --untracked -F [-i] -e <q> --` through `stream` (`-e` makes a text that starts with `-` a pattern, and `--` ends the options), which stops at 1,000 lines (`cut: Lines`) or 3,000 ms (`cut: Time`). A file over 1,048,576 bytes (one `metadata` per file, remembered) gives no lines. Outside a checkout: the walk of `walk_files` (its `SKIPPED_DIRS` and link rule, factored into a shared iterator if needed), each file at most 1,048,576 bytes with no NUL byte in its first 8,000, read as UTF-8 lossily. `hits(line: &str, q: &str, sensitive: bool) -> Vec<(usize, usize)>` compares char by char (lower-casing each char when not sensitive) and returns byte ranges on char boundaries; a line git found but `hits` doesn't gives one part with `hit: false`. The text drops leading whitespace; over 240 characters it becomes the 240 from 40 before the first hit, with "…" at each cut end. The walk also stops at 3,000 ms and 1,000 lines. Runs in `spawn_blocking` with an `Arc<AtomicBool>` cancel flag that a drop guard in the handler sets, so a request the app aborts (a newer search) kills its `git grep` at once; `spawn_blocking` alone would let it run on.

- [ ] **Step 4: Run them.** Run: `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `cargo test -p marshell-core -j 4 server::search`, `cargo test -p marshell-core -j 4 git::`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(core): a session's files can be searched for plain text, with git grep in a checkout.`

### Task 4: The app's tree and search calls, and the Files settings in React

**Files:**
- Create: `src/features/files/filesApi.ts`, `filesApi.test.ts`, `src/features/files/filesSettings.tsx`, `filesSettings.test.ts`
- Modify: `src/features/appearance/settingsSync.ts`, `settingsSync.test.ts`, `src/lib/api/client.ts` (`coreFetch` at 38 and `api` at 54 take an optional last `signal?: AbortSignal`, passed to `fetch`; every caller unchanged)

**Interfaces:**
- Consumes: Tasks 1 to 3's types.
- Produces:

```ts
// filesApi.ts (the shape of previewApi.ts's listFiles: getEndpoint, api, session(tab))
export function listTree(tab: string, asks: TreeAsk[]): Promise<TreeReply | "no-folder">
export function searchFiles(tab: string, q: string, signal: AbortSignal): Promise<SearchReply | "no-folder">
// SettingsSync
get files(): FilesSettings | null
setFiles(change: Partial<FilesSettings>): Promise<void>
// filesSettings.tsx
export const FILES_DEFAULTS: FilesSettings
export function useFilesSettings(): { settings: FilesSettings | null; failed: boolean; set(change: Partial<FilesSettings>): Promise<void> }
```

- [ ] **Step 1: Write the failing tests:** `describe("SettingsSync: files")` mirroring `"SettingsSync: browser"` (null before the first read; one changed field written and the others kept; the same object between changes; a failed read). `filesApi.test.ts`: a 404 reply is `"no-folder"`; the asks go as `{ dirs }`; the search posts `{ q }` (the text after "?", e.g. `{ q: "a b" }`) and passes the signal.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/appearance/settingsSync.test.ts src/features/files`.

- [ ] **Step 3: Implement**, as `browser` and `useBrowserSettings` are built.

- [ ] **Step 4: Run them.** Same command. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the app reads the tree, searches, and reads and writes the Files settings.`

### Task 5: The editor takes the Files settings, and auto save (worker `effort: high`)

**Files:**
- Create: `src/features/editor/editorPrefs.ts`, `editorPrefs.test.ts`, `src/features/editor/indentGuides.ts`, `indentGuides.test.ts`, `src/features/editor/autoSave.ts`, `autoSave.test.ts`
- Modify: `src/features/editor/setup.ts` (`extensions` at 186, `bindings` at 139, `indentOf` at 56), `Editor.tsx` (`indentUnit.of(indentOf(..))` at 110 takes the tab size; reconfigure on change, as `wrap` is), `TextTab.tsx` (`wrapAtStart` at 68; props; the timer), `editorDoc.ts` (the quiet save), `useEditorFile.ts` (`runSave` passes `auto`), `editorApi.ts` (`saveFile` at 33), `Editor.css` (guides, whitespace, forced colors), `src/features/preview/Preview.tsx` (passes the props through to `TextTab`), `crates/core/src/server/save.rs` (`SaveQuery` at 35, `replace_with` at 215), `crates/core/src/userfiles/history.rs` (decision 24)
- Test: `setup.test.ts`, `TextTab.test.tsx`, `editorDoc.test.ts`, `editorApi.test.ts`, the `mod tests` of `save.rs` and `history.rs`

**Interfaces:**
- Consumes: `FilesSettings` (Task 1).
- Produces:

```ts
// editorPrefs.ts
export type EditorPrefs = Pick<FilesSettings, "tab_indents" | "word_wrap" | "indent_guides" | "whitespace" | "scroll_past_end" | "tab_size">;
export const EDITOR_DEFAULTS: EditorPrefs;   // from FILES_DEFAULTS
// setup.ts
export function extensions(o: { readOnly: boolean; wrap: boolean; name: string; appChord: (e: KeyboardEvent) => boolean; prefs: EditorPrefs }): Extension[];
export function reconfigurePrefs(view: EditorView, prefs: EditorPrefs, appChord: (e: KeyboardEvent) => boolean): void;
// indentGuides.ts
export function guideLevels(lines: readonly string[], tabSize: number, unit: number): number[];
export function indentGuides(): Extension;
// autoSave.ts
export type AutoSaveFacts = { on: boolean; dirty: boolean; conflict: boolean; markers: boolean; readOnly: boolean; saving: boolean };   // markers: countConflicts(text) > 0 (conflicts.ts:43)
export function autoSaveDue(f: AutoSaveFacts): boolean;
export const AUTO_SAVE_MS = 1000;
// TextTab / Preview props, optional so the build stays green until Task 14 passes them: prefs?: EditorPrefs (EDITOR_DEFAULTS); autoSave?: boolean (false)
// PreviewHandle (and TextTab's handle) gains autoSave(): void   // saves now when autoSaveDue; Task 14 calls it before the session on screen changes
// TextTab.tsx: wrapAtStart(kind, path, firstLine, all = false)   // all → true
// setup.ts: indentOf(text, language?, tabSize = 4)   // a file with no indent yet gets tabSize spaces
// editorDoc.ts: DocEvent { type: "save"; auto?: true }; Effect { type: "save"; expect; auto?: true }; DocState.sentAuto: boolean
// editorApi.ts: saveFile(path, bytes, expect, auto = false)   // adds "&auto=1"
```

```rust
// save.rs: SaveQuery gains `auto: Option<bool>`; replace_with takes `copy: Copy`
enum Copy { Always, IfOlderThan(u64) }   // the save route passes IfOlderThan(AUTO_COPY_MS) when auto
pub const AUTO_COPY_MS: u64 = 300_000;
// history.rs
pub fn newest_ms(&self, path: &Path) -> anyhow::Result<Option<u64>>;   // created_ms of the newest copy of `path` (same_path), None with none
```

- [ ] **Step 1: Write the failing tests**

```ts
// editorPrefs.test.ts / setup.test.ts
it("tab size sets how wide a tab is drawn", () => { /* extensions({...prefs, tab_size: 2}) → state.tabSize === 2 */ });
it("a file whose indent can't be told takes the tab size in spaces", () => { /* indentOf("", undefined, 2) === "  "; indentOf("a\n    b", undefined, 2) === "    "; indentOf("", "go", 2) === "\t" */ });
it("Tab key indents off leaves Tab unbound", () => { /* bindings with tab_indents false hold no "Tab" binding; true holds indentWithTab */ });
it("indent guides, whitespace and scroll past end are on only when asked", () => { /* each extension is present or absent */ });
// indentGuides.test.ts
it("counts levels from leading spaces and tabs", () => { /* ["a", "  b", "    c", "\tc"] tab 4 unit 2 → [0, 1, 2, 2] */ });
it("a blank line takes the smaller of its neighbours", () => { /* ["  a", "", "    b"] unit 2 → [1, 1, 2] */ });
// autoSave.test.ts
it("is due only when on, dirty, not saving, not in conflict and not read-only", () => {});
it("auto_save_stops_at_a_conflict", () => { /* conflict true → false */ });
it("never_auto_saves_text_that_still_holds_conflict_markers", () => { /* markers true → false: a merge that left conflicts is saved only by Ctrl+S */ });
it("never_auto_saves_read_only", () => {});
// editorDoc.test.ts
it("an auto save that works says nothing and offers no undo", () => { /* save {auto} → effect save {auto: true}; saved → no announce effect, undoOffered false, lastSave null */ });
it("an auto save that fails or meets a conflict says what Ctrl+S would", () => { /* saveFailed / saveConflict after an auto save → the same announce as after a manual one */ });
it("an auto save with the changed-on-disk bar up does nothing and says nothing", () => {});
it("Ctrl+S still says Saved and offers undo", () => {});
// editorApi.test.ts
it("an auto save sends auto=1", () => {});
// TextTab.test.tsx
it("word wrap on wraps code too", () => { /* wrapAtStart("code", "a.ts", undefined, true) === true; false → false; markdown → true either way */ });
```

```rust
// save.rs / history.rs
#[test] fn an_auto_save_takes_no_copy_when_the_newest_is_under_5_minutes_old() { /* a copy taken at now − 299,999 ms → reply snapshot None, the file written, the index unchanged */ }
#[test] fn an_auto_save_takes_a_copy_when_the_newest_is_5_minutes_old() { /* now − 300,000 ms → snapshot Some */ }
#[test] fn an_auto_save_of_a_file_with_no_copies_takes_one() {}
#[test] fn a_save_without_auto_always_takes_a_copy() { /* a copy 1 s old → snapshot Some */ }
#[test] fn newest_ms_reads_the_newest_copy_of_that_file_only() {}
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/editor` and `cargo test -p marshell-core -j 4 server::save`.

- [ ] **Step 3: Implement.** The core first: an auto save reads `newest_ms` and takes its copy only when there is none or it is at least `AUTO_COPY_MS` old, so twenty copies reach back at least 100 minutes of auto saves; a save without `auto` takes one every time, as today. The unsave route and every other caller of `replace_with` pass `Copy::Always`. Then the editor: compartments for tab size, the keymap (rebuilt without `indentWithTab` when `tab_indents` is off), guides, whitespace (`highlightWhitespace()` from `@codemirror/view`) and `scrollPastEnd()`. `indentGuides()` is a `ViewPlugin` that puts a line decoration with `style="--guides: N"` on visible lines from `guideLevels` (unit from `indentOf`'s detected indent, else `tab_size`), and `Editor.css` draws N one-pixel lines `1ch × unit` apart in a `::before`, from a muted token; in forced colors they use `GrayText` with `forced-color-adjust: none`, as do the whitespace marks. Auto save in `TextTab`: a 1,000 ms timer restarted by every change and again when `saving` turns false while the tab is still dirty (keys typed during a save), and a save on `active` turning false, on the window's `blur` and on `visibilitychange` to hidden, each when `autoSaveDue`; each dispatches `{ type: "save", auto: true }`. In `editorDoc.ts`, an auto save with the bar up returns `unchanged` (the bar already says it), `sentAuto` remembers which kind is in flight, and `saved` after an auto save announces nothing and leaves `undoOffered` false and `lastSave` null; `saveFailed` and `saveConflict` read as they do today. A restored draft that makes the tab dirty starts the timer too. Nothing saves on unmount: an unmount may follow "Don't save" in the close dialog, so leaving a session goes through the handle's `autoSave()` instead.

- [ ] **Step 4: Run them.** Run: `pnpm test src/features/editor src/features/preview`, `cargo test -p marshell-core -j 4 server::save`, `cargo test -p marshell-core -j 4 userfiles::history`, then `pnpm build`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(editor): tab size, the Tab key, word wrap, indent guides, whitespace, scroll past end and auto save follow the Files settings.`

### Task 6: Preview tabs, and the tree's state in the pane's reducer

**Files:**
- Modify: `src/features/pane/paneState.ts` (`PaneAction` at 24, `open` at 313, `navigate` at 362, `openBackground` at 343, `paneReducer` at 450), `src/features/pane/SidePane.tsx` (`StripTab`, `tabTitle` at 27, the tab button), `SidePane.css`
- Test: `paneState.test.ts`, `SidePane.test.tsx`

**Interfaces:**
- Consumes: `PaneTab.preview`, `TreeState`, `TreeView` (Task 1).
- Produces:

```ts
// PaneAction gains
| { type: "open"; session: string; project: string; target: OpenTarget; preview?: true }   // `preview` added to the existing open
| { type: "keep"; session: string; paneTab: string }
| { type: "tree"; session: string; tree: TreeState }
| { type: "treeView"; view: Partial<TreeView> }
// paneState.ts
export function previewTab(st: SessionTabs, panel: PanelId): PaneTab | undefined;
// SidePane.tsx
StripTab.preview?: boolean;  SidePaneProps.onKeep?: (id: string) => void;   // optional until Task 14 passes it
```

- [ ] **Step 1: Write the failing tests**

```ts
it("an open with preview adds a preview tab", () => {});
it("a second preview replaces the first in place, keeping its place in the strip", () => {});
it("a preview of a file already open selects that tab and keeps it as it is", () => {});
it("an open without preview of the file in a preview tab keeps the tab", () => {});
it("keep makes a preview tab an ordinary one", () => {});
it("a preview never replaces a kept tab, and each panel has at most one preview", () => { /* a preview HTML file in Browser and a preview .ts in Files live side by side */ });
it("a navigation inside a preview tab keeps it a preview", () => {});
it("marshell open and an agent's file never make a preview", () => { /* openBackground adds no preview tab; a preview tab of the same path loses `preview` (it is kept) */ });
it("tree sets the session's tree; treeView merges the window's", () => {});
it("tree keeps only relative folders and the first 500", () => { /* treeFolderOk from paneRules.ts: "a/../b", "/x", "" dropped; 600 → 500; paneProblem(next) === null */ });
it("treeView clamps the width to 160 to 2048", () => { /* 2500 → 2048; 100 → 160; null stays null; paneProblem(next) === null */ });
it("closing a session's last tab keeps its tree", () => { /* close leaves { tabs: [], active: null, tree } when tree is set, and drops the session when it isn't */ });
it("a session with a tree and no tabs opens its next tab normally", () => {});
// SidePane.test.tsx
it("a preview tab's name is italic and read with \", preview\"", () => { /* class side-pane__tab--preview; text ", preview" */ });
it("a preview tab's tooltip says how to keep it", () => { /* tabTitle → "src/a.ts · Preview: double-click to keep" */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/pane`.

- [ ] **Step 3: Implement.** A preview open replaces the panel's preview tab by setting its `history` to `[entry]`, `index` 0 and a new `id` (so the viewer remounts), at the same place in `tabs`. An open without `preview` of a path shown in a preview tab, and `openBackground` of it, delete that tab's `preview`. The strip's `onDoubleClick` on a tab calls `onKeep`. `close` at 395 keeps the session's entry with `tabs: []` and `active: null` when it has a `tree`, and every reader of `st.tabs` copes with an empty list. A tab with unsaved edits is never replaced: the first edit keeps it (Task 14's per-tab `onDirty` dispatches `keep`), and Task 14 also dispatches `keep` before a preview open when the preview tab's path is unsaved, which covers a draft still loading; the reducer needs no dirty set of its own. Every action here leaves a pane `paneProblem` accepts, so a save of the layout is never refused (the comment at 239 says why that matters).

- [ ] **Step 4: Run them.** Run: `pnpm test src/features/pane`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(pane): a file opened from the tree can be a preview tab, and the pane keeps the tree's state.`

### Task 7: The tree as data: rows, keys, the name filter

**Files:**
- Create: `src/features/files/treeModel.ts`, `treeModel.test.ts`, `src/features/files/treeFilter.ts`, `treeFilter.test.ts`

**Interfaces:**
- Consumes: `TreeEntry`, `TreeDirState` (Task 2).
- Produces:

```ts
export type Listing = { entries: TreeEntry[]; more: number; stamp: number; state: "listed" | "gone" | "refused" };
export type NamePart = { text: string; hit: boolean };
export type TreeRow = { path: string; name: string; kind: "file" | "folder" | "link" | "more" | "empty"; ignored: boolean; level: number; expanded: boolean; loading: boolean; setSize: number; posInSet: number; more?: number; parts?: NamePart[] };
export function childPath(dir: string, name: string): string;                 // ("", "a") → "a"; ("a", "b") → "a/b"
export function ancestors(path: string): string[];                             // "a/b/c.ts" → ["a", "a/b"]
export function relativeTo(root: string, abs: string, windows: boolean): string | null;   // case-insensitive and either separator on Windows; null outside root
export function absoluteOf(root: string, rel: string, windows: boolean): string;          // ("C:\\p", "src/a.ts", true) → "C:\\p\\src\\a.ts"; "" → root; "/" joins elsewhere. Every path the tree opens, pastes, copies or reveals goes through it, so tabs and recent files never mix separators
export function openFolders(rows: readonly TreeRow[]): string[];                         // the open folders now drawn (every ancestor open), in row order
export function visibleRows(listings: ReadonlyMap<string, Listing>, expanded: ReadonlySet<string>): TreeRow[];
export type TreeMove = { type: "focus"; index: number } | { type: "expand" | "collapse"; path: string } | { type: "open"; path: string; keep: boolean } | { type: "filter" } | { type: "menu" } | null;
export function treeKey(e: { key: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean }, rows: readonly TreeRow[], index: number, rtl: boolean): TreeMove;
export function typeAhead(rows: readonly TreeRow[], from: number, typed: string): number | null;
export function nameParts(name: string, query: string): NamePart[];
export function filterRows(files: readonly string[], query: string, expanded: ReadonlySet<string>): { rows: TreeRow[]; matches: number; capped: boolean };
```

- [ ] **Step 1: Write the failing tests**

```ts
it("visible_rows_walk_open_folders_depth_first", () => {});
it("a folder asked but not yet listed shows as loading", () => {});
it("visible_rows_put_a_more_row_after_a_cut_folder", () => { /* more: 25000 → a row kind "more", more 25000 */ });
it("an open empty folder shows one empty row", () => {});
it("set size and position count only real entries", () => {});
it("relativeTo is case-insensitive on Windows and null outside the root", () => {});
it("absoluteOf joins with the system's separator", () => { /* Windows "\\", elsewhere "/", "" gives the root, a root ending in a separator gets no second one */ });
it("openFolders lists only open folders whose parents are open, in row order", () => {});
it("Right opens a closed folder, then moves to its first child; Left closes, then moves to the parent", () => {});
it("in right-to-left Left and Right swap", () => {});
it("Enter opens a file kept and Space opens it as a preview; both toggle a folder", () => {});
it("Home and End, Ctrl+F to the filter, Shift+F10 and the Menu key to the menu", () => {});
it("a link row neither opens nor expands", () => {});
it("type-ahead finds the next name that starts with the letters, wrapping", () => {});
it("nameParts marks every match ignoring case", () => {});
it("the filter keeps matching files inside their folders, opened", () => { /* ["src/a/Button.tsx", "src/b.ts"], "butt" → src, src/a, Button.tsx with parts */ });
it("a matched folder shows closed, and opened lists every file under it", () => {});
it("the filter stops at 500 matches and says it was capped", () => {});
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files/treeModel.test.ts src/features/files/treeFilter.test.ts`.

- [ ] **Step 3: Implement.** Pure functions only; no React.

- [ ] **Step 4: Run them.** Same command. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the file tree's rows, its keys and its name filter, as data.`

### Task 8: The tree on screen, its icons and its polling (worker `effort: high`)

**Files:**
- Create: `src/features/files/FileTree.tsx`, `FileTree.css`, `FileTree.test.tsx`, `FileTree.css.test.ts`, `src/features/files/fileIcons.tsx`, `fileIcons.test.ts`, `src/features/files/useTree.ts`, `useTree.test.ts`, `src/features/files/treeMenu.ts`, `treeMenu.test.ts`, `src/features/files/reveal.ts`, `reveal.test.ts` (moved here from Task 11, since the menu's last row needs its label)

**Interfaces:**
- Consumes: Task 4's `listTree`, Task 7's model.
- Produces:

```ts
// reveal.ts
export function revealLabel(os: Os): string;   // "Show in File Explorer", "Show in Finder", "Show in file manager"
export function revealRefused(path: string): boolean;   // a path that starts with "\\\\" or "//" (a network share): the opener plugin checks no scope and touches the path, so a hand-edited layout.json could make Explorer reach a share
export async function revealFile(path: string, announce: (t: string) => void, reveal = revealItemInDir): Promise<void>;   // refused → "Couldn't show the file: it's on a network share."; a failure → "Couldn't show the file: {reason}"
// fileIcons.tsx
export type IconKind = "folder" | "folder-open" | "code" | "markdown" | "text" | "image" | "pdf" | "web" | "data" | "config" | "archive" | "other" | "link";
export function iconKind(name: string, kind: TreeRow["kind"], expanded: boolean): IconKind;
export function FileIcon({ kind }: { kind: IconKind }): JSX.Element;   // 14 px, stroke currentColor, aria-hidden
// useTree.ts
export const TREE_POLL_MS = 2000, TREE_FULL_MS = 30000;
export type TreeSource = { list(asks: TreeAsk[]): Promise<TreeReply | "no-folder"> };
export function useTree(o: { session: string; source: TreeSource | null; saved: TreeState | undefined; drawn: boolean; onSaved(session: string, next: TreeState): void }): {
  root: string | null; noFolder: boolean; rows: TreeRow[];
  expand(path: string): void; collapse(path: string): void; collapseAll(): void; reveal(path: string): void;   // reveal opens ancestors of a root-relative path
};
export function nextAsks(listings: ReadonlyMap<string, Listing>, rows: readonly TreeRow[], full: boolean): TreeAsk[][];   // pure: the root and openFolders(rows), in row order, never-listed first, cut into requests of at most 64
// treeMenu.ts
export function treeMenuItems(row: TreeRow, o: { pasteTo: { name: string; refused?: string } | null; os: Os }): MenuItem[];
export type TreeChoice = "open" | "paste" | "copy-path" | "copy-relative" | "reveal";
// FileTree.tsx
export function FileTree(p: { rows: TreeRow[]; rootName: string; noFolder: boolean; selectedPath: string | null; focusedPath: string | null; rtl: boolean; onMove(m: TreeMove): void; onFocusRow(path: string): void; onMenu(row: TreeRow, anchor: { x: number; y: number }, opener: HTMLElement): void; onDoubleOpen(path: string): void }): JSX.Element;
```

- [ ] **Step 1: Write the failing tests**

```ts
// fileIcons.test.ts
it("names map to icons", () => { /* main.ts code, README.md markdown, a.png image, b.pdf pdf, i.html web, d.json data, e.csv data, .gitignore config, x.toml config, z.zip archive, notes.txt text, a.bin other */ });
// useTree.test.ts (the pure parts)
it("nextAsks asks the root and every drawn open folder, with stamps unless full", () => {});
it("nextAsks cuts 200 open folders into requests of 64, 64, 64 and 8", () => {});
it("an open folder under a closed one is not asked", () => {});
it("a_poll_waits_for_the_one_in_flight", () => { /* the scheduler function takes `inFlight` and returns no ask; a poll's requests go one after another */ });
it("a_new_root_closes_every_folder", () => { /* a reply whose root differs from saved.root → onSaved(session, { root, expanded: [] }) and every listing cleared */ });
it("a_gone_folder_is_dropped", () => { /* state gone → removed from expanded and from listings */ });
it("a reply that comes after the session changed is dropped", () => { /* the reducer takes replies tagged with the session and root they were asked for */ });
// reveal.test.ts
it("revealLabel per system", () => {});
it("revealFile calls reveal with the path and announces a failure", () => {});
it("revealFile refuses a network path without calling reveal", () => { /* "\\\\server\\share\\a.txt" and "//server/a.txt" */ });
// treeMenu.test.ts
it("a file's menu reads Open, Paste path into api-server, Copy path, Copy relative path, Show in File Explorer", () => {});
it("a folder's menu has no Open", () => {});
it("the paste row is off with the session's reason", () => {});
it("macOS says Show in Finder and Linux Show in file manager", () => {});
// FileTree.test.tsx (renderToStaticMarkup)
it("is a tree named for its root, with levels, sizes and positions", () => { /* role="tree" aria-label="Files in Airport"; treeitem aria-level, aria-setsize, aria-posinset, aria-expanded on folders */ });
it("an ignored row is dimmed and read as ignored", () => { /* data-ignored and ", ignored" */ });
it("a link row is read as a link", () => { /* ", link" */ });
it("the open file is marked selected", () => {});
it("a more row reads And 25,000 more", () => {});
it("no folder reads This session has no folder the tree can show.", () => {});
// FileTree.css.test.ts
it("rows are 24 px and the tree draws no scrollbar", () => {});
it("forced colors give the selected row Highlight and HighlightText", () => {});
it("forced colors draw an ignored row in GrayText", () => {});
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files`.

- [ ] **Step 3: Implement.** `useTree` asks once when drawn, then every 2,000 ms while `drawn`, the document is visible and the window has focus; immediately on window focus and on `expand`; every 30,000 ms and on window focus without stamps. One request at a time; a poll's requests (64 folders each) go one after another. A `Same` folder keeps its listing. A `Gone` or `Refused` folder leaves `expanded` (saved through `onSaved`, debounced 500 ms). The hook holds `session` in each request and each save: a reply or a save for a session that is no longer the hook's is dropped, the debounce is flushed (for its own session) on unmount and when `session` changes, and nothing is set after unmount. Task 14 keys the panel by session as well. `FileTree` renders only the rows in view plus 10 above and below, and always the row with the keyboard, between two spacers, with a roving `tabIndex`. A row: indent of 12 px per level, a chevron for a folder, the icon, the name as `splitName`'s head and tail (`side-pane__name-head` style, middle-shortened), the tooltip the root-relative path. A click on a file calls `onMove({ type: "open", keep: false })`, a double-click `onDoubleOpen`, a click on a folder toggles it, a right-click and Shift+F10 open the menu. A letter typed within 500 ms of the last extends the type-ahead. The "more" and "empty" rows are `aria-disabled`.

- [ ] **Step 4: Run them.** Run: `pnpm test src/features/files`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the file tree draws, follows the disk and has a menu.`

### Task 9: The filter box and the content search's results

**Files:**
- Create: `src/features/files/TreeFilter.tsx`, `TreeFilter.test.tsx`, `src/features/files/SearchResults.tsx`, `SearchResults.test.tsx`, `src/features/files/useSearch.ts`, `useSearch.test.ts`, `src/features/files/filterText.ts`, `filterText.test.ts`
- Modify: `src/features/files/FileTree.css`

**Interfaces:**
- Consumes: `searchFiles` (Task 4), `filterRows`, `nameParts` (Task 7), `quickFiles` (`src/features/preview/quickFiles.ts`, the 10 s cached list).
- Produces:

```ts
export type FilterMode = { kind: "none" } | { kind: "names"; query: string } | { kind: "search"; query: string } | { kind: "short" };   // "short": "?" with under 2 characters
export function filterMode(text: string): FilterMode;
export const SEARCH_DEBOUNCE_MS = 300;
export function useSearch(o: { tab: string | null; query: string | null; run: typeof searchFiles }): { reply: SearchReply | null; busy: boolean; failed: boolean };
export function searchSaid(reply: SearchReply): string;   // "12 matching lines in 4 files." / "1 matching line in 1 file." / "No matches."
export type SearchRow = { kind: "file"; path: string; expanded: boolean; count: number } | { kind: "line"; path: string; line: number; parts: SearchPart[] };
export function searchRows(reply: SearchReply, closed: ReadonlySet<string>): SearchRow[];   // every file open unless closed
type KeyLike = Parameters<typeof treeKey>[0];
export function searchKey(e: KeyLike, rows: readonly SearchRow[], index: number, rtl: boolean): TreeMove;   // the results' keys: Up, Down, Home, End, Right/Left to open and close a file's lines, Enter, Space as in the tree; one roving tabIndex
export function namesNote(r: { matches: number; capped: boolean; listCut: boolean }, query: string): string | null;
```

Copy: placeholder "Filter, or ? to search in files"; the box's name "Filter files"; "Type at least 2 characters to search."; "No file has “{q}”."; "Showing the first 1,000 matching lines." (`cut: lines`); "The search stopped after 3 seconds. Showing what it found." (`cut: time`); "No file or folder name has “{q}”."; "Showing 500 matches. Type more to narrow them."; "Only the first 20,000 files are filtered." when the list was cut; "The search failed. Try again." on an error.

- [ ] **Step 1: Write the failing tests**

```ts
it("filterMode reads names, ? with text, and ? alone", () => { /* "abc" names; "?ab" search "ab"; "?a" short; "" none; "  " none */ });
it("an_older_search_is_aborted", () => { /* a second query aborts the first's signal (useSearch's pure scheduler) */ });
it("a search runs 300 ms after the last key", () => {});
it("searchSaid counts lines and files, singular and plural", () => {});
it("results are a tree of files and their lines, hits in bold", () => { /* role="tree"; file rows aria-level 1, line rows aria-level 2 with "12" and <b> parts; one row has tabIndex 0 */ });
it("the results' keys move, open and close like the tree's", () => { /* searchKey: Down, Up, Home, End; Left on a line goes to its file; Enter on a line opens a preview at that line; Enter on a file keeps it */ });
it("the notes read as the plan says", () => { /* both cuts, both empties, the short text, the failure */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files`.

- [ ] **Step 3: Implement.** Esc in a box with text clears it; in an empty box it calls `onEscape` (the terminal, as Esc in a tab). Down moves to the first row of what is shown. Enter on a result line opens its file at that line as a preview (`{ path, line }`); Enter on a result file opens it kept. The count is announced when a search finishes while the box or the results have the keyboard.

- [ ] **Step 4: Run them.** Same command. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the tree's filter finds names, and "?" searches the files' text.`

### Task 10: The path bar and the Files panel's body

**Files:**
- Create: `src/features/files/PathBar.tsx`, `PathBar.test.tsx`, `src/features/files/pathSegments.ts`, `pathSegments.test.ts`, `src/features/files/FilesBody.tsx`, `FilesBody.css`, `FilesBody.test.tsx`, `FilesBody.css.test.ts`, `src/features/files/treeLayout.ts`, `treeLayout.test.ts`

**Interfaces:**
- Consumes: `TreePosition`, `TreeView` (Task 1).
- Produces:

```ts
// pathSegments.ts
export type Segment = { label: string; folder: string | null };   // folder: root-relative path to reveal; null for the file and for a path outside the project
export function pathSegments(root: string | null, path: string | null, o: { windows: boolean; home: string | null }): Segment[];
// treeLayout.ts
export const TREE = { min: 160, start: 240, max: 2048, fileMin: 240, step: 16, narrow: 480 } as const;
export function treeSide(position: TreePosition, rtl: boolean): "left" | "right";   // automatic: right in LTR, left in RTL
export function treeWidth(saved: number | null | undefined, panelWidth: number): number;   // clamped to [min, panel − fileMin], at least min
export function dividerKey(key: string, width: number, panelWidth: number, side: "left" | "right"): number | null;
export function narrowAfterOpen(how: "click" | "enter" | "double" | "space"): boolean;   // whether the tree stays over the file: only Space, which leaves the keyboard in the tree (decision 9 against 6)
// FilesBody.tsx
export function FilesBody(p: { tree: ReactNode; filter: ReactNode; file: ReactNode; pathBar: ReactNode; shown: boolean; position: TreePosition; width: number | null; panelWidth: number; rtl: boolean; overlayOpen: boolean; onWidth(w: number | null): void; onOverlay(open: boolean): void }): JSX.Element;
// PathBar.tsx
export function PathBar(p: { segments: Segment[]; windows: boolean; treeShown: boolean; onReveal(folder: string): void; onCollapseAll(): void; onToggleTree(): void; onMenu(anchor: { x: number; y: number }, opener: HTMLElement): void }): JSX.Element;
```

Copy: the bar's name "File path"; "Collapse all folders"; the toggle "Show file tree" with `aria-pressed`, its tooltip "Hide file tree" while shown and "Show file tree" while hidden; the divider "Resize the file tree"; the bar's menu "Copy path", "Copy relative path".

- [ ] **Step 1: Write the failing tests**

```ts
it("a file in the project reads root, folders, file, and each folder reveals itself", () => { /* "C:\\dev\\Airport", "C:\\dev\\Airport\\src\\a\\b.ts" → Airport(""), src("src"), a("src/a"), b.ts(null) */ });
it("a file under home starts at ~ and reveals nothing", () => {});
it("a file elsewhere starts at its drive", () => { /* "D:\\x\\y.txt" → D:, x, y.txt */ });
it("with no file the bar shows the root alone", () => {});
it("separators are \\ on Windows and / elsewhere, hidden from the reader", () => {});
it("automatic puts the tree at the window's edge", () => { /* treeSide("automatic", false) "right"; true "left"; "left" stays left in RTL */ });
it("treeWidth keeps the file 240 px and the tree 160 px", () => {});
it("the divider moves 16 px, Home and End go to the limits, toward the file grows the tree", () => {});
it("under 480 px the tree is drawn over the file", () => { /* data-narrow; overlayOpen false hides the tree */ });
it("the divider says its width", () => { /* role separator, aria-valuenow the width, aria-valuemin 160, aria-valuemax panel − 240 */ });
it("in the narrow layout a click, Enter or a double-click hides the tree, and Space keeps it", () => { /* narrowAfterOpen({ keep, space }) → false for click/Enter/double-click, true for Space */ });
it("the DOM order follows the visual order", () => { /* tree first when on the left */ });
it("the toggle is pressed while the tree shows", () => {});
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files`.

- [ ] **Step 3: Implement.** The divider is a focusable `role="separator"` with `aria-orientation="vertical"`, `aria-valuenow` the width, `aria-valuemin` 160, `aria-valuemax` panel − 240, dragged with pointer capture as `PanelStack`'s is, and a double-click goes back to 240 (`onWidth(null)`). In the narrow mode the tree is absolutely placed over the file on its side, Esc in it closes it (`onOverlay(false)`), and it has the same width rule capped at the panel's width less 40 px.

- [ ] **Step 4: Run them.** Same command. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the path bar, and the Files panel lays out its tree beside the file or over it.`

### Task 11: A file the pane can't show, and Show in File Explorer (main thread does the capability line)

**Files:**
- Modify: `src/features/preview/Preview.tsx` (the `"tooLarge"` case at 234, the `"binary"` case at 236, the header at 546), `src-tauri/capabilities/default.json`
- Test: `src/features/preview/Preview.test.tsx`

**Interfaces:**
- Consumes: `revealLabel`, `revealFile` (Task 8).
- Produces: Preview props `os?: Os`, `onReveal?: (path: string) => void` (the button shows only when given) and `pathShown?: boolean` (default false: the Files panel passes true because its path bar shows the name and folder, so the header leaves both out and keeps back, forward and the rest; the Browser panel's HTML viewer passes nothing and its header, with Rendered/Source, the viewport chip and Select region, is unchanged).

- [ ] **Step 1: Write the failing tests:** the binary tab reads "report.docx can't be shown here." with a "Show in File Explorer" button on Windows; the too-large tab keeps its sentence and gets the same button; without `onReveal` neither has a button; with `pathShown` the header has no name or folder; without it the header is as today.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/preview/Preview.test.tsx`.

- [ ] **Step 3: Implement**, and add `"opener:allow-reveal-item-in-dir"` after the `opener:allow-open-url` block in `default.json` (main thread).

- [ ] **Step 4: Run them**, then `cargo build -p marshell-app -j 4` (the capability file is checked at build). Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): a file the pane can't show says so and can be shown in File Explorer.`

### Task 12: The Files ⋯ menu, the palette's commands and Ctrl+Shift+P

**Files:**
- Create: `src/features/files/filesMenu.ts`, `filesMenu.test.ts`
- Modify: `src/lib/keymap.ts` (`open-file` at 95: label "Go to file…", `binding: { code: "KeyP" }`; two new commands after `browse-file`), `src/lib/appShortcuts.ts` (the two new kinds), `src/features/pane/plusMenu.ts` ("Go to file…"), `src/features/pane/EmptyFiles.tsx` ("Go to file…"), and the comments that name the command (`Palette.tsx` 44 and 201, `LivePalette.tsx` 58, `LiveApp.tsx` 390, `appShortcuts.ts` 31, which also loses "Has no key of its own")
- Test: `src/lib/keymap.test.ts` (111 to 117 pin Ctrl+Shift+P and ⌘P as running no command: they now run `open-file`; the test's name changes with it), `src/lib/bindings.test.ts` (152), `src/features/pane/plusMenu.test.ts` (10), `src/features/pane/EmptyPanel.test.tsx` (60 to 65), `src/features/sessions/LiveApp.test.tsx` (744)

**Interfaces:**
- Consumes: `FilesSettings` (Task 1).
- Produces:

```ts
export function filesMenuItems(o: { settings: FilesSettings | null; failed: boolean; treeShown: boolean; os: Os }): MenuItem[];
export type FilesChoice = { kind: "goToFile" } | { kind: "tree" } | { kind: "position"; position: TreePosition } | { kind: "collapse" } | { kind: "setting"; key: "preview_tabs" | "auto_save" | "tab_indents" | "word_wrap" | "indent_guides" | "whitespace" | "scroll_past_end" } | { kind: "tabSize"; size: 2 | 4 | 8 } | null;
export function filesMenuChoice(id: string): FilesChoice;
export function filesSaid(choice: Exclude<FilesChoice, null | { kind: "goToFile" } | { kind: "collapse" }>, on?: boolean): string;
// keymap: { id: "files-tree", label: "Show file tree", group: "Window" }, { id: "files-collapse", label: "Collapse all folders", group: "Window" }
```

The menu, in order: "Go to file…" (shortcut "Ctrl+Shift+P", "⌘P" on macOS); separator; "Show file tree" (checked); "File tree position" (submenu: "Automatic", "Left", "Right", radio); separator; "Collapse all folders"; separator; "Preview tabs", "Auto save", "Tab key indents" (checked); separator; "Word wrap", "Indent guides", "Show whitespace", "Scroll past end" (checked); "Tab size" (submenu: "2", "4", "8", radio). Until the settings are read no row is checked and the setting rows carry "Your settings couldn't be read" as their note when the read failed. Said after a choice: "Show file tree: off", "File tree position: Left", "Auto save: on", "Tab size: 2".

- [ ] **Step 1: Write the failing tests:** the rows and their order; checked states from settings; the radio rows; the notes when unread or failed; `filesMenuChoice` for every id and null for an unknown one; `filesSaid`; `keymap` holds `open-file` on `KeyP` with label "Go to file…" and no conflict (`bindings.test.ts`'s conflict and reserved checks pass for every system); the + menu and the empty state read "Go to file…".

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files/filesMenu.test.ts src/lib src/features/pane`.

- [ ] **Step 3: Implement**, renaming the command's label everywhere the tests show it.

- [ ] **Step 4: Run them.** Run: `pnpm test`. Expected: PASS (the whole suite, since the label moves).

- [ ] **Step 5: Commit.** `feat(files): the Files menu holds the tree's and the editor's settings, and Go to file has Ctrl+Shift+P.`

### Task 13: Settings → Files

**Files:**
- Create: `src/features/files/FilesSection.tsx`, `FilesSection.test.tsx`
- Modify: `src/features/settings/settingsSections.ts` (`files` after `browser`), `src/features/settings/LiveSettings.tsx` (the `case "files"` beside `"browser"` at 144)

Copy (each a title, then its line):
- "Preview tabs": "A file opened from the tree shows in a tab that the next one replaces, until you edit it or double-click it."
- "Auto save": "Saves a file 1 second after you stop typing and when you leave its tab. It stops when the file changed on disk."
- "Tab key indents": "When off, Tab moves to the next control. Esc, then Tab, always leaves the editor."
- "Word wrap": "When off, Markdown and plain text wrap and code doesn't. When on, every file wraps. Alt+Z changes the tab in front."
- "Indent guides": "Draws a line at each indent level."
- "Show whitespace": "Draws spaces as dots and tabs as arrows."
- "Scroll past end": "Lets the last line scroll up to the top."
- "Tab size": "How wide a tab is drawn, and the indent of a file whose own can't be told." Choices "2", "4", "8".
- "File tree position": choices "Automatic" ("At the window's edge"), "Left", "Right".

- [ ] **Step 1: Write the failing tests:** the section lists the nine settings with their lines; a switch shows the saved value; unread settings show no switch on; a failed save says "The setting wasn't saved."

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files/FilesSection.test.tsx src/features/settings`.

- [ ] **Step 3: Implement** with `Switch` and `Choice`, as `BrowserSection` does.

- [ ] **Step 4: Run them.** Same command. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(settings): a Files section holds the Files menu's settings.`

### Task 14: The Files panel in the window (worker `effort: high`)

**Files:**
- Create: `src/features/files/FilesPanel.tsx`, `FilesPanel.test.tsx`, `src/features/files/filesActions.ts`, `filesActions.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx`:
  - `renderPanel`'s Files branch, around 2241 to 2319: the `FilesPanel`, keyed by `paneSession`.
  - The per-tab `onDirty` closure in `renderPanel`, which knows the tab's id (`noteDirty` at 876 takes only `(path, dirty)`): dirty → `keep` for that tab.
  - The strip's `menu` and `onMenu` at 2215 to 2219, where `onMenu` now always calls `setBrowserMenu`: it picks the panel's menu, and Files gets `menu: { label: "Files menu" }` and a `filesMenu` state beside `plusMenu` at 1186.
  - `dropPaths` at 1190: its terminal branch becomes `pasteInto(tabId, paths)`, which the tree uses too.
  - `windowCommand` at 1716 to 1733, beside `case "openFile"`: the two new commands. Their kinds go in `appShortcuts.ts` (Task 12).
  - A `leaveSession` wrapper around the view's `dispatch` (from `useViewState` at 251). A `select` of a session other than `view.panes[0]` first calls `autoSave()` on that session's text tabs, through `textTabs`. Every way to another session goes through it: `pick` at 267, the sidebar, `focus-session-N`, `next-waiting`. A closed session's tabs are gone, so its drafts keep the edits, as they do today.
- Modify: `src/features/pane/focusPane.ts` (`paneFocusTarget`: Files with no tab and the tree shown → the tree's tabbable row)
- Test: `focusPane.test.ts`, `src/features/sessions/LiveApp.test.tsx`

**Interfaces:**
- Consumes: everything above.
- Produces: `FilesPanel` (the path bar, the filter, the tree or the results, and the tab body, laid out by `FilesBody`), and in `filesActions.ts` the pure rules:

```ts
export function treeOpen(o: { previewTabs: boolean; keep: boolean }): { preview?: true };   // the open action's preview flag
export function followTarget(root: string | null, activePath: string | null, windows: boolean): string | null;   // the root-relative path to reveal, or null
export function keepFirst(previewTab: PaneTab | undefined, unsaved: (path: string) => boolean): boolean;
export function leavingSession(action: ViewAction, onScreen: string | undefined): boolean;
```

- [ ] **Step 1: Write the failing tests:**
  - `treeOpen`: the setting off → never preview; keep → never preview.
  - `keepFirst(previewTab, unsaved)`: true when the preview tab's path is unsaved, so `keep` is dispatched before the preview open.
  - `followTarget`: inside the root → the relative path; outside → null.
  - `leavingSession(action, onScreen)`: true only for a `select` of another session.
  - `paneFocusTarget` finds the tree's row in Files with no tab.
  - `FilesPanel` renders the path bar, the filter, and the tree with the file beside it. The strip shows the Files menu button. The Preview gets `pathShown`.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files src/features/pane/focusPane.test.ts`.

- [ ] **Step 3: Implement the wiring.** A tree open dispatches `keep` first when `keepFirst` says so, then `open` with `treeOpen`'s flag and the path from `absoluteOf`. `Enter` and a double-click move the keyboard to the file (`focusPane("files")`), and Space and a click leave it in the tree. In the narrow layout, `narrowAfterOpen` decides whether the tree stays. The keyboard's way to keep an image or PDF preview is Enter on its row in the tree. The per-tab `onDirty(true)` dispatches `keep` for that tab, and a tab's double-click keeps it (`onKeep`). The Files panel passes `prefs`, `autoSave`, `os`, `onReveal` and `pathShown` to its Preview; the Browser panel passes none of them. When the Files panel's selected tab changes, `reveal(followTarget(...))` runs and the row scrolls into view. The tree's state goes through `paneDispatch({ type: "tree", ... })` for the session on screen and the width and hidden flag through `treeView`. "Paste path into <session>" calls `pasteInto`, which says "Pasted main.ts into api-server." and moves the keyboard to that terminal, as a drop does. Copy path and Copy relative path write to the clipboard and say "Copied the path." and "Copied the relative path.". Esc in the tree returns the keyboard to the terminal. In the narrow layout, Esc first closes the tree over the file, and the keyboard goes to the file. `leaveSession` auto-saves before the session on screen changes. The palette's "Show file tree" and "Collapse all folders" act on the Files panel, showing it first when hidden. The Files ⋯ menu and Settings → Files write through `useFilesSettings().set`.

- [ ] **Step 4: Run them**, then `pnpm test` and `pnpm build`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the Files panel shows its tree, opens preview tabs and follows the open file.`

### Task 15: The plan's text follows, then the live check (main thread)

**Files:**
- Modify: `docs/PLAN.md` (the Side pane row at 140, the Files panel's menu and the commands wherever §4 to §7 list them, the announcer examples)
- Create: `docs/research/2026-11-21-file-tree-live-check.md`

- [ ] **Step 1: Bring `docs/PLAN.md`'s running text in line** with decisions 1 to 24. Commit: `docs(plan): the file tree, the path bar and the Files menu are described where the plan describes the pane.`

- [ ] **Step 2: Run what CI runs**, one command per call, in the foreground: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`. Expected: all pass; the counts go in the PR body.

- [ ] **Step 3: Live check** with the `marshell-live-check` skill: a new scratch folder, a scratch git repo in it with a `.gitignore` (`dist/`, `*.log`), a `node_modules` of 6,000 files, a junction to the scratch root's parent, and a 2 MB one-line file; CDP on 9333. No desktop input. Exit criteria, each a row of the note:

| # | Check |
| --- | --- |
| 1 | The Files panel shows the tree at the window's edge with the path bar above; Left and Right move it; under 480 px it covers the file and opening a file hides it. Emulated forced colors: rows, the selected row, guides and icons visible. |
| 2 | The keyboard alone: Modifier+Tab reaches Files; arrows, Home/End, type-ahead, Right/Left, Enter (kept tab, keyboard to the file), Space (preview, keyboard stays), Shift+F10 menu, Ctrl+F to the filter, Esc to the terminal. |
| 3 | `dist/` and `x.log` are dimmed and read "ignored"; `node_modules` shows 5,000 entries and "And 1,000 more"; the junction is a link and doesn't open; `.git` isn't listed. |
| 4 | A file made in the terminal appears within 2 s; a deleted open folder closes; a `cd` outside git in a shell session moves the root and closes every folder. |
| 5 | Filter "butt" shows the matches in opened folders, bold; "?todo" lists lines, "?TODO" fewer; the 2 MB file and a PNG are absent; Enter on a line opens a preview at that line. |
| 6 | A click opens an italic preview; a second click replaces it; typing in it keeps it; a double-click on its tab keeps it; `marshell open` of a previewed file keeps it. |
| 7 | Each menu row changes what it says, survives a restart and matches Settings → Files; Tab size 2 draws tabs 2 wide; Tab key indents off lets Tab leave; indent guides and whitespace draw. |
| 8 | Auto save on: an edit is on disk 1 s later, with nothing said; an agent-side write meanwhile gives the changed-on-disk bar and nothing is overwritten; ten auto saves in a minute add one file-history copy; switching session and switching window save first. |
| 9 | Ctrl+Shift+P opens the palette's file list from the terminal and from a page; the path bar's folder reveals itself; Copy path; Paste path into the session (no Enter). |
| 10 | "Show in File Explorer" opens Explorer with the file selected (the user checks it by eye; no desktop input from the check); a `.docx` tab says it can't be shown. |
| 11 | The tree's open folders and width come back after `restart`; `~/.claude/settings.json`'s modified time is unchanged; only what the check started was stopped. |

- [ ] **Step 4: Write the note** in the skill's format, with every finding and its fix commit, and commit it: `docs(research): the file tree's live check.`

## After the tasks

1. The whole-branch review by `marshell-reviewer` (Opus) with this plan's Review Focus, then one fix pass.
2. `marshell-ship`: the checks, the PR ("What it does", "Live check", "Known limits", "Checks"), a green CI.
3. `pre-merge-review`, then the merge. Delete the ledger.

Known limits to state in the PR: the tree polls (no file watcher), so a change can take 2 s to show and a `.gitignore` edit is seen at the next full read; the name filter searches git's list, so an ignored file isn't found by name; content search is plain text; folder links are never entered; a WSL distro's own files can't be listed (6r-1b decision 29, issue #165); git marks wait for 6r-3.
