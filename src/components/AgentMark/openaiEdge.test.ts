import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./agent-colors.css", import.meta.url), "utf8");

describe("OpenAI's yellow is outlined so it shows on light surfaces", () => {
  it("edges the mark, status glyphs, caution triangle and ring with the edge colour", () => {
    const rule = /\[data-agent="codex"\]\s*:is\(([^)]*)\)\s*\{([^}]*)\}/.exec(css);
    expect(rule).not.toBeNull();
    for (const part of [".agent-mark", ".status-glyph", ".aux-glyph", ".context-ring"]) expect(rule![1]).toContain(part);
    expect(rule![2]).toContain("var(--brand-openai-edge)");
    expect(rule![2]).toContain("drop-shadow");
  });
  it("gives OpenAI's badge dark ink in both themes, since white on bright yellow is unreadable", () => {
    expect(css).toMatch(/\[data-agent="codex"\]\s*\{[^}]*--agent-ink:\s*oklch\(22% 0 none\)/);
  });
});
