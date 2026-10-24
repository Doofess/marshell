# Recording replay and files changed (slice 5c) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Who implements:**
> - Tasks 1 to 6 go to `worker` (Sonnet), one at a time, on `phase-5c`. Tasks 1, 2 and 3 run with `effort: high`: they cover a file parser, file paths from a sidecar, and the save command.
> - The main thread reads each diff, runs the task's tests, and ticks the task.
> - Task 7, the live check, stays in the main thread.

**Goal:** In the History view, a conversation has three tabs: Conversation, Recording (replays its stretch of the terminal recording in Marshell's own xterm.js player) and Files changed (Claude Code's edits, per file). The Export dialog can also save the recording as a redacted asciicast.

**Architecture:**
- **Links.**
  - Each conversation's sidecar (`SessionMeta`, version 2) lists the recording files of the tabs it ran in. The hook route passes the tab's recording to `History::note_event`.
  - `archive::write_meta` keeps the names already in the file, so no writer drops them.
  - An older sidecar with no list falls back to `<tab_id>-*.cast` files that overlap the conversation.
- **Reading a recording.**
  - A new `history/cast.rs` scans a `.cast` once for its summary (size, end, prompt marks, stop marker). A small cache keyed by path, length and modified time holds the result.
  - It serves frames from a byte offset: a seek replays the file up to the target through `vt100` and sends `state_formatted()` as the starting screen, the same reset the live reconnect uses. After that, frames continue from the offset the last answer gave, so a long file is never rescanned from the start.
- **Player.** `castPlayer.ts` is pure timing logic: speed, the idle cap, jump targets and positions. `RecordingPlayer.tsx` mounts a read-only xterm with the live terminal's theme and renderer, the way `ScriptedTerminal` does.
- **Files changed** needs no new route.
  - The transcript parser adds a structured `file` to the Edit and Write tool cards.
  - `filesChanged()` groups the cards by path in TypeScript.
- **Asciicast export** is built in the core from the conversation's parts, starting each part with its screen at the clip point. The output text is redacted in bursts. `save_conversation` learns `.cast`.

**Tech Stack:** Rust (axum, vt100 0.16, serde_json, ts-rs, all already in the tree), React + TypeScript, `@xterm/xterm` 6 (already in the tree), Vitest (node, `renderToStaticMarkup`).

**Spec:**
- `docs/PLAN.md` §9 phase 5:
  - "Slice 5c decisions (2026-10-24)" 1 to 4;
  - "Slice 5a decisions" 1 (slice list);
  - "Slice 5b decisions" 3 (redaction toggle).
- "Slice 2f decisions" 1 to 3: the recording format, the size limit and the stop markers.
- Brief §5.10 "Conversation view": a second tab replays the recording, a third lists the files changed.
- PLAN "Secrets at rest": redaction happens on export.

## Global Constraints

- **No new dependency.** No change to `Cargo.toml`, `package.json` or the lockfiles.
- **Security surface.**
  - The CSP, `capabilities/`, `tauri.conf.json` and `guard.rs` don't change.
  - New routes sit in the token-guarded router beside the other `/v1/history` routes.
  - Every `{agent}/{session_id}` route runs the existing `check(agent, session_id)` before any filesystem call.
- **Recording files are only read.**
  - A file name taken from a sidecar is used only when it matches `^[A-Za-z0-9]{1,32}-[0-9]{1,16}\.cast$`, and it is joined to `paths::recordings_dir(home)`, never used as a path on its own.
  - Nothing in this slice writes, moves or deletes a `.cast`.
- **A recording that is still being written** is read as it stands: an incomplete last line is ignored, not an error.
- **Times.**
  - A part starts at the `<unix ms>` in its file name.
  - An event's absolute time is `started_ms + round(t × 1000)`.
  - Turn times are `Turn.at_ms`. A turn with `at_ms == 0` has no "Play from here".
- **Untrusted text.** Recording output reaches only xterm's `write`, and file paths and tool details render as text. Redaction uses `diagnostics::redact::global()`.
- **Limits.**
  - Frames answer at most 1 MiB of event data each.
  - A sidecar keeps at most 100 recording names, dropping the oldest.
  - The asciicast export is refused over 64 MiB, the `save_conversation` cap.
- **Keyboard and forced colours.**
  - Every new control works with the keyboard alone and in forced colours.
  - The tabs follow the ARIA tabs pattern: arrows, Home and End, automatic activation.
  - Read `frontend-design`, then `good-css`, before writing styles. Colours come from tokens only, with no boxed outlines.
- **Commits.**
  - Run `cargo fmt --all` before each Rust commit.
  - Messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  - Regenerated `src/generated/` files go in the commit that changes them.
- **Exact copy:**

  **Tabs**
  - Tab labels: "Conversation", "Recording" and "Files changed". The last is followed by ` ({n})` when n > 0.

  **Recording tab**
  - Loading: "Loading the recording…"
  - None: "Marshell has no recording of this conversation."
  - Failed: "The recording couldn't be read." followed by the core's reason.
  - Parts, shown when there are 2 or more:
    - the caption `Part {k} of {n}, started {when}`;
    - buttons "Previous part" and "Next part".
  - Switch: the checkbox "Whole recording".
  - Stop marker: the marker text exactly as recorded, as a note under the player.
  - "Play from here" on a turn outside every part: "This turn is outside the recording."

  **Player**
  - The player is a group labelled "Recording player".
  - Buttons: "Play" and "Pause" (one button whose label changes), "Previous mark" and "Next mark".
  - The slider is labelled "Position", with `aria-valuetext` `{m:ss} of {m:ss}`. Times of an hour or more read `{h:mm:ss}`.
  - "Speed" select: 0.5×, 1× (default), 2×, 4×.
  - "Idle time" select: "Keep", "Up to 1 s", "Up to 2 s" (default) and "Up to 5 s".

  **Conversation tab**
  - Turn button: "Play from here", with the label `Play turn {n} from here`.
  - In the Files changed tab, each edit has a button "Go to turn", with the label `Go to turn {n}`.

  **Files changed tab**
  - Heading: `{n} files changed` (`1 file changed`).
  - Note, always shown: "Only edits made with Claude Code's Edit and Write tools are listed. Changes made through shell commands, such as sed or a code generator, aren't."
  - Empty: "Claude Code didn't edit any files in this conversation."
  - Per file: the path, then `+{added} −{removed}`, with an en dash for an unknown count, and "New file" for a created one.
  - Per edit: `Turn {n}`, then the diff.

  **Export dialog**
  - New format radio: "Asciicast (.cast)".
  - With it selected:
    - the Copy button is hidden;
    - the preview shows the first 200 lines, with the note "The preview shows the first 200 lines.";
    - with no recording, the preview shows "Marshell has no recording of this conversation." and Save… is `aria-disabled`.
  - Too large: "This recording is over 64 MiB, too large to export."

## Review Focus

1. **A recording still being written, or cut short.**
   - The input:
     - a `.cast` whose last line has no newline or is half a JSON array;
     - a file that ends with the "Recording stopped: it reached the size limit." marker;
     - a header-only file;
     - a file that has disappeared.
   - Expected, in order: the incomplete line is ignored; the marker shows as the note; the part has zero length and plays nothing; the part is left out.
   - Tests: Task 1 (`scan_*`) and Task 2.
2. **A file name from a sidecar that reaches outside the recordings folder.**
   - The input: a sidecar listing `..\\x.cast`, `C:\\x.cast`, `a/b-1.cast`, `t1-12.cast.exe` and a name of 200 characters.
   - Expected: each is skipped and never opened.
   - Tests: Task 1 (`recording_name_check_table`) and Task 2 (a hostile sidecar gives an empty list).
3. **A seek lands on the wrong screen.**
   - The input:
     - a seek into the middle of a full-screen redraw;
     - a seek after an `r` resize event;
     - a continued frame whose offset isn't at a line start;
     - `from_ms` before the part's start, and `from_ms` after its end.
   - Expected, in order:
     - the starting screen equals what a full replay to that moment shows;
     - the size is the resized one;
     - 400 "Not a frame boundary.";
     - the part's start;
     - an empty frame with no next offset.
   - Tests: Task 1 (`frames_*`).
4. **A secret survives the asciicast export.**
   - The input:
     - `sk-ant-abcdefgh12345678` printed in one burst but split across two `o` events 5 ms apart;
     - the user's profile folder in a prompt line;
     - the title holding a token.
   - Expected: none appears with redaction on, all appear with it off, and the result still parses line by line as asciicast v2.
   - Tests: Task 3.
5. **A conversation resumed in another tab, or restarted in its own.**
   - The input:
     - a conversation's events from tab A, then from tab B after a Resume;
     - a Mode ▸ restart in the same tab, which starts a new recording with the same tab id and session id.
   - Expected:
     - the sidecar lists both recordings in order;
     - parts come back in time order;
     - "Play from here" on a turn from tab A opens part 1.
   - Tests: Task 1 (`note_event_*`) and Task 2.

---

### Task 1: Recording links and the cast reader

**Files:**
- Modify: `crates/core/src/history/mod.rs`
  - `SessionMeta` gains `#[serde(default)] pub recordings: Vec<String>`.
  - `VERSION` becomes 2; the migration from 1 is the identity, and `serde(default)` fills the field.
- Modify: `crates/core/src/history/archive.rs`
  - `write_meta` merges the names already in the file: it keeps the old order, appends new names it hasn't seen, and keeps the last 100.
  - New `pub fn read_meta(home: &Path, agent: &str, session_id: &str) -> anyhow::Result<Option<SessionMeta>>`.
- Modify: `crates/core/src/history/recorder.rs`
  - `note_event(&self, tab_id: &str, recording: Option<&str>, body: &Value, at_ms: u64)`.
  - Whenever it writes the sidecar, and also when `recording` isn't in it yet, `meta.recordings` holds `recording`.
- Modify: `crates/core/src/server/hook.rs`
  - `record_history` passes the file name of `st.session(tab).and_then(|s| s.recording_path())`.
- Create: `crates/core/src/history/cast.rs`
- Test: unit tests in `cast.rs`, `archive.rs` and `recorder.rs`. Fix the struct literals in existing tests with `recordings: vec![]`.

**Interfaces:**
- Produces, in `cast.rs`:
  - `pub fn valid_recording_name(name: &str) -> bool`, using the regex from Global Constraints.
  - `pub fn started_ms(name: &str) -> Option<u64>`, the number after the last `-`.
  - `pub struct CastSummary { pub cols: u16, pub rows: u16, pub started_ms: u64, pub ended_ms: u64, pub marks_ms: Vec<u64>, pub stopped: Option<String> }`.
    - `ended_ms` is the last complete event's time, or `started_ms` when there is none.
    - `marks_ms` holds the `m` events labelled `prompt`.
    - `stopped` is the text of an `m` event that starts with `Recording stopped:`.
  - `pub fn scan(path: &Path, started_ms: u64) -> anyhow::Result<CastSummary>`, a streaming line reader that never holds the whole file.
  - `pub struct Frames { pub cols: u16, pub rows: u16, pub screen: Option<String>, pub events: Vec<CastEvent>, pub next_offset: Option<u64> }`.
  - `pub fn frames(path: &Path, started_ms: u64, from: FrameStart, to_ms: u64, budget: usize) -> Result<Frames, FramesError>`, with `pub enum FrameStart { At(u64), Offset(u64) }`.
    - `At(ms)` replays every event before `ms` into `vt100::Parser::new(rows, cols, 0)`, applying `r` events with `set_size`, and sets `screen = Some(parser.screen().state_formatted())` as a string.
    - `Offset(o)` starts reading at byte `o`, which must be 0 or follow a `\n`; otherwise it fails with `FramesError::NotABoundary`. It sets `screen = None`.
    - It collects events with `at_ms < to_ms` until their `data` adds up to `budget` bytes. `next_offset` is the byte offset of the first event not sent, or `None` at `to_ms` or the end of the file.
    - `cols` and `rows` are the size at the first event sent.
  - `CastEvent { at_ms: u64, code: String, data: String }`, with `#[ts(type = "number")]` on `at_ms`, in `crates/protocol/src/history.rs`. Define it in this task, so Task 2 only adds the route types.

- [ ] **Step 1: Write the failing tests.**
  - `recording_name_check_table`:
    - accepts `t0a1b2c3d4e5f-1761300000000.cast`;
    - refuses `..\x.cast`, `C:\x.cast`, `a/b-1.cast`, `t1-12.cast.exe`, `t1.cast` and a name of 200 characters.
  - `scan_reads_size_end_marks_and_stop`, from a file written with `CastEncoder`: a header 80x24, `o` events, two `prompt` marks and the limit marker.
  - `scan_ignores_an_incomplete_last_line`, and `scan_of_a_header_only_file_has_zero_length`.
  - `frames_at_matches_a_full_replay`:
    - the `screen` at 40% of a fixture with cursor moves and a redraw, fed into a fresh parser, equals `contents()` of a parser fed everything before that time;
    - `frames_at_uses_the_size_after_a_resize`.
  - `frames_continue_from_next_offset_without_gaps`: chained calls with a 64-byte budget return every event exactly once.
  - `frames_refuse_an_offset_inside_a_line`.
  - `write_meta_keeps_recordings_already_listed`, plus the cap at 100.
  - `note_event_lists_each_tab_recording_once_in_order`: tab A's file, then the same session in tab B, then a restart in tab B with a new file.
- [ ] **Step 2: Run `cargo test -p marshell-core history::` and watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `cargo test -p marshell-core` and `pnpm check:generated`. All should pass.**
- [ ] **Step 5: Commit** as `feat(history): list each conversation's recordings in its sidecar and read a recording's summary and frames`.

### Task 2: Recording routes

**Files:**
- Modify: `crates/protocol/src/history.rs`:
  - `RecordingPart { index: u32, file: String, started_ms, ended_ms, cols: u16, rows: u16, span_from_ms, span_to_ms, marks_ms: Vec<u64>, stopped: Option<String> }`. Times are `u64` with `#[ts(type = "number")]`.
  - `RecordingFrames { cols: u16, rows: u16, screen: Option<String>, events: Vec<CastEvent>, next_offset: Option<u64> }`, using Task 1's `CastEvent { at_ms: u64, code: String, data: String }`.
- Create: `crates/core/src/history/recordings.rs`:
  - `pub fn parts(home: &Path, meta: &SessionMeta, span: (u64, u64), cache: &CastCache) -> Vec<RecordingPart>`;
  - `pub struct CastCache`, holding up to 16 summaries keyed by `(PathBuf, len, modified)`. It lives on `History`.
- Modify: `crates/core/src/server/history.rs` and `server/mod.rs`.
- Test: `crates/core/tests/history_api.rs`, plus unit tests in `recordings.rs`.

**Interfaces:**
- Consumes: Task 1's `read_meta`, `valid_recording_name`, `started_ms`, `scan` and `frames`.
- The conversation's span:
  - `span_from_ms` is the first turn's `at_ms` that isn't 0, else the sidecar's `started_ms`;
  - `span_to_ms` is the sidecar's `updated_ms`.
  - A part's `span_from_ms` and `span_to_ms` are that span clipped to the part's own start and end.
- `parts()`:
  - It takes the sidecar's `recordings` that pass the name check and exist.
  - When that list is empty, it uses instead every `<tab_id>-*.cast` in the recordings folder whose start and end overlap the span.
  - It keeps the parts whose start and end overlap the span, sorted by start, and numbers them from 0.
- Routes:
  - `GET /v1/history/{agent}/{session_id}/recordings` answers `Vec<RecordingPart>`, empty when there is none. The scan runs in `blocking`.
  - `GET /v1/history/{agent}/{session_id}/recordings/{part}/frames?from_ms=&to_ms=&offset=`:
    - `offset` set means `FrameStart::Offset`; otherwise `from_ms` (default: the part's start) means `FrameStart::At`.
    - `to_ms` defaults to the part's end.
    - The budget is 1 MiB.
    - An unknown part gives 404 "No such part of the recording.". A bad offset gives 400 "Not a frame boundary.".

- [ ] **Step 1: Write the failing tests.**
  - In `recordings.rs`:
    - `parts_follow_the_sidecar_in_time_order`;
    - `parts_fall_back_to_the_tab_files_that_overlap`, with an overlapping file, a non-overlapping one and another tab's file;
    - `parts_skip_hostile_and_missing_names`, with Review Focus 2's names;
    - `span_is_clipped_to_each_part`.
  - In `history_api.rs`:
    - `recordings_route_lists_parts_and_frames_play_them`: seed a sidecar plus a `.cast` written with `CastEncoder`, then chain frames until `next_offset` is null and assert every `o` event arrives once;
    - `frames_route_answers_404_for_an_unknown_part_and_400_for_a_bad_offset`;
    - `recordings_route_404s_an_unknown_conversation`.
- [ ] **Step 2: Run `cargo test -p marshell-core recordings` and `cargo test -p marshell-core --test history_api recordings`, and watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `cargo test -p marshell-core`, `cargo test -p marshell-protocol` and `pnpm check:generated`. All should pass.**
- [ ] **Step 5: Commit** as `feat(history): serve a conversation's recording parts and their frames`.

### Task 3: Asciicast export

**Files:**
- Modify: `crates/core/src/history/export.rs` (`ExportFormat::Asciicast`, serialised as `asciicast`).
- Modify: `crates/core/src/server/history.rs`, the export handler.
- Modify: `src-tauri/src/export.rs`: accept `.cast`, with the filter "Asciicast" (`cast`).
- Modify: `src/lib/exportName.ts`, `src/features/history/ExportDialog.tsx`, `src/lib/api/history.ts` and their tests.

**Interfaces:**
- Consumes: Task 2's `parts()` and Task 1's `frames()`.
- Produces: `pub fn to_asciicast(home: &Path, parts: &[RecordingPart], title: &str, redactor: Option<&Redactor>) -> Result<String, AsciicastError>`.
  - The header:
    - `version` 2;
    - the first part's `cols` and `rows` as `width` and `height`;
    - `timestamp` from the first part's start, in seconds;
    - `title` as the (redacted) title;
    - `env.TERM` is `xterm-256color`.
  - For each part:
    - Take `frames(At(span_from_ms))`, followed through `next_offset` until `span_to_ms`.
    - Emit the part's `screen` as an `o` event at the part's first time.
    - Emit each later part after an `r` event with its size and an `m` event `Part {k}`.
  - Times run on: each part starts 1.0 s after the previous one ended.
  - Redaction:
    - Consecutive `o` events less than 100 ms apart are merged into one event (at the first one's time, up to 64 KiB) before `redactor.redact` runs over them.
    - The title is redacted too.
  - Output over 64 MiB gives `AsciicastError::TooLarge`.
- Route: `GET .../export?format=asciicast&redact=` answers `application/x-asciicast`. With no parts, it answers 404 "Marshell has no recording of this conversation.". Too large gives 413 with the exact copy.
- TS:
  - `ExportFormat` becomes `"markdown" | "json" | "asciicast"`;
  - `exportFileName` gives `.cast`;
  - `exportTitle` reads the `title` from the asciicast header line.

- [ ] **Step 1: Write the failing tests.**
  - `asciicast_export_is_valid_v2`: every line after the header parses as `[number, "o"|"r"|"m", string]`, times don't decrease, and two parts are joined with `r` and `Part 2`.
  - `asciicast_export_starts_with_the_screen_at_the_clip`.
  - `asciicast_redaction_catches_a_key_split_across_events`, with Review Focus 4's inputs, with the toggle on and off.
  - `asciicast_export_refuses_over_64_mib`, using a test-only lower limit through a `const` parameter or a `_with_limit` variant.
  - `save_name_check_table` gains `my-chat-2026-10-24.cast` accepted, and `.CAST` refused.
  - In TS:
    - `exportFileName(..., "asciicast", ...)` ends in `.cast`;
    - `exportTitle` reads a redacted cast title;
    - the ExportDialog markup with asciicast selected has no "Copy" button and has the 200-line note.
  - In `history_api.rs`: `export_asciicast_answers_cast_and_404s_without_a_recording`.
- [ ] **Step 2: Run them and watch them fail:**
  - `cargo test -p marshell-core asciicast`
  - `cargo test -p marshell-app export`
  - `pnpm test src/lib/exportName.test.ts src/features/history/ExportDialog.test.tsx`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the tests above plus `cargo test -p marshell-core --test history_api`. All should pass.**
- [ ] **Step 5: Commit** as `feat(history): export a conversation's recording as an asciicast, with secrets hidden by default`.

### Task 4: Files changed

**Files:**
- Modify: `crates/protocol/src/history.rs`:
  - `ToolCard` gains `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)] file: Option<FileEdit>`;
  - `FileEdit { path: String, added: Option<u32>, removed: Option<u32>, created: bool }`.
- Modify: `crates/core/src/agents/claude/transcript.rs`. Set `file` for `Edit` and `Write` cards that have a path and are not `error`:
  - a create gives `added = Some(lines)`, `removed = Some(0)` and `created = true`;
  - a `structuredPatch` gives its counts;
  - no result gives `None` for both counts.
- Create: `src/features/history/filesChanged.ts` and `src/features/history/FilesChanged.tsx`.
- Test: `transcript.rs` unit tests, `src/features/history/filesChanged.test.ts` and `FilesChanged.test.tsx`.

**Interfaces:**
- Produces:
  - `filesChanged(c: Conversation): ChangedFile[]`.
    - `ChangedFile = { path: string; added: number | null; removed: number | null; created: boolean; edits: { turn: number; card: ToolCard }[] }`.
    - Paths group after turning `\` into `/`.
    - Files are ordered by first edit.
    - A total is `null` when any edit's count is unknown.
    - `created` is true when the first edit created the file.
  - `editDiff(path: string, card: ToolCard): string`, a unified diff that `DiffView` can parse: a header for `path`, then the card's `detail`. A created file's lines all get `+`.
  - `FilesChanged({ conversation, onGoToTurn }: { conversation: Conversation; onGoToTurn: (turn: number) => void })`.

- [ ] **Step 1: Write the failing tests.**
  - `edit_and_write_cards_carry_the_file`, with the cases above plus an `error` edit, which gets no file.
  - `filesChanged`:
    - two edits to one file in turns 1 and 3, with summed counts;
    - a Windows-separator path grouping with its `/` twin;
    - an unknown count giving `null`.
  - `FilesChanged` markup:
    - the heading `2 files changed`;
    - the note;
    - `+3 −1` and an en dash;
    - "New file";
    - a "Go to turn" button labelled `Go to turn 3`;
    - the empty-state copy;
    - a path holding `<img onerror>` rendered as text.
- [ ] **Step 2: Run them and watch them fail:**
  - `cargo test -p marshell-core transcript`
  - `pnpm test src/features/history/filesChanged.test.ts src/features/history/FilesChanged.test.tsx`
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again, plus `cargo test -p marshell-protocol` and `pnpm check:generated`. All should pass.**
- [ ] **Step 5: Commit** as `feat(history): list the files Claude Code edited in a conversation`.

### Task 5: The player

**Files:**
- Create: `src/features/history/recording/castPlayer.ts` and its test.
- Create: `src/features/history/recording/RecordingPlayer.tsx` and `RecordingPlayer.css`.
- Create: `src/features/history/recording/RecordingTab.tsx` and its test.
- Modify: `src/lib/api/history.ts` and its test.

**Interfaces:**
- Consumes: Task 2's types and routes.
- API:
  - `recordingParts(agent, id): Promise<RecordingPart[]>`;
  - `recordingFrames(agent, id, part: number, q: { fromMs?: number; toMs?: number; offset?: number }): Promise<RecordingFrames>`.
- `castPlayer.ts` holds pure functions:
  - `playbackDelay(gapMs: number, speed: number, idleCapMs: number | null): number`, which is `min(gap, cap) / speed`;
  - `jumpTargets(part: RecordingPart, turnTimes: number[]): number[]`: the prompt marks plus the turn times inside the part, sorted, without duplicates within 500 ms;
  - `nextTarget(targets: number[], posMs: number, by: -1 | 1): number | null`, where "previous" skips a target within 1 s behind `posMs`;
  - `clock(ms: number, totalMs: number): string`, giving `m:ss` or `h:mm:ss` per the copy.
- `RecordingPlayer({ part, frames, turnTimes, startAtMs, whole }: { …; frames: (q) => Promise<RecordingFrames>; … })`:
  - It mounts `@xterm/xterm` inside an effect (as `ScriptedTerminal.tsx` does), with `disableStdin: true`, the theme from `resolveTerminalTheme` and `loadRenderer`.
  - The font size fits `cols` into the host width, between 8 px and the terminal font size. Below that it clips at the right edge and never scrolls the window.
  - A seek resets the terminal, writes `screen`, and then plays the events.
  - Playing writes events on a `requestAnimationFrame` clock using `playbackDelay`, fetching the next chunk when 256 KiB or less is left.
  - An `r` event resizes the terminal and fits the font again.
  - The playable range is `[span_from_ms, span_to_ms]`, or the part's whole length when `whole`.
  - Space on the terminal host toggles play.
- `RecordingTab({ agent, id, parts, turnTimes, request })`:
  - `request` is `{ turn: number; atMs: number } | null`, a "Play from here" waiting to be honoured.
  - It shows the loading, none and failed states, the part caption and buttons, and the "Whole recording" switch, which is per conversation and not remembered.
  - It shows the stop-marker note, and the outside-the-recording message for a request no part covers.

- [ ] **Step 1: Write the failing tests.**
  - `castPlayer`:
    - `playbackDelay(10_000, 2, 2_000) === 1_000`, and no cap keeps the gap;
    - `jumpTargets` merges a mark 200 ms from a turn into one;
    - `nextTarget` at a target's own position with `-1` goes to the one before;
    - `clock(83_000, 760_000) === "1:23"` and `clock(3_723_000, 4_000_000) === "1:02:03"`.
  - `RecordingTab` markup:
    - the loading copy;
    - the none copy;
    - `Part 2 of 3, started …`;
    - the "Whole recording" checkbox;
    - the stop-marker note;
    - the outside message.
  - The player's controls, rendered without xterm (the host renders as an empty group on the server):
    - the "Recording player" group;
    - Play, Previous mark and Next mark;
    - "Position" with `aria-valuetext`;
    - the Speed and Idle time options in the order given.
  - API: the query strings for `recordingFrames`.
- [ ] **Step 2: Run `pnpm test src/features/history/recording src/lib/api/history.test.ts` and watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again and `pnpm build`. Both should pass.**
- [ ] **Step 5: Commit** as `feat(history): replay a conversation's recording in an xterm.js player`.

### Task 6: Conversation tabs and wiring

**Files:**
- Modify: `src/features/history/ConversationView.tsx`, `LiveHistory.tsx`, `History.css` and their tests.

**Interfaces:**
- Consumes: Tasks 4 and 5.
- `ConversationView` gains these props:
  - `tab: "conversation" | "recording" | "files"`;
  - `onTab(t)`;
  - `playableTurns: ReadonlySet<number>`;
  - `onPlayFrom(turn: number)`;
  - `recording: React.ReactNode`, the Recording panel, which `LiveHistory` builds.
- The header stays above the tab list. The tab list sits under the header and its notices.
  - The Conversation panel is today's turns.
  - The Files changed panel is `FilesChanged`. Its "Go to turn" switches to Conversation and scrolls to the turn, as `scrollToTurn` does.
- "Play from here" shows only for turns in `playableTurns`. It switches to Recording with that turn's request.
- `LiveHistory`:
  - It loads `recordingParts` beside the conversation; a failure only empties `playableTurns`.
  - A turn is playable when some part's start and end contain its `at_ms`.
  - Opening another conversation resets the tab to Conversation.
  - Esc keeps closing the History view from any tab, except while focus is inside the player, where Esc first moves focus to the Recording tab.

- [ ] **Step 1: Write the failing tests.**
  - The markup has:
    - `role="tablist"`;
    - three `role="tab"` elements with `aria-selected` and `aria-controls`;
    - the count in "Files changed (2)";
    - "Play from here", labelled `Play turn 2 from here`, only on playable turns.
  - The panel switches by `tab`.
  - A pure `tabKey(current, key)` helper in `ConversationView` covers ArrowLeft, ArrowRight, Home and End, wrapping around.
- [ ] **Step 2: Run `pnpm test src/features/history` and watch them fail.**
- [ ] **Step 3: Implement, including styles for the tab list and the player under forced colours: `SelectedMark` for the chosen tab, and system colours only.**
- [ ] **Step 4: Run `pnpm test` and `pnpm build`. Both should pass.**
- [ ] **Step 5: Commit** as `feat(history): add Recording and Files changed tabs to the conversation view`.

### Task 7: Live check (main thread)

- [ ] **Step 1: Set up.** Run the `marshell-live-check` skill with a scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS` and `CLAUDE_CONFIG_DIR`. Before and after, check that the real `~/.claude/settings.json` modified time and `projects\` are unchanged.
- [ ] **Step 2: Make real recordings.**
  - Start a Claude Code session and give it two turns, one of which edits a file.
  - Restart it with Mode ▸, so there are two recordings.
  - Resume it in a second tab for a third turn.
- [ ] **Step 3: Check over CDP:**
  - The tabs work by keyboard.
  - The Files changed tab shows the edit with its counts and "Go to turn".
  - The Recording tab:
    - lists 2 or more parts in order;
    - plays, pauses and changes speed;
    - the idle cap shortens a long pause;
    - Previous mark and Next mark land on turn starts;
    - "Whole recording" widens the slider's range.
  - "Play from here" on the first and last turns opens the right part at the right moment.
  - Export as asciicast, with redaction on, hides a seeded `sk-ant-` key that the session printed.
  - Forced colours (emulated).
- [ ] **Step 4: Check the look.** Take one screenshot of the player in light and dark.
- [ ] **Step 5: Write up and finish.**
  - Write `docs/research/2026-10-24-replay-and-files-live-check.md` and link it from PLAN.md's phase 5 line.
  - Stop the processes by PID after listing the tree.
  - Ask before removing the scratch folders.

## Known limits (planned)

- A seek replays the part from its start up to the target, so it costs time in proportion to the position: about a second deep in a 200 MB recording. Keyframes can come with 5e's compression work.
- Redaction can miss a secret that the output interleaves with escape sequences (for example, coloured per character), or one printed in pieces more than 100 ms apart.
- Files changed lists only Claude Code's Edit and Write results (decision 3). A tool card's detail is capped at 4,000 characters, so a long diff shows cut, as it does in the conversation.
