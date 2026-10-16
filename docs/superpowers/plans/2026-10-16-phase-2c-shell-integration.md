# Phase 2c: Shell Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Plain shell sessions start through an app-owned init script. That script loads the user's own profile first, then emits OSC 133 prompt and command marks. The core turns those marks into real row status. The terminal turns them into gutter marks and prompt jumps.

**Architecture:**
- **Scripts:** they live in `crates/core/shell/`, are embedded with `include_str!`, and are written to `~/.marshell/shell/` when the core starts.
- **Launch:** a new `shell` module in the core rewrites a profile's argv and env so the shell loads the script. User profile files are never touched.
- **Core marks:** a non-filtering OSC 133 scanner in `Session::ingest` reports each mark through a callback. The registry folds the marks into a new `SessionInfo.command` field and broadcasts it over `/v1/events`.
- **Terminal:** xterm.js parses the same marks itself (`registerOscHandler(133)`) to place prompt markers and exit-code decorations.

**Tech stack:** Rust (axum, ts-rs), PowerShell, bash, zsh, fish, cmd `PROMPT`, React and TypeScript, xterm.js 6 (`registerOscHandler`, `registerMarker`, `registerDecoration`), Vitest. No new dependencies.

**Spec:**
- `docs/PLAN.md`:
  - §9 phase 2, slice 2c (line 738): "app-owned init scripts (OSC 7/133, PSReadLine and prompt options)".
  - The phase 2 exit criterion: "OSC 133 prompt jumps work".
  - Line 545: the scripts live in `~/.marshell/shell/`.
- `docs/BRIEF.md`, Layer 3: never edit `$PROFILE`, `.bashrc` or `.zshrc`. Each session runs an app-owned init script that loads the user's profile first, then applies app settings, and enables OSC 7 and 133.
- Design decisions the user made on 2026-10-16 are recorded in Global Constraints below.

## Global Constraints

- **Profile files:** no user profile or rc file is ever written, moved or edited. "Make permanent" moves to phase 3, with the config safety core (PLAN §6).
- **Rust:** keep it simple. Use plain structs and enums, and keep `unwrap()` on locks as the existing code does. No new crates or npm packages.
- **Shells in scope:**
  - PowerShell 7 and Windows PowerShell 5.
  - cmd, through its `PROMPT` variable: prompt marks and the folder, no exit codes, no "running" state.
  - bash: Git Bash and Unix.
  - zsh and fish: unit-tested only. Do not claim they work on macOS or Linux; the user can't test there.
  - WSL distros, through a `/bin/sh` dispatcher that picks the distro user's shell.
- **Agent sessions** (`agent` is `Some`) start without integration. Their argv stays exactly as `launch_argv` builds it today.
- **Per-profile switch:** `Profile.shell_integration: bool`, on by default; an existing `profiles.json` without the field loads as on.
- **Custom commands:** integration applies only when the profile's command has no arguments beyond the known ones:
  - PowerShell: `-NoLogo`, `-NoProfile`.
  - bash, zsh and fish: `-l`, `--login`, `-i`.
  - cmd and WSL: no arguments, except WSL's `-d <distro>`.
  - Anything else runs as written, with the reason shown on the profile page. Matching is case-insensitive for PowerShell and cmd.
- **Row status for a shell session** (agents unchanged):
  - Unknown (no mark seen yet) → `idle`, phrase `""`.
  - Running → `working`, phrase `"Running a command"`.
  - At the prompt with a last exit code that isn't 0 → `idle`, phrase `"Last command failed (exit N)"`.
  - At the prompt otherwise → `idle`, phrase `""`.
  - Exited → `ended`, as today.
  - Never show `working` without a C mark.
- **Prompt jumps:** two new keymap commands.
  - `prompt-prev`, "Jump to the previous prompt", binding `{ code: "PageUp" }`.
  - `prompt-next`, "Jump to the next prompt", binding `{ code: "PageDown" }`.
  - Group "Tools". With the default modifier these are Ctrl+Shift+PageUp/PageDown, or ⌘PageUp/PageDown on a Mac.
- **Gutter marks:** the look is decided by the `frontend-design` skill using existing tokens.
  - Only a finished command with a known exit code gets a mark.
  - A failed mark must differ from a success mark by more than colour (shape or glyph).
  - Each mark has an accessible label "Exit code N".
- **Script files:** stored with LF line endings. `.gitattributes` already sets `* text=auto eol=lf`.
- **bash 3.2:** the bash script must run on it, since macOS ships 3.2. That rules out `PROMPT_COMMAND` arrays, `${var@Q}` and `declare -n`.

## Review Focus

1. **A user whose profile is slow or broken** (an error thrown in `$PROFILE`, or a `.bashrc` that `exit`s). The shell must still start, the user's own error must still show, and the marks must still arrive. Pinned by Task 4's `profile_error_still_marks` test.
2. **A prompt framework that rewrites the prompt** (starship, oh-my-posh, a `PROMPT_COMMAND` that sets `PS1`). The marks must survive because they are re-applied on every prompt. Pinned by Task 4's `bash_prompt_command_overwriting_ps1_keeps_marks`.
3. **Marks split across PTY reads, or interleaved with DSR queries and colour output.** The scanner must neither miss nor double-count them. Pinned by Task 2's split-chunk tests.
4. **Windows PowerShell 5 with ExecutionPolicy Restricted** (the default on client Windows). Dot-sourcing a `.ps1` is refused there, so the script is loaded as a scriptblock from its text. Pinned by Task 3's argv test, which checks for `ReadAllText`.
5. **Spaces and apostrophes in the home path** (`C:\Users\Home Office`, `O'Brien`). Every script path in an argv or dispatcher is quoted. Pinned by Task 3's quoting tests.

---

### Task 1: Protocol types

**Files:**
- Modify: `crates/protocol/src/api.rs`
- Regenerate: `src/generated/*` (the repo's ts-rs export flow, then `pnpm check:generated`)
- Modify every Rust and TS construction site of `Profile` and `SessionInfo` that stops compiling. Rust detected profiles and fixtures set `shell_integration: true` and `command: CommandState::Unknown`; TS fixtures likewise.

**Interfaces:**
- Produces:
  ```rust
  // In Profile, after `env`:
  /// Start plain shell sessions through Marshell's init script (prompt marks, command status).
  #[serde(default = "default_true")]
  pub shell_integration: bool,

  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
  #[serde(tag = "kind", rename_all = "snake_case")]
  #[ts(export)]
  pub enum CommandState {
      /// No prompt mark seen: no integration, or the shell hasn't drawn its first prompt.
      Unknown,
      /// At the prompt. `last_exit` is the most recent finished command's code, if the shell reports one.
      Prompt { last_exit: Option<i32> },
      /// A command is running (an OSC 133 C mark arrived).
      Running,
  }
  // In SessionInfo, after `state`:
  pub command: CommandState,

  /// POST /v1/profiles/integration body: what integration would do for this profile.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
  #[ts(export)]
  pub struct IntegrationInfo {
      pub applies: bool,
      /// Why it doesn't apply; None when it does.
      pub reason: Option<String>,
      /// Where the script lives, for the read-only view.
      pub script_path: Option<String>,
      /// The script text (for cmd, the PROMPT value; for WSL, the dispatcher).
      pub script: Option<String>,
  }
  ```

- [ ] **Step 1:** Write `profile_without_shell_integration_loads_as_on`, a protocol test. It deserializes a 2b-era profile JSON with no `shell_integration` and asserts the field is `true`. Also write `command_state_serializes_tagged`: `Prompt{last_exit:Some(1)}` gives `{"kind":"prompt","last_exit":1}`.
- [ ] **Step 2:** Run `cargo test -p marshell-protocol`. Expected: compile FAIL, because the types don't exist yet.
- [ ] **Step 3:** Add the types, `fn default_true() -> bool`, and fix the construction sites.
- [ ] **Step 4:** Run `cargo test --workspace`, `pnpm check:generated`, then `pnpm test`. Expected: all PASS.
- [ ] **Step 5:** Commit: `feat(protocol): shell integration switch, command state and integration info`.

### Task 2: OSC 133 scanner and session callback

**Files:**
- Create: `crates/core/src/pty/osc133.rs`.
- Modify: `crates/core/src/pty/mod.rs` (export it) and `crates/core/src/pty/session.rs`.

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Mark { PromptStart, CommandStart, Executed, Finished(Option<i32>) } // A, B, C, D[;code]
  #[derive(Default)] pub struct Osc133Scanner { /* carry: Vec<u8> */ }
  impl Osc133Scanner { pub fn scan(&mut self, bytes: &[u8]) -> Vec<Mark> }
  pub type MarkCallback = Box<dyn Fn(Mark) + Send + Sync>;
  // SpawnSpec gains: pub on_mark: Option<MarkCallback>,
  ```
- Scanner rules:
  - It recognises `ESC ] 133 ; <X> [; <params>] (BEL | ESC \)`.
  - D's first parameter is parsed as `i32`. Anything unparsable gives `Finished(None)`. Extra `;key=value` parameters are ignored.
  - A sequence cut off at the end of a chunk is carried over to the next one. Carry is capped at 128 bytes; past that it is dropped and scanning resumes.
  - It never changes the output bytes.
- **Session:** `ingest` runs the scanner on the raw `bytes` under the output lock, then calls `on_mark` for each mark after `drop(out)`, never while holding a session lock.

- [ ] **Step 1:** Write the scanner tests in `osc133.rs`:
  - `finds_all_four_marks` covers BEL and ST terminators.
  - `d_with_and_without_code`: `D;1` gives `Some(1)`; `D` gives `None`; `D;x` gives `None`; `D;0;aid=3` gives `Some(0)`.
  - `split_across_chunks`: split at every byte offset of `"a\x1b]133;D;2\x07b"`, scan the two halves, and assert exactly `[Finished(Some(2))]`.
  - `ignores_other_osc`: OSC 7, OSC 9;9, OSC 1337 and `ESC]1330;A\x07` give nothing.
  - `oversized_carry_is_dropped`: `ESC]133;` followed by 1000 `x` bytes gives no mark and no panic. A later valid mark is still found.
- [ ] **Step 2:** Run `cargo test -p marshell-core osc133`. Expected: FAIL (module missing).
- [ ] **Step 3:** Implement the scanner.
- [ ] **Step 4:** Add the session wiring. Write `marks_reach_the_callback` in `crates/core/tests/session.rs`. It uses `fake-agent print` with an argument containing `\x1b]133;A\x07`, or adds a `fake-agent osc133` subcommand if `print` can't carry the bytes. It collects the marks through `on_mark` into an `mpsc` channel and asserts `PromptStart` arrives within 10 s.
- [ ] **Step 5:** Run `cargo test -p marshell-core`. Expected: PASS. Add `on_mark: None` to every other `SpawnSpec` construction site.
- [ ] **Step 6:** Commit: `feat(core): scan OSC 133 marks from session output`.

### Task 3: Scripts, install and argv rewriting

**Files:**
- Create the scripts:
  - `crates/core/shell/marshell.ps1`
  - `crates/core/shell/marshell.bash`
  - `crates/core/shell/marshell.fish`
  - `crates/core/shell/marshell.zsh`
  - `crates/core/shell/zsh/.zshenv`, `zsh/.zprofile`, `zsh/.zshrc` and `zsh/.zlogin`
- Create: `crates/core/src/shell.rs`.
- Modify: `crates/core/src/lib.rs` (`pub mod shell;`), `crates/core/src/store.rs` and `crates/core/src/paths.rs`.

**Interfaces:**
- Produces:
  ```rust
  // paths.rs
  pub fn shell_dir(home: &Path) -> PathBuf            // <home>/shell
  // store.rs: the existing atomic write, factored out (unique temp, then rename; no .bak)
  pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()>
  // shell.rs
  pub fn install(home: &Path) -> anyhow::Result<PathBuf> // writes only files whose content differs; returns shell_dir
  pub struct Applied { pub argv: Vec<String>, pub env: Vec<(String, String)> }
  /// Err is the reason shown on the profile page.
  pub fn integrate(profile: &Profile, dir: &Path, outer_env: &dyn Fn(&str) -> Option<String>) -> Result<Applied, String>
  pub fn describe(profile: &Profile, dir: Option<&Path>, outer_env: &dyn Fn(&str) -> Option<String>) -> IntegrationInfo
  ```
- **Shell kind:** taken from the basename of `command[0]`, lowercased, without `.exe`:
  - `pwsh` or `powershell` → PowerShell.
  - `cmd` → cmd.
  - `bash` → bash.
  - `zsh` → zsh.
  - `fish` → fish.
  - `wsl` → WSL.
  - Anything else: `Err("Marshell has no integration script for <name>.")`.
- **Other reasons:**
  - `"Shell integration is off for this profile."`
  - `"This command has its own arguments, so Marshell runs it as written."`
  - `"The integration scripts could not be written."` (when `dir` is None)
- **argv and env per kind** (`P` is the script path; quoting uses the existing `powershell_quote` and `posix_quote`):
  - **PowerShell:** keep the command, then append `-NoExit -Command ". ([scriptblock]::Create([IO.File]::ReadAllText(<P quoted>)))"`.
    - Ruling: dot-sourcing a scriptblock instead of the file sidesteps a Restricted ExecutionPolicy, which only blocks script files.
  - **cmd:** keep argv. Set `PROMPT` to `$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\` + `<inner>`.
    - `<inner>` is the profile env's `PROMPT`, else `outer_env("PROMPT")`, else `$P$G`.
    - If `<inner>` already contains `]133;`, use it unchanged; nested cmd inherits PROMPT.
  - **bash:** drop `-l`, `--login` and `-i`, then append `--init-file <P with forward slashes> -i`.
    - Set `MARSHELL_BASH_LOGIN=1` when the original had `-l` or `--login`.
  - **zsh:** keep argv.
    - Set `ZDOTDIR=<dir>/zsh`.
    - Set `MARSHELL_USER_ZDOTDIR` to the profile env's `ZDOTDIR`, else `outer_env("ZDOTDIR")`, else empty, meaning `$HOME`.
  - **fish:** keep argv, then append `-C`, `source <P posix-quoted>`.
  - **WSL:** keep `wsl.exe -d <distro>`, then append `--exec /bin/sh -c <dispatcher>`.
    - The dispatcher is a short POSIX script. It finds the script directory with `wslpath -u '<Windows dir>'`.
    - It picks the shell from `${SHELL:-$(getent passwd "$(id -un)" | cut -d: -f7)}`.
    - It `exec`s that shell the same way the bash, zsh and fish rules above say, exporting the env inline because profile env doesn't cross into WSL.
    - For an unknown shell it runs `exec "$s" -l`.
- **Script behaviour.** Every script:
  1. Guards against loading twice (`MARSHELL_SHELL_INTEGRATION=1`).
  2. Loads the user's own profile first, in the shell's normal order. A failure in it is shown but does not stop the script.
  3. Emits, through hooks that are re-applied on each prompt:
     - `D[;code]`, with the code only when a command ran since the last prompt.
     - `A` at the prompt start.
     - `C` when the user's command starts.
     - The folder: OSC 7 `file://<host><path>` on POSIX shells, OSC `9;9;<path>` on PowerShell.
- **Hook method per shell:**
  - **PowerShell:** wrap the existing `prompt` function, capturing `$?` and `$LASTEXITCODE` first. Wrap `PSConsoleHostReadLine` to emit C after it returns.
  - **bash:** source the login files when `MARSHELL_BASH_LOGIN=1` (`/etc/profile`, then the first of `~/.bash_profile`, `~/.bash_login`, `~/.profile`), else `~/.bashrc`; then `unset MARSHELL_BASH_LOGIN`.
    - A first `PROMPT_COMMAND` entry saves `$?` and emits D.
    - A last entry wraps `PS1` in `\[…A…\]` if it isn't wrapped already.
    - A `DEBUG` trap emits C once per prompt, guarded so it doesn't fire for `PROMPT_COMMAND` itself.
  - **zsh:**
    - The shim files keep `ZDOTDIR` on the shim while startup runs.
    - Each shim file sources the user's file of the same name with `ZDOTDIR` set to the user's directory, then sets it back to the shim.
    - The shim `.zshrc` sources `marshell.zsh` after the user's `.zshrc`, which installs `precmd` and `preexec` through `add-zsh-hook`.
    - `ZDOTDIR` goes back to the user's value at the end of `.zshrc` for an interactive non-login shell, else in `.zlogin`.
  - **fish:** `--on-event fish_prompt` emits D (from `$status` saved by `fish_postexec`) and A. `fish_preexec` emits C.

- [ ] **Step 1:** Write the `shell.rs` unit tests:
  - `pwsh_appends_scriptblock_load` asserts the last arg contains `ReadAllText(` and the quoted path. Use a dir `C:\Users\O'Brien Home\.marshell\shell`; the quote must be doubled.
  - `powershell_with_unknown_args_runs_as_written` uses `-File x.ps1` and expects Err with the arguments reason.
  - `cmd_wraps_existing_prompt`, `cmd_default_prompt` and `cmd_does_not_double_wrap`.
  - `bash_login_becomes_init_file`: `[bash.exe, -l]` gives `[bash.exe, --init-file, <fwd-slash path>, -i]` plus `MARSHELL_BASH_LOGIN=1`.
  - `zsh_sets_zdotdir_and_keeps_user_one`.
  - `fish_sources_with_c`.
  - `wsl_dispatcher_quotes_space_path`: the dispatcher contains `wslpath -u 'C:\Users\Home Office\.marshell\shell'` quoted.
  - `unknown_shell_reason` uses `nu`.
  - `switch_off_reason`.
  - `posix_scripts_have_no_cr` asserts that no embedded bash, zsh or fish file and no shim contains `\r`.
  - `install_writes_once`: a second `install` doesn't change any file's mtime.
- [ ] **Step 2:** Run `cargo test -p marshell-core shell`. Expected: FAIL (module missing).
- [ ] **Step 3:** Implement `shell.rs`, `write_bytes_atomic` and `shell_dir`, and write the script files.
- [ ] **Step 4:** Run `cargo test -p marshell-core`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): app-owned shell integration scripts and launch rewriting`.

### Task 4: Wire it into sessions, the registry and the API; real-shell tests

**Files:**
- Modify:
  - `crates/core/src/server/mod.rs`: in `AppState`, `Inner.shell_dir: Option<PathBuf>`, set from `shell::install(&home)` at startup, logging a warning on Err. Add the route `POST /v1/profiles/integration`.
  - `crates/core/src/server/sessions.rs`: in `resolve`/`create`, when `agent` is None and the profile is found, try `shell::integrate`. On Ok, use its argv and append its env after the profile env. On Err, keep the plain argv. Pass `on_mark` to `registry.mark`.
  - `crates/core/src/server/profiles.rs`: the `integration` handler takes a `Profile` JSON body and returns `IntegrationInfo` from `shell::describe`.
  - `crates/core/src/registry.rs`.
- Create: `crates/core/tests/shell_integration.rs`.

**Interfaces:**
- Produces:
  ```rust
  impl Registry { pub fn mark(&self, tab_id: &str, mark: Mark) }
  ```
- **Fold rule** (broadcast only when the state changes; ignore marks for exited or unknown tabs):
  - `Executed` → `Running`.
  - `Finished(Some(c))` → `Prompt{last_exit:Some(c)}`.
  - `Finished(None)` → `Prompt{last_exit: <previous last_exit, or None>}`.
  - `PromptStart` → `Prompt{last_exit: <previous, or None>}`.
  - `CommandStart` → no change.
  - New sessions start `Unknown`.

- [ ] **Step 1:** Write the registry tests `mark_folds_into_command_state`, `marks_after_exit_are_ignored` and `unchanged_state_does_not_broadcast`. The last checks the `watch` receiver's `has_changed()`.
- [ ] **Step 2:** Run `cargo test -p marshell-core registry`. Expected: FAIL.
- [ ] **Step 3:** Implement `Registry::mark` and the server wiring.
- [ ] **Step 4:** Write `crates/core/tests/shell_integration.rs`.
  - Each test spawns a `Session` with the argv and env from `shell::integrate`, `install`ed into a tempdir.
  - `HOME`, `USERPROFILE` and `ZDOTDIR` point at a temp home, so the runner's own rc files don't load.
  - The test returns early with a printed "skipped: <shell> not found" when the shell is missing.
  - The tests:
    - `pwsh_reports_prompt_running_and_exit`: wait for A; type `cmd /c exit 3\r`; expect C, then `Finished(Some(3))`, then A. Run it with `pwsh` and, on Windows, `powershell`.
    - `bash_reports_prompt_running_and_exit` uses Git Bash on Windows or `bash` on Unix. It types `false\r` and expects C, `Finished(Some(1))` and A.
    - `zsh_reports_prompt_running_and_exit` only runs where `zsh` exists.
    - `cmd_reports_prompts`: Windows only; type `echo hi\r` and expect a second A.
    - `profile_error_still_marks`: a temp-home bash `.bashrc` that runs `false; echo broken >&2`. The test expects `broken` in the output and A anyway.
    - `bash_prompt_command_overwriting_ps1_keeps_marks`: the temp `.bashrc` sets `PROMPT_COMMAND='PS1="x$ "'`. Expect A on two consecutive prompts.
  - Each wait has a timeout of 20 s.
- [ ] **Step 5:** Run `cargo test --workspace`. Expected: PASS, with skips printed only for shells missing on this machine. Then run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check`. Expected: clean.
- [ ] **Step 6:** Commit: `feat(core): start shells with integration and track command state`.

### Task 5: Row status from command state

**Files:**
- Modify: `src/features/sessions/toRow.ts` and `src/features/sessions/toRow.test.ts`.

- [ ] **Step 1:** Write tests for the five shell cases in Global Constraints, with the exact phrases, plus `an agent session ignores command state`: an agent with `command: running` stays `limited`.
- [ ] **Step 2:** Run `pnpm test toRow`. Expected: FAIL.
- [ ] **Step 3:** Implement. Update the doc comment above `toRow`: a shell row's status now comes from prompt marks.
- [ ] **Step 4:** Run `pnpm test toRow`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(ui): shell rows show running and failed commands`.

### Task 6: Prompt marks, gutter decorations and prompt jumps in the terminal

**Files:**
- Create: `src/features/terminal/promptMarks.ts` and `src/features/terminal/promptMarks.test.ts`.
- Modify:
  - `src/features/terminal/TerminalView.tsx` and its CSS (or a new `PromptMarks.css`).
  - `src/lib/keymap.ts` and `src/lib/appShortcuts.ts`.
  - `src/features/sessions/LiveApp.tsx`.
  - The appShortcuts and keymap tests.

**Interfaces:**
- Produces:
  ```ts
  export type Osc133 = { kind: "A" | "B" | "C" | "D"; exit: number | null };
  export function parseOsc133(data: string): Osc133 | null;           // data is the text after "133;"
  /** The buffer line to scroll to, or null when there is no prompt in that direction. */
  export function promptTarget(promptLines: number[], viewportTop: number, by: -1 | 1): number | null;
  // TerminalView gains prop:
  promptJump?: { token: number; by: -1 | 1 };   // each new token jumps once
  onPromptJump?: (result: { index: number; total: number; exit: number | null } | null) => void;
  // SessionShortcut gains: { kind: "prompt"; by: -1 | 1 }
  ```
- **Decoration rule:**
  - On A, register a marker at the cursor line and remember it as the current prompt. Drop markers whose `line` is -1.
  - On C, flag that a command ran.
  - On D with a code and a command ran since the last A, decorate the current prompt's marker. The decoration is one cell wide at column 0 and its element carries `aria-label="Exit code N"` and `title`. Then clear the flag.
  - Return `false` from the OSC handler, so other handlers still see the sequence.
- **Jump:**
  - Use `promptTarget(markers.map(m => m.line), term.buffer.active.viewportY, by)`.
  - On a target, `term.scrollToLine(line)` and report the index and total, plus the exit code of that prompt's command if decorated. Otherwise report null.
  - LiveApp announces through its existing announcer: "Prompt 3 of 7, exit 1", "Prompt 3 of 7", or "No earlier prompt" / "No later prompt".

- [ ] **Step 1:** Write the `promptMarks.test.ts` tests:
  - `parseOsc133` cases: `"A"`, `"D;1"` (exit 1), `"D"` (null), `"D;x"` (null), `"D;0;aid=2"` (exit 0), `"Z"` (null).
  - `promptTarget` cases, on lines `[0, 10, 20]`:
    - top 15, by -1 → 10
    - top 10, by -1 → 0
    - top 0, by -1 → null
    - top 10, by 1 → 20
    - top 20, by 1 → null
    - empty list → null
- [ ] **Step 2:** Write the keymap and appShortcuts tests: `prompt-prev` and `prompt-next` exist with the bindings from Global Constraints and map to `{kind:"prompt", by}`. Also check that no other command shares their binding; the keymap tests probably already check this generally.
- [ ] **Step 3:** Run `pnpm test promptMarks appShortcuts keymap`. Expected: FAIL.
- [ ] **Step 4:** Implement `promptMarks.ts`, the keymap entries, the shortcut kind and the LiveApp routing to the active session's `TerminalView`.
- [ ] **Step 5:** Wire the OSC handler and the decorations in `TerminalView`. Invoke the `frontend-design` skill for the mark's look, within the Global Constraints rule (not by colour alone, existing tokens).
- [ ] **Step 6:** Run `pnpm test` and `pnpm build`. Expected: PASS.
- [ ] **Step 7:** Commit: `feat(ui): prompt marks, exit-code gutter and prompt jumps`.

### Task 7: Profile page switch and read-only script view; docs

**Files:**
- Modify:
  - `src/features/settings/ProfilesPage.tsx`, its CSS and its tests.
  - The profiles API client (wherever `profiles.get`/`put` live; add `integration(profile)`).
  - `docs/PLAN.md` line 738.

- [ ] **Step 1:** Write the ProfilesPage tests:
  - Each profile shows a "Shell integration" switch (`role="switch"`, `aria-checked`) that toggles `shell_integration` in the saved doc.
  - A "View script" disclosure fetches `POST /v1/profiles/integration` with the edited profile.
  - It shows `script_path` and the script in a read-only `<pre>`, or the `reason` when `applies` is false.
  - A failed fetch shows "Couldn't load the script." and doesn't throw.
- [ ] **Step 2:** Run `pnpm test ProfilesPage`. Expected: FAIL.
- [ ] **Step 3:** Implement. The `frontend-design` skill decides the look within the page's existing patterns.
- [ ] **Step 4:** Update PLAN line 738 to say "make permanent" moves to phase 3 with the config safety core, and PSReadLine and prompt options come later.
- [ ] **Step 5:** Run `pnpm test`, `pnpm build`, `cargo test --workspace` and `pnpm check:generated`. Expected: PASS.
- [ ] **Step 6:** Commit: `feat(ui): shell integration switch and script view on the profiles page`.
