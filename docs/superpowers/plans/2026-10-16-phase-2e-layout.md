# Slice 2e: window layout, restore and Resume — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the prototype window live. That means a split view of up to four panes, focus mode, a resizable sidebar, a real sidebar shell panel and a project grouping toggle. All of it is saved in `~/.marshell/layout.json`, and on relaunch every tab comes back as Ended with a one-click Resume.

**Architecture:** The core owns `layout.json`.
- It writes the tab list itself, from the registry's watch channel.
- The UI writes only the `view` part, through `PUT /v1/layout/view`.
- On start (a relaunch, or a core restart), the core seeds the registry with `Ended` tabs from the file and starts nothing.
- Resume restarts a tab in place, keeping its id, number, name and position. It starts the tab's profile in its folder, and an agent tab gets the CLI's own continue-last flag from the profile detection table.

On the frontend, one pure reducer (`viewState.ts`) holds the view: panes, arrangement, ratios, zoom, sidebar, shell panel and grouping. `MainWindow` stays presentational and gains an explicit `paneIds` input.

**Tech Stack:** Rust (axum, serde, ts-rs, tokio watch), React 19 + TypeScript, xterm.js, vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md`:
- :126–143 (window, sidebar, shell panel, split view, focus mode);
- :543 (`layout.json`);
- :751–763 (store rules, relaunch, slice 2e exit);
- :385 (cold start);
- the keymap section (:658–704).

Read with `offset`/`limit`, never whole. User decisions of 2026-10-16:
- 2e is split from 2e+ (projects and task worktrees).
- Resume uses the CLI's own continue-last-here.
- The split view has up to four panes.
- Grouping is off by default.

## Global Constraints

- **Rust:** keep it simple, because the user is new to Rust. Use plain structs, `Mutex`, no new traits beyond `store::Versioned`, and no new crates.
- **Dependencies:** none added on either side (AGENTS.md: "Do not add dependencies … without a concrete need documented in the plan").
- **Every file in `~/.marshell`** has a version field, is written atomically, keeps a `.bak`, and migrates on load (PLAN :751). Use `store::write` and `store::read`.
- **On relaunch,** tabs come back as "Ended" with a one-click Resume. "Nothing relaunches itself or spends tokens unasked" (PLAN :752).
- **Vendor-specific resume flags** live only in `crates/core/src/profiles/detect.rs` next to `agent_command`. Generic shell tabs must work with no agent.
- **Keys** match by `KeyboardEvent.code`, and no shortcut uses punctuation. Avoid U, E, C, V, Z and J for new keys.
- **One live region** (`SessionAnnouncer`): no `role="alert"` and no new `aria-live`.
- **Every text control** has a class with a background. CSS uses tokens only from `src/styles/`.
- **Terminal text is never dimmed.** Inactive pane headers sit at 60%. A pane that needs you keeps its full tint.
- **Layout sizes:**
  - Window: minimum 720×480. Below 960 px the sidebar is the rail.
  - Sidebar: 288 default, 200 min, 400 max, rail 52, focus 0.
  - Pane header: 28 px. Divider: 1 px with an 8 px hit area.
  - A count is offered only if every pane gets 60×14. A compact prompt line is 42 px.
  - Shell panel: header 32, body 30% clamped to 120–280.
- **Copy** is sentence case and plain. The Resume button says "Resume", and its toast says "Resumed".
- **Tests:** vitest runs in node with `renderToStaticMarkup` and pure modules. Rust tests sit in-file with `tempfile` (`store.rs` style) or in `crates/core/tests/` for HTTP.
- **Generated bindings:** after a protocol change run `cargo test -p marshell-protocol`, then `pnpm check:generated`.
- **Commits:** end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. In this worktree run `git add` and `git commit -q -F <msgfile>` as separate calls.

## Rulings (decided while planning)

1. **The closed list is not saved.** Reopen covers this run only, and only open tabs go to `layout.json`.
2. **Focus mode key:** Ctrl+Shift+I on Windows and Linux, ⌘I on macOS (`KeyI`, id `focus-mode`). These were ruled out:
   - M, H, Q and A, because macOS uses them for window or app actions;
   - X, because it is cut;
   - U, E, C, V, Z and J, as the keymap audit says.
3. **Ended and Exited:**
   - **Ended** is a new state: "restored from the last run, nothing has run here yet." It has no terminal, only the Resume view.
   - **Exited** keeps its terminal so the output can still be read, with the same Resume bar under it. Resume replaces the old "Enter starts a new shell" text.
4. **Antigravity (`agy`)** resume is unverified, so an agy tab resumes as a fresh start with no flag. Claude uses `--continue` and Codex uses `resume --last`.
5. **A missing profile or folder:** when a restored tab's profile was deleted or its folder is gone, Resume is disabled and says why. It never falls back to another profile or folder.
6. **The shell panel** is its own pty session, kept out of the registry, so typing there can never reach a tab.
   - It starts in the focused session's folder. It never types `cd` into itself, because that would act unasked. Its header shows the folder it started in.
   - "Open in a pane" adopts that pty into the registry as a normal tab, keeping its history. The panel starts a fresh shell the next time it is shown.
   - It uses the default shell profile.
7. **Panes remount when the layout changes.** The core replays the buffered output on resume, so scrollback is kept, but the scroll position resets. Terminals never remount when the selection moves inside a layout.
8. **The arrow nav cursor** (modifier+arrows moving a cursor) moves to 2f together with the palette and cheat sheet. Tab and Backspace nav chords ship here.
9. **The focus-mode pill and next-waiting (N)** stay hidden or unbound live until phase 3 supplies real needs-you status ("no guessed status values").
10. **Grouping:** when it is on, drag and Alt+Shift+arrow move a session only within its project group. The order inside a group is the core's order.

## Review Focus

1. **A corrupt, truncated or newer-version `layout.json`** must not stop the app.
   - The app falls back to the `.bak`, then to defaults. It starts, and the broken file is not overwritten until the next real change.
   - Test: `layout::tests::corrupt_file_falls_back_to_bak_then_defaults` (Task 2).
2. **A restored tab whose profile was deleted or whose folder no longer exists.**
   - It shows as Ended with Resume disabled and a reason. Pressing Resume through the API returns 409 with that reason and starts nothing.
   - Tests: `resume_refuses_missing_profile` and `resume_refuses_missing_folder` (Task 4); `ResumeBar` disabled copy (Task 9).
3. **A double click or held Enter on Resume** must not start two processes. The second request gets 409 "already running".
   - Test: `resume_twice_starts_one_process` (Task 4).
4. **A session shown in a pane closes or exits.** Its pane becomes empty and offers the other sessions. The remaining panes do not move, and focus stays put.
   - Test: `viewState` `"a closed session leaves an empty pane in place"` (Task 6).
5. **The window shrinks below what the chosen count fits.**
   - Fewer panes show and the menu says "Showing 2 of 3". The assignments are kept and come back when the window grows. Nothing is saved as the smaller count.
   - Tests: `"shrinking keeps every assignment"` (Task 6) and `MainWindow` `"paneIds beyond the fit are hidden, not dropped"` (Task 7).

---

### Task 1: Protocol types for layout and Ended

**Files:**
- Modify: `crates/protocol/src/api.rs` (`SessionState` ~l.98; new layout types after `SessionsSnapshot`)
- Generated: `src/generated/*.ts` (via `cargo test -p marshell-protocol`)
- Modify: `src/features/sessions/toRow.ts` (map `ended`)
- Test: `crates/protocol/src/api.rs` tests module, and `src/features/sessions/toRow.test.ts`

**Interfaces:**
- Produces (Rust, all `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)] #[ts(export)]`):
  - `SessionState::Ended` serializes as `{"kind":"ended"}`.
  - `SavedTab { tab_id: String, number: u32, name: String, agent: Option<String>, profile_id: String, cwd: String }`.
  - `ViewLayout`:
    - `panes: Vec<Option<String>>` (length 1–4);
    - `arrangement: Option<String>` (an `Arrangement` name, or `None` for automatic);
    - `ratios: Vec<f32>` (empty means even);
    - `active: u32`;
    - `zoomed: Option<u32>`;
    - `sidebar: String` (`"expanded" | "rail" | "focus"`);
    - `sidebar_width: u32`;
    - `shell_hidden: bool`;
    - `grouped: bool`.

    It implements `Default` as `panes: vec![None]`, `arrangement: None`, `ratios: []`, `active: 0`, `zoomed: None`, `sidebar: "expanded"`, `sidebar_width: 288`, `shell_hidden: false` and `grouped: false`.
  - `LauncherChoice { profile_id: String, agent: Option<String> }`.
  - `LayoutDoc { tabs: Vec<SavedTab>, view: ViewLayout, launcher: BTreeMap<String, LauncherChoice> }`. The `launcher` map is keyed by folder path.
  - `ResumeError { reason: String }` is the 409 body.
- Produces (TS): `toRow` (`src/features/sessions/toRow.ts:37`, beside the `exited` case) maps `state.kind === "ended"` to `status: "ended"` (the glyph kind exited tabs already use) with the phrase `"Ended last time"`.

- [ ] **Step 1: Write the failing tests**
  - Rust: `ended_state_serializes_as_kind_ended` asserts `serde_json::to_string(&SessionState::Ended).unwrap() == r#"{"kind":"ended"}"#`.
  - Rust: `view_layout_default_matches_plan` asserts `sidebar_width == 288`, `panes == vec![None]` and `!grouped`.
  - TS: in `toRow.test.ts`, `"an ended tab reads Ended"` asserts `toRow({...info, state: { kind: "ended" }}, profiles).phrase` contains `"Ended"`.
- [ ] **Step 2: Run them.** Run `cargo test -p marshell-protocol 2>&1 | tail -5`. Expected: FAIL (no variant `Ended`).
- [ ] **Step 3: Add the variant and the types, regenerate the bindings (`cargo test -p marshell-protocol`), and add the `toRow` case.**
  - Fix every `match` on `SessionState` that the compiler flags in `crates/core`. Ended counts as not running wherever `Exited` does.
- [ ] **Step 4: Run** `cargo test --workspace 2>&1 | grep -E "test result|FAILED" | tail -5`, `pnpm check:generated` and `pnpm test -- toRow`. Expected: all pass.
- [ ] **Step 5: Commit** with `feat(protocol): Ended tab state and layout.json types`.

### Task 2: Core layout store and `/v1/layout`

**Files:**
- Create: `crates/core/src/layout.rs`
- Create: `crates/core/src/server/layout.rs`
- Modify: `crates/core/src/lib.rs` (module), `crates/core/src/server/mod.rs` (routes at :116–141, `layout: Mutex<LayoutDoc>` in `Inner`, loaded in `AppState::new`)
- Test: in-file `layout.rs` tests; `crates/core/tests/layout_api.rs`

**Interfaces:**
- Consumes: Task 1 types and `store::{Versioned, read, write}`.
- Produces:
  - `struct FileDoc(LayoutDoc)` with `impl Versioned` (`VERSION = 1`, `migrate` passes v1 through).
  - `pub fn load(home: &Path) -> LayoutDoc`: it reads `home/.marshell/layout.json` (the `store::read` fallback to `.bak` is built in), repairs the result, and returns `LayoutDoc::default()` on any error. It never writes.
  - `pub fn save(home: &Path, doc: &LayoutDoc) -> anyhow::Result<()>`.
  - `pub fn repair(doc: LayoutDoc) -> LayoutDoc`. It clamps the following:
    - `panes` to 1–4 (`vec![None]` when empty);
    - `active` and `zoomed` into range, with `zoomed` set to `None` when out of range;
    - `sidebar_width` to 200–400;
    - `sidebar` to `"expanded"` when unknown;
    - `ratios` to empty when the count is wrong or a value is outside 0.1–0.9;
    - `arrangement` to `None` when unknown. The list of names is `ARRANGEMENTS` in `src/features/window/splitLayout.ts`; copy it to a `const` slice.

    It also drops panes that point at a tab not in `tabs`, setting them to `None`.
  - `pub fn validate_view(v: &ViewLayout) -> Result<(), String>`. It applies the same rules, but returns an error that names the field instead of clamping.
  - Routes:
    - `GET /v1/layout` returns `Json<LayoutDoc>`.
    - `PUT /v1/layout/view` takes a `Json<ViewLayout>` body. A bad body gets 400 with the field name. A good one replaces `doc.view` and saves in `spawn_blocking` while holding the mutex, then returns 204. This follows `server/settings.rs`.

- [ ] **Step 1: Write the failing tests**
  - `save_then_load_round_trips`.
  - `keeps_previous_as_bak`, which mirrors `store.rs`.
  - `corrupt_file_falls_back_to_bak_then_defaults`. It writes garbage to `layout.json` with a valid `.bak` and checks that load returns the `.bak` content. Then it writes garbage to both and checks for `LayoutDoc::default()`. Finally it asserts the garbage file bytes are unchanged after `load`.
  - `repair_clamps_sidebar_width_and_drops_unknown_panes`: input width 9000 with panes `[Some("gone")]` gives 400 and `[None]`.
  - HTTP: `put_view_rejects_five_panes` returns 400 with a body containing `"panes"`. `put_view_then_get_returns_it` checks the round trip.
- [ ] **Step 2: Run** `cargo test -p marshell-core layout 2>&1 | tail -5`. Expected: FAIL (unresolved module).
- [ ] **Step 3: Implement `layout.rs` and the two routes as above.**
- [ ] **Step 4: Run** the same command and `cargo clippy -p marshell-core --all-targets -- -D warnings 2>&1 | tail -3`. Expected: pass, clean.
- [ ] **Step 5: Commit** with `feat(core): layout.json store and /v1/layout`.

### Task 3: Tabs saved from the registry, and restored as Ended

**Files:**
- Modify: `crates/core/src/registry.rs` (add `seed_ended`, `saved_tabs`)
- Modify: `crates/core/src/server/mod.rs` (`AppState::new` seeds from `layout.tabs`; spawn the saver task)
- Test: in-file `registry.rs` tests; `crates/core/tests/layout_restore.rs`

**Interfaces:**
- Consumes: `layout::{load, save}` and `SavedTab` (Tasks 1 and 2).
- Produces:
  - `Registry::seed_ended(&self, tabs: &[SavedTab], relaunch_of: impl Fn(&SavedTab) -> Relaunch)` pushes `Entry`s with `state: SessionState::Ended` and `command: CommandState` at its idle/unknown default, in the given order. It keeps the tab ids and numbers. If two tabs share a number, the later one gets `lowest_free_number`.
  - `Registry::saved_tabs(&self) -> Vec<SavedTab>` returns the open entries in tab order, of any state.
  - Saver: `fn spawn_layout_saver(state: AppState)` subscribes to `registry.subscribe()`. On each change it waits 250 ms (dropping changes that arrive in that window), then sets `doc.tabs = registry.saved_tabs()`. It also drops any `view.panes` entries that are no longer open, and saves under the layout mutex in `spawn_blocking`. A save error is logged and never panics.
  - Seeding runs in `AppState::new` before the saver subscribes, so seeding never triggers a write. It is the same path for a relaunch and for a supervisor restart.

- [ ] **Step 1: Write the failing tests**
  - `seed_ended_keeps_order_ids_and_numbers`.
  - `seed_ended_renumbers_a_duplicate_number`.
  - Integration `restart_restores_tabs_as_ended`:
    1. Start a server on a temp home and create two sessions (use the existing test helper in `crates/core/tests/`; find it with `grep -l "create_session\|/v1/sessions" crates/core/tests`).
    2. Poll `layout.json` for up to 2 s until it lists both.
    3. Drop the server and build a new `AppState` on the same home.
    4. `GET /v1/sessions` shows both with `state.kind == "ended"`, in the same order with the same names.
    5. Nothing is spawned, which shows as `sessions` map length 0.
  - `closed_tabs_are_not_saved`: close one tab, and `layout.json` lists only the other.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test layout_restore 2>&1 | tail -5`. Expected: FAIL.
- [ ] **Step 3: Implement `seed_ended`, `saved_tabs` and the saver.**
- [ ] **Step 4: Run** `cargo test -p marshell-core 2>&1 | grep -E "test result|FAILED"`. Expected: all `ok`.
- [ ] **Step 5: Commit** with `feat(core): tabs saved to layout.json come back as Ended`.

### Task 4: Resume in place

**Files:**
- Modify: `crates/core/src/profiles/detect.rs` (beside `agent_command`, and `launch_argv` at ~l.263)
- Modify: `crates/core/src/server/sessions.rs` (new `resume` handler next to `reopen` at ~l.314, reusing `resolve` at l.66 and `start_session` at l.125)
- Modify: `crates/core/src/registry.rs` (`begin_resume`)
- Modify: `crates/core/src/server/mod.rs` (route `POST /v1/sessions/:tab_id/resume`)
- Modify: `src/lib/api/sessions.ts` (`resume(endpoint, tabId, size)`)
- Test: in-file `detect.rs` tests; `crates/core/tests/resume.rs`

**Interfaces:**
- Produces:
  - `pub fn resume_args(agent: &str) -> &'static [&'static str]` returns:
    - `"claude"` → `["--continue"]`;
    - `"codex"` → `["resume", "--last"]`;
    - anything else (`"antigravity"` included, per Ruling 4) → `[]`.
  - `pub fn launch_argv_resume(profile, agent)` is `launch_argv(profile, agent, None)` with `resume_args(agent)` appended after the agent command. It never adds a prompt.
  - `Registry::begin_resume(&self, tab_id) -> Result<Relaunch, ResumeRefusal>`. It succeeds only when the entry exists and its state is `Ended` or `Exited`, and it flips the state to `Running` under the same lock, so a second call gets `ResumeRefusal::AlreadyRunning`. The refusals are `ResumeRefusal { NotFound, AlreadyRunning }`.
  - The `POST /v1/sessions/:tab_id/resume` handler takes a body of `{cols: u16, rows: u16}`. It returns 200 with `CreateSessionResponse`-shaped JSON, or one of these:

    | Status | When | Body (`ResumeError.reason`) |
    |---|---|---|
    | 404 | the tab does not exist | — |
    | 409 | it is already running | `"It is already running."` |
    | 409 | its profile is gone | `"Its shell profile was removed."` |
    | 409 | its folder is missing | `"Its folder no longer exists: <cwd>"` |

    It checks the profile and the folder before `begin_resume`. It removes any old pty `Session` under that id from the sessions map, then starts the new one under the **same tab id**. If the start fails, it puts the state back to what it was.
  - TS: `sessions.resume(endpoint: Endpoint, tabId: string, size: {cols: number; rows: number}): Promise<{ ok: true } | { ok: false; reason: string }>`.

- [ ] **Step 1: Write the failing tests**
  - `resume_args_per_agent` covers all three, with `"antigravity"` giving `[]`.
  - `begin_resume_refuses_running`.
  - Integration (`tests/resume.rs`, with the plain test shell profile used by the existing session tests):
    - `resume_restarts_ended_tab_in_place`: the id, number, name and index are unchanged and the state is `running`.
    - `resume_twice_starts_one_process`: send two resumes at once with `tokio::join!`. Exactly one gets 200 and the other gets 409.
    - `resume_refuses_missing_profile`.
    - `resume_refuses_missing_folder`: delete the temp cwd first.
- [ ] **Step 2: Run** `cargo test -p marshell-core resume 2>&1 | tail -5`. Expected: FAIL.
- [ ] **Step 3: Implement the steps as above. Add `resume` to `sessions.ts`.**
- [ ] **Step 4: Run** `cargo test -p marshell-core 2>&1 | grep -E "test result|FAILED"` and clippy. Expected: all ok, clean.
- [ ] **Step 5: Commit** with `feat(core): Resume restarts a tab in place with the CLI's continue flag`.

### Task 5: Shell panel session in the core

**Files:**
- Create: `crates/core/src/server/shell_panel.rs`
- Modify: `crates/core/src/server/mod.rs` (routes; `shell_panel: Mutex<Option<String>>` in `Inner`)
- Modify: `crates/core/src/registry.rs` (`adopt`)
- Modify: `src/lib/api/sessions.ts` (`shellPanel`, `adoptShellPanel`)
- Test: `crates/core/tests/shell_panel.rs`

**Interfaces:**
- Produces:
  - `POST /v1/shell-panel` takes a body of `{cwd: String, cols, rows}` and returns `{tab_id}`.
    - It starts the **default** shell profile with no agent, using the existing pty start path but **without** `registry.reserve`. It stores the id in `shell_panel`.
    - If a panel shell is alive, it returns that id instead. The pty WebSocket route finds it through the shared sessions map, so `TerminalView` works unchanged.
  - `POST /v1/shell-panel/adopt` takes no body.
    - It calls `Registry::adopt(&self, tab_id, Relaunch { profile_id: default, agent: None, cwd, name })`, which reserves an entry for an already-running pty and returns its number. The name is the profile's name.
    - It then clears `shell_panel` and returns `{tab_id}`. It returns 404 when there is no panel shell.
  - TS:
    - `sessions.shellPanel(endpoint, cwd, size): Promise<string>` returns the tab id.
    - `sessions.adoptShellPanel(endpoint): Promise<string>`.

- [ ] **Step 1: Write the failing tests**
  - `panel_shell_is_not_listed`: after `POST /v1/shell-panel`, `GET /v1/sessions` has no entry with that id.
  - `panel_shell_is_reused`: a second POST returns the same id.
  - `adopt_lists_it_and_frees_the_panel`: the id appears in `/v1/sessions` with state `running`, and the next `POST /v1/shell-panel` returns a new id.
  - `panel_shell_is_not_saved_to_layout`: poll for 500 ms, and `layout.json` `tabs` does not contain it.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test shell_panel 2>&1 | tail -5`. Expected: FAIL.
- [ ] **Step 3: Implement as above.**
- [ ] **Step 4: Run** `cargo test -p marshell-core 2>&1 | grep -E "test result|FAILED"` and clippy. Expected: ok, clean.
- [ ] **Step 5: Commit** with `feat(core): a shell panel pty kept out of the tab list`.

### Task 6: View state reducer and layout persistence (frontend)

**Files:**
- Create: `src/features/window/viewState.ts`, `src/features/window/viewState.test.ts`
- Create: `src/lib/api/layout.ts`
- Create: `src/features/window/useViewState.ts` (hook glue only, kept untested like `useSessions`)

**Interfaces:**
- Consumes: the generated `ViewLayout` and `LayoutDoc` (Task 1); `PaneCount`, `Arrangement` and `ARRANGEMENTS` from `splitLayout.ts`; `SIDEBAR` and `SidebarPref` from `layout.ts`.
- Produces:
  - `type ViewState = ViewLayout & { lastPref?: SidebarPref }`. `lastPref` is the pref to return to when focus mode ends; `toSaved` strips it.
  - `type ViewAction` is one of:
    - `{type: "split"}`: append `null` while `panes.length < 4`, and make the new pane active.
    - `{type: "remove", index}`: never removes the last pane.
    - `{type: "assign", index, tabId}`: if the tab is already in another pane, the two panes swap.
    - `{type: "select", tabId}`: if the tab is on screen, that pane becomes active. Otherwise it is assigned to the active pane.
    - `{type: "focusPane", index}`.
    - `{type: "nextPane"}`: wraps, and only over panes that are both shown and non-empty.
    - `{type: "zoom"}`: toggles on `active`.
    - `{type: "count", count: PaneCount}`: trims from the end or appends `null`s. It clears `zoomed`.
    - `{type: "arrange", arrangement: Arrangement | null}`, where `null` means automatic.
    - `{type: "ratios", ratios}`.
    - `{type: "sessions", openIds: string[]}`: an id no longer open becomes `null` in place, and the other panes and `active` are unchanged.
    - `{type: "sidebar", pref: SidebarPref}`.
    - `{type: "sidebarWidth", width}`: clamped to 200–400. `{type: "resetSidebarWidth"}` sets 288.
    - `{type: "toggleFocus"}`: toggles between `"focus"` and `lastPref` (default `"expanded"`).
    - `{type: "toggleShell"}`.
    - `{type: "toggleGrouped"}`.
  - `function viewReducer(s: ViewState, a: ViewAction): ViewState`.
  - `function toSaved(s: ViewState): ViewLayout`.
  - `layout.get(endpoint): Promise<LayoutDoc>` and `layout.putView(endpoint, view: ViewLayout): Promise<void>`.
  - `useViewState(endpoint, openIds): [ViewState, Dispatch<ViewAction>, LayoutDoc | null]`. It does a GET once on mount and starts from `doc.view`. `LiveApp` renders `MainWindow` once both the session list and this GET have answered (both are local), so the restored layout is the first frame and no default layout flashes first. This is the PLAN :385 cold-start path (≤ 800 ms to the layout skeleton); check it in the manual checklist. It dispatches `sessions` whenever `openIds` changes, and debounces `putView` by 300 ms. It never PUTs before the GET has returned, so it never saves defaults over the user's file.

- [ ] **Step 1: Write the failing tests** (`viewState.test.ts`; each is one `it` with exact assertions):
  - `"split adds an empty pane up to four"`: four `split` actions on `[a]` give `[a,null,null,null]`, and a fifth leaves it unchanged.
  - `"a closed session leaves an empty pane in place"`: on `[a,b,c]` with active 2, `sessions(["a","c"])` gives `[a,null,c]` and active stays 2.
  - `"shrinking keeps every assignment"`: `count` is never dispatched by a resize, so state `[a,b,c]` is unchanged. Assert there is no `resize` action type, so the visible-count logic stays in `MainWindow`.
  - `"select on screen focuses that pane"`; `"select off screen fills the active pane"`.
  - `"assign swaps when the session is already shown"`.
  - `"sidebar width clamps to 200..400 and resets to 288"`.
  - `"focus toggles back to the previous sidebar pref"`: `rail` → focus → `rail`.
  - `"toSaved drops lastPref"`.
- [ ] **Step 2: Run** `pnpm test -- viewState 2>&1 | tail -5`. Expected: FAIL (module not found).
- [ ] **Step 3: Implement `viewState.ts`, `layout.ts` and `useViewState.ts`.**
- [ ] **Step 4: Run** `pnpm test -- viewState`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(window): view state reducer saved to layout.json`.

### Task 7: Live split view

**Files:**
- Modify: `src/features/window/MainWindow.tsx`
  - New prop `paneIds?: (string | null)[]`. When it is set, it replaces the `selected`-first ordering at ~l.165. `shown` is then `paneIds.slice(0, visible)` mapped to rows, and `null` stays `null`.
  - New callbacks: `onLayout?(count: PaneCount, arrangement: Arrangement | null)`, `onPane?(action: ViewAction)`, `onLayoutMenu?(open: boolean)`.
- Modify: `src/features/window/LayoutMenu.tsx`: `onCount?(n)`, `onArrange?(a | null)`. Keep the radios' roles. Arrow keys move inside each radiogroup.
- Modify: `src/features/window/SplitView.tsx` and `Pane.tsx`:
  - the pane header's zoom and remove buttons call `onZoom` / `onRemove`;
  - a click anywhere in a pane calls `onFocus(index)`;
  - `EmptyPane` choices call `onPick(tabId)`;
  - divider pointer drag calls `onRatios(ratios)`, clamped so every pane keeps `MIN_USABLE` (use `paneCells`).
- Modify: `src/lib/appShortcuts.ts`: new kinds `split`, `zoom`, `nextPane` and `nonePane`, matched for `split` (KeyD), `zoom-pane` (KeyP), `nav-next-pane` and `nav-none`.
- Modify: `src/features/sessions/LivePalette.tsx` (`LIVE_COMMANDS` gains `split`, `unsplit`, `layout`, `zoom-pane`, `nav-next-pane` and `nav-none`).
- Modify: `src/features/sessions/LiveApp.tsx`: use `useViewState`, pass `layout={view.panes.length > 1 ? "split" : "default"}` and the pane props through, and route the new shortcut kinds to `dispatch`. `select` from the sidebar also dispatches `{type: "select"}`.
- Test: `MainWindow.test.tsx`, `LayoutMenu.test.tsx`, `SplitView.test.tsx`, `appShortcuts.test.ts`

**Interfaces:**
- Consumes: `ViewAction` and `viewReducer` (Task 6); `maxPanes`, `fitsPanes`, `chooseArrangement` and `paneCells` (existing).
- Produces: `MainWindowProps.paneIds`, `onLayout`, `onPane` and `onLayoutMenu`; `SessionShortcut` kinds `"split" | "zoom" | "nextPane" | "nonePane"`.

- [ ] **Step 1: Write the failing tests**
  - `MainWindow` `"paneIds beyond the fit are hidden, not dropped"`: render at 760×520 with `paneIds` of four rows. The markup shows fewer than four `.pane` elements, contains `"Showing"`, and the focused header contains `"more running"`.
  - `MainWindow` `"paneIds null renders the empty pane with the other sessions"`.
  - `MainWindow` `"window title follows the focused pane"`: assert `windowTitle(row)` output is in the markup's announcer, or export the computed title through `data-title` on `.window`.
  - `LayoutMenu` `"counts above max are disabled"`: `max: 2` gives the radios 3 and 4 `aria-disabled="true"`.
  - `appShortcuts` `"Ctrl+Shift+D is split and Ctrl+Shift+P is zoom on Windows"`, plus the nav chords using `DEFAULT_NAV_MODIFIER`.
- [ ] **Step 2: Run** `pnpm test -- MainWindow LayoutMenu SplitView appShortcuts 2>&1 | tail -8`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement the props, the handlers and the `LiveApp` wiring.**
  - `nonePane` blurs the active terminal and focuses the `.window__main` element (give it `tabIndex={-1}`).
  - The existing prototype stories and tests that do not pass `paneIds` must behave exactly as before.
- [ ] **Step 4: Run** `pnpm test 2>&1 | tail -4` and `pnpm build 2>&1 | tail -3`. Expected: all pass, build clean.
- [ ] **Step 5: Commit** with `feat(window): live split view up to four panes`.

### Task 8: Focus mode and the resizable sidebar

**Files:**
- Modify: `src/lib/keymap.ts` (`focus-mode` gets `binding: { code: "KeyI" }`, per Ruling 2)
- Modify: `src/lib/appShortcuts.ts` (kind `focusMode`)
- Modify: `src/features/window/MainWindow.tsx`:
  - It honours `sidebarPref` and a new `sidebarWidth` prop, passing `requested` into `sidebarWidth(mode, requested)`.
  - A new `onSidebarWidth?(w)` and `onSidebarReset?()` are fired by a 8 px-wide drag handle on the sidebar's end edge. The handle is a `role="separator"` with `aria-orientation="vertical"`, `aria-valuemin=200`, `aria-valuemax=400` and `aria-valuenow`. Left and Right arrow keys step it by 8, and a double-click resets it.
  - The header reveals on hover over an 8 px top-edge strip in focus mode (the `headerPeek` state lives in `MainWindow`).
- Modify: `src/features/window/MainWindow.css` (tokens only)
- Modify: `src/features/sessions/LiveApp.tsx` and `LivePalette.tsx` (`focus-mode` and `sidebar` commands)
- Test: `MainWindow.test.tsx`, `appShortcuts.test.ts`, `keymap.test.ts` (if it exists; otherwise add to `appShortcuts.test.ts`)

**Interfaces:**
- Consumes: `toggleFocus`, `sidebar`, `sidebarWidth` and `resetSidebarWidth` actions (Task 6).
- Produces: `MainWindowProps.sidebarWidth`, `onSidebarWidth` and `onSidebarReset`; the shortcut kind `"focusMode"`.

- [ ] **Step 1: Write the failing tests**
  - `"focus mode renders no sidebar, no rail and no prompt line"`, at 1280×800 with `sidebarPref: "focus"`.
  - `"the sidebar handle reports its width"`: `sidebarWidth: 320` gives `aria-valuenow="320"` and `--side-w: 320px`.
  - `"below 960 the stored width does not apply"`: at 900 wide, the rail is 52.
  - `"Ctrl+Shift+I is focus mode and no binding uses U E C V Z J"`: loop over `COMMANDS`.
- [ ] **Step 2: Run** `pnpm test -- MainWindow appShortcuts 2>&1 | tail -6`. Expected: FAIL.
- [ ] **Step 3: Implement as above.**
- [ ] **Step 4: Run** `pnpm test 2>&1 | tail -4`. Expected: all pass.
- [ ] **Step 5: Commit** with `feat(window): focus mode and a resizable sidebar`.

### Task 9: Ended and Exited tabs with Resume

**Files:**
- Create: `src/features/sessions/ResumeBar.tsx`, `ResumeBar.css`, `ResumeBar.test.tsx`
- Create: `src/features/sessions/resumeCommand.ts` (+ test). This is the TS mirror of the command line that Resume will run, for display only.
- Modify: `src/features/sessions/LiveApp.tsx`:
  - `renderTerminal` renders `ResumeBar` alone for an `ended` row;
  - for an `exited` row it renders `TerminalView` and then `ResumeBar`;
  - a `resume` palette command ("Resume this session").
- Modify: `src/features/sessions/sessionActions.ts` (`resume(tabId)`). It calls `sessions.resume` with the last measured size. On success it shows the toast "Resumed"; on 409 it announces the reason through the notice.
- Modify: `src/lib/keymap.ts` (`{ id: "resume", label: "Resume this session", group: "Sessions" }`, unbound)
- Test: `ResumeBar.test.tsx`, `resumeCommand.test.ts`, `sessionActions.test.ts`

**Interfaces:**
- Consumes: `sessions.resume` (Task 4) and the `ended` row phrase (Task 1).
- Produces:
  - `ResumeBar({ row, command, disabledReason?, onResume })`. It shows:
    - `"Ended last time. Nothing is running."` for ended, or `"Exited with code N."` for exited;
    - the exact command and folder (`"Resume runs claude --continue in C:\dev\billing"`);
    - one `"Resume"` button.

    The button carries `aria-describedby` pointing at the command line. With `disabledReason` set it is `aria-disabled="true"` and the reason shows under it.
  - `resumeCommand(agent: string | null, profileName: string): string` gives `"claude --continue"`, `"codex resume --last"`, `"agy"`, or the profile name for a plain shell. It is copied from `resume_args`, with a comment pointing at `detect.rs`.
  - `SessionActions.resume(tabId: string): Promise<void>`.
  - `disabledReason` is computed in `LiveApp`. If the row's `profile_id` is not in `profiles`, it is `"Its shell profile was removed."`. A folder check needs the core, so a missing folder shows only after a 409.

- [ ] **Step 1: Write the failing tests**
  - `ResumeBar` `"ended shows the command it will run"`.
  - `ResumeBar` `"a removed profile disables Resume and says why"`.
  - `resumeCommand` cases for all three agents and a plain shell.
  - `sessionActions` `"a refused resume announces the reason"`, using the existing fake-api pattern in `sessionActions.test.ts`.
- [ ] **Step 2: Run** `pnpm test -- ResumeBar resumeCommand sessionActions 2>&1 | tail -6`. Expected: FAIL.
- [ ] **Step 3: Implement as above.** Write the copy exactly as given. The button keeps focus after a refusal.
- [ ] **Step 4: Run** `pnpm test 2>&1 | tail -4`. Expected: all pass.
- [ ] **Step 5: Commit** with `feat(sessions): Ended and Exited tabs offer Resume`.

### Task 10: Live sidebar shell panel

**Files:**
- Modify: `src/features/window/ShellPanel.tsx`:
  - new optional props `terminal?: ReactNode` (rendered instead of `ScriptedTerminal` when set), `onToggle?()` and `onOpenInPane?()`;
  - the existing buttons get their `onClick`.
- Modify: `src/features/window/Sidebar.tsx` (`SidebarShell` passes them through)
- Modify: `src/features/window/MainWindow.tsx`: the `shell` prop may be `{ terminal, cwd, hidden, onToggle, onOpenInPane }`. When the user has hidden the panel, `shellLayout`'s `collapsedPref` is set to `hidden`, so their choice wins.
- Modify: `src/lib/appShortcuts.ts` (kind `shellPanel`, KeyS)
- Modify: `src/features/sessions/LiveApp.tsx`:
  - on first show, call `sessions.shellPanel(endpoint, focusedRow.cwd, size)` and render a `TerminalView` for that id with no prompt jumps;
  - Ctrl+Shift+S: if the panel is hidden, show it; if it is shown and not focused, focus its terminal; if it is focused, do nothing;
  - in the rail, focus mode or below 960 px it does nothing, and the announcer says `"The shell panel is in the expanded sidebar."`;
  - "Open in a pane" calls `adoptShellPanel`, then dispatches `split` and `assign` with the returned id.
- Test: `ShellPanel.test.tsx`, `MainWindow.test.tsx`, `appShortcuts.test.ts`

**Interfaces:**
- Consumes: `sessions.shellPanel` and `adoptShellPanel` (Task 5); `toggleShell`, `split` and `assign` (Task 6).
- Produces: `ShellPanelProps.terminal`, `onToggle` and `onOpenInPane`.

- [ ] **Step 1: Write the failing tests**
  - `ShellPanel` `"a live terminal replaces the scripted one"`: pass `terminal: <div data-testid="live"/>` and expect it in the markup, with no `.scripted-terminal`. Check the scripted class name in `ScriptedTerminal.tsx`.
  - `MainWindow` `"a hidden shell panel stays folded even when there is room"`.
  - `MainWindow` `"no shell panel in the rail or focus mode"`.
  - `appShortcuts` `"Ctrl+Shift+S is the shell panel"`.
- [ ] **Step 2: Run** `pnpm test -- ShellPanel MainWindow appShortcuts 2>&1 | tail -6`. Expected: FAIL.
- [ ] **Step 3: Implement as above.** The panel never shows a vendor stripe or a prompt line.
- [ ] **Step 4: Run** `pnpm test 2>&1 | tail -4`. Expected: all pass.
- [ ] **Step 5: Commit** with `feat(window): a live shell panel in the sidebar`.

### Task 11: Project grouping toggle

**Files:**
- Create: `src/features/sidebar/grouping.ts` (+ `grouping.test.ts`)
- Modify: `src/features/window/Sidebar.tsx`:
  - a new prop `grouped?: boolean`;
  - when it is on, each group renders a non-focusable heading (`role="presentation"` text in the list, with the project name and the count) above its rows;
  - drag and Alt+Shift+arrow are limited to the group (Ruling 10).
- Modify: `src/features/sidebar/reorder.ts`. `moveBy` and `dropOrder` take an optional `bounds: {start, end}`.
- Modify: `src/lib/keymap.ts` (`{ id: "group-by-project", label: "Group sessions by project", group: "View" }`, unbound) and `LivePalette.tsx` (live; the label says `"Stop grouping by project"` while it is on)
- Modify: `src/features/sessions/LiveApp.tsx` (pass `grouped`, dispatch `toggleGrouped`)
- Test: `grouping.test.ts`, `reorder.test.ts`, `Sidebar.test.tsx`

**Interfaces:**
- Consumes: `toggleGrouped` (Task 6) and `RowModel.project`.
- Produces: `type Group = { project: string; rows: RowModel[]; start: number }` and `groupRows(rows: RowModel[]): Group[]`. Groups are ordered by their first row's position, and rows keep the core's order inside a group.

- [ ] **Step 1: Write the failing tests**
  - `"groups follow first appearance and keep core order"`.
  - `"moveBy stops at the group edge"`.
  - `Sidebar` `"grouped shows one heading per project with its count"`.
  - `Sidebar` `"ungrouped renders no headings"`. This is the default.
- [ ] **Step 2: Run** `pnpm test -- grouping reorder Sidebar 2>&1 | tail -6`. Expected: FAIL.
- [ ] **Step 3: Implement as above.** The reorder POST still sends the full id list.
- [ ] **Step 4: Run** `pnpm test 2>&1 | tail -4`. Expected: all pass.
- [ ] **Step 5: Commit** with `feat(sidebar): group sessions by project`.

### Task 12: Last launcher choice per project

**Files:**
- Modify: `crates/core/src/server/sessions.rs`. In `create`, after a successful start, it records `doc.launcher[cwd] = LauncherChoice { profile_id, agent }` under the layout mutex and saves. Only the 32 most recent folders are kept; track insertion order with a `Vec` beside the map, or drop the oldest by re-inserting.
- Modify: `src/features/sessions/LiveLauncher.tsx` and `launchPrograms.ts`. When the project changes, the program is preselected from `doc.launcher[project.path]` if that profile still exists. The user's own pick in this open launcher always wins.
- Test: `crates/core/tests/layout_api.rs` (`create_records_launcher_choice`); `launchPrograms.test.ts` (`"remembered choice preselects only when its profile exists"`)

**Interfaces:**
- Consumes: `LayoutDoc.launcher` (Task 1) and `layout.get` (Task 6).
- Produces: `preselect(doc: LayoutDoc | null, projectPath: string, programs: LaunchProgram[]): ProgramId | undefined` in `launchPrograms.ts`.

- [ ] **Step 1: Write the two failing tests.**
- [ ] **Step 2: Run** `cargo test -p marshell-core --test layout_api 2>&1 | tail -4` and `pnpm test -- launchPrograms`. Expected: FAIL.
- [ ] **Step 3: Implement as above.**
- [ ] **Step 4: Run both again.** Expected: PASS.
- [ ] **Step 5: Commit** with `feat(launcher): remember the last choice per project`.

### Task 13: Docs

**Files:**
- Modify: `docs/PLAN.md` §9 (the 2e slice row ~:753, the projects bullet ~:761, the core-restart bullet ~:762, the v2 list :862)
- Modify: `docs/BRIEF.md` (Amendments)
- Modify: the PLAN file map, adding `layout.rs`, `server/layout.rs`, `server/shell_panel.rs`, `viewState.ts`, `grouping.ts` and `ResumeBar.tsx`

- [ ] **Step 1: Edit PLAN §9.**
  - The 2e row becomes: split up to four panes, focus mode, the sidebar shell panel, project grouping, `layout.json` restore, and Ended tabs with Resume.
  - Add **2e+**: projects and task worktrees (create, review the diff, merge back, remove; every git command shown first).
  - Move the :761 bullet under 2e+.
  - Rewrite :762: a core restart or a relaunch brings the tabs back as Ended with Resume, and Resume runs the CLI's own continue-last-here (claude `--continue`, codex `resume --last`), while exact-id resume is phase 3.
  - Strike "more than two panes" from :862.
  - Record Rulings 1–10 as the 2e decisions of 2026-10-16.
- [ ] **Step 2: Add a BRIEF amendment** dated 2026-10-16 that says the same in three or four lines.
- [ ] **Step 3: Run** `grep -n "more than two panes" docs/PLAN.md`. Expected: no output.
- [ ] **Step 4: Commit** with `docs: slice 2e layout, restore and Resume in PLAN`.
