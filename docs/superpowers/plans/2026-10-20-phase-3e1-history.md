# History (slice 3e1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every Claude Code session run in a Marshell tab is kept: its transcript is copied, compressed, into the Marshell home on every `Stop` and at `SessionEnd`. A SQLite index lists the sessions and is checked, and rebuilt if broken, on every start. Resume and Restart pick up the tab's own conversation by session id.

**Architecture:**
- A new `crates/core/src/history/` module has three parts:
  - pure archive helpers (`archive.rs`);
  - the SQLite sessions index (`index.rs`);
  - a `History` service (`recorder.rs`) that the hook route feeds and the resume routes read.
- **The source.** The hook route already sees every Claude hook body. The body's `session_id`, `transcript_path` and `cwd` are the only input.
- **The sidecars.** Each archived session has a `<session>.meta.json` sidecar. The index is a cache rebuilt from the sidecars, never from transcript contents.
- **Resume.** It asks `History` for the tab's latest session, and runs `claude --resume <id>` when that session is known.

**Tech Stack:** Rust 2024, rusqlite 0.40 (`bundled`, moved from dev-dependencies), zstd (new), axum, tokio `spawn_blocking`.

**Spec:**
- `docs/PLAN.md`:
  - §4 "Data model" (the `history/` layout and SQLite);
  - phase 3 scope ("Transcript archive copy, history index…") and the exit line "corrupting `index.sqlite` is repaired on start";
  - §9 "Slice 3e decisions (2026-10-20)";
  - phase 5 "Resume" ("Marshell never rewrites a CLI's conversation files");
  - cross-cutting "Secrets at rest".
- `docs/adr/0001-threat-model.md`.
- Fixtures: `crates/core/tests/fixtures/claude/2.1.302/{SessionStart,Stop,SessionEnd}.json`.

## Global Constraints

- **Layout:**
  - `<home>/history/archive/claude/<session_id>.jsonl.zst` with `<session_id>.meta.json` beside it.
  - The index is `<home>/history/index.sqlite`.
  - Paths come from new helpers in `crates/core/src/paths.rs`.
- **Read only:** transcripts are only ever read. Nothing under `~/.claude` (or any CLI folder) is written, renamed or deleted.
- **Hook latency:** the hook route never waits for a copy. Index writes and copies run on `tokio::task::spawn_blocking`, and the response is sent as today.
- **Locks:** never hold a `MutexGuard` across `.await` or file I/O that can block for long. The index connection lives behind a `std::sync::Mutex`, and is used only inside `spawn_blocking`.
- **Owner-only storage:** folders via `store::create_private_dir`, files via `store::write_bytes_atomic` or `store::write` (0600 on Unix from creation; Windows inherits the home's ACL).
- **Validating hook input:**
  - A `session_id` is used only when it is 1–64 chars of `[A-Za-z0-9_-]`.
  - A `transcript_path` is used only when all of these hold:
    - it is absolute;
    - it is not a UNC or device path (`crate::hooks::risk::is_remote_path`, with no filesystem call before that check);
    - its file name is exactly `<session_id>.jsonl`.
  - Anything else is skipped silently for history, and the event still folds into live status as today.
- **Scope:**
  - Only `agent == "claude"` on an open tab, from `HookRole::Event` bodies.
  - The bodies that count are `SessionStart`, `Stop` and `SessionEnd`, plus any event whose (agent, session_id) the index hasn't seen for that tab.
  - Status line bodies are ignored for history.
- **Resume:**
  - When the tab's latest session is known and its transcript file still exists, the argv gets `--resume <id>` instead of `--continue`.
  - When it is known but the transcript is gone, the route refuses with "Claude Code no longer has this conversation, so it can't be resumed." (409, like the other `cannot_resume` refusals). It never falls back to another conversation.
  - Unknown means `--continue` as today. Codex and Antigravity are unchanged.
- **Index:**
  - The schema is version 1 (`PRAGMA user_version = 1`), with the default rollback journal (no WAL).
  - A file that fails to open, or whose `PRAGMA integrity_check` isn't `ok`, is renamed to `index.sqlite.broken-<unix ms>` and rebuilt from the sidecars.
  - A file with `user_version` above 1 is left alone, and history is off for that run (logged).
- **Dependencies:** `zstd` (default features, level 3) is added to `crates/core`, and `rusqlite` with `bundled` moves from dev-dependencies to dependencies. Nothing else.
- **No UI, protocol or generated-type changes** in 3e1.
- Run `cargo fmt --all` before every commit. Commit messages follow the repo's plain-English style and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Scratch only:** tests use temp homes. The live check uses a scratch `MARSHELL_HOME` and a scratch project, never the real `~/.marshell`.

## Review Focus

- **A hostile or malformed hook body**, such as a `session_id` of `../../x`, an empty one or one of 200 chars, or a `transcript_path` that is UNC, relative or names another file. Nothing is written outside `history/archive/claude/`, no filesystem call touches a UNC path, and live status still updates. Tested in Task 1 (`rejects_unsafe_ids_and_paths`) and Task 3 (`hostile_body_writes_nothing`).
- **A transcript that is missing, locked or unreadable at `Stop`.** The previous archive copy stays, no temp file is left, the error is logged, and the hook is answered as usual. Tested in Task 1 (`missing_transcript_keeps_the_old_copy`).
- **An index file that is garbage, empty, a folder, or from a newer Marshell.** Start continues:
  - garbage and empty files are quarantined and rebuilt from the sidecars;
  - a folder is left in place and history is off;
  - a newer version is left alone and history is off.

  Tested in Task 2.
- **Several `Stop`s in a row on a large transcript.** One copy runs at a time per session, and the final archive matches the transcript as of the last `Stop`. Tested in Task 3 (`stops_in_a_row_copy_once_at_a_time_and_end_current`).
- **`/clear` in a tab, or two tabs in one folder.** Each tab resumes its own latest session. The session before a `/clear` is linked as `previous_in_tab`. Tested in Task 2 (`latest_for_tab_picks_the_newest_session_of_that_tab`) and Task 4.

---

### Task 1: Archive helpers and dependencies

**Files:**
- Modify:
  - `crates/core/Cargo.toml` (add `zstd = "0.13"`; move `rusqlite = { version = "0.40.2", features = ["bundled"] }` to `[dependencies]`);
  - `crates/core/src/lib.rs` (`pub mod history;`);
  - `crates/core/src/paths.rs`.
- Create: `crates/core/src/history/mod.rs`, `crates/core/src/history/archive.rs`.
- Test: unit tests in both new files and in `paths.rs`.

**Interfaces:**
- Produces, in `paths.rs`:
  - `pub fn history_dir(home: &Path) -> PathBuf` (`<home>/history`);
  - `pub fn archive_dir(home: &Path, agent: &str) -> PathBuf` (`<home>/history/archive/<agent>`);
  - `pub fn index_file(home: &Path) -> PathBuf` (`<home>/history/index.sqlite`).
- Produces, in `history/mod.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub agent: String,
    pub session_id: String,
    pub tab_id: String,
    pub cwd: String,
    pub transcript_path: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    /// The session this tab had before this one (after a /clear), if any.
    pub previous_in_tab: Option<String>,
}
impl store::Versioned for SessionMeta { const VERSION: u32 = 1; }

pub fn valid_session_id(id: &str) -> bool;
/// Absolute, local, and named exactly `<session_id>.jsonl`. Makes no filesystem call.
pub fn transcript_allowed(path: &str, session_id: &str) -> bool;
```

- Produces, in `history/archive.rs`:
  - `pub fn archive_paths(home: &Path, agent: &str, session_id: &str) -> (PathBuf, PathBuf)`, giving `(<id>.jsonl.zst, <id>.meta.json)`;
  - `pub fn write_meta(home: &Path, meta: &SessionMeta) -> anyhow::Result<()>`, through `store::write`;
  - `pub fn copy_transcript(home: &Path, meta: &SessionMeta) -> anyhow::Result<PathBuf>`. It reads the whole transcript, zstd-encodes it at level 3, writes the result with `store::write_bytes_atomic`, and writes the meta. On a read error it returns `Err` and touches nothing.
  - `pub fn read_all_meta(home: &Path) -> Vec<SessionMeta>`: every readable `*.meta.json` under `history/archive/*/`. An unreadable one is skipped with a `tracing::warn!`.

- [ ] **Step 1: Write the failing tests**
  - `rejects_unsafe_ids_and_paths`:
    - `valid_session_id` is true for the fixture id `00000000-0000-4000-8000-000000000001` and false for `""`, `"../x"`, `"a/b"`, `"a b"` and a 65-char id;
    - `transcript_allowed` is true for `C:\Users\user\.claude\projects\p\<id>.jsonl` on Windows (or `/home/u/.claude/projects/p/<id>.jsonl` on Unix);
    - it is false for a relative path, `\\host\share\<id>.jsonl`, `\\?\C:\x\<id>.jsonl`, and a path ending in `other.jsonl` or `<id>.json`.
  - `copy_round_trips`: write a transcript in a temp folder and `copy_transcript`. Zstd-decoding the archive gives the same bytes, and `store::read::<SessionMeta>` of the sidecar equals the meta.
  - `missing_transcript_keeps_the_old_copy`: copy once, delete the transcript, then copy again. That returns `Err`, the archive bytes are unchanged, and no `.tmp` file is in the folder.
  - `read_all_meta_skips_broken_sidecars`: two good sidecars and one file of garbage give 2 metas.
  - `history_paths`: assertions in `paths.rs` tests, in the style of the existing ones.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib history:: paths::`. Expected: FAIL (the module doesn't exist).
- [ ] **Step 3: Implement** the interfaces above.
- [ ] **Step 4: Run** the same command. Expected: PASS. Then run `cargo clippy -p marshell-core --all-targets -- -D warnings`: clean.
- [ ] **Step 5: Commit** `feat(core): copy a Claude transcript into the archive with its sidecar`.

### Task 2: The sessions index

**Files:**
- Create: `crates/core/src/history/index.rs` (plus `pub mod index;` in `history/mod.rs`).
- Test: unit tests in `index.rs`.

**Interfaces:**
- Consumes: `SessionMeta`, `paths::index_file`, `archive::read_all_meta` (Task 1).
- Produces:

```rust
pub enum Opened {
    /// No file was there; a new empty index (filled from sidecars if any exist).
    Created { sessions: usize },
    Ok,
    /// The old file was broken: moved aside and rebuilt from the sidecars.
    Rebuilt { quarantined: PathBuf, sessions: usize },
}
pub struct Index { /* rusqlite::Connection */ }
impl Index {
    /// Err when the file is from a newer Marshell or can't be replaced (history is off this run).
    pub fn open(home: &Path) -> anyhow::Result<(Index, Opened)>;
    /// Insert or replace by (agent, session_id).
    pub fn upsert(&self, meta: &SessionMeta) -> anyhow::Result<()>;
    pub fn get(&self, agent: &str, session_id: &str) -> anyhow::Result<Option<SessionMeta>>;
    /// The tab's session with the highest updated_ms.
    pub fn latest_for_tab(&self, tab_id: &str) -> anyhow::Result<Option<SessionMeta>>;
}
```

- Schema v1: one table `sessions`. Its columns are the `SessionMeta` fields, with `PRIMARY KEY (agent, session_id)` and an index on `(tab_id, updated_ms)`, plus `PRAGMA user_version = 1`.

- [ ] **Step 1: Write the failing tests**
  - `upsert_get_and_replace`: an upsert, then a second upsert of the same key with a newer `updated_ms`, leaves one row with the new values.
  - `latest_for_tab_picks_the_newest_session_of_that_tab`:
    - tab t1 has sessions A (updated 10) and B (updated 20, `previous_in_tab: Some(A)`), and tab t2 has session C (updated 30);
    - `latest_for_tab("t1")` is B and `latest_for_tab("t2")` is C.
  - `missing_index_is_built_from_sidecars`: write 2 sidecars with `archive::write_meta`, then `open` gives `Created { sessions: 2 }`, and `get` finds both.
  - `garbage_index_is_quarantined_and_rebuilt`: an `index.sqlite` holding `b"not a database"` and 1 sidecar give `Rebuilt { sessions: 1, .. }`. The quarantined file exists and is named `index.sqlite.broken-<digits>`, and holds the garbage.
  - `empty_index_file_is_rebuilt`: a 0-byte file gives a usable index. Rebuilt and Created are both accepted, but no error.
  - `newer_index_is_left_alone`: build an index and set `PRAGMA user_version = 2`. Then `open` is `Err`, and the file's bytes are unchanged.
  - `index_path_that_is_a_folder_is_an_error`: a folder at `index.sqlite` makes `open` return `Err` with the folder still present.
  - `reopen_is_ok`: `open` twice gives `Ok` the second time.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib history::index`. Expected: FAIL.
- [ ] **Step 3: Implement `Index`**:
  - create the history folder with `store::create_private_dir`;
  - open with `rusqlite::Connection::open`, then run `integrity_check` (first row must be `ok`) and read `user_version`;
  - on a bad file, drop the connection, rename the file to `index.sqlite.broken-<unix ms>`, create a fresh one, and insert every `read_all_meta` result in one transaction.
- [ ] **Step 4: Run** the same command, then clippy. Expected: PASS, clean.
- [ ] **Step 5: Commit** `feat(core): keep a sessions index that is checked and rebuilt from the sidecars on start`.

### Task 3: Recording sessions from Claude's hooks

**Files:**
- Create: `crates/core/src/history/recorder.rs`.
- Modify:
  - `crates/core/src/server/mod.rs`: `Inner` gets `history: Arc<History>`, with `AppState::history()`, and `start` opens it before serving. `AppState::new` opens it at `home` so tests get it too.
  - `crates/core/src/server/hook.rs`: after `fold_live`, for `agent == "claude"`, `role == HookRole::Event` and an open tab.
- Test: unit tests in `recorder.rs`, plus `crates/core/tests/history.rs` (new, using `common::tie_children_to_test()` like the other integration tests).

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces:

```rust
pub struct History { /* home, Mutex<Option<Index>>, Mutex<HashMap<String, bool>> copying */ }
impl History {
    /// Opens (and checks or rebuilds) the index; logs the outcome. History is off when the index can't open.
    pub fn open(home: &Path) -> History;
    /// Validates the body, updates the index and sidecar when it is a session start, stop or end or a session not
    /// yet seen in this tab, and says whether a copy should follow. Blocking: call inside spawn_blocking.
    pub fn note_event(&self, tab_id: &str, body: &serde_json::Value, at_ms: u64) -> Option<SessionMeta>;
    pub fn latest_for_tab(&self, tab_id: &str) -> Option<SessionMeta>;
}
/// Copies on a blocking thread, one copy at a time per session; a request during a copy runs one more copy after it.
pub fn schedule_copy(history: Arc<History>, meta: SessionMeta);
```

- **`note_event` rules:**
  - Read `hook_event_name`, `session_id`, `transcript_path` and `cwd`. Return `None` when any of them is missing or fails the validation in Global Constraints.
  - When there is no row for (claude, id) or the row's tab differs:
    - `started_ms = at_ms`;
    - `previous_in_tab` = the tab's latest session id when it differs from this one.

    Otherwise keep `started_ms` and `previous_in_tab` from the row.
  - Set `updated_ms = at_ms`. Upsert and write the sidecar only for `SessionStart`, `Stop`, `SessionEnd` or a new row.
  - Return `Some(meta)` only for `Stop` and `SessionEnd`.
- **`schedule_copy` coalescing:**

```
lock copying
if copying has id: set copying[id] = true (rerun); return
insert copying[id] = false; unlock; spawn_blocking:
  loop { copy_transcript (log Err); lock; if copying[id] { copying[id] = false; continue } else { remove id; break } }
```

- [ ] **Step 1: Write the failing tests**
  - `note_event_records_start_stop_end`, fed the scrubbed fixtures with `transcript_path` rewritten to a temp file named `<id>.jsonl`:
    - SessionStart gives `None` and a row with `started_ms` set;
    - Stop gives `Some`;
    - SessionEnd gives `Some`;
    - a PreToolUse for the same session gives `None` and writes no sidecar change (the sidecar's mtime and bytes are unchanged).
  - `clear_links_previous_in_tab`: a SessionStart for A then B on tab t1 gives B with `previous_in_tab == Some(A)`, and `latest_for_tab("t1") == B`.
  - `hostile_body_writes_nothing`: for `session_id: "../../x"`, for a UNC `transcript_path`, and for a mismatched file name, `note_event` gives `None`, and `history/archive` is absent or empty.
  - `stops_in_a_row_copy_once_at_a_time_and_end_current` (integration, `tests/history.rs`):
    - the setup is a 2 MB transcript;
    - call `schedule_copy` 5 times while appending a line between calls;
    - wait until `copying` is empty (poll with a 5 s limit);
    - the decoded archive equals the final transcript bytes, and no `.tmp` is left.
  - `hook_route_archives_on_stop` (integration):
    - build the app with an open tab (the existing hook test helpers);
    - post a Claude Stop body for it through `POST /v1/hook`, with a real temp transcript;
    - within 5 s the archive and sidecar exist, and the response was 204.
  - `history_is_off_when_the_index_cannot_open`: with a folder at `index.sqlite`, `History::open` doesn't panic, `note_event` gives `None`, and the hook route still answers 204.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib history::recorder` and `cargo test -p marshell-core --test history`. Expected: FAIL.
- [ ] **Step 3: Implement** `History` and `schedule_copy`, then wire them up:
  - in `hook.rs`, call `spawn_blocking(move || history.note_event(..))`, then `schedule_copy` on `Some`;
  - await the `spawn_blocking` (it is short) but not the copy;
  - `start` logs the `Opened` outcome with `tracing::info!` or `warn!`.
- [ ] **Step 4: Run** both commands, then `cargo test -p marshell-core` and clippy. Expected: all pass, clean.
- [ ] **Step 5: Commit** `feat(core): archive a Claude session's transcript on every stop and at its end`.

### Task 4: Resume and Restart by session id

**Files:**
- Modify:
  - `crates/core/src/profiles/detect.rs`: `resume_args` and `launch_argv_resume`, plus their tests;
  - `crates/core/src/server/sessions.rs`: `resolve`, and the `resume` and `restart` handlers.
- Test: `detect.rs` unit tests, plus new cases in `crates/core/tests/resume.rs`.

**Interfaces:**
- Consumes: `AppState::history()` and `History::latest_for_tab` (Task 3).
- Produces:

```rust
// detect.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeTarget<'a> { Last, Session(&'a str) }
pub fn resume_args(agent: &str, target: ResumeTarget) -> Vec<String>; // claude Session(id) => ["--resume", id]; others as today
pub fn launch_argv_resume(profile: &Profile, agent: Option<&str>, mode: Option<SessionMode>, target: ResumeTarget) -> Result<Vec<String>, String>;
// sessions.rs
pub(super) fn resolve(st, profile_id, agent, prompt, resume: Option<ResumeTarget>, mode) -> Result<Resolved, ApiError>; // was `resume: bool`
/// Ok(Some(id)) to resume that session, Ok(None) for --continue, Err(message) when the known conversation is gone.
fn resume_session(st: &AppState, tab: &str, agent: Option<&str>) -> Result<Option<String>, &'static str>;
```

- [ ] **Step 1: Write the failing tests**
  - In `detect.rs`, `resume_by_session_id`:
    - `launch_argv_resume(&ps, Some("claude"), None, ResumeTarget::Session("abc-1"))` contains `--resume abc-1` where `--continue` was;
    - with `Some(SessionMode::Bypass)`, the mode flags still follow;
    - `Some("codex")` with `Session(..)` gives `resume --last`;
    - every existing `launch_argv_resume` test passes `ResumeTarget::Last` and is otherwise unchanged.
  - In `tests/resume.rs`:
    - `resume_refuses_a_conversation_claude_no_longer_has`:
      - an Ended Claude tab whose index row names a transcript path that doesn't exist;
      - `POST /v1/sessions/{tab}/resume` gives 409 with the exact message from Global Constraints;
      - the tab stays Ended.
    - `resume_session_prefers_the_tab_id`: a unit test of `resume_session` (make it `pub(crate)`, or test it through a small `#[cfg(test)]` hook).
      - Rows for t1 with an existing transcript give `Ok(Some(id))`.
      - No row gives `Ok(None)`.
      - A non-Claude agent gives `Ok(None)`.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib profiles::detect` and `cargo test -p marshell-core --test resume`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `resume` and `restart` call `resume_session` before `begin_resume` / `begin_restart`, so a refusal leaves the tab untouched.
  - Then they pass `Some(ResumeTarget::Session(&id))` or `Some(ResumeTarget::Last)`.
  - The other `resolve` call sites pass `None`.
- [ ] **Step 4: Run** both commands, then `cargo test -p marshell-core` and clippy. Expected: all pass, clean.
- [ ] **Step 5: Commit** `feat(sessions): resume and restart a Claude tab's own conversation by its session id`.

### Task 5: Live check, ADR note

**Files:**
- Create: `docs/research/2026-10-20-history-live-check.md`.
- Modify: `docs/adr/0001-threat-model.md` (add a "History (3e1)" entry under Entry points and controls).

**Setup:** a scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS` set to a scratch project's `.claude/settings.local.json` with the status hooks installed through the consent card, and `pnpm tauri dev`.

**Checks:**
1. Start Claude in a tab and run one prompt. After it finishes, `history/archive/claude/<id>.jsonl.zst` and its sidecar exist, and the decoded copy equals the transcript.
2. Run `/clear` and one prompt. A second session is archived, and its sidecar has `previous_in_tab` set to the first.
3. **Two Claude tabs in the same folder,** each with its own prompt, then:
   - close Marshell;
   - reopen it;
   - Resume each tab.

   Each tab shows its own conversation.
4. **The session id after a resume.** Record whether `claude --resume <id>` keeps the session id or starts a new one (and so whether the archive appends to the same file), and say so in the doc.
5. Write garbage into `history/index.sqlite`, then start Marshell. The log reports the rebuild, the `.broken-` file exists, and Resume still finds each tab's session.
6. **A conversation Claude no longer has.** Rename a transcript in the scratch `~/.claude/projects` copy (scratch only, never the user's), then Resume. The bar shows the refusal message.

The ADR entry covers three points:
- `transcript_path` and `session_id` come from a token-holding hook body, and are validated before any filesystem call.
- Archives are owner-only and not encrypted ("Secrets at rest").
- A copy can only read a file named `<id>.jsonl` at a local absolute path.

Each finding from the checks is fixed with a test that fails first, and listed with its commit.

- [ ] Run the checks, write the doc and the ADR entry, and commit: `docs: record the history live check and its threat model entry`.

### Task 6: Whole-branch review and PR

- [ ] Run the final whole-branch review on Opus, then one fix pass, each fix RED→GREEN.
- [ ] Run the CI-equivalent checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `pnpm test`
  - `pnpm check:generated`
  - `pnpm build`
  - `pnpm tauri build --no-bundle`
- [ ] Open the PR (What it does / Live check / Known limits / Checks), wait for green CI, and merge.
