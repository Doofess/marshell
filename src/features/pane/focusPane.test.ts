import { describe, expect, it } from "vitest";
import { paneFocusTarget } from "./focusPane";

// A stand-in for the document: it answers a selector by what it names.
const root = (has: { tab?: string; editor?: boolean; body?: boolean; slot?: boolean; empty?: boolean; plus?: boolean }) =>
  ({
    querySelector: (sel: string) => {
      if (sel.includes("pane-tab-")) return has.tab !== undefined && sel.includes(`"pane-tab-${has.tab}"`) ? "tab" : null;
      if (sel.includes("cm-content")) return has.editor ? "editor" : null;
      if (sel.includes("preview__body")) return has.body ? "body" : null;
      if (sel.includes("browser-tab__slot")) return has.slot ? "slot" : null;
      if (sel.includes("empty-pane__button")) return has.empty ? "empty" : null;
      if (sel.includes("New tab")) return has.plus ? "plus" : null;
      return null;
    },
  }) as unknown as ParentNode;

describe("paneFocusTarget", () => {
  it("is the selected tab's body first", () => {
    expect(paneFocusTarget(root({ body: true, empty: true, plus: true }))).toBe("body");
  });
  it("is the editor of a text tab that shows one, so the keyboard comes back to the text and its cursor", () => {
    expect(paneFocusTarget(root({ editor: true, body: true, plus: true }))).toBe("editor");
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode);
    // Only the editor on screen: not one in a hidden panel, and not one behind the Rendered view.
    expect(seen[0]).toBe('[data-nav-side] [role="tabpanel"]:not([hidden]) [data-pane="editor"]:not([hidden]) .cm-content');
  });
  it("is a browser tab's slot, which enters the page, when the selected tab has no body", () => {
    expect(paneFocusTarget(root({ slot: true, empty: true, plus: true }))).toBe("slot");
    expect(paneFocusTarget(root({ body: true, slot: true }))).toBe("body");
  });
  it("is the empty pane's first button with no body", () => {
    expect(paneFocusTarget(root({ empty: true, plus: true }))).toBe("empty");
  });
  it("is nothing while a tab that is closing is still drawn, so focus doesn't land in an editor that is about to go", () => {
    // After a save the close is drawn a moment later than the frame focus is asked in.
    expect(paneFocusTarget(root({ tab: "t1", editor: true, body: true, plus: true }), "t1")).toBeNull();
    expect(paneFocusTarget(root({ tab: "t2", editor: true, body: true, plus: true }), "t1")).toBe("editor");
    expect(paneFocusTarget(root({ editor: true }), "t1")).toBe("editor");
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "t1");
    expect(seen[0]).toBe('[data-nav-side] [role="tab"][id="pane-tab-t1"]');
  });
  it("is the strip's New tab button when nothing else is there", () => {
    expect(paneFocusTarget(root({ plus: true }))).toBe("plus");
    expect(paneFocusTarget(root({}))).toBeNull();
  });
  it("looks for the body of the panel that isn't hidden, inside the side pane", () => {
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode);
    expect(seen[1]).toBe('[data-nav-side] [role="tabpanel"]:not([hidden]) .preview__body');
  });
});
