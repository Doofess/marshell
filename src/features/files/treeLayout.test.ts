import { describe, expect, it } from "vitest";
import { TREE, dividerKey, narrowAfterOpen, treeSide, treeWidth } from "./treeLayout";

describe("treeLayout", () => {
  it("automatic puts the tree at the window's edge", () => {
    expect(treeSide("automatic", false)).toBe("right");
    expect(treeSide("automatic", true)).toBe("left");
    expect(treeSide("left", true)).toBe("left");
    expect(treeSide("right", true)).toBe("right");
    expect(treeSide("left", false)).toBe("left");
  });
  it("treeWidth keeps the file 240 px and the tree 160 px", () => {
    expect(treeWidth(null, 1000)).toBe(240);
    expect(treeWidth(undefined, 1000)).toBe(240);
    expect(treeWidth(900, 1000)).toBe(760);
    expect(treeWidth(100, 1000)).toBe(160);
    expect(treeWidth(5000, 4000)).toBe(2048);
    expect(treeWidth(240, 300)).toBe(160);
  });
  it("the divider moves 16 px, Home and End go to the limits, toward the file grows the tree", () => {
    expect(dividerKey("ArrowLeft", 240, 1000, "right")).toBe(256);
    expect(dividerKey("ArrowRight", 240, 1000, "right")).toBe(224);
    expect(dividerKey("ArrowRight", 240, 1000, "left")).toBe(256);
    expect(dividerKey("ArrowLeft", 240, 1000, "left")).toBe(224);
    expect(dividerKey("Home", 240, 1000, "left")).toBe(160);
    expect(dividerKey("End", 240, 1000, "left")).toBe(760);
    expect(dividerKey("ArrowLeft", 160, 1000, "left")).toBe(160);
    expect(dividerKey("a", 240, 1000, "left")).toBeNull();
    expect(TREE.step).toBe(16);
  });
  it("only Space leaves the tree over the file", () => {
    expect(narrowAfterOpen("click")).toBe(false);
    expect(narrowAfterOpen("enter")).toBe(false);
    expect(narrowAfterOpen("double")).toBe(false);
    expect(narrowAfterOpen("space")).toBe(true);
  });
});
