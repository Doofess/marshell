import { describe, expect, it } from "vitest";
import { moveCursor } from "./spatial";
import type { NavLayout, Rect, Target } from "./types";
import { GRID_LAYOUT } from "./fixtures";

const rectOf = (layout: NavLayout, t: Target): Rect => layout.find((l) => JSON.stringify(l.target) === JSON.stringify(t))!.rect;
const go = (from: Target, dir: "left" | "right" | "up" | "down", layout: NavLayout = GRID_LAYOUT) => moveCursor(rectOf(layout, from), from, dir, layout);

describe("moveCursor: arrows move to the nearest thing in that direction", () => {
  it("steps down a list of sessions and back up", () => {
    expect(go({ kind: "row", number: 1 }, "down")).toEqual({ kind: "row", number: 2 });
    expect(go({ kind: "row", number: 3 }, "down")).toEqual({ kind: "row", number: 5 });
    expect(go({ kind: "row", number: 5 }, "up")).toEqual({ kind: "row", number: 3 });
  });
  it("crosses from a pane into the sidebar list, to the row level with its middle, and back (the window then prefers the pane's own row)", () => {
    expect(go({ kind: "pane", index: 0 }, "left")).toEqual({ kind: "row", number: 3 });
    expect(go({ kind: "pane", index: 2 }, "left")).toEqual({ kind: "row", number: 6 });
    expect(go({ kind: "row", number: 1 }, "right")).toEqual({ kind: "pane", index: 0 });
    expect(go({ kind: "row", number: 6 }, "right")).toEqual({ kind: "pane", index: 0 });
  });
  it("moves around a grid of four panes", () => {
    expect(go({ kind: "pane", index: 0 }, "right")).toEqual({ kind: "pane", index: 1 });
    expect(go({ kind: "pane", index: 0 }, "down")).toEqual({ kind: "pane", index: 2 });
    expect(go({ kind: "pane", index: 3 }, "left")).toEqual({ kind: "pane", index: 2 });
    expect(go({ kind: "pane", index: 3 }, "up")).toEqual({ kind: "pane", index: 1 });
  });
  it("reaches the top bar from above, and comes back down onto what is below it", () => {
    expect(go({ kind: "pane", index: 1 }, "up")).toEqual({ kind: "top", id: "layout" });
    expect(go({ kind: "row", number: 1 }, "up")).toEqual({ kind: "top", id: "palette" });
    expect(go({ kind: "pane", index: 0 }, "up")).toEqual({ kind: "top", id: "palette" });
    expect(go({ kind: "top", id: "brand" }, "down")).toEqual({ kind: "row", number: 1 });
    expect(go({ kind: "top", id: "layout" }, "down")).toEqual({ kind: "pane", index: 1 });
    expect(go({ kind: "top", id: "brand" }, "right")).toEqual({ kind: "top", id: "palette" });
  });
  it("stays put at an edge: there is nothing further, so it answers null", () => {
    expect(go({ kind: "pane", index: 3 }, "right")).toBeNull();
    expect(go({ kind: "pane", index: 3 }, "down")).toBeNull();
    expect(go({ kind: "top", id: "brand" }, "up")).toBeNull();
    expect(go({ kind: "row", number: 6 }, "down")).toBeNull();
    expect(go({ kind: "row", number: 1 }, "left")).toBeNull();
  });
  it("copes with an empty layout and with a target that is not in it", () => {
    expect(moveCursor({ x: 0, y: 0, w: 10, h: 10 }, { kind: "pane", index: 0 }, "right", [])).toBeNull();
    expect(go({ kind: "pane", index: 0 }, "right", [{ target: { kind: "pane", index: 0 }, rect: { x: 0, y: 0, w: 5, h: 5 } }])).toBeNull();
  });
  it("prefers what is straight ahead over something nearer but off to the side", () => {
    const layout: NavLayout = [
      { target: { kind: "pane", index: 0 }, rect: { x: 0, y: 0, w: 100, h: 100 } },
      { target: { kind: "pane", index: 1 }, rect: { x: 110, y: 0, w: 100, h: 100 } },
      { target: { kind: "pane", index: 2 }, rect: { x: 104, y: 400, w: 100, h: 100 } },
    ];
    expect(go({ kind: "pane", index: 0 }, "right", layout)).toEqual({ kind: "pane", index: 1 });
  });
});
