import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AUX_LABELS, GLYPHS, type AuxKind, type GlyphKind } from "./glyphs";
import { AuxGlyph, StatusGlyph } from "./StatusGlyph";

const kinds = Object.keys(GLYPHS) as GlyphKind[];

describe("glyph registry", () => {
  it("covers the ten row states", () => {
    expect(kinds.sort()).toEqual(
      ["done-seen", "done-unseen", "ended", "error", "idle", "limited", "needs-permission", "needs-question", "stuck", "working"],
    );
  });
  it("every motion has a reduced form", () => {
    for (const k of kinds) {
      const g = GLYPHS[k];
      if (g.motion) expect(g.reduced, k).toBeTruthy();
    }
  });
  it("labels are sentence case words, not empty", () => {
    for (const k of kinds) expect(GLYPHS[k].label, k).toMatch(/^[A-Z][a-z ,:]+$/);
  });
});

describe("StatusGlyph", () => {
  for (const k of kinds) {
    it(`${k} renders an accessible svg with its label`, () => {
      const html = renderToStaticMarkup(<StatusGlyph kind={k} />);
      expect(html).toContain('role="img"');
      expect(html).toContain(`aria-label="${GLYPHS[k].label}"`);
      expect(html).toContain(`data-kind="${k}"`);
    });
    it(`${k} uses only currentColor or tokens`, () => {
      const html = renderToStaticMarkup(<StatusGlyph kind={k} size={12} />);
      expect(html).not.toMatch(/#[0-9a-f]{3,8}\b/i);
      expect(html).not.toMatch(/(fill|stroke)="(?!none|currentColor|var\()/);
    });
  }
  it("accepts a custom label", () => {
    expect(renderToStaticMarkup(<StatusGlyph kind="error" label="Stopped: rate limit" />)).toContain(
      'aria-label="Stopped: rate limit"',
    );
  });
  it("sizes to 12 or 16", () => {
    expect(renderToStaticMarkup(<StatusGlyph kind="idle" size={12} />)).toContain('width="12"');
    expect(renderToStaticMarkup(<StatusGlyph kind="idle" />)).toContain('width="16"');
  });
});

describe("AuxGlyph", () => {
  for (const k of Object.keys(AUX_LABELS) as AuxKind[])
    it(`${k} renders at 12 px with its label`, () => {
      const html = renderToStaticMarkup(<AuxGlyph kind={k} />);
      expect(html).toContain('width="12"');
      expect(html).toContain(`aria-label="${AUX_LABELS[k]}"`);
    });
});

describe("forced colours (final review)", () => {
  const css = readFileSync(new URL("./StatusGlyph.css", import.meta.url), "utf8");
  const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
  it("overrides every per-kind colour, not just the bare class", () => {
    expect(forced).toMatch(/\.status-glyph\[data-kind\]/);
    expect(forced).toMatch(/\.aux-glyph\[data-kind\]/);
  });
  it("keeps the badge's key and question mark visible on the Highlight fill", () => {
    expect(forced).toMatch(/\.g-mark[^{]*\{[^}]*HighlightText/);
    for (const k of ["needs-permission", "needs-question"] as const)
      expect(renderToStaticMarkup(<StatusGlyph kind={k} />)).toContain('class="g-mark');
  });
  it("cuts the error cross and caution bang out in Canvas", () => {
    expect(forced).toMatch(/\.g-cut[^{]*\{[^}]*Canvas/);
    expect(renderToStaticMarkup(<StatusGlyph kind="error" />)).toContain('class="g-cut');
    expect(renderToStaticMarkup(<AuxGlyph kind="caution" />)).toContain('class="g-cut');
  });
});

describe("StatusGlyph: minimal needs-you and error marks", () => {
  const mk = (k: Parameters<typeof StatusGlyph>[0]["kind"]) => renderToStaticMarkup(<StatusGlyph kind={k} />);
  it("shows a permission request as an accent badge with a bare exclamation mark", () => {
    const h = mk("needs-permission");
    expect(h).toContain("g-badge");
    expect(h).toContain("g-bang");
    expect(h).not.toContain("M7.75 8h4.5");
  });
  it("keeps the question mark in the same badge", () => {
    const h = mk("needs-question");
    expect(h).toContain("g-badge");
    expect(h).not.toContain("g-bang");
  });
  it("shows an error as a plain disc with a cut-out cross, not an octagon", () => {
    const h = mk("error");
    expect(h).toContain("g-error-disc");
    expect(h).not.toContain("<polygon");
  });
  it("uses one mark per glyph and no more than three shapes", () => {
    for (const k of ["needs-permission", "needs-question", "error"] as const) expect((mk(k).match(/<(path|circle|rect|polygon)/g) ?? []).length, k).toBeLessThanOrEqual(3);
  });
  it("flashes the error disc on arrival", () => {
    expect(readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8")).toContain(".g-error-disc");
  });
});

describe("StatusGlyph: vendor accent", () => {
  const css = readFileSync(new URL("./StatusGlyph.css", import.meta.url), "utf8");
  it("fills the needs-you badge with the vendor's signal colour, falling back to the accent", () => {
    for (const k of ["needs-permission", "needs-question"] as const) {
      const h = renderToStaticMarkup(<StatusGlyph kind={k} />);
      expect(h).toContain('fill="var(--agent-signal, var(--accent))"');
      expect(h).toContain('stroke="var(--agent-ink, var(--accent-ink))"');
    }
  });
  it("draws the error disc in the vendor's signal colour too", () => {
    expect(css).toMatch(/\.status-glyph\[data-kind="error"\]\s*\{[^}]*var\(--agent-signal, var\(--error\)\)/);
  });
});
