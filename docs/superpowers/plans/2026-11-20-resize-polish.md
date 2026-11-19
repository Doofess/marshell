# Resize Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish what the resize fixes left: the shell panel resizes in whole rows in the window's own frame, a pane that covers the terminal never leaves the keyboard in it, the divider keeps the keyboard, and a spike says whether the window buttons can stop trailing.

**Architecture:** The app's webview is always drawn a frame or more late after a resize (S12 finding 23), so the slice removes the lag the app adds itself (the window's measure reaching React a frame late) and, for the window buttons, tries a native child of fixed size that Windows moves with the frame. The spike is a gate judged by the user's eye; its code stays on a branch that is never merged. Everything else is frontend, each change with a pure function that carries its test.

**Tech Stack:** React 19, TypeScript, xterm.js 6 (`@xterm/addon-fit` 0.11), Vitest (node, no DOM); Tauri 2 with `windows-sys` 0.61 for the spike only.

**Spec:** `docs/PLAN.md` §9, "Resize-polish slice decisions (2026-11-20)", with `docs/spikes/S12-browser-pane.md` findings 22 and 23, `docs/research/2026-11-09-resize-fixes-live-check.md` finding 6 and `docs/research/2026-11-10-side-pane-live-check.md` findings 4 and 5.

## Global Constraints

- Branch `resize-polish`, cut from `origin/main` (cbb813a4). The spike's code lives only on `resize-polish-spike`, pushed and never merged.
- No new crate or package. No change to `src-tauri/tauri.conf.json`, `tauri.windows.conf.json` or `src-tauri/capabilities/` without asking the user first (`SCOPE.md`).
- Every UI change works with the keyboard alone and in forced colors. `Announcer` is the only live region.
- Copy is exact: "The side pane is hidden. Ctrl+Shift+Q shows it." (the keys come from `shortcutLabel(commandById("preview").binding!, os)`).
- The shell panel's share stays `SHELL = { header: 32, share: 0.3, minBody: 120, maxBody: 280 }`.
- Frontend tests: `pnpm test <path>`; components through `renderToStaticMarkup`. Typecheck with `pnpm exec tsc --noEmit`, never `tsc -b`.
- Rust: `cargo fmt --all` before every Rust commit; `cargo test --workspace -j 4`, in the foreground, one check per call.
- A live check runs through the `marshell-live-check` skill with scratch homes, and stops only what it started, by PID. Real mouse drags are the user's; nothing here synthesizes desktop input.
- Commit messages read `type(scope): what changed, as a sentence.`

## Review Focus

1. **A display scale of 125% or 150%.** A row is then a fraction of a pixel tall (17.6 px). The panel must still show whole rows and never lose one to rounding. Pinned by `wholeRows` tests in Task 2 and by measure M2 at an emulated 125% in Task 6.
2. **A shell panel whose terminal has not measured a row** (not started, failed, folded since the start). The panel is as tall as before the slice, never 0 or NaN. Pinned in Task 2's `shellLayout` and `rowCell` tests.
3. **A font size change while the panel is open.** The row's height changes, and the panel must follow to the new whole rows without a loop of fits. Pinned by Task 2's "reports only a change" test and by Task 6, check C3.
4. **Leaving a covering pane when the session has ended** (no program to take the keys). The pane still hides, the line is announced once, and no error is logged. Pinned in Task 6, check C5.
5. **A window resized while a dialog or the palette is open.** The measure is applied synchronously inside the observer; React must log no "flushSync was called from inside a lifecycle method" and the overlay must keep its focus. Pinned in Task 6, check C6.

---

### Task 1: The strip spike and the baseline (gate)

Says whether a native child of fixed size keeps up with the window's edge where the webview's buttons trail. The same run takes the baseline numbers the later tasks are judged against. The user drags and judges; the agent builds, measures and writes it down.

**Files (branch `resize-polish-spike` only):**
- Create: `src-tauri/src/strip_spike.rs`
- Modify: `src-tauri/src/main.rs` (`mod strip_spike;`, one call in `setup`, one in the `WindowEvent::Resized` arm at lines 331-335)
- Modify: `src-tauri/Cargo.toml` only if the child's class needs `Win32_Graphics_Gdi` on `windows-sys`
- Create (branch `resize-polish`): `docs/spikes/S13-window-buttons-strip.md`, `docs/research/2026-11-20-resize-polish-live-check.md`

**Interfaces:**
- Produces: `strip_spike::install(app: &tauri::AppHandle)`, `strip_spike::on_resized(window: &tauri::Window, size: tauri::PhysicalSize<u32>)`. Nothing later consumes them.
- Produces for Tasks 5 and 6: the baseline numbers M1, M2 and M3 in the research note.

- [ ] **Step 1: Cut `resize-polish-spike` from `resize-polish` and write `strip_spike.rs`.**

Two blocks, each 138 × 40 logical pixels, against the window's right edge, under the app's own bar so all three can be compared at once:
- **A, a plain Win32 child** of the main window's HWND at y = 40: its own window class, background the system highlight brush (`(COLOR_HIGHLIGHT + 1)`), styles `WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS`, placed at `HWND_TOP`. Its window procedure answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE` and passes the rest to `DefWindowProcW`.
- **B, a child webview** at y = 84: `WebviewBuilder::new("strip-spike", WebviewUrl::External("about:blank"))`, `.focused(false)`, added with `add_child` as `browser::blank_page` does.

`on_resized` moves both to `x = width - 138 × scale`. The environment variable `MARSHELL_STRIP_SPIKE` picks how A is moved: `event` (the default: from Tauri's `Resized`) or `subclass` (`SetWindowSubclass` on the main HWND, moving A inside `WM_WINDOWPOSCHANGED`; `Win32_UI_Shell` is already enabled).

- [ ] **Step 2: Build and start it.** Invoke `marshell-live-check`; `cargo clippy -p marshell-app --all-targets -- -D warnings` first. Expected: the app shows a highlight-coloured block and a white block under the window buttons.

- [ ] **Step 3: Take the baseline measures** (the two blocks don't touch what is measured). Write each number into `docs/research/2026-11-20-resize-polish-live-check.md`.
  - **M1, the layout's lag.** Shell panel open, window 1200 × 780. Step the height 780 → 620 → 780 in 80 steps of 4 px with the window stepping the resize-fixes check used, one step per two frames. A `requestAnimationFrame` sampler records `[innerHeight, panel.offsetHeight]` each frame. A stale frame is a sample whose panel height isn't the settled one for that `innerHeight`. Record the count.
  - **M2, the part row.** At twelve heights, 780 down to 560 by 20, settled: `gap = body.clientHeight - pad - term.rows × cell`, with `cell` the `.xterm-screen` height over `term.rows`. Record the largest gap.
  - **M3, `windowsPty`.** In a PowerShell session print `1..200 | % { "line $_" }`, then shrink the window by ten rows and grow it back, twenty times, and read every line of `term.buffer.active`. Count numbers lost, numbers doubled and blank lines inside the run. Do it once as the app is, and once after `term.options.windowsPty = { backend: "conpty", buildNumber: 22000 }` on a fresh session. Record both counts.

- [ ] **Step 4: The hand session, with the user.** Ask the user when (AskUserQuestion). They drag, and say for each of the three (the app's buttons, A, B) "keeps up" or "trails":
  - the left edge, the right edge, the top-right corner, slowly and fast;
  - maximize, restore, Win+Left and Win+Right;
  - the same with a browser tab open in the side pane;
  - a click on A and on B: the terminal must keep the keyboard.
  If A trails under `event`, restart with `MARSHELL_STRIP_SPIKE=subclass` and repeat the edge drags. In the same session the user says what the shell panel and its bottom bar look like on top- and bottom-edge drags today (the baseline for Task 2).

- [ ] **Step 5: Stop what the check started, by PID, and write `docs/spikes/S13-window-buttons-strip.md`:** the question, the two candidates, the user's words per drag, and go or no-go.
  - **Go** (A or B keeps up on left- and right-edge drags): put the strip's open design questions to the user (which kind was better decides them: a native child needs its own drawing, colours and accessible names; a child webview needs a capability for its window calls and the cover rule for menus and dialogs). Record the answers in `docs/PLAN.md` under this slice's decisions, and write the strip's tasks into this file as Task 7 before building them.
  - **No-go:** add to decision 2 in `docs/PLAN.md` that the spike ended no-go and why, and carry the limit into the PR's Known limits.

- [ ] **Step 6: Commit.** On `resize-polish-spike`: `git commit -m "spike(shell): two fixed-size blocks follow the window's right edge, to see whether they keep up."` and push the branch. On `resize-polish`: `git add docs/spikes/S13-window-buttons-strip.md docs/research/2026-11-20-resize-polish-live-check.md docs/PLAN.md` and `git commit -m "docs(spike): S13 says whether a native strip keeps up with the window's edge."`

---

### Task 2: The shell panel in whole rows, in the window's frame

**Files:**
- Create: `src/features/terminal/rowCell.ts`, `src/features/terminal/rowCell.test.ts`
- Modify: `src/features/window/shellLayout.ts`, `src/features/window/shellLayout.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx` (the fit path at lines 316-335, the props)
- Modify: `src/features/window/Sidebar.tsx:254`, `src/features/window/Sidebar.test.tsx`
- Modify: `src/features/window/MainWindow.tsx` (the shell options it forwards at line 455)
- Modify: `src/features/sessions/LiveApp.tsx` (`useElementSize` at 180-194, `shellTerminal` at 1331-1334, the `shell` prop at 2173)

**Interfaces:**
- Produces: `type RowCell = { height: number; pad: number }`, `rowCell(screenHeight: number, rows: number, pad: number): RowCell | null` and `sameCell(a: RowCell | null, b: RowCell | null): boolean` in `rowCell.ts`.
- Produces: `wholeRows(body: number, cell: RowCell | null): number` and a sixth parameter `cell: RowCell | null = null` on `shellLayout`.
- Produces: `onCell?: (cell: RowCell) => void` on `TerminalView`; `cell?: RowCell | null` on the `shell` option of `MainWindow` and `Sidebar`.

- [ ] **Step 1: Write the failing tests.**

`src/features/terminal/rowCell.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { rowCell, sameCell } from "./rowCell";

describe("rowCell", () => {
  it("is the screen's height over its rows, with the padding beside it", () => {
    expect(rowCell(340, 20, 8)).toEqual({ height: 17, pad: 8 });
  });
  it("keeps a row that is a fraction of a pixel tall", () => {
    expect(rowCell(352, 20, 0)).toEqual({ height: 17.6, pad: 0 });
  });
  it("is null until the terminal has rows and a height", () => {
    expect(rowCell(0, 20, 8)).toBeNull();
    expect(rowCell(340, 0, 8)).toBeNull();
    expect(rowCell(Number.NaN, 20, 8)).toBeNull();
  });
  it("reports only a change", () => {
    expect(sameCell({ height: 17, pad: 8 }, { height: 17, pad: 8 })).toBe(true);
    expect(sameCell({ height: 17, pad: 8 }, { height: 18, pad: 8 })).toBe(false);
    expect(sameCell(null, { height: 17, pad: 8 })).toBe(false);
  });
});
```

Added to `src/features/window/shellLayout.test.ts`:

```ts
const CELL = { height: 17, pad: 8 };

describe("wholeRows", () => {
  it("cuts a body down to the rows that fit", () => {
    expect(wholeRows(180, CELL)).toBe(178);
    expect(wholeRows(280, CELL)).toBe(280);
    expect(wholeRows(120, CELL)).toBe(110);
  });
  it("rounds a fractional row up to a whole pixel, and never loses a row that fits exactly", () => {
    expect(wholeRows(150, { height: 17.6, pad: 8 })).toBe(149);
    expect(wholeRows(184, { height: 17.6, pad: 8 })).toBe(184);
  });
  it("leaves the body alone without a measured row, or when not one row fits", () => {
    expect(wholeRows(180.4, null)).toBe(180);
    expect(wholeRows(180, { height: 0, pad: 8 })).toBe(180);
    expect(wholeRows(20, CELL)).toBe(20);
  });
});

describe("shellLayout with a measured row", () => {
  it("opens to whole rows", () => {
    expect(shellLayout(600, 0, false, 56, false, CELL)).toEqual({ height: 210, collapsed: false, auto: false });
  });
  it("is as before without one", () => {
    expect(shellLayout(600, 0)).toEqual({ height: 212, collapsed: false, auto: false });
  });
  it("opens a forced panel to whole rows too", () => {
    expect(shellLayout(230, 0, false, 56, true, CELL)).toEqual({ height: 125, collapsed: false, auto: false });
  });
});
```

Added to `src/features/window/Sidebar.test.tsx`, with the file's own helper for a sidebar that has a shell: a sidebar 600 px tall whose `shell.cell` is `{ height: 17, pad: 8 }` renders `--shell-h:210px`, and the same sidebar without `cell` renders `--shell-h:212px`.

- [ ] **Step 2: Run them.** `pnpm test src/features/terminal/rowCell.test.ts src/features/window/shellLayout.test.ts src/features/window/Sidebar.test.tsx`. Expected: FAIL, `rowCell` and `wholeRows` are not exported.

- [ ] **Step 3: Implement `rowCell` and `sameCell` in `rowCell.ts`,** then `wholeRows` and the `cell` parameter in `shellLayout.ts`.

`wholeRows`: `n = Math.floor((body - pad) / height + 1e-6)`; with `cell` null, a height that isn't above 0, or `n < 1`, the answer is `Math.round(body)`; otherwise `Math.ceil(pad + n × height - 1e-6)`. In `shellLayout` the open body and the forced body both go through it, the forced one after its `Math.min`, and the fold test and `MIN_FORCED_BODY` use the cut values.

- [ ] **Step 4: Report the row from `TerminalView`.** After every fit that ran, read `term.element.querySelector(".xterm-screen")`'s `getBoundingClientRect().height`, `term.rows`, and the vertical padding of `term.element` and of the host (`getComputedStyle`, top plus bottom, the same padding `FitAddon` leaves out), and call `onCell` when `sameCell` says it changed. Only the shell panel's `TerminalView` gets the prop.

- [ ] **Step 5: Carry it to the layout.** `LiveApp` keeps `const [shellCell, setShellCell] = useState<RowCell | null>(null)`, passes `onCell={setShellCell}` to `shellTerminal` and `cell: shellCell` in `shell`; `MainWindow` forwards it; `Sidebar.tsx:254` passes `shell.cell ?? null` as the sixth argument.

- [ ] **Step 6: Apply the window's measure in its own frame.** In `useElementSize` the observer's callback wraps `setSize` in `flushSync` from `react-dom` and skips a measure equal to the last one. The first `measure()` inside the layout effect stays a plain `setSize` (React refuses `flushSync` there).

- [ ] **Step 7: Run the tests and the typecheck.** The command of Step 2, then `pnpm test src/features/window src/features/terminal` and `pnpm exec tsc --noEmit`. Expected: PASS, no type errors.

- [ ] **Step 8: Commit.** `git commit -m "fix(shell-panel): the panel is as tall as whole terminal rows, and the window's size reaches the layout in the frame it changes."`

---

### Task 3: Leaving a pane that covers the terminal hides it

**Files:**
- Modify: `src/features/pane/paneCommands.ts`, `src/features/pane/paneCommands.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (`applyNav` at 1474-1478, `useBrowserKeys` at 1529, the two `onEscape` props at 2046 and 2076)

**Interfaces:**
- Produces: `leavePaneOutcome(covers: boolean): "hide" | "focus"` and `paneHiddenText(keys: string): string` in `paneCommands.ts`.

- [ ] **Step 1: Write the failing tests** in `paneCommands.test.ts`:

```ts
describe("leavePaneOutcome", () => {
  it("hides a pane that covers the terminal", () => {
    expect(leavePaneOutcome(true)).toBe("hide");
  });
  it("only moves the keyboard when the terminal shows beside the pane", () => {
    expect(leavePaneOutcome(false)).toBe("focus");
  });
});

it("says the pane is hidden and which keys show it", () => {
  expect(paneHiddenText("Ctrl+Shift+Q")).toBe("The side pane is hidden. Ctrl+Shift+Q shows it.");
});
```

- [ ] **Step 2: Run them.** `pnpm test src/features/pane/paneCommands.test.ts`. Expected: FAIL, `leavePaneOutcome` is not exported.

- [ ] **Step 3: Implement both in `paneCommands.ts`,** beside `paneTabFocus`, whose doc comment already names the covered terminal.

- [ ] **Step 4: Use them in `LiveApp`.** One function, `toTerminalFromPane()`: it reads `document.querySelector('[data-pane="cover"]') !== null` as `paneRun` does at line 1032; on "hide" it runs `paneRun({ kind: "preview" })` (which hides the pane and focuses the terminal a tick later) and announces `paneHiddenText(...)`; on "focus" it calls `actions.focusTerminal()`. It replaces `actions.focusTerminal()` in exactly four places:
  - `applyNav`'s `select-pane`, only when `document.activeElement?.closest("[data-nav-side]") != null` (the key was pressed inside the pane; a session picked in the sidebar keeps today's path);
  - the `focusTerminal` given to `useBrowserKeys` (Esc in a page);
  - the browser tab's `onEscape` (line 2046) and the file tab's `onEscape` (line 2076).

- [ ] **Step 5: Run the tests and the typecheck.** `pnpm test src/features/pane src/features/sessions` and `pnpm exec tsc --noEmit`. Expected: PASS.

- [ ] **Step 6: Commit.** `git commit -m "fix(pane): Esc and Ctrl+Shift+Tab in a pane that covers the terminal hide the pane, so the keys never go to a program out of sight."`

---

### Task 4: The divider keeps the keyboard

**Files:**
- Modify: `src/features/window/paneLayout.ts`, `src/features/window/paneLayout.test.ts`
- Modify: `src/features/window/MainWindow.tsx` (`dividerLive` at line 372, the divider at 503-522, the props), `src/features/window/MainWindow.test.tsx`
- Modify: `src/features/sessions/LiveApp.tsx` (the new prop)

**Interfaces:**
- Produces: `dividerState(mode: "hidden" | "beside" | "cover", moving: boolean): { live: boolean; still: boolean }` in `paneLayout.ts` (`mode` is `paneGeometry`'s own type).
- Produces: `onDividerGone?: () => void` on `MainWindow`.

- [ ] **Step 1: Write the failing tests** in `paneLayout.test.ts`:

```ts
describe("dividerState", () => {
  it("can be reached and moved beside a settled pane", () => {
    expect(dividerState("beside", false)).toEqual({ live: true, still: false });
  });
  it("stays reachable, and doesn't move, while the layout moves around a pane that stays beside", () => {
    expect(dividerState("beside", true)).toEqual({ live: true, still: true });
  });
  it("is a line nobody can reach while the pane hides or covers", () => {
    expect(dividerState("hidden", true)).toEqual({ live: false, still: true });
    expect(dividerState("cover", false)).toEqual({ live: false, still: true });
  });
});
```

And in `MainWindow.test.tsx`: a window with the pane beside the terminal renders the divider with `role="separator"`, `tabindex="0"` and no `data-still`.

- [ ] **Step 2: Run them.** `pnpm test src/features/window/paneLayout.test.ts src/features/window/MainWindow.test.tsx`. Expected: FAIL, `dividerState` is not exported.

- [ ] **Step 3: Implement `dividerState`, and draw the divider from it.** One element for both states, so React keeps the node and its focus: `role`, the `aria-` values and `tabIndex={0}` whenever `live`; `data-still` whenever `still`; the pointer and key handlers return at once while `still`. `aria-hidden` only when not `live`.

- [ ] **Step 4: Hand the keyboard on when the divider goes.** `MainWindow` notes in a ref whether the divider has focus (`onFocus`, `onBlur`); a layout effect on `live` calls `onDividerGone` when `live` turns false with that ref set and the pane still drawn in `cover` mode. `LiveApp` passes `onDividerGone={() => focusPane()}`.

- [ ] **Step 5: Run the tests and the typecheck.** `pnpm test src/features/window` and `pnpm exec tsc --noEmit`. Expected: PASS.

- [ ] **Step 6: Commit.** `git commit -m "fix(pane): a focused divider keeps the keyboard through a layout motion, and hands it to the pane when the pane covers the terminal."`

---

### Task 5: `windowsPty`, only if M3 says so

Skip this task, and write one line in the research note saying why, unless Task 1's M3 counted fewer lost, doubled and blank lines with the option on than with it off.

**Files:**
- Create: `src/features/terminal/windowsPty.ts`, `src/features/terminal/windowsPty.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx` (`terminalOptions` at lines 82-85 and its call at line 213)

**Interfaces:**
- Produces: `windowsPtyFor(platform: Platform, platformVersion: string | undefined): { backend: "conpty"; buildNumber: number } | undefined` and `readPlatformVersion(): Promise<string | undefined>` (from `navigator.userAgentData.getHighEntropyValues(["platformVersion"])`, undefined where the webview has none).

- [ ] **Step 1: Write the failing tests** in `windowsPty.test.ts`:

```ts
describe("windowsPtyFor", () => {
  it("is ConPTY on Windows 11, whose platform version starts at 13", () => {
    expect(windowsPtyFor("windows", "13.0.0")).toEqual({ backend: "conpty", buildNumber: 22000 });
    expect(windowsPtyFor("windows", "19.0.0")).toEqual({ backend: "conpty", buildNumber: 22000 });
  });
  it("is ConPTY on Windows 10, under the build where xterm keeps its own reflow", () => {
    expect(windowsPtyFor("windows", "10.0.0")).toEqual({ backend: "conpty", buildNumber: 19045 });
  });
  it("is left unset when the version can't be read, and off Windows", () => {
    expect(windowsPtyFor("windows", undefined)).toBeUndefined();
    expect(windowsPtyFor("windows", "x")).toBeUndefined();
    expect(windowsPtyFor("linux", "13.0.0")).toBeUndefined();
    expect(windowsPtyFor("macos", "13.0.0")).toBeUndefined();
  });
});
```

(`Platform` is `src/generated/Platform.ts`; use its own spellings for the two other platforms.)

- [ ] **Step 2: Run them.** `pnpm test src/features/terminal/windowsPty.test.ts`. Expected: FAIL, the module doesn't exist.

- [ ] **Step 3: Implement both functions, and set the option.** `terminalOptions` takes `windowsPty` in its `Pick`; `TerminalView` sets `term.options.windowsPty` once `readPlatformVersion()` answers (the option can be set on a live terminal, as M3 did), for `endpoint.platform`.

- [ ] **Step 4: Run the tests and the typecheck.** `pnpm test src/features/terminal` and `pnpm exec tsc --noEmit`. Expected: PASS.

- [ ] **Step 5: Commit.** `git commit -m "fix(terminal): xterm knows it runs on ConPTY, so rows that come back on a resize don't replace lines."`

---

### Task 6: The live check after, and the limits written down

**Files:**
- Modify: `docs/research/2026-11-20-resize-polish-live-check.md`

- [ ] **Step 1: Start the app from `resize-polish`** (`marshell-live-check`, fresh scratch homes) and repeat M1, M2 and, if Task 5 ran, M3. Expected: M1 counts 0 stale frames; M2's largest gap is under 1 px; M3 is no worse than Task 1's "on" run.

- [ ] **Step 2: Run the checks, each with what was seen:**
  - **C1.** M2 again at an emulated 125% scale (`Emulation.setDeviceMetricsOverride`, `deviceScaleFactor: 1.25`): the largest gap is under 1 px and `term.rows` equals the rows `wholeRows` meant.
  - **C2.** The panel folded and opened again (its shortcut), and the app started with the panel folded: the height is whole rows once the terminal has drawn, and never 0.
  - **C3.** The terminal's font size raised by two steps in Settings with the panel open: the panel settles on whole rows of the new size, with at most two fits (`term.onResize` counted).
  - **C4.** A window 1000 px wide with a text tab open (the pane covers): Esc in the tab's body hides the pane, `document.activeElement` is the terminal's textarea, and the announcer says the line once. The same with Ctrl+Shift+Tab. With 1300 px (the pane beside) both only move the keyboard and nothing is announced.
  - **C5.** C4's Esc with the selected session ended: the pane hides, the line is announced once, the console logs nothing.
  - **C6.** The palette open (Ctrl+Shift+K) while the window's height steps as in M1: the console shows no React warning and no new "ResizeObserver loop" line beyond the baseline's count, and the palette's field still has focus.
  - **C7.** The divider focused, then the sidebar folded with its shortcut: after the motion `document.activeElement` is still the divider and Left moves the pane by 16 px. The divider focused, then the window narrowed to 1000 px: `document.activeElement` is inside `[data-nav-side]`.
  - **C8.** Forced colours emulated: nothing in the slice draws anything new; confirm the divider's focus ring and the shell panel look as on `main`.

- [ ] **Step 3: The second hand session, with the user** (ask when): top- and bottom-edge drags with the shell panel open, a pane covering the terminal and Esc in a page of a browser tab (a page's keys need real input), and, after a go in Task 1, the strip.

- [ ] **Step 4: Stop what the check started, by PID. Write the numbers, the checks and the user's words into the research note,** with "Not fixed" lines for the two limits (the sidebar's bottom bar on bottom-edge drags, the address bar's buttons on left- and right-edge drags) and anything the check found and left.

- [ ] **Step 5: Commit.** `git commit -m "docs(research): the resize-polish live check, with the numbers before and after."`

After Task 6: the whole-branch review (`marshell-reviewer`, Opus) with one fix pass, then `marshell-ship`.
