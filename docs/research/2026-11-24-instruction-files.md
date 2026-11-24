# Instruction files (memory/context files) in the five agent CLIs

Date 2026-11-24, Windows 11. Nothing was started interactively, no prompt was sent, no CLI config or repo file was changed.
Labels: **[local]** = verified in the installed binary/package or by a read-only command; **[doc URL]** = documented; **[src URL]** = upstream source; **[unverified]**.

## Installed versions (`<cli> --version`)

| CLI | Binary | Version |
|---|---|---|
| Claude Code | `~\.local\bin\claude.exe` | 2.1.331 |
| Codex CLI | `...\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe` | codex-cli 0.171.0 |
| Antigravity | `...\AppData\Local\agy\bin\agy.exe` | 1.5.14 |
| Qwen Code | `...\AppData\Local\qwen-code\bin\qwen.cmd` (node bundle in `qwen-code\lib\chunks`) | 0.26.5 |
| GitHub Copilot CLI | `...\VS Code\bin\copilot-shim\copilot.exe` -> `...\AppData\Local\copilot\pkg\win32-x64\1.0.106\cli-main.js` | 1.0.106 |

## Conflicts with what the repo recorded

1. **Claude Code now reads `AGENTS.md`** (v2.1.277+, built-in plugin `cc-plugin-agents-md`). The premise "Claude Code does NOT read AGENTS.md" is outdated. Default: AGENTS.md is read only when no CLAUDE.md / .claude/CLAUDE.md / CLAUDE.local.md exists in cwd or above (the user's `~/.claude/CLAUDE.md` does not count). See the Claude section.
2. `docs/research/2026-11-06-copilot-cli.md` Q8 says "`@path` imports work". Official doc narrows it: only in `copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`; not in `GEMINI.md` or `*.instructions.md`; targets must be inside the repo (or custom instructions dir); absolute and `~/` paths are not loaded.
3. `docs/research/codex-cli.md` says Codex "stops adding files at 32 KiB". Source shows more exactly: the file that crosses the budget is truncated to the remaining bytes (not refused), later files are skipped, each with a warning. Also new: an untrusted project, or `project_doc_max_bytes = 0`, loads no project docs.
4. Claude Code docs say import depth "four hops"; the binary constant is 5 (probably depth counted including the root file). Treat max depth as 4-5; unverified which.

---

## 1. Claude Code 2.1.331

Sources: [doc https://code.claude.com/docs/en/memory.md]; strings/source in `claude.exe` [local].

1. **Names.**
   - Managed: Windows `C:\Program Files\ClaudeCode\CLAUDE.md`, Linux/WSL `/etc/claude-code/CLAUDE.md`, macOS `/Library/Application Support/ClaudeCode/CLAUDE.md` [doc].
   - User: `~/.claude/CLAUDE.md`, rules in `~/.claude/rules/**/*.md` [doc]. `CLAUDE_CONFIG_DIR` replaces `~/.claude` [local: help text "CLAUDE_CONFIG_DIR or ~/.claude"; doc].
   - Project: `./CLAUDE.md` or `./.claude/CLAUDE.md` [doc, local: `uGn` returns both]; `./.claude/rules/**/*.md` recursive [doc, local `DWe`].
   - Local/private: `./CLAUDE.local.md` (gitignore it) [doc, local].
   - `AGENTS.md` and `.claude/AGENTS.md` as project instructions (see section 7). `AGENTS.local.md`, `AGENTS.override.md`, `.agents/` are NOT read [doc].
   - No setting renames CLAUDE.md. `claudeMd` setting injects org text, managed settings only [local schema].
2. **Discovery.**
   - At launch: CLAUDE.md and CLAUDE.local.md in cwd and every directory above it (to filesystem root, not only git root); ordered root -> cwd, CLAUDE.local.md after CLAUDE.md in each directory [doc].
   - On demand: CLAUDE.md / CLAUDE.local.md in subdirectories below cwd, loaded once Claude reads/writes/edits a file there [doc]. Path-scoped rules also on demand.
   - `--add-dir` directories: not loaded unless `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1` (then CLAUDE.md, .claude/CLAUDE.md, CLAUDE.local.md, unscoped rules; not their AGENTS.md) [doc; env name in binary, local].
   - Auto memory `~/.claude/projects/<project>/memory/MEMORY.md`: first 200 lines or 25 KB load at start; topic files on demand [doc].
   - After `/compact`, root CLAUDE.md is re-read from disk [doc].
3. **Limits.**
   - Target under 200 lines per CLAUDE.md; a startup warning and a `/status` warning appear when a file is over the recommended length, and another when files that are each within it add up past a combined limit. Each CLAUDE.md, rules file and import counts separately [doc]. The numeric thresholds are not stated [unverified].
   - A file over 4 MiB is skipped entirely (loads none of it) [doc].
   - On-demand files (subdirectory CLAUDE.md, subdirectory rules, path-scoped rules, their imports): per-file cap 40,000 characters on a 200K window; 200,000 on 1M for Opus 4.6/Sonnet 4.6, 150,000 on later models; over it Claude gets the start plus a note to read the rest (since v2.1.309) [doc]. Binary also has "this session's instruction files are too long together to send whole" with a cut-for-room message [local]: a combined cut exists.
   - HTML block comments `<!-- -->` are stripped before injection [doc], so they cost no tokens.
4. **Imports.** `@path` anywhere outside code spans/fences; relative paths resolve from the importing file; absolute and `~/` allowed; backslash-escape spaces; quoted paths are not imported; max depth 4 hops [doc] (constant 5 in binary [local]). Imported files load with the importing file, so tokens are added at start. Imports outside the working directory trigger a one-time approval dialog per project (project-level files only; user-scope files do not ask); if declined they stay off [doc]. Missing import: the loader returns nothing for a missing file and does not report it in the UI (only debug log) [local, code reading of `Hle`/`iSt`]; treat as silent.
5. **Path-scoped rules.** `.claude/rules/*.md` and `~/.claude/rules/*.md`; frontmatter `paths:` (YAML list or comma string; braces expand, budget 1,000 patterns/4 MiB); `paths` is the only frontmatter key read. No `paths` = loads at launch with .claude/CLAUDE.md priority; with `paths` = loads when Claude reads/edits a matching file [doc, local].
6. **Switches.** `claudeMdExcludes` (array of globs/absolute paths, picomatch, matched on absolute paths, applies to User/Project/Local, not Managed; arrays merge across layers) [doc, local]. `CLAUDE_CODE_DISABLE_CLAUDE_MDS` env [local]. `--setting-sources` without `project` skips project files and rules [doc]. `--bare` skips auto-memory etc. [local help]. Plugin option `instructionFiles` = `managed-only` leaves out project/local/user CLAUDE.md, rules and AGENTS.md [doc]. `autoMemoryEnabled:false` / `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` for MEMORY.md [doc, local].
7. **Other CLIs' files.** Reads `AGENTS.md` / `.claude/AGENTS.md` (v2.1.277+). Setting "Project instructions" in `/config`, stored as `pluginConfigs["cc-plugin-agents-md@builtin"].options.instructionFiles` in `~/.claude/settings.json`, `--settings` file or managed settings (ignored in project/local settings) [doc; binary confirms plugin id `cc-plugin-agents-md`, option values `claude-md`, `claude-md-or-agents-md` (default), `claude-md-and-agents-md`, `managed-only`, feature gate `tengu_agents_md_mod` default true, and a legacy `projectInstructions` key none/claude/agents-fallback/both [local]].
   - Default `claude-md-or-agents-md`: AGENTS.md is read only if no CLAUDE.md-type file exists in cwd or above. Nested subdirectory AGENTS.md loaded when a file there is read and the subdirectory has no CLAUDE.md of its own. `@path` imports and `claudeMdExcludes` apply inside AGENTS.md [doc].
   - `claude-md-and-agents-md`: both, CLAUDE.md first per directory; an AGENTS.md already loaded (imported or symlinked) is not read twice [doc].
   - Does NOT read GEMINI.md, `.github/copilot-instructions.md`, QWEN.md (only `/init` and `/import` copy from other tools) [doc].
   - Unavailable (CLAUDE.md only): before v2.1.277, built-in plugin disabled via `/plugin`, some first-after-upgrade sessions [doc].
   - Caveat for a Doctor: a `CLAUDE.local.md` makes Claude skip AGENTS.md entirely (default mode).
8. **User-level path.** Windows `%USERPROFILE%\.claude\CLAUDE.md` or `%CLAUDE_CONFIG_DIR%\CLAUDE.md`; WSL `~/.claude/CLAUDE.md` (separate home inside the distro) [doc, local]. Present on this PC: `~/.claude/CLAUDE.md`; no `~/.claude/rules`.

Other: `/doctor prompt-audit` (v2.1.283+) audits CLAUDE.md, CLAUDE.local.md, AGENTS.md, rules, skills, etc. for stale refs and contradictions [doc]; `/memory` and `/context` list loaded files [doc]. Hook `InstructionsLoaded` fires for CLAUDE.md, not for AGENTS.md read via the setting [doc].

---

## 2. Codex CLI 0.171.0

Sources: [src https://github.com/openai/codex/blob/main/codex-rs/core/src/agents_md.rs] (read in full through the GitHub API; the installed binary contains the same strings, [local] `core\src\agents_md.rs`, `agents_md_manager.rs`); [doc https://learn.chatgpt.com/docs/agent-configuration/agents-md]; repo notes `docs/research/codex-cli.md`, `2026-10-15-local-install-check.md`.

1. **Names.** Global (in `CODEX_HOME`, default `~/.codex`): `AGENTS.override.md` if it exists, else `AGENTS.md` (first non-empty one) [doc, local]. Per project directory: `AGENTS.override.md`, then `AGENTS.md`, then names in `project_doc_fallback_filenames` (config.toml, default `[]`); at most one file per directory; fallback entries with path syntax are ignored; entries not on the list are ignored [src, doc, local string "ignoring project_doc_fallback_filenames entry that is not a filename"]. `AGENTS.local.md` does not exist (0 hits) [local]. No `CLAUDE.md`/`GEMINI.md` reading (CLAUDE.md appears only in strings of other features) [local; doc silent].
2. **Discovery.** Project root = walk up from cwd until a `project_root_markers` entry (default `.git`; config key; empty list disables parent traversal). No marker found: only cwd. Collect from root down to cwd inclusive, concatenated root -> leaf, joined with `\n\n--- project-doc ---\n\n` from the user doc [src]. Does not walk past the root. Files in subdirectories below cwd are NOT loaded at start; the system prompt only tells the model to check for AGENTS.md when working in a subdirectory of cwd [local string "When working in a subdirectory of CWD ... check for any AGENTS.md files"; doc inference]. Everything above is loaded at session start.
3. **Limits.** `project_doc_max_bytes`, default 32768 [src/local: "32768" and `project_doc_max_bytes = 32768`]. Applies to project files only; global instructions are not counted, but a warning "Combined user instructions use N bytes, above the N-byte `project_doc_max_bytes` setting. The total includes global and thread instructions" exists [local]. Behaviour [src]: budget is spent root -> leaf; the file that crosses it is truncated to the remaining bytes (warning "Loaded only X of Y bytes from `path` ..."); remaining files are skipped (warning "Skipped project instructions ... budget was exhausted"). Empty files are skipped. Bytes, not tokens.
4. **Imports.** None documented, none in source. A literal `@file` is plain text [doc silent; src has no import code]. Stale paths in AGENTS.md can only be text references.
5. **Frontmatter / scoped rules.** None. Scope is the directory tree of the file (prompt text: more deeply nested wins) [local].
6. **Switches.** `project_doc_max_bytes = 0` loads no project docs; untrusted project (`active_project.is_untrusted()`) loads none [src]. `-c key=value` and `--strict-config` exist [local --help].
7. **Other CLIs' files.** Only via `project_doc_fallback_filenames = ["CLAUDE.md"]` etc. (user choice; filenames only, no paths). Default: none.
8. **User-level path.** Windows `%USERPROFILE%\.codex\AGENTS.md` (or `%CODEX_HOME%`); WSL `~/.codex/AGENTS.md` [doc]. Present on this PC: `~/.codex/AGENTS.md`; no override file; no `project_doc*` keys in config.toml (so 32 KiB default).

---

## 3. Antigravity CLI (agy) 1.5.14

Sources: docs shipped with agy at `%USERPROFILE%\.gemini\antigravity-cli\builtin\skills\agy-customizations\{SKILL.md,docs\rules.md,docs\plugins.md}` [local, same text embedded in `agy.exe`]; changelog text embedded in `agy.exe` [local]. No external doc fetched.

1. **Names.**
   - Directory rules: `GEMINI.md` and `AGENTS.md` (both, any directory) [local doc].
   - `.agents/rules/*.md` (also `.agent/`, `_agents/`, `_agent/` customization dirs) and a `rules.json` manifest listing rule paths, `include_only` for nested items [local doc/changelog].
   - Global: `~/.gemini/GEMINI.md`, `~/.gemini/AGENTS.md`, `~/.gemini/config/GEMINI.md`, `~/.gemini/config/AGENTS.md`, `~/.gemini/config/rules/` [local: changelog "global rules ... `~/.gemini/GEMINI.md`, `~/.gemini/AGENTS.md`, or their `~/.gemini/config/` counterparts"; folder `rules/` named]. Plugins: `plugins/<name>/rules/AGENTS.md` (a root-level AGENTS.md/GEMINI.md in a plugin is a validate warning) [local].
   - No `AGENTS.override.md` (0 hits). `contextFileName` appears once as a struct JSON tag (`json:"contextFileName"`) in the binary, probably from Gemini CLI settings migration; whether agy honours it is [unverified].
2. **Discovery.** `docs/rules.md`: "walks up from the current working directory to the repository root and loads these files"; they apply to their directory and subdirectories. `SKILL.md` says: "As you open or edit files, the agent walks up from the file's directory to the repository root, loading all rules it finds" [local doc]. The two statements differ: start-of-session cwd walk, plus lazy loading for files outside cwd chain; exact timing [unverified]. Global `~/.gemini/config` always applies. Dedup by resolved file path, once per conversation; symlinked globals deduped (fixed in changelog) [local].
3. **Limits.** Per file 24,000 bytes, measured after include expansion, truncated on line boundaries. Aggregate "always-on and global rules" budget 20,000 tokens (`defaultRulesBudget`); over-budget rules are demoted to file-path pointers (read on demand); rules from builtin plugins exempt [local doc]. Changelog: oversized endings were once "silently cut off on every turn".
4. **Imports.** `@[label](path)` include syntax (not `@file`) expanded before the 24 KB cap [local doc]. Relative resolution base and missing-file behaviour [unverified]. A plain `@AGENTS.md` line (as in this repo's CLAUDE.md) is not an agy include, but agy reads AGENTS.md directly anyway.
5. **Frontmatter.** Standalone GEMINI.md / AGENTS.md: no frontmatter, always active for their directory scope. Files in `rules/` need frontmatter `trigger:` one of `always_on`, `glob` (with `globs:` one comma-separated string, matched against absolute paths and file names), `model_decision` (needs `description`; loaded only when the model decides, cost = path pointer/description until then), `manual` [local: SKILL.md + embedded migration doc]. Validation (`agy plugin validate`) errors on a rule without valid `trigger` and warns on `model_decision` without description / `glob` without `globs` / content over 24 KB [local]. Only `always_on` rules load unconditionally.
6. **Switches.** `agy --help` shows no flag to disable rules; `--add-dir` exists; `--settings` takes inline JSON [local]. A setting that disables rules [unverified].
7. **Other CLIs' files.** Reads GEMINI.md and AGENTS.md. Does not read CLAUDE.md or `.github/copilot-instructions.md` (CLAUDE.md strings are only in a protected-path list and migration docs) [local].
8. **User-level path.** Windows `%USERPROFILE%\.gemini\config\` (and `~\.gemini\GEMINI.md`, `AGENTS.md`); WSL `~/.gemini/...` [local]. None of the global rule files exist on this PC. Project-level here: `.agents/rules/marshell-development.md` is referenced by agy's own saved command history.

---

## 4. Qwen Code 0.26.5

Sources: bundled source in `%LOCALAPPDATA%\qwen-code\qwen-code\lib\chunks\chunk-XBXDDFWT.js` (`loadServerHierarchicalMemory`, `getMemoryFilePathsInternal`, `processImports`, `loadRules`), `chunk-Y6HHV4LC.js` (filenames), `chunk-2D6S2HYH.js` (settings schema), `qwen --help` [local]. No external doc fetched.

1. **Names.** Default list `["QWEN.md", "AGENTS.md"]`, both searched in every location (constants `DEFAULT_CONTEXT_FILENAME`, `AGENT_CONTEXT_FILENAME`). Setting `context.fileName` (string or array) REPLACES the list (so `AGENTS.md` stops loading unless listed); `CLAUDE.md`/`GEMINI.md` load only if named there. Private/local: `<projectRoot>/.qwen/QWEN.local.md` (needs a trusted folder and a git root). Rules: `~/.qwen/rules/**/*.md` and `<projectRoot>/.qwen/rules/**/*.md` [local]. User dir = `QWEN_HOME` if set, else `~/.qwen` (`Storage.getGlobalQwenDir`). Extensions can add context files (`contextFileName` in extension manifests).
2. **Discovery.** At session start: global `~/.qwen/<name>`; then from cwd upward to the project root (nearest ancestor containing `.git`; if none, up to the parent of the home dir), root -> cwd order; stops at `~/.qwen`. Requires a trusted folder for the project walk. If cwd is the home directory only the home/global files load. Include directories (`--include-directories`) are searched only when `context.loadFromIncludeDirectories` is true. Subdirectories BELOW cwd are not scanned; only path-scoped rules load on demand [local code]. Edits are re-read on refresh.
3. **Limits.** No size limit or truncation found in the loader [local]; imports max depth 5. Token cost: whole concatenated text goes into the system prompt each session.
4. **Imports.** `@path` token must follow whitespace (or line start) and start with `.`, `/` or a letter; skipped inside code spans/blocks; resolved relative to the importing file's directory; max depth 5 ("Maximum import depth (5) reached" warning); circular/already-processed files become `<!-- File already processed -->`; path must stay inside the project root (else `<!-- Import failed: ... Path traversal attempt -->`; `file:`/`http(s):` refused). Missing file (ENOENT): the `@path` text is left in place silently, no warning. Other read errors: `<!-- Import failed -->` comment plus an error log. Imported content is inlined at start. `context.importFormat` `tree` (default) or `flat` [local].
5. **Frontmatter / path rules.** Rule files (`rules/**/*.md`): frontmatter `paths:` (list or string) and `description`. No `paths` = baseline rule, in the system prompt at start; with `paths` = injected as a system reminder the first time a matching file is touched (picomatch, `dot:true`); extension rules without `paths` are ignored. HTML comments stripped. Rule excludes: `contextRuleExcludes` exists in the loader; its settings key name was not found [unverified].
6. **Switches.** `--bare` (only explicitly provided CLI inputs; skips implicit discovery and rules), `--safe-mode` ("Disable all customizations (context files, hooks, extensions, skills, MCP servers)") [local --help]. Untrusted folder skips project files.
7. **Other CLIs' files.** Reads `AGENTS.md` by default. CLAUDE.md/GEMINI.md only via `context.fileName`. Does not read `.github/copilot-instructions.md` [local].
8. **User-level path.** Windows `%USERPROFILE%\.qwen\QWEN.md` (+ `AGENTS.md`, `rules\`), or `%QWEN_HOME%`; WSL `~/.qwen/` [local]. None present on this PC.

---

## 5. GitHub Copilot CLI 1.0.106

Sources: `copilot instruction list [--json]` run read-only in `C:\dev\Airport` [local]; `copilot --help` [local]; help-dialog string table in `cli-main.js` [local]; [doc https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions]; repo note `2026-11-06-copilot-cli.md` Q8. The loader itself is native; size limits and exact ordering were not found [unverified].

1. **Names.** `.github/copilot-instructions.md`; `.github/instructions/**/*.instructions.md`; `AGENTS.md`; `CLAUDE.md` and `.claude/CLAUDE.md`; `GEMINI.md`; `.claude/rules/**/*.md`; user `$HOME/.copilot/copilot-instructions.md` and `$HOME/.copilot/instructions/**/*.instructions.md`; extra dirs from env `COPILOT_CUSTOM_INSTRUCTIONS_DIRS` (comma list, `AGENTS.md` and `*.instructions.md` inside) [doc + local help-dialog table]. No `CLAUDE.local.md` or `AGENTS.override.md` (0 hits) [local]. No setting renames files.
2. **Discovery.** Repository root (git) and cwd "and intermediate directories" for AGENTS.md etc.; plus directories nested in the path of a file Copilot is working on (nested AGENTS.md lazily). `.github/instructions` is read at the standard locations, not intermediate directories [doc]. UI says "in git root & cwd" [local]. Repo-wide and user files load at start; `applyTo` files load only when a matching file is in use [doc]. Edits not picked up in a running session [doc].
3. **Limits.** None documented [doc silent]. Identical duplicate files are removed; no general precedence defined [doc].
4. **Imports.** `@relative/path` inside `copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`; read immediately, can nest; targets must be inside the repo or the custom instructions dir; absolute and `~/` not loaded; NOT expanded in `GEMINI.md` or `*.instructions.md` [doc]. Missing-file behaviour [unverified].
5. **Path-scoped.** `*.instructions.md` frontmatter `applyTo:` (comma-separated globs) and `excludeAgent: "code-review"|"cloud-agent"` [doc]. LOCAL: `.claude/rules/*.md` with `paths:` shows in `copilot instruction list --json` as `type:"vscode"` with `applyTo` set from the `paths` globs (this repo's six rules, e.g. `crates/core/src/pty/**`), so Copilot honours Claude rule scoping.
6. **Switches.** `--no-custom-instructions` ("Disable loading of custom instructions from AGENTS.md and related files") [local help]; `/instructions` toggles individual files (`defaultDisabled` field per source in JSON) [doc, local]; `--allow-all-mcp-server-instructions` is MCP, unrelated.
7. **Other CLIs' files.** Reads CLAUDE.md, GEMINI.md, AGENTS.md, `.claude/rules` [local, doc]. LOCAL in this repo: it lists BOTH `AGENTS.md` and `CLAUDE.md` (which imports `@AGENTS.md`) as separate sources, so the same text may be sent twice; dedupe is documented only for identical copies [unverified for imports]. Does not read QWEN.md.
8. **User-level path.** Windows `%USERPROFILE%\.copilot\` or `%COPILOT_HOME%` (replaces `$HOME/.copilot` for both user locations [doc]); WSL `~/.copilot`. None present on this PC (only `config.json`, `ide`, `logs`).

---

## Cross-CLI table: file name -> CLIs that read it

R = read by default, o = only when configured, - = not read. "(nested)" = subdirectory files below cwd are also loaded on demand.

| File | Claude Code | Codex | Antigravity | Qwen Code | Copilot CLI |
|---|---|---|---|---|---|
| `CLAUDE.md` / `.claude/CLAUDE.md` | R (also above cwd; nested) | o (fallback names) | - | o (`context.fileName`) | R (git root, cwd; nested) |
| `CLAUDE.local.md` | R | - | - | - | - |
| `AGENTS.md` | R only if no CLAUDE.md-type file in cwd/above (setting changes) | R (root->cwd, one per dir) | R (walk up) | R (default) | R (nested) |
| `.claude/AGENTS.md` | R (same rule) | - | - | - | - |
| `AGENTS.override.md` | - | R (beats AGENTS.md) | - | - | - |
| `AGENTS.local.md` | - | - | - | - | - |
| `GEMINI.md` | - | o | R | o | R |
| `QWEN.md`, `.qwen/QWEN.local.md` | - | o | - | R | - |
| `.github/copilot-instructions.md` | - | o | - | - | R |
| `.github/instructions/**/*.instructions.md` (applyTo) | - | - | - | - | R |
| `.claude/rules/**/*.md` (paths) | R | - | - | - | R (as applyTo) |
| `.agents/rules/*.md` (trigger) | - | - | R | - | - |
| `.qwen/rules/**/*.md` (paths) | - | - | - | R | - |
| user: `~/.claude/CLAUDE.md`, `~/.claude/rules` | R | - | - | - | - |
| user: `~/.codex/AGENTS.md` / override | - | R | - | - | - |
| user: `~/.gemini/{GEMINI,AGENTS}.md`, `~/.gemini/config/...` | - | - | R | - | - |
| user: `~/.qwen/{QWEN,AGENTS}.md`, `~/.qwen/rules` | - | - | - | R | - |
| user: `~/.copilot/copilot-instructions.md`, `instructions/**` | - | - | - | - | R |

Sharing rules that follow: one `AGENTS.md` serves Codex, Antigravity, Qwen and Copilot directly, and Claude Code directly only when no CLAUDE.md exists. With a CLAUDE.md in place, Claude needs `@AGENTS.md` in it (this repo's setup); then Copilot reads both files (duplicate text risk), Qwen/Codex/agy read only AGENTS.md. `.claude/rules/*.md` with `paths:` is read by Claude Code and Copilot.

## Limits summary

| CLI | Limit | Past it |
|---|---|---|
| Claude Code | 200 lines recommended per file (warning); 4 MiB hard skip; on-demand files 40,000 chars (200K window) / 150,000-200,000 (1M); MEMORY.md 200 lines / 25 KB | warning; skip whole file (4 MiB); start of file + "read the rest" note (on-demand); combined cut when too long together |
| Codex | `project_doc_max_bytes` 32,768 bytes combined, project files only | truncate crossing file, skip rest, warnings; 0 = none |
| Antigravity | 24,000 bytes per rule file; 20,000-token always-on/global rules budget | truncate on line boundary; over-budget rules become path pointers |
| Qwen | none found; import depth 5 | n/a |
| Copilot | none documented | unverified |

## Import rules summary

| CLI | Syntax | Base | Depth | Missing | Tokens at start |
|---|---|---|---|---|---|
| Claude Code | `@path` (not in code) | importing file | 4 (doc) / 5 (binary) | silent skip | yes, inlined |
| Codex | none | - | - | - | - |
| Antigravity | `@[label](path)` | unverified | unverified | unverified | yes (counted in 24 KB cap) |
| Qwen | `@path` | importing file | 5 | left as literal `@path`, no warning | yes, inlined; must stay in project root |
| Copilot | `@relative/path`, only in copilot-instructions / AGENTS / CLAUDE | file (repo-relative) | nests (limit unverified) | unverified | yes |

## Token estimate notes for the Doctor

- Claude: whole file + imports + unscoped rules + auto-memory head at start; path-scoped and nested files cost nothing until touched; HTML comments are stripped first. `/context` lists the real cost [doc].
- Codex: bytes capped at 32,768; project docs + global AGENTS; estimate chars/4.
- Antigravity: always_on rules + GEMINI/AGENTS up to the 20,000-token budget; `model_decision` costs a pointer.
- Qwen: everything concatenated plus baseline rules; conditional rules cost on first match. Qwen has an internal `estimateTextTokens` helper [local]; no public ratio found.
- Copilot: repo-wide, user, AGENTS/CLAUDE/GEMINI at start; `applyTo` files on demand; `copilot instruction list --json` gives the file list (no sizes).
- No CLI exposes a per-file token count outside the TUI; a Doctor should use a chars/4 heuristic and label it an estimate.

## Open / unverified

1. Claude Code: numeric startup-warning thresholds (recommended length, combined limit); exact import depth 4 vs 5; a missing import is silent only from code reading.
2. Antigravity: whether the walk is only at start or also per file opened (docs say both); `@[label](path)` base path and missing-file behaviour; any setting that disables rules; whether `contextFileName` is honoured; whether `~/.gemini/GEMINI.md` and `~/.gemini/config/GEMINI.md` are both loaded (changelog implies yes).
3. Qwen: settings key for `contextRuleExcludes`; no official doc checked (source only).
4. Copilot: size limit, precedence, whether an `@AGENTS.md` import in CLAUDE.md is deduped against AGENTS.md, missing-import behaviour, WSL behaviour of `.copilot`.
5. Codex: nothing in the installed 0.171.0 contradicts the source file read from main; the source on `main` could be newer than 0.171.0 (strings for the budget warnings were matched in the binary).
6. WSL: all CLIs use the distro's own `~` (separate from Windows); no WSL check was run here (Windows paths only). Marshell must read through `\\wsl$` or `wsl.exe` for those.
7. The `cc-plugin-agents-md` plugin is behind a server-side gate `tengu_agents_md_mod` (default true); a session may not have it (documented "unavailable" cases).

## What 7d1's first task saw

Seen on 2026-11-24 in a live check's scratch homes (`marshell-live-check`, started with `-CodexAuth -AgyAuth -QwenFake -CopilotFake -CodexHome`). Each run was a session the dev app started in a scratch project folder, with the CLI's non-interactive form (`codex exec`, `agy -p`, `qwen -p`, `copilot -p`). Qwen Code's and Copilot CLI's facts are read from the fake models' request logs, so they are the exact text each CLI sent. Codex's and Antigravity's are a real model's answer to "which marker words do you see in your instructions".

Versions: codex-cli 0.171.0, Qwen Code 0.26.5, Copilot CLI 1.0.106, Antigravity 1.5.14 for the first run and 1.5.16 after it (finding 20).

### Codex

1. **A folder with no trust entry loads its project `AGENTS.md`** (`codex exec`). With `trust_level = "untrusted"` the same folder's file isn't in the instructions. With `"trusted"` it is.
2. **The 32,768-byte budget cuts, with this warning** (a 40,000-byte `AGENTS.md`, markers every 4,000 bytes, the last one seen at byte 32,000): "warning: Loaded only 32768 of 40000 bytes from `<path>\AGENTS.md` in environment `local` under the 32768-byte project instruction budget. Trim your project instructions to fit the budget. If you need to keep them all, increase `project_doc_max_bytes`."
3. **No memory file of its own.** After five sessions the scratch `CODEX_HOME` holds `sessions/`, `cache/`, `skills/.system`, `plugins/` and no file Codex loads as instructions.

### Antigravity

4. **Both user files load:** `~/.gemini/GEMINI.md` and `~/.gemini/config/GEMINI.md` were both in the context, in that order, before the project's `GEMINI.md`.
5. **An include resolves from the including file's folder.** `.agents/rules/a.md` held `@[x](sub/inc.md)`; the text of `.agents/rules/sub/inc.md` was loaded, and `<project>/sub/inc.md` wasn't.
6. **A missing include is silent and stays as text.** The model quoted `@[y](nope/missing.md)` between the lines around it; nothing was printed about it.
7. **A rule file with no `trigger:` isn't loaded,** and its path isn't named in the context either.
8. **A `GEMINI.md` below the start folder never loaded.** `sub/GEMINI.md` wasn't in the context at start, nor after the agent read `sub/x.txt`, nor on the next turn of the same conversation. A session started in `sub/` had both the root's `GEMINI.md` and `sub/GEMINI.md`. So the walk is from the repository root down to the start folder, once, at start.
9. **No memory file of its own.** The scratch `~/.gemini` gained `antigravity-cli/` (conversations, `brain/<conversation>`, logs, the updater) and nothing that loads as instructions.

### Qwen Code

10. **Default names:** `QWEN.md`, then `AGENTS.md`, each as `--- Context from: <path> ---`. A `CLAUDE.md` beside them isn't read.
11. **`context.fileName` replaces the list, from the user's settings and from the project's.** With `["CLAUDE.md"]` in `~/.qwen/settings.json`, only `CLAUDE.md` was sent. With the same value only in `<project>/.qwen/settings.json`, the same.
12. **Imports go 5 deep, and a missing one stays as text.** With a chain of six, the root file's marker and the next five were sent, the sixth wasn't. `@x.md` with no such file was sent as `@x.md`.
13. **Folder trust (`security.folderTrust.enabled`):** a folder with no rule in `trustedFolders.json` gets no project file, the same as one marked `DO_NOT_TRUST`. `TRUST_FOLDER` loads it. The user's `~/.qwen/QWEN.md` loads in all three.
14. **Two files nobody planned for load at every start:**
    - `~/.qwen/output-language.md`, which Qwen Code writes itself (841 bytes), is sent as a context file after the project's.
    - **Auto memory.** `~/.qwen/memories/MEMORY.md` and `~/.qwen/projects/<name>/memory/MEMORY.md` are both sent in the first request, as a "memory catalog". `<name>` is the start folder's full path, lowercased, with every character that isn't a letter or digit as `-` (`c--users-…-lc-7d1-1-p-qw1`). A git worktree gets a folder of its own; it doesn't share the main checkout's.
    - Only the first 200 lines of each `MEMORY.md` are sent, followed by "> WARNING: MEMORY.md is 250 lines (limit: 200). Only part of it was loaded. Dropped 50 of 250 entries: …". The warning also names a "150-unit line limit" per entry, which wasn't tested.

### Copilot CLI

15. **No size limit was seen up to 256 KB.** A 65,508-byte and a 262,109-byte `AGENTS.md` were each sent whole (the last line's marker was in the request, whose size grew by the file's).
16. **An imported file that also loads by name is sent twice.** With `CLAUDE.md` holding `@AGENTS.md` beside an `AGENTS.md`, the request had `AGENTS.md`'s text once as its own `<custom_instruction>` and once inside `CLAUDE.md`'s, as `<imported_custom_instruction source="<path>">` after the literal `@AGENTS.md`.
17. **A missing import stays as text** (`@missing.md`), with no message.
18. **Nested `AGENTS.md` and `CLAUDE.md` load when used.** `sub/AGENTS.md` and `sub/CLAUDE.md` weren't in the first request. After the agent read `sub/x.txt`, the next request carried both as "Custom instructions from sub/AGENTS.md. Apply these to any code you write here:".
19. **No memory file of its own.** The scratch `COPILOT_HOME` holds `config.json`, `settings.json`, `session-store.db`, `session-state/` and logs.

### Outside the facts

20. **Antigravity updated the real install during the check.** The first `agy -p` ran 1.5.14; its updater then replaced `%LOCALAPPDATA%\agy\bin\agy.exe` with 1.5.16 (the old file is kept beside it as `agy.exe.<n>.old`), and wrote "Update successful, restart CLI to use" to the scratch `~/.gemini/antigravity-cli/updater/update_status.json`. The scratch `USERPROFILE` doesn't move the install. The same update would have come at the user's own next start.
