# S1: Webview gate (Linux WebKitGTK, macOS WKWebView)

Build: `pnpm tauri build --debug --no-bundle`, then run the binary.

| # | Check | How | Pass |
|---|---|---|---|
| a | Idle repaint | Run `while true; do date; sleep 1; done`; take hands off the keyboard and mouse for 30 s | Clock ticks every second with no input |
| b1 | Dead keys | US-International: `'` then `e` -> é; `~` then `/` -> `~/` | Exact characters, no doubles |
| b2 | IME | Japanese or Chinese IME: compose and commit a word | Committed once, no duplicates |
| b3 | Accented keys on WebKitGTK | German layout: type `äöü` | Once each |
| c1 | Flood | `yes \| head -n 2000000` | UI never freezes over 1 s; perf p95 stays < 30 ms while typing in the same tab afterwards |
| c2 | Big file | `cat` of a 50 MB text file | Completes; window stays responsive (move or resize during it) |
| d | Renderer | Perf overlay shows `webgl`, or `dom` with acceptable c1/c2 | As stated |

Linux variants to try in order if a check fails: `MARSHELL_LINUX_GFX=no-dmabuf`, then `no-compositing`, then `both`.

Known fixes to apply if needed: WebGL `preserveDrawingBuffer` (already on for Linux); the IME commit guard (orbit-desktop PR #23); the dead-key shim (xterm.js #5894).

## Results
| Platform | Machine / GPU / session | a | b1 | b2 | b3 | c1 | c2 | d | Variant | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| Windows 11 (WebView2), reference only | Dev box, 2026-10-10, Git Bash tab | Pass | Not run | Not run | n/a | No freeze. 2,000,000 lines took 2 min 40 s because MSYS `head` writes each line separately into the inbox ConPTY. Piped through `cat` it took 10.1 s. See S2 finding 3. | Pass: 52 MB in 5.9 s, responsive throughout | webgl | — | Typing p95 afterwards: pwsh 7.5 ms, Git Bash 23.3 ms (both pass after `ef659b0`, see below) |
| macOS | Pending: no Mac available yet | | | | | | | | | |
| Linux (WSLg) | Pending: this machine has no WSL distribution installed | | | | | | | | | |
| Linux desktop | Pending: no Linux machine available | | | | | | | | | |

**How the Windows reference row was measured:**
- The app was driven over WebView2 CDP.
- "No freeze" means CDP `Runtime.evaluate` round-trips stayed at 113–185 ms throughout the flood, against a baseline of about 115 ms (mostly node startup).

**Typing latency by shell.**
- Before the fix, pwsh measured p50 16.1 / p95 21.7 ms, and Git Bash measured p50 31.2 / p95 32.5 ms (two frames instead of one).
- Cause: the pty socket's coalescing held even a lone echoed keystroke until its 8 ms deadline. On Windows that deadline is really about 15.6 ms, because tokio rounds sleeps up to the system timer tick.
- Fixed in `ef659b0`: output after an idle stream is sent at once, and only sustained output is coalesced.
- After the fix, pwsh measures p50 4.3 / p95 7.5 ms, and Git Bash measures p50 16.5 / p95 23.3 ms. Both pass.

## Decision
- [ ] PASS: keep Tauri
- [ ] FAIL without a clean fix: switch the window shell to Electron (core and frontend unchanged)

Open until macOS and Linux runs exist. Windows WebView2 shows no webview-side problem.
