# Slice 6q-4a: the text editor in a side-pane tab — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A text file in the side pane opens in an editor that colours, finds, replaces and saves with Ctrl+S; unsaved edits are kept as drafts; a save can be undone; and a file that changes on disk reloads, or shows a bar when the tab has unsaved edits.

**Architecture:** CodeMirror 6 replaces `CodeView` for every text kind, coloured by a Marshell-owned bridge that feeds Shiki's tokens in as decorations, a few lines at a time. What a tab knows about its file (the text the edits started from, dirty, a conflict, the last save) is one pure reducer, `editorDoc.ts`; the React hook around it does the fetches. The core gains a save route that writes a user's file in place after a hash check and a snapshot, an undo for it, and a small drafts store in Marshell's home. 6q-4b adds the three-way merge on top.

**Tech Stack:** Rust (axum, tokio, sha2 through `config::hash`), React 19, TypeScript, CodeMirror 6 (`@codemirror/state`, `view`, `commands`, `search`, `language`, `autocomplete`), Shiki 4.5 (already in), Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 6q-4 decisions (2026-11-18)" 1–9. Carry-over: "Slice 6q decisions" first round 7 and 8, second round 7 and 8; "Slice 6q-2 decisions" 13 (the four kept tabs, the 32-tab limit) and 15; "Slice 6q-3 decisions" 13 (the app's shortcuts from a text box). Research: `docs/research/2026-11-07-tabs-pane-options.md` §2.4 and §2.5. Rules: `.claude/rules/local-api.md`, `protocol.md`, `ui.md`.

## Global Constraints

- **Security surface.** Nothing changes in `src-tauri/capabilities/`, `tauri.conf.json`, `tauri.windows.conf.json`, `crates/core/src/server/guard.rs` or token handling. The CSP is untouched. No new Tauri command or event. No new crate. The only new packages are the six CodeMirror ones (decision 7). Two changes to the local API are part of the slice and nothing wider: the CORS layer exposes one more response header (`x-content-hash`), and `.claude/rules/local-api.md`'s line "No route takes a file path to write." becomes "No route takes a file path to write, except the user-file save routes in `server/save.rs`, which check the path with `userfiles::guard` first. (phase-3e2 plan; PLAN.md §9, 6q second round 7)". If a task seems to need more, stop and report.
- **The routes** sit in the token-guarded router, each with a "refused without the bearer" test. New request and response types go in `crates/protocol/src/api.rs` with `#[derive(TS)] #[ts(export)]`, every `u64` with `#[ts(type = "number")]`. Blocking file work runs in `spawn_blocking`; no lock is held across an await.
- **A user's file is written only by `server/save.rs`,** only after: `files::refused` and the links check, no `..` part, `userfiles::guard`, the hash check, and a snapshot that is on disk. Nothing else in the slice writes outside Marshell's home.
- **Numbers.** A text file opens, saves and drafts up to 2 MiB (`TEXT_CAP`, unchanged). 20 snapshots per file and 256 MiB of snapshots in all. At most 500 drafts; a draft with no tab is dropped 30 days after its last edit. A draft is written 400 ms after the last key. Colours stop above 1 MiB of text and on a line over 2,000 characters. Colouring works in steps of at most 100 lines and yields between steps. The file is checked for a change once a second (`POLL_MS`, unchanged).
- **Names.** Encodings: `utf-8`, `utf-8-bom`, `utf-16le`, `utf-16be`. Line endings: `lf`, `crlf`. A stat's `read_only` reason: `own`, `managed`, `attribute`. The editor's code lives in `src/features/editor/`; the core's in `crates/core/src/userfiles/`.
- **Copy** (sentence case, no "!", no "successfully"; exact strings; `{name}` is the file's name):
  - Bar: "{name} changed on disk." with buttons "Keep mine" and "Take disk".
  - Notice for a deleted file: "{name} was deleted on disk. Ctrl+S saves it again."
  - After a save: "Saved {name}." with the button "Undo". After the undo: "{name} is back as it was before the save."
  - Refusals and failures: "{name} changed on disk, so it wasn't saved." · "{name} changed after the save, so the save can't be undone." · "{name} is over 2 MB, so it can't be saved." · "{name} couldn't be saved. {the core's line}" · "The unsaved edits to {name} couldn't be kept as a draft."
  - Read-only lines: `own` "Marshell doesn't save files in its own folder." · `managed` "This file is managed by your organization, so Marshell doesn't save it." · `attribute` "{name} is read-only on disk."
  - The core's lines: "That path can't be saved." · the `own` and `managed` lines above · "The file is read-only or in use by another program." · "Marshell keeps at most 500 drafts. Save some files first."
  - Dialog: title "Save the changes to {name}?", body "Its unsaved edits are lost if you don't save them.", buttons "Save", "Don't save", "Cancel".
  - Tab: the name is followed by ", unsaved" for a screen reader, and the tooltip by " · Unsaved edits".
  - Footer: "Ln {line}, Col {column}" · "UTF-8", "UTF-8 with BOM", "UTF-16 LE" or "UTF-16 BE" · "LF" or "CRLF" · "Esc, then Tab, leaves the editor".
  - The editor's `aria-label`: "{name}, editor", or "{name}, read-only".
  - Note: "Syntax colours are off for files over 1 MB."
  - Announcements (through `Announcer`): the saved, undone, refusal and failure lines above, and after a reload "{name} changed on disk. {n} lines changed." ("1 line changed." for one).
  - Commands: `pane-save` "Save file" · `pane-undo-save` "Undo last save" · `pane-go-to-line` "Go to line…" · `pane-word-wrap` "Word wrap". None has an app shortcut; each is left out of the palette when no text tab is on screen (6p decision 4).
  - CodeMirror's own words, through `EditorState.phrases`: "Find", "Replace", "Next", "Previous", "All", "Match case", "Regular expression", "Whole word", "Replace all", "Close", "Go to line".
- **Keys in the editor.** Ctrl+S saves. Ctrl+F opens find and replace. Ctrl+G opens go to line. Alt+Z switches word wrap. Tab indents; Esc then Tab leaves; Ctrl+M switches Tab between the two. Esc closes the find bar and never leaves the tab. Alt+Left and Alt+Right stay the pane's Back and Forward. Every app shortcut (`lib/appShortcuts.ts`) works from the editor, and an editor command on such a key is left out. No command adds a second cursor.
- **Styles.** Read the good-css skill before writing declarations; the frontend-design skill decides the look of the colour scheme and the editor. Tokens only, no hex outside `tokens.css`. `:hover` only inside `@media (hover: hover) and (pointer: fine)`; forced-colors rules use system colours only; no boxed outlines; no scrollbar shows.
- **Keyboard and forced colours.** Everything new works with the keyboard alone and in forced colours. `Announcer` is the only live region. An overlay over the pane calls `useCoverPage`.
- **Who writes what** (decision 4). Tasks 1 to 3 and 14 are written in the main thread. Tasks 4 to 13 go to Sonnet workers (`effort: high` where the heading says so), one at a time; the main thread reads each diff and runs its tests. A worker edits only its task's files and never runs `git checkout`, `git restore` or `git stash`.
- **Checks.** Rust: `cargo test -p <crate> -j 4`; `cargo fmt --all` before each Rust commit; `cargo clippy --workspace --all-targets -j 4 -- -D warnings` at the end of each Rust task. After a protocol change: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`. Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** the three-way merge and the bar's Merge button (6q-4b), several cursors, a list of earlier versions, Save as…, a new empty file, autosave, a minimap, code folding, autocompletion, saving a PDF, a save from inside WSL.

## Review Focus

1. **Paths that must never be written:** `\\server\share\a.txt`, `\\?\C:\a.txt`, `C:\x\..\y.txt`, `NUL`, `a.txt:stream`, a relative path, a link or junction whose target is in Marshell's home, `<home>\settings.json`, `<home>\agents\x.md`, `Managed-Settings.JSON` in a project, a file under `%ProgramData%`, a folder, a file whose folder is missing. Expected: 400 or 403 with its line, no byte written, no snapshot taken. Tests: Task 2 `save_refuses_*`, `guard_*`.
2. **The disk moves at the wrong moment:** the file changes between load and Ctrl+S; it is deleted and saved again; it changes after a save and before Undo; a second Ctrl+S while the first is in flight; the same file in two sessions' tabs; a save of text equal to the disk. Expected: a changed file is never overwritten without the bar; Undo never overwrites a later change; two saves run one after the other; both tabs show one draft. Tests: Task 2 `save_conflicts_*`, Task 3 `unsave_*`, Task 9 `step_*`.
3. **Bytes that aren't plain UTF-8 with LF:** a BOM, UTF-16 LE and BE, CRLF, both kinds of ending, a lone CR, no final newline, an empty file, invalid UTF-8, a NUL, UTF-16 with an odd byte count or a lone surrogate. Expected: decode then encode gives back the same bytes for every clean file; a file that can't round-trip is "binary" and never opens in the editor. Tests: Task 6 `roundTrips_*`, `binary_*`.
4. **A draft that outlives what it was made from:** the disk changed while Marshell was closed; the file is gone; it is now read-only, over 2 MiB or not text; the draft's text equals the disk; the draft file is cut short or holds another path. Expected: the bar, the deleted notice or the read-only line shows with the draft's text in the editor; an equal draft is dropped; a broken draft is ignored and the file opens from disk. Tests: Task 4 `get_ignores_*`, Task 9 `opened_with_a_draft_*`.
5. **Editing while colours are still coming:** a key pressed above the lines being coloured; the text replaced by a reload mid-way; a language that fails to load; a 1 MiB file opened at its end; a 100,000-character line. Expected: colours never come from an older text, a failed language leaves plain text, and no step colours more than 100 lines before yielding. Tests: Task 8 `tokens_*`.

---

### Task 1: The protocol types and the store of saved-over files (main thread)

**Files:**
- Modify: `crates/protocol/src/api.rs`, `crates/core/src/paths.rs`, `crates/core/src/lib.rs`
- Create: `crates/core/src/userfiles/mod.rs`, `crates/core/src/userfiles/history.rs`
- Regenerated: `src/generated/*.ts`

**Interfaces:**
- Produces, in `api.rs`:
  - `FileStat` gains `#[serde(default)] pub read_only: Option<String>` ("Why a save would be refused: `own`, `managed` or `attribute`. None when the file can be saved.").
  - `pub struct FileSaved { #[ts(type = "number")] pub size: u64, #[ts(type = "number")] pub modified_ms: u64, pub hash: String, #[serde(default)] pub snapshot: Option<String> }` — `snapshot` is None when the save created the file.
  - `pub struct FileConflict { #[serde(default)] pub hash: Option<String> }` — the 409 body: what the disk holds now, None for no file.
  - `pub struct FileUnsave { pub path: String, pub snapshot: String }`.
  - `pub struct Draft { pub path: String, pub text: String, pub base_text: String, #[serde(default)] pub base_hash: Option<String>, pub encoding: String, pub eol: String, #[ts(type = "number")] pub edited_ms: u64 }` — `base_hash` None means the edits started from no file.
  - `pub struct DraftInfo { pub path: String, #[ts(type = "number")] pub edited_ms: u64 }`, `pub struct DraftList { pub drafts: Vec<DraftInfo> }`.
- Produces, in `paths.rs`: `pub fn file_history_dir(home: &Path) -> PathBuf` (`<home>/file-history`), `pub fn drafts_dir(home: &Path) -> PathBuf` (`<home>/drafts`).
- Produces, in `userfiles/history.rs`:
  - `pub const KEEP_PER_FILE: usize = 20;` `pub const KEEP_BYTES: u64 = 256 * 1024 * 1024;`
  - `pub struct Saved { pub path: PathBuf, pub before: Vec<u8>, pub after_hash: String }`
  - `pub struct History`, `History::new(home: &Path)`, `fn take(&self, path: &Path, before: &[u8], after_hash: &str, now_ms: u64) -> anyhow::Result<String>`, `fn get(&self, id: &str) -> anyhow::Result<Option<Saved>>`.
  - An id is `f` and 12 lowercase hex characters (`auth::new_id("f")`); any other text is `Ok(None)` and never reaches the disk. The index is `file-history/index.json` through `store::{read, write}`; a blob is `file-history/<id>.bin`. Paths compare with `projects::same_path`. `take` writes the blob and syncs it before the index names it, then drops the oldest entries past 20 for that path and past `KEEP_BYTES` in all, never the one it just took.

- [ ] **Step 1: Write the failing tests** in `history.rs`: `take_and_read_back`, `keeps_the_last_20_per_file` (21 takes of one path → the first id is None, the other 20 read back), `another_files_snapshots_are_not_counted`, `drops_the_oldest_past_the_byte_limit` (a `History::with_limit(home, 10)` test constructor: three 4-byte takes → the first is gone), `the_newest_is_never_dropped` (one 20-byte take under a 10-byte limit reads back), `unknown_and_malformed_ids_are_none` (`""`, `"f123"`, `"s0123456789ab"`, `"f0123456789aZ"`, `"../x"`), `an_empty_file_is_a_snapshot` (`before` empty reads back empty, not None).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 userfiles` → compile error, then FAIL.
- [ ] **Step 3: Implement** the types, the two paths and the store, with one static lock around each read-modify-write of the index, as `config/snapshots.rs` does.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 userfiles`, `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `pnpm build` (fix TypeScript literals that build a `FileStat`) → all pass; clippy clean.
- [ ] **Step 5: Commit** `feat(core): the types of a file save and a draft, and a store of the files a save replaced.`

### Task 2: The save route (main thread)

**Files:**
- Create: `crates/core/src/userfiles/guard.rs`, `crates/core/src/userfiles/write.rs`, `crates/core/src/server/save.rs`
- Modify: `crates/core/src/server/files.rs` (`stat` gains the state and `read_only`; `content` sends the hash; `checked_links` becomes `pub(super)`), `crates/core/src/server/mod.rs` (route, exposed header), `.claude/rules/local-api.md` (the one line, as in Global Constraints)
- Test: `crates/core/tests/save_api.rs`

**Interfaces:**
- Consumes: Task 1's types and `History`; `files::{refused, checked_links, TEXT_CAP}`; `config::guard::{MANAGED_NAMES, denied_for_os}`; `config::hash::hash_bytes`; `projects::{canonical, is_inside}`.
- Produces:
  - `userfiles::guard`: `pub enum SaveRefusal { Own, Managed }`, `pub fn save_refusal(path: &Path, marshell_home: &Path) -> Option<SaveRefusal>`. It resolves links first (`canonical`). `Own`: inside Marshell's home, `agents` included. `Managed`: inside a `denied_for_os()` folder, or a file name in `MANAGED_NAMES` (any case on Windows).
  - `userfiles::write`: `pub fn write_in_place(path: &Path, bytes: &[u8], create: bool) -> io::Result<()>`. With `create` it opens with `create_new`; otherwise it opens the existing file for writing without truncating, writes from the start, sets the length to `bytes.len()` and syncs. It never creates a folder.
  - `GET /v1/files/content` also sends `x-content-hash`: the SHA-256 of the bytes it serves, in lowercase hex. The CORS layer exposes it beside `x-modified-ms`.
  - `GET /v1/files/stat` fills `read_only`: `own` or `managed` from the guard, else `attribute` when the file's permissions say read-only, else None.
  - `PUT /v1/files/content?path=<absolute>&expect=<hash|none>`, body the file's new bytes, body limit `TEXT_CAP`. In order, inside one `spawn_blocking` under one static lock: read the file (missing is fine only when `expect=none`; a folder is a 400), compare its hash with `expect`, take the snapshot when there was a file, write in place, stat. Answers: 200 `FileSaved`; 400 "That path can't be saved." (refused, relative, a `..` part, a refused link, a folder, a missing `expect`); 403 with the `own` or `managed` line; 403 "The file is read-only or in use by another program." for `PermissionDenied`; 409 `FileConflict` when the hashes differ, and when `expect=none` but a file is there, or `expect=<hash>` but none is; 413 past the limit; 500 with the OS's text for any other write error.
  - A save of bytes equal to the disk writes nothing and takes no snapshot: 200 with the current stat and `snapshot: None`.

- [ ] **Step 1: Write the failing tests.** In `guard.rs`: `guard_passes_a_project_file`, `guard_refuses_marshells_home_and_its_agents_folder`, `guard_refuses_a_managed_name_in_any_case` (Windows-only assertion for the case), `guard_refuses_a_denied_folder` (a `save_refusal_with(path, home, denied)` inner function takes the folders), `guard_refuses_a_link_into_marshells_home` (a symlink on Unix, a junction made with `cmd /c mklink /J` on Windows; skip the assertion when the link can't be made). In `write.rs`: `writes_over_a_longer_file` (no tail of the old bytes is left), `keeps_a_hard_link` (two names, one write, both read the new bytes), `create_refuses_an_existing_file`, `does_not_create_a_folder`. In `save_api.rs` (helpers as `layout_api.rs`; a `raw` request helper sends bytes):
  - `save_is_refused_without_the_bearer`.
  - `content_sends_the_hash_of_its_bytes` and `stat_names_why_a_file_is_read_only` (a file in the test's Marshell home → `own`; a read-only file → `attribute`; a plain file → null).
  - `save_writes_the_bytes_and_answers_the_new_stat`: the file holds the body; `hash` is the body's hash; `snapshot` is Some; `History::get` gives the old bytes.
  - `save_creates_a_deleted_file_with_expect_none` → `snapshot` null.
  - `save_of_the_same_bytes_writes_nothing` (the modified time is unchanged, `snapshot` null).
  - `save_conflicts_when_the_disk_changed` → 409 with the disk's hash, file untouched, no snapshot; `save_conflicts_when_the_file_is_gone` → 409 `hash: null`; `save_conflicts_when_expect_none_finds_a_file`.
  - `save_refuses_a_bad_path`: the table from Review Focus 1 that is a 400, each leaving the disk as it was.
  - `save_refuses_marshells_own_folder` and `save_refuses_a_managed_name` → 403 with each line.
  - `save_refuses_a_read_only_file` → 403 with the read-only line.
  - `save_refuses_a_body_over_the_cap` → 413.
  - `two_saves_of_one_file_run_one_after_the_other`: two concurrent PUTs with the same `expect` → one 200 and one 409.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test save_api` and `cargo test -p marshell-core -j 4 userfiles` → FAIL.
- [ ] **Step 3: Implement.** `save.rs` holds the handler and nothing else writes a user's file. Change the one line in `.claude/rules/local-api.md`.
- [ ] **Step 4: Run** both test commands and `cargo test -p marshell-core -j 4 files` → PASS; clippy clean.
- [ ] **Step 5: Commit** `feat(core): a text tab's save writes the file in place after a hash check and a snapshot, and refuses Marshell's own and managed files.`

### Task 3: Undoing a save (main thread)

**Files:**
- Modify: `crates/core/src/server/save.rs`, `crates/core/src/server/mod.rs`
- Test: `crates/core/tests/save_api.rs`

**Interfaces:**
- Consumes: Task 2's checks, lock and `write_in_place`; `History::{get, take}`.
- Produces: `POST /v1/files/unsave` with `FileUnsave`. It runs Task 2's path checks on `path`, loads the snapshot, and requires `same_path(path, snapshot.path)`. Then, under the save lock: the file's hash must equal the snapshot's `after_hash`; it takes a snapshot of the current bytes, writes the snapshot's `before` in place, and answers 200 `FileSaved` (`snapshot` is the new one's id). Answers besides: 404 for an unknown or malformed snapshot or another file's; 409 `FileConflict` when the file changed or is gone; the 400 and 403 answers of the save.

- [ ] **Step 1: Write the failing tests** in `save_api.rs`: `unsave_is_refused_without_the_bearer`, `unsave_puts_back_the_bytes_before_the_save`, `unsave_keeps_what_it_replaced` (the saved bytes read back from the answer's `snapshot`), `unsave_conflicts_when_the_file_changed_since` (file untouched), `unsave_conflicts_when_the_file_is_gone`, `unsave_of_another_files_snapshot_is_a_404`, `unsave_of_a_malformed_id_is_a_404` (`"../x"`, `""`), `unsave_refuses_marshells_own_folder`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test save_api unsave` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 --test save_api` → PASS; clippy clean.
- [ ] **Step 5: Commit** `feat(core): a save can be undone while the file still holds what the save wrote.`

### Task 4: Drafts in the core (worker)

**Files:**
- Create: `crates/core/src/userfiles/drafts.rs`, `crates/core/src/server/drafts.rs`
- Modify: `crates/core/src/userfiles/mod.rs`, `crates/core/src/server/mod.rs`
- Test: `crates/core/tests/drafts_api.rs`

**Interfaces:**
- Consumes: Task 1's `Draft`, `DraftInfo`, `DraftList`, `paths::drafts_dir`; `files::{refused, TEXT_CAP}`; `store::{read, write}`; `config::hash::hash_bytes`.
- Produces:
  - `pub struct Drafts`, `Drafts::new(home: &Path)`, `fn list(&self) -> Vec<DraftInfo>` (newest first), `fn get(&self, path: &str) -> Option<Draft>`, `fn put(&self, draft: &Draft) -> Result<(), PutError>` (`PutError::{Full, Io}`), `fn remove(&self, path: &str)`. `pub const MAX_DRAFTS: usize = 500;`
  - A draft's file is `drafts/<first 32 hex of the SHA-256 of its key>.json`. The key is the path with `/` as `\` and lowercased on Windows, and the path as it is elsewhere. `get` answers None for a file that doesn't parse or whose `path` gives another key. `put` replaces an existing draft even at the limit.
  - `GET /v1/drafts` → `DraftList`. `GET /v1/drafts/one?path=` → `Draft` or 404. `PUT /v1/drafts/one` with `Draft` → 204; body limit 16 MiB. `DELETE /v1/drafts/one?path=` → 204, also when there is none.
  - `PUT` answers 400 "That path can't be saved." for a path that is relative or `files::refused`; 400 naming the field for an `encoding` or `eol` outside the names in Global Constraints; 413 when `text` or `base_text` is over `TEXT_CAP` bytes; 503 "Marshell keeps at most 500 drafts. Save some files first." The draft routes never touch the path's own file.

- [ ] **Step 1: Write the failing tests.** In `drafts.rs`: `put_then_get`, `put_replaces`, `remove_then_get_is_none`, `list_is_newest_first`, `get_ignores_a_cut_short_file`, `get_ignores_a_file_that_holds_another_path`, `the_key_ignores_case_and_slashes_on_windows` (`#[cfg(windows)]`), `the_501st_draft_is_refused_and_a_replacement_is_not` (a `Drafts::with_limit(home, 2)` test constructor). In `drafts_api.rs`: `drafts_are_refused_without_the_bearer` (all four), `put_get_list_delete_round_trip`, `put_refuses_a_unc_path`, `put_refuses_an_unknown_encoding`, `put_refuses_text_over_the_cap`, `get_of_no_draft_is_a_404`, `delete_of_no_draft_is_a_204`, `a_draft_never_touches_its_file` (the path names no file; put, get and delete succeed and nothing is created there).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 drafts` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 drafts` → PASS; `cargo fmt --all`; clippy clean.
- [ ] **Step 5: Commit** `feat(core): unsaved edits are kept as drafts in Marshell's home, one per file.`

### Task 5: CodeMirror's packages and the table of file types (worker)

**Files:**
- Modify: `package.json`, `pnpm-lock.yaml`, `THIRD_PARTY_NOTICES.md`, `src/features/preview/kinds.ts` (`languageOf` and `kindOf` read the new table; `LANGUAGE` goes)
- Create: `src/features/editor/fileTypes.ts`, `src/features/editor/fileTypes.test.ts`, `src/features/editor/packages.test.ts`

**Interfaces:**
- Produces: `export function languageFor(path: string, firstLine?: string): string | undefined` — a Shiki language id. Order: the exact file name, then the extension, then the first line. `export const EXTENSIONS: Record<string, string>`, `export const FILE_NAMES: Record<string, string>` (keys lowercased), `export const FIRST_LINES: Array<[RegExp, string]>`.
- `kinds.ts`: `languageOf(path)` returns `languageFor(path)`; `kindOf` is unchanged for pdf, image, markdown and html, and answers `code` when `languageFor(path)` is defined, else `text`.
- The table covers what Sublime Text's default packages open and Shiki has a grammar for. At least: every extension `kinds.ts` has today; `mts cts` · `pyi pyw` · `erb rake gemspec` · `kts gradle groovy` · `scala sbt` · `clj cljs cljc edn` · `erl hrl ex exs` · `hs lhs` · `ml mli` · `fs fsx fsi` · `m mm` · `pl pm` · `r` · `d` · `pas` · `lisp el scm` · `tcl` · `tex sty cls bib` · `rst` · `adoc` · `yaml yml` · `plist xsd xsl xslt svg` (svg stays an image for `kindOf`) · `sass styl` · `hbs` · `pug` · `proto` · `tf hcl` · `nix` · `zig` · `dart` · `jl` · `vim` · `fish` · `awk` · `mk cmake` · `asm s` · `vb` · `csproj` (xml) · `gitattributes gitmodules` · `properties` · `reg` · `csv tsv` · `log` · `http`. File names: `Makefile`, `GNUmakefile`, `Dockerfile`, `Containerfile`, `CMakeLists.txt`, `Gemfile`, `Rakefile`, `Vagrantfile`, `Jenkinsfile`, `.gitignore`, `.gitattributes`, `.gitmodules`, `.gitconfig`, `.editorconfig`, `.npmrc`, `.env` (and `.env.<anything>`), `.bashrc`, `.bash_profile`, `.zshrc`, `.profile`, `Cargo.lock` (toml), `pnpm-lock.yaml`, `tsconfig.json` (jsonc), `.babelrc` (json), `.prettierrc` (json), `.eslintrc` (json). First lines: a shebang naming `sh bash zsh dash` → `bash`, `python python3` → `python`, `node` → `javascript`, `ruby`, `perl`, `php`, `pwsh` → `powershell`, `deno`/`bun` → `typescript`; `<?xml` → `xml`; `<!doctype html` (any case) → `html`.
- A table entry whose value is not a key of Shiki's `bundledLanguages` is a test failure, not a silent plain file.

- [ ] **Step 1: Add the packages:** `pnpm add @codemirror/state @codemirror/view @codemirror/commands @codemirror/search @codemirror/language @codemirror/autocomplete`. Add each, and each package the lockfile gains with them, to `THIRD_PARTY_NOTICES.md` in the file's existing format (name, version, licence, copyright line from the package's own `LICENSE`).
- [ ] **Step 2: Write the failing tests.** `fileTypes.test.ts`: `every_language_in_the_table_is_one_shiki_bundles` (imports `bundledLanguages` from `shiki/langs`); `the_old_table_is_all_still_there` (each of the extensions in today's `LANGUAGE`, with its value); `a_file_name_beats_its_extension` (`CMakeLists.txt` → `cmake`, `tsconfig.json` → `jsonc`); `names_match_in_any_case_and_with_any_folder` (`C:\x\MAKEFILE`, `/a/b/Dockerfile`); `dot_env_variants` (`.env`, `.env.local`); `a_first_line_names_the_language` (the shebangs above, with `/usr/bin/env` and with arguments; `<?xml`; the doctype); `an_extension_beats_the_first_line`; `nothing_known_is_undefined` (`notes`, `a.xyz`, an empty first line). `packages.test.ts`: `the_notices_name_every_codemirror_package` (each `@codemirror/*` and `@lezer/*` name in `pnpm-lock.yaml` appears in `THIRD_PARTY_NOTICES.md`); `index_html_has_no_style_element` (a `<style` in `index.html` would make Tauri add a nonce to `style-src`, which turns CodeMirror's injected styles off).
- [ ] **Step 3: Run** `pnpm test src/features/editor` → FAIL. **Implement**, then run it again with `pnpm test src/features/preview` → PASS; `pnpm build` passes.
- [ ] **Step 4: Commit** `feat(editor): CodeMirror 6 joins the tree, and a file's type comes from its name, its extension or its first line.`

### Task 6: Reading and writing a text file's bytes (worker)

**Files:**
- Create: `src/features/editor/textFile.ts`, `src/features/editor/textFile.test.ts`
- Modify: `src/features/preview/kinds.ts` (`decodeText` goes; its callers use `decodeFile`), `src/features/preview/usePreviewFile.ts`, `src/features/preview/previewApi.ts`, `src/features/preview/reload.ts` (`StatResult`)

**Interfaces:**
- Produces, in `textFile.ts`:
  - `export type Encoding = "utf-8" | "utf-8-bom" | "utf-16le" | "utf-16be"; export type Eol = "lf" | "crlf";`
  - `export type Decoded = { kind: "text"; text: string; encoding: Encoding; eol: Eol } | { kind: "binary" }`
  - `export function decodeFile(bytes: ArrayBuffer): Decoded` — `text` keeps the file's own line endings. A BOM picks the encoding and is not part of `text`. Without a BOM the bytes are UTF-8. Binary: a NUL in the first 8 KB of a UTF-8 file, invalid UTF-8, UTF-16 with an odd count of bytes or a lone surrogate. `eol` is `crlf` when the text has more `\r\n` than lone `\n`, else `lf`.
  - `export function encodeFile(text: string, encoding: Encoding, eol: Eol): Uint8Array` — every `\r\n`, lone `\r` and lone `\n` in `text` becomes the `eol`.
  - `export function encodingLabel(e: Encoding): string` and `export function eolLabel(e: Eol): string` with the footer's copy.
- `previewApi.ts`: `Content`'s ok case gains `hash: string` (from `X-Content-Hash`, `""` when absent); `StatResult`'s ok case gains `readOnly: "own" | "managed" | "attribute" | null`.
- `usePreviewFile.ts`: the `text` phase becomes `{ phase: "text"; text: string; encoding: Encoding; eol: Eol; hash: string; readOnly: "own" | "managed" | "attribute" | null }`.

- [ ] **Step 1: Write the failing tests** in `textFile.test.ts`: `roundTrips_plain_utf8`, `roundTrips_a_bom`, `roundTrips_utf16_le_and_be`, `roundTrips_crlf`, `roundTrips_no_final_newline`, `roundTrips_an_empty_file` and `roundTrips_a_bom_alone` (each: `encodeFile(decoded.text, decoded.encoding, decoded.eol)` equals the input bytes); `mixed_endings_take_the_kind_there_is_most_of` (two CRLF and one LF → `crlf`, and the encode has three CRLF); `a_lone_cr_becomes_the_files_ending`; `binary_a_nul_in_utf8`, `binary_invalid_utf8`, `binary_utf16_with_an_odd_byte`, `binary_utf16_with_a_lone_surrogate`; `a_nul_in_utf16_text_is_text` (UTF-16 of "a" has a zero byte and is text); `labels`.
- [ ] **Step 2: Run** `pnpm test src/features/editor/textFile.test.ts` → FAIL. **Implement.** UTF-16 is decoded with `TextDecoder("utf-16le" | "utf-16be", { fatal: true })` and encoded by hand.
- [ ] **Step 3: Run** `pnpm test src/features/editor src/features/preview` → PASS (update the preview tests that build a `text` phase); `pnpm build`.
- [ ] **Step 4: Commit** `feat(editor): a text file keeps its encoding and its line endings from load to save.`

### Task 7: The colour scheme (worker)

**Files:**
- Modify: `src/styles/tokens.css`, `src/design/contrast.test.ts`, `src/features/preview/highlight.ts` (`themeVariables` goes), `src/features/preview/Preview.tsx` (no inline `--code-*`), `src/features/preview/Preview.css`, their tests

**Interfaces:**
- Produces, in `tokens.css`, each as `light-dark(oklch(…), oklch(…))`: `--code-foreground`, `--code-background`, `--code-token-keyword`, `--code-token-string`, `--code-token-string-expression`, `--code-token-constant`, `--code-token-function`, `--code-token-parameter`, `--code-token-punctuation`, `--code-token-link`, `--code-token-comment`, `--code-token-inserted`, `--code-token-deleted`, `--code-token-changed`. `--code-foreground` is `var(--text-1)` and `--code-background` is `var(--bg-base)`.
- The scheme is Marshell's own, not a port of a known theme: few hues, calm, at the chroma of the app's status and brand tokens. Keywords, strings, constants and functions are four hues a person can tell apart in both schemes; comments are quieter than the text and still 4.5:1.
- `Preview.tsx` keeps its `theme` prop only if something besides `themeVariables` reads it; otherwise the prop and its callers' argument go.
- Forced colours: `.preview` and the editor show `CanvasText` on `Canvas`, and every token colour is unset.

- [ ] **Step 1: Read the frontend-design and good-css skills.** Then **write the failing test** in `contrast.test.ts`, beside the text tiers' test: `every_code_token_is_at_least_4_5_to_1_on_every_surface` — each `--code-token-*`, in light and in dark, against `--bg-base`, `--bg-raised` and `--bg-overlay`, with the file's own colour maths; and `the_four_main_code_hues_differ` — keyword, string, constant and function are pairwise at least ΔE 15 apart in each scheme (the measure `AgentMark`'s tests use).
- [ ] **Step 2: Run** `pnpm test src/design/contrast.test.ts` → FAIL. **Implement** the tokens, move every reader of `--code-*` onto them, and add the forced-colors rule.
- [ ] **Step 3: Run** `pnpm test src/design src/features/preview src/styles` → PASS; `pnpm build`.
- [ ] **Step 4: Commit** `feat(preview): code is coloured by Marshell's own light and dark scheme, held at 4.5:1 by the contrast test.`

### Task 8: Shiki's tokens for a text that changes (worker, effort high)

**Files:**
- Create: `src/features/editor/tokens.ts`, `src/features/editor/tokens.test.ts`
- Modify: `src/features/preview/highlight.ts` (exports a tokenizer that takes and returns a grammar state)

**Interfaces:**
- Produces, in `highlight.ts`: `export type LineTokens = Array<{ from: number; to: number; cls: string }>` (offsets in the line; `cls` is the token's classes) and `export async function tokenizer(lang: string): Promise<Tokenizer | null>` with `type Tokenizer = (lines: string[], state: unknown) => { tokens: LineTokens[]; state: unknown }`. It wraps Shiki's `codeToTokens` with `grammarState`. A colour `var(--code-token-keyword)` becomes the class `tok-keyword`; italic, bold and underline add `tok-i`, `tok-b`, `tok-u`; a token with the default colour and no style is left out. A line over 2,000 characters is not given to Shiki: it gets no tokens and the state passes through.
- Produces, in `tokens.ts`, with no import from CodeMirror or the DOM:
  - `export const STEP_LINES = 100; export const COLOUR_CAP = 1024 * 1024; export const LONG_LINE = 2000;`
  - `export class TokenStore { constructor(tokenize: Tokenizer); reset(lines: string[]): void; edit(fromLine: number, removed: number, inserted: string[]): void; want(fromLine: number, toLine: number): void; step(): boolean; line(n: number): LineTokens | null; get version(): number }`.
  - `reset` and `edit` set the text. `edit` keeps the tokens of the lines above `fromLine` and marks the rest stale. `want` names the lines on screen. `step` colours at most `STEP_LINES` lines, always the earliest stale line at or above the wanted range first (a line can only be coloured after every line above it), then the wanted range, then the rest; it returns false when nothing is stale. `line(n)` is the line's tokens, or null while they are stale. `version` grows whenever `line` could answer differently.
  - A grammar state is kept at every `STEP_LINES` boundary, so an edit at line 5,000 restarts from line 4,900.
- Task 10 drives it: one `step()` per idle callback, and decorations rebuilt from `line()` when `version` changes.

- [ ] **Step 1: Write the failing tests** in `tokens.test.ts`, with a fake tokenizer that returns one token per line whose class is the count of lines seen so far in its state (so a wrong restart shows): `tokens_a_step_colours_at_most_100_lines`; `tokens_the_wanted_lines_come_before_the_rest_once_the_lines_above_are_done`; `tokens_an_edit_keeps_the_lines_above_and_stales_the_rest`; `tokens_an_edit_restarts_from_the_boundary_before_it` (the fake counts its calls' first lines); `tokens_an_edit_during_stepping_never_gives_tokens_of_the_old_text` (step, edit line 3, `line(3)` is null until stepped again, then matches a fresh store over the new text); `tokens_reset_drops_everything`; `tokens_inserting_and_removing_lines_shifts_nothing_above`; `tokens_step_is_false_when_done`; `tokens_version_grows_only_when_a_line_changes`. In `highlight.test.ts`: `tokenizer_of_an_unknown_language_is_null`; `tokenizer_carries_a_block_comment_across_calls` (TypeScript: `/*` in the first call, the next call's line is all `tok-comment`); `tokenizer_skips_a_long_line`; `tokenizer_names_classes_not_colours`.
- [ ] **Step 2: Run** `pnpm test src/features/editor/tokens.test.ts src/features/preview/highlight.test.ts` → FAIL. **Implement.**
- [ ] **Step 3: Run** the same, then `pnpm test src/features/preview` → PASS; `pnpm build`.
- [ ] **Step 4: Commit** `feat(editor): Shiki colours a changing text a hundred lines at a time, the lines on screen first.`

### Task 9: What a tab knows about its file (worker, effort high)

**Files:**
- Create: `src/features/editor/editorDoc.ts`, `src/features/editor/editorDoc.test.ts`, `src/features/editor/editorApi.ts`, `src/features/editor/editorApi.test.ts`, `src/features/editor/changedLines.ts`, `src/features/editor/changedLines.test.ts`

**Interfaces:**
- Consumes: the generated `Draft`, `DraftInfo`, `DraftList`, `FileSaved`, `FileConflict`, `FileUnsave`; `lib/api/client`'s `coreFetch`, `api`, `getEndpoint`, `ApiError`; Task 6's `Encoding`, `Eol`.
- Produces, in `editorApi.ts`: `saveFile(path, bytes: Uint8Array, expect: string | null): Promise<{ kind: "saved"; saved: FileSaved } | { kind: "conflict"; hash: string | null } | { kind: "tooLarge" } | { kind: "refused"; line: string }>`; `unsaveFile(path, snapshot): Promise<{ kind: "saved"; saved: FileSaved } | { kind: "conflict" } | { kind: "refused"; line: string }>` (a 404 is `conflict`); `listDrafts(): Promise<DraftInfo[]>`; `getDraft(path): Promise<Draft | null>`; `putDraft(draft: Draft): Promise<boolean>`; `dropDraft(path): Promise<void>`.
- Produces, in `changedLines.ts`: `export function changedLines(before: string[], after: string[]): number[]` — the 1-based numbers of the lines of `after` that are new or different, by a longest-common-subsequence over the lines between the common head and tail; when that middle is over 2,000 lines on either side, every line of `after`'s middle. A removed line marks the line that now sits where it was (the last line if it was at the end); an empty `after` gives `[]`.
- Produces, in `editorDoc.ts`, a pure reducer:
  - `export type Disk = { text: string; hash: string | null }` (`hash` null: no file).
  - `export type DocState = { base: Disk; text: string; dirty: boolean; conflict: Disk | null; deleted: boolean; saving: boolean; lastSave: { snapshot: string; hash: string } | null; marks: number[]; line: string | null; undoOffered: boolean }` — `text` is the editor's text as last reported; `line` is the status line's copy.
  - `export type DocEvent = { type: "opened"; disk: Disk; draft: Draft | null } | { type: "edited"; text: string } | { type: "disk"; disk: Disk } | { type: "gone" } | { type: "save" } | { type: "saved"; hash: string; snapshot: string | null } | { type: "saveConflict"; hash: string | null } | { type: "saveFailed"; line: string } | { type: "keepMine" } | { type: "takeDisk" } | { type: "unsave" } | { type: "unsaved"; hash: string } | { type: "unsaveRefused" } | { type: "left" }`
  - `export type Effect = { type: "setText"; text: string; marks: number[] } | { type: "putDraft" } | { type: "dropDraft" } | { type: "save"; expect: string | null } | { type: "unsave"; snapshot: string } | { type: "refetch" } | { type: "announce"; text: string }`
  - `export function step(s: DocState, e: DocEvent, name: string): { state: DocState; effects: Effect[] }` and `export const start: DocState`.
- The rules `step` holds:
  - `opened` with no draft: clean, `base` is the disk. With a draft whose text equals the disk's: clean, `dropDraft`. With a draft whose `base_hash` equals the disk's hash: dirty, the draft's text, `base` the disk. With a draft over another hash: dirty, the draft's text, `base` is `{ text: draft.base_text, hash: draft.base_hash }`, `conflict` the disk.
  - `edited`: `dirty` is `text !== base.text`; dirty → `putDraft`; back to clean → `dropDraft`. It clears `marks` and `undoOffered`.
  - `disk` while clean: `setText` with `changedLines`, `base` moves, the reload announcement. While dirty: `conflict` is set and the text stays. A `disk` equal to `base` is nothing. A `disk` equal to the dirty text: `base` moves, clean, `dropDraft`.
  - `gone`: `deleted` is true, the text stays, `base.hash` becomes null, and it is dirty when the text isn't empty.
  - `save`: nothing while `saving`, while a `conflict` is up, or while clean with a file on disk; else `saving` and `save` with `expect: base.hash`.
  - `saved`: `base` is the text that was sent with the new hash, `deleted` false, `dirty` by the text now, `dropDraft` or `putDraft` to match, `lastSave` set when a snapshot came, `undoOffered` true, the saved line and its announcement.
  - `saveConflict`: `refetch`; the line "{name} changed on disk, so it wasn't saved."; the `disk` event that follows sets the conflict.
  - `keepMine`: `base.hash` becomes the conflict's hash, `conflict` null, still dirty, `putDraft`. `takeDisk`: `setText` with the conflict's text and no marks, `base` the conflict, clean, `dropDraft`.
  - `unsave`: only with a `lastSave` and not while saving → the `unsave` effect. `unsaved`: `refetch`, `lastSave` null, the "is back" line. The `disk` event that follows meets unsaved text or not, by the rules above. `unsaveRefused`: the "can't be undone" line, `lastSave` null.
  - `left`: `marks` and `undoOffered` clear.

- [ ] **Step 1: Write the failing tests.** `changedLines.test.ts`: `nothing_changed`, `one_line_changed`, `a_line_added_in_the_middle`, `a_line_removed_marks_its_place`, `everything_removed`, `a_huge_middle_is_marked_whole`, `the_head_and_tail_are_never_marked`. `editorApi.test.ts` (a stubbed `fetch`, as `previewApi`'s tests do): each status maps to its result; `saveFile` sends `expect=none` for null and the bytes as the body. `editorDoc.test.ts`, one test per rule above, named `step_<rule>`, and for Review Focus 2 and 4: `step_a_second_save_while_saving_is_nothing`; `step_text_typed_while_saving_stays_dirty_after_saved`; `step_a_disk_change_under_edits_never_touches_the_text`; `step_keep_mine_then_save_expects_the_disks_new_hash`; `step_undo_after_a_later_edit_keeps_the_edit` (`unsaved`, then the `disk` event → conflict rule does not fire because `base` is refetched: the state is dirty with `base` the restored text); `opened_with_a_draft_over_a_changed_disk_shows_the_bar`; `opened_with_a_draft_for_a_deleted_file_is_dirty_and_deleted`; `opened_with_a_draft_equal_to_the_disk_drops_it`.
- [ ] **Step 2: Run** `pnpm test src/features/editor` → FAIL. **Implement.**
- [ ] **Step 3: Run** `pnpm test src/features/editor` → PASS; `pnpm build`.
- [ ] **Step 4: Commit** `feat(editor): a tab's file state is one reducer: dirty, the disk, a conflict, the last save and the draft.`

### Task 10: The editor itself (worker, effort high)

**Files:**
- Create: `src/features/editor/setup.ts`, `src/features/editor/setup.test.ts`, `src/features/editor/shikiBridge.ts`, `src/features/editor/lineMarks.ts`, `src/features/editor/Editor.tsx`, `src/features/editor/Editor.test.tsx`, `src/features/editor/Editor.css`, `src/features/editor/EditorFooter.tsx`
- Modify: `src/features/tasks/reviewOpen.ts` (`keepsOwnKeys`), its test

**Interfaces:**
- Consumes: Task 8's `TokenStore`, `tokenizer`, `COLOUR_CAP`; Task 6's labels; Task 5's `languageFor`.
- Produces:
  - `setup.ts`: `export function extensions(o: { readOnly: boolean; wrap: boolean; name: string; appChord: (e: KeyboardEvent) => boolean }): Extension[]` and `export const WRAP: Compartment`, `export const READ_ONLY: Compartment`. It holds: line numbers, history, `drawSelection`, the active line, bracket matching, `closeBrackets`, `indentOnInput`, the search extension with its panel at the top, `EditorState.phrases` with the copy, `EditorState.allowMultipleSelections.of(false)`, `EditorState.tabSize.of(4)`, and one keymap built from `defaultKeymap`, `historyKeymap`, `searchKeymap`, `closeBracketsKeymap` and `indentWithTab`, with `Mod-g` bound to `gotoLine` and `Alt-z` to the wrap switch. Left out of that keymap: `Mod-d`, `Mod-Shift-l`, `Mod-Shift-k`, `Alt-ArrowLeft`, `Alt-ArrowRight`, `Mod-Alt-g`, and every binding for which `appChord` answers true. `Mod-s` is not bound here: the pane handles Ctrl+S (Task 11).
  - `export function editorKeys(os: Os): string[]` — the keys the editor's keymap binds, for the test that no app shortcut is among them.
  - `shikiBridge.ts`: `export function shiki(language: string | undefined): Extension` — a `ViewPlugin` that owns a `TokenStore`, feeds it each transaction's changed line ranges, calls `want` with the viewport, runs one `step()` per `requestIdleCallback` (a `setTimeout(…, 16)` where there is none), and builds `Decoration.mark({ class })` for the viewport's lines from `line()`. Nothing is coloured while the text is over `COLOUR_CAP`.
  - `lineMarks.ts`: `export const setMarks: StateEffect<number[]>` and `export function lineMarks(): Extension` — a gutter with a mark on each numbered line, mapped through changes, cleared by the first transaction that changes the document.
  - `Editor.tsx`: `export type EditorHandle = { text(): string; setText(text: string, marks: number[]): void; focus(): void; goToLine(): void; toggleWrap(): void; reveal(line: number): void }` and `export function Editor(props: { name: string; path: string; initial: string; firstLine?: string; readOnly: boolean; wrap: boolean; target?: number; onEdit: (text: string) => void; onCursor: (line: number, column: number) => void; ref?: Ref<EditorHandle> })`. `setText` replaces the whole document in one transaction that is in the undo history and keeps the cursor's line where it can. `onEdit` fires for a document change the person made, not for `setText`. `reveal` scrolls a line to about a third from the top, as `CodeView` does for `target`.
  - `EditorFooter.tsx`: `export function EditorFooter(props: { line: number; column: number; encoding: Encoding; eol: Eol })` with the footer's copy; the hint's element id is the editor's `aria-describedby`.
  - `reviewOpen.ts`: a text box with the class `APP_KEYS_BOX = "app-keys"` gives up the app's shortcuts as the Address box does (AltGr still keeps the key). The editor's content element carries it.
- The editor's content element has `role="textbox"`, `aria-multiline="true"`, the `aria-label` from the copy and `spellcheck="false"`. The gutters are `aria-hidden`.
- `Editor.css`: the editor fills the tab's body and scrolls inside it with no visible scrollbar; the font is the pane's code font; selection, cursor, active line, matching bracket, search match, the find bar's fields and buttons, and the changed-line mark all use tokens; a forced-colors block gives each a system colour (`Highlight`/`HighlightText` for selection and matches, `CanvasText` for the cursor and the mark, `ButtonText` on `ButtonFace` for the bar's buttons).

- [ ] **Step 1: Write the failing tests.** `setup.test.ts` (CodeMirror's state and commands run in node): `the_extensions_build_a_state`; `no_app_shortcut_is_bound_in_the_editor` (for `windows`, `mac` and `linux`: no key of `editorKeys(os)` is the label of a command in `keymap.ts`'s `COMMANDS`); `the_left_out_keys_are_not_bound` (the six named above); `ctrl_g_and_alt_z_are_bound`; `a_bracket_gets_its_closing_one` (`insertBracket(state, "(")` → `()`); `one_cursor_only` (a transaction with two ranges leaves one); `read_only_refuses_a_change`; `the_phrases_are_the_plans`. `Editor.test.tsx`: `renders_a_host_with_the_footers_hint_as_its_description`; `the_footer_shows_the_position_the_encoding_and_the_ending`. In `reviewOpen`'s test: `an_app_keys_box_gives_up_the_apps_shortcuts`, `an_app_keys_box_keeps_an_altgr_key`.
- [ ] **Step 2: Run** `pnpm test src/features/editor src/features/tasks` → FAIL. **Implement.** Read the good-css skill before `Editor.css`.
- [ ] **Step 3: Run** the same and `pnpm test src/styles` → PASS; `pnpm build`.
- [ ] **Step 4: Commit** `feat(editor): a CodeMirror editor with Marshell's keys, find and replace, go to line, word wrap and changed-line marks.`

### Task 11: The editor in a text tab (worker, effort high)

**Files:**
- Create: `src/features/editor/useEditorFile.ts`, `src/features/editor/useEditorFile.test.ts`, `src/features/editor/TextTab.tsx`, `src/features/editor/TextTab.test.tsx`, `src/features/editor/ChangedBar.tsx`
- Modify: `src/features/preview/Preview.tsx` (text kinds render `TextTab`; `CodeView` stays for nothing and is deleted with its test if no caller is left), `src/features/preview/usePreviewFile.ts` (a `refetch()` in its return; the poll hands every stat and fetch to the caller), `src/features/preview/MarkdownView.tsx` and `HtmlView.tsx` (take the text as a prop, as now), `src/features/preview/Preview.css`

**Interfaces:**
- Consumes: Task 9's `step`, `start`, `DocState`, `Effect` and API; Task 10's `Editor`, `EditorHandle`, `EditorFooter`; Task 6's `encodeFile`; `useAnnouncer`'s `announce` as `Preview` already receives or reaches it.
- Produces:
  - `useEditorFile(path: string, file: FileState, o: { announce: (text: string) => void; refetch: () => void; editor: RefObject<EditorHandle | null> }): { doc: DocState; ready: boolean; dispatch: (e: DocEvent) => void }`. On the first `text` phase it fetches the draft and dispatches `opened`; `ready` is false until then, and the editor mounts only when ready. Each later `text` phase whose hash differs from `doc.base.hash` dispatches `disk`; a `deleted` phase dispatches `gone`. It runs the effects: `setText` through the handle; `putDraft` 400 ms after the last one, and at once on `visibilitychange` to hidden, on window `blur` and on unmount; `dropDraft` at once (and cancels a waiting put); `save` encodes the editor's text with the file's encoding and ending and calls `saveFile`; `unsave`; `refetch`; `announce`.
  - `TextTab(props: { entry: PaneEntry; file: Extract<FileState, { phase: "text" | "deleted" }>; kind: PreviewKind; active: boolean; onDirty: (path: string, dirty: boolean) => void; commands: Ref<TextTabCommands>; … })` with `export type TextTabCommands = { save(): void; undoSave(): boolean; goToLine(): void; toggleWrap(): void; dirty(): boolean }`. It renders, top to bottom: the read-only line or the deleted notice; the `ChangedBar` while `doc.conflict` is set; the status line (`doc.line`, with the Undo button while `doc.undoOffered` and `doc.lastSave`); the body; the footer.
  - The body: for `code` and `text`, the editor. For `markdown` and `html`, both views stay mounted and the one `entry.view` names shows; the Rendered view gets the editor's text, saved or not, updated 150 ms after the last key.
  - `ChangedBar(props: { name: string; onKeep: () => void; onTake: () => void })`: `role="group"` named by its line; two buttons. It takes no focus when it appears; the announcer reads its line.
- Keys, on the tab's section: Ctrl+S (Cmd+S on macOS) calls `save` in either view and is always prevented, so the webview's own save never opens. `Preview`'s Escape handler does nothing while the key's target is inside the editor; its `r` already skips editable targets.
- `entry.line` reveals that line once per entry, as before.
- A read-only file (`file.readOnly` set) opens with the editor read-only, the line from the copy, and no draft and no save.
- `active` false (a kept tab that isn't shown) dispatches `left` once.
- Word wrap starts on for `markdown` and `text` and off for `code` and `html`; Alt+Z and "Word wrap" switch it for the tab while it stays loaded.
- A text over `COLOUR_CAP` shows the note "Syntax colours are off for files over 1 MB." above the editor.
- A `tooLarge` answer to a save becomes `saveFailed` with "{name} is over 2 MB, so it can't be saved."; a `refused` one with "{name} couldn't be saved. {line}".
- The pane's "Updated" note doesn't show for a text tab: the marks and the announcement say it.
- `onDirty` reports each change of `doc.dirty`.

- [ ] **Step 1: Write the failing tests** in `TextTab.test.tsx` (`renderToStaticMarkup`; the reducer's states are passed through a `TextTabView` that takes `doc` as a prop, so no effect needs to run): `a_clean_file_shows_no_bar_and_no_status_line`; `a_conflict_shows_the_bar_with_keep_mine_and_take_disk`; `a_save_shows_saved_with_undo`; `no_undo_button_without_a_snapshot`; `a_deleted_file_shows_its_notice`; `each_read_only_reason_shows_its_line` (three); `markdown_renders_the_editors_text_not_the_disks`; `the_footer_names_utf16_and_crlf`. In `useEditorFile.test.ts` for the effect runner, written as a plain function `runEffect(effect, deps)` so it tests without React: `put_draft_waits_400_ms_and_sends_the_newest_text` (fake timers), `drop_draft_cancels_a_waiting_put`, `save_encodes_with_the_files_encoding_and_ending` (UTF-16 LE with CRLF → the exact bytes), `a_failed_draft_put_announces_once_until_one_succeeds`.
- [ ] **Step 2: Run** `pnpm test src/features/editor src/features/preview` → FAIL. **Implement.**
- [ ] **Step 3: Run** the same → PASS; `pnpm build`.
- [ ] **Step 4: Commit** `feat(pane): a text tab is an editor that saves with Ctrl+S, keeps a draft, and shows a bar when the disk changes under it.`

### Task 12: The unsaved dot, the close question and the palette (worker)

**Files:**
- Create: `src/features/editor/SaveDialog.tsx`, `src/features/editor/SaveDialog.test.tsx`, `src/features/editor/drafts.ts`, `src/features/editor/drafts.test.ts`
- Modify: `src/features/pane/SidePane.tsx`, `SidePane.css`, `src/features/pane/paneCommands.ts`, `src/lib/keymap.ts`, `src/lib/appShortcuts.ts`, `src/features/sessions/LiveApp.tsx`, the palette's command filter, their tests; `src/features/browser/coverScan.test.ts` only if the dialog needs a listed reason (it shouldn't: it calls `useCoverPage`)

**Interfaces:**
- Consumes: Task 11's `TextTabCommands` and `onDirty`; Task 9's `listDrafts`, `dropDraft`; `useDialogFocus`, `useCoverPage`.
- Produces:
  - `drafts.ts`: `export function staleDrafts(drafts: DraftInfo[], tabPaths: string[], now: number): string[]` — the paths of drafts no tab's current entry shows and whose `edited_ms` is over 30 days old; paths compare without case and with either slash on Windows. `export function samePath(a: string, b: string, os: Os): boolean`.
  - The window keeps `unsaved: Set<string>`: filled from `listDrafts()` once the layout has loaded (after dropping `staleDrafts`), then kept by `onDirty`.
  - `SidePane`: a tab whose current entry's path is in `unsaved` shows a dot in the slot `NewMark` uses (the opener's mark wins while both apply), its name's text ends ", unsaved" and its tooltip " · Unsaved edits". Forced colours: the dot is `CanvasText`.
  - `SaveDialog(props: { name: string; onChoose: (choice: "save" | "discard" | "cancel") => void })`: a modal dialog with the copy; focus starts on Save; Esc is Cancel.
  - Closing a tab whose file is unsaved (the × button, a middle click, Delete in the strip, "Close tab") opens the dialog first. Save: the tab's `save()`, and the tab closes when the save succeeds; a refused or conflicting save leaves the tab open with its line. Don't save: `dropDraft`, then close. Cancel: nothing. A tab that isn't loaded (no `TextTabCommands`) is selected first, then asked.
  - `keymap.ts`: the four commands from Global Constraints, group "Window", no binding. `appShortcuts.ts`: `{ kind: "paneSave" }`, `{ kind: "paneUndoSave" }`, `{ kind: "paneGoToLine" }`, `{ kind: "paneWordWrap" }`. `LiveApp`'s `paneRun` calls the selected tab's `TextTabCommands`. The palette leaves the four out unless the selected tab is a text tab; "Undo last save" also needs `undoSave` to be possible.

- [ ] **Step 1: Write the failing tests.** `drafts.test.ts`: `a_draft_with_a_tab_is_never_stale`, `a_tabless_draft_is_stale_after_30_days` (29 days: kept; 31: dropped), `paths_match_without_case_on_windows_only`. `SaveDialog.test.tsx`: `it_has_the_plans_copy_and_three_buttons`, `it_is_a_modal_dialog_named_by_its_title`. `SidePane`'s test: `an_unsaved_tab_shows_the_dot_and_says_unsaved`, `the_openers_mark_wins_over_the_dot`. `paneCommands`' test: `closing_an_unsaved_tab_asks_first` (a pure `closeAsk(tab, unsaved)` answers `"ask"` or `"close"`). The palette's test: `the_four_file_commands_show_only_for_a_text_tab`. `coverScan.test.ts` passes with the dialog in the tree.
- [ ] **Step 2: Run** `pnpm test src/features/editor src/features/pane src/lib src/features/browser` → FAIL. **Implement.** Read the good-css skill before the CSS.
- [ ] **Step 3: Run** `pnpm test` (the whole suite, failures only) → PASS; `pnpm build`.
- [ ] **Step 4: Commit** `feat(pane): an unsaved file shows a dot on its tab, closing it asks first, and the palette saves, undoes a save, goes to a line and wraps.`

### Task 13: The screen-reader script and the stories (worker)

**Files:**
- Modify: `docs/a11y/screen-reader-script.md`, the review page or stories that show `CodeView` today (search `CodeView` under `src/`), `docs/PLAN.md` only where a sentence still says a file tab is read-only

**Interfaces:**
- The script gains an "Editor" section for the release sign-off: open a file; the editor's name and its hint are read; type and hear nothing spurious; Ctrl+F reads "Find"; Esc then Tab leaves; Ctrl+S is announced as saved; the bar's line is announced when the disk changes; the dialog is read on close.
- A story or review-page entry for each of: the editor clean, with the bar, with "Saved" and Undo, read-only, and the dialog.

- [ ] **Step 1: Implement**, then run `pnpm test` (failures only) and `pnpm build` → PASS.
- [ ] **Step 2: Commit** `docs(a11y): the editor joins the screen-reader script and the review page.`

### Task 14: Live check, docs, review and the pull request (main thread)

**Files:**
- Create: `docs/research/2026-11-18-text-editor-live-check.md`
- Modify: `docs/PLAN.md` §9 (what was built that differs; the numbers the check measured)

- [ ] **Step 1: Run what CI runs:** `cargo fmt --all --check`, clippy, `cargo test --workspace -j 4`, `pnpm check:generated`, `pnpm test`, `pnpm build`.
- [ ] **Step 2: Live check** with the `marshell-live-check` skill, against scratch homes, on files in a scratch project folder. Each line is a numbered finding in the note, pass or fail:
  1. A `.ts`, `.rs`, `.py`, `.json`, `.md`, `.ps1`, `Makefile` and a shebang script each open coloured, in light and in dark.
  2. Typing, undo and redo, Tab and Shift+Tab, a bracket's closing pair, Ctrl+F with replace and replace all, Ctrl+G, Alt+Z.
  3. Esc then Tab leaves the editor; Ctrl+M switches Tab; Ctrl+Shift+Tab goes to the terminal and back with the cursor where it was; Ctrl+Shift+K opens the palette from the editor; Alt+Left goes back in the tab.
  4. Ctrl+S saves: the bytes on disk are the editor's, for UTF-8, a BOM, UTF-16 LE and CRLF (byte compare against a file written by a script). "Saved" shows; Undo puts the old bytes back and the editor's text stays, now unsaved.
  5. A script changes a clean tab's file: it reloads within about a second, the changed lines are marked, and typing clears the marks.
  6. A script changes a file under unsaved edits: the bar shows; Keep mine then Ctrl+S overwrites; Take disk replaces the text and Ctrl+Z brings the edits back.
  7. Ctrl+S right after a script's change (inside the poll's second): nothing is written and the bar shows.
  8. A draft survives a session switch, five other tabs opened after it (so it unloads), and a restart of the app; the dot is on its tab each time. Text typed within a second of quitting: note whether it survived.
  9. Closing an unsaved tab asks; each of the three answers does what it says. Over a browser tab's page the dialog freezes the page.
  10. A file in the scratch Marshell home, a `managed-settings.json` in the project, and a read-only file each open read-only with their line, and Ctrl+S writes nothing.
  11. A Markdown file: typing in Source shows in Rendered without a save; the toggle keeps the undo history.
  12. Time to first colours and to full colours for 300 KB, 1 MiB and 2 MiB TypeScript files, and whether typing at the end of each stays smooth. The cap in the plan moves if it must, and PLAN §9 records the number.
  13. Forced colours (emulated): text, selection, cursor, matches, the find bar, the bar, the dot and the marks are all visible.
  14. The webview's console shows no CSP error after all of the above, and `pnpm tauri build --no-bundle`'s `index.html` has no nonce in `style-src`.
- [ ] **Step 3: Write the note,** fix what failed (each fix with a test where one can hold it), and bring PLAN §9 in line.
- [ ] **Step 4: Final review** by `marshell-reviewer` on Opus with this plan's Review Focus, then one fix pass.
- [ ] **Step 5: Ship** with the `marshell-ship` skill: the PR body has "What it does", "Live check", "Known limits" and "Checks"; a green CI; the `pre-merge-review` skill; the merge. Then delete the ledger and update the memory notes.
