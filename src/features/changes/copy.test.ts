import { describe, expect, it } from "vitest";
import { copy, stripTitle } from "./copy";

describe("stripTitle", () => {
  it("is the panel's summary while a session shows it, and the panel's name with no session, whose frame draws no panel to take the last summary back (pre-merge round 4)", () => {
    expect(stripTitle("s1", "4 files · +120 −18")).toBe("4 files · +120 −18");
    expect(stripTitle(undefined, "4 files · +120 −18")).toBe("Changes");
  });
});

const line = (o: Partial<Parameters<typeof copy.line>[0]>) =>
  copy.line({ branch: "main", detached: null, base: "origin/main", ahead: 0, uncommitted: false, moved: false, folder: "C:\\r", ...o });

describe("copy", () => {
  it("writes the summary with counts and a minus sign", () => {
    expect(copy.summary(4, 120, 18)).toBe("4 files · +120 −18");
    expect(copy.summary(1, 1, 0)).toBe("1 file · +1 −0");
    expect(copy.summary(2, 1234, 5)).toBe("2 files · +1,234 −5");
    expect(copy.summary(0, 0, 0)).toBe("No changes");
  });
  it("uses U+2212 for the minus sign", () => {
    expect(copy.summary(1, 1, 1)).toContain("\u2212");
  });
  it("writes the line under the strip", () => {
    expect(line({ branch: "feat/login", ahead: 3, uncommitted: true })).toBe("feat/login · 3 commits ahead of origin/main · uncommitted work");
    expect(line({ ahead: 1 })).toBe("main · 1 commit ahead of origin/main");
  });
  it("leaves out commits at 0, and names a detached head and a moved checkout", () => {
    expect(line({ branch: null, detached: "abc1234", uncommitted: true, moved: true, folder: "C:\\other" })).toBe("Detached at abc1234 · uncommitted work · in C:\\other");
  });
  it("leaves out the commits when there is no base, and the name when there is no branch", () => {
    expect(line({ base: null, ahead: 2, uncommitted: true })).toBe("main · uncommitted work");
    expect(line({ branch: null, base: null, uncommitted: true })).toBe("uncommitted work");
  });
  it("holds the two base sentences", () => {
    expect(copy.noBase).toBe("No main branch found. Showing uncommitted work.");
    expect(copy.unrelated("origin/main")).toBe("No common history with origin/main. Showing uncommitted work.");
  });
  it("holds the sentence for a pull request base that isn't fetched", () => {
    expect(copy.prBaseUnfetched("develop")).toBe("This pull request targets develop, which isn't fetched here.");
  });
  it("holds the empty states", () => {
    expect(copy.emptyRepo("C:\\x")).toEqual({ title: "No git repository", line: "C:\\x isn't in a git checkout." });
    expect(copy.emptyNone("origin/main")).toEqual({ title: "No changes", line: "Nothing differs from origin/main." });
    expect(copy.emptyNone(null)).toEqual({ title: "No changes", line: "Nothing is uncommitted." });
    expect(copy.emptyFailed("git took too long.")).toEqual({ title: "Can't read the changes", line: "git took too long.", action: "Try again" });
  });
  it("says how many files a cut or a cap leaves out", () => {
    expect(copy.cut(3)).toBe("The diff is cut at 4 MiB. 3 more files aren't shown.");
    expect(copy.cut(1)).toBe("The diff is cut at 4 MiB. 1 more file isn't shown.");
    expect(copy.cut(1234)).toBe("The diff is cut at 4 MiB. 1,234 more files aren't shown.");
    expect(copy.moreNew(100)).toBe("And 100 more new files aren't shown.");
    expect(copy.moreNew(1)).toBe("And 1 more new file isn't shown.");
  });
  it("holds the file notes", () => {
    expect(copy.notes).toEqual({
      binary: "Binary file, not shown.",
      large: "New file over 256 KiB, not shown.",
      whitespace: "Only whitespace changed.",
      gone: "No changes in this file now.",
      cut: "This file is past the 4 MiB cut, so its lines aren't shown.",
    });
  });
  it("says a line or lines changed", () => {
    expect(copy.changed(false)).toBe("The line changed");
    expect(copy.changed(true)).toBe("The lines changed");
  });
});
