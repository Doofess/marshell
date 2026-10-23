# History view (slice 5a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Who implements:**
> - Tasks 1 to 6 go to `worker` (Sonnet), one at a time, on `phase-5a`, with `effort: high` for Tasks 1, 2 and 3 (a parser, the index, and the API's checks).
> - The main thread reads each diff, runs the task's tests, and ticks the task.
> - Task 7, the live check, stays in the main thread.

**Goal:** Ctrl+Shift+Y opens a History view in the main area. It lists the Claude Code sessions Marshell has archived, grouped by project, newest first, with filters and full-text search, and opens each one as a readable conversation with Resume.

**Architecture:**
- The Claude adapter gets a transcript parser. It turns an archived JSONL transcript into a common `Conversation` (protocol types, generated for TypeScript). Unknown versions parse best effort and are flagged `limited`.
- The history index goes to schema 2:
  - The sessions table gains title, branch, model, version, turn count and the indexed length.
  - A new FTS5 table holds one row per turn.
  - A session is re-indexed after each archive copy. A backfill at startup indexes any archive whose indexed length is out of date.
- Four token-guarded routes serve the view: list, search, conversation, and resume.
- The frontend adds `src/features/history/`. `MainWindow` shows it in place of the panes while it is open, and the two existing commands `history` and `global-search` open it.

**Tech Stack:** Rust (axum, rusqlite 0.40 `bundled` with FTS5, zstd 0.13, serde_json, ts-rs), React + TypeScript, Vitest (node, `renderToStaticMarkup`), markdown-it and DOMPurify (both already in the tree).

**Spec:**
- `docs/PLAN.md` §9 phase 5, "Slice 5a decisions (2026-10-23)" 1 to 4, and the phase 5 bullets ("Parsing", "Resume").
- Brief §5.10 (History view, Conversation view, Titles).
- `docs/research/2026-10-23-claude-transcripts.md` for the format.
- PLAN "Secrets at rest": transcripts are stored owner-only, not encrypted.

## Global Constraints

- **No new dependency.** Everything needed is in the tree.
- **Security surface.**
  - The CSP, capabilities and `guard.rs` don't change.
  - The new routes sit in the token-guarded router beside `/v1/agents`.
  - A session id from a URL passes `history::valid_session_id` and the agent must be `claude` before any filesystem call. Otherwise the route answers 404 "No such conversation."
- **Read-only.** Marshell never writes to or deletes a CLI's files. The parser reads only Marshell's archive copy (`archive_paths(home, "claude", id).0`), never `~/.claude`.
- **Transcript text is untrusted.**
  - Assistant text renders through `safeMarkdownHtml` (DOMPurify).
  - Prompts, tool summaries and details render as text, never as HTML.
  - Search snippets mark hits with U+0002 and U+0003, which the view splits into `<mark>`. No HTML comes from the core.
- **Size caps.**
  - Each text item is cut at 100,000 characters, each tool detail at 4,000, and each prompt at 20,000, with the suffix `…`.
  - A transcript over 200 MB uncompressed isn't parsed. It lists with its title from the sidecar and `limited: true`.
- **Tested versions:** `2.1.278` to `2.1.309` inclusive (the survey). Any other or missing `version` sets `limited`.
- **Exact copy:**
  - View heading "History"; search box label and placeholder "Search conversations".
  - Empty list: "No conversations yet. Claude Code sessions you run in Marshell show up here when their first turn ends."
  - No hits: `Nothing matches “{query}”.`
  - Limited: `Status limited: Claude Code {version} saved this conversation, and Marshell hasn't been tested with that version. Some parts may be missing.` With no version: `Status limited: this conversation has no Claude Code version, so some parts may be missing.`
  - No archive: "Marshell has no copy of this conversation yet."
  - A session with no title yet (not parsed, or parsing failed) lists as "Untitled conversation".
  - Compaction: "Conversation compacted".
  - Subagent heading `Subagent: {description}`.
  - Buttons "Resume" and "Go to session". Filter labels "Agent", "Project", "Branch", "Model", "When". "When" offers "Any time", "Today", "Last 7 days", "Last 30 days".
  - List row meta: `{project} · {branch} · {relative time} · {n} turns`. A missing branch is left out, and one turn reads "1 turn".
- **Tool summaries** (Task 1), with paths shown relative to the session's `cwd` when inside it:
  - `Edit`: `Edited {path}, {a} added, {r} removed`.
  - `Write`: `Created {path}, {n} lines` when `toolUseResult.type` is `create`, else `Edited {path}, {a} added, {r} removed`.
  - `Bash`: `Ran {first line of command, cut at 80}`, plus `, failed` on `is_error` or `interrupted`.
  - `Read`: `Read {path}`. `Grep`: `Searched for {pattern}`. `Glob`: `Listed {pattern}`.
  - `WebFetch`: `Fetched {url}`. `WebSearch`: `Searched the web for {query}`.
  - Any other tool: `Used {name}`. `Agent` and `Task` become a subagent item instead.
- **Keyboard and forced colours.** Every new control works with the keyboard alone and in forced colours (AGENTS.md).
- **Commits.** Run `cargo fmt --all` before each Rust commit. Messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Regenerated `src/generated/` files are committed with the change that makes them.

## Review Focus

1. **Hostile transcript content.**
   - The input: assistant text with `<script>`, `<img onerror>`, `javascript:` links, or a 5 MB single line, and a prompt holding HTML.
   - Expected: nothing executes or loads. Prompts show as literal text, and caps apply.
   - Tests: Task 1 tests the caps. Task 5 tests that a prompt `<b>x</b>` renders escaped and that the markdown path goes through `safeMarkdownHtml`.
2. **Search syntax typed by a person.**
   - The input: `"`, `foo"bar`, `AND`, `NEAR(`, `-x`, `col:val`, `*`, a bare punctuation string, an empty or whitespace query, and Hebrew or emoji.
   - Expected: no FTS5 error ever reaches the person. Empty gives no hits, and terms match as plain words.
   - Tests: Task 2's `fts_query` table test and a search over each input.
3. **A transcript the parser doesn't know.**
   - The input: a newer `version`, a line type or block type never seen, a malformed line, and a `tool_result` with no matching `tool_use`.
   - Expected: parsing never fails. Unknown things are skipped and the conversation is `limited` when the version is untested.
   - Tests: Task 1.
4. **Hostile ids in the URL.**
   - The input: `/v1/history/claude/..%2F..%2Fx`, a 65-character id, `/v1/history/codex/<valid id>`, and `/v1/history/claude/<id>/resume` for an id the index doesn't know.
   - Expected: 404 before any file is opened.
   - Tests: Task 3.
5. **An existing schema-1 index.**
   - The input: a person upgrading with an index from 3e1.
   - Expected: it's replaced by a schema-2 index, the sessions come back from the sidecars, and the backfill fills search without blocking startup.
   - Tests: Task 2.

---

### Task 1: Transcript parser and conversation types

**Files:**
- Create: `crates/protocol/src/history.rs` (and `pub mod history;` in `crates/protocol/src/lib.rs`)
- Create: `crates/core/src/agents/claude/transcript.rs` (and `pub mod transcript;` in `agents/claude/mod.rs`)
- Create: `crates/core/tests/fixtures/claude/transcripts/2.1.309/` with `basic.jsonl`, `tools.jsonl`, `subagent-compact.jsonl`, `commands.jsonl`, and `crates/core/tests/fixtures/claude/transcripts/9.9.9/future.jsonl`
- Test: unit tests in `transcript.rs`, plus `crates/core/tests/fixtures_scrubbed.rs` (it must still pass over the new files)

**Interfaces:**
- Produces (protocol, `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)] #[ts(export)]`, `Option` fields `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]` as in `agents.rs`):
  - `Conversation { agent: String, session_id: String, title: String, cwd: String, branch: Option<String>, model: Option<String>, version: Option<String>, limited: bool, turns: Vec<Turn> }`
  - `Turn { index: u32, at_ms: u64, prompt: String, items: Vec<TurnItem> }`
  - `#[serde(tag = "kind", rename_all = "snake_case")] enum TurnItem { Text { markdown: String }, Tool { card: ToolCard }, Subagent { description: String, prompt: String, result: Option<String> }, Compacted }`
  - `ToolCard { name: String, summary: String, detail: Option<String>, error: bool }`
  - Task 2 adds to the same file: `HistoryEntry`, `SearchHit`, `HistoryQuery`.
- Produces (core): `pub fn parse(text: &str, session_id: &str) -> Conversation` and `pub const TESTED: (&str, &str) = ("2.1.278", "2.1.309")` in `agents::claude::transcript`.
  - `agent` is `"claude"`, and `session_id` is the argument.
  - `cwd` comes from the last line that carries one.

- [ ] **Step 1: Write the fixtures.**
  - Hand-written JSONL with made-up prompts and paths under `C:/work/demo`, following the research note's shapes exactly.
  - Include the noise lines: `attachment`, `api-request`, `ai-title`, `mode`, `last-prompt`.
  - What each file holds:
    - `basic`: two prompts; one assistant reply split over two text lines with the same `message.id`; a `thinking` line; an `ai-title`, then a later `custom-title`.
    - `tools`: `Edit` with a `structuredPatch` of 2 added and 1 removed lines; `Write` with type `create` of 3 lines; a `Bash` that fails (`is_error: true`); `Read`; `Grep`; an unknown tool `Frobnicate`; a `tool_result` whose `tool_use_id` matches nothing.
    - `subagent-compact`: an `Agent` call with its result; a `compact_boundary`; an `isCompactSummary` user line; `isSidechain` lines that must be ignored.
    - `commands`: user strings starting with `<command-name>/model</command-name><command-args>opus</command-args>`, `<bash-input>git status</bash-input>`, `<local-command-stdout>`, `<task-notification>`, `<system-reminder>`; an `isMeta` user line; one malformed line `{not json`; a prompt given as `text` plus `image` blocks.
    - `future`: version `9.9.9`, an unknown line type, and an assistant block of unknown type.

- [ ] **Step 2: Write the failing tests** in `transcript.rs`, with names and assertions:
  - `basic_turns_titles_and_merged_text`:
    - 2 turns, with prompts as written;
    - turn 0 has one `Text` joining the two lines with `"\n\n"`;
    - no thinking text anywhere;
    - `title` is the `customTitle`, `model` and `branch` come from the last assistant line, and `limited == false`.
  - `tools_become_cards` checks each card exactly:
    - `"Edited src/app.ts, 2 added, 1 removed"`;
    - `"Created notes.md, 3 lines"`;
    - `"Ran cargo test, failed"` with `error: true`;
    - `"Read src/app.ts"`, `"Searched for TODO"`, `"Used Frobnicate"`.
    - The orphan `tool_result` adds nothing.
  - `subagent_and_compaction`:
    - `Subagent { description, prompt, result: Some(..) }`, then `Compacted`, in order;
    - the compact summary isn't a turn, and no sidechain text appears.
  - `commands_and_wrappers`:
    - the turns' prompts are exactly `["/model opus", "! git status", "look at this [image]"]`;
    - skipped wrappers make no turn, and the malformed line is skipped.
  - `untested_version_is_limited`: `future` parses with `limited == true`, and its known items survive.
  - `caps_apply`:
    - a 150,000-character text becomes 100,000 characters plus `…`;
    - a tool detail is at most 4,001 characters;
    - a prompt is at most 20,001 characters.
  - `title_falls_back_to_first_prompt`: with no title lines, the title is the first prompt's first line, cut at 80 characters plus `…`.
  - `bash_detail_holds_command_and_output`:
    - the detail is the command, a blank line, then stdout and stderr;
    - an `Edit` detail is the patch's lines, each starting with ` `, `+` or `-`.

- [ ] **Step 3: Run the tests to check they fail.** `cargo test -p marshell-core transcript`. Expected: compile errors or failures for the missing items.

- [ ] **Step 4: Implement `parse`.**
  - Read line by line with `serde_json::from_str::<Value>`, skipping lines that don't parse and lines with `isSidechain`.
  - A turn starts at each user line that is a prompt.
  - Assistant blocks of one `message.id` merge into one `Text`, deduped by `uuid`.
  - `tool_use` makes a pending card keyed by id, and the matching `tool_result` and `toolUseResult` complete it.
  - Lines before the first prompt are dropped.
  - `at_ms` comes from the prompt's `timestamp`, read with `chrono` if it's already a dependency, otherwise with a small RFC 3339 reader.
  - Version compare is numeric per dot part.
  - Generate the TS types with `cargo test -p marshell-protocol`.

- [ ] **Step 5: Run the tests to check they pass.**
  - `cargo test -p marshell-core transcript`, then `cargo test -p marshell-core --test fixtures_scrubbed`. Expected: all pass.
  - `pnpm check:generated`. Expected: ok once the new `src/generated/*.ts` files are added.

- [ ] **Step 6: Commit.** `feat(history): parse Claude Code transcripts into conversations, flagging untested versions`

### Task 2: Index schema 2 with full-text search

**Files:**
- Modify: `crates/core/src/history/index.rs` (schema, `open`/`check`, new queries)
- Modify: `crates/core/src/history/recorder.rs` (`schedule_copy` re-indexes after a copy; `History::backfill`)
- Modify: `crates/core/src/history/mod.rs`, `crates/core/src/history/archive.rs` (`read_archive`)
- Modify: `crates/protocol/src/history.rs` (types below)
- Modify: the place that builds `History` at startup (`crates/core/src/server/mod.rs` ~182) to spawn the backfill
- Test: unit tests in `index.rs` and `recorder.rs`

**Interfaces:**
- Consumes: `transcript::parse`, `Conversation`.
- Produces (protocol):
  - `HistoryEntry { agent, session_id, tab_id, cwd, title, branch: Option<String>, model: Option<String>, version: Option<String>, limited: bool, turns: u32, started_ms: u64, updated_ms: u64 }`;
  - `SearchHit { agent, session_id, title, cwd, turn: u32, at_ms: u64, snippet: String }`;
  - `HistoryQuery { agent: Option<String>, project: Option<String>, branch: Option<String>, model: Option<String>, since_ms: Option<u64> }` with `Deserialize` for axum `Query`.
- Produces (core), on `Index`:
  - `index_conversation(&self, meta: &SessionMeta, c: &Conversation, indexed_len: u64) -> Result<()>`, which replaces that session's FTS rows and fields in one transaction;
  - `list(&self, q: &HistoryQuery) -> Result<Vec<HistoryEntry>>`, newest `updated_ms` first, limit 2000;
  - `search(&self, query: &str) -> Result<Vec<SearchHit>>`, limit 50, by FTS5 rank;
  - `stale(&self) -> Result<Vec<SessionMeta>>`, the sessions whose archive length differs from `indexed_len`.
- Produces (core), elsewhere:
  - `pub fn fts_query(input: &str) -> Option<String>`;
  - on `History`: `list`, `search`, `reindex(&self, meta: &SessionMeta)`, and `backfill(self: Arc<Self>)`, which runs on a blocking thread;
  - `pub fn read_archive(home: &Path, agent: &str, session_id: &str) -> anyhow::Result<Option<String>>` in `archive.rs`, giving `None` when there's no archive and an error past 200 MB decoded.

- [ ] **Step 1: Write the failing tests.**
  - `fts_query_quotes_every_term`, a table test:
    - `"foo bar"` gives `"\"foo\" \"bar\"*"`;
    - `"say \"hi\""` gives `"\"say\" \"\"\"hi\"\"\"*"`;
    - `"AND"` gives `"\"AND\"*"`; `"col:val"` gives `"\"col:val\"*"`;
    - `""`, `"   "` and `"*"` give `None`; `"עברית 🙂"` gives `"\"עברית\" \"🙂\"*"`.
    - Rule: split on whitespace, drop tokens without a letter, digit or symbol other than `*`, double each `"`, quote each token, and `*` after the last.
  - `search_finds_prompt_reply_tool_and_path_with_marked_snippet`: index the `tools` fixture.
    - Searching a word only in a reply, a tool summary and a path each give one hit with the right `turn`.
    - The snippet contains `\u{2}word\u{3}`.
  - `search_survives_hostile_queries`: each Review Focus 2 input returns `Ok`, and the empty ones return no hits.
  - `list_filters_and_orders`: three sessions with different cwd, branch, model and times.
    - Each filter narrows the list as expected.
    - The order is newest first, and `since_ms` excludes older ones.
  - `a_schema_1_index_is_rebuilt`:
    - Create a schema-1 file with the 3e1 `CREATE TABLE` and `user_version = 1`, plus two sidecars.
    - `Index::open` gives `Opened::Upgraded { sessions: 2 }`, the old file is moved to `index.sqlite.v1-<ms>`, and `stale()` lists both.
  - `reindex_after_copy_makes_it_searchable`: through `schedule_copy` with a short settle, the copied fixture's prompt word is found.
  - `#[ignore] search_1000_sessions_under_100ms`:
    - 1000 sessions of 20 synthetic turns, about 300 words each;
    - the median of 20 searches for a two-word query is under 100 ms;
    - run with `--release` in Task 7.

- [ ] **Step 2: Run the tests to check they fail.** `cargo test -p marshell-core history::`. Expected: they fail.

- [ ] **Step 3: Implement.**
  - Set `SCHEMA_VERSION` to 2.
  - Add columns to `sessions`: `title TEXT NOT NULL DEFAULT ''`, `branch TEXT`, `model TEXT`, `version TEXT`, `limited INTEGER NOT NULL DEFAULT 0`, `turns INTEGER NOT NULL DEFAULT 0` and `indexed_len INTEGER`.
  - Add `CREATE VIRTUAL TABLE turns_fts USING fts5(prompt, reply, tools, paths, agent UNINDEXED, session_id UNINDEXED, turn UNINDEXED, at_ms UNINDEXED, tokenize = 'unicode61 remove_diacritics 2')`.
  - Build the snippet with `snippet(turns_fts, -1, char(2), char(3), '…', 12)`.
  - Fill the FTS columns: `reply` joins the turn's `Text` and subagent results; `tools` joins the card summaries and subagent descriptions; `paths` holds the paths named in the cards.
  - `check` gains `Health::Older`, which moves the file aside as `index.sqlite.v{n}-<ms>` and rebuilds as `Opened::Upgraded`. `Broken` stays for damage.
  - A session whose parse fails, or whose archive is too big, keeps its row with `limited = 1` and no FTS rows.
  - Run the backfill once at startup on a blocking thread, after the server is listening.

- [ ] **Step 4: Run the tests to check they pass.** `cargo test -p marshell-core history::` and `cargo test -p marshell-core --test history`. Expected: pass, and the ignored test is skipped.

- [ ] **Step 5: Commit.** `feat(history): index each archived conversation for full-text search, rebuilding a schema-1 index`

### Task 3: History routes

**Files:**
- Create: `crates/core/src/server/history.rs`
- Modify: `crates/core/src/server/mod.rs` (routes beside `/v1/agents`)
- Modify: `crates/core/src/server/sessions.rs`, to pull a `start_session(...)` helper out of `create` that the resume route can call, keeping `create` behaviour identical
- Test: `crates/core/tests/history_api.rs` (follow `agents_api.rs` for setting up the server)

**Interfaces:**
- Consumes: `History::{list, search}`, `archive::read_archive`, `transcript::parse`, `resume_args`/`ResumeTarget::Session`, `transcript_allowed`, `NO_CONVERSATION`.
- Produces the routes:
  - `GET /v1/history?agent&project&branch&model&since_ms` returns `Vec<HistoryEntry>`.
  - `GET /v1/history/search?q=` returns `Vec<SearchHit>`.
  - `GET /v1/history/{agent}/{session_id}` returns `Conversation`.
  - `POST /v1/history/{agent}/{session_id}/resume`, with the body `ResumeRequest { cols, rows }`, returns `CreateSessionResponse`.
- Error answers:
  - An unknown agent, an invalid id, or an id the index doesn't have: 404 "No such conversation."
  - No archive: 404 "Marshell has no copy of this conversation yet."
  - A resume whose transcript file is gone: 409 with `ResumeError` and `NO_CONVERSATION`, the shape the tab resume uses.
  - With history off for this run: 503 "History is off for this run."

- [ ] **Step 1: Write the failing tests** in `history_api.rs`:
  - `list_and_search_after_a_copy`: seed one fixture through the recorder; the list has it with its title, and a search finds its prompt word.
  - `conversation_returns_turns`: the `basic` fixture gives 2 turns.
  - `bad_ids_are_404_without_touching_files`, which checks each URL in Review Focus 4.
  - `resume_starts_claude_with_the_session_in_its_folder`:
    - with a fake `claude` found through `with_agent_path` as the other tests do, the new session's argv ends with `--resume <id>` and its cwd is the sidecar's;
    - with the transcript file deleted, the answer is 409 with the `NO_CONVERSATION` text.
  - `routes_need_the_token`: a request without the bearer is refused, the way other routes are (look for an existing unauthenticated test in `crates/core/tests/server.rs` and copy its pattern).

- [ ] **Step 2: Run the tests to check they fail.** `cargo test -p marshell-core --test history_api`. Expected: they fail.

- [ ] **Step 3: Implement the routes.** Parsing and archive reads run in `spawn_blocking`. The resume starts with the default profile, `agent: "claude"`, the session's `cwd`, and the default mode.

- [ ] **Step 4: Run the tests to check they pass.** `cargo test -p marshell-core --test history_api`, then `cargo test -p marshell-core`. Expected: pass.

- [ ] **Step 5: Commit.** `feat(history): serve the history list, search, conversations and resume`

### Task 4: Frontend data and state

**Files:**
- Create: `src/lib/api/history.ts`, `src/features/history/historyState.ts`, `src/features/history/snippet.ts`
- Test: `src/lib/api/history.test.ts`, `src/features/history/historyState.test.ts`, `src/features/history/snippet.test.ts`

**Interfaces:**
- Consumes: the generated `HistoryEntry`, `SearchHit`, `Conversation`, `HistoryQuery` and `CreateSessionResponse`.
- Produces, in `history.ts`:
  - `history.list(ep, q: HistoryQuery)`, `history.search(ep, q: string)`, `history.get(ep, agent, id)`;
  - `history.resume(ep, agent, id, size)`, returning `Started` as `sessions.resume` does: a 409 comes back as `{ ok: false, reason }`.
- Produces, in `historyState.ts`:
  - `groupByProject(entries: HistoryEntry[]): { project: string; label: string; entries: HistoryEntry[] }[]`: groups are ordered by their newest entry, `label` is the folder's last part, and `project` is the full cwd;
  - `filterOptions(entries)`, giving `{ agents, projects, branches, models }`, each sorted and deduped;
  - `sinceFor(when: "any" | "today" | "7d" | "30d", now: number): number | undefined`;
  - `rowMeta(e: HistoryEntry, now: number): string`, the row meta line from Global Constraints;
  - `moveSelection(ids: string[], current: string | null, key: "ArrowUp" | "ArrowDown" | "Home" | "End"): string | null`;
  - `escapeAction(state: { conversationOpen: boolean; query: string }): "close-conversation" | "clear-search" | "close-history"`, in exactly that priority.
- Produces, in `snippet.ts`: `splitSnippet(s: string): { text: string; hit: boolean }[]`. An unmatched U+0002 runs to the end. Stray U+0003 characters are dropped.

- [ ] **Step 1: Write the failing tests**, with one `it` per function and its exact outputs:
  - `rowMeta` gives `"demo · main · 2 h ago · 1 turn"`. Use the existing relative-time helper if one exists (search `src/` for "ago"), otherwise a local one with tests.
  - `history.list` builds a query string that leaves out the unset fields.

- [ ] **Step 2: Run the tests to check they fail.** `pnpm test src/features/history src/lib/api/history.test.ts`. Expected: they fail.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run the tests to check they pass.** Run the same command. Expected: they pass.

- [ ] **Step 5: Commit.** `feat(history): add the webview's history client and list state`

### Task 5: History and conversation views

**Files:**
- Create: `src/features/history/HistoryView.tsx`, `ConversationView.tsx`, `ToolCard.tsx`, `MarkdownText.tsx`, `History.css`
- Test: `HistoryView.test.tsx`, `ConversationView.test.tsx`, `History.css.test.ts` (copy the forced-colours check pattern from `src/features/onboarding/Onboarding.css.test.ts`)

**Interfaces:**
- Consumes: Task 4.
- Produces:
  - `HistoryView`, a presentational component with props:
    - `{ entries, filters, query, hits, selectedId, conversation: Conversation | "loading" | { error: string } | null, liveTabFor: (e) => string | null, now: number, os }`;
    - the handlers `onQuery, onFilter, onSelect, onOpen, onResume, onGoTo, onEscape`;
    - and `searchFocus: number`: a counter, and each new value focuses the search box.
  - `ConversationView { conversation, scrollToTurn?: number, onResume, liveTab }`.
  - `MarkdownText { markdown, cwd }`. It renders `safeMarkdownHtml(markdown, cwd)` when `typeof DOMParser !== "undefined"`, and otherwise as plain text in a `pre-wrap` block, so node tests render. Link clicks follow the Preview's handling (find it in `src/features/preview/Preview.tsx`).

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup`):
  - **Grouping:** the list renders a heading per project, and rows as `role="option"` inside `role="listbox"` with `aria-activedescendant` on the selected row.
  - **Empty and no-hit copy:** the empty copy, and the no-hit copy with the query, exact.
  - **Hits:** a hit's snippet renders `<mark>`.
  - **Conversation:**
    - each turn is a `section` with an `h3` holding the prompt as text, so `<b>x</b>` is escaped;
    - a tool card is a `<details>` whose `<summary>` is the card summary, with the detail in a `<pre>`; an error card has `data-error="true"`;
    - a subagent is a `<details>` with the summary `Subagent: …`;
    - a compaction is an `hr` and its text;
    - the limited notice is exact, with and without a version;
    - "Resume" shows, or "Go to session" when `liveTab` is set.
  - **CSS:** `History.css` uses only tokens from `src/styles/` and has a `@media (forced-colors: active)` block for `mark` and the selected row.

- [ ] **Step 2: Run the tests to check they fail.** `pnpm test src/features/history`. Expected: they fail.

- [ ] **Step 3: Implement.**
  - Load the `frontend-design` skill first. The look follows `docs/PLAN.md`'s visual system.
  - Layout: the list column is 360 px with the conversation beside it. Below 900 px wide, the conversation replaces the list, and Esc goes back.
  - The search box sits above the list, with the filters as selects in one row under it.

- [ ] **Step 4: Run the tests to check they pass.** `pnpm test src/features/history`. Expected: they pass.

- [ ] **Step 5: Commit.** `feat(history): draw the history list, search hits and conversations`

### Task 6: Wiring into the window

**Files:**
- Create: `src/features/history/LiveHistory.tsx`
- Modify: `src/features/window/MainWindow.tsx` (a `mainView?: ReactNode` prop that, when set, renders inside `.window__main` instead of the panes, the empty text and the preview; the sidebar stays)
- Modify: `src/features/sessions/LiveApp.tsx` (`view` state, the commands, Esc, sidebar select)
- Test: `src/features/window/MainWindow.test.tsx`, `src/features/history/LiveHistory.test.tsx`

**Interfaces:**
- Consumes: Tasks 4 and 5, plus `commandById("history")` (Ctrl+Shift+Y) and `commandById("global-search")` (Ctrl+Shift+Alt+F), which are already bound in `src/lib/keymap.ts`.
- Produces: `LiveHistory { endpoint, rows, os, searchFocus, onGoTo(tabId), onResumed(tabId), onClose }`.
  - It fetches the list on open and on each filter change.
  - Search waits 150 ms after the last keystroke. An empty query shows the list again.
  - It opens a conversation on Enter or a click, scrolling to the hit's turn.
  - Resume calls `history.resume`, then `onResumed` with the new tab. A refusal shows its reason in the view (`role="alert"`).
  - "Go to session" appears when an entry's `tab_id` is a live row in `rows`.

- [ ] **Step 1: Write the failing tests.**
  - MainWindow: with `mainView` set, `.window__main` holds it, no terminal renders, and the sidebar still renders.
  - LiveHistory's pure helper `liveTabFor(rows)(entry)`: returns the tab id only for a row whose state isn't Ended.

- [ ] **Step 2: Run the tests to check they fail.** `pnpm test src/features/window/MainWindow.test.tsx src/features/history`. Expected: they fail.

- [ ] **Step 3: Implement the wiring in `LiveApp`.**
  - `history` toggles `view` between `"sessions"` and `"history"`.
  - `global-search` sets `view = "history"` and bumps `searchFocus`.
  - Esc inside the view follows `escapeAction`, and `close-history` returns focus to the selected session's terminal.
  - Selecting a session in the sidebar closes the view.
  - `onResumed` and `onGoTo` close the view and select that tab.
  - The palette lists both commands already. Check that it runs them.

- [ ] **Step 4: Run all the checks and see them pass.** `pnpm test`, `pnpm build`, `pnpm check:generated`. Expected: all pass.

- [ ] **Step 5: Commit.** `feat(history): open the history view in the main area with Ctrl+Shift+Y and search from anywhere`

### Task 7: Live check (main thread)

**Files:**
- Create: `docs/research/2026-10-23-history-view-live-check.md`
- Create: an `#[ignore]` test `seed_history_home` in `crates/core/tests/history_api.rs`. It writes the five fixtures as archived sessions, each with a sidecar and the zstd copy, into the folder named by `MARSHELL_SEED_HOME`.

- [ ] **Step 1:** Seed a fresh literal scratch home with `MARSHELL_SEED_HOME=<scratch>\home cargo test -p marshell-core --test history_api seed_history_home -- --ignored`.
- [ ] **Step 2:** Run `pnpm tauri dev` in the background with that `MARSHELL_HOME` and CDP on 9333. Over CDP, check each of the following and record the result:
  - Ctrl+Shift+Y opens the view, with the sidebar still there.
  - The rows are grouped, and arrows and Enter work.
  - A search finds a word in a tool summary and opens its turn.
  - A tool card opens with Enter, and the limited notice shows on `future`.
  - Esc runs in its three steps, Ctrl+Shift+Alt+F focuses the search box, and forced colours look right (one screenshot).
- [ ] **Step 3:** Resume without the real Claude Code.
  - Put a `claude.cmd` shim in a scratch `bin` folder at the front of the dev run's `PATH`. It prints its arguments and waits for a key.
  - The seed test gives one session a transcript path to a scratch `<id>.jsonl` that exists.
  - Resume opens a tab whose output shows `--resume <id>`, in the sidecar's folder.
  - A session whose transcript is missing gives the refusal text.
  - The real `claude` never runs, so `~/.claude` is untouched; check its timestamps before and after.
- [ ] **Step 4:** Run `cargo test --release -p marshell-core search_1000_sessions_under_100ms -- --ignored` and record the median.
- [ ] **Step 5:** Stop by PID after listing the process tree, write the record, and link it from the phase 5 line in PLAN §9. Commit: `docs(history): record the slice 5a live check`.
