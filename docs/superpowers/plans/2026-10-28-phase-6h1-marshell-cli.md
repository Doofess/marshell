# Slice 6h1: the `marshell` CLI and two escape hatches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task, dispatching each task that edits more than a couple of files to `worker`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `marshell list`, `send`, `new` and `focus` drive the running app from any terminal and from agents inside it, and every agent session has "Copy resume command" and "Open in system terminal".

**Architecture:** The CLI is a small client of the existing loopback API. It finds the app through `<home>/run/endpoint.json`, as the hook bridge does, and reuses `marshell_hook::endpoint`. Text goes into a session only through a new `fill` route, which refuses every control character and can wait until the agent is ready. `focus` is a new route that hands the tab to the app shell's existing `link::open`. The core builds the resume command and starts the outside terminal itself, so no command line ever comes from a request body. The frontend adds two menu items in two places.

**Tech Stack:** Rust (`marshell-protocol`, `marshell-core`, `marshell-hook` as a library, `marshell-cli`, axum, `ureq`, `windows-sys`, ts-rs), the Tauri shell, React and TypeScript, Vitest with `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, "Slice 6h1 decisions (2026-10-28)" (decisions 1–11), with slice 6a decision 7 (`send` never presses Enter) and `docs/adr/0001-threat-model.md`.

## Global Constraints

- Marshell never presses Enter. Nothing in this slice writes `\r`, `\n` or any other control character into a session (SCOPE.md "Never").
- The new routes are token-guarded like every other. Don't touch `guard.rs` or the token handling. Open in system terminal and the resume command are built only from what the core holds (the tab, its history row, its profile), never from a request body. An id reaches a command line only after `sidecar_allowed`.
- The CLI never reads the process environment in tests. It takes an `env` closure, as `uninstall::run` does. Tests start an in-process core on a temp home and never touch the real `~/.marshell`, `~/.claude`, `~/.codex` or `~/.gemini`.
- Protocol types live in `crates/protocol`. After a change, run `cargo test -j 4 -p marshell-protocol`, commit `src/generated/`, and check that `pnpm check:generated` passes.
- No new crate dependency. `marshell-cli` adds the workspace's own `marshell-hook`, `ureq`, `serde` and `serde_json`, plus `windows-sys` with the features `marshell-hook` already uses plus `Win32_UI_WindowsAndMessaging`.
- Windows is the platform verified here. On other systems Open in system terminal refuses with its line, and `focus` skips the foreground grant.
- Every UI change works with the keyboard alone and in forced colours. `Announcer` is the only live region.
- Always run cargo with `-j 4`, and run `cargo fmt --all` before each Rust commit. Commit messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Copy decided in PLAN §9 6h1: "Marshell isn't running. Start it? [y/N] " and "End the session first."

Copy proposed for approval at plan review (nothing else is new):
- CLI, app closed: "Marshell isn't running." Starting it: "Couldn't start Marshell: open it once from the Start menu first." (no record of the app), "Couldn't start Marshell: <path> isn't there.", "Marshell didn't start within 30 s."
- CLI, sessions: "No sessions." / "No session <x>." / "Session <n> has ended, so it can't take text." / "Started session <n>." / "Filled session <n>'s prompt line." / "Session <n> wasn't ready within 30 s, so its prompt line wasn't filled."
- Text refusals (from the core, shown by the CLI): "The text can't hold line breaks or other control characters: Marshell never presses Enter." / "The text is over 4000 characters."
- Menu items: "Copy resume command", "Open in system terminal".
- Toasts: "Copied the resume command." / "Copied. It continues the last conversation in this folder." (the continue-last form; see the change below) / "Copied. It starts a new conversation: this one was never saved." / "Opened in a system terminal."
- Refusals: "A DeepSeek session needs Marshell to start it, so there is no command to copy." / "<Agent> has no resume command." / "Open in system terminal works on Windows only for now." / existing `NO_CONVERSATION`, `gone()` and "Its folder no longer exists: <folder>".
- **A change to decision 5:** the menu item can't say in advance which form it copies, because the menu is built from the session row, and the row doesn't know whether a hook reported an id. Instead, the toast after copying says so. The alternative is a new `SessionInfo` field, filled from a history lookup on every snapshot.

## Review Focus

1. **Session numbers are reused** (lowest free slot). `send 6` matches only an open session's number, never a closed one. Every command also takes a tab id, and `--json` prints it, so a script can hold onto one session.
2. **`send` to an Ended or Exited row** is refused with "Session <n> has ended, so it can't take text.", never written to a dead pty.
3. **Control characters in `send` or `--prompt`**: `\x1b[13u` (CSI-u Enter), `\x1b[201~` (end of bracketed paste), `\x03`, `\x04`, a tab, U+009B (C1 CSI) and DEL are all refused by the core's fill route, whoever calls it.
4. **A session folder holding spaces or a `'`**: the copied line quotes it for the session's shell (PowerShell, Command Prompt, POSIX). The line is always one line, so pasting it runs nothing until the person presses Enter.
5. **Claude Code in a new folder** shows its folder-trust dialog with quiet output. `new --prompt` doesn't fill it, because SessionStart hasn't arrived. The live check repeats this with agy's terms screen and Codex's folder trust, and records what each CLI does.

---

### Task 1: The fill route, readiness, and `marshell` on a session's PATH

**Files:**
- Modify: `crates/protocol/src/api.rs` (add `FillRequest`), `src/generated/` (regenerated)
- Create: `crates/core/src/server/fill.rs`
- Modify: `crates/core/src/server/mod.rs` (route), `crates/core/src/registry.rs` (`session_started`), `crates/core/src/server/sessions.rs` (`spawn_env` PATH)
- Test: in `fill.rs`, `registry.rs` and `sessions.rs` test modules

**Interfaces:**
- Produces: `POST /v1/sessions/{tab}/fill`, body `FillRequest { text: String, wait: bool }`. The answers: 204 when filled. 400 with the text-refusal line. 404 "No such session." 409 `ResumeError { reason }`, with "ended" for an Ended or Exited row or "not_ready" on a wait timeout. The CLI maps those two reasons to its own lines, which hold the number.
- Produces: `pub fn check_text(text: &str) -> Result<(), &'static str>`, `pub const FILL_TIMEOUT: Duration = 30 s`, `pub const FILL_QUIET: Duration = 1500 ms`, `pub fn ready(claude_hooked: bool, started: bool, end: u64, quiet_for: Duration) -> bool` in `fill.rs`.
- Produces: `Registry::session_started(&self, tab: &str) -> bool`, true once a `SessionStart` hook event has landed for the tab's current process.

- [ ] **Step 1: Write the failing tests**
  - `check_text_refuses_controls`: each of `"a\rb"`, `"a\nb"`, `"\x1b[13u"`, `"\x1b[201~"`, `"\x03"`, `"\x04"`, `"a\tb"`, `"\u{9b}"` and `"\x7f"` gives `Err(TEXT_REFUSED)`. `"x".repeat(4001)` gives `Err(TEXT_TOO_LONG)`, and `"fix the tests — please"` is `Ok`.
  - `ready_waits_for_output_then_quiet`: `ready(false, false, 0, 10 s)` is false (no output yet). `ready(false, false, 10, 1499 ms)` is false, and `ready(false, false, 10, 1500 ms)` is true.
  - `claude_waits_for_session_start`: `ready(true, false, 10, 60 s)` is false, and `ready(true, true, 10, 1500 ms)` is true.
  - `session_started_is_per_process`: after a `SessionStart` hook for tab `a`, `session_started("a")` is true. After a restart (a new generation), it is false.
  - `fill_refuses_an_ended_row` and `fill_without_wait_writes_the_text` go through the in-process router. The second checks that the pty input received exactly the bytes of `text`.
  - `spawn_env_puts_bin_first_on_path`: with a profile PATH of `C:\a`, the env's `PATH` is `<home>\bin;C:\a`. With no profile PATH, it is `<home>\bin` followed by the app's own PATH.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-core fill session_started spawn_env`. They fail to compile, because the items don't exist yet.
- [ ] **Step 3: Implement.** Add `FillRequest` (with ts-rs export) next to `InputRequest`.
  - `fill.rs`: refuse every `char` below U+0020, U+007F, and U+0080 to U+009F, plus text over 4000 `char`s. With `wait`, poll every 100 ms until `ready(...)` or `FILL_TIMEOUT`. `claude_hooked` is `agent == "claude"` with `status_note == None`. `end` comes from `Session::counters().0`, and the quiet time is measured since `end` last grew. Then call `session.write_input(text.into_bytes())`.
  - Registry: set the flag on the entry where `SessionStart` is already matched (`registry.rs`, around line 1002), and clear it on a new generation.
  - `spawn_env` takes the home and puts `paths::bin_dir(home)` first on `PATH`, using `std::env::join_paths`. Skip it for a `ShellFamily::Wsl` profile (WSL is 6j).
- [ ] **Step 4: Run the tests and see them pass.** Run the same command, then `cargo test -j 4 -p marshell-protocol` and `pnpm check:generated`.
- [ ] **Step 5: Commit:** `feat(sessions): fill a session's prompt line without control characters, wait for the agent to be ready, and put marshell on a session's PATH`

### Task 2: The focus route, and the app records where it is

**Files:**
- Modify: `crates/core/src/server/mod.rs` (`CoreConfig.focus`, route), `crates/core/src/server/sessions.rs` (`focus` handler), `crates/core/src/supervisor.rs` (`CoreEvent::Focus`), `crates/core/src/paths.rs` (`app_record`), `src-tauri/src/main.rs`
- Test: `sessions.rs` and `paths.rs` test modules, and `src-tauri/src/link.rs` tests if `open` gains a helper

**Interfaces:**
- Produces: `POST /v1/sessions/{tab}/focus`. It answers 204 after calling the sink, 404 "No such session.", or 503 "Marshell's window isn't there to focus." when `CoreConfig.focus` is `None`.
- Produces: `pub type FocusSink = Arc<dyn Fn(String) + Send + Sync>`, `CoreConfig.focus: Option<FocusSink>`, `CoreEvent::Focus { tab_id: String }`. In `main.rs`, `emit_core_event` sends `Focus` to `link::open(app, Some(Link::Focus(tab_id)))`.
- Produces: `paths::app_record(home) -> PathBuf` = `<home>/bin/app.json` holding `{ "exe": "<absolute path>" }`. The shell writes it at every launch, next to `bridge::ensure`, with an atomic write. A failure is logged and never stops the launch.

- [ ] **Step 1: Write the failing tests**
  - `focus_calls_the_sink_with_the_tab`: a test sink records `"t1"`, and the route gives 204.
  - `focus_unknown_tab_is_404`.
  - `focus_without_a_sink_is_503`.
  - `app_record_is_in_bin`: the path ends in `bin/app.json`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-core focus app_record`.
- [ ] **Step 3: Implement.** The supervisor wraps `cfg.focus` the way it wraps `cfg.notify` (`supervisor.rs:53`). In `main.rs`, write `app_record` from `std::env::current_exe()` during setup.
- [ ] **Step 4: Run the tests and see them pass**, then build the shell: `cargo build -j 4 -p marshell-app`.
- [ ] **Step 5: Commit:** `feat(sessions): bring the window to a session from the local API, and record where the app is`

### Task 3: The resume command and Open in system terminal, in the core

**Files:**
- Modify: `crates/protocol/src/api.rs` (`ResumeCommand`), `src/generated/`
- Create: `crates/core/src/server/outside.rs` (handlers), `crates/core/src/outside.rs` (`copy_line`, `open_console`)
- Modify: `crates/core/src/server/mod.rs` (four routes), `crates/core/src/profiles/detect.rs` (expose the line builder), `docs/adr/0001-threat-model.md`
- Test: `outside.rs` test modules

**Interfaces:**
- Produces: `ResumeCommand { command: String, form: ResumeForm }`, with `ResumeForm` = `ById | ContinueLast | Fresh`, snake_case.
- Produces: `GET /v1/sessions/{tab}/resume-command` and `GET /v1/history/{agent}/{session_id}/resume-command`, giving `ResumeCommand` or 409 `ResumeError`.
- Produces: `POST /v1/sessions/{tab}/open-outside` and `POST /v1/history/{agent}/{session_id}/open-outside`, giving 204 or 409 `ResumeError`. A running tab, or an archived conversation running in a tab, gives "End the session first."
- Produces: `pub fn copy_line(family: ShellFamily, cwd: &str, line: &str) -> String`, and `pub fn open_console(argv: &[String], env: &[(String, String)], env_remove: &[String], cwd: &Path) -> io::Result<()>`.

- [ ] **Step 1: Write the failing tests**
  - `copy_line_quotes_per_shell`:
    - PowerShell, `C:\My 'x'`: `Set-Location -LiteralPath 'C:\My ''x'''; <line>`.
    - Cmd, `C:\My x`: `cd /d "C:\My x" && <line>`.
    - Posix, `/home/a b`: `cd '/home/a b' && <line>`.
    - No result contains `\r` or `\n`.
  - `resume_command_by_id`: a Claude tab whose history row passes `sidecar_allowed` gives `ById`, and the command contains `--resume <id>`. A Codex row's command contains `resume <id>` and not `--no-daemon`.
  - `resume_command_continue_last`: a manifest agent (Copilot) gives `ContinueLast` with its `continue` args. Aider, with no `continue`, is refused with "Aider has no resume command."
  - `resume_command_refuses_a_preset`: a DeepSeek tab gives the DeepSeek line.
  - `open_outside_refuses_a_running_tab` gives "End the session first."
  - `open_outside_env_drops_marshell_vars`: the env handed to `open_console` has no `MARSHELL_TAB_ID`, `MARSHELL_SESSION_NUMBER` or `MARSHELL_AGENT`. Test this through a pure `outside_env(...)`.
  - A non-Windows `open_console` returns the "works on Windows only" refusal (`cfg(not(windows))` test).
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-core outside resume_command open_outside`.
- [ ] **Step 3: Implement.**
  - The id comes from `resume_session` (sessions.rs) or `session_meta` (history.rs). Don't extend `src/features/sessions/resumeCommand.ts`.
  - The line comes from the per-family builder in `detect.rs`, with the resume target, the session's mode and `no_daemon = false`.
  - `open_console` uses `resolve()`'s argv, which runs in the profile's shell and leaves it open after the CLI exits. Check this in the test with the argv shape, and confirm it in the live check. Spawn it with `CREATE_NEW_CONSOLE | CREATE_BREAKAWAY_FROM_JOB`, in the session's folder.
  - In the ADR, add a section "`marshell` CLI routes (6h1)" covering fill, focus, resume-command and open-outside, with what each refuses.
- [ ] **Step 4: Run the tests and see them pass**, then run `cargo test -j 4 -p marshell-protocol` and `pnpm check:generated`.
- [ ] **Step 5: Commit:** `feat(sessions): build a session's resume command in the core and open it in a system terminal`

### Task 4: The CLI client, `list` and `send`

**Files:**
- Modify: `crates/cli/Cargo.toml`, `crates/cli/src/args.rs` (commands, `USAGE`), `crates/cli/src/main.rs`
- Create: `crates/cli/src/client.rs`, `crates/cli/src/list.rs`, `crates/cli/src/send.rs`
- Test: in each new file, plus `crates/cli/tests/` against an in-process core (`marshell_core::server::start` on a temp home)

**Interfaces:**
- Produces: `Command::List { json: bool }` and `Command::Send { session: String, text: String }`.
- Produces: `client::Client::connect(env) -> Result<Client, NotRunning>`, using `marshell_hook::endpoint::discover`. Then `Client::sessions() -> io::Result<SessionsSnapshot>`, `Client::fill(tab, text, wait) -> Result<(), Refusal>` and `Client::post(path, body)`.
- Produces: `pub fn resolve(snapshot: &SessionsSnapshot, session: &str) -> Option<&SessionInfo>`. A number matches only `sessions` (open), and any other string must equal a `tab_id`.
- Produces: `list --json` prints `[{ "number", "tab_id", "name", "agent", "state", "cwd", "project", "mode" }]`, where `state` is `running`, `exited` or `ended`, and `agent` and `mode` are null when there is none.
- Exit codes: 0 on success, 1 for a refusal, a failure or the app being closed, 2 for a usage error.

- [ ] **Step 1: Write the failing tests**
  - `args_parse_list_and_send`: `list --json` parses. `send 6 "hi"` gives `Send { session: "6", text: "hi" }`. `send 6` is a usage error.
  - `resolve_matches_open_numbers_and_tab_ids`: a closed session's number gives `None`. Its tab id doesn't match either, because a closed tab isn't open.
  - `list_table_and_json`: the table has a header and one row per open session. `"No sessions."` is printed when there are none. `--json` parses back to the fields above.
  - `send_fills_without_enter`: against an in-process core, the session's input gets exactly `hi`.
  - `send_refuses_controls_and_ended`: `"a\nb"` prints the core's text-refusal line and exits 1. An Ended row prints "Session <n> has ended, so it can't take text."
  - `closed_app_says_so`: with no `endpoint.json`, both commands print "Marshell isn't running." and exit 1.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-cli`.
- [ ] **Step 3: Implement.** `ureq` with the bearer token and a 5 s timeout, except `fill` with `wait`, which gets 35 s. Never print the token, and replace it in any error text, as `crates/hook/src/lib.rs` does. Print the table with plain padding: no colour and no new dependency.
- [ ] **Step 4: Run the tests and see them pass**, the same command.
- [ ] **Step 5: Commit:** `feat(cli): list sessions and fill one's prompt line from the command line`

### Task 5: `new`, `focus`, and starting the app

**Files:**
- Modify: `crates/cli/src/args.rs`, `crates/cli/src/main.rs`
- Create: `crates/cli/src/new.rs`, `crates/cli/src/focus.rs`, `crates/cli/src/start.rs`
- Test: in each new file, plus `crates/cli/tests/`

**Interfaces:**
- Consumes: Task 4's `Client` and `resolve`, Task 1's fill route, Task 2's focus route and `app.json`.
- Produces: `Command::New { agent: Option<String>, cwd: Option<String>, prompt: Option<String>, mode: Option<SessionMode>, name: Option<String>, start: bool }` and `Command::Focus { session: String, start: bool }`.
- Produces: `start::ensure_running(env, stdin_is_tty, ask: &mut dyn BufRead, out: &mut dyn Write, start: bool) -> Result<Client, String>`. It asks only when `stdin_is_tty` and `!start`. It reads `app.json`, spawns the exe with no arguments, detached, and polls `discover` and then `GET /v1/health` every 250 ms for up to 30 s.

- [ ] **Step 1: Write the failing tests**
  - `args_parse_new_and_focus`: this command line gives every field.

    ```
    new --agent claude --cwd . --prompt "x" --mode plan --name n --start
    ```

    `--cwd .` is made absolute from the CLI's own folder.
  - `ensure_running_asks_once`:
    - With a TTY and the answer `n`, it prints the decided question and returns the "isn't running" error.
    - With no TTY and no `--start`, it doesn't ask.
    - With a missing `app.json`, it gives the "open it once" line.
    - With an `exe` that is missing, it gives "<path> isn't there."
  - `new_creates_then_fills`: against an in-process core with a stand-in agent, `CreateSessionRequest.prompt` is `None`. A fill with `wait: true` follows, and the CLI prints "Started session <n>." then "Filled session <n>'s prompt line." A not-ready 409 prints the not-ready line and exits 1, and the session stays open.
  - `focus_posts_the_tab`: a number resolves to its tab, and the route is called.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-cli`.
- [ ] **Step 3: Implement.**
  - `new` creates the session at 120 × 30, which the window resizes on attach.
  - `focus` calls `AllowSetForegroundWindow(pid)` on Windows before the POST, with the `pid` read from `endpoint.json`. Extend `marshell_hook::endpoint` with `discover_pid`, rather than parsing the file twice.
  - `start` spawns with `DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB`, so a CLI run inside a session doesn't tie the app to that session's job.
  - Update `USAGE` with all six commands and `main.rs`'s doc line.
- [ ] **Step 4: Run the tests and see them pass**, then run `cargo clippy -j 4 --workspace --all-targets -- -D warnings`.
- [ ] **Step 5: Commit:** `feat(cli): start a session, bring the window to one, and offer to start Marshell when it's closed`

### Task 6: The two menu items, in the session menu and the archive

**Files:**
- Modify: `src/lib/api/sessions.ts` and `src/lib/api/history.ts` (four calls), `src/features/sidebar/sessionMenu.ts` (`SessionMenuFacts.outside`), `src/features/sessions/sessionMenuRun.ts`, `src/features/sessions/LiveApp.tsx` (the menu `case`s), `src/features/history/ConversationView.tsx` (`moreActions`), `src/features/history/LiveHistory.tsx` (handlers)
- Test: `sessionMenu.test.ts`, `sessionMenuRun.test.ts`, `ConversationView.test.tsx`, `sessions.test.ts`, `history.test.ts`

**Interfaces:**
- Consumes: Task 3's routes and `ResumeCommand`.
- Produces: `sessions.resumeCommand(ep, tab)`, `sessions.openOutside(ep, tab)`, `history.resumeCommand(ep, agent, id)` and `history.openOutside(ep, agent, id)`, each a `Started`-style result that carries the core's `reason`.
- Produces: `toastFor(form: ResumeForm): string`, which maps the three forms to the three "Copied…" toasts.

- [ ] **Step 1: Write the failing tests**
  - `agent_sessions_get_both_items`: `sessionMenuItems({...facts, agent: true}, os)` includes `copy-resume` and `open-outside`, after Resume and before Mode ▸. A plain shell (`agent: false`) has neither.
  - `open_outside_on_a_running_session_toasts`: the runner calls no API and shows "End the session first."
  - `more_actions_lists_both`: `moreActions` has both items after "Copy as markdown".
  - `toastFor_maps_forms`: one test per form.
  - A refused copy shows the core's `reason` as the toast.
- [ ] **Step 2: Run them and see them fail:** `pnpm test src/features/sidebar src/features/sessions src/features/history src/lib/api`.
- [ ] **Step 3: Implement.** Copy with `navigator.clipboard.writeText`, the way Copy as markdown does. Announce the toast through `Announcer` as an `action-result`. The menu shows the items on every agent session, and they never appear disabled (decision 11).
- [ ] **Step 4: Run the tests and see them pass**, then `pnpm build`.
- [ ] **Step 5: Commit:** `feat(sessions): copy a session's resume command and open it in a system terminal`

### Task 7: Live check

**Files:**
- Create: `docs/research/2026-10-28-marshell-cli-live-check.md`

- [ ] **Step 1:** Run the `marshell-live-check` skill. It sets a scratch `MARSHELL_HOME`, `USERPROFILE` and `CLAUDE_CONFIG_DIR`, plus a scratch `CODEX_HOME`. Every agent session in the check is one the check starts. Record each PID that is started.
- [ ] **Step 2:** From a session in the app, run `marshell list --json`. This proves the session PATH works. Then run `send <n> "hello"` to a second session. The prompt line shows `hello` and nothing is submitted. Try `send` with `$'a\x1b[13u'`: the core refuses it.
- [ ] **Step 3:** Run `marshell new --agent claude --cwd <fresh scratch folder> --prompt "say hi"`. No fill appears while the trust dialog shows. After the check answers the dialog in its own session, the prompt line fills within 30 s. Repeat with Codex and with agy. For agy, don't answer its terms screen: record what lands, and whether the 30 s timeout line prints.
- [ ] **Step 4:** Minimise the app and run `marshell focus <n>` from an outside terminal. Record whether the window comes forward or only flashes. Close the app, run `marshell new --start`, and check that the app comes up on the scratch home. Stop it by PID.
- [ ] **Step 5:** End a Claude session, then use Open in system terminal. A window opens in the session's folder with `claude --resume <id>`. Close Marshell: that window stays open. On a running session, choose the item: the toast reads "End the session first." Paste the result of Copy resume command into PowerShell and check that it resumes the same conversation.
- [ ] **Step 6:** Check that the real `~/.claude`, `~/.codex` and `~/.gemini` modification times haven't changed. Stop everything the check started, by PID. Write the note and commit it: `docs(research): record the marshell CLI live check`.
