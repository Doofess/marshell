import { describe, expect, it } from "vitest";
import { browserTabs } from "./available";

describe("browserTabs", () => {
  it("is true on Windows and nowhere else", () => {
    expect(browserTabs("windows")).toBe(true);
    expect(browserTabs("mac")).toBe(false);
    expect(browserTabs("linux")).toBe(false);
    expect(browserTabs(undefined)).toBe(false);
  });
});
