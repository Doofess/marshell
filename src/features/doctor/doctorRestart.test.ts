import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../../generated/SessionInfo";
import { planRestarts, skippedReceipt } from "./doctorRestart";

const info = (n: number, patch: Partial<SessionInfo> = {}): SessionInfo =>
  ({ tab_id: `t${n}`, number: n, name: `Claude ${n}`, agent: "claude", mode: "manual", elevated: false, state: { kind: "running" }, ...patch }) as SessionInfo;

describe("planRestarts", () => {
  const never = () => false;
  it("restarts each open tab in its current mode and drops one that is gone, silently", () => {
    const p = planRestarts(["t1", "t9", "t2"], [info(1), info(2, { mode: "plan" })], never, never);
    expect(p.go.map((g) => [g.tabId, g.mode, g.elevated])).toEqual([["t1", "manual", false], ["t2", "plan", false]]);
    expect(p.skipped).toEqual([]);
  });
  it("skips a working session and one that needs the administrator acknowledgement, and names them by number", () => {
    const p = planRestarts(["t3", "t5", "t7"], [info(3), info(5, { elevated: true }), info(7)], (id) => id === "t7" || id === "t3", (i) => i.number === 5);
    expect(p.go).toEqual([]);
    expect(p.skipped).toEqual([3, 5, 7]);
  });
  it("skips a session with no mode to restart in", () => {
    expect(planRestarts(["t1"], [info(1, { mode: null })], never, never).skipped).toEqual([1]);
  });
});

describe("skippedReceipt", () => {
  it("says nothing when nothing was skipped", () => {
    expect(skippedReceipt([])).toBeNull();
  });
  it("words one session", () => {
    expect(skippedReceipt([3])).toBe("Session 3 wasn't restarted. Restart it from its tab.");
  });
  it("words several, joined with and", () => {
    expect(skippedReceipt([5, 3])).toBe("Sessions 3 and 5 weren't restarted. Restart them from their tabs.");
    expect(skippedReceipt([1, 2, 3])).toBe("Sessions 1, 2 and 3 weren't restarted. Restart them from their tabs.");
  });
});
