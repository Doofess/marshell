# Slice 6m: maintenance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix issues #85 and #82, and the deferred minors from slices 6j2, 6k, 6l1 and 6l2 that were checked against the code and confirmed.

**Architecture:** These are independent fixes in four areas: the bash integration script, the WSL uninstall path (core surface plus the CLI), the hook and preview code of the config service, and small session, socket and UI fixes. All tasks run in the `phase-6m` worktree. The session is isolated to it, so no second worktree is used. The Rust tasks (lanes A and B) run one at a time, because one worker's half-done code would break another's build. The frontend tasks (lane F) touch no Rust and no file a Rust task touches, so they run beside them. No new dependencies.

**Tech Stack:** Rust (axum core, jsonc-parser 0.34 CST, the `marshell` CLI crate), bash, React + TypeScript (Vitest in node, no DOM), ts-rs generated types.

**Spec:** `docs/PLAN.md` §9, "Slice 6m decisions (2026-11-04)". The issues are #85 and #82. The evidence for each lead was gathered read-only on main at `d464d2c` and is copied into each task, because workers can't see the session's notes. Line numbers are from `d464d2c` and may drift a few lines.

## Global Constraints

- Commit messages read `type(scope): what changed, as a sentence`. Run `cargo fmt --all` before every Rust commit.
- Every cargo command uses `-j 3` (the plan's commands say so), for example `cargo test -j 3 -p marshell-core <filter>`. The default job count runs rustc out of memory on this machine, and a frontend worker may be running beside you.
- Two workers share the worktree. Stage only your own files, by path (`git add <paths>`), never `git add -A` or `git commit -a`. If git reports `index.lock`, wait a few seconds and retry. Run only the tests for your own files: `pnpm test <file>` and the crate's filtered `cargo test`. Don't run the whole frontend suite or `pnpm build`; the lead runs those.
- The session is isolated to the `phase-6m` worktree. Run commands from it with the Bash tool as plain commands. A compound command that changes directory, or a PowerShell call from Bash, can be refused.
- A new protocol field goes in `crates/protocol`. Then run `cargo test -j 3 -p marshell-protocol` and `pnpm check:generated`, and commit `src/generated/`. New fields are `Option` with `#[ts(optional)]` and `skip_serializing_if`, so existing fixtures stay valid.
- Never hold a `MutexGuard` across `.await` or I/O. A `wsl.exe` call or a file read on a `\\wsl.localhost` path runs in `spawn_blocking` or on a plain thread, never on an async worker.
- UI copy is sentence case, puts the subject first, and has no "!" and no "successfully". Don't guess a status value.
- No new dependencies (SCOPE.md). Don't touch `crates/core/src/server/guard.rs` or token handling.
- Never run `pnpm tauri dev`, `wsl.exe` against a real distro, or a real `marshell uninstall` from a worker. Unit tests inject every WSL lookup.
- A fake home that must not count as the user's home lives under the repo's `target` folder, canonicalized: `tempfile::tempdir_in(projects::canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")))`. The guard allows `dirs::home_dir()`, and `%TEMP%` is inside it, so a `tempdir()` fake home hides missing guard wiring.

## Review Focus

1. **#85, an rc file with `set -u`, its own DEBUG trap, or `shopt -u promptvars`.** Marks stay right: one `133;C` per real command, none at a bare prompt, and the prompt never shows a literal `${__marshell`. Task 1 tests this.
2. **#82, a distro whose file is refused or doesn't parse.** That distro is reported by path. The Windows removal and every other distro's removal still apply, because one bad distro file never reaches the whole-run abort at `uninstall.rs:361-367`. Task 4 tests this.
3. **Undo after Hooks off.** Undo puts back both the settings file and `statusline-chain.json`, because the delete is in the same change set. Task 2 tests this.
4. **An elevated session closed while its output is stalled.** Close ends the demux within a second, even when the renderer is attached but not acking. Task 9 tests this.
5. **A terminal socket for a start that fails or is declined at UAC.** The held upgrade answers 404 as soon as the start's claim ends, not at the deadline. Task 10 tests this.

---

## Lane A: config, agents and the CLI (one Rust task at a time)

### Task 2: Hooks off leaves nothing behind

**Files:**
- Modify: `crates/core/src/agents/claude/hooks.rs` (`install_plan` ~:347-379, `removal_pointers` ~:209-230, `remove_plan` ~:478-553, tests)
- Modify: `crates/core/src/agents/codex/hooks.rs` (install's event change ~:380-388, `removal_ops` ~:245-258, the loose test at ~:670-675)
- Modify: `crates/core/src/agents/antigravity/hooks.rs` (`remove_plan` ~:475-550), `crates/core/src/agents/qwen/hooks.rs` (`remove_plan` ~:544-620)
- Modify: every caller of the three `remove_plan`s: `crates/core/src/server/agents.rs` (~:442, :455, :492), `crates/cli/src/uninstall.rs` (~:169-243)
- Modify: `src/features/onboarding/consent.ts` (`removalCopy`) and `consent.test.ts`
- Test: `crates/core/tests/agents_api.rs`

**Interfaces:**
- Produces: `remove_plan(..., chain_text: Option<&str>, chain: Option<&Path>, distro: Option<&str>)` for Claude Code, Antigravity and Qwen Code. `chain` comes right after `chain_text`, as in each `install_plan`. Task 4 passes `None` for a distro.

Evidence. Only Qwen handles an empty `PermissionRequest` array (6l2 decision 8): `qwen/hooks.rs:233` counts an empty one as Marshell's, and `:416-427` removes the whole event when approvals turn off. Claude's `install_plan` removes `/hooks/PermissionRequest/<i>` element by element (`claude/hooks.rs:359-363`), which leaves `"PermissionRequest": []`. A later Hooks off skips the empty array (`:220`), so `/hooks` stays. Codex does the same (`codex/hooks.rs:245-247`, and the test at `:670-675` accepts it). Antigravity sets or deletes one whole `marshell` hook object, so it can't leave an empty array. The `remove_plan` docs say "The chain file is read and never changed" (`claude/hooks.rs:480`, `antigravity/hooks.rs:478`, `qwen/hooks.rs:546`), so `~/.marshell/agents/<cli>/statusline-chain.json` stays after Hooks off (decision 7). Codex has no status line and no chain file.

- [ ] **Step 1: Write the failing tests.**
  - Claude and Codex, inline: install both roles, then install with approvals off. Assert `hooks` has no `PermissionRequest` key. Replace Codex's loose `is_none_or(|a| a.is_empty())` with that.
  - `remove` on `{"hooks":{"PermissionRequest":[]}}` removes `/hooks` whole. With a user event beside it, only the `PermissionRequest` key goes. Pin the known limit too: a user's own empty `PermissionRequest` next to Marshell's status install is removed.
  - Claude, Antigravity and Qwen: install, then `remove_plan` with `chain = Some(path)` and `chain_text = Some(..)`. The plan has one `EditOp::DeleteFile` edit for the chain path, `ConfigScope::Marshell`, expecting the chain's hash. With `chain_text = None` or `distro = Some(..)`, there is no chain edit.
  - `agents_api.rs`: set up Claude with a status line, turn hooks off, and assert the chain file is gone. Then "Undo last fix" restores both `settings.json` and the chain, byte for byte.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core hooks` fails on the new assertions.
- [ ] **Step 3: Implement.**
  - Claude: copy Qwen's pattern. When approvals are off and every `PermissionRequest` group is Marshell's, emit one `remove_op("/hooks/PermissionRequest")`. In `removal_pointers`, treat an empty `PERMISSION_EVENT` array as found.
  - Codex: the same. Codex's install goes through `event_change`, `replaced` and `tail_clear`, so the whole-event removal must keep the index shift of sibling groups right. Don't just copy Qwen's lines.
  - Keep it scoped to `PermissionRequest`. No other user-empty event is deleted.
  - Add the chain `DeleteFile` to the same `ChangeSet` (or `RemovePlan`) as the settings edit, so one snapshot covers both.
  - The removal reason gains the chain file. Qwen's reason text for an empty array, "Removes the empty PermissionRequest hook list left by an earlier Marshell.", is used for Claude and Codex too.
  - `removalCopy` in `consent.ts` names the chain file for the three CLIs that chain.
  - Update every caller to pass the chain path (`paths::statusline_chain(home, agent)`), and `None` for a distro.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core`, `cargo test -j 3 -p marshell-cli` and `pnpm test src/features/onboarding src/features/agents` pass. `agentsState.ts:154` feeds `removalCopy` into the Agents section.
- [ ] **Step 5: Commit.** `fix(agents): hooks off removes an emptied approval list and the status line chain file, for every CLI`

### Task 3: A WSL surface the CLI can use (#82, core)

**Files:**
- Modify: `crates/core/src/wsl.rs`, `crates/core/src/profiles/detect.rs` (`run_wsl` ~:121-150, the Docker filter in `detect_profiles` ~:241-245)
- Create: `crates/core/src/agents/distro.rs` (and register it in `agents/mod.rs`)
- Modify: `crates/core/src/server/agents.rs` (`distro_target` ~:320-346, `split_home_and_bridge` and `distro_target_of` ~:70-86, `distro_remove` ~:393-435, the test `a_distro_target_is_the_distros_own_share` ~:1827)

**Interfaces:**
- Consumes: Task 2's `remove_plan(.., chain_text, chain, distro)` for Claude Code, Antigravity and Qwen Code. A distro passes `chain: None`, because a distro's status line runs with `--no-chain`.
- Produces:
  - `pub fn wsl::list_distros(timeout: Duration) -> Vec<String>` runs `wsl.exe -l -q`, decodes it and leaves out `docker-desktop` and `docker-desktop-data`. It returns empty off Windows and when `wsl.exe` is missing, fails or times out.
  - `pub fn wsl::distro_home(distro: &str, timeout: Duration) -> Option<PathBuf>` runs `wsl.exe -d <distro> --exec sh -c 'printf "%s\n" "$HOME"'` and keeps the result through `home_in_share`.
  - `pub fn wsl::home_in_share(distro: &str, linux_home: &str) -> Option<PathBuf>` maps `/home/me` to `\\wsl.localhost\<distro>\home\me`. It gives `None` for a home that is empty, not absolute, or outside the distro's share, such as `/mnt/c/...`.
  - `pub fn agents::distro::remove_plans(distro: &str, home: &Path, marshell_home: &Path) -> Vec<(&'static str, Result<Option<ChangeSet>, PlanError>)>` gives one entry per agent (`claude`, `codex`, `antigravity`, `qwen`). `Ok(None)` means there is nothing of Marshell's to remove.

Evidence. `uninstall.rs` has no WSL code. `distro_remove` (`server/agents.rs:393-435`) plans the four agents' removal for a distro home that `distro_target` found with `wsl.exe` (8 s timeout). The helpers are private, and `run_wsl` is `pub(crate)`. Two traps:
- `distro_target` also runs `wslpath -u` on the bridge path, which a partial earlier uninstall may already have deleted. `distro_home` asks for `$HOME` only.
- The doc comment at `server/agents.rs:70-72` belongs to `distro_target_of` but sits above `split_home_and_bridge`. Move it while you are here.

- [ ] **Step 1: Write the failing tests.**
  - `home_in_share`: `/home/me` maps to the UNC path; `/mnt/c/Users/me`, `""` and `relative` give `None`. Move `a_distro_target_is_the_distros_own_share` here.
  - `list_distros`'s parsing and filtering over UTF-16LE bytes that include `docker-desktop` and `docker-desktop-data`. Split it into a pure `parse_distro_list(bytes: &[u8]) -> Vec<String>` so it is testable without `wsl.exe`.
  - `remove_plans` over a fake home under `target` with Marshell's hooks for all four agents: four `Ok(Some(_))`. Over an empty home: four `Ok(None)`.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core wsl` and `cargo test -j 3 -p marshell-core distro` fail to compile.
- [ ] **Step 3: Implement.**
  - Move the Docker filter into one shared function that `detect_profiles` uses too.
  - `distro_target` keeps its single combined call (6k decision 4) and uses `home_in_share`.
  - `AppState::distro_remove` calls `remove_plans`, so uninstall runs exactly the dialog's removal (6k decision 7). The plans read files as `distro_remove` does today.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core` passes. The distro API tests `remove_plan_for_a_distro_targets_its_home` (`tests/agents_api.rs:~3092`) and its Qwen twin (`~:3143`) stay green.
- [ ] **Step 5: Commit.** `refactor(wsl): the distro list, home lookup and distro removal plans are callable from the CLI`

### Task 4: `marshell uninstall` cleans WSL distros (#82, CLI)

**Files:**
- Modify: `crates/cli/src/uninstall.rs` (`run` ~:160-441, tests ~:520-593), `crates/cli/src/main.rs` (~:87-118), `crates/cli/src/args.rs` (USAGE ~:23)

**Interfaces:**
- Consumes: Task 3's `wsl::list_distros`, `wsl::distro_home`, `wsl::under_distro`, `agents::distro::remove_plans`, and Task 2's `remove_plan(.., chain: None, distro)`.
- Produces: `pub struct Wsl<'a> { pub distros: &'a dyn Fn() -> Vec<String>, pub home: &'a (dyn Fn(&str) -> Option<PathBuf> + Sync), pub pinned: bool }`, which `run` takes as a new argument. `main.rs` passes the real lookups with a 20 s timeout and `pinned: false`.

Decisions 2 and 7 and the copy:
- **Which distros.** Look in `distros()` plus every distro that `managed::list(home)` names. Run each entry's path through `guard::unverbatim`, then `wsl::under_distro`. Look up the homes on all distros at once, with `std::thread::scope`, each with its own 20 s limit.
- **Before planning.** Build the service once, with `ConfigService::with_distros(home, ..)` instead of `ConfigService::new` (`uninstall.rs:263`), and call `record_distro_home(d, &home, pinned)` for each home. Without that wiring every distro file comes back Refused, and the refusal at `:361-367` aborts the whole uninstall.
- **Planning.** Preview each distro's change set on its own first, as skills are at `:284-296`. Merge only the admissible ones into the single change set, so there is still one prompt and one snapshot (3c decision 6). A distro file that is refused or doesn't plan is printed by path and sets `hooks_failed`, so `bin` stays and the exit code is 1.
- **A distro that doesn't answer.** It is named in the output and the exit code is 1. `bin` stays only when `managed.json` names that distro, and is removed otherwise.
- **The status-line note.** Don't add a distro's `statusline_left` to the Windows note at `:333`. Its wording, "changed since the setup", is wrong for a distro.
- **When there are no distros.** Print nothing about WSL when nothing is listed and nothing is managed.
- **Copy:**
  - `Looking in WSL distros: Ubuntu, Debian.`
  - `Ubuntu: Marshell's hooks found.`, `Ubuntu: nothing of Marshell's.`, `Ubuntu didn't answer. Start it and run marshell uninstall again.`
  - `keep_note` for a distro: `...because hooks in Ubuntu may still point at it.`
  - Update the module comment at `:1` and the USAGE line.

- [ ] **Step 1: Write the failing tests** in `uninstall.rs`. The fake distro home sits under `target`, the injected `Wsl` has `distros: || vec!["Ubuntu"]`, and `pinned` is true. Install hooks there with `Bridge::Distro { name: "Ubuntu", path: "/mnt/c/.../marshell-hook.exe" }` through a pinned service.
  1. `removes_marshells_hooks_inside_a_distro`: Claude, Codex, Antigravity and Qwen hooks, with the person's own entries and status line beside them. Run with `--yes`. No `mshl1` is left in any of the four files, the person's entries stay, the output names Ubuntu, the Windows hooks are gone, `bin` is removed, and the exit code is 0.
  2. `a_distro_that_does_not_answer_is_named`: the home lookup gives `None`. The output names Ubuntu as not cleaned, the Windows removal still applies, and the exit code is 1. Run it twice: with a `managed.json` entry under `\\wsl.localhost\Ubuntu\...`, `bin` stays; without one, `bin` is removed.
  3. `a_distro_file_that_does_not_parse_does_not_block_the_rest`: a broken `.codex/hooks.json` in the fake home. It is reported by path and left byte for byte, the others are removed, `bin` stays, and the exit code is 1.
  4. `a_refused_distro_file_is_left_out`: Ubuntu isn't in `distros()` and isn't managed. Its home is never looked up and nothing in it changes.
  5. `uninstalls_service_admits_a_recorded_distro_home`: the helper that builds the service, given `["MarshellTestDistro"]` and a recorded unpinned `\\wsl.localhost\MarshellTestDistro\home\u` home, gives `path_refusal(..\.claude\settings.json) == None`. With an empty list it gives `Some`. Follow `config/service.rs:~675`.
  6. `managed_json_names_a_distro_the_list_missed`: an empty list and a managed entry under Ubuntu. The injected lookup is called for Ubuntu.
  7. `no_wsl_says_nothing`: an empty list and nothing managed. The output has no WSL line, and every existing test is unchanged.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-cli uninstall` fails.
- [ ] **Step 3: Implement** as above.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-cli` passes.
- [ ] **Step 5: Commit.** `feat(cli): uninstall removes Marshell's hooks inside WSL distros and names any that don't answer` (closes #82)

### Task 5: Previews name the backup folder and a one-line file's new layout

**Files:**
- Modify: `crates/protocol/src/config.rs` (`FilePreview` ~:145, `ConfigPreview` ~:162), then regenerate `src/generated/`
- Modify: `crates/core/src/config/preview.rs` (`to_preview`), `crates/core/src/config/service.rs` (wherever a `ConfigPreview` is built: `store_preview`, `restore_preview`, `get_preview`)
- Modify: `src/features/agents/agentsState.ts` (the plan view model ~:150), `src/features/onboarding/ConsentCard.tsx` (~:115-124), `src/features/companies/CompanySection.tsx` (~:166, :213-218), and their tests

**Interfaces:**
- Produces:
  - `FilePreview.one_line_spread: Option<bool>`, set to `Some(true)` only when the file before had text and no line break (ignoring trailing `\r` and `\n`) and the file after has one.
  - `ConfigPreview.snapshots_dir: Option<String>`, the service's real snapshot folder.
  - The consent plan view model gains `oneLineSpread: boolean`.

Evidence. jsonc-parser 0.34 forces a new object property onto several lines (`cst/mod.rs:3485-3489`), so setup on a one-line settings file rewrites it (+109 −1 in the 6l2 live check). The Company tab's review hard-codes `~/.marshell/snapshots/` (`CompanySection.tsx:213-216`) even under a scratch `MARSHELL_HOME`. The real folder is `paths::snapshots_dir(home)`. `review.plan` there is a `SettingsPlan`, so the field is read as `review.plan.preview.snapshots_dir`. `ChangeBar` and `FixSheet` hard-code the same text, but `SettingsPage` and `FixSheet` render only in stories, and `ProfilesSection`'s `ChangeBar` has no review. They stay as they are (ruling).

- [ ] **Step 1: Write the failing tests.**
  - Rust: `one_line_spread` is `Some(true)` for `{"a":1}` to a multi-line file, and `None` for multi-line to multi-line, one-line to one-line, and a new file (no before).
  - Rust: a preview from a service on a temp home has `snapshots_dir == Some(<home>/snapshots)`.
  - Vitest:
    - The consent card with `oneLineSpread` contains "This file was on one line. Marshell lays it out over several lines." under the diff, and without it doesn't.
    - The Company tab review shows the same sentence when a file in its preview has `one_line_spread`.
    - The Company tab review with `snapshots_dir: "C:/scratch/mh/snapshots"` contains that path and not `~/.marshell/snapshots/`. Without the field, it keeps the old text.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core preview` fails. `pnpm test src/features/onboarding src/features/companies` fails.
- [ ] **Step 3: Implement.** Add both fields, then regenerate with `cargo test -j 3 -p marshell-protocol`. Fill them in the core, and show them in `ConsentCard` and `CompanySection`.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core`, `pnpm check:generated`, `pnpm test src/features/onboarding src/features/companies src/features/agents` pass.
- [ ] **Step 5: Commit.** `feat(config): previews name the real backup folder and say when a one-line file is laid out over several lines`

### Task 6: Raw saves keep CRLF, and an appended object's layout is pinned

**Files:**
- Modify: `crates/core/src/server/agents.rs` (`settings_read` ~:1202-1204, `raw_op` ~:1428-1467, tests)
- Modify: `crates/core/src/config/jsonc.rs` (`has_lone_lf` ~:60-63 becomes `pub(crate)`, tests near ~:680-717)

Evidence. `raw_op` builds `EditOp::ReplaceText` from the textarea text and puts back only a BOM (`had_bom`). A textarea always gives LF, so a CRLF file is saved as LF and Review shows every line changed (6l2 live check finding 5). The form path keeps CRLF (`jsonc.rs:60-63`). `raw_op` serves Claude, Codex (TOML), Antigravity and Qwen. The append path (`jsonc.rs:373-387`) has no test for an object element in a multi-line array. The hook installs append objects (`claude/hooks.rs:374`, `codex :383`, `qwen :437`), and a comment at `:402-404` says the CST renders a replaced object one level too shallow.

- [ ] **Step 1: Write the failing tests.**
  - Raw save on `{\r\n  "a": 1\r\n}\r\n` (and a Codex TOML twin), sent as LF text with one edit: the `ReplaceText` content keeps `\r\n` with no lone LF. Do the same with a BOM. A mixed-ending file and an LF file are saved as typed. The preview diff of a one-line edit touches one line.
  - `jsonc.rs`: append `{"y":2,"z":[1]}` to `/a/-` of `{\n  "a": [\n    {"x":1}\n  ]\n}\n` and assert the exact text: inner lines at 6 spaces, closing brace at 4. Do the same with a real Codex hook group fixture (`codex/hooks.rs` `USER_GROUP`) in a two-space file.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core raw` fails. Run the `jsonc` layout tests too.
- [ ] **Step 3: Implement.**
  - `settings_read` also reports `crlf`, using the same rule as `has_lone_lf`: the text has `\r\n` and no lone LF.
  - `raw_op` checks the text as sent, so a refusal's line numbers don't move. After the check, when `crlf` is set, it turns every LF into CRLF and then puts back the BOM.
  - Fix the append layout only if its test failed. Indent the added element's continuation lines to the separator's indent. A trailing `// comment` on the last element moving after the new element stays a known limit.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core` passes.
- [ ] **Step 5: Commit.** `fix(config): a raw save keeps the file's CRLF line endings`, plus a separate `test(config): ...` or `fix(config): ...` commit for the append layout.

### Task 7: `marshell list` marks administrator sessions

**Files:**
- Modify: `crates/cli/src/list.rs` (table ~:15-24, `Row` ~:44-54, test `list_table_and_json` ~:120-158), `crates/cli/src/args.rs` (USAGE ~:10, if it lists columns)

Decision 4. `SessionInfo` already carries `elevated: bool`.
- `--json` rows always get `"elevated": true/false`.
- The table gets an `ADMIN` column, after `MODE`, only when at least one listed row is elevated. It reads `yes` for those rows, and other rows use the table's existing empty-cell convention.
- With no elevated row, the output is byte-identical to today.
- If the Marshell skill text (`SKILL.md` under `crates/`) lists the JSON fields, add `elevated` there.

- [ ] **Step 1: Write the failing tests.**
  - An elevated sample has `"elevated": true` in the JSON and `ADMIN`/`yes` in the table, with columns still aligned and no trailing spaces.
  - An all-unelevated list has no `ADMIN` header.
  - Add `"elevated": false` to the exact-object assertion in `list_table_and_json`.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-cli list` fails.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-cli` passes.
- [ ] **Step 5: Commit.** `feat(cli): marshell list marks sessions that run as administrator`

### Task 8: Qwen's preview versions and its system settings file per OS

**Files:**
- Modify: `crates/core/src/agents/qwen/mod.rs` (`version_gate` ~:54-65, test `a_preview_or_nightly_version_is_read` ~:121-130)
- Modify: `crates/core/src/server/agents.rs` (`qwen_system_settings_path` ~:221-226)

Evidence:
- **The version gate.** `version_gate` strips the `-preview.1` suffix before calling `common::version_gate`, so `0.26.5-preview.1` counts as the recorded `0.26.5`. Semver puts it before `0.26.5`. `common::version_gate` (`common.rs:342-346`) already refuses a pre-release as Known for every other CLI.
- **The system path.** `qwen_system_settings_path` is the literal `C:\ProgramData\qwen-code\settings.json` on every OS. The research note (`docs/research/2026-11-03-qwen-code-surfaces.md:59`) gives `/Library/Application Support/QwenCode/settings.json` on macOS and `/etc/qwen-code/settings.json` on Linux. Don't read the `ProgramData` variable: the note doesn't say Qwen does. `QWEN_CODE_SYSTEM_SETTINGS_PATH` stays unread (6l2 decision 9).

- [ ] **Step 1: Write the failing tests.**
  - `version_gate(Some("0.26.5-preview.1"))` and `Some("0.26.6-nightly.20261101")` are not `Gate::Known`, and the note names the full version string. `0.26.5` and `0.26.9` stay Known.
  - `qwen::system_settings_for("windows" | "macos" | "linux")` returns the three paths.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core qwen` fails.
- [ ] **Step 3: Implement.**
  - `version_gate` passes the full version to `common::version_gate` and drops the stripping and the `replacen`. Keep `parse_version` as it is, so a preview string is still read and shown.
  - Check that the note's wording reads right for a preview. If it says "older than the versions Marshell knows", that is accurate by semver.
  - Add `pub fn system_settings_for(os: &str) -> &'static str` to `qwen/mod.rs`. `qwen_system_settings_path` uses it with `std::env::consts::OS`.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core` passes.
- [ ] **Step 5: Commit.** `fix(qwen): a preview build isn't taken for a recorded version, and the system settings file is found per OS`

## Lane B: pty, sessions and shell (one Rust task at a time, after or between lane A's)

### Task 1: Bash marks a command's start from PS0, so later prompt hooks can't fake one (#85)

**Files:**
- Modify: `crates/core/shell/marshell.bash` (the header ~:1, the trap ~:54-67, the `PROMPT_COMMAND` assignment ~:63)
- Modify: `crates/core/src/registry.rs` (only the comment at ~:709-711)
- Test: `crates/core/tests/shell_integration.rs`

Evidence. This was reproduced in WSL Ubuntu with bash 5.3.9 and systemd 259.
- `/etc/profile.d/80-systemd-osc-context.sh` makes `PROMPT_COMMAND` an array. It runs `PROMPT_COMMAND+=('')` and `PROMPT_COMMAND+=(__systemd_osc_context_precmdline)`, and sets `PS0='$(__systemd_osc_context_ps0)'"${PS0:-}"`.
- `marshell.bash:63` assigns a string, which rewrites only element `[0]`. Element `[1]` runs after `__marshell_prompt_end` has cleared `__marshell_in_prompt` (`:49`).
- The DEBUG trap (`:54-61`) fires on element `[1]`. It prints `133;C` at every prompt and sets `__marshell_ran`, so the real command sends no `133;C`. Then `registry.rs:730` refuses every agent's hook mark.
- Without systemd, a non-login `~/.bashrc` with `PROMPT_COMMAND+=('')` and `PROMPT_COMMAND+=(late_hook)` gives the same bytes. So does bash-preexec on bash 5.3, which puts its epilogue back last at every prompt.
- Two more faults have the same cause. The trap overwrites `$_` before every command, so `mkdir -p d && cd "$_"` breaks. And a user's own DEBUG trap makes Marshell install none, so such a tab never reports a running command.
- In the reproduction, array wrapping alone failed a hook appended lazily at the first prompt, and PS0 alone lost `133;A` when a later element rewrote `PS1`. Together they passed all eight setups.

- [ ] **Step 1: Write the failing tests** in `shell_integration.rs`. Each fixture goes in `~/.bashrc` through `Shell::start`'s setup closure; the start is non-login. Add a helper that keeps only `PromptStart`, `Executed` and `Finished` from `sh.marks`. Steps for A to E: expect `PromptStart`, type `\r`, expect `PromptStart`, type `false\r`, expect `Finished(Some(1))`, expect `PromptStart`. Then assert the filtered marks equal `[Finished(None), PromptStart, Finished(None), PromptStart, Executed, Finished(Some(1)), PromptStart]`.
  - A `bash_array_prompt_command_entry_after_ours_marks_only_real_commands`: `late_hook() { :; }\nPROMPT_COMMAND+=('')\nPROMPT_COMMAND+=(late_hook)\n`.
  - B `bash_lazy_prompt_hook_appended_at_the_first_prompt`: `late_hook() { :; }\nlazy() { [[ " ${PROMPT_COMMAND[*]} " == *" late_hook "* ]] || PROMPT_COMMAND+=(late_hook); }\nPROMPT_COMMAND+=(lazy)\n`.
  - C `bash_array_entry_overwriting_ps1_keeps_marks`: `PROMPT_COMMAND+=('')\nPROMPT_COMMAND+=('PS1="x$ "')\n`. `PromptStart` still comes after `\r`.
  - D `bash_without_promptvars_falls_back_to_the_trap`: fixture A plus `shopt -u promptvars`. The same marks, and the output has no `${__marshell`.
  - E `bash_with_its_own_debug_trap_reports_running_commands`: `trap ': user trap' DEBUG\n`. The same marks, and `trap -p DEBUG` still prints the user's trap.
  - F `bash_keeps_the_last_argument`: an empty rc. Type `echo one two >/dev/null; echo "U=$_"\r` and `expect_text("U=two")`. Repeat with `shopt -u promptvars`.
  - G `bash_nounset_rc_keeps_marks`: fixture A plus `set -u`.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core --test shell_integration bash` fails A, B, C, E and F by assertion. CI's Git Bash is 5.2 and this machine's is 5.3.15, so array fixtures are exercised.
- [ ] **Step 3: Implement** in `marshell.bash`:
  1. **PS0 path (bash 4.4 or later, with `promptvars` on).** Gate: `{ [ "${BASH_VERSINFO[0]}" -gt 4 ] || { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -ge 4 ]; }; } && shopt -q promptvars`.
     - Mark `C` from PS0, which bash prints once per executed command line. It prints nothing for an empty, comment-only or syntax-error line.
     - Use `__marshell_ps0='${__marshell_nil[__marshell_ran=1]-}\e]133;C\e\\'`. The subscript's arithmetic sets `__marshell_ran` in the shell, the element is unset so it prints nothing, and `-` keeps it safe under `set -u`.
     - Append it at init with `PS0="${PS0:-}$__marshell_ps0"`. `__marshell_prompt_end` puts it back if it is missing, as it already does for PS1's `133;A`.
     - Install no DEBUG trap on this path.
  2. **Fallback (older bash, or `promptvars` off).** Keep today's trap, changed to `trap '__marshell_preexec "$_"' DEBUG`. Every path of the function ends with `: "$1"`, so `$_` is kept. Keep the existing check that skips the trap when `trap -p DEBUG` is set, on this path only. Bash before 5.1 runs only `PROMPT_COMMAND[0]`, so #85 can't happen there.
  3. **Array wrap.** If `[[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]`, set `PROMPT_COMMAND=(__marshell_prompt_start "${PROMPT_COMMAND[@]}" __marshell_prompt_end)`. Otherwise keep today's string wrap.
  - Keep the header's "bash 3.2 and later". Update the comments at `:64` and `registry.rs:709-711`: a shell with its own DEBUG trap now reports running commands.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core --test shell_integration` passes, including the existing `bash_reports_prompt_running_and_exit` and `bash_prompt_command_overwriting_ps1_keeps_marks`.
- [ ] **Step 5: Commit.** `fix(shell): bash marks a command's start from PS0, so a prompt hook after Marshell's can't fake one` (closes #85)

### Task 9: The elevated output channel is bounded, and env_clear is tested

**Files:**
- Modify: `crates/core/src/pty/elevated.rs` (the channel ~:284, the demux thread ~:287-302, tests)
- Test: `crates/host/tests/elevated.rs`

Evidence. The demux thread does `let _ = tx.send(bytes)` into an unbounded `mpsc::channel` (`elevated.rs:284`). `Session::wait_for_space` (`session.rs:774-779`) parks only `ChannelReader::read`, so an attached reader that doesn't ack lets the channel grow without limit. A local session parks the pty read itself.

Two traps:
- `Session` sets `exit` only after `wait()`, which is the demux join, returns. A plain blocking `send` would hang Close while the renderer doesn't ack.
- The demux closure doesn't own `down`, which moves into `HostWriter` and `HostControl`. Pass an `Arc` clone of it to read `closing`.

No test checks that the helper's `env_clear` (`crates/host/src/main.rs:160-164`) drops the launcher's variables.

**Interfaces:**
- Produces: `fn demux(up: impl Read, tx: SyncSender<Vec<u8>>, closing: &AtomicBool) -> i32` in `elevated.rs`, private.

- [ ] **Step 1: Write the failing tests.**
  - Build a `Cursor` of 200 `FromHost::Output` frames and an `Exit { code: 7 }` with the crate's frame writer. Run `demux` on a thread with `sync_channel(4)` and never read. A counting reader shows it stalls at about capacity + 1 frames. Drain, and it returns 7.
  - While it is stalled, set `closing`. It returns within 1 s.
  - Host: `the_helper_drops_variables_the_request_does_not_carry` runs `cmd /d /c echo [%MARSHELL_HOST_ALLOW_UNELEVATED%] [%ZZ_KEPT%]` with `ZZ_KEPT=1` in the request and `MARSHELL_HOST_ALLOW_UNELEVATED` left out of it. Wait for `[%MARSHELL_HOST_ALLOW_UNELEVATED%] [1]`. Check that it fails with `cmd.env_clear()` commented out, then restore the line.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core elevated` fails to compile.
- [ ] **Step 3: Implement.**
  - Use `mpsc::sync_channel::<Vec<u8>>(64)`. 64 frames of at most 16 KiB is about `MAX_UNACKED`.
  - `demux` sends with a `try_send` loop that sleeps about 5 ms. It drops the chunk and moves on when `closing` is set or the receiver is gone.
  - Keep "a dropped reader must not stop the loop". Exit is noticed after the backlog drains, which is accepted.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core elevated` and `cargo test -j 3 -p marshell-host --test elevated` pass.
- [ ] **Step 5: Commit.** `fix(elevated): the helper's output channel is bounded, so a slow reader holds back the admin process as a local one does`

### Task 10: A terminal socket waits for a session that is starting

**Files:**
- Modify: `crates/core/src/server/pty_ws.rs` (~:36-39), `crates/core/src/server/mod.rs` (the `starting` set ~:171-173, `claim_start` ~:876, `StartClaim` ~:992-1000)
- Modify: `crates/core/src/server/sessions.rs` only if a start path has no claim (see Step 3)
- Test: `crates/core/tests/pty_ws.rs`

Evidence. The handler answers `404` when `st.session(&tab)` is `None` (`pty_ws.rs:37-39`). A row is reserved before the spawn (`sessions.rs:668`), and the session enters the map only at `insert_session` (`~:865`). So the UI's socket gets 404s and retries (`src/lib/api/ptySocket.ts:64-72`) while an elevated start waits for UAC, and on Reopen, Restart and Resume. Each failed handshake is a console error that JS can't hide.

**Interfaces:**
- Produces: `AppState::is_starting(&self, tab: &str) -> bool`.

- [ ] **Step 1: Write the failing tests.**
  - (a) Reserve a row and claim a start, then `insert_session` 200 ms later on a spawned task. A socket connect made at once succeeds, with no 404.
  - (b) Claim, then drop the claim with no session. The connect answers 404 within 100 ms of the drop.
  - (c) An unknown tab, and a seeded Ended row with no claim, answer 404 at once (under 100 ms).
  - (d) The existing refused-without-the-bearer test is unchanged.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core --test pty_ws` fails (a).
- [ ] **Step 3: Implement.**
  - After the token and subprotocol checks, while `is_starting(tab)` and no session exists, poll every 25 ms up to 120 s. Upgrade as soon as the session appears, and answer 404 when the claim ends without one or at the deadline.
  - Check that every start path holds a `StartClaim` from the row's reservation until `insert_session` or failure: create (with or without administrator), Restart, Resume and Reopen. `claim_start`'s doc says Resume and Restart. If create doesn't claim, add the claim there and widen the doc.
  - Leave the client's backoff as it is.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core --test pty_ws` and `cargo test -j 3 -p marshell-core sessions` pass.
- [ ] **Step 5: Commit.** `fix(server): a terminal socket waits for a session that is still starting instead of answering 404`

### Task 11: Resume reads stay off the async workers, and an id is checked before its meta is read

**Files:**
- Modify: `crates/core/src/server/outside.rs` (the copy branch ~:223, the `for` branch ~:233-240), `crates/core/src/server/sessions.rs` (`resume_session` ~:1315-1346, `resume_as` ~:1427, `restart_as` ~:1560)
- Modify: `crates/core/src/history/archive.rs` (`read_meta`)

Evidence:
- The resume-command route's copy branch runs `tab_plan`, and so `resume_session`, on the async worker (`outside.rs:223`). `resume_session` reads the History index, `archive::read_meta` and `Path::is_file` on the transcript, which for a WSL row is a `\\wsl.localhost` path. `resume_as` and `restart_as` call it inline too. Only the `for` branch uses `spawn_blocking`.
- `resume_session` calls `read_meta` (`:1326`) before `sidecar_allowed_in` checks the id (`:1330`), and `archive_paths` formats the id into a path unchecked. It is reachable only through a hand-edited index, so it is defence in depth.
- The transcript stat stays (ruling). Dropping it would change a WSL row's answer from Fresh to Session.

**Interfaces:**
- Produces: `async fn resume_session_blocking(st: &AppState, tab: &str, agent: &str) -> Result<Resume, String>` in `sessions.rs`, which the `for` branch, the copy branch, `resume_as` and `restart_as` use.

- [ ] **Step 1: Write the failing tests.**
  - An index row whose `session_id` is `../x`, with a decoy `.meta.json` where the traversal would land, gives `Resume::Last`.
  - `archive::read_meta(home, agent, "../x")` is `Ok(None)`.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core resume` fails.
- [ ] **Step 3: Implement.**
  - `resume_session` returns `Resume::Last` for an invalid id before any read, and `read_meta` returns `Ok(None)` for one.
  - Move the whole copy branch into one `spawn_blocking`, mapping a `JoinError` to a 500 as the `for` branch does. Use `resume_session_blocking` in `resume_as` and `restart_as`.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core` passes. The existing resume-command route tests and `resume_session_accepts_a_wsl_row` are unchanged.
- [ ] **Step 5: Commit.** `fix(sessions): resume lookups read files off the async workers and check the id first`

### Task 12: Approve cards are held only in a tab running that agent

**Files:**
- Modify: `crates/core/src/server/hook.rs` (the agent check ~:134-162, the Permission branch ~:219-235)
- Test: `crates/core/tests/permissions.rs`

Decision 6. Today a permission request from Claude Code, Codex or Qwen Code is held in any open tab, because `effective_agent(tab).is_some_and(|a| a != agent)` lets a tab with no agent through (`hook.rs:134-139`). Only Antigravity requires `effective_agent == Some("antigravity")` (`:143-162`, test `an_antigravity_request_from_a_shell_tab_passes`). A Windows shell tab learns its agent from the foreground watcher (`registry.rs:1479-1488`). A WSL tab learns it from its first hook mark (`hook.rs:117-132`). A launched agent row has `info.agent`.

- [ ] **Step 1: Write the failing tests.**
  - For each of `claude`, `codex` and `qwen`: a permission request with the tab header of an open plain shell tab with no foreground agent answers 204, and no permission is pending.
  - The same tab with the registry's foreground agent set to that agent holds the request.
  - A WSL tab marked by an earlier hook of that agent holds it.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core --test permissions` fails.
- [ ] **Step 3: Implement.** In the Permission branch, require `st.registry().effective_agent(tab).as_deref() == Some(agent)` for every agent that holds, and answer 204 otherwise. Keep Antigravity's extra rule. `fold_live` is unaffected. Existing tests that post from a bare shell tab get the tab's foreground or launched agent seeded first.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core` passes.
- [ ] **Step 5: Commit.** `fix(hooks): approve cards are held only in a tab that runs the requesting agent, for every CLI`

### Task 13: The shell panel's recording goes when the panel shell ends unadopted

**Files:**
- Modify: `crates/core/src/server/shell_panel.rs` (`open` ~:78-112, `alive_panel` ~:49-56, the duplicate-loser kill ~:113-122, `take_and_adopt` ~:160-190)
- Test: a panel API test in `crates/core/tests/` with a scratch `MARSHELL_HOME`

Evidence:
- `POST /v1/shell-panel` spawns the sidebar shell with `record = true` (`shell_panel.rs:100-112`). The recording must start at the first byte, because "Open in a pane" adopts the same pty into a tab.
- The file is `<home>/recordings/<tab_id>-<unix_ms>.cast`. The panel has no registry row, so an unadopted panel leaves an orphan recording at every app start, which is the stray `36x12` one. Retention (`history/storage/retention.rs:266`) deletes it only later.

- [ ] **Step 1: Write the failing tests.**
  - Open the panel, then kill or close it. `recordings/` has no `<tab_id>-*.cast`.
  - Open it, then adopt it. The recording stays.
  - The duplicate-loser panel's recording goes.
- [ ] **Step 2: Run.** The test fails.
- [ ] **Step 3: Implement.** At each point where a panel session is removed or killed without being adopted, remove `session.recording_path()` after the recorder has closed, when the tab id isn't in the registry. Removal errors are ignored. App exit is covered by the next start: on `open`, delete earlier panel recordings that no registry row owns. Only do that if the panel's tab ids are recognisable (`PANEL_GENERATION` or a prefix); otherwise leave the exit case to retention and say so in the report.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core` passes.
- [ ] **Step 5: Commit.** `fix(sessions): the sidebar shell's recording is deleted when the shell ends without becoming a tab`

## Lane F: frontend (beside the Rust tasks)

### Task 14: The launcher's Alt+A and its pending session number

**Files:**
- Modify: `src/features/launcher/launcherKeys.ts` (`adminKey` ~:44-46), `src/features/launcher/Launcher.tsx` (`onSheetKey` ~:235-239, the chip ~:432-434, the footer ~:605-625)
- Modify: `src/features/sessions/LiveLauncher.tsx` (`start`, `startTask`, `continueHeld` ~:388-456), `src/features/launcher/launcherSetup.ts`
- Test: `launcherKeys.test.ts`, `launcherSetup.test.ts`, `Launcher.test.tsx`

**Interfaces:**
- Produces: `adminKeyClaimed(e: KeyLike, enabled: boolean): boolean` and `shownNumber(live: number, starting: boolean, frozen: number | undefined): number`.

Evidence:
- **Alt+A.** `onSheetKey` calls `preventDefault()` on Alt+A before `toggleAdmin` checks `admin.enabled` (`Launcher.tsx:235-239`). So a disabled chip (macOS and Linux, or a CLI with `adminUnavailable`) swallows the key, and Option+A is a letter on macOS. The disabled chip also advertises `aria-keyshortcuts="Alt+A"`.
- **The pending number.** The core reserves the row and its number before the spawn (`sessions.rs:668`), and `LiveApp.tsx:1890` recomputes `nextSessionNumber` from the live list. While a start is pending, the footer "Session N" and the screen-reader sentence "It becomes session N" move on by one (6j2 live check, `docs/research/2026-11-01-elevated-host-live-check.md:53`).

- [ ] **Step 1: Write the failing tests.**
  - `adminKeyClaimed(altA, false)` is false and `adminKeyClaimed(altA, true)` is true. Ctrl+A, Alt+Shift+A and AltGr+A are false in both cases.
  - `shownNumber(3, false, 2)` is 3, `shownNumber(3, true, 2)` is 2, and `shownNumber(3, true, undefined)` is 3.
  - The disabled chip's markup has no `aria-keyshortcuts`, and the enabled one keeps it.
  - `nextNumber: 2` with `pending` renders "Session 2".
- [ ] **Step 2: Run.** `pnpm test src/features/launcher` fails.
- [ ] **Step 3: Implement.** In `LiveLauncher`, a ref takes `nextNumber` just before `setStarting(true)` in `start`, `startTask` and `continueHeld`, and clears on failure or finish. The view passes `shownNumber(nextNumber, starting, frozen.current)`.
- [ ] **Step 4: Run.** `pnpm test src/features/launcher src/features/sessions/LiveLauncher` passes.
- [ ] **Step 5: Commit.** `fix(launcher): Alt+A types normally when the Administrator chip is off, and a pending start keeps its session number`

### Task 15: A press on an armed approve card answers it

**Files:**
- Modify: `src/features/approval/arming.ts`, `src/features/approval/ApproveCard.tsx` (`onFocus` ~:149-151, `fire` ~:118-121, the risky Allow's `onPointerDown` ~:241)
- Test: `src/features/approval/arming.test.ts`, `ApproveCard.test.tsx`

**Interfaces:**
- Produces: `rearmOnFocus(f: { viaPointer: boolean; wasArmed: boolean }): boolean`.

Decision 5. Today any focus that enters the card from outside calls `rearm` (`useArming.ts:8-12`: unarmed, then armed again after `ARM_DELAY_MS` = 500). A click that is itself what focuses the card goes mousedown, focus, rearm, then the click hits `fire` while unarmed and is dropped silently. The amended rule: a pointer press that focuses a card already armed doesn't re-arm it. Mount, content change, keyboard focus and programmatic focus (Ctrl+Shift+N) still re-arm.

- [ ] **Step 1: Write the failing tests.**
  - `rearmOnFocus({ viaPointer: true, wasArmed: true })` is false.
  - `{ viaPointer: true, wasArmed: false }` and `{ viaPointer: false, wasArmed: true }` are true.
  - The existing markup tests stay: `data-armed=false` renders `aria-disabled`.
- [ ] **Step 2: Run.** `pnpm test src/features/approval` fails.
- [ ] **Step 3: Implement.** Record in `onPointerDownCapture` whether the card was armed. `onFocus` skips the rearm when `rearmOnFocus` says so, and the flag resets after the focus. The risky Allow answers on `onPointerDown`, so make sure the recorded flag is set before it runs and that its own arming check passes.
- [ ] **Step 4: Run.** `pnpm test src/features/approval` passes.
- [ ] **Step 5: Commit.** `fix(approval): a click that focuses an armed approve card answers it`

### Task 16: A leftover hooks section shows only what can still be done

**Files:**
- Modify: `src/features/agents/AgentsSection.tsx` (~:177 `TrustLine`, ~:186-187 the Live status switch), `src/lib/api/agents.ts` (the doc comments ~:21-25)
- Test: `src/features/agents/AgentsSection.test.tsx`

Evidence:
- 6k decision 2 says a section for a CLI that isn't found but still holds Marshell's hooks has "Hooks off" as its only action.
- `AgentsSection.tsx:177` still renders Codex's `TrustLine` with no `setup.found` check, and `:187` renders the Live status switch.
- The comment at `:186` keeps Live status for a CLI found only in a distro.
- In `src/lib/api/agents.ts`, `listAgents`' doc ("Claude, Codex and Antigravity") was separated from it when `listWslDistros` was inserted between them, and it misses Qwen.

- [ ] **Step 1: Write the failing tests.**
  - A Codex setup with `found: false`, hooks `installed`, trust `untrusted`, `liveStatus: true` and no distros contains the "still in" sentence and "Hooks off". It doesn't contain "Live status" or the trust text.
  - Antigravity and Qwen have no Live status in the same case.
  - The existing case, found only in a distro, keeps Live status.
- [ ] **Step 2: Run.** `pnpm test src/features/agents/AgentsSection.test.tsx` fails.
- [ ] **Step 3: Implement.**
  - Gate the trust line on `setup.found`, and the Live status switch on `setup.found || (distros?.length ?? 0) > 0`.
  - Move the doc comment above `listAgents` and make it read "Claude Code, Codex, Antigravity and Qwen Code, in that order".
- [ ] **Step 4: Run.** `pnpm test src/features/agents` passes.
- [ ] **Step 5: Commit.** `fix(agents): a section for a CLI that isn't found shows only its hooks and Hooks off`

## After the lanes (the lead)

### Task 17: Checks, live check and research note

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1` (the `$env:` block of `start`, ~:208-216) and `SKILL.md`
- Create: `docs/research/2026-11-04-maintenance-6m-live-check.md`

- [ ] **Step 1: Run what CI runs:** `cargo fmt --all --check`, `cargo clippy -j 4 --workspace --all-targets -- -D warnings`, `cargo test -j 4 --workspace`, `pnpm test`, `pnpm check:generated` and `pnpm build`.
- [ ] **Step 2: Fix `NO_COLOR` in the live-check script.** In `start`, add `Remove-Item Env:NO_COLOR -ErrorAction SilentlyContinue` with a comment, and print `NO_COLOR cleared` when it was set. Add one line to `SKILL.md`. The variable belongs to the tool's own environment and blocks Qwen's `/theme`. The app never sets it, and a real user's `NO_COLOR` must still reach their tabs.
- [ ] **Step 3: Live check** with `marshell-live-check`, scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `QWEN_HOME`:
  1. **#85 in the user's Ubuntu.** It is read-only: open a WSL tab and use no agent and no Hooks setup there.
     - `sleep 15` shows a running command, an empty line gives a bare `D`, and `mkdir -p /tmp/marshell-6m-lc/x && cd "$_"` works.
     - A hook mark posted through the bridge with the tab's id is accepted. Run `marshell-hook.exe` with a fake SessionStart payload from inside the tab, using a scratch `HOME` in `/tmp`.
  2. **#82, decision 3.**
     - Import a throwaway distro `Marshell-6m` from Canonical's Ubuntu WSL rootfs into a fresh literal scratch folder. Set hooks up there for each installed CLI from the scratch Marshell. Each CLI's Linux build isn't needed: setup writes files only.
     - Run `marshell uninstall` with the scratch `MARSHELL_HOME` and `USERPROFILE` and answer n.
     - Apply only if the preview names no file outside `\\wsl.localhost\Marshell-6m\`. Then check the files.
     - Stop the distro and run uninstall again, to check the "didn't answer" line if a stopped distro can be made not to answer. Otherwise record that it can't.
     - Ask the user before `wsl --unregister Marshell-6m`.
  3. **Approve cards.**
     - First try to reproduce the dead first click on an unfocused, armed card with a CDP click, and record whether it reproduced. Then check that one click answers.
     - A shell tab with no agent gets no card for a bridge-posted request. A pwsh tab that runs an agent gets one.
  4. **Hooks off and setup.**
     - Hooks off for Claude deletes `statusline-chain.json`, and Undo restores it.
     - Setup on a one-line settings file shows the one-line note.
     - The Company tab's review names the scratch snapshot folder.
     - A CRLF raw save shows a one-line diff.
  5. **Sockets and recordings.**
     - Reopen and Restart log no 404 in the webview console.
     - After a restart of the app, `recordings/` has no unadopted panel recording.
- [ ] **Step 4: Write** the research note with each item's result, and commit it.
- [ ] **Step 5: Commit.** `docs(research): record the slice 6m live check`
