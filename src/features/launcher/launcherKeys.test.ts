import { describe, expect, it } from "vitest";
import { adminKey, adminKeyClaimed, focusFellOut, listNavKeys, openFolder, openPrograms, stepActive } from "./launcherKeys";

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

describe("launcherKeys: one list open at a time", () => {
  it("opening the program list closes the folder field, so the program chip works while choosing a folder", () => {
    expect(openPrograms({ query: "air", programsOpen: false }, true)).toEqual({ query: null, programsOpen: true });
    expect(openPrograms({ query: "", programsOpen: false }, true)).toEqual({ query: null, programsOpen: true });
  });
  it("opening the folder field closes the program list, so it does not pop back once a folder is picked", () => {
    expect(openFolder({ query: null, programsOpen: true }, "")).toEqual({ query: "", programsOpen: false });
    expect(openFolder({ query: "", programsOpen: false }, "ai")).toEqual({ query: "ai", programsOpen: false });
  });
  it("closing either one leaves the other as it was", () => {
    expect(openPrograms({ query: null, programsOpen: true }, false)).toEqual({ query: null, programsOpen: false });
    expect(openFolder({ query: "ai", programsOpen: false }, null)).toEqual({ query: null, programsOpen: false });
  });
});

describe("the Administrator chip's key", () => {
  const key = (p: Partial<Parameters<typeof adminKey>[0]>) => ({ code: "KeyA", altKey: true, ctrlKey: false, metaKey: false, shiftKey: false, ...p });
  it("the_admin_chip_toggles_with_alt_a_and_sends_elevated: Alt+A, by its physical key, and nothing else", () => {
    expect(adminKey(key({}))).toBe(true);
    expect(adminKey(key({ altKey: false }))).toBe(false);
    expect(adminKey(key({ ctrlKey: true }))).toBe(false);
    expect(adminKey(key({ shiftKey: true }))).toBe(false);
    expect(adminKey(key({ code: "KeyB" }))).toBe(false);
  });
  it("adminKeyClaimed: Alt+A is claimed only while the chip is enabled, so Option+A still types a letter elsewhere", () => {
    expect(adminKeyClaimed(key({}), false)).toBe(false);
    expect(adminKeyClaimed(key({}), true)).toBe(true);
    for (const enabled of [false, true]) {
      expect(adminKeyClaimed(key({ ctrlKey: true }), enabled)).toBe(false);
      expect(adminKeyClaimed(key({ shiftKey: true }), enabled)).toBe(false);
      // AltGr+A arrives as Ctrl+Alt on Windows.
      expect(adminKeyClaimed(key({ ctrlKey: true, altKey: true }), enabled)).toBe(false);
    }
  });
});
