# Session Row Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A right-click menu on sidebar session rows (also Shift+F10 and the Menu key) with Rename, Split into a pane, Resume, Review changes, a separator, and Close.

**Architecture:**
- A generic, accessible `ContextMenu` component lives in `src/components/ContextMenu/`. Its key handling and placement are pure helpers, so the no-DOM Vitest setup can test them.
- `sessionMenuItems` decides which items a row shows.
- `Sidebar` owns the open menu.
- `LiveApp` runs the chosen item through the same code the palette and the shortcuts use. Actions take an explicit tab id, so they never depend on a selection update landing first.

**Tech Stack:** React 19, TypeScript, Vitest (no DOM: pure helpers plus CSS-reading tests), Storybook.

**Spec:** The user decisions of 2026-10-17, recorded in memory and repeated here:
- Items: Rename, Split into a pane, Resume, Review changes, separator, Close.
- Each item shows its shortcut, and items that don't apply are hidden.
- Shift+F10 and the Menu key open the menu, and Esc returns focus to the row.
- No move items.
- Mode ▸ comes in the next PR ("modes at launch"). Bookmark is hidden until phase 5. Pause is its own PR.

## Global Constraints

- Keyboard-first and accessible:
  - `role="menu"` with `role="menuitem"` items and `role="separator"`.
  - Up and Down wrap; Home and End work.
  - Enter and Space run the item.
  - A typed letter jumps to the next item starting with it.
  - Esc closes the menu and focuses the row. Tab closes it and leaves focus where Tab puts it.
  - A click outside closes it.
- The menu is vendor-neutral, so it uses the app accent (Sand), never `--agent-signal`. It must not sit under any `[data-agent]` element: render it as a child of `.sidebar`, outside the rows.
- Use only the existing design tokens (`--bg-overlay`, `--shadow-pill`, `--radius`, `--space-*`, `--text-*`, `--accent`, `--focus`).
- Add no new dependencies.
- Exact item labels: `Rename`, `Split into a pane`, `Resume`, `Review changes`, `Close`.
- Shortcut text comes from `shortcutLabel(commandById(id).binding!, os)`:
  - `rename` gives F2.
  - `review-task` gives Ctrl+Shift+R.
  - `close` gives Ctrl+Shift+W.
  - Split and Resume show no shortcut. Ctrl+Shift+D adds an empty pane, which is a different action.
- Copy rule (PLAN): plain words, no guessed states.

## Review Focus

1. **Right-click on a row that isn't selected:**
   - Expected: the row becomes selected, and the menu acts on that row, not the previous one.
   - Pinned by the explicit-id actions in Task 4 and the `sessionMenuItems` tests in Task 2.
2. **The menu near the window's bottom or right edge** (the last row, a narrow sidebar):
   - Expected: it stays fully on screen.
   - Pinned by the `placeMenu` tests in Task 1.
3. **The row closes or resumes while its menu is open:** the core snapshot changes underneath.
   - Expected: the menu closes. It never runs an action on a row that is gone.
   - Pinned by the `menuStillValid` test in Task 3.
4. **Shift+F10 while renaming,** when the target is the rename text box:
   - Expected: no menu; the text box keeps the key.
   - Pinned by the `opensRowMenu` test in Task 3.
5. **Split into a pane for a session that is already in a pane on screen:**
   - Expected: the item is hidden, never a no-op.
   - Pinned by a `sessionMenuItems` test in Task 2.

---

### Task 1: ContextMenu component

**Files:**
- Create: `src/components/ContextMenu/menuKeys.ts`, `src/components/ContextMenu/ContextMenu.tsx`, `src/components/ContextMenu/ContextMenu.css`, `src/components/ContextMenu/ContextMenu.stories.tsx`
- Test: `src/components/ContextMenu/menuKeys.test.ts`, `src/components/ContextMenu/ContextMenu.css.test.ts`

**Interfaces:**
- Produces:
  - `type MenuItem = { id: string; label: string; shortcut?: string } | { separator: true }`
  - `menuKey(key: string, items: MenuItem[], active: number): { kind: "move"; index: number } | { kind: "run"; id: string } | { kind: "close"; refocus: boolean } | null`
  - `placeMenu(anchor: { x: number; y: number }, size: { w: number; h: number }, viewport: { w: number; h: number }, margin = 8): { x: number; y: number }`
  - `<ContextMenu items label anchor onRun onClose />`, where `label: string` is the aria-label, `onRun: (id: string) => void` and `onClose: (opts: { refocus: boolean }) => void`

- [ ] **Step 1: Write the failing tests in `menuKeys.test.ts`.** Items: `[a "Rename", b "Resume", sep, c "Close"]`.
  - `ArrowDown` from 0 gives `{move, 1}`.
  - `ArrowDown` from 1 skips the separator and gives index 3.
  - `ArrowDown` from 3 wraps to 0.
  - `ArrowUp` from 0 gives 3.
  - `Home` gives 0, and `End` gives 3.
  - `Enter` and `" "` at 1 give `{run, "b"}`.
  - `Escape` gives `{close, refocus: true}`, and `Tab` gives `{close, refocus: false}`.
  - `"c"` from 0 gives `{move, 3}`. `"r"` from 0 gives `{move, 1}`: it searches from the next item and wraps, and matching ignores case.
  - `"x"` gives null.

  `placeMenu` tests:
  - `({x:10,y:10},{w:200,h:100},{w:800,h:600})` gives `{x:10,y:10}`.
  - At `x:700` it gives `x:592` (800-200-8).
  - At `y:580` it gives `y:492`.
  - When the menu is larger than the viewport, it gives the margin (8, 8).

- [ ] **Step 2: Run `pnpm test src/components/ContextMenu`.** Expected: FAIL, because the module is not found.

- [ ] **Step 3: Implement `menuKey` and `placeMenu` in `menuKeys.ts`.** Separators are never active.

- [ ] **Step 4: Implement `ContextMenu.tsx`.**
  - A `ul.context-menu` with `role="menu"`, `aria-label={label}`, `tabIndex={-1}`, and `position: fixed` at `placeMenu(...)`. Measure after the first layout with `useLayoutEffect` and a ref.
  - Each item is `li role="menuitem" tabIndex={-1}` holding a label span and an optional `span.context-menu__shortcut` (`aria-hidden`). The shortcut is also given through `aria-keyshortcuts` in its plain text.
  - The active item takes real DOM focus (roving), which is item 0 on open.
  - `onKeyDown` goes through `menuKey`, and calls `preventDefault` for every non-null result except the Tab close.
  - A click on an item runs it. Hover moves the active item.
  - A `pointerdown` listener on `window` in the capture phase closes the menu with `{refocus: false}` when the target is outside the menu.
  - A `contextmenu` event on the menu itself is prevented.
  - Separators are `li role="separator"`.

- [ ] **Step 5: Write `ContextMenu.css` and its test.**
  - The CSS test reads the file and asserts:
    - no `--agent` reference;
    - `.context-menu` uses `var(--bg-overlay)`;
    - the active item (`[role="menuitem"]:focus`) uses `var(--accent)` in its background or outline.
  - Size: min width 180px, item height 28px (`--space-*` padding), and the shortcut right-aligned in `--text-2`.
  - `@media (forced-colors: active)`: the focused item uses `Highlight` / `HighlightText`.
  - The menu has no focus ring; the active item shows the state instead (see the Palette precedent).

- [ ] **Step 6: Add a story with the five items, including the separator, and a shortcut on three of them.**

- [ ] **Step 7: Run `pnpm test src/components/ContextMenu`.** Expected: PASS.

- [ ] **Step 8: Commit.** Message: `feat(menu): an accessible context menu component`.

### Task 2: Which items a session row shows

**Files:**
- Create: `src/features/sidebar/sessionMenu.ts`
- Test: `src/features/sidebar/sessionMenu.test.ts`

**Interfaces:**
- Consumes: `MenuItem` (Task 1), `shortcutLabel` and `commandById` from `src/lib/keymap.ts`.
- Produces:
  - `type SessionMenuFacts = { resumable: boolean; task: boolean; inShownPane: boolean; paneAvailable: boolean }`
  - `sessionMenuItems(facts: SessionMenuFacts, os: Os): MenuItem[]`
  - Item ids are `"rename" | "split" | "resume" | "review-task" | "close"`.

- [ ] **Step 1: Write the failing tests.**
  - All facts true except `inShownPane`, on Windows: the ids in order are `rename, split, resume, review-task, <separator>, close`. The shortcuts are `F2`, none, none, `Ctrl+Shift+R`, `Ctrl+Shift+W`.
  - `inShownPane: true`: no `split`.
  - `paneAvailable: false`: no `split`.
  - `resumable: false`: no `resume`.
  - `task: false`: no `review-task`.
  - The minimal facts (all false) give `rename, <separator>, close`.
  - On mac, the close shortcut equals `shortcutLabel(commandById("close").binding!, "mac")`.

- [ ] **Step 2: Run `pnpm test src/features/sidebar/sessionMenu`.** Expected: FAIL.

- [ ] **Step 3: Implement it.**

- [ ] **Step 4: Run the same tests.** Expected: PASS.

- [ ] **Step 5: Commit.** Message: `feat(sidebar): the items a session row's menu shows`.

### Task 3: Open the menu from a row

**Files:**
- Modify: `src/features/sidebar/SessionRow.tsx` (props and handlers on the root div, lines 36-92)
- Modify: `src/features/window/Sidebar.tsx` (menu state, rendering `ContextMenu`)
- Modify: `src/features/window/MainWindow.tsx:396` (pass-through props)
- Create: `src/features/sidebar/rowMenu.ts`
- Test: `src/features/sidebar/rowMenu.test.ts`

**Interfaces:**
- Consumes: `ContextMenu` and `MenuItem` (Task 1).
- Produces:
  - `opensRowMenu(e: { key: string; shiftKey: boolean; ctrlKey: boolean; altKey: boolean; metaKey: boolean; target: { closest?: (s: string) => unknown } | null }): boolean`
  - `keyboardAnchor(rect: { left: number; bottom: number }): { x: number; y: number }`
  - `menuStillValid(openId: string | null, ids: readonly string[]): boolean`
  - New `SessionRow` prop: `onMenu?: (anchor: { x: number; y: number }) => void`
  - New `Sidebar` and `MainWindow` props: `menuItems?: (id: string) => MenuItem[]` and `onMenuRun?: (id: string, itemId: string) => void`

- [ ] **Step 1: Write the failing tests.**
  - `opensRowMenu`:
    - `{key:"F10", shiftKey:true}` gives true.
    - `{key:"ContextMenu"}` gives true.
    - `{key:"F10"}` gives false.
    - `{key:"F10", shiftKey:true, ctrlKey:true}` gives false.
    - A target whose `closest(".rename-box")` is truthy gives false. Use the class `RenameBox` renders; read `RenameBox.tsx` for it.
  - `keyboardAnchor({left: 12, bottom: 100})` gives `{x: 20, y: 100}` (left + 8).
  - `menuStillValid`:
    - `("a", ["a","b"])` gives true.
    - `("c", ["a","b"])` gives false.
    - `(null, [])` gives false.

- [ ] **Step 2: Run `pnpm test src/features/sidebar/rowMenu`.** Expected: FAIL.

- [ ] **Step 3: Implement `rowMenu.ts`. Then wire `SessionRow`.**
  - `onContextMenu` calls `preventDefault`, then `onMenu({x: clientX, y: clientY})`.
  - `onKeyDown`, when `opensRowMenu(e)` is true, calls `preventDefault` and `stopPropagation`, then `onMenu(keyboardAnchor(e.currentTarget.getBoundingClientRect()))`.
  - The row's `aria-haspopup="menu"` is set only when `onMenu` is given.

- [ ] **Step 4: Wire `Sidebar`.**
  - State: `menu: { id: string; anchor } | null`.
  - A row's `onMenu`:
    - calls `onSelect?.(id)` (Windows selects the row on right-click);
    - sets `menu`, but only when `menuItems` is given.
  - Render `<ContextMenu label={`Session ${name}`} items={menuItems(menu.id)} …/>` as a direct child of `aside.sidebar`, while `menuStillValid(menu?.id, ids)` holds. A `useEffect` clears `menu` when it stops holding.
  - `onRun`: clear the menu, then `onMenuRun(menu.id, itemId)`.
  - `onClose({refocus})`: clear the menu. If `refocus` is set, focus `[data-nav-row="<number>"]` of that row inside the sidebar, the same selector the navigation uses.
  - `MainWindow` passes `menuItems` and `onMenuRun` straight through to `Sidebar`.

- [ ] **Step 5: Run `pnpm test src/features/sidebar src/features/window`.** Expected: PASS, with existing tests unchanged.

- [ ] **Step 6: Commit.** Message: `feat(sidebar): right-click, Shift+F10 and the Menu key open a row's menu`.

### Task 4: Run the items

**Files:**
- Modify: `src/features/sessions/sessionActions.ts`
  - `requestClose(tabId?: string)` and `startRename(tabId?: string)`: an absent id means the selected session, as today.
- Modify: `src/features/sessions/LiveApp.tsx`
  - Extract `showInPane(id: string)` from the adopt effect (the `adoptTarget` block at lines 395-408), and use it there.
  - Build `menuItems` and `onMenuRun`.
- Test: `src/features/sessions/sessionActions.test.ts`, plus a new `src/features/sessions/sessionMenuRun.ts` with its test.

**Interfaces:**
- Consumes: `sessionMenuItems` and `SessionMenuFacts` (Task 2); the `Sidebar` / `MainWindow` props (Task 3).
- Produces:
  - `menuFacts(id: string, ctx: { infos: SessionInfo[]; rows: RowModel[]; panes: (string|null)[]; shown: number; fit: number }): SessionMenuFacts`
    - `resumable`: the info's `state.kind !== "running"`.
    - `task`: `row.task` is set.
    - `inShownPane`: `panes.slice(0, shown).includes(id)`.
    - `paneAvailable`: `panes.slice(0, shown).includes(null) || roomForPane(panes.length, fit)`.

- [ ] **Step 1: Write the failing tests.**
  - `sessionActions.test.ts`:
    - With `selectedId: "a"`, `requestClose("b")` closes `b` (the fake api's close is called with `"b"`).
    - `startRename("b")` sets `renaming: "b"`.
    - A no-arg call still acts on the selected session.
  - `sessionMenuRun.test.ts` covers `menuFacts`, one case per fact:
    - a running session is not resumable;
    - an exited one is;
    - one in pane 0 of 2 shown has `inShownPane` true;
    - 4 panes with no null are `paneAvailable` false;
    - `[x, null]` is true.

- [ ] **Step 2: Run `pnpm test src/features/sessions`.** Expected: the new tests FAIL.

- [ ] **Step 3: Implement the explicit-id actions and `menuFacts`. In `LiveApp`, `onMenuRun(id, itemId)` does the following:**
  - `rename`: `actions.startRename(id)`.
  - `split`: `showInPane(id)`.
  - `resume`: `startResume(row)` for that id's row.
  - `review-task`: `setTimeout(() => reviewSession(id), 0)`, so the sheet takes focus after the menu has gone.
  - `close`: `void actions.requestClose(id)`.

  `menuItems(id)` is `sessionMenuItems(menuFacts(id, …), os)`, using the same `infos`, `rows`, view panes, `shownRef` and `fitRef` that the window uses. When `os` is unknown, pass no `menuItems`.

- [ ] **Step 4: Run `pnpm test`.** Expected: all PASS.

- [ ] **Step 5: Run `pnpm build`.** Expected: no type errors.

- [ ] **Step 6: Commit.** Message: `feat(sessions): run the row menu's items`.

### Task 5: Docs and the by-eye checks

**Files:**
- Modify: `docs/PLAN.md`
  - Add a bullet under §9 after "Slice 2f decisions": **Session row menu (2026-10-17)**. It lists the items, the keys, the hidden-when-not-applicable rule, and that Mode ▸, Pause and Bookmark come later (and in which PR or phase).
  - In the keyboard section, near the bookmark line (around 666), add Shift+F10 / the Menu key, "the focused row's menu".

- [ ] **Step 1: Write the two doc edits.**

- [ ] **Step 2: Commit.** Message: `docs(plan): the session row menu`.

- [ ] **Step 3: Write the PR body.** It needs:
  - a summary;
  - the by-eye checklist for the user:
    1. Right-click a row that isn't selected: it becomes selected and the menu opens at the pointer.
    2. Shift+F10 on a focused row, then Down, Enter on Rename: the rename box opens.
    3. The Menu key, then Esc: focus is back on the row.
    4. Close from the menu on a running agent: the confirm bar.
    5. Split into a pane: the session shows in a new pane, and the item is gone the next time.
    6. Resume on an exited row.
    7. Review changes on a task row.
    8. The menu on the last row stays on screen.
  - the deferred items: Mode ▸, Pause, Bookmark.
