import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it } from "vitest";
import { interpretBlur } from "../../lib/navigation/keys";
import { reduce } from "../../lib/navigation/machine";
import { GRID_CONTEXT, GRID_LAYOUT, NAMES } from "../../lib/navigation/fixtures";
import { INITIAL_NAV } from "../../lib/navigation/types";
import { claimsKey, focusKindOf, navPanesOf, setNavState, step } from "./useNavigation";

const key = (code: string, mods: { ctrl?: boolean; shift?: boolean } = { ctrl: true, shift: true }) => ({ code, key: code, ctrlKey: mods.ctrl ?? false, shiftKey: mods.shift ?? false, altKey: false, metaKey: false });

/** A stand-in for an element: `closest` and `matches` answer from the selectors it is said to match. */
const el = (matches: string[], tagName = "DIV", extra: Record<string, unknown> = {}) =>
  ({
    tagName,
    readOnly: false,
    matches: (s: string) => s.split(",").some((p) => matches.includes(p.trim())),
    closest: (s: string) => (s.split(",").some((p) => matches.includes(p.trim())) ? {} : null),
    ...extra,
  }) as unknown as Element;

afterEach(() => setNavState(INITIAL_NAV));

describe("focusKindOf", () => {
  it("is a terminal inside xterm, a text box for an input, a control for a button, and nothing for null", () => {
    expect(focusKindOf(el([".xterm"], "TEXTAREA"))).toBe("terminal");
    expect(focusKindOf(el([], "INPUT"))).toBe("editable");
    expect(focusKindOf(el(["button"], "BUTTON"))).toBe("control");
    expect(focusKindOf(el([], "DIV"))).toBe("none");
    expect(focusKindOf(null)).toBe("none");
  });

  it("is a control on a select or a details summary, so Tab moves on from them instead of selecting a terminal", () => {
    expect(focusKindOf(el(["select"], "SELECT"))).toBe("control");
    expect(focusKindOf(el(["summary"], "SUMMARY"))).toBe("control");
  });
});

describe("claimsKey", () => {
  it("leaves Ctrl+Shift+arrow to the terminal until a Tab has started navigation", () => {
    expect(claimsKey(key("ArrowLeft"), "windows", "terminal")).toBe(false);
    const down = reduce(INITIAL_NAV, { type: "modifier-down" }, GRID_CONTEXT).state;
    setNavState(reduce(down, { type: "tab" }, GRID_CONTEXT).state);
    expect(claimsKey(key("ArrowLeft"), "windows", "terminal")).toBe(true);
  });
  it("takes Ctrl+Shift+Backspace anywhere", () => {
    for (const focus of ["terminal", "editable", "control", "none"] as const) expect(claimsKey(key("Backspace"), "windows", focus), focus).toBe(true);
  });
  it("never takes a plain key from the terminal", () => {
    expect(claimsKey(key("Tab", {}), "windows", "terminal")).toBe(false);
    expect(claimsKey(key("ArrowLeft", {}), "windows", "terminal")).toBe(false);
  });
});

describe("the terminal", () => {
  it("lets a claimed chord through to the window instead of typing it", () => {
    const src = readFileSync(new URL("../terminal/TerminalView.tsx", import.meta.url), "utf8");
    expect(src).toMatch(/claimsKey\(e, osOfPlatform\(endpoint\.platform\), "terminal"\)\) return false/);
  });
  it("is reached only through the zoomed pane while one is zoomed", () => {
    const src = readFileSync(new URL("./LiveApp.tsx", import.meta.url), "utf8");
    expect(src).toMatch(/navPanesOf\(shownPanes\.map\(numberOf\), view\.zoomed\)/);
  });
});

describe("step", () => {
  it("cancels the cursor when the window loses focus mid-hold: nothing is selected or brought into view", () => {
    const ctx = { ...GRID_CONTEXT, layout: GRID_LAYOUT };
    // Tab selects the first terminal (slot 0); the arrow leaves it for its own row.
    const held = step(INITIAL_NAV, [{ type: "modifier-down" }, { type: "tab" }, { type: "arrow", dir: "left" }], ctx);
    expect(held.state.cursor?.kind).toBe("row");
    const blurred = step(held.state, interpretBlur(held.state), ctx);
    expect(blurred.effects.some((e) => e.type === "bring-into-view" || e.type === "select-pane")).toBe(false);
    expect(blurred.effects).toContainEqual({ type: "announce", text: "Cancelled" });
    expect(blurred.state.held).toBe(false);
  });
  it("in a zoomed pane, Tab stays on the one terminal on screen and announces it", () => {
    const panes = navPanesOf([3, 5, 7], 1);
    expect(panes).toEqual([null, 5, null]);
    const r = step({ ...INITIAL_NAV, selectedPane: 1 }, [{ type: "modifier-down" }, { type: "tab" }], { panes, layout: GRID_LAYOUT, names: NAMES });
    expect(r.effects).toContainEqual({ type: "select-pane", index: 1 });
    expect(r.effects.some((e) => e.type === "announce" && /session 5/.test(e.text))).toBe(true);
    expect(navPanesOf([3, 5], null)).toEqual([3, 5]);
  });
  it("brings a session that is not on screen into the selected pane on release", () => {
    const layout = [...GRID_LAYOUT.filter((l) => l.target.kind !== "pane"), { target: { kind: "row" as const, number: 7 }, rect: { x: 0, y: 320, w: 288, h: 56 } }];
    const ctx = { panes: [3, 5], layout, names: NAMES };
    const r = step({ ...INITIAL_NAV, selectedPane: 1, held: true, engaged: true, cursor: { kind: "row", number: 7 } }, [{ type: "modifier-up" }], ctx);
    expect(r.effects).toContainEqual({ type: "bring-into-view", number: 7, pane: 1 });
  });
});
