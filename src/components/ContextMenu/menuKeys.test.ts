import { describe, expect, it } from "vitest";
import { menuKey, placeMenu, type MenuItem } from "./menuKeys";

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
