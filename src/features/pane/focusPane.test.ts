import { afterEach, describe, expect, it, vi } from "vitest";
import { askedFrom, focusPane, paneFocusTarget, panelOfElement } from "./focusPane";

// A stand-in for the document: it answers a selector by what it names.
const root = (has: { tab?: string; selected?: boolean; editor?: boolean; body?: boolean; slot?: boolean; row?: boolean; tree?: boolean; empty?: boolean; plus?: boolean; item?: boolean; title?: boolean }) =>
  ({
    querySelector: (sel: string) => {
      if (sel.includes("aria-selected")) return has.selected ? "selected tab" : null;
      if (sel.includes("pane-tab-")) return has.tab !== undefined && sel.includes(`"pane-tab-${has.tab}"`) ? "tab" : null;
      if (sel.includes("cm-content")) return has.editor ? "editor" : null;
      if (sel.includes("preview__body")) return has.body ? "body" : null;
      if (sel.includes("browser-tab__slot")) return has.slot ? "slot" : null;
      if (sel.includes("treeitem")) return has.row ? "row" : null;
      if (sel.includes('[role="tree"]')) return has.tree ? "tree" : null;
      if (sel.includes("empty-panel__button")) return has.empty ? "empty" : null;
      if (sel.includes("data-pane-plus")) return has.plus ? "plus" : null;
      if (sel.includes("data-changes-item")) return has.item ? "item" : null;
      if (sel.includes("empty-panel__title")) return has.title ? "title" : null;
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
  it("is the tree's tabbable row in Files with no tab, before the empty state's buttons", () => {
    expect(paneFocusTarget(root({ row: true, empty: true, plus: true }), "files")).toBe("row");
    expect(paneFocusTarget(root({ row: true, body: true }), "files")).toBe("body");
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "files");
    expect(seen).toContain('[data-nav-side="files"] [role="tree"] [role="treeitem"][tabindex="0"]');
  });
  it("waits while the Files tree is drawn without its rows, so the keyboard doesn't land on the empty state's button (live check 6r-2)", () => {
    // The rows come a frame or two after the tree, once it is measured, or after its first read.
    expect(paneFocusTarget(root({ tree: true, empty: true, plus: true }), "files", undefined, "pane", true)).toBeNull();
    expect(paneFocusTarget(root({ tree: true, row: true, empty: true }), "files", undefined, "pane", true)).toBe("row");
    expect(paneFocusTarget(root({ tree: true, body: true, empty: true }), "files", undefined, "pane", true)).toBe("body");
    // No tree (Browser, or a session with no folder), or the wait is over: the empty state.
    expect(paneFocusTarget(root({ empty: true, plus: true }), "files", undefined, "pane", true)).toBe("empty");
    expect(paneFocusTarget(root({ tree: true, empty: true }), "files", undefined, "pane", false)).toBe("empty");
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
  it("is Changes' list, else its empty state's button, else the title of an empty state that has no button", () => {
    expect(paneFocusTarget(root({ item: true, empty: true, title: true }), "changes")).toBe("item");
    expect(paneFocusTarget(root({ empty: true, title: true }), "changes")).toBe("empty");
    expect(paneFocusTarget(root({ title: true }), "changes")).toBe("title");
    const seen: string[] = [];
    paneFocusTarget({ querySelector: (s: string) => (seen.push(s), null) } as unknown as ParentNode, "changes");
    expect(seen).toContain('[data-nav-side="changes"] .empty-panel__title[tabindex="-1"]');
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

describe("focusPane", () => {
  afterEach(() => vi.unstubAllGlobals());
  /** Runs focusPane("files") over frames that each say whether the tree and its tabbable row are drawn; the last one repeats. */
  const run = (frames: { tree: boolean; row: boolean }[]) => {
    const queue: FrameRequestCallback[] = [];
    const focused: string[] = [];
    let frame = 0;
    const el = (name: string) => ({ dataset: {}, focus: () => void focused.push(name) });
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => (queue.push(cb), 0));
    vi.stubGlobal("document", {
      querySelector: (sel: string) => {
        const f = frames[Math.min(frame, frames.length - 1)] ?? { tree: false, row: false };
        if (sel.includes("treeitem")) return f.row ? el("row") : null;
        if (sel.includes('[role="tree"]')) return f.tree ? el("tree") : null;
        if (sel.includes("empty-panel__button")) return el("empty");
        return null;
      },
    });
    focusPane("files");
    while (queue.length > 0 && frame < 200) {
      queue.shift()?.(0);
      frame++;
    }
    return { focused, frames: frame };
  };
  it("waits for the tree's rows when they come after the tree, and gives them the keyboard", () => {
    expect(run([{ tree: true, row: false }, { tree: true, row: false }, { tree: true, row: true }]).focused).toEqual(["row"]);
    expect(run([...Array<{ tree: boolean; row: boolean }>(12).fill({ tree: true, row: false }), { tree: true, row: true }]).focused).toEqual(["row"]);
  });
  it("gives the empty state the keyboard when the rows never come, after about half a second of frames", () => {
    const r = run([{ tree: true, row: false }]);
    expect(r.focused).toEqual(["empty"]);
    expect(r.frames).toBeGreaterThan(3);
    expect(r.frames).toBeLessThanOrEqual(40);
  });
  it("gives the empty state the keyboard at once with no tree", () => {
    expect(run([{ tree: false, row: false }])).toEqual({ focused: ["empty"], frames: 1 });
  });
});

describe("focusPane in Changes", () => {
  afterEach(() => vi.unstubAllGlobals());
  /** Runs focusPane("changes") over frames that each say whether the panel is blank (no reply yet), its list is drawn, and where the keyboard is. */
  const run = (frames: { blank: boolean; item: boolean; active?: string }[]) => {
    const queue: FrameRequestCallback[] = [];
    const focused: string[] = [];
    let frame = 0;
    const at = () => frames[Math.min(frame, frames.length - 1)] ?? { blank: false, item: false };
    const el = (name: string) => ({ dataset: {}, focus: () => void focused.push(name) });
    const body = { name: "body" };
    const others = new Map<string, object>();
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => (queue.push(cb), 0));
    vi.stubGlobal("document", {
      body,
      get activeElement() {
        const name = at().active;
        if (name === undefined) return body;
        if (!others.has(name)) others.set(name, { name });
        return others.get(name);
      },
      querySelector: (sel: string) => {
        const f = at();
        if (sel.includes("data-changes-item")) return f.item ? el("item") : null;
        if (sel.includes("data-changes-blank")) return f.blank ? el("blank") : null;
        return null;
      },
    });
    focusPane("changes");
    while (queue.length > 0 && frame < 400) {
      queue.shift()?.(0);
      frame++;
    }
    return { focused, frames: frame };
  };
  it("waits for the first reply, which can take the check's 2 s, and gives the list the keyboard", () => {
    const blank = Array<{ blank: boolean; item: boolean }>(100).fill({ blank: true, item: false });
    expect(run([...blank, { blank: false, item: true }]).focused).toEqual(["item"]);
  });
  it("gives up after about 3 s of frames when no reply comes", () => {
    const r = run([{ blank: true, item: false }]);
    expect(r.focused).toEqual([]);
    expect(r.frames).toBeGreaterThan(150);
    expect(r.frames).toBeLessThanOrEqual(200);
  });
  it("leaves the keyboard alone when it went somewhere else while the panel waited", () => {
    const blank = Array<{ blank: boolean; item: boolean; active?: string }>(10).fill({ blank: true, item: false });
    const r = run([...blank, { blank: true, item: false, active: "terminal" }, ...blank.map((f) => ({ ...f, active: "terminal" })), { blank: false, item: true, active: "terminal" }]);
    expect(r.focused).toEqual([]);
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
