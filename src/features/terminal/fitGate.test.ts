import { describe, expect, it } from "vitest";
import { shouldFit } from "./fitGate";

describe("shouldFit", () => {
  it("skips a hidden terminal, whatever its host measures", () => {
    expect(shouldFit(false, 800, 600)).toBe(false);
  });

  it("skips a host that measures empty", () => {
    expect(shouldFit(true, 0, 600)).toBe(false);
    expect(shouldFit(true, 800, 0)).toBe(false);
  });

  it("fits a shown terminal with a real size", () => {
    expect(shouldFit(true, 800, 600)).toBe(true);
  });
});
