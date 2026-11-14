import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import cases from "../../../src-tauri/src/browser/tab-hold.json";
import { afterTabHold, installTabHold, tabHoldStep, type HoldKey, type HoldWindow } from "./tabHold";

type Mods = { ctrl?: boolean; shift?: boolean; alt?: boolean; meta?: boolean; repeat?: boolean };
const key = (type: HoldKey["type"], code: string, m: Mods = {}): HoldKey => ({
  type,
  code,
  ctrlKey: m.ctrl ?? false,
  shiftKey: m.shift ?? false,
  altKey: m.alt ?? false,
  metaKey: m.meta ?? false,
  repeat: m.repeat ?? false,
});
const down = (code: string, m?: Mods) => key("keydown", code, m);
const up = (code: string, m?: Mods) => key("keyup", code, m);
const NOTHING = { held: false, release: null, swallow: false };

describe("tabHoldStep", () => {
  it("holds a fresh Ctrl+Tab and keeps it from every other handler", () => {
    expect(tabHoldStep(false, down("Tab", { ctrl: true }))).toEqual({ held: true, release: null, swallow: true });
  });
  it("doesn't hold Ctrl+Shift+Tab, Ctrl+Alt+Tab, Ctrl+Meta+Tab, plain Tab or a repeated Ctrl+Tab", () => {
    for (const m of [{ ctrl: true, shift: true }, { ctrl: true, alt: true }, { ctrl: true, meta: true }, {}, { ctrl: true, repeat: true }])
      expect(tabHoldStep(false, down("Tab", m)), JSON.stringify(m)).toEqual(NOTHING);
  });
  it("sends Ctrl+Shift+Tab when Shift goes down with Ctrl during the hold, and lets the Shift go on", () => {
    for (const code of ["ShiftLeft", "ShiftRight"])
      expect(tabHoldStep(true, down(code, { ctrl: true, shift: true })), code).toEqual({ held: false, release: { shift: true }, swallow: false });
  });
  it("sends Ctrl+Tab for a Shift without Ctrl, or with Alt", () => {
    for (const m of [{ shift: true }, { ctrl: true, shift: true, alt: true }])
      expect(tabHoldStep(true, down("ShiftLeft", m)), JSON.stringify(m)).toEqual({ held: false, release: { shift: false }, swallow: false });
  });
  it("sends, at Tab's release, the chord its modifiers make", () => {
    expect(tabHoldStep(true, up("Tab", { ctrl: true }))).toEqual({ held: false, release: { shift: false }, swallow: false });
    expect(tabHoldStep(true, up("Tab", { ctrl: true, shift: true }))).toEqual({ held: false, release: { shift: true }, swallow: false });
  });
  it("swallows a repeat during the hold", () => {
    expect(tabHoldStep(true, down("Tab", { ctrl: true, repeat: true }))).toEqual({ held: true, release: null, swallow: true });
  });
  it("sends Ctrl+Tab first for another key during the hold, and lets that key go on", () => {
    expect(tabHoldStep(true, down("KeyW", { ctrl: true }))).toEqual({ held: false, release: { shift: false }, swallow: false });
  });
  it("keeps waiting when Ctrl comes up during the hold", () => {
    expect(tabHoldStep(true, up("ControlLeft"))).toEqual({ held: true, release: null, swallow: false });
  });
  it("does nothing for a key-up with nothing held", () => {
    expect(tabHoldStep(false, up("Tab", { ctrl: true }))).toEqual(NOTHING);
  });
  it("ends the hold as the shell's rules do, in every shared case (tab-hold.json)", () => {
    expect(cases.length).toBeGreaterThan(0);
    for (const c of cases) {
      let held = false;
      const sends: string[] = [];
      for (const k of c.keys as { at: string; code?: string; ctrl?: boolean; shift?: boolean; alt?: boolean; repeat?: boolean }[]) {
        if (k.at === "late") {
          // The timer sends what was held: a Shift meanwhile would have ended the hold at its key-down.
          if (held) sends.push("Ctrl+Tab");
          held = false;
          continue;
        }
        const step = tabHoldStep(held, key(k.at === "down" ? "keydown" : "keyup", k.code ?? "", k));
        held = step.held;
        if (step.release) sends.push(step.release.shift ? "Ctrl+Shift+Tab" : "Ctrl+Tab");
      }
      expect(sends, c.case).toEqual(c.sends);
    }
  });
});

describe("installTabHold", () => {
  type Listener = (e: unknown) => void;
  const TARGET = { name: "the terminal" };
  let listeners: { type: string; fn: Listener }[];
  let send: Mock<(target: EventTarget | null, shift: boolean) => void>;
  let uninstall: () => void;
  const win: HoldWindow = {
    addEventListener: (type: string, fn: Listener) => void listeners.push({ type, fn }),
    removeEventListener: (type: string, fn: Listener) => void (listeners = listeners.filter((l) => l.type !== type || l.fn !== fn)),
  } as unknown as HoldWindow;
  const fire = <E extends { type: string }>(e: E): E => {
    for (const l of listeners.filter((l) => l.type === e.type)) l.fn(e);
    return e;
  };
  const ev = (type: HoldKey["type"], code: string, m: Mods = {}, trusted = true) => ({
    ...key(type, code, m),
    isTrusted: trusted,
    target: TARGET,
    preventDefault: vi.fn(),
    stopImmediatePropagation: vi.fn(),
  });

  beforeEach(() => {
    vi.useFakeTimers();
    listeners = [];
    send = vi.fn<(target: EventTarget | null, shift: boolean) => void>();
    uninstall = installTabHold(win, send);
  });
  afterEach(() => {
    uninstall();
    vi.useRealTimers();
  });

  it("sends a held Ctrl+Tab after 50 ms, to the key's target, and keeps the press from every other handler", () => {
    const press = fire(ev("keydown", "Tab", { ctrl: true }));
    expect(press.preventDefault).toHaveBeenCalled();
    expect(press.stopImmediatePropagation).toHaveBeenCalled();
    vi.advanceTimersByTime(49);
    expect(send).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(send.mock.calls).toEqual([[TARGET, false]]);
  });
  it("sends Ctrl+Shift+Tab once for a Shift 8 ms after Tab, and the timer sends nothing more", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    vi.advanceTimersByTime(8);
    const shift = fire(ev("keydown", "ShiftLeft", { ctrl: true, shift: true }));
    expect(send.mock.calls).toEqual([[TARGET, true]]);
    expect(shift.stopImmediatePropagation).not.toHaveBeenCalled();
    vi.runAllTimers();
    expect(send).toHaveBeenCalledTimes(1);
  });
  it("sends at Tab's release", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    vi.advanceTimersByTime(20);
    fire(ev("keyup", "Tab", { ctrl: true }));
    expect(send.mock.calls).toEqual([[TARGET, false]]);
    vi.runAllTimers();
    expect(send).toHaveBeenCalledTimes(1);
  });
  it("sends Ctrl+Tab before another key goes on, and doesn't stop that key", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    const other = fire(ev("keydown", "KeyW", { ctrl: true }));
    expect(send.mock.calls).toEqual([[TARGET, false]]);
    expect(other.stopImmediatePropagation).not.toHaveBeenCalled();
    expect(other.preventDefault).not.toHaveBeenCalled();
  });
  it("stops a repeat during the hold and sends nothing more for it", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    const repeat = fire(ev("keydown", "Tab", { ctrl: true, repeat: true }));
    expect(repeat.preventDefault).toHaveBeenCalled();
    expect(repeat.stopImmediatePropagation).toHaveBeenCalled();
    vi.runAllTimers();
    expect(send).toHaveBeenCalledTimes(1);
  });
  it("keeps waiting when Ctrl comes up first", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    vi.advanceTimersByTime(10);
    fire(ev("keyup", "ControlLeft"));
    vi.advanceTimersByTime(39);
    expect(send).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(send.mock.calls).toEqual([[TARGET, false]]);
  });
  it("drops the hold unsent when the window loses the keyboard", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    vi.advanceTimersByTime(10);
    fire({ type: "blur" });
    vi.runAllTimers();
    expect(send).not.toHaveBeenCalled();
  });
  it("lets an untrusted Ctrl+Tab, such as a page's forwarded key, pass untouched", () => {
    const forwarded = fire(ev("keydown", "Tab", { ctrl: true }, false));
    expect(forwarded.preventDefault).not.toHaveBeenCalled();
    expect(forwarded.stopImmediatePropagation).not.toHaveBeenCalled();
    vi.runAllTimers();
    expect(send).not.toHaveBeenCalled();
  });
  it("runs a waiter at once with nothing held, and after the held key went out, once, when one is", () => {
    const waiter = vi.fn();
    afterTabHold(waiter);
    expect(waiter).toHaveBeenCalledTimes(1);
    fire(ev("keydown", "Tab", { ctrl: true }));
    afterTabHold(waiter);
    vi.advanceTimersByTime(20);
    expect(waiter).toHaveBeenCalledTimes(1);
    fire(ev("keyup", "Tab", { ctrl: true }));
    expect(send).toHaveBeenCalledTimes(1);
    vi.runAllTimers();
    expect(waiter).toHaveBeenCalledTimes(2);
    vi.runAllTimers();
    expect(waiter).toHaveBeenCalledTimes(2);
  });
  it("runs a waiter when a hold is dropped, too", () => {
    const waiter = vi.fn();
    fire(ev("keydown", "Tab", { ctrl: true }));
    afterTabHold(waiter);
    fire({ type: "blur" });
    vi.runAllTimers();
    expect(waiter).toHaveBeenCalledTimes(1);
    expect(send).not.toHaveBeenCalled();
  });
  it("drops a held key and removes its listeners when uninstalled", () => {
    fire(ev("keydown", "Tab", { ctrl: true }));
    uninstall();
    vi.runAllTimers();
    expect(send).not.toHaveBeenCalled();
    expect(listeners).toEqual([]);
  });
});
