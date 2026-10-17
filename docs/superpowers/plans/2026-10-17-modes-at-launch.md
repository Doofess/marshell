# Modes at Launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An agent session starts in the mode chosen in the launcher, using the CLI's native flag. The row and header show the mode it started in. Mode ▸ in the row menu, the header chip and the palette restart the agent in another mode, resuming its last conversation after an inline confirm.

**Architecture:** The core owns the mode. It travels in a new `SessionMode` protocol enum on `CreateSessionRequest`, `SessionInfo`, `SavedTab` and `LauncherChoice`. The core turns it into CLI flags inside `build_argv`, so Create, Resume, Reopen and a new Restart endpoint all start the CLI the same way. A spawn generation in the registry makes the killed process's late exit callback harmless during a restart. The frontend reads the mode from the snapshot (`toRow`) and only displays it.

**Tech Stack:** Rust (axum, ts-rs) in `crates/protocol` and `crates/core`; React + TypeScript and Vitest in `src/`.

**Spec:** `docs/PLAN.md` "Session modes" (the flag table) and §9's session row menu bullet. Session decisions from 2026-10-17 (user answers):
1. The chip shows the mode the session **started in**, and its accessible label says a change made inside the CLI isn't shown yet.
2. The launcher remembers Manual, Plan and Auto-edit per folder, but never Full auto or Bypass.
3. Restart confirms with an **inline bar in the pane**, like the close bar.
4. Antigravity: Full auto is `agy --mode auto`, and Resume and Restart use `agy --continue` (agy 1.4.0 `--help`).

## Global Constraints

- The modes are `manual`, `plan`, `auto-edit`, `full-auto` and `bypass`; the TS `Mode` in `src/features/sidebar/types.ts:7` uses the same strings. Their labels are Manual, Plan, Auto-edit, Full auto and Bypass (`modeLabel`).
- Hooks are off (the permission hook does not exist yet). The flags are the **native** column of PLAN "Session modes":
  - **Claude:** `--permission-mode manual|plan|acceptEdits|auto|bypassPermissions`.
  - **Codex:**
    - manual: `--sandbox read-only --ask-for-approval on-request`
    - plan: the same as manual (stand-in)
    - auto-edit: `--sandbox workspace-write --ask-for-approval on-request`
    - full-auto: `--approve-for-me`
    - bypass: `--dangerously-bypass-approvals-and-sandbox`
  - **Antigravity:** manual: none; plan: `--mode plan`; auto-edit: `--mode accept-edits`; full-auto: `--mode auto`; bypass: `--dangerously-skip-permissions`.
- Resume args: claude `--continue`, codex `resume --last`, antigravity `--continue`. Mode flags go **after** the resume args (`codex resume --last --sandbox …` is valid for codex-cli 0.163.0).
- A plain shell (no agent) has no mode: `None` everywhere, and a mode sent with one is ignored.
- A create that sends no mode starts with no mode flags and reports `mode: None`. No status is guessed.
- No keystrokes are ever typed into a terminal to change its mode.
- Generated bindings: after a protocol change, run `cargo test -p marshell-protocol` and commit `src/generated`. Run `cargo fmt --all` before pushing.
- Keep the TS mirror in step with Rust: `resumeCommand.ts` and `sessionModes.ts` (`launchArgs` with `hooksOn=false`) must show the same flags the core runs. Comments cross-reference the two.

## Review Focus

1. **A restart racing the old process's exit.** The killed process's `on_exit` must not mark the new process Exited. The generation test covers this.
2. **A restart refused before the kill:** cwd gone, profile removed, agent not installed. The old process must keep running and the row must stay unchanged.
3. **A layout.json written before this change:** `SavedTab` and `LauncherChoice` without `mode` must still load (serde default).
4. **Mode ▸ on a session that is not on screen:** it is shown first (as a plain click would), and then the bar appears on its pane.
5. **The palette `Mode: …` command with a plain shell focused:** nothing happens and nothing is thrown.

---

### Task 1: The mode in the protocol and in every start (Rust)

**Files:**
- Modify: `crates/protocol/src/api.rs`: add `SessionMode`, and add `mode` to `CreateSessionRequest`, `SessionInfo`, `SavedTab` and `LauncherChoice`.
- Modify: `crates/core/src/profiles/detect.rs`: add `mode_args`; `resume_args` for antigravity becomes `--continue`; `launch_argv` and `launch_argv_resume` take `mode`.
- Modify: `crates/core/src/registry.rs` (`Relaunch.mode`, plus the snapshot and `seed_ended` carry it), `crates/core/src/server/sessions.rs` (`resolve`, `create`, `reopen`, `resume`, `remember_choice`), `crates/core/src/layout.rs` (`remember`), `crates/core/src/server/mod.rs` (seed), and `crates/core/src/server/shell_panel.rs` (`mode: None`).
- Regenerate: `src/generated/*` (`SessionMode.ts` is new).
- Test: unit tests next to each function, in the existing `#[cfg(test)]` modules.

**Interfaces:**
- Produces: `#[serde(rename_all = "kebab-case")] pub enum SessionMode { Manual, Plan, AutoEdit, FullAuto, Bypass }`, which derives Copy, Eq, Serialize, Deserialize and TS with `#[ts(export)]`.
- Produces: `SessionInfo.mode: Option<SessionMode>`, the mode the process started in.
- Produces: `CreateSessionRequest.mode: Option<SessionMode>` (optional, like `prompt`).
- Produces: `SavedTab.mode` and `LauncherChoice.mode`, each `Option<SessionMode>` with `#[serde(default)]`.
- Produces: `pub fn mode_args(agent: &str, mode: SessionMode) -> &'static [&'static str]`.
- Produces: `launch_argv(profile, agent, prompt, mode: Option<SessionMode>)` and `launch_argv_resume(profile, agent, mode: Option<SessionMode>)`. `build_argv`'s `extra` is the resume args followed by the mode args.
- Produces: `resolve(st, profile_id, agent, prompt, resume, mode: Option<SessionMode>)`.

- [ ] **Step 1: Write the failing tests**
  - `detect.rs`:
    - `mode_args_match_the_plan_table`: assert every cell of the Global Constraints table, for example `mode_args("codex", SessionMode::Plan) == mode_args("codex", SessionMode::Manual)` and `mode_args("antigravity", SessionMode::FullAuto) == ["--mode", "auto"]`.
    - `resume_args("antigravity") == ["--continue"]`.
    - `launch_argv_resume` for claude with `Some(Bypass)` puts `claude --continue --permission-mode bypassPermissions` in the program line.
    - `launch_argv` with `agent: None` and `Some(Plan)` returns the profile command unchanged.
  - `registry.rs`: `snapshot_carries_mode`. A tab reserved with `Relaunch { mode: Some(Plan), .. }` lists `mode == Some(Plan)`, and `saved_tabs()` keeps it.
  - `layout.rs`:
    - `remember_keeps_safe_modes_only`: `remember(.., Some(Bypass))` and `remember(.., Some(FullAuto))` store `None`; `Some(Plan)` is stored.
    - A `LayoutDoc` JSON with no `mode` keys parses.
- [ ] **Step 2: Run** `cargo test -p marshell-core mode_args resume_args remember snapshot_carries_mode`. Expected: it fails to compile (no `SessionMode`). That counts as RED.
- [ ] **Step 3: Implement.**
  - `create` passes `req.mode`, but only when `agent.is_some()`. The mode flows into `Relaunch` and `SessionInfo`.
  - `remember_choice` gets the mode, and `layout::remember` drops `FullAuto` and `Bypass` to `None`.
  - Resume and Reopen use the saved `Relaunch.mode`.
  - Update the `resume_args` doc comment.
  - Run `cargo test -p marshell-protocol` to regenerate the bindings.
- [ ] **Step 4: Run** `cargo test --workspace` and `pnpm check:generated` (after `git add src/generated`). Expected: all pass, and the frontend still compiles (`pnpm build`), because the new TS fields are optional or nullable. Fix any TS fixture that builds a `SessionInfo` literal by adding `mode: null`.
- [ ] **Step 5: Commit** `feat(core): start an agent session in its mode with the CLI's own flags, and keep the mode for Resume and the launcher`.

### Task 2: Restart in another mode (Rust)

**Files:**
- Modify: `crates/protocol/src/api.rs`: add `RestartRequest { mode: SessionMode, cols: u16, rows: u16 }` with `#[ts(export)]`.
- Modify: `crates/core/src/registry.rs`: add a spawn generation and `begin_restart`.
- Modify: `crates/core/src/server/sessions.rs`: add `restart`. Pass the generation through `spawn_into_map` and its callers, including `shell_panel.rs`.
- Modify: `crates/core/src/server/mod.rs`: add the route `POST /v1/sessions/{tab}/restart` next to `/resume`.
- Test: `registry.rs` tests, plus the sessions handler tests where the resume handler is tested (follow that file's pattern).

**Interfaces:**
- Consumes: Task 1's `SessionMode`, `resolve(.., resume=true, mode)` and `Relaunch.mode`.
- Produces: `Entry.generation: u64`. `reserve` and `begin_resume` set a fresh generation and return it (`begin_resume` returns `(Relaunch, SessionState, u64)`).
- Produces: `pub fn mark_exited(&self, tab_id: &str, generation: u64, code: i32)`, which ignores an exit whose generation is not the entry's current one.
- Produces: `pub fn begin_restart(&self, tab_id: &str, mode: SessionMode) -> Result<(Relaunch, u64), ResumeRefusal>`. It returns `NotFound` for an unknown tab. It sets the mode on both `info` and `spec`, sets the state to Running, resets `command`, bumps the generation and broadcasts.
- Produces: `spawn_into_map(.., generation: u64)`. Its `on_exit` calls `mark_exited(&tab, generation, code)`.
- Produces: the HTTP endpoint `POST /v1/sessions/{tab}/restart`, body `RestartRequest`.
  - 200 returns `CreateSessionResponse` with the same tab and number.
  - 404 for an unknown tab.
  - 409 with `ResumeError { reason }` for a plain shell ("Only an agent session has a mode."), a removed profile, a missing folder or an agent that isn't installed. All of these checks run **before** the old process is touched.
  - A spawn failure after the kill sets the row to `Exited { code: -1 }` and returns the error.

- [ ] **Step 1: Write the failing tests**
  - `registry.rs`:
    - `stale_exit_is_ignored`: reserve with generation g1, call `begin_restart` to get g2, then `mark_exited(tab, g1, 0)`. The state stays Running and `mode` is the new one. `mark_exited(tab, g2, 3)` then gives `Exited { code: 3 }`.
    - `begin_restart_unknown_tab_is_not_found`.
  - `sessions.rs`: `restart_refuses_a_plain_shell_without_killing_it`, written in the style of the existing handler tests. If those tests cannot spawn, test the refusal ordering through a helper `restart_refusal(info, profiles_known, cwd_exists, agent) -> Option<String>` that is extracted and unit-tested.
- [ ] **Step 2: Run** `cargo test -p marshell-core stale_exit begin_restart restart_`. Expected: compile failure or FAIL.
- [ ] **Step 3: Implement** as in Interfaces. The handler order is: snapshot lookup, refusal checks, `resolve(.., true, Some(mode))`, `check_agent_installed`, `begin_restart`, remove and kill the old session, `launch`.
- [ ] **Step 4: Run** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test -p marshell-protocol` (bindings) and `pnpm check:generated`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(core): restart an agent session in another mode in the same row, resuming its last conversation`.

### Task 3: The launcher's mode chip and the started-in chip (frontend)

**Files:**
- Modify: `src/features/launcher/launchChoice.ts`: `Mode` becomes the sidebar `Mode` (all five, lowercase ids). Labels come from `modeLabel`, and the sentence uses the label.
- Modify: `src/features/launcher/Launcher.tsx`: replace `chips` with `paceChip?: boolean` (default true). The mode chip always shows for an agent program; a risky mode (`headerDanger`) shows ⚠ in the chip.
- Modify: `src/features/sessions/LiveLauncher.tsx` (`INITIAL.mode = "manual"`, `paceChip={false}`, remembered mode from `LauncherChoice.mode`) and `src/features/sessions/launchPrograms.ts` (`launchRequest` adds `mode` for an agent).
- Modify: `src/features/sessions/toRow.ts` (`mode: info.mode ?? undefined`), `src/features/window/SessionHeader.tsx` (chip `aria-label` and `title`: "Started in Plan. A change made inside the CLI isn't shown yet.") and `src/features/modes/sessionModes.ts` (antigravity full-auto `["--mode", "auto"]`; doc comment points at `mode_args`).
- Modify: `src/features/sessions/resumeCommand.ts`: `resumeCommand(agent, profileName, mode?)` shows `agy --continue` and appends `launchArgs(agent, mode, false).args` when a mode is known.
- Update the fixtures and stories that used the capitalized launcher modes.
- Test: the existing test files next to each module.

**Interfaces:**
- Consumes: the Task 1 bindings `SessionMode` and `SessionInfo.mode`.
- Produces: `resumeCommand(agent: string | null, profileName: string, mode?: Mode): string`.

- [ ] **Step 1: Write the failing tests**
  - `launchPrograms.test.ts`: `launchRequest` for claude with mode `plan` includes `mode: "plan"`; for a plain shell it has no `mode` key.
  - `toRow.test.ts`: `info.mode = "bypass"` gives `row.mode === "bypass"`, and `null` gives `undefined`.
  - `SessionHeader.test.tsx`: the chip's label contains "Started in Plan" and "isn't shown yet".
  - `resumeCommand.test.ts`:
    - `resumeCommand("antigravity", "pwsh") === "agy --continue"`;
    - `resumeCommand("claude", "pwsh", "bypass") === "claude --continue --permission-mode bypassPermissions"`;
    - codex with `auto-edit` gives `codex resume --last --sandbox workspace-write --ask-for-approval on-request`.
  - `sessionModes.test.ts`: antigravity full-auto launches with `--mode auto`.
  - `Launcher.test.tsx`: with `paceChip={false}`, the mode chip is present and the pace chip absent; Bypass shows ⚠; all five modes cycle.
- [ ] **Step 2: Run** `pnpm vitest run src/features/launcher src/features/sessions src/features/window/SessionHeader.test.tsx src/features/modes`. Expected: the new tests FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(launcher): pick the mode a session starts in, and show it as the mode it started in`.

### Task 4: Mode ▸, the header chip and the palette restart with an inline bar (frontend)

**Files:**
- Modify: `src/lib/api/sessions.ts` (`restart(ep, tabId, mode, size)`, mirroring `resume`) and `src/features/sessions/sessionActions.ts`:
  - `restarting: string | null`, `askRestart(id, mode)`, `cancelRestart()` and `confirmRestart()`.
  - On success, `resumed[id]++`, so the terminal remounts (`key` in LiveApp).
  - A 409 reason goes to the toast.
- Modify: `src/features/sidebar/sessionMenu.ts`. Add the `mode` item, "Mode ▸", for an agent row (new fact `agent: boolean`). Running it opens a second menu at the same anchor with `modeMenuItems(current)`: the five labels, the current one marked "(current)" and left out of the run.
- Modify: `src/features/sessions/sessionMenuRun.ts` and `LiveApp.tsx` to wire the menu, the header chip (a button that opens the same mode menu) and the palette `mode-*` commands for the focused agent row.
- Create: `src/features/sessions/RestartBar.tsx`: `CloseBar`'s pattern with the labels Cancel and Restart, Cancel focused first, Escape cancels, `role="alert"`. Reuse `CloseBar.css` classes rather than adding a new stylesheet.
- Modify: `src/features/window/MainWindow.tsx`, `SplitView.tsx` and `Pane.tsx`. Add a new prop, `paneBar?: { id: string; node: ReactNode }`:
  - With two or more panes on screen, the pane whose row id matches renders `node` in place of its `.pane__header`.
  - Otherwise `node` goes in the top-bar `headerBar` slot.
  - A session that isn't on screen is shown first through LiveApp's existing pick path (`showInPane`).
- Modify: `docs/PLAN.md`. In "Session modes", change the Antigravity Full auto cell to `--mode auto`, and replace the "Switching mid-session" bullet's no-hook case with the restart-with-confirm flow. In §9's row menu bullet, Mode ▸ is now done. Also note `--continue` in `docs/research/2026-10-15-local-install-check.md` (agy 1.4.0).
- Test: `sessionMenu.test.ts`, `sessionActions.test.ts`, `RestartBar.test.tsx` (new), `MainWindow.test.tsx` and `LiveApp.test.tsx` (if it covers the menu).

**Interfaces:**
- Consumes: Task 2's endpoint, and Task 3's `resumeCommand(agent, profile, mode)` and `row.mode`.
- Produces: `restartBarText(info, mode, working: boolean)`. It returns `Restart 3 billing in Bypass? It ends and runs claude --continue --permission-mode bypassPermissions.`, followed by ` Claude Code may be mid-task.` when it is working.

- [ ] **Step 1: Write the failing tests**
  - `sessionMenu.test.ts`: an agent row has "Mode ▸" and a plain shell doesn't. `modeMenuItems("plan")` lists five items and marks Plan current.
  - `sessionActions.test.ts`: `confirmRestart` calls the api with the mode and bumps `resumed`; a 409 sets the toast and leaves `resumed`.
  - `RestartBar.test.tsx`: Cancel is first and `data-initial-focus`; `closeBarKey("Escape")` is reused.
  - `restartBarText` gives the exact copy above, with and without `working`.
  - `MainWindow.test.tsx`:
    - In split with 2 panes, `paneBar` for billing replaces billing's pane header and the top bar has no bar.
    - With one pane, the bar is in `window__bar`.
- [ ] **Step 2: Run** the named test files. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test` and `pnpm build`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(sessions): Mode ▸, the header chip and the palette restart a session in another mode after an inline confirm`.
