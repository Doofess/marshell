import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..");
const walk = (dir: string): string[] => readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]));
const base = readFileSync(join(SRC, "styles/base.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/**
 * Browsers draw a text box, a select or a number input with their own raised or inset border. That is a boxed outline nobody
 * wrote, and no declaration scan can see it. So the base stylesheet flattens every text control at zero specificity (any class
 * can give it a fill), and no control in the app may be a bare element that relies on the browser's look.
 */
describe("form controls start flat", () => {
  it("has a zero-specificity reset for text inputs, selects and text areas: no border, no background, the page's font and colour", () => {
    const m = /:where\(([^)]*(?:\([^)]*\)[^)]*)*)\)\s*\{([^}]*)\}/.exec(base);
    expect(m, "base.css has no :where() reset").not.toBeNull();
    const selectors = m![1]!;
    for (const el of ["input", "select", "textarea"]) expect(selectors).toContain(el);
    expect(m![2]).toMatch(/border:\s*0/);
    expect(m![2]).toMatch(/background:\s*none/);
    expect(m![2]).toMatch(/font:\s*inherit/);
    expect(m![2]).toMatch(/color:\s*inherit/);
  });
  it("leaves checkboxes and radios to their native drawing, which is the control's own boundary", () => {
    const sel = /:where\(([^)]*(?:\([^)]*\)[^)]*)*)\)\s*\{/.exec(base)![1]!;
    expect(sel).toMatch(/:not\(\[type="checkbox"\]\)/);
    expect(sel).toMatch(/:not\(\[type="radio"\]\)/);
  });
  it("has no bare text control in the app: each one carries a class that gives it a fill", () => {
    const tsx = walk(SRC).filter((f) => f.endsWith(".tsx") && !/\.(test|stories)\.tsx$/.test(f));
    const css = walk(SRC).filter((f) => f.endsWith(".css")).map((f) => readFileSync(f, "utf8").replace(/\/\*[\s\S]*?\*\//g, "")).join("\n");
    let seen = 0;
    for (const f of tsx) {
      const text = readFileSync(f, "utf8");
      for (const m of text.matchAll(/<(input|select|textarea)\b([\s\S]*?)(?:\/>|>)/g)) {
        const attrs = m[2]!;
        if (/type="(?:checkbox|radio)"/.test(attrs)) continue;
        seen++;
        const cls = /className="([^"]+)"/.exec(attrs)?.[1];
        expect(cls, `${f}: <${m[1]}> has no className`).toBeDefined();
        const rule = new RegExp(`\\.${cls!.split(" ")[0]}\\s*\\{[^}]*\\}`).exec(css);
        expect(rule?.[0], `.${cls} has no rule`).toMatch(/background(?:-color)?:/);
      }
    }
    expect(seen).toBeGreaterThanOrEqual(5);
  });
});
