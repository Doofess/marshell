import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark } from "./AgentMark";
import { CLAUDE_RAY_ANGLES, CLAUDE_VALLEYS, CLAUDE_WEDGES, wedgeOf } from "./claudeRays";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("Claude's twelve rays, measured from the real logo", () => {
  it("has twelve rays and twelve gaps, in order around the circle", () => {
    expect(CLAUDE_RAY_ANGLES.length).toBe(12);
    expect(CLAUDE_VALLEYS.length).toBe(12);
    expect([...CLAUDE_VALLEYS].sort((a, b) => a - b)).toEqual(CLAUDE_VALLEYS);
  });
  it("cuts the circle into twelve wedges along the gaps, one ray in each", () => {
    expect(CLAUDE_WEDGES.length).toBe(12);
    CLAUDE_RAY_ANGLES.forEach((a, i) => expect(wedgeOf(a), `ray ${a}`).toBe(i));
  });
  it("puts every direction in exactly one wedge, so the pieces rebuild the logo without gaps or overlap", () => {
    for (let deg = 0; deg < 360; deg += 0.5) expect(wedgeOf(deg), `${deg}°`).toBeGreaterThanOrEqual(0);
    const counts = new Set<number>();
    for (let deg = 0; deg < 360; deg += 0.5) counts.add(wedgeOf(deg));
    expect(counts.size).toBe(12);
  });
  it("draws each wedge as a polygon from the centre to the outside of the mark", () => {
    for (const w of CLAUDE_WEDGES) {
      const pts = w.split(" ");
      expect(pts[0]).toBe("12,12");
      expect(pts.length).toBeGreaterThanOrEqual(4);
      for (const p of pts.slice(1)) {
        const [x, y] = p.split(",").map(Number) as [number, number];
        expect(Math.hypot(x - 12, y - 12)).toBeGreaterThan(13);
      }
    }
  });
});

describe("AgentMark for a working Claude session", () => {
  const h = renderToStaticMarkup(<AgentMark agent="claude" working />);
  it("keeps the whole logo for stillness and adds twelve rays for motion", () => {
    expect(h).toContain("agent-mark__still");
    expect(h).toContain("agent-mark__rays");
    expect((h.match(/class="agent-mark__ray"/g) ?? []).length).toBe(12);
  });
  it("numbers the rays clockwise so the wave can travel", () => {
    for (let i = 0; i < 12; i++) expect(h).toContain(`--i:${i}`);
  });
  it("clips each ray to its wedge on a static outer group, so the clip never moves", () => {
    expect((h.match(/clip-path="url\(#/g) ?? []).length).toBe(12);
    expect((h.match(/<clipPath/g) ?? []).length).toBe(12);
  });
  it("gives every instance its own clip ids", () => {
    const two = renderToStaticMarkup(
      <>
        <AgentMark agent="claude" working />
        <AgentMark agent="claude" working />
      </>,
    );
    const ids = [...two.matchAll(/<clipPath id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(24);
  });
  it("hides the rays from assistive technology", () => {
    expect(h).toMatch(/class="agent-mark__rays"[^>]*aria-hidden="true"/);
  });
  it("adds nothing for a Claude session that is not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="claude" />)).not.toContain("agent-mark__rays");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__rays");
  });
});

describe("motion.css for the rays", () => {
  it("shows the rays instead of the still logo only while working with motion allowed", () => {
    expect(motion).toMatch(/\.agent-mark__rays\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="claude"\] \.agent-mark__still\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="claude"\] \.agent-mark__rays\s*\{[^}]*display:\s*inline/);
  });
  it("pulses each ray in a wave, delayed by its number", () => {
    expect(motion).toMatch(/\.agent-mark__ray\s*\{[^}]*animation:\s*mark-ray var\(--dur-loop\)/);
    expect(motion).toMatch(/animation-delay:\s*calc\(var\(--i\) \* var\(--dur-loop\) \/ -12\)/);
    expect(motion).toContain("@keyframes mark-ray");
  });
  it("scales rays about the centre of the 24-unit mark and never grows them past the logo", () => {
    expect(motion).toMatch(/\.agent-mark__ray\s*\{[^}]*transform-origin:\s*12px 12px/);
    const k = motion.slice(motion.indexOf("@keyframes mark-ray"), motion.indexOf("}\n}", motion.indexOf("@keyframes mark-ray")));
    expect(k).not.toMatch(/scale\((1\.[1-9]|[2-9])/);
  });
});
