import { describe, expect, it } from "vitest";
import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import type { SessionTabs } from "../../generated/SessionTabs";
import { PANEL_COMMAND, PANEL_LABEL, activeIn, onScreen, panelOf, panelOfPath, panelTabs, panelsWithTabs, stackOf, stackPanels } from "./panels";

const tab = (id: string, kind: "file" | "browser", path: string): PaneTab => ({
  id,
  kind,
  history: [{ path, line: null, col: null, view: "rendered" }],
  index: 0,
  title: null,
  opened_by: null,
  unseen: false,
});
const file = (id: string, path = `/${id}.md`) => tab(id, "file", path);
const page = (id: string) => tab(id, "browser", `https://${id}.test/`);
const pane = (sessions: Record<string, SessionTabs>, extra: Partial<PaneView> = {}): PaneView => ({ open: true, width: null, sessions, recent: [], ...extra });

describe("panels", () => {
  it("a web page and an HTML file are in Browser, every other file in Files", () => {
    for (const p of ["https://a.test", "about:blank", "C:\\p\\index.html", "/p/A.HTM"]) expect(panelOfPath(p)).toBe("browser");
    for (const p of ["README.md", "a.pdf", "a.png", "x.htmlx"]) expect(panelOfPath(p)).toBe("files");
    expect(panelOf(page("a"))).toBe("browser");
    expect(panelOf(file("a", "/p/a.html"))).toBe("browser");
    expect(panelOf(file("a", "/p/a.md"))).toBe("files");
    const moved = { ...file("a", "/p/a.md"), history: [...file("a").history, ...file("a", "/p/b.html").history], index: 1 };
    expect(panelOf(moved)).toBe("browser");
  });

  it("the selection is per panel", () => {
    const tabs = [file("f1"), page("b1"), file("f2"), page("b2")];
    // Its own field when it names a tab of the panel.
    expect(activeIn({ tabs, active: "f2", active_browser: "b2" }, "files")).toBe("f2");
    expect(activeIn({ tabs, active: "f2", active_browser: "b2" }, "browser")).toBe("b2");
    // The other field when the own one doesn't (or is absent).
    expect(activeIn({ tabs, active: "b2" }, "browser")).toBe("b2");
    expect(activeIn({ tabs, active: "b2", active_browser: "f2" }, "files")).toBe("f2");
    expect(activeIn({ tabs, active: "b1", active_browser: "gone" }, "browser")).toBe("b1");
    // The panel's first tab otherwise.
    expect(activeIn({ tabs, active: "b2", active_browser: "b1" }, "files")).toBe("f1");
    expect(activeIn({ tabs, active: null }, "browser")).toBe("b1");
    // Null for a panel with no tab.
    expect(activeIn({ tabs: [file("f1")], active: "f1" }, "browser")).toBeNull();
    expect(activeIn({ tabs: [], active: null }, "files")).toBeNull();
  });

  it("the old active tab is the selection of whichever panel holds it", () => {
    const st: SessionTabs = { tabs: [file("t1"), page("t2")], active: "t2" };
    expect(activeIn(st, "browser")).toBe("t2");
    expect(activeIn(st, "files")).toBe("t1");
  });

  it("a layout with no panels shows the panels that hold tabs", () => {
    const p = pane({
      both: { tabs: [file("t1"), page("t2")], active: "t1" },
      web: { tabs: [page("t1")], active: "t1" },
      docs: { tabs: [file("t1"), file("t2", "/a.html")], active: "t1" },
      none: { tabs: [], active: null },
    });
    expect(stackOf(p, "both")).toEqual({ shown: ["browser", "files"], weights: {}, expanded: null });
    expect(stackOf(p, "web")).toEqual({ shown: ["browser"], weights: {}, expanded: null });
    expect(stackOf(p, "docs")).toEqual({ shown: ["browser", "files"], weights: {}, expanded: null });
    expect(stackOf(p, "none").shown).toEqual(["files"]);
    expect(stackOf(p, "missing").shown).toEqual(["files"]);
    expect(stackOf(p, null)).toEqual({ shown: ["files"], weights: {}, expanded: null });
    expect(panelsWithTabs({ tabs: [page("a")], active: "a" })).toEqual(["browser"]);
    // A saved stack is returned cleaned, whatever the session holds.
    const saved = pane({ web: { tabs: [page("t1")], active: "t1" } }, { panels: { shown: ["files", "changes", "browser", "files"], weights: { files: 5, nope: 3 } } });
    expect(stackOf(saved, "web")).toEqual({ shown: ["browser", "files", "changes"], weights: { files: 5 }, expanded: null });
  });

  it("the stack is the window's: another session shows the same panels", () => {
    const p = pane({ a: { tabs: [page("t1")], active: "t1" }, b: { tabs: [file("t1")], active: "t1" } }, { panels: { shown: ["browser", "files"], weights: { browser: 7000 } } });
    expect(stackOf(p, "a")).toEqual(stackOf(p, "b"));
    expect(stackOf(p, "b").shown).toEqual(["browser", "files"]);
  });

  it("onScreen is the session's expanded panel alone, nothing while the pane is hidden", () => {
    const p = pane({ s: { tabs: [], active: null, expanded_panel: "browser" } }, { panels: { shown: ["browser", "files"], weights: {} } });
    expect(onScreen(p, "s")).toEqual(["browser"]);
    expect(stackPanels(stackOf(p, "s"))).toEqual(["browser"]);
    expect(onScreen({ ...p, open: false }, "s")).toEqual([]);
    expect(stackPanels(stackOf({ ...p, open: false }, "s"))).toEqual(["browser"]);
  });

  it("stacks Browser, Files, Changes in that order", () => {
    const p = pane({}, { panels: { shown: ["changes", "browser", "files"], weights: {} } });
    expect(stackPanels(stackOf(p, "s"))).toEqual(["browser", "files", "changes"]);
  });
  it("keeps an expansion with three panels shown", () => {
    const p = pane({ s: { tabs: [], active: null, expanded_panel: "changes" } }, { panels: { shown: ["browser", "files", "changes"], weights: {} } });
    expect(stackOf(p, "s").expanded).toBe("changes");
    expect(stackPanels(stackOf(p, "s"))).toEqual(["changes"]);
  });
  it("keeps an expansion with one panel shown", () => {
    const p = pane({ s: { tabs: [], active: null, expanded_panel: "changes" } }, { panels: { shown: ["changes"], weights: {} } });
    expect(stackOf(p, "s").expanded).toBe("changes");
  });
  it("the expansion is the session's: another session sees every shown panel", () => {
    const p = pane({ a: { tabs: [], active: null, expanded_panel: "files" } }, { panels: { shown: ["browser", "files"], weights: {} } });
    expect(stackOf(p, "a").expanded).toBe("files");
    expect(stackOf(p, "b").expanded).toBeNull();
    expect(stackOf(p, null).expanded).toBeNull();
    expect(stackPanels(stackOf(p, "a"))).toEqual(["files"]);
    expect(stackPanels(stackOf(p, "b"))).toEqual(["browser", "files"]);
  });
  it("an expansion that names a panel not shown, an unknown panel, or comes with no saved stack is ignored", () => {
    const panels = { shown: ["files"], weights: {} };
    for (const expanded_panel of ["browser", "nope"]) {
      expect(stackOf(pane({ s: { tabs: [], active: null, expanded_panel } }, { panels }), "s").expanded).toBeNull();
    }
    expect(stackOf(pane({ s: { tabs: [file("t1")], active: "t1", expanded_panel: "files" } }), "s").expanded).toBeNull();
  });
  it("an older layout's window-wide fill is ignored", () => {
    const p = pane({}, { panels: { shown: ["browser", "files"], weights: {}, filled: "files" } as PaneView["panels"] });
    expect(stackOf(p, "s")).toEqual({ shown: ["browser", "files"], weights: {}, expanded: null });
  });
  it("Changes holds no tabs and has no selected tab", () => {
    const st = { tabs: [file("f1"), page("b1")], active: "f1" };
    expect(panelTabs(pane({ s: st }), "s", "changes")).toEqual([]);
    expect(activeIn(st, "changes")).toBeNull();
    expect(panelsWithTabs(st)).toEqual(["browser", "files"]);
    expect(PANEL_COMMAND.changes).toBe("changes-panel");
    expect(PANEL_LABEL.changes).toBe("Changes");
  });
});
