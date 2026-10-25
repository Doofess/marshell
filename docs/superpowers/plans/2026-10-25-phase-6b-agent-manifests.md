# Slice 6b: agent manifests and own agents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marshell can start, resume and recognise nine more agent CLIs from data manifests, start Claude Code on DeepSeek, and lets a person add their own agent. No Rust adapter is written for any of them.

**Architecture:** A manifest is a protocol type. The core ships nine manifests in an embedded JSON file and reads the person's own manifests from `~/.marshell/agents.json`. An `Agents` value (the three adapters' fixed data, plus shipped and own manifests) replaces the per-agent `match` tables in `profiles/detect.rs` for launch, continue, mode flags, the first prompt and foreground names. A catalog route serves the frontend the agents found on this machine with their offered modes, and the launcher, mode chip and rows read that instead of hard-coded lists. Status for a manifest agent stays terminal-only: the existing pty fallback, plus OSC 9;4 progress and OSC 99 notifications in the scanner.

**Tech Stack:** Rust (axum, serde, ts-rs, regex), React and TypeScript, Vitest.

**Spec:** `docs/PLAN.md` §9, "Slice 6b decisions (2026-10-25)" (items 1–8) and "Slice 6c decisions" item 2 (scope boundary only). Evidence: `docs/research/2026-10-25-agent-and-tool-cli-manifests.md`.

## Global Constraints

- Claude Code, Codex and Antigravity keep their Rust adapters. Their hook, permission, status, history and version-gate code is unchanged, and so is every `== "claude"` and `== "codex"` check outside launch and detection. (6b decision 1)
- Marshell writes no hook or config file for any manifest agent. (6b decision 4)
- No status is guessed. A manifest agent gets the pty fallback (Working or Idle, "Status limited"), Working from its own OSC 9;4 progress, and Needs you from its own OSC 9, 99 or 777 notification or a bell. (6b decision 4, amended)
- A mode is offered only when the manifest maps it. `[]` is allowed only for `manual`, and only where the CLI's documented default asks before it runs tools. Every other mode needs an explicit flag. Grok's `manual` is `--permission-mode default`, because `~/.grok/config.toml` can set always-approve.
- The DeepSeek key is read from the `DEEPSEEK_API_KEY` environment variable at launch and goes only into that child's environment, as `ANTHROPIC_AUTH_TOKEN`. It is never written to the layout, history, logs, diagnostics, the API or the screen. (6b decision 5)
- `~/.marshell/agents.json` is written only through `crate::store` (versioned, atomic), and only after a changed-since-read check. Tests use a temp home, never the real `~/.marshell`. (6b decision 6; SCOPE.md)
- New routes go in the token-guarded router, each with a "refused without the bearer" test. No route takes a file path. A new request or response type goes in `crates/protocol`, and `pnpm check:generated` passes. (`.claude/rules/local-api.md`)
- Never name a test binary after a real CLI (grok, agent, qwen, copilot, kimi, crush, aider, amp, opencode). Use names like `grok-marshell-test.exe`, and test name matching as pure functions with an injected `agent_path`. (`.claude/rules/pty.md`)
- Marshell never presses Enter in a terminal. (SCOPE.md)
- Copy follows PLAN §Microcopy: sentence case, subject first, no "!" or "successfully". UI works with the keyboard alone and in forced colors; tokens only, no hex; no boxed outlines. (`.claude/rules/ui.md`)
- Rust: `cargo test -j 4` (memory: default jobs run rustc out of memory). Run `cargo fmt --all` before every Rust commit.
- Commit messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Identity on this PC.** `~/.grok/bin` is on PATH and holds `agent.exe`, and the bare `copilot` is VS Code's shim. Expected: Cursor is "found" only through `cursor-agent`, or through an `agent` that resolves outside `\.grok\bin\`; Copilot is not found through the shim; picking Cursor never starts Grok. Pinned by Task 2's `cursor_is_not_found_through_groks_agent_exe` and `copilot_shim_is_not_copilot`.
2. **Claude hooks firing in a Grok tab.** Expected: no live status, no recap and no approve card on a Grok row, and a permission request from Grok gets no hold (Grok's own prompt applies). Pinned by Task 6's `a_claude_event_from_a_grok_tab_is_dropped_before_any_hold` and the bridge's `grok_hook_env_makes_the_claude_bridge_exit_quietly`.
3. **A first prompt for a CLI that takes none** (Kimi, Crush, Amp, Aider). Expected: the launcher hides the first-prompt box, and an API request with a prompt gets a 400 naming the CLI, instead of the prompt being dropped or pasted. Pinned by Task 3's `a_prompt_for_a_cli_without_one_is_refused`.
4. **The DeepSeek key leaking**, through the layout file, a Restart or Reopen (silently becoming an Anthropic session), an inherited `ANTHROPIC_API_KEY` going to api.deepseek.com, or a WSL profile. Pinned by Task 5's four tests.
5. **An own agent with a path that has a space**, such as `C:\Users\Home Office\tools\my agent.exe`, or with shell metacharacters. Expected: PowerShell and POSIX quote it, and cmd refuses it with a reason. Pinned by Task 3's `an_own_command_with_a_space_is_quoted_per_shell`.

---

## File structure

- Create `crates/protocol/src/manifest.rs`: `AgentManifest`, `PromptStyle`, `AgentCatalog`, `CatalogAgent`, `CatalogPreset`, `OwnAgents`, `OwnAgentProblem`, `OwnAgentsSave`. The catalog types live here, not in `agents.rs`, which is the hook adapters' file.
- Create `crates/core/src/agents/manifests.json`: the nine shipped manifests.
- Create `crates/core/src/agents/manifest.rs`: parse the shipped manifests, validate own agents, and the `Agents` value with its lookups.
- Create `crates/core/src/agents/own.rs`: the `~/.marshell/agents.json` load and save (mtime reload, changed-since-read hash).
- Create `crates/core/src/agents/identify.rs`: resolve a manifest to a found command (reject paths, version shape, cached version reads).
- Create `crates/core/src/agents/presets.rs`: the DeepSeek preset (env table, inherited variables to strip, refusals).
- Create `crates/core/src/server/catalog.rs`: `GET /v1/agents/catalog`, `GET /v1/agents/own`, `PUT /v1/agents/own`.
- Modify `crates/core/src/profiles/detect.rs`: the tables become `Agents` lookups; quoting the program and arguments.
- Modify `crates/core/src/server/sessions.rs`, `crates/core/src/server/foreground.rs`, `crates/core/src/server/hook.rs`, `crates/core/src/server/mod.rs`, `crates/core/src/registry.rs`, `crates/core/src/layout.rs`, `crates/core/src/pty/osc133.rs`, `crates/hook/src/lib.rs`, and `crates/protocol/src/api.rs` (the `preset` fields).
- Frontend: create `src/lib/api/catalog.ts`. Modify `src/features/sessions/launchPrograms.ts`, `LiveLauncher.tsx`, `src/features/launcher/launchChoice.ts`, `src/features/modes/sessionModes.ts`, `src/features/sessions/resumeCommand.ts`, `src/features/sessions/toRow.ts` and `src/features/agents/AgentsDialog.tsx`. Create `src/features/agents/OwnAgentsSection.tsx`.

---

### Task 1: The manifest type and the shipped manifests

**Files:**
- Create: `crates/protocol/src/manifest.rs` (register in `crates/protocol/src/lib.rs`, ts-rs export like its neighbours)
- Create: `crates/core/src/agents/manifests.json`, `crates/core/src/agents/manifest.rs` (add `pub mod manifest;` to `agents/mod.rs`)
- Test: unit tests in `manifest.rs`; regenerate `src/generated/` with `cargo test -p marshell-protocol`

**Interfaces:**
- Produces (protocol):
  ```rust
  #[serde(tag = "style", rename_all = "kebab-case")]
  pub enum PromptStyle { Positional, Flag { flag: String }, None }
  #[serde(deny_unknown_fields)]
  pub struct AgentManifest {
      pub id: String, pub name: String, pub cli: String, pub mark: String,
      pub commands: Vec<String>,
      #[serde(default)] pub reject_paths: Vec<String>,
      #[serde(default)] pub version_shape: Option<String>,
      #[serde(default)] pub exe_names: Vec<String>,
      #[serde(default)] pub packages: Vec<String>,
      pub prompt: PromptStyle,
      #[serde(rename = "continue")] pub continue_args: Option<Vec<String>>,
      #[serde(default)] pub modes: BTreeMap<SessionMode, Vec<String>>,
  }
  ```
  `SessionMode` is the existing protocol enum, with its existing serde names.
- Produces (core): `pub fn shipped() -> &'static [AgentManifest]` (a `LazyLock` over `include_str!("manifests.json")`; a parse failure panics, and a test pins that it parses); `pub fn validate_own(list: &[AgentManifest], reserved: &Reserved) -> (Vec<AgentManifest>, Vec<OwnAgentProblem>)`; `pub struct Reserved { ids: HashSet<String>, exe_names: HashSet<String> }` with `Reserved::builtin()`, which holds the adapter ids, the shipped ids, `deepseek`, every frontend tool id, and the exe names `claude`, `codex`, `agy` plus every shipped `exe_names` entry.
- `OwnAgentProblem { index: u32, id: Option<String>, reason: String }`.

- [ ] **Step 1: Write the failing tests** in `manifest.rs`:
  - `shipped_manifests_parse_and_have_unique_ids`: nine entries, ids exactly `["copilot","cursor","opencode","amp","grok","qwen","kimi","aider","crush"]` in that order.
  - `shipped_modes_obey_the_explicit_flag_rule`: every mode other than `manual` has a non-empty flag list, and no shipped manifest maps `dont-ask`.
  - `grok_manual_is_an_explicit_default`: `grok.modes[manual] == ["--permission-mode","default"]`.
  - `own_agent_validation_reasons`. Each case gives exactly this reason:
    - id `claude`: "The id \"claude\" is taken by a built-in agent."
    - id `docker`: same wording, "a built-in tool".
    - id `Bad Id`: "An id uses lowercase letters, digits and dashes, up to 32."
    - empty name: "An agent needs a name."
    - empty `commands`: "An agent needs a command."
    - a command containing a newline or any of `& | ; < > ^ % $ \` " '`: "The command \"…\" holds a character a shell would act on."
    - exe name `agy`: "The program name \"agy\" belongs to a built-in agent."
    - a `dont-ask` mode: "Own agents can't map Don't ask."
    - a non-manual mode with `[]`: "The <Mode> mode needs a flag."
    - two own entries with the same id: "Two agents share the id \"x\"."

    A valid entry passes with `mark` forced to `"generic"`.
- [ ] **Step 2: Run them** with `cargo test -j 4 -p marshell-core agents::manifest`. Expected: compile failure, because the module doesn't exist yet.
- [ ] **Step 3: Write `manifests.json`** with these values, which are 6b's spec data (sources in the research note):

  | id | name / cli / mark | commands | reject_paths | version_shape | exe_names | packages | prompt | continue | modes |
  |---|---|---|---|---|---|---|---|---|---|
  | copilot | GitHub Copilot / Copilot CLI / copilot | `copilot` | `\copilot-shim\` | – | `copilot` | `@github/copilot` | flag `-i` | `--continue` | manual `[]`, plan `--plan`, bypass `--allow-all` |
  | cursor | Cursor / Cursor CLI / cursor | `cursor-agent`, `agent` | `\.grok\bin\` | – | `cursor-agent` | – | positional | `--continue` | manual `[]`, plan `--plan`, bypass `--force` |
  | opencode | opencode / opencode / opencode | `opencode` | – | – | `opencode` | `opencode-ai` | flag `--prompt` | `--continue` | none |
  | amp | Amp / Amp / amp | `amp` | – | – | `amp` | `@sourcegraph/amp` | none | `threads continue` | bypass `[]`* |
  | grok | Grok / Grok Build / grok | `grok` | – | `^grok \d+\.\d+\.\d+ \(` | `grok` | – | positional | `--continue` | manual `--permission-mode default`, plan `… plan`, auto-edit `… acceptEdits`, full-auto `… auto`, bypass `… bypassPermissions` |
  | qwen | Qwen / Qwen Code / qwen | `qwen` | – | – | `qwen` | `@qwen-code/qwen-code` | flag `-i` | `--continue` | `--approval-mode` with `default`, `plan`, `auto-edit`, `auto`, `yolo` for the five modes |
  | kimi | Kimi / Kimi Code CLI / generic | `kimi` | – | – | `kimi` | – | none | `--continue` | manual `[]`, plan `--plan`, full-auto `--yolo`, bypass `--auto` |
  | aider | Aider / Aider / generic | `aider` | – | – | `aider` | – | none | `--restore-chat-history` | manual `[]`, bypass `--yes-always` |
  | crush | Crush / Crush / generic | `crush` | – | – | `crush` | – | none | `--continue` | manual `[]`, bypass `--yolo` |

  \*Amp runs tools without asking by default, so its only mode is Bypass with no flag. This is the one exception to the explicit-flag rule. The test allows it by name, and a comment says why.

  Reject paths compare case-insensitively against the resolved path, with `/` normalised to `\`.
- [ ] **Step 4: Implement** `shipped`, `Reserved` and `validate_own` in `manifest.rs`. The protocol types go in `manifest.rs` with `#[derive(TS)]` and an export the way `agents.rs` exports.
- [ ] **Step 5: Run** `cargo test -j 4 -p marshell-core agents::manifest` and then `cargo test -j 4 -p marshell-protocol`. Expected: PASS, and the new files appear under `src/generated/`. Then run `pnpm check:generated`. Expected: ok.
- [ ] **Step 6: Commit** with `feat(agents): add agent manifests and the nine shipped ones`.

### Task 2: Finding an agent on this machine

**Files:**
- Create: `crates/core/src/agents/identify.rs`
- Test: unit tests in `identify.rs`

**Interfaces:**
- Consumes: `AgentManifest` (Task 1).
- Produces:
  ```rust
  pub struct Found { pub command: String, pub path: PathBuf, pub version: Option<String> }
  pub fn find(m: &AgentManifest, path_of: &dyn Fn(&str) -> Option<PathBuf>,
              version_of: &dyn Fn(&Path) -> Option<String>) -> Result<Found, Missing>
  pub enum Missing { NotOnPath, Rejected { path: PathBuf }, WrongVersion { path: PathBuf, seen: Option<String> } }
  pub struct VersionCache  // keyed by (path, mtime); `get(&self, path) -> Option<String>` runs `run_version_text` with 5 s timeout on a miss
  ```
  `find` tries `commands` in order and returns the first one whose resolved path contains no `reject_paths` fragment and, when `version_shape` is set, whose `--version` output matches. `version_of` is called only when a shape is set. A `Found.version` is `parse_any_version` of that output when it was read.

- [ ] **Step 1: Write the failing tests**:
  - `cursor_is_not_found_through_groks_agent_exe`: with `path_of("cursor-agent") = None` and `path_of("agent") = C:\Users\Home Office\.grok\bin\agent.exe`, the result is `Err(Rejected)`. With `cursor-agent` resolving to `C:\Users\x\AppData\Local\cursor-agent\cursor-agent.exe`, the result is `Ok`, with command `cursor-agent`.
  - `copilot_shim_is_not_copilot`: `C:\Program Files\Microsoft VS Code\bin\copilot-shim\copilot.cmd` is rejected, and a forward-slash form of the path is rejected too.
  - `grok_needs_its_version_shape`: output `grok 1.0.50 (c58f321264ba) [stable]` is found, with version `1.0.50`. Output `grok-dev 1.1.7` gives `WrongVersion`.
  - `version_cache_reads_once_per_mtime`, using a counting closure behind the cache's injectable reader.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core agents::identify`. Expected: FAIL to compile.
- [ ] **Step 3: Implement** `find`, `Missing` and `VersionCache`. The cache holds a `Mutex<HashMap<(PathBuf, SystemTime), Option<String>>>` and never holds the lock while it runs a process.
- [ ] **Step 4: Run the same tests.** Expected: PASS.
- [ ] **Step 5: Commit** with `feat(agents): find manifest agents by command, install folder and version shape`.

### Task 3: Launching and recognising manifest agents

**Files:**
- Modify: `crates/core/src/profiles/detect.rs`: `agent_command`, `agent_of_exe`, `agent_of_package`, `agent_in`, `agent_exe`, `resume_args`, `mode_args`, `build_argv`, `launch_argv` and `launch_argv_resume`, plus their tests
- Modify: `crates/core/src/agents/manifest.rs` (the `Agents` value), `crates/core/src/server/sessions.rs`, `crates/core/src/server/foreground.rs` and `crates/core/src/server/agents.rs:371` (the callers)
- Test: `detect.rs` tests, and `crates/core/tests/session.rs` where `agent_in` is used

**Interfaces:**
- Consumes: `shipped()` and `validate_own` (Task 1), `identify::find` (Task 2).
- Produces:
  ```rust
  /// The three adapters' launch data plus shipped and valid own manifests. Cheap to clone (Arc inside).
  #[derive(Clone)] pub struct Agents { … }
  impl Agents {
      pub fn shipped() -> Self;                       // no own agents
      pub fn with_own(own: Vec<AgentManifest>) -> Self;
      pub fn manifest(&self, id: &str) -> Option<&AgentManifest>;   // None for claude/codex/antigravity
      pub fn is_known(&self, id: &str) -> bool;
      pub fn agent_of_exe(&self, name: &str) -> Option<String>;
      pub fn agent_of_package(&self, name: &str) -> Option<String>;
      pub fn manifests(&self) -> impl Iterator<Item = &AgentManifest>;  // shipped then own
  }
  pub enum Program<'a> { Name(&'a str), Path(&'a Path) }  // what build_argv puts first
  ```
  The new signatures are `launch_argv(profile, agents: &Agents, agent: Option<(&str, Program)>, prompt, mode, no_daemon)` and `launch_argv_resume(profile, agents, agent, mode, target, no_daemon)`. `agent_in(agents, chain, command_line) -> Option<String>`. For the adapters, `Program::Name` is the existing `agent_command`. For a manifest agent, the caller passes the `Found.command` from Task 2 as `Program::Name`, or `Program::Path` for an own agent whose command is a path.

- [ ] **Step 1: Write the failing tests** in `detect.rs`. The existing tests switch to `Agents::shipped()`, and these are added:
  - `grok_launches_with_a_positional_prompt_and_its_mode`: PowerShell argv ends with `& grok --permission-mode default 'fix it'`.
  - `qwen_takes_its_prompt_after_dash_i`: `& qwen --approval-mode plan -i 'x'`.
  - `opencode_takes_its_prompt_after_dash_dash_prompt`.
  - `a_prompt_for_a_cli_without_one_is_refused`: kimi, crush, amp and aider with a prompt give `Err("Kimi Code CLI can't take a first prompt. Start it, then type it.")`, with each CLI's `cli` name.
  - `manifest_resume_uses_continue_for_every_target`: Last, Session and Fork all give `--continue` for grok, `threads continue` for amp and `--restore-chat-history` for aider.
  - `an_unmapped_mode_adds_no_flags_and_is_refused_by_the_server`: `mode_args` returns `None` for cursor `auto-edit`. The return type changes to `Option<Vec<String>>`, and the adapters' `None` cases stay `Some(&[])` where they did so before.
  - `an_own_command_with_a_space_is_quoted_per_shell`: `Program::Path("C:\\Users\\Home Office\\tools\\my agent.exe")` becomes `& 'C:\Users\Home Office\tools\my agent.exe'` in PowerShell, is single-quoted in POSIX, and gives `Err("Command Prompt can't start a program whose path has a space. Use PowerShell.")` in cmd. Own flags with a space or quote are quoted the same way.
  - `foreground_names_come_from_manifests`: `agent_in(&agents, ["pwsh","grok"], None) == Some("grok")`. A bare `agent` in the chain maps to nothing. A chain `["pwsh","grok","agent"]` gives `grok`. A command line `npx @qwen-code/qwen-code` gives `qwen`. An own agent's `exe_names` are recognised.
  - `adapters_launch_exactly_as_before`: the existing claude, codex and agy argv tests pass unchanged after the signature change. This is the regression net.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core profiles::detect`. Expected: FAIL to compile.
- [ ] **Step 3: Implement.**
  - `build_argv` takes the prompt style from the manifest; `agy` keeps `-i` as an adapter rule.
  - The program word and every manifest or own argument go through `powershell_quote` or `posix_quote` when they hold anything other than `[A-Za-z0-9._:/\\=-]`. cmd refuses a program or argument that needs quoting.
  - `sessions::resolve` builds the `Agents` from `AppState` (Task 4 adds the own list; until then use `Agents::shipped()`), and returns 400 with these messages:
    - an unknown agent: "Unknown agent: x."
    - a manifest agent that isn't found: "Grok isn't installed, or isn't on PATH."
    - an unmapped mode: "Cursor has no Auto-edit mode."
  - `foreground.rs` and `server/agents.rs` pass the `Agents` value.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: PASS, including `tests/session.rs`.
- [ ] **Step 5: Commit** with `feat(agents): launch, resume and recognise manifest agents`.

### Task 4: Own agents and the catalog route

**Files:**
- Create: `crates/core/src/agents/own.rs`, `crates/core/src/server/catalog.rs`
- Modify: `crates/core/src/server/mod.rs` (routes, `AppState` field), `crates/core/src/paths.rs` (`agents_file(home)`)
- Test: `crates/core/tests/catalog_api.rs` (new, in the style of `tests/agents_api.rs`)

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces (protocol, in `manifest.rs`):
  ```rust
  pub struct CatalogAgent { pub id: String, pub name: String, pub cli: String, pub mark: String,
      pub adapter: bool, pub own: bool, pub found: bool, pub version: Option<String>,
      pub prompt: PromptStyle, pub modes: Vec<SessionMode>, pub can_continue: bool }
  pub struct CatalogPreset { pub id: String, pub name: String, pub agent: String, pub mark: String, pub problem: Option<String> }
  pub struct AgentCatalog { pub agents: Vec<CatalogAgent>, pub presets: Vec<CatalogPreset> }
  pub struct OwnAgents { pub agents: Vec<AgentManifest>, pub problems: Vec<OwnAgentProblem>, pub version: String }
  pub struct OwnAgentsSave { pub agents: Vec<AgentManifest>, pub version: String }
  ```
  For an adapter, `modes` is the five modes `sessionModes.ts` offers for it, from `mode_args`. Codex's `plan` is offered, because its stand-in exists; keep what the launcher offers today.
- Routes:
  - `GET /v1/agents/catalog` returns `AgentCatalog`, adapters first, then shipped, then own.
  - `GET /v1/agents/own` returns `OwnAgents`. `version` is the SHA-256 hex of the file bytes, or `""` when there is no file.
  - `PUT /v1/agents/own` takes `OwnAgentsSave`. It answers 409 "agents.json changed since you opened it." when the version doesn't match, 422 with the problems when validation fails, and 200 with the new `OwnAgents` otherwise.
- `own::load(home) -> OwnAgents` reads `agents.json` (`{"version":1,"agents":[…]}` through `crate::store`, `Versioned` 1). A broken or newer file gives no agents and one problem, "agents.json can't be read: <reason>.", and is never overwritten. `AppState` caches the parsed result keyed by the file's mtime and re-reads on mtime change. There is no watcher.

- [ ] **Step 1: Write the failing tests** in `tests/catalog_api.rs`, with a temp `MARSHELL_HOME`, an injected `agent_path` that finds only `grok-marshell-test` mapped as `grok`, and an injected version reader:
  - `catalog_lists_adapters_shipped_and_own_with_found_flags`.
  - `own_save_round_trips_and_reloads_on_change`: a PUT, then a hand edit of the file (new mtime), then a GET shows the edit.
  - `own_save_refuses_a_stale_version`: 409.
  - `own_save_refuses_invalid_entries`: 422 with the Task 1 reasons, and the file unchanged.
  - `a_broken_agents_file_is_reported_and_kept`.
  - `catalog_routes_refuse_without_the_bearer`: all three routes.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core --test catalog_api`. Expected: FAIL.
- [ ] **Step 3: Implement** `own.rs`, `catalog.rs` and the routes. File permissions follow `crate::store`'s existing behaviour; no new permission code (asking first would be required). Blocking file and version reads go in `spawn_blocking`.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`, then `cargo test -j 4 -p marshell-protocol && pnpm check:generated`. Expected: PASS and ok.
- [ ] **Step 5: Commit** with `feat(agents): own agents in agents.json and the agent catalog route`.

### Task 5: The DeepSeek preset

**Files:**
- Create: `crates/core/src/agents/presets.rs`
- Modify:
  - `crates/protocol/src/api.rs`: `preset: Option<String>` on the launch request, `SessionInfo`, and the layout tab record; `#[serde(default)]`, so old layouts load.
  - `crates/core/src/registry.rs`: `Relaunch.preset`.
  - `crates/core/src/layout.rs`: `remember` carries the preset.
  - `crates/core/src/server/sessions.rs`: env and refusals.
  - The history record, so Open in new tab keeps the preset, where it stores `agent`.
- Test: unit tests in `presets.rs`, plus `crates/core/tests/session.rs` or a new `tests/preset.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct PresetLaunch { pub env: Vec<(String, String)>, pub env_remove: Vec<String> }
  pub fn deepseek(key: Option<String>, family: ShellFamily) -> Result<PresetLaunch, String>
  pub const DEEPSEEK: &str = "deepseek";
  ```
- The env is DeepSeek's documented set, checked 2026-10-25 at https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code/ :
  - `ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic`
  - `ANTHROPIC_AUTH_TOKEN=<key>`
  - `ANTHROPIC_MODEL=deepseek-flash[1m]`
  - `ANTHROPIC_DEFAULT_OPUS_MODEL=deepseek-flash[1m]`
  - `ANTHROPIC_DEFAULT_SONNET_MODEL=deepseek-flash[1m]`
  - `ANTHROPIC_DEFAULT_HAIKU_MODEL=deepseek-flash`
  - `CLAUDE_CODE_SUBAGENT_MODEL=deepseek-flash`
  - `CLAUDE_CODE_EFFORT_LEVEL=max`
  - `CLAUDE_CODE_AUTO_COMPACT_WINDOW=786432`
- `env_remove` holds every `ANTHROPIC_*` variable in Marshell's own environment that the preset doesn't set, plus `CLAUDE_CODE_USE_BEDROCK` and `CLAUDE_CODE_USE_VERTEX`. It is added to `INHERITED_MARKERS` for that spawn only.
- The refusals (400):
  - with no key: "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek."
  - on a WSL profile: "DeepSeek starts from a Windows shell profile, not WSL."
- The catalog's `CatalogPreset.problem` carries the first message when the key is unset, and the frontend shows it without starting the preset.
- The row's `agent` stays `"claude"`, so the hooks, status, history and gates are unchanged. `preset` is display data and travels through `Relaunch`, the layout and history.

- [ ] **Step 1: Write the failing tests**:
  - `deepseek_env_is_the_documented_set`.
  - `deepseek_refuses_without_a_key_and_on_wsl`.
  - `inherited_anthropic_variables_are_stripped`: with an `ANTHROPIC_API_KEY` in the parent env, it is in `env_remove` and absent from `env`.
  - `the_key_never_reaches_the_layout_file`: a launch through the API with `preset: "deepseek"` and a test key `sk-marshell-test-0000`, then a layout save; neither the layout file nor the `SessionInfo` JSON contains the key.
  - `restart_and_reopen_keep_the_preset`: `Relaunch.preset == Some("deepseek")` after a restart and after a reopen.
- [ ] **Step 2: Run** the new tests with `cargo test -j 4 -p marshell-core preset`. Expected: FAIL.
- [ ] **Step 3: Implement.** The key is read with `std::env::var("DEEPSEEK_API_KEY")` at spawn time, through an injectable getter as `shell::integrate` does it. It is never logged: a `tracing` call near the spawn must not format `env`.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`, then `cargo test -j 4 -p marshell-protocol && pnpm check:generated`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(agents): start Claude Code on DeepSeek from a preset`.

### Task 6: Claude hooks fired by another agent are dropped

**Files:**
- Modify: `crates/hook/src/lib.rs` (`run_with`), `crates/core/src/server/hook.rs` (`post`), `crates/core/src/registry.rs` (a read of the row's effective agent)
- Test: `crates/hook` unit tests, plus `crates/core/tests/hook_api.rs` or wherever `post` is tested

**Interfaces:**
- Produces: `Registry::effective_agent(&self, tab: &str) -> Option<String>`, which is `info.agent`, or else `info.foreground.agent`.

- [ ] **Step 1: Write the failing tests**:
  - `grok_hook_env_makes_the_claude_bridge_exit_quietly`: `run_with` with `--agent claude`, a permission role, and the env holding `GROK_HOOK_EVENT=pre_tool_use` and a tab id. No post is made, and the output is empty. With `--agent codex`, the post is made as before.
  - `a_claude_event_from_a_grok_tab_is_dropped_before_any_hold`: a row launched as `grok`, then a PreToolUse permission post from `claude` for that tab. The answer is 204 at once, no hold is opened, the row's `live` stays `None`, and no history is written.
  - `a_claude_event_from_a_claude_tab_still_folds`: the regression guard.
  - `a_claude_event_from_a_shell_tab_running_grok_is_dropped`: a foreground agent of `grok`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-hook` and the core test. Expected: FAIL.
- [ ] **Step 3: Implement.** In the bridge, check right after the role is known and before reading stdin; the check is only for `args.agent == "claude"`. In the core, check before `fold_live`, `record_history` and `hold_permission`: an effective agent that is `Some(a)` with `a != agent` gives 204. A `None` keeps today's behaviour.
- [ ] **Step 4: Run** `cargo test -j 4 --workspace`. Expected: PASS.
- [ ] **Step 5: Commit** with `fix(hooks): drop Claude hook events that another agent fires, such as Grok`.

### Task 7: Progress and kitty notifications, and the fallback for agents typed in a shell

**Effort: high** (pty scanner).

**Files:**
- Modify: `crates/core/src/pty/osc133.rs`, `crates/core/src/registry.rs` (marks, `activity_candidates`), and the activity watcher that calls `set_activity`
- Test: `osc133.rs` and `registry.rs` unit tests

**Interfaces:**
- Produces: `Mark::Progress(u8)`, the OSC 9;4 state from 0 to 4. A malformed state is ignored.
- OSC 99 kitty notifications: `ESC ] 99 ; <metadata> ; <payload> ST` becomes `Mark::Notify(Some(payload))` when the metadata has no `d=0`, meaning it is the last or only chunk. `p=title` and `p=body` chunks are joined as `title: body`, as OSC 777 is. Payloads with `e=1` (base64) are decoded. Everything else is ignored.

- [ ] **Step 1: Write the failing tests**:
  - `osc_9_4_progress_is_a_mark`. This replaces the asserts at `osc133.rs:343` and `:576`, which expected 9;4 to be ignored. That is a ruling for the ledger: the 6b decision 4 amendment asks for it. Cases: `\x1b]9;4;1;50\x07` gives `Progress(1)`, `9;4;0` gives `Progress(0)`, and `9;4;9` gives nothing. OSC 9 free text still gives `Notify`.
  - `osc_99_notifications`: a single chunk, a `d=0` chunk followed by the final chunk (only the final yields), and a base64 `e=1` payload.
  - `progress_holds_working_until_cleared`: in the registry, for a row with `agent = grok` and no `live`, `Progress(1)` sets `activity = Working` and the watcher's quiet tick doesn't set Idle. `Progress(0)` releases it.
  - `an_agent_typed_in_a_shell_gets_the_fallback`: `activity_candidates` includes a running row whose `foreground.agent` is `Some`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core pty::osc133 registry`. Expected: FAIL.
- [ ] **Step 3: Implement.** Keep the scanner's existing caps: `MAX_CARRY` covers 9;4, and a 99 payload is capped at `MAX_NOTIFY_CHARS`.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: PASS.
- [ ] **Step 5: Commit** with `feat(status): read OSC 9;4 progress and OSC 99 notifications for agents without hooks`.

### Task 8: The launcher, mode chip and rows read the catalog

**Files:**
- Create: `src/lib/api/catalog.ts`
- Modify:
  - `src/features/sessions/launchPrograms.ts` (`livePrograms`, `launchRequest`) and `LiveLauncher.tsx`
  - `src/features/launcher/launchChoice.ts`: the DeepSeek comment, `applyRemembered`, and how `noFirst` and modes are chosen
  - `src/features/modes/sessionModes.ts`: manifest agents' modes and `switchKind`
  - `src/features/sessions/resumeCommand.ts`
  - `src/features/sessions/toRow.ts`
  - `src/components/AgentMark/agents.ts`, only if the lobehub set has a Kimi mark (below)
- Test: each file's existing `*.test.ts(x)`, plus `catalog.test.ts`

**Interfaces:**
- Consumes: `AgentCatalog` and `CatalogPreset` from `src/generated/` (Task 4).
- Produces:
  - `livePrograms(catalog: AgentCatalog, profiles: …)`: one program per found agent (adapters first, then shipped, then own) and per preset, then the shells.
  - `LaunchProgram` gains `modes?: Mode[]`, `preset?: string` and `problem?: string`.
  - `launchRequest` sends `agent: "claude", preset: "deepseek"` for the preset.

- [ ] **Step 1: Write the failing tests**:
  - `livePrograms`: a not-found agent is absent. A found grok is present with the label "Grok". The DeepSeek preset appears with its `problem` when the key is unset, and starting it shows the problem and sends nothing.
  - The launcher: `noFirst` is set for kimi, crush, amp and aider. The mode chip cycles only the program's `modes`. A remembered `auto-edit` falls back for Cursor to `manual`, the first offered mode in `MODES` order. An agent with no modes, such as opencode, shows no chip.
  - `switchKind("grok", …)` returns `"native"` for an offered mode and `"unavailable"` for an unmapped one. `launchArgs` no longer throws for a non-adapter id; audit every `CliId`-typed call site.
  - `resumeCommand`: a grok tab gives `grok --continue`.
  - `toRow`:
    - A manifest agent row without activity reads "Status limited", not "Status arrives with hooks".
    - With activity it reads `Working · Status limited`.
    - A shell row whose foreground is grok, with activity, gets the same fallback.
    - The hook-agent copy for claude and codex is unchanged.
    - A `preset: "deepseek"` row shows the `deepseek` mark.
- [ ] **Step 2: Run** `pnpm test src/features/sessions src/features/launcher src/features/modes`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Kimi mark: if `@lobehub/icons-static-svg` (MIT, already the source of the marks) has a Kimi outline, add `kimi` to `AGENT_IDS` the way the others were added, and set the manifest's `mark` to `kimi` (a one-line edit to Task 1's JSON and its test). Otherwise leave `generic`.
  - Aider and Crush stay `generic`.
- [ ] **Step 4: Run** `pnpm test`, then `pnpm build`. Expected: all pass, and the build is ok.
- [ ] **Step 5: Commit** with `feat(launcher): start manifest agents and the DeepSeek preset from the catalog`.

### Task 9: "Add your own agent" in the Agents dialog

**Files:**
- Create: `src/features/agents/OwnAgentsSection.tsx` and `OwnAgentsSection.test.tsx`
- Modify: `src/features/agents/AgentsDialog.tsx` (mount the section after the hook sections), `src/lib/api/catalog.ts` (`ownAgents.get` and `ownAgents.save`)
- Test: `OwnAgentsSection.test.tsx`, `AgentsDialog.test.tsx`

**Interfaces:**
- Consumes: `OwnAgents`, `OwnAgentsSave` and `OwnAgentProblem` (Task 4).
- The section is a list of own agents, each with Edit and Remove, plus an "Add an agent" button. The form fields:
  - Name.
  - Command, which may be a full path.
  - How it takes a first prompt: "Not at all", "As the last word" or "After a flag", with a flag field for the last.
  - Continue flags, one line, split on spaces, with quotes kept.
  - A flags field for each of the five modes, with blank meaning "not offered" and Manual allowing "no flags".
  - Program names to recognise.

  The id is derived from the name: lowercase, with dashes. On Save the section sends the version it read. A 409 shows "agents.json changed since you opened it. Reload to see the change." with a Reload button. A 422 shows each reason next to its entry. The file's own problems are listed at the top, each with its reason.

- [ ] **Step 1: Write the failing tests** through `renderToStaticMarkup`:
  - The section renders an empty state: "No agents of your own yet." with "Add an agent".
  - It renders a file problem's reason.
  - The form's fields have labels and a keyboard order.
  - The id is derived from the name ("My Agent" gives `my-agent`).
  - Building an `OwnAgentsSave` from the form gives the expected manifest. "After a flag" with `-p` gives `{style:"flag", flag:"-p"}`.
  - A 409 response renders the reload copy.
- [ ] **Step 2: Run** `pnpm test src/features/agents`. Expected: FAIL.
- [ ] **Step 3: Implement.** Follow `frontend-design`, and read the good-css skill before writing declarations. Use tokens only and fill-based fields, no outlines. Mark errors with `aria-invalid` and `aria-describedby`. Announce through the shared `Announcer`.
- [ ] **Step 4: Run** `pnpm test`, then `pnpm build`. Expected: pass and ok.
- [ ] **Step 5: Commit** with `feat(agents): add your own agent from the Agents dialog`.

### Task 10: Live check and docs

Uses the `marshell-live-check` skill. The scratch `MARSHELL_HOME`, scratch `CLAUDE_CONFIG_DIR` and the CDP port are as that skill sets them.

- [ ] **Step 1: Prepare the scratch installs** (6b decision 8). Create a fresh literal scratch folder in the session scratchpad. Install `opencode-ai`, `@qwen-code/qwen-code` and Crush (npm `@charmland/crush`, or the official Windows release if npm lacks one) into an npm prefix inside it. Install Kimi Code CLI by its documented Windows method into that folder. These run vendor install scripts; record each command. Put the prefix first on PATH for the dev app only.
- [ ] **Step 2: Check flags.** For each installed CLI, run `--help` and `--version` and compare against Task 1's table: the prompt style, continue and every mapped mode flag. A mismatch is a ruling and a one-line edit to `manifests.json`, with its test updated. Record `kimi --version` output.
- [ ] **Step 3: Real starts without signing in.** From the launcher, start each in a scratch project folder in Manual and in one other offered mode. Record that the argv shown in the tab matches; the catalog says found; the row says "Status limited"; and Resume restarts with the continue flag. Record which terminal signals each emits inside Marshell (OSC 9;4, 9, 99, 777, bell), using `browser_evaluate` on the row and a debug read of the marks.
- [ ] **Step 4: Grok on the person's account.** In a scratch project folder:
  - Start Grok in Manual, and check that `--permission-mode default` is in the argv and that Grok asks before a tool.
  - Ask it something that runs a command. The row must show no Claude-shaped live status and no approve card, and Grok's own prompt appears.
  - Run `grok inspect` in that folder. Record which hooks Grok loaded and their sources, without reading `~/.claude`.
  - Record the real process chain under the tab, to answer whether Grok runs `agent.exe` as a child.
- [ ] **Step 5: Own agent and DeepSeek.**
  - Add an own agent through the dialog that points at one of the scratch CLIs by full path. Start it, and edit the file by hand to see the reload.
  - DeepSeek: without `DEEPSEEK_API_KEY`, the picker shows the "Set DEEPSEEK_API_KEY…" problem. With a key, if the person supplies one, start a session and check the mark and status; otherwise list it under Known limits.
- [ ] **Step 6: Clean up.**
  - Stop what was started, by PID, after listing the tree and checking it for agent CLIs the person started.
  - List for the person the folders the starts created in the real home (`~/.qwen`, `~/.kimi-code`, `~/.local/share/opencode`, a project's `.crush`, and Grok's session folder for the scratch project), and delete only what they approve, by literal path.
- [ ] **Step 7: Write** `docs/research/2026-10-25-agent-manifests-live-check.md` with the results. Add a "Live check" pointer to the research note. Update `launchChoice.ts`'s comment and `docs/IDEAS.md`'s DeepSeek line to the current facts. Commit with `docs(research): record the 6b live check`.

---

## Known limits to carry into the PR

- Copilot's `--continue` falls back to the most recent session anywhere when the folder has none, so Resume can open another folder's conversation. Marshell has no session id for manifest agents.
- A Cursor started as a bare `agent` from a shell gets a neutral mark. The foreground chain has exe names only, and `agent` is also Grok's.
- Copilot, Cursor and Amp are checked from their docs only.
- Amp's continue (`threads continue` with no id) is inferred from its SDK.
- A `DEEPSEEK_API_KEY` set after Marshell started needs a Marshell restart.
