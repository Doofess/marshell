import { describe, expect, it } from "vitest";
import { displayWidth, splitForMiddle } from "./truncate";

describe("splitForMiddle", () => {
  it.each([
    ["feat/auth-flow", "feat/auth", "-flow"],
    ["main", "", "main"],
    ["release/2026-10", "release/2026", "-10"],
    ["fix_the_thing", "fix_the", "_thing"],
    ["feature/very-long-final-segment-name", "feature/very-long-final-segment", "-name"],
    ["abcdefghijklmnop", "abcdefghij", "klmnop"],
    ["fix-🚀🚀🚀", "", "fix-🚀🚀🚀"],
  ])("%s", (s, head, tail) => {
    const r = splitForMiddle(s);
    expect(r.head + r.tail).toBe(s);
    expect(r.head).toBe(head);
    expect(r.tail).toBe(tail);
  });
  it("keeps a long final segment to its last 6 characters", () => {
    expect(splitForMiddle("topic/averyveryverylongsegment")).toEqual({ head: "topic/averyveryverylongs", tail: "egment" });
  });
  it("handles emoji without splitting a surrogate pair", () => {
    const r = splitForMiddle("fix-🚀🚀🚀🚀🚀🚀🚀🚀🚀");
    expect(r.head + r.tail).toBe("fix-🚀🚀🚀🚀🚀🚀🚀🚀🚀");
    expect([...r.tail].length).toBeLessThanOrEqual(8);
    expect(r.tail.charCodeAt(0) >= 0xdc00 && r.tail.charCodeAt(0) <= 0xdfff).toBe(false);
  });
  it("returns the whole string as tail when it is short", () => {
    expect(splitForMiddle("dev")).toEqual({ head: "", tail: "dev" });
  });
});

describe("displayWidth", () => {
  it.each([
    ["billing", 7],
    ["x", 1],
    ["認証サーバー 🚀", 15],
    ["프로젝트", 8],
    ["خادم-التحقق", 11],
    ["fix-🚀", 6],
  ])("%s → %i columns", (s, w) => expect(displayWidth(s)).toBe(w));
});
