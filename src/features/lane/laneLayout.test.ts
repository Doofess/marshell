import { describe, expect, it } from "vitest";
import { APPROVALS } from "../approval/fixtures";
import type { ApprovalRequest } from "../approval/types";
import { LANE, laneHeading, laneLayout, orderLane, pending } from "./laneLayout";

const req = (id: string, waitingMs: number, phase: "pending" | "terminal" = "pending"): ApprovalRequest => ({
  ...APPROVALS.safeBash,
  id,
  waitingMs,
  state: phase === "pending" ? { phase: "pending" } : { phase: "terminal" },
});

describe("order", () => {
  it("is oldest waiting first, so waiting is first-in first-out", () => {
    expect(orderLane([req("b", 10), req("a", 500), req("c", 60)]).map((r) => r.id)).toEqual(["a", "c", "b"]);
  });
  it("breaks ties by id so the order never flickers", () => {
    expect(orderLane([req("z", 5), req("a", 5)]).map((r) => r.id)).toEqual(["a", "z"]);
  });
  it("does not mutate its input", () => {
    const input = [req("b", 1), req("a", 2)];
    orderLane(input);
    expect(input.map((r) => r.id)).toEqual(["b", "a"]);
  });
  it("counts only pending requests", () => {
    expect(pending([req("a", 1), req("b", 2, "terminal")]).map((r) => r.id)).toEqual(["a"]);
  });
});

describe("heading", () => {
  it("says All clear when nothing waits", () => {
    expect(laneHeading(0)).toBe("All clear");
  });
  it("counts what waits", () => {
    expect(laneHeading(1)).toBe("Needs you · 1");
    expect(laneHeading(2)).toBe("Needs you · 2");
  });
});

describe("layout (review focus 3): the lane never scrolls", () => {
  it("keeps the 28 px header even when nothing waits", () => {
    expect(laneLayout(0, 800)).toEqual({ height: LANE.header, clipped: false, hiddenCount: 0, expanded: false, visible: 0 });
  });
  it("sizes one expanded card and a 36 px row for each other", () => {
    expect(laneLayout(1, 1000)).toEqual({ height: LANE.header + LANE.expanded, clipped: false, hiddenCount: 0, expanded: true, visible: 1 });
    expect(laneLayout(2, 1000)).toEqual({ height: LANE.header + LANE.expanded + LANE.collapsed, clipped: false, hiddenCount: 0, expanded: true, visible: 2 });
  });
  it("never exceeds 40% of the sidebar: it shows whole rows only and counts what it left out", () => {
    const l = laneLayout(8, 800);
    const fits = Math.floor((320 - LANE.header - LANE.expanded) / LANE.collapsed);
    expect(l.expanded).toBe(true);
    expect(l.visible).toBe(1 + fits);
    expect(l.height).toBe(LANE.header + LANE.expanded + fits * LANE.collapsed);
    expect(l.height).toBeLessThanOrEqual(320);
    expect(l.hiddenCount).toBe(8 - l.visible);
    expect(l.clipped).toBe(true);
  });
  it("never cuts a row in half: the height is always the sum of whole parts", () => {
    for (const n of [1, 2, 3, 5, 12, 30])
      for (const h of [200, 400, 520, 640, 800, 1000, 1400]) {
        const l = laneLayout(n, h);
        const parts = LANE.header + (l.expanded ? LANE.expanded + (l.visible - 1) * LANE.collapsed : l.visible * LANE.collapsed);
        expect(l.height, `${n} in ${h}`).toBe(parts);
        expect(l.visible + l.hiddenCount, `${n} in ${h}`).toBe(n);
      }
  });
  it("copes with twelve requests in a short sidebar: the card is too tall for 40%, so it shows one-liners only", () => {
    const l = laneLayout(12, 400);
    expect(l.expanded).toBe(false);
    expect(l.height).toBeLessThanOrEqual(0.4 * 400);
    expect(l.visible).toBe(Math.floor((160 - LANE.header) / LANE.collapsed));
    expect(l.hiddenCount).toBe(12 - l.visible);
    expect(l.clipped).toBe(true);
  });
  it("keeps the header for nonsense heights, hiding every request behind the +N button", () => {
    for (const h of [0, Number.NaN, -5]) {
      const l = laneLayout(3, h);
      expect(l.height).toBe(LANE.header);
      expect(l.visible).toBe(0);
      expect(l.hiddenCount).toBe(3);
    }
    expect(laneLayout(-1, 800).height).toBe(LANE.header);
  });
});
