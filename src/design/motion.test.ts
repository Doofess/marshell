import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("../styles/motion.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** Splits the stylesheet into top-level blocks: [prelude, body]. */
function topLevelBlocks(src: string): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  let depth = 0;
  let start = 0;
  let prelude = "";
  for (let i = 0; i < src.length; i++) {
    if (src[i] === "{") {
      if (depth === 0) {
        prelude = src.slice(start, i).trim();
        start = i + 1;
      }
      depth++;
    } else if (src[i] === "}") {
      depth--;
      if (depth === 0) {
        out.push([prelude, src.slice(start, i)]);
        start = i + 1;
      }
    }
  }
  return out;
}

const blocks = topLevelBlocks(css);

describe("motion.css", () => {
  it("runs full-motion animations only when motion is allowed and not switched off", () => {
    for (const [prelude, body] of blocks) {
      if (prelude.startsWith("@keyframes") || !/animation\s*:/.test(body)) continue;
      const reduced = prelude.includes("prefers-reduced-motion: reduce") || prelude.includes('[data-motion="reduce"]');
      const names = [...body.matchAll(/animation\s*:\s*([\w-]+)/g)].map((m) => m[1]!).filter((n) => n !== "none");
      if (reduced) {
        for (const n of names) expect(n, `${prelude} runs ${n}`).toMatch(/-reduced$/);
      } else {
        expect(prelude, `animation outside the motion gate: ${prelude}`).toContain("prefers-reduced-motion: no-preference");
        for (const rule of body.split("}").filter((r) => /animation\s*:/.test(r)))
          expect(rule, "full-motion rule must also check the app switch").toContain(':root:not([data-motion="reduce"])');
      }
    }
  });
  it("pauses every animation while the page is hidden", () => {
    expect(css).toMatch(/:root\[data-page-hidden\][^{]*\{[^}]*animation-play-state:\s*paused/);
  });
  it("lets the row flash fade to the row's own background", () => {
    const flash = blocks.find(([p]) => p === "@keyframes row-flash")?.[1] ?? "";
    expect(flash).toContain("from");
    expect(flash).not.toMatch(/\bto\b/);
  });
  it("gives the reduced-motion flash its own non-moving keyframes that end on their own", () => {
    expect(blocks.some(([p]) => p === "@keyframes row-flash-reduced")).toBe(true);
    expect(css).not.toMatch(/\[data-flash\][^{]*\{[^}]*transition/);
  });
  it("never bounces a muted session's badge", () => {
    expect(css).toMatch(/\.session-row\[data-muted\][^{]*\.status-glyph[^{]*\{[^}]*animation:\s*none/);
  });
});
