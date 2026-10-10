import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

describe("the numbering story", () => {
  const story = readFileSync("src/components/AgentMark/ManualCli.stories.tsx", "utf8");
  it("quiets a free number with the text-3 token, never with opacity (opacity drops text below 4.5:1 on black)", () => {
    expect(story).not.toMatch(/opacity\s*:/);
    expect(story).toContain('"var(--text-3)"');
  });
});
