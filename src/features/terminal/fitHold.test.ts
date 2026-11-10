import { describe, expect, it } from "vitest";
import { fitHold } from "./fitHold";

describe("fitHold", () => {
  it("a release runs the callback once", () => {
    let runs = 0;
    const h = fitHold(() => runs++);
    expect(h.held()).toBe(false);
    const release = h.hold();
    expect(h.held()).toBe(true);
    release();
    expect(runs).toBe(1);
    expect(h.held()).toBe(false);
    // A release called twice changes nothing.
    release();
    expect(runs).toBe(1);
  });

  it("the hold ends once after overlapping motions", () => {
    for (const order of [[0, 1], [1, 0]] as const) {
      let runs = 0;
      const h = fitHold(() => runs++);
      const releases = [h.hold(), h.hold()];
      releases[order[0]]!();
      expect(runs).toBe(0);
      expect(h.held()).toBe(true);
      releases[order[1]]!();
      expect(runs).toBe(1);
      expect(h.held()).toBe(false);
    }
  });

  it("a second release of a finished hold does not end another one", () => {
    let runs = 0;
    const h = fitHold(() => runs++);
    const first = h.hold();
    const second = h.hold();
    first();
    first();
    expect(runs).toBe(0);
    expect(h.held()).toBe(true);
    second();
    expect(runs).toBe(1);
  });
});
