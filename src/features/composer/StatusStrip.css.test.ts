import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { COMPACT_BELOW, HIDE_BELOW, PRIORITY } from "./stripItems";

const css = readFileSync(new URL("./StatusStrip.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};
/** The container width below which a selector's rule applies. */
const below = (selector: string): number[] => [...css.matchAll(/@container \(inline-size < (\d+)px\) \{\s*([^{]+)\{/g)].filter((m) => m[2]!.includes(selector)).map((m) => Number(m[1]));

describe("StatusStrip.css", () => {
  it("is one 20 px line that never wraps and clips instead of overflowing", () => {
    const r = rule(".status-strip");
    expect(r).toContain("block-size: 20px");
    expect(r).toContain("white-space: nowrap");
    expect(r).toContain("overflow: clip");
    expect(r).toContain("font-size: var(--text-11)");
    expect(r).toContain("font-variant-numeric: tabular-nums");
  });
  it("draws separators with CSS, none before the first item", () => {
    expect(css).toContain(".status-strip__item:not([data-first])::before");
    expect(rule(".status-strip__item:not([data-first])::before")).toContain("var(--hairline)");
  });
  it("hides each droppable item below the width the TS table names, and only items that table names", () => {
    for (const [id, px] of Object.entries(HIDE_BELOW)) expect(below(`[data-id="${id}"]`), id).toContain(px);
    const named = new Set(Object.keys(HIDE_BELOW));
    for (const m of css.matchAll(/\[data-id="(\w+)"\](?:\[data-kind="ok"\])?:not\(\[data-first\]\)\s*\{/g)) expect(named.has(m[1]!), m[1]).toBe(true);
  });
  it("shortens a meter by hiding its detail, then its bar, at the widths the TS table names", () => {
    const at = (id: string, part: string) => below(`[data-id="${id}"] .status-strip__${part}`);
    expect(at("limit7", "detail")).toContain(COMPACT_BELOW.limit7Detail);
    expect(at("limit7", "bar")).toContain(COMPACT_BELOW.limit7Bar);
    expect(at("model", "detail")).toContain(COMPACT_BELOW.modelDetail);
    expect(at("limit5", "detail")).toContain(COMPACT_BELOW.limit5Detail);
    expect(at("limit5", "bar")).toContain(COMPACT_BELOW.limit5Bar);
    expect(at("ctx", "detail")).toContain(COMPACT_BELOW.ctxDetail);
    expect(at("ctx", "bar")).toContain(COMPACT_BELOW.ctxBar);
  });
  it("degrades in priority order: every width in the table falls as the item's priority rises", () => {
    const steps = [
      [PRIORITY.venv, HIDE_BELOW.venv],
      [PRIORITY.version, HIDE_BELOW.version],
      [PRIORITY.dirty, HIDE_BELOW.dirty],
      [PRIORITY.skills, HIDE_BELOW.skills],
      [PRIORITY.ports, HIDE_BELOW.ports],
      [PRIORITY.mcpOk, HIDE_BELOW.mcp],
      [PRIORITY.time, HIDE_BELOW.time],
      [PRIORITY.cost, HIDE_BELOW.cost],
      [PRIORITY.limit7, COMPACT_BELOW.limit7Detail],
      [PRIORITY.limit7, COMPACT_BELOW.limit7Bar],
      [PRIORITY.limit7, HIDE_BELOW.limit7],
      [PRIORITY.model, COMPACT_BELOW.modelDetail],
      [PRIORITY.limit5, COMPACT_BELOW.limit5Detail],
      [PRIORITY.limit5, COMPACT_BELOW.limit5Bar],
      [PRIORITY.ctx, COMPACT_BELOW.ctxDetail],
      [PRIORITY.ctx, COMPACT_BELOW.ctxBar],
    ] as const;
    for (let i = 1; i < steps.length; i++) {
      expect(steps[i]![0], `step ${i} priority`).toBeGreaterThanOrEqual(steps[i - 1]![0]);
      expect(steps[i]![1], `step ${i} width`).toBeLessThan(steps[i - 1]![1]);
    }
  });
  it("never hides the core items, a named problem or the first item", () => {
    for (const id of ["model", "ctx", "limit5"]) expect(css).not.toMatch(new RegExp(`\\[data-id="${id}"\\]:not\\(\\[data-first\\]\\)`));
    expect(css).not.toContain('[data-kind="problem"]:not');
    expect(css).toContain('[data-id="mcp"][data-kind="ok"]:not([data-first])');
  });
  it("fits in script too, keyed on the compaction level, and leaves the container rules to the stylesheet-fitted strip", () => {
    expect(css).toContain('.status-strip[data-fit="js"] [data-compact="1"] .status-strip__detail');
    expect(css).toContain('.status-strip[data-fit="js"] [data-compact="2"] .status-strip__bar');
    for (const m of css.matchAll(/@container[^{]*\{\s*([^{]+)\{/g)) expect(m[1]).toContain('[data-fit="css"]');
  });
  it("shows a meter's level in its bar and weight, with stage colours only on the bar and only from tokens", () => {
    expect(rule('.status-strip__item[data-tone="caution"] .status-strip__bar > i')).toContain("var(--caution)");
    expect(rule('.status-strip__item[data-tone="error"] .status-strip__bar > i')).toContain("var(--error)");
    expect(rule(".status-strip__bar > i")).toContain("var(--agent-signal");
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("uses logical properties only and has no hover, since nothing in the strip is pressable", () => {
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/(?<![-\w])(left|right|top|bottom|width|height):/);
    expect(css).not.toContain(":hover");
  });
  it("keeps the bar and divider visible in Windows high contrast", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*CanvasText/);
  });
});

describe("StatusStrip: room to read", () => {
  it("spaces the parts of an item with a gap (flex drops the typed spaces) and the items with a wide separator", () => {
    const item = rule(".status-strip__item");
    expect(item).toMatch(/gap:\s*6px/);
    expect(item).toMatch(/margin-inline-start:\s*14px/);
    expect(item).toMatch(/padding-inline-start:\s*15px/);
  });
});
