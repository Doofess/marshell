# Slice 6n2: Copilot CLI's company tab, Accounts, update, skill, History and WSL

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Copilot CLI gets the rest of what Claude Code, Codex, Antigravity and Qwen Code have: an Accounts section with a Sign in button, its version with an Update button, the `marshell` skill, an editable company tab, its conversations in History (including those run outside Marshell), and hooks inside WSL distros. Three leftovers from 6n1 are closed too: the question dialog's status, a `COPILOT_HOME` outside the profile, and four deferred minors.

**Architecture:** Every surface already has a per-CLI table or match, and 6l2 gave Qwen an arm in each. Copilot gets an arm where `"qwen"` has one, and the "until slice 6n2" refusals are removed. `agents::copilot::home` already resolves `COPILOT_HOME`, and every Copilot path (settings, `config.json`, skill, `session-state`) goes through it. History follows Qwen's model: the hooks already link a session to its tab (6n1), a new `agents/copilot/transcript.rs` reads `events.jsonl` into a `Conversation`, and the importer copies `session-state/<id>/events.jsonl`. Masking reuses the leaves-only walk Qwen has, with Copilot's own list of secret places.

**Tech Stack:** Rust (axum, serde_json, `config::jsonc`, `notify`), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, "Slice 6n2 decisions (2026-11-20)", plus 6e decisions 8 to 16, 6f, 6g, 6h2, 6j1, 6l2 and 6n1 for the patterns they set. Evidence: `docs/research/2026-11-06-copilot-cli.md` (part 1 Q6 for the event log, part 2 for the surfaces). Task 1 adds real transcripts and settles the note's open items that this slice rests on.

**Checked before planning (2026-11-20, on `origin/main` a8eaac57):**
- 6n1 left these for the slice, all in `crates/core/src/server/agents.rs` unless named: `account_of`'s Copilot arm is `SignIn::NoCommand` with the comment "not read until slice 6n2" (~1322); `settings_plan_of` refuses with "Marshell doesn't change Copilot CLI's settings yet." (~1489); `copilot_setup_of` answers `skill_problem: "Marshell has no skill for Copilot CLI yet."` (~1160) and the hooks plan skips the skill with `!matches!(adapter, Adapter::Copilot)` (~1876); `distro_install` and `distro_remove` return `copilot_in_wsl()` (~514, ~527).
- `agents/install.rs`: `npm_package` has no Copilot, and `install_kind` has no native arm for it, so the exe in `%LOCALAPPDATA%\GitHubCopilotCLI` reads `Unknown` and gets no Update button.
- `agents/skill.rs`: `AGENTS` has four entries and `skill_path` has no Copilot arm.
- `agents/settings_view.rs`: `curated` and `control` have no Copilot arm, `mask_qwen` is the only leaves-only walk, `text_problem` has no Copilot arm, and `view` sets `editable` for four agents.
- History: `history/mod.rs::parse_for` has no Copilot arm, though `copilot_transcript_allowed` exists and refuses any distro; `history/index.rs:644` leaves `'copilot'` out of the lists; `server/history.rs:28` `AGENTS` has three entries; `history/import.rs` has no Copilot root or `Kind`.
- WSL: `config/guard.rs:21` `DISTRO_AGENT_FOLDERS` has no `.copilot`; `agents/distro.rs:14` `AGENTS` has four entries; `server/catalog.rs::wsl_words` gives Copilot no word, so the launcher answers `NotDrivenHere`; `server/hook.rs:132` marks a WSL tab's agent for four agents, by `session_id` and `SessionEnd`, which are not Copilot's names.
- `agents/copilot/status.rs::fold_event` maps only `permission_prompt` among the notification types.
- Frontend: `settingsSections.ts` (`SettingsSection`, `CompanyAgent`, `COMPANY`), `LiveSettings.tsx` (`ZERO`, the `case` per company), `companyFields.ts`, `companyText.ts` (`MANUAL_UPDATE`), `AccountsSection.tsx` (`ORDER`, `NAME`, the Sign in button typed `"claude" | "codex"`) and `accountsText.ts` have no Copilot.
- `includeCoAuthoredBy` and `storeTokenPlaintext` pass `is_secret_key` (they hold "auth" and "token"), so the shared `mask` would show both as "hidden".

## Global constraints

- Marshell writes, for Copilot: `<copilot home>\hooks\marshell.json` and `statusLine` in `<copilot home>\settings.json` as in 6n1, plus the six curated keys and raw edits of `settings.json` through the company tab, `<copilot home>\skills\marshell\SKILL.md`, and the hooks file, `statusLine` and nothing else inside a distro's `~/.copilot`. It never writes `config.json`, `mcp-config.json`, `providers.json`, `session-state`, a repo's `.github\`, a managed settings file or `~/.agents/skills` for Copilot.
- `<copilot home>` is `agents::copilot::home`: `COPILOT_HOME` when it is set and not blank, else `<user home>\.copilot`.
- Marshell reads from `config.json` only `lastLoggedInUser.login` and `lastLoggedInUser.host`. No other value of that file is kept, logged or sent.
- Every write goes through the config service: preview, unified diff, backup, atomic write, `managed.json`, undo. JSON edits are CST edits through `config::jsonc`. A file that doesn't parse fails closed.
- Tests and live checks never read or write the real `~/.copilot`, `~/.claude`, `~/.codex`, `~/.agents`, `~/.qwen` or `~/.marshell`. Tests inject paths with `AppState::with_copilot_home(<temp>)` and temp roots.
- Never run the real `copilot update` or `copilot login` against the real home. Never name a test binary `copilot`; use `copilot-marshell-test`.
- No new dependency. New protocol fields go in `crates/protocol`, `cargo test -p marshell-protocol` regenerates `src/generated/`, and `pnpm check:generated` passes afterwards.
- Never hold a `MutexGuard` across `.await` or file I/O. File reads and `wsl.exe` run in `spawn_blocking`.
- Copilot's conventions stay in `agents/copilot/` and Copilot arms: its camelCase event names and `sessionId` are read by Copilot code, never by a branch in another CLI's module.
- Every UI change works with the keyboard alone and in forced colours, with the existing classes.
- Rust tests run with `-j 4`. Typecheck with `pnpm exec tsc --noEmit`, never `tsc -b`.
- Copy, exactly:
  - Accounts, first line: "Copilot CLI has no sign-in status command, so Marshell can't show one."
  - Accounts, the account line: "Copilot's own file names <login> as the last account signed in." With a host other than `github.com` (compared without `https://` and a trailing `/`): "Copilot's own file names <login> on <host> as the last account signed in."
  - Accounts, the token line: "<NAME> is set, and Copilot CLI uses it before a stored sign-in."
  - The button: "Sign in to Copilot CLI". Its tab is named as the other sign-in tabs are.
  - Update: the native command is `copilot update`, with the same "Update Copilot CLI" tab and running-tabs note as the other CLIs.
  - The change title of a settings edit is "Change Copilot CLI settings" (it exists).
  - Help lines:
    - `model`: "The model a new Copilot CLI session starts with, by the name /model shows."
    - `defaultPermissionMode`: "The permission mode a Copilot CLI session starts in. A mode picked in Marshell's launcher wins."
    - `autoUpdate`: "Copilot CLI downloads a newer version by itself and starts it the next time it runs."
    - `autoUpdatesChannel`: "Which releases Copilot CLI updates to."
    - `theme`: "Copilot CLI's colour theme, by name, such as github."
    - `includeCoAuthoredBy`: "Copilot CLI adds a Co-authored-by line to the commits it makes."
  - History's copy that names the importable CLIs reads "Claude Code, Codex, Qwen Code and Copilot CLI".
  - The question state's phrase: "Has a question".

## Review focus

1. **A Copilot `settings.json` with comments, trailing commas, a BOM, CRLF, Marshell's own `statusLine` and secrets** (a hook's `env`, `proxyUrl` with a password in it, `sandbox.userPolicy.network.proxy.password`). Form edits keep everything else byte-equal. Raw shows the secrets as "hidden" and stays read-only while one is shown. `includeCoAuthoredBy` and `storeTokenPlaintext` stay visible. Tests in Task 4.
2. **A `config.json` that is missing, doesn't parse, starts with Copilot's `//` header, holds `lastLoggedInUser` in another shape (a string, null, an object without `login`, a login of 500 characters or with control characters), or holds tokens.** Accounts never fails, shows only a login and host that look like ones, and nothing else from the file reaches the answer. Tests in Task 2.
3. **An `events.jsonl` that is still being written, ends in a cut line, holds event types Marshell doesn't know, or was resumed (a `session.resume` appended to the same file); a `session-state` folder with a name that isn't a UUID, or without `events.jsonl`.** Only `session-state/<uuid>/events.jsonl` is copied, once it has settled, and an unknown event is skipped without losing the turns around it. Tests in Tasks 6a and 6b.
4. **A session linked by 6n1's hooks and then copied by the importer, and a row 6n1 indexed whose source is gone.** The first shows once in History, with its tab, its Resume by id and the client "Marshell". The second behaves as an uncopied Codex row does. Tests in Task 6b.
5. **`COPILOT_HOME` set to a drive root, a relative path, a path with `..`, Marshell's own home, a folder under an organization path, or a folder whose file is named `managed-settings.json`.** Each is still refused, and a folder beside the named one stays closed. Tests in Task 8.

## Known limits (planned)

- Copilot's managed settings aren't read (decision 10), so a row an organization sets isn't read-only.
- A conversation's title is its first prompt; a name given with `/rename` is in `workspace.yaml`, which isn't read (decision 7).
- A Copilot conversation run in a distro outside Marshell isn't imported, and a `COPILOT_HOME` set inside a distro isn't followed (decision 11).
- Accounts' account line is tested against a file built from Copilot's source, because this PC has no Copilot sign-in (decision 2). It says nothing about a custom provider (decision 14).
- Settings shows no "newer version available" for Copilot (decision 5).
- Esc in Copilot's question dialog fires no hook, so the row keeps Needs question until the next event (decision 13).

---

### Task 1: What Copilot 1.0.106 really does (main thread, with a decision gate)

This task runs in the main thread, not through a worker. Copilot runs headless from a plain shell, never in the app, with every variable pointing into the fresh literal scratch folder `%TEMP%\copilot-6n2-t1`. Nothing is deleted without asking.

**Files:**
- Create: `crates/core/tests/fixtures/copilot/transcripts/1.0.106/*.jsonl`, and `crates/core/tests/fixtures/copilot/hooks/1.0.106/notification-elicitation_dialog.json` if the dialog can be raised
- Modify: `docs/research/2026-11-06-copilot-cli.md` (a "What 6n2's first task saw" section), and this plan and PLAN §9 where a finding differs

- [ ] **Step 1: Set up the scratch.** Start `.claude/skills/marshell-live-check/fake-openai.mjs` on a free loopback port. Set `COPILOT_HOME=<scratch>\home\.copilot`, `USERPROFILE` and `HOME` to `<scratch>\home`, `COPILOT_PROVIDER_BASE_URL=http://127.0.0.1:<port>/v1`, `COPILOT_MODEL` to the fake's model, `COPILOT_AUTO_UPDATE=false`, and clear `WT_SESSION`, `WT_PROFILE_ID`, `COPILOT_GITHUB_TOKEN`, `GH_TOKEN` and `GITHUB_TOKEN`. Use the exe `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe`, never the shim, and a project folder `<scratch>\proj`.
- [ ] **Step 2: Record transcripts.** Run `copilot -p` in `proj` once for each of: "hello" (text only); a shell command; a file write, an edit and a read; a command that fails; and `--resume=<id>` of the first session. Copy each `session-state\<id>\events.jsonl`. Note the first line, what `session.start`'s `producer` holds on a headless run, how a tool call and its result are linked (`toolCallId`), whether a resume appends to the same file, and which fields the importer needs (`sessionId`, `startTime`, `context.cwd`, `context.branch`, `copilotVersion`). If `-p` writes no event log, record from an interactive tab in the dev app (`live-check.ps1 start -CopilotFake`), as 6n1's spike did.
- [ ] **Step 3: The settings file.** With `copilot help config` and `copilot help commands`, confirm the six keys, the values of `defaultPermissionMode` and `autoUpdatesChannel`, and whether `/theme` exists. Then, on the scratch `settings.json`: does Copilot start with a file that has a `//` comment, a trailing comma and a byte-order mark (three runs, one each), and does `copilot config` setting one key keep a comment and the key order? Record each answer.
- [ ] **Step 4: The question dialog.** If the fake model can call Copilot's ask-the-user tool (read `fake-openai.mjs` for its tool words; add a word there if that is all it takes), raise the dialog in an interactive run under the hook recorder 6n1 used and save the `notification` body. If it can't be raised, the mapping in Task 2 rests on the docs' `elicitation_dialog` name, and the Known limits say so.
- [ ] **Step 5: WSL.** In the PC's distro, install `@github/copilot@1.0.106` with npm into the scratch prefix `/tmp/copilot-6n2-t1` with `HOME` and `COPILOT_HOME` there (a scratch Node tarball in the same folder if the distro's Node is too old). Write a `hooks/marshell.json` there by hand whose `exec` is a Windows exe by its `/mnt/c/...` path that appends its arguments and stdin to a scratch file, run one prompt against the fake model (reached through the distro's route to the Windows loopback, as 6l2's live check did), and record: whether the hook ran, the body it got, the `transcriptPath` it names, and whether a status line command runs. Ask before deleting the `/tmp` scratch.
- [ ] **Step 6: Scrub and commit the fixtures.** `pnpm scrub:fixtures crates/core/tests/fixtures/copilot/transcripts`, rename each fixture to its scrubbed `sessionId`, and check `cargo test -p marshell-core -j 4 --test fixtures_scrubbed` passes. Stop the fake server by PID.
- [ ] **Step 7: Decision gate.** Write the findings into the research note. Put to the user, through AskUserQuestion: whether `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `QWEN_HOME` outside the profile get the same guard rule as `COPILOT_HOME` (decision 12), and anything Steps 2 to 5 found that changes a decision (hooks that can't reach the bridge from a distro, a settings file Copilot rewrites without its comments, a `producer` that can't tell headless from interactive). Record the answers in PLAN §9 and change the tasks below to match.
- [ ] **Step 8: Commit:** `test(history): record Copilot CLI 1.0.106 event logs against the fake model`

### Task 2: Accounts, Sign in, Update and the question dialog (core)

**Files:**
- Modify:
  - `crates/protocol/src/account.rs`: `AccountStatus` gains `last_account: Option<String>` and `token_env: Option<String>`
  - `crates/core/src/agents/copilot/mod.rs`: `last_account`, `token_env`
  - `crates/core/src/server/agents.rs`: `account_of`'s Copilot arm
  - `crates/core/src/server/sessions.rs`: the sign-in argv near line 379 gains Copilot, `[<exe>, "login"]`
  - `crates/core/src/agents/install.rs`: `npm_package("copilot")` is `@github/copilot`; the native arm is `copilot.exe` in a folder named `GitHubCopilotCLI`
  - `crates/core/src/agents/copilot/status.rs`: `elicitation_dialog`
  - every other place that builds an `AccountStatus` (the compiler names them)
- Test: inline tests in those files, `crates/core/tests/agents_api.rs`, `crates/core/tests/cli_task.rs`

**Interfaces:**
- Produces:
  - `pub fn last_account(config_text: &str) -> Option<String>` in `agents::copilot`: "<login>" or "<login> on <host>", from `lastLoggedInUser` of a JSONC text. A login is kept only when it is 1 to 100 characters of ASCII letters, digits, `-` and `_`; a host only when it is 1 to 253 characters of ASCII letters, digits, `.`, `-` and `:` after `https://` or `http://` and a trailing `/` are taken off. A host of `github.com` is left out.
  - `pub fn token_env(env: &dyn Fn(&str) -> Option<String>) -> Option<&'static str>`: the first of `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN` that is set and not blank. The value is never returned.
  - `GET /v1/agents/copilot/account` answers `sign_in: "no_command"` with those two fields. `account_of` reads the environment through `st.copilot_env`, so a test that names a home reads none.

- [ ] **Step 1: Write the failing tests.**
  - `copilot/mod.rs`:
    - `last_account_reads_login_and_host`: `{"lastLoggedInUser":{"host":"https://github.com","login":"octo"}}` gives `octo`, and host `https://ghe.example.com/` gives `octo on ghe.example.com`.
    - `last_account_reads_through_copilots_comment_header`: the text starts with two `//` lines.
    - `last_account_is_none_for_other_shapes`: a string, null, an object without `login`, a login of 500 characters, a login holding `\u001b`, a file that doesn't parse, and an empty text.
    - `nothing_else_of_the_file_is_returned`: a file with `copilotTokens` and `authTokens` beside the user gives only `octo`.
    - `token_env_names_the_first_set_variable_and_skips_blank_ones`
  - `install.rs`: `copilot_in_its_own_folder_updates_with_copilot_update` (`C:\Users\u\AppData\Local\GitHubCopilotCLI\copilot.exe` is `Native`, `update_argv` gives `[<exe>, "update"]`, `update_text` gives `copilot update`), `an_npm_copilot_updates_with_npm` (`npm install -g @github/copilot@latest`), and `a_copilot_elsewhere_has_no_update` (`Unknown`).
  - `status.rs`: `an_elicitation_dialog_needs_a_question` (state `NeedsQuestion`, phrase "Has a question"), `a_tool_result_after_a_question_is_working`, `a_prompt_after_a_question_is_working`. With Task 1's recorded body when there is one, else the permission prompt's body with the type changed.
  - `agents_api.rs`: `copilot_account_names_the_last_account`, `copilot_account_without_a_config_file_has_no_account`, `copilot_account_with_a_named_home_reads_no_token_variable`.
  - `cli_task.rs` (or the inline tests near `sessions.rs:1656`): `a_copilot_sign_in_tab_runs_copilot_login`, with a `copilot-marshell-test.cmd` shim, and that tab is not recorded.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 copilot`. Expected: FAIL (the protocol fields don't exist).
- [ ] **Step 3: Implement.** `fold_event`'s notification arm maps `elicitation_dialog` to `(LiveState::NeedsQuestion, "Has a question")`; the `postToolUse` arm already turns a non-Working state to "Thinking". `config.json` is read with `std::fs::read_to_string` on the blocking thread `account` already uses.
- [ ] **Step 4: Run.** `cargo test -p marshell-protocol -j 4`, `cargo test -p marshell-core -j 4`, `pnpm check:generated`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): Copilot CLI's Accounts names its last account and offers Sign in, it updates with copilot update, and its question dialog reads Needs question`

### Task 3: The marshell skill for Copilot CLI

**Files:**
- Modify:
  - `crates/core/src/agents/skill.rs`: `AGENTS` gains `("copilot", "Copilot CLI")` (five entries), and `skill_path` gives `copilot::home(env, user_home)` as the root
  - `crates/core/src/server/agents.rs`: `skill_file` follows a home named with `with_copilot_home`, as it does Qwen's; `copilot_setup_of` uses `st.skill_of(COPILOT)` in place of the fixed `Unavailable`; the `!matches!(adapter, Adapter::Copilot)` skip in the hooks plan goes; the doc comment near 1120 loses "no skill until slice 6n2"
  - `crates/cli/src/uninstall.rs`: nothing beyond the table, but check the notes near lines 261 and 339
  - `src/features/agents/AgentsSection.tsx` and `fixtures.ts`: Copilot's skill row uses `useSkillFlow` like the others
- Test: `skill.rs` inline, `crates/core/tests/agents_api.rs`, the `crates/cli` uninstall tests, `AgentsSection.test.tsx`

- [ ] **Step 1: Write the failing tests.**
  - `skill.rs`: `skill_paths_per_cli` adds `copilot` (`/h/.copilot/skills/marshell/SKILL.md`, and `COPILOT_HOME=/c` gives `/c/skills/marshell/SKILL.md`). `every_agent_in_the_table_has_a_skill_path_and_a_name` covers it.
  - `agents_api.rs`: `copilot_skill_plan_creates_the_file_under_the_copilot_home`, `copilot_hooks_plan_with_skill_includes_it`.
  - `uninstall`: `uninstall_removes_copilots_skill`, `an_edited_copilot_skill_is_left`.
  - `AgentsSection.test.tsx`: `Copilot's skill row offers Install the marshell skill`. The test that expected "Marshell has no skill for Copilot CLI yet." goes.
- [ ] **Step 2: Run.** `cargo test --workspace -j 4 skill` and `pnpm test src/features/agents`. Expected: FAIL.
- [ ] **Step 3: Implement** the arms.
- [ ] **Step 4: Run** both suites. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): the marshell skill installs for Copilot CLI in its own skills folder`

### Task 4: Copilot's settings view and edits (core)

**Files:**
- Modify:
  - `crates/core/src/agents/settings_view.rs`:
    - `curated("copilot")` gives the six keys in decision 3's order, and `control` gives `model` and `theme` Text, `defaultPermissionMode` the choice `manual`, `assisted`, `allow-all`, `autoUpdatesChannel` the choice `stable`, `prerelease`, and `autoUpdate` and `includeCoAuthoredBy` Switch.
    - `mask_qwen`'s walk becomes `fn mask_leaves(value: &mut Value, container: &dyn Fn(&[String], &str) -> bool)`, where `container(path, key)` says whether everything under `key` at `path` is hidden. `mask_qwen` keeps its own predicate unchanged. A new `mask_copilot` passes Copilot's: a key named `env` or `headers` in any case, `credentials` under `sandbox`, a key under `telemetry` whose lower-cased name holds `headers`, and it hides the string at the top-level key `proxyUrl`. Both hide a string leaf whose own key passes `is_secret_key`, and never a number or boolean.
    - `parse_masked` and `view` use `mask_copilot` for `"copilot"`, the `NotPlainJson` lock is skipped for it as for Qwen, and `editable` includes it. No managed file is read for it (decision 10).
    - `text_problem("copilot", text)`: one leading BOM is taken off (unless Task 1 showed Copilot refuses one, in which case a BOM is a problem on line 1), then `jsonc::parse_problem` must find none and `read_value` must give an object; an empty text is line 1.
  - `crates/core/src/server/agents.rs`: the "doesn't change Copilot CLI's settings yet" refusal goes, and `raw_op` checks Copilot's text with `text_problem`.
- Test: `settings_view.rs` inline, `crates/core/tests/agents_api.rs`

**Interfaces:**
- Produces: `GET /v1/agents/copilot/settings` answers `editable: true` with the six rows. `/settings/plan` and `/apply` work as Qwen's do. No protocol change is expected.

- [ ] **Step 1: Write the failing tests.**
  - `settings_view.rs`:
    - `copilot_curated_rows_and_controls`
    - `copilot_secrets_are_hidden`: a hook entry's `env` value, an MCP-style `headers` value, `sandbox.credentials.envVars`, `telemetry.headersProvider`, `proxyUrl` and `sandbox.userPolicy.network.proxy.password` each show "hidden" in the form and in raw.
    - `co_authored_by_and_store_token_plaintext_are_not_hidden`: both booleans show, and a file holding only them and a comment keeps raw editable.
    - `qwen_masking_is_unchanged` and `claude_masking_is_unchanged`: one existing assertion each, as guards on the refactor.
    - `a_copilot_file_with_comments_and_a_trailing_comma_stays_raw_editable`
    - `a_copilot_file_with_a_repeated_key_is_raw_read_only`
    - `copilot_text_problem_names_the_line`, and `an_array_at_the_top_is_a_problem`
  - `agents_api.rs`:
    - `copilot_settings_form_edit_keeps_comments_bom_crlf_and_the_status_line`: the file holds Marshell's `statusLine`, a comment, a trailing comma and CRLF; after `model` is set, every other byte is the same.
    - `copilot_raw_edit_saves_the_typed_jsonc`
    - `copilot_raw_edit_that_does_not_parse_is_refused`
    - `copilot_raw_edit_is_refused_while_a_value_is_hidden`
    - `copilot_settings_undo_puts_the_file_back`
    - `copilot_settings_not_set_removes_the_key`
    - `copilot_settings_plan_is_409_when_the_file_changed_since_the_form_opened`
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 copilot_settings` and `cargo test -p marshell-core -j 4 settings_view`. Expected: FAIL.
- [ ] **Step 3: Implement.** The form's ops are the `JsonOp`s Qwen's use.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(agents): Copilot CLI's settings are edited through the config service, as JSONC`

### Task 5: Copilot's company tab and Accounts section (UI)

**Files:**
- Modify:
  - `src/features/settings/settingsSections.ts`: `SettingsSection`, `CompanyAgent` and `COMPANY` gain `copilot`, after `qwen`; `LiveSettings.tsx`: `ZERO` and a `case "copilot"`
  - `src/features/companies/companyFields.ts` (the six keys with the help lines above), `companyText.ts` (`MANUAL_UPDATE.copilot`: `copilot update`, and the npm line `npm install -g @github/copilot@latest`)
  - `src/features/accounts/AccountsSection.tsx`: `ORDER` and `NAME` gain Copilot ("Copilot CLI"); `onSignIn` takes `"claude" | "codex" | "copilot"`; Copilot's button shows whenever its status is `no_command`
  - `src/features/accounts/accountsText.ts`: `accountExtraLines(s: AccountStatus): string[]` replaces `accountExtraLine` (Qwen's one line, Copilot's account line then its token line)
  - the caller that starts a sign-in tab (`onSignIn` in `LiveSettings.tsx` or `LiveApp.tsx`) and its API type in `src/lib/api`
- Test: the `.test.ts(x)` beside each file, and `SettingsShell.test.tsx`

- [ ] **Step 1: Write the failing tests.**
  - `SettingsShell.test.tsx` / `settingsSections.test.ts`: `the Copilot CLI tab shows once Copilot is detected`, and the section list with five companies.
  - `CompanySection.test.tsx`: `Copilot's tab lists its six settings with help`, `Copilot's raw view is editable for a file with comments`, `Copilot's Update runs copilot update`.
  - `accountsText.test.ts`: `Copilot's line says it has no status command`, `Copilot's last account is named`, `a set token variable is named after the account`, `Qwen's extra line is unchanged`.
  - `AccountsSection.test.tsx`: `Copilot gets a Sign in button though its status is never known`, `Antigravity and Qwen still get none`.
- [ ] **Step 2: Run.** `pnpm test src/features`. Expected: FAIL.
- [ ] **Step 3: Implement.** Follow Qwen's tab. Add no running warning unless Task 10's live check shows a running Copilot undoes edits.
- [ ] **Step 4: Run.** `pnpm test`, then `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(ui): Copilot CLI gets its company tab, version and Update, and an Accounts section with Sign in`

### Task 6a: The Copilot event-log reader

**Files:**
- Create: `crates/core/src/agents/copilot/transcript.rs`
- Modify: `crates/core/src/agents/copilot/mod.rs` (`pub mod transcript`) and `crates/core/src/history/mod.rs` (the `parse_for` arm)
- Test: `transcript.rs` inline, on Task 1's fixtures

**Interfaces:**
- Consumes: Task 1's fixtures.
- Produces:
  - `pub fn parse(text: &str, session_id: &str) -> Conversation` in `agents::copilot::transcript`.
  - `pub fn meta(text: &str) -> Option<CopilotMeta>`, where `CopilotMeta { id: String, started_ms: u64, cwd: String, branch: Option<String>, version: Option<String>, client: Client }`, from the first `session.start` event. Task 6b uses it.

- [ ] **Step 1: Write the failing tests.**
  - `a_text_reply_is_one_turn`
  - `each_tool_becomes_a_card` (shell, write, edit and read, with edit and write giving `EditedFile`)
  - `a_failed_tool_is_an_error_card` (`tool.execution_complete` with `success: false`)
  - `the_title_is_the_first_prompt`
  - `a_compaction_event_is_a_compacted_item`
  - `a_resumed_session_reads_on_as_one_conversation`
  - `an_unknown_event_type_is_skipped_and_the_turns_around_it_stay`
  - `a_truncated_last_line_is_ignored`
  - `model_and_version_come_from_the_events`
  - `limited_follows_the_version_gate`
  - `meta_reads_id_cwd_branch_and_start`
  - `meta_is_none_without_a_session_start`
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 transcript`. Expected: FAIL.
- [ ] **Step 3: Implement.** The parser walks events in file order:
  - `user.message` starts a turn with its `content` (not `transformedContent`, which holds Marshell's reference note). One with `isAutopilotContinuation` true stays in the turn before it.
  - `assistant.message` `content` becomes `Text`.
  - `tool.execution_start` and the `tool.execution_complete` with the same `toolCallId` become one `ToolCard`, named and summarised as Claude Code's are for the same kind of tool. A start with no completion is a card with no result.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(history): Marshell reads Copilot CLI's event logs into conversations`

### Task 6b: Copilot conversations in History, and the importer

**Files:**
- Modify:
  - `crates/core/src/history/import.rs`: `ImportRoots.copilot` (`<copilot home>/session-state`), `Kind::Copilot`, `import_roots`, `candidates` (`<root>/<uuid>/events.jsonl`, one level, links not followed), `classify` and `prepare`; the module doc names four CLIs
  - `crates/core/src/history/index.rs:644`: only `'antigravity'` is left out
  - `crates/core/src/server/history.rs`: `AGENTS` gains `"copilot"`, and the Resume refusal text near line 506 covers Copilot where it names CLIs
  - `src/features/history/historyState.ts` (and any agent filter list in `HistoryView.tsx`): Copilot CLI in the copy and the filter
- Test: `import.rs` inline (the `Rig` gains a Copilot root), `crates/core/tests/history_api.rs`, `historyState.test.ts`

**Interfaces:**
- Consumes: `copilot::home`, and `transcript::meta` and `parse` (Task 6a).

- [ ] **Step 1: Write the failing tests.**
  - `import.rs`:
    - `copilot_root_follows_copilot_home`
    - `a_copilot_event_log_is_copied_once_settled`
    - `a_session_folder_with_another_name_or_without_an_event_log_is_not_copied` (a folder `not-a-uuid`, a UUID folder holding only `workspace.yaml`, and `events.jsonl` one level deeper under `checkpoints`)
    - `a_copilot_log_whose_session_id_differs_from_its_folder_is_not_copied`
    - `a_copilot_session_linked_by_hooks_keeps_its_tab_when_imported`
    - `a_deleted_copilot_copy_stays_deleted_until_the_log_grows`
  - `index.rs`: the test near the `NOT IN` list now expects Copilot conversations listed and Antigravity's left out.
  - `history_api.rs`: `get_conversation_reads_a_copilot_session`, and `a_copilot_row_from_6n1_with_no_copy_shows_as_history_shows_any_uncopied_row`.
  - `historyState.test.ts`: the copy names Copilot CLI.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 copilot` and `pnpm test src/features/history`. Expected: FAIL.
- [ ] **Step 3: Implement.** Client: `Marshell` when the copy is linked to a tab by hooks. Otherwise what Task 1 found tells headless from interactive (`Headless` or `Terminal`), or `Terminal` when nothing does.
- [ ] **Step 4: Run.** `cargo test --workspace -j 4`, then `pnpm test`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(history): Copilot CLI conversations show in History, and those run outside Marshell are imported`

### Task 7: Copilot CLI inside WSL

Changed at Task 1's gate if Step 5 there showed the hooks can't reach the bridge.

**Files:**
- Modify:
  - `crates/core/src/server/agents.rs`: `copilot_in_wsl` goes. `distro_install` gets a Copilot arm: `copilot_hooks::install_plan` over `<distro home>/.copilot/hooks/marshell.json` and `<distro home>/.copilot/settings.json`, with the distro's `Bridge` and no chain file. `distro_remove`'s arm names `COPILOT`. `agent_version_in` covers Copilot.
  - `crates/core/src/agents/distro.rs`: `AGENTS` gains `"copilot"`, and `remove_plan_of` gets a `"copilot"` arm (the catch-all `_` arm becomes `"qwen"`, with an unknown agent an error)
  - `crates/core/src/config/guard.rs`: `DISTRO_AGENT_FOLDERS` gains `.copilot` (five entries; approved as slice 6n2 decision 11), and the doc comments that list the folders
  - `crates/core/src/server/catalog.rs::wsl_words`: Copilot gets its word, `copilot`
  - `crates/core/src/server/hook.rs`: the WSL mark near line 132 covers Copilot. Its session id and its end event come from two functions in `agents::copilot`, `session_id_of(body: &Value) -> Option<&str>` (`sessionId`) and `ends_session(body: &Value) -> bool` (`sessionEnd`), so no Copilot name is spelt in the shared block. For a WSL tab the transcript is the body's own `transcriptPath`, translated to the distro's share (6j1 decision 7), not a path built from the Windows Copilot home.
  - `crates/core/src/history/mod.rs::copilot_transcript_allowed`: a distro is accepted, as `qwen_transcript_allowed` does
  - `crates/cli/src/uninstall.rs`: the distro pass covers Copilot through `distro::remove_plans`
  - `src/features/agents/AgentsSection.tsx`: the distro panel offers "Set up in <distro>" for Copilot; `src/features/launcher/shellAgents.ts` and its tests: Copilot is no longer `not_driven_here` in a distro
- Test: `guard.rs` inline, `distro.rs` inline, `crates/core/tests/agents_api.rs`, `crates/core/tests/hook.rs`, `crates/core/tests/catalog_api.rs` or `server/profiles.rs` inline, `AgentsSection.test.tsx`, `shellAgents.test.ts`, `Launcher.test.tsx`

- [ ] **Step 1: Write the failing tests,** and delete those that expected the 422 or `NotDrivenHere`.
  - `guard.rs`: `a_distro_copilot_folder_is_admitted`, and a distro path outside the five folders still refused.
  - `agents_api.rs`: `copilot_distro_setup_writes_the_linux_bridge_path` (the hooks file's `exec` is the `/mnt/c/...` path, and `statusLine.command` holds no chain), `copilot_distro_remove_restores_the_files`, both with the pinned distro target.
  - `hook.rs`: `a_copilot_hook_from_a_wsl_tab_marks_the_tab`, `a_copilot_session_end_from_a_wsl_tab_clears_the_mark`, `a_wsl_copilot_session_links_its_distro_transcript`.
  - `history/mod.rs`: `a_copilot_transcript_under_the_tabs_distro_share_is_allowed`, and one under another distro's share refused.
  - `profiles.rs`: the expected problem for Copilot in a distro whose PATH has it is `None`, and `NotInstalled` when it hasn't.
  - Frontend: `Copilot is offered for a distro that has it`, `the distro panel offers Set up in Ubuntu for Copilot CLI`.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 distro`, `cargo test -p marshell-core -j 4 wsl`, `pnpm test src/features/agents src/features/launcher`. Expected: FAIL.
- [ ] **Step 3: Implement** the arms.
- [ ] **Step 4: Run.** `cargo test --workspace -j 4`, `pnpm test`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(wsl): Copilot CLI's hooks are set up inside a WSL distro, and the launcher offers it there`

### Task 8: A `COPILOT_HOME` outside the profile (the config guard)

Worker effort: high. `guard.rs` is safety-critical; the change is the one decision 12 describes and nothing else.

**Files:**
- Modify: `crates/core/src/config/guard.rs`
- Test: `guard.rs` inline, `crates/core/tests/agents_api.rs`

**Interfaces:**
- Produces:
  - `pub fn moved_agent_homes(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf>`: Copilot's folder when `COPILOT_HOME` is set, not blank, absolute, free of `..` and not a drive root (`projects::is_root` of its canonical form, as `for_home` checks a project); otherwise empty. If Task 1's gate widened decision 12, the other three variables are read the same way here.
  - `Guard::for_home` adds `moved_agent_homes(&|k| std::env::var(k).ok())` to `allowed`. `Guard::check`'s order is unchanged: Marshell's own home, then the denied folders and managed names, then `allowed`.

- [ ] **Step 1: Write the failing tests.**
  - `a_copilot_home_outside_the_profile_is_admitted`: `Guard::new` with the folder `moved_agent_homes` gives for `D:\work\copilot-home` admits `…\settings.json`, `…\hooks\marshell.json` and `…\skills\marshell\SKILL.md`.
  - `a_folder_beside_the_copilot_home_stays_closed`: `D:\work\other\x.json` and `D:\work\copilot-home-2\x.json` are `OutsideAllowed`.
  - `moved_agent_homes_refuses_a_root_a_relative_path_a_dot_dot_and_a_blank`: `D:\`, `copilot-home`, `D:\a\..\b` and `"  "` each give nothing.
  - `a_copilot_home_inside_marshells_home_is_still_marshells_own`, `a_copilot_home_under_a_denied_folder_is_still_managed`, `a_managed_file_name_in_the_copilot_home_is_still_refused` (`managed-settings.json`).
  - `agents_api.rs`: `copilot_hooks_plan_applies_in_a_home_outside_the_profile`, with the service's guard built from `Guard::new` and that folder.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 guard`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(config): a COPILOT_HOME outside the user profile is open to Copilot CLI's files`

### Task 9: 6n1's deferred minors

Each is checked against the code first. One that the code doesn't confirm gets no change, and this task's commit message and the PR's Known limits say why.

**Files:**
- Modify (as each check confirms):
  - `/clear` announcing "has ended": `crates/core/src/server/hook.rs` (`copilot_session`) or the row's announcer in `src/features/sessions`. A Copilot `sessionEnd` with reason `user_exit` is held for 500 ms before the row reads Ended, and is dropped when a status line names a newer session in that time. Tests: `a_clear_never_shows_ended` and `an_exit_shows_ended_after_the_hold` in `crates/core/tests/hook.rs`.
  - The consent card's "Approve from anywhere needs live status…" line: the consent card in `src/features/agents` (or `onboarding`) leaves it out for an agent whose approve switch is locked off. Test: `Copilot's card has no approve-needs-status line`, and Claude Code's still has it.
  - A PATH that holds only the VS Code shim: `agents::copilot::exe::resolve` already answers `Missing`; the Agents section and the launcher show its reason for Copilot, "Only VS Code's Copilot launcher is on PATH. Install Copilot CLI itself to use it here." Tests in `exe.rs` and `AgentsSection.test.tsx`.
  - `is_marshells` for empty hook lists: `crates/core/src/agents/copilot/hooks.rs`. A file whose every hook list is empty is Marshell's only when its text is the one `hooks_file` writes for `status: false`. Tests: `an_empty_hooks_file_someone_else_wrote_is_not_marshells`, `marshells_own_empty_file_still_is`.
- Test: as named above

- [ ] **Step 1: Check each minor** against the code and write down, in the ledger, confirmed or not and the lines that show it.
- [ ] **Step 2: Write the failing test** for each confirmed one.
- [ ] **Step 3: Run.** `cargo test -p marshell-core -j 4 copilot` and `pnpm test src/features/agents`. Expected: FAIL for each confirmed one.
- [ ] **Step 4: Implement.**
- [ ] **Step 5: Run.** `cargo test --workspace -j 4`, `pnpm test`, `pnpm build`. Expected: PASS.
- [ ] **Step 6: Commit.** `cargo fmt --all`, then `fix(agents): Copilot CLI's /clear no longer reads as ended, a shim-only PATH says so, and the consent card and an empty hooks file read right`, with the line trimmed to what was confirmed.

### Task 10: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder and `-CopilotFake`. Over CDP:
  - Accounts shows Copilot's first line and the Sign in button. With a scratch `config.json` naming a user, the account line shows; with `GH_TOKEN` set for the app, the token line shows. Sign in opens a tab that runs `copilot login` in the scratch home, and the tab is closed without signing in.
  - The company tab shows the six rows. A form edit and a raw edit with a comment and a trailing comma both go through Review, Apply and Undo, and the scratch file is byte-equal after the undo. While a Copilot tab runs, make a form edit, then change the model with `/model` in Copilot, and check whether Marshell's edit survives. If it doesn't, add Antigravity's running warning for Copilot in a fix commit.
  - The skill: install it from the Agents section. `copilot skill list --json` in the scratch home lists `marshell`. Then install Codex's skill (scratch `~/.agents`) and record whether Copilot lists it once or twice.
  - History: a Copilot tab's conversation appears with its turns and tool cards. A `copilot -p` run from a plain shell with the same scratch variables is imported, with the right client. After `/clear`, the new session is a second conversation.
  - The question dialog, if Task 1 could raise it: the row reads Needs question, and Working after an answer.
  - The version row reads 1.0.106. Update is not clicked against the real install.
  - The Settings nav, the company tab and Accounts with the keyboard alone, and in emulated forced colours.
- [ ] **Step 2: WSL.** Install `@github/copilot@1.0.106` into a `/tmp` scratch prefix as Task 1 did. The launcher offers Copilot for the distro. "Set up in <distro>" writes the two files, a Copilot tab there shows live status and its conversation in History, and Hooks off leaves the distro's files byte-equal. Ask before deleting the `/tmp` scratch.
- [ ] **Step 3: A `COPILOT_HOME` outside the profile.** Start the app with `COPILOT_HOME` on a scratch folder outside the profile (ask the user for a place on another drive, or `C:\copilot-6n2-scratch`), set the hooks up, and check that the files land there and that Hooks off removes them.
- [ ] **Step 4:** Write `docs/research/2026-11-20-copilot-surfaces-live-check.md`: each check with its evidence, and findings numbered.
- [ ] **Step 5:** Stop the check by PID, after listing the tree and looking for agent CLIs the person started. Ask before removing the scratch folders.
- [ ] **Step 6:** Commit `docs(research): record slice 6n2's live check`. Then the final whole-branch review (marshell-reviewer, Opus), one fix pass, and `marshell-ship`.
