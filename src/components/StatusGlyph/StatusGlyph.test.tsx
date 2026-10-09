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
