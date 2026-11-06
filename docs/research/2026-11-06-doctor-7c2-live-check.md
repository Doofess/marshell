# Doctor usage scan and Measure MCP servers live check (slice 7c2)

- Date: 2026-11-06, 15:18 to 15:31 local time.
- Build: `phase-7c2` at `afed6c1` under `pnpm tauri dev` on Windows 11 Pro (26300).
- Home: `live-check.ps1 start -Scratch <scratchpad>\lc-7c2-1 -Seed <scratchpad>\seed-7c2 -CodexHome %TEMP%\lc-7c2-cx`. That gave a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME` and `USERPROFILE` (`home`, so Antigravity's `~/.gemini` and Qwen Code's `~/.qwen` were scratch too), with CDP on port 9333. Onboarding was left at its first step; the person at the window later ran its Set up (see finding 1).
- Data: `seed_7c2.py` (scratchpad) wrote:
  - **The MCP server:** a copy of `fake-agent.exe` named `mcp-probe-marshell-test.exe`, run by full path (a path with a space in it).
  - **Claude Code:**
    - `cc\.claude.json`: user servers `probe-exit` (`mcp exit3`) and `probe-silent` (`mcp silent`);
    - a user skill `old-skill`, its folder's `CreationTime` set 40 days back after the copy;
    - two transcripts:
      - `old.jsonl`, first record 40 days old, with a `deferred_tools_delta` naming `mcp__probe-ok__echo` and a `skill_listing` naming `old-skill`;
      - `recent.jsonl`, a `Skill` call to `old-skill` 3 days ago, with `cwd` the scratch project.
  - **The project, after `start`:**
    - `proj\.mcp.json` with `probe-ok` (`mcp ok`);
    - `proj\.claude\settings.local.json` holding `enabledMcpjsonServers: ["probe-ok"]`;
    - `mh\projects.json` listing `proj`.
  - **Codex:** `config.toml` with `[mcp_servers.probe-ok]` (`mcp ok`), a `thread_history_1.sqlite` with one `commandExecution` row 40 days old, and a `state_5.sqlite` mapping its thread to `proj`.
- How it was driven:
  - CDP evaluations returning small JSON, CDP keys (Tab and Enter), emulated forced colors (not a real high-contrast theme), and one screenshot. There was no desktop input.
  - A `fetch` wrapper in the webview kept the bodies of the `/v1/doctor` responses (never a header) for the item ids and `duration_ms`.

## Results

| Check | Result |
| --- | --- |
| 1. The first report has `mcpunused.claude.*` (Waste, unchecked) for the `.mcp.json` server and `skillbroad.claude.*` naming the scratch project | Pass. `mcpunused.claude.c09e8b3e`: Waste, fixable, `unchecked: true`, "Claude Code's MCP server probe-ok wasn't used in 30 days", "It still starts with every Claude Code session. The fix turns it off and doesn't delete it." `skillbroad.claude.9eb70856`: Info, "Claude Code's skill old-skill is used in one project only", "In the last 30 days it was used only in …\lc-7c2-1\proj. To load it only there, move its folder to …\lc-7c2-1\proj\.claude\skills.", with "The Doctor doesn't move it, because a project's files are shared with its team." `usage.json` (version 1): Claude Code and Codex each covered 40.0 days. Claude Code had 2 cursors, `skill:old-skill` used, and `tool:mcp__probe-ok__echo` and `skill:old-skill` available. Codex had 1 cursor. 6 `first_sight` keys. |
| `duration_ms` of the first and second runs | The run at app start came before the `fetch` wrapper, so it wasn't recorded. The run on opening the Doctor took 46 ms, and Run again took 42 ms. |
| 2. Measure gives `mcpcost.claude` (1 measured) and `mcpfail.claude.*` for the exit3 server and, after 30 s, the silent one | Pass. The button read "Measuring MCP servers…" with `aria-disabled="true"`, and the report came back 30 s after the click: <br>• `mcpfail.claude.e9869376`: Broken, unchecked, "Claude Code's MCP server probe-exit didn't start", "…it exited with code 3 before answering. A CLI started from your shell can have an environment Marshell's doesn't.", with "Claude Code can turn a user-level server off only per project. To remove it, run claude mcp remove probe-exit -s user." <br>• `mcpfail.claude.9a216090`: the same for probe-silent, "…it didn't answer within 30 s." <br>• `mcpcost.claude`: "Claude Code's MCP servers add about 30 tokens to each session", "1 measured. The largest: probe-ok (30)." <br>• `mcpcost.codex`: "about 5 tokens", "1 measured. The largest: probe-ok (5)." <br>• `mcpcost.qwen`: "Qwen Code's MCP servers weren't measured", "Not measured: 1 that Qwen Code hasn't approved." (Qwen Code reads the project's `.mcp.json` too.) <br>The status line read "MCP servers were measured 6 s ago." |
| No probe process is left 5 s later | Pass. During the run, one `mcp-probe-marshell-test` was alive (the silent one). `Get-Process mcp-probe-marshell-test` counted 0 at 15:21:57, 14 s after the report, and 0 again after the second measure. No other program was queried or touched. |
| 3. Fixing the unused item, then Undo, gives back the exact bytes of `settings.local.json` | Pass, on the second try (finding 1). From the file as onboarding left it (`D553243EBDDB…`), the fix's preview showed `"disabledMcpjsonServers": ["probe-ok"]`. Applying gave "Fixed 1 issue. 1 session picks this up after a restart." and `FB4A7831AF78…`, the same bytes as the first apply. "Undo last fix" gave "The last fix is undone." and `D553243EBDDB…` again. |
| 4. Keyboard alone: Tab reaches "Measure MCP servers", and Enter runs it | Pass. From the dialog: Tab to Close, Tab to Run CLI doctors, Tab to Measure MCP servers. Enter sent one `POST /v1/doctor/measure`. Focus stayed on the button, now "Measuring MCP servers…" with `aria-disabled="true"`. A second Enter sent nothing. |
| Forced colors: the button stays visible | Pass (emulated). The screenshot showed "Measuring MCP servers…" with a system-colour border and the focus ring, between Run CLI doctors and Run again. |
| The real `~/.claude/settings.json` | Its modified time moved from `01:15:34` to `15:18:32`. That was before the app had finished building: the webview answered at about 15:20, and nothing the check ran writes there. The cause wasn't found (finding 4). |

## Findings

1. **The person at the window used the app during the check.** Between 15:21:43 and 15:22:03, they ran onboarding's Set up for all four CLIs; it wrote hooks and the status line into the scratch `settings.local.json` and the other scratch homes. They also started a Claude Code session in `proj`, pressed the sheet's "Fix 1 issue" for the fix I had previewed, and opened Settings. The person confirmed it was them. So the seeded hash (`3E4A8CF61765…`) was no longer the file's state before the fix, and check 3 was repeated from the file onboarding left. The session had exited before the stop, and no agent CLI was left in the tree or outside it.
2. **Opening the Doctor posts `/v1/doctor/run` twice in dev.** It also reads `/v1/doctor/undo` twice. `src/main.tsx` wraps the app in `StrictMode`, which runs effects twice in dev only. A request that arrives during a run joins it and queues one more run (`request_run_with`), so in dev it costs one extra run. Not fixed (dev only).
3. **The fix sheet names `~/.marshell/snapshots/` under a scratch `MARSHELL_HOME`.** "A backup is saved first, in ~/.marshell/snapshots/." The snapshot went to the scratch home; the copy names the default folder. This predates 7c2. Not fixed.
4. **The real `~/.claude/settings.json` changed during the check window, not from the check.** Compared by key name with `C:\dev\claude-setup\settings.json`, it differs in `modelSettings.claude-opus-5-5.effortLevel` (`xhigh` against `high`), `tui`, an enabled `pyright-lsp` plugin, and `env.CLAUDE_CODE_PLUGIN_DIRS`. The write was at 15:18:32, before the dev app was running. No key was restored; the person decides.
5. **Codex's `probe-ok` got no unused item, as the Known limits say.** Codex's history covered 40 days, but a Codex server has no availability record, so its "added" date is Marshell's first sight (today).
