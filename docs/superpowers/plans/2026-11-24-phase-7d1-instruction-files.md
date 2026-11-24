# Slice 7d1: what each CLI's instruction files load

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor shows, for each project in `projects.json` and for each installed CLI, which instruction files load at session start, about how many tokens they cost, which load later, which don't load and why, and which text a CLI cuts or skips. That last kind is Broken. More than 4k tokens at start is Waste. Every item is report-only.

**Architecture:** There are three layers, as in 7c.
- **The engine** (`crates/core/src/doctor/instructions.rs`, new) knows no CLI's rules. It has:
  - a bounded, memoized file reader;
  - the import expander for the import syntaxes;
  - the nested-file walk shared by every CLI;
  - the pure `items()` that turns each CLI's `CliLoad` into report items.
- **Each CLI's rules** live in a new `agents/<cli>/instructions.rs`, with `pub fn load(place: &Place, cx: &mut Reader) -> CliLoad`. `Place` (Task 3) names the roots, the project, where the walk up stops, and the folders a test must pin.
- **The server** (`server/doctor_instr.rs`, new) calls the five readers for each project, and for each running managed distro at user level. `run_once` reads them in a blocking read of its own, after the facts. They are not part of `facts_of`, because a fix preview, Keep and Measure call `facts_of` too and must not walk 30 projects.

The protocol gains one optional field on `DoctorItem`, `loads`. The Doctor row renders it as a native `<details>` disclosure.

**Tech Stack:** Rust (serde_json, toml_edit, `ignore` 0.4's `WalkBuilder`, `globset` as a new direct dependency, see Global constraints), React + TypeScript, and Vitest through `renderToStaticMarkup`. The protocol changes once (Task 2), so `src/generated/` is regenerated once and `pnpm check:generated` runs.

**Spec:** `docs/PLAN.md` §9, "Slice 7d decisions (2026-11-24)", 1 to 11. They rest on "Slice 7a decisions" 2, 3 and 7, and on "Slice 7c decisions" 7 and 10. The evidence is `docs/research/2026-11-24-instruction-files.md`. Task 1's section there ("What 7d1's first task saw") is what the CLIs really did, and it overrides the static read above it.

**Checked before planning (2026-11-24, on `origin/main` 5a21847d):**
- No code reads instruction files. `project_doc_max_bytes` appears only as a settings-view key (`agents/settings_view.rs:38`).
- The run is `server/doctor.rs::facts_of` (blocking, ~280), which fills `Gathered`, then `run_once` (~225): `checks::items` + `ext::items` + the CLI doctors' items. `ext::items` takes `ext::Signals { projects, … }`. `Gathered.projects` is `projects::list(home)` (at most 30, `projects.rs:11`), and it is filled only when `doctor_ext::windows_roots(st)` is Some. It is None in a unit test that names no home, so no test reads the developer's files.
- `doctor::ext::ExtRoots` (`doctor/ext.rs:15`) already has `claude_dir`, `claude_settings`, `codex_home`, `qwen_dir`, `gemini_dir`, `copilot_dir` and `home`. `doctor_ext::roots_under(home, None)` builds a distro's roots from its UNC home.
- `ext::tokens(chars) = chars.div_ceil(4)` (`doctor/ext.rs:463`). The 7c skills line reads "<CLI>'s skills add about N tokens to each session".
- Trust readers exist, and neither fits as it is:
  - Codex: `agents/codex/extensions.rs:320` `fn trusted(user, project) -> bool` is private and answers false for a folder with no entry. That is 7c's rule for project configs. Codex leaves project docs out only for a folder marked untrusted (research note §2.6), so Task 5 adds its own reader and uses `trust_key` (`:313`).
  - Qwen Code: `agents/qwen/extensions.rs:255` `fn folder_trusted(rules, project) -> bool` is private, and a folder no rule names isn't trusted. Whether `security.folderTrust.enabled` is on is an inline expression inside `inventory` (~`:441`), not a function.
- Claude Code's settings:
  - `agents/claude/compact.rs:27` `read_settings` reads `<user_home>/.claude/settings.json` with no size cap and refuses every UNC path. So it is wrong under `CLAUDE_CONFIG_DIR` and reads nothing in a distro. Task 4 reads the settings through the `Reader` from `roots` instead.
  - `compact::managed_settings_paths()` gives the real `…\ClaudeCode` folder. A test must never read it: `AppState`'s `claude_compact_paths` override (`server/mod.rs:304`, set by `with_claude_compact_paths`, used in `tests/doctor_api.rs`) pins it.
- `server::local_file` is `files::existing_file` (`files.rs:248`): it refuses network paths and links that lead to them, answers the canonical path, and answers None for a folder.
- `facts_of` is called by the run, by `fix_of` (~`:1000`), by Keep (~`:1340`) and by `doctor_measure.rs`. A test builds a `Gathered { … }` literal at `server/doctor.rs` ~`:1685`.
- `doctor_ext::roots_under` is private in a private module, so `agents/*` can't call it.
- `projects::project_of(cwd)` maps a folder, a worktree included, to its project's root.
- Size caps:
  - `config::preview::MAX_CONFIG_BYTES` is 4 MiB.
  - `server/ignores.rs:33` `read_capped(path, cap) -> Option<Vec<u8>>` returns None for a file that is missing, not regular, or over the cap.
  - Issue #192 asks that every fact reader be capped. The new reader is capped from the start.
- `ignore` 0.4.34 is a direct dependency of `marshell-core`, and `globset` 0.4.21 is in `Cargo.lock` through it. `THIRD_PARTY_NOTICES.md:97` already lists both.
- The Doctor row (`src/features/doctor/DoctorRow.tsx`) renders the title, `detail`, `path` and `cannot_fix`. `Doctor.css:317` has a `forced-colors` block. The project switches `good-css` and `frontend-design` on (`.claude/settings.json:75`).
- `references::project_id(root)` (`references.rs:37`) gives a stable 16-hex id per project root.

## Global constraints

- **Read-only slice.**
  - Nothing in 7d1 writes a CLI file, a project file or a Marshell file. No `EditOp`, no fix arm, no `managed.json` change.
  - Every new item is `fixable: false`. A Broken item here is never "pre-checked", because it has no checkbox.
- **Reads:**
  - Every instruction-file read goes through `doctor::instructions::Reader` and is capped at `MAX_CONFIG_BYTES + 1`. A file over 4 MiB is "too large" and its text is never parsed.
  - On Windows, every path goes through `server::local_file` first, so a network path, or a link to one, is never opened. An import that names one is `NotLoaded("Marshell doesn't read network paths")`.
  - A distro's reader opens only paths under that distro's own UNC root (`\\wsl.localhost\<distro>\`). An absolute Linux import (`@/etc/x.md`) is mapped under that root, and anything else outside it is refused the same way.
  - Reads run in `spawn_blocking`, in `run_once`'s own read (Task 9).
  - A file is read once per run, whichever CLIs or projects name it (the reader memoizes by canonical path).
  - A file over 4 MiB is never read. Its size comes from its metadata, and a CLI that would load it counts 1 character per byte up to that CLI's own limit, with the row note "over 4 MiB, size estimated from its bytes".
  - **Time:** one run's instruction reads stop after 10 seconds. Each project not reached gets the reason "not checked: the check ran out of time" for every CLI.
- **The walk up stops at `Place::ceiling`.** Production passes None (the filesystem root, as the CLIs do). Every test passes its tempdir, so no test sees a real `C:\Users\<name>\.claude\CLAUDE.md` or `C:\CLAUDE.md` above it.
- **The nested walk:**
  - Shared by every CLI, one per project per run.
  - It uses `ignore::WalkBuilder` with git's ignore rules on (`require_git(false)`, so a folder without `.git` honours its `.gitignore` too), hidden files on, no link following and no `.git` folder.
  - It collects every `.md` file below the root once, and each CLI filters that list by its own names. Names match case-insensitively on Windows and exactly in a distro.
  - It looks 12 folders deep and stops at 10,000 entries. When the entry cap stops it, every CLI's notes for that project get "Marshell stopped looking for files in subfolders after 10,000 entries."
- **Items never quote a file's content.** They give names, paths, sizes and counts only.
- **Token figures:**
  - Every figure is `ext::tokens(chars)`, which is characters divided by 4, rounded up.
  - From 1,000 up it reads as `N.Nk`, with one decimal, rounded half up (2,449 → "2.4k", 2,450 → "2.5k"). Below 1,000 it is the plain number.
  - Text says "about".
  - The Waste threshold is more than 4,000 tokens at start, for one CLI in one project.
- **Ids:**
  - `instr.<project_id>` for a project line, `instr.user` when `projects.json` is empty, and `instr.user@<distro>` in a distro.
  - `instrsize.<agent>.<project_id>` (or `.user`, plus `@<distro>`) for the Waste item.
  - `instrcut.<agent>.<project_id>.<n>` for a Broken cut of one file, where `<n>` is the first 8 hex digits of `config::hash::hash_bytes` of the file's canonical path with forward slashes, lowercased on Windows. A distro adds `@<distro>`.
  - `instrcut.codex.<project_id>.budget` and `instrcut.antigravity.<project_id>.budget` for the two cuts that cover several files, so the id stays the same as sizes change.
- **Paths in the expansion:**
  - Project files are relative to the project root, with forward slashes (`CLAUDE.md`, `docs/AGENTS.md`).
  - User files start with `~/` when under the user home.
  - Anything else is a full path.
- **Each CLI's rules stay in `agents/<cli>/instructions.rs`.** `doctor/instructions.rs` names no CLI except in `display_name` calls, and no CLI module reads another CLI's settings.
- **Copy, exactly:**
  - **Project line:**
    - Title: "<project folder name>: at start, <first>, <next>, … ." The CLIs come in `checks::ORDER`, installed ones only. With no installed CLI there are no instruction items at all.
    - The first CLI reads "<CLI> loads about <T> tokens of instructions", or "<CLI> loads no instructions". Each later one reads "<CLI> <T>" or "<CLI> none".
    - A CLI with a reason gets it in brackets straight after, whatever its total: "Codex 1.2k (Codex doesn't trust this folder)". User-level files still count for a CLI whose project files are out.
    - Detail: "Estimates: characters divided by 4. Files that load later aren't counted."
  - **No projects:** title "Outside a project: at start, …" (same list), same detail.
  - **Distro:** title "In <distro>, at start: <CLI> loads about <T> tokens of user instructions, …"; detail "Projects inside <distro> aren't checked."
  - **Waste item:**
    - Title: "<CLI> loads about <T> tokens of instructions at every start in <project>". With no projects, "<project>" is "every folder"; in a distro it is "<distro>".
    - Detail: "The largest: <path> (<t>), <path> (<t>), <path> (<t>)." It names up to 3 files, by their own counted tokens.
    - `cannot_fix` is the CLI's way, exactly:
      - Claude Code: "Move the parts it needs only sometimes into .claude/rules/ files with paths:, or into a skill."
      - Codex: "Move the parts about one folder into an AGENTS.md in that folder."
      - Antigravity: "Move the parts it needs only sometimes into rules with trigger: model_decision."
      - Qwen Code: "Move the parts it needs only sometimes into .qwen/rules/ files with paths:."
      - Copilot CLI: "Move the parts it needs only sometimes into .github/instructions/ files with applyTo."
  - **The expansion:**
    - The disclosure's summary is "Show the files".
    - Each CLI's heading is "<CLI>: about <T> tokens at start".
    - A file row is `<path>` followed by its note. Notes are joined with ", ", from these phrases:
      - "about <t> tokens";
      - "imported by <path>";
      - "auto memory, first 200 lines or 25 KB";
      - "loads when used";
      - "not loaded: <reason>";
      - "cut: <what>";
      - "managed by your organisation";
      - "over 4 MiB, size estimated from its bytes".
    - Under a CLI's heading come its notes, one sentence each: the walk's stop, "Marshell can't read <path>.", and what isn't known about that CLI. A CLI with no size limit gets "<CLI> documents no limit." (decision 10).
  - The cut items' copy is in Tasks 4 to 8 and is exact.
- **New dependency (the user's OK, 2026-11-24, decision 12):** `globset = "0.4.21"`, the version already in `Cargo.lock`, goes into `crates/core/Cargo.toml`.
  - The need: Claude Code's `claudeMdExcludes` holds picomatch-style globs on absolute paths, and no crate in the tree matches a glob against an absolute path except through `ignore`'s root-relative builders.
  - It is BurntSushi's crate, actively maintained, MIT/Unlicense, with no network access, and it already ships inside the app through `ignore`.
  - `THIRD_PARTY_NOTICES.md` already lists it. With the user's OK, its line 99 becomes: "Linked into `marshell-app.exe`; it reads `.gitignore` files for the Files panel and the Doctor, and matches Claude Code's `claudeMdExcludes` for the Doctor." Task 4 makes that one edit and no other in the file.
- **No change to `guard.rs`,** token handling or file permissions.
- **Tests never read the real homes.**
  - Every test builds its homes and projects under `tempfile::tempdir()`, and passes a `Place` whose roots come from `ExtRoots::under(<temp home>)` (Task 3), whose `ceiling` is the tempdir, and whose `claude_managed` and `qwen_system_settings` are None or under the tempdir.
  - No test calls `windows_roots` without `ext_home` pinned. With `ext_home` pinned, the server (Task 9) sets the ceiling to each project's own folder and takes Claude Code's managed folder only from `claude_compact_paths`.
- **Commands:**
  - Rust: `cargo test -p marshell-core -j 4 <filter>`, with `cargo fmt --all` before every Rust commit.
  - Protocol: `cargo test -p marshell-protocol` and `pnpm check:generated`.
  - Frontend: `pnpm test src/features/doctor`.
- **Struct literals:** a new field on `DoctorItem` breaks every literal. Grep `DoctorItem {` under `crates/` and give each `loads: Vec::new()`. `DoctorItem` has no `Default`; adding one is fine if the task wants it. Task 9's new `Gathered` field breaks the literal in `server/doctor.rs`'s tests (~`:1685`).
- **Briefs to workers:**
  - Never run `git checkout`, `git restore`, `git stash` or `git clean` on files they didn't create.
  - Scratch goes under a new folder in `%TEMP%` that they made, and they delete nothing else.

## Review focus

1. **A CLAUDE.md that is a link to AGENTS.md** (common), or an AGENTS.md that CLAUDE.md also imports: the reader memoizes by canonical path, so Claude Code counts it once. Copilot CLI reads both names, so they appear as two rows: the second "not loaded: same text as <path>" when the bytes are identical, otherwise both counted. Tests: Task 4 `a_linked_claude_md_counts_once`, and Task 8 `identical_files_count_once`.
2. **An import cycle or a self-import** (`@CLAUDE.md` inside CLAUDE.md, or A imports B imports A) never loops. Each file is expanded once per CLI load, and the depth limit holds. Test: Task 3 `an_import_cycle_stops`.
3. **A project folder that no longer exists, is a file, is on a network path, or can't be read:** the line still comes, each CLI keeps its user-level files and gets the reason "Marshell can't read this folder", and the run doesn't fail. Test: Task 9 `a_missing_project_gives_a_line_and_no_failure`.
4. **A huge checkout** (node_modules, target, a monorepo): the walk honours `.gitignore`, skips `.git` and stops at 10,000 entries. A run over 30 such projects stays bounded. Test: Task 3 `the_walk_stops_at_the_entry_cap`, which uses a cap passed in, not 10,000 real files.
5. **Non-UTF-8 or binary text in an instruction file** (a UTF-16 file saved by Notepad, or a stray PNG named AGENTS.md): characters are counted lossily, so nothing panics and nothing is quoted. Test: Task 3 `non_utf8_text_is_counted_lossily`.

## Known limits (planned)

- **Environment switches aren't read:** Claude Code's `CLAUDE_CODE_DISABLE_CLAUDE_MDS` and `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD`, Copilot CLI's `COPILOT_CUSTOM_INSTRUCTIONS_DIRS`, and the CLIs' `--bare`, `--safe-mode`, `--no-custom-instructions` and `--setting-sources` flags. They depend on how a session is started, not on files.
- **Only the start folder:** the estimate is for a session started at the project's root. A session started in a subfolder loads more with Codex, Copilot CLI and Claude Code (7d decision 6).
- **Plugin instruction files** (Antigravity's `plugins/<name>/rules/`, Qwen Code's extension context files) aren't read. Phase 8's Extensions covers plugins (7c decision 6).
- **Antigravity's `rules.json` manifest** isn't read: only the rule files in the rules folders are.
- **Qwen Code's auto memory** is counted wherever its files exist: no setting that turns it off was looked for, and its 150-unit limit per line isn't modelled.
- **A Claude Code file that loads later and is over 40,000 characters** gets a row note, not a Broken item: Claude Code loads its start and tells the agent to read the rest, so nothing is lost for good. This is the lead's reading of decision 4, for the user to judge.
- **Below the root, only `.md` names are looked for,** 12 folders deep. A Codex fallback name without `.md` is found in the root-to-project chain but not below it.
- **Claude Code's `--add-dir` folders** aren't checked.
- **What 7d2 adds:** missing imports, stale paths, duplicates and clashes. Until then a missing import shows only as a "not loaded: not found" row.

---

### Task 1: What the CLIs really do (main thread, before any code)

Decision 10. Use the `marshell-live-check` skill and follow its rules for homes, PIDs and the desktop. Its `start` runs the dev app from this branch (still `origin/main`'s code) on a new, literal scratch folder, and the sessions run in that app's tabs. No credential file is read or copied by hand (SCOPE.md):
- **Codex and Antigravity** sign in through the skill's `-CodexAuth` and `-AgyAuth` flags, with `-CodexHome` as the skill says.
- **Qwen Code and Copilot CLI** run on the skill's fake models (`-QwenFake`, `-CopilotFake`). Their request logs (`qwen-fake.jsonl`, `copilot-fake.jsonl`) hold the exact text each CLI sends, so their facts come from bytes, not from asking a model.
- **Claude Code** uses the scratch `CLAUDE_CONFIG_DIR`, and the user signs in to it themselves (decision 13). Tell the user when the tab is ready. Don't run `/model` or `/effort` there.

The scratch project is `proj` (a `git init` folder) with a second checkout `proj-wt` made by `git worktree add`.

- [ ] **Step 1: Claude Code.**
  - Plant a `CLAUDE.md` of 199 lines and one of 201 lines (two starts), and read the start-up warning text and `/status`.
  - Plant a chain of imports 6 deep, each file holding a unique marker, then ask "Which MARKER-n lines do you see in your instructions? Answer with the numbers only." This gives the depth.
  - Plant `@missing.md` and look for any message about it.
  - Start one session in `proj` and one in `proj-wt`, and note the auto memory folder each uses under `<CLAUDE_CONFIG_DIR>\projects\`. This shows whether a worktree shares the main checkout's memory, and how the folder's name is built.
  - Check whether `25 KB` is 25,000 or 25,600 bytes, by planting a `MEMORY.md` with markers around both offsets.
- [ ] **Step 2: Antigravity.**
  - Plant `.agents/rules/a.md` (`trigger: always_on`) holding an `@[x](sub/inc.md)` include, plus `sub/inc.md` and `inc.md` with different markers. Ask which marker it sees: this gives the include's base (the including file's folder, or the project root).
  - Point an include at a missing file and look for any message.
  - Plant `sub/GEMINI.md` with a marker and ask before and after a prompt that reads `sub/x.txt`. This shows whether nested files load at start or when used.
  - Plant a rule with no `trigger:` and ask whether its marker is seen.
  - Note which of `~/.gemini/GEMINI.md` and `~/.gemini/config/GEMINI.md` load when both exist.
- [ ] **Step 3: Copilot CLI** (fake model; read `copilot-fake.jsonl`).
  - Plant a 64 KB `AGENTS.md` with markers every 8 KB, and find the last marker in the request. If none is cut, try 256 KB once; the limit is then "not seen up to 256 KB".
  - Plant `CLAUDE.md` with `@AGENTS.md` beside `AGENTS.md`, and count how often AGENTS.md's marker is in the request.
  - Plant `@missing.md` and see what the request holds in its place.
  - Plant `sub/CLAUDE.md` and `sub/AGENTS.md`, and see whether either is in the first request.
- [ ] **Step 4: Codex.**
  - Plant 40,000 bytes of project `AGENTS.md` and read the warning text.
  - Three starts in `proj`: with `trust_level = "untrusted"`, with `"trusted"`, and with no entry for the folder. Ask each time whether the marker is seen. This settles what a folder with no entry loads.
- [ ] **Step 5: Qwen Code** (fake model; read `qwen-fake.jsonl`).
  - Confirm that `context.fileName = ["CLAUDE.md"]` drops `AGENTS.md`, and see what a missing `@x.md` leaves in the request.
  - With `security.folderTrust.enabled` on: a folder with no rule, one with `TRUST_FOLDER`, and one with `DO_NOT_TRUST`. See which requests hold the project file's marker.
  - Set `context.fileName` in the project's `.qwen/settings.json` only, and see whether it is followed.
- [ ] **Step 6: Memory files of the other four** (decision 8). For each of Codex, Antigravity, Qwen Code and Copilot CLI, look in its home after the sessions above, and in its request or docs, for a file the CLI writes itself and loads at every start. Qwen Code's `save_memory` is expected to append to `~/.qwen/QWEN.md`, which is already counted as a context file. Record "none found" where that is the result.
- [ ] **Step 7: Record.** Write "What 7d1's first task saw" at the end of `docs/research/2026-11-24-instruction-files.md`, numbered findings with the version of each CLI.
  - Update this plan's constants table (below) wherever a finding differs, and amend PLAN §9 7d decision 10 with one line naming what was settled.
  - Stop with the skill's `stop`, which ends only what it started and deletes the sign-in copies. Remove the scratch folder only with the user's OK.
  - Commit: `docs(research): what the five CLIs load, seen in real sessions.`

**The constants the later tasks use** (Task 1 updates this table; each lives in that CLI's `instructions.rs`). "Finding <n>" is a numbered finding under "What 7d1's first task saw" in the research note:

| CLI | Constant | Planned value | Task 1 settles |
|---|---|---|---|
| Claude Code | `IMPORT_DEPTH` (hops below the root file) | 4 | 4 or 5 |
| Claude Code | `MEMORY_LINES`, `MEMORY_BYTES` | 200, 25_000 | the byte count |
| Claude Code | memory folder | `projects::project_of(project)`'s root, every non-alphanumeric character as `-` | the name, and whether a worktree shares it |
| Claude Code | `LATER_CHARS` (note only) | 40_000 | — |
| Claude Code | `SKIP_BYTES` | 4 MiB (`MAX_CONFIG_BYTES`) | — |
| Claude Code | `WARN_LINES` | 200 (note only) | the real line count |
| Codex | default `project_doc_max_bytes` | 32_768 | — |
| Codex | a folder with no trust entry | loads its project files | seen: loads (finding 1) |
| Antigravity | `RULE_BYTES`, `RULES_BUDGET_TOKENS` | 24_000, 20_000 | — |
| Antigravity | include base | the including file's folder | seen: that folder (finding 5) |
| Antigravity | nested `GEMINI.md`/`AGENTS.md` below the project | not loaded | seen: only a session started in that folder or below loads it (finding 8) |
| Antigravity | rule without a valid `trigger` | not loaded | seen: not loaded (finding 7) |
| Qwen Code | `IMPORT_DEPTH` | 5 | — |
| Qwen Code | folder trust on, no rule for the folder | loads no project file | seen: loads none (finding 13) |
| Qwen Code | `context.fileName` in a project's settings | followed | seen: followed (finding 11) |
| Codex, Antigravity, Copilot CLI | a memory file of their own | none | seen: none found (findings 3, 9, 19) |
| Qwen Code | auto memory: `MEMORY_LINES` | 200, for `~/.qwen/memories/MEMORY.md` and `~/.qwen/projects/<name>/memory/MEMORY.md` | seen (finding 14) |
| Qwen Code | `~/.qwen/output-language.md` | a context file at every start | seen (finding 14) |
| Copilot CLI | size limit | none | seen: none up to 256 KB (finding 15) |
| Copilot CLI | an import of a file it also loads by name | counted twice | seen: sent twice (finding 16) |
| Copilot CLI | nested names | `AGENTS.md`, `CLAUDE.md`, when used | seen: neither at start, both after a read beside them (finding 18) |
| Copilot CLI | `IMPORT_DEPTH` | 5 | — |

### Task 2: The `loads` field on Doctor items (protocol; inline)

**Files:**
- Modify: `crates/protocol/src/doctor.rs`
- Generated: `src/generated/DoctorLoad.ts`, `DoctorLoadFile.ts`, `DoctorLoadWhen.ts`, `DoctorItem.ts`
- Modify: every `DoctorItem {` literal under `crates/` (grep)

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DoctorLoadWhen { Start, WhenUsed, NotLoaded }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorLoadFile {
    /// As the item's copy rules say: project-relative, `~/…`, or full.
    pub path: String,
    pub when: DoctorLoadWhen,
    /// The row's note, already worded ("about 600 tokens, imported by CLAUDE.md").
    pub note: String,
    /// 0 for a file the CLI finds itself; 1 for a file it imports, 2 for one that file imports, and so on.
    pub depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorLoad {
    pub agent: String,
    /// "Claude Code: about 2.4k tokens at start"
    pub heading: String,
    pub files: Vec<DoctorLoadFile>,
    /// Sentences shown under the heading ("Copilot CLI documents no limit.").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub notes: Vec<String>,
}
```

  `DoctorItem` gets `#[serde(default, skip_serializing_if = "Vec::is_empty")] #[ts(optional, as = "Option<Vec<DoctorLoad>>")] pub loads: Vec<DoctorLoad>`, after `action`.

- [ ] **Step 1:** Write the failing test `an_item_without_loads_serializes_without_the_key` in `doctor.rs`'s tests: the existing `item()` serializes with no `"loads"` key, and an item with one `DoctorLoad` round-trips.
- [ ] **Step 2:** Run `cargo test -p marshell-protocol doctor`. It should FAIL to compile, because there is no `loads` field yet.
- [ ] **Step 3:** Add the types and the field, and give every `DoctorItem {` literal `loads: Vec::new()`.
- [ ] **Step 4:** Run `cargo test -p marshell-protocol` and `pnpm check:generated`; both should pass. Then `cargo test -p marshell-core -j 4 doctor` should compile and pass.
- [ ] **Step 5:** Commit: `feat(protocol): a Doctor item can list the files each CLI loads.`

### Task 3: The engine: reader, imports, nested walk and items (core; worker at `effort: high`)

**Files:**
- Create: `crates/core/src/doctor/instructions.rs`
- Modify: `crates/core/src/doctor/mod.rs` (add `pub mod instructions;`), `crates/core/src/doctor/ext.rs` (`ExtRoots::under`), `crates/core/src/server/doctor_ext.rs` (`roots_under` calls it), `crates/core/src/doctor/checks.rs` (`ORDER` becomes `pub(crate)`)

**Interfaces:**
- Consumes: Task 2's protocol types; `ext::tokens`; `checks::display_name`; `checks::ORDER`; `references::project_id`; `config::hash::hash_bytes`; `server::local_file`; `config::preview::MAX_CONFIG_BYTES`.
- Produces:

```rust
// doctor/ext.rs
impl ExtRoots { pub fn under(home: &Path) -> ExtRoots }   // what `roots_under(home, None)` builds today

// doctor/instructions.rs
pub struct Place<'a> {
    pub roots: &'a ExtRoots,
    pub project: Option<&'a Path>,            // None: user level only
    pub ceiling: Option<&'a Path>,            // the walk up stops here, inclusive; None: the filesystem root
    pub claude_managed: Option<&'a Path>,     // Claude Code's managed folder; None reads none
    pub qwen_system_settings: Option<&'a Path>,
    pub distro: Option<&'a str>,
}
impl Place<'_> {
    /// `project` and each folder above it up to the ceiling, outermost first. Empty without a project.
    pub fn ancestors(&self) -> Vec<PathBuf>;
    /// The nearest folder at or above `project`, not past the ceiling, that holds one of `markers`.
    pub fn marked_root(&self, cx: &mut Reader, markers: &[&str]) -> Option<PathBuf>;
}
pub enum When { Start, WhenUsed, NotLoaded(String) }      // the String is the reason, worded for "not loaded: <reason>"
pub enum Role { File, Import { by: PathBuf }, Rule, Memory, Managed }
pub struct LoadedFile { pub path: PathBuf, pub role: Role, pub when: When, pub chars: usize, pub notes: Vec<String>, pub depth: u32 }
pub struct Cut { pub id_part: String, pub path: Option<PathBuf>, pub title: String, pub detail: String, pub cannot_fix: String }
pub struct CliLoad {
    pub agent: &'static str,
    pub distro: Option<String>,
    pub project: Option<PathBuf>,
    pub home: PathBuf,              // paths under it show as `~/…`
    pub files: Vec<LoadedFile>,
    pub reason: Option<String>,     // why no project file loads, for the title's brackets
    pub notes: Vec<String>,         // whole sentences, shown under the CLI's heading
    pub cuts: Vec<Cut>,
}
pub enum Text { Missing, Refused, TooLarge { bytes: u64 }, Unreadable, Text(Arc<str>) }
pub struct Reader { /* memo by canonical path; the guard; the walk cap; the deadline */ }
impl Reader {
    pub fn windows() -> Reader;                       // `server::local_file` guard, walk cap 10_000, deadline 10 s from now
    pub fn distro(unc_root: PathBuf) -> Reader;       // only paths under `unc_root`; names match exactly
    #[cfg(test)] pub fn with_walk_cap(cap: usize) -> Reader;
    pub fn read(&mut self, path: &Path) -> Text;      // lossy UTF-8; memoized
    pub fn is_dir(&mut self, path: &Path) -> bool;    // guarded
    pub fn same_file(&mut self, a: &Path, b: &Path) -> bool;       // canonical paths equal
    /// The files under `dir` whose names end in `suffix`, sorted by path; at most 1,000 files and 8 folders deep.
    pub fn list(&mut self, dir: &Path, suffix: &str, recursive: bool) -> Vec<PathBuf>;
    /// Every `.md` file below `root` (not in it), found once per root.
    pub fn nested(&mut self, root: &Path) -> &Nested;
    pub fn out_of_time(&self) -> bool;
}
pub struct Nested { pub files: Vec<PathBuf>, pub stopped: bool }
impl Nested { pub fn named(&self, names: &[&str], ignore_case: bool) -> Vec<PathBuf> }
pub enum ImportSyntax { At, AgyLink }       // `@path`, or `@[label](path)`
pub enum ImportBase { File, Root(PathBuf) }
pub struct ImportRules {
    pub syntax: ImportSyntax,
    pub depth: u32,
    pub base: ImportBase,
    pub inside: Option<PathBuf>,
    pub outside_reason: &'static str,       // "outside the project", "outside the repository"
    pub home: Option<PathBuf>,              // None: a `~/` import is refused with `outside_reason`
    pub linux_root: Option<PathBuf>,        // a distro's UNC root: `/x` resolves under it
    pub clean: fn(&str) -> String,          // what the CLI strips before counting (HTML comments, or nothing)
}
/// The file's own row plus each import as a row (depth-first, in text order), each import read once per call.
pub fn expand(cx: &mut Reader, file: &Path, text: &str, rules: &ImportRules, role_of_root: Role, when: When) -> Vec<LoadedFile>;
pub fn strip_html_comments(text: &str) -> String;     // block comments only, as Claude Code and Qwen Code do
pub fn frontmatter_value(text: &str, key: &str) -> Option<String>;   // Some("") for a key with a list under it
pub fn tokens_text(tokens: usize) -> String;          // "600", "2.4k"
pub fn hash_part(path: &Path) -> String;              // the 8 hex digits of a one-file cut's id
pub fn items(loads: &[CliLoad], projects: &[PathBuf]) -> Vec<DoctorItem>;   // empty `loads` gives no item
```

Rules for `expand`:
- **Where an import can be:** an `At` import is `@` at the start of a line or after whitespace, followed by a path that starts with `.`, `/`, `~` or a letter. It runs to the next whitespace, with `\ ` escaping a space. Nothing inside a fenced code block or an inline code span counts.
- **Resolving:** `~/` resolves under `rules.home`. A relative path resolves from the importing file's folder (`ImportBase::File`) or from the root (`ImportBase::Root`). An absolute path is used as it is, or under `rules.linux_root` when that is set.
- **Outside the allowed folder:** an import outside `rules.inside` (when Some), or a `~/` import with no `rules.home`, gets `NotLoaded(rules.outside_reason)`.
- **A network path** (`Text::Refused`): `NotLoaded("Marshell doesn't read network paths")`.
- **A missing target:** `NotLoaded("not found")`.
- **Depth:** past `rules.depth`, the import gets `NotLoaded("past the import limit")`.
- **Cycles:** a file already expanded in this call gets `NotLoaded("already loaded")`.
- **`chars`:** a file's `chars` are the characters of `rules.clean(text)`, for the root file and for each import.

Rules for `items`:
- **Grouping:** loads with the same `project` and `distro` form one line in `checks::ORDER`.
- **Start total:** a CLI's start total is the sum of `chars` over its `When::Start` files, through `tokens`.
- **Waste:** over 4,000 start tokens gives the Waste item.
- **Cuts:** each `Cut` gives one Broken item with the id `instrcut.<agent>.<project_id>.<id_part>`, and `path` set to the cut file's display path when it has one.
- **Copy:** all copy is as Global constraints say.
- **Order:** project lines come first, in `projects` order, then the Waste items, then the cuts.

- [ ] **Step 1: Write the failing tests** in `instructions.rs`'s `mod tests`, all under `tempfile::tempdir()`:
  - **The reader:**
    - `a_file_over_four_mib_is_too_large_with_its_size`: a 4 MiB + 1 byte file reads as `Text::TooLarge { bytes: 4_194_305 }`.
    - `non_utf8_text_is_counted_lossily`: UTF-16LE bytes and PNG bytes give `Text::Text` with no panic.
    - `the_reader_reads_a_file_once`: a second read of a deleted file still answers the first text.
    - `a_network_path_is_refused`: `//host/share/x.md` reads as `Text::Refused`. The guard answers from the path's text, before anything is opened.
    - `a_distro_reader_refuses_paths_outside_its_root`: with `Reader::distro(<tempdir>/root)`, a file in `<tempdir>/other` is `Text::Refused`.
    - `list_finds_nested_rule_files_sorted`: `rules/a.md` and `rules/x/b.md` come back in that order, and `rules/c.txt` doesn't.
  - **The place:**
    - `ancestors_stop_at_the_ceiling`: with the ceiling at `<tmp>` and the project at `<tmp>/a/b`, the list is `<tmp>`, `<tmp>/a`, `<tmp>/a/b`.
    - `marked_root_never_passes_the_ceiling`: a `.git` above the ceiling isn't found.
  - **Imports:**
    - `an_at_import_inside_code_is_not_an_import`: an import in a fence and one in backticks give no import rows.
    - `an_import_cycle_stops`: A→B→A gives rows A, B, then A as `NotLoaded("already loaded")`.
    - `the_depth_limit_holds`: a 6-deep chain with depth 4 gives 4 loaded imports, then one `NotLoaded("past the import limit")`.
    - `an_import_outside_inside_gets_the_cli_reason`: with `outside_reason: "outside the repository"`.
    - `a_home_import_without_a_home_is_refused`.
    - `a_missing_import_is_not_found`.
    - `a_linux_absolute_import_resolves_under_the_distro_root`.
    - `an_agy_link_include_is_read`: `@[x](sub/a.md)` is read with `ImportBase::File`.
    - `imports_are_cleaned_before_counting`: with `clean: strip_html_comments`, an import that is only a comment counts 0.
  - **Text helpers:**
    - `html_block_comments_are_stripped`: a `<!-- … -->` spanning lines is removed, and `<!-- x -->` inside a code fence is kept.
    - `frontmatter_values_are_found`: `---\npaths:\n  - src/**\n---` gives `Some("")` for `paths`; a file with no frontmatter gives None; `trigger: always_on` gives "always_on".
  - **The nested walk:**
    - `the_walk_honours_gitignore_and_skips_dot_git`: a `node_modules/AGENTS.md` that is gitignored, and `.git/AGENTS.md`, are not found; `docs/AGENTS.md` is. A second case has no `.git` folder at all, which pins `require_git(false)`.
    - `the_walk_stops_at_the_entry_cap`: with `with_walk_cap(5)` and 20 folders, `stopped` is true.
    - `named_filters_by_case_rule`: `agents.md` matches `AGENTS.md` with `ignore_case` and not without.
  - **Items:**
    - `tokens_text_rounds`: 999 → "999", 2,449 → "2.4k", 2,450 → "2.5k".
    - `no_loads_give_no_items`.
    - `items_make_one_line_per_project`: two CLIs in one project give one Info item, `instr.<project_id>`, with the exact title "proj: at start, Claude Code loads about 3 tokens of instructions, Codex none." and two `loads`.
    - `a_first_cli_with_nothing_loads_no_instructions`: "proj: at start, Claude Code loads no instructions, Codex 12."
    - `a_reason_shows_beside_a_total`: "… Codex 12 (Codex doesn't trust this folder)."
    - `over_4k_is_waste`: 16,004 chars at start gives `instrsize.claude.<id>` with the exact title, and the detail names the largest three.
    - `exactly_4k_is_not_waste`: 16,000 chars gives no Waste item.
    - `a_cut_is_broken_and_report_only`, with the id `instrcut.codex.<id>.budget` for an `id_part` of "budget".
    - `no_projects_gives_the_outside_line`.
    - `a_distro_line_says_user_instructions`, with a `~/…` path under the load's own `home`.
    - `notes_ride_under_the_heading`: a load's `notes` come out in `DoctorLoad::notes`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 doctor::instructions`. It should FAIL to compile.
- [ ] **Step 3: Implement.** The walk is `ignore::WalkBuilder::new(root).hidden(false).git_ignore(true).git_exclude(true).git_global(false).require_git(false).follow_links(false).max_depth(Some(12))`, plus a `filter_entry` that drops `.git`. Count entries against the cap, and stop when `out_of_time()`.
- [ ] **Step 4:** Run the same command; it should PASS. Then `cargo test -p marshell-core -j 4 doctor` (the `roots_under` change) and `cargo clippy -p marshell-core --all-targets -j 4 -- -D warnings` should be clean.
- [ ] **Step 5:** Commit: `feat(doctor): the engine that reads instruction files, follows imports and builds the instruction items.`

### Task 4: Claude Code's instruction files (core; worker at `effort: high`)

**Files:**
- Create: `crates/core/src/agents/claude/instructions.rs`, adding `pub mod instructions;` to `agents/claude/mod.rs`
- Modify: `crates/core/Cargo.toml` (`globset = "0.4.21"`), `THIRD_PARTY_NOTICES.md` (line 99 only, as Global constraints say)

**Interfaces:**
- Consumes: Task 3's engine; `projects::project_of`.
- Produces: `pub fn load(place: &Place, cx: &mut Reader) -> CliLoad`, and `pub fn memory_dir_name(repo_root: &Path) -> String`.

**Settings.** They are read through `cx.read`, never through `compact::read_settings`: the managed `<place.claude_managed>/managed-settings.json`, the project's `.claude/settings.local.json` and `.claude/settings.json`, and the user's `roots.claude_settings`. A file that is missing, too large or doesn't parse is skipped.

The rules (from the research note, §1, and Task 1's findings):
1. **Managed:** `<place.claude_managed>/CLAUDE.md` is `Role::Managed` and loads at start. Its row note adds "managed by your organisation". `claudeMdExcludes` doesn't apply to it.
2. **The mode:** it comes from `pluginConfigs["cc-plugin-agents-md@builtin"].options.instructionFiles` in the managed settings, then in the user settings (`roots.claude_settings`). Project and local files are ignored for this key. The default is `claude-md-or-agents-md`.
   - `managed-only`: every user, project and local file is `NotLoaded("Claude Code is set to load only managed instructions")`.
   - `claude-md`: AGENTS.md files are `NotLoaded("Claude Code is set to read CLAUDE.md only")`.
3. **User level:** `roots.claude_dir/CLAUDE.md` and `roots.claude_dir/rules/**/*.md`. A rule with `paths` loads when used; one without loads at start.
4. **Ancestors:** each folder of `place.ancestors()`, outermost first. In each, in this order: `CLAUDE.md`, `.claude/CLAUDE.md`, `CLAUDE.local.md`, then `AGENTS.md` and `.claude/AGENTS.md`.
   - `or-agents` mode: the AGENTS.md files load only when no `CLAUDE.md`, `.claude/CLAUDE.md` or `CLAUDE.local.md` exists in any of these folders. Otherwise each existing one is `NotLoaded("CLAUDE.md is here")`. The user's `~/.claude/CLAUDE.md` doesn't count.
   - `and-agents` mode: CLAUDE.md comes first in each folder, and an AGENTS.md already loaded is `NotLoaded("already loaded")`.
   - The same canonical file is counted once (`cx.same_file`), which covers the user home being one of the ancestors.
5. **Project rules:** `project/.claude/rules/**/*.md`, as in step 3.
6. **Nested:** `cx.nested(project).named(&["CLAUDE.md", "CLAUDE.local.md", "AGENTS.md"], …)`. Each file loads when used, and nested AGENTS.md follows the mode. A file that loads later (these, and rules with `paths`) and is over `LATER_CHARS` characters gets the note "over 40,000 characters: Claude Code loads its start and a note to read the rest".
7. **`claudeMdExcludes`:** the arrays from user, project and local settings are merged. They match the file's absolute path with forward slashes, through `globset` (`literal_separator(true)`, case-insensitive on Windows). A match is `NotLoaded("claudeMdExcludes leaves it out")`.
8. **Imports:** `ImportSyntax::At`, `IMPORT_DEPTH`, `ImportBase::File`, `home = roots.home`, `inside = None`, `clean = strip_html_comments`, and in a distro `linux_root` set. An import of a project file that lands outside `project` adds "Claude Code asks once before loading it" to its note, and still counts.
9. **HTML comments:** every file's chars are counted after `strip_html_comments`.
10. **Auto memory:**
    - Off when `autoMemoryEnabled` is false in the merged settings.
    - The folder is `autoMemoryDirectory` from the user settings when set. Otherwise it is `roots.claude_dir/projects/<memory_dir_name(repo root)>/memory`, where the repo root is `projects::project_of(project)`, so a worktree shares its main checkout's memory (Task 1 confirms both).
    - `MEMORY.md` there is `Role::Memory`, and its `chars` are the first `MEMORY_LINES` lines or `MEMORY_BYTES` bytes, whichever is shorter.
    - `memory_dir_name` replaces every character that isn't an ASCII letter or digit with `-` (`C:\dev\Airport` → `C--dev-Airport`).
    - With no project, there is no memory row.
11. **Cuts:** a file over 4 MiB (`Text::TooLarge`) gives a Cut with `id_part = hash_part(path)`. The file is `NotLoaded("over 4 MiB")` and counts nothing.
    - Title: "Claude Code skips <path> in <project>".
    - Detail: "It is over 4 MiB, so Claude Code loads none of it."
    - `cannot_fix`: "Shorten it, or split it into files that load when used."
    - A file over `WARN_LINES` lines gets "over <WARN_LINES> lines: Claude Code warns at start" in its note (no item).

- [ ] **Step 1: Write the failing tests** in the new file's `mod tests`. Homes and projects are under a tempdir, and roots come from `roots_under`.
  - `claude_md_and_local_load_ceiling_to_project`: rows for `<tmp>/CLAUDE.md`, then `<tmp>/proj/CLAUDE.md`, then `CLAUDE.local.md`, all at start, and nothing from above `<tmp>`.
  - `settings_come_from_the_roots`: with `roots.claude_settings` at `<tmp>/elsewhere/settings.json` holding the `claude-md` mode, AGENTS.md is "Claude Code is set to read CLAUDE.md only".
  - `the_managed_file_comes_only_from_the_place`: with `claude_managed: None` there is no managed row.
  - `agents_md_is_skipped_when_claude_md_is_here`.
  - `agents_md_loads_when_no_claude_md`.
  - `and_mode_loads_both_once`.
  - `managed_only_leaves_everything_else_out`.
  - `a_linked_claude_md_counts_once`: a symlink CLAUDE.md → AGENTS.md. If the symlink can't be made (no Developer Mode), the test returns early, with a comment.
  - `path_rules_load_when_used`.
  - `excludes_leave_a_file_out`: `claudeMdExcludes: ["**/docs/CLAUDE.md"]`.
  - `memory_counts_its_first_200_lines`.
  - `a_worktree_shares_its_repos_memory`: a project whose `.git` is a file pointing at `<tmp>/main/.git/worktrees/wt` reads `<claude_dir>/projects/<name of <tmp>/main>/memory/MEMORY.md`.
  - `auto_memory_directory_moves_the_folder`.
  - `a_later_file_over_40000_characters_gets_the_note`.
  - `memory_off_when_auto_memory_is_disabled`.
  - `a_file_over_4_mib_is_a_cut`, with the exact title.
  - `nested_files_load_when_used`.
  - `memory_dir_name_replaces_non_alphanumerics`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 claude::instructions`. It should FAIL.
- [ ] **Step 3: Implement.** Add `globset = "0.4.21"` and check that `Cargo.lock` gains no new package. Change `THIRD_PARTY_NOTICES.md` line 99 to the sentence in Global constraints.
- [ ] **Step 4:** Run the same command; it should PASS. Clippy clean.
- [ ] **Step 5:** Commit: `feat(doctor): Claude Code's instruction files, imports, rules and auto memory are read.`

### Task 5: Codex's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/codex/instructions.rs`
- Modify: `agents/codex/extensions.rs` (make `trust_key` `pub(crate)`), `agents/codex/mod.rs`

**Interfaces:** `pub fn load(place: &Place, cx: &mut Reader) -> CliLoad`, and `fn untrusted(user: Option<&DocumentMut>, project: &Path) -> bool`.

The rules:
1. **User level:** `roots.codex_home/AGENTS.override.md` when it is non-empty, else `AGENTS.md`. It loads at start and isn't counted against the budget. When the override wins and `AGENTS.md` exists, `AGENTS.md` is `NotLoaded("AGENTS.override.md is here")`.
2. **`config.toml`:** read `project_doc_max_bytes` (default 32,768), `project_doc_fallback_filenames` (a fallback whose name has a `/` or `\` is ignored), and `project_root_markers` (default `[".git"]`).
3. **Trust:** `untrusted` is true only when the entry that decides for the folder says `trust_level = "untrusted"`. The deciding entry is found the way `extensions.rs::trusted` finds it: the folder's own `[projects.'<trust_key>']`, else its git root's. A folder with no entry loads its files (finding 1). When untrusted, every project file is `NotLoaded("Codex doesn't trust this folder")` and the load's `reason` is "Codex doesn't trust this folder".
4. **A zero budget:** `project_doc_max_bytes = 0` makes every project file `NotLoaded("project_doc_max_bytes is 0")`, with the `reason` "project_doc_max_bytes is 0".
5. **The project root:** `place.marked_root(cx, markers)`. With none, or with an empty marker list, it is `project` itself.
6. **Per folder,** from the root down to `project`: the first non-empty file of `AGENTS.override.md`, `AGENTS.md`, then the fallbacks, loads. The others that exist are `NotLoaded("<winner> is here")`.
7. **The budget:** bytes are spent from the root down.
   - The file that crosses the budget keeps its first remaining bytes. Its `chars` count those bytes, decoded lossily, and its note gets "cut: Codex loads <kept> of <size> bytes".
   - Every later file is `NotLoaded("past project_doc_max_bytes")`.
   - A file over 4 MiB counts by its bytes, as Global constraints say.
   - One Cut item per project, with `id_part = "budget"` and no `path`:
     - Title: "Codex cuts its project instructions in <project>".
     - Detail: "It loads <budget> bytes of project instructions, and these are over: <path> (<lost> bytes cut), <path> (not loaded)."
     - `cannot_fix`: "Set project_doc_max_bytes in ~/.codex/config.toml to raise the limit, or shorten the files." The `~/.codex` part is the display form of `roots.codex_home`.
8. **Nested:** `AGENTS.override.md`, `AGENTS.md` and the fallbacks that end in `.md`, through `cx.nested(project).named(…)`, load when used. No imports.

- [ ] **Step 1: Write the failing tests:**
  - `user_override_beats_agents_md`;
  - `one_file_per_folder_root_to_project`;
  - `fallback_names_are_followed`;
  - `a_fallback_with_a_path_is_ignored`;
  - `an_untrusted_folder_loads_no_project_file_and_still_its_user_file`;
  - `a_folder_with_no_trust_entry_loads_its_files`;
  - `zero_budget_loads_none`;
  - `the_root_search_stops_at_the_ceiling`;
  - `the_crossing_file_is_cut_and_later_ones_skipped`, with a 30,000-byte root file and a 5,000-byte sub file, giving the exact Cut title and detail numbers;
  - `no_marker_means_only_the_project_folder`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 codex::instructions`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): Codex's AGENTS.md chain, its fallbacks, trust and its 32 KiB budget are read.`

### Task 6: Antigravity's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/antigravity/instructions.rs`, adding the line to `mod.rs`

**Interfaces:** `pub fn load(place: &Place, cx: &mut Reader) -> CliLoad`.

The rules (Task 1's findings win where they differ):
1. **User level:** `gemini_dir/GEMINI.md`, `gemini_dir/AGENTS.md`, `gemini_dir/config/GEMINI.md`, `gemini_dir/config/AGENTS.md` and `gemini_dir/config/rules/*.md`.
2. **Project:** from the repo root (`place.marked_root(cx, &[".git"])`, else `project`) down to `project`, take `GEMINI.md` and `AGENTS.md` in each folder. Then the rules in `project/{.agents,.agent,_agents,_agent}/rules/*.md`.
3. **Rules,** by `trigger`:
   - `always_on`: at start.
   - `glob`: when used.
   - `model_decision`: when used. The note adds "its description loads at start", and the description's characters count at start.
   - `manual`: when used, with the note "only when you mention it".
   - Any other value, or none: per Task 1's finding.
4. **Includes:** `ImportSyntax::AgyLink`, with the base from Task 1 and depth 5 (unknown; the limit only guards against loops).
5. **Cuts.** A file whose text, includes expanded, is over `RULE_BYTES` keeps its text up to the last line break before the limit. The Cut's `id_part` is `hash_part(path)`.
   - Title: "Antigravity cuts <path> in <project>".
   - Detail: "It loads the first 24,000 bytes of each rule, so <lost> bytes of it never reach the agent."
   - `cannot_fix`: "Shorten it, or split it into rules."
6. **The budget:** when the always-on rules and the user rules together pass `RULES_BUDGET_TOKENS` (counted in load order), the rest are `NotLoaded("past Antigravity's 20,000-token budget, sent as a path")` and one Cut follows, with `id_part = "budget"` and no `path`.
   - Title: "Antigravity sends some rules as paths only in <project>".
   - Detail: "Its always-on and user rules are over 20,000 tokens, so these reach the agent as paths: <paths>."
   - `cannot_fix`: "Give the rules it needs only sometimes trigger: model_decision or glob."
7. **Nested** `GEMINI.md` and `AGENTS.md` below `project`, through `cx.nested(project).named(…)`, are `NotLoaded("Antigravity loads it only in a session started in its folder")`. Finding 8: they loaded only that way, never after a file beside them was read.
8. **Notes:** whatever Task 1 left unknown about Antigravity is one sentence in the load's `notes`, worded from the finding.

- [ ] **Step 1: Write the failing tests:**
  - `user_and_config_files_load`;
  - `rules_follow_their_trigger`;
  - `model_decision_counts_its_description`;
  - `an_include_is_read_from_its_base`;
  - `a_rule_over_24000_bytes_is_cut_on_a_line_break`;
  - `past_the_budget_rules_become_paths`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 antigravity::instructions`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): Antigravity's GEMINI.md, AGENTS.md and rules are read, with its byte cut and rules budget.`

### Task 7: Qwen Code's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/qwen/instructions.rs`
- Modify: `agents/qwen/extensions.rs`, `agents/qwen/mod.rs`

**Interfaces:**
- `pub fn load(place: &Place, cx: &mut Reader) -> CliLoad`, and `pub fn memory_dir_name(project: &Path) -> String`.
- In `extensions.rs`: `folder_trusted` becomes `pub(crate)`, and the inline `security.folderTrust.enabled` expression inside `inventory` (~`:441`) moves into `pub(crate) fn folder_trust_on(…) -> bool` with the inputs it reads today. `inventory` calls it, so 7c's behaviour doesn't change.

The rules:
1. **Settings:** the system file (`place.qwen_system_settings`), the project's `.qwen/settings.json` (only when the folder is trusted) and the user's `qwen_dir/settings.json`, merged in the order `extensions.rs` already uses for them. All are read through `cx.read`.
2. **Names:** `context.fileName`, a string or an array, replaces `["QWEN.md", "AGENTS.md"]`. When it is set and leaves out `AGENTS.md`, an existing `AGENTS.md` is `NotLoaded("context.fileName doesn't name it")`. A project-level value is followed (finding 11).
3. **User level:** `qwen_dir/<each name>`, and `qwen_dir/rules/**/*.md` through `cx.list`. A rule with `paths` loads when used, one without at start, and HTML comments are stripped. `qwen_dir/output-language.md`, which Qwen Code writes itself, loads at start when it exists (finding 14).
4. **Trust:** when `folder_trust_on` and the folder isn't trusted, the project files are `NotLoaded("Qwen Code doesn't trust this folder")`, with that `reason`. A folder no rule names isn't trusted (finding 13).
5. **The walk:** from the project root down to `project`, each name in each folder.
   - The root is `place.marked_root(cx, &[".git"])`.
   - With no `.git`: when `project` is under the user home, the walk starts just below the home; otherwise it starts at the ceiling (the filesystem root in production).
   - When `project` is the user home itself, only the user-level files load.
   - `.qwen/QWEN.local.md` at the git root loads only with a git root. `project/.qwen/rules/**/*.md` follow rule 3.
6. **Imports:** `ImportSyntax::At`, depth 5, `ImportBase::File`, `inside = Some(project root)`, `outside_reason = "outside the project"`, `home = None`, `clean = strip_html_comments`. A missing import keeps its literal text, which is already in the file's chars, and its row is `NotLoaded("not found")`.
7. **No limits, so no cuts,** and the load's `notes` hold "Qwen Code documents no limit." Nothing below `project` loads (path rules are covered above).
8. **Auto memory** (finding 14), each file `Role::Memory`, at start, counted up to its first `MEMORY_LINES` (200) lines:
   - `qwen_dir/memories/MEMORY.md`, at user level;
   - with a project, `qwen_dir/projects/<memory_dir_name(project)>/memory/MEMORY.md`.
   - `memory_dir_name` is the project's own path (a worktree has its own folder, unlike Claude Code), lowercased, with every character that isn't an ASCII letter or digit as `-`: `C:\dev\Airport` gives `c--dev-airport`.
   - A file that is missing or empty gives no row.

- [ ] **Step 1: Write the failing tests:**
  - `default_names_load_root_to_project`;
  - `file_name_setting_replaces_the_list`;
  - `an_untrusted_folder_loads_no_project_file_and_still_its_user_file`;
  - `a_folder_with_no_rule_loads_no_project_file`;
  - `local_file_needs_a_git_root`;
  - `without_git_the_walk_starts_below_the_home`;
  - `an_import_outside_the_root_is_refused`;
  - `rules_strip_html_comments`;
  - `the_no_limit_note_is_there`;
  - `the_output_language_file_loads_at_start`;
  - `memory_counts_both_indexes_up_to_200_lines`: a 250-line user index and a 250-line project index each count 200 lines' characters;
  - `memory_dir_name_lowercases_and_dashes`: `C:\dev\Airport` gives `c--dev-airport`;
  - `an_empty_memory_index_gives_no_row`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 qwen::`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS, with 7c's Qwen tests still green.
- [ ] **Step 5:** Commit: `feat(doctor): Qwen Code's context files, rules, imports and folder trust are read.`

### Task 8: Copilot CLI's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/copilot/instructions.rs`, adding the line to `mod.rs`

**Interfaces:** `pub fn load(place: &Place, cx: &mut Reader) -> CliLoad`.

The rules:
1. **User level:** `copilot_dir/copilot-instructions.md`, and `copilot_dir/instructions/**/*.instructions.md`. A file with `applyTo` loads when used, one without at start.
2. **The repo:** the git root R is `place.marked_root(cx, &[".git"])`, else `project`. For each folder from R down to `project`: `AGENTS.md`, `CLAUDE.md`, `.claude/CLAUDE.md` and `GEMINI.md` load at start.
   - At R: `.github/copilot-instructions.md` (start), `.github/instructions/**/*.instructions.md` (`applyTo` decides), and `.claude/rules/**/*.md` (with `paths`, when used; without, at start).
3. **Imports:** `ImportSyntax::At`, `ImportBase::File`, depth `IMPORT_DEPTH`, `inside = Some(R)`, `outside_reason = "outside the repository"`, `home = None` (so a `~/` or absolute import isn't loaded), and `clean` leaves the text as it is. Imports apply only in `copilot-instructions.md`, `AGENTS.md` and `CLAUDE.md`. In `GEMINI.md` and `*.instructions.md` an `@` is plain text.
4. **Duplicates:** a file whose bytes equal a file already loaded at start is `NotLoaded("same text as <path>")`. An import that repeats a file Copilot also loads by name counts twice: that is what Task 1's step 3 saw, or the finding there amends this.
5. **No size limit:** none was seen up to 256 KB (finding 15). So there are no cuts, and the load's `notes` hold "Copilot CLI documents no limit; none was seen up to 256 KB."
6. **Nested** `AGENTS.md` and `CLAUDE.md` below `project`: when used (finding 18).

- [ ] **Step 1: Write the failing tests:**
  - `repo_files_load_from_the_git_root_to_the_project`;
  - `claude_rules_with_paths_load_when_used`;
  - `imports_only_in_three_file_kinds`;
  - `a_home_import_is_not_loaded`;
  - `identical_files_count_once`;
  - `user_instructions_follow_apply_to`;
  - `the_no_limit_note_is_there`;
  - `nested_agents_and_claude_files_load_when_used`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 copilot::instructions`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): Copilot CLI's instruction files, the other CLIs' files it reads, and its imports are read.`

### Task 9: The run reads the loads (core server)

**Files:**
- Create: `crates/core/src/server/doctor_instr.rs`, with `mod doctor_instr;` in `server/mod.rs`
- Modify: `crates/core/src/server/doctor.rs` (`Gathered`, `facts_of`, `run_once`, and the `Gathered { … }` literal in its tests)
- Test: `crates/core/src/server/doctor.rs`'s tests and `crates/core/tests/doctor_api.rs`

**Interfaces:**
- Consumes: Tasks 3 to 8.
- Produces:
  - `Gathered::distro_homes: Vec<(String, PathBuf)>`, which `facts_of` fills from what `distro_facts` already answers.
  - `pub(crate) fn loads(st: &AppState, facts: &[AgentFacts], distro_homes: &[(String, PathBuf)], projects: &[PathBuf]) -> Vec<CliLoad>`.

The rules:
- **Only the run reads them.** `run_once` calls `loads` in a second `spawn_blocking`, after the facts read, and then `instructions::items(&loads, &facts.projects)` after `ext::items`. `facts_of` doesn't call it, so a fix preview, Keep and Measure read no instruction file. If that read fails, the report still comes, without instruction items, and a warning is logged.
- **Windows CLIs:** each CLI with `found && error.is_none()` is read once per project, with `windows_roots(st)` and one `Reader::windows()` shared across all of them. With no projects, each is read once with `project = None`. With no roots (a unit test that names no home), nothing is read.
- **The place:**
  - In production the ceiling is None, `claude_managed` is the folder of `compact::managed_settings_paths()[0]`, and `qwen_system_settings` is `st.qwen_system_settings_path()`.
  - With `ext_home` pinned (tests), the ceiling is the project's own folder, `claude_managed` comes only from `claude_compact_paths` (else None), and `qwen_system_settings` is the pinned one or None.
- **A project that can't be read:** when `cx.is_dir(project)` is false, each CLI is read with `project = None`, keeps that project on its `CliLoad`, and gets the `reason` "Marshell can't read this folder" and the note "Marshell can't read <path>."
- **Out of time:** once `cx.out_of_time()`, each remaining project gets, for every CLI, a `CliLoad` with no files and the `reason` "not checked: the check ran out of time".
- **Distros:** each running managed distro (`distro_homes`) is read at user level (`project = None`), with `ExtRoots::under(home)` and `Reader::distro(<the home's UNC server and share>)`. A home that isn't a UNC path is skipped.

- [ ] **Step 1: Write the failing tests:**
  - `the_report_has_a_line_per_project` (server tests, with a pinned `ext_home` and two projects in `projects.json`): two `instr.` items, each with `loads`.
  - `a_missing_project_gives_a_line_and_no_failure`: the item's title holds "(Marshell can't read this folder)", and a user-level `.claude/CLAUDE.md` under the pinned home still counts.
  - `a_fix_preview_reads_no_instruction_file`: with instruction files planted and a seeded fixable 7a item, `fix_of` answers its plan, and a `Reader` read counter that the test state exposes stays at 0 across the call.
  - `a_report_without_projects_or_roots_has_no_instruction_item`: an existing-style state with no pinned home.
  - In `tests/doctor_api.rs`, `the_instruction_line_rides_in_the_report`: `GET /v1/doctor/report` after a run holds an `instr.` item whose `loads[0].files[0].path` is `"CLAUDE.md"`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 doctor`. It should FAIL.
- [ ] **Step 3:** Implement. The read counter is a `#[cfg(test)]` `AtomicUsize` on the Doctor's state that `loads` bumps once per call.
- [ ] **Step 4:** Run `cargo test -p marshell-core -j 4 doctor` and `cargo test -p marshell-core -j 4 --test doctor_api`; both should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): each run reads every project's instruction files for every installed CLI.`

### Task 10: The expansion on the Doctor row (frontend)

**Files:**
- Modify: `src/features/doctor/DoctorRow.tsx`, `src/features/doctor/Doctor.css`, `src/features/doctor/fixtures.ts` (one fixture item with `loads`)
- Test: `src/features/doctor/DoctorPage.test.tsx`, `src/features/doctor/Doctor.css.test.ts`

Read the `good-css` skill before writing the declarations. The look follows the row's existing `doctor__file` and `doctor__why` text, so `frontend-design` decides nothing new.

The markup, after `doctor__file`:
- The row's title element gets an id from React's `useId`.
- `<details className="doctor__loads"><summary aria-describedby={titleId}>Show the files</summary>`, so thirty summaries with the same words are told apart by a screen reader.
- Then, for each load, `<p className="doctor__load-cli">{heading}</p>`, each of its `notes` as `<p className="doctor__load-note">`, and `<ul className="doctor__load-files">`.
- Each file is `<li data-when={when} style={{ "--depth": depth }}>`, holding `<code dir="ltr">{path}</code>` and `<span>{note}</span>`.

The styling:
- Rows with `whenUsed` or `notLoaded` use the muted text colour the row already uses for `doctor__why`. The note's words say the state, so colour never carries it alone.
- The indent is `calc(var(--depth) * 1.25em)` on `padding-inline-start`.
- `summary` keeps the browser's own marker (no `list-style: none`, no `display` change), and it is at least 24 px tall.
- `.doctor__loads summary:focus-visible` gets the same ring the row's buttons have (`Doctor.css` ~183 to 190): a 2 px outline with a 2 px offset.
- In forced colours, `summary` uses `CanvasText` and its focus ring is `Highlight`.

- [ ] **Step 1: Write the failing tests:**
  - `a_row_with_loads_has_a_closed_disclosure`: the static markup has `<details class="doctor__loads">` without `open`, the summary text, both headings, and a row whose text includes "not loaded: CLAUDE.md is here".
  - `a_row_without_loads_has_no_disclosure`.
  - `the_summary_is_described_by_its_rows_title`: the summary's `aria-describedby` equals the title element's id.
  - `a_loads_notes_show_under_its_heading`.
  - `Doctor.css.test.ts`: the forced-colours block names `.doctor__loads summary`; a `.doctor__loads summary:focus-visible` rule sets an outline; no rule for `.doctor__loads summary` sets `list-style` or `display`.
- [ ] **Step 2:** Run `pnpm test src/features/doctor`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `pnpm test src/features/doctor` and `pnpm build`; both should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): an instruction line opens to the files each CLI loads.`

### Task 11: Live check and exit (main thread)

Use the `marshell-live-check` skill. `pnpm tauri dev` from this branch runs against a scratch `MARSHELL_HOME` and scratch CLI homes in a new, literal scratch folder the skill's `start` makes, with three scratch projects in its `projects.json`. Sign-ins follow Task 1's rules. The dev window is never brought to the front, and the webview is driven over CDP with `browser_evaluate`.

Exit criteria:
1. Project A has CLAUDE.md importing `@AGENTS.md`, a path rule, and a nested `docs/AGENTS.md`.
   - Its line names all five CLIs.
   - The expansion shows the import at depth 1, the rule and the nested file as "loads when used", and Codex's AGENTS.md.
   - Claude Code's `/context` in a session there (one prompt) lists the same start files, and its memory-file token figure is within 15% of the estimate.
2. Project B has a 40,000-byte AGENTS.md. Codex's cut item reads exactly as Task 5 says, and Codex's own warning in a session there names the same byte counts.
3. Project C is marked untrusted for Codex and Qwen Code, and the scratch homes hold a user-level `AGENTS.md` for each. Their part of the line reads "<T> (Codex doesn't trust this folder)" and "<T> (Qwen Code doesn't trust this folder)", with the user file counted and the project files "not loaded".
4. Over 4k: Project B's Codex Waste item names AGENTS.md first.
5. Keyboard: Tab reaches "Show the files", Enter and Space open and close it, and the focus ring shows in the normal theme. The summary's accessible description is its row's title. Forced colours (emulated with `browser_emulate_media`) keep the marker and the ring.
6. A running WSL distro with a `~/.claude/CLAUDE.md` gets the distro line, with the file shown as `~/.claude/CLAUDE.md`. A stopped one keeps 7a's "Not checked" line.
7. With the Doctor page open, "Fix X issues" on a seeded 7a item opens its sheet as fast as on `origin/main`: the log shows no instruction read between the click and the preview.

Record everything in `docs/research/2026-11-24-instruction-files-live-check.md`. Then come the whole-branch review (`marshell-reviewer` on Opus), one fix pass, and `marshell-ship`.
