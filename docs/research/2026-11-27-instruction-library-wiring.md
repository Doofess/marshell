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

1. Claude Code: user-level `paths` and glob matching (the live check listed both rules in `/context`, with no globs, and made no model call).
2. Antigravity: `trigger: glob` matching and user-level rules, live.
3. Qwen Code: rules and trust, live (code reading only).
4. Copilot CLI: user-level `applyTo`; whether a byte-identical pair is sent twice.
5. Codex: whether `debug prompt-input` prints the budget warnings; any size limit on `developer_instructions`.
6. WSL: the live check wrote and removed copies in a running distro, but no real CLI ran there (the CLIs on its PATH were stand-ins), so no distro CLI was seen loading a copy.

The research agent's raw Copilot outputs are in `%TEMP%\phase8a-research\copilot-run1.json` and `copilot-run2-userprofile.json` (scratch, not in the repo).

## Live check (slice 8a)

- Date: 2026-11-27, 15:39 to 16:22 local time, then 16:37 to 16:50 for step 6 (Claude Code) and the running half of step 9 (WSL), after `live-check.ps1 restart` on the same scratch.
- Build: `phase-8a` under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Homes: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `USERPROFILE` (for Antigravity CLI, Qwen Code and Copilot CLI), and a short scratch `CODEX_HOME`, with CDP on port 9333. Onboarding was left with every CLI's hooks skipped ("Skip: use limited status"). The Qwen Code and Copilot CLI turns ran on the fake model (`-QwenFake`, `-CopilotFake`); Antigravity CLI used a copy of the sign-in (`-AgyAuth`).
- CLIs: Claude Code 2.1.332, Codex 0.171.0, Antigravity CLI 1.5.16 (1.5.18 after the check, see finding 4), Qwen Code 0.26.5, Copilot CLI 1.0.106.
- Data: a scratch project `work\airport8a` (a `git init` with `src\app.ts`), recorded by a shell session started in it. The user doc `GREETING` ("When the user says hello, answer with the word MARSHELL-GREETING-8A first.") and the project doc `SECURITY` ("Never print a secret. The project's safety word is MARSHELL-SECURITY-8A."), both for all five CLIs, no globs.
- How it was driven: CDP keys for the page (the palette's "Open Instructions", Tab, Enter, Ctrl+S, Esc), CDP evaluations to read state and to start sessions (`sessions.create`, each CLI run headless with its output in the scratch folder), the session recordings (`.cast`) to read the agy terminal, emulated forced colours (not a real high-contrast theme), two screenshots. No desktop input. In step 6 the user signed in to Claude Code at the PC, in the scratch `CLAUDE_CONFIG_DIR`; `/memory` and `/context` were typed into the session over the API and read from screenshots (no `/model`, no `/effort`, no prompt). Step 9's running half started Ubuntu with a background `wsl.exe -d Ubuntu -e sleep 1800` and, with the user's OK, put two do-nothing stand-ins (`claude` and `qwen`, `#!/bin/sh` + `exit 0`) in the distro user's `~/.local/bin`, since no agent CLI is on that user's own PATH (Copilot CLI is there only through VS Code's shim under `/mnt/c`, which Marshell skips). The project was `/tmp/lc-8a/proj` (a `git init`), recorded by a WSL shell tab started in `\\wsl.localhost\Ubuntu\tmp\lc-8a\proj`.

### Results

| Check | Result |
| --- | --- |
| 1. Make GREETING and SECURITY, Review, Apply: the bar goes and every row reads "Up to date" | Pass. After the saves the bar read "9 copies are out of date" (five user copies, four project files: Copilot CLI shares Claude Code's). The review sheet "Update the copies" listed nine files, all in the scratch homes; after Apply, "Copies updated." with Undo, no bar, and all ten rows "Up to date". |
| 2. Codex: `codex debug prompt-input` before and after trusting the project | Pass. Untrusted: only MARSHELL-GREETING-8A, from the scratch `AGENTS.md` block. Trusted (`[projects.'<long path>'] trust_level = "trusted"`): both, SECURITY as its own developer message from `.codex/config.toml`. The row's note "Codex loads it once you trust this project in Codex." went once the project was trusted. |
| 3. Copilot CLI: `copilot instruction list --json` and one fake turn | Pass. The list held `~\.copilot\instructions\marshell-greeting.instructions.md` (location `user`) and `.claude\rules\marshell-security.md` (location `repository`); the first request to the fake model held both markers. |
| 4. Qwen Code: one fake turn with the folder trusted | Pass. `folderTrust` is off in the scratch settings, which trusts every folder; two requests to the fake model each held both markers. |
| 5. Antigravity CLI: `/context` lists both rules | Pass with a note. `/context` showed "System files · auto-loaded └ .agents/rules/" and nothing for `~/.gemini/config/rules`. One real turn ("hello. Also, what is the project's safety word?") answered "MARSHELL-GREETING-8A The project's safety word is MARSHELL-SECURITY-8A.", so the user rule loads; `/context` just doesn't list it. The user copy starts with `trigger: always_on`. |
| 6. Claude Code: `/memory` and `/context` list both | Pass. After the user's sign-in, `/memory` listed `C:\Users\Home Office\AppData\Local\Temp\lc…` (cut by the width), `C:\Users\Home Office\.claude\CLAUDE.md` and `.claude\rules\marshell-security.md`. `/context all` gave the full paths under "Memory files": `C:\Users\Home Office\AppData\Local\Temp\lc-8a-1\cc\rules\marshell-greeting.md: 36 tokens`, `C:\Users\Home Office\.claude\CLAUDE.md: 1.9k tokens` (see finding 7) and `.claude\rules\marshell-security.md: 38 tokens`. |
| 7. Drift, "Use this copy's text", Undo | Pass. A hand edit of the Qwen project copy: its row read "Changed outside Marshell" with "Put the doc's text back" and "Use this copy's text", the bar "1 copy was changed outside Marshell; its row has the fixes.", and the Doctor item `libdrift.7ce9ee7a.security` ".qwen/rules/marshell-security.md changed outside Marshell" (Broken, unticked). "Use this copy's text" wrote the doc (text only, frontmatter kept) and re-marked the Qwen copy; the other rows read "Out of date", the bar "3 copies are out of date", and the Doctor showed `libstale` "3 copies of SECURITY are out of date in airport8a". Undo ("Put back.") restored the doc and the hand-edited copy. "Put the doc's text back" then rewrote the Qwen copy alone. |
| 8. Globs: `src/**` on SECURITY, Review | Pass. Codex's row read "Review removes this copy" with "Codex has no globs, so this doc doesn't load in Codex."; Review deleted `.codex\config.toml` (it held only Marshell's section), after which the row read "No copy" and `codex debug prompt-input` held only GREETING. Antigravity CLI's copy got `trigger: glob` and `globs: '**/src/**'`, with the note "Antigravity CLI matches globs anywhere in the path, so src/** also matches a deeper folder of that name."; Claude Code's and Qwen Code's got `paths: ['src/**']`. The glob was removed again before step 6. |
| 9. WSL, distro stopped | Pass. With one hand-written Ubuntu entry in the scratch `managed.json` and Ubuntu stopped, GREETING's rows ended with "Ubuntu — Waiting for the distro — Ubuntu isn't running, so its copies wait.", and the Doctor read "Not checked: Ubuntu isn't running." `wsl -l --running` said "There are no running distributions." before, after the page, and after the Doctor. |
| 9. WSL, distro running: a user doc's copy lands in the distro home only for the CLIs on its PATH; one project in the distro gets its copies | Pass after finding 6. With Ubuntu running, GREETING's rows read "Claude Code in Ubuntu — No copy yet — ~/.claude/rules/marshell-greeting.md" and the same for Qwen Code; Codex, Antigravity CLI and Copilot CLI in Ubuntu read "No copy". A project doc `DISTRO` (marker MARSHELL-DISTRO-8A, all five CLIs) saved for `/tmp/lc-8a/proj` gave the bar "6 copies are out of date". The review sheet "Update the copies" listed `\\wsl.localhost\Ubuntu\home\home_office\.claude\rules\marshell-greeting.md`, `…\.qwen\rules\marshell-greeting.md`, and the project's `.claude\rules`, `.agents\rules` (with `trigger: always_on`), `.qwen\rules` and `.codex\config.toml` (a `developer_instructions` block). After Apply ("Copies updated."), the distro had exactly those files; the user home got no `~/.codex/AGENTS.md`, `~/.gemini` or `~/.copilot`. "Delete…" on DISTRO, then on GREETING ("Delete GREETING and its copies", six scratch files and the two in Ubuntu) removed every file; the folders they had made stayed, empty (finding 5). The stand-ins and those empty folders were then removed by hand (`rm`, `rmdir`), and the distro user's home matched its listing from before the check; Ubuntu stopped by itself afterwards. |
| 10. Steps 1 and 7 by keyboard; forced colours | Pass after findings 1–3. The page opened from the palette, and docs were made, saved (Ctrl+S), reviewed and fixed by Tab and Enter alone. In forced colours (emulated) the fields, ticks, the selected doc's mark, the status strip and the Loads in rows were all legible. |

### Findings

1. **Tab skipped the doc's Text editor.** The dialog focus trap walks a fixed query (`FOCUSABLE`), which had no `contenteditable`; CodeMirror's content has no tabindex, so Tab went from Globs to Save. Fixed in f5f70306 (`dialogFocus.test.ts` "takes in an editor's contenteditable", `focusOrder.test.ts`).
2. **The page could open with the project's docs missing.** It reads the view before the projects are known (no project) and again on the first project; when the first answer came in last, the project showed "No docs yet" and no bar until "Check again". Fixed in abd57528: a view carries the project it was read for, and the reducer drops one for another project (`instructionsState.test.ts`).
3. **Esc in the editor closed the whole page.** With Tab indenting in the editor, closing the page was the only keyboard way out of it. The text editor's rule is "Esc then Tab leaves". Fixed in a7c11f5f: Esc with the keyboard in an editor leaves a dialog open, after the editor's own Esc has run (`dialogFocus.test.ts`).
4. **Antigravity CLI updated itself during the check** (known, see the live-check skill): the first interactive `agy` replaced the real install at 15:53, 1.5.16 to 1.5.18, leaving `agy.exe.1795787610277412800.old`. Its first-run screens asked for a theme, the terms (the data-sharing box was unticked) and folder trust, in the scratch home.
5. **Minor, not fixed:** after Apply or Undo removes the button that opened a sheet ("Review", "Use this copy's text", "Undo"), focus falls back to the page itself rather than a control. Removing a Codex section that empties `.codex\config.toml` deletes the file and leaves an empty `.codex` folder. In the same way, "Delete…" removes a doc's copies but not the folders the copies made (`~/.claude/rules`, `~/.qwen/rules` in the distro user's home; every rule folder in the project).
6. **A distro's row said the CLI wasn't installed "on this PC".** In Ubuntu, GREETING's Codex, Antigravity CLI and Copilot CLI rows read "Codex in Ubuntu — No copy — Not installed on this PC, so it gets no copy.", though all three are installed on Windows; a distro project's rows said the same before "The copy is for teammates who use it." Fixed in 13c946f2: in a distro the sentence names the distro ("Not installed in Ubuntu, so it gets no copy.", "Not installed in Ubuntu. The copy is for teammates who use it."); on Windows it stays as decisions 27 and 40 word it (`library.rs` tests `user_doc_skips_a_cli_not_found` and `a_distro_projects_teammates_note_names_the_distro`, failing first).
7. **The check's Claude Code also loaded the real `~\.claude\CLAUDE.md`** (read only, nothing written). Most likely because the scratch project sits under the user profile (`%TEMP%`), so `C:\Users\Home Office` is one of its ancestor folders and Claude Code reads an ancestor's `.claude\CLAUDE.md`; the scratch `CLAUDE_CONFIG_DIR` holds no `CLAUDE.md`. It doesn't touch the library, whose user copy came from the scratch `cc\rules`. A later check that must see only scratch instructions in Claude Code puts its project outside the profile.
