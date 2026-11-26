# Slice 6x: a panel that expands over the terminal, word wrap for every file, and two faults. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A panel's ⤢ button expands it over the side pane and the terminal, per session and saved. Every file wraps by default. A browser page can no longer stay blank after a layout motion, and a file opened late no longer lands behind the narrow tree.

**Architecture:**
- **Motion (the blank page).** `useLayoutMotion` reads the clock while rendering, so two renders of the same state can disagree on `moving`. React can render again, throw the render away and still keep the effect dependencies it compared, which left `useCoverPage(…)` in `MainWindow` holding a cover forever. The frame time becomes state: a render draws from the last frame's time and reads the clock only when the key changes.
- **Saved shape.** `pane.panels.filled` (one for the window) goes. `pane.sessions.<id>.expanded_panel` (one per session) takes its place. Which panels are shown, and their weights, stay the window's.
- **Geometry.** An expanded pane is the existing `"cover"` mode: `paneGeometry` takes `expanded` and returns `cover` where it would return `beside`. Everything a window under 720 px already does (terminals mounted and suspended, no divider, one grid column) applies unchanged. `<main>` carries `data-expanded` while the cover is the expansion's alone, so the keys that leave the pane can tell it from a narrow window.
- **Button.** One pure function decides the button's name, pressed state, tooltip and disabled state from three facts: on, other panels shown, a terminal beside the pane. The window publishes the third through `LayoutContext`.
- **Word wrap.** The default of `files.word_wrap` flips to on in Rust and TypeScript. `settings.json` goes to version 2, and the step from 1 turns a stored `false` into `true` once.

**Tech Stack:** React 19, TypeScript, Vitest, Rust (`marshell-protocol`, `marshell-core`), ts-rs. No new dependency. Nothing under `src-tauri/`.

**Spec:** `docs/PLAN.md` §9, "Slice 6x decisions (2026-11-26)": decisions 1 to 4 and the list under them.

## Global Constraints

- No new dependency. Nothing under `src-tauri/`, `.github/` or `scripts/` changes.
- `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4` regenerates it, and the result is committed. `pnpm check:generated` must pass.
- Cargo always runs with `-j 4`. `cargo fmt --all` before every Rust commit.
- Vitest runs in node with no DOM. Components are tested through `renderToStaticMarkup`, logic through pure functions.
- Every change works with the keyboard alone and in forced colours. `Announcer` is the only live region.
- Copy is fixed by this plan, word for word:
  - Button name: `Expand Browser`, `Expand Files`, `Expand Changes`.
  - Tooltips while on: `Show the terminal and the other panels`, `Show the terminal`, `Show the other panels`.
  - Disabled: `No other panel is shown`.
  - Announced when a key ends the expansion: `Files is beside the terminal again.` (the panel's label first).
  - Settings → Files, under "Word wrap": `When on, every file wraps. When off, Markdown and plain text wrap and code doesn't. Alt+Z changes the tab in front.`
- No CSS declaration is added. If one turns out to be needed, read the `good-css` skill first.
- Each task: write the test first and watch it fail. One file: `pnpm test <path>`.
- Commit messages: `type(scope): what changed, as a sentence.`
- Workers: stay inside the task's files; never `git checkout`, `git restore`, `git stash`, `git reset` or `git clean`; don't commit (the main thread commits after reading the diff); scratch files go under the system temp folder, in a folder of the task's own; GitHub only through the `gh` CLI. Don't start `pnpm tauri dev`.
- A dev app is running from the main checkout (`C:\dev\Airport`) for the user. Edit files only in this worktree.
- Verified before writing (2026-11-26, at `66f97a6c`):
  - `useLayoutMotion` (`src/features/window/motion.ts:74`) calls `clock()` in render; its callers are `MainWindow.tsx:294`, `:366` and `PanelStack.tsx:87–89`. `MainWindow.tsx:387` is `useCoverPage(paneHeld || (paneMotion.moving && !paneOn))`, and `:370` sets `rtl` after every render.
  - In the running app the stuck cover's only holder was the release closure of that `useCoverPage`, with dependencies `[false]`. A detached copy of the hook's loop stuck 3 times in 60 with a same-value `setState` after each render, 0 in 30 without.
  - `Panels.filled` is `crates/protocol/src/api.rs:573`; `SessionTabs` is `:520`. `layout.rs` handles `filled` at `:134`, `:521`, `:738`; a session's entry is dropped at `:649` unless it has tabs, a tree or changes. No type in `api.rs` or `layout.rs` uses `deny_unknown_fields`, so an old `filled` is ignored on read.
  - The stack is one for the window: `PaneView.panels`. `stackOf` (`src/features/pane/panels.ts:38`) and `fill` (`paneState.ts:520`) take a session only to imply a stack when none is saved.
  - `paneGeometry` (`src/features/window/paneLayout.ts:33`) returns `fill` for `alone`, `cover` under 720 px, else `beside`. `KeptTerminals` is suspended while `paneCovers && !paneMotion.moving` (`MainWindow.tsx:547`).
  - `LayoutContext.Provider` wraps `<main>` and the pane node (`MainWindow.tsx:525–612`), so `SidePane` can read it.
  - `store::read` (`crates/core/src/store.rs:85`) writes a migrated file back and returns `Err` when that write fails; `settings::load` (`settings.rs:242`) then falls back to the defaults. `settings.json` is at version 1 with no `migrate`.
  - `treeAfter` (`src/features/files/filesActions.ts:77`) keeps a narrow tree up when the path changes while the keyboard is in it. `openInPane` (`LiveApp.tsx:1067`) and the drop (`LiveApp.tsx:1340`) run after an `await`, with the keyboard still in the tree.

## Review Focus

1. **The settings file can't be written at the one-time switch** (locked by another program, a read-only folder). The person's settings still load, with wrap on; they don't fall back to the defaults. Test in Task 4.
2. **Two sessions, one expanded.** Expanding in session A leaves B's terminal and panels as they were, and hiding the expanded panel in B ends A's expansion. Tests in Task 2; the switch itself in the live check.
3. **A window narrowed under 720 px while a panel is expanded, then widened.** The pane covers either way; `data-expanded` is absent while narrow, so Esc hides the pane as before; wide again, the panel is expanded again. Tests in Task 3.
4. **No session.** With the shell panel as the pane's owner and no session, the pane already fills the room: expanding changes only which panels are drawn, and the mode stays `fill`. Test in Task 3.
5. **A layout.json from before the slice** holding `"filled": "files"` loads without an error, keeps its shown panels and weights, and is saved without `filled`. Test in Task 2.

---

### Task 1: The layout motion draws from its last frame (the blank page)

Inline. **Files:**
- Modify: `src/features/window/motion.ts`
- Test: `src/features/window/motion.test.ts`

**Interfaces.** Produces, in `motion.ts`:

```ts
export type MotionState = { key: string; motion: Motion; drawn: number };
/**
 * One render of `useLayoutMotion`; `state` is updated in place. `frame` is the time of the last animation frame.
 * `clock` and `reduced` are called only when `key` differs from `state.key`, so a render repeated with the same
 * state draws what the last one drew.
 */
export function motionRender(state: MotionState, key: string, target: number, frame: number, clock: () => number, reduced: () => boolean): { value: number; moving: boolean };
/** The next frame's time: the clock's, and never the last one's again (an equal value would skip the render and stop the motion). */
export function nextFrame(last: number, now: number): number; // now > last ? now : last + 1
```

`useLayoutMotion` keeps its signature. Inside: `const [frame, setFrame] = useState(0)`, `motionRender(state.current, key, target, frame, clock, reducedMotion)`, and the effect's frame callback is `setFrame((f) => nextFrame(f, clock()))`.

- [ ] **Step 1: Write the failing tests** in `describe("layout motion")`:

```ts
it("a render repeated at the same frame draws the same thing, whatever the clock says", () => {
  const s: MotionState = { key: "a", motion: null, drawn: 0 };
  const clockAt = (t: number) => () => t;
  expect(motionRender(s, "b", 100, 0, clockAt(1000), () => false)).toEqual({ value: 0, moving: true });
  // A stale frame (older than the start) still draws the start.
  expect(motionRender(s, "b", 100, 0, clockAt(1500), () => false)).toEqual({ value: 0, moving: true });
  const mid = motionRender(s, "b", 100, 1199, clockAt(1199), () => false);
  expect(mid.moving).toBe(true);
  // The fault: this second render, 2 ms later by the clock, used to say the motion had ended.
  expect(motionRender(s, "b", 100, 1199, clockAt(1201), () => false)).toEqual(mid);
  expect(motionRender(s, "b", 100, 1200, clockAt(1200), () => false)).toEqual({ value: 100, moving: false });
  expect(motionRender(s, "b", 100, 1200, clockAt(9999), () => false)).toEqual({ value: 100, moving: false });
});
it("the clock is read only when the key changes", () => {
  const s: MotionState = { key: "a", motion: null, drawn: 0 };
  let reads = 0;
  const clock = () => (reads++, 1000);
  motionRender(s, "a", 50, 0, clock, () => false);
  expect(reads).toBe(0);
  motionRender(s, "b", 100, 0, clock, () => false);
  motionRender(s, "b", 100, 1100, clock, () => false);
  expect(reads).toBe(1);
});
it("reduced motion follows at once and never moves", () => {
  const s: MotionState = { key: "a", motion: null, drawn: 0 };
  expect(motionRender(s, "b", 100, 0, () => 1000, () => true)).toEqual({ value: 100, moving: false });
});
it("a frame never repeats the last one's time", () => {
  expect(nextFrame(10, 12)).toBe(12);
  expect(nextFrame(10, 10)).toBe(11);
  expect(nextFrame(10, 9)).toBe(11);
});
```

- [ ] **Step 2:** Run `pnpm test src/features/window/motion.test.ts`. Expected: FAIL, `motionRender` is not exported.
- [ ] **Step 3:** Implement `motionRender` and `nextFrame`, and rewrite `useLayoutMotion` on them. Update the hook's doc comment: why the frame time is state.
- [ ] **Step 4:** Run `pnpm test src/features/window src/features/pane/PanelStack.test.tsx`. Expected: PASS.
- [ ] **Step 5:** Commit: `fix(window): the layout motion draws from its last frame, so a render run again can't leave a browser page covered.`

### Task 2: Each session's expanded panel (saved shape and pane state)

Worker, `effort: high`. **Files:**
- Modify: `crates/protocol/src/api.rs` (`Panels`, `SessionTabs`), `crates/core/src/layout.rs` (and its tests), `src/generated/` (regenerated)
- Modify: `src/features/pane/panels.ts`, `src/features/pane/paneState.ts`, `src/features/pane/paneRules.ts`, `src/features/sessions/LiveApp.tsx` (renames only)
- Test: `crates/core/src/layout.rs` tests, `src/features/pane/panels.test.ts`, `src/features/pane/paneState.test.ts`, and whatever else names `filled` or `"fill"` (`paneCommands.test.ts`, `LiveApp.test.tsx`)

**Interfaces.** Produces:

- Rust: `Panels` loses `filled`. `SessionTabs` gains
  `/// The panel expanded over the pane and the terminal for this session: one of the shown panels.`
  `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] pub expanded_panel: Option<String>`.
- TypeScript, `panels.ts`: `export type Stack = { shown: PanelId[]; weights: Weights; expanded: PanelId | null }`. `stackOf(pane, session)` sets `expanded` to the session's `expanded_panel` when the stack is a saved one and shows that panel, else `null`. `stackPanels(stack)` is `stack.expanded ? [stack.expanded] : stack.shown`.
- TypeScript, `paneState.ts`: the action `{ type: "expand"; panel: PanelId; session: string | null }` replaces `{ type: "fill"; … }`.

**Rules (Rust, `layout.rs`).**
- `lenient_panels` and `repair_panels` no longer know `filled`. `lenient_session` removes an `expanded_panel` that isn't a string.
- `repair_pane`, after `repair_panels`: a session's `expanded_panel` is kept only while `pane.panels` is `Some` and its `shown` holds that panel. The final `retain` also keeps a session whose only content is an `expanded_panel`.
- `validate_panels` loses its `filled` check. `validate_pane` returns `pane.sessions.{session}.expanded_panel must name a shown panel, not {p:?}` when a session names a panel that `pane.panels` doesn't show (or there is no `pane.panels`).

**Rules (TypeScript, `paneReducer`).** After every action the pane passes `paneProblem`.
1. `expand`: nothing when the pane is hidden, the session is `null`, or the panel isn't in `shown`. Else it switches the session's `expanded_panel` between that panel and none, and writes the stack down (`panels`). It works with one panel shown, and makes the session's entry when it has none.
2. Hiding a panel with its switch (`panel` on a drawn panel) removes that panel's expansion from every session, also when it was the last panel and the pane hides.
3. Showing a panel (`panel` on a panel not drawn) or opening into one (`reveal`) ends the acting session's expansion unless it names that panel. Other sessions keep theirs.
4. Showing a panel alone from a hidden pane (`panel` or `reveal` with `!open`) removes, in every session, an expansion that names another panel.
5. `show`, `hide` and `toggle` (the whole pane) leave every expansion alone.
6. A session's entry left with no tabs keeps its `expanded_panel` (`paneState.ts:434`), and a rename carries it. An entry left with nothing at all is removed, as today.
7. `paneProblem` returns `"expanded_panel"` for a session whose `expanded_panel` isn't a string that `panels.shown` holds; its `panels.filled` rule goes.

`LiveApp.tsx` changes only as far as the names: `stack.filled` → `stack.expanded`, `{ type: "fill" … }` → `{ type: "expand" … }`. `SidePane`'s `fill` prop keeps its shape until Task 3.

- [ ] **Step 1: Rust tests first**, in `layout.rs` (the helper `stack(shown, weights, filled)` loses its third parameter; the tests that pinned `filled` are rewritten to these):
  - `an_old_fill_is_dropped_and_the_rest_of_the_stack_is_kept`: a layout JSON with `"panels": {"shown": ["browser","files"], "weights": {"files": 20000}, "filled": "files"}` loads with those `shown` and `weights`; saved again, the file's text holds no `filled`.
  - `a_sessions_expanded_panel_round_trips`: `expanded_panel: Some("files")` with `shown` `["files"]` survives `save` → `load` → `repair`.
  - `repair_drops_an_expanded_panel_that_is_not_shown`: `"changes"` while `shown` is `["files"]`; `"nope"`; and `"files"` with `panels: None`. Each becomes `None`, the session's tabs stay.
  - `a_session_with_only_an_expanded_panel_is_kept`: no tabs, tree or changes; the entry survives `repair`.
  - `an_expanded_panel_of_the_wrong_type_loses_only_itself`: `"expanded_panel": 3` in the JSON; the session's tabs load.
  - `validate_refuses_an_expanded_panel_that_is_not_shown`: the error text above, for a panel not shown and for no `panels`.
- [ ] **Step 2:** `cargo test -p marshell-core -j 4 layout`. Expected: FAIL to compile (`expanded_panel` unknown).
- [ ] **Step 3:** Implement the Rust side. `cargo fmt --all`. `cargo test -p marshell-protocol -j 4` (regenerates `src/generated/`), then `cargo test -p marshell-core -j 4 layout`. Expected: PASS.
- [ ] **Step 4: TypeScript tests first.** In `panels.test.ts` and `paneState.test.ts`, rewrite the `filled` tests to `expanded`, and add one test per rule 1 to 7 above, named after the rule. Among them, with sessions `"a"` and `"b"`, `shown` `["browser","files"]`:
  - `expand` Files for `"a"`: `stackOf(next, "a").expanded === "files"`, `stackOf(next, "b").expanded === null`, `stackPanels` for `"a"` is `["files"]` and for `"b"` both.
  - then `panel` Files with session `"b"` (hides Files): `stackOf(next, "a").expanded === null` and `"expanded_panel" in next.sessions.a` is false.
  - `expand` with one panel shown turns on and off.
  - `expand` Files for `"a"`, then `close` of `"a"`'s last tab: still expanded.
  - `hide` then `show`: still expanded.
- [ ] **Step 5:** `pnpm test src/features/pane`. Expected: FAIL (`expand` is not an action; `expanded` undefined).
- [ ] **Step 6:** Implement the TypeScript side and the `LiveApp.tsx` renames.
- [ ] **Step 7:** `pnpm test src/features/pane src/features/sessions`, `pnpm build`, `pnpm check:generated`. Expected: all pass.
- [ ] **Step 8:** Commit: `feat(pane): each session keeps its own expanded panel, saved with the layout in place of the window's fill.`

### Task 3: An expanded panel covers the terminal

Worker, `effort: high`. **Files:**
- Modify: `src/features/window/paneLayout.ts`, `src/features/window/paneColumn.ts`, `src/features/window/MainWindow.tsx`
- Create: `src/features/pane/expandButton.ts`
- Modify: `src/features/pane/SidePane.tsx`, `src/features/pane/paneCommands.ts`, `src/features/sessions/LiveApp.tsx`
- Test: `paneLayout.test.ts`, `paneColumn.test.ts`, `MainWindow.test.tsx`, `src/features/pane/expandButton.test.ts` (new), `SidePane.test.tsx`, `paneCommands.test.ts`, `LiveApp.test.tsx`

**Interfaces.** Consumes `Stack.expanded` and the `expand` action (Task 2). Produces:

```ts
// paneLayout.ts: `expanded` turns "beside" into "cover". "hidden" and "fill" (no session) are unchanged.
export function paneGeometry(i: { room: number; shown: boolean; saved: number | null; terminalMin: number; alone?: boolean; expanded?: boolean }): PaneGeometry;

// paneColumn.ts
export type ColumnInputs = { /* as today */ expanded?: boolean };   // passed on to paneGeometry by paneColumn
export type Layout = { /* as today */
  /** Whether a pane that isn't expanded sits beside the terminal here: false under 720 px of room and with no session. Absent: true. */
  beside?: boolean };

// MainWindow.tsx: a new prop
paneExpanded?: boolean;

// expandButton.ts
export function expandButton(panel: PanelId, s: { on: boolean; others: boolean; beside: boolean }): { label: string; pressed: boolean; title: string; disabled: boolean };

// SidePane.tsx: these replace `fill` and `onFill`
expand: { on: boolean; others: boolean };
onExpand: () => void;

// paneCommands.ts
export function leavePaneOutcome(s: { covers: boolean; expanded: boolean; alone: boolean; shellShown: boolean }): "hide" | "focus" | "restore";
export const besideAgainText = (panel: PanelId) => `${PANEL_LABEL[panel]} is beside the terminal again.`;
```

**`expandButton`.** `label` is always `Expand <Panel>`.

| on | others | beside | pressed | disabled | title |
| --- | --- | --- | --- | --- | --- |
| any | no | no | false | true | `No other panel is shown` |
| no | (else) | (else) | false | false | `Expand <Panel>` |
| yes | yes | yes | true | false | `Show the terminal and the other panels` |
| yes | no | yes | true | false | `Show the terminal` |
| yes | yes | no | true | false | `Show the other panels` |

**`MainWindow`.**
- `geometry` gets `expanded: paneExpanded`.
- `const beside = paneGeometry({ room: spaceW, shown: true, saved: paneWidth, terminalMin: terminalMinWidth, alone: empty && paneAlone }).mode === "beside"`.
- `layoutValue` gains `inputs.expanded` and `beside` (and both in its dependencies).
- `<main>` gets `data-expanded=""` while `paneCovers && paneExpanded && beside`: the cover is the expansion's alone. Nothing else in the file changes; `paneCovers` already drives the terminals, the divider and the columns.

**`SidePane`.** The button reads `useContext(LayoutContext)?.beside ?? true`, calls `expandButton`, and draws `aria-label={label}`, `aria-pressed={pressed}`, `aria-disabled` when `disabled`, `title`. A disabled press does nothing. The icon stays.

**`leavePaneOutcome`.** `expanded` is whether `[data-expanded]` is on the window: `"restore"` when it is; else as today (`covers` → `"hide"`; `alone && !shellShown` → `"hide"`; else `"focus"`).

**`LiveApp`.**
- `MainWindow` gets `paneExpanded={stack.expanded !== null}`.
- Both `SidePane` sites: `expand={{ on: stack.expanded === panel, others: stack.shown.length >= 2 }}` and `onExpand={() => paneDispatch({ type: "expand", panel, session: owner.key })}`.
- `togglePanel`: `covers` reads `[data-pane="cover"]:not([data-expanded])`. A switch that hides the panel under the keyboard ends its expansion, so only a narrow window still covers afterwards.
- `toTerminalFromPane`: reads `expanded` from `[data-expanded]`. On `"restore"`: dispatch `{ type: "expand", panel, session }` for the owner's `stackOf(…).expanded`, `setTimeout(toOwnerTerminal, 0)` (the terminal is suspended until the change is drawn), `announce(besideAgainText(panel))` when `say`, and return `false`.
- `paneRun`'s `"paneTab"` keeps reading the plain `[data-pane="cover"]`: showing a hidden pane that comes back expanded puts the keyboard in it.

- [ ] **Step 1: Write the failing tests.**
  - `paneLayout.test.ts`: at room 1200, `expanded: true` → `{ mode: "cover", terminal: 0, pane: 1200 }`; with `shown: false` → `hidden`; with `alone: true` → `fill`; at room 600 → `cover` with and without it; `paneMotionKey("cover", …)` differs from `paneMotionKey("beside", …)` (so expanding moves for 200 ms).
  - `paneColumn.test.ts`: with `expanded: true` at width 1400 the column is the whole main room (`width === mainRoom(…).room`), left-to-right and right-to-left.
  - `MainWindow.test.tsx`: wide window, a session, `paneShown`, `paneExpanded` → the markup has `data-pane="cover"` and `data-expanded`; the same at a width whose room is under 720 → `data-pane="cover"` and no `data-expanded`; without `paneExpanded` → `data-pane="beside"`; no session with `paneAlone` and `paneExpanded` → `data-pane="fill"` and no `data-expanded`.
  - `expandButton.test.ts`: the table, all eight combinations, for `"files"` and one for `"changes"` (`Expand Changes`).
  - `SidePane.test.tsx`: the label and titles above replace the `Fill the side pane with …` ones; inside `<LayoutContext.Provider value={{ … beside: false }}>` with `expand={{ on: false, others: false }}` the button is `aria-disabled="true"` with `No other panel is shown`; with no provider and `others: false` it is enabled (`Expand Files`).
  - `paneCommands.test.ts`: `leavePaneOutcome` → `"restore"` when `expanded`, whatever the rest; the four existing cases with `expanded: false` unchanged; `besideAgainText("files") === "Files is beside the terminal again."`.
- [ ] **Step 2:** `pnpm test src/features/window src/features/pane`. Expected: FAIL (`expandButton` missing, `data-expanded` absent, `"restore"` not returned).
- [ ] **Step 3:** Implement, in the order of the Interfaces block. Update the doc comments that describe the fill (`paneLayout.ts`, `paneCommands.ts`, `SidePane.tsx`, `panels.ts`) and the source pins in `LiveApp.test.tsx`.
- [ ] **Step 4:** `pnpm test src/features/window src/features/pane src/features/sessions src/features/browser`, then `pnpm build`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(pane): a panel's Expand button takes the terminal's place too, and a key that goes to the terminal puts it back.`

### Task 4: Word wrap starts on

Worker. **Files:**
- Modify: `crates/protocol/src/api.rs` (`FilesSettings::default`, the field's doc comment), `crates/core/src/settings.rs`, `crates/core/src/store.rs`
- Modify: `src/features/files/filesSettings.tsx`, `src/features/appearance/settingsSync.ts`, `src/features/files/FilesSection.tsx`
- Test: `crates/core/src/settings.rs` and `store.rs` tests, `src/features/files/filesSettings.test.ts`, `src/features/files/FilesSection.test.tsx`, and the fixtures that spell the default (`settingsSync.test.ts`, `filesMenu.test.ts`)

**Interfaces.** Produces:
- `FilesSettings::default().word_wrap == true`; `FILES_DEFAULTS.word_wrap` and `DEFAULT_FILES.word_wrap` are `true`.
- `impl Versioned for FileDoc` in `settings.rs`: `const VERSION: u32 = 2;` and `fn migrate(from: u32, value: Value) -> anyhow::Result<Value>`: for `from == 1`, a `files.word_wrap` that is `false` becomes `true`; every other value and every other `from` passes through.
- `store::read`: when the write-back of a migrated file fails, it logs a warning (`tracing::warn!`, the path and the error, no file content) and still returns the migrated value.
- `wrapAtStart` and Alt+Z don't change.

- [ ] **Step 1: Rust tests first.**
  - `settings.rs`, `files_settings_default_to_the_reference_choices`: expects `word_wrap: true`.
  - `settings.rs`, new `a_settings_file_from_before_wrap_was_the_default_is_switched_once`: write `{"version": 1, "files": {"word_wrap": false, "tab_size": 2}}` to the scratch home's settings file; `load` gives `word_wrap == true` and `tab_size == 2`; the file on disk now says `"version": 2`; then `save` with `word_wrap = false` and `load` again gives `false`.
  - `settings.rs`, new `a_version_2_file_keeps_wrap_off` and `a_file_with_no_version_is_switched_too` (version 0 runs the same step).
  - `store.rs`, new `a_migrated_file_that_cannot_be_written_back_is_still_read`: a version-1 file of the test type (`VERSION` 2 there), with `<name>.bak` made as a folder so the backup copy fails; `read` returns `Ok(Some(…))` with the migrated value.
- [ ] **Step 2:** `cargo test -p marshell-core -j 4 settings` and `cargo test -p marshell-core -j 4 store`. Expected: FAIL (wrap is `false`; `read` returns `Err`).
- [ ] **Step 3:** Implement. Change `load`'s doc comment in `settings.rs` ("The file itself is not touched"): a file from an older version is rewritten once. `cargo fmt --all`. Run both commands again, then `cargo test -p marshell-protocol -j 4`. Expected: PASS, and `git status` shows nothing new under `src/generated/`.
- [ ] **Step 4: TypeScript tests first.** `filesSettings.test.ts`: the default is `true`. `FilesSection.test.tsx`: the help line is the sentence in Global Constraints. Fixtures that mean "the defaults" follow.
- [ ] **Step 5:** `pnpm test src/features/files src/features/appearance`. Expected: FAIL on those two.
- [ ] **Step 6:** Implement. `pnpm test src/features/files src/features/appearance src/features/editor`, `pnpm build`. Expected: PASS.
- [ ] **Step 7:** Commit: `feat(files): word wrap starts on for every file, and a settings file from before is switched once.`

### Task 5: A file opened late takes the narrow tree's place (issue #205)

Inline. **Files:**
- Modify: `src/features/files/FilesPanel.tsx`, `src/features/sessions/LiveApp.tsx`
- Test: `src/features/files/filesActions.test.ts`, `src/features/sessions/LiveApp.test.tsx`

**Interfaces.** Produces, on `FilesPanelHandle`:

```ts
/** In the narrow layout, puts the file in front of the tree: an open the person asked for, whose keyboard is still in the tree. */
showFile(): void;   // narrow ? setOverlay(false) : nothing
```

`openInPane` and the drop call `filesPanel.current?.showFile()` when the panel they open into is `"files"`, before `focusPane(panel)`. `openFromTree` doesn't (the tree's own open already takes the tree down).

- [ ] **Step 1: Tests.** `filesActions.test.ts`: `treeAfter(false, { path: "a", narrow: true }, { path: "b", narrow: true }, true) === false` (a tree the open took down stays down though the keyboard was in it). `LiveApp.test.tsx`, in the file's source-pin style: `openInPane`'s body and the drop's body each hold `showFile()` before `focusPane(`.
- [ ] **Step 2:** `pnpm test src/features/sessions/LiveApp.test.tsx`. Expected: FAIL on the pin. (The `treeAfter` case passes today; it pins the rule `showFile` relies on.)
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `pnpm test src/features/files src/features/sessions`, `pnpm build`. Expected: PASS.
- [ ] **Step 5:** Commit: `fix(files): a file dropped on the pane or picked with Browse… takes the narrow tree's place.`

### Task 6: Live check

`marshell-live-check`, from this worktree, on a new scratch folder, after the user's own dev app has been stopped with their OK (both use CDP port 9333). Findings go in `docs/research/2026-11-26-expand-panel-live-check.md`. Exit criteria:

1. Expand Files with three panels shown in a wide window: the panel takes the pane and the terminal's place in one press; the sidebar and status bar stay; the button is pressed with the tooltip "Show the terminal and the other panels". A second press puts everything back, and the terminal is usable (it takes typed text).
2. The same with one panel shown: tooltip "Show the terminal".
3. Expand Browser with a page loaded: the page fills the expanded panel and follows a window resize.
4. Esc in a tab of an expanded panel, and Ctrl+Shift+Tab: the terminal is back with the keyboard in it, and "Files is beside the terminal again." is announced.
5. Two sessions: one expanded, the other not; switching shows each as it was. Restart (`live-check.ps1 stop`, then `restart`): the expanded session is expanded again.
6. A window narrowed until the pane covers, with an expansion saved: Esc hides the pane (the old behaviour); widened again, the panel is expanded.
7. Keyboard only: Tab reaches the button, Space presses it. Forced colours (emulated): the pressed state is visible.
8. Word wrap: a new scratch home opens `main.ts` wrapped with no sideways scroll and no cut first character; Alt+Z unwraps the tab; a scratch `settings.json` at version 1 with `word_wrap: false` reads as on after one start and says version 2.
9. The blank page: with three panels and a page loaded, hide and show the pane 40 times over CDP (`Ctrl+Shift+Q`) and read `pageCovered()` after each settle: never left covered.
10. #205: with the Files panel under 480 px and the keyboard in the tree, "Browse…" from the palette opens the file in front of the tree.

### Task 7: Records

Inline, after the live check. **Files:** `docs/PLAN.md` (a "Settled while it was built" list under "Slice 6x decisions" with the ledger's rulings and the live check's note), `docs/research/2026-11-26-expand-panel-live-check.md`. Commit: `docs(plan): slice 6x's rulings and its live check.`
