import { describe, expect, it } from "vitest";
import { keyboardAnchor, menuStillValid, opensRowMenu } from "./rowMenu";

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
