import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark } from "./AgentMark";
import { AMP_PIECES, MARK_PATHS } from "./agents";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("Amp's mark is three arrows that can move on their own", () => {
  it("splits into three shapes that rebuild the whole mark", () => {
    expect(AMP_PIECES.length).toBe(3);
    expect(AMP_PIECES.join(" ").replace(/\s+/g, " ")).toBe(MARK_PATHS.amp.replace(/\s+/g, " "));
  });
});

describe("a working Amp fires its two secondary arrows away from the main one", () => {
  const h = renderToStaticMarkup(<AgentMark agent="amp" working />);
  it("draws the three arrows apart, hidden from assistive technology", () => {
    expect((h.match(/class="agent-mark__piece"/g) ?? []).length).toBe(3);
    expect(h).toMatch(/class="agent-mark__pieces"[^>]*aria-hidden="true"/);
  });
  it("adds nothing when not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="amp" />)).not.toContain("agent-mark__piece");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__piece");
  });
  it("shows the three arrows instead of the whole mark only while motion is allowed", () => {
    expect(motion).toMatch(/\.agent-mark__pieces\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="amp"\] \.agent-mark__body\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="amp"\] \.agent-mark__pieces\s*\{[^}]*display:\s*inline/);
  });
  it("animates only the second and third arrows, so the main one never moves", () => {
    expect(motion).toMatch(/\.agent-mark__piece:nth-child\(n \+ 2\)\s*\{[^}]*animation:\s*mark-amp-shoot var\(--dur-loop\)/);
    expect(motion).not.toMatch(/\.agent-mark__piece\s*\{[^}]*animation/);
  });
  it("fires the middle arrow first and the outer one a beat later", () => {
    // Delay is (2 - i) * -14% of the loop: the arrow with the smaller number is further into its loop, so it fires sooner.
    expect(motion).toMatch(/animation-delay:\s*calc\(\(2 - var\(--i\)\) \* var\(--dur-loop\) \* -0\.14\)/);
  });
  it("shoots along the diagonal the arrows are stacked on, fades out, and slides back in from the main arrow", () => {
    const start = motion.indexOf("@keyframes mark-amp-shoot");
    const k = motion.slice(start, motion.indexOf("\n}\n", start));
    expect(k).toMatch(/30% \{ transform: translate\(7px, -7px\); opacity: 0; \}/);
    expect(k).toMatch(/31%, 58% \{ transform: translate\(-4\.4px, 4\.4px\); opacity: 0;/);
    expect(k).toMatch(/80%, 100% \{ transform: translate\(0, 0\); opacity: 1; \}/);
    expect(k).not.toMatch(/ease-in(?!-out)/);
  });
});
