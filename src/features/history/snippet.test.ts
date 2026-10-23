import { describe, expect, it } from "vitest";
import { splitSnippet } from "./snippet";

describe("splitSnippet", () => {
  it("splits hits out of the text", () => {
    expect(splitSnippet("a \u0002b\u0003 c")).toEqual([
      { text: "a ", hit: false },
      { text: "b", hit: true },
      { text: " c", hit: false },
    ]);
  });
  it("leaves out empty segments", () => {
    expect(splitSnippet("\u0002x\u0003")).toEqual([{ text: "x", hit: true }]);
    expect(splitSnippet("")).toEqual([]);
  });
  it("runs an unmatched open mark to the end", () => {
    expect(splitSnippet("a \u0002bc")).toEqual([
      { text: "a ", hit: false },
      { text: "bc", hit: true },
    ]);
  });
  it("drops stray close marks", () => {
    expect(splitSnippet("a\u0003b")).toEqual([{ text: "ab", hit: false }]);
  });
});
