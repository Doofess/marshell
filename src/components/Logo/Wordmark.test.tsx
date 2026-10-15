import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Wordmark } from "./Wordmark";
import { WORDMARK } from "./wordmarkPath";

describe("Wordmark", () => {
  it("draws the name as outlined letters in the colour of the text around it", () => {
    const h = renderToStaticMarkup(<Wordmark />);
    expect(h).toContain(`viewBox="${WORDMARK.viewBox}"`);
    expect(h).toContain(`d="${WORDMARK.d}"`);
    expect(h).toContain('fill="currentColor"');
    expect(h).not.toMatch(/<text/);
  });
  it("is a real outline of eight letters: the path has at least eight outlines (letters, plus the counters inside a and r)", () => {
    expect((WORDMARK.d.match(/M/g) ?? []).length).toBeGreaterThanOrEqual(8);
  });
  it("keeps its proportions from the height", () => {
    const [, , w, h] = WORDMARK.viewBox.split(" ").map(Number);
    const out = renderToStaticMarkup(<Wordmark height={14} />);
    expect(out).toContain('height="14"');
    expect(out).toContain(`width="${Math.round((14 * w!) / h! * 100) / 100}"`);
  });
  it("is named Marshell, or hidden when the control around it already says so", () => {
    expect(renderToStaticMarkup(<Wordmark />)).toContain('aria-label="Marshell"');
    const d = renderToStaticMarkup(<Wordmark decorative />);
    expect(d).toContain('aria-hidden="true"');
    expect(d).not.toContain("aria-label");
  });
  it("carries no literal colour", () => {
    expect(renderToStaticMarkup(<Wordmark />)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
  });
});
