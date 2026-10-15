import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark, BREATHING } from "./AgentMark";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("the tips of Claude and Gemini draw in more than their centres", () => {
  it("covers Claude and Gemini", () => {
    expect(Object.keys(BREATHING).sort()).toEqual(["claude", "gemini"]);
  });
  const agents = Object.keys(BREATHING) as ("claude" | "gemini")[];
  for (const agent of agents) {
    const { outer, layers } = BREATHING[agent]!;
    it(`draws ${agent} in less the nearer the centre`, () => {
      const scales = [outer, ...layers.map((l) => l.scale)];
      expect([...scales].sort((a, b) => a - b)).toEqual(scales);
      expect(new Set(scales).size).toBe(scales.length);
      expect(scales.every((s) => s > 0 && s < 1)).toBe(true);
    });
    it(`keeps every ${agent} layer's reach inside the layer beneath it, so no cut edge shows`, () => {
      const reach = [12 * outer, ...layers.map((l) => l.radius * l.scale)];
      for (let i = 1; i < reach.length; i++) expect(reach[i]!, `layer ${i}`).toBeLessThan(reach[i - 1]!);
    });
  }
  for (const agent of ["claude", "gemini"] as const)
    it(`draws a clipped copy per layer on top of the whole ${agent} mark`, () => {
      const h = renderToStaticMarkup(<AgentMark agent={agent} working />);
      expect((h.match(/agent-mark__layer/g) ?? []).length).toBe(BREATHING[agent]!.layers.length);
      expect((h.match(/<clipPath/g) ?? []).length).toBe(BREATHING[agent]!.layers.length);
      expect(h.indexOf("agent-mark__layer")).toBeGreaterThan(h.indexOf("agent-mark__body"));
    });
  const h = renderToStaticMarkup(<AgentMark agent="claude" working />);
  it("hides the layers from assistive technology and gives each instance its own clip ids", () => {
    expect(h).toMatch(/<g[^>]*aria-hidden="true"/);
    const two = renderToStaticMarkup(
      <>
        <AgentMark agent="claude" working />
        <AgentMark agent="claude" working />
      </>,
    );
    const ids = [...two.matchAll(/<clipPath id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(BREATHING.claude!.layers.length * 2);
  });
  it("adds nothing for a session that is not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="claude" />)).not.toContain("agent-mark__layer");
    expect(renderToStaticMarkup(<AgentMark agent="gemini" />)).not.toContain("agent-mark__layer");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__layer");
  });
  it("animates Gemini the same way as Claude", () => {
    expect(motion).toMatch(/data-agent="gemini"\] :is\(\.agent-mark__body, \.agent-mark__layer\)/);
  });
  it("scales each layer by its own amount, about the centre of the mark", () => {
    expect(motion).toMatch(/scale\(var\(--s/);
    expect(motion).toMatch(/agent-mark__layer[^{]*\{[^}]*transform-origin:\s*12px 12px/);
  });
});
