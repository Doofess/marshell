import { describe, expect, it } from "vitest";
import { matchSpans, paint, unpaint } from "./highlight";
import type { TextMapping } from "./pdfjs";

describe("matchSpans", () => {
  it("matchSpans_in_one_item", () => {
    expect(matchSpans([6], [5], ["Hello world"])).toEqual([{ begin: { div: 0, offset: 6 }, end: { div: 0, offset: 11 } }]);
  });

  it("matchSpans_across_items", () => {
    expect(matchSpans([2], [3], ["Hel", "lo"])).toEqual([{ begin: { div: 0, offset: 2 }, end: { div: 1, offset: 2 } }]);
  });

  it("matchSpans_after_an_empty_item", () => {
    expect(matchSpans([2], [2], ["ab", "", "cd"])).toEqual([{ begin: { div: 2, offset: 0 }, end: { div: 2, offset: 2 } }]);
  });

  it("matchSpans_at_the_last_item_end", () => {
    expect(matchSpans([2], [2], ["ab", "cd"])).toEqual([{ begin: { div: 1, offset: 0 }, end: { div: 1, offset: 2 } }]);
    // A match that ends where an item ends stays in that item rather than starting the next one.
    expect(matchSpans([0], [2], ["ab", "cd"])).toEqual([{ begin: { div: 0, offset: 0 }, end: { div: 0, offset: 2 } }]);
  });

  it("matchSpans_with_no_matches_is_empty", () => {
    expect(matchSpans([], [], ["ab"])).toEqual([]);
    expect(matchSpans([0], [1], [])).toEqual([]);
  });

  it("matchSpans_past_the_text_is_dropped", () => {
    expect(matchSpans([0, 4], [1, 1], ["ab", "c"])).toEqual([{ begin: { div: 0, offset: 0 }, end: { div: 0, offset: 1 } }]);
  });

  it("matchSpans_keeps_several_matches_in_order", () => {
    expect(matchSpans([0, 3], [1, 2], ["ab", "cde"])).toEqual([
      { begin: { div: 0, offset: 0 }, end: { div: 0, offset: 1 } },
      { begin: { div: 1, offset: 1 }, end: { div: 1, offset: 3 } },
    ]);
  });
});

/** Just enough of the DOM for a text layer's spans: text and child spans with a class. */
type Fake = { className: string; children: (string | Fake)[]; ownerDocument: { createElement(tag: string): Fake }; textContent: string; replaceChildren(...n: (string | Fake)[]): void };
const doc = { createElement: (): Fake => el() };
function el(text = ""): Fake {
  const e: Fake = {
    className: "",
    children: text ? [text] : [],
    ownerDocument: doc,
    get textContent() {
      return e.children.map((c) => (typeof c === "string" ? c : c.textContent)).join("");
    },
    set textContent(v: string) {
      e.children = v ? [v] : [];
    },
    replaceChildren(...n) {
      e.children = n;
    },
  };
  return e;
}
const mapping = (...items: string[]): TextMapping => ({ divs: items.map((t) => el(t)) as unknown as HTMLElement[], items });
const shape = (d: HTMLElement) => (d as unknown as Fake).children.map((c) => (typeof c === "string" ? c : `[${c.className}:${c.textContent}]`));

describe("paint", () => {
  it("wraps a match within one span and leaves the text around it", () => {
    const m = mapping("Hello world");
    paint(m, matchSpans([6], [5], m.items), -1);
    expect(shape(m.divs[0]!)).toEqual(["Hello ", "[pdf__match:world]"]);
  });

  it("marks the selected match current and hands back its element", () => {
    const m = mapping("a b a");
    const got = paint(m, matchSpans([0, 4], [1, 1], m.items), 1);
    expect(shape(m.divs[0]!)).toEqual(["[pdf__match:a]", " b ", "[pdf__match pdf__match--current:a]"]);
    expect((got as unknown as Fake).textContent).toBe("a");
    expect((got as unknown as Fake).className).toContain("pdf__match--current");
  });

  it("tints each part of a match that runs across spans", () => {
    const m = mapping("Hel", "lo", "x");
    const got = paint(m, matchSpans([2], [3], m.items), 0);
    expect(shape(m.divs[0]!)).toEqual(["He", "[pdf__match pdf__match--current:l]"]);
    expect(shape(m.divs[1]!)).toEqual(["[pdf__match pdf__match--current:lo]"]);
    expect(shape(m.divs[2]!)).toEqual(["x"]);
    expect((got as unknown as Fake).textContent).toBe("l");
  });

  it("tints a span in the middle of a match whole", () => {
    const m = mapping("ab", "cd", "ef");
    paint(m, matchSpans([1], [4], m.items), -1);
    expect(shape(m.divs[1]!)).toEqual(["[pdf__match:cd]"]);
    expect(shape(m.divs[2]!)).toEqual(["[pdf__match:e]", "f"]);
  });

  it("is null when no match is selected", () => {
    const m = mapping("abc");
    expect(paint(m, matchSpans([0], [1], m.items), -1)).toBeNull();
  });
});

describe("unpaint", () => {
  it("puts the plain text back in every span it tinted", () => {
    const m = mapping("Hel", "lo", "x");
    paint(m, matchSpans([2], [3], m.items), 0);
    unpaint(m);
    expect(m.divs.map(shape)).toEqual([["Hel"], ["lo"], ["x"]]);
  });

  it("leaves a mapping that was never painted alone", () => {
    const m = mapping("abc");
    unpaint(m);
    expect(shape(m.divs[0]!)).toEqual(["abc"]);
  });

  it("clears the last paint before a new one", () => {
    const m = mapping("aXa");
    paint(m, matchSpans([0], [1], m.items), -1);
    paint(m, matchSpans([2], [1], m.items), -1);
    expect(shape(m.divs[0]!)).toEqual(["aX", "[pdf__match:a]"]);
  });
});
