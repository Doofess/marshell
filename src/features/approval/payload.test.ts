import { describe, expect, it } from "vitest";
import { COLLAPSED_LINES, estimateLines, isTruncated, payloadLines, showAllLabel } from "./payload";

describe("payloadLines", () => {
  it("splits a command on newlines", () => {
    expect(payloadLines({ kind: "bash", command: "a\nb\nc" })).toEqual(["a", "b", "c"]);
  });
  it("prefixes diff lines with their sign", () => {
    expect(
      payloadLines({ kind: "edit", path: "a.ts", add: 1, del: 1, hunk: [{ kind: "context", text: "x" }, { kind: "del", text: "y" }, { kind: "add", text: "z" }] }),
    ).toEqual(["  x", "- y", "+ z"]);
  });
  it("shows MCP arguments as key: value pairs", () => {
    expect(payloadLines({ kind: "mcp", server: "github", tool: "create_pr", args: { title: "Fix", base: "main" } })).toEqual(["title: Fix", "base: main"]);
  });
  it("shows the URL, the question and the file preview", () => {
    expect(payloadLines({ kind: "fetch", url: "https://a.dev/x" })).toEqual(["https://a.dev/x"]);
    expect(payloadLines({ kind: "question", question: "Which branch?", options: ["main"] })).toEqual(["Which branch?"]);
    expect(payloadLines({ kind: "write", path: "a", preview: "l1\nl2", isNew: true })).toEqual(["l1", "l2"]);
  });
});

describe("truncation", () => {
  it("collapses after four lines", () => {
    expect(COLLAPSED_LINES).toBe(4);
    expect(isTruncated(["1", "2", "3", "4"])).toBe(false);
    expect(isTruncated(["1", "2", "3", "4", "5"])).toBe(true);
  });
  it("counts a long unbroken line as several wrapped lines (review focus 1)", () => {
    expect(estimateLines(["x".repeat(44)], 44)).toBe(1);
    expect(estimateLines(["x".repeat(45)], 44)).toBe(2);
    expect(isTruncated(["x".repeat(400)], 44)).toBe(true);
  });
  it("counts an empty line as one", () => {
    expect(estimateLines(["", ""], 44)).toBe(2);
  });
  it("labels the expander with the real total", () => {
    expect(showAllLabel(23)).toBe("Show all 23 lines (Space)");
  });
});
