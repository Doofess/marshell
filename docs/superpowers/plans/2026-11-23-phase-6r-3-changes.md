# Slice 6r-3: the Changes panel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A third side-pane panel, Changes, shows the session's checkout as one diff against main. The person comments on lines and ranges, then sends the comments to the agent as one pasted message without Enter, or asks the agent to review with its own command. The Files tree marks changed files.

**Architecture:**
- **Core.** Every git run goes through a sandbox git folder of Marshell's own (`gitbox`), so no repository config is ever read or run (spike S15). Marshell reads refs from the repository's files itself.
- **Engine.** A per-checkout cache runs a cheap check every poll (a raw file list plus file stats) and reads the whole diff only when the check's stamp moves. The tree's marks come from the same check.
- **Frontend.** The panel stack grows to three panels. Changes is a tabless panel: one `role="tree"` list (file headers, then lines), inline comment boxes, and a Send bar. Drafts and folds are saved with the session's tabs in `layout.json`.

**Tech Stack:** Rust (axum core, git 2.x via `git::stream`), React 19 + TypeScript, Shiki (existing tokenizer), Vitest (node, `renderToStaticMarkup`, source pins).

**Spec:**
- `docs/PLAN.md` §9, "Slice 6r-3 decisions (2026-11-23)" 1 to 22, and "Slice 6r decisions" 1, 4, 7 and 9.
- `docs/spikes/S15-changes-git-sandbox.md`, the sandbox's proof.
- How the code this slice touches was built: the 6r-2 and 6t plans (`docs/superpowers/plans/2026-11-21-phase-6r-2-file-tree.md` and `2026-11-22-phase-6t-files-leftovers.md`).

## Global Constraints

- **Git runs for Changes, and for the tree's marks, only through `gitbox::GitBox`.** That means:
  - `GIT_DIR` is the sandbox, and `GIT_INDEX_FILE` the sandbox's own copy of the repository's index;
  - `GIT_CONFIG_NOSYSTEM=1`, and `GIT_CONFIG_GLOBAL` names an empty file;
  - `GIT_NO_LAZY_FETCH=1`;
  - every inherited `GIT_*` variable is removed first, for the config reads too;
  - `-c core.fsmonitor=false -c protocol.allow=never -c core.quotepath=off`, plus the allowlisted values as `-c` pairs;
  - `--no-ext-diff --no-textconv --ignore-submodules=all` on every diff, `uncommitted`'s included.

  No other git call is added. The task Review sheet (`tasks.rs`) is unchanged.
- **No file in a repository is ever written.** `git diff` refreshes and rewrites the index it is given whenever a file's stat moved and its content didn't (`refresh_index_quietly`), and `GIT_OPTIONAL_LOCKS=0` doesn't stop that. So git only ever gets the sandbox's copy of the index.
- **Every git run has a deadline:** the check `CHECK_BUDGET` = 2 s, the full read `READ_BUDGET` = 5 s, and the marks the rest of the tree's 2 s budget, at most 1 s.
- **Marshell never fetches, never writes to a repository, and never sends Enter.** Comments and review commands are typed or pasted and wait for the person.
- **Limits:**
  - The diff is cut at 4 MiB (`tasks::MAX_DIFF`) on a file boundary.
  - An untracked file over 256 KiB, or one with a NUL in its first 8 KiB, gets a note and no diff.
  - At most 200 untracked files get a section (`MAX_NEW_FILES`).
  - The tree gets at most 2,000 marks.
  - 50 comments per session, 2,000 characters each, with no control character but `\n` and `\t`; a quote holds 3 lines of 200 characters (code points). Control characters in quoted code become U+FFFD.
  - A pasted message holds 16,000 characters at most (lower if S16 says so), and a comments file 1 MiB.
  - `PUT /v1/layout/view` takes bodies up to 8 MiB, since drafts travel in it.
- **Exact copy:**
  - `…` is U+2026 and `−` is U+2212.
  - Thousands take a comma (`toLocaleString("en-US")`).
  - Sentences end with a period, as in the copy tables of Tasks 9, 11 and 12.
- **Every UI change works with the keyboard alone and in forced colours.** Overlays call `useCoverPage` (the line menu uses `ContextMenu`, which already does). Colours come from `src/styles/` tokens only, following the good-css rules (logical properties, `oklch`/`color-mix`, motion only inside `prefers-reduced-motion: no-preference`).
- **Multi-CLI.** Review commands and the paste-or-file choice live in vendor code (`profiles/detect.rs`), one arm per CLI. Antigravity and manifest agents get the written request. A session with no agent gets no review action.
- **Checks and commits:**
  - Rust tests run `cargo test -p marshell-core -j 4` (never more jobs). More than one name filter goes after `--` (`cargo test -p marshell-core -j 4 -- gitbox:: git::`). Run `cargo fmt --all` before every Rust commit.
  - `pnpm check:generated` passes after every protocol change; the regenerated `src/generated/` goes in the same commit.
  - Commit messages read `type(scope): sentence.` and end with the two attribution lines the session gives.
  - `git add` and `git commit -F <file>` run as separate commands.

## Review Focus

1. **A hostile checkout the shell moved into, and a busy one.**
   - Its `.git/config` sets `core.fsmonitor`, a clean filter on a changed file, `diff.external`, and an `[include]` of a missing network path. With Changes and the tree polling it for a minute, none of those programs starts and the include is never opened.
   - An agent rewrites files with the same bytes while Changes polls: the repository's index keeps its bytes and its modified time, so the agent's own `git commit` never meets Marshell's `index.lock`.
   - Pinned in Task 4: a marker file stays empty; `git config --list --show-origin` under the sandbox lists only the sandbox and the command line; and `index_is_never_written`. Also in the live check, row 2.
2. **A refresh while a comment is being written.** The agent rewrites the file, so lines above move. The open box keeps its text and focus, and its draft re-anchors to the moved lines. Pinned in Task 9 (`place` tests) and Task 12 (the box's key stays stable across a new reply).
3. **A CLI that has bracketed paste off when Send runs** (it sits in a picker or a sub-screen). The comments go by file, as one line, and no newline is ever typed. Pinned in Task 7 (`comments_go_by_file_when_bracketed_paste_is_off`, through the fake agent's `capture-input-plain`).
4. **A huge change:**
   - a lockfile with 50,000 changed lines;
   - 300 new files;
   - a 3 MB minified bundle;
   - a diff over 4 MiB.

   The lockfile and the bundle start folded, at most 10,000 rows are open at first, 200 new files get sections with "And 100 more new files aren't shown.", and the cut falls on a file boundary with its note. Pinned in Task 5 and Task 9.
5. **Removed-only and mixed ranges, a comment on a file the agent then reverted, and a CRLF file under `core.autocrlf=true`.**
   - The message cites old numbers with "(removed lines)" for a removed-only range.
   - The orphan comment stays visible under its file with "No changes in this file now." and "The line changed".
   - The CRLF file shows no change.

   Pinned in Task 9 and Task 4.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/protocol/src/api.rs` | New wire types: `DiffStatus`, `DiffNote`, `DiffFile`, `ChangesKind`, `ChangesRequest`, `ChangesReply`, `ChangeMark`, `ReviewRequest`, `ReviewReply`, `CommentsRequest`, `CommentsForm`, `CommentsReply`, `CommentSide`, `DraftComment`, `ChangesDrafts`, `ChangesSettings`; `TreeReply.changes`, `SessionTabs.changes`, `Settings.changes`. |
| `crates/core/src/gitbox/mod.rs` (new) | `GitBox`: the sandbox folder per checkout with its own copy of the index, its env and flags, `run`/`stream`, preflight refusals. |
| `crates/core/src/gitbox/config.rs` (new) | The allowlist read from the system, global, repository and worktree config files with `--no-includes`, and the `-c` pairs. |
| `crates/core/src/gitbox/refs.rs` (new) | `Oid`, `Head`, `Base`; HEAD, loose refs, `packed-refs`, and the base order, read from files. |
| `crates/core/src/changes/mod.rs` (new) | `Target`, `Check`, `check`, `read`: the cheap check, the full read, notes and caps. |
| `crates/core/src/changes/raw.rs` (new) | The parser for `diff --raw -z` output. |
| `crates/core/src/changes/cache.rs` (new) | `ChangesCache`: ≤ 8 checkouts, one work lock per checkout that no caller waits on past its deadline, fresh-check reuse, the tree's `marks` that never wait, merge-base, tracked-list and walk throttle. |
| `crates/core/src/server/changes.rs` (new) | `POST /v1/sessions/{tab}/changes`, `/review`, `/comments`. |
| `crates/core/src/server/fill.rs` | `async busy(st, tab, row, session) -> Option<Busy>` extracted, hooks read included; `fill` keeps its own replies. |
| `crates/core/src/server/tree.rs` | Marks in `TreeReply.changes`. |
| `crates/core/src/profiles/detect.rs` | `review_command`, `review_request`, `comments_delivery`. |
| `crates/core/src/layout.rs`, `settings.rs`, `paths.rs`, `tasks.rs`, `git.rs` | Drafts bounds and their lenient load, `changes` settings, `git_sandbox_dir`, `task_at`, `stream_command` with captured stderr. |
| `crates/core/src/server/mod.rs`, `server/ignores.rs`, `server/files.rs` | The `pub(crate)` re-exports the engine uses; the routes; an 8 MiB body limit on `/v1/layout/view`. |
| `crates/core/tests/changes_api.rs` (new), `tests/common/`, `src/bin/fake-agent.rs` | The routes' integration tests; `capture-input-plain`, a fake agent with bracketed paste off. |
| `src/features/pane/*`, `src/lib/keymap.ts`, `bindings.ts`, `appShortcuts.ts`, `navigation/*`, `window/MainWindow.tsx`, `window/PanelIcon.tsx` | A third panel, tabless, with Ctrl+Shift+H / ⌘⇧H. |
| `src/features/changes/model.ts` (new) | Reply → `Section[]` → `Item[]`; folding; labels. |
| `src/features/changes/comments.ts` (new) | `cleanComment`, `quoteLine`, `anchorOf`, `place`, `citation`, `messageOf`, `cyrb53`. |
| `src/features/changes/copy.ts` (new) | Every sentence the panel says. |
| `src/features/changes/changesApi.ts`, `useChanges.ts`, `changesSettings.tsx`, `ChangesSection.tsx` (new) | Requests, the poll engine, the settings hook and its Settings section. |
| `src/features/changes/ChangesPanel.tsx`, `DiffList.tsx`, `CommentBox.tsx`, `SendBar.tsx`, `changesMenu.ts`, `useDiffColours.ts`, `review.ts`, `ChangesPanel.css` (new) | The panel. |
| `src/features/files/useTree.ts`, `FileTree.tsx`, `FileTree.css`, `treeMarks.ts` (new) | Change marks on tree rows. |
| `src/features/sessions/LiveApp.tsx` | Wiring: render, poll visibility, send, review, palette. |
| `docs/spikes/S16-multiline-paste.md` (new), `docs/PLAN.md`, `docs/research/2026-11-23-changes-live-check.md` (new) | Records. |

---

### Task 1: Spike S16 — a multi-line paste into each CLI's prompt (main thread, live)

The main thread runs this one (live check, no code), before Task 7 is briefed. It decides each CLI's `comments_delivery` arm (Slice 6r-3 decision 13).

**Files:**
- Create: `docs/spikes/S16-multiline-paste.md`

- [ ] **Step 1: Start a scratch app with the marshell-live-check skill**

Run `start` with `-CopilotFake -QwenFake -AgyAuth -CodexAuth`. Claude Code stops at its login in a scratch `CLAUDE_CONFIG_DIR`, so ask the user before signing it in. If they decline, Claude stays unmeasured.

- [ ] **Step 2: For each CLI, paste three lines through the terminal's own paste**

1. Open a session for the CLI and wait for its prompt.
2. In the app's webview (CDP `Runtime.evaluate`, never desktop input), dispatch a synthetic `paste` `ClipboardEvent` whose `DataTransfer` holds `Spike line one\nSpike line two\n> spike line three` on that session's `.xterm-helper-textarea`. xterm.js turns `\n` into `\r` and wraps the text in `ESC[200~ … ESC[201~` when the CLI has DECSET 2004 on. Task 7 sends those same bytes.
3. **Pass:** the CLI's input holds the text, or its collapsed placeholder, and nothing is submitted:
   - Copilot and Qwen: their fake model's `*-fake.jsonl` gains no request.
   - Codex and agy: a webview screenshot shows no turn.
4. Repeat with 20 lines of 150 characters (3,000 characters), over each CLI's collapse threshold, then with 100 lines of 160 characters (16,000 characters), the paste form's cap. A CLI that keeps the first and not the second sets the cap: record the largest size it kept (try 8,000), and Task 7 uses the smallest such size across the CLIs as `PASTE_MAX`.
5. Clear the input with Esc, then Ctrl+U, never Enter, or end the session by its PID.

- [ ] **Step 3: Write `docs/spikes/S16-multiline-paste.md` in S15's format**

Record the verdict per CLI and the result per text: CLI version, kept/submitted, and placeholder text. A CLI is **Paste** when both texts stayed. It is **File** otherwise, and also when it was not measured. Codex's known typed-keys paste (`docs/research/attachment-ref.md`) is checked, not assumed.

- [ ] **Step 4: Commit**

```bash
git add docs/spikes/S16-multiline-paste.md
git commit -F .superpowers/msg.txt   # "docs(changes): spike S16, a multi-line paste into each CLI's prompt under ConPTY."
```

---

### Task 2: Wire types, saved drafts and the Changes settings

**Files:**
- Modify: `crates/protocol/src/api.rs` (types below, beside `TreeReply` ~1070 and `SessionTabs` ~520, `Settings` ~1411)
- Modify: `crates/core/src/layout.rs`:
  - a `lenient_changes` called from `lenient_session` (~150-171), beside `lenient_tree_state`;
  - repair ~395-416, validate ~530-575, and the `retain` at ~416.
- Modify: `crates/core/src/server/mod.rs:1315`: `/v1/layout/view` gets `.layer(DefaultBodyLimit::max(8 * 1024 * 1024))`.
- Modify: `crates/core/src/settings.rs` (a `lenient_changes` beside `lenient_files` ~83-119)
- Modify: the TS test fixtures that build a `Settings`, `TreeReply` or `SessionTabs` literal (`pnpm build` lists them)
- Regenerate: `src/generated/*` via `cargo test -p marshell-protocol`

**Interfaces:**
- Produces (Rust, all `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)] #[ts(export)]`, enums `#[serde(rename_all = "snake_case")]`):

```rust
pub enum DiffStatus { Modified, Added, Deleted, Renamed, Untracked }
pub enum DiffNote { Binary, Large }
pub struct DiffFile { pub path: String, pub old_path: Option<String>, pub status: DiffStatus, pub note: Option<DiffNote> }
pub enum ChangesKind { Same, Listed, NoRepo, Refused, Failed }
pub struct ChangesRequest { pub stamp: Option<String>, pub hide_whitespace: bool }
pub struct ChangesReply {
    pub kind: ChangesKind, pub stamp: String,
    /// The checkout (Listed, Same, Failed) or the plain folder (NoRepo, Refused), as the core spells it.
    pub folder: String,
    /// The checkout is not the launch folder's: the shell moved.
    pub moved: bool,
    pub branch: Option<String>,   // None when detached or unborn
    pub detached: Option<String>, // 7-character id when detached
    pub base: Option<String>,     // "origin/main", "main", a task's base; None: no base found
    pub unrelated: bool,          // the base shares no history with HEAD
    pub ahead: u32, pub uncommitted: bool,
    pub files: Vec<DiffFile>, pub diff: String, pub truncated: bool,
    pub more_new: u32,            // untracked files past MAX_NEW_FILES
    pub reason: Option<String>,   // Refused and Failed: the sentence
}
pub struct ChangeMark { pub path: String, pub status: DiffStatus } // path relative to TreeReply.root, '/'-separated
// TreeReply gains: #[serde(default)] pub changes: Vec<ChangeMark>,
pub struct ReviewRequest { pub base: Option<String> }
pub struct ReviewReply { pub typed: String }
pub struct CommentsRequest { pub text: String }
pub enum CommentsForm { Paste, File }
pub struct CommentsReply { pub form: CommentsForm, pub path: Option<String> }
pub enum CommentSide { New, Old }
pub struct DraftComment {
    pub id: String, pub path: String, pub side: CommentSide,
    pub start: u32, pub end: u32, pub rows: u32,
    pub first: String, pub hash: String,
    pub quote: Vec<String>, pub text: String,
}
pub struct ChangesDrafts { pub checkout: String, pub base: Option<String>, pub comments: Vec<DraftComment>, pub folds: BTreeMap<String, bool> }
// SessionTabs gains: #[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub changes: Option<ChangesDrafts>,
#[serde(default)] pub struct ChangesSettings { pub hide_whitespace: bool, pub wrap: bool } // Default: both false
// Settings gains: #[serde(default)] pub changes: ChangesSettings,
```

Bounds for `validate_layout`. Each error names the field the way the tree's errors do (`pane.sessions.{session}.changes…`).
- `checkout`: 1 to `MAX_PATH_CHARS` characters.
- `base`: ≤ 200 characters, no control character.
- `comments`: ≤ 50 of them.
  - `id`: `is_pane_id`, unique.
  - `path`: `tree_folder_ok`, i.e. relative and `/`-separated.
  - `1 ≤ start ≤ end`, and `1 ≤ rows ≤ 100_000`.
  - `first` and `hash`: 1 to 16 characters of `0-9a-f`.
  - `quote`: ≤ 3 entries of ≤ 201 characters each (a sign and 200 characters), no control character but `\t`.
  - `text`: 1 to 2,000 characters, no control character but `\n` and `\t`.
- `folds`: ≤ 1,000 keys, each `tree_folder_ok`.

**Loading.** `FileDoc::deserialize` runs `lenient_pane`, then a strict parse, and `load` falls back to the defaults on any error (`layout.rs:33-42`, `214-222`), so one bad draft must never reach the strict parse.
- `lenient_changes(value) -> bool` keeps only the comments that parse as `DraftComment` and the folds whose value is a boolean, and returns false when `checkout` isn't a string. `lenient_session` then removes `changes`.
- After the parse, the repair drops a comment or fold that breaks a bound, never the whole session.
- The session `retain` keeps a session whose `changes` is `Some`.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s `mod tests` and `settings.rs`'s tests:

```rust
#[test] fn drafts_round_trip_through_layout_json() // a ChangesDrafts with 2 comments and 1 fold saves and loads equal
#[test] fn drafts_keep_a_session_with_no_tabs()     // tabs empty, tree None, changes Some → the session survives repair
#[test] fn validate_refuses_a_51st_comment()         // Err contains "pane.sessions.s1.changes.comments holds at most 50"
#[test] fn validate_refuses_a_comment_path_with_dotdot() // "../x.ts" → Err mentions "isn't relative"
#[test] fn validate_refuses_text_with_an_escape()    // "a\u{1b}[201~b" → Err
#[test] fn repair_drops_only_the_bad_comment()       // 3 comments, one with start 0 → 2 remain
#[test] fn a_wrongly_typed_draft_costs_only_that_draft() // a file whose comment has "start":"x" and a fold "a":1 loads: the other sessions' tabs and the other comment survive
#[test] fn changes_settings_default_off_and_lenient() // {"changes":{"wrap":"yes"}} → wrap false, hide_whitespace false
```

In `crates/core/tests/layout_api.rs`: `a_view_of_three_megabytes_is_saved` (a PUT of 3 MiB of drafts answers 204, not 413).

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p marshell-core -j 4 -- layout:: settings::`
Expected: FAIL (the types don't exist yet).

- [ ] **Step 3: Add the types, bounds, repair and lenient parse**

- [ ] **Step 4: Regenerate and run**

Run: `cargo test -p marshell-protocol -j 4`, then `cargo test -p marshell-core -j 4 -- layout:: settings::`, then `pnpm check:generated`.
Expected: PASS. `pnpm build` may now fail where TS builds a `TreeReply` or `SessionTabs` literal; add `changes: []` to those test fixtures only.

- [ ] **Step 5: Commit** `feat(changes): the wire types, saved drafts and the Changes settings.`

---

### Task 3: Refs read from the repository's files

**Files:**
- Create: `crates/core/src/gitbox/refs.rs`, `crates/core/src/gitbox/mod.rs` (only `pub mod refs;` for now); `crates/core/src/lib.rs` gains `pub mod gitbox;`
- Modify: `crates/core/src/server/mod.rs` and `server/ignores.rs`. `mod ignores;` and `mod files;` are private, so make the items below `pub(crate)` in their files and re-export them from `server/mod.rs`:
  - `pub(crate) use ignores::{common_dir, git_dir, read_capped, walk_unignored, Ignores, MAX_POINTER_BYTES};`
  - `pub(crate) use files::{links_refused, refused};`

  Tasks 4 and 5 import them as `crate::server::…`.

**Interfaces:**
- Consumes: `crate::server::{git_dir, common_dir}` for the tests' fixtures. Use `crate::server::read_capped` with the 4 KiB `MAX_POINTER_BYTES` for single-ref files.
- Produces:

```rust
pub struct Oid(String); // lowercase hex, exactly 40 or 64 characters
impl Oid { pub fn parse(s: &str) -> Option<Oid>; pub fn as_str(&self) -> &str; pub fn short(&self) -> &str /* first 7 */ }
pub struct Head { pub branch: Option<String> /* "feat/login", no "refs/heads/" */, pub oid: Option<Oid> /* None: unborn */ }
pub struct Base { pub label: String, pub oid: Oid }
pub enum RefError { Reftable, Unreadable(&'static str) }
pub fn head(git_dir: &Path, common: &Path) -> Result<Head, RefError>;
pub fn resolve(git_dir: &Path, common: &Path, name: &str) -> Result<Option<Oid>, RefError>;
pub fn base(git_dir: &Path, common: &Path, task_base: Option<&str>) -> Result<Option<Base>, RefError>;
```

Rules:
- **Where a ref lives.** `HEAD` and `refs/bisect/`, `refs/worktree/` and `refs/rewritten/` are read in `git_dir`; every other ref is read in `common`. The loose file comes first, then `<common>/packed-refs`. A symbolic `ref: <name>` is followed, at most 5 hops.
- **`packed-refs` is read line by line with a capped reader.** Lines over 4,096 bytes, `#` lines and `^` (peeled) lines are skipped, and the read stops at the first match. A file over 256 MiB is `Unreadable`.
- **A name is valid only if:** it starts with `refs/` (or is `HEAD`); it is ≤ 1,024 bytes; it has no `..`, `\`, `:`, `?`, `*`, `[`, space or control character; and no component starts with `.` or ends with `.lock`. Anything else is `Ok(None)`.
- **A `<common>/reftable` folder, or `extensions.refstorage = reftable`, gives `RefError::Reftable`.** The config value is passed in by Task 4: `base` and `head` check the folder, and `GitBox::open` checks the config.
- **The base order (decision 3):**
  1. `task_base` = `b`: `refs/heads/b` (label `b`), else `refs/remotes/origin/b` (label `origin/b`).
  2. `refs/remotes/origin/HEAD` when it is `ref: refs/remotes/origin/<x>` and resolves (label `origin/<x>`).
  3. `refs/remotes/origin/main`, then `origin/master`.
  4. `refs/heads/main`, then `master`.
  5. `None`.

- [ ] **Step 1: Write the failing tests** (`refs.rs` `mod tests`, building git dirs by hand in `tempfile::tempdir()`):

```rust
#[test] fn head_on_a_branch_reads_the_loose_ref()
#[test] fn head_falls_back_to_packed_refs_and_skips_peeled_lines()
#[test] fn head_detached_has_no_branch()
#[test] fn head_unborn_has_no_oid()                 // HEAD → refs/heads/main, no such ref
#[test] fn worktree_head_lives_in_its_git_dir_and_branches_in_common()
#[test] fn base_prefers_origin_head_then_origin_main_then_local_main() // three fixtures, labels "origin/trunk", "origin/main", "main"
#[test] fn base_for_a_task_uses_its_branch()        // task_base "develop" → label "develop"
#[test] fn a_name_with_dotdot_or_lock_resolves_to_none() // "refs/heads/../../x", "refs/heads/a.lock"
#[test] fn symbolic_loops_stop_after_five_hops()     // a→b→a → Ok(None)
#[test] fn a_reftable_folder_is_refused()            // RefError::Reftable
#[test] fn sha256_ids_parse_and_others_do_not()      // 64 hex ok, 63 or uppercase → None
```

- [ ] **Step 2: Run**

`cargo test -p marshell-core -j 4 gitbox::refs`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run them again**

Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): read HEAD, branches and the main branch from the repository's files.`

---

### Task 4: The git sandbox

**Files:**
- Modify: `crates/core/src/git.rs`:
  - make `stream_cmd` `pub(crate) fn stream_command`;
  - pipe stderr into a reader thread that keeps its last 4 KiB;
  - a non-zero exit other than 1 errors with `"Git stopped with exit code {code}: {line}"`, where `line` is `said`'s rule applied to that stderr.

  Existing tests that match the old prefix keep passing.
- Modify: `crates/core/src/paths.rs`: `pub fn git_sandbox_dir(home: &Path) -> PathBuf` (`<home>/git-sandbox`).
- Create: `crates/core/src/gitbox/config.rs`; extend `gitbox/mod.rs`.

**Interfaces:**
- Consumes: Task 3's `refs`; `crate::server::{git_dir, common_dir, links_refused, refused, read_capped}` (Task 3's re-exports).
- Produces:

```rust
pub struct GitBox { /* checkout, git_dir, common, sandbox, empty, pairs: Vec<(String, String)>, sha256: bool, stamps */ }
pub enum BoxError { NoRepo, Refused(&'static str), Unsupported(&'static str), Git(String) }
impl GitBox {
    pub fn open(home: &Path, checkout: &Path) -> Result<GitBox, BoxError>;
    /// Reads the config again when the repository's or worktree's config file changed (length, modified time), or 60 s passed for the system and global files.
    pub fn refresh(&mut self) -> Result<(), BoxError>;
    pub fn stream(&self, args: &[&str], deadline: Instant, cancel: &AtomicBool, each: impl FnMut(&[u8]) -> bool) -> Result<Stopped, BoxError>;
    /// Collects at most `max` bytes; past that the run is stopped (`Stopped::Asked`) and the bytes so far kept.
    pub fn output(&self, args: &[&str], deadline: Instant, cancel: &AtomicBool, max: usize) -> Result<(Vec<u8>, Stopped), BoxError>;
    pub fn checkout(&self) -> &Path; pub fn git_dir(&self) -> &Path; pub fn common(&self) -> &Path;
    pub fn empty_tree(&self) -> &'static str; // sha1 4b825dc642cb6eb9a060e54bf8d69288fbee4904, sha256 6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321
    #[cfg(test)] pub fn runs(&self) -> usize; // git processes this box started, for Task 5's cache test
}
pub const ALLOWED: &[(&str, Allowed)]; // config.rs
```

**The sandbox.** It lives at `<git_sandbox_dir>/<16 hex of FNV-1a 64 of the checkout path>`, the path lowercased on Windows. The hash is written by hand, since `DefaultHasher` isn't stable across Rust releases and the folder must keep its name. It holds:
- `HEAD` = `ref: refs/heads/none\n`, and an empty `refs/`;
- `config`: `[core] repositoryformatversion = 0, bare = false`, or `1` with `[extensions] objectformat = sha256`;
- `index`: a copy of `<git_dir>/index`, with every `sharedindex.<id>` it names (split index) copied beside it. The copy is made again whenever the source's length or modified time changes, and it gets the source's modified time (`File::set_modified`), so git's racy-clean check judges it as it would the source. Git may refresh and rewrite this copy; that is the point of the copy.
- a copy of `<common>/shallow` and of `<common>/info/attributes`, each when it exists, capped at 1 MiB (over the cap, Unsupported "This repository's history file is too large for Changes."). A copy is deleted when its source is gone.
- next to the sandboxes, one empty file `empty-config`.

Every file is written through a temp file in the same folder and renamed into place.

**The run.** `git -C <checkout> -c core.fsmonitor=false -c protocol.allow=never -c core.quotepath=off [-c k=v for each pair] <args>` runs with:
- every inherited `GIT_*` variable removed, picked by a pure `git_vars(vars: impl Iterator<Item = (OsString, OsString)>) -> Vec<OsString>`;
- `GIT_DIR`, `GIT_WORK_TREE=<checkout>`;
- `GIT_INDEX_FILE=<sandbox>/index`, `GIT_OBJECT_DIRECTORY=<common>/objects`;
- `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=<empty-config>`, `GIT_ATTR_NOSYSTEM=1`;
- `GIT_NO_REPLACE_OBJECTS=1`, `GIT_NO_LAZY_FETCH=1`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`;
- in debug builds only, `GIT_TRACE2_EVENT` set from `MARSHELL_GIT_TRACE` when that is set, for the live check;
- `CREATE_NO_WINDOW` on Windows, as `git::command` sets it.

**The config read.** It runs with the cwd at the sandbox, `GIT_DIR` = the sandbox, the same `GIT_*` scrub, and no `NOSYSTEM`:
```
git config --system --no-includes -z --get-regexp <re>
git config --global --no-includes -z --get-regexp <re>
git config --file <common>/config --no-includes -z --get-regexp <re>
git config --file <git_dir>/config.worktree --no-includes -z --get-regexp <re>
```
The last runs only when the repository's `extensions.worktreeconfig` is true. Later files win. The regex also matches `core.repositoryformatversion`, `extensions.objectformat`, `extensions.refstorage` and `extensions.worktreeconfig`, which are read from the repository's file only, not passed on.

`ALLOWED` (key → accepted values, case-insensitive; anything else is dropped):

| Key | Values |
| --- | --- |
| `core.autocrlf` | true, false, input |
| `core.eol` | lf, crlf, native |
| `core.ignorecase`, `core.symlinks`, `core.filemode`, `core.precomposeunicode`, `core.trustctime`, `core.longpaths`, `core.fscache`, `diff.indentheuristic` | true, false, yes, no, on, off, 1, 0 |
| `core.checkstat` | default, minimal |
| `diff.renames` | the booleans, copies, copy |
| `diff.renamelimit` | an integer 0 to 1000 (git's default; a higher limit can keep `-M` past the 2 s deadline on every poll) |
| `diff.algorithm` | myers, minimal, patience, histogram |

**Preflight in `open`** (and again in `refresh` when a source file changed):
- `NoRepo`: no git dir.
- `Unsupported("Changes needs git 2.32 or later.")`: `git --version` (read once per process) is older. `GIT_CONFIG_GLOBAL` arrived in 2.32.
- `Refused("This repository reads its objects from a network folder, so Changes doesn't read it.")`:
  - `<common>/objects/info/alternates`, read as git reads it: each line taken relative to its own objects folder, and every listed folder's own `info/alternates` followed, 5 levels deep. A line that names a path `files::refused` refuses (network or device), a quoted line, or a line that names a link `links_refused` refuses is refused.
  - `links_refused` also refuses `objects`, `objects/info`, `objects/pack`, the index, each `sharedindex.*`, `shallow`, `packed-refs` and `info/attributes`, since a link to a UNC path there reaches SMB.
- `Unsupported("This repository stores its branches in a format Changes can't read yet.")`: reftable, from the `reftable` folder or from `extensions.refstorage = reftable`.
- `Unsupported("This repository uses a git format Changes can't read yet.")`: `repositoryformatversion` > 1, or an `objectformat` other than sha1/sha256.

- [ ] **Step 1: Write the failing tests** (`gitbox/mod.rs` `mod tests`, using real git like `tasks.rs`'s tests; a helper builds the S15 trap):

```rust
#[test] fn hostile_config_runs_nothing() // fsmonitor, filter.evil.clean on *.txt, diff.external, include of a missing \\server\share path, all appending to a marker; diff --raw, the patch, ls-files and rev-list via GitBox → marker empty, diff lists the changed .txt
#[test] fn only_the_sandbox_and_the_command_line_are_read() // stream(["config","--list","--show-origin"]) → every line starts "file:<sandbox>/config" or "command line:"
#[test] fn allowlisted_values_pass_and_others_do_not() // repo sets core.autocrlf=input, diff.algorithm=evil, core.pager=x → pairs == [("core.autocrlf","input")] plus system/global ones
#[test] fn crlf_file_is_unchanged_under_autocrlf_true()  // repo config core.autocrlf=true, a CRLF file committed → diff --raw empty
#[test] fn a_linked_worktree_uses_its_own_index()
#[test] fn sha256_repo_gets_objectformat_in_the_sandbox()
#[test] fn index_is_never_written()                     // touch two tracked files without changing their bytes; check and read three times → <git_dir>/index has the same bytes and modified time, and no index.lock ever appears
#[test] fn a_split_index_is_copied_with_its_shared_file()
#[test] fn alternates_on_a_network_path_are_refused()   // objects/info/alternates = \\\\server\\share\\objects → Refused
#[test] fn nested_alternates_are_followed_and_refused()  // a local alternate whose own alternates names \\\\server\\share → Refused
#[test] fn reftable_in_the_config_is_unsupported()       // extensions.refstorage = reftable, no reftable folder
#[test] fn shallow_is_copied_and_rev_list_stops_there()
#[test] fn a_stale_shallow_copy_is_removed()             // delete <common>/shallow, refresh → the sandbox has none
#[test] fn renamelimit_over_1000_is_dropped()
#[test] fn inherited_git_env_is_removed()               // the pure `git_vars(vars: impl Iterator<Item = (OsString, OsString)>) -> Vec<OsString>` picks GIT_EXTERNAL_DIFF and GIT_DIR, not PATH (no process env is set in tests)
#[test] fn exit_128_reports_gits_sentence()             // diff a missing object → BoxError::Git containing "bad object" or "unknown revision"
```

- [ ] **Step 2: Run**

`cargo test -p marshell-core -j 4 gitbox::`
Expected: FAIL.

- [ ] **Step 3: Implement `git.rs` changes, `config.rs`, then `GitBox`**

- [ ] **Step 4: Run them again with the `git::` tests too**

Run: `cargo test -p marshell-core -j 4 -- gitbox:: git::`
Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): a git sandbox that reads no repository config and runs no config program.`

---

### Task 5: The Changes engine — check, read and cache

**Files:**
- Create: `crates/core/src/changes/mod.rs`, `changes/raw.rs`, `changes/cache.rs`; `lib.rs` gains `pub mod changes;`
- Modify: `crates/core/src/tasks.rs`:
  - make `untracked_diff`, `new_file_header`, `cut_diff`, `MAX_DIFF`, `MAX_UNTRACKED` and `BINARY_SNIFF` `pub(crate)`;
  - add `pub fn task_at(home: &Path, checkout: &Path) -> Option<Task>`, the task whose `path` is the checkout (`projects::is_inside` both ways, case-insensitive on Windows).
- Uses Task 3's re-exports `crate::server::{Ignores, walk_unignored}`.

**Interfaces:**
- Consumes: Tasks 2-4.
- Produces:

```rust
// raw.rs
pub struct RawEntry { pub status: DiffStatus, pub path: String, pub old_path: Option<String> }
pub fn parse_raw(bytes: &[u8]) -> Vec<RawEntry>;
// mod.rs
pub const CHECK_BUDGET: Duration = Duration::from_secs(2);
pub const READ_BUDGET: Duration = Duration::from_secs(5);
pub const MAX_NEW_FILES: usize = 200;
pub struct Target { pub checkout: PathBuf, pub moved: bool, pub task_base: Option<String> }
pub struct Check { pub stamp: u64, pub head: Head, pub base: Option<Base>, pub from: String, pub unrelated: bool, pub entries: Vec<RawEntry>, pub new_files: Vec<String>, pub more_new: u32 }
pub enum ChangesError { NoRepo, Refused(String), Failed(String) }
pub fn reply_stamp(check: &Check, hide_whitespace: bool) -> String; // format!("{:016x}{}", check.stamp, if hide_whitespace { "w" } else { "" })
// cache.rs
pub const CHECK_FRESH: Duration = Duration::from_secs(1);
pub struct ChangesCache { /* Mutex<Vec<(PathBuf, Arc<Slot>)>>, ≤ 8, least recently used dropped */ }
// struct Slot { work: Mutex<Entry>, last: Mutex<Option<(Instant, Arc<Check>)>>, running: AtomicBool }
impl ChangesCache {
    pub fn new() -> Self;
    /// A check younger than CHECK_FRESH for this checkout is reused.
    pub fn check(&self, home: &Path, target: &Target, deadline: Instant, cancel: &AtomicBool) -> Result<Arc<Check>, ChangesError>;
    pub fn read(&self, home: &Path, target: &Target, hide_whitespace: bool, deadline: Instant, cancel: &AtomicBool) -> Result<ChangesReply, ChangesError>;
    /// For the tree: the last finished check of any age, at once. When it is older than CHECK_FRESH, or there is none, and no check runs, it starts one in `spawn_blocking` with CHECK_BUDGET, whose result the next poll gets.
    pub fn marks(self: &Arc<Self>, home: PathBuf, target: Target) -> Option<Arc<Check>>;
}
```

**Locks.** The outer `Mutex<Vec<…>>` is held only to find or insert a slot, and `last` only to read or swap it. No lock is held across a git run except `work`, which serializes the git runs for one checkout.
- `check` and `read` take `work` with `try_lock` in a loop (10 ms sleeps) until their deadline.
- Once they hold it, they look at `last` again, since another caller may just have finished.
- When the deadline passes while they wait, they return `last` if there is one, else `Failed("Git took too long to read the changes.")`.

The tree never waits: it uses `marks`.

**`parse_raw`** reads records of the form `:<mode> <mode> <oid> <oid> <letter>[score]\0<path>\0`, plus a second `<path>\0` for `R` and `C`.
- Letters map: `A`→Added, `D`→Deleted, `M`, `T` and `U`→Modified, `R`→Renamed (old_path = first path), `C`→Added (old_path set).
- `X`, an unknown letter, or a cut record is skipped. A path that isn't UTF-8 is skipped, as the tree skips one.

**`check`** runs:
1. `refs::head`, then `refs::base`.
2. The merge base: `merge-base <head> <base>`, cached per `(head, base)`. An empty answer means `unrelated`.
3. `from`: the merge base; else the head when there is no base or the histories are unrelated; else `empty_tree()` when the repository is unborn.
4. `diff --raw -z -M --no-ext-diff --no-textconv --ignore-submodules=all <from>`, capped at 8 MiB.
5. `symlink_metadata` (length, modified time) for each entry that isn't Deleted.
6. The tracked set from `ls-files --cached -z`, cached by the index file's (length, modified time).
7. The untracked files: `walk_unignored(&mut Ignores::new(checkout, checkout), checkout, deadline, cancel, &|rel| tracked.contains(rel), each)`, minus links, sorted, with the first `MAX_NEW_FILES` kept and the rest counted in `more_new`.
   - When the previous walk took over 250 ms, the walk runs at most every 10 s and the last list is reused in between.
   - A walk stopped by `Stopped::Deadline` or `Cancelled` keeps the last full list (none: an empty one). So a cut walk never moves the stamp, which would make every poll a full read.

`stamp` hashes the head, the base, `from`, the raw bytes, the stats, and the new files with their stats (`DefaultHasher`).

**`read`** builds the reply from a fresh `check`:
- **The patch:** `diff --no-color --no-ext-diff --no-textconv --ignore-submodules=all -M --src-prefix=a/ --dst-prefix=b/ [-w] <from>`, collected up to `MAX_DIFF + 1` bytes. `cut_diff(MAX_DIFF)` runs only when the text is longer than `MAX_DIFF`: it panics on a shorter one (`tasks.rs:277-278`).
- **New files:** each gets `untracked_diff(checkout, path)` appended while the total stays under `MAX_DIFF`, and `truncated` is set past it.
- **Notes:** a patch section with `Binary files … differ` gets `DiffNote::Binary`, and an untracked file over the cap or with a NUL gets `Large` or `Binary`, as `untracked_diff` decides today.
- **`ahead`:** `rev-list --count <from>..<head>` when `from` is the merge base; 0 otherwise.
- **`uncommitted`:** `diff --raw -z --no-ext-diff --no-textconv --ignore-submodules=all <head>` gives any byte (stop at the first), or there are new files. An unborn repository is uncommitted when there is any entry.
- **The stamp:** `reply_stamp(&check, hide_whitespace)`.
- **Errors:** a `BoxError::Git` becomes `Failed(sentence)`, and a deadline becomes `Failed("Git took too long to read the changes.")`.

- [ ] **Step 1: Write the failing tests** (`raw.rs` and `mod.rs` `mod tests`, real git):

```rust
#[test] fn raw_parses_modify_add_delete_rename_and_copy()
#[test] fn raw_skips_a_cut_record_and_non_utf8_paths()
#[test] fn check_on_a_branch_lists_commits_and_uncommitted_against_origin_main() // clone with origin/main, feat 2 commits + an edit + a new file
#[test] fn check_on_main_shows_unpushed_and_uncommitted()          // local main 1 ahead of origin/main → ahead 1
#[test] fn check_with_no_base_compares_with_head()                 // no main/master, no remote → base None, from = head
#[test] fn unborn_repo_shows_every_staged_and_new_file_as_new()
#[test] fn unrelated_histories_compare_with_head_and_say_so()
#[test] fn stamp_moves_when_a_file_is_touched_and_not_otherwise()  // two checks equal; write the same length, newer mtime → differs
#[test] fn new_files_are_capped_at_200_and_counted()               // 300 untracked → 200 + more_new 100
#[test] fn read_cuts_at_4_mib_on_a_file_boundary()
#[test] fn read_notes_binary_and_large_new_files()
#[test] fn hide_whitespace_drops_whitespace_only_hunks()            // reply.diff has the header and no @@ for that file
#[test] fn a_task_checkout_compares_with_its_base_branch()
#[test] fn cache_reuses_a_check_younger_than_a_second()             // two calls inside 1 s: GitBox::runs() unchanged by the second
#[test] fn a_detached_head_compares_with_the_base()                 // head.branch None, from = merge base, ahead counted
#[test] fn a_late_check_says_git_took_too_long()                    // a deadline already past, no last check → Failed("Git took too long to read the changes.")
#[test] fn a_caller_waiting_on_the_lock_gets_the_last_check()       // hold `work` from a thread; check with a 50 ms deadline → Ok(last)
#[test] fn marks_answer_at_once_and_refresh_in_the_background()     // first marks() None, then Some after the background run
#[test] fn a_cut_walk_keeps_the_last_list_and_the_stamp()
```

- [ ] **Step 2: Run**

`cargo test -p marshell-core -j 4 changes::`
Expected: FAIL.

- [ ] **Step 3: Implement `raw.rs`, then `check`, `read` and the cache**

- [ ] **Step 4: Run them again**

Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the check, the full read and the per-checkout cache.`

---

### Task 6: The Changes route, and the tree's marks

**Files:**
- Create: `crates/core/src/server/changes.rs` (the `changes` handler; Task 7 adds two more)
- Modify: `crates/core/src/server/mod.rs`:
  - `.route("/v1/sessions/{tab}/changes", post(changes::changes))` beside the tree's route (~1354);
  - `AppState` gains `changes: Arc<ChangesCache>`, `pub fn changes(&self) -> &ChangesCache` and `pub fn changes_arc(&self) -> Arc<ChangesCache>`.
- Modify: `crates/core/src/server/tree.rs`: `TreeReply.changes`.

- Create: `crates/core/tests/changes_api.rs`. The router tests live here, because `CARGO_BIN_EXE_fake-agent` exists only for integration tests. Reuse `capturing_in` and `typed` from `tests/attachments.rs` by moving them into `tests/common/`.

**Interfaces:**
- Consumes: `files::{row_of, checkout_of, checkout_above, refused, links_refused}` (make `checkout_of` and `checkout_above` `pub(super)`), `session.current_cwd()`, `tasks::task_at`, Task 5.
- Produces:
  - `POST /v1/sessions/{tab}/changes` (`ChangesRequest` → `ChangesReply`);
  - `TreeReply.changes`.

**Which checkout (decision 4).** `pub(super) fn changes_target(row: &SessionInfo, reported: Option<PathBuf>) -> Result<Target, ChangesKind>` doesn't use `project_root`: that function keeps a launch checkout ahead of the shell's folder (`files.rs:919-922`), and its `first_folder` drops a refused folder and falls back to the launch one (`files.rs:906-909`). The candidate folder is the reported folder when there is one, else the launch folder (`row.cwd`).
- **Refused:** a candidate that `refused` or `links_refused` refuses gives `Refused` with "{folder} is on a network share, so Changes doesn't read it.".
- **Otherwise:**
  - `checkout_above(candidate)` gives the checkout (6t decision 11's home rule included);
  - none gives `NoRepo` for that folder;
  - `moved` is the checkout differing from `checkout_of(row)`.
- No session, or no folder at all, gives 404, as the tree answers.
- In `spawn_blocking`, with a cancel flag the handler's drop sets (as the search does), it first runs `cache.check`.
- When `req.stamp` equals `reply_stamp(&check, req.hide_whitespace)`, it answers `kind: Same` with only `stamp` and `folder` filled. Otherwise it runs `cache.read`, which reuses that check.

**The tree.** Every tree reply carries marks, whether its folders came back `Listed` or `Same`: an edit doesn't change its folder's modified time, so a `Same` reply must still carry new marks.
- When the tree's root is `Checkout` or `InCheckout`, the handler calls `st.changes_arc().marks(home, target)` for that checkout, which never waits (Task 5).
- The marks are the check's entries that aren't Deleted, plus its new files, whose path is under the tree's root. The root's prefix is stripped, and at most 2,000 are kept.
- No check yet gives `changes: []`, and the next poll has them.

- [ ] **Step 1: Write the failing tests.** `changes_target` gets unit tests in `server/changes.rs`; the router tests go in `tests/changes_api.rs`, using a temp home and a fake shell that reports folders with OSC 9;9, as the tree's tests do:

```rust
#[test] fn target_prefers_the_reported_folder_over_a_launch_checkout() // launch in A, reported B → B, moved true
#[test] fn target_refuses_a_reported_network_folder()                   // reported \\wsl.localhost\Ubuntu\home → Refused, the exact sentence, never A
#[tokio::test] async fn changes_lists_a_checkout_and_then_answers_same()
#[tokio::test] async fn changes_in_a_plain_folder_is_no_repo()
#[tokio::test] async fn changes_follows_the_shell_from_one_checkout_into_another() // launch in repo A, the shell reports repo B → folder B, moved true
#[tokio::test] async fn changes_for_an_unknown_tab_is_404()
#[tokio::test] async fn changes_needs_the_token()                       // no bearer → 401, as local-api.md requires
#[tokio::test] async fn tree_marks_are_relative_to_the_root_and_skip_deleted()
#[tokio::test] async fn tree_marks_reach_a_same_reply_after_an_edit()  // poll, edit a file, poll until the marks change while the folder is Same
#[tokio::test] async fn tree_marks_are_empty_outside_git()
```

- [ ] **Step 2: Run**

Run: `cargo test -p marshell-core -j 4 -- server::changes server::tree`, then `cargo test -p marshell-core -j 4 --test changes_api`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run them again with the existing `layout_api` suite**

Run: the two commands of Step 2, and `cargo test -p marshell-core -j 4 --test layout_api`
Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the Changes route and the tree's change marks.`

---

### Task 7: The review command and the comments

**Files:**
- Modify: `crates/core/src/profiles/detect.rs` (beside `attachment_ref` ~509)
- Modify: `crates/core/src/server/fill.rs`: extract the refusals (~66-119) into `pub(super) enum Busy { Ended, Elevated, Paused, Waiting }` and `pub(super) async fn busy(st: &AppState, tab: &str, row: &SessionInfo, session: &Session) -> Option<Busy>`.
  - It includes the `spawn_blocking` read of the session's `hooks` (`fill.rs:66-79`), which `hooked && !started` needs (`fill.rs:108-115`).
  - `fill` maps the values back to its own replies, unchanged: `"ended"`, `API_WRITE_COPY`, `"It is paused."` and `"waiting"`.
- Modify: `crates/core/src/bin/fake-agent.rs`: a mode `capture-input-plain <f>`, like `capture-input` but never writing `ESC[?2004h`, for the bracketed-paste-off test.
- Modify: `crates/core/src/server/changes.rs`, `server/mod.rs` (two routes), `crates/core/tests/changes_api.rs`

**Interfaces:**
- Produces:

```rust
pub fn review_command(agent: Option<&str>) -> Option<&'static str>; // claude "/code-review", codex "/review", copilot "/review", qwen "/review", else None
pub fn review_request(base: Option<&str>) -> String;
pub enum CommentsDelivery { Paste, File }
pub fn comments_delivery(agent: Option<&str>) -> CommentsDelivery; // one arm per CLI from S16; `_ => File`
```

`review_request` copy:
- with a base: `Review the changes in this checkout against {base}, file by file, and list each problem with its file and line.`
- with none: `Review the uncommitted changes in this checkout, file by file, and list each problem with its file and line.`

**`POST /v1/sessions/{tab}/review` (`ReviewRequest` → `ReviewReply`):**
- 404 for an unknown tab.
- 409 `ResumeError { reason }` for a `busy` session, with `reason` one of `"ended"`, `"elevated"`, `"paused"` or `"waiting"`. No agent (`attachments::effective_agent` is None): 409 `"no_agent"`.
- `base`: ≤ 200 characters with no control character, else 400.
- The typed text is `review_command(agent)`, else `review_request(base)`. It is written as `sessions::input` writes (`release_tab`, `note_input`, `write_input(text.as_bytes())`), with no Enter, and the reply carries that text.

**`POST /v1/sessions/{tab}/comments` (`CommentsRequest` → `CommentsReply`):** the same 404 and 409 answers.
- **Refused text (400):** empty text, text over 1 MiB, or any control character but `\n` and `\t`. That refuses `\r` and `ESC`, so the text can never end the bracketed paste.
- **Paste form** when all hold:
  - `comments_delivery(agent) == Paste`;
  - `session.bracketed_paste()` is true;
  - the text is ≤ `PASTE_MAX` characters: 16,000, or the smaller size S16 recorded.

  The bytes are `paste_pieces(&[&text.replace('\n', "\r")], true)`, as xterm.js pastes.
- **Otherwise, the file form:**
  1. Write `<attachments_dir>/<tab>/comments-<unix ms>.md` (the text plus a final `\n`), through a temp file renamed into place.
  2. Paste one line, `Read my review comments in {attachment_ref(agent, &path)}`, with `paste_pieces(&[line], session.bracketed_paste())`.
  3. Reply `form: File`, `path`.

- [ ] **Step 1: Write the failing tests**

```rust
// detect.rs
#[test] fn review_commands_per_cli()          // claude "/code-review", codex/copilot/qwen "/review", antigravity None, Some("crush") None, None None
#[test] fn review_request_names_the_base()
#[test] fn comments_delivery_per_cli()        // the S16 table; unknown → File
// tests/changes_api.rs (fake-agent `capture-input`, whose written bytes are captured, as tests/attachments.rs does)
#[tokio::test] async fn review_types_the_command_with_no_enter()      // bytes == b"/code-review", no \r or \n
#[tokio::test] async fn review_for_a_shell_with_no_agent_is_refused() // 409 "no_agent"
#[tokio::test] async fn comments_paste_bracketed_with_cr_line_breaks() // bracketed on, Paste agent → ESC[200~a\rb ESC[201~
#[tokio::test] async fn comments_go_by_file_when_bracketed_paste_is_off() // `capture-input-plain` → a file exists, bytes are one line, no \r or \n
#[tokio::test] async fn comments_over_paste_max_go_by_file()
#[tokio::test] async fn comments_refuse_an_escape_or_a_cr()            // "a\u{1b}[201~b", "a\rb" → 400
#[tokio::test] async fn comments_wait_while_the_agent_asks()           // a permission hook posted through the hook route → 409 "waiting", nothing written
#[tokio::test] async fn review_and_comments_need_the_token()           // no bearer → 401 for both
```

The existing `--test fill` suite must pass unchanged, since the extracted `busy()` keeps fill's four replies.

- [ ] **Step 2: Run**

Run: `cargo test -p marshell-core -j 4 -- detect::`, then `cargo test -p marshell-core -j 4 --test changes_api` and `cargo test -p marshell-core -j 4 --test fill`
Expected: the new tests FAIL; `fill` passes.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run them again, then the whole crate**

Run: the commands of Step 2, then `cargo test -p marshell-core -j 4` and `cargo clippy --workspace --all-targets -- -D warnings`.
Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the review command and the comments, pasted without Enter or written to a file.`

---

### Task 8a: A third panel and its key

**Files (the inventory from planning):**
- Modify:
  - `src/features/pane/panels.ts`: `PANELS`, `PANEL_LABEL`, `PANEL_COMMAND`, `stackOf` (filled valid for `shown.length >= 2`), `activeIn` and `panelTabs` (Changes: none).
  - `paneState.ts`: limits; `fill` stays a no-op under 2 shown.
  - `PanelStack.tsx`: a `useLayoutMotion` per panel. Each divider `i` (between `drawn[i-1]` and `drawn[i]`) is live on its own and moves only its pair's weights. `onDividerGone` fires when a focused divider stops being live.
  - `SidePane.tsx`: `plus` becomes optional, and a new optional `title?: string` renders `<span className="side-pane__title" role="heading" aria-level={2}>` in place of the tablist when given.
  - `src/features/window/MainWindow.tsx`, `PanelIcon.tsx` (the glyph below).
  - `src/lib/keymap.ts`, `bindings.ts`, `appShortcuts.ts`.
  - `src/lib/navigation/names.ts`, `machine.ts`, `useNavigation.ts`.
  - `src/features/appearance/settingsSync.ts`.
  - `src/features/sessions/LiveApp.tsx` (togglePanel, nav ids ~1682, `renderPanel` renders a placeholder `EmptyPanel` for Changes until Task 11a).
- Modify (Rust): `crates/core/src/layout.rs`:
  - `PANEL_IDS: [&str; 3] = ["browser", "files", "changes"]`;
  - `repair_panels` and `validate_panels` accept `filled` when ≥ 2 panels are shown;
  - their tests and `crates/core/tests/layout_api.rs:297`.
- Modify (tests listing panels): `panels.test.ts`, `PanelStack.test.tsx`, `SidePane.test.tsx`, `paneState.test.ts`, `names.test.ts`, `machine.test.ts`, `useNavigation.test.ts`, `keymap.test.ts`, `appShortcuts.test.ts`, `MainWindow.test.tsx`, `LiveApp.test.tsx`.
- Modify (docs): `docs/PLAN.md`:
  - line 140: "Changes joins them in a later slice" becomes "Browser, Files and Changes (the session's git diff, slice 6r-3)". The Switches sentence gains "Changes: Ctrl+Shift+H, ⌘⇧H on macOS".
  - line 737: the row becomes "Browser panel · Files panel · Changes panel", keys "**A** · ⌘⌥A, **D** · ⌘D and **H** · ⌘⇧H", with "Changes took ⌘⇧H (Slice 6r-3 decision 14): ⌘H is Hide and ⌘⌥H Hide Others." in place of the "is kept" sentence.

**Interfaces:**
- Produces:
  - `PanelId` includes `"changes"`;
  - `PANEL_COMMAND.changes = "changes-panel"`;
  - `Binding.cmdShift?: true`;
  - nav top id `"panel-changes"` named "Changes".

**Keys:**
- keymap: `{ id: "changes-panel", label: "Changes panel", group: "Window", binding: { code: "KeyH", mac: { code: "KeyH", cmdShift: true } } }`.
- `chordOf`: on mac, `cmdShift` gives `{ meta: true, shift: true, ctrl: false, alt: false }`.
- `shortcutLabel`: `⌘⇧H`.
- Windows and Linux: Ctrl+Shift+H.

**Glyph** (16×16, stroke as the others):
```html
<rect x="2" y="2" width="12" height="12" rx="2" {...fill} />
<path d="M8 4.5v4M6 6.5h4M6 11h4" />
```
It uses `data-icon="panel-changes"`.

- [ ] **Step 1: Write the failing tests** (add to the existing files):

```ts
// panels.test.ts
it("stacks Browser, Files, Changes in that order", () => expect(stackPanels(stackOf({ shown: ["changes", "browser", "files"] } as never))).toEqual(["browser", "files", "changes"]));
it("keeps Fill with three panels shown", () => expect(stackOf({ shown: ["browser", "files", "changes"], filled: "changes" } as never).filled).toBe("changes"));
// keymap.test.ts
it("Changes is Ctrl+Shift+H and ⌘⇧H", () => { /* shortcutLabel(...,"windows") === "Ctrl+Shift+H"; (...,"mac") === "⌘⇧H" */ });
it("⌘⇧H conflicts with nothing and no reserved chord", () => { /* bindingConflicts against every COMMANDS binding on mac is false */ });
// PanelStack.test.tsx
it("draws two live dividers for three panels", () => { /* html has 2 role="separator" with aria-label naming each pair */ });
// MainWindow.test.tsx
it("shows a Changes button with its tooltip", () => { /* title "Show Changes (Ctrl+Shift+H)", aria-expanded false, data-nav-top="panel-changes" */ });
// SidePane.test.tsx
it("a tabless panel shows its title and no plus", () => { /* panel "changes", tabs [], title "4 files · +120 −18" → no role="tablist", no data-pane-plus */ });
// names.test.ts / machine.test.ts
it("the nav ring names the Changes button", () => { /* "panel-changes" → "Changes" */ });
```

```rust
#[test] fn changes_is_a_panel_and_fill_holds_with_three() // layout.rs
```

- [ ] **Step 2: Run**

Run: `pnpm test src/features/pane src/lib src/features/window` and `cargo test -p marshell-core -j 4 layout::`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the whole frontend suite, the build and the layout tests**

Run: `pnpm test`, `pnpm build` and `cargo test -p marshell-core -j 4 --lib -- layout::` and `cargo test -p marshell-core -j 4 --test layout_api`
Expected: PASS.

- [ ] **Step 5: Commit** `feat(pane): a third panel, Changes, with Ctrl+Shift+H and ⌘⇧H.`

---

### Task 8b: The tabless panel's commands, focus and drafts

**Files:**
- Modify:
  - `src/features/pane/paneCommands.ts`: `commandPanel` (rule below).
  - `src/features/sessions/LiveApp.tsx`: a `lastTabPanel` ref beside `lastPanel` (~1173), set only when the keyboard enters Browser or Files, passed to `commandPanel` as `last`.
  - `paneState.ts`: the `drafts` action, and `close` keeping drafts (below).
  - `paneRules.ts`, `focusPane.ts` (Changes' target: `[data-changes-item][tabindex="0"]`, else its empty-state button).
  - `src/features/pane/EmptyPanel.tsx`: `actions: readonly EmptyAction[]` (0 to 2), with the existing callers moved to a one-item list.
  - `src/styles/noScrollInMainWindow.test.ts`: `.changes__list` allowed. Its comment says "the Changes panel's diff list (`.changes__list`): a diff is read by scrolling it, both ways, with the themed scrollbar visible, like the preview body".

**Interfaces:**
- Produces: `PaneAction` gains `{ type: "drafts"; session: string; drafts: ChangesDrafts | null }`.
  - It sets `sessions[session].changes`, capped at 50 comments and 1,000 folds.
  - `null`, or drafts with no comments and no folds, deletes `changes`. A session left with no tabs, no tree and no `changes` is deleted.

**`close`.** A session whose last tab closes keeps its tree and its `changes`, with no tabs (today it keeps only the tree, `paneState.ts:428-431`). Without this, closing the last browser tab would throw away the comments.

**Tab commands (decision 22).** Changes holds no tabs, so `commandPanel(on, focused, last, holdsTabs)` first drops `"changes"` from `on`, and then runs its three steps on what is left: the focused panel, the last panel when it holds tabs, then the first panel that holds tabs, else the first.
- It answers null when nothing is left.
- So with the keyboard in Changes, Ctrl+Tab, Ctrl+Shift+Tab and the palette's tab commands act on the tab panel that last had the keyboard, else on the first shown one.
- Focus stays in Changes, as it stays in the terminal.

- [ ] **Step 1: Write the failing tests** (add to the existing files):

```ts
// paneCommands.test.ts
it("tab commands from Changes act on the last tab panel", () => expect(commandPanel(["browser", "files", "changes"], "changes", "files", hasTabs)).toBe("files"));
it("tab commands from Changes fall back to the first panel with tabs", () => expect(commandPanel(["browser", "files", "changes"], "changes", null, (p) => p === "files")).toBe("files"));
it("tab commands do nothing with only Changes shown", () => expect(commandPanel(["changes"], "changes", null, () => false)).toBeNull());
// paneState.test.ts
it("closing the last tab keeps the drafts", () => { /* a session with one browser tab and changes.comments [c] → close → tabs [], changes kept */ });
it("drafts with nothing in them delete the session's changes, and an empty session goes", ...);
it("drafts cap at 50 comments and 1,000 folds", ...);
// EmptyPanel.test.tsx
it("shows two actions in order", ...);
```

- [ ] **Step 2: Run**

Run: `pnpm test src/features/pane`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the whole frontend suite and the build**

Run: `pnpm test` and `pnpm build`
Expected: PASS.

- [ ] **Step 5: Commit** `feat(pane): the Changes panel holds no tabs, so tab commands reach the others and closing a tab keeps its comments.`

---

### Task 9: The Changes model and the comments' rules (pure)

**Files:**
- Create: `src/features/changes/model.ts`, `comments.ts`, `copy.ts`, `review.ts`, and a `.test.ts` for each

**Interfaces:**
- Consumes: `splitDiff` (`src/features/tasks/splitDiff.ts`), `parseUnifiedDiff` (`src/components/DiffView/diff.ts`), and the generated `ChangesReply`, `DraftComment`, `DiffStatus` and `DiffNote`.
- Produces:

```ts
// model.ts
export const FOLD_LINES = 400, OPEN_BUDGET = 10_000;
export type Row = { kind: "hunk" | "context" | "add" | "del"; text: string; oldNo?: number; newNo?: number };
export type Section = { path: string; oldPath?: string; status: DiffStatus; added: number; removed: number;
  note?: DiffNote | "whitespace" | "gone"; rows: Row[]; folded: boolean; foldReason?: "big" | "lockfile" | "minified" | "budget" };
export type Item =
  | { kind: "file"; key: string; file: Section }
  | { kind: "show"; key: string; file: Section }
  | { kind: "row"; key: string; file: Section; at: number }
  | { kind: "comment"; key: string; comment: DraftComment; placed: Placement }
  | { kind: "note"; key: string; file: Section; text: string };
export function sectionsOf(reply: ChangesReply, folds: Record<string, boolean>, drafts: DraftComment[], hideWhitespace: boolean): Section[];
export function itemsOf(sections: Section[], drafts: DraftComment[], placements: Map<string, Placement>): Item[];
export function itemLabel(item: Item): string;
export function isLockfile(path: string): boolean;
export function isMinified(path: string, rows: Row[]): boolean;
// comments.ts
export const MAX_COMMENTS = 50, MAX_COMMENT_CHARS = 2000, QUOTE_LINES = 3, QUOTE_CHARS = 200;
/** What a comment may hold: control characters other than \n and \t removed, \r\n and \r turned into \n, cut at MAX_COMMENT_CHARS code points. CommentBox runs it on every change. */
export function cleanComment(text: string): string;
/** A quoted line: control characters other than \t (C0, DEL and C1) become U+FFFD, cut at QUOTE_CHARS code points, never inside a surrogate pair. */
export function quoteLine(text: string): string;
export type Placement = { at: number | null; changed: boolean };
export function cyrb53(s: string): string; // 14 lowercase hex characters
export function anchorOf(file: Section, from: number, to: number): Omit<DraftComment, "id" | "text">;
export function place(c: DraftComment, file: Section | undefined): Placement;
export function citation(c: DraftComment, changed: boolean): string;
export function messageOf(drafts: DraftComment[], placements: Map<string, Placement>, sections: Section[], base: string | null): string;
// review.ts
export function reviewCommandOf(agent: string | null): string | null; // the same table as detect.rs
export function reviewLabel(agent: string, agentName: string): string;
```

**Rules:**
- **Sections.**
  - **Order:** sections come in `reply.files` order. A draft's path missing from the files adds a `note: "gone"` section with no rows, at the end in path order.
  - **Rows:** a section's rows start at its first `@@` line. Everything before it is dropped whatever it holds: `new file mode`, `deleted file mode`, `similarity index`, `rename from`/`rename to`, `old mode`/`new mode` and `Binary files … differ`, which `parseUnifiedDiff` would show as context, since it treats only `diff`, `---`, `+++` and `index` lines as meta. Hunk headers stay as `hunk` rows.
  - **`\ No newline at end of file`:** these lines are dropped and count for neither side's numbers, so a comment below one cites the right line.
  - **Whitespace:** a section with no rows and `hideWhitespace` gets `note: "whitespace"`.
- **Folding.**
  - A file starts folded when it has more than `FOLD_LINES` rows, or `isLockfile`, or `isMinified`. It also starts folded when the open rows above it already reach `OPEN_BUDGET`.
  - `folds[path]` overrides: `true` folded, `false` open.
  - The lockfile names are: `pnpm-lock.yaml`, `package-lock.json`, `npm-shrinkwrap.json`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `uv.lock`, `Pipfile.lock`, `go.sum`, `flake.lock`, `packages.lock.json`, `pubspec.lock`, `mix.lock` and `Podfile.lock`.
  - A minified file has a name matching `/\.min\.(js|mjs|cjs|css)$/i`, or any row over 1,000 characters.
- **Items.** Each file header comes first.
  - A folded file adds a `show` item.
  - An open file adds its rows, each placed comment right after the row at `placed.at`, and the comments whose `at` is null after the header.
  - A note becomes a `note` item.
- **Ranges:** stay inside one hunk and never include a `hunk` row. `anchorOf` gives:
  - `side`: `new` if any row of the range has a `newNo`, else `old`;
  - `start`/`end`: the min/max of that side's numbers, and `rows` the row count;
  - `first`: `cyrb53` of the first row's `` `${kind}\u0000${text}` ``;
  - `hash`: `cyrb53` of the rows' keys joined with `\n`;
  - `quote`: the first `QUOTE_LINES` rows through `quoteLine`. When the range holds more than one kind, each line is prefixed with its sign (`+`, `-` or a space); otherwise it is plain. A quote line therefore never holds `\r`, `\n` or ESC, which the core refuses.
- **`place` (re-anchoring).**
  1. Candidates are the windows of `c.rows` rows with no `hunk` row whose first row's key-hash equals `c.first`. The one whose window hash equals `c.hash` and whose first number on `c.side` is nearest `c.start` wins: `{ at: last row index, changed: false }`.
  2. Failing that, the row on `c.side` numbered `c.end`, or the nearest smaller one, gives `{ changed: true }`.
  3. With no rows at all: `{ at: null, changed: true }`.
- **`citation`:**
  - `path:N` or `path:N-M`;
  - `old` side: `path:7 (removed line)`, or `path:7-9 (removed lines)`;
  - a changed comment adds ` (changed since)`.
- **`messageOf`:**
  - The header is `Comments on the changes (compared with {base}):`, or `Comments on the uncommitted changes:` when `base` is null.
  - Then one blank line before each comment: its citation, each quote line as `> ` + line, a last `> …` line when `rows` > `QUOTE_LINES`, then the comment's text as typed.
  - Comments come in section order, then by `start`. There is no trailing newline.
- **`itemLabel`:**
  - `Added, line 42: …`, `Removed, line 7: …` and `Line 12: …`. An empty text reads `blank line`.
  - A hunk: `Changed block at line 10`.
  - A file: `{path}, {modified|new|deleted|renamed from {oldPath}}, {added} added, {removed} removed`.
  - A show item: `Show 1,234 lines`.
  - A comment: `Comment on line 42: {text}` or `Comment on lines 10 to 14: {text}`, plus `. The line changed` or `. The lines changed`.
  - A note: its text.
- **`reviewLabel`:**
  - Claude Code: `Ask Claude Code to review (types /code-review)`.
  - Codex and Copilot: `Ask {name} to review (types /review)`.
  - Qwen Code: `Ask Qwen Code to review (types /review, uncommitted work only)`.
  - Others: `Ask {name} to review (no review command, so it types a request)`.

`copy.ts` holds every string below as functions or constants. Tasks 11a-13 use only these.

| Id | Text |
| --- | --- |
| summary | `{n} file(s) · +{a} −{r}`; with none `No changes` |
| line | `{branch or "Detached at {id}"} · {n} commit(s) ahead of {base}` (left out at 0) `· uncommitted work` (when true) `· in {folder}` (when moved) |
| noBase | `No main branch found. Showing uncommitted work.` |
| unrelated | `No common history with {base}. Showing uncommitted work.` |
| emptyRepo | title `No git repository`, line `{folder} isn't in a git checkout.` |
| emptyNone | title `No changes`, line `Nothing differs from {base}.` / `Nothing is uncommitted.` |
| emptyFailed | title `Can't read the changes`, line the reason, action `Try again` |
| cut | `The diff is cut at 4 MiB. {n} more file(s) aren't shown.` |
| moreNew | `And {n} more new file(s) aren't shown.` |
| notes | binary `Binary file, not shown.`; large `New file over 256 KiB, not shown.`; whitespace `Only whitespace changed.`; gone `No changes in this file now.` |
| changed | `The line changed` / `The lines changed` |

- [ ] **Step 1: Write the failing tests** (selected; each rule above gets one):

```ts
it("folds a lockfile, a minified file and a 401-row file", ...)
it("folds what comes after 10,000 open rows", ...)
it("a fold override beats the rule", ...)
it("cites a removed-only range by its old numbers", () => expect(citation(anchor, false)).toBe("src/old.ts:7-9 (removed lines)"));
it("quotes a mixed range with signs", () => expect(anchorOf(file, 3, 5).quote).toEqual(["-old()", "+neu()", " keep()"]));
it("ends a quote of more than three rows with an ellipsis line", () => expect(messageOf(...)).toContain("> c\n> …\n"));
it("re-anchors a comment when lines above it move", () => { /* 5 rows inserted above → at shifts by 5, changed false */ });
it("marks a comment changed when its line's text changed", ...)
it("keeps a comment on a reverted file under a gone section", ...)
it("writes the message in the exact format", () => expect(messageOf(...)).toBe("Comments on the changes (compared with origin/main):\n\nsrc/a.ts:42\n> const port = 3000;\nUse PORT.\n\nsrc/old.ts:7 (removed line)\n> legacyInit();\nStill needed?"));
it("labels rows for a screen reader", () => expect(itemLabel(addRow)).toBe("Added, line 42: const x = 1;"));
it("starts a new file's rows at its first hunk", () => { /* "new file mode 100644" and "similarity index 90%" give no row */ });
it("skips a no-newline marker in the numbering", () => { /* "-a", "\\ No newline at end of file", "+a", " b" → b is new line 2 */ });
it("turns control characters in quoted code into U+FFFD", () => expect(quoteLine("a\fb\u001b[0mc\rd")).toBe("a�b�[0mc�d"));
it("cuts a quote line by code points, never inside an emoji", () => expect([...quoteLine("x".repeat(199) + "😀😀")].length).toBe(200));
it("cleans a comment of control characters but keeps tabs and line breaks", () => expect(cleanComment("a\tb\r\nc\u001b[201~d")).toBe("a\tb\nc[201~d"));
it("review commands match detect.rs", () => { /* read crates/core/src/profiles/detect.rs, find review_command's arms, compare with reviewCommandOf */ });
```

- [ ] **Step 2: Run**

`pnpm test src/features/changes`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run them again**

Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the diff model, folding, labels and the comments' message.`

---

### Task 10: Requests, the poll engine and the settings

**Files:**
- Create: `src/features/changes/changesApi.ts`, `useChanges.ts` (+ test), `changesSettings.tsx`, `ChangesSection.tsx`
- Modify:
  - `src/features/appearance/settingsSync.ts`: `changes` getter and `setChanges`, as `files`;
  - `src/features/settings/LiveSettings.tsx`: register the section after Files;
  - the settings nav list, wherever `FilesSection`'s entry is named.

**Interfaces:**
- Produces:

```ts
export function readChanges(tab: string, req: ChangesRequest): Promise<ChangesReply | "no-folder">; // 404 → "no-folder", as filesApi's orNoFolder
export function askReview(tab: string, base: string | null): Promise<ReviewReply>;
export function postComments(tab: string, text: string): Promise<CommentsReply>;
export const CHANGES_POLL_MS = 2000;
export function changesSchedule(s: { inFlight: boolean; shown: boolean; focused: boolean; urgent: boolean; lastAsk: number | null; now: number }): "none" | "ask";
export function useChanges(o: { session: string; shown: boolean; hideWhitespace: boolean }): { reply: ChangesReply | null; noFolder: boolean; busy: boolean; refresh(): void };
export function useChangesSettings(): { settings: ChangesSettings | null; failed: boolean; set(next: Partial<ChangesSettings>): void };
export const CHANGES_DEFAULTS: ChangesSettings; // both false
```

**`changesSchedule`** returns `none` when a request is in flight or the panel isn't shown. Otherwise it returns `ask` when the request is urgent, when there has been no ask yet, or when the window is focused and at least `CHANGES_POLL_MS` have passed.

**`useChanges`:**
- It ticks every 250 ms, as `useTree`, and sends the last reply's `stamp`. `Same` keeps the reply.
- `refresh()`, a change of `hideWhitespace` and a change of `session` are urgent, with `stamp: null`.
- A reply that arrives for a session no longer shown is dropped.

**`ChangesSection`:** two switches, "Hide whitespace changes" and "Wrap long lines", in `FilesSection`'s form.

- [ ] **Step 1: Write the failing tests** (`useChanges.test.ts`):

```ts
it("asks nothing while hidden or in flight", ...)
it("asks at once when urgent and every 2 s while focused", ...)
it("waits while the window is unfocused", ...)
```

The section gets a `renderToStaticMarkup` test that names both switches.

- [ ] **Step 2: Run**

`pnpm test src/features/changes src/features/settings`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run them again**

Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the requests, the 2-second poll and the Changes settings.`

---

### Task 11a: The diff list and its keys

**Files:**
- Create: `src/features/changes/ChangesPanel.tsx`, `DiffList.tsx`, `ChangesPanel.css`, `DiffList.test.tsx`
- Modify:
  - `src/features/sessions/LiveApp.tsx`: render `ChangesPanel` for `panel === "changes"`; `SidePane` gets `title={summary}`, no `plus`, `menu={{ label: "Changes menu" }}`; the ⋯ menu state beside Files'; `shown` = Changes on screen and the pane open beside sessions.
  - `src/features/pane/EmptyPanel.tsx`: a large `ChangesIcon`.

**Interfaces:**
- Consumes: Tasks 8a-10.
- Produces:

```ts
export type ChangesPanelProps = {
  session: string; agent: string | null; agentName: string; shown: boolean;
  drafts: ChangesDrafts | undefined; onDrafts(next: ChangesDrafts | null): void;
  onOpen(o: { path: string; line?: number; keep: boolean }): void; onEscape(): void;
  onSend(text: string, count: number): void; onReview(base: string | null): void;
  announce(text: string): void;
};
export function listKey(items: Item[], focus: number, key: string, mods: { shift: boolean; alt: boolean }, range: { from: number; to: number } | null): { focus: number; fold?: string; unfold?: string; range?: { from: number; to: number } | null; open?: "comment" | "menu"; escape?: true };
```

**The list:**
- **Structure.** `DiffList` is `role="tree"`, `aria-label="Changes"`, `aria-multiselectable="true"`. Every item is a `role="treeitem"` with `data-changes-item` and a roving `tabIndex`, `aria-level` 1 for files and 2 for the rest, `aria-expanded` on files, and `aria-label={itemLabel(item)}`.
- **The range.** Rows in the selected range get `aria-selected="true"` and are drawn with `SelectedMark` (`src/components/a11y/`), the only selected-state marker (ui.md), plus a tint from the tokens.
- **Keys** (decision 17):
  - Up and Down move through items.
  - Alt+Up and Alt+Down go to the previous and next file header.
  - Left on a file folds it, and on any other item goes to its file. Right on a file unfolds it.
  - Home and End go to the first and last item.
  - Enter on a row opens the comment box (Task 12); on a file it toggles the fold; on `show` it unfolds.
  - Shift+Up and Shift+Down extend the range inside its hunk, and Shift+click does the same.
  - Shift+F10 and the Menu key open the item's menu (Task 11b; until then they do nothing).
  - Esc calls `onEscape`.
- **Header.** A file header shows the path, the status letter (M, A, D, R), and `+a −r`.
- **Strip and states.**
  - The strip line under the title is `copy.line(reply)`.
  - The empty states use `EmptyPanel` with Task 9's copy; only "Can't read the changes" has an action.
  - `Refused` shows the "Can't read the changes" title with the reply's reason and no action.
  - `noFolder` (a 404) shows the same title and line the Files panel shows for a session with no folder (reuse its constant).
- **Drawing.**
  - Every file section is a block with `content-visibility: auto`, and its rows are split into blocks of 200 rows with `contain-intrinsic-block-size: auto calc(200 * var(--changes-row))`.
  - With wrap off, rows use `white-space: pre`; with wrap on, `pre-wrap` and `overflow-wrap: anywhere`.
  - The added and removed fills use DiffView's `color-mix` tokens. In forced colours the signs carry the meaning: `CanvasText` and `Canvas`, with the sign column always drawn.

- [ ] **Step 1: Write the failing tests** (`DiffList.test.tsx` via `renderToStaticMarkup`, and `listKey`, pure):

```ts
it("is one tree with one tab stop", () => { /* exactly one tabindex="0" */ });
it("labels an added row", () => { /* aria-label "Added, line 42: …" */ });
it("Alt+Down jumps to the next file", ...)
it("Shift+Down stops at the hunk's end", ...)
it("Shift+Down never takes in a hunk row", ...)
it("Left on a row goes to its file, Right on a folded file unfolds it", ...)
it("Home and End reach the first and last item", ...)
it("Esc leaves the list", () => expect(listKey(items, 3, "Escape", none, null).escape).toBe(true));
it("a folded file shows its Show row", ...)
it("draws the range with SelectedMark", () => { /* the selected rows' markup holds SelectedMark's element and aria-selected="true" */ });
it("each empty state shows its copy", () => { /* NoRepo, Listed with no files, Failed with Try again, Refused with no action, noFolder */ });
```

- [ ] **Step 2: Run**

`pnpm test src/features/changes`
Expected: FAIL.

- [ ] **Step 3: Implement** (read `good-css`'s skill before the CSS)

- [ ] **Step 4: Run the tests and the build**

Run: `pnpm test` and `pnpm build`.
Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the diff list, one tree for the keyboard, with folding.`

---

### Task 11b: The diff list's colours and menus

**Files:**
- Create: `src/features/changes/changesMenu.ts` (+ test), `useDiffColours.ts` (+ test)
- Modify: `DiffList.tsx`, `ChangesPanel.tsx`, `src/features/sessions/LiveApp.tsx` (the ⋯ menu state beside Files')

**Interfaces:**
- Consumes: Task 11a; `tokenizer` and `languageOf` (`src/features/preview/highlight.ts`, `kinds.ts`); `COLOUR_CAP` (`src/features/editor/tokens.ts`); `ContextMenu`.
- Produces:

```ts
export function changesMenuItems(o: { settings: ChangesSettings | null; failed: boolean; review: string | null; hasFiles: boolean }): MenuItem[];
export function lineMenuItems(item: Item): MenuItem[];
export function headerMenuItems(file: Section): MenuItem[];
/** The token classes for each open section's rows, keyed by row index; a section it skips has none. */
export function colourSections(sections: Section[], cache: Map<string, (string | null)[][]>): Map<string, (string | null)[][]>;
```

**Menus:**
- **Header menu:** "Open in Files", "Fold" or "Unfold", and "Copy path".
- **Line menu:**
  - "Comment" (disabled on a hunk);
  - "Open in Files at this line" (removed rows open at the nearest new line);
  - "Copy line" or "Copy lines".
- **The ⋯ menu:**
  - Refresh;
  - the review label (left out when `review` is null);
  - a separator, then "Fold all files" and "Unfold all files" (disabled when `hasFiles` is false);
  - a separator, then "Hide whitespace changes" and "Wrap long lines" as checked rows, disabled while `settings` is null; when `failed`, they read as the defaults.
- Shift+F10 and the Menu key open the line or header menu at the item.

**Colours.** `useDiffColours` runs `colourSections`, which tokenizes each open section's new side (context and add rows, in order) and old side (context and del rows) with its own tokenizer state.
- It skips a section whose rows' text is over `COLOUR_CAP` or that has no language.
- It caches by path and the section's hash.

- [ ] **Step 1: Write the failing tests**

```ts
it("the ⋯ menu leaves review out for a plain shell", ...)
it("the ⋯ menu's switches are checked rows", ...)
it("Comment is disabled on a hunk", () => expect(lineMenuItems(hunkItem).find((i) => i.label === "Comment")?.disabled).toBe(true));
it("colours the new and old sides apart", () => { /* a .ts section with a del row "const a" and an add row "let a": each row gets its own side's keyword class */ });
it("skips colours past COLOUR_CAP and for a file with no language", ...)
it("reuses a cached section", ...)
```

- [ ] **Step 2: Run**

`pnpm test src/features/changes`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the tests and the build**

Run: `pnpm test` and `pnpm build`.
Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): the diff list's colours and its menus.`

---

### Task 12: Comments and Send

**Files:**
- Create: `src/features/changes/CommentBox.tsx`, `SendBar.tsx`, `CommentBox.test.tsx`, `SendBar.test.tsx`
- Modify: `DiffList.tsx` (comment items and the box), `ChangesPanel.tsx`, `src/features/sessions/LiveApp.tsx`
- Modify: `src/lib/keymap.ts` (palette-only commands with no binding: `changes-refresh` "Refresh changes", `changes-review` "Ask the agent to review", `changes-send` "Send comments") and `appShortcuts.ts`

**Interfaces:**
- Consumes: Task 9's `anchorOf`, `place` and `messageOf`; Task 10's `postComments` and `askReview`; `deliveryFor`, `addFailure`, `focusDelivered` and `announce` in LiveApp.

**The box:**
- It opens under the range's last row, with a textarea labelled `Comment on line 42` or `Comment on lines 10 to 14` and the hint `Enter adds · Shift+Enter starts a new line · Esc closes`. Every change of its text runs `cleanComment` (Task 9), which also caps it at 2,000 characters, so a pasted escape or form feed never reaches a draft.
- **Enter** adds the comment, announces `Comment added. {n} to send.`, and focus returns to the row. **Shift+Enter** starts a new line.
- **Esc** closes. An empty new box adds nothing. A box with text keeps it as the comment, announcing `Comment kept as a draft.`.
- Its React key is the draft's anchor key (path, side, start, end), so a new reply never remounts it. It is placed by `place()` on each reply.
- When the drafts' `checkout` isn't the one shown and comments exist, the box doesn't open. It announces `Send or discard the {n} comments on {folder name} first.`
- The 51st comment announces `A session holds at most 50 comments.`

**A comment item:** Enter edits it in the box. Delete removes it and announces `Comment deleted.`. A changed comment shows "The line changed".

**The Send bar** shows while drafts exist:
- `Send {n} comment(s)`, or `Copy {n} comment(s)` when the session has no prompt line (`deliveryFor(row).via === "clipboard"`);
- `Discard all`, which turns the bar into `Discard {n} comments?` with `Discard` and `Keep`.

**Send** in LiveApp:
1. A `refused` from `deliveryFor` is announced and keeps the comments.
2. The clipboard form calls `navigator.clipboard.writeText(message)`, clears the comments, and announces `Copied {n} comment(s). {name} has no prompt line.`.
3. The paste form calls `postComments`:
   - `form: "paste"` announces `Pasted {n} comment(s) into {name}.`;
   - `form: "file"` announces `Pasted a link to {n} comment(s) into {name}.`.

   Both clear the comments and run `focusDelivered`.

   "Clear" means `onDrafts({ ...drafts, comments: [] })`: the folds stay, and Task 8b's `drafts` action deletes `changes` only once nothing is left.
4. A 409 is announced, with the comments kept:
   - `waiting`: `{name} is waiting for an answer, so the comments are kept.`;
   - `ended`: `{name} has ended, so the comments are kept.`;
   - `paused`: `{name} is paused, so the comments are kept.`;
   - `elevated`: `{name} runs elevated, so Marshell can't paste into it. The comments are kept.`.

   Any other failure is announced as `addFailure(e)`.

**Review:** `askReview(session, base)`, then focus moves to the terminal.
- A command announces `Typed {command} into {name}. Press Enter there to start the review.`.
- A request announces `Typed a review request into {name}. Press Enter there to send it.`.
- A refusal is announced as for Send, ending in `so nothing was typed.`.

- [ ] **Step 1: Write the failing tests**

```ts
it("Enter adds, Shift+Enter breaks the line, Esc keeps typed text", ...)   // CommentBox's key handler, pure
it("the box keeps its key across a reply that moves lines", ...)          // DiffList items before/after a 5-row shift: same key, same at+5
it("Send reads Copy for a session with no prompt line", ...)              // SendBar markup
it("Discard all asks first", ...)
it("a refusal keeps the drafts", ...)                                       // the send reducer: 409 waiting → drafts unchanged, notice text
it("a send keeps the folds", ...)                                           // the send reducer: paste → comments [], folds unchanged
it("the message reaches postComments unchanged", ...)
it("the box drops a pasted escape and form feed", ...)                      // CommentBox's change handler runs cleanComment
it("the palette lists the three Changes commands with no binding", ...)    // keymap.test.ts: changes-refresh, changes-review, changes-send; appShortcuts runs each
```

- [ ] **Step 2: Run**

`pnpm test src/features/changes src/features/sessions/LiveApp.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the tests and the build**

Run: `pnpm test` and `pnpm build`.
Expected: PASS.

- [ ] **Step 5: Commit** `feat(changes): line comments, the Send bar and the review action.`

---

### Task 13: The tree's change marks

**Files:**
- Create: `src/features/files/treeMarks.ts` (+ test)
- Modify: `useTree.ts` (keep the latest `reply.changes`; rows gain `change?: DiffStatus` and `holdsChanges?: boolean`), `FileTree.tsx` (~256 label, ~258 row), `FileTree.css`, `FileTree.test.tsx`

**Interfaces:**
- Produces:

```ts
export function marksOf(changes: ChangeMark[]): { files: Map<string, DiffStatus>; folders: Set<string> };
export const MARK_LETTER: Record<DiffStatus, string>; // modified M, added A, untracked A, renamed R, deleted D
export const MARK_WORD: Record<DiffStatus, string>;   // ", modified", ", new", ", new", ", renamed", ", deleted"
```

**Rules:**
- A file row ends with `<span className="file-tree__mark" aria-hidden="true">M</span>`, in the text colour, and its label gains `MARK_WORD`.
- A folder in `folders` gets `data-changes=""` and a small dot drawn by CSS, and its label gains `, has changes`.
- In forced colours the mark is `CanvasText`.
- Marks don't change the row's height or its ignored dimming.

- [ ] **Step 1: Write the failing tests**

```ts
it("marks every ancestor folder of a change", () => expect([...marksOf([{ path: "a/b/c.ts", status: "modified" }]).folders]).toEqual(["a", "a/b"]));
it("reads a modified file aloud", () => { /* aria-label "c.ts, modified" */ });
it("a folder holding changes says so", () => { /* aria-label "b, has changes" */ });
it("a Same reply's marks replace the last ones", () => { /* useTree's reply reducer: Listed with marks [a], then Same with marks [a, b] → marks [a, b] */ });
```

- [ ] **Step 2: Run**

`pnpm test src/features/files`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run them again**

Expected: PASS.

- [ ] **Step 5: Commit** `feat(files): the tree marks changed files and the folders that hold them.`

---

### Task 14: The record and the live check (main thread)

**Files:**
- Modify: `docs/PLAN.md`, Slice 6r-3 decision 22, which gains every `Ruling:` from the ledger that changes what a person sees or what the core reads or runs
- Create: `docs/research/2026-11-23-changes-live-check.md`

- [ ] **Step 1: Run the live check (marshell-live-check skill), with these rows**

| # | Check | Pass when |
| --- | --- | --- |
| 1 | A branch of a scratch repo: 2 commits ahead of `origin/main`, an edit, a new file, a rename, a CRLF file | Changes shows 4 files, `2 commits ahead of origin/main · uncommitted work`, R for the rename, nothing for the CRLF file; the tree shows M, A, R and folder dots |
| 2 | A shell that `cd`s into the S15 trap repo (fsmonitor, clean filter, `diff.external`, an include of a missing `\\server\share` path), with Changes and the tree open for 60 s, `MARSHELL_GIT_TRACE` set to a scratch file (a dev build passes it on as `GIT_TRACE2_EVENT`), and a script rewriting a tracked file with the same bytes every second | The marker file is empty; the trace shows every git run with `GIT_DIR` at the sandbox and no `config.include` read; the repo's `.git/index` keeps its modified time; Changes says `in {folder}` |
| 3 | Keyboard alone | Ctrl+Shift+H shows Changes; Up/Down, Alt+Down, Left/Right, Shift+Down, Enter, Shift+F10 and Esc do what decision 17 says; Modifier+Tab visits terminal, Browser, Files, Changes |
| 4 | Comments to each measured CLI | 3 comments: Send pastes them, or the file line per S16, into the prompt; no Enter; the announcer says "Pasted 3 comments into …"; a plain shell gets "Copy 3 comments" |
| 5 | A refresh during a comment | The agent (or a script) inserts 5 lines above an open box: the box keeps text and focus, and the draft moves 5 lines down |
| 6 | Waiting | While a Claude or Qwen permission prompt shows, Send says "… is waiting for an answer, so the comments are kept." |
| 7 | Restart | Drafts and folds come back after `restart`, and a comment whose line the agent changed reads "The line changed" |
| 8 | Forced colours and reduced motion (emulated) | One screenshot each: signs visible, fills gone, the marks in `CanvasText` |
| 9 | Big change | A lockfile, a minified bundle and 300 new files: folded, capped, with their notes; scrolling stays smooth |

- [ ] **Step 2: Write the research note** in the skill's format, with every finding and its fix commit (failing test first).

- [ ] **Step 3: Commit** `docs(changes): slice 6r-3's rulings and its live check.`

## Exit criteria

- Decisions 1-22 each have a task and a test or a live-check row, and every Review Focus line is pinned.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm build`, `pnpm check:generated` and `pnpm tauri build --no-bundle` pass.
- Live-check rows 1-9 pass, or each failure is fixed with its test, or recorded as a known limit in the PR.
