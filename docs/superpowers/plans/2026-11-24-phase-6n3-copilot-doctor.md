# Slice 6n3: Copilot CLI in the Doctor

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor checks Copilot CLI wherever it checks Claude Code, Codex, Antigravity and Qwen Code: hooks, the retention sentence, the "no doctor command" line, duplicate notifications (with a fix for Copilot's own toasts), MCP servers and skills with their turn-off fixes, the 30-day usage scan and Measure, on Windows and in WSL distros. Issue #154 is closed on the way.

**Architecture:** The Doctor's engine (`crates/core/src/doctor/`) knows no CLI's rules. Each CLI has arms in `server/doctor*.rs` and two readers, `agents/<cli>/extensions.rs` (inventory) and `agents/<cli>/usage.rs` (history scan). Copilot gets an arm where `"qwen"` has one, and every `_ =>` fallback that today sends an unknown agent to Qwen's code becomes an explicit arm. Copilot's two new readers follow Qwen's files in shape. Five small engine additions carry what Copilot has and the others don't: a toast fact with its own fixable item, a "not Marshell's file" state, `Origin::BuiltIn` for a server that comes with the CLI but is judged like the person's own, `Expand::DollarDefault`, and a "names only past N" listing cap.

**Tech Stack:** Rust (axum, serde_json, `config::jsonc`), React + TypeScript, Vitest through `renderToStaticMarkup`. No protocol type changes, so `src/generated/` stays as it is.

**Spec:** `docs/PLAN.md` §9, "Slice 6n3 decisions (2026-11-24)", 1 to 22, which rests on "Slice 7a decisions", "Slice 7b decisions" and "Slice 7c decisions". Evidence: `docs/research/2026-11-06-copilot-cli.md`, part 3, where "What 6n3's first task saw" (findings 1 to 21) is what Copilot 1.0.106 really did and overrides the static read above it.

**Checked before planning (2026-11-24, on `origin/main` 8aca488a, Copilot CLI 1.0.106):**
- No Doctor file names Copilot. `server/doctor.rs:46` `AGENTS`, `doctor/checks.rs:14` `ORDER` and `doctor/ext.rs:11` `ORDER` hold four agents, and `tests/doctor_api.rs:17` `AGENTS` too.
- Fallbacks that would run another CLI's code for Copilot, all in `server/doctor.rs`: `facts_of` (`_ => qwen_setup_of`, ~284), `distro_agent_facts` (`_ => .qwen/settings.json`, ~518, and `_ => qwen::hooks::inspect_for`, ~591), `version_gate` (~676), and `notify_facts` (~460, which reads `/hooks/Notification` of `facts.path` for any agent but Antigravity and Codex). `doctor_items_of` (~159) answers `no_doctor_item` for any agent but Claude Code and Codex, which is right for Copilot.
- `server/doctor_ext.rs::read_inventory` (~117) and `server/doctor_usage.rs::scan_all` (~48) match four agents and skip any other.
- `server/agents.rs::copilot_setup_of` (~1143) gives an `AgentSetup` with `hooks_path` (`<home>/hooks/marshell.json`) and `settings_path`; `settings_state` is the state of `settings.json` only, and the hooks file is read with `read_to_string().ok()`. `agents/copilot/hooks.rs::inspect` reads a hooks file that doesn't parse as holding nothing, and `install_plan` refuses one that doesn't parse or isn't Marshell's (`PlanError::DoesNotParse`, `PlanError::NotMarshells`).
- `server/doctor.rs::hooks_want_of` (~885) keeps a status line only on Windows and looks for `/statusLine` in the hooks file or in Antigravity's settings. Copilot's is in `<home>/settings.json`, and a distro install writes one too.
- `doctor/usage.rs` and `doctor/measure.rs` leave `Origin::Bundled` items out of every verdict, so a built-in server that is to be judged can't be `Bundled`.
- `server/doctor_measure.rs` expands `Expand::Braces` (`${VAR}`, `${VAR:-default}`) and `Expand::Dollar` (`$VAR`, `${VAR}`). Copilot's entries take all three forms.
- `server/hook.rs:259` skips Copilot's 500 ms end hold for a WSL tab (`!on_wsl`), and the WSL branch above it (157 to 163) clears the tab's agent mark at once on a `sessionEnd` and then doesn't fold (`left_wsl = cleared || !launched`).
- The server tests' state builder, `server/doctor.rs` `pinned_with` (~1507), and `tests/doctor_api.rs` `state_on` (~131) pin a home and a version for four CLIs and none for Copilot.
- Frontend: only `DoctorPage.tsx:101` (the progress line), `fixtures.ts` and `DoctorPage.test.tsx` name the CLIs.

## Global constraints

- `<copilot home>` is `agents::copilot::home`: `COPILOT_HOME` when set and not blank, else `<user home>\.copilot`. In a distro it is the distro's own `~/.copilot`.
- New writes, all through the config service (preview, diff, backup, atomic write, Undo) and all to `<copilot home>\settings.json` only: `notifications` set to `false`, a name added to `disabledMcpServers`, a name added to `disabledSkills`. They are the person's own settings, so `managed.json` records no key for them. A hooks fix writes what "Set up" writes and nothing else.
- Marshell reads from Copilot's `config.json` only `trustedFolders` and `installedPlugins` in the Doctor's readers. The file starts with `//` comment lines, so it is parsed with `config::jsonc`. No other value of it is kept, logged, shown or put in an item.
- Never written for Copilot: `config.json`, `mcp-config.json`, a project's `.mcp.json`, a plugin's folder, `session-state`.
- An item's text never quotes a config file's content: an unreadable file gets `doctor::ext::unreadable`'s sentences. A `skill.invoked` event holds the skill's whole body in `content`; the usage reader takes `name`, `path` and `pluginName` and nothing else.
- Copilot's rules stay in `agents/copilot/` and in Copilot arms. No other CLI's module gets a Copilot branch, and a shared function another CLI uses (`doctor_ext::edits`, `notification_hooks`) is not changed for Copilot.
- **Tests never reach the real Copilot home or binary.** Before `"copilot"` joins `AGENTS`, every state builder the Doctor's tests use pins Copilot: `.with_copilot_home(<temp>)` and `.with_copilot_version(Some("1.0.106".into()))` in `server/doctor.rs` `pinned_with` and in `tests/doctor_api.rs` `state_on`. Tests and live checks never read or write the real `~/.copilot`, `~/.claude`, `~/.codex`, `~/.agents`, `~/.qwen`, `~/.gemini` or `~/.marshell`. Never name a test binary `copilot`.
- No new dependency. No change to `guard.rs`, token handling or file permissions.
- File reads run in `spawn_blocking` (the Doctor's run already does). Never hold a `MutexGuard` across `.await` or file I/O.
- Rust tests run with `-j 4`: `cargo test -p marshell-core -j 4 <filter>`. Run `cargo fmt --all` before every Rust commit. Frontend: `pnpm test src/features/doctor`.
- A new field on `ExtRoots`, `AgentFacts`, `NotifyFacts` or `Caps` breaks struct literals elsewhere: grep for `<Type> {` under `crates/core` and give each literal the new field (or `..Default::default()` where the type has it).
- The CLI's name in every title is "Copilot CLI". Copy, exactly:
  - Retention (`RETENTION_OTHERS`): "Codex, Qwen Code and Copilot CLI have no setting that deletes chats by age, and Antigravity documents none."
  - The doctor line: "Copilot CLI has no doctor command." (the distro wording is whatever `doctor::cli::no_doctor_item` gives the other CLIs).
  - The toast item's title: "Copilot CLI shows its own notifications". Detail: "notifications is on in <settings path>, so Copilot CLI raises a Windows notification beside Marshell's. The fix turns it off there. Marshell's notifications stay."
  - A hooks file that isn't Marshell's: title "<hooks path> isn't Marshell's file any more"; detail "Marshell set Copilot CLI's hooks up in it, and it now holds something Marshell didn't write. The fix replaces the file with Marshell's hooks. Hooks of your own go in another .json file in that folder."; fixable and not pre-checked (Task 2b). Where it can't be replaced, the detail ends after "didn't write." and `cannot_fix` is "The file is read-only.", or for a file that isn't text or can't be read "Marshell can't show this file in a preview. Move it away, then run the check again."
  - A skill Copilot refuses for length: "Its name is over 64 characters." and "Its description is over 1,024 characters."
  - The listing item's title: "Copilot CLI lists <n> skills by name only" ("1 skill" for one). Detail: "Its skill listing is over the 15,000 characters it describes in full, so the skills past that get no description." `cannot_fix`: "Shorten the longest descriptions, or turn off skills you don't use."
  - The built-in server's extra sentence, straight after an item's own detail and before the fix note: " It comes with Copilot CLI."
  - Move hints: "To load it only there, move its entry to mcpServers in <project>\.mcp.json." and "To load it only there, move its folder to <project>\.github\skills."
  - The cost note: "Marshell counts the tools' full definitions. Copilot CLI may load them on demand for some models, which costs less."
  - The progress line: "Checking Claude Code, Codex, Antigravity, Qwen Code and Copilot CLI…"
  - Existing sentences are reused as they are with the CLI's name in them: the hooks titles, "Hooks aren't set up for Copilot CLI.", "Copilot CLI runs your own Notification hook", the unused, broken and cost items.

## Review focus

1. **A `hooks\marshell.json` that the person replaced, emptied to `{}`, saved in another encoding, made read-only, or that doesn't parse, while `managed.json` still names it; the same with `settings.json` missing; and a `settings.json` that doesn't parse beside a good hooks file.** The item names the file at fault, a fix of other picked items still goes through, and only a file that parses and is writable can be replaced, by a fix that is never pre-checked and that Undo takes back byte for byte. Tests in Tasks 2 and 2b.
2. **A `settings.json` with comments, a trailing comma, a BOM and CRLF, where `notifications` is `true`, `"true"`, `1`, missing or `false`, or `disabledMcpServers` is a string or an array that holds a number.** Only a literal `true` gives the toast item. The fix keeps every other byte, Undo gives the file back byte-equal, and a list that isn't an array of strings makes the turn-off item report-only. Tests in Tasks 3 and 4.
3. **An `mcp-config.json` or `.mcp.json` that is not an object, has entries that aren't objects, has a server with neither `command` nor `url`, has no `mcpServers` wrapper, or is larger than 4 MiB; a `config.json` that holds tokens, doesn't parse, or has `trustedFolders` or `installedPlugins` in another shape.** The reader never fails, lists what Copilot itself would load, starts nothing it can't show Copilot trusts, and no value of `config.json` reaches an item. Tests in Tasks 4 and 5.
4. **An `events.jsonl` that is being written, ends in a cut line, holds unknown event types, has an empty `mcpServerName`, or belongs to a folder whose name isn't a UUID; a second scan after a session was resumed.** Only new lines are read the second time, a cut line is read once it is whole, and nothing is counted twice. Tests in Task 6.
5. **Two servers or two skills with one name (user and project, project and plugin, or names that differ only by case).** The turn-off fix never switches off one that works: an item whose exact name another live item has is report-only with the same-name sentence, names that differ by case are two items, and a personal skill Copilot skips because a project's has its name costs nothing and gets no verdict. Tests in Tasks 4 and 5.

## Known limits (planned)

- A signed-in `config.json` and an installed plugin can't be seen on the development PC: nobody is signed in to Copilot CLI. A plugin's lines are built from Copilot's source and tested against files made by hand (decision 18).
- The servers' cost counts full tool definitions; a model for which Copilot loads tools on demand pays less (decision 16).
- The toast item is Windows only and needs Marshell's hooks set up (decision 14).
- A skill whose YAML doesn't parse is refused by Copilot, and Marshell's frontmatter reader is not a YAML parser: it reports such a skill only when the block doesn't close.
- Outside a git repository Copilot looks for project skills in every folder above the working folder, so a project under the profile that is no repository also gets the profile's own `.claude\skills`. Marshell reads the project's folder only.
- Not read: `skillDirectories`, `COPILOT_SKILLS_DIRS`, `--add-dir` roots, Copilot's bundled skills and plugins (they come last in the listing, so they take nothing from the budget of the person's skills), `.claude/commands`, managed settings, and a repository's `.github/copilot/settings*.json` except its `disabledMcpServers` and `disabledSkills`, which only say that an item is already off.
- A project server with a user server's name replaces it in that project. Both are listed and neither is fixable (the same-name rule).
- A `settings.json` of Antigravity's that doesn't parse is still titled with its hooks file; this slice fixes that for Copilot only (an issue is filed at ship).
- A distro's servers are not measured and its history is not scanned, as for the other CLIs.

---

### Task 1: What Copilot 1.0.106 really does for the Doctor (done on 2026-11-24, main thread)

Done before the tasks below were final; they are written from its results.

- Headless runs in `%TEMP%\copilot-6n3-t1` against the fake model (which got a `CALL <tool> [json]` word) and one run of the dev app on the scratch folder `C:\dev\lc-6n3-1` for the toast and a typed `/skill`.
- Findings 1 to 21 are in the research note, part 3, "What 6n3's first task saw". They changed decisions 9, 11, 13, 15 and 16 and added 19 to 21 in PLAN §9.
- Fixtures: `crates/core/tests/fixtures/copilot/transcripts/1.0.106/00000000-0000-4000-8000-000000000274.jsonl` (one MCP call of the user server `scratch-echo`) and `…000000000251.jsonl` (the model invoking the project skill `good-skill`). `cargo test -p marshell-core -j 4 --test fixtures_scrubbed` passes.
- Commit: `test(doctor): record Copilot CLI 1.0.106's MCP call and skill use against the fake model, and what its Doctor facts are.`

### Task 2: Copilot's facts, hooks item and plain lines (core)

**Files:**
- Modify: `crates/core/src/doctor/checks.rs`, `crates/core/src/server/doctor.rs`, `crates/core/src/agents/copilot/hooks.rs`, `crates/core/tests/doctor_api.rs`

**Interfaces:**
- Consumes: `server::agents::copilot_setup_of(&AppState) -> Result<AgentSetup, Fail>`, `copilot_hooks::{hooks_path, settings_path, inspect, Files}`, `agents::copilot::version_gate`, `AppState::{with_copilot_home, with_copilot_version}`.
- Produces: `copilot_hooks::file_problem(text: Option<&str>) -> Option<FileProblem>` with `pub enum FileProblem { DoesNotParse { line: u32, message: String }, NotMarshells }`; `checks::FileState::NotOurs`; `checks::AgentFacts::problem_path: Option<PathBuf>` (the file a `FileState` speaks of when it isn't `path`); `AGENTS` and `ORDER` with `"copilot"` last; `display_name("copilot") == "Copilot CLI"`; item id `hooksfile.copilot`.

- [ ] **Step 1: Pin Copilot in the test states first.** Add `.with_copilot_home(root.join("copilot-home"))` and `.with_copilot_version(Some("1.0.106".into()))` to `pinned_with` in `server/doctor.rs` and to `state_on` in `tests/doctor_api.rs`. Run `cargo test -p marshell-core -j 4 server::doctor` and `--test doctor_api`: still PASS. Only then go on.
- [ ] **Step 2: Failing tests in `checks.rs`.** `display_name("copilot")` is "Copilot CLI"; the order test puts `copilot` after `qwen`; the retention item's detail ends with the new `RETENTION_OTHERS`; a `copilot` fact with `HookInstall::None` and `set_up` gives `hooks.copilot` "Copilot CLI's hooks are gone", Broken and fixable; a fact with `FileState::NotOurs` and `set_up` gives exactly one Copilot item, `hooksfile.copilot`, Broken, with the title, detail and `cannot_fix` from Global constraints, `fixable: false`, whatever its `hooks` value is; `NotOurs` without `set_up` gives only the Info "Hooks aren't set up for Copilot CLI."; an `Unparsable` fact with `problem_path` set titles and opens that path, not `path`. Run `cargo test -p marshell-core -j 4 doctor::checks`: FAIL.
- [ ] **Step 3: `checks.rs`.** Add the arm, the constant, `ORDER: [&str; 5]`, the field and the variant. In `cli_items`, `NotOurs` returns early after its one item (set up) or the setup Info row (not set up): no hooks, notify or toast item is built from such a fact. The parse item uses `problem_path` when it is set. Run: PASS.
- [ ] **Step 4: Failing tests in `copilot/hooks.rs`.** `file_problem(None)` is `None`; Marshell's own file (from `hooks_file`) is `None`; Marshell's entry-less file is `None`; `{}` and a file with one entry of the person's own are `NotMarshells`; `{"hooks": ` is `DoesNotParse` with a line.
- [ ] **Step 5: Implement `file_problem`** over `read_value`, `parse_problem` and the existing `is_marshells`, and make `marshells_hooks` use it. Run `cargo test -p marshell-core -j 4 agents::copilot::hooks`: PASS.
- [ ] **Step 6: Failing tests in `server/doctor.rs`.** Fix the tests the fifth agent breaks (the exact agent lists and counts near 1298, 1305, 1411, 2071 and 2126), then add:
  - `facts_of` gives a fifth fact, `agent: "copilot"`, `path` the hooks file, `set_up` true after an install.
  - With the hooks file deleted after an install: `hooks.copilot` is in the report; `hooks_want_of` for that fact has `statusline: true` when `managed.json` names `/statusLine` in `settings.json`; and with the status line also deleted from `settings.json` by hand, applying `fix_of(["hooks.copilot"])` puts both back.
  - With `{}` written over the hooks file, with a file that isn't UTF-8, and with `settings.json` deleted beside a `{}` hooks file: `hooksfile.copilot` is in the report, `hooks.copilot` isn't, and a fix of another CLI's item picked alone still plans.
  - With the hooks file or `settings.json` read-only: the hooks item is not fixable ("The file is read-only.").
  - With `settings.json` unparsable: the fact's `file` is `Unparsable` and `problem_path` is the settings path.
  - `distro_agent_facts("copilot", …)` over a temp "distro home": files `<home>/.copilot/hooks/marshell.json` and `<home>/.copilot/settings.json`, read as text; Installed after the distro plan's texts are written; `NotOurs` for `{}`.
  - `hooks_want_of` for a Copilot distro fact whose `settings.json` has a `/statusLine` entry in `managed.json` asks for `statusline: true`; for a Qwen distro fact it stays `false`.
  - `cli_doctor_items` gives "Copilot CLI has no doctor command." when Copilot is found.
- [ ] **Step 7: Implement in `server/doctor.rs`.** `AGENTS: [&str; 5]`. Every `_ =>` named in "Checked before planning" becomes explicit arms for `"qwen"` and `"copilot"`; there is no `unreachable!`, and an unknown agent gets `failed_facts` (~407). `facts_from` for Copilot: whenever the settings state isn't `Unparsable`, read the hooks file as text and map `file_problem` to `FileState::Unparsable` (with `problem_path` unset, the hooks file being `path`) or `FileState::NotOurs`; a hooks file that exists and can't be read as UTF-8 is `NotOurs`; a read-only hooks file or settings file is `ReadOnly`; when `settings.json` is the file that doesn't parse, `problem_path` is its path. `distro_agent_facts`: Copilot's two files as text, `copilot_hooks::inspect` with `chain_path: None`, and the same problems. `hooks_want_of`: for Copilot the status line's file is `settings.json` beside the `hooks` folder, and it is asked for on Windows and in a distro when the line is chained or `managed.json` names `/statusLine` there. Run `cargo test -p marshell-core -j 4 server::doctor`: PASS.
- [ ] **Step 8: `tests/doctor_api.rs`.** `AGENTS` gets `"copilot"`; `Env` gets the Copilot hooks and settings paths; `hooks_file` and `seed_own_hooks` get Copilot arms (the person's own hooks go in `<home>/.copilot/hooks/mine.json`, since `marshell.json` is Marshell's whole file); in `set_up_follows_each_clis_real_plans` Copilot's hooks file is deleted instead of emptied; in `hooks_off_reads_as_never_set_up` Copilot is exempt from the `/-` pointer assert (its install has none) and "the person's hooks stay" is checked on `mine.json`, since removal deletes `marshell.json`. Run `cargo test -p marshell-core -j 4 --test doctor_api`: PASS.
- [ ] **Step 9: Commit:** `feat(doctor): Copilot CLI's hooks are checked and fixed, and it gets the retention and doctor lines.`

### Task 2b: The fix that replaces a hooks file that isn't Marshell's (decision 20; worker at `effort: high`)

Task 2 built the item with no fix; the user then chose the fix (decision 20).

**Files:**
- Modify: `crates/core/src/agents/copilot/hooks.rs`, `crates/core/src/server/agents.rs`, `crates/core/src/doctor/checks.rs`, `crates/core/src/server/doctor.rs`

**Interfaces:**
- Produces: `copilot_hooks::install_plan_over(files: &Files, bridge: &Bridge, want: &HookPlanRequest) -> Result<ChangeSet, PlanError>` (as `install_plan`, but a hooks file that parses and isn't Marshell's is replaced with `EditOp::ReplaceText` against its present text); `FileState::NotOurs { cannot_fix: Option<&'static str> }`; `fix_of` arm `Some("hooksfile")`.

- [ ] **Step 1: Failing tests in `copilot/hooks.rs`.** Over `{}` and over a file with the person's own entry: `install_plan_over` gives a `ReplaceText` of the hooks file whose content is `hooks_file(bridge, want)` and whose expected hash is the present text's, with the same settings edits `install_plan` gives over a missing hooks file, and a summary line "Replaces <hooks path> with Marshell's hooks."; over a file that doesn't parse it is `DoesNotParse`; over Marshell's own file and over no file it equals `install_plan`. `install_plan` itself still answers `NotMarshells` for `{}`.
- [ ] **Step 2: Implement** (one shared body with a private `replace_foreign: bool`). Run `cargo test -p marshell-core -j 4 --lib agents::copilot::hooks`: PASS.
- [ ] **Step 3: Failing tests in `checks.rs`.** `NotOurs { cannot_fix: None }` with `set_up`: `hooksfile.copilot`, Broken, `fixable: true`, `unchecked: true`, the detail with the fix sentences. `NotOurs { cannot_fix: Some(reason) }`: not fixable, `cannot_fix` the reason, the detail without the fix sentences. Still the only Copilot item either way.
- [ ] **Step 4: Failing tests in `server/doctor.rs`.** With `{}` over the hooks file after an install: the item is fixable; `fix_of(["hooksfile.copilot"])` plans a `ReplaceText` of the hooks file; applying it leaves no `hooksfile.copilot` and no `hooks.copilot` item and the status line in place; Undo gives the hooks file back byte-equal (`{}`). The same over a temp distro home. A hooks file that isn't UTF-8: not fixable ("Marshell can't show this file in a preview. Move it away, then run the check again."), `fix_of` refuses its id, and another CLI's item picked alone still plans. A read-only hooks file or settings file beside a `{}` hooks file: the item shows, not fixable, "The file is read-only.". The Agents page's Set up plan for Copilot still refuses `{}`.
- [ ] **Step 5: Implement.** `server/agents.rs`: the Copilot arms of `hook_install_changes` and `distro_install` share their bodies with two new `pub(crate)` entry points that call `install_plan_over` (no behaviour change for the existing callers). `server/doctor.rs`: `copilot_hooks_problem` fills `cannot_fix`; `fix_of` gets `Some("hooksfile")`, Copilot only, with `hooks_want_of` (status asked for). Run `cargo test -p marshell-core -j 4 --lib server::` and `--test doctor_api`: PASS.
- [ ] **Step 6: Commit:** `feat(doctor): a Copilot CLI hooks file that isn't Marshell's any more can be replaced by a fix the person picks.`

### Task 3: Copilot's own toasts and notification hooks (core)

**Files:**
- Create: `crates/core/src/agents/copilot/notify.rs` (and `pub mod notify;` in `agents/copilot/mod.rs`)
- Modify: `crates/core/src/doctor/checks.rs`, `crates/core/src/server/doctor.rs`

**Interfaces:**
- Consumes: Task 2's Copilot facts.
- Produces: `notify::toasts_on(settings: &Value) -> bool`; `notify::own_hooks(files: &[(PathBuf, Value)], settings: Option<&Value>) -> Vec<String>`; `checks::NotifyFacts { toasts: bool, settings: PathBuf, .. }`; item id `toasts.copilot`.

Copilot's hook files hold `{"version": 1, "hooks": {"<event>": [entry, …]}}`. Marshell's own entries are `{"type": "command", "exec": …, "args": […]}` and are told apart with `agents::common::is_ours`.

- [ ] **Step 1: Failing tests in `notify.rs`.** `toasts_on` is true only for `{"notifications": true}` (false for `"true"`, `1`, missing, `false`). `own_hooks` returns, in file-name order then file order, the command of every entry under the event `notification` that `is_ours` doesn't claim: from each hooks file value and from `settings.hooks.notification`; a flat entry and a Claude-style group (`{matcher, hooks: [...]}`) both count; the command text is the first of `powershell`, `command`, `bash`, else `exec` and its `args` joined with spaces, else the `url` of a `type: "http"` entry; a `type: "prompt"` entry and one with none of these are skipped; a file with `disableAllHooks: true` gives none.
- [ ] **Step 2: Implement `notify.rs`.** Run `cargo test -p marshell-core -j 4 agents::copilot::notify`: PASS.
- [ ] **Step 3: Failing tests in `checks.rs`.** With `banners_on`, a Windows Copilot fact with `notify.toasts` and hooks `Installed` gives `toasts.copilot`: Broken, fixable, checked by default (`unchecked: false`), the exact title and detail, `path` the settings file. The same with hooks `Partial` or `Orphaned` (Marshell's hooks are set up, and their own item says what is wrong with them). No item with banners off, with `HookInstall::None`, with a distro, or with an `Unparsable` or `NotOurs` file. A read-only settings file makes it not fixable with "The file is read-only." Copilot `hook_commands` give "Copilot CLI runs your own Notification hook" with the commands joined as the other CLIs' are.
- [ ] **Step 4: Implement in `checks.rs`.** Run: PASS.
- [ ] **Step 5: Failing tests in `server/doctor.rs`.** `notify_facts` for Copilot reads every `*.json` in `<home>/hooks` except `marshell.json` (a file that doesn't parse is skipped) and the settings value. After a setup, with `"notifications": true` in a `settings.json` that has a comment, a trailing comma, a BOM and CRLF: the report holds `toasts.copilot`; `fix_of(["toasts.copilot"])` plans one `JsonSet` of `/notifications` to `false` on the settings file with `Applies::NewSessions`; applying it changes only that value's bytes; `managed.json` gains no entry for `/notifications`; undo gives the file back byte-equal. `fix_of(["toasts.copilot", "hooks.copilot"])` with the hooks file deleted applies both in one go. `notifications: "true"` gives no item. In a distro fact `toasts` is false whatever the file says.
- [ ] **Step 6: Implement in `server/doctor.rs`.** `notify_facts` gets the Copilot arm (the shared `notification_hooks` is not changed); `fix_of` gets `Some("toasts")` (the edit is built against the file's text now, as `retention_edit` is); `is_own_edit` also matches the pointer `/notifications` when the path is Copilot's settings file. Run `cargo test -p marshell-core -j 4 server::doctor` and `--test doctor_api`: PASS.
- [ ] **Step 7: Commit:** `feat(doctor): Copilot CLI's own notifications are a Broken item whose fix turns them off, and its notification hooks are reported.`

### Task 4: Copilot's MCP servers (core; worker at `effort: high`)

**Files:**
- Create: `crates/core/src/agents/copilot/extensions.rs` (and `pub mod extensions;`)
- Modify: `crates/core/src/doctor/ext.rs`, `crates/core/src/doctor/usage.rs`, `crates/core/src/server/doctor_ext.rs`, `crates/core/src/server/doctor_measure.rs`, and the tests in `crates/core/src/server/doctor.rs` that count inventories (near 2071 and 2126)

**Interfaces:**
- Consumes: `doctor::ext::{ExtRoots, Inventory, Server, Switch, NoSwitch, Origin, Launch, Inherit, Expand, CostRule, Listing, UsedAs, Caps, Hints, unreadable}`, `config::jsonc`.
- Produces: `copilot::extensions::inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory` (servers in this task, `skills` empty until Task 5); `ExtRoots { copilot_dir: PathBuf, copilot_home_set: bool, copilot_offline: bool, .. }`; `Origin::BuiltIn`; `Expand::DollarDefault`; `pub const GITHUB_SERVER: &str = "github-mcp-server"`; `extensions::trusted(folder: &Path, trusted_folders: &[String]) -> bool`; a private `Files` cache and `disabled(list_pointer) -> Result<Vec<String>, ()>` that Task 5 reuses.

Read `agents/qwen/extensions.rs` first: Copilot's reader has the same parts (a `Files` cache with `unreadable`, `servers_in`, the same-name pass, `inventory`).

- [ ] **Step 1: Engine additions, tests first.**
  - `ext.rs`: `Origin::BuiltIn` ("comes with the CLI, and is judged and switched like the person's own"). `ORDER` gets `"copilot"`. A test: a `BuiltIn` server with a switch that is unused gives `mcpunused.copilot.<key>`, Waste, fixable, whose detail starts with "It still starts with every Copilot CLI session. It comes with Copilot CLI." (the fix note follows).
  - `usage.rs`: the `judge` closure appends the sentence for `Origin::BuiltIn` before `fixability` adds the fix note; the broad-scope verdict stays `Origin::User` only.
  - `doctor_measure.rs`: `Expand::DollarDefault` expands `$VAR`, `${VAR}` and `${VAR:-default}` (both the default branch near 112 and the `$VAR` branch near 127); extend `ready_of_builds_the_env_and_expands` or add a test beside it covering the three forms and an unset variable with and without a default.
- [ ] **Step 2: Roots.** `roots_under(home, …)` sets `copilot_dir: home.join(".copilot")` and both flags false. `windows_roots` sets `copilot_dir` from the state's Copilot home (the one `copilot_setup_of` uses, so a test's pinned home is honoured), `copilot_home_set` when `COPILOT_HOME` is set and not blank, `copilot_offline` when `COPILOT_OFFLINE` is `true`. `read_inventory` gets the `"copilot"` arm.
- [ ] **Step 3: Failing tests for servers** (temp roots; each asserts the `Server` fields named):
  - A user server in `mcp-config.json` under `mcpServers`, in a file with a `//` comment and a trailing comma: `Origin::User`, `file` the config file, `Transport::Stdio`, `switch` `JsonAdd { file: <copilot_dir>/settings.json, pointer: "/disabledMcpServers", name }`, `used_as: UsedAs::Server(name)`, `launch` Ok with `Expand::DollarDefault`, `Inherit::All`, the entry's `args`, `env`, `cwd`, and `timeout_secs` 30 (or the entry's `timeout` in ms, rounded up to seconds).
  - `url` gives `Transport::Remote` and `Err(NotMeasured::Remote)`; an entry with neither, or that isn't an object, gives `Transport::Nothing`; a name that is empty or holds a control character is skipped (Copilot skips it).
  - `off` is true when the name is in the user's `disabledMcpServers` (exact match: `Echo` doesn't turn `echo` off), and for a project server also when it is in that project's `.github/copilot/settings.json` or `settings.local.json`.
  - A project's `.mcp.json`, with the `mcpServers` wrapper and without it: `Origin::Project(project)`, the same user-file switch. `.github/mcp.json` is never read. `launch` is Ok only when `trusted(project, trustedFolders)`, else `Err(NotMeasured::NotApproved)`.
  - `trusted`: true when the folder equals a listed folder or is under one, compared component by component after `\` to `/`, a trailing separator cut, and lower case (`C:/Dev/Proj/` trusts `c:\dev\proj\sub`; `c:\dev\pro` doesn't trust `c:\dev\proj`). `trustedFolders` that isn't an array of strings, and a `config.json` that doesn't parse, trust nothing.
  - Cost: `CostRule` with `Listing::Full { prefix: "<name>-", desc_cap: None }`; `only` is the entry's `tools` unless it is `["*"]` or absent; `except` is `excludeTools`. `Inventory::cost_note` is the sentence from Global constraints.
  - The built-in: unless `copilot_offline` or a configured server (user or project) has that name, there is a `Server { name: GITHUB_SERVER, origin: Origin::BuiltIn, file: <settings.json>, transport: Remote, switch: Ok(JsonAdd …), launch: Err(Remote), used_as: UsedAs::Server(GITHUB_SERVER), .. }`, `off` when the name is in `disabledMcpServers`. `computer-use` is never listed.
  - Two live servers with exactly one name, one user and one project: both get `Err(NoSwitch::SameName)`; when one is already off the other keeps its switch; names that differ only by case are two servers, each with its switch.
  - `disabledMcpServers` that is a string, or an array that holds a number: every server's switch is `Err(NoSwitch::CantEdit(<settings.json>))`, and none is `off`.
  - A `mcp-config.json` that doesn't parse, isn't UTF-8 or is over 4 MiB is in `unreadable` with the matching sentence, and gives no server.
  - `format!("{inv:?}")` over a `config.json` with a planted `"token": "SECRET-6N3"` doesn't contain `SECRET-6N3`.
- [ ] **Step 4: Implement** the servers half of `extensions.rs`. Run `cargo test -p marshell-core -j 4 agents::copilot::extensions`: PASS.
- [ ] **Step 5: Server-side tests in `doctor_ext.rs`.** With a Copilot home under `with_ext_home`: `inventories` holds a Copilot inventory; a broken user server gives a fixable `mcp.copilot.<key>` whose fix adds its name to `/disabledMcpServers` in a `settings.json` with a comment and a trailing comma, keeping the other bytes, and Undo restores it byte-equal; two picked servers give one edit of the list; a distro inventory reads `<distro home>/.copilot` at user level only. Run `cargo test -p marshell-core -j 4 server::doctor_ext` and `server::doctor`: PASS.
- [ ] **Step 6: Commit:** `feat(doctor): Copilot CLI's MCP servers are listed, costed and turned off through disabledMcpServers.`

### Task 5: Copilot's skills and plugins (core)

**Files:**
- Modify: `crates/core/src/agents/copilot/extensions.rs`, `crates/core/src/doctor/ext.rs`, `crates/core/src/server/doctor_ext.rs` (tests)

**Interfaces:**
- Consumes: Task 4's `Files`, `disabled` and `inventory`; `doctor::ext::{Skill, SkillState, SkillUse, Caps, skill_files, created_ms}`; `doctor::frontmatter::{read, Front}`.
- Produces: `SkillState::TooLong(&'static str, usize)` (field, limit); `Caps::listing_named_over: Option<usize>`; item id `skilllisting.copilot`; `inventory` now fills `skills`.

- [ ] **Step 1: Engine additions, tests first, in `ext.rs`.**
  - `SkillState::TooLong(field, limit)`: Broken "<CLI>'s skill <name> doesn't load" with "Its <field> is over <limit> characters." (the limit with a thousands comma), fixable through its switch like the other Broken skills.
  - `Caps::listing_named_over: Some(cap)` with `n > 0` listed skills (not off, not hidden, state Ok, not Bundled) that have `name_only`: one `skilllisting.<agent>` Waste item with the title, detail and `cannot_fix` from Global constraints, no fix. With `n == 0`, or the cap `None`, no item. The four other CLIs' readers leave it `None`, and their listing tests still pass.
- [ ] **Step 2: Failing tests for skills** in `extensions.rs`.
  - Folders read: `<copilot_dir>/skills`; `<home>/.agents/skills` unless `copilot_home_set`; per project `.github/skills`, `.agents/skills`, `.claude/skills`. `~/.claude/skills` is never read.
  - The name is the frontmatter's `name`, else the folder's name. State: no closing `---` block is `NoBlock`; a name over 64 characters is `TooLong("name", 64)`; a name not matching "starts with an ASCII letter or digit, then ASCII letters, digits, `-`, `_`, `.`, `:` and spaces" is `BadName`; a description over 1,024 characters is `TooLong("description", 1024)`; `Front::Unclear` is `Ok`. A skill with no `description` is `Ok`.
  - `listed_chars` is the description's length, or for a skill with no description the length of its body's first non-empty line. `caps.per_skill` is `None` (Copilot cuts nothing).
  - `off` when `disabledSkills` holds exactly the name (`Good-Skill` doesn't turn `good-skill` off), in the user's settings or, for a project's skill, that project's `.github/copilot/settings*.json`. `switch` is `JsonAdd { pointer: "/disabledSkills", name }` on the user's settings, or `CantEdit` when the list isn't an array of strings. `used_as: [SkillUse::File(<SKILL.md>), SkillUse::Name(name)]`. `hidden` when `disable-model-invocation` is true.
  - A personal skill whose exact name a project's skill has is `off: true` (Copilot skips it); two project skills of one name in two projects each keep their place and both get `Err(NoSwitch::SameName)` when both are live.
  - The budget: `caps.listing_named_over` is `Some(15_000)`. Walking the listed skills (not off, not hidden, state Ok) in Copilot's order, project skills by name and then personal ones by name, each costing `name + listed_chars + 91`, from a start of 19: every skill from the first whose cost would take the sum past 15,000 on is `name_only`. A test with 200 skills of 8-character names and 15-character descriptions describes 131 and names 69, which is what Copilot did (research note, finding 6); one with 10 small skills names none.
  - Hints are the two sentences from Global constraints.
- [ ] **Step 3: Failing tests for plugins.** `config.json` (with its two `//` lines on top) holding `installedPlugins: [{name, marketplace, enabled, cache_path?}]`: a plugin that is `enabled` and not `false` under `enabledPlugins["<name>@<marketplace>"]` in the user's settings has its folder read (`cache_path`, else `<copilot_dir>/installed-plugins/<marketplace>/<name>`): servers from the manifest's `mcpServers` (an object, or a relative path to a file) and from `.mcp.json` in it; skills from the manifest's `skills` folder, else `skills`; the manifest is the first of `plugin.json`, `.plugin/plugin.json`, `.github/plugin/plugin.json`, `.claude-plugin/plugin.json`. Each item is `Origin::Plugin(name)` with `Err(NoSwitch::Plugin(name))`, a plugin skill is named `<plugin>:<skill>`, and a plugin server's launch is `Err(NotMeasured::NotApproved)`. A `cache_path` outside `<copilot_dir>/installed-plugins`, and a manifest path that leaves the plugin's folder (`..`, absolute), are not followed. `installedPlugins` in another shape gives no plugin item and no failure.
- [ ] **Step 4: Implement.** Run `cargo test -p marshell-core -j 4 agents::copilot::extensions` and `doctor::ext`: PASS.
- [ ] **Step 5: Server-side tests in `doctor_ext.rs`.** A skill with a 1,025-character description gives a fixable `skill.copilot.<key>` whose fix adds its name to `/disabledSkills`; a picked server and a picked skill give one edit per list in the same `settings.json`; a skill with no description gives no Broken item. Run `cargo test -p marshell-core -j 4 server::doctor_ext`: PASS.
- [ ] **Step 6: Commit:** `feat(doctor): Copilot CLI's skills and plugins are listed, with the skills it refuses, its listing budget and the disabledSkills fix.`

### Task 6: Copilot's usage scan (core)

**Files:**
- Create: `crates/core/src/agents/copilot/usage.rs` (and `pub mod usage;`)
- Modify: `crates/core/src/server/doctor_usage.rs`

**Interfaces:**
- Consumes: `doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines, norm_path}`, `ExtRoots::copilot_dir`, Task 1's fixtures.
- Produces: `copilot::usage::scan(roots: &ExtRoots, store: &AgentStore) -> Scan`.

Read `agents/qwen/usage.rs` first: the shape is the same.

- [ ] **Step 1: Failing tests.**
  - History files are `<copilot_dir>/session-state/<folder>/events.jsonl` for every folder whose name is a UUID, sorted; another folder name (`.session-operation-locks`) is skipped.
  - Over fixture `…274`: exactly one `Used` event, `UseKey::Server("scratch-echo")`, with the time of its `tool.execution_start` and the `cwd` of the file's `session.start` (`data.context.cwd`). The server's name is `data.mcpConfigServerName`, else `data.mcpServerName`. The `assistant.message` line that also carries `mcpServerName` gives nothing.
  - Over fixture `…251`: exactly one `Used` event, `UseKey::SkillFile(norm_path(data.path))` from `skill.invoked`. A `skill.invoked` with no `path` gives `UseKey::SkillName(name)`, with `<pluginName>:` in front when `pluginName` is a non-empty string (it is `null` in the fixture).
  - A `tool.execution_start` with no server name, or an empty one, gives nothing. A `session.resume` with another `context.cwd` changes the cwd of the events after it.
  - The cursor: a second scan with the store of the first reads nothing new; after lines are appended it gives only their events; a last line without a newline is not read until it has one; `first_ms` is the first timestamp of the file.
  - A `session-state` folder that can't be listed gives `unreadable: Some("Its history folder can't be listed.")` and keeps the old cursors.
  - No `Event` or `Scan` debug output holds the fixture's skill body ("Body of the skill.").
  - In `doctor_usage.rs`: `scan_all` with a Copilot inventory and fixture `…274` copied into the Copilot home stores a use under `agents["copilot"]`, and an unused user server first seen over 30 days ago becomes `mcpunused.copilot.<key>` in the report.
- [ ] **Step 2: Implement** `usage.rs` and the `"copilot"` arm in `scan_all`. Run `cargo test -p marshell-core -j 4 agents::copilot::usage` and `server::doctor_usage`: PASS.
- [ ] **Step 3: Commit:** `feat(doctor): Copilot CLI's event logs feed the 30-day usage check.`

### Task 7: Measure and the page's copy

**Files:**
- Modify: `crates/core/src/server/doctor_measure.rs` (tests only, unless one fails), `src/features/doctor/DoctorPage.tsx`, `src/features/doctor/DoctorPage.test.tsx`, `src/features/doctor/fixtures.ts`

- [ ] **Step 1: A Measure test.** In `doctor_measure.rs`'s tests, a Copilot inventory with a user server (the test's stub server), a remote one, the built-in and an untrusted project server: the stub is measured, and `mcpcost.copilot` counts 1 measured and "Not measured: 2 remote servers, 1 that Copilot CLI hasn't approved", followed by the cost note. Run `cargo test -p marshell-core -j 4 doctor_measure`: PASS (fix what fails).
- [ ] **Step 2: Frontend, test first.** `DoctorPage.test.tsx`: the progress line reads "Checking Claude Code, Codex, Antigravity, Qwen Code and Copilot CLI…", and the fixture's `cli.copilot` row "Copilot CLI has no doctor command." renders under Info. Add the row to `fixtures.ts` beside `cli.qwen`, with the same detail that row has, and change the line in `DoctorPage.tsx`. Run `pnpm test src/features/doctor`: PASS. Run `pnpm build`: passes.
- [ ] **Step 3: Commit:** `feat(doctor): the Doctor page names Copilot CLI, and Measure covers its servers.`

### Task 8: Issue #154, `/clear` in a Copilot WSL tab (core; worker at `effort: high`)

The failing tests of Step 1 are the evidence decision 4 asks for; Task 9 confirms the fix in a real distro. This task depends on no other and may run first.

**Files:**
- Modify: `crates/core/src/server/hook.rs`, `crates/core/tests/hook.rs` (the Copilot end-hold tests near 2296 to 2520 are the model)

**Interfaces:**
- Consumes: `copilot::is_requested_end`, `copilot_session`, `hold_copilot_end`, `AppState::{with_copilot_end_hold, copilot_end_hold}`, `registry().clear_hook_agent`.

- [ ] **Step 1: Failing tests in `tests/hook.rs`.** Build the state as the WSL tests do but for Copilot: `state_with(Some(KNOWN), vec![sleeper_profile(), wsl_profile()])` with `.with_wsl_version(Some("1.0.106".into()))` (Copilot's version gate knows only 1.0.106, and the default pinned WSL version is Claude's), Copilot's home and version, and a short `with_copilot_end_hold`. Reserve the tab with `profile_id: "wsl-ubuntu"`. Each test first asserts the row is live with its agent. Then:
  - A tab where a hook marked Copilot (typed into the distro's shell): a requested `sessionEnd`, then within the hold the status line naming a new session id (`copilot_statusline_of("idle", COPILOT_B)`, the real `/clear` order). The row keeps its agent mark throughout and never reads Ended.
  - A tab launched as Copilot (the row has an agent): the same sequence never reads Ended.
  - With no newer session inside the hold: after it, the mark clears (first kind) and the row reads Ended (second kind), as today.
  - An end that wasn't requested is not held, as on Windows.
  If the first two can't be made to fail, the fault isn't there: stop, report, and leave #154 for the live check.
- [ ] **Step 2: Implement in `server/hook.rs`.** For a requested Copilot end on a WSL tab, skip the immediate `clear_hook_agent` and `left_wsl` (157 to 163), and hold: give `hold_copilot_end` an `on_wsl: bool` (or write a second function beside it). After the wait, and only if `copilot_session` doesn't say Stale, the WSL path runs `clear_hook_agent` and the `launched` test and folds only when `!(cleared || !launched)`, which is what the branch did at once before. Nothing is held across the wait. The registry's ended session is then set only after the hold; say so in the function's comment. Run `cargo test -p marshell-core -j 4 --test hook`: PASS.
- [ ] **Step 3: Commit:** `fix(status): /clear in a Copilot CLI tab inside WSL keeps the row's agent and doesn't read Ended (#154).`

### Task 9: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder and `-CopilotFake`. Say first that the dev window takes the foreground when it starts. Over CDP, on the Doctor page:
  - With Copilot's hooks set up: no Copilot item but its cost and usage lines. Delete `hooks\marshell.json`: "Copilot CLI's hooks are gone" is pre-checked; Fix shows the preview, applies, and a new Copilot tab shows live status; Undo last fix removes the file again.
  - Write `{}` over the hooks file: the "isn't Marshell's file any more" item shows, not pre-checked; picked, its fix previews the replacement, applies, and Undo last fix gives `{}` back.
  - `"notifications": true` in the scratch `settings.json` (with a comment in the file): the toast item shows pre-checked; Fix sets it to `false` and keeps the comment; with Marshell's banners switched off in Settings the item is gone.
  - A hooks file `hooks\mine.json` with a `notification` entry: the Info item names its command.
  - "Run CLI doctors": "Copilot CLI has no doctor command."
  - A scratch `mcp-config.json` with one working stdio server (Task 1's `echo-mcp.mjs`), one whose command is missing and one remote; a scratch skill with a 1,025-character description and one with no description. The missing server and the long skill are Broken, the other skill isn't; Fix adds the names to the two lists; `copilot mcp list --json` and `copilot skill list --json` in the scratch home show both off.
  - "Measure MCP servers": the working server is measured and the cost line names it.
  - Run `CALL <server>-echo` in a Copilot tab, then check again: the use is stored (`<scratch>\mh\doctor\usage.json` names it under `copilot`).
  - The built-in server: `-CopilotFake` sets `COPILOT_OFFLINE`, so it is not listed there; record that, and that the unit tests cover the listed case.
  - The page with the keyboard alone (Tab to each Copilot item, Space on its checkbox, Enter on Fix) and in emulated forced colours.
- [ ] **Step 2: WSL.** With the user's OK and the recipe of 6n2's live check (`/tmp` scratch prefix): after "Set up in <distro>" the Doctor shows no Copilot item for the distro; with the distro's hooks file deleted the "in <distro>" item shows and its fix restores the hooks file and the status line; a distro server is "Not measured: in <distro>." Then #154: type `/clear` in a Copilot tab started from the launcher and in one where `copilot` was typed into the distro's shell, sampling the row every 50 ms for two seconds: it never reads Ended and never loses its agent. Ask before deleting the `/tmp` scratch.
- [ ] **Step 3:** Write `docs/research/2026-11-24-copilot-doctor-live-check.md`: each check with its evidence, findings numbered.
- [ ] **Step 4:** Stop the check by PID, after listing the tree and looking for agent CLIs the person started. Ask before removing the scratch folders (`%TEMP%\copilot-6n3-t1`, `C:\dev\lc-6n3-1`, `C:\dev\lc-6n3-facts` and this task's).
- [ ] **Step 5:** Commit `docs(research): record slice 6n3's live check`. Then the final whole-branch review (`marshell-reviewer`, Opus), one fix pass, and `marshell-ship` (the PR closes #154; file the Antigravity "wrong file named" issue).
