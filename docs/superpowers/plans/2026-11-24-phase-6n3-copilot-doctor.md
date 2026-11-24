# Slice 6n3: Copilot CLI in the Doctor

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor checks Copilot CLI wherever it checks Claude Code, Codex, Antigravity and Qwen Code: hooks, the retention sentence, the "no doctor command" line, duplicate notifications (with a fix for Copilot's own toasts), MCP servers and skills with their turn-off fixes, the 30-day usage scan and Measure, on Windows and in WSL distros. Issue #154 is closed on the way.

**Architecture:** The Doctor's engine (`crates/core/src/doctor/`) knows no CLI's rules. Each CLI has arms in `server/doctor*.rs` and two readers, `agents/<cli>/extensions.rs` (inventory) and `agents/<cli>/usage.rs` (history scan). Copilot gets an arm where `"qwen"` has one, and every `_ =>` fallback that today sends an unknown agent to Qwen's code becomes an explicit arm. Copilot's two new readers follow Qwen's files in shape. Three small engine additions carry what Copilot has and the others don't: a toast fact with its own fixable item, `Origin::BuiltIn` for a server that comes with the CLI but is judged like the person's own, and `Expand::DollarDefault`.

**Tech Stack:** Rust (axum, serde_json, `config::jsonc`, toml_edit untouched), React + TypeScript, Vitest through `renderToStaticMarkup`. No protocol type changes, so `src/generated/` stays as it is.

**Spec:** `docs/PLAN.md` §9, "Slice 6n3 decisions (2026-11-24)", which rests on "Slice 7a decisions", "Slice 7b decisions" and "Slice 7c decisions". Evidence: `docs/research/2026-11-06-copilot-cli.md`, part 3 (and part 1 Q1, Q7 for hooks and notifications). Task 1 settles part 3's "Open after the static read" list.

**Checked before planning (2026-11-24, on `origin/main` 8aca488a, Copilot CLI 1.0.106):**
- No Doctor file names Copilot. `server/doctor.rs:46` `AGENTS`, `doctor/checks.rs:14` `ORDER` and `doctor/ext.rs:11` `ORDER` hold four agents, and `tests/doctor_api.rs:17` `AGENTS` too.
- Fallbacks that would run Qwen's code for Copilot: `server/doctor.rs` `facts_of` (`_ => qwen_setup_of`, ~284), `distro_agent_facts` (`_ => .qwen/settings.json`, ~518, and `_ => qwen::hooks::inspect_for`, ~591), `version_gate` (~676). `doctor_items_of` (~159) answers `no_doctor_item` for any agent but Claude Code and Codex, which is right for Copilot.
- `server/doctor_ext.rs::read_inventory` (~117) and `server/doctor_usage.rs::scan_all` (~48) match four agents and skip any other.
- `server/agents.rs::copilot_setup_of` (~1143) gives an `AgentSetup` with `hooks_path` (`<home>/hooks/marshell.json`) and `settings_path`; `settings_state` is the state of `settings.json` only. `agents/copilot/hooks.rs::inspect` reads a hooks file that doesn't parse as holding nothing, and `install_plan` refuses one that doesn't parse or isn't Marshell's (`PlanError::DoesNotParse`, `PlanError::NotMarshells`).
- `server/doctor.rs::hooks_want_of` (~885) keeps a status line only on Windows and looks for `/statusLine` in the hooks file or in Antigravity's settings. Copilot's is in `<home>/settings.json`, and a distro install writes one too.
- `doctor/usage.rs` and `doctor/measure.rs` leave `Origin::Bundled` items out of every verdict, so a built-in server that is to be judged can't be `Bundled`.
- `server/doctor_measure.rs` expands `Expand::Braces` (`${VAR}`, `${VAR:-default}`) and `Expand::Dollar` (`$VAR`, `${VAR}`). Copilot's entries take all three forms.
- `server/hook.rs:259` skips Copilot's 500 ms end hold for a WSL tab (`!on_wsl`), and the WSL branch above it (~157) clears the tab's agent mark at once on a `sessionEnd`.
- Frontend: only `DoctorPage.tsx:101` (the progress line), `fixtures.ts` and `DoctorPage.test.tsx` name the CLIs.

## Global constraints

- `<copilot home>` is `agents::copilot::home`: `COPILOT_HOME` when set and not blank, else `<user home>\.copilot`. In a distro it is the distro's own `~/.copilot`.
- New writes, all through the config service (preview, diff, backup, atomic write, Undo) and all to `<copilot home>\settings.json` only: `notifications` set to `false`, a name added to `disabledMcpServers`, a name added to `disabledSkills`. They are the person's own settings, so `managed.json` records no key for them. A hooks fix writes what "Set up" writes and nothing else.
- Marshell reads from Copilot's `config.json` only `trustedFolders`, `installedPlugins` and `lastLoggedInUser` (login and host, through `agents::copilot::last_account`). No other value of that file is kept, logged, shown or put in an item.
- Never written for Copilot: `config.json`, `mcp-config.json`, a project's `.mcp.json`, `.github\`, a plugin's folder, `session-state`.
- An item's text never quotes a config file's content: an unreadable file gets `doctor::ext::unreadable`'s sentences.
- Copilot's rules stay in `agents/copilot/` and in Copilot arms. No other CLI's module gets a Copilot branch.
- Tests and live checks never read or write the real `~/.copilot`, `~/.claude`, `~/.codex`, `~/.agents`, `~/.qwen`, `~/.gemini` or `~/.marshell`. Rust tests use temp folders with `AppState::with_copilot_home` and `with_ext_home`. Never name a test binary `copilot`.
- No new dependency. No change to `guard.rs`, token handling or file permissions.
- File reads run in `spawn_blocking` (the Doctor's run already does). Never hold a `MutexGuard` across `.await` or file I/O.
- Rust tests run with `-j 4`: `cargo test -p marshell-core -j 4 <filter>`. Run `cargo fmt --all` before every Rust commit. Frontend: `pnpm test src/features/doctor`.
- The CLI's name in every title is "Copilot CLI". Copy, exactly:
  - Retention (`RETENTION_OTHERS`): "Codex, Qwen Code and Copilot CLI have no setting that deletes chats by age, and Antigravity documents none."
  - The doctor line: "Copilot CLI has no doctor command." (in a distro: "Copilot CLI in <distro> has no doctor command.")
  - The toast item's title: "Copilot CLI shows its own notifications". Detail: "notifications is on in <settings path>, so Copilot CLI raises a Windows notification beside Marshell's. The fix turns it off there. Marshell's notifications stay."
  - A hooks file that isn't Marshell's: title "<hooks path> isn't Marshell's file any more"; detail "Marshell set Copilot CLI's hooks up in it, and it now holds entries Marshell didn't write."; `cannot_fix` "Marshell writes only a hooks file it made. Move the file away, then run the check again."
  - The built-in server's extra sentence, after an item's detail: " It comes with Copilot CLI."
  - Move hints: "To load it only there, move its entry to mcpServers in <project>\.mcp.json." and "To load it only there, move its folder to <project>\.github\skills."
  - The cost note: "Marshell assumes Copilot CLI loads tool definitions on demand, which it decides by model."
  - The progress line: "Checking Claude Code, Codex, Antigravity, Qwen Code and Copilot CLI…"
  - Existing sentences are reused as they are with the CLI's name in them: the hooks titles, "Hooks aren't set up for Copilot CLI.", "Copilot CLI runs your own Notification hook", the unused, broken and cost items.

## Review focus

1. **A `hooks\marshell.json` that the person replaced, emptied to `{}`, or that doesn't parse, while `managed.json` still names it; and a `settings.json` that doesn't parse beside a good hooks file.** The item says which file and why, is not fixable, and a fix of other picked items still goes through. Tests in Task 2.
2. **A `settings.json` with comments, a trailing comma, a BOM and CRLF, where `notifications` is `true`, `"true"`, `1`, missing or `false`, or `disabledMcpServers` is a string or holds non-strings.** Only a literal `true` gives the toast item. The fix keeps every other byte, Undo gives the file back byte-equal, and a list that isn't an array of strings makes the turn-off item report-only. Tests in Tasks 3 and 4.
3. **An `mcp-config.json` or `.mcp.json` that is not an object, has entries that aren't objects, has a server with neither `command` nor `url`, has no `mcpServers` wrapper, or is larger than 4 MiB; a `config.json` that holds tokens, doesn't parse, or has `trustedFolders` or `installedPlugins` in another shape.** The reader never fails, lists what Copilot itself would load, starts nothing it can't show Copilot trusts, and no value of `config.json` reaches an item. Tests in Task 4.
4. **An `events.jsonl` that is being written, ends in a cut line, holds unknown event types, has an empty `mcpServerName`, or belongs to a folder whose name isn't a UUID; a second scan after a session was resumed.** Only new lines are read the second time, a cut line is read once it is whole, and nothing is counted twice. Tests in Task 5.
5. **Two servers or two skills with one name (user and project, project and plugin, or names that differ only by case).** The turn-off fix never switches off one that works: the item is report-only with the same-name sentence, and a lower-priority skill Copilot skips as a duplicate costs nothing and gets no verdict. Tests in Task 4.

## Known limits (planned)

- The built-in GitHub server, a signed-in `config.json` and an installed plugin can't be seen on the development PC: nobody is signed in to Copilot CLI. Their lines are built from Copilot's schema and source and tested against files made by hand (decisions 15 and 18).
- The servers' cost assumes tools load on demand (decision 16).
- The toast item is Windows only and needs Marshell's hooks set up (decision 14).
- Not read: skills in folders above a project's root, `skillDirectories`, `COPILOT_SKILLS_DIRS`, `--add-dir` roots, Copilot's bundled skills and plugins, `.claude/commands`, managed settings, and a repository's `.github/copilot/settings*.json` except its `disabledMcpServers` and `disabledSkills`, which only say that an item is already off.
- A distro's servers are not measured and its history is not scanned, as for the other CLIs.

---

### Task 1: What Copilot 1.0.106 really does for the Doctor (main thread, with a decision gate)

This task runs in the main thread. Copilot runs headless from a plain shell with every variable pointing into the fresh literal scratch folder `%TEMP%\copilot-6n3-t1`, as 6n2's first task did (`docs/superpowers/plans/2026-11-20-phase-6n2-copilot-surfaces.md`, Task 1, Step 1 has the variables). Nothing is deleted without asking.

**Files:**
- Create: `crates/core/tests/fixtures/copilot/transcripts/1.0.106/<scrubbed id>.jsonl` (one with an MCP call, one with a skill use)
- Modify: `.claude/skills/marshell-live-check/fake-openai.mjs` only if it has no word that calls a named tool; `docs/research/2026-11-06-copilot-cli.md` (a "What 6n3's first task saw" section in part 3); this plan and PLAN §9 where a finding differs

- [ ] **Step 1: Scratch and fake model.** Start `fake-openai.mjs` on a free loopback port. Set `COPILOT_HOME=<scratch>\home\.copilot`, `COPILOT_CACHE_HOME`, `USERPROFILE` and `HOME` under `<scratch>\home`, `COPILOT_PROVIDER_BASE_URL`, `COPILOT_MODEL`, `COPILOT_AUTO_UPDATE=false`; clear `WT_SESSION`, `WT_PROFILE_ID`, `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN`. Use `%LOCALAPPDATA%\GitHubCopilotCLI\copilot.exe` and a project folder `<scratch>\proj` (a git repository). Read the fake for how to log a request body and for its tool words.
- [ ] **Step 2: Skills.** In `<scratch>\home\.copilot\skills` make: a good skill; one with no `name`; one with no `description`; one whose frontmatter doesn't close; one with a 65-character name; one with a name holding `!`; one with a 1,025-character and one with a 3,000-character description; one with `disable-model-invocation: true`; and in `<scratch>\proj\.github\skills` a skill with the good skill's name. Run `copilot skill list --json` and one `copilot -p hello` with the request body logged. Record for each skill: listed or not, `enabled`, any warning, and how it appears in `<available_skills>` (whole, cut, name only, absent). Then add 200 small skills and record where the listing stops giving descriptions.
- [ ] **Step 3: The two lists.** Put the good skill's name in `disabledSkills` in another case, then in its own case, and a server's name in `disabledMcpServers`; record `skill list --json` and `mcp list --json` each time.
- [ ] **Step 4: MCP config.** A scratch stdio server (a small Node script in `<scratch>\tools`, one tool `echo`) in `mcp-config.json` and a second in `<scratch>\proj\.mcp.json`. Record: whether `mcp list --json` reads `mcp-config.json` with a `//` comment and with a trailing comma; whether the request body's `tools` holds the server's full definitions or a deferred listing; the tool's name as the model sees it.
- [ ] **Step 5: Events.** With the fake's tool word (add `CALL <tool> <json>` to `fake-openai.mjs` if it has none), run one prompt that calls the scratch server's tool and one that calls the `skill` tool for the good skill, and `copilot -p "/<skill>"`. Copy each `events.jsonl`. Record the exact fields of the `tool.execution_start` for an MCP call and of `skill.invoked`, and whether a typed `/name` writes one.
- [ ] **Step 6: Trust.** Record what `config.json` holds after a run with `--trust-folder` and, in Step 8's interactive run, after "remember this folder": the shape of `trustedFolders`, and whether the project server in `.mcp.json` loads in a subfolder of a trusted folder.
- [ ] **Step 7: Scrub and commit the fixtures.** `pnpm scrub:fixtures crates/core/tests/fixtures/copilot/transcripts`; `cargo test -p marshell-core -j 4 --test fixtures_scrubbed` passes.
- [ ] **Step 8: The toast.** Tell the user a Windows notification from Copilot CLI is about to show, then start the app with the `marshell-live-check` skill (`-CopilotFake`, scratch homes, the app not brought to the front), with `"notifications": true` in the scratch Copilot `settings.json` and hooks set up. Run one prompt in a Copilot tab while another window has the focus. Record whether Copilot's toast shows, and whether Marshell's banner shows beside it. Stop only what the check started.
- [ ] **Step 9: Decision gate.** Write the findings into the research note. Put to the user through AskUserQuestion whatever changes a decision: no toast from a Marshell tab (decision 2 then drops the item and Task 3 shrinks to the notification hooks); a skill limit that cuts where the plan says Broken, or the reverse (decision 11); a list that doesn't turn an item off (decisions 3 and 10); no `tool.execution_start` or `skill.invoked` on disk (decision 12 then follows Antigravity's "can't yet tell" line). Record the answers in PLAN §9 and change Tasks 3 to 5 to match, including the constants marked "Task 1" below.
- [ ] **Step 10: Commit:** `test(doctor): record Copilot CLI 1.0.106's MCP call and skill use against the fake model`

### Task 2: Copilot's facts, hooks item and plain lines (core)

**Files:**
- Modify: `crates/core/src/doctor/checks.rs`, `crates/core/src/server/doctor.rs`, `crates/core/src/agents/copilot/hooks.rs`, `crates/core/tests/doctor_api.rs`

**Interfaces:**
- Consumes: `server::agents::copilot_setup_of(&AppState) -> Result<AgentSetup, Fail>`, `copilot_hooks::{hooks_path, settings_path, inspect, Files}`, `agents::copilot::version_gate`.
- Produces: `copilot_hooks::file_problem(text: Option<&str>) -> Option<FileProblem>` with `pub enum FileProblem { DoesNotParse { line: u32, message: String }, NotMarshells }`; `checks::FileState::NotOurs`; `AGENTS` and `ORDER` with `"copilot"` last; `display_name("copilot") == "Copilot CLI"`.

- [ ] **Step 1: Failing tests in `checks.rs`.** `display_name("copilot")` is "Copilot CLI"; the order test puts `copilot` after `qwen`; the retention test's loop gains `copilot` and the item's detail ends with the new `RETENTION_OTHERS`; a `copilot` fact with `HookInstall::None` and `set_up` gives `hooks.copilot` "Copilot CLI's hooks are gone", Broken and fixable; a fact with `FileState::NotOurs` and `set_up` gives one Broken item with id `parse.copilot`, the title, detail and `cannot_fix` from Global constraints, `fixable: false`, and no `hooks.copilot` item; `NotOurs` without `set_up` gives the Info "Hooks aren't set up for Copilot CLI." only. Run `cargo test -p marshell-core -j 4 doctor::checks`: FAIL.
- [ ] **Step 2: `checks.rs`.** Add the arm, the constant, `ORDER: [&str; 5]`, the `NotOurs` variant and its item (treated like `Unparsable` for every later check in `cli_items`). Run: PASS.
- [ ] **Step 3: Failing tests in `copilot/hooks.rs`.** `file_problem(None)` is `None`; Marshell's own file (from `hooks_file`) is `None`; Marshell's entry-less file is `None`; `{}` and a file with one entry of the person's own are `NotMarshells`; `{"hooks": ` is `DoesNotParse` with a line.
- [ ] **Step 4: Implement `file_problem`** over `read_value` and the existing `is_marshells`, and make `marshells_hooks` use it. Run `cargo test -p marshell-core -j 4 agents::copilot::hooks`: PASS.
- [ ] **Step 5: Failing tests in `server/doctor.rs`** (its test module has `pinned` and a state builder; add a Copilot home with `with_copilot_home`):
  - `facts_of` gives a fifth fact, `agent: "copilot"`, `path` the hooks file, `set_up` true after an install.
  - With the hooks file deleted after an install: `hooks.copilot` is in the report, and `fix_of(["hooks.copilot"])` plans an edit that creates the file again and, when `managed.json` names `/statusLine` in `settings.json`, keeps `statusline: true` (the plan holds no edit that removes the status line).
  - With `{}` written over the hooks file: `parse.copilot` is in the report, `fix_of(["hooks.copilot"])` is a 422, and a fix of another CLI's item picked alone still plans.
  - With `settings.json` unparsable: the fact's `file` is `Unparsable` and its message starts with the settings path and `": "`.
  - `distro_agent_facts("copilot", …)` over a temp "distro home": files `<home>/.copilot/hooks/marshell.json` and `<home>/.copilot/settings.json`; Installed after the distro plan's texts are written; `NotOurs` for `{}`.
  - `hooks_want_of` for a Copilot distro fact whose `settings.json` has a `/statusLine` entry in `managed.json` asks for `statusline: true`; for a Qwen distro fact it stays `false`.
  - `cli_doctor_items` gives "Copilot CLI has no doctor command." when Copilot is found.
- [ ] **Step 6: Implement in `server/doctor.rs`.** `AGENTS: [&str; 5]`. Every `_ =>` named in "Checked before planning" becomes explicit arms for `"qwen"` and `"copilot"` with `_ => unreachable!` replaced by a `failed_facts`-style refusal where a value is needed (no panic on an unknown agent). `facts_from` for Copilot: when the settings state is Ok or ReadOnly, read the hooks file and map `file_problem` to `FileState::Unparsable` or `FileState::NotOurs`; when `settings.json` is the file that doesn't parse, prefix the message with its path. `distro_agent_facts`: Copilot's two files, `copilot_hooks::inspect` with `chain_path: None`, and the same two problems. `hooks_want_of`: for Copilot the status line's file is `<facts.path>/../../settings.json`, and it is asked for on Windows and in a distro when the line is chained or `managed.json` names `/statusLine` there. Run `cargo test -p marshell-core -j 4 server::doctor`: PASS.
- [ ] **Step 7: `tests/doctor_api.rs`.** `AGENTS` gets `"copilot"`; `Env` gets the Copilot hooks and settings paths and builds its state with `with_copilot_home`; `hooks_file` and `seed_own_hooks` get Copilot arms (the person's own hooks go in `<home>/.copilot/hooks/mine.json`, since `marshell.json` is Marshell's whole file); in `set_up_follows_each_clis_real_plans` Copilot's hooks file is deleted instead of emptied. Run `cargo test -p marshell-core -j 4 --test doctor_api`: PASS.
- [ ] **Step 8: Commit:** `feat(doctor): Copilot CLI's hooks are checked and fixed, and it gets the retention and doctor lines.`

### Task 3: Copilot's own toasts and notification hooks (core)

**Files:**
- Create: `crates/core/src/agents/copilot/notify.rs` (and `pub mod notify;` in `agents/copilot/mod.rs`)
- Modify: `crates/core/src/doctor/checks.rs`, `crates/core/src/server/doctor.rs`

**Interfaces:**
- Consumes: Task 2's Copilot facts.
- Produces: `notify::toasts_on(settings: &Value) -> bool`; `notify::own_hooks(files: &[(PathBuf, Value)], settings: Option<&Value>) -> Vec<String>`; `checks::NotifyFacts { toasts: bool, settings: PathBuf, .. }`; item id `toasts.copilot`.

- [ ] **Step 1: Failing tests in `notify.rs`.** `toasts_on` is true only for `{"notifications": true}` (false for `"true"`, `1`, missing, `false`). `own_hooks` returns, in file-name order then file order, the command of every entry under the event `notification` that `agents::common::is_ours` doesn't claim: from each hooks file value and from `settings.hooks.notification`; a flat entry and a Claude-style group (`{matcher, hooks: [...]}`) both count; the command text is the first of `powershell`, `command`, `bash`, else `exec` and its `args` joined with spaces, else the `url` of a `type: "http"` entry; a `type: "prompt"` entry and one with none of these are skipped; a file with `disableAllHooks: true` gives none.
- [ ] **Step 2: Implement `notify.rs`.** Run `cargo test -p marshell-core -j 4 agents::copilot::notify`: PASS.
- [ ] **Step 3: Failing tests in `checks.rs`.** With `banners_on`, a Windows Copilot fact with `notify.toasts` and hooks Installed gives `toasts.copilot`: Broken, fixable, checked by default (`unchecked: false`), the exact title and detail, `path` the settings file. No item with banners off, with `HookInstall::None`, with a distro, or with an unparsable file. A read-only settings file makes it not fixable with "The file is read-only." Copilot `hook_commands` give "Copilot CLI runs your own Notification hook" with the commands joined by "; ".
- [ ] **Step 4: Implement in `checks.rs`.** Run: PASS.
- [ ] **Step 5: Failing tests in `server/doctor.rs`.** `notify_facts` for Copilot reads every `*.json` in `<home>/hooks` except `marshell.json` (a file that doesn't parse is skipped) and the settings value. After a setup, with `"notifications": true` in a `settings.json` that has a comment, a trailing comma and CRLF: the report holds `toasts.copilot`; `fix_of(["toasts.copilot"])` plans one `JsonSet` of `/notifications` to `false` on the settings file with `Applies::NewSessions`; applying it changes only that value's bytes; `managed.json` gains no entry for `/notifications`; undo gives the file back byte-equal. `notifications: "true"` gives no item. In a distro fact `toasts` is false whatever the file says.
- [ ] **Step 6: Implement in `server/doctor.rs`.** `notify_facts` gets the Copilot arm; `fix_of` gets `Some("toasts")` (the edit is built against the file's text now, as `retention_edit` is); `is_own_edit` also matches the pointer `/notifications`. Run `cargo test -p marshell-core -j 4 server::doctor` and `--test doctor_api`: PASS.
- [ ] **Step 7: Commit:** `feat(doctor): Copilot CLI's own notifications are a Broken item whose fix turns them off, and its notification hooks are reported.`

### Task 4: Copilot's MCP servers and skills (core; worker at `effort: high`)

**Files:**
- Create: `crates/core/src/agents/copilot/extensions.rs` (and `pub mod extensions;`)
- Modify: `crates/core/src/doctor/ext.rs`, `crates/core/src/doctor/usage.rs`, `crates/core/src/server/doctor_ext.rs`, `crates/core/src/server/doctor_measure.rs`

**Interfaces:**
- Consumes: `doctor::ext::{ExtRoots, Inventory, Server, Skill, Switch, NoSwitch, Origin, Launch, Expand, CostRule, Listing, UsedAs, SkillUse, Caps, Hints, skill_files, created_ms, unreadable}`, `doctor::frontmatter`, `agents::copilot::last_account`.
- Produces: `copilot::extensions::inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory`; `ExtRoots { copilot_dir: PathBuf, copilot_home_set: bool, copilot_token: bool, copilot_offline: bool, .. }`; `Origin::BuiltIn`; `Expand::DollarDefault`; `pub const GITHUB_SERVER: &str = "github-mcp-server"`.

Read `agents/qwen/extensions.rs` first: Copilot's reader has the same parts (a `Files` cache with `unreadable`, `servers_in`, `skills_in`, the same-name pass, `inventory`).

- [ ] **Step 1: Engine additions, tests first.**
  - `ext.rs`: `Origin::BuiltIn` ("comes with the CLI, and is judged and switched like the person's own"). `ORDER` gets `"copilot"`. A test: a `BuiltIn` server with a switch that is unused gives `mcpunused.copilot.<key>`, Waste, fixable, with the detail "It still starts with every Copilot CLI session. It comes with Copilot CLI."
  - `usage.rs`: the `judge` closure appends the sentence for `Origin::BuiltIn`; the broad-scope verdict stays `Origin::User` only.
  - `doctor_measure.rs`: `Expand::DollarDefault` expands `$VAR`, `${VAR}` and `${VAR:-default}`; a test beside the two existing expansion tests covers the three forms and an unset variable with and without a default.
- [ ] **Step 2: Roots.** `roots_under(home, …)` sets `copilot_dir: home.join(".copilot")` and the three flags false. `windows_roots` sets `copilot_dir` from `agents::copilot::home` over the process environment, `copilot_home_set` when `COPILOT_HOME` is set and not blank, `copilot_token` when `agents::copilot::token_env` names a variable, `copilot_offline` when `COPILOT_OFFLINE` is `true`. `read_inventory` gets the `"copilot"` arm.
- [ ] **Step 3: Failing tests for servers** (temp roots; each asserts the `Server` fields named):
  - A user server in `mcp-config.json` under `mcpServers`: `Origin::User`, `file` the config file, `Transport::Stdio`, `switch` `JsonAdd { file: <copilot_dir>/settings.json, pointer: "/disabledMcpServers", name }`, `used_as: UsedAs::Server(name)`, `launch` Ok with `Expand::DollarDefault`, `Inherit::All`, the entry's `args`, `env`, `cwd`, and `timeout_secs` 30 (or the entry's `timeout` in ms, rounded up to seconds).
  - `url` gives `Transport::Remote` and `Err(NotMeasured::Remote)`; an entry with neither, or that isn't an object, gives `Transport::Nothing`; a name that is empty or holds a control character is skipped (Copilot skips it).
  - `off` is true when the name is in the user's `disabledMcpServers` (exact match, case-sensitive), and for a project server also when it is in that project's `.github/copilot/settings.json` or `settings.local.json`.
  - A project's `.mcp.json` (with the `mcpServers` wrapper and without it) and `.github/mcp.json`: `Origin::Project(project)`, the same user-file switch, `launch` Ok only when `config.json`'s `trustedFolders` holds the project folder (compared after `\` to `/`, a trailing `/` cut, lower case), else `Err(NotMeasured::NotApproved)`. `trustedFolders` in another shape trusts nothing.
  - Cost: `Listing::Names { prefix: "<name>-", instructions_cap: Some(0) }` by default; `Listing::Full { prefix: "<name>-", desc_cap: None }` when the user's `toolSearch` is `false` or the entry says `"deferTools": "never"`; `only` is the entry's `tools` unless it is `["*"]`; `except` is `excludeTools`. `Inventory::cost_note` is the sentence from Global constraints.
  - The built-in: with `lastLoggedInUser` in `config.json` (or `copilot_token`), not `copilot_offline`, and no configured server of that name, there is a `Server { name: GITHUB_SERVER, origin: Origin::BuiltIn, file: <settings.json>, transport: Remote, switch: Ok(JsonAdd …), launch: Err(Remote), .. }`, `off` when the name is in `disabledMcpServers`. With nobody signed in and no token there is none.
  - Two servers named alike, one user and one project, both in use: both get `Err(NoSwitch::SameName)`; when one is already off the other keeps its switch.
  - A `mcp-config.json` that doesn't parse, isn't UTF-8 or is over 4 MiB is in `unreadable` with the matching sentence, and gives no server.
- [ ] **Step 4: Failing tests for skills.**
  - Folders read: `<copilot_dir>/skills`; `<home>/.agents/skills` unless `copilot_home_set`; per project `.github/skills`, `.agents/skills`, `.claude/skills`. `~/.claude/skills` is never read.
  - The name is the frontmatter's `name`, else the folder's name. State (Task 1 confirms each line; change the constant or the state there, not here): no closing `---` block is `NoBlock`; no `description` is `Missing("description")`; a name over 64 characters, or not matching "starts with an ASCII letter or digit, then ASCII letters, digits, `-`, `_`, `.`, `:` and spaces", is `BadName`; a description over 1,024 characters stays `Ok` (it is a long skill, not a broken one) unless Task 1 shows Copilot refuses to load it, in which case Task 1 adds a state and its sentence to this step.
  - `listed_chars` is the description's length, `caps.per_skill` is `Some(1024)` only if Task 1 shows a cut, and `caps.listing_first_line_over` stays `None` unless Task 1 finds the budget.
  - `off` when `disabledSkills` holds the name (exact match unless Task 1 shows otherwise). `switch` is `JsonAdd { pointer: "/disabledSkills", name }` on the user's settings. `used_as: [SkillUse::Name(name)]`. `hidden` when `disable-model-invocation` is true.
  - A second skill with a name already taken by a higher-priority folder (project before personal, and within each the order above) is `off: true`, since Copilot skips it.
  - Hints are the two sentences from Global constraints.
- [ ] **Step 5: Failing tests for plugins.** `config.json` with `installedPlugins: [{name, marketplace, enabled, cache_path?}]`: a plugin that is `enabled` and not `false` under `enabledPlugins["<name>@<marketplace>"]` in the user's settings has its folder read (`cache_path`, else `<copilot_dir>/installed-plugins/<marketplace>/<name>`): servers from the manifest's `mcpServers` (an object, or a relative path to a file) and from `.mcp.json`, `mcp.json` and `.github/mcp.json` in it; skills from the manifest's `skills` folder, else `skills`; the manifest is the first of `plugin.json`, `.plugin/plugin.json`, `.github/plugin/plugin.json`, `.claude-plugin/plugin.json`. Each item is `Origin::Plugin(name)` with `Err(NoSwitch::Plugin(name))`, a plugin skill is named `<plugin>:<skill>`, and a plugin server's launch is `Err(NotMeasured::NotApproved)`. A `cache_path` or manifest path that leaves the plugin's folder (`..`, absolute) is not followed. No token or other value of `config.json` is in the inventory (`format!("{inv:?}")` doesn't contain a planted secret).
- [ ] **Step 6: Implement `extensions.rs`.** Run `cargo test -p marshell-core -j 4 agents::copilot::extensions`: PASS.
- [ ] **Step 7: Server-side tests in `doctor_ext.rs`.** With a Copilot home under `with_ext_home`: `inventories` holds a Copilot inventory; a broken user server gives a fixable `mcp.copilot.<key>` whose fix adds its name to `/disabledMcpServers` in a `settings.json` with a comment and a trailing comma, keeping the other bytes; two picked items give one edit per list; a `disabledMcpServers` that is a string makes the item report-only (`NoSwitch::CantEdit`); a distro inventory reads `<distro home>/.copilot` at user level only. Run `cargo test -p marshell-core -j 4 server::doctor_ext`: PASS.
- [ ] **Step 8: Commit:** `feat(doctor): Copilot CLI's MCP servers and skills are listed, costed and turned off through its own lists.`

### Task 5: Copilot's usage scan (core)

**Files:**
- Create: `crates/core/src/agents/copilot/usage.rs` (and `pub mod usage;`)
- Modify: `crates/core/src/server/doctor_usage.rs`

**Interfaces:**
- Consumes: `doctor::usage::{AgentStore, Cursor, Event, EventKind, Scan, UseKey, new_lines}`, `ExtRoots::copilot_dir`, Task 1's fixtures.
- Produces: `copilot::usage::scan(roots: &ExtRoots, store: &AgentStore) -> Scan`.

Read `agents/qwen/usage.rs` first: the shape is the same.

- [ ] **Step 1: Failing tests.**
  - History files are `<copilot_dir>/session-state/<folder>/events.jsonl` for every folder whose name is a UUID, sorted; another folder name is skipped.
  - Over Task 1's two fixtures: the MCP fixture gives one `Used` event with `UseKey::Server("<the scratch server's name>")`, the time of its `tool.execution_start` and the `cwd` of the file's `session.start` (`data.context.cwd`); the skill fixture gives `UseKey::SkillName("<name>")` from `skill.invoked` (`data.name`, with `data.pluginName` in front as `<plugin>:<name>` when it is there).
  - A `tool.execution_start` with no `mcpServerName`, or an empty one, gives nothing. A `session.resume` with another `context.cwd` changes the cwd of the events after it.
  - The cursor: a second scan with the store of the first reads nothing new; after lines are appended it gives only their events; a last line without a newline is not read until it has one; `first_ms` is the first timestamp of the file.
  - A `session-state` folder that can't be listed gives `unreadable: Some("Its history folder can't be listed.")` and keeps the old cursors.
  - In `doctor_usage.rs`: `scan_all` with a Copilot inventory and a fixture copied into the Copilot home stores a use under `agents["copilot"]`, and an unused user server older than 30 days becomes `mcpunused.copilot.<key>` in the report.
- [ ] **Step 2: Implement** `usage.rs` and the `"copilot"` arm in `scan_all`. Run `cargo test -p marshell-core -j 4 agents::copilot::usage` and `server::doctor_usage`: PASS.
- [ ] **Step 3: Commit:** `feat(doctor): Copilot CLI's event logs feed the 30-day usage check.`

### Task 6: Measure and the page's copy

**Files:**
- Modify: `crates/core/src/server/doctor_measure.rs` (tests only, unless one fails), `src/features/doctor/DoctorPage.tsx`, `src/features/doctor/DoctorPage.test.tsx`, `src/features/doctor/fixtures.ts`

- [ ] **Step 1: A Measure test.** In `doctor_measure.rs`'s tests, a Copilot inventory with a trusted user server (the test's stub server), a remote one, the built-in and an untrusted project server: the stub is measured, and `mcpcost.copilot` reads "1 measured. … Not measured: 2 remote servers, 1 that Copilot CLI hasn't approved." followed by the cost note. Run `cargo test -p marshell-core -j 4 doctor_measure`: PASS (fix what fails).
- [ ] **Step 2: Frontend, test first.** `DoctorPage.test.tsx`: the progress line reads "Checking Claude Code, Codex, Antigravity, Qwen Code and Copilot CLI…", and the fixture's `cli.copilot` row "Copilot CLI has no doctor command." renders under Info. Add the row to `fixtures.ts` beside `cli.qwen` and change the line in `DoctorPage.tsx`. Run `pnpm test src/features/doctor`: PASS. Run `pnpm build`: passes.
- [ ] **Step 3: Commit:** `feat(doctor): the Doctor page names Copilot CLI, and Measure covers its servers.`

### Task 7: Issue #154, `/clear` in a Copilot WSL tab (core; worker at `effort: high`)

**Files:**
- Modify: `crates/core/src/server/hook.rs` and its tests (the existing Copilot end-hold tests are the model)

**Interfaces:**
- Consumes: `copilot::is_requested_end`, `hold_copilot_end`, `st.copilot_end_hold()`, `registry().clear_hook_agent`.

- [ ] **Step 1: Failing tests,** with the hold set short through the state's test setter and a tab the test marks as a WSL tab:
  - A tab where a hook marked Copilot: a requested `sessionEnd`, then within the hold a `sessionStart` of a new session id. The row keeps its agent mark throughout and never reads Ended.
  - A tab launched as Copilot (the row has an agent): the same sequence never reads Ended.
  - With no newer session inside the hold: after it, the mark clears (first kind) and the row reads Ended (second kind), as today.
  - An end that wasn't requested (`reason` other than the person's exit) is not held, as on Windows.
  If the first two can't be made to fail, the fault isn't there: stop, report, and leave #154 for the live check.
- [ ] **Step 2: Implement.** In the WSL branch a requested Copilot end no longer clears the mark at once: it goes through `hold_copilot_end`, and the task that wakes clears the mark and folds only when `copilot_session` doesn't say Stale. Nothing is held across the wait. Run `cargo test -p marshell-core -j 4 server::hook`: PASS.
- [ ] **Step 3: Commit:** `fix(status): /clear in a Copilot CLI tab inside WSL keeps the row's agent and doesn't read Ended (#154).`

### Task 8: Live check and exit (main thread)

- [ ] **Step 1:** Run `marshell-live-check` with a fresh literal scratch folder and `-CopilotFake`, the app never brought to the front. Over CDP, on the Doctor page:
  - With Copilot's hooks set up: no Copilot item but its cost and usage lines. Delete `hooks\marshell.json`: "Copilot CLI's hooks are gone" is pre-checked; Fix shows the preview, applies, and a new Copilot tab shows live status; Undo last fix removes the file again.
  - Write `{}` over the hooks file: the "isn't Marshell's file any more" item shows, with no checkbox.
  - `"notifications": true` in the scratch `settings.json` (with a comment in the file): the toast item shows pre-checked; Fix sets it to `false` and keeps the comment; with Marshell's banners switched off in Settings the item is gone.
  - A hooks file `hooks\mine.json` with a `notification` entry: the Info item names its command.
  - "Run CLI doctors": "Copilot CLI has no doctor command."
  - A scratch `mcp-config.json` with one working stdio server, one whose command is missing and one remote; a scratch skill without a description. The missing server and the skill are Broken; Fix adds the names to the two lists; `copilot mcp list --json` and `copilot skill list --json` in the scratch home show both off.
  - "Measure MCP servers": the working server is measured and the cost line names it.
  - Run one prompt that calls the working server, then check again: its use is stored (`<scratch>\marshell\doctor\usage.json` names it under `copilot`).
  - The page with the keyboard alone (Tab to each Copilot item, Space on its checkbox, Enter on Fix) and in emulated forced colours.
- [ ] **Step 2: WSL.** With the user's OK and the recipe of 6n2's live check (`/tmp` scratch prefix): after "Set up in <distro>" the Doctor shows no Copilot item for the distro; with the distro's hooks file deleted the "in <distro>" item shows and its fix restores the hooks file and the status line; a distro server is "Not measured: in <distro>." Then #154: type `/clear` in a Copilot tab started from the launcher and in one where `copilot` was typed into the distro's shell, sampling the row every 50 ms for two seconds: it never reads Ended and never loses its agent. Ask before deleting the `/tmp` scratch.
- [ ] **Step 3:** Write `docs/research/2026-11-24-copilot-doctor-live-check.md`: each check with its evidence, findings numbered.
- [ ] **Step 4:** Stop the check by PID, after listing the tree and looking for agent CLIs the person started. Ask before removing the scratch folders.
- [ ] **Step 5:** Commit `docs(research): record slice 6n3's live check`. Then the final whole-branch review (`marshell-reviewer`, Opus), one fix pass, and `marshell-ship` (the PR closes #154).
