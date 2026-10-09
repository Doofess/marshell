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
  it("clamps out-of-range values", () => {
    expect(renderToStaticMarkup(<ContextRing pct={140} />)).toContain('aria-label="Context 100% used"');
    expect(renderToStaticMarkup(<ContextRing pct={-5} />)).toContain('aria-label="Context 0% used"');
  });
  it("picks tones at 80 and 95", () => {
    expect([ringTone(79.9), ringTone(80), ringTone(94.9), ringTone(95)]).toEqual(["neutral", "caution", "caution", "error"]);
  });
});
