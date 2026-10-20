# Diagnostics (slice 3e2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- The core writes redacted, size-capped logs.
- A "Diagnostics…" dialog turns on detailed logs and exports a previewed, redacted `.zip` of logs and setup.
- The hidden perf panel shows startup time, memory and CPU next to keypress→paint latency.
- `docs/research/code-signing.md` says how to get the app signed.

**Architecture:**
- A new `crates/core/src/diagnostics/` module has four parts:
  - a redactor (`redact.rs`);
  - a size-rotating log writer and the `tracing` subscriber (`log.rs`);
  - a process sampler (`perf.rs`);
  - the export bundle (`export.rs`).
- New token-checked routes under `/v1/diagnostics/*` and `/v1/perf`.
- **Saving.** The core builds the zip in memory and never writes outside the Marshell home. The webview hands the bytes to a Tauri command, `save_diagnostics`, which shows the native Save dialog from Rust and writes the chosen file. The webview gets no dialog or file-system permission.
- **UI.** A `DiagnosticsDialog` opened from the palette, and resource lines in `PerfOverlay`.

**Tech Stack:**
- Rust 2024, axum, tokio `spawn_blocking`.
- `tracing-subscriber` 0.3 (new), `zip` (new, deflate only), `regex` (already in the lockfile, now a direct dependency of `crates/core`), `windows-sys` features.
- React and TypeScript, Vitest with `renderToStaticMarkup`.

**Spec:**
- `docs/PLAN.md`:
  - phase 3 scope "Diagnostics" ("`tracing` logs with rotation, a size cap and redaction; "Export diagnostics" …; the hidden perf panel") and "Start code-signing procurement";
  - §9 "Slice 3a decisions" item 6 (the code-signing checklist);
  - §9 "Slice 3e2 decisions (2026-10-20)", items 1–5;
  - cross-cutting "Privacy" ("Crash information stays local in the diagnostics export") and "Budgets beyond latency".
- `docs/adr/0001-threat-model.md`.
- `SCOPE.md`'s dependency policy.

## Global Constraints

- **Logs:**
  - `<home>/logs/marshell.log`, rotated to `marshell.log.1` … `marshell.log.4`: 2 MiB per file (`2 * 1024 * 1024`), 5 files at most.
  - A line is never split across files.
  - Every line passes the redactor before it is written.
  - Info level by default for Marshell's crates (`marshell_core`, `marshell_app`) and warn for the others.
- **Detailed logs** switches Marshell's crates to debug while the app runs. It is not saved, and a restart is back to info.
- **Export contents (decision 2), redacted on the way out:**
  - Marshell's logs, `crash.log`, `bridge.log`.
  - Versions: app, OS, each CLI.
  - Settings, and profiles with every `env` value replaced by `<removed>`.
  - Claude Code's hook install state.
  - One perf snapshot.
  - Never transcripts, recordings, prompts, hook event bodies, layout or history.
- **Redaction:** the exact rules are in Task 1. They replace secrets with fixed placeholders, the user's home folder with `~` (in every spelling), and the user name with `<user>`. Redaction is best-effort: the preview exists so the person can read what leaves.
- **Export form (decision 3):**
  - The dialog lists every file with its size, and each file can be read before saving.
  - The Save dialog suggests `marshell-diagnostics-<YYYY-MM-DD>.zip`.
  - The zip uses the deflate method only.
- **Perf (decision 4):**
  - The overlay polls `GET /v1/perf` once a second while it is shown, and never otherwise.
  - Memory is the working set (RSS on Linux). CPU % is the share of all logical CPUs, so 100% means every core is busy, as in Task Manager.
  - A value that can't be read is `null` and shows as `–`.
  - macOS returns `null` resource values for now.
- **Placement (decision 5):** palette command `{ id: "diagnostics", label: "Diagnostics…", group: "Tools" }` right after `agents` in `src/lib/keymap.ts`. It opens `DiagnosticsDialog` as a LiveApp overlay, the same way Agents… opens.
- **Routes:** all behind `host_guard` and `auth_guard` like every route. No route takes a file path to write.
- **Dependencies:**
  - Add to `crates/core`:
    - `tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "std", "registry"] }`;
    - `zip` with default features off and only its flate2-backed deflate feature (pick the exact feature name of the current release, `deflate-flate2` in zip 2.x, and record it in the ledger);
    - `regex = "1"`;
    - `windows-sys` features `Win32_System_ProcessStatus` and `Win32_System_Registry`.
  - Each new shipped crate gets its line in `THIRD_PARTY_NOTICES.md`. Nothing else.
- **Protocol:**
  - New types go in `crates/protocol/src/diagnostics.rs` with `#[derive(TS)] #[ts(export)]`, and every `u64` gets `#[ts(type = "number")]` (no `bigint`).
  - Run `cargo test -p marshell-protocol` to regenerate `src/generated/` and `pnpm check:generated` to check it.
- **Locks and blocking:** no `MutexGuard` across `.await`. File reads, `claude --version` and process sampling run in `tokio::task::spawn_blocking`.
- **UI:** keyboard-first, with visible focus. Works in forced colours (system colours only, no information by colour alone). Dialog focus uses `useDialogFocus`, as `AgentsDialog` does. Tokens come from `src/styles/`.
- **Housekeeping:** run `cargo fmt --all` before every Rust commit. Commit messages read `type(scope): sentence` and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Scratch only:** tests use temp homes. The live check uses a scratch `MARSHELL_HOME`, never `~/.marshell`.

## Review Focus

- **A secret in a shape the patterns don't name:**
  - `ANTHROPIC_API_KEY=…` inside a profile's `command`;
  - `?token=…` in a URL in a log line;
  - `https://user:pass@host`.

  It reads as a placeholder in the export. Tested in Task 1 (`redacts_key_value_pairs_and_url_credentials`) and Task 4 (`profiles_lose_env_values_and_secrets`).
- **The home folder in another spelling:** forward slashes, other case, JSON-escaped `\\`, or a `MARSHELL_HOME` outside the profile. All of them become `~` or `<marshell-home>`. Tested in Task 1 (`redacts_the_home_folder_in_every_spelling`).
- **Many threads logging across a rotation:** no line is torn or lost to a half-renamed file, there are never more than 5 files, and no file goes over 2 MiB. Tested in Task 2 (`rotation_keeps_five_whole_files_under_the_cap`).
- **An export with nothing to find:**
  - no logs yet, or Claude Code not installed: it still builds, and `about.json` says `found: false`;
  - an expired or unknown preview id: 404, and the dialog says "This export has expired. Prepare it again.".

  Tested in Task 4 (`export_without_logs_or_claude_still_builds`, `unknown_export_is_404`) and Task 6.
- **A session process that exits between the listing and the read:** it is skipped, and the snapshot still returns 200 with the other sessions. Tested in Task 3 (`snapshot_skips_a_process_that_is_gone`).

---

### Task 1: Redactor

**Files:**
- Create: `crates/core/src/diagnostics/mod.rs` (`pub mod redact;`, later tasks add theirs) and `crates/core/src/diagnostics/redact.rs`.
- Modify: `crates/core/src/lib.rs` (`pub mod diagnostics;`), `crates/core/Cargo.toml` (`regex = "1"`).

**Interfaces:**
- Produces:
  - `pub struct Redactor`, with `Redactor::new(user_home: Option<PathBuf>, marshell_home: Option<PathBuf>, user_name: Option<String>) -> Redactor`;
  - `fn remember_token(&self, token: &str)` (interior mutability, so the core can add each launch's token);
  - `fn redact<'a>(&self, text: &'a str) -> Cow<'a, str>`;
  - `pub fn global() -> &'static Redactor`, built once from the user's profile folder (the same lookup `paths.rs` uses for `~`), `paths::home_dir()`, and `USERNAME` on Windows or `USER` elsewhere.

Rules, applied in this order:
1. Every remembered token → `<token>`.
2. Key patterns:
   - `sk-ant-[A-Za-z0-9_-]{8,}` → `<anthropic-key>`;
   - `sk-(?:proj-)?[A-Za-z0-9_-]{20,}` → `<openai-key>`;
   - `gh[pousr]_[A-Za-z0-9]{20,}` and `github_pat_[A-Za-z0-9_]{20,}` → `<github-token>`;
   - `AKIA[0-9A-Z]{16}` → `<aws-key>`;
   - `xox[abprs]-[A-Za-z0-9-]{10,}` → `<slack-token>`;
   - `eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}` → `<jwt>`.
3. `(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}` → `Bearer <redacted>`.
4. `(?i)\b(api[_-]?key|access[_-]?token|token|secret|password|passwd|pwd)(["']?\s*[:=]\s*["']?)[^\s"'&,;]+` → `$1$2<redacted>`. This also catches `?token=` in a URL and `KEY=value` in env text.
5. `://[^/\s:@]+:[^/\s@]+@` → `://<redacted>@`.
6. The Marshell home, when it is not inside the user home → `<marshell-home>`.
7. The user home → `~`. The match is case-insensitive, accepts `\`, `/` and JSON-escaped `\\` as separators, and covers `\\?\` prefixed forms.
8. The user name as a whole word, case-insensitive → `<user>`. Skipped when the name is shorter than 3 characters.

- [ ] **Step 1: Write the failing tests** in `redact.rs`:
  - `redacts_known_key_shapes`: one sample per rule 2 pattern, each replaced by its placeholder; surrounding text kept.
  - `redacts_key_value_pairs_and_url_credentials`:
    - `ANTHROPIC_API_KEY=sk-x` gives `ANTHROPIC_API_KEY=<redacted>`, whichever rule fires first. The test asserts the value is gone.
    - `https://h/p?token=abc123&x=1` keeps `&x=1`.
    - `"password": "hunter2"` becomes `"password": "<redacted>"`.
    - `https://me:pw@host/x` becomes `https://<redacted>@host/x`.
  - `redacts_remembered_tokens`.
  - `redacts_the_home_folder_in_every_spelling`: with home `C:\Users\Home Office`, these all become `~\x` (keeping the original separator after `~`):
    - `C:\Users\Home Office\x`
    - `c:/users/home office/x`
    - `C:\\Users\\Home Office\\x`
    - `\\?\C:\Users\Home Office\x`

    Also: with Marshell home `D:\mh`, the text `D:\mh\logs` becomes `<marshell-home>\logs`.
  - `redacts_the_user_name_as_a_word`: `Home Office` becomes `<user>`, but a user `al` is not applied to `also`.
  - `leaves_plain_text_alone`: returns `Cow::Borrowed`.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib -- diagnostics::redact`. Expected: FAIL (does not compile: `Redactor` not found).
- [ ] **Step 3: Implement `Redactor`** with `regex::Regex` patterns compiled once (`std::sync::LazyLock`). The home matcher is a regex built from the escaped path with `[\\/]+` between the parts, `(?i)`.
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `feat(diagnostics): add a redactor for secrets, the home folder and the user name`.

### Task 2: Size-capped logs and Detailed logs

**Files:**
- Create: `crates/core/src/diagnostics/log.rs`, `crates/protocol/src/diagnostics.rs` (`LoggingState`), `crates/core/src/server/diagnostics.rs`.
- Modify:
  - `crates/core/Cargo.toml` (`tracing-subscriber`);
  - `crates/core/src/paths.rs`: `pub fn marshell_log(home: &Path) -> PathBuf`, `<home>/logs/marshell.log`, next to `crash_log`;
  - `crates/core/src/server/mod.rs` (routes);
  - `crates/protocol/src/lib.rs`;
  - `src-tauri/src/main.rs`: call `log::init` right after `crash::install_panic_hook(&home)`, and on error print to stderr and go on;
  - `crates/core/src/server/mod.rs::start`: `diagnostics::redact::global().remember_token(&token)` once the token exists;
  - `THIRD_PARTY_NOTICES.md`.
- Test: unit tests in `log.rs`, plus `crates/core/tests/diagnostics.rs` (new) for the routes.

**Interfaces:**
- Consumes: `diagnostics::redact::{Redactor, global}`.
- Produces:
  - `pub struct RotatingFile`, with `RotatingFile::new(path: PathBuf, max_bytes: u64, keep: usize) -> RotatingFile` and `fn write_line(&self, line: &[u8]) -> io::Result<()>`. It rotates first when `len + line.len() > max_bytes`, renaming `.3`→`.4` … `log`→`.1` and deleting `.<keep-1>` before the shift. One internal `Mutex`.
  - `pub fn init(home: &Path) -> anyhow::Result<()>` installs the global subscriber:
    - a `fmt` layer without ANSI, wrapped in `reload::Layer` with a `Targets` filter;
    - its `MakeWriter` buffers one event, runs `global().redact`, then calls `write_line`.

    A second call returns `Err`.
  - `pub fn set_detailed(on: bool) -> bool` (false when `init` never ran) and `pub fn state() -> LoggingState`.
  - Protocol: `LoggingState { available: bool, detailed: bool }`.
  - Routes: `GET /v1/diagnostics/logging` gives `LoggingState`. `PUT /v1/diagnostics/logging` takes body `{ "detailed": bool }` and returns `LoggingState`, or 409 "Logging is not running." when unavailable.

- [ ] **Step 1: Write the failing tests:**
  - `writes_lines_and_rotates_at_the_cap`: with `max_bytes` 100 and `keep` 5, write 30 lines of 20 bytes. Exactly `marshell.log` and `.1`–`.4` exist, each ≤ 100 bytes, and the newest line is the last line of `marshell.log`.
  - `rotation_keeps_five_whole_files_under_the_cap`: 8 threads × 500 lines of `"t{n} {i} " + 40 x's + "\n"`, with `max_bytes` 4096. ≤ 5 files, each ≤ 4096. Every line in every file matches `^t\d+ \d+ x{40}$`.
  - `log_lines_are_redacted`: a `RotatingFile` behind the redacting writer, fed `"key sk-ant-abcdefghij"`, holds `<anthropic-key>`. Test the writer type directly, not the global subscriber.
  - `tests/diagnostics.rs::logging_route_reports_unavailable_without_init`: in a test process with no `init`, GET gives `{available:false, detailed:false}` and PUT gives 409.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib -- diagnostics::log` and `cargo test -p marshell-core --test diagnostics`. Expected: FAIL (does not compile).
- [ ] **Step 3: Implement** as above. `Targets` is `marshell_core`/`marshell_app` at INFO (DEBUG when detailed) with a WARN default. The handle is kept in a `OnceLock`.
- [ ] **Step 4: Run** both commands, then `cargo test -p marshell-protocol` and `pnpm check:generated`. Expected: PASS; `src/generated/LoggingState.ts` exists.
- [ ] **Step 5: Commit** `feat(diagnostics): write redacted, size-capped logs and switch detailed logs at run time`.

### Task 3: Perf snapshot

**Files:**
- Create: `crates/core/src/diagnostics/perf.rs`.
- Modify:
  - `crates/protocol/src/diagnostics.rs` (`PerfSnapshot`, `ProcessUsage`, `SessionUsage`);
  - `crates/core/src/pty/mod.rs`: `pub(crate) fn process_tree(root: u32) -> Option<Vec<u32>>`, which is root plus descendants; Windows via `proctree::descendants`, Linux by scanning `/proc/*/stat` parent ids, `None` elsewhere;
  - `crates/core/src/server/mod.rs` (a `Mutex<perf::Sampler>` in `Inner`; route `GET /v1/perf`);
  - `crates/core/Cargo.toml` (`Win32_System_ProcessStatus`);
  - `src-tauri/src/main.rs`: `marshell_core::diagnostics::perf::mark_launch()` as the first line of `main`;
  - `crates/core/src/supervisor.rs`: `perf::mark_ready()` once the first core is serving; later calls do nothing.
- Test: unit tests in `perf.rs`, route test in `tests/diagnostics.rs`.

**Interfaces:**
- Produces:
  - Protocol:
    - `ProcessUsage { processes: u32, memory_bytes: Option<u64>, cpu_percent: Option<f32> }`;
    - `SessionUsage { tab_id: String, usage: ProcessUsage }`;
    - `PerfSnapshot { startup_ms: Option<u64>, core: ProcessUsage, sessions: Vec<SessionUsage>, sampled_at_ms: u64 }`.
  - `perf::Sampler::sample(&mut self, groups: &[(String, Vec<u32>)], now: Instant) -> Vec<ProcessUsage>`. CPU % is (Δ kernel+user time) ÷ (Δ wall × logical CPUs) × 100 against this sampler's previous reading of the same pid, so the first sight of a pid is `None`. Pids absent from the input are forgotten.
  - `perf::read_process(pid: u32) -> Option<(u64 /*working set*/, Duration /*cpu time*/)>`:
    - Windows: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `GetProcessMemoryInfo` + `GetProcessTimes`.
    - Linux: `/proc/<pid>/statm` × page size and `/proc/<pid>/stat` utime+stime.
    - `None` elsewhere.
  - `perf::{mark_launch, mark_ready, startup_ms}`.
  - `pub fn snapshot(st: &AppState) -> PerfSnapshot`, blocking. The groups are the core's own pid and, for each open session with `Session::pid()`, `process_tree(pid)`.

- [ ] **Step 1: Write the failing tests:**
  - `cpu_is_none_first_then_a_share`: two samples of a fake reader, injected through a `sample_with(reader)` variant. 50 ms of CPU over 100 ms of wall on 4 CPUs gives `12.5`.
  - `snapshot_skips_a_process_that_is_gone`: a reader returning `None` for one pid. That group's `processes` counts only the readable pids, and its memory sums them.
  - `startup_is_ready_minus_launch`.
  - Route `perf_lists_each_open_session`: a test core with one `fake-agent` session. 200, `sessions.len() == 1`, the tab id matches, and `core.memory_bytes` is `Some(_)` on Windows.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib -- diagnostics::perf` and `cargo test -p marshell-core --test diagnostics`. Expected: FAIL.
- [ ] **Step 3: Implement.** The route runs `snapshot` in `spawn_blocking`.
- [ ] **Step 4: Run** both, plus `cargo test -p marshell-protocol` and `pnpm check:generated`. Expected: PASS, and no `fake-agent.exe` left running.
- [ ] **Step 5: Commit** `feat(diagnostics): report startup time, memory and CPU for the core and each session`.

### Task 4: Export bundle and its routes

**Files:**
- Create: `crates/core/src/diagnostics/export.rs`.
- Modify:
  - `crates/protocol/src/diagnostics.rs` (`ExportPreview`, `ExportFile`);
  - `crates/core/src/server/diagnostics.rs` (routes);
  - `crates/core/src/server/mod.rs` (an `export::Previews` in `Inner`; routes);
  - `crates/core/Cargo.toml` (`zip`, `Win32_System_Registry`);
  - `THIRD_PARTY_NOTICES.md`.
- Test: unit tests in `export.rs`, route tests in `tests/diagnostics.rs`.

**Interfaces:**
- Consumes:
  - `redact::global()`;
  - `perf::snapshot`;
  - `settings` (the current `Settings` in `AppState`);
  - profiles (the `profiles::get` source);
  - `server::agents::setup_of` for `claude`, made `pub(crate)` if needed;
  - `AppState::claude_version`.
- Produces:
  - `pub struct BundleFile { pub name: String, pub bytes: Vec<u8> }`.
  - `pub fn collect(st: &AppState) -> Vec<BundleFile>`, blocking. In this order, each one redacted and missing ones left out:
    1. `README.txt`. Fixed copy: "Marshell diagnostics. This holds Marshell's logs and setup only: no transcripts, recordings, prompts or hook events. Keys, tokens, your home folder and user name were replaced before saving. Read the files before you share them."
    2. `about.json`: `{ app_version, os, os_version, arch, clis: [{ agent, found, version }] }`.
       - `os_version` on Windows is `DisplayVersion` and `CurrentBuild` from `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`; it is `null` elsewhere.
       - `clis` covers claude, codex and agy: found through the same PATH lookup the launcher uses, versions through the cached `version_of`.
    3. `settings.json`.
    4. `profiles.json`: every `env` value becomes `"<removed>"`, then the whole text is redacted.
    5. `agents/claude.json`: the `AgentSetup`.
    6. `perf.json`: one `PerfSnapshot`.
    7. `logs/marshell.log`, `logs/marshell.log.1` … `.4`.
    8. `logs/crash.log`, `logs/crash.log.1`.
    9. `logs/bridge.log`, `logs/bridge.log.1`.

    Logs are read with sharing that tolerates a writer. On Windows `std::fs::read` already shares read and write; confirm this in a test that holds the file open for writing.
  - `pub fn zip(files: &[BundleFile]) -> anyhow::Result<Vec<u8>>`, deflate.
  - `pub struct Previews`: one pending bundle at most, `put(files) -> String` (a random 16-byte hex id) and `get(id) -> Option<Arc<Vec<BundleFile>>>`. It expires 15 minutes after `put`, and a new `put` replaces the old one.
  - Protocol: `ExportFile { name: String, bytes: u64 }` and `ExportPreview { id: String, files: Vec<ExportFile>, total_bytes: u64 }`.
  - Routes:
    - `POST /v1/diagnostics/export` gives `ExportPreview`.
    - `GET /v1/diagnostics/export/{id}/file?name=<name>` gives `text/plain; charset=utf-8`, the exact redacted bytes. Lossy UTF-8 is fine for logs.
    - `GET /v1/diagnostics/export/{id}/zip` gives `application/zip`.
    - An unknown or expired id, or an unknown name, gives 404 with the body "This export has expired. Prepare it again.".

- [ ] **Step 1: Write the failing tests:**
  - `export_without_logs_or_claude_still_builds`: an empty temp home, with `claude_exe` unset or not found. The names are exactly `README.txt, about.json, settings.json, profiles.json, agents/claude.json, perf.json`, and `about.json`'s claude entry has `found: false`.
  - `profiles_lose_env_values_and_secrets`: a profile with env `ANTHROPIC_API_KEY=sk-ant-…` and command `["pwsh","-c","echo token=abc12345"]`. The output has `"<removed>"`, no `sk-ant-`, and no `abc12345`.
  - `logs_are_included_and_redacted`: write `logs/marshell.log` containing the temp home path and a token. In the bundle they read as `~` and `<token>`.
  - `zip_round_trips`: `zip` then read back with `zip::ZipArchive`. Same names and bytes, and every entry uses `CompressionMethod::Deflated`.
  - `previews_expire_and_replace` (inject the clock).
  - Routes:
    - `export_preview_then_file_then_zip`: sizes in the preview equal the file route's byte lengths, and the zip has the same names.
    - `unknown_export_is_404`, with the body text.
- [ ] **Step 2: Run** `cargo test -p marshell-core --lib -- diagnostics::export` and `cargo test -p marshell-core --test diagnostics`. Expected: FAIL.
- [ ] **Step 3: Implement.** `collect` runs in `spawn_blocking` from the route.
- [ ] **Step 4: Run** both, plus `cargo test -p marshell-protocol` and `pnpm check:generated`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(diagnostics): build a redacted export of logs and setup, previewed before it is zipped`.

### Task 5: Save through the native dialog

**Files:**
- Create: `src-tauri/src/diagnostics.rs`.
- Modify: `src-tauri/src/main.rs` (register `diagnostics::save_diagnostics`).

**Interfaces:**
- Produces:
  - Tauri command `save_diagnostics(window: tauri::Window, request: tauri::ipc::Request<'_>) -> Result<Option<String>, String>`.
    - The body must be `InvokeBody::Raw` (the zip bytes).
    - Header `x-file-name` must match `^marshell-diagnostics-\d{4}-\d{2}-\d{2}\.zip$`; anything else is `Err("Not a diagnostics export.")`.
    - The bytes must start with `PK\x03\x04`; anything else gets the same error.
    - It opens `window.dialog().file().set_parent(&window).set_file_name(name).add_filter("Zip archive", &["zip"])` and calls `blocking_save_file()` inside `spawn_blocking`, as `folder_picker.rs` does.
    - Cancel returns `Ok(None)`. Otherwise it calls `write_export` and returns `Ok(Some(path.display().to_string()))`.
  - `fn write_export(path: &Path, bytes: &[u8]) -> io::Result<()>`: writes to `<path>.part` beside the target, then renames over the target. On error the `.part` file is removed.
- No capability change: the dialog is called from Rust, as `pick_folder` is.

- [ ] **Step 1: Write the failing tests** in `src-tauri/src/diagnostics.rs`:
  - `checks_name_and_zip_magic` (a pure `check(name, bytes) -> Result<(), String>`).
  - `write_export_replaces_and_leaves_no_part`.
  - `write_export_failure_leaves_nothing`: the target's parent is a file, so the write fails.
- [ ] **Step 2: Run** `cargo test -p marshell-app diagnostics`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same command. Expected: PASS.
- [ ] **Step 5: Commit** `feat(app): save a diagnostics export through the native Save dialog`.

### Task 6: Diagnostics dialog

**Files:**
- Create:
  - `src/lib/api/diagnostics.ts`;
  - `src/features/diagnostics/DiagnosticsDialog.tsx` and `DiagnosticsDialog.css`, using tokens only;
  - `src/features/diagnostics/DiagnosticsDialog.test.tsx` and `src/lib/api/diagnostics.test.ts`.
- Modify:
  - `src/lib/keymap.ts` (the `diagnostics` command after `agents`);
  - `src/features/sessions/LiveApp.tsx`: overlay union gets `"diagnostics"`; `runFromPalette` maps it like `agents`; render `<DiagnosticsDialog endpoint onClose onNotice={announce} />`.

**Interfaces:**
- Consumes:
  - generated `LoggingState`, `ExportPreview`, `ExportFile`;
  - `coreFetch`/`api` from `src/lib/api/client.ts`;
  - `invoke` from `@tauri-apps/api/core`;
  - the `Switch` and `EmptyState` primitives;
  - `useDialogFocus`.
- Produces:
  - `diagnostics.logging(ep)`, `diagnostics.setDetailed(ep, on)`, `diagnostics.prepare(ep)`, `diagnostics.file(ep, id, name): Promise<string>`, `diagnostics.zip(ep, id): Promise<ArrayBuffer>`.
  - `saveExport(bytes: ArrayBuffer, name: string): Promise<string | null>`, which is `invoke("save_diagnostics", new Uint8Array(bytes), { headers: { "x-file-name": name } })`.
  - `exportName(now: Date): string`, giving `marshell-diagnostics-YYYY-MM-DD.zip` in local time.
  - `DiagnosticsDialogView` (pure; props: `logging: LoggingState | "loading" | "failed"`, `preview: ExportPreview | "idle" | "preparing" | "expired" | "failed"`, `viewing: { name: string; text: string } | null`, plus handlers) and the `DiagnosticsDialog` container.

Copy, exact:
- Title: "Diagnostics".
- **Detailed logs:**
  - Switch label: "Detailed logs".
  - Help line: "Writes debug lines to Marshell's log until Marshell restarts."
  - When unavailable, the switch is disabled with reason "Logging isn't running in this build."
- **Export section:**
  - Heading: "Export diagnostics".
  - Line: "Logs and setup only: no transcripts, recordings, prompts or hook events. Keys, tokens, your home folder and user name are replaced."
  - Button: "Prepare export".
- **Prepared export:**
  - A list with one row per file: the name, then the size in `formatBytes` (`B`, `KB`, `MB` with one decimal above 1 MB), then a "View" button.
  - A total line: "N files, <size>".
  - Buttons "Save…" and "Prepare again".
- **Viewing a file:**
  - A labelled read-only `<pre tabindex="0">` with the text.
  - "Back to the files" returns focus to that file's View button.
- **After a save:**
  - Success: notice "Saved to <path>".
  - Cancel: nothing.
  - Failure: notice "Couldn't save the export: <message>".
  - Expired (404): the view shows "This export has expired. Prepare it again." and the Prepare button.

- [ ] **Step 1: Write the failing tests:**
  - View markup tests (`renderToStaticMarkup`):
    - the copy above;
    - the file list with sizes and the total;
    - the disabled switch with its reason when unavailable;
    - the expired state;
    - the viewer shows the text.
  - `exportName(new Date(2026, 9, 20))` is `"marshell-diagnostics-2026-10-20.zip"`.
  - `formatBytes(0)` is `"0 B"`, `formatBytes(2048)` is `"2.0 KB"` and `formatBytes(3_250_000)` is `"3.1 MB"`.
  - API tests with a mocked `fetch`: methods, paths and bodies, including the PUT `{detailed}`.
  - The keymap has `diagnostics` after `agents`.
- [ ] **Step 2: Run** `pnpm test src/features/diagnostics src/lib/api/diagnostics.test.ts src/lib/keymap`. Expected: FAIL.
- [ ] **Step 3: Implement.** Match `AgentsDialog`'s structure and focus handling. Keyboard: Tab order follows reading order and Esc closes. Forced colours: borders and focus rings in system colours.
- [ ] **Step 4: Run** the same command and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): add a Diagnostics dialog with detailed logs and a previewed export`.

### Task 7: Resource lines in the perf overlay

**Files:**
- Modify: `src/features/perf/PerfOverlay.tsx`, `src/features/terminal/TerminalView.tsx:325` (pass `endpoint` and `tabId`).
- Create: `src/features/perf/perfLines.ts` and `perfLines.test.ts`, plus `src/lib/api/perf.ts`.

**Interfaces:**
- Consumes: generated `PerfSnapshot`, `LatencyMeter.stats()`.
- Produces: `perfLines(snapshot: PerfSnapshot | null, tabId: string, latency: { p50: number; p95: number; n: number }, renderer: string): string[]`.

Lines, exact shape (`–` for null; MB is `Math.round(bytes / 1_048_576)`; CPU has one decimal):
1. Today's first line, unchanged.
2. `startup <n> ms`
3. `core <m> MB · cpu <c>%`
4. `this tab <p> processes · <m> MB · cpu <c>%`, left out when the tab has no session in the snapshot.
5. `all tabs <s> sessions · <m> MB · cpu <c>%`: sums over sessions. A null counts as nothing, and the sum is null only when every value is null.

- [ ] **Step 1: Write the failing tests** for `perfLines`:
  - a full snapshot;
  - nulls shown as `–`;
  - a tab not in the snapshot;
  - a null snapshot: only line 1 and `startup – ms`.
- [ ] **Step 2: Run** `pnpm test src/features/perf`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - `PerfOverlay` fetches `GET /v1/perf` every 1000 ms while mounted, stopping on unmount and ignoring a failed fetch (keeping the last snapshot).
  - It renders the lines in the existing `<output>`, one per line.
- [ ] **Step 4: Run** `pnpm test src/features/perf src/features/terminal` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(perf): show startup time, memory and CPU in the perf overlay`.

### Task 8: Code-signing checklist

**Files:**
- Create: `docs/research/code-signing.md`.

The note gives sources and the date checked for each fact, and covers:
- **Windows, Azure Artifact Signing** (formerly Trusted Signing):
  - eligibility for an individual developer and for an organisation, and which countries, including whether a developer in Israel qualifies today;
  - cost per month;
  - the identity validation steps;
  - what SmartScreen does with a new certificate.
- **Windows alternatives:** an OV certificate on a hardware token or in a cloud HSM, and SignPath Foundation's free signing for open-source projects (its criteria).
- **macOS:** Apple Developer Program (cost, individual versus organisation, D-U-N-S), Developer ID Application, and notarisation with `notarytool`.
- **Linux:** what is usual (signed repositories or AppImage signatures), and that nothing is required to run.
- **Order of steps:** what the user does (buying, identity) and what the release phase wires into CI (Tauri's `signCommand`, `tauri-action` secrets). No secrets in the repo.

- [ ] Research with current sources, write the note, and commit `docs: add the code-signing checklist`.

### Task 9: Live check and ADR entry

**Files:**
- Create: `docs/research/2026-10-20-diagnostics-live-check.md`.
- Modify: `docs/adr/0001-threat-model.md`, adding a "Diagnostics (3e2)" entry under Entry points and controls:
  - the routes are behind the token;
  - redaction is best-effort, and the preview is the control;
  - the save path comes only from the native dialog in Rust, and no route takes a path to write;
  - logs live in the owner-only home and are redacted per line;
  - the export never includes transcripts, recordings or hook bodies.

**Setup:** a scratch `MARSHELL_HOME` with a space in its path, `pnpm tauri dev` with CDP as in `CLAUDE.md`, and one Claude tab and one shell tab, started for the check.

**Checks:**
1. `logs/marshell.log` exists after start and holds info lines. Turning Detailed logs on adds debug lines within seconds, and off stops them. After a restart the switch reads off.
2. Palette → "Diagnostics…" opens the dialog, and focus lands on its title. Prepare export lists the files with sizes. View shows a log with the scratch home written as `~`.
3. **Save…** opens the native Save dialog with `marshell-diagnostics-<today>.zip`. Drive it through UI Automation, as the folder-picker check did, and save to a scratch folder. The zip opens. Its names and sizes equal the preview. A search of every entry finds:
   - no API token from `endpoint.json`;
   - no scratch home path;
   - no profile root path;
   - no user name.

   Cancel gives no notice.
4. Ctrl+Shift+Alt+P shows startup, core, this tab and all tabs lines, updated each second. The core and session memory are within 20% of the working set in `Get-Process`. With the panel closed, no `/v1/perf` requests are made (watch the webview's fetches).
5. **Keyboard and forced colours:**
   - Every control can be reached with Tab and used with Enter or Space.
   - Esc closes the dialog, and focus returns to where it was.
   - Under emulated forced colours, the borders, focus ring and switch stay visible.

Each finding is fixed with a test that fails first, and listed with its commit.

- [ ] Run the checks, write the doc and the ADR entry, and commit `docs: record the diagnostics live check and its threat model entry`.

### Task 10: Whole-branch review and PR

- [ ] Run the final whole-branch review on Opus, then one fix pass, each fix RED→GREEN.
- [ ] Run the CI-equivalent checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `pnpm test`
  - `pnpm check:generated`
  - `pnpm build`
  - `pnpm tauri build --no-bundle`
- [ ] Open the PR (What it does / Live check / Known limits / Checks), wait for green CI, and merge.
