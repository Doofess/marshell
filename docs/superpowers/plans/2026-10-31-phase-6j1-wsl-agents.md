# Agents inside WSL (slice 6j1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude Code, Codex and Antigravity running inside a WSL distro get the features they have on Windows: live status on the row, approvals, references and History. A person sets up each distro's hooks from Settings, and agents in a bare distro can open their sign-in page.

**Architecture:**
- **What already works.** The hook path works today with no change (spike S11). A Linux CLI runs the Windows `marshell-hook.exe` through interop. That exe inherits the tab's `MARSHELL_TAB_ID` and `MARSHELL_HOME` from the `wsl.exe` session, and posts to the core over Windows loopback.
- **The gaps** this slice closes:
  - The core never learns which agent runs in a WSL tab.
  - Hook bodies carry Linux paths.
  - Setup writes only the Windows homes.
  - The version gate asks the Windows CLI.
  - A bare distro has no browser opener.
- **The agent in a WSL tab:**
  - A hook from a WSL tab sets the row's `foreground` to that agent (decision 5), so every existing consumer works unchanged: `effective_agent`, `runs_agent`, attachments, the row's mark.
  - The shell's prompt mark already clears `foreground`, and SessionEnd clears it too.
  - The Windows foreground watcher leaves a hook-set foreground alone.
- **Paths:** a new `wsl` module translates between Linux paths and the Windows paths the core can read. `/mnt/<d>/…` maps to `<D>:\…`, and anything else to `\\wsl.localhost\<distro>\…`.
- **Setup:** it reuses each CLI's `install_plan`, with the target files under the distro's home (read through UNC) and the bridge given as its Linux path.

**Tech Stack:** Rust (axum core, `marshell-hook` bridge), ts-rs protocol types, React + Vitest, POSIX sh (shell integration).

**Spec:** `docs/PLAN.md` §9 "Slice 6j1 decisions (2026-10-31)" (binding), spike `docs/spikes/S11-wsl-agents.md`.

## Global Constraints

- **Multi-CLI:** Claude Code, Codex and Antigravity are treated alike. Per-CLI file shapes stay in `agents/claude`, `agents/codex` and `agents/antigravity`. The `wsl` module is CLI-neutral.
- **Config writes are safety-critical:**
  - Every distro write goes through the existing config service: preview, backup, atomic write, `managed.json` and undo.
  - Nothing is written without the person's Apply.
  - The target path comes from the distro's home, never from the request.
  - A distro name in a request is accepted only if it equals a detected WSL profile's distro (case-insensitive). Otherwise the answer is 400 `No WSL distro named "<name>".`
- **No guessed status:** a WSL agent whose version can't be read gets the status note, not live status.
- **Routes and concurrency:**
  - Routes stay in the token-guarded router. A changed route keeps its "refused without the bearer" test, and a new route gets one.
  - Never hold a `MutexGuard` across `.await` or I/O.
  - `wsl.exe` runs and UNC file I/O happen in `spawn_blocking`, with a timeout of 8 s per `wsl.exe` run.
- **Hook bridge:** it depends only on `protocol` and a blocking HTTP client.
- **UI:** works with the keyboard alone and in forced colors; colors from tokens only; the exact copy below; sentence case, no "!".
- **Platform:** WSL is Windows-only. Every `wsl.exe` call and UNC path sits behind `cfg(windows)` or a runtime family check. On other platforms the new code returns None or "not available" and compiles.
- **Commits:**
  - `type(scope): sentence`.
  - Run `cargo fmt --all` before Rust commits, and `cargo` with `-j 4`.
  - Run `pnpm check:generated` after protocol changes.
  - End with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

### Copy (exact)

| Where | Text |
| --- | --- |
| Agents settings, group heading in each hook agent's panel | `WSL` |
| Group help | `Agents you run inside a WSL distro read their own settings there. Set up writes Marshell's hooks into that distro.` |
| Per-distro buttons | `Set up in <distro>` / `Remove from <distro>` |
| Preview reason, Codex in a distro | `Then trust them in Codex's /hooks inside <distro>.` |
| Statusline left alone (Claude Code, agy) | `<distro> already has a status line, so Marshell leaves it. The row won't show the model or context use.` |
| No distros detected | `No WSL distros found.` |
| Refusal, unknown distro (400) | `No WSL distro named "<name>".` |
| Refusal, distro home unreadable (422) | `Couldn't reach <distro>. Start it and try again.` |
| Version note, WSL (row tooltip) | `Can't read <Agent>'s version in <distro>` |

## Review Focus

1. **Odd distro names and paths:**
   - Distro names: `Ubuntu-24.04`, `My Distro` (with a space), a distro differing only in case.
   - Linux paths:
     - relative paths;
     - paths with `..`;
     - `/mnt/` without a drive;
     - `/mnt/wsl/…`;
     - `/mnt/C/…` (an upper-case drive letter);
     - a trailing `/`.
   - Each one either translates exactly or gives None. None never panics and never yields a path outside the distro root. Task 1 tests each.
2. **A hook that arrives after the agent exited.** A late PostToolUse or statusline after the shell's prompt mark must not mark the row again: the row is at its prompt. The mark must also never overwrite a launched agent (`info.agent` is Some). Task 2 tests both.
3. **The watcher racing a hook mark.** A watcher tick that sees nothing in a WSL tab must not clear a foreground the hooks set, and a hook mark must not stick after a restart. Task 2 tests both.
4. **Setup against a stopped or unreachable distro.** Planning must time out or fail with the 422 copy, not hang. An existing non-Marshell `statusLine` in the distro is never replaced. Task 7 tests both.
5. **A History transcript under another distro's or a network UNC path.** It stays refused. Only `\\wsl.localhost\<the tab's distro>\` and `\\wsl$\<the tab's distro>\` are accepted. Task 6 tests it.

---

### Task 1: The `wsl` path module and a tab's distro

**Files:**
- Create: `crates/core/src/wsl.rs` (register in `crates/core/src/lib.rs`)
- Modify: the AppState helpers in `crates/core/src/server/mod.rs` (next to the other per-tab helpers)
- Test: unit tests in `crates/core/src/wsl.rs`

**Interfaces:**
- Produces:
  - `pub fn distro_of_argv(argv: &[String]) -> Option<String>`: the value after `-d`, `--distribution` or `--distribution=` in a `wsl.exe` argv. None when there is none.
  - `pub fn default_distro() -> Option<String>`: the `(Default)` distro, parsed from `wsl.exe --list` (UTF-16LE). Reuse `profiles::detect`'s decoder, and cache the result for the process's life.
  - `pub fn to_windows(distro: &str, linux: &str) -> Option<PathBuf>`:
    - `/mnt/<a-z>` and `/mnt/<a-z>/rest` (drive letter in either case) give `<UPPER>:\rest`.
    - Any other absolute path gives `\\wsl.localhost\<distro>\rest`, with `/` turned into `\`.
    - None for a relative path, an empty path, or any `..` component.
    - A trailing `/` is dropped.
  - `pub fn to_linux(distro: &str, windows: &Path) -> Option<String>`, the inverse:
    - `\\wsl.localhost\<distro>\rest` and `\\wsl$\<distro>\rest` (distro compared case-insensitively) give `/rest`.
    - `<D>:\rest` gives `/mnt/<d>/rest`.
    - Anything else gives None.
  - `pub fn under_distro(distro: &str, path: &Path) -> bool`: true only for paths under `\\wsl.localhost\<distro>\` or `\\wsl$\<distro>\`, distro case-insensitive.
  - `AppState::wsl_distro_of_tab(&self, tab: &str) -> Option<String>`: the tab's profile (from `Relaunch::profile_id`) is looked up. A profile of family `ShellFamily::Wsl` gives `distro_of_argv` of its argv, or `default_distro()` when the argv names none. Every other profile, or an unknown tab, gives None.

- [ ] **Step 1: Write the failing tests**
  - `to_windows_maps_mnt_drives`: `/mnt/c/Users/x` → `C:\Users\x`; `/mnt/D` → `D:\`; `/mnt/C/x` → `C:\x`.
  - `to_windows_maps_the_rest_to_the_distro_unc`: `/tmp/s11/proj` → `\\wsl.localhost\Ubuntu\tmp\s11\proj`; `/mnt/wsl/x` → `\\wsl.localhost\Ubuntu\mnt\wsl\x`; `/` → `\\wsl.localhost\Ubuntu\`.
  - `to_windows_refuses_relative_dotdot_and_empty`: `tmp/x`, `/tmp/../etc`, `` → None.
  - `to_linux_inverts`: `\\wsl.localhost\ubuntu\home\a` with distro `Ubuntu` → `/home/a`; `\\wsl$\Ubuntu\home\a` → `/home/a`; `C:\x\y` → `/mnt/c/x/y`; `\\server\share\x` → None; `\\wsl.localhost\Debian\x` with distro `Ubuntu` → None.
  - `under_distro_accepts_only_that_distro`.
  - `distro_of_argv_reads_each_form`: `["wsl.exe","-d","My Distro"]` → `My Distro`; `--distribution=Ubuntu-24.04` → `Ubuntu-24.04`; `["wsl.exe"]` → None.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core wsl::`. Expected: compile errors, then FAIL.
- [ ] **Step 3: Implement the module and `AppState::wsl_distro_of_tab`.**
- [ ] **Step 4: Run.** Run: `cargo test -j 4 -p marshell-core wsl::`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(wsl): translate between a distro's Linux paths and Windows paths, and find a tab's distro`.

### Task 2: A hook marks the agent running in a WSL tab

**Files:**
- Modify: `crates/core/src/registry.rs` (the `Entry` struct, `set_foreground`, the `mark` handler and restart/resume/exit resets)
- Modify: `crates/core/src/server/hook.rs` (`post`)
- Test: registry unit tests; an integration test in `crates/core/tests/hook.rs`

**Interfaces:**
- Consumes: `AppState::wsl_distro_of_tab` (Task 1).
- Produces:
  - `Registry::mark_hook_agent(&self, tab_id: &str, agent: &str) -> bool`:
    - It sets `info.foreground = Some(Foreground { program: <the agent's exe from ADAPTER_EXES: "claude", "codex", "agy">, command_line: None, agent: Some(agent), tool: None })` and the new `Entry::foreground_from_hooks = true`.
    - It applies only to a Running, not-paused row whose `info.agent` is None and whose `info.command` is not `CommandState::Prompt { .. }`.
    - It broadcasts only on a change and returns whether it changed.
  - `Registry::clear_hook_agent(&self, tab_id: &str, agent: &str)`: clears `foreground` and the flag when the flag is set and the foreground agent equals `agent`.
  - `set_foreground` (the watcher) leaves a row with `foreground_from_hooks` unchanged.
  - Every place that clears `foreground` also clears the flag: the `mark` handler's `command_ended`, restart, resume, `mark_exited`.
  - `hook.rs` `post`: right after `hook_log().push`, for an agent in `claude | codex | antigravity` and a tab with `wsl_distro_of_tab(tab).is_some()`:
    - a body whose `hook_event_name` is `SessionEnd` calls `clear_hook_agent`;
    - anything else of role Event, Permission or Statusline calls `mark_hook_agent`.
    - The existing mismatch check (line ~109) then runs on the updated `effective_agent`.

- [ ] **Step 1: Write the failing registry tests**
  - `a_hook_marks_a_running_shell_row`;
  - `a_hook_mark_is_refused_at_the_prompt_for_a_launched_agent_and_for_a_paused_row`;
  - `the_watcher_leaves_a_hook_mark_alone`: `set_foreground(None)` after a mark keeps it;
  - `the_prompt_mark_clears_a_hook_mark`: `Mark::PromptStart`, then `foreground` None and the flag cleared, and a later `set_foreground(Some(..))` applies again;
  - `restart_clears_a_hook_mark`;
  - `clear_hook_agent_clears_only_its_own_agent`.
- [ ] **Step 2: Write the failing integration tests** in `crates/core/tests/hook.rs`. Use a session registered with a WSL profile (argv `["wsl.exe","-d","Ubuntu"]`, family Wsl) and a POSIX shell profile for the negative case.
  - `a_wsl_tab_takes_the_agent_from_its_hooks`: a Claude `UserPromptSubmit` with header tab → the listed session's `foreground.agent == Some("claude")`, and `live` set.
  - `session_end_clears_the_wsl_agent`.
  - `a_non_wsl_shell_tab_is_not_marked_by_hooks`.
  - `another_agents_hook_in_a_marked_wsl_tab_is_ignored`: marked claude, then a codex event → 204, and the row stays claude.
- [ ] **Step 3: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core hook_agent` and `cargo test -j 4 -p marshell-core --test hook wsl`. Expected: FAIL.
- [ ] **Step 4: Implement.**
- [ ] **Step 5: Run** `cargo test -j 4 -p marshell-core`. Expected: all pass.
- [ ] **Step 6: Commit** `feat(sessions): a WSL tab's hooks tell the core which agent runs in it`.

### Task 3: A shell row shows an agent's live status

**Files:**
- Modify: `src/features/sessions/toRow.ts` (the `fg !== null` branch, ~line 197)
- Test: `src/features/sessions/toRow.test.ts`

**Interfaces:**
- Consumes: `SessionInfo.foreground.agent` and `SessionInfo.live` (unchanged types).
- Produces: in the `fg !== null` branch, a hook agent's row behaves the same in a WSL tab and on Windows (Ruling: a hand-typed agent on Windows today shows only "Status arrives with hooks" even when its hooks report; decision 5 needs this path, and the multi-CLI rule asks for the same behavior):
  - A foreground hook agent (`isHookAgent(fg.agent)`) with `info.live !== null` returns `{ ...base, ...liveRow(info.live, info.stuck, now) }`.
  - Without `live`, it keeps `"Status arrives with hooks"`.
  - `SessionInfo.running()`-style "ended" stays the core's concern; a `live.state === "ended"` row with a foreground still shows the live row.

- [ ] **Step 1: Write the failing tests**
  - `a_shell_row_running_a_hooked_agent_shows_its_live_status`: `info({ foreground: fg("claude","claude"), live: live({ state: "working", phrase: "Running \`npm test\`" }) })` → `{ status: "working", phrase: "Running \`npm test\`" }`, with the row's `agent` `"claude"`.
  - `without_live_it_still_waits_for_hooks`: the same without `live` → `phrase: "Status arrives with hooks"`.
  - `a_manifest_agent_in_front_keeps_the_fallback`: unchanged behavior, pinned.
- [ ] **Step 2: Run** `pnpm test src/features/sessions/toRow.test.ts`. Expected: the first FAILs.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same. Expected: PASS.
- [ ] **Step 5: Commit** `feat(sessions): a shell row running a hooked agent shows the agent's live status`.

### Task 4: The version gate asks the distro

**Files:**
- Modify: `crates/core/src/server/agents.rs` (next to `agent_version`), `crates/core/src/server/hook.rs` (`fold_live_with`)
- Test: unit tests beside the new function; `fold_live_with` through a test seam

**Interfaces:**
- Consumes: `AppState::wsl_distro_of_tab` (Task 1).
- Produces:
  - `AppState::agent_version_in(&self, distro: &str, agent: &str) -> Option<String>`, cached per `(distro lowercased, agent)` for 10 minutes. The run:
    ```
    wsl.exe -d <distro> --exec sh -c 'PATH="$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.bun/bin:$(ls -d "$HOME"/.nvm/versions/node/*/bin 2>/dev/null | tr "\n" :)$PATH"; exec "$0" --version' <exe>
    ```
    - `<exe>` is the constant from `ADAPTER_EXES`, never input.
    - It runs with an 8 s timeout and the first line of stdout parsed as `agent_version` does.
    - A test seam (a `fn` pointer or a trait on AppState, matching how `agent_version` is faked in tests today) replaces the run.
  - `fold_live_with`:
    - For a WSL tab, the version comes from `agent_version_in`. Otherwise it is unchanged.
    - For a WSL tab whose version is None, the note is `Can't read <Agent>'s version in <distro>`. The note uses the display names `Claude Code`, `Codex` and `Antigravity`, as `fold_live_with` names them.
    - Every other `Gate::Unknown` text is unchanged.

- [ ] **Step 1: Write the failing tests:** `a_wsl_tab_gates_on_the_distros_version` (the seam returns a recorded version, then live is folded) and `a_wsl_tab_without_a_version_gets_the_wsl_note` (the seam returns None, then `status_note == "Can't read Claude Code's version in Ubuntu"` and `live` None).
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core version_in`.
- [ ] **Step 3: Implement**, running the `wsl.exe` call in `spawn_blocking`.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(agents): a WSL agent's live status is gated on the version inside its distro`.

### Task 5: References in a WSL tab

**Files:**
- Modify: `crates/core/src/server/hook.rs` (`references_for`, the `note_tab` filter), `crates/core/src/references.rs` (`resolve`)
- Test: `crates/core/src/references.rs` unit tests; `crates/core/tests/hook.rs`

**Interfaces:**
- Consumes: `to_windows`, `to_linux`, `wsl_distro_of_tab` (Task 1); the hook mark (Task 2, so `note_tab`'s `effective_agent == agent` holds).
- Produces:
  - `references_for`:
    - For a WSL tab, the reported cwd (a Linux path from the shell's OSC 7), or else the start folder, goes through `to_windows(distro, cwd)` before `project_of`, `checkout_root` and `load`.
    - When the cwd can't be translated, there is no note.
  - `resolve`:
    - It gains a `render: &dyn Fn(&Path) -> Option<String>` parameter.
    - Each alias path in the note is `render(path)`; an alias whose render is None is dropped like a missing file.
    - Windows tabs pass `|p| Some(p.display().to_string())`. WSL tabs pass `|p| to_linux(&distro, p)`, so the agent in Linux reads `/home/a/proj/src/LoginButton.tsx` or `/mnt/c/…`.

- [ ] **Step 1: Write the failing tests**
  - `resolve_renders_paths_with_the_given_function`: the render maps `C:\p\src\a.ts` to `/mnt/c/p/src/a.ts`, and the note reads `References: 'a' = /mnt/c/p/src/a.ts.`
  - In `tests/hook.rs`, `a_wsl_prompt_gets_linux_paths_in_its_note`:
    - a project under a temp dir, with a WSL tab whose reported cwd is `/mnt/<drive>/<temp path>`;
    - a `UserPromptSubmit` naming the alias, then the answer's context names the `/mnt/...` path.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core references` and `--test hook wsl_prompt`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(references): a WSL tab's prompts get references with Linux paths`.

### Task 6: History records agent sessions run in WSL

**Files:**
- Modify:
  - `crates/core/src/history/mod.rs` (`transcript_allowed`);
  - `crates/core/src/history/recorder.rs` (`note_event`);
  - `crates/core/src/server/hook.rs` (`record_history`);
  - the history sidecar meta type (`archive::read_meta`'s type);
  - `crates/core/src/server/history.rs` (`open_in_new_tab`).
- Test: unit tests in `history/mod.rs` and `history/recorder.rs`; `crates/core/tests/history_api.rs`

**Interfaces:**
- Consumes: Task 1's functions.
- Produces:
  - `transcript_allowed(path: &str, session_id: &str, distro: Option<&str>) -> bool`:
    - It is unchanged for None.
    - With `Some(d)`, a path where `under_distro(d, path)` holds is also accepted. The other checks still apply: a valid id, the file name `<session_id>.jsonl`, and no `..`.
    - Every other UNC path stays refused.
  - `note_event(..., distro: Option<&str>)`:
    - With a distro, the body's `transcript_path` / `transcriptPath` and `cwd` go through `to_windows` before any check, store or copy.
    - The meta records `distro: Option<String>` (serde default None, so old sidecars still read).
  - `record_history` passes `wsl_distro_of_tab(tab)`.
  - `open_in_new_tab`:
    - For a meta with `distro: Some(d)`, the new tab uses the WSL profile whose distro equals `d`. With none, the answer is 409 `No WSL profile for <d>.`
    - The tab starts in `meta.cwd` (the translated Windows path; the directory check stays). `wsl.exe` maps a `\\wsl.localhost\<d>\…` or `C:\…` start folder to its Linux path; the live check verifies it.
    - It runs the same resume command as on Windows.

- [ ] **Step 1: Write the failing tests:**
  - `a_wsl_transcript_under_the_tabs_distro_is_allowed`;
  - `another_distros_and_network_unc_stay_refused`;
  - `note_event_translates_a_wsl_transcript_and_cwd`;
  - `an_old_sidecar_without_distro_still_reads`;
  - in `tests/history_api.rs`, `resume_of_a_wsl_session_opens_its_distros_profile`, faked as the existing resume tests fake launches.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core history`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(history): record and resume agent sessions run inside WSL`.

### Task 7: Set up a distro's hooks (core)

**Files:**
- Modify:
  - `crates/protocol/src/agents.rs` (`HookPlanRequest`);
  - `crates/core/src/server/agents.rs` (`plan`, `remove_plan`, input helpers);
  - `crates/core/src/agents/{claude,codex,antigravity}/hooks.rs` (the bridge path and the chain as parameters);
  - `crates/hook/src/args.rs` and `crates/hook/src/statusline.rs` (`--no-chain`).
- Test: unit tests in each `hooks.rs`; `crates/core/tests/agents_api.rs`; `crates/hook/tests/bridge.rs`

**Interfaces:**
- Consumes: Task 1's functions.
- Produces:
  - `HookPlanRequest { …, #[serde(default)] distro: Option<String> }`. `remove_plan` takes `?distro=<name>` (`Query<DistroQuery { distro: Option<String> }>`).
  - The distro is validated against the detected WSL profiles. Otherwise the answer is 400 (copy).
  - `AppState::distro_target(&self, distro: &str) -> Result<DistroTarget, Fail>`, with `DistroTarget { home: PathBuf, bridge: String }`, run in `spawn_blocking` with an 8 s timeout:
    - `home` is the UNC home: `wsl.exe -d <d> --exec sh -c 'printf %s "$HOME"'`, then `to_windows`.
    - `bridge` is the bridge's Linux path: `wsl.exe -d <d> --exec wslpath -u '<paths::bridge_exe(home) in Windows form>'`.
    - A failure or timeout gives 422 `Couldn't reach <d>. Start it and try again.`
  - Installers:
    - Each `install_plan`/`remove_plan` takes the bridge command text as a parameter (`bridge: &str`) instead of deriving it from `home`. The Windows callers pass today's value, so their output is unchanged, pinned by the existing tests.
    - Claude and agy also take `chain: Option<&Path>`. None means: don't read or write a chain file, and leave an existing non-Marshell `statusLine` in place (`RemovePreview::statusline_left` / the preview reason carries the copy). Marshell's own `statusLine` command gets `--no-chain`.
  - Targets in a distro:
    - Claude: `<home>/.claude/settings.json`.
    - Codex: `<home>/.codex/hooks.json`; the preview reason adds the Codex trust copy.
    - Antigravity: `<home>/.gemini/config/hooks.json` and `<home>/.gemini/antigravity-cli/settings.json`.
  - Bridge: `args::parse` accepts `--no-chain` on the `statusline` role, and `statusline::run` then prints only Marshell's own line, never reading the chain file.
  - The config service and Undo take UNC targets unchanged. `config/write.rs`'s temp-and-rename stays in the target's own folder.

- [ ] **Step 1: Write the failing tests**
  - In each `hooks.rs`: `a_distro_plan_writes_the_linux_bridge` (with bridge `/mnt/c/Users/x/.marshell/bin/marshell-hook.exe`, every entry names it and quotes it as that CLI already quotes); for Claude and agy, `a_distro_plan_leaves_an_existing_statusline` and `a_distro_plan_writes_no_chain_file`.
  - In `crates/hook/tests/bridge.rs`: `statusline_no_chain_skips_the_chain`.
  - In `tests/agents_api.rs`, all through a faked `distro_target`: `plan_refuses_an_unknown_distro` (400 with the copy), `plan_for_a_distro_targets_its_home` (the preview's file paths are under the faked UNC home), and `plan_is_refused_without_the_bearer` with a distro.
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core distro`, `cargo test -j 4 -p marshell-hook no_chain`.
- [ ] **Step 3: Implement.** `distro_target`'s `wsl.exe` runs go through a seam the tests replace.
- [ ] **Step 4: Run** `cargo test -j 4 --workspace` and `pnpm check:generated`. Expected: all pass; generated types updated and committed.
- [ ] **Step 5: Commit** `feat(agents): set up and remove Marshell's hooks inside a WSL distro`.

### Task 8: Set up a distro's hooks (Settings)

**Files:**
- Modify: `src/features/agents/AgentsSection.tsx` (each hook agent's panel), `src/lib/api/agents.ts` (`planHooks(agent, want, distro?)`, `planRemove(agent, distro?)`)
- Test: `src/features/agents/AgentsSection.test.tsx`, `src/lib/api/agents.test.ts`

**Interfaces:**
- Consumes: Task 7's request shapes; the session profiles the app already loads (`family === "wsl"`, with the distro taken from the profile's name as `detect.rs` sets it).
- Produces:
  - In each hook agent's panel (Claude Code, Codex, Antigravity), a `WSL` group below the Windows controls: the help line, then one row per detected distro with `Set up in <distro>` and `Remove from <distro>`. With no distros it reads `No WSL distros found.`
  - Each button opens the same preview flow the Windows buttons use (same component, same Apply/Undo), passing the distro. A 400 or 422 shows its message the way the Windows flow shows a refusal.
  - Keyboard: tab order follows the visual order; Escape keeps `escapeLeavesPanel` behavior. Forced colors: buttons use the existing button classes.

- [ ] **Step 1: Write the failing tests**
  - `the_wsl_group_lists_each_distro`;
  - `no_distros_says_so`;
  - in `agents.test.ts`, `planHooks_sends_the_distro` (the body has `distro: "Ubuntu"`) and `planRemove_sends_the_distro_query`.
- [ ] **Step 2: Run** `pnpm test src/features/agents src/lib/api/agents.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: pass.
- [ ] **Step 5: Commit** `feat(agents): set up each WSL distro's hooks from Settings`.

### Task 9: A browser opener for WSL tabs

**Files:**
- Create: `crates/core/shell/marshell-open` (POSIX sh, LF line endings; check `.gitattributes`)
- Modify: `crates/core/src/shell.rs` (`wsl_dispatcher`), the code that writes the shell scripts into `<home>/shell/`
- Test: `crates/core/src/shell.rs` unit tests

**Interfaces:**
- Produces:
  - `marshell-open`:
    ```sh
    #!/bin/sh
    # Opens an http(s) URL in the Windows default browser; anything else is refused.
    case "$1" in http://*|https://*) ;; *) exit 1 ;; esac
    exec "$(wslpath -u 'C:\Windows\System32\rundll32.exe')" url.dll,FileProtocolHandler "$1"
    ```
    The script is written into `<home>/shell/` beside `marshell.bash`.
  - The dispatcher does this before it starts the shell:
    ```sh
    [ -z "$BROWSER" ] && ! command -v wslview >/dev/null 2>&1 && ! command -v xdg-open >/dev/null 2>&1 && [ -n "$d" ] && export BROWSER="$d/marshell-open"
    ```
    With `$d` unset (the `wslpath` fallback), it does nothing.

- [ ] **Step 1: Write the failing tests:** `wsl_dispatcher_sets_browser_only_without_an_opener` (the dispatcher string contains the guarded export, in that order) and `the_opener_refuses_non_http_urls` (the script text has the `case` guard, and the written file has LF endings).
- [ ] **Step 2: Run them to see them fail.** Run: `cargo test -j 4 -p marshell-core shell::`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-core`. Expected: pass.
- [ ] **Step 5: Commit** `feat(shell): a WSL tab without a browser opener opens http(s) links in the Windows browser`.

### Task 10: Live check

Run the `marshell-live-check` skill with a new scratch folder. Inside the distro, use a new `/tmp` scratch home as in decision 3: the S11 install scripts and the packaged Codex, with `TMPDIR` pointed outside `/tmp`.

Exit criteria, each recorded in `docs/research/2026-10-31-wsl-agents-live-check.md`:
1. Settings > Agents: `Set up in Ubuntu` for each CLI shows a preview whose files are under the scratch home (scratch `HOME` exported for the distro, so the `wsl.exe` home lookup returns it). Apply, then Undo, then Apply leaves the files as previewed.
2. Claude Code in a WSL tab: the row shows its live status (Working, then Done) and the Claude mark. A permission card is answered from Marshell. A prompt naming an alias gets a note with a Linux path.
3. Codex in a WSL tab: the row shows its live status, and a card works after trusting the hooks in `/hooks`.
4. agy in a WSL tab: the row shows its live status, and a risky call shows a card.
5. History lists the WSL sessions, and Resume opens a tab in the distro in the session's folder.
6. A fresh WSL tab in a distro without `wslview`/`xdg-open` has `BROWSER` set. `claude` sign-in opens the Windows browser.
7. After the agent exits, the row returns to the shell row ("WSL, Idle").
