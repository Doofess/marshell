# Where an instruction-library doc goes in each CLI (slice 8a)

Date 2026-11-27, Windows 11. This note extends `2026-11-24-instruction-files.md` (read it first). It answers one question: where Marshell must write a copy of a library doc so that each CLI loads it once, at user level and at project level, always or only for files that match globs, and which CLIs also read a copy meant for another.

Labels as in the baseline: **[local]** verified on this PC (scratch homes and scratch projects under `%TEMP%`, nothing real touched, no prompt sent to any model); **[doc URL]**; **[src URL]**; **[unverified]**.

## Versions

Claude Code 2.1.332 (baseline 2.1.331), codex-cli 0.171.0, agy 1.5.16 (baseline 1.5.14), Qwen Code 0.26.5, Copilot CLI 1.0.106. Binary paths as in the baseline.

## Write targets

| CLI | User level | Project level | Globs |
|---|---|---|---|
| Claude Code | `~/.claude/rules/marshell-<n>.md` (`CLAUDE_CONFIG_DIR` moves it) | `.claude/rules/marshell-<n>.md` | `paths:` (YAML list or comma string, braces expand); the only key read |
| Codex | tagged block in `~/.codex/AGENTS.override.md` when it exists and isn't empty, else `~/.codex/AGENTS.md` (`CODEX_HOME` moves it) | tagged section in `developer_instructions` of the repo's `.codex/config.toml` | none |
| Antigravity | `~/.gemini/config/rules/marshell-<n>.md` | `.agents/rules/marshell-<n>.md` (flat) | `trigger: glob` with `globs: "a, b"`; else `trigger: always_on` |
| Qwen Code | `~/.qwen/rules/marshell-<n>.md` (`QWEN_HOME` moves it) | `.qwen/rules/marshell-<n>.md` | `paths:` (list or string), optional `description` |
| Copilot CLI | `~/.copilot/instructions/marshell-<n>.instructions.md` (`COPILOT_HOME` moves it) | `.github/instructions/marshell-<n>.instructions.md`, only when no `.claude/rules` copy covers it | `applyTo: 'a, b'` |

### Claude Code [doc https://code.claude.com/docs/en/memory.md]

- `.claude/rules/**` and `~/.claude/rules/**` are read recursively. A rule with no `paths` loads at launch; with `paths` it loads when Claude reads or edits a matching file. The user-level folder honours `paths` too.
- Project patterns are relative to the folder that holds `.claude`; user patterns are relative to the primary working directory.
- Not run live: the scratch home isn't signed in. Slice 8a's live check signs in once (8a decision 9).

### Codex [local: `codex debug prompt-input`, scratch `CODEX_HOME`]

- User level: `AGENTS.override.md` replaces `AGENTS.md` when it exists and isn't empty, so a writer must target the override then.
- Project level, per folder from the project root to the cwd: `AGENTS.override.md`, then `AGENTS.md`, then the first name in `project_doc_fallback_filenames`. One file per folder; a fallback entry holding a path is ignored.
- **The one Codex-only project channel is `developer_instructions` in `<repo>/.codex/config.toml`.** It adds a developer message of its own. Checked twice on this PC, in a scratch home and project of my own after the research agent's run:
  - project trusted (`[projects.'<long path>'] trust_level = "trusted"` in the scratch `config.toml`): both the `AGENTS.md` marker and the `developer_instructions` marker were in the prompt input;
  - no trust entry: only the `AGENTS.md` marker. `untrusted` drops the project's `AGENTS.md` too (the agent's run). The trust key must be the exact long path; an 8.3 short path didn't match;
  - a `developer_instructions` in the user `config.toml` and one in the trusted project's: only the project's reached the prompt. A project value replaces the user's.
- `model_instructions_file` replaces the base prompt (about 25,900 characters) and is not a route. `experimental_instructions_file` had no effect.
- `project_doc_max_bytes` (default 32,768) counts project `AGENTS.md`-family files only. A 40 KB global `AGENTS.md` loaded whole. `developer_instructions` is not counted in it [local, the agent's run].
- No globs, no frontmatter, no imports. HTML comments pass through as text.
- `codex debug prompt-input` prints the assembled input as JSON with no request to a model (source reading, and no network use was seen). Whether it prints the budget warnings: [unverified].

### Antigravity [doc antigravity.google/docs/rules.md; local: shipped `agy-customizations/SKILL.md`, strings in `agy.exe`]

- Project `.agents/rules/<n>.md` and user `~/.gemini/config/rules/<n>.md`, with `trigger:` one of `always_on`, `model_decision` (needs `description`), `glob` (with `globs:`, one comma-separated string), `manual`. A missing or invalid trigger drops the rule silently.
- `rules.json` is optional. Files directly in `.agents/rules/` load without it; subfolders are ignored unless registered, so copies are flat.
- Globs match absolute paths, so a project pattern needs a `**/` prefix.
- The changelog lists a BOM and an unquoted `description:` as past parse failures: write UTF-8 without BOM and quote strings.
- 1.5.16's shipped `rules.md` no longer describes `.agents/rules` or `trigger`; `SKILL.md`, the online doc and the text in the binary still do.
- Per file 24,000 bytes, cut on a line boundary (baseline).

### Qwen Code [local: `chunk-XBXDDFWT.js`]

- `.qwen/rules/**/*.md` and `~/.qwen/rules/**/*.md` (or under `QWEN_HOME`), recursive. No `paths`: in the prompt at start. With `paths`: injected the first time a matching file is touched, matched with picomatch against the path relative to the project root, user rules included.
- Project rules need a trusted folder (`folderTrust`); user rules load whatever the trust state.

### Copilot CLI [local: `copilot instruction list --json` with a scratch `COPILOT_HOME`; doc docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions]

- Project `.github/instructions/**/*.instructions.md` (a file without the `.instructions.md` suffix is not read) and user `$COPILOT_HOME/instructions/**/*.instructions.md`. `applyTo` scopes a file; `description` and `excludeAgent` are optional. `applyTo: '**'` printed no `applyTo`.
- It lists every `.claude/rules` file, subfolders included, with `paths` shown as `applyTo` (list, comma string and braces all worked).

## Who reads what (the double-load map)

| Location | Read by |
|---|---|
| `.claude/rules/**` | Claude Code and Copilot CLI |
| `~/.claude/rules`, `~/.claude/CLAUDE.md` | Claude Code only (Copilot ignored them with `USERPROFILE` and `HOME` moved) [local] |
| `.agents/rules`, `.qwen/rules`, `.github/instructions`, `.codex/config.toml`, `AGENTS.override.md` | the owning CLI only [local for Copilot and Codex; bundle and binary reading for Qwen and agy] |
| `.claude/CLAUDE.md` | Claude Code; **not** Copilot 1.0.106, although Copilot's doc says it reads it [local] |
| `AGENTS.md` | Codex, Antigravity, Qwen Code, Copilot CLI; Claude Code only when no `CLAUDE.md`-type file is in the folder or above |
| `CLAUDE.md` | Claude Code and Copilot CLI |
| `GEMINI.md` | Antigravity and Copilot CLI |

So the only shared write target is `.claude/rules` at project level: one copy there serves Claude Code and Copilot CLI, and a doc meant for Claude Code alone still reaches Copilot CLI.

Copilot drops identical `CLAUDE.md`/`AGENTS.md`/`GEMINI.md`/`copilot-instructions.md` files (doc and changelog). A byte-identical pair of `.instructions.md` files is listed twice; whether both are sent is [unverified].

## Seeing what loaded without a model call

- Copilot CLI: `copilot instruction list --json` (non-interactive): path, location, type, `applyTo`; no sizes.
- Codex: `codex debug prompt-input` (non-interactive).
- Claude Code: `/memory` and `/context all` in a session; `claude --debug` for frontmatter errors; the `InstructionsLoaded` hook. `claude doctor` lists no instruction files.
- Antigravity: `/context` in a session. No non-interactive form found.
- Qwen Code: `/memory` and `/context detail` in a session. No non-interactive form found.
- Qwen Code and Copilot CLI on the live check's fake model (`-QwenFake`, `-CopilotFake`) log each request, so the doc's text can be found in the request they send.

## Still unverified

1. Claude Code: everything beyond its doc (8a's live check, with the user's sign-in).
2. Antigravity: `trigger: glob` matching and user-level rules, live.
3. Qwen Code: rules and trust, live (code reading only).
4. Copilot CLI: user-level `applyTo`; whether a byte-identical pair is sent twice.
5. Codex: whether `debug prompt-input` prints the budget warnings; any size limit on `developer_instructions`.
6. WSL: nothing was run inside a distro.

The research agent's raw Copilot outputs are in `%TEMP%\phase8a-research\copilot-run1.json` and `copilot-run2-userprofile.json` (scratch, not in the repo).
