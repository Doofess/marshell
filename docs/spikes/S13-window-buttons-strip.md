# S13: The window buttons in a native strip (Windows)

Resize-polish slice, Task 1, run 2026-11-20 (the spike's code is the never-merged branch `resize-polish-spike`, commit d818fdcb) on the dev box: Windows 11 Pro (build 26300), one monitor at 100%, WebView2 157.
Build: `resize-polish-spike` under `pnpm tauri dev`, started by `live-check.ps1` with scratch homes and CDP on 9333.
Plan: `docs/superpowers/plans/2026-11-20-resize-polish.md`. Decisions: `docs/PLAN.md` §9, "Resize-polish slice decisions (2026-11-20)", decision 2.

**Verdict: go, for a plain Win32 child window. No-go for a child webview.**

## The question

The window has no system frame, and its minimize, maximize and close buttons are drawn in the webview. WebView2 draws a resized webview a frame or more late (S12 finding 23), so the buttons trail the window's right edge while it is dragged. Can something that isn't the webview hold the buttons' place and keep up?

## Driver

`src-tauri/src/strip_spike.rs` puts two blocks of 138 x 40 logical pixels against the window's right edge, where the three buttons would go:

- **A** is a plain Win32 child window of the main window (its own class, the system's highlight colour, `WM_MOUSEACTIVATE` answered with `MA_NOACTIVATE`), at y = 40.
- **B** is a wry child webview (`window.add_child`, `about:blank`, made with `focused(false)`), at y = 84.

Both are moved from Tauri's `Resized` event: A with `SetWindowPos`, B with `set_bounds`. `MARSHELL_STRIP_SPIKE=subclass` would instead move A inside the window's own `WM_WINDOWPOSCHANGED`; it was never needed.

The user dragged the window's edges and compared the app's own buttons, A and B by eye. The clicks were sent by a script as real mouse input, each one only after checking that the point lies in a window whose root is the check's Marshell window; focus was read from `GetGUIThreadInfo` and from focus and blur events recorded in the page.

## Results

| # | Check | Result |
|---|---|---|
| 1 | Right-edge drag, slow and fast | **A keeps up.** The app's own buttons and B trail. |
| 2 | Left-edge drag, slow and fast | All three stay still. |
| 3 | Top-right corner drag, slow and fast | **A keeps up.** The app's own buttons and B trail. |
| 4 | Maximize and restore | A and B both land on the right edge. |
| 5 | Right-edge drag with a browser page open in the side pane | **A keeps up.** The app's own buttons and B trail. |
| 6 | A click on A | The keyboard stays in the terminal: no blur reaches the page, and the focused window is still the app's webview. |
| 7 | A click on B | B takes the keyboard: the page gets a blur, and the focused window is B's webview. |

## Findings

1. **A plain child window moved from the `Resized` event keeps up.** Windows composes it with the frame, so it never shows the late frame the webview shows. The subclass path wasn't needed.
2. **A child webview is no better than the page.** It is drawn by the same WebView2 that draws the app late, and a click on it takes the keyboard, `focused(false)` or not.
3. **The user's own click test said neither block kept the keyboard.** The scripted clicks, with focus read at both levels, say A keeps it and B takes it. The likely reason is the order by hand: after a click on B the keyboard is in B, and a click on A doesn't bring it back, because A never moves it. A strip that acts on a click must not rely on the keyboard having stayed where it was; it gives it back to the app's webview when it has taken an action.
4. **A page is drawn above the blocks.** A browser page is a child webview made later, so it sits above A and B in the window's stacking. A strip in the title bar's 40 px never overlaps a page, which starts below the bar.
5. **Win+Left and Win+Right don't snap the window,** by the user's try. The window has `WS_THICKFRAME`, `WS_CAPTION` and `WS_MAXIMIZEBOX`. Not looked into here; it isn't the spike's doing as far as the styles show, and is a lead of its own.

## What a strip has to do (for its plan)

The spike drew nothing. A strip that replaces the look of the three buttons has to:

- draw the three glyphs and their hover, pressed and disabled states natively, in the app's theme, accent, high contrast and forced colours, at every display scale, and redraw when any of those change;
- stay out of the way of anything the page draws over the title bar's right end (a dialog's scrim, a menu, a drag overlay), the way a browser page is covered today (`useCoverPage`);
- leave the page's own buttons in place for the keyboard and for screen readers, or stand in for them through UI Automation;
- give the keyboard back to the app's webview after a click;
- exist only on Windows. The page's buttons stay the implementation everywhere else and the fallback here.

The address bar's buttons in a browser tab trail the same way and aren't helped by this: they sit in the middle of the pane's layout, not at a fixed place against the window's edge.
