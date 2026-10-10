import { describe, expect, it } from "vitest";
import { EMPTY_STATES } from "./emptyStates";

describe("EMPTY_STATES", () => {
  it("covers every empty state the plan lists", () => {
    expect(EMPTY_STATES.map((e) => e.id)).toEqual(["no-sessions", "no-projects", "no-history", "no-agents", "no-matches", "doctor-clear", "no-notifications", "departures-empty"]);
  });
  it("each says what is empty and, unless it is good news, how to start", () => {
    for (const e of EMPTY_STATES) {
      expect(e.title.length).toBeGreaterThan(0);
      if (e.id !== "doctor-clear" && e.id !== "no-notifications") expect(e.action, e.id).toBeTruthy();
    }
  });
  it("follows the copy rules", () => {
    for (const e of EMPTY_STATES) expect(`${e.title} ${e.body} ${e.action ?? ""}`).not.toMatch(/!|successfully|oops|the ai/i);
  });
  it("uses the plan's own sentence for no sessions", () => {
    expect(EMPTY_STATES.find((e) => e.id === "no-sessions")!.body).toBe("Start one with Ctrl+Shift+T.");
  });
});
