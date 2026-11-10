import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PaneTab } from "../../generated/PaneTab";
import { KeptTabs } from "./KeptTabs";

const tab = (id: string): PaneTab => ({ id, kind: "file", history: [{ path: `/p/${id}.md`, line: null, col: null, view: "rendered" }], index: 0, title: null });
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
});
