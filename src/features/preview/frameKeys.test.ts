import { describe, expect, it } from "vitest";
import { appKeyInit, forwardsToApp, trackHeld, type FrameKey } from "./frameKeys";

const key = (over: Partial<FrameKey>): FrameKey => ({ type: "keydown", key: "a", code: "KeyA", ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, repeat: false, defaultPrevented: false, ...over });
const none = new Set<string>();

describe("forwardsToApp", () => {
  it("passes a chord pressed in the frame on to the app", () => {
    expect(forwardsToApp(key({ ctrlKey: true, shiftKey: true, key: "A" }), none)).toBe(true);
    expect(forwardsToApp(key({ ctrlKey: true, shiftKey: true, key: "Tab", code: "Tab" }), none)).toBe(true);
    expect(forwardsToApp(key({ ctrlKey: true, key: "Tab", code: "Tab" }), none)).toBe(true);
    expect(forwardsToApp(key({ metaKey: true, key: "d", code: "KeyD" }), none)).toBe(true);
    expect(forwardsToApp(key({ altKey: true, key: "ArrowDown", code: "ArrowDown" }), none)).toBe(true);
  });
  it("leaves the page its own keys: letters, Shift with a letter, Tab, the arrows and Esc", () => {
    for (const k of [key({}), key({ shiftKey: true, key: "A" }), key({ key: "Tab", code: "Tab" }), key({ shiftKey: true, key: "Tab", code: "Tab" }), key({ key: "ArrowDown", code: "ArrowDown" }), key({ key: "Escape", code: "Escape" }), key({ key: " ", code: "Space" })])
      expect(forwardsToApp(k, none), k.key).toBe(false);
  });
  it("passes on nothing the pane's own handler already used", () => {
    expect(forwardsToApp(key({ ctrlKey: true, key: "s", code: "KeyS", defaultPrevented: true }), none)).toBe(false);
  });
  it("passes on the release of a modifier, and of a key whose press it passed on", () => {
    expect(forwardsToApp(key({ type: "keyup", key: "Control", code: "ControlLeft" }), none)).toBe(true);
    expect(forwardsToApp(key({ type: "keyup", key: "Shift", code: "ShiftRight" }), none)).toBe(true);
    expect(forwardsToApp(key({ type: "keyup", key: "Alt", code: "AltLeft" }), none)).toBe(true);
    expect(forwardsToApp(key({ type: "keyup", key: "Meta", code: "MetaLeft" }), none)).toBe(true);
    // Ctrl let go before Tab: Tab's release carries no modifier, and the app still waits for it.
    expect(forwardsToApp(key({ type: "keyup", key: "Tab", code: "Tab" }), new Set(["Tab"]))).toBe(true);
  });
  it("keeps the release of a key the page had to itself", () => {
    expect(forwardsToApp(key({ type: "keyup" }), none)).toBe(false);
    expect(forwardsToApp(key({ type: "keyup", key: "Tab", code: "Tab" }), none)).toBe(false);
  });
});

describe("appKeyInit", () => {
  it("copies what the app's shortcuts read, as an event that bubbles and can be cancelled", () => {
    expect(appKeyInit(key({ ctrlKey: true, shiftKey: true, key: "A", repeat: true }))).toEqual({ key: "A", code: "KeyA", ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, repeat: true, bubbles: true, cancelable: true });
  });
});

describe("AltGr and held keys", () => {
  it("does not forward a chord typed with AltGr", () => {
    expect(forwardsToApp(key({ ctrlKey: true, altKey: true, key: "@", getModifierState: (k) => k === "AltGraph" }), new Set())).toBe(false);
  });
  it("forgets a held key when told the keyboard left", () => {
    const held = new Set<string>();
    expect(trackHeld(key({ type: "keydown", ctrlKey: true, code: "KeyK" }), held)).toBe(true);
    held.clear();
    expect(trackHeld(key({ type: "keyup", code: "KeyK", key: "k" }), held)).toBe(false);
  });
});
