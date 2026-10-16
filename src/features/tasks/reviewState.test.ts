import { describe, expect, it } from "vitest";
import { initialReview, isBusy, reviewReducer, stepFocus, type ReviewAction, type ReviewState } from "./reviewState";

const run = (state: ReviewState, ...actions: ReviewAction[]) => actions.reduce(reviewReducer, state);
const at = (selected: number): ReviewState => ({ step: "changes", selected });

describe("reviewReducer: the file list", () => {
  it("starts on the changes step with the first file selected", () => {
    expect(initialReview()).toEqual({ step: "changes", selected: 0 });
  });
  it("moves down and up with the arrow keys", () => {
    expect(run(at(0), { type: "move", key: "ArrowDown", count: 3 }).selected).toBe(1);
    expect(run(at(2), { type: "move", key: "ArrowUp", count: 3 }).selected).toBe(1);
  });
  it("wraps at either end, as the launcher's lists do", () => {
    expect(run(at(2), { type: "move", key: "ArrowDown", count: 3 }).selected).toBe(0);
    expect(run(at(0), { type: "move", key: "ArrowUp", count: 3 }).selected).toBe(2);
  });
  it("jumps to the first and last file with Home and End", () => {
    expect(run(at(1), { type: "move", key: "End", count: 4 }).selected).toBe(3);
    expect(run(at(3), { type: "move", key: "Home", count: 4 }).selected).toBe(0);
  });
  it("ignores other keys and an empty list", () => {
    expect(run(at(1), { type: "move", key: "a", count: 3 })).toEqual(at(1));
    expect(run(at(0), { type: "move", key: "ArrowDown", count: 0 })).toEqual(at(0));
  });
  it("selects a file by index, ignoring one that is not there", () => {
    expect(run(at(0), { type: "select", index: 2, count: 3 }).selected).toBe(2);
    expect(run(at(1), { type: "select", index: 5, count: 3 }).selected).toBe(1);
    expect(run(at(1), { type: "select", index: -1, count: 3 }).selected).toBe(1);
  });
  it("only moves while the changes step is showing", () => {
    expect(run({ step: "merge", selected: 1 }, { type: "move", key: "ArrowDown", count: 3 })).toEqual({ step: "merge", selected: 1 });
  });
});

describe("reviewReducer: the steps", () => {
  it("goes from changes to merge or remove, and back", () => {
    expect(run(at(1), { type: "open", step: "merge" })).toEqual({ step: "merge", selected: 1 });
    expect(run(at(1), { type: "open", step: "remove" }).step).toBe("remove");
    expect(run(at(1), { type: "open", step: "remove" }, { type: "back" })).toEqual(at(1));
  });
  it("opens merge or remove only from changes", () => {
    expect(run({ step: "merge", selected: 0 }, { type: "open", step: "remove" }).step).toBe("merge");
    expect(run({ step: "done", selected: 0 }, { type: "open", step: "merge" }).step).toBe("done");
  });
  it("runs the merge: start, then done, a conflict, or back to the merge step on a failure", () => {
    const merge: ReviewState = { step: "merge", selected: 0 };
    expect(run(merge, { type: "start" }).step).toBe("merging");
    expect(run(merge, { type: "start" }, { type: "finish" }).step).toBe("done");
    expect(run(merge, { type: "start" }, { type: "conflict" }).step).toBe("conflict");
    expect(run(merge, { type: "start" }, { type: "fail" }).step).toBe("merge");
  });
  it("runs the removal: start, then done, or back to the remove step on a failure", () => {
    const remove: ReviewState = { step: "remove", selected: 0 };
    expect(run(remove, { type: "start" }).step).toBe("removing");
    expect(run(remove, { type: "start" }, { type: "finish" }).step).toBe("done");
    expect(run(remove, { type: "start" }, { type: "fail" }).step).toBe("remove");
  });
  it("leaves a running step alone until it ends: Back does nothing while merging or removing", () => {
    expect(run({ step: "merging", selected: 0 }, { type: "back" }).step).toBe("merging");
    expect(run({ step: "removing", selected: 0 }, { type: "back" }).step).toBe("removing");
  });
  it("goes back from a conflict to the changes", () => {
    expect(run({ step: "conflict", selected: 0 }, { type: "back" }).step).toBe("changes");
  });
  it("ignores an end signal that arrives in the wrong step", () => {
    expect(run(at(0), { type: "finish" }).step).toBe("changes");
    expect(run(at(0), { type: "conflict" }).step).toBe("changes");
    expect(run(at(0), { type: "start" }).step).toBe("changes");
  });
});

describe("isBusy", () => {
  it("is true only while the core is merging or removing", () => {
    expect(isBusy("merging")).toBe(true);
    expect(isBusy("removing")).toBe(true);
    for (const s of ["changes", "merge", "remove", "conflict", "done"] as const) expect(isBusy(s), s).toBe(false);
  });
});

describe("stepFocus: where focus goes when the step changes", () => {
  it("goes to the message field on the merge step, and to the confirming button on remove and conflict", () => {
    expect(stepFocus("changes", "merge")).toBe(".review__message");
    expect(stepFocus("changes", "remove")).toBe(".review__confirm");
    expect(stepFocus("merging", "conflict")).toBe(".review__confirm");
  });
  it("goes back to the file list from a step the user left with Back", () => {
    expect(stepFocus("merge", "changes")).toBe(".review__files");
    expect(stepFocus("conflict", "changes")).toBe(".review__files");
  });
  it("stays put when a run ends back on its step, so the button that has focus keeps it", () => {
    expect(stepFocus("merging", "merge")).toBeNull();
    expect(stepFocus("removing", "remove")).toBeNull();
  });
  it("does not move on the same step, or into the busy and done steps", () => {
    expect(stepFocus("merge", "merge")).toBeNull();
    expect(stepFocus("merge", "merging")).toBeNull();
    expect(stepFocus("remove", "removing")).toBeNull();
    expect(stepFocus("merging", "done")).toBeNull();
  });
});
