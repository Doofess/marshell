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
    expect(forcedRule(".changes__box-error")).toContain("color: CanvasText");
    expect(forcedRule(".changes__box-text")).toContain("border: 1px solid CanvasText");
  });

  // A GitHub thread is a second tint of the draft's look: tokens only, and in forced colours the draft's system-colour edge.
  it("draws GitHub's threads with tokens, and gives them the draft's system colours in forced colours", () => {
    const THREAD_CLASSES = ["changes__thread", "changes__thread-head", "changes__thread-chip", "changes__thread-comment", "changes__thread-body", "changes__thread-note", "changes__talk", "changes__line"];
    const rules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter((m) => THREAD_CLASSES.some((c) => m[1]!.includes(`.${c}`)));
    for (const c of THREAD_CLASSES) expect(rules.some((m) => m[1]!.includes(`.${c}`)), c).toBe(true);
    for (const m of rules) expect(m[2], m[1]!.trim()).not.toMatch(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i);
    expect(forcedRule(".changes__thread")).toContain("background: Canvas");
    expect(forcedRule(".changes__thread")).toContain("color: CanvasText");
  });

  // A thread and a talk item keep their own colours (forced-color-adjust: none), which every part of them inherits: the Markdown's paper and ink and each text part are set to system colours.
  it("gives every part of a GitHub comment system colours in forced colours", () => {
    for (const body of [".changes__thread .changes__thread-body", ".changes__talk .changes__thread-body"]) {
      const rule = forcedRule(body);
      expect(rule, body).toContain("--ink: CanvasText");
      expect(rule, body).toContain("--paper: Canvas");
      expect(rule, body).toContain("--accent: LinkText");
      expect(rule, body).toContain("--hairline: CanvasText");
    }
    for (const part of [".changes__thread-chip", '.changes__thread-comment [data-part="meta"]', '.changes__thread-comment [data-part="author"]', '.changes__thread [data-part="place"]', ".changes__thread-note", ".changes__thread-quote", ".changes__list .changes__thread-body .markdown__info"]) {
      expect(forcedRule(part), part).toContain("color: CanvasText");
    }
    expect(forcedRule(".changes__thread-chip")).toContain("border: 1px solid CanvasText");
  });
  it("draws the quote of a thread's code in the mono font and wraps its long lines", () => {
    const rule = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter((m) => m[1]!.trim() === ".changes__thread-quote").map((m) => m[2]).join("\n");
    expect(rule).toContain("font-family: var(--font-mono)");
    expect(rule).toContain("white-space: pre-wrap");
    expect(rule).toContain("overflow-wrap: anywhere");
    expect(rule).not.toMatch(/overflow(-[xy])?:\s*(auto|scroll)/);
  });

  // The rows that keep their own colours (forced-color-adjust: none) must not keep the theme's ring, hover or selection fill.
  it("gives the rows' focus ring, hover and selection system colours in forced colours", () => {
    expect(forcedRule(".changes__item:focus-visible")).toContain("outline-color: CanvasText");
    expect(forcedRule(".changes__item:hover")).toContain("background-image: none");
    expect(forcedRule(".changes__item:active")).toContain("background: Canvas");
    expect(forcedRule('.changes__item[aria-selected="true"]')).toContain("box-shadow: none");
  });
});
