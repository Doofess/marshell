import { describe, expect, it } from "vitest";
import { isAction, type MenuItem } from "../../components/ContextMenu/menuKeys";
import { treeMenuItems } from "./treeMenu";
import type { TreeRow } from "./treeModel";

const row = (over: Partial<TreeRow>): TreeRow => ({
  path: "src/main.ts",
  name: "main.ts",
  kind: "file",
  ignored: false,
  level: 1,
  expanded: false,
  loading: false,
  setSize: 1,
  posInSet: 1,
  ...over,
});
const labels = (items: MenuItem[]) => items.filter(isAction).map((i) => i.label);
const paste = { name: "api-server" };

describe("treeMenuItems", () => {
  it("a file's menu reads Open, Paste path into api-server, Copy path, Copy relative path, Show in File Explorer", () => {
    expect(labels(treeMenuItems(row({}), { pasteTo: paste, os: "windows" }))).toEqual([
      "Open",
      "Paste path into api-server",
      "Copy path",
      "Copy relative path",
      "Show in File Explorer",
    ]);
    expect(
      treeMenuItems(row({}), { pasteTo: paste, os: "windows" })
        .filter(isAction)
        .map((i) => i.id),
    ).toEqual(["open", "paste", "copy-path", "copy-relative", "reveal"]);
  });
  it("a folder's menu has no Open", () => {
    const l = labels(treeMenuItems(row({ kind: "folder", path: "src", name: "src" }), { pasteTo: paste, os: "windows" }));
    expect(l).not.toContain("Open");
    expect(l).toContain("Copy path");
  });
  it("a link's menu has no Open: a link to a folder is never opened", () => {
    const l = labels(treeMenuItems(row({ kind: "link", path: "current", name: "current" }), { pasteTo: paste, os: "windows" }));
    expect(l).not.toContain("Open");
    expect(l).toContain("Copy path");
  });
  it("the paste row is off with the session's reason", () => {
    const items = treeMenuItems(row({}), { pasteTo: { name: "api-server", refused: "The session has ended." }, os: "windows" }).filter(isAction);
    expect(items.find((i) => i.id === "paste")?.disabled).toBe("The session has ended.");
    expect(items.find((i) => i.id === "open")?.disabled).toBeUndefined();
  });
  it("has no paste row when there is no session to paste into", () => {
    expect(
      treeMenuItems(row({}), { pasteTo: null, os: "windows" })
        .filter(isAction)
        .map((i) => i.id),
    ).not.toContain("paste");
  });
  it("macOS says Show in Finder and Linux Show in file manager", () => {
    expect(labels(treeMenuItems(row({}), { pasteTo: null, os: "mac" })).at(-1)).toBe("Show in Finder");
    expect(labels(treeMenuItems(row({}), { pasteTo: null, os: "linux" })).at(-1)).toBe("Show in file manager");
  });
  it("a more or empty row has no menu", () => {
    expect(treeMenuItems(row({ kind: "more" }), { pasteTo: paste, os: "windows" })).toEqual([]);
    expect(treeMenuItems(row({ kind: "empty" }), { pasteTo: paste, os: "windows" })).toEqual([]);
  });
});
