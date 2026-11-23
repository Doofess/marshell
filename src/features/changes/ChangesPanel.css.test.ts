import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./ChangesPanel.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");

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
    const block = css.slice(from, i - 1);
    for (const r of block.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (r[1]!.split(",").some((s) => s.trim() === selector)) out.push(r[2]!);
    }
  }
  return out.join("\n");
}

describe("ChangesPanel.css", () => {
  it("gives the syntax colours back to the system in forced colours, as the preview does", () => {
    // The rows set forced-color-adjust: none, which their token spans inherit: each token colour must fall back to the text's.
    const names = [...new Set([...tokens.matchAll(/(--code-token-[a-z-]+):/g)].map((m) => m[1]!))];
    expect(names.length).toBeGreaterThan(10);
    const rule = forcedRule(".changes");
    for (const name of names) expect(rule, name).toContain(`${name}: initial`);
  });

  // A fill isn't drawn in forced colours: the Send bar's buttons and the comment box keep an edge, in a system colour.
  it("gives the Send bar's buttons and the comment box an edge in forced colours", () => {
    expect(forcedRule(".changes__send-button")).toContain("border: 1px solid ButtonText");
    expect(forcedRule(".changes__send-button--primary")).toContain("border: 1px solid Highlight");
    expect(forcedRule(".changes__box-text")).toContain("border: 1px solid CanvasText");
  });

  // The rows that keep their own colours (forced-color-adjust: none) must not keep the theme's ring, hover or selection fill.
  it("gives the rows' focus ring, hover and selection system colours in forced colours", () => {
    expect(forcedRule(".changes__item:focus-visible")).toContain("outline-color: CanvasText");
    expect(forcedRule(".changes__item:hover")).toContain("background-image: none");
    expect(forcedRule(".changes__item:active")).toContain("background: Canvas");
    expect(forcedRule('.changes__item[aria-selected="true"]')).toContain("box-shadow: none");
  });
});
