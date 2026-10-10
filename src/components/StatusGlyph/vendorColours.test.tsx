import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ContextRing } from "../ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "./StatusGlyph";

const glyphCss = readFileSync(new URL("./StatusGlyph.css", import.meta.url), "utf8");
const ringCss = readFileSync(new URL("../ContextRing/ContextRing.css", import.meta.url), "utf8");

describe("the done check, caution triangle and context ring take the vendor's colour", () => {
  it("draws both done checks in the vendor's colour, falling back to the app's outside a session", () => {
    expect(glyphCss).toMatch(/\[data-kind="done-unseen"\]\s*\{[^}]*color:\s*var\(--agent-signal, var\(--ok\)\)/);
    expect(glyphCss).toMatch(/\[data-kind="done-seen"\]\s*\{[^}]*color:\s*var\(--agent-signal, var\(--ok-dim\)\)/);
  });
  it("tells a seen check from an unseen one by a thinner stroke, so neither needs transparency", () => {
    const stroke = (kind: "done-seen" | "done-unseen") => Number(/stroke-width="([\d.]+)"/.exec(renderToStaticMarkup(<StatusGlyph kind={kind} />))?.[1]);
    expect(stroke("done-seen")).toBeLessThan(stroke("done-unseen"));
    expect(glyphCss).not.toMatch(/done-seen[^}]*opacity/);
  });
  it("draws the caution triangle in the vendor's colour", () => {
    expect(glyphCss).toMatch(/\.aux-glyph\[data-kind="caution"\]\s*\{[^}]*color:\s*var\(--agent-signal, var\(--caution\)\)/);
    expect(renderToStaticMarkup(<AuxGlyph kind="caution" />)).toContain("<path d=\"M8 2l6.25 11");
  });
  it("draws the ring in the vendor's colour at every level, never in caution yellow or error red", () => {
    expect(ringCss).toMatch(/\.context-ring__value[^{]*\{[^}]*stroke:\s*var\(--agent-signal, var\(--text-2\)\)/);
    expect(ringCss).not.toContain("--caution");
    expect(ringCss).not.toContain("--error");
  });
});

describe("the ring's level shows in its shape, not its colour", () => {
  const ring = (pct: number) => renderToStaticMarkup(<ContextRing pct={pct} />);
  const width = (h: string) => Number(/class="context-ring__value"[^>]*stroke-width="([\d.]+)"/.exec(h)?.[1]);
  it("thickens the ring from 80%", () => {
    expect(width(ring(79))).toBe(2);
    expect(width(ring(80))).toBeGreaterThan(2);
    expect(width(ring(94))).toBe(width(ring(80)));
  });
  it("adds a centre dot from 95%, and only then", () => {
    expect(ring(94)).not.toContain("context-ring__dot");
    expect(ring(95)).toContain("context-ring__dot");
    expect(ring(100)).toContain("context-ring__dot");
  });
  it("still says the level in words", () => {
    expect(ring(96)).toContain('aria-label="Context 96% used"');
  });
});
