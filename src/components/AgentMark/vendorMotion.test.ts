import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { AGENT_IDS } from "./agents";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");
const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");

/** Vendors that deliberately share one loop: Claude and Gemini breathe the same way, Qwen turns like Grok. */
const SHARED = [
  ["claude", "gemini"],
  ["grok", "qwen"],
];

/** Claude and Gemini share one breath: their tips draw in more than their centres (AgentMark.tsx). */
const BREATH = ["claude", "gemini"];

/** The `animation:` value of the rule for one vendor's working mark. */
function animationOf(id: string): string | null {
  // Copilot stays still; its loop is on the parts of the face that flash inside it (copilotSpark.test.tsx).
  // The custom agent keeps its frame and prompt still; its loop is on the underscore alone.
  if (id === "generic") return motion.match(/\.agent-mark__piece:nth-child\(3\)\s*\{[^}]*animation:\s*([^;]+);/)?.[1]?.trim() ?? null;
  // Amp's main arrow stays; its loop is on the two secondary arrows it fires away.
  if (id === "amp") return motion.match(/\.agent-mark__piece:nth-child\(n \+ 2\)\s*\{[^}]*animation:\s*([^;]+);/)?.[1]?.trim() ?? null;
  // Cursor's cube stays still; its loop is on the pointer cut out of it.
  if (id === "cursor") return motion.match(/\.agent-mark__pointer\s*\{[^}]*animation:\s*([^;]+);/)?.[1]?.trim() ?? null;
  if (id === "copilot") return motion.match(/\.agent-mark__zone\s*\{[^}]*animation:\s*([^;]+);/)?.[1]?.trim() ?? null;
  // Claude and Gemini draw their tips in and out around a centre that stays put, so the loop sits on the body of the mark and its layers (one rule, listed under both vendors).
  const target = BREATH.includes(id) ? String.raw` :is\(\.agent-mark__body, \.agent-mark__layer\)` : "";
  const re = new RegExp(String.raw`\.agent-mark\[data-working\]\[data-agent="${id}"\]${target}(?:,[^{]*)?\s*\{([^}]*)\}`);
  const body = motion.match(re)?.[1] ?? "";
  return body.match(/animation:\s*([^;]+);/)?.[1]?.trim() ?? null;
}

describe("a working session animates its vendor mark", () => {
  for (const id of AGENT_IDS)
    it(`${id} has its own loop`, () => {
      expect(animationOf(id), id).not.toBeNull();
    });
  it("gives every vendor a different animation", () => {
    const all = AGENT_IDS.map((id) => animationOf(id));
    expect(new Set(all).size).toBe(AGENT_IDS.length - SHARED.reduce((n, g) => n + g.length - 1, 0));
    for (const [first, ...rest] of SHARED) for (const id of rest) expect(animationOf(id), `${id} like ${first}`).toBe(animationOf(first!));
  });
  it("defines the keyframes each loop uses", () => {
    for (const id of AGENT_IDS) {
      const name = animationOf(id)!.split(/\s+/)[0]!;
      expect(motion, `${id} uses ${name}`).toContain(`@keyframes ${name}`);
    }
  });
  it("loops with duration tokens, never a literal time", () => {
    for (const id of AGENT_IDS) expect(animationOf(id), id).toMatch(/var\(--dur-loop(-slow)?\)/);
    expect(tokens).toMatch(/--dur-loop:\s*\d/);
    expect(tokens).toMatch(/--dur-loop-slow:\s*\d/);
  });
  it("never uses ease-in", () => {
    for (const id of AGENT_IDS) expect(animationOf(id), id).not.toMatch(/\bease-in\b(?!-out)/);
  });
  it("turns the mark about its own centre", () => {
    expect(motion).toMatch(/\.agent-mark\s*\{[^}]*transform-origin:\s*center/);
  });
});
