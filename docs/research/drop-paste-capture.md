# Drop, image paste and capture

Research and live check for slice 2.5c, 2026-10-18. A file dropped on a terminal is pasted as its quoted path, a file dropped anywhere else opens in the preview, an image pasted with Ctrl+Shift+V is saved and its path pasted, and Ctrl+Shift+X captures a screen region with the Windows snipping tool and delivers it the same way. Marshell never sends Enter.

## What the sources say

- **Drop.** wry's drop handler reads only `CF_HDROP`, so a drop means files. The position is in physical pixels relative to the webview. Turning native drop on replaces WebView2's own drop handler, so HTML5 drag and drop inside the page stops working on Windows (tauri-utils `drag_drop_enabled` doc; tauri#14373). That is why the sidebar reorder now uses pointer events.
- **xterm.js 6.0.** Ctrl+V reaches the CLI as `^V` and fires no paste event. Ctrl+Shift+V lets the browser fire `paste`, which xterm handles on its textarea and container. A paste that holds only an image reaches xterm as `paste("")`, so Marshell takes it in the capture phase before xterm sees it.
- **Snipping tool.** The documented `ms-screenclip://capture…&redirect-uri` needs a packaged MSIX app. Bare `ms-screenclip:` is deprecated but works, and it puts the snip on the clipboard. Microsoft recommends `AddClipboardFormatListener` over polling `GetClipboardSequenceNumber`; Marshell polls every 150 ms, which needs no hidden window.

## What the live check found (Windows 11, `pnpm tauri dev`)

Drops were driven through Tauri's own drag events over CDP, with the positions in physical pixels. The test files were `my notes.txt`, `my report.pdf` and a folder.

| Where | What happened |
|---|---|
| Claude Code pane | The label "Drop to paste the path into Claude Code" and the dashed frame showed. The quoted path was pasted with no Enter, the notice said "Pasted my notes.txt into Claude Code.", and focus stayed in the pane. |
| Codex CLI 0.163.0 pane | The quoted path, "Pasted my notes.txt into Codex." |
| PowerShell 7 pane | The quoted path, and the notice. |
| Shell panel | "Drop to paste the path into the shell panel", then "Pasted my notes.txt into the shell panel.", and focus moved to the panel's terminal. |
| Sidebar, PDF | "Opened my report.pdf in the preview." |
| Sidebar, folder | "A folder can't be shown in the preview." |
| Preview, text file | The frame and label showed, then "Opened my notes.txt in the preview." |

Image paste (Ctrl+Shift+V with a 160×90 test image on the clipboard):

| Session | What happened |
|---|---|
| Claude Code 2.1.302 | `[Image #1]` in the input and "Added the image to Claude Code.". After one Enter (part of the check), Claude described the image: "live check" in white on steel blue. |
| Codex CLI 0.163.0 | `[Image #1]` in the input and "Added the image to Codex.". A path pasted on its own attaches here, unlike the region paste in `attachment-ref.md`, where the comment followed. Not submitted. |
| PowerShell 7, text on the clipboard | The text was pasted as usual, with no image notice. |

Capture (Ctrl+Shift+X):

| Case | What happened |
|---|---|
| A snip lands | "Select a region of the screen. Esc in the snipping tool cancels." showed and the overlay opened. A 160×90 bitmap put on the clipboard 3 s later, while the overlay still had the focus, was announced as "Added the image to Claude Code." after 3.5 s. The saved attachment is a 160×90 PNG with the right pixels, so the bitmap went through the BMP wrapping and the PNG conversion. Its quoted path was pasted with no Enter. |
| Esc in the overlay | The window got its focus back with no image, and "The capture was cancelled." followed 1.5 s later. |

## Fixed during the check

- The dashed drop frame was invisible on a terminal, because xterm's positioned, opaque viewport paints over its host's own outline. The frame is now drawn on an `::after` layer above the terminal.
- A path dropped on the shell panel left the focus in the pane beside it. The panel's terminal now gets the focus.

## Fixed after the branch review

- A dropped name was always pasted in double quotes, where PowerShell still runs `$(…)` and a backtick, and ends the string at a typographic quote, so a file named `a$(calc).txt` would run code when Enter was pressed after the drop. A name holding such a character is now pasted in the session shell's own single quotes (PowerShell's or POSIX's); cmd keeps double quotes, since it runs nothing from inside them. The rule also holds in an agent session, since the agent may have exited to its shell.
- Consecutive drops ran together. Each dropped path is now followed by a space.
- Capture first read the image with `navigator.clipboard.read()`, which failed with "Document is not focused" while the snipping overlay still had the focus. The image is now read in the Rust shell (the registered PNG format, or the bitmap wrapped as a BMP), which needs no page focus.

## Known limits

- With the preview closed, a drag over the sidebar shows no label, since there is nothing to mark. The drop still opens the file.
- A real drag from Explorer, and drops on a scaled display, are a hand check at release sign-off.
- A console error, "WebSocket … /v1/pty/<tab> 404", recurs while the Claude pane works. This branch does not touch the pty socket, so it is left for a later look.
