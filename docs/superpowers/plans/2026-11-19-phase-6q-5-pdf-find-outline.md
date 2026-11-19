# Slice 6q-5: find, outline and thumbnails in a PDF tab — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A PDF tab gains a find bar (Ctrl+F), a sidebar with the outline and page thumbnails (F4), working links, and jumps that join the tab's Back and Forward. JBIG2 and JPEG 2000 images render in dev as they do in the build.

**Architecture:** The tab keeps its own renderer (`PdfView.tsx`: a canvas and a text layer per page, drawn near the view). pdf.js's `PDFFindController` (from `pdfjs-dist/web/pdf_viewer.mjs`, loaded on first use) searches the document. A small adapter of Marshell's own stands in for pdf.js's link service, and Marshell's own highlighter tints the matches on the drawn pages' text layers. A place in a PDF (a page and how far down it) becomes part of a history entry (`PaneEntry.page`, `PaneEntry.top`). Every jump goes through the tab's history: it records the place left, then pushes the place it goes to. The outline, the thumbnails and the links all end in that one jump.

**Tech Stack:** React 19, TypeScript, pdfjs-dist 6.4.299 (already a dependency: `build/pdf.mjs`, and `web/pdf_viewer.mjs` newly imported), Rust (`crates/protocol`, `crates/core/src/layout.rs`), Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 6q-5 decisions (2026-11-19)" 1–11, on top of "Slice 6q decisions" (first round 9, second round 1) and "Slice 2.5b decisions" 2–4. Research: `docs/research/2026-11-07-tabs-pane-options.md` §2.6. Rules: `.claude/rules/ui.md`, `protocol.md`.

## Global Constraints

- **Security surface.** Nothing changes in `src-tauri/`, the CSP, the capabilities, the local API's guard or token handling. There is no new package or crate: `pdf_viewer.mjs` ships in `pdfjs-dist`, which is already in `THIRD_PARTY_NOTICES.md`, and that file stays as it is. No code is copied from pdf.js. The highlighter follows the idea of pdf.js's `TextHighlighter` but is written fresh. If a task seems to need more, stop and report.
- **A page link never navigates the app's webview.** A link is a `span` with `role="link"`, never an `<a href>`. Only `http:` and `https:` addresses leave the PDF, through `followWebLink` (`src/features/preview/webLink.ts`), as the Markdown view's links do.
- **Numbers.** Sidebar width `200` px. The sidebar lies over the pages when the tab's body (`.preview__body`) is under `560` px wide. The toolbar's word "Page" hides under `360` px. Thumbnails are `160` px wide, drawn one at a time. A place's `top` is stored in ten-thousandths of the page's height (`0..=10_000`); in TypeScript a `Place` holds it as a fraction `0..1`. The outline keeps at most `10_000` entries, the rest dropped. A selection used to fill the find field is at most `200` characters on one line.
- **Copy** (sentence case, no "!", no "successfully"; exact strings; `{n}`, `{of}` and `{page}` are numbers, `{name}` the file's name):
  - Toolbar, in this order, all icon buttons but the field and the percentage: the sidebar button (`aria-label` "Outline and pages", title "Outline and pages (F4)", `aria-keyshortcuts` "F4", `aria-expanded`); the Page field as today; "Zoom out" (title "Zoom out (Ctrl+-)"), the percentage, "Zoom in" (title "Zoom in (Ctrl+=)") and "Fit width" (title "Fit width (Ctrl+0)", `aria-pressed`), each name now an `aria-label`; Find last (`aria-label` "Find", title "Find (Ctrl+F)", `aria-keyshortcuts` "Control+F", `aria-expanded`).
  - Find bar: field `aria-label` "Find in PDF", placeholder "Find"; buttons "Previous" (title "Previous match (Shift+Enter)") and "Next" (title "Next match (Enter)"); checkboxes "Match case" and "Whole word"; a close button with `aria-label` "Close find bar". The count reads "{n} of {of}", "No matches", "This PDF has no text to search. Its pages may be scanned images.", or nothing while the field is empty.
  - Announced: on a step, "Match {n} of {of}, page {page}." with " Continued from the start." or " Continued from the end." added after a wrap; on Enter with none, "No matches."; after a jump, and after Back or Forward within the same PDF, "Page {page} of {of}."
  - Sidebar: a tab list with `aria-label` "Sidebar views" and tabs "Outline" and "Pages". The tree's `aria-label` is "Outline of {name}" and the list's "Pages of {name}"; a thumbnail is "Page {n}". With no outline: "This PDF has no outline." and a "Show pages" button.
  - Links: `title` and `aria-label` "{url}" for a web link, "Link within the file" for a destination, and "Next page", "Previous page", "First page", "Last page", "Back" or "Forward" for an action. Announced for any other kind: "Only links within the file and web links open here." For a destination that isn't in the file: "This link leads nowhere in the file."
  - Palette commands, group "Window", no binding: `pane-pdf-find` "Find in PDF", `pane-pdf-outline` "Show PDF outline", `pane-pdf-pages` "Show PDF pages".
- **Keys in a PDF tab** (any element inside `.preview__body`): Ctrl+F opens the find bar; F3 and Shift+F3 step (and open the bar when there is no search); F4 shows or hides the sidebar. None is an app shortcut, and none goes in `lib/keymap.ts` bindings. Esc in the find field closes the bar, and Esc in the sidebar goes back to the pages. A key a PDF control handled has its default prevented, so the pane's Esc-to-terminal (`Preview.tsx` `keys`) skips it.
- **Styles.** Read the good-css skill before writing declarations; the frontend-design skill decides the details within the layout below and PLAN §9 decision 11. Tokens only, no hex outside `tokens.css`. `:hover` only inside `@media (hover: hover) and (pointer: fine)`. A forced-colors rule uses system colours only. No boxed outlines (`noBoxedOutlines.test.ts`). Icons are inline SVGs drawn like `Preview.tsx`'s `arrow` and `viewIcon` (12 or 14 px, `currentColor`, stroke 1.25, `aria-hidden`), on 24 × 20 px buttons in the `.pdf__button` look.
- **Paper tints.** A page is white paper in both themes, so matches don't use the theme's colours. Two new tokens in `tokens.css`, the same value in both themes: `--paper-match` (a highlighter yellow) and `--paper-match-current` (a deeper orange). They are drawn with `mix-blend-mode: multiply`, so black print stays dark; `contrast.test.ts` checks that black on each, laid over white, is at least 7:1. In forced colours a match is `color-mix(in srgb, Highlight 40%, transparent)` with `forced-color-adjust: none`, and the current one is also ringed with a 2 px `Highlight` outline, as `Editor.css` rings its current match.
- **Scroll.** The outline tree and the thumbnail list each scroll inside the sidebar with the themed scrollbar, as `.preview__body` does. Their selectors join `ALLOWED` in `src/styles/noScrollInMainWindow.test.ts`, with a line in its comment.
- **Motion.** None. The sidebar opens and closes at once, because the pages fit their new width.
- **Accessibility.** Everything works with the keyboard alone and in forced colours. `Announcer` (through the `announce` prop) is the only live region, and `selectedProps(…, "current")` (`src/components/a11y/SelectedMark.tsx`) marks the section in view and the page in view.
- **Who writes what** (decision 4). Tasks 1, 4 and 10 are written in the main thread. Tasks 2, 3 and 5 to 9 go to Sonnet workers, one at a time, with `effort: high` where the heading says so. The main thread reads each diff and runs its tests. A worker edits only its task's files and never runs `git checkout`, `git restore` or `git stash`.
- **Checks.** Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task. Rust (Task 2): `cargo test -p marshell-protocol -j 4`, `cargo test -p marshell-core -j 4 layout`, `pnpm check:generated`, `cargo fmt --all` before the commit, and `cargo clippy --workspace --all-targets -j 4 -- -D warnings`.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** highlights, notes, or anything that writes a PDF (6q-6); `PDFViewer` or `PDFPageView`; a Match diacritics switch; rebinding F3 or F4; a resizable sidebar; keeping the sidebar's state across a restart; links to other files.

## Layout

The toolbar and the find bar are one sticky head, full width, above both the sidebar and the pages. The sidebar sits under the head, as tall as the body below it.

```
Wide tab (≥ 560 px): the sidebar pushes the pages
┌ ◧  Page [12] of 140          −  110%  +  ⇔    ⌕ ┐  .pdf__bar (28 px)
│ [Find________] 3 of 17  Previous Next  ☐ Match case  ☐ Whole word   × │  find row, when open
├───────────────┬──────────────────────────────────┤
│ [Outline|Pages] │   ┌──────────────────────┐        │
│ ▾ 1 Intro    1 │   │                      │        │
│   1.1 Scope  3 │   │  white page          │        │
│ ▸ 2 Design  12▌│   │                      │        │  ▌ = SelectedMark: the section in view
│ ▸ 3 Results 40 │   └──────────────────────┘        │
└───────────────┴──────────────────────────────────┘
   200 px, raised surface

Narrow tab (< 560 px): the sidebar lies over the pages, with a shadow and no dimming
┌ ◧  [12] of 140   − 110% +  ⇔  ⌕ ┐   under 360 px "Page" hides
├───────────────┬──────────────────┤
│ [Outline|Pages] │ ┌──────────    │
│  ┌─────────┐  │ │ page          │
│  │  paper   │▌ │ │               │
│  └─────────┘  │ │               │
│      12       │ └──────────     │
└───────────────┴──────────────────┘
The find row wraps like the editor's: Match case and Whole word drop to a second row when one row doesn't fit.
```

- **An outline row:** a 16 px arrow (only on an entry with children; a click on it opens or closes the entry), then the title, which wraps and is never cut, then the page number at the row's end in `--text-2`, with tabular figures. Each level indents 12 px. A click anywhere but the arrow jumps.
- **A thumbnail:** a white paper plate in the page's shape, 160 px wide, with the canvas over it once drawn, and the page number centred under it. The page in view carries `selectedProps(…, "current")` on the button that holds both.
- **The Outline and Pages switch** is the filled track of `.preview__views` (Rendered and Source) with text labels, as a `tablist`.
- **The sidebar button's icon** is a rectangle with its left third filled, not three lines (three lines are the app's Menu). Its pressed look while open is `.pdf__button[aria-pressed="true"]`'s fill, keyed on `aria-expanded`.

## Review Focus

1. **A PDF that gives find nothing to work with:** a scanned PDF with no text layer, a query of spaces, a query found only across a line break or a hyphen, a ligature ("ﬁ") in the text, a PDF of 1,000 pages. Expected: the count says "This PDF has no text to search. Its pages may be scanned images." or "No matches" and the bar stays usable; a match across items is tinted on the right letters; the count grows while the pages are read, and typing is never blocked. Tests: Task 4 `statusOf_*`, `matchSpans_*`.
2. **An outline or a link that leads nowhere:** a named destination missing from the file, a page reference outside it, a `null` destination, an entry with no title or with control characters, an outline of 50,000 entries, a destination that is a bare page number. Expected: the entry still shows, and picking it says "This link leads nowhere in the file."; titles are trimmed and the outline stops at 10,000 entries. Tests: Task 6 `placeOfDest_*`, Task 7 `loadOutline_*`.
3. **A reload or a file switch under the new parts:** the file changes on disk while the find bar is open, while the sidebar is open, or right after a jump; Back from another file into a PDF that has since lost pages. Expected: find runs the same search on the new file, the outline and thumbnails are read again, the place is kept, and a place past the last page lands on the last page. Tests: Task 3 `clampPlace_*`, `a place waits for its own file`; Task 4 `a new document runs the last search again`.
4. **Keys that must keep their meaning:** Esc in the find field, in the sidebar, and on the pages; R typed in the find field; Alt+Left in the find field; Ctrl+F while the keyboard is in the terminal; F3 with no search. Expected: Esc closes the bar or leaves the sidebar, and only Esc on the pages goes to the terminal; R types; Alt+Left is Back; Ctrl+F in the terminal is the terminal's. Tests: Task 3 `escapeLeaves_*`, Task 5 `pdfKey_*`.
5. **A link with an address that must not open:** `javascript:`, `file:`, `mailto:`, a relative path, an `unsafeUrl` with no `url`, an address on the shell's refused names. Expected: only `http:` and `https:` go to `followWebLink`; every other kind says its line and nothing opens. Tests: Task 6 `linksOf_*`.

---

### Task 1: pdf.js groundwork (main thread)

**Files:**
- Modify: `vite.config.ts` (`pdfjsAssets`), `src/features/preview/pdf/pdfjs.ts`
- Test: `src/features/preview/pdf/pdfjs.test.ts`

**Interfaces:**
- Produces, in `pdfjs.ts`:
  - `export type TextMapping = { divs: HTMLElement[]; items: string[] }`: a drawn text layer's spans and their strings, in content order, as pdf.js's `TextLayer.textDivs` and `textContentItemsStr` give them.
  - `renderTextLayer(page, container, scale, signal?): Promise<TextMapping | null>` (null when cancelled). It streams the text with `{ includeMarkedContent: true, disableNormalization: true }`.
  - `export function viewerLib(): Promise<typeof import("pdfjs-dist/web/pdf_viewer.mjs")>`: sets `globalThis.pdfjsLib` to the loaded `pdfjs-dist` before it imports `pdf_viewer.mjs`, once.

- [ ] **Step 1: Write the failing tests** in `pdfjs.test.ts`, with `vi.mock("pdfjs-dist", …)` giving a fake `TextLayer` and `GlobalWorkerOptions`:
  - `renderTextLayer streams the text without normalization and with marked content`: the fake page's `streamTextContent` is called with `{ includeMarkedContent: true, disableNormalization: true }`.
  - `renderTextLayer hands back the layer's spans and strings`: resolves to `{ divs: layer.textDivs, items: layer.textContentItemsStr }`.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf/pdfjs.test.ts` and expect both to FAIL.
- [ ] **Step 3: Implement** both functions, and in `vite.config.ts` send `.js` and `.mjs` as `text/javascript`, `.wasm` as `application/wasm`, and anything else as `application/octet-stream`.
- [ ] **Step 4: Run** the test file (PASS), then `pnpm build`. The dev server's types are checked in Task 10 with `curl -sI http://localhost:1420/pdfjs/wasm/openjpeg_nowasm_fallback.js`, which must show `text/javascript`.
- [ ] **Step 5: Commit** `fix(pdf): dev serves pdf.js's decoders as JavaScript, and the text layer is built as pdf.js's viewer builds it.`

### Task 2: A place in the tab's history (worker)

**Files:**
- Modify: `crates/protocol/src/api.rs` (`PaneEntry`), `crates/core/src/layout.rs` (`validate_pane` and its tests), every Rust struct literal of `PaneEntry` (`rg "PaneEntry \{" crates src-tauri`), `src/features/pane/paneState.ts`
- Regenerated: `src/generated/PaneEntry.ts`
- Test: `crates/core/src/layout.rs` tests, `src/features/pane/paneState.test.ts`

**Interfaces:**
- Produces, in `PaneEntry` (after `col`), each with `#[serde(default, skip_serializing_if = "Option::is_none")]` and `#[ts(optional)]`, so that older builds and older files keep working:
  - `pub page: Option<u32>`: a PDF's page, from 1.
  - `pub top: Option<u32>`: how far down that page the view starts, in ten-thousandths of its height (`0..=10_000`).
- `validate_pane` refuses `page == Some(0)`, `top > 10_000`, a `top` without a `page`, and a `page` or `top` on a browser tab, with messages in the style of its others.
- Produces, in `paneState.ts`:
  - `export type Place = { page: number; top: number }`, where `top` is the fraction `0..1`.
  - `OpenTarget` gains `place?: Place`, and `entryOf` writes it as `page` and `Math.round(top * 10_000)`, clamped.
  - `export function placeOf(e: PaneEntry): Place | null`: null without a `page`.
  - Action `{ type: "place"; session: string; paneTab: string; place: Place }`: sets the current entry's `page` and `top` without pushing. It returns the same state when nothing changes, or when the tab is a browser tab.

- [ ] **Step 1: Write the failing Rust tests** in `layout.rs`: `a_file_entry_keeps_its_place` (round-trip through JSON, and the fields are absent from the JSON when `None`), `an_entry_from_an_older_file_has_no_place`, `a_place_out_of_bounds_is_refused` (page 0, top 10_001, top without page), `a_browser_tab_has_no_place`.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 layout` and expect them to FAIL; then add the fields and the checks; then `cargo test -p marshell-protocol -j 4`, `pnpm check:generated`, and the core tests again (PASS).
- [ ] **Step 3: Write the failing TS tests** in `paneState.test.ts`: `place records where the current entry is without pushing`; `place that changes nothing returns the same state`; `place leaves a browser tab alone`; `navigate with a place pushes an entry with page and top`; `back after a place and a navigate returns to the recorded place` (dispatch `place`, then `navigate`, then `back`; `placeOf(currentEntry)` equals the recorded place); `placeOf of an entry without a page is null`; `a place's top is clamped to 0..1 and its page to at least 1`.
- [ ] **Step 4: Run** `pnpm test src/features/pane/paneState.test.ts` (FAIL), implement, run it again (PASS), then `pnpm build`, `cargo fmt --all` and clippy.
- [ ] **Step 5: Commit** `feat(pane): a history entry keeps a place in a PDF, its page and how far down it.`

### Task 3: Places and jumps in the PDF view (worker, effort high)

**Files:**
- Create: `src/features/preview/pdf/place.ts`, `src/features/preview/pdf/place.test.ts`
- Modify: `src/features/preview/pdf/PdfView.tsx`, `src/features/preview/Preview.tsx`, `src/features/preview/Preview.test.tsx`, `src/features/sessions/LiveApp.tsx` (the `<Preview>` props, about line 2033)

**Interfaces:**
- Consumes: `Place`, `placeOf`, and the `place` action and `OpenTarget.place` (Task 2).
- Produces, in `place.ts`:
  - `export type PageBox = { page: number; top: number; height: number }` (client coordinates).
  - `export function placeAt(viewTop: number, pages: readonly PageBox[]): Place | null`: the page whose box spans `viewTop` (in a gap, the next page with `top: 0`; past the last page, the last page's end), with `top = (viewTop - box.top) / box.height`.
  - `export function scrollFor(place: Place, box: PageBox, viewTop: number): number`: the scroll delta that puts the place at `viewTop`.
  - `export function clampPlace(place: Place, pages: number): Place`: a page past the end becomes the last page, with `top: 0`.
  - `export function samePlace(a: Place, b: Place): boolean`: same page and `|a.top - b.top| < 0.005`.
  - `export function placeText(page: number, of: number): string`: "Page {page} of {of}."
- Produces, on `PdfView`'s props:
  - `place?: Place | null; placeKey?: unknown`: when `placeKey` changes, the view scrolls to `place` (clamped) once the document of the current `bytes` is laid out. A place that comes while another file loads waits for it; a reload of the same file keeps the scroll as now.
  - `onJump?: (to: Place, how: { keepFocus: boolean }) => void`: every jump the person asks for goes here. Without it, the view scrolls itself.
  - `placeNow?: Ref<() => Place | null>`: reads the place at the top of the view, under the sticky toolbar.
  - `announce?: (text: string) => void`, from `Body`'s own. A place applied for a `placeKey` change says "Page {page} of {of}.", except for the first place the view mounts with.
  - A change of scale keeps the reading place. When the zoom, or the width that fit width follows (the sidebar, the pane, the window), changes, the place measured before the change is applied again once the pages have their new sizes.
- Produces, in `Preview.tsx`:
  - `PreviewPaneProps.onPlace?: (place: Place) => void`.
  - `bodyTakesFocus(was, now, quietView: boolean, quietJump: boolean): boolean`: false for a quiet jump within the same file.
  - `export function escapeLeaves(e: { key: string; defaultPrevented?: boolean; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean }, inEditor: boolean): boolean`: whether Esc goes to the terminal. It doesn't when its default was prevented, or when the keyboard is in the editor.
  - Inside `PreviewPane`: `leave()` records `placeNow()` through `onPlace` right before Back, Forward (the header buttons and Alt+Left and Alt+Right) and every jump. `jump(to, how)` does nothing when `samePlace(to, placeNow())`, and otherwise calls `leave()` and then `onOpen({ path: entry.path, place: to })`.
- The Page field's Enter becomes `onJump({ page: n, top: 0 }, { keepFocus: false })`. `LiveApp` passes `onPlace` as a `place` dispatch for the tab.

- [ ] **Step 1: Write the failing tests:** `place.test.ts`: `placeAt_inside_a_page`, `placeAt_in_the_gap_takes_the_next_page`, `placeAt_above_the_first_page`, `placeAt_past_the_last_page`, `scrollFor_puts_the_place_at_the_view_top`, `clampPlace_past_the_end_is_the_last_page`, `samePlace_within_half_a_percent`, `placeText_reads_page_of`. `Preview.test.tsx`: `a quiet jump within the file keeps the keyboard where it is`, `Back to another page of the same file takes the keyboard`, `escapeLeaves_not_when_a_control_took_it`, `escapeLeaves_not_from_the_editor`, `escapeLeaves_from_the_pages`.
- [ ] **Step 2: Run** `pnpm test src/features/preview` and expect the new tests to FAIL.
- [ ] **Step 3: Implement** `place.ts`, the props, and the wiring in `Preview.tsx` (which also passes `announce` to `PdfView`) and `LiveApp.tsx`. The view's top is the scroller's top plus the sticky toolbar's height (`.pdf__bar`, which will also hold the find bar) plus the deleted-file notice when it shows, plus `SIDE`, as `jump()` measures today. `jump()` becomes `applyPlace()`.
- [ ] **Step 4: Run** `pnpm test src/features/preview src/features/pane` (PASS) and `pnpm build`.
- [ ] **Step 5: Commit** `feat(pdf): a jump in a PDF joins the tab's Back and Forward, and the place left is kept.`

### Task 4: The find engine (main thread)

**Files:**
- Create: `src/features/preview/pdf/highlight.ts`, `highlight.test.ts`, `findText.ts`, `findText.test.ts`
- Modify: `src/features/preview/pdf/PdfView.tsx` (each drawn page hands its `TextMapping` to find; the scroller gets `scroll-padding-block-start` of the sticky head plus 48 px, so a match that pdf.js scrolls to lands under it)

**Interfaces:**
- Consumes: `TextMapping`, `viewerLib()` (Task 1).
- Produces, in `highlight.ts`:
  - `export type Span = { begin: { div: number; offset: number }; end: { div: number; offset: number } }`.
  - `export function matchSpans(matches: readonly number[], lengths: readonly number[], items: readonly string[]): Span[]`: maps a page's match offsets, which count characters in `items.join("")`, onto the item and the offset within it.
  - `export function paint(m: TextMapping, spans: readonly Span[], selected: number): HTMLElement | null` and `export function unpaint(m: TextMapping): void`: wrap each matched run in `<span class="pdf__match">`, plus `pdf__match--current` for the selected one, and put the plain text back.
- Produces, in `findText.ts`:
  - `export type FindQuery = { text: string; caseSensitive: boolean; entireWord: boolean }`.
  - `export type FindStatus = { state: "idle" | "pending" | "found" | "notFound" | "noText"; current: number; total: number; page: number | null; wrapped: "start" | "end" | null }`.
  - `export function statusOf(state: number, matches: { current: number; total: number }, previous: boolean, page: number | null, textless: boolean): FindStatus`: pdf.js's `FindState` values (FOUND 0, NOT_FOUND 1, WRAPPED 2, PENDING 3) mapped to ours.
  - `export type FindView = { page(): number; pages(): number; showPage(n: number): void; isPageVisible(n: number): boolean }`.
  - `export type PdfFind = { setDocument(doc: PDFDocumentProxy | null): void; search(q: FindQuery): void; step(previous: boolean): void; close(): void; attach(pageIndex: number, m: TextMapping): () => void; onStatus(cb: (s: FindStatus) => void): () => void; destroy(): void }`.
  - `export async function createPdfFind(view: FindView, lib?: Pick<typeof import("pdfjs-dist/web/pdf_viewer.mjs"), "EventBus" | "PDFFindController">): Promise<PdfFind>`: `lib` defaults to `viewerLib()`, so tests can pass fakes. `highlightAll` is always on, and `matchDiacritics` always off.
  - `export function usePdfFind(doc: PDFDocumentProxy | null, view: FindView): { status: FindStatus; query: FindQuery; search(q: FindQuery): void; step(previous: boolean): void; close(): void; attach: PdfFind["attach"] }` for `PdfView` (Task 5 renders the bar on it).

- [ ] **Step 1: Write the failing tests:** `matchSpans_in_one_item`, `matchSpans_across_items`, `matchSpans_after_an_empty_item`, `matchSpans_at_the_last_item_end`, `matchSpans_with_no_matches_is_empty`; `statusOf_found`, `statusOf_wrapped_forward_is_from_the_start`, `statusOf_wrapped_back_is_from_the_end`, `statusOf_pending_keeps_the_count`, `statusOf_no_matches`, `statusOf_no_text`; with a fake `EventBus` and `PDFFindController`: `search dispatches find with highlightAll and no diacritics`, `step dispatches again with findPrevious`, `a new document runs the last search again`, `close dispatches findbarclose and clears every attached page`.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf` (FAIL), implement, run it again (PASS), and run `pnpm build`.
- [ ] **Step 3: Commit** `feat(pdf): find runs pdf.js's find controller over the tab's own pages and tints the matches.`

### Task 5: The toolbar and the find bar (worker)

**Files:**
- Create: `src/features/preview/pdf/FindBar.tsx`, `FindBar.test.tsx`, `PdfToolbar.tsx`, `PdfToolbar.test.tsx`
- Modify: `src/features/preview/pdf/PdfView.tsx` (the toolbar moves into `PdfToolbar`; the bar becomes a second row inside `.pdf__bar`; the keys), `PdfView.css`, `src/styles/tokens.css` (`--paper-match`, `--paper-match-current`), `src/design/contrast.test.ts`

**Interfaces:**
- Consumes: `usePdfFind`, `FindStatus`, `FindQuery` (Task 4); `escapeLeaves` (Task 3).
- Produces:
  - `export function FindBar(props: { query: FindQuery; status: FindStatus; onQuery(q: FindQuery): void; onStep(previous: boolean): void; onClose(): void; field: Ref<HTMLInputElement> })`.
  - `export function countText(s: FindStatus, query: string): string` and `export function stepText(s: FindStatus): string`, with the copy above.
  - `export function pdfKey(e: { key: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean }): "find" | "next" | "previous" | "sidebar" | null`: Ctrl+F, F3, Shift+F3 and F4, each with no other modifier.
  - `export function selectionText(sel: { text: string; inside: boolean }): string | null`: the selection when it is inside the PDF, on one line, and 1 to 200 characters long after trimming.
  - `export function PdfToolbar(props: { page: string; count: number; percent: number; fit: boolean; findOpen: boolean; sidebar: ReactNode; onPageDraft(v: string): void; onPageCommit(): void; onPageCancel(): void; onZoomOut(): void; onZoomIn(): void; onFit(): void; onFind(): void; findRow: ReactNode })`: the toolbar in the Copy order, with `sidebar` as the first slot (Task 7 fills it; empty until then) and `findRow` below the buttons. The word "Page" is a `span` that a container query hides under 360 px; the field's `aria-label` stays "Page, of {count}".
- Behaviour: Ctrl+F opens the bar, focuses the field and selects its text, filled from `selectionText` or else the last query. Typing searches as the person types. Enter, Shift+Enter, F3 and Shift+F3 step and announce `stepText`. Esc in the field, and the close button, close the bar, clear the tints, and put the keyboard back where it was when the bar opened (the pages, the sidebar or the toolbar; the pages when that element is gone). Esc prevents its default. The find row wraps as `.editor .cm-panel.cm-search` does (`flex-wrap`, the field growing), so the two checkboxes drop to a second line when one doesn't fit. `PdfView` listens for `pdfKey` where it listens for the zoom keys (`"sidebar"` is wired in Task 7). This task writes the `.pdf__match` and `.pdf__match--current` rules (Task 4 makes the spans) with the paper tokens and their forced-colours rule (Global Constraints).

- [ ] **Step 1: Write the failing tests:** `pdfKey_*` (each key, and Ctrl+Shift+F and Alt+F3 give null); `countText_*` and `stepText_*` (every copy line, wraps included); `selectionText_*` (outside the PDF, two lines, 201 characters, spaces only); the bar's markup has the field's label, both checkboxes, and the count in an element with no live region; the toolbar's markup has its buttons in the Copy order with their `aria-label`s, titles and `aria-keyshortcuts`, no visible text on the zoom buttons, and the "Page" word in its own element; `contrast.test.ts`: black over each paper tint, laid over white, is at least 7:1.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf src/design` (FAIL), implement, run it again (PASS), then `pnpm test src/features/preview src/design` and `pnpm build`.
- [ ] **Step 3: Commit** `feat(pdf): a toolbar of icons that fits a narrow pane, and a find bar with Ctrl+F, F3 and Shift+F3.`

### Task 6: Links on the pages (worker)

**Files:**
- Create: `src/features/preview/pdf/dest.ts`, `dest.test.ts`, `links.ts`, `links.test.ts`
- Modify: `src/features/preview/pdf/PdfView.tsx` (a drawn page reads `page.getAnnotations({ intent: "display" })` and renders `PageLinks`), `PdfView.css`, `src/features/preview/Preview.tsx` (`onWebLink` goes to `follow({ kind: "web", url })`; `onHistory` goes to Back or Forward through `leave()`)

**Interfaces:**
- Consumes: `Place`, `clampPlace`, `onJump` (Task 3).
- Produces, in `dest.ts`:
  - `export type Dest = string | readonly unknown[]`.
  - `export function topOf(explicit: readonly unknown[], toViewY: (y: number) => number, height: number): number`: `XYZ` takes `args[1]` and `FitH` and `FitBH` take `args[0]`, each converted with `toViewY`, as a fraction of `height` clamped to `0..1`. Any other kind, or a `null` height, gives 0.
  - `export async function placeOfDest(doc: PDFDocumentProxy, dest: Dest): Promise<Place | null>`: a string goes through `getDestination`; the first item is a page reference (`getPageIndex`) or a zero-based page number; null when anything is missing or out of range.
- Produces, in `links.ts`:
  - `export type LinkGoal = { kind: "dest"; dest: Dest } | { kind: "web"; url: string } | { kind: "action"; action: "NextPage" | "PrevPage" | "FirstPage" | "LastPage" | "GoBack" | "GoForward" } | { kind: "other" }`.
  - `export type PageLink = { box: { left: number; top: number; width: number; height: number }; goal: LinkGoal; label: string }`: the box as fractions of the page.
  - `export function goalOf(data: { url?: unknown; dest?: unknown; action?: unknown }): LinkGoal`: `url` with `http:` or `https:` is web, `dest` is dest, a named action in the list is an action, and anything else is other.
  - `export function linksOf(annotations: readonly unknown[], view: { width: number; height: number; toView(rect: readonly number[]): number[] }): PageLink[]`: only `subtype === "Link"`, with a normalised rectangle.
  - `export function PageLinks(props: { links: readonly PageLink[]; onFollow(goal: LinkGoal): void })`: each link is a `span` with `role="link"`, `tabIndex={0}`, the label, and `inset` and `size` in percentages; a web link also has the label as its `title`. A click or Enter follows it. It shows `cursor: pointer`, a faint accent tint under the pointer (`color-mix(in oklch, var(--accent) 14%, transparent)`, multiplied like the matches), and the app's 2 px focus ring for the keyboard; in forced colours the ring is `Highlight`.
- Produces, on `PdfView`: `onWebLink?: (url: string) => void`, `onHistory?: (by: -1 | 1) => void`, and an internal `follow(goal: LinkGoal, how: { keepFocus: boolean })` that Task 7's outline uses too. A destination jumps to `placeOfDest`, or announces "This link leads nowhere in the file." when that is null. Next, Previous, First and Last page jump to that page's top. GoBack and GoForward call `onHistory`. Other announces its line.

- [ ] **Step 1: Write the failing tests:** `topOf_*` (XYZ, FitH, FitBH, Fit, a null top); `placeOfDest_*` with a fake doc (a named destination, a missing name, a page reference, a bare page number, a page out of range, a `null`); `goalOf_*` and `linksOf_*` (http, https, `javascript:`, `file:`, `mailto:`, an `unsafeUrl` with no `url`, a relative path, each named action, an unknown action, a non-link annotation, a flipped rectangle); `PageLinks` markup (`role="link"`, the labels, a `title` on a web link only, no `href`).
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf` (FAIL), implement, run it again (PASS), then `pnpm build`.
- [ ] **Step 3: Commit** `feat(pdf): a PDF's links work: in-file links jump and web links open a browser tab.`

### Task 7: The sidebar and the outline (worker, effort high)

**Files:**
- Create: `src/features/preview/pdf/outline.ts`, `outline.test.ts`, `sidebarPref.ts`, `sidebarPref.test.ts`, `PdfSidebar.tsx`, `PdfSidebar.test.tsx`
- Modify: `src/features/preview/pdf/PdfView.tsx` (the sidebar button in `PdfToolbar`'s `sidebar` slot, F4, the layout, the lying-over mode, fit width less the sidebar), `PdfView.css`, `src/styles/noScrollInMainWindow.test.ts` (`.pdf-sidebar__tree` joins `ALLOWED`)

**Interfaces:**
- Consumes: `LinkGoal`, `goalOf`, `placeOfDest`, `follow` (Task 6); `pdfKey` (Task 5).
- Produces, in `outline.ts`:
  - `export type OutlineNode = { id: string; title: string; goal: LinkGoal | null; page: number | null; open: boolean; children: OutlineNode[] }`: `id` is the index path ("0", "0.2"); `open` is a positive `count`.
  - `export async function loadOutline(doc: PDFDocumentProxy): Promise<OutlineNode[]>`: titles trimmed, control characters dropped, an empty one read as "Untitled", at most 10,000 nodes, each page resolved with `placeOfDest`.
  - `export function sectionAt(nodes: readonly OutlineNode[], page: number): string | null`: the last node in document order whose page is at most `page`.
  - `export type Row = { id: string; level: number; title: string; parent: string | null; hasChildren: boolean; expanded: boolean; posinset: number; setsize: number }`.
  - `export function visibleRows(nodes: readonly OutlineNode[], expanded: ReadonlySet<string>): Row[]`, `export function ancestorsOf(id: string): string[]`.
  - `export function treeKey(rows: readonly Row[], at: string, key: string): { focus: string } | { expand: string } | { collapse: string } | { activate: string } | null`, for Up, Down, Right, Left, Home, End and Enter.
- Produces, in `sidebarPref.ts` (decision 10; module state, in memory only, shared by every PDF tab):
  - `export type SidebarPref = { open: boolean; view: "outline" | "pages" | null }`, starting `{ open: false, view: null }`; `export function sidebarPref(): SidebarPref` and `export function setSidebarPref(p: Partial<SidebarPref>): void`.
  - `export function startOpen(pref: SidebarPref, overlay: boolean): boolean`: open only when `pref.open` and the sidebar wouldn't lie over the pages.
  - `export function startView(pref: SidebarPref, hasOutline: boolean): "outline" | "pages"`: `pref.view`, except that Outline in a PDF with none, and no view yet in one with none, give Pages; no view yet in a PDF with an outline gives Outline. Showing Pages for want of an outline doesn't change the pref.
  - The pref records only what the person chose: the button, F4 and the palette's commands set `open`, and the Outline and Pages tabs (and the palette's commands) set `view`. A close after a pick, by Esc or by a click outside a sidebar that lies over the pages records nothing.
- Produces `PdfSidebar(props: { name: string; view: "outline" | "pages"; onView(v: "outline" | "pages"): void; outline: OutlineNode[] | null; section: string | null; onPick(node: OutlineNode): void; onLeave(): void; pages: ReactNode })`: a `tablist` with Outline and Pages, drawn as the filled track of `.preview__views` with text labels; Outline is a `role="tree"` with roving `tabIndex`, `aria-level`, `aria-expanded`, `aria-setsize` and `aria-posinset`, and the section marked with `selectedProps(…, "current")`; `pages` is Task 8's list. A row is as in the Layout section: the arrow (its own click target, `aria-hidden`, toggling the entry), the wrapping title, and the page number at the end in `--text-2` with `font-variant-numeric: tabular-nums`, hidden from the tree item's name by `aria-hidden` (the title is the name). Hover is `--hover`, the keyboard's place the app's focus ring, the section the SelectedMark; the three can show at once. The empty outline's line has a "Show pages" button that calls `onView("pages")`.
- The sidebar button: the panel-left icon, `aria-expanded`, `aria-controls` the sidebar, the pressed fill while open, and `disabled` until the PDF has loaded.
- Behaviour, in `PdfView`: F4 and the button show or hide the sidebar. Showing it puts the keyboard on the section's entry (Outline) or the page's thumbnail (Pages); hiding it puts the keyboard on the pages. When the section changes, its ancestors join the expanded set and nothing is collapsed. A pick calls `follow(node.goal)`, with `keepFocus: true` while the sidebar sits beside the pages; when it lies over them, it closes after the pick. Esc in the sidebar calls `onLeave`, which also closes a sidebar that lies over the pages. A tab opens with `startOpen` and `startView`.
- The lying-over mode: the sidebar is `--bg-raised` with `--shadow-menu`, over the pages with nothing dimmed. It closes, with the keyboard left where it went, on a `pointerdown` outside it and its button, and on a `focusout` whose `relatedTarget` is outside both. Beside the pages, it is `--bg-raised` with a `--hairline` edge on its end side.
- Both modes: no motion. The tree scrolls inside the sidebar (`.pdf-sidebar__tree`, `overflow-y: auto`, the themed scrollbar), under the tab list, which stays put.

- [ ] **Step 1: Write the failing tests:** `loadOutline_*` with a fake doc (nesting, titles, the 10,000 cap, an entry whose destination is missing, a `url` entry); `sectionAt_*` (before the first entry, between entries, nested); `visibleRows_*`; `treeKey_*` (each key, at both ends, Right on a leaf, Left at the top level); `startOpen_*` (open and beside, open and over, closed) and `startView_*` (each pref with and without an outline); `PdfSidebar` markup (the tab list, the tree's roles and levels, a page number marked `aria-hidden`, the empty outline's line and its "Show pages" button, the section marked); `noScrollInMainWindow` passes with the new entry.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf src/styles` (FAIL), implement, run it again (PASS), then `pnpm build`.
- [ ] **Step 3: Commit** `feat(pdf): an outline in a sidebar that follows the scroll, with F4.`

### Task 8: Page thumbnails (worker)

**Files:**
- Create: `src/features/preview/pdf/thumbs.ts`, `thumbs.test.ts`, `Thumbnails.tsx`, `Thumbnails.test.tsx`
- Modify: `src/features/preview/pdf/PdfView.tsx` (passes `<Thumbnails>` as `pages` to `PdfSidebar`), `PdfView.css`, `src/styles/noScrollInMainWindow.test.ts` (`.pdf-thumbs` joins `ALLOWED`)

**Interfaces:**
- Consumes: `renderPage`, `canvasScale` (`pdfjs.ts`); `PdfSidebar`'s `pages` slot (Task 7); `onJump` (Task 3).
- Produces:
  - `export function nextToDraw(wanted: ReadonlySet<number>, drawn: ReadonlySet<number>, current: number): number | null`: the wanted page that isn't drawn and is nearest to `current`.
  - `export function listKey(at: number, count: number, key: string, perView: number): number | null`, for Up, Down, Home, End, Page Up and Page Down.
  - `export function Thumbnails(props: { doc: PDFDocumentProxy; sizes: readonly { w: number; h: number }[]; current: number; name: string; onPick(page: number): void; focusOnShow: boolean })`: a `role="list"` of buttons named "Page {n}", with roving `tabIndex` and the page in view marked with `selectedProps(…, "current")`. The list (`.pdf-thumbs`) scrolls inside the sidebar with the themed scrollbar. Each button holds a paper plate 160 px wide and as tall as its page's ratio makes it, white as `.pdf__page` is (`background: Canvas; color-scheme: light`) before and after its canvas is drawn, and the page number centred under it in `--text-2` with tabular figures. Hover is `--hover` on the button, the keyboard's place the app's focus ring. Pages near the list's view (one view out) are wanted, and their canvases are dropped three views out. One page is drawn at a time, at low priority. While the keyboard isn't in the list, the page in view's thumbnail is scrolled into the list's view (`block: "nearest"`). `PdfView` turns a pick into `onJump({ page, top: 0 }, …)`, with the keyboard and the closing as for an outline pick (Task 7).

- [ ] **Step 1: Write the failing tests:** `nextToDraw_*` (nearest first, ties going to the later page, nothing left), `listKey_*` (each key at both ends), and the `Thumbnails` markup (the list's label, "Page 3", the visible number, the current mark); `noScrollInMainWindow` passes with the new entry.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf src/styles` (FAIL), implement, run it again (PASS), then `pnpm build`.
- [ ] **Step 3: Commit** `feat(pdf): page thumbnails in the sidebar, drawn one at a time.`

### Task 9: The palette's PDF commands (worker)

**Files:**
- Modify: `src/lib/keymap.ts`, `src/lib/appShortcuts.ts`, `src/features/palette/runnable.ts`, `src/features/sessions/LiveApp.tsx`, `src/features/preview/Preview.tsx` (`PreviewHandle.pdf()`), `src/features/preview/pdf/PdfView.tsx` (a `commands` ref)
- Test: `src/lib/keymap.test.ts`, `src/lib/appShortcuts.test.ts`, `src/features/palette/runnable.test.ts`

**Interfaces:**
- Produces: `export type PdfCommands = { find(): void; outline(): void; pages(): void }` in `PdfView.tsx`; `PreviewHandle.pdf(): PdfCommands | null` (null until a PDF has loaded); `RunFacts.pdfTab: boolean`; app shortcut kinds `paneFind`, `paneOutline`, `panePages`, run like `paneGoToLine`, including the `setTimeout` list near `LiveApp.tsx:1741`.
- `outline()` and `pages()` show the sidebar on that view, record both in the sidebar pref (Task 7), and put the keyboard in it. `find()` opens the bar as Ctrl+F does.

- [ ] **Step 1: Write the failing tests:** the three commands exist with their labels, in group "Window", with no binding; `appShortcuts` maps each id to its kind; `cannotRun` leaves the three out without `pdfTab` and keeps them with it.
- [ ] **Step 2: Run** `pnpm test src/lib src/features/palette` (FAIL), implement, run it again (PASS), then `pnpm test` and `pnpm build`.
- [ ] **Step 3: Commit** `feat(palette): Find in PDF, Show PDF outline and Show PDF pages.`

### Task 10: The live check and the docs (main thread)

**Files:**
- Create: `docs/research/2026-11-19-pdf-find-outline-live-check.md`
- Modify: `docs/PLAN.md` (§9, "Slice 6q-5 decisions": what the check changed, if anything)

- [ ] **Step 1: Make the fixtures** in the check's scratch folder, outside the repo:
  - A 40-page PDF with a three-level outline, links within the file, an `https` link to a local test site, a `mailto:` link, a NextPage action, a ligature, "résumé" and a word split across two lines.
  - A scanned PDF with a JPEG 2000 image (and a JBIG2 one if a test file can be found).
  - A text-free PDF.
- [ ] **Step 2: Run the live check** with the `marshell-live-check` skill. Each line is a check with its result in the note:
  1. `curl -sI` on `/pdfjs/wasm/openjpeg_nowasm_fallback.js` from the dev server shows `text/javascript`, and the JPEG 2000 PDF renders in dev with no console error.
  2. Ctrl+F and "résumé" with Match case off: the tint sits on the right letters (screenshot), and "resume" finds it too. The count reads "1 of 2" or similar, and F3 goes round with "Continued from the start." announced.
  3. Find in the text-free PDF: "This PDF has no text to search. Its pages may be scanned images."
  4. F4 opens the sidebar on Outline with the keyboard on the section in view. Down, Right and Enter jump, and "Page {n} of {of}." is announced; Alt+Left comes back to the place left; Alt+Right goes forward again.
  5. The sidebar beside the pages at 800 px and over them at 480 px (closing after a pick, and on a click on the pages), and the Pages view: thumbnails draw on white plates and the page in view is marked. A second PDF tab opens with the sidebar as the first left it; at 480 px it opens closed.
  6. Links: the in-file link jumps and Back returns; the `https` link shows its address on hover and opens a browser tab; the `mailto:` link says its line.
  7. Esc: in the find field it closes the bar and the keyboard goes back where it was (try from the pages and from the sidebar); in the sidebar it goes to the pages; on the pages it goes to the terminal. Ctrl+Shift+Tab goes back to the terminal from each.
  8. The pane at 320 px: the toolbar is one row with "Page" hidden, and the find bar wraps to two rows with nothing cut (screenshot). Zoom in and out, and opening the sidebar with fit width on, keep the line at the top of the view.
  9. Forced colours (emulated) and dark theme: matches, the current match, the section mark, the pressed sidebar button and link focus are visible, and the print under a match stays dark (screenshots).
  10. A file change on disk with the bar and the sidebar open: the search runs again and the place holds.
  11. Restart the app: the PDF tab opens at the place last recorded, with the sidebar closed.
  12. `pnpm tauri build --no-bundle`, then the built exe with the same scratch homes: the JPEG 2000 PDF renders.
- [ ] **Step 3: Fix what the check finds,** each with a failing test first, and note each in the research file.
- [ ] **Step 4: Run what CI runs:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -j 4 -- -D warnings`, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`.
- [ ] **Step 5: Commit** `docs(6q-5): the live check of PDF find, outline and thumbnails.`
