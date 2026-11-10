import { describe, expect, it } from "vitest";
import { paneFocusTarget } from "./focusPane";

// A stand-in for the document: it answers a selector by what it names.
const root = (has: { body?: boolean; empty?: boolean; plus?: boolean }) =>
  ({
    querySelector: (sel: string) => {
      if (sel.includes("preview__body")) return has.body ? "body" : null;
      if (sel.includes("empty-pane__button")) return has.empty ? "empty" : null;
      if (sel.includes("New tab")) return has.plus ? "plus" : null;
      return null;
    },
  }) as unknown as ParentNode;

describe("paneFocusTarget", () => {
  it("is the selected tab's body first", () => {
    expect(paneFocusTarget(root({ body: true, empty: true, plus: true }))).toBe("body");
  });
  it("is the empty pane's first button with no body", () => {
    expect(paneFocusTarget(root({ empty: true, plus: true }))).toBe("empty");
  });
  it("is the strip's New tab button when nothing else is there", () => {
    expect(paneFocusTarget(root({ plus: true }))).toBe("plus");
    expect(paneFocusTarget(root({}))).toBeNull();
  });
  it("looks for the body of the panel that isn't hidden, inside the side pane", () => {
    let seen = "";
    paneFocusTarget({ querySelector: (s: string) => ((seen ||= s), null) } as unknown as ParentNode);
    expect(seen).toBe('[data-nav-side] [role="tabpanel"]:not([hidden]) .preview__body');
  });
});
