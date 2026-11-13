import { describe, expect, it } from "vitest";
import { tabHoldStep, type HoldKey } from "./tabHold";

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
});
