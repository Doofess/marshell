# Slice 6d: the Antigravity CLI adapter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Antigravity CLI (`agy`) gets what Claude Code and Codex have: hooks and a chained status line installed and removed through the config service, live status in its row, and its tool requests answered from the approve card, Departures and banners. Every piece is checked against a real agy first.

**Architecture:** A new `crates/core/src/agents/antigravity/` module, shaped like `agents/codex/`: `hooks.rs` (the plan for `~/.gemini/config/hooks.json` and the `statusLine` in `~/.gemini/antigravity-cli/settings.json`), `status.rs` (hook events and the status line folded into `LiveStatus`), `request.rs` (the card's view of a `PreToolUse`, and agy's answer JSON) and `policy.rs` (what the hook answers at once, without a card). agy's payloads carry no event name, so the bridge takes `--event <Name>` and writes `hook_event_name` into the body; the core's routes, fold, history and holds stay agent-neutral and gain an `"antigravity"` arm where they match `"codex"` today. The config service gains a `DeleteFile` edit (decision 6).

**Tech stack:** Rust (axum, serde_json, the `config` service and its JSONC edits), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` "Corrections to the brief" item 1 (Antigravity), "Session modes", "Config safety copy", and §9 "Slice 6d decisions (2026-10-26)". Evidence: `docs/research/2026-10-15-local-install-check.md` (Antigravity) and the bundled `agy-customizations/docs/hooks.md`; Task 1 writes `docs/research/2026-10-26-antigravity-hooks.md`.

**Checked before planning (2026-10-26):**
- `agy --version` prints `1.4.0`; the bundled changelog runs to 1.4.7. Task 1 records the version that runs.
- agy has no home-override variable (a scan of the binary found none). With `USERPROFILE` set to a scratch folder, `agy mcp list` read `<scratch>\.gemini\config\mcp_config.json`, and nothing under the real `~/.gemini` changed.
- The two copied files (`oauth_creds.json`, `google_accounts.json`) sign a scratch agy in: `agy models` listed the models.
- The core's `server::files::home_dir()` reads `USERPROFILE` too, so an app started with a scratch `USERPROFILE` plans agy's files in the same scratch home the agy tabs use.
- Baseline on `origin/main` (6b305ab): `pnpm test` 338 files, 5034 tests pass. `cargo test --workspace -j 4`: 1148 pass, 2 ignored, 1 fails under the full parallel run only: `history::storage::backup::tests::an_install_id_that_cannot_be_read_is_not_replaced`, which passes alone (3 of 3). It predates 6d and isn't touched here.

## Global constraints

- Marshell writes only `~/.gemini/config/hooks.json` and `~/.gemini/antigravity-cli/settings.json` (its `statusLine`, and `dangerouslySkipPermissions` when the person hands the mode over), plus its own `~/.marshell/agents/antigravity/statusline-chain.json`. It never writes a workspace `.agents/hooks.json`, a plugin's `hooks.json`, `config.json`, or anything else under `~/.gemini`.
- Marshell's entries live under one named hook, `"marshell"`, in `hooks.json`. The person's own named hooks are left byte-equal on install and removal.
- Tests and live checks never read or write the real `~/.gemini`. Tests inject paths (`AppState::with_agy_home(<temp>)`). The live check starts the app with a scratch `USERPROFILE` (`live-check.ps1 -AgyAuth`), which copies `~/.gemini/oauth_creds.json` and `google_accounts.json` into the scratch home and reads nothing else there; `stop` deletes them with the scratch folder (decision 1).
- No new dependency.
- New protocol fields go in `crates/protocol`, and `pnpm check:generated` passes afterwards. An `Option<u64>` gets `#[ts(type = "number | null")]`.
- No new route. The `/v1/agents/{agent}/...` routes accept `antigravity`; unknown ids still get 404 "no such agent".
- Never hold a `MutexGuard` across `.await` or file I/O. `agy --version` and file reads run in `spawn_blocking`.
- Fixtures come from the real agy in Task 1 and are scrubbed with `pnpm scrub:fixtures` before they are committed (`fixtures_scrubbed.rs` enforces this).
- Antigravity's conventions stay in `agents/antigravity/`. `agents/claude/` and `agents/codex/` gain no Antigravity branch. Shared pieces move to `agents/common.rs`.
- The bridge never answers `decision: continue` to `Stop`, never injects steps from `PreInvocation`, and never sends `overwrite` (PLAN, Corrections item 1).
- Every UI change works with the keyboard alone and in forced colours. Each CLI keeps its own consent card, with no bulk allow.
- Copy, exactly:
  - Change titles: "Set up Antigravity hooks", "Turn Antigravity hooks off".
  - Status notes: "Live status is off for Antigravity"; "Antigravity's version could not be read"; "Antigravity <v> is older than the versions Marshell knows"; "Antigravity <v> is not one Marshell has recorded yet".
  - Always allow refused (decision 5): "Antigravity can't keep an always-allow rule from Marshell. Allow once, or answer in the terminal."
  - Removal copy (PLAN, "Config safety copy"): "Marshell removes the hook entries it added, deletes hooks.json if nothing else is in it, and puts your own status line back from statusline-chain.json; the rest of each file stays as it is".
  - Hand-over undo: "turns dangerouslySkipPermissions back on".
  - A Needs you raised by the status line alone (decision 3): phrase "Waiting for you in the terminal", marked Status limited.

## Assumptions Task 1 confirms or corrects

Task 1 checks each one against the real agy. If a finding differs, the plan is amended (shapes, constants, copy) and committed before Task 2 starts. Findings and evidence go in `docs/research/2026-10-26-antigravity-hooks.md`.

- **A1.** `hooks.json` is `{ "<name>": { "enabled": true, "PreToolUse": [{ "matcher": "<regex>", "hooks": [{ "type": "command", "command": "<string>", "timeout": <secs> }] }], "PostToolUse": [...], "PreInvocation": [{ "type": "command", ... }], "PostInvocation": [...], "Stop": [...] } }`, read from `~/.gemini/config/hooks.json` with no trust step. The finding notes whether `SessionStart` (in the binary, not the doc) fires, and whether an unknown event name is a warning or an error.
- **A2.** On Windows the command runs through `cmd /c` (the doc), with `"<bridge>" <role> --agent antigravity --event <Name> --tag mshl1` working for a bridge path with a space. The finding records the real shell (6a found Codex used PowerShell, not what its docs said) and what a `%`, `&` or `^` in the path does under it (the analogue of 6a decision 15).
- **A3.** What agy does with each `PreToolUse` answer, for `run_command` and for `view_file`, in its default mode: empty stdout, `{}`, a non-zero exit, invalid JSON, a timeout, `allow`, `deny` with `reason`, `ask`, `force_ask`. The same with `dangerouslySkipPermissions: true` in settings and with `--dangerously-skip-permissions`, and whether `PreToolUse` fires at all under either. The answers fix the bridge's no-tab answer (Task 2), the release answer (decision 4) and the R1 options.
- **A4.** `MARSHELL_HOME` and `MARSHELL_TAB_ID` reach the hook process. Two agy tabs at once each report their own tab id and `conversationId` (no shared daemon, unlike Codex in 6a decision 12). The finding notes whether `ANTIGRAVITY_CONVERSATION_ID` is in the hook's environment.
- **A5.** Payloads are camelCase with `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `modelName`; `PreToolUse` adds `toolCall{name,args}` and `stepIdx`; `PostToolUse` adds `stepIdx` and `error`; `PreInvocation`/`PostInvocation` add `invocationNum` and `initialNumSteps`; `Stop` adds `executionNum`, `terminationReason`, `error` and `fullyIdle`. The finding names the exact tool names and argument keys for a shell command, an edit, a new file, a read inside and outside the workspace, and a URL read, and whether `lastUserInput` is present (1.4.7 changelog).
- **A6.** The status line is `"statusLine": { "type": "command", "command": "<string>" }` with optional `padding`, `enabled`, `stack_with_default`, and gets JSON on stdin with `model`, `context_window.used_percentage`, `quota`, `agent_state`, `tool_confirmation_pending`, `pending_input_count`, `execution_mode`, `transcript_path`, `version`. The finding records the exact keys, how often agy runs it while idle and while working, how soon `tool_confirmation_pending` turns true after agy shows its own prompt, and whether empty output leaves agy's own line in place (with and without `stack_with_default`).
- **A7.** `permissionOverrides` on an `allow` is a grant for this one call only, and agy keeps no rule from it (decision 5). The finding checks a second identical command and whether `~/.gemini/config/config.json` or `settings.json` changed.
- **A8.** `agy --conversation <id>` resumes that conversation in a new process. `transcriptPath` is `<user home>/.gemini/antigravity-cli/brain/<conversationId>/.system_generated/logs/transcript.jsonl`, and the finding notes when the file first exists (at the first prompt, or later).
- **A9.** `agy --version` prints the bare version (`1.4.0`), which `parse_any_version` reads.

**Rulings asked after Task 1** (through AskUserQuestion, recorded as §9 decisions 7 and 8, then folded into Tasks 2, 4, 6 and 7):

- **R1: what gets a card.** `PreToolUse` fires before every tool, not only when agy would ask, so holding each one for a card would ask about every file read. The options depend on A3. The planned default is PLAN's "Hybrid" from "Session modes", done in the core: the hook answers at once by the tab's mode (reads pass; Manual asks about every edit and command; Auto-edit lets in-project edits pass; Full auto and Bypass ask only about the risky list), and agy starts in its strictest mode. An alternative is a matcher on Marshell's blocking entry, so read tools never reach it.
- **R2: the plan meter.** agy's `quota` is one bucket per model with `remaining_fraction` and `reset_time`, and its window isn't known to be five hours. The row and the footer label the meter "5h". The planned default leaves Antigravity's meter out in 6d (model and context % only) unless Task 1 shows a five-hour window.

## Review focus

1. **agy run outside a Marshell tab with the hooks installed** (Windows Terminal, Marshell closed, or another Antigravity app reading the shared `hooks.json`). The bridge's answer must leave agy exactly as it would be with no hook: it never allows and never forces a prompt agy wouldn't show. The answer is the one A3 shows to be neutral. Tests in Task 2.
2. **A person's own `hooks.json` and `settings.json`** (BOM, CRLF, their own named hook, their own `statusLine`). Their bytes stay as they were, apart from Marshell's entries, on install and on removal, and `hooks.json` is deleted only when nothing else is in it. Tests in Tasks 4 and 5.
3. **`dangerouslySkipPermissions: true` left on** (it is on on the dev machine). Sessions show as Bypass and the mode chip says why. Marshell's card still guards the risky list if A3 shows `PreToolUse` fires, and a release never turns a skipped prompt into an allow. Tests in Tasks 7 and 9.
4. **Two agy tabs asking at once.** Each card answers its own request and never the other tab's. Test in Task 7.
5. **agy updated to a version Marshell hasn't recorded.** The row gets the "not one Marshell has recorded yet" note instead of guessed status. Approvals still reach the card, and an unknown tool shows as "other" with its raw arguments. Tests in Tasks 6 and 7.

---

### Task 1: Spike: real agy hooks and status line, the fixtures, and the rulings (main thread, live)

This task runs in the main thread, not through a worker. It needs real agy sessions in Marshell tabs.

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1`, `.claude/skills/marshell-live-check/SKILL.md`
- Create: `docs/research/2026-10-26-antigravity-hooks.md`
- Create: `crates/core/tests/fixtures/antigravity/hooks/<version>/*.json`, `crates/core/tests/fixtures/antigravity/statusline/<version>/*.json`
- Modify: `docs/PLAN.md` §9 (decisions 7 and 8), this plan

- [ ] **Step 1: Add `-AgyAuth` to `live-check.ps1 start`.** With the switch it creates `<Scratch>\home\.gemini`, copies `$HOME\.gemini\oauth_creds.json` and `google_accounts.json` into it and nothing else, and starts the app with `USERPROFILE` set to `<Scratch>\home`. It pins `CARGO_HOME` and `RUSTUP_HOME` to the real ones first, when they're unset, so the build still finds the toolchain. `stop` deletes those two copies once the tree is gone. Add one line to SKILL.md's start step saying what the switch copies and that `stop` deletes it.
- [ ] **Step 2: A recorder for every event.** In the scratch folder, write a small recorder script. It saves its stdin and the values of `MARSHELL_TAB_ID`, `MARSHELL_HOME` and `ANTIGRAVITY_CONVERSATION_ID` to `<Scratch>\rec\<Event>-<n>.json`, notes which shell ran it, and prints the contents of `<Scratch>\answer\<Event>.txt` (empty when missing). Point every event of A1 at it in `<Scratch>\home\.gemini\config\hooks.json`, under a named hook `marshell-spike`, with the scratch bridge folder's space-containing path in the command (A2). Point `statusLine` at a second recorder that logs each stdin with a timestamp.
- [ ] **Step 3: Drive a session.** Start the check with a fresh literal scratch folder and `-AgyAuth`, and open an agy tab in a scratch project. Over CDP, send prompts that run a shell command, edit a file, create a file, read a file inside and outside the workspace, and read a URL. Answer agy's own prompts in the terminal. Then start a second agy tab and send a prompt in each (A4).
- [ ] **Step 4: The A3 matrix.** For `run_command` and `view_file`, set `answer\PreToolUse.txt` to each answer in A3 in turn, with the hook timeout at 5 s for the timeout case, and record what agy did (ran it silently, showed its own prompt, refused, or errored). Repeat the `ask`, `{}` and empty cases with `dangerouslySkipPermissions: true` in the scratch `settings.json`, then with `--dangerously-skip-permissions`. Check A7 with `permissionOverrides`.
- [ ] **Step 5: The status line and resume.** Record A6 (keys, cadence idle and working, `tool_confirmation_pending` timing, empty output with and without `stack_with_default`). Quit agy, run `agy --conversation <id>` in the same tab, and record A8. Run `agy --version` (A9).
- [ ] **Step 6: Write the fixtures.** Copy one recorded body per case into `crates/core/tests/fixtures/antigravity/hooks/<version>/`, naming each `<Event>[-<case>].json` (for example `PreToolUse-run_command.json`, `PreToolUse-<edit tool>.json`, `PreToolUse-<write tool>.json`, `PreToolUse-view_file.json`, `PostToolUse.json`, `PreInvocation.json`, `PostInvocation.json`, `Stop.json`, and `Stop-error.json` if one was seen), and the status-line bodies into `statusline/<version>/` (`idle.json`, `working.json`, `pending.json`). Run `pnpm scrub:fixtures crates/core/tests/fixtures/antigravity`, then `cargo test -p marshell-core -j 4 --test fixtures_scrubbed`. Expected: PASS.
- [ ] **Step 7: Write `docs/research/2026-10-26-antigravity-hooks.md`.** It answers A1 to A9, each with evidence, plus the agy version, the exact `hooks.json` and `statusLine` that ran, and the real `~/.gemini` check: the newest modified time under it before and after. Stop the check by PID (list the tree first and look for agent CLIs the person started) and remove the scratch folder after showing its path and getting an OK.
- [ ] **Step 8: Ask R1 and R2** through AskUserQuestion with the findings in the options. Record the answers as §9 decisions 7 and 8. Then amend this plan wherever a finding or ruling differs: the event table and matchers in Task 4, the bridge's neutral answer in Task 2, the fold table in Task 6, the policy and release answer in Task 7. Commit:

```bash
git add .claude/skills/marshell-live-check docs/research/2026-10-26-antigravity-hooks.md crates/core/tests/fixtures/antigravity docs/PLAN.md docs/superpowers/plans/2026-10-26-phase-6d-antigravity-adapter.md
git commit -m "docs(research): record agy's hooks, status line and payloads, with scrubbed fixtures"
```

### Task 2: The bridge's `--event` argument and its neutral answer

**Files:**
- Modify: `crates/hook/src/args.rs`, `crates/hook/src/lib.rs`, `crates/hook/src/permission.rs`, `crates/hook/src/record.rs`
- Test: unit tests in those files; `crates/hook/tests/` if a bridge integration test lives there

**Interfaces:**
- Produces: `Args { role, agent, tag, event: Option<String> }`. `--event <Name>` is accepted once, with a non-empty value that is an identifier (`[A-Za-z]+`); anything else is an argument error, as an unknown flag is today.
- Produces: when `event` is set and the stdin JSON is an object with no `hook_event_name`, the bridge adds `"hook_event_name": "<Name>"` before posting. An existing `hook_event_name` is never replaced. The `record` role saves the body as posted, under a file name that carries the event.
- Produces in `permission.rs`: `pub fn neutral(agent: &str) -> &'static [u8]`: what the bridge prints for a `permission` call it can't take to a card (no tab, no core, the core's 204, the core's timeout, an answer that fails `accepted`). Claude and Codex: empty, as today. Antigravity: the answer A3 shows leaves agy as it would be with no hook (empty if empty is neutral; otherwise the neutral JSON A3 names).

- [ ] **Step 1: Write the failing tests.**
  - `args.rs`: `parses_an_event`, `an_event_needs_a_value`, `an_event_that_is_not_an_identifier_is_an_error`, `a_second_event_is_an_error`.
  - `lib.rs`: `the_event_is_written_into_the_body` (body `{"conversationId":"c"}` with `--event Stop` posts `hook_event_name: "Stop"`), `an_existing_event_name_is_kept`, `a_body_that_is_not_an_object_is_posted_as_it_is`.
  - `permission.rs` or `lib.rs`: `antigravity_outside_a_tab_prints_the_neutral_answer` (no `MARSHELL_TAB_ID`), `antigravity_with_no_core_prints_the_neutral_answer` (no endpoint), `claude_and_codex_outside_a_tab_print_nothing`.
- [ ] **Step 2: Run `cargo test -p marshell-hook -j 4`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's command.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(hook): take agy's event name from --event and answer neutrally outside a tab`.

### Task 3: The config service's `DeleteFile` edit (decision 6)

**Files:**
- Modify: `crates/protocol/src/config.rs` (`EditOp`), `crates/core/src/config/preview.rs`, `crates/core/src/config/service.rs`, `crates/core/src/config/write.rs` if needed
- Modify: `src/generated/EditOp.ts` (generated); the frontend's change-set preview if it switches on `EditOp`'s kind
- Test: unit tests in `preview.rs` and `service.rs`

**Interfaces:**
- Produces: `EditOp::DeleteFile` (no fields), applied to a file whose hash must equal `expected_hash`, which is required for this op. Preview: a unified diff from the file to nothing, with `after_hash: None` and `FileStatus::Changes`. A missing file is `NoChange`. Apply: the snapshot is taken first, then the file is removed with `write.rs`'s retrying remove. Undo restores the snapshot's bytes. The service's guard applies as to every op (managed, outside the allowed roots, Marshell's own, read-only and too large are refused with their existing reasons).
- Produces: `Change::Deleted { path }` beside `Change::Created`, so undo knows to restore rather than remove.

- [ ] **Step 1: Write the failing tests.**
  - `delete_previews_every_line_removed`, `delete_of_a_missing_file_is_no_change`, `delete_without_an_expected_hash_is_refused` (`BadValue`), `delete_with_a_changed_file_is_a_conflict`, `delete_takes_a_snapshot_and_removes_the_file`, `undo_of_a_delete_puts_the_bytes_back` (BOM and CRLF content, byte-equal), `delete_of_a_managed_file_is_refused`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib config::`.** Expected: FAIL.
- [ ] **Step 3: Implement it.** Then run `cargo test -p marshell-protocol` and `pnpm check:generated`.
- [ ] **Step 4: Run Step 2's command and `pnpm test src/features/onboarding src/features/agents`.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(config): delete a file through the config service, with its snapshot and undo`.

### Task 4: The Antigravity hook and status-line plan

**Files:**
- Create: `crates/core/src/agents/antigravity/mod.rs`, `crates/core/src/agents/antigravity/hooks.rs`
- Modify: `crates/core/src/agents/mod.rs`, `crates/core/src/agents/common.rs` (move only, if a Claude status-line helper becomes shared)
- Test: unit tests in `antigravity/hooks.rs`, reusing the BOM, CRLF and comments texts from `crates/core/tests/fixtures/claude/settings/`

**Interfaces:**
- Consumes: `common::{TAG, PlanError, is_ours}`, `EditOp::DeleteFile` (Task 3), `paths::statusline_chain(home, "antigravity")`.
- Produces in `agents::antigravity::hooks`:
  - `pub const HOOK_NAME: &str = "marshell"`
  - `pub const STATUS_EVENTS: [&str; 4] = ["PostToolUse", "PreInvocation", "PostInvocation", "Stop"]`; `PreToolUse` is the approve entry (role `permission`, timeout 600 s; the bridge waits 590 s). With approve off, `PreToolUse` gets a status entry (role `event`, timeout 5 s) so the row can say what's running. Routine timeouts are 5 s. Task 1 may change this list (R1's matcher, `SessionStart`).
  - `pub fn hooks_path(user_home: &Path) -> PathBuf` = `<user_home>/.gemini/config/hooks.json`; `pub fn settings_path(user_home: &Path) -> PathBuf` = `<user_home>/.gemini/antigravity-cli/settings.json`
  - `pub fn hook_command(home: &Path, role: &str, event: &str) -> String`: `"<bridge>" <role> --agent antigravity --event <event> --tag mshl1`, with the bridge path quoted in the form Task 1 found safe for agy's shell (A2)
  - `pub fn statusline_command(home: &Path) -> String`: `"<bridge>" statusline --agent antigravity --tag mshl1`, in the same form
  - `pub struct Found { pub install: HookInstall, pub approve_installed: bool, pub has_own_statusline: bool, pub chained: bool, pub skips_all: bool }`
  - `pub fn inspect(hooks: Option<&Value>, settings: Option<&Value>, chain: Option<&Value>, home: &Path) -> Found`
  - `pub fn install_plan(user_home: &Path, hooks_text: Option<&str>, settings_text: Option<&str>, chain_text: Option<&str>, home: &Path, want: &HookPlanRequest) -> Result<ChangeSet, PlanError>`, titled "Set up Antigravity hooks"
  - `pub fn remove_plan(user_home: &Path, hooks_text: Option<&str>, settings_text: Option<&str>, chain_text: Option<&str>, home: &Path) -> Result<ChangeSet, PlanError>`, titled "Turn Antigravity hooks off"
- Rules:
  - Install sets `/marshell` to `{ "enabled": true, <events> }`, creating `hooks.json` when it's missing. Removal removes `/marshell`, and uses `DeleteFile` when no other key would be left.
  - The status line follows Claude's chain: the person's own `statusLine` object is kept whole in the chain file and put back on removal. Marshell's `statusLine` keeps the person's `padding` and `stack_with_default`; with no line of their own, it sets `stack_with_default` as Task 1 found keeps agy's own line showing (A6). A `statusLine` the person changed since the install is left alone, as Claude's is.
  - `want.hand_over` (new `#[serde(default)] pub hand_over: bool` on `HookPlanRequest`) adds a `JsonRemove` of `/dangerouslySkipPermissions` in `settings.json`, only when it is `true` there.
  - Scope `ConfigScope::User` and `Applies::NewSessions`, for both files.

- [ ] **Step 1: Write the failing tests.**
  - `install_into_missing_files_adds_the_hook_and_the_line`: one `/marshell` hook with each status event and the approve `PreToolUse`, every command carrying `--agent antigravity --event <Name> --tag mshl1`, `PreToolUse`'s with role `permission` and timeout 600, and a `CreateFile` for `hooks.json`.
  - `status_only_puts_an_event_entry_on_pre_tool_use`.
  - `install_keeps_the_users_own_named_hook` (byte-equal around ours).
  - `install_on_an_installed_file_changes_nothing`, and `inspect` reports `Installed` with `approve_installed`.
  - `an_entry_from_another_home_is_orphaned`.
  - `remove_with_only_ours_deletes_hooks_json`; `remove_with_a_users_hook_keeps_the_file` (their bytes equal, over the BOM, CRLF and comments texts).
  - `the_users_status_line_is_chained_and_put_back`; `a_status_line_changed_since_the_install_is_left_alone`.
  - `hand_over_removes_skip_all`; `hand_over_with_skip_all_off_changes_nothing`; `inspect_reports_skip_all`.
  - `an_unparsable_file_is_refused_with_its_line` (`PlanError::DoesNotParse`, for each file).
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib antigravity::hooks`.** Expected: FAIL.
- [ ] **Step 3: Implement it** with the same `JsonSet`/`JsonRemove` edits and `expected_hash` as `claude::hooks`. Run `cargo test -p marshell-protocol` and `pnpm check:generated` for `hand_over`.
- [ ] **Step 4: Run Step 2's command, then `cargo test -p marshell-core -j 4 --lib claude::hooks codex::hooks`.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): plan the hooks.json entries and status line that install and remove Marshell's Antigravity hooks`.

### Task 5: Setup, plan and removal routes for Antigravity, and `marshell uninstall`

**Files:**
- Modify: `crates/core/src/server/agents.rs` (`Adapter`, setup, plan and remove routes, `agent_version`), `crates/core/src/server/mod.rs` (`with_agy_home`, beside `with_codex_hooks`)
- Modify: `crates/protocol/src/agents.rs` (`AgentSetup`)
- Modify: `crates/cli/src/uninstall.rs`
- Test: `crates/core/tests/agents_api.rs`; unit tests in `uninstall.rs`

**Interfaces:**
- Consumes: Task 4's `antigravity::hooks` functions.
- Produces: `Adapter::Antigravity`. `AgentSetup` for Antigravity has `settings_path` = `settings.json`, a new `hooks_path: Option<String>` (`hooks.json`; `None` for Claude and Codex), a new `skips_all: bool` (false for Claude and Codex), `needs_trust: false` and `trust: None`.
- Produces: `AppState::with_agy_home(user_home: PathBuf) -> Self`, the test seam for both files.
- `marshell uninstall` removes Antigravity's hooks too, and goes on past a file it can't read or plan, reporting the problem, as 6a decision 16 does for Codex.

- [ ] **Step 1: Write the failing tests.**
  - `antigravity_setup_reports_both_files`, `antigravity_setup_reports_skip_all`, `antigravity_install_then_remove_round_trips` (the temp home's files end byte-equal to where they started, or gone when they started missing), `antigravity_plan_on_an_unparsable_hooks_file_is_422`, `an_unknown_agent_is_still_404`.
  - `uninstall_removes_antigravity_hooks`, `uninstall_goes_on_past_an_unreadable_hooks_json`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --test agents_api` and `cargo test -p marshell-cli -j 4`.** Expected: FAIL.
- [ ] **Step 3: Implement it.** Then `cargo test -p marshell-protocol` and `pnpm check:generated`.
- [ ] **Step 4: Run Step 2's commands.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): set up, plan and remove Antigravity hooks through the agents routes and uninstall`.

### Task 6: Live status from agy's hooks and status line

**Files:**
- Create: `crates/core/src/agents/antigravity/status.rs`; Modify: `crates/core/src/agents/antigravity/mod.rs`
- Modify: `crates/core/src/server/hook.rs` (`post`, `fold_live`), `crates/core/src/registry.rs` (the status line's limited Needs you)
- Test: unit tests in `antigravity/status.rs` and `antigravity/mod.rs`; `crates/core/tests/agents_api.rs`; `crates/core/src/registry.rs` tests

**Interfaces:**
- Produces in `agents::antigravity`:
  - `pub const FIXTURE_VERSIONS: &[(u32, u32, u32)]`, holding Task 1's version
  - `pub fn version_gate(version: Option<&str>) -> Gate`, with name "Antigravity"
  - `status::fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus>`
  - `status::fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus`: `model` and `context_pct` (and the plan meter only if R2 says so), the state left as the hooks set it
  - `status::confirmation_pending(body: &Value) -> Option<bool>`: `tool_confirmation_pending`
- Produces in `Registry`: `pub fn agent_prompt(&self, tab: &str, pending: bool)`. `true` sets an `Attention { message: "Waiting for you in the terminal", limited: true, .. }` when the row has no hook-raised Needs permission of its own. `false`, the next hook event, or typing into the tab clears it.
- Folds (tool names and argument keys from Task 1's fixtures; these are the planned defaults):

  | Event | State, phrase |
  |---|---|
  | `PreInvocation` | Working, "Thinking" |
  | `PreToolUse` `run_command` | Working, "Running \`<CommandLine, first line, 60>\`" |
  | `PreToolUse` edit or write tool | Working, "Editing \`<file name>\`" |
  | `PreToolUse` read tool | Working, "Reading \`<file name>\`" |
  | `PreToolUse` other | Working, "Using \`<tool>\`" |
  | `PreToolUse` held for a card | NeedsPermission, "Wants to run \`<command>\`" or "Wants to use \`<tool>\`" |
  | `PostToolUse` | Working, "Thinking" after NeedsPermission, else the phrase is kept |
  | `PostInvocation` | unchanged (None) |
  | `Stop`, `terminationReason` `model_stop` | Done, "Done", no recap |
  | `Stop`, `error` or `max_steps_exceeded` | Error, the `error` text (first line, 120) or "Stopped after too many steps" |
  | anything else | None |

- `post`'s `matches!(agent, "claude" | "codex")` becomes `"claude" | "codex" | "antigravity"`. `fold_live` takes `Statusline` from Claude and Antigravity, and gates Antigravity with its gate.

- [ ] **Step 1: Write the failing tests.**
  - One test per table row, loading Task 1's fixtures with `include_str!`.
  - `stop_with_an_error_is_error`, `a_first_event_with_no_prev_folds`, `post_invocation_changes_nothing`.
  - `the_status_line_sets_model_and_context`, `the_status_line_leaves_the_state_alone`.
  - Gate: `antigravity_<version>_is_known`; `a_later_version_is_not_recorded`, asserting the exact note; `an_earlier_version_is_older`.
  - Registry: `an_agent_prompt_is_a_limited_needs_you`, `an_agent_prompt_does_not_cover_a_held_request`, `an_agent_prompt_clears_on_false_on_the_next_event_and_on_typing`.
  - `agents_api.rs`: `an_antigravity_event_folds_into_its_tab`, `an_antigravity_status_line_with_pending_raises_needs_you`, `antigravity_status_off_sets_its_note`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib antigravity:: registry::` and `cargo test -p marshell-core -j 4 --test agents_api`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's commands, then `cargo test -p marshell-core -j 4 --lib claude:: codex::`.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): fold agy's hook events and status line into live status, gated on the recorded version`.

### Task 7: Antigravity approvals

**Files:**
- Create: `crates/core/src/agents/antigravity/request.rs`, `crates/core/src/agents/antigravity/policy.rs`
- Modify: `crates/core/src/agents/mod.rs` (`supports_permissions`, `permission_view`, `permission_output`), `crates/core/src/server/hook.rs` (`hold_permission` asks the policy first), the release and timeout path that produces the hook's answer
- Test: unit tests in `request.rs` and `policy.rs`; `crates/core/tests/permissions.rs`

**Interfaces:**
- Produces in `agents::antigravity::request`:
  - `pub fn detail(request: &Value) -> PermissionDetail`: `run_command` → `Bash { command, cwd }` (`cwd` only when it differs from the workspace); the edit tool → `Edit`; the write tool → `Write`; a URL read → `Fetch`; an MCP tool → `Mcp`; anything else → `Other` with its arguments flattened by `common::flatten_input`. Key names from Task 1's fixtures.
  - `pub fn view(p: &PendingPermission) -> Option<PermissionView>`, with `risk::classify` over the detail.
  - `pub fn permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str>`: allow → `{"decision":"allow"}`; deny → `{"decision":"deny","reason":<the person's message or common::DENIED_TEXT>}`; always allow → `Err` with the decision 5 copy (unless A7 found agy keeps the rule).
  - `pub const RELEASE: &str`: `"ask"`, or `"force_ask"` if A3 found `ask` skipped under skip-all (decision 4). A release and a timeout answer `{"decision": RELEASE}`.
- Produces in `agents::antigravity::policy`: `pub fn immediate(mode: SessionMode, detail: &PermissionDetail, risky: bool, cwd: &Path) -> Option<Value>`. `Some(answer)` is returned at once with no card; `None` holds for the card. Its rules are R1's ruling. `mode` is the tab's `SessionInfo.mode`, Manual when it is absent. The planned default ports `sessionModes.ts`'s `decide` with reads always passing: a risky request → `None`; a read tool → allow; Manual and Plan → `None`; Auto-edit → allow for an in-project edit, else `None`; Full auto and Bypass → allow.
- `supports_permissions` includes `"antigravity"`.

- [ ] **Step 1: Write the failing tests.**
  - `request.rs`: one `detail` test per Task 1 `PreToolUse` fixture; `an_unknown_tool_is_other_with_its_args`; `allow_answers_allow`; `deny_carries_the_reason`; `always_allow_is_refused_with_the_copy`; `a_release_answers_the_release_decision`.
  - `policy.rs`: one test per rule, and `a_risky_command_is_held_in_bypass`.
  - `permissions.rs`: `an_antigravity_request_in_manual_waits_for_the_card`, `an_antigravity_read_is_answered_at_once`, `two_antigravity_tabs_each_get_their_own_answer`, `an_antigravity_request_from_an_unrecorded_version_still_reaches_the_card`, `an_antigravity_release_answers_ask`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib antigravity::` and `cargo test -p marshell-core -j 4 --test permissions`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's commands, then the Claude and Codex permission tests in the same file.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): answer agy's tool requests from the approve card and by the session's mode`.

### Task 8: A tab's agy conversation, for Resume and Restart (decision 2)

**Files:**
- Modify: `crates/core/src/history/recorder.rs` (`note_event`), `crates/core/src/history/mod.rs` (`sidecar_allowed`), `crates/core/src/server/sessions.rs` (`resume_session`), `crates/core/src/profiles/detect.rs` (`resume_args`)
- Test: unit tests in those files; `crates/core/tests/preset.rs` or the existing resume tests

**Interfaces:**
- `note_event("antigravity", …)` reads `hook_event_name` (from the bridge), `conversationId` as the session id, `transcriptPath`, and `workspacePaths[0]` as the folder, and links the conversation to the tab on the first event that names it. It never asks for a copy.
- `sidecar_allowed("antigravity", path, id)`: `id` is a UUID, and `path` ends in `antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl` (A8).
- `resume_session` takes `"antigravity"` as Codex's arm: a transcript that is a file → `Resume::Session(id)`; a missing one → `Resume::Fresh`, unless A8 found the file appears only later, and then `Resume::Session(id)` while the row exists.
- `resume_args`: `("antigravity", Session(id)) => ["--conversation", id]`; `Last` and `Fork` stay `["--continue"]`.
- History's `AGENTS` stays `["claude", "codex"]`: Antigravity rows serve Resume only and don't show in History (decision 2).

- [ ] **Step 1: Write the failing tests.**
  - `an_antigravity_event_links_its_conversation_to_the_tab`, `an_antigravity_path_outside_brain_is_refused`, `an_antigravity_id_that_is_not_a_uuid_is_refused`.
  - `resume_of_an_antigravity_tab_uses_its_conversation` (`--conversation <id>`), `an_antigravity_tab_with_none_on_record_continues`, `history_lists_no_antigravity_rows`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib history:: profiles::detect` and the resume tests.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's commands.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): resume a tab's own agy conversation by id`.

### Task 9: Antigravity in the Agents dialog, onboarding, rows and cards

**Files:**
- Modify: `src/lib/api/agents.ts` (`HookAgent`), `src/features/agents/AgentsDialog.tsx`, `src/features/agents/agentsState.ts`, `src/features/onboarding/LiveOnboarding.tsx`, `src/features/onboarding/consent.ts` (entry counts from Task 4), `src/features/onboarding/ConsentCard.tsx` (the hand-over switch sends `hand_over`), `src/features/approval/fromCore.ts`, `src/features/sessions/toRow.ts`, `src/features/modes/sessionModes.ts` (skip-all from `AgentSetup.skips_all`)
- Test: the matching `*.test.ts(x)` files

**Interfaces:**
- Consumes: `AgentSetup.hooks_path` and `skips_all` (Task 5), `HookPlanRequest.hand_over` (Task 4), `Attention.limited` (Task 6).
- The Agents dialog's Antigravity section names both files, offers status, approve and status line, and shows the hand-over switch only when `skips_all` is true. Its removal copy is the Global constraints copy. If Task 1 confirms other Antigravity apps read the same `hooks.json`, the consent card's source line says so, in words from the R1/R2 question round.

- [ ] **Step 1: Write the failing tests.**
  - `the_agents_dialog_lists_antigravity_with_both_files`, `the_hand_over_switch_shows_only_with_skip_all`, `the_hand_over_switch_sends_hand_over`, `antigravity_removal_copy_is_exact`.
  - `onboarding_offers_antigravity_when_agy_is_found`, `antigravity_entry_counts_match_the_plan`.
  - `an_antigravity_card_shows_its_command`, `an_antigravity_row_with_skip_all_shows_bypass_and_why`, `an_antigravity_row_with_a_limited_needs_you_says_status_limited`.
  - Each new control is reached by Tab, and its forced-colours rule exists (`rules.test.tsx` passes).
- [ ] **Step 2: Run `pnpm test src/features/agents src/features/onboarding src/features/approval src/features/sessions src/features/modes`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's command, then `pnpm test` and `pnpm build`.** Expected: PASS.
- [ ] **Step 5: Commit** as `feat(antigravity): show Antigravity in the Agents dialog, onboarding, rows and cards`.

### Task 10: Live check and the slice's exit (main thread)

**Files:**
- Create: `docs/research/2026-10-26-antigravity-adapter-live-check.md`

- [ ] **Step 1: Start the check** with a fresh literal scratch folder and `-AgyAuth`. Install the Antigravity hooks from the Agents dialog, with the hand-over switch on (the scratch `settings.json` has `dangerouslySkipPermissions: true` written in by hand first). Check the preview names both files and that the real `~/.gemini` is unchanged.
- [ ] **Step 2: Drive it.** In an agy tab in Manual: a read passes, a shell command raises a card, Allow runs it, Deny stops it with the reason, Answer in terminal brings up agy's own prompt, and the status line's pending raises a limited Needs you there. In Auto-edit, an in-project edit passes. A risky command gets a card in Bypass. Two agy tabs at once each answer their own card. Restart a tab and check it resumes the same conversation (`--conversation <id>`). Run agy in a plain terminal outside Marshell with the hooks installed and check nothing changes for it (Review focus 1).
- [ ] **Step 3: Turn the hooks off** and check `hooks.json` is deleted, `settings.json` has `dangerouslySkipPermissions` and the person's own status line back, and Undo restores the install.
- [ ] **Step 4: Write the live-check note** with what passed and failed and the agy version. Stop the check by PID, after listing the tree, and remove the scratch folder after showing its path and getting an OK. Commit as `docs(research): record the Antigravity adapter live check`.
- [ ] **Step 5: Run what CI runs**: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`. Quote the counts. Then the final review on Opus (`marshell-reviewer`, with this plan's Review focus) and one fix pass, then `marshell-ship`.
