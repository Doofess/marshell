# Slice 6q: options for the tabbed pane (research, 2026-11-07)

Purpose: give the user what is needed to decide the follow-up questions of slice 6q, and give each later slice a plan to start from. Nothing here is built. Checked against the repo on branch `phase-6o` and against upstream sources on 2026-11-07. The user's answers are in `docs/PLAN.md` §9, "Slice 6q decisions (2026-11-07, second round)".

How to read the marks. **Verified** means a second reader checked the claim against the source. **Unverified** means one researcher read it and nobody checked it. The split-removal and Preview/integration areas had no second reader, so their findings are unverified (they cite file and line, and most are plain reads of this repo; the spot checks I made are marked). **Unclear** means the second reader could not settle it. A claim a verifier called wrong is not relied on.

## 1. What was asked

Decided on 2026-11-07 in `docs/PLAN.md` §9, "Slice 6q decisions (2026-11-07, first round)", items 1 to 10:

- Split view goes completely (item 2). A saved split opens as its focused session.
- The Preview (slice 2.5) becomes a per-session tabbed pane on the right, resizable with a remembered width, with Ctrl+Shift+Q to show and hide it (items 3 to 5).
- Three main tab kinds: a real web browser (item 6), editable text files (items 7 and 8), and PDF with find, outline, thumbnails, highlights and notes (item 9). Markdown and images stay.
- Tabs open from Ctrl+click, port chips, drop, quick open, Open file…, Open URL… and a + menu (item 10).

Already decided earlier and not re-asked: the slice 2.5a/b/c decisions (Ctrl+click opens paths, quick open, files read through the token-checked API, reload on change, drop behaviour, region-to-prompt delivery by paste and never Enter, no eval and no WebAssembly in the webview).

## 2. Findings per area

### 2.1 Removing split view (unverified)

Split view runs through one layout state (`ViewLayout`: panes, arrangement, ratios, active, zoomed), one component stack (`SplitView`, `Pane`, `LayoutMenu`, `splitLayout`) and the keymap, navigation, palette and row layers. Source: `src/features/window/viewState.ts:8-255`, `MainWindow.tsx:313-345,356-384,574-582`, `LiveApp.tsx:983-1263`.

What the removal must not break:

- The single-terminal path is already separate. `MainWindow.tsx:574-582` renders `KeptTerminals`, not `SplitView`, when there is one pane.
- The Preview divider uses the `.split__handle` class, which lives in `SplitView.css`. I checked `MainWindow.tsx:588`. Move those rules out before deleting the file. `noBoxedOutlines.test.ts` and `forcedColorsEdges.test.ts` name `SplitView.css` and need edits.
- The selected session is saved only as `panes[active]`, so a replacement field is needed.
- `SessionHeader`, `KeptTerminals`, the top-bar restart and Pause bars, and `PortChips` stay.
- The full Composer and the StatusBar are switched off in the live app (`LiveApp.tsx:1733-1734`), so removing the compact prompt line changes nothing users see today.
- The 80-column floor has no helper. `CELL.w=8`, `padX=16` and the 80x24 `COMFORT` constant sit in `splitLayout.ts:17-20`. Copy them out first.
- The shell panel's "Open in a pane" adopts the shell as a session. It is not split-only. Keep it and rename it.

Saved layouts. `ViewLayout` has no `#[serde(default)]` (`crates/protocol/src/api.rs:418-453`), so a file missing a field fails to parse and the whole layout falls back, saved tabs included. Option A keeps the file shape and folds on load.

| Option | Cost | Risk | Verdict |
|---|---|---|---|
| A. Keep the `layout.json` shape. Core `repair` folds a split to its focused session (zoomed pane, else active, else the first non-empty); `validate_view` tightens to one pane; the frontend folds on load and writes `panes:[id]` | Small: about 40 lines of Rust and test updates | Low. Dead fields remain; an older build can still read the file | Recommended |
| B. Replace the split fields with `selected`, read old files through a legacy shape | Medium | Medium. A downgrade silently loses saved tabs | Later, in the tabs foundation, which adds `#[serde(default)]` fields anyway |
| C. Keep up to four panes in the schema, never create more than one | Smallest | A saved split still reaches the frontend and disagrees with the core | Not recommended |

The fold rule must match the frontend's `focusedIndex` (`viewState.ts:113`). The frontend writes the view only when it differs from what it loaded (`useViewState.ts:64-68`), so it must fold on load too, or the split stays on disk until the next change.

Freed keys: Ctrl+Shift+D and Ctrl+Shift+P. Ctrl+Shift+Tab ("next terminal on screen") has nothing to cycle. `ShortcutSheet.tsx:26` keys a line on `nav-next-pane` and needs re-keying. The keyboard navigation cursor is simplified, not deleted: target kind `pane` stays as slot 0 because rows and the top bar reach it.

Docs and tests that name split view are listed in the research appendix of this slice's plan: `docs/PLAN.md` lines 91, 129, 139, 141, 142, 203, 315, 676, 682, 717, 771, 789, 866-881 and 1040, `docs/BRIEF.md`, `README.md`, `docs/a11y/screen-reader-script.md` A5 and A8, `src/design/review/renderReview.tsx`, `scripts/build-review.mjs`, and about 30 test and story files. Counts come from grep only.

### 2.2 The Preview and its integration points (unverified)

Today the Preview is one global, in-memory, one-file-at-a-time history in `LiveApp` (`previewState.ts:4-63`). It is not per session, not saved, and has no tab kind (the kind is worked out from the extension, `kinds.ts:23-30`). Its width is local state, "not saved; every opening starts at half" (`MainWindow.tsx:404-426`).

What carries over unchanged: the file API (`GET /v1/files/stat` and `/content`, caps 2 MB text, 20 MB image, 50 MB PDF, read-only; `crates/core/src/server/files.rs`), the one-second stat polling (`usePreviewFile.ts`, `reload.ts`), quick open (`quickOpen.ts`, already session-scoped), drop (`dropTarget.ts` sends anything not on a terminal to the preview), region-to-prompt for PDF and images, and the Markdown, image and code views.

What must change:

- State becomes per session: `{ tabs[], activeId }` with a typed tab union (browser, text, pdf, markdown, image), saved in `layout.json` with serde defaults.
- `PreviewContext.open(entry)` becomes an open-tab call that takes a typed target.
- Layout: a saved width and the 80-column rule replace `previewRatio`. The full-width-below-720-px rule in `previewLayout.ts` stays as the narrow-window behaviour.
- Polling runs for the visible tab only. Hidden tabs refresh when shown.
- Tab reorder uses pointer events, not HTML5 drag and drop, because native file drop turns the latter off on Windows (`CLAUDE.md` gotchas).
- Terminal links today handle paths only. There is no URL link provider, so Ctrl+click on a URL is new work.
- Port chips open the system browser only for loopback (`PortChips.tsx:47`, `ports.ts`). The change is small: call "open browser tab".

Not found anywhere: a core push event for file changes (the config watcher in `config/watch.rs` serves previews only), a user-file write path, and any caller of `merge3` outside its tests. The `/src/features/capture` folder is the snipping tool and is unrelated.

### 2.3 The web browser kind (partly verified)

The only route that meets "a real browser, any site, signed in, isolated" is a native WebView2 per tab, added as a Tauri child webview over a placeholder element. An iframe is blocked by the CSP (`default-src 'self'`, no `frame-src`, `src-tauri/tauri.conf.json:26`) and the CSP is never loosened. A separate window, and CEF through Tauri's v3 alpha runtime, are worse; those two comparisons are reasoning, not sources (unclear).

Verified facts (versions: tauri 2.12.2, wry 0.57.0, webview2-com 0.39.1):

- `add_child` needs Tauri's `unstable` Cargo feature, which is off. There are 23 open issues under that label, and a maintainer said in September 2025 the team has no capacity for multiwebview issues (wry#1177). Source: `tauri-2.12.2/src/window/mod.rs:1224`.
- A child webview always draws above the main webview and there is no API to reorder (tauri#15682, #11124, both open). No HTML overlay can draw over a page: palette, dialogs, menus, toasts, tooltips, the + menu, the region box.
- Linux is blocked upstream: child bounds are ignored (tauri#10420, fix PR #15463 still open) and Wayland bounds are wrong (#15656). macOS has open keyboard bugs under `unstable`.
- Isolation is well supported. Tauri refuses IPC from any non-app origin unless a `remote` capability exists, and Marshell has none. One fix is needed: scope the default capability by `webviews` instead of `windows`, because a child in window `main` would otherwise inherit the window's permissions when it shows a local origin such as the dev server (`tauri-2.12.2/src/ipc/authority.rs:462-489`). Add `on_navigation` allowing only http, https and about:blank, and blocking `*.localhost` custom-protocol hosts.
- One profile folder via `data_directory` keeps sign-in across restarts. Each folder is its own process group, and every environment on one folder must use identical browser arguments.
- Tauri exposes no Back, Forward, screenshot, certificate or history events. Each needs raw WebView2 COM through `with_webview`, and separate code for WKWebView and WebKitGTK. Marshell already uses `with_webview` in `src-tauri/src/main.rs:170`.
- DevTools: in release without Tauri's `devtools` feature, WebView2's `AreDevToolsEnabled` is false for every webview. F12 on the child needs `SetAreDevToolsEnabled(true)` on that child plus `OpenDevToolsWindow` (bindings exist in webview2-com-sys 0.39.1).
- Keystrokes inside the child never reach the main webview. Forwarding needs a raw `AcceleratorKeyPressed` handler per child. The handler runs synchronously, so it may only set `Handled` and post an event; it must not call back into the app. Whether this works well is **unclear**: a maintainer reported in wry#451 (2021, old SDK) that it "didn't work" for Ctrl combinations. This is the go or no-go test.
- `CapturePreview` (screenshot) fails before the first content load, and after a navigation starts it can still return the old page. Capture first, hide second.
- Popups that need `window.opener` cannot become tabs: Tauri's `NewWindowResponse::Create` needs a top-level window sharing the opener's environment.

Correction to the research. It says Marshell does not set `decorations: false`. It does on Windows: `src-tauri/tauri.windows.conf.json` sets `decorations: false` and `shadow: true`. That is the setup of tauri#12207 and #9634 (window resizing breaks with child webviews and a custom title bar, both open). Resize and sash-drag behaviour with the frameless frame is therefore a first-order spike item, not a footnote. Also retest tauri#13466 (Alt+Tab focus) and #10521 (native file drop, which Marshell relies on).

Overlay options:

| Option | Cost | Risk |
|---|---|---|
| Hide the page and show a `CapturePreview` image while any DOM overlay is open or a sash is dragged (hide-and-swap). Region-to-prompt then happens on the frozen screenshot, reusing `RegionLayer`, `RegionComment` and `cropImage` | Medium | A short flash on every overlay; the capture is asynchronous. Recommended |
| Add window-region cut-outs around each DOM layer, later, for tooltips and toasts | Medium, second phase | Per-layer geometry bugs |
| WebView2 composition hosting through a vendored wry fork (what the Orden app did; its issue lxorb/orden#1137 closed 2026-11-06) | Very large | IME, pen, DPI and device loss were unverified there; accessibility likely regresses; permanent fork upkeep; Windows only. Do not start here |

Verdict: build Windows first, behind a spike. Show macOS as "preview" and Linux as "not available yet" in the UI, as AGENTS.md requires. Treat Linux as blocked until tauri#15463 ships.

### 2.4 The text editor kind (partly verified)

Recommendation: CodeMirror 6, with a small Marshell-owned extension that colours from Shiki.

- CodeMirror needs no eval, WebAssembly, workers or blob (verified by grepping the shipped `dist` of view, search, merge and commands). Its injected styles fit the existing `style-src 'self' 'unsafe-inline'`. Size is about 435 KB raw and 147 KB gzip with search, history and one language (measured by the researcher; not independently measured).
- Monaco: default workers are `blob:` workers, which the CSP blocks (verified in vscode source); themes accept hex colours only, so Marshell's CSS tokens cannot drive them; forced-colors mode uses a fixed palette, against the rule in `.claude/rules/ui.md` that forced-colors blocks use system colours. About 2.8 MB raw for the core. Rejected.
- Ace and a textarea overlay: no advantage over CodeMirror, or too much work.
- Language coverage follows Shiki (361 grammars, all running on the JS regex engine, verified). Sublime's default packages have 120 non-embedded syntaxes (verified). The "about 100 of 120 covered" figure is the researcher's hand mapping (unclear). About 20 have no grammar (ASP, Binary, Graphviz DOT, Cabal, Textile, JSP, Make Output, OCaml tools, LaTeX Log and others); those open plain.
- "The file types Sublime knows" is a mapping Marshell must own: extension, exact filename (Makefile, Dockerfile, .gitignore), and shebang or first line. `kinds.ts` has about 55 extensions and does not match names without a dot (unverified).
- The bridge: `codeToTokens` accepts and returns a `grammarState`, so tokenizing can resume mid-file (verified in `@shikijs/core` 4.5.0). Colours are `var(--code-token-*)` strings that work as inline styles. The researcher measured about 1.5 s per MB on Node; I rate this unclear and it must be re-measured inside WebView2. Tokenize the viewport first, in idle chunks, and stop colouring above a cap (today's Preview cap is 300 KB).
- Theming: today's `--code-*` variables come from the terminal's ANSI palette (`highlight.ts:17-36`), not from Marshell's tokens. Decision 8 asks for one light and one dark scheme from Marshell's tokens, so new `--code-*` values are needed and must be contrast-checked in both schemes. A yellow or bright-green ANSI value on white would fail.
- Keyboard: CodeMirror does not trap Tab by default; Esc then Tab leaves, and Ctrl+M toggles tab focus (verified, codemirror.net/examples/tab).
- Forced colors: none of the CodeMirror packages has a forced-colors rule (verified). Marshell writes a block with system colours for selection, cursor, active line, find panel and merge chunks.
- Screen readers: unclear. The find inputs have aria-labels (verified). The announcer and aria-hidden gutters were not checked, and nobody ran NVDA on WebView2. This is the one thing that could flip the choice, so run an NVDA pass right after the editor slice, not only at release sign-off.
- Known limit: CodeMirror's drag-to-move text uses HTML5 drag and drop, which native file drop disables on Windows.
- Latent risk: Tauri adds a nonce or hash to `style-src` at build time if the served HTML ever has a `<style>` element, and a nonce makes browsers ignore `'unsafe-inline'`. `index.html` has none today. Add a check on the built app that `style-src` carries no nonce.
- Merge. `@codemirror/merge` is two-way only. `diffy::merge` in the core (`config/merge.rs`) is three-way but has no route and no caller. Plan: keep the last-saved text as the base, send base, mine and disk to a new core merge route, apply a clean merge in place, and for a conflict offer Keep mine, Take disk, or open with conflict markers. That needs no merge dependency in the first editor slice.

### 2.5 Saving user files (unverified)

The config write path (ChangeSet, preview, apply: per-path lock, expected-hash check, snapshot of 20 per file, temp file and rename, undo) is sound, but it is a config allowlist: home, project folders and `<home>/agents`; UNC, device and managed names are refused; text only, 4 MB; and `.claude/rules/local-api.md` says "No route takes a file path to write" (read: line 14). A PDF save needs bytes, up to 50 MB.

| Option | Cost | Risk |
|---|---|---|
| A separate small user-file write service that reuses `write.rs`'s atomic replace and hash code and the snapshot code, with its own scope, byte and text modes, an expected size and mtime check, and the same refusals (UNC, device, Marshell home, managed names, links to them) | Medium | Two write paths to keep aligned; needs an explicit ruling on the "no path to write" rule. Recommended |
| Widen the config guard to user files | Medium | Changes what the guard means; diff preview is awkward for Ctrl+S |

`modified_ms` alone is a weak token (mtime in milliseconds; FAT has 2 s granularity). The save checks size and mtime together and may add a hash of the loaded bytes.

### 2.6 The PDF kind (mostly verified)

All four additions build on pdf.js 6.4.299, already shipped. `pdfjs-dist` ships no find bar, outline, thumbnail, sidebar or comment-dialog UI, so Marshell writes all of it.

- Find: `PDFFindController` runs without `PDFViewer`, driven by an EventBus and a link service (verified exports). Highlights land on the wrong text unless the text layer is built with `disableNormalization:true`; `pdfjs.ts` calls `streamTextContent()` with no options (verified). `pdf_viewer.mjs` reads `globalThis.pdfjsLib`, so set it before importing (verified).
- Outline: `getOutline()` and `getDestination()`; a `role=tree` list that Marshell writes. Outline URLs must go to the browser-tab policy, never navigate the app webview.
- Thumbnails: lazy renders at low priority, one or two at a time; reuse the main view's canvas when drawn.
- Highlights and notes: `saveDocument()` is an incremental update that appends to the original bytes (code path verified; the byte-level experiment is the researcher's). A highlight is a standard `/Highlight` with an appearance stream; a comment is `/Contents` plus a `/Popup`. Rendering in Acrobat, Edge and Chrome is not verified and needs a manual check of saved files.
- Free text in non-Latin scripts is saved with no appearance stream, so Chrome and Edge may show nothing. The fix upstream (PR #21858) is Firefox-only (verified).
- Keyboard. The finding that "highlight has no keyboard path" is wrong. `uiManager.highlightSelection()` is public and the `editingaction` event handles `highlightSelection` and `commentSelection` (`pdf.mjs:3019,3425-3433`). Marshell can select the current find match programmatically and dispatch it, giving a keyboard path with no quad-point code. That removes the main reason to build option B below. Untested.
- Key hijack (verified by reading code): while an editor mode is on, the editor's window-level key handler runs for any target that is not a text `<input>`, which includes xterm's textarea. Ctrl+Z, Ctrl+A (readline's line start), Backspace and Delete would be stopped. Set the editor mode to NONE when focus leaves the pane, and add a regression test that those keys reach the terminal.
- Whether `PDFViewer` is the only way to get the editor is unclear: `PDFPageView` is exported and takes an external UI manager, so a middle path may exist.
- CSP: pdf.js 6 has no eval option left (the `isEvalSupported:false` in the 2.5b plan is obsolete and harmless). With WebAssembly off, JBIG2, JPX and CCITT use JS fallbacks; ICC profiles have no fallback and use the alternate colour space. Marshell already passes `useWasm:false`.
- A dev-only bug is likely: `vite.config.ts:31` serves every `/pdfjs/` file except `.wasm` as `application/octet-stream`, so the JS decoder fallbacks loaded by `import()` may fail in `pnpm tauri dev`. One-line fix; confirm with a scanned JBIG2 or JPX PDF.
- The standard pdfjs-dist 6 build uses `Math.sumPrecise` and `Map.getOrInsertComputed` without polyfills; fine on current WebView2, unconfirmed on WebKitGTK and WKWebView.

PDF options: **A** adopt `PDFViewer` for the whole tab (large; replaces `PdfView` and the region overlay hook); **B** keep the custom renderer and write annotation values directly into annotation storage (brittle internal format, now unneeded for keyboard reasons; rejected); **C** staged: find, outline and thumbnails on the current renderer, then move to `PDFViewer` (or a `PDFPageView` assembly, if a prototype proves it) for highlights, comments and free text (recommended); **D** read-only additions only (contradicts decision 9).

Saving flow: the client sends the saved bytes with the expected size and mtime; the core re-stats, refuses on a mismatch, writes a temp file in the same folder and replaces atomically. On a conflict the tab offers Save as a copy, Overwrite, or Discard edits and reload. Never re-apply in-memory edits to a changed file. A password-protected, signed or no-modify PDF opens read-only in the first version. A reload with unsaved annotations loses them, so the tab needs a dirty state and a notice. On Windows an atomic replace can fail while another program holds the file; the error offers Save as a copy (unverified).

### 2.7 Agents opening pages (unverified)

There is a `marshell` CLI with `list`, `send`, `new`, `focus` and `uninstall` (`crates/cli`). A new `open` command and route `POST /v1/sessions/{tab}/open` copy the `focus` pattern. The window gets the command as a one-shot event; the repo has a Tauri event for focus (`CoreEvent::Focus`), but the `/v1/events` socket carries only state snapshots, so the frontend needs a small queue there to stay core-API-only. The route sits inside the same host, CORS and bearer layers, needs a request type with `ts(export)`, and a test that it refuses without the bearer. Every CLI gets it equally because it is not agent-specific.

It must refuse: any URL scheme but http and https, userinfo in a URL, the core's own port and the Tauri origins, a path that fails `refused()` (UNC, device, NUL, reserved names), folders, a session that does not exist or has ended. It opens in the background, without taking focus or raising the window, with a cap on tabs per session and a rate limit, because any process of the same user can read `endpoint.json` and call it. Whether `marshell` is on PATH inside agent sessions was not found; check it in the live check.

## 3. Choices and recommendations

Decided here from the research, not asked:

- Layout file: option A in 2.1. Schema cleanup moves into the tabs foundation.
- One shared browser profile for every session, kept under `MARSHELL_HOME` (so live checks use a scratch profile). It is the natural reading of "stays signed in". Per-session profiles cost a process group each.
- Region to prompt on a page: the visible area, from the frozen screenshot, Windows first.
- Shell panel "Open in a pane" becomes "Open as a session" and selects the new session.
- Ctrl+Shift+Tab is removed in 6q-1; reuse for moving between terminal and pane is a 6q-2 call. Ctrl+Shift+D and P stay unbound until a slice needs them.
- Reload on change: poll the visible tab only. A push event is later work if needed.
- Markdown web links: a link opens in a browser tab, with a "system browser" action beside it, replacing the current question.
- Hidden browser tabs: cap live tabs per session (propose 8), suspend background ones, keep the URL.
- WebView2 SmartScreen: turn it on for the browser (wry's default arguments switch it off, and custom arguments replace the defaults). Decide the arguments once, in the spike.
- Tab strip semantics (ARIA tablist, roving focus) follow the pattern of the existing rail and sidebar; written in the 6q-2 plan.
- DevTools: `SetAreDevToolsEnabled(true)` on the browser child plus `OpenDevToolsWindow`. The whole-app `devtools` feature stays off.

Asked of the user, and answered on 2026-11-07: `docs/PLAN.md` §9, "Slice 6q decisions (2026-11-07, second round)".

## 4. Proposed slicing

Each slice gets its own plan, live check and pull request. The order puts the removal first (it is mechanical and frees the space), proves the risky browser design with a time-boxed spike before the foundation fixes its geometry, and ships the browser before the editor because it carries the most risk.

| Slice | Content | Exit check |
|---|---|---|
| 6q-0 spike (no merge; findings in `docs/spikes/`) | A child webview on the real frameless window: own data folder, Rust-owned bounds on resize and sash drag at 100%, 125% and 150%, hide-and-swap on palette open, key forwarding of Ctrl+Shift+K, Ctrl+Shift+Q and Esc plus Tab out, Alt+Tab, file drop, F12, CapturePreview, Back and Forward, NVDA or Narrator at the placeholder | A written go or no-go. If key forwarding or focus return is unreliable, stop and re-plan with the user |
| 6q-1 split removal | Core fold and tightened validation with tests; frontend reducer; `.split__handle` moved; strip `MainWindow` and `LiveApp`; delete the stack, `rowPick`, compact Composer; keymap, palette, nav; stories, docs, review page | A saved 3-pane layout loads as its focused session; the single window, Preview divider, `KeptTerminals` and top-bar bars still pass; `cargo test`, `pnpm test`, `check:generated`; live check |
| 6q-2 pane and tabs foundation | Per-session typed tab list in `layout.json` with serde defaults; tab strip with + menu, Open file…, Open URL… and recent files; saved width, show and hide, 80-column rule; per-tab history; open-tab API; Ctrl+click on URLs; drop and quick open as sinks; existing kinds only | Tabs per session survive a restart and swap with the session; resize and keyboard divider work; forced colors; all open paths work |
| 6q-3 browser | Child webviews from the spike; capability moved to `webviews`; navigation allow-list; shared profile; address bar, Back, Forward, Reload; port chips; F12; hide-and-swap overlays; region to prompt on the screenshot; `marshell open` and the route; tab cap | Signed-in state survives a restart; the page cannot call the API; every route refuses what 2.7 lists; Windows live check on a real site and a localhost server |
| 6q-4 text editor | CodeMirror 6; the Shiki bridge; Sublime-style detection table; light and dark scheme with contrast check; find and replace, undo, line numbers; user-file write service; Ctrl+S; reload with merge route | Edit and save a file an agent also edits; clean merge and conflict both work; NVDA pass; forced colors |
| 6q-5 PDF read features | Find, outline, thumbnails; text layer without normalization; fix the dev MIME type | Find lands on the right text; a scanned JBIG2 or JPX PDF renders in dev and in the build |
| 6q-6 PDF annotations | Staged move to `PDFViewer` or `PDFPageView`; highlight, comment, free text; save through the write service; conflict, read-only and dirty handling; key guard | Saved files open with the marks in Acrobat, Edge and Chrome; Ctrl+A and Ctrl+Z reach the terminal while annotation mode is on |

6q-4 to 6q-6 are independent once 6q-2 is in; the order can change. If the spike fails, 6q-3 is replaced by "port chips and links open the system browser", and the rest proceeds.

## 5. New dependencies, against SCOPE.md

- `@codemirror/state`, `view`, `commands`, `search`, `language`, plus one language module for fallback if needed (6q-4). Need: an editor with find, undo, virtualized view and no eval (no such library is in `package.json`, which lists only shiki, pdfjs-dist and dompurify). Licence MIT. Upstream: releases continue (view 6.43.15 on 2026-10-19), though the GitHub repo is archived and development moved to code.haverbeke.berlin, run by one maintainer. No network or telemetry. Notices go in `THIRD_PARTY_NOTICES.md`. `@codemirror/merge` is not added in the first pass.
- No new Rust crate is planned. The browser uses `webview2-com` (already a direct dependency, `src-tauri/Cargo.toml:26`) for raw COM. The `unstable` feature on `tauri` is a Cargo feature change (6q-3) and, with the capability change from `windows` to `webviews`, needs the user's approval under SCOPE.md "Security surface" and "Dependencies". The user-file write service and the merge route reuse `diffy` and the existing write code.
- `pdfjs-dist` (Apache-2.0) and `shiki` (MIT) are already in. 6q-6 may add `pdf_viewer.mjs`, its CSS and about 330 KB of images, served from the app's own origin; add those to the `pdfjsAssets` Vite plugin and to the notices.
- Not added: Monaco, Ace, `codemirror-shiki` (0.x, one author; copy the approach instead), `tauri-runtime-cef`.

## 6. Risks and what could sink each slice

- 6q-0 and 6q-3. The child webview is on an `unstable` feature with no maintainer capacity. The likely killers: key forwarding unreliable (wry#451), focus not returning to the app after Alt+Tab (tauri#13466), resize breaking on the frameless window (tauri#12207, #9634), native file drop dying (tauri#10521), and a flash on every overlay that users find unacceptable. A Tauri minor update could regress it; Tauri 2.12 pins the unstable crates to minor versions.
- 6q-3. Screen-reader and keyboard access to the page are unproven: the page is a separate window with its own UI Automation tree, and the placeholder needs a label and a route in and out. SmartScreen and browser arguments must match across Marshell instances using one profile folder.
- 6q-1. Layout files in the field: if the fold rule differs between core and frontend, a split reopens on the wrong session.
- 6q-4. NVDA on CodeMirror's contenteditable may be poor; the fallback is Monaco, which needs three decisions first (blob workers, hex-only themes, forced-colors palette). Tokenizing speed inside WebView2 may force a lower cap than 1 to 2 MB.
- 6q-6. Annotation display in other viewers is unverified; non-Latin free text; editor key handling; sharing violations on save.
- Cross-platform. Linux child webviews do not work upstream today; macOS has keyboard bugs. Shiki's JS engine and pdf.js 6 target current engines; older WebKit may need the legacy pdf.js build and a catch that opens a file plain.

## 7. Unknown until a spike or a live check

- Whether key forwarding and focus return work on Windows with Tauri 2.12.2 (the go or no-go).
- Whether the frameless window and a child webview resize together without jitter at 125% and 150%.
- How long the hide-and-swap flash is, and what `CapturePreview` returns right after a navigation.
- Whether DevTools opens on the child with `AreDevToolsEnabled` set through `with_webview`.
- Whether `marshell` is on PATH in each CLI's sessions.
- Tokenizing time inside WebView2 on a typical PC.
- NVDA and Narrator behaviour for the editor and for the browser placeholder.
- How saved PDF annotations look in Acrobat, Edge and Chrome; how a JBIG2 or JPX PDF renders in dev.
- Whether a `PDFPageView` assembly can replace the full `PDFViewer`.
- Whether native drop still works with `unstable` on.
