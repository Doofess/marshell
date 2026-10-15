# Phase 2a: process hygiene and resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A crash or panic never leaves orphan processes or a dead app: Windows children die with Marshell, a failed core restarts itself and the window says so, every file in `~/.marshell` is versioned, atomic and backed up, and Windows builds ship the faster sideloaded ConPTY.

**Architecture:** Four small core modules: `store` (versioned atomic JSON files), `crash` (panic hook to a log), `supervisor` (owns the running core, health-checks it every second, restarts it on a new port and token), and `job` (Windows-only app-wide Job Object). The Tauri shell talks to the supervisor instead of a bare `RunningCore` and emits a `core-restarted` event that the terminal view handles. A pinned Node script fetches the ConPTY NuGet package; `build.rs` and a Windows-only Tauri config put the files next to the exe.

**Tech Stack:** Rust (tokio, axum, serde_json, `windows-sys` 0.61), Tauri 2 events, React + TypeScript, Node (fetch script), GitHub Actions.

**Spec:** `docs/PLAN.md` §9 phase 2 ("Windows process hygiene (from S2)", "Resilience"), and `docs/spikes/S2-conpty.md` (Findings 3–4, Decision). Decisions from the 2026-10-15 session: in-process supervisor that ends running sessions; one app-wide Job Object; a pinned fetch script (no binaries in git); WebView2 bootstrapper and arm64 build deferred to phase 9.

## Global Constraints

- Windows 11 is the only tested platform. Every Windows-only item is behind `#[cfg(windows)]` and the other OSes still build and pass in CI.
- Keep the Rust simple: plain functions and structs, no new traits beyond the one in Task 1, no macros.
- New dependency allowed: `windows-sys` 0.61 (already in `Cargo.lock` as 0.61.2), Windows target only, features `Win32_Foundation`, `Win32_System_JobObjects`, `Win32_System_Threading`. No other new crates.
- Nothing in a log, a crash line, an event payload or a panic message contains the token (`Endpoint`'s `Debug` already redacts it; keep using it).
- Files in `~/.marshell` stay owner-only: reuse `create_private_dir` / `write_private_file` from `auth.rs` (moved in Task 1).
- ConPTY package: `Microsoft.Windows.Console.ConPTY` version `1.25.260930003`, MIT, from nuget.org. No binaries are committed.
- UI copy follows PLAN's plain-words rules; the ended line is dim text in the terminal, not a dialog.
- Do not change `crates/protocol` types (no regenerated bindings in this slice).

## Review Focus

1. **A restart storm.** A core that dies right after every start must not restart forever: after 3 restarts within 60 s the supervisor stops and emits `core-failed` (Task 3 test `stops_after_three_restarts_in_a_minute`).
2. **A corrupt or newer file.** A file that does not parse falls back to its `.bak`; a file with a `version` higher than the app knows is never overwritten (Task 1 tests `reads_bak_when_main_is_corrupt`, `refuses_newer_version`).
3. **A poisoned lock after a panic.** Shutdown and restart must still kill sessions when the sessions mutex is poisoned (Task 3 test `restarts_when_state_is_poisoned` checks the old session's process is gone).
4. **Marshell started inside someone else's job** (some launchers and CI do this). Assigning the app to a job can fail; the app must log a warning and keep running (Task 5 test `job_failure_is_not_fatal`).
5. **Dev builds without the ConPTY files.** `build.rs` must not fail when `src-tauri/resources/conpty/` is missing; the app falls back to the inbox ConPTY (Task 6 step: `cargo build -p marshell` with the folder absent).

---

### Task 1: Versioned, atomic, backed-up files (`store`)

**Files:**
- Create: `crates/core/src/store.rs`
- Modify: `crates/core/src/auth.rs` (move `create_private_dir`, `write_private_file` into `store.rs` as `pub(crate)`; `write_endpoint_file` calls `store::write`)
- Modify: `crates/core/src/lib.rs` (add `pub mod store;`)
- Test: `crates/core/src/store.rs` (`#[cfg(test)] mod tests`, using `tempfile`)

**Interfaces:**
- Produces:
  - `pub trait Versioned: Serialize + DeserializeOwned { const VERSION: u32; fn migrate(from: u32, value: serde_json::Value) -> anyhow::Result<serde_json::Value> { let _ = from; Ok(value) } }`
  - `pub fn write<T: Versioned>(path: &Path, value: &T) -> anyhow::Result<()>`: serialises `value` to a JSON object and inserts `"version": T::VERSION` at the top level; if `path` exists, copies it to `<path>.bak` first; writes `<dir>/.<file name>.<pid>.tmp` with `write_private_file`, then renames over `path`; removes the temp file on error.
  - `pub fn read<T: Versioned>(path: &Path) -> anyhow::Result<Option<T>>`: `Ok(None)` when neither file exists. Parses `path`; if it is missing or fails to parse, tries `<path>.bak`. A file without `version` counts as version 0. `version < VERSION`: run `T::migrate(version, value)`, deserialise, and `write` the migrated value (so the old file becomes the `.bak`). `version > VERSION`: `Err` with the message `"<path> was written by a newer Marshell (version N); this one knows up to M"`, and the file is left untouched.
  - `pub fn bak_path(path: &Path) -> PathBuf` (appends `.bak` to the full file name).
- `auth::write_endpoint_file` keeps its signature. It writes through a private `EndpointFile(Endpoint)` wrapper that implements `Versioned` with `VERSION = 1`, serialised with `#[serde(transparent)]`.

- [ ] **Step 1: Write the failing tests** in `store.rs` with a test type `struct Prefs { name: String }` (`VERSION = 2`; `migrate(1, v)` renames the key `"title"` to `"name"`):
  - `writes_version_at_top_level`: after `write`, the file parses as JSON with `["version"] == 2` and `["name"] == "a"`.
  - `keeps_previous_as_bak`: write `"a"`, then `"b"`; `read` gives `"b"` and `bak_path` holds `"a"`.
  - `reads_bak_when_main_is_corrupt`: write `"a"`, write `"b"`, overwrite the main file with `"{not json"`; `read` gives `"a"`.
  - `migrates_old_version_and_rewrites`: hand-write `{"version":1,"title":"x"}`; `read` gives `name == "x"`; the main file now has `version` 2; the `.bak` has `version` 1.
  - `refuses_newer_version`: hand-write `{"version":3,"name":"x"}`; `read` is `Err` and its message contains `"newer Marshell"`; the file bytes are unchanged.
  - `missing_is_none`: `read` on an empty dir returns `Ok(None)`.
  - `no_temp_file_left`: after `write`, the directory holds only the file and its `.bak` (no `*.tmp`).
- [ ] **Step 2: Run `cargo test -p marshell-core store`.** Expected: fails to compile (module missing).
- [ ] **Step 3: Implement `store.rs`** per the Interfaces, and move the two private-file helpers out of `auth.rs`. Use `std::fs::copy` for the `.bak`. On Windows, `rename` over an existing file works through `std::fs::rename` (it uses `MoveFileExW` with replace).
- [ ] **Step 4: Point `write_endpoint_file` at `store::write`.** Extend the existing endpoint test (or add `endpoint_file_has_version_1`) to check that `endpoint.json` has `"version": 1` and still deserialises as an `Endpoint`.
- [ ] **Step 5: Run `cargo test -p marshell-core`.** Expected: all pass.
- [ ] **Step 6: Commit** `feat(core): versioned, atomic, backed-up files in ~/.marshell`.

### Task 2: Panic hook to a crash log (`crash`)

**Files:**
- Create: `crates/core/src/crash.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod crash;`), `crates/core/src/paths.rs` (add `pub fn crash_log(home: &Path) -> PathBuf` = `<home>/logs/crash.log`)
- Test: `crates/core/tests/crash_log.rs` (its own test binary, because the hook is process-global)

**Interfaces:**
- Produces: `pub fn install_panic_hook(home: &Path)`. It keeps the previous hook (`std::panic::take_hook`) and calls it after writing. Each panic appends one line: `<unix seconds> thread=<name or "unnamed"> at <file>:<line>: <message>` (the message from `&str` or `String` payloads, otherwise `"(non-string panic)"`). It creates `<home>/logs` with `store::create_private_dir`. Before appending, a file over 1 MiB is renamed to `crash.log.1`, replacing any old one. Write errors are ignored: the hook never panics itself.

- [ ] **Step 1: Write the failing test** `panics_are_appended_to_crash_log`: set a temp home, `install_panic_hook(home)`, spawn a thread named `"probe"` that panics with `"boom 42"`, then join it (expect `Err`). Read `logs/crash.log`; assert it contains `thread=probe` and `boom 42`, and that it is exactly one line.
- [ ] **Step 2: Write the failing test** `rotates_past_one_mebibyte`: pre-fill `crash.log` with 1 MiB + 1 byte, panic once, then assert that `crash.log.1` exists and `crash.log` is one line.
- [ ] **Step 3: Run `cargo test -p marshell-core --test crash_log`.** Expected: fails to compile.
- [ ] **Step 4: Implement `crash.rs`.**
- [ ] **Step 5: Run the tests.** Expected: pass.
- [ ] **Step 6: Commit** `feat(core): panics are written to ~/.marshell/logs/crash.log`.

### Task 3: The supervisor restarts a failed core

**Files:**
- Create: `crates/core/src/supervisor.rs`
- Modify: `crates/core/src/server/mod.rs`:
  - `CoreConfig` derives `Clone`.
  - `RunningCore` gains a private `serve: tokio::task::JoinHandle<()>` and `pub fn is_healthy(&self) -> bool`: false when the serve task has finished, or when the sessions mutex is poisoned.
  - `shutdown` locks with `.lock().unwrap_or_else(|e| e.into_inner())` so it still kills sessions after a poison.
  - Add `#[cfg(test)] pub(crate) fn poison_for_test(&self)`: it panics inside a scoped thread while holding the sessions lock.
  - Add `#[cfg(test)] pub(crate) fn stop_server_for_test(&self)`: it aborts the serve task and leaves the sessions running.
- Modify: `crates/core/src/lib.rs` (`pub mod supervisor;`)
- Test: `crates/core/src/supervisor.rs` (`#[cfg(test)] mod tests`, `#[tokio::test]`)

**Interfaces:**
- Consumes: `server::start(CoreConfig) -> anyhow::Result<RunningCore>`, `RunningCore::{endpoint, shutdown, is_healthy}`.
- Produces:
  - `pub enum CoreEvent { Restarted { generation: u32 }, Failed }`
  - `pub struct Supervisor`, which holds `cfg: CoreConfig`, `current: Mutex<Option<RunningCore>>`, `generation: AtomicU32` and `restarts: Mutex<Vec<Instant>>`, plus `on_event: Box<dyn Fn(CoreEvent) + Send + Sync>`.
  - `pub async fn start(cfg: CoreConfig, on_event: impl Fn(CoreEvent) + Send + Sync + 'static) -> anyhow::Result<Arc<Supervisor>>`
  - `pub fn endpoint(&self) -> Option<Endpoint>`: the current core's endpoint; `None` after `Failed` or `shutdown`.
  - `pub fn generation(&self) -> u32`: 0 at start, +1 per restart.
  - `pub async fn check_and_restart(&self) -> bool`. When the core is healthy, it returns false. Otherwise it takes the old core and calls `shutdown()` on it. If 3 restarts already happened in the last 60 s, it emits `Failed` and returns false. Otherwise it starts a new core, bumps the generation, emits `Restarted`, and returns true. A failed `server::start` counts as a restart attempt; the next check tries again.
  - `pub fn watch(self: &Arc<Self>, every: Duration)`: a `tokio::spawn` loop calling `check_and_restart` on a `tokio::time::interval`. It stops after `Failed` or `shutdown`.
  - `pub fn shutdown(&self)`: takes and shuts down the current core; the watch loop then ends.
  - `#[cfg(test)] fn current_for_test(&self) -> MutexGuard<'_, Option<RunningCore>>`.

- [ ] **Step 1: Write the failing tests** (temp `home`, `allowed_origins: vec![]`; the callback pushes into an `Arc<Mutex<Vec<CoreEvent>>>`; derive `Debug, PartialEq` on `CoreEvent`):
  - `healthy_core_is_left_alone`: `check_and_restart()` is false and the generation is 0.
  - `restarts_after_server_stops`: `current_for_test().as_ref().unwrap().stop_server_for_test()` and wait until `is_healthy()` is false, then `check_and_restart()` is true. The generation is 1, the new endpoint's port and token differ from the old ones, `endpoint.json` holds the new token, and the events are `[Restarted { generation: 1 }]`.
  - `restarts_when_state_is_poisoned`: insert a session into the state (`crate::pty::Session::spawn` with a long-running shell command, as the existing session tests do), then `poison_for_test()`. `check_and_restart()` is true, and within 2 s that session reports exited.
  - `stops_after_three_restarts_in_a_minute`: four times, call `stop_server_for_test()` on the current core and then `check_and_restart()`. The 4th `check_and_restart()` returns false, `endpoint()` is `None`, and the last event is `Failed`.
- [ ] **Step 2: Run `cargo test -p marshell-core supervisor`.** Expected: fails to compile.
- [ ] **Step 3: Implement** the `server/mod.rs` changes, then `supervisor.rs`.
- [ ] **Step 4: Run `cargo test -p marshell-core`.** Expected: all pass.
- [ ] **Step 5: Wire it into `src-tauri/src/main.rs`.**
  - `CoreHandle` becomes `struct CoreHandle(Arc<Supervisor>)`.
  - `setup` calls `marshell_core::crash::install_panic_hook(&home)` first. It then calls `Supervisor::start(cfg, ...)`, whose callback emits `core-restarted` with payload `{ "generation": n }` or `core-failed` with `()` through a cloned `AppHandle` (`tauri::Emitter::emit`). Then it calls `supervisor.watch(Duration::from_secs(1))`.
  - `core_endpoint` returns `supervisor.endpoint()`, with `"the core is not running"` as the error.
  - `RunEvent::Exit` calls `supervisor.shutdown()`.
- [ ] **Step 6: Run `cargo build --workspace` and `cargo test --workspace`.** Expected: both pass.
- [ ] **Step 7: Commit** `feat(core): a supervisor restarts a failed core on a new port and token`.

### Task 4: The window handles a restarted or failed core

**Files:**
- Create: `src/lib/api/coreEvents.ts`
- Modify: `src/lib/api/session.ts` (add `forgetSession()`, which sets `pending = null`), `src/features/terminal/TerminalView.tsx`, and the component that calls `startSession()` and renders `TerminalView` (find it with `grep -rn startSession src`)
- Test: `src/lib/api/coreEvents.test.ts`, `src/features/terminal/coreRestart.test.ts`

**Interfaces:**
- Produces:
  - `export function onCoreEvent(cb: (e: { kind: "restarted"; generation: number } | { kind: "failed" }) => void): Promise<() => void>`. It wraps `listen` from `@tauri-apps/api/event` for `core-restarted` and `core-failed`, and resolves to one unlisten that removes both.
  - `export const ENDED_LINE = "\r\n\x1b[2mMarshell's core restarted, so this session ended. Press Enter for a new shell.\x1b[0m\r\n"`
  - `export const FAILED_LINE = "\r\n\x1b[2mMarshell's core stopped and could not restart. Restart Marshell to continue.\x1b[0m\r\n"` (both in `src/features/terminal/coreRestart.ts`)
  - `TerminalView` gets the prop `onNewShell: () => void`.

- [ ] **Step 1: Write the failing tests.**
  - `coreEvents.test.ts` mocks `@tauri-apps/api/event` (`vi.mock`). It checks that `onCoreEvent` registers both event names, maps `core-restarted` with payload `{ generation: 2 }` to `{ kind: "restarted", generation: 2 }`, and that the returned function calls both unlisteners.
  - `coreRestart.test.ts` asserts the two lines' exact text.
- [ ] **Step 2: Run `pnpm vitest run src/lib/api/coreEvents.test.ts src/features/terminal/coreRestart.test.ts`.** Expected: FAIL (modules missing).
- [ ] **Step 3: Implement.**
  - In `TerminalView`'s effect, subscribe with `onCoreEvent`.
  - On `restarted`: `socket.close()`, then `term.write(ENDED_LINE)`. From then on, keystrokes go nowhere except Enter (`\r`), which calls `onNewShell()`.
  - On `failed`: `socket.close()`, then `term.write(FAILED_LINE)`.
  - Unsubscribe in the effect's cleanup.
  - The parent's `onNewShell` calls `forgetSession()`, then `startSession()`, and remounts `TerminalView` with a new `key` (the new tab id).
- [ ] **Step 4: Run `pnpm test`.** Expected: all pass.
- [ ] **Step 5: Commit** `feat(terminal): a restarted core ends the session and Enter starts a new shell`.

### Task 5: Children die with the app on Windows (`job`)

**Files:**
- Create: `crates/core/src/job.rs`, `crates/core/src/bin/job-probe.rs`
- Modify: `crates/core/Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = [...] }`, and a `[[bin]] name = "job-probe"` if bins are listed explicitly), `crates/core/src/lib.rs` (`pub mod job;`), `src-tauri/src/main.rs` (call it first in `main`)
- Test: `crates/core/tests/job_object.rs` (`#![cfg(windows)]`)

**Interfaces:**
- Produces:
  - `pub fn tie_children_to_app() -> anyhow::Result<()>`. On Windows it calls `CreateJobObjectW(null, null)`, then `SetInformationJobObject` with `JobObjectExtendedLimitInformation` and `LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK`, then `AssignProcessToJobObject(job, GetCurrentProcess())`. The job handle is deliberately never closed: Windows closes it when the process ends, which kills every child. With `BREAKAWAY_OK`, a program started with `CREATE_BREAKAWAY_FROM_JOB` can still outlive Marshell. On other OSes it is a no-op `Ok(())`. Each `unsafe` block has a `// SAFETY:` line.
  - `src-tauri` `main()` calls `if let Err(e) = marshell_core::job::tie_children_to_app() { eprintln!("Marshell: children may outlive a crash: {e}"); }` before `linux_webview_workarounds()`.
  - The `job-probe` bin calls `tie_children_to_app()` and prints `ok` or `err <message>`. It then spawns `ping -n 60 127.0.0.1` (stdout null), prints the child's pid on its own line, flushes, and sleeps 60 s.

- [ ] **Step 1: Write the failing test** `child_dies_when_app_is_killed`.
  - Start the `job-probe` bin (path from `env!("CARGO_BIN_EXE_job-probe")`) with piped stdout, and read the first two lines: `ok` and a pid.
  - Kill the probe with `Child::kill()` (TerminateProcess, as a crash would).
  - Within 3 s, `tasklist /FI "PID eq <pid>" /NH` must not list `PING.EXE`. Poll every 100 ms.
- [ ] **Step 2: Write the test** `job_failure_is_not_fatal`. Call `tie_children_to_app()` twice in one process. The second call either succeeds (nested jobs are allowed on Windows 8 and later) or returns `Err`; either way it must not panic, and the process keeps running. Assert only that it returns.
- [ ] **Step 3: Run `cargo test -p marshell-core --test job_object`.** Expected: fails to compile.
- [ ] **Step 4: Implement** `job.rs`, the probe, and the `main()` call.
- [ ] **Step 5: Run `cargo test --workspace`.** Expected: pass on Windows. The job test file compiles to nothing elsewhere.
- [ ] **Step 6: Manual check.** Run `pnpm tauri dev`, then end Marshell's process in Task Manager. `Get-Process pwsh, OpenConsole -ErrorAction SilentlyContinue` must show none of its children left. Record the result in `docs/spikes/S2-conpty.md` under Finding 4.
- [ ] **Step 7: Commit** `feat(core): on Windows every child process dies with Marshell (Job Object)`.

### Task 6: Sideloaded ConPTY in Windows builds

**Files:**
- Create: `scripts/fetch-conpty.mjs`, `src-tauri/tauri.windows.conf.json`, `THIRD_PARTY_NOTICES.md`
- Modify: `src-tauri/build.rs`, `.gitignore` (add `src-tauri/resources/conpty/`), `package.json` (script `"fetch:conpty": "node scripts/fetch-conpty.mjs"`), `.github/workflows/ci.yml` (Windows job runs `pnpm fetch:conpty` before the build), `docs/spikes/S2-conpty.md` (Decision: done), `CONTRIBUTING.md` (one line: run `pnpm fetch:conpty` once on Windows)

**Interfaces:**
- `fetch-conpty.mjs`: Node 20+, no dependencies.
  - Constants `VERSION = "1.25.260930003"` and `SHA512`, the base64 `packageHash` from `https://api.nuget.org/v3/registration5-semver1/microsoft.windows.console.conpty/1.25.260930003.json` → `catalogEntry` → `packageHash`. Read it once while implementing and paste it in.
  - It downloads `https://www.nuget.org/api/v2/package/Microsoft.Windows.Console.ConPTY/1.25.260930003` into a temp dir and verifies SHA-512 (it exits 1 with `hash mismatch` otherwise). It extracts with `tar -xf` (bsdtar on Windows reads zip).
  - For each of `x64` and `arm64`, it copies `conpty.dll` and `OpenConsole.exe` to `src-tauri/resources/conpty/<arch>/`. Find the paths inside the package once while implementing and keep them as constants. It also copies the package's licence to `src-tauri/resources/conpty/LICENSE`.
  - It skips the work when the files exist and a `src-tauri/resources/conpty/VERSION` file matches.
- `build.rs`: on a Windows target, if `resources/conpty/x64/conpty.dll` exists, copy both files next to the built exe. The profile dir is the third ancestor of `OUT_DIR`. Add `cargo:rerun-if-changed=resources/conpty`. If the folder is missing, print `cargo:warning=` with "ConPTY files not fetched; using the inbox ConPTY (run pnpm fetch:conpty)" and continue. Do this before `tauri_build::build()`.
- `tauri.windows.conf.json`: `bundle.resources` maps `resources/conpty/x64/conpty.dll` → `conpty.dll`, `resources/conpty/x64/OpenConsole.exe` → `OpenConsole.exe` and `resources/conpty/LICENSE` → `licenses/conpty-LICENSE.txt`. This uses the install directory, which is the exe directory on Windows. arm64 waits for phase 9.
- `THIRD_PARTY_NOTICES.md`: a heading per bundled component. This slice adds Microsoft ConPTY (OpenConsole) with its MIT text and the package version.

- [ ] **Step 1: Run `node scripts/fetch-conpty.mjs`** once with a wrong `SHA512`. Expected: exit 1 with `hash mismatch`. Then put the real value in and run it again. Expected: the four binaries and `LICENSE` exist. Run it a third time. Expected: it reports it is up to date and downloads nothing.
- [ ] **Step 2: Implement `build.rs`.** Delete `src-tauri/resources/conpty/` and run `cargo build -p marshell`. Expected: success with the warning line (Review Focus 5). Fetch again and rebuild. Expected: `target/debug/conpty.dll` and `target/debug/OpenConsole.exe` exist.
- [ ] **Step 3: Manual check.** Run `pnpm tauri dev` and open the tab. `Get-Process OpenConsole | Select-Object Path` must show the copy in `target\debug`. Optionally re-run the S2 flood benchmark (`bash -c "yes | head -n 2000000 | cat"`) and note the time in S2.
- [ ] **Step 4: Add the CI step, `tauri.windows.conf.json`, the notices, `.gitignore`, and the S2 and CONTRIBUTING lines.** Push and check that the Windows CI job logs the fetch and passes.
- [ ] **Step 5: Commit** `build(windows): ship the sideloaded ConPTY (OpenConsole) next to the exe, with its MIT notice`.

### Task 7: PLAN records the slice and the deferrals

**Files:**
- Modify: `docs/PLAN.md` §9 phase 2

- [ ] **Step 1: Edit `docs/PLAN.md`.**
  - In the "Windows process hygiene" bullet, move "WebView2 bootstrapper; arm64 build" to phase 9's scope with "(moved from phase 2 on 2026-10-15: installer concerns)".
  - Under phase 2, add one line listing the slices: 2a hygiene and resilience; 2b tabs, sidebar, profiles and launcher; 2c shell integration; 2d themes; 2e split, layout restore and Resume; 2f recording, palette and cheat sheet.
  - Add one line saying the core restart ends running sessions. The Ended + Resume tab state arrives in 2e; until then the terminal says the session ended and Enter starts a new shell.
- [ ] **Step 2: Commit** `docs: phase 2 slices; WebView2 bootstrapper and arm64 move to phase 9`.
