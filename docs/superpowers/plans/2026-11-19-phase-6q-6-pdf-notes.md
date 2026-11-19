# Slice 6q-6: notes in a PDF tab — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A PDF tab can highlight text in five colours, put a comment on a highlight, and place free text. Notes from other apps can be edited too, a Notes view lists every note, and Ctrl+S saves the notes into the file with a backup and Undo. Unsaved notes are kept as a draft across a quit, and a change on disk never takes them along.

**Architecture:** pdf.js 6.4.299's own editor (`AnnotationEditorUIManager`, `AnnotationEditorLayer` and `DrawLayer` from `pdfjs-dist`) is fitted onto the tab's existing page renderer. One UI manager per loaded document, and one editor layer over each drawn page's canvas and text layer, made and dropped as `PdfPage` draws and drops the page. Saving is `PDFDocumentProxy.saveDocument()`, which appends an incremental update to the loaded bytes; the bytes go through the text tab's save route (`PUT /v1/files/content`, which keeps a backup snapshot and offers Undo). A pure reducer (`notesDoc.ts`, the PDF counterpart of `editorDoc.ts`) owns the save state: dirty, saving, the last save, a conflict with the disk, and the draft. The draft is the whole saved-bytes output, kept by the core under `<home>/drafts/pdf/`. Marshell writes its own toolbar tools, selection bar, comment popup and Notes view, and its own CSS for pdf.js's editor classes; nothing is copied from pdf.js.

**Tech Stack:** React 19, TypeScript, pdfjs-dist 6.4.299 (`build/pdf.mjs`'s editor classes and `web/pdf_viewer.mjs`'s `EventBus` and `GenericL10n`, both already shipped), Rust (`crates/core` save and draft routes), Vitest (node, `renderToStaticMarkup`).

**Spec:** `docs/PLAN.md` §9, "Slice 6q-6 decisions (2026-11-19)" 1–9, on top of "Slice 6q decisions" (first round 9, second round 7 and 9), "Slice 6q-4 decisions" 8 (an undone save keeps the edits) and "Slice 6q-5 decisions". Research: `docs/research/2026-11-07-tabs-pane-options.md` §2.6 (the saving flow, the key hijack, free text in non-Latin scripts). Rules: `.claude/rules/ui.md`, `local-api.md`, `protocol.md`.

## Global Constraints

- **Task 1 is a gate** (decision 5). If its checks can't pass on the tab's own pages, stop: report to the user, who gets a new plan that moves the tab to `PDFViewer`. No later task starts before the gate passes.
- **Security surface.** Nothing changes in `src-tauri/`, the CSP, the capabilities, `guard.rs` or token handling. No new package or crate; `THIRD_PARTY_NOTICES.md` stays as it is. No code or CSS is copied from pdf.js: Marshell's CSS for pdf.js's editor classes is written fresh, and pdf.js's `web/images/` are not served. Every new route sits in the token-guarded router with a "refused without the bearer" test, and only `server/save.rs` routes write a user's file.
- **Keys never leak out of the tab.** pdf.js's UI manager listens on `window` and, while its mode isn't NONE, takes Ctrl+A, Ctrl+Z, Ctrl+Y, Backspace, Delete, Enter, Space, Esc and the arrows from any target that isn't a text `<input>`, the terminal's textarea included. Its mode is NONE whenever the keyboard is outside the PDF tab's root (`.pdf`), and while the window has no focus; the person's chosen tool is kept in Marshell's state and given back to pdf.js when the keyboard comes back.
- **Numbers.** Colours, in this order, named and valued as pdf.js's defaults: Yellow `#FFFF98`, Green `#53FFBC`, Blue `#80EBFF`, Pink `#FFCBE6`, Red `#FF4F5F` (written into the PDF, so they are data in TypeScript, not tokens). Free text uses pdf.js's defaults: 10 pt, black. A PDF draft is put `1500` ms after the last change (a text draft waits `DRAFT_MS` = 400). A save or a draft of a PDF may be up to `SAVE_PDF_CAP` = `PDF_CAP` + 8 MiB = 58 MiB; other saves stay at `TEXT_CAP`. PDF drafts take at most `PDF_DRAFTS_BYTES` = 1 GiB in all, and count toward `MAX_DRAFTS` (500). A copy is named `{stem} (notes).pdf`, then `{stem} (notes 2).pdf` up to `(notes 99)`.
- **Copy** (sentence case, no "!", no "successfully"; `{name}` is the file's name, `{page}` a number, `{colour}` a colour's name):
  - Toolbar, in this order: the sidebar button (its label becomes "Outline, pages and notes", title "Outline, pages and notes (F4)"); the Page field; the zoom group; "Highlight" (`aria-pressed`, title "Highlight: drag across text"); "Free text" (`aria-pressed`, title "Free text: click where it goes, or press Enter"); Find. Under `360` px the zoom percentage hides as the word "Page" does (it stays in the zoom buttons' `aria-describedby`).
  - Tools off, their title instead: "Notes can't be added: {reason}" with the reason "This PDF is signed, and a note would break its signature.", "This PDF's owner doesn't allow notes.", "This PDF is an XFA form.", "{name} is read-only." or, only if Task 1 finds pdf.js can't write them, "This PDF is encrypted.".
  - Selection bar (`role="toolbar"`, `aria-label` "Selected text"): "Highlight", a colour menu button (`aria-label` "Highlight colour: {colour}", `aria-haspopup="menu"`, items `menuitemradio` "Yellow" … "Red"), "Comment".
  - Find bar, after Next: "Highlight" (title "Highlight this match") and "Comment" (title "Highlight this match and comment on it"), disabled with no current match or with the tools off.
  - Comment popup (`role="dialog"`, `aria-label` "Comment"): a textarea (`aria-label` "Comment on the note on page {page}"), "Done" (title "Done (Ctrl+Enter)"), "Cancel" (Esc), and "Delete comment" when the note had one.
  - Notes view: the tab "Notes"; the list's `aria-label` "Notes in {name}"; a row reads "{Kind}, page {page}: {text}" then its comment, with Kind "Highlight, {colour}", "Free text", or the kind's name and ", read-only" for other kinds. Empty: "This PDF has no notes yet." and "Select text to highlight it, or use the Free text tool.". A free text row whose text has a character outside WinAnsi adds "May show blank in Edge and Chrome.".
  - Save line, under the toolbar like the find row: "Saved {name}." with an "Undo" button; after Undo, "{name} is back as it was before the save." (the notes stay, unsaved); "{name} changed after the save, so the save can't be undone."; a refusal says the core's line (e.g. "The file is read-only or in use by another program.") with a "Save as a copy" button.
  - Changed bar: "{name} changed on disk. Your notes aren't saved." with "Save as a copy", "Reload" and "Overwrite". Deleted: "{name} was deleted. Your notes aren't saved." with "Save as a copy" and "Save". After a copy: "Saved the notes in {copy}.".
  - Announced: "Highlighted on page {page}."; "Free text added on page {page}."; "Note deleted."; for free text outside WinAnsi, once when it is committed, "Edge and Chrome may show this note blank: it has letters outside the Latin alphabet.".
  - Palette, group "Window", no binding: `pane-pdf-notes` "Show PDF notes". `pane-save` and `pane-undo-save` also list for a loaded PDF tab whose tools aren't off.
  - Core: a full PDF-draft budget answers 503 "Marshell keeps at most 1 GB of unsaved PDF notes. Save some PDFs first.".
- **Keys in a PDF tab.** Ctrl+S saves (the existing `isSaveKey`). Esc on the pages first ends a note being typed or unselects a selected note; only with neither does it go to the terminal (it is then not default-prevented). F4 shows and hides the sidebar on its last view, as before. In the comment popup, Ctrl+Enter is Done and Esc is Cancel. With a tool on and the keyboard on the pages: Enter or Space places free text at the middle of the page in view; with a note focused: the arrows move it (Ctrl+arrows by 10), Delete removes it, Ctrl+Z and Ctrl+Y undo and redo note edits. None of these is an app shortcut.
- **Styles.** Read the good-css skill before writing declarations; the frontend-design skill decides the details. Tokens only. `:hover` only inside `@media (hover: hover) and (pointer: fine)`. Forced colours use system colours only: a highlight keeps its PDF colour on the page (it is paper, `forced-color-adjust: none` as the canvas already is), and a selected note gets a 2 px `Highlight` outline. No boxed outlines. Icons are inline SVGs as in 6q-5 (14 px, `currentColor`, stroke 1.25). The selection bar and the comment popup have the raised surface and a shadow, like the sidebar lying over the pages. Nothing new animates.
- **Accessibility.** Everything works with the keyboard alone and in forced colours. `Announcer` (the `announce` prop) is the only live region. A note on the page is a Tab stop after the page's links. The Notes view is a list with Up, Down, Home, End, Page Up and Page Down, as the thumbnails are.
- **Who writes what** (decision 8). Tasks 1, 4 and 8 in the main thread. Tasks 2, 3, 5, 6 and 7 go to Sonnet `worker`s one at a time (`effort: high` where the heading says so), each briefed with its task, its files and its test command; the main thread reads the diff and runs the tests. A worker edits only its task's files, keeps scratch files in its own scratch folder, and never runs `git checkout`, `git restore` or `git stash`.
- **Checks.** Frontend: `pnpm test <file>`, and `pnpm build` at the end of every frontend task. Rust (Task 2): `cargo test -p marshell-core -j 4 <filter>`, `cargo fmt --all` before the commit, `cargo clippy --workspace --all-targets -j 4 -- -D warnings`, and `pnpm check:generated` if `crates/protocol` changed.
- **Commits** read `type(scope): what changed, as a sentence`.
- **Not in this slice:** ink, stamps, signatures, drawn (free-hand) highlights, underline or strike-out, free text font or colour settings, a custom colour, carrying notes onto a changed file, a Save As dialog, notes in an image or a browser tab, macOS key variants beyond `isSaveKey`'s.

## Layout

```
Toolbar (the find row and the save line are rows of the same sticky head)
┌ ◧  Page [3] of 40   −  110%  +  ⇔   🖍  T   ⌕ ┐
│ Saved report.pdf.  [Undo]                       │  save line, when there is one
├────────────────┬───────────────────────────────┤
│ [Outline|Pages|Notes] │   ...the quick brown fox...  │
│ Page 3  ▮ Highlight   │       ┌───────────────────────┐ │
│   "quick brown"       │       │ Highlight ▾ ● Comment │ │  selection bar, under the selection's end
│   Check this date     │       └───────────────────────┘ │
│ Page 7  T Free text   │                               │
│   Sign here          ▌│   ┌ Comment ───────────────┐  │
│                       │   │ [textarea            ] │  │  comment popup, by its note
└────────────────┴──────│   │ Delete comment  Cancel Done│ │
                        └───────────────────────────────┘

Changed bar (in place of the save line, takes no focus, announced)
│ report.pdf changed on disk. Your notes aren't saved.  [Save as a copy] [Reload] [Overwrite] │
```

- **The selection bar** sits under the end of the selection's last line, inside the page's box, and above the selection when it would pass the page's bottom. It shows after a pointer selection inside one page's text layer ends, never while the Highlight tool is on (the tool highlights at once), and goes when the selection collapses, the page is dropped, or Esc is pressed in it.
- **The comment popup** sits beside its note, inside the page's box, 280 px wide at most. It takes the keyboard when it opens and gives it back to the note when it closes.
- **A Notes row** has the colour as a 10 px swatch (a "T" for free text, the kind's icon otherwise), the page at its end in `--text-2` with tabular figures, the quoted text on up to two lines, and the comment under it in `--text-2`. The note in view carries `selectedProps(…, "current")`.

## Review Focus

1. **Keys outside the tab while notes are live:** with the Highlight or Free text tool on and a note selected, the keyboard goes to the terminal (click, Ctrl+Shift+Tab, Esc) and Ctrl+Z, Ctrl+A, Backspace, Delete, Enter, Space and Esc are pressed there; also after Alt+Tab away and back. Expected: each reaches the terminal, and the tool is on again when the keyboard comes back to the tab. Tests: Task 1 `editorKeysAllowed_*`; Task 4 `the tool is handed to pdf.js only while the keyboard is in the tab`.
2. **A save that meets the disk:** the file changed between load and Ctrl+S, it is locked by another program or read-only, it was deleted, Ctrl+S pressed twice, a 45 MB PDF. Expected: a conflict shows the changed bar and writes nothing; a refusal shows its line and Save as a copy; a second Ctrl+S waits for the first; a 45 MB file saves. Tests: Task 2 `a_pdf_save_may_pass_the_text_cap`, `a_text_save_still_stops_at_the_text_cap`; Task 3 `notesDoc_*`.
3. **A draft across a quit or an unload:** unsaved notes, then quit and start, with the file unchanged, changed, or deleted meanwhile; the budget full; the tab unloaded by a session switch right after a note. Expected: unchanged shows the notes and the dot; changed or deleted shows the bar with the draft's notes held back; a full budget says its line and the notes stay in the tab; an unload flushes the draft before the document goes. Tests: Task 2 `pdf_drafts_*`; Task 3 `notesDoc_draft_*`; Task 4 `an unload flushes the draft before the document is destroyed`.
4. **A PDF that can't take notes, or takes them oddly:** encrypted without the annotation permission, signed, XFA, read-only on disk, scanned with no text, a page rotated 90°, pages of mixed sizes, an Acrobat highlight with a comment. Expected: tools off with the reason; a scanned page takes free text but has nothing to select; a note on a rotated page is placed and saved where it was put; the Acrobat note is listed and editable. Tests: Task 5 `notesBlocked_*`; the rest in Task 8's live check.
5. **Notes over the tab's own life:** a page scrolled three screens away and back, a zoom, the sidebar opening beside the pages, a tab switch and back, the poll seeing the tab's own save. Expected: every note is still there once, in place, and the own save causes no reload and no bar. Tests: Task 1 `a page drawn again gets its notes back`; Task 3 `notesDoc_own_save_is_not_a_disk_change`.

---

### Task 1: pdf.js's editor on the tab's pages — the gate (main thread)

**Files:**
- Create: `src/features/preview/pdf/notes/editor.ts`, `src/features/preview/pdf/notes/editor.test.ts`, `src/features/preview/pdf/notes/notes.css`
- Modify: `src/features/preview/pdf/pdfjs.ts`, `src/features/preview/pdf/PdfView.tsx` (`PdfPage`), `src/styles/noBoxedOutlines.test.ts` only if its list must name the new file

**Interfaces:**
- Produces, in `notes/editor.ts`:
  - `export type NoteTool = "none" | "highlight" | "freetext"`.
  - `export const HIGHLIGHT_COLORS: readonly { name: "Yellow" | "Green" | "Blue" | "Pink" | "Red"; value: string }[]` (the five, in order).
  - `export type CommentRequest = { page: number; anchor: DOMRect; text: string; done(text: string | null): void }`: pdf.js asked for a comment on a note; `done(null)` cancels and `done("")` deletes the comment.
  - `export type NotesEditor = { attachPage(n: number, page: PDFPageProxy, layers: { holder: HTMLElement; text: HTMLElement }, scale: number): () => void; setTool(t: NoteTool): void; setLive(live: boolean): void; setColor(c: string): void; highlightSelection(comment: boolean): boolean; escape(): boolean; modified(): boolean; onChange(cb: () => void): () => void; save(): Promise<Uint8Array>; markSaved(): void; destroy(): void }`. `attachPage` returns the detach; `setLive(false)` puts pdf.js in NONE whatever the tool; `highlightSelection` is false when the selection isn't inside one attached text layer; `escape` is true when it ended an edit or unselected a note.
  - `export async function createNotesEditor(doc: PDFDocumentProxy, o: { container: HTMLElement; onComment(req: CommentRequest): void }): Promise<NotesEditor>`.
  - `export function editorKeysAllowed(active: Element | null, root: Element | null, windowFocused: boolean): boolean`: true only when the window has focus and `active` is inside `root`.
- Produces, in `pdfjs.ts`: `editorLib()`, loading `AnnotationEditorUIManager`, `AnnotationEditorLayer`, `DrawLayer` and `AnnotationEditorType` from the same `pdfjs-dist` copy as `lib()`, and `EventBus` and `GenericL10n` from `viewerLib()`.

- [ ] **Step 1: Write the failing tests** in `editor.test.ts` (pure parts only; the rest is the gate's live check):
  - `editorKeysAllowed_inside_the_tab`, `editorKeysAllowed_not_from_the_terminal` (an element outside `root`), `editorKeysAllowed_not_while_the_window_is_away`.
  - `HIGHLIGHT_COLORS_are_pdfjs_defaults_in_order`.
  - `a page drawn again gets its notes back`: with a fake UI manager (only the calls `attachPage` makes), attach page 2, detach it, attach it again: the second layer is registered for page index 1 and the manager still holds the page's editors (no `removeEditor` on detach).
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf/notes` and expect FAIL.
- [ ] **Step 3: Build the fit.** The UI manager gets the tab's root as its container, a duck-typed viewer (`{ classList }` of the page list), the `EventBus`, the doc, the five colours (`enableHighlightFloatingButton` false: the selection bar is Marshell's), and a `commentManager` adapter that turns `showDialog` into `onComment`. `attachPage` makes a `DrawLayer` and an `AnnotationEditorLayer` over the page's canvas and text layer, with the page's viewport at `scale`, and `GenericL10n` (its built-in en-US). Notes pdf.js loads from the file (`getAnnotationsByType`) must show once, and the canvas must not draw them a second time. `PdfPage` calls `attachPage` after it swaps the canvas in, and the detach before it drops or redraws.
- [ ] **Step 4: Find out, and write into the plan's ledger:** which pdf.js mode lets notes be selected, moved, recoloured, commented and deleted with no tool on, while a click creates nothing (record the mode; `setTool("none")` uses it while live); how to keep drawn highlights from starting on a pointer outside text in Highlight mode; whether `saveDocument()` on an encrypted PDF with the annotation permission writes notes Edge can read.
- [ ] **Step 5: Gate check in the real app** (`marshell-live-check` skill, a scratch home, a 3-page text PDF and an encrypted one), each pass or fail recorded in the ledger:
  1. A mouse highlight draws, survives scrolling three screens away and back, and a zoom.
  2. With Free text on, Enter on the pages places a box at the middle of the page in view; typing, Esc, then Tab reaches it.
  3. `save()`'s bytes start with the loaded bytes; written to a scratch path and opened in Marshell, the notes show once and are editable; opened in Edge, both notes show.
  4. With a tool on and a note selected, clicking into the terminal, then Ctrl+Z, Ctrl+A, Backspace, Delete, Enter, Space and Esc reach the terminal (`setLive(false)` on `focusout` and window `blur`, wired here minimally).
  5. Find's tints, links and region select still work on a page with notes.

  If 1–4 can't all pass, stop and report (decision 5).
- [ ] **Step 6: Run** the tests (PASS) and `pnpm build`.
- [ ] **Step 7: Commit** `feat(pdf): pdf.js's note editor runs on the PDF tab's own pages.`

### Task 2: PDF saves and PDF drafts in the core (worker)

**Files:**
- Modify: `crates/core/src/server/mod.rs` (the `/v1/files/content` PUT layer and the new routes), `crates/core/src/server/save.rs`, `crates/core/src/server/files.rs` (`SAVE_PDF_CAP`), `crates/core/src/server/drafts.rs`, `crates/core/src/userfiles/drafts.rs`
- Test: the tests in those files, and the router's bearer tests

**Interfaces:**
- `pub const SAVE_PDF_CAP: u64 = PDF_CAP + 8 * 1024 * 1024` in `files.rs`. The PUT's `DefaultBodyLimit` becomes `SAVE_PDF_CAP`; `save.rs` answers 413 for a body over `TEXT_CAP` unless the path's extension is `pdf` (any case).
- `Drafts` gains `put_pdf(&self, path: &str, base_hash: Option<&str>, edited_ms: u64, bytes: &[u8]) -> Result<(), PutError>`, `get_pdf(&self, path: &str) -> Option<(PdfDraftMeta, Vec<u8>)>`, and `PutError::PdfBudget`. They live in `<home>/drafts/pdf/<first 32 hex of sha256(key)>.json` (the meta: `path`, `base_hash`, `edited_ms`, through `store::write`) and `.pdf` (the bytes, written to a temp file and renamed). `list()` includes them; `remove()` drops both kinds; both kinds count toward `limit`. A put that would take the PDF drafts past `PDF_DRAFTS_BYTES` (1 GiB, the replaced draft's bytes not counted) is `PdfBudget`.
- Routes, token-guarded: `GET /v1/drafts/pdf?path=` → 200 `application/pdf` with headers `X-Base-Hash` (absent for none) and `X-Edited-Ms`, or 204; `PUT /v1/drafts/pdf?path=&base=<64 hex|none>&edited=<ms>` with an `application/octet-stream` body up to `SAVE_PDF_CAP` → 204, 400 with "That path can't be saved." for a refused or relative path or a path not ending in `.pdf`, 413 over the cap, 503 with the budget line or the existing "at most 500 drafts" line. `DELETE /v1/drafts/one` drops a PDF draft too.

- [ ] **Step 1: Write the failing tests:** `a_pdf_save_may_pass_the_text_cap` (3 MiB to `x.pdf`, saved), `a_text_save_still_stops_at_the_text_cap` (3 MiB to `x.txt`, 413), `pdf_drafts_round_trip` (bytes, base hash and edited time back), `pdf_drafts_are_listed_with_text_drafts`, `pdf_drafts_removed_by_the_one_delete`, `pdf_drafts_refuse_a_path_not_ending_in_pdf`, `pdf_drafts_keep_to_the_budget` (with a test budget), `pdf_drafts_replacing_one_does_not_count_it_twice`, `pdf_drafts_count_toward_the_draft_limit`, and `refused_without_the_bearer` for each new route.
- [ ] **Step 2: Run** `cargo test -p marshell-core -j 4 drafts` and `cargo test -p marshell-core -j 4 save` and expect the new tests to FAIL.
- [ ] **Step 3: Implement.** Blocking file work in `spawn_blocking`; no lock held across I/O beyond `DRAFTS_LOCK` as today.
- [ ] **Step 4: Run** both filters (PASS), `cargo fmt --all`, clippy.
- [ ] **Step 5: Commit** `feat(core): a PDF save may be as large as a PDF, and a PDF's unsaved notes are kept as a draft.`

### Task 3: The notes document and its client calls (worker, effort high)

**Files:**
- Create: `src/features/preview/pdf/notes/notesDoc.ts`, `src/features/preview/pdf/notes/notesDoc.test.ts`
- Modify: `src/features/editor/editorApi.ts` (`getPdfDraft`, `putPdfDraft`; `dropDraft` is reused), `src/features/preview/usePreviewFile.ts` (the PDF phase keeps `hash` and `readOnly`), `src/features/preview/usePreviewFile.test.ts`

**Interfaces:**
- Consumes: Task 2's routes; `saveFile`, `unsaveFile`, `FileSaved`.
- Produces, in `editorApi.ts`: `getPdfDraft(path): Promise<{ bytes: ArrayBuffer; baseHash: string | null; editedMs: number } | null>` and `putPdfDraft(path, bytes: Uint8Array, baseHash: string | null, editedMs: number): Promise<"kept" | { refused: string }>`.
- Produces, in `notesDoc.ts`, a pure reducer like `editorDoc.ts`:
  - `export type NotesState = { base: { hash: string | null; bytes: ArrayBuffer }; dirty: boolean; saving: boolean; lastSave: { snapshot: string; hash: string } | null; line: string | null; refusedSave: boolean; conflict: { hash: string | null; bytes: ArrayBuffer | null } | null; fromDraft: boolean }`.
  - `export type NotesEvent = { type: "edited" } | { type: "save" } | { type: "serialized"; bytes: Uint8Array; for: "save" | "draft" | "copy" | "overwrite" } | { type: "saved"; saved: FileSaved; bytes: Uint8Array } | { type: "saveConflict"; hash: string | null } | { type: "saveRefused"; line: string } | { type: "unsave" } | { type: "unsaved"; hash: string } | { type: "unsaveRefused" } | { type: "disk"; hash: string | null; bytes: ArrayBuffer | null } | { type: "draft"; bytes: ArrayBuffer; baseHash: string | null } | { type: "copy" } | { type: "copied"; name: string } | { type: "reload" } | { type: "overwrite" } | { type: "draftRefused"; line: string }`.
  - `export type NotesEffect = { type: "serialize"; then: "save" | "draft" | "copy" | "overwrite" } | { type: "save"; bytes: Uint8Array; expect: string | null } | { type: "saveCopy"; bytes: Uint8Array } | { type: "unsave"; snapshot: string } | { type: "putDraft" } | { type: "dropDraft" } | { type: "load"; bytes: ArrayBuffer; dirty: boolean } | { type: "markSaved" } | { type: "announce"; text: string }`.
  - `export function notesReducer(s: NotesState, e: NotesEvent, name: string): { state: NotesState; effects: NotesEffect[] }`.
  - `export function copyName(stem: string, taken: (name: string) => boolean): string | null`: `{stem} (notes).pdf`, then `(notes 2)` up to `(notes 99)`, else null.
  - `export function winAnsiSafe(text: string): boolean`: every character is in Windows-1252.
- Rules the tests pin: a disk event whose hash is the base's, or the last save's, changes nothing (the poll seeing the tab's own save); a disk change while dirty is a conflict and loads nothing, while clean it loads; Reload drops the draft and loads the disk's bytes, clean; Overwrite saves with the conflict's hash as `expect`; a save while saving is ignored; Undo's `unsaved` keeps the notes and makes the tab dirty again (6q-4 decision 8); a draft whose base hash isn't the disk's is a conflict at once; a deleted file (`hash: null`) while dirty is a conflict with the deleted copy.

- [ ] **Step 1: Write the failing tests** `notesDoc_*`, one per rule above, plus `notesDoc_own_save_is_not_a_disk_change`, `notesDoc_draft_unchanged_file_shows_notes_dirty`, `notesDoc_draft_changed_file_is_a_conflict`, `notesDoc_draft_deleted_file_is_a_conflict`, `notesDoc_draft_refused_keeps_the_notes_and_says_why`, `notesDoc_saved_offers_undo_with_the_line`, `notesDoc_refused_save_offers_a_copy`, `copyName_*` (free, taken once, all taken), `winAnsiSafe_*` (Latin with accents and "€" true; Hebrew, Cyrillic, CJK false), and in `usePreviewFile.test.ts` `a pdf keeps its hash and read-only flag`.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf/notes src/features/preview/usePreviewFile.test.ts` and expect FAIL.
- [ ] **Step 3: Implement**, with the copy lines from Global Constraints.
- [ ] **Step 4: Run** them (PASS) and `pnpm build`.
- [ ] **Step 5: Commit** `feat(pdf): the save state of a PDF's notes, with drafts, Undo and the disk's changes.`

### Task 4: Saving notes from the tab (main thread)

**Files:**
- Create: `src/features/preview/pdf/notes/useNotes.ts`, `src/features/preview/pdf/notes/NotesBars.tsx` (the save line and the changed bar), their tests
- Modify: `src/features/preview/pdf/PdfView.tsx`, `src/features/preview/Preview.tsx` (`keys`, the handle's commands), `src/features/sessions/LiveApp.tsx` (`textTabs` takes a PDF tab's commands; `answerSaveAsk`), `src/features/pane/paneCommands.ts` (`closeAsk` asks for a PDF tab too), `src/features/palette/runnable.ts`, their tests

**Interfaces:**
- Consumes: Task 1's `NotesEditor`; Task 3's reducer and client calls.
- Produces: `export type SaveCommands = Pick<TextTabCommands, "save" | "undoSave" | "canUndoSave" | "discard" | "dirty" | "focus">` in `TextTab.tsx`; `TextTabCommands` extends it. The preview's handle keeps `text()` and gains `saving(): SaveCommands | null` (the text tab's or the PDF tab's). `LiveApp`'s close question (`textTabs`, `loadedText`, `answerSaveAsk`) and the palette's Save and Undo save use `saving()`; the text-only commands keep `text()`. `PdfView` gains `path`, `hash`, `readOnly` and `onDirty(path, dirty)` props.
- Produces, in `useNotes.ts`: `export function onFocusMove(editor: Pick<NotesEditor, "setLive">, root: Element | null, active: Element | null, windowFocused: boolean): void`, which calls `setLive(editorKeysAllowed(active, root, windowFocused))`; `PdfView` calls it on the root's `focusin` and `focusout` (read a tick later, as `keyboardLeft` is) and on the window's `focus` and `blur`.
- `useNotes(doc, editor, { path, name, hash, bytes, announce, onDirty })` runs the reducer's effects: `serialize` calls `editor.save()`; `load` hands bytes back to `PdfView` to load; a draft is put `1500` ms after the last `onChange`, and at once when the view unmounts, the document being destroyed only after that put's bytes are taken.
- [ ] **Step 1: Write the failing tests:** `closeAsk_asks_for_an_unsaved_pdf`; `the palette lists save for a pdf tab with notes`; `NotesBars` markup for the save line, the changed bar and the deleted bar (copy exact, buttons in order, no focus taken); `the tool is handed to pdf.js only while the keyboard is in the tab` (`onFocusMove` with a fake editor: inside and focused is live; outside, or the window away, is not); `an unload flushes the draft before the document is destroyed` (with fakes); `escape on the pages goes to the editor first` (`escapeLeaves` false after `editor.escape()` returned true).
- [ ] **Step 2: Run** `pnpm test src/features/preview src/features/pane src/features/palette` (FAIL).
- [ ] **Step 3: Implement.** Ctrl+S in `Preview.tsx`'s `keys` calls the active tab's `SaveCommands`. A PDF tab opens on its draft when `getPdfDraft` has one. The dot comes through `onDirty` as for a text tab. "Save as a copy" writes beside the file (`saveFile` with `expect` null on `copyName`'s name) and opens the copy in a new tab of the session.
- [ ] **Step 4: Run** the tests (PASS) and `pnpm build`.
- [ ] **Step 5: Commit** `feat(pdf): Ctrl+S saves a PDF's notes into the file, with Undo, a draft and the changed bar.`

### Task 5: The tools, the colours, the selection bar and find's buttons (worker)

**Files:**
- Create: `src/features/preview/pdf/notes/SelectionBar.tsx`, `src/features/preview/pdf/notes/ColorMenu.tsx`, `src/features/preview/pdf/notes/blocked.ts`, their tests
- Modify: `src/features/preview/pdf/PdfToolbar.tsx`, `src/features/preview/pdf/FindBar.tsx`, `src/features/preview/pdf/findText.ts` (the current match's range), `src/features/preview/pdf/PdfView.tsx`, `src/features/preview/pdf/PdfView.css`, `notes/notes.css`, their tests

**Interfaces:**
- Consumes: `NotesEditor` (`setTool`, `setColor`, `highlightSelection`).
- Produces: `export async function notesBlocked(doc: PDFDocumentProxy, readOnly: boolean, name: string): Promise<string | null>` (the reason line, or null), reading `getPermissions()` (allowed when null or holding `PermissionFlag.MODIFY_ANNOTATIONS`), `getFieldObjects()` (a `signature` field with a value), and `isPureXfa`; `PdfFind.currentRange(): Range | null`, the current match's letters on its drawn text layer; the last colour picked kept in a module variable (`notesPref.ts` style, as `sidebarPref.ts`), so it lasts while Marshell runs.
- [ ] **Step 1: Write the failing tests:** `notesBlocked_*` (permissions without the flag, with it, unencrypted, signed with a value, a blank signature field, XFA, read-only, each with its exact line); `PdfToolbar` markup with the two tools in place, pressed states, and the blocked title; `SelectionBar` markup (toolbar name, buttons, the colour button's name); `ColorMenu` keys (Up, Down, Home, End, Enter, Esc) on a pure `colorMenuKey`; `FindBar` shows Highlight and Comment after Next, disabled with no match; `currentRange_*` on a fake mapping (one span, across two spans).
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf` (FAIL).
- [ ] **Step 3: Implement**, with the layout and copy above. Find's buttons select `currentRange()` and call `highlightSelection`, then announce "Highlighted on page {page}." (Comment opens the popup instead).
- [ ] **Step 4: Run** the tests (PASS) and `pnpm build`.
- [ ] **Step 5: Commit** `feat(pdf): Highlight and Free text tools, five colours, a bar over selected text, and highlight from find.`

### Task 6: The comment popup (worker, effort high)

**Files:**
- Create: `src/features/preview/pdf/notes/CommentPopup.tsx`, its test
- Modify: `src/features/preview/pdf/PdfView.tsx`, `notes/notes.css`, `notes/editor.ts` (only the `onComment` wiring, if Task 1 left a gap)

**Interfaces:**
- Consumes: `CommentRequest` (Task 1).
- Produces: `<CommentPopup request={…} onClose={…} />`, positioned from `request.anchor` within the page box. Done (or Ctrl+Enter) calls `done(text)`; Cancel or Esc calls `done(null)`; Delete comment calls `done("")`. Focus goes to the textarea on open and back to the note's element on close; a pointer down outside closes it as Cancel. A free text note committed with text outside WinAnsi announces the warning line once (`winAnsiSafe`).
- [ ] **Step 1: Write the failing tests:** markup (dialog name, textarea name with the page, buttons, Delete comment only with an existing comment); a pure `popupKey` (Ctrl+Enter is done, Esc is cancel, Enter types); a pure `popupPlace(anchor, pageBox, size)` (beside the note, flipped inside the page box at its edges).
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf/notes` (FAIL).
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** (PASS) and `pnpm build`.
- [ ] **Step 5: Commit** `feat(pdf): a note's comment is written in a popup beside it.`

### Task 7: The Notes view and its palette command (worker)

**Files:**
- Create: `src/features/preview/pdf/notes/NotesList.tsx`, `src/features/preview/pdf/notes/noteRows.ts`, their tests
- Modify: `src/features/preview/pdf/sidebarPref.ts` (`SidebarView` gains `"notes"`), `src/features/preview/pdf/PdfSidebar.tsx`, `src/features/preview/pdf/PdfView.tsx` (`PdfCommands.notes()`), `notes/editor.ts` (`notes()` and `focusNote(id)`), `src/features/palette/` (the command), `src/features/sessions/LiveApp.tsx`, their tests

**Interfaces:**
- Produces, in `editor.ts`: `export type Note = { id: string; page: number; kind: "highlight" | "freetext" | "other"; kindName: string; color: string | null; text: string; comment: string; editable: boolean; top: number }` (top as a fraction of the page) and `NotesEditor.notes(): Promise<Note[]>` (the file's markup annotations with the editor's adds, edits and deletions applied; pages read once each, at most 2,000 notes), `NotesEditor.focusNote(id): boolean`.
- `noteRows(notes, colourName)` sorts by page then top and builds each row's text and `aria-label` from the copy above.
- A pick jumps through `onJump` to `{ page, top }` and then focuses the note on the page (an editable one); the sidebar's keyboard and closing rules are the outline's (6q-5 decision 8).
- [ ] **Step 1: Write the failing tests:** `noteRows_*` (order, kinds, colour names, an empty comment, a read-only kind, the WinAnsi line); `NotesList` markup (list name, empty state, the current note's mark); `PdfSidebar` shows three tabs; `the palette lists Show PDF notes for a loaded PDF tab`.
- [ ] **Step 2: Run** `pnpm test src/features/preview/pdf src/features/palette` (FAIL).
- [ ] **Step 3: Implement.** The list refreshes on the editor's `onChange`, at most every 500 ms.
- [ ] **Step 4: Run** (PASS) and `pnpm build`.
- [ ] **Step 5: Commit** `feat(pdf): a Notes view lists every note in a PDF, and the palette shows it.`

### Task 8: The live check and the docs (main thread)

**Files:**
- Create: `docs/research/2026-11-19-pdf-notes-live-check.md`
- Modify: `docs/PLAN.md` (6q-6 decisions: anything execution changed), the plan's ledger

- [ ] **Step 1:** Run the live check (`marshell-live-check`, scratch homes, fixtures in a fresh scratch folder: a text PDF, an encrypted one without the annotation permission, a rotated-page one, one with an Acrobat-style highlight and comment). Check: each way to make a note (mouse selection bar, the Highlight tool, find's buttons, Free text by click and by Enter); recolour, comment, move, delete; the Notes view by keyboard; Ctrl+S, Undo, and the dot; the saved file in Edge and, if installed, Chrome and Acrobat (free text in Hebrew too); a quit and start with unsaved notes (file unchanged, then changed); the changed bar's three choices; the close question; Review Focus 1's keys in the terminal; forced colours; a 320 px pane.
- [ ] **Step 2:** Run what CI runs: `cargo fmt --all -- --check`, clippy, `cargo test --workspace -j 4`, `pnpm test`, `pnpm check:generated`, `pnpm build`.
- [ ] **Step 3: Commit** `docs(6q-6): the live check of PDF notes.`
