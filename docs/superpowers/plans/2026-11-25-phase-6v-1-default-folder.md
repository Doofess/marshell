# Slice 6v-1: the default folder, and the shell panel with no session. Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marshell gets a default folder (`%USERPROFILE%\Marshell`, changeable in Settings) that new sessions, the launcher's folder dialog and the sidebar's shell panel use when nothing else names a folder, and the shell panel starts with no session open.

**Architecture:** The core owns the folder. `CoreConfig` tells a core its built-in default folder (none in tests), `settings.json` can name another, and one place (`AppState::default_folder_path`) puts the two together. Marshell makes only its own folder, when the core starts; a folder chosen in Settings must already be there. A start that finds no folder falls back to the home folder as before. The client reads the effective folder from `GET /v1/default-folder` and never writes it back into the settings.

**Tech Stack:** Rust (axum, tokio, serde, ts-rs), React and TypeScript, Vitest. No new dependency.

**Spec:** `docs/PLAN.md` §9, "Slice 6v decisions (2026-11-25)": decisions 4, 5, 6, 9, 11, 12 and 13. Decisions 1 to 3, 7, 8 and 10 are slice 6v-2 and are out of scope here.

## Global Constraints

- Tests and live checks never make or read the real `%USERPROFILE%\Marshell`. A core whose `CoreConfig.default_folder` is `None` has no default folder at all.
- Marshell makes one folder only: its own built-in default folder (`CoreConfig.default_folder`), when the core starts and when Settings goes back to it. A path that arrives in a request is never made (`.claude/rules/local-api.md`: no route takes a file path to write). Nothing is made when a session or the shell panel starts.
- The plan was reviewed against the code before it ran; the review's corrections are in the text.
- The effective path never goes into `settings.json`. `default_folder: null` there means "Marshell's own folder".
- Local API rules (`.claude/rules/local-api.md`): the new route sits in the token-guarded router and has a "refused without the bearer" test; no `MutexGuard` is held across `.await` or disk work; disk work runs in `spawn_blocking`; new request and response types go in `crates/protocol`; `src/generated/` is regenerated with `cargo test -p marshell-protocol -j 4` and never edited by hand.
- `guard.rs`, token handling, the CSP, `src-tauri/capabilities/`, `tauri.conf.json`, `.github/workflows/` and the installer's `hooks.nsh` are not touched.
- No new dependency.
- Copy is fixed by this plan, word for word. No dash as punctuation in UI copy.
- Every UI change works with the keyboard alone and in forced colours. Styles reuse the classes the backup-folder row uses (`src/features/history/StorageSection.tsx`); if a new declaration is needed, read the `good-css` skill first.
- Each task: write the test first and watch it fail. `cargo fmt --all` before every Rust commit. `cargo test … -j 4`, never more jobs.
- Commit messages: `type(scope): what changed, as a sentence.`
- Workers: stay inside the task's files; never `git checkout`, `git restore` or `git stash` anything; scratch files go under the system temp folder in a folder of the task's own.

## Review Focus

1. **The very first run.** The shell panel asks for its shell as soon as the window is up and no session exists. If the folder is still being made then, the shell starts in the home folder on exactly the run the feature is for. Expected: the folder exists before the core advertises its endpoint (Task 2, `start_makes_the_default_folder_before_it_serves`).
2. **Sessions exist but none is focused yet.** The panel must wait for the focused session's folder as it does today, not jump to the default folder (Task 3, `shellPanelAsk` tests; the effect's wiring is live-check item 5).
3. **A default folder that can't be reached** (an unplugged drive, a deleted folder, a file named `Marshell` where the folder should be). Expected: starts fall back to the home folder within two seconds, a theme change still saves and keeps the chosen folder, and Settings says the folder isn't there (Task 1 `a theme change keeps the default folder`; Task 2 `a_missing_default_folder_gives_way_to_home`, `put_does_not_look_at_an_unchanged_default_folder`, `a_file_where_the_default_folder_should_be_is_reported_missing`; Task 4 view test).
4. **A hand-edited `settings.json`.** A `default_folder` that is a number, blank, relative or a share must load as none, keep every other setting, and not make later saves fail (Task 1).
5. **The profile's folder against the default folder.** A profile that sets a folder wins for sessions and for the shell panel, and the launcher's "started in" line names the folder really used (Task 2 `start_folder_order`, Task 3 `the_profile_folder_wins_for_the_panel`, Task 4 `startFolderName` tests). The reviewer also looks at the starts that must not change: a restart, a resume from the tab or from History, "Open in system terminal", a script, and a sign-in or update tab.

---

### Task 1: The protocol types and the setting in `settings.json`

Worker, `effort: high` (settings file parsing and writing).

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Settings` at about line 2060 and its `Default`; new types beside it)
- Modify: `crates/core/src/settings.rs` (`FileDoc`, `load`, `save`, `validate`, tests)
- Regenerate: `src/generated/` (`Settings.ts`, new `DefaultFolder.ts`, `ShellPanelRequest.ts`, `ShellPanelResponse.ts`)
- Modify: `crates/protocol/src/api.rs`, the doc comment of `Profile.cwd` ("None means the user's home directory" becomes "None means the default folder")
- Modify: `src/features/appearance/settingsSync.ts` and `settingsSync.test.ts` (below); `pnpm build` finds any other TypeScript that builds a whole `Settings`

**Interfaces:**
- Produces, in `marshell_protocol::api`:
  ```rust
  // in Settings, after history_backup_dir; Default: None
  /// Where the shell panel and new sessions start when nothing else names a folder. None is Marshell's own folder.
  pub default_folder: Option<String>,

  /// GET /v1/default-folder: the default folder as it is now.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
  #[ts(export)]
  pub struct DefaultFolder {
      /// The folder's full path. None when this core has none (then starts use the home folder).
      pub path: Option<String>,
      /// Whether a folder is there now.
      pub exists: bool,
      /// Whether the person chose it in Settings. False for Marshell's own folder.
      pub chosen: bool,
  }

  /// POST /v1/shell-panel body.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
  #[ts(export)]
  pub struct ShellPanelRequest {
      /// The focused session's folder. None with no session: the profile's folder, else the default folder.
      #[serde(default)]
      pub cwd: Option<String>,
      pub cols: u16,
      pub rows: u16,
  }

  /// POST /v1/shell-panel answer.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
  #[ts(export)]
  pub struct ShellPanelResponse {
      pub tab_id: String,
      /// The folder the panel's shell started in.
      pub cwd: String,
  }
  ```
- Produces, in `crates/core/src/settings.rs`:
  ```rust
  pub const FOLDER_NOT_FULL: &str = "Choose a full folder path.";
  pub const FOLDER_NOT_LOCAL: &str = "Choose a folder on this PC.";
  /// Whether `dir` may be the default folder, without looking at the disk: a full path, and not a network share.
  pub fn check_default_folder(dir: &str) -> Result<(), String>
  ```
  Blank text is not full. Then the share check, which runs before the full-path check: a share is text that starts with two slashes of either kind (`\\server\share`, `//server/share`, `\\?\UNC\…`, `\\wsl.localhost\…`, `\\.\…`), except the verbatim form of a drive path (`\\?\C:\x`), which is local. Then "full" is `std::path::Path::is_absolute` on the text after a leading `\\?\` is dropped (on Windows `C:\x` and `C:/x` are full; `C:x`, `\x` and `/x` are not).
- Produces, in `settingsSync.ts`: `default_folder` is carried through every save exactly as `history_backup_dir` is: a private field read in `load`, sent in both `put` calls (about lines 261 and 348), taken back from both answers (about lines 270 and 370), and `"default_folder"` joins `FlagChange` (about line 33). The getter and setter are Task 4's.

**Steps:**

- [ ] **Step 1: Write the failing tests** in `crates/protocol/src/api.rs` tests and `crates/core/src/settings.rs` tests:
  - `settings_default_has_no_default_folder`: `Settings::default().default_folder == None`.
  - `a_shell_panel_request_without_a_folder_reads`: `{"cols":80,"rows":24}` parses with `cwd == None`; `{"cwd":"C:/x","cols":80,"rows":24}` with `Some("C:/x")`.
  - `default_folder_round_trips`: save `Some("D:/Work")` (use a temp-dir path made absolute for the platform), load gives the same; save `None`, load gives `None`.
  - `a_default_folder_that_does_not_read_is_dropped_alone`: a file with `"default_folder": 7` and `"onboarded": true` loads with `default_folder == None` and `onboarded == true`; the same for `"  "`, `"relative/path"` and `"\\\\server\\share\\x"`.
  - `check_default_folder_wants_a_full_local_path`: `""`, `"  "`, `"work"`, `"..\\x"` give `Err(FOLDER_NOT_FULL)`; `"\\\\server\\share"`, `"//server/share"`, `"\\\\?\\UNC\\server\\share"`, `"\\\\wsl.localhost\\Ubuntu\\home\\u"` give `Err(FOLDER_NOT_LOCAL)`; a temp dir's path is `Ok`; on Windows `"\\\\?\\C:\\Users\\x"` is `Ok` (`#[cfg(windows)]` for that line).
  - `validate_refuses_a_default_folder_that_is_not_full`: `validate` of settings with `default_folder: Some("work".into())` is `Err(FOLDER_NOT_FULL)`; with `None` it is `Ok`.
  - `settingsSync.test.ts`, `a theme change keeps the default folder`: the loaded settings hold `default_folder: "D:/Work"`; `save(appearance)` puts a body whose `default_folder` is `"D:/Work"`; the same for one flag setter (`setCloseToTray`).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 default_folder`, `cargo test -p marshell-protocol -j 4` and `pnpm test src/features/appearance/settingsSync.test.ts`. Expected: the Rust runs FAIL to compile (the field and the function don't exist); the TypeScript test FAILS (the body holds no `default_folder`).
- [ ] **Step 3: Implement.** Add the field and the three types to the protocol. In `settings.rs`: `FileDoc.default_folder: Option<String>` read with a lenient deserializer that takes any JSON value and keeps only a string (the pattern of `lenient_browser`, one field); `load` keeps it only when it is not blank and `check_default_folder` passes, with a `tracing::warn!` naming the field and not its value otherwise; `save` writes it; `validate` calls `check_default_folder` when it is `Some`.
- [ ] **Step 4: Regenerate and build.** Run `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated` (Expected: ok). Carry `default_folder` through `settingsSync.ts` as the Interfaces say: never a literal `default_folder: null` in a body that is saved, or every theme change would wipe the chosen folder (the core saves the body as given). Only test fixtures get `default_folder: null`. Then `pnpm build`.
- [ ] **Step 5: Run** `cargo test -p marshell-core -j 4 settings` and `cargo test -p marshell-protocol -j 4`. Expected: PASS. Run `pnpm test src/features/appearance`. Expected: PASS.
- [ ] **Step 6: Commit** `feat(settings): a default folder setting, and protocol types for it and for the shell panel.`

---

### Task 2: The core's default folder

Worker, `effort: high` (local API, locks, start-up order).

**Files:**
- Modify: `crates/core/src/paths.rs` (new functions beside `downloads_dir`)
- Modify: `crates/core/src/server/mod.rs` (`CoreConfig`, `Inner`, `AppState`, `start`, `router`)
- Modify: `crates/core/src/server/sessions.rs` (`start_folder` at about line 490 and its callers at 576, 1368, 1500)
- Modify: `crates/core/src/server/outside.rs` (about line 308) and `crates/core/src/server/shell_panel.rs:115`: both only gain the new third argument, `None` (Task 3 replaces the panel's call)
- Modify: `crates/core/src/server/history.rs` (about lines 633 and 660) only if it stops compiling
- Modify: `crates/core/src/server/settings.rs` (`put`, new `default_folder` handler, tests)
- Modify: every place that builds a `CoreConfig`: `default_folder: None` in `crates/cli/src/start.rs:275`, `crates/cli/tests/cli.rs:39`, `crates/core/src/supervisor.rs:190`, `crates/core/tests/elevated_api.rs:650`, `crates/core/tests/events.rs:15`, `crates/core/tests/pty_ws.rs:14` and `:667`, `crates/core/tests/server.rs:340` and `:372`; `default_folder: marshell_core::paths::builtin_default_folder()` in `src-tauri/src/main.rs:440`
- Test: `crates/core/tests/server.rs` (or a new `crates/core/tests/default_folder.rs` with the same helpers)

**Interfaces:**
- Consumes: Task 1's `Settings.default_folder`, `DefaultFolder`, `settings::check_default_folder`.
- Produces:
  ```rust
  // crates/core/src/paths.rs
  /// Marshell's own default folder: `$MARSHELL_DEFAULT_FOLDER` when set and not empty (live checks), else `<home>/Marshell`.
  pub fn builtin_default_folder() -> Option<PathBuf>
  fn builtin_default_folder_from(over: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf>

  // crates/core/src/server/mod.rs
  pub struct CoreConfig { /* … */
      /// Marshell's own default folder (`paths::builtin_default_folder()`). None in tests: then this core has none,
      /// makes none, and a start with no folder falls back to the home folder.
      pub default_folder: Option<PathBuf>,
  }
  impl AppState {
      pub fn with_default_folder(self, dir: PathBuf) -> Self;
      /// The folder Settings names, else Marshell's own. Doesn't look at the disk.
      pub(crate) fn default_folder_path(&self) -> Option<PathBuf>;
      /// The default folder when a folder is there now. Looked at off the async threads and for at most 2 seconds:
      /// a folder on a dead mapped drive reads as not there.
      pub(crate) async fn default_folder_now(&self) -> Option<String>;
  }

  // crates/core/src/server/sessions.rs
  /// The folder a session starts in: the requested one, else the profile's, else `fallback`, else the home folder.
  pub(super) fn start_folder(run: &Resolved, requested: Option<String>, fallback: Option<String>) -> String
  /// `start_folder` with the default folder as the fallback. The disk is only looked at when neither a request nor the profile names a folder.
  pub(super) async fn folder_for(st: &AppState, run: &Resolved, requested: Option<String>) -> String
  ```
  The folder name is `brand::APP_NAME`; the variable is `brand::env_var("DEFAULT_FOLDER")`.
- `start_folder` treats a profile folder that is blank as none (`run.cwd` filtered with `!c.trim().is_empty()`), so the core and the launcher's line agree.
- Route: `GET /v1/default-folder` → `Json<DefaultFolder>`, registered beside `/v1/settings` in the guarded router. `path` is `default_folder_path()`, `exists` is whether `default_folder_now()` answered, `chosen` is whether the setting is `Some`.
- `start()`: once the state is built and before `auth::write_endpoint_file`, when the setting names no folder and `cfg.default_folder` is `Some`, run `std::fs::create_dir_all` on that built-in folder in `spawn_blocking`, awaited. A failure is `tracing::warn!("could not make the default folder: …")` and nothing else. A folder chosen in Settings is never made: it lies where the person put it, possibly on a drive that isn't there now.
- `put` (settings), in the blocking closure and before the settings lock is taken:
  - when `to_save.default_folder` is `Some(dir)` and differs from the saved one: `Path::new(dir).is_dir()` must be true, else a 400 with `FOLDER_MISSING` (`pub const FOLDER_MISSING: &str = "That folder isn't there.";` in `crates/core/src/settings.rs`). Nothing is made. An unchanged value is never looked at.
  - when the saved one is `Some` and `to_save.default_folder` is `None` ("Use Marshell's folder"): after the save, `create_dir_all` on the built-in folder when the core has one; a failure is only logged. That path comes from `CoreConfig`, never from the request.
  - The closure's refusal becomes a `String` (the backup refusal's `.message()` goes into it).
- Callers: only a new session from the create route gets the default folder. The create route (`sessions.rs` about line 901) computes `folder_for(st, &run, req.cwd.clone()).await` and hands `start_session_as` `Some(folder)`; it keeps remembering the launcher choice from `req.cwd` alone (about lines 896 to 900), so the default folder is never remembered as a choice. `start_session_as` itself calls `start_folder(&run, cwd, None)`. Everything else passes `None` and behaves as before: a restart and a resume (about 1368 and 1500), a resume from History (`history.rs` about 633 and 660), "Open in system terminal" (`outside.rs:308`, whose copied command must keep matching the folder), a reopen, a script, and the sign-in and update tabs (`sessions.rs` about 457 to 468, which stay in the home folder as their comments say).

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `paths.rs`: `the_builtin_default_folder_is_the_override_else_home_marshell`: `from(Some("D:/x"), Some(home))` is `D:/x`; `from(Some(""), Some(home))` and `from(None, Some(home))` are `home/Marshell`; `from(None, None)` is `None`.
  - `sessions.rs` unit test `start_folder_order`: with a `Resolved` whose `cwd` is `Some("P")`: requested `Some("R")` gives `R`; requested `None` or `Some("")` gives `P` even with fallback `Some("F")`. With `cwd: None`: fallback `Some("F")` gives `F`; fallback `None` gives the home folder.
  - Integration (state built with `.with_default_folder(tmp.join("Marshell"))`):
    - `a_session_with_no_folder_starts_in_the_default_folder`: the folder exists; `POST /v1/sessions` with no `cwd` and a profile with none; the listed session's `cwd` is the default folder.
    - `a_missing_default_folder_gives_way_to_home`: the folder is not made; the session's `cwd` is `dirs::home_dir()`.
    - `a_restart_keeps_the_sessions_folder` (a guard of today's behaviour; it passes before the change): a session started in folder A, default folder B; after restart its `cwd` is still A.
    - `a_sign_in_tab_stays_in_the_home_folder`: with a default folder that exists, the tab the sign-in route starts (follow the existing sign-in tab test in `crates/core/tests/`) has the home folder as its `cwd`.
    - `default_folder_answers_path_exists_and_chosen`: `GET /v1/default-folder` gives `{path: <tmp>/Marshell, exists: false, chosen: false}`; after `create_dir_all`, `exists: true`; after a PUT of settings with another folder, that path with `chosen: true`.
    - `a_core_with_no_default_folder_answers_none`: plain `AppState::new`: `{path: null, exists: false, chosen: false}`.
    - `a_file_where_the_default_folder_should_be_is_reported_missing`: a file at the path: `exists: false`, and a session with no folder starts in the home folder.
    - `default_folder_is_refused_without_the_bearer`: 401 as the neighbouring routes' guard tests assert.
    - `put_takes_a_default_folder_that_is_there`: PUT with `default_folder` an existing temp folder: 200, and `GET /v1/default-folder` gives it with `exists: true, chosen: true`.
    - `put_refuses_a_default_folder_that_is_not_there`: PUT with `<tmp>/nope`: 400 `That folder isn't there.`, `<tmp>/nope` still doesn't exist, and `GET /v1/settings` still has `default_folder: null`.
    - `going_back_to_marshells_folder_makes_it`: the state has a built-in folder that doesn't exist and a chosen one saved; a PUT with `default_folder: null` answers 200 and the built-in folder exists.
    - `put_refuses_a_share_as_the_default_folder`: `\\server\share\x`: 400 `Choose a folder on this PC.` (from `validate`; nothing touches the network).
    - `put_does_not_look_at_an_unchanged_default_folder` (unit test in `server/settings.rs`; it does not use `CHECK_GATE`, which the backup test owns): `default_folder = Some(<tmp>/gone)` is put straight into `st.0.settings` (a full path with no folder there); a PUT that keeps that folder and changes only the accent answers 200 and saves the accent.
    - `start_makes_the_default_folder_before_it_serves`: `start(CoreConfig { default_folder: Some(tmp.join("Marshell")), … })` returns and the folder exists; `settings.json`, if written, has no path in `default_folder`.
    - `start_does_not_make_a_chosen_folder`: the home's `settings.json` names `<tmp>/chosen` (not there) and the config's built-in is `<tmp>/Marshell`: after `start` neither folder exists.
    - `a_slow_folder_reads_as_not_there`: only if the 2-second bound can be tested without a real dead drive (a test hook as `before_folder_check` is); otherwise leave it out and say so in the report.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 default_folder` (and the named tests). Expected: FAIL to compile.
- [ ] **Step 3: Implement** the interfaces above. `default_folder_path` clones the setting under the settings lock and drops the lock before returning. `default_folder_now` calls it, then `is_dir()` in `spawn_blocking` under `tokio::time::timeout(Duration::from_secs(2), …)`.
- [ ] **Step 4: Add `default_folder` to every `CoreConfig`** (`None` in tests, the built-in one in `src-tauri/src/main.rs`).
- [ ] **Step 5: Run** `cargo test -p marshell-core -j 4` whole and `cargo test -p marshell-cli -j 4`, output to a file, read the tail. Expected: PASS, no test newly ignored. Run `cargo clippy --workspace --all-targets -j 4 -- -D warnings`. Expected: clean.
- [ ] **Step 6: Commit** `feat(core): a default folder that sessions start in when nothing names one, made at start and served to the app.`

---

### Task 3: The shell panel with no session

Worker, `effort: high` (the panel's start race).

**Files:**
- Modify: `crates/core/src/server/shell_panel.rs` (`PanelRequest`/`PanelResponse` at lines 31 to 40, `alive_panel`, `open`)
- Modify: `crates/core/src/server/sessions.rs` (`spawn_into_map` and its callers: the new `record_project` argument; the recording is at about lines 783 to 790)
- Modify: `crates/core/tests/shell_panel.rs`, and the `open_panel` helpers in `crates/core/tests/foreground.rs` and `crates/core/tests/tasks.rs` only if they stop compiling
- Modify: `src/lib/api/sessions.ts` (`shellPanel`, about line 82) and `src/lib/api/sessions.test.ts` (the existing "opens it with a folder and size" test at about lines 133 to 137)
- Create: `src/features/sessions/shellPanelAsk.ts`, `src/features/sessions/shellPanelAsk.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (the shell panel effect, about lines 1474 to 1508)

**Interfaces:**
- Consumes: Task 1's `ShellPanelRequest`, `ShellPanelResponse`; Task 2's `folder_for`.
- Produces:
  - Core: `open` takes `Json<ShellPanelRequest>` and answers `Json<ShellPanelResponse>`; the folder is `folder_for(&st, &run, req.cwd).await`. A panel that already runs answers its own id and the folder it started in, whatever the request names (`alive_panel` returns the id and the folder); so does the loser of the two-requests race. `adopt` keeps a local `PanelResponse { tab_id }`: only `open` answers `ShellPanelResponse`.
  - Core: the panel's start records a recent project only when the request names a folder. `spawn_into_map` takes `record_project: bool`: `req.cwd.is_some()` from `shell_panel::open` (read before the folder is resolved), `true` from every other caller. Without this, every start of the app with no session would put "Marshell" at the top of the launcher's recent folders.
  - Client: `shellPanel(ep: Endpoint, cwd: string | null, size: { cols: number; rows: number }): Promise<{ tabId: string; cwd: string }>`.
  - `shellPanelAsk.ts`:
    ```ts
    /** What the shell panel asks the core for: a session's folder, the default folder (null), or nothing yet. */
    export function shellPanelAsk(i: { shown: boolean; sessions: number; focusedCwd: string | undefined }): { cwd: string | null } | null;
    ```
    `shown` false gives `null`. A focused folder gives `{ cwd: focusedCwd }`. No sessions gives `{ cwd: null }`. Sessions but no focused folder gives `null` (wait, as today).
- `LiveApp`: the effect asks when `shellPanelAsk` is not null, with `sessions: infos.length`; its dependency on `hasFocusedCwd` becomes a dependency on the ask's kind (`"wait" | "default" | "session"`). The panel's `folder` is `lastSegment(res.cwd)`, from the answer. The comment above the state says the panel starts in the default folder when no session is open.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `crates/core/tests/shell_panel.rs` (state with `.with_default_folder(…)`, the folder made):
    - `a_panel_with_no_folder_starts_in_the_default_folder`: POST `{cols, rows}`; the answer's `cwd` is the default folder.
    - `the_profile_folder_wins_for_the_panel`: the default profile has `cwd` set to folder P (PUT `/v1/profiles` as other tests there do, or the state's profile helper): the answer's `cwd` is P.
    - `a_panel_with_a_folder_starts_there`: POST with `cwd` A: the answer's `cwd` is A (today's behaviour, now asserted).
    - `a_running_panel_answers_its_own_folder`: open in A, open again with B: same `tab_id`, `cwd` A.
    - `two_panel_requests_at_once_share_one_shell` (existing): also assert both answers carry the same `cwd`.
    - `a_panel_in_the_default_folder_records_no_project`: open the panel with no `cwd`; `GET /v1/projects` doesn't list the default folder. And `a_panel_in_a_named_folder_records_it` (today's behaviour, a guard): open with `cwd` A; A is listed.
  - `src/features/sessions/shellPanelAsk.test.ts`: the four cases above, by name: `hidden asks nothing`, `a focused session gives its folder`, `no session gives the default folder`, `sessions with none focused waits`.
  - `src/lib/api/sessions.test.ts`: the existing "opens it with a folder and size" test now expects `{ tabId: "p1", cwd: … }` from an answer of `{ tab_id, cwd }`; add `posts a null cwd with no session`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test shell_panel` and `pnpm test src/features/sessions/shellPanelAsk.test.ts`. Expected: FAIL (compile error; missing module).
- [ ] **Step 3: Implement** the core change, then the client function, `shellPanelAsk` and the effect.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 --test shell_panel --test foreground --test tasks`, `pnpm test src/features/sessions`, `pnpm build`. Expected: PASS and a clean build.
- [ ] **Step 5: Commit** `feat(shell-panel): the sidebar's shell starts with no session open, in the profile's folder or the default folder.`

---

### Task 4: Settings, the launcher and the copy

Worker (medium).

**Files:**
- Create: `src/lib/api/defaultFolder.ts`
- Create: `src/features/settings/DefaultFolderRow.tsx`, `src/features/settings/DefaultFolderRow.test.tsx`
- Create: `src/features/launcher/startFolder.ts`, `src/features/launcher/startFolder.test.ts`
- Modify: `src/features/appearance/settingsSync.ts` and its test (`defaultFolder` getter, `setDefaultFolder`, beside `historyBackupDir` and `setHistoryBackupDir` at about lines 219 and 313; Task 1 already carries the field through every save)
- Modify: `src/features/settings/LiveSettings.tsx` (the `"profiles"` case, about line 156)
- Modify: `src/features/settings/ProfilesSection.tsx:35` and the tests that quote that line
- Modify: `src/features/sessions/LiveLauncher.tsx` (`browse` at about line 255; the two `"your home folder"` lines at about 401 and 464)
- Modify: `src/features/sessions/launchPrograms.ts:171` (comment only: "the core's default (the default folder)")

**Interfaces:**
- Consumes: Task 1's generated `DefaultFolder`; Task 2's `GET /v1/default-folder`.
- Produces:
  ```ts
  // src/lib/api/defaultFolder.ts
  export function getDefaultFolder(ep: Endpoint): Promise<DefaultFolder>;

  // settingsSync.ts
  get defaultFolder(): string | null;
  setDefaultFolder(dir: string | null): Promise<void>;

  // src/features/launcher/startFolder.ts
  export const HOME_FOLDER = "your home folder";
  /** The name of the folder a session really starts in, for the launcher's "started in" line. */
  export function startFolderName(project: string | undefined, profileCwd: string | null | undefined, folder: DefaultFolder | null): string;
  /** Where the launcher's folder dialog opens: the typed folder, else the chosen project, else the default folder when it is there. */
  export function browseStart(typed: string | null, project: string | null, folder: DefaultFolder | null): string | null;

  // DefaultFolderRow.tsx
  export function DefaultFolderRowView(p: { folder: DefaultFolder | null; error: string | null; busy: boolean; onChoose: () => void; onReset: () => void }): JSX.Element;
  export function DefaultFolderRow(p: { endpoint: Endpoint; sync: SettingsSync | undefined; announce?: (text: string) => void }): JSX.Element;
  ```
- `startFolderName`: the project's name when there is one; else the last segment of a non-blank `profileCwd`; else the last segment of `folder.path` when `folder?.exists` and the path is set; else `HOME_FOLDER`.
- The launcher reads `getDefaultFolder` once per endpoint when it mounts (a failure leaves `null`), and takes the profile's `cwd` from `loaded.doc.profiles` by the shown shell's id.

**Copy (exact):**
- Row label: `Default folder`
- Path line: the full `folder.path`; `–` while `folder` is null; `Your home folder` when `folder.path` is null.
- Buttons: `Choose folder…`, and `Use Marshell's folder` only when `folder.chosen`.
- Help: `Where the shell panel and new sessions start when no folder is chosen and the shell profile sets none. Marshell makes it when it's missing.`
- When `folder.path` is set and `folder.exists` is false: `This folder isn't there now, so shells and sessions start in your home folder.`
- Announced after a save: `Default folder set to {last segment}.`; after the reset: `Default folder set back to Marshell's own.`
- A refusal shows the core's own words in the row's error line (the 400's body), as the backup-folder row does.
- When the folder can't be read from the core (`getDefaultFolder` fails): the path line stays `–` and the error line reads `Couldn't read the default folder. Try again in a moment.`
- `ProfilesSection.tsx:35`: `Where sessions start. Empty means the default folder.`

**Behaviour:**
- The row is a `role="group"` labelled by its label, drawn above `<ProfilesSection>` in the `"profiles"` case, with the classes of the backup-folder field in `StorageSection.tsx` (import that CSS as it does). It saves at once and does not take part in the profiles' Save and Discard.
- `Choose folder…` opens `pickFolder(folder.path)`; a cancel changes nothing; a choice calls `sync.setDefaultFolder(dir)` and reads `getDefaultFolder` again. After a choice and after `Use Marshell's folder` (whose button then goes away), focus is put on `Choose folder…`. Both buttons are disabled while a save runs.
- Both buttons are plain `<button type="button">`, reached with Tab in reading order.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `startFolder.test.ts`: `a project names itself`; `a profile folder is named by its last part` (`C:\Work\api` gives `api`); `the default folder is named when the profile sets none` (`C:\Users\u\Marshell`, exists, gives `Marshell`); `a missing default folder reads your home folder`; `no default folder reads your home folder`; `browseStart prefers the typed folder, then the project, then the default folder`; `browseStart skips a default folder that is not there`.
  - `DefaultFolderRow.test.tsx` (`renderToStaticMarkup`): `shows the full path, the label and the help line`; `shows the reset button only for a chosen folder`; `says the folder is not there`; `shows the core's refusal`; `says when the folder could not be read`; `reads Your home folder when the core has none`; `disables both buttons while busy`; `the group is labelled by Default folder`.
  - `settingsSync.test.ts`: `setDefaultFolder saves the folder and keeps the other settings`; `setDefaultFolder(null) clears it` (follow the `setHistoryBackupDir` tests).
  - `ProfilesSection.test.tsx`: the folder field's help reads the new line.
- [ ] **Step 2: Run** `pnpm test src/features/launcher/startFolder.test.ts src/features/settings/DefaultFolderRow.test.tsx src/features/appearance`. Expected: FAIL (modules missing).
- [ ] **Step 3: Implement** the modules, the row, the `LiveSettings` mount (it already holds `sync` and `endpoint`), and the launcher's use of `startFolderName` and `browseStart`.
- [ ] **Step 4: Run** `pnpm test`, output to a file, read the tail. Expected: PASS. Run `pnpm build`. Expected: clean.
- [ ] **Step 5: Commit** `feat(settings): a Default folder row in Shell profiles, and the launcher names and opens the default folder.`

---

### Task 5: The live check's scratch default folder, and the instructions

Inline (small, tooling and docs).

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1` (beside `MARSHELL_DOWNLOADS`, about line 211, and the header comment at about line 22)
- Modify: `.claude/skills/marshell-live-check/SKILL.md` (the Start step's paragraph that names `MARSHELL_DOWNLOADS`)
- Modify: `AGENTS.md` ("Verifying a change": the list of scratch-home variables gains `MARSHELL_DEFAULT_FOLDER`)

**Steps:**

- [ ] **Step 1:** In `start` and `restart`, set `$env:MARSHELL_DEFAULT_FOLDER` to `<Scratch>\home\Marshell` and print it, as the script does for `MARSHELL_DOWNLOADS`. The script does not make the folder: the app does.
- [ ] **Step 2:** Say so in `SKILL.md` and add the variable to the list in `AGENTS.md`.
- [ ] **Step 3: Verify:** `pwsh -NoProfile -Command "$null = [scriptblock]::Create((Get-Content -Raw .claude/skills/marshell-live-check/live-check.ps1)); 'parses'"`. Expected: `parses`.
- [ ] **Step 4: Commit** `chore(live-check): a scratch default folder, so a live check never makes the real one.`

---

## Live check (exit criteria)

Run with `marshell-live-check` on a fresh scratch folder. Before the start, note whether `Marshell` exists in the real profile folder (`[Environment]::GetFolderPath('UserProfile')`, not `$env:USERPROFILE`, which a check's own processes see as the scratch home).

1. With no session after the start, `<Scratch>\home\Marshell` exists, the shell panel shows a running shell, and its header reads `Marshell`.
2. The folder the panel's shell is in is `<Scratch>\home\Marshell` (read from the core's answer, and from the prompt line the shell prints).
3. A shell session started from the launcher with no folder chosen starts in the default folder, and the announcement names `Marshell`.
4. With the default profile's Folder set to a scratch folder: a new session with no folder chosen starts there and the announcement names it; then, with every session closed and the app started again (`stop`, `restart`), the shell panel's shell starts there too.
5. With a session in focus when the panel is first shown, the panel's shell starts in that session's folder (unchanged).
6. Settings, Shell profiles: the row is above the profiles list and shows the full path, with no `Use Marshell's folder` button. After the setting is changed to a scratch folder, the row shows that path and the button; pressing it brings Marshell's own folder back and is announced.
7. A save of `\\server\share\x` is refused with `Choose a folder on this PC.`, a relative path with `Choose a full folder path.`, a full path with no folder there with `That folder isn't there.` (and no folder appears), and a theme change in the same visit still saves and keeps the chosen folder.
8. With the scratch default folder deleted while the app runs: a new session with no folder starts in the home folder, and the row reads `This folder isn't there now, so shells and sessions start in your home folder.` After a restart of the app (`restart`), the folder is back.
9. Tab reaches both buttons of the row in order; in emulated forced colours the row's text and buttons are readable.
10. After the check, `Marshell` in the real profile folder is as it was before it (absent stays absent), `settings.json` in the scratch home never holds the built-in path, and the launcher's recent folders don't list the default folder unless a session was started in it.

The native folder dialog (`Choose folder…` in Settings, Browse in the launcher) can't be driven over CDP: its start folder is covered by `browseStart`'s tests, and a hand check with the user goes in the slice's leftovers issue.

## After the tasks

The final whole-branch review (`marshell-reviewer`, Opus) with this plan's Review Focus, one fix pass, then `marshell-ship` and `pre-merge-review`. The research note is `docs/research/2026-11-25-default-folder-live-check.md`.
