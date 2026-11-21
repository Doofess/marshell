import { describe, expect, it } from "vitest";
import { GRID_CONTEXT, GRID_LAYOUT, GRID_WITH_PANEL_BUTTONS, NAMES } from "./fixtures";
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

describe("Tab with the navigation modifier selects the terminal on screen, and only that", () => {
  it("selects the first terminal when nothing is selected", () => {
    const r = run(INITIAL_NAV, [DOWN, TAB], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "select-pane", index: 0 }]);
    expect(r.state.selectedPane).toBe(0);
    expect(announced(r.effects)).toEqual(["Selected session 1, billing"]);
  });
  it("never lands on a sidebar row, however many times it is pressed", () => {
    const r = run(INITIAL_NAV, [DOWN, ...Array(12).fill(TAB)], GRID_CONTEXT);
    for (const e of r.effects) expect(e.type).not.toBe("bring-into-view");
    expect(r.state.cursor).toBeNull();
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
  it("nav Tab starts the cursor from a terminal with one slot", () => {
    const ctx: NavContext = {
      panes: [3],
      names: NAMES,
      layout: [
        { target: { kind: "pane", index: 0 }, rect: { x: 288, y: 40, w: 600, h: 600 } },
        { target: { kind: "row", number: 3 }, rect: { x: 0, y: 40, w: 288, h: 56 } },
      ],
    };
    const first = run(INITIAL_NAV, [DOWN, TAB], ctx);
    expect(first.state.selectedPane).toBe(0);
    expect(without(first.effects)).toEqual([{ type: "select-pane", index: 0 }]);
    const moved = run(first.state, [arrow("left")], ctx);
    expect(moved.state.cursor).toEqual({ kind: "row", number: 3 });
    expect(announced(run(INITIAL_NAV, [DOWN, TAB], { ...ctx, panes: [null] }).effects)).toEqual(["No terminal on screen"]);
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
    expect(r.state).toEqual({ selectedPane: 2, selectedSide: null, cursor: null, held: false, engaged: false });
    expect(announced(r.effects).at(-1)).toBe("Selected session 3, api-server");
  });
  it("brings a session that is not on screen into the focused pane, then selects it", () => {
    // Session 5 is only in the sidebar. From slot 0: left to row 1, then down through 2 and 3 to 5.
    const r = run(at(0), [DOWN, arrow("left"), arrow("down"), arrow("down"), arrow("down"), UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([{ type: "bring-into-view", number: 5, pane: 0 }]);
    expect(r.state).toEqual({ selectedPane: 0, selectedSide: null, cursor: null, held: false, engaged: false });
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
    expect(without(r.effects)).toEqual([{ type: "activate", id: "sidebar" }]);
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
  it("selects an empty slot you moved to, so its chooser can take the keys", () => {
    const ctx: NavContext = { ...GRID_CONTEXT, panes: [1, null, 3, 6] };
    const r = run(at(0), [DOWN, arrow("right"), UP], ctx);
    expect(r.state.selectedPane).toBe(1);
    expect(announced(r.effects).at(-1)).toBe("Selected empty slot 2");
  });
  it("releasing the modifier with no cursor out changes nothing", () => {
    const r = run(at(2), [DOWN, UP], GRID_CONTEXT);
    expect(r.effects).toEqual([]);
    expect(r.state).toEqual({ selectedPane: 2, selectedSide: null, cursor: null, held: false, engaged: false });
  });
});

describe("Esc and the select-none chord", () => {
  it("Esc while the cursor is out cancels it and keeps the selection", () => {
    const r = run(at(0), [DOWN, arrow("left"), arrow("down"), { type: "escape" }, UP], GRID_CONTEXT);
    expect(without(r.effects)).toEqual([]);
    expect(r.state).toEqual({ selectedPane: 0, selectedSide: null, cursor: null, held: false, engaged: false });
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
    expect(r.state).toEqual({ selectedPane: null, selectedSide: null, cursor: null, held: false, engaged: false });
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

describe("each panel of the side pane is a stop after the terminals", () => {
  const BROWSER = { kind: "side", panel: "browser" } as const;
  const FILES = { kind: "side", panel: "files" } as const;
  const PANELS = [
    { id: "browser", name: "Browser" },
    { id: "files", name: "Files" },
  ];
  const layout = [
    { target: { kind: "pane", index: 0 } as const, rect: { x: 288, y: 40, w: 300, h: 300 } },
    { target: BROWSER, rect: { x: 600, y: 40, w: 300, h: 150 } },
    { target: FILES, rect: { x: 600, y: 191, w: 300, h: 150 } },
  ];
  const both: NavContext = { panes: [1], names: NAMES, side: PANELS, layout };
  const filesOnly: NavContext = { ...both, side: [{ id: "files", name: "Files" }] };
  it("Tab goes terminal, Browser, Files, terminal", () => {
    const toBrowser = run(at(0), [DOWN, TAB], both);
    expect(without(toBrowser.effects)).toEqual([{ type: "select-side", panel: "browser" }]);
    expect(toBrowser.state.selectedSide).toBe("browser");
    const toFiles = run(toBrowser.state, [TAB], both);
    expect(without(toFiles.effects)).toEqual([{ type: "select-side", panel: "files" }]);
    expect(toFiles.state.selectedSide).toBe("files");
    const back = run(toFiles.state, [TAB], both);
    expect(without(back.effects)).toEqual([{ type: "select-pane", index: 0 }]);
    expect(announced(back.effects)).toEqual(["Selected session 1, billing"]);
    expect(back.state.selectedSide).toBeNull();
  });
  it("Tab skips a panel that isn't on screen", () => {
    const first = run(at(0), [DOWN, TAB], filesOnly);
    expect(without(first.effects)).toEqual([{ type: "select-side", panel: "files" }]);
    expect(without(run(first.state, [TAB], filesOnly).effects)).toEqual([{ type: "select-pane", index: 0 }]);
  });
  it("from Browser Tab goes to Files, not to the terminal", () => {
    const r = run(at(null, { selectedSide: "browser" }), [DOWN, TAB], both);
    expect(without(r.effects)).toEqual([{ type: "select-side", panel: "files" }]);
    expect(r.state.selectedSide).toBe("files");
  });
  it("with no panel on screen Tab stays on the terminal's ring", () => {
    for (const side of [[], null, undefined]) {
      const ctx: NavContext = { ...both, side };
      const r = run(at(0), [DOWN, TAB], ctx);
      expect(without(r.effects)).toEqual([{ type: "select-pane", index: 0 }]);
      expect(r.state.selectedSide).toBeNull();
    }
  });
  it("a selected panel that left the screen is forgotten", () => {
    const r = run(at(0, { selectedSide: "browser" }), [DOWN, TAB], filesOnly);
    expect(without(r.effects)).toEqual([{ type: "select-side", panel: "files" }]);
    expect(r.state.selectedSide).toBe("files");
  });
  it("with the cursor out Tab carries it over each panel and selects on release", () => {
    const a = run(at(0, { cursor: { kind: "pane", index: 0 } }), [DOWN, TAB], both);
    expect(a.state.cursor).toEqual(BROWSER);
    expect(announced(a.effects)).toEqual(["Cursor on Browser"]);
    const b = run(a.state, [TAB], both);
    expect(b.state.cursor).toEqual(FILES);
    expect(announced(b.effects)).toEqual(["Cursor on Files"]);
    const released = run(b.state, [UP], both);
    expect(without(released.effects)).toEqual([{ type: "select-side", panel: "files" }]);
    expect(released.state.selectedSide).toBe("files");
    expect(released.state.cursor).toBeNull();
  });
  it("an arrow moves the cursor between the two panels by their boxes", () => {
    const down = run(at(null, { selectedSide: "browser" }), [DOWN, arrow("down")], both);
    expect(down.state.cursor).toEqual(FILES);
    expect(announced(down.effects)).toEqual(["Cursor on Files"]);
    expect(run(at(null, { selectedSide: "files" }), [DOWN, arrow("up")], both).state.cursor).toEqual(BROWSER);
  });
  it("an arrow can carry the cursor onto a panel", () => {
    const r = run(at(0), [DOWN, arrow("right")], both);
    expect(r.state.cursor).toEqual(BROWSER);
    expect(announced(r.effects)).toEqual(["Cursor on Browser"]);
  });
  it("an arrow starts from the panel that is selected", () => {
    const r = run(at(null, { selectedSide: "files" }), [DOWN, arrow("left")], both);
    expect(r.state.cursor).toEqual({ kind: "pane", index: 0 });
  });
  it("says Selected Browser and Selected Files", () => {
    expect(announced(run(INITIAL_NAV, [{ type: "choose", target: BROWSER }], both).effects)).toEqual(["Selected Browser"]);
    expect(announced(run(INITIAL_NAV, [{ type: "choose", target: FILES }], both).effects)).toEqual(["Selected Files"]);
  });
  it("selecting a terminal clears selectedSide", () => {
    const r = run(at(null, { selectedSide: "files" }), [{ type: "choose", target: { kind: "pane", index: 0 } }], filesOnly);
    expect(r.state.selectedSide).toBeNull();
    expect(r.state.selectedPane).toBe(0);
  });
  it("Esc deselects a selected panel; one that left the screen is nothing to deselect", () => {
    expect(without(run(at(null, { selectedSide: "files" }), [{ type: "escape" }], both).effects)).toEqual([{ type: "deselect" }]);
    expect(run(at(null, { selectedSide: "browser" }), [{ type: "escape" }], filesOnly).effects).toEqual([]);
  });
});

describe("the cursor and a choice speak a title-bar target's name", () => {
  it("the cursor speaks a title-bar target's name, not its id", () => {
    const withButtons: NavContext = { ...GRID_CONTEXT, layout: GRID_WITH_PANEL_BUTTONS };
    let state = at(null, { cursor: { kind: "top", id: "brand" } });
    for (const [id, name] of [["palette", "Command palette"], ["sidebar", "Sidebar"], ["panel-browser", "Browser"], ["panel-files", "Files"]] as const) {
      const r = run(state, [DOWN, arrow("right")], withButtons);
      expect(r.state.cursor).toEqual({ kind: "top", id });
      expect(announced(r.effects)).toEqual([`Cursor on ${name}`]);
      state = r.state;
    }
  });
  it("activating a title-bar target speaks its name", () => {
    for (const [id, name] of [["palette", "Command palette"], ["sidebar", "Sidebar"], ["panel-browser", "Browser"], ["panel-files", "Files"]] as const)
      expect(announced(run(at(0), [{ type: "choose", target: { kind: "top", id } }], GRID_CONTEXT).effects)).toEqual([`Activated ${name}`]);
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
