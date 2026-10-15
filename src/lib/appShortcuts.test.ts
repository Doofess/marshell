import { describe, expect, it } from "vitest";
import { sessionShortcut } from "./appShortcuts";
import type { KeyEventLike } from "./bindings";

const ev = (code: string, m: Partial<Omit<KeyEventLike, "code">> = {}): KeyEventLike => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
const cs = { ctrlKey: true, shiftKey: true };

describe("sessionShortcut", () => {
  it("names the keys the app takes from a focused terminal", () => {
    expect(sessionShortcut(ev("KeyT", cs), "windows")).toEqual({ kind: "launcher" });
    expect(sessionShortcut(ev("KeyW", cs), "windows")).toEqual({ kind: "close" });
    expect(sessionShortcut(ev("KeyT", { ...cs, altKey: true }), "windows")).toEqual({ kind: "reopen" });
    expect(sessionShortcut(ev("F2"), "windows")).toEqual({ kind: "rename" });
    expect(sessionShortcut(ev("Digit3", cs), "windows")).toEqual({ kind: "goto", number: 3 });
    expect(sessionShortcut(ev("ArrowUp", { altKey: true, shiftKey: true }), "linux")).toEqual({ kind: "move", by: -1 });
    expect(sessionShortcut(ev("ArrowDown", { altKey: true, shiftKey: true }), "linux")).toEqual({ kind: "move", by: 1 });
  });
  it("uses Cmd on macOS", () => {
    expect(sessionShortcut(ev("KeyW", { metaKey: true }), "mac")).toEqual({ kind: "close" });
    expect(sessionShortcut(ev("Digit9", { metaKey: true }), "mac")).toEqual({ kind: "goto", number: 9 });
  });
  it("leaves every other key to the program in the terminal", () => {
    expect(sessionShortcut(ev("KeyW", { ctrlKey: true }), "windows")).toBeNull();
    expect(sessionShortcut(ev("KeyK", cs), "windows")).toBeNull();
    expect(sessionShortcut(ev("ArrowUp"), "windows")).toBeNull();
    expect(sessionShortcut(ev("ArrowUp", { ctrlKey: true, shiftKey: true }), "windows")).toBeNull();
    expect(sessionShortcut(ev("F3"), "windows")).toBeNull();
    expect(sessionShortcut(ev("Digit0", cs), "windows")).toBeNull();
  });
});
