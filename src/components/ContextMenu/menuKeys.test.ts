import { describe, expect, it } from "vitest";
import { inMenu, leavesMenu, menuKey, placeMenu, type MenuItem } from "./menuKeys";

const items: MenuItem[] = [
  { id: "a", label: "Rename" },
  { id: "b", label: "Resume" },
  { separator: true },
  { id: "c", label: "Close" },
];

describe("menuKey", () => {
  it("moves down past the separator and wraps", () => {
    expect(menuKey("ArrowDown", items, 0)).toEqual({ kind: "move", index: 1 });
    expect(menuKey("ArrowDown", items, 1)).toEqual({ kind: "move", index: 3 });
    expect(menuKey("ArrowDown", items, 3)).toEqual({ kind: "move", index: 0 });
  });
  it("moves up and wraps", () => {
    expect(menuKey("ArrowUp", items, 0)).toEqual({ kind: "move", index: 3 });
    expect(menuKey("ArrowUp", items, 3)).toEqual({ kind: "move", index: 1 });
  });
  it("jumps to the ends", () => {
    expect(menuKey("Home", items, 3)).toEqual({ kind: "move", index: 0 });
    expect(menuKey("End", items, 0)).toEqual({ kind: "move", index: 3 });
  });
  it("runs the active item on Enter and Space", () => {
    expect(menuKey("Enter", items, 1)).toEqual({ kind: "run", id: "b" });
    expect(menuKey(" ", items, 1)).toEqual({ kind: "run", id: "b" });
  });
  it("closes on Escape with a refocus, and on Tab without", () => {
    expect(menuKey("Escape", items, 0)).toEqual({ kind: "close", refocus: true });
    expect(menuKey("Tab", items, 0)).toEqual({ kind: "close", refocus: false });
  });
  it("jumps to the next item starting with the typed letter, wrapping, ignoring case", () => {
    expect(menuKey("c", items, 0)).toEqual({ kind: "move", index: 3 });
    expect(menuKey("r", items, 0)).toEqual({ kind: "move", index: 1 });
    expect(menuKey("R", items, 1)).toEqual({ kind: "move", index: 0 });
  });
  it("ignores keys it has no use for", () => {
    expect(menuKey("x", items, 0)).toBeNull();
    expect(menuKey("F5", items, 0)).toBeNull();
  });
});

describe("placeMenu", () => {
  const size = { w: 200, h: 100 };
  const vp = { w: 800, h: 600 };
  it("opens at the anchor when it fits", () => {
    expect(placeMenu({ x: 10, y: 10 }, size, vp)).toEqual({ x: 10, y: 10 });
  });
  it("pulls the menu back from the right and bottom edges", () => {
    expect(placeMenu({ x: 700, y: 10 }, size, vp).x).toBe(592);
    expect(placeMenu({ x: 10, y: 580 }, size, vp).y).toBe(492);
  });
  it("keeps the margin when the menu is larger than the viewport", () => {
    expect(placeMenu({ x: 300, y: 300 }, { w: 900, h: 700 }, vp)).toEqual({ x: 8, y: 8 });
  });
});

describe("menuKey with modifiers held", () => {
  it("type-ahead ignores a letter pressed with Ctrl, Alt or Meta (an app shortcut, not a search)", () => {
    expect(menuKey("c", items, 0, { ctrlKey: true })).toBeNull();
    expect(menuKey("c", items, 0, { altKey: true })).toBeNull();
    expect(menuKey("c", items, 0, { metaKey: true })).toBeNull();
  });
  it("still searches with Shift or no modifier", () => {
    expect(menuKey("C", items, 0, { ctrlKey: false, altKey: false, metaKey: false })).toEqual({ kind: "move", index: 3 });
  });
});

describe("inMenu", () => {
  it("is true for a target inside a menu, false for anything else", () => {
    expect(inMenu({ closest: (s: string) => (s === '[role="menu"]' ? {} : null) })).toBe(true);
    expect(inMenu({ closest: () => null })).toBe(false);
    expect(inMenu({})).toBe(false);
    expect(inMenu(null)).toBe(false);
  });
});

describe("leavesMenu", () => {
  const menu = { contains: (n: object) => n === inside };
  const inside = {};
  it("is true when focus lands on something outside the menu", () => {
    expect(leavesMenu({}, menu)).toBe(true);
  });
  it("is false while focus moves between the menu's own items", () => {
    expect(leavesMenu(inside, menu)).toBe(false);
  });
  it("is false when focus goes to nothing (the window lost focus, or the item unmounted)", () => {
    expect(leavesMenu(null, menu)).toBe(false);
  });
});
