import { describe, expect, it } from "vitest";
import { DEFAULT_NAV_MODIFIER, NAV_MODIFIERS, isNavModifierHeld, navModifierLabel, navModifierWarnings } from "./modifier";

const flags = (o: Partial<Record<"ctrlKey" | "shiftKey" | "altKey" | "metaKey", boolean>>) => ({ ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...o });

describe("the navigation modifier", () => {
  it("is Ctrl+Shift by default on every system: the one pair no OS, layout or terminal program takes", () => {
    expect(DEFAULT_NAV_MODIFIER).toBe("ctrl+shift");
    expect(navModifierLabel("windows", "ctrl+shift")).toBe("Ctrl+Shift");
    expect(navModifierLabel("linux", "ctrl+shift")).toBe("Ctrl+Shift");
    expect(navModifierLabel("mac", "ctrl+shift")).toBe("⌃⇧");
  });
  it("counts as held only when exactly its keys are down, so Ctrl+Shift+Alt (the reopen chord) is not navigation", () => {
    expect(isNavModifierHeld(flags({ ctrlKey: true, shiftKey: true }), "ctrl+shift")).toBe(true);
    expect(isNavModifierHeld(flags({ ctrlKey: true }), "ctrl+shift")).toBe(false);
    expect(isNavModifierHeld(flags({ ctrlKey: true, shiftKey: true, altKey: true }), "ctrl+shift")).toBe(false);
    expect(isNavModifierHeld(flags({ ctrlKey: true, shiftKey: true, metaKey: true }), "ctrl+shift")).toBe(false);
    expect(isNavModifierHeld(flags({ ctrlKey: true, altKey: true }), "ctrl+alt")).toBe(true);
  });
  it("lists every choice with the systems where it is a bad idea, each with a reason", () => {
    expect(NAV_MODIFIERS.map((m) => m.id)).toEqual(["ctrl+shift", "ctrl+alt", "alt+shift"]);
    expect(navModifierWarnings("windows", "ctrl+shift")).toEqual([]);
    expect(navModifierWarnings("mac", "ctrl+shift")).toEqual([]);
    expect(navModifierWarnings("windows", "ctrl+alt").join(" ")).toMatch(/AltGr/);
    expect(navModifierWarnings("mac", "ctrl+alt").join(" ")).toMatch(/VoiceOver/);
    expect(navModifierWarnings("linux", "ctrl+alt").join(" ")).toMatch(/workspace/);
    expect(navModifierWarnings("windows", "alt+shift").join(" ")).toMatch(/keyboard layout/);
    expect(navModifierWarnings("mac", "alt+shift").join(" ")).toMatch(/symbols/);
  });
});
