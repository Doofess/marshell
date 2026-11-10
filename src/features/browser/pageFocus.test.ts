import { afterEach, describe, expect, it } from "vitest";
import { dispatchPage } from "./pageStore";
import { enterReason, handBack, isQuiet, keyForCode, onDragArea, quietly, tabNeighbour, windowFocused } from "./pageFocus";

describe("tabNeighbour", () => {
  const all = ["a", "b", "c"];
  it("wraps at both ends", () => {
    expect(tabNeighbour(all, "c", "next")).toBe("a");
    expect(tabNeighbour(all, "a", "previous")).toBe("c");
    expect(tabNeighbour(all, "b", "next")).toBe("c");
    expect(tabNeighbour(all, "b", "previous")).toBe("a");
  });
  it("is the slot itself when it is the only stop, and nothing when it isn't one", () => {
    expect(tabNeighbour(["a"], "a", "next")).toBe("a");
    expect(tabNeighbour(all, "z", "next")).toBeUndefined();
    expect(tabNeighbour([], "a", "previous")).toBeUndefined();
  });
});

describe("enterReason", () => {
  it("focus from an element after the slot enters at the page's end", () => {
    expect(enterReason(true)).toBe("previous");
    expect(enterReason(false)).toBe("next");
  });
});

describe("handBack", () => {
  it("is true only when the page had the keys, nothing was pressed, nothing covers it and the document has focus", () => {
    for (let n = 0; n < 16; n++) {
      const s = { pageHadKeys: !!(n & 1), pointerSince: !!(n & 2), covered: !!(n & 4), docHasFocus: !!(n & 8) };
      expect(handBack(s)).toBe(s.pageHadKeys && !s.pointerSince && !s.covered && s.docHasFocus);
    }
    expect(handBack({ pageHadKeys: true, pointerSince: false, covered: false, docHasFocus: true })).toBe(true);
  });
});

describe("keyForCode", () => {
  it("names the character a physical key types, as far as the keymap needs it", () => {
    expect(keyForCode("KeyK", false)).toBe("k");
    expect(keyForCode("KeyK", true)).toBe("K");
    expect(keyForCode("Digit5", false)).toBe("5");
    expect(keyForCode("Tab", false)).toBe("Tab");
    expect(keyForCode("ArrowLeft", true)).toBe("ArrowLeft");
    expect(keyForCode("Backquote", false)).toBe("`");
    expect(keyForCode("Backquote", true)).toBe("~");
    expect(keyForCode("F12", false)).toBe("F12");
    expect(keyForCode("Space", false)).toBe(" ");
  });
});

describe("onDragArea", () => {
  const el = (attr: string | null, deep: boolean, control: boolean) => ({
    getAttribute: (n: string) => (n === "data-tauri-drag-region" ? attr : null),
    closest: (s: string) => (s.includes("deep") ? (deep ? {} : null) : control ? {} : null),
  });
  it("is the title bar's own area, not a button on it", () => {
    expect(onDragArea(el("", false, false))).toBe(true);
    expect(onDragArea(el("false", false, false))).toBe(false);
    expect(onDragArea(el(null, true, false))).toBe(true);
    expect(onDragArea(el(null, true, true))).toBe(false);
    expect(onDragArea(el(null, false, false))).toBe(false);
  });
});

describe("quietly", () => {
  it("is on only while the callback runs, and off after a throw", () => {
    const seen: boolean[] = [];
    quietly(() => seen.push(isQuiet()));
    seen.push(isQuiet());
    expect(seen).toEqual([true, false]);
    expect(() =>
      quietly(() => {
        throw new Error("x");
      }),
    ).toThrow();
    expect(isQuiet()).toBe(false);
  });
});

describe("windowFocused", () => {
  afterEach(() => dispatchPage({ type: "gone" }));
  it("is true while a page has the keys, though the document says it hasn't focus", () => {
    const doc = { hasFocus: () => false };
    expect(windowFocused(doc)).toBe(false);
    dispatchPage({ type: "focus", key: "s/p", focused: true });
    expect(windowFocused(doc)).toBe(true);
    expect(windowFocused({ hasFocus: () => true })).toBe(true);
  });
});
