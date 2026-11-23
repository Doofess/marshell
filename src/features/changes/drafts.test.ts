import { describe, expect, it } from "vitest";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { DraftComment } from "../../generated/DraftComment";
import type { Placement } from "./comments";
import { boxAnchor, elsewhere, givesWay, keptBox, sendMessageOf, shownComments, unkept, withComments, withFolds } from "./drafts";
import type { Section } from "./model";

const A = "C:\\work\\repoA";
const B = "C:\\work\\repoB";
const c = (id: string, start: number): DraftComment => ({ id, path: "src/a.ts", side: "new", start, end: start, rows: 1, first: "f", hash: "h", quote: ["x"], text: `note ${id}` });
const onA: ChangesDrafts = { checkout: A, base: "main", comments: [c("1", 12)], folds: { "src/a.ts": false } };
const showingB = { folder: B, base: "origin/main" };
// B has a file at the same path, with a row the comment would match by its numbers.
const sectionB: Section = { path: "src/a.ts", status: "modified", added: 1, removed: 0, folded: false, rows: [{ kind: "add", newNo: 40, text: "x" }] };

describe("withFolds", () => {
  it("keeps the checkout and base the comments were written against", () => {
    expect(withFolds(onA, showingB, { "lock.json": true })).toEqual({ ...onA, folds: { "src/a.ts": false, "lock.json": true } });
  });
  it("takes the shown checkout when there are no comments", () => {
    expect(withFolds({ ...onA, comments: [] }, showingB, { "lock.json": true })).toEqual({ checkout: B, base: "origin/main", comments: [], folds: { "src/a.ts": false, "lock.json": true } });
    expect(withFolds(undefined, showingB, { x: true })).toEqual({ checkout: B, base: "origin/main", comments: [], folds: { x: true } });
  });
  it("keeps only the folds of files in the diff shown now, so they never pile up to the layout's limit (pre-merge round 3)", () => {
    const inDiff = new Set(["lock.json", "src/b.ts"]);
    const had = { ...onA, comments: [], folds: { "src/a.ts": false, "src/b.ts": true } };
    expect(withFolds(had, showingB, { "lock.json": true }, inDiff).folds).toEqual({ "src/b.ts": true, "lock.json": true });
    // A fold asked for a file that isn't in the diff is not kept either.
    expect(withFolds(undefined, showingB, { "gone.ts": true, "lock.json": false }, inDiff).folds).toEqual({ "lock.json": false });
  });
});

describe("withComments", () => {
  const showingA = { folder: A, base: "origin/develop" };
  it("keeps the base the earlier comments were written against when one more is added", () => {
    const next = [...onA.comments, c("2", 20)];
    expect(withComments(onA, showingA, next)).toEqual({ ...onA, comments: next });
  });
  it("takes the shown checkout and base for the first comment, and keeps the folds", () => {
    expect(withComments({ ...onA, comments: [] }, showingA, [c("1", 1)])).toEqual({ checkout: A, base: "origin/develop", comments: [c("1", 1)], folds: { "src/a.ts": false } });
    expect(withComments(undefined, showingA, [c("1", 1)])).toEqual({ checkout: A, base: "origin/develop", comments: [c("1", 1)], folds: {} });
  });
});

describe("keptBox", () => {
  const { id: _id, text: _text, ...anchor } = c("9", 30);
  const same = (x: DraftComment) => x;
  it("keeps a new comment on the checkout its box was opened on, even when another is shown by now", () => {
    const kept = keptBox(undefined, { state: { anchor, id: null }, text: "typed", on: { folder: A, base: "main" } }, anchor, "n1", same);
    expect(kept).toEqual({ checkout: A, base: "main", comments: [{ ...anchor, id: "n1", text: "typed" }], folds: {} });
  });
  it("adds after the earlier comments, and an edit changes only its own text", () => {
    const added = keptBox(onA, { state: { anchor, id: null }, text: "two", on: { folder: A, base: "main" } }, anchor, "n2", same);
    expect(added.comments.map((x) => x.id)).toEqual(["1", "n2"]);
    const edited = keptBox(onA, { state: { anchor: c("1", 12), id: "1" }, text: "new words", on: { folder: A, base: "main" } }, anchor, "unused", same);
    expect(edited.comments).toEqual([{ ...c("1", 12), text: "new words" }]);
  });
});

describe("keptBox for a comment that is gone", () => {
  const { id: _id, text: _text, ...anchor } = c("1", 12);
  it("keeps an edit of a comment deleted or sent meanwhile as a new comment, so its text isn't lost (pre-merge round 4)", () => {
    const sent: ChangesDrafts = { ...onA, comments: [c("2", 20)] };
    const kept = keptBox(sent, { state: { anchor, id: "1" }, text: "edited words", on: { folder: A, base: "main" } }, anchor, "n3", (x) => x);
    expect(kept.comments).toEqual([c("2", 20), { ...anchor, id: "n3", text: "edited words" }]);
  });
});

describe("givesWay", () => {
  const { id: _id, text: _text, ...anchor } = c("9", 30);
  const { id: _i2, text: _t2, ...other } = c("9", 44);
  const on = { folder: A, base: "main" };
  it("opens the box when none is open, or the open one holds no text", () => {
    expect(givesWay(null, { anchor, id: null })).toBe("open");
    expect(givesWay({ state: { anchor: other, id: null }, text: "  ", on }, { anchor, id: null })).toBe("open");
  });
  it("keeps the open box's typed text before another box opens, as Esc keeps it (pre-merge round 4)", () => {
    expect(givesWay({ state: { anchor: other, id: null }, text: "typed", on }, { anchor, id: null })).toBe("keep");
    expect(givesWay({ state: { anchor: other, id: "7" }, text: "typed", on }, { anchor, id: "1" })).toBe("keep");
    // A new comment on the lines of the comment being edited is another box.
    expect(givesWay({ state: { anchor, id: "7" }, text: "typed", on }, { anchor, id: null })).toBe("keep");
  });
  it("leaves the open box as it is when the same one is asked for again, so its text isn't reset", () => {
    expect(givesWay({ state: { anchor, id: null }, text: "typed", on }, { anchor, id: null })).toBe("same");
    expect(givesWay({ state: { anchor, id: null }, text: "", on }, { anchor, id: null })).toBe("same");
    expect(givesWay({ state: { anchor: other, id: "7" }, text: "typed", on }, { anchor, id: "7" })).toBe("same");
  });
});

describe("boxAnchor", () => {
  const { id: _id, text: _text, ...anchor } = c("9", 30);
  const box = { state: { anchor, id: null }, text: "typed", on: { folder: A, base: "main" } };
  const placed: Placement = { at: 3, changed: false, start: 40, end: 41 };
  it("takes the lines its rows are on now, on the checkout the box was opened on", () => {
    expect(boxAnchor(box, A, placed)).toEqual({ ...anchor, start: 40, end: 41 });
    expect(boxAnchor(box, A, undefined)).toEqual(anchor);
  });
  it("keeps the lines it was opened at when another checkout is shown by now, whose file of the same name isn't its own (pre-merge round 3)", () => {
    expect(boxAnchor(box, B, placed)).toEqual(anchor);
  });
});

describe("elsewhere", () => {
  it("names the checkout the comments are on when another is shown", () => {
    expect(elsewhere(onA, B)).toBe("Send or discard the 1 comment on repoA first.");
    expect(elsewhere({ ...onA, comments: [c("1", 1), c("2", 2)] }, B)).toBe("Send or discard the 2 comments on repoA first.");
  });
  it("is null on their own checkout or with no comments", () => {
    expect(elsewhere(onA, A)).toBeNull();
    expect(elsewhere({ ...onA, comments: [] }, B)).toBeNull();
    expect(elsewhere(undefined, B)).toBeNull();
  });
});

describe("shownComments", () => {
  it("lists another checkout's comments nowhere in the shown diff", () => {
    expect(shownComments(onA, B)).toEqual([]);
    expect(shownComments(onA, A)).toEqual(onA.comments);
  });
});

describe("sendMessageOf", () => {
  it("cites another checkout's comments where they were written, against its base, naming it", () => {
    const placements = new Map<string, Placement>([["1", { at: 0, changed: false, start: 40, end: 40 }]]);
    expect(sendMessageOf(onA, showingB, [sectionB], placements)).toBe(
      ["Comments on the changes in C:\\work\\repoA (compared with main):", "src/a.ts:12\n> x\nnote 1"].join("\n\n"),
    );
  });
  it("names the folder of uncommitted changes too", () => {
    expect(sendMessageOf({ ...onA, base: null }, showingB, [], new Map())).toMatch(/^Comments on the uncommitted changes in C:\\work\\repoA:\n\n/);
  });
  it("cites the shown checkout's comments at the lines they are on now", () => {
    const onB: ChangesDrafts = { ...onA, checkout: B };
    const placements = new Map<string, Placement>([["1", { at: 0, changed: false, start: 40, end: 40 }]]);
    expect(sendMessageOf(onB, showingB, [sectionB], placements)).toBe(["Comments on the changes (compared with origin/main):", "src/a.ts:40\n> x\nnote 1"].join("\n\n"));
  });
});

describe("unkept", () => {
  // The layout keeps a draft's path only relative with / separators and no colon, as the core's rule for saved paths.
  it("says why a file whose name holds a colon or a backslash can't hold a comment or a fold", () => {
    const why = "This file's name holds a colon or a backslash, so a comment or a fold on it can't be kept.";
    expect(unkept("x:y.ts")).toBe(why);
    expect(unkept("dir\\b.ts")).toBe(why);
    expect(unkept("src/a.ts")).toBeNull();
  });
});
