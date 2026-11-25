# Slice 6v-2: the side pane for the shell panel, with no session too. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The sidebar's shell panel owns a side-pane tab set of its own. When focus is in the panel, the pane shows the panel's tabs. With no session open, the pane fills the area right of the sidebar and shows them too. "Open as a session" takes those tabs along, and their browser pages stay live.

**Architecture:**
- **Owner.** The pane gets an owner: a session id, or the reserved key `"shell"`. A pure function (`paneOwner`) picks it from three things: where focus last was (the shell panel or a session), whether the panel is on screen, and whether any session exists.
- **State and server calls.** Pane state is keyed by the owner's key. Server calls, and browser page keys, use the panel shell's real tab id.
- **Handover.** Adopting keeps that id (`shell_panel::take_and_adopt`). So moving the set from `sessions["shell"]` to `sessions[<id>]` leaves every page key unchanged, and nothing under `src-tauri/` changes.
- **Core.** `row_of` answers for the panel's shell with a row built from the panel's record, so Files, Changes, GitHub and resolve work for it. `opens::open` and the CLI's `marshell open` learn the panel too.
- **Always there.** The panel's shell is asked for as soon as the window is ready, shown or not. When it ends, it is asked for again.

**Tech Stack:** Rust (axum, tokio, serde), React and TypeScript, Vitest. No new dependency. No protocol type changes, so `src/generated/` is not regenerated.

**Spec:** `docs/PLAN.md` §9, "Slice 6v decisions (2026-11-25)":
- Decisions 1, 2, 3, 7, 8 and 14 to 21. Decision 14 replaces decision 10's mechanism and keeps its aim.
- Decision 22 is what this plan settles without asking. It is recorded in §9 with this plan and repeated below; Task 8 brings it up to date with any ruling made while the tasks ran.

**Decision 22 (in `docs/PLAN.md` §9): Decided, not asked (6v-2).**
- **The saved key.** The shell's tab set is saved in `layout.json` under the reserved key `"shell"`.
  - The panel's shell gets a new tab id at every start, so its id can't be the key.
  - A real tab id is `t` and 12 hex digits (`auth::new_id`), so no session can be named `"shell"`.
- **The "no agent" sentence shows for the shell panel only:** "The shell panel runs no agent, so Review is off and comments are copied, not sent." A plain-shell session's Changes is as before.
- **The handover never merges two sets.**
  - Into a set that already has tabs, the handover does nothing. Pane tab ids are per set (`nextId`), so a merge would renumber tabs and reload their pages.
  - A target that holds only a tree (or only drafts) keeps them and takes the shell's tabs.
- **The fill is its own mode, `"fill"`, not `"cover"`.** Cover's Esc hides the pane and cover suspends the terminals; a filling pane has no terminal under it.
- **History.** With History on screen the pane stays off, as today. The blocked line becomes "Close History to show the side pane." The old "The side pane shows beside a session." is no longer true with no session.
- **The shell panel's 32-tab limit reads** "The shell panel has 32 tabs. Close one to open another." in the app. Through `marshell open` it reads "The shell panel already has 32 tabs in its side pane."
- **The pane's copy says "folder", never "session" or "directory",** where the owner can be the shell (decision 21; the copy is in Task 7).
- **A panel folded for lack of room counts as on screen:** its header is drawn.
- **When the shell is asked for.** As soon as the window has its endpoint and its saved view, whether the panel is shown or not (decision 19 read this way). Hiding and showing the panel no longer starts or retries anything.
- **A shell that ends while the sidebar is collapsed.** The window learns that the panel's shell ended from the panel's terminal, which is drawn only while the sidebar is expanded. A shell that ends while the sidebar is a rail or off screen is replaced once the sidebar is expanded again, and the pane goes on listing its folder meanwhile.
- **Restart loop guard.** A panel shell that ends three times within 10 seconds is not started again. The panel shows "The shell didn't start." with Try again. Without this, a profile whose shell exits at once would start shells forever (decision 20 read with this bound).
- **What decides the owner.** Only focus in a session's terminal or in the sidebar outside the panel ("session"), or in the shell panel ("shell"). Focus in the pane, a browser page, the title bar or an overlay leaves the preference as it was.
- **The panel's row is not a session.** It has no number and no agent. Push and pull-request actions in Changes work for the shell's folder, since they are not agent-only (decision 2).
- **Where pane deliveries go.** A region's Add and "Paste path into" from the shell's tabs go to the shell panel. A region goes through the clipboard, as for any terminal with no prompt line.
- **`--session`.** `marshell open --session <the panel's id>` typed outside the panel is refused as today ("No session …"). Only the panel's own shell reaches its set; `--session` still names sessions (decision 17).

## Global Constraints

- Nothing under `src-tauri/` changes: no Tauri command, nothing in `capabilities/`, `build.rs`, `tauri.conf.json` or `tauri.windows.conf.json` (decision 14). `guard.rs`, token handling, the CSP and `.github/workflows/` are not touched.
- No new dependency.
- No new route. If one turns out to be needed, it sits in the token-guarded router and gets a "refused without the bearer" test.
- Never hold a `MutexGuard` across `.await` or I/O.
  - The panel lock (`st.0.shell_panel`) is never held while the registry is locked, and the registry lock is never held while the panel lock is taken. Each reader takes one, clones, drops it, then takes the other.
  - The one existing nesting, `take_and_adopt` calling `registry.adopt` under the panel lock, stays as it is. Everything new reads the panel first, so it sees either the panel or the row.
  - Disk work (`projects::project_of`) runs in `spawn_blocking`.
- No route takes a file path to write (`.claude/rules/local-api.md`).
- Tests and live checks never touch the real `%USERPROFILE%\Marshell`, `~/.marshell`, `~/.claude` or other CLI homes. Core tests use temp folders and `.with_default_folder(…)`; the live check uses the scratch homes of `marshell-live-check`.
- Every UI change works with the keyboard alone and in forced colours. A new style declaration takes colours from tokens only; read the `good-css` skill first. The `forced-colors` rule uses system colours only.
- Vitest runs in node with no DOM. Components are tested through `renderToStaticMarkup`, logic through pure functions. Focus logic takes `Pick<Element, "closest">` so tests can pass plain objects.
- Copy is fixed by this plan, word for word. No dash as punctuation in UI copy. "Folder", never "directory".
- Vendor-neutral: nothing here names a CLI. The shell panel's shell is a plain shell.
- Each task: write the test first and watch it fail. Run `cargo fmt --all` before every Rust commit. Use `cargo test … -j 4`, never more jobs.
- Commit messages: `type(scope): what changed, as a sentence.`
- Workers: stay inside the task's files; never `git checkout`, `git restore` or `git stash` anything; scratch files go under the system temp folder, in a folder of the task's own.
- Verified before writing:
  - `rows` and `infos` change in the same render. `rows` is a `useMemo` over `state.infos` (`src/features/sessions/useSessions.ts:184`).
  - The opens test file is `crates/core/tests/open_api.rs`.
  - The core replaces an exited panel shell on the next `POST /v1/shell-panel` (`alive_panel` at `shell_panel.rs:43–53`; `open` at 156–158; test `a_panel_shell_that_exited_is_replaced`, `crates/core/tests/shell_panel.rs:193`).
  - The panel's shell gets `MARSHELL_TAB_ID` set to its own id (`sessions.rs:721`, inside `spawn_into_map`).
- Items marked "not verified" below are for the worker to check first, and to report if the code says otherwise.

## Review Focus

1. **The handover keeps every page key in one render.** The render that sets `panel` to null must also hold `sessions[<id>]` and list `<id>` in `rows`. If not, `setLoadable` unloads the page, and the half-filled form of decision 10 is lost with no error. A late `FilesPanel.onSaved` for the same id must not make a tabless target first. (Task 4 `a handover keeps every key`; Task 3 `handOver …` tests; the wiring is live-check item 9.)
2. **Focus decides the owner, so focus returns must not flip it.** Any `actions.focusTerminal()` left on the shell's path flips the pane to the session mid-action, and the next dispatch goes to the wrong set. That covers Esc, hiding a panel, the pane's toggle, Ctrl+Shift+Tab, `onTyped`, a region's Add and "Paste path into". So does a programmatic focus into the pane that changes the preference. (Task 3 `focusOwnerOf …` and `leavePaneOutcome …`; Task 7 wiring; live-check items 3 and 5.)
3. **The panel's shell ending.** `exit` must give a fresh shell at once and keep the shell's tabs. A shell that exits at once must not start shells forever. The ask effect must not ask while the panel shows its failure. (Task 6 `shellRespawn …`, `shellPanelAsk …`; live-check item 12.)
4. **The core answering for a shell with no row.** `row_of` reads the panel first and the registry after. Neither lock is held while the other is taken. A request racing "Open as a session" sees the panel or the row, never neither. `opens::open` words its refusals for the panel. The CLI posts to its own tab when the list doesn't hold it, and still refuses a typed `--session` it can't find. (Task 1 `an_adopted_panel_resolves_as_its_row`; Task 2 `an_open_for_the_panel_at_its_limit_names_the_panel`, `open_in_the_panel_shell_posts_to_its_own_tab`, `a_typed_session_that_isnt_listed_is_still_refused`.)
5. **The fill mode and the page that follows it.** With no session, an open pane fills the room in every window width, cover's width included. Closing it brings the empty state back. A browser page in a filling pane follows a window resize (`paneColumn` gets `alone`), and the title-bar buttons are no longer disabled with no session. (Task 5 `alone, the pane fills the room`, `a filling pane is the whole room`, MainWindow tests; live-check items 1, 2 and 13.)

---

### Task 1: The core: the panel's row, `row_of`, and the saved `"shell"` key

Worker, `effort: high` (locks, local API).

**Files:**
- Modify: `crates/core/src/layout.rs` (`repair_pane` at about line 525 and its tests; a new constant)
- Modify: `crates/core/src/server/shell_panel.rs` (`PanelShell`, `open`, a new `AppState::panel_row`)
- Modify: `crates/core/src/server/files.rs` (`row_of` at line 357)
- Test: `crates/core/tests/shell_panel.rs` (reuse its `state_with`, `open_panel_as`, `adopt` and `api` helpers)

**Interfaces:**
- Produces:
  ```rust
  // crates/core/src/layout.rs
  /// The pane's key for the shell panel's tab set. Never a tab id: those are `t` and 12 hex digits.
  pub const SHELL_OWNER: &str = "shell";

  // crates/core/src/server/shell_panel.rs
  pub(super) struct PanelShell { tab_id, profile_id, name, cwd, /* new: */ project: String }
  impl AppState {
      /// A row for the panel's shell while `tab` is its id: no number, no agent, Running, the folder it started in.
      /// Locks the panel only to clone the record; builds the row after the lock is dropped.
      pub(super) fn panel_row(&self, tab: &str) -> Option<SessionInfo>;
  }

  // crates/core/src/server/files.rs
  /// The session's row, or the shell panel's while `tab` is its shell. The panel is read first: `take_and_adopt`
  /// adds the row while it holds the panel, so a reader that finds the panel empty finds the row.
  pub(super) fn row_of(st: &AppState, tab: &str) -> Option<SessionInfo>
  ```
- `repair_pane` keeps the set keyed `SHELL_OWNER` along with every set whose key is a saved tab. `validate_pane` already takes any key (`layout.rs:744`), so it is not changed; a test guards that.
- `panel_row` builds the `SessionInfo` field for field as `Registry::adopt_script` does (`registry.rs:328`). It uses `number: 0`, `agent: None`, `mode: None`, `preset: None`, `state: SessionState::Running`, `command: CommandState::Unknown`, `cwd` and `project` from the record, `name` the profile's name, `profile_id` the profile's id, and every other field empty or false. An exited shell still answers: its folder is still there, and the next `open` replaces it.
- `open` computes `project` with `tokio::task::spawn_blocking(move || crate::projects::project_of(&cwd))` before the record is stored. A join error gives the `cwd` itself.
- Every route that calls `row_of` now answers for the panel's shell: files, tree, search, changes, review, comments and every `github` route. `review`, `comments` and `github/ask` answer 409 `no_agent` through `agent_ready` (`changes.rs:241`). No route file other than `files.rs` changes.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `layout.rs` unit tests (next to the `repair` tests at about line 1276):
    - `repair_keeps_the_shell_panels_tab_set`: a doc with saved tab `ta` and `pane.sessions` holding `ta`, `shell` and `gone`, each with one file tab. After `repair`, the keys are `["shell", "ta"]` and the shell's tab is unchanged.
    - `repair_cleans_the_shell_set_like_any_other`: a `shell` set with a tab of kind `"weird"` and two tabs with the same id keeps one tab.
    - `validate_takes_the_shell_key`: `validate_pane` of a pane whose only set is `shell` is `Ok(())`.
  - `crates/core/tests/shell_panel.rs` (state built with a temp default folder, which the test makes and writes `a.txt` into):
    - `the_panel_shell_lists_its_folders_files`: open the panel with no `cwd`. `GET /v1/sessions/{tab}/files` answers 200; its `root` is the default folder and its `files` hold `a.txt`.
    - `the_panel_shell_has_a_tree`: `POST /v1/sessions/{tab}/tree` with `{"dirs":[{"dir":"","stamp":null}]}` (as `crates/core/tests/github_api.rs:359`) answers 200.
    - `the_panel_shell_has_changes`: `POST /v1/sessions/{tab}/changes` with `{"stamp": null, "hide_whitespace": false}` (as `crates/core/tests/changes_api.rs:264`; `hide_whitespace` has no default, so `{}` is refused with 422) answers 200, and its `kind` is `"no_repo"`.
    - `the_panel_shell_takes_no_review`: `POST /v1/sessions/{tab}/review` with `{"base":null}` answers 409 with reason `no_agent`.
    - `the_panel_shell_takes_no_comments`: `POST /v1/sessions/{tab}/comments` with `{"text":"x"}` answers 409 with reason `no_agent`.
    - `an_adopted_panel_resolves_as_its_row`: open, adopt, then `files` answers 200. `GET /v1/sessions` lists the tab with `number >= 1`.
    - `a_tab_that_is_neither_is_not_found`: `GET /v1/sessions/tnope/files` answers 404.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --lib repair_keeps_the_shell_panels_tab_set` and `cargo test -p marshell-core -j 4 --test shell_panel the_panel_shell`. Expected: FAIL (the `shell` key is dropped; the panel's routes answer 404).
- [ ] **Step 3: Implement** the constant, the `repair_pane` retain, `PanelShell.project`, `panel_row` and `row_of` as the Interfaces say.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 --test shell_panel --test files --test changes_api --test github_api --test layout_api --test layout_restore` and `cargo test -p marshell-core -j 4 --lib layout`. Expected: PASS. Run `cargo clippy --workspace --all-targets -j 4 -- -D warnings`. Expected: clean.
- [ ] **Step 5: Commit** `feat(core): the shell panel's shell answers the pane's routes with a row of its own, and its saved tab set survives a restart.`

---

### Task 2: `marshell open` from the shell panel

Worker, `effort: high` (local API, CLI).

**Files:**
- Modify: `crates/core/src/server/opens.rs` (`open` at line 216; its re-check at about line 237)
- Modify: `crates/cli/src/open.rs` (`run` at line 107, and its tests)
- Test: `crates/core/tests/open_api.rs` (reuse `state`, `api`, `open`, `fake_app`, `waiting`; add two helpers shaped like the ones in `crates/core/tests/shell_panel.rs`: `open_panel(app, dir) -> String`, which POSTs `/v1/shell-panel` with `{"cols":80,"rows":24,"cwd": dir}`, and `adopt(app)`, which POSTs `/v1/shell-panel/adopt`. `dir` is a temp folder the test makes: this file's `state` (lines 20 to 40) sets no default folder and its profile has no `cwd`, so a panel asked for no folder would start in the real home folder)

**Interfaces:**
- Consumes: Task 1's `AppState::panel_row`.
- Produces, in `opens.rs`:
  ```rust
  /// Whose tabs an open is for: a session (its number, for the refusals) or the shell panel.
  enum OpenOwner { Session(u32), Panel }
  /// The panel first, then the registry (the order `row_of` uses).
  fn owner_of(st: &AppState, tab: &str) -> Option<OpenOwner>;
  const PANEL_CLOSED: &str = "The shell panel's shell has closed.";
  fn panel_full() -> String; // format!("The shell panel already has {} tabs in its side pane.", crate::layout::MAX_PANE_TABS)
  ```
  - `open` answers 404 "No such session." when `owner_of` is None, as now.
  - The limit refusal and the closed refusal use the panel's words for `OpenOwner::Panel` and today's words for a session.
  - The re-check after the file check drops the queue when `owner_of` is None.
  - `opener()` is unchanged: it already answers `"shell"` for a tab with no row.
- Produces, in the CLI's `run`:
  - `session` named with `--session` goes through `client::resolve` exactly as now.
  - With no `--session`, when the snapshot doesn't list `MARSHELL_TAB_ID`, the request goes to that id as it is, and the core decides. Its 404 "No such session." becomes "No session {id}." as now.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `open_api.rs`:
    - `an_open_for_the_panel_shell_is_queued`: `fake_app(&st, &app, None)`, then `open(&app, &panel, <a temp file>, None)` answers 204. The fake app's list holds one `PaneOpen` with `tab_id == panel`.
    - `an_open_from_the_panel_is_by_the_shell`: `from: Some(&panel)` gives a `PaneOpen` whose `by == "shell"`.
    - `an_open_for_the_panel_at_its_limit_names_the_panel`: `fake_app(…, Some("limit"))`, then 409 with the body `"The shell panel already has 32 tabs in its side pane."`
    - `an_open_for_a_closed_panel_shell_names_the_panel`: `fake_app(…, Some("session"))`, then 409 with the body `"The shell panel's shell has closed."`
    - `an_open_for_an_adopted_panel_names_its_session`: open the panel, adopt it, `fake_app(…, Some("limit"))`, then 409 with the body `"Session 1 already has 32 tabs in its side pane."`
  - `crates/cli/src/open.rs` tests (the `Fake` client there):
    - `open_in_the_panel_shell_posts_to_its_own_tab`: env `MARSHELL_TAB_ID=t-panel`, the `Fake`'s snapshot, which doesn't list it (`Fake::sessions` always lists `tab-b`, `open.rs:200`), and `Fake` answers `Opened::Done`. The result is 0 and `asked[0].0 == "t-panel"`.
    - `a_typed_session_that_isnt_listed_is_still_refused`: `--session t-panel` with `MARSHELL_TAB_ID=tab-b` (listed) writes `"No session t-panel."`, returns 1, and asks nothing.
    - `a_panel_shell_that_is_gone_reads_no_session`: env `t-panel`, not listed, `Fake` answers `Opened::Refused("No such session.")`. The output is `"No session t-panel."` and the result is 1.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test open_api panel` and `cargo test -p marshell-cli -j 4 open`. Expected: FAIL (404 for the panel; the CLI says "No session t-panel." without asking).
- [ ] **Step 3: Implement** `OpenOwner`, `owner_of`, the two refusals, and the CLI's fallback.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 --test open_api` and `cargo test -p marshell-cli -j 4`. Expected: PASS. Run `cargo clippy --workspace --all-targets -j 4 -- -D warnings`. Expected: clean.
- [ ] **Step 5: Commit** `feat(open): marshell open typed in the shell panel opens in the shell's tabs.`

---

### Task 3: The owner model, pure

Worker (medium).

**Files:**
- Create: `src/features/pane/paneOwner.ts`, `src/features/pane/paneOwner.test.ts`
- Modify: `src/features/pane/paneState.ts` and `paneState.test.ts` (the `handOver` action)
- Modify: `src/features/pane/paneNews.ts` and `paneNews.test.ts` (the `rename` action)
- Modify: `src/features/pane/opens.ts` and `opens.test.ts` (`OpensContext.owner`)
- Modify: `src/features/pane/paneCommands.ts` and `paneCommands.test.ts` (`leavePaneOutcome`, `tabLimitText`, `SHELL_TAB_LIMIT`)
- Modify: `src/features/sessions/LiveApp.tsx` (the two stopgaps of Step 3 only, so `pnpm build` stays clean; Task 7 does the wiring)

**Interfaces:**
- Produces, in `paneOwner.ts`:
  ```ts
  /** The pane's key for the shell panel's tab set (`SHELL_OWNER` in crates/core/src/layout.rs). */
  export const SHELL_OWNER = "shell";
  export type OwnerPref = "shell" | "session";
  /** Whose tabs the pane shows: `key` keys the pane state, `tabId` is the core's tab (null while the panel's shell isn't there). */
  export type PaneOwner = { key: string; tabId: string | null; shell: boolean };
  export const SHELL_PANEL_AREA = ".shell-panel";
  export const PANE_AREA = ".window__pane, .pane__divider";
  export const SESSION_AREA = ".window__main, .window__side";
  /** What focus on `el` says about the owner: the shell panel, a session's terminal or the sidebar, or nothing (null keeps the last). */
  export function focusOwnerOf(el: Pick<Element, "closest"> | null): OwnerPref | null;
  /** No session: the shell. Else the shell while it was focused last and the panel is on screen, else the focused session. */
  export function paneOwner(i: { pref: OwnerPref; focusedId: string | undefined; shellShown: boolean; panelTabId: string | undefined }): PaneOwner;
  /** The pane key a core tab id is under: the shell's while it is the panel's shell, else the id itself. */
  export function ownerKeyOf(tabId: string, panelTabId: string | undefined): string;
  /** A tab's key for pages and the tab maps: the panel shell's id stands in for the shell's key; with no shell, `shell/<tab>`. */
  export function ownerPageKey(key: string, paneTab: string, panelTabId: string | undefined): string;
  /** `set` with `from` replaced by `to` (for the sessions told of the tab limit); the same set when `from` isn't in it. */
  export function renamed(set: ReadonlySet<string>, from: string, to: string): ReadonlySet<string>;
  /** Where the shell's tabs deliver: the panel, through the clipboard (it has no prompt line). The name starts the lines that use it
   *  ("The shell panel has no prompt line, so the region is on the clipboard.", `regions/deliver.ts:35`, `changes/send.ts:68`). */
  export const SHELL_DELIVERY: { name: string; via: "clipboard" } = { name: "The shell panel", via: "clipboard" };
  ```
  - `focusOwnerOf` checks in this order: `PANE_AREA` gives null; `SHELL_PANEL_AREA` gives `"shell"`; `SESSION_AREA` gives `"session"`; anything else gives null. The panel sits inside `.window__side`, so it must be checked before `SESSION_AREA`.
  - `paneOwner`:
    - With `focusedId` undefined: `{ key: SHELL_OWNER, tabId: panelTabId ?? null, shell: true }`.
    - With `pref === "shell" && shellShown`: the same.
    - Else: `{ key: focusedId, tabId: focusedId, shell: false }`.
- Produces, in `paneState.ts`:
  ```ts
  /** "Open as a session": the shell's set becomes the new session's; the shell has none after. */
  | { type: "handOver"; from: string; to: string }
  ```
  - With no `from` set: the same `s`.
  - With a `to` set that has tabs: the same `s`.
  - Else `sessions[to]` takes `from`'s `tabs`, `active` and `active_browser`, with `tree` and `changes` from `to` when it holds them and from `from` otherwise. `sessions[from]` is deleted.
- Produces, in `paneNews.ts`: `| { type: "rename"; from: string; to: string }`. Every `panel:from` becomes `panel:to`; with nothing to move, the same set comes back.
- Produces, in `opens.ts`: `OpensContext.sessions` is replaced by `owner(tabId: string): { key: string; project: string } | null`, and `shown` is the owner key on screen.
  - An open whose `owner` is null is answered `"session"`, as now.
  - The action, `news` and `full` use the owner's key; `openRefused` is checked on the key.
- Produces, in `paneCommands.ts`:
  ```ts
  export const SHELL_TAB_LIMIT = `The shell panel has ${PANE_LIMITS.tabs} tabs. Close one to open another.`;
  export function tabLimitText(key: string): string; // SHELL_OWNER gives SHELL_TAB_LIMIT, any other key TAB_LIMIT
  /** Esc or Ctrl+Shift+Tab in a pane tab. A covering pane hides; a filling pane hides only when the shell panel isn't on screen (decision 16). */
  export function leavePaneOutcome(s: { covers: boolean; alone: boolean; shellShown: boolean }): "hide" | "focus";
  ```

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `paneOwner.test.ts`:
    - `no session gives the shell`:
      ```ts
      expect(paneOwner({ pref: "session", focusedId: undefined, shellShown: false, panelTabId: "t9" })).toEqual({ key: "shell", tabId: "t9", shell: true });
      ```
    - `focus in the panel on screen gives the shell`: `{ pref: "shell", focusedId: "s1", shellShown: true, panelTabId: "t9" }` gives the shell.
    - `a panel off screen gives the session`: `{ pref: "shell", focusedId: "s1", shellShown: false, panelTabId: "t9" }` gives `{ key: "s1", tabId: "s1", shell: false }` (decision 18).
    - `a shell not there yet has no tab id`: `panelTabId: undefined` with no session gives `tabId: null`.
    - `focusOwnerOf` with fakes whose `closest(sel)` answers for the named area (for example `{ closest: (s: string) => (s === SHELL_PANEL_AREA ? {} : null) }`):
      - `the shell panel is the shell`;
      - `a session's terminal is the session`;
      - `a sidebar row is the session`;
      - `the pane leaves it` (`PANE_AREA` answers, and so does `SESSION_AREA`: still null);
      - `the title bar leaves it`;
      - `nothing focused leaves it` (null gives null).
    - `ownerKeyOf`: `("t9", "t9") === "shell"`; `("s1", "t9") === "s1"`; `("t9", undefined) === "t9"`.
    - `ownerPageKey`: `("shell", "p1", "t9") === "t9/p1"`; `("s1", "p1", "t9") === "s1/p1"`; `("shell", "p1", undefined) === "shell/p1"`.
    - `renamed`: `renamed(new Set(["shell", "s1"]), "shell", "t9")` equals `new Set(["t9", "s1"])`; a set without `"shell"` comes back as the same object.
  - `paneState.test.ts`:
    - `handOver moves the shell's set to the new session`: `sessions.shell` holds two tabs, `active` and `tree`. After `handOver { from: "shell", to: "t9" }`, `sessions.t9` deep-equals the old shell set and `sessions.shell` is undefined.
    - `handOver into a set with only a tree keeps that tree`: `sessions.t9 = { tabs: [], active: null, tree: T2 }`. After the handover `t9.tabs` are the shell's tabs and `t9.tree` is `T2`.
    - `handOver into a set with tabs does nothing`: the result is the same object (`toBe`).
    - `handOver with no shell set does nothing`: the same object.
  - `paneNews.test.ts`:
    - `rename moves the shell's news to the session`: `{"browser:shell", "files:s1"}` becomes `{"browser:t9", "files:s1"}`.
    - `rename with nothing to move is the same set`.
  - `opens.test.ts`: switch the existing tests' context to `owner: (id) => (map.has(id) ? { key: id, project: map.get(id)!.project } : null)`, then add:
    - `an open for the panel's shell goes into the shell's set`: `owner("t9")` is `{ key: "shell", project: "C:/M" }`. A file open for `t9` gives `actions[0].session === "shell"` and `done` equal to `[{ id, refused: null }]`.
    - `the shell's set meets its own limit`: 32 tabs under `shell` give `refused: "limit"` and `full` equal to `["shell"]` for an auto open.
  - `paneCommands.test.ts`:
    - `leavePaneOutcome`: `{covers: true, alone: false, shellShown: true}` gives `"hide"`; `{covers: false, alone: true, shellShown: true}` gives `"focus"`; `{covers: false, alone: true, shellShown: false}` gives `"hide"`; `{covers: false, alone: false, shellShown: false}` gives `"focus"`.
    - `tabLimitText`: `tabLimitText("shell")` reads `"The shell panel has 32 tabs. Close one to open another."` and `tabLimitText("s1")` is `TAB_LIMIT`.
- [ ] **Step 2: Run** `pnpm test src/features/pane`. Expected: FAIL (the module is missing; the actions are unknown).
- [ ] **Step 3: Implement** as the Interfaces say. Two callers in `LiveApp.tsx` are wired in Task 7; until then they get stopgaps, so `pnpm build` (which type-checks all of `src`) stays clean:
  - `leavePaneOutcome`'s one caller (about line 1244) gets `{ covers, alone: false, shellShown: false }`.
  - `takeOpens`'s call (about line 996) gets `owner: (id) => { const r = rows.find((x) => x.id === id); return r ? { key: id, project: r.projectKey } : null; }` in place of `sessions`.
  - In `paneCommands.test.ts`, the two old `leavePaneOutcome(true)` / `leavePaneOutcome(false)` cases (lines 308 to 313) are replaced by the four above.
  - `SHELL_DELIVERY` gets a test: its `name` is `"The shell panel"` and its `via` is `"clipboard"`.
- [ ] **Step 4: Run** `pnpm test src/features/pane` and `pnpm build`. Expected: PASS and a clean build.
- [ ] **Step 5: Commit** `feat(pane): a pane owner that is a session or the shell panel, and the handover of the shell's tabs.`

---

### Task 4: Browser pages for the shell

Worker, `effort: high` (the browser page lifecycle).

**Files:**
- Modify: `src/features/browser/useBrowserPages.ts` (`owners`, `openKeys`, `reloadPlan`, `Options`)
- Test: `src/features/browser/useBrowserPages.test.ts` (it holds the `reloadPlan` tests too, at line 42; its existing two-argument calls at lines 21 to 65 get `null` as the third argument)
- Modify: `src/features/sessions/LiveApp.tsx` (the one stopgap of Step 4). No other file calls `owners`, `openKeys` or `reloadPlan`.

**Interfaces:**
- Consumes: Task 3's `SHELL_OWNER`, `handOver`, `tabLimitText`, `SHELL_TAB_LIMIT`.
- Produces:
  ```ts
  /** Every browser tab that may have a page, by key. The shell's tabs are keyed by `shellTabId` and left out while it is null. */
  export function owners(pane: PaneView, sessions: ReadonlySet<string> | null, shellTabId: string | null): Map<string, Owner>;
  export const openKeys: (pane: PaneView, sessions: ReadonlySet<string> | null, shellTabId: string | null) => Set<string>;
  export function reloadPlan(pane: PaneView, keys: readonly string[], shellTabId: string | null): { key: string; url: string | null }[];
  type Options = { /* as now */ shellTabId: string | null };
  ```
  - `Owner.session` is the pane key (`"shell"` for the shell). `pageAction` and `popupOutcome` therefore dispatch to the shell's set unchanged.
  - The `"shell"` set is never filtered by `sessions`.
  - `browserSession()` answers the owner key on screen (Task 7 passes it).
  - `popupOutcome` says `tabLimitText(session)` where it says `TAB_LIMIT` today (`useBrowserPages.ts:53`), so a popup in a full shell set gives the shell's line.

**Steps:**

- [ ] **Step 1: Write the failing tests** in `useBrowserPages.test.ts`:
  - `the shell's tabs are keyed by the panel's tab id`: `sessions.shell` holds browser tab `p1` and file tab `p2`.
    ```ts
    const o = owners(pane, new Set(["s1"]), "t9");
    expect([...o.keys()]).toEqual(["t9/p1"]);
    expect(o.get("t9/p1")).toEqual({ session: "shell", paneTab: "p1" });
    ```
  - `with no panel shell the shell's pages can't load`: `owners(pane, new Set(["s1"]), null)` holds no key that ends in `/p1` of the shell's set.
  - `a handover keeps every key`:
    ```ts
    const before = openKeys(pane, new Set(["s1"]), "t9");
    const after = openKeys(paneReducer(pane, { type: "handOver", from: "shell", to: "t9" }), new Set(["s1", "t9"]), null);
    expect(after).toEqual(before);
    ```
  - `a page report of a shell page moves the shell's tab`: `pageAction(owners(pane, …, "t9").get("t9/p1"), { url, title, loading: false })` gives `{ type: "page", session: "shell", paneTab: "p1", … }`.
  - `a popup in a shell page opens in the shell's set`: `popupOutcome(pane, "shell", "https://example.com/", true, [])` gives `actions[0].session === "shell"`.
  - `a popup in a full shell set says the shell's limit`: with 32 tabs under `shell`, `popupOutcome(pane, "shell", "https://example.com/", true, [])` has `say === SHELL_TAB_LIMIT`.
  - `reloadPlan finds a shell page`: `reloadPlan(pane, ["t9/p1"], "t9")` gives `[{ key: "t9/p1", url: <p1's address> }]`.
- [ ] **Step 2: Run** `pnpm test src/features/browser/useBrowserPages.test.ts`. Expected: FAIL (the third argument is missing; the shell's set is filtered out).
- [ ] **Step 3: Implement.** In the hook, `owners(pane, sessions === null ? null : new Set(sessions), shellTabId)`; `reloadLoaded` passes the shell id from a ref kept like `paneRef`.
- [ ] **Step 4: Run** `pnpm test src/features/browser` and `pnpm build`. Expected: PASS; the build fails only at `LiveApp.tsx`'s call. Give that call `shellTabId: null` for now (Task 7 wires it).
- [ ] **Step 5: Commit** `feat(browser): the shell's browser tabs are keyed by the panel shell's id, so a handover keeps their pages loaded.`

---

### Task 5: The fill layout and the blocked-pane copy

Worker (medium).

**Files:**
- Modify: `src/features/window/paneLayout.ts` and `src/features/window/paneLayout.test.ts`
- Modify: `src/features/window/paneColumn.ts` and `src/features/window/paneColumn.test.ts` (`src/features/browser/pageRule.test.ts` and `usePageSlot.test.ts` also call it; they change only if `pnpm build` or their run says so)
- Modify: `src/features/sessions/LiveApp.tsx` (the import at line 103 and the call at line 1172 only) and `src/features/sessions/LiveApp.test.tsx` (line 13 and lines 733 to 736, which call `paneToggleOutcome("t1", false)`)
- Modify: `src/features/window/MainWindow.tsx`, `MainWindow.test.tsx`, `MainWindow.css`, `MainWindow.css.test.ts`
- Modify: `src/features/pane/paneCommands.ts` and its test (`paneToggleOutcome`, the History line)

**Interfaces:**
- Produces:
  ```ts
  // paneLayout.ts
  export type PaneMode = "hidden" | "beside" | "cover" | "fill";
  /** `alone`: no session, so an open pane fills the room in any width (6v decision 7). */
  export function paneGeometry(i: { room: number; shown: boolean; saved: number | null; terminalMin: number; alone?: boolean }): PaneGeometry;
  // paneMotionKey("fill", any) === "fill"; dividerState("fill", m) === { live: false, still: true }

  // paneColumn.ts
  export type ColumnInputs = { /* as now */ alone?: boolean }; // passed through to paneGeometry

  // MainWindow.tsx, new prop
  /** With no session an open pane fills the area right of the sidebar. Left out, the pane needs a session, as before. */
  paneAlone?: boolean;

  // paneCommands.ts
  export const PANE_NOT_WITH_HISTORY = "Close History to show the side pane.";
  export function paneToggleOutcome(viewing: boolean): "toggle" | "blocked";
  ```
- `paneGeometry`: room 0 gives hidden; not shown gives hidden; `alone` and shown gives `{ mode: "fill", terminal: 0, pane: room, max }`. The cover and beside rules are untouched.
- `MainWindow`:
  - `paneWanted = paneShown && !viewing && (!empty || paneAlone)`.
  - `paneGeometry` gets `alone: empty && paneAlone`, and `layoutValue.inputs` carries `alone` too, so the page's resize rule follows the fill.
  - `paneBlocked = viewing || (empty && !paneAlone)`.
  - The button title reads `Close History to show the side pane` when `viewing`, and the old `The side pane shows beside a session` only when `empty && !paneAlone` (stories and tests that leave `paneAlone` out).
  - The empty state is drawn when `empty && !paneDrawn`.
  - `data-pane` reads `"fill"`.
  - `dividerLeft` (about line 401) gets `covers: paneCovers || geometry.mode === "fill"`: a divider that had focus when the last session closed hands the keyboard to the pane, as it does when the pane starts to cover.
- CSS: `.window__main[data-pane="fill"]` gets the cover rule's `grid-template-columns: minmax(0, 1fr)`. No colour.
- `PANE_BESIDE_SESSION` is renamed `PANE_NOT_WITH_HISTORY` with the new text. `noTabText`, `closeNothingText` and `LiveApp.tsx`'s `togglePanel` use it. `LiveApp`'s calls of `paneToggleOutcome(session, viewing)` become `paneToggleOutcome(viewing)`.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `paneLayout` tests:
    - `alone, the pane fills the room`: `paneGeometry({ room: 1000, shown: true, saved: 400, terminalMin: 700, alone: true })` has `mode === "fill"`, `terminal === 0` and `pane === 1000`.
    - `alone in a narrow window still fills`: room 500 gives `"fill"`, not `"cover"`.
    - `alone and closed is hidden`.
    - `a filling pane has no divider`: `dividerState("fill", false)` equals `{ live: false, still: true }`.
    - `the fill moves as one key`: `paneMotionKey("fill", "expanded") === "fill"`.
    - The last two pass as soon as `"fill"` is a `PaneMode`: `dividerState` (`paneLayout.ts:58`) and `paneMotionKey` (`:49`) already treat every mode but `"beside"` this way. They pin it; say in the report that they were not watched failing.
  - `paneColumn` tests: `a filling pane is the whole room`: `paneColumn(1400, { pref: "expanded", sidebarWidth: 280, drawer: false, shown: true, saved: 400, terminalMin: 700, rtl: false, alone: true })` has `width` equal to `mainRoom(1400, …).room` and `x === 1400 - width`.
  - `MainWindow.test.tsx` (`renderToStaticMarkup`):
    - `with no session an open pane fills the area`: `rows: []`, `paneShown`, `paneAlone`, `pane={<p>the pane</p>}`. The markup has `data-pane="fill"` and `the pane`, and not `No sessions yet.`
    - `with no session a closed pane shows the empty state`: `paneShown={false}` and `paneAlone` give `No sessions yet.`
    - `the panel buttons work with no session`: with `paneAlone`, no `.window__pane-toggle` has `aria-disabled`.
    - `under History the buttons say why`: with `mainView` set, a button's title starts with `Close History to show the side pane`.
  - `MainWindow.css.test.ts`: `a filling pane has one column`: the rule `.window__main[data-pane="fill"]` sets `grid-template-columns: minmax(0, 1fr)`.
  - `paneCommands.test.ts`:
    - `the pane toggles with no session`: `paneToggleOutcome(false) === "toggle"`.
    - `History blocks the pane`: `paneToggleOutcome(true) === "blocked"`.
    - `noTabText(null, true)` reads `"Close History to show the side pane."`.
- [ ] **Step 2: Run** `pnpm test src/features/window src/features/pane/paneCommands.test.ts src/features/browser/pageRule.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement** the Interfaces. Update the tests that quote the old line (find them with Grep for `shows beside a session`): `MainWindow.test.tsx:600`, with `mainView` set, now expects `Close History to show the side pane`. In `LiveApp.tsx` change only the import and the call at line 1172 (`paneToggleOutcome(mainViewRef.current !== "sessions")`, announcing `PANE_NOT_WITH_HISTORY`); in `LiveApp.test.tsx` the calls become `paneToggleOutcome(false)` and `paneToggleOutcome(true)`.
- [ ] **Step 4: Run** `pnpm test src/features/window src/features/pane src/features/browser src/features/sessions/LiveApp.test.tsx` and `pnpm build`. Expected: PASS and a clean build.
- [ ] **Step 5: Commit** `feat(window): with no session an open side pane fills the area right of the sidebar.`

---

### Task 6: The shell is always there

Worker, `effort: high` (the panel's start race).

**Files:**
- Modify: `src/features/sessions/shellPanelAsk.ts` and `shellPanelAsk.test.ts`
- Create: `src/features/sessions/shellRespawn.ts`, `src/features/sessions/shellRespawn.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx` (a new `onExited` prop; the `onExit` callback at line 284)
- Modify: `src/features/sessions/LiveApp.tsx` (the panel effect at about lines 1475 to 1519, the panel's `TerminalView` at about line 1564, and the panel state, which keeps the full folder)

**Interfaces:**
- Produces:
  ```ts
  // shellPanelAsk.ts: `shown` becomes `ready` (settled and loaded); the folder rule is as before.
  export function shellPanelAsk(i: { ready: boolean; sessions: number; focusedCwd: string | undefined }): { cwd: string | null } | null;

  // shellRespawn.ts
  export const QUICK_EXITS = { count: 3, withinMs: 10_000 } as const;
  /** After the panel's shell ended at `now`: whether to start another, and the exits to keep (those within the window, this one included). */
  export function shellRespawn(exits: readonly number[], now: number): { again: boolean; exits: number[] };

  // TerminalView.tsx
  /** The process behind this terminal ended (the core's exit frame), after the line saying so is written. Read through a ref. */
  onExited?: (code: number) => void;
  ```
  - `shellRespawn`: `again` is false when the kept exits number `QUICK_EXITS.count` or more.
- `LiveApp`:
  - The ask effect asks while `shellPanelAsk({ ready: settled && loaded, sessions: infos.length, focusedCwd })` is not null, whether the panel is shown or not (decision 19). It does not ask while `failedFor === endpoint`. A new guard is needed: today a failure leaves `panel` null and the effect's dependencies unchanged, but a stopped respawn changes `panel`.
  - The panel's state becomes `{ endpoint; tabId; folder; cwd }`, where `cwd` is the full `res.cwd`. Task 7 needs it for Browse, recent files and the pane's project.
  - The panel's `TerminalView` gets `onExited`. When `panel.tabId` is that terminal's id: run `shellRespawn(exitsRef.current, Date.now())` and keep its `exits`, then `setPanel(null)`. When `again` is false, also `setFailedFor(endpoint)` and `announce(SHELL_FAILED)`.
  - `retryShell` also empties `exitsRef`.
  - The core replaces the exited shell on the next `open` (verified, see Global Constraints).
  - The comment above the panel's state says the shell is asked for as soon as the window is ready, and again when it ends. Hiding and showing the panel is no longer a retry.
- The shell's tab set is untouched by an exit (decision 20). Its pages reload under the new id through Task 4's `owners`.
- The panel's terminal is drawn only while the sidebar is expanded (`MainWindow.tsx:520`), so the window can't see an exit in rail or focus mode until the sidebar is expanded again (decision 22). Not verified: whether a terminal that connects to a shell that has already ended gets the core's exit frame. Read the pty WebSocket in `crates/core` first and say what you found in the report. If it gets none, change nothing in the core and report it: the main thread decides.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `shellPanelAsk.test.ts`: rename `hidden asks nothing` to `not ready asks nothing` (`ready: false` gives null). Add `a hidden panel still asks`: `{ ready: true, sessions: 0, focusedCwd: undefined }` gives `{ cwd: null }`. Keep the other three cases with `ready: true`.
  - `shellRespawn.test.ts`:
    - `the first exit starts another`: `shellRespawn([], 1000)` equals `{ again: true, exits: [1000] }`.
    - `the third exit within ten seconds stops`: `shellRespawn([1000, 4000], 9000).again === false`.
    - `old exits are forgotten`: `shellRespawn([1000, 2000], 20_000)` equals `{ again: true, exits: [20_000] }`.
  - TerminalView: no test can drive the socket's `onExit` (`TerminalView.test.tsx` has no DOM; it renders static markup and reads the source, line 24). The `onExited` wiring is checked by live-check item 12; say so in the report.
- [ ] **Step 2: Run** `pnpm test src/features/sessions/shellPanelAsk.test.ts src/features/sessions/shellRespawn.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement** the modules, the prop (kept in a ref, so a new callback doesn't remake the socket) and the `LiveApp` changes above.
- [ ] **Step 4: Run** `pnpm test src/features/sessions src/features/terminal` and `pnpm build`. Expected: PASS and a clean build.
- [ ] **Step 5: Commit** `feat(shell-panel): the panel's shell is always there: asked for once the window is ready, and started again when it ends.`

---

### Task 7: `LiveApp` wiring: the pane follows its owner

Worker, `effort: high` (the big `LiveApp` task). Read the whole of this task before the first edit.

**Files:**
- Modify: `src/features/sessions/LiveApp.tsx`
- Create: `src/features/changes/ShellNote.tsx`, `src/features/changes/ShellNote.test.tsx` (a style only if needed, in `ChangesPanel.css`, from tokens)
- Modify: `src/features/changes/ChangesPanel.tsx` (a `note?: string` prop drawn first in the panel's body; the line at about 362)
- Modify, decision 21's copy: `src/features/files/FileTree.tsx:10`, `src/features/files/filesWait.ts:9`, `src/features/pane/EmptyBrowser.tsx:17` and `:22`, `src/features/changes/githubCopy.ts:119`, `src/features/window/ShellPanel.tsx` (a new `SHELL_STARTING`), and every test that quotes the old lines (Grep each one)

**Interfaces:**
- Consumes: everything from Tasks 3 to 6.
- Produces:
  ```ts
  // ShellNote.tsx
  export const SHELL_CHANGES_NOTE = "The shell panel runs no agent, so Review is off and comments are copied, not sent.";
  export function ShellNote(p: { text: string }): JSX.Element; // one <p>, read in order, no live region
  // ShellPanel.tsx
  export const SHELL_STARTING = "The shell is still starting.";
  ```

**Copy (exact):**
- `NO_FOLDER_LINE`: `There is no folder the tree can show.`
- `listFailed(null)`: `Can't list the files.` (`listFailed(folder)` is unchanged.)
- EmptyBrowser line with ports: `Open an address, an HTML file or a local server.`; its list heading: `Local servers`.
- `ChangesPanel.tsx:362`: `` `Changes keeps at most ${MAX_COMMENTS} comments.` ``
- `githubCopy.moved`: `This isn't the folder the terminal started in. A push runs this repository's hooks and settings.`
- The Changes note: `SHELL_CHANGES_NOTE` above. While the shell has no tab id: `SHELL_STARTING`, or `SHELL_FAILED` after a failure.
- Unchanged: Changes' `No git repository` and `<folder> isn't in a git checkout.` (`copy.ts:36`); `TAB_LIMIT` for sessions; `No session to send the comments to.` (reached only with no owner terminal).

**Behaviour, by place in `LiveApp.tsx` (line numbers are today's, about):**
1. **Order of hooks.** `owner` is needed at about line 750. Move only these above it, unchanged and in this order:
   - `const panelIdRef = useRef<string | undefined>(undefined)` (from about 1320; used before its declaration otherwise);
   - the `panel` and `adopted` `useState`s (about 1480 and 1482);
   - `failedFor` and `attempt` (about 1492 and 1493);
   - `shellShown` (about 1483);
   - `panelTab` and `panelIdRef.current = …` (about 1513 and 1514).

   `shellAsk`, `asking`, the ask effect and `shellFailed` stay where they are.
2. **The owner.**
   - Add `const [ownerPref, setOwnerPref] = useState<OwnerPref>("session")`.
   - In the `focusin`/`focusout` listener (764 to 772), when `focusOwnerOf(document.activeElement)` isn't null, set it.
   - Then `const owner = paneOwner({ pref: ownerPref, focusedId, shellShown, panelTabId: panelTab?.tabId })`, with `ownerRef` and `shellShownRef` kept as the file keeps its other refs.
   - `alone = rows.length === 0`. Pass `paneAlone` to `MainWindow` as `true`.
   - Don't special-case `focusedId` undefined while rows exist: the pane then shows the shell's set, where today it shows none.
3. **Pane state on screen** (750 to 752): `stack = stackOf(view.pane, owner.key)`. `panelsOn = mainView === "sessions" ? onScreen(view.pane, owner.key) : []`. `beside` goes.
4. **The command target** (774 to 788): `onNow` holds `owner.key`. `shownTabAnswer` uses `owner.key`, and `ownerPageKey(owner.key, tab.id, panelTab?.tabId)` for every map key. The same key function is used at 956, 2475, 2491 and 2532, and in `focusAddressSoon`.
5. **`marshell open`** (996 to 1012): `takeOpens` gets `owner: (id) => id === panelIdRef.current ? { key: SHELL_OWNER, project: panelTab?.cwd ?? "" } : rows.find(…) ? { key: id, project } : null`, and `shown: owner.key`. `limitNews(…, (s) => s === owner.key)`. The line said is `tabLimitText(…)` of the key told.
6. **News and seen marks** (1019 to 1031): `unseenPanels` and the `seen` dispatches use `owner.key`. The `"only"` set is `new Set([...rows.map((r) => r.id), SHELL_OWNER])`.
7. **Opens** (1033 to 1161, 1282 to 1300):
   - `openInPane`, `openFromTree`, `openPageInPane`, `dropPaths`, `browseFile` and `openHtmlFile` read `ownerRef.current.key`.
   - The refusals say `tabLimitText(key)`.
   - The dialogs start in `owner.shell ? panelTab?.cwd : <the focused info's cwd>`.
   - `paneProject()` is `owner.shell ? panelTab?.cwd ?? "" : projectKeyOf(…)`.
   - `NO_SESSION_TO_OPEN` stays only for a missing endpoint.
   - Known limit, for the leftovers issue and not for this task: a session's project key is its repository's root (`registry.rs:337`) and the shell's is its folder, so after a handover from a subfolder of a repository the pane's recent files start a new list.
8. **Pin, toggle, run** (1164, 1170, 1187): use `owner.key`. `paneToggleOutcome(mainViewRef.current !== "sessions")` gives `PANE_NOT_WITH_HISTORY` when blocked.
9. **Every focus return goes to the owner's terminal.** Add:
   ```ts
   const toOwnerTerminal = (): void => {
     if (!ownerRef.current.shell) return actions.focusTerminal();
     if (shellShownRef.current) return focusShellTerminal(document);
     document.querySelector<HTMLElement>(".window__main")?.focus();
   };
   ```
   - It replaces `actions.focusTerminal()` in `togglePanel`'s hide (1182), `paneRun`'s hide (1198) and `toTerminalFromPane` (1245).
   - `toTerminalFromPane` uses `leavePaneOutcome({ covers, alone, shellShown: shellShownRef.current })` (decision 16).
   - `addRegion`'s last focus and `focusDelivered`'s session branch stay the session's when the owner is a session.
   - `applyNav`'s `select-pane` and the History jumps stay `actions.focusTerminal()`: they pick a session.
10. **Browser pages** (1761 to 1769, 1791): `shownTab` and `frontTab` come from `owner.key`, with keys from `ownerPageKey`. `useBrowserPages` gets `shellTabId: panelTab?.tabId ?? null`, and `browserSession: () => (onNow.current.panelsOn.includes("browser") ? onNow.current.session : null)`, where `session` is the owner key.
11. **The handover** (the listing effect, 1548 to 1556): when `infos` lists `adopted.tabId`, dispatch in this order before `setPanel(null)` and `pick(id)`:
    - `paneDispatch({ type: "handOver", from: SHELL_OWNER, to: id })`;
    - `newsDispatch({ type: "rename", from: SHELL_OWNER, to: id })`;
    - `limitTold.current = renamed(limitTold.current, SHELL_OWNER, id)`;
    - `setOwnerPref("session")`.

    Both "Open as a session" and the core's own adopt (`handedOff`) reach this effect, so no other place hands over (decision 8).
12. **The pane's render** (2392 to 2615):
    - `paneSession` goes. Every `paneDispatch` uses `owner.key`.
    - `KeptTabs key={`${owner.key}:${panel}`}`.
    - `stripTitle(owner.tabId ?? undefined, …)`.
    - `EmptyBrowser ports={owner.shell ? [] : focusedRow?.ports ?? []}`.
    - `regionTarget` and `addRegion` use `owner.shell ? SHELL_DELIVERY : deliveryFor(focusedRow ?? null)`, and save the attachment for `owner.tabId`.
    - Files: `FilesPanel key={owner.tabId} session={owner.tabId}` when `owner.tabId !== null`, else `tabsBody ?? <EmptyFiles …/>`. Its `onSaved(session, tree)` maps `ownerKeyOf(session, panelIdRef.current)`, and dispatches when the key is `SHELL_OWNER` or `infos` lists it. `pasteTo` uses `deliveryTo(owner.tabId, …)`.
    - Changes, when `owner.tabId !== null`: `key={owner.tabId}` and `session={owner.tabId}`. For the shell: `agent={owner.shell ? null : …}`, `agentName` "the shell", `via="clipboard"` and `note={SHELL_CHANGES_NOTE}`. `onSend` takes a delivery: `sendComments` gets `owner.shell ? SHELL_DELIVERY : deliveryFor(row)` from its caller. `onTyped` focuses through `toOwnerTerminal`.
    - With no tab id: `EmptyPanel` with the Changes title and the line `shellFailed ? SHELL_FAILED : SHELL_STARTING`.
    - `PanelStack onWeights` uses `owner.key`.
    - More places that feed the pane and follow the owner:
      - `filesCommand` (about 1838) reads `stateRef.current.focusedId` for "Collapse all folders": it uses the owner's key.
      - `paneProjectNow` and `paneRecent` (2394 to 2396): the owner's project, as in item 7.
      - `FilesPanel onPaste` (2598): `pasteInto(owner.tabId, [path])`.
      - The dependency lists of the `shouldPin` effect (1166) and the `seen` effect (1026) name `owner.key` where they name `focusedId`.
      - Changes' `sessionName` (2447): `owner.shell ? SHELL_DELIVERY.name : focusedRow?.name`.
13. **Unchanged:** `onScreenIds` and `useViewReport` (sessions on screen, for notifications); the drafts read at 912 (it already reads every set, the shell's included); `renderTerminal`.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `ShellNote.test.tsx`: `the note says why Review is off`: the markup holds `The shell panel runs no agent, so Review is off and comments are copied, not sent.` and is one `<p>`.
  - The copy tests: update `FileTree`'s, `filesWait`'s, `EmptyPanel.test.tsx`'s EmptyBrowser cases, `src/features/changes/PushSheet.test.tsx:24` (the `moved` line; `githubCopy.test.ts` doesn't quote it) and `src/features/sessions/LiveApp.test.tsx:816` and `:818` ("Running in this session") to the exact lines above. They FAIL until the strings change.
- [ ] **Step 2: Run** `pnpm test src/features/changes/ShellNote.test.tsx src/features/files src/features/pane src/features/changes/PushSheet.test.tsx src/features/sessions/LiveApp.test.tsx`. Expected: FAIL.
- [ ] **Step 3: Implement** the Behaviour list in order, building after each numbered item (`pnpm build`), so a type error points at one change.
- [ ] **Step 4: Check by hand that no `focusedId` is left in the pane's paths.** Grep `LiveApp.tsx` for `focusedId` and `focusTerminal()`, and justify each remaining use in the report against item 13 and item 9's exceptions.
- [ ] **Step 5: Run** `pnpm test` (output to a file, read the tail) and `pnpm build`. Expected: PASS and a clean build.
- [ ] **Step 6: Commit** `feat(pane): the side pane follows the shell panel's focus, works with no session, and hands its tabs to the new session.`

---

### Task 8: The live check, its note, and decision 22

Inline (main thread, with `marshell-live-check`).

**Files:**
- Create: `docs/research/2026-11-25-shell-pane-live-check.md`
- Modify: `docs/PLAN.md` §9 (decision 22: add every ruling the ledger holds that it doesn't yet say)

**Steps:**

- [ ] **Step 1:** Run the live check below on a fresh scratch folder. Record each item's result, the PIDs started and stopped, and the screenshots' paths in the note.
- [ ] **Step 2:** Bring decision 22 in `docs/PLAN.md` §9 up to date: one bullet for each `Ruling:` line in the ledger that changes what a person sees or what is saved.
- [ ] **Step 3: Commit** `docs(research): the shell pane live check, and decision 22 of slice 6v.`

---

## Live check (exit criteria)

Run with `marshell-live-check` on a fresh scratch folder (scratch `MARSHELL_HOME`, `MARSHELL_DEFAULT_FOLDER`, CLI homes). Put a `README.md` and a git checkout with one change under `<Scratch>\home\Marshell`. For item 9, serve one page with a single text field from a loopback server the check starts itself, and stop it by PID.

1. With no session after the start, Ctrl+Shift+D (the Files panel's key, `lib/keymap.ts:98`) opens the pane. `.window__main` has `data-pane="fill"`, the pane runs from the sidebar's edge to the window's edge, and the tree lists `README.md`.
2. The same key again closes it, and the window reads the empty state's `No sessions yet. Start one with …` line.
3. With no session and the panel shown: Esc in the pane puts the keyboard in `.shell-panel .xterm-helper-textarea` and the pane stays open. With the panel hidden (its header button): Esc hides the pane and the announcer says `The side pane is hidden. … shows it.`
4. With the panel hidden from the start (hide it, then `restart`), the panel's shell is already running: showing the panel shows a prompt at once. Opening the pane with no session lists the folder.
5. With two sessions open: a click in the panel's terminal shows the shell's tabs, and a click in a session's terminal shows that session's. Hiding the panel or collapsing the sidebar while the panel had focus shows the focused session's tabs, and the shell's come back when the panel is shown and focused again.
6. `marshell open README.md` typed in the panel adds a background tab to the shell's set and announces `Opened README.md in Files.` Typed with `--session 1`, it lands in session 1's set.
7. Changes for the shell in the checkout shows the diff and `The shell panel runs no agent, so Review is off and comments are copied, not sent.`. The Changes menu has no review item, and the send button reads `Copy`.
8. Changes for the shell in a plain folder reads `No git repository` and `<folder> isn't in a git checkout.`
9. With text typed into the scratch page's field in a shell browser tab, "Open as a session" leaves the text there. CDP lists the same page target before and after.
10. After item 9, the new session holds the shell's tabs. The panel shows a fresh shell, and the pane with focus in it shows no tabs.
11. A fake agent started in the panel is adopted by the core, and the shell's tabs go with it as in items 9 and 10. The fake agent is a copy of fake-agent named `codex-marshell-test.exe`, run by its full path, as `an_agent_in_the_panel_is_handed_off` does (`crates/core/tests/foreground.rs:286`); the watcher names an agent from its exe name and needs two readings.
12. `exit` typed in the panel gives a new prompt within a second; the shell's tabs are still there, and its page reloads at its saved address. Typing `exit` three times within ten seconds shows `The shell didn't start.` and `Try again`, which starts a shell. Then, with the sidebar collapsed, the panel's shell is ended by its own PID (read from the tree) and the sidebar expanded: the note records whether a fresh shell starts.
13. Resizing the window with a page in a filling pane keeps the page inside the pane. At 600 px wide the pane still fills (no `data-pane="cover"` with no session).
14. With the shell's tabs saved, `restart` brings them back under the panel. Tab alone reaches the pane in fill mode, and in emulated forced colours the note and the strip are readable.

## After the tasks

The final whole-branch review (`marshell-reviewer`, Opus) with this plan's Review Focus, one fix pass, then `marshell-ship` and `pre-merge-review`. The research note is `docs/research/2026-11-25-shell-pane-live-check.md`.
