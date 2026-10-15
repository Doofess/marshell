import { describe, expect, it } from "vitest";
import { focusFellOut, listNavKeys, stepActive } from "./launcherKeys";

describe("launcherKeys: which keys move the highlighted option", () => {
  it("in the folder field only Up and Down do: Left and Right move the caret, so a typo can be fixed", () => {
    expect(listNavKeys("folder")).toEqual(["ArrowDown", "ArrowUp"]);
    expect(listNavKeys("folder")).not.toContain("ArrowLeft");
    expect(listNavKeys("folder")).not.toContain("ArrowRight");
  });
  it("in the program grid all four arrows do", () => {
    expect([...listNavKeys("programs")].sort()).toEqual(["ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp"]);
  });
  it("with no list open, none do", () => {
    expect(listNavKeys(null)).toEqual([]);
  });
});

describe("launcherKeys: stepping the highlighted option", () => {
  it("moves forward on Down and Right, back on Up and Left, and wraps", () => {
    expect(stepActive(0, "ArrowDown", 3)).toBe(1);
    expect(stepActive(2, "ArrowRight", 3)).toBe(0);
    expect(stepActive(0, "ArrowUp", 3)).toBe(2);
    expect(stepActive(1, "ArrowLeft", 3)).toBe(0);
  });
  it("stays at 0 for an empty list, and recovers from a stale index", () => {
    expect(stepActive(5, "ArrowDown", 0)).toBe(0);
    expect(stepActive(99, "ArrowDown", 3)).toBe(0); // clamped to the last row, then one step on wraps
    expect(stepActive(-4, "ArrowUp", 3)).toBe(2);
  });
});

describe("launcherKeys: focus when the folder field closes", () => {
  it("goes back to the folder chip when the field closed and focus fell to the page", () => {
    expect(focusFellOut({ wasOpen: true, isOpen: false, activeIsBody: true })).toBe(true);
  });
  it("is left alone when focus is somewhere real, when the field is still open, or when it was never open", () => {
    expect(focusFellOut({ wasOpen: true, isOpen: false, activeIsBody: false })).toBe(false);
    expect(focusFellOut({ wasOpen: true, isOpen: true, activeIsBody: true })).toBe(false);
    expect(focusFellOut({ wasOpen: false, isOpen: false, activeIsBody: true })).toBe(false);
  });
});
