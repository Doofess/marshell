# Phase 4b: the tray, the close button and the needs-you count Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marshell lives in the tray. The close button hides the window to the tray, after a one-time choice. The tray menu lists who needs you and can pause notifications, and the window title and taskbar badge show the same needs-you count.

**Architecture:** The webview already derives the needs-you list (`laneEntries`). It computes one count from it, puts the count in the window title, and sends the tray model (menu items, tooltip, waiting count, built by the phase-0 `trayItems`/`trayTooltip`) to the shell through a `set_tray` command. The shell (`src-tauri/src/tray.rs`) owns the Tauri tray icon:
- it draws the icon itself, the mono wands plus an accent dot;
- it renders the menu natively;
- it handles Open and Quit itself, so they work even if the webview is stuck;
- it emits every other menu choice to the webview as `tray-action`.

On a close request, the shell reads the saved choice from `settings.json` and then hides, quits, or asks the webview to show the one-time dialog. Pause notifications is a flag the supervisor owns, so it outlives a core restart. The core's notifier drops what it would deliver while the flag is set. The flag is reached through `GET`/`PUT /v1/notify/paused`.

**Tech Stack:** Rust (marshell-core, marshell-protocol with ts-rs, marshell-app on Tauri 2), React + TypeScript, Vitest. No new crate: Tauri's `tray-icon` feature turns on `tray-icon` 0.25.1, which is already in `Cargo.lock` (MIT OR Apache-2.0, tauri-apps, active). It goes in `THIRD_PARTY_NOTICES.md`. windows-sys gains the `Win32_System_Registry` feature to read the taskbar theme.

**Spec:** `docs/PLAN.md` §9 "Phase 4 decisions (2026-10-21)" 1, 3 and 4, and "Slice 4b decisions (2026-10-22)". Also PLAN:671 (no tray mouse events on Linux) and slice 3d decision 8 (the overlay shows the count above 0). The phase-0 tray design is `docs/superpowers/plans/2026-10-10-phase-0-batch-3-onboarding-doctor-settings-departures.md` Task 16 and `src/features/tray/`. The 4a live-check finding 1 is in `docs/research/2026-10-21-notifications-live-check.md`.

## Global Constraints

- One count everywhere: the distinct tabs in `laneEntries` (held requests plus needs-permission or needs-question rows with no hold, including an expired hold). Muted sessions count. The badge, the title and the tray tooltip, menu and dot all use it (4b decision 1).
- Title: `(N) <session> — Marshell`, or `(N) Marshell` with nothing selected, and unchanged at 0 (4b decision 2).
- Close, from the title bar's button, Alt+F4 or the taskbar's Close window, goes through the shell's `CloseRequested`:
  - no tray → quit;
  - choice saved as quit → quit;
  - saved as tray → hide;
  - nothing saved → keep the window open, show and focus it, and emit `close-choice` (4b decisions 3 and 5).
- The dialog copy is exactly "Marshell keeps running in the tray, so your sessions keep going." Its buttons are "Keep running in the tray" (the default, focused first) and "Quit when I close". Escape cancels without saving.
- Tray menu, in this order:
  - Open Marshell;
  - a separator;
  - Nothing needs you (disabled), or up to 5 sessions and then "N more…";
  - Go to the oldest waiting session, with Ctrl+Shift+N shown and disabled at 0;
  - a separator;
  - Pause notifications or Resume notifications;
  - Notifications…;
  - a separator;
  - Quit Marshell.

  No Departures, Doctor or Settings item in the live app (4b decision 4).
- The tooltip comes from `trayTooltip`: "Marshell", "Marshell · 1 needs you", "Marshell · 2 need you", with " (notifications paused)" appended while paused.
- Pause stops banners, sounds and the flash, never the count. It lasts until Resume or app quit, and survives a core restart. The app starts unpaused.
- Left click shows and focuses the window, and right click opens the menu. Quit never asks. Every path that brings the window back calls `show()` as well as `unminimize()` and `set_focus()`: the banner click, a second launch, tray Open, left click and a session menu item.
- The saved window state never includes visibility: quitting while hidden must not start the next run hidden.
- macOS and Linux sit behind `cfg` stubs and are unverified (phase 4 decision 4). Linux has no click events, so its tray is menu-only. macOS uses a template icon.
- Everything works with the keyboard alone. The close dialog is a real dialog with focus trapped and returned, and it works in forced colors. The native tray menu is the OS's own (Win+B reaches it).
- `cargo fmt --all` before every Rust commit, and `pnpm check:generated` after a protocol change. Tests and live checks never touch the real `~/.marshell`.
- Commit messages read `type(scope): what changed, as a sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Execution

Tasks 1, 2, 3 and 4 go to Codex CLI (`codex exec`). Each task gets its own worktree under `C:/dev/codex-wt/`, branched from `phase-4b`. When Codex is out of quota, the task goes to a `worker` instead, at high effort for Tasks 1 and 2 (the local API; Win32 and threading). The main thread reads each diff, runs the tests and cherry-picks. They run in order, 1 → 2 → 3 → 4, because each consumes the one before it, and 2 and 3 both edit `main.rs`. Before Task 2, the main thread turns on the `tray-icon` feature and builds once, so Codex's sandbox finds the crates already fetched. Task 5 is docs, done inline. Task 6 is the live check in the main thread.

## Review Focus

1. **Quitting while hidden.** Quit from the tray while the window is hidden, then start again: the window opens visible, at its saved size and place. Task 3 pins it with `window_state_flags` excluding `VISIBLE`.
2. **Hostile text in a menu label.** A session name or phrase with `&`, a tab, a newline or 300 characters must stay one plain line. `&` is doubled so it is not read as a mnemonic, control characters become spaces, and the label is cut to 64 characters with "…". Task 2 has `menu_label` tests for each.
3. **Closing before the webview can answer.** With nothing saved and the webview not listening, the window stays open and visible: nothing hides it out of reach. Quit in the tray always works. Task 3 tests `close_action`, and Task 6 checks it live.
4. **Pause across a core restart.** The flag lives in the supervisor and is handed to each new core, so a restart keeps it. A wait that started while paused never gets its banner after Resume. Task 1 has a supervisor test and a notifier test.
5. **A hidden window keeps no tabs in view.** After the window hides, the tab that was on screen is out of sight: its Done banner comes after 30 s and its reminder still fires, even if the hidden webview reports its old view again. Task 1 has a core test (`a_hidden_window_has_no_tab_in_view`) and Task 6 checks it live.
6. **An expired hold.** When a 120 s hold runs out and the row still reads needs-permission, the count stays the same. Task 4 has a `laneEntries` test, and Task 6 checks it live with the sessions snapshot recorded, because the 4a live check saw the count drop.

---

### Task 1: Pause notifications and the close choice (protocol, core)

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Settings`), `crates/core/src/settings.rs` (`FileDoc`, load and save), `crates/core/src/server/mod.rs` (`CoreConfig`, `AppState`, routes), `crates/core/src/notify/mod.rs` (`spawn`), `crates/core/src/supervisor.rs`, every `CoreConfig { .. }` literal (8, in tests and `src-tauri/src/main.rs`)
- Generated: `src/generated/` through `cargo test -p marshell-protocol`
- Test: `crates/core/src/settings.rs` tests, `crates/core/tests/server.rs`, `crates/core/src/notify/mod.rs` or its tests, `crates/core/src/supervisor.rs` tests

**Interfaces:**
- Produces:
  - `Settings.close_to_tray: Option<bool>`. `None` means not chosen yet. Serialized as `close_to_tray`, absent or `null` reads as `None`, and ts-rs gives `boolean | null`. The settings PUT accepts and saves it like the other flags.
  - `CoreConfig.notify_paused: Arc<AtomicBool>`.
  - `AppState::notify_paused(&self) -> bool`.
  - `GET /v1/notify/paused` → `{"paused": bool}`.
  - `PUT /v1/notify/paused` with `{"paused": bool}` → 204. Both sit behind the existing token and origin guard.
  - The supervisor creates one flag and passes a clone to every core it starts.
  - `AppState::set_window_hidden(&self, hidden: bool)` and `Supervisor::set_window_hidden(&self, hidden: bool)`. While hidden, the notifier hands the engine an effective view with `window_focused: false` and no visible tabs, whatever `PUT /v1/ui/view` last said. The flag lives in the supervisor too, so a restarted core starts with it.

- [ ] **Step 1: Write the failing tests.**
  - `a_settings_file_without_close_to_tray_reads_none`.
  - `close_to_tray_round_trips_through_save_and_load` (`Some(false)` stays `Some(false)`).
  - In `server.rs`: `paused_starts_false_and_round_trips`; `paused_routes_need_the_token` (401 without the token, on both routes); `a_paused_body_that_is_not_a_bool_is_rejected` (400 or 422, whichever the router's JSON rejection already gives).
  - In the notifier: `nothing_is_delivered_while_paused_and_nothing_replays_after_resume`. A wait starts while paused, gets no request, and after Resume the next tick delivers no needs-you banner for it. Drive `spawn` with a test sink, or factor the loop body into a function you can call.
  - In the supervisor: `the_paused_flag_survives_a_core_restart`.
  - In the notifier: `a_hidden_window_has_no_tab_in_view`. The view says focused with tab t1 visible, then the window hides and the view is reported again unchanged; a main Stop on t1 still gives a Done request once 30 s out of sight have passed. After `set_window_hidden(false)`, the reported view counts again.
- [ ] **Step 2:** Run `cargo test -p marshell-core` and see those tests fail (compile errors count).
- [ ] **Step 3:** Implement the changes.
  - In the notifier loop, keep calling `observe` and `tick` every turn so the engine's state advances, and drop the requests while paused. A one-minute reminder armed during the pause can still fire after Resume for a wait that is still open, which is fine.
  - Test literals use `notify_paused: Default::default()`.
- [ ] **Step 4:** Run `cargo test -p marshell-protocol && cargo test -p marshell-core && pnpm check:generated`. Expected: all pass, and generated is clean after committing the regenerated `Settings.ts`.
- [ ] **Step 5:** Run `cargo fmt --all` and commit: `feat(core): add a pause for notifications and the saved close choice`.

### Task 2: The tray icon and its menu (shell)

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/Cargo.toml` (tauri `features = ["tray-icon"]`, windows-sys `Win32_System_Registry`), `Cargo.lock`, `src-tauri/src/main.rs` (module, tray built in `setup`, `set_tray` in the handler list, single-instance calls `tray::show_main`), `src-tauri/src/notify.rs` (its click calls `tray::show_main`), `THIRD_PARTY_NOTICES.md` (a `tray-icon` section in the existing format)
- Test: unit tests in `tray.rs`

**Interfaces:**
- Consumes: the webview's `TrayItem` shape: `{ id: string; label: string; kind: "item" | "session" | "separator"; shortcut?: string; number?: number; disabled?: boolean }`.
- Produces:
  - The `#[tauri::command] set_tray(app, model: TrayModel) -> Result<(), String>`, where `TrayModel { items: Vec<TrayItemDto>, tooltip: String, waiting: u32 }`, deserialized from camelCase JSON.
  - `pub fn show_main(app: &tauri::AppHandle)`, which unminimizes, shows and focuses the window, and calls `Supervisor::set_window_hidden(false)` through the managed `CoreHandle` when there is one.
  - `pub fn build(app: &tauri::AppHandle) -> tauri::Result<()>`, which creates the tray with id `"main"`, tooltip "Marshell", the "Open Marshell" and "Quit Marshell" items, and the icon for 0.
  - `pub fn tray_ready(app) -> bool`, true once `build` succeeded (Task 3 uses it).
  - `pub fn menu_label(item: &TrayItemDto) -> String`.
  - `pub fn tray_rgba(waiting: bool, light_taskbar: bool) -> (Vec<u8>, u32, u32)`.
  - The event `tray-action` with payload `{ "id": string }`.

- [ ] **Step 1: Write the failing tests.**
  - `a_session_item_reads_number_name_and_phrase`: number 3 with label "api-server: Wants to run `npm test`" gives "3 · api-server: Wants to run `npm test`".
  - `an_ampersand_is_doubled_so_it_is_not_a_mnemonic`.
  - `control_characters_become_spaces`: a tab or newline in a label leaves one line.
  - `a_long_label_is_cut_to_64_characters_with_an_ellipsis` (counted in chars, not bytes, and safe for multi-byte text).
  - `the_shortcut_follows_a_tab`: "Go to the oldest waiting session\tCtrl+Shift+N", the Windows menu convention, display only.
  - `tray_rgba_is_32_square_and_the_dot_shows_only_when_waiting`: the dot's top-right pixel is transparent at 0 and the accent `[0xD9, 0x4F, 0x2B, 0xFF]` when waiting.
  - `the_wands_are_white_on_a_dark_taskbar_and_black_on_a_light_one`: a pixel on a wand stroke reads `[0xFF,0xFF,0xFF,_]` or `[0,0,0,_]`.
  - `a_model_reads_from_the_webviews_json`: a camelCase JSON sample deserializes into `TrayModel`.
- [ ] **Step 2:** Run `cargo test -p marshell-app tray` and see them fail.
- [ ] **Step 3:** Implement.
  - **The icon.** Draw it as `badge_rgba` does in `attention.rs`: two round-capped strokes on the 1024 viewBox of `assets/icon/tray-mono.svg`, from (400,884) to (168,150) and from (624,884) to (856,150), stroke 170, scaled to 32 px. Anti-alias by the pixel's distance to the segment, and draw the dot as a circle at (830,190), r 170.
  - **The taskbar theme.** Read `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme` with `RegGetValueW`; a missing value means dark. Off Windows it is dark, and on macOS the icon is set `icon_as_template(true)`. Read it again on every `set_tray` and on `WindowEvent::ThemeChanged`.
  - **Clicks.** Set `show_menu_on_left_click(false)`, and on `TrayIconEvent::Click` with the left button and state `Up`, call `show_main`.
  - **Menu events.** `open` calls `show_main`, and `quit` calls `app.exit(0)`. `pause` emits `tray-action` without showing the window. Every other id (`session-N`, `next-waiting`, `more`, `notifications`) calls `show_main` first, then emits `tray-action`.
  - **Updates.** `set_tray` rebuilds the menu from the items: separators, disabled items and `menu_label` text. It sets the tooltip, and sets the icon only when `waiting > 0` changes or the theme changes.
- [ ] **Step 4:** Run `cargo test -p marshell-app && cargo clippy --workspace --all-targets -- -D warnings`. Expected: pass and clean.
- [ ] **Step 5:** Run `cargo fmt --all` and commit: `feat(tray): add the tray icon and menu, drawn and handled by the shell`.

### Task 3: The close button hides to the tray (shell, inline)

**Files:**
- Modify: `src-tauri/src/main.rs`, plus `src-tauri/src/tray.rs` if `close_action` lives there
- Test: unit tests in `main.rs`

**Interfaces:**
- Consumes: `tray::tray_ready`, `tray::show_main`, `marshell_core::settings::load(&home).close_to_tray` (Task 1). The shell keeps the home path in managed state.
- Produces: `enum CloseAction { Quit, Hide, Ask }` and `fn close_action(tray_ready: bool, close_to_tray: Option<bool>) -> CloseAction`, plus the event `close-choice` with no payload.

- [ ] **Step 1: Write the failing tests.**
  - `close_action_quits_without_a_tray_whatever_was_saved`.
  - `close_action_follows_the_saved_choice` (`Some(true)` → Hide, `Some(false)` → Quit).
  - `close_action_asks_when_nothing_is_saved`.
  - Extend `saved_window_state_leaves_the_frame_to_the_config` so it also asserts that `!window_state_flags().contains(StateFlags::VISIBLE)`.
- [ ] **Step 2:** Run `cargo test -p marshell-app` and see them fail.
- [ ] **Step 3:** Implement.
  - In `on_window_event`, on `CloseRequested { api, .. }` for the window labelled `"main"`, match `close_action`:
    - Quit: `api.prevent_close()`, then `app.exit(0)` explicitly, so a live tray icon can't keep the process alive.
    - Hide: `api.prevent_close()`, then `Supervisor::set_window_hidden(true)`, then `window.hide()`.
    - Ask: `api.prevent_close()`, then `show_main`, then emit `close-choice`.
  - Remove `VISIBLE` from `window_state_flags`.
- [ ] **Step 4:** Run `cargo test -p marshell-app && cargo clippy --workspace --all-targets -- -D warnings`. Expected: pass and clean.
- [ ] **Step 5:** Run `cargo fmt --all` and commit: `feat(window): hide the window to the tray on close, asking once`.

### Task 4: The webview side: count, title, tray sync, pause, close dialog

**Files:**
- Modify:
  - `src/features/attention/attention.ts`: rename `badgeCount` to `needsYouCount`, same body, and update its users.
  - `src/features/attention/useAttention.ts`: return the count.
  - `src/features/window/windowTitle.ts` and `MainWindow.tsx`: the title takes the count.
  - `src/features/sessions/LiveApp.tsx`: wiring.
  - `src/features/tray/trayItems.ts`, its tests, and `Tray.stories.tsx`/`TrayMenu.test.tsx`, so the design stories keep showing every item.
  - `src/lib/keymap.ts` and `src/features/sessions/LivePalette.tsx`: the palette command.
  - `src/features/notifications/NotificationsDialog.tsx`: the Close button section.
  - `src/features/appearance/settingsSync.ts`: write `close_to_tray`.
- Create: `src/features/tray/useTray.ts` and its test, `src/features/window/CloseDialog.tsx` and its test, and a small API client for `/v1/notify/paused` next to the existing settings or notifications client.
- Test: `pnpm test src/features/tray src/features/attention src/features/window src/features/notifications src/features/approval`

**Interfaces:**
- Consumes: `set_tray` and `tray-action` `{ id }` from Task 2, `close-choice` from Task 3, `GET`/`PUT /v1/notify/paused` and `Settings.close_to_tray` from Task 1.
- Produces:
  - `needsYouCount(entries: { tab: string }[]): number`.
  - `windowTitle(row: RowModel | null, waiting: number): string`.
  - `TrayState` becomes `{ os; sessions; paused; doctorIssues: number | null; departures: boolean }`. `null` hides Doctor, and `departures: false` hides Open Departures. The `settings` item becomes `{ id: "notifications", label: "Notifications…" }`.
  - `traySessions(entries, rows): TraySession[]`, built from the lane entries in their order (oldest first), with each entry's row for status and phrase. A held entry whose row is missing uses status `needs-permission` and the entry's session name.
  - The palette command `{ id: "pause-notifications", label: "Pause or resume notifications", group: "Tools" }`, palette-only.

- [ ] **Step 1: Write the failing tests.**
  - In `fromCore.test.ts`, `an_expired_hold_still_counts_while_the_row_waits`: no views, and a row with `needs-permission` that was held before, so the entries hold one terminal-only entry for the tab. Also `a_muted_row_still_counts`.
  - `windowTitle`:
    - `(2) api-server — Marshell` at 2;
    - `(1) Marshell` with no row;
    - `api-server — Marshell` at 0;
    - a whitespace-only name with 3 waiting gives `(3) Marshell`.
  - `trayItems`:
    - with `doctorIssues: null` and `departures: false`, the ids are exactly `open, sep-1, none, next-waiting, sep-2, pause, notifications, sep-3, quit`;
    - with waiting sessions, the ids are `open, sep-1, session-3, session-2, next-waiting, sep-2, pause, notifications, sep-3, quit`;
    - the design state (`doctorIssues: 4, departures: true`) still lists departures and doctor.
  - `traySessions`: it keeps the lane order, includes an expired-hold row, and falls back for a held entry without a row.
  - `useTray`, through its pure helper `trayModel(state): { items, tooltip, waiting }`:
    - waiting equals the number of sessions and not just the five shown, so 8 waiting gives `waiting: 8`;
    - the tooltip carries "(notifications paused)" while paused.
  - The `tray-action` handler, through a pure `actionFor(id)`. The tray's `pause` toggles notifications, never the palette's Pause session: `session-7` → `{ kind: "goto", number: 7 }`; `next-waiting` and `more` → `{ kind: "nextWaiting" }`; `notifications` → `{ kind: "overlay", overlay: "notifications" }`; `pause` → `{ kind: "togglePause" }`; anything else → `null`.
  - `CloseDialog` (`renderToStaticMarkup`): it is `role="dialog"` with an accessible name, has the exact copy and both buttons, and "Keep running in the tray" comes first and is marked as the default (autofocus).
  - The Notifications dialog: a "Close button" group with a checkbox "Keep Marshell running in the tray when I close the window". It is checked unless `close_to_tray` is `false`.
- [ ] **Step 2:** Run `pnpm test <those folders>` and see them fail.
- [ ] **Step 3:** Implement.
  - **The title.** `LiveApp` computes `needsYouCount` once from the lane entries and passes it to `useAttention` (the badge) and to `MainWindow` (the title).
  - **The tray.** `useTray` calls `invoke("set_tray", { model })` only when the serialized model changes, and ignores failures as `attention.ts`'s `tell` does.
  - **Pause.** `LiveApp` reads the paused state at start from `GET /v1/notify/paused`, and again after `core-restarted`. Toggling sends a PUT, updates the state, and announces "Notifications paused" or "Notifications resumed".
  - **`close-choice`.** It opens `CloseDialog`. Keep running saves `close_to_tray: true` and then calls `appWindow.close()` again; Quit when I close saves `false` and then closes. Escape or cancel only closes the dialog. Wait for the save before calling close.
  - **The Notifications dialog.** Its checkbox saves `Some(checked)` through `settingsSync`.
- [ ] **Step 4:** Run `pnpm test && pnpm build`. Expected: all pass, and the build succeeds.
- [ ] **Step 5:** Commit: `feat(tray): sync the tray and title with the needs-you count, and ask once on close`.

### Task 5: Docs (inline)

- [ ] **Step 1:** Add a short "Tray and close button (phase 4b)" note to the BRIEF amendments if §5.5 or the window section says the close button quits; otherwise there is nothing to amend. Update `docs/a11y/` only if a live-app tray row exists there; the batch 3 script rows F5 and F6 cover the design page and stay.
- [ ] **Step 2:** Commit: `docs(tray): record the tray and close button behaviour`.

### Task 6: Live check (main thread)

Run it on a `pnpm tauri dev` build of the branch with a scratch `MARSHELL_HOME` and CDP on port 9333, using plain cmd sessions and hook bodies posted to `/v1/hook` as in 4a. Write the findings to `docs/research/2026-10-22-tray-live-check.md`.
- [ ] **Step 1: The count.** Two sessions need you. Check the title "(2) … — Marshell", the badge 2, the tooltip "Marshell · 2 need you" and the menu listing both. Then let a hold expire after 120 s, record `GET /v1/sessions` and the permissions, and check that the count holds.
- [ ] **Step 2: The first close.** With nothing saved, the dialog appears. Escape keeps the window open. Keep running hides it, sessions keep running, and a banner still arrives. A Done on the tab that was on screen gets its banner after 30 s. A left click on the tray brings the window back.
- [ ] **Step 2b: A hidden webview keeps the tray current.** Hide the window and wait more than 5 minutes. Start a new wait and time how long the tooltip and menu take to show it. If they lag by more than a few seconds, stop and rethink who drives the tray before going on.
- [ ] **Step 3: The menu.** A session item selects that session, Go to oldest works, Pause stops the banner and the sound for a new wait while the count still rises, Resume restores them, and Notifications… opens the dialog.
- [ ] **Step 4: Quit and restart.** Quit from the tray while hidden; the process is gone and the sessions have ended, and the next start shows the window. The Notifications checkbox switched off makes the close button quit, and the process ends.
- [ ] **Step 5: The look and the keyboard.** Check the icon on a dark and a light taskbar (screenshot only to judge the icon, never shared). Win+B and the arrow keys reach the menu. The close dialog works in forced colors.
- [ ] **Step 6:** Commit: `docs(research): record the tray live check`.

### Task 7: Final review, PR

- [ ] Run the Opus whole-branch review and one fix pass. Then CI's commands locally, the PR with "What it does", "Live check", "Known limits" and "Checks", a green CI and the merge.
