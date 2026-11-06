# Slice 6n1: the GitHub Copilot CLI adapter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** GitHub Copilot CLI (`copilot`) gets what Claude Code, Codex, Antigravity and Qwen Code have:
- hooks and a chained status line, installed and removed through the config service;
- live status in its row;
- permission requests answered from the approve card, Departures and banners;
- Resume and Restart that pick up the tab's exact session;
- project references that reach its model.

Every piece is first checked against a real Copilot CLI 1.0.106 driven by a fake model.

**Architecture:**
- Copilot leaves the manifest tier. Its entry in `manifests.json` goes, and it gets a `"copilot"` arm wherever `"qwen"` has one today, except the arms 6n2 and 6n3 own.
- A new `crates/core/src/agents/copilot/` module reads Copilot's own camelCase hook events (decision 9). Its files:
  - `hooks.rs`: the whole Marshell-owned `hooks/marshell.json`, the `statusLine` key of Copilot's `settings.json`, and the chain file.
  - `status.rs`: events and the status line folded into `LiveStatus`.
  - `request.rs`: the card's view and Copilot's answer JSON.
  - `exe.rs`: finds the real `copilot.exe`, never VS Code's shim.
  - `mod.rs`: home, version gate, context output, session ids, history link.
- The bridge runs as a direct `exec` with `args`, so no shell sits between Copilot and the bridge.
- The core's routes, holds, history and resume stay agent-neutral and gain a `"copilot"` arm.

**Tech Stack:** Rust (axum, serde_json, `getrandom`, the `config` service and its JSONC edits), ts-rs, React + TypeScript, and Vitest through `renderToStaticMarkup`. The fake model server is the Node script from 6l1.

**Spec:** `docs/PLAN.md` §9, "Slice 6n1 decisions (2026-11-06)" (decisions 1 to 11), plus "Session modes" and "Config safety copy". Evidence: `docs/research/2026-11-06-copilot-cli.md`, Part 1. Task 1 adds what the spike saw. The pattern to follow is `docs/superpowers/plans/2026-11-03-phase-6l1-qwen-adapter.md`.

**Checked before planning (2026-11-06):**
- The winget install of Copilot CLI 1.0.106 is at `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe`. The user PATH has VS Code's `copilot-shim` folder first, and that shim forwards to the real CLI. No Copilot account is signed in on this PC. The real `~/.copilot` exists, because Copilot has run once, and neither tests nor live checks may touch it.
- The readable bundle is `%LOCALAPPDATA%\copilot\pkg\win32-x64\1.0.106\cli-main.js`. Hook execution is native code, so hook facts come from strings and the docs until the spike.
- `common::is_ours` already reads `--tag mshl1` from an entry's `args`.
- The config service already creates, replaces and deletes whole files (`EditOp::CreateFile`, `ReplaceText`, `DeleteFile`, as `agents/skill.rs` uses them).
- The bridge's `--event <name>` already adds `hook_event_name` to a body that has none.
- Today `identify::find` refuses the shim. Because `path_of` returns only the first PATH hit, a manifest Copilot launch on this PC fails with "isn't installed".

## Global constraints

- Marshell writes only these three files. Nothing else under `~/.copilot` and nothing in a repo (decision 3):
  - `<COPILOT_HOME or ~/.copilot>\hooks\marshell.json`, as a whole file;
  - the `statusLine` key of `<COPILOT_HOME or ~/.copilot>\settings.json`;
  - `~/.marshell/agents/copilot/statusline-chain.json`.

  That rules out the inline `hooks` key, `.github/hooks/`, `.github/copilot/settings*.json`, `C:\ProgramData\GitHub\Copilot\policy.d`, `config.json`, `permissions-config.json`, `mcp-config.json` and `providers.json`.
- Every write goes through the config service: preview, unified diff, backup, atomic write, `managed.json` and undo.
  - `marshell.json` is made with `CreateFile`, replaced with `ReplaceText` against its hash, and removed with `DeleteFile`, as `skill.rs` does.
  - `settings.json` is edited only by CST edits through the `config::jsonc` helpers. A file that doesn't parse fails closed.
  - The person's own keys, comments, BOM and CRLF stay byte-equal outside `statusLine`.
- Each hook entry's `args` hold `--tag mshl1`, read by `common::is_ours`.
- **The bridge never exits non-zero, and with no answer it prints nothing.** Copilot denies a tool call when a `preToolUse` hook exits non-zero (research Q3). The bridge already does this; Task 2 pins it for Copilot.
- Tests and live checks never read or write the real `~/.copilot`, `~/.claude`, `~/.codex` or `~/.marshell`. Tests inject paths with `AppState::with_copilot_home(<temp>)`.
- No new dependency, in Rust or in `package.json`. Session ids come from `getrandom`, which `marshell-core` already uses.
- New protocol fields go in `crates/protocol`, and `pnpm check:generated` passes afterwards.
- Never hold a `MutexGuard` across `.await` or file I/O. `copilot --version`, PATH searches and file reads run in `spawn_blocking`.
- Fixtures come from the real Copilot in Task 1 and are scrubbed with `pnpm scrub:fixtures` before they're committed.
- Copilot's conventions stay in `agents/copilot/`. `agents/claude/` and `agents/qwen/` gain no Copilot branch. A helper two modules need unchanged moves to `agents/common.rs`.
- The core never keeps a body's `prompt` or `initialPrompt`. Both are taken out before the log, fold, history or hold sees the body.
- Never launch through VS Code's `copilot-shim` (decision 11).
- Never name a test binary or copy after a real CLI: use `copilot-marshell-test`. Never taskkill a `copilot.exe` the check didn't start.
- Copilot is never bundled (its licence).
- Every UI change works with the keyboard alone and in forced colours. Copilot gets its own consent card, with no bulk allow.
- Copy, exactly:
  - Change titles: "Set up Copilot CLI hooks", "Turn Copilot CLI hooks off".
  - Status notes, from `common::version_gate` with the name "Copilot CLI":
    - "Live status is off for Copilot CLI"
    - "Copilot CLI's version could not be read"
    - "Copilot CLI <v> is older than the versions Marshell knows"
    - "Copilot CLI <v> is not one Marshell has recorded yet"
  - "Always allow" refused (decision 5): "Copilot CLI can't keep an always-allow rule from Marshell. Allow this once instead."
  - Removal: "Marshell removes the hook entries it added and puts your own status line back from statusline-chain.json; the rest of the file stays as it is" (the shared wording).
  - A `marshell.json` Marshell didn't write: "<path> holds hooks Marshell didn't write, so Marshell leaves it as it is."
  - Only the shim was found: "Only VS Code's copilot shim was found. Install Copilot CLI with winget install GitHub.Copilot, then try again."
  - Not found at all: "Copilot CLI isn't installed, or isn't on PATH."

## Assumptions Task 1 confirms or corrects

These come from the research note. Task 1 checks each one against a real Copilot 1.0.106 and records the evidence. If a finding differs, Task 1 amends this plan (shapes, constants, copy) and commits the change before Task 2 starts.

- **Q1. The file.**
  - Copilot loads `<COPILOT_HOME>\hooks\marshell.json` at session start, beside any other `*.json` there.
  - `"version": 1` is accepted, a BOM is accepted, and a user hook needs no folder trust.
  - The finding records whether the hooks run before the trust prompt is answered, and in a folder left untrusted.
- **Q2. The entry form.**
  - `{"type":"command","exec":"<bridge path, forward slashes>","args":[...],"timeoutSec":<n>}` runs the bridge directly with no shell. It works with a space and an `&` in the folder.
  - An entry with no `matcher` runs for every tool.
  - If `exec` doesn't exist in 1.0.106 (the docs may be newer), the fallback is `powershell` plus `command` with `& '<path>' <args>`, where each `'` in the path is doubled. The finding then records the start time PowerShell adds per hook.
- **Q3. Bodies.**
  - Every body carries `sessionId`, epoch-ms `timestamp` and `cwd`. `hook_event_name` comes from the bridge's `--event`, except `notification`, which carries `"Notification"` itself.
  - `preToolUse` adds `toolName`, `toolArgs` and `toolCallId`. The finding records whether `toolArgs` is a JSON string or an object, and Copilot's tool names on Windows for a shell call, a new file, an edit and a read.
  - `permissionRequest` adds `toolName` and `toolInput`, and maybe `permissionSuggestions` and a tool id. The finding records the exact keys.
  - `agentStop` adds `transcriptPath` and `stopReason`. `sessionEnd` adds `reason`. `sessionStart` adds `source`, plus `initialPrompt` with `-i`.
  - `MARSHELL_TAB_ID` and `MARSHELL_HOME` reach the hook process, and two Copilot tabs report their own tab ids.
  - The finding records which `COPILOT_*` variables are set in every Copilot hook process. `COPILOT_HOOK_DIR` is expected.
- **Q4. When `permissionRequest` fires.** The docs say it fires before Copilot's permission service for every request except reads. This item is a decision gate (Step 8). The finding records, for one shell call in each case, whether the hook fires and whether Copilot would have asked:
  - Manual with no rule;
  - after "allow all similar" in Copilot's own dialog (a released card);
  - a launch with `--allow-tool 'shell(echo:*)'`;
  - `--allow-all`;
  - Plan;
  - `--autopilot`;
  - a read, where none is expected.
- **Q5. Answers to `permissionRequest`.**
  - `{"behavior":"allow"}` runs the call with no dialog.
  - `{"behavior":"deny","message":"<text>"}` refuses it, and the model sees the message.
  - Empty stdout with exit 0 goes on to Copilot's own flow (its dialog).
  - The finding records what exit 1 and a timeout do. For the timeout, the entry gets `timeoutSec` 5 and a recorder that sleeps 10 s.
  - The finding also records when `toolInput.requestSandboxBypass` appears.
- **Q6. Neutral answers.**
  - For every other event, empty stdout with exit 0 leaves Copilot as it would be with no hook.
  - A `preToolUse` hook that exits 1 denies the call. This is checked once, because it is why the bridge must never fail loudly.
- **Q7. Context.** `userPromptSubmitted` answered with `{"additionalContext":"<note>"}` reaches the model: the note shows in the fake server's log.
  - If it doesn't, try `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmitted","additionalContext":"<note>"}}`.
  - If neither works, decision 7's fallback applies.
- **Q8. Status line.**
  - `"statusLine": {"type":"command","command":"<cmd_form bridge> statusline --agent copilot --tag mshl1"}` runs through `cmd.exe /d /s /c` with a 10 s timeout.
  - The finding records the stdin keys with their nesting (model, context %, session id, and any mode or `allow_all_enabled`) and when the line runs. It also records whether a `$` in the command is expanded.
- **Q9. Sessions.**
  - `--session-id <new uuid>` starts a new session with that id. The events carry the id, and `session-state\<uuid>\` appears.
  - `--resume=<uuid>` reopens it in a new process from the same folder.
  - `transcriptPath` is `<COPILOT_HOME>\session-state\<id>\events.jsonl`.
  - The finding records what `/clear` sends (expected: `sessionEnd` with `user_exit`, then a `sessionStart` with a new id), and what `--continue` does with no earlier session.
- **Q10. Version.** `copilot --version` prints `GitHub Copilot CLI 1.0.106.`, with the full stop, and maybe a second line.
- **Q11. Processes.**
  - A Copilot launched by its full path runs as one `copilot.exe`. One typed as `copilot` in a shell runs the shim's `copilot.exe`, then the real one.
  - The finding records each process's image and command line, and what Marshell's foreground reader reports for both.
  - The taskbar relaunch is not exercised (Task 1 sets `taskbarPresence: false`) and stays an open item.
- **Q12. The fake model.** Copilot starts with no sign-in prompt given this environment:
  - `COPILOT_PROVIDER_BASE_URL=http://127.0.0.1:<port>/v1`
  - `COPILOT_PROVIDER_TYPE=openai`
  - `COPILOT_PROVIDER_WIRE_API=completions`
  - `COPILOT_MODEL=marshell-fake`
  - `COPILOT_OFFLINE=true`
  - `COPILOT_HOME` and `COPILOT_CACHE_HOME` in the scratch folder

  The finding records the minimal set that works, the endpoints Copilot calls, whether it streams, and every first-run prompt.
- **Q13. Skip-all settings.**
  - With `"defaultPermissionMode": "allow-all"` in `settings.json`, a Manual launch (no mode flag) shows no dialog.
  - `COPILOT_ALLOW_ALL=true` in the tab's environment does the same.
  - If so, `skips_all` reports them (6j2 decision 11).
- **Q14. Claude Code's hooks.**
  - Copilot doesn't run a Marshell Claude hook from the scratch `~/.claude\settings.json`.
  - The finding records whether it runs one from a project's `.claude\settings.json`.

## Review focus

1. **Copilot run outside a Marshell tab while the hooks are installed.** This covers Windows Terminal, Marshell closed, and a stale tab id.
   - Every bridge path with no answer from the core prints nothing and exits 0: the core unreachable, an unknown tab, a closed tab, and a tab running another agent.
   - A loud failure here would deny every tool call.
   - Tests in Tasks 2 and 4.
2. **The person's own Copilot files.**
   - Their `settings.json` stays byte-equal apart from `statusLine.command`, on install and on removal: comments, BOM, CRLF, `padding` and `refreshInterval`.
   - Their own files in `hooks\` are never touched.
   - A `marshell.json` they wrote is neither replaced nor deleted.
   - Tests in Task 3.
3. **A card released, timed out, or whose core goes away.** Each gives no decision, so Copilot goes on to its own flow and the call never runs unasked. The core's hold deadline ends before the bridge's wait, and the bridge's wait ends before the entry's 600 s `timeoutSec`. Tests in Task 4.
4. **A request that Copilot would have handled itself never waits on a card.** That covers what Step 8 rules for Q4 (a Bypass tab, a rule), and a `requestSandboxBypass` request, which a hook's allow can't approve. Tests in Task 4.
5. **VS Code's shim first on PATH, or alone.** The launch runs the real `copilot.exe` by its full path, or refuses with the shim copy. It never runs the shim. Tests in Task 5.

---

### Task 1: Spike: a real Copilot against a fake model, the fixtures and the findings (main thread, live)

This task runs in the main thread, not through a worker.

**Files:**
- Modify: `.claude/skills/marshell-live-check/fake-openai.mjs`, `.claude/skills/marshell-live-check/live-check.ps1`, `.claude/skills/marshell-live-check/SKILL.md`
- Modify: `docs/research/2026-11-06-copilot-cli.md` (a new section, "What 6n1's spike saw", at the top of Part 1)
- Create: `crates/core/tests/fixtures/copilot/hooks/1.0.106/*.json`, `crates/core/tests/fixtures/copilot/statusline/1.0.106/*.json`
- Modify: this plan (only where a finding differs)

- [ ] **Step 1: The fake model.**
  - `fake-openai.mjs` picks each role's tool from the request's `tools` by a list of names per role:
    - shell: `run_shell_command`, `powershell`, `bash`, `shell`;
    - write: `write_file`, `create`;
    - edit: `edit`, `str_replace_editor`, `str_replace`;
    - read: `read_file`, `view`.
  - The first request's tool names go to the log once.
  - The header comment names Copilot.
  - Qwen's replies stay as they are.
- [ ] **Step 2: `live-check.ps1 start -CopilotFake`.**
  - Start the fake server as `-QwenFake` does, and share its start and stop code.
  - Set Q12's environment for the app, so every tab inherits it, with `COPILOT_HOME=<Scratch>\home\.copilot` and `COPILOT_CACHE_HOME=<Scratch>\copilot-cache`.
  - Write `<Scratch>\home\.copilot\settings.json` as `{"autoUpdate": false, "taskbarPresence": false}`.
  - `stop` ends the fake server by its recorded PID.
  - Add one line to SKILL.md's start step.
  - Before the first run, list the real `~/.copilot` with each file's modified time. After the last run, list it again: it must be unchanged.
- [ ] **Step 3: A recorder.**
  - In the scratch folder, write `rec.mjs`. It saves to `<Scratch>\rec\<event>-<n>.json`:
    - stdin;
    - `MARSHELL_TAB_ID`, `MARSHELL_HOME`, `COPILOT_HOOK_DIR`, `COPILOT_PROJECT_DIR` and `CLAUDE_PROJECT_DIR`;
    - the names only of every other variable.
  - It prints `answer\<event>.txt` (empty when missing), sleeps `answer\<event>.sleep` seconds, and exits with `answer\<event>.exit` (0 when missing).
  - Write `<Scratch>\home\.copilot\hooks\rec.json`. It points the 10 events of Task 3's table at `node.exe` with `args` `["<Scratch>\rec dir\rec.mjs", "<event>"]`, a folder with a space in it (Q2). Add one `powershell` entry to time Q2's fallback.
  - If no recording appears, 1.0.106 has no `exec`. Switch `rec.json` to the `powershell` form and record Q2's fallback.
  - Point `statusLine` at a second recorder that logs each stdin with a timestamp.
- [ ] **Step 4: Drive it.**
  - Start the check with a fresh literal scratch folder and `-CopilotFake`.
  - Open a shell tab in a scratch project and type the full path `& "$env:LOCALAPPDATA\GitHubCopilotCLI\copilot.exe" --session-id <uuid>`. Until Task 5, a manifest launch hits the shim refusal. Answer the trust prompt in the terminal.
  - Over CDP, send `RUN echo marshell`, `WRITE a.txt`, `EDIT a.txt` and `READ a.txt`, and answer Copilot's dialogs in the terminal.
  - Start a second Copilot tab and send a prompt in each (Q3).
  - Work through the Q4 cases, then Q5 with `answer\permissionRequest.txt` set to: allow, deny with a message, empty, exit 1, and the timeout.
  - Check Q6 with `answer\preToolUse.exit` set to 1 once.
  - Check Q7 by answering `userPromptSubmitted` with a note, then finding it in the fake log.
  - Check Q13 with each setting and a Manual launch.
  - Check Q14 with Marshell's Claude hook in the scratch `~/.claude\settings.json` and in the project's `.claude\settings.json`.
- [ ] **Step 5: The status line, sessions, version and processes.**
  - Record Q8.
  - Exit with `/exit`, run `copilot --resume=<uuid>` in the same tab, and record Q9. Then `/clear` once.
  - Run `copilot --version` (Q10).
  - List both tabs' process trees with `Get-CimInstance Win32_Process` (image and command line), and Marshell's foreground answer for each (Q11).
- [ ] **Step 6: Fixtures.**
  - Copy one recorded body per case into `crates/core/tests/fixtures/copilot/hooks/1.0.106/`, named `<event>[-<case>].json`:
    - `sessionStart`, `sessionStart-clear`, `userPromptSubmitted`;
    - `preToolUse-<shell tool>`, `preToolUse-<write tool>`, `preToolUse-<edit tool>`, `preToolUse-<read tool>`;
    - `postToolUse`, `postToolUseFailure`;
    - `permissionRequest-<shell tool>`, `permissionRequest-<edit tool>`, and `permissionRequest-sandbox` if seen;
    - `notification-permission_prompt`, `agentStop`, `errorOccurred` if seen;
    - `sessionEnd`, `sessionEnd-clear`.
  - Copy the status-line bodies into `statusline/1.0.106/` as `idle.json` and `working.json`.
  - Run `pnpm scrub:fixtures crates/core/tests/fixtures/copilot`, then `cargo test -p marshell-core -j 4 --test fixtures_scrubbed`. Expected: PASS.
- [ ] **Step 7: Write the findings.**
  - In the research note's new section, answer Q1 to Q14, each with its evidence, and give the exact `hooks\rec.json` and `settings.json` that ran.
  - Strike each UNVERIFIED item the spike answered, with a pointer to the answer.
  - Record that the real `~/.copilot` is unchanged.
  - Stop the check by PID. First list the tree and look for agent CLIs the person started. Remove the scratch folder only after showing its path and getting an OK.
- [ ] **Step 8: Amend and commit.**
  - Where a finding differs from an assumption, amend the plan: Task 3's table and entry form, Task 4's fold and answers, Task 5's arguments, and the copy.
  - **Decision gate (Q4).** If `permissionRequest` fires for a request Copilot would have allowed by itself (a rule, a session approval, `--allow-all`), ask the person through AskUserQuestion before Task 4. Decision 4 would otherwise raise cards Copilot never shows.
    - The recommended option: a Copilot request from a tab whose mode is Bypass passes with no decision. Copilot's own rules are not read, and that limit is recorded.
    - Record the answer as a new 6n1 decision in PLAN §9, and add the rule to Task 4's approvals.
  - Any other finding that changes a §9 decision (Q7's fallback, say) goes to the person through AskUserQuestion first.

```bash
git add .claude/skills/marshell-live-check docs/research/2026-11-06-copilot-cli.md crates/core/tests/fixtures/copilot docs/superpowers/plans/2026-11-06-phase-6n1-copilot-adapter.md
git commit -m "docs(research): record what Copilot CLI's hooks, status line and sessions do, with scrubbed fixtures"
```

### Task 2: The bridge speaks Copilot

**Files:**
- Modify:
  - `crates/hook/src/context.rs` (`accepted`)
  - `crates/hook/src/permission.rs` (`accepted`)
  - `crates/hook/src/statusline.rs` (`shell_for`)
  - `crates/hook/src/lib.rs` (the filter beside Grok's)
- Test: `crates/hook/tests/bridge.rs`, and inline tests in each file

**Interfaces:**
- Produces:
  - `context::accepted("copilot", answer)` is true only for an object whose one key is `additionalContext`, with a string value (Q7's shape).
  - `permission::accepted("copilot", answer, request, elevated)` is true only for `{"behavior":"allow"}` exactly, or for `{"behavior":"deny"}` with at most a string `message`. It never passes `interrupt` or any other key, and never a `hookSpecificOutput` wrapper.
  - `statusline::shell_for("copilot")` on Windows runs a chained line through `cmd.exe /d /s /c`, as for `qwen` (Q8).
  - A call with `--agent claude` made by Copilot prints nothing, as Grok's does. The test is Q3's variable (expected `COPILOT_HOOK_DIR`) being set and not empty.

- [ ] **Step 1: Write the failing tests.**
  - `context::tests::copilot_takes_only_top_level_additional_context`: `{"additionalContext":"x"}` passes. Refused for `"copilot"`: the `hookSpecificOutput` form, `{"additionalContext":1}`, `{"additionalContext":"x","decision":"block"}` and `{"injectSteps":[]}`.
  - `permission::tests::copilot_allow_and_deny_are_its_top_level_behavior`
  - `permission::tests::copilot_interrupt_or_extra_keys_are_refused`: the answers `{"behavior":"deny","interrupt":true}`, `{"behavior":"allow","message":"x"}` and the Claude-shaped answer.
  - In `tests/bridge.rs`:
    - `a_copilot_call_with_no_core_prints_nothing_and_exits_0`: roles `event` (with `--event preToolUse`) and `permission`, and `MARSHELL_HOME` with no `run/endpoint.json`. Assert empty stdout and exit status 0.
    - `a_copilot_permission_allow_from_the_core_is_printed`
    - `a_claude_call_made_by_copilot_prints_nothing`
    - `a_copilot_body_gets_its_event_name_from_the_flag`: the core receives `hook_event_name: "agentStop"`. A body that already holds `"Notification"` keeps it.
  - `statusline::tests::copilot_runs_its_chained_line_through_cmd_on_windows` (`#[cfg(windows)]`).
- [ ] **Step 2: Run them.** `cargo test -p marshell-hook -j 4 copilot`. Expected: FAIL for the context, permission, filter and status-line cases.
- [ ] **Step 3: Implement.** Add a `"copilot"` arm to each function. In `permission::accepted`, check Copilot's answer before the `hookSpecificOutput` path, as Antigravity's is. Leave `with_event` as it is.
- [ ] **Step 4: Run.** `cargo test -p marshell-hook -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(hook): the bridge answers Copilot CLI's permission requests and prompt context and runs its chained status line`

### Task 3: The Copilot hooks file and status-line plan

**Files:**
- Create: `crates/core/src/agents/copilot/mod.rs`, `crates/core/src/agents/copilot/hooks.rs`
- Modify:
  - `crates/core/src/agents/mod.rs` (`pub mod copilot;`)
  - `crates/core/src/agents/common.rs`:
    - `PlanError::NotMarshells { path: PathBuf }`, with Display giving the copy above;
    - any chain helper moved out of `qwen/hooks.rs` that both modules use unchanged.

**Interfaces:**
- Produces, in `agents::copilot` (`mod.rs`):
  - `pub fn home(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf`: `COPILOT_HOME` when it is set and not blank, otherwise `<user_home>/.copilot`.
  - `pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(1, 0, 106)]`
  - `pub fn version_gate(version: Option<&str>) -> Gate`, through `common::version_gate("Copilot CLI", …)`.
  - `pub fn parse_version(stdout: &str) -> Option<String>`: the `x.y.z` after `GitHub Copilot CLI ` on the first line, without the full stop.
  - `pub fn context_output(note: &str) -> Value`: `{"additionalContext": note}` (Q7).
  - `pub fn skips_all(settings_text: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Option<String>`:
    - `Some("defaultPermissionMode")` when the settings say `"allow-all"`;
    - `Some("COPILOT_ALLOW_ALL")` when that variable is truthy (`true` or `1`, any case);
    - otherwise None, per Q13.
- Produces, in `agents::copilot::hooks`:
  - `pub const EVENTS: [(&str, &str, u32); 10]`: event, bridge role and `timeoutSec`, from the table below.
  - `pub fn hooks_path(copilot_home: &Path) -> PathBuf` (`hooks/marshell.json`) and `pub fn settings_path(copilot_home: &Path) -> PathBuf` (`settings.json`).
  - `pub fn hooks_file(bridge: &Bridge, want: &HookPlanRequest) -> String`: the whole file. It is `serde_json::to_string_pretty` of the layout below, plus `\n`.
  - `pub fn statusline_command(bridge: &Bridge) -> Result<String, PlanError>`: `<common::bridge_prefix_of("Copilot CLI", bridge)> statusline --agent copilot --tag mshl1`.
  - `pub struct Files<'a> { pub hooks_path: &'a Path, pub hooks_text: Option<&'a str>, pub settings_path: &'a Path, pub settings_text: Option<&'a str>, pub chain_path: &'a Path, pub chain_text: Option<&'a str> }`
  - `pub struct Found { pub install: HookInstall, pub approve_installed: bool, pub has_own_statusline: bool, pub chained: bool }`
  - `pub fn inspect(files: &Files, bridge: &Bridge) -> Found`
  - `pub fn install_plan(files: &Files, bridge: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError>`
  - `pub fn remove_plan(files: &Files) -> Result<RemovePlan, PlanError>`, using `claude::hooks::RemovePlan`, re-exported as Qwen does.

The entries:

| Event | Role | `timeoutSec` |
| --- | --- | --- |
| `sessionStart`, `userPromptSubmitted`, `preToolUse`, `postToolUse`, `postToolUseFailure`, `notification`, `agentStop`, `errorOccurred` | `event` | 5 |
| `sessionEnd` | `event` | 2 |
| `permissionRequest` | `permission` | 600 |

The layout, shown for one entry:

```json
{
  "version": 1,
  "hooks": {
    "sessionStart": [
      {
        "type": "command",
        "exec": "C:/Users/Jane Doe/.marshell/bin/marshell-hook.exe",
        "args": ["event", "--agent", "copilot", "--event", "sessionStart", "--tag", "mshl1"],
        "timeoutSec": 5
      }
    ]
  }
}
```

The entries are written in the table's order. No entry has a `matcher`, `command`, `bash` or `powershell` (Q2). `want.status` false leaves out every `event` entry. `want.approve` false leaves out `permissionRequest`.

The hooks file:
- **Install:**
  - The file is missing: `CreateFile`.
  - It is Marshell's (it parses, and every entry under `hooks` is `is_ours`) but its text differs from `hooks_file`: `ReplaceText` against its hash.
  - It is Marshell's and the same text: no edit.
  - `want` asks for no entries: the file is planned as for removal.
- **Refused:** a file that holds an entry that isn't Marshell's, or that has no `hooks` object, gives `PlanError::NotMarshells`. A file that doesn't parse gives `PlanError::DoesNotParse`. Either way the plan fails as a whole, and nothing is written.
- **Removal:** a Marshell file gets `DeleteFile` against its hash. The `hooks` folder stays.

The status line in `settings.json` (JSONC):
- **Theirs is a command** (`statusLine.command` is a string that isn't Marshell's): the chain file holds `{"statusLine": <their whole object>}`. The install replaces only `statusLine.command`, so their `type`, `padding` and `refreshInterval` stay.
- **Already Marshell's:** nothing is saved.
- **Missing or `null`:** the install sets `"statusLine": {"type": "command", "command": <statusline_command>}`, and the chain holds `{"statusLine": null}`. A missing or blank `settings.json` is created holding only that key.
- **Any other shape** (no string `command`, or not an object): left as it is. No chain is written, and the status line counts as not Marshell's.
- **`want.statusline` false:** no settings edit and no chain.
- **Removal:**
  - While `statusLine.command` is still Marshell's, removal puts the chain's `command` back, or removes the `statusLine` key when the chain holds `null`. It then deletes the chain (`common::chain_delete`).
  - A `statusLine.command` the person changed since the install is left alone, and `RemovePlan.statusline_left` is true.
  - Removal never deletes `settings.json`.
- **Never written:** Marshell never writes `footer`, `hooks`, `disableAllHooks` or any other key.

`inspect`, as Qwen's:
- **Installed:** the file holds the 9 `event` entries with the current bridge path, and the status line is Marshell's or is left by the "any other shape" rule. `approve_installed` separately says whether `permissionRequest` is there.
- **Orphaned:** entries are present but their `exec` file doesn't exist, as for Claude.
- **Partial:** anything else that has entries.
- **None:** no file.

- [ ] **Step 1: Write the failing tests** (inline in `copilot/hooks.rs`, against string fixtures, like `qwen/hooks.rs`'s):
  - `the_home_follows_copilot_home`, `a_blank_copilot_home_is_ignored`
  - `install_into_an_empty_home_creates_the_file_and_the_status_line`: three edits:
    - `marshell.json`, whose exact text is the table above for the bridge `C:/Users/Jane Doe/.marshell/bin/marshell-hook.exe`;
    - `settings.json` holding only `statusLine`;
    - the chain `{"statusLine": null}`.
  - `each_entry_runs_the_bridge_by_exec_with_its_event_and_tag`: no entry has `command`, `bash`, `powershell` or `matcher`.
  - `want_picks_the_entries`: status only, approve only, and status line off.
  - `reinstall_with_the_same_bridge_plans_nothing`
  - `a_moved_bridge_replaces_the_file_against_its_hash`
  - `a_marshell_json_the_person_wrote_is_refused`: one untagged entry gives `NotMarshells` with the copy.
  - `a_hooks_file_that_does_not_parse_plans_nothing`
  - `install_keeps_the_persons_settings_bytes`: a file with a BOM, CRLF, a comment, `"model"`, and their `statusLine` with `padding` 2 and `refreshInterval` 5. After the install, every byte outside `statusLine.command` is unchanged.
  - `install_chains_their_command_status_line`, `install_over_no_status_line_chains_null`, `a_status_line_without_a_command_is_left`
  - `removal_restores_the_original_bytes`: install then remove gives back the original `settings.json` text exactly, for their own status line and for no status line. The hooks file gets `DeleteFile`.
  - `removal_leaves_a_status_line_the_person_changed`
  - `inspect_reports_installed_partial_orphaned_and_none`
  - `settings_that_do_not_parse_plan_nothing`
  - In `copilot/mod.rs`:
    - `copilot_1_0_106_is_known`
    - `a_later_minor_is_not_recorded`: "Copilot CLI 1.1.0 is not one Marshell has recorded yet"
    - `an_earlier_patch_is_older`: "Copilot CLI 1.0.105 is older than the versions Marshell knows"
    - `parse_version_drops_the_full_stop`: `"GitHub Copilot CLI 1.0.106.\nRun 'copilot update' to check for updates.\n"` gives `1.0.106`.
    - `the_context_output_is_top_level_additional_context`
    - `skips_all_reads_the_setting_and_the_env`: also checks a JSONC file with a comment, `"manual"`, and a blank file.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 agents::copilot`. Expected: FAIL to compile, then FAIL.
- [ ] **Step 3: Implement** `copilot/mod.rs` and `copilot/hooks.rs`. For the `statusLine` CST edits and the chain, follow `qwen/hooks.rs`'s, using the top-level key `statusLine` in place of `ui.statusLine`. Add the `NotMarshells` arm wherever `PlanError` is matched.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4 agents::`. Expected: PASS, including the existing Claude, Antigravity and Qwen tests.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): plan Copilot CLI's own hooks file and chained status line`

### Task 4: Live status, approvals and references from Copilot's hooks

**Files:**
- Create: `crates/core/src/agents/copilot/status.rs`, `crates/core/src/agents/copilot/request.rs`
- Modify:
  - `crates/core/src/agents/mod.rs`: `supports_permissions`, `permission_view`, `permission_output`, `release_output`.
  - `crates/core/src/server/hook.rs`:
    - the prompt take-out;
    - `starts_prompt`;
    - the fold match;
    - the context match;
    - the version-gate table;
    - `fold_live_with`, with its status-line gate;
    - the hold, with the pass rules below.

    The WSL agent match stays as it is: Copilot in WSL is 6n2.
  - `crates/core/src/server/mod.rs`: `copilot_home: Option<PathBuf>`, `with_copilot_home`, and Copilot in the version cache.
- Test: `crates/core/tests/hook.rs`, `crates/core/tests/permissions.rs`, and inline tests

**Interfaces:**
- Consumes: Task 3's `version_gate`, `context_output` and the fixtures.
- Produces:
  - `copilot::status::fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus>`
  - `copilot::status::fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus`
  - `copilot::request::detail(request: &Value) -> PermissionDetail`
  - `copilot::request::view(p: &PendingPermission) -> Option<PermissionView>`
  - `copilot::request::permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str>`
  - `copilot::request::passes(request: &Value) -> bool`: true for a request no card may hold.
  - `AppState::with_copilot_home(self, user_home: PathBuf) -> Self`

The fold reads `hook_event_name` from Q3's bodies:

| Event | Result |
| --- | --- |
| `sessionStart` | Idle. After an Ended from `/clear` (Q9), the row is live again. |
| `userPromptSubmitted` | Working |
| `preToolUse` | Working, with a phrase from the tool and Q3's `toolArgs` (a JSON string is parsed first): "Running <command>" for the shell tool, "Editing <relative file>" for writes and edits, "Reading <relative file>" for reads, and Qwen's phrase for any other tool. |
| `postToolUse`, `postToolUseFailure` | Working |
| `permissionRequest` | Needs permission |
| `notification` with `permission_prompt` (its body names the event `Notification`, which the fold takes as `notification`) | Needs permission. This is Copilot's own dialog, with no card behind it. |
| `notification`, any other type | No change. `agent_idle` is a background agent, not the turn ending. |
| `agentStop` | Idle (Done, as Claude's) |
| `errorOccurred` with `recoverable: false` | Idle, with the error phrase Claude's `StopFailure` uses. With `recoverable: true`, no change. |
| `sessionEnd` | Ended |

- The bodies carry no mode, so the row's mode stays the launch mode. There's no `mode_of` and no `set_live_mode` call for Copilot. If Q8 finds `allow_all_enabled` or a mode in the status line, `fold_statusline` leaves it unread in 6n1, and the finding notes it for later.
- The status line sets the model and context % from Q8's keys. A context window size of 0 or missing leaves context % unknown, not 0 %. There's no plan meter.

Approvals:
- A Copilot `permissionRequest` from an open tab whose effective agent is `copilot` is held for a card (decision 4), unless `passes` says otherwise:
  - `toolInput.requestSandboxBypass` is `true`, because Copilot ignores a hook's allow for it;
  - the rule Step 8 records for Q4, if any.

  A request that passes is answered at once with 204, so the bridge prints nothing.
- Allow gives `{"behavior":"allow"}`.
- Deny gives `{"behavior":"deny","message": common::DENIED_TEXT}`.
- Always allow gives `Err` with decision 5's copy.
- `release_output("copilot", _)` is `None`.
- The card's detail comes from `toolName` and `toolInput`. An unknown tool shows as other, with its raw arguments flattened by `common::flatten_input`. Nothing keys on a tool id unless Q3 found one.
- The card's risk line uses the same risky-command rules the Claude card uses.

The prompt:
- For `copilot`, `prompt` and `initialPrompt` are taken out of the body before anything else sees it.
- `starts_prompt` is a `userPromptSubmitted` whose taken-out `prompt` isn't empty.
- The context answer is `copilot::context_output`. Only `userPromptSubmitted` ever gets one: a note on `notification` would wake the session as a user message, and one on `preToolUse` or `postToolUse` would reach the model.

- [ ] **Step 1: Write the failing tests.**
  - In `copilot/status.rs`, one test per table row, each fed the Task 1 fixture for that event:
    - `session_start_is_idle`, `a_prompt_is_working`, `a_shell_tool_says_running_its_command`;
    - `tool_args_as_a_json_string_are_read`;
    - `a_permission_request_needs_permission`, `a_permission_prompt_notification_needs_permission`, `agent_idle_changes_nothing`;
    - `agent_stop_is_done`, `an_unrecoverable_error_is_idle_with_its_phrase`, `a_recoverable_error_changes_nothing`;
    - `session_end_ends_it`, `clear_ends_then_starts_again`;
    - `the_status_line_sets_model_and_context`, `a_status_line_with_no_window_size_leaves_context_unknown`.
  - In `copilot/request.rs`:
    - `a_shell_request_shows_its_command`, `a_write_shows_the_file`, `an_unknown_tool_shows_raw_arguments`;
    - `allow_and_deny_answer_in_copilots_shape`, `always_allow_is_refused_with_the_copy`;
    - `a_sandbox_bypass_request_passes`.
  - In `tests/hook.rs`, through the real router with `with_copilot_home(<temp>)` and a fixed version `1.0.106`:
    - `copilot_events_fold_into_the_tabs_live_status`
    - `a_copilot_prompt_is_never_logged_or_kept`: the hook log and the event ring hold no `prompt` and no `initialPrompt` key.
    - `a_copilot_prompt_naming_an_alias_gets_the_note_as_additional_context`
    - `no_other_copilot_event_gets_the_note`: a `notification`, a `preToolUse` and an `agentStop` naming an alias each answer 204.
    - `an_unrecorded_copilot_version_gets_the_note_and_no_status`
    - `copilot_status_off_says_so`
  - In `tests/permissions.rs`:
    - `a_copilot_permission_request_is_held_for_a_card`
    - `allow_from_the_card_answers_copilot`
    - `a_released_copilot_card_answers_nothing`: 204, so the bridge prints nothing.
    - `a_copilot_request_from_an_unknown_tab_or_a_tab_running_another_agent_passes`
    - `a_copilot_sandbox_bypass_request_is_not_held`
    - `two_copilot_tabs_get_their_own_answers`
    - `a_held_copilot_request_times_out_before_the_bridge`: the core's deadline is less than the bridge's wait, and the bridge's wait is less than 600 s.
    - Plus Step 8's Q4 rule, if any (for example `a_copilot_request_from_a_bypass_tab_is_not_held`).
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 copilot`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Write `status.rs` and `request.rs`, then the `"copilot"` arms at each listed spot.
  - In `fold_live_with`, the status-line role is admitted for `claude | antigravity | qwen | copilot`.
  - Copilot gets its own `Gate::Known if agent == "copilot"` arm. It has no Done hold, subagent count or stuck check.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS. Note in the ledger any test that already failed on `origin/main`.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): Copilot CLI's hooks drive live status, approve cards and project references`

### Task 5: Setup, removal and launch: Copilot becomes an adapter agent

**Files:**
- Create: `crates/core/src/agents/copilot/exe.rs`
- Modify:
  - `crates/core/src/agents/manifests.json`: remove the `copilot` entry.
  - `crates/core/src/agents/manifest.rs`:
    - `ADAPTER_IDS` and `ADAPTER_EXES` gain `copilot`;
    - the shipped-manifests test lists the ids without `copilot`;
    - an own agent may not take `copilot`.
  - `crates/core/src/agents/copilot/mod.rs`:
    - `pub fn new_session_id() -> String`: a lowercase v4 UUID from `getrandom`.
    - `pub fn link_body(body: &Value, copilot_home: &Path) -> Option<Value>`: `{hook_event_name, session_id, transcript_path, cwd}`, read from `sessionId` and `cwd`. `transcript_path` is the body's `transcriptPath` when present, otherwise `<copilot_home>/session-state/<sessionId>/events.jsonl` (Q9). None without a valid `sessionId`.
  - `crates/core/src/profiles/detect.rs`:
    - `agent_command`: `"copilot" => Some("copilot")`, the word for WSL and for copy.
    - `adapter_of_exe`: `"copilot"`.
    - `adapter_of_package`: `"@github/copilot"`.
    - `mode_args` as the manifest had them: Manual `[]`, Plan `["--plan"]`, Bypass `["--allow-all"]`. Any other mode is refused, as before.
    - The prompt flag `-i`.
    - `resume_args`: `Session(id)` gives `[format!("--resume={id}")]`, one argument. `Last` and `Fork` give `["--continue"]`.
    - `pub fn new_session_args(agent: &str, id: &str) -> Vec<String>`: `["--session-id", id]` for `copilot`, empty for every other agent.
  - `crates/core/src/server/sessions.rs`:
    - the display name "Copilot CLI";
    - `program_command`: for `copilot` off WSL, `exe::resolve` in `spawn_blocking` returns the full path. It searches the PATH folders the finder searches (the profile's PATH when set, otherwise the process's), then `LOCALAPPDATA`. `Missing::Rejected` gives a 400 with the shim copy, and `Missing::NotOnPath` the not-found copy. On WSL it is `copilot`.
    - a new, not resumed, Copilot launch gets `new_session_args` with `new_session_id()`. The core then links the tab to that id at once, through `history().note_event("copilot", …, &link_body(…))`, so Restart finds it with the hooks off as well (decision 10).
    - the `resume_session` gate gains `copilot`.
  - `crates/core/src/server/hook.rs`:
    - For `copilot`, the body given to `history().note_event` is `link_body` of it.
    - A Copilot `sessionEnd` whose `sessionId` isn't the tab's latest linked session (`history().latest_for_tab_of(tab, "copilot")`) changes nothing. Copilot sends `/clear`'s `sessionEnd` detached, so it can arrive after the new session's `sessionStart`.
  - `AppState::agent_exe_of("copilot")`, the program whose `--version` the version cache reads, is `exe::resolve`'s path.
  - `crates/core/src/history/recorder.rs`: `"codex" | "qwen" | "copilot"` link by the first event that names them, and ask for no copy.
  - `crates/core/src/history/mod.rs`: `sidecar_allowed_in` for `copilot`:
    - the id is a UUID;
    - the path is absolute, local and free of `..`, and ends in `session-state/<id>/events.jsonl`;
    - any distro is refused, because WSL is 6n2.
  - `crates/core/src/history/index.rs`: `filters()` also excludes `'copilot'` until 6n2's importer.
  - `crates/core/src/server/agents.rs`:
    - `Adapter::Copilot`, `adapter()`, `cli_name` "Copilot CLI".
    - Setup, plan and remove arms, with a `copilot_user_home` resolver that honours `with_copilot_home`. `AgentSetup.settings_path` is `settings.json` and `hooks_path` is `marshell.json`. `skips_all` comes from Task 3.
    - The list of CLIs that can start.
    - `sign_in = SignIn::NoCommand`, the settings view not editable, and no skill, until 6n2.
  - `crates/core/src/server/catalog.rs`: `("copilot", "GitHub Copilot", "Copilot CLI", PromptStyle::Flag { flag: "-i".into() })`, found through `exe::resolve`.
  - `crates/core/src/diagnostics/export.rs`: `CLIS` gains `copilot`.
  - `crates/cli/src/uninstall.rs`: delete Copilot's `marshell.json` and restore its status line, in the same order and with the same error handling as Qwen's. A file it can't read or plan is reported, and the rest goes on.
  - Not touched (6n2 and 6n3): `agents/install.rs`, `agents/distro.rs`, `agents/settings_view.rs`, `agents/skill.rs`, `server/history.rs`, and every Doctor file.
- Test: `crates/core/tests/agents_api.rs`, `crates/core/tests/catalog_api.rs`, `crates/cli` tests, and inline tests in `exe.rs`, `detect.rs`, `sessions.rs` and `history`.

**Interfaces:**
- Consumes: Task 3's `home`, `install_plan`, `remove_plan`, `inspect` and `skips_all`, and Task 4's `with_copilot_home`.
- Produces:
  - `copilot::exe::resolve(path_dirs: &[PathBuf], local_app_data: Option<&Path>) -> Result<PathBuf, identify::Missing>`:
    1. The first `copilot.exe` or `copilot.cmd` in a PATH folder whose path has no `copilot-shim` segment. The check ignores case and slash direction.
    2. Otherwise `<local_app_data>/GitHubCopilotCLI/copilot.exe` if it is a file.
    3. Otherwise `Rejected { path }` when only a shim was seen, and `NotOnPath` when nothing was.
  - `/v1/agents/copilot/setup`, `/plan` and `/remove` answer as Qwen's do.

- [ ] **Step 1: Write the failing tests.**
  - `exe.rs`:
    - `the_shim_is_skipped_for_a_later_path_entry`
    - `the_winget_folder_is_used_when_path_has_only_the_shim`
    - `only_the_shim_is_rejected`
    - `nothing_is_not_on_path`
    - `a_shim_folder_in_either_case_or_slash_is_skipped`
  - `detect.rs`:
    - `copilot_resumes_by_session_id_in_one_argument`: `--resume=<uuid>`.
    - `copilot_continues_without_an_id`
    - `a_new_copilot_session_gets_its_id`
    - `copilot_modes_are_the_manifests`
    - `a_copilot_exe_or_npm_package_is_copilot`
  - `copilot/mod.rs`:
    - `new_session_ids_are_v4_uuids`: matches `^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`, and two calls differ.
    - `link_body_derives_the_transcript_path`
    - `link_body_needs_a_session_id`
  - `history`:
    - `a_copilot_transcript_path_is_allowed`
    - `a_copilot_path_outside_session_state_or_with_dotdot_is_refused`
    - `copilot_is_linked_by_its_first_event`
  - `sessions`:
    - `a_new_copilot_tab_runs_the_real_exe_with_a_session_id`: the argv holds the full path and `--session-id <uuid>`, and the tab is linked to that id.
    - `a_copilot_launch_with_only_the_shim_is_refused_with_the_copy`
    - `a_copilot_tab_resumes_its_chosen_session`: its `events.jsonl` exists, and `--resume=<id>` follows.
    - `a_copilot_tab_whose_transcript_is_gone_starts_fresh`
  - `tests/hook.rs`: `a_late_session_end_from_clear_is_ignored`. A `sessionStart` for id B comes after id A's, then A's `sessionEnd` arrives: the row stays live.
  - `agents_api.rs`:
    - `copilot_setup_reports_none_then_installed_after_apply`
    - `copilot_plan_shows_the_hooks_file_the_settings_diff_and_the_chain`
    - `copilot_remove_deletes_the_file_and_restores_the_status_line`
    - `copilot_orphaned_entries_are_reported`
    - `a_marshell_json_the_person_wrote_is_reported_not_overwritten`
    - `copilot_setup_reports_skips_all`
  - `catalog_api.rs`: `copilot_is_found_past_the_shim`
  - `uninstall`: `uninstall_removes_copilots_hooks_and_restores_its_status_line`
- [ ] **Step 2: Run them.** `cargo test --workspace -j 4 copilot`. Expected: FAIL.
- [ ] **Step 3: Implement** each listed arm.
- [ ] **Step 4: Run.** `cargo test --workspace -j 4`, then `pnpm check:generated`. Expected: PASS, and `cargo test -p marshell-core -j 4 doctor` is unchanged.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): Copilot CLI is set up, launched past VS Code's shim and resumed by its own session id`

### Task 6: Copilot in the Agents section, onboarding, launcher, rows and cards

**Files:**
- Modify:
  - Unions:
    - `src/lib/api/agents.ts` (`HookAgent`, and the later list in `toDetections`)
    - `src/features/onboarding/types.ts` (`CliId`)
  - Agents section and onboarding:
    - `src/features/agents/AgentsSection.tsx`:
      - `agentTitle`, the props, the state and `getSetup`;
      - the `setups`, `skillFlows` and `installs` maps.

      Copilot's skill row says the skill isn't available for Copilot CLI yet (6n2).
    - `src/features/agents/agentsState.ts`
    - `src/features/onboarding/LiveOnboarding.tsx`: the state, the flows, `consentQueue` (after Qwen), and `getSetup`.
    - `src/features/onboarding/consent.ts`: `CLI_NAME` "Copilot CLI", `CAPABILITIES`, `entryCount`.
    - `src/features/onboarding/Detected.tsx` and `src/features/onboarding/emptyStates.ts`
    - `src/features/settings/ProjectsSection.tsx`: `CLIS_HELP` lists Copilot CLI as taking references. If Q7 failed, it says "Copilot CLI can't take project references yet."
  - Approve card:
    - `src/features/approval/ApproveCard.tsx`: a Copilot card offers Allow and Deny, with no Always allow. Its release text is the "asking in its own terminal now" one.
  - Modes, launcher and sessions:
    - `src/features/modes/sessionModes.ts`: `isAdapter`, the mode map (Manual, Plan, Bypass), `launchArgs` (`--plan`, `--allow-all`), `classifies`, `switchKind`.
    - `src/features/launcher/launchChoice.ts`: the entry comes from the adapter list, not the manifest list.
    - `src/features/sessions/launchPrograms.ts`: `ADAPTER_AGENTS`.
    - `src/features/sessions/resumeCommand.ts`: `copilot: "copilot --continue"`.
    - `src/features/sessions/sessionActions.ts`: `CLOSE_BAR_NAMES`.
    - `src/features/sessions/LiveApp.tsx`: the hook-state loops and the `runningCount` record.
    - `src/features/sessions/toRow.ts`: `isHookAgent` and `isManifestAgent`.
  - Not touched, because 6n2 and 6n3 own them: `src/features/accounts/*`, `src/features/companies/*`, `settingsSections.ts`'s `CompanyAgent`, `src/features/history/*` and `src/features/doctor/*`.
- Test: the `.test.ts(x)` beside each file, plus `src/features/onboarding/fixtures.ts` and `src/features/agents/fixtures.ts`, which gain a Copilot setup.

**Interfaces:**
- Consumes: the `copilot` id on `/v1/agents/{agent}/...` (Task 5).

- [ ] **Step 1: Write the failing tests.**
  - `AgentsSection.test.tsx`:
    - `lists Copilot CLI with its hook state and a Set up button`
    - `Copilot's skill row says the skill isn't available for Copilot CLI yet`
  - `ConsentCard.test.tsx` and `consentCopy.test.ts`: `Copilot's card is titled "Set up Copilot CLI hooks" and its removal uses the removal copy`
  - `ApproveCard.test.tsx`: `a Copilot card has Allow and Deny and no Always allow`
  - `sessionModes.test.ts`: `Copilot starts Plan with --plan and Bypass with --allow-all`
  - `toRow.test.ts`: `a Copilot row waits for hooks and is not status limited`
  - `resumeCommand.test.ts`: `copilot --continue`
  - `LiveOnboarding.test.tsx`: `Copilot's consent comes after Qwen's`
  - `ProjectsSection.test.tsx`: `the references help names Copilot CLI`
- [ ] **Step 2: Run them.** `pnpm test src/features src/lib`. Expected: FAIL.
- [ ] **Step 3: Implement** each arm. The copy is exactly as in the global constraints. A new button or line works by keyboard and in forced colours, using the existing classes.
- [ ] **Step 4: Run.** `pnpm test`, then `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(ui): Copilot CLI is set up, launched and answered like the other adapter agents`

### Task 7: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder and `-CopilotFake`. Put a status line of the person's own in the scratch `settings.json` first.
  - Over CDP:
    - Set up Copilot's hooks from the Agents section, and read the diff for `marshell.json`, `settings.json` and the chain.
    - Open a Copilot tab in Manual from the launcher.
      - The argv runs `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe` by its full path, with `--session-id <uuid>`.
      - Send `RUN echo marshell`. The row goes Working, then Needs permission.
      - The card shows the command, and Allow runs it.
      - The row returns to Idle with the model and context %.
    - Send another `RUN` and release the card: Copilot's own dialog appears in the terminal.
    - Deny one: the model got the message (fake log).
    - Try "Always allow", and read decision 5's copy.
    - With a project alias, send a prompt naming it: the note is in the fake log.
    - Check Step 8's Q4 rule in a Bypass tab, if there is one.
  - Restart the tab and read the argv: `--resume=<the same uuid>`.
  - Two Copilot tabs ask at once, and each card answers its own.
  - Turn the hooks off:
    - `hooks\marshell.json` is gone;
    - the scratch `settings.json` is byte-equal to what it was before the setup;
    - the chain file is gone.
- [ ] **Step 2:** Write `docs/research/2026-11-06-copilot-adapter-live-check.md`: each check with its evidence, and the findings numbered. Wherever `docs/PLAN.md` lists the CLIs Marshell installs hooks for, add Copilot CLI.
- [ ] **Step 3:** Stop the check by PID, after listing the tree and looking for agent CLIs the person started. Ask before removing the scratch folder.
- [ ] **Step 4:** Commit: `docs(research): record slice 6n1's live check`. Then run the final whole-branch review (marshell-reviewer, Opus), one fix pass, and `marshell-ship`.
