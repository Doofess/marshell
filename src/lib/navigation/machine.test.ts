import { describe, expect, it } from "vitest";
import { GRID_CONTEXT, GRID_LAYOUT, NAMES } from "./fixtures";
import { reduce, run } from "./machine";
import { INITIAL_NAV, type NavContext, type NavEffect, type NavEvent, type NavState } from "./types";

const at = (selectedPane: number | null, extra: Partial<NavState> = {}): NavState => ({ ...INITIAL_NAV, selectedPane, ...extra });
const effects = (s: NavState, evs: NavEvent[], ctx: NavContext = GRID_CONTEXT) => run(s, evs, ctx).effects;
const without = (e: NavEffect[]) => e.filter((x) => x.type !== "announce");
const announced = (e: NavEffect[]) => e.filter((x): x is Extract<NavEffect, { type: "announce" }> => x.type === "announce").map((x) => x.text);
const TAB: NavEvent = { type: "tab" };
const arrow = (dir: "left" | "right" | "up" | "down"): NavEvent => ({ type: "arrow", dir });
const DOWN: NavEvent = { type: "modifier-down" };
const UP: NavEvent = { type: "modifier-up" };

describe("Tab with the navigation modifier cycles the terminals on screen, and only those", () => {
  it("selects the first terminal when nothing is selected", () => {
    const r = run(INITIAL_NAV, [DOWN, TAB], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "select-pane", index: 0 }]);
    expect(r.state.selectedPane).toBe(0);
    expect(announced(r.effects)).toEqual(["Selected session 1, billing"]);
  });
  it("moves on to the next one, and wraps from the last back to the first", () => {
    expect(run(at(0), [DOWN, TAB], GRID_CONTEXT).state.selectedPane).toBe(1);
    expect(run(at(2), [DOWN, TAB], GRID_CONTEXT).state.selectedPane).toBe(3);
    expect(run(at(3), [DOWN, TAB], GRID_CONTEXT).state.selectedPane).toBe(0);
  });
  it("walks through all the panes in order with repeated taps while the modifier stays down", () => {
    const seen = run(INITIAL_NAV, [DOWN, TAB, TAB, TAB, TAB, TAB], GRID_CONTEXT).effects.filter((e) => e.type === "select-pane").map((e) => (e as { index: number }).index);
    expect(seen).toEqual([0, 1, 2, 3, 0]);
  });
  it("never lands on a sidebar row, however many times it is pressed", () => {
    const r = run(INITIAL_NAV, [DOWN, ...Array(12).fill(TAB)], GRID_CONTEXT);
    for (const e of r.effects) expect(e.type).not.toBe("bring-into-view");
    expect(r.state.cursor).toBeNull();
  });
  it("skips an empty slot: there is no terminal in it to select", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [1, null, 3, 6] };
    expect(run(at(0), [DOWN, TAB], ctx).state.selectedPane).toBe(2);
    expect(run(at(3), [DOWN, TAB], ctx).state.selectedPane).toBe(0);
  });
  it("does nothing, and says so, when no terminal is on screen", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [null, null] };
    const r = run(INITIAL_NAV, [DOWN, TAB], ctx);
    expect(without(r.effects)).toEqual([]);
    expect(r.state.selectedPane).toBeNull();
    expect(announced(r.effects)).toEqual(["No terminal on screen"]);
  });
  it("stays on the one terminal when there is only one", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [6] };
    expect(run(at(0), [DOWN, TAB], ctx).state.selectedPane).toBe(0);
  });
  it("does not need the modifier flag to be set first: the adapter may send Tab alone (outside a terminal)", () => {
    expect(reduce(INITIAL_NAV, TAB, GRID_CONTEXT).state.selectedPane).toBe(0);
  });
});

describe("holding the modifier and pressing an arrow starts a cursor that can go anywhere", () => {
  it("starts on the selected terminal and moves from there, without changing the selection", () => {
    const r = run(at(0), [DOWN, arrow("left")], GRID_CONTEXT);
    expect(r.state.cursor).toEqual({ kind: "row", number: 1 });
    expect(r.state.selectedPane).toBe(0);
    expect(without(r.effects)).toEqual([]);
  });
  it("moving left from a terminal lands on the row of that terminal's own session when it is in the list", () => {
    // Slot 2 holds session 3, and the row level with the middle of that tall pane is session 3's anyway; slot 0 holds session 1, which is not the middle row.
    expect(run(at(2), [DOWN, arrow("left")], GRID_CONTEXT).state.cursor).toEqual({ kind: "row", number: 3 });
    expect(run(at(0), [DOWN, arrow("left")], GRID_CONTEXT).state.cursor).toEqual({ kind: "row", number: 1 });
    // From the right-hand column the first left reaches the pane beside it, and only the next one the sidebar.
    expect(run(at(3), [DOWN, arrow("left")], GRID_CONTEXT).state.cursor).toEqual({ kind: "pane", index: 2 });
    expect(run(at(3), [DOWN, arrow("left"), arrow("left")], GRID_CONTEXT).state.cursor).toEqual({ kind: "row", number: 3 });
  });
  it("moves down the sidebar list one row per arrow", () => {
    const r = run(at(0), [DOWN, arrow("left"), arrow("down"), arrow("down")], GRID_CONTEXT);
    expect(r.state.cursor).toEqual({ kind: "row", number: 3 });
  });
  it("selects a session that is on screen by selecting its pane, on release", () => {
    const r = run(at(0), [DOWN, arrow("left"), arrow("down"), arrow("down"), UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "select-pane", index: 2 }]);
    expect(r.state).toEqual({ selectedPane: 2, cursor: null, held: false, engaged: false });
    expect(announced(r.effects).at(-1)).toBe("Selected session 3, api-server");
  });
  it("brings a session that is not on screen into the focused pane, then selects it", () => {
    // Session 5 is only in the sidebar. From slot 0: left to row 1, then down through 2 and 3 to 5.
    const r = run(at(0), [DOWN, arrow("left"), arrow("down"), arrow("down"), arrow("down"), UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "bring-into-view", number: 5, pane: 0 }]);
    expect(r.state).toEqual({ selectedPane: 0, cursor: null, held: false, engaged: false });
    expect(announced(r.effects).at(-1)).toBe("Selected session 5, docs-site");
  });
  it("uses the first empty slot when nothing is selected", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [1, null, 3, null] };
    const r = run(INITIAL_NAV, [{ type: "choose", target: { kind: "row", number: 5 } }], ctx);
    expect(without(r.effects)).toEqual([{ type: "bring-into-view", number: 5, pane: 1 }]);
  });
  it("uses the first slot when nothing is selected and every slot is taken", () => {
    const r = run(INITIAL_NAV, [{ type: "choose", target: { kind: "row", number: 5 } }], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "bring-into-view", number: 5, pane: 0 }]);
  });
  it("activates a top bar control on release instead of selecting anything", () => {
    const r = run(at(1), [DOWN, arrow("up"), UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "activate", id: "layout" }]);
    expect(r.state.selectedPane).toBe(1);
  });
  it("starts on the first terminal when nothing is selected, without moving yet", () => {
    const r = run(INITIAL_NAV, [DOWN, arrow("right")], GRID_CONTEXT);
    expect(r.state.cursor).toEqual({ kind: "pane", index: 0 });
    expect(r.state.selectedPane).toBeNull();
  });
  it("stays where it is at an edge", () => {
    const r = run(at(3), [DOWN, arrow("right"), arrow("down")], GRID_CONTEXT);
    expect(r.state.cursor).toEqual({ kind: "pane", index: 3 });
  });
  it("lets Tab move the cursor on to the next terminal when one is out, and still releases onto it", () => {
    const r = run(at(0), [DOWN, arrow("left"), TAB, UP], GRID_CONTEXT);
    expect(r.state.selectedPane).toBe(1);
    expect(without(r.effects)).toEqual([{ type: "select-pane", index: 1 }]);
  });
  it("selects an empty slot you moved to, so its chooser can take the keys", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [1, null, 3, 6] };
    const r = run(at(0), [DOWN, arrow("right"), UP], ctx);
    expect(r.state.selectedPane).toBe(1);
    expect(announced(r.effects).at(-1)).toBe("Selected empty slot 2");
  });
  it("releasing the modifier with no cursor out changes nothing", () => {
    const r = run(at(2), [DOWN, UP], GRID_CONTEXT);
    expect(r.effects).toEqual([]);
    expect(r.state).toEqual({ selectedPane: 2, cursor: null, held: false, engaged: false });
  });
});

describe("Esc and the select-none chord", () => {
  it("Esc while the cursor is out cancels it and keeps the selection", () => {
    const r = run(at(0), [DOWN, arrow("left"), arrow("down"), { type: "escape" }, UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([]);
    expect(r.state).toEqual({ selectedPane: 0, cursor: null, held: false, engaged: false });
    expect(announced(r.effects).at(-1)).toBe("Cancelled");
  });
  it("Esc with no cursor out selects nothing", () => {
    const r = run(at(2), [{ type: "escape" }], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "deselect" }]);
    expect(r.state.selectedPane).toBeNull();
    expect(announced(r.effects)).toEqual(["No session selected"]);
  });
  it("Esc with nothing selected does nothing", () => {
    expect(run(INITIAL_NAV, [{ type: "escape" }], GRID_CONTEXT).effects).toEqual([]);
  });
  it("the select-none chord deselects, also from the middle of a move", () => {
    const r = run(at(0), [DOWN, arrow("left"), { type: "select-none" }, UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "deselect" }]);
    expect(r.state).toEqual({ selectedPane: null, cursor: null, held: false, engaged: false });
  });
  it("Tab after selecting none selects the first terminal again", () => {
    expect(run(at(2), [{ type: "select-none" }, DOWN, TAB], GRID_CONTEXT).state.selectedPane).toBe(0);
  });
});

describe("the mouse", () => {
  it("selects at once, with no modifier", () => {
    const r = run(INITIAL_NAV, [{ type: "choose", target: { kind: "pane", index: 2 } }], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "select-pane", index: 2 }]);
    expect(r.state.selectedPane).toBe(2);
  });
  it("selects the pane of a session that is on screen when its sidebar row is clicked", () => {
    const r = run(at(0), [{ type: "choose", target: { kind: "row", number: 6 } }], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "select-pane", index: 3 }]);
  });
  it("activates a top bar control when it is clicked", () => {
    const r = run(at(0), [{ type: "choose", target: { kind: "top", id: "palette" } }], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "activate", id: "palette" }]);
    expect(r.state.selectedPane).toBe(0);
  });
});

describe("odd input", () => {
  it("copes with an empty layout and with nobody on screen", () => {
    const empty: NavContext = { panes: [], layout: [] };
    for (const ev of [TAB, arrow("left"), { type: "escape" } as NavEvent, { type: "select-none" } as NavEvent]) expect(() => run(INITIAL_NAV, [DOWN, ev, UP], empty)).not.toThrow();
  });
  it("forgets a selection whose slot no longer exists (the window shrank to fewer panes)", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [1, 2] };
    const r = run(at(3), [DOWN, TAB], ctx);
    expect(r.state.selectedPane).toBe(0);
  });
  it("names a session by its number alone when it has no name", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, names: {} };
    expect(announced(run(INITIAL_NAV, [DOWN, TAB], ctx).effects)).toEqual(["Selected session 1"]);
  });
  it("has the names the fixtures promise", () => {
    expect(NAMES[6]).toBe("scratch");
    expect(GRID_LAYOUT.length).toBe(3 + 5 + 4);
  });
});
