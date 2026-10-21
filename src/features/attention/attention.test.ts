import { describe, expect, it } from "vitest";
import { badgeCount } from "./attention";

const e = (id: string, tab = "t1") => ({ id, tab });

describe("badgeCount", () => {
  it("counts sessions even when a tab has several requests", () => {
    expect(badgeCount([e("a"), e("b")])).toBe(1);
  });
  it("counts distinct tabs", () => {
    expect(badgeCount([e("a"), e("b", "t2")])).toBe(2);
  });
  it("is zero with no entries", () => {
    expect(badgeCount([])).toBe(0);
  });
});
