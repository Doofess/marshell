# S4: Status line chaining (Windows)

Run 2026-10-18 on the dev box (Windows 11 Pro 26300) against Claude Code 2.1.302, in the same session as S10.
The scratch project's `statusLine` was `"<home>/bin/marshell-hook.exe" statusline --agent claude --tag mshl1`, and
`<home>/agents/claude/statusline-chain.json` held the user's real line, `python ~/.claude/statusline.py`. That script
was only run, never written.

| Check | Pass criterion | Result |
|---|---|---|
| Output unchanged | The bridge prints the same bytes as the original | **Pass.** Both the original and the bridge printed 708 bytes, identical by `cmp`. A second run of the original matched too, so the output is deterministic for one stdin. |
| Shown in the pane | Claude shows the user's line | **Pass.** It's the same line as without Marshell: icons, model, effort, context, rate limits and cost. |
| Core receives the update | The core's ring has the status JSON | **Pass.** Each post carried model, context_window, version, effort, cost and rate_limits. |
| Uninstall restores the original | Byte-for-byte | Not in 3a; the install and uninstall land in 3c. |
| macOS, Linux | Same | Not run; Windows only for now. |

How it was compared: the stdin came from the core's last `statusline` record. In Git Bash, the original
(`python ~/.claude/statusline.py`) and `marshell-hook.exe statusline` were each run on that stdin, and the outputs were
compared with `cmp`.

## Findings

1. **No tee fallback is needed.** The bridge runs the original through Git Bash (found next to `git` on `PATH`,
   `crates/hook/src/shell.rs`), so `~` in the user's command expands, and it passes stdout through untouched.
2. **The quoted path with a space works for `statusLine`** (S10).
3. **A post every second.** `refreshInterval: 1` means one core post per second per session, which
   floods the 200-event ring (S10, finding 4).

Decision: [x] chain through the bridge as planned  [ ] `marshell-hook tee | <original>` fallback
