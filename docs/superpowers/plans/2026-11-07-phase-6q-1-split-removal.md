# Slice 6q-1: split view removed — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The window shows one terminal. The layout menu, Split (Ctrl+Shift+D), Zoom pane (Ctrl+Shift+P), pane headers, the compact prompt lines, the row menu's "Split into a pane" and the Ctrl+click / Ctrl+Space pane toggle are gone. A saved split opens as its focused session.

**Architecture:** `layout.json` keeps its shape (research option A). The core's `repair` folds a saved split to one pane on load, and `validate_view` refuses anything but one pane, so the frontend only ever sees one. The frontend doesn't fold on its own: one rule in one place. `ViewLayout.panes[0]` is the selected session; `arrangement`, `ratios` and `zoomed` stay in the schema as dead fields, always null or empty, so an older build can still read the file. The schema cleanup is 6q-2's job. The split component stack (`SplitView`, `Pane`, `LayoutMenu`, `splitLayout`) is deleted; the single-terminal path (`KeptTerminals`, `SessionHeader`, the top-bar restart and Pause bars, `PortChips`, the Preview divider) stays.

**Tech Stack:** React 19, TypeScript, Vitest (node, `renderToStaticMarkup`), Rust (`marshell-protocol`, `marshell-core`), ts-rs.

**Spec:** `docs/PLAN.md` §9, "Slice 6q decisions (2026-11-07, first round)" decision 2, and "Slice 6q-1 decisions (2026-11-07)" 1–4. Research: `docs/research/2026-11-07-tabs-pane-options.md` §2.1 and §3.

## Global Constraints

- **What goes (6q decision 2):** the layout menu, split (Ctrl+Shift+D), zoom pane (Ctrl+Shift+P), pane headers, the compact prompt lines, the row menu's "Split into a pane", and Ctrl+click or Ctrl+Space toggling a pane.
- **What stays:** `KeptTerminals`, `SessionHeader`, the top-bar restart and Pause bars, `PortChips`, the Preview and its divider, focus mode, the rail, the shell panel, "Select no terminal" (Ctrl+Shift+Backspace) and the navigation cursor.
- **The fold rule (6q-1 decision 4):** after panes whose tab is gone are emptied, keep the zoomed pane's tab if it has one, else the active pane's, else the first non-empty pane's, else none. Then `panes = [that]`, `active = 0`, `zoomed = None`, `arrangement = None`, `ratios = []`.
- **Rows (6q-1 decision 1):** Ctrl+click (Cmd+click on macOS) selects the row like a plain click. Ctrl+Space with no other modifier, outside the rename box, selects it like Enter.
- **Ctrl+Shift+Tab (6q-1 decision 2):** the command keeps its id `nav-next-pane` and its binding, and its label becomes exactly "Select the terminal". With one terminal, it selects that terminal and starts the cursor.
- **Freed keys:** Ctrl+Shift+D and Ctrl+Shift+P (⌘D and ⌘P on macOS) run no command.
- **Copy.** Sentence case, no "!", no "successfully".
  - Shell panel button: aria-label and title `Open ${shellName} as a session`.
  - Shortcut sheet arrows row: "Move the cursor to the terminal, a session or a control".
- **Styles.** Tokens only, with no hex. Follow the good-css skill. `:focus-visible` with `outline`; `:hover` only inside `@media (hover: hover) and (pointer: fine)`; forced-colors rules use system colours only; no boxed outlines.
- **Keyboard and forced colours.** Every remaining UI works with the keyboard alone and in forced colours.
- **Dependencies and builds.**
  - No new dependencies.
  - Rust: `cargo test -p <crate> -j 4`, and `cargo fmt --all` before each Rust commit.
  - After the protocol change, `pnpm check:generated`. Never hand-edit `src/generated/`.
- **Don't touch** `docs/BRIEF.md` (6q-1 decision 3), the old §9 decision lists' wording (Task 6 only adds "replaced" notes), or `docs/IDEAS.md`.

## Review Focus

1. **A saved split whose focused pane is empty or names a closed tab** (a file written before 6q-1). Expected: the window opens on the first open session in pane order. With none open, it shows one empty pane that takes the store's selection. Tests: Task 1 `repair_folds_past_an_empty_or_closed_focused_pane`, `repair_folds_a_split_with_no_open_tab_to_one_empty_pane`; Task 5 `an empty pane takes the store's selection`.
2. **A saved split with a zoomed pane.** Expected: it opens on the zoomed session, not the active one. Test: Task 1 `repair_folds_a_split_to_the_zoomed_session`.
3. **`recent_commands` saved by 6p holding `split`, `unsplit`, `layout` or `zoom-pane`.** Expected: Recent leaves them out and never shows a row that runs nothing. Test: Task 3 `recent_drops_the_removed_split_ids`.
4. **Ctrl+Space in the rename box, with Alt or Shift held, or Cmd+Space on macOS.** Expected: the rename box keeps it, the other chords do nothing to the row, and Cmd+Space stays Spotlight's. Test: Task 2 `rowPick.test.ts` cases.
5. **Ctrl+Shift+Tab inside a terminal, then Ctrl+Shift+Left.** Expected: the cursor starts on the terminal and moves to that session's row; with no session on screen it announces "No terminal on screen". Tests: Task 3 `nav Tab starts the cursor from a terminal with one slot`, `the freed keys run no command`.

---

### Task 1: The core folds a saved split to its focused session

**Files:**
- Modify: `crates/core/src/layout.rs` (`repair`, `validate_view`, constants, module tests)
- Modify: `crates/protocol/src/api.rs:418-449` (doc comments only)
- Modify: `crates/core/tests/layout_api.rs` (`put_view_rejects_five_panes`, and any test that PUTs several panes)
- Modify: `crates/core/tests/layout_restore.rs` (new test)
- Regenerated: `src/generated/ViewLayout.ts`

**Interfaces:**
- Produces: `GET /v1/layout` always returns `view.panes` of length 1, with `active == 0`, `zoomed == null`, `arrangement == null` and `ratios == []`. `PUT /v1/layout/view` answers 400 for anything else, and the message names the field.

- [ ] **Step 1: Write the failing tests** in `layout.rs`'s test module. Tabs `a`, `b`, `c` exist unless a test says otherwise.
  - `repair_folds_a_split_to_the_zoomed_session`: panes `[a, b, c]`, active 1, zoomed `Some(2)`, arrangement `Some("columns")`, ratios `[0.4, 0.5]`. Expect panes `[Some(c)]`, active 0, zoomed `None`, arrangement `None`, ratios empty.
  - `repair_folds_a_split_to_the_active_session`: panes `[a, b, c]`, active 1, zoomed `None`. Expect `[Some(b)]`.
  - `repair_folds_past_an_empty_or_closed_focused_pane`: only tabs `b` and `c` exist. Panes `[a, None, c]`, active 0, zoomed `Some(1)`. Expect `[Some(c)]`.
  - `repair_folds_a_split_with_no_open_tab_to_one_empty_pane`: no tabs. Panes `[x, y]`. Expect `[None]`, active 0.
  - `validate_view_refuses_a_split_and_its_settings`: a default view with panes `[None, None]` gives an Err containing `"panes"`. `zoomed: Some(0)` gives `"zoomed"`, `arrangement: Some("single")` gives `"arrangement"` and `ratios: vec![0.5]` gives `"ratios"`. The default view is Ok.
  - Update `repair_clamps_sidebar_width_and_drops_unknown_panes`, `repair_fixes_the_rest_of_the_view` and `validate_view_names_the_field` to the one-pane rules.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p marshell-core -j 4 --lib layout::`
Expected: FAIL in the new `repair_folds_*` and `validate_view_refuses_*` tests.

- [ ] **Step 3: Implement.**
  - In `repair`, after emptying the panes whose tab is gone, apply the fold rule from Global Constraints with a private `fn fold_to_one_pane(view: &mut ViewLayout)`. Use `.get(z)` for the zoomed and active indexes, so one out of range counts as no pane.
  - `validate_view`:
    - `panes.len() != 1` gives `"panes must hold exactly 1 entry, not {n}"`;
    - `zoomed.is_some()` gives `"zoomed must be null"`;
    - `arrangement.is_some()` gives `"arrangement must be null"`;
    - non-empty `ratios` gives `"ratios must be empty"`;
    - the `active` check stays.
  - Delete `ARRANGEMENTS`, `MAX_PANES`, `RATIO` and `MAX_RATIOS`.

- [ ] **Step 4: The API and restore tests.**
  - Rename `put_view_rejects_five_panes` to `put_view_rejects_two_panes`: it PUTs `[null, null]` and expects 400 with a body that mentions `panes`.
  - Fix any other `layout_api.rs` test that PUTs several panes.
  - Add `a_saved_split_reopens_on_its_focused_session` to `layout_restore.rs`:
    1. Write a version-1 `layout.json` into the scratch home. It has three saved tabs and `view.panes` holding their three ids, with `active: 2`, `zoomed: null`, `arrangement: "columns"` and `ratios: [0.5]`. Copy the tab shape from the existing restore tests.
    2. Build the app with `state_in`.
    3. Expect `GET /v1/layout` to return `view.panes == [<pane 2's id, as restored>]`, `zoomed == null`, `arrangement == null` and `ratios == []`.

- [ ] **Step 5: Protocol doc comments** in `api.rs` (the `Default` stays as it is):
  - struct: "How the window was laid out: the selected tab and the sidebar."
  - `panes`: "Always one entry: the selected tab, or None. A file from before slice 6q-1 can hold up to four; the core keeps only the focused one."
  - `arrangement`, `ratios` and `zoomed`: "Unused since slice 6q-1 removed split view: always None (or empty). Kept so an older build can still read the file."

- [ ] **Step 6: Run everything that covers it**

Run: `cargo test -p marshell-protocol -j 4`, then `cargo test -p marshell-core -j 4 layout`, then `pnpm check:generated`.
Expected: all pass; `src/generated/ViewLayout.ts` changed only in its comments.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add crates/core/src/layout.rs crates/core/tests/layout_api.rs crates/core/tests/layout_restore.rs crates/protocol/src/api.rs src/generated/ViewLayout.ts
git commit -m "feat(layout): a saved split opens as its focused session, and the view takes one pane only"
```

### Task 2: Rows: Ctrl+click and Ctrl+Space select, and the row menu loses "Split into a pane"

**Files:**
- Modify: `src/features/sidebar/rowPick.ts` (+ `rowPick.test.ts`)
- Modify: `src/features/sidebar/SessionRow.tsx:119,132-145`, `src/features/window/Rail.tsx:62-85`, `src/features/window/Sidebar.tsx:243` (the `onToggle` prop chain), `src/features/window/MainWindow.tsx` (`onToggle` prop only)
- Modify: `src/features/sidebar/sessionMenu.ts:15-16,39` (+ `rowMenu.test.ts`), `src/features/sessions/sessionMenuRun.ts:42-47,79` (+ `sessionMenuRun.test.ts`)
- Modify: `src/features/sessions/LiveApp.tsx`: `togglePane` (around 1019-1035), `onToggle={togglePane}` (1791), the row menu's `case "split"` (1130), and `menuFacts(...)`'s `panes`/`shown`/`fit` (1065)

**Interfaces:**
- Produces: `rowPick.ts` exports only `selectsByCtrlSpace(e: Mods & { key: string; target: object | null }): boolean`, which is the old `togglesPaneKey` renamed with the same rules. `togglesPane` is deleted. `RowMenuFacts` has no `inShownPane` and no `paneAvailable`. `menuFacts`'s context has no `panes`, `shown` or `fit`. The row menu has no `"split"` item id. `SessionRow`, `Rail`, `Sidebar` and `MainWindow` have no `onToggle` prop.

- [ ] **Step 1: Write the failing tests.**
  - `rowPick.test.ts`, "selectsByCtrlSpace":
    - Ctrl+Space gives true;
    - Ctrl+Alt+Space, Ctrl+Shift+Space, Meta+Space and plain Space give false;
    - Ctrl+Space whose target is inside the rename box gives false.
  - `rowMenu.test.ts`: "no row menu offers Split into a pane". For every fact combination the file already builds, no item has id `"split"`.
  - `sessionMenuRun.test.ts`: delete the `inShownPane` and `paneAvailable` tests, and `focusAfterMenu("split", …)`.
- [ ] **Step 2: Run** `pnpm test src/features/sidebar src/features/sessions/sessionMenuRun.test.ts`. Expected: FAIL (there is no `selectsByCtrlSpace` export yet).
- [ ] **Step 3: Implement.**
  - `SessionRow` and `Rail`: `onClick` is `onSelect`.
  - The keydown branch that called `onToggle` now calls `onSelect` when `selectsByCtrlSpace(e)`, after `preventDefault()`. In `SessionRow` it stays after the Peek and row-menu branches, so plain Space still opens Peek.
  - Remove the `onToggle` props and `LiveApp`'s `togglePane`.
  - Remove the `"split"` item and its run case, and `menuFacts`'s split inputs.
- [ ] **Step 4: Run** `pnpm test src/features/sidebar src/features/window src/features/sessions`, then `pnpm build`. Expected: PASS, with tsc clean.
- [ ] **Step 5: Commit** `feat(sidebar): Ctrl+click and Ctrl+Space select a row, and the row menu no longer splits`.

### Task 3: Keys and commands: Split, Zoom pane and the layout commands go, and Ctrl+Shift+Tab becomes "Select the terminal"

**Files:**
- Modify: `src/lib/keymap.ts:79,87-90` (+ `keymap.test.ts`)
- Modify: `src/lib/appShortcuts.ts:14-20,76-81` (+ `appShortcuts.test.ts`)
- Modify: `src/features/palette/runnable.ts:8,25` (+ `runnable.test.ts`), `src/features/palette/paletteList.test.ts`
- Modify: `src/features/sessions/sessionActions.ts:243-246`
- Modify: `src/features/shortcuts/ShortcutSheet.tsx:26`
- Modify: `src/lib/navigation/machine.ts` (doc comment only), `src/lib/navigation/machine.test.ts`, `src/lib/navigation/keys.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx`:
  - `NO_PANE_ROOM` (128);
  - `split:` in the palette context (683);
  - `paneCommand`'s `split`, `zoom` and `nextPane` cases (1263-1289);
  - the `unsplit` id (1412).
- Modify: `src/features/sessions/LivePalette.test.tsx` and the other tests that name the removed ids.

**Interfaces:**
- Produces:
  - `COMMANDS` has no `split`, `unsplit`, `layout` or `zoom-pane`.
  - `nav-next-pane` keeps its binding (`{ code: "Tab", nav: true }`), with the label "Select the terminal".
  - `NAV_COMMAND_IDS` is unchanged.
  - `AppShortcut` has no `split`, `zoom` or `nextPane` kind.
  - The palette's runnable context has no `split` field.
  - The nav `"tab"` event stays.

- [ ] **Step 1: Write the failing tests.**
  - `keymap.test.ts`: "the split commands are gone". No command id is in `["split", "unsplit", "layout", "zoom-pane"]`, and `commandById("nav-next-pane")!.label === "Select the terminal"`.
  - `keymap.test.ts` (or `bindings.test.ts`, wherever `findCommand` is tested): "the freed keys run no command". `findCommand` returns undefined for Ctrl+Shift+KeyD and Ctrl+Shift+KeyP on `"windows"`, and for Meta+KeyD and Meta+KeyP on `"mac"`.
  - `paletteList.test.ts`: `recent_drops_the_removed_split_ids`. With `recent_commands: ["split", "zoom-pane", "layout", "unsplit"]` plus one id that is still offered, Recent lists only that id.
  - `keys.test.ts` and `machine.test.ts`: "nav Tab starts the cursor from a terminal with one slot".
    - Context: panes `[3]`, a layout with the pane and row 3.
    - Ctrl+Shift+Tab in a terminal selects pane 0.
    - A following Ctrl+Shift+Left, still held, moves the cursor to row 3.
    - With panes `[null]`, Tab announces "No terminal on screen".
  - Delete the tests that cycle two or more panes with Tab.
- [ ] **Step 2: Run** `pnpm test src/lib src/features/palette`. Expected: FAIL on the new cases.
- [ ] **Step 3: Implement.**
  - Remove the four commands, their `AppShortcut` kinds, the `sessionActions` cases, the `runnable` rule and the `LiveApp` handlers.
  - Relabel `nav-next-pane`.
  - In `ShortcutSheet`, the arrows row stays keyed on `runs("nav-next-pane")`, with the new label from Global Constraints.
  - Reword the doc comment at the top of `machine.ts`: Tab with the navigation modifier selects the terminal on screen and starts the cursor.
- [ ] **Step 4: Run** `pnpm test src/lib src/features/palette src/features/sessions src/features/shortcuts`, then `pnpm build`. Expected: PASS, with tsc clean.
- [ ] **Step 5: Commit** `feat(keys): Split, Zoom pane and the layout commands are removed, and Ctrl+Shift+Tab selects the terminal`.

### Task 4: The window draws one terminal, and the split stack is deleted

**Files:**
- Modify: `src/features/window/MainWindow.tsx`:
  - `WindowLayout` loses `"split"`;
  - the props at 85-112 and their uses;
  - the layout menu and its button (378-390, about 500-512);
  - the split ordering and arithmetic (318-372);
  - `SplitView` rendering (579-580);
  - the divider class (594).
- Delete: `src/features/window/SplitView.tsx`, `SplitView.css`, `SplitView.test.tsx`, `SplitView.css.test.ts`, `Pane.tsx` (and its tests, if any), `LayoutMenu.tsx`, `LayoutMenu.css`, `LayoutMenu.test.tsx`, `LayoutMenu.css.test.ts`
- Modify: `src/features/window/MainWindow.css` (the divider rules move in), `src/styles/noBoxedOutlines.test.ts`, `src/styles/forcedColorsEdges.test.ts`, and the other style tests that name `SplitView.css` or `LayoutMenu.css`
- Modify: `src/features/composer/Composer.tsx` (the `compact` variant goes), `Composer.css`, `Composer.stories.tsx`, `Composer.test.tsx`, `composerHeight.test.ts`, and the `COMPOSER.compact` height constant
- Modify: `src/features/window/MainWindow.test.tsx`, `MainWindow.stories.tsx`, `MainWindow.stories.test.tsx`, `NavCursor.stories.tsx`, `NavCursor.stories.test.tsx`, `manualCli.test.tsx`, and any other test or story that passes a removed prop
- Modify: `src/design/review/renderReview.tsx:52,56` (prose), `scripts/build-review.mjs:42,261`
- Modify: `src/features/sessions/LiveApp.tsx`, only the `<MainWindow …>` props block (1686-1710): drop `layout`, `paneIds`, `activePane`, `zoomedPane`, `arrangement`, `ratios`, `onFit`, `onLayout`, `onPane`, `onVisible` and `composerValues`.

**Interfaces:**
- Consumes: Task 2 (no `onToggle`).
- Produces:
  - `MainWindowProps` has no `splitCount`, `arrangement`, `ratios`, `activePane`, `zoomedPane`, `emptySlot`, `layoutMenu`, `paneIds`, `onLayout`, `onPane`, `onLayoutMenu`, `onVisible`, `onFit` or `composerValues`.
  - `WindowLayout = "default" | "focus" | "rail"`.
  - `Composer` has no `variant` prop.
  - The divider is `<div className="preview__divider" role="separator" aria-orientation="vertical" …>`, with every aria value and key handler unchanged.
  - `splitLayout.ts` still exists: Task 5 deletes it.

- [ ] **Step 1: Write the failing tests** in `MainWindow.test.tsx`.
  - "the top bar has no layout button": the markup of a window with three rows has none of the layout button's label (take it from `LayoutMenu.tsx` before deleting the file) and no `class="split`.
  - "the preview divider keeps its role and value": with a preview open beside the session, the markup has `class="preview__divider"`, `role="separator"`, `aria-label="Resize the preview"` and `aria-valuenow="50"`.
- [ ] **Step 2: Move the divider's CSS.** Move `SplitView.css`'s `.split__handle` base, `::before`, vertical orientation, `:focus-visible`, `:active`, hover and forced-colors rules (8-45 and 311-) into `MainWindow.css` as `.preview__divider`, merging the vertical-orientation rules into the base. The horizontal rules aren't moved. Point `noBoxedOutlines.test.ts` and `forcedColorsEdges.test.ts` at `MainWindow.css` for these rules.
- [ ] **Step 3: Strip `MainWindow`** to the single path.
  - `KeptTerminals` gets `selectedId={selected.id}`.
  - `data-nav-pane` is `0` whenever a session is shown and no main view covers it.
  - The top bar is `headerBar ?? paneBar?.node`.
  - Delete the files listed above and the compact `Composer` variant.
  - Fix the stories, story tests and review page. In `renderReview.tsx`, drop the sentences about split view. In `build-review.mjs`, drop the `SplitView.css` path and the split checklist line.
- [ ] **Step 4: Run** `pnpm test`, then `pnpm build`. Expected: every test passes and tsc is clean. Quote the counts.
- [ ] **Step 5: Commit** `feat(window): the window draws one terminal, and the split view, pane and layout menu components are deleted`.

### Task 5: The view state holds one session, and LiveApp's split wiring goes

**Files:**
- Modify: `src/features/window/viewState.ts` (+ `viewState.test.ts`), `src/features/window/useViewState.ts`
- Delete: `src/features/window/splitLayout.ts`, `splitLayout.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx`:
  - the `shownCount` and fit state and their refs (193-240);
  - `focusIdx` and `visibleIds` (206-217);
  - `showInPane` (999-1018);
  - `openInPane` (986-998), renamed `openAsSession`;
  - the adopt and hand-off effects (1036-1053);
  - `askRestart` and `pauseSession` (1087-1116);
  - the navigation block (1180-1225);
  - `onScreenIds` (1678);
  - the imports (89-93).
- Modify: `src/features/sessions/useNavigation.ts` (`navPanesOf` goes), `src/features/window/ShellPanel.tsx:45,65` (+ its test), `Sidebar.tsx`'s `SidebarShell`, `MainWindow.tsx`'s `ShellOptions` (`onOpenInPane` becomes `onOpenAsSession`)
- Modify: the comments only in `src/features/sessions/handOff.ts:5` and `src/lib/api/sessions.ts:84` ("Open as a session")

**Interfaces:**
- Consumes: Task 1 (the core sends one pane), Tasks 2–4.
- Produces:
  - `ViewAction` is exactly `select {tabId}`, `sessions {openIds}`, `sidebar`, `sidebarWidth`, `resetSidebarWidth`, `toggleFocus`, `toggleShell`, `toggleGrouped` and `ranCommand`.
  - `viewState.ts` no longer exports `ratiosOf`, `toRatioList`, `arrangementFor`, `checkedView`, `focusedIndex`, `roomForPane`, `adoptTarget`, `visibleSessions`, `paneToggle` or `PaneToggle`.
  - `fillsEmptyPane(panes, selectedId)` stays.
  - `DEFAULT_VIEW` is unchanged.

- [ ] **Step 1: Write the failing tests** in `viewState.test.ts`:
  - "select puts the session in the one pane": `viewReducer({ ...DEFAULT_VIEW, panes: ["a"] }, { type: "select", tabId: "b" })` gives panes `["b"]` and active 0;
  - "a closed session leaves the pane empty": panes `["a"]` and `sessions { openIds: ["b"] }` give panes `[null]`;
  - "an empty pane takes the store's selection": `fillsEmptyPane([null], "a")` is true, `fillsEmptyPane([null], undefined)` is false, and `fillsEmptyPane(["b"], "a")` is false.

  Delete the tests for the removed actions and helpers.
- [ ] **Step 2: Run** `pnpm test src/features/window/viewState.test.ts`. Expected: FAIL (`select` still takes the visible-pane path).
- [ ] **Step 3: Implement the reducer.**
  - `select` gives `{ ...s, panes: [a.tabId], active: 0 }`.
  - `sessions` empties `panes[0]` when its tab isn't open.
  - Delete the other split actions and helpers, and `splitLayout.ts`.
  - `useViewState` loads the view as the core sent it: `checkedView` goes.
- [ ] **Step 4: Rewire `LiveApp`.**
  - `focusedId = view.panes[0] ?? selectedId`.
  - On-screen ids are `[focusedId]` unless a main view covers the terminal.
  - Where `showInPane(id)` was used (the adopted shell, `askRestart`, `pauseSession`), select the session the way a row click does: the existing `pick`.
  - Navigation:
    - `navPanes = [numberOf(focusedId)]`;
    - `select-pane` focuses the terminal;
    - `bring-into-view` picks the session and focuses its terminal.
  - Shell panel: `openInPane` becomes `openAsSession`, and the button's copy comes from Global Constraints.
- [ ] **Step 5: Run** `pnpm test`, then `pnpm build`. Expected: every test passes and tsc is clean. Then `grep -rn "splitLayout\|SplitView\|LayoutMenu\|zoomedPane\|paneToggle\|adoptTarget\|showInPane" src` prints nothing.
- [ ] **Step 6: Commit** `feat(window): the view holds one session, and the shell panel opens its shell as a session`.

### Task 6: PLAN text, README and the screen-reader script

Done inline (the text is binding, so it needs judgement).

**Files:** `docs/PLAN.md`, `README.md:11`, `docs/a11y/screen-reader-script.md:25,28`

- [ ] **Step 1:** Run `grep -n -i "split\|pane\|zoom\|Ctrl+Space\|Ctrl+click" docs/PLAN.md` and rewrite the binding UX and architecture lines so they describe one terminal. Research §2.1 lists them: 91, 122-142, 165, 203, 315, 410-429, 553-559, 676-682, 700-718, 771, 789, 866-881, 1040.
  - The shortcut table loses Split and Zoom pane, and Ctrl+Shift+Tab reads "Select the terminal".
  - The §9 decision lists of earlier slices keep their wording. Each split-specific decision gets "(Replaced by slice 6q decision 2.)".
- [ ] **Step 2:** In `README.md:11`, drop "Split view takes up to four panes"; keep focus mode. In the screen-reader script, delete rows A5 and A8 and leave the other IDs as they are.
- [ ] **Step 3:** Run `grep -n -i "split view\|split (ctrl\|zoom pane" docs/PLAN.md README.md docs/a11y`. Expected: hits only in §9's historical lists and the 6q decisions.
- [ ] **Step 4: Commit** `docs(plan): the window shows one terminal; split view is removed from the layout, keys and row text`.

### Task 7: The live check and its note

**Files:** Create `docs/research/2026-11-07-split-removal-live-check.md`

- [ ] **Step 1:** Run the `marshell-live-check` skill (`pnpm tauri dev` against scratch homes). Before the first start, write a scratch `MARSHELL_HOME/layout.json` holding a 3-pane split with `active: 2` and three plain shell tabs. Check:
  - the app opens on pane 2's session, and the other two are rows;
  - after any view change, the file on disk holds `"panes":["<id>"]`;
  - no layout button shows, and Ctrl+Shift+D and Ctrl+Shift+P run nothing;
  - Ctrl+click and Ctrl+Space on a row select it;
  - the row menu has no "Split into a pane", and the palette has no Split, Zoom or layout rows;
  - Ctrl+Shift+Tab in the terminal, then Ctrl+Shift+Left, puts the cursor on the session's row;
  - the Preview divider drags, the arrow keys move it, and it shows its focus ring;
  - the restart and Pause bars show in the top bar for a session picked from the row menu;
  - the shell panel's "Open PowerShell as a session" selects the new session;
  - focus mode and the rail still work.
- [ ] **Step 2:** Write the note: setup, each check with its result, and anything found. A fix found here gets its own commit, with a test.
- [ ] **Step 3: Commit** `docs(window): the 6q-1 live check note`.
