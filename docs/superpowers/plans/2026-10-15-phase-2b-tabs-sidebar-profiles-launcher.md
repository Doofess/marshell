# Phase 2b: Tabs, Sidebar, Profiles and Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the one-tab app into a real session manager. The Rust core owns a numbered, ordered list of sessions. The sidebar shows that list live. The launcher starts a shell, or Claude Code, Codex or Antigravity inside a shell, from detected shell profiles that can be edited on a settings page.

**Architecture:**
- The core gains a `Registry` that holds session metadata and publishes snapshots through a `tokio::sync::watch` channel.
- A new `/v1/events` WebSocket pushes those snapshots to the UI.
- Profiles are detected shells merged with the user's edits from `~/.marshell/profiles.json`. An agent runs as a command inside a profile's shell.
- The frontend mounts the existing phase 0 `MainWindow`, `Sidebar` and `Launcher` and feeds them from the core. Terminals render through a new `renderTerminal` slot, so the design components stay presentational.

**Tech stack:** Rust (axum 0.8, tokio `watch`, serde, ts-rs), React and TypeScript, xterm.js, Vitest. No new dependencies.

**Spec:**
- `docs/PLAN.md`:
  - §9 phase 2, slice 2b.
  - "Session numbers" (around line 155).
  - "Closing a busy session" (line 288).
  - The API table (lines 498–502).
  - The keyboard table (line 654).
- `docs/BRIEF.md` amendments.
- Design decisions made with the user on 2026-10-15 are recorded in Global Constraints below.

## Global Constraints

- Keep the Rust simple: plain structs, `Mutex`, and no new traits beyond `store::Versioned`. Keep `unwrap()` on locks as in the existing code. No new crates or npm packages.
- The Rust core owns the session list: number, name, order, profile, cwd and agent. The UI never invents a session number. Numbers come from the lowest free slot, stay stable while the session lives, and are never renumbered on reorder.
- The launcher's agents are `claude`, `codex` and `antigravity` (binary `agy`), run inside the chosen shell. Shell programs are the detected profiles. Other agents in `LAUNCH_PROGRAMS` stay hidden until their adapters exist.
- Hide the mode, pace and worktree chips in the live launcher until phases 3 and 6. Keep them in stories.
- Status is limited before phase 3:
  - A running agent session shows `limited`.
  - A running shell session shows `idle`.
  - An exited session shows `ended`.
  - Never guess `working`.
- Windows detection covers PowerShell 7 (`pwsh.exe`), Windows PowerShell 5 (`powershell.exe`), `cmd.exe`, Git Bash, and each WSL distro.
  - WSL is launch only: `wsl.exe -d <distro>`, with no cwd translation. The S11 spike is phase 6.
  - On unix the only detected profile is `$SHELL` (`-l`).
- Profiles persist in `~/.marshell/profiles.json` through `crates/core/src/store.rs` (`version` field, atomic write, `.bak`).
  - A profile has an id, name, command (argv), folder (optional cwd) and env (pairs).
  - Detected profiles are never written to the file unless the user edited them.
- Env set on every session:
  - `MARSHELL_TAB_ID`
  - `MARSHELL_SESSION_NUMBER`
  - `MARSHELL_AGENT`: the agent id, or `shell`
  - `TERM=xterm-256color`
  - `COLORTERM=truecolor`
- Shortcuts come from `src/lib/keymap.ts`, unchanged:
  - Launcher: Ctrl+Shift+T.
  - Close: Ctrl+Shift+W.
  - Reopen: Ctrl+Shift+Alt+T.
  - Rename: F2.
  - Go to session: Ctrl+Shift+1–9.
- Recently closed sessions: keep 10 at most. Reopen starts a new process with the same profile, agent, cwd and name. The scrollback is not restored; Resume is 2e.
- The busy-close bar needs a known status. In 2b:
  - A **shell** session closes at once, with Reopen in the toast.
  - A **running agent** session (status unknown) shows the header bar "<Agent> may be mid-task in <project>." with [Cancel] (focused) and [Close now]. "Close when done" is phase 3.
- The launcher sheet is 560 px wide. The new tab appears at once with "Starting Claude Code in my-app…" (`startingText`). Under 100 ms is measured in manual verification.
- The sidebar shell panel, project grouping and focus mode move to slice 2e. This plan's last task edits PLAN §9 to say so.
- After a core restart, the session list resets to empty: the old sessions died with the core. The existing "Press Enter for a new shell" flow becomes "open the launcher".

## Review Focus

1. **A session exits on its own** (the user types `exit`). Its row must turn `ended` and keep its number until closed; the terminal must stay readable. Test: `registry_marks_exit_and_keeps_number` (Task 2) and `exited_session_is_ended_in_snapshot` (Task 6).
2. **A launch fails** (agent binary missing, or a bad profile argv). There must be no ghost row and no number leak. The launcher shows the core's error text inline and stays open. Test: `failed_spawn_releases_number` (Task 6) and `launcher shows the error and stays open` (Task 11).
3. **`profiles.json` is corrupt or from a newer Marshell.** Detected profiles must still load, and the file must never be overwritten. Test: `corrupt_profiles_file_falls_back_to_detected` (Task 5).
4. **Typing `exit` and closing race each other**, or a double close or a stale tab id. DELETE must stay idempotent and never panic. Test: `close_twice_is_ok_and_unknown_tab_404` (Task 6).
5. **The events socket drops** (core restart, sleep). The client must reconnect and replace its list with the next snapshot, never merge with a stale one. Test: `EventSocket replaces the list on reconnect` (Task 9).

---

## File map

**Rust (protocol)**
- Modify `crates/protocol/src/api.rs` to add the session, profile and project types and to extend `CreateSessionRequest`.

**Rust (core)**
- Create `crates/core/src/registry.rs`: session metadata, numbering, order, recently closed, and a watch channel. Pure, with no PTY.
- Create `crates/core/src/profiles/mod.rs`, `detect.rs` and `store.rs`: detection (pure parsers and probes), argv building, and persistence.
- Create `crates/core/src/projects.rs`: the recent projects list and the git branch read from `.git/HEAD`.
- Modify `crates/core/src/pty/session.rs` to add an `on_exit` callback to `SpawnSpec`.
- Modify `crates/core/src/server/mod.rs`: `Inner` gains a `registry`, `profiles` and `home`; register the routes.
- Modify `crates/core/src/server/sessions.rs` to add the create, list, rename, reorder, close and reopen handlers.
- Create `crates/core/src/server/events.rs`: the `/v1/events` WebSocket.
- Create `crates/core/src/server/profiles.rs` and `projects.rs`: their handlers.
- Delete `crates/core/src/shell.rs`, moving `default_shell` and `on_path` into `profiles/detect.rs`.
- Tests: unit tests in each new module, plus `crates/core/tests/server.rs` (HTTP) and `crates/core/tests/events.rs` (new).

**Frontend**
- Create `src/lib/api/client.ts`: `api<T>(endpoint, method, path, body?)`, a fetch helper with the token.
- Create `src/lib/api/sessions.ts`, `profiles.ts` and `projects.ts`: typed calls.
- Create `src/lib/api/events.ts`: `EventSocket`, which reconnects with backoff.
- Create `src/features/sessions/toRow.ts`: `SessionInfo` → `RowModel`.
- Create `src/features/sessions/useSessions.ts`: the hook that owns the list, the selection and the endpoint.
- Create `src/features/sessions/LiveApp.tsx`: mounts `MainWindow` and wires the keys, launcher, close and reopen, and rename.
- Modify `src/App.tsx` to render `LiveApp`.
- Modify `src/features/window/MainWindow.tsx` and `Pane.tsx` to add the optional `renderTerminal?: (row: RowModel) => ReactNode` and `onRename`/`onReorder` callbacks.
- Modify `src/features/window/Sidebar.tsx` for drag reorder, inline F2 rename and Alt+Shift+↑/↓ as the keyboard alternative.
- Modify `src/features/launcher/Launcher.tsx` to add the props `chips?: boolean` (default true), `programs?: LaunchProgram[]`, `error?: string` and `busy?: boolean`.
- Create `src/features/settings/ProfilesPage.tsx`: list and edit profiles.
- Modify `src/features/terminal/TerminalView.tsx`: `onNewShell` becomes optional, and the exited state says "Session ended."
- Remove `src/lib/api/session.ts`, which `sessions.ts` replaces.

---

### Task 1: Protocol types

**Files:**
- Modify: `crates/protocol/src/api.rs`
- Generated: `src/generated/*.ts` (via the existing export test)

**Interfaces — Produces (all `#[derive(Debug, Clone, Serialize, Deserialize, TS)] #[ts(export)]`):**
```rust
pub enum SessionState { Running, Exited { code: i32 } }          // #[serde(tag = "kind", rename_all = "snake_case")]
pub struct SessionInfo { pub tab_id: String, pub number: u32, pub name: String,
    pub agent: Option<String>,   // "claude" | "codex" | "antigravity"; None = plain shell
    pub profile_id: String, pub cwd: String, pub state: SessionState }
pub struct SessionsSnapshot { pub sessions: Vec<SessionInfo>, pub closed: Vec<SessionInfo> } // ordered; closed newest first
pub struct CreateSessionRequest { shell: Option<Vec<String>>, cwd: Option<String>, cols: u16, rows: u16,
    pub profile_id: Option<String>, pub agent: Option<String>, pub name: Option<String>, pub prompt: Option<String> } // all new fields optional, skip-if-none
pub struct CreateSessionResponse { pub tab_id: String, pub number: u32 }
pub struct RenameRequest { pub name: String }
pub struct ReorderRequest { pub tab_ids: Vec<String> }
pub enum ShellFamily { PowerShell, Cmd, Posix, Wsl }                // rename_all = "snake_case"
pub struct Profile { pub id: String, pub name: String, pub command: Vec<String>, pub family: ShellFamily,
    pub cwd: Option<String>, pub env: Vec<(String, String)>, pub detected: bool }
pub struct ProfilesDoc { pub profiles: Vec<Profile>, pub default_id: String }   // GET/PUT /v1/profiles body
pub struct ProjectInfo { pub path: String, pub name: String, pub branch: Option<String>, pub last_used_ms: u64 }
```

- [ ] **Step 1: Write failing serde tests in `api.rs` `mod tests`**
  - `session_state_serializes_tagged`: `serde_json::to_string(&SessionState::Exited{code:2})` == `{"kind":"exited","code":2}`.
  - `create_request_old_shape_still_parses`: `{"cols":80,"rows":24}` parses with every new field `None`.
- [ ] **Step 2: Run** `cargo test -p marshell-protocol`. Expected: FAIL, because the types are undefined.
- [ ] **Step 3: Add the types above.** Doc-comment each field in the existing style.
- [ ] **Step 4: Run** `cargo test -p marshell-protocol`, then `pnpm check:generated`. Expected: the tests PASS. `check:generated` reports new files until they are committed; regenerate with the export test as the existing workflow does, and confirm `src/generated/SessionInfo.ts` exists.
- [ ] **Step 5: Commit** `feat(protocol): session list, profile and project types`.

### Task 2: Session registry

**Files:**
- Create: `crates/core/src/registry.rs` (+ `pub mod registry;` in `lib.rs`)

**Interfaces — Produces:**
```rust
pub struct Entry { pub info: SessionInfo, pub spec: Relaunch }        // spec = how to reopen
#[derive(Clone)] pub struct Relaunch { pub profile_id: String, pub agent: Option<String>, pub cwd: String, pub name: String }
pub struct Registry { /* Mutex<State>, watch::Sender<SessionsSnapshot> */ }
impl Registry {
  pub fn new() -> Self;
  pub fn reserve(&self, tab_id: &str, relaunch: Relaunch) -> u32;   // lowest free number; row appears Running
  pub fn release(&self, tab_id: &str);                              // failed spawn: drop row, free number, no closed entry
  pub fn mark_exited(&self, tab_id: &str, code: i32);
  pub fn rename(&self, tab_id: &str, name: &str) -> bool;           // trims; empty → false
  pub fn reorder(&self, tab_ids: &[String]) -> bool;                // must be a permutation of open ids
  pub fn close(&self, tab_id: &str) -> Option<Relaunch>;            // moves to closed (max 10), frees number
  pub fn take_closed(&self, tab_id: Option<&str>) -> Option<Relaunch>; // None = most recent
  pub fn snapshot(&self) -> SessionsSnapshot;
  pub fn subscribe(&self) -> tokio::sync::watch::Receiver<SessionsSnapshot>;
  pub fn clear(&self);                                              // core shutdown
}
```
The default name is the agent's display name ("Claude Code", "Codex", "Antigravity") or the profile name, followed by ` <number>` only when another open session has the same name.

- [ ] **Step 1: Write failing unit tests** in `registry.rs`:
  - `numbers_are_lowest_free`: reserve 1–6, close 4, then reserve again → 4.
  - `reorder_never_renumbers`: after `reorder` the order changes and the numbers do not.
  - `reorder_rejects_non_permutation`: a missing or extra id → false, unchanged.
  - `registry_marks_exit_and_keeps_number`: the state becomes `Exited{code}` and the number is kept.
  - `release_frees_number_without_closed_entry`.
  - `closed_list_caps_at_ten_newest_first`.
  - `rename_trims_and_rejects_empty`.
  - `subscribe_sees_every_change`: `rx.has_changed()` is true after `reserve`, and `borrow()` has one session.
- [ ] **Step 2: Run** `cargo test -p marshell-core registry`. Expected: FAIL (it does not compile).
- [ ] **Step 3: Implement.** Use one `Mutex<State { open: Vec<Entry>, closed: VecDeque<Entry> }>`. Call `tx.send_replace(snapshot)` after each mutation while still holding the lock, so the order is consistent.
- [ ] **Step 4: Run** `cargo test -p marshell-core registry`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): session registry with stable lowest-free numbers`.

### Task 3: Exit callback on `Session`

**Files:**
- Modify: `crates/core/src/pty/session.rs` and every `SpawnSpec { .. }` literal (tests included)
- Test: `crates/core/tests/session.rs`

**Interfaces — Produces:** `SpawnSpec.on_exit: Option<Box<dyn FnOnce(i32) + Send>>`, called once from `on_exit(code)` after the exit is stored.

- [ ] **Step 1: Write failing test** `on_exit_callback_gets_the_code`. Spawn a shell that runs `exit 3` (Windows `cmd /c exit 3`; unix `sh -c "exit 3"`), with a callback that sends the code down a `std::sync::mpsc` channel. Assert `recv_timeout(5s) == Ok(3)`.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test session on_exit`. Expected: FAIL (no field).
- [ ] **Step 3: Implement.** Store the callback in a `Mutex<Option<..>>` on `Session` and `take()` it in `on_exit`. `SpawnSpec` cannot derive `Debug`/`Clone` any more, so drop those derives if they are present.
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: PASS (the whole crate, since literals changed).
- [ ] **Step 5: Commit** `feat(core): sessions report their exit to a callback`.

### Task 4: Shell detection and agent argv

**Files:**
- Create: `crates/core/src/profiles/mod.rs` and `crates/core/src/profiles/detect.rs`
- Delete: `crates/core/src/shell.rs`. Move `default_shell` and `on_path` into `detect.rs` and update `lib.rs` and the callers.

**Interfaces — Produces:**
```rust
pub fn parse_wsl_list(stdout: &[u8]) -> Vec<String>;    // UTF-16LE (with or without BOM) or UTF-8; trims NULs, \r, blanks
pub fn git_bash_path(roots: &[PathBuf]) -> Option<PathBuf>; // first <root>\Git\bin\bash.exe that exists
pub fn detect_profiles(probe: &Probe) -> Vec<Profile>;  // ids: "pwsh", "powershell", "cmd", "git-bash", "wsl-<distro lowercased>", unix "login-shell"
pub struct Probe { pub on_path: fn(&str) -> bool, pub wsl_list: fn() -> Vec<u8>, pub program_roots: Vec<PathBuf>, pub login_shell: Option<String> }
impl Probe { pub fn system() -> Self }                  // the real probes; wsl_list runs `wsl.exe -l -q`, empty on any error, 2 s timeout
pub fn agent_command(agent: &str) -> Option<&'static str>; // claude→"claude", codex→"codex", antigravity→"agy"; else None
pub fn launch_argv(profile: &Profile, agent: Option<&str>, prompt: Option<&str>) -> Result<Vec<String>, String>;
```
`launch_argv` rules:
- No agent: return `profile.command`.
- Agent: the command line is `cmd = <agent_command>` plus the prompt. Claude and Codex take it as one positional argument; agy takes `-i <prompt>`. The rest depends on the shell family:
  - PowerShell: `command + ["-NoExit", "-Command", "& <cmd> <single-quoted args>"]`, where `'` becomes `''`.
  - Cmd: `command + ["/k", "<cmd> \"<prompt>\""]`. A prompt containing `"`, `%`, `^`, `&`, `|`, `<` or `>` → `Err("This prompt has characters cmd can't pass safely. Use PowerShell or start without a prompt.")`.
  - Posix: `command + ["-c", "<cmd> '<prompt>'; exec \"$SHELL\" -l"]` (POSIX single-quote escaping `'\''`). Git Bash uses its own path instead of `$SHELL`: `exec <command[0]> -l`.
  - Wsl: `command + ["--", "bash", "-lc", "<same posix line with exec bash -l>"]`.

- [ ] **Step 1: Write failing unit tests** in `detect.rs`:
  - `parses_utf16_wsl_list_with_bom`: bytes of `"\u{feff}Ubuntu\r\nDebian\r\n"` as UTF-16LE → `["Ubuntu","Debian"]`.
  - `parses_empty_and_utf8_wsl_list`: an empty input → `[]`; `b"Ubuntu\n"` → `["Ubuntu"]`.
  - `detects_windows_set` (`#[cfg(windows)]`): a probe where everything is on PATH, the WSL list has Ubuntu, and a temp root holds `Git\bin\bash.exe` → ids `["pwsh","powershell","cmd","git-bash","wsl-ubuntu"]` in that order, all `detected: true`.
  - `missing_pwsh_is_skipped` (`#[cfg(windows)]`).
  - `powershell_agent_argv_quotes_prompt`: a prompt `it's` → the last arg is `& claude 'it''s'`.
  - `agy_prompt_uses_dash_i`.
  - `cmd_rejects_unsafe_prompt`.
  - `posix_agent_argv_returns_to_shell`.
  - `no_agent_returns_profile_command`.
- [ ] **Step 2: Run** `cargo test -p marshell-core profiles`. Expected: FAIL.
- [ ] **Step 3: Implement.** Pure functions; `Probe::system()` is the only code that touches the OS. Decode UTF-16 with `char::decode_utf16` when the length is even and every odd byte is 0 for ASCII text, or when a BOM is present; otherwise decode as UTF-8 lossily.
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): detect shells and build agent launch commands`.

### Task 5: Profile persistence and endpoints

**Files:**
- Create: `crates/core/src/profiles/store.rs` and `crates/core/src/server/profiles.rs`
- Modify: `crates/core/src/server/mod.rs`: `Inner` gets `home: PathBuf`, `registry: Registry`, `profiles: Mutex<ProfilesDoc>`. Add the routes `GET /v1/profiles` and `PUT /v1/profiles`. `AppState::new` takes `home` and `detected: Vec<Profile>`; `start` calls `Probe::system()` inside `spawn_blocking`.
- Test: `crates/core/tests/server.rs`

**Interfaces — Produces:**
```rust
// store.rs: file = home/"profiles.json"; FileDoc { version: u32 (=1), profiles: Vec<Profile> (edited or custom only), default_id: Option<String> }
pub fn load(home: &Path, detected: Vec<Profile>) -> ProfilesDoc; // merge: file entry with a detected id replaces it; custom ids appended; default_id falls back to first
pub fn save(home: &Path, doc: &ProfilesDoc, detected: &[Profile]) -> anyhow::Result<()>; // writes only entries that differ from detection or are custom
pub fn validate(doc: &ProfilesDoc) -> Result<(), String>; // unique non-empty ids and names, non-empty command, default_id present
```
`PUT /v1/profiles` validates and saves, then replaces the in-memory doc and returns it. A failed validation → 400 with the message. `GET` returns the doc.

- [ ] **Step 1: Write failing tests.**
  - Unit tests in `store.rs`:
    - `load_without_file_returns_detected`.
    - `edited_detected_profile_wins_by_id`.
    - `save_skips_unchanged_detected`: the file holds only the custom entry.
    - `corrupt_profiles_file_falls_back_to_detected`: write `{nope` → detected only, and the file bytes are unchanged afterwards.
    - `validate_rejects_duplicate_ids_and_empty_command`.
  - Server tests in `tests/server.rs`:
    - `profiles_get_then_put_roundtrip`: PUT adds a custom profile `{"id":"my-pwsh",...}`; GET shows it; the file exists in the temp home.
    - `profiles_put_invalid_is_400`.
- [ ] **Step 2: Run** `cargo test -p marshell-core profiles`. Expected: FAIL.
- [ ] **Step 3: Implement.** Use `store::read`/`store::write` with `Versioned`. A newer-version file is already refused by `store.rs`, which surfaces as `None` → fall back to detected and never save over it. To keep that true, `save` refuses when `store::read` failed for a reason other than "missing".
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): profiles persisted in profiles.json with GET/PUT`.

### Task 6: Session endpoints on the registry

**Files:**
- Modify: `crates/core/src/server/sessions.rs` and `server/mod.rs` (routes)
- Test: `crates/core/tests/server.rs`

**Routes:**
- `GET /v1/sessions` → `SessionsSnapshot`.
- `POST /v1/sessions` (create).
- `PATCH /v1/sessions/{tab}` with `RenameRequest` → 204, 400 when empty, 404 when unknown.
- `POST /v1/sessions/reorder` with `ReorderRequest` → 204, 400.
- `DELETE /v1/sessions/{tab}` → 204, **also 204 when the tab is already closed or unknown** (idempotent): `registry.close`, remove from `sessions`, then `session.kill()`.
- `POST /v1/sessions/reopen` with optional body `{ "tab_id": "..." }` → `CreateSessionResponse` or 404 when none.

**Create flow:**
1. A request with an explicit `shell` argv keeps today's behaviour and uses profile `"custom"`.
2. Otherwise: profile = `profile_id`, else the default.
3. Agent ids outside `claude`/`codex`/`antigravity` → 400 "Unknown agent".
4. argv = `launch_argv`; an `Err` → 400 with its text.
5. cwd = request, else profile, else home.
6. `number = registry.reserve(...)`; env adds `MARSHELL_SESSION_NUMBER` and `MARSHELL_AGENT` = agent or `"shell"`, merged after the profile env.
7. `on_exit` → `registry.mark_exited`, holding a `Weak`/clone of the registry handle.
8. A failed spawn → `registry.release(tab)`, then return 500 as today.
9. The default name per Task 2.

- [ ] **Step 1: Write failing server tests:**
  - `create_returns_number_and_lists_session`: two creates → numbers 1 and 2; GET shows both in order.
  - `failed_spawn_releases_number`: `shell: ["definitely-not-a-program-xyz"]` → 500, then a valid create gets number 1.
  - `rename_and_reorder`.
  - `close_twice_is_ok_and_unknown_tab_404`: DELETE twice → 204 both times; PATCH on the closed tab → 404.
  - `reopen_restores_profile_and_name`: create, rename to "api", close, reopen → a new tab id with name "api" and the same profile, and `closed` is empty.
  - `exited_session_is_ended_in_snapshot`: create with `shell` argv exiting 0 (`cmd /c exit 0` or `sh -c "exit 0"`); poll GET up to 5 s for `state.kind == "exited"`.
  - `unknown_agent_is_400`.
  - `session_env_has_number`: on Windows run `cmd /c echo %MARSHELL_SESSION_NUMBER%`; read the PTY via the existing ws test helper and expect `1`. Use `#[cfg(windows)]` plus a unix twin.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test server`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement** the handlers.
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): session list, rename, reorder, close and reopen endpoints`.

### Task 7: `/v1/events` WebSocket

**Files:**
- Create: `crates/core/src/server/events.rs`
- Modify: `server/mod.rs` (route `GET /v1/events`)
- Test: `crates/core/tests/events.rs`

**Protocol:**
- The token arrives in a `token.<t>` subprotocol, as for the PTY socket; `auth_guard` already accepts it.
- The server echoes `WS_SUBPROTOCOL` as `pty_ws.rs` does.
- Every message is a text frame `{"type":"sessions","data":SessionsSnapshot}`: one on connect, then one per `watch` change.
- No client messages are expected. The server ignores text and closes on close.

- [ ] **Step 1: Write failing test** `events_send_snapshot_on_connect_and_change`, using tokio-tungstenite as in `tests/pty_ws.rs`. Connect → first message with 0 sessions. POST a create → the next message has 1 session with number 1.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test events`. Expected: FAIL (404 upgrade).
- [ ] **Step 3: Implement.** Loop with `tokio::select!` on `rx.changed()` and `socket.recv()`; send `rx.borrow_and_update().clone()`.
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): push the session list over /v1/events`.

### Task 8: Recent projects

**Files:**
- Create: `crates/core/src/projects.rs` and `crates/core/src/server/projects.rs`
- Modify: `sessions.rs` (record on successful create) and `server/mod.rs` (`GET /v1/projects`)

**Interfaces — Produces:**
```rust
pub fn git_branch(dir: &Path) -> Option<String>;        // walks up to find .git (dir or file "gitdir: …"); HEAD "ref: refs/heads/x" → "x"; detached → first 7 chars of the sha
pub fn record(home: &Path, cwd: &Path, now_ms: u64) -> anyhow::Result<()>; // projects.json { version: 1, projects: [{path, last_used_ms}] }, newest first, max 30, dedup by path (case-insensitive on Windows)
pub fn list(home: &Path) -> Vec<ProjectInfo>;           // name = last path component; branch via git_branch; drops paths that no longer exist
```
The home directory itself is never recorded as a project.

- [ ] **Step 1: Write failing unit tests:**
  - `branch_from_head_ref`.
  - `branch_detached_is_short_sha`.
  - `branch_through_gitdir_file` (a worktree).
  - `no_git_is_none`.
  - `record_dedups_and_caps_at_30`.
  - `list_drops_missing_paths`.
  - `home_is_not_recorded`.
- [ ] **Step 2: Run** `cargo test -p marshell-core projects`. Expected: FAIL.
- [ ] **Step 3: Implement.** In `sessions::create`, record the project after a successful spawn, inside the existing `spawn_blocking` closure. A record error is logged with `tracing::warn!`, never fatal.
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): recent projects with git branch`.

### Task 9: Frontend API client and events

**Files:**
- Create: `src/lib/api/client.ts`, `sessions.ts`, `profiles.ts`, `projects.ts` and `events.ts` (+ `*.test.ts`)
- Delete: `src/lib/api/session.ts`

**Interfaces — Produces:**
```ts
export class ApiError extends Error { status: number }            // message = response text
export function api<T>(ep: Endpoint, method: "GET"|"POST"|"PATCH"|"PUT"|"DELETE", path: string, body?: unknown): Promise<T>; // 204 → undefined
export const sessions = { list, create, rename, reorder, close, reopen };   // thin wrappers over api() with the generated types
export const profiles = { get, put };
export const projects = { list };
export class EventSocket { constructor(ep: Endpoint, onSnapshot: (s: SessionsSnapshot) => void); close(): void } // backoff 250 ms ×2 up to 5 s, reset on open
export function getEndpoint(): Promise<Endpoint>;                  // invoke("core_endpoint"), cached; resetEndpoint() after a core restart
```

- [ ] **Step 1: Write failing Vitest tests:**
  - `api sends bearer token and JSON` (with a stubbed `fetch`).
  - `api throws ApiError with response text on 400`.
  - `api returns undefined on 204`.
  - `EventSocket delivers snapshot messages`, using a fake `WebSocket` class like the one in `ptySocket.test.ts`.
  - `EventSocket replaces the list on reconnect`: close → reconnect after the timer → the new snapshot is delivered and nothing is merged.
  - `EventSocket ignores unknown message types`.
- [ ] **Step 2: Run** `pnpm test src/lib/api`. Expected: FAIL.
- [ ] **Step 3: Implement.** Copy the subprotocol and backoff pattern from `ptySocket.ts`.
- [ ] **Step 4: Run** `pnpm test src/lib/api`. Expected: PASS. `App.tsx` still imports `session.ts` until Task 10, so delete `session.ts` in Task 10.
- [ ] **Step 5: Commit** `feat(ui): typed API client and events socket`.

### Task 10: Live sessions in `MainWindow`

**Files:**
- Create: `src/features/sessions/toRow.ts`, `useSessions.ts` and `LiveApp.tsx` (+ tests)
- Modify: `src/App.tsx`, `MainWindow.tsx`, `Pane.tsx` and `TerminalView.tsx`. Delete `src/lib/api/session.ts`.

**Interfaces — Produces:**
```ts
export function toRow(info: SessionInfo, profileNames: Record<string, string>): RowModel;
// agent → SessionKind: "claude"|"codex"|"antigravity" as is; shell profiles by family: power_shell → "powershell", cmd → "cmd",
// posix (git-bash id) → "gitbash", posix → "bash", wsl → "wsl". status: exited → "ended"; agent → "limited"; shell → "idle".
// phrase: exited → `Exited with code ${code}`; agent running → "Status arrives with hooks"; shell → "".
// project: last segment of cwd.
export function useSessions(): { endpoint: Endpoint | null; rows: RowModel[]; infos: SessionInfo[]; closed: SessionInfo[];
  selectedId: string | undefined; select(id: string): void; error: string | null };
// MainWindow/Pane prop:
renderTerminal?: (row: RowModel) => ReactNode;   // when set, replaces ScriptedTerminal at MainWindow.tsx:229 and Pane.tsx:109
```
`useSessions` behaviour:
- Selection follows the newest session on create, and the next row (or the previous one at the end) when the selected session closes.
- On `core-restarted` (via `onCoreEvent`), call `resetEndpoint()`, clear the rows and reconnect.

`LiveApp` behaviour:
- Renders `MainWindow` with `width`/`height` from the window size (a `ResizeObserver` on the root) and `layout="default"`.
- `renderTerminal = row => <TerminalView key={row.id} endpoint tabId={row.id} />`.
- An empty list shows the launcher open.
- `TerminalView` exit text: "Session ended. Close it with Ctrl+Shift+W." `onNewShell` is removed; a core restart opens the launcher.

- [ ] **Step 1: Write failing tests:**
  - `toRow.test.ts`: `maps exited to ended with code phrase`, `agent session is limited, never working`, `git bash profile maps to gitbash mark`.
  - `useSessions.test.tsx`: `selects the new session`, `selection moves to the neighbour when the selected session closes`, `core restart clears the list`. Stub `EventSocket` with `vi.mock`.
  - `MainWindow.test.tsx`: `renderTerminal replaces the scripted terminal`.
- [ ] **Step 2: Run** `pnpm test src/features/sessions src/features/window/MainWindow.test.tsx`. Expected: FAIL.
- [ ] **Step 3: Implement.** Keep `MainWindow` presentational: its new callbacks are optional, so the stories keep working.
- [ ] **Step 4: Run** `pnpm test`, then `pnpm build`. Expected: PASS and the build succeeds. The stories' snapshot tests are unchanged.
- [ ] **Step 5: Commit** `feat(ui): mount MainWindow with the core's live session list`.

### Task 11: Launcher wired to the core

**Files:**
- Modify: `src/features/launcher/Launcher.tsx` (+ test) and `src/features/sessions/LiveApp.tsx`
- Create: `src/features/sessions/launchPrograms.ts` (+ test)

**Interfaces — Produces:**
```ts
export function livePrograms(profiles: Profile[]): LaunchProgram[]; // agents claude, codex, antigravity, then one program per profile (id `profile:<id>`), kind by toRow's family mapping
export function launchRequest(choice: LauncherChoice, prompt: string, programs: LaunchProgram[], profiles: ProfilesDoc,
  projects: Project[], size: { cols: number; rows: number }): CreateSessionRequest;
// agent program → { agent, profile_id: profiles.default_id }; profile program → { profile_id }; cwd = chosen project path; prompt only for agents and only if non-empty
```
`Launcher` props: `chips?: boolean` (false hides mode, pace and worktree), `programs?: LaunchProgram[]` (defaults to `LAUNCH_PROGRAMS`), `error?: string` (shown under Start with `role="alert"`), `busy?: boolean` (Start disabled, label unchanged, no spinner under 150 ms).

`LiveApp` flow:
1. Ctrl+Shift+T (`launcher` command) opens the launcher with projects from `projects.list()` mapped to the launcher's `Project` (`id = path`).
2. Start calls `sessions.create`, with cols and rows from the last measured terminal size, default 120×30.
3. On success: close the launcher, select the new tab (the snapshot brings the row), and show `startingText(...)` in the terminal area until the first output, which `TerminalView` already handles if a `placeholder` prop is added.
4. On `ApiError`: keep the launcher open with `error = e.message`.

- [ ] **Step 1: Write failing tests:**
  - `launchPrograms.test.ts`: `agents come first and only the three`, `agent request uses the default profile`, `shell request carries no prompt`.
  - `Launcher.test.tsx`: `chips hidden when chips is false`, `launcher shows the error and stays open`.
- [ ] **Step 2: Run** `pnpm test src/features/launcher src/features/sessions`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): launcher starts shells and agents through the core`.

### Task 12: Close, reopen, rename, reorder, go to session

**Files:**
- Modify: `src/features/sessions/LiveApp.tsx`, `src/features/window/Sidebar.tsx`, `SessionHeader.tsx` (+ tests)
- Create: `src/features/sessions/CloseBar.tsx` (+ test) and `src/features/sessions/Toast.tsx` if no shared toast exists (check `src/components` first and reuse what is there)

**Behaviour:**
- **Ctrl+Shift+W, a shell or an exited session:** `sessions.close` at once, then the toast "Closed <name>." with [Reopen] for 10 s. Reopen calls `sessions.reopen({tab_id})`.
- **Ctrl+Shift+W, a running agent session:** `CloseBar` in the header, `role="alert"`, with the text "<Agent name> may be mid-task in <project>." and [Cancel] (focused) and [Close now]. Escape cancels. Focus returns to the terminal afterwards.
- **Ctrl+Shift+Alt+T:** `sessions.reopen()` (most recent). Nothing closed → toast "Nothing to reopen."
- **F2:** the selected row's name becomes an input, pre-selected. Enter → `sessions.rename`; Escape or blur cancels; an empty value is refused with the original kept.
- **Reorder:**
  - Drag a row in the sidebar → `sessions.reorder(newOrder)`. Use native HTML5 drag and drop; no new package.
  - Keyboard alternative on a focused row: Alt+Shift+↑/↓. Add the commands `move-up` and `move-down` to `COMMANDS` with `{ code: "ArrowUp"/"ArrowDown", alt: true }`. Check `Binding` supports arrows plus alt, and rule otherwise.
  - The order on screen follows the next snapshot; nothing optimistic.
- **Ctrl+Shift+1–9:** select the session whose **number** is N (not its position). No such session → nothing.

- [ ] **Step 1: Write failing tests:**
  - `LiveApp.test.tsx`: `close shell asks nothing and offers reopen`, `close agent shows bar with cancel focused`, `escape on close bar keeps the session`, `ctrl+shift+3 selects number 3 not third row`, `reopen with nothing closed says so`.
  - `Sidebar.test.tsx`: `F2 edits name inline and enter renames`, `empty rename keeps the old name`, `alt+shift+arrow moves the row`.
- [ ] **Step 2: Run** `pnpm test src/features/sessions src/features/window`. Expected: FAIL.
- [ ] **Step 3: Implement.** Follow `frontend-design` for the bar and toast look, using the existing tokens in `src/styles/`.
- [ ] **Step 4: Run** `pnpm test`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): close with reopen, F2 rename, reorder and go to session`.

### Task 13: Profiles settings page

**Files:**
- Create: `src/features/settings/ProfilesPage.tsx` and `ProfilesPage.css` (+ test and a story)
- Modify: `LiveApp.tsx`. Open it from the palette command `profiles` ("Shell profiles…", group Tools, unbound) until 2f wires the full settings window.

**Behaviour:**
- On the left, a list of profiles; a detected one carries a "Detected" badge.
- On the right, a form: Name, Command (one text field, split on spaces with `"…"` grouping, shown back joined), Folder (text plus "Browse…" via the Tauri dialog if already available, otherwise text only), and Env (key/value rows with add and remove).
- "Make default" button.
- "Add profile" duplicates the selected one with the id `custom-<n>`.
- "Delete" is enabled only for custom profiles. "Reset" restores a detected profile to its detected values.
- Saving reuses the existing `ChangeBar` (Save / Discard) and calls `profiles.put`. A 400 shows the message in the bar. Saving never auto-runs.

- [ ] **Step 1: Write failing tests:**
  - `edits a detected profile and saves the doc`.
  - `delete is disabled for detected profiles`.
  - `server validation error shows in the change bar`.
  - `command field round-trips quoted args`: `"C:\Program Files\Git\bin\bash.exe" -l` ↔ `["C:\\Program Files\\Git\\bin\\bash.exe","-l"]`.
- [ ] **Step 2: Run** `pnpm test src/features/settings/ProfilesPage`. Expected: FAIL.
- [ ] **Step 3: Implement** with `FieldRow` and `ChangeBar`. `frontend-design` decides the look.
- [ ] **Step 4: Run** `pnpm test`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): shell profiles settings page`.

### Task 14: PLAN update and manual verification

**Files:**
- Modify: `docs/PLAN.md` §9 phase 2, the slices line at line 747.

- [ ] **Step 1:** Edit the slices line so that 2b reads "tabs, sidebar, profiles and launcher (agents run inside a shell profile; status limited until phase 3)" and 2e adds "the sidebar shell panel, project grouping and focus mode".
- [ ] **Step 2:** Run `cargo test --workspace`, `pnpm test`, `pnpm build` and `pnpm check:generated`. Expected: everything passes; save the outputs to the workspace and read the tails.
- [ ] **Step 3:** Manual check with `pnpm tauri dev` (needs the user's desktop):
  - Five tabs across pwsh, cmd and Git Bash.
  - A Claude Code tab from the launcher with a prompt.
  - Close and reopen.
  - Rename.
  - Reorder.
  - Ctrl+Shift+2.
  - Edit a profile's env and confirm with `echo $env:FOO`.
  - Typing `exit` turns the row Ended.

  Record the results in the PR body. If this cannot run here, say so in the PR.
- [ ] **Step 4: Commit** `docs: slice 2b scope; shell panel and grouping move to 2e`.
