import { describe, expect, it } from "vitest";
import { pruneClosed, without } from "./startingTexts";

const TEXTS = { a: "Starting Claude Code in app…", b: "Starting Codex CLI in api…" };

describe("without", () => {
  it("drops the tab whose first output arrived", () => {
    expect(without(TEXTS, "a")).toEqual({ b: TEXTS.b });
  });
  it("returns the same object when there is nothing to drop, so React does not re-render", () => {
    expect(without(TEXTS, "zzz")).toBe(TEXTS);
  });
});

describe("pruneClosed", () => {
  it("drops the text of a tab the core listed before and no longer lists", () => {
    expect(pruneClosed(TEXTS, new Set(["b"]), new Set(["a", "b"]))).toEqual({ b: TEXTS.b });
  });
  it("keeps the text of a tab whose row has not arrived yet", () => {
    expect(pruneClosed(TEXTS, new Set(), new Set())).toBe(TEXTS);
    expect(pruneClosed(TEXTS, new Set(["a"]), new Set(["a"]))).toBe(TEXTS);
  });
});
