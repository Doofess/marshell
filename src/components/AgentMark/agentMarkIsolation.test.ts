import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./agent-colors.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("a vendor never inherits another vendor's mark colour", () => {
  // The window root carries the focused session's vendor, so every row inside it sits under that vendor's custom properties.
  // A property only some vendors set would leak down: another vendor's row showed Claude's orange mark.
  it("resets --agent-mark to the vendor's own colour for every vendor, before Claude's override", () => {
    const reset = css.search(/\[data-agent\]\s*\{[^}]*--agent-mark:\s*var\(--agent\)/);
    const claude = css.search(/\[data-agent="claude"\]\s*\{/);
    expect(reset).toBeGreaterThanOrEqual(0);
    expect(reset).toBeLessThan(claude);
  });
  it("lets only Claude override it", () => {
    const overrides = [...css.matchAll(/([^{}]+)\{[^}]*--agent-mark:\s*var\(--brand-[\w-]+\)/g)].map((m) => m[1]!.trim());
    expect(overrides).toEqual(['[data-agent="claude"]']);
  });
});
