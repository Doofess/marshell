# Slice 8a: the instruction library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named instruction docs, at user level and per project, that Marshell copies into all five CLIs' own rule files with a marker and a hash, reviewed as one diff with one Undo, with copies edited by hand flagged on the page and in the Doctor.

**Architecture:** A core module `crates/core/src/instructions/` reads docs (its own frontmatter parser), renders each CLI's copy through the adapters' `library.rs` modules (vendor rules stay in `crates/core/src/agents/<cli>/`), judges every copy against its marker, and plans one `ChangeSet` per action for the existing config service (preview, diff, atomic apply, snapshot, Undo). New routes under `/v1/instructions` serve a full-window "Instructions" page; the Doctor gets two item kinds and a new row action; `marshell uninstall` removes the user-level copies.

**Tech Stack:** Rust (marshell-core, marshell-protocol with ts-rs, marshell-cli), `toml_edit` 0.25, `sha2`; React + TypeScript, the CodeMirror `Editor` of 6q-4, Vitest with `renderToStaticMarkup`.

**Spec:** `docs/PLAN.md` §9 item 8, "Slice 8a decisions (2026-11-27)" 1–52 (52 added with this plan), and `docs/research/2026-11-27-instruction-library-wiring.md`. Read both before any task. Read `docs/research/codex-cli.md` before Task 4.

## Global Constraints

- Every write to a CLI's file, a project file or a doc goes through `ConfigService` (`crates/core/src/config/service.rs`): `preview_own` then `apply`. Nothing writes with `std::fs::write`. Library edits record nothing in `managed.json` (decision 49).
- Edit TOML only with `toml_edit`; edit `AGENTS.md` only inside `marshell:begin`/`marshell:end` lines (`.claude/rules/config-writes.md`).
- Vendor rules live in `crates/core/src/agents/<cli>/library.rs`; `crates/core/src/instructions/` never names a CLI's folder or key.
- CLI ids, in this order everywhere: `claude`, `codex`, `antigravity`, `qwen`, `copilot`. Names shown: Claude Code, Codex, Antigravity CLI, Qwen Code, Copilot CLI (`crates/core/src/agents/skill.rs:19`, `src/features/onboarding/consent.ts` `CLI_NAME`).
- Copy file name: `marshell-<name lower case>.md`; Copilot CLI's: `marshell-<name lower case>.instructions.md`.
- Marker line, exactly: `<!-- marshell:doc <name> <hash> from <source>; edit it in Marshell -->`. Codex's: `<!-- marshell:begin doc <name> <hash> from <source>; edit it in Marshell -->` … `<!-- marshell:end doc <name> -->`. `<hash>` is 12 lowercase hex digits. `<source>` is the doc's path relative to the project with `/` for a project doc, and `~/…` (the `display` of `doctor/instructions.rs:1071`) for a user doc.
- Text written by Marshell uses `\n`, UTF-8 without a BOM. Text read is judged with `\r\n` taken as `\n` and a leading BOM dropped.
- No new dependency (decision 37). No YAML crate.
- Tests never touch the real `~/.claude`, `~/.codex`, `~/.gemini`, `~/.qwen`, `~/.copilot` or `~/.marshell`: core tests use `tempfile` folders, server tests `AppState::with_ext_home` (`server/mod.rs:623`), `with_running_distros` (`:752`) and `with_distro_target` (`:737`). A test that reaches `wsl.exe` panics (`server/agents.rs:437`).
- Rust tests run with `-j 4`. Run `cargo fmt --all` before every Rust commit. Commit messages: `type(scope): what changed, as a sentence.`, with the session's attribution lines.
- UI: keyboard alone and forced colors work; every state is in words, never colour only; anything drawn over the side pane calls `useCoverPage` (`src/features/browser/pageCover.ts`). Before writing CSS, read the `good-css` skill (this repo switches it on).

## Review Focus

1. **A teammate's checkout with `core.autocrlf=true`** turns every committed copy's `\n` into `\r\n`. Every copy must still read "Up to date", never "Changed outside Marshell" (Task 3 `crlf_copy_is_in_sync`, Task 5 `crlf_checkout_is_in_sync`).
2. **A doc whose frontmatter breaks** (a teammate's bad merge) must not make Review delete its copies as orphans: its copies stay as they are and its rows say why (Task 5 `broken_doc_keeps_its_copies`).
3. **The person's own file that has a copy's name**, `.claude/rules/marshell-security.md` without a marker, is never overwritten or deleted, by Review, the Doctor or uninstall (Task 5 `taken_name_is_left_alone`, Task 8 `uninstall_keeps_files_without_a_marker`).
4. **Codex text outside Marshell's lines** (the person's `developer_instructions`, the rest of `AGENTS.md`, other keys of `.codex/config.toml`) survives every add, update and removal byte for byte (Task 4 `keeps_the_persons_developer_text`, `removes_a_block_and_its_blank_line`).
5. **A project in a stopped distro** is never read, since reading `\\wsl$` starts the distro (decision 47; Task 6 `stopped_distro_project_reads_nothing`).

---

### Task 0: Record the plan's rulings (done with this plan)

- [x] Decisions 39 to 52 were added to `docs/PLAN.md` §9 "Slice 8a decisions" in the commit that added this plan.

---

### Task 1: Protocol types

**Files:**
- Create: `crates/protocol/src/instructions.rs`
- Modify: `crates/protocol/src/lib.rs` (add `pub mod instructions;`), `crates/protocol/src/doctor.rs:23-28` (add `UseCopy`)
- Generated: `src/generated/*.ts` (by `cargo test -p marshell-protocol`)

**Interfaces:**
- Produces (all `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)] #[ts(export)]`, enums `#[serde(rename_all = "snake_case")]`, `Option` fields `#[serde(default, skip_serializing_if = "Option::is_none")] #[ts(optional)]` as `AgentFound` does):
  - `enum DocScope { User, Project }`
  - `struct LibraryDoc { name: String, scope: DocScope, path: String, description: String, clis: Vec<String>, globs: Vec<String>, enabled: bool, body: String, hash: String, error: Option<String> }` (`hash`: SHA-256 hex of the file's bytes, `hash_bytes`; `enabled` is true for user docs)
  - `enum CopyState { InSync, OutOfDate, Missing, ChangedOutside, Extra, Taken, NoCopy, Waiting, Refused }`
  - `struct CopyRow { cli: Option<String>, distro: Option<String>, path: Option<String>, state: CopyState, notes: Vec<String>, shared: bool }` (`cli` None only for a stopped distro's Waiting row; `shared`: Copilot CLI's row that reads Claude Code's `.claude/rules` copy)
  - `struct DocView { doc: LibraryDoc, rows: Vec<CopyRow> }`
  - `struct LibraryView { project: Option<String>, project_note: Option<String>, user_docs: Vec<DocView>, project_docs: Vec<DocView>, out_of_date: u32, changed_outside: u32, found: Vec<String> }` (`project_note`: decision 47's sentence when the project can't be read)
  - `struct DocSave { scope: DocScope, project: Option<String>, name: String, old_name: Option<String>, description: String, clis: Vec<String>, globs: Vec<String>, enabled: bool, body: String, expected_hash: Option<String> }` (`expected_hash` None: a new doc)
  - `struct DocSaved { snapshot_id: String, view: LibraryView }`
  - `struct DocRef { scope: DocScope, project: Option<String>, name: String }`
  - `struct LibraryReview { project: Option<String> }`
  - `struct CopyFix { project: Option<String>, path: String, take_copy: bool }` (`take_copy` false: "Put the doc's text back"; true: "Use this copy's text")
  - `DoctorAction::UseCopy` (serializes `"use_copy"`)

- [ ] **Step 1: Write the failing test** in `crates/protocol/src/instructions.rs`

```rust
#[test]
fn copy_state_and_action_serialize_snake_case() {
    assert_eq!(serde_json::to_string(&CopyState::ChangedOutside).unwrap(), "\"changed_outside\"");
    assert_eq!(serde_json::to_string(&crate::doctor::DoctorAction::UseCopy).unwrap(), "\"use_copy\"");
    let row = CopyRow { cli: None, distro: Some("Ubuntu".into()), path: None, state: CopyState::Waiting, notes: vec![], shared: false };
    assert_eq!(serde_json::to_string(&row).unwrap(), r#"{"distro":"Ubuntu","state":"waiting","notes":[],"shared":false}"#);
}
```

- [ ] **Step 2: Run it to see it fail:** `cargo test -p marshell-protocol -j 4 copy_state_and_action` → compile error, the types don't exist.
- [ ] **Step 3: Add the types and `UseCopy`.** Fix every `match` on `DoctorAction` the compiler names (in `crates/core` the keep route refuses `UseCopy` with 422 "This item has no file to keep."; `src/features/doctor/keep.ts` gets its labels in Task 11).
- [ ] **Step 4: Run** `cargo test -p marshell-protocol -j 4` → PASS, then `pnpm check:generated` → no stale file, no `bigint`.
- [ ] **Step 5: Commit** `feat(protocol): the instruction library's types and the Doctor's Use this copy's text action.`

---

### Task 2: Library docs and their frontmatter

**Files:**
- Create: `crates/core/src/instructions/mod.rs`, `crates/core/src/instructions/doc.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod instructions;`)

**Interfaces:**
- Produces in `instructions::doc`:
  - `pub const CLIS: [&str; 5] = ["claude", "codex", "antigravity", "qwen", "copilot"];`
  - `pub struct Doc { pub name: String, pub scope: DocScope, pub path: PathBuf, pub description: String, pub clis: Vec<&'static str>, pub globs: Vec<String>, pub enabled: bool, pub body: String, pub extra: Vec<String>, pub hash: String, pub error: Option<String> }` (`extra`: the lines of unknown keys, kept verbatim)
  - `pub fn parse(name: &str, scope: DocScope, path: PathBuf, bytes: &[u8]) -> Doc` (never fails; a problem sets `error` and leaves the defaults)
  - `pub fn render(doc: &Doc) -> String` (the doc file's text)
  - `pub fn check_name(name: &str, taken: &[&str]) -> Result<(), String>`
  - `pub fn check_globs(globs: &[String]) -> Result<Vec<String>, String>`
  - `pub fn check_description(text: &str) -> Result<(), String>`
  - `pub fn read_dir(dir: &Path, scope: DocScope) -> Vec<Doc>` (only `*.md` files directly in `dir`, sorted by name without case; a missing folder is an empty list)
  - `pub fn to_protocol(doc: &Doc) -> LibraryDoc`

Defaults when a key is absent: `description` "", `clis` all five, `globs` none, `enabled` true. The body is everything after the closing `---` line, with line endings as `\n`, trailing blank lines cut to one final `\n` (an empty body stays empty). `render` writes, in this order: `description: '<text>'`, `clis: [a, b]`, `globs: ['x', 'y']` (or `globs: []`), `enabled: true|false` (project docs only), then `extra`, between `---` lines, then the body. Strings are single-quoted with `'` doubled. The parser reads: plain, single-quoted and double-quoted (`\"` and `\\` only) scalars; flow lists `[a, 'b']`; block lists (`key:` then `  - item` lines); `true`/`false`; lines starting with `#` are skipped.

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
  - `unknown_keys_survive_a_save`: a doc with `owner: team-a` and `tags:\n  - x` → `render` holds `"enabled: true\nowner: team-a\ntags:\n  - x\n---\n"`.
  - `render_round_trips`: a project doc with description `"It's"`, two globs, enabled false → `render` holds `"description: 'It''s'\n"`; `parse(render(d))` gives the same fields.
  - `user_doc_renders_no_enabled`: scope User → `render` has no `enabled:` line.
  - `bad_values_name_their_line`: `"---\nclis: claude\n---\n"` → error `"Line 2: clis must be a list, such as [claude, codex]."`; `"---\nenabled: maybe\n---\n"` → `"Line 2: enabled must be true or false."`; `"---\nclis: [claude, gemini]\n---\n"` → `"Line 2: clis names gemini, which isn't one of claude, codex, antigravity, qwen, copilot."`; `"---\ndescription: x\n"` → `"The frontmatter has no closing --- line."`.
  - `names`: `check_name("security", &[])` Ok; `"Sec Ret"`, `""`, 65 × `a` give the three messages; `check_name("SECURITY", &["security"])` → `Err("Another doc here is named security.")`.
  - `globs`: `check_globs(&["src/**", " ", "src/**", " tests/*.rs "])` → `Ok(["src/**", "tests/*.rs"])`; `"src/{a,b}"`, `"/abs/**"`, `"../x"` give their messages.
  - `read_dir_lists_md_files`: a temp folder with `b.md`, `A.md`, `notes.txt`, `bad name.md`, and a folder `sub/` → names `["A", "b", "bad name"]`, the last with the file-name error.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 instructions::doc` → FAIL (module missing).
- [ ] **Step 3: Implement** `doc.rs` as specified.
- [ ] **Step 4: Run** the same command → PASS.
- [ ] **Step 5: Commit** `feat(core): library docs are read and written with their own small frontmatter parser.`

---

### Task 3: Copies: marker, hash, state, and the four rule-file CLIs

**Files:**
- Create: `crates/core/src/instructions/roots.rs`, `crates/core/src/instructions/copy.rs`, `crates/core/src/agents/claude/library.rs`, `crates/core/src/agents/antigravity/library.rs`, `crates/core/src/agents/qwen/library.rs`, `crates/core/src/agents/copilot/library.rs`
- Modify: each of those adapters' `mod.rs` (`pub(crate) mod library;`)

**Interfaces:**
- Consumes: `Doc` (Task 2).
- Produces in `instructions::roots`: `pub struct Roots { pub claude: PathBuf, pub codex: PathBuf, pub gemini: PathBuf, pub qwen: PathBuf, pub copilot: PathBuf, pub home: PathBuf }` with `Roots::under(home: &Path) -> Roots` (each CLI's default folder under `home`: `.claude`, `.codex`, `.gemini`, `.qwen`, `.copilot`), `Roots::from_env(env: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> Roots` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `qwen::home`, `copilot::home`; `.gemini` under the home) and `Roots::from_ext(r: &ExtRoots) -> Roots` (`doctor/ext.rs:15`). `home` is the level's user home, which paths are shown under as `~/...`.
- Produces in `instructions::copy`:
  - `pub struct Marker { pub name: String, pub hash: String, pub source: String }`
  - `pub fn marker_line(name: &str, hash: &str, source: &str) -> String`
  - `pub fn parse_marker(line: &str) -> Option<Marker>` (also reads the `marshell:begin doc` form; `parse_end(line) -> Option<String>` reads `<!-- marshell:end doc <name> -->`)
  - `pub fn normalized(text: &str) -> String` (BOM dropped, `\r\n` → `\n`)
  - `pub fn payload_hash(payload: &str) -> String` (12 hex of SHA-256 of `normalized(payload)`)
  - `pub fn file_copy(frontmatter: &str, name: &str, source: &str, body: &str) -> String` (frontmatter, then the marker line with the hash of `frontmatter + body`, then the body)
  - `pub enum Judged { InSync, OutOfDate, ChangedOutside, Taken }`
  - `pub fn judge_file(expected: &str, actual: &str, name: &str) -> Judged` (the marker is the first line, or the line after a leading frontmatter's closing `---`; a missing marker or one for another name, compared without case, is `Taken`)
  - `pub fn copy_body(actual: &str) -> Option<String>` (the text after the marker line, normalized; None without a marker)
  - `pub fn source_of(doc: &Doc, project: Option<&Path>, home: &Path) -> String`
- Produces in each `agents/<cli>/library.rs` (Claude Code, Antigravity CLI, Qwen Code, Copilot CLI):
  - `pub(crate) fn user_dir(roots: &Roots) -> PathBuf`
  - `pub(crate) fn project_dir(project: &Path) -> PathBuf`
  - `pub(crate) fn file_name(name: &str) -> String`
  - `pub(crate) fn frontmatter(globs: &[String]) -> String` ("" or a `---` block ending in `\n`)
  - Antigravity only: `pub(crate) const RULE_CAP: usize = 24_000;`

| CLI | `user_dir` | `project_dir` | `file_name("Security")` | `frontmatter([])` | `frontmatter(["src/**","tests/**"])` |
|---|---|---|---|---|---|
| claude | `<claude>/rules` | `.claude/rules` | `marshell-security.md` | `""` | `---\npaths:\n  - 'src/**'\n  - 'tests/**'\n---\n` |
| antigravity | `<gemini>/config/rules` | `.agents/rules` | `marshell-security.md` | `---\ntrigger: always_on\n---\n` | `---\ntrigger: glob\nglobs: '**/src/**, **/tests/**'\n---\n` |
| qwen | `<qwen>/rules` | `.qwen/rules` | `marshell-security.md` | `""` | as claude |
| copilot | `<copilot>/instructions` | `.github/instructions` | `marshell-security.instructions.md` | `""` | `---\napplyTo: 'src/**, tests/**'\n---\n` |

Antigravity's `**/` prefix is added only to a glob that doesn't already start with `**/`.

- [ ] **Step 1: Write the failing tests** (in `copy.rs` and each `library.rs`):
  - `marker_round_trips`: `marker_line("security", "0123456789ab", ".marshell/instructions/security.md")` == `"<!-- marshell:doc security 0123456789ab from .marshell/instructions/security.md; edit it in Marshell -->"`; `parse_marker` gives the three parts back.
  - `marker_rejects_other_comments`: `"<!-- note -->"`, a hash of 11 digits, and upper-case hex → None.
  - `payload_hash_ignores_crlf`: `payload_hash("a\r\nb\n") == payload_hash("\u{feff}a\nb\n")`, length 12.
  - `states`: with `expected = file_copy("", "security", "s", "Body\n")`: `judge_file(&expected, &expected, "security")` InSync; an older copy `file_copy("", "security", "s", "Old\n")` → OutOfDate; that old copy with `Old` edited to `Mine` and its marker kept → ChangedOutside; `"Body\n"` with no marker → Taken; a copy whose marker names `other` → Taken.
  - `crlf_copy_is_in_sync`: `judge_file(&expected, &expected.replace('\n', "\r\n"), "security")` → InSync.
  - `frontmatter_edit_is_changed_outside`: an Antigravity copy whose `trigger: always_on` was edited to `trigger: manual` → ChangedOutside.
  - `copy_body_takes_the_text_after_the_marker`: `copy_body(&file_copy("---\npaths:\n  - 'a'\n---\n", "x", "s", "Line\n"))` == `Some("Line\n")`.
  - One test per CLI module pinning its table row.
  - `roots_from_env`: `CODEX_HOME` set moves only `codex`; with nothing set every folder is under the user home.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 library` and `instructions::copy` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(core): library copies carry a marker and a hash, written in each CLI's rule format.`

---

### Task 4: Codex: the `AGENTS.md` block and the project's `developer_instructions`

Read `docs/research/codex-cli.md` and the research note's Codex section first.

**Files:**
- Create: `crates/core/src/agents/codex/library.rs`
- Modify: `crates/core/src/agents/codex/mod.rs`; `crates/core/src/agents/codex/instructions.rs:63` (make `trust_key` and `untrusted` `pub(crate)` if the new code needs them; nothing else changes)

**Interfaces:**
- Consumes: `marker_line`/`parse_marker`/`parse_end`/`payload_hash`/`Judged` (Task 3).
- Produces in `agents::codex::library`:
  - `pub(crate) struct Block { pub name: String, pub hash: String, pub source: String, pub body: String }`
  - `pub(crate) fn block_text(name: &str, source: &str, body: &str) -> String` (begin line with `payload_hash(body)`, the body, the end line)
  - `pub(crate) fn blocks_in(text: &str) -> Vec<Block>` (a begin with no matching end is skipped; text inside it is the person's)
  - `pub(crate) fn judge_block(expected_body: &str, found: &Block) -> Judged` (InSync when `normalized` bodies match; else OutOfDate when `payload_hash(found.body) == found.hash`; else ChangedOutside)
  - `pub(crate) fn with_blocks(text: &str, wanted: &[(String, String)], keep: &[String]) -> Option<String>` (`wanted`: (name, full `block_text`); every block not in `wanted` and not in `keep` is removed; `keep` names blocks left exactly as they are (changed outside, or a broken doc's); None: only whitespace is left, so the file goes)
  - `pub(crate) fn user_file(codex_home: &Path) -> PathBuf` (`AGENTS.override.md` when it exists and holds more than whitespace, else `AGENTS.md`)
  - `pub(crate) fn other_user_file(codex_home: &Path) -> PathBuf` (the one `user_file` didn't pick, whose blocks Review removes)
  - `pub(crate) fn project_config(project: &Path) -> PathBuf` (`<project>/.codex/config.toml`)
  - `pub(crate) fn sections_in(config_text: &str) -> Result<Vec<Block>, String>`
  - `pub(crate) fn with_sections(config_text: Option<&str>, wanted: &[(String, String)], keep: &[String]) -> Result<Option<String>, String>` (None: delete the file)
  - `pub(crate) enum Trust { Trusted, Untrusted, NoEntry }` and `pub(crate) fn project_trust(user_config: Option<&str>, project: &Path, git_root: Option<&Path>) -> Trust` (the entry for the folder, else its git root's, as `untrusted` at `instructions.rs:63` finds it)
  - `pub(crate) fn user_sets_developer_instructions(user_config: Option<&str>) -> bool`

Placement (decision 45): a new block or section goes after the person's text, separated by one blank line, after the existing Marshell blocks, new ones in name order. The section lives inside the `developer_instructions` string, written with `toml_edit::DocumentMut` and `value(String)`, which `toml_edit` 0.25 writes as a `"""` multi-line string. Errors (exact): "Codex's .codex/config.toml can't be read, so Marshell doesn't change it." and "developer_instructions in .codex/config.toml isn't text, so Marshell doesn't change it."

- [ ] **Step 1: Write the failing tests:**
  - `adds_a_block_after_the_persons_text`: `with_blocks("Mine\n", &[("security", block_text("security", "~/s.md", "Body\n"))], &[])` == `Some("Mine\n\n<!-- marshell:begin doc security <h> from ~/s.md; edit it in Marshell -->\nBody\n<!-- marshell:end doc security -->\n")` with `<h> = payload_hash("Body\n")`.
  - `replaces_a_block_in_place`: `"A\n\n<old security block>\n\nB\n"` updated → `"A\n\n<new block>\n\nB\n"`.
  - `removes_a_block_and_its_blank_line`: `"Mine\n\n<block>\n"` with nothing wanted → `Some("Mine\n")`; `"<block>\n"` → None.
  - `keeps_a_block_it_is_told_to_keep`: a changed-outside block in `keep` is byte-identical in the result.
  - `block_states`: a fresh block → InSync; its body edited → ChangedOutside; an older body with its own hash → OutOfDate.
  - `user_file_prefers_a_non_empty_override`: no override → `AGENTS.md`; override `"  \n"` → `AGENTS.md`; override `"x"` → `AGENTS.override.md`.
  - `section_lives_in_developer_instructions`: `with_sections(Some("model = \"o3\"\n"), &[security], &[])` → text that parses with `toml_edit`, keeps `model = "o3"`, holds `developer_instructions = """`, holds no `\n` escape, and whose string has the begin line and `Body`.
  - `keeps_the_persons_developer_text`: from `developer_instructions = "Be brief."` → the string starts `"Be brief.\n\n<!-- marshell:begin doc security "`; removing the section again gives back the string `"Be brief."`.
  - `last_section_out_removes_the_key_and_the_file`: a file holding only the key with one section, nothing wanted → `Ok(None)`; with `model = "o3"` as well → the key is gone and `model = "o3"` stays.
  - `new_file_is_created`: `with_sections(None, &[security], &[])` → `Ok(Some(text))` holding the section.
  - `bad_toml_is_refused`: `"x = ["` → the first error; `developer_instructions = 3` → the second.
  - `trust`: a user config with `[projects.'C:\\p'] trust_level = "trusted"` → Trusted for `C:\p`; `"untrusted"` → Untrusted; no entry → NoEntry; an entry only for the git root `C:\` with `git_root = Some("C:\\")` → that entry's state.
  - `user_developer_instructions`: `"developer_instructions = \"x\"\n"` → true; `""` and None → false.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 codex::library` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS, and `cargo test -p marshell-core -j 4 codex::` → PASS (the trust readers still pass).
- [ ] **Step 5: Commit** `feat(core): Codex gets library docs in a tagged AGENTS.md block and in the project's developer_instructions.`

---

### Task 5: The library planner: rows, Review, and every other change

**Files:**
- Create: `crates/core/src/instructions/library.rs`
- Modify: `crates/core/src/config/guard.rs:178-181` (admit `<marshell home>/instructions` as `agents` is admitted)

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces in `instructions::library`:
  - `pub enum Level { User { roots: Roots, distro: Option<String> }, Project { root: PathBuf, notes: Notes } }` and `pub struct Notes { pub codex: Option<String>, pub codex_user_warning: bool, pub qwen: Option<String> }` (decision 17's sentences, read by the caller)
  - `pub struct Place { pub level: Level, pub docs: Vec<Doc>, pub found: Vec<&'static str> }` (`docs`: the level's docs, user or project; `found`: the CLIs found where the level is)
  - `pub struct CopyAt { pub doc: String, pub cli: &'static str, pub path: PathBuf, pub state: CopyState }`
  - `pub struct LevelState { pub rows: Vec<(String, Vec<CopyRow>)>, pub update: Vec<FileEdit>, pub out_of_date: u32, pub changed_outside: u32, pub copies: Vec<CopyAt> }` (`rows` per doc name in doc order; `update`: Review's edits at this level; `copies`: every copy found or wanted, for the Doctor and the copy fixes)
  - `pub fn level_state(place: &Place, user_home: &Path) -> LevelState` (`user_home`: the Windows user home, for the `~` of a user doc's `source`; rows show paths with `display` under `roots.home`, or relative to the project)
  - `pub fn review_plan(states: &[LevelState]) -> ChangeSet` (title "Update the copies", reason "Write each library doc's copies as the doc is now.")
  - `pub fn save_plan(doc: &Doc, old: Option<&Doc>, expected: Option<&str>) -> ChangeSet` (title "Save <name>": `CreateFile` or `ReplaceText` of the doc with `expected_hash`, plus `DeleteFile` of the old file on a rename)
  - `pub fn delete_plan(places: &[Place], name: &str, user_home: &Path) -> ChangeSet` (title "Delete <name> and its copies": the doc's `DeleteFile` once, and in each place the copy edits `level_state` makes with the doc left out, for that doc's copies only; a user doc's places are Windows and every running distro)
  - `pub fn copy_fix_plan(place: &Place, path: &Path, take_copy: bool, user_home: &Path) -> Option<ChangeSet>` (None when `path` isn't a changed-outside copy now; put back: the copy's edit alone, title "Put the doc's text back in <display path>"; take: the doc's `ReplaceText` with the copy's body (`copy_body`, or the block's body for Codex) and that copy's edit rendered from the new doc, title "Use the text of <display path> in <name>")
  - `pub fn uninstall_plan(roots: &Roots) -> ChangeSet` (title "Remove the instruction library's copies": `DeleteFile` of every `marshell-*.md` / `marshell-*.instructions.md` in the four user folders whose marker parses and names the file's own name, and Codex's two user files with every block removed; empty `edits` when there is nothing)

Rules `level_state` follows (decisions 13, 14, 27, 40–43):
- A doc with `error`, or a project doc with `enabled == false`, wants no copies. A broken doc's found copies go in `keep` (Codex) and get no edit; its rows read "The doc's frontmatter can't be read, so its copies are left as they are." An Off doc's copies are `Extra` and removed.
- A user doc wants a copy in each ticked CLI in `found`; a ticked CLI not found gets a `NoCopy` row with "Not installed on this PC, so it gets no copy." A project doc wants one in each ticked CLI; one not in `found` gets the note "Not installed on this PC. The copy is for teammates who use it."
- A doc with globs wants no Codex copy; Codex's row is `NoCopy` with "Codex has no globs, so this doc doesn't load in Codex."
- In a project, a doc for Claude Code serves Copilot CLI too: Copilot CLI's row has `shared: true`, the Claude Code copy's path and state, and the note "Copilot CLI reads .claude/rules, so it loads this too."; a `.github/instructions` copy of that doc is `Extra`. At user level each has its own copy.
- An unticked CLI's row is `NoCopy` with "Not ticked for this doc."; a copy left there is `Extra`.
- States per wanted copy: no file → `Missing`; `judge_file`/`judge_block` → `InSync`, `OutOfDate`, `ChangedOutside` or `Taken` ("<display path> isn't a copy Marshell wrote, so Marshell leaves it."). A Codex project file that `with_sections` refuses → `Refused` with its error.
- Every `marshell-*` file in a target folder (and every Codex block or section) whose marker parses and that no doc wants is `Extra`. It is counted, and Review removes it.
- `out_of_date` counts `Missing`, `OutOfDate` and `Extra` copies, a shared copy once; `changed_outside` counts `ChangedOutside`. `ChangedOutside` and `Taken` copies get no edit.
- Antigravity's copy over `RULE_CAP` bytes gets the note "Antigravity CLI cuts a rule past 24,000 bytes, and this copy is <n> bytes." (with a thousands comma); a doc with globs gets "Antigravity CLI matches globs anywhere in the path, so src/** also matches a deeper folder named src." (with its first glob in place of `src/**`).
- At project level the Codex row gets `notes.codex`, and with `codex_user_warning` also "Your developer_instructions in ~/.codex/config.toml stop loading in this project, because the project sets its own."; the Qwen row gets `notes.qwen`.
- Every edit carries the file's current `hash_file` as `expected_hash` (None for a new file), scope `User` or `Project`, `applies: NewSessions`.

- [ ] **Step 1: Write the failing tests** (`library.rs`, temp folders; docs built with `doc::parse`):
  - `new_project_doc_wants_four_files`: one doc, all five CLIs, no globs, empty project → `update` creates `.claude/rules/marshell-security.md`, `.agents/rules/marshell-security.md`, `.qwen/rules/marshell-security.md`, `.codex/config.toml`, and nothing under `.github`; `out_of_date == 4`; Copilot CLI's row `shared`.
  - `copilot_alone_gets_github_instructions`: clis `[copilot]` → `.github/instructions/marshell-security.instructions.md`.
  - `github_copy_goes_once_claude_is_ticked`: a marked `.github/instructions` copy and clis `[claude, copilot]` → its `DeleteFile` in `update`.
  - `applied_review_is_in_sync`: write each `update` edit's content to disk, run `level_state` again → every row `InSync`, `out_of_date == 0`, `update` empty.
  - `crlf_checkout_is_in_sync`: as above, then rewrite every copy with `\r\n` → still `InSync`.
  - `doc_change_makes_copies_out_of_date`: apply, change the body → four `OutOfDate`, `out_of_date == 4`.
  - `off_doc_removes_its_copies`: apply, set `enabled: false` → four removals (`DeleteFile` for the rule files, Codex's file deleted as it holds nothing else).
  - `orphan_copy_is_removed`: a marked `.qwen/rules/marshell-gone.md` and no doc `gone` → its `DeleteFile`, counted.
  - `broken_doc_keeps_its_copies`: apply, then make the doc's frontmatter unclosed → `update` empty, rows say the frontmatter sentence.
  - `taken_name_is_left_alone`: an unmarked `.claude/rules/marshell-security.md` holding `"Mine\n"` → row `Taken`, no edit touches it.
  - `changed_outside_is_not_in_review`: apply, edit the Qwen copy's body → that row `ChangedOutside`, `changed_outside == 1`, no edit for it.
  - `codex_skips_a_doc_with_globs`, `user_doc_skips_a_cli_not_found`, `project_doc_notes_teammates`, `agy_size_warning` (a 24,001-byte body).
  - `user_level_copies`: a `Place` with `Level::User` under a temp home and `found = ["claude", "codex", "copilot"]` → creates `<claude>/rules/marshell-security.md`, a block in `<codex>/AGENTS.md`, `<copilot>/instructions/marshell-security.instructions.md`; no `.gemini` or `.qwen` path.
  - `codex_override_takes_the_blocks`: a non-empty `AGENTS.override.md` → the block goes there, and a block in `AGENTS.md` is removed.
  - `delete_plan_holds_doc_and_copies`, `copy_fix_put_back`, `copy_fix_take_copy` (after applying it the copy is `InSync` and the other copies `OutOfDate`), `copy_fix_of_an_in_sync_copy_is_none`.
  - `save_plan_renames`: old `security.md`, new name `secrets` → `CreateFile` of `secrets.md` and `DeleteFile` of `security.md`.
  - `uninstall_plan_removes_marked_copies_only`: marked copies in four folders, an unmarked `marshell-mine.md`, and `AGENTS.md` holding `"Mine\n\n<block>\n"` → four `DeleteFile`s and `AGENTS.md` back to `"Mine\n"`; `marshell-mine.md` untouched.
  - In `guard.rs` tests: `instructions_folder_is_admitted`: `check(home/instructions/a.md)` is Ok; `check(home/settings.json)` still `MarshellOwn`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 instructions::` and `config::guard` → FAIL.
- [ ] **Step 3: Implement** `library.rs` and the guard line.
- [ ] **Step 4: Run** both → PASS.
- [ ] **Step 5: Commit** `feat(core): the library plans every copy, its review, delete, copy fixes and uninstall as config changes.`

---

### Task 6: The `/v1/instructions` routes

**Files:**
- Create: `crates/core/src/server/instructions.rs`
- Modify: `crates/core/src/server/mod.rs` (routes next to `/v1/doctor/*` at `:1576-1583`; `mod instructions;`), `crates/core/src/server/agents.rs:1328` (pull the body into `pub(crate) fn found_agents(st: &AppState) -> Vec<&'static str>`, blocking, and have `found` call it), `crates/core/src/server/doctor.rs:405-447` (make `partition_managed_distros` and `distro_facts` `pub(crate)`)

**Interfaces:**
- Consumes: Task 5; `doctor_ext::windows_roots` (`server/doctor_ext.rs:41`); `projects::entries`; `wsl::running_recent`; the config service on `AppState`.
- Produces:
  - `GET /v1/instructions?project=<root>` → `LibraryView`
  - `POST /v1/instructions/save` `DocSave` → `DocSaved`
  - `POST /v1/instructions/review` `LibraryReview` → `ConfigPreview`
  - `POST /v1/instructions/delete` `DocRef` → `ConfigPreview`
  - `POST /v1/instructions/copy` `CopyFix` → `ConfigPreview`
  - `pub(crate) fn user_places(st: &AppState) -> (Vec<Place>, Vec<String>)` (the Doctor reuses it): the Windows user level from `windows_roots` with `found_agents`, then each running managed distro's user level (`distro_facts`, which also admits the distro's home to the config service through `distro_target`, `server/agents.rs:465`; its `found` = the facts that are `found` with no `error`); the second list is the stopped managed distros.
  - `pub(crate) fn project_place(st: &AppState, root: &Path) -> Result<Place, String>` (Err: decision 47's sentence)
  - The view: each user doc's rows are its rows from every user place in that order, then one `Waiting` row per stopped distro, with "<distro> isn't running, so its copies wait."; each project doc's rows come from the project place. `out_of_date` and `changed_outside` add up every place shown.

Behaviour:
- Every handler runs its file work in `spawn_blocking`.
- `project` must be a root in `projects::entries`, else 404 "Marshell doesn't know this project.". A project whose path is under a distro's share (`wsl::distro_of_share`-style test, as `uninstall.rs:37` does) and whose distro isn't in `running_recent` reads nothing: the view has `project_note` "<distro> isn't running, so this project's docs can't be read." and no project docs (decision 47).
- User docs are read from `<st.0.home>/instructions`, project docs from `<root>/.marshell/instructions`.
- Trust notes: Codex from `<codex home>/config.toml` via `project_trust` (`NoEntry` → "Codex loads it once you trust this project in Codex.", `Untrusted` → "Codex doesn't trust this project, so it loads none of its instructions."); Qwen Code when `folder_trust_on` and not `folder_trusted` → "Qwen Code loads it once you trust this folder in Qwen Code.". A distro project reads both from the distro's home when it is known, else shows none.
- `save`: validates with `check_name` (against the scope's other docs, the old name left out), `check_globs`, `check_description`, known CLI ids; a project scope without `project` → 422 "A project doc needs a project."; a rename that differs only in case → 422 "A new name must differ by more than case."; a new doc whose file exists → 409. Validation errors are 422 with the message as the body. Then `save_plan` → `preview_own` → `apply` with the preview's hashes; an apply conflict answers as `/v1/config/apply` answers one (409 with its body, which `conflictBody` reads). Answers `DocSaved` with the new view.
- `review` plans every user place and the project's; `delete` of a user doc plans every user place, of a project doc the project's; `copy` finds the place whose `copies` hold the path. Each answers `preview_own`'s `ConfigPreview` (the page applies it with `/v1/config/apply` and undoes it with `/v1/snapshots/{id}/undo`). `copy` with a path that isn't a changed-outside copy now → 409 "This copy changed. Check again."

- [ ] **Step 1: Write the failing tests** (in `server/instructions.rs`, an `AppState` with a temp Marshell home, `with_ext_home`, one project in `projects.json`, `with_running_distros(|| vec![])`):
  - `view_lists_user_and_project_docs`: a user doc and a project doc on disk → `user_docs.len() == 1`, `project_docs.len() == 1`, rows in CLI order.
  - `unknown_project_is_404`.
  - `save_creates_the_folder_and_the_doc`: a `DocSave` with no `expected_hash` → `<home>/instructions/security.md` exists with the rendered text; the view's `out_of_date` > 0; `managed.json` has no entry for it.
  - `save_with_a_stale_hash_conflicts` → 409.
  - `save_refuses_a_bad_name` → 422 "A name holds letters, digits, - and _ only.".
  - `review_then_apply_is_in_sync`: `review` → apply the preview through the config service → the view's `out_of_date == 0`.
  - `review_records_nothing_in_managed_json`.
  - `delete_preview_holds_the_doc_and_copies`.
  - `copy_fix_of_an_in_sync_copy_conflicts` → 409.
  - `stopped_distro_project_reads_nothing`: a project at `\\wsl.localhost\Ubuntu\home\u\repo` with no running distro → 200, `project_note` "Ubuntu isn't running, so this project's docs can't be read.", no read of the path (pin with a path that would panic or fail if read; the test asserts `project_docs` is empty and the note).
  - `stopped_managed_distro_gets_a_waiting_row`: `managed.json` names a distro path, `with_running_distros` returns none → each user doc has a row `{cli: None, distro: "Ubuntu", state: Waiting}`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 server::instructions` → FAIL.
- [ ] **Step 3: Implement** the routes and the two small extractions.
- [ ] **Step 4: Run** → PASS, and `cargo test -p marshell-core -j 4 server::agents` → PASS.
- [ ] **Step 5: Commit** `feat(core): the local API serves the instruction library, its saves, review and copy fixes.`

---

### Task 7: The Doctor's library items and fixes

**Files:**
- Create: `crates/core/src/doctor/library.rs`
- Modify: `crates/core/src/doctor/mod.rs`; `crates/core/src/server/doctor.rs` (the gather at `:244-272`, `fix_of` at `:1018`, the keep route's refusal of `UseCopy` from Task 1)

**Interfaces:**
- Consumes: `user_places`, `project_place` (Task 6), `level_state`, `copy_fix_plan`, `review_plan` (Task 5), `display`, `hash_part`, `id_tail`, `folder_name` (`doctor/instructions.rs:1047-1227`).
- Produces: `pub fn items(levels: &[(&Place, &LevelState)]) -> Vec<DoctorItem>` (the project and distro come from `place.level`).

Items (decisions 25, 32, 34, 50):
- Each `ChangedOutside` copy: id `libdrift.<hash_part(path)>`, group `Broken`, `agent` the CLI, `distro`, `path`, title "<display path> changed outside Marshell", detail "<CLI name> loads text the doc <name> doesn't hold. The fix puts the doc's text back. Use this copy's text takes the copy's text into the doc instead: the text, not the frontmatter.", `fixable: true`, `unchecked: true`, `action: Some(UseCopy)`.
- Each level with `out_of_date > 0`: id `libstale.<place><suffix>` (`id_tail`), group `Info`, title by count: one doc → "<n> copies of <NAME> are out of date in <project folder>" ("1 copy of <NAME> is out of date …"); several docs → "<n> copies of <m> docs are out of date in <project folder>"; at user level "… at user level", in a distro "… at user level in <distro>". `<NAME>` is the doc's name as written. Detail "The CLIs load what the copies held before the docs changed. The fix writes them as Review on the Instructions page does.", `fixable: true`, `unchecked: true`.
- `fix_of`: an id starting `libdrift.` plans `copy_fix_plan(.., take_copy: false)`; `libadopt.<hash>` plans `take_copy: true` and is what the row's `UseCopy` button sends; `libstale.` plans `review_plan` for that level. These edits go in the fix's preview through `preview_mixed` with `own` true for them, so they record nothing in `managed.json`. An id whose copy or level is gone → the existing `CANT_FIX` refusal.

- [ ] **Step 1: Write the failing tests:**
  - In `doctor/library.rs`: `drift_item`: a `LevelState` with one ChangedOutside Qwen copy → one item with id `libdrift.<8 hex>`, group Broken, title `".qwen/rules/marshell-security.md changed outside Marshell"`, `unchecked`, `action == Some(UseCopy)`.
  - `stale_titles`: 4 copies of one doc in project `C:\p\Airport` → `"4 copies of SECURITY are out of date in Airport"`; 1 → `"1 copy of SECURITY is out of date in Airport"`; 6 of 2 docs → `"6 copies of 2 docs are out of date in Airport"`; user level → `"… at user level"`; distro → `"… at user level in Ubuntu"`.
  - In `server/doctor.rs` tests: `library_fix_puts_the_doc_back` (a drift item's fix preview rewrites the copy only), `use_copy_fix_writes_doc_and_copy` (`libadopt.` id), `stale_fix_updates_the_level`, `library_fix_records_nothing_in_managed_json`, `gone_copy_is_cant_fix`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 doctor::library` and `server::doctor::tests::library` → FAIL.
- [ ] **Step 3: Implement**; the gather reads `user_places` and `project_place` for each of `facts.projects` (a project that gives Err is skipped), inside its existing blocking section, and `fix_of` reads them again to find an id's place.
- [ ] **Step 4: Run** → PASS, and the whole `cargo test -p marshell-core -j 4 doctor` → PASS.
- [ ] **Step 5: Commit** `feat(doctor): copies edited outside Marshell and copies out of date are Doctor items with their fixes.`

---

### Task 8: `marshell uninstall` removes the user-level copies

**Files:**
- Modify: `crates/cli/src/uninstall.rs` (after Copilot CLI's plan, `:390`, and in the distro loop, `:441`)

**Interfaces:**
- Consumes: `instructions::roots::Roots::from_env`, `Roots::under`, `instructions::library::uninstall_plan` (Task 5).

Behaviour (decision 31): on Windows `uninstall_plan(&Roots::from_env(env, user_home))`; in each answering distro `uninstall_plan(&Roots::under(distro_home))`, previewed on its own first and reported as the hooks are. A non-empty plan goes into `extras`. Output lines (exact): "Instruction library copies found." (Windows) and "<distro>: instruction library copies found."; and, when `<home>/instructions` holds a `.md` file, "Your library docs stay in <home>/instructions." after the removal.

- [ ] **Step 1: Write the failing tests** (the file's existing harness of temp homes, `Wsl` stubs and `yes`):
  - `uninstall_removes_library_copies`: a marked `~/.claude/rules/marshell-security.md` and a block in `~/.codex/AGENTS.md` after `"Mine\n"` → after `run(.., yes: true)` the copy is gone, `AGENTS.md` is `"Mine\n"`, and the output holds "Instruction library copies found.".
  - `uninstall_keeps_files_without_a_marker`: an unmarked `marshell-mine.md` stays.
  - `uninstall_keeps_the_docs`: `<home>/instructions/security.md` stays, and the output holds "Your library docs stay in".
  - `uninstall_removes_distro_copies`: a distro home stub with a marked Qwen copy → gone.
- [ ] **Step 2: Run** `cargo test -p marshell-cli -j 4 uninstall` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(cli): uninstall removes the instruction library's user-level copies and keeps the docs.`

---

### Task 9: The page's client and model

**Files:**
- Create: `src/lib/api/instructions.ts`, `src/lib/api/instructions.test.ts`, `src/features/instructions/instructionsModel.ts`, `src/features/instructions/instructionsModel.test.ts`

**Interfaces:**
- Consumes: the generated types (Task 1); `api`, `ApiError` (`src/lib/api/client.ts`); `applyPreview`, `undoSnapshot`, `conflictBody` (`src/lib/api/agents.ts:43-83`); `CLI_NAME` (`src/features/onboarding/consent.ts`).
- Produces in `src/lib/api/instructions.ts`:
  - `libraryView(ep: Endpoint, project: string | null): Promise<LibraryView>`
  - `type SaveOutcome = { ok: true; saved: DocSaved } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string }` and `saveDoc(ep: Endpoint, req: DocSave): Promise<SaveOutcome>` (409 → conflict; 422 → its message)
  - `reviewCopies(ep, project: string | null): Promise<ConfigPreview>`, `previewDelete(ep, ref: DocRef)`, `previewCopyFix(ep, fix: CopyFix)`
- Produces in `instructionsModel.ts`:
  - `type Draft = { scope: DocScope; name: string; oldName: string | null; description: string; clis: string[]; globsText: string; enabled: boolean; body: string; hash: string | null }`
  - `draftOf(doc: LibraryDoc): Draft`, `newDraft(scope: DocScope, found: string[]): Draft` (user: the found CLIs; project: all five, decision 10), `isDirty(d: Draft, doc: LibraryDoc | null): boolean`, `toSave(d: Draft, project: string | null): DocSave`
  - `toggleCli(d: Draft, cli: string): Draft` (decision 51: in a project doc, ticking `claude` ticks `copilot`, and `copilot` can't be unticked while `claude` is)
  - `copilotLocked(d: Draft): boolean`
  - `barText(outOfDate: number, changed: number): string | null`
  - `stateText(row: CopyRow): string`
  - `rowLabel(row: CopyRow): string` (the CLI name, with " in <distro>" for a distro row, or the distro alone for a Waiting row)
  - `copyFixes(row: CopyRow): boolean` (true only for `changed_outside`)

Copy (exact):
- `barText`: `(0,0)` → null; `(1,0)` → "1 copy is out of date"; `(4,0)` → "4 copies are out of date"; `(4,1)` → "4 copies are out of date. 1 copy was changed outside Marshell; its row has the fixes."; `(0,2)` → "2 copies were changed outside Marshell; their rows have the fixes."
- `stateText`: in_sync "Up to date", out_of_date "Out of date", missing "No copy yet", changed_outside "Changed outside Marshell", extra "Review removes this copy", taken "Not Marshell's file", no_copy "No copy", waiting "Waiting for the distro", refused "Marshell can't write this copy".

- [ ] **Step 1: Write the failing tests:** each `barText` case; each `stateText`; `toggleCli` on a project draft (`claude` on → `copilot` on; `copilot` off while `claude` on → unchanged) and on a user draft (independent); `newDraft("user", ["claude","qwen"]).clis` == `["claude","qwen"]`; `newDraft("project", []).clis.length` == 5; `isDirty` false right after `draftOf`, true after a body edit; `toSave` splits `globsText` by lines and trims; `saveDoc` maps a 409 `ApiError` to `{ ok: false, conflict: true }` and a 422 to its message (with a stubbed `api`, as `src/lib/api/doctor.test.ts` stubs it).
- [ ] **Step 2: Run** `pnpm test src/features/instructions/instructionsModel.test.ts src/lib/api/instructions.test.ts` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(instructions): the library page's client and model.`

---

### Task 10: The page's components

Read the `good-css` skill before the CSS; follow `src/features/doctor/doctor.css` for the page frame and tokens from `src/styles/`.

**Files:**
- Create: `src/features/instructions/InstructionsPage.tsx`, `DocList.tsx`, `DocForm.tsx`, `LoadsIn.tsx`, `CopiesReview.tsx`, `instructions.css`, `InstructionsPage.test.tsx`
- Modify: `src/features/doctor/FixSheet.tsx:11-34` (export `FileDiffs`, unchanged otherwise)

**Interfaces:**
- Consumes: Task 9; `FileDiffs`; `Editor` (`src/features/editor/Editor.tsx:101`) with `EditorPrefs`.
- Produces (presentational; every handler a prop):
  - `InstructionsPage({ view, projects, project, open, draft, errors, busy, status, prefs, appChord, onPickProject, onOpen, onNew, onDraft, onSave, onDelete, onReview, onCopyFix, onCheckAgain, onClose, editorRef })` where `open: { scope: DocScope; name: string } | null`, `errors: Partial<Record<"name" | "description" | "globs" | "save", string>>`, `status: { text: string; undo?: () => void } | null`
  - `CopiesReview({ title, preview, busy, conflict, onApply, onCancel, onReload })` (role `dialog`, `aria-modal="true"`, calls `useCoverPage()` and `useDialogFocus`; Apply has the focus)

Layout and copy (decisions 3, 6, 11, 19, 20, 26):
- Root: `<section role="region" aria-labelledby=…>` with heading "Instructions", a "Check again" button, and a close button labelled "Close Instructions".
- The bar under the heading when `barText` gives text: the text, then "Review". Status after an apply: "Copies updated." with an "Undo" button; after an undo: "Put back.".
- Left list: heading "Every project", its docs as buttons (the open one `aria-current="true"`), "No docs yet." when empty, and "New doc for every project". Then a `<select aria-label="Project">` of the known projects (as `ProjectsView` in Settings → Projects), the project's docs, and "New doc for this project". With no project: "No project yet. Open a session in a folder, or add one in Settings → Projects." With `project_note`: that note.
- Form: "Name" (text input), "Description" (text input), "CLIs" (fieldset of five checkboxes with `CLI_NAME`; a locked Copilot CLI box is `disabled` with the hint "Copilot CLI reads .claude/rules, so it loads this too."), "Globs" (textarea, hint "One per line, relative to the project. Empty: the doc always loads."), "On in this project" (checkbox, project docs only), "Text" (the `Editor` with `name` = `<name>.md`). Each error is under its field, tied with `aria-describedby`. A doc with `error` shows it above the form.
- Buttons: "Save" (hint "Ctrl+S"), "Delete…".
- "Loads in": a list, one row per `CopyRow`: `rowLabel`, `stateText`, the path, each note on its own line; a `changed_outside` row has "Put the doc's text back" and "Use this copy's text" with the hint "Takes the text, not the frontmatter.".
- Review sheet titles: "Update the copies", "Delete <name> and its copies", "Put the doc's text back", "Use this copy's text". Buttons "Apply", "Cancel". Conflict: "A file changed since this preview. Reload it to see the change." with "Reload". A preview with no file: "Nothing to update: the copies are up to date." and only "Close".

- [ ] **Step 1: Write the failing tests** (`renderToStaticMarkup`, as `src/features/doctor/renderReview.test.tsx` does): the bar's text and Review button for `out_of_date: 4`; no bar for `0, 0`; "No docs yet." for an empty user list; the no-project sentence; a `changed_outside` row renders both fix buttons and a `in_sync` row none; a shared Copilot CLI row shows its note; a project draft with `claude` ticked renders the Copilot CLI box `disabled` and checked; a user draft renders no "On in this project"; a name error renders with `aria-describedby` pointing at it; `CopiesReview` renders `aria-modal="true"` and its title.
- [ ] **Step 2: Run** `pnpm test src/features/instructions/InstructionsPage.test.tsx` → FAIL.
- [ ] **Step 3: Implement** the components and CSS (forced colors: borders and focus rings use system colours; no state shown by colour alone).
- [ ] **Step 4: Run** the test and `pnpm test src/features/browser/coverScan.test.ts` → PASS.
- [ ] **Step 5: Commit** `feat(instructions): the Instructions page's list, form, Loads in rows and review sheet.`

---

### Task 11: The live page, its entries, and the Doctor's button

**Files:**
- Create: `src/features/instructions/LiveInstructions.tsx`, `src/features/instructions/liveInstructions.ts` (its pure reducer), `liveInstructions.test.ts`
- Modify: `src/features/sessions/LiveApp.tsx` (`:341` overlay union gains `"instructions"`; `:2215` the palette; `:3034` the app menu; mount next to the Doctor at `:3092`), `src/features/appMenu/appMenu.ts:7,21-26` (`AppMenuRun` gains `"instructions"`; item `{ id: "instructions", label: "Instructions" }` after Doctor), `src/features/appMenu/appMenu.test.ts:6-8`, `src/lib/keymap.ts:162-166` (`{ id: "instructions", label: "Open Instructions", group: "App" }` after `doctor`; `"instructions"` in `MOVED_COMMANDS`), `src/features/sessions/LivePalette.tsx:13` (`"instructions"` in `PALETTE_ONLY`), `src/features/doctor/keep.ts` (labels for `use_copy`), `src/features/doctor/LiveDoctor.tsx:134-149` (`use_copy` opens the fix sheet)

**Interfaces:**
- Consumes: Tasks 9–10; `applyPreview`, `undoSnapshot`; `SaveDialog` (`src/features/editor/SaveDialog.tsx`); `useDialogFocus`, `useCoverPage`; `editorPrefsOf` (imported in `LiveApp.tsx:148`); `pickProject` (`projectsDoc.ts:74`) and `GET /v1/projects`.
- Produces: `LiveInstructions({ endpoint, cwd, prefs, announce, onClose })` (`cwd`: the focused session's folder, else the shell panel's, as `focusedCwd` at `LiveApp.tsx:1574` gives it).

Behaviour:
- The project shown first is `pickProject(projects, cwd)`; the picker changes it (decision 11).
- Ctrl+S (`appChord` true for it, so the editor lets it through) saves; a save's 409 shows "<name> changed on disk since you opened it." with "Reload" and "Save over it" (the latter saves with the disk's current hash from a fresh view). Validation messages go under their fields and are announced.
- It calls `useCoverPage()`, as `LiveDoctor` does, since it lies over the side pane. Esc closes, as the Doctor (`LiveDoctor.tsx:60-62`), unless a sheet or dialog is open. Closing or opening another doc with unsaved edits shows `SaveDialog` named after the doc (decision 48): Save saves then goes on, Don't save drops the draft, Cancel stays.
- Review, Delete… and the two copy fixes fetch their preview, show `CopiesReview`, apply with `applyPreview`, then show the Undo status; Undo calls `undoSnapshot`; each step reads the view again (decision 52).
- Announcements (exact): after a save "Saved <name>."; after an apply "Copies updated."; after an undo "Put back."; a failed fetch "Couldn't read the instruction library: <message>".
- App menu "Instructions" and palette "Open Instructions" open the page; it closes to the terminal as the Doctor does (`setOverlay(null)` then `focusTerminal`).
- Doctor: `keepLabel("use_copy", busy)` is "Use this copy's text" / "Preparing…"; pressing it calls `planFix(endpoint, ["libadopt." + item.id.slice("libdrift.".length)])` and opens the existing fix sheet, so apply and Undo go through the Doctor's own routes.

- [ ] **Step 1: Write the failing tests:** `appMenu.test.ts` labels become `["Settings","History","Doctor","Instructions","Departures"]`; in `liveInstructions.test.ts` the reducer: opening a doc while dirty asks (`state.asking` set) and doesn't switch; `discard` switches; a save conflict sets the conflict line; an apply sets the Undo status; `keep.test.ts` (or the file that tests `keepLabel`) gets the `use_copy` labels; the palette's `LIVE_COMMANDS` holds `instructions`.
- [ ] **Step 2: Run** `pnpm test src/features/appMenu src/features/instructions src/features/doctor src/features/sessions` → FAIL on the new cases.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `pnpm test` → PASS; `pnpm build` → no type errors.
- [ ] **Step 5: Commit** `feat(instructions): the Instructions page opens from the app menu and the palette, and the Doctor offers Use this copy's text.`

---

### Task 12: The live check (decision 30)

Run it with the `marshell-live-check` skill, against scratch homes only (`MARSHELL_HOME`, `MARSHELL_DEFAULT_FOLDER`, `MARSHELL_CLAUDE_SETTINGS`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `COPILOT_HOME`, and `USERPROFILE` for Antigravity CLI and Qwen Code). Note agy's version before and after. Never use `/model` or `/effort`. Findings go in `docs/research/2026-11-27-instruction-library-wiring.md` under a new "Live check (slice 8a)" heading.

- [ ] **Step 1:** In a fresh scratch project (added in Settings → Projects), make the user doc `GREETING` (text: a unique marker sentence) and the project doc `SECURITY` (another marker, globs empty), Review, Apply. Check with `browser_evaluate` that the bar is gone and every row reads "Up to date".
- [ ] **Step 2: Codex:** `codex debug prompt-input` in the project, before and after trusting it in the scratch `config.toml` → both markers only once trusted; the `GREETING` block in the scratch `AGENTS.md`.
- [ ] **Step 3: Copilot CLI:** `copilot instruction list --json` lists `.claude/rules/marshell-security.md` and the user copy; one turn on the fake model (`-CopilotFake`) whose logged request holds both markers.
- [ ] **Step 4: Qwen Code:** one turn on the fake model (`-QwenFake`) with the folder trusted; the logged request holds both markers.
- [ ] **Step 5: Antigravity CLI:** `/context` in a session (sign-in copied as in 6g decision 3) lists both rules.
- [ ] **Step 6: Claude Code:** the user signs in once to the scratch `CLAUDE_CONFIG_DIR` at the PC (decision 9); `/memory` and `/context` list both.
- [ ] **Step 7: Drift:** edit the Qwen copy by hand → its row and a Doctor item read "Changed outside Marshell"; "Use this copy's text" → the doc holds the edit, the other copies are out of date, the Doctor's out-of-date item appears; Undo puts both back.
- [ ] **Step 8: Globs:** give `SECURITY` the glob `src/**`, Review → Codex's row reads "Codex has no globs, so this doc doesn't load in Codex." and the Codex section is removed.
- [ ] **Step 9: WSL:** with a distro running that Marshell set a CLI up in, a user doc's copy lands in the distro home and one project in the distro gets its copies; with the distro stopped, the row reads "<distro> isn't running, so its copies wait." and nothing starts it (`wsl -l --running` before and after).
- [ ] **Step 10: Keyboard and forced colours:** the whole of steps 1 and 7 by keyboard alone; a screenshot in forced colours to judge the look.
- [ ] **Step 11:** Stop only what the check started, by PID, after listing the tree for any CLI the user started. Commit the research note: `docs(research): slice 8a's live check of the instruction library in all five CLIs.`

---

### Task 13: Ship

- [ ] **Step 1:** The `marshell-reviewer` agent (Opus) reviews the whole branch with this plan's Review Focus; one fix pass for its findings.
- [ ] **Step 2:** `marshell-ship`: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`, each in the foreground; the PR with "What it does", "Live check", "Known limits" (the page doesn't watch files, decision 52; Claude Code's user `paths` are relative to the session's folder) and "Checks"; a green CI; `pre-merge-review`; merge.
