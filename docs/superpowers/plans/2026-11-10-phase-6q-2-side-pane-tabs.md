# Slice 6q-2: the side pane and its tabs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Preview becomes the side pane: each session has its own tabs, saved in `layout.json` and restored on restart; the pane has a tab strip with a + menu, a saved width, an 80-column rule, keyboard access, and 200 ms motion on every layout change. Its tabs show the kinds the Preview already has (Markdown, code, HTML, image, PDF). The browser, the editor and the PDF features come in 6q-3 to 6q-6.

**Architecture:** The pane's whole state (`open`, `width`, each session's tabs, recent files) is one `PaneView` inside `ViewLayout`, owned by the frontend and saved through the existing `PUT /v1/layout/view`; the core only validates and repairs it. A pure reducer (`paneState.ts`) replaces `previewState.ts`. The window's geometry is one pure function (`paneLayout.ts`), and discrete layout switches are tweened in JS from constants (`motion.ts`), so 6q-3 can hand the same rule and curve to Rust. The existing viewers (`PreviewPane` and what it renders) become the body of a tab, unchanged inside.

**Tech Stack:** React 19, TypeScript, Vitest (node, `renderToStaticMarkup`), xterm.js 6, Rust (`marshell-protocol`, `marshell-core`, `marshell-app`), ts-rs, `tauri-plugin-dialog` (already a dependency).

**Spec:** `docs/PLAN.md` §9: "Slice 6q-2 decisions (2026-11-10)" 1–13, "Slice 6q decisions (2026-11-07, first round)" 3–5 and 10, and "Slice 6q-0 decisions" 6. Research: `docs/research/2026-11-07-tabs-pane-options.md` §2.2. Carry-over: `docs/spikes/S12-browser-pane.md`, "What 6q-2 and 6q-3 take from this": of it, 6q-2 builds only the app's side of Bounds (the layout as a rule, the motion's curve) and the drop on the pane opening a tab; everything that needs a page, the listener on a page's webview included, is 6q-3's.

## Global Constraints

- **Names.** The UI says "side pane", never "Preview". In code the feature folder is `src/features/pane/`; `src/features/preview/` keeps the viewers. A session's id is `tab_id` (the core's word), so a tab of the pane is always "pane tab" in identifiers (`PaneTab`, `paneTab`), never bare `tab`.
- **Numbers.** Pane minimum 320 px. The pane covers the terminal when the room is under 720 px. The terminal's floor is 80 columns. The divider's keyboard step is 16 px. A saved width is 320 to 4096, or null (half the room). Motion: 200 ms, `cubic-bezier(0.2, 0.8, 0.2, 1)` (`--dur-drawer`, `--ease-out`). Limits: 32 tabs per session, 50 history entries per tab, 200 recent files in the file, 10 shown, 4 tabs kept loaded.
- **Copy** (sentence case, no "!", no "successfully"; exact strings):
  - Commands: `preview` → "Side pane"; `nav-next-pane` → "Switch between the terminal and the side pane"; new `pane-next-tab` "Next tab", `pane-previous-tab` "Previous tab", `pane-close-tab` "Close tab", `browse-file` "Browse…". `open-file` stays "Open file…".
  - Labels: pane `aria-label="Side pane"`; tab list `aria-label="Tabs"`; divider `aria-label="Resize the side pane"`; hide button `aria-label="Hide the side pane"`; + button `aria-label="New tab"`; a tab's close button `aria-label="Close ${title}"`.
  - Empty pane: "No tabs open in this session.", buttons "Open file…" and "Browse…", heading "Recent in this project", hint "Drop a file here to open it."
  - Announcements: "Opened ${name} in the side pane." · "Closed ${name}." · "No tabs open." · "This session has 32 tabs. Close one to open another." · "A folder can't be shown in the side pane." · "No session to open it in." · "Close ${cover} to open a file." (`${cover}`: "the launcher", "the command palette", "Settings", or "the dialog" for anything else) · "Selected the side pane" · "Cursor on the side pane" · "Moved ${name} to position ${n} of ${count}."
  - Tooltips: a path in the terminal "Ctrl+click to open in the side pane" (macOS "⌘+click …"); a web address "Ctrl+click to open in your browser" (macOS "⌘+click …"); drop hint "Drop to open in the side pane".
- **Styles.** Read the good-css skill before writing declarations. Tokens only, no hex. `:focus-visible` with `outline`; `:hover` only inside `@media (hover: hover) and (pointer: fine)`; forced-colors rules use system colours only; no boxed outlines (`noBoxedOutlines.test.ts`); the main window shows no scrollbar. `SelectedMark` is the only selected-state marker and `Announcer` the only live region.
- **Keyboard and forced colours.** Everything new works with the keyboard alone and in forced colours.
- **No HTML5 drag and drop** (native file drop turns it off on Windows): tab dragging uses pointer events.
- **Dependencies and security surface.** No new dependency. No change to `src-tauri/capabilities/`, `tauri.conf.json`, the CSP, or `crates/core/src/server/guard.rs`. If a task seems to need one, stop and report.
- **Checks.** Rust: `cargo test -p <crate> -j 4`, `cargo fmt --all` before each Rust commit. After the protocol change: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`; never hand-edit `src/generated/`. Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** a browser tab kind, Open URL…, New browser tab, the child webview, any Rust placement of a page, the text editor, PDF find. Port chips and Markdown web links keep their behaviour.

## Review Focus

1. **A tab whose file is gone, moved or unreadable when the app restarts.** Expected: the tab is still in the strip with its title, its body says what the Preview says today ("This file was deleted." / "This file can't be previewed."), and it closes like any tab. Tests: Task 5 `a tab whose file can't be read still shows in the strip and closes`.
2. **The same file opened under two spellings** (`C:\work\README.md`, `c:/work/readme.md`; a second Ctrl+click with another `:line`). Expected: one tab, selected, scrolled to the new line. Tests: Task 2 `an open of a path already open selects its tab`, `a Windows path matches whatever its case and slashes`, `a second open with a line moves the open tab to it`.
3. **A saved width that no longer fits** (a smaller monitor, the sidebar opened, a bigger terminal font). Expected: the pane shows at what fits, never under 320 px beside the terminal, and the saved width is not rewritten, so it comes back when there is room. Tests: Task 3 `a saved width wider than the room is shown clamped`, `the terminal goes under 80 columns only when the pane is at its minimum`, `under 720 px the pane covers`.
4. **A layout switch that interrupts another** (Ctrl+Shift+Q twice quickly; the window dragged across 960 px while the pane is still opening). Expected: the motion turns round from where it is with no jump, and the terminal fits once, after the last motion ends. Tests: Task 4 `a second switch starts from the value on screen`, `a target that changes without a switch is followed at once`, `the hold ends once after overlapping motions`.
5. **A `layout.json` that was edited by hand or written by another build:** a 33rd tab, an `index` past the history, two tabs with one id, an unknown `kind`, a session id that no longer exists, the three dead split fields. Expected: it loads, the bad parts are dropped or clamped, and nothing else is lost. Tests: Task 1 `repair_*` and `a_file_from_before_6q_2_loads_with_an_empty_pane`.

---

### Task 1: `layout.json` holds the pane

**Files:**
- Modify: `crates/protocol/src/api.rs:418-457` (`ViewLayout`, new types)
- Modify: `crates/core/src/layout.rs` (`repair`, `fold_to_one_pane`, `validate_view`, tests)
- Modify: `crates/core/tests/layout_api.rs`, `crates/core/tests/layout_restore.rs`
- Regenerated: `src/generated/ViewLayout.ts`, new `PaneView.ts`, `SessionTabs.ts`, `PaneTab.ts`, `PaneEntry.ts`, `RecentFile.ts`
- Modify: `src/features/window/viewState.ts` (`DEFAULT_VIEW`), and every fixture or test that names `arrangement`, `ratios` or `zoomed`

**Interfaces:**
- Produces (Rust, all `#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)] #[ts(export)]`; `Eq` and `Default` where they derive):

```rust
pub struct PaneEntry { pub path: String, #[serde(default)] pub line: Option<u32>, #[serde(default)] pub col: Option<u32>, /// "rendered" or "source".
    pub view: String }
pub struct PaneTab { /// Unique in its session: a-z, 0-9 and "-", 1 to 64 long.
    pub id: String, /// "file" is the only kind this build knows.
    pub kind: String, pub history: Vec<PaneEntry>, pub index: u32 }
pub struct SessionTabs { pub tabs: Vec<PaneTab>, pub active: Option<String> }
pub struct RecentFile { pub project: String, pub path: String }
pub struct PaneView { pub open: bool, pub width: Option<u32>, /// Keyed by the session's tab_id.
    pub sessions: BTreeMap<String, SessionTabs>, /// Newest first.
    pub recent: Vec<RecentFile> }
// ViewLayout: `arrangement`, `ratios` and `zoomed` are removed; added:
#[serde(default)] pub pane: PaneView,
```
- Produces (TS): `DEFAULT_VIEW.pane = { open: false, width: null, sessions: {}, recent: [] }`.
- `GET /v1/layout` returns a repaired `view.pane`; `PUT /v1/layout/view` answers 400 naming the field for a pane that breaks a rule below.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s test module (helper `fn file_tab(id: &str, paths: &[&str]) -> PaneTab`):
  - `repair_drops_a_session_that_has_no_saved_tab`: `doc.tabs = [a]`, pane sessions `a` and `gone`. Expect only `a`.
  - `repair_drops_a_tab_of_an_unknown_kind`: tabs `t1` kind `"file"`, `t2` kind `"browser"`. Expect `[t1]`.
  - `repair_drops_duplicate_ids_and_empty_histories`: `[t1, t1, t2(history empty)]` → `[t1]` (the first `t1`).
  - `repair_clamps_the_index_and_the_history`: history of 60 entries, index 59 → the last 50 entries, index 49; index 99 on a 3-entry history → 2.
  - `repair_keeps_at_most_32_tabs`: 40 tabs → the first 32.
  - `repair_points_active_at_a_tab_that_exists`: active `Some("nope")` → `Some(first id)`; a session left with no tabs is removed.
  - `repair_resets_a_width_out_of_range`: `Some(100)` and `Some(9000)` → `None`; `Some(480)` stays.
  - `repair_fixes_an_unknown_view`: view `"tree"` → `"rendered"`.
  - `repair_trims_recent_files`: 250 entries, one with an empty path → at most 200, none empty.
  - `validate_view_names_the_pane_field`: each of these alone gives an Err containing the quoted word — width `Some(100)` "width"; 33 tabs "tabs"; id `"A b"` "id"; kind `"browser"` "kind"; empty history "history"; index 3 on 2 entries "index"; view `"tree"` "view"; an empty path "path"; a path of 4097 chars "path"; active naming no tab "active"; tabs non-empty with active `None` "active"; 201 recent "recent". A default view, and a view with one good tab, are Ok.
  - In `crates/core/tests/layout_restore.rs`, `a_file_from_before_6q_2_loads_with_an_empty_pane`: write a version-1 file whose view has `"arrangement": null, "ratios": [], "zoomed": null` and no `pane`; `GET /v1/layout` returns `pane` equal to `PaneView::default()` and the same `panes`, `sidebar` and `sidebar_width`.
  - In `crates/core/tests/layout_api.rs`, `put_view_round_trips_the_pane`: PUT a view with one session holding two tabs, width 480, open true, one recent file; GET returns the same pane. Update the tests there that build a `ViewLayout` with the removed fields.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p marshell-core -j 4 layout`
Expected: compile errors for the missing types, then FAIL in the new tests.

- [ ] **Step 3: Implement.**
  - `api.rs`: the types above; remove the three dead fields and their defaults; `ViewLayout::default()` gets `pane: PaneView::default()`.
  - `layout.rs`: `fold_to_one_pane` no longer reads `zoomed` (keep the active pane's tab, else the first non-empty), so a split saved before 6q-1 and never saved since opens on its active session, not its zoomed one; `repair_folds_a_split_to_the_zoomed_session` goes and the other fold tests lose the dead fields. `validate_view` loses the three dead-field checks, and `validate_view_refuses_a_split_and_its_settings` keeps only its `panes` case. Add `fn repair_pane(pane: &mut PaneView, tabs: &[SavedTab])` and `fn validate_pane(p: &PaneView) -> Result<(), String>` with constants `PANE_WIDTH: RangeInclusive<u32> = 320..=4096`, `MAX_PANE_TABS = 32`, `MAX_PANE_HISTORY = 50`, `MAX_RECENT_FILES = 200`, `MAX_PATH_CHARS = 4096`. `validate_pane` does not check that a session id exists (as `validate_view` doesn't for `panes`); `repair` does. A history over 50 keeps its last 50 and moves `index` with them.
  - Error text names the field, as the existing ones do: e.g. `"pane.width must be 320 to 4096, not 100"`.

- [ ] **Step 4: Regenerate and fix the frontend's types**

Run: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`
Expected: both pass; `git status` shows the five new generated files and a changed `ViewLayout.ts`.

Then update `DEFAULT_VIEW` in `viewState.ts` and every TS fixture that names a removed field (`rg "arrangement|ratios|zoomed" src`).

- [ ] **Step 5: Run everything this touched**

Run: `cargo test -p marshell-core -j 4 layout` · `cargo test -p marshell-core -j 4 --test layout_api --test layout_restore` · `pnpm test src/features/window` · `pnpm build`
Expected: all pass.

- [ ] **Step 6: Commit** — `cargo fmt --all`, then `feat(layout): layout.json holds the side pane's tabs, and the split view's dead fields go.`

---

### Task 2: The pane's state, as a pure reducer

**Files:**
- Create: `src/features/pane/paneState.ts`, `src/features/pane/paneState.test.ts`
- Modify: `src/features/window/viewState.ts` (one new action), `src/features/window/viewState.test.ts`

**Interfaces:**
- Consumes: `PaneView`, `SessionTabs`, `PaneTab`, `PaneEntry` from `src/generated/` (Task 1).
- Produces:

```ts
export const PANE_LIMITS = { tabs: 32, history: 50, recent: 200, shownRecent: 10, kept: 4 } as const;
export type View = "rendered" | "source";
export type OpenTarget = { path: string; line?: number; col?: number; view?: View };
export type PaneAction =
  | { type: "open"; session: string; project: string; target: OpenTarget }      // a new tab, or its existing tab; shows the pane
  | { type: "navigate"; session: string; paneTab: string; project: string; target: OpenTarget } // a link inside a tab: pushes its history
  | { type: "back" | "forward"; session: string; paneTab: string }
  | { type: "setView"; session: string; paneTab: string; view: View }
  | { type: "select"; session: string; paneTab: string }
  | { type: "step"; session: string; by: 1 | -1 }                               // next or previous tab, wrapping
  | { type: "close"; session: string; paneTab: string }
  | { type: "move"; session: string; paneTab: string; to: number }              // to: the index it ends at
  | { type: "show" } | { type: "hide" } | { type: "toggle" }
  | { type: "width"; width: number | null };
export function paneReducer(s: PaneView, a: PaneAction): PaneView;
export function tabsOf(s: PaneView, session: string | null | undefined): SessionTabs;   // { tabs: [], active: null } when none
export function activeTab(s: PaneView, session: string | null | undefined): PaneTab | undefined;
export function currentEntry(t: PaneTab): PaneEntry;
export function samePath(a: string, b: string): boolean;
/** True when `open` would be refused: the session has 32 tabs and none shows this path. */
export function openRefused(s: PaneView, session: string, path: string): boolean;
/** Each tab's title: the file's name, plus " — folder" for tabs that share a name. */
export function tabTitles(tabs: readonly PaneTab[]): Map<string, string>;
/** The project's recent paths, newest first, without those open in the session: at most 10. */
export function recentFor(s: PaneView, project: string, session: string): string[];
/** The tab ids to keep loaded: `active` first, then the previous ones still open, at most 4. */
export function keptTabs(prev: readonly string[], active: string | null, open: readonly string[]): string[];
```
- `viewReducer` gains `{ type: "pane"; action: PaneAction }`, which sets `pane: paneReducer(s.pane, a.action)`.

- [ ] **Step 1: Write the failing tests** (`paneState.test.ts`), one `it` per line:
  - `an open makes a tab, selects it and shows the pane` — from the default pane: one tab, kind `"file"`, id `"t1"`, history `[{ path, line: null, col: null, view: "rendered" }]`, index 0, `active === "t1"`, `open === true`.
  - `a new tab's id is one past the highest id open in the session` — open a, b, c → `"t1"`, `"t2"`, `"t3"`; close b, open d → `"t4"`; close d and c, open e → `"t2"`.
  - `an open of a path already open selects its tab` — no second tab.
  - `a Windows path matches whatever its case and slashes` — `samePath("C:\\work\\README.md", "c:/work/readme.md")` is true; `samePath("/home/a/B.md", "/home/a/b.md")` is false.
  - `a second open with a line moves the open tab to it` — the tab's current entry gets the new `line` and `col`; its history length is unchanged.
  - `the 33rd open is refused` — state unchanged, and `openRefused` is true; an open of a path already open among 32 is not refused.
  - `navigate pushes onto the tab's history and drops what was forward` · `a history keeps its last 50 entries`.
  - `back and forward stop at the ends` · `setView changes only the current entry`.
  - `step wraps both ways and does nothing with no tabs`.
  - `closing the selected tab selects the one to its right, else the one to its left`.
  - `closing the last tab removes the session's entry and leaves the pane open`.
  - `move puts the tab at the index and keeps the selection`; an index out of range is clamped.
  - `toggle, show and hide set open`; `width keeps null and rounds a number`; a width outside 320–4096 is clamped.
  - `every open and navigate puts the file first in recent, once per project` — same path opened twice appears once; the list is cut at 200.
  - `recentFor leaves out other projects and files open in the session, and stops at 10`.
  - `tabTitles adds the folder only to tabs that share a name` — `docs/README.md` and `api/README.md` → "README.md — docs", "README.md — api"; `a.ts` → "a.ts".
  - `keptTabs keeps the active tab and the three shown before it` · `keptTabs forgets a tab that was closed`.
  - `an action for a tab or session that isn't there changes nothing` (returns the same object).
  - In `viewState.test.ts`: `a pane action reaches the pane` and `toSaved keeps the pane`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/pane/paneState.test.ts` → FAIL, module not found.

- [ ] **Step 3: Implement `paneState.ts`** and the `viewReducer` case. `samePath`: a path that starts with a drive letter or `\\` compares with `\` as `/` and lowercased; any other path compares exactly. The reducer returns the same object when nothing changes, so `useViewState` saves nothing.

- [ ] **Step 4: Run** — `pnpm test src/features/pane src/features/window/viewState.test.ts` → PASS. `pnpm build` → passes.

- [ ] **Step 5: Commit** — `feat(pane): the side pane's tabs, history and recent files are one pure reducer.`

---

### Task 3: The geometry: a saved width, the 80-column floor, the cover rule

**Files:**
- Create: `src/features/window/paneLayout.ts`, `paneLayout.test.ts`; `src/features/terminal/columns.ts`, `columns.test.ts`
- Delete: `src/features/window/previewLayout.ts`, `previewLayout.test.ts`
- Modify: `src/features/window/MainWindow.tsx:299-321, 410-450`, `MainWindow.css:207-261` (`.preview__divider` → `.pane__divider`, `[data-preview]` → `[data-pane]`), `MainWindow` stories and tests that pass `preview`

**Interfaces:**
- Produces:

```ts
// paneLayout.ts
export const PANE = { min: 320, max: 4096, step: 16, coverBelow: 720 } as const;
export type PaneMode = "hidden" | "beside" | "cover";
export type PaneGeometry = { mode: PaneMode; terminal: number; pane: number; /** The widest the pane may be dragged, for aria-valuemax. */ max: number };
/** `room`: the main area's width. `terminalMin`: the width of 80 columns. The divider is 1 px in "beside". */
export function paneGeometry(i: { room: number; shown: boolean; saved: number | null; terminalMin: number }): PaneGeometry;
/** The width to save when the divider is dragged to `fromEnd` px from the main area's end edge, or nudged. */
export function draggedWidth(i: { room: number; terminalMin: number; fromEnd: number }): number;

// columns.ts
/** The CSS width of `cols` terminal columns in this font. `measure` returns one cell's width; without a canvas it is 0.6 × size. */
export function columnsWidth(cols: number, font: { family: string; size: number }, measure?: (font: { family: string; size: number }) => number): number;
```
- `MainWindowProps`: `preview` is replaced by `pane?: ReactNode`, `paneShown?: boolean`, `paneWidth?: number | null`, `onPaneWidth?: (width: number | null) => void`, `terminalMinWidth?: number` (default 0).
- Rule (PLAN 6q-2 decision 7): hidden → all the room is the terminal's. `room < 720` → `cover` (pane = room, terminal 0, terminals stay mounted and suspended as today). Else `beside`: `max = Math.max(320, room − 1 − terminalMin)`, `pane = clamp(saved ?? Math.round((room − 1) / 2), 320, max)`, `terminal = room − 1 − pane`.

- [ ] **Step 1: Write the failing tests** (`paneLayout.test.ts`):
  - `a hidden pane leaves the room to the terminal`.
  - `with nothing saved the pane takes half the room` — room 1201, terminalMin 600 → pane 600, terminal 600.
  - `the pane stops where the terminal would drop under 80 columns` — room 1201, saved 900, terminalMin 700 → pane 500, max 500.
  - `a saved width wider than the room is shown clamped` — room 900, saved 2000, terminalMin 400 → pane 499.
  - `the terminal goes under 80 columns only when the pane is at its minimum` — room 900, terminalMin 700 → pane 320, terminal 579.
  - `under 720 px the pane covers` — room 719 → mode "cover", pane 719, terminal 0; room 720 → "beside".
  - `a drag is held between the minimum and the 80-column stop` — `draggedWidth` at fromEnd 100 → 320; at fromEnd 5000 → max.
  - In `columns.test.ts`: `80 columns of a 14 px font with an 8.4 px cell are 672 px`; `without a measure the cell is 0.6 of the size`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/window/paneLayout.test.ts src/features/terminal/columns.test.ts`.

- [ ] **Step 3: Implement** the two modules. `columnsWidth`'s default `measure` draws nothing: it sets a 2D canvas context's font and reads `measureText("M")`, as `RecordingPlayer.tsx:127` does; where `document` is undefined it uses the fallback.

- [ ] **Step 4: Wire `MainWindow`.** The `previewRatio` state goes; `paneGeometry({ room: spaceW, shown: paneShown && !viewing && !empty, saved: paneWidth, terminalMin: terminalMinWidth })` gives the grid columns as pixels (`${terminal}px 1px ${pane}px`). The divider keeps its markup and gets `aria-label="Resize the side pane"`, `aria-valuemin={320}`, `aria-valuemax={max}`, `aria-valuenow={pane}`; a drag calls `onPaneWidth(draggedWidth(...))` measured from the main area's end edge (start edge in RTL), Left and Right nudge by 16 (mirrored in RTL), a double-click calls `onPaneWidth(null)`. `data-pane="beside" | "cover"` replaces `data-preview`. Update the component's tests to the new props; add `the divider reports the pane's width in pixels` and `a cover pane has no divider`.

- [ ] **Step 5: Run** — `pnpm test src/features/window src/features/terminal/columns.test.ts` and `pnpm build`. `LiveApp` must still compile: pass `pane={previewPane}`, `paneShown={previewPane !== undefined}`, `paneWidth={null}` for now (Task 6 wires the real state).

- [ ] **Step 6: Commit** — `feat(window): the side pane's width is in pixels, with an 80-column stop for the terminal.`

---

### Task 4: Motion on every layout switch (worker `effort: high`)

**Files:**
- Create: `src/features/window/motion.ts`, `motion.test.ts`, `src/features/terminal/fitHold.ts`, `fitHold.test.ts`
- Modify: `src/features/window/MainWindow.tsx`, `MainWindow.css`, `src/features/terminal/TerminalView.tsx:273-300`

**Interfaces:**
- Produces:

```ts
// motion.ts
export const LAYOUT_MOTION = { ms: 200, curve: [0.2, 0.8, 0.2, 1] } as const;   // --dur-drawer, --ease-out
/** The curve's y at time fraction t (0 to 1), solved as CSS's cubic-bezier is. */
export function easeAt(t: number, curve?: readonly [number, number, number, number]): number;
export type Motion = { from: number; startedAt: number } | null;
/** The value to draw at `now`: `target` when idle or done, else between `from` and the live `target`. */
export function motionValue(m: Motion, target: number, now: number): { value: number; done: boolean };
/** What a render does with its motion: a changed `key` starts one from the value last drawn; anything else keeps it. */
export function nextMotion(prev: { key: string; motion: Motion; drawn: number }, key: string, now: number, reduced: boolean): Motion;
/** The animated number for a layout value. `key` names the discrete state; only a change of key moves. */
export function useLayoutMotion(key: string, target: number): { value: number; moving: boolean };

// fitHold.ts
/** Counts overlapping holds; `onRelease` runs once, when the last one ends. */
export function fitHold(onRelease: () => void): { hold(): () => void; held(): boolean };
export const FitHoldContext: React.Context<boolean>;   // true while a layout motion runs
```
- `MainWindow` runs two motions: the sidebar column (`key` = the sidebar mode, target = `sideW`) and the pane (`key` = `PaneGeometry.mode`, target = `pane`). The grid uses the animated numbers; `terminal = room − divider − animated pane`, never below 0.
- While either moves, `FitHoldContext` is true. `TerminalView` skips its resize-observer fit while it is true and fits once when it turns false.

- [ ] **Step 1: Write the failing tests.**
  - `motion.test.ts`:
    - `the curve starts at 0, ends at 1 and matches the CSS curve at the middle` — `easeAt(0) === 0`, `easeAt(1) === 1`, `easeAt(0.5)` within 0.005 of 0.946 (the curve's parameter is about 0.7245 where x is 0.5), and `easeAt` never decreases over 0, 0.1, … 1.
    - `an idle motion draws the target`.
    - `a motion draws between from and the live target, and is done after 200 ms`.
    - `a target that changes without a switch is followed at once` — same key, target 300 → 500: `nextMotion` returns null and the value is 500.
    - `a second switch starts from the value on screen` — key a→b at t=0 (from 0, target 400); at t=100 key b→a: the new motion's `from` equals the value drawn at t=100, not 0 or 400.
    - `reduced motion never starts a motion`.
    - `the constants match the tokens` — read `src/styles/tokens.css` as text (as `stickyStack.css.test.ts` reads its stylesheet): `--dur-drawer: 200ms` and `--ease-out: cubic-bezier(0.2, 0.8, 0.2, 1)`.
  - `fitHold.test.ts`: `a release runs the callback once`; `the hold ends once after overlapping motions` — two holds, released in either order → one callback, after the second.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/window/motion.test.ts src/features/terminal/fitHold.test.ts`.

- [ ] **Step 3: Implement `motion.ts` and `fitHold.ts`.** `useLayoutMotion` keeps `{ key, motion, drawn }` in a ref, calls `nextMotion` during render, and while a motion runs re-renders on `requestAnimationFrame` until `motionValue` reports done. Reduced motion is `matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.motion === "reduce"`, read when a motion would start (`LiveApp.tsx:1792` reads the same two). The first render never moves.

- [ ] **Step 4: Wire `MainWindow` and the stylesheet.**
  - The column that is moving clips its content (`overflow: clip`), and the content keeps its settled width, so nothing reflows mid-motion: the sidebar column shows `Sidebar` at its full width while either end of the motion is `expanded`, and swaps to `Rail` (or nothing, in focus mode) when the motion ends; the pane keeps its settled width, pinned to the main area's end edge, and stays rendered until a hide's motion ends (keep the last `pane` node in a ref for that).
  - `--side-w` and the main area's grid columns take the animated numbers.
  - No CSS `transition` on these tracks: the numbers are the motion.

- [ ] **Step 5: Hold the terminal's fit.** In `TerminalView`, read `FitHoldContext` into a ref; the `ResizeObserver` callback (`:282`) returns early while it is true; an effect on the context value calls `fitIfShown()` when it turns false. The host element clips (`overflow: clip`) so a terminal wider than its box during a motion is cut, not squeezed. Add a test beside the existing `TerminalView` tests if the fit path is unit-tested there; otherwise the live check's "one resize per motion" measurement is the check (Task 11, check 4).

- [ ] **Step 6: Run** — `pnpm test src/features/window src/features/terminal` and `pnpm build` → pass. `src/design/motion.test.ts` (the stylesheet gate) must still pass.

- [ ] **Step 7: Commit** — `feat(window): every layout switch moves for 200 ms, and the terminal fits once when it ends.`

---

### Task 5: The tab strip, the empty pane and the kept tabs

**Files:**
- Create: `src/features/pane/SidePane.tsx`, `SidePane.css`, `SidePane.test.tsx`, `stripKeys.ts`, `stripKeys.test.ts`, `EmptyPane.tsx`, `EmptyPane.test.tsx`, `KeptTabs.tsx`, `KeptTabs.test.tsx`, `SidePane.css.test.ts`
- Modify: `src/features/preview/Preview.tsx` (`PreviewPaneProps`, the root element, the Close button), `usePreviewFile.ts` (a `paused` option), `Preview.test.tsx`, `usePreviewFile.test.ts`

**Interfaces:**
- Consumes: `PaneTab`, `tabTitles`, `currentEntry`, `keptTabs` (Task 2); `ContextMenu` is used by Task 6, not here.
- Produces:

```tsx
// SidePane.tsx
export type StripTab = { id: string; title: string; path: string };
export type SidePaneProps = {
  tabs: StripTab[]; activeId: string | null; os: Os; children: ReactNode;   // children: <KeptTabs/> or <EmptyPane/>
  onSelect(id: string): void; onClose(id: string): void; onMove(id: string, to: number): void;
  onHide(): void; onPlus(anchor: { x: number; y: number }, opener: HTMLElement): void;
};
export function SidePane(p: SidePaneProps): JSX.Element;

// stripKeys.ts — what a key on a focused tab does. `rtl` swaps Left and Right.
export type StripKey = { type: "select"; id: string } | { type: "close"; id: string } | { type: "move"; id: string; to: number } | null;
export function stripKey(e: { key: string; altKey: boolean; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }, ids: readonly string[], focused: string, rtl: boolean): StripKey;
/** The index a dragged tab should take: the tab whose middle the pointer has passed. */
export function dragIndex(tabs: ReadonlyArray<{ left: number; right: number }>, x: number, from: number): number;

// EmptyPane.tsx
export function EmptyPane(p: { recent: string[]; onOpenFile(): void; onBrowse(): void; onRecent(path: string): void }): JSX.Element;

// KeptTabs.tsx — renders the kept tabs' panels; only `activeId`'s is shown, the rest carry `hidden`.
export function KeptTabs(p: { tabs: PaneTab[]; activeId: string; render(tab: PaneTab, active: boolean): ReactNode }): JSX.Element;
```
- `PreviewPaneProps` and the `Preview` wrapper lose `onClose` and gain `active: boolean` (the tab panel's `role`, `id` and `aria-labelledby` are `KeptTabs`' wrapper's, not the viewer's). `usePreviewFile(path, { paused })`: no polling while paused, one check when it resumes; `Preview` pauses it while not `active`. Until Task 6, `LiveApp` passes `active` and no `onClose`, so the build stays green.

- [ ] **Step 1: Write the failing tests.**
  - `stripKeys.test.ts`: `Right and Left select the neighbour and wrap` · `RTL swaps them` · `Home and End select the ends` · `Delete closes the focused tab` · `Alt+Shift+Right moves it one place and stops at the end` · `any other key, or a key with Ctrl or Meta, is not the strip's` · `dragIndex moves a tab once the pointer passes a neighbour's middle, and not before`.
  - `SidePane.test.tsx` (static markup): the root is `<section` with `aria-label="Side pane"`, `data-nav-side` and `data-drop-pane`; a `role="tablist"` with `aria-label="Tabs"`; each tab is a `<button role="tab"` with `id="pane-tab-${id}"`, `aria-controls="pane-panel-${id}"`, `aria-selected`, `title` = the path; exactly one tab has `tabindex="0"` and it is the active one; each tab has a sibling close button `aria-label="Close ${title}"` with `tabindex="-1"`; the strip ends with `aria-label="New tab"` (`aria-haspopup="menu"`) and `aria-label="Hide the side pane"` whose title holds the `preview` command's shortcut; with no tabs the tablist is not rendered and the two buttons are.
  - `a tab whose file can't be read still shows in the strip and closes` — render `SidePane` with one tab around a `PreviewPane` whose `file` is `{ phase: "error" }`: the markup holds the tab, its close button and "This file can't be previewed."
  - `EmptyPane.test.tsx`: the exact copy from Global Constraints; recent files are buttons showing the file's name and its folder; with no recent files the heading is not rendered.
  - `KeptTabs.test.tsx`: after rendering active `t1`, then `t2`, the markup holds both panels, `t1`'s with `hidden`; each panel is `role="tabpanel"` with `id="pane-panel-${id}"` and `aria-labelledby="pane-tab-${id}"`; a fifth tab pushes the oldest out.
  - `Preview.test.tsx`: no "Close preview" button; the root has no `aria-label="Preview, …"`. `usePreviewFile.test.ts`: `a paused file is not polled` and `resuming checks once`.
  - `SidePane.css.test.ts`: the stylesheet has no hex colour, has a `forced-colors: active` block that names only system colours, puts `:hover` only inside `(hover: hover) and (pointer: fine)`, and the strip sets `scrollbar-width: none`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/pane src/features/preview`.

- [ ] **Step 3: Implement.**
  - `SidePane`: selection follows focus (as the History view's tabs do, `ConversationView.tsx:405-419`): a `select` from `stripKey` calls `onSelect` and focuses that tab. A middle click on a tab closes it. A drag starts on `pointerdown` with capture and calls `onMove(id, dragIndex(...))` when the index changes. The active tab is scrolled into view (`scrollIntoView({ inline: "nearest", block: "nearest" })`) when it changes. Use `selectedProps` from `components/a11y/SelectedMark.tsx` for the selected tab.
  - `PreviewPane`: the root `<section>` keeps `className="preview"` and its key handling, drops `data-drop-preview` and its `aria-label`; the focus-on-open effect (`:123`) runs only when `active`; the Close button and `onClose` go.
  - `KeptTabs` keeps its list with `keptTabs` in a ref across renders.
  - **The look** (`SidePane.css`, after reading good-css): the strip is one row the height of `.preview__header`, on the window bar's surface. Tabs are flat, with no border and no box: the selected tab takes the pane body's background and full-strength text; the others are transparent with muted text and take the hover tint the sidebar rows use. A tab is 96 to 200 px wide and cuts its title with an ellipsis; the close button shows on the selected tab, and on hover or focus for the others. The strip scrolls sideways with no scrollbar. In forced colours the selected tab is `Highlight` / `HighlightText` and focus is an `outline`. The user judges the look in the live check.

- [ ] **Step 4: Run** — `pnpm test src/features/pane src/features/preview src/design` and `pnpm build` → pass (`noBoxedOutlines.test.ts` included).

- [ ] **Step 5: Commit** — `feat(pane): a tab strip, an empty state and kept tabs for the side pane.`

---

### Task 6: The app uses the pane (worker `effort: high`: it rewires `LiveApp`)

**Files:**
- Create: `src/features/pane/PaneContext.tsx`, `src/features/pane/dropCover.ts`, `dropCover.test.ts`, `src/features/pane/plusMenu.ts`, `plusMenu.test.ts`
- Delete: `src/features/preview/previewState.ts`, `previewState.test.ts`, `src/features/preview/PreviewContext.tsx`
- Modify: `src/features/sessions/LiveApp.tsx` (`:196-200`, `:747-760`, `:843-844`, `:1168-1187`, `:1322-1358`, `:1573-1594`, `:1600`, `:1614`, `:1634`, `:1787`), `src/features/drop/dropTarget.ts` (+ test), `src/features/terminal/TerminalView.tsx:141`, `src/features/terminal/pathLinks.ts` (the type and `previewHint`), `src/lib/keymap.ts`, `src/lib/appShortcuts.ts`, `src/features/sessions/LivePalette.tsx`, and their tests

**Interfaces:**
- Consumes: Tasks 2, 3, 5.
- Produces:

```ts
// PaneContext.tsx — replaces PreviewContext
export type OpenInPane = (target: OpenTarget) => void;
export const PaneContext: React.Context<OpenInPane>;
export function useOpenInPane(): OpenInPane;

// dropCover.ts — what covers the window, in the announcement's words, or null
export function dropCover(s: { launcherOpen: boolean; overlay: string | null; review: boolean; closeAsk: boolean }): string | null;

// plusMenu.ts
export function plusMenuItems(recent: readonly string[]): MenuItem[];   // "Open file…", "Browse…", then one row per recent file
export type PlusChoice = { kind: "openFile" } | { kind: "browse" } | { kind: "recent"; path: string } | null;
export function plusChoice(id: string, recent: readonly string[]): PlusChoice;
```
- With `previewState.ts` gone, the viewers take the generated `PaneEntry` (re-exported from `paneState.ts`) where they took `PreviewEntry`, reading a null `line` or `col` as absent, and `Preview`'s `onOpen` is an `OpenInPane`.
- `keymap.ts`: labels and new commands from Global Constraints (`pane-next-tab` has no binding until Task 7; all four new ids are in the "Window" group). `appShortcuts.ts`: `SessionShortcut` gains `{ kind: "paneTab"; by: 1 | -1 }`, `{ kind: "closePaneTab" }`, `{ kind: "browse" }`; `commandShortcut` maps the new ids. `dropTarget.ts`: `{ kind: "preview" }` becomes `{ kind: "pane" }`.

- [ ] **Step 1: Write the failing tests.**
  - `dropCover.test.ts`: launcher → "the launcher"; overlay `"palette"` → "the command palette"; `"settings"` → "Settings"; any other overlay, the review sheet or the close bar → "the dialog"; nothing → null; the launcher wins over an overlay.
  - `plusMenu.test.ts`: the first two items are "Open file…" and "Browse…"; a recent file's label is its name and folder; `plusChoice` maps each id back; an unknown id is null.
  - `dropTarget.test.ts`: a drop outside a terminal is `{ kind: "pane" }`.
  - `appShortcuts.test.ts` / `bindings.test.ts`: the new ids map to their shortcut kinds; `preview` reads "Side pane"; `problemsIn(COMMANDS, os)` is empty on all three systems.
  - `pathLinks.test.ts`: the hint is "Ctrl+click to open in the side pane" (macOS "⌘+click to open in the side pane").
  - `LiveApp.test.tsx` (as far as the existing harness reaches; follow its pattern): `Ctrl+Shift+Q with no tabs shows the empty pane`, `a session with no tabs shows the empty state while the pane is open`.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/pane src/features/drop src/lib src/features/terminal/pathLinks.test.ts`.

- [ ] **Step 3: Rewire `LiveApp`.** `usePreview` goes; the pane's state is `view.pane`, changed with `dispatch({ type: "pane", action })`.
  - `openInPane(target)`: with no session on screen announce "No session to open it in."; when `openRefused` announce the 32-tab sentence; else dispatch `open` with `project` = the focused row's `project` and announce "Opened ${name} in the side pane." It is the `PaneContext` value, and what quick open (`:1787`), History's `onOpenFile` (`:1634`) and the + menu's recent rows call.
  - The pane node: `<SidePane>` around `<KeptTabs key={focusedId}>` (so a session switch starts fresh) or `<EmptyPane>`; each kept tab renders the existing `Preview` with `entry={currentEntry(tab)}`, `active`, `canBack`/`canForward` from its `index`, `onOpen` → `navigate`, `onView` → `setView`, `onBack`/`onForward`. `recentFor(view.pane, project, focusedId)` feeds the empty pane and the + menu.
  - `MainWindow` gets `pane`, `paneShown={view.pane.open}`, `paneWidth={view.pane.width}`, `onPaneWidth` → `width`, and `terminalMinWidth={Math.ceil(columnsWidth(80, terminalFont))}` plus the terminal host's inline padding as the stylesheet sets it.
  - The `preview` shortcut (`:1174`): dispatch `toggle`; when it shows the pane, focus moves into it (the active panel's `.preview__body`, or the empty pane's first button) after the frame; when it hides it, focus goes to the terminal. The palette no longer opens for it.
  - `paneTab`: with the pane hidden and tabs open, show the pane and stop; with no tabs announce "No tabs open."; else dispatch `step`. `closePaneTab`: close the active tab and announce "Closed ${name}."; with none, "No tabs open." `browse`: Task 9 fills it in; until then it does nothing.
  - `SidePane`'s `onClose` announces "Closed ${name}."; `onMove` announces "Moved ${name} to position ${n} of ${count}."; `onHide` is the toggle; `onPlus` opens a `ContextMenu` (label "New tab", `plusMenuItems`) rendered beside the port-chip menu, outside any `[data-agent]`, and gives focus back to the + button when it closes.
  - Drops (`:749`): first `dropCover(...)`; a cover announces "Close ${cover} to open a file." and nothing else happens, for a terminal target too. For the pane: every path that `statFile` says is a file opens, in order, so the last is selected; none is a file → "A folder can't be shown in the side pane." The hover hint (`:843`) reads `[data-drop-pane]` and "Drop to open in the side pane"; with the pane hidden the hint has no element to mark, as today.
  - `runFromPalette` (`:1355`): the kinds that wait for the palette to hand focus back now include `paneTab`, `closePaneTab` and `browse`.

- [ ] **Step 4: Run** — `pnpm test` (the whole suite: this task touches shared modules) and `pnpm build` → pass. `rg -i "preview" src --glob '!src/features/preview/**'` shows no user-visible "Preview" string left (identifiers such as the `preview` command id and `features/preview` imports stay).

- [ ] **Step 5: Commit** — `feat(pane): sessions keep their own tabs in the side pane, with a + menu, recent files and an empty state.`

---

### Task 7: The keys: Ctrl+Tab, and Ctrl+Shift+Tab between the terminal and the pane

**Files:**
- Modify: `src/lib/keymap.ts` (`Binding`, `shortcutLabel`, `pane-next-tab`), `src/lib/bindings.ts` (`chordOf`), `src/lib/navigation/types.ts`, `machine.ts`, `src/features/sessions/navLayout.ts`, `useNavigation.ts`, `LiveApp.tsx` (`:1125`, `:1160`), the shortcuts sheet's rows, and the tests beside each

**Interfaces:**
- `Binding` gains `ctrl?: boolean`: Ctrl with the key and nothing else, on every OS. `chordOf` → `{ ctrl: true, shift: false, alt: false, meta: false }`; `shortcutLabel` → "Ctrl+Tab" (macOS "⌃Tab"). `pane-next-tab` binds `{ code: "Tab", ctrl: true }`.
- Navigation: `Target` gains `{ kind: "side" }`; `NavContext` gains `side: string | null` (null: the pane is not on screen; else its spoken name, "the side pane"); `NavState` gains `selectedSide: boolean` (false in `INITIAL_NAV`); `NavEffect` gains `{ type: "select-side" }`. `useNavigation`'s options gain `side: string | null` and `sideFocused: boolean`.
- The Tab chord's order is the terminals on screen, then the side pane; from the side pane it goes to the first terminal. With the pane hidden the order is the terminal alone, as today.

- [ ] **Step 1: Write the failing tests.**
  - `bindings.test.ts`: `Ctrl+Tab is Next tab on every system` — an event `{ code: "Tab", ctrlKey: true }` finds `pane-next-tab` on windows, mac and linux, and `{ code: "Tab", ctrlKey: true, shiftKey: true }` still finds `nav-next-pane`; `problemsIn` stays empty; `shortcutLabel` gives "Ctrl+Tab" and "⌃Tab".
  - `machine.test.ts`: `Tab goes from the terminal to the side pane and back` (effects `select-side` then `select-pane` 0; announcements "Selected the side pane", then the session's); `with the pane hidden Tab selects the terminal as before`; `with a cursor out, Tab carries it to the side pane and the release selects it`; `an arrow can carry the cursor onto the side pane` (layout holds a `side` rect to the right; announcement "Cursor on the side pane"); `selecting a terminal clears selectedSide`.
  - `navLayout.test.ts` (or where the selector is tested): an element with `data-nav-side` is the `side` target.

- [ ] **Step 2: Run to see them fail** — `pnpm test src/lib`.

- [ ] **Step 3: Implement.** In `machine.ts` the Tab case walks one ordered list of targets instead of `nextVisible` alone; `commit` handles `side`; `moveWith` starts from the side pane when `selectedSide`. `navLayout.ts`'s selector adds `[data-nav-side]`. `LiveApp` passes `side` (`"the side pane"` while `PaneGeometry.mode !== "hidden"`), tracks `sideFocused` from `focusin`/`focusout` on the pane's element, and on `select-side` focuses what the `preview` shortcut focuses (Task 6). The key handler needs no change: `sessionShortcut` finds `pane-next-tab` through `findCommand`, so the terminal's handler (`TerminalView.tsx:183-194`) lets go of Ctrl+Tab by itself. If the Keyboard shortcuts page lets a person rebind keys, the `ctrl` flavour must show and survive a round trip there; say so in the report if it can't.

- [ ] **Step 4: Run** — `pnpm test src/lib src/features/sessions src/features/window` and `pnpm build` → pass.

- [ ] **Step 5: Commit** — `feat(keys): Ctrl+Tab shows the next tab, and Ctrl+Shift+Tab moves between the terminal and the side pane.`

---

### Task 8: Ctrl+click on a web address in terminal output (worker `effort: high`: a parser)

**Files:**
- Create: `src/features/terminal/urlLinks.ts`, `urlLinks.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx:181` (register it before the path provider; `terminalOptions` for `linkHandler`)

**Interfaces:**
- Consumes: `logicalLine`, `BufferLike` from `pathLinks.ts`; `openUrl` from `@tauri-apps/plugin-opener` (as `Preview.tsx:144` uses it).
- Produces:

```ts
/** The http and https addresses in a line of output: `start` inclusive, `end` exclusive, in UTF-16 units. */
export function findUrls(text: string): Array<{ start: number; end: number; url: string }>;
export const urlHint = (platform: Endpoint["platform"]): string;
export function urlLinkProvider(term: { buffer: { active: BufferLike }; element?: HTMLElement | undefined }, platform: Endpoint["platform"], open?: (url: string) => Promise<void>): ILinkProvider;
```

- [ ] **Step 1: Write the failing tests** (`urlLinks.test.ts`):
  - `finds http and https addresses and nothing else` — `file:///c:/x`, `javascript:alert(1)`, `ftp://h/x`, `www.example.com` and `localhost:3000` give nothing.
  - `leaves out a full stop, comma or bracket that ends the sentence` — "see https://example.com/a." → `https://example.com/a`; "(https://example.com/a)" → without the bracket; "https://en.wikipedia.org/wiki/Rust_(language)" keeps its balanced bracket; a trailing `"`, `'`, `>`, `]`, `;`, `:`, `!`, `?` is left out.
  - `keeps a port, a query and a fragment` — `http://127.0.0.1:5173/a?b=c#d`.
  - `finds two addresses on one line`.
  - `an address longer than 2048 characters is not a link`.
  - `needs a host` — "https://" alone gives nothing.
  - `a wrapped address is one link across its rows` — a fake buffer of two rows, the second `isWrapped`: `provideLinks` for either row returns one link whose range starts on the first row and ends on the second.
  - `only Ctrl+click opens it (Cmd on macOS)` — `activate` with no modifier calls nothing; with the modifier it calls `open` with the address.
  - `the hint names the browser` — "Ctrl+click to open in your browser", "⌘+click to open in your browser".

- [ ] **Step 2: Run to see them fail** — `pnpm test src/features/terminal/urlLinks.test.ts`.

- [ ] **Step 3: Implement.** The link looks and behaves like a path link (`pathLinks.ts:94-108`: underline on hover, the hint as the element's title, cleared on leave). Register it before the path provider, so a path-like stretch inside an address belongs to the address. An OSC 8 hyperlink follows the same rule: set xterm's `linkHandler` in `terminalOptions` so its `activate` needs the modifier, opens only http and https, and uses `openUrl`; if a `linkHandler` is already set, extend it. A failed `openUrl` is swallowed, as in `Preview.tsx:144`.

- [ ] **Step 4: Run** — `pnpm test src/features/terminal` and `pnpm build` → pass.

- [ ] **Step 5: Commit** — `feat(terminal): Ctrl+click on a web address in the output opens the system browser.`

---

### Task 9: Browse…

**Files:**
- Create: `src-tauri/src/file_picker.rs`, `src/lib/filePicker.ts`, `src/lib/filePicker.test.ts`
- Modify: `src-tauri/src/folder_picker.rs` (`start_folder` becomes `pub(crate)`), `src-tauri/src/main.rs` (the module and the `invoke_handler` entry beside `pick_folder`), `src/features/sessions/LiveApp.tsx` (the `browse` shortcut and the + menu's and the empty pane's Browse…)

**Interfaces:**
- Produces: `#[tauri::command] pub async fn pick_file(window: tauri::Window, start: Option<String>) -> Result<Option<String>, String>` (the native open-file dialog, modal to the window, starting in `start` when it is an existing folder; `None` on cancel) and `export async function pickFile(start: string | null): Promise<string | null>` (`invoke("pick_file", { start })`).

- [ ] **Step 1: Write the failing test** `filePicker.test.ts`, modelled on `folderPicker.test.ts`: `pickFile("C:\\work")` invokes `pick_file` with `{ start: "C:\\work" }` and returns the path; a cancel returns null.

- [ ] **Step 2: Run to see it fail** — `pnpm test src/lib/filePicker.test.ts`.

- [ ] **Step 3: Implement.** `file_picker.rs` mirrors `folder_picker.rs` with `blocking_pick_file()`; no file-type filter. No capability changes: like `pick_folder`, it is the app's own command. In `LiveApp`, `browse` calls `pickFile(focusedInfo?.cwd ?? null)` and passes a picked path to `openInPane`; a rejected call announces "Couldn't open the file dialog." A file the core refuses to read shows the Preview's own message in its tab.

- [ ] **Step 4: Run** — `pnpm test src/lib/filePicker.test.ts` · `cargo test -p marshell-app -j 4 picker` · `cargo clippy -p marshell-app --all-targets -j 4 -- -D warnings` · `pnpm build` → pass.

- [ ] **Step 5: Commit** — `cargo fmt --all`, then `feat(pane): Browse… opens any file through the system's file dialog.`

---

### Task 10: The plan's binding text follows

**Files:**
- Modify: `docs/PLAN.md` ("Layout", "Keyboard navigation and shortcuts", the slice 2.5 decision lines the new decisions replace, "Microcopy" if it names the Preview), the shortcuts sheet's and the review page's copy if they still say "Preview" (`rg -i "preview" scripts src/features/shortcuts docs/a11y`)

- [ ] **Step 1:** In "Layout" and "Keyboard navigation and shortcuts", describe the side pane as built: one switch, the pixel width with the 80-column stop, the cover rule under 720 px, the 200 ms motion and its reduced-motion rule, Ctrl+Shift+Tab's cycle, Ctrl+Tab, the strip's keys. Add "(Replaced by slice 6q-2 decision N.)" to each older decision line a 6q-2 decision replaces (2.5's "one file at a time", "every opening starts at half", "Ctrl+Shift+Q opens the palette with nothing open", 6q-1 decision 2's label); don't rewrite the old wording.
- [ ] **Step 2:** `pnpm test` and `pnpm build` if any source copy changed.
- [ ] **Step 3: Commit** — `docs(plan): the layout and keyboard sections describe the side pane.`

---

### Task 11: Live check (main thread, `marshell-live-check` skill)

Findings go in `docs/research/2026-11-10-side-pane-live-check.md`. Scratch homes for Marshell and every CLI; stop only what the check started, by PID.

1. **Tabs per session and restart.** Two shell sessions; three tabs in one (a Markdown file, an image, a PDF), one in the other. Switching sessions swaps the strip and never changes the terminal's column count. Quit and start again: the tabs, the selected tab, the pane's switch and its width are back.
2. **Every way in.** Ctrl+click on a path in output; quick open; the + menu's recent row; the empty pane's buttons; a second open of an open file selects its tab. By the user's hand: Browse… and a drop from Explorer (on the pane, on a terminal, and with the launcher open: nothing opens and the sentence is announced).
3. **Width.** Drag the divider and use the arrow keys; the divider stops at 80 columns (read the terminal's `cols`); narrow the window until the pane is 320 px, then until it covers; widen it again: the saved width returns.
4. **Motion, measured.** For Ctrl+Shift+Q, the sidebar toggle, focus mode and a window resize across 960 px and across 720 px of room: the layout value over the frames (about 200 ms, no jump at either end), and the count of PTY resizes per switch (expected: 1). With `data-motion="reduce"`: one frame. Two toggles 80 ms apart: no jump, one resize at the end. No blank terminal frame during any of them (the method of `docs/research/2026-11-09-resize-fixes-live-check.md`).
5. **Keys.** Ctrl+Shift+Tab from a terminal lands in the pane and the next one returns; Ctrl+Tab from the terminal steps the tabs; Left, Right, Home, End, Delete and Alt+Shift+arrows in the strip; Esc in a tab returns to the terminal. Everything in checks 1 to 3 that isn't a drag is done once with the keyboard alone.
6. **Web address.** A scratch local server's `http://127.0.0.1:<port>/` printed in a terminal: hover shows the hint, a plain click does nothing, Ctrl+click opens the system browser. One address only, and only this loopback one.
7. **Forced colours** (emulated over CDP) and a 125% scale: a screenshot of the strip with a selected and a focused tab, and of the empty pane.
8. **An old file.** A `layout.json` written by `main` before this branch (with the three dead fields, one session selected) loads with its sidebar and selection intact and an empty pane.
9. **The user's judgement:** the motion's feel (decision 6) and the strip's look.

- [ ] Run the checks, write the findings, fix what fails (each fix with its test), commit `docs(research): the side pane's live check.`

---

## After the tasks

The whole-branch review runs on Opus (`marshell-reviewer`, with this plan's Review Focus), then one fix pass, then `marshell-ship`: the PR body has "What it does", "Live check", "Known limits" and "Checks". Known limits to carry into it unless the work removes them: a tab's place is lost after a session switch and beyond the four kept tabs; the pane's page and Rust's half of the layout rule are 6q-3's; a build from before 6q-2 reads the new file as defaults.
