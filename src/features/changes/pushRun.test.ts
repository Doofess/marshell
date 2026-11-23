import { describe, expect, it } from "vitest";
import type { PushProgress } from "../../generated/PushProgress";
import { plan } from "./pullSheetFixtures";
import { type RunEvent, type RunState, INITIAL_RUN, linesOf, pushStep, shouldFollow } from "./pushRun";

const prog = (o: Partial<PushProgress>): PushProgress => ({ state: "running", start: 0, lines: [], total: 0, reason: null, ...o });
const run = (events: RunEvent[], from: RunState = INITIAL_RUN) => events.reduce(pushStep, from);

describe("pushStep", () => {
  it("goes from confirm to running to pushed", () => {
    let s = run([{ type: "plan", plan: plan() }]);
    expect(s.kind).toBe("confirm");
    s = run([{ type: "confirm", create: false }, { type: "started", id: "p1" }], s);
    expect(s).toMatchObject({ kind: "running", id: "p1" });
    s = run([{ type: "progress", progress: prog({ lines: ["a", "b"] }) }], s);
    expect(s).toMatchObject({ kind: "running", lines: ["a", "b"] });
    s = run([{ type: "progress", progress: prog({ state: "pushed", start: 2, reason: "pushed to a fork" }) }], s);
    expect(s).toMatchObject({ kind: "pushed", reason: "pushed to a fork", lines: ["a", "b"] });
  });
  it("shows a refusal or a failed call as refused", () => {
    expect(run([{ type: "plan", plan: plan({ refusal: "No." }) }])).toMatchObject({ kind: "refused", reason: "No." });
    expect(run([{ type: "plan_failed", reason: "Down." }])).toMatchObject({ kind: "refused", reason: "Down." });
  });
  it("ends in failed or cancelled with the output kept", () => {
    const running = run([{ type: "plan", plan: plan() }, { type: "confirm", create: false }, { type: "started", id: "p1" }, { type: "progress", progress: prog({ lines: ["x"] }) }]);
    expect(run([{ type: "progress", progress: prog({ state: "failed", start: 1, reason: "rejected" }) }], running)).toMatchObject({ kind: "failed", reason: "rejected", lines: ["x"] });
    expect(run([{ type: "progress", progress: prog({ state: "cancelled", start: 1 }) }], running).kind).toBe("cancelled");
    const refused = run([{ type: "plan", plan: plan() }, { type: "confirm", create: false }, { type: "start_refused", reason: "Busy." }]);
    expect(refused).toMatchObject({ kind: "failed", reason: "Busy." });
  });
  it("Try again starts a new push", () => {
    const failed = run([{ type: "plan", plan: plan() }, { type: "confirm", create: false }, { type: "started", id: "p1" }, { type: "progress", progress: prog({ state: "failed", lines: ["x"], reason: "no" }) }]);
    const again = pushStep(failed, { type: "try_again" });
    expect(again).toMatchObject({ kind: "running", id: null, lines: [], reason: null });
    expect(pushStep(again, { type: "started", id: "p2" }).id).toBe("p2");
  });
  it("a create failure after a push keeps the form and skips the push", () => {
    let s = run([{ type: "plan", plan: plan() }, { type: "confirm", create: true }, { type: "started", id: "p1" }, { type: "progress", progress: prog({ state: "pushed" }) }]);
    expect(s.kind).toBe("pushed");
    s = run([{ type: "create_started" }], s);
    expect(s.kind).toBe("creating");
    s = run([{ type: "create_failed", reason: "GitHub said no." }], s);
    expect(s).toMatchObject({ kind: "create_failed", reason: "GitHub said no." });
    expect(pushStep(s, { type: "confirm", create: true }).kind).toBe("creating");
    expect(run([{ type: "create_started" }, { type: "created", number: 9, url: "https://github.com/octo-org/widgets/pull/9" }], s)).toMatchObject({ kind: "created", created: { number: 9 } });
  });
  it("Create with nothing to push goes straight to creating", () => {
    expect(run([{ type: "plan", plan: plan({ to_push: 0 }) }, { type: "confirm", create: true }]).kind).toBe("creating");
    expect(run([{ type: "plan", plan: plan({ to_push: 0 }) }, { type: "confirm", create: false }]).kind).toBe("running");
  });
  it("ignores events that don't fit the state", () => {
    expect(pushStep(INITIAL_RUN, { type: "progress", progress: prog({ state: "pushed" }) })).toBe(INITIAL_RUN);
  });
});

describe("linesOf", () => {
  // The second argument is how many lines the core had when the sheet last asked: the core numbers its lines from the
  // push's first, and drops old ones, so the sheet's own count of held lines says nothing about them.
  it("replaces the redrawn last line and appends the rest", () => {
    expect(linesOf(["a", "b 10%"], 2, prog({ start: 1, lines: ["b 100%", "c"], total: 3 }))).toEqual(["a", "b 100%", "c"]);
  });
  it("keeps the numbering of a sheet that holds fewer lines than the core has had", () => {
    // Opened late, or the core dropped old lines: two lines are held and the core has had 500.
    expect(linesOf(["x", "y 10%"], 500, prog({ start: 499, lines: ["y 100%", "z"], total: 501 }))).toEqual(["x", "y 100%", "z"]);
    expect(linesOf(["x", "y"], 500, prog({ start: 500, lines: ["z"], total: 501 }))).toEqual(["x", "y", "z"]);
  });
  it("appends when lines between were lost", () => {
    expect(linesOf(["a", "b"], 2, prog({ start: 50, lines: ["x"], total: 51 }))).toEqual(["a", "b", "x"]);
  });
  it("starts over when the answer begins before everything held", () => {
    expect(linesOf(["a", "b", "c"], 3, prog({ start: 0, lines: ["z"], total: 1 }))).toEqual(["z"]);
  });
  it("holds no more lines than the core keeps", () => {
    const many = Array.from({ length: 2_000 }, (_, i) => `l${i}`);
    const held = linesOf(many, 2_000, prog({ start: 2_000, lines: ["new"], total: 2_001 }));
    expect(held.length).toBe(2_000);
    expect(held[0]).toBe("l1");
    expect(held.at(-1)).toBe("new");
  });
  it("a long push shows no line twice", () => {
    let s = pushStep({ ...INITIAL_RUN, kind: "running", id: "p" }, { type: "progress", progress: prog({ start: 300, lines: ["a", "b 10%"], total: 302 }) });
    expect(s.seen).toBe(302);
    s = pushStep(s, { type: "progress", progress: prog({ start: 301, lines: ["b 100%", "c"], total: 303 }) });
    expect(s.lines).toEqual(["a", "b 100%", "c"]);
    expect(s.seen).toBe(303);
  });
});

describe("shouldFollow", () => {
  it("follows at the bottom and stops once scrolled up", () => {
    expect(shouldFollow(900, 100, 1000)).toBe(true);
    expect(shouldFollow(895, 100, 1000)).toBe(true);
    expect(shouldFollow(500, 100, 1000)).toBe(false);
  });
});
