import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { AGENT_IDS } from "./agents";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const motion = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8");
const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");

/** The `animation:` value of the rule for one vendor's working mark. */
function animationOf(id: string): string | null {
  // Claude moves its twelve rays, not the whole mark (claudeRays.test.tsx).
  if (id === "claude") return motion.match(/\.agent-mark__ray\s*\{[^}]*animation:\s*([^;]+);/)?.[1]?.trim() ?? null;
  const re = new RegExp(String.raw`\.agent-mark\[data-working\]\[data-agent="${id}"\]\s*\{([^}]*)\}`);
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
    expect(new Set(all).size).toBe(AGENT_IDS.length);
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
