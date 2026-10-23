# Phase 4c: the Departures board and the attention choreography Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Departures board, built and signed off in phase 0, goes live: it opens full window from the palette and the tray and shows every session's live state. Its status and waiting cells flip when they change, Y and N open the real request inline, and Enter jumps to the session. In the sidebar, a row flashes once when its session starts needing you, is Done or errors.

**Architecture:** All webview work, with no core, protocol or shell change. The tray already forwards any item id as `tray-action`.
- The phase-0 `DeparturesBoard` stays the presentational component.
- A new `LiveDepartures` owns the live parts:
  - the measured height;
  - the keyboard (`reduceBoard`);
  - holding the order while the pointer is over the board (`freezeOrder`, plus the gate membership);
  - the rows of sessions that closed during the hold;
  - the flips, computed from the previous texts;
  - the inline `LiveCard`.
- Live flipping plays `flipFrame` on animation frames inside `FlipCell`. Under reduced motion the cell crossfades. Screen readers read only the final text.
- A row flash is a pure transition check (`attentionFlashes`) over successive row snapshots, plus a hook that holds `flash: true` on a row for one `--dur-flash`. The CSS (`.session-row[data-flash]`, `row-flash`, its reduced form) already exists.

**Tech Stack:** React + TypeScript, Vitest (node, `renderToStaticMarkup`). No new dependency.

**Spec:**
- `docs/PLAN.md` §9, "Slice 4c decisions (2026-10-23)".
- PLAN "Departures board" (the visual rules) and the "Motion" table.
- PLAN batch 3 decisions, "Departures": hold under the pointer; Y opens on the card and N on Deny, neither answering; the layout letter; table roles; the roving tab stop; "+N more" counted in the height.
- BRIEF §5 feature 4 and feature 8, with the 2026-10-23 amendment.
- The phase-0 design is `src/features/departures/` and its plan, `docs/superpowers/plans/2026-10-10-phase-0-batch-3-onboarding-doctor-settings-departures.md`.

## Global Constraints

- **Placement.** The board covers the window below the 40 px title bar (`position: absolute; inset: 40px 0 0 0; z-index: 20`), so the window controls stay usable. It is true black with 44 px rows, as built. Sessions keep running under it.
- **Opening and closing.** It opens from the palette command `departures` ("Open Departures board", made live) and from the tray item "Open Departures" (`departures: true` in the tray state). There is no default key.
  - Escape closes an open card first, then the board.
  - Closing returns focus to where it was: the terminal, or the palette's opener.
  - While the board is open, the window's other shortcuts don't fire (`overlay !== null`).
- **Rows.** Exactly the sidebar's rows (`taskRows`), in the board's own order: gate calls oldest waiting first, then the rest by session number.
- **Gate calls** are the tabs in the needs-you list (`requests`, from `laneEntries` through `useCards`). These are the same tabs the title count, the badge and the tray count (4b decision 1), so a plain shell holding a request is a gate call even though its own status reads Idle (4b known limit: plain-shell rows ignore hooks).
  - Such a row shows the waiting status the tray would give it: its own needs-permission or needs-question, else needs-permission.
  - Its waiting time is the longest of its entries.
- **Hold under the pointer.** While the pointer is over the board's table (`.board__table`, not the whole full-window layer):
  - no row moves, and a row keeps the group it had;
  - new rows are appended at the end;
  - a row whose session closed stays in place, dimmed, drawn from its last `RowModel`;
  - a row that now needs you, outside Gate calls, shows "Moves up when you move away".

  When the pointer leaves, everything settles at once.
- **Flips.** Only the status and waiting cells flip, at 60 ms per character with a 15 ms stagger and a 400 ms cap (`flip.ts`), and only when the text they show changes. A row that appears doesn't flip. The waiting text counts whole minutes, so a waiting cell flips at most once a minute. Under reduced motion (`prefers-reduced-motion` or `data-motion="reduce"`) the cell crossfades over 200 ms. A flip mid-way when the text changes again restarts from the text on screen.
- **Screen readers.** A flipping cell's moving letters are `aria-hidden`, and its final text is always in the accessibility tree. Nothing is announced per frame.
- **Keys.**
  - The board is a modal layer: Tab and Shift+Tab stay inside it (`trapKey`), and the title-bar controls stay reachable by pointer.
  - The keyboard's place is a row id, not an index. When rows reorder (the keyboard doesn't freeze them), the tab stop stays on the same session; if that session is gone, it moves to the row now at its index.
  - Arrow keys, Home and End move the one tab stop.
  - Enter jumps to the session (`pick`, then focus its terminal) and closes the board.
  - Y on a gate row opens that row's live request inline with focus on the card. N opens it with focus on Deny.
  - Neither key answers, and Enter never answers.
  - Keys are physical (`e.code`), and the hint shows the user's layout letter.
- **Inline card.** It is the live `LiveCard` for the row's oldest pending entry, answered through the same `answerRequest`.
  - A terminal-only entry (a needs-you status with no hold) opens its one-line card, and N focuses that card, since it has no Deny.
  - On the board, the card's own focus restore goes to the board row, never to the terminal hidden underneath. This covers the card closing on Escape and an answer removing it.
  - The card's jump closes the board and jumps.
  - The card's expand closes the board and opens the drawer.
- **"+N more" line.** Its "All sessions" button closes the board and opens the command palette.
- **Row flash.** A row flashes when its status changes between snapshots into an attention state: `needs-permission`, `needs-question`, `done-unseen` or `error`.
  - Working, idle, running and ended don't flash, and neither does the first snapshot a row appears in (no flash on load or restore).
  - A muted row doesn't flash.
  - The flash lasts 600 ms (`--dur-flash`; 300 ms reduced, as CSS has it), then `flash` is cleared, so the next change can flash again.
- **Already live and kept:** the glyph bounce (needs you), the check drawn on Done, the error flash, and the count in the title and badge. They are checked live (Task 6), not rebuilt.
- **Other rules:**
  - Everything works with the keyboard alone and in forced colors.
  - A board row's accessible name stays `boardRowLabel`.
  - Tests never touch the real `~/.marshell`.
- **Commits** read `type(scope): what changed, as a sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A session closes while the pointer holds the board.** Its row stays, dimmed, from the last `RowModel` kept for it. The board must never look up a row that is gone (today `rows.find(...)!` would throw). Task 3 has a test.
2. **The texts change during a flip.** Status goes NEEDS YOU → WORKING → DONE within 400 ms: the cell restarts from what is on screen and ends on DONE, with no stale frame left. Task 1 has a test.
3. **A plain shell holding a request, and a needs-you row with no held request.**
   - The first is a gate call: the Gate calls count equals the title's count.
   - Y on the second opens its one-line card, and N doesn't throw for want of a Deny.

   Task 3 has tests.
4. **A burst of status changes.** A snapshot that brings back 10 rows at once (a core restart or a relaunch) flashes none of them. A row that goes needs-you → working → needs-you within 600 ms flashes again on the second change. Task 2 has tests.
5. **Focus on close.** Each of these leaves focus somewhere real, never `body` and never a terminal hidden under the board:
   - Escape on the board returns to the opener;
   - Enter on a row goes to the jumped-to session's terminal;
   - "All sessions" goes to the palette input;
   - an answered or Escaped inline card returns to its board row.

   Task 3 tests the focus-target choice, and Task 6 checks it live.
6. **Rows reorder under the keyboard.** The tab stop sits on session 4 (row 3) when session 2 starts needing you and moves up into Gate calls. The tab stop stays on session 4, and Enter jumps to session 4. Task 3 has a test.

---

## Execution

Tasks 1 to 4 go to Codex CLI (`codex exec`), each in its own worktree under `C:/dev/codex-wt/`, branched from `phase-4c`.
- Codex can't commit in a linked worktree, so the main thread reads each diff, runs the tests, copies the files over and commits.
- Tasks 1 and 2 share nothing and run in parallel. Task 3 consumes Task 1, and Task 4 consumes Task 3.
- When Codex is out of quota, the task goes to a `worker` (Sonnet).
- Task 5 is docs, done inline. Task 6 is the live check in the main thread, and Task 7 the review.

### Task 1: Live flipping cells and holding the board's groups

**Files:**
- Create: `src/features/departures/LiveFlipCell.tsx`
- Modify: `src/features/departures/flip.ts`, `src/features/departures/board.ts`, `src/features/departures/Board.css`
- Test: `src/features/departures/flip.test.ts`, `src/features/departures/board.test.ts`, `src/features/departures/DeparturesBoard.test.tsx`, `src/features/departures/Board.css.test.ts`

**Interfaces:**
- Produces:
  - `type FlipRun = { from: string; to: string; start: number }`.
  - `retarget(run: FlipRun | null, shown: string, to: string, now: number): FlipRun | null`, pure. `shown` is the settled text when no run is going. It returns:
    - `null` when no run is going and `to === shown`;
    - the same run when `to` is the run's own `to`;
    - otherwise a new run from the text now on screen (`flipFrame(run.from, run.to, now - run.start)`, or `shown` with no run) to `to`, starting at `now`.
  - `holdGroups(groups: { gate: BoardRow[]; others: BoardRow[] }, hold: { order: string[]; gate: string[] }): { gate: BoardRow[]; others: BoardRow[] }`: the rows in `hold.order`, each in the group `hold.gate` says. Ids not in `hold.order` are appended to the end of the group their own `gate` flag picks.
  - `LiveFlipCell({ text, reduced }: { text: string; reduced: boolean })` is its own component, so the static `FlipCell` (stories, review page, `data-frames`) is untouched and no hook sits behind an early return.
    - It keeps its own last text in a ref and never takes a `from` from the parent. Rows re-render every second while anything waits, so a `from` taken from the previous render would cut a flip short.
    - On mount it shows `text` with no flip.
    - On a new `text` it calls `retarget` and plays frames on `requestAnimationFrame` until the run settles. It cancels the frame on unmount.
  - The markup is `<span class="flip" data-live><span aria-hidden="true">{frame}</span><span class="board__sr">{text}</span></span>`.
  - Reduced: no frames. The cell sets `data-fade` for one 200 ms crossfade (`.flip[data-fade]` keyframes in `Board.css`, opacity .4 → 1), restarted on each change.

- [ ] **Step 1: Write the failing tests.**
  - `flip.test.ts`:
    - `retarget(null, "IDLE", "IDLE", 0)` is `null`;
    - `retarget(null, "IDLE", "NEEDS YOU", 5)` is `{ from: "IDLE", to: "NEEDS YOU", start: 5 }`;
    - the same target mid-run keeps the run;
    - a run NEEDS YOU → WORKING started at 0 and retargeted to DONE at 100 gives `{ from: flipFrame("NEEDS YOU", "WORKING", 100), to: "DONE", start: 100 }`;
    - that run's frame at `100 + flipDuration(9)` is "DONE".
  - `board.test.ts`:
    - `holdGroups` keeps the held order and gate membership: a row that became gate stays in `others`, and a row that stopped being gate stays in `gate`;
    - it appends a new id at the end of its own group.
  - `DeparturesBoard.test.tsx`:
    - `LiveFlipCell` with text "NEEDS YOU" renders "NEEDS YOU" inside `.board__sr`, and an `aria-hidden="true"` span holding "NEEDS YOU" (no flip on mount);
    - the existing `FlipCell` cases are unchanged.
  - `Board.css.test.ts`: `.flip[data-fade]` has an animation with a 200ms duration.

- [ ] **Step 2: Run them.** `pnpm test src/features/departures`. Expected: the new cases FAIL (no `retarget`, no `holdGroups`, no `LiveFlipCell`).

- [ ] **Step 3: Implement.** Add `retarget` to `flip.ts` and `holdGroups` to `board.ts`. Create `LiveFlipCell.tsx`, and add the fade keyframes in `Board.css` beside `.flip`.

- [ ] **Step 4: Run them again.** `pnpm test src/features/departures`. Expected: PASS, and every existing case still passes.

- [ ] **Step 5: Commit.**
  ```bash
  git add src/features/departures
  git commit -m "feat(departures): flip the board's cells live, and hold its groups under the pointer"
  ```

### Task 2: A row flashes once on an attention change

**Files:**
- Create: `src/features/attention/rowFlash.ts`, `src/features/attention/rowFlash.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (the rows given to the sidebar)

**Interfaces:**
- Produces:
  - `attentionFlashes(prev: ReadonlyMap<string, GlyphKind> | null, rows: RowModel[]): string[]`: ids whose status changed into `needs-permission`, `needs-question`, `done-unseen` or `error`, skipping muted rows and rows missing from `prev`. A `null` `prev` (the first snapshot) gives `[]`.
  - `FLASH_MS = 600`.
  - `useRowFlash(rows: RowModel[]): RowModel[]`: the same rows, each id from `attentionFlashes` set to `flash: true` for `FLASH_MS` and then cleared. A new flash for an id already flashing restarts it: clear and set again in the next frame, so the CSS animation restarts.

- [ ] **Step 1: Write the failing tests** in `rowFlash.test.ts`, on `attentionFlashes`:
  - `null` → `[]`;
  - working → needs-permission flashes;
  - working → needs-question flashes;
  - working → done-unseen flashes;
  - working → error flashes;
  - working → idle doesn't flash;
  - idle → working doesn't flash;
  - needs-permission → needs-permission (no change) doesn't flash;
  - a muted row going working → needs-permission doesn't flash;
  - a row new to `prev` doesn't flash;
  - 10 rows all new in one snapshot give `[]`.

- [ ] **Step 2: Run them.** `pnpm test src/features/attention/rowFlash.test.ts`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement** `rowFlash.ts`. In `LiveApp.tsx`, the sidebar gets `useRowFlash(taskRows)` instead of `taskRows`. Only the sidebar gets them: the board and the lane keep their own rows.

- [ ] **Step 4: Run them again.** `pnpm test src/features/attention src/features/sessions`. Expected: PASS.

- [ ] **Step 5: Commit.**
  ```bash
  git add src/features/attention/rowFlash.ts src/features/attention/rowFlash.test.ts src/features/sessions/LiveApp.tsx
  git commit -m "feat(attention): flash a sidebar row once when its session needs you, is done or errors"
  ```

### Task 3: The live board

**Files:**
- Create: `src/features/departures/LiveDepartures.tsx`, `src/features/departures/liveBoard.ts`, `src/features/departures/liveBoard.test.ts`, `src/features/departures/LiveDepartures.css`
- Modify:
  - `src/features/departures/DeparturesBoard.tsx`: `renderCard`, `hold`, `onAllSessions`, `live`, `rowsById`;
  - `src/features/approval/LiveCard.tsx`: `initialFocus`;
  - `src/features/departures/DeparturesBoard.test.tsx`;
  - `src/features/approval/LiveCard.test.tsx`, if present; otherwise the nearest LiveCard test.

**Interfaces:**
- Consumes `holdGroups`, `LiveFlipCell` and `retarget` from Task 1, and `freezeOrder` (`freeze.ts`) and `reduceBoard`/`CardFocus` (`nav.ts`) as they are.
- `DeparturesBoard` gains these optional props. With none of them, the stories render as today:
  - `live?: boolean`: the status and waiting cells render `LiveFlipCell` instead of `FlipCell`;
  - `hold?: { order: string[]; gate: string[] }`, applied with `holdGroups` after `splitGroups`;
  - `renderCard?: (rowId: string) => ReactNode`, used instead of the fixture `ApproveCard` when given. A row it returns `null` for gets no card row;
  - `onAllSessions?: () => void`, on the "All sessions" button;
  - `onRowClick?: (rowId: string) => void`;
  - the source row lookup no longer asserts: `source?.recap?.text`.
- `LiveCard` gains `initialFocus?: CardFocus`. On mount, `"title"` focuses the card element and `"deny"` focuses its Deny button, or the card when there is no Deny (a terminal-only entry). Never Allow.
- `liveBoard.ts` (pure):
  - `gateRows(rows: RowModel[], requests: ApprovalRequest[]): RowModel[]`. Each row whose tab has an entry gets a waiting status (its own needs-permission or needs-question, else needs-permission) and `waitingMs` set to the longest of its entries. Other rows are unchanged. This is the board's own copy of the rows: the sidebar's rows are not touched.
  - `heldRows(rows: RowModel[], kept: ReadonlyMap<string, RowModel>, stale: string[]): RowModel[]`: the live rows plus the kept copy of each stale id, in place of what is gone.
  - `cardFor(requests: ApprovalRequest[], rowId: string): ApprovalRequest | undefined`: the row's oldest pending entry, terminal-only included.
  - `activeIndex(shownIds: string[], activeId: string | null, lastIndex: number): number`: the index of `activeId`. If it is gone, `lastIndex` clamped to the rows shown. With no rows, 0.
  - `focusAfter(action: "escape" | "jump" | "all-sessions" | "card-closed"): "opener" | "terminal" | "palette" | "board-row"`.
- `LiveDepartures(props: { rows: RowModel[]; requests: ApprovalRequest[]; reduced: boolean; layout?: ReadonlyMap<string, string>; renderCard: (r: ApprovalRequest, focus: CardFocus, restore: () => void) => ReactNode; onJump: (rowId: string) => void; onAllSessions: () => void; onClose: () => void })` is a `<div className="departures" role="dialog" aria-modal="true" aria-label="Departures">`. It:
  - feeds `gateRows(heldRows(...), requests)` to the board;
  - measures its height with a `ResizeObserver` for `heightPx`;
  - keeps the active row as an id (`activeIndex` on each render) and acts on `reduceBoard` keys (`e.code`, preventDefault on the handled ones);
  - keeps Tab inside with `trapKey`;
  - freezes `order` and `gate` on `pointerenter` of `.board__table` and releases them on its `pointerleave`, keeping each row's last `RowModel` while frozen;
  - on mount, focuses the active row and keeps the opener's element to return to;
  - passes `renderCard` a `restore` that focuses the active board row: the card's restore on unmount, after an answer or Escape;
  - on Escape with no card open, calls `onClose`.

- [ ] **Step 1: Write the failing tests.**
  - `liveBoard.test.ts`:
    - `gateRows` turns a plain-shell row (status idle) with a held request into needs-permission, with the entry's `waitingMs`; the Gate calls count from `boardRows`/`splitGroups` then equals the distinct tabs in `requests`;
    - `gateRows` keeps needs-question for a row that already has it;
    - `heldRows` keeps a stale row's last copy at its index and drops nothing;
    - `cardFor` returns the pending entry for the tab, including a terminal-only one, and `undefined` for an answered one or none;
    - `activeIndex` follows the id when rows reorder (session 4 at index 2 moves to index 3 → 3), and falls back to the clamped last index when the id is gone;
    - `focusAfter` maps escape → opener, jump → terminal, all-sessions → palette, card-closed → board-row.
  - `DeparturesBoard.test.tsx`:
    - with `renderCard` returning a marker, the open card row shows the marker and not the fixture command;
    - with `renderCard` returning `null`, no `.board__card` row renders;
    - a `stale` id not in `rows`, passed with its kept row through `heldRows`, renders dimmed (`data-stale`) without throwing;
    - with `hold`, a row that became needs-you renders outside Gate calls with "Moves up when you move away".
  - LiveCard test: with `initialFocus: "deny"`, the markup marks Deny as the element to focus. Assert whatever ref or data attribute the implementation exposes, such as `data-initial-focus` on Deny.

- [ ] **Step 2: Run them.** `pnpm test src/features/departures src/features/approval`. Expected: the new cases FAIL.

- [ ] **Step 3: Implement** the pure helpers, the `DeparturesBoard` props, `LiveCard`'s `initialFocus` and `LiveDepartures`. In `LiveDepartures.css`, `.departures` is `position: absolute; inset: 40px 0 0 0; z-index: 20; background: #000; display: grid`, and the board fills it. Add `.departures` to the dialog list in `src/features/window/dialogStacking.css.test.ts`, so it stays above the sidebar handle.

- [ ] **Step 4: Run them again.** `pnpm test src/features/departures src/features/approval src/features/window`. Expected: PASS.

- [ ] **Step 5: Commit.**
  ```bash
  git add src/features/departures src/features/approval src/features/window/dialogStacking.css.test.ts
  git commit -m "feat(departures): add the live board, holding rows under the pointer and opening real requests inline"
  ```

### Task 4: Open it from the palette and the tray

**Files:**
- Modify:
  - `src/features/sessions/LiveApp.tsx`: the overlay union, the render, the tray state, `runFromPalette`, and tray actions;
  - `src/features/sessions/LivePalette.tsx`: add `departures` to `PALETTE_ONLY`;
  - `src/features/tray/useTray.ts`: `actionFor("departures")`, and `TrayAction`'s overlay gaining `"departures"`;
  - `src/features/tray/trayItems.ts`: the doc comment.
- Test: `src/features/sessions/LivePalette.test.tsx`, if present; otherwise a new `livePaletteCommands.test.ts`. Also `src/features/tray/useTray.test.ts`.

**Interfaces:**
- Consumes `LiveDepartures` from Task 3, `liveCard` and `answerRequest` in `LiveApp` as they are, and `pick` and `actions.focusTerminal`.
- In `LiveApp`:
  - `overlay` gains `"departures"`;
  - `runFromPalette("departures")` and the tray action `{ kind: "overlay", overlay: "departures" }` set it;
  - `trayState.departures` becomes `true`;
  - the board renders when `overlay === "departures"`:
    - `onJump`: `setOverlay(null)`, `pick(id)`, then `actions.focusTerminal()` on the next tick;
    - `onAllSessions`: `setOverlay("palette")`;
    - `onClose`: `setOverlay(null)` and restore focus;
    - `renderCard`: a `LiveCard` for the board, not `liveCard(r)`, wired like this:
      - `initialFocus` as given;
      - `onRestoreFocus` set to the board's `restore`;
      - `onJump`: close the board, then `jumpToRequest`;
      - `onExpand`: close the board, then `setDrawerId`;
      - `onAnswer`: `answerRequest`;
    - `reduced`: read from `matchMedia("(prefers-reduced-motion: reduce)")` or `document.documentElement.dataset.motion === "reduce"`.
  - The window-level Escape capture that lets an open session bar take Escape first (LiveApp's close, restart and pause bars) steps aside while `overlay === "departures"`, so Escape always reaches the board.

- [ ] **Step 1: Write the failing tests.**
  - `LIVE_COMMANDS` contains `departures`.
  - `actionFor("departures")` is `{ kind: "overlay", overlay: "departures" }`.
  - `trayItems({ ...state, departures: true })` lists "Open Departures" second, after Open Marshell. This test may exist already; keep it.

- [ ] **Step 2: Run them.** `pnpm test src/features/sessions src/features/tray`. Expected: FAIL on the first two.

- [ ] **Step 3: Implement** the wiring above.

- [ ] **Step 4: Run them again.** Same command, then `pnpm build`. Expected: PASS, and the build succeeds.

- [ ] **Step 5: Commit.**
  ```bash
  git add src/features/sessions src/features/tray
  git commit -m "feat(departures): open the board from the palette and the tray"
  ```

### Task 5: Docs

- [ ] Update `docs/a11y/screen-reader-script.md` section F:
  - F1 opens the board from the palette or the tray;
  - add F3: a row that becomes needs-you while the pointer holds the board says "Moves up when you move away", and a flipping cell reads its final text once;
  - add F4: Escape returns focus to where it was.
- [ ] Commit: `docs(a11y): add the live Departures board to the screen-reader pass`.

### Task 6: Live check

The live check runs `pnpm tauri dev` with a scratch `MARSHELL_HOME` and CDP, with plain `cmd.exe` sessions and Claude Code hook bodies posted to `/v1/hook`, as in 4b. The webview is driven over CDP only. Any desktop input is first checked for the target's process and rectangle, and asked about.
- **Opening and closing:** from the palette over CDP. For the tray, check that the menu model sent to `set_tray` lists "Open Departures", and open the board by emitting the `tray-action` event the shell would send. Picking the menu item with real input happens only if the user agrees.
- **Board content:** the board's rows match the sidebar.
- **Flips:** a status flip plays (sampled frames over CDP), and the reduced crossfade appears under `Emulation.setEmulatedMedia`.
- **Hold under the pointer:**
  - CDP `Input.dispatchMouseEvent` inside the webview only;
  - a row that becomes needs-you stays put with its note;
  - a closed session's row stays dimmed;
  - on leave, the rows settle.
- **Keys:** Y and N open the real card with the right focus, Enter jumps and closes, Escape restores focus, and "All sessions" opens the palette.
- **Row flash:** the sidebar row flashes on needs-you, Done and error, and not on working ↔ idle.
- **Choreography already live:** the bounce, the Done check, the error flash and the title count.
- **Forced colours:** emulated.

Findings go in `docs/research/2026-10-23-departures-live-check.md`.

### Task 7: Final review

Opus whole-branch review, then one fix pass, the CI command set run locally, the PR, green CI and the merge.
