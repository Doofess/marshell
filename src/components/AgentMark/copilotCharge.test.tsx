import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark } from "./AgentMark";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("a working Copilot mark flashes dark in parts, as if a current ran through it, and does not move", () => {
  const h = renderToStaticMarkup(<AgentMark agent="copilot" working />);
  it("draws six zones of the face, clipped to the shape of the mark so they stay inside it", () => {
    expect(h).toContain("agent-mark__charge");
    expect((h.match(/class="agent-mark__zone"/g) ?? []).length).toBe(6);
    expect(h).toMatch(/<clipPath id="[^"]+"><path[^>]*clip-rule="evenodd"/);
    expect(h).toMatch(/class="agent-mark__charge"[^>]*aria-hidden="true"/);
  });
  it("numbers the zones so the current runs from one end to the other", () => {
    const order = [...h.matchAll(/--i:(\d+)/g)].map((m) => Number(m[1]));
    expect(order).toEqual([5, 4, 3, 2, 1, 0]);
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
    expect(renderToStaticMarkup(<AgentMark agent="copilot" />)).not.toContain("agent-mark__charge");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__charge");
  });
  it("shows the zones only while motion is allowed, in a darker shade of the mark's own colour", () => {
    expect(motion).toMatch(/\.agent-mark__charge\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="copilot"\] \.agent-mark__charge\s*\{[^}]*display:\s*inline/);
    expect(motion).toMatch(/\.agent-mark__zone\s*\{[^}]*fill:\s*color-mix\(in oklch, currentColor \d+%, black\)/);
  });
  it("moves nothing: the mark itself has no animation, only the zones flash", () => {
    expect(motion).not.toMatch(/\[data-agent="copilot"\]\s*\{[^}]*animation/);
    expect(motion).toMatch(/\.agent-mark__zone\s*\{[^}]*animation:\s*mark-zap var\(--dur-loop\)/);
    const k = motion.slice(motion.indexOf("@keyframes mark-zap"), motion.indexOf("\n}\n", motion.indexOf("@keyframes mark-zap")));
    expect(k).not.toContain("transform");
  });
});
