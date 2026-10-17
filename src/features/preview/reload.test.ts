import { describe, expect, it } from "vitest";
import { START, next, type ReloadState, type StatResult } from "./reload";

const CAP = 1000;
const ok = (size: number, modified_ms: number): StatResult => ({ kind: "ok", size, modified_ms });
const MISSING: StatResult = { kind: "missing" };
const step = (state: ReloadState, stat: StatResult) => next(state, stat, CAP);

describe("next", () => {
  it("loads the file the first time it is seen", () => {
    expect(step(START, ok(10, 1)).action).toBe("refetch");
  });
  it("keeps what it has while the stat is the same", () => {
    const first = step(START, ok(10, 1));
    expect(step(first.state, ok(10, 1)).action).toBe("keep");
  });
  it("refetches when the size or the modification time changes", () => {
    const { state } = step(START, ok(10, 1));
    expect(step(state, ok(11, 1)).action).toBe("refetch");
    expect(step(state, ok(10, 2)).action).toBe("refetch");
  });
  it("says deleted on a missing file, once", () => {
    const { state } = step(START, ok(10, 1));
    const gone = step(state, MISSING);
    expect(gone.action).toBe("deleted");
    expect(step(gone.state, MISSING).action).toBe("keep");
  });
  it("says deleted for a file that was never there", () => {
    expect(step(START, MISSING).action).toBe("deleted");
  });
  it("refetches when a deleted file comes back, even with the stat it had before", () => {
    const loaded = step(START, ok(10, 1)).state;
    const gone = step(loaded, MISSING).state;
    expect(step(gone, ok(10, 1)).action).toBe("refetch");
  });
  it("says tooLarge past the cap, and keeps quiet while it stays so", () => {
    const { state } = step(START, ok(10, 1));
    const big = step(state, ok(CAP + 1, 2));
    expect(big.action).toBe("tooLarge");
    expect(step(big.state, ok(CAP + 1, 2)).action).toBe("keep");
  });
  it("says tooLarge again when a large file changes, and refetches when it shrinks under the cap", () => {
    const big = step(START, ok(CAP + 1, 2));
    expect(step(big.state, ok(CAP + 5, 3)).action).toBe("tooLarge");
    expect(step(big.state, ok(CAP, 3)).action).toBe("refetch");
  });
  it("allows exactly the cap", () => {
    expect(step(START, ok(CAP, 1)).action).toBe("refetch");
  });
});
