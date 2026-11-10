import { describe, expect, it } from "vitest";
import { draggedWidth, paneGeometry } from "./paneLayout";

describe("paneGeometry", () => {
  it("a hidden pane leaves the room to the terminal", () => {
    expect(paneGeometry({ room: 1000, shown: false, saved: 500, terminalMin: 600 })).toMatchObject({ mode: "hidden", terminal: 1000, pane: 0 });
  });
  it("with nothing saved the pane takes half the room", () => {
    expect(paneGeometry({ room: 1201, shown: true, saved: null, terminalMin: 600 })).toEqual({ mode: "beside", terminal: 600, pane: 600, max: 600 });
  });
  it("the pane stops where the terminal would drop under 80 columns", () => {
    expect(paneGeometry({ room: 1201, shown: true, saved: 900, terminalMin: 700 })).toMatchObject({ pane: 500, max: 500 });
  });
  it("a saved width wider than the room is shown clamped", () => {
    expect(paneGeometry({ room: 900, shown: true, saved: 2000, terminalMin: 400 }).pane).toBe(499);
  });
  it("the terminal goes under 80 columns only when the pane is at its minimum", () => {
    expect(paneGeometry({ room: 900, shown: true, saved: null, terminalMin: 700 })).toMatchObject({ pane: 320, terminal: 579 });
  });
  it("under 720 px the pane covers", () => {
    expect(paneGeometry({ room: 719, shown: true, saved: null, terminalMin: 0 })).toMatchObject({ mode: "cover", pane: 719, terminal: 0 });
    expect(paneGeometry({ room: 720, shown: true, saved: null, terminalMin: 0 }).mode).toBe("beside");
  });
  it("a room that is not a number gives zeros, not NaN", () => {
    expect(paneGeometry({ room: Number.NaN, shown: true, saved: null, terminalMin: 0 })).toMatchObject({ mode: "hidden", terminal: 0, pane: 0 });
    expect(paneGeometry({ room: -5, shown: true, saved: null, terminalMin: 0 })).toMatchObject({ mode: "hidden", terminal: 0, pane: 0 });
  });
});

describe("draggedWidth", () => {
  it("a drag is held between the minimum and the 80-column stop", () => {
    const i = { room: 1201, terminalMin: 700 };
    expect(draggedWidth({ ...i, fromEnd: 100 })).toBe(320);
    expect(draggedWidth({ ...i, fromEnd: 5000 })).toBe(500);
  });
});
