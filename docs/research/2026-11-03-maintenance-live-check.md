# Maintenance live check (slice 6k)

- Date: 2026-11-03, about 02:48 to 03:05 local time.
- Build: `phase-6k` at f9fcbb1 under `pnpm tauri dev` on Windows 11 Pro (10.0.26300).
- Home: two runs, each with a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `USERPROFILE` and `MARSHELL_CLAUDE_SETTINGS`, and CDP on port 9333. The scratch folders were `%TEMP%\lc-6k-1` and `%TEMP%\lc-6k-2`. In run 1, onboarding was skipped at the consent step ("Skip: use limited status"), so no Claude Code hooks were set up.
- Data: in run 1, Codex hooks were set up in the scratch `cx\hooks.json` through the app's own API (`POST /v1/agents/codex/hooks/plan`, then `POST /v1/config/apply`). The preview named only that file, and the result was 8 `mshl1` entries. Run 2 was started with `-Seed` from run 1 and with Codex's `bin` folder removed from `PATH`.
- How it was driven: CDP evaluations returning small JSON, and CDP keys (Ctrl+Shift+T, Ctrl+Shift+1 and 2, Ctrl+Shift+Alt+T, Escape, Enter). One viewport resize was made with `page.setViewportSize`. The app's API was called with the scratch endpoint's token. No screenshots, and no desktop input.

## Results

| Check | Result |
| --- | --- |
| #65: a hidden tab keeps its size. Open two tabs, show the second, and the first never becomes 10x4. | Pass. The tabs were a Claude Code tab and "Shell B". Each tab's recording (`mh\history\recordings\*.cast`) had only its first resize, `126x42`, after the other tab was shown, and no `10x4`. |
| #65: resize the window and show the hidden tab again, and it takes the new size. | Pass. The viewport went to 900x600 with Shell B hidden. The shown tab resized to `116x32` at once. Shell B stayed at `126x42` until Ctrl+Shift+2 showed it 0.4 s later, and then it went to `116x32`. |
| Finding 10: on an empty window, the auto-opened launcher closes and the last closed session reopens on Ctrl+Shift+Alt+T. | Pass. Both tabs were closed through the API, Claude Code first. The launcher "Start a session" opened by itself, with the focus in "First prompt (optional)". Ctrl+Shift+Alt+T closed it, and "Shell B" came back as a new tab (`tc53419b66f78`). Peek read `PS C:\Users\Home Office>`, and the size was `116x32`. |
| The core's WSL distro list (slice 6k decision 5). | Pass. `GET /v1/agents/wsl-distros` answered `{"distros":["Ubuntu"]}` on this machine. Its profiles are Ubuntu and docker-desktop, and only Ubuntu is a detected WSL profile. |
| #62: Codex's section with leftover hooks while Codex isn't found. | Not checked live. With Codex's folder off `PATH`, `GET /v1/agents/codex/setup` still answered `found: true` (`exe_path` `...\Programs\OpenAI\Codex\bin\codex.EXE`), because detection merges the registry's user `PATH` (`profiles/detect.rs`). Staging a Codex that isn't found would mean editing the user's registry `PATH` or uninstalling Codex, and neither belongs in a check. The unit tests in `agentsState.test.ts` and `AgentsSection.test.tsx` cover it. Run 2 did show the seeded hooks as `orphaned`, because they name run 1's bridge. |
| The real `~/.claude/settings.json` is untouched. | Pass. Its modified time stayed 2026-10-25T14:04:04, before and after both runs. |

## Findings

1. **The reopened tab's first WebSocket gets a 404 (`/v1/pty/tc53419b66f78`), and the terminal then connects.** This is the "WS 404 retries" minor deferred from 6j2, which is outside this slice (decision 9). Not fixed.
2. **Run 1 left a third recording, `taa475dec29fc` (`36x12`), that no open or closed session names.** It was made 15 ms after the first tab, at app start. It predates this slice and doesn't touch #65. Not investigated.
3. **A Codex that isn't found can't be staged by taking it off the app's `PATH`.** Detection also reads the registry's `PATH`. A later check of #62 needs a machine without Codex, or a test hook in detection. Not fixed (it isn't a bug).
