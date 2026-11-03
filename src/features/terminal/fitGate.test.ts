import { describe, expect, it } from "vitest";
import { sendsFirstSize, shouldFit } from "./fitGate";

describe("sendsFirstSize", () => {
  it("sends a terminal's first size only once it is shown, and only once", () => {
    expect(sendsFirstSize(false, false)).toBe(false);
    expect(sendsFirstSize(true, false)).toBe(true);
    expect(sendsFirstSize(true, true)).toBe(false);
  });
});

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
