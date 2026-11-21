import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./AppearanceSections.css", import.meta.url), "utf8");
const forcedAt = css.indexOf("@media (forced-colors: active)");

describe("AppearanceSections.css", () => {
  it("shows a disabled chip in GrayText in forced colors, where --text-2 is replaced", () => {
    expect(forcedAt).toBeGreaterThan(-1);
    const forced = css.slice(forcedAt);
    const rule = forced.match(/\.appearance__chip:has\(input:disabled\)\s*\{([^{}]*)\}/);
    expect(rule, "a disabled-chip rule inside forced-colors").not.toBeNull();
    expect(rule![1]).toMatch(/color:\s*GrayText\s*;/);
    expect(rule![1]).not.toContain("border-color");
    expect(forced).not.toMatch(/var\(|#[0-9a-f]{3}|oklch/i);
  });
});
