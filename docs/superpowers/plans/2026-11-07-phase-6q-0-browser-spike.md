# Slice 6q-0: browser spike — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A throwaway prototype answers whether a WebView2 child webview on Marshell's real frameless window is good enough to build 6q-3's browser tab on, and gives the user a written go or no-go.

**Architecture:** A Tauri child webview (`Window::add_child`, behind the `unstable` feature) sits over a placeholder `div` that the frontend draws in the preview slot beside the terminal. Rust owns the child: its bounds, a raw WebView2 COM layer for keys, focus, history, capture and DevTools, and a JSONL log every check reads. The frontend has a dev-only pane (address bar, Back, Forward, Reload, placeholder) and a few hooks in `LiveApp`. Scripts in `spikes/browser-pane/` give a loopback test site and the real-input, UI Automation and screenshot helpers.

**Tech Stack:** Tauri 2.12.2 (`unstable`), wry 0.57.0, webview2-com 0.39.1 and windows 0.62 (both already direct dependencies), React, Node for the test server, PowerShell with C# `Add-Type` for SendInput, PrintWindow and UI Automation.

**Spec:** `docs/PLAN.md` §9, "Slice 6q decisions (2026-11-07, second round)" 2 to 6 and "Slice 6q-0 decisions (2026-11-07)" 1 to 5; `docs/research/2026-11-07-tabs-pane-options.md` §2.3, §4 (the 6q-0 row), §6 and §7.

## Global Constraints

- The branch `phase-6q-0` is never merged. Only the docs (this plan, the §9 decisions, `docs/spikes/S12-browser-pane.md`) reach `main`, through a docs-only PR cut from `origin/main`. Commit docs and code separately so the docs commits cherry-pick cleanly.
- Allowed on this branch only (6q-0 decision 1): `features = ["tray-icon", "unstable"]` on `tauri`, and `"webviews": ["main"]` in place of `"windows": ["main"]` in `src-tauri/capabilities/default.json`. Nothing else in SCOPE.md's "Ask the user first" list changes: no CSP, window setting or guard change, and no new crate or package. New `features` on the existing `windows` crate are fine.
- The main webview's CSP is never loosened, and nothing loads a remote URL into it.
- The browser profile lives at `<Marshell home>/browser-profile`, using the home `main.rs` already resolves (`HomeDir`). The spike runs only under `live-check.ps1`, so that home is a scratch one.
- The child's browser arguments are `--disable-features=msWebOOUI,msPdfOOUI` (wry's defaults minus `msSmartScreenProtection`, research §3). Whether `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (the live check's CDP port) overrides them is a check, not an assumption.
- Real input (6q-0 decision 2): `keys.ps1` sends a key only while `GetForegroundWindow` belongs to the dev app's PID and its rect is non-empty, and aborts otherwise. It never moves the mouse. The user does Alt+Tab, the edge drag, the Explorer drop and the scale changes by hand, when asked.
- Live-check rules from the `marshell-live-check` skill hold throughout: a new literal scratch folder, stop only what the check started (by PID), and `~/.claude/settings.json`'s modified time unchanged at the end.
- The key test's pass mark is 20 of 20 for each app key. A miss is investigated. An unexplained or unfixable miss, or focus that doesn't come back after Alt+Tab, stops the spike at the gate after Task 3, and the user re-plans (research §4).

## Review Focus

- AltGr is Ctrl+Alt on European layouts: AltGr+Q (`@` on German) typed in a page must stay in the page, never reach the app as a chord. Pinned by `altgr_combos_stay_in_the_page` (Task 2).
- Holding a chord repeats it: a held Ctrl+Shift+K must reach the app with `repeat` set, so the keymap's own repeat rule applies. Pinned by `a_held_chord_is_sent_as_a_repeat` (Task 2) and the live hold check.
- A redirect or a dropped file can reach `file:` or the app's own `*.localhost` hosts without the address bar: the allow-list must refuse those too. Pinned by `redirects_and_drops_are_checked_like_typed_urls` (Task 1) and the live redirect and drop checks (Tasks 7 and 8).
- Minimize and restore, Win+arrow snaps, maximize and hide-to-tray all move or resize the window without a pointer drag: the child must land back on its placeholder after each. Live check in Task 4.
- Closing or restarting the app with the page open must leave no `msedgewebview2.exe` holding the spike's profile, or the next start can't open it. Live check in Task 6 (restart) and Task 9 (stop).

---

### Task 0: The decisions and this plan

**Files:**
- Modify: `docs/PLAN.md` (§9, "Slice 6q-0 decisions (2026-11-07)", already written)
- Create: this plan

- [ ] **Step 1: Commit both, docs only**

```bash
git add docs/PLAN.md docs/superpowers/plans/2026-11-07-phase-6q-0-browser-spike.md
git commit -m "docs(plan): slice 6q-0's decisions and the browser spike's plan"
```

### Task 1: The child webview, the pane and the test site

**Files:**
- Modify: `src-tauri/Cargo.toml` (`unstable`), `Cargo.lock` if it changes
- Create: `src-tauri/src/spike_browser/mod.rs` (commands, the child's builder, the log), `src-tauri/src/spike_browser/rules.rs` (pure rules and their tests), `src-tauri/src/spike_browser/com.rs` (Windows COM handlers, `#[cfg(windows)]`)
- Modify: `src-tauri/src/main.rs` (`mod spike_browser;`, register the commands)
- Create: `src/features/spike/BrowserSpike.tsx`, `src/features/spike/BrowserSpike.css`, `src/features/spike/useBrowserSpike.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (the pane in the preview slot)
- Create: `spikes/browser-pane/server.mjs`, `spikes/browser-pane/probe.html`, `spikes/browser-pane/shot.ps1`
- Create: `docs/spikes/S12-browser-pane.md`

**Interfaces:**
- Produces, Rust commands (all `Result<_, String>`):
  - `spike_browser_open(url: String, rect: PaneRect)` creates the child labelled `spike-page`, or navigates it if it exists;
  - `spike_browser_rect(rect: PaneRect)` and `spike_browser_mode(anchor: bool)` (Task 4 fills in anchor);
  - `spike_browser_visible(visible: bool)`, `spike_browser_close()`;
  - `spike_log(event: String, data: serde_json::Value)` appends `{"t": <ms since UNIX epoch>, "by": "front", "event", ...data}` to `<home>/spike-browser.jsonl`. Rust writes there with `"by": "rust"` through `spike_browser::log(app, event, data)`.
- `shot.ps1 -AppPid <int> -Out <png>` captures the main window with `PrintWindow(hwnd, PW_RENDERFULLCONTENT)`, never the screen, so no other app's window is captured.
- `PaneRect { x, y, width, height, win_width, win_height }`, all `f64` CSS pixels, serde `camelCase`.
- `rules::allowed_navigation(url: &Url) -> bool`, used by `on_navigation` and refused URLs logged as `refused` with an emitted `spike://refused { url }`.
- Frontend: `useBrowserSpike()` returns `{ active: boolean; visible: boolean; toggle(): void; pane(frozen: boolean): ReactNode }`. While `import.meta.env.DEV`, it puts `window.__spike = { open(url), close(), setInset(px), setMode(anchor), keyTest(on) }` for CDP to call.
- Test site on `http://127.0.0.1:47620`: `/` (links, a text box, a `target=_blank` link, a long list for Tab), `/cookie`, `/redirect-file` (302 to `file:///C:/Windows/win.ini`), `/probe` (`probe.html`), and `POST /report`. The site logs every request and report to the file given as `--log <path>`, which is in the scratch folder.

- [ ] **Step 1: Write the failing tests in `rules.rs`**

```rust
#[test]
fn web_pages_and_blank_are_allowed() {
    for u in ["https://example.com/", "http://127.0.0.1:47620/", "http://localhost:3000/", "about:blank"] {
        assert!(allowed_navigation(&Url::parse(u).unwrap()), "{u}");
    }
}

#[test]
fn files_and_the_apps_own_hosts_are_refused() {
    for u in ["file:///C:/Windows/win.ini", "http://tauri.localhost/", "https://tauri.localhost/", "http://ipc.localhost/x",
              "http://asset.localhost/x", "tauri://localhost/", "data:text/html,hi", "javascript:alert(1)"] {
        assert!(!allowed_navigation(&Url::parse(u).unwrap()), "{u}");
    }
}

#[test]
fn redirects_and_drops_are_checked_like_typed_urls() {
    // One rule for every way in: on_navigation sees redirects and drops as plain URLs.
    assert!(!allowed_navigation(&Url::parse("file://server/share/a.pdf").unwrap()));
    assert!(!allowed_navigation(&Url::parse("http://sub.tauri.localhost/").unwrap()));
}
```

- [ ] **Step 2: Run and see them fail to compile**

Run: `cargo test -p marshell-app -j 4 spike_browser`
Expected: FAIL, `allowed_navigation` not found.

- [ ] **Step 3: Implement `allowed_navigation`.** http and https are allowed unless the host ends in `.localhost` (exactly `localhost` is allowed, for port chips). `about:blank` is allowed. Every other scheme is refused.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p marshell-app -j 4 spike_browser`
Expected: PASS, 3 tests.

- [ ] **Step 5: Build the child.** In `spike_browser_open`, get `app.get_webview_window("main")`, then its `Window` (`.as_ref().window()`), and call `add_child` with `WebviewBuilder::new("spike-page", WebviewUrl::External(url))`, using:
  - `.data_directory(home.join("browser-profile"))`;
  - `.additional_browser_args(<the constraint's string>)`;
  - `.devtools(false)`, as a release build has it;
  - `.on_navigation(...)`, which logs and emits a refusal;
  - and the logical position and size from `rect`.

  On Windows, `com::install(&webview)` runs once after creation. This task it logs the child's `GotFocus` and `LostFocus` as `page-focus` and `page-blur`. `spike_browser_visible` calls `hide` or `show`. Turn `unstable` on.

- [ ] **Step 6: Build the pane.** `BrowserSpike` has:
  - a bar with an address box (`aria-label="Address"`), Back, Forward and Reload buttons;
  - a status line;
  - a placeholder `div` (`tabIndex={0}`, `aria-label="Web page"`). A `ResizeObserver` and a window `resize` listener on it send `spike_browser_rect` with `getBoundingClientRect()` and `innerWidth` and `innerHeight`.

  In `LiveApp`, when `spike.active && spike.visible`, `MainWindow`'s `preview` prop gets `spike.pane(frozen)` in place of `previewPane`. `frozen` is `overlay !== null || launcherOpen || closeAsk !== null || review !== null`; it is unused until Task 5. Every edit in `LiveApp` carries a `// spike 6q-0` comment, so 6q-3 can find them.

- [ ] **Step 7: Write the test site** (`server.mjs`, Node built-ins only, port 47620). Write `probe.html`, a script that Task 7 fills in.

- [ ] **Step 8: Write the skeleton of `S12-browser-pane.md`.** It has a header naming the build, Windows' version and the WebView2 runtime's version. Its results table has one row per check from Tasks 1 to 8, in the S8 table's shape: Check, Pass criterion, Result. It ends with "Findings", "What 6q-2 and 6q-3 take from this" and "Go or no-go" sections.

- [ ] **Step 9: Start the live check.** Run `live-check.ps1 start -Scratch <scratchpad>\lc-6q0-1` and the test site, `node spikes/browser-pane/server.mjs --log <scratch>\site.jsonl`, recording its PID. Wait for CDP on 9333. Over CDP, call `__spike.open("http://127.0.0.1:47620/")`.

  Record:
  - (a) the page shows inside the placeholder at 100%;
  - (b) CDP on 9333 still reaches the main webview;
  - (c) the `msedgewebview2.exe` command lines (CIM `Win32_Process`, filtered to `--user-data-dir` under the scratch home) show which browser arguments the child really got;
  - (d) `http://127.0.0.1:47620/redirect-file` and an address-bar `file:` URL are refused and logged.

- [ ] **Step 10: Commit the code and, separately, the S12 rows**

```bash
git add src-tauri src spikes/browser-pane Cargo.lock
git commit -m "spike(browser): a child webview over the preview slot, with a loopback test site"
git add docs/spikes/S12-browser-pane.md
git commit -m "docs(spikes): S12's checks and the child webview's first results"
```

### Task 2: Keys — the go or no-go

**Files:**
- Modify: `src-tauri/src/spike_browser/rules.rs`, `com.rs`, `mod.rs`
- Modify: `src/features/spike/useBrowserSpike.ts`, `src/features/sessions/LiveApp.tsx` (Ctrl+Shift+Q and Esc hooks)
- Create: `spikes/browser-pane/keys.ps1`

**Interfaces:**
- Produces:
  - `rules::route_key(vk: u32, ctrl: bool, shift: bool, alt: bool) -> KeyRoute`, where `enum KeyRoute { App, DevTools, Page }`;
  - `rules::vk_to_code(vk: u32) -> Option<&'static str>` (DOM `KeyboardEvent.code`);
  - the event `spike://key { code: String, ctrl: bool, shift: bool, alt: bool, repeat: bool, n: u64 }`, where `n` counts up per webview;
  - `spike_browser_focus(reason: String)`, where the reason is `"next"`, `"previous"` or `"programmatic"` and maps to `COREWEBVIEW2_MOVE_FOCUS_REASON_*` through the child's `MoveFocus`.
- `keys.ps1 -AppPid <int> -Log <path> -Keys "Ctrl+Shift+K","Ctrl+Shift+Q","Ctrl+L","Escape" -Reps 20` prints one line per key: sent, forwarded by Rust, received by the front, acted on. `keys.ps1 -AppPid <int> -Type <text>` and `-Key <chord>` send one text or one key, with the same guard and no counting.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn marshells_chords_go_to_the_app() {
    assert_eq!(route_key(VK_K, true, true, false), KeyRoute::App);   // palette
    assert_eq!(route_key(VK_Q, true, true, false), KeyRoute::App);   // the pane
    assert_eq!(route_key(VK_I, true, true, false), KeyRoute::App);   // focus mode, not DevTools
    assert_eq!(route_key(VK_TAB, true, true, false), KeyRoute::App); // select the terminal
    assert_eq!(route_key(VK_L, true, false, false), KeyRoute::App);  // address bar
    assert_eq!(route_key(VK_ESCAPE, false, false, false), KeyRoute::App);
}

#[test]
fn f12_opens_the_pages_devtools() {
    assert_eq!(route_key(VK_F12, false, false, false), KeyRoute::DevTools);
}

#[test]
fn the_page_keeps_the_browsers_keys() {
    assert_eq!(route_key(VK_F5, false, false, false), KeyRoute::Page);
    for vk in [VK_R, VK_F, VK_P, VK_C, VK_V, VK_A, VK_Z] {
        assert_eq!(route_key(vk, true, false, false), KeyRoute::Page);
    }
}

#[test]
fn altgr_combos_stay_in_the_page() {
    // AltGr arrives as Ctrl+Alt: AltGr+Q is @ on a German layout, AltGr+Shift+K must not open the palette.
    assert_eq!(route_key(VK_Q, true, false, true), KeyRoute::Page);
    assert_eq!(route_key(VK_K, true, true, true), KeyRoute::Page);
}

#[test]
fn virtual_keys_map_to_dom_codes() {
    assert_eq!(vk_to_code(VK_K), Some("KeyK"));
    assert_eq!(vk_to_code(0x31), Some("Digit1"));
    assert_eq!(vk_to_code(VK_TAB), Some("Tab"));
    assert_eq!(vk_to_code(VK_ESCAPE), Some("Escape"));
    assert_eq!(vk_to_code(VK_F12), Some("F12"));
}
```

`a_held_chord_is_sent_as_a_repeat` goes in `com.rs`'s pure part: `key_event(vk, mods, was_key_down: bool) -> KeyEvent` sets `repeat` to `was_key_down`. Assert it is `true` for a held key and `false` for a first press.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-app -j 4 spike_browser`
Expected: FAIL, `route_key` not found.

- [ ] **Step 3: Implement the two rules and `key_event`.** `App` means Esc with no modifier, Ctrl+L, or any Ctrl+Shift chord without Alt. `DevTools` is F12. Everything else is `Page`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p marshell-app -j 4 spike_browser`
Expected: PASS, 9 tests.

- [ ] **Step 5: Add the COM handler.**
  - `add_AcceleratorKeyPressed` on the child's controller acts on `KEY_DOWN` and `SYSTEM_KEY_DOWN`.
    - `App` and `DevTools` set `Handled(true)`, also on the matching key-up.
    - The handler only reads the event and calls `app.run_on_main_thread`. Inside that closure, `App` runs the main webview's `set_focus()`, logs `key-forwarded`, then emits `spike://key`. `DevTools` calls `OpenDevToolsWindow` on the child.
  - At creation, `com::install` sets `SetAreDevToolsEnabled(true)` on the child's settings.

- [ ] **Step 6: Add the frontend side.**
  - On `spike://key`, `spike_log("key-received")`.
    - `KeyL` focuses the address box.
    - `Escape` calls `actions.focusTerminal()`.
    - Anything else dispatches `new KeyboardEvent("keydown", { code, key, ctrlKey, shiftKey, altKey, repeat, bubbles: true })` on `document.activeElement ?? document.body`, so the existing keymap acts on it.
  - In `windowCommand`'s `preview` case, an active spike toggles its own pane (logged `pane-hidden` or `pane-shown`, with the child hidden or shown) instead of the Preview.
  - Log `overlay` changes as `overlay`.
  - With `keyTest(true)`, after each logged action the pane waits 400 ms, puts things back and logs `reset`: it closes the palette with a synthetic Escape, shows the pane, and calls `spike_browser_focus("programmatic")`.

- [ ] **Step 7: Write `keys.ps1`.** It uses SendInput with scan codes. Before every key it calls `GetForegroundWindow`, `GetWindowThreadProcessId` and `GetWindowRect` (the guard). After each reset it waits up to 3 s for a new `page-focus` line, and a key it couldn't place counts as "not sent". The text it types for the box check is `abc`.

- [ ] **Step 8: Run the key checks** (the app restarts on the Rust edit; re-open the page over CDP).
  - (a) `keys.ps1 -Reps 20` for Ctrl+Shift+K, Ctrl+Shift+Q, Ctrl+L and Escape. Pass: 20 of 20 at every stage, for every key.
  - (b) After a forwarded Ctrl+Shift+K, `abc` lands in the palette's box (CDP reads its value).
  - (c) A 1 s hold of Ctrl+Shift+K logs one action and repeats marked `repeat`.
  - (d) F5, Ctrl+R, Ctrl+F and Ctrl+P act in the page: a reload logged by the site, the find bar, print preview. Five presses each, with the outcome read from the log, the site or a `shot.ps1` capture.
  - (e) F12 opens a DevTools window for the page: EnumWindows finds a new top-level window titled `DevTools - 127.0.0.1:47620…`. Ctrl+Shift+I reaches the app.
  - (f) In the page's text box, AltGr+Q with the German layout added for the check (the user's OK first, and removed after) types `@` and forwards nothing. If the user declines, record (f) as not run.

  Write each result into S12.

- [ ] **Step 9: Commit the code, then the S12 rows, as in Task 1**

### Task 3: Focus — Tab in and out, Alt+Tab, UI Automation

**Files:**
- Modify: `src-tauri/src/spike_browser/com.rs`, `mod.rs`; `src/features/spike/BrowserSpike.tsx`, `useBrowserSpike.ts`
- Create: `spikes/browser-pane/uia.ps1`

**Interfaces:**
- Consumes: `spike_browser_focus(reason)` (Task 2).
- Produces the event `spike://focus-out { direction: "next" | "previous" }`.
- `uia.ps1 -AppPid <int>` prints JSON with:
  - `documents`, the Name of every `ControlType.Document` under the main window;
  - `focused`, the focused element's Name and ControlType, plus the Document it sits under.

- [ ] **Step 1: Wire Tab in and out.**
  - The placeholder's `onFocus` calls `spike_browser_focus`. The reason is `"previous"` when focus came from after it (`relatedTarget` follows it in document order), else `"next"`.
  - `add_MoveFocusRequested` on the child sets `Handled(true)`, then on the main thread runs main `set_focus()`, logs and emits `spike://focus-out`.
  - The front moves focus out:
    - `next` goes to the first tabbable element after the placeholder, else to the first in the document;
    - `previous` goes to the Reload button.

- [ ] **Step 2: Write `uia.ps1`** (`UIAutomationClient`, `AutomationElement.FromHandle` on the process's main window).

- [ ] **Step 3: Run the focus checks.** Keys and text go through `keys.ps1 -Key` and `-Type`.
  - (a) Tab from Reload enters the page: `page-focus` is logged, and the site logs the first link's focus. Tab past the page's last element lands on the next app element. Shift+Tab back works. Repeat 10 times; pass is 10 of 10.
  - (b) The user clicks into the page's text box, then Alt+Tabs away and back. `abc` lands in the box, as the site logs.
  - (c) The user clicks the terminal, then Alt+Tabs away and back. `abc` lands in the shell; CDP reads the terminal's last line.
  - (d) A click on the page, then a click on the terminal: typing follows each click.
  - (e) `uia.ps1` lists both documents. After (a), `focused` sits under the page's document.

  Write each result into S12.

- [ ] **Step 4: Commit the code, then the S12 rows**

- [ ] **Step 5: The gate.** If Task 2 (a)–(c) or Task 3 (a)–(d) failed with no fix found, stop. Show the user S12's rows and ask, with AskUserQuestion, whether to re-plan or end the spike as a no-go. Otherwise carry on.

### Task 4: Resize, the frameless frame and display scale

**Files:**
- Modify: `src-tauri/src/spike_browser/rules.rs`, `mod.rs`; `src/features/spike/BrowserSpike.tsx`

**Interfaces:**
- Produces `rules::anchored(rect: PaneRect, win_width: f64, win_height: f64) -> LogicalRect`:
  - the pane keeps its right and bottom insets and its width;
  - its height stretches with the window;
  - x and height never go below 0.
- `LogicalRect { x, y, width, height }` is `f64`.
- Consumes `shot.ps1` (Task 1).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_pane_rides_the_right_edge() {
    let r = PaneRect { x: 700.0, y: 40.0, width: 480.0, height: 700.0, win_width: 1200.0, win_height: 780.0 };
    assert_eq!(anchored(r, 1200.0, 780.0), LogicalRect { x: 700.0, y: 40.0, width: 480.0, height: 700.0 });
    assert_eq!(anchored(r, 1300.0, 830.0), LogicalRect { x: 800.0, y: 40.0, width: 480.0, height: 750.0 });
}

#[test]
fn a_tiny_window_never_gives_negative_bounds() {
    let r = PaneRect { x: 700.0, y: 40.0, width: 480.0, height: 700.0, win_width: 1200.0, win_height: 780.0 };
    let a = anchored(r, 300.0, 60.0);
    assert!(a.x >= 0.0 && a.height >= 0.0);
}
```

- [ ] **Step 2: Run them and see them fail**, then implement `anchored`. Run them again and see them pass.

Run: `cargo test -p marshell-app -j 4 spike_browser`
Expected: FAIL, then PASS (11 tests).

- [ ] **Step 3: Add anchor mode.**
  - With `spike_browser_mode(true)`, a `WindowEvent::Resized` on `main` sets the child's bounds from `anchored` with the last `PaneRect`, at once and without waiting for the front.
  - The front's report still corrects the left edge after it.
  - Every bounds change logs `bounds`, with the requested logical rect, the child's `bounds()` afterwards and the scale factor.
  - `__spike.setInset(px)` pads the placeholder's right and bottom edges.

- [ ] **Step 4: Run the resize checks.** For each check, the user acts and the agent reads the log and a `shot.ps1` capture.
  - (a) The user drags the window's right edge, bottom edge and bottom-right corner, with inset 0. Record whether the frame still resizes where the page touches the edge (tauri#12207, #9634). If it doesn't, raise the inset 1 px at a time until it does, and record the smallest that works.
  - (b) The user drags the divider between the terminal and the pane. Do it in follow mode, then anchor mode, and the user says which lags less.
  - (c) Run maximize and restore, Win+Left and Win+Right snaps, minimize and restore, and hide to the tray and show again. After each, the child's logged bounds match the placeholder's to within 1 px.
  - (d) The user sets Windows' display scale to 125%, then 150%, while the app runs, then back to 100%. At each scale, run the (c) bounds check and take a capture, and the user redoes (a)'s right-edge drag.

  Write each result into S12.

- [ ] **Step 5: Commit the code, then the S12 rows**

### Task 5: The overlay freeze

**Files:**
- Modify: `src-tauri/src/spike_browser/com.rs`, `mod.rs`; `src/features/spike/BrowserSpike.tsx`, `BrowserSpike.css`
- Modify: `src-tauri/Cargo.toml` (only `features` on the `windows` crate, if the IStream calls need them)

**Interfaces:**
- Produces `spike_browser_capture() -> String`, a `data:image/png;base64,…` URL from the child's `CapturePreview` into an in-memory `IStream`, and `window.__spike.setFreeze(on: boolean)` (on by default).

- [ ] **Step 1: Wire the freeze.** When `frozen` turns true:
  - the front logs `freeze-start`;
  - it awaits `spike_browser_capture` and logs `captured`;
  - it puts the image over the placeholder (`object-fit: fill`), then awaits `img.decode()` and one animation frame, and logs `painted`;
  - only then does it call `spike_browser_visible(false)` and log `hidden`.

  When `frozen` turns false, it shows the child, waits one frame, removes the image and logs `thawed`. A capture that fails leaves the page hidden behind a plain fill, and logs `capture-failed` with the HRESULT.

- [ ] **Step 2: Run the freeze checks.**
  - (a) Ctrl+Shift+K from the page and from the terminal, 10 times each. Log `freeze-start` to `hidden` and `thawed`. The user watches the flash at normal speed and says whether it is acceptable.
  - (b) With `__spike.setFreeze(false)`, the palette is hidden behind the page, as the research says (tauri#15682).
  - (c) Click a link, then press Ctrl+Shift+K at once, 5 times. Record whether the picture shows the old page or the new one.
  - (d) Freeze before the first page has loaded: `open` then an immediate freeze. Record the HRESULT.

  Write each result into S12.

- [ ] **Step 3: Commit the code, then the S12 rows**

### Task 6: Navigation, popups and the profile

**Files:**
- Modify: `src-tauri/src/spike_browser/com.rs`, `mod.rs`; `src/features/spike/BrowserSpike.tsx`

**Interfaces:**
- Produces:
  - `spike_browser_nav(action: String, url: Option<String>)`, where the action is `"go"`, `"back"`, `"forward"` or `"reload"`, through `GoBack`, `GoForward`, `Reload` and `Navigate`;
  - the event `spike://history { url, title, canBack, canForward }`, sent from `HistoryChanged`, `SourceChanged` and `DocumentTitleChanged`;
  - the event `spike://popup { url }`.

- [ ] **Step 1: Wire it up.**
  - The bar's buttons follow `canBack` and `canForward`, and the address box follows `url`.
  - `on_new_window` logs the URL, emits `spike://popup`, navigates the child to it (one tab stands in for "a popup opens as a new tab") and returns `NewWindowResponse::Deny`.

- [ ] **Step 2: Run the navigation checks.**
  - (a) Go through three site pages, then Back twice, Forward once and Reload. The address box and the buttons follow each step, and the site logs the reload.
  - (b) The `target=_blank` link logs `popup` and opens in the pane. No new window appears (EnumWindows).
  - (c) `/cookie` sets `spike=<time>; Max-Age=86400` and shows the cookie it received. Restart the app with `touch src-tauri/src/spike_browser/mod.rs`, and `tauri dev` rebuilds it. Re-open `/cookie`; it shows the old value. Before the new start, no `msedgewebview2.exe` from the old run still holds the profile (CIM command lines).
  - (d) A real public page (`https://example.com/`) loads and renders.

  Write each result into S12.

- [ ] **Step 3: Commit the code, then the S12 rows**

### Task 7: Isolation

**Files:**
- Modify: `spikes/browser-pane/probe.html`
- Modify: `src-tauri/capabilities/default.json` (6q-0 decision 1; in its own commit, after the baseline run)

**Interfaces:**
- `probe.html` POSTs `{ origin, hasInternals, invokeVersion, apiStatus }` to `http://127.0.0.1:47620/report` (`mode: "no-cors"`), where:
  - `hasInternals` is whether `window.__TAURI_INTERNALS__` exists;
  - `invokeVersion` is the result or error of `__TAURI_INTERNALS__.invoke("plugin:app|version")`;
  - `apiStatus` is the status or error of `fetch` to the local API's `/v1/health` on the port passed as `?api=<port>`. The port is read from `endpoint.json`, never the token.

- [ ] **Step 1: The baseline, on the `windows` capability.** Open the probe at the site (`http://127.0.0.1:47620/probe?api=<port>`). Open it again at Vite's origin (`http://localhost:1420/spikes/browser-pane/probe.html?api=<port>`), which counts as the app's own origin in dev. Record both reports. The research expects the Vite-origin probe to get the version (authority.rs:462-489).

- [ ] **Step 2: Scope the capability to `"webviews": ["main"]`, and commit only that file**

```bash
git add src-tauri/capabilities/default.json
git commit -m "spike(browser): the default capability names the app's webview, not its window"
```

- [ ] **Step 3: Run the probes again, and check that the app works.** Both probes must fail to get the version, and the API must refuse with no token. The app's own window still minimizes, maximizes, closes to the tray and opens a link. Run the allow-list checks from the address bar: `file:///C:/Windows/win.ini`, `http://tauri.localhost/`, `http://ipc.localhost/` and `tauri://localhost/` are refused, and `about:blank` and `http://localhost:47620/` open.

  Write each result into S12, then commit the S12 rows.

### Task 8: File drop

**Files:**
- Modify: `src-tauri/src/spike_browser/mod.rs` (log the drag-and-drop events for each webview label)

- [ ] **Step 1: Log every `DragDrop` event** with its webview label and position. It already reaches the main webview; add the child's.

- [ ] **Step 2: Run the drop checks.** The user drags a `.txt` file from Explorer:
  - (a) onto the terminal: Marshell's existing drop still works as it does on `main`;
  - (b) onto the page: record which events fire and what the page does (a navigation to `file:` must be refused);
  - (c) onto the pane's bar.

  Write what 6q-2's "drop a file on the pane" can build on into S12.

- [ ] **Step 3: Commit the code, then the S12 rows**

### Task 9: Findings, the recommendation and the user's go or no-go

**Files:**
- Modify: `docs/spikes/S12-browser-pane.md`, `docs/PLAN.md` (§9, 6q-0 decision 6, after the user answers)

- [ ] **Step 1: Stop the live check.** Run `tree`, and stop the app and the test site by PID. Then check:
  - no `msedgewebview2.exe` with the scratch profile remains;
  - `~/.claude/settings.json`'s modified time is unchanged.

  Don't delete the scratch folder yet: list it for the user at the end.

- [ ] **Step 2: Finish S12.** Fill in:
  - each check's result;
  - the numbered findings, each with what failed, why and the fix or workaround;
  - "What 6q-2 and 6q-3 take from this": the edge inset, bounds mode, browser arguments, key route table, focus route, freeze timings and drop behaviour;
  - a recommendation (go, go with limits, or no-go), with the reasons.

  Commit it.

- [ ] **Step 3: Push the spike branch.** `git push -u origin phase-6q-0`. There is no PR for the code.

- [ ] **Step 4: Ask the user for the go or no-go** with AskUserQuestion, giving S12's recommendation first. Record the answer as 6q-0 decision 6 in `docs/PLAN.md` §9.

- [ ] **Step 5: Open the docs PR.**
  - Cut `docs/6q-0-spike` from `origin/main`.
  - Cherry-pick the docs commits: Task 0, every S12 commit, and decision 6.
  - Run `pnpm test` and `pnpm build`; docs don't change them, but CI runs them.
  - Open the PR with "What it does", "Live check", "Known limits" and "Checks" sections, then merge it on green, following `marshell-ship`.

### Task 10: Hide the late redraw, and the terminal's own flicker (6q-0 decision 7)

Added 2026-11-09, after the research round (S12 finding 23). Task 9's go or no-go waits for this task's hand checks.

**Files:**
- Create: `src-tauri/src/spike_browser/clip.rs`
- Modify: `src-tauri/src/spike_browser/mod.rs`, `src-tauri/src/main.rs` (two commands), `src/features/spike/useBrowserSpike.ts`, `src/features/terminal/TerminalView.tsx`, `docs/spikes/S12-browser-pane.md`

- [ ] **Step 1: Oversize and clip (Rust).** `__spike.setOversize(px)` sends `spike_browser_oversize`; 0, the default, keeps today's `set_bounds`. With px > 0, `set_bounds` hands the pane's rect V to `clip::place`, inline on the main thread through `with_webview`:
  - V in physical pixels goes to wry's container HWND (`controller.ParentWindow()`) with a synchronous `SetWindowPos`. The container already clips its child (`WS_CHILD | WS_CLIPCHILDREN`, wry `webview2/mod.rs:230`).
  - The controller's size C changes only when V outgrows it, per axis: C = V + px. A shrinking pane keeps C, so the old frame and its scrollbar stay outside the crop.
  - Every placement bumps a counter and starts a 200 ms timer thread; when it fires with the counter unchanged, C = V on the main thread ("clip-settle").
  - Each placement logs `clip` with V, C and why.

- [ ] **Step 2: Background colours (Rust).** `__spike.setBg({ window, webview })` sends `spike_bg` with `#rrggbb` or `"none"` for either layer: `Window::set_background_color` on the main window (tao's WM_ERASEBKGND fill) and `Webview::set_background_color` on the main webview (WebView2's DefaultBackgroundColor). Runtime only; `tauri.conf.json` doesn't change.

- [ ] **Step 3: The terminal's sync render (front).** `__spike.setXterm(on)`: when on, TerminalView's ResizeObserver fits at once instead of one animation frame later, and when the fit changed the grid it calls xterm's private `_core._renderService._renderRows(0, rows - 1)`, so the canvas xterm 6.0 just cleared is drawn before the frame is shown (upstream PR #5529 does this in 6.1). Off is today's behaviour.

- [ ] **Step 4: Checks.** `cargo test -p marshell-app -j 4 spike_browser`, clippy, `pnpm test`, `pnpm build` (`tsc --noEmit` only; never an emitting `tsc`).

- [ ] **Step 5: Live check, judged by the user's eye, each A/B in one session:**
  - **10a, the terminal:** `setXterm(false)` then `true`; drag the divider between the shell and the page, the divider by the sidebar, and a window edge. Does the shell's text still flicker?
  - **10b, the white strip:** `setBg({ window: "#ff00ff" })`, then `setBg({ webview: "#00ffff" })`; a bottom-edge drag each time. Which colour fills the strip? Then both layers set to the app's background.
  - **10c, the page:** `setOversize(0)` then `setOversize(64)` (then 128 if 64 trails); drag the page divider both ways and the right and bottom window edges. Do the scrollbar jump and the edge flicker go, and does the page settle to the right size?

- [ ] **Step 6: Record.** S12 finding 23 (the research and what the hand checks showed), the 6q-2 section, and the go-or-no-go limits updated. Commit, push, then Task 9 Step 4.
