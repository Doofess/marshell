# How agents take a pasted image reference

Research for slice 2.5b (region to prompt), 2026-10-17. Marshell pastes a region's PNG into the agent's terminal input as its quoted absolute path, in a bracketed paste of its own, followed by a second bracketed paste with ` <file>, page <n>: <comment>`. It never sends Enter. `attachment_ref` in `crates/core/src/profiles/detect.rs` gives every agent the quoted absolute path for now.

## What the sources say

- **Codex CLI.** The composer attaches an image only when one bracketed paste is exactly one image path (quoted is fine, drive and UNC paths are taken as they are, spaces are fine). A paste with other text in it stays plain text, and typed characters never attach. The model also has a `view_image` tool as a fallback. Source: openai/codex at ce0323f, `tui/src/bottom_pane/chat_composer/paste_input.rs` and `clipboard_paste.rs` lines 259–370.
- **Claude Code.** The docs say to give Claude an image path; a pasted or dragged image path is treated as an image (CHANGELOG 2.1.290), and the exact syntax is not documented. The fallback is the Read tool. Formats: JPEG, PNG, GIF, WebP.

## What the live check found (Windows 11, `pnpm tauri dev`)

A 3-page test PDF, the box moved onto "INVOICE TOTAL 4,217 EUR" on page 2 with the arrow keys alone, the comment "What number is in this box?".

| Session | Input line after Add | After one Enter |
|---|---|---|
| Claude Code | `invoice.pdf, page 2: What number is in this box?[Image #1]`: the path became an image chip about 20 ms after the paste, placed after the comment. | Answered "4,217" from the crop. |
| Codex CLI 0.163.0 | The quoted path as plain text, then the comment. No attached-image indicator. | Opened the file itself ("Viewed image ….png") and answered "4,217 EUR". No approval was asked. |
| PowerShell 7 (no prompt line) | Nothing pasted. The clipboard held `"<path>" invoice.pdf, page 2: What number is in this box?` and the notice said "PowerShell 7 has no prompt line, so the region is on the clipboard." | — |

In every case nothing was submitted, focus was in the session's terminal and the preview stayed open.

## Open

- **Codex on Windows does not attach the pasted path.** The exit criterion (the agent reads the crop) is met through `view_image`, but the composer doesn't show an attachment. A likely cause, not verified: on Windows the TUI reads console input records through ConPTY, and its terminal library emits paste events only on Unix, so the two bracketed pastes reach Codex as typed characters. Changing `attachment_ref` or the delivery for Codex waits for the user's call.
- Claude Code places the chip where the path was resolved, which ends up after the comment. It reads fine; sending the comment first would not change what the agent gets.
