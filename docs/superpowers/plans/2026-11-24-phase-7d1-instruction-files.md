# Slice 7d1: what each CLI's instruction files load

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor shows, for each project in `projects.json` and for each installed CLI, which instruction files load at session start, about how many tokens they cost, which load later, which don't load and why, and which text a CLI cuts or skips. That last kind is Broken. More than 4k tokens at start is Waste. Every item is report-only.

**Architecture:** There are three layers, as in 7c.
- **The engine** (`crates/core/src/doctor/instructions.rs`, new) knows no CLI's rules. It has:
  - a bounded, memoized file reader;
  - the import expander for the import syntaxes;
  - the nested-file walk shared by every CLI;
  - the pure `items()` that turns each CLI's `CliLoad` into report items.
- **Each CLI's rules** live in a new `agents/<cli>/instructions.rs`, with `pub fn load(roots: &ExtRoots, project: Option<&Path>, cx: &mut Reader) -> CliLoad`.
- **The server** (`server/doctor_instr.rs`, new) calls the five readers for each project, and for each running managed distro at user level. It hands the loads to `run_once` through `Gathered`.

The protocol gains one optional field on `DoctorItem`, `loads`. The Doctor row renders it as a native `<details>` disclosure.

**Tech Stack:** Rust (serde_json, toml_edit, `ignore` 0.4's `WalkBuilder`, `globset` as a new direct dependency, see Global constraints), React + TypeScript, and Vitest through `renderToStaticMarkup`. The protocol changes once (Task 2), so `src/generated/` is regenerated once and `pnpm check:generated` runs.

**Spec:** `docs/PLAN.md` §9, "Slice 7d decisions (2026-11-24)", 1 to 11. They rest on "Slice 7a decisions" 2, 3 and 7, and on "Slice 7c decisions" 7 and 10. The evidence is `docs/research/2026-11-24-instruction-files.md`. Task 1's section there ("What 7d1's first task saw") is what the CLIs really did, and it overrides the static read above it.

**Checked before planning (2026-11-24, on `origin/main` 5a21847d):**
- No code reads instruction files. `project_doc_max_bytes` appears only as a settings-view key (`agents/settings_view.rs:38`).
- The run is `server/doctor.rs::facts_of` (blocking, ~280), which fills `Gathered`, then `run_once` (~225): `checks::items` + `ext::items` + the CLI doctors' items. `ext::items` takes `ext::Signals { projects, … }`. `Gathered.projects` is `projects::list(home)` (at most 30, `projects.rs:11`), and it is filled only when `doctor_ext::windows_roots(st)` is Some. It is None in a unit test that names no home, so no test reads the developer's files.
- `doctor::ext::ExtRoots` (`doctor/ext.rs:15`) already has `claude_dir`, `claude_settings`, `codex_home`, `qwen_dir`, `gemini_dir`, `copilot_dir` and `home`. `doctor_ext::roots_under(home, None)` builds a distro's roots from its UNC home.
- `ext::tokens(chars) = chars.div_ceil(4)` (`doctor/ext.rs:463`). The 7c skills line reads "<CLI>'s skills add about N tokens to each session".
- Trust readers exist:
  - Codex: `agents/codex/extensions.rs:320` `fn trusted(user: Option<&DocumentMut>, project: &Path) -> bool`, private.
  - Qwen Code: `agents/qwen/extensions.rs:255` `fn folder_trusted(rules: &Option<Value>, project: &Path) -> bool`, private. Whether `security.folderTrust.enabled` is on is read near `:120`.
- Claude Code's settings reader: `agents/claude/compact.rs:27` `read_settings(user_home, project_dir, managed) -> Vec<Value>`, highest precedence first. It reads through `server::local_file`, which is `files::existing_file`: it refuses network paths and links that lead to them, and returns the canonical path. Managed paths come from `compact.rs::managed_settings_paths()`, whose directory is `…\ClaudeCode`.
- Size caps:
  - `config::preview::MAX_CONFIG_BYTES` is 4 MiB.
  - `server/ignores.rs:33` `read_capped(path, cap) -> Option<Vec<u8>>` returns None for a file that is missing, not regular, or over the cap.
  - Issue #192 asks that every fact reader be capped. The new reader is capped from the start.
- `ignore` 0.4 is a direct dependency of `marshell-core`, and `globset` is already in `Cargo.lock` through it.
- The Doctor row (`src/features/doctor/DoctorRow.tsx`) renders the title, `detail`, `path` and `cannot_fix`. `Doctor.css:317` has a `forced-colors` block. The project switches `good-css` and `frontend-design` on (`.claude/settings.json:75`).
- `references::project_id(root)` (`references.rs:37`) gives a stable 16-hex id per project root.

## Global constraints

- **Read-only slice.**
  - Nothing in 7d1 writes a CLI file, a project file or a Marshell file. No `EditOp`, no fix arm, no `managed.json` change.
  - Every new item is `fixable: false`. A Broken item here is never "pre-checked", because it has no checkbox.
- **Reads:**
  - Every instruction-file read goes through `doctor::instructions::Reader` and is capped at `MAX_CONFIG_BYTES + 1`. A file over 4 MiB is "too large" and its text is never parsed.
  - On Windows, every path goes through `server::local_file` first, so a link to a network path is never followed. A distro's files are read under its UNC home with the same cap.
  - Reads run inside `facts_of`, which is already in `spawn_blocking`.
  - A file is read once per run, whichever CLIs or projects name it (the reader memoizes by canonical path).
- **The nested walk:**
  - Shared by every CLI, one per project per run.
  - It uses `ignore::WalkBuilder` with git's ignore rules on, hidden files on, no link following and no `.git` folder.
  - It stops at 10,000 entries or depth 12, whichever comes first. Then the project's line gets the note "Marshell stopped looking for files in subfolders after 10,000 entries."
- **Items never quote a file's content.** They give names, paths, sizes and counts only.
- **Token figures:**
  - Every figure is `ext::tokens(chars)`, which is characters divided by 4, rounded up.
  - From 1,000 up it reads as `N.Nk`, with one decimal, rounded half up (2,449 → "2.4k", 2,450 → "2.5k"). Below 1,000 it is the plain number.
  - Text says "about".
  - The Waste threshold is more than 4,000 tokens at start, for one CLI in one project.
- **Ids:**
  - `instr.<project_id>` for a project line, `instr.user` when `projects.json` is empty, and `instr.user@<distro>` in a distro.
  - `instrsize.<agent>.<project_id>` (or `.user`, plus `@<distro>`) for the Waste item.
  - `instrcut.<agent>.<project_id>.<n>` for each Broken cut, where `<n>` is the first 8 hex digits of `config::hash::hash_bytes` of the cut file's path text. A distro adds `@<distro>`.
- **Paths in the expansion:**
  - Project files are relative to the project root, with forward slashes (`CLAUDE.md`, `docs/AGENTS.md`).
  - User files start with `~/` when under the user home.
  - Anything else is a full path.
- **Each CLI's rules stay in `agents/<cli>/instructions.rs`.** `doctor/instructions.rs` names no CLI except in `display_name` calls, and no CLI module reads another CLI's settings.
- **Copy, exactly:**
  - **Project line:**
    - Title: "<project folder name>: at start, <CLI> loads about <T> tokens of instructions, <CLI> <T>, … ." The CLIs come in `checks::ORDER`, installed ones only.
    - A CLI with nothing at start reads "<CLI> none". A CLI whose files are all out reads "<CLI> none (<reason>)", where the reason is the CLI's note, such as "Codex doesn't trust this folder".
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
      - "managed by your organisation".
  - The cut items' copy is in Tasks 4 to 8 and is exact.
- **New dependency:** `globset` (the version already in `Cargo.lock`) goes into `crates/core/Cargo.toml`.
  - The need: Claude Code's `claudeMdExcludes` holds picomatch-style globs on absolute paths, and no crate in the tree matches a glob against an absolute path except through `ignore`'s root-relative builders.
  - It is BurntSushi's crate, actively maintained, MIT/Unlicense, with no network access.
  - It already ships inside the app through `ignore`, so `THIRD_PARTY_NOTICES.md` needs no new entry. Task 4 checks this.
- **No change to `guard.rs`,** token handling or file permissions.
- **Tests never read the real homes.** Every test builds its homes and projects under `tempfile::tempdir()` and passes an `ExtRoots` built with `roots_under`. No test calls `windows_roots` without `ext_home` pinned.
- **Commands:**
  - Rust: `cargo test -p marshell-core -j 4 <filter>`, with `cargo fmt --all` before every Rust commit.
  - Protocol: `cargo test -p marshell-protocol` and `pnpm check:generated`.
  - Frontend: `pnpm test src/features/doctor`.
- **Struct literals:** a new field on `DoctorItem` breaks every literal. Grep `DoctorItem {` under `crates/` and give each `loads: Vec::new()`. `DoctorItem` has no `Default`; adding one is fine if the task wants it.
- **Briefs to workers:**
  - Never run `git checkout`, `git restore`, `git stash` or `git clean` on files they didn't create.
  - Scratch goes under a new folder in `%TEMP%` that they made, and they delete nothing else.

## Review focus

1. **A CLAUDE.md that is a link to AGENTS.md** (common), or an AGENTS.md that CLAUDE.md also imports: the reader memoizes by canonical path, so Claude Code counts it once. Copilot CLI reads both names, so they appear as two rows: the second "not loaded: same text as <path>" when the bytes are identical, otherwise both counted. Tests: Task 4 `a_linked_claude_md_counts_once`, and Task 8 `identical_files_count_once`.
2. **An import cycle or a self-import** (`@CLAUDE.md` inside CLAUDE.md, or A imports B imports A) never loops. Each file is expanded once per CLI load, and the depth limit holds. Test: Task 3 `an_import_cycle_stops`.
3. **A project folder that no longer exists, is a file, is on a network path, or can't be read:** the line still comes, with "none" for every CLI and the note "Marshell can't read <path>.", and the run doesn't fail. Test: Task 9 `a_missing_project_gives_a_line_and_no_failure`.
4. **A huge checkout** (node_modules, target, a monorepo): the walk honours `.gitignore`, skips `.git` and stops at 10,000 entries. A run over 30 such projects stays bounded. Test: Task 3 `the_walk_stops_at_the_entry_cap`, which uses a cap passed in, not 10,000 real files.
5. **Non-UTF-8 or binary text in an instruction file** (a UTF-16 file saved by Notepad, or a stray PNG named AGENTS.md): characters are counted lossily, so nothing panics and nothing is quoted. Test: Task 3 `non_utf8_text_is_counted_lossily`.

## Known limits (planned)

- **Environment switches aren't read:** Claude Code's `CLAUDE_CODE_DISABLE_CLAUDE_MDS` and `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD`, Copilot CLI's `COPILOT_CUSTOM_INSTRUCTIONS_DIRS`, and the CLIs' `--bare`, `--safe-mode`, `--no-custom-instructions` and `--setting-sources` flags. They depend on how a session is started, not on files.
- **Only the start folder:** the estimate is for a session started at the project's root. A session started in a subfolder loads more with Codex, Copilot CLI and Claude Code (7d decision 6).
- **Plugin instruction files** (Antigravity's `plugins/<name>/rules/`, Qwen Code's extension context files) aren't read. Phase 8's Extensions covers plugins (7c decision 6).
- **Claude Code's `--add-dir` folders** aren't checked.
- **What 7d2 adds:** missing imports, stale paths, duplicates and clashes. Until then a missing import shows only as a "not loaded: not found" row.

---

### Task 1: What the CLIs really do (main thread, before any code)

Decision 10. It runs in a scratch home, with the `marshell-live-check` skill's rules for homes, PIDs and the desktop. It needs a few short prompts in real sessions, started in Windows Terminal or Marshell's dev app from `origin/main`. The scratch folder is literal and new: `C:\lc-7d1\`. It holds `home\` (with `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME` and `USERPROFILE` pointed into it, and copies of only the login files each CLI needs) and `proj\` (a `git init` folder).

- [ ] **Step 1: Claude Code.**
  - Plant a `CLAUDE.md` of 199 lines and one of 201 lines (two runs), and read the start-up warning text and `/status`.
  - Plant a chain of imports 6 deep, each file holding a unique marker, then ask "Which MARKER-n lines do you see in your instructions? Answer with the numbers only." This gives the depth.
  - Plant `@missing.md` and look for any message about it.
  - Note the auto memory folder's name for `C:\lc-7d1\proj` under `<CLAUDE_CONFIG_DIR>\projects\`.
  - Check whether `25 KB` is 25,000 or 25,600 bytes, by planting a `MEMORY.md` with markers around both offsets.
- [ ] **Step 2: Antigravity.**
  - Plant `.agents/rules/a.md` (`trigger: always_on`) holding an `@[x](sub/inc.md)` include, plus `sub/inc.md` and `inc.md` with different markers. Ask which marker it sees: this gives the include's base (the including file's folder, or the project root).
  - Point an include at a missing file and look for any message.
  - Plant `sub/GEMINI.md` with a marker and ask before and after a prompt that reads `sub/x.txt`. This shows whether nested files load at start or when used.
  - Plant a rule with no `trigger:` and ask whether its marker is seen.
  - Note which of `~/.gemini/GEMINI.md` and `~/.gemini/config/GEMINI.md` load when both exist.
- [ ] **Step 3: Copilot CLI.**
  - Plant a 64 KB `AGENTS.md` with markers every 8 KB, and ask for the last marker it sees. If none is cut, a limit is "not seen up to 64 KB"; then try 256 KB once.
  - Plant `CLAUDE.md` with `@AGENTS.md` beside `AGENTS.md`. `copilot instruction list --json` shows whether the text is sent twice; read only the sources and their flags.
  - Plant `@missing.md` and look for any message.
- [ ] **Step 4: Codex and Qwen Code, one run each.**
  - Codex: plant 40,000 bytes of project `AGENTS.md`, read the warning text, and confirm that `[projects.'C:\lc-7d1\proj'] trust_level = "untrusted"` loads no project file.
  - Qwen Code: confirm that `context.fileName = ["CLAUDE.md"]` drops `AGENTS.md`, and look at what a missing `@x.md` leaves.
- [ ] **Step 5: Record.** Write "What 7d1's first task saw" at the end of `docs/research/2026-11-24-instruction-files.md`, numbered findings with the version of each CLI.
  - Update this plan's constants table (below) wherever a finding differs, and amend PLAN §9 7d decision 10 with one line naming what was settled.
  - Stop only the PIDs this task started. Remove `C:\lc-7d1` only with the user's OK.
  - Commit: `docs(research): what the five CLIs load, seen in real sessions.`

**The constants the later tasks use** (Task 1 updates this table; each lives in that CLI's `instructions.rs`):

| CLI | Constant | Planned value | Task 1 settles |
|---|---|---|---|
| Claude Code | `IMPORT_DEPTH` (hops below the root file) | 4 | 4 or 5 |
| Claude Code | `MEMORY_LINES`, `MEMORY_BYTES` | 200, 25_000 | the byte count |
| Claude Code | `SKIP_BYTES` | 4 MiB (`MAX_CONFIG_BYTES`) | — |
| Claude Code | `WARN_LINES` | 200 (note only) | the real line count |
| Codex | default `project_doc_max_bytes` | 32_768 | — |
| Antigravity | `RULE_BYTES`, `RULES_BUDGET_TOKENS` | 24_000, 20_000 | — |
| Antigravity | include base | the including file's folder | which folder |
| Antigravity | nested `GEMINI.md`/`AGENTS.md` below the root | when used | start or when used |
| Antigravity | rule without a valid `trigger` | not loaded | loaded or not |
| Qwen Code | `IMPORT_DEPTH` | 5 | — |
| Copilot CLI | size limit | none | a limit, or "not seen up to N KB" |
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
- Modify: `crates/core/src/doctor/mod.rs` (add `pub mod instructions;`), `crates/core/Cargo.toml` (`globset`, at the version `Cargo.lock` has)

**Interfaces:**
- Consumes: Task 2's protocol types; `ext::tokens`; `checks::display_name`; `checks::ORDER` (make it `pub(crate)`); `references::project_id`; `config::hash::hash_bytes`; `server::local_file`; `config::preview::MAX_CONFIG_BYTES`.
- Produces:

```rust
pub enum When { Start, WhenUsed, NotLoaded(String) }      // the String is the reason, worded for "not loaded: <reason>"
pub enum Role { File, Import { by: PathBuf }, Rule, Memory, Managed }
pub struct LoadedFile { pub path: PathBuf, pub role: Role, pub when: When, pub chars: usize, pub cut: Option<String>, pub depth: u32 }
pub struct Cut { pub path: PathBuf, pub title: String, pub detail: String, pub cannot_fix: String }
pub struct CliLoad {
    pub agent: &'static str,
    pub distro: Option<String>,
    pub project: Option<PathBuf>,   // None: user level only
    pub files: Vec<LoadedFile>,
    pub note: Option<String>,       // why no project file loads, e.g. "Codex doesn't trust this folder"
    pub cuts: Vec<Cut>,
}
pub enum Text { Missing, TooLarge, Unreadable, Text(String) }
pub struct Reader { /* memo by canonical path; guard: bool (true on Windows); walk_cap: usize */ }
impl Reader {
    pub fn windows() -> Reader;               // guarded, walk cap 10_000
    pub fn distro() -> Reader;                // unguarded (UNC home), same caps
    #[cfg(test)] pub fn with_walk_cap(cap: usize) -> Reader;
    pub fn read(&mut self, path: &Path) -> Text;      // capped at MAX_CONFIG_BYTES; lossy UTF-8; memoized
    pub fn same_file(&mut self, a: &Path, b: &Path) -> bool;   // canonical paths equal
    pub fn nested(&mut self, root: &Path, names: &[&str]) -> Nested;  // memoized per root; names matched case-insensitively on Windows
}
pub struct Nested { pub files: Vec<PathBuf>, pub stopped: bool }   // below root only, root's own files excluded
pub enum ImportSyntax { At, AgyLink }       // `@path`, or `@[label](path)`
pub struct ImportRules { pub syntax: ImportSyntax, pub depth: u32, pub base: ImportBase, pub inside: Option<PathBuf>, pub home: Option<PathBuf> }
pub enum ImportBase { File, Root(PathBuf) }
/// The file's own row plus each import as a row (depth-first, in text order), each import read once per call.
pub fn expand(cx: &mut Reader, file: &Path, text: &str, rules: &ImportRules, role_of_root: Role, when: When) -> Vec<LoadedFile>;
pub fn strip_html_comments(text: &str) -> String;     // block comments only, as Claude Code and Qwen Code rules do
pub fn has_frontmatter_key(text: &str, key: &str) -> bool;   // `paths`, `applyTo`, `trigger` …
pub fn frontmatter_value(text: &str, key: &str) -> Option<String>;
pub fn tokens_text(tokens: usize) -> String;          // "600", "2.4k"
pub fn items(loads: &[CliLoad], projects: &[PathBuf], home: Option<&Path>) -> Vec<DoctorItem>;
```

Rules for `expand`:
- **Where an import can be:** an `At` import is `@` at the start of a line or after whitespace, followed by a path that starts with `.`, `/`, `~` or a letter. It runs to the next whitespace, with `\ ` escaping a space. Nothing inside a fenced code block or an inline code span counts.
- **Resolving:** `~/` resolves under `rules.home`. A relative path resolves from the importing file's folder (`ImportBase::File`) or from the root (`ImportBase::Root`). An absolute path is used as it is.
- **Outside the allowed folder:** an import outside `rules.inside` (when Some) gets `NotLoaded("outside the project")`.
- **A missing target:** `NotLoaded("not found")`.
- **Depth:** past `rules.depth`, the import gets `NotLoaded("past the import limit")`.
- **Cycles:** a file already expanded in this call gets `NotLoaded("already loaded")`.
- **`chars`:** a file's `chars` are its own text's characters after `strip_html_comments` when the CLI strips them. Each CLI's caller passes the already-stripped text.

Rules for `items`:
- **Grouping:** loads with the same `project` and `distro` form one line in `checks::ORDER`.
- **Start total:** a CLI's start total is the sum of `chars` over its `When::Start` files, through `tokens`.
- **Waste:** over 4,000 start tokens gives the Waste item.
- **Cuts:** each `Cut` gives one Broken item, with `path` set to the cut file's display path.
- **Copy:** all copy is as Global constraints say.
- **Order:** project lines come first, in `projects` order, then the Waste items, then the cuts.

- [ ] **Step 1: Write the failing tests** in `instructions.rs`'s `mod tests`, all under `tempfile::tempdir()`:
  - **The reader:**
    - `the_reader_caps_at_four_mib`: a 4 MiB + 1 byte file reads as `Text::TooLarge`.
    - `non_utf8_text_is_counted_lossily`: UTF-16LE bytes and PNG bytes give `Text::Text` with no panic.
    - `the_reader_reads_a_file_once`: a second read of a deleted file still answers the first text.
  - **Imports:**
    - `an_at_import_inside_code_is_not_an_import`: an import in a fence and one in backticks give no import rows.
    - `an_import_cycle_stops`: A→B→A gives rows A, B, then A as `NotLoaded("already loaded")`.
    - `the_depth_limit_holds`: a 6-deep chain with depth 4 gives 4 loaded imports, then one `NotLoaded("past the import limit")`.
    - `an_import_outside_inside_is_refused` and `a_missing_import_is_not_found`.
    - `an_agy_link_include_is_read`: `@[x](sub/a.md)` is read with `ImportBase::File`.
  - **Text helpers:**
    - `html_block_comments_are_stripped`: a `<!-- … -->` spanning lines is removed, and `<!-- x -->` inside a code fence is kept.
    - `frontmatter_keys_are_found`: `---\npaths:\n  - src/**\n---` has `paths`; a file with no frontmatter has none; `trigger: always_on` gives the value "always_on".
  - **The nested walk:**
    - `the_walk_honours_gitignore_and_skips_dot_git`: a `node_modules/AGENTS.md` that is gitignored, and `.git/AGENTS.md`, are not found; `docs/AGENTS.md` is.
    - `the_walk_stops_at_the_entry_cap`: with `with_walk_cap(5)` and 20 folders, `stopped` is true.
  - **Items:**
    - `tokens_text_rounds`: 999 → "999", 2,449 → "2.4k", 2,450 → "2.5k".
    - `items_make_one_line_per_project`: two CLIs in one project give one Info item, `instr.<project_id>`, with the exact title "proj: at start, Claude Code loads about 3 tokens of instructions, Codex none." and two `loads`.
    - `over_4k_is_waste`: 16,004 chars at start gives `instrsize.claude.<id>` with the exact title, and the detail names the largest three.
    - `exactly_4k_is_not_waste`: 16,000 chars gives no Waste item.
    - `a_cut_is_broken_and_report_only`.
    - `no_projects_gives_the_outside_line`.
    - `a_distro_line_says_user_instructions`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 doctor::instructions`. It should FAIL to compile.
- [ ] **Step 3: Implement.** Use `ignore::WalkBuilder::new(root).hidden(false).git_ignore(true).git_exclude(true).git_global(false).follow_links(false).max_depth(Some(12))`, plus a `filter_entry` that drops `.git`. Count entries against the cap. `globset` isn't used here: Task 4 uses it.
- [ ] **Step 4:** Run the same command; it should PASS. Then `cargo clippy -p marshell-core --all-targets -j 4 -- -D warnings` should be clean.
- [ ] **Step 5:** Commit: `feat(doctor): the engine that reads instruction files, follows imports and builds the instruction items.`

### Task 4: Claude Code's instruction files (core; worker at `effort: high`)

**Files:**
- Create: `crates/core/src/agents/claude/instructions.rs`, adding `pub mod instructions;` to `agents/claude/mod.rs`
- Modify: `crates/core/Cargo.toml` (`globset`, if Task 3 didn't add it)

**Interfaces:**
- Consumes: Task 3's engine; `compact::read_settings`; `compact::managed_settings_paths`.
- Produces: `pub fn load(roots: &ExtRoots, project: Option<&Path>, cx: &mut Reader) -> CliLoad`, and `pub fn memory_dir_name(project: &Path) -> String`.

The rules (from the research note, §1, and Task 1's findings):
1. **Managed:** `<managed dir>/CLAUDE.md` is `Role::Managed` and loads at start. Its row note adds "managed by your organisation". `claudeMdExcludes` doesn't apply to it.
2. **The mode:** it comes from `pluginConfigs["cc-plugin-agents-md@builtin"].options.instructionFiles` in the managed settings, then in the user settings (`roots.claude_settings`). Project and local files are ignored for this key. The default is `claude-md-or-agents-md`.
   - `managed-only`: every user, project and local file is `NotLoaded("Claude Code is set to load only managed instructions")`.
   - `claude-md`: AGENTS.md files are `NotLoaded("Claude Code is set to read CLAUDE.md only")`.
3. **User level:** `roots.claude_dir/CLAUDE.md` and `roots.claude_dir/rules/**/*.md`. A rule with `paths` loads when used; one without loads at start.
4. **Ancestors:** each folder from the filesystem root down to `project`. In each, in this order: `CLAUDE.md`, `.claude/CLAUDE.md`, `CLAUDE.local.md`, then `AGENTS.md` and `.claude/AGENTS.md`.
   - `or-agents` mode: the AGENTS.md files load only when no `CLAUDE.md`, `.claude/CLAUDE.md` or `CLAUDE.local.md` exists in any of these folders. Otherwise each existing one is `NotLoaded("CLAUDE.md is here")`. The user's `~/.claude/CLAUDE.md` doesn't count.
   - `and-agents` mode: CLAUDE.md comes first in each folder, and an AGENTS.md already loaded is `NotLoaded("already loaded")`.
   - The same canonical file is counted once (`cx.same_file`), which covers the user home being one of the ancestors.
5. **Project rules:** `project/.claude/rules/**/*.md`, as in step 3.
6. **Nested:** `cx.nested(project, &["CLAUDE.md", "CLAUDE.local.md", "AGENTS.md"])`. Each file loads when used, and nested AGENTS.md follows the mode.
7. **`claudeMdExcludes`:** the arrays from user, project and local settings are merged. They match the file's absolute path with forward slashes, through `globset` (`literal_separator(true)`, case-insensitive on Windows). A match is `NotLoaded("claudeMdExcludes leaves it out")`.
8. **Imports:** `ImportSyntax::At`, `IMPORT_DEPTH`, `ImportBase::File`, `home = roots.home`, `inside = None`. An import of a project file that lands outside `project` adds "Claude Code asks once before loading it" to its note, and still counts.
9. **HTML comments:** every file's chars are counted after `strip_html_comments`.
10. **Auto memory:** off when `autoMemoryEnabled` is false in the merged settings. Otherwise `roots.claude_dir/projects/<memory_dir_name(project)>/memory/MEMORY.md` is `Role::Memory`, and its `chars` are the first `MEMORY_LINES` lines or `MEMORY_BYTES` bytes, whichever is shorter. `memory_dir_name` replaces every character that isn't an ASCII letter or digit with `-` (`C:\dev\Airport` → `C--dev-Airport`); Task 1 confirms it. With no project, there is no memory row.
11. **Cuts:** a file over 4 MiB (`Text::TooLarge`) gives a Cut.
    - Title: "Claude Code skips <path> in <project>".
    - Detail: "It is over 4 MiB, so Claude Code loads none of it."
    - `cannot_fix`: "Shorten it, or split it into files that load when used."
    - A file over `WARN_LINES` lines gets "over <WARN_LINES> lines: Claude Code warns at start" in its note (no item).

- [ ] **Step 1: Write the failing tests** in the new file's `mod tests`. Homes and projects are under a tempdir, and roots come from `roots_under`.
  - `claude_md_and_local_load_root_to_project`: rows for `C/CLAUDE.md`, then `C/proj/CLAUDE.md`, then `CLAUDE.local.md`, all at start.
  - `agents_md_is_skipped_when_claude_md_is_here`.
  - `agents_md_loads_when_no_claude_md`.
  - `and_mode_loads_both_once`.
  - `managed_only_leaves_everything_else_out`.
  - `a_linked_claude_md_counts_once`: a symlink CLAUDE.md → AGENTS.md. If the symlink can't be made (no Developer Mode), the test returns early, with a comment.
  - `path_rules_load_when_used`.
  - `excludes_leave_a_file_out`: `claudeMdExcludes: ["**/docs/CLAUDE.md"]`.
  - `memory_counts_its_first_200_lines`.
  - `memory_off_when_auto_memory_is_disabled`.
  - `a_file_over_4_mib_is_a_cut`, with the exact title.
  - `nested_files_load_when_used`.
  - `memory_dir_name_replaces_non_alphanumerics`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 claude::instructions`. It should FAIL.
- [ ] **Step 3: Implement.** Check that `THIRD_PARTY_NOTICES.md` already names `globset`. If it doesn't, add it in the same commit, the way `ignore` is listed.
- [ ] **Step 4:** Run the same command; it should PASS. Clippy clean.
- [ ] **Step 5:** Commit: `feat(doctor): Claude Code's instruction files, imports, rules and auto memory are read.`

### Task 5: Codex's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/codex/instructions.rs`
- Modify: `agents/codex/extensions.rs` (make `trusted` `pub(crate)`), `agents/codex/mod.rs`

**Interfaces:** `pub fn load(roots: &ExtRoots, project: Option<&Path>, cx: &mut Reader) -> CliLoad`.

The rules:
1. **User level:** `roots.codex_home/AGENTS.override.md` when it is non-empty, else `AGENTS.md`. It loads at start and isn't counted against the budget. When the override wins and `AGENTS.md` exists, `AGENTS.md` is `NotLoaded("AGENTS.override.md is here")`.
2. **`config.toml`:** read `project_doc_max_bytes` (default 32,768), `project_doc_fallback_filenames` (a fallback whose name has a `/` or `\` is ignored), and `project_root_markers` (default `[".git"]`).
3. **Trust:** when `trusted(user_config, project)` is false, every project file is `NotLoaded("Codex doesn't trust this folder")` and the load's note is "Codex doesn't trust this folder".
4. **A zero budget:** `project_doc_max_bytes = 0` makes every project file `NotLoaded("project_doc_max_bytes is 0")`, with the same note.
5. **The project root:** the nearest folder at or above `project` that holds any marker. With none, it is `project` itself.
6. **Per folder,** from the root down to `project`: the first non-empty file of `AGENTS.override.md`, `AGENTS.md`, then the fallbacks, loads. The others that exist are `NotLoaded("<winner> is here")`.
7. **The budget:** bytes are spent from the root down.
   - The file that crosses the budget keeps its first remaining bytes. Its `chars` count those bytes, decoded lossily, and its note gets "cut: Codex loads <kept> of <size> bytes".
   - Every later file is `NotLoaded("past project_doc_max_bytes")`.
   - One Cut item per project:
     - Title: "Codex cuts its project instructions in <project>".
     - Detail: "It loads <budget> bytes of project instructions, and these are over: <path> (<lost> bytes cut), <path> (not loaded)."
     - `cannot_fix`: "Set project_doc_max_bytes in ~/.codex/config.toml to raise the limit, or shorten the files." The `~/.codex` part is the display form of `roots.codex_home`.
8. **Nested:** the same names, through `cx.nested`, load when used. No imports.

- [ ] **Step 1: Write the failing tests:**
  - `user_override_beats_agents_md`;
  - `one_file_per_folder_root_to_project`;
  - `fallback_names_are_followed`;
  - `a_fallback_with_a_path_is_ignored`;
  - `untrusted_loads_no_project_file`;
  - `zero_budget_loads_none`;
  - `the_crossing_file_is_cut_and_later_ones_skipped`, with a 30,000-byte root file and a 5,000-byte sub file, giving the exact Cut title and detail numbers;
  - `no_marker_means_only_the_project_folder`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 codex::instructions`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): Codex's AGENTS.md chain, its fallbacks, trust and its 32 KiB budget are read.`

### Task 6: Antigravity's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/antigravity/instructions.rs`, adding the line to `mod.rs`

**Interfaces:** `pub fn load(roots: &ExtRoots, project: Option<&Path>, cx: &mut Reader) -> CliLoad`.

The rules (Task 1's findings win where they differ):
1. **User level:** `gemini_dir/GEMINI.md`, `gemini_dir/AGENTS.md`, `gemini_dir/config/GEMINI.md`, `gemini_dir/config/AGENTS.md` and `gemini_dir/config/rules/*.md`.
2. **Project:** from the repo root (the nearest `.git` at or above `project`, else `project`) down to `project`, take `GEMINI.md` and `AGENTS.md` in each folder. Then the rules in `project/{.agents,.agent,_agents,_agent}/rules/*.md`.
3. **Rules,** by `trigger`:
   - `always_on`: at start.
   - `glob`: when used.
   - `model_decision`: when used. The note adds "its description loads at start", and the description's characters count at start.
   - `manual`: when used, with the note "only when you mention it".
   - Any other value, or none: per Task 1's finding.
4. **Includes:** `ImportSyntax::AgyLink`, with the base from Task 1 and depth 5 (unknown; the limit only guards against loops).
5. **Cuts.** A file whose text, includes expanded, is over `RULE_BYTES` keeps its text up to the last line break before the limit.
   - Title: "Antigravity cuts <path> in <project>".
   - Detail: "It loads the first 24,000 bytes of each rule, so <lost> bytes of it never reach the agent."
   - `cannot_fix`: "Shorten it, or split it into rules."
6. **The budget:** when the always-on rules and the user rules together pass `RULES_BUDGET_TOKENS` (counted in load order), the rest are `NotLoaded("past Antigravity's 20,000-token budget, sent as a path")` and one Cut follows.
   - Title: "Antigravity sends some rules as paths only in <project>".
   - Detail: "Its always-on and user rules are over 20,000 tokens, so these reach the agent as paths: <paths>."
   - `cannot_fix`: "Give the rules it needs only sometimes trigger: model_decision or glob."
7. **Nested** `GEMINI.md` and `AGENTS.md`, per Task 1's finding.

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
- Modify: `agents/qwen/extensions.rs` (make `folder_trusted`, and whatever reads `security.folderTrust.enabled`, `pub(crate)`), `mod.rs`

**Interfaces:** `pub fn load(roots: &ExtRoots, project: Option<&Path>, cx: &mut Reader) -> CliLoad`.

The rules:
1. **Names:** `context.fileName` from the user's `qwen_dir/settings.json`, a string or an array, replaces `["QWEN.md", "AGENTS.md"]`. When it is set and leaves out `AGENTS.md`, an existing `AGENTS.md` is `NotLoaded("context.fileName doesn't name it")`.
2. **User level:** `qwen_dir/<each name>`, and `qwen_dir/rules/**/*.md`. A rule with `paths` loads when used, one without at start, and HTML comments are stripped.
3. **Trust:** when folder trust is on and `project` isn't trusted, the project files are `NotLoaded("Qwen Code doesn't trust this folder")`, with that note.
4. **The walk:** from the project root (the nearest `.git` at or above `project`; with none, every folder up to, but not including, the user home's parent) down to `project`, each name in each folder. `.qwen/QWEN.local.md` at the git root (only with a git root), and `project/.qwen/rules/**/*.md`.
5. **Imports:** `ImportSyntax::At`, depth 5, `ImportBase::File`, `inside = Some(project root)`. A missing import keeps its literal text, which is already in the file's chars, and its row is `NotLoaded("not found")`.
6. **No limits, so no cuts.** Nothing below `project` loads (path rules are covered above).

- [ ] **Step 1: Write the failing tests:**
  - `default_names_load_root_to_project`;
  - `file_name_setting_replaces_the_list`;
  - `untrusted_loads_no_project_file`;
  - `local_file_needs_a_git_root`;
  - `an_import_outside_the_root_is_refused`;
  - `rules_strip_html_comments`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 qwen::instructions`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): Qwen Code's context files, rules, imports and folder trust are read.`

### Task 8: Copilot CLI's instruction files (core)

**Files:**
- Create: `crates/core/src/agents/copilot/instructions.rs`, adding the line to `mod.rs`

**Interfaces:** `pub fn load(roots: &ExtRoots, project: Option<&Path>, cx: &mut Reader) -> CliLoad`.

The rules:
1. **User level:** `copilot_dir/copilot-instructions.md`, and `copilot_dir/instructions/**/*.instructions.md`. A file with `applyTo` loads when used, one without at start.
2. **The repo:** the git root R is the nearest `.git` at or above `project`, else `project`. For each folder from R down to `project`: `AGENTS.md`, `CLAUDE.md`, `.claude/CLAUDE.md` and `GEMINI.md` load at start.
   - At R: `.github/copilot-instructions.md` (start), `.github/instructions/**/*.instructions.md` (`applyTo` decides), and `.claude/rules/**/*.md` (with `paths`, when used; without, at start).
3. **Imports:** `ImportSyntax::At`, `ImportBase::File`, depth `IMPORT_DEPTH`, `inside = Some(R)`, `home = None` (a `~/` import isn't loaded: `NotLoaded("outside the repository")`). Imports apply only in `copilot-instructions.md`, `AGENTS.md` and `CLAUDE.md`. In `GEMINI.md` and `*.instructions.md` an `@` is plain text.
4. **Duplicates:** a file whose bytes equal a file already loaded at start is `NotLoaded("same text as <path>")`. An import that repeats a file Copilot also loads by name counts twice: that is what Task 1's step 3 saw, or the finding there amends this.
5. **The size limit** comes from Task 1. With none, there are no cuts.
6. **Nested** `AGENTS.md` below `project`: when used.

- [ ] **Step 1: Write the failing tests:**
  - `repo_files_load_from_the_git_root_to_the_project`;
  - `claude_rules_with_paths_load_when_used`;
  - `imports_only_in_three_file_kinds`;
  - `a_home_import_is_not_loaded`;
  - `identical_files_count_once`;
  - `user_instructions_follow_apply_to`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 copilot::instructions`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the same command; it should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): Copilot CLI's instruction files, the other CLIs' files it reads, and its imports are read.`

### Task 9: The run gathers the loads (core server)

**Files:**
- Create: `crates/core/src/server/doctor_instr.rs`, with `mod doctor_instr;` in `server/mod.rs`
- Modify: `crates/core/src/server/doctor.rs` (`Gathered`, `facts_of`, `run_once`)
- Test: `crates/core/src/server/doctor.rs`'s tests and `crates/core/tests/doctor_api.rs`

**Interfaces:**
- Consumes: Tasks 3 to 8.
- Produces: `pub(crate) fn loads(st: &AppState, facts: &[AgentFacts], distro_homes: &[(String, PathBuf)], projects: &[PathBuf]) -> Vec<CliLoad>`, and the field `Gathered::loads: Vec<CliLoad>`.

The rules:
- **Windows CLIs:** each CLI with `found && error.is_none()` is read once per project, with `windows_roots(st)` and one `Reader::windows()` shared across all of them. With no projects, each is read once with `project = None`.
- **A project that can't be read:** a project whose folder isn't a readable local directory gives each CLI a `CliLoad` with no files and the note "Marshell can't read <path>".
- **Distros:** each running managed distro (`distro_homes`) is read at user level (`project = None`), with `roots_under(home, None)` and `Reader::distro()`.
- **`run_once`:** it calls `instructions::items(&facts.loads, &facts.projects, home)` after `ext::items`.

- [ ] **Step 1: Write the failing tests:**
  - `the_report_has_a_line_per_project` (server tests, with a pinned `ext_home` and two projects in `projects.json`): two `instr.` items, each with `loads`.
  - `a_missing_project_gives_a_line_and_no_failure`.
  - In `tests/doctor_api.rs`, `the_instruction_line_rides_in_the_report`: `GET /v1/doctor/report` after a run holds an `instr.` item whose `loads[0].files[0].path` is `"CLAUDE.md"`.
- [ ] **Step 2:** Run `cargo test -p marshell-core -j 4 doctor`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p marshell-core -j 4 doctor` and `cargo test -p marshell-core -j 4 --test doctor_api`; both should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): each run reads every project's instruction files for every installed CLI.`

### Task 10: The expansion on the Doctor row (frontend)

**Files:**
- Modify: `src/features/doctor/DoctorRow.tsx`, `src/features/doctor/Doctor.css`, `src/features/doctor/fixtures.ts` (one fixture item with `loads`)
- Test: `src/features/doctor/DoctorPage.test.tsx`, `src/features/doctor/Doctor.css.test.ts`

Read the `good-css` skill before writing the declarations. The look follows the row's existing `doctor__file` and `doctor__why` text, so `frontend-design` decides nothing new.

The markup, after `doctor__file`:
- `<details className="doctor__loads"><summary>Show the files</summary>`.
- Then, for each load, `<p className="doctor__load-cli">{heading}</p>` and `<ul className="doctor__load-files">`.
- Each file is `<li data-when={when} style={{ "--depth": depth }}>`, holding `<code dir="ltr">{path}</code>` and `<span>{note}</span>`.

The styling:
- Rows with `whenUsed` or `notLoaded` use the muted text colour the row already uses for `doctor__why`. The note's words say the state, so colour never carries it alone.
- The indent is `calc(var(--depth) * 1.25em)` on `padding-inline-start`.
- In forced colours, `summary` keeps its marker and uses `CanvasText`, and its focus ring is `Highlight`.

- [ ] **Step 1: Write the failing tests:**
  - `a_row_with_loads_has_a_closed_disclosure`: the static markup has `<details class="doctor__loads">` without `open`, the summary text, both headings, and a row whose text includes "not loaded: CLAUDE.md is here".
  - `a_row_without_loads_has_no_disclosure`.
  - `Doctor.css.test.ts`: the forced-colours block names `.doctor__loads summary`.
- [ ] **Step 2:** Run `pnpm test src/features/doctor`. It should FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `pnpm test src/features/doctor` and `pnpm build`; both should PASS.
- [ ] **Step 5:** Commit: `feat(doctor): an instruction line opens to the files each CLI loads.`

### Task 11: Live check and exit (main thread)

Use the `marshell-live-check` skill. `pnpm tauri dev` from this branch runs against a scratch `MARSHELL_HOME` and scratch CLI homes under the new literal folder `C:\lc-7d1-app\`, with three scratch projects in its `projects.json`. The dev window is never brought to the front, and the webview is driven over CDP with `browser_evaluate`.

Exit criteria:
1. Project A has CLAUDE.md importing `@AGENTS.md`, a path rule, and a nested `docs/AGENTS.md`.
   - Its line names all five CLIs.
   - The expansion shows the import at depth 1, the rule and the nested file as "loads when used", and Codex's AGENTS.md.
   - Claude Code's `/context` in a session there (one prompt) lists the same start files, and its memory-file token figure is within 15% of the estimate.
2. Project B has a 40,000-byte AGENTS.md. Codex's cut item reads exactly as Task 5 says, and Codex's own warning in a session there names the same byte counts.
3. Project C is untrusted for Codex and Qwen Code. Their part of the line reads "none (Codex doesn't trust this folder)" and "none (Qwen Code doesn't trust this folder)".
4. Over 4k: Project B's Codex Waste item names AGENTS.md first.
5. Keyboard: Tab reaches "Show the files", Enter and Space open and close it, and the focus ring shows. Forced colours (emulated with `browser_emulate_media`) keep the marker and the ring.
6. A running WSL distro with a `~/.claude/CLAUDE.md` gets the distro line. A stopped one keeps 7a's "Not checked" line.

Record everything in `docs/research/2026-11-24-instruction-files-live-check.md`. Then come the whole-branch review (`marshell-reviewer` on Opus), one fix pass, and `marshell-ship`.
