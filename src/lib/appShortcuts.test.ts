import { describe, expect, it } from "vitest";
import { cancelsCloseBar, commandShortcut, sessionShortcut } from "./appShortcuts";
import type { KeyEventLike } from "./bindings";
import { DEFAULT_NAV_MODIFIER } from "./navigation/modifier";

const ev = (code: string, m: Partial<Omit<KeyEventLike, "code">> = {}): KeyEventLike => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
const cs = { ctrlKey: true, shiftKey: true };

describe("cancelsCloseBar", () => {
  const key = (k: string, m: Partial<{ ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean; isComposing: boolean }> = {}) => ({ key: k, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
  it("is a plain Escape while the bar is up, wherever focus is", () => {
    expect(cancelsCloseBar(key("Escape"), true)).toBe(true);
  });
  it("is nothing when the bar is not up, so Escape still reaches the terminal", () => {
    expect(cancelsCloseBar(key("Escape"), false)).toBe(false);
  });
  it("ignores other keys, modified Escape and an Escape inside an input method", () => {
    expect(cancelsCloseBar(key("Enter"), true)).toBe(false);
    expect(cancelsCloseBar(key("Escape", { shiftKey: true }), true)).toBe(false);
    expect(cancelsCloseBar(key("Escape", { isComposing: true }), true)).toBe(false);
  });
});

describe("sessionShortcut", () => {
  it("names the keys the app takes from a focused terminal", () => {
    expect(sessionShortcut(ev("KeyT", cs), "windows")).toEqual({ kind: "launcher" });
    expect(sessionShortcut(ev("KeyW", cs), "windows")).toEqual({ kind: "close" });
    expect(sessionShortcut(ev("KeyT", { ...cs, altKey: true }), "windows")).toEqual({ kind: "reopen" });
    expect(sessionShortcut(ev("F2"), "windows")).toEqual({ kind: "rename" });
    expect(sessionShortcut(ev("KeyK", cs), "windows")).toEqual({ kind: "palette" });
    expect(sessionShortcut(ev("Digit3", cs), "windows")).toEqual({ kind: "goto", number: 3 });
    expect(sessionShortcut(ev("ArrowUp", { altKey: true, shiftKey: true }), "linux")).toEqual({ kind: "move", by: -1 });
    expect(sessionShortcut(ev("ArrowDown", { altKey: true, shiftKey: true }), "linux")).toEqual({ kind: "move", by: 1 });
    expect(sessionShortcut(ev("PageUp", cs), "windows")).toEqual({ kind: "prompt", by: -1 });
    expect(sessionShortcut(ev("PageDown", { metaKey: true }), "mac")).toEqual({ kind: "prompt", by: 1 });
  });
  it("Ctrl+Shift+D is split and Ctrl+Shift+P is zoom on Windows", () => {
    expect(sessionShortcut(ev("KeyD", cs), "windows")).toEqual({ kind: "split" });
    expect(sessionShortcut(ev("KeyP", cs), "windows")).toEqual({ kind: "zoom" });
    expect(sessionShortcut(ev("KeyD", { metaKey: true }), "mac")).toEqual({ kind: "split" });
  });
  it("moves between panes with the navigation modifier", () => {
    expect(DEFAULT_NAV_MODIFIER).toBe("ctrl+shift");
    expect(sessionShortcut(ev("Tab", cs), "windows")).toEqual({ kind: "nextPane" });
    expect(sessionShortcut(ev("Backspace", cs), "windows")).toEqual({ kind: "nonePane" });
    expect(sessionShortcut(ev("Tab", cs), "mac")).toEqual({ kind: "nextPane" });
    expect(sessionShortcut(ev("Tab"), "windows")).toBeNull();
  });
  it("Ctrl+Shift+I is focus mode on Windows and Linux and Cmd+I on macOS", () => {
    expect(sessionShortcut(ev("KeyI", cs), "windows")).toEqual({ kind: "focusMode" });
    expect(sessionShortcut(ev("KeyI", cs), "linux")).toEqual({ kind: "focusMode" });
    expect(sessionShortcut(ev("KeyI", { metaKey: true }), "mac")).toEqual({ kind: "focusMode" });
    expect(sessionShortcut(ev("KeyI"), "windows")).toBeNull();
  });
  it("uses Cmd on macOS", () => {
    expect(sessionShortcut(ev("KeyW", { metaKey: true }), "mac")).toEqual({ kind: "close" });
    expect(sessionShortcut(ev("Digit9", { metaKey: true }), "mac")).toEqual({ kind: "goto", number: 9 });
  });
  it("leaves every other key to the program in the terminal", () => {
    expect(sessionShortcut(ev("KeyW", { ctrlKey: true }), "windows")).toBeNull();
    expect(sessionShortcut(ev("ArrowUp"), "windows")).toBeNull();
    expect(sessionShortcut(ev("ArrowUp", { ctrlKey: true, shiftKey: true }), "windows")).toBeNull();
    expect(sessionShortcut(ev("F3"), "windows")).toBeNull();
    expect(sessionShortcut(ev("Digit0", cs), "windows")).toBeNull();
  });
});

describe("the shell panel shortcut", () => {
  it("Ctrl+Shift+S is the shell panel", () => {
    expect(sessionShortcut(ev("KeyS", cs), "windows")).toEqual({ kind: "shellPanel" });
    expect(sessionShortcut(ev("KeyS", { metaKey: true }), "mac")).toEqual({ kind: "shellPanel" });
    expect(commandShortcut("shell-panel")).toEqual({ kind: "shellPanel" });
  });
});

describe("the cheat sheet shortcut", () => {
  it("Ctrl+Shift+O (Cmd+O) is the cheat sheet", () => {
    expect(sessionShortcut(ev("KeyO", cs), "windows")).toEqual({ kind: "cheatsheet" });
    expect(sessionShortcut(ev("KeyO", { metaKey: true }), "mac")).toEqual({ kind: "cheatsheet" });
    expect(commandShortcut("cheatsheet")).toEqual({ kind: "cheatsheet" });
  });
});

describe("the needs-you shortcut", () => {
  it("Ctrl+Shift+N (Cmd+N) goes to the oldest waiting request", () => {
    expect(sessionShortcut(ev("KeyN", cs), "windows")).toEqual({ kind: "nextWaiting" });
    expect(sessionShortcut(ev("KeyN", { metaKey: true }), "mac")).toEqual({ kind: "nextWaiting" });
    expect(commandShortcut("next-waiting")).toEqual({ kind: "nextWaiting" });
  });
});

describe("commandShortcut", () => {
  it("maps the palette commands the live window runs, and nothing else", () => {
    expect(commandShortcut("close")).toEqual({ kind: "close" });
    expect(commandShortcut("focus-session-4")).toEqual({ kind: "goto", number: 4 });
    expect(commandShortcut("move-down")).toEqual({ kind: "move", by: 1 });
    expect(commandShortcut("prompt-prev")).toEqual({ kind: "prompt", by: -1 });
    expect(commandShortcut("prompt-next")).toEqual({ kind: "prompt", by: 1 });
    expect(commandShortcut("split")).toEqual({ kind: "split" });
    expect(commandShortcut("zoom-pane")).toEqual({ kind: "zoom" });
    expect(commandShortcut("nav-next-pane")).toEqual({ kind: "nextPane" });
    expect(commandShortcut("nav-none")).toEqual({ kind: "nonePane" });
    expect(commandShortcut("focus-mode")).toEqual({ kind: "focusMode" });
    expect(commandShortcut("sidebar")).toEqual({ kind: "sidebar" });
    expect(commandShortcut("doctor")).toBeNull();
    expect(commandShortcut("profiles")).toBeNull();
  });
});

describe("history shortcuts", () => {
  it("takes Ctrl+Shift+Y and Ctrl+Shift+Alt+F, and the palette's two commands", () => {
    expect(sessionShortcut(ev("KeyY", cs), "windows")).toEqual({ kind: "history" });
    expect(sessionShortcut(ev("KeyF", { ...cs, altKey: true }), "windows")).toEqual({ kind: "globalSearch" });
    expect(commandShortcut("history")).toEqual({ kind: "history" });
    expect(commandShortcut("global-search")).toEqual({ kind: "globalSearch" });
  });
});

describe("bookmark shortcut", () => {
  it("runs on Ctrl+Shift+B and from the palette", () => {
    expect(sessionShortcut(ev("KeyB", cs), "windows")).toEqual({ kind: "bookmark" });
    expect(commandShortcut("bookmark")).toEqual({ kind: "bookmark" });
  });
});
