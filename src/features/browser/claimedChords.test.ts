import { afterEach, describe, expect, it } from "vitest";
import codes from "../../../src-tauri/src/browser/codes.json";
import type { KeyEventLike } from "../../lib/bindings";
import { INITIAL_NAV } from "../../lib/navigation/types";
import { setNavState } from "../sessions/useNavigation";
import { appClaims, claimedChords } from "./claimedChords";

afterEach(() => setNavState(INITIAL_NAV));

const chordKey = (c: { code: string; ctrl: boolean; shift: boolean; alt: boolean }) => `${c.ctrl ? "C" : ""}${c.shift ? "S" : ""}${c.alt ? "A" : ""}+${c.code}`;
const keys = (list: { code: string; ctrl: boolean; shift: boolean; alt: boolean }[]) => list.map(chordKey).sort();
const event = (code: string, mods: { ctrl?: boolean; shift?: boolean; alt?: boolean } = {}): KeyEventLike => ({
  code,
  ctrlKey: mods.ctrl ?? false,
  shiftKey: mods.shift ?? false,
  altKey: mods.alt ?? false,
  metaKey: false,
});

describe("claimedChords", () => {
  it("the list holds exactly what the predicate claims", () => {
    const claims = (e: KeyEventLike) => (e.code === "KeyK" && e.ctrlKey && e.shiftKey && !e.altKey) || (e.code === "Tab" && e.ctrlKey && !e.shiftKey && !e.altKey);
    expect(keys(claimedChords(claims))).toEqual(["CS+KeyK", "C+Tab"].sort());
  });

  it("every code Rust maps is tried", () => {
    const tried = new Set<string>();
    const combos = new Set<string>();
    claimedChords((e) => {
      tried.add(e.code);
      combos.add(`${e.ctrlKey}${e.shiftKey}${e.altKey}`);
      return false;
    });
    expect([...tried].sort()).toEqual([...codes].sort());
    expect(combos.size).toBe(8);
  });

  it("never claims with the meta key down", () => {
    const seen: boolean[] = [];
    claimedChords((e) => (seen.push(e.metaKey), false));
    expect(seen.every((m) => m === false)).toBe(true);
  });

  it("with the app's own predicate, Ctrl+Shift+K is claimed and Ctrl+Shift+V, Ctrl+Shift+Z, Ctrl+L and Escape are not", () => {
    const list = keys(claimedChords(appClaims("windows", "terminal")));
    expect(list).toContain("CS+KeyK");
    expect(list).not.toContain("CS+KeyV");
    expect(list).not.toContain("CS+KeyZ");
    expect(list).not.toContain("C+KeyL");
    expect(list.filter((k) => k.endsWith("+Escape") && k.startsWith("+"))).toEqual([]);
  });

  it("the navigation chords are claimed only once navigation has started", () => {
    const arrow = "CS+ArrowLeft";
    expect(keys(claimedChords(appClaims("windows", "terminal")))).not.toContain(arrow);
    // The Tab chord starts navigation, so it is claimed from the first.
    expect(keys(claimedChords(appClaims("windows", "terminal")))).toContain("CS+Tab");
    setNavState({ ...INITIAL_NAV, held: true, engaged: true });
    expect(keys(claimedChords(appClaims("windows", "terminal")))).toContain(arrow);
  });

  it("the perf overlay's chord is claimed, as the terminal claims it", () => {
    expect(appClaims("windows", "terminal")(event("KeyP", { ctrl: true, shift: true, alt: true }))).toBe(true);
    expect(appClaims("windows", "terminal")(event("KeyP", { ctrl: true, shift: true }))).toBe(false);
  });
});
