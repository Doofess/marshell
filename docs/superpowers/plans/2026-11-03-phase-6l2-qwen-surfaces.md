# Slice 6l2: Qwen Code's company tab, Accounts, update, skill, History and WSL

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qwen Code gets the rest of what Claude Code, Codex and Antigravity have. That is an Accounts line, its version with an Update button, the `marshell` skill, an editable company tab, its conversations in History (including those run outside Marshell), and hooks inside WSL distros. The four Qwen minors that 6l1 deferred are fixed too.

**Architecture:** Every surface already has a per-CLI table or match. Qwen gets an arm where `"antigravity"` has one, and the "comes in slice 6l2" refusals are removed. One new helper, `agents::qwen::home`, resolves `QWEN_HOME` the way Qwen does (a leading `~` expanded), and every Qwen path (settings, skill, chats) goes through it. History follows Codex's model: the hooks link a chat to its tab, and the importer copies the `chats/<id>.jsonl` file, whether it ran in Marshell or outside. A new `agents/qwen/transcript.rs` reads it into a `Conversation`. The restart bar stops guessing in the frontend and asks the core for the command a restart will run.

**Tech Stack:** Rust (axum, serde_json, `config::jsonc`, `notify`), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, "Slice 6l2 decisions (2026-11-03)", plus 6e decisions 8 to 16, 6f, 6g, 6h2 and 6j1 for the patterns they set. Evidence: `docs/research/2026-11-03-qwen-code-surfaces.md` (the source read) and `docs/research/2026-11-03-qwen-code-hooks.md` (6l1's spike). Task 1 adds real transcripts.

**Checked before planning (2026-11-03):**
- 6l1 left these hooks for the slice: `SignIn::NoCommand` for Qwen (`server/agents.rs` ~1106); `update_argv`/`update_text` return None for a native Qwen (`agents/install.rs` 116-138); `skill.rs`'s `AGENTS` has no Qwen, `skill_problem` refuses it, and the hooks plan skips it (`server/agents.rs` ~932, ~1628); `settings_view::curated` has no Qwen, `editable` is false, and the settings routes refuse it (`server/agents.rs` ~1249, ~1509); History's `filters()` excludes `'qwen'` (`history/index.rs:644`), `parse_for` has no Qwen arm (`history/mod.rs:162`), `server/history.rs:28` lists only Claude and Codex, and `ImportRoots` has no Qwen; `QWEN_NO_WSL` answers 422 (`server/agents.rs:180, 339, 380`), and `config/guard.rs:21`'s `DISTRO_AGENT_FOLDERS` has no `.qwen`.
- `qwen::hooks::settings_path` and `server/agents.rs::qwen_settings_path` read `QWEN_HOME` raw (no `~`).
- `qwen::hooks::removal_pointers` skips an event that holds no Marshell group (`if !found continue`), so an empty `PermissionRequest: []` survives both prune passes. Claude's `hooks.rs` has the same shape, and that is not in scope.
- `qwen::parse_version` accepts exactly three numeric parts, so `0.27.0-preview.1` is "could not be read".
- `resumeCommand.ts` hard-codes `--continue` (and Codex's `resume --last`), while the core restarts by id when it knows one (`detect.rs::resume_args`).

## Global constraints

- Marshell writes, for Qwen: `<qwen home>\settings.json` (hooks and `ui.statusLine` as in 6l1, plus the six curated keys and raw edits through the company tab), `<qwen home>\skills\marshell\SKILL.md`, and the same two inside a distro's `~/.qwen`. It never writes `oauth_creds.json`, a project `.qwen\`, `C:\ProgramData\qwen-code\settings.json` or `~/.agents/skills` for Qwen.
- `<qwen home>` is `QWEN_HOME` (trimmed, a leading `~` or `~\`/`~/` replaced by the user home, otherwise taken as given) when it is set and not blank, else `<user home>\.qwen`. It is never `QWEN_HOME\.qwen`.
- Every write goes through the config service: preview, unified diff, backup, atomic write, `managed.json`, undo. JSON edits are CST edits through `config::jsonc`. A file that doesn't parse fails closed.
- Tests and live checks never read or write the real `~/.qwen`, `~/.claude`, `~/.codex`, `~/.agents` or `~/.marshell`. Tests inject paths with `AppState::with_qwen_home(<temp>)` and temp roots.
- Never run the real `qwen update` against `%LOCALAPPDATA%\qwen-code`. Never name a test binary `qwen`; use `qwen-marshell-test`.
- No new dependency. New protocol fields go in `crates/protocol`, and `pnpm check:generated` passes afterwards.
- Never hold a `MutexGuard` across `.await` or file I/O. File reads and `wsl.exe` run in `spawn_blocking`.
- Qwen's conventions stay in `agents/qwen/` and Qwen arms. `agents/claude/` gains no Qwen branch.
- Every UI change works with the keyboard alone and in forced colours, with the existing classes.
- Copy, exactly:
  - Accounts: "Qwen Code has no sign-in status command, so Marshell can't show one. Sign in with /auth in a Qwen Code session." When `security.auth.selectedType` is set, a second line: "Its settings choose <label>." The labels are `qwen-oauth` "Qwen OAuth", `openai` "an OpenAI-compatible API", `openai-responses` "the OpenAI Responses API", `anthropic` "an Anthropic API", `gemini` "a Gemini API", `vertex-ai` "Vertex AI", and any other value as `` `<value>` ``.
  - Update: the native command is `qwen update`, with the same "Update Qwen Code" tab and running-tabs note as the other CLIs.
  - The change title of a settings edit is "Change Qwen Code settings".
  - Help lines:
    - `model.name`: "The model a new Qwen Code session starts with, by the name /model shows."
    - `tools.approvalMode`: "The approval mode Qwen Code starts in. Sessions Marshell starts pass their own mode, which wins."
    - `general.enableAutoUpdate`: "Qwen Code checks for a newer version when it starts and installs it after the session ends."
    - `privacy.usageStatisticsEnabled`: "Qwen Code sends usage statistics to its developers."
    - `ui.theme`: "The theme's name as /theme lists it, such as Qwen Dark."
    - `general.vimMode`: "Vim keys in Qwen Code's prompt."
  - A curated key the system file sets: the row is read-only, and names the file as Claude Code's managed rows do.
  - History's copy that names the importable CLIs reads "Claude Code, Codex and Qwen Code".

## Review focus

1. **`QWEN_HOME` set to `~\qwen`, a relative path, or blank.** Every Qwen path (settings, skill, chats root, Accounts) lands in the same folder Qwen itself uses. Tests in Task 2.
2. **A Qwen `settings.json` with comments, trailing commas, a BOM, CRLF and secrets** (`security.auth.apiKey`, `env`, `modelProviders`, an `mcpServers` entry's `headers`). Form edits keep everything else byte-equal. Raw shows secrets as "hidden" and stays read-only while one is shown. Raw saves the typed JSONC. Tests in Task 4.
3. **A Qwen chat file that is still being written, archived (`chats/archive/`), a sidecar (`<id>.runtime.json`), or a subagent file.** Only `projects/<slug>/chats/<uuid>.jsonl` is copied, and only once it has settled. A truncated last line parses as far as it goes. Tests in Task 6.
4. **A conversation linked by hooks and then copied by the importer.** It shows once in History, with its tab, its Resume by id, and the client "Marshell". Test in Task 6.
5. **Restarting a tab whose chat id the core knows, and one it doesn't.** The bar names `--resume <id>` and `--continue` respectively, and the text matches the argv the core then runs. If the command can't be fetched, the bar still opens with today's text. Tests in Task 8.

## Known limits (planned)

- `advanced.runtimeOutputDir` in Qwen's settings also moves `projects/`. The importer follows only `QWEN_RUNTIME_DIR` and the Qwen home.
- `QWEN_CODE_SYSTEM_SETTINGS_PATH` is not read (6l1 decision 3), so a moved system file isn't shown as the source of a read-only row.
- A distro's Qwen uses `<distro home>/.qwen`. A `QWEN_HOME` set inside the distro isn't followed, as for the other CLIs' overrides there.
- Claude Code's `removal_pointers` has the same empty-array shape as Qwen's. Only Qwen's is fixed here.

---

### Task 1: Real transcripts from Qwen against the fake model (main thread)

This task runs in the main thread, not through a worker. It runs Qwen headless from a plain shell, never in the app, with every variable pointing into a fresh literal scratch folder.

**Files:**
- Create: `crates/core/tests/fixtures/qwen/transcripts/0.26.5/*.jsonl`
- Modify: `scripts/scrub-fixtures.mjs` (`.jsonl` too), `docs/research/2026-11-03-qwen-code-surfaces.md` (a "Transcripts seen" section), and this plan where a finding differs.

- [ ] **Step 1: Set up the scratch.** Use a fresh literal folder `%TEMP%\qwen-6l2-t1`. Start `fake-openai.mjs` on a free loopback port. Write `<scratch>\home\.qwen\settings.json` with `security.auth.selectedType: "openai"`, `model.name: "marshell-fake"`, `general.enableAutoUpdate: false` and `tools.approvalMode: "yolo"`. Set these variables:
  - `USERPROFILE` and `HOME` to `<scratch>\home`, so `os.homedir()` moves too and Qwen reads no real `~/.agents/skills` or `~/.claude/settings.json`. `QWEN_HOME` stays unset.
  - `QWEN_CODE_SKIP_UPDATE_CHECK_ONCE=true`.
  - `OPENAI_API_KEY=x`, `OPENAI_BASE_URL=http://127.0.0.1:<port>/v1` and `OPENAI_MODEL=marshell-fake`.

  Use a project folder `<scratch>\proj`. If `qwen -p` writes no `chats\*.jsonl`, take the transcripts from an interactive Qwen tab in the dev app (`live-check.ps1 start -QwenFake`), as 6l1's spike did.
- [ ] **Step 2: Record.** Run `qwen -p` in `proj` once for each of these prompts:
  - "hello", for a text-only reply.
  - `RUN mkdir made`, `WRITE notes.txt`, `EDIT notes.txt` and `READ notes.txt`, for each tool.
  - A `RUN` of a command that fails, for an error result.
  - A `--resume <id>` of the first chat, to see whether it appends to the same file.

  Copy each `chats\*.jsonl`. Note:
  - the first line;
  - `provenance` on a headless run (does it tell headless from interactive? Compare with the 6l1 interactive transcript if one survives, else leave it for the live check);
  - whether a `custom_title` record appears;
  - how a tool call and its result are linked (ids in `parts`).
- [ ] **Step 3: Scrub and commit the fixtures.** `scripts/scrub-fixtures.mjs` rewrites only `.json` today, so extend its file filter to `.jsonl` (its replacements are plain text) and run `pnpm scrub:fixtures crates/core/tests/fixtures/qwen/transcripts`. The scrubber maps each UUID to a placeholder, so rename each fixture to its scrubbed `sessionId` (Task 6 requires the file name to match). Then check that `cargo test -p marshell-core --test fixtures_scrubbed` passes, and stop the fake server by PID. Ask before deleting the scratch folder.
- [ ] **Step 4: Commit:** `test(history): record Qwen Code 0.26.5 transcripts against the fake model`

### Task 2: Qwen's home, version, update, Accounts and the empty hooks array (core)

**Files:**
- Modify:
  - `crates/core/src/agents/qwen/mod.rs`: `home`, `parse_version`, `auth_label`
  - `crates/core/src/agents/qwen/hooks.rs`: `settings_path` uses `home`, and `removal_pointers` prunes an emptied or empty `PermissionRequest`
  - `crates/core/src/server/agents.rs`: `qwen_settings_path` uses `home`, `account_of`'s Qwen arm
  - `crates/core/src/agents/install.rs`: Qwen native update
- Test: inline tests in those files, and `crates/core/tests/agents_api.rs`

**Interfaces:**
- Produces:
  - `pub fn home(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf` in `agents::qwen`. Tasks 3, 4, 6 and 7 use it.
  - `pub fn auth_label(selected: &str) -> String` in `agents::qwen`.
  - `GET /v1/agents/qwen/account` answers `sign_in: "no_command"`, with `method: Some(label)` when the user file (or the system file, which wins) sets `security.auth.selectedType`. The system file is `C:\ProgramData\qwen-code\settings.json` only. `QWEN_CODE_SYSTEM_SETTINGS_PATH` is not read (6l1 decision 3, 6l2 decision 9). Tests pin the path with a new `AppState::with_qwen_system_settings(<path>)`, which Task 4 uses too.

- [ ] **Step 1: Write the failing tests.**
  - `qwen/mod.rs`:
    - `home_expands_a_leading_tilde`: `~\q` and `~/q` give `<home>\q`, and `~` alone gives `<home>`.
    - `home_ignores_a_blank_value_and_takes_others_as_given`
    - `a_preview_or_nightly_version_is_read`: `0.27.0-preview.1` and `0.27.0-nightly.20261101.abc` are `Some`, and the gate gives "not one Marshell has recorded yet".
    - `auth_labels`: each of the six labels in the global constraints, plus an unknown value in backticks.
  - `install.rs`: `qwen_standalone_updates_with_qwen_update` (`update_argv` gives `[<exe>, "update"]`, `update_text` gives `qwen update`). Replace today's None assertion.
  - `hooks.rs`:
    - `approvals_off_then_hooks_off_leaves_no_empty_permission_request`
    - `an_empty_permission_request_left_by_an_older_marshell_is_removed_with_the_hooks`. Remove the `is_none_or(Vec::is_empty)` tolerance from the test near line 930.
    - A person's own non-empty `PermissionRequest` group stays.
  - `agents_api.rs`: `qwen_account_names_the_configured_auth_type` and `qwen_account_without_a_selected_type_has_no_method`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 qwen`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `parse_version` accepts `x.y.z` optionally followed by `-<suffix>`, and returns the whole token. The gate compares only the numeric part.
  - Removal drops an event key whose array is empty after Marshell's groups go, and also one that was already empty. It drops it only when that key is `PermissionRequest` or Marshell had a group in it, so a person's own empty event list elsewhere is left alone.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `fix(agents): Qwen Code's home follows a ~ in QWEN_HOME, a preview version is read, it updates with qwen update and Accounts names its auth type`

### Task 3: The marshell skill for Qwen Code

**Files:**
- Modify:
  - `crates/core/src/agents/skill.rs`: `AGENTS` gains `("qwen", "Qwen Code")`, and `skill_path` gives `qwen::home(..)` (the folder under which `skills/marshell/SKILL.md` goes, as for the others)
  - `crates/core/src/server/agents.rs`: remove Qwen's `skill_problem` refusal and the `!matches!(adapter, Adapter::Qwen)` skip in the hooks plan
  - `crates/cli/src/uninstall.rs`: nothing beyond the table, but check the note near line 195
  - `src/features/agents/AgentsSection.tsx`: Qwen's skill row uses `useSkillFlow` like the others, in place of the "later version" note
  - `src/features/agents/fixtures.ts`
- Test: `skill.rs` inline, `crates/core/tests/agents_api.rs`, the `crates/cli` uninstall tests, `AgentsSection.test.tsx`

- [ ] **Step 1: Write the failing tests.**
  - `skill.rs`: `skill_paths_per_cli` adds `qwen` (`/h/.qwen`, and `QWEN_HOME=~/q` gives `/h/q`). `every_agent_in_the_table_has_a_skill_path_and_a_name` covers it.
  - `agents_api.rs`: `qwen_skill_plan_creates_the_file_under_the_qwen_home`, and `qwen_hooks_plan_with_skill_includes_it`.
  - `uninstall`: `uninstall_removes_qwens_skill`, and `an_edited_qwen_skill_is_left`.
  - `AgentsSection.test.tsx`: `Qwen's skill row offers Install the marshell skill`. The old "later version" test goes.
- [ ] **Step 2: Run.** `cargo test --workspace -j 4 skill` and `pnpm test src/features/agents`. Expected: FAIL.
- [ ] **Step 3: Implement** the arms.
- [ ] **Step 4: Run** both suites. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(agents): the marshell skill installs for Qwen Code in its own skills folder`

### Task 4: Qwen's settings view and edits (core)

**Files:**
- Modify:
  - `crates/core/src/agents/settings_view.rs`:
    - `curated("qwen")` and `control` give the six keys and controls from decision 5.
    - Qwen masks leaves only (decision 9), through a Qwen arm, not a change to `is_secret_key` or the shared walk the other CLIs use. It hides `security.auth.apiKey`, everything under `env` and `modelProviders`, `mcpServers.*.env`, `mcpServers.*.headers`, a hook's `env`, `telemetry` headers, and any string leaf whose own key passes `is_secret_key`. It walks into an object with a secret-looking name rather than hiding it whole, and never hides a number or boolean.
    - The system file is read as the managed source.
    - `editable` includes `qwen`.
    - `text_problem` keeps raw editing on for a Qwen file with comments or trailing commas (decision 6), and keeps the repeated-key refusal.
  - `crates/core/src/server/agents.rs`: remove both "doesn't change Qwen Code's settings yet" refusals, and fill the `Adapter::Qwen` arms of `settings_read`, plan and apply, with the change title from the global constraints. `raw_op` for Qwen checks that the text parses as JSONC, not as plain JSON.
- Test: `settings_view.rs` inline, `crates/core/tests/agents_api.rs`

**Interfaces:**
- Consumes: `qwen::home` and `AppState::with_qwen_system_settings` (Task 2).
- Produces: `GET /v1/agents/qwen/settings` answers `editable: true` with the six rows. `/settings/plan` and `/apply` work as Antigravity's do. No protocol change is expected. If one is needed, add it and regenerate.

- [ ] **Step 1: Write the failing tests.**
  - `settings_view.rs`:
    - `qwen_curated_rows_and_controls`: the six keys, with `tools.approvalMode`'s five choices in the decision's order.
    - `qwen_secrets_are_hidden`: each masked path shows "hidden" in the form and in raw.
    - `qwen_selected_type_is_shown_and_raw_stays_editable`: a file with only `security.auth.selectedType` and a comment.
    - `git_co_author_and_token_limits_are_not_hidden`: `general.gitCoAuthor.commit: true` and a numeric `…Tokens` key.
    - `claude_masking_is_unchanged`: one existing Claude masking assertion still holds, as a guard on the shared rule.
    - `qwen_system_file_makes_a_row_read_only`
    - `a_qwen_file_with_comments_stays_raw_editable`
    - `a_qwen_file_with_a_repeated_key_is_raw_read_only`
  - `agents_api.rs`:
    - `qwen_settings_form_edit_keeps_comments_bom_and_crlf`
    - `qwen_raw_edit_saves_the_typed_jsonc`
    - `qwen_raw_edit_that_does_not_parse_is_refused`
    - `qwen_raw_edit_is_refused_while_a_value_is_hidden`
    - `qwen_settings_undo_puts_the_file_back`
    - `qwen_settings_not_set_removes_the_key`
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 qwen_settings` and `cargo test -p marshell-core -j 4 settings_view`. Expected: FAIL.
- [ ] **Step 3: Implement.** The form's ops are the same `JsonOp`s Antigravity's use. The raw path saves the text as typed after `jsonc::parse_problem` finds no problem, so the BOM rule (6g decision 6) holds.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): Qwen Code's settings are edited through the config service, as JSONC`

### Task 5: Qwen's company tab and Accounts line (UI)

**Files:**
- Modify:
  - `src/features/settings/settingsSections.ts`: `CompanyAgent` gains `qwen`, and its tab shows once Qwen is detected.
  - `src/features/companies/*` (`CompanySection.tsx`, `companyText.ts`, `companyFields.ts`, `companyFlow.ts`): Qwen's name, the version row and the Update button with `qwen update`, and help lines from the core.
  - `src/features/accounts/AccountsSection.tsx` and `accountsText.ts`: Qwen in the list. A `no_command` status with a `method` shows the Accounts copy from the global constraints, and Antigravity's line is unchanged.
  - `src/features/agents/AgentsSection.tsx`: the version and update lines for Qwen, if that section shows them for the others (lines ~392-431).
- Test: the `.test.ts(x)` beside each file

- [ ] **Step 1: Write the failing tests.**
  - `settingsSections.test.ts`: `the Qwen Code tab shows once Qwen is detected`
  - `CompanySection.test.tsx`:
    - `Qwen's tab lists its six settings with help`
    - `Qwen's raw view is editable for a file with comments`
    - `Qwen's Update runs qwen update`
  - `accountsText.test.ts`:
    - `Qwen's line says it has no status command and how to sign in`
    - `Qwen's configured type is named`
    - `an unknown type shows in backticks`
- [ ] **Step 2: Run.** `pnpm test src/features`. Expected: FAIL.
- [ ] **Step 3: Implement.** Follow Antigravity's tab, and add no Antigravity-only running warning unless Task 9 shows that a running Qwen undoes edits.
- [ ] **Step 4: Run.** `pnpm test`, then `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(ui): Qwen Code gets its company tab, version and Update, and an Accounts line`

### Task 6a: The Qwen transcript reader

**Files:**
- Create: `crates/core/src/agents/qwen/transcript.rs`
- Modify: `crates/core/src/agents/qwen/mod.rs` (`pub mod transcript`) and `crates/core/src/history/mod.rs` (the `parse_for` arm)
- Test: `transcript.rs` inline, on Task 1's fixtures

**Interfaces:**
- Consumes: Task 1's fixtures.
- Produces:
  - `pub fn parse(text: &str, session_id: &str) -> Conversation` in `agents::qwen::transcript`.
  - `pub fn meta(text: &str) -> Option<QwenMeta>`, where `QwenMeta { id: String, started_ms: u64, cwd: String, branch: Option<String>, version: Option<String>, client: Client }`, taken from the first record that has a `sessionId`. Task 6b uses it.

- [ ] **Step 1: Write the failing tests.**
  - `transcript.rs`:
    - `a_text_reply_is_one_turn`
    - `each_tool_becomes_a_card` (shell, write, edit and read, with edit and write giving `EditedFile`)
    - `a_failed_tool_is_an_error_card`
    - `the_title_is_the_last_custom_title_else_the_first_prompt`
    - `a_chat_compression_record_is_a_compacted_item`
    - `a_truncated_last_line_is_ignored`
    - `model_and_version_come_from_the_records`
    - `limited_follows_the_version_gate`
    - `meta_reads_id_cwd_branch_and_start`
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 transcript`. Expected: FAIL.
- [ ] **Step 3: Implement.** The parser walks records in file order:
  - A `user` record whose parts hold text and no function response starts a turn.
  - `assistant` text parts become `Text`.
  - A function call plus its matching `tool_result` (or its function response part) becomes a `ToolCard`, named and summarised as Claude Code's are for the same kind of tool.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(history): Marshell reads Qwen Code's chat files into conversations`

### Task 6b: Qwen conversations in History, and the importer

**Files:**
- Modify:
  - `crates/core/src/history/import.rs`: `ImportRoots.qwen`, `Kind::Qwen`, `import_roots` (`QWEN_RUNTIME_DIR` when set and not blank, else `qwen::home`, then `projects`), `candidates`, `classify` and `prepare`
  - `crates/core/src/history/index.rs`: `filters()` excludes only `'antigravity'`
  - `crates/core/src/server/history.rs`: `AGENTS` gains `"qwen"`
  - `src/features/history/historyState.ts` (and any agent filter list in `HistoryView.tsx`): Qwen Code in the copy and the filter
- Test: `import.rs` inline (the `Rig` gains a Qwen root), `crates/core/tests/history_api.rs`, and `historyState.test.ts`

**Interfaces:**
- Consumes: `qwen::home` (Task 2), and `transcript::meta` and `parse` (Task 6a).

- [ ] **Step 1: Write the failing tests.**
  - `import.rs`:
    - `qwen_root_follows_qwen_runtime_dir_then_qwen_home`
    - `a_qwen_chat_is_copied_once_settled`
    - `qwen_archive_runtime_json_and_subagent_files_are_not_copied`
    - `a_qwen_chat_linked_by_hooks_keeps_its_tab_when_imported`
    - `a_deleted_qwen_copy_stays_deleted_until_the_chat_grows`
  - `index.rs`: the test near line 1365 now expects Qwen conversations listed, and Antigravity's still left out.
  - History route:
    - `get_conversation_reads_a_qwen_chat`
    - `a_qwen_row_from_6l1_with_no_copy_shows_as_history_shows_any_uncopied_row`: a row 6l1's hooks indexed whose source chat is gone (Qwen's `cleanupPeriodDays`). It behaves as an uncopied Codex row does today (listed or left out, and its open answer), never a crash or a blank page.
  - `historyState.test.ts`: the copy names Qwen Code.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 qwen` and `pnpm test src/features/history`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Client: `Marshell` when the copy is linked to a tab by hooks (the sidecar already exists). Otherwise use what Task 1 found tells headless from interactive (`Headless` or `Terminal`), or `Terminal` when nothing does.
  - The file name must be a UUID that matches its records' `sessionId`.
- [ ] **Step 4: Run.** `cargo test --workspace -j 4`, then `pnpm test`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(history): Qwen Code conversations show in History, and those run outside Marshell are imported`

### Task 7: Qwen Code inside WSL

**Files:**
- Modify:
  - `crates/core/src/server/agents.rs`: delete `QWEN_NO_WSL`. Give `distro_install` and `distro_remove` Qwen arms shaped like Antigravity's: `<distro home>/.qwen/settings.json`, the bridge by its Linux path, and no chain file, exactly as Antigravity passes `None`. `agent_version_in` covers Qwen.
  - `crates/core/src/config/guard.rs`: `DISTRO_AGENT_FOLDERS` gains `.qwen` (approved as slice 6l2 decision 7).
  - `src/features/agents/AgentsSection.tsx`: the distro panel offers "Set up in <distro>" for Qwen.
- Test: `guard.rs` inline (`a_distro_qwen_folder_is_admitted`, and a distro path outside `.qwen` still refused), `crates/core/tests/agents_api.rs` (`qwen_distro_setup_writes_the_linux_bridge_path` and `qwen_distro_remove_restores_the_file`, both with the pinned distro target), and `AgentsSection.test.tsx`.

- [ ] **Step 1: Write the failing tests** listed above, and delete the tests that expected the 422.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 distro` and `pnpm test src/features/agents`. Expected: FAIL.
- [ ] **Step 3: Implement** the arms.
- [ ] **Step 4: Run** both suites. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(wsl): Qwen Code's hooks are set up inside a WSL distro`

### Task 8: The restart and resume bars name the command the core will run

**Files:**
- Modify:
  - `crates/core/src/server/outside.rs` and `server/mod.rs`: `GET /v1/sessions/{tab}/resume-command` (`tab_resume_command`, 6h1's copy route) gains an optional query `?for=restart&mode=<mode>` (or `for=resume`). With `for`, it answers the bare command the restart or resume would run for that tab, in that mode, joined for display: no `cd` line, and the same `tab_plan` and `resume_args` the handlers use, so the two can't drift. Without `for`, it is unchanged. If `tab_plan` doesn't resolve the same target as the restart handler in `server/sessions.rs`, build the answer from the handler's own function, or add a route `/v1/sessions/{tab}/restart-command` that does.
  - `crates/protocol`: a response type only if the route's answer type doesn't already fit.
  - `src/features/sessions/sessionActions.ts`, `resumeCommand.ts` and `LiveApp.tsx`: opening the restart bar or the Resume bar fetches the command first. `restartBarText` (and the Resume bar's text) take the command as a parameter. On a failed fetch they fall back to today's `resumeCommand` text, so the bar always opens.
- Test: route tests in `crates/core/tests/restart.rs`:
  - `restart_command_names_resume_by_id_for_a_qwen_tab_with_a_chat`
  - `restart_command_continues_when_no_id_is_known`
  - `restart_command_for_codex_names_resume_by_id`
  - `restart_command_is_refused_without_the_bearer`

  - `resume_command_without_for_is_unchanged`, guarding 6h1's copy line.

  Plus, in `sessionActions.test.ts`:
  - `the restart bar shows the command it was given`
  - `the resume bar shows the command it was given`
  - `falls back to the local text`

- [ ] **Step 1: Write the failing tests.**
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 restart_command` and `pnpm test src/features/sessions`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `cargo test --workspace -j 4`, `pnpm check:generated`, `pnpm test` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `fix(sessions): the restart bar names the command the core will run, by id where it knows one`

### Task 9: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder and `-QwenFake`. Over CDP:
  - Accounts shows Qwen's line and "Its settings choose an OpenAI-compatible API."
  - The company tab shows the six rows. A form edit and a raw edit with a comment both go through Review, Apply and Undo, and the scratch file is byte-equal after the undo. While a Qwen tab runs, make a form edit, then change the theme with `/theme` in Qwen, and check whether Marshell's edit survives. If it doesn't, add Antigravity's running warning for Qwen in a fix commit.
  - The skill: install it from the Agents section. In a Qwen tab, `/skills` (or the skill listing) shows `marshell`. Then install Codex's skill (scratch `~/.agents`) and record whether Qwen lists it once or twice.
  - History: a Qwen tab's conversation appears with its turns. A `qwen -p` run from a plain shell with the same scratch env is imported, with the right client.
  - Restart a Qwen tab whose id is known. The bar names `--resume <id>`, and the argv matches.
  - The version row reads 0.26.5. Update is not clicked against the real install. If a scratch copy of the standalone install is put first on the app's PATH, Update may run there, and its outcome is recorded.
- [ ] **Step 2: WSL.** Install `@qwen-code/qwen-code@0.26.5` with npm into a `/tmp` scratch prefix with `HOME` there (using a scratch Node 22 tarball if the distro has none). Set it up with "Set up in <distro>", run it against the fake model, and check live status and a card. Turn the hooks off, and the distro file is byte-equal. Ask before deleting the `/tmp` scratch.
- [ ] **Step 3:** Write `docs/research/2026-11-03-qwen-surfaces-live-check.md`: each check with its evidence, and findings numbered.
- [ ] **Step 4:** Stop the check by PID, after listing the tree and looking for agent CLIs the person started. Ask before removing the scratch folder.
- [ ] **Step 5:** Commit `docs(research): record slice 6l2's live check`. Then the final whole-branch review (marshell-reviewer, Opus), one fix pass, and `marshell-ship`.
