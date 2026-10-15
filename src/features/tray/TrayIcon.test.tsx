import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { TrayIcon } from "./TrayIcon";

const tokens = parseTokens(readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8"), ":root");

describe("TrayIcon", () => {
  it("is the mono marshalling wands, drawn as strokes in the current colour, at the size asked for", () => {
    for (const size of [16, 32] as const) {
      const h = renderToStaticMarkup(<TrayIcon size={size} />);
      expect(h).toContain('viewBox="0 0 1024 1024"');
      expect(h).toContain(`width="${size}"`);
      expect(h).toContain(`height="${size}"`);
      expect(h.match(/<line /g)).toHaveLength(2);
      expect(h).toContain('stroke="currentColor"');
      expect(h).toContain('stroke-width="170"');
    }
  });
  it("adds one accent dot when sessions are waiting, and is named with the count", () => {
    const h = renderToStaticMarkup(<TrayIcon size={32} waiting={2} />);
    expect(h.match(/<circle /g)).toHaveLength(1);
    expect(h).toContain('fill="var(--accent)"');
    expect(h).toContain('role="img"');
    expect(h).toContain('aria-label="Marshell, 2 need you"');
    expect(renderToStaticMarkup(<TrayIcon size={32} waiting={1} />)).toContain('aria-label="Marshell, 1 needs you"');
    expect(renderToStaticMarkup(<TrayIcon size={16} />)).toContain('aria-label="Marshell"');
    expect(renderToStaticMarkup(<TrayIcon size={16} />).match(/<circle /g)).toBeNull();
  });
  it("hides the drawing from assistive technology (the wrapper carries the name) and has no live region", () => {
    const h = renderToStaticMarkup(<TrayIcon size={16} waiting={3} />);
    expect(h).toMatch(/<svg[^>]*aria-hidden="true"/);
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  it("holds 3:1 against the light and the dark panel behind it (non-text contrast)", () => {
    for (const theme of ["light", "dark"] as const) expect(contrast(tokens["--text-1"]![theme], tokens["--bg-raised"]![theme])).toBeGreaterThanOrEqual(3);
  });
});
