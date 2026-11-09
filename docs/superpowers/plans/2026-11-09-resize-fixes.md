# Resize fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** While the window or a divider resizes, the terminal's text doesn't flicker and no white strip shows at the window's moving edge.

**Architecture:** Two independent fixes, both judged by eye in the 6q-0 spike (S12 finding 23, checks 10a and 10b). The terminal fits inside its `ResizeObserver` callback and, when the grid changed, draws its rows in the same frame through xterm 6.0's private `RenderService._renderRows`, which is what upstream PR #5529 does from 6.1 on. The main window's native background follows the app's computed background: the UI reads it, sends three bytes to a Tauri command, and Rust calls `Window::set_background_color`.

**Tech Stack:** React 19, TypeScript, Vitest (node, no DOM), xterm.js 6.0.0 with the WebGL addon 0.19.0, Tauri 2.12 (`src-tauri`, Rust).

**Spec:** `docs/PLAN.md` §9, "Slice 6q-0 decisions" 7, 8 and 10. Evidence: `docs/spikes/S12-browser-pane.md` finding 23 and "What 6q-2 and 6q-3 take from this", "Drawing during a resize".

## Global Constraints

- **No dependency changes.** xterm stays on 6.0.0 (decision 10): PR #5529 is only in the 6.1.0 betas (`latest` is 6.0.0, `beta` is 6.1.0-beta.337 on 2026-11-09).
- **No changes to `src-tauri/tauri.conf.json`, the CSP or `src-tauri/capabilities/`.** The window's colour is set at runtime only (decision 10); the moment before the UI loads stays as it is.
- **The private call fails loudly (S12 finding 23).** A missing `_renderRows` is a `console.error` once and the old next-frame fit, never a silent no-op, and a test fails CI when the installed xterm no longer has the name or is no longer 6.0.x.
- **No hex colours in the front end.** The colour sent to Rust is read from the page's computed background, so light themes, the contrast setting and forced colours match.
- **`src-tauri` stays thin.** The command validates nothing beyond its type (`[u8; 3]`) and sets the calling window's background.
- Rust: `cargo test -p marshell-app -j 4`, `cargo fmt --all` before each Rust commit. Front end: `pnpm test <file>`, `pnpm build`.
- The spike's code (`phase-6q-0`) is a reference only. Nothing named `spike` reaches this branch.

## Review Focus

- **A resize that lands while a program holds synchronized output (DEC mode 2026).** `_renderRows` buffers instead of drawing, so the cleared canvas may stay blank until the program ends its update. Expected: no blank frame the person can see with Claude Code's or Codex's TUI open. Task 3 measures it; Task 4 exists only if it fails.
- **A fractional display scale (125%, 150%).** The WebGL addon's own device-pixel observer can set the canvas's size a second time in the same frame, which clears it again after the draw. Expected: no blank frame. Task 3 measures it; Task 4 exists only if it fails.
- **A hidden terminal.** A hidden host measures near empty; `fitIfShown` must stay the gate, and nothing draws for a grid that didn't change. Task 1's tests pin "no grid change, no draw".
- **The DOM renderer** (WebGL unavailable or its context lost). The private call exists there too and draws the rows directly. Expected: no error, no behaviour change. Task 3 doesn't cover it; record it under Known limits.
- **The theme changes while the app runs** (the Appearance setting, the system's scheme, the contrast setting, forced colours). Expected: the window's background follows within the same moment. Task 2's tests pin the send-on-change logic; Task 3 checks a light and a dark theme in the app.

---

### Task 1: The terminal fits and draws in the same frame

**Files:**
- Create: `src/features/terminal/syncRedraw.ts`
- Test: `src/features/terminal/syncRedraw.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx` (the `ResizeObserver` block, about lines 267–272)

**Interfaces:**
- Produces:
  - `type Redraw = (start: number, end: number) => void`
  - `privateRedraw(term: unknown): Redraw | null` — xterm's `_core._renderService._renderRows`, bound to its service; `null` when any link is missing or it isn't a function.
  - `fitAndRedraw(term: { cols: number; rows: number }, fit: () => void, redraw: Redraw): void` — runs `fit`, and when `cols` or `rows` changed calls `redraw(0, term.rows - 1)` with the new grid.
  - `reportMissingRedraw(log?: (message: string) => void): void` — logs once per page load: `Marshell: xterm no longer has RenderService._renderRows; the terminal falls back to the next-frame fit and may flicker while resizing.`

- [ ] **Step 1: Write the failing tests** in `syncRedraw.test.ts`:
  - `privateRedraw` returns a function that calls the service's method with `this` set to the service (a fake `{ _core: { _renderService: { rows: [], _renderRows(s, e) { this.rows.push([s, e]) } } } }`).
  - `privateRedraw` returns `null` for `{}`, for `{ _core: {} }`, and when `_renderRows` isn't a function.
  - `fitAndRedraw` draws `(0, 39)` when `fit` changes the fake from 80x24 to 100x40; draws when only `cols` changes; draws nothing when `fit` changes nothing.
  - `reportMissingRedraw` calls its log once across two calls (reset through `vi.resetModules()` or an exported test reset).
  - The installed xterm: `package.json`'s version matches `/^6\.0\./`, and both `lib/xterm.js` and `lib/xterm.mjs` contain `_renderService` and `_renderRows(`. Failure message: `xterm changed: if this release carries PR #5529 (6.1 or later), delete syncRedraw's private call and let xterm draw; otherwise find the new name`.
- [ ] **Step 2:** `pnpm test src/features/terminal/syncRedraw.test.ts` — fails, the module doesn't exist.
- [ ] **Step 3: Implement `syncRedraw.ts`.** The header comment names PR #5529, the three S12 warnings, and when to delete the file.
- [ ] **Step 4: Use it in `TerminalView.tsx`.** After `loadRenderer(...)`, `const redraw = privateRedraw(term)`; `reportMissingRedraw()` when it is `null`. The observer's callback: with `redraw`, `fitAndRedraw(term, fitIfShown, redraw)` at once; without it, today's `cancelAnimationFrame` + `requestAnimationFrame(fitIfShown)`. The other fits (first fit, font change, shown again) don't change.
- [ ] **Step 5:** `pnpm test src/features/terminal` and `pnpm build` pass.
- [ ] **Step 6: Commit** `fix(terminal): the terminal fits and draws in the same frame while it resizes`.

### Task 2: The main window's background follows the app's

**Files:**
- Create: `src-tauri/src/window_bg.rs`
- Modify: `src-tauri/src/main.rs` (`mod window_bg;`, the handler list)
- Create: `src/features/appearance/windowBackground.ts`
- Test: `src/features/appearance/windowBackground.test.ts`
- Modify: `src/features/appearance/AppearanceProvider.tsx`

**Interfaces:**
- Produces (Rust): `#[tauri::command] pub fn set_window_background(window: tauri::Window, rgb: [u8; 3]) -> Result<(), String>` — `window.set_background_color(Some(Color(r, g, b, 255)))`.
- Produces (TypeScript):
  - `type Rgb = [number, number, number]`
  - `backgroundSync(read: () => Rgb | null, send: (rgb: Rgb) => Promise<void>): () => void` — each call reads; sends when the colour differs from the last one sent; a failed send is forgotten so the next call sends again; `null` sends nothing.
  - `readPageBackground(doc?: Document): Rgb | null` — the body's computed `background-color` painted on a 1x1 2D canvas and read back as bytes (the tokens are `oklch()`, which a computed style doesn't give as `rgb()`); `null` when there is no 2D context.
  - `watchWindowBackground(): () => void` — syncs now and on: a change of `data-theme`, `data-contrast` or `data-accent` on the root, and a change of `(prefers-color-scheme: dark)`, `(prefers-contrast: more)` or `(forced-colors: active)`. Returns its cleanup. The send is `invoke("set_window_background", { rgb })`.

- [ ] **Step 1: Write the failing tests** for `backgroundSync` in `windowBackground.test.ts`: the first call sends; the same colour twice sends once; a changed colour sends again; after a rejected send the same colour is sent again; `null` sends nothing.
- [ ] **Step 2:** `pnpm test src/features/appearance/windowBackground.test.ts` — fails, the module doesn't exist.
- [ ] **Step 3: Implement `windowBackground.ts`.**
- [ ] **Step 4: Call `watchWindowBackground()`** from an effect in `AppearanceProvider` (mount only; it returns the cleanup).
- [ ] **Step 5: Add `window_bg.rs`** and register `window_bg::set_window_background` in `main.rs`.
- [ ] **Step 6:** `pnpm test src/features/appearance`, `pnpm build`, `cargo fmt --all`, `cargo clippy -p marshell-app --all-targets -- -D warnings` and `cargo test -p marshell-app -j 4` pass.
- [ ] **Step 7: Commit** `fix(window): the main window's background follows the app's theme`.

### Task 3: Live check

Run with the `marshell-live-check` skill, on scratch homes. Findings go in `docs/research/2026-11-09-resize-fixes-live-check.md`.

- [ ] **Step 1: Measure blank frames over CDP.** An injected script records each canvas size set (a clear) and each WebGL draw per animation frame, and counts frames that end cleared. Resize the window in steps. Run it with a PowerShell session, with Claude Code's TUI and with Codex's TUI, at device scale 1, 1.25 and 1.5.
- [ ] **Step 2: Prove the measurement sees the flicker.** With a temporary, never-committed local edit that forces the fallback path, the same run must count blank frames. Revert the edit.
- [ ] **Step 3: The window's background.** Read the window's background back after a theme switch (dark, light), and check the strip on a bottom-edge resize.
- [ ] **Step 4: The user's hand check.** The user drags the window's edges, the sidebar's divider and the preview's divider with a shell and with an agent TUI open, in a dark and a light theme, and judges the flicker and the strip.
- [ ] **Step 5: Write the findings and commit** `docs(research): the resize fixes' live check`.

### Task 4: Only if Task 3 finds blank frames

- **Under synchronized output:** ask the user, with the numbers, whether a mid-update draw (the renderer's own `renderRows`, one private level deeper) is better than a blank frame.
- **At a fractional scale:** a second `ResizeObserver` on the WebGL canvas (`device-pixel-content-box`), created after the addon's own, draws once more in the same frame.

Each gets its own test in `syncRedraw.test.ts` and a rerun of Task 3's measurement.

**Outcome (2026-11-09):** Task 3 found both, so both were built (decision 10's last two bullets).
- `fitAndRedraw` became `syncRedraw(term, fit, redraw, canvas): { fit(), deviceResized() }`. `TerminalView` sends every fit through `fit()` (the observer's, the first one, a font change, a tab shown again) and calls `deviceResized()` from its own device-pixel observer.
- `privateRedraw` calls `_renderService._renderer.value.renderRows` while `term.modes.synchronizedOutputMode` is true, and `_renderRows` otherwise.

**Added after the hand check (2026-11-10):** the user saw Codex's text jump while the top or bottom edge moved, and chose to fix it here (decision 10's last bullet).
- Create: `src/features/terminal/topAnchor.ts`, test `topAnchor.test.ts` (it runs the installed xterm with no DOM, including a control test that fails once xterm keeps the top rows itself).
- `keepFullScreenTop(term: unknown, next: { cols: number; rows: number } | undefined): boolean` — before a resize that loses rows, moves the alternate screen's private cursor (`_core._bufferService.buffers.alt.y`) onto the new last row; false only when it was needed and the field is gone.
- `topAnchoredFit(term, fit, missing?): () => void` — anchors for `fit.proposeDimensions()`, then fits. `TerminalView` uses it for every fit, with or without the same-frame draw.

### Task 5: Review and ship

- [ ] One `/code-review` pass on the branch, then a fix pass.
- [ ] `marshell-ship`: the checks CI runs, the PR ("What it does", "Live check", "Known limits", "Checks"), a green CI, the merge.
- [ ] Known limits to carry: each resize step draws the viewport twice (as upstream does); the history player and the scripted terminal keep their own fits; the window is white until the UI has loaded.
