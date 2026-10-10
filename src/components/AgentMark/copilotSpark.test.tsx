import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark } from "./AgentMark";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("a working Copilot mark crackles with electricity and does not move", () => {
  const h = renderToStaticMarkup(<AgentMark agent="copilot" working />);
  it("draws bolts, clipped to the shape of the mark so they stay inside it", () => {
    expect(h).toContain("agent-mark__sparks");
    expect((h.match(/class="agent-mark__bolt"/g) ?? []).length).toBeGreaterThanOrEqual(3);
    expect(h).toMatch(/<clipPath id="[^"]+"><path[^>]*clip-rule="evenodd"/);
    expect(h).toMatch(/class="agent-mark__sparks"[^>]*aria-hidden="true"/);
  });
  it("gives every instance its own clip id", () => {
    const two = renderToStaticMarkup(
      <>
        <AgentMark agent="copilot" working />
        <AgentMark agent="copilot" working />
      </>,
    );
    const ids = [...two.matchAll(/<clipPath id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(2);
  });
  it("adds nothing when not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="copilot" />)).not.toContain("agent-mark__sparks");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__sparks");
  });
  it("shows the bolts only while motion is allowed", () => {
    expect(motion).toMatch(/\.agent-mark__sparks\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="copilot"\] \.agent-mark__sparks\s*\{[^}]*display:\s*inline/);
  });
  it("moves nothing: the mark itself has no transform animation, only the bolts animate", () => {
    expect(motion).not.toMatch(/\[data-agent="copilot"\]\s*\{[^}]*animation/);
    expect(motion).toMatch(/\.agent-mark__bolt\s*\{[^}]*animation:\s*mark-zap var\(--dur-loop\)/);
    expect(motion).toContain("@keyframes mark-zap");
  });
});
