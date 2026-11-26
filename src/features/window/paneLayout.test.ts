import { describe, expect, it } from "vitest";
import { dividerLeft, dividerState, draggedWidth, emptySlot, paneGeometry, paneMotionKey } from "./paneLayout";

describe("paneGeometry", () => {
  it("a hidden pane leaves the room to the terminal", () => {
    expect(paneGeometry({ room: 1000, shown: false, saved: 500, terminalMin: 600 })).toMatchObject({ mode: "hidden", terminal: 1000, pane: 0 });
  });
  it("with nothing saved the pane takes half the room", () => {
    expect(paneGeometry({ room: 1201, shown: true, saved: null, terminalMin: 600 })).toEqual({ mode: "beside", terminal: 600, pane: 600, max: 600 });
  });
  it("the pane stops where the terminal would drop under 80 columns", () => {
    expect(paneGeometry({ room: 1201, shown: true, saved: 900, terminalMin: 700 })).toMatchObject({ pane: 500, max: 500 });
  });
  it("a saved width wider than the room is shown clamped", () => {
    expect(paneGeometry({ room: 900, shown: true, saved: 2000, terminalMin: 400 }).pane).toBe(499);
  });
  it("the terminal goes under 80 columns only when the pane is at its minimum", () => {
    expect(paneGeometry({ room: 900, shown: true, saved: null, terminalMin: 700 })).toMatchObject({ pane: 320, terminal: 579 });
  });
  it("under 720 px the pane covers", () => {
    expect(paneGeometry({ room: 719, shown: true, saved: null, terminalMin: 0 })).toMatchObject({ mode: "cover", pane: 719, terminal: 0 });
    expect(paneGeometry({ room: 720, shown: true, saved: null, terminalMin: 0 }).mode).toBe("beside");
  });
  it("an expanded pane takes the terminal's place where it would sit beside", () => {
    expect(paneGeometry({ room: 1200, shown: true, saved: 500, terminalMin: 600, expanded: true })).toEqual({ mode: "cover", terminal: 0, pane: 1200, max: 599 });
    expect(paneGeometry({ room: 1200, shown: false, saved: 500, terminalMin: 600, expanded: true }).mode).toBe("hidden");
    expect(paneGeometry({ room: 1200, shown: true, saved: 500, terminalMin: 600, expanded: true, alone: true }).mode).toBe("fill");
    expect(paneGeometry({ room: 600, shown: true, saved: null, terminalMin: 0, expanded: true }).mode).toBe("cover");
    expect(paneGeometry({ room: 600, shown: true, saved: null, terminalMin: 0 }).mode).toBe("cover");
  });
  it("a room that is not a number gives zeros, not NaN", () => {
    expect(paneGeometry({ room: Number.NaN, shown: true, saved: null, terminalMin: 0 })).toMatchObject({ mode: "hidden", terminal: 0, pane: 0 });
    expect(paneGeometry({ room: -5, shown: true, saved: null, terminalMin: 0 })).toMatchObject({ mode: "hidden", terminal: 0, pane: 0 });
  });
});

describe("draggedWidth", () => {
  it("a drag is held between the minimum and the 80-column stop", () => {
    const i = { room: 1201, terminalMin: 700 };
    expect(draggedWidth({ ...i, fromEnd: 100 })).toBe(320);
    expect(draggedWidth({ ...i, fromEnd: 5000 })).toBe(500);
  });
});

describe("paneMotionKey", () => {
  it("a sidebar switch beside an open pane moves the pane too", () => {
    // The room changes with the sidebar, so a pane at half the room changes width: it must ease with the sidebar, not jump.
    expect(paneMotionKey("beside", "expanded")).not.toBe(paneMotionKey("beside", "rail"));
    expect(paneMotionKey("beside", "rail")).not.toBe(paneMotionKey("beside", "focus"));
  });
  it("a hidden or covering pane starts no motion of its own when the sidebar switches", () => {
    expect(paneMotionKey("hidden", "expanded")).toBe(paneMotionKey("hidden", "rail"));
    expect(paneMotionKey("cover", "expanded")).toBe(paneMotionKey("cover", "focus"));
  });
  it("expanding moves: a cover's key differs from a beside pane's", () => {
    expect(paneMotionKey("cover", "rail")).not.toBe(paneMotionKey("beside", "rail"));
  });
  it("showing, hiding and covering each change the key", () => {
    const keys = new Set([paneMotionKey("hidden", "rail"), paneMotionKey("beside", "rail"), paneMotionKey("cover", "rail")]);
    expect(keys.size).toBe(3);
  });
});

describe("dividerState", () => {
  it("can be reached and moved beside a settled pane", () => {
    expect(dividerState("beside", false)).toEqual({ live: true, still: false });
  });
  it("stays reachable, and doesn't move, while the layout moves around a pane that stays beside", () => {
    expect(dividerState("beside", true)).toEqual({ live: true, still: true });
  });
  it("is a line nobody can reach while the pane hides or covers", () => {
    expect(dividerState("hidden", true)).toEqual({ live: false, still: true });
    expect(dividerState("cover", false)).toEqual({ live: false, still: true });
    expect(dividerState("cover", true)).toEqual({ live: false, still: true });
  });
});

describe("dividerLeft", () => {
  it("hands the keyboard on when a focused divider goes because the pane covers", () => {
    expect(dividerLeft({ was: true, live: false, focused: true, covers: true })).toEqual({ focused: false, handOn: true });
  });
  it("forgets a focus it held when the pane hides, and hands nothing on", () => {
    // The divider is taken out with its focus and no blur is heard; a flag left set would move the keyboard at a later narrowing.
    expect(dividerLeft({ was: true, live: false, focused: true, covers: false })).toEqual({ focused: false, handOn: false });
  });
  it("hands nothing on from a divider that wasn't focused", () => {
    expect(dividerLeft({ was: true, live: false, focused: false, covers: true })).toEqual({ focused: false, handOn: false });
  });
  it("leaves a divider that stays, or was never there, as it is", () => {
    expect(dividerLeft({ was: true, live: true, focused: true, covers: false })).toEqual({ focused: true, handOn: false });
    expect(dividerLeft({ was: false, live: false, focused: false, covers: true })).toEqual({ focused: false, handOn: false });
    expect(dividerLeft({ was: false, live: true, focused: true, covers: false })).toEqual({ focused: true, handOn: false });
  });
});

describe("a pane with no session", () => {
  const alone = { room: 1000, shown: true, saved: 400, terminalMin: 700, alone: true };
  it("alone, the pane fills the room", () => {
    const g = paneGeometry(alone);
    expect(g.mode).toBe("fill");
    expect(g.terminal).toBe(0);
    expect(g.pane).toBe(1000);
  });
  it("alone in a narrow window still fills", () => {
    expect(paneGeometry({ ...alone, room: 500 }).mode).toBe("fill");
  });
  it("alone and closed is hidden", () => {
    expect(paneGeometry({ ...alone, shown: false }).mode).toBe("hidden");
  });
  it("a filling pane has no divider", () => {
    expect(dividerState("fill", false)).toEqual({ live: false, still: true });
  });
  it("the fill moves as one key", () => {
    expect(paneMotionKey("fill", "expanded")).toBe("fill");
  });
});

describe("emptySlot", () => {
  it("with no pane the empty state shows", () => {
    expect(emptySlot({ viewing: false, paneDrawn: false, threeTracks: false })).toBe("empty");
  });
  it("a settled filling pane is alone in the room", () => {
    expect(emptySlot({ viewing: false, paneDrawn: true, threeTracks: false })).toBe("nothing");
  });
  it("a filling pane that moves keeps the terminal's track held, so it slides in its own", () => {
    expect(emptySlot({ viewing: false, paneDrawn: true, threeTracks: true })).toBe("hold");
  });
  it("under History nothing shows", () => {
    expect(emptySlot({ viewing: true, paneDrawn: false, threeTracks: false })).toBe("nothing");
  });
});
