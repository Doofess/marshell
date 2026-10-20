# Phase 3g: exit gaps (subagent count, live mode) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Claude Code row shows how many subagents are running and the permission mode Claude is in now, so phase 3's exit check can pass.

**Architecture:** The core already folds Claude's hook events into a per-tab `LiveStatus` (`agents/claude/status.rs` → `Registry::update_live`). This slice adds `SubagentStart` to the installed hooks, keeps a set of running subagent ids beside each tab's live status, and reads `permission_mode` from hook bodies into the tab's `SessionInfo.mode` and its relaunch spec, so the chip, Resume and Restart follow Claude. The UI already renders `subagents` and `mode`; it gains one mode, "Don't ask".

**Tech Stack:** Rust (marshell-core, marshell-protocol, ts-rs), React + TypeScript, Vitest.

**Spec:** `docs/PLAN.md` §9, phase 3 exit criterion and "Slice 3g decisions (2026-10-21)"; Claude Code hooks reference (https://code.claude.com/docs/en/hooks): `SubagentStart`/`SubagentStop` carry `agent_id` and `agent_type`; `permission_mode` is one of `default`, `plan`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, and Manual arrives as `"default"`.

## Global Constraints

- Hooks off, a gated version or the per-CLI status switch: no live mode and no subagent count either (same gate as `fold_event`, `server/hook.rs` `fold_live`).
- Unknown values are left out of rows (PLAN "Unknown values"): a subagent count of 0 shows nothing; an unknown `permission_mode` string changes nothing.
- No new dependencies. `cargo fmt --all` before every Rust commit; `pnpm check:generated` after the protocol change.
- Tests and live checks never write the real `~/.claude` or `~/.marshell` (`MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS`).
- Commit messages: `type(scope): what changed, as a sentence`, ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A subagent whose `SubagentStop` never comes (cancelled, crashed): the count must not stay up forever. It clears at the main turn's `Stop`, at `SessionEnd`, and when the tab's process changes (restart, exit).
2. `SubagentStop` for Claude's internal agents (prompt suggestions, `/btw`) with no matching Start: a no-op, never a negative or wrapped count.
3. A `SubagentStart` for an id already running (a resumed subagent, a teammate message): counted once.
4. A hook body with `permission_mode` on a tab whose session is not Running, or on a plain shell row: nothing changes.
5. An install made before this slice (8 status entries) reads "Partial" and "Set up again" adds exactly the one missing entry; uninstall and Hooks off still remove everything Marshell added.

---

### Task 1: Live permission mode (core + protocol)

**Files:**
- Modify: `crates/protocol/src/api.rs` (`SessionMode`, ~L86), regenerate `src/generated/SessionMode.ts`
- Modify: `crates/core/src/profiles/detect.rs` (`mode_args`, ~L399)
- Modify: `crates/core/src/agents/claude/status.rs` (new `mode_of`)
- Modify: `crates/core/src/registry.rs` (new `set_live_mode`)
- Modify: `crates/core/src/server/hook.rs` (`fold_live`, ~L107)
- Test: `status.rs` unit tests, `registry.rs` unit tests, `crates/core/tests/` hook route test file that covers `fold_live` today

**Interfaces:**
- Produces: `SessionMode::DontAsk` (serde `"dont-ask"`); `pub fn mode_of(body: &serde_json::Value) -> Option<SessionMode>` in `agents/claude/status.rs`; `pub fn set_live_mode(&self, tab_id: &str, mode: SessionMode)` on `Registry`.

- [ ] **Step 1: Write the failing tests**
  - `status.rs` `mode_of_maps_every_documented_value`: `{"permission_mode":"default"}`→`Manual`, `"plan"`→`Plan`, `"acceptEdits"`→`AutoEdit`, `"auto"`→`FullAuto`, `"bypassPermissions"`→`Bypass`, `"dontAsk"`→`DontAsk`, `"manual"`→`Manual`; `"somethingNew"`, a number, and a missing field → `None`.
  - `detect.rs`: `mode_args("claude", DontAsk) == ["--permission-mode", "dontAsk"]`; `mode_args("codex", DontAsk)` and `mode_args("antigravity", DontAsk)` are empty.
  - `registry.rs` `live_mode_updates_the_row_and_the_relaunch_spec`: a Running claude row started in `Manual`; `set_live_mode(tab, Plan)` → `info.mode == Some(Plan)`, the relaunch spec's mode is `Plan`, one broadcast; the same call again broadcasts nothing; on a row not Running, or a row whose `agent` is None (a plain shell), nothing changes.
  - The hook route test: POST a `PreToolUse` event body with `"permission_mode":"plan"` for an open Claude tab (fake agent, known version as the existing `fold_live` tests set it up) → `GET /v1/sessions` shows `mode: "plan"`; with the status switch off the mode stays as launched.
  - `api.rs` serde test (the one at ~L878): `DontAsk` ⇄ `"dont-ask"`.

- [ ] **Step 2: Run them and see them fail** — `cargo test -p marshell-core mode_of live_mode mode_args` and `cargo test -p marshell-protocol`. Expected: compile errors for the missing variant and functions.

- [ ] **Step 3: Implement**
  - `SessionMode::DontAsk` after `Bypass`. `mode_args`: `("claude", DontAsk) => &["--permission-mode", "dontAsk"]`; the existing `_ => &[]` covers the others.
  - `mode_of`: reads `body["permission_mode"]` as a string and maps as the test says; anything else `None`.
  - `set_live_mode`: like `update_live` — open entry, `state == Running`, `info.agent.is_some()`, and a change; sets `info.mode` and `spec.mode`, broadcasts once.
  - `fold_live`: in the `Gate::Known` branch, for `HookRole::Event | HookRole::Permission` only (not the status line), `if let Some(m) = mode_of(body) { st.registry().set_live_mode(tab, m) }`.
  - Any `match` on `SessionMode` that no longer compiles (e.g. `layout.rs` remember, ~L54) treats `DontAsk` like the modes that ask first, unless the match is about launch flags.

- [ ] **Step 4: Run tests** — `cargo test -p marshell-core` and `cargo test -p marshell-protocol`, then `pnpm check:generated`. Expected: all pass; `SessionMode.ts` gains `"dont-ask"`.

- [ ] **Step 5: Commit** — `feat(sessions): follow the permission mode Claude Code reports, so the chip, Resume and Restart use the mode it is in now`

### Task 2: Subagent count (core)

**Files:**
- Modify: `crates/core/src/agents/claude/hooks.rs` (`STATUS_EVENTS` ~L19, the "Adds 8 hook entries" copy ~L427)
- Modify: `crates/core/src/agents/claude/status.rs` (new `fold_subagents`)
- Modify: `crates/core/src/registry.rs` (`Entry` gains the running set; `clear_live` clears it; new `update_subagents`)
- Modify: `crates/core/src/server/hook.rs` (`fold_live`)
- Test: `status.rs`, `registry.rs`, `hooks.rs` unit tests; the hook route test file

**Interfaces:**
- Produces: `STATUS_EVENTS: [&str; 9]` with `"SubagentStart"` before `"SubagentStop"`; `pub fn fold_subagents(running: &mut BTreeSet<String>, body: &Value) -> bool` (true when the set changed) in `status.rs`; `pub fn update_subagents(&self, tab_id: &str, body: &Value)` on `Registry`, which applies `fold_subagents` to the tab's set and, if it changed, sets `live.subagents` to the set's size (creating the live status from `status::blank` when there is none) and broadcasts.

- [ ] **Step 1: Write the failing tests**
  - `status.rs` `subagents_count_running_ids`: Start `a` → {a}; Start `b` → {a,b}; Start `a` again → unchanged (false); Stop `a` → {b}; Stop `zzz` (never started) → unchanged (false); main `Stop` → {}; Start `c`, `SessionEnd` → {}. A Start or Stop with no `agent_id` (or not a string) changes nothing.
  - `registry.rs` `subagent_count_follows_the_running_set_and_clears_with_the_process`: two Starts → `live.subagents == 2`, one broadcast each; a restart (`begin_restart`) or the process exiting clears the set, and the next Start counts 1.
  - `hooks.rs`: `install_appends_one_group_per_event` still passes with the new const; new `an_install_without_subagent_start_is_partial` — a settings text holding the 8 entries of the previous list (all Marshell's, bridge present) reads `HookInstall::Partial`, and its install plan adds exactly one group, `SubagentStart`. The install copy test: the headline says "Adds 9 hook entries" (derived from `STATUS_EVENTS.len()`).
  - Hook route test: two `SubagentStart` bodies then one `SubagentStop` on an open Claude tab → `live.subagents == 1` in `GET /v1/sessions`.

- [ ] **Step 2: Run them and see them fail** — `cargo test -p marshell-core subagent`. Expected: missing functions, count 0.

- [ ] **Step 3: Implement** as the Interfaces say. `fold_live` (Known gate, event/permission roles) calls `update_subagents` after `update_live`. The copy at ~L427 reads `format!("Adds {} hook entries so Marshell can show what Claude Code is doing", STATUS_EVENTS.len())`. `SubagentStart` gets the default 5 s timeout.

- [ ] **Step 4: Run tests** — `cargo test -p marshell-core` and `cargo test -p marshell-cli`. Expected: all pass (fix any test that pinned 8 status entries by deriving it from `STATUS_EVENTS`).

- [ ] **Step 5: Commit** — `feat(status): count the subagents Claude Code has running, from SubagentStart and SubagentStop`

### Task 3: UI (Don't ask, consent counts)

**Files:**
- Modify: `src/features/sidebar/types.ts` (`Mode` gains `"dont-ask"`), `src/features/window/SessionHeader.tsx` (`MODE_LABELS`: `"dont-ask": "Don't ask"`), and every `Record<Mode, …>` or exhaustive switch the TypeScript build flags
- Modify: `src/features/onboarding/consent.ts` (`claude.status.entries` 8 → 9) and the tests that pin 8/9 (`consent.test.ts`, `ConsentCard.test.tsx`, `consentCopy.test.ts` where Claude's numbers appear)
- Test: `src/features/window/SessionHeader.test.tsx` (or the file that tests `modeLabel`/`headerDanger`), `src/features/onboarding/*.test.*`

**Interfaces:**
- Consumes: `SessionMode` `"dont-ask"` from Task 1 (`src/generated/SessionMode.ts`).

- [ ] **Step 1: Write the failing tests** — `modeLabel("dont-ask") === "Don't ask"`; `headerDanger("dont-ask") === false`; `modeCaution("dont-ask")` gives no caution; the launcher's and the Mode ▸ menu's mode lists do not contain `"dont-ask"` (it is reported, never chosen); Claude's consent headline with status and approve reads "Add 10 hook entries", status only "Add 9 hook entries".
- [ ] **Step 2: Run** `pnpm test src/features/window src/features/onboarding src/features/sidebar src/features/launcher` — Expected: FAIL.
- [ ] **Step 3: Implement** the label, the type, and the counts. A row's meta line and the composer show "Don't ask" like any other non-Plan mode.
- [ ] **Step 4: Run** `pnpm test` and `pnpm build` — Expected: all pass, build clean.
- [ ] **Step 5: Commit** — `feat(ui): show the Don't ask mode Claude Code can report, and count its SubagentStart hook in the consent card`

### Task 4: Docs

**Files:** `docs/PLAN.md` (the hook-count line ~L263 "Add 9 hook entries" → "Add 10 hook entries"; the 3g slice line and decisions are already in from the plan commit), `docs/adr/0001-threat-model.md` only if it lists the installed events.

- [ ] **Step 1:** Update the counts and any event list that names the 8 status events.
- [ ] **Step 2: Commit** — `docs(plan): count SubagentStart in Claude Code's install`

### Task 5: Live check

Per CLAUDE.md "Live check (Windows)", with a scratch `MARSHELL_HOME` and `MARSHELL_CLAUDE_SETTINGS` in a scratch project, one Claude session started for the check. Findings in `docs/research/2026-10-21-exit-gaps-live-check.md`.

1. **Partial → Set up again:** install with the previous build's 8 entries (or write them), open the app on this branch: the row reads "Status limited · set up" or the Agents dialog says Partial; Set up again adds only `SubagentStart` (diff +1 group).
2. **Subagents:** prompt "Use two Explore subagents in parallel: one lists the files in this folder, the other prints the date. Then say done." The row shows "2 subagents" while both run, then nothing after Stop. Record the core's `live` JSON.
3. **Live mode:** Shift+Tab in the session (the Enter/Shift+Tab is part of the check) → the chip changes to the next mode within one hook event (send a prompt to raise one); Resume after closing starts in that mode (`--permission-mode` in the launched command line).
4. **Fixtures:** save one real `SubagentStart` and one `SubagentStop` body to `crates/core/tests/fixtures/claude/<version>/`, scrubbed as `docs/spikes` and the fixture scrub test require, and add a test that folds them (Start then Stop → 0).
5. Stop only what the check started, by PID, after listing the tree; remove the scratch folders; the real `~/.claude/settings.json` and `~/.marshell` keep their dates.
