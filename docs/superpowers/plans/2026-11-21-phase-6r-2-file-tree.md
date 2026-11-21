# Slice 6r-2: the file tree, the path bar and the Files menu: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (the user's default). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Files panel gains a file tree of the session's project folder (with a name filter and a "?" content search), a path bar, preview tabs, and a ⋯ menu of editor settings that are also on a new Settings → Files page.

**Architecture:** Two new core routes read the disk: `POST /v1/sessions/{tab}/tree` lists asked folders one level deep with git's ignore marks and a per-folder stamp, and `POST /v1/sessions/{tab}/search` runs a plain-text search (`git grep` in a checkout, a capped walk outside one). The app keeps the tree as pure data (`treeModel.ts`) fed by a polling hook, draws it as a virtualized ARIA tree beside the open file, and saves the tree's state per session and its size per window in `layout.json`. Editor settings live in `settings.json` under `files` and reach CodeMirror through compartments.

**Tech Stack:** Rust (`marshell-protocol`, `marshell-core`), React 19 and TypeScript, CodeMirror 6, Vitest (node, `renderToStaticMarkup`), ts-rs, Tauri's opener plugin.

**Spec:** `docs/PLAN.md` §9, "Slice 6r decisions" 1, 3, 12 and 13, and "Slice 6r-2 decisions" 1 to 23. Reference screenshots (mechanics only, not the look): `C:\Users\Home Office\OneDrive\Projects\Marshell\screenshots\Screenshot 2026-11-20 233234.png` (the menu), `233740.png` and `232822.png` (the tree and path bar).

**Who does what:** the main thread writes Task 11's capability line and Task 15 (the plan's text and the live check), and checks every worker's diff and tests before ticking a task. Every other task goes to a `worker` (Sonnet) with the task's text, this header, the Global Constraints and the exact test command; `effort: high` is marked on the task. Each worker is told: touch only the task's files, never `git checkout`, `git restore` or `git stash`, and report the test command's last lines.

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
- Numbers, everywhere: tree folder cap 5,000 entries; at most 64 folders per tree request; at most 500 open folders saved per session; tree width 240 px at first, 160 px at least, 2,048 at most, the file keeps 240 px; narrow below 480 px of panel width; poll every 2,000 ms, a full re-read every 30,000 ms; search text 2 to 200 characters, 300 ms after the last key, at most 1,000 lines, files over 1,048,576 bytes skipped, a line cut to 240 characters, a 3,000 ms budget; name filter at most 500 matches; auto save 1,000 ms after the last key.

## Review Focus

1. **A huge folder, or a slow one** (`node_modules` with 30,000 entries, a drive that answers slowly). Expected: the tree shows 5,000 entries and "And 25,000 more", stays scrollable without lag (virtualized rows), and polls never overlap. Pinned by Task 2's `cuts_a_folder_at_5000_and_counts_the_rest`, Task 7's `visible_rows_put_a_more_row_after_a_cut_folder` and Task 8's `a_poll_waits_for_the_one_in_flight`.
2. **Links and junctions** (pnpm's `node_modules`, a junction to `C:\`, a symlink loop). Expected: a folder link is listed as a link and never entered; asking for a path through one is refused; nothing outside the project folder is listed. Pinned by Task 2's link and junction tests.
3. **The root changes or vanishes under the tree** (a `cd` outside git, a removed worktree, a deleted folder that was open). Expected: a new root starts with every folder closed; a gone folder closes and leaves the saved list; no error is said twice. Pinned by Task 8's `a_new_root_closes_every_folder` and `a_gone_folder_is_dropped`.
4. **A preview tab meets an edit, an agent's write or `marshell open`.** Expected: an edit keeps the tab before anything can replace it; a tab with unsaved edits is never replaced; an open by any other way keeps it. Pinned by Task 6's reducer tests.
5. **Auto save against an agent writing the same file.** Expected: a changed-on-disk conflict stops auto save and nothing on disk is overwritten; a read-only file is never auto-saved. Pinned by Task 5's `auto_save_stops_at_a_conflict` and `never_auto_saves_read_only`.
6. **A content search over minified or binary files, or one typed fast.** Expected: a 5 MB one-line bundle and a PNG are skipped, a long line is cut around its hit, and every older search's `git grep` is killed. Pinned by Task 3's tests and Task 9's `an_older_search_is_aborted`.

---

## File structure

| File | Responsibility |
| --- | --- |
| `crates/protocol/src/api.rs` | `FilesSettings`, `TreePosition`, `TreeState`, `TreeView`, the tree and search request and reply types; `preview` on `PaneTab`, `tree` on `SessionTabs` and `PaneView`. |
| `crates/core/src/settings.rs`, `layout.rs` | Reading and checking the new fields. |
| `crates/core/src/server/tree.rs` (new) | `POST /v1/sessions/{tab}/tree`. |
| `crates/core/src/server/search.rs` (new) | `GET /v1/sessions/{tab}/search`. |
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
// layout.rs
#[test] fn a_tab_keeps_preview_and_drops_an_unreadable_one() { /* "preview": true kept; "preview": "yes" → None, the tab kept */ }
#[test] fn a_session_keeps_its_tree_and_drops_bad_folders() {
    // {"root": "C:\\p", "expanded": ["src", "src/a", "", "/abs", "a/../b", "a\\b", 7]} → expanded ["src", "src/a"]
}
#[test] fn a_session_tree_keeps_at_most_500_folders() { /* 600 in, 500 out, the first 500 */ }
#[test] fn an_unreadable_session_tree_is_dropped_and_the_tabs_kept() { /* "tree": 5 and "tree": {"root": 3} */ }
#[test] fn the_window_tree_width_outside_160_to_2048_is_dropped() { /* 100 and 5000 → width None, hidden kept */ }
#[test] fn validate_pane_refuses_a_bad_tree() { /* 501 folders: "tree.expanded holds at most 500 folders"; width 100: "tree width must be 160 to 2048" */ }
```

```ts
// paneRules.test.ts
it("refuses what validate_pane refuses for the tree", () => { /* the same two messages as the Rust test */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 files_settings` (does not compile: `FilesSettings` is unknown).

- [ ] **Step 3: Implement.** `FilesSettings` has a hand-written `Default` (several fields are on). `lenient_files` takes each field alone like `lenient_browser`: a wrong type, a `tab_size` that isn't 2, 4 or 8, or an unknown position reads as the default with a `tracing::warn!` naming the field, never the value. A folder in `expanded` is kept when it is non-empty, at most 4,096 characters, has no `\`, no `:`, doesn't start with `/`, and no component is empty, `.` or `..`. `lenient_session` drops a non-bool `preview` and an unreadable `tree` before the session is parsed; `lenient_pane` does the same for `PaneView.tree`; `repair_pane` filters and truncates `expanded` and drops a width outside 160 to 2048; `validate_pane` refuses with `"{at}: tree.expanded holds at most 500 folders"` and `"tree width must be 160 to 2048"`. Every `PaneTab { .. }`, `SessionTabs { .. }` and `PaneView { .. }` literal in the crates gets the new field.

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
pub struct TreeAsk { pub dir: String, pub stamp: Option<u64> }           // "" is the root
pub struct TreeReply { pub root: String, pub dirs: Vec<TreeDir> }         // one per ask, in order
pub struct TreeDir { pub dir: String, pub state: TreeDirState, pub stamp: u64, pub entries: Vec<TreeEntry>, pub more: u32 }
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
#[test] fn a_link_to_a_file_is_a_file() {}
#[tokio::test] async fn more_than_64_asks_is_a_bad_request() {}
#[tokio::test] async fn a_session_with_no_folder_is_not_found() {}
#[tokio::test] async fn the_tree_is_refused_without_the_bearer() {}
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 server::tree`. Expected: does not compile.

- [ ] **Step 3: Implement.** A dir is checked as text first (the rules in the test's name), then `root.join(dir)`: `refused`, `links_refused`, and `crate::projects::canonical(path)` must start with the canonical root, else `Refused`; not a folder is `Gone`. `stamp` is `modified_ms` of the folder; an equal `stamp` in the ask answers `Same`. Entries come from `read_dir`: a name that isn't UTF-8 or is `.git` is left out; a symlink or junction (`file_type().is_symlink()`, which covers both on Windows) is left out when `links_refused`, else `Link` when its target is a folder and `File` when it is a file. The sort is folders and links first, then files, by a natural, case-insensitive compare (`natural_cmp(a: &str, b: &str) -> Ordering`, digits compared as numbers, with its own unit test). In a checkout, one `git check-ignore -z --stdin` run (through a new `crate::git::run_input(dir, args, stdin: &[u8])`, which keeps `run`'s no-window flag) gets every listed entry's relative path, folders with a trailing `/`, plus each asked folder itself; exit code 1 means none is ignored. An asked folder that is ignored marks all its entries ignored without asking git. Without git on PATH, nothing is ignored.

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
pub struct SearchReply { pub root: String, pub files: Vec<SearchFile>, pub truncated: bool }
pub struct SearchFile { pub path: String, pub lines: Vec<SearchLine> }        // relative, "/" separators, sorted by path
pub struct SearchLine { pub line: u32, pub parts: Vec<SearchPart> }           // line from 1
pub struct SearchPart { pub text: String, pub hit: bool }
```

- Produces (core): `POST /v1/sessions/{tab}/search` with the body `SearchRequest { q: String }` (a body, so the text stays out of URLs and logs); 400 "Search for 2 to 200 characters." outside 2..=200 characters; 404 "This session has no folder."; `pub fn stream(dir: &Path, args: &[&str], each: impl FnMut(&[u8]) -> bool) -> anyhow::Result<bool>` in `git.rs` (calls `each` per NUL- or newline-ended record as `git grep -z` writes them, kills the child when `each` returns false, returns whether it stopped early).

- [ ] **Step 1: Write the failing tests** in `search.rs` (temp repos made by the test):

```rust
#[test] fn finds_plain_text_with_line_numbers() {}
#[test] fn ignores_case_unless_the_text_has_a_capital() { /* "todo" finds TODO and todo; "Todo" finds Todo only */ }
#[test] fn a_regex_character_is_plain_text() { /* "a.b" finds "a.b", not "axb" */ }
#[test] fn skips_ignored_binary_and_big_files() { /* dist/ ignored, a PNG, a 1,048,577-byte text file: none in the reply */ }
#[test] fn finds_untracked_files_that_are_not_ignored() {}
#[test] fn stops_at_1000_lines_and_says_so() { /* 1,200 matching lines → 1,000 lines, truncated */ }
#[test] fn outside_git_walks_and_skips_node_modules() {}
#[test] fn long_lines_are_cut_around_the_first_hit() { /* a 5,000-char line → text ≤ 241 chars, starts with "…", the hit part present */ }
#[test] fn parts_split_at_every_hit() { /* "a foo b foo" with "foo" → [a ][foo][ b ][foo] */ }
#[tokio::test] async fn a_query_under_2_or_over_200_characters_is_refused() {}
#[tokio::test] async fn search_is_refused_without_the_bearer() {}
```

- [ ] **Step 2: Run and see them fail.** Run: `cargo test -p marshell-core -j 4 server::search`.

- [ ] **Step 3: Implement.** Case-sensitive when the text has a character for which `char::is_uppercase` holds. In a checkout: `git grep -I -n -z --no-color --untracked -F [-i] -e <q>` through `stream`, which stops at 1,000 lines or 3,000 ms. A file over 1,048,576 bytes (one `metadata` per file, remembered) gives no lines. Outside a checkout: the walk of `walk_files` (its `SKIPPED_DIRS` and link rule, factored into a shared iterator if needed), each file at most 1,048,576 bytes with no NUL byte in its first 8,000, read as UTF-8 lossily. `hits(line: &str, q: &str, sensitive: bool) -> Vec<(usize, usize)>` compares char by char (lower-casing each char when not sensitive) and returns byte ranges on char boundaries; a line git found but `hits` doesn't gives one part with `hit: false`. The text drops leading whitespace; over 240 characters it becomes the 240 from 40 before the first hit, with "…" at each cut end. Runs in `spawn_blocking`; the child is killed if the request is dropped (`kill_on_drop` or the stop flag).

- [ ] **Step 4: Run them.** Run: `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `cargo test -p marshell-core -j 4 server::search`, `cargo test -p marshell-core -j 4 git::`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(core): a session's files can be searched for plain text, with git grep in a checkout.`

### Task 4: The app's tree and search calls, and the Files settings in React

**Files:**
- Create: `src/features/files/filesApi.ts`, `filesApi.test.ts`, `src/features/files/filesSettings.tsx`, `filesSettings.test.ts`
- Modify: `src/features/appearance/settingsSync.ts`, `settingsSync.test.ts`

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
- Modify: `src/features/editor/setup.ts` (`extensions` at 186, `bindings` at 139, `indentOf` at 56), `Editor.tsx` (reconfigure on change, as `wrap` is at 143), `TextTab.tsx` (`wrapAtStart` at 68; props), `Editor.css` (guides, whitespace, forced colors), `src/features/preview/Preview.tsx` (passes the props through to `TextTab`)
- Test: `setup.test.ts`, `TextTab.test.tsx`

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
export type AutoSaveFacts = { on: boolean; dirty: boolean; conflict: boolean; readOnly: boolean; saving: boolean };
export function autoSaveDue(f: AutoSaveFacts): boolean;
export const AUTO_SAVE_MS = 1000;
// TextTab / Preview props: prefs: EditorPrefs; autoSave: boolean
// PreviewHandle (and TextTab's handle) gains autoSave(): void   // saves now when autoSaveDue; Task 14 calls it before the session on screen changes
// TextTab.tsx: wrapAtStart(kind, path, firstLine, all: boolean)   // all → true
```

- [ ] **Step 1: Write the failing tests**

```ts
// editorPrefs.test.ts / setup.test.ts
it("tab size sets how wide a tab is drawn", () => { /* extensions({...prefs, tab_size: 2}) → state.tabSize === 2 */ });
it("a file whose indent can't be told takes the tab size in spaces", () => { /* indentOf("", "x.ts", 2) === "  "; a file indented with 4 spaces keeps "    " */ });
it("Tab key indents off leaves Tab unbound", () => { /* bindings with tab_indents false hold no "Tab" binding; true holds indentWithTab */ });
it("indent guides, whitespace and scroll past end are on only when asked", () => { /* each extension is present or absent */ });
// indentGuides.test.ts
it("counts levels from leading spaces and tabs", () => { /* ["a", "  b", "    c", "\tc"] tab 4 unit 2 → [0, 1, 2, 2] */ });
it("a blank line takes the smaller of its neighbours", () => { /* ["  a", "", "    b"] unit 2 → [1, 1, 2] */ });
// autoSave.test.ts
it("is due only when on, dirty, not saving, not in conflict and not read-only", () => {});
it("auto_save_stops_at_a_conflict", () => { /* conflict true → false */ });
it("never_auto_saves_read_only", () => {});
// TextTab.test.tsx
it("word wrap on wraps code too", () => { /* wrapAtStart("code", "a.ts", undefined, true) === true; false → false; markdown → true either way */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/editor`.

- [ ] **Step 3: Implement.** Compartments for tab size, the keymap (rebuilt without `indentWithTab` when `tab_indents` is off), guides, whitespace (`highlightWhitespace()` from `@codemirror/view`) and `scrollPastEnd()`. `indentGuides()` is a `ViewPlugin` that puts a line decoration with `style="--guides: N"` on visible lines from `guideLevels` (unit from `indentOf`'s detected indent, else `tab_size`), and `Editor.css` draws N one-pixel lines `1ch × unit` apart in a `::before`, from a muted token; in forced colors they use `GrayText` with `forced-color-adjust: none`, as do the whitespace marks. Auto save in `TextTab`: a 1,000 ms timer restarted by every change, and a save on `active` turning false, on the window's `blur` and on `visibilitychange` to hidden, each when `autoSaveDue`. An auto save that works says nothing; one that fails or meets a conflict says what Ctrl+S's would. Closing a tab with unsaved edits still asks. Nothing saves on unmount: an unmount may follow "Don't save" in the close dialog, so leaving a session goes through the handle's `autoSave()` instead.

- [ ] **Step 4: Run them.** Run: `pnpm test src/features/editor src/features/preview`. Expected: PASS.

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
StripTab.preview?: boolean;  SidePaneProps.onKeep(id: string): void;
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
it("marshell open and an agent's file never make a preview", () => { /* openBackground keeps preview absent, and keeps a preview tab of the same path */ });
it("tree sets the session's tree; treeView merges the window's", () => {});
// SidePane.test.tsx
it("a preview tab's name is italic and read with \", preview\"", () => { /* class side-pane__tab--preview; text ", preview" */ });
it("a preview tab's tooltip says how to keep it", () => { /* tabTitle → "src/a.ts · Preview: double-click to keep" */ });
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/pane`.

- [ ] **Step 3: Implement.** A preview open replaces the panel's preview tab by setting its `history` to `[entry]`, `index` 0 and a new `id` (so the viewer remounts), at the same place in `tabs`. The strip's `onDoubleClick` on a tab calls `onKeep`. A tab with unsaved edits is never a preview, because the first edit keeps it: `LiveApp`'s `noteDirty(true)` dispatches `keep` (Task 14 wires it), so the reducer needs no dirty set of its own.

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
- Create: `src/features/files/FileTree.tsx`, `FileTree.css`, `FileTree.test.tsx`, `FileTree.css.test.ts`, `src/features/files/fileIcons.tsx`, `fileIcons.test.ts`, `src/features/files/useTree.ts`, `useTree.test.ts`, `src/features/files/treeMenu.ts`, `treeMenu.test.ts`

**Interfaces:**
- Consumes: Task 4's `listTree`, Task 7's model.
- Produces:

```ts
// fileIcons.tsx
export type IconKind = "folder" | "folder-open" | "code" | "markdown" | "text" | "image" | "pdf" | "web" | "data" | "config" | "archive" | "other" | "link";
export function iconKind(name: string, kind: TreeRow["kind"], expanded: boolean): IconKind;
export function FileIcon({ kind }: { kind: IconKind }): JSX.Element;   // 14 px, stroke currentColor, aria-hidden
// useTree.ts
export const TREE_POLL_MS = 2000, TREE_FULL_MS = 30000;
export type TreeSource = { list(asks: TreeAsk[]): Promise<TreeReply | "no-folder"> };
export function useTree(o: { source: TreeSource | null; saved: TreeState | undefined; drawn: boolean; onSaved(next: TreeState): void }): {
  root: string | null; noFolder: boolean; rows: TreeRow[];
  expand(path: string): void; collapse(path: string): void; collapseAll(): void; reveal(path: string): void;   // reveal opens ancestors of a root-relative path
};
export function nextAsks(listings: ReadonlyMap<string, Listing>, expanded: ReadonlySet<string>, full: boolean): TreeAsk[];   // pure, for the tests
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
it("nextAsks asks the root and every open folder, with stamps unless full", () => {});
it("nextAsks never asks more than 64 folders", () => {});
it("a_poll_waits_for_the_one_in_flight", () => { /* the scheduler function takes `inFlight` and returns no ask */ });
it("a_new_root_closes_every_folder", () => { /* a reply whose root differs from saved.root → onSaved({ root, expanded: [] }) */ });
it("a_gone_folder_is_dropped", () => { /* state gone → removed from expanded and from listings */ });
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
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files`.

- [ ] **Step 3: Implement.** `useTree` asks once when drawn, then every 2,000 ms while `drawn`, the document is visible and the window has focus; immediately on window focus and on `expand`; every 30,000 ms and on window focus without stamps. One request at a time. A `Same` folder keeps its listing. A `Gone` or `Refused` folder leaves `expanded` (saved through `onSaved`, debounced 500 ms). `FileTree` renders only the rows in view plus 10 above and below, and always the row with the keyboard, between two spacers, with a roving `tabIndex`. A row: indent of 12 px per level, a chevron for a folder, the icon, the name as `splitName`'s head and tail (`side-pane__name-head` style, middle-shortened), the tooltip the root-relative path. A click on a file calls `onMove({ type: "open", keep: false })`, a double-click `onDoubleOpen`, a click on a folder toggles it, a right-click and Shift+F10 open the menu. A letter typed within 500 ms of the last extends the type-ahead. The "more" and "empty" rows are `aria-disabled`.

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
export function namesNote(r: { matches: number; capped: boolean; listCut: boolean }, query: string): string | null;
```

Copy: placeholder "Filter, or ? to search in files"; the box's name "Filter files"; "Type at least 2 characters to search."; "No file has “{q}”."; "Showing the first 1,000 matching lines."; "No file or folder name has “{q}”."; "Showing 500 matches. Type more to narrow them."; "Only the first 20,000 files are filtered." when the list was cut; "The search failed. Try again." on an error.

- [ ] **Step 1: Write the failing tests**

```ts
it("filterMode reads names, ? with text, and ? alone", () => { /* "abc" names; "?ab" search "ab"; "?a" short; "" none; "  " none */ });
it("an_older_search_is_aborted", () => { /* a second query aborts the first's signal (useSearch's pure scheduler) */ });
it("a search runs 300 ms after the last key", () => {});
it("searchSaid counts lines and files, singular and plural", () => {});
it("results are a tree of files and their lines, hits in bold", () => { /* role="tree"; file rows aria-level 1, line rows aria-level 2 with "12" and <b> parts */ });
it("the notes read as the plan says", () => {});
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
it("the DOM order follows the visual order", () => { /* tree first when on the left */ });
it("the toggle is pressed while the tree shows", () => {});
```

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files`.

- [ ] **Step 3: Implement.** The divider is a `role="separator"` with `aria-orientation="vertical"`, `aria-valuemin` 160, `aria-valuemax` panel − 240, dragged with pointer capture as `PanelStack`'s is, and a double-click goes back to 240 (`onWidth(null)`). In the narrow mode the tree is absolutely placed over the file on its side, Esc in it closes it (`onOverlay(false)`), and it has the same width rule capped at the panel's width less 40 px.

- [ ] **Step 4: Run them.** Same command. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the path bar, and the Files panel lays out its tree beside the file or over it.`

### Task 11: A file the pane can't show, and Show in File Explorer (main thread does the capability line)

**Files:**
- Modify: `src/features/preview/Preview.tsx` (the `"binary"` case at 236), `src-tauri/capabilities/default.json`
- Create: `src/features/files/reveal.ts`, `reveal.test.ts`
- Test: `src/features/preview/Preview.test.tsx`

**Interfaces:**
- Produces: `export function revealLabel(os: Os): string` ("Show in File Explorer", "Show in Finder", "Show in file manager"); `export async function revealFile(path: string, announce: (t: string) => void, reveal = revealItemInDir): Promise<void>` (announces "Couldn't show the file: {reason}" on a failure).

- [ ] **Step 1: Write the failing tests:** the binary tab reads "report.docx can't be shown here." with a "Show in File Explorer" button on Windows; `revealLabel` per system; `revealFile` calls `reveal` with the path and announces a failure.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/preview/Preview.test.tsx src/features/files/reveal.test.ts`.

- [ ] **Step 3: Implement**, and add `"opener:allow-reveal-item-in-dir"` after the `opener:allow-open-url` block in `default.json` (main thread).

- [ ] **Step 4: Run them**, then `cargo build -p marshell-app -j 4` (the capability file is checked at build). Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): a file the pane can't show says so and can be shown in File Explorer.`

### Task 12: The Files ⋯ menu, the palette's commands and Ctrl+Shift+P

**Files:**
- Create: `src/features/files/filesMenu.ts`, `filesMenu.test.ts`
- Modify: `src/lib/keymap.ts` (`open-file` at 95: label "Go to file…", `binding: { code: "KeyP" }`; two new commands after `browse-file`), `src/lib/appShortcuts.ts` (the two new kinds), `src/features/pane/plusMenu.ts` ("Go to file…"), `src/features/pane/EmptyFiles.tsx` ("Go to file…"), every test that pins "Open file…" for this command (`rg -n "Open file…" src`)
- Test: `src/lib/keymap.test.ts`, `src/lib/bindings.test.ts`, `src/features/pane/plusMenu.test.ts`, `EmptyPanel.test.tsx`

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
- Modify: `src/features/sessions/LiveApp.tsx` (`renderPanel`'s Files branch around 2241 to 2319; `noteDirty`; the menu state beside `plusMenu` at 1186; `dropPaths` at 1190: its terminal branch becomes `pasteInto(tabId, paths)`, used by the tree too; the palette's `case` list beside `"panel-files"` at 1604), `src/features/pane/focusPane.ts` (`paneFocusTarget`: Files with no tab and the tree shown → the tree's tabbable row), `src/features/pane/SidePane.tsx` (Files gets `menu: { label: "Files menu" }`)
- Test: `focusPane.test.ts`

**Interfaces:**
- Consumes: everything above.
- Produces: `FilesPanel` (the path bar, the filter, the tree or the results, and the tab body, laid out by `FilesBody`), and in `filesActions.ts` the pure rules:

```ts
export function treeOpen(o: { previewTabs: boolean; keep: boolean }): { preview?: true };   // the open action's preview flag
export function followTarget(root: string | null, activePath: string | null, windows: boolean): string | null;   // the root-relative path to reveal, or null
```

- [ ] **Step 1: Write the failing tests:** `treeOpen` (setting off → never preview; keep → never preview); `followTarget` (inside the root → relative path; outside → null); `paneFocusTarget` finds the tree's row in Files with no tab; `FilesPanel` renders the path bar, the filter and the tree with the file beside it, and the Files menu button in the strip.

- [ ] **Step 2: Run and see them fail.** Run: `pnpm test src/features/files src/features/pane/focusPane.test.ts`.

- [ ] **Step 3: Implement the wiring.** A tree open dispatches `open` with `treeOpen`'s flag; `Enter` and a double-click move the keyboard to the file (`focusPane("files")`), Space and a click leave it in the tree. `noteDirty(true)` dispatches `keep` for that tab. A tab's double-click keeps it. When the Files panel's selected tab changes, `reveal(followTarget(...))` runs and the row scrolls into view. The tree's state goes through `paneDispatch({ type: "tree", ... })` for the session on screen and the width and hidden flag through `treeView`. "Paste path into <session>" calls `pasteInto`, which says "Pasted main.ts into api-server." and moves the keyboard to that terminal, as a drop does. Copy path and Copy relative path write to the clipboard and say "Copied the path." and "Copied the relative path.". Esc in the tree returns the keyboard to the terminal; in the narrow mode it first closes the tree over the file and the keyboard goes to the file. Before the session on screen changes, every text tab of the session being left gets `autoSave()` (through `textTabs`), so auto save covers leaving the session. The palette's "Show file tree" and "Collapse all folders" act on the Files panel, showing it first when hidden. The Files ⋯ menu and Settings → Files write through `useFilesSettings().set`.

- [ ] **Step 4: Run them**, then `pnpm test` and `pnpm build`. Expected: PASS.

- [ ] **Step 5: Commit.** `feat(files): the Files panel shows its tree, opens preview tabs and follows the open file.`

### Task 15: The plan's text follows, then the live check (main thread)

**Files:**
- Modify: `docs/PLAN.md` (the Side pane row at 140, the Files panel's menu and the commands wherever §4 to §7 list them, the announcer examples)
- Create: `docs/research/2026-11-21-file-tree-live-check.md`

- [ ] **Step 1: Bring `docs/PLAN.md`'s running text in line** with decisions 1 to 23. Commit: `docs(plan): the file tree, the path bar and the Files menu are described where the plan describes the pane.`

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
| 8 | Auto save on: an edit is on disk 1 s later; an agent-side write meanwhile gives the changed-on-disk bar and nothing is overwritten. |
| 9 | Ctrl+Shift+P opens the palette's file list from the terminal and from a page; the path bar's folder reveals itself; Copy path; Paste path into the session (no Enter). |
| 10 | "Show in File Explorer" opens Explorer with the file selected (the user checks it by eye; no desktop input from the check); a `.docx` tab says it can't be shown. |
| 11 | The tree's open folders and width come back after `restart`; `~/.claude/settings.json`'s modified time is unchanged; only what the check started was stopped. |

- [ ] **Step 4: Write the note** in the skill's format, with every finding and its fix commit, and commit it: `docs(research): the file tree's live check.`

## After the tasks

1. The whole-branch review by `marshell-reviewer` (Opus) with this plan's Review Focus, then one fix pass.
2. `marshell-ship`: the checks, the PR ("What it does", "Live check", "Known limits", "Checks"), a green CI.
3. `pre-merge-review`, then the merge. Delete the ledger.

Known limits to state in the PR: the tree polls (no file watcher), so a change can take 2 s to show and a `.gitignore` edit is seen at the next full read; the name filter searches git's list, so an ignored file isn't found by name; content search is plain text; folder links are never entered; a WSL distro's own files can't be listed (6r-1b decision 29, issue #165); git marks wait for 6r-3.
