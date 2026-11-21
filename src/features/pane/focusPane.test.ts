import { describe, expect, it } from "vitest";
import { askedFrom, paneFocusTarget, panelOfElement } from "./focusPane";

// A stand-in for the document: it answers a selector by what it names.
const root = (has: { tab?: string; selected?: boolean; editor?: boolean; body?: boolean; slot?: boolean; empty?: boolean; plus?: boolean }) =>
  ({
    querySelector: (sel: string) => {
      if (sel.includes("aria-selected")) return has.selected ? "selected tab" : null;
      if (sel.includes("pane-tab-")) return has.tab !== undefined && sel.includes(`"pane-tab-${has.tab}"`) ? "tab" : null;
      if (sel.includes("cm-content")) return has.editor ? "editor" : null;
      if (sel.includes("preview__body")) return has.body ? "body" : null;
      if (sel.includes("browser-tab__slot")) return has.slot ? "slot" : null;
      if (sel.includes("empty-panel__button")) return has.empty ? "empty" : null;
      if (sel.includes("data-pane-plus")) return has.plus ? "plus" : null;
      return null;
    },
  }) as unknown as ParentNode;

describe("paneFocusTarget", () => {
  it("is the selected tab's body first", () => {
    expect(paneFocusTarget(root({ body: true, empty: true, plus: true }), "files")).toBe("body");
  });
  it("is the editor of a text tab that shows one, so the keyboard comes back to the text and its cursor", () => {
    expect(paneFocusTarget(root({ editor: true, body: true, plus: true }), "files")).toBe("editor");
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files");
    // Only the editor on screen: not one in a hidden panel, and not one behind the Rendered view.
    expect(seen[0]).toBe('[data-nav-side="files"] [role="tabpanel"]:not([hidden]) [data-pane="editor"]:not([hidden]) .cm-content');
  });
  it("is a browser tab's slot, which enters the page, when the selected tab has no body", () => {
    expect(paneFocusTarget(root({ slot: true, empty: true, plus: true }), "files")).toBe("slot");
    expect(paneFocusTarget(root({ body: true, slot: true }), "files")).toBe("body");
  });
  it("is the empty pane's first button with no body", () => {
    expect(paneFocusTarget(root({ empty: true, plus: true }), "files")).toBe("empty");
  });
  it("is nothing while a tab that is closing is still drawn, so focus doesn't land in an editor that is about to go", () => {
    // After a save the close is drawn a moment later than the frame focus is asked in.
    expect(paneFocusTarget(root({ tab: "t1", editor: true, body: true, plus: true }), "files", "t1")).toBeNull();
    expect(paneFocusTarget(root({ tab: "t2", editor: true, body: true, plus: true }), "files", "t1")).toBe("editor");
    expect(paneFocusTarget(root({ editor: true }), "files", "t1")).toBe("editor");
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files", "t1");
    expect(seen[0]).toBe('[data-nav-side="files"] [role="tab"][id="pane-tab-t1"]');
  });
  it("is the strip's selected tab for a close asked from the strip, so the next Delete closes a tab and edits no file", () => {
    expect(paneFocusTarget(root({ selected: true, editor: true, body: true, plus: true }), "files", "t1", "strip")).toBe("selected tab");
    expect(paneFocusTarget(root({ plus: true }), "files", "t1", "strip")).toBe("plus");
    // The tab that is closing is still drawn: wait, as for the pane.
    expect(paneFocusTarget(root({ tab: "t1", selected: true }), "files", "t1", "strip")).toBeNull();
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files", undefined, "strip");
    expect(seen[0]).toBe('[data-nav-side="files"] .side-pane__strip [role="tab"][aria-selected="true"]');
  });
  it("is the strip's New tab button when nothing else is there", () => {
    expect(paneFocusTarget(root({ plus: true }), "files")).toBe("plus");
    expect(paneFocusTarget(root({}), "files")).toBeNull();
  });
  it("the + button is found by data-pane-plus", () => {
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files");
    expect(seen.at(-1)).toBe('[data-nav-side="files"] button[data-pane-plus]');
  });
  it("the empty panel's first usable button takes the keyboard", () => {
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "browser");
    expect(seen).toContain('[data-nav-side="browser"] .empty-panel__button:not([aria-disabled="true"])');
    expect(paneFocusTarget(root({ empty: true, plus: true }), "browser")).toBe("empty");
  });
  it("a closing tab is looked for in its own panel", () => {
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "browser", "t9");
    expect(seen[0]).toBe('[data-nav-side="browser"] [role="tab"][id="pane-tab-t9"]');
  });
  it("looks for the body of the panel that isn't hidden, inside the side pane", () => {
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files");
    expect(seen[1]).toBe('[data-nav-side="files"] [role="tabpanel"]:not([hidden]) .preview__body');
  });
});

describe("panelOfElement", () => {
  const el = (value: string | null) => ({ closest: (sel: string) => (sel === "[data-nav-side]" && value !== null ? { getAttribute: (n: string) => (n === "data-nav-side" ? value : null) } : null) }) as unknown as Element;
  it("is the panel the element is inside", () => {
    expect(panelOfElement(el("browser"))).toBe("browser");
    expect(panelOfElement(el("files"))).toBe("files");
  });
  it("is null outside a panel, for no element, and for a value that is no panel", () => {
    expect(panelOfElement(el(null))).toBeNull();
    expect(panelOfElement(null)).toBeNull();
    expect(panelOfElement(el(""))).toBeNull();
  });
});

describe("askedFrom", () => {
  const el = (inside: string[]) => ({ closest: (sel: string) => (inside.includes(sel) ? {} : null) }) as unknown as Element;
  it("is the strip for the keyboard on a tab or its close button", () => {
    expect(askedFrom(el([".side-pane__strip", "[data-nav-side]"]))).toBe("strip");
  });
  it("is the pane for the keyboard anywhere else in the side pane", () => {
    expect(askedFrom(el(["[data-nav-side]"]))).toBe("pane");
  });
  it("is nowhere for the terminal or no focus at all, so a close from there moves no focus", () => {
    expect(askedFrom(el([]))).toBeNull();
    expect(askedFrom(null)).toBeNull();
  });
});
