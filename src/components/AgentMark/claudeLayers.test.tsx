import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark, CLAUDE_LAYERS, CLAUDE_OUTER_SCALE } from "./AgentMark";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("Claude's tips draw in more than its centre", () => {
  it("draws in less the nearer the centre", () => {
    const scales = [CLAUDE_OUTER_SCALE, ...CLAUDE_LAYERS.map((l) => l.scale)];
    expect([...scales].sort((a, b) => a - b)).toEqual(scales);
    expect(new Set(scales).size).toBe(scales.length);
    expect(scales.every((s) => s > 0 && s < 1)).toBe(true);
  });
  it("keeps every layer's reach inside the layer beneath it, so no cut edge shows", () => {
    const reach = [12 * CLAUDE_OUTER_SCALE, ...CLAUDE_LAYERS.map((l) => l.radius * l.scale)];
    for (let i = 1; i < reach.length; i++) expect(reach[i]!, `layer ${i}`).toBeLessThan(reach[i - 1]!);
  });
  const h = renderToStaticMarkup(<AgentMark agent="claude" working />);
  it("draws a clipped copy per layer on top of the whole mark", () => {
    expect((h.match(/agent-mark__layer/g) ?? []).length).toBe(CLAUDE_LAYERS.length);
    expect((h.match(/<clipPath/g) ?? []).length).toBe(CLAUDE_LAYERS.length);
    expect(h.indexOf("agent-mark__layer")).toBeGreaterThan(h.indexOf("agent-mark__body"));
  });
  it("hides the layers from assistive technology and gives each instance its own clip ids", () => {
    expect(h).toMatch(/<g[^>]*aria-hidden="true"/);
    const two = renderToStaticMarkup(
      <>
        <AgentMark agent="claude" working />
        <AgentMark agent="claude" working />
      </>,
    );
    const ids = [...two.matchAll(/<clipPath id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(CLAUDE_LAYERS.length * 2);
  });
  it("adds nothing for a Claude session that is not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="claude" />)).not.toContain("agent-mark__layer");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__layer");
  });
  it("scales each layer by its own amount, about the centre of the mark", () => {
    expect(motion).toMatch(/scale\(var\(--s/);
    expect(motion).toMatch(/agent-mark__layer[^{]*\{[^}]*transform-origin:\s*12px 12px/);
  });
});
