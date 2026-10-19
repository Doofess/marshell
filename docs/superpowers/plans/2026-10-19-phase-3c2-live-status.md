# Live status (slice 3c2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Claude Code session's row shows what it is doing, from its hook events and status line, with the context and plan rings, a recap, a version gate and a pty fallback.

**Architecture:** The core folds each hook post and status line into a `LiveStatus` on the tab's `SessionInfo`. The registry broadcasts it in the existing `sessions` frame on `/v1/events`, so no new socket frame is needed. A pure normaliser (`agents/claude/status.rs`) does the folding and is tested against the recorded 2.1.302 fixtures. The UI only renders `info.live`. A gated, unreadable-version or switched-off session gets no `live`; its row says "Status limited" and the pty fallback (Working, Idle, Ended) fills `info.activity`.

**Tech Stack:** Rust 2024 (axum, tokio watch), ts-rs, React/TS, Vitest (`renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §2 (SessionEvent), "Sidebar rows" status table (lines 167–207), §9 "Slice 3c decisions" item 4 and "Slice 3c2 decisions (2026-10-19)".

## Global Constraints

- Unknown values are `None` and left out of the UI: no dash, no zero, no guessed status (PLAN line 207, "no guessed status values").
- Ring colours: green below 70%, yellow from 70%, red from 90% (PLAN line 905).
- Version gate: same minor line as a recorded fixture set (2.1.x) and not older than the oldest (2.1.302). Anything else, or an unreadable version, is "Status limited" with a doctor note naming the version.
- The pty fallback never yields Needs you, Done or Error.
- Row copy: "Status limited · set up" links to setup when hooks are missing (PLAN line 205).
- Generated types in `src/generated/` come from `cargo test -p marshell-protocol`, never by hand. Run `cargo fmt --all` before every Rust commit.
- Never write the real `~/.claude`, `~/.codex` or `~/.marshell`. Fixtures never hold the user's e-mail.
- No new dependency.

## Review Focus

- An event for a tab that has closed, or one with no tab: ignored, no panic, no leak.
- Events out of order or repeated (Stop before UserPromptSubmit, a second SessionStart): the state stays valid.
- A status line with `context_window` or `rate_limits` missing, or `used_percentage` null or above 100: the ring hides or clamps, never NaN.
- A Claude version of 2.2.0, 3.0.0, 2.0.9, "garbage" or none: gated, no `live`.
- A session that ends mid-turn (SessionEnd, or the process exits): never stays Working.
- A hook post for an agent whose normalisation is switched off: stored in the log as before, no `live`.

---

### Task 1: Protocol types

**Files:**
- Modify: `crates/protocol/src/api.rs` (SessionInfo, Settings)
- Test: the existing ts-rs export test in `crates/protocol`

**Interfaces:**
- Produces, in `marshell_protocol::api`:
  - `enum LiveState { Idle, Working, NeedsPermission, NeedsQuestion, Done, Error, Ended }` (`snake_case`, `#[ts(export)]`)
  - `struct Recap { text: String, at_ms: u64 }`
  - `struct LiveStatus { state: LiveState, phrase: String, since_ms: u64, recap: Option<Recap>, model: Option<String>, effort: Option<String>, context_pct: Option<f32>, five_hour_pct: Option<f32>, seven_day_pct: Option<f32>, subagents: u32 }`
  - `enum Activity { Working, Idle }`
  - `SessionInfo.live: Option<LiveStatus>` and `SessionInfo.activity: Option<Activity>`, both `#[serde(default)]`
  - `SessionInfo.status_note: Option<String>`, `#[serde(default)]`: why status is limited ("Claude Code 2.0.4 is older than the versions Marshell knows", "Live status is off for Claude Code")
  - `Settings.status_off: Vec<String>` (`#[serde(default)]`), the agent ids whose normalisation is off

- [ ] **Step 1:** Add the types and fields. Fix every `SessionInfo { .. }` and `Settings { .. }` construction the compiler reports with `live: None, activity: None, status_note: None` / `status_off: vec![]`.
- [ ] **Step 2:** Run `cargo test -p marshell-protocol` and `pnpm check:generated`. Expected: pass, with `src/generated/LiveStatus.ts`, `LiveState.ts`, `Recap.ts`, `Activity.ts` new and `SessionInfo.ts`, `Settings.ts` changed. Commit the generated files.
- [ ] **Step 3:** Run `cargo test --workspace`. Expected: pass.
- [ ] **Step 4:** Commit `feat(protocol): live status, activity and status note on a session`.

### Task 2: The normaliser

**Files:**
- Create: `crates/core/src/agents/claude/status.rs`; add `mod status;` in `agents/claude/mod.rs`
- Test: in `status.rs`, using `crates/core/tests/fixtures/claude/2.1.302/*.json` (load with `include_str!`)

**Interfaces:**
- Produces:
  - `pub fn fold_event(prev: Option<&LiveStatus>, body: &serde_json::Value, now_ms: u64) -> Option<LiveStatus>`: `None` for an event with no status meaning (InstructionsLoaded, MessageDisplay, unknown names).
  - `pub fn fold_statusline(prev: Option<&LiveStatus>, body: &serde_json::Value, now_ms: u64) -> LiveStatus`: fills model, effort, context_pct, five_hour_pct, seven_day_pct, and leaves state, phrase and recap as they were (a first status line gives `Idle` with an empty phrase).
  - `pub fn ring_level(pct: f32) -> RingLevel { Green, Yellow, Red }` (70 and 90 thresholds).

Mapping, keyed on `hook_event_name`:
- SessionStart → Idle, phrase "".
- UserPromptSubmit → Working, "Thinking".
- PreToolUse → Working, "Running `<command>`" for Bash (first line, 60 chars), "Editing `<file_path>`" for Edit/Write/MultiEdit, "Reading `<file_path>`" for Read, "Using `<tool_name>`" otherwise. A tool input missing its field falls back to "Using `<tool_name>`".
- PostToolUse, PostToolBatch → Working, phrase kept.
- PermissionRequest → NeedsPermission, "Wants to run `<command>`" (Bash) or "Wants to use `<tool_name>`".
- Notification with `notification_type` `idle_prompt` → Idle; `permission_prompt` → NeedsPermission, keeping the phrase; anything else keeps the state.
- Stop → Done, phrase "Done", recap `{ text: last_assistant_message, at_ms: now }` when that is non-empty (first line, 200 chars).
- SessionEnd → Ended, phrase "Ended".
- `since_ms` changes only when the state changes.

- [ ] **Step 1: Write the failing tests.** `fold_event` on each fixture gives the state and phrase above (PreToolUse fixture → `Working` and a phrase starting "Running `"; Stop fixture → `Done` and `recap.is_some()`; Notification fixture → `Idle`; InstructionsLoaded → `None`). Further tests: `since_ms` is kept when PostToolUse follows PreToolUse; a PreToolUse with no `tool_input` gives "Using `Bash`"; a Stop with an empty message has no recap and keeps the previous recap; `fold_statusline` on the fixture gives `context_pct == Some(6.0)` and the fixture's rate limits; a body with `rate_limits` absent gives `None`; `used_percentage` of 140 clamps to 100 and of `null` gives `None`; `ring_level(69.9/70/89.9/90)` is Green/Yellow/Yellow/Red.
- [ ] **Step 2:** Run `cargo test -p marshell-core status::` and see them fail.
- [ ] **Step 3:** Implement the functions above, with the mapping as a `match` on `hook_event_name`.
- [ ] **Step 4:** Run `cargo test -p marshell-core status::`. Expected: pass.
- [ ] **Step 5:** Commit `feat(agents): fold Claude Code hook events and the status line into a live status`.

### Task 3: Version gate and the off switch

**Files:**
- Modify: `crates/core/src/agents/claude/mod.rs` (next to `parse_version`), `crates/core/src/settings.rs` (FileDoc, load, save carry `status_off`)
- Test: both files' test modules

**Interfaces:**
- Produces:
  - `pub const FIXTURE_VERSIONS: &[(u32, u32, u32)] = &[(2, 1, 302)];`
  - `pub fn version_gate(version: Option<&str>) -> Gate` with `enum Gate { Known, Unknown(String) }`; the string is the status note.
  - `settings::load`/`save` round-trip `status_off`, and `repair` drops duplicates and empty strings.

Rule: known when major.minor equals a fixture's and patch ≥ the oldest patch on that line. `None`, a non-numeric version, 2.1.301, 2.2.0, 3.0.0 and 2.0.9 are `Unknown`. Notes: "Claude Code <v> is not one Marshell has recorded yet" for a newer or other line, "Claude Code <v> is older than the versions Marshell knows" for older, "Claude Code's version could not be read" for `None`.

- [ ] **Step 1:** Write the failing tests: 2.1.302, 2.1.303, 2.1.999 known; 2.1.301, 2.0.9, 2.2.0, 3.0.0, "garbage", `None` unknown with the notes above. A settings file with `status_off: ["claude"]` loads and saves back, and a file without the field loads as empty.
- [ ] **Step 2:** Run both test modules and see them fail.
- [ ] **Step 3:** Implement. `version_gate` parses with the existing `parse_version`.
- [ ] **Step 4:** Run `cargo test -p marshell-core`. Expected: pass.
- [ ] **Step 5:** Commit `feat(agents): gate live status on the Claude Code version and add a per-agent off switch`.

### Task 4: Registry and hook wiring

**Files:**
- Modify: `crates/core/src/registry.rs`, `crates/core/src/server/hook.rs`, `crates/core/src/server/mod.rs` (the `forget_tab` call at ~246 and the exit path)
- Test: `registry.rs` tests, a route test next to the existing hook route tests

**Interfaces:**
- Consumes: Task 1 types, Task 2 `fold_event`/`fold_statusline`, Task 3 `version_gate`.
- Produces:
  - `Registry::update_live(&self, tab_id: &str, f: impl FnOnce(Option<&LiveStatus>) -> Option<LiveStatus>)`: applies only to a Running row; broadcasts only when the value changed.
  - `Registry::set_status_note(&self, tab_id: &str, note: Option<String>)`: broadcasts only on a change.
  - `Registry::clear_live(&self, tab_id: &str)`, called when a session exits, is restarted or closed.
  - In `hook::post`, after the log push: for `agent == "claude"` with a tab, if the gate is Known and `claude` is not in `status_off`, fold an Event role body with `fold_event` and a Statusline role body with `fold_statusline` into `update_live`; otherwise set the status note (version note, or "Live status is off for Claude Code") and leave `live` empty. The version comes from the same cached `run_version` that the setup route uses.

- [ ] **Step 1: Write the failing tests.** Registry: `update_live` on an unknown tab does nothing and sends no frame; an unchanged value sends no frame; `mark_exited` clears `live`. Route (post through the router with the 2.1.302 fixtures and a known version): SessionStart then UserPromptSubmit gives `live.state == working` in `registry.snapshot()`; Stop gives `done` with a recap; with `status_off` containing `claude` or a gated version, the same posts leave `live == None` and set `status_note`; a post with an unknown tab changes nothing; Permission-role posts still hold as before (the existing permission tests keep passing).
- [ ] **Step 2:** Run `cargo test -p marshell-core registry:: hook` and see them fail.
- [ ] **Step 3:** Implement. Keep `hold_permission` untouched; folding happens before it.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: pass.
- [ ] **Step 5:** Commit `feat(core): keep a live status on each Claude Code session and broadcast it`.

### Task 5: pty fallback

**Files:**
- Modify: `crates/core/src/registry.rs` (`set_activity`), `crates/core/src/server/sessions.rs` (the output notify path) or a small watcher beside `server/foreground.rs`
- Test: registry tests; a watcher test with a fake clock if the watcher is time-based

**Interfaces:**
- Produces: `Registry::set_activity(&self, tab_id: &str, generation: u64, activity: Option<Activity>)`: only for a Running agent row with `live == None`; broadcasts on a change. Output within the last 2 s gives `Working`; quiet for longer gives `Idle`; exit clears it (the row's Ended state speaks).

- [ ] **Step 1:** Write the failing tests: `set_activity` is refused for a row with `live`, for a shell row (no agent), and for an old generation; a row that exits has `activity == None`.
- [ ] **Step 2:** Run and see them fail.
- [ ] **Step 3:** Implement `set_activity` and drive it from pty output counters (`counters()` at `pty/session.rs:493`) with a once-a-second sweep over the rows that have no `live`, the way the foreground watcher sweeps. Output timestamps come from the counters; no new reader thread.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: pass.
- [ ] **Step 5:** Commit `feat(core): fall back to output activity for an agent row without live status`.

### Task 6: Frontend rows, rings and recap

**Files:**
- Modify: `src/features/sessions/toRow.ts`, `src/features/sidebar/types.ts` (only if a field is missing), the sidebar footer component that holds the plan ring (grep for "plan ring"/footer in `src/features/sidebar`), `SessionRow.tsx` for the recap age, `src/features/agents/AgentsDialog.tsx` and `agentsState.ts` for the "Live status" switch, `src/lib/api` for saving `status_off`
- Test: `toRow.test.ts`, the footer and row tests (`renderToStaticMarkup`), `agentsState` tests

**Interfaces:**
- Consumes: `info.live`, `info.activity`, `info.status_note`, `Settings.status_off`.
- Produces: `toRow` maps `live.state` → `GlyphKind` (idle→idle, working→working, needs_permission→needs-permission, needs_question→needs-question, done→done-unseen until the row has been seen then done-seen (reuse the existing seen mechanism if present, else done-seen), error→error, ended→ended), `phrase`, `model`, `effort`, `subagents` (omitted when 0), `contextPct`, `recap { text, ageMs }`, `waitingMs` for needs-you and `ageMs` for done. Without `live`: `activity` working → `working`/"Working", idle → `idle`/"Idle", both with the "Status limited" note on the tooltip; no activity → the existing limited row, now `phrase: "Status limited · set up"` when hooks are missing, else `info.status_note` when set. `planRing(rows | infos): { pct, level } | undefined` takes the highest `five_hour_pct` across sessions that report one and returns `undefined` when none do.

- [ ] **Step 1: Write the failing tests.** One `toRow` case per state; `contextPct` of 69/70/90 maps to the ring level the existing `ContextRing` uses (assert the rendered markup); no `context_pct` renders no ring; limited-with-note and limited-with-setup phrases; the activity fallback; `planRing` with two sessions and with none; the footer markup shows the ring and "5h N%" or nothing; the Agents dialog shows a "Live status" switch that saves `status_off` without `claude` when on and with it when off.
- [ ] **Step 2:** Run `pnpm test src/features` and see them fail.
- [ ] **Step 3:** Implement the mappings and the switch. Use `frontend-design`'s existing tokens; no new colours.
- [ ] **Step 4:** Run `pnpm test`, `pnpm build`. Expected: pass.
- [ ] **Step 5:** Commit `feat(ui): show live status, the context and plan rings and the recap`.

### Task 7: Live check and docs

**Files:**
- Create: `docs/research/2026-10-19-claude-live-status-live-check.md`

- [ ] **Step 1:** Per CLAUDE.md "Live check (Windows)": scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS` on a scratch project's `settings.local.json`, hooks installed through the consent card, a Claude session started for the check.
- [ ] **Step 2:** Observe over CDP, as small JSON, that the row goes Working while Claude answers "Reply with only the word ok." and Done after, that the recap shows with its age, that the context ring appears from the status line, and that the footer plan ring shows (or is absent, if the account sends no rate limits; record which).
- [ ] **Step 3:** Switch "Live status" off in the Agents dialog: the row reads limited with the note, and the activity fallback shows Working then Idle.
- [ ] **Step 4:** Record the results, the Claude version, and any Known limits in the research doc. Stop what was started by PID and remove the scratch folders. Commit `docs(research): live check of live status`.

### Task 8: Whole-branch review and PR

- [ ] **Step 1:** Opus whole-branch review (explicit `model`), then one fix pass, each fix RED→GREEN.
- [ ] **Step 2:** Run what CI runs: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm test`, `pnpm check:generated`, `pnpm build`, `pnpm tauri build --no-bundle`. Quote the results in the PR body.
- [ ] **Step 3:** Push, open the PR ("What it does", "Live check", "Known limits", "Checks"), wait for green CI, merge.
