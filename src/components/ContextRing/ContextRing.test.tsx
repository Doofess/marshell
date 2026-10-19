import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ContextRing, ringTone } from "./ContextRing";

describe("ContextRing", () => {
  it("renders nothing when unknown", () => {
    expect(renderToStaticMarkup(<ContextRing pct={undefined} />)).toBe("");
    expect(renderToStaticMarkup(<ContextRing pct={Number.NaN} />)).toBe("");
  });
  it("labels the percentage for screen readers", () => {
    expect(renderToStaticMarkup(<ContextRing pct={42} />)).toContain('aria-label="Context 42% used"');
  });
  it("names what it measures when told", () => {
    expect(renderToStaticMarkup(<ContextRing pct={42} label="5-hour plan" />)).toContain('aria-label="5-hour plan 42% used"');
  });
  it("clamps out-of-range values", () => {
    expect(renderToStaticMarkup(<ContextRing pct={140} />)).toContain('aria-label="Context 100% used"');
    expect(renderToStaticMarkup(<ContextRing pct={-5} />)).toContain('aria-label="Context 0% used"');
  });
  it("picks tones at 70 and 90", () => {
    expect([ringTone(69.9), ringTone(70), ringTone(89.9), ringTone(90)]).toEqual(["neutral", "caution", "caution", "error"]);
  });
  it("sets data-tone in the markup at 69, 70 and 90", () => {
    const tone = (pct: number) => /data-tone="(\w+)"/.exec(renderToStaticMarkup(<ContextRing pct={pct} />))?.[1];
    expect([tone(69), tone(70), tone(90)]).toEqual(["neutral", "caution", "error"]);
  });
  describe("colour", () => {
    // Read from disk: Vitest hands CSS imports to tests as empty strings.
    const css = readFileSync(new URL("./ContextRing.css", import.meta.url), "utf8");
    const rule = (selector: string) => {
      const i = css.indexOf(`${selector} {`);
      return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
    };
    it("is green below 70%, yellow from 70% and red from 90%, by the status tokens", () => {
      expect(rule('.context-ring[data-tone="neutral"] .context-ring__value')).toContain("var(--ok)");
      expect(rule('.context-ring[data-tone="caution"] .context-ring__value')).toContain("var(--caution)");
      expect(rule('.context-ring[data-tone="error"] .context-ring__value')).toContain("var(--error)");
    });
    it("colours the centre dot red", () => {
      expect(rule(".context-ring__dot")).toContain("var(--error)");
    });
    it("is not the vendor's colour any more", () => {
      expect(css).not.toContain("--agent-signal");
    });
    it("draws ring and dot in CanvasText in forced colours", () => {
      const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
      expect(forced).toContain("CanvasText");
      expect(forced).toContain(".context-ring__dot");
    });
  });
});
