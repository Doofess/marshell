# Phase 3a: Hook Bridge and Spikes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A real Claude Code session's hooks and status line reach the Marshell core through the `marshell-hook` bridge, a held permission request can be answered or released through the API, and spikes S10, S4 and S9 settle the open bridge questions.

**Architecture:**
- **Bridge (`crates/hook`, package and binary `marshell-hook`).** A blocking program with no tokio. It reads stdin, finds the core through `run/endpoint.json`, and posts to `POST /v1/hook`. It obeys the failure rule: exit 0, empty stdout, and one line in `logs/bridge.log`. The `statusline` role also runs the user's original command and prints its output unchanged. The `record` role also saves stdin as a fixture.
- **Core.** `POST /v1/hook` keeps the last 200 raw events per tab in memory, readable through `GET /v1/hook/events`. The `permission` role holds the request until a decision, a release, the hold expiring, the bridge disconnecting, or the person typing in that tab. Claude's answer JSON is built in a Claude-scoped module. On launch the app copies the bridge into `~/.marshell/bin`, renaming around a running, locked exe.
- **Spikes.** A live check against a real Claude Code, through a scratch folder's `.claude/settings.local.json`, answers S10, S4 and S9 and records the scrubbed fixtures that 3c's normaliser will test against.

**Tech Stack:** Rust 2024 (toolchain 1.98), axum 0.8 and tokio in the core, `ureq` 3 (blocking, no TLS) in the bridge. Node for the scrub script, with a Vitest test. Tests: `cargo test --workspace` and `pnpm test`.

**Spec:**
- `docs/PLAN.md` §5 "Hook bridge protocol" (around line 572), §3 IPC routes (around line 513), §4 data model (around line 548), §8 spikes S4, S9 and S10 (around line 733), and §9 phase 3 "Slices" and "Slice 3a decisions (2026-10-18)".
- Claude Code docs, checked 2026-10-18 (hooks, hooks-guide, statusline, permissions). These are the facts the plan relies on:
  - `PermissionRequest` stdin carries `tool_name`, `tool_input` and an optional `permission_suggestions` array of `{type:"addRules", rules:[{toolName, ruleContent?}], behavior, destination}`.
  - The allow answer is `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`. `updatedPermissions` (an array of addRules entries) is allowed only with allow. `message` and `interrupt` are allowed only with deny.
  - `destination` is `session`, `localSettings`, `projectSettings` or `userSettings`.
  - Exit 0 with empty output on `PermissionRequest` is not documented; S9 settles it.
  - A hook `command` with `args` runs with no shell, and on Windows the command must be a real `.exe`. Without `args`, it runs in Git Bash, or in PowerShell when Git Bash is missing. `shell: "bash" | "powershell"` picks one. `CLAUDE_CODE_GIT_BASH_PATH` points at Git Bash. Use forward slashes and quote paths.
  - Hook `timeout` is in seconds, with a default of 600 for command hooks.
  - `statusLine` is `{type:"command", command, padding?, refreshInterval?}`. It runs in Git Bash, or PowerShell without it. It is debounced at 300 ms, and an in-flight run is cancelled by the next update. Output shows only on exit 0. Its stdin includes `version`; no hook payload does.
  - Workspace trust: the docs disagree on whether settings-file hooks wait for trust. The spike records what happens.

## Global Constraints

- **Bridge invocation.**
  - The form is `"<home>/.marshell/bin/marshell-hook(.exe)" <role> --agent claude --tag mshl1`, with forward slashes and always quoted.
  - Roles are exactly `event`, `context`, `permission`, `statusline` and `record`.
  - An unknown role or a missing `--agent` counts as a failure, which follows the failure rule.
- **Discovery.**
  - `MARSHELL_HOME` if set, else `<user home>/.marshell` (`std::env::home_dir`). Then `<home>/run/endpoint.json` with `{version, port, token, platform}`.
  - A missing file, an unreadable file or a refused connection means exit 0 at once with no output. There is no pid check.
- **Request.**
  - `POST http://127.0.0.1:<port>/v1/hook`.
  - Headers: `Authorization: Bearer <token>`, `X-Marshell-Agent`, `X-Marshell-Role`, and `X-Marshell-Version` (the bridge's `CARGO_PKG_VERSION`). `X-Marshell-Tab` is sent only when `MARSHELL_TAB_ID` is set.
  - The body is stdin, capped at 1 MiB (1,048,576 bytes). Over the cap, nothing is posted and a log line is written.
- **Timeouts.**
  - Connect: 50 ms on every role.
  - Whole request: `event` and `record` 400 ms; `context` 150 ms; `statusline` forward 100 ms; `permission` 590 s, just under the hook's 600 s.
  - The core's hold defaults to 120 s. `MARSHELL_PERMISSION_HOLD_SECS` (1 to 590) overrides it, for the spike; it is read once in `server::start`.
- **Output.**
  - `event`, `statusline` forward and `record`: the core's response is ignored.
  - `context`: the response body is printed only on 200 with a non-empty body; the core returns 204 in 3a.
  - `permission`: 200 means the body is printed verbatim; 204 means release, so no output.
- **Failure rule.** On any error the bridge exits 0, prints nothing, never waits past its budget, and appends one line to `<home>/logs/bridge.log`.
  - The line is `<unix_ms> <role> <message>`.
  - It never holds the token or the stdin body.
  - Before appending, a log over 1 MiB is renamed to `bridge.log.1`, replacing any old one.
  - The one exception: `statusline` still prints the original command's output when the core part fails.
- **Core limits.**
  - `POST /v1/hook` bodies are capped at 1 MiB (413 above it) and must be JSON (400 otherwise).
  - The event log holds 200 records per tab, plus 200 untabbed. A record goes under its tab only when that tab is a live session; otherwise it goes in the untabbed ring.
- **Auth.** Every new route sits behind the existing `host_guard` and `auth_guard`. `guard.rs` does not change.
- **Vendor scoping.** Claude's permission JSON lives only in `crates/core/src/agents/claude.rs`. The hold registry and routes are vendor-neutral.
- **Dependencies.** One new runtime dependency: `ureq = { version = "3.4.2", default-features = false }` in `crates/hook`. It is already in `Cargo.lock` as a dev-dependency of the core. The concrete need is a blocking loopback HTTP client with no tokio, which keeps the bridge's start-up in milliseconds (PLAN §2). It is MIT/Apache-2.0, active, and makes no network access beyond the request it is asked to make. No new npm package.
- **Never touched.** `~/.claude`, `~/.codex` and the real `~/.marshell`. The live check uses a scratch `MARSHELL_HOME` and a scratch project folder. `python ~/.claude/statusline.py` is only run.
- **Enter.** Marshell never sends Enter. In the live check, Enter is pressed only in Claude sessions started for the check.
- **Commits.** `type(scope): sentence`. Run `cargo fmt --all` before every Rust commit.

## Review Focus

1. **The bridge disconnects mid-hold.** Claude times the hook out, or the person answers in the terminal and Claude kills the hook. The pending request must leave `GET /v1/permissions` at once, and a later decision for it must get 404. A test pins this in Task 2.
2. **Git Bash vs WSL bash.** On this machine, `bash.exe` on PATH includes the WSL launcher (`%LOCALAPPDATA%\Microsoft\WindowsApps\bash.exe`). The status line chain must find Git Bash the way Claude Code does, and never the first `bash` on PATH. A unit test pins the resolution order in Task 4.
3. **A slow or broken original status line.** A user's command that exits non-zero, prints nothing, or doesn't exist must give the same visible result as without Marshell: nothing shown. It must never print Marshell's own text, and the forward thread must not delay it. Tests in Task 4.
4. **Two decisions racing.** Two `decision` posts for one rid, or a decision after release: exactly one wins, and the other gets 404. A test in Task 2.
5. **A stale `endpoint.json` after a crash.** The file names a port nothing listens on, or a port another program owns. The connection is refused, or the request gets a non-2xx answer; either way the bridge exits 0, quickly and silently. A test in Task 3 points the bridge at a port that is listening but answers 401.

---

### Task 1: Core receives hook events

**Files:**
- Create: `crates/protocol/src/hook.rs`
- Modify: `crates/protocol/src/lib.rs`
- Create: `crates/core/src/hooks/mod.rs`, `crates/core/src/hooks/log.rs`
- Create: `crates/core/src/server/hook.rs`
- Modify: `crates/core/src/server/mod.rs` (state field, routes), `crates/core/src/lib.rs`, `crates/core/src/paths.rs`
- Test: `crates/core/tests/hook.rs`

**Interfaces:**
- Produces, in `marshell_protocol::hook` (ts-rs exported, lowercase serde):
  - `enum HookRole { Event, Context, Permission, Statusline, Record }`, with `HookRole::parse(&str) -> Option<HookRole>` and `HookRole::as_str(self) -> &'static str`.
  - `enum PermissionBehavior { Allow, Deny }`.
  - `enum PermissionScope { Once, Session, Project }`, with `#[default] Once`.
  - `struct PermissionDecision { behavior: PermissionBehavior, #[serde(default)] scope: PermissionScope, message: Option<String> }`.
  - Header-name constants `HEADER_AGENT = "x-marshell-agent"`, `HEADER_TAB = "x-marshell-tab"`, `HEADER_ROLE = "x-marshell-role"` and `HEADER_VERSION = "x-marshell-version"`, and `HOOK_BODY_LIMIT: usize = 1_048_576`.
- Produces, in core:
  - `hooks::log::HookEventRecord { at_ms: u64, role: HookRole, agent: String, tab: Option<String>, bridge_version: String, body: serde_json::Value }` (Serialize).
  - `hooks::log::HookLog::push(&self, rec: HookEventRecord, live: impl Fn(&str) -> bool)` and `HookLog::recent(&self, tab: Option<&str>) -> Vec<HookEventRecord>`, oldest first.
  - `AppState::hook_log(&self) -> &HookLog`.
  - `paths::bin_dir(home)`, i.e. `bin/`. `paths::bridge_exe(home)`, i.e. `bin/marshell-hook` plus `std::env::consts::EXE_SUFFIX`. `paths::bridge_log(home)`, i.e. `logs/bridge.log`. `paths::statusline_chain(home, agent)`, i.e. `agents/<agent>/statusline-chain.json`.
  - Routes: `POST /v1/hook` and `GET /v1/hook/events?tab=<id>`.

- [ ] **Step 1: Write the failing tests** in `crates/core/tests/hook.rs`, using the `app_in` / `Host` / `Bearer` pattern of `tests/server.rs`:
  - `hook_event_is_kept_under_its_live_tab`: create a session (or use the test helper that registers one); post `{"hook_event_name":"Stop"}` with role `event`, agent `claude` and that tab. Expect 204. Then `GET /v1/hook/events?tab=<tab>` gives one record with `role == "event"`, `agent == "claude"` and `body.hook_event_name == "Stop"`.
  - `hook_event_for_unknown_tab_goes_untabbed`: tab `nope` → `GET /v1/hook/events` (no tab) holds it, `?tab=nope` is empty.
  - `hook_log_keeps_the_last_200`: 205 posts give 200 records, and the first one has the 6th body.
  - `hook_rejects_non_json_body_with_400`, `hook_rejects_unknown_role_with_400`, `hook_requires_agent_header_with_400`.
  - `hook_body_over_1_mib_gets_413`: a body of 1,048,577 bytes.
  - `hook_requires_token`: no Authorization header gives 401.
  - `context_role_answers_204_with_no_body`.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test hook`. Expected: it fails to compile (no route, no `HookRole`).
- [ ] **Step 3: Implement** the protocol types, `HookLog`, a `Mutex<HashMap<String, VecDeque<_>>>` plus an untabbed `VecDeque` (with the cap as a constant `HOOK_LOG_CAP = 200`), the paths, and `server/hook.rs`.
  - Put `DefaultBodyLimit::max(HOOK_BODY_LIMIT)` on `/v1/hook` only.
  - The handler takes the headers and `Bytes`, parses the JSON, pushes, and returns 204. The `permission` role is a 400 stub ("not yet") until Task 2.
  - `live` is "the tab is in `sessions`".
- [ ] **Step 4: Run** `cargo test -p marshell-core --test hook`, then `cargo test -p marshell-protocol`, which regenerates `src/generated/`. Then `pnpm check:generated`. Expected: all pass; the new `.ts` files are committed.
- [ ] **Step 5: Commit**: `feat(hook): the core takes hook events from the bridge and keeps the last 200 per tab`.

### Task 2: Permission hold, decision and release

**Files:**
- Create: `crates/core/src/hooks/holds.rs`, `crates/core/src/agents/mod.rs`, `crates/core/src/agents/claude.rs`, `crates/core/src/server/permissions.rs`
- Modify: `crates/core/src/server/hook.rs`, `crates/core/src/server/mod.rs` (state, routes, `start` reads `MARSHELL_PERMISSION_HOLD_SECS`), `crates/core/src/server/pty_ws.rs` (typing releases)
- Test: `crates/core/tests/permissions.rs`, plus unit tests in `agents/claude.rs`

**Interfaces:**
- Consumes: `PermissionDecision` and the other types from Task 1, plus `HookLog`. A permission request is logged too, as role `permission`.
- Produces:
  - `hooks::holds::Holds::open(&self, tab: Option<String>, agent: String, request: Value) -> (Hold, oneshot::Receiver<Outcome>)`. `Hold` is a guard whose `Drop` removes the entry.
  - `Holds::decide(&self, rid, PermissionDecision) -> bool` and `Holds::release(&self, rid) -> bool`. Each returns false when the rid is unknown or already settled.
  - `Holds::release_tab(&self, tab)`, and `Holds::pending(&self) -> Vec<PendingPermission>`.
  - `enum Outcome { Decided(PermissionDecision), Released }`.
  - `PendingPermission { rid, tab, agent, tool: Option<String>, at_ms: u64, request: Value }` (Serialize). `tool` is the request's `tool_name`.
  - `agents::claude::permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str>`.
  - `AppState::with_permission_hold(self, Duration) -> Self` for tests; the default is `DEFAULT_PERMISSION_HOLD = 120 s`.
  - Routes: `GET /v1/permissions`, `POST /v1/permissions/{rid}/decision` and `POST /v1/permissions/{rid}/release`.
- `rid` is 16 random bytes in hex (`getrandom`, as in `auth::new_token`).

- [ ] **Step 1: Write the failing unit tests** in `agents/claude.rs`:
  - `allow_once_is_bare_allow`: the output equals `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`.
  - `deny_carries_the_message`: `message: Some("Not now")` gives `decision == {"behavior":"deny","message":"Not now"}`. A deny with no message uses `"Denied in Marshell."`.
  - `allow_for_session_adds_the_suggested_rules_to_session`: given a request with `permission_suggestions: [{type:"addRules", rules:[{toolName:"Bash", ruleContent:"npm test"}], behavior:"allow", destination:"localSettings"}]`, the output has `updatedPermissions == [{type:"addRules", rules:[…same…], behavior:"allow", destination:"session"}]`.
  - `allow_for_project_uses_local_settings`: as above, with `destination: "localSettings"`.
  - `scoped_allow_without_a_suggestion_is_an_error`: no `permission_suggestions`, scope `session`, gives `Err("This request has no rule to add.")`.
  - The rule: take every `addRules` suggestion whose `behavior` is `allow`, and rewrite `destination` by scope. A deny ignores the scope.
- [ ] **Step 2: Write the failing integration tests** in `tests/permissions.rs`. Each uses `with_permission_hold(Duration::from_millis(…))` and runs the hook post on a spawned task:
  - `decision_answers_the_held_hook`: post a permission hook; poll `GET /v1/permissions` until one entry has `tool == "Bash"`; post `{"behavior":"allow"}` to its decision route (expect 204); the hook response is 200 and its body is the allow JSON; `GET /v1/permissions` is empty.
  - `release_answers_204_with_no_body`.
  - `hold_expiry_releases`: a 200 ms hold gives a 204 hook response within 1 s.
  - `second_decision_gets_404` (Review Focus 4): a decision, then a decision → 404; a release, then a decision → 404.
  - `dropped_bridge_removes_the_request` (Review Focus 1): abort the hook task while it holds; within 500 ms `GET /v1/permissions` is empty and a decision gets 404.
  - `typing_in_the_tab_releases_its_requests`: call `holds.release_tab(tab)`, the same function `pty_ws` calls on client input. The held hook gets 204.
  - `scoped_allow_without_a_rule_is_409`: decision `{"behavior":"allow","scope":"session"}` on a request with no suggestions gives 409 with the text "This request has no rule to add.", and the request stays pending.
  - `unknown_rid_is_404`, and a bad decision body gives 400.
- [ ] **Step 3: Run** `cargo test -p marshell-core --test permissions` and `cargo test -p marshell-core agents::claude`. Expected: they fail to compile.
- [ ] **Step 4: Implement.** The decision route checks `permission_output` before settling: on `Err` it answers 409 and leaves the request open. On `Ok` it settles, and the hook handler returns 200 with `content-type: application/json`.
  - The hook handler awaits `tokio::time::timeout(hold, rx)`. On timeout or `Released` it returns 204.
  - The handler owns the `Hold` guard, so axum dropping the future removes the request.
  - `pty_ws::run` receives `AppState` and the tab, and calls `release_tab` on every client input frame. It should check first, so the call is cheap when nothing is pending.
  - `start` reads `MARSHELL_PERMISSION_HOLD_SECS` and clamps it to 1..=590.
- [ ] **Step 5: Run** both commands again, then `cargo test -p marshell-core`. Expected: all pass.
- [ ] **Step 6: Commit**: `feat(hook): hold a permission request until it is decided, released, expires or the person types`.

### Task 3: The bridge binary

**Files:**
- Create: `crates/hook/Cargo.toml` (package `marshell-hook`, `[[bin]] name = "marshell-hook"`, plus a `lib` for tests), `crates/hook/src/main.rs`, `crates/hook/src/lib.rs`, `crates/hook/src/args.rs`, `crates/hook/src/endpoint.rs`, `crates/hook/src/post.rs`, `crates/hook/src/log.rs`, `crates/hook/src/record.rs`
- Modify: root `Cargo.toml` (member, and `ureq` in workspace deps), `Cargo.lock`
- Test: `crates/hook/tests/bridge.rs`, using a fake server: a `std::net::TcpListener` on `127.0.0.1:0` in a thread that captures the raw request and replies with a canned status and body. Also unit tests in `args.rs`.

**Interfaces:**
- Consumes: `marshell_protocol::{brand, hook::{HookRole, HEADER_*, HOOK_BODY_LIMIT}}`.
- Produces:
  - `pub fn run(argv: &[String], stdin: &mut dyn Read, env: &dyn Fn(&str) -> Option<String>) -> Vec<u8>`, which returns the bytes to print. `main` calls it with the real stdin and env, writes the result and exits 0. Nothing else ever sets the exit code.
  - `args::parse(&[String]) -> Result<Args, String>`, with `Args { role: HookRole, agent: String, tag: Option<String> }`.
  - `endpoint::discover(env) -> Option<(home: PathBuf, port: u16, token: String)>`.
  - `log::line(home, role, msg)`.
  - `record::save(dir, body) -> io::Result<PathBuf>`. The directory is `MARSHELL_RECORD_DIR` or `<home>/fixtures/<agent>/`. The file is `<hook_event_name or role>.json`, and the second save of one name becomes `<name>-2.json`, and so on.

- [ ] **Step 1: Write the failing tests:**
  - `args.rs`: `parses_role_agent_and_tag`; `unknown_role_is_an_error`; `missing_agent_is_an_error`.
  - `tests/bridge.rs`. Each test uses a temp `MARSHELL_HOME` through the `env` closure and writes `run/endpoint.json` with the fake server's port and token `t`:
    - `event_posts_stdin_with_headers_and_prints_nothing`: the fake server saw `POST /v1/hook`, `authorization: Bearer t`, `x-marshell-role: event`, `x-marshell-agent: claude`, `x-marshell-tab: tab1` and a `x-marshell-version`; its body equals stdin, and `run` returned empty.
    - `no_tab_env_sends_no_tab_header`.
    - `missing_endpoint_file_prints_nothing_and_logs_nothing`: there is no `logs/bridge.log`, because a closed app is not an error.
    - `refused_connection_prints_nothing_quickly`: the port of a dropped listener, finished in under 500 ms.
    - `non_2xx_answer_prints_nothing_and_logs_one_line` (Review Focus 5): the server answers 401; `run` returns empty and `bridge.log` has exactly one line, without `t` in it.
    - `permission_prints_a_200_body_verbatim`; `permission_204_prints_nothing`.
    - `context_prints_a_200_body_and_nothing_on_204`.
    - `slow_event_answer_gives_up_within_budget`: the server sleeps 2 s; `run` returns within 1 s, empty, and logs one line.
    - `stdin_over_1_mib_posts_nothing_and_logs`.
    - `record_saves_stdin_and_still_posts`: `MARSHELL_RECORD_DIR` is a temp dir and stdin is `{"hook_event_name":"Stop"}`; `Stop.json` holds the stdin bytes; a second run writes `Stop-2.json`; the server saw role `record`.
    - `bad_args_print_nothing_and_log_one_line`.
    - `big_log_rotates_to_dot_1`: a pre-made 1 MiB+1 `bridge.log` becomes `bridge.log.1`, and the new `bridge.log` has one line.
- [ ] **Step 2: Run** `cargo test -p marshell-hook`. Expected: it fails to compile.
- [ ] **Step 3: Implement.** Use one `ureq::Agent` per run, with `timeout_connect` 50 ms, `timeout_global` from the role's budget and `http_status_as_error(false)`. Read stdin through `take(HOOK_BODY_LIMIT + 1)`. `statusline` returns empty for now, logging nothing; it is wired in Task 4.
- [ ] **Step 4: Run** `cargo test -p marshell-hook`, then `cargo build -p marshell-hook --release`. Report the size of the release exe in the ledger (a sanity check, with no limit).
- [ ] **Step 5: Commit**: `feat(hook): add the marshell-hook bridge, which posts hook payloads to the core and fails silent`.

### Task 4: Status line chaining in the bridge

**Files:**
- Create: `crates/hook/src/statusline.rs`, `crates/hook/src/shell.rs`
- Modify: `crates/hook/src/lib.rs`
- Test: unit tests in `shell.rs`, and `crates/hook/tests/statusline.rs`

**Interfaces:**
- Consumes: `endpoint::discover`, `post`, `log::line` from Task 3, and the chain path from Task 1's `paths::statusline_chain`. Rebuild that path inside the bridge as `<home>/agents/<agent>/statusline-chain.json`, because the bridge does not depend on the core.
- Produces:
  - The chain file format `{"version":1,"statusLine":{…the user's original object, verbatim…}}`. Task 4 only reads it; 3c writes it.
  - `shell::resolve(env, exists: &dyn Fn(&Path) -> bool, git_on_path: Option<PathBuf>) -> ShellCmd`, with `ShellCmd { program: PathBuf, args_before_command: Vec<String> }`.
  - `statusline::run(home, agent, stdin_bytes, env) -> Vec<u8>`.

- [ ] **Step 1: Write the failing tests.** `shell.rs` (Review Focus 2) on Windows:
  - `git_bash_path_env_wins`: `CLAUDE_CODE_GIT_BASH_PATH=D:/g/bin/bash.exe` existing gives that program, with `-c`.
  - `bash_is_found_next_to_git`: `git_on_path = C:/Program Files/Git/cmd/git.exe` gives `C:/Program Files/Git/bin/bash.exe`, with `-c`.
  - `default_git_install_is_tried`: no git on PATH, but `C:/Program Files/Git/bin/bash.exe` exists, gives that program.
  - `never_the_windowsapps_bash`: the only `bash.exe` anywhere is `…/Microsoft/WindowsApps/bash.exe`, which gives PowerShell (`powershell.exe -NoProfile -Command`).
  - Off Windows: `sh` with `-c`.
- [ ] **Step 2: Write the failing tests** in `tests/statusline.rs`. Each uses the fake server from Task 3 (move it to `tests/common/mod.rs`) and a chain file whose original command is a small script under the test's temp dir, run through the resolved shell:
  - `prints_the_original_output_byte_for_byte`: the original is `cat`, so stdout must equal stdin; with stdin `{"model":{"display_name":"Opus"}}\n`, the output is exactly those bytes, and the server got the same body with role `statusline`.
  - `still_prints_when_the_app_is_closed`: there is no endpoint file, and the output is unchanged.
  - `failing_original_prints_nothing_of_ours` (Review Focus 3): the original is `exit 3` after printing `x`. The bridge prints nothing, because Claude would show nothing for a non-zero exit, and logs one line.
  - `missing_chain_file_prints_nothing_and_logs`.
  - `slow_core_does_not_delay_output`: the server sleeps 2 s before answering; the run takes under 1 s.
- [ ] **Step 3: Run** `cargo test -p marshell-hook`. Expected: the new tests fail.
- [ ] **Step 4: Implement.**
  - Start the forward on a thread with the 100 ms budget. Run `<program> <args_before_command> <statusLine.command>`, passing stdin to the child, and set `COLUMNS` and `LINES` from the bridge's own env when they are present.
  - Wait for the child, print its stdout only on exit 0, then join the forward thread for at most whatever remains of the 100 ms.
  - The git on PATH is found by scanning `PATH` for `git.exe`. The resolution order is: the env var; then `<git root>/bin/bash.exe`, where the root is the parent of a `cmd`, `bin` or `mingw64/bin` folder holding `git.exe`; then `C:/Program Files/Git/bin/bash.exe`; then PowerShell. A path containing `\WindowsApps\` is never used.
- [ ] **Step 5: Run** `cargo test -p marshell-hook`. Expected: all pass, on the Windows CI runner too, which has Git for Windows.
- [ ] **Step 6: Commit**: `feat(hook): chain the user's status line through the bridge and print its output unchanged`.

### Task 5: Install the bridge on launch

**Files:**
- Create: `crates/core/src/bridge.rs`
- Modify: `crates/core/src/lib.rs`, `src-tauri/src/main.rs` (call it in `setup`, after `paths::home_dir()`), `src-tauri/tauri.conf.json` (`beforeDevCommand`, `beforeBuildCommand`), `package.json` (scripts)
- Test: `crates/core/tests/bridge_install.rs`

**Interfaces:**
- Consumes: `paths::bin_dir`, `paths::bridge_exe` (Task 1).
- Produces:
  - `bridge::install(source: &Path, home: &Path) -> io::Result<BridgeInstall>`, with `enum BridgeInstall { Installed, Updated, Unchanged }`.
  - `bridge::source_next_to(exe: &Path) -> Option<PathBuf>`: the sibling `marshell-hook` plus `EXE_SUFFIX`, if it exists.
  - `bridge::sweep_leftovers(home)`.
- Algorithm:
  1. Run `sweep_leftovers`, which deletes `bin/marshell-hook.old-*` and `bin/marshell-hook.new*`, ignoring errors such as a file still locked.
  2. If the destination's bytes equal the source's, return `Unchanged`. The bytes are compared in full, so no hash dependency is needed.
  3. Otherwise copy the source to `bin/marshell-hook.new<EXE_SUFFIX>`.
  4. If the destination exists, rename it to `bin/marshell-hook.old-<unix_ms><EXE_SUFFIX>`.
  5. Rename `.new` into place, retrying 5 times, 50 ms apart.
  6. Return `Updated` or `Installed`.
  - On Unix, set mode 0o755 on the new file.

- [ ] **Step 1: Write the failing tests:**
  - `installs_into_an_empty_home`; `same_bytes_are_unchanged`; `new_bytes_update_and_leave_an_old_copy`; `sweep_removes_leftovers`.
  - `updates_while_the_old_bridge_is_running` (Windows only): copy the `fake-agent` binary (`env!("CARGO_BIN_EXE_fake-agent")`) as the installed bridge, start it from `bin/` with `sleep 30`, so it is running and locked. Install a source with different bytes and expect `Updated`. The destination now has the new bytes, and one `marshell-hook.old-*` exists. Kill the child by its handle, then `sweep_leftovers` removes it.
- [ ] **Step 2: Run** `cargo test -p marshell-core --test bridge_install`. Expected: it fails to compile.
- [ ] **Step 3: Implement** `bridge.rs`. In `main.rs`, call it on a `std::thread` with `bridge::source_next_to(&std::env::current_exe()?)`. A missing source or an error is logged with `tracing::warn!` and never stops the launch.
  - `package.json`: add `"build:hook": "cargo build -p marshell-hook"` and `"build:hook:release": "cargo build --release -p marshell-hook"`.
  - `tauri.conf.json`: `beforeDevCommand: "pnpm build:hook && pnpm dev"` and `beforeBuildCommand: "pnpm build:hook:release && pnpm build"`. The bridge then lands in the same `target/<profile>` folder as the app exe.
  - Bundling it as a Tauri `externalBin` waits for the release phase, because tauri-build fails `cargo test --workspace` when a sidecar file is missing. Record this as a known limit.
- [ ] **Step 4: Run** `cargo test -p marshell-core --test bridge_install`, then `cargo clippy --workspace --all-targets -- -D warnings`. Expected: pass and clean.
- [ ] **Step 5: Commit**: `feat(hook): copy the bridge into ~/.marshell/bin on launch, renaming around a running copy`.

### Task 6: Fixture scrubbing

**Files:**
- Create: `scripts/scrub-fixtures.mjs`, `scripts/scrub-fixtures.test.ts`, `crates/core/tests/fixtures_scrubbed.rs`
- Modify: `package.json` (`"scrub:fixtures": "node scripts/scrub-fixtures.mjs"`)

**Interfaces:**
- Produces:
  - `export function scrub(text: string, opts: { home: string, user: string }): string`.
  - The CLI `node scripts/scrub-fixtures.mjs <dir>` rewrites every `.json` file under `<dir>` in place. `home` comes from `os.homedir()` and `user` from its last path part.
- Rules, applied in order:
  1. Each spelling of the home path becomes the matching spelling of `C:\Users\user`: the JSON-escaped backslash form, the plain backslash form, the forward-slash form, and Git Bash's `/c/Users/<user>`.
  2. Claude's project slug form of the user name, with spaces as `-` (`Home-Office`), becomes `user`.
  3. Any remaining occurrence of the user name becomes `user`.
  4. Each distinct UUID becomes `00000000-0000-4000-8000-` followed by a 12-digit counter, in order of first appearance across the directory, so links between files survive.
  5. Any e-mail address becomes `user@example.com`.

- [ ] **Step 1: Write the failing Vitest tests:**
  - `replaces every spelling of the home path`, with input holding `C:\\Users\\Home Office\\.claude`, `C:/Users/Home Office/x` and `/c/Users/Home Office/y`.
  - `replaces the project slug form`: `C--Users-Home-Office-AppData` becomes `C--Users-user-AppData`.
  - `maps the same UUID to the same placeholder`.
  - `replaces e-mail addresses`.
  - `leaves other text alone`.
  - Use the user name `Home Office` as a test value, not a real e-mail.
- [ ] **Step 2: Write the failing Rust test** `fixtures_are_scrubbed`. It walks `crates/core/tests/fixtures/` and asserts:
  - there is at least one `.json` file under `claude/`;
  - no file contains `Home Office` or `Home-Office`;
  - no file matches the e-mail regex, except `user@example.com`;
  - every UUID starts with `00000000-0000-4000-8000-`.
  - It stays red until Task 7 commits fixtures. That is expected; note it in the ledger, and keep the test `#[ignore]`d in this task's commit.
- [ ] **Step 3: Run** `pnpm test scripts/scrub-fixtures.test.ts`. Expected: FAIL (no module).
- [ ] **Step 4: Implement** `scrub-fixtures.mjs`.
- [ ] **Step 5: Run** `pnpm test scripts/scrub-fixtures.test.ts`. Expected: PASS.
- [ ] **Step 6: Commit**: `test(hook): scrub recorded fixtures of the home path, user name, ids and e-mail before commit`.

### Task 7: Live check, spikes S10, S4 and S9, and fixtures

The real app, a real Claude Code, and the user's real `statusline.py`, which is only run. The recipe is CLAUDE.md's "Live check (Windows)". Everything below is set up by the person running the check (me), not by Marshell.

**Files:**
- Create: `docs/spikes/S10-hook-path-spaces.md`, `docs/spikes/S4-statusline-chain.md`, `docs/spikes/S9-permission-hold.md`
- Create: `crates/core/tests/fixtures/claude/<claude --version>/*.json`
- Modify: `crates/core/tests/fixtures_scrubbed.rs` (drop `#[ignore]`), `docs/PLAN.md` (§5 and §8 outcomes, §9 3a decisions if anything changes), `docs/research/` only if a finding doesn't belong in a spike file

- [ ] **Step 1: Set up.**
  - Scratch `MARSHELL_HOME` under `%TEMP%`. Its path holds `Home Office`, which is the S10 condition.
  - Run `pnpm tauri dev` with that home, `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`, and `MARSHELL_PERMISSION_HOLD_SECS=120`.
  - Check that `<home>/bin/marshell-hook.exe` was installed, which is Task 5 live.
  - Make the scratch project `C:\dev\spike-3a`, outside the repo, with `.claude/settings.local.json`:
    - For every documented event, two hook entries. One is shell form, with the command `"<home with forward slashes>/bin/marshell-hook.exe" record --agent claude --tag mshl1`. The other is exec form, with the command as the bare path and `args: ["event","--agent","claude","--tag","mshl1"]`.
    - `PermissionRequest` gets one more entry, shell form, with role `permission` and `timeout: 600`.
    - `statusLine` is `{"type":"command","command":"\"<home>/bin/marshell-hook.exe\" statusline --agent claude --tag mshl1"}`, with `<home>/agents/claude/statusline-chain.json` holding `{"version":1,"statusLine":{"type":"command","command":"python ~/.claude/statusline.py"}}`.
  - Set `MARSHELL_RECORD_DIR` to a scratch folder.
- [ ] **Step 2: S10.** Start a Claude session in `C:\dev\spike-3a` from Marshell, answer the folder trust prompt, and give it a made-up prompt that runs one Bash command.
  - Check `GET /v1/hook/events?tab=<tab>` for records from both the shell-form and the exec-form entries.
  - Record in `S10-hook-path-spaces.md`: which forms ran, the shell Claude used, the trust behaviour, the latency from the hook to the core (`at_ms` against the transcript's timestamps), and anything in `bridge.log`.
  - The finding decides the form 3c installs.
- [ ] **Step 3: S4.**
  - Compare the bridge's status line, as Claude shows it, with `python ~/.claude/statusline.py` fed the same stdin. Take that stdin from the core's last `statusline` record.
  - Compare byte for byte by running both on that stdin in a terminal.
  - Check that the core got the status line JSON (model, context window, version).
  - Record the quoting, the shell used, and whether the fallback (`marshell-hook tee | <original>`) is needed.
- [ ] **Step 4: S9.** Prompt for a command that needs permission.
  - **While it is held:** is Claude's own dialog visible in the pane? Read the screen through the terminal buffer over CDP.
  - **Decisions:** allow through `POST /v1/permissions/{rid}/decision`, and check that the command ran. Repeat with deny and a message. Repeat with release, and check that the native prompt appears; time how long it takes.
  - **Expiry:** restart with `MARSHELL_PERMISSION_HOLD_SECS=10` and let the hold expire.
  - **Typing:** type a key into the pane while a request is held, and check that it is released.
  - **Scoped allow:** test "allow for session", and check that the same command is not asked again.
  - Record all of it in `S9-permission-hold.md`, with the default hold it leads to. If it differs from 120 s, update PLAN §5 and the `DEFAULT_PERMISSION_HOLD` constant, with a test.
- [ ] **Step 5: Fixtures.**
  - Copy the recorded files to `crates/core/tests/fixtures/claude/<version>/`, where the version comes from `claude --version`.
  - Keep one file per event name, plus one `statusline.json` from the core's log.
  - Run `pnpm scrub:fixtures crates/core/tests/fixtures/claude`, remove `#[ignore]`, and run `cargo test -p marshell-core --test fixtures_scrubbed`. Expected: PASS.
  - Read each fixture by eye before committing, so no prompt text of the user's own is included.
- [ ] **Step 6: Tear down.** Stop the app and the Claude session started for the check, by PID. Delete the scratch home, `C:\dev\spike-3a` and the record folder.
- [ ] **Step 7: Commit**:
  - `docs(spikes): S10, S4 and S9 against Claude Code <version>`
  - `test(hook): recorded, scrubbed Claude hook fixtures`

## Exit criteria for 3a

- A Claude session started in Marshell posts every hook it fires to the core, and the status line still shows the user's own line.
- A held permission request is allowed, denied, released, expired and released by typing, each seen in the real CLI.
- The bridge in `~/.marshell/bin` updates while a copy is running.
- `cargo test --workspace`, `pnpm test`, clippy, fmt, `pnpm build` and `pnpm check:generated` are green.
