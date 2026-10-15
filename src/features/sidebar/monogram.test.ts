import { describe, expect, it } from "vitest";
import { monogram } from "./monogram";

describe("monogram", () => {
  it("takes the initials of the first two words", () => {
    expect(monogram("api-server")).toBe("AS");
    expect(monogram("my_cool project")).toBe("MC");
  });
  it("takes the first two letters of one word", () => {
    expect(monogram("billing")).toBe("BI");
    expect(monogram("x")).toBe("X");
  });
  it("counts whole characters, not code units (review focus 1)", () => {
    expect(monogram("✅✅✅✅")).toBe("✅✅");
    expect(monogram("認証サーバー")).toBe("認証");
    expect(monogram("\u{1F680}rocket")).toBe("\u{1F680}R");
  });
  it("handles right-to-left names", () => {
    expect(monogram("خادم-التحقق").length).toBeGreaterThan(0);
  });
  it("returns nothing for an empty or blank name", () => {
    expect(monogram("")).toBe("");
    expect(monogram("   ")).toBe("");
  });
});
