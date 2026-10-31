import { describe, expect, it } from "vitest";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { addRow, forSave, firstProblem, patchRow, pickProject, removeRow, rowsOf } from "./projectsDoc";

describe("rows", () => {
  it("start from the core's aliases, keeping which paths are broken", () => {
    expect(rowsOf([{ name: "login", path: "src/a.ts", exists: false }])).toEqual([{ name: "login", path: "src/a.ts", exists: false }]);
  });
  it("add an empty row, remove one and rename one without touching the others", () => {
    const rows = addRow([{ name: "a", path: "x" }]);
    expect(rows).toEqual([{ name: "a", path: "x" }, { name: "", path: "" }]);
    expect(patchRow(rows, 1, { name: "b" })[1]).toEqual({ name: "b", path: "" });
    expect(removeRow(rows, 0)).toEqual([{ name: "", path: "" }]);
  });
  it("forget a row's broken mark once its path is edited", () => {
    expect(patchRow([{ name: "a", path: "x", exists: false }], 0, { path: "y" })[0]).toEqual({ name: "a", path: "y" });
    expect(patchRow([{ name: "a", path: "x", exists: false }], 0, { name: "b" })[0]!.exists).toBe(false);
  });
  it("save the trimmed name and path only", () => {
    expect(forSave([{ name: "  a ", path: " x/y ", exists: true }])).toEqual([{ name: "a", path: "x/y" }]);
  });
});

describe("firstProblem", () => {
  const ok = { name: "a", path: "x" };
  it("is null for a good list", () => {
    expect(firstProblem([ok, { name: "b", path: "y" }])).toBeNull();
  });
  it("says the core's sentences", () => {
    expect(firstProblem([{ name: " ", path: "x" }])).toBe("An alias needs a name.");
    expect(firstProblem([{ name: "a", path: " " }])).toBe("An alias needs a path.");
    expect(firstProblem([{ name: "a".repeat(41), path: "x" }])).toBe("A name can be at most 40 characters.");
    expect(firstProblem([ok, { name: "A ", path: "y" }])).toBe('Two aliases are named "A".');
    expect(firstProblem(Array.from({ length: 101 }, (_, i) => ({ name: `n${i}`, path: "x" })))).toBe("A project can have at most 100 aliases.");
  });
});

describe("pickProject", () => {
  const p = (path: string, name: string): ProjectInfo => ({ path, name, branch: null, last_used_ms: 1 });
  const list = [p("C:\\dev\\a", "a"), p("C:\\dev\\b", "b")];
  it("preselects the project holding the cwd, case and slash blind", () => {
    expect(pickProject(list, "c:/dev/b/src")).toEqual({ options: list, selected: "C:\\dev\\b" });
  });
  it("adds the cwd itself when no listed project holds it", () => {
    const r = pickProject(list, "C:\\other\\app");
    expect(r.selected).toBe("C:\\other\\app");
    expect(r.options[0]).toMatchObject({ path: "C:\\other\\app", name: "app" });
    expect(r.options).toHaveLength(3);
  });
  it("picks the first project with no cwd, and none with no projects", () => {
    expect(pickProject(list, undefined).selected).toBe("C:\\dev\\a");
    expect(pickProject([], undefined).selected).toBeNull();
  });
});
