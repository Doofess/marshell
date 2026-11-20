# Slice 6s: tab conversations are copied at each turn end for every CLI, and a wide CLI home is refused by name

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Codex, Qwen Code or Copilot CLI conversation run in a Marshell tab (Windows or WSL) gets its copy in History at each turn end, whatever the import switch says, as Claude Code's does; rows left without a copy are filled at start; and a home variable that names a folder holding the user profile or Marshell's home opens nothing and says so by name (issues #152 and #155).

**Architecture:** `History::note_event` already links every CLI's session to its tab and already returns the meta that `server/hook.rs` hands to `schedule_copy`; only its `ends_turn` test stops the three CLIs. That test becomes a per-CLI table (`history::copy_event`). Copilot's `sessionEnd` and `errorOccurred` name no transcript, so the Copilot link body takes the path the tab's record of that session already holds. A start-up pass reads the sidecars, finds tab conversations with no archive file and copies them through `History::import`, asking for the running distros before it touches a distro's share. The guard keeps the home variables it refuses for being wide, and `preview::locate` builds the refusal sentence from them. No protocol type changes.

**Tech Stack:** Rust (`marshell-core`: rusqlite index, zstd archive, axum hook route), React + TypeScript for one hint string, Vitest through `renderToStaticMarkup`, PowerShell for the live-check script.

**Spec:** `docs/PLAN.md` §9, "Slice 6s decisions (2026-11-21)", decisions 1 to 12. It builds on 6j1 decision 7 (WSL transcripts), 6m decision 10, and 6n2 decisions 7, 11 and 12. Evidence: `docs/research/2026-11-20-copilot-surfaces-live-check.md` (findings 1 and 2), `docs/research/2026-10-24-codex-hooks.md`, `docs/research/2026-10-24-codex-rollouts.md`, `docs/research/2026-11-03-qwen-code-hooks.md`, `docs/research/2026-11-06-copilot-cli.md`.

**Checked before planning (2026-11-21, on `origin/main` 3531eb38, by seven readers and an Opus second reader, then by hand):**
- `crates/core/src/history/recorder.rs:208-213`: the local `codex` is true for `"codex" | "qwen" | "copilot"` and for `"antigravity"`. Line 311, `let ends_turn = !codex && matches!(event, "Stop" | "StopFailure" | "SessionEnd");`, is the only thing that asks for a copy (`ends_turn.then_some(meta)`, line 348), and it also gates the index upsert and the sidecar write (line 321). Qwen and Copilot were never excluded on purpose: commit c557ddc2 widened the `codex` arm.
- Event names reach `note_event` as each CLI sends them. Codex: `Stop`, `Interrupt`, `SessionEnd` (no `StopFailure`; `crates/core/src/agents/codex/hooks.rs:22`). Qwen: `Stop`, `StopFailure`, `SessionEnd`. Copilot, added by the bridge from `--event`: `agentStop`, `errorOccurred`, `sessionEnd` (`crates/core/src/agents/copilot/hooks.rs:25`). A Copilot status line reaches it as `statusLine` (`agents/copilot/mod.rs:153`). Antigravity installs `Stop` and has no transcript reader (`history::parse_for` gives None).
- `crates/core/src/server/hook.rs:412-447` (`record_history`) is the only production caller of `note_event` and `schedule_copy`. It runs only for an open tab (line 255). For Copilot in a WSL tab it calls `copilot::link_body_in_distro(&body)?`, which is None unless the body has `transcriptPath` (only `agentStop` has one) or is a status line. On Windows `copilot::link_body` builds `<Marshell's COPILOT_HOME>/session-state/<id>/events.jsonl` when the body names none.
- `archive::copy_transcript` (`history/archive.rs:77`) reads `meta.transcript_path` as it is, decodes a `.zst` by its extension, writes the archive atomically and merges the sidecar. `History::reindex` parses Claude Code, Codex, Qwen and Copilot archives. So a hook meta of any of the three copies with no new code. A WSL meta already holds the `\\wsl.localhost\<distro>\…` path, because `note_event` translates before it checks or stores (`recorder.rs:237`).
- `schedule_copy` (`recorder.rs:839`): one running copy and one waiting slot per session id, a 1500 ms settle (`SETTLE`, `recorder.rs:21`; `History::set_settle` for tests). `History::import_bytes` (`recorder.rs:711`) answers `Ok(false)` while a hook copy runs, and `finish_copying` runs a hook copy that waited behind an import.
- The import switch is read only by the importer's `Worker::can_start` (`history/import.rs:474`). The hook path never reads it.
- "Linked to a tab" is `tab_id` not empty; the importer writes `tab_id: ""` and `distro: None`. Index rows keep no distro (`index.rs` `read_row`), and `Index::stale` skips a session with no archive file, so only the sidecars (`archive::read_all_meta`, `archive.rs:248`) can list tab rows with no copy. `delete_copy` removes the sidecar (`recorder.rs:649`).
- `AppState` has the running-distro list as a `fn() -> Vec<String>` (`server/mod.rs:319`, `with_running_distros` at 674, `wsl::list_running` at `wsl.rs:78`, which starts nothing). `History` has no access to it. `server/mod.rs:1647` starts the backfill: `drop(state.history().backfill());`.
- `history/mod.rs:113-116`: the Codex arm of `sidecar_allowed_in` checks the id, the place and a `.jsonl`/`.jsonl.zst` ending only: no `rollout-`, no id in the name, no `..` check. Its callers: `recorder.rs:239`, `archive.rs:264`, `server/history.rs:564`, `server/sessions.rs:1242`. The importer's `codex_file` (`import.rs:621`) takes any `rollout-*` and `codex_meta` (`import.rs:846`) takes the id from the first line without comparing it to the name. Test ids are not always UUIDs (`cdx-1` in `crates/core/tests/outside.rs`).
- `crates/core/src/config/guard.rs:132` (`moved_agent_homes_as`) returns only the admitted folders; `Guard::for_home` (line 66) adds them to `allowed`, next to `dirs::home_dir()` (line 58). A refused variable is just absent, so a write under it is `Refusal::OutsideAllowed`, and `preview::locate` (`config/preview.rs:166`) gives "The path is outside the home folder and the project folders." `FileStatus::Refused.detail` is already a `String`, and the frontend (`src/features/agents/agentsState.ts:100`) and the skill route (`server/agents.rs:333`) put "Marshell will not edit <path>: " before it.
- The guard reads the profile with `dirs::home_dir()`; the adapters read `USERPROFILE` (`server/files.rs:227`). A live check's scratch `USERPROFILE` is seen only by the second.
- `projects::is_inside(path, folder)` (`projects.rs:63`) canonicalises both sides when they exist (case, 8.3 names, links, `\\?\`), trims trailing separators and folds ASCII case; `projects::same_path` is its equality.
- `Index::latest_for_tab` and `latest_for_tab_of` (`index.rs:216-227`) order by `updated_ms DESC LIMIT 1` with no tie-break. The `sessions` table has a rowid.
- Tests that pin today's behaviour: `recorder.rs` `a_codex_session_with_only_session_end_is_linked` (~1318), `a_later_codex_event_does_not_rewrite_the_link` (~1332), `qwen_is_linked_by_its_first_event` (~1464), `copilot_is_linked_by_its_first_event` (~1532); the Codex rollout named `…-abc.jsonl` for another id in `recorder.rs` ~1175 and `history/mod.rs` ~246; `src/features/history/HistorySection.test.tsx:19`.
- No fixture exists for Qwen's `StopFailure` or Copilot's `errorOccurred`; Codex's `Interrupt.json` carries `transcript_path`.

## Global constraints

- Tests and live checks never read or write the real `~/.marshell`, `~/.claude`, `~/.codex`, `~/.qwen`, `~/.copilot` or `~/.gemini`. Rust tests use temp folders, `History::open(<temp>)`, `AppState::with_copilot_home(<temp>)` and `ConfigService::with_guard`.
- No new dependency, no new route, no protocol change: `pnpm check:generated` must pass without regenerating.
- Each CLI's event names stay in one table, `history::copy_event`; no other module compares another CLI's event names. Copilot's body shaping stays in `agents/copilot/mod.rs`.
- Antigravity never gets a copy. A Copilot status line never asks for one.
- `guard.rs` changes only as decisions 5, 6 and 12 say (the user's OK: issue #155 and the second round, 2026-11-21). `Guard::check`'s order and its `Refusal` enum don't change.
- Nothing may call a file function on a `\\wsl.localhost\…` or `\\wsl$\…` path of a distro before the running list says that distro runs, and nothing in this slice calls `wsl::default_distro` or `AppState::wsl_distro_of_tab` outside `record_history`.
- Never hold a `MutexGuard` across `.await` or file I/O; file reads and `wsl.exe` run in `spawn_blocking`.
- Rust tests run with `-j 4`. `cargo fmt --all` before every Rust commit. Typecheck with `pnpm build`.
- A worker never runs `git checkout`, `git restore`, `git stash` or `git clean`, never edits a file its task doesn't list, and leaves the untracked `hand-keys-1.json` alone.
- Copy, exactly:
  - The hint under "Copy conversations run outside Marshell": "Marshell reads the conversation folders of Claude Code, Codex, Qwen Code and Copilot CLI and keeps its own copy of each conversation. It never changes their files. Conversations run in a Marshell tab are kept whether this is on or off."
  - The refusal detail: "<VARIABLE> names <folder>, a folder that holds your user profile." and "<VARIABLE> names <folder>, a folder that holds Marshell's own folder." The folder is spelt as the variable spells it (for Qwen's `~` forms, as expanded).
  - Unchanged: "The path is outside the home folder and the project folders." and "This is Marshell's own data folder."

## Review focus

1. **A copy taken before the CLI wrote the turn's last lines.** Codex, Qwen and Copilot flush times are unmeasured. The copy waits the settle, a later copy replaces the whole file, and the session end's copy is the last. Tests in Task 2 (a second Stop replaces a short copy); the live check compares each archive's last line with its source.
2. **A Copilot `sessionEnd` or `errorOccurred` in a tab whose own `COPILOT_HOME` differs from Marshell's, or in a WSL tab.** The row keeps the path its `agentStop` named, and the end's copy reads that path. Tests in Task 3.
3. **The start-up pass meeting a stopped distro, a conversation whose copy was deleted, an Antigravity row, a Claude Code row that never had a prompt, a Codex rollout that is now `.jsonl.zst`, and a transcript that is gone.** No distro starts, nothing deleted comes back by the pass alone, and a missing file is skipped without an error. Tests in Task 4.
4. **A Codex transcript path that names another session's rollout, a file not named `rollout-…`, a name where the id is only the tail of another id (`rollout-<time>-cdx-1.jsonl` for id `1`), or a path with `..`.** None is linked, copied, resumed or imported, and a rollout whose time Codex writes in a new form still is. Tests in Task 1.
5. **A home variable spelt with another case, a trailing separator, a link that leads to a wide folder, `~` for Qwen, the profile itself, and the default `<profile>\.codex`.** The wide ones open nothing and name the variable; the profile and the folders inside it behave as before. Tests in Task 5.

## Known limits (planned)

- An open History view shows a new copy only after it is reopened, a filter changes, a session is renamed or an import finishes (decision 9, issue #157).
- A turn end that reaches Marshell after its tab closed, or as Marshell quits, is copied at the next start (decisions 10 and 12).
- After Copilot's `/clear`, a late `sessionEnd` of the old session is dropped before History sees it (as today); the old session's last turn was copied at its `agentStop`.
- A conversation in a WSL distro's `/tmp` is gone when the distro stops, so the start-up pass has nothing to copy for it.
- The uninstaller can't remove Marshell's hooks from a home the new rule refuses (decision 12).
- With the import switch on, a tab conversation is copied by its hook and again by the importer when the file grows. Both replace the whole copy; the tab, the client and the distro are kept.
- Codex in a WSL tab is covered by tests, not by the live check (decision 11).

---

### Task 1: Codex's transcript must be named for its session (decision 7)

Worker effort: high (a path check that Resume and the index rebuild share).

**Files:**
- Modify: `crates/core/src/history/mod.rs` (the Codex arm of `sidecar_allowed_in`, its doc comments, the test near line 246)
- Modify: `crates/core/src/history/import.rs` (`codex_meta`)
- Modify: `crates/core/src/history/recorder.rs` (only the test fixture near line 1175)
- Test: inline in `history/mod.rs` and `history/import.rs`

**Interfaces:**
- Produces: `fn codex_transcript_allowed(path: &str, session_id: &str, distro: Option<&str>) -> bool` in `history/mod.rs` (private; `sidecar_allowed_in`'s Codex arm returns it). True when `valid_session_id(session_id)`, `readable_place(path, distro)`, no part of the path (split on `/` and `\`) is `..`, and the last component is `rollout-` + time + `-` + `session_id` + `.jsonl` or `.jsonl.zst`, where the time is not empty and holds only ASCII digits and `T`, `-`, `:`, `.`, `Z`, `+`, `_`. No filesystem call.
- `codex_meta` in `import.rs` returns `Prepared::Skip(CODEX)` when `!sidecar_allowed(CODEX, &path.to_string_lossy(), &meta.id)`.

- [ ] **Step 1: Write the failing tests.**
  - `history/mod.rs`, `a_codex_rollout_must_be_named_for_its_session`, with `ID` and `dir = r"C:\Users\me\.codex\sessions\2026\10\24"`:
    - allowed: `{dir}\rollout-2026-10-24T19-31-03-{ID}.jsonl`, the same with `.jsonl.zst`, `{dir}\rollout-2026-10-24T19:31:03.123Z-{ID}.jsonl`, `C:\x\rollout-2026-01-01T00-00-00-cdx-1.jsonl` for id `cdx-1`, and the `\\wsl.localhost\Ubuntu\home\u\.codex\sessions\2026\10\31\rollout-2026-10-31T10-00-00-{ID}.jsonl` form with `Some("Ubuntu")`.
    - refused: `{dir}\rollout-2026-10-24T19-31-03-abc.jsonl` for `ID`; `{dir}\{ID}.jsonl`; `{dir}\rollout-{ID}.jsonl` (no time); `{dir}\rollout--{ID}.jsonl`; `C:\x\rollout-2026-01-01T00-00-00-cdx-1.jsonl` for id `1`; `{dir}\..\rollout-2026-10-24T19-31-03-{ID}.jsonl`; `{dir}\rollout-2026-10-24T19-31-03-{ID}.json`; `{dir}\rollout-2026-10-24T19-31-03-{ID}.jsonl\`; the WSL form with `None`.
  - Change the two fixtures that name `…-abc.jsonl` for another id (`history/mod.rs` ~246, `recorder.rs` ~1175-1186) so the name ends with the session's id; their assertions stay.
  - `history/import.rs`, `a_rollout_named_for_another_session_is_not_imported`: with the `Rig`, a file `rollout-2026-10-21T09-00-00-other.jsonl` whose first line names id `cdx-9` is never archived (`rig.archived("codex", "cdx-9")` stays None after `wait_idle`), while a second, rightly named rollout written after it is.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 --lib history::`. Expected: the two new tests FAIL (the first on its refused cases).
- [ ] **Step 3: Implement** `codex_transcript_allowed` and the `codex_meta` skip. Rewrite the doc comment of `sidecar_allowed` (it says "only its place and kind are checked").
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS. If an existing test fails because its Codex rollout isn't named for its id, fix that fixture's file name, not the rule, and list each in the report.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `fix(history): a Codex transcript is linked, copied, resumed and imported only when its rollout is named for the session.`

### Task 2: Codex, Qwen Code and Copilot CLI ask for a copy at each turn end (decisions 1, 8 and 12)

**Files:**
- Modify: `crates/core/src/history/mod.rs` (add `copy_event` beside `parse_for`)
- Modify: `crates/core/src/history/recorder.rs` (`note_event`, its doc comment, the four tests named above, new tests)
- Modify: `crates/core/src/history/index.rs` (`latest_for_tab`, `latest_for_tab_of`)
- Modify: `crates/core/src/history/import.rs` (one new `Rig` test)
- Modify: `crates/core/tests/hook.rs` (the state helpers shorten the settle)
- Modify: `docs/PLAN.md` (6n2 decision 11 gains the sentence "Slice 6s fixes both (its decisions 1 to 4).")

**Interfaces:**
- Produces, in `history/mod.rs`:

  ```rust
  /// True when `event` is one after which `agent`'s conversation is copied: a turn's end or the session's.
  pub fn copy_event(agent: &str, event: &str) -> bool {
      match agent {
          "claude" | "qwen" => matches!(event, "Stop" | "StopFailure" | "SessionEnd"),
          "codex" => matches!(event, "Stop" | "Interrupt" | "SessionEnd"),
          "copilot" => matches!(event, "agentStop" | "errorOccurred" | "sessionEnd"),
          _ => false,
      }
  }
  ```
- `note_event`: `let ends_turn = super::copy_event(agent, event);`. The local `codex` is renamed `by_first_event` (it means "linked by the first event that names the session", and is true for four CLIs). Nothing else in the function changes: a turn end of the three CLIs now takes the `ends_turn` branch of line 321, so the row's `updated_ms` moves and the sidecar is written, as for Claude Code.
- `Index::latest_for_tab` and `latest_for_tab_of` order by `updated_ms DESC, rowid DESC`.

- [ ] **Step 1: Rewrite the four tests so they state the new rule, and add the new ones** (all in `recorder.rs` unless named):
  - `a_codex_session_with_only_session_end_is_linked`: the `SessionEnd` note returns `Some(meta)` with the tab and the rollout path.
  - `a_later_codex_event_does_not_rewrite_the_link` becomes `a_codex_turn_end_moves_the_row_and_keeps_its_start`: after `CODEX_START` at 1000, `CODEX_END` at 9000 returns `Some`, the row's `started_ms` is 1000 and `updated_ms` 9000, and `previous_in_tab` is unchanged. A `PostToolUse` at 9500 still returns None and writes nothing.
  - `qwen_is_linked_by_its_first_event`: the first `QWEN_STOP` returns `Some`; the later `QWEN_START` still returns None.
  - `copilot_is_linked_by_its_first_event`: `agentStop` returns `Some`; the later `sessionEnd` at 2000 returns `Some` and `updated_ms` is 2000. Drop the "no importer yet" comments.
  - `each_cli_asks_for_a_copy_at_its_own_turn_and_session_ends`: a table over `(agent, event, expected)` through `note_event` with each CLI's body helper: codex `Stop`, `Interrupt`, `SessionEnd` true and `SessionStart`, `UserPromptSubmit`, `PostToolUse`, `StopFailure` false; qwen `Stop`, `StopFailure`, `SessionEnd` true and `SessionStart`, `Notification` false; copilot `agentStop`, `errorOccurred`, `sessionEnd` true and `sessionStart`, `postToolUse`, `statusLine`, `Stop` false; claude `Stop`, `StopFailure`, `SessionEnd` true; antigravity `Stop` and `PostInvocation` false. Qwen's `StopFailure` and Copilot's `errorOccurred` bodies are inline `json!` (no fixture exists): the Qwen `Stop` fixture with the event name replaced, and `copilot_body(dir, id, "errorOccurred")`.
  - `a_tab_copy_fills_the_row_for_codex_qwen_and_copilot` (`#[tokio::test]`): for each of the three, with `history.set_settle(Duration::from_millis(10))`, write a real transcript at the path the body names (copy `tests/fixtures/codex/tui.jsonl`, `qwen/transcripts/0.26.5/…0002.jsonl`, `copilot/transcripts/1.0.106/…0001.jsonl`, with the session id the fixture's own lines carry), note the turn end, pass the meta to `schedule_copy`, wait for `history.is_idle()`, then assert the archive file exists, the decoded archive equals the source bytes, and `history.get(agent, id)` lists a title and `turns > 0`.
  - `a_second_turn_end_replaces_the_copy`: after the first copy, append a line to the source, note a second end, and the decoded archive equals the grown source.
  - `a_wsl_tab_copy_keeps_its_distro`: `archive::copy_transcript` of a meta with `distro: Some("Ubuntu")` and a local transcript path leaves a sidecar whose `distro` is `Some("Ubuntu")` (the share can't exist in a test; `copy_transcript` reads the path as it is).
  - `index.rs`, `the_session_linked_later_wins_a_tie`: two rows of one tab and agent with equal `updated_ms`; `latest_for_tab_of` and `latest_for_tab` give the one inserted second. Then upsert the first again with the same `updated_ms`: the second still wins.
  - `import.rs`, `a_tab_conversation_copied_by_its_hook_and_by_the_importer_stays_the_tabs`: with the `Rig` and the switch on, link a Codex session to tab `t1` with `note_event` (`Stop`), `schedule_copy` it, let the importer scan the same rollout, wait for idle, and assert the sidecar's `tab_id` is `t1`, its client is Marshell, the archive equals the source, and History lists the conversation once.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 --lib history::`. Expected: the rewritten and new tests FAIL (the table on its first true row for codex).
- [ ] **Step 3: Implement** `copy_event`, the one-line change and the rename in `note_event`, and the two `ORDER BY` clauses. Rewrite `note_event`'s doc comment: it no longer says a Codex session "never asks for a copy", and says which events ask for one by naming `copy_event`.
- [ ] **Step 4: Keep the hook tests quick.** In `crates/core/tests/hook.rs`, `state_in` (and any helper that builds an `AppState` without going through it: `copilot_state`, `wsl_state`, `wsl_copilot_state`, `wsl_state_reporting`) calls `st.history().set_settle(Duration::from_millis(10))`. Before the change and after it, time `cargo test -p marshell-core -j 4 --test hook` and put both times in the report.
- [ ] **Step 5: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 6: Commit.** `cargo fmt --all`, then `fix(history): a Codex, Qwen Code or Copilot CLI conversation in a tab is copied at each turn end and at the session's end.`

### Task 3: A Copilot end that names no transcript uses the session's own path (decisions 4 and 12)

**Files:**
- Modify: `crates/core/src/agents/copilot/mod.rs` (`linked`, a new `link_body_known`)
- Modify: `crates/core/src/server/hook.rs` (`record_history`)
- Test: inline in `agents/copilot/mod.rs`; `crates/core/tests/hook.rs`

**Interfaces:**
- Consumes: Task 2's `copy_event` (a `sessionEnd` now reaches the copy).
- Produces, in `agents/copilot/mod.rs`:

  ```rust
  /// `link_body`, for a session the tab already has a record of: `known` is the transcript path that record holds.
  /// The path is the body's own `transcriptPath`, else `known`, else what `link_body` (with a home) or
  /// `link_body_in_distro` (without) would build.
  pub fn link_body_known(body: &Value, copilot_home: Option<&Path>, known: Option<&str>) -> Option<Value>
  ```
  `link_body` and `link_body_in_distro` keep their signatures and behaviour (they pass `known: None`).
- `record_history`, inside its `spawn_blocking` closure and only for Copilot: `known` is `noter.get("copilot", id)`'s `transcript_path` when that row's `tab_id` is this tab, where `id` is `copilot::session_id_of(&body)`; then `copilot::link_body_known(&body, <the Copilot home for a Windows tab, None for a WSL tab>, known.as_deref())?`. The early return when `st.copilot_dir()` is None stays.
- A stored WSL path is already `\\wsl.localhost\<distro>\…`; `recorder::as_windows` passes a path that doesn't start with `/` through, and `copilot_transcript_allowed` accepts it for that distro.

- [ ] **Step 1: Write the failing tests.**
  - `agents/copilot/mod.rs`:
    - `a_body_with_its_own_transcript_path_ignores_the_known_one`.
    - `an_end_with_no_transcript_path_uses_the_known_one`: a `sessionEnd` body, `Some(home)`, `known = Some(r"D:\other\session-state\<id>\events.jsonl")` gives that path, not the one under `home`.
    - `an_end_in_a_distro_with_no_transcript_path_uses_the_known_one`: `None` home, `known = Some(r"\\wsl.localhost\Ubuntu\tmp\c\session-state\<id>\events.jsonl")` gives it; with `known: None` the answer is None, as today.
    - `a_first_event_with_nothing_known_builds_the_path_from_the_home` (today's `link_body`).
  - `crates/core/tests/hook.rs`:
    - `a_copilot_session_end_keeps_the_path_its_agent_stop_named`: with `copilot_state`, post `agentStop` whose `transcriptPath` is under another folder than the state's Copilot home, then `sessionEnd` with no path. The row's `transcript_path` is still the `agentStop` one, and its `updated_ms` moved.
    - `a_wsl_copilot_session_end_is_noted_from_its_linked_path`: with `wsl_copilot_state`, post `agentStop` with a Linux `transcriptPath` (as `a_wsl_copilot_session_links_its_distro_transcript` does), then `sessionEnd` and, in a second run, `errorOccurred`, both with no path. Each time the row keeps the `\\wsl.localhost\Ubuntu\…` path and its `updated_ms` moved.
    - `a_wsl_copilot_end_of_a_session_the_tab_never_linked_notes_nothing`: a `sessionEnd` for an unknown id leaves History empty.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 copilot`. Expected: FAIL.
- [ ] **Step 3: Implement.** `linked` gains the `known` parameter between the body's own path and the built one.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `fix(history): a Copilot CLI session's end is copied from the path its tab already recorded, in a WSL tab too.`

### Task 4: The start-up pass (decisions 3, 4, 10 and 12)

Worker effort: high (concurrency with hook copies, and the rule that no distro is started).

**Files:**
- Modify: `crates/core/src/history/recorder.rs` (`History::copy_uncopied`, `History::catch_up`, `backfill` keeps its signature)
- Modify: `crates/core/src/server/mod.rs` (line ~1647)
- Test: inline in `recorder.rs`

**Interfaces:**
- Produces, on `History`:

  ```rust
  /// Copies every conversation that ran in a Marshell tab and has no copy, when its transcript is still there.
  /// `running_distros` lists the WSL distros that run now; it is asked at most once, and only when a conversation
  /// from a distro is waiting. Returns how many copies were made. Blocking.
  pub fn copy_uncopied(&self, running_distros: &dyn Fn() -> Vec<String>) -> usize

  /// The start-up work, on one blocking thread: `copy_uncopied`, then what `backfill` does.
  pub fn catch_up(self: Arc<Self>, running_distros: fn() -> Vec<String>) -> tokio::task::JoinHandle<()>
  ```
- `copy_uncopied`, in this order for each meta of `archive::read_all_meta(&self.home)` (which already drops a sidecar that fails `sidecar_allowed_in`):
  1. Stop when `self.is_hurrying()`.
  2. Skip when `meta.tab_id` is empty, when `parse_for(&meta.agent, "", &meta.session_id)` is None (Antigravity), or when the archive file of `archive::archive_paths` exists.
  3. When `meta.distro` is `Some(d)`: get the running list (the first time only) and skip unless one of its names equals `d` ignoring ASCII case. No file call on the transcript path happens before this.
  4. The source is `meta.transcript_path` when it is a file; for `codex`, else the same path with `.zst` appended when that is a file; else skip.
  5. `self.import(&meta, &source)`: `Ok(true)` counts, `Ok(false)` (a hook copy of it runs) is skipped, an error is logged with `tracing::warn!` and the pass goes on.
- `catch_up` runs `copy_uncopied(&running_distros)` and then the body of `backfill` (move that body into a private `fn index_stale(&self)` that both call).
- `server/mod.rs`: `drop(state.history().backfill());` becomes `drop(state.history().catch_up(state.0.running_distros));`, with the comment extended by "and copy the tab conversations that have none".

- [ ] **Step 1: Write the failing tests** in `recorder.rs`. A helper `linked(history, agent, dir, id)` notes that CLI's session start (so a row and a sidecar exist, with no copy) and writes its transcript from the fixture Task 2 uses.
  - `the_start_up_pass_copies_a_tab_conversation_that_has_no_copy`: for claude, codex, qwen and copilot, `copy_uncopied(&|| panic!("no distro is waiting"))` returns 1 each, the archive equals the source, and `history.get` lists `turns > 0`.
  - `it_skips_a_conversation_that_has_its_copy`: after one pass a second returns 0 and the archive's modified time is unchanged.
  - `it_skips_a_conversation_no_tab_ran`: a sidecar with `tab_id: ""` and no archive (written with `archive::write_meta` and `index.upsert`).
  - `it_skips_antigravity`.
  - `it_skips_a_transcript_that_is_gone`: returns 0, the row stays listed with 0 turns, and nothing panics.
  - `it_copies_a_distro_conversation_only_while_its_distro_runs`: a sidecar with `distro: Some("Ubuntu")` and a local transcript path. With `&|| vec![]` it returns 0; with `&|| vec!["ubuntu".to_string()]` it returns 1.
  - `it_asks_for_the_running_distros_once`: two distro conversations and one Windows one, with a closure that counts its calls: 1.
  - `it_reads_a_codex_rollout_that_was_compressed_where_it_lay`: the sidecar names `rollout-…-<id>.jsonl`, only `rollout-…-<id>.jsonl.zst` exists; returns 1 and the archive decodes to the rollout's text.
  - `it_stops_when_a_flush_has_started`: after `history.flush(Duration::ZERO)` it returns 0.
  - `a_deleted_copy_is_not_brought_back`: copy a conversation, `delete_copy` it, run the pass: 0, and no archive file.
  - `catch_up_copies_and_then_indexes` (`#[tokio::test]`): one uncopied tab conversation and one archive the index hasn't read; after `history.clone().catch_up(|| Vec::new()).await` both list their turns.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 --lib history::recorder`. Expected: FAIL (the functions don't exist).
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4`. Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `fix(history): at start, a tab conversation that has no copy is copied when its transcript is still there, and no distro is started for it.`

### Task 5: A wide CLI home opens nothing and is refused by name (decisions 5, 6 and 12)

Worker effort: high. `guard.rs` is safety-critical; the change is what the three decisions say and nothing else.

**Files:**
- Modify: `crates/core/src/config/guard.rs`
- Modify: `crates/core/src/config/preview.rs` (`locate`)
- Modify: `docs/PLAN.md` (6n2 decision 12 gains "Slice 6s decision 6 adds one refusal: a folder that holds the user profile or Marshell's home.")
- Test: inline in `guard.rs` and `preview.rs`; `crates/core/tests/agents_api.rs`

**Interfaces:**
- Produces, in `guard.rs`:

  ```rust
  /// What the folder of a refused home variable holds.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Holds { Profile, MarshellHome }

  /// A home variable that opens nothing because its folder holds the user profile or Marshell's home.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct WideHome { pub variable: &'static str, pub folder: PathBuf, pub holds: Holds }

  /// The folders the home variables open, and the variables refused for being wide.
  #[derive(Debug, Default, Clone, PartialEq, Eq)]
  pub struct AgentHomes { pub open: Vec<PathBuf>, pub wide: Vec<WideHome> }

  pub fn agent_homes(env: &dyn Fn(&str) -> Option<String>, marshell_home: &Path) -> AgentHomes
  impl Guard {
      pub fn with_wide_homes(mut self, wide: Vec<WideHome>) -> Self
      /// The refused home variable whose folder `path` is in, if any.
      pub fn wide_home_of(&self, path: &Path) -> Option<&WideHome>
  }
  ```
- `moved_agent_homes(env)` keeps its signature (the test at `agents_api.rs:3896` uses it) and returns `agent_homes`' `open` computed with no Marshell home.
- The private worker function takes what a test must fake: `fn agent_homes_as(env, resolve: &dyn Fn(&Path) -> PathBuf, profiles: &[PathBuf], marshell_home: Option<&Path>) -> AgentHomes`. `agent_homes` passes `canonical`, and as `profiles` both spellings of the profile, each once: `dirs::home_dir()` and `env("USERPROFILE")` on Windows (`env("HOME")` elsewhere) when set and not blank.
- The rule, for each variable's folder that passes today's filter (set, absolute, on a drive as written and as resolved, no `..`, not a root), with "holds X" meaning `projects::is_inside(X, folder)` and not `projects::same_path(X, folder)`, tried on the folder as written and on `resolve(folder)`:
  1. The folder is one of the profiles (`same_path`): `open` (decision 6, second round: the profile adds nothing and stays open).
  2. It holds a profile: `wide`, `Holds::Profile`.
  3. It holds Marshell's home: `wide`, `Holds::MarshellHome`.
  4. Otherwise `open`.
  `WideHome.folder` is the folder as the variable spells it (after the adapter's own reading, so Qwen's `~` is expanded). `variable` is `"COPILOT_HOME"`, `"CLAUDE_CONFIG_DIR"`, `"CODEX_HOME"` or `"QWEN_HOME"`.
- `Guard::for_home`: `let homes = agent_homes(&|k| std::env::var(k).ok(), home);`, `allowed.extend(homes.open)`, and `.with_wide_homes(homes.wide)` on the guard it returns. `Guard::check` and `Refusal` don't change.
- `wide_home_of(path)`: the first stored wide home with `projects::is_inside(path, folder)`.
- `preview::locate`: for `Refusal::OutsideAllowed`, the detail is the wide-home sentence when `guard.wide_home_of(path)` is Some, else today's sentence; the reason stays `RefuseReason::OutsideAllowed`. A private `fn wide_home_detail(wide: &WideHome) -> String` in `preview.rs` builds: `format!("{} names {}, a folder that holds {}.", wide.variable, wide.folder.display(), what)` with `what` "your user profile" or "Marshell's own folder".

- [ ] **Step 1: Write the failing tests.**
  - `guard.rs`, beside the moved-home tests and `#[cfg(windows)]` like them, with `profiles = [D:\u\me]`, Marshell's home `D:\m\home`, through `agent_homes_as` and the `vars` helper:
    - `a_home_variable_that_holds_the_profile_opens_nothing`: for each of the four variables set to `D:\u`, `open` is empty and `wide` is `[WideHome { variable, folder: D:\u, holds: Profile }]`.
    - `the_profile_itself_and_a_folder_in_it_stay_open`: `CODEX_HOME=D:\u\me` and `COPILOT_HOME=D:\u\me\.copilot` are both in `open`, `wide` is empty.
    - `a_home_variable_that_holds_marshells_home_opens_nothing`: `CODEX_HOME=D:\m` gives `Holds::MarshellHome`; `CODEX_HOME=D:\m\home` and `D:\m\home\x` give no wide home (they stay `MarshellOwn` through `check`, as `a_copilot_home_inside_marshells_home_is_still_marshells_own` shows).
    - `a_folder_that_holds_both_names_the_profile`: profiles `[D:\w\me]`, Marshell's home `D:\w\mh`, `CODEX_HOME=D:\w`.
    - `case_and_a_trailing_separator_do_not_hide_a_wide_home`: `CODEX_HOME=d:\U\`.
    - `a_link_to_a_wide_folder_opens_nothing`: `CODEX_HOME=D:\link` with a fake `resolve` that maps it to `D:\u` (as `a_home_that_really_is_on_a_share_adds_nothing` fakes one); `folder` in the result is `D:\link`.
    - `the_second_spelling_of_the_profile_counts`: `profiles = [D:\u\me, E:\scratch\home]`, `CODEX_HOME=E:\scratch` is wide.
    - `a_wide_home_does_not_close_the_others`: `CODEX_HOME=D:\u` and `QWEN_HOME=D:\w\qw` give `open == [D:\w\qw]`.
    - `qwens_tilde_is_the_profile_and_stays_open` (every OS): `QWEN_HOME=~` with `profiles = [dirs::home_dir()]` gives no wide home.
    - `a_write_under_a_wide_home_is_refused_and_names_it`: a guard built `Guard::new(homes.open, vec![], D:\m\home).with_wide_homes(homes.wide)`: `check(D:\u\hooks.json)` is `Err(Refusal::OutsideAllowed)` and `wide_home_of` of it names `CODEX_HOME`; `wide_home_of(D:\elsewhere\x.json)` is None.
    - `each_cli_home_variable_opens_its_own_folder` and the other existing moved-home tests pass unchanged.
  - `preview.rs`:
    - `a_write_under_a_wide_home_says_which_variable`: with a guard that has `WideHome { variable: "CODEX_HOME", folder: <root>/u, holds: Holds::Profile }` (temp folders, every OS), the planned file `<root>/u/hooks.json` is `FileStatus::Refused { reason: RefuseReason::OutsideAllowed, detail }` with `detail == format!("CODEX_HOME names {}, a folder that holds your user profile.", <root>/u)`.
    - `…_or_marshells_own_folder`: `Holds::MarshellHome` gives "…, a folder that holds Marshell's own folder."
    - `another_refused_path_keeps_the_general_sentence`: a path under no wide home still reads "The path is outside the home folder and the project folders."
  - `crates/core/tests/agents_api.rs`, `codex_hooks_plan_under_a_wide_home_is_refused_by_name`, the twin of `copilot_hooks_plan_applies_in_a_home_outside_the_profile`: temp `root`, Marshell's home `root/mh`, `CODEX_HOME=root`; the guard is `Guard::new(vec![], vec![], root/mh).with_wide_homes(vec![WideHome { variable: "CODEX_HOME", folder: root, holds: Holds::MarshellHome }])`; the Codex hooks install plan's preview has `can_apply == false`, its `hooks.json` file is refused with the sentence, and no `root/hooks.json` exists afterwards.
- [ ] **Step 2: Run.** `cargo test -p marshell-core -j 4 --lib config::` and `cargo test -p marshell-core -j 4 --test agents_api wide_home`. Expected: FAIL to compile, then FAIL.
- [ ] **Step 3: Implement.** Update the rustdoc of `moved_agent_homes`, `Guard::for_home` and the module with the new refusal.
- [ ] **Step 4: Run.** `cargo test -p marshell-core -j 4` and `cargo test -p marshell-cli -j 4` (the uninstaller builds the same guard). Expected: PASS.
- [ ] **Step 5: Commit.** `cargo fmt --all`, then `fix(config): a CLI home variable that names a folder holding the user profile or Marshell's home opens nothing, and the refusal names it.`

### Task 6: The hint under the import switch (decision 2) (main thread)

**Files:**
- Modify: `src/features/history/HistorySection.tsx:11` (`HISTORY_IMPORT_HINT`)
- Test: `src/features/history/HistorySection.test.tsx:16-20`

- [ ] **Step 1:** Change the test's literal to the full hint in Global constraints.
- [ ] **Step 2: Run.** `pnpm test src/features/history/HistorySection.test.tsx`. Expected: FAIL on the hint.
- [ ] **Step 3:** Append " Conversations run in a Marshell tab are kept whether this is on or off." to `HISTORY_IMPORT_HINT`.
- [ ] **Step 4: Run.** `pnpm test src/features/history` and `pnpm build`. Expected: PASS.
- [ ] **Step 5: Commit.** `fix(history): the import switch's hint says tab conversations are kept either way.`

### Task 7: The live-check script can start with a wide Codex home (main thread)

**Files:**
- Modify: `.claude/skills/marshell-live-check/live-check.ps1` (a `-CodexHomeWide` switch and its help line)
- Modify: `.claude/skills/marshell-live-check/SKILL.md` (one line for the switch)

- [ ] **Step 1:** Add `[switch]$CodexHomeWide`. With it, `start` sets `CODEX_HOME` to `<Scratch>` itself (the folder that holds the scratch `USERPROFILE`, `home`, and the scratch Marshell home, `mh`) and makes no `cx` folder. It throws when combined with `-CodexHome` or `-CodexAuth`. Nothing else changes; `stop` is untouched.
- [ ] **Step 2:** `pwsh -NoProfile -File .claude/skills/marshell-live-check/live-check.ps1 -?` parses, and a `start` with `-CodexHomeWide -CodexAuth` throws before it creates anything.
- [ ] **Step 3: Commit.** `chore(live-check): the script can start the app with a Codex home that holds the scratch profile.`

### Task 8: Live check and exit (main thread)

Run with the `marshell-live-check` skill, each run in a fresh literal scratch folder. Never bring the app's window to the front. Findings go in `docs/research/2026-11-21-history-tab-copies-live-check.md`, numbered, each with its evidence.

- [ ] **Step 1: Windows tabs, the switch off** (`-QwenFake -CopilotFake -CodexAuth`). In Settings > History turn "Copy conversations run outside Marshell" off and read the hint: it is the Global constraints text, and it is still the switch's description for a screen reader. Then, for Qwen Code and Copilot CLI (fake model) and Codex (the user's free account; trust Marshell's hooks in `/hooks` in the scratch `CODEX_HOME`; two prompts of one line each, such as "Reply with the word one."):
  - Three seconds after each turn end, `GET /v1/history` lists the conversation with a title and that many turns, and `<mh>\history\archive\<agent>\<id>.jsonl.zst` exists. The switch is still off.
  - After the last turn, the archive's last line equals the source's last line. If one is short, record by how much and whether the session end's copy completes it.
  - Codex: start a longer reply and press Esc; the interrupted turn is in the copy within three seconds. Record whether Codex also sent `Stop`.
  - Copilot: `/clear`, one prompt, then `/exit`: two conversations, each with its turns. Qwen: `/quit`: the copy holds the last turn.
  - Open History while a Qwen turn runs: the row changes only after History is reopened (decision 9); record it as the known limit.
- [ ] **Step 2: The start-up pass.** Stop the app by PID. Rename one Qwen archive file in the scratch home (`<mh>\history\archive\qwen\<id>.jsonl.zst` to `…​.jsonl.zst.aside`, by its literal path), start the app again on the same scratch, and without opening any tab `GET /v1/history` lists that conversation's turns again within ten seconds, with a new archive file.
- [ ] **Step 3: WSL** (decision 11). Read `docs/research/2026-11-20-copilot-surfaces-live-check.md`'s WSL part first and list for the user every file its setup puts outside `/tmp` in the Ubuntu distro (6n2 put a wrapper in `~/.local/bin`); ask before creating them and before removing them. Install Copilot CLI and Qwen Code into `/tmp` prefixes with `HOME` and each CLI's home under `/tmp`, and a fake model inside the distro. Then in a Marshell WSL tab of each:
  - Two prompts: History lists the conversation with its turns, and its sidecar holds `distro` and a `\\wsl.localhost\Ubuntu\…` path.
  - Copilot: `/exit`. The log shows no "could not archive transcript", and the copy holds the last turn (the end's copy read the path the tab recorded).
  - `wsl.exe -l --running` before the app starts and ten seconds after: the app's start changes nothing in that list. If the user agrees to `wsl --terminate Ubuntu` for it, also start the app with the distro stopped and show it stays stopped; otherwise record that this case rests on the tests.
- [ ] **Step 4: The guard** (`-CodexHomeWide`, a new scratch). In Settings > Agents, set up Codex's hooks: the card reads "Marshell will not edit <Scratch>\hooks.json: CODEX_HOME names <Scratch>, a folder that holds your user profile.", offers no Allow, and no `<Scratch>\hooks.json` exists afterwards. Claude Code's setup in the same run still previews normally. Check the card with the keyboard alone and in emulated forced colours (the text is the existing refusal line).
- [ ] **Step 5:** Stop each run by PID, after listing its tree and looking for agent CLIs the person started. Ask before removing the scratch folders and the `/tmp` prefixes.
- [ ] **Step 6:** Write the research note and commit it: `docs(research): record slice 6s's live check.` Then the final whole-branch review (`marshell-reviewer`, Opus), one fix pass, and `marshell-ship`. The PR closes #152 and #155.
