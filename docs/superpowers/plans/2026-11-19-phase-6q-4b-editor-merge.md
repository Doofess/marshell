# Slice 6q-4b: the three-way merge on the "changed on disk" bar — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The bar a text tab shows when its file changed on disk under unsaved edits gains Merge: the disk's changes are folded into the person's text, lines changed on both sides get conflict markers, the editor tints conflicts and jumps between them, and Ctrl+Z brings the bar back.

**Architecture:** The core merges three texts (the file as loaded, the person's text, the disk's) with `diffy` behind one new route that takes no path and writes nothing. The reducer `editorDoc.ts` asks for the merge as an effect, takes the answer only if nothing moved meanwhile, and remembers the text before and after so that an undo or redo of the merge switches the base and the bar by comparing texts. Conflicts are found by one pure scan of the text (`conflicts.ts`), which the tints, the jump commands and the save line all use.

**Tech Stack:** Rust (axum, tokio, `diffy` 0.5.2, already a dependency), React 19, TypeScript, CodeMirror 6 (the packages 6q-4a added), Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 6q-4b decisions (2026-11-19)" 1–5, on top of "Slice 6q-4 decisions (2026-11-18)" 3, 4 and 9–11. The slice before it: `docs/superpowers/plans/2026-11-18-phase-6q-4a-text-editor.md`. Rules: `.claude/rules/local-api.md`, `protocol.md`, `ui.md`.

## Global Constraints

- **Security surface.** Nothing changes in `src-tauri/`, `crates/core/src/server/guard.rs`, token handling, the CSP or the CORS layer. No new crate and no new package. The one new route takes no file path and writes nothing, so `.claude/rules/local-api.md` stays as it is. Nothing in the slice writes a user's file: a merged text reaches the disk only through 6q-4a's save. If a task seems to need more, stop and report.
- **The route** sits in the token-guarded router with a "refused without the bearer" test. Its types go in `crates/protocol/src/api.rs` with `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]` and `#[ts(export)]`. The merge runs in `spawn_blocking`; no lock is held across an await.
- **Numbers.** `MAX_CHANGED_LINES = 20_000` (per side, against the base). `MERGE_TIMEOUT = 10 s`. One merge at a time per core. Each text in, and the text out, is at most `DRAFT_TEXT_CAP` (3 MiB of UTF-8, `server/drafts.rs`). The route's body limit is 24 MiB. Tints stop above `COLOUR_CAP` (1 MiB of text, `tokens.ts`).
- **Markers.** A merge writes exactly `<<<<<<< mine`, `=======`, `>>>>>>> disk`, each on its own line. A conflict in a text is: a line matching `^<{7}( .*)?$`, then optionally one matching `^\|{7}( .*)?$`, then one that is exactly `=======`, then one matching `^>{7}( .*)?$`, in that order with any lines between. A new start line before the end line starts the conflict over.
- **Every text the reducer, the route and the scan see has `\n` line endings** (`lf()` in `useEditorFile.ts`). A save writes the file's own endings, as before.
- **Copy** (sentence case, no "!", no "successfully"; exact strings; `{name}` is the file's name, `{n}` a count of 2 or more):
  - Bar: "{name} changed on disk." with the buttons "Merge", "Keep mine", "Take disk", in that order.
  - While a merge runs, under the text and not announced: "Merging {name}…"
  - After a merge, under the text and announced: "Merged {name}. No conflicts. Ctrl+Z undoes the merge." · "Merged {name}. 1 conflict to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge." · "Merged {name}. {n} conflicts to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge." · when the merged text is the person's own text: "Merged {name}. No conflicts."
  - A refused merge, under the text and announced: "{name} couldn't be merged. {the core's line}"
  - The core's lines: "The two versions differ in too many lines." · "The merged text would be over 3 MB." · "A text is over 3 MB." · "The merge took too long." · "Another merge is still running."
  - After a save of a text with conflicts: "Saved {name}. 1 conflict left." · "Saved {name}. {n} conflicts left." With none, "Saved {name}." as before.
  - Jumps, announced only: "Conflict {at} of {of}." · "{name} has no conflicts."
  - Commands: `pane-next-conflict` "Next conflict" · `pane-previous-conflict` "Previous conflict", group "Window", no app shortcut, listed in the palette only while a text tab is on screen.
- **Keys in the editor.** F7 is Next conflict and Shift+F7 is Previous conflict, in CodeMirror's keymap, not in `lib/keymap.ts` bindings. Every app shortcut still wins over an editor key.
- **Styles.** Read the good-css skill before writing declarations; the frontend-design skill decides the tints. Tokens only, no hex outside `tokens.css`. `:hover` only inside `@media (hover: hover) and (pointer: fine)`; forced-colors rules use system colours only; no boxed outlines; no scrollbar shows.
- **Keyboard and forced colours.** Everything new works with the keyboard alone and in forced colours. `Announcer` is the only live region.
- **Who writes what** (6q-4 decision 4). Tasks 1, 2 and 7 are written in the main thread. Tasks 3 to 6 go to Sonnet workers, one at a time (`effort: high` where the heading says so); the main thread reads each diff and runs its tests. A worker edits only its task's files and never runs `git checkout`, `git restore` or `git stash`.
- **Checks.** Rust: `cargo test -p <crate> -j 4`; `cargo fmt --all` before each Rust commit; `cargo clippy --workspace --all-targets -j 4 -- -D warnings` at the end of each Rust task. After the protocol change: `cargo test -p marshell-protocol -j 4`, then `pnpm check:generated`. Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** buttons on a conflict (Take mine, Take disk, Take both), a side-by-side compare, `@codemirror/merge`, a merge that runs on its own, a three-part conflict written by Marshell, a merge of a file that is gone, rebinding F7.

## Review Focus

1. **A text that already holds marker lines:** a file git left conflicted (`<<<<<<< HEAD` … `>>>>>>> feature/x`, with or without `||||||| base`), a Markdown heading underlined with `=======`, a line that is exactly `<<<<<<< ours` or `<<<<<<< mine`, a start line with no end. Expected: a merge leaves such lines as they are and labels only its own conflicts; the scan counts a conflict only when start, middle and end come in order. Tests: Task 1 `a_text_that_holds_marker_lines_keeps_them`, Task 3 `conflictsIn_*`.
2. **The answer comes after something moved:** the person typed during the merge, the file changed again, Keep mine or Take disk was pressed, the file was deleted, Merge was pressed twice. Expected: a stale answer never replaces the text; the merge runs again while the bar is still up; a second press does nothing. Tests: Task 4 `merged_*`, `merge_*`.
3. **Undo and redo around a merge:** Ctrl+Z right after; edits after the merge undone past it; the file changed again before the undo; a save and then an undo; Ctrl+Y; the same text typed by hand; a merge that changed nothing. Expected: with the text from before the merge the bar is up and a save is refused; with the merged text the bar is down and a save is checked against the merged file; after a save nothing comes back. Tests: Task 4 `undo_*`, `redo_*`.
4. **A merge that is too much work:** a 20,001-line rewrite on one side, the same lines in another order (which the count can't see), a result over 3 MiB, a text over 3 MiB, two merges at once. Expected: each is refused with its line within 10 seconds and the bar stays; the core keeps serving other requests. Tests: Task 1 `a_side_past_the_limit_is_too_different`, Task 2 `merge_refuses_*`, `a_merge_past_the_time_limit_*`.
5. **Line endings and edges of a file:** no final newline in any of the three texts, an empty base (the file was new), an empty disk file, a CRLF file, one very long line. Expected: every marker is on a line of its own; the three texts go to the core with `\n`; the save after a merge writes the file's own endings. Tests: Task 1 `a_last_line_without_a_newline_*`, `an_empty_base_*`; Task 4 `the_merge_sends_lf_texts`.

---

### Task 1: The merge and its types (main thread)

**Files:**
- Modify: `crates/protocol/src/api.rs`, `crates/core/src/userfiles/mod.rs`, `crates/core/src/config/mod.rs` (drop `pub mod merge;`)
- Move: `crates/core/src/config/merge.rs` → `crates/core/src/userfiles/merge.rs` (`git mv`), then rewrite
- Regenerated: `src/generated/FileMerge.ts`, `src/generated/FileMerged.ts`

**Interfaces:**
- Produces, in `api.rs`, after `FileUnsave`:
  - `pub struct FileMerge { pub base: String, pub mine: String, pub disk: String }` — the body of `POST /v1/files/merge`: the text the edits started from, the person's text, and the text on disk, each with `\n` line endings.
  - `pub struct FileMerged { pub text: String, pub conflicts: u32 }` — `conflicts` is how many conflicts this merge wrote.
- Produces, in `userfiles/merge.rs`:
  - `pub const MAX_CHANGED_LINES: usize = 20_000;`
  - `pub struct Merged { pub text: String, pub conflicts: u32 }`, `pub struct TooDifferent;` (both `Debug, PartialEq, Eq`)
  - `pub fn changed_lines(base: &str, other: &str) -> usize` — lines of `base` with no equal line left in `other`, plus lines of `other` with none left in `base`, counted as multisets. A lower bound on the diff's size: moved lines count 0.
  - `pub fn merge(base: &str, mine: &str, disk: &str) -> Result<Merged, TooDifferent>` and, for tests, `fn merge_with(base, mine, disk, limit: usize)`.
- How `merge_with` works: `TooDifferent` when `changed_lines(base, mine)` or `changed_lines(base, disk)` is over `limit`. Otherwise `diffy::MergeOptions` with `ConflictStyle::Merge`, `IncompleteHunkStyle::Git` and a marker length `L`: the smallest `L >= 7` for which no line of the three texts is exactly `"<" * L + " ours"`, `"=" * L` or `">" * L + " theirs"`. Each output line that is exactly one of those three is diffy's own and becomes `<<<<<<< mine`, `=======` or `>>>>>>> disk`; the start lines are counted.

- [ ] **Step 1: Write the failing tests** in `userfiles/merge.rs`:
  - `edits_apart_merge_clean`: `merge("a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n")` → `Ok(Merged { text: "A\nb\nC\n", conflicts: 0 })`.
  - `edits_to_one_line_conflict_as_mine_and_disk`: `merge("a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n")` → text `"a\n<<<<<<< mine\nX\n=======\nY\n>>>>>>> disk\nc\n"`, conflicts 1.
  - `the_same_edit_on_both_sides_is_no_conflict`: mine and disk both `"a\nZ\nc\n"` → that text, 0.
  - `two_conflicts_are_counted`: lines 1 and 5 of a 5-line base changed differently on each side → conflicts 2, and `text.matches("<<<<<<< mine\n").count() == 2`.
  - `a_last_line_without_a_newline_keeps_markers_on_their_own_lines`: `merge("a", "b", "c")` → `text.lines().collect::<Vec<_>>() == ["<<<<<<< mine", "b", "=======", "c", ">>>>>>> disk"]`.
  - `an_empty_base_conflicts_two_new_texts`: `merge("", "a\n", "b\n")` → conflicts 1; `merge("x\n", "x\ny\n", "")` (the disk file was emptied) → an `Ok` whose text holds `y`.
  - `a_text_that_holds_marker_lines_keeps_them`: base, mine and disk all hold the lines `<<<<<<< ours`, `=======`, `>>>>>>> theirs` and `<<<<<<< mine`; one other line differs on each side → those four lines appear unchanged, once each besides the merge's own markers, and conflicts is 1.
  - `changed_lines_counts_both_ways`: `("a\nb\nc\n", "a\nx\ny\nc\n")` → 3; equal texts → 0; `("a\nb\n", "b\na\n")` → 0; `("a\na\n", "a\n")` → 1.
  - `a_side_past_the_limit_is_too_different`: `merge_with` with limit 2: mine changes 3 lines → `Err(TooDifferent)`; the disk changes 3 → `Err(TooDifferent)`; 2 on each side → `Ok`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 userfiles::merge` → compile error, then FAIL.
- [ ] **Step 3: Implement** the two types in `api.rs` and the module.
- [ ] **Step 4: Run** `cargo test -p marshell-core -j 4 userfiles::merge`, `cargo test -p marshell-protocol -j 4`, `pnpm check:generated` → all pass; clippy clean.
- [ ] **Step 5: Commit** `feat(core): the three-way merge of a user's file labels its conflicts mine and disk, and refuses sides that differ by more than 20,000 lines.`

### Task 2: The merge route (main thread)

**Files:**
- Create: `crates/core/src/server/merge.rs`
- Modify: `crates/core/src/server/mod.rs` (the module, the route, `AppState`), `crates/core/src/server/drafts.rs` (`DRAFT_TEXT_CAP` becomes `pub(super)`)
- Test: `crates/core/tests/merge_api.rs`

**Interfaces:**
- Consumes: Task 1's `FileMerge`, `FileMerged`, `userfiles::merge::{merge, Merged, TooDifferent}`; `drafts::DRAFT_TEXT_CAP`.
- Produces: `POST /v1/files/merge`, body `FileMerge` as JSON, body limit `24 * 1024 * 1024`. Answers: 200 `FileMerged`; 413 "A text is over 3 MB." when any of the three is over `DRAFT_TEXT_CAP`; 422 "The two versions differ in too many lines."; 422 "The merged text would be over 3 MB."; 503 "The merge took too long." after `MERGE_TIMEOUT`; 503 "Another merge is still running." while the slot is taken; 500 when the task panicked. The lines are plain-text bodies, as `save.rs` sends its refusals.
- `AppState` gains `merge_slot: Arc<tokio::sync::Semaphore>` with one permit, so each test server has its own. The handler takes the permit with `try_acquire_owned` and moves it into the `spawn_blocking` closure: a merge that outlives its time limit keeps the slot until it really ends.
- For tests, in `merge.rs`: `async fn run<F>(slot: Arc<Semaphore>, limit: Duration, work: F) -> Done where F: FnOnce() -> Result<Merged, TooDifferent> + Send + 'static`, with `enum Done { Merged(Merged), TooDifferent, TooLong, Slow, Busy, Failed }`.

- [ ] **Step 1: Write the failing tests.** In `merge.rs`: `a_merge_past_the_time_limit_answers_slow_and_keeps_the_slot_until_it_ends` (work that sleeps 300 ms under a 50 ms limit → `Slow`; a second `run` at once → `Busy`; after 400 ms a third → `Merged`), `a_merge_that_panics_frees_the_slot` (→ `Failed`, then a second → `Merged`). In `merge_api.rs` (helpers as `save_api.rs`):
  - `merge_is_refused_without_the_bearer`.
  - `merge_answers_the_text_and_its_conflicts`: the Task 1 one-line conflict → 200, `conflicts: 1`, the exact text.
  - `merge_of_edits_apart_has_no_conflicts` → 200, `conflicts: 0`.
  - `merge_refuses_a_text_over_the_cap`: `mine` of `DRAFT_TEXT_CAP + 1` bytes → 413 with its line.
  - `merge_refuses_sides_that_differ_too_much`: a 10,001-line base and a `disk` with every line different → 422 "The two versions differ in too many lines."
  - `merge_refuses_a_result_over_the_cap`: base one line; `mine` and `disk` each one different line of 1.6 MiB → 422 "The merged text would be over 3 MB."
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 --test merge_api` and `cargo test -p marshell-core -j 4 server::merge` → FAIL.
- [ ] **Step 3: Implement** the handler and register the route beside `/v1/files/unsave`.
- [ ] **Step 4: Run** both commands and `cargo test -p marshell-core -j 4 --test save_api --test drafts_api` → PASS; clippy clean.
- [ ] **Step 5: Commit** `feat(core): a route merges three texts within ten seconds, one merge at a time.`

### Task 3: The conflict scan and the merge request (worker)

**Files:**
- Create: `src/features/editor/conflicts.ts`, `src/features/editor/conflicts.test.ts`
- Modify: `src/features/editor/editorApi.ts`, `src/features/editor/editorApi.test.ts`

**Interfaces:**
- Consumes: `src/generated/FileMerge.ts`, `FileMerged.ts` (Task 1).
- Produces, in `conflicts.ts` (pure, no imports from CodeMirror):
  - `export type Conflict = { start: number; base: number | null; middle: number; end: number }` — the 1-based line numbers of a conflict's marker lines; `base` is the `|||||||` line when there is one.
  - `export function conflictsIn(lines: Iterable<string>): Conflict[]` — by the rule in Global Constraints, "Markers".
  - `export function countConflicts(text: string): number` — `conflictsIn` over the text's `\n` lines.
  - `export function conflictFrom(conflicts: readonly Conflict[], line: number, which: "first" | "next" | "previous"): number | null` — an index into `conflicts`, null when there are none. `next`: the first whose `start` is after `line`, else index 0. `previous`: the last whose `start` is before `line`, else the last index. `first`: 0.
- Produces, in `editorApi.ts`:
  - `export type MergeResult = { kind: "merged"; text: string; conflicts: number } | { kind: "refused"; line: string }`
  - `export async function mergeTexts(base: string, mine: string, disk: string): Promise<MergeResult>` — `coreFetch(ep, "POST", "/v1/files/merge", body)`; 200 → `merged`; 413 → `refused` with "A text is over 3 MB."; anything else, and a failed request, → `refused` with `lineOf(res)` or the generic line, as `unsaveFile` does.

- [ ] **Step 1: Write the failing tests.**
  - `conflicts.test.ts`: `conflictsIn finds a two-part conflict` (lines `a`, `<<<<<<< mine`, `x`, `=======`, `y`, `>>>>>>> disk`, `b` → `[{ start: 2, base: null, middle: 4, end: 6 }]`); `…a three-part one` (`||||||| base` on line 4 → `base: 4`); `…git's labels` (`<<<<<<< HEAD`, `>>>>>>> feature/x`); `…two in one text`; `ignores a lone ======= line` (a Markdown underline → `[]`); `ignores a start with no end`; `starts over at a second start line` (`<<<<<<< a`, `x`, `<<<<<<< b`, `=======`, `>>>>>>> c` → one conflict with `start: 3`); `ignores an end before a middle`; `needs exactly seven` (`<<<<<<`, `<<<<<<<<`, `<<<<<<<mine`, `======= x` → `[]`); `countConflicts` of `""` → 0; `conflictFrom` next from before, between and after two conflicts (after → 0), previous likewise (before → last), `first`, and null for none.
  - `editorApi.test.ts`, in the style of the `unsaveFile` tests: `mergeTexts posts the three texts` (the request's method, path and JSON body); a 200 → `{ kind: "merged", text, conflicts }`; a 422 with a body → `refused` with that line; a 413 → "A text is over 3 MB."; a thrown fetch → the generic line.
- [ ] **Step 2: Run** `pnpm test src/features/editor/conflicts.test.ts src/features/editor/editorApi.test.ts` → FAIL.
- [ ] **Step 3: Implement** both.
- [ ] **Step 4: Run** the same command and `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(editor): a scan that finds the conflicts in a text, and the request that merges three texts.`

### Task 4: The merge in the document (worker, effort high)

**Files:**
- Modify: `src/features/editor/editorDoc.ts`, `src/features/editor/editorDoc.test.ts`, `src/features/editor/useEditorFile.ts`, `src/features/editor/useEditorFile.test.ts`, `src/features/editor/Editor.tsx` (only the `EditorHandle` type gains `toConflict`, with a body that does nothing yet)

**Interfaces:**
- Consumes: `countConflicts` and `mergeTexts` (Task 3).
- Produces, in `editorDoc.ts`:
  - `DocState` gains `merging: boolean` (false at `start`) and `merge: { mine: string; merged: string; before: Disk; after: Disk } | null` (null at `start`): `mine` is the text before the merge, which rests on the file `before`; `merged` rests on the file `after`.
  - `DocEvent` gains `{ type: "merge" }`, `{ type: "merged"; text: string; conflicts: number; mine: string; disk: string | null }` (`mine` and `disk` are the text and the disk hash the request was made from) and `{ type: "mergeFailed"; line: string }`.
  - `Effect` gains `{ type: "merge"; base: string; mine: string; disk: Disk }` and `{ type: "showConflict" }`.
- Produces, in `Editor.tsx`: `EditorHandle.toConflict(which: "first" | "next" | "previous"): void`.
- Produces, in `useEditorFile.ts`: `EffectDeps.api` gains `mergeTexts`; `runEffect` runs `merge` (calls `mergeTexts(effect.base, effect.mine, effect.disk.text)`, then dispatches `merged` with `mine: effect.mine, disk: effect.disk.hash`, or `mergeFailed`) and `showConflict` (`d.editor()?.toConflict("first")`).
- The reducer's rules:
  - `merge`: nothing when `conflict` is null, or `saving`, or `merging`. Else `merging: true`, `line: "Merging {name}…"`, and the `merge` effect with `base: s.base.text`, `mine: s.text`, `disk: s.conflict`.
  - `merged`, three cases. When `conflict` is null (the bar was answered meanwhile): `merging` goes false and nothing else happens. When `e.mine !== s.text` or `e.disk !== s.conflict.hash`: the answer is stale, so `merging` stays true and a new `merge` effect goes out for the state as it is. Otherwise `merging` goes false and: `text` is `e.text`, `base` is the old `conflict`, `conflict` null, `deleted` false, `dirty` is `e.text !== base.text`, `marks` is `changedLines(lines(e.mine), lines(e.text))`, `merge` is `{ mine: e.mine, merged: e.text, before: <old base>, after: <old conflict> }` or null when `e.text === e.mine`, `line` is the "Merged …" copy for `e.conflicts` (the short one when `e.text === e.mine`). Effects, in order: `setText` with the marks, `showConflict` when `e.conflicts > 0`, `putDraft` or `dropDraft` by `dirty`, `announce` with the line.
  - `mergeFailed`: `merging` false; `said` "{name} couldn't be merged. {line}". The bar stays.
  - `edited`, before the existing rule, while `s.merge` is not null: when `text === s.merge.mine` and `s.base.hash === s.merge.after.hash`, the state becomes `base: s.merge.before`, `conflict: s.conflict ?? s.merge.after`, `dirty: true`; when `text === s.merge.merged` and `s.base.hash === s.merge.before.hash` and `s.conflict !== null`, it becomes `base: s.merge.after`, `conflict: null` when `s.conflict.hash === s.merge.after.hash` and unchanged otherwise, `dirty` as `edit()` computes it. In both, `merge` is kept, the rest of `edited` applies (marks, `undoOffered` and `line` cleared), and the draft effect follows the new `dirty`. The draft a `putDraft` writes takes its base from the state, so it follows.
  - `merge` goes back to null on `opened`, `saved`, `keepMine`, `takeDisk`, `gone`, in `reload`, and in `disk` when the disk holds the editor's text. `merging` goes false on `gone`.
  - `saved`: the line is "Saved {name}." plus " 1 conflict left." or " {n} conflicts left." by `countConflicts(s.sent)`.

- [ ] **Step 1: Write the failing tests** in `editorDoc.test.ts`, with a helper that builds the state "base B, text M, the bar up with disk D" from `opened` with a draft:
  - `merge_asks_for_the_three_texts`: the effect is `{ type: "merge", base: B.text, mine: M, disk: D }`, `merging` true, the line "Merging a.txt…" and no announce.
  - `merge_does_nothing_without_the_bar_while_saving_or_twice`.
  - `merged_puts_the_text_in_and_takes_the_disk_as_the_base`: `text`, `base === D`, `conflict` null, `dirty` true, marks from mine to merged; effects are exactly `setText`, `showConflict`, `putDraft`, `announce` in that order for `conflicts: 2`, with the line "Merged a.txt. 2 conflicts to resolve. F7 goes to the next conflict, and Ctrl+Z undoes the merge."
  - `merged_with_one_conflict_and_with_none_say_so`: the two other lines; none has no `showConflict`.
  - `merged_to_the_disks_own_text_is_clean`: `e.text === D.text` → `dirty` false and `dropDraft`.
  - `merged_to_the_persons_own_text_remembers_nothing`: `e.text === e.mine` → `merge` null, the line "Merged a.txt. No conflicts.".
  - `merged_for_an_older_text_runs_again`: an `edited` between `merge` and `merged` → the text is not replaced, one new `merge` effect carries the newer text, `merging` stays true.
  - `merged_for_an_older_disk_runs_again`: a `disk` event with another hash in between → the same, with the newer disk.
  - `merged_after_keep_mine_or_take_disk_is_dropped`: no `setText`, `merging` false.
  - `merge_failed_keeps_the_bar_and_says_why`: "a.txt couldn't be merged. The merge took too long."
  - `undo_of_a_merge_brings_the_bar_back`: after `merged`, `edited` with `M` → `base === B`, `conflict === D`, `dirty` true, a `putDraft`; then `save` → refused with "a.txt changed on disk, so it wasn't saved."
  - `redo_of_a_merge_takes_the_bar_away`: then `edited` with the merged text → `base === D`, `conflict` null; then `save` → a `save` effect with `expect: D.hash`.
  - `undo_past_a_merge_after_more_edits`: `merged`, `edited` with other text (the bar stays down, `base === D`), `edited` with `M` → the bar is back.
  - `undo_of_a_merge_after_the_file_changed_again`: `merged`, `disk` with D2 (the bar shows D2), `edited` with `M` → `base === B`, `conflict === D2`; then `edited` with the merged text → `base === D`, `conflict === D2`.
  - `undo_after_a_save_brings_nothing_back`: `merged`, `save`, `saved`, `edited` with `M` → `conflict` null, `base` is the saved text.
  - `keep_mine_and_take_disk_forget_the_merge`: undo, then `keepMine`, then `edited` with the merged text → the bar stays down and `base === D`.
  - `saved_counts_the_conflicts_left`: a save of a text with one conflict → "Saved a.txt. 1 conflict left."; with two → "Saved a.txt. 2 conflicts left."; with none → "Saved a.txt."
  - `a_file_deleted_during_a_merge_drops_the_answer`.
  In `useEditorFile.test.ts`: `the_merge_effect_calls_the_api_and_dispatches_its_answer` (merged and refused), `the_merge_sends_lf_texts` (a file opened with `\r\n` text and a draft → the three texts the fake `mergeTexts` gets hold no `\r`), `showConflict_goes_to_the_first_conflict` (the fake editor records `toConflict("first")`). Add `toConflict() {}` to the fake editors the file already builds.
- [ ] **Step 2: Run** `pnpm test src/features/editor/editorDoc.test.ts src/features/editor/useEditorFile.test.ts` → FAIL.
- [ ] **Step 3: Implement.** Keep `step` pure; the only new async work is the `merge` effect's request.
- [ ] **Step 4: Run** the same command, `pnpm test src/features/editor`, and `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(editor): the document merges the disk's text into the person's, and an undo of the merge brings the bar back.`

### Task 5: Conflicts in the editor: tints, F7 and Shift+F7 (worker)

**Files:**
- Create: `src/features/editor/conflictTints.ts`, `src/features/editor/conflictTints.test.ts`
- Modify: `src/features/editor/Editor.tsx`, `src/features/editor/Editor.test.tsx`, `src/features/editor/Editor.css`, `src/styles/tokens.css`, `src/design/contrast.test.ts`

**Interfaces:**
- Consumes: `conflictsIn`, `conflictFrom`, `Conflict` (Task 3); `COLOUR_CAP` (`tokens.ts`); the `toConflict` type (Task 4).
- Produces, in `conflictTints.ts`:
  - `export function conflictTints(): Extension` — a `StateField` of line decorations, built from the document when it is made and when it changes, and empty while the document is longer than `COLOUR_CAP`. Classes: every line of a conflict gets `editor__conflict`; a marker line also `editor__conflict--mark`; the lines between start and middle (or start and base) `editor__conflict--mine`; between base and middle `editor__conflict--base`; between middle and end `editor__conflict--disk`.
  - `export function tintsOf(state: EditorState): Array<{ line: number; classes: string }>` — what the field holds, for tests.
  - `export function jumpToConflict(view: EditorView, which: "first" | "next" | "previous"): { at: number; of: number }` — scans the whole document (also above `COLOUR_CAP`), puts the cursor at the start of the chosen conflict's start line, scrolls it to about a third from the top as `reveal` does, and focuses the view. `{ at: 0, of: 0 }` and no change when there is none; `at` is 1-based.
  - `export function conflictKeys(report: (at: number, of: number) => void): KeyBinding[]` — `F7` → next, `Shift-F7` → previous, both `preventDefault: true`, each calling `report` with what `jumpToConflict` returned.
- Produces, in `Editor.tsx`: the prop `onConflict?: (at: number, of: number) => void`; the extensions `conflictTints()` and `keymap.of(conflictKeys(...))` beside `lineMarks()`; `toConflict(which)` on the handle: `"first"` jumps and reports nothing, `"next"` and `"previous"` jump and call `onConflict`.
- Produces, in `tokens.css`: `--code-conflict-mine`, `--code-conflict-disk` and `--code-conflict-mark`, each with `light-dark()`. In `Editor.css`: the three tints on the classes above (`--base` uses the mark tint), and in the forced-colors block no background and a `border-inline-start: 3px solid CanvasText` on `.editor__conflict`.
- The tints must leave a selection visible on a tinted line, as the active line's background does. Every `--code-token-*` colour and `--code-foreground` keeps at least 4.5:1 on each tint as it is drawn over `--code-background`, light and dark; `contrast.test.ts` holds that (blend a translucent tint over the background before measuring).

- [ ] **Step 1: Write the failing tests.**
  - `conflictTints.test.ts` (an `EditorState` needs no DOM): `tints the three kinds of line` (the 7-line text of Task 3's first test → line 2 and 4 and 6 `--mark`, line 3 `--mine`, line 5 `--disk`, lines 1 and 7 nothing); `tints a base section`; `follows an edit` (a transaction that deletes the middle line → no tints); `has none above the colour cap` (a document of `COLOUR_CAP + 1` characters holding a conflict → `[]`); `conflictKeys binds F7 and Shift-F7`.
  - `contrast.test.ts`: the three tints exist, and each code colour is at least 4.5:1 on each, in both themes.
  - `Editor.test.tsx`: the markup is unchanged by the new prop (the editor still renders its host).
- [ ] **Step 2: Run** `pnpm test src/features/editor/conflictTints.test.ts src/design/contrast.test.ts` → FAIL.
- [ ] **Step 3: Implement.** Read the frontend-design skill for the tints and the good-css skill before the declarations.
- [ ] **Step 4: Run** `pnpm test src/features/editor src/design` and `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(editor): a conflict is tinted, and F7 and Shift+F7 go to the next and the previous one.`

### Task 6: The Merge button and the two commands (worker)

**Files:**
- Modify: `src/features/editor/ChangedBar.tsx`, `src/features/editor/TextTab.tsx`, `src/features/editor/TextTab.test.tsx`, `src/features/editor/TextTab.stories.tsx`, `src/lib/keymap.ts`, `src/lib/keymap.test.ts`, `src/lib/appShortcuts.ts`, `src/lib/appShortcuts.test.ts`, `src/features/palette/runnable.ts`, `src/features/palette/runnable.test.ts`, `src/features/sessions/LiveApp.tsx`

**Interfaces:**
- Consumes: the `merge` event (Task 4); `EditorHandle.toConflict` and `onConflict` (Tasks 4 and 5).
- Produces:
  - `ChangedBar` gains `onMerge: () => void` and the button "Merge" before "Keep mine", with the bar's existing button class.
  - `TextTabViewProps` gains `onMerge: () => void` and `onConflict: (at: number, of: number) => void`; `TextTab` passes `onMerge={() => pressed({ type: "merge" })}` and an `onConflict` that announces "Conflict {at} of {of}.", or "{name} has no conflicts." when `of` is 0.
  - `TextTabCommands` gains `nextConflict(): void` and `previousConflict(): void` (`editor.current?.toConflict("next" | "previous")`).
  - `keymap.ts`: `{ id: "pane-next-conflict", label: "Next conflict", group: "Window" }` and `{ id: "pane-previous-conflict", label: "Previous conflict", group: "Window" }` after `pane-word-wrap`, no binding. `appShortcuts.ts`: the kinds `paneNextConflict` and `panePreviousConflict`. `runnable.ts`: both ids in the `textTab` list. `LiveApp.tsx`: the two cases beside `paneGoToLine`, calling `shownTextRef.current()?.nextConflict()` and `previousConflict()`.

- [ ] **Step 1: Write the failing tests.** `TextTab.test.tsx`: `the bar offers Merge, Keep mine and Take disk in that order` (the three button texts, in order, in the markup of a state with a conflict); `the line under the text shows a merge's result` (a state after `merged` with 2 conflicts renders the "Merged …" line and no Undo button). `keymap.test.ts`: the two commands by label and group, with no binding (extend the test at the `pane-go-to-line` assertions). `appShortcuts.test.ts`: `commandShortcut("pane-next-conflict")` → `{ kind: "paneNextConflict" }`, and the previous one. `runnable.test.ts`: the `textTab` list holds six ids.
- [ ] **Step 2: Run** `pnpm test src/features/editor/TextTab.test.tsx src/lib/keymap.test.ts src/lib/appShortcuts.test.ts src/features/palette/runnable.test.ts` → FAIL.
- [ ] **Step 3: Implement.** Add two stories to `TextTab.stories.tsx`: `ChangedOnDisk`'s text now names the three buttons, and `Merged` shows a text with one conflict after a merge.
- [ ] **Step 4: Run** `pnpm test` and `pnpm build` → PASS.
- [ ] **Step 5: Commit** `feat(editor): the bar offers Merge, and the palette has Next conflict and Previous conflict.`

### Task 7: The live check and the docs (main thread)

**Files:**
- Create: `docs/research/2026-11-19-editor-merge-live-check.md`
- Modify: `docs/PLAN.md` (§9, "Slice 6q-4b decisions": what the live check changed, if anything)

- [ ] **Step 1: Run the live check** with the `marshell-live-check` skill, in a scratch project folder. Each line is a check with its result in the note:
  1. A file edited in the tab and changed on disk in other lines: the bar shows three buttons; Merge puts both changes in the text, the line reads "Merged … No conflicts. Ctrl+Z undoes the merge.", the gutter marks the disk's lines, and Ctrl+S saves the merged text (read the file back).
  2. The same line changed on both sides: the markers read `<<<<<<< mine` and `>>>>>>> disk`, the three kinds of line are tinted (a screenshot, light and dark), and the cursor is on the first conflict.
  3. F7 and Shift+F7 go round three conflicts; the palette's "Next conflict" does the same; the announcer's text is "Conflict 2 of 3."
  4. Ctrl+Z after a merge: the text is back and the bar is back with three buttons; Ctrl+S is refused; Ctrl+Y: the merged text and no bar.
  5. Ctrl+S with a conflict left: "Saved … 1 conflict left.", and Undo puts the file back.
  6. A file that git left conflicted (three-part, `HEAD` labels) opens tinted, and F7 finds it.
  7. A merge refused: a 12,000-line file whose every line changed on disk under one edited line → "… couldn't be merged. The two versions differ in too many lines." and the bar stays.
  8. Forced colours (emulated): the conflict's lines carry the bar at their start and no tint; a selection across a tinted line is visible in both themes.
  9. Keyboard alone: Tab reaches Merge from the editor (Esc, then Tab), Enter runs it, and the keyboard is back in the editor.
  10. A Markdown file in Rendered view: Merge works and the keyboard stays in the pane.
  11. A CRLF file: after a merge and a save the file still has CRLF endings (read the bytes).
- [ ] **Step 2: Fix what the check finds,** each with a failing test first, and note each in the research file.
- [ ] **Step 3: Run what CI runs:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -j 4 -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`.
- [ ] **Step 4: Commit** `docs(6q-4b): the live check of the editor's merge.`
