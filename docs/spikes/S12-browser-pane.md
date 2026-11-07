# S12: Browser pane, a WebView2 child webview (Windows)

Slice 6q-0, run 2026-11-07 on the dev box: Windows 11 Pro 26H2 (build 26300), one 3440×1440 monitor at 100%, WebView2 Runtime 157.0.4341.42, Tauri 2.12.2 with `unstable`, wry 0.57.0, webview2-com 0.39.1.
Build: the never-merged branch `phase-6q-0` under `pnpm tauri dev`, started by `live-check.ps1` with a scratch `MARSHELL_HOME` and CDP on 9333.
Plan: `docs/superpowers/plans/2026-11-07-phase-6q-0-browser-spike.md`. Decisions: `docs/PLAN.md` §9, "Slice 6q decisions (second round)" 2 to 6 and "Slice 6q-0 decisions".

Driver:
- The child webview `spike-page` sits over a placeholder in the preview slot. Rust logs to `<home>/spike-browser.jsonl`, and the loopback test site (`spikes/browser-pane/server.mjs`, port 47620) logs to `site.jsonl` in the scratch folder.
- Keys come from `spikes/browser-pane/keys.ps1` (SendInput, guarded by the foreground window's PID).
- The user did Alt+Tab, the edge drags, the Explorer drop and the scale changes by hand.

## Results

| # | Check | Pass criterion | Result |
|---|---|---|---|
| 1a | The child shows | The page draws inside the placeholder at 100% | **Pass.** The page drew in the placeholder (745, 102, 456×679 physical px at 100%); the logged bounds matched the placeholder's rect to within the half pixel Tauri rounds. The child took focus as it opened (`page-focus` 12 ms after `shown`). |
| 1b | CDP beside the child | CDP on 9333 still reaches the main webview | **Pass.** `/json/list` on 9333 lists only the main webview (`http://localhost:1420/`), and evaluation works. The child is not on CDP: see finding 2. |
| 1c | Browser arguments | The child's `msedgewebview2.exe` command line shows which arguments it got | **Our arguments, plus the variable's.** The child's browser process got `--disable-features=msWebOOUI,msPdfOOUI --remote-debugging-port=9333`: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` is added to the builder's arguments, not put in their place. SmartScreen protection is not disabled. The main webview keeps wry's `msSmartScreenProtection` off. |
| 1d | Refusals | `/redirect-file` and a typed `file:` URL are refused and logged | **Pass.** A typed `file:///C:/Windows/win.ini` was refused by `on_navigation` and shown as "refused: …". `/redirect-file` (302 to `file:`) never reached `on_navigation` as a `file:` URL: Chromium itself stopped it with `ERR_UNSAFE_REDIRECT` and showed its error page, which then retried the request twice on its own. |
| 2a | App keys from the page | Ctrl+Shift+K, Ctrl+Shift+Q, Ctrl+L and Esc: 20 of 20 each, at every stage | Pass. 20 of 20 for each of the four keys, at every stage: forwarded by the page, received by the app, acted on (palette open, pane hidden, address box focused, terminal focused), focus back in the page. The mouse did not move during the counted run. In an earlier run the user was clicking the window: 2 of 20 Ctrl+Shift+K presses went straight to the app, because a click had taken focus out of the page. Two other runs lost the foreground to another app, and the guard sent nothing. |
| 2b | Typing after a forward | `abc` lands in the palette's box | Pass. After a forwarded Ctrl+Shift+K, typed `abc` was the value of the palette's box (`combobox`, "Type a command, or a session number"). |
| 2c | A held chord | One action, with the repeats marked `repeat` | Pass, by another route. Held for 1 s, the page forwarded only the first press (`repeat: false`) and the palette opened once. The forward moves focus to the app, so the repeats land in the app's own webview, where they did nothing more (one dialog open). |
| 2d | Browser keys stay | F5, Ctrl+R, Ctrl+F and Ctrl+P act in the page | Pass. F5 and Ctrl+R: 5 of 5 reloads each (site log). Ctrl+F opened the find bar ("Find on page"), a popup window of the WebView2 process that takes the foreground while it is open. Ctrl+P opened the print preview inside the page, with focus on its Save button; leaving the page closed it. Each F5 reached the page as one keydown plus two `repeat` keydowns around the reload. |
| 2e | F12 | A DevTools window for the page; Ctrl+Shift+I reaches the app | Pass. F12 opened a DevTools window for the page only ("DevTools - 127.0.0.1:47620/…"); closing it gave the foreground back to Marshell, with focus in the page. Ctrl+Shift+I toggled focus mode (`data-mode` went expanded, focus, expanded). |
| 2f | AltGr | AltGr+Q types `@` in the page and forwards nothing | Partial. The user chose not to add a German layout. Ctrl+Alt+Q and Ctrl+Alt+Shift+K reached the page as keydowns with Ctrl and Alt set, and nothing was forwarded. Typing `@` with a real AltGr layout is not verified. |
| 3a | Tab in and out | 10 of 10 each way | Pass. 10 of 10 at every stage. Ctrl+L focused the address box. Tab entered the page (`page-focus`), with focus on its first link, "Page one". Shift+Tab went back out to the address box. Tab past the page's last link (`focus-out next`) landed on the "Marshell menu" button. The pane's Reload button is disabled until Task 6, so Shift+Tab lands on the element before the page, the address box. |
| 3b | Alt+Tab, page focused | Typing lands in the page's box afterwards | |
| 3c | Alt+Tab, terminal focused | Typing lands in the shell afterwards | |
| 3d | Clicks | Typing follows each click, page or terminal | |
| 3e | UI Automation | Both documents listed; focus sits under the page's document after 3a | Pass. `uia.ps1` listed both documents: the page ("Spike home") and the app ("PowerShell 7 3 — Marshell"). After 3a, the focused element was the page's link "Page two in a new window", under "Spike home". The first query after the app started missed the page's document; see finding 10. |
| 4a | Frame edges | The frame resizes where the page touches the edge; the smallest inset that works | |
| 4b | Divider drag | Follow mode against anchor mode | |
| 4c | Window moves | Maximize, snaps, minimize and the tray: bounds within 1 px | |
| 4d | 125% and 150% | 4c's bounds check and 4a's right edge at each scale | |
| 5a | Freeze | Timings from `freeze-start` to `hidden`; the user judges the flash | |
| 5b | No freeze | The palette is hidden behind the page | |
| 5c | Capture after a navigation | Old page or new | |
| 5d | Capture before the first load | The HRESULT | |
| 6a | Back, Forward, Reload | The bar follows each step | |
| 6b | Popup | `target=_blank` opens in the pane; no new window | |
| 6c | Profile | The cookie survives a restart; no old `msedgewebview2.exe` holds the profile | |
| 6d | A real site | `https://example.com/` renders | |
| 7a | Baseline isolation | Probe reports on the `windows` capability, site origin and Vite origin | |
| 7b | Scoped isolation | No version from either probe; the API refuses; the app's window still works | |
| 7c | Allow-list | `file:`, `tauri.localhost`, `ipc.localhost`, `tauri://` refused; `about:blank` and localhost open | |
| 8a | Drop on the terminal | Marshell's drop still works | |
| 8b | Drop on the page | Which events fire; no `file:` navigation | |
| 8c | Drop on the pane's bar | Which events fire | |

## Findings

1. **Creating the child takes focus.** `WebviewBuilder::focused` defaults to true. 6q-3's background open (6q decision 6) must pass `.focused(false)`.
2. **Two webviews, one CDP port.** The live check's `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` reaches every WebView2 environment in the process. The child's profile runs its own browser process, so it can't bind 9333 too, and has no CDP. 6q-3's live checks need the variable scoped to the main webview, or `additional_browser_args` with a second port on the child in dev.
3. **A `ResizeObserver` sees size changes only.** The child followed the placeholder when the status line pushed it down, because its height changed too. A pure move, with no size change, would not be reported. 6q-2 needs a position source as well, such as a layout effect that re-measures whenever the main grid's tracks change.
4. **wry moves focus to the main webview on the window's `WM_SETFOCUS` and `WM_ENTERSIZEMOVE`** (`wry-0.57.0/src/webview2/mod.rs:1325`, the parent subclass only the main webview gets). Expect Alt+Tab back, and the start of a frame drag, to take focus out of the page (tauri#13466). Task 3 checks it.
5. **The app's keymap needs no copy in Rust.** The page's `AcceleratorKeyPressed` handler only decides who owns a key (`route_key`). The app re-dispatches the key as a synthetic `KeyboardEvent`, and the existing keymap acts on it: the palette, the pane toggle and focus mode all worked that way. Ctrl+L and Esc are the two keys the spike handles directly, because they are about the page itself.
6. **A forwarded key ends the page's turn at the keyboard.** The forward moves focus to the app, so a held chord's repeats and the next key land in the app, not the page. When the app is done (the palette closes, the pane shows again), it has to put focus back in the page itself with `MoveFocus`. The spike's key test did, and 6q-3 must decide where that happens.
7. **The find bar is a window of its own.** Ctrl+F's bar is a popup owned by the WebView2 browser process, and it holds the foreground while it is open. Whether Marshell's chords pressed in the find bar reach the handler was not tested. Anything that checks the foreground window's process, such as the key script's guard, sees `msedgewebview2`, not Marshell.
8. **The app's document can't tell that the page has the keyboard.** While the child holds focus, the main webview's `document.hasFocus()` stays true. Only the child's `GotFocus` and `LostFocus` say where the keys go; the spike relays them as `page-focus` and `page-blur`. 6q-3 keeps that state in the app, and anything that asks "is the terminal focused?" has to ask it too.
9. **Tab off either end of the page arrives as `MoveFocusRequested`**, with the reason Next or Previous. With `Handled(true)` set, WebView2 leaves the focus to the app, and the app puts it on its own element on that side. Tab into the page is the app's own `focus` event on the placeholder, which calls `MoveFocus` with the matching reason, so the page starts at its first or last element.
10. **UI Automation builds the page's tree on first use.** The first `uia.ps1` query after the app started listed only the app's document. Later queries listed both. Whether a screen reader meets the same gap is for the release's NVDA pass.

## What 6q-2 and 6q-3 take from this

## Go or no-go
