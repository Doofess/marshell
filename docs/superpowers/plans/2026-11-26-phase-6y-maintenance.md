# Slice 6y: faults from the open issues — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix nine faults a person meets (issues #89, #90, #91, #92, #93, #100, #150, #158 and the ended session's keyboard from #208), close #88 as already fixed, and tag `v0.1.0-alpha.2` after the merge.

**Architecture:** Each fault is its own task with its own tests. The Rust tasks stay in `crates/core`: the pty's environment, the settings view's secret rule, the project list with the config guard, the profile detection, and one new undo route for a snapshot. The frontend tasks are small changes in `LiveApp.tsx`, `LiveSettings.tsx`, the appearance settings, the Company page, the resume bar and Settings → Agents. The tasks run in their numbered order: 5, 6 and 7 all edit `LiveApp.tsx` (5 and 7 the same `<LiveSettings>` element), and 7 calls the handler 5 adds.

**Tech Stack:** Rust (axum, tokio, portable-pty), React 19 + TypeScript, Vitest in node with no DOM (pure functions, `renderToStaticMarkup`, source-pin tests).

**Spec:** `docs/PLAN.md` §9, "Slice 6y decisions (2026-11-26)". The issues' own text is the report of each fault; where an issue and the decisions differ, the decisions win.

**Reviewed:** by an Opus `Plan` agent against the code on 2026-11-26, before execution; its three Critical and seven Important findings are folded in (the guard's own checks would have started the distro; the agents section is mounted by `LiveSettings`, not `LiveApp`; the snapshot id is `out.result.snapshot_id`).

## Global Constraints

- Branch `phase-6y`, worktree `C:\dev\Airport\.claude\worktrees\phase-6y`. Commit messages read `type(scope): what changed, as a sentence.` and end with the two attribution lines of this session.
- Rust: `cargo test -p marshell-core -j 4 <filter>` (never more jobs), `cargo fmt --all` before every Rust commit, `cargo clippy --workspace --all-targets -j 4 -- -D warnings` clean.
- Frontend: `pnpm test <path>`, `pnpm build`. Tests run in node with no DOM. `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4` regenerates it and the result is committed (`pnpm check:generated`).
- Every CLI equally: nothing here is built for one agent CLI only. Hooks off Undo works for all five, on Windows and in a distro.
- Config writes are safety-critical: an undo writes only when every file is still as the change left it, through the config service (preview, backup, atomic write).
- A new local API route gets a test that it is refused without the bearer (`.claude/rules/local-api.md`).
- UI: keyboard-only use and forced colours work; the `Announcer` is the only live region; exact copy as written in this plan; tokens only; read the good-css skill before writing a CSS declaration.
- No test touches a real `\\wsl.localhost` or `\\wsl$` path on the disk (no `exists`, `canonicalize`, `check`): on a developer's PC that starts a distro. Such paths are only ever compared as text in tests.
- Workers: no `git checkout`, `git restore`, `git stash`, `git reset` or `git clean`; no file outside the task's list without saying so in the report; scratch files go in `C:\Users\Home Office\AppData\Local\Temp\6y-plan\<task>` only; GitHub only through `gh`.
- No new dependency.

## Review Focus

1. A project path spelt `\\wsl$\ubuntu\…` (old host name, another case, forward slashes, or the verbatim `\\?\UNC\wsl.localhost\…`) while `wsl -l --running` says `Ubuntu`: it is the same distro. Pinned in Task 3 (`share_distro`, `a_share_path_counts_as_running_whatever_its_spelling`, the guard's two-spellings test).
2. `wsl.exe` missing, failing or slow: every WSL project is kept unread, no other project changes, nothing errors, and checking a path outside the user profile touches no WSL folder. Pinned in Task 3 (`entries` with an empty running list; `an_unread_folder_is_never_resolved`).
3. Undo after the file changed, was deleted or was already undone: 409 with the paths and nothing written, the second time too. Pinned in Task 7.
4. Keys near the `auth` rule: `Authorization`, `authorisationHeader`, `oauth`, `AUTH`, `authToken` stay secret; `author`, `authors`, `includeCoAuthoredBy`, `authorName` are shown. Pinned in Task 2.
5. The Resume button takes the keyboard only for the selected, visible session with no launcher, sheet or review over it and no link-click hold, takes it when such an overlay lifts, and never from a terminal that exists (a paused or resuming session). Pinned in Task 6.

Not pinned, noted only: a drive letter mapped to a WSL share can't be told from a local drive by its text, so such a project is still read.

---

### Task 1: A session's environment leaves out Windows Terminal's marks (#100)

Inline.

**Files:**
- Modify: `crates/core/src/pty/session.rs` (`apply_env`, tests beside `env_spec`)
- Modify: `crates/core/src/outside.rs` (`outside_env`, its tests)

**Interfaces:**
- Produces: `pub const HOST_TERMINAL_MARKERS: &[&str] = &["WT_SESSION", "WT_PROFILE_ID"];` in `crates/core/src/pty/session.rs`.

Every pty start goes through `apply_env` (tabs, WSL tabs, the shell panel through `Session::spawn`, the elevated helper through `child_env`), and the outside console filters by `outside_env`'s removed names, so these two places cover every session.

- [ ] **Step 1: Write the failing tests.** The test process's own variables are never changed (the neighbouring test says so, `session.rs:1016`; unit tests run threaded). In `session.rs`'s tests, `a_session_never_inherits_windows_terminals_marks`: build `CommandBuilder::new("")`, give it both names with `cmd.env("WT_SESSION", "x")` and `cmd.env("WT_PROFILE_ID", "y")` plus `cmd.env("MARSHELL_6Y_KEEP", "z")`, call `apply_env(&mut cmd, &env_spec(vec![], vec![], None))`, and assert with `cmd.get_env` that the two are gone and the third stays. `a_profile_can_still_set_windows_terminals_marks`: a spec whose `env` holds `("WT_SESSION", "mine")` leaves `WT_SESSION=mine` (the spec's `env` is applied last). In `outside.rs`'s tests: `outside_env(&[])`'s removed names hold `WT_SESSION` and `WT_PROFILE_ID`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 windows_terminals_marks` and `cargo test -p marshell-core -j 4 outside_env`. Expected: the new assertions FAIL.
- [ ] **Step 3: Implement.** `apply_env` removes every name in `HOST_TERMINAL_MARKERS` before the spec's own `env_remove`; `outside_env` chains them into `removed`. Doc comments say why: a tab is not a Windows Terminal session.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 pty::session` and `cargo test -p marshell-core -j 4 outside`. Expected: PASS.
- [ ] **Step 5: Commit** `fix(pty): a session doesn't inherit Windows Terminal's WT_SESSION and WT_PROFILE_ID.`

### Task 2: `auth` inside "author" is not a secret (#91)

Inline.

**Files:**
- Modify: `crates/core/src/agents/settings_view.rs` (`is_secret_key`, `is_secret_key_cases`)

The rule: a key is secret when its lower-cased text holds `key`, `token`, `secret` or `password` (as today), or holds `auth` at a place where it is not the start of `author` — except that `authoriz` and `authoris` still count. So `author`, `authors`, `authored`, `authority` are shown; `auth`, `oauth`, `authToken`, `Authorization`, `authorisation` are secret. Every occurrence of `auth` in the key is tried: one that counts is enough.

- [ ] **Step 1: Extend `is_secret_key_cases`.** True list gains `AUTH`, `oauth`, `authToken`, `auth_token`, `authorisationHeader`, `x-authorization`, `authorAuth` (the second `auth` counts), `awsAuthRefresh`. False list gains `includeCoAuthoredBy`, `author`, `authors`, `authorName`, `certificateAuthority`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 is_secret_key_cases`. Expected: FAIL on `includeCoAuthoredBy`.
- [ ] **Step 3: Implement** the rule above in `is_secret_key`; update its doc comment to state it.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 settings_view`. Expected: PASS (the masking tests beside it too).
- [ ] **Step 5: Commit** `fix(settings): a key that only holds "author" is no longer masked as a secret.`

### Task 3: Listing projects never starts a WSL distro (#158, decision 1)

Worker, `effort: high` (the config guard).

**Files:**
- Modify: `crates/core/src/wsl.rs` (new `share_distro`, new `running_recent`, tests)
- Modify: `crates/core/src/projects.rs` (`list`, new `Listed` and `entries`, tests)
- Modify: `crates/core/src/config/guard.rs` (`Guard`'s new field, `for_home`, new `with_unread`, `check`, `wide_home_of`, tests)
- Modify: `crates/core/src/server/doctor.rs:333`, `crates/core/src/server/doctor_ext.rs:135` (their two `projects::list` calls)

Why the guard needs its own list: `Guard::check` and `wide_home_of` test every allowed folder with `projects::is_inside`, and `is_inside` runs `canonical` (`std::fs::canonicalize`) on the folder each time (`projects.rs:63-65`). A WSL folder in `allowed` would therefore start its distro on any check of a path outside the user profile. So an unread project never enters `allowed`.

**Interfaces:**
- Produces, in `wsl.rs`: `pub fn share_distro(path: &str) -> Option<&str>`: the distro of a path under `\\wsl.localhost\<distro>` or `\\wsl$\<distro>` (either separator, host compared without case, a leading `\\?\UNC\` accepted), `None` for any other path and for an empty distro name. `pub fn running_recent() -> Vec<String>`: `list_running` with an 8 s limit, remembered for 5 s in a process-wide `Mutex<Option<(Instant, Vec<String>)>>`, so one page load asks `wsl.exe` once. The lock is held only to read or store, never across the `wsl.exe` call.
- Produces, in `projects.rs`: `pub struct Listed { pub info: ProjectInfo, pub read: bool }` (the private `Entry` at `projects.rs:14` keeps its name) and `pub fn entries(home: &Path, running: &dyn Fn() -> Vec<String>) -> Vec<Listed>`. `running` is called at most once, and only when the file holds a path `share_distro` recognises. A project on a share whose distro is not in `running` (names compared without case) is kept with `read: false`, `branch: None`, its name from the path's last part; nothing touches the disk for it (`Path::exists`, `git_branch`, `canonical` are not called). Every other project is treated as today (`read: true`). `pub fn list(home)` keeps its signature and is `entries(home, &crate::wsl::running_recent)` mapped to `info`.
- Produces, in `guard.rs`: a field `unread: Vec<(String, String)>` (distro, the folder's Linux path), empty from `Guard::new`, and `pub fn with_unread(mut self, folders: &[PathBuf]) -> Self`, which for each folder takes `share_distro` of its text (after `unverbatim`) and `wsl::share_to_linux` of it, and keeps the pair unless the Linux path is `/` (a share root would open the whole distro). Nothing in it touches the disk. A private `fn in_unread(&self, path: &Path) -> bool`: true when, for some pair, `linux_in_share(distro, path)` (it already unverbatims and compares the host and distro without case) is the pair's Linux path or below it, the next character being `/` (so `/home/me2` is not in `/home/me`; the Linux path is compared with case, as `in_agent_folder` does).
  - `check`: allowed also when `self.in_unread(&resolved)`. The denied and Marshell's-own tests stay before it.
  - `wide_home_of`: its early `None` also when `self.in_unread(path)`, tested before the `allowed` loop.
  - `for_home`: uses `entries(home, &crate::wsl::running_recent)`; `read` ones go into `allowed` exactly as today (with the `is_root(&canonical(p))` filter), the others through `with_unread`.
- The Doctor's two callers use `entries(&st.0.home, &|| (st.0.running_distros)())` and take only `read` entries: the Doctor reads a project in a distro only while the distro runs, as it already does for a distro's own CLIs, and it keeps the `with_running_distros` test seam (`server/mod.rs:751`).

- [ ] **Step 1: Write the failing tests.**
  - `wsl.rs`: `share_distro_names_the_distro_of_a_share_path`: `\\wsl.localhost\Ubuntu\home\me` → `Ubuntu`; `//wsl$/ubuntu/home` → `ubuntu`; `\\WSL.LOCALHOST\Debian` → `Debian`; `\\?\UNC\wsl.localhost\Ubuntu\x` → `Ubuntu`; `\\wsl.localhost\My Distro\x` → `My Distro`; `C:\dev`, `\\server\share\x` and `\\wsl.localhost\` → `None`.
  - `projects.rs`: `a_project_in_a_stopped_distro_is_kept_and_not_read`: a projects file with a temp folder and `\\wsl.localhost\Ubuntu\home\me\app`; `entries(home, &|| vec![])` gives both, the share one with `read: false`, `branch: None`, `name: "app"`. `a_share_path_counts_as_running_whatever_its_spelling`: `entries` takes the decision from a private `fn unread(path: &str, running: &[String]) -> bool`, and the test calls that (not the disk) for `\\wsl$\ubuntu\x`, `//WSL.LOCALHOST/UBUNTU/x` and `\\?\UNC\wsl.localhost\Ubuntu\x`: all `false` with `["Ubuntu"]`, all `true` with `[]` and with `["Debian"]`; `C:\dev\x` is `false` with any list. `the_running_list_is_asked_once_and_only_for_a_share_path`: a counter closure is called 0 times for a file with no share path and once for a file with three share paths (all in a distro the closure doesn't name, so nothing is read).
  - `guard.rs`: `an_unread_folder_opens_what_is_in_it_by_either_spelling`: a guard with `with_unread(&[r"\\wsl$\ubuntu\home\me\app".into()])`; `in_unread` is true for `\\wsl.localhost\Ubuntu\home\me\app\.claude\settings.json`, `\\wsl$\UBUNTU\home\me\app` and `\\?\UNC\wsl.localhost\Ubuntu\home\me\app\x`, false for `\\wsl.localhost\Ubuntu\home\me\app2\x`, `\\wsl.localhost\Ubuntu\home\me`, `\\wsl.localhost\Debian\home\me\app\x` and `C:\home\me\app`. `a_share_root_is_never_unread_allowed`: `with_unread(&[r"\\wsl.localhost\Ubuntu".into(), r"\\wsl.localhost\Ubuntu\".into()])` leaves `unread` empty. `an_unread_folder_is_never_resolved`: a guard built with one unread folder and one ordinary temp folder; `check` of a file in the temp folder passes and `guard.allowed` holds no path whose text `share_distro` recognises (the only way a check could reach the share). `wide_home_of` returns `None` for a path in the unread folder even with a wide home recorded that holds it by text.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 share_distro`, `… stopped_distro`, `… running_list_is_asked`, `… unread`. Expected: FAIL (the names do not exist).
- [ ] **Step 3: Implement** as the Interfaces block says.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 projects`, `cargo test -p marshell-core -j 4 config::guard`, `cargo test -p marshell-core -j 4 wsl`, `cargo test -p marshell-core -j 4 doctor`, `cargo test -p marshell-core -j 4 --test tasks`. Expected: PASS.
- [ ] **Step 5: Commit** `fix(projects): a project in a WSL distro that isn't running is listed unread, so listing projects never starts a distro.`

### Task 4: A slow WSL at start is asked again (#92)

Worker, `effort: high` (shared state, a background task).

**Files:**
- Modify: `crates/core/src/profiles/detect.rs` (new `second_look`, new `Probe::patient`, tests)
- Modify: `crates/core/src/server/mod.rs` (`detected` becomes replaceable, new `AppState::adopt_detected`, the background task at start)
- Modify: `crates/core/src/server/profiles.rs` (reads of `detected` at lines 18, 37-38)

**Interfaces:**
- Produces, in `detect.rs`: `pub fn second_look(first: &[Profile], list: &dyn Fn() -> Vec<String>, detect: &dyn Fn() -> Vec<Profile>) -> Option<Vec<Profile>>`: `None` when `first` already holds a profile of `ShellFamily::Wsl`, or when `list()` is empty; else `Some(detect())` when that result holds a WSL profile, `None` otherwise. `list` is not called when `first` has a WSL profile. `impl Probe { pub fn patient() -> Self }`: `Probe { wsl_list: patient_wsl_list, ..Probe::system() }`, where `fn patient_wsl_list() -> Vec<u8>` is `run_wsl(&["-l", "-q"], Duration::from_secs(30))` (the field is a `fn` pointer, so it is a named function).
- Produces, in `server/mod.rs`: `detected: RwLock<Vec<Profile>>` in the state, and `pub(crate) fn adopt_detected(&self, detected: Vec<Profile>)`: reads the new document with `crate::profiles::store::load(&home, detected.clone())` before taking any lock (it only reads the file), then stores the list, then replaces the profiles document. The two locks are taken one after the other, never nested. A `PUT /v1/profiles` that lands between the two is the accepted race: the document it saved wins until the next start.
- At start, after the state is built and only on Windows: one `tokio::task::spawn_blocking` that holds a `Weak` of the state's inner value, runs `second_look(&first, &|| crate::wsl::list_distros(Duration::from_secs(30)), &|| detect_profiles(&Probe::patient()))`, and on `Some` upgrades the `Weak` and calls `adopt_detected` (a core that stopped meanwhile is simply gone). It logs one `tracing::info!` line when it adds profiles. It never delays start. No separate check that `wsl.exe` exists: without it the list is empty at once, and nothing happens.

- [ ] **Step 1: Write the failing tests.** `detect.rs`: `second_look_is_skipped_when_wsl_was_found`, `second_look_gives_nothing_when_the_list_stays_empty`, `second_look_gives_nothing_when_the_new_detection_has_no_wsl`, `second_look_gives_the_new_detection_when_distros_answer` (fake closures; a counter proves `list` was not called in the first). In `server/profiles.rs`'s tests (they build an `AppState` with a temp home): `adopting_a_later_detection_adds_its_profiles_and_keeps_saved_edits`: a state built with one detected shell and a `profiles.json` that renames it; after `adopt_detected` with that shell plus a WSL profile, `GET /v1/profiles` holds both, the rename is kept, and the detected list holds both.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 second_look` and `… adopting_a_later_detection`. Expected: FAIL (the names do not exist).
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 profiles` and `cargo test -p marshell-core -j 4 server::`. Expected: PASS.
- [ ] **Step 5: Commit** `fix(profiles): a WSL list that was too slow at start is asked again in the background, and its profiles are added.`

### Task 5: Three small frontend faults (#150, #90, the rest of #89)

Worker.

**Files:**
- Create: `src/features/terminal/useAppScheme.ts`, `src/features/terminal/useAppScheme.test.ts`
- Modify: `src/features/appearance/AppearanceSections.tsx` (the `app:` read in `useAppearanceSection`, line ~422)
- Modify: `src/features/sessions/LiveApp.tsx` (new `settingsRefresh` state, the agents-changed handler at the `<LiveSettings>` element ~3114, `openAsSession` ~1616)
- Modify: `src/features/settings/LiveSettings.tsx` (passes `settingsRefresh` to the five Company sections)
- Modify: `src/features/companies/CompanySection.tsx` (the new prop and its effect), `src/features/companies/companyFlow.ts` (the pure rule)
- Modify: `src/features/window/ShellPanel.tsx` (the new line, beside `SHELL_FAILED`)
- Test: `src/features/sessions/LiveApp.test.tsx`, `src/features/companies/companyFlow.test.ts`, the test files beside `AppearanceSections.tsx`, `LiveSettings.tsx` and `CompanySection.tsx`

**Interfaces:**
- Produces: `export function schemeStore(root: SchemeRoot, media: SchemeMedia, watch: WatchTheme): { subscribe(cb: () => void): () => void; get(): Scheme }` and `export function useAppScheme(): Scheme` in `useAppScheme.ts`. `get()` is `appScheme(root)`. `subscribe` listens to the root's `data-theme` (through `watch`, a `MutationObserver` in the app) and to the media query's `change`, and its return value stops both. The three parameter types are the narrow shapes the store needs, so a test passes fakes. The hook is `useSyncExternalStore(store.subscribe, store.get, () => "dark")`: the third argument is required, since React throws in `renderToStaticMarkup` without a server snapshot. The store is built on first use and kept in a module variable (no `document` or `matchMedia` at module load or during a node render; `subscribe` and `get` keep their identity). The other observers on the root (`TerminalView.tsx:383`, `ScriptedTerminal.tsx:68`, `appearance/windowTheme.ts:45`) are not shared helpers and stay as they are.
- Produces: `export const OPEN_AS_SESSION_FAILED = "Couldn't open the shell as a session.";` in `ShellPanel.tsx`.
- Produces: `export const rereadsAfterHooksChange = (s: { draftKeys: number; rawDraft: string | null; busy: boolean }): boolean` in `companyFlow.ts`: true only with no pending form edit, no Raw draft and nothing in flight.
- Produces: `CompanySection` prop `settingsRefresh?: number`; `LiveSettings` prop `settingsRefresh: number`; `LiveApp` state `const [settingsRefresh, setSettingsRefresh] = useState(0)`.

What changes:
- `useAppearanceSection`: `app` is `appearance.theme === "follow-system" ? scheme : appearance.theme`, with `scheme = useAppScheme()` called unconditionally at the top of the hook. It is the only read of `appScheme()` during a render; the other call sites read it in handlers and effects and stay.
- A hooks change no longer reuses `companyRefresh`: bumping that also asks every mounted Company page for a fresh version probe (`getInstall(…, refresh > 0)`, `CompanySection.tsx:381`). Instead the handler given to `<LiveSettings onAgentsChanged=…>` calls `refreshSetup()` and `setSettingsRefresh((n) => n + 1)`. In `CompanySection`, an effect on `[settingsRefresh]` (skipping the first run) calls the existing `reread()` when `rereadsAfterHooksChange(…)` says so. With an edit in progress the page keeps its old view, so a save is still refused as changed, exactly as today: a draft written against the old file must never be planned against the new one without the person seeing it.
- `openAsSession`'s `catch`: `announce(OPEN_AS_SESSION_FAILED)`; and when the error is an `ApiError` with status 404, `setPanel(null)` so the ask effect gives the panel a new shell. The `console.warn` stays. `announce` joins the callback's dependency list.

- [ ] **Step 1: Write the failing tests.** `useAppScheme.test.ts`: `the store answers with the root's scheme`, `it tells its listener when the root's theme changes and when the system's scheme changes` (fakes that capture the callbacks), `unsubscribing stops both`, and `useAppScheme renders in node` (`renderToStaticMarkup` of a component that calls it does not throw). `companyFlow.test.ts`: `rereadsAfterHooksChange` true for `{ draftKeys: 0, rawDraft: null, busy: false }`, false for one draft key, for a Raw draft (also the empty string), and for busy. Source pins: in `LiveApp.test.tsx`, the handler passed as `onAgentsChanged` holds `refreshSetup()` and `setSettingsRefresh(` and no `setCompanyRefresh(`; `openAsSession`'s catch holds `announce(OPEN_AS_SESSION_FAILED)` and `setPanel(null)` behind a `404` test; in the settings test, each Company section gets `settingsRefresh={settingsRefresh}`; in the Company test, the effect is keyed by `[settingsRefresh]` and calls `reread()` behind `rereadsAfterHooksChange(`. The appearance test pins `useAppScheme()` in `useAppearanceSection` and that `appScheme()` is no longer called in that file.
- [ ] **Step 2: Run** `pnpm test src/features/terminal/useAppScheme.test.ts src/features/sessions/LiveApp.test.tsx src/features/appearance src/features/companies src/features/settings`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test src/features/terminal src/features/sessions src/features/appearance src/features/window src/features/companies src/features/settings` and `pnpm build`. Expected: PASS, build ok.
- [ ] **Step 5: Commit** `fix(app): the settings preview follows the scheme as it changes, a hooks change refreshes the Company pages, and a failed "Open as a session" says so.`

### Task 6: An ended session's view puts the keyboard on Resume (decision 3)

Worker.

**Files:**
- Modify: `src/features/sessions/ResumeBar.tsx`
- Modify: `src/features/sessions/LiveApp.tsx` (`renderTerminal` ~2456-2500, `SessionResumeBar`)
- Test: `src/features/sessions/ResumeBar.test.tsx`, `src/features/sessions/LiveApp.test.tsx`

**Interfaces:**
- Consumes: `terminalFocusProps({ selected, linkHeld, overlayUp }): { autoFocus, focusBlocked }`, `focusOnChange({ selected, blocked }): boolean` and `focusOnUnblock` from `src/features/terminal/terminalFocus.ts`, as `LiveApp.tsx` (line ~2474) and `TerminalView.tsx` (lines ~456-468) use them.
- Produces: `ResumeBar` prop `keyboard?: { selected: boolean; blocked: boolean; visible: boolean; token: number }`. Two effects, mirroring `TerminalView`'s pair:
  - on `[keyboard?.selected, keyboard?.visible, keyboard?.token]`: focus the Resume button when `resumeTakesKeyboard(keyboard)`;
  - on `[keyboard?.blocked]`: when it went from blocked to not blocked (a `wasBlocked` ref, as `TerminalView.tsx:459-468`), focus the button when `keyboard.visible` and `focusOnUnblock` (called with the arguments `TerminalView` gives it) say so.
  `blocked` is read at effect time through a ref (not `useEffectEvent`: some suites mock React's hooks without it). Without the prop the bar never takes the keyboard.
- Produces: `export function resumeTakesKeyboard(k: { selected: boolean; blocked: boolean; visible: boolean } | undefined): boolean` in `ResumeBar.tsx`: `k !== undefined && k.visible && focusOnChange({ selected: k.selected, blocked: k.blocked })`.
- `renderTerminal` computes the terminal's props once, `const focus = terminalFocusProps({ selected: row.id === focusedId, linkHeld, overlayUp: … })` with the `overlayUp` expression it already has, spreads them on `TerminalView` as today, and passes `keyboard={hasTerminal ? undefined : { selected: focus.autoFocus, blocked: focus.focusBlocked, visible: shown, token: focusToken }}` to `SessionResumeBar`, which hands it to `ResumeBar`. `selected` is `autoFocus`, so the link-click hold (`linkHeld`) keeps the keyboard off Resume as it keeps it off a terminal: Enter typed for another window must not start a process. A bar drawn beside a terminal (paused, resuming) passes nothing, so the terminal keeps the keyboard.
- The button stays the target also when it is `aria-disabled` (the bypass acknowledgement not ticked, or the profile gone): it is focusable, and its description says why.

- [ ] **Step 1: Write the failing tests.** `ResumeBar.test.tsx`: the `resumeTakesKeyboard` table: `undefined` → false; selected, visible, not blocked → true; not visible → false; blocked → false; not selected → false. Source pins: the first effect's dependency list is `[keyboard?.selected, keyboard?.visible, keyboard?.token]` and it focuses the button's ref behind `resumeTakesKeyboard(`; the second is keyed by `[keyboard?.blocked]` and holds `focusOnUnblock(`. `LiveApp.test.tsx`: `SessionResumeBar` gets `keyboard={hasTerminal ? undefined : {`, and the object holds `selected: focus.autoFocus`, `blocked: focus.focusBlocked`, `token: focusToken` and `visible: shown`.
- [ ] **Step 2: Run** `pnpm test src/features/sessions/ResumeBar.test.tsx src/features/sessions/LiveApp.test.tsx`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test src/features/sessions src/features/window src/features/terminal` and `pnpm build`. Expected: PASS, build ok.
- [ ] **Step 5: Commit** `fix(sessions): going to an ended session puts the keyboard on its Resume button.`

### Task 7: Hooks off gets Undo (#93, decision 2)

Worker, `effort: high` (config writes).

**Files:**
- Modify: `crates/core/src/server/agents.rs` (new `undo_snapshot`, beside `settings_undo_of`, `changes_of` and `changed_since`), `crates/core/src/server/config.rs` (new handler `undo`), `crates/core/src/server/mod.rs` (the route, beside `/v1/snapshots/{id}/restore`), `crates/core/src/server/doctor.rs` (`undo_of` calls the shared function)
- Test: `crates/core/tests/agents_api.rs` (beside `hooks_off_deletes_the_chain_and_undo_puts_back_both_files`, line 231)
- Modify: `src/lib/api/agents.ts` (+ its test), `src/features/agents/AgentsSection.tsx`, `src/features/agents/agentsState.ts` (+ their tests), `src/features/settings/LiveSettings.tsx` (passes the two props through; it is what mounts `<AgentsSection>`, line ~196), `src/features/sessions/LiveApp.tsx` (holds the offer), a CSS file only if no existing class fits

**Interfaces:**
- Produces, core: `pub(super) fn undo_snapshot(st: &AppState, info: &SnapshotInfo) -> Result<ApplyResult, Refusal>` in `agents.rs` (`Refusal` is `server::agents::Refusal`; the snapshot's type is what `st.0.config.snapshots().get` returns). It is the body of the Doctor's `undo_of` (`doctor.rs:1663`) from `record_distros_of` to the apply's result: every file that `differs_from_fix` → `Err(changed_since(paths))` and nothing is written; else one `ChangeSet` of `EditOp::Restore` edits with `expected_hash: file.after_hash`, previewed and applied; `ApplyError::Conflict` → `changed_since(paths)`, `UnknownChangeSet` → `changed_since(vec![])`, `Failed` → 500. `differs_from_fix` moves with it or is made `pub(super)`. The Doctor's `undo_of` keeps its record handling (`current_record`, `forget_record`) and calls this.
- Produces, core: `POST /v1/snapshots/{id}/undo` → handler `config::undo`: 200 with the `ApplyResult`; 404 for an unknown or invalid id; 409 with the body `changed_since` builds; 500 otherwise. The router-wide bearer guard covers it.
- Produces, frontend: `export async function undoSnapshot(ep: Endpoint, snapshotId: string): Promise<UndoOutcome>` in `src/lib/api/agents.ts`, with `import type { UndoOutcome } from "./doctor"` (a type import: `doctor.ts` already imports a value from `agents.ts`): 200 → `{ ok: true }`, 409 → `{ ok: false, changed }` (the paths, read as `undoFix` reads them), 404 → `{ ok: false, message: "There is nothing to undo." }`, any other `ApiError` → `{ ok: false, message }`.
- Produces, state: `export type HooksUndo = { agent: HookAgent; distro?: string; snapshotId: string; statuslineLeft: boolean; refused?: string }` in `agentsState.ts`. `LiveApp` holds `const [hooksUndo, setHooksUndo] = useState<HooksUndo | null>(null)` and gives `hooksUndo={hooksUndo}` and `onHooksUndo={setHooksUndo}` to `<LiveSettings>`, which passes both to `<AgentsSection>`, which passes `hooksUndo`, a busy flag and an `onUndo` handler into `AgentsSectionView` and down to `AgentList` and `WslGroup`. The offer outlives closing Settings; it does not outlive the app.
- Copy, in `agentsState.ts` (`where` is the CLI's name, or `<name> in <distro>` for a distro's offer):
  - `hooksUndoLine(where, statuslineLeft)`: `Hooks off for ${where}. Sessions started from now show limited status.` plus the status-line sentence of `hooksOffNotice` when `statuslineLeft`.
  - Button: `Undo`.
  - `hooksUndone(where)`: `Hooks are back for ${where}, as they were before Hooks off.` (announced; the block goes).
  - `hooksUndoRefused(paths)`: `Nothing was undone: ${files} changed since Hooks off, so undo would overwrite ${paths.length > 1 ? "them" : "it"}.` with `files` joined as the Doctor's line joins them and `a file` when `paths` is empty (`doctorUi.ts:17-20`). The block then shows this line and no button.
  - `hooksUndoFailed(message)`: `Couldn't undo Hooks off: ${message}` (announced; the button stays).

What the page does:
- After Hooks off succeeds (`turnOff`, `AgentsSection.tsx` ~501): as today (the notice is announced, the list shows, `load()`, `onChanged()`), and `onHooksUndo({ agent, distro, snapshotId: out.result.snapshot_id, statuslineLeft })`.
- The list shows one block for the offer, under the row of that CLI (in `AgentList`) or of that distro (in `WslGroup`): a container with `tabIndex={-1}` and `data-focus="hooks-undo"`, the line, and the `Undo` button. It reuses the Company page's result classes (`company__result`, `CompanySection.tsx:313`) if they read right there, else the agents list's own existing classes; a new declaration only if neither fits.
- Keyboard: `AgentsSection` gets an effect that, when the mode has just gone from `"off"` to `"list"` and an offer for the shown CLI exists, focuses the `[data-focus="hooks-undo"]` block inside the section's root. The block is inside the Settings page, so `LiveSettings`' stray-focus guard (`usePopupFocus`, which sends lost focus to the heading) has nothing to pull back; the live check confirms the block holds the focus (criterion 2). After Undo succeeds the block is gone and the guard puts the keyboard on the heading, as it does for any control that leaves.
- Undo: busy state on the button (`aria-disabled`, as `Btn` in the Company page); then by outcome as the copy says; on success `onHooksUndo(null)`, `load()`, `onChanged()` (which, after Task 5, re-reads the Company pages).
- The offer is withdrawn (`onHooksUndo(null)`) when a set-up or another Hooks off succeeds for the same CLI and distro.
- Every CLI that has Hooks off gets it, Windows and WSL alike: the code has no per-CLI branch.

- [ ] **Step 1: Write the failing tests.**
  - `agents_api.rs`: `hooks_off_can_be_undone_through_the_snapshot_while_the_files_are_unchanged`: set up, keep `GET /v1/agents/claude/setup`'s body and the two files' bytes, Hooks off, `POST /v1/snapshots/{id}/undo` → 200; the settings file and `statusline-chain.json` are byte-identical to before Hooks off, and the setup body equals the kept one (so the row reads as set up again). The same for Codex with the fixtures the neighbouring Codex tests use. `an_undo_after_the_file_changed_is_refused_and_writes_nothing`: edit the settings file after Hooks off → 409, the body names the path, the file is as edited, the chain file is still absent. `an_undo_of_an_unknown_snapshot_is_not_found` (404). `a_second_undo_is_refused_as_changed` (the first undo changed the files → 409). `snapshot_undo_is_refused_without_the_bearer` (the pattern at `agents_api.rs:2716`). The Doctor's existing undo tests must pass unchanged.
  - `src/lib/api/agents.test.ts`: `undoSnapshot` maps 200, 409, 404 and 500 as the Interfaces block says, and posts to `/v1/snapshots/<id>/undo` with the id encoded.
  - `agentsState.test.ts`: the four copy functions, exact strings: a Windows offer and a distro's (`Claude Code in Ubuntu`), with and without the status-line sentence, and `hooksUndoRefused` with no, one and two paths.
  - `AgentsSection.test.tsx` (`renderToStaticMarkup` of `AgentsSectionView`): with an offer for Codex the list holds the line and a button named `Undo` under Codex's row and under no other; with an offer for a distro it is under that distro's row; with `refused` set it holds that line and no `Undo`; with no offer, neither. A source pin for the focus effect (`[data-focus="hooks-undo"]`).
  - `LiveApp.test.tsx`: `<LiveSettings` gets `hooksUndo={hooksUndo}` and `onHooksUndo={setHooksUndo}`; the settings test: `<AgentsSection` gets both.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test agents_api undo` and `pnpm test src/lib/api/agents.test.ts src/features/agents src/features/settings src/features/sessions/LiveApp.test.tsx`. Expected: FAIL.
- [ ] **Step 3: Implement** the core route first, then the frontend.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 --test agents_api`, `cargo test -p marshell-core -j 4 doctor`, `pnpm test src/lib/api src/features/agents src/features/settings src/features/sessions src/features/doctor`, `pnpm check:generated`, `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(agents): Hooks off offers Undo, which puts the files back while they are unchanged.`

### Task 8: Live check

`marshell-live-check`, from this worktree, on a new scratch folder (`%TEMP%\lc-6y-1`), with scratch homes for Marshell and every CLI. Nothing is brought to the front. Findings go in `docs/research/2026-11-26-maintenance-6y-live-check.md`. Exit criteria:

1. An ended session: with two shell sessions, `stop` and `restart`; Ctrl+Shift+2 leaves `document.activeElement` on the "Resume" button of session 2's bar; with the launcher open the keyboard stays in the launcher, and closing it puts the keyboard on Resume; going to a live session puts it in the terminal.
2. Hooks off Undo, Claude Code (scratch `MARSHELL_CLAUDE_SETTINGS`): Set up, note the file's hash, Hooks off → `document.activeElement` is the Undo block; Undo → the file's hash is the one noted and the row reads as set up again; the announcer said "Hooks are back for Claude Code, as they were before Hooks off."
3. Undo refused: Set up, Hooks off, edit the scratch settings file by hand, Undo → the block reads "Nothing was undone: … changed since Hooks off, so undo would overwrite it.", no Undo button, the file is as edited.
4. The same Undo for Codex in its scratch `CODEX_HOME`. The note records whether a Codex started afterwards asks to trust the hooks again.
5. The offer outlives closing Settings: Hooks off, close Settings, open Settings → Agents → the Undo block is there.
6. Company page refresh: with Claude Code's Company page showing the scratch settings in Raw and nothing edited, Hooks off in Agents, back on the Company page → the text is the new file, and a Raw save is not refused as changed. With a Raw edit typed first, the page keeps its text and the save is refused as changed.
7. A scratch Claude settings file holding `"includeCoAuthoredBy": false`: the Company page shows the key and its value, and Raw is editable.
8. Follow system: Appearance pinned to the scheme the system doesn't have, Settings → Terminal open, switch to Follow system → the preview's colours are the system scheme's without another action.
9. WSL (the recipe in the lead's notes; the distro is the user's Ubuntu, and only `/tmp` in it is touched): `/tmp/lc-6y-proj` made beforehand with `git init -b lc-6y-branch`, a scratch `projects.json` naming `\\wsl.localhost\Ubuntu\tmp\lc-6y-proj`, and the distro stopped (`wsl -l --running` doesn't name Ubuntu after its idle stop). Reload the page three times, 20 s apart, and ask `GET /v1/agents/<cli>/setup` for all five → `wsl -l --running` still doesn't name Ubuntu, and the launcher lists `lc-6y-proj` with no branch. With the distro running (a shell held open in it by the check), the launcher shows the project with `lc-6y-branch`.
10. Not live: #100 (the live-check script itself clears `WT_SESSION`), #92 (a slow `wsl.exe` can't be forced), "Open as a session" failing (needs a core without the panel's shell), and the guard never resolving an unread folder on a check outside the profile (the scratch homes are under the profile). Their tests are the evidence; the note says so.

### Task 9: Records

Inline, after the live check. **Files:** `docs/PLAN.md` (a "Settled while it was built" list under "Slice 6y decisions" with the ledger's rulings and the live check's note), `docs/research/2026-11-26-maintenance-6y-live-check.md`. Issue #88 is closed with a comment naming `src/styles/app.css` and `src/styles/overlayStacking.test.ts`. The pull request's body closes #89, #90, #91, #92, #93, #100, #150 and #158; #208 gets a comment that its ended-session item is fixed. Deferred minors from the review go into one new issue. Commit: `docs(plan): slice 6y's rulings and its live check.`

### After the merge: alpha.2 (decision 4)

Not a task of the branch. Once the pull request is merged and CI is green on `main`, follow `docs/RELEASING.md` for `v0.1.0-alpha.2`: read it first, then push the tag and watch the release workflow to its end. A failure there is reported to the user, not worked around.
