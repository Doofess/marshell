# Foreground Program Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The core learns which program a shell session is running. The row's mark and phrase follow it, a known agent CLI started in a shell shows as that agent, and an agent started in the shell panel is handed off to the session list.

**Architecture:**
- **Two sources.**
  - PowerShell's integration reports the typed command line on Enter, as `OSC 633;E`. That is immediate.
  - A core watcher reads the shell's foreground process chain once a second while a command runs. It uses Toolhelp on Windows and the foreground process group on Linux, and catches wrappers, aliases and shells that report no command line.
- **Protocol.** The registry carries the result as `SessionInfo.foreground`.
- **Frontend.** `toRow` maps it to a mark and a phrase. The panel shell is adopted by the core when it runs an agent, and the frontend notices the panel's tab being listed.

**Tech Stack:** Rust core (axum, tokio, portable-pty, windows-sys 0.61), ts-rs bindings, React and TypeScript, Vitest.

**Spec:**
- `docs/PLAN.md` § "Manual CLIs", line 162: the mark follows the foreground process, as in "Running docker compose up".
- `docs/IDEAS.md` 2026-10-10: promote a known agent to its own row.
- The decisions from 2026-10-17 in memory, `marshell-project.md`:
  - Panel hand-off: a known agent in the panel moves that session to the end of the list via adopt, promoted to the agent's row. Focus follows it, and the panel opens a fresh shell in the same folder.
  - When the agent exits, the row goes back to the shell.
  - The check runs every 1 s, only while a command runs.
  - The phrase is the command line.

## Global Constraints

- **No new dependencies.** windows-sys feature additions are allowed.
- **Vendor names stay in one table.** Executable and package names of agent CLIs live only in `crates/core/src/profiles/detect.rs`, next to `agent_command`. Nothing else names a vendor.
- **Launch agent vs. foreground.** `SessionInfo.agent` stays the *launch* agent. Restart, resume, Mode ▸ and every other action read `agent` only. The mark, the phrase and the status read `foreground` too. A promoted row never offers Mode ▸.
- **Phrase copy.**
  - A running manual CLI reads `Running <command line>`, or `Running <program>` when no command line is known.
  - A promoted agent reads like a launched one: status `limited`, phrase "Status arrives with hooks".
- **Command line limit.** The command line on the wire is its first line, trimmed and capped at 200 chars on a char boundary.
- **When the watcher polls.** It polls a session only if all of these hold:
  - Its launch agent is `None`.
  - It is running and not paused.
  - Its command is running, or it has never sent a C mark (cmd and shells without integration).
- **Tests.** Windows is the tested platform. Unix code compiles behind `cfg` and is marked untested in the PR.

## Review Focus

1. **A command line with a secret** (`$env:TOKEN="..."; x`) is shown in the sidebar. Only the first line, capped at 200 chars, ever leaves the core. Task 1 tests the cap.
2. **The agent exits.** The D mark clears `foreground` at once. A cmd shell, which has no marks, clears on the next poll that finds no child. Tasks 3 and 4 test both.
3. **A paused session** is never polled, and its `foreground` stays as it was. Task 3 tests this.
4. **"Open in a pane" clicked while the watcher hands off the panel.** One critical section takes the panel, so exactly one adopt wins and the other gets 404. The frontend's adopted effect is idempotent. Task 3 tests this.
5. **A launched agent session** (`agent` is Some) never gets a `foreground`. Task 3 tests this.

---

### Task 1: The command line from PowerShell, tracked per session

**Files:**
- Modify: `crates/core/src/pty/osc133.rs` (`Mark`, scanner)
- Modify: `crates/core/shell/marshell.ps1:19-29`
- Modify: `crates/core/src/pty/session.rs` (command tracking in `ingest`, new accessors)
- Modify: `crates/core/src/registry.rs:300` (`mark` ignores the new variant)
- Modify: `crates/core/src/bin/fake-agent.rs` (new `cmdline <text>` mode)
- Test: the `osc133.rs` unit tests, the `shell.rs` unit tests, `crates/core/tests/session.rs`

**Interfaces:**
- Produces:
  - `Mark::CommandLine(String)`.
  - `Session::command_running(&self) -> bool`.
  - `Session::ever_executed(&self) -> bool`.
  - `Session::command_line(&self) -> Option<String>`. This is the line as shown, already trimmed to its first line and capped at 200 chars.

- [ ] **Step 1: Write the failing tests** in `osc133.rs`'s test module:
  - `scan(b"\x1b]633;E;git status\x07") == [Mark::CommandLine("git status".into())]`, with ST (`\x1b\\`) also ending it.
  - Escapes: `\x1b]633;E;a\x3bb\\\\c\x07` gives `"a;b\\c"`. `\xHH` decodes any byte, and invalid UTF-8 is decoded lossily.
  - Any other `633` letter is ignored (`\x1b]633;A\x07` gives `[]`), and an `E` split across two `scan` calls is still found.
  - A body over 4096 bytes gives no mark, and the scanner recovers for the next mark, as the existing long-carry test does for 133.
- [ ] **Step 2: Write the failing session test.** In `tests/session.rs`, `command_line_is_tracked_from_the_shell_marks`:
  - Spawn fake-agent `cmdline "docker compose up"`. It prints `633;E;docker compose up`, then `133;C`, sleeps 2 s, then prints `133;D;0`.
  - Within 3 s, `command_running()` and `ever_executed()` are true, and `command_line() == Some("docker compose up")`.
  - After the D mark, `command_running()` is false and `command_line()` is None.
  - Also: a line of 300 `x` followed by `\nsecond` gives exactly 200 `x`.
- [ ] **Step 3: Run them and watch them fail.** Run `cargo test -p marshell-core osc133` and `cargo test -p marshell-core --test session command_line`. Expected: compile errors, because the variant and the methods don't exist yet.
- [ ] **Step 4: Implement.**
  - **Scanner.** Extend `Osc133Scanner` to recognise the prefix `ESC ] 633 ; E ;` beside `ESC ] 133 ;`, with the same carry and length rules.
  - **Session.** Keep `Mutex<CommandTrack { running, ever_executed, line }>`, updated in `ingest` before the callback:
    - `CommandLine` sets `line`.
    - `Executed` sets `running` and `ever_executed`.
    - `Finished` and `PromptStart` clear `running` and `line`.
  - **Registry.** `registry.mark` ignores `CommandLine`.
  - **PowerShell.** In `marshell.ps1`, before the C write, write `ESC]633;E;<escaped>ESC\`. Escape `\` as `\\`, `;` as `\x3b`, and every char below 0x20 as `\xHH`:

```powershell
$esc = ($line -replace '\\', '\\' -replace ';', '\x3b') -replace '[\x00-\x1f]', { '\x{0:x2}' -f [int][char]$_.Value }
[Console]::Write("$([char]27)]633;E;$esc$([char]27)\$([char]27)]133;C$([char]27)\")
```

  (The scriptblock replacement needs PowerShell 6 or later. On Windows PowerShell 5.1, use `[regex]::Replace($s, '[\x00-\x1f]', { param($m) '\x{0:x2}' -f [int][char]$m.Value })`, which works on both. Use that form.)
- [ ] **Step 5: Add a `shell.rs` unit test.** `ps1_reports_the_command_line_before_c`: in `FILES`, the ps1 contains `]633;E;`, and it appears before `]133;C`.
- [ ] **Step 6: Run all of `cargo test -p marshell-core`.** Expected: all pass.
- [ ] **Step 7: Commit** with `feat(core): PowerShell reports the typed command line, and a session tracks its running command`.

### Task 2: The foreground chain and the agent table

**Files:**
- Create: `crates/core/src/pty/proctree.rs`. It takes `Owned`, `snapshot`, `processes`, `creation_time` and `descendants` from `freeze.rs`'s `mod windows`. `processes` now also returns the exe name.
- Modify: `crates/core/src/pty/freeze.rs` (use `proctree`), `crates/core/src/pty/mod.rs`
- Modify: `crates/core/src/pty/session.rs` (`foreground_chain`)
- Modify: `crates/core/src/profiles/detect.rs` (`program_name`, `program_of`, `agent_in`)
- Test: the `detect.rs` unit tests, `crates/core/tests/session.rs`

**Interfaces:**
- Produces:
  - `proctree::foreground_chain(root: u32) -> Option<Vec<String>>`. This is Windows only. `None` means the snapshot failed.
  - `Session::foreground_chain(&self) -> Option<Vec<String>>`. It holds normalized names, outermost first, with the root excluded. It is empty when the shell has no child.
  - `detect::program_name(exe: &str) -> String`. It takes the basename, lowercases it, and strips one `.exe`, `.cmd`, `.bat`, `.ps1` or `.com`.
  - `detect::program_of(command_line: &str) -> Option<String>`.
  - `detect::agent_in(chain: &[String], command_line: Option<&str>) -> Option<&'static str>`.

- [ ] **Step 1: Write the failing `detect.rs` tests.**
  - `program_name("C:\\x\\Codex.EXE") == "codex"`.
  - `program_of`:
    - `program_of("docker compose up") == Some("docker")`.
    - It skips the wrappers `npx`, `bunx`, `winpty`, `pnpm dlx`, `pnpm exec`, `yarn dlx`, `&`, and a leading `./` or `.\`. So `program_of("npx @openai/codex") == Some("@openai/codex")`.
    - A quoted path: `program_of("& 'C:\\a b\\claude.cmd' -c") == Some("claude")`.
    - `program_of("   ") == None`.
  - `agent_in`:
    - `agent_in(&["cmd","node","codex-x86_64-pc-windows-msvc"], None) == Some("codex")`.
    - `agent_in(&["claude"], None) == Some("claude")`.
    - `agent_in(&["agy"], None) == Some("antigravity")`.
    - `agent_in(&[], Some("npx @anthropic-ai/claude-code")) == Some("claude")`.
    - `agent_in(&["node"], Some("git status")) == None`.
    - `agent_in(&["claude-helper"], None) == None`: an exe matches only `claude` exactly. `codex` matches exactly or as the prefix `codex-`.
- [ ] **Step 2: Write the failing Windows session tests.** Both are `#[cfg(windows)]` in `tests/session.rs`.
  - `foreground_chain_names_the_running_child`:
    - The root is `cmd.exe /d /c "<fake-agent> sleep"`.
    - Within 3 s, the chain is `["fake-agent"]`.
    - After a kill, `foreground_chain()` returns None or empty, and never panics.
  - `a_renamed_agent_binary_is_found`: copy fake-agent into a temp dir as `codex.exe` and run it under cmd the same way. `agent_in(&chain, None) == Some("codex")`.
- [ ] **Step 3: Run them and watch them fail.** Run `cargo test -p marshell-core detect` and `cargo test -p marshell-core --test session foreground`. Expected: compile errors.
- [ ] **Step 4: Implement.**
  - **Move the helpers.** Move the Toolhelp helpers into `proctree.rs`. `freeze.rs` keeps the suspend logic and calls `proctree::descendants`. Behaviour must not change: the freeze tests still pass.
  - **`foreground_chain(root)`:**
    1. Take one process snapshot.
    2. From `root`, repeatedly pick the child whose parent is the current pid and whose creation time is `>=` the parent's (the same rule as `descendants`). Skip `conhost.exe` and `openconsole.exe`, and take the newest by creation time.
    3. Push `program_name(exe)` for each one, until there is no child.
  - **On Linux,** `Session::foreground_chain` reads the master's `process_group_leader()`. If it differs from the root pid, the chain is `[program_name(/proc/<pgid>/comm)]`, else empty.
  - **On other Unix,** it returns None.
  - **The agent table.** It lives in `detect.rs` beside `agent_command`:
    - exe names: `claude` → claude, `codex` and `codex-*` → codex, `agy` → antigravity.
    - npm packages for `program_of` results: `@anthropic-ai/claude-code` → claude, `@openai/codex` → codex.
    - `agent_in` checks the chain first, deepest first, then `program_of(line)` against both tables.
- [ ] **Step 5: Run `cargo test -p marshell-core`.** Expected: all pass, the freeze tests included.
- [ ] **Step 6: Commit** with `feat(core): read a session's foreground process chain and name the agent it runs`.

### Task 3: `SessionInfo.foreground`, the watcher, and the panel hand-off

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Foreground`, `SessionInfo.foreground`). Then regenerate `src/generated`.
- Modify: `crates/core/src/registry.rs` (`set_foreground`, clearing)
- Create: `crates/core/src/server/foreground.rs` (the watcher)
- Modify: `crates/core/src/server/mod.rs` (`AppState` gets `foreground_wake: tokio::sync::Notify`; start the watcher where the server starts)
- Modify: `crates/core/src/server/sessions.rs:263-267` (`on_mark` wakes the watcher on `Executed`)
- Modify: `crates/core/src/server/shell_panel.rs` (factor adopt's critical section into `fn take_and_adopt(st: &AppState) -> Option<String>`, used by the route and by the watcher)
- Test: the `registry.rs` unit tests, `crates/core/tests/foreground.rs` (new)

**Interfaces:**
- Consumes: from Task 1, `command_running`, `ever_executed` and `command_line`. From Task 2, `foreground_chain`, `program_of` and `agent_in`.
- Produces:
  - `pub struct Foreground { pub program: String, pub command_line: Option<String>, pub agent: Option<String> }`, with ts-rs and serde like its neighbours.
  - `SessionInfo.foreground: Option<Foreground>`.
  - `Registry::set_foreground(&self, tab_id: &str, generation: u64, foreground: Option<Foreground>)`.

- [ ] **Step 1: Write the failing registry unit tests.**
  - `set_foreground` is applied only to a Running row whose `agent` is None and whose generation matches.
  - It is ignored for an agent row, an exited row, or a stale generation.
  - It does not send on the watch channel when the value is unchanged.
  - `mark(Finished(_))` and `mark(PromptStart)` clear it. Exit, restart, resume and close clear it wherever `paused` is cleared.
- [ ] **Step 2: Write the failing route tests** in `tests/foreground.rs`. Build state like `tests/pause.rs` does. Use a profile whose command is `cmd.exe /d /c "<fake-agent> sleep"` with `shell_integration: false`, and a second profile running a temp copy named `codex.exe`. Poll `GET /v1/sessions` for up to 4 s.
  - `a_shell_running_a_program_shows_it`: `foreground.program == "fake-agent"`, `foreground.agent == null` and `agent == null`.
  - `a_shell_running_an_agent_names_it`: `foreground.agent == "codex"`, while the row's `agent` stays `null`.
  - `a_paused_session_is_not_polled`: pause the session, then wait 2 s. `foreground` is unchanged.
  - `a_launched_agent_has_no_foreground`: a session created with `agent: "claude"` and the fake-agent profile has `foreground == null` after 2 s.
  - `the_foreground_clears_when_the_program_ends`: fake-agent `sleep-then-exit 1` under `cmd /k`, so the shell survives. `foreground` becomes null within 3 s of the child exiting. Add a fake-agent mode if none fits.
  - `an_agent_in_the_panel_is_handed_off`:
    - `POST /v1/shell-panel` starts the panel, with the default profile set to the codex profile.
    - Within 4 s, `GET /v1/sessions` lists the panel's `tab_id` with `foreground.agent == "codex"`.
    - Then `POST /v1/shell-panel/adopt` gives 404.
- [ ] **Step 3: Run them and watch them fail.** Run `cargo test -p marshell-core --test foreground` and `cargo test -p marshell-core registry`. Expected: compile errors.
- [ ] **Step 4: Implement.**
  - **The watcher loop.** `loop { select!(sleep(1s), wake.notified()); tick }`.
  - **Each tick.** Over a snapshot of the sessions map, take the registry rows that pass the polling rule in Global Constraints, plus the panel shell under the same rule except the row checks. For each one:
    1. In one `spawn_blocking` for the whole tick, compute the chain and the line.
    2. `fg = None` when the chain is empty or None *and* the line is None.
    3. Otherwise `program` is the last item of the chain if there is one, else `program_of(line)`, else skip.
    4. `agent = agent_in(chain, line).map(String::from)`.
  - **Rows:** `set_foreground(tab, generation_of_row, fg)`.
  - **The panel:** when `fg.agent.is_some()`, call `take_and_adopt(&st)`. If it returns `Some(tab)`, call `set_foreground(tab, PANEL_GENERATION, fg)`.
  - **The 404 path.** The adopt route keeps returning 404 when `take_and_adopt` returns None.
  - **Regenerate the bindings.** Run `cargo test -p marshell-protocol`, then `pnpm check:generated`.
- [ ] **Step 5: Run `cargo test --workspace`.** Expected: exit 0, all suites ok.
- [ ] **Step 6: Commit** with `feat(core): sessions report their foreground program, and an agent in the shell panel moves to the list`.

### Task 4: The frontend follows the foreground

**Files:**
- Modify: `src/components/AgentMark/tools.ts` (`toolForProgram`)
- Modify: `src/features/sessions/toRow.ts`
- Create: `src/features/sessions/handOff.ts` (pure). Modify `src/features/sessions/LiveApp.tsx` (an effect near line 460)
- Modify: `src/features/sidebar/fixtures.ts` and any test fixture that builds a `SessionInfo` (add `foreground: null`)
- Modify: `docs/PLAN.md:162` (one sentence: how the mark is found, the 1 s check while a command runs, and the PowerShell command line)
- Test: `tools.test.ts`, `toRow` tests, `handOff.test.ts`, the `sessionMenuRun` tests

**Interfaces:**
- Consumes: `SessionInfo.foreground: Foreground | null` from `src/generated`.
- Produces:
  - `toolForProgram(program: string): ToolId`.
  - `handedOff(panelTabId: string | undefined, adoptedTabId: string | undefined, infos: SessionInfo[]): string | null`. It returns the panel's tab id once the list holds it and no adopt of it is already pending.

- [ ] **Step 1: Write the failing tests.**
  - **`toolForProgram`** maps:
    - `pwsh`/`powershell` → powershell, `bash`, `zsh`, `cmd`, `wsl`
    - `git`, `docker`/`docker-compose` → docker, `kubectl`, `node`, `python`/`python3`/`py` → python, `cargo`, `gh`, `terraform`
    - `ssh`, `aws`, `gcloud`, `az`, `supabase`, `vercel`
    - anything else → `program`
  - **`toRow`:**
    - A shell with `foreground {program:"docker", command_line:"docker compose up", agent:null}` gives agent `docker`, status `working` and phrase `Running docker compose up`.
    - Without a command line it reads `Running fake-agent`, with agent `program`.
    - With `agent:"codex"`, it gives agent `codex`, status `limited` and phrase "Status arrives with hooks".
    - With `foreground:null` it is the shell's own kind, as today.
    - Paused, exited and ended still win over the foreground.
    - A launched agent row ignores `foreground`.
  - **`sessionMenuRun`:** a promoted row (`agent:null` with `foreground.agent:"claude"`) has `facts.agent === false`, so there is no Mode ▸.
  - **`handedOff`:**
    - It returns null while the panel's tab isn't listed.
    - It returns the id once it is listed.
    - It returns null when `adoptedTabId` already equals it, because "Open in a pane" is already completing.
- [ ] **Step 2: Run `pnpm test`.** Expected: the new tests fail, because `toolForProgram` and `handOff` don't exist yet.
- [ ] **Step 3: Implement.**
  - `toRow` uses the foreground only when `info.agent === null` and the row is running and not paused.
  - In LiveApp: `const handed = handedOff(panelTab?.tabId, adopted?.tabId, infos); useEffect(() => { if (handed && endpoint) setAdopted({ endpoint, tabId: handed }); }, [handed, endpoint]);`.
  - The existing adopted effect then releases the panel and shows the session in a pane, with focus following. The panel reopens in the focused session's folder, which is the same folder.
  - Update every `SessionInfo` fixture with `foreground: null`.
- [ ] **Step 4: Run `pnpm test` and `pnpm build`.** Expected: all pass, and the build exits 0.
- [ ] **Step 5: Commit** with `feat(sessions): a shell's row shows the program it runs, an agent started in a shell shows as that agent, and the panel hands an agent off`.
