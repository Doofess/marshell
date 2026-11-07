import { describe, expect, it } from "vitest";
import { adminKey, adminKeyClaimed, chipLetter, focusFellOut, listNavKeys, listToClose, nameKey, nameKeyClaimed, openFolder, openList, stepActive } from "./launcherKeys";

describe("launcherKeys: which keys move the highlighted option", () => {
  it("in the folder field only Up and Down do: Left and Right move the caret, so a typo can be fixed", () => {
    expect(listNavKeys("folder")).toEqual(["ArrowDown", "ArrowUp"]);
    expect(listNavKeys("folder")).not.toContain("ArrowLeft");
    expect(listNavKeys("folder")).not.toContain("ArrowRight");
  });
  it("in the shell and agent lists all four arrows do", () => {
    for (const list of ["shells", "agents"] as const) expect([...listNavKeys(list)].sort()).toEqual(["ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp"]);
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
  const closed = { query: null, shellsOpen: false, agentsOpen: false };
  it("opening_one_list_closes_the_others", () => {
    expect(openList({ ...closed, query: "air" }, "shells", true)).toEqual({ query: null, shellsOpen: true, agentsOpen: false });
    expect(openList({ ...closed, query: "" }, "agents", true)).toEqual({ query: null, shellsOpen: false, agentsOpen: true });
    expect(openList({ ...closed, shellsOpen: true }, "agents", true)).toEqual({ query: null, shellsOpen: false, agentsOpen: true });
    expect(openList({ ...closed, agentsOpen: true }, "shells", true)).toEqual({ query: null, shellsOpen: true, agentsOpen: false });
    expect(openFolder({ ...closed, shellsOpen: true }, "")).toEqual({ query: "", shellsOpen: false, agentsOpen: false });
    expect(openFolder({ ...closed, agentsOpen: true }, "")).toEqual({ query: "", shellsOpen: false, agentsOpen: false });
    expect(openFolder({ query: "", shellsOpen: false, agentsOpen: false }, "ai")).toEqual({ query: "ai", shellsOpen: false, agentsOpen: false });
  });
  it("closing one leaves the others as they were", () => {
    expect(openList({ ...closed, shellsOpen: true }, "shells", false)).toEqual(closed);
    expect(openList({ ...closed, query: "ai" }, "agents", false)).toEqual({ ...closed, query: "ai" });
    expect(openFolder({ ...closed, query: "ai" }, null)).toEqual(closed);
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

describe("the Name field's key", () => {
  const key = (p: Partial<Parameters<typeof nameKey>[0]> = {}) => ({ code: "KeyN", altKey: true, ctrlKey: false, metaKey: false, shiftKey: false, ...p });
  it("alt_n_is_the_name_key", () => {
    expect(nameKey(key())).toBe(true);
    expect(nameKey(key({ ctrlKey: true }))).toBe(false);
    expect(nameKey(key({ metaKey: true }))).toBe(false);
    expect(nameKey(key({ shiftKey: true }))).toBe(false);
    expect(nameKey(key({ altKey: false }))).toBe(false);
    expect(nameKey(key({ code: "KeyA" }))).toBe(false);
  });
  it("is left alone on a Mac, where Option+N is the dead key for a tilde", () => {
    expect(nameKeyClaimed(key(), "windows")).toBe(true);
    expect(nameKeyClaimed(key(), "linux")).toBe(true);
    expect(nameKeyClaimed(key(), "mac")).toBe(false);
  });
});

describe("launcherKeys: a held modifier is a shortcut, not a letter pick", () => {
  const press = (key: string, mods: Partial<{ altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }> = {}) => ({ key, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...mods });
  it("a_chip_letter_ignores_alt_ctrl_and_meta", () => {
    // Alt+N moves to Name and Alt+A toggles Administrator: neither may also pick "NixOS" or "Antigravity" on the focused chip.
    expect(chipLetter(press("n"))).toBe("n");
    expect(chipLetter(press("N", { shiftKey: true }))).toBe("N");
    expect(chipLetter(press("n", { altKey: true }))).toBeNull();
    expect(chipLetter(press("a", { ctrlKey: true }))).toBeNull();
    expect(chipLetter(press("a", { metaKey: true }))).toBeNull();
  });
});

describe("launcherKeys: leaving a list closes it", () => {
  it("alt_n_and_esc_close_the_one_open_list", () => {
    // Alt+N from an open list goes to Name; left open, the list would take Name's Enter and arrows.
    expect(listToClose({ folderOpen: true, shellsOpen: false, agentsOpen: false })).toBe("folder");
    expect(listToClose({ folderOpen: false, shellsOpen: true, agentsOpen: false })).toBe("shells");
    expect(listToClose({ folderOpen: false, shellsOpen: false, agentsOpen: true })).toBe("agents");
    expect(listToClose({ folderOpen: false, shellsOpen: false, agentsOpen: false })).toBeNull();
  });
});
