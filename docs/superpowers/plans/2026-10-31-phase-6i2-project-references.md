# Project references (slice 6i2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** when a prompt names one of a project's aliases, the agent is told which file or folder it means, in one short factual note, and the session header shows that it happened.

**Architecture:**
- **Storage:** a project's aliases live in `<home>/projects/<id>/references.json`, keyed by the main repo root, and are edited through `GET/PUT /v1/references`.
- **Resolver:** the existing `UserPromptSubmit` event hook (Claude Code, Codex) and `PreInvocation` (agy, after a spike) already reach `POST /v1/hook`. The core matches the prompt against the tab's project aliases, keeps no prompt text, and answers 200 with the CLI's own context JSON. The bridge passes that answer to stdout after checking its shape.
- **No CLI config changes:** no hook entry changes, so nothing needs re-consent or Codex hook trust.
- **UI:** a "Projects" section in Settings edits the aliases, and a header chip shows the note.

**Tech Stack:** Rust (axum core, `marshell-hook` bridge), ts-rs protocol types, React + Vitest.

**Spec:** `docs/PLAN.md` §9 "Slice 6i2 decisions (2026-10-31)" (binding), brief `docs/BRIEF.md` §5.11 (its Rules), research `docs/research/2026-10-31-prompt-context-injection.md`.

## Global Constraints

- **Rules from brief §5.11:**
  - "never more than about 300 tokens per injection": the note is capped at 1,200 characters and 8 aliases.
  - "plain factual statements, not instructions".
  - "a per-project toggle", off until turned on.
  - "never blocks a prompt for more than 100 ms": the resolver is in memory, plus one small file read; the bridge's Event budget is 400 ms (`crates/hook/src/post.rs`).
- **Privacy:** the core keeps no prompt text. The resolver reads `prompt` (Claude Code, Codex) or `lastUserInput` (agy) before `antigravity::strip_personal` runs, matches, and drops it. The text is never logged, stored or put in `SessionInfo`.
- **No match means no output:** the core answers 204, and the bridge prints nothing.
- **Codex output is always JSON:** plain stdout that looks like JSON but doesn't parse fails Codex's hook.
- **Routes:** new routes sit in the token-guarded router with a "refused without the bearer" test. No route takes a file path to write: the file's location comes from a hash of the root, never from the request. Never hold a `MutexGuard` across `.await` or I/O, and file I/O runs in `spawn_blocking`.
- **Multi-CLI:** per-CLI output shapes live in each agent's module (`agents/claude`, `agents/codex`, `agents/antigravity`). The resolver itself is CLI-neutral.
- **UI:** keyboard alone and forced colors; tokens only; the exact copy below; sentence case, no "!" or "successfully". `Announcer` is the only live region.
- **Commits:** `cargo fmt --all` before Rust commits; `cargo -j 4`; `pnpm check:generated` after protocol changes. Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

### Copy (exact)

| Where | Text |
| --- | --- |
| Settings nav | `Projects` |
| Project picker label | `Project` |
| Toggle | `Add references to prompts` |
| Toggle help | `When a prompt names an alias, Marshell tells the agent which file or folder it means. Off until you turn it on.` |
| Column heads | `Alias`, `Path in the project` |
| Add button | `Add alias` |
| Remove button (aria-label) | `Remove <alias>` |
| Empty list | `No aliases yet.` |
| No projects | `Start a session in a folder first; its project shows here.` |
| Broken path | `Not found in the project, so it isn't sent.` |
| Save button / saved state | `Save` / `Saved.` |
| Refusals (422) | `An alias needs a name.`, `An alias needs a path.`, `A name can be at most 40 characters.`, `Two aliases are named "<name>".`, `"<path>" isn't inside the project.`, `A project can have at most 100 aliases.` |
| Header chip | `1 reference` / `<n> references`; its tooltip and accessible description is the note. |
| Palette command | id `edit-references`, label `Edit this project's references`, group `Tools` |
| agy, only if the spike fails | `Antigravity can't take references yet.` |
| The note sent to the agent | `References: '<alias>' = <absolute path>; '<alias>' = <absolute path>.` |

## Review Focus

1. **Alias matching at word edges:** an alias in another case ("Login Button") matches. One inside a longer word ("loginbuttons", "relogin button") doesn't, and neither does a plural ("login buttons"). A shorter alias inside a longer matched one ("login" within "login button") is not added twice. Task 1 tests each.
2. **A broken or escaping path:** an alias whose path is gone, or that resolves outside the checkout (`..`, an absolute path, or a link out), is never sent and shows as broken. Tasks 1 and 2 test it.
3. **A worktree session:** a session in a linked task worktree uses the main repo's aliases, but its note names paths in its own checkout, checked to exist there. Task 3 tests it.
4. **A slow, absent or unreadable references file:** the prompt goes on with nothing added, and the bridge prints nothing on 204, an error or a timeout. Task 3 tests it.
5. **Codex's JSON parse rule, and agy's repeated `PreInvocation`:** Codex always gets valid JSON. agy gets the note once per prompt, not once per model call. Tasks 3 and 5 test them.

---

### Task 1: The references core (store and resolver)

**Files:**
- Create: `crates/core/src/references.rs` (add `pub mod references;` in `crates/core/src/lib.rs`)
- Modify: `crates/protocol/src/api.rs` (types below; ts-rs export like the neighbours)
- Test: unit tests in `crates/core/src/references.rs`

**Interfaces:**
- Produces (protocol):
  ```rust
  pub struct Alias { pub name: String, pub path: String }                 // path: relative, '/'-separated
  pub struct AliasInfo { pub name: String, pub path: String, pub exists: bool }
  pub struct ProjectReferences { pub root: String, pub enabled: bool, pub aliases: Vec<AliasInfo> }
  pub struct ReferencesUpdate { pub root: String, pub enabled: bool, pub aliases: Vec<Alias> }
  pub struct ReferenceNote { pub names: Vec<String>, pub note: String }
  ```
- Produces (core):
  - `pub fn project_id(root: &Path) -> String`: the first 16 hex characters of the SHA-256 of the canonical root, lowercased on Windows.
  - `pub fn load(home: &Path, root: &Path) -> anyhow::Result<RefDoc>`: a missing file gives an empty, disabled doc.
  - `pub fn save(home: &Path, update: &ReferencesUpdate) -> Result<(), String>`: the `Err` is one of the 422 sentences.
  - `pub fn info(home: &Path, root: &Path) -> anyhow::Result<ProjectReferences>`: `exists` is checked against `root`.
  - `pub fn resolve(doc: &RefDoc, checkout: &Path, prompt: &str) -> Option<ReferenceNote>`
  - `RefDoc { root: String, enabled: bool, aliases: Vec<Alias> }`: `Versioned`, VERSION 1, kept in `<home>/projects/<id>/references.json` through `crate::store`. A doc whose `root` differs from the asked root is treated as missing (a hash collision).

- [ ] **Step 1: Write the failing tests.** In `references.rs`:
  - `matches_whole_words_in_any_case`: `"Fix the Login Button now"` matches alias `login button`, and the note is `References: 'login button' = <checkout>/src/LoginButton.tsx.` with the platform's absolute path.
  - `ignores_part_of_a_word_and_plurals`: the prompts `"loginbuttons"`, `"relogin button"` and `"login buttons"` give None.
  - `a_shorter_alias_inside_a_longer_match_is_skipped`: aliases `login` and `login button`, prompt `"the login button"`, give names `["login button"]`.
  - `aliases_are_listed_in_prompt_order_and_capped`: 10 matching aliases give 8 names; the note is at most 1,200 characters, and it is cut at a whole alias, never mid-path.
  - `a_missing_or_escaping_path_is_never_sent`:
    - a path that doesn't exist gives None;
    - `../x` and an absolute path give None;
    - on unix, a symlink pointing outside the checkout gives None. Resolve with `std::fs::canonicalize` and require the result to stay under the canonical checkout.
  - `disabled_sends_nothing`: `enabled: false` gives None, even when an alias matches.
  - `save_refuses_bad_updates`: each 422 sentence from the copy table, exactly, from:
    - an empty name or path;
    - a name over 40 characters;
    - two names that differ only in case;
    - `..` or an absolute path;
    - more than 100 aliases.

    Names and paths are trimmed. `\` in a path is stored as `/`.
  - `save_then_load_round_trips` and `info_marks_broken_paths`.
  - `the_file_lives_under_the_root_hash`: the doc is at `projects/<project_id(root)>/references.json`, with `project_id` stable for the same root spelled with another case on Windows.
- [ ] **Step 2: Run them to watch them fail.**
  Run: `cargo test -j 4 -p marshell-core --lib references::`
  Expected: it doesn't compile (`references` module missing).
- [ ] **Step 3: Implement.** Matching lowercases both sides with `to_lowercase`. An occurrence counts only when the characters just before and after it are not `char::is_alphanumeric` (or are the text's edges). Try the longest aliases first, and skip an occurrence that overlaps a span already taken. Order the result by first position in the prompt. Write the note with `'` around the names and paths joined by `; `, ending with `.`.
- [ ] **Step 4: Run them to watch them pass**, plus `cargo test -j 4 -p marshell-protocol` (it regenerates `src/generated/`).
  Expected: PASS. `pnpm check:generated` is ok.
- [ ] **Step 5: Commit.**
  `git add crates/core/src/references.rs crates/core/src/lib.rs crates/protocol/src/api.rs src/generated`
  `feat(references): store a project's aliases and match a prompt against them`

### Task 2: The references routes and API client

**Files:**
- Create: `crates/core/src/server/references.rs`
- Modify: `crates/core/src/server/mod.rs` (router, next to `/v1/scripts`)
- Create: `src/lib/api/references.ts`, `src/lib/api/references.test.ts`
- Test: `crates/core/tests/references_api.rs`

**Interfaces:**
- Consumes: Task 1's `info`, `save` and protocol types.
- Produces:
  - `GET /v1/references?cwd=<abs path>` gives `ProjectReferences` for `projects::project_of(cwd)`.
  - `PUT /v1/references` with a `ReferencesUpdate` body gives 204. A refusal is a 422 with the sentence.
  - A non-absolute `cwd` or `root` is a 400 "not a full path", and a root that isn't a folder is a 400 `"<root>" isn't a folder.`
  - TS: `getReferences(endpoint, cwd): Promise<ProjectReferences>`, `putReferences(endpoint, update): Promise<void>`.

- [ ] **Step 1: Write the failing tests** in `references_api.rs`, following `scripts_api.rs`:
  - `get_and_put_refuse_without_the_bearer` (401 for both);
  - `put_then_get_round_trips`;
  - `a_worktree_cwd_reads_the_main_repos_references`;
  - `put_refuses_a_path_outside_the_project` (422 with the sentence);
  - `relative_cwd_is_a_400`.

  In `references.test.ts`, test the URL, method and body, as `scripts.test.ts` does.
- [ ] **Step 2: Run them to watch them fail.**
  Run: `cargo test -j 4 -p marshell-core --test references_api` and `pnpm test src/lib/api/references.test.ts`
  Expected: compile error / module not found.
- [ ] **Step 3: Implement** the handlers. They do their file work in `spawn_blocking`, and the PUT re-canonicalizes `root` through `projects::project_of`.
- [ ] **Step 4: Run them to watch them pass.**
  Expected: PASS.
- [ ] **Step 5: Commit:** `feat(references): read and save a project's aliases through the local API`

### Task 3: The note reaches Claude Code and Codex

**Files:**
- Modify:
  - `crates/core/src/server/hook.rs`: read the prompt before `strip_personal`; after the fold, answer `UserPromptSubmit`.
  - `crates/core/src/agents/claude/mod.rs` and `agents/codex/mod.rs`: the context JSON.
  - `crates/hook/src/lib.rs`: pass an Event 200 through after the check.
  - Create `crates/hook/src/context.rs`: `accepted`.
  - `crates/protocol/src/api.rs` + `crates/core/src/registry.rs`: `SessionInfo.references: Option<ReferenceNote>`, cleared by Restart and resume like `setup`.
- Test: `crates/core/tests/hook.rs`, `crates/hook` unit tests, `registry.rs` tests

**Interfaces:**
- Consumes: Task 1's `load` and `resolve`.
- Produces:
  - `agents::claude::context_output(note: &str) -> serde_json::Value` and `agents::codex::context_output(note: &str) -> serde_json::Value`, both `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":note}}`.
  - Bridge: `context::accepted(agent: &str, answer: &[u8]) -> bool`. It is true only for that exact object shape for claude/codex, and for agy's shape (Task 5), with no other top-level keys. The bridge prints the answer only for role Event with status 200 and `accepted`, and is neutral otherwise.
  - `registry.set_references(tab, Option<ReferenceNote>)`.

- [ ] **Step 1: Write the failing tests.**
  - In `tests/hook.rs`:
    - `a_prompt_naming_an_alias_gets_its_note`: an open Claude tab whose cwd is a repo with an enabled alias. `UserPromptSubmit` with `prompt: "fix the login button"` gives a 200 with Claude's JSON, and the session list's `references.names` is `["login button"]`.
    - The same for Codex: `codex_gets_the_same_note_as_json`.
    - `no_match_is_a_204_and_clears_the_chip`: a second prompt with no alias gives 204 and `references: None`.
    - `a_worktree_tab_gets_paths_in_its_own_checkout`.
    - `a_disabled_project_or_unreadable_file_is_a_204`: a broken JSON references file gives a 204 and no panic.
    - `the_prompt_is_not_kept`: after the call, neither the hook log's record nor `SessionInfo` contains the prompt text. For agy, `strip_personal` still runs.
    - `a_tab_not_opened_by_marshell_gets_nothing`: no `X-Tab`, or an unknown tab, gives 204.
  - In `crates/hook`:
    - `an_event_answer_is_printed_only_when_it_is_context`: Claude's shape is printed, while `{"decision":"block"}`, extra keys or invalid JSON print nothing.
    - `a_204_prints_nothing`.
  - In `registry.rs`: `restart_clears_references`.
- [ ] **Step 2: Run them to watch them fail.**
  Run: `cargo test -j 4 -p marshell-core --test hook` and `cargo test -j 4 -p marshell-hook`
  Expected: FAIL (204 where 200 is expected; `references` field missing).
- [ ] **Step 3: Implement.**
  - The hook log's record must not carry the prompt either. Remove `prompt` from the logged copy for claude and codex in the same place `strip_personal` runs. This is a ruling: it also changes the diagnostics log, so check `tests/diagnostics.rs` still passes.
  - The checkout is `scripts::checkout_root(cwd_of_tab)` and the project root is `projects::project_of(cwd)`.
  - Load and resolve in `spawn_blocking`, and set `references` on every `UserPromptSubmit`, matched or not.
- [ ] **Step 4: Run them to watch them pass**, plus `cargo test -j 4 -p marshell-core --test diagnostics`.
  Expected: PASS.
- [ ] **Step 5: Commit:** `feat(references): add a matched alias's note to Claude Code and Codex prompts`

### Task 4: Spike: agy's `PreInvocation` `injectSteps` (live)

**Files:**
- Create: `docs/research/2026-10-31-antigravity-context-spike.md`

This is a hand-run check in the live-check app (`marshell-live-check` skill) with a fresh scratch folder. **Ask the user before starting it.** It needs `-AgyAuth` (a copy of agy's sign-in files into the scratch home) and real model answers.

- [ ] **Step 1:** Start the app with `live-check.ps1 start -Scratch <new literal path> -AgyAuth`, and skip onboarding's hook setup. In `<scratch>\home\.gemini\config\hooks.json`, write one `PreInvocation` hook. It appends its stdin and a timestamp to `<scratch>\spike.log`, then prints `{"injectSteps":[{"ephemeralMessage":"Spike fact: the codeword is TANGERINE-42."}]}`.
- [ ] **Step 2:** In a shell tab, run `agy` in `<scratch>\proj`.
  - Prompt 1: "What is the codeword? Answer in one line."
  - Prompt 2: "List the files here, then tell me the codeword again."
  - Then a third prompt with the hook changed to sleep 2 s first.
- [ ] **Step 3: Record:**
  - whether the answer names TANGERINE-42;
  - how many `PreInvocation` calls each prompt made, with their `invocationNum` and `lastUserInput`;
  - whether the codeword shows in agy's terminal;
  - whether it appears in `transcript.jsonl` and `transcript_full.jsonl`;
  - whether the 2 s hook delayed the answer.
- [ ] **Step 4:** Write the note: a results table and the rule for "once per prompt" (for example `invocationNum == 1`, or a change of `lastUserInput`). End with a verdict: build (Task 5a) or say so (Task 5b). Stop the tree by PID, leave the scratch folder for the final cleanup, and commit: `docs(research): record the Antigravity context spike`

### Task 5: agy references (5a), or the line that says why not (5b)

**5a, if the spike passes:**
- **Files:** `crates/core/src/agents/antigravity/mod.rs` (`context_output(note) -> {"injectSteps":[{"ephemeralMessage": note}]}`), `server/hook.rs` (`PreInvocation` answered once per prompt by the spike's rule), `crates/hook/src/context.rs` (agy's shape accepted).
- **The once-per-prompt rule:**
  - If the rule needs state, the registry keeps only a per-tab marker: the `invocationNum` seen, or a SHA-256 of `lastUserInput`, never the text.
  - The chip is set on the prompt's first invocation, and cleared on a new prompt with no match.
- **Tests (first, RED then GREEN):**
  - in `tests/hook.rs`: `antigravity_gets_the_note_once_per_prompt` (a second `PreInvocation` of the same prompt gives 204) and `antigravity_input_is_still_stripped`;
  - in `crates/hook`: agy's shape is accepted.
- **Commit:** `feat(references): add a matched alias's note to Antigravity prompts`

**5b, if it fails:** the Projects section shows the agy line from the copy table when agy is installed. Test it in Task 6's section test. Ledger the ruling.

### Task 6: The Projects section in Settings, and the palette command

**Files:**
- Create: `src/features/settings/ProjectsSection.tsx`, `ProjectsSection.css`, `ProjectsSection.test.tsx`, `projectsDoc.ts`, `projectsDoc.test.ts` (pure edit and validation helpers)
- Modify:
  - `src/features/settings/settingsSections.ts` (`projects`, after `history`)
  - `LiveSettings.tsx` (render), `settingsRouting.ts` (open at a project)
  - `src/lib/keymap.ts`, `src/features/sessions/LivePalette.tsx` and `LiveApp.tsx` (`edit-references`, opened like `run-script`, with the focused session's cwd)

**Interfaces:**
- Consumes: Task 2's `getReferences`/`putReferences` and `GET /v1/projects`.
- Produces:
  - `paletteSection("edit-references")` returns `"projects"` (`settingsRouting.ts`), so the palette opens Settings there the way other palette ids do.
  - LiveApp passes the focused session's cwd as the section's `initialCwd`, the project it preselects.

**Behaviour:**
- The picker lists the recent projects (their `name`, with the path as a description), preselected to the routed cwd's project.
- The toggle and the alias rows each have a name field, a path field and a remove button.
- **Save** sends the whole list. A 422 shows its sentence under the list, and focus goes to it. `Saved.` goes through the `Announcer`.
- A row with `exists: false` shows the broken line beside the path.
- Every control is reachable by keyboard.
- Follow `ProfilesSection`'s structure and `ChangeBar` if it fits.
- Check `noScrollInMainWindow.test.ts` and `noBoxedOutlines.test.ts` (the Settings page body already scrolls).

- [ ] **Step 1: Write the failing tests:**
  - `projectsDoc.test.ts`: add, remove and rename rows; trimming; client-side duplicate detection mirroring the 422 sentences.
  - `ProjectsSection.test.tsx` (`renderToStaticMarkup`):
    - the copy table's texts;
    - the empty and no-projects states;
    - the broken line;
    - the remove button's aria-label;
    - (5b) the agy line.
  - Palette test: `edit-references` is listed only when a session is focused.
- [ ] **Step 2: Run them to watch them fail.**
  Run: `pnpm test src/features/settings src/features/sessions`
  Expected: FAIL.
- [ ] **Step 3: Implement.** Hand the CSS to the frontend-design and good-css skills first.
- [ ] **Step 4: Run them to watch them pass**, plus `pnpm test src/styles` and `pnpm build`.
- [ ] **Step 5: Commit:** `feat(settings): edit a project's references in a Projects section, and open it from the palette`

### Task 7: The header chip

**Files:**
- Create: `src/features/references/ReferencesChip.tsx`, `ReferencesChip.css`, `ReferencesChip.test.tsx`
- Modify: `src/features/window/SessionHeader.tsx` (beside `PortChips`)

**Interfaces:** consumes `SessionInfo.references` (Task 3).

**Behaviour:**
- The chip shows only when `references` is set.
- It is a focusable element whose label is `1 reference` or `<n> references`. Its tooltip and `aria-describedby` text is the note.
- Forced colors use system colors. Style it like the port chips, by fill, with no outline.

- [ ] **Step 1: Write the failing test.** It checks the label for 1 and for 3, that the note is in the description, nothing for `null`, and that the chip is in the header markup.
- [ ] **Step 2: Run it to watch it fail.**
  Run: `pnpm test src/features/references src/features/window`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run it to watch it pass**, plus `pnpm test src/styles`.
- [ ] **Step 5: Commit:** `feat(references): show the note a prompt got as a chip in the session header`

### Task 8: Live check

Use `marshell-live-check` with a new scratch folder and a scratch repo. Ask the user before any check that needs a real agent answer. Exit criteria:

1. Turn references on for the scratch repo in Settings → Projects, add `login button` → `src/LoginButton.tsx`, and save. `references.json` appears under the scratch home's `projects/<id>/`.
2. In a Claude Code tab, then a Codex tab, a prompt naming "the Login Button":
   - the header shows `1 reference`, whose tooltip is the note;
   - the CLI's terminal shows no hook text;
   - asked "which file did Marshell point you to?", the agent names the path.
3. A prompt with no alias clears the chip, and nothing is added.
4. Deleting the file shows the broken line, and the next prompt gets nothing.
5. A task worktree session gets the path in its own checkout.
6. agy: as Task 5 decided (the note once per prompt, or the line in Settings).
7. Keyboard only, and emulated forced colors, on the section and the chip.

Write `docs/research/2026-10-31-project-references-live-check.md` and commit it.
