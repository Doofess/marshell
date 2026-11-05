# Slice 7b: each CLI's own doctor, and duplicate notifications — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor page gets a "Run CLI doctors" button. It runs `claude doctor` and `codex doctor --json` on Windows and in running managed WSL distros, and folds each problem they report into the three groups as a report-only item. Marshell's own background checks gain duplicate-notification items, and Antigravity's `notifications: true` can be fixed with preview and undo.

**Architecture:**
- **Parsing.** A new pure module, `crates/core/src/doctor/cli.rs`, turns a doctor's exit code and output into `DoctorItem`s. It does no I/O.
- **Running.** `crates/core/src/server/doctor.rs` gains the CLI doctor run:
  - a runner seam on `AppState`, as `status_runner` is;
  - a `POST /v1/doctor/cli` route;
  - the CLI items kept in `DoctorState` and merged into every later report.
- **Duplicates.** These are local, zero-token facts, so they join 7a's `AgentFacts` and `checks::items`. The new fix kind `notify.*` is one `JsonSet`, marked as the person's own key, as retention is.
- **Frontend.** `DoctorPage` gets the button, a status line and the running state. `LiveDoctor` makes the call.

No new dependencies.

**Tech Stack:** Rust (axum core, the config service, ts-rs, `toml_edit`), React + TypeScript (Vitest in node, no DOM).

**Spec:** `docs/PLAN.md` §9, "Slice 7b decisions (2026-11-05)", on top of "Slice 7a decisions (2026-11-05)" and BRIEF §5.4 and §5.5. Facts were read on main at `19181ae`; line numbers may drift a few lines.

Facts this plan relies on, checked on this PC on 2026-11-05:
- **`claude doctor`** (2.1.318) needs no TTY, takes about 1.3 s and exits 0. The binary builds its tail from these lines:
  - `"<n> warning found"` or `"<n> warnings found"`, coloured yellow;
  - then, for each warning, `- <issue>` and on the next line `  Fix: <fix>`;
  - or, when there's no warning, `No installation issues found.`.
- **`codex doctor --json`** (0.164.0) gives `{schemaVersion: 1, generatedAt, overallStatus, codexVersion, checks: {<id>: {id, category, status, summary, details, remediation, durationMs}}}`.
  - The statuses are `ok`, `warning` and `fail`, per Codex's own tests in `codex-rs/cli/tests/doctor_path_safety.rs`.
  - A `fail` exits 1, and the JSON is still printed.
- **`qwen doctor`** isn't a command: it would be read as a prompt. Never run it. `agy` has no doctor either.
- **The runner and WSL helpers.** `crate::agents::run_capture(exe, args, timeout) -> Option<(i32, String)>` joins stdout, then stderr. `profiles::detect::run_wsl` returns stdout only on success. `WSL_VERSION_SCRIPT` in `server/agents.rs` ends in `exec "$0" --version`.

## Global Constraints

- **Commits and staging.**
  - Commit messages read `type(scope): what changed, as a sentence`. Run `cargo fmt --all` before every Rust commit.
  - Every cargo command uses `-j 3`, for example `cargo test -j 3 -p marshell-core doctor`.
  - Stage only your own files, by path. Never `git add -A` or `git commit -a`.
  - Run only your own tests: the crate's filtered `cargo test` and `pnpm test <file>`. The lead runs the full suites.
- **Worktree.** Work in `C:\dev\Airport\.claude\worktrees\phase-7b` on branch `phase-7b`, with plain commands from its root.
- **Protocol.**
  - Changed types stay in `crates/protocol/src/doctor.rs`. Every `u64` gets `#[ts(type = "number")]`.
  - An optional field is `Option`, with `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]`.
  - A new `bool` in a type the UI already reads gets `#[serde(default)]`.
  - Run `cargo test -j 3 -p marshell-protocol` and `pnpm check:generated`, and commit `src/generated/`.
- **Async and local API.**
  - Never hold a `MutexGuard` across `.await` or I/O. Process runs and file reads go in `spawn_blocking`.
  - A new route sits in the token-guarded router and has a "refused without the bearer" test.
  - Don't touch `guard.rs` or token handling. No route takes a path.
- **Never in a test or worker:**
  - A test never runs a real CLI or `wsl.exe`. The doctor runner, the running distros and the distro targets are pinned on `AppState`.
  - A fake home lives under the repo's `target` folder, as in 7a's tests.
  - Never run `pnpm tauri dev` or a real CLI from a worker.
- **Report-only (7b decision 3).** A CLI doctor item always has `fixable: false`, `open_agents: false` and `path: None`. The only new fix is `notify.antigravity`.
- **Never shown:** Codex's `details`, or any output beyond the parsed summary, issue and remediation text.
- **Copy.** Sentence case, subject first, no "!", no "successfully". This plan's copy is exact.
- **Frontend.**
  - Read the `frontend-design` and `good-css` skills before styling.
  - Keyboard alone and forced colours work. The Announcer is the only live region.
  - `dialogBodies.test.ts`, `forcedColorsEdges.test.ts` and `noBoxedOutlines.test.ts` stay green.

## Review Focus

1. **A doctor that hangs or floods.** A CLI doctor that doesn't exit is killed after 60 s and gives the "didn't finish" item. A grandchild holding the pipe doesn't hold the run. The rest of the CLIs still report. Task 3 tests this with a runner that answers `None`.
2. **A second click while the CLI doctors run, and a background run meanwhile.**
   - The click joins the run; nothing runs twice.
   - A background run during the CLI run keeps the previous CLI items.
   - When the CLI run ends, `running` goes true before `cli_running` goes false, so the page reloads the report that has the new items.

   Task 3 tests this.
3. **Output that isn't what the parser expects.** For example, Claude's text in another language, a header with no `- ` lines, JSON after a stderr warning, or a `checks` that is an array. Each gives the one "can't read" item, never a panic and never raw output. Task 1 tests this.
4. **Antigravity's fix while its `settings.json` was edited after the report.** Apply refuses with 409 and writes nothing. A Hooks off later leaves `notifications` alone, because `managed.json` doesn't record it. Task 2 tests the `managed.json` half; 7a's apply tests cover the 409.
5. **Banners turned off in Settings.** No `notify.*` item appears, and the sidebar count drops on the next run. Task 2 tests this.

---

### Task 1: Parsing the CLI doctors' output

**Files:**
- Create: `crates/core/src/doctor/cli.rs`
- Modify: `crates/core/src/doctor/mod.rs` (add `pub mod cli;`)

**Interfaces:**
- Consumes: `marshell_protocol::doctor::{DoctorItem, DoctorGroup}` and `checks::display_name`.
- Produces:
  ```rust
  /// What running one CLI doctor gave: `None` when it didn't finish in time or couldn't start.
  pub type Ran = Option<(i32, String)>;
  pub const CLI_DOCTOR_TIMEOUT_SECS: u64 = 60;
  pub fn claude_items(ran: &Ran, distro: Option<&str>) -> Vec<DoctorItem>;
  pub fn codex_items(ran: &Ran, distro: Option<&str>) -> Vec<DoctorItem>;
  /// Antigravity and Qwen Code.
  pub fn no_doctor_item(agent: &str, distro: Option<&str>) -> DoctorItem;
  ```

Every item has `agent: Some(<agent>)`, `distro` as given, `fixable: false`, `path: None` and `open_agents: false`.
- **Ids.** An id is `cli.<agent>` or `cli.<agent>.<key>`, with `@<distro>` appended in a distro.
- **Places.** `<place>` is empty on Windows and ` in <distro>` in a distro.
- **ANSI codes.** Strip them (`\x1b[` … letter) before matching lines.

Copy (exact):

| Case | id | group | title | detail | cannot_fix |
|---|---|---|---|---|---|
| Claude warning | `cli.claude.<key>`; `<key>` is the first 8 hex digits of `hash_bytes(issue)` | Info | `Claude Code's doctor<place>: <issue>` | `Found by claude doctor. Marshell reports it and doesn't change it.` | `Fix: <fix>`, or absent when there's no `Fix:` line |
| Claude ok | none (no item) | | | | |
| Codex `warning` | `cli.codex.<check id>` | Info | `Codex's doctor<place>: <summary>` (the check id when `summary` is empty or missing) | `Check <check id>, found by codex doctor. Marshell reports it and doesn't change it.` | `remediation` when it's a non-empty string, else absent |
| Codex `fail` | as above | Broken | as above | as above | as above |
| Codex another status, not `ok` | as above | Info | as above | as above | as above |
| Didn't finish (`None`) | `cli.<agent>` | Info | `<CLI>'s doctor<place> didn't give a result` | `It didn't finish in 60 seconds.` | `Run <command> in a terminal to see what it says.` (`claude doctor` or `codex doctor`) |
| Can't read | `cli.<agent>` | Info | as above | `Marshell can't read what it printed (exit code <n>).` | as above |
| No doctor command | `cli.<agent>` | Info | `<CLI><place> has no doctor command.` | `Marshell's own checks still cover its hooks and settings.` | absent |

Parsing rules:
- **Claude.**
  - Exit 0 with a `No installation issues found.` line is ok.
  - Otherwise, a line matching `^\d+ warnings? found$`, followed by at least one `- <issue>` line, gives one item per `- ` line after it, in order. Each item takes the `Fix: ` line that follows it, trimmed.
  - Anything else is "can't read", whatever the exit code.
- **Codex.**
  - Read the first JSON value at the start of the output, skipping leading whitespace, with `serde_json::Deserializer::from_str(..).into_iter::<Value>()`, so text after it is ignored.
  - `checks` must be an object, and the items are sorted by check id.
  - Exit code 0 or 1 with a readable object is a result. Any other code, or no readable object, is "can't read".

- [ ] **Step 1: Write the failing tests** in `cli.rs`'s `mod tests`. Use the real tail strings above as fixtures, and assert every field the table pins:
  - `claude_ok_gives_nothing`: the full 2.1.318 output above gives `[]`.
  - `claude_warnings_give_one_item_each`: `"2 warnings found\n- Multiple installs found\n  Fix: Remove the npm one\n- Ripgrep missing\n"` gives two Info items. The first has cannot_fix `Fix: Remove the npm one`, and the second has none.
  - `claude_ansi_header_is_read`: `"\x1b[33m1 warning found\x1b[39m\n- X\n  Fix: Y\n"` gives one item.
  - `claude_unreadable_and_header_without_lines_are_one_item`: French text, and a header with no `- ` lines, each give exactly one `cli.claude` "can't read" item, with that exit code in the detail.
  - `claude_timeout`: `None` gives the "didn't finish" item.
  - `codex_maps_statuses`: checks with `ok`, `warning`, `fail` and `skipped` give no item for `ok`, Info for `warning`, Broken for `fail` and Info for `skipped`. Exit 1 is still read. The items are sorted by id, and `details` text appears in no field.
  - `codex_json_then_noise_is_read`: an object followed by `"\nWARN something"` is read.
  - `codex_bad_shapes_are_one_item`: `checks` as an array, invalid JSON, empty output, and exit 2 with valid JSON each give one `cli.codex` "can't read" item.
  - `distro_ids_and_titles`: with `Some("Ubuntu")`, the ids end in `@Ubuntu` and the titles read `Codex's doctor in Ubuntu: …` and `Qwen Code in Ubuntu has no doctor command.`.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-core doctor::cli`. Expected: FAIL (the functions are not defined).
- [ ] **Step 3:** Implement the three functions in `cli.rs`, with `crate::config::hash::hash_bytes` for the Claude key.
- [ ] **Step 4:** Run `cargo test -j 3 -p marshell-core doctor::cli`. Expected: PASS.
- [ ] **Step 5:** Run `cargo fmt --all`, then commit `feat(doctor): the CLI doctors' output becomes report-only items`.

### Task 2: Duplicate notifications in Marshell's own checks, and Antigravity's fix

**Files:**
- Modify: `crates/core/src/doctor/checks.rs` (new facts, new items, the `items` signature)
- Modify: `crates/core/src/server/doctor.rs`:
  - `facts_from`, `distro_agent_facts` and `failed_facts` gather the facts;
  - `fix_of` gains the `notify` arm;
  - `is_retention_edit` becomes `is_own_edit`;
  - `run_once` and `fix_of` pass `banners_on`.
- Test: the same two files' `mod tests`.

**Interfaces:**
- Produces in `checks.rs`:
  ```rust
  #[derive(Debug, Clone, Default, PartialEq, Eq)]
  pub struct NotifyFacts {
      /// Antigravity: its settings.json. Codex: its config.toml. Claude Code and Qwen Code: the hooks file (`path`).
      pub file: PathBuf,
      /// Antigravity's `notifications` is `true`.
      pub desktop: bool,
      /// Codex's top-level `notify` array, its strings joined with spaces.
      pub command: Option<String>,
      /// The `command` of every hook under `hooks.Notification` that `agents::common::is_ours` doesn't claim
      /// (Claude Code, Codex and Qwen Code), in file order.
      pub hook_commands: Vec<String>,
  }
  // AgentFacts gains: pub notify: NotifyFacts,
  pub fn items(facts: &[AgentFacts], stopped: &[String], banners_on: bool) -> Vec<DoctorItem>;
  ```
- In `server/doctor.rs`, `banners_on` is the `needs_you`, `done` or `error` banner switch of `st.notify_settings()`, any one being on.

Items, only when `banners_on` is true and the file parsed. They come after a CLI's other items. Copy (exact):

| id (`@<distro>` in a distro) | group | fixable | title | detail | cannot_fix | path |
|---|---|---|---|---|---|---|
| `notify.antigravity` when `desktop` | Broken | true | `Antigravity<place> sends its own desktop notifications` | `With Marshell's banners on, you get each one twice. The fix sets notifications to false.` | — | `notify.file` |
| `notify.codex` when `command` is some | Info | false | `Codex<place> runs its own notify command` | `After each turn it runs <command>. If that shows a desktop notification, you get two.` | `Marshell doesn't change your notify command.` | `notify.file` |
| `notify.<agent>` when `hook_commands` isn't empty | Info | false | `<CLI><place> runs your own Notification hook` | `It runs <commands joined with "; ">. If that shows a desktop notification, you get two.` | `Marshell doesn't change your hooks.` | `notify.file` |

A Codex that has both a `notify` command and Notification hooks gets `notify.codex` and `notify.codex.hooks`; every other CLI has at most one `notify.*` item. 7a's read-only rule applies: a read-only file makes `notify.antigravity` unfixable with `The file is read-only.`.

Gathering the facts:
- **Antigravity.** Windows reads `notifications` from `setup.settings_path`. A distro reads it from the already-read `values[1]`. Only `Value::Bool(true)` counts.
- **Codex.**
  - Read `config.toml` beside its hooks file, `path.with_file_name("config.toml")`, as the distro code already does for trust.
  - Parse it with `toml_edit::DocumentMut`, and take the top-level `notify` array of strings.
  - A file that is missing or doesn't parse gives `None`.
- **Notification hooks.** Read `hooks.Notification[*].hooks[*]` from the value the hooks file already gave. Skip an entry when `is_ours` claims it or when it has no string `command`.

The fix:
- `fix_of`'s `Some("notify")` arm accepts only `agent == "antigravity"`; any other agent gets `cant_fix()`.
- It builds a `FileEdit`, as `retention_edit` does: `JsonSet { pointer: "/notifications", value: "false" }` on `notify.file`, with the hash of the file as it is now and `Applies::NewSessions`.
- `is_own_edit` is true for `/cleanupPeriodDays` and `/notifications`, so `managed.json` records neither.

- [ ] **Step 1: Write the failing tests.**
  - In `checks.rs`, update `facts()` to `notify: NotifyFacts::default()` and every `items(..)` call to pass `true`. Then add:
    - `antigravity_desktop_notifications_are_broken_and_fixable`: every field in the table, plus path.
    - `codex_notify_and_hooks_are_info`: both items appear, unfixable, with the exact copy.
    - `claude_notification_hook_is_info`: `hook_commands: ["toast.exe a", "b.sh"]` gives the detail `It runs toast.exe a; b.sh. If that shows a desktop notification, you get two.`.
    - `banners_off_gives_no_notify_item`: with `banners_on` false, nothing appears.
    - `read_only_antigravity_cannot_be_fixed`: a read-only file makes the item unfixable.
    - `a_distro_notify_item_has_its_suffix`: the id is `notify.antigravity@Ubuntu`, and the title says ` in Ubuntu`.
  - In `server/doctor.rs`, add:
    - `notify_facts_are_read_for_each_cli` (a fake home): Antigravity's `settings.json` has `"notifications": true`, Codex's `config.toml` has `notify = ["a.exe", "x"]`, and Claude's `settings.json` has a Notification hook of its own beside one of Marshell's (made by Marshell's installer, or with `--tag` args). The test asserts the three `NotifyFacts`.
    - `the_antigravity_fix_sets_notifications_false_and_managed_json_skips_it`: it plans `notify.antigravity`, applies, and asserts that the file has `"notifications": false` and that `managed.json` names no `/notifications` pointer.
    - `a_notify_fix_for_codex_is_refused`: 422.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-core doctor`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement the facts, the items, the fix arm and `is_own_edit`.
- [ ] **Step 4:** Run `cargo test -j 3 -p marshell-core doctor`. Expected: PASS, with 7a's tests still green.
- [ ] **Step 5:** Run `cargo fmt --all`, then commit `feat(doctor): duplicate notifications are reported, and Antigravity's can be turned off`.

### Task 3: Running the CLI doctors: protocol, runner, route and report merge

**Files:**
- Modify: `crates/protocol/src/doctor.rs`:
  - `DoctorSummary.cli_running: bool` with `#[serde(default)]`;
  - `DoctorReport.cli_checked_at_ms: Option<u64>`.
- Modify: `src/generated/` (regenerated)
- Modify: `crates/core/src/doctor/state.rs`:
  - `pub cli: Mutex<CliRun>`;
  - `#[derive(Default)] pub struct CliRun { pub running: bool, pub items: Vec<DoctorItem>, pub checked_at_ms: Option<u64> }`.
- Modify: `crates/core/src/server/agents.rs`:
  - split `WSL_VERSION_SCRIPT` into a PATH prefix plus `exec "$0" "$@"`;
  - add `pub(crate) fn wsl_exec_argv(distro: &str, exe: &str, args: &[&str]) -> Vec<String>`;
  - `wsl_version_argv` calls it with `["--version"]`.
- Modify: `crates/core/src/profiles/detect.rs`: `pub(crate) fn run_wsl_capture(args: &[&str], timeout: Duration) -> Option<(i32, Vec<u8>)>`, which keeps stdout whatever the exit code and gives `None` on timeout or when it can't start.
- Modify: `crates/core/src/server/mod.rs`:
  - the field `doctor_runner: DoctorRunner`, with its default;
  - `pub fn with_doctor_runner(self, run: DoctorRunner) -> Self`;
  - the route `POST /v1/doctor/cli`.
- Modify: `crates/core/src/server/doctor.rs`: `cli_run`, `request_cli_run`, `cli_doctor_items`, and the merge in `run_once`.

**Interfaces:**
- Consumes: Task 1's `claude_items`, `codex_items`, `no_doctor_item` and `CLI_DOCTOR_TIMEOUT_SECS`. Also `st.agent_exe_of(agent)`, `st.agent_version_in(distro, agent)` and `agent_command(agent)`.
- Produces:
  ```rust
  /// One CLI doctor to run: on Windows `exe` is the resolved program, in a distro it is the Linux program name.
  pub(crate) struct DoctorCommand { pub distro: Option<String>, pub exe: PathBuf, pub args: Vec<&'static str> }
  pub(crate) type DoctorRunner = fn(&DoctorCommand) -> crate::doctor::cli::Ran;
  pub async fn cli_run(State(st): State<AppState>) -> (StatusCode, Json<DoctorSummary>); // 202
  ```
  The frontend gets `DoctorSummary.cli_running` and `DoctorReport.cli_checked_at_ms`.

Behaviour:
- **The default runner.** On Windows it is `run_capture(exe, args, 60 s)`. In a distro it is `run_wsl_capture(wsl_exec_argv(distro, exe, args), 60 s)`, with the stdout read as lossy UTF-8.
- **What runs.** `cli_doctor_items(st)` runs in `spawn_blocking`:
  - On Windows, in `["claude", "codex", "antigravity", "qwen"]` order, for each agent with `agent_exe_of`: `claude doctor` gives `claude_items`, and `codex doctor --json` gives `codex_items`. Antigravity and Qwen Code give `no_doctor_item`, with no run.
  - Then, for each running managed distro (the same partition `distro_facts` makes; factor it out as `running_managed_distros(st, &entries) -> Vec<String>`), and each agent with `agent_version_in(distro, agent).is_some()`, the same thing with `distro: Some`.
  - A stopped distro is never touched; 7a's "Not checked" item already covers it.
- **`request_cli_run`.**
  - Under the `cli` lock: if `running`, return; nothing runs twice. Otherwise set `running` and send `summary.cli_running = true`.
  - Spawn a task that awaits `cli_doctor_items`, then stores `items` and `checked_at_ms = now_ms()` under the lock.
  - Then call `request_run(&st)` first, and only after that clear `running` and send `summary.cli_running = false`.
  - A failed join logs, keeps the old items, and still clears the flag.
- **`run_once`.** It appends a clone of `cli.items` after `checks::items(..)`, and sets `cli_checked_at_ms` from `cli.checked_at_ms`. The lock is taken and released while the report is built, never across an await. `checks::issues` counts them like any item.

- [ ] **Step 1: Write the failing tests.**
  - Protocol: `a_summary_round_trips` also covers `cli_running`, and `{"running":false,"issues":0}` still parses (with `cli_running` false).
  - `server/doctor.rs`, each with a pinned `doctor_runner` and a fake home where `agent_exe_of` finds a fake `claude` and `codex`, as the account-status tests do:
    - `cli_doctors_fold_into_the_report`: the runner answers Claude ok and Codex with one `fail`. After the run, the report has `cli.codex.<id>` in Broken, `cli_checked_at_ms` is set, and `summary.issues` counts it.
    - `a_cli_doctor_that_times_out_leaves_the_others`: Claude `None` and Codex ok give one `cli.claude` "didn't finish" item.
    - `a_second_request_joins_the_cli_run`: a runner that blocks on a channel; two `request_cli_run` calls make one runner call per CLI, and `cli_running` is true until the release.
    - `cli_items_survive_a_background_run`: after a CLI run, a `request_run` alone keeps the `cli.*` items.
    - `running_is_set_before_cli_running_clears`: watch the summary channel, and assert that no frame has `running == false && cli_running == false` before the report holds the new items.
    - `cli_doctors_run_only_in_running_managed_distros`: pinned running distros and versions (`wsl_version`). A runner that records its commands sees `distro: Some("Ubuntu")` for the running one and never the stopped one.
    - `cli_route_is_refused_without_the_bearer`.
  - `server/agents.rs`: `wsl_exec_argv_passes_the_args`. The argv ends `[.., WSL_SCRIPT, "codex", "doctor", "--json"]`, and the version test still holds.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-protocol` and `cargo test -j 3 -p marshell-core doctor`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement as above. Run `cargo test -j 3 -p marshell-protocol` to regenerate, and `pnpm check:generated`.
- [ ] **Step 4:** Run `cargo test -j 3 -p marshell-core doctor`, `cargo test -j 3 -p marshell-core agents` and `pnpm check:generated`. Expected: PASS.
- [ ] **Step 5:** Run `cargo fmt --all`, then commit `src/generated/` with `feat(doctor): Run CLI doctors runs claude doctor and codex doctor and folds their findings in`.

### Task 4: The Doctor page's "Run CLI doctors"

**Files:**
- Modify: `src/lib/api/doctor.ts`: `export const runCliDoctors = (ep: Endpoint) => api<DoctorSummary>(ep, "POST", "/v1/doctor/cli");`
- Modify: `src/features/doctor/DoctorPage.tsx`, `src/features/doctor/LiveDoctor.tsx`, `src/features/doctor/Doctor.css`, `src/features/doctor/fixtures.ts` and `src/features/doctor/Doctor.stories.tsx`
- Test: `src/features/doctor/DoctorPage.test.tsx` and a new `src/features/doctor/cliDoctors.test.ts`

**Interfaces:**
- Consumes: Task 3's `cli_running` and `cli_checked_at_ms`, and `POST /v1/doctor/cli`.
- Produces:
  - `DoctorPageProps` gains `cliRunning: boolean` and `onRunCliDoctors: () => void`.
  - `cliDoctors.ts` exports `cliLine(checkedAtMs: number | undefined, nowMs: number): string` and `cliDoneAnnouncement(report: DoctorReport): string`.

Behaviour and copy (exact):
- **The button.** "Run CLI doctors" is a secondary `doctor__btn` in the header actions, before "Run again", shown whenever "Run again" is shown.
  - While `cliRunning`, it reads "Running CLI doctors…" with `aria-disabled="true"`, and a click does nothing.
  - It stays in the tab order.
- **The status line.** It sits under the summary line, as `<p className="doctor__cli">`:
  - before a run: `CLI doctors haven't run. Codex's doctor contacts OpenAI.`
  - after: `CLI doctors ran <n> s ago. Codex's doctor contacts OpenAI.`, with `<n>` worked out as the header's `Checked` is;
  - for 60 s and over: `<n> min ago`, rounded down.
- **The click.** `LiveDoctor` calls `runCliDoctors` and keeps a local `cliPending` until a summary frame shows `cli_running` false after it was true, or the call fails. The button state is `cliPending || summary?.cli_running`.
  - A failed call announces `Couldn't run the CLI doctors: <message>`.
- **The reload.** The report reloads through the existing `checked_at_ms` effect.
- **The finish.** When `cli_running` goes true → false, `LiveDoctor` announces `cliDoneAnnouncement(report)` once the reloaded report is in:
  - `The CLI doctors found nothing.` when no Broken or Waste `cli.*` items;
  - `The CLI doctors found 1 issue.` or `The CLI doctors found <n> issues.` otherwise, where `<n>` counts the Broken and Waste `cli.*` items.
- **The rows.** CLI items render through the existing `DoctorRow`: no checkbox, and the `cannot_fix` text. No row change is needed.
- **Storybook.** `fixtures.ts` gains a report with one Codex `fail` item, one Claude warning and one "has no doctor command" item. The stories show the page with it, and with `cliRunning`.

- [ ] **Step 1: Write the failing tests.**
  - `cliDoctors.test.ts` covers `cliLine` with `undefined`, 12 s and 125 s (`CLI doctors ran 2 min ago. …`), and `cliDoneAnnouncement` with 0, 1 and 3 issues, where an Info `cli.*` item isn't counted.
  - In `DoctorPage.test.tsx`:
    - the button and the status line render in `ready`;
    - with `cliRunning`, "Running CLI doctors…" has `aria-disabled="true"`;
    - in `fixing`, the button is absent;
    - a CLI item renders no checkbox and shows its `cannot_fix`.
- [ ] **Step 2:** Run `pnpm test src/features/doctor`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement. Style `.doctor__cli` as `.doctor__summary`'s quieter sibling, with tokens only. Read `good-css` first.
- [ ] **Step 4:** Run `pnpm test src/features/doctor` and `pnpm build`. Expected: PASS.
- [ ] **Step 5:** Commit `feat(doctor): the Doctor page runs the CLI doctors on a click`.

### Task 5: Checks, live check and research note (the lead)

- [ ] **Step 1: The full suites.** Run `cargo fmt --all -- --check`, `cargo clippy -j 3 --workspace --all-targets -- -D warnings`, `cargo test -j 3 --workspace`, `pnpm test`, `pnpm check:generated` and `pnpm build`.
  - The two known `crates/host` `core_killed` store-shell failures on this PC are noted, not fixed.
- [ ] **Step 2: The live check** (the `marshell-live-check` skill), with a scratch `MARSHELL_HOME`, a scratch `CLAUDE_CONFIG_DIR` and a scratch Antigravity home. Over CDP:
  - Open the Doctor and click "Run CLI doctors". Expect the running state, then real items or none, and the status line.
  - Expect a second click during the run to change nothing.
  - **Antigravity toast (decision 4).** Turn `notifications` on in the scratch Antigravity settings, and start an `agy` session that finishes a turn while Marshell's window is in the background. Record whether a native Windows toast that isn't Marshell's appears, and whether Antigravity hooks see it.
    - If no native toast appears, change `notify.antigravity` to Info and unfixable, and amend decision 4 with the evidence.
    - Then check that the item is in the report, that "Fix 1 issue" sets it false through the sheet, and that Undo brings `true` back.
  - Turn the banners off in Settings, run again, and expect no `notify.*` item.
- [ ] **Step 3: The research note.** Write the findings to `docs/research/2026-11-05-cli-doctors-live-check.md` and commit it.
- [ ] **Step 4: Review and ship.** Run the final whole-branch review (`marshell-reviewer`, Opus) with this plan's Review Focus, then one fix pass, then `marshell-ship`.
