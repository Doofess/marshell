# Folder picker live check (slice 3f)

- Date: 2026-10-19
- Build: `phase-3f` at add8e88, `pnpm tauri dev` on Windows 11 Pro (26300)
- Home: a scratch `MARSHELL_HOME` with spaces in its path (`%LOCALAPPDATA%\Temp\marshell live 3f\home`), with the scratch folders `my project` (a git repo on `master`) and `picked one` (not a repo) next to it
- Driven over CDP (`--remote-debugging-port=9333`) with trusted input (`Input.insertText`, `Input.dispatchKeyEvent`). The native dialog was found through UI Automation as a `#32770` child of the Marshell window

## Results

| Check | Result |
|---|---|
| Type the `my project` path | Pass: "Checking …" (disabled), then "Use <path>" as the first row. Enter set the chip to "my project · master" and offered the worktree chip |
| Type a missing path `…\nope` | Pass: "No folder at …" (disabled) as the first row, and Enter did nothing |
| Type `/x` | Pass: no path row. Only Browse… and "No folder matches “/x”" |
| Paste the path in double quotes | Pass: the quotes were dropped and the folder found |
| Ctrl+O with a typed folder | Pass: "Select Folder" opened, owned by the Marshell window, at the typed folder |
| Ctrl+O again while the dialog is open | Pass: still one dialog, and none after it was cancelled |
| Cancel the dialog | Pass: the typed text stayed, focus went back to the folder field, and no error showed |
| ArrowDown to Browse…, Enter, then OK | Pass: the dialog opened, and OK took the folder |
| Browse to `picked one`, a folder never used | Pass: the chip read "picked one", with no worktree chip |
| Start PowerShell 7 in the typed `my project` | Pass: the tab read "PowerShell 7 my project", the shell's `Get-Location` was that folder, and `projects.json` listed it |
| Type `C:\`, use it, start PowerShell 7 | Pass: "Use C:\" was offered and the shell started in `C:\`. `projects.json` still listed only `my project` |
| Forced colours (emulated with `Emulation.setEmulatedMedia`) | Pass: the disabled row's name and path were GrayText (`rgb(63, 242, 63)` in the High Contrast Black palette), and Browse… was CanvasText. Without forced colours the disabled row used `--text-2` at weight 400 |

## Notes

- The webview first loaded `about:blank` because the dev server wasn't ready yet. `location.replace('http://localhost:1420/')` over CDP loaded the app.
- Screenshots came out black (`CopyFromScreen` on the WebView2 window), so the dialog was read through UI Automation and the forced-colours check through computed styles.
- The dialog's buttons weren't reachable through UI Automation by their IDs, and its Folder edit isn't exposed. Cancel and OK were sent as `WM_COMMAND` `IDCANCEL` and `IDOK` with `PostMessage`. To pick a folder, the check typed it first so the dialog opened there.
- A mode chip that moved from Manual to Plan and then Auto-edit came from the check's script, not the app. While the folder field is open it is an input rather than a chip, so the script's second chip was the Mode chip, which cycles on click and Enter. Opening the folder chip by its role fixed the script, and the mode then stayed Manual.
- Using `C:\` showed Claude as the program, because a folder with no session started in it falls back to the default program. This is the existing preselect behaviour, not part of 3f.
- The shells were started for this check, and Enter was sent only into them. The app and its `pnpm`, `cargo`, `node` and WebView2 children were stopped by the PID started for this check, and the scratch folders were removed.
