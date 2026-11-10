import { describe, expect, it } from "vitest";
import { DRAWER, drawerPlacement, sidebarMode, sidebarWidth as widthFor, type SidebarPref } from "./layout";
import { mainRoom, paneColumn, type ColumnInputs } from "./paneColumn";

const base: ColumnInputs = { pref: "expanded", drawer: false, shown: true, saved: null, terminalMin: 560, rtl: false };

describe("mainRoom", () => {
  it("matches the window's arithmetic", () => {
    for (const width of [800, 959, 960, 1400]) {
      for (const pref of ["expanded", "rail", "focus"] as SidebarPref[]) {
        for (const drawer of [false, true]) {
          const mode = sidebarMode(width, pref);
          const side = widthFor(mode, undefined);
          const pushed = drawer && drawerPlacement(width, mode, DRAWER.default) === "push" ? DRAWER.default : 0;
          expect(mainRoom(width, { pref, drawer })).toEqual({ mode, side, pushed, room: width - side - pushed });
        }
      }
    }
  });

  it("takes a requested sidebar width", () => {
    expect(mainRoom(1400, { pref: "expanded", drawer: false, sidebarWidth: 340 }).side).toBe(340);
  });
});

describe("paneColumn", () => {
  it("sits at the end edge", () => {
    expect(paneColumn(1400, { ...base, saved: 480 })).toEqual({ x: 1400 - 480, width: 480 });
  });

  it("sits beside a pushed drawer", () => {
    const c = paneColumn(1800, { ...base, drawer: true, saved: 480 });
    expect(c).toEqual({ x: 1800 - DRAWER.default - 480, width: 480 });
  });

  it("in a right-to-left window sits at the start of the screen", () => {
    expect(paneColumn(1400, { ...base, saved: 480, rtl: true })).toEqual({ x: 0, width: 480 });
    expect(paneColumn(1800, { ...base, saved: 480, rtl: true, drawer: true })).toEqual({ x: DRAWER.default, width: 480 });
  });

  it("is the room under 720 px of room", () => {
    // Rail (52) beside 960 - 288 = 672 of room: the expanded sidebar leaves too little.
    const c = paneColumn(960, base);
    expect(c).toEqual({ x: 288, width: 672 });
  });

  it("has no width when hidden", () => {
    expect(paneColumn(1400, { ...base, shown: false }).width).toBe(0);
  });
});
