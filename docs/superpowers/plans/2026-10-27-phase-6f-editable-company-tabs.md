# Editable company tabs (slice 6f) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude Code's and Codex's company sections in Settings edit their user settings file, from the form's six curated keys or from the raw text, through the config service's preview, backup, atomic write and undo.

**Architecture:** The config service learns TOML edits (`toml_edit`) and a whole-text replace. Two new token-guarded routes per agent, `settings/plan` and `settings/undo`, build the ChangeSet in the core from the path the core resolves. No route takes a path. The plan route returns a preview whose diff never carries a secret. The settings view gains what the form needs: each curated key's control, any lock on it, the file's hash, and the file's own text when it holds no hidden value. The frontend renders the controls, reviews the core's diff, applies it with the existing `applyPreview`, and offers Undo.

**Tech Stack:** Rust (axum, `jsonc-parser`, `toml`, new direct dependency `toml_edit` 0.25), ts-rs, React 19, Vitest with `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, "Slice 6f decisions (2026-10-27)" 1–4, together with slice 6a decision 6, slice 6e decisions 7, 12 and 15, and §6's config service rules (`.claude/rules/config-writes.md`).

## Global Constraints

- Every write goes plan → the person sees the unified diff → apply. Edit JSON only with the core's jsonc CST, and TOML only with `toml_edit`. A file that doesn't parse is never edited.
- No route takes a file path to write. New routes go in the token-guarded router, each with a "refused without the bearer" test, and use `spawn_blocking` for file work.
- Decision 15 stands: no response, diff, error message or stored preview carries a value the view shows as `hidden`.
- Tests never touch `~/.claude`, `~/.codex`, `~/.gemini` or `~/.marshell`. They use `tempfile` and the `AppState` seams `with_claude_settings`, `with_codex_hooks` and `with_claude_compact_paths`. No test runs a real CLI.
- Protocol types live in `crates/protocol` with `#[ts(export)]`. Regenerate with `cargo test -p marshell-protocol`, and `pnpm check:generated` passes.
- `toml_edit`'s need, under SCOPE.md's dependency policy: Codex's `config.toml` must keep the person's comments, blank lines and key order through an edit (slice 6a decision 6). The `toml` crate re-serialises and drops them. `toml_edit` 0.25.17 is already in `Cargo.lock` through Tauri. It is MIT or Apache-2.0, actively maintained in toml-rs, and makes no network access. It isn't bundled as an asset, so it needs no `THIRD_PARTY_NOTICES.md` entry beyond the crates already covered.
- UI:
  - It works with the keyboard alone and in forced colours.
  - Use tokens only, and no boxed outlines.
  - `Announcer` (the `announce` prop) is the only live region.
  - An unknown value is an en dash.
  - Read the good-css skill before writing CSS.
  - Copy is exactly as written below: sentence case, no "!" and no "successfully".
- Cargo runs with `-j 4`. Run `cargo fmt --all` before every Rust commit. Commits read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A file with secrets.** A form edit to `model` in a Claude file that also holds `env.ANTHROPIC_API_KEY` and `apiKeyHelper`. The preview diff, `GET /v1/config/preview/{id}`, every 422 message and the view all leave the secret out. (Task 3.)
2. **Comments survive.** A Codex `config.toml` with comments, blank lines and a `[profiles.x]` table keeps all of them through a form edit. Undo then puts back the identical bytes. (Tasks 1 and 3.)
3. **A hand edit between opening and Apply.** If the file changes on disk between opening and Apply, the plan or apply gets 409, nothing is written, and the form keeps the person's draft. (Tasks 3 and 6.)
4. **A curated value outside the offered set.** For example `theme = "custom:ocean"`, `effortLevel` missing from the file, `approval_policy = { granular = {...} }`, or `approval_policy = "on-failure"`. It is shown as it is, and is never rewritten unless the person changes that key. A table value locks its row. (Tasks 2, 4 and 5.)
5. **A missing file.** A form edit creates it: Claude gets `{}` plus the key, and Codex gets just the key. Undo removes the file again. (Tasks 1 and 3.)

---

### Task 1: TOML edits and a whole-text replace in the config service

**Files:**
- Modify: `crates/core/Cargo.toml` (add `toml_edit = "0.25"`), `Cargo.lock`.
- Create: `crates/core/src/config/toml_doc.rs`.
- Modify: `crates/core/src/config/mod.rs`, `crates/core/src/config/preview.rs` (`apply_op`), `crates/core/src/config/managed.rs` (only if it matches on `EditOp` exhaustively).
- Modify: `crates/protocol/src/config.rs` (`EditOp`), then `src/generated/EditOp.ts` (regenerated).

**Interfaces:**
- Produces, in protocol `EditOp`:
  - `TomlSet { key: Vec<String>, value: String }`. `value` is JSON text and must be a string, an integer, a float or a bool. Anything else is refused as `BadValue`.
  - `TomlRemove { key: Vec<String> }`.
  - `ReplaceText { content: String }`. Its `FileEdit` needs `expected_hash` and an existing file, else it is refused as `BadValue`.
- Produces, in core: `config::toml_doc::apply_toml_ops(text: &str, ops: &[TomlOp]) -> Result<String, EditError>`, with `pub enum TomlOp<'a> { Set { key: &'a [String], value: &'a serde_json::Value }, Remove { key: &'a [String] } }`. It reuses `jsonc::EditError` (`DoesNotParse`, `BadPointer` for a key that runs through a non-table, `Internal`).

- [ ] **Step 1: Write the failing tests** in `toml_doc.rs` `mod tests`:
  - `set_keeps_comments_blank_lines_and_order`: `"# mine\nmodel = \"a\" # why\n\n[profiles.x]\nmodel = \"b\"\n"` with Set `["model"]` to `"c"` gives `"# mine\nmodel = \"c\" # why\n\n[profiles.x]\nmodel = \"b\"\n"`.
  - `set_adds_a_missing_top_level_key_before_the_first_table`: `"[profiles.x]\nmodel = \"b\"\n"` with Set `["sandbox_mode"]` to `"read-only"`. The new key is a top-level key: parse the result with `toml` and check `sandbox_mode` at the root, and `[profiles.x]` unchanged.
  - `set_on_an_empty_file`: `""` with Set `["check_for_update_on_startup"]` to `false` gives `"check_for_update_on_startup = false\n"`.
  - `remove_takes_the_key_and_its_line`: removing `["model"]` from `"model = \"a\"\nx = 1\n"` gives `"x = 1\n"`. Removing a missing key gives the text unchanged.
  - `values_keep_their_type`: integer `32768` writes `32768`, bool writes `true`, and a string writes a basic string with quotes escaped.
  - `a_table_or_array_value_is_refused`, and `a_key_through_a_non_table_is_bad_pointer` (`model = "a"`, Set `["model","x"]`).
  - `text_that_does_not_parse_is_refused` (`"model = \n"`).

  In `preview.rs` `mod tests`:
  - `replace_text_needs_an_expected_hash`.
  - `replace_text_on_a_missing_file_is_refused`.
  - `replace_text_with_the_right_hash_gives_the_new_bytes`.
  - `toml_set_on_a_missing_file_starts_empty`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-core --lib config::` fails, because `toml_doc` and the new ops don't exist yet.
- [ ] **Step 3: Implement** `toml_doc.rs` on `toml_edit::DocumentMut`. When Set changes an existing key, it replaces the value and keeps the decor, so a trailing comment stays. A new top-level key goes after the last top-level key, so it stays before the first `[table]`. Then add `apply_op` arms: `TomlSet` and `TomlRemove` read text as `json_text` does, with `""` for a missing file. `TomlRemove` on a missing file gives `None`, unchanged. `ReplaceText` is refused unless `working.is_some()`, and `compute` refuses it when its `expected_hash` is `None`, as `DeleteFile` is. Add the protocol variants with doc comments in the style of `JsonSet`.
- [ ] **Step 4: Run the tests and see them pass:** `cargo test -j 4 -p marshell-core --lib config::` passes, then `cargo test -j 4 -p marshell-protocol` and `pnpm check:generated` (ok).
- [ ] **Step 5: Commit:** `feat(config): edit TOML with toml_edit and replace a file's whole text against its hash`.

### Task 2: What the settings view tells the form

**Files:**
- Modify: `crates/protocol/src/settings_view.rs`, `crates/core/src/agents/settings_view.rs`, `crates/core/src/server/agents.rs` (`settings_view_of` passes what `view` needs), and the generated `src/generated/*.ts`.

**Interfaces:**
- Produces, in protocol:
  - `#[serde(tag = "kind", rename_all = "snake_case")] enum FieldControl { Text, Number { min: u32 }, Switch, Choice { values: Vec<String> } }`.  - `#[serde(tag = "kind", rename_all = "snake_case")] enum FieldLock { Managed { path: String }, NotScalar }`.
  - `SettingValue` gains `#[serde(default, skip_serializing_if = "Option::is_none")] control: Option<FieldControl>` and `lock: Option<FieldLock>`, set on curated entries only.
  - `AgentSettingsView` gains:
    - `editable: bool`: Claude and Codex only, and only when `state` is `Ok` or `Missing`.
    - `hash: Option<String>`: `config::hash::hash_bytes` of the file's bytes, `None` when missing.
    - `text: Option<String>`: the file's own text when `state` is `Ok` and masking changes nothing in it, else `None`.
- Produces, in core: `pub(crate) fn control(agent: &str, key: &str) -> Option<FieldControl>`, the single source of the controls. Task 3 validates against it.

| Agent | Key | Control |
| --- | --- | --- |
| claude | `model` | Text |
| claude | `effortLevel` | Choice `low`, `medium`, `high`, `xhigh` |
| claude | `permissions.defaultMode` | Choice `default`, `acceptEdits`, `plan`, `auto`, `dontAsk`, `bypassPermissions` |
| claude | `cleanupPeriodDays` | Number min 1 |
| claude | `autoUpdatesChannel` | Choice `stable`, `latest` |
| claude | `theme` | Choice `auto`, `dark`, `light`, `dark-daltonized`, `light-daltonized`, `dark-ansi`, `light-ansi` |
| codex | `model` | Text |
| codex | `model_reasoning_effort` | Choice `none`, `minimal`, `low`, `medium`, `high`, `xhigh` |
| codex | `approval_policy` | Choice `on-request`, `never` |
| codex | `sandbox_mode` | Choice `read-only`, `workspace-write`, `danger-full-access` |
| codex | `project_doc_max_bytes` | Number min 0 |
| codex | `check_for_update_on_startup` | Switch |

These values come from SchemaStore's `claude-code-settings.json` (read 2026-10-27) and openai/codex tag `rust-v0.164.0`. Codex's `untrusted` is refused at load, and `on-failure` is a deprecated alias, so neither is offered. Antigravity's curated entries get `control: None`, since its view is not editable.

A curated row's lock:
- `Managed { path }`: its scope is `Managed`. `path` is the managed file the value came from; `read_managed` returns it alongside the values.
- `NotScalar`: the file sets it to a table or an array (Codex's `approval_policy = { granular = … }`).

- [ ] **Step 1: Write the failing tests** in `settings_view.rs` `mod tests`:
  - `curated_claude_keys_carry_their_controls`: the six controls in the table, in order.
  - `curated_codex_keys_carry_their_controls`.
  - `antigravity_is_not_editable_and_has_no_controls`.
  - `a_file_without_secrets_gives_its_own_text_and_hash`: a Claude file with a `// note` comment and `"model": "opus"` gives `text` byte-equal to the file, and `hash` equal to `hash_bytes`.
  - `a_file_with_a_secret_gives_no_text`: one with `env.X` or `apiKeyHelper` gives `text: None`, while `raw` is still the masked render.
  - `a_missing_file_is_editable_with_no_hash_or_text`.
  - `an_unparsable_file_is_not_editable`.
  - `a_managed_curated_key_is_locked_with_its_file`.
  - `a_table_value_locks_its_row`: Codex `approval_policy = { granular = { sandbox_approval = true, rules = true, mcp_elicitations = false } }`.
  - `a_value_outside_the_choices_is_shown_as_it_is`: Claude `theme = "custom:ocean"` gives value `custom:ocean`, no lock and the Choice control.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-core --lib settings_view` fails to compile on the missing fields.
- [ ] **Step 3: Implement:** fill the new fields in `view(...)`. "Masking changes nothing" means the masked value tree equals the parsed one. `read_managed` also returns the path of the file that won. Update `settings_view_of` and the existing route tests' expectations (`crates/core/tests/agents_api.rs`) for the new fields.
- [ ] **Step 4: Run the tests and see them pass:** `cargo test -j 4 -p marshell-core --lib settings_view` passes, then `cargo test -j 4 -p marshell-core --test agents_api` and `pnpm check:generated` (ok).
- [ ] **Step 5: Commit:** `feat(agents): tell the settings form each key's control and lock, and the file's own text when it holds no secret`.

### Task 3: The plan and undo routes

**Files:**
- Modify: `crates/protocol/src/settings_view.rs` (request types), `crates/core/src/config/service.rs` (a preview whose shown diff is supplied), `crates/core/src/server/agents.rs`, `crates/core/src/server/mod.rs` (routes), and `crates/core/src/agents/settings_view.rs` (the masked render, reused for the shown diff).
- Test: `crates/core/tests/agents_api.rs`.

**Interfaces:**
- Consumes: Task 1's ops, and Task 2's `control()`, lock and `hash`.
- Produces, in protocol:
  - `SettingEdit { key: String, value: Option<String> }`: `value` is JSON text, and `None` removes the key.
  - `#[serde(tag = "kind", rename_all = "snake_case")] enum SettingsChange { Form { edits: Vec<SettingEdit> }, Raw { text: String } }`.
  - `SettingsPlanRequest { expected_hash: Option<String>, change: SettingsChange }`.
  - `SettingsPlan { preview: ConfigPreview, normalized: bool }`.
  - `SettingsUndoRequest { snapshot_id: String }`.
- Produces, routes:
  - `POST /v1/agents/{agent}/settings/plan` answers 200 with a `SettingsPlan`, 409 with an `ApplyConflict` when the hash differs, 422 with a plain sentence, and 404 for an unknown agent.
  - `POST /v1/agents/{agent}/settings/undo` answers 200 with an `ApplyResult`, 409 with an `ApplyConflict`, 404 for an unknown snapshot, and 422 for a snapshot that isn't this agent's settings file.
- Produces, in `ConfigService`: `pub fn preview_shown(&self, cs: ChangeSet, shown: &dyn Fn(&PlannedFile) -> Option<String>) -> ConfigPreview`. When `shown` gives `Some`, that text replaces the file's diff in the returned and the stored preview, so `get_preview(id)` returns the same diff.

Behaviour:
- **The ChangeSet.**
  - It has one `FileEdit` on the path the view reads, with `scope: User`, `applies: NewSessions` and the request's `expected_hash`.
  - Its title is `Change Claude Code settings` or `Change Codex settings`, and its reason is `Edited in Settings.`.
  - A Claude form edit uses `JsonSet` or `JsonRemove` with the pointer built from the dotted key (`/permissions/defaultMode`). Codex uses `TomlSet` or `TomlRemove` with the key split on `.`.
  - Raw uses `ReplaceText`.
- **Validation before planning.** Each failure is a 422 with exactly this text:
  - Agent `antigravity`: `Antigravity's settings are read-only in Marshell.`
  - File unparsable: `The file doesn't parse (line <n>), so Marshell doesn't change it.`
  - A key with no control: `<key> isn't one of the settings this form changes.`
  - A locked key, managed: `<key> is set by <path>, so Marshell can't change it here.`
  - A locked key, not scalar: `<key> is set as a table in the file, so the form can't change it.`
  - Number: `<key> needs a whole number of at least <min>.`
  - Choice: `<key> needs one of: <v1>, <v2>, ….`
  - Switch: `<key> needs on or off.`
  - Text: `<key> needs text.` (an empty string is that, too)
  - Raw on a file whose view has `text: None`: `This file holds secret values, so Marshell doesn't edit it as text.`
  - Raw text that doesn't parse: Claude needs strict JSON with an object at the top (`serde_json`), and Codex needs TOML. The message is `The text doesn't parse at line <n>.`
  - None of these messages quote a value.
- **Conflict.** A preview with a file `Refused { reason: Conflict }` is answered 409 with `ApplyConflict { reason: "changed", paths: [path] }`. Any other refused file is a 422 with that file's `detail`. A preview with no change is a 422: `Nothing changes.`
- **The shown diff.** `normalized` is false, and the diff is the file's real diff, when neither the before nor the after text holds a value the view masks. Otherwise `normalized` is true, and the diff is the unified diff of the masked render (Task 2's `raw` render) of before and after, with the same header lines.
- **Undo.** The core finds the snapshot. Its one file must resolve to the agent's settings path. It plans `Restore` with `expected_hash` = that file's `after_hash`, or `None` with the file absent when the change created it, then applies. A different file on disk is a 409.

- [ ] **Step 1: Write the failing route tests** in `agents_api.rs`. Each uses a temp Claude settings file (`with_claude_settings`), a temp Codex dir (`with_codex_hooks`) and temp managed paths:
  - `settings_plan_is_refused_without_the_bearer`, and `settings_undo_is_refused_without_the_bearer`.
  - `a_claude_form_edit_plans_applies_and_writes_the_key`: plan `model` → `"sonnet"`, then `POST /v1/config/apply` with the preview's id and hashes. The file is now `{"model": "sonnet"}` plus what it had.
  - `a_codex_form_edit_keeps_comments`: the Review Focus 2 file. After plan and apply, the file equals the input with only the value changed.
  - `undo_puts_back_the_identical_bytes`: after that apply, undo with the snapshot id, and the file's bytes equal the original.
  - `undo_after_a_hand_edit_is_409`: nothing is written.
  - `undo_of_another_files_snapshot_is_422`.
  - `a_form_edit_on_a_missing_file_creates_it_and_undo_removes_it`, for both agents.
  - `a_plan_against_an_old_hash_is_409`.
  - `the_diff_never_shows_a_secret`: the Claude file holds `env.ANTHROPIC_API_KEY = "sk-live-9"` and `apiKeyHelper`, and the edit is to `model`. Neither the plan body nor `GET /v1/config/preview/{id}` contains `sk-live-9`, and `normalized` is true.
  - `a_file_without_secrets_shows_its_real_diff`: `normalized` is false, and a comment line appears as context.
  - `bad_form_edits_are_422_with_the_exact_message`: one case for each 422 line above. Each asserts the body text and that it holds no value.
  - `raw_on_a_file_with_secrets_is_422`, `raw_that_does_not_parse_names_the_line`, `claude_raw_with_a_comment_is_refused`, and `antigravity_settings_plan_is_422`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -j 4 -p marshell-core --test agents_api settings_` fails on a 404 or 405 for the new routes.
- [ ] **Step 3: Implement** the protocol types, `preview_shown`, both handlers (inside `spawn_blocking`) and the routes in the token-guarded router beside `settings-view`.
- [ ] **Step 4: Run the tests and see them pass:** `cargo test -j 4 -p marshell-core --test agents_api` passes, then `cargo test -j 4 -p marshell-core --lib config::` and `pnpm check:generated`.
- [ ] **Step 5: Commit:** `feat(agents): plan and undo a company settings change in the core, with a diff that never shows a secret`.

### Task 4: The form's model and the API client

**Files:**
- Modify: `src/lib/api/accounts.ts`, `src/lib/api/accounts.test.ts`, `src/features/companies/companyFields.ts`.
- Create: `src/features/companies/companyEdit.ts`, `src/features/companies/companyEdit.test.ts`.

**Interfaces:**
- Consumes: Task 3's types (`SettingsPlanRequest`, `SettingsPlan`, `SettingsUndoRequest`) and Task 2's `FieldControl`, `FieldLock` and `SettingValue`.
- Produces:
  - `planSettings(ep, agent: "claude" | "codex", req: SettingsPlanRequest): Promise<SettingsPlan>`, where a 409 throws `ApiError` with status 409 and a 422 throws with the core's sentence as `message`.
  - `undoSettings(ep, agent, snapshotId: string): Promise<ApplyResult>`.
  - In `companyEdit.ts`:
    - `type Draft = Record<string, string>`, keyed by curated key. The string is what the control shows: `""` is Not set; a Switch uses `"on"` or `"off"`.
    - `draftValue(v: SettingValue): string`
    - `parseField(control: FieldControl, input: string): { ok: true; json: string | null } | { ok: false; message: string }`
    - `formEdits(curated: SettingValue[], draft: Draft): { edits: SettingEdit[]; errors: Record<string, string> }`. A key appears only when its parsed value differs from the file's; locked keys never appear.
    - `choiceOptions(control: Choice, current: string): { value: string; label: string }[]`
    - `editSummary(n: number): string`, which reuses `changeSummary` from `../settings/edit`.
  - `companyFields.ts` gains `CHOICE_LABELS: Record<string, string>` for choice values whose raw spelling reads badly (`acceptEdits` reads `Accept edits`, `bypassPermissions` reads `Bypass permissions`, `dontAsk` reads `Don't ask`, `on-request` reads `On request`, `read-only` reads `Read-only`, `workspace-write` reads `Workspace write`, `danger-full-access` reads `Full access`, `dark-daltonized` reads `Dark, colour-blind friendly`, `light-daltonized` reads `Light, colour-blind friendly`, `dark-ansi` reads `Dark, terminal colours`, `light-ansi` reads `Light, terminal colours`, and `xhigh` reads `Extra high`). Any other value shows capitalised (`low` reads `Low`).

The choices are `Not set` (value `""`) first, then each control value. A current value that isn't among them comes next, with the label `<value> (from the file)`.

`parseField`'s messages for the field's error line:
- Number: `Use a whole number of at least <min>.`
- Text: none, since an empty input means Not set.
- Choice and Switch: none, since they can't hold a bad value.

- [ ] **Step 1: Write the failing tests:**
  - `parseField` per control kind:
    - `"30"` gives `30`;
    - `"0"` with min 1 gives the message;
    - `"1.5"` and `"x"` give the message;
    - `""` gives `null` for every kind;
    - a Switch `"on"` gives `true`;
    - Text `"opus"` gives `"\"opus\""`, as JSON text.
  - `formEdits` returns no edit for an untouched draft, an edit for a changed key, `value: null` for a cleared key, and nothing for a locked key. A key whose value came from the file outside the choices is untouched until it changes.
  - `choiceOptions` includes `custom:ocean (from the file)`, and `draftValue` round-trips each control.
  - The API client sends the right method, path and body. A 422's message is the core's text.
- [ ] **Step 2: Run them and see them fail:** `pnpm test src/features/companies src/lib/api/accounts.test.ts` fails on the missing module.
- [ ] **Step 3: Implement** the client and the model.
- [ ] **Step 4: Run the tests and see them pass:** the same command passes.
- [ ] **Step 5: Commit:** `feat(companies): model the company form's draft and edits, and call the plan and undo routes`.

### Task 5: The editable form and raw text, presentational

**Files:**
- Modify: `src/features/companies/CompanySection.tsx` (`CompanyView` and its props), `src/features/companies/CompanySection.css`, `src/features/companies/CompanySection.test.tsx`, and `src/styles/forcedColorsEdges.test.ts` (new control classes).

**Interfaces:**
- Consumes: Task 4's model.
- Produces: `CompanyViewProps` adds:
  - `draft: Draft`
  - `onDraft(key: string, input: string)`
  - `rawDraft: string | null`
  - `onRawDraft(text: string)`
  - `review: CompanyReview`, with `type CompanyReview = { kind: "editing" } | { kind: "planning" } | { kind: "review"; plan: SettingsPlan } | { kind: "applying" } | { kind: "saved"; snapshotId: string } | { kind: "conflict" } | { kind: "error"; message: string }`
  - `onReview()`, `onApply()`, `onBack()`, `onDiscard()`, `onReload()`, `onUndo()`

  `CompanySection` keeps rendering it read-only for Antigravity, so `view.editable === false` shows today's markup unchanged.

What it renders when `view.editable`:
- **Each curated row.**
  - Its control is labelled by the row's label (`<label>` or `aria-labelledby`), with the help as `aria-describedby`.
    - Choice: a native `<select>` with `choiceOptions`.
    - Switch: a `<select>` with `Not set`, `On` and `Off`.
    - Number: `<input type="number" min>`.
    - Text: `<input type="text">`.
  - Number and Text add the help sentence `Empty means not set.`.
  - A field error appears under the control, linked with `aria-describedby`.
  - A locked row shows its value as today, with one line: `Set by <path>, so Marshell can't change it here.` for Managed, or `Set as a table in the file. Edit it in Raw file.` for NotScalar. That second line becomes `Set as a table in the file.` when `view.text === null`.
- **Raw file.**
  - With `view.text !== null`, it is an editable textarea (class `raw-editor__text`, `aria-label` the path) holding `rawDraft ?? view.text`, with the note `This is the file itself. Review shows the exact change before anything is written.`
  - Otherwise it is today's read-only render and note, plus the line `Raw editing is off: this file holds secret values, and they show as hidden here.`
- **The change strip.** It shows only while there is a change and `review.kind` is `editing`.
  - It holds `editSummary(n)` (or `Raw file changed` for raw), then `Review changes` and `Discard`.
  - `Review changes` is disabled while a field has an error.
- **Review.** A `DiffView` of `previewDiff(plan.preview)`, labelled `Changes to <path>`.
  - When `normalized` is true, the note `Shown as Marshell reads it: comments and layout are left out, and secret values show as hidden. The file keeps its comments and layout.` comes first.
  - Then `A backup is saved first, in ~/.marshell/snapshots/. To undo, Marshell puts that backup back, for as long as the file is unchanged since this change.`, with the path in `<code>`.
  - Then `Apply` and `Back to editing`. `Back to editing` calls `onBack()`, which returns to `editing` and keeps the drafts.
- **Saved.** `Saved. New <Name> sessions use it.` with an `Undo` button.
- **Conflict.** `<path> changed since you opened it. Reload to see it.` with `Reload`.
- **Error.** The message word for word, with the strip still shown.
- **Busy states.** While `planning` or `applying`, the buttons are disabled with `aria-disabled`, and focus stays where it was.

- [ ] **Step 1: Read the good-css skill**, then write the failing `renderToStaticMarkup` tests in `CompanySection.test.tsx`:
  - the select for `permissions.defaultMode` lists `Not set` first and `Accept edits`;
  - `theme = custom:ocean` gives the `(from the file)` option, selected;
  - a managed row has no control and has the managed line;
  - a NotScalar row's two copies;
  - the raw textarea is editable with `view.text`, and locked with the off line without it;
  - each review kind's exact copy;
  - Antigravity's markup is unchanged from 6e2: there's no `<select>` and no strip;
  - an unparsable file shows no controls.
- [ ] **Step 2: Run them and see them fail:** `pnpm test src/features/companies` fails.
- [ ] **Step 3: Implement** with tokens only and no boxed outlines. Register the new control and button classes in `forcedColorsEdges.test.ts`, which also covers the forced-colours rule.
- [ ] **Step 4: Run the tests and see them pass:** `pnpm test src/features/companies src/styles` passes.
- [ ] **Step 5: Commit:** `feat(companies): show the company form's controls, raw text, review and undo`.

### Task 6: The editing flow in `CompanySection`

**Files:**
- Modify: `src/features/companies/CompanySection.tsx` (the `CompanySection` container), `src/features/settings/LiveSettings.tsx` (pass `announce`), and `src/features/companies/companyFlow.ts` with its test (new; the pure transitions).

**Interfaces:**
- Consumes: Tasks 4 and 5, and `applyPreview` and `previewDiff` from `src/lib/api/agents.ts`.
- Produces: `CompanySection` takes `announce: (text: string) => void`. In `companyFlow.ts`:
  - `afterPlan(result) -> CompanyReview`
  - `afterApply(outcome: ApplyOutcome) -> CompanyReview`
  - `afterUndo(...)`
  - `announcement(review: CompanyReview, name: string, path: string): string | null`

Behaviour:
- **Review changes** calls `planSettings` with `expected_hash: view.hash ?? null`. The change is `Form` with `formEdits(...).edits`, or `Raw` when the raw mode is shown and `rawDraft` differs from `view.text`. A 409 gives `conflict`, and a 422 gives `error` with the message.
- **Apply** calls `applyPreview(endpoint, plan.preview)`.
  - On `ok`, it re-reads the view, clears the drafts and goes to `saved`.
  - `conflict: true` goes to `conflict`, and `conflict: false` goes to `error`.
- **Undo** calls `undoSettings`.
  - On success, it re-reads the view and announces `Put <path> back as it was.`.
  - A 409 gives `error` with `<path> changed since this change, so undo would overwrite it.`
- **Reload** re-reads the view and keeps the drafts. A draft key whose new file value equals the draft drops out naturally through `formEdits`.
- **Discard** clears the drafts and goes back to `editing`.
- **Announcements**, through `announce` only:
  - on review: `Review the change to <path>.`
  - on saved: `Saved <path>.`
  - on conflict: the conflict sentence
  - on error: the message
- `saved` lasts until the next draft edit, a section change or Settings closing, because `CompanySection` unmounts or resets. Undo is offered only while it lasts.

- [ ] **Step 1: Write the failing tests:**
  - in `companyFlow.test.ts`, each transition and each announcement's exact text;
  - in `LiveSettings.test.tsx`, a test that `CompanySection` gets `announce`, following that file's existing prop tests.
- [ ] **Step 2: Run them and see them fail:** `pnpm test src/features/companies src/features/settings` fails.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the tests and see them pass:** the same command passes, then `pnpm build` is ok.
- [ ] **Step 5: Commit:** `feat(companies): plan, apply and undo a company settings change from Settings`.

### Task 7: Live check

**Files:**
- Create: `docs/research/2026-10-27-editable-company-tabs-live-check.md`.

Run the `marshell-live-check` skill with a new scratch folder and a short `-CodexHome`. Seed the scratch Claude settings with `model` and an `env.ANTHROPIC_API_KEY` fake, and the scratch `config.toml` with comments and a `[profiles.x]` table. Exit items:

1. A Claude form edit of `permissions.defaultMode`:
   - Review shows the normalized note and no secret.
   - Apply writes the scratch file.
   - Undo puts it back, with an `fc` or hash compare against a copy taken before.
2. A Codex form edit of `sandbox_mode`: Review shows the real diff with a comment line as context. After Apply, the comments and the table are still in the file.
3. Codex raw edit: change a value in the text. Review shows the diff, and Apply writes it.
4. Raw is locked for the Claude file with the secret, showing the off line.
5. A hand edit of the scratch `config.toml` after opening and before Apply: Apply gives the conflict line, and Reload keeps the draft.
6. Keyboard only through the Claude form, review, apply and undo, then emulated forced colours on the form and the review.
7. The real `~/.claude/settings.json` and `~/.codex/config.toml` modified times are unchanged.

The commit is `docs(research): record the editable company tabs live check`. Fix any finding with a failing-first test before the note is committed.
