# Window Buttons Strip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On Windows, Minimize, Maximize/Restore and Close stop trailing the window's right edge while it is dragged, hovering Maximize shows Snap Layouts, and Win+Left/Right either snap the window or the reason they can't is recorded.

**Architecture:** A plain Win32 child window (the strip) sits over the page's three buttons and is moved from Tauri's `Resized` event, which spike S13 showed keeps up with the edge. The strip draws nothing of its own: the page paints each button's two pictures (at rest, under the pointer) on a canvas from its own computed colours and sends them through one command, `set_window_strip`; the strip blits them, hit-tests the pointer and calls the same window methods the page's buttons call. The page's buttons stay under the strip for the keyboard, for screen readers and as the fallback whenever the strip is hidden.

**Tech Stack:** Tauri 2.12 (tao 0.37, wry 0.57), `windows-sys` 0.61 (GDI blit, window class, hit test), React 19, TypeScript, a 2D canvas, Vitest (node, no DOM).

**Spec:** `docs/PLAN.md` §9, "Window-buttons strip slice decisions (2026-11-20)", with `docs/spikes/S13-window-buttons-strip.md` (the spike's code: branch `resize-polish-spike`, `src-tauri/src/strip_spike.rs`) and issue #23.

**Implementers:** Sonnet `worker` agents (the user's choice), `effort: high` for Tasks 3 and 6. Tasks 1 to 3 (Rust) and Tasks 4 to 5 (TypeScript) share only the payload below and can run side by side.

## Global Constraints

- Branch `window-strip`, cut from `origin/main` (ca9ef743), in the worktree `.claude/worktrees/resize-fixes`.
- No new crate or package. `windows-sys` gains features only (`Win32_Graphics_Gdi`, `Win32_System_LibraryLoader`, and others only if a call needs them).
- The one capability change is the line `"allow-set-window-strip"` in `src-tauri/capabilities/default.json` (allowed by the user). No other change to `capabilities/`, `tauri.conf.json` or `tauri.windows.conf.json`.
- Windows only. Off Windows the command exists, stores nothing and returns `Ok(())`; the page never calls it there.
- The page's three buttons keep their markup, names (`Minimize`, `Maximize`/`Restore`, `Close`) and handlers. Every change works with the keyboard alone and in forced colours.
- Colours come from computed styles of the page's own elements. No hex value and no second colour table, in CSS, TypeScript or Rust.
- **The payload** (the page sends, Rust reads; all sizes in device pixels, pictures as RGBA bytes, rows top-down, `width * height * 4` each):
  ```ts
  type ButtonPaint = { width: number; rest: number[]; hover: number[] };
  type StripPaint = { height: number; minimize: ButtonPaint; maximize: ButtonPaint; restore: ButtonPaint; close: ButtonPaint };
  invoke<void>("set_window_strip", { shown: boolean, paint: StripPaint | null });
  ```
  `paint: null` keeps the pictures the strip has. The strip's window is visible only while `shown` is true and it has pictures.
- Geometry, in CSS pixels: a button is 46 wide, the bar 40 tall with a 1 px hairline at its bottom, a glyph 10 x 10.
- Frontend tests: `pnpm test <path>`; components through `renderToStaticMarkup`. Typecheck with `pnpm exec tsc --noEmit`, never `tsc -b`. Read the good-css skill before writing CSS declarations.
- Rust: `cargo fmt --all` before every Rust commit; `cargo test -p marshell-app -j 4`; clippy with `-D warnings`. Checks run in the foreground, one per call.
- A live check runs through the `marshell-live-check` skill with scratch homes and stops only what it started, by PID. Real mouse and key input on the desktop is the user's, or a script's only with the user present and only after it has checked that the point lies in the check's own Marshell window.
- Commit messages read `type(scope): what changed, as a sentence.`

## Review Focus

1. **A picture set whose sizes don't agree** (a scale change between two sends, a zoomed webview, a truncated array). The strip must refuse it whole and keep the last good set, never blit past a buffer. Pinned by `Paint::check` tests in Task 2 and the `stripSync` scale test in Task 5.
2. **Something drawn over the bar's right end** (a dialog's scrim, a menu, a drag overlay, a toast). The strip must not sit on top of it. Pinned by Task 5's `shown` tests for every cover, and by Task 8 check C6, which also tries the two overlays that don't call `useCoverPage` (`Peek`, `AllSessionsMenu`) and a session toast.
3. **The window's top edge and top-right corner above the buttons.** They must still resize the window, and the strip must not turn the top rows of the buttons into a dead band. The first look found that the top edge resizes only on the one row above the client area, which the strip never covers (the live-check note, finding 1), so the strip takes the pointer over its whole height and no test pins a band. Task 8 check C4 covers it.
4. **A window maximized or restored by something other than the strip** (a double click on the bar, Win+Up, a snap, the page's own button by keyboard). The middle button must show Restore exactly when the window is maximized, and the strip must sit on the new right edge. Pinned by `slot_picture` tests in Task 3 and Task 8 check C3.
5. **A click on the strip when Marshell isn't the front window, or with the keyboard in a browser page.** The window comes to the front, the action happens once, and the keyboard is where it was. Pinned by `Pointer` tests in Task 1 and Task 8 check C5.

---

### Task 1: The pointer's rules (Rust, no window)

**Files:**
- Create: `src-tauri/src/window_strip/mod.rs` (declares `pub mod pointer;`), `src-tauri/src/window_strip/pointer.rs`
- Modify: `src-tauri/src/main.rs` (`mod window_strip;`)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Button { Minimize, Maximize, Close }
  /// `x` from the strip's left edge; `widths` are Minimize, Maximize, Close in device pixels.
  pub fn hit(x: i32, widths: [u32; 3]) -> Option<Button>;
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Event { Move(Option<Button>), Leave, Down(Button), Up(Option<Button>) }
  #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
  pub struct Pointer { hover: Option<Button>, pressed: Option<Button> }
  #[derive(Debug, PartialEq, Eq)]
  pub struct Outcome { pub redraw: bool, pub act: Option<Button> }
  impl Pointer {
      pub fn on(&mut self, event: Event) -> Outcome;
      /// The one button drawn with its hover picture, if any.
      pub fn lit(&self) -> Option<Button>;
  }
  ```

- [ ] **Step 1: Write the failing tests** in `pointer.rs`:
  - `hit_finds_the_button_by_its_column`: widths `[58, 57, 58]`: x 0 and 57 are Minimize, 58 and 114 Maximize, 115 and 172 Close; x -1 and 173 are `None`.
  - `a_press_and_release_on_one_button_acts_once`: `Down(Close)` then `Up(Some(Close))` gives `act: Some(Close)`; a second `Up(Some(Close))` gives `act: None`.
  - `a_release_on_another_button_or_outside_does_nothing`: `Down(Minimize)`, `Up(Some(Close))` and `Down(Minimize)`, `Up(None)` both give `act: None`.
  - `leaving_forgets_the_press`: `Down(Maximize)`, `Leave`, `Up(Some(Maximize))` gives `act: None`.
  - `only_a_change_asks_for_a_redraw`: `Move(Some(Minimize))` twice gives `redraw: true` then `false`; `Leave` with nothing lit gives `false`.
  - `a_pressed_button_is_lit_only_under_the_pointer`: after `Down(Close)`, `lit()` is `Some(Close)`; after `Move(Some(Minimize))` it is `None`; after `Move(Some(Close))` it is `Some(Close)` again.
- [ ] **Step 2:** Run `cargo test -p marshell-app -j 4 window_strip::pointer`. Expected: FAIL, the items don't exist.
- [ ] **Step 3:** Implement `hit`, `Pointer::on` and `Pointer::lit`. `redraw` is true exactly when `lit()` differs before and after the event.
- [ ] **Step 4:** Run the same command. Expected: 6 passed.
- [ ] **Step 5:** `cargo fmt --all`, then commit: `feat(shell): the window strip's pointer rules say which button is lit and when a click counts.`

### Task 2: The pictures and the command

**Files:**
- Modify: `src-tauri/src/window_strip/mod.rs`, `src-tauri/src/main.rs` (the handler list), `src-tauri/build.rs` (`COMMANDS`), `src-tauri/capabilities/default.json` (the one line, after `"allow-set-window-theme"`)

**Interfaces:**
- Produces:
  ```rust
  pub const MAX_SIDE: u32 = 512;
  #[derive(Debug, Clone, PartialEq, serde::Deserialize)]
  pub struct ButtonPaint { pub width: u32, pub rest: Vec<u8>, pub hover: Vec<u8> }
  #[derive(Debug, Clone, PartialEq, serde::Deserialize)]
  pub struct Paint { pub height: u32, pub minimize: ButtonPaint, pub maximize: ButtonPaint, pub restore: ButtonPaint, pub close: ButtonPaint }
  impl Paint {
      pub fn check(&self) -> Result<(), String>;
      /// Minimize, Maximize, Close.
      pub fn widths(&self) -> [u32; 3];
  }
  /// What the page last asked for. `apply` returns the pictures to show, or `None` to hide.
  #[derive(Default)]
  pub struct Wanted { shown: bool, paint: Option<Paint> }
  impl Wanted { pub fn apply(&mut self, shown: bool, paint: Option<Paint>) -> Result<Option<&Paint>, String>; }
  #[tauri::command]
  pub fn set_window_strip(window: tauri::Window, shown: bool, paint: Option<Paint>) -> Result<(), String>;
  ```
- In this task the command checks the window's label is `main` (else `Err("only the main window has a strip")`), runs `Wanted::apply` on a `static` behind a `Mutex`, and does nothing with the result. Task 3 hands the result to the window.

- [ ] **Step 1: Write the failing tests** in `mod.rs` (a helper `paint(height, [w_min, w_max, w_close])` builds a valid set):
  - `a_matching_set_passes`: `paint(40, [46, 46, 46]).check()` is `Ok(())`.
  - `a_picture_of_the_wrong_length_is_refused`: one byte short in `close.hover` gives `Err` naming "Close" and both byte counts.
  - `a_side_of_zero_or_over_the_limit_is_refused`: `height: 0`, `height: 513`, `minimize.width: 0`, `close.width: 513`.
  - `maximize_and_restore_must_be_as_wide`: `restore.width` 45 with `maximize.width` 46 is `Err`.
  - `a_refused_set_leaves_the_last_good_one`: `apply(true, Some(good))`, then `apply(true, Some(bad))` is `Err`, then `apply(true, None)` returns the good set.
  - `nothing_shows_without_pictures_or_when_hidden`: `apply(true, None)` on a new `Wanted` is `Ok(None)`; `apply(false, Some(good))` is `Ok(None)`; then `apply(true, None)` is `Ok(Some(_))`.
- [ ] **Step 2:** Run `cargo test -p marshell-app -j 4 window_strip`. Expected: FAIL.
- [ ] **Step 3:** Implement the types, `check`, `widths`, `Wanted::apply` and the command; add `"set_window_strip"` to `COMMANDS`, `window_strip::set_window_strip` to `generate_handler!` and the capability line.
- [ ] **Step 4:** Run `cargo test -p marshell-app -j 4`. Expected: PASS, including main.rs's test that the three command lists are equal.
- [ ] **Step 5:** `cargo fmt --all`, commit: `feat(shell): set_window_strip takes the window buttons' pictures and refuses a set whose sizes don't agree.`

### Task 3: The strip's window (Windows)

**Files:**
- Create: `src-tauri/src/window_strip/win.rs` (`#![cfg(windows)]`)
- Modify: `src-tauri/src/window_strip/mod.rs`, `src-tauri/src/main.rs` (the `Resized` arm and `setup`, where the spike branch hooked in), `src-tauri/Cargo.toml` (`windows-sys` features)

**Interfaces:**
- Consumes: Task 1's `Button`, `hit`, `Event`, `Pointer`; Task 2's `Paint`, `Wanted`.
- Produces (`win.rs`; `mod.rs` re-exports the two entry points under `#[cfg(windows)]`):
  ```rust
  pub fn install(app: &tauri::AppHandle);                       // setup, after the app's webview exists
  pub fn on_resized(window: &tauri::Window, size: tauri::PhysicalSize<u32>);
  pub(super) fn show(paint: Option<&Paint>);                    // called by the command with `Wanted::apply`'s result
  // Pure, tested here:
  pub(super) fn left_edge(client_width: i32, widths: [u32; 3]) -> i32;         // client_width - sum, never below 0
  pub(super) fn slot_picture<'a>(paint: &'a Paint, button: Button, maximized: bool, lit: bool) -> (&'a [u8], u32);
  pub(super) fn bgra(rgba: &[u8]) -> Vec<u8>;
  ```

What the window is (each line is a decision; the spike's `strip_spike.rs` shows the calls for the first two):
- Class `MarshellWindowStrip`, title `Window buttons`, style `WS_CHILD | WS_CLIPSIBLINGS`, not visible when made, parent the main window, arrow cursor, no background brush. `WM_ERASEBKGND` answers 1.
- Placement: x = `left_edge(client width, widths)`, y = 0, size = the widths' sum by `paint.height`, with `SetWindowPos(HWND_TOP, SWP_NOACTIVATE)`: from `on_resized`, and from `show` whenever pictures arrive. `on_resized` also redraws when the parent's `IsZoomed` changed.
- `WM_PAINT`: per button, `slot_picture` (the middle slot uses `restore` while the parent `IsZoomed`; `lit` from `Pointer::lit`), blitted with `SetDIBitsToDevice` as a top-down 32-bit `BI_RGB` picture. Pictures are turned to BGRA once, when they arrive.
- `WM_NCHITTEST`: left to the default, `HTCLIENT` over the whole strip. (As first written, the top `SM_CYFRAME` rows answered `HTTRANSPARENT` for a Tauri top-edge window below. This app has none: under Tauri's `unstable` feature the app's webview is a child webview, and Tauri makes that window only for a window-content webview. The band was removed at the first look; see the live-check note, finding 1.)
- Mouse: `WM_MOUSEMOVE` is `Move(hit)` and arms `TrackMouseEvent(TME_LEAVE)`; `WM_MOUSELEAVE` and `WM_CAPTURECHANGED` are `Leave`; `WM_LBUTTONDOWN` and `WM_LBUTTONDBLCLK` are `Down` with `SetCapture`; `WM_LBUTTONUP` releases the capture and is `Up(hit)`, `Up(None)` outside the strip. `Outcome.redraw` invalidates the window.
- `WM_MOUSEACTIVATE` answers `MA_NOACTIVATE`. On `Down`, if `GetForegroundWindow()` isn't the parent, `SetForegroundWindow(parent)`.
- An action is posted to the strip as a `WM_APP` message and carried out when it arrives, not inside the mouse message: Minimize is `window.minimize()`, Maximize is `unmaximize()` when `is_maximized()` and `maximize()` otherwise, Close is `window.close()` (it raises `CloseRequested`, so the tray choice runs as for the page's button). `window` is `crate::main_window(app)`.
- Threads: `set_window_strip` stays a plain (non-async) command, which Tauri runs on the main thread, the one that owns the window. `install`, `on_resized` and the window's procedure run there too, so nothing here crosses threads.
- State (the window's handle, the BGRA pictures, the `Pointer`) is behind one lock. **Never hold the lock across `SetWindowPos`, `ShowWindow`, `UpdateWindow`, `SetCapture` or a `tauri::Window` call:** they send messages to this window's own procedure, which takes the lock again.

- [ ] **Step 1: Write the failing tests** in `win.rs`:
  - `the_strip_ends_at_the_right_edge`: `left_edge(1280, [46, 46, 46])` is 1142; `left_edge(100, [46, 46, 46])` is 0.
  - `the_middle_slot_shows_restore_while_maximized`: with pictures whose first byte differs per picture, `slot_picture(&p, Maximize, false, false)` is `maximize.rest`, `(.., true, false)` is `restore.rest`, `(.., true, true)` is `restore.hover`; `Minimize` and `Close` ignore `maximized`.
  - `bgra_swaps_red_and_blue`: `[1, 2, 3, 4, 5, 6, 7, 8]` becomes `[3, 2, 1, 4, 7, 6, 5, 8]`.
- [ ] **Step 2:** Run `cargo test -p marshell-app -j 4 window_strip::win`. Expected: FAIL.
- [ ] **Step 3:** Implement the four functions, then the window as described, and wire `install`, `on_resized` and the command's call to `show`.
- [ ] **Step 4:** Run `cargo test -p marshell-app -j 4` and `cargo clippy --workspace --all-targets -- -D warnings`. Expected: PASS, no warnings. Every `unsafe` block has a `// SAFETY:` line, as in `strip_spike.rs`.
- [ ] **Step 5:** `cargo fmt --all`, commit: `feat(shell): a native strip shows the window buttons' pictures at the right edge and acts on a click.`

### Task 4: The page paints the pictures

**Files:**
- Create: `src/features/window/strip/tiles.ts`, `src/features/window/strip/tiles.test.ts`
- Modify: `src/features/window/MainWindow.tsx` (glyph paths from `GLYPHS`; the two probes), `src/features/window/MainWindow.css`, `src/features/window/WindowFrame.test.tsx`, `src/features/window/MainWindow.css.test.ts`

**Interfaces:**
- Produces (`tiles.ts`):
  ```ts
  export const BUTTON_W = 46, BAR_H = 40, GLYPH = 10;
  export const GLYPHS = { minimize: "M0 5h10", maximize: "M0.5 0.5h9v9h-9z", restore: "M2.5 2.5v-2h7v7h-2M0.5 2.5h7v7h-7z", close: "M0 0l10 10M10 0L0 10" } as const;
  export type TileLayout = { dpr: number; height: number; hairline: number; edges: [number, number, number, number] };
  export function tileLayout(dpr: number): TileLayout;
  export type Look = { bg: string; fg: string };
  export type StripLooks = { bar: string; hairline: string; rest: Look; hover: Look; closeHover: Look };
  export type ButtonPaint = { width: number; rest: number[]; hover: number[] };
  export type StripPaint = { height: number; minimize: ButtonPaint; maximize: ButtonPaint; restore: ButtonPaint; close: ButtonPaint };
  export type Surface = { ctx: CanvasRenderingContext2D; path(d: string): Path2D };
  export function paintStrip(layout: TileLayout, looks: StripLooks, make: (width: number, height: number) => Surface | null): StripPaint | null;
  /** The looks as the page's own elements have them now, or null when an element is missing. */
  export function readLooks(controls: HTMLElement): StripLooks | null;
  export function canvasSurface(doc: Document): (width: number, height: number) => Surface | null;
  ```
- `tileLayout`: `height = Math.round(BAR_H * dpr)`, `hairline = Math.max(1, Math.floor(dpr))`, `edges[i] = Math.round(i * BUTTON_W * dpr)`.
- One picture, for slot `i` (0 Minimize, 1 Maximize and Restore, 2 Close), width `w = edges[i + 1] - edges[i]`, in this order: fill all with `looks.bar`; fill the bottom `hairline` rows with `looks.hairline`; fill the rows above them with the look's `bg`; then with the transform `setTransform(dpr, 0, 0, dpr, -edges[i], 0)` and `translate(i * BUTTON_W + (BUTTON_W - GLYPH) / 2, (BAR_H - 1 - GLYPH) / 2)`, stroke the glyph's path with the look's `fg`, `lineWidth` 1. Then read the pixels back.
- A colour the canvas refuses (the `UNSET` check in `src/features/appearance/windowBackground.ts`) or a `make` that returns null makes `paintStrip` return null.
- `readLooks` reads computed styles: `bar` and `hairline` from the `.window__bar` the controls are in (`backgroundColor`, `borderBlockEndColor`); `rest` from the Minimize button; `hover` and `closeHover` from the two probes.
- Markup: after `.window__controls`, inside `.window__bar-end`, `<div hidden>` holding `<button type="button" tabIndex={-1} className="window__control" data-look="hover" />` and the same with `window__control--close`.
- CSS: `.window__bar-end { align-self: stretch; }`, `.window__controls { align-self: stretch; }`, and `.window__control` loses `block-size: 40px` (it stretches to the bar's 39 px above the hairline). In the hover media block the two selectors become `.window__control:hover, .window__control[data-look="hover"]` and `.window__control--close:hover, .window__control--close[data-look="hover"]`.

- [ ] **Step 1: Write the failing tests.**
  `tiles.test.ts` (a fake `Surface` records `fillStyle`/`fillRect`, `setTransform`, `translate`, `strokeStyle`/`stroke(path)`, and answers `getImageData` with `width * height * 4` bytes; it refuses the colour `"bad"` by leaving `fillStyle` unchanged):
  - `lays the buttons out in whole device pixels`: dpr 1 gives height 40, hairline 1, edges `[0, 46, 92, 138]`; 1.25 gives 50, 1, `[0, 58, 115, 173]`; 1.5 gives 60, 1, `[0, 69, 138, 207]`; 2 gives 80, 2, `[0, 92, 184, 276]`.
  - `paints the bar, the hairline, the button and its glyph, in that order`: for Close's hover picture at dpr 1 the record is `fillRect(0,0,46,40)` in `bar`, `fillRect(0,39,46,1)` in `hairline`, `fillRect(0,0,46,39)` in `closeHover.bg`, `setTransform(1,0,0,1,-92,0)`, `translate(110, 14.5)`, a stroke of `GLYPHS.close` in `closeHover.fg`.
  - `gives every picture its own width at a fractional scale`: at 1.25 the widths are 58, 57, 57, 58 (minimize, maximize, restore, close) and every array is `width * 50 * 4` long.
  - `uses the hover look for Minimize and Maximize and the close look for Close`.
  - `returns null when a colour is refused or no canvas can be made`.
  `WindowFrame.test.tsx`: `keeps two hidden probes beside the controls, out of the tab order`; the existing "draws the three controls for Windows only" still counts three buttons inside `.window__controls`.
  `MainWindow.css.test.ts`: `the probes take the hover rules` (both selector lists as above) and `a window button has no height of its own` (no `block-size` in the `.window__control` rule).
- [ ] **Step 2:** Run `pnpm test src/features/window`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement `tiles.ts`, the markup (MainWindow's `icon()` calls take `GLYPHS.*`) and the CSS.
- [ ] **Step 4:** Run `pnpm test src/features/window src/styles` and `pnpm exec tsc --noEmit`. Expected: PASS, no errors.
- [ ] **Step 5:** Commit: `feat(window): the page paints the window buttons' pictures from its own colours.`

### Task 5: The page keeps the strip up to date

**Files:**
- Create: `src/features/window/strip/windowStrip.ts`, `src/features/window/strip/windowStrip.test.ts`
- Modify: `src/features/window/MainWindow.tsx` (a ref on `.window__controls`, one effect)

**Interfaces:**
- Consumes: Task 4's `tileLayout`, `paintStrip`, `readLooks`, `canvasSurface`, `StripPaint`; `pageCovered`, `subscribeCover` from `src/features/browser/pageCover.ts`; `inTauri` from `src/lib/window/appWindow.ts`.
- Produces:
  ```ts
  export type StripRead = { shown: boolean; key: string | null; paint(): StripPaint | null };
  /** Sends when `shown` or `key` differs from the last send. `paint()` is called only when `key` changed. */
  export function stripSync(read: () => StripRead, send: (shown: boolean, paint: StripPaint | null) => Promise<void>): () => void;
  export function stripShown(covered: boolean, keyboardOnButtons: boolean): boolean;
  export function watchWindowStrip(controls: HTMLElement, doc?: Document): () => void;
  ```
- `key` is `JSON.stringify([devicePixelRatio, looks])`, or null when `readLooks` gives null. With a null key, or when `paint()` returns null, the send is `(false, null)` and the key is not remembered, so the next sync tries again.
- A send that rejects is forgotten whole (the next sync sends `shown` and the pictures again) and logs once per failure: `console.error("Marshell: the window buttons' strip couldn't be updated.", error)`.
- `watchWindowStrip` returns a no-op outside Tauri. It syncs once, then on: a change of `(prefers-color-scheme: dark)`, `(forced-colors: active)` or `(prefers-contrast: more)`; any attribute change on the root element; a change of `devicePixelRatio` (a `(resolution: <dpr>dppx)` query, made again after each change); `subscribeCover`; `focusin` and `focusout` on `controls`. `keyboardOnButtons` is `controls.querySelector(":focus-visible") !== null`. Its stop function removes every listener and sends `(false, null)`.
- MainWindow: `useEffect(() => (os === "windows" && controlsRef.current ? watchWindowStrip(controlsRef.current) : undefined), [os])`.

- [ ] **Step 1: Write the failing tests** in `windowStrip.test.ts`:
  - `shows only when nothing covers the page and the keyboard isn't on a button`: the four cases of `stripShown`.
  - `sends the pictures once and then only what changed`: same key and `shown` twice is one send with pictures; flipping `shown` sends `(false, null)`; `paint` was called once.
  - `paints again when the scale or a colour changes`: a new key sends new pictures.
  - `hides and tries again when the looks can't be read`: key null sends `(false, null)`; the next sync with a key sends pictures.
  - `sends everything again after a failed send`: a rejected send, then a sync with the same read, sends `shown` with pictures.
- [ ] **Step 2:** Run `pnpm test src/features/window/strip`. Expected: FAIL.
- [ ] **Step 3:** Implement `windowStrip.ts` and the effect.
- [ ] **Step 4:** Run `pnpm test`, `pnpm exec tsc --noEmit` and `pnpm build`. Expected: PASS. `src/features/browser/coverScan.test.ts` still passes untouched.
- [ ] **Step 5:** Commit: `feat(window): the strip gets new pictures when the theme, contrast or scale changes, and hides under an overlay or a focused button.`
- [ ] **Step 6: First look.** Start the app with `marshell-live-check`, confirm in the webview that `set_window_strip` was called without an error, and ask the user to drag the right edge once. If the strip is missing, misplaced or visibly unlike the page's buttons, fix that before Task 6.

### Task 6 (gate): Snap Layouts on Maximize

**Files:**
- Modify: `src-tauri/src/window_strip/win.rs`

- [ ] **Step 1:** Add a test `the_maximize_column_is_the_systems_maximize_button`: a pure `hit_code(x, widths) -> u32` gives `HTMAXBUTTON` over the middle column and `HTCLIENT` over the other two. Run it; expected FAIL.
- [ ] **Step 2:** `WM_NCHITTEST` returns `hit_code`. The middle column's pointer now arrives as non-client messages, fed to the same `Pointer`: `WM_NCMOUSEMOVE` is `Move(Some(Maximize))` and arms `TrackMouseEvent(TME_LEAVE | TME_NONCLIENT)`; `WM_NCMOUSELEAVE` is `Leave`; `WM_NCLBUTTONDOWN` and `WM_NCLBUTTONDBLCLK` are `Down(Maximize)`; `WM_NCLBUTTONUP` is `Up(Some(Maximize))`. None of the four reaches `DefWindowProcW` for the middle column (the default would draw and track an old caption button).
- [ ] **Step 3:** Run `cargo test -p marshell-app -j 4` and clippy. Expected: PASS.
- [ ] **Step 4: The gate, with the user, in the live app on Windows 11.** Go: hovering Maximize shows the Snap Layouts flyout, picking a layout snaps the window, a plain click still maximizes and restores, the hover picture shows and clears, and Minimize and Close behave as before. No-go on any of them that a fix within this task doesn't cure: revert the task's commit, leave issue #23 open with what was seen, and record it in the live-check note.
- [ ] **Step 5:** On go: `cargo fmt --all`, commit `feat(shell): hovering Maximize shows Windows 11's Snap Layouts.` (`Closes #23` goes in the PR body).

### Task 7 (gate): Win+Left and Win+Right

No file is known in advance; this task finds the cause first.

- [ ] **Step 1:** In the live app, read the main window's `GWL_STYLE` and `GWL_EXSTYLE`, `GetWindowPlacement` and whether it is resizable and maximizable, and record them. With the user: press Win+Left on Notepad (does Windows snap at all on this PC, or does another tool own the keys?), then on Marshell built from `main`, then on this branch.
- [ ] **Step 2:** If Notepad doesn't snap either, the keys aren't Windows' on this PC: record that and stop. If only Marshell doesn't, compare its styles with what Windows needs for a snap (`WS_THICKFRAME`, `WS_MAXIMIZEBOX`, and a window that isn't a tool window) and read how tao 0.37 handles `WM_SYSCOMMAND` and `WM_GETMINMAXINFO` for an undecorated window (`~/.cargo/registry/src/*/tao-0.37.1/src/platform_impl/windows/event_loop.rs`).
- [ ] **Step 3: The gate.** A cause in the app's own window code (`src-tauri/src/`), with a fix that changes neither `tauri.conf.json`, `tauri.windows.conf.json` nor the capabilities, is fixed here with a test where a pure piece allows one, and checked with the user's keys. A cause in tao, in Windows or in the window settings is recorded in the live-check note with the evidence, and a settings change is put to the user as a question, not made.
- [ ] **Step 4:** Commit what changed: `fix(shell): <the cause, as a sentence>.`, or nothing.

### Task 8: Live check, note and plan text

**Files:**
- Create: `docs/research/2026-11-20-window-buttons-strip-live-check.md`
- Modify: `docs/PLAN.md` (§9: results under the slice's decisions; any line elsewhere that says the window buttons trail), this plan (a "What was built" block if it differs)

Run with `marshell-live-check` (scratch homes, CDP). Each check gets a line in the note: what was done, what was seen, pass or fail.

- [ ] **C1, the pictures match the page.** For each of dark, light, dark with High contrast, light with High contrast and emulated forced colours: capture the page's own three buttons through CDP (the strip isn't in a CDP capture), at rest and with `data-look="hover"` set on each in turn, and compare with the bytes `paintStrip` gives for the same state (import `/src/features/window/strip/tiles.ts` in the dev webview). Record the share of pixels that differ and the largest difference. Any difference a person can see when the strip hides and shows (open and close the palette) is fixed in `tiles.ts` before going on.
- [ ] **C2, scale.** The same compare at 125% and 150%. Say in the note whether the scale was the display's or emulated. Moving the window between scales isn't possible on one monitor: record it as not checked.
- [ ] **C3, the middle button.** Maximize and restore by the strip, by a double click on the bar, by Win+Up and Win+Down, and by the page's button with the keyboard: Restore shows exactly while maximized, and the strip is on the right edge each time.
- [ ] **C4, the edges (the user's hands).** Right edge, left edge and top-right corner, slow and fast, with and without a browser page in the side pane: the strip keeps up. The top edge above the buttons and the top-right corner still resize the window, as they do on `main` (the one row above the client area, and the corner outside it).
- [ ] **C5, clicks and the keyboard.** With the keyboard in the terminal, then in a browser page, then with another app in front: a click on each button acts once, and the keyboard is where it was (`GetGUIThreadInfo` and the page's focus events, as in S13). Close shows the tray question on a scratch home that hasn't answered it, and the strip is hidden under that dialog.
- [ ] **C6, overlays.** With each of the palette, the app menu, a context menu, Settings, the launcher, a file dragged over the window, `Peek`, the all-sessions menu and a session toast open: nothing of theirs is hidden by the strip. An overlay that can reach the strip's rectangle without calling `useCoverPage` gets the call (and leaves `NOT_COVERING` in `coverScan.test.ts`).
- [ ] **C7, the keyboard alone.** Tab to each of the three buttons: the strip hides and the page's ring shows; Enter acts; after focus leaves, the strip is back.
- [ ] **C8, before and after the page.** At start no strip shows before the page's first pictures. After a reload of the webview (dev) the strip still works and takes new pictures.
- [ ] **Step 9:** Write the note, update `docs/PLAN.md` and this plan, run what CI runs (`marshell-ship`), commit: `docs(shell): the window buttons strip's live check.`

## Known limits to record in the PR

- The side-pane toggle and the address bar's buttons still trail (decision 1 and S13).
- The strip is a pane named "Window buttons" in the UI Automation tree; the named buttons are the page's. A screen reader that follows the mouse reads the pane's name over the strip. The NVDA hand pass is at release sign-off.
- Whatever C2 and Task 7 leave unchecked.
