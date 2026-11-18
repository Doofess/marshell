import { describe, expect, it } from "vitest";
import { isUnsaved, samePath, staleDrafts, withUnsaved } from "./drafts";

const DAY = 24 * 60 * 60 * 1000;
const NOW = 100 * DAY;
const draft = (path: string, ageDays: number) => ({ path, edited_ms: NOW - ageDays * DAY });

describe("staleDrafts", () => {
  it("a_draft_with_a_tab_is_never_stale", () => {
    // A tab whose current entry shows the path keeps its draft however old it is, and the paths match across case and slash.
    expect(staleDrafts([draft("C:\\w\\a.txt", 400)], ["c:/w/A.txt"], NOW)).toEqual([]);
  });
  it("a_tabless_draft_is_stale_after_30_days", () => {
    expect(staleDrafts([draft("C:\\w\\a.txt", 29)], [], NOW)).toEqual([]);
    expect(staleDrafts([draft("C:\\w\\b.txt", 31)], [], NOW)).toEqual(["C:\\w\\b.txt"]);
    expect(staleDrafts([draft("C:\\w\\a.txt", 29), draft("C:\\w\\b.txt", 31), draft("C:\\w\\c.txt", 31)], ["C:\\w\\c.txt"], NOW)).toEqual(["C:\\w\\b.txt"]);
  });
});

describe("samePath", () => {
  it("paths_match_without_case_on_windows_only", () => {
    expect(samePath("C:\\w\\A.txt", "c:/w/a.txt", "windows")).toBe(true);
    expect(samePath("C:\\w\\A.txt", "C:\\w\\b.txt", "windows")).toBe(false);
    expect(samePath("/home/me/A.txt", "/home/me/a.txt", "linux")).toBe(false);
    expect(samePath("/home/me/A.txt", "/home/me/A.txt", "linux")).toBe(true);
    expect(samePath("/Users/me/A.txt", "/users/me/a.txt", "mac")).toBe(false);
  });
});

describe("the unsaved set", () => {
  it("answers_and_changes_by_the_same_path", () => {
    const one = withUnsaved(new Set<string>(), "C:\\w\\a.txt", true, "windows");
    expect(isUnsaved(one, "c:/w/A.txt", "windows")).toBe(true);
    expect(isUnsaved(one, "C:\\w\\b.txt", "windows")).toBe(false);
    // Marking it again keeps one entry; clearing it by another spelling removes it.
    expect(withUnsaved(one, "c:/w/A.txt", true, "windows")).toBe(one);
    expect([...withUnsaved(one, "c:/w/A.txt", false, "windows")]).toEqual([]);
    // Nothing to change returns the same set, so the window does not render for it.
    expect(withUnsaved(one, "C:\\w\\b.txt", false, "windows")).toBe(one);
  });
});
