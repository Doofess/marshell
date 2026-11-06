# Doctor MCP servers and skills live check (slice 7c1)

- Date: 2026-11-06, 02:16 to 02:24 local time, and a re-check from 12:44 to 12:48 after the review's fix pass.
- Build: `phase-7c` at `f442fa4` under `pnpm tauri dev` on Windows 11 Pro (26300); the re-check at `5ab24e6`.
- Home: `live-check.ps1 start -Scratch <scratchpad>\lc-7c1-1 -CodexHome %TEMP%\lc-7c1-cx`. That gave a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME` and `USERPROFILE` (`home`, so Antigravity's `~/.gemini` and Qwen Code's `~/.qwen` were scratch too), with CDP on port 9333. Onboarding was left at its first step, with nothing set up.
- Data: `seed_7c1.py` (scratchpad) wrote:
  - **Marshell:** `mh\projects.json` holding the scratch project.
  - **Claude Code:**
    - `proj\.mcp.json` with a server `proj-ghost` whose command is `ghost-mcp`;
    - `cc\.claude.json` with a local-scope server `local-ghost` (`ghost-mcp`) under the project's key;
    - a user skill `long-reader` with a 1,631-character description, and a short `fine` one.
  - **Codex:** `config.toml` with `[mcp_servers.ghost]` and an inline `inline-ghost = { command = "ghost-mcp" }` (both with comments), and a skill `nodesc` with no description.
  - **Qwen Code:** a user skill named `a b`.
  - **Antigravity:** `~/.gemini/config/mcp_config.json` with a server `agy-ghost`.
- How it was driven:
  - CDP evaluations returning small JSON, and one screenshot. There was no desktop input.
  - `claude mcp list` ran from a shell in the scratch project, with the scratch `CLAUDE_CONFIG_DIR` set for that one process.
  - The real configs (step 3b) were read by a temporary ignored unit test, never committed: `roots_under(USERPROFILE)`, the same readers and `ext::items`, and a scratch copy of `~/.marshell/projects.json`. The app wasn't started on the real homes, because History would copy the real transcripts into the scratch home, and the core writes Antigravity's `settings.json`.

## Results

| Check | Result |
| --- | --- |
| Re-check at `5ab24e6`: a bare command missing from the PATH starts unchecked (decision 11) | Pass. A new scratch (`lc-7c1-2`, `%TEMP%\lc-7c1-cx2`) with the same seed, except that `agy-ghost`'s command was the absolute `C:\nope-7c1\ghost-mcp.exe`. "Broken 8" and "Fix 3 issues": the five `ghost-mcp` servers were unchecked, with "Its command, ghost-mcp, isn't on the PATH Marshell's tabs get. The fix turns it off and doesn't delete it." `agy-ghost` ("Its command, C:\nope-7c1\ghost-mcp.exe, isn't on this PC."), `nodesc` and `a b` were checked. Nothing was applied. All 19 processes were stopped, and the real `settings.json` kept its modified time. |
| The Broken items, pre-checked (at `f442fa4`, before decision 11) | Pass. "Broken 8": Claude Code's `local-ghost` and `proj-ghost`; Codex's `ghost`, `inline-ghost` and skill `nodesc`; Antigravity's `agy-ghost`; Qwen Code's `proj-ghost` (it reads the project's `.mcp.json` too) and skill `a b`. All 8 checkboxes were checked and the button read "Fix 8 issues". Copy: "Its command, ghost-mcp, isn't on this PC. The fix turns it off and doesn't delete it.", "Its SKILL.md has no description.", "Its name has characters Qwen Code refuses." |
| The Waste long-description item, unchecked and report-only | Pass. "Claude Code's skill long-reader has a description Claude Code cuts", "It is 1631 characters, and Claude Code keeps the first 1536.", "Shorten the description in its SKILL.md.", with no checkbox. |
| The cost lines | Pass. "Claude Code's skills add about 393 tokens to each session", "2 skills are listed, by name and description. The largest: long-reader (387), fine (6)." Codex and Qwen Code had no line: their one skill doesn't load. |
| One diff per file | Pass. Five regions: `cc\.claude.json`, `proj\.claude\settings.local.json`, `home\.qwen\settings.json` (new), `home\.gemini\config\mcp_config.json`, and `lc-7c1-cx\config.toml` (one edit for its three items). |
| Apply changes the files as the round-trip test says | Pass. "Fixed 8 issues.":<br>• `disabledMcpServers: ["local-ghost"]` under the project's key;<br>• `disabledMcpjsonServers: ["proj-ghost"]`;<br>• Qwen Code's new file with `mcp.excluded: ["proj-ghost"]` and `skills.disabled: ["a b"]`;<br>• `"disabled": true` on `agy-ghost`;<br>• `enabled = false` on both Codex servers and a `[[skills.config]]` rule for `nodesc`, with both comments kept.<br>`managed.json` held `"entries": []`. "Run again" showed no Broken group. |
| Undo puts them back | Pass. "The last fix is undone." All four files had their seeded hashes again (`4AF3A6EF90E4`, `E3566B3A0643`, `54FBE9A94C1F`, `EF2651BC7B1F`), and Qwen Code's created `settings.json` was gone. |
| Decision 8: Claude Code keeps and honours `disabledMcpServers` in `~/.claude.json` | Pass, so decision 8 stands. After the `local-ghost` fix alone, `claude mcp list` printed "local-ghost: ghost-mcp - ⊘ Disabled for this project (re-enable via /mcp)" and "proj-ghost: ghost-mcp - ⏸ Pending approval". Claude Code rewrote `.claude.json` in that run (it added `firstStartTime`, `machineID` and its migration flags) and kept `disabledMcpServers: ["local-ghost"]`. The interactive `/mcp` wasn't reached: the scratch config has no login. |
| Undo after the CLI rewrote the file | Pass. "Nothing was undone: …\cc\.claude.json changed since the fix, so undo would overwrite it." (409). |
| The real configs, read only | Pass after one fix (finding 2). No Broken or Waste item. Claude Code: 1 server, 22 skills, about 954 tokens. Codex: 3 servers, 15 skills, about 1,071 tokens. Antigravity: 0 servers, 8 skills, about 652 tokens. Qwen Code: 1 server, 2 skills (`~/.agents/skills`), about 104 tokens. 1 of the 5 recent projects still exists (`C:/dev/Airport`, whose `.mcp.json` gives Claude Code and Qwen Code their one server). |
| The real `~/.claude/settings.json` | Untouched: its modified time was `2026-11-06T01:15:34` before and after. |

## Findings

1. **The inline Codex server gets a space before its new comma.** The fix wrote `inline-ghost = { command = "ghost-mcp" , enabled = false } # inline`. It is valid TOML, and Codex reads it, but `toml_edit` keeps the old last value's trailing space. Not fixed (cosmetic).
2. **A Claude Code skill set to `"user-invocable-only"` was counted in the cost.** The real `~/.claude/settings.json` uses that value (for bundled skills, which the reader doesn't list). On a user skill, it keeps the skill out of the model's listing, as `disable-model-invocation` does. Fixed in `5f0f655`; `claude_skills` asserts it and failed first.
3. **Two fix-path defects were found in review before the check and exercised here.** The inline Codex server above was turned off; before `f442fa4` its fix refused with 409 on every try. A Codex skill in a distro is now named by its Linux path; no distro ran in this check.
