import { describe, expect, it } from "vitest";
import { needsYouCount } from "./attention";

const e = (id: string, tab = "t1") => ({ id, tab });

describe("needsYouCount", () => {
  it("counts sessions even when a tab has several requests", () => {
    expect(needsYouCount([e("a"), e("b")])).toBe(1);
  });
  it("counts distinct tabs", () => {
    expect(needsYouCount([e("a"), e("b", "t2")])).toBe(2);
  });
  it("is zero with no entries", () => {
    expect(needsYouCount([])).toBe(0);
  });
});
