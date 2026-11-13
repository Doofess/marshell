# A rolled Ctrl+Shift+Tab: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Ctrl+Shift+Tab whose Tab lands a few milliseconds before Shift runs Ctrl+Shift+Tab, not Ctrl+Tab ("Next tab"), in the app and in a page.

**Architecture:** Ctrl+Tab is a prefix of the navigation chord Ctrl+Shift+Tab, so a fresh Ctrl+Tab is held until Tab's release or 50 ms, and becomes Ctrl+Shift+Tab if Shift goes down meanwhile. In the app a window capture listener holds the real key press and then dispatches the settled chord as a synthetic `keydown` to the same target, so every existing handler (xterm's filter, the nav layer, `LiveApp`'s shortcuts, Settings, dialogs) runs unchanged. In a page the shell's `AcceleratorKeyPressed` handler holds the key with the same rules before forwarding it, and the app passes forwarded (untrusted) keys through without holding them again. The rules are pure functions on both sides.

**Tech Stack:** React 19 and TypeScript with Vitest (node, no DOM); Rust in `src-tauri` (`marshell-app`) with WebView2 COM through `webview2-com`.

**Spec:** `docs/PLAN.md` §9, "Slice 6q-3 decisions", 14 (last bullet, the user's ruling) and 15 (what was decided without asking). Evidence: `docs/research/2026-11-10-browser-tab-live-check.md`, finding 30.

## Global Constraints

- Branch `ctrl-tab-rollover`, cut from `origin/main` at `8e4876aa`, in the main checkout. Ledger: `.superpowers/sdd/2026-11-13-ctrl-tab-rollover/progress.md`. Built inline, no workers (decision 15).
- The hold is 50 ms on both sides: `TAB_HOLD_MS = 50` in TypeScript, `TAB_HOLD = Duration::from_millis(50)` in Rust.
- A held key is a fresh Ctrl+Tab only: code `Tab`, Ctrl down, Shift, Alt and Meta up, not a repeat. Shift ends the hold as Ctrl+Shift+Tab only with Ctrl down and Alt (and Meta) up.
- The app holds only trusted (`isTrusted`) events. A page's forwarded key and the hold's own re-dispatch are untrusted and pass untouched.
- The shell holds only a Ctrl+Tab that `route_key` sends to the app.
- No new dependency, no capability, CSP or `tauri.conf.json` change.
- Tests fail first. Frontend: `pnpm test <file>`. Rust: `cargo test -p marshell-app -j 4 browser::rules`. Before the PR, what CI runs (`marshell-ship`).
- Commit messages read `type(scope): what changed, as a sentence.` and end with the session's trailers. The rustfmt hook formats Rust on every edit.

## Review Focus

1. **A page's forwarded Ctrl+Tab held a second time by the app:** that would add another 50 ms, and a real Shift landing in the app after focus moved there would convert it again. Forwarded keys must pass untouched. Task 2 pins it ("an untrusted Ctrl+Tab passes untouched").
2. **Holding Ctrl+Tab down (auto-repeat):** exactly one tab step, as today. Task 1 and Task 3 pin it (a repeat during the hold is dropped).
3. **Ctrl released, then Shift pressed, within 50 ms:** that's Ctrl+Tab then Shift, not Ctrl+Shift+Tab. Task 1 and Task 3 pin it (Shift without Ctrl releases Ctrl+Tab).
4. **Ctrl+Tab where it moves focus like Tab** (Settings, a dialog's focus trap): it must still move focus after the hold, through React's handlers, on a synthetic event. The live check (Task 5) presses it in Settings.
5. **The Shift the shell took from a page:** the page must not see a Shift down without its up. Task 4 puts the Shift in `taken`, so its key-up is handled too. The hand check (Task 5) rolls the chord in a page with a text field focused.

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
  - "Tab's release sends Ctrl+Tab": `keyup Tab` while held → `{ held: false, release: { shift: false }, swallow: false }`.
  - "a repeat during the hold is dropped": Ctrl+Tab down with `repeat: true` while held → `{ held: true, release: null, swallow: true }`.
  - "another key during the hold sends Ctrl+Tab first and goes on itself": `KeyW` down with ctrl → `{ held: false, release: { shift: false }, swallow: false }`.
  - "Ctrl's release during the hold keeps waiting": `keyup ControlLeft` while held → `{ held: true, release: null, swallow: false }`.
  - "a key-up with nothing held does nothing": → `{ held: false, release: null, swallow: false }`.

- [ ] **Step 2: Run, expect FAIL** (module missing): `pnpm test src/lib/navigation/tabHold.test.ts`

- [ ] **Step 3: Implement `tabHoldStep`** in `tabHold.ts`. The doc comment cites PLAN §9 "Slice 6q-3 decisions" 14 and 15.

- [ ] **Step 4: Run, expect PASS**: `pnpm test src/lib/navigation/tabHold.test.ts`

- [ ] **Step 5: Commit** `feat(navigation): a fresh Ctrl+Tab waits for Tab's release or 50 ms, and Shift meanwhile makes it Ctrl+Shift+Tab.`

### Task 2: The app's hold, installed on the window

**Files:**
- Modify: `src/lib/navigation/tabHold.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (one effect beside the `useNavigation` call, ~line 1349)
- Test: `src/lib/navigation/tabHold.test.ts`

**Interfaces:**
- Consumes: `tabHoldStep`, `TAB_HOLD_MS` (Task 1).
- Produces:
  - `export type HoldWindow = { addEventListener(type: "keydown" | "keyup", fn: (e: KeyboardEvent) => void, capture: true): void; removeEventListener(type: "keydown" | "keyup", fn: (e: KeyboardEvent) => void, capture: true): void };`
  - `export function installTabHold(win: HoldWindow, send: (target: EventTarget | null, shift: boolean) => void): () => void;`. It listens for `keydown` and `keyup` in the capture phase and reads only `isTrusted` events. On `swallow` it calls `preventDefault()` and `stopImmediatePropagation()`. On a hold it keeps `e.target` and starts a `TAB_HOLD_MS` timer. On a release it clears the timer and calls `send(target, shift)` before the current key goes on. The timer sends `(target, false)`. The returned function removes the listeners and clears the timer, dropping a held key.
  - `export function sendHeldTab(target: EventTarget | null, shift: boolean): void;`. It dispatches `new KeyboardEvent("keydown", { code: "Tab", key: "Tab", ctrlKey: true, shiftKey: shift, bubbles: true, cancelable: true })` on `target` when it is a connected `Node`, or else on `document.activeElement ?? document.body`.

- [ ] **Step 1: Write the failing tests.** Use a fake `HoldWindow` that records listeners, plain event objects (`isTrusted`, `target`, `preventDefault: vi.fn()`, `stopImmediatePropagation: vi.fn()`), `vi.useFakeTimers()`, and `send = vi.fn()`:
  - "a held Ctrl+Tab reaches send after 50 ms, at the key's target": after 49 ms `send` has no calls; after 1 ms more it was called once with `(target, false)`. The press had `preventDefault` and `stopImmediatePropagation` called.
  - "Shift 8 ms after Tab sends Ctrl+Shift+Tab once, and the timer sends nothing more": `send` was called once with `(target, true)`, also after running all timers. The Shift's `stopImmediatePropagation` wasn't called.
  - "an untrusted Ctrl+Tab passes untouched": `isTrusted: false` → no `preventDefault`, and no `send` after the timers run.
  - "uninstalling drops a held key and removes the listeners": install, press Ctrl+Tab, uninstall, run the timers → no `send`; the fake has no listeners left.

- [ ] **Step 2: Run, expect FAIL**: `pnpm test src/lib/navigation/tabHold.test.ts`

- [ ] **Step 3: Implement** `installTabHold` and `sendHeldTab`. Wire them in `LiveApp.tsx` with `useEffect(() => installTabHold(window, sendHeldTab), []);` and a comment naming the decision.

- [ ] **Step 4: Run, expect PASS**: `pnpm test src/lib/navigation/tabHold.test.ts src/features/sessions src/lib`

- [ ] **Step 5: Commit** `feat(sessions): the window holds a real Ctrl+Tab and sends on Ctrl+Tab or Ctrl+Shift+Tab to the same element.`

### Task 3: The shell's hold rules (pure)

**Files:**
- Modify: `src-tauri/src/browser/rules.rs` (beside `route_key` and `key_event`; tests in its `mod tests`, which already has `VK_TAB` and `claimed()`)

**Interfaces:**
- Produces:
  - `pub const VK_TAB: u32 = 0x09;` and `pub const VK_SHIFT: u32 = 0x10;`, `VK_LSHIFT: u32 = 0xA0;`, `VK_RSHIFT: u32 = 0xA1;` (move the tests' `VK_TAB` to the module).
  - `pub const TAB_HOLD: Duration = Duration::from_millis(50);`
  - `#[derive(Debug, Default)] pub struct TabHold { held: Option<u64>, next: u64 }`
  - `#[derive(Debug, Default, PartialEq, Eq)] pub struct HoldStep { pub release: Option<KeyEvent>, pub taken: bool, pub started: Option<u64> }`. `release` is the chord to forward now. `taken` means the key is the hold's: keep it from the page and skip its routing. `started` means a hold began, so call `timeout` with this after `TAB_HOLD`.
  - `impl TabHold { pub fn down(&mut self, vk: u32, ctrl: bool, shift: bool, alt: bool, repeat: bool, route: KeyRoute) -> HoldStep; pub fn up(&mut self, vk: u32) -> Option<KeyEvent>; pub fn timeout(&mut self, hold: u64) -> Option<KeyEvent> }`. A released chord is `KeyEvent { code: "Tab", ctrl: true, shift, alt: false, repeat: false }`.

- [ ] **Step 1: Write the failing tests:**
  - `a_claimed_ctrl_tab_from_a_page_is_held`: `down(VK_TAB, true, false, false, false, KeyRoute::App)` → `taken`, `started.is_some()`, no `release`.
  - `an_unclaimed_ctrl_tab_or_another_chord_is_not_held`: `KeyRoute::Page`, Ctrl+Shift+Tab, Ctrl+Alt+Tab and a repeat each → `HoldStep::default()`.
  - `shift_during_the_hold_sends_ctrl_shift_tab_and_takes_the_shift`: for each of `VK_SHIFT`, `VK_LSHIFT` and `VK_RSHIFT` with ctrl → `release` is Ctrl+Shift+Tab, `taken`; a following `timeout(started)` → `None`.
  - `shift_without_ctrl_sends_ctrl_tab`: → `release` is Ctrl+Tab, not `taken`.
  - `tabs_release_sends_ctrl_tab`: `up(VK_TAB)` → Ctrl+Tab; again → `None`.
  - `the_timeout_sends_ctrl_tab_once_and_a_stale_timeout_nothing`: `timeout(n)` → Ctrl+Tab, `timeout(n)` again → `None`. For a second hold `m`, `timeout(n)` → `None` and `timeout(m)` → Ctrl+Tab.
  - `a_repeat_during_the_hold_is_taken_and_sends_nothing`: → `taken`, no `release`, still held (`up(VK_TAB)` releases).
  - `another_key_during_the_hold_sends_ctrl_tab_and_is_not_taken`: `down(VK_L, true, …)` → `release` is Ctrl+Tab, not `taken`.

- [ ] **Step 2: Run, expect FAIL** (no `TabHold`): `cargo test -p marshell-app -j 4 browser::rules`

- [ ] **Step 3: Implement `TabHold`**, with a doc comment citing the decisions.

- [ ] **Step 4: Run, expect PASS**: `cargo test -p marshell-app -j 4 browser::rules`

- [ ] **Step 5: Commit** `feat(browser): the rules hold a page's Ctrl+Tab for Tab's release or 50 ms, and Shift meanwhile makes it Ctrl+Shift+Tab.`

### Task 4: The shell holds a page's Ctrl+Tab

**Files:**
- Modify: `src-tauri/src/browser/com.rs` (`install`, `key_handler`)

**Interfaces:**
- Consumes: `TabHold`, `HoldStep`, `TAB_HOLD` (Task 3); `post`, `emit`, `focus_app` and `PageKeyEvent` from `browser/mod.rs`.

- [ ] **Step 1: Wire it.** `install` makes one `Arc<Mutex<TabHold>>` per page and passes it to `key_handler`. Factor the existing App branch's job into `fn forward(app: &AppHandle, key: &str, event: KeyEvent)` (`focus_app` and `emit` of `browser://key`).
  - **Key-up:** `hold.up(vk)` first, posting `forward` for a release. Then the existing `taken` removal.
  - **Key-down:** after `route`, `hold.down(vk, ctrl, shift, alt, WasKeyDown, route)`.
    - A `release` posts `forward`.
    - `started: Some(n)` spawns a thread that sleeps `TAB_HOLD`, then `post`s a job: `if let Some(event) = lock(&hold).timeout(n) { forward(..) }`.
    - `taken` inserts `vk` into `taken`, calls `SetHandled(true)` and returns.
    - Otherwise the existing routing runs.
  - Comments say why: the decision, and that the job runs on the main thread like every other callback.

- [ ] **Step 2: Build and lint**: `cargo clippy -p marshell-app --all-targets -j 4 -- -D warnings` is clean, and `cargo test -p marshell-app -j 4` passes.

- [ ] **Step 3: Commit** `feat(browser): a page's Ctrl+Tab waits in the shell, so a rolled Ctrl+Shift+Tab reaches the app as itself.`

### Task 5: Live check and hand check

- [ ] **Step 1: Live check** with the `marshell-live-check` skill (scratch homes, CDP, stop only what it started). CDP `Input.dispatchKeyEvent` makes trusted key presses in the app's webview. Pass when:
  - Ctrl down, Tab down, Shift down 8 ms later, then the releases: the result is Ctrl+Shift+Tab ("Selected the side pane"), and the tab doesn't step.
  - A plain Ctrl+Tab steps the tab within 50 ms, with the terminal focused and with the pane focused.
  - Ctrl+Tab in Settings moves focus as before.
  - Ctrl+Tab held down steps exactly one tab.
- [ ] **Step 2: Hand check with the user** (real keyboard; the page side can't be synthesized):
  - Roll Ctrl+Shift+Tab ten times from a page: it never steps the tabs.
  - Plain Ctrl+Tab from a page still steps them.
  - Roll it once with a text field in the page focused: the field doesn't act as if Shift is stuck.
- [ ] **Step 3:** Record both in `docs/research/2026-11-13-ctrl-tab-rollover-live-check.md`, and mark finding 30 fixed in PLAN §9 decision 15. Commit `docs(research): …`.

### Task 6: Review and ship

- [ ] **Step 1:** Whole-branch review by the `marshell-reviewer` agent (Opus) with this plan's Review Focus, then one fix pass.
- [ ] **Step 2:** `marshell-ship`: CI's commands, PR, green CI, merge, ledger deleted.
