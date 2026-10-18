import { describe, expect, it } from "vitest";
import type { Hit, Project } from "./filter";
import { browseKey, folderEnter, folderRows, rowEnabled, typedPath, type PathCheck } from "./folderRows";

const hit = (id: string): Hit<Project> => ({ item: { id, name: id, path: id, lastUsedMs: 0 }, ranges: [] });
const found: PathCheck = { path: "C:\\dev", state: "found", folder: { path: "C:\\dev", name: "dev", branch: null } };

describe("folderRows: which typed text is a path to check", () => {
  it("takes text that starts like a path in either OS's syntax, or with ~", () => {
    for (const p of ["C:\\dev", "c:/dev", "\\\\srv\\share", "/usr", "~", "~/x"]) expect(typedPath(p)).toBe(p);
  });
  it("strips surrounding space and one pair of pasted quotes", () => {
    expect(typedPath("  C:\\x  ")).toBe("C:\\x");
    expect(typedPath('"C:\\a b"')).toBe("C:\\a b");
    expect(typedPath("'/a b'")).toBe("/a b");
  });
  it("leaves a folder name, empty text and a lone quote to the filter", () => {
    for (const q of ["dev", "my-app", "", "   ", '"dev"', '"', "a:"]) expect(typedPath(q)).toBeNull();
  });
});

describe("folderRows: the rows of the folder list", () => {
  it("lists the matching folders, then Browse… last", () => {
    expect(folderRows(null, [hit("a"), hit("b")]).map((r) => r.kind)).toEqual(["project", "project", "browse"]);
  });
  it("puts the typed path first, so Enter takes it", () => {
    expect(folderRows(found, [hit("a")]).map((r) => r.kind)).toEqual(["path", "project", "browse"]);
  });
  it("keeps Browse… when nothing matches", () => {
    expect(folderRows(null, [])).toEqual([{ kind: "browse" }]);
  });
});

describe("folderRows: rows Enter can't take", () => {
  it("are a path still being checked, or one with no folder there", () => {
    expect(rowEnabled({ kind: "path", check: { path: "C:\\de", state: "checking" } })).toBe(false);
    expect(rowEnabled({ kind: "path", check: { path: "C:\\nope", state: "missing" } })).toBe(false);
  });
  it("everything else can be taken", () => {
    expect(rowEnabled({ kind: "path", check: found })).toBe(true);
    expect(rowEnabled({ kind: "project", hit: hit("a") })).toBe(true);
    expect(rowEnabled({ kind: "browse" })).toBe(true);
  });
});

describe("folderRows: what Enter does on a row", () => {
  it("uses a found path, and picks a recent folder or opens Browse…", () => {
    expect(folderEnter({ kind: "path", check: found })).toEqual({ kind: "use", folder: { path: "C:\\dev", name: "dev", branch: null } });
    expect(folderEnter({ kind: "project", hit: hit("a") })).toEqual({ kind: "pick", id: "a" });
    expect(folderEnter({ kind: "browse" })).toEqual({ kind: "browse" });
  });
  it("does nothing on a path still being checked, a missing one, or no row", () => {
    expect(folderEnter({ kind: "path", check: { path: "C:\\de", state: "checking" } })).toBeNull();
    expect(folderEnter({ kind: "path", check: { path: "C:\\nope", state: "missing" } })).toBeNull();
    expect(folderEnter(undefined)).toBeNull();
  });
});

describe("browseKey", () => {
  const key = { code: "KeyO", ctrlKey: true, shiftKey: false, altKey: false, metaKey: false };
  it("is Ctrl+O by the physical key, whatever the layout types", () => {
    expect(browseKey({ ...key, key: "ם" } as typeof key)).toBe(true);
  });
  it("is not Ctrl+Shift+O, Alt, Meta, a bare O, or another key", () => {
    expect(browseKey({ ...key, shiftKey: true })).toBe(false);
    expect(browseKey({ ...key, altKey: true })).toBe(false);
    expect(browseKey({ ...key, metaKey: true })).toBe(false);
    expect(browseKey({ ...key, ctrlKey: false })).toBe(false);
    expect(browseKey({ ...key, code: "KeyP" })).toBe(false);
  });
});

describe("a failed check", () => {
  const failed: PathCheck = { path: "C:\dev", state: "failed" };
  it("can't be taken and does nothing on Enter", () => {
    expect(rowEnabled({ kind: "path", check: failed })).toBe(false);
    expect(folderEnter({ kind: "path", check: failed })).toBeNull();
  });
});
