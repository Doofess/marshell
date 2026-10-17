# Sidebar Pane Marks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The session list mirrors the panes on screen.

- Every session visible in a pane is marked, and the focused pane's session is marked most strongly.
- A plain click fills an empty pane first.
- Ctrl+click (Cmd+click on macOS) or Ctrl+Space adds a session in a new pane, or removes the pane of a session already on screen.

**Architecture:**

- The pane logic is pure functions in `viewState.ts`: which sessions are visible, what a toggle does, and the select rule that fills an empty pane first.
- The row input rules are pure functions in a new `rowPick.ts`.
- The UI change is thin:
  - `Sidebar` and `Rail` take the visible ids and an `onToggle` callback.
  - `SessionRow` marks rows with `aria-selected` and `data-focused`.
  - `LiveApp` computes the visible ids and runs the toggle.

**Tech Stack:** React, TypeScript, and Vitest with no DOM. Markup tests use static rendering, as `Sidebar.test.tsx` does. CSS tests read the CSS files.

**Spec:** the user's answers of 2026-10-17, recorded in Task 5's PLAN.md bullet, and `docs/PLAN.md` §Split view (line ~139) and the keyboard section (~690).

## Global Constraints

- **Visible means visible.** While a pane is zoomed, only the zoomed pane's session is visible. Panes past the count the window shows (`shownCount`) are not visible. Empty panes contribute nothing.
- **Plain click on a session that isn't visible:**
  - It fills the first empty visible pane and focuses it.
  - If there is none, it replaces the focused pane's session, as today.
  - While a pane is zoomed, empty panes are not visible, so it replaces the zoomed pane's session.
  - A click on a visible session only focuses its pane, as today.
- **Toggle** (Ctrl+click on Windows and Linux, Cmd+click on macOS, or Ctrl+Space on a focused row on every OS):
  - On a visible session, it removes that session's pane. It does nothing when that is the only pane.
  - On a session in a pane that isn't visible, it brings that session into view: the same path as "Split into a pane".
  - Otherwise it shows the session in an empty visible pane, or in a new pane if the window fits one more.
  - If the window fits no more panes, nothing changes and the announcer says "No room for another pane." (`NO_PANE_ROOM` in `LiveApp.tsx`).
- **Marks:**
  - Every visible session's row has `aria-selected="true"` and the `--selected` fill.
  - The focused pane's row also has `data-focused` and a 3px `--accent` bar on the row's inline-end edge. The inline-start edge already holds the vendor's brand stripe.
  - The list gets `aria-multiselectable="true"` when visible ids are passed.
  - The roving tab stop stays on the focused row.
- **Forced colors:** a visible row has a `1px dashed Highlight` outline, and the focused row a `2px solid Highlight` outline. The two differ in shape, not only in colour.
- **No change to the keyboard nav cursor, the row menu, or the core.**

## Review Focus

1. **Ctrl+click on the row inside its rename box:** expected no toggle. The text box keeps the click, and Ctrl+Space stays in the box. Pinned by the `togglesPaneKey` rename-box test in Task 2.
2. **Ctrl+click on the only pane's session:** expected nothing. The layout never reaches zero panes. Pinned by the `paneToggle` "only" test in Task 1.
3. **Zoomed layout, click on a session that isn't visible:** expected it to replace the zoomed pane's session. It must never fill an empty pane hidden behind the zoom. Pinned by a `select` test in Task 1.
4. **A window too small for all panes:**
   - Expected: sessions in cut-off panes are unmarked.
   - Ctrl+click on one of them brings it into view, and never removes its hidden pane.
   - Pinned by the `visibleSessions` and `paneToggle` tests in Task 1.
5. **macOS Ctrl+click is a right-click:**
   - Expected: on macOS only Cmd+click toggles, and Ctrl+click never does.
   - On Windows and Linux, a Meta+click doesn't toggle.
   - Pinned by the `togglesPane` tests in Task 2.

---

### Task 1: Pane rules in viewState

**Files:**
- Modify: `src/features/window/viewState.ts`
- Test: `src/features/window/viewState.test.ts`

**Interfaces:**
- Produces:
  - `visibleSessions(panes: readonly (string | null)[], shown: number, zoomed: number | null | undefined): string[]`
  - `paneToggle(view: Pick<ViewLayout, "panes" | "active" | "zoomed">, id: string, shown: number, fit: number): { kind: "remove"; index: number } | { kind: "only" } | { kind: "show" } | { kind: "full" }`
  - The `select` action with `visible` given fills an empty visible pane first.

- [ ] **Step 1: Write failing tests**
  - `visibleSessions`:
    - `(["a","b",null], 3, null)` gives `["a","b"]`.
    - `(["a","b","c"], 2, null)` gives `["a","b"]`.
    - `(["a","b"], 2, 1)` gives `["b"]`.
    - A zoomed index of 2 that is not shown (`shown` 2) is ignored, so `(["a","b","c"], 2, 2)` gives `["a","b"]`.
  - `paneToggle`:
    - "remove": visible `b` in `["a","b"]` with shown 2 gives `{kind:"remove", index:1}`.
    - "only": `a` in `["a"]` gives `{kind:"only"}`.
    - "show", from a hidden pane: `c` in `["a","b","c"]` with shown 2 gives `{kind:"show"}`.
    - "show", into an empty pane: `x` with panes `["a",null]` and fit 2 gives `{kind:"show"}`.
    - "show", into a new pane: `x` with `["a"]` and fit 2 gives `{kind:"show"}`.
    - "full": `x` with `["a","b"]` and fit 2 gives `{kind:"full"}`.
    - Zoomed on 0 in `["a","b"]`, `b` (not visible) gives `{kind:"show"}`.
  - `select`, with visible given:
    - `["a",null]`, active 0, select `x` with visible 2, gives panes `["a","x"]` and active 1.
    - Zoomed 0 in `["a",null]`, select `x`, gives panes `["x",null]`.
    - The existing select tests must still pass unchanged.

- [ ] **Step 2: Run** `pnpm vitest run src/features/window/viewState.test.ts`. Expected: the new tests FAIL, because the functions are not defined or `select` doesn't fill the empty pane.

- [ ] **Step 3: Implement.**
  - `visibleSessions` and `paneToggle` reuse `focusedIndex`, `roomForPane` and `adoptTarget`'s empty-pane rule. Don't duplicate them.
  - In `select`'s visible branch, when `s.zoomed === null` and an empty pane exists in `0..last`, assign there and make it active.

- [ ] **Step 4: Run** the same file. Expected: all PASS.

- [ ] **Step 5: Commit** `feat(panes): which sessions are visible, what a toggle does, and a click fills an empty pane first`.

### Task 2: Row input rules

**Files:**
- Create: `src/features/sidebar/rowPick.ts`
- Test: `src/features/sidebar/rowPick.test.ts`

**Interfaces:**
- Consumes: `inRenameBox(target)` from `src/features/sidebar/rowMenu.ts`.
- Produces:
  - `togglesPane(e: { ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }, os: Os): boolean`. On macOS it is true for Meta alone. Elsewhere it is true for Ctrl alone.
  - `togglesPaneKey(e: { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean; target: object | null }): boolean`. It is true for `key === " "` with Ctrl alone, on every OS, and false inside the rename box.

- [ ] **Step 1: Write failing tests.** Cover each of these:
  - Windows: Ctrl gives true; Ctrl+Shift false; Meta false; none false.
  - macOS: Meta gives true; Ctrl false (it is a right-click there); Meta+Alt false.
  - Ctrl+Space gives true; Ctrl+Shift+Space false; plain Space false; Ctrl+Space inside the rename box false.

- [ ] **Step 2: Run** `pnpm vitest run src/features/sidebar/rowPick.test.ts`. Expected: FAIL, because the module is missing.

- [ ] **Step 3: Implement** both functions.

- [ ] **Step 4: Run** the file. Expected: PASS.

- [ ] **Step 5: Commit** `feat(sidebar): the clicks and keys that add or remove a session's pane`.

### Task 3: Marks and toggle in the list and the rail

**Files:**
- Modify:
  - `src/features/sidebar/SessionRow.tsx`
  - `src/features/sidebar/SessionRow.css`
  - `src/features/window/Sidebar.tsx`
  - `src/features/window/Rail.tsx`
  - `src/features/window/Rail.css`
  - `src/features/window/MainWindow.tsx`
- Test:
  - `src/features/window/Sidebar.test.tsx`
  - `src/features/sidebar/SessionRow.css.test.ts`
  - `src/features/window/Rail.test.tsx`

**Interfaces:**
- Consumes: `togglesPane` and `togglesPaneKey` from Task 2.
- Produces:
  - `Sidebar` props: `visibleIds?: readonly string[]`, `onToggle?: (id: string) => void`, `os?: Os`.
  - `selectedId` keeps its meaning: the focused session.
  - `Rail` gets `visibleIds` and `onToggle`.
  - `MainWindow` passes `visibleIds` and `onToggle` through to both.
  - `SessionRow` gets a `focused?: boolean` prop. Its `onSelect` becomes `(e: { ctrlKey; metaKey; altKey; shiftKey }) => void`, or the row calls `onToggle` itself; pick the smaller diff.

- [ ] **Step 1: Write failing tests**
  - `Sidebar.test.tsx`, with `visibleIds={["a","b"]}` and `selectedId="b"`:
    - Rows a and b have `aria-selected="true"`, and the others `"false"`.
    - Only b has `data-focused`.
    - The listbox has `aria-multiselectable="true"`.
    - The single tab stop is on b.
  - With `visibleIds` omitted, the markup is unchanged: only the selected row is selected, and there is no `aria-multiselectable`. The existing "does not change the markup" test covers this; keep it.
  - `SessionRow.css.test.ts`:
    - `.session-row[data-focused]` draws a 3px `var(--accent)` bar at the inline end. Any technique is fine, but no physical `left`/`right`.
    - In `@media (forced-colors: active)`, `.session-row[aria-selected="true"]` has `1px dashed Highlight`, and `.session-row[data-focused]` has `2px solid Highlight`.
  - `Rail.test.tsx`: with `visibleIds={["a","b"]}` and `selectedId="b"`:
    - Item a has `data-visible`.
    - Item b has `aria-current` as today, plus `data-visible`.

- [ ] **Step 2: Run** `pnpm vitest run src/features/window/Sidebar.test.tsx src/features/sidebar/SessionRow.css.test.ts src/features/window/Rail.test.tsx`. Expected: the new tests FAIL.

- [ ] **Step 3: Implement**
  - Row click: `togglesPane(e, os) ? onToggle(id) : onSelect(id)`, with `os` defaulting to `"windows"`.
  - Row keydown: `togglesPaneKey(e)` calls `preventDefault`, then `onToggle(id)`. The row-menu check runs first.
  - Rail: the same click rule.
  - The rail's visible mark uses the existing selected token (`--selected`) as a fill. Read `Rail.css` for how `aria-current` is drawn, and keep the focused item's look as it is.
  - Before writing the CSS, read the `frontend-design` skill (and `good-css` if installed). The look decided here is fixed by Global Constraints.

- [ ] **Step 4: Run** the three files, then `pnpm test`. Expected: all PASS.

- [ ] **Step 5: Commit** `feat(sidebar): mark every session on screen, and toggle a pane with Ctrl+click or Ctrl+Space`.

### Task 4: Wire LiveApp

**Files:**
- Modify: `src/features/sessions/LiveApp.tsx`

**Interfaces:**
- Consumes:
  - `visibleSessions` and `paneToggle` (Task 1).
  - The `Sidebar`/`Rail`/`MainWindow` props (Task 3).
  - The existing `showInPane`, `announce`, `NO_PANE_ROOM` and `dispatch({type:"remove", index})`.

- [ ] **Step 1: Implement**
  - `visibleIds = visibleSessions(view.panes, shownCount ?? view.panes.length, view.zoomed)`. Memoise it on those three values.
  - `togglePane(id)` reads `viewRef`, `shownRef` and `fitRef` and acts on `paneToggle`'s result:
    - "remove": dispatch `remove`.
    - "show": `showInPane(id)`.
    - "full": `announce` `NO_PANE_ROOM`, the same way the split shortcut announces it. Find that call and copy its form.
    - "only": nothing.
  - Pass `visibleIds`, `onToggle={togglePane}` and `os` to `MainWindow`.

- [ ] **Step 2: Run** `pnpm test` and `pnpm exec tsc --noEmit -p .`. Expected: all pass, and tsc is clean.

- [ ] **Step 3: Commit** `feat(sessions): the session list shows and toggles the panes on screen`.

### Task 5: Docs

**Files:**
- Modify: `docs/PLAN.md`

- [ ] **Step 1:** Add a bullet **Sidebar pane marks (2026-10-17)** after the "Session row menu (2026-10-17)" bullet. It holds the Global Constraints rules in the plan's numbered style.
- [ ] **Step 2:** In the keyboard section, after the Shift+F10 line, add: "Ctrl+Space on a focused session row (and Ctrl+click, Cmd+click on macOS) adds that session in a pane, or removes its pane if it is on screen."
- [ ] **Step 3: Commit** `docs(plan): the sidebar marks the panes`.
