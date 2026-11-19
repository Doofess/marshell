import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
import { editorLib } from "../pdfjs";
import { pageText, quadBoxes, readFileNotes, textUnder, type TextItem } from "./fileNotes";
import { winAnsiSafe } from "./notesDoc";

/**
 * pdf.js's note editor, fitted onto the PDF tab's own pages: one UI manager per loaded document, and over each drawn page a
 * draw layer (the highlights), a hidden annotation layer (the file's own notes the editor can change, handed to it) and an
 * editor layer (the notes as things to select, move and type in). The editor is never in pdf.js's NONE mode: switching in
 * and out of it redraws every page and rebuilds the file's notes. "No tool" is pdf.js's Highlight mode with its instant
 * highlight held back, and the keys, the clipboard and drops pdf.js listens for on the window and the document reach it
 * only while the keyboard is on the tab's pages (`pdfjsTakesKey`).
 */

export type NoteTool = "none" | "highlight" | "freetext";
export type ColorName = "Yellow" | "Green" | "Blue" | "Pink" | "Red";

/** pdf.js's own highlight colours, in its order: they are written into the PDF, so they are data, not tokens. */
export const HIGHLIGHT_COLORS: readonly { name: ColorName; value: string }[] = [
  { name: "Yellow", value: "#FFFF98" },
  { name: "Green", value: "#53FFBC" },
  { name: "Blue", value: "#80EBFF" },
  { name: "Pink", value: "#FFCBE6" },
  { name: "Red", value: "#FF4F5F" },
];

/** pdf.js asked for a note's comment: `done(null)` cancels, `done("")` deletes the comment. */
export type CommentRequest = { page: number; anchor: DOMRect; note: HTMLElement; text: string; done(text: string | null): void };

/** A note in the Notes view. `top` is how far down its page it starts, as a fraction. `added` is set on a note the editor made, not read from the file. */
export type Note = { id: string; page: number; kind: "highlight" | "freetext" | "other"; kindName: string; color: string | null; text: string; comment: string; editable: boolean; top: number; added?: boolean };

/** A note pdf.js holds, as the list sees it: `fileId` is the file annotation it stands for, `ready` when it holds something and isn't being typed. */
export type HeldNote = { id: string; fileId: string | null; page: number; kind: "highlight" | "freetext"; color: string | null; text: string; comment: string; top: number | null; ready: boolean };

/**
 * The file's notes with the editor's changes applied: a held note that stands for a file note replaces it (its colour, comment
 * and text), a file note that was deleted goes, and a new note that holds something and isn't being typed is added.
 */
export function mergeNotes(file: readonly Note[], held: readonly HeldNote[], deleted: (fileId: string) => boolean): Note[] {
  const edits = new Map<string, HeldNote>();
  const added: HeldNote[] = [];
  for (const h of held) {
    if (h.fileId !== null && file.some((f) => f.id === h.fileId)) edits.set(h.fileId, h);
    else if (h.ready) added.push(h);
  }
  const out: Note[] = [];
  for (const f of file) {
    if (deleted(f.id)) continue;
    const h = edits.get(f.id);
    out.push(h ? { ...f, color: h.color ?? f.color, comment: h.comment, text: h.text !== "" ? h.text : f.text, top: h.top ?? f.top } : f);
  }
  for (const h of added) {
    out.push({ id: h.id, page: h.page, kind: h.kind, kindName: h.kind === "highlight" ? "Highlight" : "Free text", color: h.color, text: h.text, comment: h.comment, editable: true, top: h.top ?? 0, added: true });
  }
  return out;
}

export type NotesEditor = {
  /** Puts the editor over a drawn page; the function returned takes it off again (the page's notes stay with the editor). */
  attachPage(n: number, page: PDFPageProxy, layers: { holder: HTMLElement; text: HTMLElement }, scale: number): () => void;
  setTool(t: NoteTool): void;
  /** Whether pdf.js may have the keys, the clipboard and drops: only while the keyboard is in the tab and the window has focus. */
  setLive(live: boolean): void;
  /** The page in view: where Enter places free text, and the scale pdf.js measures notes with. */
  setPage(n: number, scale: number): void;
  /** The colour the next highlight gets. */
  setColor(c: string): void;
  /** Highlights the text selected on one attached page, then asks for its comment when `comment`; false when there is none. */
  highlightSelection(comment: boolean): boolean;
  /** Ends a note being typed, or unselects the selected notes; false when there was neither. */
  escape(): boolean;
  /** A note is being typed: its text is in pdf.js's box and not yet in the note, so a save now would end the typing. */
  editing(): boolean;
  modified(): boolean;
  /** Every note in the PDF, the file's with the editor's adds, edits and deletions applied. */
  notes(): Promise<Note[]>;
  /** Puts the keyboard on an editable note's element; false when the note is gone or its page isn't drawn. */
  focusNote(id: string): boolean;
  onChange(cb: () => void): () => void;
  /** The loaded bytes with the notes appended as an incremental update. */
  save(): Promise<Uint8Array>;
  markSaved(): void;
  destroy(): void;
};

/** A note as the announcements see it: `ready` once it holds something and isn't being typed, `text` a free text's. */
export type LiveNote = { id: string; kind: string; page: number; fromFile: boolean; ready: boolean; text: string | null };
/** What the announcements remember between changes: the notes already told of, the free text already warned about, and how many notes went since. */
export type NewsState = { known: Set<string>; warned: Set<string>; gone: number };

const BLANK_WARNING = "Edge and Chrome may show this note blank: it has letters outside the Latin alphabet.";

/**
 * The lines to announce after a change: a highlight or a free text that is new since the last change, a free text whose
 * letters Edge and Chrome can't show (once per note), and "Note deleted." when notes went. The file's own notes are never
 * news; `s` is updated.
 */
export function noteNews(s: NewsState, notes: Iterable<LiveNote>): string[] {
  const lines: string[] = [];
  for (const n of notes) {
    if (n.fromFile || !n.ready) continue;
    if (!s.known.has(n.id)) {
      s.known.add(n.id);
      if (n.kind === "highlight") lines.push(`Highlighted on page ${n.page}.`);
      else if (n.kind === "freetext") lines.push(`Free text added on page ${n.page}.`);
    }
    if (n.kind === "freetext" && n.text !== null && !s.warned.has(n.id) && !winAnsiSafe(n.text)) {
      s.warned.add(n.id);
      lines.push(BLANK_WARNING);
    }
  }
  if (s.gone > 0) lines.push("Note deleted.");
  s.gone = 0;
  return lines;
}

/** pdf.js may take the keys only while the window has focus and the keyboard is inside the tab. */
export function editorKeysAllowed(active: Element | null, root: Element | null, windowFocused: boolean): boolean {
  return windowFocused && !!active && !!root && root.contains(active);
}

const EDITABLE = "input, textarea, select, [contenteditable]";
const FOLLOWS = "a, button, [role=button], [role=link]";
/** Marshell's own controls drawn over the pages: their keys are theirs, never pdf.js's undo. */
const OWN_CONTROLS = ".pdf-comment, .pdf-selbar";

/** What pdf.js asks of a page's layer when it looks for the page a selection is on. */
type TextLayerOwner = { hasTextLayer(text: HTMLElement): boolean };
const NO_LAYER: TextLayerOwner = { hasTextLayer: () => false };

/**
 * The layer pdf.js should treat as the page in view: that page's, else any drawn page's (the page in view may still be
 * drawing), else, with no page drawn at all (a hidden tab), one that owns no text, so pdf.js's look-up finds nothing
 * instead of throwing.
 */
export function layerInView<L extends TextLayerOwner>(current: L | undefined | null, drawn: ReadonlyMap<number, L>): L | TextLayerOwner {
  return current ?? drawn.values().next().value ?? NO_LAYER;
}

/**
 * The text layer a selection lies in, when it is one of this tab's and the selection doesn't cross pages: only then is it
 * this tab's to highlight. Every PDF tab's editor hears the document's selection changes, and pdf.js empties a selection
 * before it looks for the layer, so a hidden tab must never reach that far.
 */
/**
 * pdf.js puts a highlight on its undo stack each time a page's layer takes it (`onceAdded`), and a page drawn again (a zoom,
 * a scroll back, a tab shown again) gets new layers: a note already held that comes back is no step and no change. The add
 * and its step run in one go, so the mark lasts until the task ends.
 */
export function readdGuard() {
  let again = false;
  return {
    added(back: boolean) {
      again = back;
      if (back)
        queueMicrotask(() => {
          again = false;
        });
    },
    /** Whether the step pdf.js adds now is a note coming back to its page, not a person's change. */
    takesCommand(): boolean {
      const r = again;
      again = false;
      return r;
    },
  };
}

/**
 * A highlight's text: the page's text under its quads. pdf.js sets the text as the note's label, but its own l10n then names
 * the note "Highlight editor". A file note no one changed serializes to nothing, and the list keeps the file's text.
 */
export async function highlightText(e: { pageIndex: number; serialize(): { quadPoints?: ArrayLike<number> } | null }, items: (page: number) => Promise<readonly TextItem[]>): Promise<string> {
  const quads = e.serialize()?.quadPoints;
  if (!quads) return "";
  return textUnder(await items(e.pageIndex + 1), quadBoxes(quads));
}

/** pdf.js toggles a note's comment for a press of its Show comment button (opens it), a hover, a drag and a hide: only the press opens Marshell's popup. */
export function opensComment(editor: unknown, isSelected: boolean | null | undefined, visibility: boolean | undefined): boolean {
  return editor !== null && editor !== undefined && isSelected === true && visibility === undefined;
}

export function selectionLayer(sel: { isCollapsed: boolean; anchorNode: Node | null; focusNode: Node | null } | null, container: Element): Element | null {
  if (!sel || sel.isCollapsed || !sel.anchorNode || !sel.focusNode) return null;
  const layerOf = (n: Node) => ("closest" in n ? (n as Element) : n.parentElement)?.closest(".textLayer") ?? null;
  const from = layerOf(sel.anchorNode);
  return from && from === layerOf(sel.focusNode) && container.contains(from) ? from : null;
}

/**
 * Whether a press on a page would start pdf.js's free-hand highlight: it draws when the press lands on the text layer
 * itself or on one of its blank fillers, not on text. Marshell highlights text only, so these presses never reach pdf.js.
 */
export function drawsFreehand(target: Element | null, text: HTMLElement): boolean {
  if (!target) return false;
  if ((target as unknown) === text) return true;
  const blank = target.getAttribute("role") === "img" || ["endOfContent", "textLayerImages", "textLayerImagePlaceholder"].some((c) => target.classList.contains(c));
  return blank && text.contains(target);
}

/**
 * Whether a key pressed on `target` (inside the tab, with the editor live) goes to pdf.js. A note's keys do (Delete, the
 * arrows, Ctrl+Z and Ctrl+Y); on the pages, Ctrl+Z and Ctrl+Y do too, so a deleted note can come back (pdf.js acts only on
 * their Ctrl forms), and Enter and Space do with the Free text tool on, and place a box. Esc never does: pdf.js would
 * swallow it, and the tab ends a note's editing itself before letting Esc go to the terminal. Text being typed keeps every
 * key.
 */
export function pdfjsTakesKey(key: string, target: Element | null, tool: NoteTool): boolean {
  if (!target || key === "Escape") return false;
  if (target.closest(EDITABLE) || target.closest(OWN_CONTROLS)) return false;
  if (target.closest(".annotationEditorLayer")) return true;
  const onPages = target.closest(".pdf__pagelist") !== null || target.closest(".preview__body") === target;
  if (["z", "Z", "y"].includes(key)) return onPages;
  if (key !== "Enter" && key !== " ") return false;
  if (tool !== "freetext" || target.closest(FOLLOWS)) return false;
  return onPages;
}

/** The pdf.js layers over one drawn page. */
export type PageLayers = { editor: { destroy(): void }; draw: { destroy(): void } };
export type LayerMaker = (pageIndex: number, page: PDFPageProxy, parts: { holder: HTMLElement; text: HTMLElement }, scale: number) => Promise<PageLayers>;

function drop(l: PageLayers) {
  // The editor layer first: its notes leave the draw layer as they are taken off the page.
  l.editor.destroy();
  l.draw.destroy();
}

/**
 * Keeps each attached page's layers until its detach. A detach drops the layers only, never a note: pdf.js's UI manager
 * keeps them, and the page's next layers draw them again. Layers that arrive after their page was dropped are dropped.
 */
export function pageAttacher(make: LayerMaker) {
  const made = new Map<number, PageLayers>();
  return {
    attach(n: number, page: PDFPageProxy, parts: { holder: HTMLElement; text: HTMLElement }, scale: number): () => void {
      let gone = false;
      let mine: PageLayers | null = null;
      make(n - 1, page, parts, scale).then(
        (l) => {
          if (gone) return drop(l);
          const old = made.get(n);
          if (old) drop(old);
          mine = l;
          made.set(n, l);
        },
        () => {},
      );
      return () => {
        if (gone) return;
        gone = true;
        if (!mine) return;
        drop(mine);
        if (made.get(n) === mine) made.delete(n);
      };
    },
    /** The pages that have layers now, in order. */
    attached: (): number[] => [...made.keys()].sort((a, b) => a - b),
  };
}

const COLORS = HIGHLIGHT_COLORS.map((c) => `${c.name.toLowerCase()}=${c.value}`).join(",");

// pdf.js's editor reads a few things this file can't type exactly (the layer options' text layer is a builder with a `div`,
// the d.ts says an element); these are the members used.
type Ui = {
  isShiftKeyDown: boolean;
  readonly hasSelection: boolean;
  get currentLayer(): unknown;
  keydown(e: KeyboardEvent): void;
  highlightSelection(method?: string, comment?: boolean): void;
  addCommands(params: unknown): void;
  addEditor(e: unknown): void;
  removeEditor(e: unknown): void;
  undo(): void;
  redo(): void;
  registerEditorTypes(types: unknown[]): void;
  getActive(): { isInEditMode?(): boolean } | null;
  isDeletedAnnotationElement(annotationElementId: string): boolean;
  unselectAll(): void;
  endCurrentEditing(): void;
  updateMode(mode: number): Promise<void>;
  onPageChanging(e: { pageNumber: number }): void;
  onScaleChanging(e: { scale: number }): void;
  destroy(): void;
};
type EditorClass = { _editorType: number; updateDefaultParams(type: number, value: unknown): void };
type Editor = {
  id: string;
  pageIndex: number;
  uid: string;
  div: HTMLElement;
  comment: { text?: string } | null;
  editComment(): void;
  readonly editorType: string;
  annotationElementId?: string | null;
  editorDiv?: HTMLElement;
  color?: string;
  isEmpty(): boolean;
  isInEditMode(): boolean;
  serialize(): { quadPoints?: ArrayLike<number> } | null;
};

/** A note editor for `doc`, over pages inside `container` (the tab's scrolling body). */
export async function createNotesEditor(
  doc: PDFDocumentProxy,
  o: { container: HTMLElement; onComment(req: CommentRequest): void; /** A line to announce: a note made or deleted, or free text that may show blank. */ onNews?(line: string): void },
): Promise<NotesEditor> {
  const lib = await editorLib();
  const { AnnotationEditorType: Type, AnnotationEditorParamsType: Param } = lib;
  const eventBus = new lib.EventBus();
  const l10n = new lib.GenericL10n(undefined as never);
  void l10n.translate(o.container);

  let live = false;
  let tool: NoteTool = "none";
  let types: readonly EditorClass[] = [];
  let changes = 0;
  const listeners = new Set<() => void>();
  // Every note pdf.js holds, the file's own and new ones, on drawn pages or not (a dropped page's layer leaves its notes here).
  const held = new Map<string, Editor>();
  const readd = readdGuard();
  const news: NewsState = { known: new Set(), warned: new Set(), gone: 0 };
  // A tab going away says nothing of the notes pdf.js lets go as it is destroyed.
  let quiet = false;
  const changed = () => {
    changes++;
    const now = [...held.values()].map((e) => ({
      id: e.id,
      kind: e.editorType,
      page: e.pageIndex + 1,
      fromFile: !!e.annotationElementId,
      ready: !e.isEmpty() && !e.isInEditMode(),
      text: e.editorType === "freetext" ? (e.editorDiv?.innerText ?? null) : null,
    }));
    const lines = noteNews(news, now);
    if (lines.length > 0 && !quiet) o.onNews?.(lines.join(" "));
    for (const cb of listeners) cb();
  };
  const layers = new Map<number, TextLayerOwner>();

  // pdf.js's comment dialog, as the editor asks for it, and a press of a note's Show comment: Marshell's own popup answers.
  const askComment = (editor: Editor) => {
    o.onComment({
      page: editor.pageIndex + 1,
      anchor: editor.div.getBoundingClientRect(),
      note: editor.div,
      text: editor.comment?.text ?? "",
      done(text) {
        if (text === null) return;
        editor.comment = text as never;
        changed();
      },
    });
  };
  const commentManager = {
    showDialog(_ui: unknown, editor: Editor) {
      askComment(editor);
    },
    setSidebarUiManager() {},
    updateComment() {},
    updatePopupColor() {},
    removeComments() {},
    toggleCommentPopup(editor: Editor | null, isSelected?: boolean, visibility?: boolean) {
      if (editor && opensComment(editor, isSelected, visibility)) askComment(editor);
    },
    makeCommentColor: () => null,
    // pdf.js points a note's comment button at it (`ariaControlsElements`) and throws on null.
    dialogElement: document.createElement("div"),
    destroyPopup() {},
    hideSidebar() {},
    showSidebar() {},
    destroy() {},
  };

  // The UI manager's window and document listeners are its methods, bound when they are added; overriding them here is how
  // they reach pdf.js only while the keyboard is on the pages.
  class Manager extends (lib.AnnotationEditorUIManager as unknown as new (...a: unknown[]) => Ui) {
    keydown(e: KeyboardEvent) {
      if (e.key === "Shift") this.isShiftKeyDown = true;
      if (live && pdfjsTakesKey(e.key, e.target instanceof Element ? e.target : null, tool)) super.keydown(e);
    }
    // Notes aren't copied, cut, pasted or dropped: pdf.js would take every copy and paste in the window while editing.
    copy() {}
    cut() {}
    async paste() {}
    dragOver() {}
    drop() {}
    // A selection's end highlights at once only with the Highlight tool on; otherwise the selection stays for the person.
    highlightSelection(method = "", comment = false) {
      if (method === "main_toolbar" && tool !== "highlight") return;
      if (!selectionLayer(document.getSelection(), o.container)) return;
      super.highlightSelection(method, comment);
    }
    addCommands(params: unknown) {
      if (readd.takesCommand()) return;
      super.addCommands(params);
      changed();
    }
    // The notes pdf.js holds, mirrored for the announcements; a removal is a deletion only for a note already told of or from the file.
    addEditor(e: Editor) {
      readd.added(held.has(e.id));
      super.addEditor(e);
      held.set(e.id, e);
      if (e.annotationElementId) news.known.add(e.id);
    }
    removeEditor(e: Editor) {
      super.removeEditor(e);
      held.delete(e.id);
      if (news.known.delete(e.id)) news.gone++;
    }
    undo() {
      super.undo();
      changed();
    }
    redo() {
      super.redo();
      changed();
    }
    registerEditorTypes(t: EditorClass[]) {
      types = t;
      super.registerEditorTypes(t);
    }
    get currentLayer() {
      return layerInView(super.currentLayer as TextLayerOwner | undefined, layers);
    }
  }
  const ui = new Manager(
    o.container,
    { classList: o.container.classList },
    null,
    null,
    commentManager,
    null,
    eventBus,
    doc,
    null,
    COLORS,
    false,
    false,
    false,
    null,
    null,
    false,
  );

  // pdf.js asks the viewer to change mode (selecting a note of another kind asks for that kind's): the person's tool decides.
  const modeOf = (t: NoteTool) => (t === "freetext" ? Type.FREETEXT : Type.HIGHLIGHT);
  await ui.updateMode(modeOf(tool));

  const pages = pageAttacher(async (pageIndex, page, { holder, text }, scale) => {
    const viewport = page.getViewport({ scale }).clone({ dontFlip: true });
    for (const [k, v] of [
      ["--scale-factor", String(scale)],
      ["--total-scale-factor", String(scale)],
      ["--user-unit", "1"],
      ["--scale-round-x", "1px"],
      ["--scale-round-y", "1px"],
    ] as const)
      holder.style.setProperty(k, v);
    const annotations = (await page.getAnnotations({ intent: "display" })).filter((a: { isEditable?: boolean }) => a.isEditable);
    const host = document.createElement("div");
    host.className = "pdf__draw";
    const draw = new lib.DrawLayer({ pageIndex });
    draw.setParent(host);
    // The file's own notes, as pdf.js's annotation layer reads them; never shown, only handed to the editor.
    const kept = document.createElement("div");
    kept.className = "annotationLayer";
    kept.hidden = true;
    const fileNotes = new lib.AnnotationLayer({
      div: kept,
      page,
      viewport,
      annotationEditorUIManager: ui,
      annotationStorage: doc.annotationStorage,
      accessibilityManager: null,
      annotationCanvasMap: null,
      structTreeLayer: null,
      commentManager: null,
      linkService: null,
    } as never);
    await fileNotes.render({ annotations, viewport, div: kept, page, renderForms: false, imageResourcesPath: "", enableScripting: false, hasJSActions: false, fieldObjects: null } as never);
    const div = document.createElement("div");
    div.className = "annotationEditorLayer";
    div.hidden = true;
    div.dir = "ltr";
    text.before(host);
    holder.append(kept, div);
    // pdf.js listens on the text layer; stopping the press on its way down keeps the drawing from starting. The press
    // still lets go of a selected note, which is the rest of what pdf.js's listener does.
    const noDrawing = (e: PointerEvent) => {
      if (!drawsFreehand(e.target instanceof Element ? e.target : null, text)) return;
      e.stopPropagation();
      ui.unselectAll();
    };
    holder.addEventListener("pointerdown", noDrawing, true);
    const layer = new lib.AnnotationEditorLayer({
      uiManager: ui,
      pageIndex,
      div,
      structTreeLayer: null,
      accessibilityManager: null,
      annotationLayer: fileNotes,
      drawLayer: draw,
      textLayer: { div: text },
      viewport,
      l10n,
    } as never);
    layers.set(pageIndex, layer);
    await layer.render({ viewport } as never);
    return {
      editor: {
        destroy() {
          if (layers.get(pageIndex) === layer) layers.delete(pageIndex);
          holder.removeEventListener("pointerdown", noDrawing, true);
          layer.destroy();
          div.remove();
          kept.remove();
        },
      },
      draw: {
        destroy() {
          draw.destroy();
          host.remove();
        },
      },
    };
  });

  // Where pdf.js measures a note's size and places free text from the keyboard; it keeps `realScale` as the scale times
  // its CSS-per-point factor, and the tab's scale already is CSS pixels per point.
  const toPdfjsScale = (scale: number) => scale / lib.PixelsPerInch.PDF_TO_CSS_UNITS;
  let shownScale = 0;

  return {
    attachPage: (n, page, parts, scale) => pages.attach(n, page, parts, scale),
    setTool(t) {
      if (t === tool) return;
      tool = t;
      void ui.updateMode(modeOf(t));
    },
    setLive(l) {
      live = l;
    },
    setPage(n, scale) {
      ui.onPageChanging({ pageNumber: n });
      if (scale !== shownScale) {
        shownScale = scale;
        ui.onScaleChanging({ scale: toPdfjsScale(scale) });
      }
    },
    setColor(c) {
      for (const t of types) t.updateDefaultParams(Param.HIGHLIGHT_COLOR, c);
    },
    highlightSelection(comment) {
      if (!selectionLayer(document.getSelection(), o.container)) return false;
      ui.highlightSelection("floating_button", comment);
      return true;
    },
    escape() {
      const active = ui.getActive();
      if (active?.isInEditMode?.()) {
        ui.endCurrentEditing();
        return true;
      }
      if (ui.hasSelection) {
        ui.unselectAll();
        return true;
      }
      return false;
    },
    editing: () => ui.getActive()?.isInEditMode?.() === true,
    modified: () => changes > 0,
    async notes() {
      const file = await readFileNotes(doc);
      const mine: HeldNote[] = [];
      for (const e of held.values()) {
        if (e.editorType !== "highlight" && e.editorType !== "freetext") continue;
        const kind = e.editorType;
        const at = parseFloat(e.div.style.top);
        mine.push({
          id: e.id,
          fileId: e.annotationElementId ?? null,
          page: e.pageIndex + 1,
          kind,
          color: kind === "highlight" ? (e.color ?? null) : null,
          text: kind === "highlight" ? await highlightText(e, (n) => pageText(doc, n)) : (e.editorDiv?.innerText.trim() ?? ""),
          comment: kind === "highlight" ? (e.comment?.text ?? "") : "",
          top: e.div.style.top.endsWith("%") && Number.isFinite(at) ? at / 100 : null,
          ready: !e.isEmpty() && !e.isInEditMode(),
        });
      }
      return mergeNotes(file, mine, (id) => ui.isDeletedAnnotationElement(id));
    },
    focusNote(id) {
      for (const e of held.values()) {
        if (e.id !== id && e.annotationElementId !== id) continue;
        if (!e.div.isConnected) return false;
        e.div.focus({ preventScroll: true });
        return document.activeElement === e.div;
      }
      return false;
    },
    onChange(cb) {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    async save() {
      ui.endCurrentEditing();
      return doc.saveDocument();
    },
    markSaved() {
      changes = 0;
    },
    destroy() {
      quiet = true;
      listeners.clear();
      ui.destroy();
      void l10n.destroy();
    },
  };
}
