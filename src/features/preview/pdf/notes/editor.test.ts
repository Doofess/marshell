import { describe, expect, it } from "vitest";
import { drawsFreehand, editorKeysAllowed, HIGHLIGHT_COLORS, highlightText, layerInView, mergeNotes, noteNews, opensComment, pageAttacher, pdfjsTakesKey, readdGuard, selectionLayer, type HeldNote, type LiveNote, type NewsState, type Note, type PageLayers } from "./editor";

/** A stand-in for a DOM element: `contains` follows the parent chain, `closest` matches by a list of selectors it answers to. */
type Fake = { parent: Fake | null; matches: string[]; contains(o: unknown): boolean; closest(sel: string): Fake | null };
function el(parent: Fake | null = null, matches: string[] = []): Fake {
  const self: Fake = {
    parent,
    matches,
    contains(o) {
      for (let at = o as Fake | null; at; at = at.parent) if (at === self) return true;
      return false;
    },
    closest(sel) {
      const wanted = sel.split(",").map((s) => s.trim());
      for (let at: Fake | null = self; at; at = at.parent) if (at.matches.some((m) => wanted.includes(m))) return at;
      return null;
    },
  };
  return self;
}
const asEl = (f: Fake | null) => f as unknown as Element | null;

describe("editorKeysAllowed", () => {
  const root = el();
  const page = el(el(root));
  const terminal = el(el());

  it("editorKeysAllowed_inside_the_tab", () => {
    expect(editorKeysAllowed(asEl(page), asEl(root), true)).toBe(true);
    expect(editorKeysAllowed(asEl(root), asEl(root), true)).toBe(true);
  });

  it("editorKeysAllowed_not_from_the_terminal", () => {
    expect(editorKeysAllowed(asEl(terminal), asEl(root), true)).toBe(false);
    expect(editorKeysAllowed(null, asEl(root), true)).toBe(false);
    expect(editorKeysAllowed(asEl(page), null, true)).toBe(false);
  });

  it("editorKeysAllowed_not_while_the_window_is_away", () => {
    expect(editorKeysAllowed(asEl(page), asEl(root), false)).toBe(false);
  });
});

describe("pdfjsTakesKey", () => {
  const scroller = el(null, [".preview__body"]);
  const layer = el(el(scroller, [".pdf__pagelist"]), [".annotationEditorLayer"]);
  const note = el(layer, [".freeTextEditor"]);
  const typing = el(note, ["[contenteditable]"]);
  const link = el(el(scroller, [".pdf__pagelist"]), ["a"]);
  const find = el(scroller, ["input"]);
  const t = (f: Fake) => f as unknown as Element;

  it("a note's keys go to pdf.js", () => {
    for (const key of ["Delete", "Backspace", "ArrowLeft", "z", "y"]) expect(pdfjsTakesKey(key, t(note), "none")).toBe(true);
  });

  it("Esc never goes to pdf.js, which would swallow it", () => {
    expect(pdfjsTakesKey("Escape", t(note), "highlight")).toBe(false);
  });

  it("text being typed keeps its keys", () => {
    expect(pdfjsTakesKey("Backspace", t(typing), "freetext")).toBe(false);
    expect(pdfjsTakesKey("Backspace", t(find), "freetext")).toBe(false);
  });

  it("Enter and Space on the pages place free text only with that tool on", () => {
    expect(pdfjsTakesKey("Enter", t(scroller), "freetext")).toBe(true);
    expect(pdfjsTakesKey(" ", t(scroller), "freetext")).toBe(true);
    expect(pdfjsTakesKey(" ", t(scroller), "none")).toBe(false);
    expect(pdfjsTakesKey("Enter", t(scroller), "highlight")).toBe(false);
  });

  it("Marshell's own controls over the pages keep their keys: the comment popup and the selection bar", () => {
    const list = el(scroller, [".pdf__pagelist"]);
    const popupButton = el(el(list, [".pdf-comment"]), ["button"]);
    const barButton = el(el(list, [".pdf-selbar"]), ["button"]);
    for (const key of ["z", "Z", "y"]) {
      expect(pdfjsTakesKey(key, t(popupButton), "none")).toBe(false);
      expect(pdfjsTakesKey(key, t(barButton), "highlight")).toBe(false);
    }
  });

  it("Enter on a link follows the link", () => {
    expect(pdfjsTakesKey("Enter", t(link), "freetext")).toBe(false);
  });

  it("Delete, the arrows and Ctrl+A on the pages but off a note stay the page's", () => {
    for (const key of ["Delete", "ArrowDown", "a"]) expect(pdfjsTakesKey(key, t(scroller), "freetext")).toBe(false);
  });

  it("Ctrl+Z and Ctrl+Y on the pages undo and redo, so a deleted note can come back", () => {
    for (const key of ["z", "Z", "y"]) expect(pdfjsTakesKey(key, t(scroller), "none")).toBe(true);
    expect(pdfjsTakesKey("z", t(find), "none")).toBe(false);
  });
});

describe("drawsFreehand", () => {
  /** A text layer's child as pdf.js tells them apart: by its role and its classes. */
  const child = (parent: object, o: { role?: string; cls?: string[] } = {}) => ({
    parentElement: parent,
    getAttribute: (a: string) => (a === "role" ? (o.role ?? null) : null),
    classList: { contains: (c: string) => (o.cls ?? []).includes(c) },
  });
  const text = { contains: (t: { parentElement?: unknown }) => t.parentElement === text } as unknown as HTMLElement;
  const t = (x: unknown) => x as Element;

  it("a press on the bare text layer would draw, and is held back", () => {
    expect(drawsFreehand(t(text), text)).toBe(true);
    expect(drawsFreehand(t(child(text, { role: "img" })), text)).toBe(true);
    expect(drawsFreehand(t(child(text, { cls: ["endOfContent"] })), text)).toBe(true);
    expect(drawsFreehand(t(child(text, { cls: ["textLayerImages"] })), text)).toBe(true);
  });

  it("a press on the text selects it, as without notes", () => {
    expect(drawsFreehand(t(child(text)), text)).toBe(false);
    expect(drawsFreehand(t(child({}, { role: "img" })), text)).toBe(false);
    expect(drawsFreehand(null, text)).toBe(false);
  });
});

describe("selectionLayer", () => {
  const tab = el();
  const layer = el(el(tab), [".textLayer"]);
  const other = el(el(tab), [".textLayer"]);
  const hidden = el(el(el()), [".textLayer"]);
  /** A text node: no `closest` of its own, only its parent span. */
  const textIn = (l: Fake) => ({ parentElement: el(l) }) as unknown as Node;
  const sel = (a: Node, f: Node, isCollapsed = false) => ({ anchorNode: a, focusNode: f, isCollapsed });
  const tabEl = asEl(tab)!;

  it("a selection in one of the tab's text layers is the tab's to highlight", () => {
    expect(selectionLayer(sel(textIn(layer), textIn(layer)), tabEl)).toBe(layer);
  });

  it("a selection in another tab's pages is left alone, so a hidden tab never empties it", () => {
    expect(selectionLayer(sel(textIn(hidden), textIn(hidden)), tabEl)).toBeNull();
  });

  it("a selection across two pages, or an empty one, highlights nothing", () => {
    expect(selectionLayer(sel(textIn(layer), textIn(other)), tabEl)).toBeNull();
    expect(selectionLayer(sel(textIn(layer), textIn(layer), true), tabEl)).toBeNull();
    expect(selectionLayer(null, tabEl)).toBeNull();
  });
});

describe("layerInView", () => {
  it("the page in view's layer, else any drawn page's", () => {
    const a = { hasTextLayer: () => true };
    const b = { hasTextLayer: () => true };
    expect(layerInView(a, new Map([[1, b]]))).toBe(a);
    expect(layerInView(undefined, new Map([[1, b]]))).toBe(b);
  });

  it("with no page drawn (a hidden tab), a selection elsewhere finds no layer here instead of throwing", () => {
    expect(layerInView(undefined, new Map()).hasTextLayer({} as HTMLElement)).toBe(false);
  });
});

describe("HIGHLIGHT_COLORS", () => {
  it("HIGHLIGHT_COLORS_are_pdfjs_defaults_in_order", () => {
    expect(HIGHLIGHT_COLORS).toEqual([
      { name: "Yellow", value: "#FFFF98" },
      { name: "Green", value: "#53FFBC" },
      { name: "Blue", value: "#80EBFF" },
      { name: "Pink", value: "#FFCBE6" },
      { name: "Red", value: "#FF4F5F" },
    ]);
  });
});

/** The UI manager's side of a page's layers, as pdf.js keeps it: a layer registers on making and leaves on destroy, while the editors stay. */
function fakeManager() {
  const layers = new Map<number, object>();
  const editors = new Map<string, { pageIndex: number }>([["e1", { pageIndex: 1 }]]);
  const removed: string[] = [];
  let made = 0;
  let destroyed = 0;
  const make = async (pageIndex: number): Promise<PageLayers> => {
    const layer = { pageIndex, n: ++made };
    layers.set(pageIndex, layer);
    return {
      editor: {
        destroy() {
          destroyed++;
          if (layers.get(pageIndex) === layer) layers.delete(pageIndex);
        },
      },
      draw: { destroy() {} },
    };
  };
  return { layers, editors, removed, make, made: () => made, destroyed: () => destroyed };
}
const tick = () => new Promise((r) => setTimeout(r, 0));
const page = {} as never;
const parts = { holder: {} as HTMLElement, text: {} as HTMLElement };

describe("pageAttacher", () => {
  it("a page drawn again gets its notes back", async () => {
    const m = fakeManager();
    const pages = pageAttacher(m.make);
    const detach = pages.attach(2, page, parts, 1);
    await tick();
    expect(m.layers.get(1)).toMatchObject({ n: 1 });
    detach();
    expect(m.layers.has(1)).toBe(false);
    expect(m.editors.get("e1")).toEqual({ pageIndex: 1 });
    pages.attach(2, page, parts, 1.5);
    await tick();
    expect(m.layers.get(1)).toMatchObject({ n: 2 });
    expect([...m.editors.keys()]).toEqual(["e1"]);
    expect(m.destroyed()).toBe(1);
  });

  it("a page dropped before its layers are made drops them when they come", async () => {
    const m = fakeManager();
    const pages = pageAttacher(m.make);
    pages.attach(3, page, parts, 1)();
    await tick();
    expect(m.made()).toBe(1);
    expect(m.destroyed()).toBe(1);
    expect(m.layers.has(2)).toBe(false);
    expect(pages.attached()).toEqual([]);
  });

  it("knows which pages have layers", async () => {
    const m = fakeManager();
    const pages = pageAttacher(m.make);
    pages.attach(1, page, parts, 1);
    const off = pages.attach(4, page, parts, 1);
    await tick();
    expect(pages.attached()).toEqual([1, 4]);
    off();
    expect(pages.attached()).toEqual([1]);
  });
});

describe("noteNews", () => {
  const fresh = (): NewsState => ({ known: new Set(), warned: new Set(), gone: 0 });
  const note = (id: string, o: Partial<LiveNote> = {}): LiveNote => ({ id, kind: "highlight", page: 1, fromFile: false, ready: true, text: null, ...o });

  it("noteNews_a_new_highlight_says_its_page", () => {
    const s = fresh();
    expect(noteNews(s, [note("a", { page: 3 })])).toEqual(["Highlighted on page 3."]);
    // Told once: the next change with the same note says nothing of it.
    expect(noteNews(s, [note("a", { page: 3 })])).toEqual([]);
  });

  it("noteNews_free_text_is_told_once_it_is_typed_and_ended", () => {
    const s = fresh();
    // Made, empty or being typed: not a note yet.
    expect(noteNews(s, [note("f", { kind: "freetext", page: 2, ready: false })])).toEqual([]);
    expect(noteNews(s, [note("f", { kind: "freetext", page: 2, ready: true, text: "Check" })])).toEqual(["Free text added on page 2."]);
    // An edit of it later is not news.
    expect(noteNews(s, [note("f", { kind: "freetext", page: 2, text: "Check this" })])).toEqual([]);
  });

  it("noteNews_the_file's_own_notes_are_not_news", () => {
    const s = fresh();
    expect(noteNews(s, [note("x", { fromFile: true }), note("y", { kind: "freetext", fromFile: true, text: "Привет" })])).toEqual([]);
  });

  it("noteNews_removals_say_note_deleted_once", () => {
    const s = fresh();
    noteNews(s, [note("a"), note("b")]);
    s.gone = 2;
    expect(noteNews(s, [])).toEqual(["Note deleted."]);
    expect(s.gone).toBe(0);
    expect(noteNews(s, [])).toEqual([]);
  });

  it("noteNews_free_text_outside_winansi_warns_once", () => {
    const s = fresh();
    const warn = "Edge and Chrome may show this note blank: it has letters outside the Latin alphabet.";
    expect(noteNews(s, [note("f", { kind: "freetext", page: 1, text: "Café" })])).toEqual(["Free text added on page 1."]);
    expect(noteNews(s, [note("f", { kind: "freetext", page: 1, text: "Café 東京" })])).toEqual([warn]);
    expect(noteNews(s, [note("f", { kind: "freetext", page: 1, text: "Café 東京 again" })])).toEqual([]);
    expect(noteNews(fresh(), [note("g", { kind: "freetext", page: 4, text: "東京" })])).toEqual(["Free text added on page 4.", warn]);
  });

  it("noteNews_other_kinds_are_not_told_as_added", () => {
    expect(noteNews(fresh(), [note("i", { kind: "ink" })])).toEqual([]);
  });
});

describe("mergeNotes", () => {
  const fileNote = (o: Partial<Note> = {}): Note => ({ id: "12R", page: 1, kind: "highlight", kindName: "Highlight", color: "#FFFF98", text: "quick", comment: "old", editable: true, top: 0.2, ...o });
  const heldNote = (o: Partial<HeldNote> = {}): HeldNote => ({ id: "pdfjs_internal_editor_1", fileId: null, page: 1, kind: "highlight", color: "#53FFBC", text: "brown", comment: "", top: 0.5, ready: true, ...o });
  const none = () => false;

  it("mergeNotes_keeps_the_file_notes_when_nothing_changed", () => {
    expect(mergeNotes([fileNote()], [], none)).toEqual([fileNote()]);
  });

  it("mergeNotes_an_edited_comment_and_colour_replace_the_file_note", () => {
    const out = mergeNotes([fileNote()], [heldNote({ fileId: "12R", comment: "new", color: "#FF4F5F", text: "", top: null })], none);
    expect(out).toEqual([fileNote({ comment: "new", color: "#FF4F5F" })]);
  });

  it("mergeNotes_a_deleted_file_note_goes", () => {
    expect(mergeNotes([fileNote(), fileNote({ id: "13R" })], [], (id) => id === "12R").map((n) => n.id)).toEqual(["13R"]);
  });

  it("mergeNotes_a_new_highlight_is_added_with_its_place_and_marked_as_made_here", () => {
    const out = mergeNotes([fileNote()], [heldNote()], none);
    expect(out).toHaveLength(2);
    expect(out[1]).toEqual({ id: "pdfjs_internal_editor_1", page: 1, kind: "highlight", kindName: "Highlight", color: "#53FFBC", text: "brown", comment: "", editable: true, top: 0.5, added: true });
    expect(mergeNotes([], [heldNote({ top: null })], none)[0]!.top).toBe(0);
  });

  it("mergeNotes_a_free_text_being_typed_is_left_out", () => {
    expect(mergeNotes([], [heldNote({ kind: "freetext", ready: false })], none)).toEqual([]);
    expect(mergeNotes([], [heldNote({ kind: "freetext", color: null, text: "Hi" })], none)[0]).toMatchObject({ kind: "freetext", kindName: "Free text", text: "Hi", added: true });
  });
});

describe("live check 6q-6: notes coming back to a redrawn page", () => {
  it("readdGuard_a_note_coming_back_to_a_redrawn_page_adds_no_undo_step", () => {
    // pdf.js puts a highlight on the undo stack again as its page's new layer takes it (`onceAdded`): a zoom made the tab
    // unsaved, and Ctrl+Z then took a note away.
    const g = readdGuard();
    g.added(true);
    expect(g.takesCommand()).toBe(true);
  });
  it("readdGuard_a_new_note_s_step_counts", () => {
    const g = readdGuard();
    g.added(false);
    expect(g.takesCommand()).toBe(false);
  });
  it("readdGuard_takes_one_step_only", () => {
    const g = readdGuard();
    g.added(true);
    g.takesCommand();
    expect(g.takesCommand()).toBe(false);
  });
  it("readdGuard_lasts_only_until_the_redraw_s_task_ends", async () => {
    // A free text coming back adds no step; a person's next change, in a later task, still counts.
    const g = readdGuard();
    g.added(true);
    await Promise.resolve();
    expect(g.takesCommand()).toBe(false);
  });
});

describe("live check 6q-6: a highlight's text in the Notes view", () => {
  // pdf.js's own l10n puts "Highlight editor" on the note's element, over the text pdf.js set there.
  const items = [
    { str: "Page 1: The ", transform: [1, 0, 0, 1, 72, 720], width: 60, height: 12 },
    { str: "quick brown fox", transform: [1, 0, 0, 1, 152, 720], width: 96, height: 12 },
    { str: " jumps", transform: [1, 0, 0, 1, 250, 720], width: 40, height: 12 },
  ];
  it("highlightText_is_the_page_text_under_the_highlight_not_its_label", async () => {
    const e = { pageIndex: 0, serialize: () => ({ quadPoints: Float32Array.of(150, 734, 250, 734, 150, 716, 250, 716) }) };
    expect(await highlightText(e, async (n) => (n === 1 ? items : []))).toBe("quick brown fox");
  });
  it("highlightText_an_unchanged_file_highlight_has_none_and_keeps_the_file_s", async () => {
    // pdf.js serializes nothing for a file note no one changed; the list keeps the text read from the file.
    expect(await highlightText({ pageIndex: 0, serialize: () => null }, async () => items)).toBe("");
  });
});

describe("live check 6q-6: Show comment", () => {
  it("opensComment_on_a_press_of_show_comment", () => {
    expect(opensComment({}, true, undefined)).toBe(true);
  });
  it("opensComment_not_for_a_hover_a_drag_or_a_hide", () => {
    expect(opensComment({}, false, true)).toBe(false);
    expect(opensComment({}, false, false)).toBe(false);
    expect(opensComment({}, true, false)).toBe(false);
    expect(opensComment(null, true, undefined)).toBe(false);
  });
});
