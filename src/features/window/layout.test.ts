import { describe, expect, it } from "vitest";
import { AUTO_COLLAPSE_BELOW, DRAWER, MIN_WINDOW, SIDEBAR, drawerPlacement, sidebarMode, sidebarWidth } from "./layout";

describe("constants from docs/PLAN.md", () => {
  it("match", () => {
    expect(SIDEBAR).toEqual({ default: 288, min: 200, max: 400, rail: 52 });
    expect(AUTO_COLLAPSE_BELOW).toBe(960);
    expect(MIN_WINDOW).toEqual({ width: 720, height: 480 });
    expect(DRAWER).toEqual({ default: 360, min: 300, max: 560 });
  });
});

describe("sidebarMode", () => {
  it("collapses to the rail below 960 px", () => {
    expect(sidebarMode(959, "expanded")).toBe("rail");
    expect(sidebarMode(960, "expanded")).toBe("expanded");
    expect(sidebarMode(720, "expanded")).toBe("rail");
  });
  it("keeps what the user chose otherwise", () => {
    expect(sidebarMode(1400, "rail")).toBe("rail");
    expect(sidebarMode(1400, "focus")).toBe("focus");
    expect(sidebarMode(800, "focus")).toBe("focus");
  });
});

describe("sidebarWidth", () => {
  it("is 288 by default, clamped between 200 and 400", () => {
    expect(sidebarWidth("expanded")).toBe(288);
    expect(sidebarWidth("expanded", 150)).toBe(200);
    expect(sidebarWidth("expanded", 999)).toBe(400);
    expect(sidebarWidth("expanded", Number.NaN)).toBe(288);
  });
  it("is 52 for the rail and 0 in focus mode", () => {
    expect(sidebarWidth("rail")).toBe(52);
    expect(sidebarWidth("focus")).toBe(0);
  });
});

describe("drawerPlacement", () => {
  it("pushes the terminal when it stays at 720 px or more", () => {
    expect(drawerPlacement(1600, "expanded", 360)).toBe("push");
  });
  it("overlays when the terminal would drop below 720 px", () => {
    expect(drawerPlacement(1280, "expanded", 360)).toBe("overlay");
    expect(drawerPlacement(720, "rail", 360)).toBe("overlay");
  });
});
