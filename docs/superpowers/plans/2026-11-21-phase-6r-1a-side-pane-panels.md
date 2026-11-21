# Slice 6r-1a: the side pane becomes the Browser and Files panels

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The side pane's one tab strip becomes two panels, Browser (web pages and HTML files) and Files (every other file), each with its own title-bar button, key, tab strip, selected tab and empty state; they show alone or stacked with a movable divider, one can fill the pane, and the Browser panel gets a ⋯ menu with Copy link, Open in the system browser, Open HTML file… and Clear browsing data….

**Architecture:** A session keeps its one ordered list of tabs. Which panel a tab is in is derived from what it shows (`panelOf`), so nothing about a tab is saved twice. The window's stack (`view.pane.panels`: the panels shown, their height weights, the filled panel) is new, optional state in `layout.json`, and each session gains the Browser panel's selected tab (`active_browser`) beside `active`. The pure `paneReducer` owns every rule; `PanelStack` lays the panels out from pure maths in `stackLayout.ts` with the app's own 200 ms motion; `SidePane` becomes one panel's frame and is drawn once per panel; the navigation machine gets one Ctrl+Shift+Tab stop per panel; a page's placement rule learns how its panel's height follows the window's.

**Tech Stack:** Rust (`marshell-protocol` serde + ts-rs types, `marshell-core` layout repair and validation, the page placer and one command in `src-tauri`), React + TypeScript, Vitest through `renderToStaticMarkup`, plain CSS with the project's tokens.

**Spec:** `docs/PLAN.md` §9, "Slice 6r decisions (2026-11-20)" (1, 3, 6 to 11) and "Slice 6r-1a decisions (2026-11-21)" (1 to 10). It builds on 6q-2 decisions 4, 6 and 13, 6q-3 decisions 4, 12 and 14, and resize-polish decision 3.

**Checked before planning (2026-11-21, on `origin/main` eddce674, by four readers, by hand, and by an Opus second reader whose 27 findings are folded in):**
- `PaneView` (`crates/protocol/src/api.rs:525`) is `{ open, width, sessions, recent }`; `SessionTabs` (`:509`) is `{ tabs, active }`; both derive `Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS`. `PaneEntry.page` shows the optional-field pattern: `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]`, which leaves every existing TypeScript fixture valid.
- `crates/core/src/layout.rs`: `lenient_pane` and `lenient_session` (`:42-96`) drop the parts of a pane that don't parse, so a bad value never costs the rest of the file; `repair_pane` (`:212`) drops what can't be drawn and resets a session's `active` that names no tab; `validate_pane` (`:329`) is the same rules as errors, and **refuses `active: null` for a session that has tabs** (`:428-435`). A refused save is only a `console.warn("Could not save the layout")` (`src/features/window/useViewState.ts:73-77`), and nothing is saved again in that run.
- `src/features/pane/paneState.ts` (401 lines) is the whole reducer. `open` and `openPage` set `open: true`; `openBackground` never does and selects only in a session with no tabs; `select`, `step`, `close`, `move` and `seen` work on the session's one list and one `active`.
- The readers of "the selected tab": `LiveApp.tsx` `shownPreview` (`:710`), `shownPdf` (`:728`), `askToClose` (`:859`), the seen effect (`:938`), `openPageInPane` (`:990`), `shownPage` (`:1565`), `selectRegion` (`:1594`), `paneActive` (`:2029`); `useBrowserPages.ts` `popupActions` (`:38`, called at `:119` with "the session is the focused one"); `paneCommands.ts` `closeTabOutcome`.
- `SidePane.tsx` is the frame: `<section className="side-pane" aria-label="Side pane" data-nav-side="" data-drop-pane="">`, a strip (tablist `aria-label="Tabs"`, tabs `id="pane-tab-<id>"`, a + button `aria-label="New tab" aria-haspopup="menu"`, a "Hide the side pane" button), then the body. `focusPane.ts`'s selectors are rooted at `[data-nav-side]` and find the + button by its `aria-label`. `src/features/drop/Drop.css:4,10,37` draws the drop frame on `.side-pane[data-drop-over]`.
- `MainWindow.tsx:454` draws the one title-bar button (`window__pane-toggle`, `data-nav-top="pane"`, `aria-expanded`, `aria-disabled`, `data-news`, `SidePaneIcon`). `paneWanted = paneShown && !viewing && !empty`. On a hide, `usePaneHold` keeps the pane shown until its page has frozen, and the window keeps drawing the `pane` node it is given until the fold ends (`:343-375`); the divider's lost focus is tracked with a `dividerFocused` ref (`:383-392`). `LayoutContext` (`paneColumn.ts`) publishes `{ inputs, moving }`.
- A page's place: `usePageSlot.ts` measures the slot against `.window` and reports `{ rect, win, rule, moving }`; a `ResizeObserver` on the slot re-reports, and while `LayoutContext.moving` is true it reports every frame. Its effect depends on the context's value and its cleanup cancels the report that is waiting for a paint (`:216-248`), so a context value that is a new object every frame would cancel every report. In Rust, `Placer::on_report` takes a `moving` report as measured, but **inside a window resize `Placer::on_resize` places by the rule alone, with `height = win.height − top − bottom`** (`src-tauri/src/browser/rules.rs:489-499`): a slot whose distance from the window's foot grows with the window would be overdrawn until the next report.
- `useLayoutMotion(key, target)` (`src/features/window/motion.ts:73`) animates a number for 200 ms when `key` changes, follows `target` at once otherwise, never moves on its first render, and honours reduced motion.
- Navigation: `Target` has `{ kind: "side" }`, `NavContext.side?: string | null`, `NavState.selectedSide: boolean`, effect `{ type: "select-side" }` (`src/lib/navigation/types.ts`, `machine.ts:35-62`, `:117-130`); `navLayout.ts:6,13` reads `[data-nav-side]`; `useNavigation.ts:54` `TOP_NAMES` has `pane: "Side pane"`.
- Keys: a default binding is Ctrl+Shift+key on Windows and Linux and **⌘+key on macOS** (`keymap.ts:25-33`). `bindings.ts` `RESERVED` holds ⌘A and ⌘H on macOS; Ctrl+Shift+A, D and H are free on Windows and Linux; ⌘D is free on macOS; no command uses KeyA, KeyD or KeyH. `bindings.test.ts:158,189` fail on a reserved or duplicate default. The chords a page hands back to the app are computed from `sessionShortcut`, so new keys need nothing there.
- `MenuItem` (`src/components/ContextMenu/menuKeys.ts:3`) is `{ id, label, shortcut?, disabled?: string, submenu? } | { separator: true }`; a disabled row stays reachable and shows its reason; `ContextMenu` calls `useCoverPage` (`src/features/browser/coverScan.test.ts` enforces it for overlays).
- `pickFile(start)` (`src/lib/filePicker.ts:4`) invokes `pick_file(window, start)` (`src-tauri/src/file_picker.rs:8`), which sets no filter; a new parameter needs no capability. The clipboard is written with `navigator.clipboard.writeText`. `openUrl` is allowed for `http://*` and `https://*` only. The palette hides `clear-browser-data` where `!browserTabs(os)` (`LiveApp.tsx:743`).
- A session's ports are `RowModel.ports?: PortChip[]` (`{ port, url }`); `focusedRow` is `LiveApp.tsx:552`. `kindOf(path) === "html"` for `.html` and `.htm`, whatever the case (`preview/kinds.ts:20`).
- `pnpm build` is `tsc --noEmit && vite build` over all of `src`, tests included: a test that uses an old signature fails the build.
- `LiveApp.test.tsx` does not mount `LiveApp`; its "LiveApp: the side pane" describe (`:719-755`) renders `SidePane` and `EmptyPane` directly and runs reducer actions.

## Global Constraints

- **Scope.** No file tree, no Changes panel, no Save screenshot, viewport sizes, cookie setting, Ctrl+click setting or auto-open (6r-1b, 6r-2, 6r-3). No new dependency, no CSP or capability change, no new route.
- **`layout.json` stays version 1.** Every new field is optional on read, and a new field that doesn't parse costs only itself. An old file loads; an older build ignores the new fields.
- **What the core accepts is what the reducer writes.** While a session has tabs its `active` names one of them: the Files panel's selected tab, or the Browser panel's when Files has none. `active_browser` is present only while the Browser panel has a tab, and names its selected one. `panels.shown` is never empty; `panels.filled` is in `shown`, and only while `shown` has two.
- **Panel ids and order:** `"browser"` then `"files"`, top to bottom. Labels: "Browser", "Files". A tab's panel: a browser tab, or a file tab whose current path `kindOf` calls `"html"`, is in Browser; every other tab is in Files. The same on every OS.
- **Limits.** 32 tabs per session over both panels; 4 kept tabs per panel; a panel is at least 160 px tall; the stack's divider is 1 px; a key step is 16 px; a weight is a whole number from 1 to 100000, and 10000 when absent.
- **Keys.** `browser-panel`: `{ code: "KeyA", mac: { code: "KeyA", alt: true } }`. `files-panel`: `{ code: "KeyD" }`. Ctrl+Shift+Q (`preview`) keeps toggling the whole pane.
- **Copy (exact).** "Opened README.md in Files." · "Opened localhost:3000 in Browser." · "Claude Code opened report.html in Browser." · "No tabs open in Browser." · "Browser is hidden." · "The side pane shows beside a session" · "Not available on this system yet" · "No other panel is shown" · "Resize Browser and Files" · "Copied the link." · "Couldn't copy the link." · "No page is open" · "An HTML file has no address". The empty states' copy is in Task 7; the menu's rows in Task 12. Sentence case, no "!".
- **Frontend rules** (`.claude/rules/ui.md`): tokens only, no hex; no boxed outlines; `:hover` only under `(hover: hover) and (pointer: fine)`; a `forced-colors` block names system colours only; every control works with the keyboard alone; `Announcer` is the only live region; components are tested through `renderToStaticMarkup` (node, no DOM). Read the `good-css` skill before writing declarations.
- **Protocol rules** (`.claude/rules/protocol.md`): `#[derive(TS)] #[ts(export)]`; never hand-edit `src/generated/`; `cargo test -p marshell-protocol -j 4` regenerates, then `pnpm check:generated`.
- **Checks.** Rust: `cargo test -p <crate> -j 4`, `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo fmt --all` before each Rust commit. Frontend: `pnpm test <path>`, and `pnpm build` at the end of every frontend task; a task that changes a signature fixes every test the build then names, whether or not this plan lists it. Commit messages read `type(scope): what changed, as a sentence.`
- **Workers** never run `git checkout`, `git restore` or `git stash` on a file they don't own, never touch `hand-keys-1.json`, and never start `pnpm tauri dev`.
- **Between Task 2 and Task 11** the app compiles and every test passes, but the pane on screen is not right (one strip, no drop target). Nothing is live-checked before Task 11 is in. A line added only to keep the app compiling carries the comment `// 6r-1a: replaced in Task 10 or 11.`

## Review Focus

1. **A layout save is never refused.** Every state the reducer can reach passes the core's rules, the first page opened in a session with no tabs and the close of a panel's last tab included. (Task 2: `every state the reducer reaches is one the core accepts`; Task 13 reads the console after every criterion.)
2. **An old `layout.json` with the pane open and mixed tabs.** The first start shows the panels that hold tabs for the session on screen, every tab is in one of them, and the tab that was selected stays selected in its panel after the first click elsewhere. (Tasks 1, 2, 10: `an_old_pane_reads_with_no_panels`, `a layout with no panels shows the panels that hold tabs`, `the old active tab stays its panel's selection after a select in the other panel`, `nothing is pinned with no session on screen`.)
3. **A session switch never changes the panels**, and hiding the pane never changes what is drawn in it while it folds. (Task 2: `the stack is the window's`, `the stack's panels are the same with the pane hidden`; Task 13 live.)
4. **A pane too short for two 160 px panels** splits evenly, nothing overflows, and the divider's keys change nothing. (Task 4.)
5. **A page stays inside its panel**: while the stack moves, fills or a panel hides, while the window's height is dragged, and it is gone with its panel. (Task 4: `gives the page's slot one layout object per state`; Task 5: `a_rule_with_a_slope_keeps_the_page_in_its_panel`; Task 13 samples it every frame.)
6. **`marshell open` into a panel that isn't on screen** shows nothing, moves no focus, puts the dot on that panel's button only, and counts toward the 32. (Task 2, Task 3; Task 13 live.)
7. **The keyboard is never left on something that went away or is out of sight:** a panel hidden while its ⤢, × or the divider has focus; a panel hidden in a narrow window where the pane covers the terminal. (Task 3: `hideFocus`; Task 13 live.)

---

### Task 1: `layout.json` holds the stack and the Browser panel's selected tab

**Files:**
- Modify: `crates/protocol/src/api.rs` (near `:509-535`), `crates/core/src/layout.rs` (`lenient_pane`, `lenient_session`, `repair_pane`, `validate_pane`, their tests), any Rust struct literal of `PaneView` or `SessionTabs` the compiler names
- Regenerated: `src/generated/Panels.ts`, `PaneView.ts`, `SessionTabs.ts`
- Test: `crates/core/src/layout.rs` (unit tests), `crates/core/tests/layout_api.rs`

**Interfaces — Produces:**
```rust
/// The side pane's stack of panels.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Panels {
    /// The panels shown while the pane is open: "browser" and "files", in that order, at least one.
    pub shown: Vec<String>,
    /// Each panel's weight in the stack's height: 1 to 100000; a panel with none has 10000.
    #[serde(default)] pub weights: BTreeMap<String, u32>,
    /// The shown panel that fills the pane, hiding the other.
    #[serde(default)] pub filled: Option<String>,
}
// PaneView gains:
#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub panels: Option<Panels>,
// SessionTabs gains (the Browser panel's selected tab):
#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub active_browser: Option<String>,
```
TypeScript then has `PaneView.panels?: Panels` and `SessionTabs.active_browser?: string`. The rule for `active` (`:428-435`) is unchanged.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s test module:
  - `an_old_pane_reads_with_no_panels`: a view JSON whose `pane` has only the four old fields loads with `panels == None` and each session's `active_browser == None`.
  - `a_stack_of_the_wrong_type_loses_only_itself` (on the pattern of `a_pane_with_values_of_the_wrong_type_loses_only_those_values`): a file whose `panels` is `"files"`, or `{ "shown": "files" }`, or has `weights: { "files": 1.5 }`, or `filled: 3`, and a session whose `active_browser` is `7`, loads with every tab, session and other field intact, `panels == None` and that `active_browser == None`.
  - `repair_keeps_a_good_stack`: `shown ["browser","files"]`, `weights {browser: 6000, files: 14000}`, `filled Some("files")` is unchanged.
  - `repair_orders_and_dedupes_shown`: `["files","browser","files"]` → `["browser","files"]`.
  - `repair_drops_an_unknown_panel`: `["changes","files"]` → `["files"]`; `["changes"]` → `panels == None`.
  - `repair_drops_a_bad_weight`: a key `"tree"`, a value `0` and a value `100001` are removed; a good one stays.
  - `repair_clears_a_fill_that_cannot_be`: `filled Some("browser")` with `shown ["files"]` → `None`; `filled Some("files")` with `shown ["files"]` → `None`.
  - `repair_clears_an_active_browser_that_names_no_tab`: → `None`; one that names a tab stays.
  - `validate_names_the_stack_field`: unordered or duplicate `shown`, empty `shown`, an unknown id, a bad weight key or value, an impossible fill and an `active_browser` naming no tab are each `Err`, the text starting with `pane.panels.shown`, `pane.panels.weights.<key>`, `pane.panels.filled` or `pane.sessions.<id>.active_browser`.
  - In `layout_api.rs`: `put_view_keeps_the_stack` (a PUT with a good `panels` and `active_browser` reads back the same from GET) and `put_view_refuses_a_bad_stack` (400 naming `pane.panels.shown`).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 layout` → compile errors for the missing fields.
- [ ] **Step 3: Implement.** Add the types. In `layout.rs`: `const PANEL_IDS: [&str; 2] = ["browser", "files"]`, `const PANEL_WEIGHT: RangeInclusive<u32> = 1..=100_000`; `lenient_pane` removes a `panels` that doesn't parse as `Panels`; `lenient_session` removes an `active_browser` that isn't a string; `repair_pane` gets a `repair_panels(&mut pane.panels)` and the `active_browser` line; `validate_pane` the matching errors.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 layout`, `cargo test -p marshell-core -j 4 --test layout_api --test layout_restore`, `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, `cargo clippy -p marshell-core -p marshell-protocol --all-targets -- -D warnings`, `pnpm build` → all pass.
- [ ] **Step 5: Commit** — `cargo fmt --all`, then `feat(layout): the side pane's stack of panels and the Browser panel's selected tab are saved.`

### Task 2: A tab's panel, the stack and the reducer (worker `effort: high`)

**Files:**
- Create: `src/features/pane/panels.ts`, `panels.test.ts`, `src/features/pane/paneRules.ts` (test helper: the core's rules in TypeScript)
- Modify: `src/features/pane/paneState.ts`, `paneState.test.ts`
- Modify for the changed action shapes (the build names them): `src/features/window/viewState.test.ts:74,78`, `src/features/sessions/LiveApp.test.tsx:736`, `src/features/sessions/LiveApp.tsx`, `src/features/pane/opens.ts`, `paneCommands.ts`, `src/features/browser/useBrowserPages.ts`

**Interfaces — Consumes:** Task 1's generated types.

**Interfaces — Produces:**
```ts
// panels.ts — imports nothing from paneState.ts (paneState imports it)
export const PANELS = ["browser", "files"] as const;
export type PanelId = (typeof PANELS)[number];
export const PANEL_LABEL: Record<PanelId, string> = { browser: "Browser", files: "Files" };
export const PANEL_COMMAND: Record<PanelId, string> = { browser: "browser-panel", files: "files-panel" };
export const DEFAULT_WEIGHT = 10_000;
export type Weights = Partial<Record<PanelId, number>>;
export function isPanelId(s: string): s is PanelId;
export function panelOfPath(path: string): PanelId;            // a page address or an .html/.htm file → "browser"
export function panelOf(tab: PaneTab): PanelId;                // kind "browser" → "browser"; else panelOfPath(its current path)
export type Stack = { shown: PanelId[]; weights: Weights; filled: PanelId | null };
export function panelsWithTabs(st: SessionTabs): PanelId[];    // in the stack's order
/** The saved stack, cleaned; with none saved, the panels that hold tabs for the session, Files alone when it has none. */
export function stackOf(pane: PaneView, session: string | null | undefined): Stack;
/** What the stack draws, whether or not the pane is open: the filled panel alone, else the shown ones. */
export function stackPanels(stack: Stack): PanelId[];
/** What is in front of the person: nothing while the pane is hidden, else `stackPanels`. */
export function onScreen(pane: PaneView, session: string | null | undefined): PanelId[];
export function panelTabs(pane: PaneView, session: string | null | undefined, panel: PanelId): PaneTab[];
/** The panel's selected tab: its own field when that names a tab of the panel, else the other field when it does, else the panel's first tab, else null. Own field: `active_browser` for Browser, `active` for Files. */
export function activeIn(st: SessionTabs, panel: PanelId): string | null;
```
`paneState.ts`: `activeTab(s, session, panel: PanelId)`; `keptTabs` and `drawnTabs` unchanged; `recentFor` unchanged (an HTML file stays in the recent list, and opens in Browser). `PaneAction` changes:
```ts
| { type: "step"; session: string; panel: PanelId; by: 1 | -1 }
| { type: "move"; session: string; paneTab: string; to: number }      // `to`: the index among its panel's tabs
| { type: "seen"; session: string; panel: PanelId }
| { type: "openPage"; session: string; url: string; background?: true }
| { type: "openBackground"; …; onScreen: boolean }                    // onScreen: its panel is in front of the person with this session shown
| { type: "show"; session: string | null } | { type: "hide" } | { type: "toggle"; session: string | null }
| { type: "panel"; panel: PanelId; session: string | null }           // a panel's key, button or ×
| { type: "fill"; panel: PanelId; session: string | null }            // ⤢
| { type: "weights"; weights: Weights }                                // the divider
| { type: "pin"; session: string }                                     // write the stack down when none is saved
```
`paneRules.ts`: `export function paneProblem(pane: PaneView): string | null` — the Global Constraint "What the core accepts" as code (the `active`, `active_browser`, `panels.shown`, `panels.filled` and weight rules of Task 1), returning the field's name.

**The rules the tests pin** (each line is a test in `panels.test.ts` or `paneState.test.ts`, named as written):
- `a web page and an HTML file are in Browser, every other file in Files` — `https://a.test`, `about:blank`, `C:\\p\\index.html`, `/p/A.HTM` → browser; `README.md`, `a.pdf`, `a.png`, `x.htmlx` → files.
- `the selection is per panel` — the four clauses of `activeIn`.
- `the old active tab is the selection of whichever panel holds it` — `{ active: "t2" }` with t2 a browser tab and no `active_browser`: Browser's selection is t2, Files' is its first tab.
- `the old active tab stays its panel's selection after a select in the other panel` — from that state, `select` a Files tab: `active_browser === "t2"` is now written down.
- `a layout with no panels shows the panels that hold tabs` — `stackOf` with `panels` absent: `["browser","files"]`, `["browser"]`, `["files"]` by the session's tabs, and `["files"]` for none or no session; `weights {}`, `filled null`. A saved stack is returned cleaned (known ids, in order), whatever the session holds.
- `the stack is the window's: another session shows the same panels`.
- `the stack's panels are the same with the pane hidden` — `stackPanels(stackOf(...))` is equal before and after `hide`, and after the last panel's own toggle hid the pane; `onScreen` is `[]` then.
- `a filled panel is the stack's only panel`.
- `an open selects the tab in its panel, shows the pane and adds the panel` — pane open with `shown ["browser"]`, a file opens: `shown ["browser","files"]`, Files' selection is the tab, Browser's is untouched.
- `an open into another panel ends a fill; into the filled panel keeps it`.
- `an open while the pane is hidden shows that panel alone` — `shown` becomes `[panel]`, `filled null`, weights kept.
- `an open of the tab already selected in a panel on screen returns the same state`.
- `the first page in a session with no tabs is active and active_browser` — both name it; `paneProblem` is null.
- `the 33rd tab is refused whatever its panel`.
- `a link to a file of the other panel opens a tab there` — `navigate` from a Markdown tab to `page.html` leaves that tab's history alone and behaves as `open`; at 32 tabs it returns the same state.
- `Back onto an entry of the other panel takes the tab there, selected, with its panel shown` (a tab saved by an older build).
- `select sets its panel's selection only`; `step wraps inside the panel`; `move reorders inside the panel and leaves the other panel's tabs where they were`.
- `closing a panel's selected tab selects its neighbour in that panel`; `closing the Files panel's last tab leaves the panel shown and active on the Browser panel's tab`; `closing the Browser panel's last tab removes active_browser`; `closing the session's last tab forgets the session`.
- `a background open never changes the stack` — `openBackground` and `openPage` with `background: true` leave `open` and `panels` identical, and select the tab only when its panel had none; into a session with no tabs, `paneProblem` is null.
- `seen clears the mark of that panel's selected tab only`.
- `Ctrl+Shift+Q shows the remembered panels, and writes the stack down when none was saved` (`toggle` and `show`); `hide keeps the stack`.
- `a panel's toggle`: on screen with another → it leaves `shown`; the only one shown → `open: false` and `shown` still `[panel]`; not on screen while the pane is open → added, a fill ended; the filled one → it leaves `shown`, `filled null`; pane hidden → that panel alone.
- `fill`: sets `filled` when two are shown; pressed again clears it; with one shown, or the pane hidden, returns the same state.
- `weights are whole, 1 to 100000; NaN, an infinite number and an unknown key are dropped`.
- `pin writes the stack once`.
- `every state the reducer reaches is one the core accepts` — a seeded pseudo-random walk of 2000 actions over three sessions (every action type; files, HTML files and addresses; valid and stale tab ids) with `expect(paneProblem(state)).toBeNull()` after each.

- [ ] **Step 1: Write the failing tests** above and `paneRules.ts`; update the existing reducer tests to the new shapes (their titles stay).
- [ ] **Step 2: Run** `pnpm test src/features/pane/panels.test.ts src/features/pane/paneState.test.ts` → FAIL (missing exports).
- [ ] **Step 3: Implement.** Two private helpers carry the invariants. `settle(st: SessionTabs): SessionTabs` rewrites both selection fields from `activeIn` (`active` = Files' ?? Browser's ?? null; `active_browser` = Browser's, the property removed when null) and runs on every write of a session's tabs or selection, before and after the change. `reveal(s, session, panel)` carries "open, add the panel, end another's fill, alone when the pane was hidden"; `open`, `openPage` (not `background`), the cross-panel `navigate` and the cross-panel `back`/`forward` call it.
- [ ] **Step 4: Keep the build green.** Give the changed actions their new fields at every call the build names: `session: focusedId ?? null` for `toggle`/`show`; for `step`, `seen` and `activeTab` the panel of the tab in hand (`panelOf(tab)`), or of the session's first tab, or `"files"`; `popupActions` keeps its shape for now. Mark each line in `LiveApp.tsx` with the Global Constraints' comment.
- [ ] **Step 5: Run** `pnpm test src/features/pane src/features/browser src/features/sessions src/features/window` and `pnpm build` → pass.
- [ ] **Step 6: Commit** — `feat(pane): tabs belong to the Browser or Files panel, and the stack has its rules.`

### Task 3: What is said and decided around the reducer

**Files:**
- Modify: `src/features/pane/paneCommands.ts`, `opens.ts`, `paneNews.ts`, `plusMenu.ts` and their four tests; `src/features/browser/useBrowserPages.ts` (`popupActions`, `:38`, `:119`) and its test; `src/features/terminal/urlLinks.test.ts` and any other test the build or the run names for the changed lines
- Modify, only to keep compiling: `src/features/sessions/LiveApp.tsx`

**Interfaces — Consumes:** Task 2. **Produces** (all in `paneCommands.ts` unless said):
```ts
export const openedText = (name: string, panel: PanelId) => `Opened ${name} in ${PANEL_LABEL[panel]}.`;
export const openedByText = (who: string, name: string, panel: PanelId) => `${who} opened ${name} in ${PANEL_LABEL[panel]}.`;
export const pageOpenedText = (url: string) => openedText(pageTitle(null, url), "browser");
export const noTabsText = (panel: PanelId) => `No tabs open in ${PANEL_LABEL[panel]}.`;
export const panelHiddenText = (panel: PanelId) => `${PANEL_LABEL[panel]} is hidden.`;
export function paneTabOutcome(pane: PaneView, session: string | null | undefined, panel: PanelId | null): "none" | "show" | "step";
export function closeTabOutcome(pane: PaneView, session: string | null | undefined, panel: PanelId | null): { paneTab: string; path: string } | null;
/** What `{ type: "panel" }` is about to do. */
export function panelToggleOutcome(pane: PaneView, session: string | null | undefined, panel: PanelId): { shows: boolean; paneHides: boolean };
/** The panel a tab command acts on: the one with the keyboard; else the last one that had it, when it is on screen and holds tabs; else the first on screen that holds tabs; else the first on screen; null with none on screen. */
export function commandPanel(on: readonly PanelId[], focused: PanelId | null, last: PanelId | null, holdsTabs: (p: PanelId) => boolean): PanelId | null;
/** Where the keyboard goes when `panel` hides: nowhere when it was elsewhere; to the terminal; or, where the pane covers the terminal and another panel stays, to that panel. */
export function hideFocus(panel: PanelId, focused: PanelId | null, covers: boolean, staying: readonly PanelId[]): { to: "stay" } | { to: "terminal" } | { to: "panel"; panel: PanelId };
/** `current` first, then the others in the stack's order. */
export function panelOrder(on: readonly PanelId[], current: PanelId | null): PanelId[];
export function firstWith<T>(order: readonly PanelId[], get: (p: PanelId) => T | null): T | null;
/** The title bar's buttons. `keys(panel)` is the panel command's shortcut label. */
export function panelButtonsFor(on: readonly PanelId[], news: ReadonlySet<string>, keys: (p: PanelId) => string): { id: PanelId; label: string; shown: boolean; news: boolean; keys: string }[];
```
`opens.ts`: `OpensContext.paneOpen` becomes `onScreen: readonly PanelId[]` (the panels in front of the person, for the session `shown`); each request's `openBackground.onScreen` is `ctx.shown === tab_id && ctx.onScreen.includes(its panel)`; `TakenOpens.news: { session: string; panel: PanelId }[]`. `paneNews.ts`: the set holds keys `${panel}:${session}`; `NewsAction` is `{ type: "add"; items: readonly { session: string; panel: PanelId }[] } | { type: "seen"; session: string; panel: PanelId } | { type: "closed"; session: string }`; `panelHasNews(news, panel)` is true when any session has news for that panel. `plusMenu.ts`: `plusMenuItems(recent)` (no browser row), `PlusChoice` loses `browserTab`. `popupActions(pane, session, url, onScreen, own)`: on screen → `[{ type: "openPage", session, url }]` as today; else exactly `[{ type: "openPage", session, url, background: true }]`.

- [ ] **Step 1: Write the failing tests.** `paneCommands.test.ts`: the five strings exactly; `commandPanel` (one test per clause, and null); `paneTabOutcome` — `"none"` when `panel` is null or holds no tabs while the pane is open, `"show"` when the pane is hidden and the session has any tab, `"step"` otherwise; `panelToggleOutcome` (shows; hides one of two; hides the last one and the pane); `a hide from inside the panel gives the terminal the keyboard`; `a hide from elsewhere leaves the keyboard alone`; `a hide in a pane that covers the terminal gives the other panel the keyboard`, `and the terminal when no panel stays`; `panelOrder` and `firstWith` (`the editor's commands fall to the other panel on screen`); `panelButtonsFor` (`a button is shown for a panel on screen`, `news is per panel`, `with nothing on screen no button is shown`). `opens.test.ts`: `news names the panel` (an HTML file → browser, a `.md` → files); `a request for a panel in front of the person makes no news, one for the other panel does`; the spoken lines name the panel. `paneNews.test.ts`: add; seen for one panel leaves the other; closed drops both; `panelHasNews`. `plusMenu.test.ts`: no browser row. `useBrowserPages` test: `a popup from a session that isn't on screen opens in the background`.
- [ ] **Step 2: Run** `pnpm test src/features/pane src/features/browser/useBrowserPages.test.ts` → FAIL.
- [ ] **Step 3: Implement**, then keep `LiveApp.tsx` compiling: `takeOpens` gets `onScreen: viewRef.current.pane.open && mainView === "sessions" ? PANELS : []`; news items map to the new action; `plusMenuItems(paneRecent)` and no `browserTab` branch; the spoken lines pass `panelOfPath(path)`; `paneTabOutcome` and `closeTabOutcome` pass the panel of the session's `active` tab. Mark each line.
- [ ] **Step 4: Run** `pnpm test` (the whole suite: the strings are asserted in several places) and `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(pane): the announcements, the news dot and the tab commands know the panels.`

### Task 4: The stack: heights, the divider and the motion (worker `effort: high`)

**Files:**
- Create: `src/features/pane/stackLayout.ts`, `stackLayout.test.ts`, `PanelStack.tsx`, `PanelStack.test.tsx`, `PanelStack.css`, `PanelStack.css.test.ts`
- Modify: `src/features/window/paneColumn.ts` (the `Layout` type), `src/features/drop/Drop.css:4,10,37` (`.side-pane[data-drop-over]` → `.panel-stack[data-drop-over]`) and its test if it names the selector

**Interfaces — Consumes:** `PANELS`, `PanelId`, `PANEL_LABEL`, `DEFAULT_WEIGHT`, `Weights` (Task 2); `useLayoutMotion`; `LayoutContext`.

**Interfaces — Produces:**
```ts
// paneColumn.ts
export type Layout = { inputs: ColumnInputs; moving: boolean; /** How many px the slot's distance from the window's foot grows per px of window height: the share of the stack below it. Absent: 0. */ bottomPerPx?: number };

// stackLayout.ts
export const STACK = { min: 160, divider: 1, step: 16 } as const;
/** Whole pixels for each shown panel, summing to `total` less the dividers. Proportional to the weights; a panel under 160 takes 160 and the rest share what is left; when `total` can't hold 160 each, equal. Zero for a `total` that is not a positive number. */
export function stackHeights(total: number, shown: readonly PanelId[], weights: Weights): Weights;
/** How much of one more pixel of `total` goes to the panels below `panel`: 0 to 1, from the same rule before rounding. */
export function belowPerPx(total: number, shown: readonly PanelId[], weights: Weights, panel: PanelId): number;
/** The pair's heights after the divider between them moved `by` px down: each at least 160, their sum kept. The same heights when the pair can't hold 2 × 160. */
export function movedPair(upperPx: number, lowerPx: number, by: number): { upper: number; lower: number };
/** The weights that give the pair these heights: the pair's weight sum is kept and split in proportion, each at least 1; other weights untouched. The same weights when a height is not a positive number. */
export function pairWeights(weights: Weights, upper: PanelId, lower: PanelId, upperPx: number, lowerPx: number): Weights;
/** The discrete state a motion keys on: the panels, joined with "+". */
export function stackKey(panels: readonly PanelId[]): string;

// PanelStack.tsx
export type PanelStackProps = {
  /** The stack's panels (`stackPanels`), in order. The same while the pane is hidden or folding. */
  panels: readonly PanelId[];
  weights: Weights;
  /** One panel's frame. Called for a panel that is leaving too, until its motion ends. */
  render(panel: PanelId): ReactNode;
  onWeights(weights: Weights): void;
  /** The divider went away while it had the keyboard. */
  onDividerGone?(): void;
};
export function PanelStack(props: PanelStackProps): JSX.Element;
export function dividerProps(upper: PanelId, lower: PanelId, upperPx: number, lowerPx: number): { "aria-label": string; "aria-valuemin": number; "aria-valuemax": number; "aria-valuenow": number };
export function dividerKey(key: string, upperPx: number, lowerPx: number): { upper: number; lower: number } | null;
export function rowLayout(outer: Layout | null, moving: boolean, bottomPerPx: number): Layout | null;
```
Markup: `<div class="panel-stack" data-drop-pane="">`; one `<div class="panel-stack__row" data-panel="browser">` per drawn panel (plus `data-moving=""` while its height moves); between two drawn rows always one 1 px track holding `<div class="panel-stack__divider">`, which is the separator (`role="separator" aria-orientation="horizontal" tabindex="0"` and `dividerProps`) when the stack is measured and no row moves, and an inert hairline (`aria-hidden="true"`, no `tabindex`) otherwise.

- [ ] **Step 1: Write the failing tests.**
  - `stackLayout.test.ts`: `one panel takes the whole height` (600 → 600); `two equal weights split the height less the divider` (601 → 300, 300); `weights 3 to 1 split 3 to 1` (801, {browser: 30000, files: 10000} → 600, 200); `a panel under the minimum takes 160 and the other the rest` (601, 1 to 99 → 160, 440); `a pane too short for the minimum splits evenly` (301 → 150, 150; 300 → 150, 149); `heights are whole and sum to the total less the dividers` (a loop over totals 200 to 900 and three weight pairs); `zero, negative and NaN totals give zero heights`; `belowPerPx` — `the last panel has nothing below it` (0), `equal weights put half of each pixel below Browser` (0.5), `3 to 1 puts a quarter below` (0.25), `Browser held at its minimum puts every pixel below` (1), `Files held at its minimum puts none` (0), `a pane too short puts half` (0.5), `one panel has none` (0); `movedPair keeps the sum and the minimum` (300/300 by +200 → 440/160; by −1000 → 160/440); `a drag in a pane too short changes nothing` (150/150 by 40 → 150/150); `pairWeights keeps the pair's sum` ({} with 450/150 → {browser: 15000, files: 5000}; equal px → equal weights); `pairWeights with a zero or NaN height returns the weights it was given`; `stackKey` (`["browser","files"]` → `"browser+files"`, `[]` → `""`).
  - `PanelStack.test.tsx` (static markup, never measured): `draws a row per panel, in order, each holding its frame`; `is the pane's drop target`; `before it is measured the rows share the height by weight` (the root's inline `grid-template-rows` is `minmax(0, 30000fr) 1px minmax(0, 10000fr)` for those weights) `and the divider is inert`; `one panel draws no divider track`; `the divider names both panels and reports the upper one's height in pixels` (`dividerProps("browser","files",440,160)` → label "Resize Browser and Files", min 160, max 440, now 440); `Up and Down move 16 px, Home and End go to the minimum of each side, any other key is null` (`dividerKey`); `gives the page's slot one layout object per state` (`rowLayout`: `null` stays null; with `moving false` and `bottomPerPx 0` it returns `outer` itself; otherwise `{ ...outer, moving: outer.moving || moving, bottomPerPx }`. The component must memoise the call per row on `[outer, moving, bottomPerPx]`: a new object on every frame cancels the page's reports).
  - `PanelStack.css.test.ts`, on the pattern of `SidePane.css.test.ts`: no hex colour; the forced-colors block names only system colours; `:hover` only inside `(hover: hover) and (pointer: fine)`; the divider has `cursor: row-resize` and `touch-action: none`; a moving row and the root have `overflow: clip`.
- [ ] **Step 2: Run** `pnpm test src/features/pane/stackLayout.test.ts src/features/pane/PanelStack` → FAIL.
- [ ] **Step 3: Implement `stackLayout.ts`.** `stackHeights` and `belowPerPx` share one unrounded function.
- [ ] **Step 4: Implement `PanelStack`.**
  - The root measures its own height with a `ResizeObserver` set up in a layout effect.
  - It calls `useLayoutMotion(stackKey(panels), heights[panel] ?? 0)` once for each entry of `PANELS`, in the component itself (the list is a constant, so the hooks' order never changes; say so beside the lint exception).
  - A row is drawn when its panel is in `panels` or its drawn value is above 0. Unmeasured, the tracks are `minmax(0, <weight>fr)`; measured, they are the drawn pixel values. A moving row has `data-moving`; its content keeps `block-size: 100%`, so it is squeezed, never pinned.
  - The separator drags with pointer capture: `movedPair` from the heights at pointer down, `onWeights(pairWeights(...))` on every move (never animated: the key doesn't change). Keys go through `dividerKey`. A double-click calls `onWeights(pairWeights(weights, upper, lower, 1, 1))`.
  - `onDividerGone` uses MainWindow's pattern (`MainWindow.tsx:383-392`): a ref set on the separator's focus and blur, read in an effect when the separator stops being drawn.
  - Each row's content is wrapped in `<LayoutContext.Provider value={useMemo(() => rowLayout(outer, anyRowMoving, belowPerPx(total, panels, weights, panel)), [...])}>`.
  - Read the `good-css` skill, then write `PanelStack.css` on the model of `.pane__divider` (`MainWindow.css:308-355`): the hairline token, `Highlight` when active in forced colours.
- [ ] **Step 5: Run** `pnpm test src/features/pane src/features/drop src/features/window src/design` and `pnpm build` → pass (`src/design/motion.test.ts` and `noBoxedOutlines.test.ts` included).
- [ ] **Step 6: Commit** — `feat(pane): a stack lays the panels out by weight, with a divider and the layout's motion.`

### Task 5: A page's rule follows its panel's height (worker `effort: high`: the placer)

**Files:**
- Modify: `src-tauri/src/browser/rules.rs` (`Rule`, `Rule::place`, tests), `src/features/browser/browserApi.ts` (the `Rule` type the app sends), `src/features/browser/usePageSlot.ts` (`slotReport`, `measure`), `usePageSlot.test.ts`

**Interfaces — Consumes:** `Layout.bottomPerPx` (Task 4). **Produces:** the report's rule is `{ pieces, top, bottom, bottomPerPx, atHeight }`; Rust:
```rust
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub pieces: Vec<Piece>, pub top: f64, pub bottom: f64,
    /// How much the distance from the window's foot grows for each pixel the window is taller than `at_height`.
    #[serde(default)] pub bottom_per_px: f64,
    /// The window height `bottom` was measured at.
    #[serde(default)] pub at_height: f64,
}
// place(): bottom = self.bottom + self.bottom_per_px * (win.height - self.at_height)
```
`slotReport(slot, win, inputs, moving, bottomPerPx = 0)`.

- [ ] **Step 1: Write the failing tests.** `rules.rs`: `a_rule_without_a_slope_places_as_before` (the existing expectations with the two new fields at 0); `a_rule_with_a_slope_keeps_the_page_in_its_panel` (top 40, bottom 300, `bottom_per_px` 0.5, `at_height` 800: at height 900 the box is 510 tall, at 700 it is 410); `a_slope_never_gives_a_negative_height` (a window far shorter than `at_height` → height 0); `a_report_without_the_new_fields_parses` (JSON with only `pieces`, `top`, `bottom`); `the_rule_reads_camel_case` (`bottomPerPx`, `atHeight`). `usePageSlot.test.ts`: `the rule carries the stack's slope and the height it was measured at`; `with no slope the rule is as before`; `a slot with no size still reports no rule`.
- [ ] **Step 2: Run** `cargo test -p marshell-app -j 4 rules` and `pnpm test src/features/browser/usePageSlot.test.ts` → FAIL.
- [ ] **Step 3: Implement.** `measure` reads `layout?.bottomPerPx ?? 0`. A non-finite slope or height is 0.
- [ ] **Step 4: Run** `cargo test -p marshell-app -j 4`, `cargo clippy -p marshell-app --all-targets -- -D warnings`, `pnpm test src/features/browser`, `pnpm build` → pass.
- [ ] **Step 5: Commit** — `cargo fmt --all`, then `feat(browser): a page's placement rule follows its panel when the window's height changes.`

### Task 6: `SidePane` becomes one panel's frame

**Files:**
- Modify: `src/features/pane/SidePane.tsx`, `SidePane.test.tsx`, `SidePane.css`, `SidePane.css.test.ts`, `focusPane.ts`, `focusPane.test.ts`, `src/features/sessions/LiveApp.test.tsx:725` (it renders `SidePane`)
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
Markup: `<section class="side-pane" aria-label="Browser" data-nav-side="browser">` (no `data-drop-pane`: the stack has it). The tablist is `aria-label="Browser tabs"`. After the spacer, in this order: + (`data-pane-plus=""`, `aria-label` and `title` = `plus.label`, `aria-haspopup="menu"` only when `plus.menu`; when `plus.disabled`: `aria-disabled="true"`, title `${plus.label}: ${plus.disabled}`, no click), ⋯ when `menu` (`aria-label` and `title` = `menu.label`, `aria-haspopup="menu"`), ⤢ (`aria-label="Fill the side pane"`, `aria-pressed`, title "Fill the side pane", or "Show the other panels" while `fill.on`; when `fill.disabled`: `aria-disabled="true"`, title = the reason, no click), × (`aria-label="Hide Browser"`, title `Hide Browser (${hideKeys})`). `os` leaves the props. `focusPane`'s first-button selector for an empty panel is `.empty-panel__button:not([aria-disabled="true"])`.

- [ ] **Step 1: Write the failing tests.** `SidePane.test.tsx`: `is its panel's section and a navigation stop` (label "Files", `data-nav-side="files"`, no `data-drop-pane`); `names its tablist after the panel`; `ends with +, the menu when it has one, Fill and Hide, in that order`; `the Files panel has no menu button`; `+ opens a menu only when told to`; `a + that can't be used is aria-disabled and says why`; `Fill is pressed while its panel fills, and its tooltip says how to go back`; `Fill with one panel shown is aria-disabled and says why`; `Hide names the panel and its key`; `with no tabs shows no tablist, and still its buttons`. The tests of the tab marks keep their titles. `focusPane.test.ts`: every selector starts with `[data-nav-side="files"]` for Files; `the + button is found by data-pane-plus`; `the empty panel's first usable button takes the keyboard`; `panelOfElement` (inside Browser → "browser", outside → null); `a closing tab is looked for in its own panel`. `SidePane.css.test.ts` keeps its titles.
- [ ] **Step 2: Run** `pnpm test src/features/pane/SidePane src/features/pane/focusPane.test.ts` → FAIL.
- [ ] **Step 3: Implement.** The strip's `focusAfter` fallback focuses the + button as before. The ⋯ and ⤢ icons are 12 px paths in the file's `icon()` style.
- [ ] **Step 4: Keep the build green.** `LiveApp.tsx` passes `panel="files"`, `plus={{ label: "New tab", menu: true }}`, `fill={{ on: false, disabled: "No other panel is shown" }}`, `hideKeys`, `onFill={() => {}}`, and calls `focusPane("files", …)` everywhere (marked); `LiveApp.test.tsx:725` gets the new props.
- [ ] **Step 5: Run** `pnpm test src/features/pane src/features/sessions src/design` and `pnpm build` → pass.
- [ ] **Step 6: Commit** — `feat(pane): the pane's frame is one panel's, with its own +, menu, Fill and Hide.`

### Task 7: The empty states (main thread: the look is a judgement)

**Files:**
- Create: `src/features/pane/EmptyPanel.tsx` (the shared block), `EmptyBrowser.tsx`, `EmptyFiles.tsx`, `EmptyPanel.test.tsx`, `EmptyPanel.css`, `EmptyPanel.css.test.ts`
- Delete: `src/features/pane/EmptyPane.tsx`, `EmptyPane.test.tsx`; the `.empty-pane*` rules in `SidePane.css` (the hover block about `:186-190`, the rules about `:208-275`, and those in its forced-colors block)
- Modify: `src/features/sessions/LiveApp.test.tsx` (`:9` imports `EmptyPane`; `:743,751` pin "No tabs open in this session." and "Drop a file here to open it."), `LiveApp.tsx` (use `EmptyFiles`, marked)

**Interfaces — Produces:**
```ts
export function EmptyBrowser(p: { available: boolean; ports: readonly { port: number; url: string }[]; onNewTab(): void; onOpenHtml(): void; onPort(url: string): void }): JSX.Element;
export function EmptyFiles(p: { recent: readonly string[]; onOpenFile(): void; onBrowse(): void; onRecent(path: string): void }): JSX.Element;
```
**Copy and structure (exact).** Both: a centred column, `<div class="empty-panel">`, holding a decorative 40 px icon (`aria-hidden`), `<h2 class="empty-panel__title">`, `<p class="empty-panel__line">`, a row of two buttons (`empty-panel__button`, the first also `empty-panel__button--primary`; each an icon and its words), then an optional list under a small heading (`<h3 class="empty-panel__heading">`).
- Browser: globe · "No pages open" · line "Open an address, an HTML file or a server this session runs." with ports, "Open an address or an HTML file." without · primary "New tab" (plus icon) · "Open HTML file…" (file icon) · heading "Running in this session" over one button per port, labelled `localhost:<port>`, in the order given.
- Browser with `available: false`: the line is "Open an HTML file. Web pages aren't available on this system yet."; "Open HTML file…" comes first and is the primary one; "New tab" follows, `aria-disabled="true"`, title "Not available on this system yet", no click; no ports are listed.
- Files: folder · "No files open" · "Open a file from this project, or drop one here." · primary "Open file…" (search icon) · "Browse…" (folder-open icon) · heading "Recent in this project" over one button per recent file showing the file's name and its folder (as `EmptyPane` does today); no heading with no recent files.

- [ ] **Step 1: Write the failing tests** in `EmptyPanel.test.tsx`: `the Browser block says there are no pages and offers New tab and Open HTML file…` (the exact strings; the first button has the primary class); `lists the session's ports as buttons, in order`; `leaves the ports row out and changes its line when the session listens on nothing`; `where pages aren't available Open HTML file… leads, New tab is aria-disabled with its reason, and no port is listed`; `the Files block says there are no files and offers Open file… and Browse…`; `lists recent files by name and folder under Recent in this project`; `shows no heading with no recent files`; `each block's icon is hidden from a screen reader`. `EmptyPanel.css.test.ts`: no hex; system colours only in forced colours; `:hover` only under a fine pointer; no `border` on `.empty-panel__button`, and `outline` only under `:focus-visible` and in the forced-colors block. Move `LiveApp.test.tsx`'s two empty-pane assertions to the new copy.
- [ ] **Step 2: Run** `pnpm test src/features/pane/EmptyPanel` → FAIL.
- [ ] **Step 3: Design and implement.** Invoke the `frontend-design` and `good-css` skills first. The block is centred in its panel and scrolls inside it when the panel is short; a panel can be 160 px tall, so under a container height of 280 px the icon is dropped with a container query, and the buttons never are. The primary button is filled with the accent tokens, the second tinted, both at least 28 px tall; the icon takes the quietest text token; the lists are quiet rows with the hover tint. Forced colours: the primary button is `Highlight`/`HighlightText`, the second `ButtonFace`/`ButtonText`, focus rings `Highlight`.
- [ ] **Step 4: Run** `pnpm test src/features/pane src/features/sessions src/design` and `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(pane): the empty Browser and Files panels say what to do, with an icon and two clear buttons.`

### Task 8: The title bar's panel buttons

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
- [ ] **Step 3: Implement.** `LiveApp.tsx` passes two buttons built from `PANELS` with `shown: view.pane.open`, `news: paneNews.size > 0`, `keys` from `commandById("preview")` for now, and `onTogglePanel={togglePane}`; its `activate` case maps both panel ids to `togglePane()` (marked).
- [ ] **Step 4: Run** `pnpm test src/features/window src/styles src/features/sessions src/design` and `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(window): the title bar has a button for each panel in place of the side pane's.`

### Task 9: Ctrl+Shift+Tab stops at each panel (worker `effort: high`: the navigation machine)

**Files:**
- Modify: `src/lib/navigation/types.ts`, `machine.ts`, `machine.test.ts`, `src/features/sessions/navLayout.ts` (+ test), `useNavigation.ts` (+ test), `src/lib/keymap.ts:81`, `src/design/NavDemo.tsx` if it builds a `side` context
- Modify for the label: `src/lib/bindings.test.ts:142`, `src/lib/keymap.test.ts:92`, `src/features/sessions/LivePalette.test.tsx:49`, `LiveShortcuts.test.tsx:35`, `src/features/shortcuts/ShortcutSheet.test.tsx:38`
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

- [ ] **Step 1: Write the failing tests** in `machine.test.ts`: `Tab goes terminal, Browser, Files, terminal`; `Tab skips a panel that isn't on screen` (side `[Files]`); `from Browser Tab goes to Files, not to the terminal`; `with no panel on screen Tab stays on the terminal's ring`; `a selected panel that left the screen is forgotten` (`{ selectedPane: 0, selectedSide: "browser" }` with side `[Files]`: Tab selects Files, as it does from terminal 0); `with the cursor out Tab carries it over each panel and selects on release`; `an arrow moves the cursor between the two panels by their boxes` (Browser above Files: Down from Browser → Files); `says Selected Browser and Selected Files`. `navLayout` test: `a panel is a target named by its id`. `useNavigation.test.ts`: `the cursor's hint on a panel is its name`. The five label tests take the new label.
- [ ] **Step 2: Run** `pnpm test src/lib src/features/sessions src/features/shortcuts` → FAIL.
- [ ] **Step 3: Implement.** `sameTarget` compares `panel`. `sideSelected` becomes "the selected panel, when it is still in `ctx.side`".
- [ ] **Step 4: Keep the build green.** `LiveApp.tsx`: `side: sidePaneDrawn ? [{ id: "files", name: "Files" }] : []`, `sideFocused: sideFocused ? "files" : null`, `select-side` → `focusPane("files")` (marked).
- [ ] **Step 5: Run** `pnpm test src/lib src/features/sessions src/features/shortcuts src/design` and `pnpm build` → pass.
- [ ] **Step 6: Commit** — `feat(nav): each panel of the side pane is a stop for Ctrl+Shift+Tab.`

### Task 10: The app shows the stack: panels, buttons, keys, focus (worker `effort: high`: it rewires `LiveApp`)

**Files:**
- Modify: `src/features/sessions/LiveApp.tsx` (`:938-943` only for `panelButtons`, `:1027-1090`, `:1484-1568`, `:2025-2165`, `:2279-2300`), `LiveApp.test.tsx`, `src/lib/keymap.ts`, `src/lib/appShortcuts.ts` (+ tests), `src/features/pane/KeptTabs.test.tsx`

**Interfaces — Consumes:** Tasks 2 to 9. **Produces:** `COMMANDS` gains, after `preview`: `{ id: "browser-panel", label: "Browser panel", group: "Window", binding: { code: "KeyA", mac: { code: "KeyA", alt: true } } }` (comment: ⌘A is Select all on macOS) and `{ id: "files-panel", label: "Files panel", group: "Window", binding: { code: "KeyD" } }`; `SessionShortcut` gains `{ kind: "panel"; panel: "browser" | "files" }` and `commandShortcut` maps the two ids to it. In `LiveApp`: `panelsOn`, `currentPanel()`, `togglePanel(panel)`, `renderPanel(panel)`, which Task 11 uses.

**What `LiveApp` does:**
1. `const stack = stackOf(view.pane, focusedId)`; `const beside = mainView === "sessions" && focusedId != null`; `const panelsOn = beside ? onScreen(view.pane, focusedId) : []`. `paneShown={view.pane.open}` as today. An effect dispatches `{ type: "pin", session: focusedId }` once `settled && loaded`, the pane is open, `view.pane.panels` is absent and `focusedId != null` — never with no session on screen.
2. `paneNode` is `<PanelStack panels={stackPanels(stack)} weights={stack.weights} render={renderPanel} onWeights={(weights) => paneDispatch({ type: "weights", weights })} onDividerGone={() => focusPane(currentPanel() ?? "files")} />`: the stack's panels whether or not the pane is open, so nothing inside changes while the pane holds and folds.
3. `renderPanel(panel)` is a `SidePane` for `panelTabs(view.pane, focusedId, panel)` with `activeIn`, holding `<KeptTabs key={`${session}:${panel}`}>` (the one `render` for a browser tab and a file tab that exists today) or `EmptyBrowser` / `EmptyFiles`. `movedText`'s count is the panel's tab count.
4. Browser's `plus` is `{ label: "New tab", menu: false, disabled: browserTabs(os) ? undefined : "Not available on this system yet" }` and its `onPlus` calls `openPageInPane(BLANK, { focus: "address" })`. Files' is `{ label: "Open a file", menu: true }` and opens today's + menu, labelled "Open a file". Browser has `menu={{ label: "Browser menu" }}` with `onMenu` a no-op until Task 12. `fill` is `{ on: stack.filled === panel, disabled: stack.shown.length < 2 ? "No other panel is shown" : undefined }`; `onFill` dispatches `{ type: "fill" }`. `hideKeys` is the panel command's `shortcutLabel`.
5. `EmptyBrowser`: `available={browserTabs(os)}`, `ports={focusedRow?.ports ?? []}`, `onNewTab` as the + button, `onPort={(url) => openPageInPane(url)}`, `onOpenHtml` a no-op until Task 12.
6. `togglePanel(panel)` (the key, the title-bar button, the cursor's release on it, ×): blocked as `togglePane` is (announce `PANE_BESIDE_SESSION`); else read `panelToggleOutcome` and where the keyboard is, dispatch `{ type: "panel" }`, then: it shows → `focusPane(panel)`; it hides → `announce(panelHiddenText(panel))` and `hideFocus(panel, panelFocus, covers, the panels that stay)`: "terminal" → `actions.focusTerminal()`, "panel" → `focusPane(that)`, "stay" → nothing. `windowCommand` handles `{ kind: "panel" }` with it; `activate` maps `panel-browser` and `panel-files` to it.
7. Ctrl+Shift+Q (`preview`): `{ type: "toggle", session: focusedId ?? null }`; on show the keyboard goes to `commandPanel(...)` of the panels then on screen (computed from the reducer's result); on hide to the terminal.
8. `panelFocus` replaces `sideFocusedInDom`: the panel `document.activeElement` is inside (`panelOfElement`), from `focusin`/`focusout`; a page with the keys counts as Browser. A ref `lastPanel` holds the last non-null value. `currentPanel()` is `commandPanel(panelsOn, panelFocus, lastPanel.current, (p) => panelTabs(view.pane, focusedId, p).length > 0)`. `useNavigation` gets `side: panelsOn.map((id) => ({ id, name: PANEL_LABEL[id] }))` and `sideFocused: panelFocus`; `select-side` → `focusPane(effect.panel)`.
9. `panelButtons={panelButtonsFor(panelsOn, paneNews, keysOf)}`.
10. `toTerminalFromPane` and Esc are unchanged. `shownPage`: Browser is in `panelsOn` and its selected tab is a browser tab.

- [ ] **Step 1: Write the failing tests.** `appShortcuts` test: `Ctrl+Shift+A is the Browser panel and Ctrl+Shift+D the Files panel` (and ⌘⌥A, ⌘D on macOS); `bindings.test.ts`'s "free of a reserved chord on every system" and "free of duplicates" hold with the two. `KeptTabs.test.tsx`: `two panels keep their own four tabs` (two instances with their own `keptRef`). `LiveApp.test.tsx`, in its existing style (it renders the parts as `LiveApp` composes them): `the stack draws a section for each panel, Browser first`; `a filled panel is drawn alone`; `an empty Browser panel lists the session's ports`; `nothing is pinned with no session on screen` (export the pure `shouldPin(pane, focusedId, ready): boolean` beside `paneCommands.ts` and test: absent stack + open + session → true; no session → false; a saved stack → false; hidden → false).
- [ ] **Step 2: Run** `pnpm test src/features/sessions src/lib src/features/pane` → FAIL.
- [ ] **Step 3: Implement** lines 1 to 10.
- [ ] **Step 4: Run** `pnpm test` (the whole suite) and `pnpm build` → pass.
- [ ] **Step 5: Commit** — `feat(pane): the app shows the Browser and Files panels, with their buttons, keys and focus.`

### Task 11: The tab commands, opens and drops act per panel (worker `effort: high`)

**Files:**
- Modify: `src/features/sessions/LiveApp.tsx` (`:700-735`, `:855-860`, `:905-945`, `:951-996`, `:1027-1067`, `:1093-1125`, `:1590-1602`, the Preview's `onOpen`, `onBack`, `onForward` in `renderPanel`), `src/features/browser/useBrowserPages.ts:119` only if its "on screen" needs the panel, tests named below

**What `LiveApp` does:**
1. Ctrl+Tab and "Previous tab": the pane hidden and the session has a tab → show it (as today, with line 7 of Task 10's focus); else `panel = currentPanel()`; no panel, or no tabs in it → `announce(panel ? noTabsText(panel) : NO_TABS_OPEN)`; else `{ type: "step", session, panel, by }`, and focus as `paneTabFocus` says, into that panel. "Close tab" closes `activeTab(pane, session, currentPanel())`, asking first for unsaved edits as today, and keeps the keyboard in that panel when it was there.
2. `shownPreview`, `shownPdf` and `selectRegion` go through `firstWith(panelOrder(panelsOn, currentPanel()), …)`: the current panel's selected tab first, then the other panel on screen, the first that has what they need.
3. `openInPane` and `openPageInPane`: the panel is `panelOfPath(target.path)` / `"browser"`; announce `openedText(name, panel)`; `focusPane(panel)`. `openPageInPane` finds the opened tab with `activeTab(next, session, "browser")`.
4. A Preview's `onOpen` calls `openInPane(target)` when `panelOfPath(target.path) !== panel` (so the 32-tab limit is said), else dispatches `navigate`. Its `onBack`/`onForward`: when the entry they land on belongs to the other panel and the keyboard was inside this tab, `focusPane(thatPanel)` after the dispatch.
5. `marshell open`: `takeOpens` gets `onScreen: panelsOn`; news is added per panel; the seen effect runs for each panel in `panelsOn` (`newsDispatch({ type: "seen", session, panel })`, and `{ type: "seen", session, panel }` to the pane when that panel's selected tab is unseen).
6. `askToClose` compares with `activeIn(tabs, panelOf(tab))`.
7. Drops keep today's loop (`:1100-1118`: the reducer is followed locally, so the 32-tab limit lands on the right file): each file opens in its own panel; the one line said at the end is `openedText(name, panelOfPath(last))`, and `focusPane` goes to the last file's panel. The label stays "Drop to open in the side pane".
8. Remove every marker: `rg "6r-1a: replaced" src` → nothing.

- [ ] **Step 1: Write the failing tests** for whatever pure piece each line needs and doesn't have yet (the outcomes of Task 3 cover 1, 2 and the focus rules). New: `dropPlan(pane, session, project, paths: readonly { path: string; ok: boolean }[]): { actions: PaneAction[]; last: string | null; refused: boolean }` in `src/features/pane/opens.ts`, with `a drop of 3 files at 31 tabs opens one and says the limit`, `a drop of an HTML file and a text file opens each in its panel and reports the last`, `a folder alone opens nothing`; `LiveApp` calls it instead of its inline loop.
- [ ] **Step 2: Run** `pnpm test src/features/pane/opens.test.ts` → FAIL.
- [ ] **Step 3: Implement** lines 1 to 8.
- [ ] **Step 4: Run** `pnpm test` and `pnpm build` → pass. `rg -n "Hide the side pane|aria-label=\"Side pane\"|data-nav-top=\"pane\"|6r-1a: replaced" src` → nothing outside a test's negative assertion.
- [ ] **Step 5: Commit** — `feat(pane): tab commands, opens, marshell open and drops go to the panel they belong to.`

### Task 12: The Browser menu

**Files:**
- Create: `src/features/browser/browserMenu.ts`, `browserMenu.test.ts`
- Modify: `src/features/browser/UrlBar.tsx` (`:24`, `:59`, `:145-154`), `UrlBar.test.tsx` (`:31`, `:41`, `:58-70`, `:99`), `BrowserTab.tsx` (`:124-137`: stop passing `onOpenOutside` to `UrlBar`; the unavailable branch keeps its button), `src/lib/filePicker.ts`, `src-tauri/src/file_picker.rs`, `src/features/sessions/LiveApp.tsx`

**Interfaces — Produces:**
```ts
export type MenuTarget = { kind: "none" } | { kind: "blank" } | { kind: "file" } | { kind: "page"; url: string };
export function menuTarget(tab: PaneTab | undefined): MenuTarget;        // the Browser panel's selected tab
export function browserMenuItems(t: MenuTarget, pagesAvailable: boolean): MenuItem[];
export type BrowserMenuChoice = "copy" | "outside" | "html" | "clear" | null;
export function browserMenuChoice(id: string): BrowserMenuChoice;
// filePicker.ts
export async function pickFile(start: string | null, kind?: "html"): Promise<string | null>;
```
Rows, in order: "Copy link", "Open in the system browser", a separator, "Open HTML file…", a separator, "Clear browsing data…". The first two carry `disabled: "No page is open"` for `none` and `blank`, and `disabled: "An HTML file has no address"` for `file`. "Clear browsing data…" carries `disabled: "Not available on this system yet"` when `!pagesAvailable`. Rust: `pick_file(window, start: Option<String>, kind: Option<String>)`; `kind == Some("html")` adds the filter `("HTML files", ["html", "htm"])`; any other value adds none.

- [ ] **Step 1: Write the failing tests.** `browserMenu.test.ts`: `lists the four rows in order, with two separators`; `a page enables Copy link and Open in the system browser`; `a blank tab and no tab say No page is open`; `an HTML file says it has no address`; `Open HTML file… is never disabled`; `Clear browsing data… says why where pages aren't available`; `a row's id is its choice, and an unknown id is null`. `UrlBar.test.tsx`: `has no Open in the system browser button`, and the order tests without it. Rust (in `file_picker.rs`, on a pure `fn filter_for(kind: Option<&str>) -> Option<(&'static str, &'static [&'static str])>`): `html_gets_the_html_filter`, `any_other_kind_gets_none`.
- [ ] **Step 2: Run** `pnpm test src/features/browser` and `cargo test -p marshell-app -j 4 file_picker` → FAIL.
- [ ] **Step 3: Implement.** In `LiveApp`: a `browserMenu` state like `plusMenu`; `onMenu` opens a `ContextMenu` labelled "Browser menu" beside the + menu's; a pick gives focus back to the ⋯ button first, then: `copy` → `navigator.clipboard.writeText(url)` then "Copied the link." (on failure "Couldn't copy the link."); `outside` → `openUrl(url)` as `onOpenOutside` does; `html` → `pickFile(cwd, "html")` then `openInPane({ path })` (the failure line is Browse…'s); `clear` → `setClearData("ask")`. `EmptyBrowser`'s `onOpenHtml` is the same `html` action.
- [ ] **Step 4: Run** `pnpm test src/features/browser src/features/sessions` (`src/features/browser/coverScan.test.ts` is among them), `pnpm build`, `cargo test -p marshell-app -j 4`, `cargo clippy -p marshell-app --all-targets -- -D warnings` → pass.
- [ ] **Step 5: Commit** — `cargo fmt --all`, then `feat(browser): the Browser panel has a menu, and Open in the system browser moves into it.`

### Task 13: The plan's text follows, then the live check (main thread)

**Files:** `docs/PLAN.md` outside §9's past decisions: the UX table's side pane row (about `:140`) and §7's rows (about `:716` and `:732`); `docs/research/2026-11-21-side-pane-panels-live-check.md`.

- [ ] **Step 1:** Bring those three places to what was built (the two panel buttons and keys, Ctrl+Shift+Tab's stops), each with a pointer to "Slice 6r-1a decisions". Earlier slices' decision records stay as written; the one at about `:888` ("There is no file tree; that stays in v2") gets only "(overturned by Slice 6r decisions)". Commit — `docs(plan): the side pane's text follows the Browser and Files panels.`
- [ ] **Step 2: Live check** with the `marshell-live-check` skill (scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, `USERPROFILE`; the app is never brought to the front; only what the check started is stopped, by PID). Prefer `browser_evaluate` returning small JSON; screenshots only for the look. After every criterion read the webview's console: "Could not save the layout" anywhere fails the check. Write what was seen, criterion by criterion, in the research note:
  1. A scratch `layout.json` saved by `main` with the pane open, a web tab selected and two file tabs: the first start shows Browser over Files, the web tab selected in Browser, a file in Files; `layout.json` then holds `panels` and `active_browser`.
  2. Ctrl+Shift+A and Ctrl+Shift+D show and hide their panels; the last one hides the pane; Ctrl+Shift+Q brings back what was shown; a key with the pane hidden shows that panel alone. `aria-expanded` and the tooltips follow.
  3. Ctrl+click on a path in the terminal with only Browser shown adds Files under it with the keyboard in the file; Ctrl+click on a `localhost` address with only Files shown adds Browser.
  4. The divider: a drag, Up/Down, Home/End, a double-click; the split survives a window resize and a restart; a 300 px tall pane splits evenly.
  5. ⤢ fills and restores; opening into the other panel ends the fill; hiding the filled panel brings the other back; the fill survives a restart.
  6. A real page in Browser over Files. Sampled every frame (a `requestAnimationFrame` loop that records the slot's box and the page's box as the shell reports it) during: a show and a hide of Files, a fill and its end, a divider drag, and a window-height drag of 200 px. The page's foot is never more than 2 px below its slot's once the window is still, and during the height drag never more than one frame's step. Screenshots show nothing of the page over Files. The page is gone when Browser hides.
  7. Ctrl+Shift+Q with Browser over Files and a page on screen: screenshots at about 50 ms and 150 ms show both panels at their settled heights inside the folding pane, the page as its picture.
  8. Ctrl+Shift+Tab goes terminal, Browser, Files, terminal; Ctrl+Tab steps the tabs of the panel that had the keyboard last while typing in the terminal.
  9. `marshell open` from a shell session for a `.md` and for an address while only the other panel is shown: no panel appears, focus stays, the dot is on the right button and goes when that panel shows.
  10. A narrow window where the pane covers the terminal, Browser over Files, the keyboard in Files: Ctrl+Shift+D leaves the keyboard in Browser; Ctrl+Shift+A then hides the pane and the terminal has it.
  11. A drop of three files on the pane at 31 tabs opens one and says the limit; a drop of an `.html` and a `.md` opens each in its panel.
  12. The empty states (screenshots in the dark and the light theme, and in forced colours through `browser_emulate_media`): the ports row with a listening dev server; "New tab", "Open HTML file…", "Open file…", "Browse…" each work by keyboard. Send the screenshots to the user.
  13. The ⋯ menu: Copy link puts the address on the clipboard; the disabled rows say why on a blank tab and on an HTML file; Open HTML file… opens the dialog (cancelled by the check); Clear browsing data… opens its question. An `.html` file shows in Browser with Rendered and Source.
  14. Reduced motion: a panel shows at once.
- [ ] **Step 3:** Fix what the check finds (each fix with its test), rerun the criterion, commit the note — `docs(research): record slice 6r-1a's live check.`

## After the tasks

- Final whole-branch review by `marshell-reviewer` (Opus) with this plan's Review Focus, one fix pass, then `marshell-ship` (the CI commands, the PR with "What it does", "Live check", "Known limits", "Checks"), a green CI, `pre-merge-review`, the merge. Delete the ledger `.superpowers/sdd/2026-11-21-phase-6r-1a-side-pane-panels/progress.md` after the merge, and switch the advisor off.
- Known limits to state in the PR: macOS and Linux are unverified (the Browser panel holds HTML files only there); an HTML file runs no scripts (6r-1b's spike); the palette's "Clear the browser's data…" and the menu's "Clear browsing data…" are two names for one thing; the ⌘ keys for macOS are chosen from the reserved list, not tried; the Files panel's recent list can name an HTML file, which opens in Browser.
- Risks: (a) the page under a moving stack and a height drag — proven by live criteria 6 and 7; the fallback is to hide the page for the motion's 200 ms; (b) `LiveApp.tsx` is large and Tasks 10 and 11 touch many places — the tasks list them, and the whole suite runs at each end; (c) the interim tasks leave the pane wrong on screen — nothing is checked live before Task 11.
