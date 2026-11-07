# Slice 6p: the bottom-left menu and a cleaner palette — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A menu button at the bottom left opens Settings (a submenu of sections), History, Doctor and Departures. The command palette loses those rows from its empty box, starts with recent commands, folds the five Mode rows into "Change mode…", regroups into Session, Terminal, Window and App, and leaves out what can't run now.

**Architecture:** Pure helpers carry every rule (`paletteList.ts`, `runnable.ts`, `appMenu.ts`, `menuKeys.ts`), and the components only render them. Recent commands are saved with the window's view (`ViewLayout.recent_commands`), so the core validates and repairs them like the rest of `layout.json`. The menu is its own component, `AppMenu`. It reuses the context menu's key rules, extended with a submenu, and its look.

**Tech Stack:** React 19, TypeScript, Vitest (node, `renderToStaticMarkup`), Rust (`marshell-protocol`, `marshell-core`), ts-rs.

**Spec:** `docs/PLAN.md` §9, "Slice 6p decisions (2026-11-07)" 1–10. Also binding: PLAN's "Keyboard navigation and shortcuts" (physical keys, the reserved list, no punctuation) and the row-menu key rules (PLAN line 869: arrows wrap, Home/End, Enter or Space runs, typeahead, Esc closes and returns focus, Tab closes without moving it).

## Global Constraints

- **Moved items (decision 2).** `MOVED_COMMANDS = ["settings", "appearance", "agents", "profiles", "notifications", "history-settings", "diagnostics", "history", "doctor", "departures"]`.
  - With an empty box the palette doesn't list them.
  - Typing finds them, for example "appear" finds "Settings: Appearance…" (decision 3).
- **Groups (decision 4.3).** In this order: `Session`, `Terminal`, `Window`, `App`.
- **Menu shortcut (decision 7).** Ctrl+Shift+M on Windows and Linux, and ⌘E on macOS (`binding: { code: "KeyM", mac: { code: "KeyE" } }`). Ctrl+Shift+Alt+F and the `global-search` command go (decision 8).
- **Copy.** Sentence case, no "!", no "successfully".
  - Menu: the button and the menu are both named "Menu". The items are "Settings", "History", "Doctor" and "Departures".
  - Settings submenu: named "Settings". The items are "Appearance", "Agents", "Shell profiles", "Notifications", "History settings" and "Diagnostics".
  - Palette rows: "Open menu" and "Change mode…".
  - Palette groups: "Recent", and "Modes" for the second list. The second list's prompt is "Type a mode".
- **Styles.** Colours come from the tokens only, with no hex. Follow the good-css skill:
  - `:focus-visible` with `outline`, and `:hover` only inside `@media (hover: hover) and (pointer: fine)`;
  - no boxed outlines (`noBoxedOutlines.test.ts`);
  - forced-colors rules use system colours only.
- **Keyboard and forced colours.** Every UI change works with the keyboard alone and in forced colours.
- **Status.** Don't guess a status. An unknown Doctor count shows no number.
- **Dependencies and builds.**
  - No new dependencies.
  - Rust: `cargo test -p <crate> -j 4`, and `cargo fmt --all` before each Rust commit.
  - After the protocol change, run `pnpm check:generated`. Never hand-edit `src/generated/`.
- **Split view.** It is removed in 6q-1. Add nothing to it, and only leave out its rows when they can't run.

## Review Focus

1. **A saved `recent_commands` that is too long, has duplicates, unknown ids or bad characters** (a hand edit, another build). Expected: the core repairs it on load and refuses it on PUT, and the palette drops ids it doesn't know. It never shows a row that runs nothing. Tests: Task 2 `repair_trims_recent_commands`, `validate_refuses_a_bad_recent_command`; Task 3 `recent_drops_unknown_and_moved_ids`.
2. **A recent command that can't run now** ("Close session" with nothing focused, a mode the agent doesn't offer). Expected: it is left out of Recent, as it is out of its group. Test: Task 3 `recent_lists_only_commands_that_are_offered`, fed from Task 4's filtered list.
3. **Ctrl+Shift+M with no menu button on screen** (focus mode). Expected:
   - the menu opens inside the window at the bottom left;
   - Esc returns focus to what had it (usually the terminal);
   - Ctrl+Shift+M while it is open closes it.

   Tests: Task 5 `a_menu_with_no_anchor_opens_at_the_bottom_left`; Task 6 `the_menu_key_toggles_and_remembers_where_focus_was`.
4. **Esc in the mode list.** Expected: Esc goes back to the commands, and a second Esc closes the palette. Test: Task 3 `escape_goes_back_from_the_mode_list_then_closes`.
5. **Enter after the empty box was reordered** (Recent first, then the groups). Expected: Enter runs the row shown as active, in every group. Test: Task 3 `enter_runs_the_row_shown_active_in_every_group`.

---

### Task 1: The four groups, the Settings labels, and Ctrl+Shift+Alt+F removed

**Files:**
- Modify: `src/lib/keymap.ts`, `src/features/palette/Palette.tsx:32`, `src/lib/appShortcuts.ts`, `src/features/sessions/LiveApp.tsx` (the `globalSearch` case near line 1239, the text-box exemption at line 1350, the palette deferral list at line 1415), `src/features/history/HistoryView.tsx:216`
- Test: `src/lib/keymap.test.ts`, `src/features/palette/Palette.test.tsx`, `src/features/modes/sessionModes.test.ts`, `src/lib/appShortcuts.test.ts`, `src/features/sessions/LivePalette.test.tsx`, `src/features/shortcuts/ShortcutSheet.test.tsx`

**Interfaces:**
- Produces:
  - `Command["group"]: "Session" | "Terminal" | "Window" | "App"`;
  - `GROUPS` in `Palette.tsx`, in that order;
  - `export const MOVED_COMMANDS: readonly string[]` in `keymap.ts`, with the value from Global Constraints.
  - Removes the `global-search` command and the `{ kind: "globalSearch" }` shortcut.

- [ ] **Step 1: Write the failing tests** in `keymap.test.ts`:

```ts
it("puts every command in one of the four groups, each group contiguous and in order", () => {
  const order = ["Session", "Terminal", "Window", "App"];
  const seen = COMMANDS.map((c) => order.indexOf(c.group));
  expect(seen.every((g) => g >= 0)).toBe(true);
  expect([...seen].sort((a, b) => a - b)).toEqual(seen);
});
it("names the Settings pages so typing a section finds them", () => {
  expect(["appearance", "agents", "profiles", "notifications", "history-settings", "diagnostics"].map((id) => commandById(id).label)).toEqual([
    "Settings: Appearance…", "Settings: Agents…", "Settings: Shell profiles…", "Settings: Notifications…", "Settings: History…", "Settings: Diagnostics…",
  ]);
});
it("has no Search all sessions command", () => {
  expect(COMMANDS.some((c) => c.id === "global-search")).toBe(false);
});
```

Change the existing `win("global-search")` assertion (line 59) to `expect(COMMANDS.some((c) => c.binding?.code === "KeyF" && c.binding.alt)).toBe(false)`. In `Palette.test.tsx:23` use `["Session", "Terminal", "Window", "App"]`. In `sessionModes.test.ts:18` use `"Session"`. In `LivePalette.test.tsx:16` and `ShortcutSheet.test.tsx:54`, drop `"global-search"` from the lists.

- [ ] **Step 2: Run them.** `pnpm test src/lib/keymap.test.ts src/features/palette/Palette.test.tsx` should FAIL on the group names and labels.
- [ ] **Step 3: Make the changes in `keymap.ts`.**
  - The group of each command:
    - Session: launcher, focus-prompt, focus-session-1…9, new-session-admin, go-to-session, next-waiting, close, reopen, rename, resume, review-task, clean-up-task, move-up, move-down, pause, continue, the five mode-* rows, announce-status, copy-recording-path, run-script, stop-script, edit-references.
    - Terminal: find, bookmark, prompt-prev, prompt-next, capture, nav-next-pane, nav-none.
    - Window: split, unsplit, layout, zoom-pane, focus-mode, preview, open-file, sidebar, group-by-project, shell-panel.
    - App: palette, cheatsheet, pause-notifications, settings, the six Settings pages, history, doctor, departures.
  - Reorder `COMMANDS` so each group is contiguous, in group order.
  - Relabel the six Settings pages as in the test. "Settings…", "History", "Open Doctor" and "Open Departures board" keep their labels.
  - Add `MOVED_COMMANDS`.
  - Delete `global-search`.

  **In the other files:**
  - `appShortcuts.ts`: drop the `globalSearch` kind and its case.
  - `LiveApp.tsx`: drop the `globalSearch` case and its two mentions at lines 1350 and 1415. Update the comment at line 1349 to name only Ctrl+Shift+Y.
  - `HistoryView.tsx:216`: the hint becomes the plain `"Search conversations"`. Inside History, Ctrl+Shift+Y now leaves the view, so naming it there would mislead.
- [ ] **Step 4: Run** `pnpm test src/lib src/features/palette src/features/modes src/features/sessions/LivePalette.test.tsx src/features/shortcuts src/features/history`. Expected: PASS.
- [ ] **Step 5: Commit.** `refactor(palette): commands sit in the Session, Terminal, Window and App groups, the Settings pages read "Settings: …", and Ctrl+Shift+Alt+F is gone`

### Task 2: Recent commands saved with the window's view

**Files:**
- Modify: `crates/protocol/src/api.rs:421-450` (`ViewLayout` and its `Default`), `crates/core/src/layout.rs` (`repair`, `validate_view`, and the struct literals in its tests), `src/features/window/viewState.ts` (the default view at lines 30–41, `ViewAction`, `viewReducer`), and `src/generated/` (regenerated)
- Test: `crates/core/src/layout.rs` tests, `src/features/window/viewState.test.ts`

**Interfaces:**
- Produces:
  - `ViewLayout.recent_commands: string[]`: newest first, at most 5 ids;
  - `ViewAction` gains `{ type: "ranCommand"; id: string }`;
  - `export const RECENT_LIMIT = 5` and `export function recentAfter(list: readonly string[], id: string): string[]` in `viewState.ts`. The id moves to the front, its duplicates go, and the list is cut to `RECENT_LIMIT`.

- [ ] **Step 1: Write the failing Rust tests** in `layout.rs`:

```rust
#[test]
fn repair_trims_recent_commands() {
    let mut doc = LayoutDoc::default();
    doc.view.recent_commands = ["close", "close", "Bad Id", "", "find", "rename", "history", "focus-mode", "split", "preview"].map(String::from).to_vec();
    let fixed = repair(doc);
    assert_eq!(fixed.view.recent_commands, ["close", "find", "rename", "history", "focus-mode"]);
}
#[test]
fn validate_refuses_a_bad_recent_command() {
    assert!(bad(|v| v.recent_commands = vec!["a b".into()]).contains("recent_commands"));
    assert!(bad(|v| v.recent_commands = vec!["x".into(); 6]).contains("recent_commands"));
}
```

An id is valid when it has 1 to 64 characters, all from `a-z`, `0-9` and `-`. `repair` keeps the first occurrence of each valid id, in order, up to 5. `validate_view` errors with a message naming `recent_commands`: "recent_commands must hold at most 5 command ids of a-z, 0-9 and -".

- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 layout`. Expected: it FAILS to compile (no field).
- [ ] **Step 3: Add the field** `#[serde(default)] pub recent_commands: Vec<String>`, with the doc comment "The palette's recent commands, newest first: at most 5 command ids." Set `Vec::new()` in `Default`, and implement the two rules. Add `recent_commands: Vec::new()` (or `..Default::default()`) wherever a `ViewLayout` literal fails to compile.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 layout && cargo test -p marshell-protocol -j 4 && pnpm check:generated`. Expected: PASS, and `src/generated/ViewLayout.ts` has `recent_commands: Array<string>`.
- [ ] **Step 5: Write the failing frontend test** in `viewState.test.ts`:

```ts
it("keeps the last five commands run, newest first, each once", () => {
  expect(recentAfter(["a", "b", "c", "d", "e"], "c")).toEqual(["c", "a", "b", "d", "e"]);
  expect(recentAfter(["a", "b", "c", "d", "e"], "f")).toEqual(["f", "a", "b", "c", "d"]);
  const s = viewReducer(DEFAULT_VIEW, { type: "ranCommand", id: "find" });
  expect(toSaved(s).recent_commands).toEqual(["find"]);
});
```

- [ ] **Step 6: Implement** `recentAfter`, the action, and `recent_commands: []` in the default view.
- [ ] **Step 7: Run** `pnpm test src/features/window`. Expected: PASS.
- [ ] **Step 8: Commit** after `cargo fmt --all`. `feat(layout): the window's saved view keeps the palette's last five commands, and the core repairs or refuses a bad list`

### Task 3: The palette's empty box, Recent first, and the "Change mode…" list

**Files:**
- Create: `src/features/palette/paletteList.ts`, `src/features/palette/paletteList.test.ts`
- Modify: `src/lib/keymap.ts` (adds `change-mode`), `src/features/palette/Palette.tsx`, `src/features/sessions/LivePalette.tsx` (`PALETTE_ONLY` gets `change-mode`), `src/features/sessions/LiveApp.tsx` (passes `recent`, `moved`, and records the run in `runFromPalette`)
- Test: `src/features/palette/Palette.test.tsx`, `src/features/sessions/LivePalette.test.tsx`

**Interfaces:**
- Consumes: `MOVED_COMMANDS` (Task 1), `view.recent_commands` and the `ranCommand` action (Task 2).
- Produces, in `paletteList.ts`:
  - `export const MODE_IDS: readonly string[]`, the five mode ids;
  - `export function emptyBox(commands: readonly Command[], recent: readonly string[], moved: readonly string[]): { recent: Command[]; rest: Command[] }`;
  - `export function withModeStep(commands: readonly Command[]): Command[]`, which drops `change-mode` when no mode row is in `commands`;
  - `export function recordsRecent(id: string): boolean`, false for `session:`/`file:` ids, `palette`, `change-mode` and `MOVED_COMMANDS`;
  - `export function paletteEscape(list: "commands" | "modes"): "back" | "close"`.
- Produces in `Palette`: new props `recent?: readonly string[]`, `moved?: readonly string[]` and `list?: { title: string; prompt: string }`.
- Produces in keymap: the command `{ id: "change-mode", label: "Change mode…", group: "Session" }`, placed before the five mode rows.

- [ ] **Step 1: Write the failing tests** in `paletteList.test.ts`:
  - `recent_drops_unknown_and_moved_ids`: `emptyBox(cmds, ["gone", "appearance", "find"], MOVED_COMMANDS).recent` lists only `find`.
  - `recent_lists_only_commands_that_are_offered`: a recent id missing from `commands` (hidden because it can't run) is not listed.
  - `the_empty_box_leaves_out_moved_items_mode_rows_and_what_recent_shows`: `rest` has no id from `MOVED_COMMANDS` or `MODE_IDS`, and nothing in `recent`. It keeps `change-mode`.
  - `recent_shows_at_most_five`.
  - `the_mode_step_goes_when_no_mode_is_offered`.
  - `only_palette_runs_of_listed_commands_are_recorded`: `recordsRecent("find")` is true. `recordsRecent("session:3")`, `("file:a.ts")`, `("change-mode")` and `("appearance")` are false.
  - `escape_goes_back_from_the_mode_list_then_closes`: `paletteEscape("modes") === "back"`, and `paletteEscape("commands") === "close"`.

  In `Palette.test.tsx`:
  - `an_empty_box_starts_with_recent_then_sessions_then_the_groups`. Check the markup order of the headings "Recent", "Open sessions", "Session" and "App". There is no "Settings: Appearance…" row and no "Mode: Plan…" row.
  - `typing_finds_a_moved_item_and_a_mode`. The query "appear" lists "Settings: Appearance…", and "bypass" lists "Mode: Bypass, only risky commands ask".
  - `enter_runs_the_row_shown_active_in_every_group`. Use recent `["find"]` and one session. For each `view.activeIndex` of 0, 1 and 2, the `data-active` row's label is, in turn, "Search in scrollback", the session, and the first Session-group command. This holds because Step 4 renders from the one `items` array that Enter reads.
  - `rest_is_in_group_order`, in `paletteList.test.ts`: `emptyBox(...).rest` is sorted stably by group order, so a group's rows are always together.
  - `a_second_list_shows_one_group_and_its_prompt`. `list={{ title: "Modes", prompt: "Type a mode" }}` renders one "Modes" group, and the input's aria-label is "Type a mode". There are no sessions and no Recent.
- [ ] **Step 2: Run** `pnpm test src/features/palette`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement `paletteList.ts`** as the Interfaces block says. `emptyBox` maps `recent` in order to the commands present in `commands`. It skips moved ids and duplicates, and stops at `RECENT_LIMIT`.
- [ ] **Step 4: Change `Palette.tsx`.**
  - **The empty box.**
    - `items` is `[...recent, ...sessions, ...rest]`, built once.
    - Render it in a single pass over `items`, opening a group heading wherever the group changes: "Recent", "Open sessions", then each command group.
    - The row ids and `data-active` then follow the same order Enter reads, so they can't disagree.
  - **Typing.** It filters over all of `commands`: moved items, mode rows and `change-mode` included.
  - **With `list`.**
    - It renders `commands` as one group titled `list.title`, with no sessions, files or Recent.
    - `list.prompt` is both the placeholder and the input's aria-label.
- [ ] **Step 5: Change `LivePalette.tsx`.**
  - **State.** It holds `list: "commands" | "modes"`, and `commands` passes through `withModeStep`.
  - **"Change mode…".** Running it sets `list` to `"modes"`, empties the query and resets the active row. The palette then shows only the mode rows of `commands`, with `{ title: "Modes", prompt: "Type a mode" }`.
  - **Esc.** The window-capture handler follows `paletteEscape(list)`: "back" returns to the commands with an empty query, and "close" closes.
  - **Props.** New `recent` and `moved` pass through.
  - **LiveApp.**
    - It passes `recent={view.recent_commands}` and `moved={MOVED_COMMANDS}`.
    - At the top of `runFromPalette`, it dispatches `{ type: "ranCommand", id }` when `recordsRecent(id)`.
  - Add `change-mode` to `PALETTE_ONLY`, and add `change-mode` to `keymap.ts`.
- [ ] **Step 6: Run** `pnpm test src/features/palette src/features/sessions src/lib`. Expected: PASS.
- [ ] **Step 7: Commit.** `feat(palette): the empty box starts with recent commands, leaves out the menu's destinations, and the five modes sit behind "Change mode…"`

### Task 4: A command that can't run now is left out

**Files:**
- Create: `src/features/palette/runnable.ts`, `src/features/palette/runnable.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (`hiddenCommands`, near line 671)

**Interfaces:**
- Produces:
  - `export type RunFacts = { focused: boolean; sessions: number; split: boolean; waiting: boolean; closed: boolean; shellPanel: boolean; sidebarToggle: boolean }`;
  - `export function cannotRun(f: RunFacts): readonly string[]`.

- [ ] **Step 1: Write the failing test** `each_missing_fact_leaves_out_its_commands`. Assert each rule on its own, starting from all facts true or ≥ 2, where `cannotRun` is `[]`:
  - `focused: false` gives close, rename, focus-prompt, find, bookmark, announce-status, prompt-prev, prompt-next, move-up and move-down;
  - `sessions: 0` gives go-to-session;
  - `sessions: 1` gives move-up and move-down;
  - `split: false` gives unsplit, zoom-pane and layout;
  - `waiting: false` gives next-waiting;
  - `closed: false` gives reopen;
  - `shellPanel: false` gives shell-panel;
  - `sidebarToggle: false` gives sidebar.

  Assert also that the result has no duplicates.
- [ ] **Step 2: Run** `pnpm test src/features/palette/runnable.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement `cannotRun`, and merge it into `hiddenCommands`.** The facts come from:
  - focused: `focusedInfo !== undefined`;
  - sessions: `rows.length`;
  - split: `view.panes.length > 1`;
  - waiting: `nextWaiting(requests) !== null`;
  - closed: `(closed?.length ?? 0) > 0`;
  - shellPanel: `sidebarExpanded`;
  - sidebarToggle: `sidebarToggleFor(sidebarPref, sidebarMode(width, sidebarPref)) !== null`, the check `MainWindow.tsx:293` makes.

  Use the pending-requests value LiveApp already holds for `nextWaiting`.
- [ ] **Step 4: Run** `pnpm test src/features/palette src/features/sessions`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(palette): commands that can't run now are left out, such as Close with no session in focus or Reopen with nothing closed`

### Task 5: Submenu keys, and the AppMenu component

**Files:**
- Modify: `src/components/ContextMenu/menuKeys.ts`, `src/components/ContextMenu/ContextMenu.tsx` (ignores the new results)
- Create: `src/features/appMenu/appMenu.ts`, `src/features/appMenu/AppMenu.tsx`, `src/features/appMenu/AppMenu.css`, `src/features/appMenu/appMenu.test.ts`, `src/features/appMenu/AppMenu.test.tsx`, `src/features/appMenu/AppMenu.css.test.ts`
- Test: `src/components/ContextMenu/menuKeys.test.ts`

**Interfaces:**
- **In `menuKeys.ts`.**
  - An action `MenuItem` gains `submenu?: true`. `MenuKeyResult` gains `{ kind: "open"; id: string }` and `{ kind: "back" }`.
  - Enter, Space or ArrowRight on a submenu item gives `open`. ArrowRight elsewhere gives null, and ArrowLeft always gives `back`.
  - `ContextMenu` treats `open` and `back` as no-ops.
- **In `appMenu.ts`.**
  - `export type AppMenuRun = "history" | "doctor" | "departures" | \`settings:${SettingsSection}\``;
  - `export const SETTINGS_ITEMS: ReadonlyArray<{ section: SettingsSection; label: string }>`: appearance "Appearance", agents "Agents", profiles "Shell profiles", notifications "Notifications", history "History settings", advanced "Diagnostics";
  - `export function appMenuItems(os: Os): MenuItem[]`: settings (submenu), history (shortcut = `shortcutLabel(history binding, os)`), doctor, departures;
  - `export function doctorItemLabel(issues: number | undefined): string`;
  - `export function menuPlace(anchor: { left: number; top: number } | null, size: { w: number; h: number }, viewport: { w: number; h: number }): { x: number; y: number }`;
  - `export function submenuPlace(menu: { left: number; right: number }, itemTop: number, size: { w: number; h: number }, viewport: { w: number; h: number }): { x: number; y: number }`.
- **In `AppMenu.tsx`:** `AppMenu({ os, anchor, doctorIssues, onRun, onClose }: { os: Os; anchor: { left: number; top: number } | null; doctorIssues?: number; onRun: (r: AppMenuRun) => void; onClose: (o: { refocus: boolean }) => void })`.

- [ ] **Step 1: Write the failing tests.**

  In `menuKeys.test.ts`:
  - `a_submenu_item_opens_on_enter_space_and_right_arrow`;
  - `left_arrow_goes_back`;
  - `right_arrow_on_a_plain_item_does_nothing`.

  In `appMenu.test.ts`:
  - `the_menu_lists_settings_history_doctor_departures`: labels in order, with Settings as a submenu and History's shortcut "Ctrl+Shift+Y" on Windows.
  - `settings_lists_its_six_sections`: `SETTINGS_ITEMS.map((s) => s.label)` equals the six labels.
  - `doctor_reads_its_count_and_no_number_before_a_check`: `doctorItemLabel(undefined) === "Doctor"`, `(0) === "Doctor, no issues"`, `(1) === "Doctor, 1 issue"` and `(3) === "Doctor, 3 issues"`.
  - `a_menu_with_no_anchor_opens_at_the_bottom_left`: `menuPlace(null, { w: 200, h: 160 }, { w: 1200, h: 800 })` gives `{ x: 8, y: 632 }`.
  - `a_menu_opens_above_its_button`: `menuPlace({ left: 14, top: 760 }, { w: 200, h: 160 }, { w: 1200, h: 800 })` gives `{ x: 14, y: 596 }`.
  - `the_submenu_opens_beside_the_menu_or_on_the_other_side`: to the right at `menu.right + 2` when it fits, else at `menu.left - w - 2`, with y clamped by `placeMenu`'s margin.

  In `AppMenu.test.tsx` (`renderToStaticMarkup`):
  - `role="menu" aria-label="Menu"`;
  - the Settings item has `aria-haspopup="menu"` and `aria-expanded="false"`;
  - History's `aria-keyshortcuts` is in the form the other menus use;
  - the Doctor item's `aria-label` is "Doctor, 3 issues", and the count is `aria-hidden`;
  - with `doctorIssues` undefined there is no count.
- [ ] **Step 2: Run** `pnpm test src/components/ContextMenu src/features/appMenu`. Expected: FAIL.
- [ ] **Step 3: Implement the key results and `appMenu.ts`.**
  - `menuPlace` with an anchor is `placeMenu({ x: anchor.left, y: anchor.top - size.h - 4 }, size, viewport)`.
  - `menuPlace` with no anchor is `placeMenu({ x: 8, y: viewport.h - size.h - 8 }, size, viewport)`.
- [ ] **Step 4: Implement `AppMenu.tsx`,** following `ContextMenu.tsx`:
  - **Focus.** Roving real focus with `usePopupFocus(ref, activeItem, { tab: false })`. Placement is measured in `useLayoutEffect`. A pointerdown outside both lists closes it without refocus, and so does a blur that `leavesMenu` both lists. Keys stop propagating.
  - **Opening the submenu.** `open` (or a click on Settings) shows `<ul role="menu" aria-label="Settings">` beside it, at `submenuPlace`, and focus goes to its first item.
  - **Leaving the submenu.** `back` or Esc inside it closes it and focuses Settings. Esc at the top level closes with refocus, and Tab closes without it.
  - **The Doctor item.** It renders `AuxGlyph kind="caution"` or `StatusGlyph kind="done-seen" size={12}` plus the number, `aria-hidden`, as the footer does at `Sidebar.tsx:318`.
  - **The Settings item** shows `›` (`aria-hidden`).
  - **Look.** Both lists use the `context-menu` classes, plus `app-menu` for its own rules.
  - **`AppMenu.css`.**
    - `.app-menu` sets `z-index: 20`, the window-wide layer, so it draws over the terminal's layers.
    - The count and the `›` use `--text-2`, and take the item's ink while it is focused.
    - In forced colours they are `CanvasText`, and `HighlightText` on the focused item.
    - The test asserts these rules, as `ContextMenu.css.test.ts` does.
- [ ] **Step 5: Run** `pnpm test src/components src/features/appMenu src/styles`. Expected: PASS.
- [ ] **Step 6: Commit.** `feat(menu): an app menu with a Settings submenu, opening above its button or at the window's bottom left`

### Task 6: The menu wired in: the footer and rail buttons, Ctrl+Shift+M, and the palette row

**Files:**
- Modify: `src/lib/keymap.ts` (the `menu` command), `src/lib/appShortcuts.ts` (`{ kind: "menu" }`), `src/features/appMenu/appMenu.ts` (`toggleMenu`), `src/features/window/Sidebar.tsx` and `Sidebar.css` (footer button), `src/features/window/Rail.tsx` and `Rail.css` (bottom button), `src/features/window/MainWindow.tsx` (passes the props through), `src/features/sessions/LiveApp.tsx`
- Test: `src/lib/keymap.test.ts`, `src/lib/appShortcuts.test.ts`, `src/features/appMenu/appMenu.test.ts`, `src/features/window/Sidebar.test.tsx`, `src/features/window/Rail.test.tsx`, `src/styles/overlayStacking.test.ts`, `src/features/window/Sidebar.css.test.ts`, and a new `src/features/window/Rail.css.test.ts`

**Interfaces:**
- Consumes: `AppMenu`, `AppMenuRun` and `menuPlace` (Task 5); `MOVED_COMMANDS` (Task 1).
- Produces:
  - **Command:** `{ id: "menu", label: "Open menu", group: "App", binding: { code: "KeyM", mac: { code: "KeyE" } } }`.
  - **State,** in `appMenu.ts`: `export type MenuState = { anchor: { left: number; top: number } | null; back: { focus(): void; isConnected: boolean } | null } | null`.
  - **Toggle:** `export function toggleMenu(open: MenuState, request: NonNullable<MenuState>): MenuState`. It returns `null` when a menu is open, else `request`.
  - **Props:** `Sidebar` and `Rail` gain `onOpenMenu?: (button: HTMLElement) => void` and `menuOpen?: boolean`, and MainWindow passes both through.

- [ ] **Step 1: Write the failing tests.**
  - **`keymap.test.ts`:**
    - "Open menu" reads "Ctrl+Shift+M" on Windows and "⌘E" on macOS;
    - `problemsIn(COMMANDS, os)` is `[]` for all three OSes.
  - **`appShortcuts.test.ts`:** `commandShortcut("menu")` is `{ kind: "menu" }`.
  - **`appMenu.test.ts`, `the_menu_key_toggles_and_remembers_where_focus_was`:** `toggleMenu(null, { anchor: null, back: el })` returns `{ anchor: null, back: el }`, and `toggleMenu(that, …)` returns `null`.
  - **`Sidebar.test.tsx`:**
    - with `onOpenMenu`, the footer renders even with no `footer` data;
    - its first child is `<button … class="sidebar__menu" aria-label="Menu" aria-haspopup="menu" aria-expanded="false">`, before the ring;
    - the Doctor button is unchanged (decision 9).
  - **`Rail.test.tsx`:**
    - with `onOpenMenu`, a `rail__menu` button named "Menu" renders after the `nav aria-label="Sessions"`, outside it;
    - without `onOpenMenu` there is none.
  - **`overlayStacking.test.ts`:** `<AppMenu` comes after `<LiveLauncher` and before the palette line.
  - **CSS tests:**
    - `.sidebar__menu` and `.rail__menu` have no `outline: none`;
    - their hover rules sit inside `@media (hover: hover) and (pointer: fine)`;
    - their forced-colors icon colour is `ButtonText`.
- [ ] **Step 2: Run** `pnpm test src/lib src/features/appMenu src/features/window src/styles`. Expected: FAIL.
- [ ] **Step 3: Implement the buttons.**
  - **Icon.** A three-line inline SVG, 16 px, in `currentColor`, `aria-hidden`.
  - **Footer button.** A 28 px square at the footer's left. The ring, the 5h % and the Doctor button follow it unchanged.
  - **Rail button.** The same, pinned to the rail's bottom in a 40 px row. The session list keeps scrolling above it.
  - **Shortcut.** Both buttons carry `aria-keyshortcuts` in the form the other buttons use.
  - **Styles.** Follow the top bar's sidebar toggle (`window__sidebar-toggle`): fill on hover and on press, and the base focus ring.
  - **Focus mode.** It renders neither the sidebar nor the rail, so there is no button (decision 6).
- [ ] **Step 4: Implement it in `LiveApp.tsx`.**
  - **State.** `const [menu, setMenu] = useState<MenuState>(null)`.
  - **Opening by key or palette.**
    - The anchor is the rect of the visible `.sidebar__menu` or `.rail__menu`, else `null`, and `back` is `document.activeElement`.
    - The palette's "menu" opens it in a `setTimeout`, after the palette has given focus back.
    - `shortcut.kind === "menu"` in the main key handler calls `setMenu((m) => toggleMenu(m, …))`.
  - **Opening by button.** The anchor is the button's rect, and `back` is the button.
  - **Running an item.**
    - `settings:<section>` calls `openSettings(section)`.
    - `history` runs `setMainView("history")` and bumps `setSearchFocus`.
    - `doctor` and `departures` call `setOverlay(id)`.
  - **Closing with `refocus`.** Focus goes to `back` when it is still connected, else to the terminal (`actions.focusTerminal()`).
  - **Placement and props.**
    - Render `<AppMenu os={os} anchor={menu.anchor} doctorIssues={doctorIssues} …/>` between `<LiveLauncher` and the palette line, so every dialog draws over it.
    - Pass `menuOpen={menu !== null}` to the sidebar and the rail.
- [ ] **Step 5: Run** `pnpm test`. Expected: all files pass.
- [ ] **Step 6: Commit.** `feat(menu): a Menu button at the bottom left of the sidebar and the rail opens Settings, History, Doctor and Departures, and Ctrl+Shift+M opens it anywhere`

### Task 7: PLAN text, the live check and its note

**Files:**
- Modify: `docs/PLAN.md`:
  - line 132, the footer: "40 px: the Menu button, plan ring, 5h %, and doctor glyph with issue count";
  - line 143, the rail: add "the Menu button at its bottom";
  - line 680: remove the Ctrl+Shift+Alt+F line;
  - the shortcut table near line 716: add "Menu | M | ⌘E", and in its notes say that ⌘M is Minimize;
  - line 1073: drop "and Ctrl+Shift+Alt+F opens it with the search box focused", and add that Ctrl+Shift+Y opens History with its search box focused.
- Create: `docs/research/2026-11-07-menu-palette-live-check.md`

- [ ] **Step 1: Update PLAN.md** at the lines above. Leave the old slices' decision lists as they are.
- [ ] **Step 2: Run the live check** with the `marshell-live-check` skill, under a new literal scratch folder. Check each of these with CDP keys and evaluations. Emulate forced colours, and take one screenshot each of the footer, the rail and the open submenu:
  1. The footer shows Menu, then the ring, the 5h % and the Doctor count.
  2. Ctrl+Shift+M opens the menu, and the arrows, typeahead, Right, Left and Esc behave as decision 10 says. Esc returns focus to the terminal.
  3. Each menu item opens its place. "History" focuses History's search box.
  4. On the rail, the button at the bottom opens the menu above itself.
  5. In focus mode, Ctrl+Shift+M opens the menu at the bottom left.
  6. Shrinking the window below 960 px while the menu is open leaves focus somewhere real, never the page.
  7. The empty palette starts with Recent after a command was run from it, and Recent survives an app restart.
  8. No Settings or History rows show with an empty box, and "appear" finds "Settings: Appearance…".
  9. "Change mode…" opens the Modes list for a Claude Code session. Esc goes back, and Esc again closes.
  10. With no session focused, "Close session" is not listed.
  11. Ctrl+Shift+Alt+F does nothing, and Ctrl+Shift+Y opens History with the search box focused.
- [ ] **Step 3: Write the research note** in the skill's format. Fix any finding the plan's tests missed, each with a test that fails first, and record it in the note.
- [ ] **Step 4: Commit.** `docs(plan): the footer, rail and shortcut table name the Menu button and drop Ctrl+Shift+Alt+F, with the 6p live check`
