# Slice 7a: the Doctor's engine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A live Doctor. It checks what Marshell's code already detects for all four CLIs, on Windows and in running WSL distros. Its report has three groups with checkboxes. "Fix X issues" goes through a preview, one snapshot and one-click undo, and that undo survives a restart.

**Architecture:**
- **Checks.** They are a pure function, `doctor::checks::items(&[AgentFacts], stopped distros) -> Vec<DoctorItem>`, in a new `crates/core/src/doctor/` module.
- **Facts.** The server gathers them in a new `crates/core/src/server/doctor.rs`, reusing the Agents dialog's own readers (`setup_of`, `codex_setup_of`, `agy_setup_of`, `qwen_setup_of`), `managed.json` and each adapter's `version_gate`.
- **Fixes.** A fix reuses each CLI's own hook setup plan and one `JsonSet` for retention. They are merged into one `ChangeSet`, previewed, and applied through the config service. A new per-pointer "own" flag keeps `managed.json` from claiming the person's `cleanupPeriodDays`.
- **Notifying the UI.** The `/v1/events` socket gains a `doctor` frame with a summary, which feeds the sidebar count and the tray.
- **Frontend.** The phase-0 Doctor page is reworked to the generated types, the groups and checkboxes, and is mounted in the live app.

No new dependencies.

**Tech Stack:** Rust (axum core, the config service, ts-rs), React + TypeScript (Vitest in node, no DOM).

**Spec:** `docs/PLAN.md` §9, "Slice 7a decisions (2026-11-05)". The bindings are:
- BRIEF §5.4 (groups, checkboxes, fix rules);
- PLAN "Key flows → Doctor fix and undo" (the toast and "Undo last fix");
- PLAN "Batch 3 design decisions" (fix sheet, focus, config safety copy);
- PLAN §6 (preview, snapshot, undo hash rule).

The phase-0 mock is `src/features/doctor/`; its copy stands unless this plan changes it. Facts were read on main at `000936b`, and line numbers may drift a few lines.

## Global Constraints

- **Commits and staging.**
  - Commit messages read `type(scope): what changed, as a sentence`. Run `cargo fmt --all` before every Rust commit.
  - Every cargo command uses `-j 3`, for example `cargo test -j 3 -p marshell-core doctor`. The default job count runs rustc out of memory here, and a frontend worker may run beside you.
  - Stage only your own files, by path. Never `git add -A` or `git commit -a`. On `index.lock`, wait and retry.
  - Run only your own tests: the crate's filtered `cargo test` and `pnpm test <file>`. The lead runs the full suites.
- **Worktree.** Work in the `phase-7a` worktree, with plain commands from its root. A compound command that changes directory can be refused.
- **Protocol.**
  - New types go in `crates/protocol/src/doctor.rs` with `#[derive(TS)] #[ts(export)]`. Every `u64` gets `#[ts(type = "number")]`.
  - Optional fields are `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]`.
  - Run `cargo test -j 3 -p marshell-protocol` and `pnpm check:generated`, and commit `src/generated/`.
- **Async rules.** Never hold a `MutexGuard` across `.await` or I/O. File reads, `managed.json` reads, `wsl.exe` calls and `\\wsl.localhost` reads run in `spawn_blocking`.
- **Local API.** Every new route sits in the token-guarded router and has a "refused without the bearer" test. Don't touch `crates/core/src/server/guard.rs` or token handling. No route takes a path to write.
- **Paths.** Every check reads through the existing resolvers on `AppState`, so `MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME`, `QWEN_HOME` and the test pins hold. Never join `~` yourself. The resolvers are `claude_settings_path`, `codex_hooks_path`, `agy_user_home`, `qwen_settings_path` and `paths::*`.
- **Never in a test or worker:**
  - Tests never run `wsl.exe` and never read the real home. Running distros and distro targets are pinned on `AppState`, as `with_distro_target` is.
  - A fake home lives under the repo's `target` folder: `tempfile::tempdir_in(projects::canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")))`.
  - Never run `pnpm tauri dev` or a real CLI from a worker.
- **Fix rules (BRIEF §5.4).**
  - Disable, never delete. 7a's fixes are only Marshell's own hook setup plans and one retention `JsonSet`. No fix removes an entry of the person's.
  - Managed (organisation) files are reported and never written.
  - Every write goes preview → diff → apply.
- **Copy.** UI copy is sentence case, subject first, with no "!" and no "successfully". This plan's copy is exact. Don't guess a status value.
- **Frontend.**
  - Before styling, read the `frontend-design` and `good-css` skills.
  - Every control works with the keyboard alone and in forced colours. The window's Announcer is the only live region.
  - Tests use Vitest in node with `renderToStaticMarkup`. `dialogBodies.test.ts`, `forcedColorsEdges.test.ts` and `noBoxedOutlines.test.ts` stay green.

## Review Focus

1. **A file edited between the fix sheet and Allow.** Apply answers 409 "changed" and writes nothing. No last-fix record appears. The sheet offers Reload and never an overwrite. Task 6 tests this.
2. **Undo after the person edited a fixed file, and undo after a restart.** If a file was edited, undo refuses, names the file and writes nothing, and the header offers "Show differences". If all files are unchanged since the fix, an Undo after an app restart works from the persisted record. Task 6 tests this.
3. **A hooks fix and the retention fix in the same `settings.json`.** They make one file preview and one snapshot. `managed.json` records the hook pointers but not `/cleanupPeriodDays`, so a later Hooks off leaves `cleanupPeriodDays` alone. Tasks 2 and 6 test this.
4. **A managed distro that is stopped.** It is never started: no `wsl.exe` call names it and no share path under it is read. It is reported "Not checked". A running distro whose file can't be read is reported per item, and the rest of the report stands. Task 5 tests this.
5. **One CLI's file that doesn't parse, is read-only, or whose reader errors.**
   - That CLI gets the item that says why.
   - No fix is offered for that file and nothing is counted for it.
   - The other CLIs' items are unaffected, and the run as a whole never fails.

   Tasks 3 and 4 test this.

---

## Lane R: core (one Rust task at a time, in order)

### Task 1: Protocol types for the Doctor

**Files:**
- Create: `crates/protocol/src/doctor.rs`; Modify: `crates/protocol/src/lib.rs` (the `mod`/`pub use` line, as the other modules)
- Generated: `src/generated/Doctor*.ts`

**Interfaces:**
- Produces, all `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)] #[ts(export)]`, with snake_case fields as in `agents.rs`:
  - `enum DoctorGroup { Broken, Waste, Info }`, `#[serde(rename_all = "camelCase")]`.
  - `struct DoctorItem`, with these fields:
    - `id: String`;
    - `agent: Option<String>` (None: Marshell itself);
    - `distro: Option<String>`;
    - `group: DoctorGroup`;
    - `title: String`;
    - `detail: String`;
    - `fixable: bool`;
    - `cannot_fix: Option<String>`: why it can't be fixed, or what the person does instead;
    - `path: Option<String>`;
    - `open_agents: bool`: the row offers "Open Agents".
  - `struct DoctorReport { items: Vec<DoctorItem>, checked_at_ms: u64, duration_ms: u64 }`.
  - `struct DoctorSummary { running: bool, issues: u32, checked_at_ms: Option<u64> }`. `issues` counts the Broken and Waste items.
  - `struct DoctorFixRequest { item_ids: Vec<String> }`.
  - `struct DoctorFixPlan { preview: crate::config::ConfigPreview, item_ids: Vec<String>, restart: Vec<u32> }`. `restart` is the session numbers that pick the fix up only after a restart.
  - `struct DoctorLastFix`, with these fields:
    - `snapshot_id: String`;
    - `fixed_at_ms: u64`;
    - `item_ids: Vec<String>`;
    - `count: u32`;
    - `restart: Vec<u32>`;
    - `files_match: bool`.

- [ ] **Step 1: Write the failing test** in `doctor.rs`. A `DoctorItem` with `agent: None, cannot_fix: None` serialises without those keys. `DoctorGroup::Broken` serialises as `"broken"`. A `DoctorSummary` round-trips.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-protocol doctor`. Expected: it fails to compile, because the module doesn't exist.
- [ ] **Step 3: Implement** the types.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-protocol` (this regenerates `src/generated/`), then `pnpm check:generated`. Expected: both pass, and there is no `bigint`.
- [ ] **Step 5: Commit.** `feat(protocol): types for the doctor's report, its fix plan and the last fix`

### Task 2: The config service logs why it refused an apply, and a preview can mark some JSON keys as the person's own (#94)

**Files:**
- Modify: `crates/core/src/config/service.rs` (`apply_with` ~:425-475, managed recording ~:517-541, `store_preview` ~:287, the stored preview's `own` ~:85)

**Interfaces:**
- Produces: `ConfigService::preview_mixed(&self, cs: ChangeSet, own: &dyn Fn(&FileEdit) -> bool) -> ConfigPreview`. An edit for which `own` is true is the person's: `managed.json` records none of its JSON keys. The other edits are Marshell's, as with `preview`. `preview` and `preview_own` keep their behaviour.
- Produces: `fn changed_check(...)`, private, which returns which "changed" branch refused: the file list or targets differ, or a file is now refused or its hash moved.

Evidence:
- `apply_with` has two `"changed"` branches. At `:458-460`, the file count or targets differ from the stored ones. At `:461-473`, a file is now `Refused`, or its before or after hash moved.
- Neither branch logs anything, which is #94's blind spot.
- `tracing::warn!` is already used in this file (`:356`).
- The stored preview's `own: bool` (`:85`) is checked per JSON op at `:524`: `EditOp::JsonSet { .. } | EditOp::JsonRemove { .. } if own => continue`. So one preview is all Marshell's or all the person's.

- [ ] **Step 1: Write the failing tests.**
  - `a_mixed_preview_records_only_marshells_keys`: one ChangeSet sets `/hooks/Stop` and `/cleanupPeriodDays` in one file, with `own` true for the second. After apply, `managed::list` has an entry for `/hooks/Stop` and none for `/cleanupPeriodDays`. There is one snapshot.
  - `changed_check_names_the_branch`: the target-list branch and the hash branch each return their own variant, and an unchanged file returns none.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core config::service`. Expected: it fails to compile.
- [ ] **Step 3: Implement.**
  - Store the own flag per edit (for example `own: Vec<bool>` aligned with `cs.edits`, or a set of `(path, pointer)`), and check it per op at the managed-recording step.
  - Move the two branch conditions into `changed_check` without changing what they test.
  - Before each `"changed"` return, log `tracing::warn!` with the branch. For the target branch, also log the stored targets and the new ones (`display()`). For the hash branch, log each refused or moved path and which of before, after or status moved.
  - The response doesn't change.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core config`. Expected: all pass, the existing apply tests included.
- [ ] **Step 5: Commit.** `fix(config): an apply refused as changed logs which check refused it, and a preview can mark keys as the person's own`

### Task 3: The checks

**Files:**
- Create: `crates/core/src/doctor/mod.rs`, `crates/core/src/doctor/checks.rs`. Modify: `crates/core/src/lib.rs` (`pub mod doctor;`).

**Interfaces:**
- Consumes: Task 1's types; `marshell_protocol::agents::{HookInstall, HookTrust}`.
- Produces, in `doctor::checks`:
  ```rust
  pub enum FileState { Ok, Missing, ReadOnly, Unparsable { line: u32, message: String } }
  pub struct AgentFacts {
      pub agent: &'static str,              // "claude" | "codex" | "antigravity" | "qwen"
      pub distro: Option<String>,
      pub found: bool,                      // the CLI is on this machine (always true for a distro's facts)
      pub path: PathBuf,                    // the file its hooks live in
      pub file: FileState,
      pub hooks: HookInstall,
      pub approve_installed: bool,
      pub has_own_statusline: bool,
      pub chained: bool,
      pub trust: Option<HookTrust>,         // Codex only
      pub set_up: bool,                     // managed.json names hooks Marshell wrote in `path`
      pub bridge: PathBuf,                  // the bridge program the hooks run
      pub version_note: Option<String>,     // Gate::Unknown's note; None when known, not found, or a distro
      pub retention_unset: Option<bool>,    // Some(true): the key is unset; Some(false): the person set it; None: no such setting or not read
      pub managed_settings: Option<PathBuf>,// an organisation file applies on top (Claude, Qwen)
      pub error: Option<String>,            // the reader failed; every other field is a default
  }
  pub fn items(facts: &[AgentFacts], stopped: &[String]) -> Vec<DoctorItem>
  pub fn display_name(agent: &str) -> &'static str   // reuse the existing display name if one exists (grep "Claude Code" in crates/core/src/agents)
  ```
- Produces item ids, which Task 6 parses:
  - `hooks.<agent>`, `trust.codex`, `parse.<agent>`, `setup.<agent>`, `version.<agent>`, `managed.<agent>`, `retention.<agent>`, `error.<agent>`;
  - `wsl.<distro>`;
  - a distro's items add `@<distro>`, for example `hooks.codex@Ubuntu`.

The items, in this order per CLI (Windows CLIs in the order claude, codex, antigravity, qwen, then each distro by name, then the stopped distros). `<CLI>` is `display_name`, `<path>` is `path`.

| When | id | group | fixable | title | detail | cannot_fix |
| --- | --- | --- | --- | --- | --- | --- |
| `error` is Some | `error.<agent>` | Info | no | `<CLI> couldn't be checked` | the error text | – |
| `file` is Unparsable | `parse.<agent>` | Broken | no | `<path> doesn't parse` | `Line <line>: <message>.` | `Marshell doesn't change a file it can't read. Fix it by hand, then run the check again.` |
| `hooks` is None and `set_up` | `hooks.<agent>` | Broken | yes | `<CLI>'s hooks are gone` | `Marshell set them up in <path>, and they aren't there now. The fix sets them up again.` | – |
| `hooks` is Partial | `hooks.<agent>` | Broken | yes | `<CLI>'s hooks are incomplete` | `Some of Marshell's entries in <path> are missing or changed.` then, when `has_own_statusline && !chained` and the CLI chains, ` Your status line isn't chained through Marshell any more.` then ` The fix sets them up again.` | – |
| `hooks` is Orphaned | `hooks.<agent>` | Broken | yes | `<CLI>'s hooks run a program that's missing` | `The hooks in <path> run <bridge>, which isn't there. The fix puts it back and sets the hooks up again.` | – |
| Codex, `hooks` isn't None, `trust` is Untrusted | `trust.codex` | Broken | no | `Codex doesn't trust Marshell's hooks yet` | `Codex runs a hook only after you trust it.` | `Marshell can't trust hooks for you. Open /hooks in Codex and trust them.` |
| Codex, `trust` is Changed | `trust.codex` | Broken | no | `Codex's trust in Marshell's hooks is out of date` | `The hooks changed since you trusted them, so Codex doesn't run them.` | as above |
| `version_note` is Some | `version.<agent>` | Info | no | the note | `Status limited: rows show less for <CLI> until Marshell has recorded this version.` | – |
| `managed_settings` is Some | `managed.<agent>` | Info | no | `Managed settings apply on top of yours` | `<managed path> is set by your organisation. Marshell reports it and never changes it.` | – |
| `retention_unset == Some(true)` | `retention.<agent>` | Info | yes | per Task 3's retention copy below | per Task 3's retention copy below | – |
| `found`, Windows, `hooks` is None, not `set_up` | `setup.<agent>` | Info | no | `Hooks aren't set up for <CLI>.` | `Agents sets them up, with a preview first.` | – (`open_agents: true`) |
| a stopped distro | `wsl.<distro>` | Info | no | `Not checked: <distro> isn't running.` | `Start it, then run the check again.` | – |

- A distro item's title says where: `<CLI>'s hooks in <distro> are gone`, and likewise for the others.
- When `file` is ReadOnly, every fixable item of that CLI that writes `path` becomes `fixable: false`, with `cannot_fix: "The file is read-only."`. That is the config planner's copy, `preview.rs:202`.
- When `file` is Unparsable, that CLI gets only its `parse` item, plus `version` and `managed`.
- Retention copy:
  - Only Claude Code has a retention item (decision 6 as amended, `docs/research/2026-11-05-transcript-retention.md`). Its title is `Claude Code deletes transcripts after 30 days`. Its detail is `History and resume lose sessions older than that. The fix sets cleanupPeriodDays to 365. ` followed by `RETENTION_OTHERS`.
  - `const RETENTION_OTHERS: &str = "Codex and Qwen Code have no setting that deletes chats by age, and Antigravity documents none.";`
  - A `retention_unset` of `Some(true)` on any other agent makes no item: the check is Claude Code's only. A test pins this.

- [ ] **Step 1: Write the failing tests** in `checks.rs`:
  - One test per table row, asserting the id, group, `fixable`, and exact title, detail and `cannot_fix`.
  - `read_only_turns_off_the_fix_and_says_why`.
  - `an_unparsable_file_has_only_its_parse_item_and_the_notes`.
  - `a_reader_error_is_one_info_item_and_the_other_clis_are_untouched`.
  - `order_is_claude_codex_antigravity_qwen_then_distros_then_stopped`.
  - `hooks_never_set_up_is_info_and_not_counted`: a summary helper `issues(&items)` counts Broken and Waste only.
  - `a_distro_title_names_the_distro`.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core doctor::checks`. Expected: it fails to compile.
- [ ] **Step 3: Implement** `items` and `pub fn issues(items: &[DoctorItem]) -> u32` as the table says.
- [ ] **Step 4: Run** the same command. Expected: all pass.
- [ ] **Step 5: Commit.** `feat(doctor): the checks for hooks, Codex trust, files that don't parse, versions, managed settings and retention`

### Task 4: Running the checks: facts, routes, the startup run and the events frame

**Files:**
- Create: `crates/core/src/server/doctor.rs`, `crates/core/src/doctor/state.rs`
- Modify: `crates/core/src/server/mod.rs` (`Inner` gets a `doctor` field and a `doctor_on_start: bool` pin, default false; routes in the chain at ~:1058-1184; `router()` spawns the startup run beside the watchers at ~:1046-1049); `crates/core/src/server/events.rs` (the `doctor` frame); the app's startup code that builds `AppState` (grep the callers of `AppState` construction in `src-tauri/` and `crates/core/src/`) sets `doctor_on_start(true)`
- Modify: `docs/PLAN.md` §3's Doctor row (~:526): the routes below
- Test: `crates/core/tests/doctor_api.rs` (copy the harness pattern of `crates/core/tests/agents_api.rs`)

**Interfaces:**
- Consumes: Task 3's `AgentFacts`, `items` and `issues`; `setup_of`, `codex_setup_of`, `agy_setup_of` and `qwen_setup_of` (`server/agents.rs:665-966`, `pub(crate)`); `managed::list` (`config/managed.rs:37`); each adapter's `version_gate(version: Option<&str>) -> Gate` (`agents/<cli>/mod.rs`); and `settings_view::read_managed(paths)` (`agents/settings_view.rs:467`), with the paths its server caller passes (grep `read_managed(` in `server/agents.rs`).
- Produces: `doctor::state::DoctorState`, which holds:
  - the last report: `Mutex<Option<DoctorReport>>`;
  - a running flag;
  - the summary: `watch::Sender<DoctorSummary>`;
  - the pending fix: `Mutex<Option<PendingFix { changeset_id, item_ids, restart }>>`.
- Produces: `pub(crate) fn request_run(st: &AppState)`. It starts a run unless one is running, in which case the request joins that run. Tasks 5 and 6 call it.
- Produces the routes:
  - `POST /v1/doctor/run`: 202 with the `DoctorSummary`.
  - `GET /v1/doctor/report`: 200 with the `DoctorReport`, or 204 before the first run ends.
  - Task 6 adds the `fix` and `undo` routes.
- Produces the events frame `{"type":"doctor","data":DoctorSummary}`. It is sent on connect, after `sessions` and `permissions`, and then on each change.

Evidence:
- `setup_of` already reads the file state (`settings_state`: `Ok`, `Missing`, `Unparsable` with `parse_error`, or `ReadOnly`), `hooks`, `approve_installed`, `has_own_statusline`, `chained`, `trust`, `version`, `found` and `bridge_path` (`agents.rs:665-737`).
- `ManagedEntry { path, pointer, changeset_id, applied_ms }` is the entry type (`managed.rs:9-16`).
- Own edits (Settings) record no keys (`service.rs:524`), so an entry in a hooks file is Marshell's.

- [ ] **Step 1: Write the failing tests.**
  - Inline in `server/doctor.rs`, `set_up_of(entries, path)` is true for an entry whose path is `path`, compared after `config::guard::unverbatim` and case-insensitively on Windows. The pointer must start with `/hooks`, or be None (a file Marshell created). It is false for a `/statusLine` entry alone and for another file.
  - `doctor_api.rs`, `the_report_finds_claudes_hooks_gone`:
    - set Claude's hooks up through `/v1/agents/claude/hooks/plan` and `/v1/config/apply`;
    - write a settings file without them;
    - `POST /v1/doctor/run`, then poll `GET /v1/doctor/report` until it answers 200.
    - It has `hooks.claude` Broken and fixable.
  - `a_cli_never_set_up_gets_the_setup_info_line`: with a found Codex pinned the way `agents_api.rs` pins it.
  - `one_cli_failing_to_read_leaves_the_rest`: a reader error becomes `error.<agent>`.
  - `the_events_socket_sends_the_doctor_summary`.
  - `doctor_routes_refuse_without_the_bearer`.
  - `no_run_at_start_unless_pinned_on`: with `doctor_on_start` false, a fresh router answers 204 to the report.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core --test doctor_api` and `cargo test -j 3 -p marshell-core server::doctor`. Expected: they fail to compile.
- [ ] **Step 3: Implement.**
  - **Gathering.** `facts_of(st) -> Vec<AgentFacts>` runs in `spawn_blocking`. It calls each `*_setup_of`; an `Err` gives `error: Some(text)`. It reads `managed.json` once.
  - **Version.** `version_note` is `Gate::Unknown(note)` from the adapter's `version_gate(setup.version.as_deref())` when `found`.
  - **Retention.**
    - Claude Code: `Some(!has_key)` for `cleanupPeriodDays` at the top level of the settings JSON.
    - Qwen Code, Codex and Antigravity: `None`. Qwen's `general.cleanupPeriodDays` doesn't delete chats. So is a CLI that isn't found, or whose file isn't `Ok` and isn't `ReadOnly`.
  - **Managed settings.** Fill `managed_settings` from `read_managed` for Claude Code and Qwen Code.
  - **Running.** A run stores the report and sends the summary, with `running` true while it runs. It is the only writer of the report.
  - **Startup.** The startup run is spawned only when `doctor_on_start` is true.
  - **PLAN.** Update §3's Doctor row to the six routes of this plan, in place of the four.
- [ ] **Step 4: Run** the same commands. Expected: all pass.
- [ ] **Step 5: Commit.** `feat(doctor): the core runs the checks at startup and on request and reports them over the API and the events socket`

### Task 5: Checks inside running WSL distros

**Files:**
- Modify: `crates/core/src/wsl.rs` (add `list_running`); `crates/core/src/server/agents.rs` (extract `record_distros_of`'s "path is in this distro" test, `in_share` ~:301-303, into a `pub(crate) fn distros_of_paths(&self, paths) -> Vec<String>` that `record_distros_of` and the doctor both use); `crates/core/src/server/doctor.rs`; `crates/core/src/server/mod.rs` (a `with_running_distros(list: fn() -> Vec<String>)` pin, in the style of `with_tab_path`); possibly `crates/core/src/agents/<cli>/hooks.rs` (see step 3)
- Test: `crates/core/tests/doctor_api.rs`

**Interfaces:**
- Produces: `pub fn list_running(timeout: Duration) -> Vec<String>` = `parse_distro_list(&run_wsl(&["-l", "--running", "-q"], timeout))`. It blocks.
- Produces: the managed distros. They are the detected WSL distros (`super::wsl_distros(&profiles_snapshot())`) that hold a `managed.json` entry path, judged as `record_distros_of` judges (`under_distro`, or a test's pinned home).
- Produces: the running managed distros. They are the managed distros whose name is in `list_running` (case-insensitive), so a localised "no running distributions" sentence never reads as a name.
- Produces: `facts_of` also returns each running managed distro's `AgentFacts`. `stopped` is the managed distros that aren't running.

Evidence:
- `AppState::distro_target` runs `wsl.exe -d <distro>`, which starts a stopped distro (`agents.rs:317-346`). Reading `\\wsl.localhost\<distro>\...` may start it as well.
- A test pins one target for every distro with `with_distro_target(Some(DistroTarget))` (`mod.rs:531`), whose plain-folder home stands in for every distro's share.
- `managed.json` has no distro field; `record_distros_of` (`agents.rs:290-314`) already finds a path's distro.
- A distro's files are `<home>/.claude/settings.json`, `<home>/.codex/hooks.json`, `agy::hooks_path(home)` with `agy::settings_path(home)`, and `<home>/.qwen/settings.json` (`agents/distro.rs:31-36`). There is no chain file in a distro.
- The bridge there is `Bridge::Distro { name, path }`.

- [ ] **Step 1: Write the failing tests** (`doctor_api.rs`). Each sets up a detected WSL profile for Ubuntu and a pinned distro target, as `agents_api.rs` does, and installs Claude's hooks in Ubuntu through `/v1/agents/claude/hooks/plan` with `distro: "Ubuntu"`.
  - `a_stopped_managed_distro_is_not_checked`: break the pinned home's Claude settings, and pin the running list to empty. The result has `wsl.Ubuntu` with its exact copy and no other Ubuntu item. Had the file been read, `hooks.claude@Ubuntu` would be there.
  - `a_running_distro_with_missing_hooks_is_broken`: running is `["Ubuntu"]`. The result is `hooks.claude@Ubuntu`, Broken and fixable, and its title is `Claude Code's hooks in Ubuntu are gone`.
  - `an_intact_distro_setup_reads_as_installed`: just after the install, there is no Ubuntu item. This proves the distro's bridge path is judged right.
  - `a_running_distro_whose_file_doesnt_parse_reports_that_cli_only`.
  - `no_wsl_call_without_a_managed_distro`: the running pin counts its calls in a static `AtomicUsize`. With no distro in `managed.json`, the count is 0 after a run.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core --test doctor_api`. Expected: the new tests fail.
- [ ] **Step 3: Implement.**
  - **Running list.** Consult the running list only when there is a managed distro, with an 8 s timeout. Without a pin, call `wsl::list_running`.
  - **Inspect.** For a running one, get its target with `distro_target`, which doesn't start a running distro. Read each CLI's file and run that CLI's `inspect`. A read or parse failure becomes that CLI's item, as on Windows.
  - **Exact-command check.** If `inspect` judges the entries against the local bridge (`paths::bridge_exe(home)`), a distro's entries would read as Partial or Orphaned. In that case:
    - add an `inspect_for(…, bridge: &Bridge)` beside each `inspect`;
    - keep `inspect` as the `Bridge::local` wrapper;
    - judge Orphaned in a distro by the share path of the distro's bridge;
    - test both.
  - **Trust.** A distro's Codex trust comes from `codex::trust::state(&path, &hooks, config_toml)` with the distro's `.codex/config.toml`.
  - **Not covered in a distro:** version, retention, managed settings and the setup line.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core`. Expected: all pass.
- [ ] **Step 5: Commit.** `feat(doctor): the doctor checks the hooks in each WSL distro Marshell set up while it runs, and never starts one`

### Task 6: Fix and undo

**Files:**
- Modify: `crates/core/src/server/doctor.rs`, `crates/core/src/server/agents.rs` (extract the per-CLI install-plan call of `plan` ~:1564-1641 into `pub(crate) fn hook_install_changes(st: &AppState, adapter: Adapter, want: &HookPlanRequest) -> Result<ChangeSet, Fail>`, which `plan` and the doctor both use; make `distro_install` `pub(crate)`), `crates/core/src/paths.rs` (`pub fn doctor_last_fix(home: &Path) -> PathBuf` → `<home>/doctor/last-fix.json`), `crates/core/src/server/mod.rs` (routes)
- Test: `crates/core/tests/doctor_api.rs`

**Interfaces:**
- Consumes: Task 2's `preview_mixed`; Task 4's `DoctorState` and `request_run`; `store::write` (`store.rs:35`, a `Versioned` value, as `layout.rs` saves) and `store::read` for the record; `Snapshots::get` (`snapshots.rs:233`); and the `settings_undo` pattern (`agents.rs:1495-1562`).
- Produces the routes:
  - `POST /v1/doctor/fix`, with a `DoctorFixRequest`. It answers 200 with a `DoctorFixPlan`. It answers 422 `This item can't be fixed any more. Run the check again.` when an id isn't a fixable item in the last report.
  - `POST /v1/doctor/fix/apply`, with an `ApplyRequest`. It answers 200 with the `DoctorLastFix`. It answers 404 for an unknown or expired preview, and 409 with an `ApplyConflict` exactly as `/v1/config/apply` does.
  - `GET /v1/doctor/undo`: 200 with the `DoctorLastFix`, or 204.
  - `POST /v1/doctor/undo`: 200 with an `ApplyResult`; 204 when there is nothing to undo; 409 with `ApplyConflict { reason: "changed", paths }`.

The fix:
- **Each hooks item** becomes the same plan "Set up" makes. Windows uses `hook_install_changes`; a distro uses `distro_install`. The `want` is:
  - `status: true`;
  - `approve`: `approve_installed`, or a `managed.json` entry in that file whose pointer contains `/PermissionRequest`;
  - `statusline`: the CLI chains and either `chained` or a `managed.json` entry `/statusLine` in that file;
  - `skill: false`;
  - `distro` set for a distro item.

  The bridge is put back at plan time by `ensure_bridge`, as "Set up" does.
- **A retention item** is one `FileEdit`, which the person owns. It is `JsonSet { pointer: "/cleanupPeriodDays", value: 365 }` (a JSON number) on Claude Code's settings path, with `expected_hash` the hash of the text read and `applies: Applies::NewSessions`, as the settings plan uses (`server/agents.rs:1293`).
- **One preview.** All edits go into one `ChangeSet`, titled `Fix N issues`, with a reason listing the item titles one per line. It is previewed with `preview_mixed(cs, &|e| e is a retention edit)`.
- **Restart list.** `restart` is the running sessions whose agent is an item's agent, in the item's distro or on Windows (`wsl_distro_of_tab`), sorted, from the registry's snapshot.
- **Pending fix.** Store `PendingFix`.

On apply success:
1. Write `DoctorLastFix` to `doctor_last_fix(home)`, with `files_match: true`.
2. Clear the pending fix.
3. Call `request_run`.
4. Return the record.

A conflict writes no record.

The undo:
- **Read the record and its snapshot.** For each file, `after_hash` None means the path must not exist. Otherwise the file's hash must equal `after_hash`, or the answer is 409 with the files that differ.
- **Checking the files.**
  - `GET /v1/doctor/undo` computes `files_match` only from files it can read without starting a distro. A file in a distro that isn't running, per Task 5's running list, is left out of `files_match`; the POST checks it.
  - `POST /v1/doctor/undo` first calls `record_distros_of(snapshot paths)` (`agents.rs:290`), so the config guard admits those paths after a restart. This is a click by the person, so starting a distro is allowed here.
- **Restoring.** Restore through the config service, as `settings_undo` does: one `Restore { snapshot_id }` per file with `expected_hash: after_hash`, then preview, then apply with that preview's own hashes.
- **On success:** delete the record, call `request_run`, and return the result.

- [ ] **Step 1: Write the failing tests** (`doctor_api.rs`):
  - `the_seeded_broken_config_is_repaired_and_undone`, the exit criterion:
    - Claude's hooks are gone, Codex's are partial, and Claude's `cleanupPeriodDays` is unset.
    - Fix the three items, then run the report: no fixable item is left, and Codex's trust item is the only Broken one.
    - Then undo: every file is byte for byte as seeded, and `managed.json` is as before the fix.
  - `hooks_and_retention_in_one_file_are_one_preview_and_retention_isnt_managed`.
  - `an_edit_between_the_sheet_and_allow_is_a_409_and_writes_no_record`.
  - `undo_after_an_edit_is_a_409_naming_the_file`.
  - `undo_works_after_a_restart`: build a new `AppState` on the same home, and `GET /v1/doctor/undo` answers the record.
  - `fixing_an_item_that_isnt_fixable_is_a_422`.
  - `the_restart_list_names_the_running_sessions_of_that_agent`.
  - `a_distro_hooks_fix_uses_the_distro_plan`.
- [ ] **Step 2: Run.** `cargo test -j 3 -p marshell-core --test doctor_api`. Expected: the new tests fail.
- [ ] **Step 3: Implement** as described above. `plan` must behave exactly as before; `cargo test -j 3 -p marshell-core --test agents_api` proves it.
- [ ] **Step 4: Run.** `cargo test -j 3 -p marshell-core`. Expected: all pass.
- [ ] **Step 5: Commit.** `feat(doctor): fix the checked issues through one preview and snapshot, and undo the last fix, also after a restart`

## Lane F: frontend (after Task 1; beside lane R)

### Task 7: The Doctor page: groups, checkboxes, the real fix sheet and undo

**Files:**
- Modify: `src/features/doctor/` (`types.ts`, `report.ts`, `fixtures.ts`, `DoctorPage.tsx`, `DoctorRow.tsx`, `FixSheet.tsx`, `UndoToast.tsx`, `undo.ts`, `doctorUi.ts`, `Doctor.css`, `Doctor.stories.tsx`, and their tests). Remove `mergeDiff.ts` and its test if nothing uses them any more.
- Modify: `src/features/window/scenario.ts` (`SCENARIO_FOOTER` from the new fixtures), `src/design/review/renderReview.tsx` if its props change

**Interfaces:**
- Consumes: the generated `DoctorItem`, `DoctorReport`, `DoctorGroup`, `DoctorFixPlan`, `DoctorLastFix` and `ConfigPreview`; `DiffView` (`src/components/DiffView/DiffView.tsx`); `previewDiff` and `deletedFiles` (`src/lib/api/agents.ts:77-85`).
- Produces, in `report.ts`:
  - `initialSelection(r: DoctorReport, prev?: ReadonlySet<string>, prevReport?: DoctorReport): Set<string>`. Fixable Broken items are on. An id that was in the previous report keeps its previous state.
  - `selectedFixable(r, sel): DoctorItem[]`.
  - `fixHeadline(n: number): string`, giving `Fix 1 issue` or `Fix N issues`.
  - `groups(r): { group: DoctorGroup; heading: "Broken" | "Waste" | "Info"; items: DoctorItem[] }[]`. Empty groups are left out, and the order is Broken, Waste, Info.
- Produces the component props:
  - `DoctorPage({ report: DoctorReport | null; phase: DoctorPhase; selected: ReadonlySet<string>; undo?: DoctorLastFix; nowMs: number; elapsedMs?: number; failure?: string; onToggle(id: string): void; onFix(): void; onRunAgain(): void; onUndo(): void; onShowDifferences(): void; onOpenAgents(): void })`.
  - `FixSheet({ plan: DoctorFixPlan; busy: boolean; conflict: boolean; onConfirm(): void; onCancel(): void; onReload(): void })`, and `DifferencesSheet({ preview: ConfigPreview; onClose(): void })` for "Show differences".
  - `UndoToast({ undo: DoctorLastFix; nowMs: number; onUndo(): void; onRestart(): void })`.

Rules:
- **Group headings** are "Broken", "Waste" and "Info", with the item count. The rows no longer show "Error", "Warning" or "Information". A fixed row still shows "Fixed".
- **Checkboxes.**
  - A fixable row's checkbox is a native `<input type="checkbox">`, labelled by the row's title.
  - A row with `cannot_fix` has no checkbox and shows that text.
  - A row with `open_agents` has an "Open Agents" button.
- **The fix button.** "Fix X issues" counts `selectedFixable` and is disabled at 0.
- **The fix sheet.**
  - One `DiffView` per changed file, labelled `Exact changes to <path>`, plus the deleted files.
  - It keeps the backup note at `FixSheet.tsx:42` as it is.
  - The confirm button is the headline.
  - On conflict it says `A file changed after this preview was made, so nothing was written. Reload to see what it says now.` and offers Reload and Cancel, never an overwrite.
  - Focus starts on the title. Escape closes, and focus returns to the opener, through `useDialogFocus` as now.
- **The undo offer.** The toast and header use `undoOffer` from `undo.ts`, with `count` and `fixedAtMs` from `DoctorLastFix` and `filesMatch` from `files_match`. "Restart them" appears only when `restart` isn't empty. When `files_match` is false, the header shows the existing stale text, and its button reads "Show differences".
- **Copy that stays.** The summary line (`DoctorPage.tsx:64-68`), the progress line after 150 ms, "Run again", "Fixing…", the failed line, and `afterFixMessage` all stay.
- **The all-clear state.** It keeps its current copy.
- **Look.** Styles follow the existing `Doctor.css` tokens. A checkbox gets a visible focus ring and a forced-colours rule.

- [ ] **Step 1: Write the failing tests.**
  - `report.test.ts`:
    - `initialSelection` pre-checks Broken only and keeps earlier choices across a new report;
    - `groups` order, with empties dropped;
    - `fixHeadline(1)` and `fixHeadline(3)`.
  - `DoctorPage.test.tsx`:
    - the headings with counts;
    - a checkbox for fixable rows only, checked per `selected`;
    - `cannot_fix` text;
    - "Open Agents";
    - "Fix 2 issues" with two selected, and a disabled button at 0;
    - no "Error", "Warning" or "Information" words.
  - `FixSheet.test.tsx`: a diff per file, the conflict copy with Reload, and no Allow button while in conflict.
  - `UndoToast` and `undo` tests: from `DoctorLastFix`, Restart only with sessions, and the stale header with "Show differences".
- [ ] **Step 2: Run.** `pnpm test src/features/doctor`. Expected: the new tests fail.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `pnpm test src/features/doctor src/styles src/features/window src/design`. Expected: all pass.
- [ ] **Step 5: Commit.** `feat(doctor): the doctor page groups issues as Broken, Waste and Info with a checkbox per fix, and shows the real fix diff`

### Task 8: The live Doctor: API client, events, entry points, fix, undo and restart

**Files:**
- Create: `src/lib/api/doctor.ts` and test; `src/features/doctor/doctorModel.ts` and test (a pure reducer for the page's phase, selection, plan, conflict and undo)
- Modify: `src/lib/api/events.ts` (the `doctor` frame), `src/features/sessions/LiveApp.tsx` (overlay `"doctor"`, `runFromPalette`, the tray state and `actionFor`, the footer), `src/features/tray/useTray.ts` (`actionFor("doctor")` → `{ kind: "overlay", overlay: "doctor" }`), `src/features/window/Sidebar.tsx` only if the footer needs it

**Interfaces:**
- Consumes: Tasks 4 and 6's routes; Task 7's components; `api`, `ApiError` and `getEndpoint` (`src/lib/api/client.ts`); the sessions restart client (`src/lib/api/sessions.ts:119-120`) and the restart path the session's own restart bar uses in `LiveApp.tsx` (~:1730).
- Produces, in `doctor.ts`:
  - `runDoctor(ep): Promise<DoctorSummary>`;
  - `doctorReport(ep): Promise<DoctorReport | null>`;
  - `planFix(ep, ids): Promise<DoctorFixPlan>`;
  - `applyFix(ep, preview): Promise<{ ok: true; fix: DoctorLastFix } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string }>`, mapping the codes as `applyPreview` does;
  - `lastFix(ep): Promise<DoctorLastFix | null>`;
  - `undoFix(ep): Promise<{ ok: true } | { ok: false; changed: string[] } | { ok: false; message: string }>`;
  - `undoDifferences(ep, snapshotId): Promise<ConfigPreview>`, through `POST /v1/snapshots/{id}/restore`, which only previews.

Rules:
- **Entry points.**
  - The palette's "Open Doctor", the tray's Doctor item and a click on the sidebar's Doctor count each open the `"doctor"` overlay.
  - The tray item and the sidebar count use the `doctor` frame's `issues`. They are left out until the first summary arrives (decision 3).
- **Runs.** Opening the page calls `runDoctor`, as "Run again" does. A finished summary (`running` false, with a new `checked_at_ms`) reloads the report while the page is open.
- **Fix.**
  1. "Fix X issues" calls `planFix` with the selected fixable ids and opens the sheet.
  2. Allow calls `applyFix`.
  3. On success, the rows tick off, the toast shows, and the Announcer says `fixAnnouncement`'s text as an `action-result`.
  4. A conflict shows the sheet's conflict state, and Reload re-plans.
- **Undo** calls `undoFix`.
  - Success announces `The last fix is undone.` as an `action-result`.
  - `changed` turns the header to the stale offer.
  - "Show differences" opens `DifferencesSheet` with `undoDifferences`.
  - When the page opens, it reads `lastFix` for the header after a restart.
- **"Restart them"** restarts each listed session through the same call the session's own restart bar makes, in its current mode. A session that is working, or whose restart needs an acknowledgement (`restartNeedsAck`, `src/features/sessions/sessionActions.ts:415`), is skipped. The receipt names the skipped sessions:
  - one session: `Session 3 wasn't restarted. Restart it from its tab.`
  - several: `Sessions 3 and 5 weren't restarted. Restart them from their tabs.`
  - It uses `listNumbers` from `report.ts`.
- **"Open Agents"** calls `openSettings("agents")`.

- [ ] **Step 1: Write the failing tests.**
  - `doctor.test.ts`: each function's method, path and body, with fetch stubbed as `agents.test.ts` does; the `applyFix` code mapping; and `undoFix`'s 409 → `changed`.
  - `doctorModel.test.ts`:
    - phases running → ready → fixing → fixed;
    - selection kept across a new report;
    - conflict then reload;
    - the undo record after a restart.
  - `events.test.ts`: a `doctor` frame reaches its subscriber.
  - `useTray.test.ts`: `actionFor("doctor")`.
  - `LivePalette`/`LiveApp` tests in the existing style: "Open Doctor" opens the overlay.
  - A pure test for the restart receipt copy.
- [ ] **Step 2: Run.** `pnpm test src/lib/api/doctor.test.ts src/features/doctor src/features/tray src/lib/api/events.test.ts`. Expected: the new tests fail.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `pnpm test src/lib/api src/features/doctor src/features/tray src/features/sessions src/features/window`. Expected: all pass.
- [ ] **Step 5: Commit.** `feat(doctor): the live app opens the doctor from the palette, the tray and the sidebar, and fixes, undoes and restarts through the core`

## After the lanes (the lead)

### Task 9: Checks, live check and research note

**Files:**
- Create: `docs/research/2026-11-05-doctor-7a-live-check.md`

- [ ] **Step 1: Full checks.** Run what CI runs (`marshell-ship` step 1), with `cargo test -j 4 --workspace`.
- [ ] **Step 2: Live check (exit criterion).** Use `marshell-live-check` with a new scratch folder `lc-7a-1`.
  1. Set up Claude Code's and Codex's hooks in the scratch homes through the Agents dialog.
  2. Break them by hand in the scratch files: remove Claude's entries, change one Codex entry, and leave Claude's `cleanupPeriodDays` unset.
  3. Open the Doctor from the palette. Check that the groups, the pre-checked Broken items and the unchecked retention item are right. Check that "Fix 2 issues" changes to "Fix 3 issues" when retention is ticked.
  4. Fix, read the sheet's diffs, and Allow. Check that the rows tick off, the toast shows, and the files are repaired, comparing hashes before and after.
  5. Run again, and check that no Broken item is left apart from Codex trust.
  6. Undo. Check that every file matches its seeded hash.
  7. Then fix again, restart the app, and check that "Undo last fix" is offered and works.
  8. Check the keyboard-only path and emulated forced colours.
  9. Check that the tray item and the sidebar count show the summary.
  10. WSL distros are covered by tests only, unless a managed distro is at hand. Say so in the note.
- [ ] **Step 3: Write the note**, using the skill's template.
- [ ] **Step 4: Commit.** `docs(research): the doctor 7a live check`

