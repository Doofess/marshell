# Slice 5d: conversations run outside Marshell, and Codex conversations

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** History also holds Claude Code conversations run outside Marshell and Codex CLI conversations, copied read-only from each CLI's folder, marked by where they ran, with headless runs hidden by default.

**Architecture:** A new importer in the core scans and watches `~/.claude/projects` and `~/.codex/sessions` (the `notify` crate, already a dependency), and copies each changed file into the existing archive once its size has been still for 3 s. Each sidecar and index row gains a `client` (where it ran). A new Codex rollout parser produces the same `Conversation` the Claude parser does, so the conversation view, search, Files changed and export work unchanged. The routes accept `codex` as an agent.

**Tech stack:** Rust (tokio, notify 8, zstd, rusqlite with FTS5), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, phase 5 and "Slice 5d decisions (2026-10-24)". Evidence: `docs/research/2026-10-24-codex-rollouts.md` (Codex format, Claude entrypoints) and `docs/research/2026-10-23-claude-transcripts.md`.

## Global constraints

- Marshell never writes, moves or deletes anything under `~/.claude` or `~/.codex`. It only reads (decision 1, 3).
- Tests and live checks never read the real `~/.claude` or `~/.codex`: they use temp folders through `CLAUDE_CONFIG_DIR` and `CODEX_HOME`, and `MARSHELL_HOME` for Marshell.
- No new dependencies (`notify`, `zstd`, `rusqlite` are already in `crates/core/Cargo.toml`).
- New request and response types go in `crates/protocol`, and `pnpm check:generated` passes. Every `u64` gets `#[ts(type = "number")]`.
- New routes sit in the token-guarded router, each with a "refused without the bearer" test. No route takes a path to read or write.
- Never hold a `MutexGuard` across `.await` or file I/O. File reads run in `spawn_blocking` or on the importer's own thread.
- Fixtures are written by hand from the research notes' shapes. Never copy a real transcript or rollout into the repo.
- Copy, exactly:
  - Row marks: "Outside Marshell", "VS Code", "Desktop app", "Headless".
  - Filter: label "Where", options "Any", "Marshell", "Outside Marshell"; checkbox "Show headless runs".
  - Progress line: "Copying conversations from Claude Code and Codex: <done> of <total>."
  - Settings dialog title "History settings"; switch "Copy conversations run outside Marshell"; hint "Marshell reads Claude Code's and Codex's conversation folders and keeps its own copy of each conversation. It never changes their files."
  - Resume confirm: "This conversation changed <n> min ago and may still be open in another terminal. Resume here anyway?" (under a minute: "changed less than a minute ago"), buttons "Resume here" and "Cancel".
- Every UI change works with the keyboard alone and in forced colours.

## Review focus

1. **A file read while its CLI is still writing it.** The last line can be half written. The parser skips a line that isn't valid JSON, and the next size change copies the file again. Test in Task 3.
2. **A Marshell-run Claude session that the importer also sees.** It must keep `client: marshell`, its tab and its recording links, whichever writer comes first or last. Tests in Tasks 1 and 3.
3. **"Delete Marshell's copy" on an imported conversation.** The importer must not bring it back while the source file is unchanged; it may when the file grows. Test in Task 3.
4. **A missing folder** (no Codex installed, or `~/.claude/projects` absent). The importer watches what exists, logs once, and picks the folder up on the next periodic rescan after it appears. Test in Task 3.
5. **A large first backfill** (hundreds of files, hundreds of MB). It copies one file at a time on its own thread, newest first, takes the index lock only per write, stops when the switch goes off, and stops at shutdown. Tests in Task 3 for the switch and the flush; the lock discipline is for the reviewer.

---

### Task 1: Where a conversation ran, in the archive and the index

**Files:**
- Modify: `crates/protocol/src/history.rs` (new `Client`, `Place`; fields on `HistoryEntry`, `HistoryQuery`)
- Modify: `crates/core/src/history/mod.rs` (`SessionMeta.client`), `archive.rs` (`write_meta` keeps a Marshell owner), `index.rs` (column, filters, `SCHEMA_VERSION = 4`), `recorder.rs` (`History::list`/`search` pass the filters)
- Generated: `src/generated/Client.ts`, `Place.ts`, `HistoryEntry.ts`, `HistoryQuery.ts`

**Interfaces:**
- Produces:
  - `marshell_protocol::history::Client` — `#[serde(rename_all = "kebab-case")] enum Client { Marshell, Terminal, Vscode, Desktop, Headless }`, `Default = Marshell`.
  - `Place` — `enum Place { Marshell, Outside }` (kebab-case).
  - `HistoryEntry.client: Client`.
  - `HistoryQuery.place: Option<Place>`, `HistoryQuery.headless: Option<bool>`.
  - `History::search(q: &str, archived: bool, headless: bool)`; `GET /v1/history/search?q=…&archived=…&headless=…`.
  - `SessionMeta.client: Client` with `#[serde(default)]`, so existing sidecars read as `marshell`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests.**
  - `archive::tests::an_old_sidecar_without_a_client_reads_as_marshell`: a sidecar JSON with no `client` key reads back with `Client::Marshell`.
  - `archive::tests::write_meta_keeps_a_marshell_owner`: write a meta with `tab_id: "t1"`, `client: Marshell`; then write the same session with `tab_id: ""`, `client: Terminal`, a later `updated_ms`. The sidecar keeps `tab_id == "t1"` and `client == Marshell`, takes the later `updated_ms` and the earlier `started_ms`.
  - `archive::tests::write_meta_lets_marshell_take_over_an_imported_session`: the reverse order (imported first, then a hook meta with a tab) ends with the tab and `Marshell`.
  - `index::tests::list_hides_headless_runs_unless_asked`: three rows (`Marshell`, `Terminal`, `Headless`). `list(&HistoryQuery::default())` gives 2; with `headless: Some(true)` it gives 3.
  - `index::tests::place_filters_marshell_and_outside`: `place: Some(Marshell)` gives only the `Marshell` row; `Some(Outside)` gives every other client (`Terminal`, `Vscode`, `Desktop`, and `Headless` only when `headless` is on).
  - `index::tests::search_leaves_out_headless_runs`: a turn matching "needle" in a `Headless` row isn't found by `search("needle", false, false)`, and is with `search("needle", false, true)`. The search route's query gains `headless: bool` (default false) and passes it on.
  - `index::tests::an_index_from_schema_3_is_rebuilt_with_clients`: an index file at schema 3 is rebuilt, and rows take `client` from their sidecars.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core history::` — Expected: FAIL (no `client` field).
- [ ] **Step 3: Implement.** `client` column `TEXT NOT NULL DEFAULT 'marshell'`. `SCHEMA_VERSION` 4, so the existing rebuild-from-sidecars path runs. In `write_meta`, when the stored sidecar has a non-empty `tab_id`, keep its `tab_id` and `client`; `started_ms` is the minimum, `updated_ms` the maximum; recording links merge as today. `search` takes the headless rule from the same SQL condition as `list` (one helper). `HistoryEntry.client` is filled from the row.
- [ ] **Step 4: Regenerate and run.** `cargo test -p marshell-protocol` then `cargo test -p marshell-core` — Expected: PASS. `pnpm check:generated` — Expected: exit 0.
- [ ] **Step 5: Commit.** `feat(history): record where each conversation ran, and hide headless runs from the list and search`

### Task 2: The Codex rollout parser

**Files:**
- Create: `crates/core/src/agents/codex/mod.rs`, `crates/core/src/agents/codex/rollout.rs`
- Create: `crates/core/tests/fixtures/codex/` hand-written rollouts: `tui.jsonl`, `exec.jsonl`, `desktop.jsonl`, `subagent.jsonl`, `unknown_version.jsonl`
- Modify: `crates/core/src/agents/mod.rs` (`pub mod codex;`)
- Modify (only if needed to share it): the Claude card summary wording in `crates/core/src/agents/claude/transcript.rs`, moved to a shared helper

**Interfaces:**
- Produces:
  - `agents::codex::rollout::TESTED: (&str, &str) = ("0.162.0", "0.163.0")`, compared on the numeric major.minor.patch with any pre-release suffix ignored (so `0.162.0-alpha.17.2` is tested).
  - `pub struct RolloutMeta { pub id: String, pub started_ms: u64, pub cwd: String, pub branch: Option<String>, pub cli_version: Option<String>, pub client: Client, pub listed: bool }` — from the first `session_meta` line only. `listed` is true for `thread_source` `user` and `chatgpt_handoff`. `client`: `codex_exec` → `Headless`, `codex-tui` → `Terminal`, `Codex Desktop` and `codex_work_desktop` → `Desktop`, anything else → `Terminal`. Never reads `creator_user_id` or `creator_account_id`.
  - `pub fn meta(first_line: &str) -> Option<RolloutMeta>`.
  - `pub fn parse(text: &str, session_id: &str) -> Conversation` — never fails, skips lines that aren't JSON.
- Consumes: `Client` (Task 1); `Conversation`, `Turn`, `TurnItem`, `ToolCard`, `EditedFile` (protocol).

Mapping (from `docs/research/2026-10-24-codex-rollouts.md`; only `event_msg` lines are read):
- A turn runs from `task_started` to its `task_complete` (same `turn_id`). Its prompt is the first `item_completed` `UserMessage` in it: the `text` blocks joined, with a `<send_user_message_question_reply>…</…>` wrapper stripped to its inner text and an `<in-app-browser-context>` block removed. A turn with no `UserMessage` and no items is dropped. An open turn at the end of the file is kept.
- `AgentMessage` → `Text` (the `Text` blocks joined).
- `CommandExecution` → card `name: "Shell"`, summary `Ran <first line of the script>`, detail `aggregated_output` (4,000-character cap, as Claude cards), `error` when `status == "failed"` or `exit_code != 0`.
- `FileChange` → one card per entry of `changes`, `name: "apply_patch"`, with `file` set: `add` → `created: true`, `added` = content lines, detail = content; `update` → `added`/`removed` from the `+`/`-` lines of `unified_diff`, detail = the diff with its `@@` lines removed; `delete` → `removed` = content lines, detail = the content lines each prefixed `-`. Paths are made relative to `cwd` when inside it. The summary uses the same wording as Claude's Edit cards.
- `McpToolCall` → card `name: "<server>.<tool>"`, `error` when `status == "failed"` or `result.isError`.
- `Extension` with `kind: "web.search"` → card `name: "Web search"`, summary the query.
- `ContextCompaction` → `Compacted`.
- A `task_complete` with `error` → a `Text` item holding `error.message`.
- `Reasoning`, `SubAgentActivity`, `CollabAgentToolCall`, `ImageView`, `token_count`, `world_state` and every `response_item` are ignored.
- `limited` when `cli_version` is missing or outside `TESTED`. Title, cwd, branch, model and version are filled the way the Claude parser fills them (model from `turn_context.model`).

- [ ] **Step 1: Write the fixtures and failing tests** (`rollout::tests`):
  - `reads_the_meta_of_a_tui_session` (`tui.jsonl`): id, cwd, branch, `client == Terminal`, `listed`.
  - `an_exec_run_is_headless` and `a_desktop_thread_is_desktop_app`.
  - `a_subagent_thread_is_not_listed`.
  - `turns_take_their_prompt_from_the_user_message`: two turns, prompts as written, `final_answer` text last in each turn.
  - `a_question_reply_shows_its_inner_text`.
  - `file_changes_become_edit_cards` : an `add`, an `update` with two hunks, a `delete`; counts, `created`, relative paths, and `filesChanged`-ready `file` fields.
  - `a_failed_command_is_an_error_card` and `a_failed_turn_shows_its_message`.
  - `a_half_written_last_line_is_skipped`.
  - `an_unknown_version_is_limited` (`unknown_version.jsonl`, `cli_version: "0.170.0"`) and `an_alpha_of_a_tested_version_is_not_limited`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core agents::codex` — Expected: FAIL (module missing).
- [ ] **Step 3: Implement** `meta` and `parse` in `rollout.rs`.
- [ ] **Step 4: Run them.** Same command — Expected: PASS.
- [ ] **Step 5: Commit.** `feat(history): parse Codex CLI rollouts into conversations`

### Task 3: The importer

**Files:**
- Create: `crates/core/src/history/import.rs`
- Modify: `crates/core/src/history/archive.rs` (`copy_from`), `crates/core/src/history/recorder.rs` (hand the import to `schedule_copy` and `reindex` by agent), `crates/core/src/history/mod.rs`
- Modify: `crates/core/src/settings.rs` and `crates/protocol/src/api.rs` (`Settings.history_import: bool`, default on)
- Modify: `crates/core/src/server/mod.rs` (start the importer after the backfill; stop it in the shutdown flush), `crates/core/src/server/history.rs` (`GET /v1/history/import`)
- Modify: `crates/protocol/src/history.rs` (`ImportStatus`)

**Interfaces:**
- Produces:
  - `pub struct ImportRoots { pub claude: Option<PathBuf>, pub codex: Option<PathBuf> }` and `pub fn import_roots(env: impl Fn(&str) -> Option<String>, user_home: &Path) -> ImportRoots`: Claude is `$CLAUDE_CONFIG_DIR/projects`, else `<home>/.claude/projects`; Codex is `$CODEX_HOME/sessions`, else `<home>/.codex/sessions`.
  - `pub struct Importer` with `pub fn start(history: Arc<History>, roots: ImportRoots, on: bool) -> Arc<Importer>`, `pub fn set_on(&self, on: bool)`, `pub fn status(&self) -> ImportStatus`, `pub fn stop(&self)`.
  - `marshell_protocol::history::ImportStatus { pub on: bool, pub copying: bool, pub done: u32, pub total: u32 }`.
  - `archive::copy_from(home: &Path, meta: &SessionMeta, source: &Path) -> anyhow::Result<PathBuf>`: reads `source` (a `.jsonl.zst` source is decoded first, with the archive's 200 MB cap) and writes the archive copy and sidecar as `copy_transcript` does.
  - `Settings.history_import: bool` (`FileDoc` stores `Option<bool>`, `None` means on).
- Consumes: `Client` (Task 1); `rollout::meta`, `rollout::parse` (Task 2); `History::reindex` picks the parser by `meta.agent`.

Behaviour:
- The candidates are `<claude root>/*/<uuid>.jsonl` (main transcripts only, no subfolders) and `<codex root>/YYYY/MM/DD/rollout-*-<uuid>.jsonl` or `.jsonl.zst`. Symlinks and junctions are not followed.
- State lives in `<home>/history/import.json`: for each source path, the size last copied and the `(agent, session_id)`. It is written atomically.
- A file is copied when its size differs from the state's and has stayed the same for 3 s. The size, not the modification time, is the signal (the Codex note shows mtimes lag). `notify` events and a full rescan every 60 s both feed the check.
- The first run (no state file) queues every candidate, newest first by its first line's time, and `status` reports `done` of `total`.
- Claude client: from the first line that has an `entrypoint`: `cli` → `Terminal`, `claude-vscode` → `Vscode`, `claude-desktop` → `Desktop`, `sdk-*` → `Headless`, otherwise `Terminal`. Codex: `RolloutMeta.client`, and a file with `listed == false` is recorded in the state and never copied.
- The importer upserts the index row before it calls `schedule_copy`, so the existing "is it still in the index" check passes. A session that "Delete Marshell's copy" removed keeps its state entry, so an unchanged file isn't copied again. A file that grows afterwards is.
- `set_on(false)` stops the queue after the current file; `set_on(true)` rescans. `stop()` is called from the server's shutdown flush.

- [ ] **Step 1: Write the failing tests** (`import::tests`, all on temp roots and a temp `MARSHELL_HOME`):
  - `roots_follow_claude_config_dir_and_codex_home`.
  - `the_first_run_copies_every_transcript_newest_first`: two Claude files and one Codex `.jsonl.zst`. After the run, three archive copies, `status` gives `done == total == 3`, and the order of copies follows the first-line times.
  - `a_file_is_copied_again_when_it_grows_and_its_size_settles`.
  - `a_half_written_line_is_copied_and_completed_later` (Review focus 1).
  - `headless_and_vscode_runs_get_their_client` (`sdk-cli`, `claude-vscode`, `cli`).
  - `a_codex_subagent_thread_is_never_copied`.
  - `a_marshell_session_seen_by_the_importer_stays_marshell` (Review focus 2): a sidecar with a tab and a recording link exists; importing the same transcript keeps both and `client == Marshell`.
  - `a_deleted_copy_stays_deleted_until_its_file_grows` (Review focus 3).
  - `a_folder_that_appears_later_is_picked_up_by_the_rescan` (Review focus 4; the rescan interval is a parameter so the test can shorten it).
  - `turning_the_switch_off_stops_the_queue` (Review focus 5).
  - `nothing_under_the_roots_is_written`: every source file's bytes and modification time are unchanged after a full run.
  - In `server/history.rs` tests: `import_status_route_reports_progress` and `import_status_route_refuses_without_the_bearer`.
  - In `settings.rs` tests: `history_import_defaults_to_on`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core import` — Expected: FAIL (module missing).
- [ ] **Step 3: Implement** `import.rs`, `copy_from`, the settings field, the route and the server wiring. Changing the setting through the existing settings route calls `set_on`.
- [ ] **Step 4: Run them.** `cargo test -p marshell-core` and `cargo test -p marshell-protocol` — Expected: PASS. `pnpm check:generated` — Expected: exit 0.
- [ ] **Step 5: Commit.** `feat(history): copy Claude Code and Codex conversations run outside Marshell into the archive`

### Task 4: Codex in the routes, and Resume

**Files:**
- Modify: `crates/core/src/server/history.rs` (`check` accepts `claude` and `codex`; `load_conversation`, export, recordings and the actions use the URL's agent instead of `AGENT`; Fork refuses Codex)
- Modify: `crates/core/src/profiles/detect.rs` (`("codex", ResumeTarget::Session(id)) => vec!["resume", id]`)
- Modify: `crates/core/src/history/recordings.rs` only if a conversation with an empty `tab_id` can match recordings

**Interfaces:**
- Produces: every `/v1/history/{agent}/{id}…` route works for `codex`. `POST …/fork` for `codex` answers 409 "Fork works only for Claude Code conversations."
- Consumes: Task 1–3.

- [ ] **Step 1: Write the failing tests.**
  - `history_api` tests: `a_codex_conversation_opens_from_the_archive`, `search_finds_a_codex_turn`, `export_works_for_a_codex_conversation` (markdown), `an_unknown_agent_is_not_found` (`/v1/history/gemini/<id>` → 404), `fork_refuses_a_codex_conversation`.
  - `profiles::detect` test: `codex_resumes_a_session_by_id` gives `["resume", "<id>"]`.
  - `recordings` test: `a_conversation_with_no_tab_has_no_parts` (empty `tab_id`, a recordings folder with other tabs' files).
  - The resume route test: `resume_opens_codex_in_the_conversations_folder` (the created session's command line and cwd).
- [ ] **Step 2: Run them.** `cargo test -p marshell-core history` and `cargo test -p marshell-core --test history_api` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** Same — Expected: PASS.
- [ ] **Step 5: Commit.** `feat(history): open, search, export and resume Codex conversations`

### Task 5: The list: where it ran, headless runs, progress

**Files:**
- Modify: `src/features/history/HistoryView.tsx` (`HistoryFilters.place`, `.headless`; the Where select and the checkbox), `historyState.ts` (`listQuery`, `filterOptions`), `LiveHistory.tsx` (defaults; poll `GET /v1/history/import` every second while History is open and `copying`), the list row component (the mark)
- Modify: `src/lib/api/history.ts` (`importStatus(ep)`)
- Modify: the agent label map, so `codex` reads "Codex CLI" in the Agent filter and the row
- Tests: the matching `*.test.ts(x)` files

**Interfaces:**
- Consumes: `Client`, `Place`, `HistoryQuery.place`/`.headless`, `ImportStatus` (Tasks 1, 3).
- Produces: `HistoryFilters = { …existing, place: "any" | "marshell" | "outside"; headless: boolean }`, default `{ place: "any", headless: false }`. `clientMark(c: Client): string | null` in `historyState.ts`: `marshell` → null, `terminal` → "Outside Marshell", `vscode` → "VS Code", `desktop` → "Desktop app", `headless` → "Headless".

- [ ] **Step 1: Write the failing tests.**
  - `historyState.test.ts`: `listQuery` maps `place: "outside"` to `place: "outside"` and leaves it out for `"any"`; `headless: true` maps to `headless: true`, false leaves it out. The search request sends `headless=true` only when the checkbox is on. `clientMark` for each client.
  - The row's markup test: a `terminal` row shows "Outside Marshell"; a `marshell` row shows no mark.
  - `HistoryView.test.tsx`: the filters show a "Where" select with "Any", "Marshell", "Outside Marshell" and a "Show headless runs" checkbox.
  - `LiveHistory.test.tsx` (or the extracted helper): `importLine({copying: true, done: 120, total: 355, on: true})` is "Copying conversations from Claude Code and Codex: 120 of 355."; not copying gives null.
- [ ] **Step 2: Run them.** `pnpm test src/features/history` — Expected: FAIL.
- [ ] **Step 3: Implement.** The mark is text beside the row's meta, styled like the other row marks (read the good-css skill first). The progress line goes through `Announcer` only when copying starts and ends, not on every count.
- [ ] **Step 4: Run them.** Same — Expected: PASS. `pnpm build` — Expected: ok.
- [ ] **Step 5: Commit.** `feat(history): mark where each conversation ran, filter by it, and show the import's progress`

### Task 6: The conversation view and the settings dialog

**Files:**
- Modify: `src/features/history/ConversationView.tsx` (Resume confirm; Fork hidden for `codex`), `LiveHistory.tsx` (passes `client` and `updated_ms`)
- Create: `src/features/history/HistorySettingsDialog.tsx` (+ test), opened from the palette command "History settings…" (`src/lib/keymap.ts`, group "Tools") and from a "History settings" button in the History header
- Modify: `src/lib/api/settings.ts` (or where settings are saved) for `history_import`

**Interfaces:**
- Consumes: `HistoryEntry.client`, `Settings.history_import` (Tasks 1, 3); the resume route (Task 4).
- Produces: `resumeWarning(client: Client, updatedMs: number, now: number): string | null` in `historyState.ts` — null for `marshell`, or when `now - updatedMs >= 10 min`; otherwise the confirm copy with whole minutes (`Math.floor`), or "less than a minute ago" under one minute.

- [ ] **Step 1: Write the failing tests.**
  - `historyState.test.ts`: `resumeWarning` for a Marshell row (null), an outside row changed 3 min ago (the exact copy with "3 min ago"), 30 s ago ("less than a minute ago"), 10 min ago (null).
  - `ConversationView.test.tsx`: with a pending warning the inline confirm shows the copy, "Resume here" and "Cancel"; a `codex` conversation has no Fork button.
  - `HistorySettingsDialog.test.tsx`: the title, the switch label, the hint, and the switch's checked state from the setting.
  - `keymap.test.ts`: the "History settings…" command sits in Tools.
- [ ] **Step 2: Run them.** `pnpm test src/features/history src/lib` — Expected: FAIL.
- [ ] **Step 3: Implement.** The confirm is inline under the Resume button, as Mode ▸'s restart bar is; Resume here sends the existing resume request, Cancel and Esc return focus to Resume. The dialog follows the Notifications dialog's pattern (focus trap, Esc closes, focus returns to the opener).
- [ ] **Step 4: Run them.** `pnpm test` — Expected: PASS. `pnpm build` — Expected: ok.
- [ ] **Step 5: Commit.** `feat(history): warn before resuming a conversation that may be open elsewhere, and add History settings`

### Task 7: Live check

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1` (`start` also creates `<scratch>\cx` and sets `CODEX_HOME` to it, so the app never reads the real `~/.codex`)
- Create: `docs/research/2026-10-24-outside-and-codex-live-check.md`
- Modify: `docs/PLAN.md` (the phase 5 line links the note)

- [ ] **Step 1:** Change `live-check.ps1` and check that `start` prints `CODEX_HOME=<scratch>\cx`.
- [ ] **Step 2:** Seed the scratch `cc\projects\<folder>\` with hand-written Claude transcripts (`cli`, `claude-vscode`, `sdk-cli`) and `cx\sessions\2026\10\24\` with hand-written rollouts (`codex-tui`, `codex_exec`, a sub-agent, one `.jsonl.zst`), then start the app.
- [ ] **Step 3:** Check, over CDP: the progress line during the first run; the rows and their marks; Where and Show headless runs; a Codex conversation's view, search hit and Files changed; Resume on a just-changed outside conversation shows the confirm, and Resume here opens a tab with `claude --resume <id>` in its folder (the session is one this check starts; never press Enter in it); appending a turn to a transcript updates its row; "Delete Marshell's copy" stays deleted; the History settings switch off stops the import; keyboard only and forced colours (emulated).
- [ ] **Step 4:** Stop by PID after reading the tree; check `~/.claude/settings.json`'s modified time is unchanged; write the note in the live-check skill's format.
- [ ] **Step 5: Commit.** `docs(history): record the slice 5d live check`
