import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark } from "./AgentMark";
import { GENERIC_PIECES, MARK_PATHS } from "./agents";

const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");

describe("the custom agent's mark is a frame, a chevron and an underscore", () => {
  it("splits into three shapes that rebuild the whole mark, the underscore last", () => {
    expect(GENERIC_PIECES.length).toBe(3);
    expect(GENERIC_PIECES.join("").replace(/\s+/g, "")).toBe(MARK_PATHS.generic.replace(/\s+/g, ""));
    expect(GENERIC_PIECES[2]).toMatch(/^M12 14\.2h5\.5v1\.4H12v-1\.4z$/);
  });
});

describe("a working custom agent blinks only its underscore, like an old terminal cursor", () => {
  const h = renderToStaticMarkup(<AgentMark agent="generic" working />);
  it("draws the three shapes apart, hidden from assistive technology", () => {
    expect((h.match(/class="agent-mark__piece"/g) ?? []).length).toBe(3);
    expect(h).toMatch(/class="agent-mark__pieces"[^>]*aria-hidden="true"/);
  });
  it("adds nothing when not working", () => {
    expect(renderToStaticMarkup(<AgentMark agent="generic" />)).not.toContain("agent-mark__piece");
  });
  it("shows the shapes instead of the whole mark only while motion is allowed", () => {
    expect(motion).toMatch(/\[data-agent="generic"\] \.agent-mark__body\s*\{[^}]*display:\s*none/);
    expect(motion).toMatch(/\[data-agent="generic"\] \.agent-mark__pieces\s*\{[^}]*display:\s*inline/);
  });
  it("animates the underscore alone, in hard steps, never fading", () => {
    expect(motion).toMatch(/\.agent-mark__piece:nth-child\(3\)\s*\{[^}]*animation:\s*mark-gen-blink var\(--dur-loop\) steps\(1, end\) infinite/);
    expect(motion).not.toMatch(/\[data-agent="generic"\][^{]*\.agent-mark__piece:nth-child\([12]\)/);
    const start = motion.indexOf("@keyframes mark-gen-blink");
    const k = motion.slice(start, motion.indexOf("\n}\n", start));
    const values = [...k.matchAll(/opacity:\s*([\d.]+)/g)].map((m) => Number(m[1]));
    expect(values.every((v) => v === 0 || v === 1)).toBe(true);
  });
  it("blinks twice a loop, at a steady cadence", () => {
    const start = motion.indexOf("@keyframes mark-gen-blink");
    const k = motion.slice(start, motion.indexOf("\n}\n", start));
    expect([...k.matchAll(/^ {2}(\d+)%/gm)].map((m) => Number(m[1]))).toEqual([0, 25, 50, 75]);
  });
});
