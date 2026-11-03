# Slice 6l1: the Qwen Code adapter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qwen Code (`qwen`) gets what Claude Code, Codex and Antigravity have. That means hooks and a chained status line, installed and removed through the config service, and live status in its row. Its permission dialogs are answered from the approve card, Departures and banners. Resume and Restart pick up the tab's exact conversation, and project references reach its model. Every piece is checked first against a real Qwen 0.26.5 driven by a fake model.

**Architecture:** Qwen leaves the manifest tier: its entry in `manifests.json` goes, and it gets a `"qwen"` arm wherever `"antigravity"` has one today. A new `crates/core/src/agents/qwen/` module is shaped like `agents/claude/`, because Qwen's hooks are Claude-shaped: the `hooks` key in a `settings.json`, snake_case bodies with `hook_event_name` and `session_id`, and a `PermissionRequest` answered with `decision.behavior`. Its files are `hooks.rs` (the plan for the hooks and `ui.statusLine` in Qwen's user `settings.json`, with the chain file), `status.rs` (events and status line folded into `LiveStatus`), `request.rs` (the card's view and Qwen's answer JSON) and `mod.rs` (version gate, context output). The core's routes, holds, history and resume stay agent-neutral and gain a `"qwen"` arm. The bridge needs only its context shape and its status-line shell for `qwen`.

**Tech Stack:** Rust (axum, serde_json, the `config` service and its JSONC edits), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`. The fake model server is a Node script, run with the `node` that pnpm already needs.

**Spec:** `docs/PLAN.md` §9, "Slice 6l1 decisions (2026-11-03)", and "Session modes" and "Config safety copy". Evidence: the source read summarised under "Assumptions" below. Task 1 writes `docs/research/2026-11-03-qwen-code-hooks.md`. The pattern to follow is `docs/superpowers/plans/2026-10-26-phase-6d-antigravity-adapter.md`.

**Checked before planning (2026-11-03):**
- Qwen Code 0.26.5 is installed standalone. `%LOCALAPPDATA%\qwen-code\bin\qwen.cmd` calls `qwen-code\bin\qwen.cmd`, which runs `qwen-code\node\node.exe qwen-code\lib\cli-entry.js`. `~/.qwen` does not exist: Qwen has never run on this machine.
- The live check already starts the app with `USERPROFILE=<Scratch>\home`, so Qwen, which reads `os.homedir()`, uses `<Scratch>\home\.qwen`.
- Qwen does not run Claude Code's hooks. It reads `~/.claude/settings.json` only to import MCP servers. So the Grok filter (6b decision 7) is not needed.

## Global constraints

- Marshell writes only `<QWEN_HOME or user home\.qwen>\settings.json` (its `hooks` entries and `ui.statusLine`) and its own `~/.marshell/agents/qwen/statusline-chain.json`. It never writes a project `.qwen/settings.json`, `C:\ProgramData\qwen-code\settings.json`, `.env`, `oauth_creds.json` or `tools.approvalMode` (decision 3).
- Every write goes through the config service: preview, unified diff, backup, atomic write, `managed.json`, undo. Every JSON edit is a CST edit through the existing `config::jsonc` helpers. A file that doesn't parse fails closed. The person's own hooks, keys, comments, BOM and CRLF stay byte-equal outside Marshell's entries.
- Marshell's entries are tagged as every agent's are (`--tag mshl1` in the command; `common::is_ours`).
- Tests and live checks never read or write the real `~/.qwen`, `~/.claude`, `~/.codex` or `~/.marshell`. Tests inject paths with `AppState::with_qwen_home(<temp>)`.
- No new dependency, in Rust or in `package.json`. The fake model server uses Node's built-in `http` only.
- New protocol fields go in `crates/protocol`, and `pnpm check:generated` passes afterwards.
- Never hold a `MutexGuard` across `.await` or file I/O. `qwen --version` and file reads run in `spawn_blocking`.
- Fixtures come from the real Qwen in Task 1 and are scrubbed with `pnpm scrub:fixtures` before they're committed (`fixtures_scrubbed.rs` enforces this).
- Qwen's conventions stay in `agents/qwen/`. `agents/claude/` gains no Qwen branch. A helper both need moves to `agents/common.rs`.
- The core never keeps a hook body's `prompt` (it is taken out before the log, fold, history or hold sees it, as for Claude Code) or `last_assistant_message`.
- Never name a test binary or test copy after a real CLI. Use `qwen-marshell-test`, not `qwen`.
- Every UI change works with the keyboard alone and in forced colours. Qwen gets its own consent card, with no bulk allow.
- Copy, exactly:
  - Change titles: "Set up Qwen Code hooks", "Turn Qwen Code hooks off".
  - Status notes: "Live status is off for Qwen Code"; "Qwen Code's version could not be read"; "Qwen Code <v> is older than the versions Marshell knows"; "Qwen Code <v> is not one Marshell has recorded yet". These come from `common::version_gate` with the name "Qwen Code".
  - "Always allow" refused (decision 5): "Qwen Code can't keep an always-allow rule from Marshell. Allow this once instead."
  - Removal copy: "Marshell removes the hook entries it added and puts your own status line back from statusline-chain.json; the rest of settings.json stays as it is".
  - A preset status line: "Your Qwen Code status line is a preset, so Marshell leaves it as it is. Rows show context % once per turn."

## Assumptions Task 1 confirms or corrects

These come from reading the 0.26.5 bundle. Task 1 checks each one against a real Qwen and records the evidence in `docs/research/2026-11-03-qwen-code-hooks.md`. If a finding differs, Task 1 amends this plan (shapes, constants, copy) and commits the change before Task 2 starts.

**Amended after Task 1 (2026-11-03).** The research note has each answer. What changed in the tasks below:
- Q2: the plain quoted form fails, so Task 3 uses Antigravity's `cmd_form`.
- Q3: `UserPromptSubmit` also carries `submitted_prompt`, which is taken out with `prompt`. A tool result sent back to the model fires a second `UserPromptSubmit` with `prompt: ""`. `PermissionRequest` carries no tool id. `idle_prompt` also fires mid-turn, 10 to 20 ms after each `PreToolUse`. A "No" in Qwen's own dialog sends no `Stop`.
- Q7: the status line has no mode. With a model Qwen doesn't know, `context_window_size` is 0, so context % comes from `Stop`.
- Qwen writes `"$version": 4` into the user `settings.json` on its first run, keeping the rest of the file.
- Qwen installs a found update when a session exits. A live check runs Qwen with `general.enableAutoUpdate: false`, which `live-check.ps1 -QwenFake` writes.

- **Q1. File and shape.** Qwen reads the hooks from the user settings at `<QWEN_HOME>\settings.json`, where `QWEN_HOME` defaults to `~/.qwen`. The file is JSONC, there's no trust step, and the hooks load at session start.
  - Shape: `{"hooks":{"<Event>":[{"matcher":"<regex>","hooks":[{"type":"command","command":"<string>","timeout":<secs>}]}]}}`. A `timeout` of 1000 or more is read as milliseconds, and the default is 60.
  - The finding also records whether the events without a tool (`SessionStart`, `Stop`) accept a group with no `matcher`.
- **Q2. The shell.** With no `shell` key, Windows runs the command through `%ComSpec% /d /s /c`.
  - The command under test is `"<bridge>" event --agent qwen --tag mshl1`, with the bridge path in double quotes, forward slashes and a space in a folder name. It should work as written.
  - The finding records what a `%`, `&`, `^` or `"` in the path does. If the plain form fails, the plan adopts Antigravity's `cmd_form` (moved to `common.rs`) and its two refusal reasons with "Qwen Code" in place of "Antigravity".
- **Q3. Bodies.** Every body carries `session_id`, `transcript_path`, `cwd`, `hook_event_name` and `permission_mode`. `permission_mode` reads `default|plan|auto_edit|auto|yolo`, except in `PermissionRequest`, where it reads `auto-edit`.
  - `UserPromptSubmit` adds `prompt`.
  - `PreToolUse` and `PermissionRequest` add `tool_name` and `tool_input`. The tool ids are `run_shell_command` (`command`), `write_file` (`file_path`, `content`), `edit` or `replace` (to confirm), and `read_file`.
  - `Notification` adds `notification_type`: `permission_prompt`, `idle_prompt` or `auth_success`.
  - `Stop` adds `context_usage` (a ratio) and `context_limit`. `SessionStart` adds `source` and `model`.
  - `MARSHELL_TAB_ID` and `MARSHELL_HOME` reach the hook process, and two Qwen tabs report their own tab ids.
- **Q4. `PermissionRequest`.** It fires only when Qwen is about to show its dialog. Qwen waits for the hook before it shows the dialog.
  - `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}` runs the call with no dialog.
  - `"deny"` with a `message` refuses it, and the model sees the message.
  - Empty stdout with exit 0, a non-zero exit other than 2, or a timeout shows Qwen's own dialog.
  - It never fires in `yolo`. In `auto-edit` it doesn't fire for an edit. In `auto` it fires only when the classifier falls back.
  - The finding records exit 2 too, and whether `updatedPermissions` is ignored (decision 5).
- **Q5. Neutral answers.** For every other event, empty stdout with exit 0 leaves Qwen as it would be with no hook. A bridge that can't start (exit 1 from cmd) is non-blocking.
- **Q6. Context.** `UserPromptSubmit` answered with `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"<note>"}}` reaches the model. The fake server logs the request it gets, so the note shows in it. Decision 7 depends on this.
- **Q7. Status line.** `ui.statusLine` is `{"type":"command","command":"<string>"}`, run through `cmd.exe` with a 5 s timeout and 300 ms debounce. Its stdin holds `session_id`, `version`, `model.display_name`, `context_window.used_percentage` and `context_window.context_window_size`.
  - The finding records the exact keys and whether `type: "preset"` exists in 0.26.5.
- **Q8. Sessions.** `--resume <id>` reopens that session in a new process when Qwen is started from the same folder. `transcript_path` is `<QWEN_HOME>\projects\<sanitized cwd>\chats\<session_id>.jsonl`, and the id is a UUID.
  - The finding notes when the file first exists, and whether `--continue` with no session errors or starts fresh.
- **Q9. Version.** `qwen --version` prints the bare `0.26.5`.
- **Q10. Process chain.** The live tree is `cmd → node cli-entry.js → node cli.js → node cli.js`.
  - The finding records each process's image name and command line, and what Marshell's foreground reader reports for a Qwen started by typing `qwen` in a shell tab.
- **Q11. Mode flag over setting.** `--approval-mode default` wins over `"tools": {"approvalMode": "yolo"}` in the user settings.
  - If it does, Qwen has no "skip every prompt" setting that a Marshell launch would miss (6j2 decision 11), and `skips_all` returns None.
- **Q12. Fake model sign-in.** Qwen starts with no auth dialog when the scratch settings say `security.auth.selectedType: "openai"` and the env has `OPENAI_API_KEY`, `OPENAI_BASE_URL` and `OPENAI_MODEL`.
  - The finding records the minimal set that works, and whether Qwen streams (SSE) or not.

## Review focus

1. **Qwen run outside a Marshell tab while the hooks are installed.** This covers Windows Terminal, Marshell closed, and a stale tab id. Every bridge path with no answer from the core prints nothing and exits 0. That includes the core unreachable, an unknown tab, a closed tab, and a tab that runs another agent. Qwen then behaves as with no hook and shows its own dialog. Tests in Tasks 2 and 4.
2. **A person's own `settings.json`.** BOM, CRLF, comments, their own hooks in the same events, their own `ui` keys, and their own `statusLine` (a command or a preset) all stay byte-equal apart from Marshell's entries, on install and on removal. Removal leaves no empty `"hooks": {}` that the install added. Tests in Task 3.
3. **A card released, timed out, or whose core goes away.** Each of these gives no decision, so Qwen shows its own dialog and the call never runs unasked. The core's hold deadline ends before the bridge's wait, and the bridge's wait ends before the entry's 600 s `timeout`. Tests in Task 4.
4. **Two Qwen tabs asking at once.** Each card answers only its own request. Test in Task 4.
5. **Qwen updated to an unrecorded version, or its version unreadable.** The row gets the note instead of a guessed status, and cards still work. Tests in Task 4.

---

### Task 1: Spike: a real Qwen against a fake model, the fixtures and the findings (main thread, live)

This task runs in the main thread, not through a worker.

**Files:**
- Create: `.claude/skills/marshell-live-check/fake-openai.mjs`
- Modify: `.claude/skills/marshell-live-check/live-check.ps1`, `.claude/skills/marshell-live-check/SKILL.md`
- Create: `docs/research/2026-11-03-qwen-code-hooks.md`
- Create: `crates/core/tests/fixtures/qwen/hooks/0.26.5/*.json`, `crates/core/tests/fixtures/qwen/statusline/0.26.5/*.json`
- Modify: this plan (only where a finding differs)

- [ ] **Step 1: The fake model.** Write `fake-openai.mjs` (Node `http`, no packages).
  - It serves `POST /v1/chat/completions` on `127.0.0.1:<port from argv>`, in streaming SSE and non-streaming JSON, and `GET /v1/models`.
  - It appends each request body to `<log path from argv>` (JSONL).
  - It picks a reply from the last user message:
    - `RUN <cmd>` gives a `run_shell_command` tool call with that command.
    - `WRITE <file>` gives `write_file` with one line.
    - `EDIT <file>` gives Qwen's edit tool.
    - `READ <file>` gives `read_file`.
    - After a tool result, or for any other text, it replies with the text `done`.
  - Exact tool and argument names are taken from the bundle's tool declarations (`lib\` grep for `run_shell_command`).
- [ ] **Step 2: `live-check.ps1 start -QwenFake`.**
  - Start `node fake-openai.mjs <port> <Scratch>\qwen-fake.jsonl` in the background and record its PID in the state file.
  - Set `OPENAI_API_KEY=marshell-fake`, `OPENAI_BASE_URL=http://127.0.0.1:<port>/v1` and `OPENAI_MODEL=marshell-fake` for the app, so every tab inherits them.
  - Create `<Scratch>\home\.qwen\settings.json` with only what Q12 needs.
  - `stop` ends the fake server by its recorded PID.
  - Add one line to SKILL.md's start step.
- [ ] **Step 3: A recorder.**
  - In the scratch folder, write a recorder script. It saves stdin, `MARSHELL_TAB_ID`, `MARSHELL_HOME` and `QWEN_CODE_SESSION_ID` to `<Scratch>\rec\<Event>-<n>.json`, and prints `<Scratch>\answer\<Event>.txt` (empty when missing), with the exit code from `answer\<Event>.exit` (0 when missing).
  - Point the 9 events of Task 3's table at it in the scratch `settings.json`, using a recorder path with a space in it (Q2). Add one entry whose path holds `&`, and one whose path holds `%`.
  - Point `ui.statusLine` at a second recorder that logs each stdin with a timestamp.
- [ ] **Step 4: Drive it.**
  - Start the check with a fresh literal scratch folder and `-QwenFake`, and open a Qwen tab in Manual in a scratch project.
  - Over CDP, send `RUN echo marshell`, `WRITE a.txt`, `EDIT a.txt` and `READ a.txt`, and answer Qwen's dialogs in the terminal.
  - Start a second Qwen tab and send a prompt in each (Q3).
  - Then work through the Q4 matrix with `answer\PermissionRequest.txt` set to allow, deny with a message, empty, exit 1, exit 2, and a 5 s timeout.
  - Repeat one `RUN` in each of Plan, Auto-edit, Full auto (`auto`) and Bypass (`yolo`).
  - Check Q6 by answering `UserPromptSubmit` with a context note and finding it in `qwen-fake.jsonl`.
  - Check Q11 with `tools.approvalMode: "yolo"` in the scratch settings and a Manual launch.
- [ ] **Step 5: The status line, resume, version and processes.**
  - Record Q7.
  - Quit Qwen with `/quit`, run `qwen --resume <id>` in the same tab, and record Q8.
  - Run `qwen --version` (Q9).
  - List the tab's process tree with `Get-CimInstance Win32_Process` (image and command line) for a launched Qwen and for one typed into a shell (Q10).
- [ ] **Step 6: Fixtures.**
  - Copy one recorded body per case into `crates/core/tests/fixtures/qwen/hooks/0.26.5/`, named `<Event>[-<case>].json`. That gives `SessionStart`, `UserPromptSubmit`, `PreToolUse-run_shell_command`, `PreToolUse-write_file`, `PreToolUse-<edit tool>`, `PreToolUse-read_file`, `PermissionRequest-run_shell_command`, `PermissionRequest-<edit tool>`, `PostToolUse`, `Notification-permission_prompt`, `Notification-idle_prompt`, `Stop`, and `SessionEnd` if seen.
  - Copy the status-line bodies into `statusline/0.26.5/` as `idle.json` and `working.json`.
  - Run `pnpm scrub:fixtures crates/core/tests/fixtures/qwen`, then `cargo test -p marshell-core -j 4 --test fixtures_scrubbed`. Expected: PASS.
- [ ] **Step 7: Write the research note.**
  - Write `docs/research/2026-11-03-qwen-code-hooks.md`. It answers Q1 to Q12, each with its evidence, and gives the exact `settings.json` that ran.
  - Record the check that the real `~/.qwen` still doesn't exist.
  - Stop the check by PID. First list the tree and look for agent CLIs the person started. Remove the scratch folder only after showing its path and getting an OK.
- [ ] **Step 8: Amend and commit.**
  - Where a finding differs from an assumption, amend the plan: the event table in Task 3, the shell form in Task 3, the fold in Task 4, the answers in Task 4, and the copy.
  - A finding that changes a §9 decision goes to the person through AskUserQuestion first.

```bash
git add .claude/skills/marshell-live-check docs/research/2026-11-03-qwen-code-hooks.md crates/core/tests/fixtures/qwen docs/superpowers/plans/2026-11-03-phase-6l1-qwen-adapter.md
git commit -m "docs(research): record Qwen Code's hooks, status line and payloads, with scrubbed fixtures"
```

### Task 2: The bridge speaks Qwen

**Files:**
- Modify: `crates/hook/src/context.rs` (the `accepted` match), `crates/hook/src/statusline.rs` (`shell_for`)
- Test: `crates/hook/tests/bridge.rs`, inline tests in both files

**Interfaces:**
- Produces: the bridge prints a context answer for `--agent qwen` only in the shape `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":<string>}}`. On Windows it runs a chained Qwen status line through `cmd.exe /d /s /c`, which is how Qwen runs it (Q7).

- [ ] **Step 1: Write the failing tests.**
  - `context::tests::qwen_takes_the_additional_context_shape`: Claude's accepted shape is accepted for `"qwen"`, and `{"injectSteps":[...]}` is refused for it.
  - In `tests/bridge.rs`:
    - `a_qwen_event_with_no_core_prints_nothing`: roles `event`, `permission` and `context`, with `MARSHELL_HOME` pointing at a folder with no `run/endpoint.json`. Expect empty stdout and exit 0.
    - `a_qwen_permission_allow_from_the_core_is_printed`
    - `a_qwen_permission_answer_with_updated_permissions_is_refused`: the bridge prints nothing.
  - `statusline::tests::qwen_runs_its_chained_line_through_cmd_on_windows` (`#[cfg(windows)]`).
- [ ] **Step 2: Run them.** `cargo test -p marshell-hook -j 4 qwen`. Expected: FAIL for the context, status-line and refusal cases.
- [ ] **Step 3: Implement.** In `context::accepted`, match `"claude" | "codex" | "qwen"`. In `statusline::shell_for`, give `qwen` the same Windows shell as `antigravity` only if that is the cmd form Q7 recorded. Otherwise add a `qwen` arm with Q7's form. `permission::accepted` already takes the Claude shape for every agent other than `antigravity`, so the allow and deny cases pass unchanged. Leave it as it is.
- [ ] **Step 4: Run.** `cargo test -p marshell-hook -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(hook): the bridge answers Qwen Code's prompt context and runs its chained status line`

### Task 3: The Qwen hook and status-line plan

**Files:**
- Create: `crates/core/src/agents/qwen/mod.rs`, `crates/core/src/agents/qwen/hooks.rs`
- Modify: `crates/core/src/agents/mod.rs` (`pub mod qwen;`), `crates/core/src/agents/common.rs` (only for helpers moved out of `claude/hooks.rs` or `antigravity/hooks.rs` that both use)

**Interfaces:**
- Produces, in `agents::qwen::hooks`:
  - `pub const STATUS_EVENTS: [&str; 8] = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Notification", "Stop", "StopFailure", "SessionEnd"]`, plus `PermissionRequest` as the approve entry.
  - `pub fn settings_path(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf`: `QWEN_HOME` when it's set and not blank, joined with `settings.json`. Otherwise `<user_home>/.qwen/settings.json`.
  - `pub fn hook_command(home: &Path, role: &str) -> Result<String, PlanError>`: `<cmd_form of the bridge path> <role> --agent qwen --tag mshl1`, with the arguments unquoted. Q2 found that Qwen escapes `"` to `\"` before `cmd /d /s /c`, so `"<path>" …` fails and Antigravity's form works (`C:"/Users/Jane Doe/.marshell/bin"/marshell-hook.exe …`, tried with a space and an `&` in the folder). Move `cmd_form` and its two refusal reasons from `antigravity/hooks.rs` to `common.rs`, taking the CLI's name, so Antigravity's copy reads as before and Qwen's says "Qwen Code".
  - `pub fn statusline_command(home: &Path) -> Result<String, PlanError>`: `… statusline --agent qwen --tag mshl1`, the same form.
  - `pub struct Found { pub install: HookInstall, pub approve_installed: bool, pub has_own_statusline: bool, pub chained: bool, pub preset_statusline: bool }`
  - `pub fn inspect(settings: &Value, chain: Option<&Value>, home: &Path) -> Found`
  - `pub fn install_plan(...)` and `pub fn remove_plan(...) -> Result<RemovePlan, PlanError>`, with the same parameters and return types as `claude::hooks::install_plan` and `remove_plan`, plus the chain path `paths::statusline_chain(home, "qwen")`.
- Produces, in `agents::qwen` (`mod.rs`):
  - `pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(0, 26, 5)]`
  - `pub fn version_gate(version: Option<&str>) -> Gate`, using `common::version_gate("Qwen Code", …)`.
  - `pub fn parse_version(stdout: &str) -> Option<String>`
  - `pub fn context_output(note: &str) -> Value`: `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":note}}`.
  - `pub fn skips_all(user_home: &Path) -> Option<String>`: None, per Q11. If Q11 shows the setting wins over the flag, it reads `tools.approvalMode == "yolo"` and returns `"tools.approvalMode"`.

The plan's rules:

| Event | Matcher | Role | Timeout |
| --- | --- | --- | --- |
| `SessionStart`, `UserPromptSubmit`, `Notification`, `Stop`, `StopFailure` | none (Q1) | `event` | 5 |
| `PreToolUse`, `PostToolUse` | `".*"` | `event` | 5 |
| `SessionEnd` | none | `event` | 2 |
| `PermissionRequest` | `".*"` | `permission` | 600 |

- Entries are appended as new groups at the end of each event's array, after the person's own groups. A reinstall replaces Marshell's entries in place.
- The status line:
  - Theirs is a command: it is saved whole in the chain file, and `ui.statusLine` becomes `{"type":"command","command":<statusline_command>}`.
  - It is already Marshell's: nothing is saved.
  - It is a preset (`type` other than `command`): it is left as it is, no chain is written, and `preset_statusline` is true.
  - There is no `ui` object: one is added holding only `statusLine`.
- Removal:
  - It takes out Marshell's entries, and drops a group, an event or `hooks` itself only when Marshell's entries were all that was in it (`claude::hooks::removal_pointers` with prune).
  - It restores the chained status line, or removes `ui.statusLine` if there was none. It drops a `ui` that the install created and that now holds nothing.
  - If the person changed `ui.statusLine` since the install, it is left as it is, and `RemovePlan.statusline_left` is true.

- [ ] **Step 1: Write the failing tests** (inline in `qwen/hooks.rs`, against string fixtures, like `claude/hooks.rs`'s):
  - `the_settings_path_follows_qwen_home`
  - `a_blank_qwen_home_is_ignored`
  - `install_into_a_missing_file_writes_every_event`: the exact table above, with `timeout` 600 on `PermissionRequest`.
  - `the_hook_command_uses_the_cmd_form`, and `a_bridge_path_with_a_quote_or_percent_is_refused_naming_qwen_code`.
  - `install_keeps_qwens_version_key`: a file holding `"$version": 4` keeps it, byte for byte, on install and removal.
  - `install_keeps_the_persons_own_hooks_and_bytes`: a file with a BOM, CRLF, a comment, their own `PreToolUse` group and `ui.theme`. After the install, every byte outside the new entries is unchanged (compare the text with Marshell's spans cut out).
  - `install_chains_their_command_status_line` and `install_leaves_a_preset_status_line`
  - `reinstall_replaces_entries_in_place`
  - `removal_restores_the_original_bytes`: install then remove gives back the original text exactly, for each of the fixtures above.
  - `removal_leaves_a_status_line_the_person_changed`
  - `inspect_reports_installed_partial_orphaned_and_none`: orphaned means the bridge path in the entries doesn't exist, as for Claude.
  - `a_file_that_does_not_parse_plans_nothing`
  - `a_hooks_value_of_the_wrong_shape_plans_nothing`: `"hooks": []`, and `"PreToolUse": {}`.
  - In `qwen/mod.rs`: `qwen_0_26_5_is_known`, `a_later_minor_is_not_recorded` ("Qwen Code 0.27.0 is not one Marshell has recorded yet"), `an_earlier_version_is_older`, `the_context_output_is_additional_context`, `parse_version_reads_the_bare_version`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 agents::qwen`. Expected: FAIL to compile, then FAIL.
- [ ] **Step 3: Implement** `qwen/mod.rs` and `qwen/hooks.rs`. `claude/hooks.rs` has the same JSON layout, so follow its parse, edit and pointer code. Move a helper into `common.rs` rather than copy it when both modules need it unchanged, and keep the Claude tests green.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4 agents::`. Expected: PASS, including the existing Claude and Antigravity tests.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): plan Qwen Code's hooks and chained status line in its user settings`

### Task 4: Live status, approvals and references from Qwen's hooks

**Files:**
- Create: `crates/core/src/agents/qwen/status.rs`, `crates/core/src/agents/qwen/request.rs`
- Modify:
  - `crates/core/src/agents/mod.rs`: `supports_permissions`, `permission_view`, `permission_output`, `release_output`.
  - `crates/core/src/server/hook.rs`: the prompt take-out, the WSL agent match, `starts_prompt`, the fold match, the context match, the version-gate table, and `fold_live_with` with its status-line gate.
  - `crates/core/src/server/mod.rs`: `qwen_home: Option<PathBuf>`, `with_qwen_home`, and Qwen in the version cache.
- Test: `crates/core/tests/hook.rs`, `crates/core/tests/permissions.rs`, inline tests

**Interfaces:**
- Consumes: Task 3's `version_gate`, `context_output` and fixtures.
- Produces:
  - `qwen::status::fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus>`
  - `qwen::status::fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus`
  - `qwen::status::mode_of(body: &Value) -> Option<SessionMode>`
  - `qwen::request::detail(request: &Value) -> PermissionDetail`
  - `qwen::request::view(p: &PendingPermission) -> Option<PermissionView>`
  - `qwen::request::permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str>`
  - `AppState::with_qwen_home(self, user_home: PathBuf) -> Self`

The fold follows Claude Code's, from Q3's bodies:

| Event | Result |
| --- | --- |
| `SessionStart` | Idle, with `model` |
| `UserPromptSubmit` | Working. One with an empty `prompt` is Qwen sending a tool result back, so it continues the turn: Working, and it is not a new prompt. |
| `PreToolUse` | Working, with the phrase from the tool ("Running <command>" for `run_shell_command`, "Editing <relative file>" for writes and edits, "Reading <relative file>" for reads) |
| `PostToolUse` | Working |
| `PermissionRequest` | Needs permission |
| `Notification` `permission_prompt` | Needs permission. This is a dialog with no card behind it: released, or no hook decision. |
| `Notification` `idle_prompt` | Idle, except right after a `PreToolUse`: Qwen sends one 10 to 20 ms after each `PreToolUse`, before the tool runs, so while the status is the Working that a `PreToolUse` set, it changes nothing. After a "No" in Qwen's own dialog, no `Stop` comes, and this is what ends the turn. |
| `Stop` | Idle (Done, as Claude's), with context % = `context_usage` × 100 when present. This is the only reliable context %. |
| `StopFailure` | Idle, with the error phrase Claude's fold uses |
| `SessionEnd` | Ended |

- `mode_of` maps `default` to Manual, `plan` to Plan, `auto_edit` and `auto-edit` to AutoEdit, `auto` to FullAuto, and `yolo` to Bypass.
- The status line sets model (`model.display_name`), and context % (`context_window.used_percentage`) only when `context_window.context_window_size` is above 0. Qwen sends 0 for both with a model it doesn't know, and that is unknown, not 0 %. The status line carries no mode. There's no plan meter: no `rate_limits`.
- Approvals:
  - Every Qwen `PermissionRequest` from an open tab whose effective agent is `qwen` is held for a card (decision 4). Qwen sends it only when it would ask.
  - Allow gives `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`.
  - Deny gives `behavior: "deny"` with `message: common::DENIED_TEXT`.
  - "Always allow" gives `Err` with decision 5's copy.
  - `release_output("qwen", _)` is `None`: no decision, so Qwen shows its dialog.
  - The card's detail comes from `tool_name` and `tool_input`. An unknown tool shows as other, with its raw arguments flattened by `common::flatten_input`. The request carries no tool id (Q3), so nothing may key on one.
  - The card's risk line uses the same risky-command rules the Claude card uses.
- The prompt: for `qwen`, `prompt` and `submitted_prompt` are both taken out of the body, as `prompt` is for Claude and Codex, before anything else sees it. `starts_prompt` is a `UserPromptSubmit` whose taken-out `prompt` isn't empty, so a tool-result continuation neither gets the note again nor clears the chip. The context answer is `qwen::context_output`.

- [ ] **Step 1: Write the failing tests.**
  - In `qwen/status.rs`, one test per table row, each fed the Task 1 fixture for that event: `session_start_is_idle_with_its_model`, `a_prompt_is_working`, `a_shell_tool_says_running_its_command`, `a_permission_request_needs_permission`, `a_permission_prompt_notification_needs_permission`, `stop_is_done_with_context_from_its_ratio`, `session_end_ends_it`, `the_status_line_sets_model_and_context`, and `mode_of_reads_both_auto_edit_spellings`. Also:
    - `an_empty_prompt_continues_the_turn` (fixture `UserPromptSubmit-continuation`).
    - `idle_prompt_right_after_a_tool_starts_keeps_working`: `PreToolUse-run_shell_command`, then `Notification-idle_prompt`, gives the PreToolUse's Working.
    - `idle_prompt_after_a_dialog_is_idle`: `Notification-permission_prompt`, then `Notification-idle_prompt`, gives Idle.
    - `a_status_line_with_no_window_size_leaves_context_unknown` (fixture `statusline/0.26.5/idle.json`).
    - `mode_of_reads_each_recorded_mode`: the `UserPromptSubmit-<mode>` fixtures.
  - In `qwen/request.rs`: `a_shell_request_shows_its_command`, `a_write_shows_the_file`, `an_unknown_tool_shows_raw_arguments`, `allow_and_deny_answer_in_qwens_shape`, `always_allow_is_refused_with_the_copy`.
  - In `tests/hook.rs`, through the real router with `with_qwen_home(<temp>)` and a fixed version `0.26.5`:
    - `qwen_events_fold_into_the_tabs_live_status`
    - `a_qwen_prompt_is_never_logged_or_kept`: the hook log and the event ring hold no `prompt` and no `submitted_prompt` key.
    - `a_qwen_continuation_gets_no_note`: an empty-prompt `UserPromptSubmit` answers 204 and leaves the chip.
    - `a_qwen_prompt_naming_an_alias_gets_the_note_as_additional_context`
    - `an_unrecorded_qwen_version_gets_the_note_and_no_status`
    - `qwen_status_off_says_so`
  - In `tests/permissions.rs`:
    - `a_qwen_permission_request_is_held_for_a_card`
    - `allow_from_the_card_answers_qwen`
    - `a_released_qwen_card_answers_nothing`: 204, so the bridge prints nothing.
    - `a_qwen_request_from_an_unknown_tab_or_a_tab_running_another_agent_passes`
    - `two_qwen_tabs_get_their_own_answers`
    - `a_held_qwen_request_times_out_before_the_bridge`: the core's deadline is less than the bridge's wait, and the bridge's wait is less than 600 s.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 qwen`. Expected: FAIL.
- [ ] **Step 3: Implement** `status.rs` and `request.rs`, then the `"qwen"` arms at each listed spot in `agents/mod.rs`, `server/hook.rs` and `server/mod.rs`. In `fold_live_with`, the status-line role is admitted for `claude | antigravity | qwen`. Qwen gets its own `Gate::Known if agent == "qwen"` arm, and has no Done hold, subagent count or stuck check.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS. Note any test that already failed on `origin/main` in the ledger.
- [ ] **Step 5: Commit.** `feat(agents): Qwen Code's hooks drive live status, approve cards and project references`

### Task 5: Setup, removal and launch: Qwen becomes an adapter agent

**Files:**
- Modify:
  - `crates/core/src/agents/manifests.json`: remove the `qwen` entry.
  - `crates/core/src/agents/manifest.rs`: `ADAPTER_IDS` and `ADAPTER_EXES` gain `qwen`. The shipped-manifests test now expects 8 ids, without `qwen`.
  - `crates/core/src/profiles/detect.rs`:
    - `agent_command`: `"qwen" => Some("qwen")`.
    - The program-to-agent map: `"qwen" => Some("qwen")`.
    - `mode_args`, `resume_args` and the prompt flag (`-i`): the same flags the manifest had, plus `ResumeTarget::Session(id) => ["--resume", id]` and `_ => ["--continue"]`.
    - Foreground recognition of a node-run Qwen: the npm package `@qwen-code/qwen-code` and the standalone install, per Q10. Typed into a shell, the chain is `cmd.exe /c ""%LOCALAPPDATA%\qwen-code\bin\qwen.cmd" <args>"`, then `"%LOCALAPPDATA%\qwen-code\qwen-code\bin\..\node\node.exe"  "%LOCALAPPDATA%\qwen-code\qwen-code\bin\..\lib\cli-entry.js" <args>`, then `"…\qwen-code\qwen-code\node\node.exe" --expose-gc "…\qwen-code\qwen-code\lib\cli.js" <args>` twice.
  - `crates/core/src/agents/install.rs`: the `install_kind` arm for the standalone `qwen-code\bin\qwen.cmd`, plus npm.
  - `crates/core/src/server/agents.rs`:
    - `Adapter::Qwen`, `adapter()`, `cli_name` "Qwen Code".
    - Setup, plan and remove arms, with the user-home resolver `qwen_user_home` honouring `with_qwen_home`.
    - The list of CLIs that can start.
    - `sign_in = SignIn::NoCommand` until 6l2.
    - The settings view stays not editable (6l2).
  - `crates/core/src/server/sessions.rs`: the agent display name, and the `resume_session` gate (`"claude" | "codex" | "antigravity" | "qwen"`).
  - `crates/core/src/history/recorder.rs`: Qwen is linked by the first event that names it, as Codex is, and asks for no copy.
  - `crates/core/src/history/mod.rs`: `sidecar_allowed_in` for `qwen`. The id is a UUID, and the path is absolute, local, free of `..`, and ends in `projects/<one part>/chats/<id>.jsonl`.
  - `crates/core/src/history/index.rs`: `filters()` also excludes `'qwen'` until 6l2's importer.
  - `crates/core/src/diagnostics/export.rs`: `CLIS` gains `qwen`.
  - `crates/core/src/server/catalog.rs`: the Qwen entry.
  - `crates/cli/src/uninstall.rs`: remove Qwen's hooks and restore its chain, in the same order and with the same error handling as Antigravity's. A settings file it can't read or plan is reported, and the rest goes on.
- Test: `crates/core/tests/agents_api.rs`, `crates/core/tests/catalog_api.rs`, `crates/core/tests/cli_task.rs`, `crates/cli` tests, and inline tests in `detect.rs` and `history`.

**Interfaces:**
- Consumes: Task 3's `install_plan`, `remove_plan` and `inspect`.
- Produces: `/v1/agents/qwen/setup`, `/plan` and `/remove` answer as Antigravity's do. `AgentSetup` for `qwen` reports `preset_statusline` through a new protocol field `statusline_preset: bool` on `AgentSetup`, which is false for the other agents.

- [ ] **Step 1: Write the failing tests.**
  - `agents_api.rs`:
    - `qwen_setup_reports_none_then_installed_after_apply`
    - `qwen_plan_shows_the_settings_diff_and_the_chain_file`
    - `qwen_remove_restores_the_status_line`
    - `qwen_orphaned_entries_are_reported`
    - `qwen_setup_reports_a_preset_status_line`
  - `detect.rs`:
    - `qwen_resumes_by_session_id`: `& qwen --approval-mode default --resume '<uuid>'`.
    - `qwen_continues_without_an_id`
    - The existing `qwen_takes_its_prompt_after_dash_i` stays green.
    - `a_standalone_qwen_node_is_recognised`: each of Q10's three node command lines.
  - `history`:
    - `a_qwen_transcript_path_is_allowed`
    - `a_qwen_path_outside_chats_or_with_dotdot_is_refused`
    - `qwen_is_linked_by_its_first_event`
  - `sessions`:
    - `a_qwen_tab_resumes_its_latest_session`
    - `a_qwen_tab_whose_transcript_is_gone_starts_fresh`
  - `uninstall`: `uninstall_removes_qwens_hooks_and_restores_its_status_line`
  - `manifest.rs`: `shipped_manifests_parse_and_have_unique_ids` now lists the 8 without `qwen`, and `an_own_agent_may_not_take_qwen`.
- [ ] **Step 2: Run them.** `cargo test --workspace -j 4 qwen`. Expected: FAIL.
- [ ] **Step 3: Implement** each listed arm.
- [ ] **Step 4: Run.** `cargo test --workspace -j 4`, then `pnpm check:generated`. Expected: PASS. The generated `AgentSetup.ts` gains `statusline_preset`.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): Qwen Code is set up, launched and resumed as an adapter agent`

### Task 6: Qwen in the Agents section, onboarding, launcher, rows and cards

**Files:**
- Modify:
  - Unions:
    - `src/lib/api/agents.ts` (`HookAgent`, `toDetections`' later list)
    - `src/features/onboarding/types.ts` (`CliId`)
  - Agents section and onboarding:
    - `src/features/agents/AgentsSection.tsx`: `agentTitle`, the props, the state, `getSetup`, and the `setups`, `skillFlows` and `installs` maps. Qwen's skill row says the skill comes in a later version: no skill flow until 6l2.
    - `src/features/agents/agentsState.ts`
    - `src/features/onboarding/LiveOnboarding.tsx`: state, flows, `consentQueue` (after Antigravity), `getSetup`.
    - `src/features/onboarding/consent.ts`: `CLI_NAME` "Qwen Code", `CAPABILITIES`, `entryCount`.
    - `src/features/onboarding/ConsentCard.tsx`: the preset line copy when `statusline_preset`.
    - `src/features/onboarding/Detected.tsx` and `src/features/onboarding/emptyStates.ts`
    - `src/features/doctor/DoctorPage.tsx`: the copy names Qwen Code.
    - `src/features/settings/ProjectsSection.tsx`: `CLIS_HELP`.
  - Approve card:
    - `src/features/approval/ApproveCard.tsx`: a Qwen card offers Allow and Deny, with no Always allow. Its release text is the "asking in its own terminal now" one, not Antigravity's "denied it".
  - Modes, launcher and sessions:
    - `src/features/modes/sessionModes.ts`: `isAdapter`, the mode map, `launchArgs` (native `--approval-mode`), `classifies`, `switchKind`.
    - `src/features/launcher/launchChoice.ts`: the entry comes from the adapter list, not the manifest list.
    - `src/features/sessions/launchPrograms.ts`: `ADAPTER_AGENTS`.
    - `src/features/sessions/resumeCommand.ts`: `qwen: "qwen --continue"`.
    - `src/features/sessions/sessionActions.ts`: `CLOSE_BAR_NAMES`.
    - `src/features/sessions/LiveApp.tsx`: the hook-state loops and the `runningCount` record.
    - `src/features/sessions/toRow.ts`: `isHookAgent` and `isManifestAgent`.
  - Not touched: `src/features/accounts/*`, `src/features/companies/*` and `settingsSections.ts`'s `CompanyAgent` (6l2).
- Test: the `.test.ts(x)` beside each file, plus `src/features/onboarding/fixtures.ts` and `src/features/agents/fixtures.ts`, which gain a Qwen setup.

**Interfaces:**
- Consumes: `AgentSetup.statusline_preset` (Task 5), and the `qwen` id on `/v1/agents/{agent}/...`.

- [ ] **Step 1: Write the failing tests.**
  - `AgentsSection.test.tsx`:
    - `lists Qwen Code with its hook state and a Set up button`
    - `Qwen's skill row says the skill isn't available for Qwen Code yet`
  - `ConsentCard.test.tsx` and `consentCopy.test.ts`:
    - `Qwen's card is titled "Set up Qwen Code hooks" and its removal uses the removal copy`
    - `a preset status line shows the preset copy`
  - `ApproveCard.test.tsx`: `a Qwen card has Allow and Deny and no Always allow`
  - `sessionModes.test.ts`: `Qwen starts each mode with its --approval-mode flag`
  - `toRow.test.ts`: `a Qwen row waits for hooks and is not status limited`
  - `resumeCommand.test.ts`: `qwen --continue`
  - `LiveOnboarding.test.tsx`: `Qwen's consent comes after Antigravity's`
- [ ] **Step 2: Run them.** `pnpm test src/features src/lib`. Expected: FAIL.
- [ ] **Step 3: Implement** each arm. Copy is exactly as in the global constraints. A new button or line works by keyboard and in forced colours, using the existing classes.
- [ ] **Step 4: Run.** `pnpm test`, then `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(ui): Qwen Code is set up, launched and answered like the other adapter agents`

### Task 7: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder and `-QwenFake`.
  - Over CDP:
    - Set up Qwen's hooks from the Agents section and read the diff.
    - Open a Qwen tab in Manual and send `RUN echo marshell`. The row goes Working, then Needs permission. The card shows the command, Allow runs it, and the row returns to Idle with model and context %.
    - Send another `RUN`, release the card, and see that Qwen's own dialog appears in the terminal.
    - Deny one, and see that the model got the message (fake log).
    - Try "Always allow" and read decision 5's copy.
    - With a project alias, send a prompt naming it, and find the note in the fake log.
  - Restart the tab and read the argv: `--resume <id>`.
  - Two Qwen tabs ask at once, and each card answers its own.
  - Turn the hooks off, and check that the scratch `settings.json` is byte-equal to before the setup (a person's own status line was put in it first).
- [ ] **Step 2:** Write `docs/research/2026-11-03-qwen-adapter-live-check.md`: each check with its evidence, and findings numbered.
- [ ] **Step 3:** Stop the check by PID, after listing the tree and looking for agent CLIs the person started. Ask before removing the scratch folder.
- [ ] **Step 4:** Commit: `docs(research): record slice 6l1's live check`. Then the final whole-branch review (marshell-reviewer, Opus), one fix pass, and `marshell-ship`.
