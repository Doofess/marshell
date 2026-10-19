# Phase 3c1: Claude Code hooks, install and remove — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marshell installs its Claude Code hooks and status line chain through the config service after a consent card, and takes them out again from Settings or from a standalone `marshell uninstall`.

**Architecture:** The Claude adapter (`crates/core/src/agents/claude/`) turns a settings file's text into a `ChangeSet`: hook entries appended at `/hooks/<Event>/-`, `/statusLine/command` swapped for the bridge, and the user's status line saved to the chain file. Previews and writes go through the existing `ConfigService`, so every change has a diff, a hash check and a snapshot. The live app gets onboarding (Detected, Consent), an Agents dialog and a row link. A new `marshell` binary links the core for uninstall. The carried bridge fixes from 3a (status line timeout, tab cleanup, status line flood, inherited Claude markers) come first.

**Tech Stack:** Rust 2024 (axum 0.8, jsonc-parser CST, windows-sys 0.61), ts-rs, React/TypeScript, Vitest.

**Spec:** `docs/PLAN.md`: §4 (hook bridge, status line chaining), §6 (config safety layer), §9 slice 3c and "Slice 3c decisions (2026-10-19)", "Config safety copy". `docs/spikes/S10-hook-path-spaces.md` (exec form, findings 3 and 4).

## Global Constraints

- Hooks are installed in the exec form: `{"type":"command","command":"<home>/bin/marshell-hook.exe","args":["event","--agent","claude","--tag","mshl1"],"timeout":5}`, with forward slashes in `command`. `SessionEnd` gets `"timeout": 2` (its maximum is 3). Each entry is its own matcher group with no `matcher` key, `{"hooks":[<entry>]}`, appended at `/hooks/<Event>/-`.
- The 8 status events, in this order: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Notification`, `Stop`, `SubagentStop`, `SessionEnd`. This matches `CAPABILITIES.claude.status.entries = 8`. No `PermissionRequest` entry until 3d.
- The status line command is the shell form: `"<home>/bin/marshell-hook.exe" statusline --agent claude --tag mshl1`, quoted, with forward slashes.
- An entry is Marshell's when its `args` hold `--tag` followed by `mshl1`, or its `command` string contains `--tag mshl1`. The bridge path is not part of the match, so an orphan from another home is still found.
- The chain file is `<marshell home>/agents/claude/statusline-chain.json` (`paths::statusline_chain`). It is written with `JsonSet /statusLine` (which creates the file when it is missing), holding the user's original `statusLine` object, or `null` when they had none.
- When the user has a status line, install changes only `/statusLine/command` and keeps the other keys. When they have none and "status" is on, install sets `/statusLine` to `{"type":"command","command":<ours>}`, so context and plan data arrive. The headline then reads "Add 8 hook entries and a status line in …".
- The "Keep my status line working" switch is offered only when the user has a status line of their own. Off means `statusLine` is not touched.
- Every `FileEdit` has `applies: NewSessions`. Its `expected_hash` is the hash of the text the plan was built from.
- Settings path: `MARSHELL_CLAUDE_SETTINGS` when it is set and non-empty, else `<user home>/.claude/settings.json`. Tests never read the real one.
- Copy: removal copy is built by `removalCopy` (PLAN "Config safety copy"). A refused file is never described as about to change. A conflict offers "Review the difference" and "Reload", never an overwrite.
- New dependencies: none. `windows-sys` 0.61, already in the lockfile, is added to `marshell-hook` under `cfg(windows)`; that is a new edge, not a new package.
- No CI change. The new binary is built by extending the `build:hook` scripts in `package.json`.
- Commit messages read `type(scope): sentence`, a blank line, then `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Run `cargo fmt --all` before each Rust commit.
- Live checks set `MARSHELL_HOME` and `MARSHELL_CLAUDE_SETTINGS` to scratch paths. The real `~/.claude` and `~/.marshell` are never written.

## Review Focus

1. **Set up run a second time** (already installed, partial, or a bridge path from another home): the entries come out once and go back in once, never duplicated. The chain file is never overwritten with Marshell's own command, which would make the bridge call itself. Pinned in Task 4 (`reinstall_replaces_entries_and_keeps_the_chain`), with the bridge's own refusal in Task 2.
2. **Status line edited by the person after install:** uninstall leaves their new `statusLine` alone and says so, and does not apply the chain. Pinned in Task 4 (`remove_leaves_a_status_line_the_user_changed`).
3. **Claude writes `settings.json` between preview and apply** (for example `/model` saves): the apply is a conflict, and the UI offers Reload rather than an overwrite. Pinned in Task 7 (`applyPlan reports a conflict on 409`) and Task 8 (`consent shows reload on a conflict`).
4. **Real-world settings text** (BOM, CRLF, comments, trailing commas, the user's own hooks in the same event): install then remove gives back the original bytes. Pinned in Task 4's byte-identity table.
5. **`hooks` that is not an object, or an event that is not an array:** the plan fails closed with the file named, and nothing is written. Pinned in Task 4 (`install_fails_closed_on_a_hooks_array`).

---

### Task 1: Session environment and per-tab hook state

**Files:**
- Modify: `crates/core/src/pty/session.rs` (`SpawnSpec` gains `env_remove`, applied before `env`)
- Modify: `crates/core/src/agents/mod.rs` (`INHERITED_MARKERS`)
- Modify: `crates/core/src/server/sessions.rs` (`spawn_into_map` passes the markers)
- Modify: `crates/core/src/hooks/log.rs`, `crates/core/src/server/hook.rs`, `crates/core/src/server/mod.rs` (`remove_session`)
- Test: unit tests beside each, plus `crates/core/tests/` if a spawn test already lives there

**Interfaces:**
- Produces: `pub const INHERITED_MARKERS: &[&str]` in `agents/mod.rs`; `SpawnSpec.env_remove: Vec<String>`; `HookLog::forget_tab(&self, tab: &str)`; `HookLog::latest_statusline(&self, tab: Option<&str>) -> Option<HookEventRecord>`.

- [ ] **Step 1: Write the failing tests**
  - `agents::tests::markers_name_the_session_identity_only`: the list equals `["CLAUDECODE","CLAUDE_CODE_ENTRYPOINT","CLAUDE_CODE_CHILD_SESSION","CLAUDE_CODE_SESSION_ID","CLAUDE_CODE_SESSION_ATTENDED","CLAUDE_CODE_MESSAGING_SOCKET","CLAUDE_CODE_MESSAGING_TOKEN","CLAUDE_CODE_EXECPATH","CLAUDE_CODE_SSE_PORT","CLAUDE_PID"]`, and contains neither `CLAUDE_CODE_GIT_BASH_PATH` nor `CLAUDE_CODE_USE_BEDROCK`.
  - `pty::session` spawn test (Windows, `cmd /c set`, the pattern of the existing env tests): with `CLAUDE_CODE_CHILD_SESSION=1` set in the test process and named in `env_remove`, the output has no `CLAUDE_CODE_CHILD_SESSION=`. A name that is also in `env` keeps the `env` value, so a profile can still set it on purpose.
  - `hooks::log::tests::status_lines_keep_only_the_latest`: push 300 `Statusline` records for tab `t1`. `recent(Some("t1"))` is empty, and `latest_statusline(Some("t1"))` has the last body. Untabbed status lines behave the same under `None`.
  - `hooks::log::tests::forget_tab_drops_ring_and_status_line`.
  - `server` test: closing a session (`DELETE /v1/sessions/{tab}`) leaves `GET /v1/hook/events?tab=` empty for that tab.
- [ ] **Step 2: Run them and watch them fail**
  Run: `cargo test -p marshell-core markers status_lines forget_tab` (plus the spawn and server test names).
  Expected: compile errors or failures for the missing items.
- [ ] **Step 3: Implement**
  - `push` files a `HookRole::Statusline` record in a per-tab `latest` map (and an untabbed slot), never in a ring.
  - `AppState::remove_session` also calls `holds().release_tab(tab)` and `hook_log().forget_tab(tab)`. It is the one place every close, restart and task removal goes through.
  - `Session::spawn` calls `cmd.env_remove(k)` for each `env_remove` name before the `env` loop.
- [ ] **Step 4: Run the tests and watch them pass**
  Run: `cargo test -p marshell-core`
  Expected: all pass.
- [ ] **Step 5: Commit** `fix(core): drop Claude's session markers from a new session, keep one status line per tab, and clear a closed tab's hook state`

### Task 2: The bridge's status line: timeout, process tree, empty chain

**Files:**
- Modify: `crates/hook/Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_Foundation","Win32_Security","Win32_System_JobObjects","Win32_System_Threading"] }`)
- Create: `crates/hook/src/job.rs` (a kill-on-close Job Object, modelled on `crates/core/src/job.rs`; a no-op off Windows)
- Modify: `crates/hook/src/statusline.rs`

**Interfaces:**
- Produces: `const ORIGINAL_TIMEOUT: Duration = Duration::from_secs(10)`; `read_chain(home, agent) -> Result<Option<String>, String>`, where `Ok(None)` means the chain says `"statusLine": null`.

- [ ] **Step 1: Write the failing tests** (in `statusline.rs`)
  - `null_chain_prints_nothing_and_logs_nothing`: the chain file is `{"statusLine": null}`. `run` returns empty bytes and `logs/bridge.log` does not exist.
  - `a_chain_naming_the_bridge_is_refused`: the chain command contains `--tag mshl1`. `run` returns empty bytes and logs one line that says the chain names Marshell's own status line. It does not run it.
  - `a_hung_original_is_cut_off`: the timeout is injected as 300 ms (make `run_original` take a `timeout: Duration`). The command `ping -n 30 127.0.0.1 >NUL` on Windows (`sleep 30` elsewhere) returns `Err` in under 2 s, and the log line says it took longer than the limit.
  - `a_quick_original_still_prints`: the existing echo test passes unchanged.
- [ ] **Step 2: Run and watch them fail**
  Run: `cargo test -p marshell-hook statusline`
  Expected: the three new tests fail.
- [ ] **Step 3: Implement**
  - The child is spawned suspended (`CREATE_SUSPENDED | CREATE_NO_WINDOW`), assigned to the job, then resumed, so a grandchild can't escape. If that is not possible through `std::process`, spawn normally and assign right away, and note it as a ruling.
  - Stdout is read on a thread. The wait is `try_wait` polling against the deadline. On timeout the job is terminated and the function returns `Err("the original status line took longer than 10 s")`.
- [ ] **Step 4: Run** `cargo test -p marshell-hook`. Expected: all pass.
- [ ] **Step 5: Commit** `fix(hook): stop a hung status line after 10 s with its whole process tree, and print nothing for an empty chain`

### Task 3: Protocol types and the onboarded flag

**Files:**
- Create: `crates/protocol/src/agents.rs`, and add `pub mod agents;` to `lib.rs`
- Modify: `crates/protocol/src/api.rs` (`Settings.onboarded: bool` with `#[serde(default)]`)
- Modify: `crates/core/src/settings.rs` (`FileDoc.onboarded`, the `pause_warned` pattern)
- Modify: `src/features/appearance/settingsSync.ts` (`markOnboarded()`; `markPauseWarned` and `save` send `onboarded` back unchanged, and `markOnboarded` sends `pause_warned` back)
- Regenerated: `src/generated/*` via `cargo test -p marshell-protocol`

**Interfaces:**
- Produces (all `#[ts(export)]`, numbers as `u32`, `Option` fields `#[ts(optional)]` with `skip_serializing_if`):
  ```rust
  #[serde(rename_all = "camelCase")] pub enum HookInstall { None, Installed, Partial, Orphaned }
  #[serde(rename_all = "camelCase")] pub enum SettingsState { Ok, Missing, Unparsable, ReadOnly }
  pub struct ParseProblem { pub line: u32, pub message: String }
  pub struct AgentSetup {
      pub agent: String,               // "claude"
      pub found: bool,                 // a detected profile runs it
      pub version: Option<String>,
      pub exe_path: Option<String>,
      pub settings_path: String,
      pub settings_state: SettingsState,
      pub parse_error: Option<ParseProblem>,
      pub hooks: HookInstall,
      pub has_own_statusline: bool,    // a status line that is not Marshell's, or the one the chain keeps
      pub chained: bool,               // ours is in place and the chain holds the user's
      pub bridge_path: String,
      pub snapshots_dir: String,
  }
  pub struct HookPlanRequest { pub status: bool, pub statusline: bool }
  ```
  `Partial` means some of the 8 events or the status line are missing, or entries carry a different bridge path. `Orphaned` means entries are present but the file at `bridge_path` is missing.
- Produces (TS): `SettingsSync.markOnboarded(): Promise<void>`; `Settings.onboarded` in `src/generated/Settings.ts`.

- [ ] **Step 1: Write the failing tests**
  - `settings::tests::onboarded_round_trips_and_defaults_false`: an old file without the key loads with `onboarded == false`.
  - `settingsSync.test.ts`: "markOnboarded sends onboarded true and keeps pause_warned", and "markPauseWarned keeps onboarded".
- [ ] **Step 2: Run** `cargo test -p marshell-core settings` and `pnpm test src/features/appearance/settingsSync.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement**, then run `cargo test -p marshell-protocol` to regenerate.
- [ ] **Step 4: Run** `cargo test -p marshell-core settings`, `pnpm test src/features/appearance`, `pnpm check:generated`. Expected: pass, exit 0.
- [ ] **Step 5: Commit** `feat(protocol): describe an agent's hook setup, and remember that onboarding was seen`

### Task 4: The Claude adapter's install and remove plans (effort: high)

**Files:**
- Move: `crates/core/src/agents/claude.rs` → `crates/core/src/agents/claude/mod.rs` (the content stays)
- Create: `crates/core/src/agents/claude/hooks.rs`
- Create: `crates/core/tests/fixtures/claude/settings/*.json` (one per byte-identity case below; invented content, no real e-mail or paths)
- Modify: `crates/core/src/config/jsonc.rs` (`pub fn parse_problem(text: &str) -> Option<(u32, String)>`: the 1-based line of the first parse error)

**Interfaces:**
- Consumes: `config::jsonc::{apply_ops, read_value, JsonOp}`, `config::hash::hash_bytes`, `paths::{bridge_exe, statusline_chain, snapshots_dir}`, the `marshell_protocol::config` types, and Task 3's `HookInstall`.
- Produces, in `agents::claude::hooks`:
  ```rust
  pub const TAG: &str = "mshl1";
  pub const STATUS_EVENTS: [&str; 8];
  pub fn settings_path(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf;
  pub fn statusline_command(home: &Path) -> String;
  pub struct Found { pub install: HookInstall, pub has_own_statusline: bool, pub chained: bool }
  pub fn inspect(settings: &Value, chain: Option<&Value>, home: &Path) -> Found;
  pub enum PlanError { DoesNotParse { path: PathBuf, problem: (u32, String) }, NotAnObject { path: PathBuf, pointer: String } }
  pub fn install_plan(settings_path: &Path, text: Option<&str>, chain_text: Option<&str>, home: &Path, want: &HookPlanRequest) -> Result<ChangeSet, PlanError>;
  pub fn remove_plan(settings_path: &Path, text: Option<&str>, chain_text: Option<&str>, home: &Path) -> Result<RemovePlan, PlanError>;
  pub struct RemovePlan { pub changes: ChangeSet, pub statusline_left: bool } // true: the user changed it since; it is left alone
  ```
  `text: None` means the file is missing. `ChangeSet.title` is "Set up Claude Code hooks" or "Turn Claude Code hooks off", and `reason` is one plain sentence.

- [ ] **Step 1: Write the failing tests.** Each test applies the plan's ops with `apply_ops` to the fixture text, the way `preview` does.
  - `settings_path_prefers_a_non_empty_override`: `MARSHELL_CLAUDE_SETTINGS=""` is ignored.
  - `install_appends_one_group_per_event`: in an empty `{}`, the 8 events appear in `STATUS_EVENTS` order, each with one exec-form entry (Global Constraints values), and SessionEnd has timeout 2.
  - `install_keeps_the_users_hooks_first`: in a fixture with a user `Stop` hook, the user's group is still at index 0.
  - `install_chains_an_existing_status_line`: only `/statusLine/command` changes (`padding` and `refreshInterval` are kept), and the chain edit's value is the original object.
  - `install_adds_a_status_line_when_there_is_none`: the chain value is `null`.
  - `statusline_off_leaves_status_line_alone`: `HookPlanRequest{status:true, statusline:false}` with a user status line has no `/statusLine` op and no chain edit.
  - `reinstall_replaces_entries_and_keeps_the_chain` (Review Focus 1): install, then install again over the result, gives 8 entries, not 16. The chain edit is absent or still holds the user's original command, never one containing `--tag mshl1`. The same holds for a fixture whose entries name another home's bridge.
  - `install_fails_closed_on_a_hooks_array` (Review Focus 5): `"hooks": []` and `"hooks": {"Stop": {}}` give `PlanError::NotAnObject` with the pointer.
  - `unparsable_settings_name_the_line`: gives `PlanError::DoesNotParse` with the right line.
  - `remove_leaves_a_status_line_the_user_changed` (Review Focus 2): after install the command is edited by hand, and remove gives `statusline_left == true` with no `/statusLine` op.
  - `remove_finds_orphans_by_tag`: entries tagged `mshl1` with a bridge path that does not exist are removed. An entry merged by hand into a user group is removed alone, and the group stays.
  - `inspect_states`: one case each for None, Installed, Partial (7 events), Orphaned (bridge file missing), and chained true/false.
  - `byte_identity_install_then_remove` (Review Focus 4), table-driven over these fixtures. For each, `remove(install(text)) == text`, byte for byte:
    1. empty object `{}\n`
    2. user hooks in Stop and PreToolUse
    3. comments and trailing commas
    4. CRLF
    5. BOM
    6. status line with a comment inside the object
    7. no status line
    8. `"hooks": {}` already present

    Where removing the containers it emptied cannot give back the exact bytes, the test asserts value identity for that fixture only, with a comment saying why, and the executor records a ruling.
- [ ] **Step 2: Run** `cargo test -p marshell-core agents::claude`. Expected: FAIL (the module is missing).
- [ ] **Step 3: Implement.**
  - Install builds its removal ops first (descending array indices, so earlier removals don't shift later ones), then the appends.
  - Remove deletes Marshell's entries, then any group, event key or `hooks` object it left empty.
  - When `/statusLine/command` is ours and the chain holds an object, remove sets `/statusLine/command` back to the chain's `command`. When the chain is `null`, it removes `/statusLine`.
  - The chain edit is a `FileEdit` on `paths::statusline_chain(home, "claude")` with `scope: Marshell`.
- [ ] **Step 4: Run** `cargo test -p marshell-core`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(core): plan the Claude Code hook install and removal as config changes`

### Task 5: Agent routes and detection

**Files:**
- Create: `crates/core/src/server/agents.rs`
- Modify: `crates/core/src/server/mod.rs` (routes, plus a version cache in `Inner`)
- Modify: `crates/core/src/agents/claude/mod.rs` (`parse_version`, `run_version`)
- Modify: `crates/core/src/profiles/detect.rs` (`pub fn agent_exe(profiles: &[Profile], agent: &str) -> Option<PathBuf>`)

**Interfaces:**
- Consumes: Task 4's `hooks::*`, `ConfigService::preview`, Task 3's types.
- Produces routes, all behind the token:
  - `GET /v1/agents/{agent}/setup` → `AgentSetup`.
  - `POST /v1/agents/{agent}/hooks/plan`, body `HookPlanRequest` → `ConfigPreview`.
  - `POST /v1/agents/{agent}/hooks/remove-plan` → `{ preview: ConfigPreview, statusline_left: bool }`. Name the type `RemovePreview` in `protocol::agents`.
  - An `{agent}` other than `claude` gives 404. A `PlanError` gives 422, with the message `"<path> does not parse (line N), so Marshell will not edit it."` or `"<path> has a <pointer> that is not an object, so Marshell will not edit it."`.
  Applying uses the existing `POST /v1/config/apply`.
- Produces: `parse_version(stdout: &str) -> Option<String>` (the leading semver of `2.1.302 (Claude Code)`); `run_version(exe: &Path, timeout: Duration) -> Option<String>`, with a 5 s timeout and `CREATE_NO_WINDOW`, cached per exe path for the app's lifetime.

- [ ] **Step 1: Write the failing tests**
  - `parse_version_takes_the_leading_semver`: covers `"2.1.302 (Claude Code)\n"`, `"garbage"` → None, and `"v2.1.0"` → None.
  - `agent_exe_finds_the_claude_profile`.
  - Router tests with `MARSHELL_HOME` and `MARSHELL_CLAUDE_SETTINGS` in a temp dir:
    - `setup_reports_missing_file`
    - `plan_then_apply_writes_the_hooks`: preview, then `/v1/config/apply` with the preview's hashes, then setup reports `installed`.
    - `remove_plan_after_install_restores_the_file`: byte-equal to the start.
    - `unknown_agent_is_404`
    - `unparsable_settings_is_422_with_the_line`
- [ ] **Step 2: Run** `cargo test -p marshell-core agents`. Expected: FAIL.
- [ ] **Step 3: Implement.** The router tests must not run a real `claude`; with no detected profile, `version` is `None`.
- [ ] **Step 4: Run** `cargo test -p marshell-core` and `pnpm check:generated`. Expected: pass, exit 0.
- [ ] **Step 5: Commit** `feat(core): serve Claude Code's hook setup state and its install and removal previews`

### Task 6: The `marshell` binary and `marshell uninstall`

**Files:**
- Create: `crates/cli/Cargo.toml` (package `marshell-cli`, `[[bin]] name = "marshell"`, depends on `marshell-core` and `marshell-protocol`), `crates/cli/src/main.rs`, `crates/cli/src/args.rs`, `crates/cli/src/uninstall.rs`
- Modify: the root `Cargo.toml` workspace members; `package.json` (`build:hook` and `build:hook:release` also build `-p marshell-cli`)
- Modify: `crates/core/src/bridge.rs` (`install` and `sweep_leftovers` take the exe name, plus `pub const CLI_EXE: &str = brand::CLI_BIN`), `crates/core/src/paths.rs` (`cli_exe(home)`), and the app's startup call site in `src-tauri` (copy both)

**Interfaces:**
- Consumes: Task 4's `remove_plan`, `ConfigService::{preview, apply}`, `paths::home_dir`.
- Produces:
  - `args::parse(argv: &[String]) -> Result<Command, String>`, with `enum Command { Uninstall { yes: bool }, Help, Version }`.
  - `uninstall::run(home: &Path, user_home: &Path, env: &dyn Fn(&str) -> Option<String>, input: &mut dyn BufRead, out: &mut dyn Write, yes: bool) -> i32`.
  - Exit codes: 0 done or nothing to do, 1 refused, failed or declined, 2 usage error.

- [ ] **Step 1: Write the failing tests**
  - `args`: covers `[]` → Err (usage), `["uninstall"]`, `["uninstall","--yes"]`, `["--help"]`, `["--version"]`, and `["uninstall","--bogus"]` → Err.
  - `uninstall` (temp home and settings):
    - `prints_the_diff_and_stops_on_no`: input `"n\n"`, the file is unchanged, exit 1, and the output holds the unified diff and `Nothing changed.`
    - `yes_flag_applies_and_removes_bin`: the file is back to its original bytes, `<home>/bin` is gone, the rest of `<home>` stays, and the output names the snapshot id.
    - `nothing_installed_says_so`: exit 0, with "Marshell has no hooks in <path>."
    - `refused_file_exits_1`: an unparsable settings file.
    - `status_line_left_is_reported`: the user changed it, so the output says it was left alone.
  - `bridge::tests`: the existing tests pass with the name parameter, plus `installs_the_cli_beside_the_bridge`.
- [ ] **Step 2: Run** `cargo test -p marshell-cli` and `cargo test -p marshell-core bridge`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - The prompt is `Apply these changes? [y/N] `. Only `y` or `yes`, case-insensitive, applies.
  - The bin folder is removed file by file. A file that can't be removed (the running `marshell.exe`) is printed as `Couldn't remove <path>; delete it once this window is closed.`, and the exit stays 0.
- [ ] **Step 4: Run** `cargo test --workspace` and `pnpm build:hook`. Expected: pass, and `target/debug/marshell.exe` exists.
- [ ] **Step 5: Commit** `feat(cli): add marshell uninstall, which shows the diff, asks, and takes the hooks and the bridge out`

### Task 7: Frontend API client and the consent card's live inputs

**Files:**
- Create: `src/lib/api/agents.ts`, `src/lib/api/agents.test.ts`
- Modify: `src/features/onboarding/types.ts` (`Detection.hasOwnStatusLine?: boolean`), `consent.ts` (headline and removal copy for an added status line), `ConsentCard.tsx` (`offer?: Capability[]`, `busy?: boolean`, `onApply?`, `onSkip?`, `error?: string`, `conflict?: { onReload: () => void }`), and their tests

**Interfaces:**
- Consumes: `api<T>`, `ApiError` from `client.ts`; the generated `AgentSetup`, `ConfigPreview`, `RemovePreview` and `ApplyResult`.
- Produces:
  ```ts
  getSetup(ep: Endpoint, agent: "claude"): Promise<AgentSetup>
  planInstall(ep, agent, want: { status: boolean; statusline: boolean }): Promise<ConfigPreview>
  planRemove(ep, agent): Promise<RemovePreview>
  applyPreview(ep, preview: ConfigPreview): Promise<{ ok: true; result: ApplyResult } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string }>
  toDetection(s: AgentSetup): Detection
  previewDiff(p: ConfigPreview): string   // the files' diffs joined, in order
  ```
  `applyPreview` sends `expected_hashes` from each file's `before_hash`.

- [ ] **Step 1: Write the failing tests**
  - `agents.test.ts`:
    - "applyPreview sends each file's before hash"
    - "applyPlan reports a conflict on 409" (Review Focus 3)
    - "toDetection maps unparsable with its line"
    - "previewDiff joins file diffs"
  - `consent.test.ts`:
    - With `hasOwnStatusLine: false` and status on, the headline is `Add 8 hook entries and a status line in <path>.`
    - The removal copy for that case is `Marshell removes the hook entries and the status line it added; the rest of the file stays as it is.`
    - The existing copy cases still pass.
  - `ConsentCard.test.tsx`:
    - `offer={["status","statusline"]}` renders no "Approve from anywhere" switch.
    - With `hasOwnStatusLine: false`, no "Keep my status line working" switch.
    - `conflict` renders "Review the difference" and "Reload", and no apply button.
- [ ] **Step 2: Run** `pnpm test src/lib/api/agents.test.ts src/features/onboarding`. Expected: FAIL.
- [ ] **Step 3: Implement.** The stories keep working with the defaults: `offer` is all three capabilities.
- [ ] **Step 4: Run** `pnpm test src/lib/api src/features/onboarding`. Expected: pass.
- [ ] **Step 5: Commit** `feat(onboarding): feed the consent card from the core's preview, and say when a status line is added rather than chained`

### Task 8: Live onboarding, the Agents dialog and the row link

**Files:**
- Create: `src/features/onboarding/LiveOnboarding.tsx` (+ `.test.tsx`)
- Create: `src/features/agents/AgentsDialog.tsx`, `AgentsDialog.css`, `agentsState.ts` (+ tests)
- Modify: `src/features/sessions/LiveApp.tsx` (overlay `"agents"`, the palette command "Agents…", the first-launch gate), `src/features/sessions/toRow.ts`, `src/components/SessionRow/SessionRow.tsx` (`onSetup?: () => void`)

**Interfaces:**
- Consumes: Task 7's client and `ConsentCard` props; `SettingsSync.markOnboarded`; `useDialogFocus` and `trapKey`; `Detected`, `OnboardingFrame`.
- Produces:
  - `agentsState.ts`: `setupLine(s: AgentSetup): string` and `actions(s: AgentSetup): Array<"setup" | "hooksOff">`, as pure functions with the copy below.
  - `LiveOnboarding({ endpoint, onDone })`. Steps are Detected, then Consent. Applying or "Skip: use limited status" calls `markOnboarded` and `onDone`.
  - `AgentsDialog({ endpoint, onClose, onNotice?, startWith?: "setup" })`.
  - `toRow(info, setup?: { claude?: HookInstall })` gives the phrase `Status limited` and `setupLink: true` for a `claude` row when the install is not `installed`.
- Copy (`setupLine`):
  - none: `Not set up. Sessions show limited status.`
  - installed: `Hooks on in <path>. New sessions report status.`
  - partial: `Some hooks are missing in <path>. Set up again to repair them.`
  - orphaned: `Marshell's hooks are in <path>, but the bridge at <bridge_path> is missing, so Claude Code reports a failing hook each time one fires. Turn the hooks off, or set up again.`
  - unparsable: the `consentHeadline` text.
  - Not found: `Claude Code was not found on this computer.` with no actions.
  - The Hooks off flow: `planRemove`, then `DiffView` and a "Turn hooks off" button. On success the notice reads `Hooks off. Sessions started from now show limited status.`, plus `Your status line was changed since setup, so it was left as it is.` when `statusline_left`.

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup` and pure functions, as the repo does)
  - `agentsState.test.ts`: one case per `setupLine` copy, and `actions` for each state (orphaned gives both actions; not found gives none).
  - `toRow.test.ts`:
    - "a claude row without hooks links to set up"
    - "an installed claude row has no link"
    - "a shell row never has one"
  - `SessionRow.test.tsx`: `onSetup` renders a button named "Set up status for <name>" whose visible text is "set up". It is reachable by Tab, and it is not rendered without `onSetup`.
  - `LiveOnboarding.test.tsx`: the first step renders Detected with the Claude row. The consent step renders the card with the preview diff.
  - "consent shows reload on a conflict" (Review Focus 3).
  - `AgentsDialog.test.tsx`: the orphaned state renders the warning and both buttons. The dialog has `role="dialog"` and an accessible name "Agents".
- [ ] **Step 2: Run** `pnpm test src/features/agents src/features/onboarding src/features/sessions/toRow.test.ts src/components/SessionRow`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - LiveApp reads `onboarded` from the settings sync at start. When it is false, LiveOnboarding is the overlay.
  - The setup state is fetched at start, after each apply, and when the Agents dialog closes.
  - The row link opens `AgentsDialog` with `startWith="setup"`.
  - Forced colours: buttons and the warning use system colours by token (`frontend-design` decides the look; read the `good-css` skill if installed).
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: all pass, exit 0.
- [ ] **Step 5: Commit** `feat(agents): set up and turn off Claude Code hooks from onboarding, the session row and an Agents dialog`

### Task 9: Live check

**Files:**
- Create: `docs/research/2026-10-19-claude-hooks-install-live-check.md`

- [ ] **Step 1: Prepare the scratch folders**
  - A scratch `MARSHELL_HOME` under `%TEMP%` (path with a space).
  - A scratch project `%TEMP%\marshell-3c1 project\` whose `.claude\settings.local.json` holds:
    - a comment
    - a user `Stop` hook (`echo stop`)
    - `"statusLine": {"type":"command","command":"echo user-line","padding":0}`
  - Record the file's hash.
- [ ] **Step 2: Start the app.** Run `pnpm build:hook` and `pnpm tauri dev` in the background, with `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS=<project>\.claude\settings.local.json` and `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`. Record the PID.
- [ ] **Step 3: Check over CDP.** Use small JSON evaluations, and screenshot only to judge the look.
  1. Onboarding shows Detected with the Claude version, then Consent. The headline reads `Add 8 hook entries and chain your status line in …`, and the diff shows both files.
  2. Apply. The settings file has 8 tagged groups after the user's Stop hook, and the chain file holds `echo user-line`.
  3. A shell session: `set CLAUDE` shows no `CLAUDE_CODE_CHILD_SESSION` (the app was started from a Claude Code session, so the variable was inherited).
  4. A Claude session in the scratch project, started for this check:
     - `GET /v1/hook/events?tab=` shows SessionStart.
     - The ring holds no status line records.
     - The latest status line is present.
     - Claude's status line shows `user-line`.
     - Sending one prompt and pressing Enter is allowed in this session only.
  5. Agents dialog, then Hooks off, then apply. The settings file hash equals the recorded one.
  6. Set up again, then close the app. `<home>/bin/marshell.exe uninstall` answered `n` changes nothing. `uninstall --yes` restores the hash, removes `bin`, and keeps the rest of the home.
  7. Install, delete `<home>/bin`, restart the app. The Agents dialog shows the orphaned warning.
  8. Forced colours (emulated): the dialog and the row link stay readable.
- [ ] **Step 4: Clean up.** Stop what was started, by PID. Remove the scratch folders.
- [ ] **Step 5: Write up and commit.** Record the results table and any finding in the research doc. Commit with `docs(research): live check of the Claude Code hook install and removal`.
