# Slice 6k: maintenance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix issues #65, #62 and #63, the confirmed leads from PR #79's review, and the launcher-shortcut finding from the 6j2 live check.

**Architecture:** These are independent fixes, each in its own area: the terminal fit (frontend), the JSONC editor (core config), the Agents dialog (frontend plus one new core route), WSL lookups and resume (core), and the window's shortcut effect (frontend). No new dependencies.

**Tech Stack:** Rust (axum core, jsonc-parser 0.34 CST), React + TypeScript (Vitest, node, no DOM), ts-rs generated types.

**Spec:** `docs/PLAN.md` §9, "Slice 6k decisions (2026-11-03)". The issues are #65, #62 and #63. The leads' evidence is in this plan's tasks.

## Global Constraints

- Commit messages read `type(scope): what changed, as a sentence`. Run `cargo fmt --all` before every Rust commit.
- Rust tests run with `-j 4`, for example `cargo test -j 4 -p marshell-core <filter>`. The default job count runs rustc out of memory on this machine.
- A new protocol type goes in `crates/protocol`. Then run `cargo test -p marshell-protocol` and `pnpm check:generated`, and commit `src/generated/`.
- A new route sits in the token-guarded router and has a "refused without the bearer" test.
- Never hold a `MutexGuard` across `.await` or I/O. A `wsl.exe` call runs in `spawn_blocking`.
- UI copy is sentence case, puts the subject first, and has no "!" and no "successfully". Don't guess a status value.
- No new dependencies (SCOPE.md).

## Review Focus

1. **#63, an array holding only Marshell's group.** Appending to `[]` and removing again gives back `[]` exactly, and an absent key that the install created is removed whole. Task 2 tests this.
2. **#62, a CLI that isn't found and whose hooks file is unparsable or refused.** The section never offers a "Hooks off" that can't work: `actions` returns `[]` when the file is refused. Task 3 tests this.
3. **#65, a tab hidden while the window is resized, then shown.** It fits to the new size once shown. The refit in the existing `visible` effect does this, and the gate must not block it. Task 1 tests this.
4. **The distro list with two profiles for one distro in different case.** It is listed once, spelt as the first profile spells it. Task 4 tests this.
5. **Reopen while the launcher's search field has focus.** Ctrl+Shift+Alt+T still passes, because it types no text, and no other shortcut does. Task 8 tests this.

---

### Task 1: A hidden terminal doesn't fit itself (#65)

**Files:**
- Create: `src/features/terminal/fitGate.ts`, `src/features/terminal/fitGate.test.ts`
- Modify: `src/features/terminal/TerminalView.tsx` (the ResizeObserver at about line 250, the font effect at about line 313, and the `visible` effect at about line 333)

**Interfaces:**
- Produces: `export function shouldFit(visible: boolean, width: number, height: number): boolean`

- [ ] **Step 1: Write the failing test** in `fitGate.test.ts`. `shouldFit(false, 800, 600)` is false. `shouldFit(true, 0, 600)` and `shouldFit(true, 800, 0)` are false. `shouldFit(true, 800, 600)` is true.
- [ ] **Step 2: Run it.** `pnpm test src/features/terminal/fitGate.test.ts` fails, because the module is missing.
- [ ] **Step 3: Implement `shouldFit` and use it.** In `TerminalView`, keep `visible` in a ref (`visibleRef.current = visible` on each render). Add a local `fitIfShown()` that calls `fit.fit()` only when `shouldFit(visibleRef.current, el.clientWidth, el.clientHeight)`. Use it in the ResizeObserver's frame and in the font effect. The `visible` effect keeps its own refit when `visible` turns true, gated on the host's size only, because the ref may not have updated yet. The first `fit.fit()` when the terminal is made stays as it is.
- [ ] **Step 4: Run.** `pnpm test src/features/terminal` passes.
- [ ] **Step 5: Commit.** `fix(terminal): a hidden tab keeps its size instead of fitting its program to 10x4`

### Task 2: Appending and removing a last array element gives back the bytes (#63)

**Files:**
- Modify: `crates/core/src/config/jsonc.rs` (`apply_to_cst`, the `Set "-"` and `Remove` arms on `Cur::Arr`, and its tests)
- Modify: `crates/core/src/agents/codex/hooks.rs` (remove the `#[ignore]` from `removing_a_trailing_group_restores_the_original_bytes`, and make `remove_takes_only_ours` compare exact text)
- Modify: `crates/core/src/agents/claude/hooks.rs` tests (add `removing_a_trailing_group_restores_the_original_bytes`, following the Codex one)

**Interfaces:**
- Consumes and produces: `apply_ops(text: &str, ops: &[JsonOp]) -> Result<String, EditError>`. Its signature doesn't change.

- [ ] **Step 1: Write the failing tests** in `jsonc.rs`. Add `append_then_remove_of_a_last_element_restores_the_bytes`: for each of `{"a":[{"x":1}]}`, `{"a":[{"x":1},]}`, `{"a":[\n  {"x":1}\n]}`, `{"a":[\r\n  {"x":1}\r\n]}` and `{"a":[]}`, apply `Set /a/- {"y":2}`, then `Remove /a/<last index>`, and `assert_eq!` the result to the input. Add `append_keeps_a_trailing_comma`: `{"a":[1,]}` plus `Set /a/- 2` parses to `[1,2]`, and the comma before `]` is still in the text. Turn on the Codex test and add the Claude one.
- [ ] **Step 2: Run.** `cargo test -j 4 -p marshell-core jsonc` and `cargo test -j 4 -p marshell-core trailing_group` fail on the newline before `]`.
- [ ] **Step 3: Implement.** Appending puts `, <element>` straight after the current last element and keeps whatever stood between it and `]`: whitespace, a trailing comma, comments. For an empty array it uses the CST's own append. Removing a last element takes the span from the end of the element before it to the end of the removed one, so the text after it (a trailing comma, the whitespace before `]`) stays. Removing an element that isn't last keeps today's behaviour. If jsonc-parser's `CstArray` can't give these results through its node API, do these two cases as text splices on the CST's ranges and re-parse. Every existing `jsonc` test must still pass: comments, BOM, CRLF and `set_replaces_an_array_element_at_its_indent`.
- [ ] **Step 4: Run.** `cargo test -j 4 -p marshell-core` passes, with no test ignored for #63.
- [ ] **Step 5: Commit.** `fix(config): removing a trailing hook group gives back the file's original bytes` (closes #63)

### Task 3: A CLI that isn't found but still holds Marshell's hooks keeps its section (#62)

**Files:**
- Modify: `src/features/agents/agentsState.ts` (`actions`, `setupLine`), `src/features/agents/AgentsSection.tsx` (the two section conditions at about line 122)
- Test: `src/features/agents/agentsState.test.ts`, and the AgentsSection test file next to it

**Interfaces:**
- Produces: `export const leftover = (s: AgentSetup): boolean => !s.found && s.hooks !== "none"` in `agentsState.ts`

- [ ] **Step 1: Write the failing tests.**
  - `actions({found: false, hooks: "installed", ...})` is `["hooksOff"]`. The same with `hooks: "orphaned"` or `"partial"` is `["hooksOff"]`. With `hooks: "none"` it is `[]`. A refused file (`settings_state: "unparsable"`) gives `[]`.
  - `setupLine` for not found with `hooks !== "none"` is exactly `` `${name} was not found on this computer, but Marshell's hooks are still in ${file}. Turn them off to remove them.` ``, where `file` is `hooks_path ?? settings_path`.
  - A rendered AgentsSection with Codex `found: false, hooks: "installed"` and no distros shows Codex's section with a "Hooks off" button and no "Set up". With `hooks: "none"` the section isn't shown.
- [ ] **Step 2: Run.** `pnpm test src/features/agents` fails.
- [ ] **Step 3: Implement.** `actions` returns `leftover(s) && !refused(s) ? ["hooksOff"] : []` when the CLI isn't found. `setupLine` says the line above for a leftover, before its not-found line. Both section conditions become `found || distros > 0 || leftover(setup)`. The skill row, version, hooks-file line and approve line stay found-only.
- [ ] **Step 4: Run.** `pnpm test src/features/agents` passes.
- [ ] **Step 5: Commit.** `fix(agents): a CLI that isn't found keeps its section while Marshell's hooks are in its files` (closes #62)

### Task 4: The core sends the WSL distros the Agents dialog may offer

**Files:**
- Modify: `crates/protocol/src/agents.rs` (new type), `crates/core/src/server/mod.rs` (`wsl_distros`, the router at about line 1087), `crates/core/src/server/agents.rs` (handler)
- Modify: `src/lib/api/agents.ts`, `src/features/agents/AgentsSection.tsx` (the effect at about line 435), `src/features/agents/agentsState.ts` (delete `wslDistros`) and its test
- Test: the `server` tests for agents routes, `src/lib/api/agents.test.ts`

**Interfaces:**
- Produces (protocol): `pub struct WslDistros { pub distros: Vec<String> }`, with `#[ts(export)]` like its neighbours
- Produces (route): `GET /v1/agents/wsl-distros` → `WslDistros`
- Produces (TS): `export const listWslDistros = (ep: Endpoint) => api<WslDistros>(ep, "GET", "/v1/agents/wsl-distros")`

- [ ] **Step 1: Write the failing tests.**
  - Core: `wsl_distros` on profiles `wsl -d Ubuntu`, `wsl -d ubuntu`, `wsl.exe` (no `-d`) and `wsl --distribution=Debian` gives `["Ubuntu", "Debian"]`, deduplicated case-insensitively in first-seen spelling.
  - The route answers the same list.
  - The route is refused without the bearer.
  - TS: `listWslDistros` calls GET `/v1/agents/wsl-distros`.
- [ ] **Step 2: Run.** `cargo test -j 4 -p marshell-core wsl_distros` and `pnpm test src/lib/api/agents.test.ts` fail.
- [ ] **Step 3: Implement.** Add the case-insensitive dedupe to `wsl_distros`. `detected_distro` keeps working unchanged. The handler reads `profiles_snapshot()` and needs no `spawn_blocking`, because nothing is started. AgentsSection sets `distros` from `listWslDistros(...).distros`, replacing `profiles.get` + `wslDistros`, and keeps the same error swallowing. Delete `wslDistros` and its tests.
- [ ] **Step 4: Run.** `cargo test -j 4 -p marshell-protocol`, `pnpm check:generated`, `cargo test -j 4 -p marshell-core` and `pnpm test src/features/agents src/lib/api` all pass.
- [ ] **Step 5: Commit.** `fix(agents): the Agents dialog offers only the WSL distros the core accepts`

### Task 5: Resuming an ended WSL agent tab resumes its own conversation

**Files:**
- Modify: `crates/core/src/server/sessions.rs` (`resume_session`, about line 1300)
- Test: the `sessions.rs` test module, beside `resume_session_prefers_the_tab_id` (about line 2068)

- [ ] **Step 1: Write the failing test** `resume_session_accepts_a_wsl_row`. Record the row with `st.history().note_event("claude", "t1", None, &body, 1, Some("Ubuntu"))`, where the body's `transcript_path` is a Linux path such as `/home/u/.claude/projects/p/abc-1.jsonl`, following the pattern at `history/recorder.rs:1135-1148`. Then assert that `resume_session(&st, "t1", Some("claude"))` is `Ok(Resume::Fresh)`, because the file doesn't exist in the test.
- [ ] **Step 2: Run.** `cargo test -j 4 -p marshell-core resume_session` fails with `Ok(Resume::Last)`.
- [ ] **Step 3: Implement.** Read the distro with `crate::history::archive::read_meta(st.home(), agent, &meta.session_id).ok().flatten().and_then(|m| m.distro)` and call `sidecar_allowed_in(agent, &meta.transcript_path, &meta.session_id, distro.as_deref())`. This covers Resume, Restart and "Open in system terminal" (`outside.rs:107`).
- [ ] **Step 4: Run.** `cargo test -j 4 -p marshell-core resume_session` passes.
- [ ] **Step 5: Commit.** `fix(history): resuming an ended WSL agent tab resumes its own conversation instead of the last one`

### Task 6: WSL lookups that work on any Windows language, in one call

**Files:**
- Modify: `crates/core/src/wsl.rs` (`default_distro`, `parse_default_distro`), `crates/core/src/server/agents.rs` (`distro_target`, about line 278)
- Test: the `wsl.rs` tests and the `server/agents.rs` tests

**Interfaces:**
- Produces: `fn cached_default(cache: &Mutex<Option<String>>, fetch: impl FnOnce() -> Option<String>) -> Option<String>` (private, `wsl.rs`)
- Produces: `fn split_home_and_bridge(out: &str) -> Option<(String, String)>` (private, `server/agents.rs`)

- [ ] **Step 1: Write the failing tests.**
  - `cached_default` with a `fetch` that returns `None` doesn't store it, so a second call runs `fetch` again. A `Some("Ubuntu")` is stored, and the next `fetch` isn't called.
  - `split_home_and_bridge("/home/u\n/mnt/c/x/marshell-hook.exe")` is `Some(("/home/u", "/mnt/c/x/marshell-hook.exe"))`. CRLF is trimmed. One line, or an empty part, is `None`.
- [ ] **Step 2: Run.** `cargo test -j 4 -p marshell-core wsl` fails.
- [ ] **Step 3: Implement.**
  - `default_distro` asks the default distro itself, with `run_wsl(&["--exec", "sh", "-c", r#"printf %s "$WSL_DISTRO_NAME""#], LIST_TIMEOUT)`, and decodes the output as UTF-8. It is only called for a running tab's bare `wsl.exe`, so that distro is already running. It goes through `cached_default` on a `static Mutex<Option<String>>`. Delete `parse_default_distro` and the test of the `(Default)` marker.
  - `distro_target` makes one call, `run_wsl(&["-d", distro, "--exec", "sh", "-c", r#"printf '%s\n' "$HOME"; wslpath -u "$1""#, "sh", bridge_exe], ...)`. The bridge path goes as an argument, never interpolated into the script. The output goes through `split_home_and_bridge` and then `distro_target_of`.
- [ ] **Step 4: Run.** `cargo test -j 4 -p marshell-core` passes.
- [ ] **Step 5: Commit.** `fix(wsl): find the default distro on any Windows language and look up a distro's home and bridge in one call`

### Task 7: One agent-to-program table, and an opener that finds Windows

**Files:**
- Modify: `crates/core/src/registry.rs` (`mark_hook_agent`, about line 714), `crates/core/src/agents/install.rs` (`bare_name`, about line 110), `crates/core/shell/marshell-open`
- Test: the `registry.rs` tests, the `shell.rs` tests (beside `the_opener_refuses_non_http_urls`, about line 534)

- [ ] **Step 1: Write the tests.**
  - `the_opener_finds_windows_through_systemroot` asserts that the `marshell-open` entry in `FILES` contains `%SystemRoot%`. It fails now.
  - `a_hook_mark_names_the_agents_program`: for each of `claude`, `codex` and `antigravity`, `mark_hook_agent` sets the foreground program to `agent_command(agent).unwrap()`. This one guards against drift and already passes.
- [ ] **Step 2: Implement.**
  - `mark_hook_agent` uses `let Some(program) = crate::profiles::detect::agent_command(agent) else { return false };`, and `bare_name` is replaced by calls to `agent_command`.
  - `marshell-open` sets `root="$(cmd.exe /d /c 'echo %SystemRoot%' 2>/dev/null | tr -d '\r')"`, falls back to `C:\Windows` when that is empty, and runs `exec "$(wslpath -u "$root\\System32\\rundll32.exe")" url.dll,FileProtocolHandler "$1"`. The URL check stays first.
- [ ] **Step 3: Run.** `cargo test -j 4 -p marshell-core` passes.
- [ ] **Step 4: Commit.** `refactor(core): one agent-to-program table, and the WSL opener finds Windows through %SystemRoot%`

### Task 8: Reopen closed session works while the launcher is open (6j2 live check finding 10)

**Files:**
- Modify: `src/features/sessions/launcherOpens.ts`, `src/features/sessions/LiveApp.tsx` (the session-shortcut effect at about line 1303)
- Test: `src/features/sessions/launcherOpens.test.ts`

**Interfaces:**
- Produces: `export const passesLauncher = (kind: AppShortcut["kind"]): boolean => kind === "reopen"` in `launcherOpens.ts`, using the shortcut union type from `src/lib/appShortcuts.ts`

- [ ] **Step 1: Write the failing test.** `passesLauncher("reopen")` is true. `"launcher"`, `"palette"`, `"cheatsheet"`, `"close"` and `"goto"` are false.
- [ ] **Step 2: Run.** `pnpm test src/features/sessions/launcherOpens.test.ts` fails.
- [ ] **Step 3: Implement.** The effect still returns early for an overlay, review or `closeAsk`. While `launcherOpen`, it attaches a listener that acts only on a shortcut where `passesLauncher(kind)` is true, and it ignores `isTextBox`, because the launcher's search field has focus and the key types no text. That listener prevents the key's default, ignores `e.repeat`, calls `setLauncherOpen(false)` and then `actions.handle(shortcut)`.
- [ ] **Step 4: Run.** `pnpm test src/features/sessions` passes.
- [ ] **Step 5: Commit.** `fix(ui): Ctrl+Shift+Alt+T reopens the last closed session while the launcher is open`

### Task 9: Live check and research note

Run the `marshell-live-check` skill with a scratch `MARSHELL_HOME`, a scratch `CODEX_HOME`, and a `PATH` with Codex's folder taken out.

- [ ] **#65.** Open two shell tabs, then show the second. The first tab's size, as read from the core (`/v1/sessions` or the recording), stays at its full size and never becomes 10x4. Resize the window and show the first tab again: it takes the new size.
- [ ] **#62.** Seed the scratch `CODEX_HOME/hooks.json` with Marshell's hooks (use Set up with Codex on PATH, then restart without it). The Agents dialog shows Codex's section with the leftover line and "Hooks off". Apply it, and the section goes away.
- [ ] **Finding 10.** On an empty window, the auto-opened launcher closes and the last closed session reopens on Ctrl+Shift+Alt+T.
- [ ] Write the findings in `docs/research/2026-11-03-maintenance-live-check.md` and commit: `docs(research): record slice 6k's live check`
