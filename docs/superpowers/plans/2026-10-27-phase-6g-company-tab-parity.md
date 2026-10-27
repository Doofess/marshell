# Slice 6g: company tab parity and polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task, dispatching each task that edits more than a couple of files to `worker`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Antigravity's company tab edits its settings file the way Claude Code's and Codex's do, and the deferred UX minors from slice 6f are fixed for all three CLIs.

**Architecture:** The core's settings view says why raw editing is off (a new `RawLock`), strips a file's byte-order mark from the text it hands out, and treats Antigravity's `settings.json` as a strict-JSON file like Claude Code's. The plan and undo routes stop refusing Antigravity, and a raw save puts the BOM back when the file had one. The frontend drops its Antigravity branches, shows the new lock lines, keeps odd values as they are, and moves focus back to the person's last edit.

**Tech Stack:** Rust (`marshell-protocol`, `marshell-core`, axum, `jsonc-parser`, `toml_edit`, ts-rs), React and TypeScript, Vitest with `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, "Slice 6g decisions (2026-10-27)" (decisions 1–7), with "Slice 6f decisions (2026-10-27)" for what already exists. Antigravity's file is described in `docs/research/antigravity-notes/02-configuration-and-hooks.md`, section "The settings file, verified 2026-10-27".

## Global Constraints

- Every write is preview → diff → apply through the config service; JSON only through the jsonc CST, TOML only through `toml_edit`; an unparsable file is never edited; one snapshot per apply. (`.claude/rules/config-writes.md`)
- No new route. The plan and undo routes stay token-guarded, take no file path, and run file work in `spawn_blocking`. Don't touch `guard.rs` or token handling.
- Protocol types live in `crates/protocol`; run `cargo test -j 4 -p marshell-protocol` after a change, commit `src/generated/`, and `pnpm check:generated` must pass.
- No test or live check edits the real `~/.claude/`, `~/.codex/`, `~/.gemini/` or `~/.marshell/`. Tests reach Antigravity's file only through `with_agy_home(<temp>)`.
- Copy is exact, from PLAN §9 slice 6g; no other new copy. Sentence case, no "!".
- Every UI change works with the keyboard alone and in forced colours; a `forced-colors` rule uses system colours only; tokens only; `Announcer` is the only live region. Read the good-css skill before writing CSS.
- cargo always with `-j 4`; `cargo fmt --all` before each Rust commit. Commits read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Exact copy used in this slice:
- Model help (Antigravity): "The model a new Antigravity session starts with, by the name /model shows, such as Gemini 3.1 Pro."
- Telemetry lock: "Antigravity keeps this with your Google account, not in this file. Change it with /config in Antigravity."
- Raw lock, not plain JSON (comments or a trailing comma): "Raw editing is off: this file isn't plain JSON, and Marshell saves it as plain JSON."
- Raw lock, repeated key: "Raw editing is off: a key appears twice in this file."
- Raw lock, hidden (existing): "Raw editing is off: this file holds secret values, and they show as hidden here."
- Undo result (existing): "Put <path> back as it was."
- Number error (existing): "Use a whole number of at least N."
- Change title (mirrors "Change Claude Code settings" and "Change Codex settings"): "Change Antigravity settings".
- Running warning (Task 8 only): "Antigravity is running in 1 tab. If it saves its settings, it may undo this change." / "Antigravity is running in <n> tabs. If it saves its settings, it may undo this change."

## Review Focus

1. A file that starts with a BOM, saved from the raw editor: the written file starts with exactly one BOM. A file without one gets none, and a draft that pastes a BOM in doesn't add a second one.
2. Antigravity with no `settings.json` and no `antigravity-cli` folder yet (a fresh install): a form edit creates the file, and Undo removes it again.
3. A Claude Code or Antigravity file with a comment that holds a secret and a repeated key: the raw lock is `hidden` (a secret wins), and the view's `text` is null, so the file's text never reaches the page.
4. After Reload or Discard, the control to focus no longer exists, because it is now locked or the view switched to Form. Focus falls back to the section's own fallback, and nothing throws.
5. An odd value left untouched is never written: a switch holding `"yes"`, a number holding `"abc"`, and an Antigravity `colorScheme` of `"dracula"` all plan no edit until the person changes them.

---

### Task 1: The core's view says why raw editing is off, strips the BOM, and covers Antigravity

**Files:**
- Modify: `crates/protocol/src/settings_view.rs` (`FieldLock`, `AgentSettingsView`, new `RawLock`)
- Modify: `crates/core/src/agents/settings_view.rs` (`control`, `parse_masked`, `view`, tests)
- Regenerate: `src/generated/` (`RawLock.ts`, `FieldLock.ts`, `AgentSettingsView.ts`)

**Interfaces:**
- Produces:
  - `pub enum RawLock { Hidden, NotPlainJson, RepeatedKey }`, with `#[serde(rename_all = "snake_case")]` and `#[ts(export)]`, so the TypeScript type is `"hidden" | "not_plain_json" | "repeated_key"`.
  - `AgentSettingsView.raw_lock: Option<RawLock>`, serialized as `null` when None (not skipped), like `text`. It is Some only when `editable` is true and the state is `ok`.
  - `FieldLock::Account`, set on Antigravity's `enableTelemetry` row whatever its value.
  - `AgentSettingsView.text` is the file's text without a leading U+FEFF. It is Some when `raw_lock` is None or `NotPlainJson`, and None for `Hidden` and `RepeatedKey`.
  - `pub(crate) fn has_bom(text: &str) -> bool` in `settings_view.rs`, used by Task 2.
  - `control("antigravity", …)`: `model` → `Text`; `toolPermission` → `Choice["request-review", "proceed-in-sandbox", "always-proceed", "strict"]`; `enableTerminalSandbox` → `Switch`; `colorScheme` → `Choice["terminal", "light", "solarized light", "colorblind-friendly light", "dark", "solarized dark", "colorblind-friendly dark", "tokyo night"]`; `verbosity` → `Choice["high", "medium", "low"]`; `enableTelemetry` → `Switch` (its `Account` lock keeps it read-only).
  - `editable` is true for `claude`, `codex` and `antigravity` when the state is `ok` or `missing`.

- [ ] **Step 1: Write the failing tests** in `settings_view.rs`'s test module (use the module's `run(agent, text)` helper):
  - `antigravity_is_editable_with_its_controls`: `run("antigravity", "{}")` has `editable == true`. The curated controls equal the list above in order. `enableTelemetry`'s lock is `Some(FieldLock::Account)`, both when the file omits it and when the file sets `"enableTelemetry": true`.
  - `a_json_file_with_comments_has_the_not_plain_json_raw_lock`: for both `claude` and `antigravity`, `"{\n  // mine\n  \"model\": \"x\"\n}"` gives `raw_lock == Some(RawLock::NotPlainJson)` and `text == Some(<the same text>)`.
  - `a_trailing_comma_has_the_not_plain_json_raw_lock`: `"{\"model\": \"x\",}"` for `claude` gives `Some(RawLock::NotPlainJson)`.
  - `a_repeated_key_has_its_raw_lock_and_no_text`: `"{\"model\": \"a\", \"model\": \"b\"}"` for `claude` and `antigravity` gives `raw_lock == Some(RawLock::RepeatedKey)` and `text == None`.
  - `a_secret_wins_over_comments_and_a_repeated_key` (Review Focus 3): `"{\n  // api_key = \"x\"\n  \"model\": \"a\", \"model\": \"b\"\n}"` gives `Some(RawLock::Hidden)` and `text == None`.
  - `codex_comments_need_no_raw_lock`: `run("codex", "# mine\nmodel = \"x\"\n")` gives `raw_lock == None` and `text == Some(..)`.
  - `the_text_leaves_out_a_bom`: for `claude` `"\u{feff}{\"model\": \"x\"}"` and for `codex` `"\u{feff}model = \"x\"\n"`, `text` doesn't start with U+FEFF, `raw_lock == None`, and `hash` equals `hash_bytes` of the original bytes, BOM included.
  - `an_odd_antigravity_value_shows_as_it_is`: `"{\"colorScheme\": \"dracula\"}"` gives the `colorScheme` row `value == Some("dracula")` and no lock.
  - Update the existing `control("antigravity", "model") == None` assertion (around line 1104) to `Some(FieldControl::Text)`.
- [ ] **Step 2: Run them.** `cargo test -j 4 -p marshell-core settings_view`. Expected: FAIL, because `RawLock`, `raw_lock` and `FieldLock::Account` don't exist (a compile error is the expected failure here).
- [ ] **Step 3: Implement.**
  - Add `RawLock` and the two fields to the protocol. Update the doc comments of `editable`, `text` and `raw` to say what they now mean.
  - In `parse_masked`, replace `plain: bool` with `raw_lock: Option<RawLock>`, computed once per text:
    - `Hidden` when masking changed the tree or any comment fails `comment_is_clean`;
    - else, for JSON only, `RepeatedKey` when `repeats_a_key`;
    - else, for JSON only, `NotPlainJson` when `serde_json::from_str::<Value>(body)` fails on the BOM-stripped body;
    - else None.
    
    `json_hides_nothing` splits into the parts this needs.
  - In `view`:
    - set `text` from the BOM-stripped text when `raw_lock` is None or `NotPlainJson`;
    - set `raw_lock` only when editable;
    - set `editable = matches!(agent, "claude" | "codex" | "antigravity")`;
    - give `("antigravity", "enableTelemetry")` the `Account` lock in `value_of`, ahead of the `Hidden`/`NotScalar` checks.
  - Add the Antigravity arms to `control()`, and update its doc comment to name Antigravity's schema (`antigravity.google/schemas/v1/settings.json`).
- [ ] **Step 4: Regenerate and run.** `cargo test -j 4 -p marshell-protocol` then `cargo test -j 4 -p marshell-core settings_view` then `pnpm check:generated`. Expected: all pass, and `src/generated/RawLock.ts` exists.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then commit the protocol, core and generated files as `feat(settings): say why raw editing is off, leave the BOM out of the text, and give Antigravity's file its controls`.

### Task 2: The plan and undo routes edit Antigravity's file, and a raw save keeps the BOM

**Files:**
- Modify: `crates/core/src/server/agents.rs` (`settings_plan_of`, `settings_undo_of`, `form_ops`, `raw_op`, `settings_view_of`, the `READ_ONLY` const)
- Test: `crates/core/tests/agents_api.rs`, beside the existing settings plan tests (`plan_env()`; `antigravity_settings_plan_is_422` at line 2101 is replaced); reach Antigravity with `with_agy_home(<tempdir>)`

**Interfaces:**
- Consumes: Task 1's `RawLock`, `FieldLock::Account`, `has_bom`, BOM-free `text`.
- Produces:
  - `fn settings_read(st: &AppState, agent: &str, kind: Adapter) -> Result<(AgentSettingsView, bool), Fail>`. It is the view plus whether the file's text starts with a BOM, from one read. `settings_view_of` calls it and drops the bool.
  - `fn raw_op(agent: &str, view: &AgentSettingsView, had_bom: bool, text: &str) -> Result<EditOp, Refusal>`.

- [ ] **Step 1: Write the failing tests:**
  - `antigravity_form_edit_plans_applies_and_undoes`: in an agy home holding `{"model": "x"}`, plan `toolPermission = "strict"` and apply. The file now holds `"toolPermission": "strict"` and still `"model": "x"`. Undo with the snapshot id puts back the original bytes.
  - `antigravity_form_edit_creates_a_missing_file` (Review Focus 2): with an agy home with no `.gemini` folder, plan `verbosity = "low"` with `expected_hash: None` and apply. The file exists holding `"verbosity": "low"`. The atomic write makes the missing folders (`config/write.rs:70`). Undo removes the file; the folders may stay.
  - `a_codex_form_edit_keeps_the_files_bom`: in a Codex `config.toml` of `"\u{feff}model = \"a\"\n"`, a form edit of `model = "b"` writes bytes that start with exactly one `EF BB BF`. If `toml_doc.rs` drops it, re-insert it there the way `jsonc.rs:52` does for JSON.
  - `the_telemetry_row_is_refused`: a form edit of `enableTelemetry` gets 422 with the telemetry lock line from Global Constraints as its body, and nothing is written.
  - `a_raw_save_keeps_the_files_bom` (Review Focus 1): for Claude, `"\u{feff}{\"model\": \"a\"}"` with raw text `{"model": "b"}` applies to bytes that start with exactly one `EF BB BF`. The same holds when the sent text itself starts with U+FEFF. For Codex, `"\u{feff}model = \"a\"\n"` keeps one BOM. A Claude file without a BOM saved from raw text that starts with U+FEFF gets no BOM.
  - `a_raw_save_is_refused_for_each_raw_lock`: a Claude file with a comment gets 422 with the body "Raw editing is off: this file isn't plain JSON, and Marshell saves it as plain JSON.". A repeated key gets "Raw editing is off: a key appears twice in this file.". The existing secret-file test keeps its line. Nothing is written in any of the three.
  - Delete or invert the existing tests that assert Antigravity's plan and undo answer 422 with "Antigravity's settings are read-only in Marshell." They become the two tests above.
- [ ] **Step 2: Run them.** `cargo test -j 4 -p marshell-core --test agents_api`. Expected: the new tests FAIL, with Antigravity refused and the BOM lost.
- [ ] **Step 3: Implement.**
  - Remove `READ_ONLY` and both Antigravity refusals.
  - `settings_plan_of` uses `settings_read`, and its title match gets `Adapter::Antigravity => "Change Antigravity settings"`.
  - `form_ops` refuses `FieldLock::Account` with the line from the test.
  - `raw_op`:
    - refuses per `view.raw_lock`: `Hidden` keeps today's sentence, and `NotPlainJson` and `RepeatedKey` use the two lines from the test;
    - strips one leading U+FEFF from `text` before `text_problem`;
    - prepends U+FEFF to the `ReplaceText` content when `had_bom`.
  - Check that `masked_diff` and the undo path treat `"antigravity"` as JSON. The non-Codex arm should already do so; if a match names `"claude"` alone, widen it.
- [ ] **Step 4: Run.** `cargo test -j 4 -p marshell-core`. Expected: all pass, including the existing "refused without the bearer" tests for both routes.
- [ ] **Step 5: Commit.** Run `cargo fmt --all`, then commit `feat(settings): edit Antigravity's settings file and keep a file's BOM on a raw save`.

### Task 3: The live-check script never leaves Antigravity on the real home

Now that the tab writes Antigravity's file, `start` without `-AgyAuth` would point the dev app at the real `~/.gemini`.

A scratch `USERPROFILE` doesn't move the config service's path guard. `Guard::for_home` takes its root from `dirs::home_dir()`, which on Windows asks for `FOLDERID_Profile` and doesn't read the variable. So the scratch folders under `%TEMP%` stay inside the allowed real home, as they were in 6f's live check.

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1` (around lines 150–195, and the header at 19–21)
- Modify: `.claude/skills/marshell-live-check/SKILL.md` (step 3's description of `-AgyAuth`)

- [ ] **Step 1: Change `start`:**
  - It always creates `<Scratch>\home\.gemini` and sets `USERPROFILE` to `<Scratch>\home`, with `CARGO_HOME` and `RUSTUP_HOME` pinned as they are today.
  - `-AgyAuth` now only adds the two copies.
  - `pids.json` records `agyHome` every time.
  - `stop` still deletes only the copies, and only when `agyAuth` is set.
- [ ] **Step 2: Update** the header comment and SKILL.md step 3 to say so.
- [ ] **Step 3: Verify.** Read the diff. Expected: every path through `start` sets `USERPROFILE` to `<Scratch>\home` before `pnpm tauri dev` runs. Task 7's `start` is the run that proves it: its output prints the `USERPROFILE=` line.
- [ ] **Step 4: Commit** `chore(live-check): always start the app with a scratch USERPROFILE, so Antigravity's file is never the real one`.

### Task 4: The form and raw view for all three CLIs

**Files:**
- Modify: `src/features/companies/CompanySection.tsx` (`SWITCH_OPTIONS`, `lockLine`, `EditableRow`, the raw branch at 213–231, `onReview`, `onUndo`)
- Modify: `src/features/companies/companyEdit.ts` (`draftValue`, new `switchOptions`)
- Modify: `src/features/companies/companyFlow.ts` (`rawChange`)
- Modify: `src/features/companies/companyFields.ts` (Antigravity's `model` help)
- Test: `CompanySection.test.tsx`, `companyEdit.test.ts`, `companyFlow.test.ts`, `companyFields.test.ts`

**Interfaces:**
- Consumes: generated `RawLock`, `AgentSettingsView.raw_lock`, `FieldLock` `{ kind: "account" }`.
- Produces:
  - `switchOptions(current: string): Array<{ value: string; label: string }>` in `companyEdit.ts`. It returns `SWITCH_OPTIONS` (moved there, unchanged), plus `{ value: current, label: \`${current} (from the file)\` }` when `current` isn't `""`, `"on"` or `"off"`.
  - `draftValue` keeps `"true"` → `"on"` and `"false"` → `"off"` for a switch, and returns any other value as it is.
  - `rawLockLine(lock: RawLock): string` in `CompanySection.tsx`, with the three lines from Global Constraints.

- [ ] **Step 1: Write the failing tests:**
  - `companyEdit.test.ts`:
    - `switchOptions("yes")` ends with `{ value: "yes", label: "yes (from the file)" }`, and `switchOptions("on")` has three options.
    - `parseField` for a number control: `"-"` and `"1e"` give `{ ok: false, message: "Use a whole number of at least 1." }` (min 1); `" 45 "` is ok with 45; `""` is ok and means not set.
    - Review Focus 5: `formEdits` with an untouched switch row `"yes"`, an untouched number row `"abc"` and an untouched choice row `"dracula"`, each with the draft not holding the key, gives `edits.length === 0` and no errors.
  - `companyFlow.test.ts`: `rawChange("raw", view({ raw_lock: "not_plain_json", text: "x" }), "y")` is null, and the same with `raw_lock: null` is `"y"`.
  - `companyFields.test.ts`: Antigravity's `model` help equals the exact model help line.
  - `CompanySection.test.tsx`:
    - Antigravity with `editable: true` renders `<select` for `toolPermission` and Review changes when drafted.
    - The `enableTelemetry` row with `lock: { kind: "account" }` renders the telemetry lock line and no control.
    - A number row renders `type="text"` and `inputmode="numeric"` and never `type="number"`.
    - A switch row holding `"yes"` renders `<option value="yes" selected="">yes (from the file)</option>`.
    - Raw mode with `raw_lock: "not_plain_json"` and `text` set renders the file's text read-only (no editable textarea) and the not-plain-JSON line.
    - Raw mode with `raw_lock: "repeated_key"` renders the repeated-key line.
    - Raw mode with `raw_lock: "hidden"` renders today's hidden line.
    - Replace the "renders Antigravity read-only" test with these.
- [ ] **Step 2: Run them.** `pnpm test src/features/companies`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `EditableRow`:
    - the number control is `<input type="text" inputMode="numeric" …>`, and the browser's `min` goes;
    - a switch uses `switchOptions(input)`;
    - `lockLine` gets `case "account"` with the telemetry line.
  - The raw branch:
    - an editable textarea only when `editable && view.text !== null && view.raw_lock === null`;
    - with `raw_lock === "not_plain_json"`, the file's own `view.text` read-only (the Codex-style read-only textarea for every agent), then the not-plain-JSON line;
    - otherwise today's masked render with the "Shown as Marshell reads it" note, plus `rawLockLine(view.raw_lock)` when it is set.
  - Remove both `agent === "antigravity"` checks and the comment on `onReview`.
  - `rawChange` also needs `view.raw_lock === null`.
  - Update Antigravity's model help.
- [ ] **Step 4: Run.** `pnpm test src/features/companies`, then `pnpm test`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(companies): edit Antigravity's settings, show odd values as they are, and say why raw editing is off`.

### Task 5: Focus goes back to the last edit, and Undo leaves a visible line

**Files:**
- Modify: `src/features/companies/companyFlow.ts` (`CompanyReview` users: `afterUndo`, `afterEdit`, `focusAfter`, `announcement`; new `focusBack`)
- Modify: `src/features/companies/CompanySection.tsx` (`CompanyReview` gains `{ kind: "undone" }`; data-focus on controls and the raw textarea; `onDiscard`, `onReload`, `onUndo`; the result block)
- Test: `companyFlow.test.ts`, `CompanySection.test.tsx`

**Interfaces:**
- Produces:
  - `CompanyReview` gains `{ kind: "undone" }`.
  - `afterUndo({ ok: true }, path)` returns `{ kind: "undone" }`, `afterEdit` turns `undone` into `editing`, `focusAfter(_, "undone")` returns `"result"`, and `announcement({ kind: "undone" }, …, path)` returns `undoneLine(path)`. `onUndo` stops calling `announce` itself.
  - `fieldFocus(key: string): string` returns `` `field:${key}` ``, and the raw textarea's key is `"raw"`.
  - `focusBack(action: "discard" | "reload", lastEdited: string | null, lastFocused: string | null): string | null`. Discard returns `lastEdited`, Reload returns `lastFocused`.

- [ ] **Step 1: Write the failing tests:**
  - `companyFlow.test.ts`:
    - `afterUndo({ ok: true }, "p")` deep-equals `{ kind: "undone" }`;
    - `focusAfter("undoing", "undone")` is `"result"`;
    - `announcement({ kind: "undone" }, "n", "p")` is `"Put p back as it was."`;
    - `afterEdit({ kind: "undone" })` is editing;
    - `focusBack("discard", "field:model", "raw")` is `"field:model"`, `focusBack("reload", "field:model", "raw")` is `"raw"`, and `focusBack("discard", null, null)` is null.
  - `CompanySection.test.tsx`:
    - with `review: { kind: "undone" }`, the markup holds `<div class="company__result" tabindex="-1" data-focus="result"><p class="company__line">Put <path> back as it was.</p></div>`;
    - a choice row's `<select` carries `data-focus="field:<key>"`, as do number and text inputs;
    - the editable raw textarea carries `data-focus="raw"`.
- [ ] **Step 2: Run them.** `pnpm test src/features/companies`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `CompanySection` keeps two refs: `lastEdited` and `lastFocused`.
    - `lastEdited` is set in `onDraft` (`fieldFocus(key)`) and in `onRawDraft` (`"raw"`).
    - `lastFocused` is set by an `onFocus` on each control and on the textarea, passed down through a new optional `onFocusField(focusKey: string)` prop on `CompanyView` and `EditableRow`.
  - `onDiscard` and a successful `onReload` set a `pendingFocus` state to `focusBack(...)`. An effect focuses `[data-focus="<pendingFocus>"]` after render, then clears it.
  - When nothing matches (Review Focus 4: the control is now locked, or the view switched to Form), focus is left where the existing fallback puts it, and nothing throws.
  - Discard clears `lastEdited` after reading it.
  - The `undone` block renders as in the test.
- [ ] **Step 4: Run.** `pnpm test src/features/companies`, then `pnpm test`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(companies): put focus back on the last edit after Discard and Reload, and leave Undo's line on screen`.

### Task 6: Busy buttons show as unavailable in forced colours

**Files:**
- Modify: `src/features/companies/CompanySection.css` (its existing `@media (forced-colors: active)` block, line 103)
- Test: no unit test can see forced colours (Vitest has no DOM). Task 7 Step 5 checks this in the app.

- [ ] **Step 1: Read** the good-css skill's `references/interaction.md` entry for button states, and the pattern in `src/features/history/ExportDialog.css:115-123`.
- [ ] **Step 2: Add** `.company .settings__btn[aria-disabled="true"] { color: GrayText; outline-color: GrayText; }` inside the forced-colours block. Scope it to `.company`, so the other Settings sections don't change.
- [ ] **Step 3: Run.** `pnpm test`, including `noBoxedOutlines.test.ts`. Expected: all pass.
- [ ] **Step 4: Commit** `fix(companies): show a busy button as unavailable in forced colours`.

### Task 7: Live check

Use the marshell-live-check skill with a new literal scratch folder, `<scratchpad>\lc-6g-1`. Start it with `-AgyAuth`, which the owner allowed for this slice (decision 3), and a short `-CodexHome` (`%TEMP%\lc-6g-cx`). Before starting, note the modified times of the real `~/.claude/settings.json`, `~/.codex/config.toml` and `~/.gemini/antigravity-cli/settings.json` (the times only; don't open the files).

Seed through the app's own paths, with no hand edits under the real homes:
- Antigravity: the scratch `home\.gemini\antigravity-cli\settings.json`;
- Claude: the scratch `MARSHELL_CLAUDE_SETTINGS`;
- Codex: the scratch `CODEX_HOME`'s `config.toml`.

- [ ] **Step 1: The overwrite probe** (decision 3), before anything else in Antigravity's tab:
  1. Open an Antigravity tab from its profile and let it start signed in.
  2. In Marshell's Antigravity tab, set Verbosity to `low` and apply.
  3. In the agy session (one this check started), change Colour scheme through `/config`, and nothing else. Never touch Enable Telemetry or any setting the docs say is saved with the Google account: that would change the owner's real account.
  4. Read the scratch file. If `verbosity` is gone or back to the old value, the probe shows an overwrite.
  5. Record the outcome and exact file contents in the note, with no token or user name.

  Also record whether agy's start rewrote the file in its own format.
- [ ] **Step 2: Antigravity's tab.**
  - A form edit (Tool permission → `strict`), Review, Apply, and Undo to the same bytes.
  - A raw edit.
  - The Telemetry row: read-only, with the line.
  - An odd `colorScheme` value seeded as `"dracula"`, shown as "dracula (from the file)".
- [ ] **Step 3: Raw locks and the BOM.**
  - A Claude file with a comment shows the not-plain-JSON line, and its form still applies.
  - A repeated key shows its line.
  - A Claude file and a Codex file that start with a BOM: a raw save keeps exactly one BOM. Check the first three bytes of each file.
- [ ] **Step 4: Odd values and focus.**
  - A switch holding `"yes"` and a number holding `"abc"` show as they are.
  - Typing `-` in a number field shows the number error.
  - Keyboard only: Discard returns focus to the control changed last, and Reload returns it to the control focused before. A successful Undo focuses "Put <path> back as it was.".
- [ ] **Step 5: Forced colours.** With emulated forced colours, a busy Apply or Undo button reads as GrayText.
- [ ] **Step 6: Stop and write up.**
  1. Run the tree, then stop by PID, naming the agy session this check started in `-AlsoStop`.
  2. Check that the three real files' modified times are unchanged.
  3. Write `docs/research/2026-10-27-company-tab-parity-live-check.md` in the skill's format.

  Each finding gets a failing-first test and a fix commit before the next task.
- [ ] **Step 7: Commit** `docs(research): record the company tab parity live check`.

### Task 8: The running-Antigravity warning (only if Task 7's probe showed an overwrite)

If the probe showed no overwrite, skip this task and ledger `Task 8: Ruling: no overwrite seen in the probe, so no warning (decision 3) — <the probe's evidence> — cost if wrong: a running session can silently undo a Marshell change`.

**Files:**
- Modify: `src/features/companies/companyText.ts` (new `rewriteNote`)
- Modify: `src/features/companies/CompanySection.tsx` (the review section)
- Test: `companyText.test.ts`, `CompanySection.test.tsx`

**Interfaces:**
- Produces: `rewriteNote(agent: CompanyAgent, running: number): string | null`. It returns null unless `agent === "antigravity"` and `running > 0`. One running tab gives the 1-tab line from Global Constraints, and more give the `<n>` line.

- [ ] **Step 1: Write the failing tests:**
  - `rewriteNote("antigravity", 0)` is null, `rewriteNote("antigravity", 1)` is the 1-tab line, `rewriteNote("antigravity", 3)` is the 3-tab line, and `rewriteNote("claude", 2)` is null.
  - The review section renders the line in a `<p class="change-bar__note">` when the view's agent is antigravity and `running` is 2, and leaves Apply as it is.
- [ ] **Step 2: Run them.** `pnpm test src/features/companies`. Expected: FAIL.
- [ ] **Step 3: Implement** the function, and render it in the review `<section>` under the backup note.
- [ ] **Step 4: Run.** `pnpm test`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(companies): warn at Review that a running Antigravity may undo the change`.
