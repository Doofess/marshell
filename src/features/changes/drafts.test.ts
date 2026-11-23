import { describe, expect, it } from "vitest";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { DraftComment } from "../../generated/DraftComment";
import type { Placement } from "./comments";
import { elsewhere, sendMessageOf, shownComments, unkept, withFolds } from "./drafts";
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
