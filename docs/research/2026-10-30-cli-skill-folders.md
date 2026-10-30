# Agent skill folders (slice 6h2)

- Date: 2026-10-30. Web and docs research, plus `--version` of the CLIs installed here: Claude Code 2.1.314, Codex 0.164.0, agy 1.4.10. Latest at the time: Claude Code 2.1.314, Codex 0.166.0, agy 1.4.11, Gemini CLI 0.66.0.
- Nothing under the real `~/.claude`, `~/.codex`, `~/.gemini` or `~/.agents` was read.
- "Documented" means an official doc or the CLI's own source; "inferred" is a reading of sources or of the agy binary. Items marked **open** are for the live check.

## Claude Code

- Documented ([skills](https://code.claude.com/docs/en/skills)):
  - User skills live in `~/.claude/skills/<name>/SKILL.md`, project skills in `.claude/skills/<name>/SKILL.md` (also found in parent folders up to the repo root, nested folders and `--add-dir` folders), and enterprise skills in the managed-settings folder.
  - Precedence: enterprise, then personal, then project.
- Documented ([claude-directory](https://code.claude.com/docs/en/claude-directory)): with `CLAUDE_CONFIG_DIR` set, every `~/.claude` path moves under it, so skills go in `$CLAUDE_CONFIG_DIR/skills/`. Older issues reported skills ignoring the variable ([#36172](https://github.com/anthropics/claude-code/issues/36172)). **Open:** whether 2.1.314 honours it.
- Format:
  - Every frontmatter field is optional, and unknown ones are ignored.
  - `name` defaults to the folder name.
  - `description` defaults to the first body line. With `when_to_use` it is cut at 1,536 characters in the listing.
  - Keep the body under 500 lines. Extra files are allowed.
- Loading:
  - Descriptions are always in context, and the body loads on use.
  - The model can invoke a skill, and so can `/name`.
  - The skill folders are watched for changes. A top-level skills folder created after start needs `/reload-skills`.
  - `skillOverrides` and denying `Skill` in permissions turn skills off.
- No approval prompt for a plain skill. `~/.claude/skills/.trash/` and the names `anthropic-skills` and `synced` are reserved for claude.ai sync.
- Claude Code doesn't read `.agents/skills`.

## Codex CLI

- Documented ([build-skills](https://learn.chatgpt.com/docs/build-skills)):
  - Skills are found in the repo's `.agents/skills` (at the cwd and each parent up to the repo root), the user's `$HOME/.agents/skills`, the admin `/etc/codex/skills`, and the bundled system skills.
- Source ([host_roots.rs](https://github.com/openai/codex/blob/main/codex-rs/ext/skills/src/host_roots.rs)):
  - `$CODEX_HOME/skills` is still scanned, marked "Deprecated... kept for backward compatibility".
  - System skills are cached in `$CODEX_HOME/skills/.system`.
  - A project `.codex/skills` is scanned too.
  - `CODEX_HOME` doesn't move `~/.agents/skills`, since that path is built from the home folder. **Open:** which home Codex uses on Windows (`USERPROFILE` or the known-folder API), which decides whether a live check's scratch home holds the file.
- Format ([parser.rs](https://github.com/openai/codex/blob/main/codex-rs/skills/src/parser.rs)):
  - `name` is optional, at most 64 characters, and defaults to the folder name.
  - `description` is required and is folded to one line.
  - An optional `agents/openai.yaml` with `policy.allow_implicit_invocation: false` makes a skill explicit-only.
- Loading:
  - Names and descriptions are listed up front, capped at 2% of the context or 8,000 characters. Bodies load on selection.
  - The model can select a skill, and so can `$name` or `/skills`.
  - Changes are picked up automatically.
  - A `[[skills.config]]` entry with `enabled = false` turns one off.
- Same-name skills are not merged, and both are listed (scope order Repo, User, System, Admin, per [host_merge.rs](https://github.com/openai/codex/blob/main/codex-rs/ext/skills/src/loader/host_merge.rs)).
- Codex manages `.system` and its `$skill-installer`. No trust prompt for skills.

## Antigravity CLI (`agy`)

- Documented ([skills](https://antigravity.google/docs/skills)):
  - Workspace skills live in `<root>/.agents/skills/<name>/`, and the older `.agent/skills` is still read.
  - The global folder differs by doc: the CLI page says `~/.gemini/antigravity-cli/skills/`, and the App and IDE pages say `~/.gemini/config/skills/`.
  - A [hands-on test](https://atamel.dev/posts/2026/07-01_where_agy_agent_skills/) found `~/.gemini/config/skills/` works in all three surfaces. The 1.4.10 binary holds that string (inferred).
  - **Open:** whether agy also reads `~/.agents/skills`. Gemini CLI does ([docs](https://geminicli.com/docs/cli/skills/)), so a skill there could be listed twice.
- Format:
  - `description` is required.
  - `name` is optional, lowercase with hyphens, and defaults to the folder name. `agy plugin validate` warns when the name differs from the folder.
  - The body limit is 24 KB ([changelog](https://github.com/google-antigravity/antigravity-cli/blob/main/CHANGELOG.md)).
  - Quote the description: an unquoted colon was a bug.
- Loading:
  - Name and description at start, body on match, and a `/<skill>` command per skill.
  - `/skills` lists them, and `/skills reload` reloads.
  - No off switch is documented for agy.
- agy has no home override variable that could be found, so a live check moves it with `USERPROFILE`, as the live-check script already does.

## What 6h2 takes from this

- One `SKILL.md` for all three:
  - The folder is `marshell`.
  - The frontmatter holds only `name: marshell` and a quoted one-line `description`.
  - The body is well under 24 KB and 500 lines.
- Installed at user level:
  - Claude Code: `<CLAUDE_CONFIG_DIR or ~/.claude>/skills/marshell/SKILL.md`.
  - Codex: `~/.agents/skills/marshell/SKILL.md`.
  - Antigravity: `~/.gemini/config/skills/marshell/SKILL.md`.
