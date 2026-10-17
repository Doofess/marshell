# Slice 2f: recording, palette and cheat sheet, navigation cursor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every session writes an asciicast v2 recording to disk. The cheat sheet and the arrow navigation cursor stop being design mocks and run in the live window. The webview's own browser shortcuts are switched off, so none of them can shadow an app shortcut.

**Architecture:**
- **Recording.** It lives in the core, inside `pty::Session`. A pure `CastEncoder` turns output bytes, resizes and OSC 133 prompt marks into asciicast lines. A `Recorder` sends those lines over a bounded channel to its own writer thread, so disk I/O never runs on the pty reader thread or under the output lock.
- **Cheat sheet.** The live window mounts the existing `ShortcutSheet` as an overlay. It lists only commands the live window actually runs.
- **Navigation.** The live window drives the existing pure navigation machine (`src/lib/navigation/`) through a new hook. The hook measures `data-nav-*` elements and turns the machine's effects into view-state actions. It adds one new state: no terminal selected.

**Tech Stack:** Rust (std threads, `serde_json`, axum), Tauri 2 / WebView2, React 19 + TypeScript, vitest (node, `renderToStaticMarkup`, no DOM).

**Spec:** `docs/PLAN.md`, read by line range and never whole:
- :756 (the phase 2 scope line);
- :761 (the slice list);
- :779 item 8 (the cursor moves to 2f);
- :814 (the phase 2 exit: "a recording replays in asciinema-player");
- :554 (`history/recordings/<tab>-<ts>.cast.zst`);
- :905 (secrets at rest);
- :658–704 (keyboard routing, the navigation layer, the audit table).

`docs/BRIEF.md`:261 says layer (b), the raw recording, is captured "for every session, automatically".

User decisions of 2026-10-17:
1. Recordings hold output, resize and OSC 133 marker events only. Keystrokes (`"i"`) are never written.
2. Plain `.cast` now. zstd, cleanup and retention wait for phase 5.
3. The cheat sheet reads the live keymap. The rebinding UI and a keybindings file come later.
4. The navigation cursor ships to the full spec: panes, sidebar rows and top bar; release selects; an off-screen session is brought into a pane; Backspace selects none; announcements.

## Global Constraints

- Shortcuts match by `KeyboardEvent.code`. No shortcut uses punctuation. App chords are Ctrl+Shift on Windows and Linux and Cmd on macOS. The navigation modifier defaults to Ctrl+Shift on every system.
- In a terminal, plain Tab, Esc and the arrows always go to the program. Ctrl+Shift+arrow in a terminal or text box selects a word until a Tab has been pressed in that hold.
- One polite live region for the window. Navigation announcements go through the window's existing `announce()` (LiveApp), not a second `NavAnnouncer`.
- Never guess. The cheat sheet shows no key for a command the live window does not run.
- Recording must never slow down, stall or break a session. If the disk fails or fills, recording stops for that session and the terminal goes on.
- Recordings live in `~/.marshell/history/recordings/`. The folder is created with `store::create_private_dir` and the file is 0600 on unix; on Windows it inherits the user-only ACL (as `auth.rs` notes).
- No new crate dependency, except `webview2-com` in `src-tauri` if Task 1 needs it, pinned to the version `tauri` already pulls in (`cargo tree -i webview2-com`).
- Copy is in plain sentences, as in nearby UI strings. Run `cargo fmt --all` before every commit that touches Rust: CI checks it.
- Keep `LiveApp.tsx` (672 lines) from growing. New window logic goes in its own hook file, and LiveApp only wires it.

## Review Focus

1. **A UTF-8 character split across two pty reads.** Expected: the recording holds the whole character in the later event, never U+FFFD (`CastEncoder` keeps a carry). Invalid bytes still become U+FFFD, so a binary `cat` cannot stop recording. Tested in Task 2.
2. **Runaway output (`yes`) or a full disk.** Expected: the file stops at `MAX_RECORDING_BYTES`, its last event is a marker saying so, and the session keeps running at full speed. Tested in Task 3.
3. **Ctrl+Shift+arrow typed in a terminal with no Tab in the hold.** Expected: it reaches the program (word selection in PSReadLine), and no cursor appears. Tested in Task 6 (`claimsKey`).
4. **The window loses focus while Ctrl+Shift is held with a cursor out** (Alt+Tab away). Expected: the cursor is cancelled, not committed, and nothing is brought into a pane. Tested in Task 6.
5. **The cheat sheet opened from inside a terminal, then closed with Esc.** Expected: focus returns to that terminal, and Esc never reaches the program. Tested in Task 5.

---

### Task 1: Switch off the webview's browser shortcuts

**Files:**
- Modify: `src-tauri/src/main.rs` (the `setup` closure, at about :81)
- Modify: `src-tauri/Cargo.toml` (only if the fallback below is needed)

**Interfaces:**
- Produces: nothing other tasks call. After this task Ctrl+Shift+I, Ctrl+R, F5, Ctrl+P, Ctrl+F and F12 no longer reach WebView2.

- [ ] **Step 1: Find the supported switch.** Use context7 (`/tauri-apps/tauri` and `/tauri-apps/wry`) to find the current Tauri 2 way to set WebView2's `AreBrowserAcceleratorKeysEnabled`. Check first for a window config key or a `WebviewWindowBuilder` / wry `browser_accelerator_keys` option. If neither exists in the pinned `tauri`, fall back to `webview.with_webview(|w| …)`: on `w.controller()`, call `CoreWebView2()?.Settings()?`, cast it to `ICoreWebView2Settings3`, and call `SetAreBrowserAcceleratorKeysEnabled(false)`. For that path, add `webview2-com` under `[target."cfg(windows)".dependencies]` at the version from `cargo tree -i webview2-com`.
- [ ] **Step 2: Implement it in `setup` for the `main` window, Windows only (`#[cfg(windows)]`).** It applies in debug builds too, because Ctrl+Shift+I is focus mode. Add a comment on macOS and Linux: WKWebView and WebKitGTK bind none of these keys to the app's chords, so there is nothing to switch off yet (PLAN :704).
- [ ] **Step 3: Verify.** Run `cargo build -p marshell-app`; expect success. Then `pnpm tauri dev`: Ctrl+Shift+I toggles focus mode and does not open devtools; F5 does not reload.
- [ ] **Step 4: Commit:** `fix(app): switch off WebView2's browser shortcuts so none shadows an app shortcut`

### Task 2: The asciicast encoder (pure)

**Files:**
- Create: `crates/core/src/pty/record.rs` (encoder part)
- Modify: `crates/core/src/pty/mod.rs` (`mod record; pub use record::…`)

**Interfaces:**
- Produces:
  - `pub struct CastHeader { pub cols: u16, pub rows: u16, pub started_unix: u64, pub shell: Option<String> }`
  - `pub fn header_line(h: &CastHeader) -> String`: the v2 header JSON plus `\n`: `{"version":2,"width":C,"height":R,"timestamp":T,"env":{"TERM":"xterm-256color","SHELL":S}}`. SHELL is left out when `None`.
  - `#[derive(Default)] pub struct CastEncoder { carry: Vec<u8> }` with:
    - `pub fn output(&mut self, t: f64, bytes: &[u8]) -> Option<String>`: `[t,"o",data]\n`, or `None` when nothing complete is left (only a carried partial character).
    - `pub fn resize(t: f64, cols: u16, rows: u16) -> String`: `[t,"r","COLSxROWS"]\n`
    - `pub fn marker(t: f64, label: &str) -> String`: `[t,"m",label]\n`
  - Times are written with 6 decimals (`format!("{t:.6}")`). Strings are encoded with `serde_json::to_string`.

- [ ] **Step 1: Write failing tests** in `record.rs` `#[cfg(test)]`:
  - `header_has_size_time_and_term`: parse the line with serde_json and assert `version == 2`, width, height, timestamp and `env.TERM == "xterm-256color"`; there is no SHELL key when `shell` is `None`.
  - `output_escapes_control_bytes`: `output(1.5, b"a\x1b[0m\r\n")` equals `"[1.500000,\"o\",\"a\\u001b[0m\\r\\n\"]\n"`.
  - `split_utf8_is_carried`: `"é"` is `[0xC3, 0xA9]`. `output(0.1, &[b'x', 0xC3])` gives data `"x"`; `output(0.2, &[0xA9])` gives data `"é"`. No U+FFFD appears.
  - `invalid_bytes_become_replacement`: `output(0.0, &[0xFF, b'a'])` gives data `"\u{FFFD}a"`.
  - `lone_partial_returns_none`: `output(0.0, &[0xE2, 0x82])` is `None`, and then `output(0.1, &[0xAC])` gives `"€"`.
  - `resize_and_marker_lines`: `resize(2.0, 120, 30)` is `"[2.000000,\"r\",\"120x30\"]\n"`; `marker(3.0, "prompt")` is `"[3.000000,\"m\",\"prompt\"]\n"`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -p marshell-core pty::record`. Expect a compile failure (`CastEncoder` not found).
- [ ] **Step 3: Implement the encoder.** Carry rule: append the input to the carry, then find the longest prefix that is valid UTF-8 or invalid only at an incomplete tail. Use `std::str::from_utf8` and its `Utf8Error::error_len() == None` to detect an incomplete tail, and carry at most 3 bytes. Decode the prefix with `String::from_utf8_lossy`.
- [ ] **Step 4: Run the tests and see them pass.** Same command.
- [ ] **Step 5: Commit:** `feat(record): asciicast v2 encoder with a UTF-8 carry`

### Task 3: Record every session to disk

**Files:**
- Modify: `crates/core/src/pty/record.rs` (add `Recorder`)
- Modify: `crates/core/src/pty/session.rs` (`SpawnSpec.record`, `ingest`, `resize`)
- Modify: `crates/core/src/paths.rs` (`recordings_dir`)
- Modify: `crates/core/src/server/sessions.rs` (`spawn_into_map`, at about :262)
- Modify: `crates/core/src/supervisor.rs` (the test `SpawnSpec` literal at about :176 gets `record: None`), plus any other `SpawnSpec {` literal that `cargo build` reports

**Interfaces:**
- Consumes: Task 2's `CastHeader`, `header_line` and `CastEncoder`.
- Produces:
  - `pub fn recordings_dir(home: &Path) -> PathBuf` → `home/history/recordings`.
  - `pub const MAX_RECORDING_BYTES: u64 = 256 * 1024 * 1024;`
  - `pub struct Recorder` with:
    - `pub fn start(path: PathBuf, header: CastHeader) -> anyhow::Result<Recorder>`: creates the parent with `create_private_dir`, opens the file 0600 (the same `OpenOptions` as `store::write_private_file`, without truncating a fresh file), writes the header, and spawns thread `rec-<file stem>`.
    - `pub fn output(&self, bytes: &[u8])`
    - `pub fn resize(&self, cols: u16, rows: u16)`
    - `pub fn marker(&self, label: &str)`
    - `pub fn path(&self) -> &Path`
    - Dropping it closes the channel; the thread flushes and exits.
  - `SpawnSpec` gains `pub record: Option<(PathBuf, Option<String>)>`: the file path and the SHELL value for the header. `Session` gains `pub fn recording_path(&self) -> Option<PathBuf>`, which is `None` when recording never started or was stopped by an error.

- [ ] **Step 1: Write failing tests** in `record.rs`, using `tempfile::tempdir()`:
  - `recorder_writes_header_and_events`: start, then `output(b"hi")`, `resize(100, 40)`, `marker("prompt")`, then drop. Wait for the thread with a join handle kept for tests (`#[cfg(test)] fn finish(self)`). Read the file: line 1 parses as a header; lines 2–4 have codes `o`, `r` and `m`, with times that never decrease.
  - `recorder_stops_at_the_size_limit`: with a test-only limit (make it a field set by `Recorder::start_with_limit(path, header, limit)`, which `start` calls with `MAX_RECORDING_BYTES`) of 200 bytes, write 50 outputs of 20 bytes. The file is at most 200 bytes plus one final marker line, and the last line is `[t,"m","Recording stopped: it reached the size limit."]`.
  - `start_fails_cleanly_on_a_bad_path`: `Recorder::start(dir.path().to_path_buf(), header)` (the path is an existing directory) returns `Err` and spawns no thread. What the caller does then is the session test below.
  And in `session.rs` tests (follow the existing spawn tests and their `fake-agent` / echo argv):
  - `session_records_its_output`: spawn with `record: Some((dir.join("t-1.cast"), None))`, have the child print `hello`, wait for exit, and assert the file contains an `o` event whose data contains `hello`. `recording_path()` is `Some`.
  - `session_without_a_recordable_path_still_runs`: `record` points into a directory that cannot be made (a path under a regular file). The session spawns, runs, and `recording_path()` is `None`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -p marshell-core record`.
- [ ] **Step 3: Implement.**
  - **Channel.** Use `std::sync::mpsc::sync_channel::<Event>(1024)` and a blocking `send`. The writer is far faster than a pty, and blocking is what the reader already does for backpressure. After a send error (the thread stopped), set a stopped flag and stop sending.
  - **Thread.** Run `BufWriter<File>` with `recv_timeout(1 s)` and flush on timeout, so a crash loses at most about a second. Times are `Instant` elapsed since `start`, taken by the sender at the moment of the event, not by the writer. Count bytes written; past the limit, write the stop marker once and drop every later event. On any I/O error, log once with `tracing::warn!` and stop.
  - **Session.** `Output` gains `recorder: Option<Recorder>`. In `ingest`, record `pass` (the bytes the terminal got, after the DSR filter) and, for each `Mark::PromptStart`, `marker("prompt")`. Move the `out.marks.scan` guard to scan when `mark_callback.is_some() || recorder.is_some()`. Send after dropping the lock: clone a cheap sender handle out, or make `Recorder` methods take `&self` on a `SyncSender` held in an `Arc`. `resize` records `r` only when the size really changed and is not 0×0.
  - **`spawn_into_map`.** Build the path `recordings_dir(&st.0.home).join(format!("{tab_id}-{unix_ms}.cast"))`, with the SHELL value `run.argv.first().cloned()`. If `Recorder::start` fails, the session still spawns with no recorder (`warn!` once). This covers the shell panel too, which keeps its recording when adopted, and a Resume, which writes a new file.
- [ ] **Step 4: Run the tests and see them pass:** `cargo test -p marshell-core`, all of it, then `cargo clippy --workspace -- -D warnings`.
- [ ] **Step 5: Commit:** `feat(record): every session writes an asciicast v2 recording (output, resize, prompt marks)`

### Task 4: Find a session's recording from the palette

**Files:**
- Modify: `crates/core/src/server/sessions.rs` (handler `recording`)
- Modify: `crates/core/src/server/mod.rs` (route `/v1/sessions/{tab}/recording`, next to `resume` at about :175)
- Modify: the frontend API client that holds `resume` (find it with `grep -rn "/resume" src/lib src/features --include=*.ts`)
- Modify: `src/lib/keymap.ts` (command), `src/lib/appShortcuts.ts`, `src/features/sessions/LivePalette.tsx` (`PALETTE_ONLY`), `src/features/sessions/LiveApp.tsx` (`runFromPalette`, one line that calls the new helper)
- Test: the existing server test file for sessions (beside the other `/v1/sessions` tests) and `src/features/sessions/LivePalette.test.tsx` (create it if there is none)

**Interfaces:**
- Consumes: `Session::recording_path()` (Task 3).
- Produces:
  - `GET /v1/sessions/{tab}/recording` → `200 {"path": "<absolute path>"}`. It returns `404` with `"This session has no recording."` when the session is unknown, or when it has no recording (recording failed or was stopped by an error).
  - TS `getRecordingPath(endpoint, tabId): Promise<string>`.
  - Command `{ id: "copy-recording-path", label: "Copy this session's recording path", group: "Tools" }`, unbound and palette-only.

- [ ] **Step 1: Write failing tests.**
  - **Rust:** create a session, then `GET` its recording and assert 200 with a path ending `.cast` that exists. `GET` an unknown tab and assert 404.
  - **TS:** `LIVE_COMMANDS` contains `copy-recording-path`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -p marshell-core recording` and `pnpm test LivePalette`.
- [ ] **Step 3: Implement.** In `runFromPalette`, for `copy-recording-path`, call a helper in a new `src/features/sessions/recording.ts`, `copyRecordingPath(endpoint, tabId, announce)`. It fetches the path, writes it with `navigator.clipboard.writeText`, and announces `"Copied the recording's path."`, or `"Couldn't copy the recording's path: <reason>"` on failure. Hide the command (through `hiddenCommands`) when there is no focused session.
- [ ] **Step 4: Run the tests and see them pass.** Same commands.
- [ ] **Step 5: Commit:** `feat(record): copy a session's recording path from the palette`

### Task 5: The live cheat sheet

**Files:**
- Modify: `src/features/shortcuts/ShortcutSheet.tsx` (`sheetGroups` takes a filter; a dialog wrapper)
- Create: `src/features/sessions/LiveShortcuts.tsx`
- Modify: `src/lib/appShortcuts.ts` (`{ kind: "cheatsheet" }`), `src/features/sessions/sessionActions.ts` (the window-handled list at about :162), `src/features/sessions/LiveApp.tsx` (overlay `"shortcuts"`)
- Test: `src/features/shortcuts/ShortcutSheet.test.tsx`, `src/lib/appShortcuts.test.ts`

**Interfaces:**
- Produces:
  - `sheetGroups(os, nav, runs?: (id: string) => boolean)`: when `runs` is given, rows are kept only for commands where `runs(id)` is true. The "Go to session 1 to 9" row needs `runs("focus-session-1")`. The arrows row needs `runs("nav-next-pane")`. A group left with no rows is dropped.
  - `LiveShortcuts({ os, onClose })`: a modal dialog (`role="dialog"`, `aria-modal`, labelled by the sheet title). It uses `useDialogFocus` (the existing hook: focus starts on the title, Tab stays inside, Escape closes, and focus returns to the opener). Its `runs` is `(id) => LIVE_COMMANDS.some((c) => c.id === id) || NAV_COMMAND_IDS.includes(id) || /^focus-session-/.test(id)`.

- [ ] **Step 1: Write failing tests.**
  - `sheetGroups` with a `runs` that rejects `history`, `find`, `global-search`, `bookmark` and `next-waiting` shows none of those labels, and still shows "Command palette" and "Keyboard shortcuts".
  - A group with no runnable rows is not in the output.
  - `commandShortcut("cheatsheet")` is `{ kind: "cheatsheet" }`, and `cheatsheet` is in `LIVE_COMMANDS`.
  - `renderToStaticMarkup(<LiveShortcuts os="windows" onClose={() => {}} />)` has `role="dialog"` and `aria-modal="true"`, and contains `Ctrl+Shift+K`.
  - Review Focus 5: test `trapKey` / `useDialogFocus` the way existing dialog tests do (find one with `grep -rln useDialogFocus src --include=*.test.tsx`). Escape calls `onClose` and is prevented.
- [ ] **Step 2: Run them and see them fail:** `pnpm test ShortcutSheet appShortcuts`.
- [ ] **Step 3: Implement.** Ctrl+Shift+O and the palette row both call `setOverlay("shortcuts")`; a second Ctrl+Shift+O while it is open closes it. The window's shortcut listener already stands down while an overlay is up, so the sheet's Escape is its own. Put the `TerminalView` handler's existing `sessionShortcut` check in front of `cheatsheet`, so Ctrl+Shift+O never types into the terminal.
- [ ] **Step 4: Run the tests and see them pass.** Same command, then `pnpm build`.
- [ ] **Step 5: Commit:** `feat(shortcuts): the cheat sheet opens on Ctrl+Shift+O and lists only what the window runs`

### Task 6: The live navigation cursor

**Files:**
- Create: `src/features/sessions/navLayout.ts` (pure: elements → `NavLayout`)
- Create: `src/features/sessions/useNavigation.ts` (the hook and the shared claim check)
- Modify: `src/features/window/MainWindow.tsx`, `Sidebar.tsx`, `Rail.tsx`, `SplitView.tsx` / `Pane.tsx` (`data-nav-*` attributes, `noneSelected`, and the cursor drawn with `NavCursor`)
- Modify: `src/features/terminal/TerminalView.tsx` (`attachCustomKeyEventHandler` at :112)
- Modify: `src/features/sessions/LiveApp.tsx` (wiring only; replace the `nextPane` and `nonePane` branches of `paneCommand` with the hook)
- Test: `src/features/sessions/navLayout.test.ts`, `src/features/sessions/useNavigation.test.ts`, `src/features/window/MainWindow.test.tsx`

**Interfaces:**
- Consumes: `reduce`, `INITIAL_NAV`, `NavState`, `NavEffect` and `NavContext` (`src/lib/navigation/machine.ts`, `types.ts`); `interpretKeyDown`, `interpretKeyUp`, `interpretBlur` and `FocusKind` (`keys.ts`); `NavCursor` and `NO_SELECTION_TEXT` (`NavCursor.tsx`); the view actions `focusPane`, `assign` and `select` (`viewState.ts`).
- Produces:
  - **Markup.** Each pane slot carries `data-nav-pane="<index>"`. Each sidebar or rail session row carries `data-nav-row="<session number>"`. The top bar's launcher and palette buttons carry `data-nav-top="launcher"` and `data-nav-top="palette"`. Only drawn, visible elements count.
  - `export type Measured = { attrs: { pane?: string; row?: string; top?: string }; rect: Rect }` and `export function navLayoutOf(items: Measured[], origin: Rect): NavLayout`. It is pure: it turns the attributes into `Target`s, makes each rect relative to `origin` (the window), and drops zero-size rects. The DOM wrapper is `measureNavLayout(root: HTMLElement): NavLayout`, which uses `querySelectorAll("[data-nav-pane],[data-nav-row],[data-nav-top]")` and `getBoundingClientRect`.
  - `export function focusKindOf(el: Element | null): FocusKind`. It returns `"terminal"` inside `.xterm`, `"editable"` for `isTextBox`, `"control"` for a button, link, `[role=option]` or `[tabindex]`, and `"none"` otherwise.
  - `export function claimsKey(e: KeyEventLike, os: Os, focus: FocusKind): boolean`. It reads the module's current `NavState`, and is true when `interpretKeyDown(...)` returns events with `preventDefault`. `TerminalView` returns `false` from its handler when `claimsKey(e, os, "terminal")` is true.
  - `useNavigation(opts: { os: Os; enabled: boolean; panes: Array<number | null>; selectedPane: number | null; names: Record<number, string>; root: RefObject<HTMLElement>; apply: (effect: NavEffect) => void }): { cursor: { rect: Rect; label: string } | null; noneSelected: boolean }`
    - It installs capture-phase `keydown` and `keyup` listeners on `window`, and `blur` on `window`. They are off while `enabled` is false (launcher, overlay or review open).
    - `selectedPane` is taken from the view (the focused pane index), or `null` while `noneSelected`.
    - It measures the layout on each arrow, Tab or release, never on every render.
    - `announce` effects go to the window's announcer through `apply`.
  - **LiveApp's `apply`.**
    - `select-pane` → `focusPane`, then focus that pane's terminal, and clear none.
    - `bring-into-view` → `assign` the tab with that session number into `pane`, then `focusPane`.
    - `activate` → run the command id as `runFromPalette` does (`launcher`, `palette`).
    - `deselect` → the existing `nonePane` blur; set none.
    - `announce` → `announce(text)`.
  - **`MainWindow`** gets `noneSelected?: boolean`. While it is true, no pane carries the focused accent, the status bar shows `NO_SELECTION_TEXT`, and plain Tab with focus on nothing selects the first terminal (`keys.ts` already sends that `tab`). It also gets `navCursor?: { rect: Rect; label: string } | null`, drawn with `<NavCursor>` inside the positioned window root.

- [ ] **Step 1: Write failing tests.**
  - `navLayoutOf`: two panes, three rows and one top control give six targets with rects relative to the origin; a 0×0 row is dropped; an element with `data-nav-row="x"` (not a number) is dropped.
  - `focusKindOf` (with plain objects shaped like `Element` that `matches`/`closest` against fixed selectors, since there is no DOM): an xterm textarea → `terminal`, an input → `editable`, a button → `control`, null → `none`.
  - `claimsKey`, Review Focus 3: with the state at `INITIAL_NAV`, `ctrlKey+shiftKey+ArrowLeft` in `"terminal"` → false; after a `Tab` chord (feed `reduce` with `modifier-down`, `tab`), the same arrow → true. `Ctrl+Shift+Backspace` → true anywhere.
  - Pure driver: export `step(state, events, ctx)` from `useNavigation.ts`, a thin wrapper over `run` that the hook uses, so the scenarios are tested without React.
    - Review Focus 4: `modifier-down`, `tab`, `arrow left` puts a cursor on a row; then `interpretBlur(state)` events through `step` yield no `bring-into-view` and no `select-pane`, and an `announce` of "Cancelled".
    - Release on a row not on screen (panes `[3, 5]`, selected 1, row 7) → `bring-into-view {number: 7, pane: 1}`.
  - `MainWindow` with `noneSelected` renders `No session selected · Tab to select a terminal` and no `data-focused` pane. With `navCursor`, it renders `.nav-cursor` and the hint `Release Ctrl+Shift to select · Backspace for none`.
- [ ] **Step 2: Run them and see them fail:** `pnpm test navLayout useNavigation MainWindow`.
- [ ] **Step 3: Implement** to the interfaces above. Keep `paneCommand`'s `nextPane` and `nonePane` routes working for the palette rows by sending the same events through the hook (`tab`, `select-none`), so the palette and the chords behave the same.
- [ ] **Step 4: Run the tests and see them pass.** Run `pnpm test` (all of it) and `pnpm build`.
- [ ] **Step 5: By eye, in `pnpm tauri dev`, with 3 sessions and 2 panes.**
  - Ctrl+Shift+Tab cycles the panes.
  - Hold Ctrl+Shift, press Tab, then ←: the cursor reaches the pane's own row. ↑ and ↓ walk the list. Releasing on an off-screen session brings it into the focused pane.
  - Ctrl+Shift+Backspace clears the selection, and the status bar says so.
  - Ctrl+Shift+← in PowerShell with no Tab still selects a word.
  - NVDA reads "Cursor on session 2, …" and "Selected session 2, …".
- [ ] **Step 6: Commit:** `feat(nav): hold Ctrl+Shift and use the arrows to move a cursor across panes, the session list and the top bar`

### Task 7: Docs and the exit check

**Files:**
- Modify: `docs/PLAN.md`: add "Slice 2f decisions (2026-10-17)" after the 2e+ decisions in §9, with the four user decisions above and the 256 MiB cap. Mark 2f done in the slice list.
- Modify: `docs/IDEAS.md` only if something was found and deferred.

- [ ] **Step 1: Exit check.** Start a session, run a few commands, close it, and copy the path from the palette (before closing). Copy the `.cast` into the scratchpad beside a small `play.html`. It loads `asciinema-player@3.8.2` (its `.css` and `.min.js` from cdn.jsdelivr.net) and calls `AsciinemaPlayer.create("x.cast", el, { markers: true })`. Serve the folder (`npx --yes serve`) and open it in Playwright. Expected: it plays with no console error (`browser_console_messages`), and the prompt markers show on the timeline.
- [ ] **Step 2: Write the decisions.** Run `pnpm check:generated`; expect no diff (no protocol type changed).
- [ ] **Step 3: Commit:** `docs(plan): slice 2f decisions and status`
