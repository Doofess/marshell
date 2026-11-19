import { describe, expect, it, vi } from "vitest";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { hexOf, inBoxes, quadBoxes, readFileNotes, subtypeName, textUnder } from "./fileNotes";

const quad = [10, 712, 100, 712, 10, 700, 100, 700];
const item = (str: string, x: number, y: number, width = 30, height = 10) => ({ str, transform: [10, 0, 0, 10, x, y], width, height });

describe("fileNotes helpers", () => {
  it("subtypeName_gives_a_sentence_case_name_and_falls_back_to_the_subtype", () => {
    expect(subtypeName("Text")).toBe("Sticky note");
    expect(subtypeName("StrikeOut")).toBe("Strikeout");
    expect(subtypeName("Squiggly")).toBe("Squiggly underline");
    expect(subtypeName("Ink")).toBe("Drawing");
    expect(subtypeName("FileAttachment")).toBe("Attachment");
    expect(subtypeName("PolyLine")).toBe("Polyline");
    expect(subtypeName("Redact")).toBe("Redact");
  });

  it("quadBoxes_reads_flat_and_nested_quad_points", () => {
    expect(quadBoxes(quad)).toEqual([[10, 700, 100, 712]]);
    expect(quadBoxes([...quad, 10, 690, 50, 690, 10, 680, 50, 680])).toEqual([
      [10, 700, 100, 712],
      [10, 680, 50, 690],
    ]);
    const nested = [
      { x: 10, y: 712 },
      { x: 100, y: 712 },
      { x: 10, y: 700 },
      { x: 100, y: 700 },
    ];
    expect(quadBoxes([nested])).toEqual([[10, 700, 100, 712]]);
    expect(quadBoxes(undefined)).toEqual([]);
  });

  it("inBoxes_is_true_for_a_point_inside_a_box_and_false_outside", () => {
    const boxes = quadBoxes(quad);
    expect(inBoxes(50, 706, boxes)).toBe(true);
    expect(inBoxes(50, 720, boxes)).toBe(false);
    expect(inBoxes(5, 706, boxes)).toBe(false);
  });

  it("textUnder_keeps_words_whose_middle_is_in_a_quad_in_order_joined_and_trimmed", () => {
    const items = [item("before", 200, 706), item("quick", 12, 702), item("brown", 50, 702), item("after", 12, 600)];
    expect(textUnder(items, quadBoxes(quad))).toBe("quick brown");
    expect(textUnder(items, [])).toBe("");
    expect(textUnder([item("x".repeat(300), 12, 702, 60)], quadBoxes(quad)).length).toBe(200);
  });

  it("hexOf_writes_a_colour_as_RRGGBB_or_null", () => {
    expect(hexOf([255, 255, 152])).toBe("#FFFF98");
    expect(hexOf(new Uint8ClampedArray([0, 10, 255]))).toBe("#000AFF");
    expect(hexOf(null)).toBeNull();
    expect(hexOf([1, 2])).toBeNull();
  });
});

function fakeDoc(pages: unknown[][], text: Record<number, unknown[]> = {}) {
  const getAnnotations = vi.fn(async (n: number) => pages[n - 1] ?? []);
  const getTextContent = vi.fn(async (n: number) => ({ items: text[n] ?? [] }));
  const doc = {
    numPages: pages.length,
    getPage: async (n: number) => ({
      getAnnotations: () => getAnnotations(n),
      getTextContent: () => getTextContent(n),
      getViewport: () => ({ height: 800, convertToViewportPoint: (x: number, y: number) => [x, 800 - y] }),
    }),
  } as unknown as PDFDocumentProxy;
  return { doc, getAnnotations, getTextContent };
}

const highlight = { id: "12R", subtype: "Highlight", rect: [10, 700, 100, 712], quadPoints: quad, color: [255, 255, 152], contentsObj: { str: "Check this date" } };
const freeText = { id: "13R", subtype: "FreeText", rect: [20, 400, 120, 420], color: null, contentsObj: { str: "Sign here" } };

describe("readFileNotes", () => {
  it("reads_markup_notes_with_kind_text_comment_colour_and_top", async () => {
    const { doc } = fakeDoc([[highlight, { id: "9R", subtype: "Popup" }, { id: "8R", subtype: "Link" }, { id: "7R", subtype: "Widget" }], [freeText, { id: "14R", subtype: "Text", rect: [5, 790, 20, 800], contentsObj: { str: "Hi" } }]], { 1: [item("quick", 12, 702)] });
    const notes = await readFileNotes(doc);
    expect(notes).toEqual([
      { id: "12R", page: 1, kind: "highlight", kindName: "Highlight", color: "#FFFF98", text: "quick", comment: "Check this date", editable: true, top: (800 - 712) / 800 },
      { id: "13R", page: 2, kind: "freetext", kindName: "Free text", color: null, text: "Sign here", comment: "", editable: true, top: (800 - 420) / 800 },
      { id: "14R", page: 2, kind: "other", kindName: "Sticky note", color: null, text: "", comment: "Hi", editable: false, top: 0 },
    ]);
  });

  it("stops_at_the_limit", async () => {
    const { doc } = fakeDoc([[highlight, freeText], [highlight]]);
    expect(await readFileNotes(doc, 2)).toHaveLength(2);
  });

  it("reads_each_page_once_per_document", async () => {
    const { doc, getAnnotations, getTextContent } = fakeDoc([[highlight]]);
    await readFileNotes(doc);
    await readFileNotes(doc);
    expect(getAnnotations).toHaveBeenCalledTimes(1);
    expect(getTextContent).toHaveBeenCalledTimes(1);
  });

  it("reads_no_text_for_a_page_without_a_highlight", async () => {
    const { doc, getTextContent } = fakeDoc([[freeText]]);
    await readFileNotes(doc);
    expect(getTextContent).not.toHaveBeenCalled();
  });
});

describe("live check 6q-6: a highlight on a file whose lines are one text item", () => {
  // text3.pdf has one item per line: a highlight got its whole line when it covered the line's middle, and nothing otherwise.
  const line = { str: "Page 1: The quick brown fox jumps over the lazy dog.", transform: [12, 0, 0, 12, 72, 720], width: 312, height: 12 };
  it("textUnder_takes_the_words_under_the_highlight_not_the_whole_line", () => {
    expect(textUnder([line], [[143, 716, 235, 734]])).toBe("quick brown fox");
  });
  it("textUnder_finds_words_away_from_the_line_s_middle", () => {
    expect(textUnder([line], [[290, 716, 384, 734]])).toBe("the lazy dog.");
  });
  it("textUnder_follows_a_rotated_line", () => {
    // Text running up the page: the words lie along y, and the glyphs stand to the line's left.
    const up = { str: "abc def", transform: [0, 10, -10, 0, 100, 500], width: 70, height: 10 };
    expect(textUnder([up], [[90, 540, 100, 575]])).toBe("def");
  });
});
