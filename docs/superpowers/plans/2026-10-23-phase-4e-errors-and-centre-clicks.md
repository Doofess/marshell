# Errors from Claude and clicks in the notification centre (slice 4e) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Who implements:** Tasks 1 (after the fixture) to 4 go to Codex CLI (`codex exec`), one worktree each under `C:/dev/codex-wt/`, branched from `phase-4e`. The main thread reviews each diff, runs the tests and cherry-picks. If Codex's quota runs out, a `worker` finishes the task, with `effort: high` for Tasks 2 and 3. Task 5 and the fixture capture stay in the main thread.

**Goal:** Claude's `StopFailure` hook turns a row Error, a banner clicked in the Windows notification centre opens its session when the person has turned the link on, and the phase 4 exit check runs on an installed build.

**Architecture:**
- The Claude adapter installs a tenth status hook, `StopFailure`, and folds it into `LiveState::Error` with a phrase from a fixed table. The 4a engine already turns Error into a banner, sound and flash.
- On Windows the shell builds toasts itself through the `windows` crate. Every toast carries one link, `marshell://focus/<tab>` (or `marshell://show`), in its `launch` attribute and its button's `arguments`. The toast uses protocol activation when the link is registered for this exe, and foreground activation otherwise.
- One parser, `link::parse`, reads the link from both click paths: an in-process `Activated` event, and a second launch that the single-instance plugin forwards. Both call `link::open`, which ignores the same link arriving again within 2 s, in case Windows reports one click both ways. The second launch hands its foreground right to the running Marshell (`AllowSetForegroundWindow`) before it forwards.
- A switch in the Notifications dialog registers the link under `HKCU\Software\Classes\marshell` through two shell commands. Its state is read from the registry, never stored. An NSIS uninstall hook removes a registration that points at the uninstalled exe.

**Tech Stack:** Rust (Tauri 2, `windows` 0.62, `windows-sys` 0.61, serde), React + TypeScript, Vitest, NSIS.

**Spec:** `docs/PLAN.md` §9, "Slice 4e decisions (2026-10-23)", 1 to 7. Also brief §5.5, `docs/spikes/S3-notification-click.md` finding 3, `docs/adr/0001-threat-model.md` ("The `marshell://` link"), and Claude's hook docs for `StopFailure` (fields `error`, optional `error_details`, optional `last_assistant_message`).

## Global Constraints

- New direct dependency, already in the lockfile: `windows = { version = "0.62", features = ["Data_Xml_Dom", "Foundation", "UI_Notifications"] }` under `[target."cfg(windows)".dependencies]` in `src-tauri/Cargo.toml`. The need is decision 5. It is Microsoft's crate, MIT OR Apache-2.0, with no network access or telemetry. Nothing else is added.
- Security surface touched, as the user chose (decision 6): `src-tauri/tauri.conf.json` gains only `bundle.windows.nsis.installerHooks`. The CSP and capabilities don't change. The two new commands are app commands; no plugin permission is added.
- The link carries no authority. It brings the window forward and selects a tab, and nothing else: no input, no answers, no settings. It accepts exactly two shapes, and the tab id must match `^[A-Za-z0-9_-]{1,64}$`. A cold start launched with a link starts normally and ignores the link.
- The registry is touched only by the switch and the uninstaller, never at startup. Turning the switch off deletes the key only when its command points at the running exe.
- Error copy is exact (decision 7), keyed by `error`:
  - `rate_limit` "Rate limit reached"
  - `overloaded` "Claude is overloaded"
  - `authentication_failed` "Sign-in failed"
  - `oauth_org_not_allowed` "Organization not allowed"
  - `account_on_hold` "Account on hold"
  - `billing_error` "Billing problem"
  - `invalid_request` "Invalid request"
  - `model_not_found` "Model not found"
  - `server_error` "Server error"
  - `max_output_tokens` "Reply hit the output limit"
  - `cloud_credential_error` "Cloud sign-in failed"
  - anything else: `last_assistant_message` through `first_line(_, 120)`, or "API error" when it is missing or empty.
- Switch copy is exact:
  - heading "Notification centre";
  - label "Open sessions from the notification centre";
  - reason "Adds a marshell:// link for your Windows account. It can only bring Marshell forward and select a session."
- Run `cargo fmt --all` before each Rust commit. Commit messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Windows 11 is the tested platform. Everything in Tasks 2 and 3 is `cfg(windows)`. On macOS and Linux, banners stay on the plugin, and `notification_link` returns `None`.

## Review Focus

1. **A hostile or malformed link.** For example `marshell://focus/../x`, `marshell://focus/t1?x=1`, `marshell://focus/t1 extra`, a 10 KB argument, `MARSHELL://FOCUS/t1`, a trailing `/`, and a link that is not the first argument. Only the two exact shapes parse. The scheme and the word `focus` or `show` are read in any case, the tab id exactly as written, and one trailing `/` is accepted. Every other second launch only shows the window, as it does today. Task 2 tests each case.
2. **A stale tab.** A notification-centre entry for a session that has since closed shows the window and leaves the selection alone. Task 4 adds `focusTarget` with a test, and the exit check clicks one.
3. **Error, then `idle_prompt`.** Claude sends `idle_prompt` about a minute after a turn ends. A row that is Error must stay Error, not turn Idle. Task 1 tests it.
4. **An install made before `StopFailure`.** Settings with the nine old events read Partial, and the repair adds only `StopFailure`. Task 1 tests it, like `an_install_without_subagent_start_is_partial`.
5. **A link registered by another Marshell.** Examples are a dev build, an older install path, or a key someone else wrote. The switch reads off. Turning it off leaves the key, and turning it on points it here. The uninstaller deletes the key only when it points into `$INSTDIR`. Task 2 tests the pure comparison, and the exit check runs the uninstall.

---

### Task 1: `StopFailure` turns a Claude row Error

**Files:**
- Create: `crates/core/tests/fixtures/claude/2.1.309/StopFailure.json` (captured live)
- Modify: `crates/core/src/agents/claude/hooks.rs` (`STATUS_EVENTS` ~19, tests ~767-800)
- Modify: `crates/core/src/agents/claude/status.rs` (`fold_event` ~165-215, tests)
- Modify: `crates/core/src/history/recorder.rs:112` (`ends_turn`)
- Modify: `src/features/onboarding/consent.ts:15` (`entries: 9` → `10`) and `consent.test.ts` (the counts follow)
- Test: `crates/core/tests/hook.rs` or `crates/core/tests/notify.rs`, whichever already posts hook bodies and reads `NotifyRequest`s

**Interfaces:**
- Produces: `STATUS_EVENTS: [&str; 10]`, with `"StopFailure"` right after `"Stop"`. `fn error_phrase(body: &Value) -> String` in `status.rs`, private.

- [ ] **Step 1: Capture the fixture (main thread, live).**
  - Create a fresh literal scratch folder under the session scratchpad. Use it as `CLAUDE_CONFIG_DIR`, never the real `~/.claude`.
  - In it, a `settings.json` whose only hook is `StopFailure`, running `cat > <scratch>/stopfailure.json`.
  - Run `claude -p "hi"` with `ANTHROPIC_API_KEY=sk-ant-invalid` and that `CLAUDE_CONFIG_DIR`, from a second scratch folder as the cwd so no project settings apply.
  - Expected: `error` is `authentication_failed`.
  - Save the body as the fixture, with `session_id`, `transcript_path` and `cwd` replaced the way `2.1.302/Stop.json` has them.
  - If the run gives a different `error`, keep what it gave and say so in the commit. If no `StopFailure` fires (for example, the key is refused before a turn starts), use the documented example body (`rate_limit`, "429 Too Many Requests", "API Error: Rate limit reached") and say so in the commit and the exit-check record.
  - Ask before removing the scratch folder.
- [ ] **Step 2: Write the failing tests.**
  - `status.rs`:
    - `stop_failure_is_error_with_our_line`: the fixture gives `LiveState::Error`, the phrase "Sign-in failed" (or the table's line for the captured `error`), and `recap` unchanged.
    - `stop_failure_with_unknown_type_uses_claudes_text`: `{"hook_event_name":"StopFailure","error":"new_thing","last_assistant_message":"API Error: Something new\nmore"}` gives "API Error: Something new".
    - `stop_failure_with_no_text_says_api_error`: `{"hook_event_name":"StopFailure","error":"unknown"}` gives "API error".
    - `idle_prompt_keeps_error`: Error, then `Notification` with `idle_prompt`, stays Error with the same phrase and `since_ms`.
  - `hooks.rs`:
    - `an_install_without_stop_failure_is_partial`: the pattern of `an_install_without_subagent_start_is_partial` with `"StopFailure"` filtered out, so 9 events. The repair leaves one group per event, and `StopFailure` is the new one.
    - The existing count asserts follow `STATUS_EVENTS.len()`.
  - Integration: `stop_failure_raises_the_error_banner`. Post the fixture to `/v1/hook` for a live tab while the window is unfocused. Expect one `NotifyRequest` with `sound == Some(SoundKind::Error)`, `banner.title == "<name> stopped"` and `banner.body == "Stopped: Sign-in failed."`. A Done hold isn't involved: `is_stop` is false for `StopFailure`.
  - `recorder.rs`: `stop_failure_ends_a_turn`. The recorder returns `Some(meta)` for a `StopFailure` body, as it does for `Stop`.
- [ ] **Step 3: Run them and see them fail.** `cargo test -p marshell-core stop_failure` and `cargo test -p marshell-core install_without_stop_failure`. Expected: FAIL. The arm is missing, and the array has 9.
- [ ] **Step 4: Implement.**
  - Add `"StopFailure"` to `STATUS_EVENTS`.
  - Add a `"StopFailure"` arm in `fold_event` returning `(LiveState::Error, error_phrase(body))`. It doesn't touch `recap`.
  - In the `idle_prompt` arm, keep `LiveState::Error` alongside Done, Idle and Ended.
  - Add `"StopFailure"` to `ends_turn`.
  - `fold_subagents` and `stuck_signal` stay as they are. A failed turn leaves the running set alone. Error isn't Working, so `clear_stuck_outside_working` already clears stuck.
- [ ] **Step 5: Run the tests.** Run `cargo test -p marshell-core`, `cargo test -p marshell-core --test agents_api` and `pnpm test src/features/onboarding`. Expected: all pass.
- [ ] **Step 6: Commit.** `feat(claude): turn a turn that ends on an API error into an Error row through the StopFailure hook`

### Task 2: The link: parser, registration, second launch and uninstall (worker, `effort: high`)

**Files:**
- Create: `src-tauri/src/link.rs`
- Create: `src-tauri/windows/hooks.nsh`
- Modify: `src-tauri/src/main.rs` (`mod link;`, the single-instance callback ~191, `generate_handler!` ~284)
- Modify: `src-tauri/tauri.conf.json` (`bundle.windows.nsis.installerHooks: "windows/hooks.nsh"`)

**Interfaces:**
- Produces:
  - `pub enum Link { Show, Focus(String) }` with `Debug, Clone, PartialEq, Eq`
  - `pub fn parse(s: &str) -> Option<Link>`
  - `pub fn link_for(tab_id: Option<&str>) -> String`: `marshell://focus/<tab>`, or `marshell://show` when there is no tab or the id fails the pattern
  - `pub fn from_argv(argv: &[String]) -> Option<Link>`: only `argv[1]`, and only when `argv.len() == 2`
  - `pub struct Recent` (the last link opened and when), with `pub fn admit(&mut self, link: &Link, now_ms: u64) -> bool`: false for the same link within 2 000 ms of the last admitted one
  - `pub fn open(app: &tauri::AppHandle, link: Option<Link>)`: `tray::show_main(app)`, then, for `Focus` that `Recent` (an `app.manage`d `Mutex<Recent>`) admits, emit `focus-session {tab_id}`
  - `#[cfg(windows)] pub fn hand_over_foreground()`: `AllowSetForegroundWindow(ASFW_ANY)`. This needs the `Win32_UI_WindowsAndMessaging` feature added to the existing `windows-sys` line.
  - `#[cfg(windows)] pub fn registered_here() -> bool`
  - `pub fn command_for(exe: &Path) -> String`: `"\"<exe>\" \"%1\""`
  - `pub fn points_at(command: &str, exe: &Path) -> bool`: ASCII case-insensitive equality with `command_for(exe)`
  - Commands: `#[tauri::command] fn notification_link() -> Option<bool>` (`None` off Windows), and `#[tauri::command] fn set_notification_link(on: bool) -> Result<bool, String>`, which returns `registered_here()` after the change
- Consumes: `tray::show_main`.

- [ ] **Step 1: Write the failing tests (`link.rs`, `#[cfg(test)]`).**
  - `parses_the_two_shapes`: `marshell://show` → `Show`; `marshell://focus/t3f9a0c41b2de` → `Focus("t3f9a0c41b2de")`; `MARSHELL://focus/t1/` → `Focus("t1")`.
  - `refuses_everything_else`: each gives `None`.
    - `marshell://focus/`
    - `marshell://focus/../x`
    - `marshell://focus/t1?x=1`
    - `marshell://focus/t1 extra`
    - `marshell://focus/t1//`
    - `marshell://answer/t1`
    - `marshell:focus/t1`
    - `https://focus/t1`
    - `"marshell://focus/" + 65 × "a"`
    - a 10 000-byte string
    - `""`
  - `link_for_round_trips`: `parse(&link_for(Some("t1"))) == Some(Focus("t1"))`, `link_for(None) == "marshell://show"`, and `link_for(Some("bad id")) == "marshell://show"`.
  - `recent_admits_a_repeat_only_after_2_s`: `Focus(t1)` at 0 is admitted, `Focus(t1)` at 1 500 is not, `Focus(t2)` at 1 600 is, and `Focus(t1)` at 4 000 is.
  - `argv_takes_only_a_lone_link`: `["m.exe", "marshell://focus/t1"]` gives `Focus`. `["m.exe"]`, `["m.exe", "--flag", "marshell://focus/t1"]` and `["m.exe", "marshell://focus/t1", "x"]` give `None`.
  - `points_at_compares_the_whole_command`: true for `command_for(exe)` in another case. False for another exe, for the exe without `"%1"`, and for `""`.
- [ ] **Step 2: Run them and see them fail.** `cargo test -p marshell-app link`. Expected: FAIL, because the module is missing.
- [ ] **Step 3: Implement `link.rs`.**
  - Registry through `windows-sys` `Win32_System_Registry`, which is already enabled.
  - Turning on writes three values:
    - `HKCU\Software\Classes\marshell` default `URL:Marshell`;
    - its `URL Protocol` as `""`;
    - `shell\open\command` default `command_for(current_exe)`.
  - Turning off runs `RegDeleteTreeW` on `Software\Classes\marshell`, only when `registered_here()`. On Windows it then clears Marshell's banners from the notification centre (`ToastNotificationManager::History()?.ClearWithId(aumid)`, using `toast::aumid` from Task 3, so this line lands with Task 3). Their protocol links would otherwise open Windows' "find an app" prompt.
  - `registered_here()` reads the command, applies `points_at`, and requires the `URL Protocol` value.
- [ ] **Step 4: Wire it in `main.rs`.**
  - The single-instance callback becomes `link::open(app, link::from_argv(&argv))`. A second launch without a link still shows the window.
  - At the top of `main`, before the plugins, call `link::hand_over_foreground()` when `link::from_argv(&std::env::args().collect::<Vec<_>>())` is `Some`.
  - Register both commands.
  - Add `src-tauri/windows/hooks.nsh`, with an `NSIS_HOOK_POSTUNINSTALL` macro that:
    - reads `HKCU\Software\Classes\marshell\shell\open\command`;
    - `StrCmp`s it with `"$INSTDIR\${MAINBINARYNAME}.exe" "%1"`;
    - runs `DeleteRegKey HKCU "Software\Classes\marshell"` only on a match.
- [ ] **Step 5: Check it.**
  - `cargo test -p marshell-app` passes.
  - `cargo clippy --workspace --all-targets -- -D warnings` is clean.
  - `pnpm tauri build` succeeds. Then read `target/release/nsis/x64/installer.nsi` (or wherever the build log says the generated script is). Confirm the hook's `StrCmp` and `DeleteRegKey` are in the uninstall section, and quote them in the report.
- [ ] **Step 6: Commit.** `feat(shell): add the marshell:// link, its switch commands and its removal on uninstall`

### Task 3: Windows toasts built by Marshell (worker, `effort: high`)

**Files:**
- Create: `src-tauri/src/toast.rs` (`#![cfg(windows)]`)
- Modify: `src-tauri/src/notify.rs` (`init` and `show` split by `cfg`)
- Modify: `src-tauri/Cargo.toml` (the `windows` line from Global Constraints)
- Modify: `THIRD_PARTY_NOTICES.md`: add `windows` 0.62.2 (MIT OR Apache-2.0, used under MIT) beside the notification crates. This is an owner file; the PR names the change for the user's approval.

**Interfaces:**
- Consumes: `link::{parse, link_for, open, registered_here}`; `notify::body_of`; `marshell_core::notify::{Banner, BannerAction}`.
- Produces:
  - `pub fn toast_xml(banner: &Banner, link: &str, protocol: bool) -> windows::core::Result<XmlDocument>`
  - `pub fn aumid(exe_dir: &Path, identifier: &str) -> String`: the PowerShell AUMID `{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe` when `exe_dir` ends in `target\debug` or `target\release`, else `identifier` (the plugin's rule)
  - `pub fn show(app: &tauri::AppHandle, banner: &Banner) -> windows::core::Result<()>`

- [ ] **Step 1: Write the failing tests (`toast.rs`).**
  - `the_toast_carries_the_link_everywhere`:
    - For `protocol = true`, `GetXml()` has `launch="marshell://focus/t3"` and `activationType="protocol"` on `<toast>`.
    - It has exactly one `<action>`, with `content="Open"`, `arguments="marshell://focus/t3"` and `activationType="protocol"`.
    - It has `<audio silent="true"/>`.
  - `foreground_when_the_link_is_not_registered`: with `protocol = false`, both `activationType`s are `foreground`.
  - `review_is_the_button_for_a_risky_request`: `BannerAction::Review` gives `content="Review"`.
  - `text_stays_text`:
    - The title `x</text><action activationType="protocol" arguments="https://example.com"/>` keeps one `<action>` element.
    - The title round-trips through the first `<text>`'s `InnerText`.
    - The body's two lines are `body_of(banner)`.
  - `aumid_follows_the_plugin`: `C:\r\target\debug` and `C:\r\target\release` give the PowerShell id, and `C:\Program Files\Marshell` gives `dev.marshell.app`.
- [ ] **Step 2: Run them and see them fail.** `cargo test -p marshell-app toast`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `toast_xml` builds the skeleton with no person's text in it. Then it sets the two `<text>`s through `SetInnerText`, and `launch`, `arguments` and `activationType` through `SetAttribute`.
  - `show` takes the following steps:
    - creates a notifier with `ToastNotificationManager::CreateToastNotifierWithId(aumid(..))`;
    - adds an `Activated` handler that casts the args to `ToastActivatedEventArgs`, runs `link::parse(Arguments())` and calls `link::open`;
    - keeps the last 32 `ToastNotification`s in an `app.manage`d `Mutex<VecDeque<_>>` so their handlers stay alive;
    - shows the toast.
  - `protocol` is `link::registered_here()`, read once per banner.
  - In `notify.rs`, `body_of` becomes `pub(crate)`, and Windows calls `toast::show`. `init`'s plugin action types and `on_action` become `#[cfg(not(windows))]`.
  - Leave the plugin in `main.rs` for the other platforms.
- [ ] **Step 4: Check it.** Run `cargo test -p marshell-app` and `cargo clippy --workspace --all-targets -- -D warnings`, then `cargo fmt --all`.
- [ ] **Step 5: Commit.** `feat(notify): build Windows toasts with the session's link so a click anywhere can reach it`

### Task 4: The switch and stale tabs (frontend)

**Files:**
- Create: `src/features/notifications/notificationLink.ts` and `notificationLink.test.ts`
- Modify: `src/features/notifications/NotificationsDialog.tsx` (view props and the wrapper, ~32-180) and `NotificationsDialog.test.tsx`
- Modify: `src/features/sessions/LiveApp.tsx` (the `focus-session` listener ~263-277)

**Interfaces:**
- Consumes: the commands `notification_link` and `set_notification_link` from Task 2.
- Produces:
  - `readNotificationLink(): Promise<boolean | null>`: `null` when not running in Tauri, when the invoke fails, or when the command returns `null`
  - `setNotificationLink(on: boolean): Promise<boolean>`
  - `focusTarget(tabId: string | null, tabIds: readonly string[]): string | null`
  - `NotificationsDialogView` gets the props `link?: boolean | null` and `onLink?: (on: boolean) => void`

- [ ] **Step 1: Write the failing tests.**
  - `focusTarget`: `("t1", ["t1","t2"])` gives `"t1"`. `("gone", ["t1"])`, `(null, ["t1"])` and `("t1", [])` give `null`.
  - Dialog: with `link: false`, the markup has the heading "Notification centre", the exact label and the exact reason. With `link: null` or omitted, none of them.
  - `readNotificationLink` resolves `null` when the invoke rejects. Mock `@tauri-apps/api/core` the way `attention.test.ts` (or the nearest existing invoke test) does.
- [ ] **Step 2: Run them and see them fail.** `pnpm test src/features/notifications src/features/sessions`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - The section goes after the terminal-alerts switch and before "Close button", and is shown only when `link` is a boolean.
  - A toggle calls `setNotificationLink`, shows the returned state, and on failure shows `failureText(e)` in the dialog's existing error line.
  - The `focus-session` listener runs `focusTarget(payload.tab_id, context.current.infos.map(i => i.tab_id))` and returns early on `null`, so a stale tab leaves the selection alone.
- [ ] **Step 4: Check it.** Run `pnpm test`, then `pnpm build`.
- [ ] **Step 5: Commit.** `feat(notifications): add the notification centre switch and ignore clicks for closed sessions`

### Task 5: Records and the phase 4 exit check (main thread)

**Files:**
- Modify: `docs/adr/0001-threat-model.md` ("The `marshell://` link"): where it is registered, what the two shapes are, the switch, and the uninstall rule.
- Create: `docs/research/2026-10-2x-phase-4-exit-check.md` (the date of the run)
- Modify: `docs/PLAN.md` §9 phase 4: the exit line points at the record.

- [ ] **Step 1: Update the threat model and run the full CI set.** `cargo fmt --all --check`, clippy, `cargo test --workspace`, `pnpm test`, `pnpm check:generated`, `pnpm build`. Quote the counts.
- [ ] **Step 2: Install the NSIS build of the `phase-4e` head per user, with a scratch `MARSHELL_HOME` and CDP on 9333.**
  - Before anything else, export `HKCU\Software\Classes\marshell` if it exists (`reg export`) to the scratch folder, and restore it at the end.
  - Sessions are plain `cmd.exe` ones started for the check.
  - Agent states are hook bodies posted to `/v1/hook` with the scratch token, as in `docs/research/2026-10-21-notifications-live-check.md`.
  - Error uses Task 1's fixture.
  - Guard every synthesized click by the target's process and rect.
- [ ] **Step 3: Verify each brief §5.5 rule and write one table row each.** The rules:
  - four categories with Done and subagent done distinct;
  - silent banners and Marshell's own sounds;
  - the per-category switches;
  - no notification when focused on the tab;
  - batching;
  - the subagent sound and its switch;
  - the one-minute reminder;
  - quiet hours, and do-not-disturb (the known limit);
  - per-session mute;
  - a banner click focusing the session;
  - the badge and the taskbar flash;
  - BEL, OSC 9 and OSC 777;
  - the doctor rule, recorded as moved to phase 7;
  - every sound having a visual equivalent.

  The new rows:
  - Error from `StopFailure` gives the banner "<name> stopped" / "Stopped: Sign-in failed.", the sound, the flash and the row.
  - Notification centre, link off: nothing happens.
  - Turn the switch on, and the key appears.
  - A centre click with the app running gives exactly one `focus-session`, the right selection, and `document.hasFocus()` true. Count the events over CDP and note the delay.
  - An on-screen body click and an Open click each select once.
  - A centre click for a closed session shows the window and keeps the selection.
  - Switch off: the key is gone.
  - Switch on, then a silent uninstall: the key is gone, and so are the shortcut and the folder.

  Not run: a centre click with Marshell quit. Windows would start Marshell outside the check's environment, against the real `~/.marshell`. The record lists it as not run.
- [ ] **Step 4: Stop what was started, by PID.** List the tree first. Ask before removing the scratch folders.
- [ ] **Step 5: Commit.** `docs(research): record the phase 4 exit check on an installed build`

After Task 5 comes the final whole-branch review on Opus. It is briefed against the threat model's "The `marshell://` link" section: parsing, and what a hostile link could do through a focus change, such as stealing focus into a terminal while the person types. It ends with a "Reviewed for 4e" entry in the ADR, as 3d had.
