import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PaneTab } from "../../generated/PaneTab";
import { KeptTabs } from "./KeptTabs";

const tab = (id: string): PaneTab => ({ id, kind: "file", history: [{ path: `/p/${id}.md`, line: null, col: null, view: "rendered" }], index: 0, title: null, opened_by: null, unseen: false });
const tabs = ["t1", "t2", "t3", "t4", "t5", "t6"].map(tab);
const render = (t: PaneTab) => <i data-body={t.id} />;

/** Renders with one shared list, as successive renders of one component would. */
function show(keptRef: { current: string[] }, activeId: string, list = tabs) {
  return renderToStaticMarkup(<KeptTabs tabs={list} activeId={activeId} render={render} keptRef={keptRef} />);
}

describe("KeptTabs", () => {
  it("after rendering active t1, then t2, holds both panels and hides t1's", () => {
    const keptRef = { current: [] as string[] };
    show(keptRef, "t1");
    const out = show(keptRef, "t2");
    expect(out).toContain('data-body="t1"');
    expect(out).toContain('data-body="t2"');
    const p1 = out.match(/<div[^>]*id="pane-panel-t1"[^>]*>/)![0];
    const p2 = out.match(/<div[^>]*id="pane-panel-t2"[^>]*>/)![0];
    expect(p1).toContain("hidden");
    expect(p2).not.toContain("hidden");
  });
  it("draws one panel for a file two kept tabs show: the selected tab's", () => {
    const twice = [tab("t1"), { ...tab("t2"), history: tab("t1").history }, tab("t3")];
    const keptRef = { current: [] as string[] };
    show(keptRef, "t1", twice);
    const out = show(keptRef, "t2", twice);
    expect(out).toContain('data-body="t2"');
    expect(out).not.toContain('data-body="t1"');
    const back = show(keptRef, "t1", twice);
    expect(back).toContain('data-body="t1"');
    expect(back).not.toContain('data-body="t2"');
  });
  it("gives each panel its role, id and label", () => {
    const out = show({ current: [] }, "t1");
    expect(out).toContain('role="tabpanel"');
    expect(out).toContain('id="pane-panel-t1"');
    expect(out).toContain('aria-labelledby="pane-tab-t1"');
  });
  it("keeps panels in the order of the tabs, whatever was shown last", () => {
    const keptRef = { current: [] as string[] };
    show(keptRef, "t2");
    const out = show(keptRef, "t1");
    expect(out.indexOf("pane-panel-t1")).toBeLessThan(out.indexOf("pane-panel-t2"));
  });
  it("a fifth tab pushes the oldest out", () => {
    const keptRef = { current: [] as string[] };
    let out = "";
    for (const id of ["t1", "t2", "t3", "t4", "t5"]) out = show(keptRef, id);
    expect(out).not.toContain('data-body="t1"');
    for (const id of ["t2", "t3", "t4", "t5"]) expect(out).toContain(`data-body="${id}"`);
  });
  it("two panels keep their own four tabs", () => {
    const browser = ["b1", "b2", "b3", "b4", "b5", "b6"].map(tab);
    const files = ["f1", "f2", "f3", "f4", "f5", "f6"].map(tab);
    const browserKept = { current: [] as string[] };
    const filesKept = { current: [] as string[] };
    for (const id of ["b1", "b2", "b3", "b4", "b5"]) show(browserKept, id, browser);
    for (const id of ["f1", "f2", "f3", "f4", "f5"]) show(filesKept, id, files);
    // Each panel pushes out its own oldest, and never the other panel's.
    expect([...browserKept.current].sort()).toEqual(["b2", "b3", "b4", "b5"]);
    expect([...filesKept.current].sort()).toEqual(["f2", "f3", "f4", "f5"]);
    const out = show(browserKept, "b5", browser);
    expect(out).not.toContain('data-body="b1"');
    expect(out).toContain('data-body="b2"');
    expect(out).not.toContain('data-body="f2"');
  });
});
