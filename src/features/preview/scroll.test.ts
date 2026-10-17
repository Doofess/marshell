import { describe, expect, it } from "vitest";
import { restoreScroll } from "./scroll";

describe("restoreScroll", () => {
  const saved = { x: 12, y: 480 };
  it("gives back the saved position when the same file reloads", () => {
    expect(restoreScroll("C:\\p\\a.html", "C:\\p\\a.html", saved)).toEqual({ x: 12, y: 480 });
  });
  it("starts at the top for a different file", () => {
    expect(restoreScroll("C:\\p\\a.html", "C:\\p\\b.html", saved)).toBeNull();
  });
  it("has nothing to restore on the first load, or when nothing was saved", () => {
    expect(restoreScroll(null, "C:\\p\\a.html", saved)).toBeNull();
    expect(restoreScroll("C:\\p\\a.html", "C:\\p\\a.html", null)).toBeNull();
  });
});
