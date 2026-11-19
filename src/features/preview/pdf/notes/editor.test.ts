import { describe, expect, it } from "vitest";
import { drawsFreehand, editorKeysAllowed, HIGHLIGHT_COLORS, layerInView, pageAttacher, pdfjsTakesKey, selectionLayer, type PageLayers } from "./editor";

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
