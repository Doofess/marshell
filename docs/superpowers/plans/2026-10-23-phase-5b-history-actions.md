# History actions (slice 5b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Who implements:**
> - Tasks 1 to 6 go to `worker` (Sonnet), one at a time, on `phase-5b`, with `effort: high` for Tasks 1, 2 and 3 (the index, concurrency on the marks files, deleting files, and the save command).
> - The main thread reads each diff, runs the task's tests, and ticks the task.
> - Task 7, the live check, stays in the main thread.

**Goal:** The History view can rename, tag, pin, bookmark, fork, export (markdown or JSON, redacted by default), archive and delete Marshell's copy of a conversation, and Ctrl+Shift+B bookmarks the current point of a live Claude Code session.

**Architecture:**
- **Marks file.** Each archived conversation gets a marks file, `<archive dir>/<id>.marks.json`, beside its sidecar. It holds the person's name, tags, pin, archive flag and bookmarks, and it is the source of truth.
- **Index.** The index (schema 3) caches the marks so the list, the filters and search stay fast. A rebuild reads the marks files back.
- **Bookmarks** are stored as times (`at_ms`) and resolved to a turn when read, because a live session's turn number isn't known until its transcript is parsed.
- **Names.** A conversation and its tab share one name. `History` keeps the names the person gave tabs, and a conversation that starts in such a tab takes that name.
- **Saving.** Export is built by the core and saved by a new Tauri command, `save_conversation`, which opens the native Save dialog from Rust, as `save_diagnostics` does.

**Tech Stack:** Rust (axum, rusqlite 0.40 `bundled`, serde_json, ts-rs, tauri-plugin-dialog already in the shell), React + TypeScript, Vitest (node, `renderToStaticMarkup`).

**Spec:**
- `docs/PLAN.md` §9 phase 5:
  - "Slice 5b decisions (2026-10-23)" 1 to 4;
  - "Slice 5a decisions" 1 (slice list) and 3 (keys);
  - the "Resume" bullet.
- Brief §5.10: "Actions", "Titles" and "Retention and backup" (redaction on export).
- PLAN "Secrets at rest": redaction happens on export.
- PLAN line 571: the index has tags and bookmarks tables.

## Global Constraints

- **No new dependency.**
- **Security surface.**
  - The CSP, `capabilities/`, `tauri.conf.json` and `guard.rs` don't change.
  - New routes sit in the token-guarded router beside the 5a history routes.
  - Every route with `{agent}/{session_id}` runs the existing `check(agent, session_id)` before any filesystem call (404 "No such conversation.").
- **Claude Code's files are never written or deleted.**
  - "Delete Marshell's copy" removes only files under `<home>/history/claude/`: `<id>.jsonl.zst`, `<id>.meta.json`, `<id>.marks.json` and their `.bak` files.
  - The `transcript_path` file must still exist after a delete, and a test proves it.
- **Writes.** Marks files go through `store::write` (versioned, atomic, `.bak`). Read-modify-write of one conversation's marks holds a `History` lock, so two quick edits never lose one.
- **Untrusted text.** Names and tags render as text, never HTML. Export redaction uses `diagnostics::redact::global()`.
- **Limits.**
  - Name: trimmed, at most 120 characters, no control characters (they become spaces). Empty clears the name.
  - Tags: trimmed, 1 to 40 characters, no commas or control characters, at most 20 per conversation, kept in the order given. Duplicates are dropped case-insensitively, keeping the first spelling.
  - Bookmarks: at most 200 per conversation; a 201st drops the oldest.
- **Keyboard and forced colours.** Every new control works with the keyboard alone and in forced colours (AGENTS.md). Read the `frontend-design` skill before writing styles, and the `good-css` skill if it is installed.
- **Commits.**
  - Run `cargo fmt --all` before each Rust commit.
  - Messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  - Regenerated `src/generated/` files go in the commit that changes them.
- **Exact copy:**

  **Buttons and labels**
  - Conversation buttons: "Resume", "Fork", and "More actions". The menu offers, in this order:
    - "Rename…"
    - "Tags…"
    - "Pin" or "Unpin"
    - "Archive" or "Unarchive"
    - "Export…"
    - "Copy as markdown"
    - "Delete Marshell's copy…"
  - Rename box label: "Conversation name". Hint under it: "Leave empty to use Claude Code's title."
  - Tags box label: "Tags, separated by commas".
  - The list's first group heading: "Pinned".
  - Row chip: "Archived".
  - Turn bookmark button: label "Bookmark turn {n}", with `aria-pressed`. The visible text is "Bookmark", or "Bookmarked" when pressed.
  - Filters: a "Tag" select whose first option is "Any", and checkboxes "Bookmarked" and "Show archived".

  **Delete**
  - Confirm: `Delete Marshell's copy of “{title}”? Claude Code's own file is not touched.`, with buttons "Delete" and "Cancel".
  - Done: `Deleted Marshell's copy of “{title}”.`
  - Running: "This conversation is running in a tab. Close the tab first, then delete Marshell's copy."
  - Copy in progress: "Marshell is saving this conversation right now. Try again in a moment."

  **Rename**
  - Announced as `Renamed to “{name}”.` or "Name cleared."

  **Tags**
  - Errors: "A conversation can have up to 20 tags.", "A tag can be up to 40 characters." and "A tag can't contain a comma."

  **Bookmarks**
  - Live bookmark, done: `Bookmarked this point in “{title}”.`
  - Nothing yet: "Nothing to bookmark yet: the conversation starts with its first prompt."
  - Another agent: "Bookmarks work in Claude Code sessions for now."
  - Bookmarked filter, empty: `No bookmarks yet. Press {shortcut} in a Claude Code session, or bookmark a turn here.` The shortcut comes from `shortcutLabel` for the OS.

  **Export dialog**
  - Title: "Export conversation".
  - Format radios: "Markdown" and "JSON".
  - Checkbox: "Hide secrets and personal paths", checked by default.
  - Preview label: "Preview".
  - Buttons: "Save…", "Copy" and "Close".
  - Announced: `Saved to {path}.` and "Copied as markdown." (or "Copied as JSON.").

  **Pin and archive**
  - Announced as "Pinned.", "Unpinned.", "Archived." and "Unarchived.".

  **Fork**
  - Tab name: `{name} (fork)` when the conversation has a name the person gave it, otherwise the default tab name.

## Review Focus

1. **Hostile names and tags.**
   - The input: newlines, U+202E, NUL, a 10,000-character name, `<img src=x onerror=alert(1)>`, a tag list of 25, `a,b` as one tag, and `Bug` with `bug`.
   - Expected: names are normalized or cut, tags are refused or de-duplicated with the messages above, and everything renders as text.
   - Tests: the Task 1 normalizer table, and the Task 5 escaped-render test.
2. **A delete that reaches the wrong file.**
   - The input: `DELETE /v1/history/claude/..%2Fx`, a conversation running in a tab, one whose copy is in progress, and a sidecar whose `transcript_path` points at a real file.
   - Expected: 404 or 409 as listed, and Claude's file still exists afterwards.
   - Tests: Task 2.
3. **Rebuilding the index loses the marks.**
   - The input: delete `index.sqlite`, or open a schema-2 index from 5a.
   - Expected: names, tags, pins, archive flags and bookmarks come back from the marks files.
   - Tests: Task 1.
4. **Redaction misses in JSON.**
   - The input:
     - a prompt holding `sk-ant-abcdefgh12345678`;
     - a `password: "x\"y"` inside assistant text;
     - a cwd under the user's profile folder;
     - a tool detail with a GitHub token.
   - Expected: none appears in either format with the box ticked, all appear with it unticked, and the JSON still parses.
   - Tests: Task 3.
5. **A bookmark resolves to the wrong turn.**
   - The input:
     - a bookmark before the first prompt, and one after the last;
     - two bookmarks inside one turn;
     - a conversation with no indexed turns (limited or not yet parsed);
     - a toggle-off on a turn holding two bookmarks.
   - Expected, in order: turn 0, then the last turn, then one bookmarked turn, then turn 0, and the toggle-off removes both.
   - Tests: Task 1 (`turn_for`) and Task 2.

---

### Task 1: Marks files and index schema 3

**Files:**
- Create: `crates/core/src/history/marks.rs` (and `pub mod marks;` in `history/mod.rs`)
- Modify: `crates/core/src/history/index.rs` and `archive.rs`
- Modify: `crates/protocol/src/history.rs` (regenerate `src/generated/`)

**Interfaces:**
- Produces (core, `history::marks`):
  - `#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)] pub struct Marks { name: Option<String>, tags: Vec<String>, pinned: bool, archived: bool, bookmarks: Vec<u64> }`, with all fields `pub`, `#[serde(default)]`, and `impl store::Versioned` with `VERSION = 1`. `bookmarks` is a sorted, de-duplicated list of `at_ms` values.
  - Functions:
    - `pub fn marks_path(home: &Path, agent: &str, session_id: &str) -> PathBuf`
    - `pub fn read(home: &Path, agent: &str, session_id: &str) -> Marks`: a missing or unreadable file is `Marks::default()`, logged at warn when unreadable.
    - `pub fn write(home: &Path, agent: &str, session_id: &str, marks: &Marks) -> anyhow::Result<()>`
    - `pub fn normalize_name(input: &str) -> Option<String>`
    - `pub fn normalize_tags(input: &[String]) -> Result<Vec<String>, &'static str>`, where the error is one of the three tag messages.
    - `pub fn add_bookmark(marks: &mut Marks, at_ms: u64)`
    - `pub fn turn_for(at_ms: u64, turns: &[(u32, u64)]) -> u32`
- Produces (index):
  - `SCHEMA_VERSION = 3`. `sessions` gains `name TEXT`, `pinned INTEGER NOT NULL DEFAULT 0` and `archived INTEGER NOT NULL DEFAULT 0`.
  - New tables:
    - `tags (agent, session_id, tag, key, PRIMARY KEY (agent, session_id, key))`, where `key` is the tag lowercased;
    - `bookmarks (agent, session_id, at_ms, PRIMARY KEY (agent, session_id, at_ms))`.
  - `create` reads each sidecar's marks file too.
  - Methods on `Index`:
    - `pub fn set_marks(&self, agent: &str, session_id: &str, marks: &Marks) -> anyhow::Result<()>`
    - `pub fn remove(&self, agent: &str, session_id: &str) -> anyhow::Result<()>`, which removes the conversation's rows from every table.
    - `pub fn turn_times(&self, agent: &str, session_id: &str) -> anyhow::Result<Vec<(u32, u64)>>`, from the session's `turns_fts` rowid block.
    - `pub fn bookmarks(&self, q: &HistoryQuery) -> anyhow::Result<Vec<SearchHit>>`
- Produces (protocol):
  - `HistoryEntry` gains `tags: Vec<String>`, `pinned: bool`, `archived: bool`, `bookmarks: u32` and `named: bool`. Its `title` becomes the person's name when one is set.
  - `HistoryQuery` gains `tag: Option<String>`, `bookmarked: Option<bool>` and `archived: Option<bool>`, where `Some(true)` means include archived.
  - `SearchHit.title` becomes the person's name when one is set.
  - `Conversation` gains `marks: ConversationMarks`, which is `{ name: Option<String>, tags: Vec<String>, pinned: bool, archived: bool, bookmarked_turns: Vec<u32> }`. The parser leaves it at its default, and Task 2 fills it in.
  - New types:
    - `HistoryPatch { name: Option<String>, tags: Option<Vec<String>>, pinned: Option<bool>, archived: Option<bool> }`, where `name: Some("")` clears the name;
    - `Bookmarked { title: String }`.

- [ ] **Step 1: Write the failing tests.**
  - In `marks.rs`:
    - `normalize_name_table`:
      - `"  x  "` gives `x`;
      - `""` and `"   "` give `None`;
      - `"a\nb\u{0}c"` gives `"a b c"`;
      - `"\u{202E}x"` keeps `x` with the U+202E turned into a space and trimmed;
      - 10,000 characters are cut to 120.
    - `normalize_tags_table`:
      - `[" Bug ", "bug", "ui"]` gives `["Bug", "ui"]`;
      - empty entries are dropped;
      - `"a,b"`, 41 characters and 21 tags each give their message.
    - `bookmarks_stay_sorted_unique_and_capped_at_200`.
    - `turn_for_table`:
      - turns `[(0,100),(1,200),(2,300)]`: 50 gives 0, 100 gives 0, 250 gives 1, 999 gives 2;
      - an empty list gives 0.
    - `missing_or_broken_marks_read_as_default`.
  - In `index.rs`:
    - `marks_survive_a_rebuild`: write marks for two seeded sessions, delete the index file, `Index::open`, and every field comes back through `list` and `bookmarks`.
    - `schema_2_index_upgrades_to_3_with_marks`.
    - `list_hides_archived_unless_asked`, `list_filters_by_tag_case_insensitively`, `bookmarked_filter_lists_only_bookmarked`.
    - `search_skips_archived_unless_asked`: `search` gains an `include_archived: bool` parameter. Update its callers.
    - `name_overrides_title_in_list_and_search`.
    - `bookmarks_resolve_to_turns_newest_first`: the snippet is the turn's prompt, cut at 120 characters with `…`.
    - `remove_clears_every_table`.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p marshell-core history::` should fail to compile (no `marks` module).
- [ ] **Step 3: Implement.**
  - The upgrade path is the existing move-aside-and-rebuild path. Rebuilding schema 2 to 3 re-lists from sidecars plus marks, and the backfill fills search.
  - `set_marks` replaces the tags and bookmarks rows in one transaction.
- [ ] **Step 4: Run the tests.**
  - `cargo test -p marshell-core history::` should pass.
  - `cargo test -p marshell-protocol` regenerates the TypeScript, and `pnpm check:generated` should print ok.
  - `pnpm build` should pass. Update the TS fixtures or tests that build a `HistoryEntry` or `Conversation` by adding the new fields.
- [ ] **Step 5: Commit** as `feat(history): keep each conversation's name, tags, pin, archive flag and bookmarks in a marks file the index rebuilds from`.

### Task 2: The marks API, delete, live bookmark and shared names

**Files:**
- Modify: `crates/core/src/history/recorder.rs`, `crates/core/src/server/history.rs`, `crates/core/src/server/sessions.rs` (rename), and `crates/core/src/server/mod.rs` (routes)
- Test: `crates/core/tests/history_api.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces (`History`):
  - `pub fn patch(&self, agent: &str, session_id: &str, patch: &HistoryPatch) -> Result<Marks, &'static str>`
  - `pub fn bookmark_turn(&self, agent: &str, session_id: &str, turn: u32, on: bool) -> anyhow::Result<()>`:
    - On: adds the turn's `at_ms` from `turn_times`, or `0` with no rows.
    - Off: removes every bookmark whose `turn_for` is that turn.
  - `pub fn bookmark_now(&self, tab_id: &str, at_ms: u64) -> Result<String, BookmarkRefusal>`, which returns the title.
  - `pub fn marks(&self, agent: &str, session_id: &str) -> ConversationMarks`
  - `pub fn delete_copy(&self, agent: &str, session_id: &str) -> Result<(), DeleteRefusal>`
  - `pub fn name_tab(&self, tab_id: &str, name: Option<&str>)`
  - In `note_event`, when `is_new`: if the tab has a name in `name_tab` and the conversation's marks have no name, write that name.
- Produces (routes, all token-guarded):
  - `PATCH /v1/history/{agent}/{session_id}`:
    - body `HistoryPatch`; answers 200 `HistoryEntry`, or 400 with a tag message as text.
    - A new name, when the conversation is running in a tab (the resume check's `RunningInATab` rule), also calls `registry.rename(tab, name)` and `name_tab`.
  - `PUT` and `DELETE /v1/history/{agent}/{session_id}/bookmarks/{turn}` answer 204.
  - `GET /v1/history/bookmarks` takes the `HistoryQuery` fields and answers `Vec<SearchHit>`.
  - `DELETE /v1/history/{agent}/{session_id}` answers 204, or 409 with the "Running" or "Copy in progress" text.
  - `POST /v1/sessions/{tab}/bookmark` answers 200 `Bookmarked`, or 409 with the "Nothing yet" or "Another agent" text, or 503 "History is off for this run.".
  - `GET /v1/history/{agent}/{session_id}` fills `Conversation.marks`.
  - `list` and `search` take the new query fields. `search` reads `archived=true` as including archived.
- `PATCH /v1/sessions/{tab}` (rename), after the registry accepts it:
  - calls `name_tab(tab, Some(name))`;
  - sets the name on `latest_for_tab(tab)` when there is one.

- [ ] **Step 1: Write the failing tests** in `history_api.rs`, seeding as the 5a tests do. Names and assertions:
  - `patch_sets_name_tags_pin_and_archive`: each field round-trips through `GET /v1/history`, and `name: ""` clears the name back to the parsed title.
  - `patch_refuses_bad_tags_with_the_message`: 21 tags give 400 and "A conversation can have up to 20 tags.".
  - `renaming_a_running_conversation_renames_its_tab`: the registry tab's name changes.
  - `renaming_a_tab_names_its_conversation_and_the_next_one`:
    - `PATCH /v1/sessions/{tab}` changes the conversation's title;
    - a later `SessionStart` hook for a new id in that tab lists under the same name.
  - `bookmark_turn_on_and_off`, including the two-bookmarks-in-one-turn toggle-off.
  - `bookmark_now_marks_the_tabs_conversation`: a tab with a session, then `POST /v1/sessions/{tab}/bookmark` answers 200 with the title, and `GET /v1/history/bookmarks` holds it.
  - `bookmark_now_refusals`: a tab with no session answers 409 "Nothing yet", and a non-Claude tab answers 409 "Another agent".
  - `delete_removes_only_marshells_copy`:
    - The 204 is followed by a 404 on GET and no list row.
    - The four files and their `.bak` files are gone.
    - The fixture `transcript_path` file still exists.
  - `delete_refuses_a_running_conversation` (409), and `delete_rejects_a_bad_id` (404, no file touched).
  - `marks_writes_do_not_lose_an_edit`: 20 concurrent PATCHes, half setting a tag list and half toggling the pin. The final marks equal the last of each kind applied, and no `.part` file is left.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p marshell-core --test history_api` should fail (404 on the new routes).
- [ ] **Step 3: Implement.**
  - `delete_copy` takes the marks lock and refuses while the session is in `copying`.
  - It removes the index rows first, then the files.
  - A missing file is not an error.
- [ ] **Step 4: Run the tests.**
  - `cargo test -p marshell-core --test history_api` and `cargo test -p marshell-core history::` should pass.
  - Clippy with `-D warnings` should be clean.
- [ ] **Step 5: Commit** as `feat(history): rename, tag, pin, archive, bookmark and delete Marshell's copy through the API, with tab and conversation sharing one name`.

### Task 3: Fork, names on Resume, and export with redaction

**Files:**
- Modify: `crates/core/src/profiles/detect.rs` (`ResumeTarget::Fork(&str)`, which gives `--resume <id> --fork-session` for `claude`)
- Create: `crates/core/src/history/export.rs`
- Modify: `crates/core/src/server/history.rs` and `server/mod.rs`
- Create: `src-tauri/src/export.rs` (`save_conversation`). Make `diagnostics::write_export` `pub(crate)` and reuse it. Register the command in `main.rs`.
- Create: `src/lib/exportName.ts`
- Test: unit tests in `export.rs`, `detect.rs` and `src-tauri/src/export.rs`; `history_api.rs`; `src/lib/exportName.test.ts`

**Interfaces:**
- Produces:
  - `pub fn to_markdown(c: &Conversation) -> String`
  - `pub fn redact_conversation(c: &mut Conversation, r: &Redactor)`, which redacts every string field: title, cwd, branch, prompts, markdown, tool summaries and details, and subagent fields.
  - `pub fn export_text(c: Conversation, format: ExportFormat, redact: bool) -> String`
  - `ExportFormat { Markdown, Json }`, for which JSON is `serde_json::to_string_pretty` of the redacted `Conversation`.
- Routes:
  - `POST /v1/history/{agent}/{session_id}/fork`:
    - body `ResumeRequest`;
    - answers as resume does, but with no running-in-a-tab refusal;
    - the tab is named `{name} (fork)` when named.
  - Resume opens its tab with the conversation's name when it has one. Both routes call `name_tab(new tab, name)`.
  - `GET /v1/history/{agent}/{session_id}/export?format=markdown|json&redact=true|false`:
    - `redact` defaults to true;
    - answers `text/markdown; charset=utf-8` or `application/json`;
    - with no archive, answers 404 "Marshell has no copy of this conversation yet.".
- Tauri: `save_conversation(window, request)`, with the body as raw bytes and the name in `x-file-name`.
  - The name must match `^[a-z0-9]+(?:-[a-z0-9]+)*-\d{4}-\d{2}-\d{2}\.(md|json)$` and be at most 60 characters.
  - The bytes must be valid UTF-8 and at most 64 MiB.
  - It filters to "Markdown" (`md`) or "JSON" (`json`).
  - It answers `Option<String>`, the saved path or `None`. A wrong name or wrong bytes give "Not a conversation export.".
- TS: `exportFileName(title: string, format: "markdown" | "json", now: Date): string`:
  - lowercase ASCII letters and digits, with other runs becoming `-`;
  - trimmed and cut to 40;
  - `conversation` when nothing is left;
  - then `-YYYY-MM-DD.md` or `.json` in local time.
- Markdown shape, exactly:
  ```
  # {title}

  - Folder: {cwd}
  - Branch: {branch}            (omitted when none)
  - Model: {model}              (omitted when none)
  - Claude Code: {version}      (omitted when none)
  - Session: {session_id}

  ## Turn {index + 1}

  > {each prompt line}         (prompt as a quote, one "> " per line)

  {Text markdown, verbatim}
  - {tool summary}             (one line per tool card; details left out)
  - Subagent: {description}
  ---
  *Conversation compacted*
  ```
  Blocks are separated by one blank line. Times are left out; the JSON has `at_ms`.

- [ ] **Step 1: Write the failing tests.**
  - `fork_args_add_fork_session`, in `detect.rs`.
  - `markdown_shape_matches`, against the `tools` fixture, as an exact string.
  - `redaction_covers_every_field_and_json_still_parses`, with the Review Focus 4 inputs, in both formats and both toggle states.
  - `save_name_check_table`:
    - accepts `my-chat-2026-10-23.md`;
    - refuses `../x-2026-10-23.md`, `x-2026-10-23.md.exe`, `x-2026-10-23.txt`, `X-2026-10-23.md`, and a 61-character name;
    - refuses non-UTF-8 bytes.
  - `exportFileName` cases: a Hebrew-only title gives `conversation-…`, and `"Fix: the API!"` gives `fix-the-api-…`.
  - In `history_api.rs`:
    - `fork_starts_a_tab_with_fork_session_even_while_running`: assert the command line holds `--fork-session` and the tab name `{name} (fork)`.
    - `resume_opens_the_tab_under_the_name`.
    - `export_routes_answer_both_formats_and_404_without_a_copy`.
- [ ] **Step 2: Run them and watch them fail.**
  - `cargo test -p marshell-core export`
  - `cargo test -p marshell-app export`
  - `pnpm test src/lib/exportName.test.ts`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the tests above, plus `cargo test -p marshell-core --test history_api`. All should pass.**
- [ ] **Step 5: Commit** as `feat(history): fork a conversation, resume under its name, and export it as markdown or JSON with secrets hidden by default`.

### Task 4: The list: Pinned group, archived, tags and bookmark filters

**Files:**
- Modify:
  - `src/lib/api/history.ts`
  - `src/features/history/historyState.ts`
  - `src/features/history/HistoryView.tsx` and its CSS
  - `src/features/history/LiveHistory.tsx`
- Test: the matching `*.test.ts(x)`

**Interfaces:**
- Produces (API client, `history`):
  - `patch(ep, agent, id, p: HistoryPatch): Promise<{ ok: true; entry: HistoryEntry } | { ok: false; reason: string }>`
  - `bookmarkTurn(ep, agent, id, turn: number, on: boolean): Promise<void>`
  - `bookmarks(ep, q: HistoryQuery): Promise<SearchHit[]>`
  - `remove(ep, agent, id): Promise<{ ok: true } | { ok: false; reason: string }>`
  - `fork(ep, agent, id, size): Promise<Resumed>`
  - `exportText(ep, agent, id, format: "markdown" | "json", redact: boolean): Promise<string>`
  - `sessions.bookmark(ep, tabId): Promise<{ ok: true; title: string } | { ok: false; reason: string }>`, in `src/lib/api/sessions.ts`
  - `saveConversation(text: string, name: string): Promise<string | null>`, which calls `invoke("save_conversation", bytes, { headers: { "x-file-name": name } })`
- Produces (view state):
  - `HistoryFilters` gains `tag?: string`, `bookmarked: boolean` and `archived: boolean`.
  - `FilterName` gains `"tag"`.
  - `filterOptions` returns tags.
  - `groupByProject` puts pinned entries in a first group `{ project: "\u0000pinned", label: "Pinned", entries }` and removes them from their project groups. No group is shown while searching.
  - With `bookmarked` on and no query, the list shows `SearchHit` rows from `history.bookmarks` with the existing hit row, and Enter opens at the hit's turn.
  - An archived row has the "Archived" chip, and tags show as `Chip`s in the row meta.

- [ ] **Step 1: Write the failing tests:**
  - `groupByProject puts pinned first and only once`
  - `the Tag filter offers each tag once, sorted`
  - `Bookmarked shows bookmark rows that open at their turn`
  - `Show archived passes archived=true to list and search`
  - `an archived row says Archived`
  - `the empty Bookmarked list names the shortcut`
  - an API client test for each new function's method and URL
- [ ] **Step 2: Run them and watch them fail.** `pnpm test src/features/history src/lib/api`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** `pnpm test src/features/history src/lib/api` and `pnpm build` should pass.
- [ ] **Step 5: Commit** as `feat(history): show pinned conversations first, hide archived ones, and filter by tag or bookmark`.

### Task 5: The conversation's actions and the export dialog

**Files:**
- Modify: `src/features/history/ConversationView.tsx` (header actions, the turn bookmark button, the inline rename, tags and delete-confirm editors)
- Create: `src/features/history/ExportDialog.tsx` (with a test)
- Test: `ConversationView.test.tsx`

**Interfaces:**
- Consumes: Task 4's client.
- `ConversationView` gets new props:
  - `onFork`
  - `onPatch(p: HistoryPatch)`
  - `onBookmark(turn: number, on: boolean)`
  - `onExport()`
  - `onCopyMarkdown()`
  - `onDelete()`
  - `editor: "rename" | "tags" | "delete" | null` and `onEditor(e)`
  - `error?: string`
- "More actions" uses `ContextMenu`, anchored to its button. Each editor is inline in the header:
  - The editor's input or Delete button takes focus.
  - Enter saves, and Esc cancels and returns focus to "More actions".
- The tag editor's error shows under the box, tied to it with `aria-describedby`.
- `ExportDialog`:
  - props: `{ title, load(format, redact): Promise<string>, onSave(text, name): Promise<string | null>, onCopy(text): Promise<void>, onClose, announce }`;
  - a modal `role="dialog"` that follows the diagnostics dialog's structure;
  - the preview is a read-only `<pre tabindex="0">` labelled "Preview";
  - it re-loads when the format or the box changes.

- [ ] **Step 1: Write the failing tests:**
  - `the header offers Resume, Fork and More actions`
  - `the menu lists the actions in order, with Unpin and Unarchive when set`
  - `a name with HTML renders as text`
  - `each turn has a Bookmark toggle with aria-pressed`
  - `the delete confirm names the title and says Claude's file is kept`
  - `the tag error is tied to the box`
  - `ExportDialog starts on Markdown with secrets hidden`
  - `Save names the file with exportFileName`
- [ ] **Step 2: Run them and watch them fail.** `pnpm test src/features/history`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** `pnpm test src/features/history` and `pnpm build` should pass.
- [ ] **Step 5: Commit** as `feat(history): rename, tag, pin, archive, export, copy and delete from the conversation, and bookmark any turn`.

### Task 6: Wiring, and Ctrl+Shift+B

**Files:**
- Modify: `src/features/history/LiveHistory.tsx`
- Modify: `src/features/sessions/LiveApp.tsx`, following how the `history` command was wired in 5a T6
- Modify: the session row menu, whose "Bookmark" item has been hidden since `docs/superpowers/plans/2026-10-17-session-menu.md`
- Modify: the shortcut sheet's list of commands that run

**Wiring:**
- `LiveHistory` connects each action to the client and announces with the copy above.
- After a change it refreshes the list and the open conversation.
- Fork opens its tab the way Resume does (`onResumed(tabId)`).
- After a delete, the conversation closes and focus goes to the list.
- `bookmark`, from Ctrl+Shift+B, the palette, or the row menu for a Claude tab, calls `sessions.bookmark` on the selected tab and announces the answer.
- Opening History, or a session rename, refreshes the list's titles.

- [ ] **Step 1: Write the failing tests:**
  - `bookmark runs on Ctrl+Shift+B and announces the title`
  - `bookmark announces the refusal`
  - `the shortcut sheet lists Bookmark this point`, by updating `ShortcutSheet.test.tsx`'s off list to drop `"bookmark"`
  - `the row menu shows Bookmark for a Claude tab only`
  - `a delete closes the conversation and refocuses the list`
  - `Fork hands the new tab to onResumed`
- [ ] **Step 2: Run them and watch them fail.** `pnpm test src/features`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the full checks:** `pnpm test`, `pnpm build`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm check:generated`. All should pass.
- [ ] **Step 5: Commit** as `feat(history): bookmark a live Claude Code session with Ctrl+Shift+B and wire every History action`.

### Task 7: Live check (main thread)

- [ ] **Step 1: Set up.**
  - Seed a scratch home with `seed_history_home`.
  - Start `pnpm tauri dev` with `MARSHELL_HOME` set to the scratch folder.
  - Also set **`CLAUDE_CONFIG_DIR` to a scratch folder** (memory: a PATH shim does not reach sessions).
  - Use CDP on port 9333.
  - Before and after, check that the real `~/.claude/settings.json` modified time and `projects\` are unchanged.
- [ ] **Step 2: Check each action over CDP:**
  - rename, round-tripping to a resumed tab's name;
  - tags and the Tag filter;
  - Pin and the Pinned group;
  - Archive and Show archived, with search hiding archived conversations;
  - Ctrl+Shift+B and the per-turn bookmark, with the Bookmarked filter jumping to the turn;
  - Fork, checking the command line has `--fork-session` and the tab is named `… (fork)`;
  - Export in both formats, toggling redaction, and checking the preview hides a seeded `sk-ant-` key;
  - Save…, which opens the native dialog; cancel it, and note that saving is covered by tests;
  - Copy as markdown;
  - Delete Marshell's copy, then check the files are gone and Claude's file is kept;
  - the keyboard alone, and forced colours (emulated).
- [ ] **Step 3: Write up and finish.**
  - Write `docs/research/2026-10-23-history-actions-live-check.md` and link it from PLAN.md's phase 5 line.
  - Stop the processes by PID after listing the tree.
  - Ask before removing the scratch folders.
