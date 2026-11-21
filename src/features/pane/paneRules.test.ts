import { describe, expect, it } from "vitest";
import type { PaneView } from "../../generated/PaneView";
import { paneProblem, treeFolderOk } from "./paneRules";

const withTree = (expanded: string[]): PaneView => ({
  open: true,
  width: null,
  sessions: { s: { tabs: [], active: null, tree: { root: "C:/p", expanded } } },
  recent: [],
});

describe("paneProblem and the file tree", () => {
  it("names the tree's field validate_pane refuses", () => {
    expect(paneProblem(withTree(Array.from({ length: 501 }, (_, i) => `d${i}`)))).toBe("tree.expanded");
    expect(paneProblem(withTree(["a/../b"]))).toBe("tree.expanded");
    expect(paneProblem({ ...withTree([]), tree: { hidden: false, width: 100 } })).toBe("tree.width");
  });

  it("accepts a session with a tree and no tabs, and a width in range", () => {
    expect(paneProblem(withTree(["src", "src/a"]))).toBeNull();
    expect(paneProblem({ ...withTree([]), tree: { hidden: true, width: 160 } })).toBeNull();
    expect(paneProblem({ ...withTree([]), tree: { hidden: true, width: 2048 } })).toBeNull();
  });

  it("treeFolderOk keeps relative folders with / separators only", () => {
    for (const ok of ["src", "src/a"]) expect(treeFolderOk(ok)).toBe(true);
    for (const bad of ["", "/abs", "a/../b", "a\\b", "C:/x", "a//b", "./a", "a/"]) expect(treeFolderOk(bad)).toBe(false);
  });
});
