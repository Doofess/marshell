# The elevated helper (slice 6j2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On Windows, any profile (a shell, an agent or a tool) can start as administrator from the launcher. A small `marshell-host` helper, started through UAC, owns the elevated ConPTY and pumps its bytes to the non-elevated core over named pipes. The tab renders like any other, and only the app's own window can steer it.

**Architecture:**
- **A dumb byte pump.**
  - `marshell-host` runs elevated. It opens the two pipes the core created and checks that the core is the server. It puts itself in a kill-on-close Job Object, reads one Spawn frame and starts the shell in a ConPTY.
  - From then on it only copies bytes. Output goes up, and Input and Resize frames come down.
  - Everything else stays in the core and is unchanged: the DSR answer, vt100, the ring buffer, recording, OSC 133 marks and the classifier. This meets S8 finding 1 without a second filter.
- **Two one-way pipes, not one duplex pipe.** On a synchronous handle, Windows serializes I/O. A reader blocked in `ReadFile` would hold up the `WriteFile` that carries ConPTY's startup `ESC[6n` answer, and the session would deadlock before its first prompt. With one pipe per direction, each handle has one thread and stays synchronous.
- **A seam in `Session`.** `Session::spawn` keeps building a local portable-pty backend. `Session::spawn_elevated` builds a pipe backend. Both feed one `Session::start(id, spec, PtyParts)`.
- **No token (decision 1).**
  - The core creates both pipes before it launches anything: single-instance, with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, and a DACL for the current user only.
  - It launches the helper with `ShellExecuteExW` `runas` and keeps `hProcess` open until the client pid is checked.
  - The helper checks `GetNamedPipeServerProcessId` against `--core-pid`.
- **The full environment travels in Spawn.** An elevated process started through `runas` gets a fresh environment block, not the core's. So the core computes the child's whole environment (its own, with `env_remove`, `env_strip` and `env` applied) and sends it, and the helper clears its own before applying it. Without this the hooks lose `MARSHELL_TAB_ID`, `MARSHELL_HOME` and the bridge on `PATH`. A live check would also lose its scratch `CLAUDE_CONFIG_DIR` and touch the real `~/.claude`.
- **Trusted input (decision 8).**
  - The token-guarded API refuses input, resize and approval answers for an elevated row.
  - The window sends them through three Tauri commands that call the in-process core through `Supervisor`.
  - Output still streams over the pty WebSocket.

**Tech Stack:** Rust (axum core, portable-pty 0.9, windows-sys 0.61), a new `crates/host` binary, Tauri 2 commands, ts-rs protocol types, React + Vitest.

**Spec:** `docs/PLAN.md` §9 "Slice 6j2 decisions (2026-11-01)" (binding), spike `docs/spikes/S8-elevated-host.md`, BRIEF "Elevated sessions" (§5.7).

## Global Constraints

- **Windows only (decision 4).** `crates/host` builds everywhere, but its `main` on other systems prints `marshell-host runs on Windows only.` and exits 2. A create with `elevated: true` on another system is a 400: `Running as administrator is available on Windows only for now.`
- **The helper accepts nothing but its frames.**
  - The first frame is Spawn, exactly once.
  - After it, only Input and Resize (cols and rows each 1..=1000) are accepted.
  - Anything else closes both pipes and exits: an unknown kind, a frame over `MAX_FRAME` (1 MiB), a second Spawn, or Resize out of bounds.
  - The helper never reads a file, a socket or the registry on the core's behalf.
- **The test-only unelevated mode** exists only under `#[cfg(debug_assertions)]`. It is enabled by `MARSHELL_HOST_ALLOW_UNELEVATED=1`, and a release build ignores the variable.
- **Where the helper runs from:**
  - The production path is always `ShellExecuteExW` with `runas`. Tests inject a launcher that starts the helper unelevated.
  - The helper runs from next to the app's exe (`bridge::source_next_to(current_exe, HOST_BIN)`), never from `<home>/bin`, and is not part of `bridge::ensure`.
  - `HOST_BIN = "marshell-host"` goes in `brand.rs`.
- **Elevation is never remembered (decision 3).** `LauncherChoice` gets no elevated field. Restart, Resume and Reopen of an elevated tab start elevated again (a new UAC prompt) and never fall back to unelevated.
- **No guessed status:** a refused or failed elevated start shows the core's words, never a generic error.
- **Concurrency:**
  - `ShellExecuteExW`, pipe connect and pipe I/O run on `spawn_blocking` or plain threads.
  - Never hold a `MutexGuard` across `.await` or I/O.
- **Routes:** a changed route keeps its "refused without the bearer" test.
- **UI:**
  - Keyboard alone and forced colors.
  - Colors from tokens.
  - Sentence case, no "!".
  - The exact copy in the table below.
- **Dependencies:** no new crate. windows-sys gains features only: `Win32_System_Pipes`, `Win32_System_IO`, and `Win32_Security` (already present) for the DACL.
- **Shared rules:** run `cargo fmt --all` before every Rust commit. Cargo tests run with `-j 4`.

**Copy (exact):**

| Where | Text |
|---|---|
| UAC declined (create 409) | `Administrator rights were declined. Nothing was started.` |
| Wrong pipe client (create 500) | `Another program connected to the administrator session's pipe, so it was refused. Nothing was started.` |
| Helper didn't connect in 10 s (create 500) | `The administrator helper didn't connect within 10 seconds. Nothing was started.` |
| Helper exited before connecting (create 500) | `The administrator helper stopped before it connected (exit code <n>). Nothing was started.` |
| Helper missing (create 500) | `The administrator helper (marshell-host) is missing next to Marshell. Reinstall Marshell.` |
| Folder not reachable elevated (helper Failed) | `The folder <cwd> can't be reached as administrator. A mapped or substituted drive is visible only to your own sessions. Nothing was started.` |
| API write to an elevated row (409) | `This session runs as administrator, so it takes input only from Marshell's own window.` |
| Always allow on an elevated row (409) | `A session running as administrator can't keep an always-allow rule from Marshell. Allow this once instead.` |
| Pause on an elevated row (409, and the menu's reason line) | `Pause isn't available for a session running as administrator.` |
| Non-Windows (create 400) | `Running as administrator is available on Windows only for now.` |
| Launcher chip, off-Windows reason line | `Administrator: available on Windows only for now.` |
| Launcher, Mode chip while Administrator is on | `An agent running as administrator starts in Manual.` |
| New tab text while UAC is up | `Waiting for administrator approval…` |
| Ended elevated row's action | `Reopen as administrator` |
| Bypass confirmation (restart bar checkbox) | `I understand this agent runs as administrator and Bypass lets it run admin commands without asking.` |
| Palette command | `New session as administrator` |

## Review Focus

1. **The startup DSR deadlock.** A fresh elevated `pwsh` must draw its first prompt with no input sent. Pinned by `first_prompt_renders_with_no_input` (Task 3).
2. **The environment does not reach the elevated child.** The shell must see `MARSHELL_TAB_ID` and a profile env value. Pinned by `the_shell_sees_the_core_computed_environment` (Task 3). The fresh-block claim itself is verified in the live check (Task 8).
3. **The app dies while an elevated session runs.** The admin shell must not outlive it. Pinned by `dropping_the_core_side_ends_the_shell` (Task 3). The live check kills the app by PID.
4. **The helper dies before connecting, or never starts.** Create must fail with the copy within 10 s, not hang. Pinned by `a_helper_that_exits_before_connecting_fails_fast` (Task 3).
5. **A folder on a mapped or `subst` drive.** It must fail with the copy rather than start in another folder. Pinned by `a_missing_cwd_fails_with_the_copy` (Task 2).

---

### Task 1: Protocol: the host frames and the elevated fields

**Files:**
- Create: `crates/protocol/src/host.rs` (plain Rust, no ts-rs)
- Modify: `crates/protocol/src/lib.rs`, `crates/protocol/src/brand.rs`, `crates/protocol/src/api.rs` (`CreateSessionRequest`, `SessionInfo`, `SavedTab`)
- Regenerated: `src/generated/CreateSessionRequest.ts`, `SessionInfo.ts`, `SavedTab.ts`

**Interfaces:**
- Produces:
  - `pub const MAX_FRAME: usize = 1 << 20;`
  - `pub enum ToHost { Spawn(SpawnRequest), Input(Vec<u8>), Resize { cols: u16, rows: u16 } }`
  - `pub enum FromHost { Started { pid: u32 }, Failed(String), Output(Vec<u8>), Exit { code: i32 } }`
  - `#[derive(Serialize, Deserialize)] pub struct SpawnRequest { pub argv: Vec<String>, pub cwd: Option<String>, pub env: Vec<(String, String)>, pub cols: u16, pub rows: u16 }`. Here `env` is the child's whole environment. No `Debug`: it can hold a preset's key.
  - `pub fn write_to_host(w: &mut impl Write, m: &ToHost) -> io::Result<()>`, `pub fn read_to_host(r: &mut impl Read) -> io::Result<Option<ToHost>>`, and the same pair for `FromHost`. `None` means a clean EOF at a frame boundary.
  - `brand::HOST_BIN = "marshell-host"`.
  - `elevated: bool` with `#[serde(default)]` on `CreateSessionRequest`, `SessionInfo` and `SavedTab`.
- Wire format: `[kind u8][len u32 LE][payload]`.
  - Kinds: Spawn 1 (JSON), Input 2 (raw), Resize 3 (`cols u16 LE, rows u16 LE`), Started 0x81 (`u32 LE`), Failed 0x82 (UTF-8), Output 0x83 (raw), Exit 0x84 (`i32 LE`).
  - Errors are `InvalidData`: `len > MAX_FRAME`, an unknown kind, a wrong fixed payload length, a Resize with 0 or above 1000, or EOF mid-frame.

- [ ] **Step 1: Write the failing tests** in `host.rs`:
  - `every_message_round_trips`
  - `eof_at_a_boundary_is_none_and_mid_frame_is_an_error`
  - `a_frame_over_max_is_refused_before_reading_its_payload`: a header claiming `MAX_FRAME + 1` with no payload behind it gives `InvalidData`, not an EOF.
  - `unknown_kind_and_bad_resize_are_invalid`
  - `a_from_host_kind_is_not_a_to_host_message`: `read_to_host` on a Started frame is `InvalidData`.
  - In `api.rs` tests, `elevated_defaults_to_false_in_old_json`: a `SavedTab` and a `CreateSessionRequest` without the field.
- [ ] **Step 2:** `cargo test -p marshell-protocol -j 4 host`. Expect FAIL (module missing).
- [ ] **Step 3:** Implement the module and the fields. Fix every struct literal of the three types across the workspace with `elevated: false`.
- [ ] **Step 4:** Run `cargo test -p marshell-protocol -j 4` and expect PASS. Then run `pnpm check:generated`, `cargo build --workspace -j 4` and `pnpm build`, and expect them clean. The frontend may need `elevated: false` in fixtures that build `SessionInfo`.
- [ ] **Step 5:** Commit `feat(protocol): add the elevated helper's frames and an elevated flag on sessions`.

### Task 2: The `marshell-host` binary

**Files:**
- Create: `crates/host/Cargo.toml` (package `marshell-host`, bin `marshell-host`), `crates/host/src/main.rs`, `crates/host/src/win.rs` (pipe client, pid check, elevation, job), `crates/host/src/pump.rs`
- Modify: root `Cargo.toml` (`members` += `crates/host`), `package.json` (`build:hook` and `build:hook:release` gain `-p marshell-host`)
- Test: unit tests in `pump.rs`

**Interfaces:**
- Consumes: Task 1's `host` module.
- Produces: the CLI `marshell-host --pipe <base> --core-pid <pid>`. The helper opens `\\.\pipe\<base>-down` for reading (core to helper) and `\\.\pipe\<base>-up` for writing (helper to core), and connects within 10 s.
- Dependencies: `marshell-protocol`, `portable-pty`, `anyhow`, `serde_json`, and windows-sys (`Win32_Foundation`, `Win32_Security`, `Win32_Storage_FileSystem`, `Win32_System_Pipes`, `Win32_System_JobObjects`, `Win32_System_Threading`). Versions come from the workspace or match `crates/core`.

Order in `main` (Windows):
1. Parse the args, or exit 2.
2. Require elevation (`TokenElevation`). In debug builds only, `MARSHELL_HOST_ALLOW_UNELEVATED=1` skips this. Otherwise exit 3.
3. Create a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and no breakaway, and assign the helper to it, so every child inherits it.
4. Open both pipes with `CreateFileW` and `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`, retrying for up to 10 s.
5. Check that `GetNamedPipeServerProcessId` equals `--core-pid` on both, or exit 4.
6. Read one frame, which must be Spawn.
7. Check that `cwd` exists. If not, send `Failed(<folder copy>)` and exit 5.
8. Build the `CommandBuilder` with `env_clear()` and then each pair. Open the pty and spawn, then send `Started { pid }`.
9. Start the pump threads:
   - pty reader → `Output`
   - pipe reader → Input to the pty writer, Resize to `master.resize`
   - wait → `Exit { code }`
   Exit on pipe EOF, a protocol error, or after `Exit` is written. Leaving `main` closes the job handle, which kills the tree.

`pump.rs` holds the frame loop as `fn serve_frames(down: impl Read, sink: &mut dyn PtySink) -> io::Result<()>`, with `trait PtySink { fn input(&mut self, b: &[u8]); fn resize(&mut self, cols: u16, rows: u16); }`, so it is testable without Windows.

- [ ] **Step 1: Write the failing tests** in `pump.rs`:
  - `input_and_resize_reach_the_sink_in_order`
  - `a_second_spawn_stops_the_loop_with_an_error`
  - `an_unknown_kind_stops_the_loop`
  - `eof_ends_the_loop_cleanly`
  - In `main.rs` tests (Windows), `a_missing_cwd_fails_with_the_copy`: the cwd check function returns the exact folder copy for `Z:\no-such-marshell-folder`.
- [ ] **Step 2:** `cargo test -p marshell-host -j 4`. Expect FAIL.
- [ ] **Step 3:** Implement. Copy the job code from `crates/core/src/job.rs` rather than depending on the core: the helper links only protocol, portable-pty and windows-sys.
- [ ] **Step 4:** `cargo test -p marshell-host -j 4` should PASS. `cargo clippy -p marshell-host --all-targets -j 4 -- -D warnings` should be clean.
- [ ] **Step 5:** Commit `feat(host): add marshell-host, the elevated pty helper`.

### Task 3: Core: launch the helper and connect

**Files:**
- Create: `crates/core/src/pty/elevated.rs` (`#[cfg(windows)]`, with a non-Windows stub returning the non-Windows copy)
- Modify: `crates/core/src/pty/mod.rs`, `crates/core/Cargo.toml` (the windows-sys features above)
- Test: `crates/host/tests/elevated.rs`, with `marshell-core` as a dev-dependency of `marshell-host`, using `env!("CARGO_BIN_EXE_marshell-host")` and setting `MARSHELL_HOST_ALLOW_UNELEVATED=1` on the child. Pipe-only unit tests go in `elevated.rs`.

**Interfaces:**
- Consumes: Task 1's frames, Task 2's CLI.
- Produces:
  - `pub trait HostLauncher: Send + Sync { fn launch(&self, exe: &Path, args: &str) -> Result<LaunchedHost, ElevatedError>; }`
  - `pub struct LaunchedHost { pub pid: u32, pub process: OwnedHandle }`
  - `pub struct RunAs;`, the production launcher: `ShellExecuteExW` with `runas`, `SW_HIDE`, and `SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI`. `ERROR_CANCELLED` (1223) maps to `ElevatedError::Declined`.
  - `pub struct PlainLaunch { pub env: Vec<(String, String)> }`, test-only (`#[doc(hidden)] pub`): `CreateProcessW` with the inherited environment plus `env`. Tests pass `MARSHELL_HOST_ALLOW_UNELEVATED=1` here and never set it on the test process.
  - `pub enum ElevatedError { Declined, WrongPeer, Timeout, ExitedEarly(i32), Missing, Failed(String), Io(io::Error) }`, with `Display` giving the copy table's text.
  - `pub fn connect(launcher: &dyn HostLauncher, exe: &Path, req: SpawnRequest) -> Result<RemotePty, ElevatedError>`
  - `pub struct RemotePty { pub pid: u32, pub reader: Box<dyn Read + Send>, pub writer: Box<dyn Write + Send>, pub control: Box<dyn PtyControl>, pub wait: Box<dyn FnOnce() -> i32 + Send> }`. Here `reader` yields the bytes of `Output` frames, `writer` wraps bytes as `Input` frames, and `wait` returns `Exit.code`, or -1 at EOF without one.
  - `pub trait PtyControl: Send + Sync { fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()>; fn close(&self); }`

The steps of `connect`:
1. Make a base name `marshell-host-<32 hex from getrandom>`.
2. Create `-down` (outbound) and `-up` (inbound) with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_TYPE_BYTE | PIPE_REJECT_REMOTE_CLIENTS`, max instances 1, and a security descriptor granting `GENERIC_ALL` to the current user's SID only (built with `ConvertStringSecurityDescriptorToSecurityDescriptorW("D:P(A;;GA;;;<user sid>)")`).
3. Launch with `--pipe <base> --core-pid <own pid>`.
4. Wait on three things together: connect on both pipes (a thread each), the helper's process handle, and a 10 s deadline. Whichever comes first decides between connected, `ExitedEarly(code)` and `Timeout`.
5. Check that `GetNamedPipeClientProcessId` equals `LaunchedHost.pid` on both pipes. On a mismatch, close everything and return `WrongPeer`. Close the process handle only after this check.
6. Send Spawn, then read one frame: `Started` → `RemotePty`, `Failed(t)` → `Failed(t)`.
7. Start one demux thread that reads the up pipe and feeds a channel-backed `Read` (`Output`), and records `Exit`.

- [ ] **Step 1: Write the failing tests:**
  - **Unit tests** (`elevated.rs`, in-process, no helper):
    - `a_second_client_cannot_open_the_pipe`: after one `CreateFileW`, a second open fails with `ERROR_PIPE_BUSY`.
    - `a_client_that_is_not_the_launched_pid_is_refused`: a fake launcher returns pid 1 and the test process opens both pipes itself, giving `WrongPeer`.
    - `a_pipe_name_already_taken_fails_the_create`: `FIRST_PIPE_INSTANCE`.
  - **End-to-end tests** (`crates/host/tests/elevated.rs`, with `PlainLaunch` and the real helper):
    - `first_prompt_renders_with_no_input`: spawn `cmd.exe /d /q /k echo ready` through the core's `connect`, answer the `ESC[6n` the way `Session` does, and expect `ready` within 5 s.
    - `the_shell_sees_the_core_computed_environment`: env contains `MARSHELL_TAB_ID=t-elev` and `ZZ_PROFILE=1`, the argv is `cmd /d /c echo %MARSHELL_TAB_ID% %ZZ_PROFILE%`, and the output contains `t-elev 1`.
    - `input_and_resize_reach_the_shell`
    - `dropping_the_core_side_ends_the_shell`: take the shell pid from `Started`, drop the `RemotePty`, and within 3 s the pid is gone (`OpenProcess` plus `GetExitCodeProcess` is not 259).
    - `a_helper_that_exits_before_connecting_fails_fast`: a launcher that starts the helper with bad args gives `ExitedEarly(2)` in under 3 s.
    - `the_helper_refuses_unelevated_without_the_switch`: no env var, exit code 3.
- [ ] **Step 2:** `cargo test -p marshell-core -j 4 elevated` and `cargo test -p marshell-host -j 4`. Expect FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Both commands PASS, and clippy is clean.
- [ ] **Step 5:** Commit `feat(core): launch marshell-host through UAC and connect over two checked pipes`.

### Task 4: Core: a session with an elevated backend

**Files:**
- Modify: `crates/core/src/pty/session.rs` (spawn 187-297, resize 690-713, kill around 926-938, freeze)
- Test: `crates/host/tests/elevated.rs` (session-level), `crates/core/src/pty/session.rs` tests (the local path stays unchanged)

**Interfaces:**
- Consumes: Task 3's `connect`, `RemotePty` and `PtyControl`.
- Produces:
  - `pub struct PtyParts { reader, writer, control: Box<dyn PtyControl>, wait: Box<dyn FnOnce() -> i32 + Send>, pid: Option<u32>, elevated: bool }`
  - `Session::spawn(id, spec)`, unchanged for callers, now builds local parts.
  - `pub fn spawn_elevated(id: String, spec: SpawnSpec, launcher: &dyn HostLauncher, host_exe: &Path) -> anyhow::Result<Arc<Session>>`. It computes the child environment with `pub(crate) fn child_env(spec: &SpawnSpec) -> Vec<(String, String)>`, which uses a `CommandBuilder` with the same remove, strip and env steps as the local path and then `iter_full_env_as_str()`. Then it connects and starts.
  - `Session::is_elevated(&self) -> bool`
  - `Session::freeze` returns `Err(PauseError::Elevated)` for an elevated session, with `Display` giving the Pause copy.
  - Kill of an elevated session calls `control.close()` (closes both pipes) and never `taskkill`.
  - `#[doc(hidden)] pub fn spawn_marked_elevated_for_tests(id: String, spec: SpawnSpec) -> anyhow::Result<Arc<Session>>`: a local backend with `elevated: true`. `crates/core/tests` can't reach the helper binary, so Task 5's API tests use this. Production never calls it, and a grep in review confirms that.
- A local `PtyControl` wraps `Box<dyn MasterPty + Send>`, and its `close` drops the master.

- [ ] **Step 1: Write the failing tests:**
  - `an_elevated_session_answers_the_startup_dsr_and_records`: `Session::spawn_elevated` with `PlainLaunch` and the real helper on `cmd /d /q /k echo ready`. The session's screen contains `ready` with no input, and the recording file has the output.
  - `killing_an_elevated_session_ends_its_shell`
  - `pause_on_an_elevated_session_is_refused_with_the_copy`
  - `child_env_applies_remove_strip_and_env_in_order`: a unit test in `session.rs` with a strip on `ZZ_STRIP_` and a remove that `env` then sets again.
- [ ] **Step 2:** Run the tests. Expect FAIL.
- [ ] **Step 3:** Refactor `spawn` into `start(id, spec, PtyParts)`, then add `spawn_elevated`. `read_loop`, `ingest` and the recorder are untouched.
- [ ] **Step 4:** `cargo test -p marshell-core -j 4` passes in full, which proves no regression on the local path. `cargo test -p marshell-host -j 4` PASS.
- [ ] **Step 5:** Commit `feat(core): run a session through the elevated helper behind the same pty seam`.

### Task 5: Core: create, restart, resume and layout; the API refusals; approvals

**Files:**
- Modify:
  - `crates/core/src/server/sessions.rs` (create about 760-800, restart, resume, `send` at 1038, the pause route)
  - `crates/core/src/server/pty_ws.rs:96-105`
  - `crates/core/src/server/fill.rs:140`
  - `crates/core/src/server/attachments.rs:143,197`
  - `crates/core/src/server/permissions.rs`
  - `crates/core/src/server/hook.rs` (where the pending permission's view is built)
  - `crates/core/src/registry.rs` (`elevated` on the entry, `SessionInfo`, `SavedTab`)
  - `crates/core/src/layout.rs`
  - `crates/core/src/supervisor.rs`
  - `crates/core/src/server/mod.rs` (`AppState` gains `launcher: Arc<dyn HostLauncher>`, `RunAs` in production)
- Test: `crates/core/tests/elevated_api.rs`, plus the existing `layout_restore.rs`, `permissions.rs` and `restart.rs`. Tests construct `AppState` with a fake launcher that returns `Declined`. Rows that must already be elevated are inserted through `spawn_marked_elevated_for_tests` (Task 4). The real-helper end-to-end case stays in `crates/host/tests`.

**Interfaces:**
- Consumes: Task 4's `spawn_elevated` and `is_elevated`.
- Produces:
  - `pub struct Trusted(AppState)`, returned by `Supervisor::trusted(&self) -> Option<Trusted>`, with:
    - `pub fn input(&self, tab: &str, bytes: Vec<u8>) -> Result<(), String>`
    - `pub fn resize(&self, tab: &str, cols: u16, rows: u16) -> Result<(), String>`
    - `pub async fn decide(&self, rid: &str, decision: PermissionDecision) -> Result<(), (u16, String)>`
  - Each refuses a non-elevated tab (`"Not an administrator session."`), so other sessions keep one input path.
  - `server::permissions::decide(st, rid, decision)` holds the shared body that the route and `Trusted::decide` call.
- Rules:
  - **Create** with `elevated`:
    - On a non-Windows system, return 400 with the copy.
    - On Windows, an elevated agent's mode is forced to `Manual` whatever was sent (decision 2).
    - Find the helper with `bridge::source_next_to(current_exe, HOST_BIN)`. If it is missing, return 500 with the copy.
    - Spawn with `spawn_elevated` on `spawn_blocking`.
    - `Declined` maps to 409, and the other errors to 500, each with their copy.
    - `projects::record` still runs.
  - **Restart, Resume and Reopen** of an elevated tab start elevated again. The restart keeps the requested mode, including Bypass: the confirmation lives in the UI (Task 7).
  - **`SessionInfo.elevated`** comes from the registry entry. **`SavedTab.elevated`** round-trips through `layout.json`, and a restored elevated tab is `Ended` like any other restored tab.
  - **API writes** to an elevated session return 409 with the API-write copy:
    - `send`, `fill` and the attachment pastes
    - the pty WebSocket: an Input or Resize frame for an elevated session is dropped, and the socket gets one `error` frame with the copy and stays open for output and acks
    - `/permissions/<rid>`, when the pending request's tab is elevated
  - **The Pause route** on an elevated session returns 409 with the Pause copy.
  - **Approvals:** a pending permission from an elevated tab has `always_rule: None` in its view. A decision with `PermissionScope::Project` for an elevated tab is a 409 with the Always copy, through `Trusted::decide` too.

- [ ] **Step 1: Write the failing tests** in `elevated_api.rs`:
  - `create_elevated_declined_is_409_with_the_copy`
  - `create_elevated_agent_starts_in_manual`: a unit test in `sessions.rs` of `fn launch_mode(elevated: bool, agent: Option<&str>, requested: Option<SessionMode>) -> Option<SessionMode>`, which create calls. It covers Bypass and Plan becoming Manual for an agent, a shell keeping None, and the unelevated cases unchanged.
  - `send_fill_and_attachments_refuse_an_elevated_row`
  - `ws_input_to_an_elevated_row_is_dropped_with_an_error_frame`
  - `the_permission_route_refuses_an_elevated_row_and_trusted_decide_accepts`
  - `always_scope_on_an_elevated_row_is_409_and_the_view_has_no_rule`
  - `pause_on_an_elevated_row_is_409`
  - `elevated_round_trips_through_layout_and_restores_ended`
  - `trusted_input_refuses_a_plain_session`
  - Each changed route keeps its "refused without the bearer" test.
  - Off Windows only: `create_elevated_off_windows_is_400` (`#[cfg(not(windows))]`).
- [ ] **Step 2:** `cargo test -p marshell-core -j 4 --test elevated_api`. Expect FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --workspace -j 4` PASS, and clippy is clean.
- [ ] **Step 5:** Commit `feat(core): start, restore and guard administrator sessions; their input comes only through the window`.

### Task 6: The window's trusted path (Tauri commands and the frontend's input sink)

**Files:**
- Modify: `src-tauri/src/main.rs` (three commands in `generate_handler!`), and the frontend input sinks:
  - the terminal's `onData` and resize (where `src/lib/api/ptySocket.ts` sends Input and Resize)
  - the prompt line's send (`api.send` in `src/lib/api/sessions.ts`)
  - the approval answer (`src/lib/api/permissions.ts`)
- Create: `src/lib/api/elevated.ts`
- Test: `src/lib/api/elevated.test.ts`, plus the existing `ptySocket.test.ts` and `permissions.test.ts`

**Interfaces:**
- Consumes: Task 5's `Supervisor::trusted()`.
- Produces:
  - Tauri commands:
    - `elevated_input(tab_id: String, text: String) -> Result<(), String>`
    - `elevated_resize(tab_id: String, cols: u16, rows: u16) -> Result<(), String>`
    - `elevated_decide(rid: String, decision: PermissionDecision) -> Result<(), String>`. The error string is the core's copy.
  - App commands need no capability entry, as with `close_once`. Check `src-tauri/capabilities/default.json` and change nothing there.
  - `src/lib/api/elevated.ts` exports `sendInput(tabId, text)`, `resize(tabId, cols, rows)` and `decide(rid, decision)`. They wrap `invoke` with an injectable `invoke` for tests.
  - For a row with `elevated: true`:
    - The terminal sends keystrokes, DSR replies and resizes through these, never the socket.
    - The prompt line's send writes the same bytes through `sendInput`.
    - An approval answer goes through `decide`.
    - Every other row is unchanged.

- [ ] **Step 1: Write the failing tests:**
  - `elevated_rows_send_input_through_invoke_not_the_socket`
  - `resize_of_an_elevated_row_goes_through_invoke`
  - `a_plain_row_still_uses_the_socket`
  - `an_elevated_approval_answer_goes_through_invoke`
  - `an_invoke_error_shows_the_core_copy`
- [ ] **Step 2:** `pnpm test src/lib/api/elevated.test.ts`. Expect FAIL.
- [ ] **Step 3:** Implement. On the Rust side, `cargo build -p marshell-app -j 4` must build.
- [ ] **Step 4:** Run `pnpm test`, `pnpm build`, and `cargo clippy --workspace --all-targets -j 4 -- -D warnings`. All should PASS or be clean.
- [ ] **Step 5:** Commit `feat(app): send an administrator session's input and approvals only through the window`.

### Task 7: The UI: launcher, palette, rows, Reopen, Pause, the Bypass confirmation, approve cards

**Files:**
- Modify:
  - `src/features/launcher/Launcher.tsx`, `launchChoice.ts`, `fixtures.ts`, `Launcher.stories.tsx`
  - `src/lib/keymap.ts` (the palette command `new-session-admin`), `src/features/sessions/LivePalette.tsx`
  - `src/features/sessions/LiveApp.tsx` (the row mapping `elevated` from `SessionInfo`, and the Reopen label)
  - `src/features/sessions/sessionActions.ts` (Pause, `askRestart` and the instant switch)
  - the restart bar component
  - `src/features/approval/ApproveCard.tsx`, `types.ts`
- Test: `Launcher.test.tsx`, `launchChoice.test.ts`, `sessionActions.test.ts`, `ApproveCard.test.tsx`, `SessionRow.test.tsx`, `labels.test.ts`, and the restart bar's test

**Interfaces:**
- Consumes: `SessionInfo.elevated` and `CreateSessionRequest.elevated` (Task 1), and the core's copy (Task 5).
- Produces:
  - The launcher's `elevated` UI state, which is not in `LauncherChoice` and not remembered. When on, the request carries `elevated: true`.
  - `ApprovalRequest.elevated?: boolean`
- Rules:
  - **The Administrator chip.**
    - It sits after Pace and is shown for every program, using the worktree chip's `role="checkbox"` pattern.
    - A shield (`AuxGlyph kind="elevated"`) shows when it is checked, with `aria-keyshortcuts="Alt+A"`. Alt+A toggles it anywhere in the launcher sheet.
    - Off Windows (`os !== "windows"`) it is disabled, with the off-Windows reason line under the chips.
    - When it is on and the program is an agent, the Mode chip shows Manual, is disabled, and carries the Mode copy as its reason line.
  - **The palette** command `New session as administrator` opens the launcher with the chip on. It is hidden off Windows.
  - **The new tab** reads `Waiting for administrator approval…` until the create returns, then follows the usual path. A 409 or 500 shows the core's copy in the existing failed-start UI.
  - **Rows and the header** already draw `row.elevated` (the shield and "Administrator"). Wire it from `SessionInfo.elevated`.
  - **An Ended or Exited elevated row's** Resume action reads `Reopen as administrator`, in the button, the row menu and the palette.
  - **Pause on an elevated row** is disabled in the row menu with the Pause copy as its reason, and the palette hides it.
  - **The Bypass confirmation.**
    - Any path that would put an elevated agent in Bypass shows the restart bar with the Bypass checkbox, and Restart stays disabled until it is checked: `askRestart`, and an instant mode switch, which must use the bar for elevated rows.
    - The same applies when the effective mode is Bypass (`effectiveMode`), for example because the CLI's own settings skip every prompt.
    - The checkbox is reachable by Tab and toggled by Space.
  - **The approve card** on an elevated row shows the shield before the session name and never renders "Always allow". Its key map drops the Always key: pass `hasAlways: false`.

- [ ] **Step 1: Write the failing tests:**
  - `the_admin_chip_toggles_with_alt_a_and_sends_elevated`
  - `the_admin_chip_is_disabled_off_windows_with_the_reason`
  - `an_agent_with_admin_on_is_manual_and_the_mode_chip_is_disabled`
  - `admin_is_never_remembered_for_the_project`
  - `the_palette_command_opens_the_launcher_with_admin_on`
  - `the_new_tab_waits_for_approval`
  - `an_ended_elevated_row_offers_reopen_as_administrator`
  - `pause_is_unavailable_on_an_elevated_row`
  - `bypass_on_an_elevated_row_needs_the_checkbox`
  - `an_instant_switch_to_bypass_on_an_elevated_row_uses_the_bar`
  - `an_elevated_card_has_the_shield_and_no_always_allow`
  - Plus `labels.test.ts`, which already covers "Administrator." and must stay green.
- [ ] **Step 2:** Run `pnpm test` on those files. Expect FAIL.
- [ ] **Step 3:** Implement. Read the `frontend-design` and `good-css` skills before any CSS, and use colors from tokens only. Add a story per new state with `ThemePair`.
- [ ] **Step 4:** Run `pnpm test` and `pnpm build`. Both should PASS.
- [ ] **Step 5:** Commit `feat(ui): start a session as administrator from the launcher, with Reopen, no Pause and a Bypass confirmation`.

### Task 8: Live check, research notes and the spike's record

**Files:**
- Create: `docs/research/2026-11-01-elevated-host-live-check.md`
- Modify: `docs/spikes/S8-elevated-host.md` (a "Shipped in 6j2" note: named pipes instead of a token, and its decline and wrong-peer results)

Run it with the `marshell-live-check` skill (scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, CDP on 9333). The user answers UAC (decision 7), and each prompt is announced before it appears.

1. **The decline path.** Launcher, PowerShell, Administrator on, Enter. The user declines. The tab shows the declined copy, and no `marshell-host` or `pwsh` is left running (list the tree by PID).
2. **An approved shell.**
   - Approve. The first prompt draws without input.
   - `whoami /groups | findstr Mandatory` shows High Mandatory Level.
   - The row has the shield and "Administrator".
   - The `pwsh` environment lacks the core's own variables but has `MARSHELL_TAB_ID`, which confirms the fresh block.
3. **API input is refused.** A loopback request to `send` with the scratch token gets the 409 copy, and nothing appears in the tab. Typing in the window works.
4. **An approved agent.** Claude Code starts in Manual with the scratch `CLAUDE_CONFIG_DIR`. Its approve card has the shield and no Always allow, and an Allow from the card works. Switching to Bypass needs the checkbox and raises UAC again.
5. **Pause is unavailable.** Close the tab, and the helper and shell PIDs are gone within 3 s.
6. **Restart Marshell.** The elevated row comes back Ended with `Reopen as administrator`. No UAC prompt appears until the user chooses it.
7. **Kill the app by PID** while an elevated shell runs. Its helper and shell are gone within 3 s.
8. **A `subst` drive.** If the user agrees to a scratch `subst`, a cwd on it fails with the folder copy.

- [ ] **Step 1:** Run the check and write the findings, including every PID started and stopped.
- [ ] **Step 2:** Commit `docs(research): record the elevated helper's live check`.
