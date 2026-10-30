# Slice 6h2: the built-in `marshell` skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marshell installs one `SKILL.md` per agent CLI (Claude Code, Codex, Antigravity) at user level, so an agent inside a Marshell session knows how to use the `marshell` CLI. The install, update and removal go through the config service's preview, diff, backup and undo.

**Architecture:** A new core module `agents/skill.rs` owns the skill text, which is embedded at build time. It also owns each CLI's skill path, the install state read from the file's hash, and the plans (create, replace, delete). Those plans are `ChangeSet`s that the existing config service previews and applies. `AgentSetup` reports the skill's state and path. The hooks plan route takes `skill: true`, so onboarding and the Settings "Set up" panel install the skill in the same preview as the hooks. Two new routes plan the skill on its own, for the Settings row's Install, Update and Remove. `marshell uninstall` removes a skill that still holds Marshell's text.

**Tech Stack:** Rust (marshell-core, marshell-protocol with ts-rs, marshell-cli), React and TypeScript (Vitest, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 6h2 decisions (2026-10-30)", decisions 1–5. Research: `docs/research/2026-10-30-cli-skill-folders.md`.

## Global Constraints

- Skill paths (decision 1). Nothing is written into project folders.
  - Claude Code: `<CLAUDE_CONFIG_DIR, if set and non-empty, else <user home>/.claude>/skills/marshell/SKILL.md`.
  - Codex: `<user home>/.agents/skills/marshell/SKILL.md`.
  - Antigravity: `<user home>/.gemini/config/skills/marshell/SKILL.md`.
- One file serves all three CLIs (decision 5). The frontmatter holds exactly `name: marshell` and a double-quoted one-line `description`. The folder is `marshell`. The file is under 24 KB and 500 lines.
- Every write is `preview → diff shown → apply` through `ConfigService` (PLAN.md §6). A file Marshell didn't write, or one the person edited, is never overwritten without its diff on screen (decision 3).
- Tests never touch the real home: every test sets `AppState::with_skill_home(<temp>)`, or calls the pure functions with a temp `user_home` and a fake `env`.
- New protocol types go in `crates/protocol`, followed by `cargo test -p marshell-protocol` and `pnpm check:generated`.
- Run cargo with `-j 4`. Commits read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Copy follows PLAN.md §Microcopy: sentence case, no "!", no "successfully". The exact strings are in each task, and they go to the user for approval with the live check.
- Windows 11 is the only verified platform. The paths are built the same way on all OSes.

## Review Focus

1. **`CLAUDE_CONFIG_DIR` set**: the Claude skill goes under it, not `~/.claude`. That is where Claude reads it, and where a live check's scratch config lives. Task 1 tests it.
2. **A `marshell` skill already there that Marshell didn't write**: its state is `edited`. Install becomes a whole-file replace that shows the diff and is checked against the file's hash, never a blind create. Tasks 1 and 2 test it.
3. **The file changes between preview and apply**: apply reports a conflict and writes nothing. Task 2 tests it.
4. **A home with no `.agents` or `.gemini/config` folder yet**: the create makes the parent folders, and removal deletes the empty `marshell` folder it leaves. Task 2 tests it.
5. **`marshell uninstall` with an edited skill**: it leaves the file and says so. With no skill, it says nothing about skills. Task 3 tests it.

---

### Task 1: The skill text, paths, state and plans (core)

**Files:**
- Create: `crates/core/src/agents/skill/SKILL.md`
- Create: `crates/core/src/agents/skill.rs` (declare `pub mod skill;` in `crates/core/src/agents/mod.rs`)
- Modify: `crates/protocol/src/agents.rs` (add `SkillInstall`), regenerate `src/generated/`

**Interfaces:**
- Produces, in `marshell_protocol::agents`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
  #[serde(rename_all = "snake_case")] #[ts(export)]
  pub enum SkillInstall { None, Installed, Outdated, Edited }
  ```
- Produces, in `marshell_core::agents::skill`:
  - `pub const SKILL_TEXT: &str`, which is `include_str!("skill/SKILL.md")`.
  - `pub const PREVIOUS: &[&str]`, the `hash_bytes` hex of every earlier shipped `SKILL_TEXT`. It is empty for now. A later release adds the old hash when it changes the text.
  - `pub fn skill_path(agent: &str, env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> Option<PathBuf>`. It returns `None` for an agent that isn't `claude`, `codex` or `antigravity`.
  - `pub fn state_of(bytes: Option<&[u8]>) -> SkillInstall`:
    - missing → `None`;
    - equal to `SKILL_TEXT` → `Installed`;
    - hash in `PREVIOUS` → `Outdated`;
    - anything else → `Edited`.
  - `pub fn install_edit(path: &Path, bytes: Option<&[u8]>) -> Option<FileEdit>`:
    - `None` state → `CreateFile { content: SKILL_TEXT }`;
    - `Outdated` or `Edited` → `ReplaceText { content: SKILL_TEXT }` with `expected_hash = hash_bytes(bytes)`;
    - `Installed` → `None` (nothing to do).
    - `scope: ConfigScope::User` and `applies` as the hooks edits set it.
  - `pub fn install_plan(agent_name: &str, path: &Path, bytes: Option<&[u8]>) -> ChangeSet`:
    - for `None`: title `Install the marshell skill for <agent_name>`;
    - for `Outdated` or `Edited`: `Update the marshell skill for <agent_name>`;
    - reason: `So <agent_name> knows how to use the marshell command.`;
    - edits: the `install_edit`, if there is one.
  - `pub fn remove_plan(agent_name: &str, path: &Path, bytes: Option<&[u8]>) -> ChangeSet`:
    - title `Remove the marshell skill for <agent_name>`, reason `Marshell takes out the skill it installed.`;
    - one `DeleteFile` with `expected_hash`, or no edits when the file is missing.
  - `agent_name` is the display name `CLI_NAME` uses: "Claude Code", "Codex", "Antigravity CLI".

- [ ] **Step 1: Write the skill text** as `crates/core/src/agents/skill/SKILL.md`, exactly:

```markdown
---
name: marshell
description: "Use inside a Marshell session (MARSHELL_TAB_ID is set) when the person asks to list, start, message or bring forward Marshell sessions, or to hand work to another agent CLI in a new Marshell session."
---

# Marshell

Marshell is the desktop app this terminal runs in. Its `marshell` command lists, starts, fills and focuses the app's sessions, and it is on this session's PATH.

Use it only when `MARSHELL_TAB_ID` is set. Without it you are not inside Marshell: say so and stop. Your own session's number is `MARSHELL_SESSION_NUMBER`.

## Commands

- `marshell list --json`: the open sessions as JSON, each with `number`, `tab_id`, `name`, `agent`, `state` (`running`, `ended` or `exited`), `cwd`, `project` and `mode`. Read this, not the plain table.
- `marshell send <session> "<text>"`: puts the text on that session's prompt line. Marshell never presses Enter: the person reads it and sends it. `<session>` is a number or a tab id. The text is one line, at most 4000 characters.
- `marshell new --agent <agent> [--cwd <folder>] [--prompt "<text>"] [--mode <mode>] [--name <name>]`: starts a session. `<agent>` is an agent id such as `claude`, `codex` or `antigravity`. `--prompt` fills the new session's prompt line once the agent is ready, without pressing Enter. `<mode>` is `manual`, `plan`, `auto-edit`, `full-auto`, `bypass` or `dont-ask`.
- `marshell focus <session>`: brings Marshell's window to that session.

## Rules

- Send only to a session the person named, or one you started for them. Never send to your own session.
- Don't start a session in `bypass`, `dont-ask` or `full-auto` mode unless the person asked for that mode.
- Don't pass `--start`. If the command says "Marshell isn't running.", tell the person.
- When `send` or `new` refuses, tell the person what it said and don't retry:
  - "is waiting for an answer": that session shows a question or a permission request.
  - "has ended": that session can't take text.
  - "wasn't ready within 30 s": the prompt line wasn't filled.
```

- [ ] **Step 2: Write the failing tests** in `skill.rs`'s `mod tests`:
  - `skill_text_fits_every_cli`:
    - the text starts with `"---\nname: marshell\ndescription: \""`;
    - the frontmatter has exactly two keys;
    - the description is one line under 1024 characters;
    - `SKILL_TEXT.len() < 24 * 1024`;
    - `SKILL_TEXT.lines().count() < 500`;
    - `SKILL_TEXT` holds no `\r`.
  - `skill_paths_per_cli`, with `home = Path::new("/h")`:
    - claude with no env → `/h/.claude/skills/marshell/SKILL.md`;
    - claude with `CLAUDE_CONFIG_DIR=/c` → `/c/skills/marshell/SKILL.md`;
    - claude with `CLAUDE_CONFIG_DIR=" "` → the `/h/.claude` path;
    - codex → `/h/.agents/skills/marshell/SKILL.md`;
    - antigravity → `/h/.gemini/config/skills/marshell/SKILL.md`;
    - `grok` → `None`.
  - `state_follows_the_file`:
    - `None` → `SkillInstall::None`;
    - `SKILL_TEXT` bytes → `Installed`;
    - `b"mine"` → `Edited`.
    - (`Outdated` is covered by a `#[cfg(test)]` helper `state_with(bytes, previous: &[&str])` that `state_of` calls with `PREVIOUS`: `state_with(b"old", &[&hash_bytes(b"old")]) == Outdated`.)
  - `plans_create_replace_or_nothing`:
    - missing file → one `CreateFile` with no `expected_hash`;
    - `b"mine"` → one `ReplaceText` with `expected_hash == Some(hash_bytes(b"mine"))` and title `Update the marshell skill for Codex`;
    - `SKILL_TEXT` → `install_plan(..).edits.is_empty()`;
    - `remove_plan` on `SKILL_TEXT` → one `DeleteFile` with that hash;
    - `remove_plan` on a missing file → no edits.

- [ ] **Step 3: Run them to see them fail**: `cargo test -j 4 -p marshell-core --lib agents::skill`. Expected: compile errors, then FAIL.
- [ ] **Step 4: Implement** `SkillInstall`, then the module.
- [ ] **Step 5: Run** `cargo test -j 4 -p marshell-core --lib agents::skill` (PASS), `cargo test -j 4 -p marshell-protocol` (regenerates `src/generated/SkillInstall.ts`) and `pnpm check:generated` (ok).
- [ ] **Step 6: Commit**: `feat(skill): the marshell skill's text, paths, install state and plans`.

### Task 2: Setup state and routes (core)

**Files:**
- Modify: `crates/protocol/src/agents.rs` (`AgentSetup` gets `skill: SkillInstall` and `skill_path: String`; `HookPlanRequest` gets `#[serde(default)] pub skill: bool`)
- Modify: `crates/core/src/server/mod.rs`:
  - new field `skill_home: Option<PathBuf>` and `pub fn with_skill_home(mut self, home: PathBuf) -> Self`, documented like `with_agy_home`;
  - routes `POST /v1/agents/{agent}/skill/plan` and `POST /v1/agents/{agent}/skill/remove-plan`, in the token-guarded router beside `hooks/plan`.
- Modify: `crates/core/src/server/agents.rs`:
  - `fn skill_file(&self, agent: &str) -> Result<PathBuf, Fail>`: `skill_home`, else `files::home_dir()`, with the process env, through `skill::skill_path`;
  - `setup_of`, `codex_setup_of` and `agy_setup_of` fill `skill` and `skill_path` from reading that file;
  - `plan` appends the skill's `install_edit` to the hooks `ChangeSet` when `want.skill`;
  - new handlers `skill_plan` and `skill_remove_plan`, each returning `Json<ConfigPreview>` built in `spawn_blocking`.
- Modify: `crates/core/src/config/service.rs`: after a successful apply, for each `DeleteFile` whose target file name is `SKILL.md`, run `std::fs::remove_dir` on its parent and ignore the error, so only an empty folder goes.
- Test: `crates/core/tests/skill_api.rs`
- Modify: frontend fixtures that build an `AgentSetup` (`src/features/agents/fixtures.ts` and any test literal that `pnpm build` flags) to carry `skill: "none"` and a `skill_path`.

**Interfaces:**
- Consumes: Task 1's `skill_path`, `state_of`, `install_edit`, `install_plan`, `remove_plan` and `SkillInstall`.
- Produces:
  - `AgentSetup.skill` and `AgentSetup.skill_path`;
  - `HookPlanRequest.skill`;
  - `POST /v1/agents/{agent}/skill/plan` → `ConfigPreview` (409-free: an installed skill gives a preview with no files);
  - `POST /v1/agents/{agent}/skill/remove-plan` → `ConfigPreview`;
  - an unknown agent → the same refusal as `hooks/plan`.

- [ ] **Step 1: Write the failing tests** in `crates/core/tests/skill_api.rs`. Build the state like `tests/agents_api.rs`, with `.with_skill_home(tmp)`, `.with_claude_settings(tmp/settings.json)`, `.with_codex_hooks(..)` and `.with_agy_home(tmp)`. Check that no path outside `tmp` is written.
  - `skill_routes_refuse_without_the_bearer`: both routes return 401 without the header.
  - `skill_installs_updates_and_removes_through_the_config_service`, for each of `claude`, `codex` and `antigravity`:
    - `GET setup` says `skill == None` and gives the `skill_path` of Task 1's layout under `tmp`;
    - plan, then apply through `/v1/config/apply` → the file holds `SKILL_TEXT`, and the parent folders were created;
    - `setup` now says `Installed`;
    - write `b"mine"` to it → `Edited`; plan → a `ReplaceText` preview whose diff shows the change; apply → back to `Installed`;
    - remove-plan, then apply → the file and its `marshell` folder are gone, and `skills/` stays.
  - `skill_conflict_writes_nothing`: plan an install, then create the file with `b"other"` before apply → apply answers the conflict, and the file still holds `b"other"`.
  - `hooks_plan_with_skill_adds_the_skill_file`: Claude `hooks/plan` with `{status:true, statusline:true, approve:true, skill:true}` → the preview's files include the settings file and the skill path. With `skill:false`, only the settings file. With the skill already installed and `skill:true`, only the settings file.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-core --test skill_api`. Expected: compile errors, then FAIL.
- [ ] **Step 3: Implement** the protocol fields, `with_skill_home`, `skill_file`, the setup fields, the `plan` change, the two handlers and routes, and the empty-folder removal.
- [ ] **Step 4: Run**:
  - `cargo test -j 4 -p marshell-core --test skill_api` (PASS);
  - `cargo test -j 4 -p marshell-core --test agents_api` (PASS, existing tests);
  - `cargo test -j 4 -p marshell-protocol`;
  - `pnpm check:generated`;
  - `pnpm build`, which flags each `AgentSetup` literal missing the new fields; fix the fixtures.
- [ ] **Step 5: Commit**: `feat(skill): report the skill in each agent's setup and plan its install, update and removal`.

### Task 3: `marshell uninstall` removes the skill (CLI)

**Files:**
- Modify: `crates/cli/src/uninstall.rs` (`run`)
- Modify: `crates/cli/src/args.rs` (`USAGE`'s uninstall line)

**Interfaces:**
- Consumes: Task 1's `skill_path`, `state_of` and `remove_plan`, with `run`'s own `env` and `user_home`.

- [ ] **Step 1: Write the failing tests** in `uninstall.rs`'s tests, using its existing temp-home fixture:
  - `uninstall_removes_skills_marshell_wrote`: with `SKILL_TEXT` at the Claude and Codex skill paths and none for Antigravity, `run(.., yes=true)` deletes both files and their `marshell` folders. The printed plan names both paths.
  - `uninstall_leaves_an_edited_skill`: with `b"mine"` at the Codex path, the file is still there and the output holds `Left <path>: it was edited after Marshell installed it.`
  - `uninstall_without_skills_says_nothing_about_them`: no skill files → the output doesn't contain `skill`.
- [ ] **Step 2: Run** `cargo test -j 4 -p marshell-cli uninstall`. Expected: FAIL.
- [ ] **Step 3: Implement.** Each of the three skill paths in an `Installed` or `Outdated` state joins the removal: its `DeleteFile` goes through the same service apply as the hooks, then `remove_dir` on the empty `marshell` folder. An `Edited` one prints the Left line. Change the `USAGE` uninstall line to: `uninstall [--yes]   Take Marshell's hooks and its marshell skill out of each agent CLI's files and remove the program folder <home>/bin. Shows what changes first and asks; --yes skips the question.`, wrapped like the other lines.
- [ ] **Step 4: Run** `cargo test -j 4 -p marshell-cli` (PASS).
- [ ] **Step 5: Commit**: `feat(cli): uninstall removes the marshell skill it installed and leaves an edited one`.

### Task 4: The skill in Settings and onboarding (frontend)

**Files:**
- Modify: `src/lib/api/agents.ts`:
  - `planSkill(ep, agent): Promise<ConfigPreview>` posts `${base(agent)}/skill/plan`;
  - `planSkillRemove(ep, agent): Promise<ConfigPreview>` posts `${base(agent)}/skill/remove-plan`.
- Modify: `src/features/onboarding/types.ts`: `Capability` becomes `"status" | "approve" | "statusline" | "skill"`.
- Modify: `src/features/onboarding/consent.ts`:
  - `CAPABILITIES[*].skill = { available: true, entries: 0 }` for all three;
  - `consentHeadline`: when `on.skill` and the detection's skill isn't `installed`, it appends `Add the marshell skill in <skillPath>.`, or `Update the marshell skill in <skillPath>.` for `outdated` or `edited`;
  - the "Change nothing" branch also needs `!on.skill`.
  - `Detection` gains `skill` and `skillPath` from `toDetection`.
- Modify: `src/features/agents/agentsState.ts`:
  - `DEFAULT_WANT` gets `skill: true`;
  - `installRequest` passes `skill`;
  - `setupWant` sets `skill: s.skill !== "installed"`.
- Modify: `src/features/onboarding/ConsentCard.tsx`: the skill switch, labelled `Install the marshell skill`, with the hint `Teaches <CLI name> the marshell command: list, send, new and focus.`. It sits after the existing switches and uses the same switch component.
- Create: `src/features/agents/SkillRow.tsx` and `src/features/agents/useSkillFlow.ts`, the latter modelled on `useInstallFlow`: plan, apply, a diff toggle, and a conflict offering Reload.
- Modify: `src/features/agents/AgentsSection.tsx`: `AgentList` renders `SkillRow` under the hooks line for each agent.
- Tests: `src/features/agents/SkillRow.test.tsx`, plus additions to `src/features/onboarding/consent.test.ts` and `src/features/agents/agentsState.test.ts`.

**Interfaces:**
- Consumes: Task 2's `AgentSetup.skill` and `skill_path`, `HookPlanRequest.skill`, and the two skill routes.
- `useSkillFlow(endpoint, agent, onApplied)` returns `{ flow, install, remove, apply, toggleDiff, reload, cancel }`. Here `type SkillFlow = { kind: "install" | "remove"; preview: ConfigPreview | null; open: boolean; busy: boolean; error?: string; conflict?: boolean }`, `install` and `remove` plan their kind, and `reload` plans the same kind again.
- `SkillRow` props: `{ agent: HookAgent; setup: AgentSetup; flow: SkillFlow | null; on: { install(): void; remove(): void; apply(): void; toggleDiff(): void; reload(): void; cancel(): void } }`.

**Copy for the row** (a `<section aria-label="marshell skill">`):

| `setup.skill` | Line | Button |
| --- | --- | --- |
| `none` | `Not installed.` | `Install the marshell skill` |
| `installed` | `Installed in <skill_path>.` | `Remove the marshell skill` |
| `outdated` | `A newer version is ready.` | `Update the marshell skill` |
| `edited` | `Changed since Marshell wrote it. Updating shows what changes.` | `Update the marshell skill` |

- An open flow shows the preview's diff toggle and the apply button `Apply`, using the same diff view and busy state as `SetupPanel`.
- The Announcer says:
  - after an install: `Installed the marshell skill for <CLI name>.`;
  - after an update: `Updated the marshell skill for <CLI name>.`;
  - after a removal: `Removed the marshell skill for <CLI name>.`;
  - after a conflict: `The skill file changed. Reload to see it.`.
- Errors use `failureText`.

- [ ] **Step 1: Write the failing tests**:
  - `SkillRow.test.tsx`: each of the four states renders its line and button text exactly as in the table, through `renderToStaticMarkup`. The `installed` state shows the path. An open flow renders `Apply` and the diff toggle. No `outline` or `border` style is added (`noBoxedOutlines.test.ts` passes).
  - `consent.test.ts`:
    - a Claude detection with `skill: "none"`, `skillPath: "C:\\u\\.claude\\skills\\marshell\\SKILL.md"` and all switches on → the headline ends with `Add the marshell skill in C:\u\.claude\skills\marshell\SKILL.md.`;
    - with `skill: "edited"` → `Update the marshell skill in …`;
    - with `skill: "installed"` → no skill sentence;
    - every switch off, including skill → `Change nothing. Claude Code will show limited status.`.
  - `agentsState.test.ts`: `setupWant` of a setup with hooks installed and `skill: "none"` → `{ status: false, statusline: false, approve: true, skill: true }`. `installRequest(DEFAULT_WANT, "codex").skill === true`.
- [ ] **Step 2: Run** `pnpm test src/features/agents src/features/onboarding`. Expected: FAIL.
- [ ] **Step 3: Implement** the API calls, the types, `consent.ts`, `agentsState.ts`, the switch, `useSkillFlow`, `SkillRow` and its place in `AgentList`. Colours come from tokens. Forced colours use system colours only. Every control is reachable with Tab and works with Enter and Space.
- [ ] **Step 4: Run** `pnpm test` (all pass) and `pnpm build` (ok).
- [ ] **Step 5: Commit**: `feat(agents): offer, update and remove the marshell skill in Settings and onboarding`.

### Task 5: Live check

Run the `marshell-live-check` skill with a new scratch folder `lc-6h2-1`. Use `-CodexHome %TEMP%\lc-6h2-cx`, plus `-CodexAuth` and `-AgyAuth` if the user agrees. The skill files then land under the scratch `cc` (Claude, through `CLAUDE_CONFIG_DIR`) and the scratch `home` (Codex and agy, through `USERPROFILE`). Before every Allow, read the target paths in the preview. Abort if a path is outside the scratch folder.

Checks for `docs/research/2026-10-30-marshell-skill-live-check.md`:
1. Onboarding's consent card shows the skill switch and the headline sentence. Allow writes all three files under the scratch homes and nothing else (compare the modified times of the real `~/.claude/settings.json`, `~/.agents`, `~/.gemini` and `~/.codex`).
2. Each CLI lists the skill: Claude `/skills`, Codex `/skills`, agy `/skills`.
   - Record whether agy lists it twice (the research's open item, decision 2).
   - Record whether Claude 2.1.314 reads it under `CLAUDE_CONFIG_DIR`.
   - Record whether Codex reads `~/.agents` from `USERPROFILE`.
3. With the user's OK, since it needs a real model answer, ask the Claude session "List my Marshell sessions." It should run `marshell list --json` and answer from it.
4. Settings:
   - edit the Codex skill file by hand → the row says `Changed since Marshell wrote it. Updating shows what changes.`;
   - Update shows the diff and restores the file;
   - Remove deletes the file and its folder;
   - the Announcer lines match the copy.
5. `marshell uninstall --yes` with `MARSHELL_HOME` and `USERPROFILE` set to the scratch folders removes the two skills Marshell wrote, and leaves an edited one with the Left line.
6. Keyboard only, and emulated forced colours, on the skill row and switch.

Findings get a failing-first test and a fix on the branch, or go to the user as limits. Show the copy in this plan to the user for approval with the results.
