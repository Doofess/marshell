import { describe, expect, it } from "vitest";
import { interpretBlur, interpretKeyDown, interpretKeyUp, type FocusKind } from "./keys";
import { INITIAL_NAV, type NavState } from "./types";
import type { KeyEventLike } from "../bindings";

const ev = (code: string, m: Partial<Omit<KeyEventLike, "code">> = {}): KeyEventLike => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
const CS = { ctrlKey: true, shiftKey: true };
const down = (e: KeyEventLike, focus: FocusKind = "control", state: NavState = INITIAL_NAV, os: "windows" | "mac" | "linux" = "windows") => interpretKeyDown(e, { os, nav: "ctrl+shift", focus, state });
const held = (extra: Partial<NavState> = {}): NavState => ({ ...INITIAL_NAV, held: true, ...extra });

describe("plain keys in a terminal belong to the program", () => {
  it("lets Tab, Esc, Enter, Backspace, Space and the arrows through untouched", () => {
    for (const code of ["Tab", "Escape", "Enter", "Backspace", "Space", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "KeyA"])
      for (const state of [INITIAL_NAV, { ...INITIAL_NAV, selectedPane: 2 }]) expect(down(ev(code), "terminal", state), code).toBeNull();
    expect(down(ev("Tab", { shiftKey: true }), "terminal")).toBeNull();
  });
  it("lets them through in a text box too (the prompt line)", () => {
    for (const code of ["Tab", "Escape", "ArrowLeft", "Backspace"]) expect(down(ev(code), "editable"), code).toBeNull();
  });
});

describe("the navigation modifier going down and up", () => {
  it("is noticed when the second of Ctrl and Shift goes down", () => {
    expect(down(ev("ShiftLeft", CS))).toEqual({ events: [{ type: "modifier-down" }], preventDefault: false });
    expect(down(ev("ControlRight", CS))).toEqual({ events: [{ type: "modifier-down" }], preventDefault: false });
  });
  it("is not noticed when only one of them is down, or when Alt joins them", () => {
    expect(down(ev("ControlLeft", { ctrlKey: true }))).toBeNull();
    expect(down(ev("AltLeft", { ...CS, altKey: true }))).toBeNull();
  });
  it("is noticed once, not on every key repeat", () => {
    expect(down(ev("ShiftLeft", CS), "control", held())).toBeNull();
  });
  it("ends when either key comes up, and cancels the cursor on blur", () => {
    expect(interpretKeyUp(ev("ShiftLeft", { ctrlKey: true }), { nav: "ctrl+shift", state: held() })).toEqual([{ type: "modifier-up" }]);
    expect(interpretKeyUp(ev("ShiftLeft", { ctrlKey: true }), { nav: "ctrl+shift", state: INITIAL_NAV })).toEqual([]);
    expect(interpretKeyUp(ev("KeyK", CS), { nav: "ctrl+shift", state: held() })).toEqual([]);
    expect(interpretBlur(held({ cursor: { kind: "row", number: 3 } }))).toEqual([{ type: "escape" }, { type: "modifier-up" }]);
    expect(interpretBlur(held())).toEqual([{ type: "modifier-up" }]);
    expect(interpretBlur(INITIAL_NAV)).toEqual([]);
  });
});

describe("keys with the modifier held", () => {
  it("Tab cycles the terminals on screen, even from inside a terminal, and is kept from the program", () => {
    for (const focus of ["terminal", "editable", "control", "none"] as FocusKind[]) expect(down(ev("Tab", CS), focus, held()), focus).toEqual({ events: [{ type: "tab" }], preventDefault: true });
  });
  it("sends modifier-down first when the press of Tab is the first the page has heard of the modifier", () => {
    expect(down(ev("Tab", CS))).toEqual({ events: [{ type: "modifier-down" }, { type: "tab" }], preventDefault: true });
  });
  it("Backspace selects no terminal", () => {
    expect(down(ev("Backspace", CS), "terminal", held())).toEqual({ events: [{ type: "select-none" }], preventDefault: true });
  });
  it("Esc cancels or deselects (Windows never delivers Ctrl+Shift+Esc, which is Task Manager; macOS and Linux do)", () => {
    expect(down(ev("Escape", CS), "terminal", held())).toEqual({ events: [{ type: "escape" }], preventDefault: true });
  });
  it("arrows move the cursor outside a text box, and inside one only after Tab has started navigation", () => {
    const left = ev("ArrowLeft", CS);
    expect(down(left, "control", held())?.events).toEqual([{ type: "arrow", dir: "left" }]);
    expect(down(left, "none", held())?.events).toEqual([{ type: "arrow", dir: "left" }]);
    // In a terminal or the prompt line Ctrl+Shift+Left selects a word, so it is left alone until Tab was used.
    expect(down(left, "terminal", held())).toBeNull();
    expect(down(left, "editable", held())).toBeNull();
    expect(down(left, "terminal", held({ engaged: true }))).toEqual({ events: [{ type: "arrow", dir: "left" }], preventDefault: true });
    expect(down(left, "editable", held({ cursor: { kind: "pane", index: 0 } }))?.events).toEqual([{ type: "arrow", dir: "left" }]);
  });
  it("maps all four arrows", () => {
    for (const [code, dir] of [["ArrowLeft", "left"], ["ArrowRight", "right"], ["ArrowUp", "up"], ["ArrowDown", "down"]] as const) expect(down(ev(code, CS), "control", held())?.events).toEqual([{ type: "arrow", dir }]);
  });
  it("leaves every other chord to the app shortcuts", () => {
    expect(down(ev("KeyK", CS), "terminal", held())).toBeNull();
    expect(down(ev("Digit3", CS), "terminal", held())).toBeNull();
  });
  it("uses the navigation modifier that is set: Ctrl+Alt instead", () => {
    const e = ev("Tab", { ctrlKey: true, altKey: true });
    expect(interpretKeyDown(e, { os: "linux", nav: "ctrl+alt", focus: "terminal", state: held() })?.events).toEqual([{ type: "tab" }]);
    expect(interpretKeyDown(ev("Tab", CS), { os: "linux", nav: "ctrl+alt", focus: "terminal", state: held() })).toBeNull();
  });
  it("uses Ctrl+Shift on macOS too (Cmd+Tab is the system's app switcher and never arrives)", () => {
    expect(down(ev("Tab", CS), "terminal", held(), "mac")?.events).toEqual([{ type: "tab" }]);
    expect(down(ev("Tab", { metaKey: true }), "terminal", held(), "mac")).toBeNull();
  });
});

describe("plain keys outside a terminal", () => {
  it("Tab with nothing focused selects the first terminal: that is the 'Tab to select a terminal' hint", () => {
    expect(down(ev("Tab"), "none")).toEqual({ events: [{ type: "tab" }], preventDefault: true });
  });
  it("Tab on a button goes on to the next control as usual, so the page stays keyboard-reachable", () => {
    expect(down(ev("Tab"), "control")).toBeNull();
    expect(down(ev("Tab", { shiftKey: true }), "control")).toBeNull();
  });
  it("Esc with a terminal selected but focus on a control (after a click on a header button) selects nothing", () => {
    expect(down(ev("Escape"), "control", { ...INITIAL_NAV, selectedPane: 1 })).toEqual({ events: [{ type: "escape" }], preventDefault: false });
    expect(down(ev("Escape"), "control", INITIAL_NAV)).toBeNull();
  });
  it("Esc closes nothing it should not: in a terminal it still goes to the program", () => {
    expect(down(ev("Escape"), "terminal", { ...INITIAL_NAV, selectedPane: 1 })).toBeNull();
  });
});
