import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark } from "./AgentMark";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

/** The one keyframe between 0% and 100% is the peak of the stroke. */
function peakOf(name: string): number {
  const start = motion.indexOf(`@keyframes ${name}`);
  const block = motion.slice(start, motion.indexOf("\n}\n", start));
  const steps = [...block.matchAll(/^ {2}(\d+)% \{/gm)].map((m) => Number(m[1]));
  return steps.find((n) => n > 0 && n < 100) ?? NaN;
}

describe("a working Antigravity mark swims like a jellyfish", () => {
  const h = renderToStaticMarkup(<AgentMark agent="antigravity" working />);
  it("is drawn as a bell and tentacles, each in its own clipped group", () => {
    expect(h).toContain("agent-mark__bell");
    expect(h).toContain("agent-mark__tentacles");
    expect((h.match(/<clipPath/g) ?? []).length).toBe(2);
    expect(h).toMatch(/class="agent-mark__parts"[^>]*aria-hidden="true"/);
  });
  it("gives every instance its own clip ids", () => {
    const two = renderToStaticMarkup(
      <>
        <AgentMark agent="antigravity" working />
        <AgentMark agent="antigravity" working />
      </>,
    );
    const ids = [...two.matchAll(/<clipPath id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(4);
  });
  it("adds nothing when not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="antigravity" />)).not.toContain("agent-mark__bell");
    expect(renderToStaticMarkup(<AgentMark agent="claude" working />)).not.toContain("agent-mark__bell");
  });
  it("shows the parts instead of the whole mark only while motion is allowed", () => {
    expect(motion).toMatch(/\.agent-mark__parts\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="antigravity"\] \.agent-mark__body\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="antigravity"\] \.agent-mark__parts\s*\{[^}]*display:\s*inline/);
  });
  it("has the tentacles trail the bell: they peak later in the stroke", () => {
    expect(peakOf("mark-jelly-bell")).toBeGreaterThan(0);
    expect(peakOf("mark-jelly-tentacles")).toBeGreaterThan(peakOf("mark-jelly-bell"));
  });
});
