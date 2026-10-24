# Slice 6a: the Codex CLI adapter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Codex CLI gets what Claude Code has: hooks installed and removed through the config service, live status in its row, and its permission requests answered from the approve card, Departures and banners. Every piece is checked against a real Codex 0.163.

**Architecture:** Codex gets three new modules in `crates/core/src/agents/codex/`: `hooks.rs` (the plan for `~/.codex/hooks.json`), `status.rs` (hook events folded into `LiveStatus`) and `request.rs` (the card's view of a request, plus Codex's answer JSON). The routes, the bridge and the hold machinery stay agent-neutral. They pick the adapter by agent id where they hard-code `"claude"` today. The frontend's agents API, Agents dialog, onboarding and rows accept `"codex"` beside `"claude"`. Codex hooks run only after the person trusts them in Codex's `/hooks`, so Marshell reports the last time it saw them run and never claims they are active.

**Tech stack:** Rust (axum, serde_json, the existing `config` service and JSONC edits), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, phase 6 and "Slice 6a decisions (2026-10-24)". Evidence: `docs/research/codex-cli.md` (Hooks; Hook sources, trust, and config safety) and `docs/research/2026-10-24-codex-rollouts.md`.

**Ruling made in planning:** a Codex session that runs in a Marshell tab is linked to that tab in History (Task 6). The hooks now say which tab ran it. Without the link, 5d's importer labels these sessions "Outside Marshell", which is wrong.

## Global constraints

- Marshell writes only `$CODEX_HOME/hooks.json`, or `~/.codex/hooks.json` when `CODEX_HOME` is unset. It never writes `config.toml`, and never writes anything else under the Codex home (decision 3).
- Marshell never automates, bypasses or writes Codex's hook trust. The copy tells the person to trust the hooks in `/hooks` (decision 3). It reads `config.toml` read-only to report whether they are trusted (decision 10).
- Tests and live checks never read or write the real `~/.codex`. They use `AppState::with_codex_hooks(<temp>)` or an injected env, and the live check uses a scratch `CODEX_HOME`. The live check copies `~/.codex/auth.json` into the scratch home and deletes it with the scratch folder. That copy is the only file it reads from the real Codex home.
- One new dependency: `toml = "1.1"` in `marshell-core` (decision 10), already in `Cargo.lock` through Tauri (MIT/Apache, active upstream, no network). It parses `config.toml` to read `[hooks.state]` and nothing else. `sha2` is already a core dependency.
- New protocol fields go in `crates/protocol`, and `pnpm check:generated` passes afterwards. An `Option<u64>` gets `#[ts(type = "number | null")]`.
- No new route. Existing `/v1/agents/{agent}/...` routes accept `codex`. `antigravity` and unknown ids still get 404 "no such agent".
- Never hold a `MutexGuard` across `.await` or file I/O. Running `codex --version` and reading files happen in `spawn_blocking`.
- Fixtures are recorded from the real Codex in Task 1 and scrubbed with `pnpm scrub:fixtures` before they are committed (`fixtures_scrubbed.rs` enforces this).
- Keep Codex's conventions in Codex code. `agents/claude/` gains no Codex branch, and the shared pieces move to `agents/common.rs`.
- Copy, exactly:
  - Change titles: "Set up Codex hooks", "Turn Codex hooks off".
  - Status notes: "Live status is off for Codex"; "Codex's version could not be read"; "Codex <v> is older than the versions Marshell knows"; "Codex <v> is not one Marshell has recorded yet".
  - The core's refusal of an "Always allow" decision: "Codex can't keep an always-allow rule from Marshell. Allow once, or answer in the terminal."
  - Consent card source line: "The hook entries go in <code>hooks.json</code> only, not in <code>config.toml</code> as well: Codex reads both and would warn about duplicates." Consent card trust line (amended after Task 1, A3): "Codex runs hooks only after you trust them. The next time Codex starts, it lists them as new: choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>."
  - Agents dialog trust line (decision 10), by `AgentSetup.trust`:
    - `trusted`: "Trusted: Codex runs these hooks."
    - `untrusted`: "Not trusted yet: Codex won't run these hooks until you trust them. The next time Codex starts, choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>."
    - `changed`: "Changed since you trusted them: Codex won't run the changed hooks until you trust them again in <code>/hooks</code>."
    - `null` (Claude, no entries of Marshell's, or a `config.toml` that doesn't parse): no trust line.
- Every UI change works with the keyboard alone and in forced colours. Each CLI gets its own consent card, and there is no bulk allow.

## Assumptions Task 1 confirms or corrects

The research describes the documented Codex behaviour. Task 1 checks it against Codex 0.163. If a finding differs, the plan is amended (shapes, constants, copy) and committed before Task 2 starts.

Every finding and its evidence is in `docs/research/2026-10-24-codex-hooks.md`.

- **A1.** `$CODEX_HOME/hooks.json` is `{"hooks": {"<Event>": [{"matcher": "<regex>", "hooks": [{"type": "command", "command": "<string>", "timeout": <secs>}]}]}}`, and `matcher` is optional for events that have no tool.
  - **Finding (Task 1):** confirmed, with no matcher on any group. Codex also loads an empty group `{"hooks": []}` without a warning and counts its position (decision 11).
- **A2.** On Windows, `command` = `"C:/Users/<user>/.marshell/bin/marshell-hook.exe" event --agent codex --tag mshl1` runs, quoted path with a space included. If it doesn't, the form that does run is recorded (`commandWindows`, or a different quoting).
  - **Finding (Task 1):** it doesn't run. Codex 0.163 runs a hook command through PowerShell on Windows (pwsh 7.6.8 here), where a quoted path followed by arguments is a parse error. `& "<bridge>" <role> --agent codex --tag mshl1` runs, with a space in the path. So Windows uses the call operator, and other platforms use the plain quoted form.
  - **Finding (A3):** trust is per entry and hash-bound. It's stored in the Codex home's `config.toml` as `[hooks.state.'<hooks.json path>:<event_snake>:<group>:<handler>'] trusted_hash = "sha256:…"`. At its next start, Codex lists new or changed entries ("12 hooks are new or changed") with "Trust all and continue". Changing one entry asks again for that entry only. Group and handler are raw positions: removing a group shifts the later groups of that event, and Codex then asks for them again (decision 11, Task 2b).
  - **Finding (A4):** `MARSHELL_HOME` and `MARSHELL_TAB_ID` reach the hook. The core received the record post with the tab.
  - **Finding:** a turn that ends in an error (a usage limit) fires no `Stop`, and Codex has no StopFailure-like event. The row stays Working until the next event, and the slice's Known limits say so.
- **A3.** Hooks don't run until they are trusted in `/hooks`. The finding records where trust is kept and whether changing one entry un-trusts the others.
- **A4.** `TAB_ID` and the rest of the session's environment reach the hook process. If Codex scrubs the environment, the bridge can't name the tab, and this plan stops for a ruling.
- **A5.** Payloads carry `session_id`, `transcript_path` (the rollout), `cwd`, `hook_event_name`, `model` and `permission_mode`. `PreToolUse` and `PermissionRequest` add `tool_name` and `tool_input`. The finding names the exact tool names and input fields for a shell command and an `apply_patch`.
  - **Finding (Task 1):** confirmed with corrections. The shell tool is `tool_name: "Bash"` (on Windows too) with `tool_input.command`. `apply_patch` puts the whole patch in `tool_input.command`, and the file is on its `*** Add File:`, `*** Update File:` or `*** Delete File:` line. `PermissionRequest` has no `tool_use_id`, and for `Bash` adds `tool_input.description` (Codex's reason). `Stop.last_assistant_message` is `null` when the turn ends without a reply. `SessionEnd` carries only `reason` beyond the common four fields, with no `model` or `permission_mode`. Only `permission_mode: "default"` was seen. No MCP tool ran, and the research gives no source for its `tool_name` format.
  - **Finding (Task 1):** `SessionStart` fires with the first prompt, not at launch. A Codex quit with no prompt fires only `SessionEnd`, and still leaves a rollout. In the sandbox, a failed command is retried with escalation: two `PreToolUse`, and a `PostToolUse` for the second only. A denied request goes `PermissionRequest`, then `Stop`, with no `PostToolUse`. An interrupt fires `Interrupt` and no `Stop`.
- **A6.** Stdout `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}` allows, `behavior: "deny"` with `message` denies, and empty stdout leaves Codex's own prompt.
  - **Finding (Task 1):** confirmed, all three. A hook that passes its timeout leaves Codex's own prompt, and a later answer is ignored.

## Review focus

1. **Hooks installed but not trusted.** Codex runs nothing, so the row must not show live status or claim the hooks work. The row keeps the limited fallback, and the Agents dialog says "Not trusted yet". Tests in Tasks 3, 3b and 8.
2. **A user's own `hooks.json`** (BOM, CRLF, comments, their own hook on the same event). Their bytes stay as they were, apart from Marshell's entries, on install and on removal. Tests in Task 2.
3. **Codex run outside Marshell with the hooks installed** (Windows Terminal, or Marshell closed). The bridge prints nothing, Codex asks the person itself, and the only delay is the 50 ms connect timeout. Test in Task 5.
4. **Two Codex tabs asking at once.** Each card answers its own request, and an answer never reaches the other tab. Test in Task 5.
5. **Codex updated to a version Marshell hasn't recorded.** The row gets the "not one Marshell has recorded yet" note instead of guessed status. Approvals still reach the card, and an unknown tool shape shows as "other" with its raw input. Tests in Tasks 4 and 5.

---

### Task 1: Spike: real Codex hooks, and the fixtures (main thread, live)

This task runs in the main thread, not through a worker. It needs a real Codex session and the `/hooks` trust screen.

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1`, `.claude/skills/marshell-live-check/SKILL.md`
- Create: `docs/research/2026-10-24-codex-hooks.md`
- Create: `crates/core/tests/fixtures/codex/hooks/0.163.0/*.json`

- [x] **Step 1: Add `-CodexAuth` to `live-check.ps1 start`.** With the switch, it copies `$HOME\.codex\auth.json` into `<Scratch>\cx\auth.json` after the scratch folders exist, and copies nothing else. `stop` already removes the whole scratch folder. Add one line to SKILL.md's start step saying what the switch copies and that `stop` deletes it.
- [x] **Step 2: Start the live check.** Run `live-check.ps1 start -CodexAuth` with a fresh literal scratch folder. Write `<Scratch>\cx\hooks.json` by hand: every event in the research table, each running `"<scratch bridge>" record --agent codex --tag mshl1`. The `record` role saves each stdin under the record directory and posts it.
- [x] **Step 3: Drive a session.** Open a Codex tab in the dev app with `-a untrusted`, so shell commands ask. (Ran as `codex --no-daemon -s read-only -a on-request`: 0.163 has no `untrusted`, and the scratch `CODEX_HOME` is too long for the daemon's socket.) Over CDP, run `/hooks`, trust the entries, then send one prompt that runs a shell command, one that makes an `apply_patch` edit, and then interrupt a turn. Answer each Codex prompt in the terminal.
- [x] **Step 4: Check A2 and A6 by hand.** Point the `PermissionRequest` entry at a two-line `.cmd` stub in the scratch folder that prints the A6 allow JSON. (Ran as a second `PermissionRequest` group whose stub prints a file, so the answer changed without a new trust.) Confirm Codex runs the command without asking. Then use a deny stub with a message, then an empty stub. Note whether editing the entry made `/hooks` ask for trust again (A3).
- [x] **Step 4b: Record two trust-hash test vectors (decision 10).** Add two extra groups to the scratch `hooks.json` whose commands name a neutral path, `& "C:/Users/someone/.marshell/bin/marshell-hook.exe" event --agent codex --tag mshl1`: one on `Stop` with `timeout` 5, one on `SessionEnd` with `timeout` 2. Restart Codex, choose **Trust all and continue**, and copy each entry's `trusted_hash` from the scratch `config.toml` into the research doc beside its event, command and timeout. Check that the recipe in Task 3b reproduces both. These are Task 3b's vectors, and they carry no real user name.
- [x] **Step 5: Write the fixtures.** Copy one recorded body per event (`SessionStart.json`, `UserPromptSubmit.json`, `PreToolUse-shell.json`, `PreToolUse-apply_patch.json`, `PermissionRequest-shell.json`, `PermissionRequest-apply_patch.json`, `PostToolUse.json`, `PostToolUse-apply_patch.json`, `Stop.json` (no reply), `Stop-recap.json`, `Interrupt.json`, `SessionEnd.json`) into `crates/core/tests/fixtures/codex/hooks/0.163.0/`. Run `pnpm scrub:fixtures crates/core/tests/fixtures/codex/hooks`, then `cargo test -p marshell-core -j 4 --test fixtures_scrubbed`. Expected: PASS.
- [x] **Step 6: Write `docs/research/2026-10-24-codex-hooks.md`.** It answers A1–A6, each with evidence, plus the Codex version, the exact `hooks.json` that ran, and the real `~/.codex` check: `config.toml`'s modified time before and after, and `hooks.json` still absent. Stop the check by PID and remove the scratch folder.
- [x] **Step 7: Amend this plan where a finding differs, and commit.**

```bash
git add .claude/skills/marshell-live-check docs/research/2026-10-24-codex-hooks.md crates/core/tests/fixtures/codex/hooks docs/superpowers/plans/2026-10-24-phase-6a-codex-adapter.md
git commit -m "docs(research): record Codex 0.163's hooks, trust and payloads, with scrubbed fixtures"
```

### Task 2: The Codex hook plan

**Files:**
- Modify: `crates/core/src/agents/common.rs`, `crates/core/src/agents/claude/hooks.rs` (move only)
- Create: `crates/core/src/agents/codex/hooks.rs`; Modify: `crates/core/src/agents/codex/mod.rs`
- Test: unit tests in `codex/hooks.rs`, reusing `crates/core/tests/fixtures/claude/settings/{bom,crlf,comments-trailing-commas}.json` as hand-copied text

**Interfaces:**
- Produces (moved to `agents/common.rs`, re-exported from `claude::hooks` under the same names so Claude's callers don't change): `pub const TAG: &str = "mshl1"`, `pub enum PlanError`, `pub fn is_ours(entry: &Value) -> bool`.
- Produces in `agents::codex::hooks`:
  - `pub const STATUS_EVENTS: [&str; 7] = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "Interrupt", "SessionEnd"]`
  - `pub fn hooks_path(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf`: `$CODEX_HOME/hooks.json` when `CODEX_HOME` is non-empty, else `<user_home>/.codex/hooks.json`
  - `pub fn hook_command(home: &Path, role: &str) -> String`: on Windows `& "<bridge>" <role> --agent codex --tag mshl1`, elsewhere `"<bridge>" <role> --agent codex --tag mshl1`, with the bridge path in forward slashes and quoted (the A2 finding)
  - `pub struct Found { pub install: HookInstall, pub approve_installed: bool }`
  - `pub fn inspect(hooks: &Value, home: &Path) -> Found`
  - `pub fn install_plan(path: &Path, text: Option<&str>, home: &Path, want: &HookPlanRequest) -> Result<ChangeSet, PlanError>`, titled "Set up Codex hooks"
  - `pub fn remove_plan(path: &Path, text: Option<&str>, home: &Path) -> Result<ChangeSet, PlanError>`, titled "Turn Codex hooks off"
- Constants: timeout 5 s for routine events, 2 s for `SessionEnd` and `Interrupt`, 600 s for `PermissionRequest` (role `permission`; the bridge waits 590 s). The scope is `ConfigScope::User`, and edits apply to `Applies::NewSessions`. `want.statusline` is ignored, because Codex has no status line.

- [ ] **Step 1: Write the failing tests.**
  - `install_into_a_missing_file_adds_every_event`: with `text = None` and status plus approve wanted, the planned file has one Marshell group under each of the 7 events and under `PermissionRequest`, every command contains `--agent codex --tag mshl1`, and `PermissionRequest`'s command has role `permission` and timeout 600.
  - `status_only_adds_no_permission_entry`.
  - `install_keeps_the_users_own_hooks`: the user's own `Stop` group stays first and unchanged.
  - `install_on_an_installed_file_changes_nothing`, and `inspect` reports `Installed` with `approve_installed`.
  - `an_entry_from_another_home_is_orphaned`: a tag match with a different bridge path gives `Orphaned`.
  - `remove_takes_only_ours`: after removal the user's groups are byte-equal to the input. Run it over the BOM, CRLF and comments texts.
  - `an_unparsable_file_is_refused_with_its_line`: `PlanError::DoesNotParse`.
  - `codex_home_names_the_file`: with `CODEX_HOME=X` the path is `X/hooks.json`, and with it empty or unset the path is `<home>/.codex/hooks.json`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 codex::hooks`.** Expected: FAIL, because the module doesn't exist.
- [ ] **Step 3: Implement it.** Follow `claude::hooks`: remove Marshell's own entries highest index first, then append one group per event (or set the event to `[group]`), using the same `JsonSet`/`JsonRemove` edits with `expected_hash`.
- [ ] **Step 4: Run Step 2's command, then `cargo test -p marshell-core -j 4 claude::hooks`.** Expected: both PASS. The Claude tests prove the move changed nothing.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(codex): plan the hooks.json entries that install and remove Marshell's Codex hooks`.

### Task 2b: Keep the user's hooks at their positions (decision 11)

Added after Task 1, which found that Codex keys trust by position. It changes Task 2's plan code. Task 3's routes and `marshell uninstall` use `remove_plan` and `install_plan`, so they pick the change up without edits.

**Files:**
- Modify: `crates/core/src/agents/codex/hooks.rs`
- Test: unit tests in `codex/hooks.rs` and `codex/trust.rs`

**Interfaces (unchanged signatures):**
- Removal, in `remove_plan` and in the removal half of `install_plan`: a Marshell group that has any group after it that stays (the user's, or an empty one) becomes `{"hooks": []}` through `JsonSet` at its pointer. A Marshell group with nothing staying after it is removed as before, highest index first. `prune` removes an event only when every group in it was removed. Empty groups already in the file are left alone, Marshell's leftovers included, because Marshell can't tell them from the user's.
- Reinstall: where an event already has a Marshell group, `install_plan` sets the wanted entry at that group's position (`JsonSet` on `/hooks/<Event>/<g>`) instead of removing it and appending. Any further Marshell groups for that event follow the removal rule. An event with no Marshell group gets one appended at the end, as before.
- A Marshell entry inside a group that also holds the user's entries is removed as before (an entry can't be left empty), and the slice's Known limits say so.
- `inspect` and `trust::state` don't count an empty group as Marshell's, and still count its position.

- [x] **Step 1: Write the failing tests.**
  - `removal_keeps_a_later_user_group_in_place`: `Stop: [ours, user]` gives `[{"hooks": []}, user]`, and the user's group is byte-equal to the input. Run it over the BOM, CRLF and comments texts.
  - `removal_drops_a_trailing_group`: `Stop: [user, ours]` gives `[user]`.
  - `an_event_with_a_placeholder_left_stays`: after removing `[ours, user]`, the event is still there, and an event that held only Marshell's group goes.
  - `reinstall_replaces_ours_where_it_is`: `Stop: [ours with another timeout, user]` gives `[ours as wanted, user]`, with Marshell's group still at index 0.
  - `install_after_a_placeholder_appends`: `Stop: [{"hooks": []}, user]` gives `[{"hooks": []}, user, ours]`.
  - In `trust.rs`: `a_placeholder_counts_as_a_group`: `Stop: [{"hooks": []}, ours]` looks Marshell's entry up under `stop:1:0`.
- [x] **Step 2: Run `cargo test -p marshell-core -j 4 --lib codex::`.** Expected: FAIL on the new tests.
- [x] **Step 3: Implement it.**
- [x] **Step 4: Run Step 2's command, then `cargo test -p marshell-core -j 4 --test agents_api` and `cargo test -p marshell-cli -j 4`.** Expected: PASS.
- [x] **Step 5: Run `cargo fmt --all` and commit** as `fix(codex): leave the user's hooks at their positions when Marshell's are removed or replaced`.

### Task 3: Setup, plan and removal routes for Codex, and `marshell uninstall`

**Files:**
- Modify: `crates/protocol/src/agents.rs`, `src/generated/AgentSetup.ts` (regenerated)
- Modify: `crates/core/src/server/agents.rs`, `crates/core/src/server/mod.rs`, `crates/core/src/server/hook.rs`
- Modify: `crates/cli/src/uninstall.rs`
- Test: `crates/core/tests/agents_api.rs`; unit tests in `uninstall.rs`

**Interfaces:**
- Consumes: Task 2's `codex::hooks::*`.
- Consumes Task 3b's `codex::trust::state` and `HookTrust`.
- Produces, on `AgentSetup`: `pub needs_trust: bool` (true only for codex) and `pub trust: Option<HookTrust>` (serialized as `null` when `None`), filled from `codex::trust::state` over the hooks file and the `config.toml` beside it. Always `None` for Claude.
- Produces on `AppState`:
  - `with_codex_hooks(path: PathBuf) -> Self` and `with_codex_version(version: Option<String>) -> Self`, which mirror the Claude ones.
  - Codex's `config.toml` is `with_codex_hooks`'s path with the file name `config.toml`, read in the same `spawn_blocking` as the hooks file.
- In `server/agents.rs`, `only_claude` becomes `fn adapter(agent: &str) -> Result<Adapter, Fail>` with `enum Adapter { Claude, Codex }`. `setup`, `plan` and `remove_plan` dispatch on it, and `codex_setup_of(st: &AppState) -> Result<AgentSetup, Fail>` fills `has_own_statusline: false` and `chained: false`.

- [ ] **Step 1: Write the failing tests** in `agents_api.rs`, each with `with_codex_hooks(<temp>/hooks.json)` and `with_codex_version(Some("0.163.0"))`:
  - `codex_setup_reports_a_missing_file`: `Missing`, `needs_trust: true`, `trust: null`.
  - `codex_plan_then_apply_installs`: the plan, `/v1/config/apply`, then setup reads `Installed` with `approve_installed`.
  - `codex_remove_plan_after_install_leaves_the_users_hooks`.
  - `codex_setup_reads_trust_from_config_toml`: after install, setup says `untrusted`; with a `config.toml` holding `trust::hook_hash` for every entry it says `trusted`; with one hash altered, `changed`. The `config.toml` bytes are unchanged afterwards.
  - `antigravity_is_still_404`.
  - In `uninstall.rs`: `removes_codex_hooks_too` (both diffs printed, one question, both applied on yes), `a_codex_only_install_is_found`, and `nothing_installed_says_so` still passing with neither installed.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --test agents_api` and `cargo test -p marshell-cli -j 4`.** Expected: FAIL.
- [ ] **Step 3: Implement it.** `uninstall::run` builds both remove plans, prints each diff under its file's path, asks once, and applies each in turn. A refused file still exits 1, as today.
- [ ] **Step 4: Run Step 2's commands, plus `cargo test -p marshell-protocol` and `pnpm check:generated`.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(codex): set up, preview and remove Codex hooks through the agents routes and marshell uninstall`.

### Task 3b: Read Codex's hook trust (decision 10)

Runs before Task 3, which consumes it.

**Files:**
- Modify: `crates/core/Cargo.toml` (`toml = "1.1"`), `Cargo.lock`
- Modify: `crates/protocol/src/agents.rs` (`HookTrust`), `src/generated/HookTrust.ts` (generated)
- Create: `crates/core/src/agents/codex/trust.rs`; Modify: `crates/core/src/agents/codex/mod.rs`
- Test: unit tests in `trust.rs`

**Interfaces:**
- Produces in `marshell_protocol::agents`: `pub enum HookTrust { Trusted, Untrusted, Changed }`, serialized lowercase, with ts-rs.
- Produces in `agents::codex::trust`:
  - `pub fn hook_hash(event: &str, group: &Value, entry: &Value) -> String`: `"sha256:"` + lowercase hex of SHA-256 over Codex's identity JSON, compact, keys sorted, non-ASCII unescaped: `{"event_name": <snake_case event>, "hooks": [{"async": false, "command": <command>, "timeout": <t>, "type": "command"}], "matcher": <the group's matcher>}`. `matcher` appears only when the group has one and the event is not `UserPromptSubmit`, `Stop` or `Interrupt`. `t` is the entry's `timeout` or 600, capped at 3 for `SessionEnd` and `Interrupt`, and at least 1. `statusMessage` joins the hook object only when the entry sets it. `commandWindows` never counts. Insert each object's keys in sorted order, so the bytes don't depend on serde_json's `preserve_order`. Source: openai/codex `rust-v0.163.0`, `hooks/src/engine/discovery.rs` (`hook_hash`) and `config/src/fingerprint.rs`.
  - `pub fn state(hooks_path: &Path, hooks: &Value, config_toml: Option<&str>) -> Option<HookTrust>`. `None` when Marshell has no entries in `hooks` or `config_toml` doesn't parse. Each Marshell entry at event `E`, group `g`, handler `h` is looked up in `[hooks.state]` under the key `<hooks_path>:<snake E>:<g>:<h>`. The path part is Codex's (native separators) and is compared ignoring case and `/` against `\`. A key whose `trusted_hash` differs from `hook_hash` is changed. A missing key, a missing `trusted_hash` or `enabled = false` is untrusted. Any changed gives `Changed`, else any untrusted gives `Untrusted`, else `Trusted`. A missing `config.toml` (`None`) is `Untrusted`.

- [ ] **Step 1: Write the failing tests** in `trust.rs`:
  - `hash_matches_codex_for_the_recorded_vectors`: the two vectors Task 1 recorded in its research doc (event, command and timeout in; `trusted_hash` out).
  - `a_missing_config_is_untrusted`, `every_entry_trusted_is_trusted`, `one_altered_hash_is_changed`, `an_entry_without_state_is_untrusted`, `a_disabled_entry_is_untrusted`.
  - `the_key_counts_the_users_groups_first`: a user group at index 0 puts Marshell's entry at `:1:0`.
  - `a_config_that_does_not_parse_gives_no_answer` and `no_entries_of_ours_gives_no_answer`.
- [ ] **Step 2: Run `cargo test -p marshell-core -j 4 --lib codex::trust`.** Expected: FAIL, because the module doesn't exist.
- [ ] **Step 3: Implement it.** Add `toml` with `cargo add -p marshell-core toml@1.1`, and check that `Cargo.lock` gains no new package.
- [ ] **Step 4: Run Step 2's command, then `cargo test -p marshell-protocol` and `pnpm check:generated`.** Expected: PASS.
- [ ] **Step 5: Run `cargo fmt --all` and commit** as `feat(codex): read whether Codex trusts Marshell's hook entries from config.toml`.

### Task 4: Live status from Codex's hooks

**Files:**
- Modify: `crates/core/src/agents/common.rs`, `crates/core/src/agents/claude/mod.rs`, `crates/core/src/agents/claude/status.rs` (move only)
- Create: `crates/core/src/agents/codex/status.rs`; Modify: `crates/core/src/agents/codex/mod.rs`
- Modify: `crates/core/src/server/hook.rs` (`fold_live`)
- Test: unit tests in `codex/status.rs` and `codex/mod.rs`; `crates/core/tests/agents_api.rs`

**Interfaces:**
- Produces in `agents::common`: `pub enum Gate { Known, Unknown(String) }`, `pub fn version_gate(name: &str, known: &[(u32, u32, u32)], version: Option<&str>) -> Gate` (Claude's body, with `name` in each message), and `pub(crate) fn blank(now_ms: u64) -> LiveStatus`. `claude::version_gate(v)` stays as a wrapper whose messages are unchanged.
- Produces in `agents::codex`:
  - `pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(0, 163, 0)]`
  - `pub fn version_gate(version: Option<&str>) -> Gate`, with name "Codex"
  - `status::fold_event(prev: Option<&LiveStatus>, body: &Value, now_ms: u64) -> Option<LiveStatus>`
  - `status::mode_of(body: &Value) -> Option<SessionMode>`, over the five documented `permission_mode` values
- Folds:

  | Event | State, phrase |
  |---|---|
  | `SessionStart` | Idle, "" |
  | `UserPromptSubmit` | Working, "Thinking" |
  | `PreToolUse` shell (`tool_name` `Bash`) | Working, "Running \`<command, first line, 60>\`" |
  | `PreToolUse` apply_patch | Working, "Editing \`<first file>\`", the file from the first `*** Add File:`, `*** Update File:` or `*** Delete File:` line of `tool_input.command`; "Editing files" when there is none |
  | `PreToolUse` other tool | Working, "Using \`<tool>\`" |
  | `PermissionRequest` | NeedsPermission, "Wants to run \`<command>\`" or "Wants to use \`<tool>\`" |
  | `PostToolUse` | Working, "Thinking" after NeedsPermission, else the phrase is kept |
  | `Stop` | Done, "Done", with the recap from `last_assistant_message` (first line, 200); no recap when it is `null` |
  | `Interrupt` | Idle, "" |
  | `SessionEnd` | Ended, "Ended" |
  | anything else | None |

  Field names come from Task 1's fixtures. `since_ms` changes only when the state changes, as in Claude's fold.

  Task 1's timing findings the fold must tolerate:
  - `SessionStart` arrives with the first prompt, so a Codex tab has no live status until then and keeps the limited fallback. Any event can be the first, with `prev = None`; `SessionEnd` often is.
  - `SessionEnd` has no `permission_mode`, so `mode_of` returns `None` for it and the tab's mode is left as it was.
  - A sandboxed command that fails is retried: `PreToolUse` twice and one `PostToolUse`. A denied request goes `PermissionRequest`, then `Stop`, with no `PostToolUse`, and must end Done, not NeedsPermission. An interrupt sends `Interrupt` and no `Stop`.
  - A turn that ends in an error (a usage limit) sends no `Stop`, so the row stays Working until the next event. This goes in the slice's Known limits.
  - `codex --version` prints `codex-cli 0.163.0`, and the gate reads the version after the prefix.
- `fold_live(st, agent, role, tab, body, at_ms)` checks `status_off` for `agent`. It gets the version from `agent_version(agent, exe)` in `spawn_blocking` and gates with that agent's gate. Only Claude takes `Statusline`. The `if agent == "claude"` guard in `post` becomes `matches!(agent.as_str(), "claude" | "codex")`.

- [x] **Step 1: Write the failing tests.**
  - One test per row of the table, loading the Task 1 fixture with `include_str!`: `session_start_is_idle`, `prompt_is_thinking`, `shell_tool_is_running_its_command`, `apply_patch_is_editing_its_file`, `permission_request_needs_permission`, `post_tool_after_a_permission_is_thinking`, `stop_is_done_with_its_recap` (`Stop-recap.json`), `stop_without_a_reply_has_no_recap` (`Stop.json`), `interrupt_is_idle`, `session_end_is_ended`, `an_unknown_event_is_none`.
  - Timing: `session_end_with_no_earlier_event_is_ended` (`prev = None`), `a_denied_request_then_stop_is_done`, `a_retried_tool_keeps_running`, `session_end_leaves_the_mode_alone`, `apply_patch_without_a_file_line_is_editing_files`.
  - `the_version_is_read_after_codex_cli`: `codex-cli 0.163.0` gives a known version.
  - Gate tests: `codex_0_163_is_known`; `codex_0_170_is_not_recorded`, asserting the exact note "Codex 0.170.0 is not one Marshell has recorded yet"; `codex_0_162_is_older`.
  - `claude_gate_messages_are_unchanged`.
  - In `agents_api.rs`: `a_codex_event_folds_into_its_tab` (needs a live tab, as Claude's fold tests do) and `codex_status_off_sets_its_note`.
- [x] **Step 2: Run `cargo test -p marshell-core -j 4 --lib codex::` and `cargo test -p marshell-core -j 4 --test agents_api`.** Expected: FAIL.
- [x] **Step 3: Implement it.**
- [x] **Step 4: Run Step 2's commands, then `cargo test -p marshell-core -j 4 claude::`.** Expected: PASS.
- [x] **Step 5: Run `cargo fmt --all` and commit** as `feat(codex): fold Codex's hook events into live status, gated on the recorded version`.

### Task 5: Codex approvals

**Files:**
- Create: `crates/core/src/agents/codex/request.rs`; Modify: `crates/core/src/agents/codex/mod.rs`, `crates/core/src/agents/mod.rs`
- Test: unit tests in `codex/request.rs`; `crates/core/tests/permissions.rs`; a unit test in `crates/hook/src/permission.rs`

**Interfaces:**
- Produces:
  - `codex::request::detail(request: &Value) -> PermissionDetail`: `tool_name` `Bash` gives the bash kind with its command, and Codex's `tool_input.description` as the description when the bash kind has one. `apply_patch` gives the edit kind with its first file, read from the patch as in Task 4. Anything else gives other with the raw `tool_input`. MCP requests fall under other too, because Task 1 recorded no MCP tool name and the research gives no source for one. The request has no `tool_use_id`, so nothing keys on it.
  - `codex::request::view(p: &PendingPermission) -> Option<PermissionView>`, with `always_rule: None`.
  - `codex::permission_output(decision: &PermissionDecision, request: &Value) -> Result<Value, &'static str>`: allow gives exactly the A6 JSON, and deny adds `message` from the decision (Claude's text when there is none). `PermissionScope::Project` gives `Err` with the exact refusal copy from the constraints.
- `agents::supports_permissions` matches `"claude" | "codex"`. `permission_view` and `permission_output` dispatch on the agent.

- [x] **Step 1: Write the failing tests.**
  - `a_shell_request_is_a_bash_detail` and `an_apply_patch_request_is_an_edit_of_its_file`, from the fixtures.
  - `an_unknown_tool_is_other_with_its_input`.
  - `allow_is_exactly_codexs_shape`: string equality with the A6 JSON.
  - `deny_carries_the_message`.
  - `always_allow_is_refused`.
  - In `permissions.rs`:
    - `a_codex_permission_is_held_and_answered`: post role `permission`, agent `codex`, with a live tab. `GET /v1/permissions` lists it with no always rule, and an allow decision makes the held post return 200 with the allow JSON.
    - `a_codex_project_decision_is_409`.
    - `two_codex_tabs_get_their_own_answers`.
    - `no_decision_returns_204`, using `with_permission_hold`.
  - In `permission.rs`: `accepts_codex_allow_and_deny` for a Codex request body.
  - In the bridge tests: `a_codex_permission_without_tab_prints_nothing`.
- [x] **Step 2: Run `cargo test -p marshell-core -j 4 --lib codex::request`, `cargo test -p marshell-core -j 4 --test permissions` and `cargo test -p marshell-hook -j 4`.** Expected: FAIL.
- [x] **Step 3: Implement it.**
- [x] **Step 4: Run Step 2's commands.** Expected: PASS.
- [x] **Step 5: Run `cargo fmt --all` and commit** as `feat(codex): hold Codex's permission requests for the approve card and answer in Codex's shape`.

### Task 6: A Codex session in a tab belongs to that tab in History

**Files:**
- Modify: `crates/core/src/history/recorder.rs` (`note_event`), `crates/core/src/server/hook.rs` (`record_history`)
- Test: unit tests in `recorder.rs` and `crates/core/src/history/import.rs`

**Interfaces:**
- `History::note_event(&self, agent: &str, tab_id: &str, recording: Option<&str>, body: &Value, at_ms: u64) -> Option<SessionMeta>`. For `codex`, the path check is `sidecar_allowed("codex", path, id)`. It writes the sidecar's agent, tab, recording and `client: Marshell`, and returns None, so no copy is scheduled: 5d's importer copies the rollout. Claude's behaviour is unchanged.
- For Codex, the link is made by the first event of a session that carries `session_id` and `transcript_path`, not only by `SessionStart`. Task 1 found `SessionStart` fires with the first prompt, and a Codex quit with no prompt sends only `SessionEnd` yet leaves a rollout, which would otherwise be labelled "Outside Marshell". `record_history` passes every Codex event through, and `note_event` makes the link once.

- [x] **Step 1: Write the failing tests.**
  - `a_codex_start_in_a_tab_records_the_tab`.
  - `the_importer_keeps_a_codex_tab_session_marshells`: note a Codex `SessionStart` for `t1`, then import the same rollout. The row has `client: Marshell` and `tab_id: "t1"`, whichever runs first.
  - `a_codex_rollout_path_with_the_wrong_name_is_ignored`.
  - `a_codex_session_with_only_session_end_is_linked`, from `SessionEnd.json`.
  - `a_later_codex_event_does_not_rewrite_the_link`.
- [x] **Step 2: Run `cargo test -p marshell-core -j 4 history::`.** Expected: FAIL.
- [x] **Step 3: Implement it.** Add the `agent` argument at every caller, and widen `record_history`'s guard to Codex.
- [x] **Step 4: Run Step 2's command.** Expected: PASS.
- [x] **Step 5: Run `cargo fmt --all` and commit** as `feat(history): link a Codex session run in a tab to that tab`.

### Task 7: Codex in the Agents dialog and onboarding

**Files:**
- Modify: `src/lib/api/agents.ts`, `src/features/agents/{AgentsDialog.tsx,agentsState.ts,useInstallFlow.ts}`, `src/features/onboarding/{LiveOnboarding.tsx,ConsentCard.tsx,consent.ts,fixtures.ts}`
- Test: `src/features/agents/{AgentsDialog.test.tsx,agentsState.test.ts,useInstallFlow.test.tsx}`, `src/features/onboarding/{consentCopy.test.ts,ConsentCard.test.tsx}`

**Interfaces:**
- `export type HookAgent = "claude" | "codex"` in `src/lib/api/agents.ts`. `getSetup`, `planInstall` and `planRemove` take a `HookAgent`. `toDetections` no longer marks Codex `later`.
- `useInstallFlow(endpoint, agent: HookAgent, onApplied)`.
- `consentProps(s, flow)` uses `s.agent` for `cli` and `removalCopy`, and `s.needs_trust` for `needsTrust`.
- The Agents dialog shows the trust line for `s.trust` (the exact strings from the constraints) under Codex's install state.
- `CAPABILITIES.codex.status.entries` becomes 7.
- `fixtures.ts`'s Codex sample becomes the `hooks.json` JSON shape from Task 1.

- [ ] **Step 1: Write the failing tests.**
  - `shows a Codex section with its own consent when Codex is found`.
  - `Codex's trust line for each trust state, and none for null` (the exact strings from the constraints).
  - `consentProps for Codex asks for trust and uses Codex's removal copy`.
  - `the install flow plans at /v1/agents/codex/hooks/plan`.
  - `the source line names hooks.json, not config.toml`.
  - `onboarding offers one card per found agent with hooks`.
  - `the Codex entry count matches its diff`.
- [ ] **Step 2: Run `pnpm test src/features/agents src/features/onboarding`.** Expected: FAIL.
- [ ] **Step 3: Implement it.** Each found hook agent gets its own section with the existing controls, and the live-status switch writes `status_off` for that agent.
- [ ] **Step 4: Run Step 2's command, then `pnpm build`.** Expected: PASS.
- [ ] **Step 5: Commit** as `feat(agents): set up and remove Codex hooks from onboarding and the Agents dialog`.

### Task 8: Codex rows and cards

**Files:**
- Modify: `src/features/sessions/toRow.ts` (the setup link at about line 143), `src/features/sessions/LiveApp.tsx` (fetch the setup state for both hook agents), `src/features/approval/fromCore.ts`
- Test: `src/features/sessions/toRow.test.ts`, `src/features/approval/ApproveCard.test.tsx`

**Interfaces:**
- The setup map passed to `toRow` is `Partial<Record<HookAgent, HookInstall>>`. The setup link shows for `setup?.[info.agent]` when it is set and not `"installed"`.

- [x] **Step 1: Write the failing tests.**
  - `a Codex row without hooks offers the setup`.
  - `an installed but untrusted Codex keeps the limited fallback with no setup link`.
  - `a Codex row with live status shows its phrase`.
  - `a Codex card offers no Always allow`.
- [x] **Step 2: Run `pnpm test src/features/sessions/toRow.test.ts src/features/approval`.** Expected: FAIL.
- [x] **Step 3: Implement it.**
- [x] **Step 4: Run Step 2's command, then `pnpm build`.** Expected: PASS.
- [x] **Step 5: Commit** as `feat(sessions): show Codex's live status and setup link in its row`.

### Task 9: Live check and the slice's exit (main thread)

**Files:**
- Create: `docs/research/2026-10-24-codex-adapter-live-check.md`
- Modify: `docs/PLAN.md` (the phase 6 line names this live check)

- [ ] **Step 1: Run `cargo test -p marshell-core -j 4 codex`.** Expected: PASS. This is the exit's "fixture suites from the real CLIs" for Codex.
- [ ] **Step 2: Run the live check.** Use `marshell-live-check` with `-CodexAuth` on the `phase-6a` head. Codex 0.163 can't start its background server under a `CODEX_HOME` longer than about 60 characters ("path must be shorter than SUN_LEN"), and a tab opened from the Codex profile can't add `--no-daemon`. So first give `live-check.ps1 start` a `-CodexHome <short literal path>` option, for example `%TEMP%\lc-6a-4-cx`: it refuses a non-empty folder, records the path in `pids.json`, and `stop` deletes the copied `auth.json` from it as it does today. Add it to SKILL.md's start step, and remove the folder with the scratch folder at the end of the work. Then check, in order:
  1. Onboarding shows a Codex card beside Claude's, with the exact `hooks.json` diff in the scratch `CODEX_HOME`.
  2. Apply it. The Agents dialog says "Not trusted yet".
  3. A Codex tab: choose **Trust all and continue**. The row shows Thinking, then Running, then Done, and the dialog, reopened, says "Trusted". Change one entry's timeout in the scratch `hooks.json` by hand: the dialog says "Changed since you trusted them".
  4. In Manual mode (`--sandbox read-only --ask-for-approval on-request`), a shell write reaches the card. Allow runs it, and Deny stops it with the message.
  5. The banner appears with the window unfocused, and the request shows in Departures.
  6. With Marshell closed, Codex asks itself.
  7. History lists the conversation under Marshell, linked to its tab.
  8. Removing the hooks from the Agents dialog leaves a seeded user hook. Seed it after Marshell's group on `Stop`, and trust it. After removal, Codex starts without asking about it (decision 11).
  9. `marshell uninstall` (answer no) prints both diffs.
  10. The real `~/.codex` is unchanged: same `config.toml` modified time, no `hooks.json`.
- [ ] **Step 3: Fix each finding test-first and record it** in the research doc, with its commit.
- [ ] **Step 4: Run what CI runs:**
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace -j 4`
  - `pnpm test`
  - `pnpm check:generated`
  - `pnpm build`

  Expected: all pass.
- [ ] **Step 5: Commit** as `docs(research): the slice 6a live check`. Then the whole-branch review (`marshell-reviewer`), one fix pass, and `marshell-ship`.
