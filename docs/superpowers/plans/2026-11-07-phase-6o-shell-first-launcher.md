# Slice 6o: the shell-first launcher

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The new-session launcher (Ctrl+Shift+T) changes in four ways:
- the person picks the shell first, then an agent CLI that shell can start;
- tool profiles (slice 6c) are gone;
- the session can be named before it starts;
- the launcher's focus look is fill and tint, with no box and no rule.

**Architecture:**
- **The core drops `crates/core/src/tools/`.** Three parts that need no profile move out and keep working:
  - listener port chips go to `crates/core/src/listen.rs`;
  - secret-command detection and secret-flag scrubbing go to `crates/core/src/secrets.rs`.

  Everything a profile fed goes, down to the protocol types and their generated TypeScript.
- **A new route says which agents a shell profile can start:** `GET /v1/profiles/{id}/agents`.
  - A Windows shell answers from the same PATH lookup the catalog uses, plus Command Prompt's refusal of a program path with a space.
  - A WSL distro answers from one `wsl.exe` run of `command -v` inside the distro.
- **The frontend's single Program chip becomes a Shell chip and an Agent chip.** The agent list comes from that route, the Name field takes the footer's "Session N" place, and the launcher's stylesheet gets the new focus look.

**Tech Stack:** Rust (axum, ts-rs), React + TypeScript, and Vitest through `renderToStaticMarkup`. No new dependency.

**Spec:** `docs/PLAN.md` §9, "Slice 6o decisions (2026-11-07)", decisions 1 to 9. They replace 6c decisions 1 and 5 to 9 and the announced-URL half of 6c decision 3. Also binding: PLAN's "Launch a session" bullets, the visual system's "No boxed outlines", and the Accessibility safety net.

**Checked before planning (2026-11-07):**
- An agent session already runs inside a shell profile. `build_argv` and `line_argv` (`profiles/detect.rs`) compose the agent line for every `ShellFamily`. Today the launcher always sends `profile_id = default_id` for an agent, so only the frontend has to change for the shell to be chosen.
- The per-folder memory in `layout.json` is already a pair, `LauncherChoice { profile_id, agent, mode, preset }` (`crates/core/src/layout.rs:49`). Nothing in the stored shape changes.
- `CreateSessionRequest.name` exists (`protocol/src/api.rs:73`), but `launchRequest` never sets it.
  - The core takes a non-empty trimmed name as the base, else `agent_display_name`, else the profile's name (`server/sessions.rs:696`).
  - `agent_display_name` gives Claude Code, Codex, Antigravity, Qwen Code and Copilot CLI, and None for own agents.
  - `adopt_script` appends the session number when an open session already has that name.
- The 2 px accent ring is the app's focus style (`src/styles/base.css:30`). The launcher repeats it on its chips, inputs, Back and Start, and the prompt line adds an accent top edge on `:focus-within`. `Launcher.css.test.ts` pins that edge.
- `Attention.limited` is also set at `registry.rs:1005`, by an agent waiting in its own terminal, so the field stays. Only the tool-prompt path that sets it goes.
- `Foreground.tool` and `SessionInfo.tool` have no reader in `src/`.
- WSL:
  - `run_wsl_capture` (`detect.rs:155`) returns `None` on a timeout and kills the tree.
  - `WSL_SCRIPT` (`server/agents.rs:63`) builds the distro's PATH, without `/mnt/*` and with the usual Linux install folders, then runs `exec "$0" "$@"`.
  - `agent_version_in` (`server/agents.rs:660`) is the existing per-distro probe. Copilot is refused in WSL by `copilot_in_wsl()` (`server/agents.rs:141`) until 6n2.
- Command Prompt: `build_line` refuses a first prompt, and a program word that isn't plain. Copilot runs by its full path (`program_command`, `server/sessions.rs:306`), which has a space on this PC (`C:\Users\Home Office\…`).
- The profiles that kept a login's ports quiet all used the prefixes `login` or `auth login` after the program word: vercel, wrangler, netlify, firebase, supabase, docker, gh, gcloud, aws, az, terraform, fly and stripe.

## Global constraints

- Every agent CLI goes through the same shell check. The one per-CLI difference is Copilot in WSL ("Marshell runs it in WSL later", until 6n2), and the UI says so (AGENTS.md).
- Copy is exact as written in each task: sentence case, no "!", and no "successfully".
- `~/.marshell/tools.json` is never read, written or deleted again. A copy already on disk stays there.
- No new live region. `Announcer` stays the only one. Launcher notes are plain paragraphs, tied to their chip by `aria-describedby`.
- Every UI change works with the keyboard alone and in forced colors, and a forced-colors rule uses system colors only. Colours come from tokens only, with no hex and no `rgb(`, and the launcher uses logical properties only.
- Read the `good-css` skill before writing CSS. Task 6's look also goes through `frontend-design`.
- No test may start a real WSL distro. Each new WSL probe has a `cfg!(test)` guard that panics unless a test pinned it, as `distro_target` does (`server/agents.rs:402`).
- Rust: `cargo fmt --all` before each commit, and `cargo test -p marshell-core -j 4` (never more jobs). After a protocol change, `cargo test -p marshell-protocol` regenerates `src/generated/` and `pnpm check:generated` must pass.
- A new route sits in the token-guarded router and has a "refused without the bearer" test.
- Commit messages: `type(scope): what changed, as a sentence`, ending with the Co-Authored-By line.

## Review focus

1. **A WSL distro that is stopped, or doesn't answer within 8 s.** The agent list shows "Marshell couldn't reach Ubuntu, so only the shell is offered.", Just the shell still starts, and nothing hangs. Tested in Task 3 (`a_wsl_shell_that_doesnt_answer_is_not_reached`) and Task 4 (`an_unreached_shell_offers_only_the_shell_and_can_start`).
2. **A late answer for a shell that is no longer chosen.** Picking Ubuntu and then PowerShell 7 before Ubuntu answers must never fill PowerShell 7's list with Ubuntu's agents, or start Ubuntu's choice. Tested in Task 4 (`an_answer_for_another_shell_never_fills_this_one`).
3. **A remembered choice that no longer fits.** The folder remembers a profile that is gone, or an agent that isn't available in the remembered shell. The launcher falls back to the default shell or to Just the shell, and Start never sends an agent that the current offer lacks. Tested in Task 4 (`preselect_falls_back_when_the_remembered_shell_is_gone`, `a_remembered_agent_the_shell_lacks_becomes_just_the_shell`).
4. **Name edge cases.**
   - Only spaces means the default.
   - Text pasted with control characters, or longer than 120 characters, is cut in the field the way the core cuts it.
   - With the worktree on, an untouched Name follows the task name; a typed name stops following, and clearing it follows again.
   - Tested in Task 5 (`the_name_sent_is_trimmed_and_empty_means_the_default`, `name_follows_the_task_until_typed`).
5. **Own-agent command words with shell metacharacters in the WSL check.** A manifest whose command is `a;rm -rf ~` must reach `sh` as an argument, never as script text. Tested in Task 3 (`the_words_reach_sh_as_arguments`).

---

### Task 1: The frontend stops using tool profiles

**Files:**
- Delete:
  - `src/lib/api/tools.ts`
  - `src/features/agents/ToolsSection.tsx`, `OwnToolsSection.tsx`, `OwnToolsSection.css`, `ownTools.ts`, and their tests and stories
- Modify:
  - `src/features/agents/AgentsSection.tsx` (the Tools list and the own-tools editor at :19 and :544)
  - `src/lib/api/catalog.ts` (own tools, :23–25)
  - `src/features/sessions/launchPrograms.ts` (`TOOL_PREFIX`, the tool presets in `livePrograms`, the `toolLaunch` branch of `launchRequest`, `fieldsFilled`)
  - `src/features/sessions/LiveLauncher.tsx` (`toolsApi.get` at :132, `Loaded.tools`, `fieldValues` and the fields at :249–273)
  - `src/features/launcher/launchChoice.ts`:
    - the `"Tools"` group and the static `tool(...)` entries (python to "Other program");
    - `LaunchProgram.fields` and `toolLaunch`;
    - the `PresetField` import.
  - `src/features/launcher/Launcher.tsx` (the `fields` slot at :570, and the listbox label "Agents, shells and tools", which becomes "Agents and shells")
  - `src/features/launcher/Launcher.stories.tsx` and `fixtures.ts`
- Test:
  - `src/features/sessions/launchPrograms.test.ts`
  - `src/features/launcher/launchChoice.test.ts`, `Launcher.test.tsx`, `Launcher.stories.test.tsx`
  - `src/features/agents/AgentsSection.test.tsx`, `src/features/sessions/launcherSetup.test.ts`, `LiveApp.test.tsx`
- Not touched:
  - `src/generated/` (Task 2 regenerates it);
  - `src/features/ports/` (listener chips stay);
  - `src/components/AgentMark/tools.ts` (shells and the sidebar's program marks still use `ToolId`).

**Interfaces:**
- Produces:
  - `LAUNCH_GROUPS = ["Agents", "Shells"]`;
  - `livePrograms(catalog: AgentCatalog | null, profiles: Profile[]): LaunchProgram[]`, with no third parameter;
  - `launchRequest(choice, prompt, programs, profiles, projects, size, taskPath?, elevated = false)`, with no `fields` parameter.

  After this task no file in `src/` imports `ToolCatalog`, `CatalogTool`, `CatalogToolPreset`, `OwnTools`, `OwnToolsSave`, `OwnToolProblem`, `LaunchPreset`, `PresetField`, `NeedsYouPattern`, `PortPattern`, `PromptKind`, `ToolProfile` or `ToolLaunch`.

- [ ] **Step 1: Write the failing tests.**
  - `launchPrograms.test.ts`: `live_programs_list_no_tools_group`, where a catalog plus two profiles gives only the groups `Agents` and `Shells`. Delete the tool-preset cases (501–545).
  - `Launcher.test.tsx`: `the_program_list_has_no_tools_heading`, where the rendered listbox has no `>Tools<` text and is labelled "Agents and shells".
  - `AgentsSection.test.tsx`: `the_agents_dialog_has_no_tools_list`, with no "Tools" heading and no own-tools editor.
- [ ] **Step 2: Run them.** `pnpm test src/features/sessions/launchPrograms.test.ts src/features/launcher src/features/agents/AgentsSection.test.tsx`. Expected: FAIL on the three new cases.
- [ ] **Step 3: Implement.** Remove the code listed under Files. Then grep `src/` for the UI copy "tool profile", "Tools list", "tools.json" and "own tools" outside `src/generated/`, and remove each hit that belongs to the profiles. The approval cards' MCP "tool" is a different word and stays.
- [ ] **Step 4: Run.** `pnpm test` and `pnpm build`. Expected: PASS, with nothing that imports a removed type.
- [ ] **Step 5: Commit.** `refactor(ui): the launcher and the Agents dialog drop tool profiles`

### Task 2: The core drops tool profiles and keeps three general parts

Worker at `effort: high`, because this is pty and concurrency code.

**Files:**
- Create:
  - `crates/core/src/listen.rs`: moved from `tools/listen.rs`, plus `quiet_line` and `quiet_argv`.
  - `crates/core/src/secrets.rs`: moved from `tools/secrets.rs`.
  - `crates/core/src/watch.rs`: the slimmed `Watch` with `watch_for` and `watch_for_argv`, moved out of `tools/mod.rs`.
- Delete:
  - `crates/core/src/tools/`: `mod.rs`, `profile.rs`, `matcher.rs`, `own.rs` and `profiles.json`;
  - `crates/core/src/server/tools.rs`;
  - `crates/core/tests/tool_presets.rs`;
  - `crates/protocol/src/tools.rs`, except `PortChip`, which moves into `api.rs` beside `SessionInfo`;
  - the generated files of the removed types.
- Modify:
  - `crates/core/src/lib.rs` (modules);
  - `crates/core/src/server/mod.rs`:
    - `mod tools`;
    - `own_tools` and `own_tools_write`;
    - `tools()` and `tools_reader()`;
    - the `/v1/tools` routes (:1303–1304).
  - `crates/core/src/server/sessions.rs`:
    - `TOOL_NO_AGENT` and `resolve_tool`;
    - the `req.tool` branch (:986–998);
    - `on_tool` and `classify` (:855–901);
    - `Resolved.initial_command`;
    - the tests at :2027–2160.
  - `crates/core/src/server/foreground.rs` (`tick` stops reading profiles; it calls `watch::watch_for` and `watch_for_argv` with no `Tools`)
  - `crates/core/src/server/ports.rs` (imports from `crate::listen`)
  - `crates/core/src/pty/session.rs`:
    - `ToolCallback`, `ToolMatcher`, `ToolView` and `tool_view`;
    - `settle_tools`, `emit_tool_events` and `watch_tool`;
    - the `tool` and `tool_gen` output state;
    - the profile half of `set_watch` and `track_command`.

    `record_chunk`, `secret_label`, `secret_now` and `shown_command_line` keep their behaviour.
  - `crates/core/src/registry.rs`:
    - `tool_attention` and `tool_event`;
    - the announced-chip lifetime and `announced_seen` (:838–849);
    - the attention filter (:1029) and the clear on command end (:1094–1095).

    `set_listening` and `Attention.limited` stay.
  - `crates/core/src/agents/settings_view.rs` (`crate::secrets`)
  - `crates/core/src/paths.rs` (`tools_file`)
  - `crates/protocol/src/api.rs`:
    - `CreateSessionRequest.tool` goes.
    - `SessionInfo.tool` and `Foreground.tool` go.
    - `PortChip` stays; its `announced` field goes, since every chip is now a listener's.
  - `crates/protocol/src/lib.rs`
  - `src/features/ports/*` (only what drops `announced`)
- Test: inline tests in the moved files, plus `crates/core/tests/session.rs:685` (rewired to `watch::watch_for`), `pty/session.rs`, `registry.rs`, `server/foreground.rs:450` and `server/ports.rs`

**Interfaces:**
- Consumes: Task 1. Nothing in `src/` imports a removed type.
- Produces:
  - `pub struct Watch { pub secret: bool, pub label: String, pub quiet_ports: bool }`, with `differs(&self, &Watch) -> bool`.
  - `pub fn watch_for(line: &str) -> Watch` and `pub fn watch_for_argv(argv: &[String]) -> Watch`.
  - `pub fn quiet_line(line: &str) -> bool` and `pub fn quiet_argv(argv: &[String]) -> bool`. True when the words after the program word start with `login` or `auth login`. A runner and its package come first and are skipped: `npx <pkg>`, `bunx <pkg>`, `pnpm dlx <pkg>`, `pnpm exec <pkg>`, `npm exec <pkg>`.
  - `crate::secrets::{scrub, shown, shown_words, prints_secrets, prints_secrets_words, takes_secret_words}`, unchanged.

- [ ] **Step 1: Write the failing tests.**
  - In `listen.rs`, `quiet_commands_are_logins_after_the_program_word`:
    - quiet: `vercel login`, `npx vercel login`, `pnpm dlx netlify-cli login`, `gh auth login`, `gcloud auth login --no-launch-browser`, `docker login -u x`;
    - not quiet: `vercel dev`, `npx vite`, `docker compose up`, `login`, `gh auth status`.
  - `quiet_argv` gets the same cases, as argv.
  - In `watch.rs`:
    - `a_secret_command_is_hidden_with_its_label`: `op read op://v/i/f` gives `secret` true and a label without the reference.
    - `a_plain_dev_server_is_neither_secret_nor_quiet`: `npx vite`.
  - In `server/foreground.rs`, rewrite the `vercel login` test so it asserts `quiet_ports` from the static rule.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 quiet` (and `watch`). Expected: FAIL to compile, because `crate::listen::quiet_line` and `crate::watch` don't exist yet.
- [ ] **Step 3: Implement.**
  - Move the files and slim `Watch`.
  - Delete the profile code listed under Files, then the tests that only covered profile events. Keep every listener-chip and secret-hiding test, rewired to the new paths.
  - Run `cargo test -p marshell-protocol` to regenerate, and delete the generated files of the removed types.
- [ ] **Step 4: Run.**
  - `cargo test --workspace -j 4`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `pnpm check:generated`, `pnpm test`, `pnpm build`

  Expected: PASS, and `rg -n "tools::|ToolEvent|tool_attention|tools_file" crates src src-tauri` prints nothing.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `refactor(core): tool profiles are gone, and listener port chips and secret handling move out of the tools module`

### Task 3: The core says which agents a shell can start

Worker at `effort: high`, because this adds a new route and a probe that runs a process.

**Files:**
- Modify:
  - `crates/protocol/src/api.rs` (new types beside `Profile`)
  - `crates/core/src/server/profiles.rs` (handler `agents`)
  - `crates/core/src/server/mod.rs`:
    - the route `GET /v1/profiles/{id}/agents` in the token-guarded router;
    - the pin `distro_which: Option<fn(&str, &[String]) -> Option<Vec<String>>>`;
    - the builder `with_distro_which`.
  - `crates/core/src/server/agents.rs`: split `WSL_SCRIPT` into a `WSL_PATH` prelude and `exec "$0" "$@"`, add `wsl_which_argv`, and add `AppState::agents_in_distro`.
  - `crates/core/src/profiles/detect.rs` (`is_plain_word` becomes `pub(crate)`)
  - `src/lib/api/profiles.ts` (`agents`)
- Test: inline in `server/profiles.rs` and `server/agents.rs`, and `crates/core/tests/catalog_api.rs`, whose seams for `found` are reused

**Interfaces:**
- Produces:
  - Protocol types (ts-rs exported, snake_case):

    ```rust
    pub struct ShellAgents { pub profile_id: String, pub reached: bool, pub agents: Vec<ShellAgent> }
    pub struct ShellAgent { pub id: String, pub problem: Option<ShellAgentProblem> }
    pub enum ShellAgentProblem { NotInstalled, NotDrivenHere, PathHasSpace }
    ```

    `agents` has one entry per catalog agent (adapters and own manifests, in catalog order, presets excluded). `reached: false` comes with an empty `agents`.
  - `GET /v1/profiles/{id}/agents` returns `ShellAgents`. An unknown id gets 404 "No such shell profile."
  - `pub(crate) fn wsl_which_argv(distro: &str, words: &[String]) -> Vec<String>`:
    - The argv is `-d <distro> --exec sh -c <WSL_PATH; for c do command -v "$c" >/dev/null 2>&1 && printf '%s\n' "$c"; done; exit 0> sh <words…>`.
    - The words go only after `sh`.
  - `AppState::agents_in_distro(&self, distro: &str, words: &[String]) -> Option<Vec<String>>`:
    - It returns the pinned answer when one is set, panics under `cfg!(test)` when none is, and otherwise runs `run_wsl_capture` with `WSL_VERSION_TIMEOUT`.
    - `None` means no answer or a non-zero exit. `Some` holds the found words (UTF-8 lines).
    - It keeps no cache: a CLI installed a moment ago shows up on the next launcher open.
- Rules in the handler (inside `spawn_blocking`, with the profiles lock dropped before the probe):
  - **Windows families (PowerShell, Cmd, Posix):**
    - An agent not found the way `catalog::build` decides `found`, with `st.agent_finder(&profile.env)`, gets `NotInstalled`.
    - In `Cmd`, an agent whose program word from `program_command` isn't plain (`is_plain_word`) gets `PathHasSpace`. Today that's Copilot under a profile path with a space.
    - Everything else gets None.
  - **`Wsl`:**
    - The distro comes from `wsl::distro_of_argv(&profile.command)`.
    - Copilot gets `NotDrivenHere` and is not probed.
    - The adapters claude, codex, antigravity and qwen (words from `agent_command`) and the own manifests (`commands.first()`) go into one `agents_in_distro` call. A word it returns gets None, and every other word gets `NotInstalled`.
    - `None` from the probe gives `reached: false`.

- [ ] **Step 1: Write the failing tests.**
  - `a_wsl_shell_offers_the_driven_clis_its_path_has`:
    - pin `["claude","qwen"]`;
    - claude and qwen None, codex and antigravity `NotInstalled`, copilot `NotDrivenHere`;
    - `reached` true;
    - the pinned fn never receives `copilot`.
  - `a_wsl_shell_that_doesnt_answer_is_not_reached`: pin `None` gives `reached` false and `agents` `[]`.
  - `the_words_reach_sh_as_arguments`: `wsl_which_argv("Ubuntu", ["claude", "a;rm -rf ~"])` ends with `["sh", "claude", "a;rm -rf ~"]`, and the `-c` script contains neither word.
  - `command_prompt_refuses_an_agent_whose_path_has_a_space`: Copilot's exe pinned to `C:\Users\A B\copilot.exe` gives `PathHasSpace` in a `cmd` profile and None in a `pwsh` profile.
  - `a_windows_shell_marks_an_agent_off_path_not_installed`
  - `an_unknown_profile_is_404`
  - `shell_agents_are_refused_without_the_bearer`
- [ ] **Step 2: Run them.** `cargo test -p marshell-core -j 4 shell_agents` (and the names above). Expected: FAIL to compile.
- [ ] **Step 3: Implement** the types, route, handler, probe and pin. Add `agents(endpoint: Endpoint, profileId: string): Promise<ShellAgents>` to `src/lib/api/profiles.ts` beside `get`, with the id `encodeURIComponent`-ed.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`, `cargo test -p marshell-protocol`, `pnpm check:generated` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `feat(core): the core says which agents a shell profile can start, checking a WSL distro's own PATH`

### Task 4: The launcher picks the shell first

**Files:**
- Create: `src/features/launcher/shellAgents.ts` and `shellAgents.test.ts`
- Modify:
  - `src/features/launcher/launchChoice.ts`:
    - `LauncherChoice`, `LaunchShell` and `LAUNCH_SHELLS`;
    - `LAUNCH_PROGRAMS` becomes `LAUNCH_AGENTS`, agents only;
    - `launcherSentence`, `isAgentProgram` and `applyRemembered`.
  - `src/features/launcher/Launcher.tsx`: two chips, two listboxes, the missing line, the checking and unreached notes, and the Command Prompt note.
  - `src/features/launcher/launcherKeys.ts` (`OpenList` becomes `"shells" | "agents" | "folder"`; `openPrograms` becomes `openList(which)`)
  - `src/features/launcher/Launcher.css`: the second chip and list reuse `.launcher__chip`, `.launcher__programs` and `.launcher__program`, with no new look.
  - `src/features/sessions/launchPrograms.ts`:
    - `livePrograms` splits into `liveAgents(catalog)` and `liveShells(profiles)`;
    - `launchRequest`, `preselect` and `nameSource`.
  - `src/features/sessions/LiveLauncher.tsx`: shell state, and the per-open memo of answers keyed by profile id.
  - `src/features/sessions/launcherSetup.ts`: `wsl` comes from the chosen shell's family.
  - The launcher's stories and `fixtures.ts`.
- Test:
  - `shellAgents.test.ts`
  - `src/features/launcher/Launcher.test.tsx`, `launchChoice.test.ts`, `launcherKeys.test.ts`, `Launcher.stories.test.tsx`
  - `src/features/sessions/launchPrograms.test.ts`, `launcherSetup.test.ts`

**Interfaces:**
- Consumes: Task 3's `ShellAgents`, `ShellAgentProblem` and `profilesApi.agents`.
- Produces:
  - `type LaunchShell = { id: string; label: string; kind: ToolId; family: ShellFamily }`.
  - `type LauncherChoice = { shell: string; agent: ProgramId | null; projectId: string; mode: Mode; pace: Pace; worktree: boolean }`, where a null `agent` means Just the shell.
  - `type ShellOffer = { state: "checking" } | { state: "unreached" } | { state: "ready"; offered: LaunchProgram[]; missing: { label: string; problem: ShellAgentProblem }[] }`.
  - `offerFor(agents: LaunchProgram[], answer: ShellAgents | "failed" | undefined, shellId: string): ShellOffer`:
    - `undefined` gives checking, and `"failed"` (the request failed) or `reached: false` gives unreached.
    - An answer whose `profile_id !== shellId` also gives checking.
    - A preset (`preset:deepseek`) is offered exactly when its `agent` is.
  - `missingLine(shellLabel: string, missing): string | null`
  - `effectiveAgent(choice, offer): ProgramId | null`: the chosen agent only while it is in `offer.offered`. While checking, it is the chosen agent, and Start is disabled.
- Copy (exact):
  - The sentence reads `[Shell] running [Agent] in [folder]`. With no agent it reads `[Shell] with [no agent] in [folder]`.
  - Chip labels: `Shell: <label>`, and `Agent: <label>` or `Agent: none`.
  - Listbox labels: `Shells` and `Agents in <shell>`. The first agent option is `Just the shell`.
  - The missing line is `Not in <shell>: <label> (<reason>), <label> (<reason>)`. The reasons:
    - `not_installed` reads `not installed`;
    - `not_driven_here` reads `Marshell runs it in WSL later`;
    - `path_has_space` reads `its path has a space, which Command Prompt can't start`.
  - Checking: `Checking which agents <shell> has…`
  - Unreached: `Marshell couldn't reach <shell>, so only the shell is offered.`
  - In Command Prompt with an agent, the prompt line is replaced by `Command Prompt can't pass a first prompt. Type it once <agent> starts.`
- Behaviour:
  - The default shell is `ProfilesDoc.default_id`.
  - `preselect` maps the remembered `{profile_id, agent, preset}` to `{ shell, agent }`. The shell falls back to `default_id` when the profile is gone, and a preset becomes `preset:<id>`.
  - Picking a shell keeps the agent. Once the answer is ready, `effectiveAgent` drops an agent the shell lacks.
  - `launchRequest` sends `profile_id: choice.shell`, plus the agent or preset, the mode and the prompt as today. It sends no prompt in `cmd`, and with no agent it sends `profile_id` only.
  - LiveLauncher asks `profilesApi.agents` once per shell per open. Each answer is stored under its own `profile_id`, and the shown offer is read only under the chosen shell's id.
  - The mode and pace chips show only with an agent. Letter-pick, arrows, Enter and Esc behave on both chips as on today's Program chip.

- [ ] **Step 1: Write the failing tests.**
  - `shellAgents.test.ts`:
    - `an_unanswered_shell_is_checking`;
    - `an_unreached_shell_offers_only_the_shell_and_can_start`: the state is unreached, and `effectiveAgent` is null for a chosen claude;
    - `a_preset_follows_its_agent`;
    - `missing_agents_keep_catalog_order_and_exact_reasons`: the full line for Ubuntu, with Antigravity not installed and GitHub Copilot `not_driven_here`;
    - `an_answer_for_another_shell_never_fills_this_one`;
    - `a_remembered_agent_the_shell_lacks_becomes_just_the_shell`.
  - `launchPrograms.test.ts`:
    - `an_agent_launch_sends_the_chosen_shell`;
    - `just_the_shell_sends_only_the_profile`;
    - `command_prompt_sends_no_prompt`;
    - `preselect_falls_back_when_the_remembered_shell_is_gone`;
    - `a_remembered_preset_preselects_the_preset_agent`.
  - `Launcher.test.tsx`:
    - `the_sentence_starts_with_the_shell_then_the_agent` (the order of the two comboboxes and their labels);
    - `the_agent_list_starts_with_just_the_shell`;
    - `the_missing_line_sits_under_the_agent_list_outside_the_listbox`;
    - `checking_disables_start_and_describes_the_agent_chip`;
    - `command_prompt_replaces_the_prompt_line_with_its_note`;
    - `with_no_agent_the_sentence_reads_with_no_agent`;
    - `an_unreached_shell_shows_its_note_and_start_stays_enabled`.
  - `launcherKeys.test.ts`: `opening_one_list_closes_the_others`, covering the three lists.
- [ ] **Step 2: Run them.** `pnpm test src/features/launcher src/features/sessions`. Expected: FAIL.
- [ ] **Step 3: Implement** to the interfaces and copy above. Rewrite the existing launcher tests that drove the Program chip so they drive the two chips; don't drop what they assert.
- [ ] **Step 4: Run.** `pnpm test` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(ui): the launcher picks the shell first, then an agent that shell can start`

### Task 5: The session's name in the launcher

**Files:**
- Modify:
  - `src/features/launcher/Launcher.tsx`: the footer's `launcher__next` "Session N" becomes the Name field.
  - `src/features/launcher/launcherKeys.ts` (`nameKey`)
  - `src/features/launcher/launchChoice.ts` (`defaultSessionName`)
  - `src/features/sessions/LiveLauncher.tsx` (`nameEdit` state and the request)
  - `src/features/sessions/launchPrograms.ts` (`launchRequest` takes `name`)
  - `src/features/launcher/Launcher.css` (layout only; Task 6 styles focus)
- Test: `Launcher.test.tsx`, `launcherKeys.test.ts`, `launchChoice.test.ts`, `launchPrograms.test.ts`

**Interfaces:**
- Consumes: Task 4's `LauncherChoice` and `effectiveAgent`.
- Produces:
  - `defaultSessionName(agent: ProgramId | null, agents: LaunchProgram[], shell: LaunchShell): string`. It mirrors `agent_display_name` (`server/sessions.rs:39`): claude gives Claude Code, codex Codex, antigravity Antigravity, qwen Qwen Code and copilot Copilot CLI. A preset takes its agent's name. Anything else gives the shell's label.
  - `nameKey(e: KeyboardEvent): boolean`: Alt plus the physical `KeyN`, with no Ctrl, Meta or Shift, as `adminKey` does.
  - `LauncherView` gains `name: string` (what was typed, `""` when untouched) and `namePlaceholder: string`. Props gain `onName(value: string)`.
  - `launchRequest(…, name?: string)` sets `name` only when the trimmed value isn't empty.
- Field:
  - It is `<input>` labelled `Name`, with `maxLength={120}`, `spellCheck={false}`, `autoComplete="off"` and `aria-keyshortcuts="Alt+N"`.
  - It sits beside Start, after the prompt line, the Task name and the Bypass acknowledgement in DOM order. Alt+N focuses it from anywhere in the sheet.
  - Control characters are dropped on input, as `label_text` does.
- With the worktree chip on:
  - The placeholder is the Task name while Name is empty. An empty Name sends the task name, so the session is named after the task.
  - A typed Name wins over the task name. Clearing it follows the task again.

- [ ] **Step 1: Write the failing tests.**
  - `the_footer_has_a_name_field_where_session_n_was` (labelled `Name`, the placeholder `Claude Code` for claude in PowerShell 7, and no `launcher__next`);
  - `the_name_comes_after_the_prompt_in_tab_order`;
  - `alt_n_is_the_name_key` (`KeyN` with Alt is true; with Ctrl+Alt it is false);
  - `the_name_sent_is_trimmed_and_empty_means_the_default` (`"  "` sends no name, `" fix "` sends `"fix"`, a tab or newline is dropped);
  - `name_follows_the_task_until_typed`;
  - `an_own_agent_defaults_to_the_shell_name`.
- [ ] **Step 2: Run them.** `pnpm test src/features/launcher src/features/sessions`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `pnpm test` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(ui): a session can be named in the launcher before it starts`

### Task 6: The launcher's focus look (main thread)

**Files:**
- Modify:
  - `src/features/launcher/Launcher.css`
  - `src/features/launcher/Launcher.css.test.ts`
  - `docs/PLAN.md`: the visual system's "No boxed outlines" line records the look once it is settled.
- Test: `Launcher.css.test.ts`, `src/styles/noBoxedOutlines.test.ts`, `forcedColorsEdges.test.ts` and `tokensOnly.test.ts`

**Interfaces:**
- Consumes: the class names from Tasks 4 and 5, among them the Name field's class.
- Produces: no TypeScript interface.

- [ ] **Step 1: Run the skills.** Run `good-css`, then `frontend-design`, with decision 9 as the brief:
  - fill and tint only: no outline box, and no accent rule on the prompt line;
  - an accent caret in text fields;
  - each focusable thing's focus differs from both its rest and its hover state;
  - forced colors: `outline: 2px solid Highlight` on each focus selector.
- [ ] **Step 2: Write the failing tests** in `Launcher.css.test.ts`:
  - `focus_in_the_launcher_is_fill_and_tint_not_a_ring`: outside the forced-colors block, no launcher `:focus-visible` or `:focus-within` rule sets a visible `outline` colour or a `border-block-start-color`, and each sets a `background`.
  - `forced_colors_draws_focus_with_highlight`: each of those selectors has `outline: 2px solid Highlight` in the forced-colors block.
  - Replace the old test that pins the prompt line's accent edge on focus.
  - The rule `not.toMatch(/outline:\s*none/)` stays. A focus rule hides the base ring with `outline-color: transparent`, so forced colors still forces a visible outline.
- [ ] **Step 3: Run them.** `pnpm test src/features/launcher/Launcher.css.test.ts`. Expected: FAIL.
- [ ] **Step 4: Implement** the look from Step 1 in `Launcher.css` only. Then record it in PLAN's visual-system line in one sentence.
- [ ] **Step 5: Run.** `pnpm test src/styles src/features/launcher`, then `pnpm test` and `pnpm build`. Expected: PASS.
- [ ] **Step 6: Commit.** `style(launcher): focus in the launcher is fill and tint, with a Highlight outline in forced colors`

### Task 7: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder under `%TEMP%`. Over CDP, with `browser_evaluate` returning small JSON values:
  - **The shell list.** Ctrl+Shift+T lists PowerShell 7, Windows PowerShell, Command Prompt, Git Bash and each WSL distro.
  - **Command Prompt.** With Command Prompt chosen, Copilot is in the missing line as `its path has a space, which Command Prompt can't start`. Choosing Claude Code replaces the prompt line with its note, and the started tab's argv is `cmd.exe /k claude …`.
  - **Git Bash and PowerShell 7.** Each starts an agent, and the argv shows that shell's wrapper.
  - **WSL.** Ask the user first, because picking a distro starts it; the probe only runs `command -v`. After picking it:
    - the checking note shows, then the offer;
    - Copilot reads `Marshell runs it in WSL later`;
    - a CLI not installed there reads `not installed`.
  - **Name.**
    - A typed name reaches the sidebar row, and an empty one gives the default.
    - Alt+N moves focus to the field.
    - With the worktree on, the row is named after the task.
  - **Tools are gone.** The launcher has no Tools group, and the Agents dialog has no Tools list. `python -m http.server 8765` in a shell tab still gets a port chip.
  - **Secret handling.** `kubectl get secret x -o yaml` still gets the "Output hidden" marker in the recording. It is harmless without kubectl, since detection reads the command line.
  - **Focus look.** Screenshot the launcher with the keyboard focus on a chip, the prompt line and the Name field, in normal colours and with `forced-colors: active` emulated. The user judges the look (decision 9).
- [ ] **Step 2:** Write `docs/research/2026-11-07-shell-first-launcher-live-check.md`: each check with its evidence, and the findings numbered.
- [ ] **Step 3:** List the process tree and look for agent CLIs the person started. Then stop the check by PID, and ask before removing the scratch folder.
- [ ] **Step 4:** Commit: `docs(research): record slice 6o's live check`. Then run the final whole-branch review (marshell-reviewer, Opus), one fix pass, and `marshell-ship`.
