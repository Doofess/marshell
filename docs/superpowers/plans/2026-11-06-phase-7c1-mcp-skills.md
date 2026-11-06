# Slice 7c1: MCP servers and skills, listed, checked and turned off — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Doctor's background checks list each CLI's MCP servers and skills, at user level and in Marshell's recent projects. They report the following, and the Broken items can be turned off with preview, backup and Undo:
- servers that can't start;
- skills that don't load;
- descriptions the CLI cuts;
- the skills' token cost.

**Architecture:**
- **Pure core.** A new module, `crates/core/src/doctor/ext.rs`, holds the inventory types and turns inventories into `DoctorItem`s, with no I/O. `crates/core/src/doctor/frontmatter.rs` reads a `SKILL.md` frontmatter without a YAML dependency.
- **Readers.** Each CLI's reader lives in its own adapter folder (`agents/<cli>/extensions.rs`). It knows that CLI's files, its off switches, its skill rules and its listing caps, and returns an `Inventory`.
- **Server wiring.** `crates/core/src/server/doctor_ext.rs` builds the roots, gathers the inventories into 7a's `Gathered`, and turns a picked item's `Switch` into `FileEdit`s. `server/doctor.rs` calls it from `run_once` and `fix_of`.

No protocol change and no frontend change: `DoctorItem` already carries everything, and the Doctor page already shows report-only and fixable items in all three groups.

No new dependencies. `toml_edit`, `serde_json` and `jsonc` editing are already in the tree.

**Tech Stack:** Rust (axum core, the config service, `toml_edit`, `serde_json`).

**Spec:**
- `docs/PLAN.md` §9, "Slice 7c decisions (2026-11-06)", decisions 1, 3, 6, 7, 8, 9 and 10. Decisions 2, 4 and 5 are 7c2's.
- These sit on top of 7a's decisions 2, 3 and 7, and BRIEF §5.4.
- The facts and their sources are in `docs/research/2026-11-06-mcp-and-skills.md`.
- The code was read on `97a6470`; line numbers may drift a little.

Facts this plan relies on (from the research note, unless marked):
- **Skill rules per CLI.**
  - **Claude Code:** no frontmatter field is required. The folder name stands in for `name`, and the first paragraph for `description`.
  - **Codex** (source, `codex-rs/skills/src/parser.rs`): the file's first line must be `---`, and a closing `---` must follow with at least one line between. A `description` must be non-empty. `name` is optional, defaults to the folder name, and is at most 64 characters.
  - **Antigravity** (its shipped `skills.md`): `name` and `description` are required.
  - **Qwen Code** (its bundled `features/skills.md`): `name` is non-empty and matches `^[\p{L}\p{N}_:.-]+$`, and `description` is non-empty.
- **Listing caps.**
  - Claude Code cuts `description` plus `when_to_use` at 1,536 characters.
  - Codex cuts `description` at 1,024.
  - Qwen Code cuts every non-bundled description to its first line once the listing passes 8,000 characters.
  - Antigravity documents no cut.
  - Claude Code's `skillOverrides` value `"name-only"` lists the name alone, and `disable-model-invocation: true` keeps a skill out of the listing.
- **On this PC:**
  - `~/.claude.json` keys its projects with forward slashes (`"C:/dev/Airport"`). Each project entry already has `disabledMcpServers`, `disabledMcpjsonServers` and `mcpServers`.
  - `~/.gemini/config/mcp_config.json` exists and is 0 bytes.
  - Claude Code's `plugins/installed_plugins.json` is `{"version", "plugins": {"<name>@<market>": [{"scope", "installPath", "version", …}]}}`.
  - Codex keeps plugins in `~/.codex/plugins/cache/<market>/<plugin>/<version>/`, sometimes with a `latest` folder, and lists them as `[plugins."<plugin>@<market>"] enabled`.
- **Existing helpers:**
  - `AppState::on_path(command)` has a test swap.
  - `crate::projects::list(home) -> Vec<ProjectInfo>` returns up to 30 entries with a `path`.
  - `crate::config::hash::hash_bytes`.
  - In a `ChangeSet` with several edits to one file, the config service applies them in order (`service.rs` l.309).
  - A `JsonSet` on a missing file with `expected_hash: None` creates it, as hook installs do (`agents/claude/hooks.rs` l.332).
  - `ReplaceText` needs `expected_hash` and an existing file.
  - The JSON pointer reader takes `~1` and `~0` (`config/jsonc.rs` l.567).
  - A distro's user home is a UNC path (`DistroTarget.home`), and the config service writes there, as `distro_install` does.

## Global Constraints

- **Commits and staging.**
  - Commit messages read `type(scope): what changed, as a sentence`.
  - Run `cargo fmt --all` before every Rust commit.
  - Every cargo command uses `-j 3`, for example `cargo test -j 3 -p marshell-core doctor::ext`.
  - Stage only your own files, by path. Never `git add -A` or `git commit -a`.
  - Run only your own tests. The lead runs the full suites.
- **Worktree.** Work in `C:\dev\Airport\.claude\worktrees\phase-7c` on branch `phase-7c`, with plain commands from its root.
- **Vendor code stays in adapters.** Each CLI's file locations, off switches, skill rules and caps live in its `agents/<cli>/extensions.rs`. `doctor/ext.rs` knows no CLI's file names.
- **Async.**
  - Never hold a `MutexGuard` across `.await` or I/O.
  - The readers run inside 7a's `facts_of`, which already runs in `spawn_blocking`.
  - No new route.
- **Writes (`.claude/rules/config-writes.md`).**
  - Every fix goes through `ConfigService::preview_mixed`, and every edit is the person's own (`own` is true), so `managed.json` records nothing.
  - JSON is edited only with `JsonSet`, never by text.
  - TOML is edited with `toml_edit` and written as one `ReplaceText` per file.
  - An edit carries the hash of the file text it was built from, or `None` for a file that doesn't exist.
  - The fix only turns things off. It never deletes an entry, a file or a skill folder.
- **Only per-user files are written (decision 8).** Never a project's `.mcp.json`, `.codex/config.toml`, `.agents/mcp_config.json`, `.claude/settings.json` or any skill file.
- **Never in a test or worker:**
  - A test never reads the real home. Every reader takes its roots as arguments.
  - The server tests use `AppState::with_ext_home` (Task 4) and a fake home under the repo's `target` folder, as 7a's tests do.
  - Never run a real CLI, `wsl.exe` or `pnpm tauri dev` from a worker.
- **Copy.** Sentence case, subject first, no "!", no "successfully". This plan's copy is exact. `<CLI>` is `checks::display_name(agent)`, and `<place>` is empty on Windows and ` in <distro>` in a distro, as in 7a.

## Review Focus

1. **Empty, commented or odd config files.**
   - Antigravity's 0-byte `mcp_config.json` (this PC), a JSONC file with comments and trailing commas, and a `config.toml` with no `[mcp_servers]` each give zero servers and no item.
   - A file that truly doesn't parse gives one Info "can't be read" item and no Broken item.
   - Tasks 2 and 3 test this.
2. **Frontmatter the small parser doesn't fully follow.** Examples:
   - block scalars (`>`, `|-`);
   - quoted values holding `:` or `#`;
   - CRLF endings;
   - a BOM;
   - a description that continues on indented lines.

   None of these may give a Broken item, because a Broken item is pre-checked and turns the skill off. When unsure, the parser says `Unclear` and no Broken item follows. Task 1 tests this.
3. **Several fixes in one file.**
   - Two Qwen servers and a Qwen skill go into one `settings.json`. A Codex server and two Codex skills go into one `config.toml`.
   - Each gives one edit per list (or one `ReplaceText` per TOML file) holding every name, and keeps the names already in the list.
   - Applying it and then Undo gives back the exact bytes.
   - Task 4 tests this.
4. **Names that need escaping.**
   - A server or skill name with `/`, `~`, `.`, a space or a quote. The JSON pointer escapes `~` and `/`, and the TOML key is quoted, so `my.server` stays one key.
   - A Claude project key `C:/dev/x` matches the Marshell project `C:\dev\x`, whatever the case.
   - Tasks 2 and 4 test this.
5. **Something already off, or not listed.**
   - A server that is already turned off (Codex `enabled = false`, Antigravity `disabled: true`, a Qwen `mcp.excluded` name, a Claude `disabledMcpServers` or `disabledMcpjsonServers` entry) gives no item.
   - A skill turned off (`skillOverrides: "off"`, a Codex `[[skills.config]]` with `enabled = false`, a Qwen `skills.disabled` name) also gives no item. So does one hidden by `disable-model-invocation`, and neither adds to the cost.
   - Tasks 2, 3 and 1 test this.

---

### Task 1: Frontmatter, inventory types and the items

**Files:**
- Create: `crates/core/src/doctor/frontmatter.rs`
- Create: `crates/core/src/doctor/ext.rs`
- Modify: `crates/core/src/doctor/mod.rs` (add `pub mod ext; pub mod frontmatter;`)

**Interfaces:**
- Consumes: `marshell_protocol::doctor::{DoctorItem, DoctorGroup}`, `checks::display_name`, `crate::config::hash::hash_bytes`.
- Produces (`frontmatter.rs`):
  ```rust
  #[derive(Debug, Clone, Default, PartialEq, Eq)]
  pub struct SkillMeta {
      pub name: Option<String>,
      pub description: Option<String>,
      pub when_to_use: Option<String>,
      /// `disable-model-invocation: true`
      pub hidden: bool,
  }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum Front {
      /// Every top-level line was read.
      Read(SkillMeta),
      /// Some line wasn't understood; what was read is kept, but nothing may be called missing from it.
      Unclear(SkillMeta),
      /// No `---` first line, no closing `---`, or nothing between them.
      NoBlock,
  }
  pub fn read(text: &str) -> Front;
  ```
- Produces (`ext.rs`):
  ```rust
  /// Where the CLI's files are, as the server resolved them. A reader never reads the environment.
  pub struct ExtRoots {
      pub home: PathBuf,           // the user home (UNC in a distro)
      pub claude_dir: PathBuf,     // CLAUDE_CONFIG_DIR or <home>/.claude
      pub claude_json: PathBuf,    // <CLAUDE_CONFIG_DIR>/.claude.json or <home>/.claude.json
      pub claude_settings: PathBuf,// AppState's claude_settings_path
      pub codex_home: PathBuf,     // CODEX_HOME or <home>/.codex
      pub qwen_dir: PathBuf,       // QWEN_HOME or <home>/.qwen
  }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum Origin { User, Project(PathBuf), Plugin(String), Bundled }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum Switch {
      /// Adds `name` to the JSON array at `pointer` in `file`; a missing file or key starts an empty array.
      JsonAdd { file: PathBuf, pointer: String, name: String },
      /// Sets `pointer` in `file` to the JSON text `value`.
      JsonSet { file: PathBuf, pointer: String, value: String },
      /// Codex: `[mcp_servers.<name>] enabled = false` in `file`.
      CodexServer { file: PathBuf, name: String },
      /// Codex: a `[[skills.config]]` table `{ path = "<skill>", enabled = false }` in `file`.
      CodexSkill { file: PathBuf, skill: PathBuf },
  }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum NoSwitch { ClaudeUser, Shared(PathBuf), Plugin(String), NoSkillSwitch, Bundled }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum CommandState { Found, Missing, NotChecked }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum Transport { Stdio { command: String, state: CommandState }, Remote, Nothing }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct Server { pub name: String, pub file: PathBuf, pub origin: Origin, pub transport: Transport, pub off: bool, pub switch: Result<Switch, NoSwitch> }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum SkillState { Ok, NoBlock, Missing(&'static str), BadName }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct Skill {
      pub name: String, pub file: PathBuf, pub origin: Origin, pub state: SkillState,
      /// What the CLI lists besides the name: `description` (+ `when_to_use` for Claude Code), in characters.
      pub listed_chars: usize,
      /// The description's first line, in characters (Qwen Code's cut).
      pub first_line_chars: usize,
      /// Listed by name only (Claude Code's `"name-only"`).
      pub name_only: bool,
      pub off: bool, pub hidden: bool,
      pub switch: Result<Switch, NoSwitch>,
  }
  /// What a CLI cuts. Set by its reader.
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
  pub struct Caps { pub per_skill: Option<usize>, pub listing_first_line_over: Option<usize> }
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct Inventory {
      pub agent: &'static str, pub distro: Option<String>,
      /// The CLI is on this machine; an inventory of a CLI that isn't gives no item.
      pub found: bool,
      pub caps: Caps,
      pub servers: Vec<Server>, pub skills: Vec<Skill>,
      /// Config files that exist and don't parse: (file, message).
      pub unreadable: Vec<(PathBuf, String)>,
  }
  /// The first 8 hex digits of `hash_bytes("<file>\n<name>")`, with the file as `display()` prints it.
  pub fn key(file: &Path, name: &str) -> String;
  pub fn items(inv: &[Inventory]) -> Vec<DoctorItem>;
  /// The switch of the fixable item `id` in these inventories, or None.
  pub fn switch_of(inv: &[Inventory], id: &str) -> Option<Switch>;
  /// Shared by the readers: every `<dir>/<folder>/SKILL.md`, sorted by folder name. A missing dir is empty.
  pub fn skill_files(dir: &Path) -> Vec<(String, PathBuf)>;
  ```

**Frontmatter rules (`read`):**
- Strip a leading BOM, and read `\r\n` as `\n`.
- The block is the lines between a first line that is `---` (trimmed) and the next line that is `---`. Otherwise the result is `NoBlock`. This matches Codex's `extract_frontmatter`.
- Each top-level line `key: value` is read like this:
  - A value in matching single or double quotes is unquoted. For double quotes, `\"` and `\\` are unescaped.
  - A value of `|`, `>`, or either with `-` or `+`, or an empty value, takes the following lines that are indented more than the key. For `|` they are joined with `\n`; otherwise with a space.
  - An unquoted plain value continues on following indented lines, joined with a space.
  - `#` starts a comment only after whitespace in an unquoted value.
  - Keys other than `name`, `description`, `when_to_use` and `disable-model-invocation` are skipped together with their indented lines.
- A top-level line that isn't blank, a comment, or `key:` with an identifier-like key makes the result `Unclear`. Other cases that make it `Unclear`: an opening quote with no closing one, and flow syntax (`[`, `{`) in a field we read.
- `hidden` is true only for the value `true` (any case).

**The items (`items`), in this order:**
- Inventories are sorted by `(distro, CLI rank)`, as in `checks::items`.
- Within each inventory:
  1. unreadable files;
  2. Broken servers;
  3. Broken skills;
  4. long descriptions;
  5. Qwen Code's listing item;
  6. the cost line.
- Within each kind, items go by name.

An inventory with `found: false` gives nothing. A server or skill with `off` gives nothing. A skill that is `hidden` gives no long-description item. A `Bundled` item never gives Broken or long-description items, but its cost counts.

| Case | id | group | fixable | title | detail | cannot_fix | path |
|---|---|---|---|---|---|---|---|
| Unreadable file | `extfile.<agent>.<key(file, "")><@distro>` | Info | no | `<CLI>'s <file name><place> can't be read` | `<message> Its MCP servers and skills weren't checked.` | absent | file |
| Server, `Stdio` with `Missing` | `mcp.<agent>.<key><@distro>` | Broken | `switch` is `Ok` | `<CLI>'s MCP server <name><place> can't start` | `Its command, <command>, isn't on this PC.` | see below | file |
| Server, `Nothing` | as above | Broken | as above | as above | `Its entry has no command and no URL.` | see below | file |
| Skill `NoBlock` | `skill.<agent>.<key><@distro>` | Broken | `switch` is `Ok` | `<CLI>'s skill <name><place> doesn't load` | `Its SKILL.md doesn't start with a --- block that closes.` | see below | SKILL.md |
| Skill `Missing(f)` | as above | Broken | as above | as above | `Its SKILL.md has no <f>.` | see below | SKILL.md |
| Skill `BadName` | as above | Broken | as above | as above | `Its name has characters <CLI> refuses.` | see below | SKILL.md |
| Long description: `per_skill` is set and `listed_chars > per_skill` | `skilldesc.<agent>.<key><@distro>` | Waste | no | `<CLI>'s skill <name><place> has a description <CLI> cuts` | `It is <listed_chars> characters, and <CLI> keeps the first <per_skill>.` | `Shorten the description in its SKILL.md.` | SKILL.md |
| Listing over `listing_first_line_over` | `skilllisting.<agent><@distro>` | Waste | no | `<CLI><place> cuts its skills' descriptions to their first line` | `Its skill listing is <n> characters, over the <cap> it keeps in full.` | `Shorten the longest descriptions, or turn off skills you don't use.` | absent |
| Cost, at least one skill counted | `skillcost.<agent><@distro>` | Info | no | `<CLI>'s skills<place> add about <tokens> tokens to each session` | `<k> skills are listed, by name and description. The largest: <a> (<n>), <b> (<n>), <c> (<n>).` (as many as there are, up to 3, tokens each) | absent | absent |

- **Fixable items.**
  - A fixable Broken item adds ` The fix turns it off and doesn't delete it.` to its detail.
  - A fixable Broken item's `cannot_fix` is absent. A report-only one's comes from its `NoSwitch`:
    - `ClaudeUser`: `Claude Code can turn a user-level server off only per project. To remove it, run claude mcp remove <name> -s user.`
    - `Shared(f)`: `Only <f>, which the project shares, can turn it off. Marshell doesn't edit a project's shared files.`
    - `Plugin(p)`: `It comes from the plugin <p>, whose next update would undo a change.`
    - `NoSkillSwitch`: `<CLI> has no switch for one skill.`
    - `Bundled`: `It comes with <CLI>.`
- **Counting the cost.**
  - Each skill that isn't `off` or `hidden` counts its name's characters, plus `listed_chars` capped at `per_skill` (0 when `name_only`).
  - When `listing_first_line_over` is set and the non-bundled skills' total passes it, the non-bundled skills count `first_line_chars` instead.
  - Tokens are the character total divided by 4, rounded up. A skill's tokens are counted the same way.
  - `<n>` in the listing item is the uncut total of the non-bundled skills.

- [ ] **Step 1: Write the failing tests.** In `frontmatter.rs`'s `mod tests`:
  - `plain_keys_are_read`: `"---\nname: pdf\ndescription: Reads PDFs: text and tables\n---\nbody"` gives `Read` with both values. The `:` inside the value is kept.
  - `block_and_folded_scalars`: `description: >-\n  one\n  two\n` gives `"one two"`, and `|` gives `"one\ntwo"`.
  - `quoted_values_with_hash_and_colon`: `description: "a: b # c"` gives `a: b # c`, and `description: a # note` gives `a`.
  - `crlf_and_bom`: the first test's text with `\r\n` and a leading `\u{feff}` gives the same `Read`.
  - `continued_plain_value`: `description: one\n  two\nname: x` gives `"one two"` and `x`.
  - `no_block_cases`: no leading `---`, no closing `---`, and `---\n---` each give `NoBlock`.
  - `unclear_cases`: `description: [a, b]`, a line `- item` at top level, and `description: "open` each give `Unclear`, keeping what was read.
  - `hidden_flag`: `disable-model-invocation: true` gives `hidden`, and `false` doesn't.

  In `ext.rs`'s `mod tests`, with a `fn inv(agent) -> Inventory` builder:
  - `missing_command_is_broken_and_fixable_with_switch`: a Codex `Stdio` server with `Missing` and an `Ok` switch gives one Broken `mcp.codex.<key>` item. It has `fixable: true`, the exact title and detail above, and `path` set.
  - `no_switch_reasons_are_the_cannot_fix_copy`: each `NoSwitch` gives a report-only item with its exact `cannot_fix`.
  - `off_found_and_bundled_rules`: an `off` server, an `off` skill, `found: false`, and a `Bundled` skill with `NoBlock` give no item.
  - `long_description_uses_the_cap`: Claude, with `per_skill: 1536` and `listed_chars` of 1,537, gives a Waste `skilldesc.claude.*` item with `It is 1537 characters, and Claude Code keeps the first 1536.` 1,536 gives none, and a `hidden` skill gives none.
  - `qwen_listing_cut`: non-bundled skills totalling 8,001 characters with `listing_first_line_over: 8000` give the `skilllisting.qwen` item. The cost then counts the first lines.
  - `cost_line`: three skills (names `a`, `bb`, `ccc`; `listed_chars` 7, 10 and 0; one with `name_only`) give the exact token sum, rounded up, and the largest three in descending order. An `off` skill isn't counted.
  - `distro_ids_and_titles`: with `distro: Some("Ubuntu")`, ids end in `@Ubuntu` and titles say ` in Ubuntu`.
  - `switch_of_finds_only_fixable_items`: it gives the `Switch` for a fixable item's id, and `None` for a report-only id or an unknown id.
  - `key_is_stable`: the same file and name give the same 8 hex digits, and another name gives other digits.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-core doctor::frontmatter doctor::ext`. Expected: FAIL, because nothing is defined yet.
- [ ] **Step 3:** Implement `frontmatter::read`, then everything in `ext.rs`, to the rules and table above. `skill_files` lists the subfolders of `dir` that hold a `SKILL.md`. It skips folders starting with `.`, which the readers handle themselves.
- [ ] **Step 4:** Run the same command. Expected: PASS.
- [ ] **Step 5:** Run `cargo fmt --all`, then commit `feat(doctor): MCP servers and skills become Doctor items, read from a small frontmatter reader`.

### Task 2: Claude Code's and Qwen Code's readers

**Files:**
- Create: `crates/core/src/agents/claude/extensions.rs` (add `pub mod extensions;` to `agents/claude/mod.rs`)
- Create: `crates/core/src/agents/qwen/extensions.rs` (add `pub mod extensions;` to `agents/qwen/mod.rs`)

**Interfaces:**
- Consumes: Task 1's types, `frontmatter::read`, `ext::skill_files`, and `crate::config::jsonc::read_value` (JSONC text to a value; an empty or all-whitespace text is no servers, not an error).
- Produces, in each file:
  ```rust
  /// `command` judges a stdio server's command. `projects` are the folders to read project files in.
  pub fn inventory(roots: &ExtRoots, projects: &[PathBuf], command: &dyn Fn(&str) -> CommandState) -> Inventory;
  ```
  It returns `distro: None` and `found: true`; the caller sets both.

**Claude Code** (`caps: per_skill 1536`):
- **Servers.**
  - **User level:** top-level `mcpServers` in `claude_json`. Origin `User`, switch `Err(ClaudeUser)`. It is never `off`.
  - **Local scope:** `projects[<key>].mcpServers` in `claude_json`, for each key that matches a project.
    - A key matches when, with `\` read as `/`, a trailing `/` dropped and the case ignored, it equals the project path.
    - Origin `Project(p)`.
    - `off` when `projects[<key>].disabledMcpServers` holds the name.
    - Switch `JsonAdd { file: claude_json, pointer: "/projects/<key escaped>/disabledMcpServers" }`.
  - **Project scope:** `<p>/.mcp.json` `mcpServers`. Origin `Project(p)`.
    - `off` when `disabledMcpjsonServers` holds the name in any of these:
      - `<p>/.claude/settings.local.json`;
      - `<p>/.claude/settings.json`;
      - `claude_settings`;
      - the project's entry in `claude_json`.
    - Switch `JsonAdd { file: <p>/.claude/settings.local.json, pointer: "/disabledMcpjsonServers" }`.
  - **Plugins:**
    - The enabled ones are each `enabledPlugins` key whose value is `true` in `claude_settings`.
    - Each is looked up in `<claude_dir>/plugins/installed_plugins.json` `plugins[<key>][0].installPath`, and its servers are read from `<installPath>/.mcp.json`.
    - Origin `Plugin(<key>)`, switch `Err(Plugin(<key>))`.
  - **Transport:**
    - `type` of `http`, `streamable-http` or `sse`, or no `command` with a `url`, is `Remote`.
    - A `command` is `Stdio`, judged by `command`.
    - Neither is `Nothing`.
- **Skills.**
  - **User level:** `<claude_dir>/skills/*`. Origin `User`.
    - `off` when `claude_settings`'s `skillOverrides[name]` is `"off"`.
    - Switch `JsonSet { file: claude_settings, pointer: "/skillOverrides/<name escaped>", value: "\"off\"" }`.
  - **Project:** `<p>/.claude/skills/*`. Origin `Project(p)`.
    - `off` when the override is `"off"` in `<p>/.claude/settings.local.json`, `<p>/.claude/settings.json` or `claude_settings`.
    - Switch `JsonSet` on `<p>/.claude/settings.local.json`.
  - **Plugin:** `<installPath>/skills/*`, named `<plugin name before @>:<skill>`. Switch `Err(Plugin(<key>))`.
  - **Fields:**
    - `name` is the frontmatter `name`, else the folder name.
    - `name_only` when the override is `"name-only"`.
    - `listed_chars` is the character count of `description` plus `when_to_use`. When there's no description, it is the first paragraph after the block.
    - `state` is always `Ok`.

**Qwen Code** (`caps: listing_first_line_over 8000`):
- **The settings file:** `<qwen_dir>/settings.json`.
- **Servers.**
  - **User level:** its `mcpServers`, origin `User`.
  - **Project:** `<p>/.qwen/settings.json` `mcpServers` and `<p>/.mcp.json` `mcpServers`, origin `Project(p)`.
  - **Extensions:** `<qwen_dir>/extensions/<ext>/qwen-extension.json` `mcpServers`, when it is an object, origin `Plugin(<ext>)`. Skip any extension that `extensions/extension-enablement.json` marks as disabled.
  - **`off`:** the name matches an entry of `mcp.excluded` in the user's or the project's settings. A `*` in the entry matches any run of characters; otherwise the match is exact.
  - **Switch:** `JsonAdd { file: <qwen_dir>/settings.json, pointer: "/mcp/excluded" }` for user and project servers, and `Err(Plugin)` for extensions.
  - **Transport:** `httpUrl` or `url` is `Remote`, and `command` is `Stdio`.
- **Skills.**
  - **Folders:** `<qwen_dir>/skills/*` and `<home>/.agents/skills/*` (origin `User`); `<p>/.qwen/skills/*` and `<p>/.agents/skills/*` (origin `Project(p)`); and `<ext>/skills/*` (origin `Plugin(<ext>)`, named `<ext>:<name>`).
  - **`off`:** `skills.disabled` in the user's or the project's settings holds the name, case-insensitive. An extension skill also matches by its bare name.
  - **Switch:** `JsonAdd { file: <qwen_dir>/settings.json, pointer: "/skills/disabled" }`. Plugin skills get `Err(Plugin)`.
  - **`state`:**
    - `NoBlock` stays `NoBlock`.
    - `Read` with no non-empty `name` is `Missing("name")`, and with no non-empty `description` is `Missing("description")`.
    - A name that fails `^[\p{L}\p{N}_:.-]+$` is `BadName`. Check it with `char::is_alphanumeric` plus `_:.-`; no regex dependency.
    - `Unclear` is `Ok`.
  - **`listed_chars`** is the description's length.

**Both readers:**
- **Unreadable files.** A file that exists and doesn't parse goes in `unreadable` with its message, and is skipped.
- **Pointers.** A pointer segment escapes `~` as `~0` and `/` as `~1`.

- [ ] **Step 1: Write the failing tests.** In each file's `mod tests`, build a fake home under `target/test-ext-<name>` with `ExtRoots` pointing into it.

  Claude:
  - `claude_scopes_and_switches`: one user server, one local server under the key `"C:/x/proj"` with project path `C:\x\proj`, one `.mcp.json` server and one plugin server. Each gets the origin and switch above, and the local pointer is `/projects/C:~1x~1proj/disabledMcpServers`.
  - `claude_off_lists`: a local server in `disabledMcpServers`, and a `.mcp.json` server in a `settings.local.json` `disabledMcpjsonServers`, are `off`.
  - `claude_skills`:
    - A user skill with no frontmatter is `Ok`, with `listed_chars` equal to the first paragraph's length and its name taken from the folder.
    - `skillOverrides` `"off"` and `"name-only"` give `off` and `name_only`.
    - A plugin skill is named `plug:skill`.
  - `claude_transports`: `type: "sse"` and a bare `url` are `Remote`, `{}` is `Nothing`, and the closure's answer reaches `Stdio.state`.
  - `claude_unreadable_json`: a `.mcp.json` of `{` is in `unreadable` and gives no server.

  Qwen:
  - `qwen_servers_and_globs`: an `mcp.excluded` of `["gh*"]` makes `github` `off` and leaves `gitlab` alone. Extension servers get `Plugin`. User and project servers get the `/mcp/excluded` switch on the user file.
  - `qwen_skill_rules`: no `name` gives `Missing("name")`; `name: "a b"` gives `BadName`; `name: 日本:x` gives `Ok`; and an extension skill is `ext:x`, `off` when `skills.disabled` holds `x`.
  - `qwen_empty_settings_file`: an empty `settings.json` gives zero servers and nothing in `unreadable`.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-core agents::claude::extensions agents::qwen::extensions`. Expected: FAIL.
- [ ] **Step 3:** Implement both `inventory` functions to the rules above.
- [ ] **Step 4:** Run the same command. Expected: PASS.
- [ ] **Step 5:** Run `cargo fmt --all`, then commit `feat(doctor): Claude Code's and Qwen Code's MCP servers and skills are read with their off switches`.

### Task 3: Codex's and Antigravity's readers

**Files:**
- Create: `crates/core/src/agents/codex/extensions.rs` (add `pub mod extensions;` to `agents/codex/mod.rs`)
- Create: `crates/core/src/agents/antigravity/extensions.rs` (add `pub mod extensions;` to `agents/antigravity/mod.rs`)

**Interfaces:** the same `inventory` signature as Task 2.

**Codex** (`caps: per_skill 1024`):
- **The config file:** `<codex_home>/config.toml`, parsed with `toml_edit::DocumentMut`. A missing file is empty.
- **Servers.**
  - **User level:** each `[mcp_servers.<name>]`, origin `User`.
    - `off` when `enabled = false`.
    - Switch `CodexServer { file: <codex_home>/config.toml, name }`.
  - **Project:** `<p>/.codex/config.toml` servers, origin `Project(p)`, switch `Err(Shared(<that file>))`.
  - **Plugins:**
    - The enabled ones are each `[plugins."<plugin>@<market>"]` with `enabled = true`.
    - The folder is `<codex_home>/plugins/cache/<market>/<plugin>/latest` when it exists, else the version subfolder that sorts last.
    - Its servers are read from `.mcp.json` `mcpServers`, origin `Plugin(<plugin>@<market>)`.
  - **Transport:** `url` is `Remote`, and `command` is `Stdio`.
- **Skills.**
  - **Folders:**
    - origin `User`: `<home>/.agents/skills/*` and `<codex_home>/skills/*` (folders not starting with `.`);
    - origin `Bundled`: `<codex_home>/skills/.system/*`;
    - origin `Project(p)`: `<p>/.agents/skills/*`;
    - origin `Plugin`: `<plugin folder>/skills/*`, named `<plugin>:<name>`.
  - **`off`:** the last `[[skills.config]]` entry whose `path` equals the SKILL.md's path (compared case-insensitively, with `\` as `/`), or whose `name` equals the name, has `enabled = false`.
  - **Switch:** `CodexSkill { file: <codex_home>/config.toml, skill: <SKILL.md path> }` for `User` and `Project`, and `Err(Bundled)` or `Err(Plugin)` otherwise.
  - **`state`:** `NoBlock` stays `NoBlock`; `Read` with no non-empty `description` is `Missing("description")`; `Unclear` is `Ok`.
  - **`name`** is the frontmatter `name`, else the folder name. **`listed_chars`** is the description's length.

**Antigravity** (`caps` none):
- **The config folder:** `<home>/.gemini/config`.
- **Servers.**
  - **User level:** `<config>/mcp_config.json` `mcpServers`, origin `User`.
    - `off` when `"disabled": true`.
    - Switch `JsonSet { file, pointer: "/mcpServers/<name escaped>/disabled", value: "true" }`.
    - A 0-byte or all-whitespace file is empty.
  - **Project:** `<p>/.agents/mcp_config.json`, origin `Project(p)`, switch `Err(Shared(file))`.
  - **Plugins:** `<config>/plugins/<dir>/mcp_config.json`, origin `Plugin(<dir>)`. Skip a plugin that `<config>/config.json` marks `plugins.<dir>.enabled == false`.
  - **Transport:** `serverUrl` or `url` is `Remote`, and `command` is `Stdio`.
- **Skills.**
  - **Folders:**
    - origin `User`: `<config>/skills/*`, `<home>/.gemini/antigravity-cli/skills/*` and `<home>/.gemini/skills/*`;
    - origin `Bundled`: `<home>/.gemini/antigravity-cli/builtin/skills/*`;
    - origin `Project(p)`: `<p>/.agents/skills/*` and `<p>/.agent/skills/*`;
    - origin `Plugin`: `<config>/plugins/<dir>/skills/*`.
  - **Switch:** `Err(NoSkillSwitch)` for every skill, except `Err(Bundled)` and `Err(Plugin)` where those apply.
  - **`state`:** `NoBlock`; `Missing("name")`; `Missing("description")`; and `Unclear` is `Ok`.

- [ ] **Step 1: Write the failing tests** (a fake home under `target/test-ext-<name>`).

  Codex:
  - `codex_servers_off_and_switch`: `[mcp_servers.a]` with `command`, `[mcp_servers."my.server"]` with `enabled = false`, and a project server. `a` gets `CodexServer`, `my.server` is `off`, and the project server gets `Shared`.
  - `codex_plugins_pick_latest`: an enabled plugin with folders `1.0`, `2.0` and `latest` reads `latest/.mcp.json`. A disabled plugin is skipped.
  - `codex_skills`:
    - a `.system` skill is `Bundled`;
    - a `[[skills.config]]` entry with `path` (spelled with `\`) and `enabled = false` makes that skill `off`;
    - a later entry with `enabled = true` makes it on again;
    - no description gives `Missing("description")`.
  - `codex_empty_config`: no `config.toml` gives zero servers. A `config.toml` that doesn't parse goes in `unreadable`.

  Antigravity:
  - `agy_zero_byte_config`: a 0-byte `mcp_config.json` gives zero servers and nothing in `unreadable`.
  - `agy_jsonc_and_disabled`: a file with a comment and a trailing comma is read. `"disabled": true` is `off`. The switch pointer for the server `a/b` is `/mcpServers/a~1b/disabled`.
  - `agy_skills_have_no_switch`: a user skill gets `Err(NoSkillSwitch)`, and a builtin one gets `Bundled`.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-core agents::codex::extensions agents::antigravity::extensions`. Expected: FAIL.
- [ ] **Step 3:** Implement both to the rules above.
- [ ] **Step 4:** Run the same command. Expected: PASS.
- [ ] **Step 5:** Run `cargo fmt --all`, then commit `feat(doctor): Codex's and Antigravity's MCP servers and skills are read with their off switches`.

### Task 4: Wiring: the inventories in every report, and the turn-off fix

**Files:**
- Create: `crates/core/src/server/doctor_ext.rs` (add `mod doctor_ext;` in `server/mod.rs`)
- Modify:
  - `crates/core/src/server/doctor.rs`: `Gathered`, `facts_of`, `run_once`, `fix_of`, `is_own_edit`'s use.
  - `crates/core/src/server/mod.rs`: an `ext_home: Option<PathBuf>` field, and `pub fn with_ext_home(self, home: PathBuf) -> Self`, like `with_qwen_home`.

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces (`doctor_ext.rs`):
  ```rust
  /// Windows roots. With `ext_home` set by a test, every CLI folder is at its default under it and the environment
  /// isn't read; `claude_settings` is the pinned `claude_settings` when set, else `<ext_home>/.claude/settings.json`.
  /// Otherwise the user home with CLAUDE_CONFIG_DIR, CODEX_HOME and QWEN_HOME, and `claude_settings_path`.
  pub(crate) fn windows_roots(st: &AppState) -> Option<ExtRoots>;
  /// Every CLI's inventory on Windows (with Marshell's recent projects), then in each running managed distro
  /// (user level only, commands NotChecked). Reads files: callers are already off the async threads.
  pub(crate) fn inventories(st: &AppState, facts: &[AgentFacts]) -> Vec<Inventory>;
  /// The edits that turn off the picked items, grouped as below.
  pub(crate) fn edits(switches: &[Switch]) -> Result<Vec<FileEdit>, Fail>;
  ```
- `Gathered` gains `pub ext: Vec<Inventory>`.
- **`inventories`:**
  - `found` and `distro` come from the matching `AgentFacts`. An agent whose facts have an `error` gets no inventory.
  - A distro's roots use `DistroTarget.home`, with every CLI folder at its default under it and `claude_settings` at `<home>/.claude/settings.json`.
  - The Windows command judge:
    - a command holding `\` or `/` that is absolute is `Found` when the file exists, else `Missing`;
    - a relative one is `NotChecked`;
    - a bare name is `Found` or `Missing` by `st.on_path`.

**Edits (`edits`):**
- **`JsonAdd`**, grouped by `(file, pointer)`:
  - Read the file now. A missing file gives `expected_hash: None` and an empty array.
  - Take the array at `pointer`. A missing key starts an empty array. A value that isn't an array refuses with 409 `CANT_FIX`.
  - Append each name not already in it, in the switches' order.
  - Emit one `JsonSet { pointer, value: <the array as JSON text> }`.
- **`JsonSet`:** one edit each, with the current hash, or `None` for a missing file.
- **`CodexServer` and `CodexSkill`**, grouped by `file`:
  - Parse the current text with `toml_edit`, and use `CreateFile` when the file is missing.
  - Set `mcp_servers.<name>.enabled = false`. It refuses with 409 when `mcp_servers.<name>` isn't a table.
  - Append a `[[skills.config]]` table `{ path = "<SKILL.md>", enabled = false }` for each skill.
  - Emit one `ReplaceText` with the new text and the current hash.
- **Every edit:** `scope: ConfigScope::User` for a file under the user home, else `ConfigScope::Local`; `applies: Applies::NewSessions`.

**`fix_of` and `run_once`:**
- Both build items as `checks::items(...)`, then `ext::items(&gathered.ext)`, then (`run_once` only) the CLI doctor items.
- In `fix_of`, ids whose kind (the text before the first `.`) is `mcp` or `skill` take `ext::switch_of(&gathered.ext, id)`. `None` refuses with `cant_fix()`. Their switches go through `edits` once, after the loop.
- The `own` closure passed to `preview_mixed` is true for retention's pointer and for every edit `edits` made.

- [ ] **Step 1: Write the failing tests** in `doctor_ext.rs`'s `mod tests`, and in `server/doctor.rs`'s tests for the route-level ones. Use a fake home with `with_ext_home` (and the existing test pins that make each CLI `found`, as 7a's tests do), a pinned `on_path` swap, and a `projects.json` written with `crate::projects::record`.
  - `json_adds_merge_per_list`:
    - two Qwen servers and one Qwen skill, with an existing `mcp.excluded: ["old"]`, give exactly two `JsonSet` edits on that one file;
    - `/mcp/excluded` is `["old","a","b"]` and `/skills/disabled` is `["s"]`;
    - both carry the file's hash.
  - `codex_server_and_skills_are_one_replace`:
    - a server named `my.server` and two skills give one `ReplaceText`;
    - its text parses with `[mcp_servers."my.server"] enabled = false` and two `[[skills.config]]` tables;
    - the comments of the original text are kept.
  - `codex_skill_with_no_config_creates_it`: with no `config.toml`, a Codex skill switch gives one `CreateFile`.
  - `non_array_refuses`: `mcp.excluded: "x"` gives 409.
  - `fix_apply_and_undo_round_trip` (server tests, end to end through the existing routes):
    - **Seed:**
      - a Codex server with the missing command `ghost`;
      - a Qwen skill with no description;
      - a Claude `.mcp.json` server in a recent project.
    - **Run:** the report has three Broken items, each `fixable`. POST `/v1/doctor/fix` with their ids, then apply.
    - **After apply:**
      - `config.toml` has `enabled = false`;
      - Qwen's `skills.disabled` has the name;
      - `<project>/.claude/settings.local.json` has `disabledMcpjsonServers`;
      - the project's `.mcp.json` is byte-for-byte unchanged;
      - `managed.json` has no new key;
      - the next report has none of the three.
    - **Undo:** the three files are byte-for-byte as before.
  - `report_only_items_are_refused`: a fix request for a `Plugin`, `Shared` or `ClaudeUser` item's id is refused with `cant_fix`.
  - `distro_inventory_has_no_projects_and_no_command_check`: an inventory in a pinned running distro reads the distro home's user files only, and its stdio servers are `NotChecked`.
  - `an_agent_that_failed_to_read_has_no_inventory`.
- [ ] **Step 2:** Run `cargo test -j 3 -p marshell-core doctor_ext server::doctor`. Expected: the new tests FAIL.
- [ ] **Step 3:** Implement `doctor_ext.rs` and the changes to `doctor.rs` and `mod.rs` above.
- [ ] **Step 4:** Run `cargo test -j 3 -p marshell-core doctor`. Expected: PASS. 7a's and 7b's tests also stay green.
- [ ] **Step 5:** Run `cargo fmt --all` and `cargo clippy -j 3 -p marshell-core --all-targets -- -D warnings`, then commit `feat(doctor): the Doctor reports MCP servers and skills, and turns broken ones off with undo`.

### Task 5: Live check, research note and checks (the lead)

- [ ] **Step 1: Seed the scratch folders.** Use the `marshell-live-check` skill, with scratch `MARSHELL_HOME`, `MARSHELL_CLAUDE_SETTINGS` and `CLAUDE_CONFIG_DIR`. Also start the app with scratch `CODEX_HOME` and `QWEN_HOME`. Antigravity's real `~/.gemini` is read only; no Antigravity fix is applied in the live check.
  - **Claude Code:** a scratch project in Marshell's recent list, holding:
    - a `.mcp.json` server whose command is `ghost-mcp`;
    - a local-scope server in the scratch `.claude.json`;
    - a user skill whose description is over 1,536 characters.
  - **Codex:** a `config.toml` server `ghost` with `command = "ghost-mcp"`, and a skill with no description.
  - **Qwen Code:** a user skill named `a b`.
- [ ] **Step 2: Check the page.** Over CDP, the Doctor page shows:
  - the Broken items, pre-checked;
  - the Waste long-description item, unchecked and report-only;
  - the cost lines.

  "Fix 3 issues" (or the count that's there) previews one diff per file. Apply. The files change as Task 4's round-trip says, and Undo puts them back. Record the counts.
- [ ] **Step 3: The `~/.claude.json` check (decision 8).**
  1. Apply the local-scope server's fix.
  2. Start a Claude Code session in the scratch project, with the scratch `CLAUDE_CONFIG_DIR`. `/mcp` shows the server off.
  3. Exit the session, then read the scratch `.claude.json`.
  4. If the key is gone, change the Claude reader's local-scope switch to report-only, with a new `NoSwitch::ClaudeLocal` and the copy `Turn it off in Claude Code's /mcp.`, plus its test, and record that as an amendment to decision 8.
- [ ] **Step 3b:** Read this PC's real configs, without fixing anything. Note what the report says about the real servers and skills: the counts per CLI, the cost lines, and any item that looks wrong. A wrong item is a bug to fix before the PR.
- [ ] **Step 4:** Write `docs/research/2026-11-06-doctor-7c1-live-check.md` with what was seen, then stop what was started, by PID, per the skill.
- [ ] **Step 5:** Run what CI runs:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -j 3 -- -D warnings`;
  - `cargo test --workspace -j 4`;
  - `pnpm test`;
  - `pnpm check:generated`;
  - `pnpm build`.

  Then the final whole-branch review on Opus (`marshell-reviewer`), one fix pass, and `marshell-ship`.

## Known limits (for the PR)

These are left for later on purpose:
- Claude Code's `managed-mcp.json` and plugin servers declared inline in `plugin.json`.
- Qwen Code's bundled skills and its project `.mcp.json` approvals.
- A command checked inside a distro.
- Relative command paths.
- Usage and the MCP cost are 7c2's.
