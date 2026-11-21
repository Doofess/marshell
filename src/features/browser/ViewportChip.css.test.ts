import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./ViewportChip.css", import.meta.url), "utf8");
const forcedAt = css.indexOf("@media (forced-colors: active)");
const outside = css.slice(0, forcedAt < 0 ? css.length : forcedAt);

describe("ViewportChip.css", () => {
  it("has no border or outline declaration outside :focus-visible and forced-colors", () => {
    const rules = [...outside.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter((m) => !m[1]!.includes(":focus-visible"));
    expect(rules.length).toBeGreaterThan(0);
    for (const m of rules) expect(m[2], m[1]!.trim()).not.toMatch(/\b(border|outline)(-width|-style|-color)?\s*:/);
  });
  it("names system colors only inside forced-colors", () => {
    expect(forcedAt).toBeGreaterThan(-1);
    const forced = css.slice(forcedAt);
    expect(forced).toContain("viewport-chip");
    expect(forced).not.toMatch(/var\(|#[0-9a-f]{3}|oklch/i);
    expect(forced).toMatch(/CanvasText|ButtonText|Highlight/);
  });
});
