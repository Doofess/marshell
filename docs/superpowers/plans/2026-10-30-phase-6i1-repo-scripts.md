# Repo scripts (slice 6i1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A repo's committed `marshell.json` gives Marshell three scripts: `setup` runs in a new task worktree, `run` starts a dev server as its own tab from a Run button, and `teardown` runs before a worktree is removed. All three run only after the person allows that repo's exact scripts.

**Architecture:** The core reads, resolves and hashes `marshell.json` (`crates/core/src/scripts.rs`) and keeps the trust list in `<home>/scripts-trust.json`. The core alone turns a script into a process. `POST /v1/scripts/run` starts `setup` or `run` as a session only when the file's current hash is trusted and matches what the UI showed, and the remove plan runs `teardown` the same way. The frontend shows the exact scripts and asks for trust, runs setup before the agent session starts, and adds Run/Stop to the session header and the palette.

**Tech Stack:** Rust (axum, serde_json, the existing `store`, `config::hash`, `projects` and `git` modules), React + TypeScript, ts-rs for the generated types.

**Spec:** `docs/PLAN.md` §9 "Slice 6i1 decisions (2026-10-30)" (decisions 1–8); brief "Repo scripts" (`docs/BRIEF.md`, the line starting `- **Repo scripts**`).

## Global Constraints

- Nothing in `marshell.json` runs until the person has allowed that repo's scripts. Trust is the pair (main repo root, SHA-256 of the file's bytes). A changed file asks again (decision 2).
- The core never runs a script whose hash the UI didn't show. `run` requests carry the hash, and a different hash is a 409.
- `setup` runs only for a task worktree that Marshell creates, never for the main checkout (decision 3).
- Script values: a string, or `{"windows": "…", "default": "…"}`. On Windows the script is `windows`, or `default` when `windows` is missing; elsewhere it is `default`. A script with no value for this OS is absent (decision 6).
- A script runs in the default profile's shell. Windows: that profile's program (PowerShell 7 `pwsh.exe`, or Windows PowerShell) with `-NoLogo -Command <script>`. Elsewhere: `$SHELL` (or `/bin/sh`) with `-lc <script>`. The script is one argv element and is never pasted into a command string.
- `marshell.json` is read only up to 64 KiB, as UTF-8 JSON; unknown fields are ignored.
- Every new route goes in the token-guarded router and has a "refused without the bearer" test. No route takes a path to write.
- Copy is exactly as written in this plan; anything new goes to the user for approval with the results.
- Keyboard-only and forced colours work for every new control (AGENTS.md). Use the existing button and card components; no new borders or outlines.
- Use `cargo test -j 4`. Windows 11 is the only verified platform.

## Review Focus

1. **`marshell.json` changed after Allow**, including a branch whose copy differs, so a new worktree's file isn't the trusted one: the trust prompt shows again, and `POST /v1/scripts/run` with the old hash is a 409. Tasks 1 and 2 test it.
2. **A broken file**: invalid JSON, a non-string script, a file over 64 KiB, or an object with no value for this OS. No crash, and no Run button for a missing script. A problem line names the file and the parse error. Task 1 tests it.
3. **The person closes the setup tab before it ends**: the agent session still starts, and its row says `Setup was closed before it finished.` Task 5 tests the pure decision.
4. **Teardown hangs or fails**: removal goes on, after at most 5 minutes, and the result names what happened. Task 3 tests it with a short test timeout.
5. **A script with quotes, `&`, `;` or a newline** reaches the shell unchanged, as one argument. Task 1 tests the argv; Task 2 runs one through a real session.

---

### Task 1: The scripts module: read, resolve, hash, trust, argv

**Files:**
- Create: `crates/core/src/scripts.rs` (and `pub mod scripts;` in `crates/core/src/lib.rs`)
- Modify: `crates/protocol/src/api.rs` (new types below), regenerated `src/generated/*`
- Test: unit tests in `scripts.rs`

**Interfaces:**
- Produces (protocol, all `#[ts(export)]`, snake_case serde like their neighbours):
  - `pub enum ScriptKind { Setup, Run, Teardown }`
  - `pub struct RepoScripts { pub root: String, pub file: String, pub hash: Option<String>, pub setup: Option<String>, pub run: Option<String>, pub teardown: Option<String>, pub trusted: bool, pub problem: Option<String> }`: `root` is the checkout root (a worktree's own root), `file` is `<root>/marshell.json`, the scripts are already resolved for this OS, and `hash`/`problem` are `None` when there's no file.
- Produces (core, `scripts.rs`):
  - `pub const FILE_NAME: &str = "marshell.json";` and `pub const MAX_BYTES: u64 = 64 * 1024;`
  - `pub fn checkout_root(dir: &Path) -> Option<PathBuf>`: the nearest ancestor (or `dir`) holding a `.git` file or folder.
  - `pub fn read(dir: &Path, home: &Path) -> RepoScripts`: finds the root, reads and parses the file, resolves for `cfg!(windows)`, and sets `trusted` from the trust store under the main repo root (`projects::repo_root`).
  - `fn resolve(value: &ScriptValue, windows: bool) -> Option<String>`, with `#[serde(untagged)] enum ScriptValue { Text(String), PerOs { windows: Option<String>, default: Option<String> } }`. Blank strings count as absent.
  - `pub fn allow(home: &Path, repo_root: &Path, hash: &str) -> anyhow::Result<()>` and `pub fn is_trusted(home: &Path, repo_root: &Path, hash: &str) -> bool`, over `<home>/scripts-trust.json`: `{ version, entries: [{ root, hash }] }` through `store::write`/`store::read` with `Versioned`. A newer hash for the same root replaces the old entry. Paths are compared case-insensitively on Windows.
  - `pub fn argv(shell_program: &str, script: &str) -> Vec<String>`: Windows `[program, "-NoLogo", "-Command", script]`; elsewhere `[program, "-lc", script]`.
  - `pub fn default_shell() -> String`: the program of `profiles::detect::default_shell()` (its first element: `pwsh.exe` or `powershell.exe` on Windows, `$SHELL` elsewhere), or `/bin/sh` when that is empty.

- [ ] **Step 1: Write the failing tests** in `scripts.rs`:
  - `reads_strings_and_per_os_values`: the file `{"setup":"pnpm i","run":{"windows":"pnpm dev:win","default":"pnpm dev"},"teardown":{"default":"rm -rf .cache"}}` gives `setup` `pnpm i`; on Windows `run` is `pnpm dev:win` and `teardown` is `rm -rf .cache`; with `windows=false` `run` is `pnpm dev`. Call `resolve` directly for both values of `windows`.
  - `a_script_with_no_value_for_this_os_is_absent`: `{"run":{"windows":"x"}}` resolved with `windows=false` gives `None`. A blank string gives `None`.
  - `no_file_is_no_scripts_and_no_problem`: `hash` and `problem` are `None` and all scripts are `None`.
  - `a_broken_file_names_its_problem`: invalid JSON gives `problem` starting `marshell.json can't be read:`. `{"setup": 3}` gives the same prefix. A file of `MAX_BYTES + 1` bytes gives `marshell.json can't be read: it is larger than 64 KiB.`. In each case every script is `None`.
  - `checkout_root_stops_at_a_git_file_or_folder`: a `.git` folder and a `.git` file (a linked worktree) each mark their folder as the root, from a subfolder two levels down.
  - `trust_is_per_repo_and_hash`: `allow(home, root, "h1")` makes `is_trusted(root,"h1")` true and `is_trusted(root,"h2")` false. Then `allow(root,"h2")` makes h2 trusted and h1 not. Another root stays untrusted.
  - `read_reports_trust_under_the_main_repo`: a linked worktree, as in the `projects.rs` tests' fixture with `commondir`, holds the same file bytes as the trusted main checkout, so `read(worktree).trusted` is true. Change one byte and it's false.
  - `argv_keeps_the_script_as_one_argument`: `argv("pwsh.exe", "echo \"a & b\"; echo 'c'\necho d")` on Windows is exactly `["pwsh.exe","-NoLogo","-Command","echo \"a & b\"; echo 'c'\necho d"]` (cfg-gated; the unix branch asserts `["/bin/sh","-lc",…]`).
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core --lib scripts`. Expected: FAIL to compile, since nothing is defined yet.
- [ ] **Step 3: Implement** `scripts.rs` and the protocol types. Hash with `config::hash::hash_bytes`. Check `metadata().len()` before reading.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core --lib scripts` and `cargo test -j 4 -p marshell-protocol`, then `pnpm check:generated`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(scripts): read, resolve and hash a repo's marshell.json, and keep the repos whose scripts are allowed`.

### Task 2: Routes: read, allow, run

**Files:**
- Create: `crates/core/src/server/scripts.rs`
- Modify: `crates/core/src/server/mod.rs` (routes), `crates/protocol/src/api.rs` (`SessionInfo.script`, request types), `crates/core/src/server/sessions.rs` (start a script session), `crates/core/src/registry.rs` (carry `script`)
- Test: `crates/core/tests/scripts_api.rs`

**Interfaces:**
- Consumes: Task 1's `read`, `allow`, `is_trusted`, `argv`, `default_shell`, `RepoScripts`, `ScriptKind`.
- Produces:
  - `GET /v1/scripts?cwd=<dir>` returns `RepoScripts`.
  - `POST /v1/scripts/allow` takes `AllowScriptsRequest { cwd: String, hash: String }` and returns `RepoScripts`. A hash that isn't the file's current one is a 409 with `marshell.json changed. Check the scripts again.`
  - `POST /v1/scripts/run` takes `RunScriptRequest { cwd: String, script: ScriptKind, hash: String, cols: Option<u16>, rows: Option<u16> }` and returns `SessionInfo`. `Teardown` is a 422. A missing script is a 422 `marshell.json has no <setup|run> script for this computer.` A changed hash is the 409 above. An untrusted hash is a 403 `This repo's scripts aren't allowed yet.`
  - `SessionInfo.script: Option<ScriptKind>` (optional in TS). The session is named `Setup` or `Run`, its cwd is the checkout root, and its argv is `argv(default_shell(), script)`, with no agent and the default profile.
  - TS client `src/lib/api/scripts.ts`: `getScripts(ep, cwd)`, `allowScripts(ep, cwd, hash)` and `runScript(ep, req)`.

- [ ] **Step 1: Write the failing tests** in `scripts_api.rs`, using the harness the other `tests/*_api.rs` files use:
  - Each of the three routes is refused without the bearer (401).
  - `get_reports_the_file_and_untrusted`: a temp repo with `.git` and the file gives `trusted: false`, the hash, and the resolved scripts.
  - `allow_then_get_is_trusted`; `allow_with_an_old_hash_is_409_and_trusts_nothing`.
  - `run_refuses_untrusted_changed_missing_and_teardown`: the 403, 409 and two 422s, with their messages.
  - `run_starts_a_named_session_in_the_checkout_root`: once trusted, `run` with `script: "run"`, where the script writes a marker file (`Set-Content marker.txt ok` on Windows, `echo ok > marker.txt` elsewhere), returns a `SessionInfo` named `Run` with `script: "run"` and cwd the root. The marker appears within 10 s.
  - `a_script_with_quotes_and_ampersands_reaches_the_shell_whole` (Windows): the script `Set-Content out.txt "a & b; 'c'"` writes exactly `a & b; 'c'`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core --test scripts_api`. Expected: FAIL.
- [ ] **Step 3: Implement** the handlers, with file reads in `spawn_blocking` and no lock held across await. `run` re-reads the file, compares the hash and trust, then starts the session through the same `start_session` path as `sessions::create` with a custom argv. Register the routes beside `/v1/tasks` in the guarded router.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core --test scripts_api`, `cargo test -j 4 -p marshell-core --test sessions_api` (or whichever file covers `POST /v1/sessions`), `cargo test -j 4 -p marshell-protocol` and `pnpm check:generated`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(scripts): routes to read a repo's scripts, allow them, and start setup or run as a session`.

### Task 3: Teardown in the worktree's remove plan

**Files:**
- Modify: `crates/core/src/tasks.rs` (`plan_remove` ~757, `remove` ~965, `run_removal` ~902), `crates/protocol/src/api.rs` (`RemovePlan`, the remove response), `src/features/tasks/reviewRun.ts`, `src/features/tasks/ReviewSheet.tsx`
- Test: `tasks.rs` unit tests; `src/features/tasks/*.test.ts(x)`

**Interfaces:**
- Consumes: Task 1's `read`, `argv` and `default_shell`.
- Produces:
  - `RemovePlan.teardown: Option<String>` (optional in TS): the trusted teardown script, shown as the first line of the command list as `teardown: <script>`.
  - `RemoveRequest.teardown: Option<String>`: what the UI showed. The core re-plans and refuses with the existing `CHANGED` if it differs.
  - The remove route's success body gains `notes: Vec<String>`, which the sheet shows after the removal.
  - When the worktree's file has a teardown that isn't trusted, `RemovePlan.warnings` gains `marshell.json's teardown won't run: this repo's scripts aren't allowed.` and `teardown` is `None`.
  - `const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(300);` (a test seam may shorten it).

- [ ] **Step 1: Write the failing tests**:
  - `plan_remove_puts_a_trusted_teardown_first`: a task worktree with `{"teardown":"…"}` and trust gives `RemovePlan.teardown == Some(script)` and the git steps unchanged.
  - `plan_remove_warns_about_an_untrusted_teardown`: the warning above, and `teardown` is `None`.
  - `teardown_runs_in_the_worktree_before_git`: the teardown writes a marker outside the worktree (an absolute temp path) and the removal succeeds, so the marker exists and the worktree is gone.
  - `a_failed_teardown_still_removes`: a script that exits 3 gives notes `["Teardown failed (exit 3); the worktree was removed anyway."]`, and the worktree is gone.
  - `a_teardown_that_hangs_is_stopped`: with a 1 s test timeout, a sleeping script gives the note `Teardown didn't finish within 5 minutes; the worktree was removed anyway.`. Its process tree is killed, and the removal finishes.
  - Frontend: the sheet renders `teardown: <script>` as the first `CommandList` item and shows the notes after a removal.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core --lib tasks` and `pnpm test src/features/tasks`. Expected: FAIL.
- [ ] **Step 3: Implement.** `run_removal` runs the teardown first with `std::process::Command` (argv from Task 1, cwd = the worktree, no console window on Windows, output discarded), waits up to the timeout, then the git steps. It returns `Ok(notes)`.
- [ ] **Step 4: Run** the same commands. Expected: PASS.
- [ ] **Step 5: Commit** `feat(tasks): run a repo's trusted teardown before its worktree is removed`.

### Task 4: The setup result on the agent's row

**Files:**
- Modify: `crates/protocol/src/api.rs` (`CreateSessionRequest.setup`, `SessionInfo.setup`), `crates/core/src/server/sessions.rs`, `crates/core/src/registry.rs`, `src/features/sidebar/types.ts`, the row model builder and row component under `src/features/sidebar/`
- Test: `crates/core/tests/` (the sessions API test file), `src/features/sidebar/*.test.tsx`

**Interfaces:**
- Produces:
  - `pub struct SetupResult { pub tab_id: String, pub outcome: SetupOutcome }` and `pub enum SetupOutcome { Passed, Failed { code: i32 }, Closed }`.
  - `CreateSessionRequest.setup: Option<SetupResult>` is copied to `SessionInfo.setup`.
  - `RowModel.setupNote?: { text: string; tabId: string }`. Its text is `Setup failed (exit <code>).` for `Failed` and `Setup was closed before it finished.` for `Closed`; `Passed` gives no note.
  - The row shows the note with a link `Open its output`, which selects the setup tab. When that tab is gone, the link is left out.

- [ ] **Step 1: Write the failing tests**: core, a session created with `setup: Failed{code:1}` lists it back; frontend, the row model and markup for `Failed`, `Closed` and `Passed`, and no link when the tab is missing.
- [ ] **Step 2: Run** the core test and `pnpm test src/features/sidebar`. Expected: FAIL.
- [ ] **Step 3: Implement.** Use the row's existing secondary-line styling (`note`/`statusNote`), with no new colours.
- [ ] **Step 4: Run** the same commands. Expected: PASS.
- [ ] **Step 5: Commit** `feat(sessions): show a failed or closed setup on the agent's row with a link to its output`.

### Task 5: Launcher, trust card, Run and Stop

**Files:**
- Create: `src/features/scripts/ScriptsTrust.tsx` (+ `.css` only if the existing card classes don't fit), `src/features/scripts/afterSetup.ts`, `src/lib/api/scripts.ts` (if Task 2 didn't create it)
- Modify: `src/features/sessions/LiveLauncher.tsx` (`createAndStart` ~280), `src/features/launcher/Launcher.tsx` (confirm view ~254), `src/features/window/SessionHeader.tsx` (~73), `src/lib/keymap.ts` (`COMMANDS`), `src/features/sessions/LivePalette.tsx`, `src/features/sessions/LiveApp.tsx` (the after-setup watcher and the palette handlers)
- Test: `src/features/scripts/*.test.ts(x)`, plus additions to the Launcher and SessionHeader tests

**Interfaces:**
- Consumes: Task 2's API and `SessionInfo.script`; Task 4's `SetupResult`.
- Produces:
  - `ScriptsTrust({ scripts: RepoScripts, allowLabel: string, onAllow(): void, onCancel(): void, cancelLabel: string })`. It renders the heading `Allow this repo's scripts?`, then `marshell.json in <root> runs these commands on this computer:`, then a list of `setup: …`, `run: …`, `teardown: …` (only the present ones, in `<code>`, through `CommandList`), then the two buttons.
  - `setupOutcome(info: SessionInfo | undefined): SetupOutcome | null` in `afterSetup.ts`: `null` while running; `Passed` for exit 0; `Failed{code}` for any other exit; `Closed` when the tab is gone or `Ended`.
- Behaviour:
  - **Task confirm view** (launcher, after `tasksApi.plan`): `getScripts(project root)`.
    - **Untrusted file with any script:** the confirm view shows `ScriptsTrust` above the git commands, with `allowLabel` `Allow and create` and `cancelLabel` `Create without scripts`.
    - **Trusted setup:** the command list gains a last line `setup: <script>`.
    - **A `problem`:** it's shown as one line, and the task is created without scripts.
  - **After create:** with a trusted setup, `runScript({cwd: task.path, script: "setup", hash})`, focus the setup tab, and close the launcher with `Running setup in <task name>…`. When `setupOutcome` stops being `null` (watched in LiveApp over the session list), create the agent session from the launch request the launcher built, with `setup: { tab_id, outcome }`. Without setup, everything is as today.
  - **Header:** when the selected session's `getScripts(cwd)` has `run`, show a `Run` button after the port chips (its `title` is the script). If a session with `script: "run"` and the same root is running, show `Stop` instead, which closes that session (the existing close route). An untrusted file opens `ScriptsTrust` (`Allow and run` / `Cancel`) first. Re-read the scripts when the selected session changes, not on a timer.
  - **Palette:** a `run-script` command labelled `Run the repo's run script`, and `stop-script` labelled `Stop the repo's run script`, both in group `Tools` and both palette-only. They do what the header buttons do.

- [ ] **Step 1: Write the failing tests**:
  - `setupOutcome` for running, exit 0, exit 2, `Ended` and missing.
  - `ScriptsTrust` markup lists only the present scripts, with the exact heading, line and labels.
  - The launcher confirm view, untrusted, shows the trust card and both buttons; trusted with setup, its last command line is `setup: <script>`; with a problem, it shows the problem line.
  - `SessionHeader`: no button without `run`; `Run` with it; `Stop` when a run session for that root is running.
  - `COMMANDS` holds both ids with their labels.
- [ ] **Step 2: Run** `pnpm test src/features/scripts src/features/launcher src/features/window src/lib`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same, then `pnpm test` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(scripts): ask once per repo, run setup before a task's agent starts, and run or stop the dev script from the header and the palette`.

### Task 6: Live check

Use the `marshell-live-check` skill, with a scratch repo under the scratch folder holding `marshell.json`. The setup writes `setup.txt` and exits 0 (and once with `exit 2`). The run is `node -e "require('http').createServer((q,s)=>s.end('ok')).listen(5199)"` (or `python -m http.server 5199`). The teardown writes a marker outside the worktree.

Checks for `docs/research/2026-10-30-repo-scripts-live-check.md`:
1. A new task worktree shows the trust card with the exact scripts. `Create without scripts` creates the worktree and runs nothing. `Allow and create` runs setup in a `Setup` tab, then the agent starts. `setup.txt` is in the worktree.
2. Setup with `exit 2`: the agent still starts, and its row reads `Setup failed (exit 2).`, with `Open its output` selecting the Setup tab. Closing a running setup tab gives `Setup was closed before it finished.`
3. `Run` in the header starts a `Run` tab, and a port chip for 5199 appears. `Stop` ends it. The palette commands do the same.
4. Edit `marshell.json`: Run asks again.
5. Remove the task: the plan's first line is `teardown: …`. The marker appears and the worktree is gone. With the file untrusted, the warning line shows and no teardown runs.
6. Keyboard only, and emulated forced colours, on the trust card, Run/Stop and the row note.

Findings get a failing-first test and a fix on the branch, or go to the user as limits. Show the copy in this plan to the user for approval with the results.
