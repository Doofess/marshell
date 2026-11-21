# Slice 6r-1a: the side pane becomes the Browser and Files panels

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The side pane's one tab strip becomes two panels, Browser (web pages and HTML files) and Files (every other file), each with its own title-bar button, key, tab strip, selected tab and empty state; they show alone or stacked with a movable divider, one can fill the pane, and the Browser panel gets a ⋯ menu with Copy link, Open in the system browser, Open HTML file… and Clear browsing data….

**Architecture:** A session keeps its one ordered list of tabs. Which panel a tab is in is derived from what it shows (`panelOf`), so nothing about a tab is saved twice. The window's stack (`view.pane.panels`: the panels shown, their height weights, the filled panel) is new, optional state in `layout.json`, and each session gains the Browser panel's selected tab (`active_browser`); `active` becomes the Files panel's. The pure `paneReducer` owns every rule; `PanelStack` lays the panels out from pure maths in `stackLayout.ts` with the app's own 200 ms motion; `SidePane` becomes one panel's frame and is drawn once per panel; the navigation machine gets one Ctrl+Shift+Tab stop per panel.

**Tech Stack:** Rust (`marshell-protocol` serde + ts-rs types, `marshell-core` layout repair and validation, one Tauri command in `src-tauri`), React + TypeScript, Vitest through `renderToStaticMarkup`, plain CSS with the project's tokens.

**Spec:** `docs/PLAN.md` §9, "Slice 6r decisions (2026-11-20)" (1, 3, 6 to 11) and "Slice 6r-1a decisions (2026-11-21)" (1 to 10). It builds on 6q-2 decisions 4, 6 and 13, 6q-3 decisions 4, 12 and 14, and resize-polish decision 3.

**Checked before planning (2026-11-21, on `origin/main` eddce674, by four readers and then by hand):**
- `PaneView` (`crates/protocol/src/api.rs:525`) is `{ open, width, sessions, recent }`; `SessionTabs` (`:509`) is `{ tabs, active }`. `ViewLayout.pane` is `#[serde(default)]`. `PaneEntry.page` shows the optional-field pattern: `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]`, which leaves every existing TypeScript fixture valid.
- `crates/core/src/layout.rs`: `repair_pane` (`:212`) drops what can't be drawn and resets a session's `active` that names no tab; `validate_pane` (`:329`) is the same rules as errors naming the field. Unknown JSON fields are ignored on read.
- `src/features/pane/paneState.ts` (401 lines) is the whole reducer. `open` and `openPage` set `open: true`; `openBackground` never does and selects only in a session with no tabs; `select`, `step`, `close`, `move` and `seen` work on the session's one list and one `active`.
- The readers of "the selected tab": `LiveApp.tsx` `shownPreview` (`:710`), `shownPdf` (`:728`), `askToClose` (`:859`), the seen effect (`:938`), `openPageInPane` (`:990`), `shownPage` (`:1565`), `selectRegion` (`:1594`), `paneActive` (`:2029`); `useBrowserPages.ts` `popupActions` (`:38`); `paneCommands.ts` `closeTabOutcome`.
- `SidePane.tsx` is the frame: `<section className="side-pane" aria-label="Side pane" data-nav-side="" data-drop-pane="">`, a strip (tablist `aria-label="Tabs"`, tabs `id="pane-tab-<id>"`, a + button `aria-label="New tab" aria-haspopup="menu"`, a "Hide the side pane" button), then the body. `focusPane.ts`'s selectors are rooted at `[data-nav-side]` and find the + button by its `aria-label`.
- `MainWindow.tsx:454` draws the one title-bar button (`window__pane-toggle`, `data-nav-top="pane"`, `aria-expanded`, `aria-disabled`, `data-news`, `SidePaneIcon`). `paneWanted = paneShown && !viewing && !empty`; `usePaneHold` holds a hide until a page has frozen; `LayoutContext` (`paneColumn.ts`) publishes `{ inputs, moving }`.
- A page's place: `usePageSlot.ts` measures the slot against `.window` and reports `{ rect, win, rule, moving }`; the rule's `top` and `bottom` come from the measurement, a `ResizeObserver` on the slot re-reports, and while `LayoutContext.moving` is true it reports every frame. Rust's `Placer::on_report` takes a `moving` report as measured. Nothing assumes the slot is as tall as the pane.
- `useLayoutMotion(key, target)` (`src/features/window/motion.ts:74`) animates a number for 200 ms when `key` changes, follows `target` at once otherwise, and honours reduced motion.
- Navigation: `Target` has `{ kind: "side" }`, `NavContext.side?: string | null`, `NavState.selectedSide: boolean`, effect `{ type: "select-side" }` (`src/lib/navigation/types.ts`, `machine.ts:35-62`, `:117-130`); `navLayout.ts:6,13` reads `[data-nav-side]`; `useNavigation.ts:54` `TOP_NAMES` has `pane: "Side pane"`.
- Keys: a default binding is Ctrl+Shift+key on Windows and Linux and **⌘+key on macOS** (`keymap.ts:25-33`). `bindings.ts` `RESERVED` holds ⌘A (Select all) and ⌘H (Hide) on macOS; Ctrl+Shift+A, D and H are free on Windows and Linux; ⌘D is free on macOS. `bindings.test.ts:158,189` fail on a reserved or duplicate default.
- `ContextMenu`'s `MenuItem` is `{ id, label, shortcut?, disabled?: string, submenu? } | { separator: true }`; a disabled row stays reachable and shows its reason; the menu calls `useCoverPage`.
- `pickFile(start)` (`src/lib/filePicker.ts:4`) invokes `pick_file(window, start)` (`src-tauri/src/file_picker.rs:8`), which sets no filter. The clipboard is written with `navigator.clipboard.writeText` (no helper). `openUrl` is allowed for `http://*` and `https://*` only (`src-tauri/capabilities/default.json:14-17`).
- A session's ports are `RowModel.ports?: PortChip[]` (`{ port, url }`); `focusedRow` is `LiveApp.tsx:552`. `kindOf(path) === "html"` for `.html` and `.htm` (`preview/kinds.ts:20`).
- User-facing "side pane" strings and the tests that pin them are listed in Task 8.

## Global Constraints

- **Scope.** No file tree, no Changes panel, no Save screenshot, viewport sizes, cookie setting, Ctrl+click setting or auto-open (6r-1b, 6r-2, 6r-3). No new dependency, no CSP or capability change, no new route.
- **`layout.json` stays version 1.** Every new field is optional on read. An old file loads; an older build ignores the new fields.
- **Panel ids and order:** `"browser"` then `"files"`, top to bottom. Labels: "Browser", "Files". A tab's panel: a browser tab, or a file tab whose current path `kindOf` calls `"html"`, is in Browser; every other tab is in Files. The same on every OS.
- **Limits.** 32 tabs per session over both panels; 4 kept tabs per panel; a panel is at least 160 px tall; the stack's divider is 1 px; a key step is 16 px; a weight is 1 to 100000 and 10000 when absent.
- **Keys.** `browser-panel`: `{ code: "KeyA", mac: { code: "KeyA", alt: true } }`. `files-panel`: `{ code: "KeyD" }`. Ctrl+Shift+Q (`preview`) keeps toggling the whole pane.
- **Copy (exact).** "Opened README.md in Files." · "Opened localhost:3000 in Browser." · "Claude Code opened report.html in Browser." · "No tabs open in Browser." · "Browser is hidden." · "The side pane shows beside a session" · "Not available on this system yet" · "No other panel is shown" · "Resize Browser and Files" · "Copied the link." · "Couldn't copy the link." · "No page is open" · "An HTML file has no address". The empty states' copy is in Task 5; the menu's rows in Task 9. Sentence case, no "!".
- **Frontend rules** (`.claude/rules/ui.md`): tokens only, no hex; no boxed outlines; `:hover` only under `(hover: hover) and (pointer: fine)`; a `forced-colors` block names system colours only; every control works with the keyboard alone; `Announcer` is the only live region; components are tested through `renderToStaticMarkup`. Read the `good-css` skill before writing declarations.
- **Protocol rules** (`.claude/rules/protocol.md`): `#[derive(TS)] #[ts(export)]`; never hand-edit `src/generated/`; `cargo test -p marshell-protocol -j 4` regenerates, then `pnpm check:generated`.
- **Checks.** Rust: `cargo test -p <crate> -j 4`, `cargo fmt --all` before each Rust commit. Frontend: `pnpm test <path>`, and `pnpm build` at the end of every frontend task. Commit messages read `type(scope): what changed, as a sentence.`
- **Workers** never run `git checkout`, `git restore` or `git stash` on a file they don't own, never touch `hand-keys-1.json`, and never start `pnpm tauri dev`.
- **Between Task 2 and Task 8** the app compiles and its tests pass, but the pane on screen is not right. Nothing is live-checked before Task 8 is in.

## Review Focus

1. **An old `layout.json` with the pane open and mixed tabs.** The first start shows the panels that hold tabs for the session on screen, every tab is in one of them, and the tab that was selected is still selected in its panel. (Tasks 1, 2: `an_old_pane_reads_with_no_panels`, `a layout with no panels shows the panels that hold tabs`, `the old active tab is the selection of whichever panel holds it`.)
2. **A session switch never changes the panels.** Another session with no web tabs shows the Browser panel's empty state, not one panel fewer; a fill stays. (Task 2: `the stack is the window's: another session shows the same panels`; Task 10 live.)
3. **A pane too short for two 160 px panels** splits evenly, nothing overflows, and the divider's keys do nothing harmful. (Task 3: `a pane too short for the minimum splits evenly`, `a drag in a pane too short changes nothing`.)
4. **A page never draws over Files and is never left behind.** While the stack moves, fills or a panel hides, the page follows its slot and is hidden with its panel. (Task 3: `publishes moving to the page's slot while a row moves`; Task 10 live, with screenshots.)
5. **`marshell open` into a panel that isn't on screen** shows nothing, moves no focus, puts the dot on that panel's button only, and counts toward the 32. (Task 2: `a background open never changes the stack`, `news names the panel`; Task 10 live.)
6. **The keyboard is never left on something that went away:** a panel hidden while its ⤢, × or the divider has focus. (Task 8: `a hide from inside the panel gives the terminal the keyboard`; Task 10 live.)

---

### Task 1: `layout.json` holds the stack and the Browser panel's selected tab

**Files:**
- Modify: `crates/protocol/src/api.rs` (near `:509-535`), `crates/core/src/layout.rs` (`repair_pane`, `validate_pane`, their tests), any Rust struct literal of `PaneView` or `SessionTabs` the compiler names
- Regenerated: `src/generated/Panels.ts`, `PaneView.ts`, `SessionTabs.ts`
- Test: `crates/core/src/layout.rs` (unit tests), `crates/core/tests/layout_api.rs`

**Interfaces — Produces:**
```rust
/// The side pane's stack of panels.
pub struct Panels {
    /// The panels shown while the pane is open: "browser" and "files", in that order, at least one.
    pub shown: Vec<String>,
    /// Each panel's weight in the stack's height: 1 to 100000; a panel with none has 10000.
    #[serde(default)] pub weights: BTreeMap<String, u32>,
    /// The shown panel that fills the pane, hiding the others.
    #[serde(default)] pub filled: Option<String>,
}
// PaneView gains:
#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub panels: Option<Panels>,
// SessionTabs gains (the Browser panel's selected tab; `active` is the Files panel's):
#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub active_browser: Option<String>,
```
TypeScript then has `PaneView.panels?: Panels` and `SessionTabs.active_browser?: string`.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s test module:
  - `an_old_pane_reads_with_no_panels`: a view JSON whose `pane` has only the four old fields deserialises with `panels == None`, and a session's `active_browser == None`.
  - `repair_keeps_a_good_stack`: `shown ["browser","files"]`, `weights {browser: 6000, files: 14000}`, `filled Some("files")` is unchanged.
  - `repair_orders_and_dedupes_shown`: `["files","browser","files"]` → `["browser","files"]`.
  - `repair_drops_an_unknown_panel`: `["changes","files"]` → `["files"]`; `["changes"]` → `panels == None`.
  - `repair_drops_a_bad_weight`: a key `"tree"`, a value `0` and a value `100001` are removed; a good one stays.
  - `repair_clears_a_fill_that_cannot_be`: `filled Some("browser")` with `shown ["files"]` → `None`; `filled Some("files")` with `shown ["files"]` → `None` (one panel can't fill).
  - `repair_clears_an_active_browser_that_names_no_tab`: → `None`; one that names a tab stays. The existing `active` repair is unchanged.
  - `validate_names_the_stack_field`: each of the cases above (unordered or duplicate `shown`, empty `shown`, unknown id, bad weight, impossible fill, `active_browser` naming no tab) is `Err` whose text starts with `pane.panels.shown`, `pane.panels.weights.<key>`, `pane.panels.filled` or `pane.sessions.<id>.active_browser`.
  - In `layout_api.rs`: `put_view_keeps_the_stack` (a PUT with a good `panels` and `active_browser` reads back the same from GET) and `put_view_refuses_a_bad_stack` (400 naming `pane.panels.shown`).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 layout` → compile errors for the missing fields.
- [ ] **Step 3: Implement.** Add the types; add `const PANEL_IDS: [&str; 2] = ["browser", "files"]` and `const PANEL_WEIGHT: RangeInclusive<u32> = 1..=100_000` to `layout.rs`; `repair_pane` gets a `repair_panels(&mut pane.panels)` and the `active_browser` line; `validate_pane` the matching errors.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 layout`, `cargo test -p marshell-core -j 4 --test layout_api --test layout_restore`, `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `pnpm build` → all pass.
- [ ] **Step 5: Commit** — `cargo fmt --all`, then `feat(layout): the side pane's stack of panels and the Browser panel's selected tab are saved.`

### Task 2: The rules, as pure functions (worker `effort: high`)

**Files:**
- Create: `src/features/pane/panels.ts`, `src/features/pane/panels.test.ts`
- Modify: `src/features/pane/paneState.ts`, `paneCommands.ts`, `opens.ts`, `paneNews.ts`, `plusMenu.ts` and their tests; `src/features/browser/useBrowserPages.ts` (`popupActions`) and its test
- Modify, only to keep compiling: `src/features/sessions/LiveApp.tsx`

**Interfaces — Consumes:** Task 1's generated types.

**Interfaces — Produces:**
```ts
// panels.ts — imports nothing from paneState.ts (paneState imports it)
export const PANELS = ["browser", "files"] as const;
export type PanelId = (typeof PANELS)[number];
export const PANEL_LABEL: Record<PanelId, string> = { browser: "Browser", files: "Files" };
export const PANEL_COMMAND: Record<PanelId, string> = { browser: "browser-panel", files: "files-panel" };
export const DEFAULT_WEIGHT = 10_000;
export function isPanelId(s: string): s is PanelId;
export function panelOfPath(path: string): PanelId;            // a page address or an .html/.htm file → "browser"
export function panelOf(tab: PaneTab): PanelId;                // kind "browser" → "browser"; else panelOfPath(its current path)
export type Stack = { shown: PanelId[]; weights: Partial<Record<PanelId, number>>; filled: PanelId | null };
export function panelsWithTabs(st: SessionTabs): PanelId[];    // in the stack's order
export function stackOf(pane: PaneView, session: string | null | undefined): Stack;
export function onScreen(pane: PaneView, session: string | null | undefined): PanelId[];
export function panelTabs(pane: PaneView, session: string | null | undefined, panel: PanelId): PaneTab[];
export function activeIn(st: SessionTabs, panel: PanelId): string | null;
```
`paneState.ts`: `activeTab(s, session, panel: PanelId)`; `keptTabs` and `drawnTabs` unchanged. `PaneAction` changes:
```ts
| { type: "step"; session: string; panel: PanelId; by: 1 | -1 }
| { type: "move"; session: string; paneTab: string; to: number }      // `to`: the index among its panel's tabs
| { type: "seen"; session: string; panel: PanelId }
| { type: "openPage"; session: string; url: string; background?: true }
| { type: "show"; session: string | null } | { type: "hide" } | { type: "toggle"; session: string | null }
| { type: "panel"; panel: PanelId; session: string | null }           // a panel's key, button or ×
| { type: "fill"; panel: PanelId; session: string | null }            // ⤢
| { type: "weights"; weights: Partial<Record<PanelId, number>> }      // the divider
| { type: "pin"; session: string | null }                              // write the stack down when none is saved
```
`paneCommands.ts`: `openedText(name: string, panel: PanelId)`, `openedByText(who, name, panel)`, `pageOpenedText(url)` (always Browser), `noTabsText(panel)` = `No tabs open in ${label}.`, `panelHiddenText(panel)` = `${label} is hidden.`, `paneTabOutcome(pane, session, panel: PanelId | null): "none" | "show" | "step"`, `closeTabOutcome(pane, session, panel)`, `panelToggleOutcome(pane, session, panel): { shown: boolean; paneHides: boolean }` (what `{ type: "panel" }` is about to do), and
```ts
/** The panel a tab command acts on: the one with the keyboard; else the last one that had it, when it is on screen and holds tabs; else the first on screen that holds tabs; else the first on screen. */
export function commandPanel(on: readonly PanelId[], focused: PanelId | null, last: PanelId | null, holdsTabs: (p: PanelId) => boolean): PanelId | null;
```
`opens.ts`: `OpensContext.paneOpen` becomes `onScreen: readonly PanelId[]` (the panels in front of the person for the session `shown`); `TakenOpens.news: { session: string; panel: PanelId }[]`. `paneNews.ts`: `NewsAction` is `{ type: "add"; items: readonly { session: string; panel: PanelId }[] } | { type: "seen"; session: string; panel: PanelId } | { type: "closed"; session: string }`, plus `panelHasNews(news: ReadonlySet<string>, panel: PanelId): boolean`. `plusMenu.ts`: `plusMenuItems(recent)` (no browser row), `PlusChoice` loses `browserTab`.

**The rules the tests pin** (each line is a test in `panels.test.ts` or `paneState.test.ts`, named as written):
- `a web page and an HTML file are in Browser, every other file in Files` — `https://a.test`, `about:blank`, `C:\\p\\index.html`, `/p/A.HTM` → browser; `README.md`, `a.pdf`, `a.png`, `x.htmlx` → files.
- `the selection is per panel` — `activeIn`: the panel's own field when it names a tab of that panel; else the other field when it does; else the panel's first tab; else null.
- `the old active tab is the selection of whichever panel holds it` — `{ active: "t2" }` with t2 a browser tab and no `active_browser`: Browser's selection is t2, Files' is its first tab.
- `a layout with no panels shows the panels that hold tabs` — `stackOf` with `panels` absent: `["browser","files"]`, `["browser"]`, `["files"]` by the session's tabs, and `["files"]` for none or no session; `weights {}`, `filled null`. A saved stack is returned cleaned (known ids, in order), whatever the session holds.
- `the stack is the window's: another session shows the same panels` — with a saved stack, `onScreen` is the same for two sessions with different tabs.
- `on screen is nothing while the pane is hidden, the filled panel alone while one fills`.
- `an open selects the tab in its panel, shows the pane and adds the panel` — pane open with `shown ["browser"]`, a file opens: `shown ["browser","files"]`, `active` is the tab, `active_browser` untouched.
- `an open into another panel ends a fill; into the filled panel keeps it`.
- `an open while the pane is hidden shows that panel alone` — `shown` becomes `[panel]`, `filled null`, weights kept.
- `an open of the tab already selected in a panel on screen returns the same state`.
- `the 33rd tab is refused whatever its panel` — 32 tabs across both panels.
- `a link to a file of the other panel opens a tab there` — `navigate` from a Markdown tab to `page.html` leaves that tab's history alone and behaves as `open`.
- `Back onto an entry of the other panel takes the tab there, selected, with its panel shown` (a tab saved by an older build).
- `select sets its panel's selection only`; `step wraps inside the panel`; `move reorders inside the panel and leaves the other panel's tabs where they were`.
- `closing a panel's selected tab selects its neighbour in that panel`; `closing a panel's last tab leaves the panel shown`; `closing the session's last tab forgets the session`.
- `a background open never changes the stack` — `openBackground` and `openPage` with `background: true` leave `open` and `panels` identical, and select the tab only when its panel had none.
- `seen clears the mark of that panel's selected tab only`.
- `Ctrl+Shift+Q shows the remembered panels, and writes the stack down when none was saved` (`toggle` and `show` with `panels` absent store `stackOf`); `hide keeps the stack`.
- `a panel's toggle`: shown and on screen with another → it leaves `shown`; the only one shown → `open: false` and `shown` still `[panel]`; not on screen while the pane is open → added, a fill ended; the filled one → it leaves `shown`, `filled null` (the others are back); pane hidden → that panel alone.
- `fill`: sets `filled` when two are shown; pressed again clears it; with one shown, or the pane hidden, returns the same state.
- `weights are clamped to 1..100000 and rounded; an unknown key is dropped`.
- `pin writes the stack once` — a second `pin` returns the same state.
- `paneCommands.test.ts`: the new strings exactly; `commandPanel` (four cases, one per clause); `paneTabOutcome` is `"none"` when `panel` is null or holds no tabs while the pane is open, `"show"` when the pane is hidden and the session has any tab.
- `opens.test.ts`: `news names the panel` (an HTML file → browser, a `.md` → files); `a request for a panel on screen makes no news`; the spoken lines name the panel.
- `paneNews.test.ts`: add, seen for one panel leaves the other, closed drops both.
- `useBrowserPages` test: `a popup from a session that isn't on screen opens in the background` — `popupActions(..., onScreen: false, ...)` is exactly `[{ type: "openPage", session, url, background: true }]`.

- [ ] **Step 1: Write the failing tests** above, and update the existing tests of these files to the new signatures (their titles stay).
- [ ] **Step 2: Run** `pnpm test src/features/pane src/features/browser/useBrowserPages.test.ts` → FAIL (missing exports).
- [ ] **Step 3: Implement.** One private `reveal(s, session, panel)` in `paneState.ts` carries "open, pin, add the panel, end another's fill, alone when hidden"; `open`, `openPage` (not `background`), the cross-panel `navigate` and the cross-panel `back`/`forward` call it.
- [ ] **Step 4: Keep the app compiling.** In `LiveApp.tsx` only: pass `session: stateRef.current.focusedId ?? null` to `toggle`/`show`; for `step`, `seen`, `activeTab`, `paneTabOutcome`, `closeTabOutcome`, `openedText`, `openedByText` pass the panel of the tab in hand (`panelOf(tab)`), or of the session's first tab, or `"files"`; give `takeOpens` `onScreen: viewRef.current.pane.open && mainView === "sessions" ? PANELS : []`; map news items to the new action; `plusMenuItems(paneRecent)`. Mark each with `// 6r-1a Task 8 replaces this.`
- [ ] **Step 5: Run** `pnpm test src/features/pane src/features/browser src/features/sessions src/features/window` and `pnpm build` → pass.
- [ ] **Step 6: Commit** — `feat(pane): tabs belong to the Browser or Files panel, and the stack has its rules.`

### Task 3: The stack: heights, the divider and the motion (worker `effort: high`)

**Files:**
- Create: `src/features/pane/stackLayout.ts`, `stackLayout.test.ts`, `PanelStack.tsx`, `PanelStack.test.tsx`, `PanelStack.css`, `PanelStack.css.test.ts`

**Interfaces — Consumes:** `PANELS`, `PanelId`, `PANEL_LABEL`, `DEFAULT_WEIGHT` (Task 2); `useLayoutMotion` (`src/features/window/motion.ts`); `LayoutContext` (`src/features/window/paneColumn.ts`).

**Interfaces — Produces:**
```ts
// stackLayout.ts
export const STACK = { min: 160, divider: 1, step: 16 } as const;
type Weights = Partial<Record<PanelId, number>>;
/** Whole pixels for each shown panel, summing to `total` less the dividers. Proportional to the weights; a panel under 160 takes 160 and the rest share what is left; when `total` can't hold 160 each, equal. */
export function stackHeights(total: number, shown: readonly PanelId[], weights: Weights): Partial<Record<PanelId, number>>;
/** The pair's heights after the divider between them moved `by` px down: each at least 160, their sum kept. The same heights when the pair can't hold 2 × 160. */
export function movedPair(upperPx: number, lowerPx: number, by: number): { upper: number; lower: number };
/** The weights that give the pair these heights: the pair's weight sum is kept and split in proportion; other panels' weights are untouched. */
export function pairWeights(weights: Weights, upper: PanelId, lower: PanelId, upperPx: number, lowerPx: number): Weights;
/** The discrete state a motion keys on: the panels on screen, joined with "+". */
export function stackKey(on: readonly PanelId[]): string;

// PanelStack.tsx
export type PanelStackProps = {
  /** The panels on screen, in the stack's order. */
  panels: readonly PanelId[];
  weights: Weights;
  /** One panel's frame. Called for a panel that is leaving too, until its motion ends. */
  render(panel: PanelId): ReactNode;
  onWeights(weights: Weights): void;
  /** The divider went away while it had the keyboard. */
  onDividerGone?(): void;
};
export function PanelStack(props: PanelStackProps): JSX.Element;
```
Markup: `<div class="panel-stack" data-drop-pane="">`, one `<div class="panel-stack__row" data-panel="browser">` per drawn panel (plus `data-moving=""` while its height moves), and between two settled rows `<div class="panel-stack__divider" role="separator" aria-orientation="horizontal" tabindex="0" aria-label="Resize Browser and Files" aria-valuemin="160" aria-valuemax={upper + lower - 160} aria-valuenow={upper}>`.

- [ ] **Step 1: Write the failing tests.**
  - `stackLayout.test.ts`: `one panel takes the whole height` (600 → 600); `two equal weights split the height less the divider` (601 → 300, 300); `weights 3 to 1 split 3 to 1` (801, {browser: 30000, files: 10000} → 600, 200); `a panel under the minimum takes 160 and the other the rest` (601, 1 to 99 → 160, 440); `a pane too short for the minimum splits evenly` (301 → 150, 150; 300 → 150, 149); `heights are whole and sum to the total less the dividers` (a loop over totals 200 to 900 and three weight pairs); `zero, negative and NaN totals give zero heights`; `movedPair keeps the sum and the minimum` (300/300 by +200 → 440/160; by −1000 → 160/440); `a drag in a pane too short changes nothing` (150/150 by 40 → 150/150); `pairWeights keeps the pair's sum` ({} with 450/150 → {browser: 15000, files: 5000}; equal px → equal weights); `stackKey` (`["browser","files"]` → `"browser+files"`, `[]` → `""`).
  - `PanelStack.test.tsx` (static markup, so no measured height): `draws a row per panel on screen, in order, each holding its frame`; `is the pane's drop target`; `before it is measured the rows share the height by weight` (the root's inline `grid-template-rows` is `minmax(0, 30000fr) 1px minmax(0, 10000fr)` for those weights) `and no divider is drawn`; `one panel draws no divider`. Export the pure `dividerProps(upper: PanelId, lower: PanelId, upperPx: number, lowerPx: number)` and test `the divider names both panels and reports the upper one's height in pixels`. Export the pure `dividerKey(key: string, upperPx: number, lowerPx: number): { upper: number; lower: number } | null` and test `Up and Down move 16 px, Home and End go to the minimum of each side, any other key is null`. Export `movingLayout(outer: Layout | null, moving: boolean): Layout | null` and test `publishes moving to the page's slot while a row moves` (outer `{ inputs, moving: false }` with `true` → `moving: true`, same `inputs`; `false` → the same object; `null` stays null).
  - `PanelStack.css.test.ts`, on the pattern of `SidePane.css.test.ts`: no hex colour; the forced-colors block names only system colours; `:hover` only inside `(hover: hover) and (pointer: fine)`; the divider has `cursor: row-resize` and `touch-action: none`; a moving row has `overflow: clip`.
- [ ] **Step 2: Run** `pnpm test src/features/pane/stackLayout.test.ts src/features/pane/PanelStack` → FAIL.
- [ ] **Step 3: Implement `stackLayout.ts`.**
- [ ] **Step 4: Implement `PanelStack`.** The root measures its own height with a `ResizeObserver` set up in a layout effect. One inner `Row` per entry of `PANELS` is always mounted so its hook is stable: `useLayoutMotion(stackKey(panels), heights[panel] ?? 0)`; a row is drawn while its target or its drawn value is above 0. Once measured the root's `grid-template-rows` is the drawn pixel values; a moving row gets `data-moving` and its content keeps `block-size: 100%`, so it is squeezed, never pinned. The divider is drawn only between two rows when none moves; it drags with pointer capture (`movedPair` from the heights at pointer down, `onWeights(pairWeights(...))` on every move, never animated because the key doesn't change), takes `dividerKey`, and a double-click calls `onWeights(pairWeights(weights, upper, lower, 1, 1))`. When the divider unmounts while it is `document.activeElement`, call `onDividerGone`. Wrap the rows in `<LayoutContext.Provider value={movingLayout(outer, anyRowMoving)}>`. Read the `good-css` skill, then write `PanelStack.css` on the model of `.pane__divider` (`MainWindow.css:308-355`): the hairline token, `Highlight` when active in forced colours.
- [ ] **Step 5: Run** `pnpm test src/features/pane src/design` and `pnpm build` → pass (`src/design/motion.test.ts` and `noBoxedOutlines.test.ts` included).
- [ ] **Step 6: Commit** — `feat(pane): a stack lays the panels out by weight, with a divider and the layout's motion.`

### Task 4: `SidePane` becomes one panel's frame

**Files:**
- Modify: `src/features/pane/SidePane.tsx`, `SidePane.test.tsx`, `SidePane.css`, `SidePane.css.test.ts`, `focusPane.ts`, `focusPane.test.ts`
- Modify, only to keep compiling: `src/features/sessions/LiveApp.tsx`

**Interfaces — Produces:**
```ts
export type SidePaneProps = {
  panel: PanelId;
  tabs: StripTab[]; activeId: string | null; children: ReactNode;
  /** + : its name ("New tab" for Browser, "Open a file" for Files), whether it opens a menu, and why it can't be used. */
  plus: { label: string; menu: boolean; disabled?: string };
  /** The ⋯ button, for a panel that has a menu: its name ("Browser menu"). */
  menu?: { label: string };
  /** ⤢ : whether this panel fills the pane, and why it can't. */
  fill: { on: boolean; disabled?: string };
  /** The panel's own key, for ×'s tooltip: "Ctrl+Shift+A". */
  hideKeys: string;
  onSelect(id: string): void; onClose(id: string): void; onMove(id: string, to: number): void;
  onPlus(anchor: { x: number; y: number }, opener: HTMLElement): void;
  onMenu?(anchor: { x: number; y: number }, opener: HTMLElement): void;
  onFill(): void; onHide(): void;
};
// focusPane.ts
export function panelOfElement(el: Element | null): PanelId | null;      // the panel `el` is inside
export function paneFocusTarget(root: ParentNode, panel: PanelId, closing?: string, where?: "pane" | "strip"): HTMLElement | null;
export function focusPane(panel: PanelId, closing?: string, where?: "pane" | "strip"): void;
```
Markup: `<section class="side-pane" aria-label="Browser" data-nav-side="browser">` (no `data-drop-pane`: the stack has it). The tablist is `aria-label="Browser tabs"`. After the spacer, in this order: + (`data-pane-plus=""`, `aria-label` and `title` = `plus.label`, `aria-haspopup="menu"` only when `plus.menu`; when `plus.disabled`: `aria-disabled="true"`, title `${plus.label}: ${plus.disabled}`, no click), ⋯ when `menu` (`aria-label` and `title` = `menu.label`, `aria-haspopup="menu"`, three dots), ⤢ (`aria-label="Fill the side pane"`, `aria-pressed`, title "Fill the side pane", or "Show the other panels" while `fill.on`; when `fill.disabled`: `aria-disabled="true"`, title = the reason, no click), × (`aria-label="Hide Browser"`, title `Hide Browser (${hideKeys})`). `os` leaves the props.

- [ ] **Step 1: Write the failing tests.** `SidePane.test.tsx`: `is its panel's section and a navigation stop` (label "Files", `data-nav-side="files"`, no `data-drop-pane`); `names its tablist after the panel`; `ends with +, the menu when it has one, Fill and Hide, in that order`; `the Files panel has no menu button`; `+ opens a menu only when told to`; `a + that can't be used is aria-disabled and says why`; `Fill is pressed while its panel fills, and its tooltip says how to go back`; `Fill with one panel shown is aria-disabled and says why`; `Hide names the panel and its key`; `with no tabs shows no tablist, and still its buttons`. The tests of the tab marks keep their titles. `focusPane.test.ts`: every selector starts with `[data-nav-side="files"]` for Files; `the + button is found by data-pane-plus`; `the empty panel's first button is .empty-panel__button`; `panelOfElement` (inside Browser → "browser", outside → null); `a closing tab is looked for in its own panel`. `SidePane.css.test.ts` keeps its titles.
- [ ] **Step 2: Run** `pnpm test src/features/pane/SidePane src/features/pane/focusPane.test.ts` → FAIL.
- [ ] **Step 3: Implement.** The strip's `focusAfter` fallback focuses the + button as before. The ⋯ and ⤢ icons are 12 px paths in the file's `icon()` style (⤢: two diagonal arrows; ⋯: three dots as `M3 6h.01M6 6h.01M9 6h.01` with `strokeWidth` 1.75).
- [ ] **Step 4: Keep the app compiling.** `LiveApp.tsx` passes `panel="files"`, `plus={{ label: "New tab", menu: true }}`, `fill={{ on: false, disabled: "No other panel is shown" }}`, `hideKeys`, `onFill={() => {}}`, and calls `focusPane("files", …)`; mark each `// 6r-1a Task 8 replaces this.`
- [ ] **Step 5: Run** `pnpm test src/features/pane src/features/sessions src/design` and `pnpm build` → pass.
- [ ] **Step 6: Commit** — `feat(pane): the pane's frame is one panel's, with its own +, menu, Fill and Hide.`

### Task 5: The empty states (main thread: the look is a judgement)

**Files:**
- Create: `src/features/pane/EmptyPanel.tsx` (the shared block), `EmptyBrowser.tsx`, `EmptyFiles.tsx`, `EmptyPanel.test.tsx`, `EmptyPanel.css`, `EmptyPanel.css.test.ts`
- Delete: `src/features/pane/EmptyPane.tsx`, `EmptyPane.test.tsx`; the `.empty-pane*` rules in `SidePane.css` (about `:208-275` and in its forced-colors block)
- Modify, only to keep compiling: `LiveApp.tsx` (use `EmptyFiles`)

**Interfaces — Produces:**
```ts
export function EmptyBrowser(p: { available: boolean; ports: readonly { port: number; url: string }[]; onNewTab(): void; onOpenHtml(): void; onPort(url: string): void }): JSX.Element;
export function EmptyFiles(p: { recent: readonly string[]; onOpenFile(): void; onBrowse(): void; onRecent(path: string): void }): JSX.Element;
```
**Copy and structure (exact).** Both: a centred column, `<div class="empty-panel">`, holding a decorative 40 px icon (`aria-hidden`), `<h2 class="empty-panel__title">`, `<p class="empty-panel__line">`, a row of two buttons (`empty-panel__button` + `empty-panel__button--primary` for the first; each an icon and its words), then an optional list under a small heading (`<h3 class="empty-panel__heading">`).
- Browser: globe · "No pages open" · line "Open an address, an HTML file or a server this session runs." with ports, "Open an address or an HTML file." without · primary "New tab" (plus icon) · "Open HTML file…" (file icon) · heading "Running in this session" over one button per port, labelled `localhost:<port>`, in the port's order. With `available: false`: the line is "Open an HTML file. Web pages aren't available on this system yet.", "New tab" is `aria-disabled="true"` with title "Not available on this system yet", "Open HTML file…" is the primary one, and no ports are listed.
- Files: folder · "No files open" · "Open a file from this project, or drop one here." · primary "Open file…" (search icon) · "Browse…" (folder-open icon) · heading "Recent in this project" over one button per recent file showing the file's name and its folder (as `EmptyPane` does today); no heading with no recent files.

- [ ] **Step 1: Write the failing tests** in `EmptyPanel.test.tsx`: `the Browser block says there are no pages and offers New tab and Open HTML file…` (the exact strings; the first button has the primary class); `lists the session's ports as buttons, in order` (`[{port: 3000, …}, {port: 5173, …}]` → "localhost:3000" before "localhost:5173", under "Running in this session"); `leaves the ports row out and changes its line when the session listens on nothing`; `where pages aren't available New tab is aria-disabled with its reason, Open HTML file… leads, and no port is listed`; `the Files block says there are no files and offers Open file… and Browse…`; `lists recent files by name and folder under Recent in this project`; `shows no heading with no recent files`; `each block's icon is hidden from a screen reader`. `EmptyPanel.css.test.ts`: no hex; system colours only in forced colours; `:hover` only under a fine pointer; no `border` or `outline` on `.empty-panel__button` outside `:focus-visible` and the forced-colors block.
- [ ] **Step 2: Run** `pnpm test src/features/pane/EmptyPanel` → FAIL.
- [ ] **Step 3: Design and implement.** Invoke the `frontend-design` and `good-css` skills first. The block is centred in its panel and scrolls inside it when the panel is short (the panel can be 160 px tall: the icon is dropped under a container height of 280 px with a container query, the buttons never). The primary button is filled with the accent tokens, the second one tinted (`--hover`/`--selected` family), both at least 28 px tall; the icon takes `--text-3`; the lists are quiet rows with the hover tint. Forced colours: the primary button is `Highlight`/`HighlightText`, the second `ButtonFace`/`ButtonText`, focus rings `Highlight`.
- [ ] **Step 4: Run** `pnpm test src/features/pane src/design` and `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(pane): the empty Browser and Files panels say what to do, with an icon and two clear buttons.`

### Task 6: The title bar's panel buttons

**Files:**
- Create: `src/features/window/PanelIcon.tsx`
- Delete: `src/features/window/SidePaneIcon.tsx` (after `rg SidePaneIcon src` shows no other user)
- Modify: `src/features/window/MainWindow.tsx` (`:166-180`, `:256-266`, `:454`), `MainWindow.css` (`:153-190`, `:468-478`), `MainWindow.test.tsx` (`:500-555`), `pointer.test.tsx` (`:92`), `src/styles/paneToggle.css.test.ts`, `src/features/sessions/useNavigation.ts` (`:54`, `topAction`), `useNavigation.test.ts` (`:25-30`)
- Modify, only to keep compiling: `LiveApp.tsx`

**Interfaces — Produces:**
```ts
// MainWindow props: `paneNews` and `onTogglePane` go.
export type PanelButton = { id: string; label: string; shown: boolean; news: boolean; keys: string };
panelButtons?: readonly PanelButton[];
onTogglePanel?(id: string): void;
// PanelIcon.tsx: 16 px, drawn in the current colour, a tinted fill (fill-opacity 0.35) while `open`
export function PanelIcon({ panel, open }: { panel: string; open: boolean }): JSX.Element;   // "browser": a globe; "files": a folder
// useNavigation.ts
TOP_NAMES = { palette, sidebar, launcher, "panel-browser": "Browser", "panel-files": "Files" };
topAction(id): "palette" | "sidebar" | "launcher" | "panel-browser" | "panel-files" | null;
```
Each button: `<button type="button" class="window__pane-toggle" data-nav-top="panel-browser" aria-label="Browser" aria-expanded={shown && paneWanted} aria-disabled={paneBlocked || undefined} data-news={news ? "" : undefined} title=…>`. The title is "The side pane shows beside a session" when blocked, else `${expanded ? "Hide" : "Show"} ${label} (${keys})`, each followed by " · A new tab is waiting" when `news`. They sit where the one button was, in the order given, before the window controls. Only the last keeps `margin-inline-end`; the two are 2 px apart.

- [ ] **Step 1: Write the failing tests.** In `MainWindow.test.tsx` the describe becomes "MainWindow: the panel buttons": `are the first children of the bar's end side, Browser then Files, before the window controls`; `a panel's button shows its own news` (news on Files only → one `data-news`, and its title ends " · A new tab is waiting"); `no_news_no_dot`; `says Show with aria-expanded false for a panel that isn't shown` (title "Show Browser (Ctrl+Shift+A)"); `says Hide with aria-expanded true for a shown panel`; `a shown panel isn't expanded while a main view replaces the terminal`; `stay in the bar in the rail, in focus mode and on another OS`; `are aria-disabled, not disabled, with no session selected`; `are aria-disabled while a main view replaces the terminal`. `pointer.test.tsx`: `a panel's button calls onTogglePanel with its id, unless no session is shown or a main view replaces it`. `paneToggle.css.test.ts` keeps its four tests and gains `only the last button keeps the end margin`. `useNavigation.test.ts`: `knows the two panel buttons, as well as the palette, sidebar and launcher`; `names a panel's button for the cursor's hint` ("Browser", "Files").
- [ ] **Step 2: Run** `pnpm test src/features/window src/styles/paneToggle.css.test.ts src/features/sessions/useNavigation.test.ts` → FAIL.
- [ ] **Step 3: Implement.** `LiveApp.tsx` passes two buttons built from `PANELS` with `shown: view.pane.open`, `news: paneNews.size > 0`, `keys` from `commandById("preview")` for now, and `onTogglePanel={togglePane}`; its `activate` case maps both panel ids to `togglePane()`; mark each `// 6r-1a Task 8 replaces this.`
- [ ] **Step 4: Run** `pnpm test src/features/window src/styles src/features/sessions src/design` and `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(window): the title bar has a button for each panel in place of the side pane's.`

### Task 7: Ctrl+Shift+Tab stops at each panel (worker `effort: high`: the navigation machine)

**Files:**
- Modify: `src/lib/navigation/types.ts`, `machine.ts`, `machine.test.ts`, `src/features/sessions/navLayout.ts` (+ test), `useNavigation.ts` (+ test), `src/lib/keymap.ts:81`, `src/design/NavDemo.tsx` if it builds a `side` context
- Modify, only to keep compiling: `LiveApp.tsx`

**Interfaces — Produces:**
```ts
// types.ts
export type SideStop = { id: string; name: string };            // name: "Browser"
Target: … | { kind: "side"; panel: string };
NavContext.side?: readonly SideStop[] | null;                   // the panels on screen, in order
NavState.selectedSide: string | null;                           // the panel that has the keyboard
NavEffect: … | { type: "select-side"; panel: string };
// useNavigation.ts Options
side: readonly SideStop[];        // empty: Tab never stops in the pane
sideFocused: string | null;
```
Tab's order: the terminals on screen, then each panel in `side`'s order, then round to the first terminal. Spoken: "Selected Browser", "Cursor on Files". `navLayout.ts` reads the attribute's value: `data-nav-side="files"` → `{ kind: "side", panel: "files" }`. `nav-next-pane`'s label becomes "Go to the next terminal or panel".

- [ ] **Step 1: Write the failing tests** in `machine.test.ts`: `Tab goes terminal, Browser, Files, terminal`; `Tab skips a panel that isn't on screen` (side `[Files]`); `from Browser Tab goes to Files, not to the terminal`; `with no panel on screen Tab stays on the terminal's ring`; `a selected panel that left the screen is forgotten` (`selectedSide: "browser"`, side `[Files]` → Tab goes to the first terminal's next stop as from nothing selected in the pane); `with the cursor out Tab carries it over each panel and selects on release`; `an arrow moves the cursor between the two panels by their boxes` (Browser above Files: Down from Browser → Files); `says Selected Browser and Selected Files`. `navLayout` test: `a panel is a target named by its id`. `useNavigation.test.ts`: `the cursor's hint on a panel is its name`.
- [ ] **Step 2: Run** `pnpm test src/lib/navigation src/features/sessions/navLayout.test.ts src/features/sessions/useNavigation.test.ts` → FAIL.
- [ ] **Step 3: Implement.** `sameTarget` compares `panel`. `sideSelected` becomes "the selected panel, when it is still in `ctx.side`".
- [ ] **Step 4: Keep the app compiling.** `LiveApp.tsx`: `side: sidePaneDrawn ? [{ id: "files", name: "Files" }] : []`, `sideFocused: sideFocused ? "files" : null`, `select-side` → `focusPane("files")`; mark `// 6r-1a Task 8 replaces this.`
- [ ] **Step 5: Run** `pnpm test src/lib src/features/sessions src/design` and `pnpm build` → pass.
- [ ] **Step 6: Commit** — `feat(nav): each panel of the side pane is a stop for Ctrl+Shift+Tab.`

### Task 8: The app uses the panels (worker `effort: high`: it rewires `LiveApp`)

**Files:**
- Modify: `src/features/sessions/LiveApp.tsx` (the lines marked "6r-1a Task 8 replaces this", `:700-735`, `:855-860`, `:905-945`, `:951-996`, `:1027-1090`, `:1196-1210`, `:1484-1568`, `:1590-1602`, `:2025-2165`, `:2279-2300`), `LiveApp.test.tsx`, `src/lib/keymap.ts`, `src/lib/appShortcuts.ts` (+ tests), `src/lib/bindings.test.ts` if it lists ids, `src/features/browser/useBrowserPages.ts` (the caller of `popupActions`)
- Modify (copy): the tests that assert the old lines: `paneCommands.test.ts`, `opens.test.ts`, `urlLinks.test.ts` only where the announcement changed

**Interfaces — Consumes:** everything Tasks 2 to 7 produce. **Produces:** `COMMANDS` gains, after `preview`: `{ id: "browser-panel", label: "Browser panel", group: "Window", binding: { code: "KeyA", mac: { code: "KeyA", alt: true } } }` (comment: ⌘A is Select all on macOS) and `{ id: "files-panel", label: "Files panel", group: "Window", binding: { code: "KeyD" } }`; `SessionShortcut` gains `{ kind: "panel"; panel: "browser" | "files" }` and `commandShortcut` maps the two ids to it.

**What `LiveApp` does** (each line is behaviour the tests or the live check pin):
1. `const panelsOn = mainView === "sessions" && focusedId != null ? onScreen(view.pane, focusedId) : []`. `paneShown={panelsOn.length > 0}`. An effect dispatches `{ type: "pin", session }` once `settled && loaded`, the pane is open and `view.pane.panels` is absent.
2. `paneNode` is `<PanelStack panels={panelsOn} weights={stackOf(view.pane, focusedId).weights} render={renderPanel} onWeights={(weights) => paneDispatch({ type: "weights", weights })} onDividerGone={() => focusPane(currentPanel() ?? "files")} />`. `renderPanel(panel)` is a `SidePane` for `panelTabs(view.pane, focusedId, panel)` with `activeIn`, holding `<KeptTabs key={`${session}:${panel}`}>` (the one `render` for a browser tab and a file tab that exists today) or `EmptyBrowser` / `EmptyFiles`.
3. Browser's `plus` is `{ label: "New tab", menu: false, disabled: browserTabs(os) ? undefined : "Not available on this system yet" }` and its `onPlus` calls `openPageInPane(BLANK, { focus: "address" })`. Files' is `{ label: "Open a file", menu: true }` and opens today's + menu (label "Open a file"). Browser has `menu={{ label: "Browser menu" }}` with `onMenu` left as a no-op until Task 9. `fill` is `{ on: stack.filled === panel, disabled: stack.shown.length < 2 ? "No other panel is shown" : undefined }`; `onFill` dispatches `{ type: "fill" }`. `hideKeys` is the panel command's `shortcutLabel`.
4. `EmptyBrowser`: `available={browserTabs(os)}`, `ports={focusedRow?.ports ?? []}`, `onNewTab` as the + button, `onPort={(url) => openPageInPane(url)}`, `onOpenHtml` a no-op until Task 9.
5. `togglePanel(panel)` (the key, the title-bar button, the cursor's release on it, ×): blocked as `togglePane` is (announce `PANE_BESIDE_SESSION`); else read `panelToggleOutcome`, dispatch `{ type: "panel" }`, then: shown → `focusPane(panel)`; hidden → `announce(panelHiddenText(panel))`, and `actions.focusTerminal()` when the keyboard was inside that panel (`panelOfElement(document.activeElement) === panel`, or a page has the keys and `panel === "browser"`), else focus stays. `windowCommand` handles `{ kind: "panel" }` with it; `activate` maps `panel-browser` and `panel-files` to it.
6. Ctrl+Shift+Q (`preview`): `{ type: "toggle", session }`; on show the keyboard goes to `commandPanel(...)` of the panels now on screen; on hide to the terminal.
7. `panelFocus` replaces `sideFocusedInDom`: the panel `document.activeElement` is inside, from `focusin`/`focusout`; a page with the keys counts as Browser. A ref `lastPanel` holds the last non-null value. `currentPanel()` is `commandPanel(panelsOn, panelFocus, lastPanel.current, (p) => panelTabs(...).length > 0)`. `useNavigation` gets `side: panelsOn.map((id) => ({ id, name: PANEL_LABEL[id] }))` and `sideFocused: panelFocus`; `select-side` → `focusPane(effect.panel)`.
8. Ctrl+Tab and "Previous tab": the pane hidden and the session has a tab → show it (as today); else `panel = currentPanel()`; no panel or no tabs in it → `announce(panel ? noTabsText(panel) : NO_TABS_OPEN)`; else `{ type: "step", session, panel, by }`, and focus as `paneTabFocus` says, into that panel. "Close tab" closes `activeTab(pane, session, currentPanel())`, asking first for unsaved edits as today.
9. `shownPreview`, `shownPdf` and `selectRegion` look at `currentPanel()`'s selected tab first, then at the other panels on screen, and take the first that has what they need.
10. `openInPane` and `openPageInPane`: the panel is `panelOfPath(target.path)` / `"browser"`; announce `openedText(name, panel)`; `focusPane(panel)`. `openPageInPane` finds the opened tab with `activeTab(next, session, "browser")`. A Preview's `onOpen` calls `openInPane(target)` when `panelOfPath(target.path) !== panel`, else dispatches `navigate`.
11. `marshell open`: `takeOpens` gets `onScreen: panelsOn`; news is added per panel; the seen effect runs per panel on screen (`{ type: "seen", session, panel }` for the news and, when that panel's selected tab is unseen, for the tab). `panelButtons` are built from `PANELS` with `shown: panelsOn.includes(id)`, `news: panelHasNews(paneNews, id)`, `keys`.
12. `shownPage`: the Browser panel is on screen and its selected tab is a browser tab. `useBrowserPages`' popup is on screen when its session is focused and Browser is on screen.
13. `askToClose` compares with `activeIn(tabs, panelOf(tab))`. A close from a strip keeps today's focus rules, inside its own panel. Drops: every dropped file goes through `openInPane`; the label stays "Drop to open in the side pane".
14. `toTerminalFromPane` and Esc are unchanged.

- [ ] **Step 1: Write the failing tests.** `appShortcuts` test: `Ctrl+Shift+A is the Browser panel and Ctrl+Shift+D the Files panel` (and ⌘⌥A, ⌘D on macOS). `bindings.test.ts`'s existing "free of a reserved chord on every system" and "free of duplicates" must hold for the two. `LiveApp.test.tsx` (static markup, on its existing harness): `draws a section for each panel on screen` (a layout with `panels.shown ["browser","files"]` → `data-nav-side="browser"` before `data-nav-side="files"`); `a filled panel is drawn alone`; `an empty Browser panel lists the session's ports`; `the title bar has a button per panel, expanded for the ones on screen`; `with the pane hidden no panel is drawn and no button is expanded`. Export from `LiveApp.tsx`'s neighbours any pure helper it needs and test it there: `a hide from inside the panel gives the terminal the keyboard` and `a hide from elsewhere leaves the keyboard alone` (a pure `hideFocus(panel, focused: PanelId | null): "terminal" | "stay"` in `paneCommands.ts`); `the editor's commands fall to the other panel on screen` (a pure `firstWith<T>(order: readonly PanelId[], get: (p: PanelId) => T | null): T | null`, with `panelOrder(on, current)` = current first).
- [ ] **Step 2: Run** `pnpm test src/features/sessions src/lib` → FAIL.
- [ ] **Step 3: Implement** lines 1 to 14; remove every "6r-1a Task 8 replaces this" marker (`rg "6r-1a Task 8" src` → nothing).
- [ ] **Step 4: Run** `pnpm test` (the whole suite) and `pnpm build` → pass. `rg -n "Hide the side pane|aria-label=\"Side pane\"|data-nav-top=\"pane\"" src` → nothing outside tests' negative assertions.
- [ ] **Step 5: Commit** — `feat(pane): the app shows the Browser and Files panels, with their buttons, keys and focus.`

### Task 9: The Browser menu

**Files:**
- Create: `src/features/browser/browserMenu.ts`, `browserMenu.test.ts`
- Modify: `src/features/browser/UrlBar.tsx` (`:24`, `:51`, `:145-154`), `UrlBar.test.tsx` (`:31`, `:41`, `:58-70`, `:99`), `BrowserTab.tsx` (`:124-137`: stop passing `onOpenOutside` to `UrlBar`; the unavailable branch keeps its button), `src/lib/filePicker.ts`, `src-tauri/src/file_picker.rs` (+ its test if it has one), `src/features/sessions/LiveApp.tsx`

**Interfaces — Produces:**
```ts
export type MenuTarget = { kind: "none" } | { kind: "blank" } | { kind: "file" } | { kind: "page"; url: string };
export function menuTarget(tab: PaneTab | undefined): MenuTarget;        // the Browser panel's selected tab
export function browserMenuItems(t: MenuTarget): MenuItem[];
export type BrowserMenuChoice = "copy" | "outside" | "html" | "clear" | null;
export function browserMenuChoice(id: string): BrowserMenuChoice;
// filePicker.ts
export async function pickFile(start: string | null, kind?: "html"): Promise<string | null>;
```
Rows, in order: "Copy link", "Open in the system browser", a separator, "Open HTML file…", a separator, "Clear browsing data…". The first two carry `disabled: "No page is open"` for `none` and `blank`, and `disabled: "An HTML file has no address"` for `file`. Rust: `pick_file(window, start: Option<String>, kind: Option<String>)`; `kind == Some("html")` adds the filter `("HTML files", ["html", "htm"])`; any other value adds none.

- [ ] **Step 1: Write the failing tests.** `browserMenu.test.ts`: `lists the four rows in order, with two separators`; `a page enables Copy link and Open in the system browser`; `a blank tab and no tab say No page is open`; `an HTML file says it has no address`; `Open HTML file… and Clear browsing data… are never disabled`; `a row's id is its choice, and an unknown id is null`. `UrlBar.test.tsx`: `has no Open in the system browser button` and the order tests without it. Rust (in `file_picker.rs`, on a pure `fn filter_for(kind: Option<&str>) -> Option<(&'static str, &'static [&'static str])>`): `html_gets_the_html_filter`, `any_other_kind_gets_none`.
- [ ] **Step 2: Run** `pnpm test src/features/browser` and `cargo test -p marshell-app -j 4 file_picker` → FAIL.
- [ ] **Step 3: Implement.** In `LiveApp`: a `browserMenu` state like `plusMenu`; `onMenu` opens a `ContextMenu` labelled "Browser menu" beside the + menu's; a pick gives focus back to the ⋯ button first, then: `copy` → `navigator.clipboard.writeText(url)` then "Copied the link." (on failure "Couldn't copy the link."); `outside` → `openUrl(url)` as `onOpenOutside` does; `html` → `pickFile(cwd, "html")` then `openInPane({ path })` (the failure line is Browse…'s); `clear` → `setClearData("ask")`. `EmptyBrowser`'s `onOpenHtml` is the same `html` action.
- [ ] **Step 4: Run** `pnpm test src/features/browser src/features/sessions src/features/coverScan.test.ts`, `pnpm build`, `cargo test -p marshell-app -j 4`, `cargo clippy -p marshell-app --all-targets -- -D warnings` → pass.
- [ ] **Step 5: Commit** — `cargo fmt --all`, then `feat(browser): the Browser panel has a menu, and Open in the system browser moves into it.`

### Task 10: The plan's binding text follows, then the live check (main thread)

**Files:** `docs/PLAN.md` (the side pane's sections outside §9 that name the "Side pane button", the + menu's "New browser tab" row, the "Hide the side pane" button, the keyboard table's Ctrl+Shift+Tab row, and "There is no file tree; that stays in v2" at about `:888`), `docs/research/2026-11-21-side-pane-panels-live-check.md`.

- [ ] **Step 1:** `rg -n "Side pane button|Hide the side pane|New browser tab|no file tree" docs/PLAN.md`; bring each to what was built, with a pointer to "Slice 6r-1a decisions". Add the two commands to the keyboard table. Commit — `docs(plan): the side pane's text follows the Browser and Files panels.`
- [ ] **Step 2: Live check** with the `marshell-live-check` skill (scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, `USERPROFILE`; the app is never brought to the front; only what the check started is stopped, by PID). Prefer `browser_evaluate` returning small JSON; screenshots only for the look. Prove each exit criterion and write what was seen, criterion by criterion, in the research note:
  1. A scratch `layout.json` saved by `main` with the pane open, a web tab selected and two file tabs: the first start shows Browser over Files, the web tab selected in Browser, a file in Files; `layout.json` then holds `panels`.
  2. Ctrl+Shift+A and Ctrl+Shift+D show and hide their panels; the last one hides the pane; Ctrl+Shift+Q brings back what was shown; a key with the pane hidden shows that panel alone. `aria-expanded` and the tooltips follow.
  3. Ctrl+click on a path in the terminal with only Browser shown adds Files under it with the keyboard in the file; Ctrl+click on a `localhost` address with only Files shown adds Browser.
  4. The divider: a drag, Up/Down, Home/End, a double-click; the split survives a window resize and a restart; a 300 px tall pane splits evenly.
  5. ⤢ fills and restores; opening into the other panel ends the fill; hiding the filled panel brings the others back; the fill survives a restart.
  6. A real page in Browser: its box matches its slot (`getBoundingClientRect` against the shell's report) after a show, a hide of Files, a fill, a divider drag and a window resize; nothing of it shows over Files (screenshot); it is gone when Browser hides.
  7. Ctrl+Shift+Tab goes terminal, Browser, Files, terminal; Ctrl+Tab steps the tabs of the panel that had the keyboard last while typing in the terminal.
  8. `marshell open` from a shell session for a `.md` and for an address while only the other panel is shown: no panel appears, focus stays, the dot is on the right button and goes when that panel shows.
  9. The empty states (screenshots in the dark and the light theme, and in forced colours through `browser_emulate_media`): the ports row with a listening dev server; "New tab", "Open HTML file…", "Open file…", "Browse…" each work by keyboard.
  10. The ⋯ menu: Copy link puts the address on the clipboard; the disabled rows say why on a blank tab and on an HTML file; Open HTML file… opens the dialog (cancelled by the check); Clear browsing data… opens its question. An `.html` file shows in Browser with Rendered and Source.
  11. Reduced motion: a panel shows at once.
- [ ] **Step 3:** Fix what the check finds (each fix with its test), rerun the criterion, commit the note — `docs(research): record slice 6r-1a's live check.`

## After the tasks

- Final whole-branch review by `marshell-reviewer` (Opus) with this plan's Review Focus, one fix pass, then `marshell-ship` (the CI commands, the PR with "What it does", "Live check", "Known limits", "Checks"), a green CI, `pre-merge-review`, the merge. Delete the ledger `.superpowers/sdd/2026-11-21-phase-6r-1a-side-pane-panels/progress.md` after the merge.
- Known limits to state in the PR: macOS and Linux are unverified (the Browser panel holds HTML files only there); an HTML file runs no scripts (6r-1b's spike); the palette's "Clear the browser's data…" and the menu's "Clear browsing data…" are two names for one thing; the ⌘ keys for macOS are chosen from the reserved list, not tried.
- Risks: (a) the page under a moving stack — proven by live criterion 6, and the fallback is to hide the page for the motion's 200 ms; (b) `LiveApp.tsx` is large and Task 8 touches many places — the task lists them, and the whole suite runs at its end; (c) the interim tasks leave the pane wrong on screen — nothing is checked live before Task 8.
