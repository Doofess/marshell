# Instruction checks in the Doctor (slice 7d2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor reports what is wrong inside the instruction files 7d1 lists (dead imports, paths that don't exist, duplicates, clashes), offers two fixes (remove a dead import, raise Codex's limit), reports the tokens Marshell's reference notes add, and opens a finding's file in the side pane's editor.

**Architecture:** Pure text checks live in a new `doctor/instr_text.rs`. A new `doctor/instr_checks.rs` turns the run's `CliLoad`s plus the run's memoized `Reader` into `Finding`s and then into `DoctorItem`s; it names no CLI's files. The server computes the findings inside `doctor_instr::loads`, while the reader is alive, and plans the two fixes as ordinary `FileEdit`s (`ReplaceText`, `TomlSet`) for the config service. Reference notes are counted in `references.rs` when the hook endpoint sends one.

**Tech Stack:** Rust (`marshell-core`, `marshell-protocol`, ts-rs), the `ignore` crate's gitignore matcher (already a dependency), React and TypeScript (Vitest with `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 7d2 decisions (2026-11-25)" (1 to 17) and "Slice 7d decisions (2026-11-24)" (2, 3, 6). Facts: `docs/research/2026-11-24-instruction-files.md` ("Import rules summary", "What 7d1's first task saw").

This plan was reviewed against the spec and the code before it ran (an Opus review, 2026-11-25); its 14 findings are worked in.

## Global Constraints

- No new dependency. `ignore::gitignore` and `toml_edit` are already in `marshell-core`.
- Nothing is written outside the config service. The two fixes are `FileEdit`s; every other item has `fixable: false`.
- Every CLI is treated alike: a check runs for each CLI whose load holds the file. What differs between CLIs (import syntax, whether HTML comments are read, Codex's limit and its copy) is stated by that CLI's own module, never by a CLI's name in shared code.
- A quoted line is at most 80 characters (cut with `…`), and is shown only in an item's `detail`. `instr_text.rs` and `instr_checks.rs` hold no `tracing::` call at all, since the logs are exported (decision 14).
- Token figures are `ext::tokens(chars)` (characters divided by 4, rounded up), worded with `tokens_text` and "about".
- Copy is exactly as written in the 7d2 decisions. Plurals: "a path" / "3 paths", "a paragraph" / "3 paragraphs", "1 reference note" / "14 reference notes".
- Item ids are stable across runs: they are built from `project_id`, `hash_part` and fixed words only.
- An item about files that are all at user level is one item with the place `user` and no project in its title (decision 10), however many projects' loads carry it.
- Rust tests run with `-j 4`. `cargo fmt --all` before every Rust commit. The rustfmt hook reformats on edit: read a file again after it says so.
- `src/generated/` is never edited by hand: `cargo test -p marshell-protocol -j 4` regenerates it, and it is committed.
- Commit messages read `type(scope): sentence.` and end with the two attribution lines this session uses.
- Workers: stay inside the files a task lists, never run `git checkout`, `git restore` or `git stash` on files you don't own, never start `pnpm tauri dev`, and write scratch files only under your own scratch subfolder.

## Review Focus

1. **A file with CRLF line ends, a BOM, non-ASCII text or no final newline** keeps every byte the fix doesn't remove. A person expects the diff to show one line. (Task 2's table, Task 8's CRLF test.)
2. **An import inside an HTML comment** is dead only for the CLIs that read comments (decision 17); when one of them reads the file, the fix removes that one too, or the item would come back with nothing left to fix. (Tasks 2 and 4.)
3. **Two dead imports ticked in one file, and one of them gone since the report** is a 422 for the whole fix, never half a change. (Task 8.)
4. **A paragraph one CLI gets through two routes** (a file loaded by name and as an import) is the "loads twice" item only, not also a "repeats" item. (Task 5.)
5. **A reference note sent while another session's note is being counted** loses neither count, and a counter file that can't be written never delays or drops the note. (Task 7.)

---

### Task 1: Imports carry their place, and prose `@words` stop being imports

**Files:**
- Modify: `crates/core/src/doctor/instructions.rs` (`LoadedFile`, `CliLoad`, `imports_in`, `Expander::imports_of`, `Line::id_tail`, the tests module)
- Modify: `crates/core/src/agents/claude/instructions.rs`, `agents/qwen/instructions.rs`, `agents/copilot/instructions.rs`, `agents/antigravity/instructions.rs`, `agents/codex/instructions.rs` (their `LoadedFile` and `CliLoad` literals only), and any other file with such a literal (`crates/core/src/server/doctor_instr.rs`, tests)

**Interfaces:**
- Produces:
  - `pub struct ImportAt { pub raw: String, pub line: usize, pub start: usize, pub end: usize }`: `line` is 1-based; `start..end` are **byte** offsets in that line and cover the whole written import (`@docs/a.md`, `@docs/my\ file.md`, `@[label](path)`).
  - `pub fn imports_at(text: &str, syntax: ImportSyntax) -> Vec<ImportAt>`; `imports_in` stays, as `imports_at(..)` mapped to `raw`.
  - `pub fn mask_html_comments(text: &str) -> String`: what `strip_html_comments` strips, replaced by spaces, one space for each byte, with every `\n` and `\r` kept. Same fence and unclosed-comment rules. The result has the text's byte length and line structure.
  - `pub fn looks_like_path(raw: &str) -> bool` (decision 3).
  - `LoadedFile` gains `pub raw: Option<String>`: the import as written, on an import row; `None` on every other row.
  - `pub struct ImportStyle { pub syntax: ImportSyntax, pub in_comments: bool }` and `CliLoad` gains `pub imports: Option<ImportStyle>`: Claude Code `At`/`false`, Qwen Code `At`/`false`, Copilot CLI `At`/`true`, Antigravity `AgyLink`/`true`, Codex `None`. `in_comments` is true exactly when the CLI's `ImportRules.clean` leaves comments in. `ImportSyntax` derives `Copy, PartialEq, Eq, Hash` if it doesn't.
  - `pub(crate) fn id_tail(project: Option<&Path>, distro: Option<&str>) -> (String, String)`, which `Line::id_tail` now calls; and `pub(crate)` on `is_fence`, `code_mask`, `cli_rank`, `folder_name`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests** in `instructions.rs`'s tests module:

```rust
#[test]
fn imports_at_gives_the_line_and_the_byte_span() {
    let text = "intro\n- @docs/a.md\nsee @b/c.md, and `@code.md`\ncafé @d/e.md\n";
    let got = imports_at(text, ImportSyntax::At);
    assert_eq!(got.len(), 3);
    assert_eq!((got[0].raw.as_str(), got[0].line, got[0].start, got[0].end), ("docs/a.md", 2, 2, 12));
    assert_eq!((got[1].raw.as_str(), got[1].line), ("b/c.md,", 3));
    assert_eq!((got[2].line, got[2].start, got[2].end), (4, 6, 13)); // "café " is 6 bytes
}

#[test]
fn an_agy_link_spans_the_whole_link() {
    let got = imports_at("x @[label](sub/inc.md) y", ImportSyntax::AgyLink);
    assert_eq!((got[0].raw.as_str(), got[0].start, got[0].end), ("sub/inc.md", 2, 22));
}

#[test]
fn masking_keeps_every_byte_and_line_where_it_was() {
    let text = "a\n<!-- café @x.md\n@y.md -->\r\n@z.md\n";
    let masked = mask_html_comments(text);
    assert_eq!(masked.len(), text.len());
    assert_eq!(masked.lines().count(), text.lines().count());
    assert_eq!(imports_at(&masked, ImportSyntax::At).iter().map(|i| i.line).collect::<Vec<_>>(), vec![4]);
}

#[test]
fn what_looks_like_a_path() {
    for yes in ["./a", "../a/b", "/etc/x", "~/notes.md", "C:/x/y.md", "AGENTS.md", "docs/setup.md", "a/b.rs", "b/c.md,", "notes.md).", "Cargo.lock", "a/b.weird1"] {
        assert!(looks_like_path(yes), "{yes}");
    }
    for no in ["someone", "types/node", "v2", "1.2", "team.", "org/repo", "john.doe", "example.com", "a/1.2"] {
        assert!(!looks_like_path(no), "{no}");
    }
}
```

and, with the module's own `write`, `rules()` and `expand` helpers, asserting each row's `path`, `when` and `raw`:

  - `a_prose_at_word_that_isnt_a_file_gets_no_row`: `CLAUDE.md` = `"@someone asked, see @types/node and @john.doe and @gone.md and @gone.md again"`. Rows: `CLAUDE.md`, then `gone.md` once (`NotLoaded("not found")`, `raw == Some("gone.md")`).
  - `an_at_word_whose_file_exists_is_still_an_import`: `CLAUDE.md` = `"@notes"` beside a file `notes`: the row is there, loaded, `raw == Some("notes")`.
  - `two_spellings_of_one_dead_import_are_two_rows`: `"@gone.md\n@./gone.md\n"` gives two "not found" rows with the same `path` and the two `raw`s.

In each of the four import-reading CLI modules, one test `the_load_says_how_this_cli_writes_imports` asserting `load.imports` as listed above, and in Codex's that it is `None`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 instructions`
Expected: FAIL to compile (`imports_at`, `mask_html_comments`, `looks_like_path`, `raw`, `imports` don't exist).

- [ ] **Step 3: Implement**

`looks_like_path(raw)`: with any of `.,;:!?)]}'"` dropped from its end first, true when `raw` starts with `./`, `../`, `.\`, `..\`, `/`, `~/`, `~\`, or a drive (`X:` then `/` or `\`); or when it holds a `/` or `\` and the text after the last one ends in `.` plus 1 to 10 ASCII letters or digits, at least one a letter; or when it holds no slash and ends in `.` plus one of (any case) `md markdown mdc mdx txt json jsonc yaml yml toml xml ini cfg conf js jsx ts tsx py rs go sh ps1 css html sql lock`.

In `Expander::imports_of`, a `Text::Missing` target gives a row only when `looks_like_path(&raw)`, and only the first time for that importing file and `raw` (a `HashSet<(PathBuf, String)>` on the expander). Every import row's `raw` is `Some(raw)`.

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p marshell-core -j 4`
Expected: PASS. 7d1's tests that expected a "not found" row for a prose word are changed to expect none, each with a comment naming 7d2 decision 3; list them in the report.

- [ ] **Step 5: Commit**

`fix(doctor): an @word that isn't a path is prose, and an import knows its line.`

---

### Task 2: The text checks

**Files:**
- Create: `crates/core/src/doctor/instr_text.rs`
- Modify: `crates/core/src/doctor/mod.rs` (`pub mod instr_text;`)

**Interfaces:**
- Consumes: `ImportAt`, `imports_at`, `mask_html_comments`, `ImportSyntax`, `is_fence`, `code_mask` from Task 1.
- Produces (all pure; all take the file's text as read; all but `remove_imports` with `in_comments` never read inside an HTML comment):

```rust
/// At most 80 characters of a line, trimmed, with "…" when cut.
pub fn quote(line: &str) -> String;
/// Backticked relative paths (decision 4), with their 1-based line, in order, each path once.
pub fn backticked_paths(text: &str) -> Vec<(usize, String)>;
pub struct Block { pub line: usize, pub folded: String, pub quote: String, pub chars: usize }
/// Paragraphs and fenced blocks of 200 characters or more (decision 5). `folded` has every run of whitespace as one space.
pub fn blocks(text: &str) -> Vec<Block>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Manager { Npm, Pnpm, Yarn, Bun }
impl Manager { pub fn name(self) -> &'static str }   // "npm", "pnpm", "yarn", "bun"
pub struct Said<T> { pub line: usize, pub what: T, pub quote: String }
/// Lines that name a package manager for use (decision 12). Capitals don't matter.
pub fn package_managers(text: &str) -> Vec<Said<Manager>>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentKind { Tabs, Spaces }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indent { pub kind: Option<IndentKind>, pub width: Option<u8> }
/// Lines that ask for an indent (decision 13). A line gives at least one of `kind` and `width`.
pub fn indents(text: &str) -> Vec<Said<Indent>>;
/// The text without every import written as one of `raws` (decision 2). With `in_comments`, the ones inside HTML
/// comments go too (decision 17). None when nothing was removed.
pub fn remove_imports(text: &str, raws: &[String], syntax: ImportSyntax, in_comments: bool) -> Option<String>;
```

- [ ] **Step 1: Write the failing tests** in `instr_text.rs`. Each entry below is one test, with these exact cases.

`backticked_paths_are_relative_paths_with_a_slash`: the text has four parts, and the result is `[(1, "src/a.ts"), (1, "docs/x.md")]`:

- Line 1 names, each as a code span: `src/a.ts`, `src/a.ts:12`, `docs/x.md#top`, and `src/a.ts` again.
- Line 2 holds the spans that never count: `pnpm`, `README.md`, `src/*.ts`, `<root>/x`, `$HOME/x`, `%TEMP%/x`, `https://a/b`, `a/.../b`, `path/to/file.ts`, `/abs/x`, `~/x`, `C:/x/y`, `-r/x`, `@scope/pkg` and `a b/c`.
- A fenced block whose one line is the span `in/fence.ts`.
- An HTML comment holding the span `in/comment.ts`.

`blocks_are_paragraphs_of_200_characters_or_more`: a 199-character paragraph gives none; a 200-character one gives one with its first line's number; two paragraphs that differ only in line breaks and doubled spaces have equal `folded`; a fenced block of 250 characters is one block from its opening fence; `quote` is the first line cut at 80; a 300-character paragraph inside an HTML comment gives none.

`package_managers_named_for_use`: one `Said` each for `Use pnpm for the frontend.` (Pnpm), `Run npm install first` (Npm), `yarn add react` (Yarn), `The package manager is bun.` (Bun), `Use Bun here.` (Bun), `use pnpm instead of npm` (Pnpm only), and a fenced `pnpm test` (Pnpm). None for `npx vitest`, `bunx tsc`, `npm install -g typescript`, `Don't use npm.`, `never run yarn install`, `pnpm, not npm`, `the pnpm-lock.yaml file`, `bundle the app`, and `npm` on its own.

`indents_asked_for`: `Indent with tabs.` (Tabs, no width), `Use 2 spaces for indentation.` (Spaces, 2), `Indentation: 4-space` (Spaces, 4), `indent width 8` (no kind, 8), `Indent with spaces, not tabs.` (Spaces), `indent 12 spaces` (Spaces, no width: over 8), `indent 2 or 4 spaces` (Spaces, no width: two widths). None for `Indent with tabs or spaces.`, `Open two tabs in the browser` and a fenced `indent with tabs`.

`removing_an_import`, with `At`, the raws `["gone.md"]` and `in_comments` false unless said. The rule: a line that holds only the import (a list marker `-`, `*` or `+` aside) goes with its line end; otherwise the import goes with one space after it, or, at a line's end, with one space before it.

| Text | Result |
| --- | --- |
| `"a\n@gone.md\nb\n"` | `"a\nb\n"` |
| `"a\r\n- @gone.md\r\nb"` | `"a\r\nb"` |
| `"a\n@gone.md"` | `"a\n"` |
| `"see @gone.md here\n"` | `"see here\n"` |
| `"@gone.md, then\n"`, raws `["gone.md,"]` | `"then\n"` |
| `"see @gone.md\n"` | `"see\n"` |
| `"x @[l](gone.md) y"`, `AgyLink` | `"x y"` |
| `"\u{feff}see @gone.md here\n"` | `"\u{feff}see here\n"` |
| `"@gone.md\nx\n@./gone.md\n"`, raws `["gone.md", "./gone.md"]` | `"x\n"` |
| `"<!-- café --> @gone.md here\n"` | `"<!-- café --> here\n"` |
| `"<!-- @gone.md -->\n@gone.md\nx\n"` | `"<!-- @gone.md -->\nx\n"` |
| the same text, `in_comments` true | `"<!-- -->\nx\n"` |
| `"@other.md\n"` | `None` |
| the import only inside a code span or a fence | `None` |
| the import only inside an HTML comment | `None` (and `Some("<!-- -->\n")` for `"<!-- @gone.md -->\n"` with `in_comments`) |

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 doctor::instr_text`
Expected: FAIL to compile (the module is empty).

- [ ] **Step 3: Implement** the functions. `remove_imports` finds positions with `imports_at` on the masked text (on the text itself with `in_comments`) and cuts bytes from the original, line by line, keeping each line's own ending. The package-manager and indent rules are decisions 12 and 13 word for word.

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p marshell-core -j 4 doctor::instr_text`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(doctor): the text checks behind stale paths, repeats, clashes and the import fix.`

---

### Task 3: A row can name files to open (protocol)

**Files:**
- Modify: `crates/protocol/src/doctor.rs`, every `DoctorItem { .. }` literal in `crates/core` and `src-tauri` (add `opens: Vec::new()`), `src/generated/` (regenerated)

**Interfaces:**
- Produces:

```rust
/// A file a row can open in the side pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DoctorOpen {
    /// Absolute.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub line: Option<u32>,
    /// "Open CLAUDE.md".
    pub label: String,
}
// on DoctorItem, after `loads`:
#[serde(default, skip_serializing_if = "Vec::is_empty")]
#[ts(optional, as = "Option<Vec<DoctorOpen>>")]
pub opens: Vec<DoctorOpen>,
```

- [ ] **Step 1: Write the failing test** in `crates/protocol/src/doctor.rs`: `an_item_with_nothing_to_open_has_no_opens_key` (serialized JSON has no `"opens"`; with one `DoctorOpen` it has `"opens":[{"path":"C:\\p\\CLAUDE.md","line":4,"label":"Open CLAUDE.md"}]`; JSON without the key reads back with an empty `opens`).

- [ ] **Step 2: Run it and see it fail**

Run: `cargo test -p marshell-protocol -j 4`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**, and add `opens: Vec::new()` to every literal.

- [ ] **Step 4: Run**

Run: `cargo test -p marshell-protocol -j 4`, then `cargo test -p marshell-core -j 4`, then `pnpm check:generated`
Expected: PASS, and `src/generated/DoctorOpen.ts` exists.

- [ ] **Step 5: Commit**

`feat(protocol): a Doctor item can name files to open.`

---

### Task 4: Dead imports and stale paths

**Files:**
- Create: `crates/core/src/doctor/instr_checks.rs`
- Modify: `crates/core/src/doctor/mod.rs`

**Interfaces:**
- Consumes: Tasks 1 to 3; `CliLoad`, `LoadedFile`, `Role`, `When`, `Reader`, `Text`, `display`, `hash_part`, `id_tail`, `cli_rank`, `folder_name` from `doctor::instructions`; `display_name` from `doctor::checks`; `project_id` from `references`.
- Produces:

```rust
/// Where a finding sits. `project` is None for one whose files are all at user level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Seat { pub project: Option<PathBuf>, pub distro: Option<String>, pub home: PathBuf }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    DeadImport {
        seat: Seat, file: PathBuf, target: PathBuf, syntax: ImportSyntax,
        /// The first line it is written on, and the import as written there (`@docs/setup.md`).
        line: usize, written: String,
        /// Every spelling of it in the file.
        raws: Vec<String>,
        /// Some CLI that reads the file reads HTML comments.
        in_comments: bool,
        agents: Vec<&'static str>,
    },
    StalePaths { seat: Seat, file: PathBuf, paths: Vec<(usize, String)> },
}
/// Every finding in these loads. `cx` is the reader that made them, so no file is read twice.
pub fn findings(loads: &[CliLoad], cx: &mut Reader) -> Vec<Finding>;
/// The findings as items, Broken first, then in `projects` order with user level last. `editable` says whether the
/// config service may write a path.
pub fn items(findings: &[Finding], projects: &[PathBuf], editable: &dyn Fn(&Path) -> bool) -> Vec<DoctorItem>;
/// The id of the finding's item.
pub fn id_of(f: &Finding) -> String;
```

Ids, with `(place, suffix) = id_tail(seat.project, seat.distro)`: `instrimport.<place>.<hash_part(file)>.<hash_part(target)>.<at|link><suffix>` and `instrstale.<place>.<hash_part(file)>`.

Rules the tests pin:
- **Rows looked at:** `When::Start` or `When::WhenUsed`, and not under a `Role::Memory` or `Role::Managed` root (decision 15). A row's root is the nearest earlier row in the load whose role isn't `Import` (check that `expand` pushes rows in that order; if it doesn't, follow `Role::Import { by }` up).
- **Seat:** a file inside the load's project has that project; any other file has `project: None`.
- **DeadImport:** import rows with `When::NotLoaded("not found")` and `raw: Some(_)` whose importing file is a row looked at, grouped by importing file (canonical), `path` and the load's `imports.syntax`, across all loads; `agents` are the CLIs with such a row, in `cli_rank` order; `raws` are the rows' `raw`s. `in_comments` is true when one of those CLIs has `imports.in_comments`. `line` and `written` are the first `imports_at` hit whose `raw` is in `raws`, on the masked text, or on the text itself with `in_comments`.
- **StalePaths:** Windows loads with a project only (decision 16), files inside the project, each file once however many CLIs read it. A path from `backticked_paths` is dropped when `cx.exists` finds it from the project root or from the file's folder; when its first folder is a folder in neither place and its last part has no extension (decision 4: `origin/main`); or when a `.gitignore` covers it. The gitignore check walks from the path's folder up to the project root; each folder's `.gitignore` (read with `cx.read`) is its own `ignore::gitignore::Gitignore` rooted at that folder, asked with `matched_path_or_any_parents`; the deepest one with an answer decides. `.git/info/exclude` and the global excludes file are not read. A file with no path left gives no finding. The walk stops when `cx.out_of_time()`.
- **Items:** a `DeadImport` is Broken; `fixable` and `unchecked: Some(true)` when the seat has no distro and `editable(file)`; otherwise `cannot_fix` is "Edit the file in <distro>." or "Marshell doesn't change this file." `StalePaths` is Info. `opens` holds one `Open <file name>` with the finding's line (the first path's line for `StalePaths`), and nothing for a seat with a distro. `distro` is the seat's. Sentences for a dead import: Claude Code "loads nothing for it"; the others "leave it in as text" ("leaves" for one), each sentence present only when it has a CLI; CLI lists read "A", "A and B", "A, B and C". Which sentence a CLI gets is `imports.in_comments`-independent: it comes from a `pub fn dead_import_drops(agent) -> bool` beside `display_name` in `doctor/checks.rs`, true for `claude` only.

- [ ] **Step 1: Write the failing tests** in `instr_checks.rs`, on real files in a `tempfile::tempdir()` with hand-built `CliLoad`s (a `fn load(agent, project, rows)` helper that sets `imports` as Task 1 lists), reading through `Reader::windows()`; the project folder is named `proj`:

  - `a_dead_import_names_its_line_and_the_clis_that_read_it`: `CLAUDE.md` = `"# A\n\n\n@gone.md\n"`, loads for claude and copilot each with the "not found" row. One finding, `line == 4`, `agents == ["claude", "copilot"]`. Its item: group Broken, title `"CLAUDE.md in proj imports a file that doesn't exist"`, detail `"Line 4: @gone.md. Claude Code loads nothing for it. Copilot CLI leaves it in as text."`, `fixable`, `unchecked == Some(true)`, `opens == [DoctorOpen { path: <full path>, line: Some(4), label: "Open CLAUDE.md" }]`.
  - `a_comment_counts_only_for_a_cli_that_reads_comments`: `CLAUDE.md` = `"<!-- @gone.md -->\n\n@gone.md\n"`. With claude's load alone: `line == 3`, `in_comments == false`. With copilot's too: `line == 1`, `in_comments == true`.
  - `the_same_dead_import_twice_is_one_item_at_its_first_line`: lines 3 and 9, two spellings (`@gone.md`, `@./gone.md`, two rows): one finding, `line == 3`, `raws.len() == 2`.
  - `a_dead_import_at_user_level_is_one_item_for_two_projects`: two project loads of claude, each carrying `~/.claude/CLAUDE.md`'s row and its dead import: one finding with `seat.project == None`, title `"~/.claude/CLAUDE.md imports a file that doesn't exist"`, id starting `instrimport.user.`.
  - `a_dead_import_marshell_may_not_edit_says_why`: `editable` false gives `fixable == false` and `cannot_fix == Some("Marshell doesn't change this file.")`; a load with `distro: Some("Ubuntu")` gives "Edit the file in Ubuntu.", no `opens`, `distro == Some("Ubuntu")` and an id ending `@Ubuntu`.
  - `a_link_import_shows_as_written`: an antigravity load, `GEMINI.md` = `"see @[notes](sub/gone.md)\n"`: detail `"Line 1: @[notes](sub/gone.md). Antigravity leaves it in as text."`, id ending `.link`.
  - `an_import_of_a_managed_or_memory_file_is_left_alone`: the dead import row under a `Role::Managed` root gives no finding.
  - `stale_paths_skip_what_exists_and_what_git_ignores`: project with `src/a.ts`, `.gitignore` = `dist/\n`, `sub/.gitignore` = `*.gen.ts\n`, `AGENTS.md` naming `` `src/a.ts` ``, `` `src/gone.ts` `` (line 3), `` `dist/app.js` ``, `` `sub/x.gen.ts` ``, `` `origin/main` `` and `` `src/gone` `` (line 7), and `docs/AGENTS.md` naming `` `./notes.md` `` beside a `docs/notes.md`. Loads for codex and copilot both hold `AGENTS.md`. One finding, for `AGENTS.md`, `paths == [(3, "src/gone.ts"), (7, "src/gone")]`; its item is Info, title `"AGENTS.md in proj names 2 paths that don't exist"`, detail `"Line 3: src/gone.ts. Line 7: src/gone."`.
  - `one_stale_path_is_singular`: title `"AGENTS.md in proj names a path that doesn't exist"`.
  - `six_stale_paths_show_five_and_count_the_rest`: detail ends `"And 1 more."`, title says "6 paths".
  - `a_user_level_file_and_a_distro_get_no_stale_paths`.
  - `ids_dont_change_between_runs`: two calls give equal ids; the ids of two projects differ.
  - `the_check_modules_log_nothing`: `include_str!("instr_checks.rs")` and `include_str!("instr_text.rs")` hold no `tracing::` outside this test (build the needle as `concat!("tracing", "::")`).

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 doctor::instr_checks`
Expected: FAIL to compile.

- [ ] **Step 3: Implement** `findings`, `items`, `id_of` and `dead_import_drops`.

- [ ] **Step 4: Run**

Run: `cargo test -p marshell-core -j 4`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(doctor): dead imports and stale paths as findings.`

---

### Task 5: Duplicates and clashes

**Files:**
- Modify: `crates/core/src/doctor/instr_checks.rs`

**Interfaces:**
- Consumes: Task 4's module; `blocks`, `package_managers`, `indents`, `quote` from Task 2; `ext::tokens`, `tokens_text`.
- Produces three more `Finding` variants, found by `findings` and worded by `items`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot { pub file: PathBuf, pub line: usize, pub says: String, pub quote: String, pub user_level: bool }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClashKind { Manager, TabsSpaces, Widths }
// in Finding:
LoadedTwice { seat: Seat, agent: &'static str, file: PathBuf,
    /// How each of the two loads came about: None by its own name, Some(the importing file) as an import.
    first: Option<PathBuf>, second: Option<PathBuf>, chars: usize },
Repeats { seat: Seat, a: PathBuf, b: PathBuf, agents: Vec<&'static str>, count: usize,
    lines: (usize, usize), quote: String, chars: usize },
Clash { seat: Seat, kind: ClashKind, agents: Vec<&'static str>, a: Spot, b: Spot },
```

Ids: `instrtwice.<agent>.<place>.<hash_part(file)><suffix>`, `instrrepeat.<place>.<hash_part(x)>.<hash_part(y)><suffix>` with `x`, `y` the pair sorted by path, `instrclash.<place>.<manager|tabs|widths><suffix>`.

Rules the tests pin (all over one load's `When::Start` rows looked at, in load order; they run for Windows and distro loads alike):
- **LoadedTwice:** two rows with the same canonical path and `chars > 0`. Seat by the file.
- **Repeats:** blocks with equal `folded` in two different files, or at two places in one file (a block is never matched with itself). Grouped across loads by the unordered pair of files; `agents` are the CLIs whose load holds both; `a` is the file that comes first in the first such load, `lines` and `quote` are the first repeated block's, `count` the number of distinct repeated blocks, `chars` their sum. A file that is `LoadedTwice` in a load is not paired with itself for that load (Review Focus 4). Seat: the project when either file is inside it.
- **Clash:** `Manager`: two `Said<Manager>` with different managers; `TabsSpaces`: a `Tabs` and a `Spaces`; `Widths`: two different widths. For a seat, the pair is the first one found, ordering pairs by the later line's position in the load and then the earlier's. The project's item takes pairs with at least one file inside the project; the user-level item takes pairs with both files outside it. One item for each kind and seat: the loads are taken in `cli_rank` order, the first load with a pair gives it, and `agents` are the CLIs whose load holds both files. `says` is the manager's name, `tabs` / `spaces`, or `width 2`.
- **Items**, all Info, `fixable: false`, with `opens` for each distinct file (none in a distro) and `agent: None` except `LoadedTwice`, which carries its agent as 7d1's `instrsize` items do:
  - `LoadedTwice`: "Copilot CLI loads AGENTS.md twice in proj" (user level: "Claude Code loads ~/.claude/CLAUDE.md twice"); detail "Once by its own name and once as an import of CLAUDE.md. About 250 tokens are sent twice." (two imports: "Once as an import of A.md and once as an import of B.md.")
  - `Repeats`: "CLAUDE.md and AGENTS.md in proj repeat a paragraph" / "repeat 3 paragraphs"; detail "Claude Code and Copilot CLI load both. The first is at CLAUDE.md:3 and AGENTS.md:7: “<quote>”. About 55 tokens are sent twice." One file: "CLAUDE.md in proj repeats a paragraph" / "repeats 3 paragraphs"; detail "Claude Code loads it. The first is at lines 12 and 40: “<quote>”. About 60 tokens are sent twice." User level: no " in proj".
  - `Clash`: "Two package managers are named in proj", "Tabs and spaces are both asked for in proj", "Two indent widths are asked for in proj" (user level: "… at user level"); detail "<a file>:<line>[ (user level)] says <says>: “<quote>”. <b file>:<line>[ (user level)] says <says>: “<quote>”. Claude Code loads both." "(user level)" is written only in a project's item.

- [ ] **Step 1: Write the failing tests:**

  - `a_file_loaded_by_name_and_as_an_import_is_loaded_twice`: copilot load with `CLAUDE.md` (`"@AGENTS.md"`), its import `AGENTS.md` and `AGENTS.md` itself, the file being one 1,000-character paragraph. Title `"Copilot CLI loads AGENTS.md twice in proj"`, detail `"Once by its own name and once as an import of CLAUDE.md. About 250 tokens are sent twice."`; and no `Repeats` finding.
  - `a_paragraph_in_two_files_is_a_repeat`: one 220-character paragraph in `CLAUDE.md` (line 3) and `AGENTS.md` (line 7), loads for claude and copilot holding both. Title `"CLAUDE.md and AGENTS.md in proj repeat a paragraph"`; detail starts `"Claude Code and Copilot CLI load both. The first is at CLAUDE.md:3 and AGENTS.md:7: “"` and ends `"About 55 tokens are sent twice."`; two `opens`; one item though two loads found it.
  - `three_repeated_paragraphs_are_plural`: title ends `"repeat 3 paragraphs"`.
  - `a_paragraph_twice_in_one_file`: one 240-character paragraph at lines 12 and 40 of `CLAUDE.md`. Title `"CLAUDE.md in proj repeats a paragraph"`, detail `"Claude Code loads it. The first is at lines 12 and 40: “…”. About 60 tokens are sent twice."` with the quote in place of `…`; one `opens`.
  - `two_package_managers_clash_and_the_user_level_one_is_marked`: project `AGENTS.md` line 2 `Use pnpm for the frontend.`, user-level `~/.claude/CLAUDE.md` line 1 `Always run npm install first.`; a claude load with the user file first. Title `"Two package managers are named in proj"`, detail `"~/.claude/CLAUDE.md:1 (user level) says npm: “Always run npm install first.”. AGENTS.md:2 says pnpm: “Use pnpm for the frontend.”. Claude Code loads both."`.
  - `tabs_against_spaces_and_two_widths_are_their_own_items`: `AGENTS.md` with `Indent with tabs.` (line 1), `Use 2 spaces for indentation.` (line 2) and `indent width 4` (line 3). Titles `"Tabs and spaces are both asked for in proj"` and `"Two indent widths are asked for in proj"`; the first's detail `"AGENTS.md:1 says tabs: “Indent with tabs.”. AGENTS.md:2 says spaces: “Use 2 spaces for indentation.”. Claude Code loads both."`; the second's says `width 2` and `width 4`.
  - `two_clis_with_different_pairs_are_one_item`: claude's load clashes through `CLAUDE.md` and `AGENTS.md`, codex's through `AGENTS.md` and `sub/AGENTS.md`: one `instrclash.<place>.manager` item, with claude's pair.
  - `a_clash_between_user_level_files_is_one_item_at_user_level`: two user-level files clash and two projects' loads carry them, and one project file names a third manager: one item `"Two package managers are named at user level"` with the id `instrclash.user.manager`, and each project's own item pairs a project line.
  - `memory_and_managed_files_are_left_alone`: the same clash with one line in a `Role::Memory` row gives nothing.
  - `a_distro_gets_duplicates_and_clashes`: a load with `distro: Some("Ubuntu")` and two clashing user-level files: one clash item, id ending `@Ubuntu`, no `opens`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 doctor::instr_checks`
Expected: FAIL to compile (the variants don't exist).

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run**

Run: `cargo test -p marshell-core -j 4`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(doctor): files loaded twice, repeated paragraphs and clashing rules as findings.`

---

### Task 6: Cuts: one user-level item, and Codex's limit can be raised

**Files:**
- Modify: `crates/core/src/doctor/instructions.rs` (`Cut`, `items`), `crates/core/src/agents/codex/instructions.rs`, `agents/antigravity/instructions.rs`, `agents/claude/instructions.rs`, every other `Cut { .. }` literal, and the one call of `instructions::items` in `crates/core/src/server/doctor.rs` (pass `&|p| config.editable(p)` from the state's config service)

**Interfaces:**
- Produces:
  - `Cut` gains `pub user_level: bool` and `pub raise: Option<Raise>`, with `pub struct Raise { pub config: PathBuf, pub bytes: u64, pub says: String }`. `says` is written by the CLI's module.
  - `pub const RAISE_STEP: u64 = 8_192;` and `pub const RAISE_MAX: u64 = 1_048_576;` in `agents/codex/instructions.rs`.
  - `pub fn cut_id(load: &CliLoad, cut: &Cut) -> String` in `doctor/instructions.rs`: `instrcut.<agent>.<place>.<id_part><suffix>`, with the place `user` when `cut.user_level`. `items` builds its ids with it.
  - `pub fn items(loads: &[CliLoad], projects: &[PathBuf], editable: &dyn Fn(&Path) -> bool) -> Vec<DoctorItem>`: a `user_level` cut appears once however many loads carry it. A cut with `raise` whose `config` is `editable` is `fixable: true`, `unchecked: Some(true)`, has no `cannot_fix`, and its detail is the 7d1 detail, a space, and `raise.says`; when `config` isn't editable the item is as in 7d1.
- Consumes: `id_tail` from Task 1.

- [ ] **Step 1: Write the failing tests**

`agents/codex/instructions.rs`: `a_cut_knows_what_raising_the_limit_takes`: a project whose one `AGENTS.md` is 40,000 bytes under the default 32,768 gives `raise.bytes == 40_960`, `raise.config == <codex home>/config.toml` and `raise.says == "The fix sets project_doc_max_bytes to 40960, for every project. Codex then loads about 1.8k more tokens at each start here."`; `a_project_past_one_mebibyte_is_not_raised`: 1,100,000 bytes gives `raise == None`; `a_budget_of_zero_is_not_raised`.

`agents/antigravity/instructions.rs` and `agents/claude/instructions.rs`: `a_user_level_file_over_the_limit_names_no_project`: the cut for the user's own file has `user_level == true` and the title `"Antigravity cuts ~/.gemini/GEMINI.md"` (`"Claude Code skips ~/.claude/CLAUDE.md"`); a project file's cut keeps its 7d1 title and `user_level == false`.

`doctor/instructions.rs`: `a_user_level_cut_is_one_item_for_two_projects` (two loads, two projects, the same user-level cut: one item, id `instrcut.antigravity.user.<part>`); `a_cut_codex_can_raise_is_fixable_and_unticked` (detail ends with `raise.says`, `cannot_fix == None`); `a_raise_marshell_may_not_write_stays_report_only` (`editable` false: `fixable == false` and 7d1's `cannot_fix`).

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 instructions`
Expected: FAIL to compile (`user_level`, `raise`, `cut_id`).

- [ ] **Step 3: Implement.** `bytes` is the sum of the bytes of every file Codex's budget counts for the project (loaded, cut and skipped; 7d1 counts no separators), rounded up to a multiple of `RAISE_STEP`; over `RAISE_MAX`, or with a budget of 0, there is no `raise`. The token figure in `says` is `tokens_text(ext::tokens(<bytes not loaded today>))`.

- [ ] **Step 4: Run**

Run: `cargo test -p marshell-core -j 4`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(doctor): a user-level cut is one item, and Codex's cut knows the limit that would fit.`

---

### Task 7: Reference notes are counted and reported

**Files:**
- Modify: `crates/core/src/references.rs`, `crates/core/src/server/hook.rs` (`references_for` and its caller)
- Create: `crates/core/src/doctor/refnotes.rs`
- Modify: `crates/core/src/doctor/mod.rs`

**Interfaces:**
- Produces:

```rust
// references.rs
pub const NOTE_DAYS: u64 = 30;
/// Counts one sent note for the project. Holds alias names and numbers, never the prompt or the note.
pub fn record_note(home: &Path, root: &Path, note: &ReferenceNote, now_ms: u64) -> anyhow::Result<()>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteTotals { pub notes: u32, pub chars: u64, pub top_alias: Option<(String, u32)> }
/// The last 30 days' totals, or None with no note in them.
pub fn note_totals(home: &Path, root: &Path, now_ms: u64) -> Option<NoteTotals>;

// doctor/refnotes.rs
pub fn items(projects: &[(PathBuf, NoteTotals)]) -> Vec<DoctorItem>;
```

The file is `<home>/projects/<project_id(root)>/reference-notes.json`, a `Versioned` (version 1) `{ root, days: [{ day, notes, chars, aliases: { name: count } }] }`, `day` being `now_ms / 86_400_000`. `record_note` drops days older than `NOTE_DAYS`, and takes a process-wide `Mutex` around its read and write. A file kept for another root is started again. `top_alias` is the alias in the most notes (ties: the first by name).
- Consumes: `ReferenceNote` (`names`, `note`), `project_id`, `store::{read, write}`.

- [ ] **Step 1: Write the failing tests**

`references.rs`: `a_note_is_counted_with_its_aliases` (two notes on one day: `notes == 2`, `chars` the two notes' character counts, `top_alias == Some(("app".into(), 2))`, and the file is at `<home>/projects/<project_id>/reference-notes.json`); `a_day_older_than_thirty_is_dropped` (a note at day 0 and one at day 31: totals at day 31 count one, and the file holds one day); `no_note_is_none`; `the_file_holds_no_note_text` (the file's text contains neither the note's path nor "References:"); `two_threads_lose_no_count` (8 threads, 25 notes each: `notes == 200`); `a_counter_that_cant_be_written_is_an_error` (`projects/<id>` is a file: `Err`, no panic).

`doctor/refnotes.rs`: `a_project_with_notes_gets_its_line` (14 notes, 4,480 chars: Info, id `refnotes.<project id>`, title `"proj: 14 reference notes in the last 30 days, about 1.1k tokens"`, detail `"About 80 tokens a note. Marshell adds a note to a prompt that names an alias."`, `fixable == false`); `one_note_is_singular` (`"proj: 1 reference note in the last 30 days, about 80 tokens"`); `long_notes_are_waste` (5 notes averaging 210 tokens: Waste, id `refnotes.long.<id>`, title `"Reference notes in proj average about 210 tokens"`, detail `"A note this long names several aliases at once."`, `cannot_fix == Some("Shorten or remove aliases in Settings, Projects.")`; 4 such notes give none); `an_alias_in_most_notes_is_waste` (12 of 14: Waste, id `refnotes.alias.<id>`, title `"The alias “app” is in 12 of 14 reference notes in proj"`, detail `"A name this common adds a note to most prompts."`, `cannot_fix == Some("Rename it to something you only type when you mean it, or remove it, in Settings, Projects.")`; 5 of 9 gives none, 7 of 14 gives none).

`server/hook.rs` tests, on the existing references hook test's setup: `a_sent_note_is_counted_and_its_text_is_unchanged` (the hook's answer holds the same note text as before this task, and `note_totals` for the project becomes `Some` with `notes == 1` within 2 s); `a_note_dropped_for_time_is_not_counted` (with the context budget at zero); `a_counter_that_cant_be_written_still_sends_the_note` (`projects/<id>` is a file: the answer still holds the note).

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 references`
Expected: FAIL to compile.

- [ ] **Step 3: Implement.** `references_for` returns the project root with the note. The hook handler, once it knows the note goes out, starts `record_note` on `spawn_blocking` and does not wait for it; a failure is one `tracing::warn!` with the error's kind only: no prompt text, no note text and no path.

- [ ] **Step 4: Run**

Run: `cargo test -p marshell-core -j 4`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(references): notes are counted for each project, and the Doctor has their items.`

---

### Task 8: The run and the two fixes

**Files:**
- Modify: `crates/core/src/server/doctor_instr.rs`, `crates/core/src/server/doctor.rs`

**Interfaces:**
- Consumes: `instr_checks::{findings, items, id_of, Finding}`, `instr_text::remove_imports`, `instructions::{cut_id, Cut, Raise}`, `references::note_totals`, `refnotes::items`, `ConfigService::editable`, `EditOp::{ReplaceText, TomlSet}`, `config::hash::{hash_bytes, hash_file}`, `Applies::NewSessions`.
- Produces:
  - `doctor_instr::loads` returns `pub(crate) struct InstrRead { pub loads: Vec<CliLoad>, pub findings: Vec<Finding> }`; the findings are computed for each reader (Windows, then each distro) before it is dropped.
  - The run's items, in order: 7d1's instruction items, then `instr_checks::items`, then `refnotes::items` for each of the run's projects with totals.
  - `fix_of` handles ids that start `instrimport.` and `instrcut.`, split off before it looks up agent facts:
    - It calls `doctor_instr::loads` only when such an id is asked for (decision 11).
    - `instrimport.`: each id must be the `id_of` a `DeadImport` finding of this read that is on Windows and `editable`; else the usual 422. The ids are grouped by file. A file's bytes are read with `std::fs::read` and must be UTF-8; each of its findings is removed in turn with `remove_imports(text, &raws, syntax, in_comments)`, and a `None` from any of them is a 422 for the whole fix. One `FileEdit { path, scope: Project when the seat has a project, else User, op: ReplaceText { content }, expected_hash: Some(hash_bytes(the bytes read)), applies: Applies::NewSessions }` for each file.
    - `instrcut.`: each id must be the `cut_id` of a cut with `raise` whose `config` is `editable`; else 422. One `FileEdit` for each distinct `raise.config`: `TomlSet { key: ["project_doc_max_bytes"], value: "<the largest bytes among its ticked cuts>" }`, `scope: User`, `expected_hash: hash_file(config)` (`None` for a file that isn't there: `TomlSet` creates it, and nothing records it as Marshell's file), `applies: Applies::NewSessions`.

- [ ] **Step 1: Write the failing tests** in `server/doctor.rs`'s tests, with the module's own test state and scratch homes:

  - `a_run_reports_a_dead_import_and_a_fix_preview_removes_it`: a project `CLAUDE.md` = `"# A\r\n@gone.md\r\nrest\r\n"`. The report has the `instrimport.` item, unticked. `POST /v1/doctor/fix` with its id plans one edit whose `ReplaceText` content is `"# A\r\nrest\r\n"`.
  - `the_import_fix_applies_and_undoes_byte_for_byte`: apply, the file is `"# A\r\nrest\r\n"` and the next report has no such item; undo, the file's bytes are the first ones again.
  - `two_dead_imports_in_one_file_are_one_edit`.
  - `one_of_two_ticked_imports_gone_refuses_the_whole_fix`: the file is rewritten between report and fix so one import is gone: 422, and the file is as it was rewritten.
  - `a_dead_import_in_a_comment_is_fixed_for_a_cli_that_reads_comments`: a project with only `.github/copilot-instructions.md` (or whichever file the Copilot reader takes alone) = `"<!-- @gone.md -->\nx\n"` and Copilot found: the item is there; after the fix is applied the next report has none.
  - `raising_codexs_limit_sets_the_key`: a 40,000-byte `AGENTS.md` and a Codex `config.toml` holding `model = "x"\n`; the fix plans `TomlSet` to 40960; applied, the file holds both keys and the next report has no cut item; undo restores the file's bytes.
  - `raising_codexs_limit_creates_a_missing_config`: applied, `config.toml` is `project_doc_max_bytes = 40960\n`; undo removes it; and the managed list has no entry for it.
  - `two_projects_over_the_limit_set_the_larger_value_once`.
  - `a_fix_preview_reads_no_instruction_file` stays as it is and passes; `an_instruction_fix_preview_reads_them_once` is new (`instr_reads` grows by one).
  - `a_run_reports_reference_notes`: `record_note` for the test project, then the report has the `refnotes.` Info item.
  - `the_instruction_findings_come_after_the_file_lists`: item order as above.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p marshell-core -j 4 server::doctor`
Expected: FAIL (no `instrimport.` item in the report).

- [ ] **Step 3: Implement.** The fix sheet's wording for a file comes from the existing preview; nothing new is added to it.

- [ ] **Step 4: Run**

Run: `cargo test -p marshell-core -j 4`
Expected: PASS.

- [ ] **Step 5: Commit**

`feat(doctor): the run reports the instruction findings and plans their two fixes.`

---

### Task 9: The row opens the file

**Files:**
- Modify: `src/features/doctor/DoctorRow.tsx`, `src/features/doctor/DoctorPage.tsx`, `src/features/doctor/LiveDoctor.tsx`, `src/features/sessions/LiveApp.tsx`, `src/features/doctor/fixtures.ts`, `src/features/doctor/Doctor.css` only if the buttons need a gap
- Test: `src/features/doctor/DoctorPage.test.tsx`, `src/features/doctor/fixtures.test.ts` if it lists the fixtures

**Interfaces:**
- Consumes: `DoctorOpen` and `DoctorItem.opens` from `src/generated/`; `openInPane(target: OpenTarget): boolean` and the overlay state in `LiveApp.tsx`.
- Produces: `onOpenFile?: (open: DoctorOpen) => void` on `DoctorRow`, `DoctorPage` and `LiveDoctor`. In `LiveApp.tsx` the handler calls `openInPaneRef.current({ path: open.path, line: open.line ?? undefined })` and closes the Doctor only when it returned true, so with no session the existing "No session to open it in." is announced and the Doctor stays open.

- [ ] **Step 1: Write the failing tests** in `DoctorPage.test.tsx`, with a new fixture `DOCTOR_FINDINGS: DoctorReport` holding one item of each 7d2 kind with the decisions' copy (its repeat item has two `opens`, its reference-note item none):

```tsx
const repeat = DOCTOR_FINDINGS.items.find((i) => i.id.startsWith("instrrepeat."))!;
it("a finding's row has a button for each file it names", () => {
  const html = renderToStaticMarkup(<DoctorRow item={repeat} />);
  expect(html).toContain(">Open CLAUDE.md</button>");
  expect(html).toContain(">Open AGENTS.md</button>");
});
it("a row with no file to open has no such button", () => {
  expect(renderToStaticMarkup(<DoctorRow item={DOCTOR_LOADS.items[0]} />)).not.toContain(">Open CLAUDE.md");
});
it("a fixable finding starts unticked", () => {
  // the dead-import item of DOCTOR_FINDINGS renders its checkbox without `checked`
});
```

and, through the file's own page helper, that the "Open …" buttons render as `<button type="button" class="doctor__btn">` inside the row, after its detail.

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm test src/features/doctor`
Expected: FAIL (no "Open CLAUDE.md").

- [ ] **Step 3: Implement.** The buttons are `button.doctor__btn` in `opens` order, placed where the row's other buttons are ("Open Agents"), each calling `onOpenFile(open)`. `doctor__btn` already has its focus ring and forced-colours rules; a gap between two buttons, if the row's layout doesn't give one, is written as the `good-css` skill says.

- [ ] **Step 4: Run**

Run: `pnpm test`, then `pnpm build`
Expected: PASS, and the build completes.

- [ ] **Step 5: Commit**

`feat(doctor): a finding's row opens its file in the side pane.`

---

### Task 10: Live check

**Files:**
- Create: `docs/research/2026-11-25-instruction-checks-live-check.md`

Run with the `marshell-live-check` skill: a new literal scratch, `-CodexHome` (60 characters at most), `-CodexAuth`, `-QwenFake`, `-CopilotFake`. Claude Code's own rows may carry the real `~/.claude/CLAUDE.md` when a project sits under the real home folder, as 7d1's check found; no criterion depends on them.

Exit criteria:

1. **Dead import.** `pA/CLAUDE.md` has `@docs/missing.md` on its own line, `see @gone/notes.md here`, `@someone`, `@john.doe` and `@types/node`. Two Broken items, unticked, with the decisions' copy; no item and no row for the three prose words. One ticked, "Fix 1 issue": the sheet shows that one line going; applied, the next run has one item left; Undo puts the file back byte for byte (hash compared).
2. **A comment.** `pA/AGENTS.md` holds `<!-- @old/gone.md -->`: the item names the CLIs that read comments and not Qwen Code or Claude Code.
3. **Stale paths.** `pA/AGENTS.md` names an existing path, `src/gone.ts`, `dist/app.js` (in `.gitignore`), `origin/main` and `src/*.ts`: one Info item naming only `src/gone.ts` with its line.
4. **Duplicates.** `pB` with `CLAUDE.md` importing `AGENTS.md`: "Copilot CLI loads AGENTS.md twice in pB". `pC` with one 300-character paragraph in `CLAUDE.md` and `AGENTS.md`: the repeat item with both `file:line`.
5. **Clashes.** `pC/AGENTS.md` says "Use pnpm", the scratch Codex home's `AGENTS.md` says "Always run npm install first.": the item marks the user-level line and names Codex.
6. **Codex's limit.** `pD/AGENTS.md` of 40,000 bytes: the cut item is unticked and fixable; the fix sets `project_doc_max_bytes = 40960` in the scratch Codex home; one real `codex exec -s read-only` turn in `pD` prints no "Loaded only" warning; Undo puts `config.toml` back.
7. **Reference notes.** An alias added for `pA` in Settings, Projects, references on; one prompt naming it in a Qwen Code session on the fake model: the request log has the "References:" note, and the Doctor shows "pA: 1 reference note in the last 30 days, about N tokens". The two Waste items are checked from a counter file seeded in the scratch home.
8. **Open file.** With a session on screen, "Open CLAUDE.md" closes the Doctor and the text editor shows the file with the caret on the finding's line. With no session, the press announces "No session to open it in." and the Doctor stays.
9. **Keyboard and forced colours.** Tab reaches every "Open …" button; Enter presses it; the focus ring shows; under emulated forced colours the buttons keep their border and ring.
10. **One user-level cut.** An Antigravity global rule over its limit with two projects listed: one Broken item, with no project in its title.
11. **A 7a fix reads no instruction file.** As in 7d1's check: a seeded MCP item's "Fix 1 issue" shows its sheet, and the suite's test for it is green.

End state as the skill says: tree listed and read before `stop`, only the check's own PIDs stopped, the real `~/.claude/settings.json`'s time unchanged, `agy --version` before and after, and the scratch folders listed for the user. Findings go in the note; a defect gets a failing test first.
