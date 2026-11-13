# Side pane fixes from the 2026-11-13 hand checks: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the three bugs the user's hand checks of 6q-3a found (Esc after a palette opened from a page, Ctrl+Shift+Tab back into a page, the page during the pane's hide), the placer bug found while tracing them, and show every side-pane tab's type.

**Architecture:** Frontend only; no Rust, capability, CSP or dependency change. The terminal stops taking focus back from an overlay that restored it. Pages remember a Tab exit, and the ring follows the page store instead of `:focus-visible`. A hide holds the pane until the page has frozen as a picture. The placer keeps one waiting report through a motion. The strip renders a kind icon and splits a file name around its extension.

**Tech Stack:** React 19, TypeScript, Vitest (node, no DOM; components through `renderToStaticMarkup`), CSS with the tokens in `src/styles/`, PowerShell 7 for the live-check script.

**Spec:** the user's rulings of 2026-11-13, recorded by Task 1 as PLAN §9 "Slice 6q-2 decisions" 15 and "Slice 6q-3 decisions" 14. The evidence is in `docs/research/2026-11-10-browser-tab-live-check.md`, "The user's hand checks (2026-11-13)", findings 24 to 29.

## Global Constraints

- Branch `pane-fixes`, cut from `origin/main` at `3824d4b1`, in the worktree `.claude/worktrees/resize-fixes`. Ledger: `.superpowers/sdd/2026-11-13-pane-fixes/progress.md`.
- Every UI change works with the keyboard alone and in forced colors. A `forced-colors` rule uses system colors only. Colors come from the tokens; there are no hex values in a stylesheet (`.claude/rules/ui.md`).
- No boxed outlines: `noBoxedOutlines.test.ts` must stay green. A focus ring is an outline, which is allowed.
- Read the good-css skill before writing CSS declarations.
- Vitest runs in node with no DOM: put the rules in pure functions and test those; components are tested with `renderToStaticMarkup`.
- Frontend checks: `pnpm test <file>` per task; `pnpm test` and `pnpm build` before the PR. The PR runs what CI runs (`marshell-ship`).
- Commit messages read `type(scope): what changed, as a sentence.` and end with the session's trailers.
- Tests that fail first: each fix's new test is run and seen failing before the fix.

## Review Focus

1. **A hide undone during its hold or its fold** (Ctrl+Shift+Q twice quickly): the page must come back live and take clicks, never stay a picture or stay hidden. Task 6 pins it with `holdOnChange(true, …)`; the live check presses it twice.
2. **Esc from a palette opened from the terminal, and closing an overlay that restores nothing** (settings, the launcher after a start): the keyboard must still land in the terminal. Task 2 pins it with `focusOnUnblock`; the live check runs both.
3. **Shift+Tab off a page's first stop, then Ctrl+Shift+Tab:** the ruling's "tabbed out of" covers both directions, so the page starts at its first stop. Task 3 has the test.
4. **A click into a page after a Tab exit, then Esc and Ctrl+Shift+Tab:** the page was entered again, so it restores where the click left it. Task 3 has the test (`markEntered` clears the mark).
5. **A window resize while a page shows:** Rust places the page by the rule without asking the app, so the 2 px gutter must be in the rule too, or the page jumps 2 px at the end of every resize. Task 4 has the test: the rule's insets come from the inset box.

---

### Task 1: Record the checks, the rulings and the live-check homes (main thread)

**Files:**
- Modify: `docs/research/2026-11-10-browser-tab-live-check.md` (a new section after finding 23)
- Modify: `docs/research/2026-11-10-side-pane-live-check.md` (a new section at the end)
- Modify: `docs/PLAN.md` §9 (6q-2 decision 15 after 14; 6q-3 decision 14 after 13)
- Modify: `.claude/skills/marshell-live-check/live-check.ps1` (after `home` is made, near line 196) and its help text (lines 19–22)
- Modify: `.claude/skills/marshell-live-check/SKILL.md` (the sentence on `start`'s `home`)

- [ ] **Step 1: Write the browser-tab note's section** "The user's hand checks (2026-11-13)". Use a table: Alt+Tab with the keyboard in a page, with the alert; Shift+Tab inside a page; Tab out and Ctrl+Shift+Tab back; Esc after a palette opened from a page; the title-bar button over a page; window narrowing and motion; Quit from the tray then the sign-in; not run (the request drawer, a real 125% or 150% display). Then findings 24 to 29: the toast raised under PowerShell's app id in a dev build, with sound and a red badge; C and its cause; B7 and its two causes; D and its cause, with the measured times (show(null) sent at 204 ms, done at 231 ms; capture 27 ms, hidden at 52 ms); the placer bug (32 measures, 0 sent, a stale 0×0); and pnpm's "exit code -1" at Quit. Each says "Fixed in this branch" until Task 9 adds the commits.
- [ ] **Step 2: Add the side-pane note's section** "The user's hand checks (2026-11-13)": Browse… (with the scratch home's "Desktop is unavailable"), an Explorer drop on the pane, a drop with the launcher open, and the 8 px scrollbar under `sample.txt`. All pass.
- [ ] **Step 3: Write PLAN §9.** 6q-2 decision 15 records the tab type: middle-shortened file names that keep the extension, four icons, and the globe replacing decision 10's site icon in the strip. 6q-3 decision 14 records the user's answers of 2026-11-13: Ctrl+Shift+Tab into a page that was tabbed out of starts at its first stop, otherwise it restores; a ring just outside the page; the hide folds a picture; the placer fix. Its "Decided, not asked" lists: the ring shows whenever the page has the keys, a click included; the gutter is 2 px; the hold's cap is 150 ms; Esc returns focus to what opened an overlay, and the terminal takes it only when nothing has it.
- [ ] **Step 4: Make `start` create `Desktop`, `Documents` and `Downloads` in `home`.** Use `New-Item -ItemType Directory` for each, after `$agyHome`. Update the help text and SKILL.md to say so.
- [ ] **Step 5: Check the script parses.** Run: `pwsh -NoProfile -Command "$null = [System.Management.Automation.Language.Parser]::ParseFile('.claude/skills/marshell-live-check/live-check.ps1', [ref]$null, [ref]$e); $e.Count"`. Expected: `0`.
- [ ] **Step 6: Commit** `docs(research): record the user's hand checks of 2026-11-13 and the fixes they asked for.`

### Task 2: Esc from an overlay keeps the focus it restored (bug C)

**Builder:** worker (Sonnet). **Files:**
- Create: `src/features/terminal/terminalFocus.ts`, `src/features/terminal/terminalFocus.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx:56` (props), `:94` (signature), `:416-428` (the focus effect)
- Modify: `src/features/sessions/LiveApp.tsx:1739`

**Interfaces:**
- Produces: `focusOnChange(s: { selected: boolean; blocked: boolean }): boolean`, `focusOnUnblock(s: { selected: boolean; onNothing: boolean }): boolean`, and `TerminalView`'s new prop `focusBlocked?: boolean` (default `false`).

**Cause:** LiveApp passes `autoFocus={!linkHeld && !launcherOpen && !reopening && overlay === null && review === null && row.id === focusedId}`. When an overlay closes, `autoFocus` turns true, and TerminalView's effect focuses the terminal in the same commit. LivePalette's cleanup has just restored focus to its opener (`LivePalette.tsx:102-108`), so the restore is overridden.

- [ ] **Step 1: Write the failing tests** in `terminalFocus.test.ts`:
  - `focusOnChange({ selected: true, blocked: false })` is `true`; with `blocked: true`, or with `selected: false`, it is `false`.
  - `focusOnUnblock({ selected: true, onNothing: true })` is `true` ("an overlay that restored nothing leaves the keyboard to the terminal").
  - `focusOnUnblock({ selected: true, onNothing: false })` is `false` ("an overlay that restored focus keeps it there").
  - `focusOnUnblock({ selected: false, onNothing: true })` is `false`.
- [ ] **Step 2: Run** `pnpm test src/features/terminal/terminalFocus.test.ts`. Expected: FAIL (module not found).
- [ ] **Step 3: Implement both functions,** each one line, with a doc comment that states the rule.
- [ ] **Step 4: Wire TerminalView.** `autoFocus` now means "this is the selected session's terminal". The effect at `:418` focuses when `focusOnChange({ selected: autoFocus, blocked })`; read `blocked` through `useEffectEvent`, so a block that lifts doesn't re-run that effect. A new effect, declared after it, runs only when `focusBlocked` goes from `true` to `false` (keep the last value in a ref). It focuses when `focusOnUnblock({ selected: autoFocus, onNothing: document.activeElement === null || document.activeElement === document.body })`.
- [ ] **Step 5: Wire LiveApp `:1739`:** `autoFocus={row.id === focusedId}` and `focusBlocked={linkHeld || launcherOpen || reopening || overlay !== null || review !== null}`. The shell panel's TerminalView (`:1166`, `autoFocus={false}`) is unchanged.
- [ ] **Step 6: Run** `pnpm test src/features/terminal src/features/sessions`. Expected: PASS.
- [ ] **Step 7: Commit** `fix(terminal): an overlay that put focus back on what opened it keeps it there.`

### Task 3: Ctrl+Shift+Tab into a page that Tab left starts at its first stop (bug B7, keys)

**Builder:** worker (Sonnet). **Files:**
- Modify: `src/features/browser/pageFocus.ts`, `src/features/browser/pageFocus.test.ts`
- Modify: `src/features/browser/useBrowserKeys.ts:69-78` (the `focus` and `focus-out` handlers)
- Modify: `src/features/pane/focusPane.ts:31` (`browser.focus(page, "programmatic")`)

**Interfaces:**
- Produces: `markTabbedOut(key: string): void`, `markEntered(key: string): void`, `entryReason(key: string): Extract<FocusReason, "next" | "programmatic">`, all in `pageFocus.ts`, over a module-level `Set<string>`.

**Cause:** `focusPane` always asks for `"programmatic"`, and WebView2 puts the keyboard back where the page last had it. After Tab left the page, that is its `BODY`, so typed keys go nowhere.

- [ ] **Step 1: Write the failing tests** in `pageFocus.test.ts`, `describe("entryReason")`. Each test clears its keys with `markEntered` afterwards.
  - "a page never left by Tab is restored": `entryReason("a")` is `"programmatic"`.
  - "a page left by Tab starts at its first stop": after `markTabbedOut("a")`, `entryReason("a")` is `"next"`, and `entryReason("b")` is still `"programmatic"`.
  - "entering the page again clears it": after `markTabbedOut("a")` then `markEntered("a")`, the answer is `"programmatic"`.
- [ ] **Step 2: Run** `pnpm test src/features/browser/pageFocus.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement the three functions.**
- [ ] **Step 4: Wire them.** In `useBrowserKeys`, `focus-out` calls `markTabbedOut(key)` in either direction, and `focus` with `focused: true` calls `markEntered(key)`. In `focusPane`, the call becomes `browser.focus(page, entryReason(page))`.
- [ ] **Step 5: Run** `pnpm test src/features/browser src/features/pane`. Expected: PASS.
- [ ] **Step 6: Commit** `fix(browser): Ctrl+Shift+Tab into a page that Tab left puts the keyboard on its first stop.`

### Task 4: A ring just outside a page that has the keys (bug B7, ring)

**Builder:** worker (Sonnet), who reads the good-css skill first. **Files:**
- Modify: `src/features/browser/usePageSlot.ts` (`measure`, lines 82-90)
- Modify: `src/features/browser/usePageSlot.test.ts`
- Modify: `src/features/browser/BrowserTab.tsx` (`BrowserTabViewProps`, the slot at `:50`, `BrowserTab` at `:101-105`)
- Modify: `src/features/browser/BrowserTab.css:109-114` and its forced-colors block
- Test: the BrowserTab markup test beside it (`BrowserTab.test.tsx`; create it if there is none)

**Interfaces:**
- Produces: `PAGE_GUTTER = 2` and `insetRect(r: DOMRectLike, by: number): DOMRectLike`, exported from `usePageSlot.ts`, and `BrowserTabViewProps.keys: boolean`.

**Cause:** the ring is `.browser-tab__slot:focus-visible` with `outline-offset: -2px`. While a page has the keys, the app's document has no focus, so the selector never matches (seen live: `hasFocus: false`, `:focus` false). Even when it does match, the page covers the slot's whole box.

- [ ] **Step 1: Write the failing tests.**
  - `usePageSlot.test.ts`, "the page sits 2 px inside its slot": `insetRect({ x: 10, y: 20, width: 100, height: 50, right: 110, bottom: 70 }, 2)` equals `{ x: 12, y: 22, width: 96, height: 46, right: 108, bottom: 68 }`.
  - "a slot no wider than the gutter gives no box": `insetRect` of a 3 × 50 box has `width: 0`, and `slotReport` of that box reports the zero rect and `rule: null`.
  - Review Focus 5, "the rule keeps the gutter": `slotReport(insetRect(slot, PAGE_GUTTER), win, inputs, false)` has `rule.top` and `rule.bottom` 2 px larger than the report of the same box without the inset, and the first piece's `x` 2 px larger (left to right). Use the `inputs` fixture this test file already has.
  - Markup test, "the slot says when its page has the keys": `renderToStaticMarkup(<BrowserTabView … keys />)` contains `data-keys=""`, and with `keys={false}` it doesn't.
- [ ] **Step 2: Run** `pnpm test src/features/browser/usePageSlot.test.ts src/features/browser/BrowserTab.test.tsx`. Expected: FAIL.
- [ ] **Step 3: Implement.** `insetRect` clamps the width and height at 0. `measure` passes `insetRect(slot, PAGE_GUTTER)` to `slotReport`. `BrowserTab` passes `keys={useFocusedPage() === pageKey}`, and the slot gets `data-keys={keys || undefined}`.
- [ ] **Step 4: CSS.** `.browser-tab__slot[data-keys]` draws the same ring as `:focus-visible` (2 px `var(--focus, var(--accent))`, offset −2 px, so it lies in the gutter). Keep the `:focus-visible` rule for a slot with no page. In forced colors the ring is `Highlight`.
- [ ] **Step 5: Run** `pnpm test src/features/browser src/styles`. Expected: PASS, `noBoxedOutlines.test.ts` included.
- [ ] **Step 6: Commit** `fix(browser): a page that has the keys shows a ring in a 2 px gutter around it.`

### Task 5: The placer sends during the app's own motion and forgets an unmounted slot (main thread)

**Files:**
- Modify: `src/features/browser/placing.ts`, `src/features/browser/placing.test.ts`
- Modify: `src/features/browser/usePageSlot.ts:184-215` (the slot effect's cleanup)

**Interfaces:**
- Produces: the placer gains `cancel(): void`.

**Cause:** at a still window size, `process` starts a paint wait with a new token, and the next frame's report replaces that token before the wait ends. During a motion no report is ever sent; the one left at the end measures a slot that has unmounted, and goes out as 0 × 0.

- [ ] **Step 1: Write the failing tests** in `placing.test.ts`:
  - "a report each frame at one size still goes out once a paint": with `paint` a deferred, call `report(m1)` once and flush, so `last` is set. Then call `report(m2)`, then `report(m3)` before the paint resolves. Resolve it and flush: `send` has been called twice, and the second call carries `m3`'s report (measured when sent).
  - "cancel drops a report waiting for a paint": `report` at the same size, `cancel()`, resolve the paint: no second send.
  - "cancel drops a report queued behind one in flight": the first send is a deferred; `report(m2)`, `cancel()`, resolve the send: `m2` is never called.
- [ ] **Step 2: Run** `pnpm test src/features/browser/placing.test.ts`. Expected: the three new tests FAIL.
- [ ] **Step 3: Implement.** A report that arrives while a paint wait runs only replaces the measure that wait will use; it doesn't restart the wait. A report at a new size still drops the wait and goes at once. One report stays in flight, as before. `cancel()` clears the waiting and the queued report.
- [ ] **Step 4: Call `placer.current!.cancel()`** in the slot effect's cleanup.
- [ ] **Step 5: Run** `pnpm test src/features/browser`. Expected: PASS.
- [ ] **Step 6: Commit** `fix(browser): the page placer sends a report each paint while the layout moves, and none for a slot that is gone.`

### Task 6: Hiding the pane folds the page as a picture (bug D)

**Builder:** worker (Sonnet), `effort: high` (timing between React commits). **Files:**
- Create: `src/features/window/paneHold.ts`, `src/features/window/paneHold.test.ts`
- Modify: `src/features/browser/usePageSlot.ts` (export `pageShown`)
- Modify: `src/features/window/MainWindow.tsx:330-360`

**Interfaces:**
- Consumes: `useCoverPage(on)` from `src/features/browser/pageCover.ts`.
- Produces: `pageShown(): boolean` in `usePageSlot.ts` (`shownKey !== null`: the app has a page shown). In `paneHold.ts`: `HOLD_CAP_MS = 150`, `holdOnChange(wanted: boolean, pageShown: boolean): boolean`, `holdEnds(pageShown: boolean, elapsed: number): boolean`, and `usePaneHold(wanted: boolean): boolean`.

**Cause:** the hide motion cuts `.window__pane`'s track, while the kept pane keeps its settled width (`MainWindow.tsx:351-362`). So the slot's box never changes and the page stays full size above the folding pane. The page is hidden only when the kept pane unmounts after the motion (show(null) sent 204 ms after the click, done at 231 ms).

**The fix (the user's answer, 2026-11-13):** a hide first covers the page, which freezes it (capture, then the picture, then the page hidden: 52 ms measured). The pane is held shown until then, or for at most 150 ms. Then the fold runs with the picture inside it, and the cover stays until the pane unmounts.

- [ ] **Step 1: Write the failing tests** in `paneHold.test.ts`:
  - "a hide holds while a page is on screen": `holdOnChange(false, true)` is `true`.
  - "a hide with no page on screen doesn't hold": `holdOnChange(false, false)` is `false`.
  - Review Focus 1, "a show ends any hold at once": `holdOnChange(true, true)` and `holdOnChange(true, false)` are `false`.
  - "the hold ends when the freeze has hidden the page": `holdEnds(false, 10)` is `true`.
  - "the hold ends at the cap": `holdEnds(true, 149)` is `false`, and `holdEnds(true, 150)` is `true`.
- [ ] **Step 2: Run** `pnpm test src/features/window/paneHold.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement the pure functions and `usePaneHold`.** The hook decides during render, so no frame draws the pane hidden before the hold starts. Use React's "adjust state when a prop changes" pattern: keep the previous `wanted` in state, and on a change set `held` to `holdOnChange(wanted, pageShown())`. While `held` is true, an effect polls once per animation frame from `performance.now()` at the start and clears `held` when `holdEnds(pageShown(), elapsed)`. It cancels its frame on cleanup. The hook returns `held`.
- [ ] **Step 4: Wire MainWindow.** `const paneWanted = paneShown && !viewing && !empty;` and `const paneHeld = usePaneHold(paneWanted);`. `paneGeometry`'s `shown` and `layoutValue.inputs.shown` become `paneWanted || paneHeld`. The toggle's `aria-expanded` and its tooltip keep `paneWanted`, so the button answers at once. Add `useCoverPage(paneHeld || (paneMotion.moving && !paneOn));`.
- [ ] **Step 5: Run** `pnpm test src/features/window src/features/browser`. Expected: PASS.
- [ ] **Step 6: Commit** `fix(window): hiding the side pane freezes its page first, so the page folds with the pane.`

### Task 7: Every side-pane tab shows its type

**Builder:** worker (Sonnet), who reads the good-css skill first. **Files:**
- Create: `src/features/pane/tabType.ts`, `src/features/pane/tabType.test.ts`
- Modify: `src/features/pane/SidePane.tsx` (`StripTab` at `:9`, the globe at `:32-38`, the tab's content at `:161-162`)
- Modify: `src/features/pane/SidePane.css` (`.side-pane__icon` and `.side-pane__title`, `:64-80`), `src/features/pane/SidePane.test.tsx`
- Modify: `src/features/sessions/LiveApp.tsx:1366` (drop `usePageIcons` and its import) and `:1793` (build `kind`)

**Interfaces:**
- Consumes: `kindOf(path): PreviewKind` from `src/features/preview/kinds.ts`.
- Produces: `type TabKind = "web" | "text" | "pdf" | "image"`, `tabKind(browser: boolean, path: string): TabKind`, `splitName(name: string): { head: string; tail: string }`, and `StripTab = { id: string; title: string; path: string; kind: TabKind }` (replacing `icon`).

- [ ] **Step 1: Write the failing tests.**
  - `tabType.test.ts`: `tabKind(true, "https://example.com/a.pdf")` is `"web"`. `tabKind(false, …)` is `"pdf"` for `C:\a\b.pdf`, `"image"` for `x.png`, and `"text"` for `x.ts`, `x.txt`, `x.md`, `x.js`, `x.ps1`, `x.html` and `Makefile`.
  - `splitName("sample-long-name.txt")` is `{ head: "sample-long-name", tail: ".txt" }`. `"archive.tar.gz"` gives `{ head: "archive.tar", tail: ".gz" }`. `".gitignore"`, `"README"`, `"notes."` and `"a.backup-old"` each give `tail: ""` with the whole name as `head`. An extension is 1 to 8 letters or digits after the last dot, and the dot is not the first character.
  - `SidePane.test.tsx`: a `web` tab renders the globe and no `<img>`; `text`, `pdf` and `image` tabs each render their own icon (assert on a `data-kind` attribute on the icon's `svg`). A file tab named `sample-long-name.txt` renders `side-pane__name-head` holding `sample-long-name` and `side-pane__name-tail` holding `.txt`. A web tab titled `Example.com` is not split.
- [ ] **Step 2: Run** `pnpm test src/features/pane`. Expected: FAIL.
- [ ] **Step 3: Implement `tabType.ts`**, then the strip. Every tab draws a 14 px icon in the globe's style (viewBox `0 0 14 14`, stroke `currentColor` at 1.1, round caps and joins, no fill, `aria-hidden`, `data-kind`): `web` the globe; `text` a page with a folded corner and three lines; `pdf` the same page with a short band across its lower half; `image` a frame with a small circle and a hill line. A file tab's title is two spans, a head that shortens with an ellipsis and a tail that never shrinks, so `sample-long-name.txt` shows as `sam….txt`. A web tab's title shortens at its end, as now. The text the tab is read by is the whole name.
- [ ] **Step 4: Build `kind` in LiveApp** with `tabKind(isBrowserTab(t), currentEntry(t).path)`, and remove the `pageIcons` lookup and the `usePageIcons` call. The page store keeps its icons; nothing else changes there.
- [ ] **Step 5: Run** `pnpm test src/features/pane src/features/sessions src/styles`. Expected: PASS.
- [ ] **Step 6: Commit** `feat(pane): every side-pane tab shows its type, and a long file name keeps its extension.`

### Task 8: Live check (main thread)

Run it with the `marshell-live-check` skill, from this worktree, on a new scratch with the loopback test site (`site.mjs`, as in the hand checks). Record the results in the browser-tab note's 2026-11-13 section.

- [ ] **Step 1: C.** Open the palette from a page and press Esc: the keyboard is back in the page (the site logs a key). Open it from the terminal and press Esc: the terminal. Close settings: the terminal. Start a session from the launcher: its terminal.
- [ ] **Step 2: B7.** Tab past the page's last stop, then Ctrl+Shift+Tab: the site logs `focusin` on its first stop, and a typed key reaches it. Shift+Tab off the first stop, then Ctrl+Shift+Tab: the first stop. Click the second box, press Esc, then Ctrl+Shift+Tab: the second box. The ring is drawn while the page has the keys (a screenshot), and in emulated forced colors.
- [ ] **Step 3: D.** Hide with the title-bar button: the log shows `capture`, then `show(null)`, then the motion; no `show(null)` after it. Hide and show again within 100 ms: the page is live and takes a click. Do the same with a file tab: no hold. The user judges the fold's look by eye.
- [ ] **Step 4: Placer.** Expand the sidebar while a page shows: reports go out during the motion (log `browser.place`), one in flight, and there is no 0 × 0 report after a hide.
- [ ] **Step 5: Tab type.** A long `.txt` name shows `…` before `.txt`; a PDF, an image and a page show their icons; forced colors emulated.
- [ ] **Step 6:** Stop the check by the skill's rules and check `~/.claude/settings.json`'s modified time.

### Task 9: Review and ship (main thread)

- [ ] **Step 1:** Fill in each finding's fix commit in the research note and in PLAN §9 where they're named.
- [ ] **Step 2:** The whole-branch review on Opus (`marshell-reviewer`) with this plan's Review Focus, then one fix pass.
- [ ] **Step 3:** `marshell-ship`: the CI checks, the PR ("What it does", "Live check", "Known limits", "Checks"), green CI and the merge. CI flakes #112 and #115 get one rerun each, named in the PR.
- [ ] **Step 4:** Update the memory notes and delete the ledger.
