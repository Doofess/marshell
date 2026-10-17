import { describe, expect, it } from "vitest";
import { togglesPane, togglesPaneKey } from "./rowPick";

const mods = (over: Partial<Parameters<typeof togglesPane>[0]>) => ({ ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...over });
const key = (over: Partial<Parameters<typeof togglesPaneKey>[0]>) => ({ key: " ", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, target: null, ...over });

describe("togglesPane", () => {
  it("takes Ctrl alone on Windows and Linux", () => {
    expect(togglesPane(mods({ ctrlKey: true }), "windows")).toBe(true);
    expect(togglesPane(mods({ ctrlKey: true }), "linux")).toBe(true);
    expect(togglesPane(mods({ ctrlKey: true, shiftKey: true }), "windows")).toBe(false);
    expect(togglesPane(mods({ metaKey: true }), "windows")).toBe(false);
    expect(togglesPane(mods({}), "windows")).toBe(false);
  });
  it("takes Cmd alone on macOS, where Ctrl+click is a right-click", () => {
    expect(togglesPane(mods({ metaKey: true }), "mac")).toBe(true);
    expect(togglesPane(mods({ ctrlKey: true }), "mac")).toBe(false);
    expect(togglesPane(mods({ metaKey: true, altKey: true }), "mac")).toBe(false);
  });
  it("leaves a click in the rename text box to the box", () => {
    const target = { closest: (s: string) => (s === ".session-row__rename" ? {} : null) };
    expect(togglesPane({ ...mods({ ctrlKey: true }), target }, "windows")).toBe(false);
  });
});

describe("togglesPaneKey", () => {
  it("takes Ctrl+Space and nothing else", () => {
    expect(togglesPaneKey(key({ ctrlKey: true }))).toBe(true);
    expect(togglesPaneKey(key({ ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(togglesPaneKey(key({}))).toBe(false);
    expect(togglesPaneKey(key({ key: "a", ctrlKey: true }))).toBe(false);
  });
  it("leaves the key to the rename text box", () => {
    const target = { closest: (s: string) => (s === ".session-row__rename" ? {} : null) };
    expect(togglesPaneKey(key({ ctrlKey: true, target }))).toBe(false);
  });
});
