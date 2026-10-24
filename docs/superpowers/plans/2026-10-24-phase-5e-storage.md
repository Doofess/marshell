# Slice 5e: retention, the backup mirror, disk usage, recording compression and the phase 5 exit check

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** History stops growing without bound when you want it to, and has a copy somewhere else. Finished recordings are zstd-compressed, optional size and age limits remove old recordings and images, a backup folder holds an exact mirror of the history, and a Storage section shows what uses the disk. Phase 5's exit check follows.

**Architecture:** The recorder now closes its file when the session's output ends, so "finished" means something. A new `history::storage` module in the core owns one thread, as the importer does. It ticks every 60 s: it compresses finished plain recordings, runs the retention cleanup when due (at startup, every 24 h, and at once after a limit is saved), and mirrors `<home>/history` into the backup folder. Sidecars keep linking recordings by their `.cast` name, and readers resolve that name to `.cast` or `.cast.zst` on disk, so no sidecar is rewritten. Settings gain the two limits and the backup folder. The History settings dialog gains a Storage section built on two new routes.

**Tech stack:** Rust (std threads, zstd 0.13, rusqlite), ts-rs, React + TypeScript, Vitest through `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9, phase 5 (Scope and Exit lines) and "Slice 5e decisions (2026-10-24)". Brief §5.10 "Retention and backup". PLAN line 885 (attachments follow the session's retention).

## Global constraints

- No new dependencies. zstd, getrandom and the dialog plugin are already in.
- Transcript copies, sidecars and marks files are never removed by retention (decision 1). Only `<home>/history/recordings/*` and `<home>/attachments/<tab>/*` are.
- A recording that is still being written is never compressed, removed or mirrored.
- The backup mirror deletes only inside `<backup folder>/Marshell history/`, and only after that folder's marker names this install. Nothing outside it is ever written or removed.
- Nothing follows a symbolic link or junction: walks use `symlink_metadata` and skip links.
- Sizes are 1024-based, labelled as Windows Explorer labels them: KB, MB, GB, one decimal at GB.
- Allowed limits, exactly: size `None | 1 | 2 | 5 | 10 | 20 | 50` GB; age `None | 30 | 90 | 180 | 365` days. The core refuses any other value with 400.
- New request and response types go in `crates/protocol`, and `pnpm check:generated` passes. Every `u64` gets `#[ts(type = "number")]`.
- New routes sit in the token-guarded router, each with a "refused without the bearer" test. No route takes a path to read; the backup folder is set through the settings PUT, which validates it.
- Never hold a `MutexGuard` across `.await` or file I/O. File work runs on the storage thread or in `spawn_blocking`.
- Tests use temp folders and `MARSHELL_HOME`, never the real `~/.marshell`.
- Copy, exactly:
  - Section heading "Storage". Rows "Transcripts", "Recordings", "Images", "Snapshots", "Logs", "Search index", "Total". A size not read yet shows an en dash.
  - Selects: "Keep recordings and images up to" with "No size limit", "1 GB", "2 GB", "5 GB", "10 GB", "20 GB", "50 GB"; "Keep recordings and images for" with "Forever", "30 days", "90 days", "180 days", "1 year".
  - Limits hint: "Transcripts are always kept. Recordings and images of pinned and bookmarked conversations are never removed."
  - Confirm: "This removes <n> recordings and images now (<size>), the oldest from <d MMMM yyyy>." (`n == 1`: "This removes 1 recording or image now (<size>), from <d MMMM yyyy>."). Buttons "Remove and save" and "Cancel". A limit that removes nothing saves without asking.
  - Last cleanup: "Last cleanup <relative time>: removed <n> files (<size>)." / "Last cleanup <relative time>: nothing to remove." Shown only while a limit is set.
  - Backup: label "Backup folder", value the path or "None", buttons "Choose folder…" and "Stop backup". Hint: "Marshell keeps an exact copy of its history in a \"Marshell history\" folder there. A recording or conversation removed here is removed there too."
  - Backup status: "Last copy <relative time>." / "Not copied yet." / "Can't reach <path>. Marshell will try again in a minute."
  - Backup refusals (400 body, shown as the dialog's error): "That folder doesn't exist." / "Choose a folder outside Marshell's own folder." / "That folder holds another Marshell's backup. Choose another folder." / "That folder already has a \"Marshell history\" folder that Marshell didn't make. Choose another folder."
- Every UI change works with the keyboard alone and in forced colours.

## Review focus

1. **A recording finished between the directory listing and the open-set snapshot, or a tab closed while its writer still flushes.** List the folder first, then take the open set, and also skip any file modified within the last 10 s. Tests in Tasks 3 and 4.
2. **The same backup folder chosen on a second PC or a second Marshell home** (a fresh install pointed at the old OneDrive folder). An exact mirror would wipe the other install's backup. The marker's install id must match, or the folder is refused. Test in Task 5.
3. **A reader holding a plain recording open while it is compressed** (the Recording tab paging with `Offset`). Windows must still allow the delete (Rust opens with share-delete), the zst must be complete before the plain file goes, and an `Offset` page after the swap must return the same events. Tests in Tasks 2 and 3.
4. **The backup folder goes offline** (a USB drive unplugged, OneDrive signed out). The tick logs once, sets the status error, never blocks compression or cleanup, and the next tick that finds the folder copies again. Test in Task 5.
5. **A tightened limit saved while files are being added.** Cleanup applies the limit as saved, not the preview's list, and never removes a pinned or bookmarked conversation's recording or an open tab's files. Tests in Task 4.

---

### Task 1: The recorder closes its file when the output ends

**Files:**
- Modify: `crates/core/src/pty/record.rs` (`Event::Close`, `Recorder::close`, `Recorder::is_closed`)
- Modify: `crates/core/src/pty/session.rs` (`read_loop` closes the recorder after EOF; `Session::open_recording`)
- Modify: the session registry (where `recording_path()` is gathered today, see `server/hook.rs:98`): `open_recordings()`

**Interfaces:**
- Produces:
  - `Recorder::close(&self)`: queues a close after the events already sent; the writer flushes, ends a behind-recording with its marker as today, and drops the file. Never blocks: a full queue still closes (the writer drains and closes).
  - `Recorder::is_closed(&self) -> bool`: true once the writer has dropped the file.
  - After `close`, `output`/`resize`/`marker` are ignored silently and `is_recording()` stays true, so `recording_path()` keeps returning the path for hook events that arrive after the session ended.
  - `Session::open_recording(&self) -> Option<PathBuf>`: the path while the recorder exists, is recording and is not closed.
  - `Registry::open_recordings(&self) -> HashSet<PathBuf>` (use the registry's real type name): every session's `open_recording()`, taken without holding a session lock during I/O.

- [ ] **Step 1: Write the failing tests.**
  - `record::tests::close_flushes_and_closes_the_file`: start, `output(b"last")`, `close()`. Within 2 s `is_closed()` is true; the file's last event carries `"last"`; a later `output(b"after")` is not in the file; `is_recording()` is true.
  - `record::tests::close_with_a_full_queue_still_closes`: with the test gate holding the writer and a queue of 2, send 3 outputs then `close()`; release the gate; within 2 s `is_closed()` is true and the file ends with the "fell behind" marker.
  - `session::tests::the_recording_closes_when_the_output_ends` (use the existing fake-program session test pattern): a program that prints and exits; once the reader is done, `open_recording()` is `None` and `recording_path()` is `Some`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core pty::` — Expected: FAIL (no `close`).
- [ ] **Step 3: Implement.** A `closed: Arc<AtomicBool>` set by the writer thread after the file is dropped. `read_loop` calls `close()` after the loop, before setting `reader_done`. `send` checks a `closing` flag first.
- [ ] **Step 4: Run.** `cargo test -p marshell-core pty::` — Expected: PASS.
- [ ] **Step 5: Commit.** `feat(recording): close a session's recording when its output ends, so a finished recording can be told apart`

Worker effort: high (pty code).

### Task 2: Compressed recordings play, page and export like plain ones

**Files:**
- Modify: `crates/core/src/history/cast.rs` (`resolve`, `open` reads `.zst`, `Offset` on a compressed file)
- Modify: `crates/core/src/history/recordings.rs` (`summaries` and `tab_files` go through `resolve`; `tab_files` returns link names for both forms)
- Modify: `crates/core/src/history/export.rs:195-207`, `crates/core/src/server/history.rs:304` (join through `resolve`)

**Interfaces:**
- Produces:
  - `cast::resolve(dir: &Path, name: &str) -> Option<PathBuf>`: `None` unless `valid_recording_name(name)`; then `dir/name` when it is a regular file, else `dir/<name>.zst` when that is, else `None`. Plain wins when both exist.
  - `cast::scan` and `cast::frames` accept either form, chosen by the `.zst` extension. Offsets are always positions in the uncompressed text, so an offset from a plain read stays valid after compression.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests.** Build the compressed fixture with `zstd::encode_all(plain_bytes, 3)`.
  - `cast::tests::scan_reads_a_compressed_recording_like_plain`: same `CastSummary` from both.
  - `cast::tests::frames_match_on_a_compressed_recording`: `At(t)` gives the same screen and events; `Offset(next_offset)` from the plain read gives the same next page from the zst; an offset that isn't a line start gives `NotABoundary` on both.
  - `cast::tests::resolve_prefers_plain_then_zst_and_refuses_hostile_names`: plain only, zst only, both (plain), neither (`None`), and the hostile names in the existing `valid_recording_name` test give `None`.
  - `recordings::tests::parts_find_a_compressed_recording_by_its_cast_link`: a sidecar linking `t1-1000.cast` with only `t1-1000.cast.zst` on disk gives one part with `file == "t1-1000.cast"`; the tab fallback (no links) finds it too.
  - `export::tests::asciicast_from_a_compressed_recording_matches_plain`.
  - `server::history` route test: `recording_frames` serves a compressed part.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core history::` — Expected: FAIL.
- [ ] **Step 3: Implement.** `open` returns `Lines<Box<dyn BufRead + Send>>`. For `Offset` on a compressed file, decode from the start and discard `offset - 1` bytes (`io::copy` of a `take` into `io::sink`), then check the byte before as the plain path does.
- [ ] **Step 4: Run.** `cargo test -p marshell-core` — Expected: PASS.
- [ ] **Step 5: Commit.** `feat(history): read zstd-compressed recordings in the player, the export and the recording routes`

### Task 3: The storage thread, and compressing finished recordings

**Files:**
- Create: `crates/core/src/history/storage/mod.rs` (`Storage`, `StorageTiming`, the tick), `crates/core/src/history/storage/compress.rs`
- Modify: `crates/core/src/history/mod.rs` (`pub mod storage;`), `crates/core/src/server/mod.rs` (start after the importer at `:799`; halt at shutdown beside `importer.halt()` at `:699`)

**Interfaces:**
- Consumes: `Registry::open_recordings()` (Task 1), `cast::resolve` (Task 2).
- Produces:
  - `StorageTiming { tick: Duration, cleanup_every: Duration, settle: Duration }`, `Default` = 60 s, 24 h, 10 s.
  - `Storage::start_with(home: PathBuf, history: Arc<History>, registry: <registry type>, timing: StorageTiming) -> Arc<Storage>`; `Storage::start(..)` with the default timing.
  - `Storage::nudge(&self, what: Nudge)` with `enum Nudge { Tick, Cleanup }` (Cleanup also runs a tick); `Storage::halt(&self)` returns at once and the thread stops between files; `Drop` halts.
  - `AppState::storage() -> Option<Arc<Storage>>` beside `history()`.
  - `compress::finished_plain(dir: &Path, open: &HashSet<PathBuf>, now: SystemTime, settle: Duration) -> Vec<PathBuf>`: plain `.cast` files with valid names, not open, not modified within `settle`, oldest first by the name's start time. The caller lists the folder before taking `open`.
  - `compress::compress(path: &Path) -> anyhow::Result<u64>` (bytes saved): stream-encode at level 3 to `<name>.zst.tmp`, `sync_all`, rename to `<name>.zst`, then remove the plain file. A plain file whose `.zst` sibling already exists is removed without encoding. A failed remove is logged and retried next tick.
  - One tick, in order: compress (checking halt between files), cleanup when due (Task 4), mirror (Task 5). Each step logs its own failure and the next still runs.

- [ ] **Step 1: Write the failing tests.**
  - `compress::tests::finished_plain_skips_open_recent_and_compressed`: four files (open, modified now, already `.zst`, old plain): only the old plain one, and two old plains come oldest first.
  - `compress::tests::compress_round_trips_and_removes_the_plain_file`: decoded zst equals the original; plain gone; return value is original size minus zst size.
  - `compress::tests::a_plain_file_left_beside_its_zst_is_removed`.
  - `compress::tests::compress_succeeds_while_a_reader_holds_the_plain_file`: open the plain file with `File::open`, compress, then finish reading through the old handle; the bytes match and the plain file is gone.
  - `storage::tests::a_tick_compresses_a_finished_recording_and_skips_an_open_one`: with a fake open set, `nudge(Tick)` and wait up to 2 s.
  - `storage::tests::halt_stops_between_files`: 20 old plain files and a halt right after the nudge; the thread joins within 2 s and leaves every file either plain or zst, never a `.zst.tmp`.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core history::storage` — Expected: FAIL.
- [ ] **Step 3: Implement** as the Interfaces say. The thread is a plain `std::thread` named `storage`, with a channel as the importer's (`import.rs:113-229`), `recv_timeout(tick)`.
- [ ] **Step 4: Run.** `cargo test -p marshell-core` — Expected: PASS.
- [ ] **Step 5: Commit.** `feat(history): compress each finished recording with zstd on a storage thread, and the plain ones already on disk`

### Task 4: Retention limits

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Settings.history_limit_gb`, `history_limit_days`), `crates/protocol/src/history.rs` (`RetentionLimits`, `RetentionPreview`, `CleanupRun`)
- Modify: `crates/core/src/settings.rs` (read, write, validate), `crates/core/src/server/settings.rs` (400 on an unlisted value; `Nudge::Cleanup` after a save that changes a limit)
- Create: `crates/core/src/history/storage/retention.rs`
- Modify: `crates/core/src/server/history.rs`, `server/mod.rs` (route `POST /v1/history/storage/preview`)
- Generated: `src/generated/RetentionLimits.ts`, `RetentionPreview.ts`, `CleanupRun.ts`, `Settings.ts`

**Interfaces:**
- Consumes: `Storage` (Task 3), `archive::read_all_meta`, `marks::read`, the registry's open tab ids (its existing session listing).
- Produces:
  - `RetentionLimits { limit_gb: Option<u32>, limit_days: Option<u32> }`; `Settings` gets the same two fields, `None` by default.
  - `RetentionPreview { files: u32, bytes: u64, oldest_ms: Option<u64> }`; `CleanupRun { at_ms: u64, files: u32, bytes: u64 }`.
  - `retention::Item { path: PathBuf, bytes: u64, at_ms: u64, protected: bool }` (`at_ms` = modified time).
  - `retention::collect(home: &Path, open: &HashSet<PathBuf>, open_tabs: &HashSet<String>, now: SystemTime, settle: Duration) -> Vec<Item>`: every recording file (both forms) and every regular file in `<home>/attachments/<tab>/`, leaving out open recordings, open tabs' attachments and files modified within `settle`. `protected` when a sidecar links the recording (by its `.cast` name) or has the attachment's tab as `tab_id`, and that conversation's marks are `pinned` or have a bookmark.
  - `retention::plan(items: &[Item], limits: &RetentionLimits, now_ms: u64) -> Vec<usize>`: first every unprotected item older than the age limit; then, while the total of all items (protected ones included) less what is chosen is over the size limit (`gb × 1024³`), the oldest unprotected item left.
  - `retention::apply(items: &[Item], chosen: &[usize]) -> CleanupRun`: removes each file, logs and skips one that fails or is already gone, removes an attachment tab folder left empty. Logs `retention removed {files} files ({bytes} bytes)`.
  - `Storage::preview(&self, limits: &RetentionLimits) -> RetentionPreview` (runs `collect` + `plan` on the caller's blocking thread); `Storage::last_cleanup() -> Option<CleanupRun>`.
  - Cleanup runs on the first tick after start, then when `cleanup_every` has passed, and on `Nudge::Cleanup`, with the limits read from the settings file at that moment. No limit set: no cleanup and no `last_cleanup`.

- [ ] **Step 1: Write the failing tests.**
  - `retention::tests::age_limit_removes_only_old_unprotected_items`.
  - `retention::tests::size_limit_removes_oldest_first_until_under`: items of 400, 400, 400 MiB at times 1, 2, 3 with a 1 GB limit choose only the first.
  - `retention::tests::protected_items_count_toward_size_but_stay`: a protected oldest item is skipped and the next oldest goes.
  - `retention::tests::collect_protects_pinned_and_bookmarked_conversations`: a pinned sidecar's recording, a bookmarked sidecar's recording and its tab's attachment are `protected`; an unmarked one's are not.
  - `retention::tests::collect_leaves_out_open_and_recent_files`.
  - `retention::tests::apply_skips_a_file_already_gone_and_removes_an_empty_tab_folder`.
  - `settings` test: `history_limit_gb: Some(3)` and `history_limit_days: Some(7)` are refused with 400; `Some(5)` and `Some(90)` save and read back.
  - Route tests: preview is refused without the bearer; preview with `limit_days: 30` over a home with two old recordings and one new gives `files == 2` and their bytes; after saving that limit, a cleanup run removes the same two files and `last_cleanup` reports them.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core` — Expected: FAIL.
- [ ] **Step 3: Implement** as the Interfaces say.
- [ ] **Step 4: Regenerate and run.** `cargo test -p marshell-protocol`, `cargo test -p marshell-core` — Expected: PASS. `pnpm check:generated` — Expected: exit 0.
- [ ] **Step 5: Commit.** `feat(history): remove the oldest recordings and images past a size or age limit, never a pinned or bookmarked conversation's`

### Task 5: The backup mirror

**Files:**
- Modify: `crates/protocol/src/api.rs` (`Settings.history_backup_dir: Option<String>`), `crates/protocol/src/history.rs` (`BackupStatus`)
- Create: `crates/core/src/history/storage/backup.rs`
- Modify: `crates/core/src/settings.rs`, `crates/core/src/server/settings.rs` (validate the folder; 400 with the refusal copy), `storage/mod.rs` (mirror step of the tick)
- Generated: `src/generated/BackupStatus.ts`, `Settings.ts`

**Interfaces:**
- Consumes: `Storage` (Task 3), `Registry::open_recordings()` (Task 1).
- Produces:
  - `BackupStatus { dir: Option<String>, last_copy_ms: Option<u64>, error: Option<String> }`.
  - `backup::TARGET_NAME = "Marshell history"`; the mirror lives in `<dir>/Marshell history/`, with the marker `.marshell-backup.json` `{ "install_id": "<32 hex>" }`.
  - `backup::install_id(home: &Path) -> anyhow::Result<String>`: read from `<home>/history/install-id`, or 16 random bytes from `getrandom` as hex, written once with `store::write_private_file`.
  - `backup::check_folder(home: &Path, dir: &Path) -> Result<(), BackupRefusal>` with `enum BackupRefusal { Missing, InsideHome, OtherInstall, NotOurs }`, each mapped to the refusal copy. `InsideHome` covers both directions, on canonical paths. `NotOurs` is a target that exists, isn't empty and has no marker.
  - `backup::mirror(home: &Path, dir: &Path, open: &HashSet<PathBuf>, settle: Duration) -> anyhow::Result<MirrorReport>` with `MirrorReport { copied: u32, removed: u32, bytes: u64 }`. It runs `check_folder` first and writes the marker when the target is new. The source is every regular file under `<home>/history` except `index.sqlite*`, `install-id`, `*.tmp`, open recordings and files modified within `settle`. A file is copied when the target's copy is missing or differs in size or modified time: to `<name>.marshell-tmp`, then the source's modified time is set (`File::set_modified`), then a rename. Then every file in the target that isn't in the source set, other than the marker, is removed, and empty folders go.
  - The tick mirrors when `history_backup_dir` is set. A failure sets `BackupStatus.error` (logged once until it changes) and the next tick tries again. Success sets `last_copy_ms` and clears the error. `Storage::backup_status() -> BackupStatus`.
  - "Stop backup" sets the folder to `None`. The backup folder is left as it is.

- [ ] **Step 1: Write the failing tests.**
  - `backup::tests::mirror_copies_new_and_changed_files_and_keeps_times`.
  - `backup::tests::mirror_removes_what_history_no_longer_has` (a removed recording and a deleted copy's three files).
  - `backup::tests::mirror_leaves_out_the_index_open_recordings_and_temp_files`.
  - `backup::tests::mirror_never_touches_anything_beside_its_target`: a file in `<dir>` next to `Marshell history` survives every pass.
  - `backup::tests::check_folder_refuses_home_inside_and_outside`, `..._another_installs_backup` (a marker with another id: refused, and the mirror removes nothing there), `..._an_unmarked_folder_with_files`, `..._a_missing_folder`.
  - `storage::tests::an_unreachable_backup_sets_the_error_and_recovers`: backup dir missing → `error` set; create it → next tick clears `error` and sets `last_copy_ms`.
  - `#[cfg(unix)]` `backup::tests::mirror_does_not_follow_links`.
  - Settings route test: a refused folder gives 400 with the exact copy and isn't saved.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core` — Expected: FAIL.
- [ ] **Step 3: Implement** as the Interfaces say.
- [ ] **Step 4: Regenerate and run.** `cargo test -p marshell-protocol`, `cargo test -p marshell-core` — Expected: PASS. `pnpm check:generated` — Expected: exit 0.
- [ ] **Step 5: Commit.** `feat(history): mirror the history into a backup folder, removing there what is removed here`

### Task 6: The Storage section

**Files:**
- Modify: `crates/protocol/src/history.rs` (`StorageReport`), `crates/core/src/history/storage/mod.rs` (`usage`), `crates/core/src/server/history.rs`, `server/mod.rs` (route `GET /v1/history/storage`)
- Create: `src/lib/api/storage.ts`, `src/features/history/StorageSection.tsx`, `src/features/history/StorageSection.test.tsx`
- Modify: `src/features/history/HistorySettingsDialog.tsx` (+ its test), `src/features/appearance/settingsSync.ts` (`historyLimits`, `setHistoryLimits`, `historyBackupDir`, `setHistoryBackupDir`)
- Generated: `src/generated/StorageReport.ts`

**Interfaces:**
- Consumes: Tasks 3–5; `pickFolder(start)` from `src/lib/folderPicker.ts`.
- Produces:
  - `StorageReport { transcripts: u64, recordings: u64, attachments: u64, snapshots: u64, logs: u64, index: u64, limits: RetentionLimits, last_cleanup: Option<CleanupRun>, backup: BackupStatus }`. Transcripts = everything under `<home>/history/archive`; index = `index.sqlite` with its `-wal` and `-shm`; the rest are their folders (`paths.rs`). Walked in `spawn_blocking`, links skipped.
  - `getStorage(): Promise<StorageReport>`, `previewRetention(limits: RetentionLimits): Promise<RetentionPreview>` in `src/lib/api/storage.ts`.
  - `StorageSectionView` (pure, rendered by the test) and `StorageSection` (loads and saves). The dialog shows it under the import switch.
  - Changing a select calls `previewRetention`. A preview of 0 files saves at once; otherwise the confirm line shows with "Remove and save" (saves) and "Cancel" (puts the select back). Focus stays on the select after either. "Choose folder…" opens `pickFolder(current)` and saves the choice; a 400 shows its body as the error. Sizes and the report reload after each save.
  - Reuse a byte formatter if `src/` has one; otherwise add `formatBytes(n: number): string` in `src/lib/format.ts` (1024-based, "512 KB", "3.4 MB", "1.3 GB").

- [ ] **Step 1: Write the failing tests.**
  - Rust: `storage::tests::usage_counts_each_folder` (a byte count per category from a seeded home); route test refused without the bearer.
  - `StorageSection.test.tsx`: renders every row label with formatted sizes and "Total"; en dashes before the report arrives; the confirm text for 42 files / 1.3 GB / 3 March 2026 and for one file; the last cleanup line only with a limit set; each backup status line; the refusal copy as the error.
  - `HistorySettingsDialog.test.tsx`: the Storage section renders under the switch.
- [ ] **Step 2: Run them.** `cargo test -p marshell-core history::storage`, `pnpm test src/features/history` — Expected: FAIL.
- [ ] **Step 3: Implement.** Native `<select>` elements, as `AppearanceDialog.tsx` uses. The rows are a `<dl>`. Styles go through the frontend-design and good-css skills, tokens only, no boxed outlines.
- [ ] **Step 4: Run.** `cargo test -p marshell-protocol`, `cargo test -p marshell-core`, `pnpm test`, `pnpm check:generated`, `pnpm build` — Expected: PASS.
- [ ] **Step 5: Commit.** `feat(history): show disk usage, retention limits and the backup folder in History settings`

### Task 7: Phase 5 exit check

**Files:**
- Create: `docs/research/2026-10-24-storage-and-phase-5-exit-check.md`
- Modify: `docs/PLAN.md` (phase 5's first line links the note; its Exit line states the measured MB-per-hour budget)

- [ ] **Step 1: Search budget.** `cargo test -p marshell-core --release search_1000_sessions_under_100ms -- --nocapture` — Expected: PASS; record the printed median. Also time `GET /v1/history/search` in the live check against a home seeded with 1000 sidecars and transcripts (median of 20, from CDP).
- [ ] **Step 2: Live check** (marshell-live-check skill, scratch home). Record a heavy Claude Code session (one this check starts, never pressing Enter in it for the user) and a busy shell (`Get-ChildItem -Recurse C:\Windows\System32` repeated) for 10 minutes each. Then check: each recording is compressed within a minute of its tab ending; its plain and compressed sizes, and MB per hour compressed; replay and Offset paging of a compressed part; a 30-day limit's preview and confirm against seeded old recordings and images, with a pinned conversation's recording kept; a backup folder under the scratch folder fills, a removed recording leaves it within a minute, a folder with another install's marker is refused, and renaming the backup folder away shows the warning and then recovers; the Storage rows; keyboard only and forced colours (emulated).
- [ ] **Step 3: State the budget.** The phase 5 Exit line gets the measured heavy-session figure rounded up with about 50% headroom ("recordings stay under <n> MB per hour, compressed"). The note gives the raw numbers.
- [ ] **Step 4: Stop by PID after reading the tree, write the note in the live-check skill's format, and commit.** `docs(history): record the slice 5e live check and the phase 5 exit check`
