# A rolled Ctrl+Shift+Tab: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Ctrl+Shift+Tab whose Tab lands a few milliseconds before Shift runs Ctrl+Shift+Tab, not Ctrl+Tab ("Next tab"), in the app and in a page.

**Architecture:** Ctrl+Tab is a prefix of the navigation chord Ctrl+Shift+Tab, so a fresh Ctrl+Tab is held until Tab's release or 50 ms, and becomes Ctrl+Shift+Tab if Shift is in by then.
- **In the app:** a window capture listener holds the real key press, then dispatches the settled chord as a synthetic `keydown` to the same target. Every existing handler (xterm's filter, the nav layer, `LiveApp`'s shortcuts, Settings, dialogs) then runs unchanged. The app sees Shift go down and settles at once.
- **In a page:** the shell's `AcceleratorKeyPressed` handler holds the key before forwarding it. WebView2 never reports Shift there, so the shell reads Shift's state when the hold ends.
- **Both sides:** the rules are pure functions. The app passes forwarded (untrusted) keys through without holding them again.

**Tech Stack:** React 19 and TypeScript with Vitest (node, no DOM); Rust in `src-tauri` (`marshell-app`, edition 2024) with WebView2 COM through `webview2-com`.

**Spec:** `docs/PLAN.md` §9, "Slice 6q-3 decisions", 14 (last bullet, the user's ruling) and 15 (what was decided without asking). Evidence: `docs/research/2026-11-10-browser-tab-live-check.md`, finding 30.

## Global Constraints

- Branch `ctrl-tab-rollover`, cut from `origin/main` at `8e4876aa`, in the main checkout. Ledger: `.superpowers/sdd/2026-11-13-ctrl-tab-rollover/progress.md`. Built inline, no workers (decision 15).
- The hold is 50 ms on both sides: `TAB_HOLD_MS = 50` in TypeScript, `TAB_HOLD = Duration::from_millis(50)` in Rust.
- A held key is a fresh Ctrl+Tab only: code `Tab`, Ctrl down, Shift, Alt and Meta up, not a repeat. The settled chord is Ctrl+Shift+Tab only with Ctrl and Shift down and Alt (and Meta) up; otherwise it is Ctrl+Tab.
- A hold ends unsent when the window (app) or the page (shell) loses the keyboard.
- The app holds only trusted (`isTrusted`) events. A page's forwarded key and the hold's own re-dispatch are untrusted and pass untouched.
- The shell holds only a Ctrl+Tab that `route_key` sends to the app.
- Rust: never hold the hold's `Mutex` across a call out (`post`, `focus_app`, `emit`, COM). Take the result into a local first. In edition 2024 the guard in `if let Some(x) = lock(..).f()` lives through the whole block.
- No new dependency, no capability, CSP or `tauri.conf.json` change.
- Tests fail first, except pins of existing behaviour. Frontend: `pnpm test <file>`. Rust: `cargo test -p marshell-app -j 4 browser::rules`. Before the PR, what CI runs (`marshell-ship`).
- Commit messages read `type(scope): what changed, as a sentence.` and end with the session's trailers. The rustfmt hook formats Rust on every edit.

## Review Focus

1. **The shell never sees Shift go down** (WebView2 doesn't report it). If the end-of-hold Shift read is wrong or stale, pages keep stepping tabs while every Rust test passes. Task 3 tests the rule with the state passed in. The page-side check (Task 5) logs each `AcceleratorKeyPressed` with the Shift state read, and drives a timed rolled chord.
2. **A page's forwarded Ctrl+Tab held a second time by the app:** that would add another 50 ms, and a real Shift landing in the app after focus moved there would convert it again. Task 2 pins it ("an untrusted Ctrl+Tab passes untouched").
3. **A re-entrant key event while the shell forwards** (`focus_app` is a cross-process COM call that can pump messages): it must not deadlock on the hold's lock. Task 4's constraint and code review; the page-side check rolls the chord repeatedly.
4. **Holding Ctrl+Tab down (auto-repeat):** exactly one tab step, as today. Tasks 1 to 3 pin it (a repeat during the hold is swallowed with no second send).
5. **Ctrl released, then Shift pressed, within 50 ms:** that's Ctrl+Tab, not Ctrl+Shift+Tab. Tasks 1 and 3 pin it.

---

### Task 1: The app's hold rules (pure)

**Files:**
- Create: `src/lib/navigation/tabHold.ts`
- Test: `src/lib/navigation/tabHold.test.ts`

**Interfaces:**
- Produces:
  - `export const TAB_HOLD_MS = 50;`
  - `export type HoldKey = { type: "keydown" | "keyup"; code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean; repeat: boolean };`
  - `export type HoldStep = { held: boolean; release: { shift: boolean } | null; swallow: boolean };`. `release` sends the held chord now (`shift: true` for Ctrl+Shift+Tab). `swallow` keeps this key from every other handler.
  - `export function tabHoldStep(held: boolean, e: HoldKey): HoldStep;`

- [ ] **Step 1: Write the failing tests** in `tabHold.test.ts`, with a `key(type, code, mods)` helper:
  - "a fresh Ctrl+Tab is held and kept from every other handler": `tabHoldStep(false, Ctrl+Tab down)` → `{ held: true, release: null, swallow: true }`.
  - "Ctrl+Shift+Tab, Ctrl+Alt+Tab, plain Tab and a repeated Ctrl+Tab aren't held": each → `{ held: false, release: null, swallow: false }`.
  - "Shift with Ctrl during the hold sends Ctrl+Shift+Tab and goes on itself": `ShiftLeft` and `ShiftRight` down with ctrl+shift → `{ held: false, release: { shift: true }, swallow: false }`.
  - "Shift without Ctrl, or with Alt, sends Ctrl+Tab": → `release: { shift: false }`, `held: false`, `swallow: false`.
  - "Tab's release sends the chord its modifiers make": `keyup Tab` with ctrl → `release: { shift: false }`; with ctrl+shift → `release: { shift: true }`; `held: false`, `swallow: false`.
  - "a repeat during the hold is swallowed": Ctrl+Tab down with `repeat: true` while held → `{ held: true, release: null, swallow: true }`.
  - "another key during the hold sends Ctrl+Tab first and goes on itself": `KeyW` down with ctrl → `{ held: false, release: { shift: false }, swallow: false }`.
  - "Ctrl's release during the hold keeps waiting": `keyup ControlLeft` while held → `{ held: true, release: null, swallow: false }`.
  - "a key-up with nothing held does nothing": → `{ held: false, release: null, swallow: false }`.

- [ ] **Step 2: Run, expect FAIL** (module missing): `pnpm test src/lib/navigation/tabHold.test.ts`

- [ ] **Step 3: Implement `tabHoldStep`** in `tabHold.ts`, with a doc comment citing PLAN §9 "Slice 6q-3 decisions" 14 and 15.

- [ ] **Step 4: Run, expect PASS**: `pnpm test src/lib/navigation/tabHold.test.ts`

- [ ] **Step 5: Commit** `feat(navigation): a fresh Ctrl+Tab waits for Tab's release or 50 ms, and Shift meanwhile makes it Ctrl+Shift+Tab.`

### Task 2: The app's hold, installed on the window

**Files:**
- Modify: `src/lib/navigation/tabHold.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (one effect beside the `useNavigation` call, ~line 1349)
- Test: `src/lib/navigation/tabHold.test.ts`, `src/lib/navigation/keys.test.ts`

**Interfaces:**
- Consumes: `tabHoldStep`, `TAB_HOLD_MS` (Task 1).
- Produces:
  - `export type HoldWindow = Pick<Window, "addEventListener" | "removeEventListener">;` The tests' fake implements the three calls it uses: `keydown` and `keyup` in the capture phase, and `blur`.
  - `export function installTabHold(win: HoldWindow, send: (target: EventTarget | null, shift: boolean) => void): () => void;`
    - It reads only `isTrusted` key events.
    - On `swallow` it calls `preventDefault()` and `stopImmediatePropagation()`.
    - On a hold it keeps `e.target` and starts a `TAB_HOLD_MS` timer.
    - On a release it clears the timer and calls `send(target, shift)` before the current key goes on. The timer sends `(target, false)`.
    - A window `blur` drops a hold unsent.
    - The returned function removes the listeners and drops a held key.
  - `export function sendHeldTab(target: EventTarget | null, shift: boolean): void;` dispatches `new KeyboardEvent("keydown", { code: "Tab", key: "Tab", ctrlKey: true, shiftKey: shift, bubbles: true, cancelable: true })`. The target is `target` when it is a connected `Node`, or else `document.activeElement ?? document.body`.

- [ ] **Step 1: Write the failing tests.** Use a fake `HoldWindow` that records listeners, plain event objects (`type`, `isTrusted`, `target`, `preventDefault: vi.fn()`, `stopImmediatePropagation: vi.fn()`), `vi.useFakeTimers()`, and `send = vi.fn()`:
  - "a held Ctrl+Tab reaches send after 50 ms, at the key's target": after 49 ms, no `send`; after 1 ms more, one call with `(target, false)`. The press had `preventDefault` and `stopImmediatePropagation` called.
  - "Shift 8 ms after Tab sends Ctrl+Shift+Tab once, and the timer sends nothing more": one call with `(target, true)`, still one after running all timers. The Shift's `stopImmediatePropagation` wasn't called.
  - "Tab's release sends at once": `keyup Tab` at 20 ms → one call with `(target, false)`, none after the timers.
  - "another key sends Ctrl+Tab first and isn't stopped": `KeyW` down → `send` was called before the listener returned, and `KeyW`'s `stopImmediatePropagation` wasn't called.
  - "a repeat during the hold is stopped and sends nothing more": the repeat's `preventDefault` and `stopImmediatePropagation` were called; after the timers, one call in all.
  - "Ctrl's release keeps waiting": `keyup ControlLeft` at 10 ms → no call until 50 ms.
  - "a blur drops the hold": blur at 10 ms → no call after the timers.
  - "an untrusted Ctrl+Tab passes untouched": `isTrusted: false` → no `preventDefault`, and no `send` after the timers.
  - "uninstalling drops a held key and removes the listeners": install, press, uninstall, run the timers → no `send`; the fake has no listeners left.
  - In `keys.test.ts`, a pin that listener order doesn't matter: "Shift with the modifier already held gives nothing" (`down(ev("ShiftLeft", CS), "terminal", held())` → `null`). Ctrl+Shift+Tab not held → `[modifier-down, tab]`, and held → `[tab]`, are already pinned at lines 46 and 55.

- [ ] **Step 2: Run, expect FAIL**: `pnpm test src/lib/navigation/tabHold.test.ts`

- [ ] **Step 3: Implement** `installTabHold` and `sendHeldTab`. Wire them in `LiveApp.tsx` with `useEffect(() => installTabHold(window, sendHeldTab), []);` and a comment naming the decision.

- [ ] **Step 4: Run, expect PASS**: `pnpm test src/lib/navigation src/features/sessions src/lib`

- [ ] **Step 5: Commit** `feat(sessions): the window holds a real Ctrl+Tab and sends on Ctrl+Tab or Ctrl+Shift+Tab to the same element.`

### Task 3: The shell's hold rules (pure)

**Files:**
- Modify: `src-tauri/src/browser/rules.rs` (beside `route_key` and `key_event`; tests in its `mod tests`, which already has `VK_TAB` and `claimed()`)

**Interfaces:**
- Produces:
  - `pub const VK_TAB: u32 = 0x09;`, moved from the tests to the module. The tests add `const VK_CONTROL: u32 = 0x11;` for themselves, which keeps the name apart from the `windows` crate's `VK_CONTROL` that `com.rs` uses.
  - `pub const TAB_HOLD: Duration = Duration::from_millis(50);` (add `use std::time::Duration`).
  - `#[derive(Debug, Default)] pub struct TabHold { held: Option<u64>, next: u64 }`
  - `#[derive(Debug, Default, PartialEq, Eq)] pub struct HoldStep { pub release: Option<KeyEvent>, pub taken: bool, pub started: Option<u64> }`. `release` is the chord to forward now. `taken` means the key is the hold's: keep it from the page and skip its routing. `started` means a hold began, so call `timeout` with this after `TAB_HOLD`.
  - `impl TabHold`:
    - `pub fn down(&mut self, vk: u32, ctrl: bool, shift: bool, alt: bool, repeat: bool, route: KeyRoute) -> HoldStep`
    - `pub fn up(&mut self, vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<KeyEvent>`
    - `pub fn timeout(&mut self, hold: u64, ctrl: bool, shift: bool, alt: bool) -> Option<KeyEvent>`
    - `pub fn drop_hold(&mut self)`
  - The modifiers are the state when the hold ends. The released chord is `KeyEvent { code: "Tab", ctrl: true, shift: ctrl && shift && !alt, alt: false, repeat: false }`.
  - While held, a key-down other than a repeated Tab releases with that event's modifiers and is not `taken`. This also covers a Shift key-down, should WebView2 ever report one.

- [ ] **Step 1: Write the failing tests:**
  - `a_claimed_ctrl_tab_from_a_page_is_held`: `down(VK_TAB, true, false, false, false, KeyRoute::App)` → `taken`, `started.is_some()`, no `release`.
  - `an_unclaimed_ctrl_tab_or_another_chord_is_not_held`: `KeyRoute::Page`, Ctrl+Shift+Tab, Ctrl+Alt+Tab and a repeat each → `HoldStep::default()`.
  - `tabs_release_with_shift_in_sends_ctrl_shift_tab`: `up(VK_TAB, true, true, false)` → Ctrl+Shift+Tab; again → `None`.
  - `tabs_release_without_shift_or_ctrl_sends_ctrl_tab`: `up(VK_TAB, true, false, false)` and, in a new hold, `up(VK_TAB, false, true, false)` → Ctrl+Tab.
  - `the_timeout_reads_shift_and_sends_once_and_a_stale_timeout_nothing`:
    - `timeout(n, true, true, false)` → Ctrl+Shift+Tab, and again → `None`.
    - For a second hold `m`: `timeout(n, ..)` → `None`, and `timeout(m, true, false, false)` → Ctrl+Tab.
  - `ctrls_release_keeps_the_hold`: `up(VK_CONTROL, false, false, false)` → `None`, then `up(VK_TAB, ..)` releases.
  - `a_repeat_during_the_hold_is_taken_and_sends_nothing`: → `taken`, no `release`, still held.
  - `another_key_during_the_hold_sends_the_chord_and_is_not_taken`: `down(VK_L, true, false, false, false, ..)` → `release` is Ctrl+Tab, not `taken`. With `shift: true` → Ctrl+Shift+Tab.
  - `a_dropped_hold_sends_nothing`: `drop_hold()`, then `timeout(n, ..)` and `up(VK_TAB, ..)` → `None`.

- [ ] **Step 2: Run, expect FAIL** (no `TabHold`): `cargo test -p marshell-app -j 4 browser::rules`

- [ ] **Step 3: Implement `TabHold`**, with a doc comment citing the decisions and the WebView2 Shift limit.

- [ ] **Step 4: Run, expect PASS**: `cargo test -p marshell-app -j 4 browser::rules`

- [ ] **Step 5: Commit** `feat(browser): the rules hold a page's Ctrl+Tab for Tab's release or 50 ms, and read Shift when it ends.`

### Task 4: The shell holds a page's Ctrl+Tab

**Files:**
- Modify: `src-tauri/src/browser/com.rs` (`install`, `focus_handlers`, `key_handler`, a key-state helper beside `down()`)

**Interfaces:**
- Consumes: `TabHold`, `HoldStep`, `TAB_HOLD` (Task 3); `post`, `emit`, `focus_app` and `PageKeyEvent` from `browser/mod.rs`.

- [ ] **Step 1: Wire it.**
  - `install` makes one `Arc<Mutex<TabHold>>` per page and passes it to `focus_handlers` and `key_handler`. `LostFocus` calls `drop_hold()`.
  - Factor the existing App branch's job into `fn forward(app: &AppHandle, key: &str, event: KeyEvent)` (`focus_app`, then `emit` of `browser://key`).
  - Add `fn held_now(vk: u16) -> bool` with `GetAsyncKeyState`, for the timer. `down()` (`GetKeyState`) stays for events.
  - **Key-up:** `let released = lock(&hold).up(vk, ctrl, shift, alt);`, with the modifiers from `down()`. Post `forward` for `Some`. Then the existing `taken` removal.
  - **Key-down:** after `route`, `let step = lock(&hold).down(vk, ctrl, shift, alt, WasKeyDown, route);`
    - A `release` posts `forward`.
    - `started: Some(n)` spawns a thread that sleeps `TAB_HOLD` and then `post`s a job. The job runs `let released = lock(&hold).timeout(n, held_now(VK_CONTROL), held_now(VK_SHIFT), held_now(VK_MENU));`, then `forward` for `Some`.
    - `taken` inserts `vk` into `taken`, calls `SetHandled(true)` and returns.
    - Otherwise the existing routing runs.
  - No lock is held across `post`, `forward` or a COM call (Global Constraints). Comments say why the hold exists, why Shift is read at the end, and that the job runs on the main thread like every other callback.

- [ ] **Step 2: Build and lint**: `cargo clippy -p marshell-app --all-targets -j 4 -- -D warnings` is clean, and `cargo test -p marshell-app -j 4` passes.

- [ ] **Step 3: Commit** `feat(browser): a page's Ctrl+Tab waits in the shell, so a rolled Ctrl+Shift+Tab reaches the app as itself.`

### Task 5: Live check (app by CDP; page by real keys, with the user)

- [ ] **Step 1: App side, unattended,** with the `marshell-live-check` skill (scratch homes, CDP, stop only what it started). Drive the keys with `browser_run_code_unsafe` and `page.keyboard.down/up` and `waitForTimeout`, which gives trusted CDP key presses. An injected listener logs each key's `event.timeStamp`, to show each gap stayed under 50 ms. Pass when:
  - **Rolled, timer path:** Ctrl, Tab, Shift 8 ms later, Tab held 80 ms → navigation runs ("Selected the side pane"), and the tab doesn't step.
  - **Rolled, release path:** Ctrl, Tab, Shift 8 ms later, Tab up at 20 ms → the same.
  - **Plain Ctrl+Tab:** steps the tab within 50 ms, with the terminal focused and with the pane focused.
  - **Auto-repeat:** a second `down("Tab")` while held steps exactly one tab.
  - **Settings:** Ctrl+Tab in Settings moves focus as before.
- [ ] **Step 2: Page side, together with the user.** Ask first. Use keyboard `SendInput` (real input, so it goes through `AcceleratorKeyPressed`). Before each send, check that the foreground window belongs to the dev app's process and that the page has the keys, and abort otherwise. Send Ctrl down, Tab down, Shift down 8–15 ms later, then the releases, ten times.
  - A temporary, uncommitted `eprintln!` in `key_handler` logs kind, vk, ctrl and the Shift state read at each hold's end.
  - Load a local key-logger page served on loopback from the check's scratch folder, to see which keys the page got.
  - Pass when navigation runs every time and the tab never steps.
  - A plain Ctrl+Tab from the page steps the tab.
  - The user then rolls the chord by hand a few times.
- [ ] **Step 3:** Record both in `docs/research/2026-11-13-ctrl-tab-rollover-live-check.md`, including whether WebView2 reported Shift. Mark finding 30 fixed in PLAN §9 decision 15. Remove the temporary log. Commit `docs(research): …`.

### Task 6: Review and ship

- [ ] **Step 1:** Whole-branch review by the `marshell-reviewer` agent (Opus) with this plan's Review Focus, then one fix pass.
- [ ] **Step 2:** `marshell-ship`: CI's commands, PR, green CI, merge, ledger deleted.
