# Tool profiles live check (slice 6c)

- Date: 2026-10-25, 14:34 to 14:50 local time.
- Build: `phase-6c` under `pnpm tauri dev` on Windows 11 Pro (build 26300). The first run was at 0515c0f; the npx re-check ran at 0f1275b.
- Home: a scratch `MARSHELL_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `MARSHELL_CLAUDE_SETTINGS`, with CDP on port 9333. Onboarding was left with "Skip: use limited status" for both agents, so no hooks were installed.
- Data: a scratch project with `vite/index.html` (one line of HTML) and `compose/compose.yaml` plus `compose/.env` holding the fake `FAKE_SECRET_6C=marshell-fake-6c`. Nothing was seeded into the scratch homes.
- Tools: npm 11.19.0 on Node 26.8.1, vite 8.3.5 (through `npx`), Docker 29.9.2, Python 3.13.15, gh 2.101.0, kubectl 1.36.1, 1Password CLI 2.41.0, the Google Cloud SDK. Terraform, Vercel, Wrangler, Netlify, Firebase, fly, aws and az are not installed.
- How it was driven: CDP evaluations returning small JSON, CDP key events and `Input.insertText`, emulated forced colors (not a real high-contrast theme), and 7 screenshots. The `webview` MCP server failed to connect, so a small CDP script did the same over Node's WebSocket. No desktop input. Recordings were checked only with `grep -c` on the cast file. No terminal text was read back over CDP.

## Results

| Check | Result |
| --- | --- |
| Tool presets in the picker | Pass. A Tools group after Shells lists "Vite dev server", "Next.js dev server", "Vercel dev", "Wrangler dev", "Netlify dev", "Firebase emulators", "Compose up", "Sign in", "Port forward", "Plan" and "Apply". |
| A preset whose program is missing | Pass. Start on "Plan" (terraform, not installed) shows "Not installed, or not on PATH." and starts nothing. |
| `npx vite` from the Vite preset: npx asks Ok to proceed, Needs you shows, then clears on `y` | Fail at 0515c0f, pass at 0f1275b. npm 11.19 prints `Ok to proceed? (y) `, which the shipped pattern `Ok to proceed\?\s*$` didn't match (finding 1). After the fix, `npx cowsay@1.4.0 hi` showed the row "Asks to continue. Node.js, Needs you: question." and the title "(1) PowerShell 7 — Marshell" for 10 s, and both cleared after `n`. |
| The chip `localhost:5173` appears and opens the browser | Pass. After `y`, the chip "localhost:5173" (name "Open http://localhost:5173/ in the browser", announced) appeared. Enter on it opened the default browser: 0, then 6 established connections on :5173. |
| Kill the server: the chip goes | Pass. Ctrl+C ended Vite; the chip went and the row read "Last command failed (exit 1)". |
| `npx next dev` in a minimal Next app | Not run. A Next app needs next, react and react-dom installed in the scratch project, too heavy for this check. The Next preset starts the same way as the Vite one. |
| `python -m http.server 8123` in a plain shell: a listening chip with no profile | Pass. The chip "localhost:8123", not announced, appeared after about two polls, and went on Ctrl+C. |
| `gh auth status`, `kubectl version --client`, `gcloud version`, `op --version`, without changing anything | Pass. Their rows read "Running gh auth status. GitHub CLI, Working.", "Running kubectl version --client. kubectl, Working.", "Running gcloud version. Google Cloud CLI, Working." and "Running op --version. Terminal program, Working." (op has no profile). None of them hid output, and none made a chip. While gcloud ran, its row turned to "Python, Working." (finding 3). |
| `docker compose config` with the fake `.env` value is neither scanned nor recorded | Pass. In the cast: `marshell-fake-6c` 0, "Output hidden" 1, "Output recorded again" 1, `633;E` 0. Run outside the app, the same command prints the fake value once. |
| A y/N prompt that changes nothing: `docker system prune` answered `N` | Pass. The row read "Asks to continue. Docker, Needs you: question.", the lane "Needs you · 1" and the title "(1) Vite dev server — Marshell". It stayed for 5 s while the prompt was on screen, and after `N` the title went back to "Vite dev server — Marshell" and the row to "PowerShell, Idle." |
| A scrubbed line: `docker login -u x -p notreal localhost:1` | Pass. The row read "Running docker login …. Docker, Working.", then "Last command failed (exit 1)". The cast has 11 lines with `notreal`, all within 3 ms of each other and before the command's first output: PowerShell redrawing the typed line one character at a time. None comes from Docker's output, and no `633;E` copy was recorded. The fake password is in no other file of the scratch home. |
| Keyboard: Tab to a chip, Enter opens, Shift+F10 opens the menu, Copy URL copies | Pass. Shift+F10 on the chip opened "Open in browser" and "Copy URL". Copy URL put `http://localhost:5173/` on the clipboard and focus went back to the chip. |
| Forced colors: chips still read as buttons | Pass (emulated). The chip keeps its border and gets a 2px Highlight focus outline. |
| `~/.claude/settings.json` untouched | Pass. Its modified time was 1792929844 before and after both runs. |

## Findings

1. **npx's prompt is `Ok to proceed? (y) `, so a launch through npx raised no Needs you.** npm 11 shows the default answer after the question, and the shipped pattern required the line to end at the `?`. Fixed in 0f1275b: the pattern is now `Ok to proceed\?(?:\s*\(y\))?\s*$`. The test `needs_you_patterns_match_their_texts` used to assert that `(y)` does not match; it now asserts that it does, and that the answered line `Ok to proceed? (y) y` does not (RED, then GREEN). Re-checked live as above. The research note's `(y)` moved from [unverified] to [local].
2. **The preset names "Sign in", "Plan" and "Apply" don't say which tool they start.** In the picker's flat list they sit under the Tools heading with no tool name. Not fixed: the names come from `profiles.json`, and naming them "GitHub sign in" or "Terraform plan" is a copy decision.
3. **A tool started through a `.cmd` shim is named after its interpreter while it runs.** `gcloud.cmd` runs `python`, and once the foreground reading sees the python process the row says "Python" instead of "Google Cloud CLI". The command line's tool is in `Foreground.tool`, but the row's name comes from `Foreground.program`, the deepest process, as it has since slice 5. A Needs you raised by gcloud would read "Python, Needs you". Not fixed.
