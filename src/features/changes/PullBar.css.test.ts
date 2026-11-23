import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./PullBar.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** The declarations of the rule for `selector` inside the `@media (forced-colors: active)` blocks. */
function forcedRule(selector: string): string {
  const out: string[] = [];
  for (const m of css.matchAll(/@media \(forced-colors: active\) \{/g)) {
    let depth = 1;
    let i = m.index! + m[0].length;
    const from = i;
    while (depth > 0 && i < css.length) {
      if (css[i] === "{") depth++;
      else if (css[i] === "}") depth--;
      i++;
    }
    for (const r of css.slice(from, i - 1).matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (r[1]!.split(",").some((s) => s.trim() === selector)) out.push(r[2]!);
    }
  }
  return out.join("\n");
}

describe("PullBar.css", () => {
  it("draws an edge in forced colours", () => {
    expect(forcedRule(".pull-bar")).toContain("1px solid CanvasText");
    expect(forcedRule(".pull-bar__button")).toContain("border: 1px solid ButtonText");
    expect(forcedRule(".pull-bar__glyph")).toContain("color: CanvasText");
  });
  it("keeps the focus ring visible in forced colours", () => {
    expect(css).toMatch(/\.pull-bar__check:focus-visible/);
    expect(forcedRule(".pull-bar__button:focus-visible")).toContain("outline-color: Highlight");
  });
  it("greys a button that is off with a system colour", () => {
    expect(forcedRule('.pull-bar__button[aria-disabled="true"]')).toContain("GrayText");
  });
});
describe("PullBar.css: forced colours", () => {
  it("lets the system force the colours of the notes, the reason and the menus inside the bar", () => {
    expect(css).not.toContain("forced-color-adjust");
  });
});
