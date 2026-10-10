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

describe("layout (review focus 3)", () => {
  it("keeps the 28 px header even when nothing waits", () => {
    expect(laneLayout(0, 800)).toEqual({ height: LANE.header, scrolls: false, hiddenCount: 0 });
  });
  it("sizes one expanded card and a 36 px row for each other", () => {
    expect(laneLayout(1, 1000)).toEqual({ height: LANE.header + LANE.expanded, scrolls: false, hiddenCount: 0 });
    expect(laneLayout(2, 1000)).toEqual({ height: LANE.header + LANE.expanded + LANE.collapsed, scrolls: false, hiddenCount: 0 });
  });
  it("never exceeds 40% of the sidebar, scrolling instead and counting what is hidden", () => {
    const l = laneLayout(8, 800);
    expect(l.height).toBe(320);
    expect(l.scrolls).toBe(true);
    expect(l.hiddenCount).toBe(8 - 1 - Math.floor((320 - LANE.header - LANE.expanded) / LANE.collapsed));
  });
  it("copes with twelve requests in a short sidebar", () => {
    const l = laneLayout(12, 400);
    expect(l.height).toBeLessThanOrEqual(0.4 * 400);
    expect(l.scrolls).toBe(true);
    expect(l.hiddenCount).toBe(11);
  });
  it("keeps the header for nonsense heights", () => {
    expect(laneLayout(3, 0).height).toBe(LANE.header);
    expect(laneLayout(3, Number.NaN).height).toBe(LANE.header);
    expect(laneLayout(-1, 800).height).toBe(LANE.header);
  });
});
