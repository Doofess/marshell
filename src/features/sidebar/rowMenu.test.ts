import { describe, expect, it } from "vitest";
import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import { inRenameBox, keyboardAnchor, menuStillValid, opensRowMenu, sameMenuItems } from "./rowMenu";

const key = (over: Partial<Parameters<typeof opensRowMenu>[0]>) => ({ key: "", shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, target: null, ...over });

describe("opensRowMenu", () => {
  it("opens on Shift+F10 and on the Menu key", () => {
    expect(opensRowMenu(key({ key: "F10", shiftKey: true }))).toBe(true);
    expect(opensRowMenu(key({ key: "ContextMenu" }))).toBe(true);
  });
  it("ignores F10 alone and F10 with other modifiers", () => {
    expect(opensRowMenu(key({ key: "F10" }))).toBe(false);
    expect(opensRowMenu(key({ key: "F10", shiftKey: true, ctrlKey: true }))).toBe(false);
  });
  it("leaves the key to the rename text box", () => {
    const target = { closest: (s: string) => (s === ".session-row__rename" ? {} : null) };
    expect(opensRowMenu(key({ key: "F10", shiftKey: true, target }))).toBe(false);
    expect(opensRowMenu(key({ key: "ContextMenu", target }))).toBe(false);
  });
});

describe("keyboardAnchor", () => {
  it("sits under the row, a little in from its left edge", () => {
    expect(keyboardAnchor({ left: 12, bottom: 100 })).toEqual({ x: 20, y: 100 });
  });
});

describe("menuStillValid", () => {
  it("holds only while the row is still listed", () => {
    expect(menuStillValid("a", ["a", "b"])).toBe(true);
    expect(menuStillValid("c", ["a", "b"])).toBe(false);
    expect(menuStillValid(null, [])).toBe(false);
  });
});

describe("inRenameBox", () => {
  it("is true for a target inside the rename box, so the row's contextmenu leaves it its native menu", () => {
    expect(inRenameBox({ closest: (s: string) => (s === ".session-row__rename" ? {} : null) })).toBe(true);
  });
  it("is false elsewhere in the row, and for no target", () => {
    expect(inRenameBox({ closest: () => null })).toBe(false);
    expect(inRenameBox({})).toBe(false);
    expect(inRenameBox(null)).toBe(false);
  });
});

describe("sameMenuItems", () => {
  const items = (...ids: string[]): MenuItem[] => ids.map((id) => (id === "-" ? { separator: true } : { id, label: id }));
  it("holds when the ids match in order, whatever the objects", () => {
    expect(sameMenuItems(items("rename", "resume", "-", "close"), items("rename", "resume", "-", "close"))).toBe(true);
  });
  it("fails when an item is gone, added or moved", () => {
    expect(sameMenuItems(items("rename", "resume", "close"), items("rename", "close"))).toBe(false);
    expect(sameMenuItems(items("rename", "close"), items("rename", "resume", "close"))).toBe(false);
    expect(sameMenuItems(items("rename", "split"), items("split", "rename"))).toBe(false);
  });
  it("tells a separator from an item", () => {
    expect(sameMenuItems(items("rename", "-"), items("rename", "close"))).toBe(false);
  });
});
