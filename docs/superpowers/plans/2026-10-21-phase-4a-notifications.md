# Phase 4a: notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a session needs you, finishes, stops on an error or a subagent ends while you are elsewhere, Marshell tells you once, with a silent Windows banner that jumps to the session and a Windows sound per category, following the rules in brief §5.5.

**Architecture:** The notification rules move into the Rust core (`crates/core/src/notify/`): a pure engine folds session snapshots, the UI's view (window focus, visible tabs), settings and the clock into `NotifyRequest`s. A driver task feeds it and hands each request to the shell through the supervisor's event callback (`CoreEvent::Notify`). The shell shows the banner (spike S3 picks the crate), plays the sound unless Windows is in do-not-disturb, and flashes the taskbar. A banner click focuses the window and tells the webview which tab to select. Done settles for 1.5 s in the core before anything sees it. The webview keeps only the taskbar badge count.

**Tech Stack:** Rust (marshell-core, marshell-protocol with ts-rs, marshell-app on Tauri 2), React + TypeScript, Vitest. One new dependency (decided by Task 1): `tauri-plugin-notification` (brief §3 plugin list), or `tauri-winrt-notification` if the plugin's click callback fails on Windows.

**Spec:** `docs/PLAN.md` §9 "Phase 4 decisions (2026-10-21)" and "Slice 4a decisions (2026-10-21)"; `docs/BRIEF.md` §5.5; PLAN "UX risks" (notification fatigue row), the per-OS table rows "Notifications" and "DND for our sounds", "Batch 3 design decisions" (Banners); the copy and rules in `src/features/notifications/banner.ts` (designed and reviewed in batch 3); `docs/research/2026-10-21-phase-3-exit-check.md` finding 4.

## Global Constraints

- Banners are always silent, never raise the window, and a click only focuses the window and selects the session. 4a banners offer Open, or Review for a risky request; never Allow or Deny (slice 4a decision 1).
- `marshell://` and any activation path only ever focus; nothing in this slice answers a permission request.
- Window focused → no banner, no sound, no flash, for any session (slice 3d decision 8, `bannerFor`).
- Batching window 3 s (`BATCH_WINDOW_MS`); Done banners only for a session in the background for more than 30 s (`DONE_MIN_BACKGROUND_MS`); one escalation per wait, 60 s after a needs-you that has not been seen; at most one sound per category per 3 s; subagent done never gets a banner and its sound is off by default.
- Done settle 1.5 s (slice 4a decision 2). Quiet hours off by default, prefilled 22:00–08:00, silence sounds and the flash only (slice 4a decision 3). Windows do-not-disturb silences sounds and the flash the same way.
- Sounds are Windows sound aliases played with `PlaySoundW(SND_ALIAS | SND_ASYNC | SND_NODEFAULT)`, one per category, with no per-category volume (phase 4 decision 2). macOS and Linux get `cfg` no-op stubs, marked unverified (phase 4 decision 4).
- Copy follows PLAN microcopy rules: sentence case, the session name, concrete numbers, no "!".
- Unknown values are left out: no session name → "Session N"; no phrase → the title alone.
- New dependencies only as Task 1 records them (SCOPE.md policy; notice in `THIRD_PARTY_NOTICES.md`). windows-sys gains features (`Win32_UI_Shell`, `Win32_System_SystemInformation`), not a new crate.
- `cargo fmt --all` before every Rust commit; `pnpm check:generated` after a protocol change. Tests and live checks never touch the real `~/.marshell` or `~/.claude`.
- Commit messages `type(scope): what changed, as a sentence`, ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Execution

Tasks 2, 3, 4, 5 and 7 go to Codex CLI (`codex exec`, one worktree per task under `C:/dev/codex-wt/`, branched from `phase-4a`), or to a `worker` when Codex is out of quota; Tasks 3 and 5 (timers, concurrency) run at high effort. The main thread reviews each diff, runs the tests and cherry-picks. Tasks 1, 6 and 9 need the live app and run in the main thread; Task 8 is inline.

## Review Focus

1. A needs-you that is answered within the batching window or before the escalation: no trailing batch banner and no escalation for it (the engine re-checks the tab's current state at flush and escalation time).
2. Quiet hours that wrap midnight (22:00–08:00), equal start and end (treated as off), and a malformed time string saved by hand in `settings.json` (load falls back to the defaults, never fails the whole settings file).
3. A core restart (supervisor generation +1): the new core's engine starts empty and must not replay a banner for every session already waiting; the first snapshot only seeds state.
4. A muted session that needs you: no banner, no sound, no flash, and it still counts in the taskbar badge.
5. The UI never reports a view (webview not loaded yet, or a crash): the engine treats the window as not focused and every tab as in the background, so nothing is silently dropped.

---

### Task 1: Spike S3, banner click-to-focus on Windows (inline, not a worker)

**Files:**
- Create: `docs/spikes/S3-notification-click.md`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, `src-tauri/capabilities/default.json` (only if the plugin needs a permission), `Cargo.lock`
- Create: `src-tauri/src/notify.rs` (kept and finished in Task 6)

- [ ] **Step 1:** Add `tauri-plugin-notification` (latest 2.x; check licence MIT/Apache, active upstream). Register it, add `on_action` that logs the action id and, for a tap, unminimizes, shows and focuses the `main` window. Add a temporary debug-only command `debug_banner` that sends one banner with no sound and an `Open` action, tagged with a test tab id.
- [ ] **Step 2:** Live check under `pnpm tauri dev` (scratch `MARSHELL_HOME`, CDP per CLAUDE.md): fire it with the window minimised and behind another window; record whether the toast shows, whether it is silent, what app name it carries (expect PowerShell or similar without an AUMID), and whether a click on the body and on Open reaches `on_action` and focuses the window.
- [ ] **Step 3:** Repeat on an installed build (`pnpm tauri build`, install the NSIS per-user package, run with a scratch `MARSHELL_HOME`), where the AUMID comes from the Start-menu shortcut. Also check a click after the toast moved to the Action Center. The dev and installed builds share the identifier and single-instance, so never run both at once. The install goes on the user's own machine (told in the plan handoff); uninstall it after the spike and check that its Start-menu entry is gone.
- [ ] **Step 3b:** Turn on Windows 11 do-not-disturb and call `SHQueryUserNotificationState`; record the value it returns. If it still says `QUNS_ACCEPTS_NOTIFICATIONS`, the do-not-disturb rule becomes a known limit in §9, and Task 7 leaves out the dialog line that promises it.
- [ ] **Step 4:** If the click never reaches the app in the installed build, replace the plugin with `tauri-winrt-notification` (toast with an activation callback) and repeat Step 3. If neither works, the fallback is the `marshell://focus/<tab>` link plus single-instance, and that goes back to the user before Task 6.
- [ ] **Step 5:** Write `docs/spikes/S3-notification-click.md` (pass/fail per PLAN's S3 criterion, the crate chosen, dev vs installed, Action Center, the AUMID note). Remove `debug_banner`. Commit: `spike(notify): check banner click-to-focus on Windows and pick the banner crate`.

### Task 2: Protocol and settings

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Settings` ~L751, `SessionInfo` ~L131), regenerate `src/generated/`
- Modify: `crates/core/src/settings.rs` (`FileDoc` ~L10, `load` ~L37, `save` ~L73)
- Modify: `src/features/appearance/settingsSync.ts`, `src/features/agents/agentsState.ts`, `src/features/agents/AgentsDialog.tsx` (compile fixes only; the switch moves in Task 7)
- Test: `settings.rs` unit tests, `api.rs` serde tests, `settingsSync` test

**Interfaces:**
- Produces (protocol, ts-rs exported, serde `snake_case` fields like the rest of `Settings`):
  - `pub struct NotifyChannel { pub banner: bool, pub sound: bool }`
  - `pub struct QuietHours { pub enabled: bool, pub start: String, pub end: String }`, with `"HH:MM"` 24-hour strings.
  - `pub struct NotifySettings { pub needs_you: NotifyChannel, pub done: NotifyChannel, pub error: NotifyChannel, pub subagent_sound: bool, pub quiet_hours: QuietHours }`, with `Default` = needs_you/done/error `{banner: true, sound: true}`, `subagent_sound: false`, quiet hours `{enabled: false, start: "22:00", end: "08:00"}`.
  - `Settings.notifications: NotifySettings` (`#[serde(default)]`); `Settings.sound_off` is removed from the protocol.
  - `SessionInfo.muted: bool` (`#[serde(default)]`).
  - `pub struct UiView { pub window_focused: bool, pub visible_tabs: Vec<String> }`.
  - `pub fn parse_hhmm(s: &str) -> Option<u16>` (minutes since midnight) in `crates/core/src/settings.rs`.

- [ ] **Step 1: Write the failing tests**
  - `settings_default_notifications`: loading a file without `notifications` gives the defaults above.
  - `legacy_sound_off_maps_to_needs_you_sound`: a file with `"sound_off": true` and no `notifications` loads `needs_you.sound == false` and the rest default; saving writes `notifications` and no `sound_off`.
  - `bad_quiet_hours_fall_back`: `"quiet_hours": {"enabled": true, "start": "25:00", "end": "08:00"}` loads as the default quiet hours (disabled), and the other settings in the file still load.
  - `parse_hhmm`: `"00:00"`→0, `"08:00"`→480, `"23:59"`→1439; `"24:00"`, `"8:00"`, `"08:60"`, `""` → `None`.
  - Serde round-trip of `NotifySettings` and `UiView`; `SessionInfo` without `muted` deserializes with `false`.
- [ ] **Step 2:** `cargo test -p marshell-protocol` and `cargo test -p marshell-core settings` fail to compile.
- [ ] **Step 3:** Implement. `FileDoc` keeps a read-only `#[serde(default, skip_serializing)] sound_off: bool` for the migration; validation on load: both times parse with `parse_hhmm`, else replace `quiet_hours` with the default. In the frontend, `settingsSync.ts` writes `notifications` through and drops `sound_off`; `soundOff`/`setSoundOff` read and write `notifications.needs_you.sound` until Task 7 replaces them.
- [ ] **Step 4:** `cargo test -p marshell-protocol`, `cargo test -p marshell-core settings`, `pnpm check:generated`, `pnpm test src/features/appearance src/features/agents`. Expected: pass.
- [ ] **Step 5:** Commit: `feat(protocol): add notification settings, the session mute flag and the UI view`.

### Task 3: Done settles for 1.5 s (core)

**Files:**
- Modify: `crates/core/src/registry.rs` (beside `follow_subagent_wait` ~L512), `crates/core/src/server/hook.rs` (~L151), `crates/core/src/server/mod.rs` (`DEFAULT_SUBAGENT_WAIT` ~L125, `subagent_wait` ~L171)
- Test: `registry.rs` unit tests; the hook route integration test file that covers the 3g subagent wait

**Interfaces:**
- Produces: `pub const DEFAULT_DONE_SETTLE: Duration = Duration::from_millis(1500)` and `AppState::done_settle() -> Duration` (overridable in tests the same way `subagent_wait` is); on `Registry`: `pub fn hold_done(&self, tab_id: &str, done: LiveStatus) -> bool` (true when held) and `pub fn settle_done(&self, tab_id: &str)`.

- [ ] **Step 1: Write the failing tests**
  - Registry `held_done_keeps_working_until_settled`: a Running claude row in Working "Editing a.rs"; `hold_done(tab, done_with_recap)` → the row is still Working with the same phrase, and no broadcast; `settle_done(tab)` → Done with the recap, one broadcast.
  - Registry `newer_event_cancels_held_done`: `hold_done`, then `update_live` with a `UserPromptSubmit` fold → Working "Thinking"; a later `settle_done` changes nothing. The same with `PreToolUse`, and with `SessionEnd` (→ Ended, stays Ended).
  - Hook route `stop_then_resume_never_shows_done` (finding 4 replay): with `done_settle` at 300 ms, POST `Stop` (no running `background_tasks`), then 78 ms later `UserPromptSubmit`; poll `GET /v1/sessions` every 20 ms for 600 ms: no snapshot has `live.state == "done"`.
  - Hook route `stop_alone_settles_to_done`: POST `Stop`; at 100 ms the row is Working; at 500 ms it is Done with the recap from `last_assistant_message`.
  - The existing 3g subagent-wait tests still pass unchanged (a Stop that keeps "Waiting for subagents" is not held; the final `settle_subagent_wait` Done applies at once).
- [ ] **Step 2:** `cargo test -p marshell-core held_done newer_event stop_then_resume stop_alone` fail.
- [ ] **Step 3:** Implement. In `hook.rs`, for a `Stop` body: compute `fold_event` first; if it returns `Done`, call `hold_done` instead of applying it and spawn `sleep(done_settle)` then `settle_done`, the same bare `tokio::spawn` pattern as the subagent wait. `Entry` gains `pending_done: Option<LiveStatus>`; the next hook event folded for the tab (not a status line update, which arrives every few seconds and must not cancel it; add a test for that), `settle_subagent_wait` or a process change on the tab clears it; `settle_done` applies it only if still present and the row is Running.
- [ ] **Step 4:** `cargo test -p marshell-core`. Expected: pass.
- [ ] **Step 5:** Commit: `fix(status): hold a Stop for 1.5 s before the row reads Done`.

### Task 4: Mute and the UI view (core routes)

**Files:**
- Modify: `crates/core/src/registry.rs`, `crates/core/src/server/mod.rs` (routes ~L362-428, `AppState`), `crates/core/src/layout.rs` only if the tab name is persisted there (then `muted` is persisted the same way)
- Create: `crates/core/src/server/view.rs`
- Test: `crates/core/tests/server.rs` (token and origin guard tests follow the existing route tests)

**Interfaces:**
- Produces: `PUT /v1/sessions/{tab_id}/muted` with body `{"muted": bool}` → 204, 404 for an unknown tab; `Registry::set_muted(&self, tab_id: &str, muted: bool) -> bool` (broadcasts on a change). `PUT /v1/ui/view` with a `UiView` body → 204; `AppState::view() -> watch::Receiver<UiView>` (initial value `UiView { window_focused: false, visible_tabs: vec![] }`).

- [ ] **Step 1: Write the failing tests:** mute round-trip shows in `GET /v1/sessions`; a second identical PUT broadcasts nothing; unknown tab 404; no token 401 on both routes; a `UiView` PUT is visible through `AppState::view()`; `visible_tabs` over 16 entries or a body over 4 KiB is rejected with 400.
- [ ] **Step 2:** Run, see them fail.
- [ ] **Step 3:** Implement with the existing token guard and router conventions. If the tab name survives an app restart through `layout.json`, `muted` does too; otherwise it lives with the tab.
- [ ] **Step 4:** `cargo test -p marshell-core`. Expected: pass.
- [ ] **Step 5:** Commit: `feat(core): add per-session mute and the UI view route`.

### Task 5: The notification engine and its driver (core)

**Files:**
- Create: `crates/core/src/notify/mod.rs` (driver), `crates/core/src/notify/engine.rs` (pure state machine), `crates/core/src/notify/copy.rs` (banner text), `crates/core/src/notify/clock.rs` (local minutes of the day)
- Modify: `crates/core/src/lib.rs`, `crates/core/src/server/mod.rs` (`CoreConfig` ~L43, `start` ~L528), `crates/core/src/supervisor.rs` (`CoreEvent` ~L17, `on_event` becomes an `Arc`), `crates/core/Cargo.toml` (windows-sys `Win32_System_SystemInformation`)
- Test: in-file unit tests in `engine.rs` and `copy.rs`; `crates/core/tests/notify.rs` for the driver

**Interfaces:**
- Consumes: `UiView`, `NotifySettings`, `SessionInfo.muted` (Task 2); `AppState::view()` (Task 4); `Registry::subscribe()`; `holds().views()` (`PermissionView.tab`, `.risk`).
- Produces:
  - `pub enum SoundKind { NeedsYou, Escalation, Done, SubagentDone, Error }`
  - `pub enum BannerAction { Open, Review }`
  - `pub struct Banner { pub title: String, pub body: String, pub meta: String, pub action: BannerAction, pub tab_id: Option<String> }`, where `tab_id` is `None` for a batch (a click opens the window only).
  - `pub struct NotifyRequest { pub banner: Option<Banner>, pub sound: Option<SoundKind>, pub flash: bool }`
  - `pub type NotifySink = Arc<dyn Fn(NotifyRequest) + Send + Sync>`; `CoreConfig.notify: Option<NotifySink>` (None in tests that don't care); `CoreEvent::Notify(NotifyRequest)`.
  - `engine::Engine::new() -> Engine`, `fn observe(&mut self, snap: &SessionsSnapshot, risky_tabs: &HashSet<String>, settings: &NotifySettings, minute_of_day: u16, now: Instant) -> Vec<NotifyRequest>`, `fn set_view(&mut self, view: UiView, now: Instant)`, `fn tick(&mut self, settings: &NotifySettings, minute_of_day: u16, now: Instant) -> Vec<NotifyRequest>`.
  - `clock::local_minute_of_day() -> u16` (Windows `GetLocalTime`; Unix `libc::localtime_r` if `libc` is already a dependency, else a `cfg` stub returning UTC minutes, marked unverified).

The rules the engine owns (each one a test in `engine.rs`):
- **Seeding.** The first `observe` only records states, so a core restart replays nothing (Review Focus 3).
- **Events** come from differences between snapshots. The registry is a `watch` channel, so snapshots coalesce, and a state comparison alone would miss a second wait. Each wait therefore has an identity: a permission wait is the tab's open hold `rid` (`holds().views()`); a question, Done or Error is `live.since_ms`. A new identity in `NeedsPermission`/`NeedsQuestion` → needs-you (so two requests on one tab inside one interval, and permission → question, are two events); a new Done → done; a new Error → error. `Ended`, `Idle` and `Working` make no event. A muted tab, or a category whose banner and sound are both off, makes none.
- **Subagent done** is a drop in `live.subagents` only while the tab stays Working on the same process (the same `SessionInfo` generation, or whatever field marks a restart); one event per drop. The count also clears at the main `Stop`, at `SessionEnd` and on a restart (slice 3g), and those clears make no event (one test for each).
- **Error has no producer yet**: nothing in the core sets `LiveState::Error` today (no Claude hook in `fold_event` maps to it). The category is built and tested in the engine, so it fires once an adapter sets Error; the live check skips it, and the PR's Known limits say so.
- **Focus.** `window_focused` → nothing at all (no banner, no sound, no flash). A tab is "in the background since T" from the moment it stopped being (window focused and the tab in `visible_tabs`); with no view ever reported, everything is in the background since the engine started (Review Focus 5).
- **Done** banner only when the tab has been in the background for more than 30 s; the done sound follows the same rule.
- **Batching**, per category (needs-you, done, error): the first event goes out at once. Further events of that category within the next 3 s are collected; at the end of the window, those whose tab is still in that state go out as one batch banner listing every collected session, the first one included. If none are left, nothing goes out (Review Focus 1). Sounds in a batch follow the 3 s per-category limit.
- **Escalation.** A tab still needing you 60 s after its needs-you event, never seen in between (seen = window focused with the tab visible), gets one escalation banner and `SoundKind::Escalation`. A new needs-you wait on that tab resets it.
- **Sound and flash.** `sound` comes from the category's switch (`subagent_sound` for subagent done) and the 3 s limit. `flash` is true only for needs-you and its escalation. Quiet hours (start ≤ now < end, or wrapping midnight when start > end; equal start and end means off) set `sound: None, flash: false` and leave the banner.
- **Banners.** Needs-you, done and error get banners when their switch is on; subagent done never does. A risky tab (an open hold with `risk`) gets `BannerAction::Review` and a body that never shows the command.

Copy (`copy.rs`, ported from `banner.ts`; each line is a test):
- Needs you: title `"<name> needs you"`; body = the row's phrase (`"Wants to run `npm test`"`, `"Has a question"`); risky body `"Needs a careful look."`; a permission phrase longer than 160 characters or with a line break → `"Wants to run a command that does not fit here."`
- Done: `"<name> is done"`, body = the recap's first line, else `"Done"`.
- Error: `"<name> stopped"`, body `"Stopped: <phrase>."`
- Meta: `"Session <n> · <project>"`, or `"Session <n>"` with no project.
- Batches: `"<k> sessions need you"` / `"<k> sessions are done"` / `"<k> sessions stopped"`, body `"Sessions 3 api-server, 5 web and 7 docs"`, meta `"<k> sessions"`, Review if any is risky, else Open.
- Escalation: the needs-you title, body `"<name> has waited <m> min to run a command"` or `"… for an answer"`.

- [ ] **Step 1: Write the failing tests:** one per rule above in `engine.rs`, driving `Instant`s by hand (no sleeping); the copy lines in `copy.rs`; a quiet-hours table test (22:00–08:00 at 21:59/22:00/03:00/07:59/08:00; 09:00–17:00; 10:00–10:00 → off).
- [ ] **Step 2:** `cargo test -p marshell-core notify` fails to compile.
- [ ] **Step 3:** Implement the engine and copy. Then the driver: `notify::spawn(state: AppState, sink: NotifySink)` started from `server::start` when `cfg.notify` is `Some`. It `select!`s on registry changes, view changes and a 250 ms interval; it reads settings per tick from the in-memory settings the core already holds, never from disk, computes `risky_tabs` from `holds().views()`, and calls the sink for each request. `Supervisor::start` sets `cfg.notify` to a sink that calls `on_event(CoreEvent::Notify(req))`, and keeps it on restarts.
- [ ] **Step 4:** Driver test in `tests/notify.rs`: start a core with a sink that collects requests, put a view with `window_focused: false`, drive a fake tab into NeedsPermission through `POST /v1/hook` (as the 3d tests do) → one request within 500 ms with a banner titled `"<name> needs you"`, `sound: Some(NeedsYou)`, `flash: true`; `PUT /v1/ui/view` with `window_focused: true`, and a second tab needing you → no request. Then `cargo test -p marshell-core` and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Step 5:** Commit: `feat(notify): decide banners, sounds and flashes in the core`.

### Task 6: The shell delivers them (inline, not a worker)

**Files:**
- Modify: `src-tauri/src/notify.rs` (from Task 1), `src-tauri/src/attention.rs`, `src-tauri/src/main.rs` (`emit_core_event` ~L27, single-instance ~L116, `invoke_handler` ~L183), `src-tauri/Cargo.toml` (windows-sys `Win32_UI_Shell`)
- Modify: `src/lib/api/coreEvents.ts`
- Test: unit tests in `attention.rs`/`notify.rs` for the pure parts (alias table, DND mapping)

**Interfaces:**
- Consumes: `CoreEvent::Notify(NotifyRequest)` (Task 5); the banner crate from Task 1.
- Produces: the Tauri event `"focus-session"` with payload `{ "tab_id": string | null }`; in TS, `CoreEvent` gains `{ kind: "focus-session"; tabId: string | null }`.

- [ ] **Step 1:** Tests for `fn sound_alias(kind: SoundKind) -> &'static str`: NeedsYou `"Notification.Default"`, Escalation `"Notification.Reminder"`, Done `"Notification.Mail"`, SubagentDone `"Notification.IM"`, Error `"SystemHand"`. In the default Windows 11 scheme these are five different files (checked on the dev machine 2026-10-21: Generic, Calendar, Email, Messaging, Foreground; `Notification.SMS` shares Messaging with IM, so it is not used). And `fn quiet_for_os(state: i32) -> bool`: true for every `QUERY_USER_NOTIFICATION_STATE` except `QUNS_ACCEPTS_NOTIFICATIONS` (5) and `QUNS_APP` (7). And a banner escaping test: a phrase containing `<`, `&`, `"` and `</text><action activationType="protocol" arguments="https://example.com"/>` reaches the toast as text. Check the toast XML the crate builds, or escape before handing it over if the crate does not, so no action or link can be injected.
- [ ] **Step 2:** Implement `emit_core_event` for `Notify`: banner through the chosen crate (silent, tagged by category so a batch replaces its first banner where the crate allows); sound through `PlaySoundW` with the alias unless `SHQueryUserNotificationState` says quiet; flash through `request_user_attention(Informational)` with the same quiet check. A banner click unminimizes, shows and focuses `main`, then emits `"focus-session"`. `alert_needs_you` and its sound path are removed (the core decides now); `set_badge` stays.
- [ ] **Step 3:** `cargo test -p marshell-app` (or the src-tauri package name), `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm build`.
- [ ] **Step 4:** Commit: `feat(shell): show the core's banners, play its sounds and jump to the session on a click`.

### Task 7: The webview side

**Files:**
- Create: `src/features/notifications/NotificationsDialog.tsx` (+ `.css`, `.test.tsx`), `src/features/notifications/useViewReport.ts` (+ test)
- Modify: `src/features/sessions/LiveApp.tsx` (overlay ~L1262, `pick` ~L149, `useAttention` ~L266), `src/features/attention/attention.ts` and `useAttention.ts` (badge only), `src/features/sidebar/sessionMenu.ts` (+ test), `src/features/agents/AgentsDialog.tsx` (the needs-you sound switch moves out), `src/lib/keymap.ts` (a palette command "Notifications…"), the row mapping that fills `muted` from `SessionInfo.muted`
- Test: Vitest beside each file

**Interfaces:**
- Consumes: `PUT /v1/ui/view`, `PUT /v1/sessions/{id}/muted` (Task 4); `Settings.notifications` (Task 2); `"focus-session"` (Task 6).

- [ ] **Step 1: Write the failing tests**
  - `useViewReport`: the report `{window_focused, visible_tabs}` is sent on focus, blur and pane changes, at most once per 200 ms, never twice with the same value, and again after a core restart.
  - `sessionMenu`: a running or ended session has "Mute notifications" / "Unmute notifications" (one label per state, as the toggle rule in PLAN "Accessibility safety net" needs: the label names the action and is not a toggle that renames itself; follow how Pause/Continue does it).
  - `NotificationsDialog` (rendered with `renderToStaticMarkup`): per category a "Banner" and a "Sound" switch for Needs you, Done and Error, and one "Sound" switch for Subagent done (off by default, help "A short sound when a subagent finishes."); Quiet hours with an on switch and two time fields prefilled 22:00 and 08:00, and the help "No sounds or taskbar flashes between these times. Banners still arrive, silently."; a line saying Windows do-not-disturb silences sounds too, only if Task 1 Step 3b showed the API sees it. Every control has a label; the dialog follows `useDialogFocus` like the Agents dialog.
  - `useAttention`: it sets only the badge count; it never calls a sound or flash.
- [ ] **Step 2:** `pnpm test src/features/notifications src/features/sidebar src/features/attention` fails.
- [ ] **Step 3:** Implement. `"focus-session"` with a tab id calls `pick(tabId)`; with `null` it does nothing more (the window is already focused). Muted rows show the existing bell-slash. The dialog opens from the palette and from the Agents dialog's old sound row, which now reads "Sounds and banners: Notifications…".
- [ ] **Step 4:** `pnpm test`, `pnpm build`. Expected: pass.
- [ ] **Step 5:** Commit: `feat(notify): add the Notifications dialog, per-session mute and the view report`.

### Task 8: Docs

**Files:**
- Modify: `docs/BRIEF.md` (Amendments: §5.5 sounds are Windows system sounds per category, no per-category volume; phase 4 exit on Windows), `docs/PLAN.md` (event model ~L543: `NotifyRequest` goes core → shell through `CoreEvent::Notify`, not to the webview; the sound brief marked superseded by phase 4 decision 2), `THIRD_PARTY_NOTICES.md` (the banner crate)

- [ ] **Step 1:** Make the edits; commit: `docs(notify): record where notifications are decided and how they sound`.

### Task 9: Live check

**Files:**
- Create: `docs/research/2026-10-21-notifications-live-check.md`

- [ ] **Step 1:** Run per CLAUDE.md "Live check (Windows)", scratch `MARSHELL_HOME`, on an installed build for the banner click (Task 1 says why). Sessions are fake agents or Claude sessions started for the check.
- [ ] **Step 2:** Check and record:
  - the banner and sound of needs-you, done and subagent done (Error has no producer yet) with the window in the background;
  - nothing while focused;
  - three needs-you within 3 s → one banner, then one batch banner;
  - the escalation at ~60 s;
  - Done only after 30 s in the background, and no Done flash on a resumed turn;
  - quiet hours set around the current time;
  - Windows do-not-disturb on;
  - a muted session;
  - a click on a banner selects the session (an entry in the notification centre does nothing, a known limit from spike S3).
  The user listens to the five sounds and picks replacements for any that sound wrong; that goes into §9 as a 4a decision.
- [ ] **Step 3:** Stop what was started by PID, remove the scratch folder, commit the research note.
