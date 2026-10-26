# Slice 6d: the Antigravity CLI adapter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Antigravity CLI (`agy`) gets what Claude Code and Codex have: hooks and a chained status line installed and removed through the config service, live status in its row, and its tool requests answered from the approve card, Departures and banners. Every piece is checked against a real agy first.

**Architecture:** A new `crates/core/src/agents/antigravity/` module, shaped like `agents/codex/`: `hooks.rs` (the plan for `~/.gemini/config/hooks.json` and the `statusLine` in `~/.gemini/antigravity-cli/settings.json`), `status.rs` (hook events and the status line folded into `LiveStatus`), `request.rs` (the card's view of a `PreToolUse`, and agy's answer JSON) and `policy.rs` (what the hook answers at once, without a card). agy's payloads carry no event name, so the bridge takes `--event <Name>` and writes `hook_event_name` into the body; the core's routes, fold, history and holds stay agent-neutral and gain an `"antigravity"` arm where they match `"codex"` today. The config service gains a `DeleteFile` edit (decision 6).

**Tech stack:** Rust (axum, serde_json, the `config` service and its JSONC edits), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` "Corrections to the brief" item 1 (Antigravity), "Session modes", "Config safety copy", and §9 "Slice 6d decisions (2026-10-26)". Evidence: `docs/research/2026-10-15-local-install-check.md` (Antigravity) and the bundled `agy-customizations/docs/hooks.md`; Task 1 writes `docs/research/2026-10-26-antigravity-hooks.md`.

**Checked before planning (2026-10-26):**
- `agy --version` prints `1.4.0`; the bundled changelog runs to 1.4.7. Task 1 records the version that runs. *(It auto-updated: Task 1 ran 1.4.7.)*
- agy has no home-override variable (a scan of the binary found none). With `USERPROFILE` set to a scratch folder, `agy mcp list` read `<scratch>\.gemini\config\mcp_config.json`, and nothing under the real `~/.gemini` changed.
- The two copied files (`oauth_creds.json`, `google_accounts.json`) sign a scratch agy in: `agy models` listed the models.
- The core's `server::files::home_dir()` reads `USERPROFILE` too, so an app started with a scratch `USERPROFILE` plans agy's files in the same scratch home the agy tabs use.
- Baseline on `origin/main` (6b305ab): `pnpm test` 338 files, 5034 tests pass. `cargo test --workspace -j 4`: 1148 pass, 2 ignored, 1 fails under the full parallel run only: `history::storage::backup::tests::an_install_id_that_cannot_be_read_is_not_replaced`, which passes alone (3 of 3). It predates 6d and isn't touched here.

## Global constraints

- Marshell writes only `~/.gemini/config/hooks.json` and `~/.gemini/antigravity-cli/settings.json` (its `statusLine` only; no hand-over, decision 9), plus its own `~/.marshell/agents/antigravity/statusline-chain.json`. It never writes a workspace `.agents/hooks.json`, a plugin's `hooks.json`, `config.json`, or anything else under `~/.gemini`.
- Marshell's entries live under one named hook, `"marshell"`, in `hooks.json`. The person's own named hooks are left byte-equal on install and removal.
- Tests and live checks never read or write the real `~/.gemini`. Tests inject paths (`AppState::with_agy_home(<temp>)`). The live check starts the app with a scratch `USERPROFILE` (`live-check.ps1 -AgyAuth`), which copies `~/.gemini/oauth_creds.json` and `google_accounts.json` into the scratch home and reads nothing else there; `stop` deletes them with the scratch folder (decision 1).
- No new dependency.
- agy's neutral `PreToolUse` answer is zero bytes on stdout with exit 0 (A3). `{}` is a deny, and a non-zero exit, invalid JSON, a timeout or a command that can't start all block the tool. So every bridge path that has no answer from the core prints nothing and exits 0, in every role, for agent `antigravity`.
- The core never persists the status line's `email` or a hook body's `lastUserInput` (personal data): not in live status, the event ring, history rows or logs.
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
  - Always allow refused (decision 7, replacing decision 5's copy): "Antigravity can't keep an always-allow rule from Marshell. Allow this once instead."
  - Removal copy (PLAN, "Config safety copy"): "Marshell removes the hook entries it added, deletes hooks.json if nothing else is in it, and puts your own status line back from statusline-chain.json; the rest of each file stays as it is".
  - A bridge path agy's shell can't run (A2): "Antigravity can't run Marshell's hook from <path>: the path holds a % or a quote." and "Antigravity can't run Marshell's hook from <path>: the path has no drive letter."
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

- **R1: what gets a card.** `PreToolUse` fires before every tool, not only when agy would ask, so holding each one for a card would ask about every file read. The options depend on A3. The planned default answers at once by the tab's mode, in the core: reads pass; Manual asks about every edit and command; Auto-edit lets in-project edits pass; Full auto and Bypass ask only about the risky list. "Pass" means the neutral answer (Task 2), so agy's own mode and allow list decide, as they would with no hook. Launches keep the native flags the core's `mode_args` gives today (`--mode accept-edits`, `--dangerously-skip-permissions` and the rest), not PLAN's strict start, because a pass hands the call back to that mode. So in Bypass the risky-list card exists only if A3 shows `PreToolUse` fires under `--dangerously-skip-permissions`, and the question says so. An alternative is a matcher on Marshell's blocking entry, so read tools never reach it.
- **R2: the plan meter.** agy's `quota` is one bucket per model with `remaining_fraction` and `reset_time`, and its window isn't known to be five hours. The row and the footer label the meter "5h". The planned default leaves Antigravity's meter out in 6d (model and context % only) unless Task 1 shows a five-hour window.

**Answered on 2026-10-26** (PLAN §9, slice 6d decisions 7 to 9; evidence in `docs/research/2026-10-26-antigravity-hooks.md`):

- **R1: cards in Bypass only.** `allow` never skips agy's own prompt, so the core can't approve anything. agy starts with `mode_args`' native flags. In Manual, Plan, Auto-edit and Full auto the core passes every call (204, so the bridge prints nothing) and agy asks in its terminal; the status line's `tool_confirmation_pending` raises the limited Needs you. In Bypass (`--dangerously-skip-permissions`, where empty, `ask` and `force_ask` all run the call) a risky-list call is held for a card: Allow → pass (it runs), Deny → `deny`, release or timeout → `deny`.
- **R2: the plan meter is left out.** `quota` has two weekly buckets (`3p-weekly`, `gemini-weekly`), no five-hour window.
- **Hand-over dropped.** agy 1.4.7 ignores `dangerouslySkipPermissions` in `settings.json`. No `hand_over`, no `skips_all`, no switch.

## Review focus

1. **agy run outside a Marshell tab with the hooks installed** (Windows Terminal, Marshell closed, or another Antigravity app reading the shared `hooks.json`). The bridge's answer must leave agy exactly as it would be with no hook: it never allows and never forces a prompt agy wouldn't show. The answer is the one A3 shows to be neutral. Tests in Task 2.
2. **A person's own `hooks.json` and `settings.json`** (BOM, CRLF, their own named hook, their own `statusLine`). Their bytes stay as they were, apart from Marshell's entries, on install and on removal, and `hooks.json` is deleted only when nothing else is in it. Tests in Tasks 4 and 5.
3. **A Bypass tab whose card is released, times out, or whose core goes away.** A release and the core's timeout answer `deny`, because under `--dangerously-skip-permissions` an empty answer, `ask` and `force_ask` all run the call. The core's hold deadline ends before the bridge's 590 s, so the core's `deny` arrives first. Tests in Task 7.
4. **Two agy tabs asking at once.** Each card answers its own request and never the other tab's. Test in Task 7.
5. **agy updated to a version Marshell hasn't recorded.** The row gets the "not one Marshell has recorded yet" note instead of guessed status. Approvals still reach the card, and an unknown tool shows as "other" with its raw arguments. Tests in Tasks 6 and 7.
6. **The bridge missing or moved while the hooks stay installed** (an uninstall that stopped halfway, a Marshell home moved). agy blocks every tool call when the `PreToolUse` command can't start (A3), so the person's agy stops working outside Marshell too. The Agents dialog must show such entries as orphaned (`HookInstall::Orphaned`, as for Claude and Codex) with a way to remove them, and `marshell uninstall` removes them even with the app closed. Tests in Tasks 4 and 5.

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
- Produces in `permission.rs`: `pub fn neutral(agent: &str) -> &'static [u8]`: what the bridge prints for a `permission` call it can't take to a card (no tab, no core, the core's 204, the core's timeout, an answer that fails `accepted`). Claude and Codex: empty, as today. Antigravity: empty too (A3: zero bytes and exit 0 is agy's neutral answer; `{}` would deny). The function stays so the neutral bytes live in one place.
- Every Antigravity path without an answer from the core (no `MARSHELL_TAB_ID`, no endpoint, connection refused, a 5xx, the bridge's own 590 s deadline, an answer that fails `accepted`) prints nothing and exits 0, in every role. The `event` role prints nothing, as it does for Claude and Codex.
- Produces: `permission::accepted(answer, request)` takes agy's shape for agent `antigravity`: an object whose `decision` is `allow`, `deny`, `ask` or `force_ask`, with an optional string `reason` and nothing else. The core's answer is printed as it is. A Claude- or Codex-shaped answer, or `{}`, on agent `antigravity` fails, and so falls back to `neutral`. (After decision 7 the core only ever sends `deny`; the other values are accepted so the bridge needn't change if that does.)

- [ ] **Step 1: Write the failing tests.**
  - `permission.rs`: `an_antigravity_allow_is_printed_as_it_is`, `an_antigravity_deny_with_a_reason_is_printed_as_it_is`, `a_codex_shaped_answer_on_antigravity_is_rejected`, `an_antigravity_answer_with_an_overwrite_is_rejected`, `an_empty_object_on_antigravity_is_rejected`, `the_antigravity_neutral_answer_is_empty`.
  - `args.rs`: `parses_an_event`, `an_event_needs_a_value`, `an_event_that_is_not_an_identifier_is_an_error`, `a_second_event_is_an_error`.
  - `lib.rs`: `the_event_is_written_into_the_body` (body `{"conversationId":"c"}` with `--event Stop` posts `hook_event_name: "Stop"`), `an_existing_event_name_is_kept`, `a_body_that_is_not_an_object_is_posted_as_it_is`.
  - `permission.rs` or `lib.rs`: `antigravity_outside_a_tab_prints_the_neutral_answer` (no `MARSHELL_TAB_ID`), `antigravity_with_no_core_prints_the_neutral_answer` (no endpoint), `antigravity_with_a_server_error_prints_nothing` (a 500 from a test server), `an_antigravity_event_prints_nothing`, `claude_and_codex_outside_a_tab_print_nothing`. Each asserts empty stdout and exit code 0.
- [ ] **Step 2: Run `cargo test -p marshell-hook -j 4`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's command.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(hook): take agy's event name from --event and answer neutrally outside a tab`.

### Task 3: The config service's `DeleteFile` edit (decision 6)

**Files:**
- Modify: `crates/protocol/src/config.rs` (`EditOp`), `crates/core/src/config/preview.rs`, `crates/core/src/config/service.rs`, `crates/core/src/config/write.rs` if needed
- Modify: `src/generated/EditOp.ts` (generated); `src/features/onboarding/ConsentCard.tsx`
- Test: unit tests in `preview.rs` and `service.rs`; `src/features/onboarding/ConsentCard.test.tsx`

**Interfaces:**
- Produces: `EditOp::DeleteFile` (no fields), applied to a file whose hash must equal `expected_hash`, which is required for this op. Preview: a unified diff from the file to nothing, with `after_hash: None` and `FileStatus::Changes`. A missing file is `NoChange`. Apply: the snapshot is taken first, then the file is removed with `write.rs`'s retrying remove. Undo restores the snapshot's bytes. The service's guard applies as to every op (managed, outside the allowed roots, Marshell's own, read-only and too large are refused with their existing reasons).
- Produces: `Change::Deleted { path }` beside `Change::Created`, so undo knows to restore rather than remove.
- The consent card names a file whose preview has `after_hash: None` and status `Changes` as "<file name> will be deleted" above its diff.

- [ ] **Step 1: Write the failing tests.**
  - `delete_previews_every_line_removed`, `delete_of_a_missing_file_is_no_change`, `delete_without_an_expected_hash_is_refused` (`BadValue`), `delete_with_a_changed_file_is_a_conflict`, `delete_takes_a_snapshot_and_removes_the_file`, `undo_of_a_delete_puts_the_bytes_back` (BOM and CRLF content, byte-equal), `delete_of_a_managed_file_is_refused`.
  - `ConsentCard.test.tsx`: `a_deleted_file_is_named_as_deleted` (not as unchanged or created).
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
  - `pub const STATUS_EVENTS: [&str; 4] = ["PostToolUse", "PreInvocation", "PostInvocation", "Stop"]`; `PreToolUse` is the approve entry (role `permission`, timeout 600 s; the bridge waits 590 s). With approve off, `PreToolUse` gets a status entry (role `event`, timeout 5 s) so the row can say what's running. Routine timeouts are 5 s. The `PreToolUse` and `PostToolUse` groups use `"matcher": ".*"`. (Task 1: `SessionStart` fires only for a new conversation, and `PreInvocation` follows it at once, so it isn't installed.)
  - `pub fn hooks_path(user_home: &Path) -> PathBuf` = `<user_home>/.gemini/config/hooks.json`; `pub fn settings_path(user_home: &Path) -> PathBuf` = `<user_home>/.gemini/antigravity-cli/settings.json`
  - `pub fn hook_command(home: &Path, role: &str, event: &str) -> Result<String, PlanError>`: on Windows, `<drive>:"<bridge folder, forward slashes, no drive>"/marshell-hook.exe <role> --agent antigravity --event <event> --tag mshl1`, for example `C:"/Users/Home Office/.marshell/bin"/marshell-hook.exe permission --agent antigravity --event PreToolUse --tag mshl1`. Task 1 (A2) found agy runs `cmd /c "<command>"` with every inner `"` escaped to `\"`, so a command that starts with a quote fails, and this form is the one that ran with spaces and `& ( ) ; , = ^` in the path. It returns `PlanError::BridgePath { path, reason }` when the bridge path holds `%` or `"` (cmd expands `%` inside quotes) or has no drive letter (a UNC path), with the Global constraints copy. Elsewhere, the bridge path in single quotes (`common`'s POSIX quoting; agy's Unix shell is unverified).
  - `pub fn statusline_command(home: &Path) -> Result<String, PlanError>`: `statusline --agent antigravity --tag mshl1` after the bridge, in the same form
  - New `PlanError::BridgePath { agent: &'static str, path: PathBuf, reason: &'static str }` in `common.rs`, displayed as "<agent> can't run Marshell's hook from <path>: <reason>" (agent "Antigravity"); the plan route turns it into a 422 like `DoesNotParse`.
  - `pub struct Found { pub install: HookInstall, pub approve_installed: bool, pub has_own_statusline: bool, pub chained: bool }`
  - `pub fn inspect(hooks: Option<&Value>, settings: Option<&Value>, chain: Option<&Value>, home: &Path) -> Found`
  - `pub fn install_plan(user_home: &Path, hooks_text: Option<&str>, settings_text: Option<&str>, chain_text: Option<&str>, home: &Path, want: &HookPlanRequest) -> Result<ChangeSet, PlanError>`, titled "Set up Antigravity hooks"
  - `pub fn remove_plan(user_home: &Path, hooks_text: Option<&str>, settings_text: Option<&str>, chain_text: Option<&str>, home: &Path) -> Result<ChangeSet, PlanError>`, titled "Turn Antigravity hooks off"
- Rules:
  - Install sets `/marshell` to `{ "enabled": true, <events> }`, creating `hooks.json` when it's missing. Removal removes `/marshell`, and uses `DeleteFile` when no other key would be left.
  - The status line follows Claude's chain: the person's own `statusLine` object is kept whole in the chain file and put back on removal. Marshell's `statusLine` keeps the person's `padding` and `stack_with_default`; with no line of their own, it sets `"stack_with_default": true`, which keeps agy's own line showing under the bridge's empty output (A6: without it, empty output blanks agy's line). Removal with no line of their own removes the whole `statusLine`. A `statusLine` the person changed since the install is left alone, as Claude's is.
  - The bridge's `statusline` role for agent `antigravity` never fails: with no line of its own to run it prints nothing and exits 0 (a failing command shows "⚠ Statusline Error" in agy). Covered by a test in `crates/hook/src/statusline.rs`.
  - No hand-over (decision 9): the plan never touches `dangerouslySkipPermissions`.
  - Scope `ConfigScope::User` and `Applies::NewSessions`, for both files.

- [ ] **Step 1: Write the failing tests.**
  - `install_into_missing_files_adds_the_hook_and_the_line`: one `/marshell` hook with each status event and the approve `PreToolUse`, every command carrying `--agent antigravity --event <Name> --tag mshl1`, `PreToolUse`'s with role `permission` and timeout 600, and a `CreateFile` for `hooks.json`.
  - `status_only_puts_an_event_entry_on_pre_tool_use`.
  - `install_keeps_the_users_own_named_hook` (byte-equal around ours).
  - `install_on_an_installed_file_changes_nothing`, and `inspect` reports `Installed` with `approve_installed`.
  - `an_entry_from_another_home_is_orphaned`.
  - `remove_with_only_ours_deletes_hooks_json`; `remove_with_a_users_hook_keeps_the_file` (their bytes equal, over the BOM, CRLF and comments texts).
  - `the_users_status_line_is_chained_and_put_back`; `a_status_line_changed_since_the_install_is_left_alone`.
  - `with_no_line_of_their_own_the_line_stacks_with_agys`; `removal_with_no_line_of_their_own_removes_the_line`; `skip_all_in_settings_is_left_alone` (`dangerouslySkipPermissions: true` byte-equal through install and removal).
  - `the_command_quotes_after_the_drive` (a home under `C:\Users\Home Office`, asserting the exact string above); `a_home_with_a_percent_is_refused`; `a_home_with_a_quote_is_refused`; `a_unc_home_is_refused` (each `PlanError::BridgePath` with its exact copy).
  - `an_unparsable_file_is_refused_with_its_line` (`PlanError::DoesNotParse`, for each file).
  - In `crates/hook/src/statusline.rs`: `antigravity_with_no_line_of_its_own_prints_nothing`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib antigravity::hooks` and `cargo test -p marshell-hook -j 4 statusline`.** Expected: FAIL.
- [ ] **Step 3: Implement it** with the same `JsonSet`/`JsonRemove` edits and `expected_hash` as `claude::hooks`.
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
- Produces: `Adapter::Antigravity`. `AgentSetup` for Antigravity has `settings_path` = `settings.json`, a new `hooks_path: Option<String>` (`hooks.json`; `None` for Claude and Codex), `needs_trust: false` and `trust: None`. (No `skips_all`: decision 9.)
- Produces: `AppState::with_agy_home(user_home: PathBuf) -> Self`, the test seam for both files.
- `marshell uninstall` removes Antigravity's hooks too, and goes on past a file it can't read or plan, reporting the problem, as 6a decision 16 does for Codex.

- [ ] **Step 1: Write the failing tests.**
  - `antigravity_setup_reports_both_files`, `antigravity_install_then_remove_round_trips` (the temp home's files end byte-equal to where they started, or gone when they started missing), `antigravity_plan_on_an_unparsable_hooks_file_is_422`, `antigravity_plan_with_a_bridge_path_agy_cant_run_is_422` (a `MARSHELL_HOME` holding `%`, with the copy), `antigravity_entries_from_another_home_are_orphaned` (Review focus 6), `an_unknown_agent_is_still_404`.
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
  - `pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(1, 4, 7)]`
  - `pub fn version_gate(version: Option<&str>) -> Gate`, with name "Antigravity"
  - `status::fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64, held: bool) -> Option<LiveStatus>`. `held` says the request waits for a card. In this task the caller passes `false` (no card exists yet); Task 7 passes the policy's outcome.
  - `status::fold_statusline(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> LiveStatus`: `model` (`model.display_name`, else `model.id`) and `context_pct` (`context_window.used_percentage`), no plan meter (decision 8), the state left as the hooks set it. `agent_state`, `cycle_mode`, `quota`, `email` and the rest are read for nothing else; `email` is never copied anywhere.
  - `status::confirmation_pending(body: &Value) -> Option<bool>`: `Some(true)` when `tool_confirmation_pending` is `true`; `Some(false)` when the key is absent or false (agy drops the key when no prompt shows); `None` for a body that isn't an object. Task 1 measured agy runs the status line only on a change (every 0.2 to 0.5 s while working, never on a timer), so the flag clears on the next run after the prompt is answered.
- Produces in `Registry`: `pub fn agent_prompt(&self, tab: &str, pending: bool)`. `true` sets an `Attention { message: "Waiting for you in the terminal", limited: true, .. }` when the row has no hook-raised Needs permission of its own. `false`, the next hook event, or typing into the tab clears it.
- Folds (tool names and argument keys from Task 1's 1.4.7 fixtures; agy's body has no event name, so the fold reads the `hook_event_name` the bridge adds):

  | Event | State, phrase |
  |---|---|
  | `PreInvocation` | Working, "Thinking" |
  | `PreToolUse` `run_command` | Working, "Running \`<toolCall.args.CommandLine, first line, 60>\`" |
  | `PreToolUse` `write_file` or `edit_file` | Working, "Editing \`<file name of toolCall.args.Path>\`" |
  | `PreToolUse` `read_file` | Working, "Reading \`<file name of toolCall.args.Path>\`" |
  | `PreToolUse` other | Working, "Using \`<toolCall.name>\`" |
  | `PreToolUse` with `held` | NeedsPermission, "Wants to run \`<command>\`" or "Wants to use \`<tool>\`" |
  | `PostToolUse` | Working, "Thinking" after NeedsPermission, else the phrase is kept |
  | `PostInvocation` | unchanged (None) |
  | `Stop` with `error` empty or absent | Done, "Done", no recap (Task 1 saw `terminationReason: "NO_TOOL_CALL"`, `fullyIdle: true`) |
  | `Stop` with a non-empty `error` | Error, the `error` text (first line, 120) |
  | anything else (`SessionStart` included) | None |

  No fold reads `lastUserInput`, and nothing from it reaches `LiveStatus`.

- `post`'s `matches!(agent, "claude" | "codex")` becomes `"claude" | "codex" | "antigravity"`. `fold_live` takes `Statusline` from Claude and Antigravity, and gates Antigravity with its gate.

- [ ] **Step 1: Write the failing tests.**
  - One test per table row, loading Task 1's fixtures with `include_str!`.
  - `stop_with_an_error_is_error` (the Stop fixture with `error` set by the test), `a_first_event_with_no_prev_folds`, `post_invocation_changes_nothing`, `no_fold_carries_the_last_user_input`.
  - `the_status_line_sets_model_and_context` (from `statusline/1.4.7/working.json`), `the_status_line_leaves_the_state_alone`, `the_status_line_sets_no_plan_meter`, `pending_is_read_from_pending_json`, `a_missing_pending_key_is_false` (`idle.json`).
  - Gate: `antigravity_1_4_7_is_known`; `a_later_version_is_not_recorded`, asserting the exact note; `an_earlier_version_is_older`.
  - Registry: `an_agent_prompt_is_a_limited_needs_you`, `an_agent_prompt_does_not_cover_a_held_request`, `an_agent_prompt_clears_on_false_on_the_next_event_and_on_typing`.
  - `agents_api.rs`: `an_antigravity_event_folds_into_its_tab`, `an_antigravity_status_line_with_pending_raises_needs_you`, `antigravity_status_off_sets_its_note`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib antigravity:: registry::` and `cargo test -p marshell-core -j 4 --test agents_api`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's commands, then `cargo test -p marshell-core -j 4 --lib claude:: codex::`.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): fold agy's hook events and status line into live status, gated on the recorded version`.

### Task 7: Antigravity approvals

**Files:**
- Create: `crates/core/src/agents/antigravity/request.rs`, `crates/core/src/agents/antigravity/policy.rs`
- Modify: `crates/core/src/agents/mod.rs` (`supports_permissions`, `permission_view`, `permission_output`), `crates/core/src/server/hook.rs` (`post`), the release and timeout path that produces the hook's answer
- Test: unit tests in `request.rs`, `policy.rs` and `profiles/detect.rs`; `crates/core/tests/permissions.rs`

**Interfaces:**
- Produces in `agents::antigravity::request`:
  - `pub fn detail(request: &Value) -> PermissionDetail`, from `toolCall{name,args}` (1.4.7 names and keys): `run_command` → `Bash { command: args.CommandLine, cwd }` (`cwd` only when it differs from the tab's); `edit_file` → `Edit` (`Path`, `Target`, `Replacement`); `write_file` → `Write` (`Path`, `Content`); an `mcp_`-prefixed tool → `Mcp`; anything else (`read_file` included) → `Other` with its arguments flattened by `common::flatten_input`. Task 1 saw no URL tool (the agent fetched with `curl.exe` through `run_command`), so there is no `Fetch` arm.
  - `pub fn view(p: &PendingPermission) -> Option<PermissionView>`, with `risk::classify` over the detail.
  - `pub fn permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str>` (the shared signature): allow → `{"decision":"ask"}`, which Task 1 saw run the call with no prompt under `--dangerously-skip-permissions` (a bare `allow` was never tried under the flag, and only Bypass holds); deny → `{"decision":"deny","reason":<the person's message or common::DENIED_TEXT>}`; always allow → `Err` with the decision 7 copy (A7: `permissionOverrides` grants nothing).
  - `pub fn release_output() -> Value`: `{"decision":"deny","reason":common::DENIED_TEXT}`. A release and the core's hold timeout both answer it (decision 7; only Bypass holds, and under its flag an empty answer, `ask` and `force_ask` all run the call). The default hold (120 s, `MARSHELL_PERMISSION_HOLD_SECS` clamped to 590) ends before the bridge's 590 s deadline.
- Produces in `agents`: `pub fn release_output(agent: &str, request: &Value) -> Option<Value>`: `None` for Claude and Codex (204, as today), `Some(antigravity::request::release_output())` for Antigravity. `server::hook::hold_permission` answers 200 with it for `Outcome::Released` and the timeout, instead of the 204.
- Produces in `agents::antigravity::policy`: `pub fn holds(mode: SessionMode, risky: bool) -> bool`: `true` only for `Bypass` with a risky request (decision 7). Every other call passes at once (204), and agy's own mode decides, asking in its terminal where it would. `mode` is the tab's `SessionInfo.mode`, Manual when it is absent.
- For an Antigravity `permission` post, `post` runs in this order: the tab must be open and its effective agent `Some("antigravity")`, or the core answers 204 at once (an Antigravity IDE started from a plain shell tab inherits `MARSHELL_TAB_ID` and reads the same `hooks.json`). Then `policy::holds`. Then `fold_live` with `held`. Then, when held, `hold_permission`. A pass folds as Working and raises no banner, sound or Departures entry.
- `supports_permissions` includes `"antigravity"`.
- Launches keep `mode_args`' native flags (Manual none, Plan `--mode plan`, Auto-edit `--mode accept-edits`, Full auto `--mode auto`, Bypass `--dangerously-skip-permissions`) with the hooks installed; the strict start of PLAN's hybrid doesn't apply to agy.

- [ ] **Step 1: Write the failing tests.**
  - `request.rs`: one `detail` test per 1.4.7 `PreToolUse` fixture (`run_command`, `write_file`, `edit_file`, `read_file`, `read_file-outside`); `an_unknown_tool_is_other_with_its_args`; `allow_answers_ask`; `deny_carries_the_reason`; `always_allow_is_refused_with_the_copy`; `a_release_denies`.
  - `policy.rs`: `a_risky_command_is_held_in_bypass`, `a_safe_command_passes_in_bypass`, and `nothing_is_held_outside_bypass` (Manual, Plan, Auto-edit, Full auto, each with a risky and a safe request).
  - `permissions.rs`: `an_antigravity_risky_request_in_bypass_waits_for_the_card`, `an_antigravity_request_in_manual_passes_at_once`, `two_antigravity_tabs_each_get_their_own_answer` (both Bypass, both risky), `an_antigravity_request_from_an_unrecorded_version_still_reaches_the_card`, `an_antigravity_release_denies`, `an_antigravity_hold_that_times_out_denies` (a 1 s hold), `allow_on_an_antigravity_card_answers_ask`, `a_claude_release_still_answers_204`, `a_pass_never_shows_needs_permission`, `a_pass_raises_no_banner`, `an_antigravity_request_from_a_shell_tab_passes`.
  - `profiles/detect.rs`: `antigravity_launches_keep_their_native_mode_flags` (Manual none, Auto-edit `--mode accept-edits`, Bypass `--dangerously-skip-permissions`, with the hooks installed).
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib antigravity::` and `cargo test -p marshell-core -j 4 --test permissions`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's commands, then the Claude and Codex permission tests in the same file.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): answer agy's tool requests from the approve card and by the session's mode`.

### Task 8: A tab's agy conversation, for Resume and Restart (decision 2)

**Files:**
- Modify: `crates/core/src/history/recorder.rs` (`note_event`), `crates/core/src/history/mod.rs` (`sidecar_allowed`), `crates/core/src/server/sessions.rs` (`resume_session`), `crates/core/src/profiles/detect.rs` (`resume_args`)
- Test: unit tests in those files; `crates/core/tests/preset.rs` or the existing resume tests

**Interfaces:**
- `note_event("antigravity", …)` reads `hook_event_name` (from the bridge), `conversationId` as the session id and `transcriptPath`, takes the folder from the tab's cwd (Task 1: `workspacePaths` is always `[]` in 1.4.7), and links the conversation to the tab on any event that names it, so a later `conversationId` in the same tab replaces the earlier one. It never reads `lastUserInput` and never asks for a copy.
- `sidecar_allowed("antigravity", path, id)`: `id` is a UUID, and `path` ends in `antigravity-cli/brain/<id>/.system_generated/logs/transcript_full.jsonl`, the hook's `transcriptPath` (A8; the status line's `transcript_path` names a folder that doesn't exist, so it is never used).
- `resume_session` takes `"antigravity"` as Codex's arm: a transcript that is a file → `Resume::Session(id)`; a missing one → `Resume::Fresh` (A8: the file exists from the first prompt).
- `resume_args`: `("antigravity", Session(id)) => ["--conversation", id]`; `Last` and `Fork` stay `["--continue"]`.
- History's `AGENTS` stays `["claude", "codex"]`: Antigravity rows serve Resume only and don't show in History (decision 2).

- [ ] **Step 1: Write the failing tests.**
  - `an_antigravity_event_links_its_conversation_to_the_tab` (the 1.4.7 `PreInvocation` fixture), `a_later_conversation_in_the_tab_replaces_the_earlier`, `an_antigravity_path_outside_brain_is_refused`, `an_antigravity_id_that_is_not_a_uuid_is_refused`, `the_last_user_input_is_not_kept` (no stored row or file holds the fixture's `lastUserInput` text).
  - `resume_of_an_antigravity_tab_uses_its_conversation` (`--conversation <id>`), `an_antigravity_tab_with_none_on_record_continues`, `history_lists_no_antigravity_rows`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib history:: profiles::detect` and the resume tests.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's commands.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(antigravity): resume a tab's own agy conversation by id`.

### Task 9: Antigravity in the Agents dialog, onboarding, rows and cards

**Files:**
- Modify: `src/lib/api/agents.ts` (`HookAgent`), `src/features/agents/AgentsDialog.tsx`, `src/features/agents/agentsState.ts`, `src/features/onboarding/LiveOnboarding.tsx`, `src/features/onboarding/consent.ts` (entry counts from Task 4), `src/features/approval/fromCore.ts`, `src/features/approval/ApproveCard.tsx` or `LiveCard.tsx` (no Answer in terminal for Antigravity), `src/features/sessions/toRow.ts`
- Test: the matching `*.test.ts(x)` files

**Interfaces:**
- Consumes: `AgentSetup.hooks_path` (Task 5), `Attention.limited` (Task 6), `PermissionView.agent`.
- The Agents dialog's Antigravity section names both files and offers status, approve and status line. There is no hand-over switch (decision 9). Its removal copy is the Global constraints copy. Its approve toggle says what it does for agy: "Cards for risky commands in Bypass. In other modes Antigravity asks in its terminal." A plan refused with `PlanError::BridgePath` shows its message as the dialog's error.
- An Antigravity card offers Allow and Deny only: no Answer in terminal (agy in Bypass has no prompt to hand over to, so a release denies) and no Always allow. A card that runs out denies, and the card's countdown copy says "Denies in <n> s".

- [ ] **Step 1: Write the failing tests.**
  - `the_agents_dialog_lists_antigravity_with_both_files`, `the_antigravity_approve_toggle_says_bypass_only`, `antigravity_removal_copy_is_exact`, `a_bridge_path_refusal_shows_its_message`.
  - `onboarding_offers_antigravity_when_agy_is_found`, `antigravity_entry_counts_match_the_plan`.
  - `an_antigravity_card_shows_its_command`, `an_antigravity_card_offers_allow_and_deny_only`, `an_antigravity_card_says_it_denies_when_it_runs_out`, `an_antigravity_row_with_a_limited_needs_you_says_status_limited`, `an_antigravity_row_shows_no_plan_meter`.
  - Each new control is reached by Tab, and its forced-colours rule exists (`rules.test.tsx` passes).
- [ ] **Step 2: Run `pnpm test src/features/agents src/features/onboarding src/features/approval src/features/sessions`.** Expected: FAIL.
- [ ] **Step 3: Implement it.**
- [ ] **Step 4: Run Step 2's command, then `pnpm test` and `pnpm build`.** Expected: PASS.
- [ ] **Step 5: Commit** as `feat(antigravity): show Antigravity in the Agents dialog, onboarding, rows and cards`.

### Task 10: Live check and the slice's exit (main thread)

**Files:**
- Create: `docs/research/2026-10-26-antigravity-adapter-live-check.md`

- [ ] **Step 1: Start the check** with a fresh literal scratch folder and `-AgyAuth`. Write a status line of the person's own and `dangerouslySkipPermissions: true` into the scratch `settings.json` by hand first. Install the Antigravity hooks from the Agents dialog. Check the preview names both files, that the hook command has the quote-after-drive form, and that the real `~/.gemini` is unchanged.
- [ ] **Step 2: Drive it.** In an agy tab in Manual: a read runs, a shell command brings up agy's own prompt with no card, and the row shows the limited Needs you ("Waiting for you in the terminal") until it is answered. In Auto-edit, an in-project edit runs. In Bypass: a safe command runs with no card; a risky one gets a card with Allow and Deny only; Allow runs it, Deny stops it with the reason, and a card left to run out denies. Two Bypass agy tabs at once each answer their own card. The row shows model and context %, and no plan meter. Restart a tab and check it resumes the same conversation (`--conversation <id>`). Run agy in a plain terminal outside Marshell with the hooks installed and check nothing changes for it (Review focus 1).
- [ ] **Step 3: Turn the hooks off** and check `hooks.json` is deleted, `settings.json` has the person's own status line back and `dangerouslySkipPermissions` untouched, and Undo restores the install.
- [ ] **Step 4: Write the live-check note** with what passed and failed and the agy version. Stop the check by PID, after listing the tree, and remove the scratch folder after showing its path and getting an OK. Commit as `docs(research): record the Antigravity adapter live check`.
- [ ] **Step 5: Run what CI runs**: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`. Quote the counts. Then the final review on Opus (`marshell-reviewer`, with this plan's Review focus) and one fix pass, then `marshell-ship`.
