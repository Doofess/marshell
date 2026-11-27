# Slice 8a: the instruction library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named instruction docs, at user level and per project, that Marshell copies into all five CLIs' own rule files with a marker and a hash, reviewed as one diff with one Undo, with copies edited by hand flagged on the page and in the Doctor.

**Architecture:** A core module `crates/core/src/instructions/` reads docs (its own frontmatter parser), renders each CLI's copy through the adapters' `library.rs` modules (vendor rules stay in `crates/core/src/agents/<cli>/`), judges every copy against its marker, and plans each action's file edits, one edit per file, for the existing config service (preview, diff, atomic apply, snapshot, Undo). Routes under `/v1/instructions` serve a full-window "Instructions" page. The Doctor gets two item kinds and a new row action, and `marshell uninstall` removes the user-level copies.

**Tech Stack:** Rust (marshell-core, marshell-protocol with ts-rs, marshell-cli), `toml_edit` 0.25, `sha2`; React + TypeScript, the CodeMirror `Editor` of 6q-4, Vitest with `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9 item 8, "Slice 8a decisions (2026-11-27)" 1–53, and `docs/research/2026-11-27-instruction-library-wiring.md`. Read both before any task. Read `docs/research/codex-cli.md` before Task 4.

## Global Constraints

- Every write to a CLI's file, a project file or a doc goes through `ConfigService` (`crates/core/src/config/service.rs`): `preview_own` (or `preview_mixed` in the Doctor) then `apply`. Nothing writes with `std::fs::write`. Library edits record nothing in `managed.json` (decision 49).
- One `FileEdit` per file per change. Two edits of one file in one preview lose one (`config/preview.rs:314-318`).
- Edit TOML only with `toml_edit`; edit `AGENTS.md` only inside `marshell:begin`/`marshell:end` lines (`.claude/rules/config-writes.md`).
- No route takes a file path to write (`.claude/rules/local-api.md`): a doc is named by scope, project root and name, a copy by those and its CLI and distro. A project root must be one of `projects::entries`; a name from a request is found among `doc::read_dir`'s docs by an equal name, without case, and never joined into a path. A new doc's name is joined only after `check_name` passes.
- Every new route is in the token-guarded router and has a "refused without the bearer" test (as `server/search.rs:1208`).
- Vendor rules live in `crates/core/src/agents/<cli>/library.rs`; `crates/core/src/instructions/` never names a CLI's folder or key.
- CLI ids, in this order everywhere: `claude`, `codex`, `antigravity`, `qwen`, `copilot`. Names shown come from `skill::cli_name` (`crates/core/src/agents/skill.rs:28`) in Rust and `CLI_NAME` (`src/features/onboarding/consent.ts`) in TypeScript: Claude Code, Codex, Antigravity CLI, Qwen Code, Copilot CLI.
- Copy file name: `marshell-<name lower case>.md`; Copilot CLI's: `marshell-<name lower case>.instructions.md`.
- Marker line, exactly: `<!-- marshell:doc <name> <hash> from <source>; edit it in Marshell -->`. Codex's: `<!-- marshell:begin doc <name> <hash> from <source>; edit it in Marshell -->` … `<!-- marshell:end doc <name> -->`. `<name>` is the doc's name in lower case. `<hash>` is 12 lowercase hex digits. `<source>` is the doc's path relative to the project with `/` for a project doc, and `~/…` (`display`, `doctor/instructions.rs:1071`, under the Windows user home) for a user doc.
- Text written by Marshell uses `\n`, UTF-8 without a BOM. Text read is judged with `\r\n` taken as `\n` and a leading BOM dropped.
- No new dependency (decision 37). No YAML crate.
- Tests never touch the real `~/.claude`, `~/.codex`, `~/.gemini`, `~/.qwen`, `~/.copilot` or `~/.marshell`. Core tests use `tempfile` folders. Server tests use `AppState::with_ext_home` (`server/mod.rs:623`), `with_agent_path` (`:881`), `with_running_distros` (`:752`), `with_distro_target` (`:737`), `with_distro_which` (`:745`) and the Doctor tests' `pinned_with(&root, profiles)` (`server/doctor.rs:2023`). A test that reaches `wsl.exe` panics (`server/agents.rs:437`). Test distros are named `MarshellTestDistro`, never a real distro's name.
- Rust tests run with `-j 4`. Run `cargo fmt --all` before every Rust commit. Commit messages: `type(scope): what changed, as a sentence.`, with the session's attribution lines.
- UI: keyboard alone and forced colours work; every state is in words, never colour only; anything drawn over the side pane calls `useCoverPage` (`src/features/browser/pageCover.ts`); a sheet that writes puts the focus on its heading, never on Apply (`FixSheet.tsx:37-48`). Before writing CSS, read the `good-css` skill (this repo switches it on).

## Review Focus

1. **A teammate's checkout with `core.autocrlf=true`** turns every committed copy's `\n` into `\r\n`, and Codex's TOML string with it. Every copy must still read "Up to date", never "Changed outside Marshell", and a rewrite of `developer_instructions` must not fill it with `\r` (Task 3 `crlf_copy_is_in_sync`, Task 4 `crlf_config_is_normalized`, Task 5 `crlf_checkout_is_in_sync`).
2. **A doc whose frontmatter breaks** (a teammate's bad merge) must not make Review delete its copies as orphans: its copies stay as they are and its rows say why (Task 5 `broken_doc_keeps_its_copies`).
3. **The person's own files near a copy**: `marshell-security.md` without a marker, a backup `marshell-security-old.md` with a copied marker, an orphan copy edited by hand. None is overwritten or deleted, by Review, the Doctor or uninstall (Task 5 `taken_name_is_left_alone`, `backup_with_a_copied_marker_is_ignored`, `edited_orphan_stays`; Task 8 `uninstall_leaves_edited_and_unmarked_files`).
4. **Codex text outside Marshell's lines**: the person's `developer_instructions` text keeps its decoded value, the rest of `.codex/config.toml` (other keys, comments) and of `AGENTS.md` stays byte for byte, through every add, update and removal (Task 4 `keeps_the_persons_developer_text`, `keeps_comments_and_other_keys`, `removes_a_block_and_its_blank_line`).
5. **A project in a stopped distro** is never read by any route, since reading `\\wsl$` starts the distro (decision 47; Task 6 `stopped_distro_project_reads_nothing`, `stopped_distro_project_refuses_writes`).

---

### Task 0: Record the plan's rulings (done with this plan)

- [x] Decisions 39 to 53 and the amendments to 31, 36, 38, 43, 45 and 51 are in `docs/PLAN.md` §9 "Slice 8a decisions", committed with this plan.

---

### Task 1: Protocol types

**Files:**
- Create: `crates/protocol/src/instructions.rs`
- Modify: `crates/protocol/src/lib.rs` (`pub mod instructions;`), `crates/protocol/src/doctor.rs:18-28` (add `UseCopy`; reword the enum's doc comment, which says its buttons change what Marshell remembers, not a file)
- Generated: `src/generated/*.ts` (by `cargo test -p marshell-protocol`)

**Interfaces:**
- Produces (all `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)] #[ts(export)]`; the new enums `#[serde(rename_all = "snake_case")]`; `Option` fields `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]`, as `AgentFound` has them):
  - `enum DocScope { User, Project }`
  - `struct LibraryDoc { name: String, scope: DocScope, path: String, description: String, clis: Vec<String>, globs: Vec<String>, enabled: bool, body: String, hash: String, error: Option<String> }` (`hash`: `hash_bytes` of the file; `enabled` is true for user docs)
  - `enum CopyState { InSync, OutOfDate, Missing, ChangedOutside, Extra, Taken, NoCopy, Waiting, Refused }`
  - `struct CopyRow { cli: Option<String>, distro: Option<String>, path: Option<String>, state: CopyState, notes: Vec<String>, shared: bool }` (`cli` None only on a stopped distro's Waiting row; `shared`: Copilot CLI's row that reads Claude Code's `.claude/rules` copy)
  - `struct DocView { doc: LibraryDoc, rows: Vec<CopyRow> }`
  - `struct LibraryView { project: Option<String>, project_note: Option<String>, user_docs: Vec<DocView>, project_docs: Vec<DocView>, user_orphans: Vec<CopyRow>, project_orphans: Vec<CopyRow>, out_of_date: u32, changed_outside: u32, found: Vec<String> }` (`project_note`: decision 47's sentence when the project can't be read; `found`: the CLIs found on this PC)
  - `struct DocSave { scope: DocScope, project: Option<String>, name: String, old_name: Option<String>, description: String, clis: Vec<String>, globs: Vec<String>, enabled: bool, body: String, expected_hash: Option<String> }` (`project`: the project the page shows, for any scope, and where a project doc lives; `expected_hash` None: a new doc)
  - `struct DocSaved { snapshot_id: String, view: LibraryView }`
  - `struct DocRef { scope: DocScope, project: Option<String>, name: String }`
  - `struct LibraryReview { project: Option<String> }`
  - `struct CopyFix { scope: DocScope, project: Option<String>, name: String, cli: String, distro: Option<String>, take_copy: bool }` (`take_copy` false: "Put the doc's text back"; true: "Use this copy's text")
  - `DoctorAction::UseCopy`. `DoctorAction` is `rename_all = "camelCase"` (`doctor.rs:20`), so it serializes as `"useCopy"`. Don't change the enum's serde attribute.

- [ ] **Step 1: Write the failing test** in `crates/protocol/src/instructions.rs`

```rust
#[test]
fn copy_state_and_action_serialize() {
    assert_eq!(serde_json::to_string(&CopyState::ChangedOutside).unwrap(), "\"changed_outside\"");
    assert_eq!(serde_json::to_string(&crate::doctor::DoctorAction::UseCopy).unwrap(), "\"useCopy\"");
    assert_eq!(serde_json::to_string(&crate::doctor::DoctorAction::StopKeeping).unwrap(), "\"stopKeeping\"");
    let row = CopyRow { cli: None, distro: Some("MarshellTestDistro".into()), path: None, state: CopyState::Waiting, notes: vec![], shared: false };
    assert_eq!(serde_json::to_string(&row).unwrap(), r#"{"distro":"MarshellTestDistro","state":"waiting","notes":[],"shared":false}"#);
}
```

- [ ] **Step 2: Run it to see it fail:** `cargo test -p marshell-protocol -j 4 copy_state_and_action` → compile error, the types don't exist.
- [ ] **Step 3: Add the types and `UseCopy`.** In core, `keep_of` takes a bool (`server/doctor.rs:1530`) and needs no change; fix any `match` the compiler names.
- [ ] **Step 4: Run** `cargo test -p marshell-protocol -j 4` → PASS, then `cargo build -p marshell-core -j 4` → builds, then `pnpm check:generated` → no stale file, no `bigint`.
- [ ] **Step 5: Commit** `feat(protocol): the instruction library's types and the Doctor's Use this copy's text action.`

---

### Task 2: Library docs and their frontmatter

**Files:**
- Create: `crates/core/src/instructions/mod.rs`, `crates/core/src/instructions/doc.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod instructions;`)

**Interfaces:**
- Produces in `instructions::doc`:
  - `pub const CLIS: [&str; 5] = ["claude", "codex", "antigravity", "qwen", "copilot"];`
  - `pub struct Doc { pub name: String, pub scope: DocScope, pub path: PathBuf, pub description: String, pub clis: Vec<&'static str>, pub globs: Vec<String>, pub enabled: bool, pub body: String, pub extra: Vec<String>, pub hash: String, pub error: Option<String>, pub duplicate: bool }` (`extra`: the lines of unknown keys, kept verbatim; `duplicate`: decision 53's second file)
  - `pub fn parse(name: &str, scope: DocScope, path: PathBuf, bytes: &[u8]) -> Doc` (never fails; a problem sets `error` and leaves the defaults)
  - `pub fn render(doc: &Doc) -> String` (the doc file's text)
  - `pub fn check_name(name: &str, taken: &[&str]) -> Result<(), String>`
  - `pub fn check_globs(globs: &[String]) -> Result<Vec<String>, String>`
  - `pub fn check_description(text: &str) -> Result<(), String>`
  - `pub fn read_dir(dir: &Path, scope: DocScope) -> Vec<Doc>` (only `*.md` files directly in `dir`, sorted by name without case; a missing folder is an empty list; of two names equal without case the second gets `duplicate: true` and the error "Another doc here is named <first>.")
  - `pub fn find<'a>(docs: &'a [Doc], name: &str) -> Option<&'a Doc>` (equal without case, skipping `duplicate` docs)
  - `pub fn to_protocol(doc: &Doc) -> LibraryDoc`

Defaults when a key is absent: `description` "", `clis` all five, `globs` none, `enabled` true. The body is everything after the closing `---` line, with line endings as `\n` and trailing blank lines cut to one final `\n` (an empty body stays empty). `render` writes these lines, between `---` lines and in this order: `description: '<text>'`, `clis: [a, b]`, `globs: ['x', 'y']` (or `globs: []`), `enabled: true|false` (project docs only), then `extra`. The body follows. Strings are single-quoted with `'` doubled. The parser reads:
- plain, single-quoted and double-quoted scalars (double quotes handle `\"` and `\\` only);
- flow lists `[a, 'b']`, and block lists (`key:` then `  - item` lines);
- `true` and `false`.

It skips lines starting with `#`.

Error copy (exact):
- "The frontmatter has no closing --- line."
- "Line <n>: clis must be a list, such as [claude, codex]." (and the same for `globs`)
- "Line <n>: enabled must be true or false."
- "Line <n>: clis names <x>, which isn't one of claude, codex, antigravity, qwen, copilot."
- "Line <n> of the frontmatter can't be read." (anything else in a known key)
- File name: "This file's name can't name a doc: use letters, digits, - and _."
- `check_name`: "A doc needs a name." / "A name holds letters, digits, - and _ only." / "A name is at most 64 characters." / "Another doc here is named <taken>."
- `check_globs`: "<glob>: a glob can't hold a comma. Write it as two globs." / "<glob>: a glob is relative to the project, so it can't start with /." / "<glob>: a glob can't go above the project with .."
- `check_description`: "A description is one line." / "A description is at most 200 characters."

- [ ] **Step 1: Write the failing tests** in `doc.rs` (`mod tests`):
  - `parses_the_four_keys`: `"---\ndescription: 'Security rules'\nclis: [claude, codex]\nglobs:\n  - 'src/**'\n  - tests/**\nenabled: false\n---\nBody line\n"` → description `"Security rules"`, clis `["claude","codex"]`, globs `["src/**","tests/**"]`, `enabled == false`, body `"Body line\n"`, `error == None`.
  - `no_frontmatter_takes_defaults`: `"Just text\n"` → clis == `CLIS`, globs empty, enabled, description `""`, body `"Just text\n"`.
  - `crlf_and_bom_read_as_lf`: `"\u{feff}---\r\ndescription: x\r\n---\r\nA\r\nB\r\n\r\n\r\n"` → description `"x"`, body `"A\nB\n"`.
  - `unknown_keys_survive_a_save`: a project doc with `owner: team-a` and `tags:\n  - x` → `render` holds `"enabled: true\nowner: team-a\ntags:\n  - x\n---\n"`.
  - `render_round_trips`: a project doc with description `"It's"`, two globs, enabled false → `render` holds `"description: 'It''s'\n"`; `parse(render(d))` gives the same fields.
  - `user_doc_renders_no_enabled`: scope User → `render` has no `enabled:` line.
  - `bad_values_name_their_line`: `"---\nclis: claude\n---\n"` → `"Line 2: clis must be a list, such as [claude, codex]."`; `"---\nenabled: maybe\n---\n"` → `"Line 2: enabled must be true or false."`; `"---\nclis: [claude, gemini]\n---\n"` → `"Line 2: clis names gemini, which isn't one of claude, codex, antigravity, qwen, copilot."`; `"---\ndescription: x\n"` → `"The frontmatter has no closing --- line."`.
  - `names`: `check_name("security", &[])` Ok; `"Sec Ret"`, `""`, 65 × `a` give the three messages; `check_name("SECURITY", &["security"])` → `Err("Another doc here is named security.")`.
  - `globs`: `check_globs(&["src/**", " ", "src/**", " tests/*.rs "])` → `Ok(["src/**", "tests/*.rs"])`; `"src/{a,b}"`, `"/abs/**"` and `"../x"` give their messages.
  - `read_dir_lists_md_files`: a temp folder with `b.md`, `A.md`, `notes.txt`, `bad name.md`, and a folder `sub/` → names `["A", "b", "bad name"]`, the last with the file-name error.
  - `case_twins_mark_the_second` (Linux only, `#[cfg(not(windows))]`): `Sec.md` and `sec.md` → the second has `duplicate` and `"Another doc here is named Sec."`; `find(&docs, "SEC")` is the first.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 instructions::doc` → FAIL (module missing).
- [ ] **Step 3: Implement** `doc.rs` as specified.
- [ ] **Step 4: Run** the same command → PASS.
- [ ] **Step 5: Commit** `feat(core): library docs are read and written with their own small frontmatter parser.`

---

### Task 3: Roots, copies, and the four rule-file CLIs

**Files:**
- Create: `crates/core/src/instructions/roots.rs`, `crates/core/src/instructions/copy.rs`, `crates/core/src/agents/claude/library.rs`, `crates/core/src/agents/antigravity/library.rs`, `crates/core/src/agents/qwen/library.rs`, `crates/core/src/agents/copilot/library.rs`
- Modify: each of those adapters' `mod.rs` (`pub(crate) mod library;`)

**Interfaces:**
- Consumes: `Doc` (Task 2).
- Produces in `instructions::roots`: `pub struct Roots { pub claude: PathBuf, pub codex: PathBuf, pub gemini: PathBuf, pub qwen: PathBuf, pub copilot: PathBuf, pub home: PathBuf }`, with:
  - `Roots::under(home: &Path) -> Roots`: each CLI's default folder under `home` (`.claude`, `.codex`, `.gemini`, `.qwen`, `.copilot`).
  - `Roots::from_env(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> Roots`: `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `qwen::home`, `copilot::home`, and `.gemini` under the home.
  - `Roots::from_ext(r: &ExtRoots) -> Roots` (`doctor/ext.rs:15`).

  `home` is the level's user home, which its paths are shown under as `~/…`.
- Produces in `instructions::copy`:
  - `pub struct Marker { pub name: String, pub hash: String, pub source: String }`
  - `pub fn marker_line(name: &str, hash: &str, source: &str) -> String`
  - `pub fn parse_marker(line: &str) -> Option<Marker>` (both the `marshell:doc` and the `marshell:begin doc` form) and `pub fn parse_end(line: &str) -> Option<String>` (`<!-- marshell:end doc <name> -->`)
  - `pub fn normalized(text: &str) -> String` (BOM dropped, `\r\n` → `\n`)
  - `pub fn payload_hash(payload: &str) -> String` (12 hex digits of SHA-256 of `normalized(payload)`)
  - `pub fn file_copy(frontmatter: &str, name: &str, source: &str, body: &str) -> String` (the frontmatter, then the marker line with the hash of `frontmatter + body`, then the body)
  - `pub enum Judged { InSync, OutOfDate, ChangedOutside, Taken }` and `pub fn judge_file(expected: &str, actual: &str, name: &str) -> Judged`. The marker is the first line, or the line after a leading frontmatter's closing `---`. A missing marker, or one for another name (compared without case), is `Taken`.
  - `pub enum Unwanted { Ours, Edited, NotOurs }` and `pub fn judge_unwanted(actual: &str, own_name: &str) -> Unwanted` (a copy no doc wants: `Ours` when its marker names `own_name`, the name in its file name, and its text hashes to the marker; `Edited` when the marker names `own_name` and doesn't hash to it; `NotOurs` otherwise)
  - `pub fn copy_body(actual: &str) -> Option<String>` (the text after the marker line, normalized; None without a marker)
  - `pub fn source_of(doc: &Doc, project: Option<&Path>, user_home: &Path) -> String`
- Produces in each `agents/<cli>/library.rs` (Claude Code, Antigravity CLI, Qwen Code, Copilot CLI):
  - `pub(crate) fn user_dir(roots: &Roots) -> PathBuf`
  - `pub(crate) fn project_dir(project: &Path) -> PathBuf`
  - `pub(crate) fn file_name(name: &str) -> String`
  - `pub(crate) fn own_name(file_name: &str) -> Option<String>` (the name inside `marshell-<n>.md`, or `marshell-<n>.instructions.md` for Copilot CLI; None for any other file)
  - `pub(crate) fn frontmatter(globs: &[String]) -> String` ("" or a `---` block ending in `\n`)
  - Antigravity only: `pub(crate) const RULE_CAP: usize = 24_000;`

| CLI | `user_dir` | `project_dir` | `file_name("Security")` | `frontmatter([])` | `frontmatter(["src/**","tests/**"])` |
|---|---|---|---|---|---|
| claude | `<claude>/rules` | `.claude/rules` | `marshell-security.md` | `""` | `---\npaths:\n  - 'src/**'\n  - 'tests/**'\n---\n` |
| antigravity | `<gemini>/config/rules` | `.agents/rules` | `marshell-security.md` | `---\ntrigger: always_on\n---\n` | `---\ntrigger: glob\nglobs: '**/src/**, **/tests/**'\n---\n` |
| qwen | `<qwen>/rules` | `.qwen/rules` | `marshell-security.md` | `""` | as claude |
| copilot | `<copilot>/instructions` | `.github/instructions` | `marshell-security.instructions.md` | `""` | `---\napplyTo: 'src/**, tests/**'\n---\n` |

Antigravity's `**/` prefix is added only to a glob that doesn't already start with `**/`.

- [ ] **Step 1: Write the failing tests** (in `roots.rs`, `copy.rs` and each `library.rs`):
  - `marker_round_trips`: `marker_line("security", "0123456789ab", ".marshell/instructions/security.md")` == `"<!-- marshell:doc security 0123456789ab from .marshell/instructions/security.md; edit it in Marshell -->"`; `parse_marker` gives the three parts back.
  - `marker_rejects_other_comments`: `"<!-- note -->"`, a hash of 11 digits, and upper-case hex → None.
  - `payload_hash_ignores_crlf`: `payload_hash("a\r\nb\n") == payload_hash("\u{feff}a\nb\n")`, length 12.
  - `states`: with `expected = file_copy("", "security", "s", "Body\n")`:
    - `judge_file(&expected, &expected, "security")` → InSync;
    - an older copy, `file_copy("", "security", "s", "Old\n")` → OutOfDate;
    - that old copy with `Old` edited to `Mine` and its marker kept → ChangedOutside;
    - `"Body\n"` with no marker → Taken;
    - a copy whose marker names `other` → Taken.
  - `crlf_copy_is_in_sync`: `judge_file(&expected, &expected.replace('\n', "\r\n"), "security")` → InSync.
  - `frontmatter_edit_is_changed_outside`: an Antigravity copy whose `trigger: always_on` was edited to `trigger: manual` → ChangedOutside.
  - `unwanted`: an untouched copy of `gone` → `judge_unwanted(.., "gone")` Ours; its body edited → Edited; the same text under the file name `marshell-gone-old.md` (`own_name` `gone-old`) → NotOurs.
  - `copy_body_takes_the_text_after_the_marker`: `copy_body(&file_copy("---\npaths:\n  - 'a'\n---\n", "x", "s", "Line\n"))` == `Some("Line\n")`.
  - One test per CLI module pinning its table row and `own_name` (`own_name("marshell-a.md")` → `Some("a")` for Claude Code; `own_name("marshell-a.md")` → None and `own_name("marshell-a.instructions.md")` → `Some("a")` for Copilot CLI; `own_name("style.md")` → None).
  - `roots_from_env`: `CODEX_HOME` set moves only `codex`; with nothing set every folder is under the user home.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 instructions::` and `cargo test -p marshell-core -j 4 library::` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** both → PASS.
- [ ] **Step 5: Commit** `feat(core): library copies carry a marker and a hash, written in each CLI's rule format.`

---

### Task 4: Codex: the `AGENTS.md` block and the project's `developer_instructions`

Read `docs/research/codex-cli.md` and the research note's Codex section first.

**Files:**
- Create: `crates/core/src/agents/codex/library.rs`
- Modify: `crates/core/src/agents/codex/mod.rs`; `crates/core/src/agents/codex/instructions.rs:63` (make `untrusted` `pub(crate)`; nothing else changes there). `trust_key` is already `pub(crate)` in `codex/extensions.rs:313`.

**Interfaces:**
- Consumes: `marker_line`, `parse_marker`, `parse_end`, `payload_hash`, `normalized`, `Judged`, `Unwanted` (Task 3).
- Produces in `agents::codex::library`:
  - `pub(crate) struct Block { pub name: String, pub hash: String, pub source: String, pub body: String }`
  - `pub(crate) fn block_text(name: &str, source: &str, body: &str) -> String` (the begin line with `payload_hash(body)`, the body, the end line)
  - `pub(crate) fn blocks_in(text: &str) -> Vec<Block>` (a begin with no matching end is skipped, and the text after it is the person's)
  - `pub(crate) fn judge_block(expected_body: &str, found: &Block) -> Judged` (InSync when the `normalized` bodies match; else OutOfDate when `payload_hash(found.body) == found.hash`; else ChangedOutside)
  - `pub(crate) fn judge_unwanted_block(found: &Block) -> Unwanted` (Ours when its body hashes to its marker, else Edited)
  - `pub(crate) fn with_blocks(text: &str, wanted: &[(String, String)], keep: &[String]) -> Option<String>`:
    - `wanted` is a list of (name, full `block_text`);
    - `keep` names blocks left exactly as they are;
    - every block in neither list is removed;
    - None means only whitespace is left, so the file goes.
  - `pub(crate) fn person_text(text: &str) -> String` (the text with every Marshell block taken out)
  - `pub(crate) fn user_file(codex_home: &Path) -> PathBuf` (`AGENTS.override.md` when `person_text` of it holds more than whitespace, else `AGENTS.md`; decision 45)
  - `pub(crate) fn other_user_file(codex_home: &Path) -> PathBuf` (the one `user_file` didn't pick, whose blocks are unwanted)
  - `pub(crate) fn project_config(project: &Path) -> PathBuf` (`<project>/.codex/config.toml`)
  - `pub(crate) fn sections_in(config_text: &str) -> Result<Vec<Block>, String>`
  - `pub(crate) fn with_sections(config_text: Option<&str>, wanted: &[(String, String)], keep: &[String]) -> Result<Option<String>, String>` (None: delete the file)
  - `pub(crate) enum Trust { Trusted, Untrusted, NoEntry }` and `pub(crate) fn project_trust(user_config: Option<&str>, project_key: &str, git_root_key: Option<&str>) -> Trust` (the folder's own entry, else its git root's, compared through `trust_key`, as `untrusted` finds them; the keys are Windows paths, or Linux paths for a project in a distro)
  - `pub(crate) fn user_sets_developer_instructions(user_config: Option<&str>) -> bool`

Placement (decision 45):
- New blocks and sections go after the person's text, separated by one blank line, after the existing Marshell blocks; new ones are in name order.
- An update replaces a block in place.
- A removal takes the block's separating blank line with it.

`with_sections` reads the file with `toml_edit::DocumentMut`, then:
- takes the decoded `developer_instructions` string through `normalized` before splitting it;
- edits the block lines in that text;
- writes the string back with `value(String)`, copying the old value's `decor` (its comments and spacing) onto the new value;
- leaves every other key and comment untouched.

`toml_edit` 0.25 picks the string's form (`"""` for plain multi-line text, `'''` or escapes otherwise), so tests compare the decoded value, not the form.

Errors (exact): "Codex's .codex/config.toml can't be read, so Marshell doesn't change it." and "developer_instructions in .codex/config.toml isn't text, so Marshell doesn't change it."

- [ ] **Step 1: Write the failing tests:**
  - `adds_a_block_after_the_persons_text`: `with_blocks("Mine\n", &[("security".into(), block_text("security", "~/s.md", "Body\n"))], &[])` == `Some("Mine\n\n<!-- marshell:begin doc security <h> from ~/s.md; edit it in Marshell -->\nBody\n<!-- marshell:end doc security -->\n")`, with `<h> = payload_hash("Body\n")`.
  - `replaces_a_block_in_place`: `"A\n\n<old security block>\n\nB\n"` updated → `"A\n\n<new block>\n\nB\n"`.
  - `removes_a_block_and_its_blank_line`: `"Mine\n\n<block>\n"` with nothing wanted → `Some("Mine\n")`; `"<block>\n"` → None.
  - `keeps_a_block_it_is_told_to_keep`: a block named in `keep` is byte-identical in the result, beside an added one.
  - `block_states`: a fresh block → InSync; its body edited → ChangedOutside; an older body with its own hash → OutOfDate; `judge_unwanted_block` Ours and Edited.
  - `override_with_only_marshell_blocks_is_empty`: no override → `AGENTS.md`; override `"  \n"` → `AGENTS.md`; override holding only a Marshell block → `AGENTS.md`; override `"x\n"` → `AGENTS.override.md`.
  - `section_lives_in_developer_instructions`: `with_sections(Some("model = \"o3\"\n"), &[security], &[])` → text that parses with `toml_edit`, keeps `model = "o3"`, and whose decoded `developer_instructions` is `"<begin line>\nBody\n<end line>\n"`.
  - `keeps_the_persons_developer_text`: from `developer_instructions = "Be brief."` → the decoded string starts `"Be brief.\n\n<!-- marshell:begin doc security "`; removing the section again gives the decoded string `"Be brief."`.
  - `keeps_comments_and_other_keys`: `"# top\nmodel = \"o3\" # mine\ndeveloper_instructions = \"Be brief.\" # why\n[tools]\nx = 1\n"` with a section added → `# top`, `model = "o3" # mine`, `# why` and the `[tools]` table are still in the text.
  - `body_with_backslashes_round_trips`: a body `"Use C:\\\\tmp and \"\"\" quotes\n"` → the decoded section body equals it.
  - `crlf_config_is_normalized`: a file whose `developer_instructions` is a `"""` string written with `\r\n` → after an update the decoded string holds no `\r`.
  - `last_section_out_removes_the_key_and_the_file`: a file holding only the key with one section, nothing wanted → `Ok(None)`; with `model = "o3"` as well → the key is gone and `model = "o3"` stays.
  - `new_file_is_created`: `with_sections(None, &[security], &[])` → `Ok(Some(text))` holding the section.
  - `bad_toml_is_refused`: `"x = ["` → the first error; `developer_instructions = 3` → the second.
  - `trust`:
    - a user config with `[projects.'C:\\p'] trust_level = "trusted"` → Trusted for `C:\p`;
    - `"untrusted"` → Untrusted;
    - no entry → NoEntry;
    - an entry only for the git root `C:\`, with `git_root_key = Some("C:\\")` → that entry's state.
  - `user_developer_instructions`: `"developer_instructions = \"x\"\n"` → true; `""` and None → false.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 codex::library` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS, and `cargo test -p marshell-core -j 4 codex::` → PASS (the trust readers still pass).
- [ ] **Step 5: Commit** `feat(core): Codex gets library docs in a tagged AGENTS.md block and in the project's developer_instructions.`

---

### Task 5: The library planner: rows, and every change as file edits

**Files:**
- Create: `crates/core/src/instructions/library.rs`
- Modify: `crates/core/src/config/guard.rs` (the user agreed on 2026-11-27, decision 36): in `check`, `:176-180`, `<marshell home>/instructions` is excepted from `MarshellOwn` as `agents` is; in the allowed list, `:85`, `allowed.push(home.join("instructions"))` goes next to `agents`.

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces in `instructions::library`:
  - `pub enum Level { User { roots: Roots, distro: Option<String> }, Project { root: PathBuf, notes: Notes } }` and `pub struct Notes { pub codex: Option<String>, pub codex_user_warning: bool, pub qwen: Option<String> }` (decision 17's sentences, read by the caller)
  - `pub struct Place { pub level: Level, pub docs: Vec<Doc>, pub found: Vec<&'static str> }` (`docs`: the level's docs, user or project; `found`: the CLIs found where the level is)
  - `pub struct CopyAt { pub doc: String, pub cli: &'static str, pub path: PathBuf, pub state: CopyState }` (each physical copy once; a copy shared by Claude Code and Copilot CLI is listed under `claude`; Codex's copies share their file and differ by `doc`)
  - `pub struct LevelState { pub rows: Vec<(String, Vec<CopyRow>)>, pub orphans: Vec<CopyRow>, pub out_of_date: u32, pub changed_outside: u32, pub copies: Vec<CopyAt> }` (`rows` per doc name in doc order)
  - `pub enum Pick { Review, Fixes { update: bool, put_back: Vec<(String, &'static str)> }, Delete(String), PutBack(String, &'static str) }`:
    - `Review`: every out-of-date, missing and extra copy.
    - `Fixes`: the Doctor's ticked fixes in one place: Review's edits when `update` is true, and the named changed-outside copies put back.
    - `Delete(doc)`: that doc's copies only, as if the doc were gone.
    - `PutBack(doc, cli)`: one changed-outside copy.
  - `pub fn level_state(place: &Place, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> LevelState`. `user_home` is the Windows user home, for the `~` of a user doc's source. Rows show paths with `display`, under `roots.home` or relative to the project. `editable` is the guard's answer, as `doctor/instructions.rs:1236` takes it.
  - `pub fn plan_edits(place: &Place, pick: &Pick, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> Vec<FileEdit>` (one edit per file; a Codex file's one edit is computed from every block's fate, the blocks the pick doesn't touch in `keep`)
  - `pub fn review_plan(places: &[Place], user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> ChangeSet` (title "Update the copies", reason "Write each library doc's copies as the doc is now.")
  - `pub fn save_plan(doc: &Doc, old: Option<&Doc>, expected: Option<&str>) -> ChangeSet` (title "Save <name>": `CreateFile` or `ReplaceText` of the doc with `expected_hash`, plus `DeleteFile` of the old file on a rename)
  - `pub fn delete_plan(places: &[Place], name: &str, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> ChangeSet` (title "Delete <name> and its copies": the doc's `DeleteFile` once, then `plan_edits(Pick::Delete)` in each place; a user doc's places are Windows and every running distro)
  - `pub fn put_back_plan(place: &Place, name: &str, cli: &str, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> Option<ChangeSet>` (None unless that copy is ChangedOutside now; title "Put the doc's text back in <display path>")
  - `pub fn take_copy_plan(place: &Place, name: &str, cli: &str, user_home: &Path, editable: &dyn Fn(&Path) -> bool) -> Option<ChangeSet>` (None unless ChangedOutside now; title "Use the text of <display path> in <name>": the doc's `ReplaceText` with the copy's body (`copy_body`, or the block's body for Codex), and that copy's edit from `plan_edits(PutBack)` on the place with the doc's new body; decisions 35 and 44)
  - `pub struct Uninstall { pub changes: ChangeSet, pub left: Vec<String> }` and `pub fn uninstall_plan(roots: &Roots) -> Uninstall`:
    - `changes` (title "Remove the instruction library's copies") holds a `DeleteFile` for every `marshell-*` copy in the four user folders that `judge_unwanted` calls Ours, and Codex's two user files with their Ours blocks removed.
    - `left` holds, for every Edited copy or block: "Left <display path>: it was edited after Marshell wrote it." or "Left the <name> block in <display path>: it was edited after Marshell wrote it."
    - NotOurs files are neither removed nor listed.

Rules `level_state` and `plan_edits` follow (decisions 13, 14, 27, 40–43, 51, 53):

**Which copies a doc wants**
- No copies: a doc with `error`, a `duplicate` doc, and a project doc with `enabled == false`.
- A broken doc's copies are found by its name, get no edit, and are in `keep`. Its rows read "The doc's frontmatter can't be read, so its copies are left as they are."
- An Off doc's copies are `Extra`.
- A user doc wants a copy in each ticked CLI in `found`. A ticked CLI that isn't found gets a `NoCopy` row: "Not installed on this PC, so it gets no copy."
- A project doc wants a copy in each ticked CLI. One not in `found` gets the note "Not installed on this PC. The copy is for teammates who use it."
- A doc with globs wants no Codex copy. Codex's row is `NoCopy`: "Codex has no globs, so this doc doesn't load in Codex."
- An unticked CLI's row is `NoCopy`: "Not ticked for this doc."

**The shared copy in a project**
- A doc for Claude Code serves Copilot CLI too, whatever Copilot CLI's own tick.
- Copilot CLI's row then has `shared: true`, the Claude Code copy's path and state, and the note "Copilot CLI reads .claude/rules, so it loads this too."
- That doc's `.github/instructions` copy is unwanted.
- At user level, each CLI has its own copy.

**States**
- For each wanted copy:
  - no file → `Missing`;
  - otherwise `judge_file` or `judge_block`: `InSync`, `OutOfDate`, `ChangedOutside`, or `Taken` with the note "<display path> isn't a copy Marshell wrote, so Marshell leaves it.";
  - a path `editable` rejects → `Refused` with "Marshell may not write here, so this copy is left out.";
  - a Codex project file `with_sections` refuses → `Refused` with its error.
- For copies no doc wants, scanned in the user folders that exist (found CLIs or not) and in the project folders, using `own_name`:
  - `judge_unwanted` Ours → `Extra`, removed by Review, with the orphan note "From <name>, a doc that is gone. Review removes it." when no doc of that name is in `docs`;
  - Edited → a `ChangedOutside` orphan row with "From <name>, a doc that is gone. It was edited, so Marshell leaves it; delete it yourself if you don't need it.", no edit, not counted, kept in Codex's `keep`;
  - NotOurs → no row, no edit.
  - Unwanted copies of a doc that exists (unticked, Off, or no longer found) are `Extra` rows on that doc. Orphans of a gone doc go in `orphans`.
  - Codex's `other_user_file` blocks are unwanted blocks.

**Counts, notes and edits**
- `out_of_date` counts `Missing`, `OutOfDate` and `Extra` copies. `changed_outside` counts `ChangedOutside` copies of docs that exist. A shared copy counts once in each.
- `ChangedOutside`, `Taken` and `Refused` copies get no edit unless the pick names them.
- Notes:
  - Antigravity's copy over `RULE_CAP` bytes: "Antigravity CLI cuts a rule past 24,000 bytes, and this copy is <n> bytes." (with a thousands comma).
  - A doc with globs, on Antigravity's row: "Antigravity CLI matches globs anywhere in the path, so <first glob> also matches a deeper folder of that name."
  - At project level, the Codex row gets `notes.codex`, and with `codex_user_warning` also "Your developer_instructions in ~/.codex/config.toml stop loading in this project, because the project sets its own."
  - The Qwen row gets `notes.qwen`.
- Every edit carries:
  - the file's current `hash_file` as `expected_hash` (None for a new file);
  - scope `User` or `Project`;
  - `applies: NewSessions`.

- [ ] **Step 1: Write the failing tests** (`library.rs`, temp folders; docs built with `doc::parse`; `editable` is `&|_| true` unless named; "apply" means writing each edit's content to disk, or deleting the file):
  - `new_project_doc_wants_four_files`: one doc, all five CLIs, no globs, an empty project:
    - `plan_edits(Review)` creates `.claude/rules/marshell-security.md`, `.agents/rules/marshell-security.md`, `.qwen/rules/marshell-security.md` and `.codex/config.toml`, and nothing under `.github`;
    - `out_of_date == 4`;
    - Copilot CLI's row is `shared`.
  - `copilot_alone_gets_github_instructions`: clis `[copilot]` → `.github/instructions/marshell-security.instructions.md`.
  - `hand_written_doc_with_claude_shares_with_copilot`: clis `[claude]` → Copilot CLI's row `shared`, no `.github` copy.
  - `github_copy_goes_once_claude_is_ticked`: an untouched `.github/instructions` copy and clis `[claude, copilot]` → its `DeleteFile`.
  - `applied_review_is_in_sync`: apply, run again → every row `InSync`, `out_of_date == 0`, `plan_edits(Review)` empty.
  - `crlf_checkout_is_in_sync`: as above, then rewrite every copy with `\r\n` → still `InSync`.
  - `doc_change_makes_copies_out_of_date`: apply, change the body → four `OutOfDate`, `out_of_date == 4`.
  - `off_doc_removes_its_copies`: apply, then set `enabled: false`:
    - a `DeleteFile` for each of the three rule files;
    - a `DeleteFile` for `.codex/config.toml`, which holds nothing else.
  - `orphan_copy_is_removed`: an untouched `.qwen/rules/marshell-gone.md` and no doc `gone` → its `DeleteFile`; `orphans` has its row and note; counted.
  - `edited_orphan_stays`: the same copy with its body edited → no edit; an orphan row `ChangedOutside` with the "It was edited" note; not counted.
  - `backup_with_a_copied_marker_is_ignored`: `.qwen/rules/marshell-security-old.md` holding a copy of `security` → no edit, no row.
  - `broken_doc_keeps_its_copies`: apply, then make the doc's frontmatter unclosed → no edit, and the rows carry the frontmatter sentence.
  - `taken_name_is_left_alone`: an unmarked `.claude/rules/marshell-security.md` holding `"Mine\n"` → row `Taken`, no edit touches it.
  - `changed_outside_is_not_in_review`: apply, then edit the Qwen copy's body → that row `ChangedOutside`, `changed_outside == 1`, no edit for it.
  - `refused_path_is_left_out`: `editable` false for `.qwen` paths → the Qwen row `Refused`, the other three edits still planned.
  - Rows and notes: `codex_skips_a_doc_with_globs`, `user_doc_skips_a_cli_not_found`, `project_doc_notes_teammates`, `agy_size_warning` (a 24,001-byte body gives "… this copy is 24,0xx bytes.").
  - `user_level_copies`: a user `Place` under a temp home with `found = ["claude", "codex", "copilot"]` → creates:
    - `<claude>/rules/marshell-security.md`;
    - a block in `<codex>/AGENTS.md`;
    - `<copilot>/instructions/marshell-security.instructions.md`.

    No `.gemini` or `.qwen` path is created.
  - `user_copy_of_a_cli_no_longer_found_is_extra`: an untouched `<qwen>/rules/marshell-security.md` with `qwen` not in `found` → its `DeleteFile`.
  - `codex_override_takes_the_blocks`: an override with the person's text → the block goes there, and a block in `AGENTS.md` is removed.
  - `two_codex_docs_share_a_file`: two docs → one edit for `.codex/config.toml` holding both sections; `plan_edits(PutBack("a", "codex"))`, with `b`'s section edited by hand, changes only `a`'s section and keeps `b`'s byte for byte.
  - `fixes_touch_one_file_once`: `Pick::Fixes { update: true, put_back: [("a","codex")] }`, with `a` changed outside and `b` out of date in one `.codex/config.toml` → exactly one edit for that file.
  - Plans:
    - `delete_plan_holds_doc_and_copies`;
    - `put_back_plan_rewrites_one_copy`;
    - `take_copy_plan` (after applying it the copy is `InSync` and the others `OutOfDate`);
    - `put_back_of_an_in_sync_copy_is_none`.
  - `save_plan_renames`: old `security.md`, new name `secrets` → `CreateFile` of `secrets.md` and `DeleteFile` of `security.md`.
  - `uninstall_plan_removes_ours_only`, with these copies in place:
    - untouched copies in the four folders;
    - an edited `<qwen>/rules/marshell-b.md`;
    - an unmarked `marshell-mine.md`;
    - `AGENTS.md` holding `"Mine\n\n<untouched block>\n\n<edited block c>\n"`.

    Expected:
    - four `DeleteFile`s;
    - `AGENTS.md` becomes `"Mine\n\n<edited block c>\n"`;
    - `left` names `marshell-b.md` and block `c`;
    - `marshell-mine.md` is untouched.
  - In `guard.rs` tests, with `Guard::new(vec![], vec![], home)`: `instructions_folder_is_admitted` (`check(home/instructions/a.md)` is Ok); `rest_of_home_stays_closed` (`check(home/settings.json)` is `MarshellOwn`, `check(home/instructions-x/a.md)` is refused).
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 instructions::` and `cargo test -p marshell-core -j 4 config::guard` → FAIL.
- [ ] **Step 3: Implement** `library.rs` and the two guard lines.
- [ ] **Step 4: Run** both → PASS.
- [ ] **Step 5: Commit** `feat(core): the library plans every copy, its review, delete, copy fixes and uninstall as config edits, and the guard admits the user library's folder.`

---

### Task 6: The `/v1/instructions` routes

**Files:**
- Create: `crates/core/src/server/instructions.rs`
- Modify:
  - `crates/core/src/server/mod.rs`: `mod instructions;`, and the routes, in the token-guarded router next to `/v1/doctor/*` at `:1576-1583`.
  - `crates/core/src/server/agents.rs:1328`: pull the body into `pub(crate) fn found_agents(st: &AppState) -> Vec<&'static str>`, which blocks, and have `found` call it.
  - `crates/core/src/server/doctor.rs`: make `managed_entries` (`:353`) and `partition_managed_distros` (`:407`) `pub(crate)`.

**Interfaces:**
- Consumes:
  - Task 5;
  - `doctor_ext::windows_roots` (`server/doctor_ext.rs:41`);
  - `projects::entries(&st.0.home, &|| (st.0.running_distros)())` and its `Listed.read` (`projects.rs:228-247`);
  - `st.distro_target` (it admits the distro's home to the config service, `server/agents.rs:465`);
  - `catalog::wsl_words` and `st.agents_in_distro` (as `server/profiles.rs:122-131` asks);
  - `wsl::to_linux`;
  - the config service on `AppState`;
  - the `editable` the Doctor's gather builds for `instructions::items`.
- Produces:
  - `GET /v1/instructions?project=<root>` → `LibraryView`
  - `POST /v1/instructions/save` `DocSave` → `DocSaved`
  - `POST /v1/instructions/review` `LibraryReview` → `ConfigPreview`
  - `POST /v1/instructions/delete` `DocRef` → `ConfigPreview`
  - `POST /v1/instructions/copy` `CopyFix` → `ConfigPreview`
  - `pub(crate) fn user_places(st: &AppState, distro_homes: &[(String, PathBuf)]) -> Vec<Place>`:
    - first the Windows user level, from `windows_roots`, with `found_agents`;
    - then each `(distro, home)` given, under `Roots::under(home)`, with its `found` asked of the distro's PATH.
  - `pub(crate) fn library_distros(st: &AppState) -> (Vec<(String, PathBuf)>, Vec<String>)`: the running managed distros' homes (from `partition_managed_distros(st, &managed_entries(st))` and `distro_target`), and the stopped managed distros. The Doctor passes its own `Gathered.distro_homes` and `stopped` instead.
  - `pub(crate) fn project_place(st: &AppState, root: &Path, distro_homes: &[(String, PathBuf)]) -> Result<Place, String>` (Err: decision 47's sentence)

Behaviour:
- Every handler runs its file work in `spawn_blocking`.
- `project` must be the root of one of `projects::entries`, else 404 "Marshell doesn't know this project.".
- A stopped distro's project (`Listed.read == false`) reads nothing:
  - the view has `project_note` "<distro> isn't running, so this project's docs can't be read." and no project docs or orphans;
  - `save`, `review`, `delete` and `copy` for it answer 409 with that sentence (decision 47).
- User docs are read from `<st.0.home>/instructions`, project docs from `<root>/.marshell/instructions`, both with `read_dir`.
- Trust notes:
  - Codex, from the level's `<codex>/config.toml`, with `project_trust`:
    - `NoEntry` → "Codex loads it once you trust this project in Codex.";
    - `Untrusted` → "Codex doesn't trust this project, so it loads none of its instructions.";
    - `codex_user_warning` from `user_sets_developer_instructions`.
  - Qwen Code, when `folder_trust_on` and not `folder_trusted`: "Qwen Code loads it once you trust this folder in Qwen Code.".
  - A distro project reads both from that distro's home in `distro_homes`, with the trust key `wsl::to_linux(root)`. With no home it shows none.
- The view:
  - each user doc's rows are its rows from every user place in that order, then one `Waiting` row per stopped distro: "<distro> isn't running, so its copies wait.";
  - each project doc's rows come from the project place;
  - `user_orphans` and `project_orphans` are the places' `orphans`;
  - `out_of_date` and `changed_outside` add up every place shown.
- `save`:
  - It checks the request:
    - `check_name`, against the scope's other docs with the old name left out;
    - `check_globs` and `check_description`;
    - every CLI id is known;
    - a project scope without `project` → 422 "A project doc needs a project.";
    - `old_name` is found with `doc::find` (404 "No doc of that name here." when none);
    - a rename that differs only in case → 422 "A new name must differ by more than case.";
    - a new doc whose file exists → 409.
  - A validation error is a 422 with the message as the body.
  - Then `save_plan`, `preview_own`, and `apply` with the preview's hashes. An apply conflict answers as `/v1/config/apply` answers one (409 with the body `conflictBody` reads).
  - It answers `DocSaved` with the view of `DocSave.project`.
- `review` plans every user place and the project's.
- `delete`: a user doc plans every user place, a project doc the project's. The name is found with `doc::find`, with a 404 as above.
- `copy` builds the place from `scope`, `project` and `distro`, then calls `put_back_plan` or `take_copy_plan`. None → 409 "This copy changed. Check again."
- `review`, `delete` and `copy` answer `preview_own`'s `ConfigPreview`. The page applies it with `/v1/config/apply` and undoes it with `/v1/snapshots/{id}/undo`.

- [ ] **Step 1: Write the failing tests** (in `server/instructions.rs`). The state:
  - a temp Marshell home and `with_ext_home`;
  - `with_agent_path` finding `claude`, `codex` and `copilot` only;
  - one project in `projects.json`;
  - `with_running_distros(|| vec![])`.

  The tests:
  - `view_lists_user_and_project_docs`: a user doc and a project doc on disk → one of each, rows in CLI order; the user doc's Antigravity CLI and Qwen Code rows read "Not installed on this PC, so it gets no copy."
  - `unknown_project_is_404`.
  - `save_creates_the_folder_and_the_doc`: a `DocSave` with no `expected_hash` → `<home>/instructions/security.md` exists with the rendered text; the view's `out_of_date` > 0; `managed.json` has no entry for it.
  - `save_with_a_stale_hash_conflicts` → 409.
  - `save_refuses_a_bad_name` → 422 "A name holds letters, digits, - and _ only.".
  - `delete_of_an_unknown_name_is_404`.
  - `review_then_apply_is_in_sync`: `review` → apply the preview through the config service → the view's `out_of_date == 0`.
  - `review_records_nothing_in_managed_json`.
  - `delete_preview_holds_the_doc_and_copies`.
  - `copy_fix_of_an_in_sync_copy_conflicts` → 409.
  - `stopped_distro_project_reads_nothing`:
    - the setup: `pinned_with(&root, vec![<a WSL profile for MarshellTestDistro>])`, and a project at `\\wsl.localhost\MarshellTestDistro\home\u\repo` in `projects.json`;
    - no running distro;
    - the GET answers 200 with `project_note` "MarshellTestDistro isn't running, so this project's docs can't be read." and empty `project_docs`.
  - `stopped_distro_project_refuses_writes`: the same project → `save`, `review`, `delete` and `copy` answer 409 with that sentence.
  - `stopped_managed_distro_gets_a_waiting_row`: the WSL profile as above, and `managed.json` naming a path under its share, with no running distro → each user doc has the row `{cli: None, distro: "MarshellTestDistro", state: Waiting}`.
  - `running_distro_copies_only_found_clis`:
    - the setup: `with_running_distros` returns the distro, `with_distro_target` gives a temp home, and `with_distro_which` answers `claude` only;
    - a user doc ticked for all five;
    - Review plans one copy in the distro home, under `.claude/rules`.
  - `routes_refuse_without_the_bearer`: each of the five routes → 401 without the token.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 server::instructions` → FAIL.
- [ ] **Step 3: Implement** the routes and the extractions.
- [ ] **Step 4: Run** → PASS, and `cargo test -p marshell-core -j 4 server::agents` → PASS.
- [ ] **Step 5: Commit** `feat(core): the local API serves the instruction library, its saves, review and copy fixes.`

---

### Task 7: The Doctor's library items and fixes

**Files:**
- Create: `crates/core/src/doctor/library.rs`
- Modify: `crates/core/src/doctor/mod.rs`; `crates/core/src/server/doctor.rs` (the gather at `:244-272`; `fix_of` at `:1018-1150`)

**Interfaces:**
- Consumes:
  - `user_places`, `project_place` (Task 6);
  - `level_state`, `plan_edits`, `take_copy_plan`, `Pick` (Task 5);
  - `display`, `hash_part`, `id_tail`, `folder_name` (`doctor/instructions.rs:1047-1227`);
  - `skill::cli_name`.
- Produces: `pub fn items(levels: &[(&Place, &LevelState)]) -> Vec<DoctorItem>` (the project and distro come from `place.level`).

Items (decisions 25, 32, 34, 50):
- Each `ChangedOutside` entry in `copies`:
  - id `libdrift.<hash_part(path)>.<doc name lower>`, group `Broken`;
  - `agent` the CLI, `distro`, `path`;
  - title "<display path> changed outside Marshell";
  - detail "<CLI name> loads text the doc <name> doesn't hold. The fix puts the doc's text back. Use this copy's text takes the copy's text into the doc instead: the text, not the frontmatter.";
  - `fixable: true`, `unchecked: true`, `action: Some(UseCopy)`.
  - Orphans give no item.
- Each place with `out_of_date > 0`:
  - id `libstale.<place><suffix>` (`id_tail(project, distro)`), group `Info`;
  - `fixable: true`, `unchecked: true`;
  - detail "The CLIs load what the copies held before the docs changed. The fix writes them as Review on the Instructions page does.".

  The docs counted are those named by the out-of-date copies, orphans by their marker's name. Titles:

  | Case | Title |
  |---|---|
  | one doc, several copies, a project | "4 copies of SECURITY are out of date in Airport" |
  | one doc, one copy | "1 copy of SECURITY is out of date in Airport" |
  | several docs | "6 copies of 2 docs are out of date in Airport" |
  | user level | "4 copies of SECURITY are out of date at user level" |
  | a distro's user level | "4 copies of SECURITY are out of date at user level in MarshellTestDistro" |

  `SECURITY` is the doc's name as written; `Airport` is `folder_name(root)`.
- `fix_of`. The `lib*` ids are split off before the facts' ids, as `is_instr_id` splits (`server/doctor.rs:1056`), and are planned from `user_places(st, &gathered.distro_homes)` and `project_place` for each of `gathered.projects`:
  - `libdrift.<tail>` must be a fixable item in the stored report;
  - the ticked `libdrift` and `libstale` ids of one place become one `plan_edits(Pick::Fixes { update, put_back })`, so each file has one edit;
  - `libadopt.<tail>` is accepted only when the report holds a fixable `libdrift.<tail>` with `action == Some(UseCopy)`, and only on its own. It plans `take_copy_plan`. Its plan's `item_ids` is `["libadopt.<tail>"]`, so the sheet's Reload plans it again. The title lookup (`:1150`) uses the `libdrift.<tail>` item's title.
  - These edits join the fix's preview through `preview_mixed` with `own` true for them, so they record nothing in `managed.json`.
  - A library file that another ticked fix also edits → 409 "Two of the ticked fixes change <display path>. Apply them one at a time."
  - An id whose copy or place is gone → `cant_fix()`.

- [ ] **Step 1: Write the failing tests:**
  - In `doctor/library.rs`:
    - `drift_item`: a `LevelState` with one ChangedOutside Qwen copy of `security` in project `C:\p\Airport` → one item with id `libdrift.<8 hex>.security`, group Broken, title `".qwen/rules/marshell-security.md changed outside Marshell"`, `unchecked`, `action == Some(UseCopy)`.
    - `two_codex_drifts_have_two_ids`: two changed-outside sections in one `.codex/config.toml` → two items, different ids.
    - `stale_titles`: the five titles of the table, exactly.
  - In `server/doctor.rs` tests:
    - `library_fix_puts_the_doc_back` (a drift item's fix preview rewrites that copy only);
    - `use_copy_fix_writes_doc_and_copy` (a `libadopt.` id);
    - `use_copy_id_without_its_drift_item_is_cant_fix`;
    - `stale_and_drift_in_one_codex_file_is_one_edit`;
    - `library_fix_records_nothing_in_managed_json`;
    - `gone_copy_is_cant_fix`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 doctor::library` and `cargo test -p marshell-core -j 4 server::doctor` → FAIL on the new tests.
- [ ] **Step 3: Implement.** The gather reads `user_places(st, &facts.distro_homes)`, and `project_place` for each of `facts.projects`, inside its existing blocking section. A project that gives Err is skipped.
- [ ] **Step 4: Run** → PASS, and `cargo test -p marshell-core -j 4 doctor` → PASS.
- [ ] **Step 5: Commit** `feat(doctor): copies edited outside Marshell and copies out of date are Doctor items with their fixes.`

---

### Task 8: `marshell uninstall` removes the user-level copies

**Files:**
- Modify: `crates/cli/src/uninstall.rs` (after Copilot CLI's plan, `:390`, and in the distro loop, `:441-470`)

**Interfaces:**
- Consumes: `Roots::from_env`, `Roots::under`, `uninstall_plan` (Task 5).

Behaviour (decision 31):
- Windows: `uninstall_plan(&Roots::from_env(env, user_home))`.
- Each answering distro: `uninstall_plan(&Roots::under(distro_home))`, previewed on its own first and reported as the hooks are.
- A non-empty `changes` goes into `extras`, and sets the distro's `found`, so "nothing of Marshell's." isn't printed for a distro that had copies.
- Output lines (exact):
  - "Instruction library copies found." (Windows) and "<distro>: instruction library copies found.";
  - each `left` line;
  - "Your library docs stay in <home>/instructions." after the removal, when that folder holds a `.md` file.

- [ ] **Step 1: Write the failing tests** (the file's existing harness of temp homes, `Wsl` stubs and `yes`):
  - `uninstall_removes_library_copies`: an untouched `~/.claude/rules/marshell-security.md` and a block in `~/.codex/AGENTS.md` after `"Mine\n"` → after `run(.., yes: true)`:
    - the copy is gone;
    - `AGENTS.md` is `"Mine\n"`;
    - the output holds "Instruction library copies found.".
  - `uninstall_leaves_edited_and_unmarked_files`: an edited copy and an unmarked `marshell-mine.md` stay; the output holds "Left ~/.claude/rules/marshell-b.md: it was edited after Marshell wrote it.".
  - `uninstall_keeps_the_docs`: `<home>/instructions/security.md` stays, and the output holds "Your library docs stay in".
  - `uninstall_removes_distro_copies`: a distro home stub with an untouched Qwen copy and no hooks → the copy is gone, and the output holds "MarshellTestDistro: instruction library copies found." and not "nothing of Marshell's.".
- [ ] **Step 2: Run** `cargo test -p marshell-cli -j 4 uninstall` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(cli): uninstall removes the instruction library's user-level copies and keeps the docs and edited copies.`

---

### Task 9: The page's client and model

**Files:**
- Create: `src/lib/api/instructions.ts`, `src/lib/api/instructions.test.ts`, `src/features/instructions/instructionsModel.ts`, `src/features/instructions/instructionsModel.test.ts`

**Interfaces:**
- Consumes:
  - the generated types (Task 1);
  - `api`, `ApiError` (`src/lib/api/client.ts`);
  - `applyPreview`, `undoSnapshot`, `conflictBody` (`src/lib/api/agents.ts:43-83`);
  - `CLI_NAME` (`src/features/onboarding/consent.ts`).
- Produces in `src/lib/api/instructions.ts`:
  - `libraryView(ep: Endpoint, project: string | null): Promise<LibraryView>`
  - `type SaveOutcome = { ok: true; saved: DocSaved } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string }` and `saveDoc(ep: Endpoint, req: DocSave): Promise<SaveOutcome>`. A 409 whose body `conflictBody` reads as a change → conflict. Any other 409, a 404 or a 422 → its message.
  - `reviewCopies(ep, project: string | null): Promise<ConfigPreview>`, `previewDelete(ep, ref: DocRef)`, `previewCopyFix(ep, fix: CopyFix)`
- Produces in `instructionsModel.ts`:
  - `type Draft = { scope: DocScope; name: string; oldName: string | null; description: string; clis: string[]; globsText: string; enabled: boolean; body: string; hash: string | null }`
  - `draftOf(doc: LibraryDoc): Draft`, `newDraft(scope: DocScope, found: string[]): Draft` (user: the found CLIs; project: all five, decision 10), `isDirty(d: Draft, doc: LibraryDoc | null): boolean`, `toSave(d: Draft, project: string | null): DocSave`
  - `toggleCli(d: Draft, cli: string): Draft` (decision 51: in a project doc, ticking `claude` ticks `copilot`, and `copilot` can't be unticked while `claude` is)
  - `copilotLocked(d: Draft): boolean`
  - `barText(outOfDate: number, changed: number): string | null`
  - `stateText(row: CopyRow): string`
  - `rowLabel(row: CopyRow): string` (the CLI name, with " in <distro>" for a distro row, or the distro alone for a Waiting row)
  - `copyFixes(row: CopyRow, orphan: boolean): boolean` (true only for a `changed_outside` row that isn't an orphan)
  - `copyFixOf(doc: LibraryDoc, row: CopyRow, project: string | null, takeCopy: boolean): CopyFix`

Copy (exact):
- `barText`:

  | `(outOfDate, changed)` | Text |
  |---|---|
  | `(0,0)` | null |
  | `(1,0)` | "1 copy is out of date" |
  | `(4,0)` | "4 copies are out of date" |
  | `(4,1)` | "4 copies are out of date. 1 copy was changed outside Marshell; its row has the fixes." |
  | `(0,2)` | "2 copies were changed outside Marshell; their rows have the fixes." |

- `stateText`:

  | State | Text |
  |---|---|
  | in_sync | "Up to date" |
  | out_of_date | "Out of date" |
  | missing | "No copy yet" |
  | changed_outside | "Changed outside Marshell" |
  | extra | "Review removes this copy" |
  | taken | "Not Marshell's file" |
  | no_copy | "No copy" |
  | waiting | "Waiting for the distro" |
  | refused | "Marshell can't write this copy" |

- [ ] **Step 1: Write the failing tests:**
  - every `barText` and `stateText` case;
  - `toggleCli`:
    - on a project draft: `claude` on → `copilot` on; `copilot` off while `claude` is on → unchanged;
    - on a user draft: the two are independent;
  - `newDraft("user", ["claude","qwen"]).clis` == `["claude","qwen"]`, and `newDraft("project", []).clis.length` == 5;
  - `isDirty`: false right after `draftOf`, true after a body edit;
  - `toSave` splits `globsText` by lines and trims;
  - `copyFixOf` fills `name`, `cli`, `distro` and `take_copy`;
  - `saveDoc` maps:
    - a 409 change `ApiError` → `{ ok: false, conflict: true }`;
    - a 422 → its message.

    It uses a stubbed `api`, as `src/lib/api/doctor.test.ts` stubs it.
- [ ] **Step 2: Run** `pnpm test src/features/instructions/instructionsModel.test.ts src/lib/api/instructions.test.ts` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(instructions): the library page's client and model.`

---

### Task 10: The page's components

Read the `good-css` skill before the CSS. Follow `src/features/doctor/Doctor.css` for the page frame, and use the tokens in `src/styles/`.

**Files:**
- Create: `src/features/instructions/InstructionsPage.tsx`, `DocList.tsx`, `DocForm.tsx`, `LoadsIn.tsx`, `CopiesReview.tsx`, `instructions.css`, `InstructionsPage.test.tsx`
- Modify: `src/features/doctor/FixSheet.tsx:11-34` (export `FileDiffs`, unchanged otherwise)

**Interfaces:**
- Consumes: Task 9; `FileDiffs`; `Editor` (`src/features/editor/Editor.tsx:101`) with `EditorPrefs`; `useCoverPage`, `useDialogFocus`.
- Produces (presentational; every handler a prop):
  - `InstructionsPage({ view, projects, project, open, draft, errors, busy, status, prefs, appChord, onPickProject, onOpen, onNew, onDraft, onSave, onDelete, onReview, onCopyFix, onCheckAgain, onClose, editorRef })`:
    - `open: { scope: DocScope; name: string } | null`;
    - `errors: Partial<Record<"name" | "description" | "globs" | "save", string>>`;
    - `status: { text: string; undo?: () => void } | null`;
    - `onCopyFix: (row: CopyRow, takeCopy: boolean) => void`.
  - `CopiesReview({ title, preview, busy, conflict, onApply, onCancel, onReload })`: role `dialog`, `aria-modal="true"`. It calls `useCoverPage()`, and `useDialogFocus` with the focus on its heading, as `FixSheet` does.

Layout and copy (decisions 3, 6, 11, 19, 20, 26, 43):
- **Root:** `<section aria-labelledby=…>` with the heading "Instructions", a "Check again" button, and a close button labelled "Close Instructions".
- **Bar,** under the heading, when `barText` gives text: the text, then "Review".
  - After an apply: "Copies updated." with an "Undo" button.
  - After an undo: "Put back.".
- **Left list:**
  - The heading "Every project", its docs as buttons (the open one has `aria-current="true"`), "No docs yet." when there are none, and "New doc for every project".
  - Then a `<select aria-label="Project">` of the known projects (as `ProjectsView` in Settings → Projects), the project's docs, and "New doc for this project".
    - With no project: "No project yet. Open a session in a folder, or add one in Settings → Projects."
    - With `project_note`: that note, and no "New doc for this project".
  - Under each group with orphans, the heading "Copies with no doc", and their rows as in "Loads in", with no fixes.
- **Form:**
  - "Name" and "Description": text inputs.
  - "CLIs": a fieldset of five checkboxes named with `CLI_NAME`. A locked Copilot CLI box is `disabled` and checked, with the hint "Copilot CLI reads .claude/rules, so it loads this too."
  - "Globs": a textarea, with the hint "One per line, relative to the project. Empty: the doc always loads."
  - "On in this project": a checkbox, project docs only.
  - "Text": the `Editor`, with `name` = `<name>.md`.
  - Each error sits under its field, tied to it with `aria-describedby`. A doc with `error` shows it above the form.
- **Buttons:** "Save" (hint "Ctrl+S") and "Delete…".
- **Loads in:** a list with one row per `CopyRow`: `rowLabel`, `stateText`, the path, and each note on its own line. A row for which `copyFixes` is true gets "Put the doc's text back" and "Use this copy's text", with the hint "Takes the text, not the frontmatter.".
- **Review sheet:**
  - Titles: "Update the copies", "Delete <name> and its copies", "Put the doc's text back", "Use this copy's text".
  - Buttons: "Apply" and "Cancel".
  - Conflict: "A file changed since this preview. Reload it to see the change." with "Reload".
  - A preview with no file: "Nothing to update: the copies are up to date." and only "Close".

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup`, as `src/features/doctor/FixSheet.test.tsx` does):
  - the bar's text and Review button for `out_of_date: 4`, and no bar for `0, 0`;
  - "No docs yet." for an empty user list, and the no-project sentence;
  - a `project_note` renders the note and no "New doc for this project";
  - a `changed_outside` row renders both fix buttons; an `in_sync` row and a `changed_outside` orphan row render none;
  - "Copies with no doc" renders with an orphan;
  - a shared Copilot CLI row shows its note;
  - a project draft with `claude` ticked renders the Copilot CLI box `disabled` and checked;
  - a user draft renders no "On in this project";
  - a name error renders with `aria-describedby` pointing at it;
  - `CopiesReview` renders `aria-modal="true"`, its title, and a heading with `tabIndex={-1}`.
- [ ] **Step 2: Run** `pnpm test src/features/instructions/InstructionsPage.test.tsx` → FAIL.
- [ ] **Step 3: Implement** the components and CSS. In forced colours, borders and focus rings use system colours, and no state is shown by colour alone.
- [ ] **Step 4: Run** that test and `pnpm test src/features/browser/coverScan.test.ts` → PASS.
- [ ] **Step 5: Commit** `feat(instructions): the Instructions page's list, form, Loads in rows and review sheet.`

---

### Task 11: The live page, its entries, and the Doctor's button

**Files:**
- Create: `src/features/instructions/LiveInstructions.tsx`, `src/features/instructions/liveInstructions.ts` (its pure reducer), `liveInstructions.test.ts`
- Modify:
  - `src/features/sessions/LiveApp.tsx`: the overlay union at `:341` gains `"instructions"`; the palette at `:2215`; the app menu at `:3034`; the mount next to the Doctor's at `:3092`.
  - `src/features/appMenu/appMenu.ts:7,21-26`: `AppMenuRun` gains `"instructions"`, and the item `{ id: "instructions", label: "Instructions" }` goes after Doctor.
  - `src/features/appMenu/appMenu.test.ts:6-8`.
  - `src/lib/keymap.ts:162-166`: `{ id: "instructions", label: "Open Instructions", group: "App" }` after `doctor`, and `"instructions"` in `MOVED_COMMANDS`.
  - `src/features/sessions/LivePalette.tsx:13`: `"instructions"` in `PALETTE_ONLY`.
  - `src/features/doctor/keep.ts` and its test: labels for `useCopy`.
  - `src/features/doctor/LiveDoctor.tsx:134-149`: `useCopy` opens the fix sheet.

**Interfaces:**
- Consumes:
  - Tasks 9–10;
  - `applyPreview`, `undoSnapshot`;
  - `SaveDialog` (`src/features/editor/SaveDialog.tsx`);
  - `useDialogFocus`, `useCoverPage`;
  - `editorPrefsOf` (imported at `LiveApp.tsx:148`);
  - `pickProject` (`projectsDoc.ts:74`) and `GET /v1/projects`.
- Produces: `LiveInstructions({ endpoint, folder, prefs, announce, onClose })`. `folder` is `dialogFolder()` (`LiveApp.tsx:1182`): the focused session's folder, else the shell panel's.

Behaviour:
- **The first project:** `pickProject(projects, folder)` when the project it gives is among the known projects, else none. The picker changes it (decision 11).
- **Saving:**
  - Ctrl+S saves. `appChord` is true for it, so the editor lets it through.
  - A save's 409 conflict shows "<name> changed on disk since you opened it." with "Reload" and "Save over it". "Save over it" saves with the disk's current hash, taken from a fresh view.
  - Validation messages go under their fields and are announced.
- **Closing:**
  - The page calls `useCoverPage()`, as `LiveDoctor` does, since it lies over the side pane.
  - Esc closes it, as it closes the Doctor (`LiveDoctor.tsx:60-62`), unless a sheet or dialog is open.
  - Closing it, or opening another doc, with unsaved edits shows `SaveDialog` named after the doc (decision 48). Save saves, then goes on; Don't save drops the draft; Cancel stays.
- **Review and fixes:** Review, Delete… and the two copy fixes fetch their preview and show `CopiesReview`. Apply uses `applyPreview`, then the Undo status shows; Undo calls `undoSnapshot`. Each step reads the view again (decision 52).
- **Announcements (exact):**
  - after a save: "Saved <name>.";
  - after an apply: "Copies updated.";
  - after an undo: "Put back.";
  - a failed fetch: "Couldn't read the instruction library: <message>".
- **Opening:** the app menu's "Instructions" and the palette's "Open Instructions" open the page. It closes to the terminal as the Doctor does (`setOverlay(null)`, then `focusTerminal`).
- **The Doctor's button:**
  - `keepLabel("useCopy", busy)` is "Use this copy's text", or "Preparing…" while busy.
  - Pressing it calls `planFix(endpoint, ["libadopt." + item.id.slice("libdrift.".length)])` and opens the existing fix sheet, so apply and Undo go through the Doctor's own routes.

- [ ] **Step 1: Write the failing tests:**
  - the `appMenu.test.ts` labels become `["Settings","History","Doctor","Instructions","Departures"]`;
  - in `liveInstructions.test.ts`, the reducer:
    - opening a doc while dirty asks (`state.asking` set) and doesn't switch;
    - `discard` switches;
    - a save conflict sets the conflict line;
    - an apply sets the Undo status;
    - an unknown folder starts with no project;
  - `keep.test.ts`: the `useCopy` labels;
  - the palette's `LIVE_COMMANDS` holds `instructions`.
- [ ] **Step 2: Run** `pnpm test src/features/appMenu src/features/instructions src/features/doctor src/features/sessions` → FAIL on the new cases.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test` → PASS; then `pnpm build` → no type errors.
- [ ] **Step 5: Commit** `feat(instructions): the Instructions page opens from the app menu and the palette, and the Doctor offers Use this copy's text.`

---

### Task 12: The live check (decision 30)

Run it with the `marshell-live-check` skill, against scratch homes only: `MARSHELL_HOME`, `MARSHELL_DEFAULT_FOLDER`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, and `USERPROFILE` for Antigravity CLI and Qwen Code.
- Note agy's version before and after.
- Never use `/model` or `/effort`.
- Findings go in `docs/research/2026-11-27-instruction-library-wiring.md`, under a new heading "Live check (slice 8a)".

- [ ] **Step 1: Set up.** In a fresh scratch project, added in Settings → Projects:
  - make the user doc `GREETING`, whose text is a unique marker sentence;
  - make the project doc `SECURITY`, with another marker and no globs;
  - Review, then Apply.

  With `browser_evaluate`, check that the bar is gone and every row reads "Up to date".
- [ ] **Step 2: Codex:** run `codex debug prompt-input` in the project, before and after trusting it in the scratch `config.toml`. Both markers appear only once it is trusted. The `GREETING` block is in the scratch `AGENTS.md`.
- [ ] **Step 3: Copilot CLI:** `copilot instruction list --json` lists `.claude/rules/marshell-security.md` and the user copy. One turn on the fake model (`-CopilotFake`) logs a request that holds both markers.
- [ ] **Step 4: Qwen Code:** with the folder trusted, one turn on the fake model (`-QwenFake`) logs a request that holds both markers.
- [ ] **Step 5: Antigravity CLI:** `/context` in a session lists both rules (the sign-in copied as in 6g decision 3).
- [ ] **Step 6: Claude Code:** the user signs in once to the scratch `CLAUDE_CONFIG_DIR`, at the PC (decision 9). `/memory` and `/context` list both.
- [ ] **Step 7: Drift:**
  - Edit the Qwen copy by hand. Its row and a Doctor item read "Changed outside Marshell".
  - "Use this copy's text": the doc holds the edit, the other copies are out of date, and the Doctor's out-of-date item appears.
  - Undo puts both back.
- [ ] **Step 8: Globs:** give `SECURITY` the glob `src/**`, then Review. Codex's row reads "Codex has no globs, so this doc doesn't load in Codex." and the Codex section is removed.
- [ ] **Step 9: WSL:**
  - With a distro running that Marshell set a CLI up in: a user doc's copy lands in the distro home, only for the CLIs on its PATH. One project in the distro gets its copies.
  - With the distro stopped: the row reads "<distro> isn't running, so its copies wait." and nothing starts it. Check with `wsl -l --running` before and after.
- [ ] **Step 10: Keyboard and forced colours:** do steps 1 and 7 by keyboard alone. Take a screenshot in forced colours to judge the look.
- [ ] **Step 11: Wind down.** List the tree for any CLI the user started, then stop only what the check started, by PID. Commit the research note: `docs(research): slice 8a's live check of the instruction library in all five CLIs.`

---

### Task 13: Ship

- [ ] **Step 1: Review.** The `marshell-reviewer` agent (Opus) reviews the whole branch with this plan's Review Focus. One fix pass follows for its findings.
- [ ] **Step 2: Ship** with `marshell-ship`:
  - the checks, each in the foreground:
    - `cargo fmt --all`;
    - `cargo clippy --workspace --all-targets -- -D warnings`;
    - `cargo test --workspace -j 4`;
    - `pnpm test`;
    - `pnpm check:generated`;
    - `pnpm build`;
  - the PR, with these sections:
    - "What it does";
    - "Live check";
    - "Known limits": the page doesn't watch files (decision 52), Claude Code's user `paths` are relative to the session's folder, and a distro home recorded as a project shares folders with that distro's user level;
    - "Checks";
  - a green CI, then `pre-merge-review`, then the merge.
