# Slice 6e2: Accounts, CLI version and update, and the read-only company tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Settings gains Accounts (sign-in status per CLI, with a Sign in tab), and one read-only section per detected company CLI (Claude Code, Codex, Antigravity) that opens with its version and an Update button.

**Architecture:** The core owns every CLI-specific decision: it runs the status commands, reads the install type from the found executable's path, builds the sign-in and update argv, and reads each CLI's user settings file into masked values. The frontend asks for those through four new token-guarded routes and one new `CreateSessionRequest` field, and never builds a command line itself. The new sections plug into the 6e1 Settings page.

**Tech Stack:** Rust (axum, serde_json, jsonc-parser, toml 1.1, ts-rs), React 19 + TypeScript, Vitest with `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, "Slice 6e decisions (2026-10-26)", decisions 1, 7 to 16 (lines 1198 to 1225), with the "Accounts" and "Company tabs" rules at lines 1104 to 1106 and slice 6a decision 9 (line 1127). Research: `docs/research/2026-10-16-accounts-cloud-extensions.md` (status commands, settings surfaces, key groups) and `docs/research/2026-10-15-local-install-check.md` (install paths, `version.json`).

## Global Constraints

- The frontend talks only to the core API, never to a CLI directly. The frontend never sends an argv for a sign-in or update tab; the core builds it (`.claude/rules/ui.md`).
- New routes sit in the token-guarded router and each gets a "refused without the bearer" test. No route takes a file path. Blocking reads and process runs go in `spawn_blocking`. New request and response types go in `crates/protocol`; `pnpm check:generated` passes (`.claude/rules/local-api.md`).
- Marshell never reads credential files (`~/.claude/.credentials.json`, `~/.codex/auth.json`, `~/.gemini/oauth_creds.json`). Status comes only from `claude auth status` and `codex login status`. Their raw output never leaves the core.
- No network request of Marshell's own (decision 10). "Newer version available" comes only from Codex's `version.json`.
- No background polling, no Sign out button (decision 8).
- Nothing is written to any CLI's files in this slice. The company tabs are read-only (decision 7).
- Secret masking (decision 15): every value under a key named `env` at any depth, and every value whose key contains, case-insensitively, `key`, `token`, `secret`, `password` or `auth`, shows as the word `hidden`. This holds in the form, in "Other settings" and in the raw view. A file that doesn't parse shows no values and no raw text.
- A "Sign in to …" tab gets no recording (decision 16). Update tabs are recorded.
- Copy is exact as written in each task: sentence case, no "!", no "successfully". An unknown value is an en dash (`–`) in the form and is left out of a sentence.
- Keyboard alone and forced colours work for every new control. `Announcer` is the only live region; no `role="alert"`.
- Cargo runs with `-j 4`. `cargo fmt --all` before every Rust commit. Commit messages read `type(scope): sentence.` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- No test binary is named after a real CLI. Tests never run the real `claude`, `codex` or `agy`.

## Review Focus

1. **A secret in an unexpected shape:** `"Authorization"` under `mcp_servers.x.headers`, `API_KEY` in a nested `env`, a secret in an array of tables, a TOML inline table. Expected: masked in "Other settings" and in the raw view. Task 4 pins it.
2. **A settings file that doesn't parse, or that is a folder or unreadable.** Expected: no values and no raw text leave the core, only the parse error's line and a fixed sentence. Task 4 pins it.
3. **An npm install whose `claude.cmd` shim keeps its modified time across `npm install -g`.** Expected: the version read after an Update tab exits is fresh, not the cached one. Task 1 pins it with `recheck`.
4. **`codex login status` that prints a partly masked API key** ("Logged in using an API key - sk-proj-***abcd"). Expected: only "an API key" reaches the frontend. Task 2 pins it.
5. **Settings remembered at a CLI section whose CLI is no longer found** (the tray's Settings… after an uninstall). Expected: it opens at Appearance, not at a hidden section. Task 5 pins it.

---

### Task 1: Install type, update command and the newer-version note (core)

**Files:**
- Create: `crates/core/src/agents/install.rs`
- Modify: `crates/core/src/agents/mod.rs` (add `pub mod install;`, and `run_capture` beside `run_version_text`)
- Create: `crates/protocol/src/install.rs`, export from `crates/protocol/src/lib.rs`
- Modify: `crates/core/src/server/agents.rs` (route handler `install`, and a `forget_version(exe)` on the version cache)
- Modify: `crates/core/src/server/mod.rs` (route `GET /v1/agents/{agent}/install`)
- Test: unit tests in `install.rs`; route tests beside the existing `server/agents.rs` tests

**Interfaces:**
- Produces (protocol, ts-rs exported):
  - `#[serde(tag = "kind", rename_all = "snake_case")] pub enum InstallKind { Native, Npm { package: String }, Winget { id: String }, Unknown }`
  - `pub struct AgentInstall { pub agent: String, pub found: bool, pub version: Option<String>, pub install: InstallKind, pub update_command: Option<String>, pub newer_version: Option<String> }`
- Produces (core):
  - `pub fn install_kind(agent: &str, exe: &Path, exists: &dyn Fn(&Path) -> bool) -> InstallKind`. `exe` is already canonicalized by the caller (`std::fs::canonicalize`, so a WinGet `Links` symlink resolves to its `Packages` target; on failure the caller passes the path as found).
  - `pub fn update_argv(agent: &str, kind: &InstallKind, exe: &Path) -> Option<Vec<String>>`
  - `pub fn update_text(agent: &str, kind: &InstallKind) -> Option<String>`: the command as shown to the person.
  - `pub fn codex_newer(version_json: &str, installed: &str) -> Option<String>`
  - `pub fn npm_package(agent: &str) -> Option<&'static str>`: `claude` → `@anthropic-ai/claude-code`, `codex` → `@openai/codex`, `antigravity` → None.
  - `pub fn run_capture(exe: &Path, args: &[&str], timeout: Duration) -> Option<(i32, String)>` in `agents/mod.rs`: stdout then stderr joined, stdin null, `CREATE_NO_WINDOW` on Windows as `run_version_text` does, killed at the timeout (None). Task 2 uses it.

The rules for `install_kind`, compared case-insensitively on path components, first match wins:
1. WinGet: a component sequence `Microsoft`, `WinGet`, `Packages`, `<dir>`. The id is `<dir>` up to `_Microsoft.Winget.Source` (case-insensitive). An id that doesn't match `^[A-Za-z0-9][A-Za-z0-9._-]*$` (or a `<dir>` without that suffix) is `Unknown`.
2. npm: `npm_package(agent)` is Some, and `exists(<exe's folder>/node_modules/<package>/package.json)`.
3. Native: `claude` whose folder is `<anything>\.local\bin` and file `claude.exe`; `codex` with the sequence `Programs`, `OpenAI`, `Codex`, `bin`; `antigravity` whose folder is `agy\bin` and file `agy.exe`.
4. Otherwise `Unknown`.

`update_argv`: Native → `[exe, "update"]`. Npm → `["cmd.exe", "/d", "/c", "npm", "install", "-g", "<package>@latest"]`. Winget → `["winget", "upgrade", "--id", id, "--exact"]`. Unknown → None. `update_text` is the same words without `cmd.exe /d /c`, with the program's bare name for Native (`claude update`, `codex update`, `agy update`).

`codex_newer`: `version.json` holds `latest_version` and `dismissed_version`. It returns `latest_version` when both it and `installed` parse as `X.Y.Z` (a `-suffix` after the third number is ignored for the comparison), latest is greater, and `dismissed_version` is not equal to it. Anything else, including bad JSON, is None.

Route: `GET /v1/agents/{agent}/install?recheck=true|false`. 404 for an agent without an adapter. `found: false` gives `version: None, install: Unknown`, no command. With `recheck=true` the version cache entry for that exe is dropped before reading. `newer_version` is filled for `codex` only, from `<CODEX_HOME or ~/.codex>/version.json` (the folder `codex_hooks_path` uses). It runs in `spawn_blocking`.

- [ ] **Step 1: Write the failing tests** in `install.rs`:
  - `winget_id_comes_from_the_packages_folder`: `C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\Anthropic.ClaudeCode_Microsoft.Winget.Source_8wekyb3d8bbwe\claude.exe` → `Winget { id: "Anthropic.ClaudeCode" }`; the same with lower-case `winget\packages` → same id.
  - `a_winget_folder_with_an_odd_id_is_unknown`: `...\Packages\Bad&Id_Microsoft.Winget.Source_x\claude.exe` → `Unknown`; `...\Packages\NoSuffix\claude.exe` → `Unknown`.
  - `npm_needs_the_package_beside_the_shim`: `C:\Users\u\AppData\Roaming\npm\claude.cmd` with `exists` true only for `...\npm\node_modules\@anthropic-ai\claude-code\package.json` → `Npm { package: "@anthropic-ai/claude-code" }`; with `exists` always false → `Unknown`; `antigravity` with `exists` always true → not Npm.
  - `native_paths`: `C:\Users\u\.local\bin\claude.exe` → Native; `C:\Users\u\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe` → Native for `codex`; `C:\Users\u\AppData\Local\agy\bin\agy.exe` → Native for `antigravity`; `C:\tools\claude.exe` → Unknown; `C:\Users\u\.local\bin\claude.exe` for `codex` → Unknown.
  - `update_argv_per_kind`: the four argv above, exactly; `update_text` gives `claude update`, `npm install -g @openai/codex@latest`, `winget upgrade --id Anthropic.ClaudeCode --exact`, None.
  - `codex_newer_cases`: `{"latest_version":"0.164.0","dismissed_version":null}` with `0.163.2` → `Some("0.164.0")`; with `0.164.0` → None; `0.163.0-alpha.2` installed → `Some("0.164.0")`; dismissed `"0.164.0"` → None; `"nope"` → None; latest `"x"` → None.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core agents::install`. Expected: FAIL, module not found.
- [ ] **Step 3: Implement** `install.rs`, `run_capture`, the protocol types.
- [ ] **Step 4: Run the unit tests.** Same command. Expected: PASS.
- [ ] **Step 5: Write the failing route tests** (beside the existing agents route tests, with the existing test helpers that point the agents at temp folders):
  - `install_is_refused_without_the_bearer` → 401.
  - `install_for_an_unknown_agent_is_404`.
  - `install_reports_not_found` when no exe is found → `found: false`, `install.kind == "unknown"`, `update_command: None`.
  - `install_reads_codex_version_json`: a temp `CODEX_HOME` with `version.json` newer than a fake exe's version → `newer_version` set. If the existing harness can't fake a found Codex exe with a version, rule it and test `newer_version` through a `pub(crate)` helper the handler calls with the folder and version instead.
  - `recheck_drops_the_cached_version`: `cached_version` keeps an entry; after `forget_version(exe)` the next read calls `ask` again.
- [ ] **Step 6: Run them to see them fail**, then add the handler, the route and `forget_version`. Run: `cargo test -j 4 -p marshell-core install`. Expected: PASS. Then `cargo test -j 4 -p marshell-protocol` and `pnpm check:generated`. Expected: ok.
- [ ] **Step 7: Commit** `feat(agents): read each CLI's install type, update command and Codex's newer version`.

### Task 2: Sign-in status (core)

**Files:**
- Create: `crates/core/src/agents/account.rs`
- Create: `crates/protocol/src/account.rs`, export from `lib.rs`
- Modify: `crates/core/src/server/agents.rs`, `crates/core/src/server/mod.rs` (route `GET /v1/agents/{agent}/account`)

**Interfaces:**
- Consumes: `run_capture` (Task 1).
- Produces (protocol):
  - `#[serde(rename_all = "snake_case")] pub enum SignIn { SignedIn, SignedOut, Unknown, NoCommand }`
  - `pub struct AccountStatus { pub agent: String, pub sign_in: SignIn, pub email: Option<String>, pub plan: Option<String>, pub method: Option<String>, pub checked_ms: u64 }`
- Produces (core): `pub fn claude_status(stdout: &str, code: i32) -> (SignIn, Option<String>, Option<String>)` (state, email, plan) and `pub fn codex_status(text: &str, code: i32) -> (SignIn, Option<String>)` (state, method).

Rules:
- Claude: run `<exe> auth status` (JSON by default). Parse JSON; `loggedIn: true` → SignedIn with `email` and `subscriptionType` as plan, each only when a non-empty string. `loggedIn: false` → SignedOut. Exit 1 with no parseable JSON → SignedOut. Anything else (other exit, bad JSON, timeout) → Unknown with no email or plan.
- Codex: run `<exe> login status`. Exit 0 and text containing "Logged in": method `"ChatGPT"` if the text contains `ChatGPT`, else `"an API key"` if it contains `API key` (case-insensitive), else None. Text containing "Not logged in", or exit ≠ 0 → SignedOut. Timeout → Unknown. The text itself is never returned.
- Antigravity: `NoCommand`, nothing run.
- Not found: Unknown, nothing run.
- Timeout 10 s. `checked_ms` is the time the check finished. Runs in `spawn_blocking`. The child gets the core's environment, so a live check's scratch `CLAUDE_CONFIG_DIR` and `CODEX_HOME` apply.

- [ ] **Step 1: Write the failing unit tests** in `account.rs`:
  - `claude_signed_in`: `{"loggedIn":true,"authMethod":"claude.ai","email":"a@example.com","subscriptionType":"max"}`, 0 → (SignedIn, Some("a@example.com"), Some("max")).
  - `claude_signed_in_without_email`: `{"loggedIn":true,"authMethod":"api_key"}`, 0 → (SignedIn, None, None); `"email":""` → None.
  - `claude_signed_out`: `{"loggedIn":false,"authMethod":"none"}`, 1 → SignedOut; `""`, 1 → SignedOut.
  - `claude_unknown`: `"garbage"`, 0 → Unknown; `""`, 2 → Unknown.
  - `codex_chatgpt`: `"Logged in using ChatGPT\n"`, 0 → (SignedIn, Some("ChatGPT")).
  - `codex_api_key_never_returns_the_key`: `"Logged in using an API key - sk-proj-***abcd\n"`, 0 → (SignedIn, Some("an API key")); assert no returned string contains `sk-`.
  - `codex_signed_out`: `"Not logged in\n"`, 1 → (SignedOut, None); `"Not logged in"`, 0 → SignedOut.
  - `codex_other_text`: `"Logged in\n"`, 0 → (SignedIn, None).
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core agents::account`. Expected: FAIL.
- [ ] **Step 3: Implement**, then run again. Expected: PASS.
- [ ] **Step 4: Route tests** (fail first, then the handler): `account_is_refused_without_the_bearer` (401); `account_for_antigravity_is_no_command` (200, `sign_in: "no_command"`, and nothing ran); `account_for_a_missing_cli_is_unknown`; unknown agent → 404. Run: `cargo test -j 4 -p marshell-core account`. Expected: PASS. Then `cargo test -j 4 -p marshell-protocol`, `pnpm check:generated`.
- [ ] **Step 5: Commit** `feat(agents): check the sign-in status with each CLI's own status command`.

### Task 3: Sign-in and update tabs (core)

**Files:**
- Modify: `crates/protocol/src/api.rs` (`CreateSessionRequest.cli_task`), new `CliTask` type there
- Modify: `crates/core/src/server/sessions.rs` (`create`, `start_session`, `spawn_into_map` gain `record: bool`)

**Interfaces:**
- Consumes: `install_kind`, `update_argv` (Task 1); `agent_exe_of`.
- Produces (protocol): `#[serde(rename_all = "snake_case")] pub enum CliTaskKind { SignIn, Update }`, `pub struct CliTask { pub agent: String, pub kind: CliTaskKind }`, and `#[serde(default)] pub cli_task: Option<CliTask>` on `CreateSessionRequest` ("A sign-in or update tab for a company CLI. It takes no shell, agent, prompt, mode, preset or tool.").
- Produces (core): `pub(super) fn cli_task_argv(agent: &str, kind: CliTaskKind, exe: &Path, install: &InstallKind) -> Result<Vec<String>, ApiError>`.

Rules:
- Any of `shell`, `agent`, `prompt`, `mode`, `preset`, `tool` set with `cli_task` → 400 "A sign-in or update tab takes nothing else."
- Unknown agent → 400 "Unknown agent: <agent>."; CLI not found → 409 "<Name> isn't installed." (`Claude Code`, `Codex`, `Antigravity`, from `agent_display_name`).
- SignIn: `claude` → `[exe, "auth", "login"]`, `codex` → `[exe, "login"]`; `antigravity` → 400 "Antigravity has no sign-in command." When `exe` ends in `.cmd` or `.bat` (case-insensitive), the argv is `["cmd.exe", "/d", "/c", exe, ...args]`.
- Update: `update_argv`; None → 409 "Marshell can't tell how <Name> was installed."
- The tab runs as a custom argv (`custom(argv)`), named "Sign in to <Name>" or "Update <Name>", `agent: None`, starting in the home folder. It is not remembered for any folder. A restart or a reopen of the tab starts the default shell, as every custom-argv tab already does, so an update never runs again by itself.
- `record` is false for SignIn (`SpawnSpec.record = None`), true otherwise. Every existing caller passes true.

- [ ] **Step 1: Write the failing tests:**
  - `cli_task_argv` unit tests: the claude and codex sign-in argv; `C:\x\npm\claude.cmd` → `["cmd.exe","/d","/c","C:\\x\\npm\\claude.cmd","auth","login"]`; antigravity sign-in → 400 with the copy; update with `Unknown` → 409 with "Marshell can't tell how Codex was installed."
  - Route tests with the existing session-route harness and a fake exe that is not named after a real CLI, if the harness can point an agent at one; otherwise the route tests cover only the refusals: `cli_task_with_a_shell_is_refused` (400, copy), `cli_task_for_a_missing_cli_is_409`.
  - `a_sign_in_tab_has_no_recording`: start a CLI-task SignIn session through the code path that takes `record` (use the harness's fake program for the argv if the route can't find a fake CLI; in that case test `spawn_into_map` with `record: false`), then `GET /v1/sessions/{tab}/recording` → 404 and no `<tab>-*.cast` file in the recordings folder.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core cli_task` and `... sign_in_tab`. Expected: FAIL.
- [ ] **Step 3: Implement**, then run again. Expected: PASS. Then the whole crate: `cargo test -j 4 -p marshell-core`, `cargo test -j 4 -p marshell-protocol`, `pnpm check:generated`. Expected: all pass.
- [ ] **Step 4: Commit** `feat(sessions): start sign-in and update tabs whose command the core builds`.

### Task 4: The read-only settings view, masked (core)

**Files:**
- Create: `crates/core/src/agents/settings_view.rs`
- Create: `crates/protocol/src/settings_view.rs`, export from `lib.rs`
- Modify: `crates/core/src/server/agents.rs`, `crates/core/src/server/mod.rs` (route `GET /v1/agents/{agent}/settings-view`)

**Interfaces:**
- Produces (protocol):
  - `#[serde(tag = "kind", rename_all = "snake_case")] pub enum ViewState { Ok, Missing, Unparsable { line: u32 }, Unreadable }`
  - `pub struct SettingValue { pub key: String, pub value: Option<String>, pub scope: ConfigScope }`. `ConfigScope` is the existing enum; add a `Managed` variant if it has none, and check that the config service still compiles against it (it never writes one).
  - `pub struct AgentSettingsView { pub agent: String, pub path: String, pub state: ViewState, pub curated: Vec<SettingValue>, pub others: Vec<SettingValue>, pub raw: Option<String> }`
- Produces (core):
  - `pub fn curated(agent: &str) -> &'static [&'static str]`:
    - `claude`: `model`, `effortLevel`, `permissions.defaultMode`, `cleanupPeriodDays`, `autoUpdatesChannel`, `theme`
    - `codex`: `model`, `model_reasoning_effort`, `approval_policy`, `sandbox_mode`, `project_doc_max_bytes`, `check_for_update_on_startup`
    - `antigravity`: `model`, `toolPermission`, `enableTerminalSandbox`, `colorScheme`, `verbosity`, `enableTelemetry`
  - `pub fn is_secret_key(key: &str) -> bool`: lower-cased key contains `key`, `token`, `secret`, `password` or `auth`.
  - `pub fn mask(value: &mut serde_json::Value)`: every leaf under a key named `env` (any case, any depth) and every value under a secret key (the whole subtree) becomes the string `"hidden"`. Arrays are walked.
  - `pub fn view(agent: &str, path: &Path, text: Result<String, std::io::Error>, managed: Option<serde_json::Value>) -> AgentSettingsView`. TOML for `codex` (parsed with `toml`, converted to `serde_json::Value`), JSON with comments for the others (the jsonc parser the crate already uses).

Rules:
- Files: `claude` → `st.claude_settings_path(user_home)`; `codex` → `config.toml` in the folder of `st.codex_hooks_path(user_home)`; `antigravity` → `antigravity::hooks::settings_path(&st.agy_user_home()?)`.
- Missing file → `Missing`, `curated` with every key `value: None`, `others` empty, `raw: None`. A read error other than not-found (a folder, no permission) → `Unreadable`, nothing else. A parse failure → `Unparsable { line }`, `curated` and `others` empty, `raw: None`. Neither text nor values leave the core for a file that doesn't parse.
- Values are masked first, then shown: a string as written; a number or bool as JSON; an object or array as compact JSON cut at 200 chars with `…`. A dotted curated key is looked up through nested objects. Scope is `User`.
- `others`: the top-level keys not in `curated` (a curated dotted key `permissions.defaultMode` does not remove `permissions`), sorted by key, each with its value as above.
- Claude's managed settings: for each path in `claude::compact::managed_settings_paths()` that reads and parses, a curated key it sets is shown with its masked value and scope `Managed`, in place of the user value. Codex and Antigravity show the user file only.
- `raw`: the masked value serialized back: `serde_json::to_string_pretty` for JSON, `toml::to_string_pretty` for TOML. (The text as written can't be masked without a span-preserving editor the crate doesn't have; comments and layout are left out, and the frontend says so.)
- 404 for an agent without an adapter. Runs in `spawn_blocking`.

- [ ] **Step 1: Write the failing unit tests** in `settings_view.rs`:
  - `masks_env_and_secret_keys`: `{"env":{"ANTHROPIC_API_KEY":"sk-ant-x","DEBUG":"1"},"apiKeyHelper":"op read x","model":"opus","mcp":{"s":{"headers":{"Authorization":"Bearer y"}}},"list":[{"token":"z"}]}` → after `mask`, the strings `sk-ant-x`, `1` (under env), `op read x`, `Bearer y` and `z` are gone and each is `"hidden"`; `model` stays `opus`.
  - `codex_toml_masks_nested_env`: `model = "gpt-5"\n[mcp_servers.x]\ncommand = "node"\n[mcp_servers.x.env]\nAPI_KEY = "abc"\n` → `view` gives curated `model` = `gpt-5`, an `others` entry `mcp_servers` whose value contains `hidden` and not `abc`, and `raw` contains `hidden` and not `abc`.
  - `unparsable_returns_nothing`: `{"model": "opus",` → `Unparsable { line: 1 }`, `curated` and `others` empty, `raw` None; a TOML `model = ` → `Unparsable`, same.
  - `missing_has_curated_dashes`: `Err(NotFound)` → `Missing`, curated keys all `value: None`, length 6.
  - `unreadable`: `Err(PermissionDenied)` → `Unreadable`, nothing else.
  - `dotted_curated_and_others`: `{"permissions":{"defaultMode":"plan","deny":["x"]},"zeta":1,"alpha":true}` for claude → curated `permissions.defaultMode` = `plan`; `others` keys in order `alpha`, `permissions`, `zeta`; `alpha` = `true`.
  - `long_values_are_cut`: an array of 100 strings → value ends with `…` and is at most 201 chars.
  - `managed_wins_for_curated`: managed `{"permissions":{"defaultMode":"default"}}` → that curated entry has scope `Managed`, value `default`.
  - `is_secret_key_cases`: true for `apiKeyHelper`, `GITHUB_TOKEN`, `Authorization`, `password`, `client_secret`; false for `model`, `theme`, `cleanupPeriodDays`.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core agents::settings_view`. Expected: FAIL.
- [ ] **Step 3: Implement**, run again. Expected: PASS.
- [ ] **Step 4: Route tests** (fail first): `settings_view_is_refused_without_the_bearer` (401); `settings_view_reads_the_claude_file` with the harness's `claude_settings` override pointing at a temp file holding an `env` secret → 200, `raw` contains `hidden` and not the secret; unknown agent → 404. Run: `cargo test -j 4 -p marshell-core settings_view`. Expected: PASS. Then `cargo test -j 4 -p marshell-protocol`, `pnpm check:generated`.
- [ ] **Step 5: Commit** `feat(agents): read each CLI's user settings for a read-only view, with secrets masked`.

### Task 5: The sections list, the API client and the scrolling nav (frontend)

**Files:**
- Modify: `src/features/settings/settingsSections.ts`, `settingsRouting.ts`, `SettingsShell.tsx`, `SettingsShell.css`, `SettingsNav.tsx`
- Create: `src/lib/api/accounts.ts` (client functions for Tasks 1, 2 and 4)
- Test: `src/features/settings/LiveSettings.test.tsx`, `SettingsShell.test.tsx`, `src/lib/api/accounts.test.ts`

**Interfaces:**
- Produces:
  - `SettingsSection` adds `"accounts" | "claude" | "codex" | "antigravity"`. `SETTINGS_SECTIONS` order: Appearance, Terminal, Notifications, Shell profiles, History, Agents, Accounts, Claude Code, Codex, Antigravity, Advanced. Labels exactly `Accounts`, `Claude Code`, `Codex`, `Antigravity`.
  - `export type CompanyAgent = "claude" | "codex" | "antigravity"`, and `export function shownSections(found: ReadonlySet<CompanyAgent>): Array<{ id: SettingsSection; label: string }>`: a company section is listed only when its CLI is in `found` (decision 7). Accounts is always listed.
  - `settingsStart(last, shown: readonly SettingsSection[])`: `last` when it is in `shown`, else `"appearance"`.
  - `SettingsShell` takes `sections: Array<{ id; label }>` (default `SETTINGS_SECTIONS`) and passes it to the nav.
  - `src/lib/api/accounts.ts`: `getInstall(ep, agent, recheck = false): Promise<AgentInstall>` (`GET /v1/agents/{agent}/install?recheck=…`), `getAccount(ep, agent): Promise<AccountStatus>`, `getSettingsView(ep, agent): Promise<AgentSettingsView>`, written like `src/lib/api/agents.ts`.
- Nav (decision 13): in the wide layout, `.settings-shell__nav` keeps `position: sticky` and gets `max-block-size: 100%` of the page body's scrollport height (use the page body as a size container or a `dvh` calc; the task decides with good-css) and `overflow-y: auto` with `overscroll-behavior: contain`, so it scrolls on its own. On a section change the current nav button is scrolled into view with `scrollIntoView({ block: "nearest" })`. The narrow layout keeps wrapping.

- [ ] **Step 1: Write the failing tests:**
  - `shownSections`: empty set → 8 entries, no Claude Code, Codex or Antigravity, Accounts after Agents; `{"codex"}` → Codex between Accounts and Advanced; all three → 11 in the order above.
  - `settingsStart("codex", shown without codex)` → `"appearance"`; `settingsStart("codex", shown with codex)` → `"codex"`; `settingsStart(null, …)` → `"appearance"`.
  - `paletteSection` tests stay green, and `"accounts"`, `"claude"` map to null (no new palette commands).
  - `SettingsShell` with a `sections` list of two renders only those two nav buttons.
  - CSS test in `SettingsShell.test.tsx` (the file's existing CSS-rule helper style): `.settings-shell__nav` has `overflow-y: auto` and a `max-block-size`, and the narrow container rule still sets one column.
  - `accounts.test.ts`: each client calls the right method and path, `recheck=true` when asked (stub `fetch` the way `agents` client tests do, if they exist; else test the path builder).
- [ ] **Step 2: Run them to see them fail.** Run: `pnpm test src/features/settings src/lib/api`. Expected: FAIL.
- [ ] **Step 3: Implement.** Read the good-css skill before the CSS.
- [ ] **Step 4: Run again.** Expected: PASS. Then `pnpm build`. Expected: ok.
- [ ] **Step 5: Commit** `feat(settings): list Accounts and the company sections, and let the nav scroll on its own`.

### Task 6: Accounts (frontend)

**Files:**
- Create: `src/features/accounts/AccountsSection.tsx`, `AccountsSection.css`, `accountsText.ts`
- Test: `src/features/accounts/AccountsSection.test.tsx`, `accountsText.test.ts`

**Interfaces:**
- Consumes: `getAccount` (Task 5), `relativeTime` (`src/features/history/historyState.ts`).
- Produces:
  - `export function accountLine(s: AccountStatus, name: string, now: number): string`
  - `export function AccountsSection(props: { endpoint: Endpoint; found: ReadonlySet<CompanyAgent>; onSignIn(agent: "claude" | "codex"): void; refresh: number; announce(text: string): void })`. `refresh` is a counter LiveApp bumps when a sign-in tab exits (Task 8); a change re-runs the checks.
  - `export function AccountsView(props: { rows: Array<{ agent: CompanyAgent; status: AccountStatus | "checking" | "failed" }>; now: number; onSignIn; onCheckAgain(): void })`: the pure view the tests render.

Copy (`accountLine`), with `<time>` = `relativeTime(now - checked_ms)`:
- Claude SignedIn: `Signed in as <email>, <Plan>, checked <time>`; the email or the plan is left out with its comma when absent (`Signed in as a@example.com, checked just now`, `Signed in, checked 5 min ago`). `<Plan>` is the plan with its first letter upper-cased.
- Codex SignedIn: `Signed in with <method>, checked <time>` (`Signed in with ChatGPT, …`, `Signed in with an API key, …`); no method → `Signed in, checked <time>`.
- SignedOut: `Not signed in, checked <time>`.
- Unknown: `Couldn't check the sign-in status, checked <time>`.
- NoCommand (Antigravity): `Antigravity has no sign-in status command, so Marshell can't show one.`
- `"checking"`: `Checking…`. `"failed"` (the request itself failed): `Couldn't reach the Marshell core.`

Layout: one row per CLI in `found`, in the order Claude Code, Codex, Antigravity: the CLI's name as an `h3`, the line, and for a SignedOut or Unknown Claude Code or Codex a button `Sign in to Claude Code` / `Sign in to Codex`. One `Check again` button below the rows, disabled while a check runs. None found: `No agent CLI is installed, so there is nothing to sign in to.` and no Check again. The checks run when the section mounts (it mounts the first time Accounts is shown), on Check again, and when `refresh` changes. After Check again, announce `Checked the sign-in status.` Rows are cards told apart by fill (no boxed outlines), tokens only.

- [ ] **Step 1: Write the failing tests:** `accountLine` for each case above with exact strings (including both left-out variants and `Max` capitalisation); `AccountsView` markup: buttons present only for signed-out or unknown Claude Code and Codex, never for Antigravity; the none-found copy; `Checking…` while checking; Check again disabled while any row is checking.
- [ ] **Step 2: Run them to see them fail.** Run: `pnpm test src/features/accounts`. Expected: FAIL.
- [ ] **Step 3: Implement.** Read good-css before the CSS.
- [ ] **Step 4: Run again.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(accounts): show each CLI's sign-in status in Settings, with Sign in and Check again`.

### Task 7: The company sections (frontend)

**Files:**
- Create: `src/features/companies/CompanySection.tsx`, `CompanySection.css`, `companyFields.ts`, `companyText.ts`
- Test: `src/features/companies/CompanySection.test.tsx`, `companyFields.test.ts`, `companyText.test.ts`

**Interfaces:**
- Consumes: `getInstall`, `getSettingsView` (Task 5); `ScopeBadge`, `RawEditor` (read-only), the settings form row look (`FieldRow`'s classes, read-only).
- Produces:
  - `companyFields.ts`: `export const COMPANY_FIELDS: Record<CompanyAgent, Array<{ key: string; label: string; help: string }>>`, in the core's curated order (Task 4). Labels and help:
    - Claude Code: `model` "Default model" / "The model a new Claude Code session starts with."; `effortLevel` "Effort" / "How hard Claude Code thinks by default."; `permissions.defaultMode` "Default permission mode" / "How Claude Code asks before it acts."; `cleanupPeriodDays` "Delete transcripts after (days)" / "Claude deletes its own transcripts after this long. Marshell keeps its own copy."; `autoUpdatesChannel` "Update channel" / "Which releases Claude Code updates itself to."; `theme` "Theme" / "Claude Code's own colours in the terminal."
    - Codex: `model` "Default model" / "The model a new Codex session starts with."; `model_reasoning_effort` "Reasoning effort" / "How hard Codex reasons by default."; `approval_policy` "Approval policy" / "When Codex asks before it runs a command."; `sandbox_mode` "Sandbox" / "What Codex's commands may touch."; `project_doc_max_bytes` "AGENTS.md size limit (bytes)" / "Codex reads instruction files up to this size."; `check_for_update_on_startup` "Check for updates at start" / "Whether Codex looks for a new version when it starts."
    - Antigravity: `model` "Default model" / "The model a new Antigravity session starts with."; `toolPermission` "Tool permission" / "When Antigravity asks before it uses a tool."; `enableTerminalSandbox` "Terminal sandbox" / "Whether Antigravity's commands run in a sandbox."; `colorScheme` "Colour scheme" / "Antigravity's own colours in the terminal."; `verbosity` "Verbosity" / "How much Antigravity says as it works."; `enableTelemetry` "Telemetry" / "Whether Antigravity sends usage data to Google."
  - `companyText.ts`:
    - `versionLine(i: AgentInstall)`: `Version <v>`; no version → `Version –`.
    - `newerLine(i)`: `Version <n> is available.` or null.
    - `manualUpdateLine(agent, name)` for an Unknown install: Claude Code `Marshell can't tell how Claude Code was installed. Update it with claude update, or npm install -g @anthropic-ai/claude-code@latest for an npm install.`; Codex the same with `codex update` and `@openai/codex`; Antigravity `Marshell can't tell how Antigravity was installed. Update it with agy update.` (commands in `<code>` in the markup).
    - `runningNote(name, count)`: count 0 → null; 1 → `Claude Code is running in 1 tab. The update may fail while it runs.`; n → `Claude Code is running in <n> tabs. The update may fail while they run.`
    - `fileLine(view)`: Missing → `<path> doesn't exist yet, so every setting has its default.`; Unparsable → `<path> doesn't parse (line <n>), so Marshell shows nothing from it.`; Unreadable → `Marshell can't read <path>.`; Ok → null.
  - `export function CompanySection(props: { endpoint: Endpoint; agent: CompanyAgent; name: string; running: number; onUpdate(agent: CompanyAgent): void; refresh: number })`, plus a pure `CompanyView(props: { name; install: AgentInstall | null; view: AgentSettingsView | null; running: number; mode: "form" | "raw"; onMode; onUpdate })` that the tests render. `refresh` is bumped by LiveApp when an Update tab for this agent exits; a change refetches with `recheck=true`.

Layout, top to bottom: the version block (`versionLine`, `newerLine` if any, `Update <Name>` button when `update_command` is set, else `manualUpdateLine`; `runningNote` under the button); then the file path as text with `fileLine` if any; then a two-choice radio group `Form` / `Raw file` (a native radio group, one Tab stop); Form shows the curated rows (label, value or `–`, `ScopeBadge` for the scope, help) and, when `others` is not empty, an `h3` `Other settings in this file` with one row per key (the key in `<code>`, its value); Raw shows `RawEditor` with `readOnly` and the line `Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden.` Raw file is disabled when `raw` is null. No value is editable. Unset curated rows show `–`.

- [ ] **Step 1: Write the failing tests:** every `companyText` function with the exact strings; `COMPANY_FIELDS` keys equal the core's curated lists in order (a test that copies the three Rust lists as literals); `CompanyView` markup: the Update button only with an `update_command`, the manual line otherwise, the running note with 2 tabs, `–` for an unset curated key, the Managed badge for a managed value, the Other settings heading only when `others` is non-empty, Raw file disabled with `raw: null`, the raw note in raw mode, no `<input>` other than the two radios and no enabled `<textarea>`.
- [ ] **Step 2: Run them to see them fail.** Run: `pnpm test src/features/companies`. Expected: FAIL.
- [ ] **Step 3: Implement.** Read good-css before the CSS.
- [ ] **Step 4: Run again.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(settings): read-only Claude Code, Codex and Antigravity sections with version and Update`.

### Task 8: Wiring in the live window (frontend)

**Files:**
- Modify: `src/features/settings/LiveSettings.tsx`, `src/features/sessions/LiveApp.tsx`
- Create: `src/features/settings/cliTasks.ts`
- Test: `src/features/settings/cliTasks.test.ts`, `LiveSettings.test.tsx`

**Interfaces:**
- Consumes: Tasks 3, 5, 6, 7; `sessionsApi.create`; the sessions snapshot rows LiveApp already has; `listAgents` for `found`.
- Produces (`cliTasks.ts`, pure):
  - `cliTaskRequest(agent: CompanyAgent, kind: "sign_in" | "update", size: { cols: number; rows: number }): CreateSessionRequest` with only `cols`, `rows` and `cli_task` set.
  - `runningCount(sessions: SessionInfo[], agent: CompanyAgent): number`: sessions whose state is `running` and whose `agent` is that agent, or whose `foreground` names that agent (read the `Foreground` type for the field; if it names none, count by `agent` only and ledger the ruling).
  - `exitedTasks(prev: Map<string, PendingTask>, sessions: SessionInfo[]): { done: PendingTask[]; rest: Map<string, PendingTask> }` where `PendingTask = { tabId: string; agent: CompanyAgent; kind: "sign_in" | "update" }`: a pending tab is done when its row's state is no longer `running`, or the row is gone.
  - `updateResult(name, before: string | null, after: AgentInstall)`: `<Name> is now version <v>.` when it changed, `<Name> is still version <v>.` when not, `Couldn't read <Name>'s version.` with none.
- LiveApp: `found` comes from `listAgents` (company agents with `found: true`), refreshed with `refreshSetup`. Sign in / Update call `sessionsApi.create(cliTaskRequest(…))`, close Settings, select the new tab, and add it to the pending map. A failed create announces the core's message after `Couldn't open the tab: `. When `exitedTasks` reports a sign-in done, LiveApp bumps the Accounts `refresh` counter and, once that check returns, announces `<Name>: <accountLine>`. When an update is done, it fetches `getInstall(…, true)`, announces `updateResult`, and bumps that company section's `refresh`. The version before the update is the one the section last showed.
- LiveSettings renders `accounts` and the three company sections, gets `sections = shownSections(found)` for the shell, and opens at `settingsStart` with that list.

- [ ] **Step 1: Write the failing tests** for the four `cliTasks.ts` functions (exact request shape with no other field set; running count with an exited row, a foreground row and another agent's row; exitedTasks for running, exited, ended and a removed row; the three `updateResult` strings) and a `SettingsPanels` test that `accounts` and `codex` render through `render`.
- [ ] **Step 2: Run them to see them fail.** Run: `pnpm test src/features/settings`. Expected: FAIL.
- [ ] **Step 3: Implement**, then run `pnpm test`, `pnpm build`. Expected: all pass.
- [ ] **Step 4: Commit** `feat(settings): open sign-in and update tabs from Settings, and check again when they exit`.

### Task 9: Live check

Use the `marshell-live-check` skill with a new scratch folder, `lc-6e2-1`, and `-CodexHome` set to a short path such as `%TEMP%\lc-6e2-cx`. Never press Update for a real CLI: it would update the person's own install. Ask the user first if the check needs it.

Exit list:
1. Accounts with the scratch homes shows `Not signed in, checked just now` for Claude Code and Codex, each with its Sign in button, and Antigravity's sentence. Check again announces `Checked the sign-in status.`
2. Sign in to Codex opens a tab named "Sign in to Codex", Settings closes and the tab is selected; Ctrl+C in that tab (a tab the check started) ends it, and the status is announced. `GET /v1/sessions/{tab}/recording` is 404.
3. The Claude Code, Codex and Antigravity sections are listed only for the CLIs found, and each shows `Version <v>`, the install type's Update button (native on this machine), and, for Codex with a `version.json` seeded newer in the scratch `CODEX_HOME`, `Version <n> is available.`
4. A Claude settings file seeded in the scratch `MARSHELL_CLAUDE_SETTINGS` with `env.ANTHROPIC_API_KEY`, `apiKeyHelper` and `model` shows `hidden` for both secrets in the form, in Other settings and in the raw view, and the model in the form. A broken copy shows the parse line and nothing else.
5. The running note is covered by unit tests. It is checked live only if a Codex tab can be started in the scratch `CODEX_HOME`.
6. 720×480 at text size Larger: the nav scrolls on its own and the current section stays in view. Keyboard only through Accounts and one company section. Forced colours emulated on both.

Write `docs/research/2026-10-27-accounts-and-company-tabs-live-check.md`. Each bug found gets a failing test first, then the fix, each its own commit.

- [ ] **Step 1: Run the check** and record every result.
- [ ] **Step 2: Fix what it found**, failing test first.
- [ ] **Step 3: Commit the note** `docs(research): record the accounts and company tabs live check`.
