import { describe, expect, it } from "vitest";
import { orderLane } from "../lane/laneLayout";
import { SCENARIO_FOOTER, SCENARIO_IDS, SCENARIO_PORTS, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";

describe("the 5-agent scenario (docs/PLAN.md deliverable 5)", () => {
  it("has two needing you, one working, one done-unseen and one error", () => {
    expect(SCENARIO_ROWS.length).toBe(5);
    const by = (s: string) => SCENARIO_ROWS.filter((r) => r.status === s).length;
    expect(by("needs-permission") + by("needs-question")).toBe(2);
    expect(by("working")).toBe(1);
    expect(by("done-unseen")).toBe(1);
    expect(by("error")).toBe(1);
  });
  it("keeps the ids in the plan's order", () => {
    expect(SCENARIO_ROWS.map((r) => r.id)).toEqual([...SCENARIO_IDS]);
  });
  it("has a lane request for each needs-you row, matched by session name", () => {
    const needs = SCENARIO_ROWS.filter((r) => r.status.startsWith("needs"));
    expect(SCENARIO_REQUESTS.map((q) => q.session.name).sort()).toEqual(needs.map((r) => r.name).sort());
  });
  it("targets the question first because it has waited longest", () => {
    expect(orderLane(SCENARIO_REQUESTS)[0]!.detail.kind).toBe("question");
  });
  it("has a footer and ports to show", () => {
    expect(SCENARIO_FOOTER.doctorIssues).toBeGreaterThan(0);
    expect(SCENARIO_PORTS.length).toBeGreaterThan(0);
  });
});
