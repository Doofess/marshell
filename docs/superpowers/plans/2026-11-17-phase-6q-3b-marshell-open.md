# Slice 6q-3b: `marshell open`, the opener's mark, region on a page — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `marshell open <address or file>` adds a background tab to a session's side pane, marked with who opened it; a web page gets region to prompt; and the five leads 6q-3a left are fixed.

**Architecture:** The CLI posts to a new core route. The core checks the request, queues it, publishes the queue on the existing `/v1/events` stream and waits up to 5 s for the app's answer. The app stays the only writer of the pane's tabs: it applies each queued request to its own layout through a pure function, reports each one done or refused, and saves the layout as it always does. Region to prompt on a page reuses `src/features/regions/` over the picture the page is already frozen into. The page-side fixes are in `src-tauri/src/browser/`.

**Tech Stack:** Rust (axum, tokio, ureq), Tauri 2.12 with wry 0.57 and webview2-com 0.39.1, React 19, TypeScript, Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 6q-3b decisions (2026-11-13 and 2026-11-17)" 1–11. Carry-over: "Slice 6q decisions" first round 6 and second round 3 and 6; "Slice 6q-3 decisions" 4, 12 and 13; slice 2.5b's region decisions. Rules: `.claude/rules/local-api.md`, `protocol.md`, `tauri-shell.md`, `ui.md`, `config-writes.md`.

## Global Constraints

- **Security surface.** Nothing changes in `src-tauri/capabilities/`, `tauri.conf.json`, `tauri.windows.conf.json`, `crates/core/src/server/guard.rs` or token handling. No new Tauri command, no new Tauri event, no new crate or package (Cargo features of `windows`/`webview2-com` already in the tree are fine). If a task seems to need more, stop and report.
- **The routes** sit in the token-guarded router, each with a "refused without the bearer" test. New request and response types go in `crates/protocol/src/api.rs` with `#[derive(TS)] #[ts(export)]`, every `u64` with `#[ts(type = "number")]`.
- **The app is the only writer of the pane's tabs.** The core never edits `layout.json`'s tabs for an open request.
- **Numbers.** 32 tabs per session (unchanged). 64 requests may wait. The route waits 5 s for the app. A target is at most 4096 characters. A title is at most 200 characters. A region label's address is cut at 500 characters. `opened_by` is 1 to 32 characters of a-z, 0-9 and "-".
- **Names.** `opened_by` holds an agent id (`claude`, `codex`, …) or `shell`. A session's id is `tab_id`; a pane's tab is "pane tab" in identifiers.
- **Copy** (sentence case, no "!", no "successfully"; exact strings):
  - CLI usage line: `open [--session <session>] <address or file>...` with the text "Open a web address or a file as a tab in a session's side pane, in the background. An address starts with http:// or https://; anything else is a file. --session defaults to the session the command runs in."
  - CLI errors: "open takes at least one address or file." · "Outside a Marshell session, open needs --session <session>." · "No session {session}." · "No file {path}." · "No file {text}. For a web page, type the whole address: http://{text}" · "{path} is a folder, not a file." · "That path can't be opened in the side pane." · "That address can't be opened in the side pane." · "Session {n} already has 32 tabs in its side pane." · "Session {n} has closed." · "Too many tabs are waiting to open. Try again when Marshell's window is back." · "Browser tabs aren't available on this system yet."
  - CLI on a late answer (exit 0): "Marshell hasn't answered yet. The tab opens when its window does."
  - Announcements: "{Agent name} opened {title} in the side pane." · for `shell`: "Opened {title} in the side pane." (the existing text).
  - Tab tooltip: `{path or address} · Opened by {Agent name}` · for `shell`: `{path or address} · Opened with marshell open`.
  - Pane button's `title` while it shows the dot: the existing title, then " · A new tab is waiting".
  - URL bar button: `aria-label` and `title` "Select region". Command: `select-region` "Select region", no default key.
  - Status line under the bar: "This page tried to open a tab." · "The page's picture couldn't be taken, so there is nothing to select."
- **Styles.** Read the good-css skill before writing declarations. Tokens only, no hex. `:hover` only inside `@media (hover: hover) and (pointer: fine)`; forced-colors rules use system colours only; no boxed outlines. Tooltips on the URL bar are native `title`s.
- **Keyboard and forced colours.** Everything new works with the keyboard alone and in forced colours. `Announcer` is the only live region.
- **Windows only** for everything that touches a page. `marshell open <file>` works on every system.
- **Who writes what** (decision 8). Tasks 2 and 8 are written in the main thread. Tasks 1, 3–7 and 9 go to Sonnet workers (`effort: high` where the heading says so); the main thread reads each diff and runs its tests.
- **Checks.** Rust: `cargo test -p <crate> -j 4`; `cargo fmt --all` before each Rust commit; `cargo clippy --workspace --all-targets -j 4 -- -D warnings` at the end of each Rust task. After a protocol change: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`. Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** `--start`, a `--show` flag, opening from inside WSL, a path with a line number (`file.ts:12`), the text editor, a hint in the hook's context note.

## Review Focus

1. **Targets that aren't plain:** `localhost:3000`, `example.com`, `C:\x\a b.md`, `..\x.md`, a folder, a missing file, `file:///C:/x`, `javascript:alert(1)`, `http://user:pw@h/`, `\\server\share\x`, `""`, a 5000-character address, `HTTP://EXAMPLE.COM/`. Expected: only `http://`/`https://` text (any case) is an address; everything else is a path made absolute; each bad one gets its one line and exit 1, and nothing is queued. Tests: Task 3 `target_for` table; Task 2 `open_refuses_*`.
2. **Which session:** no `MARSHELL_TAB_ID` and no `--session`; `--session` naming a closed row; a number against a tab id; `--session` twice; a `from` that names no session. Expected: a usage error or "No session …", never another session's pane; an unknown `from` gives the plain dot. Tests: Task 3 `parse_open_*`, `open_run_*`; Task 2 `open_from_an_unknown_tab_is_the_shell`.
3. **The app isn't there to take it:** no events client, the window reloading, a 65th waiting request, an answer after the 5 s, a session closed while its request waits, `done` twice or for an unknown id. Expected: 202 with the request still waiting; 503 for the 65th; a closed session's requests are dropped; a late or repeated `done` changes nothing. Tests: Task 2 `open_waits_*`, `the_65th_*`, `closing_a_session_drops_its_requests`, `done_twice_is_harmless`.
4. **The same thing twice:** one address twice in one command; an open of the tab on screen; a tab closed before it was ever selected; the 33rd tab; the same queue frame delivered twice. Expected: one tab, marked again unless it is on screen; "limit" refused; a frame seen twice applies once. Tests: Task 4 `openBackground_*`, `takeOpens_*`.
5. **A region with no picture or a page that moves:** the capture failed; a blank tab; the tab, the session or the pane changes while a box is up; an overlay opens over the box; Esc at each step. Expected: the button is disabled with no live page; a failed capture gives the status line; any change of what is on screen ends the selection and unfreezes the page; a crop is never taken from a picture of another page. Tests: Task 7 `pageRegion_*`.

---

### Task 1: The layout holds who opened a tab (worker)

**Files:**
- Modify: `crates/protocol/src/api.rs` (`PaneTab`; new `OpenRequest`, `PaneOpen`, `PaneOpenDone`, `PaneOpensDone`)
- Modify: `crates/core/src/layout.rs` (`repair_pane`, `validate_pane`, tests), `crates/core/tests/layout_api.rs`
- Regenerated: `src/generated/*.ts`

**Interfaces:**
- Produces, in `api.rs`:
  - `PaneTab` gains `#[serde(default)] pub opened_by: Option<String>` ("Who opened it with `marshell open`: an agent's id, or `shell`. None when the person opened it in the app.") and `#[serde(default)] pub unseen: bool` ("True until the tab is first selected after `marshell open` added or marked it.").
  - `pub struct OpenRequest { pub target: String, #[serde(default)] pub from: Option<String> }` — `target` is an address or an absolute path; `from` is the caller's own `tab_id`.
  - `pub struct PaneOpen { #[ts(type = "number")] pub id: u64, pub tab_id: String, pub kind: String, pub target: String, pub by: String }` — `kind` is `"file"` or `"browser"`.
  - `pub struct PaneOpenDone { #[ts(type = "number")] pub id: u64, #[serde(default)] pub refused: Option<String> }` — `refused` is `"limit"` or `"address"`.
  - `pub struct PaneOpensDone { pub done: Vec<PaneOpenDone> }`.
- Valid `opened_by`: None, or 1 to 32 characters of a-z, 0-9 and "-". `unseen` may be true only with `opened_by` set.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s test module and `layout_api.rs`:
  - `repair_keeps_who_opened_a_tab`: a file tab with `opened_by: Some("claude")`, `unseen: true` stays as it is.
  - `repair_drops_a_bad_opener`: `Some("")`, `Some("Claude Code")`, a 33-character id → `opened_by: None`, `unseen: false`, tab kept.
  - `repair_clears_unseen_without_an_opener`: `opened_by: None, unseen: true` → `unseen: false`.
  - `validate_view_names_the_opener_field`: the three bad openers answer an error naming "opened_by"; `unseen` without an opener names "unseen".
  - `an_older_layout_file_loads_without_the_new_fields`: a `PaneTab` JSON with no `opened_by`/`unseen` deserializes to `None`/`false`.
  - `layout_api.rs`, `put_view_round_trips_an_opened_tab`: PUT a browser tab with `opened_by: "codex"`, `unseen: true`; GET returns the same.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 layout` → compile error for the fields, then FAIL.
- [ ] **Step 3: Implement** the fields, the four types, and the rules in `repair_pane` and `validate_pane`. Check that nothing in `layout.rs`'s file types uses `deny_unknown_fields` (an older build must still load a file with the new fields); if something does, stop and report.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 layout`, `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `pnpm build` → all pass (fix TypeScript object literals that build a `PaneTab`: `opened_by: null, unseen: false`).
- [ ] **Step 5: Commit** `feat(protocol): a pane tab records who opened it with marshell open, and the open request's types.`

### Task 2: The core's open route and its queue (main thread)

**Files:**
- Create: `crates/core/src/server/opens.rs`
- Modify: `crates/core/src/server/mod.rs` (state field, two routes), `crates/core/src/server/events.rs` (a fourth frame), `crates/core/src/server/sessions.rs` (`close` drops the session's requests)
- Test: `crates/core/tests/open_api.rs`

**Interfaces:**
- Consumes: Task 1's types; `files::refused(path: &str) -> bool`; the registry's `row(&tab)`.
- Produces:
  - `POST /v1/sessions/{tab}/open` with `OpenRequest`. Answers: 204 the app added or marked the tab; 202 with the late line when 5 s passed (the request keeps waiting); 404 "No such session."; 400 with one of the copy lines for a bad target; 409 with the line for `limit` (built with the session's number) or `address`; 503 with the "Too many tabs" line.
  - `POST /v1/pane/opens/done` with `PaneOpensDone` → 204 always; unknown ids are ignored.
  - `/v1/events` sends `{"type":"opens","data":[PaneOpen…]}` at connect and on every change of the queue.
  - `pub struct Opens` in `opens.rs`: `fn push(&self, open) -> Result<(u64, oneshot::Receiver<Option<String>>), Full>`, `fn done(&self, &[PaneOpenDone])`, `fn drop_session(&self, tab: &str)`, `fn subscribe(&self) -> watch::Receiver<Vec<PaneOpen>>`. `AppState::opens()`.
- Target rules, in `fn classify(target: &str) -> Result<(&'static str, String), &'static str>`: text starting with `http://` or `https://` in any case is a `browser` target, refused ("That address can't be opened in the side pane.") when it is longer than 4096 characters, holds a control character or a space, has an empty host, or has `@` before the first `/`, `?` or `#` after the scheme; on a system that isn't Windows every address answers "Browser tabs aren't available on this system yet.". Anything else is a `file` target: refused ("That path can't be opened in the side pane.") when it isn't absolute, is longer than 4096 characters, or `files::refused` says so; then, in `spawn_blocking`, "No file {path}." when it doesn't exist and "{path} is a folder, not a file." for a folder.
- `by`: the agent of `from`'s row when `from` names an open session with an agent, else `"shell"`.

- [ ] **Step 1: Write the failing tests** in `open_api.rs` (helpers as `layout_api.rs` builds its state; a fake app is a task that reads `st.opens().subscribe()` and calls the done route):
  - `open_is_refused_without_the_bearer`, `done_is_refused_without_the_bearer`.
  - `open_an_address_queues_it_and_answers_when_the_app_is_done` → 204; the frame held `kind: "browser"`, the target, `by: "shell"`.
  - `open_a_file_queues_a_file_target` (a temp file) and `open_names_the_callers_agent` (`from` = an agent session → `by` is its agent id).
  - `open_from_an_unknown_tab_is_the_shell`.
  - `open_refuses_a_bad_address`: the table from Review Focus 1 that reaches the route as an address → 400, nothing queued.
  - `open_refuses_a_bad_path`: relative, UNC, a device path, a missing file, a folder → 400 with each line.
  - `open_for_no_such_session_is_a_404`.
  - `open_answers_the_apps_refusal`: done with `refused: "limit"` → 409 "Session 1 already has 32 tabs in its side pane."; `"address"` → 409 with the address line.
  - `open_waits_five_seconds_then_answers_202` (the wait is a field on the state so the test sets 50 ms) and the request is still in the next frame.
  - `the_65th_waiting_request_is_refused` → 503.
  - `closing_a_session_drops_its_requests` → the waiting route answers 409 "Session 1 has closed.".
  - `done_twice_is_harmless`, `done_for_an_unknown_id_is_harmless`.
  - `events_sends_the_queue_at_connect_and_on_change` (beside the existing events tests).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test open_api` → FAIL (no route).
- [ ] **Step 3: Implement.** The queue is a `Mutex<Vec<(PaneOpen, oneshot::Sender<..>)>>` beside a `watch::Sender<Vec<PaneOpen>>`; no lock is held across an await. The 5 s is `AppState::with_open_wait`, default `Duration::from_secs(5)`.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 --test open_api` and `cargo test -p marshell-core -j 4 events` → PASS; clippy clean.
- [ ] **Step 5: Commit** `feat(core): an open request waits in the core for the app, which reports each one done or refused.`

### Task 3: `marshell open` (worker)

**Files:**
- Create: `crates/cli/src/open.rs`
- Modify: `crates/cli/src/args.rs` (`USAGE`, `Command::Open`, parsing), `crates/cli/src/client.rs` (`open`), `crates/cli/src/main.rs` (dispatch)

**Interfaces:**
- Consumes: `POST /v1/sessions/{tab}/open` (Task 2), `client::resolve`, `Client::sessions`.
- Produces:
  - `Command::Open { targets: Vec<String>, session: Option<String> }`.
  - `pub fn target_for(text: &str, cwd: &dyn Fn() -> io::Result<PathBuf>) -> Result<String, String>` in `open.rs`: text starting with `http://` or `https://` in any case passes through; anything else becomes an absolute path (joined to `cwd()` when relative, with `.` and `..` folded without touching the disk); empty text is an `Err`.
  - `pub enum Opened { Done, Late(String), Refused(String) }` and `Client::open(&self, tab: &str, req: &OpenRequest) -> Opened` (204, 202 with the body, anything else with the body or the failure text; timeout 10 s).
  - `pub fn run(env, targets, session, cwd, out, err) -> i32` in `open.rs`: exit 0 when every target was `Done` or `Late`, 1 when any was refused or the app is closed; a `Late` line and each refusal go to `err`, nothing to `out`.
- The session is `--session` when given, else `MARSHELL_TAB_ID`; with neither, the "Outside a Marshell session" line and exit 1. `from` is `MARSHELL_TAB_ID` when set. A 400 "No file {text}." for text that has no `\`, `/` or drive letter and holds a `.` or `:` gets the "For a web page" hint instead (decided in the CLI, from the text the person typed).

- [ ] **Step 1: Write the failing tests** (unit tests in `args.rs` and `open.rs`, in the style of `focus`'s):
  - `parse_open_one_target`, `parse_open_several_targets`, `parse_open_with_session` (before and after the targets), `parse_open_without_a_target_is_a_usage_error`, `parse_open_session_twice_is_a_usage_error`, `parse_open_unknown_flag_is_a_usage_error`.
  - `target_for` table: `https://example.com/a` → itself; `HTTP://EXAMPLE.COM/` → itself; `a.md` in `C:\w` → `C:\w\a.md`; `..\x.md` in `C:\w\p` → `C:\w\x.md`; `C:\x\a b.md` → itself; `localhost:3000` in `C:\w` → `C:\w\localhost:3000`; `file:///C:/x` → a path under the folder; `""` → `Err`.
  - `open_run_outside_marshell_needs_a_session`, `open_run_uses_the_callers_session`, `open_run_reports_each_refusal_and_exits_1`, `open_run_a_late_answer_exits_0`, `open_run_hints_at_the_whole_address` (`localhost:3000` → the hint line), `open_run_says_when_marshell_is_closed`.
- [ ] **Step 2: Run** `cargo test -p marshell-cli -j 4 open` → FAIL.
- [ ] **Step 3: Implement.** `run` takes the client behind a small trait or closures so the tests need no server, as `send`'s or `focus`'s tests do; follow whichever of the two is nearer.
- [ ] **Step 4: Run** `cargo test -p marshell-cli -j 4` → PASS; clippy clean.
- [ ] **Step 5: Commit** `feat(cli): marshell open asks the app for a background tab, for an address or a file.`

### Task 4: The app takes the queue (worker, effort high)

**Files:**
- Modify: `src/features/pane/paneState.ts` (+ test), `src/lib/api/events.ts` (+ test), `src/features/pane/paneCommands.ts` (+ test)
- Create: `src/features/pane/opens.ts`, `src/features/pane/opens.test.ts`, `src/lib/api/opens.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (wiring only)

**Interfaces:**
- Consumes: `PaneOpen`, `PaneOpenDone` (generated), `shellRefuses` (Task 9; until it lands, pass `() => false`).
- Produces:
  - `PaneAction` gains `{ type: "openBackground"; session: string; kind: "file" | "browser"; target: string; by: string; project: string; onScreen: boolean }` (`onScreen`: the pane is open with this session shown; `takeOpens` sets it from `ctx`). It never changes `open`. A new tab is appended with `opened_by: by`, `unseen: true`; it becomes `active` only when the session had no tabs. A target already open (`tabHas`/`tabShows`) gets `opened_by: by` and `unseen: true`, unless it is the session's active tab and `onScreen` holds; a file target also goes to the recent list. At 32 tabs with no match the state is returned unchanged.
  - Every action that makes a tab the session's active one (`select`, `step`, `open`, `openPage`, `close` choosing a neighbour) clears that tab's `unseen`. `opened_by` stays.
  - `export function takeOpens(view: PaneView, opens: readonly PaneOpen[], seen: ReadonlySet<number>, ctx: { sessions: ReadonlyMap<string, { project: string }>; shown: string | null; paneOpen: boolean; refuses(url: string): boolean }): { view: PaneView; done: PaneOpenDone[]; said: string[]; news: string[] }` in `opens.ts`: ids in `seen` are skipped; a request for a session not in `sessions` is `done` with no change; a browser target that `refuses` or isn't `isPageUrl` is `refused: "address"`; `openRefused` → `refused: "limit"`; otherwise the reducer runs. `said` holds one announcement per added or marked tab (copy above; the title is `pageTitle(null, url)` or the file's name); `news` holds the `tab_id`s that got a tab while the pane was hidden or another session was on screen.
  - `EventsClient` gains an `onOpens?: (opens: PaneOpen[]) => void` handler for the `"opens"` frame; `opensApi.done(ep, done: PaneOpenDone[])` posts `/v1/pane/opens/done`.
- LiveApp: on each frame, run `takeOpens` against the current pane state, dispatch the result, announce `said`, add `news` to the pane-news set (Task 5), and post `done`. Keep the applied ids in a ref so a frame delivered twice applies once.

- [ ] **Step 1: Write the failing tests:**
  - `paneState.test.ts`: `openBackground_adds_an_unselected_tab_and_keeps_the_pane_hidden`, `openBackground_in_an_empty_session_selects_it_and_still_keeps_the_pane_hidden`, `openBackground_marks_a_tab_already_open`, `openBackground_leaves_the_tab_on_screen_unmarked`, `openBackground_at_32_tabs_changes_nothing`, `selecting_a_tab_clears_unseen_and_keeps_the_opener`, `stepping_onto_a_tab_clears_unseen`, `closing_the_active_tab_clears_unseen_on_the_one_that_takes_over`.
  - `opens.test.ts`: `takeOpens_applies_each_request_once`, `takeOpens_skips_ids_already_seen`, `takeOpens_refuses_an_address_the_shell_would`, `takeOpens_refuses_the_33rd_tab`, `takeOpens_reports_a_closed_session_done`, `takeOpens_says_who_opened_what` (an agent, and `shell`), `takeOpens_lists_news_only_for_a_hidden_pane_or_another_session`, `takeOpens_twice_in_one_frame_makes_one_tab` (the same address twice).
  - `events.test.ts`: `an_opens_frame_reaches_its_handler`, `a_malformed_opens_frame_is_dropped`.
- [ ] **Step 2: Run** `pnpm test src/features/pane src/lib/api/events.test.ts` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests, then `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(pane): the app adds the tabs marshell open asks for, in the background, and reports each one.`

### Task 5: The mark on the tab and the dot on the pane button (worker)

**Files:**
- Modify: `src/features/pane/SidePane.tsx`, its CSS and tests; `src/features/window/MainWindow.tsx`, its CSS and tests; `src/features/sessions/LiveApp.tsx` (props)
- Create: `src/features/pane/paneNews.ts`, `src/features/pane/paneNews.test.ts`

**Interfaces:**
- Consumes: `PaneTab.opened_by`, `PaneTab.unseen`; `AgentMark` (`src/components/AgentMark/AgentMark.tsx`), `AGENT_NAMES`, `isAgentId` (`agents.ts`); Task 4's `news`.
- Produces:
  - `SidePane`'s tab items gain `openedBy: string | null` and `unseen: boolean`. An unseen tab shows, before its type icon, `<AgentMark agent={id} size={12} />` for a known agent id, else a plain dot (`<span className="side-pane__new" aria-hidden="true" />`); the tab's accessible name gains ", new" (a visually hidden span, not `aria-label`). `title` follows the tooltip copy whenever `openedBy` is set.
  - `paneNews.ts`: `export function newsReducer(news: ReadonlySet<string>, a: { type: "add"; sessions: readonly string[] } | { type: "seen"; session: string } | { type: "closed"; session: string }): ReadonlySet<string>`; the same set comes back when nothing changes. LiveApp sends `seen` whenever the pane is open with that session on screen.
  - `MainWindow` gains `paneNews?: boolean`: the toggle gets `data-news=""`, the " · A new tab is waiting" title, and a dot drawn by CSS on the button.

- [ ] **Step 1: Write the failing tests:** `SidePane.test.tsx` `an_unseen_tab_shows_its_agents_mark`, `an_unseen_shell_tab_shows_a_plain_dot`, `a_seen_tab_keeps_only_the_tooltip`, `an_unknown_agent_id_shows_the_plain_dot`; `paneNews.test.ts` `add_then_seen`, `seen_for_another_session_changes_nothing`, `closed_forgets_the_session`; `MainWindow.test.tsx` `the_pane_button_shows_news`, `no_news_no_dot`; the CSS tests the folder already has (`noBoxedOutlines`, forced colours) stay green with the new rules.
- [ ] **Step 2: Run** `pnpm test src/features/pane src/features/window` → FAIL.
- [ ] **Step 3: Implement.** Read the good-css skill first. The dot is a filled circle from an accent token; in forced colours it uses `Highlight`.
- [ ] **Step 4: Run** the tests, `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(pane): a tab opened by marshell open carries its opener's mark until it is selected, and the pane button shows a dot.`

### Task 6: The skill and the help text (worker)

**Files:**
- Modify: `crates/core/src/agents/skill/SKILL.md`, `crates/core/src/agents/skill.rs` (`PREVIOUS`, tests)

**Interfaces:**
- Produces: the skill's description ends "…, to hand work to another agent CLI in a new Marshell session, or to show a web page or a file in the side pane."; the intro sentence says the command "lists, starts, fills and focuses the app's sessions, and opens tabs in the side pane"; a fifth command line: "`marshell open <address or file>...`: opens each as a tab in the side pane of your own session, in the background. The person sees a mark on the tab and selects it when they want to. An address starts with `http://` or `https://`; anything else is a file path. `--session <session>` names another session."; a rule: "Open only what the person asked to see, or a page or file you made for them. Don't open the same thing again and again."; and under the refusals: "\"already has 32 tabs\": tell the person, and don't close tabs for them."
- `PREVIOUS` gains the `hash_bytes` hex of the text as it is on `main` (alpha.1 shipped it).

- [ ] **Step 1: Write the failing tests** in `skill.rs`: `the_skill_names_every_command` (`list`, `send`, `new`, `focus`, `open` each appear as "`marshell <name>"); `the_shipped_alpha_1_text_is_a_previous_one` (the hash of `git show origin/main:crates/core/src/agents/skill/SKILL.md`, written as a literal, is in `PREVIOUS` and differs from the current text's hash).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 skill` → FAIL.
- [ ] **Step 3: Implement** the text and the hash. Read how `PREVIOUS` is used (the "is the file on disk ours" check and the update plan) and confirm in the report that an installed alpha.1 copy is offered an update and not treated as the person's own file.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 skill` and `cargo test -p marshell-core -j 4 agents` → PASS.
- [ ] **Step 5: Commit** `feat(core): the marshell skill tells agents about marshell open.`

### Task 7: Region to prompt on a page (worker, effort high)

**Files:**
- Modify: `src/features/browser/BrowserTab.tsx`, `BrowserTab.css`, `UrlBar.tsx` and their tests; `src/features/regions/region.ts` (+ test); `src/lib/keymap.ts`; `src/features/preview/Preview.tsx` (the command starts its region too); `src/features/sessions/LiveApp.tsx` (wiring)
- Create: `src/features/browser/pageRegion.ts`, `src/features/browser/pageRegion.test.ts`

**Interfaces:**
- Consumes: `RegionLayer`, `RegionComment`, `initialRegion`, `fromDrag`, `regionText` (`src/features/regions/`), `cropImage(url, region)` (`crop.ts`), `useCoverPage` (`pageCover.ts`), the freeze's picture (`usePageSlot`), LiveApp's `addRegion({ png, label, comment })`.
- Produces:
  - `export function pageRegionLabel(title: string, url: string): string` in `region.ts`: `` `${title} (${url cut at 500 characters})` ``; with an empty title, the address alone.
  - `pageRegion.ts`: `export type PageRegion = { step: "wait" | "box" | "comment"; region: Region; busy: boolean; error: string | null; url: string } | null` and `export function pageRegionStep(s: PageRegion, e: { type: "start"; url: string } | { type: "picture"; ok: boolean } | { type: "confirm" } | { type: "back" } | { type: "cancel" } | { type: "moved"; url: string } | { type: "hidden" } | { type: "busy" } | { type: "failed"; error: string } | { type: "added" }): PageRegion`. `start` → `wait`; `picture ok` → `box`; `picture` not ok → null (the caller shows the status line); `moved` with another address, `hidden`, `cancel` and `added` → null.
  - `UrlBar` gains `onRegion?: () => void` and `canRegion?: boolean`: an icon button before "Open in the system browser", `aria-label`/`title` "Select region", `aria-pressed` while selecting, disabled without a live page.
  - `BrowserTab` gains `onRegion?: (r: { png: Blob; label: string; comment: string }) => Promise<void>` and `ref` handle method `selectRegion()`. While a selection is up it holds a page cover (`useCoverPage(selecting)`), draws the capture surface, `RegionLayer` and `RegionComment` over the picture inside the slot, and crops from that same picture. The keys are `RegionLayer`'s (arrows, Ctrl, Shift, Enter, Esc). A change of the page's address, the tab going off screen, or the picture going away ends the selection.
  - Command `{ id: "select-region", label: "Select region", group: <the side pane's group> }` with no binding: LiveApp runs the active tab's `selectRegion()` for a page and the preview's `startSelect` for a PDF or an image; for any other tab it announces "This tab has nothing to select a region in.".

- [ ] **Step 1: Write the failing tests:** `pageRegion.test.ts` — `pageRegion_waits_for_the_picture_then_shows_the_box`, `pageRegion_ends_when_the_capture_failed`, `pageRegion_ends_when_the_page_moved`, `pageRegion_ends_when_the_tab_is_hidden`, `pageRegion_esc_in_the_comment_goes_back_to_the_box`, `pageRegion_keeps_the_box_after_a_failed_add`; `region.test.ts` — `pageRegionLabel` (title and address; no title; a 600-character address is cut at 500); `UrlBar.test.tsx` — `the_region_button_is_disabled_without_a_live_page`, `the_region_button_is_pressed_while_selecting`; `BrowserTab.test.tsx` — `a_selection_draws_the_box_over_the_picture`, `no_picture_no_box`.
- [ ] **Step 2: Run** `pnpm test src/features/browser src/features/regions` → FAIL.
- [ ] **Step 3: Implement.** The crop uses the picture's natural size, so a 125% or 150% display gives a full-resolution PNG. Overlays drawn over the slot must not double-freeze: the selection's own layer lives inside the slot, above the picture.
- [ ] **Step 4: Run** the tests, `pnpm test src/lib`, `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(browser): region to prompt on a web page, from the URL bar and the palette.`

### Task 8: The page-side leads (main thread)

**Files:**
- Modify: `src-tauri/src/browser/mod.rs`, `com.rs`, `rules.rs`

**Interfaces:**
- Produces:
  - `rules.rs`: `pub fn clipped_title(title: String) -> String` (200 characters, by `char`); `pub fn reported_address(url: String) -> String` (itself up to 4096 characters; longer, its origin and "/" when it parses, else empty); `pub struct PopupGate` with `fn asked(&mut self, user: bool)` and `fn take(&mut self) -> Option<bool>` (a queue: the COM handler records each request's `IsUserInitiated`, the posted job reads it; `None` means nobody recorded it, which lets the popup through as before).
  - `com.rs`: a `NewWindowRequested` handler that only records `IsUserInitiated` in the page's `PopupGate`; `Page::state` sends `clipped_title` and `reported_address`.
  - `mod.rs`: the popup job emits `browser://popup` only when the gate doesn't say "no person asked". A blocked one goes out as the existing `browser://refused` event, whose payload `PageUrl` gains `#[serde(skip_serializing_if = "Option::is_none")] why: Option<&'static str>`, here `Some("popup")`. No new event name.
  - `browser_open` runs `create` through `tauri::async_runtime::spawn_blocking`.
  - `create` makes the page at `about:blank`, installs every handler, then navigates to the address; the `loading` flag still starts true.
- Frontend follow-up in the same task: `BrowserEvents`' refused payload gains `why?: "popup"`, and the status line reads "This page tried to open a tab." for it (`pageStore.ts`'s refused handling and its test).

- [ ] **Step 1: Write the failing tests** in `rules.rs`: `a_long_title_is_cut_at_200_characters` (300 "é" → 200), `an_address_over_4096_characters_is_reported_as_its_site` (`https://example.com/` + 5000 "a" → `https://example.com/`; unparsable and long → ""), `a_short_address_is_reported_as_it_is`, `popup_gate_answers_in_order` (asked true, asked false → take true, take false, take None); in `mod.rs`: `making_a_page_leaves_the_runtime_free` (a job handed to the same helper `browser_open` uses blocks until a future on a current-thread runtime completes; the test fails by timeout if the job ran on the worker); `pageStore.test.ts`: `a_refused_popup_reads_as_a_blocked_tab`.
- [ ] **Step 2: Run** `cargo test -p marshell-app -j 4 browser` and `pnpm test src/features/browser/pageStore.test.ts` → FAIL.
- [ ] **Step 3: Implement.** Keep every COM call's `// SAFETY:` note in the file's style. If `IsUserInitiated` isn't in webview2-com 0.39.1's `ICoreWebView2NewWindowRequestedEventArgs`, stop and report.
- [ ] **Step 4: Run** `cargo test -p marshell-app -j 4`, clippy, `pnpm test src/features/browser` → PASS.
- [ ] **Step 5: Commit** one commit per lead, each `fix(browser): …` naming what a person sees.

### Task 9: An address the shell would refuse never gets a tab (worker)

**Files:**
- Create: `src-tauri/src/browser/allowed.json`
- Modify: `src-tauri/src/browser/rules.rs` (a test reads the file), `src/features/browser/address.ts` (+ test), `src/features/pane/paneCommands.ts` (+ test), `src/features/browser/useBrowserPages.ts` (popups), `src/features/sessions/LiveApp.tsx` (the own origins)

**Interfaces:**
- Produces:
  - `allowed.json`: an array of `{ "url": string, "own": [[host, port]…], "allowed": boolean }`, at least 40 cases taken from `rules.rs`'s existing tables: plain sites, every spelling of this computer against an own origin on the same and another port, `tauri` and `ipc` names on both schemes, userinfo, other schemes, `about:blank`, a trailing dot, upper case.
  - `export function shellRefuses(url: string, own: readonly (readonly [string, number])[]): boolean` in `address.ts`, the mirror of `allowed_navigation`: true when the shell would refuse it.
  - `pageOpenCheck` gains a fourth argument `own` and answers `"refused"` for such an address; the popup path runs the same check and announces the existing "That address can't be opened in the side pane.".
  - LiveApp builds `own` from `window.location` (host and port, the scheme's default port when none) and the endpoint's port, and passes it to `pageOpenCheck`, the popup path and `takeOpens`' `refuses`.

- [ ] **Step 1: Write the failing tests:** `rules.rs` `every_shared_case_matches_the_rule` (reads `allowed.json` with `include_str!`); `address.test.ts` `shellRefuses_agrees_with_the_shell_in_every_shared_case`; `paneCommands.test.ts` `an_own_origin_is_refused_before_a_tab_opens`; `useBrowserPages.test.ts` `a_popup_to_an_own_origin_opens_no_tab`.
- [ ] **Step 2: Run** `cargo test -p marshell-app -j 4 shared_case` and `pnpm test src/features/browser src/features/pane` → the Rust test passes once the file exists (it pins the file to the rule); the TypeScript ones FAIL.
- [ ] **Step 3: Implement** `shellRefuses` with the platform's `URL` parser; a case where the two parsers disagree is recorded in the file with a comment key and reported.
- [ ] **Step 4: Run** the tests, `pnpm build` → PASS.
- [ ] **Step 5: Commit** `fix(browser): a link to one of the app's own addresses is refused before a tab is made for it.`

### Task 10: Live check, docs, review and the pull request (main thread)

- [ ] **Step 1: Live check** with the `marshell-live-check` skill (scratch homes; the CLI is `target/debug/marshell.exe`; stop only what it started). Checks:
  1. In a shell session: `marshell open https://example.com/` → exit 0, a tab with the plain dot, not selected, the keyboard still in the terminal.
  2. With the pane hidden: the pane button shows the dot; showing the pane clears it; selecting the tab clears its mark and the tooltip keeps "Opened with marshell open".
  3. `marshell open README.md other.pdf` from the project folder → two file tabs.
  4. From a second terminal outside Marshell: without `--session` → the line and exit 1; with `--session 1` → the tab.
  5. Refusals, each with its line and exit 1: a missing file, a folder, `localhost:3000` (the hint), the app's own dev address, a 33rd tab.
  6. An agent session (a signed-in CLI the check starts): ask it to run `marshell open` → the agent's icon on the tab, and the announcement text in the DOM's live region.
  7. Region on a page: the button and the palette command; arrows, Shift, Enter, a comment, Enter → the prompt line holds the image reference and `Example Domain (https://example.com/): <comment>`; Esc at each step; switching tabs mid-selection ends it and the page comes back.
  8. Leads: a test page whose script calls `window.open` on load → no tab, the status line; a button on it → a tab; a page that sets a 5000-character title → the strip shows 200; a first address that redirects → the allow-list still decides; 10 quick `marshell open` calls → the window stays responsive.
  9. Forced colours and keyboard only for the mark, the dot and the region box.
  Findings go in `docs/research/2026-11-17-marshell-open-live-check.md`.
- [ ] **Step 2: Docs.** `docs/PLAN.md` §9: where 6q-3b stands; `.claude/rules/tauri-shell.md` and `local-api.md` only if a rule's wording is now wrong; the CLI's own docs page if one lists the commands (`grep -rn "marshell focus" docs README.md`).
- [ ] **Step 3: Final review** by the `marshell-reviewer` agent with this plan's Review Focus, then one fix pass.
- [ ] **Step 4: Ship** with the `marshell-ship` skill, then the `pre-merge-review` skill before the merge.
